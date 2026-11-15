//! Pedidos por WhatsApp (bot "Nexi").
//!
//! - `POST /v1/whatsapp/webhook` recibe los mensajes de Meta (firma verificada),
//!   responde con el motor de conversación y registra el pedido.
//! - `/v1/whatsapp/orders` es la bandeja del equipo: preparar, despachar,
//!   entregar o cancelar avisa al cliente por WhatsApp.
//! - `/v1/whatsapp/bot` configura el número, horario, zonas y opciones por sede.

pub mod cloud;
pub mod engine;
pub mod text;

#[cfg(test)]
mod db_tests;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::{AuthUser, Claims};
use crate::error::{self, ApiError, DomainError};
use crate::routes::{Body, Path, Query};
use crate::tenancy::{self, Action};
use crate::{AppState, catalog, db, outbox};

use engine::{
    BotConfig, CartLine, Context, Conversation, Input, OrderStatus, Outcome, PaymentMethod, Reply,
    Step, Zone,
};
use text::Product;

type Tx<'a> = Transaction<'a, Postgres>;
type ApiResult<T> = Result<T, ApiError>;

/// Una conversación sin mensajes por este tiempo empieza de cero.
const CONVERSATION_TTL_HOURS: i64 = 12;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/whatsapp/webhook", get(webhook_verify).post(webhook))
        .route("/v1/whatsapp/orders", get(list_orders))
        .route("/v1/whatsapp/orders/{id}", get(get_order))
        .route("/v1/whatsapp/orders/{id}/status", post(set_status))
        .route("/v1/whatsapp/orders/{id}/paid", post(mark_paid))
        .route("/v1/whatsapp/bot", get(get_bot).put(put_bot))
        .route("/v1/whatsapp/bot/qr", get(get_qr))
}

/// Transacción del sistema para un negocio y sede ya resueltos (sin usuario).
async fn system_tx(pool: &PgPool, tenant: Uuid, branch: Uuid) -> Result<Tx<'_>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("set local role authenticated")
        .execute(&mut *tx)
        .await?;
    sqlx::query("select set_config('request.jwt.claims', '', true)")
        .execute(&mut *tx)
        .await?;
    db::set_scope(&mut tx, tenant, Some(branch)).await?;
    Ok(tx)
}

// ------------------------------------------------------------------ webhook

#[derive(Deserialize)]
struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

/// Verificación del webhook al configurarlo en Meta.
async fn webhook_verify(axum::extract::Query(q): axum::extract::Query<VerifyQuery>) -> Response {
    let Some(env) = cloud::env() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if q.mode.as_deref() == Some("subscribe")
        && q.token.as_deref() == Some(env.verify_token.as_str())
    {
        (StatusCode::OK, q.challenge.unwrap_or_default()).into_response()
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

async fn webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(env) = cloud::env() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let sig = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    if !cloud::verify_signature(&env.app_secret, &body, sig) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    for m in cloud::parse_webhook(&v) {
        match process(&state.pool, &m).await {
            Ok(Some((pnid, replies))) => cloud::send_all(env, &pnid, &m.from, &replies).await,
            Ok(None) => {}
            Err(e) => {
                // 500 hace que Meta reintente; el id del mensaje evita duplicados.
                tracing::error!(error = %e, message = %m.wa_message_id, "falló un mensaje de WhatsApp");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    StatusCode::OK.into_response()
}

#[derive(sqlx::FromRow)]
struct BotRow {
    phone_number_id: String,
    business_name: String,
    opens_at: NaiveTime,
    closes_at: NaiveTime,
    min_order: Decimal,
    prepay_enabled: bool,
    usual_enabled: bool,
    substitutes_enabled: bool,
    age_check: bool,
}

impl BotRow {
    fn config(&self) -> BotConfig {
        BotConfig {
            business_name: self.business_name.clone(),
            opens_at: self.opens_at,
            closes_at: self.closes_at,
            min_order: self.min_order,
            prepay_enabled: self.prepay_enabled,
            usual_enabled: self.usual_enabled,
            substitutes_enabled: self.substitutes_enabled,
            age_check: self.age_check,
        }
    }
}

const BOT_COLS: &str = "phone_number_id, business_name, opens_at, closes_at, min_order, prepay_enabled, usual_enabled, substitutes_enabled, age_check";

#[derive(sqlx::FromRow)]
struct CatalogRow {
    product_id: Uuid,
    name: String,
    category: Option<String>,
    price: Option<Decimal>,
    tax_code: Option<String>,
    available: Option<Decimal>,
}

/// Catálogo de la sede con precio final (con impuesto, redondeado al peso).
async fn load_catalog(tx: &mut Tx<'_>, branch: Uuid) -> Result<Vec<Product>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CatalogRow>(catalog::CATALOG_SQL)
        .bind(branch)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let base = r.price?;
            let rate = catalog::tax_rate_for(r.tax_code.as_deref()).unwrap_or(Decimal::ZERO);
            Some(Product {
                id: r.product_id,
                name: r.name,
                category: r.category,
                price: (base * (Decimal::ONE + rate)).round(),
                available: r.available.map(|a| a.floor().try_into().unwrap_or(0i64)),
            })
        })
        .collect())
}

/// Procesa un mensaje. Devuelve el número de la sede y las respuestas a
/// enviar, o `None` si no hay nada que responder (duplicado o sin bot).
pub(crate) async fn process(
    pool: &PgPool,
    m: &cloud::Inbound,
) -> anyhow::Result<Option<(String, Vec<Reply>)>> {
    let route: Option<(Uuid, Uuid)> =
        sqlx::query_as("select tenant_id, branch_id from private.whatsapp_bot_route($1)")
            .bind(&m.phone_number_id)
            .fetch_optional(pool)
            .await?;
    let Some((tenant, branch)) = route else {
        tracing::warn!(phone_number_id = %m.phone_number_id, "mensaje para un número sin bot activo");
        return Ok(None);
    };
    let from: String = m.from.chars().filter(char::is_ascii_digit).collect();
    let mut tx = system_tx(pool, tenant, branch).await?;

    let (conv_id, state, updated_at): (Uuid, serde_json::Value, DateTime<Utc>) = sqlx::query_as(
        "insert into whatsapp_conversations (tenant_id, branch_id, wa_phone, contact_name)
         values ($1, $2, $3, $4)
         on conflict (branch_id, wa_phone) do update
            set contact_name = coalesce(excluded.contact_name, whatsapp_conversations.contact_name)
         returning id, state, updated_at",
    )
    .bind(tenant)
    .bind(branch)
    .bind(&from)
    .bind(&m.contact_name)
    .fetch_one(&mut *tx)
    .await?;

    let fresh: Option<Uuid> = sqlx::query_scalar(
        "insert into whatsapp_messages (tenant_id, conversation_id, direction, wa_message_id, body)
         values ($1, $2, 'IN', $3, $4)
         on conflict (wa_message_id) do nothing
         returning id",
    )
    .bind(tenant)
    .bind(conv_id)
    .bind(&m.wa_message_id)
    .bind(&m.body)
    .fetch_optional(&mut *tx)
    .await?;
    if fresh.is_none() {
        tx.commit().await?;
        return Ok(None); // reintento de Meta: ya se respondió
    }

    let bot: BotRow = sqlx::query_as(&format!(
        "select {BOT_COLS} from whatsapp_bots where branch_id = $1 and active"
    ))
    .bind(branch)
    .fetch_one(&mut *tx)
    .await?;
    let cfg = bot.config();

    let mut conv: Conversation = serde_json::from_value(state).unwrap_or_default();
    if Utc::now() - updated_at > chrono::Duration::hours(CONVERSATION_TTL_HOURS) {
        conv = Conversation {
            address: conv.address,
            zone_id: conv.zone_id,
            ..Default::default()
        };
    }

    let replies_and_outcome = if matches!(&m.input, Input::Text(t) if t.trim().is_empty())
        && conv.step != Step::Human
    {
        (conv.clone(), vec![Reply::Text("Por ahora solo leo mensajes escritos. Escribime tu pedido, por ejemplo: «2 Águila y 1 hielo».".into())], Outcome::None)
    } else {
        let zones: Vec<Zone> = sqlx::query_as::<_, (Uuid, String, Decimal, i32, i32)>(
            "select id, name, fee, eta_min, eta_max from whatsapp_delivery_zones where branch_id = $1 and active order by name",
        )
        .bind(branch)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(id, name, fee, eta_min, eta_max)| Zone { id, name, fee, eta_min, eta_max })
        .collect();
        let products = load_catalog(&mut tx, branch).await?;
        let customer: Option<(Uuid, String)> = sqlx::query_as(
            "select id, name from customers
              where archived_at is null and phone is not null
                and (phone = $1 or '57' || phone = $1)
              order by created_at limit 1",
        )
        .bind(&from)
        .fetch_optional(&mut *tx)
        .await?;
        let last: Option<serde_json::Value> = sqlx::query_scalar(
            "select items from whatsapp_orders where branch_id = $1 and wa_phone = $2 and status <> 'CANCELADO'
              order by created_at desc limit 1",
        )
        .bind(branch)
        .bind(&from)
        .fetch_optional(&mut *tx)
        .await?;
        let usual: Vec<CartLine> = last
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let saved: Option<(String, Option<Uuid>)> = sqlx::query_as(
            "select address, zone_id from whatsapp_orders
              where branch_id = $1 and wa_phone = $2 and not pickup and address is not null
              order by created_at desc limit 1",
        )
        .bind(branch)
        .bind(&from)
        .fetch_optional(&mut *tx)
        .await?;
        // Hora local de la sede (cada sede tiene su zona horaria).
        let now: NaiveTime = sqlx::query_scalar(
            "select (now() at time zone timezone)::time from branches where id = $1",
        )
        .bind(branch)
        .fetch_one(&mut *tx)
        .await?;
        let name = customer
            .as_ref()
            .map(|c| c.1.as_str())
            .or(m.contact_name.as_deref());
        let ctx = Context {
            cfg: &cfg,
            catalog: &products,
            zones: &zones,
            customer_name: name,
            usual: Some(&usual),
            saved_address: saved.as_ref().map(|(a, z)| (a.as_str(), *z)),
            now,
        };
        let (c, r, o) = engine::handle(&conv, &m.input, &ctx);
        if let Some((cid, _)) = customer {
            sqlx::query("update whatsapp_conversations set customer_id = $2 where id = $1")
                .bind(conv_id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        }
        (c, r, o)
    };
    let (conv, mut replies, outcome) = replies_and_outcome;

    if let Outcome::PlaceOrder(draft) = &outcome {
        let n: i32 = sqlx::query_scalar(
            "update whatsapp_bots set next_order_number = next_order_number + 1, updated_at = now()
              where branch_id = $1 returning next_order_number - 1",
        )
        .bind(branch)
        .fetch_one(&mut *tx)
        .await?;
        let code = format!("W-{n:04}");
        let order_id: Uuid = sqlx::query_scalar(
            "insert into whatsapp_orders (tenant_id, branch_id, code, conversation_id, customer_id, wa_phone, contact_name,
                items, subtotal, delivery_fee, total, pickup, address, zone_id, age_confirmed, payment_method)
             select $1, $2, $3, $4, c.customer_id, $5, c.contact_name, $6, $7, $8, $9, $10, $11, $12, $13, $14
               from whatsapp_conversations c where c.id = $4
             returning id",
        )
        .bind(tenant)
        .bind(branch)
        .bind(&code)
        .bind(conv_id)
        .bind(&from)
        .bind(serde_json::to_value(&draft.lines)?)
        .bind(draft.subtotal)
        .bind(draft.delivery_fee)
        .bind(draft.total)
        .bind(draft.pickup)
        .bind(&draft.address)
        .bind(draft.zone_id)
        .bind(conv.age_confirmed)
        .bind(payment_code(draft.payment))
        .fetch_one(&mut *tx)
        .await?;
        if draft.payment == PaymentMethod::Link {
            // El worker de outbox genera el link de pago (Bold) y lo envía.
            outbox::enqueue_tx(
                &mut tx,
                tenant,
                "WHATSAPP_PAYMENT_LINK",
                serde_json::json!({ "order_id": order_id, "code": code, "total": draft.total, "wa_phone": from, "phone_number_id": bot.phone_number_id }),
            )
            .await
            .map_err(|e| anyhow::anyhow!("outbox: {e:?}"))?;
        }
        replies.push(Reply::Text(engine::confirmation_text(&code, draft)));
    }

    sqlx::query("update whatsapp_conversations set state = $2, needs_human = $3, updated_at = now() where id = $1")
        .bind(conv_id)
        .bind(serde_json::to_value(&conv)?)
        .bind(conv.step == Step::Human)
        .execute(&mut *tx)
        .await?;
    for r in &replies {
        sqlx::query("insert into whatsapp_messages (tenant_id, conversation_id, direction, body) values ($1, $2, 'OUT', $3)")
            .bind(tenant)
            .bind(conv_id)
            .bind(cloud::reply_text(r))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Some((bot.phone_number_id, replies)))
}

fn payment_code(p: PaymentMethod) -> &'static str {
    match p {
        PaymentMethod::Link => "LINK",
        PaymentMethod::Efectivo => "EFECTIVO",
        PaymentMethod::Datafono => "DATAFONO",
    }
}

// ------------------------------------------------------------------ bandeja

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WaOrder {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub code: String,
    pub customer_id: Option<Uuid>,
    pub wa_phone: String,
    pub contact_name: Option<String>,
    pub items: serde_json::Value,
    pub subtotal: Decimal,
    pub delivery_fee: Decimal,
    pub total: Decimal,
    pub pickup: bool,
    pub address: Option<String>,
    pub zone_name: Option<String>,
    pub age_confirmed: bool,
    pub payment_method: String,
    pub payment_status: String,
    pub payment_reference: Option<String>,
    pub status: String,
    pub courier_name: Option<String>,
    pub sale_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const ORDER_SELECT: &str = "select o.id, o.branch_id, o.code, o.customer_id, o.wa_phone, o.contact_name, o.items,
       o.subtotal, o.delivery_fee, o.total, o.pickup, o.address, z.name as zone_name, o.age_confirmed,
       o.payment_method, o.payment_status, o.payment_reference, o.status, o.courier_name, o.sale_id,
       o.created_at, o.updated_at
  from whatsapp_orders o
  left join whatsapp_delivery_zones z on z.id = o.zone_id";

#[derive(Deserialize)]
struct ListQuery {
    branch_id: Uuid,
    status: Option<String>,
}

async fn list_orders(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<WaOrder>>> {
    let (scope, mut tx) = tenancy::begin_in_branch(&state.pool, &user.claims, q.branch_id).await?;
    scope.require(Action::Read)?;
    if q.status
        .as_deref()
        .is_some_and(|s| OrderStatus::parse(s).is_none())
    {
        return Err(error::validation("status inválido"));
    }
    let rows = sqlx::query_as::<_, WaOrder>(&format!(
        "{ORDER_SELECT} where o.branch_id = $1 and ($2::text is null or o.status = $2)
           and (o.status not in ('ENTREGADO', 'CANCELADO') or o.created_at > now() - interval '1 day')
         order by o.created_at desc limit 200"
    ))
    .bind(q.branch_id)
    .bind(&q.status)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn get_order(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    scope.require(Action::Read)?;
    let order = sqlx::query_as::<_, WaOrder>(&format!("{ORDER_SELECT} where o.id = $1"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Domain(DomainError::NotFound))?;
    let messages: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as(
        "select m.direction, m.body, m.created_at from whatsapp_messages m
           join whatsapp_orders o on o.conversation_id = m.conversation_id
          where o.id = $1 order by m.created_at desc limit 50",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let messages: Vec<_> = messages
        .into_iter()
        .rev()
        .map(|(d, b, t)| serde_json::json!({ "direction": d, "body": b, "created_at": t }))
        .collect();
    Ok(Json(
        serde_json::json!({ "order": order, "messages": messages }),
    ))
}

#[derive(Deserialize)]
pub struct StatusBody {
    pub status: String,
    pub courier_name: Option<String>,
    /// Venta del POS con la que se cobró (al despachar o entregar).
    pub sale_id: Option<Uuid>,
}

/// Avisa al cliente por WhatsApp después de confirmar el cambio.
async fn notify(tx: &mut Tx<'_>, order_id: Uuid, text: String) -> ApiResult<Notice> {
    let row: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "select o.tenant_id, o.conversation_id, o.wa_phone, b.phone_number_id
           from whatsapp_orders o join whatsapp_bots b on b.branch_id = o.branch_id
          where o.id = $1 and b.active",
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((tenant, conv, phone, pnid)) = row else {
        return Ok(None);
    };
    sqlx::query("insert into whatsapp_messages (tenant_id, conversation_id, direction, body) values ($1, $2, 'OUT', $3)")
        .bind(tenant)
        .bind(conv)
        .bind(&text)
        .execute(&mut **tx)
        .await?;
    Ok(Some((pnid, phone, Reply::Text(text))))
}

/// Número de la sede, teléfono del cliente y mensaje para avisarle.
type Notice = Option<(String, String, Reply)>;

async fn send_notice(n: Notice) {
    if let (Some(env), Some((pnid, phone, reply))) = (cloud::env(), n) {
        cloud::send_all(env, &pnid, &phone, &[reply]).await;
    }
}

async fn set_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(b): Body<StatusBody>,
) -> ApiResult<Json<WaOrder>> {
    let (order, notice) = change_status(&state.pool, &user.claims, id, b).await?;
    send_notice(notice).await;
    Ok(Json(order))
}

/// Cambia el estado de un pedido y deja listo el aviso para el cliente.
pub async fn change_status(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    b: StatusBody,
) -> ApiResult<(WaOrder, Notice)> {
    let to = OrderStatus::parse(&b.status).ok_or_else(|| error::validation("status inválido"))?;
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(if to == OrderStatus::Cancelado {
        Action::CancelOrder
    } else {
        Action::TakeOrder
    })?;
    let cur: (String, bool, String, String, String) = sqlx::query_as(
        "select o.status, o.pickup, o.code, o.payment_method, o.payment_status from whatsapp_orders o where o.id = $1 for update",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;
    let from = OrderStatus::parse(&cur.0).ok_or_else(|| error::state("estado desconocido"))?;
    if !from.can_go_to(to) {
        return Err(error::state(format!(
            "no se puede pasar de {} a {}",
            from.as_str(),
            to.as_str()
        )));
    }
    // Un pedido con link de pago no sale sin estar pagado.
    if to == OrderStatus::EnCamino && cur.3 == "LINK" && cur.4 != "PAGADO" {
        return Err(error::state("el pedido aún no está pagado"));
    }
    let courier = b
        .courier_name
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    sqlx::query(
        "update whatsapp_orders set status = $2, courier_name = coalesce($3, courier_name),
                sale_id = coalesce($4, sale_id), updated_at = now() where id = $1",
    )
    .bind(id)
    .bind(to.as_str())
    .bind(&courier)
    .bind(b.sale_id)
    .execute(&mut *tx)
    .await?;
    let business: String = sqlx::query_scalar(
        "select b.business_name from whatsapp_bots b join whatsapp_orders o on o.branch_id = b.branch_id where o.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_default();
    let courier_now: Option<String> =
        sqlx::query_scalar("select courier_name from whatsapp_orders where id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let text = engine::status_text(&cur.2, to, cur.1, courier_now.as_deref(), &business);
    let notice = notify(&mut tx, id, text).await?;
    let order = sqlx::query_as::<_, WaOrder>(&format!("{ORDER_SELECT} where o.id = $1"))
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((order, notice))
}

#[derive(Deserialize)]
struct PaidBody {
    reference: String,
}

async fn mark_paid(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(b): Body<PaidBody>,
) -> ApiResult<Json<WaOrder>> {
    let (order, notice) = confirm_payment(&state.pool, &user.claims, id, &b.reference).await?;
    send_notice(notice).await;
    Ok(Json(order))
}

/// Marca pagado un pedido (link de pago confirmado) y deja listo el aviso.
pub async fn confirm_payment(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    reference: &str,
) -> ApiResult<(WaOrder, Notice)> {
    let reference = reference.trim().to_string();
    if reference.chars().count() < 3 {
        return Err(error::validation(
            "la referencia debe tener al menos 3 caracteres",
        ));
    }
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Charge)?;
    let code: Option<String> = sqlx::query_scalar(
        "update whatsapp_orders set payment_status = 'PAGADO', payment_reference = $2, updated_at = now()
          where id = $1 and payment_status = 'PENDIENTE' and status <> 'CANCELADO' returning code",
    )
    .bind(id)
    .bind(&reference)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(code) = code else {
        return Err(error::state(
            "el pedido no existe, ya está pagado o fue cancelado",
        ));
    };
    let notice = notify(
        &mut tx,
        id,
        format!("Recibimos tu pago del pedido {code}. Ya lo estamos alistando."),
    )
    .await?;
    let order = sqlx::query_as::<_, WaOrder>(&format!("{ORDER_SELECT} where o.id = $1"))
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((order, notice))
}

// ------------------------------------------------------------------ configuración

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ZoneDto {
    pub name: String,
    pub fee: Decimal,
    pub eta_min: i32,
    pub eta_max: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BotDto {
    pub branch_id: Uuid,
    pub phone_number_id: String,
    pub display_phone: Option<String>,
    pub business_name: String,
    pub active: bool,
    pub opens_at: NaiveTime,
    pub closes_at: NaiveTime,
    pub min_order: Decimal,
    pub prepay_enabled: bool,
    pub usual_enabled: bool,
    pub substitutes_enabled: bool,
    pub age_check: bool,
    pub zones: Vec<ZoneDto>,
}

#[derive(Deserialize)]
struct BranchQuery {
    branch_id: Uuid,
}

type BotSettingsRow = (
    String,
    Option<String>,
    String,
    bool,
    NaiveTime,
    NaiveTime,
    Decimal,
    bool,
    bool,
    bool,
    bool,
);

async fn get_bot(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Option<BotDto>>> {
    let (scope, mut tx) = tenancy::begin_in_branch(&state.pool, &user.claims, q.branch_id).await?;
    scope.require(Action::Read)?;
    let row: Option<BotSettingsRow> = sqlx::query_as(
        "select phone_number_id, display_phone, business_name, active, opens_at, closes_at, min_order,
                prepay_enabled, usual_enabled, substitutes_enabled, age_check
           from whatsapp_bots where branch_id = $1",
    )
    .bind(q.branch_id)
    .fetch_optional(&mut *tx)
    .await?;
    let zones = sqlx::query_as::<_, ZoneDto>(
        "select name, fee, eta_min, eta_max from whatsapp_delivery_zones where branch_id = $1 and active order by name",
    )
    .bind(q.branch_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(row.map(|r| BotDto {
        branch_id: q.branch_id,
        phone_number_id: r.0,
        display_phone: r.1,
        business_name: r.2,
        active: r.3,
        opens_at: r.4,
        closes_at: r.5,
        min_order: r.6,
        prepay_enabled: r.7,
        usual_enabled: r.8,
        substitutes_enabled: r.9,
        age_check: r.10,
        zones,
    })))
}

fn validate_bot(b: &BotDto) -> ApiResult<()> {
    if b.phone_number_id.trim().is_empty() || !b.phone_number_id.chars().all(|c| c.is_ascii_digit())
    {
        return Err(error::validation(
            "phone_number_id debe ser el id numérico del número en Meta",
        ));
    }
    if b.business_name.trim().is_empty() {
        return Err(error::validation("falta el nombre del negocio"));
    }
    if b.min_order < Decimal::ZERO {
        return Err(error::validation("el pedido mínimo no puede ser negativo"));
    }
    let mut names = std::collections::HashSet::new();
    for z in &b.zones {
        if z.name.trim().is_empty()
            || z.fee < Decimal::ZERO
            || z.eta_min < 0
            || z.eta_max < z.eta_min
        {
            return Err(error::validation(format!("zona inválida: {}", z.name)));
        }
        if !names.insert(text::normalize(&z.name)) {
            return Err(error::validation(format!("zona repetida: {}", z.name)));
        }
    }
    Ok(())
}

async fn put_bot(
    State(state): State<AppState>,
    user: AuthUser,
    Body(b): Body<BotDto>,
) -> ApiResult<Json<BotDto>> {
    validate_bot(&b)?;
    let (scope, mut tx) = tenancy::begin_in_branch(&state.pool, &user.claims, b.branch_id).await?;
    if !scope.is_admin() {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    sqlx::query(
        "insert into whatsapp_bots (tenant_id, branch_id, phone_number_id, display_phone, business_name, active,
            opens_at, closes_at, min_order, prepay_enabled, usual_enabled, substitutes_enabled, age_check)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         on conflict (branch_id) do update set
            phone_number_id = excluded.phone_number_id, display_phone = excluded.display_phone,
            business_name = excluded.business_name, active = excluded.active, opens_at = excluded.opens_at,
            closes_at = excluded.closes_at, min_order = excluded.min_order, prepay_enabled = excluded.prepay_enabled,
            usual_enabled = excluded.usual_enabled, substitutes_enabled = excluded.substitutes_enabled,
            age_check = excluded.age_check, updated_at = now()",
    )
    .bind(scope.tenant)
    .bind(b.branch_id)
    .bind(b.phone_number_id.trim())
    .bind(&b.display_phone)
    .bind(b.business_name.trim())
    .bind(b.active)
    .bind(b.opens_at)
    .bind(b.closes_at)
    .bind(b.min_order)
    .bind(b.prepay_enabled)
    .bind(b.usual_enabled)
    .bind(b.substitutes_enabled)
    .bind(b.age_check)
    .execute(&mut *tx)
    .await?;
    // Las zonas que ya no vienen se apagan (los pedidos viejos las siguen nombrando).
    sqlx::query("update whatsapp_delivery_zones set active = false where branch_id = $1")
        .bind(b.branch_id)
        .execute(&mut *tx)
        .await?;
    for z in &b.zones {
        sqlx::query(
            "insert into whatsapp_delivery_zones (tenant_id, branch_id, name, fee, eta_min, eta_max, active)
             values ($1, $2, $3, $4, $5, $6, true)
             on conflict (branch_id, name) do update set fee = excluded.fee, eta_min = excluded.eta_min,
                eta_max = excluded.eta_max, active = true",
        )
        .bind(scope.tenant)
        .bind(b.branch_id)
        .bind(z.name.trim())
        .bind(z.fee)
        .bind(z.eta_min)
        .bind(z.eta_max)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(b))
}

// ------------------------------------------------------------------ QR de la tienda

/// Número para wa.me: solo dígitos, con indicativo de Colombia si es un
/// celular de 10 dígitos.
pub fn wa_number(phone: &str) -> Option<String> {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        10 if digits.starts_with('3') => Some(format!("57{digits}")),
        11..=15 => Some(digits),
        _ => None,
    }
}

/// Enlace que abre el chat con el mensaje escrito.
pub fn wa_link(number: &str, text: &str) -> String {
    let mut enc = String::new();
    for b in text.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(*b as char)
            }
            _ => enc.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://wa.me/{number}?text={enc}")
}

/// QR en SVG, en negro sobre blanco y con margen, listo para imprimir.
pub fn qr_svg(data: &str) -> Option<String> {
    use qrcode::render::svg;
    let code =
        qrcode::QrCode::with_error_correction_level(data.as_bytes(), qrcode::EcLevel::M).ok()?;
    Some(
        code.render::<svg::Color<'_>>()
            .min_dimensions(512, 512)
            .dark_color(svg::Color("#14161C"))
            .light_color(svg::Color("#FFFFFF"))
            .quiet_zone(true)
            .build(),
    )
}

#[derive(Deserialize)]
struct QrQuery {
    branch_id: Uuid,
    /// Mensaje que llega escrito al abrir el chat.
    text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StoreQr {
    pub business_name: String,
    pub display_phone: String,
    pub link: String,
    pub text: String,
    pub svg: String,
}

async fn get_qr(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<QrQuery>,
) -> ApiResult<Json<StoreQr>> {
    let (scope, mut tx) = tenancy::begin_in_branch(&state.pool, &user.claims, q.branch_id).await?;
    scope.require(Action::Read)?;
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "select business_name, display_phone from whatsapp_bots where branch_id = $1",
    )
    .bind(q.branch_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some((business_name, phone)) = row else {
        return Err(error::state(
            "esta sede todavía no tiene configurado el WhatsApp del bot",
        ));
    };
    let phone = phone.unwrap_or_default();
    let number = wa_number(&phone).ok_or_else(|| {
        error::state(
            "el número de WhatsApp de la sede no es válido: revisalo en la configuración del bot",
        )
    })?;
    let text = q
        .text
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Hola, quiero hacer un pedido".to_string());
    if text.chars().count() > 200 {
        return Err(error::validation(
            "el mensaje del QR es muy largo (máximo 200 caracteres)",
        ));
    }
    let link = wa_link(&number, &text);
    let svg = qr_svg(&link).ok_or_else(|| error::validation("no se pudo generar el QR"))?;
    Ok(Json(StoreQr {
        business_name,
        display_phone: phone,
        link,
        text,
        svg,
    }))
}

#[cfg(test)]
mod qr_tests {
    use super::*;

    #[test]
    fn builds_wa_links() {
        assert_eq!(wa_number("300 555 0015").as_deref(), Some("573005550015"));
        assert_eq!(
            wa_number("+57 300-555-0015").as_deref(),
            Some("573005550015")
        );
        assert_eq!(
            wa_number("6057274410"),
            None,
            "fijo sin indicativo no sirve para WhatsApp"
        );
        assert_eq!(
            wa_link("573005550015", "Hola, quiero hacer un pedido"),
            "https://wa.me/573005550015?text=Hola%2C%20quiero%20hacer%20un%20pedido"
        );
        assert!(wa_link("57300", "¿Tienen hielo?").contains("%C2%BFTienen%20hielo%3F"));
    }

    #[test]
    fn renders_printable_svg() {
        let svg = qr_svg("https://wa.me/573005550015?text=Hola").unwrap();
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
        assert!(svg.contains("#14161C") && svg.contains("<path") || svg.contains("<rect"));
    }
}

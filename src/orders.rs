use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::db;
use crate::error::{ApiError, DomainError};
use crate::{inventory, tenancy};

const CHANNELS: &[&str] = &[
    "dine_in",
    "qr_self_service",
    "whatsapp",
    "pos_counter",
    "delivery",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OrderStatus {
    Creada,
    EnPreparacion,
    Lista,
    Entregada,
    Cobrada,
    Cerrada,
    Cancelada,
}

impl OrderStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            OrderStatus::Creada => "creada",
            OrderStatus::EnPreparacion => "en_preparacion",
            OrderStatus::Lista => "lista",
            OrderStatus::Entregada => "entregada",
            OrderStatus::Cobrada => "cobrada",
            OrderStatus::Cerrada => "cerrada",
            OrderStatus::Cancelada => "cancelada",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        Some(match value {
            "creada" => OrderStatus::Creada,
            "en_preparacion" => OrderStatus::EnPreparacion,
            "lista" => OrderStatus::Lista,
            "entregada" => OrderStatus::Entregada,
            "cobrada" => OrderStatus::Cobrada,
            "cerrada" => OrderStatus::Cerrada,
            "cancelada" => OrderStatus::Cancelada,
            _ => return None,
        })
    }
}

fn can_transition(from: OrderStatus, to: OrderStatus) -> bool {
    use OrderStatus::*;
    matches!(
        (from, to),
        (Creada, EnPreparacion)
            | (EnPreparacion, Lista)
            | (Lista, Entregada)
            | (Entregada, Cobrada)
            | (Cobrada, Cerrada)
    ) || (to == Cancelada && matches!(from, Creada | EnPreparacion | Lista | Entregada))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub channel: String,
    pub status: String,
    pub table_id: Option<Uuid>,
    pub location_id: Uuid,
    pub responsible_user_id: Uuid,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderLine {
    pub id: Uuid,
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub line_total: Decimal,
}

const ORDER_COLS: &str = "id, channel::text as channel, status::text as status, \
     table_id, location_id, responsible_user_id";

/// Crea una orden. `dine_in` exige mesa libre (se ocupa en la misma transaccion).
pub async fn create_order(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
    channel: &str,
    table_id: Option<Uuid>,
    notes: Option<String>,
) -> Result<Order, ApiError> {
    if !CHANNELS.contains(&channel) {
        return Err(ApiError::Domain(DomainError::Validation(
            "invalid channel".into(),
        )));
    }

    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    tenancy::ensure_location_in_tenant(pool, claims, tenant, location_id).await?;

    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    match (channel, table_id) {
        ("dine_in", Some(table)) => {
            let status: Option<String> =
                sqlx::query_scalar("select status::text from tables where id = $1 and location_id = $2 for update")
                    .bind(table)
                    .bind(location_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            match status.as_deref() {
                Some("libre") => {
                    sqlx::query("update tables set status = 'ocupada' where id = $1")
                        .bind(table)
                        .execute(&mut *tx)
                        .await?;
                }
                Some(_) => return Err(ApiError::Domain(DomainError::Conflict)),
                None => return Err(ApiError::Domain(DomainError::NotFound)),
            }
        }
        ("dine_in", None) => {
            return Err(ApiError::Domain(DomainError::Validation(
                "dine_in requires a table".into(),
            )));
        }
        (_, Some(_)) => {
            return Err(ApiError::Domain(DomainError::Validation(
                "table is only allowed for dine_in".into(),
            )));
        }
        (_, None) => {}
    }

    let order = sqlx::query_as::<_, Order>(&format!(
        "insert into orders
           (tenant_id, location_id, table_id, channel, status, responsible_user_id, notes)
         values ($1, $2, $3, $4::order_channel, 'creada', $5, $6)
         returning {ORDER_COLS}"
    ))
    .bind(tenant)
    .bind(location_id)
    .bind(table_id)
    .bind(channel)
    .bind(claims.sub)
    .bind(notes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(order)
}

/// Agrega una linea con el precio vigente (snapshot) de la sucursal de la orden.
pub async fn add_line(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    notes: Option<String>,
) -> Result<OrderLine, ApiError> {
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }

    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let row: Option<(String, Uuid)> =
        sqlx::query_as("select status::text, location_id from orders where id = $1 for update")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (status, location) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if status != "creada" {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let price: Option<Decimal> = sqlx::query_scalar(
        "select price from product_prices
          where product_id = $1 and (location_id = $2 or location_id is null)
          order by location_id nulls last
          limit 1",
    )
    .bind(product_id)
    .bind(location)
    .fetch_optional(&mut *tx)
    .await?;
    let price = price.ok_or(ApiError::Domain(DomainError::Validation(
        "product has no price for this location".into(),
    )))?;

    let line = sqlx::query_as::<_, OrderLine>(
        "insert into order_lines (tenant_id, order_id, product_id, qty, unit_price, notes)
         values ($1, $2, $3, $4, $5, $6)
         returning id, product_id, qty, unit_price, line_total",
    )
    .bind(tenant)
    .bind(order_id)
    .bind(product_id)
    .bind(qty)
    .bind(price)
    .bind(notes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(line)
}

/// Lee una orden del tenant activo.
pub async fn order(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<Order, ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
    let found = sqlx::query_as::<_, Order>(&format!(
        "select {ORDER_COLS} from orders where id = $1"
    ))
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Lee una linea del tenant activo.
pub async fn line(pool: &PgPool, claims: &Claims, line_id: Uuid) -> Result<OrderLine, ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
    let found = sqlx::query_as::<_, OrderLine>(
        "select id, product_id, qty, unit_price, line_total from order_lines where id = $1",
    )
    .bind(line_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Confirma la orden: exige lineas, reserva inventario y pasa a `en_preparacion`.
pub async fn confirm(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;

    // Lectura sin lock para no bloquear la FK de reservations durante la reserva.
    let (status, location): (String, Uuid) = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let row: Option<(String, Uuid)> =
            sqlx::query_as("select status::text, location_id from orders where id = $1")
                .bind(order_id)
                .fetch_optional(&mut *tx)
                .await?;
        tx.commit().await?;
        row.ok_or(ApiError::Domain(DomainError::NotFound))?
    };
    if status != "creada" {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let lines: Vec<(Uuid, Uuid, Decimal)> = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let rows = sqlx::query_as("select id, product_id, qty from order_lines where order_id = $1")
            .bind(order_id)
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        rows
    };
    if lines.is_empty() {
        return Err(ApiError::Domain(DomainError::Validation(
            "order has no lines".into(),
        )));
    }

    let mut reservation_ids = Vec::new();
    for (line_id, product_id, qty) in lines {
        match inventory::reserve(
            pool,
            claims,
            location,
            Some(order_id),
            Some(line_id),
            product_id,
            qty,
            None,
        )
        .await
        {
            Ok(ids) => reservation_ids.extend(ids),
            Err(err) => {
                let _ = inventory::release(pool, claims, &reservation_ids).await;
                return Err(err);
            }
        }
    }

    let updated = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let affected = sqlx::query(
            "update orders set status = 'en_preparacion' where id = $1 and status = 'creada'",
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        affected
    };

    if updated == 0 {
        let _ = inventory::release(pool, claims, &reservation_ids).await;
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    Ok(())
}

/// Avanza el estado de la orden segun la maquina de estados.
pub async fn transition(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    to: OrderStatus,
) -> Result<(), ApiError> {
    if to == OrderStatus::Cancelada {
        return Err(ApiError::Domain(DomainError::Validation(
            "use cancel for cancellation".into(),
        )));
    }

    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let current: Option<String> =
        sqlx::query_scalar("select status::text from orders where id = $1 for update")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?;
    let current = current.ok_or(ApiError::Domain(DomainError::NotFound))?;
    let from = OrderStatus::from_db(&current)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("unknown order status")))?;

    if !can_transition(from, to) {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    if to == OrderStatus::Cerrada {
        sqlx::query("update orders set status = 'cerrada', closed_at = now() where id = $1")
            .bind(order_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("update orders set status = $2::order_status where id = $1")
            .bind(order_id)
            .bind(to.as_db())
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Cancela la orden (antes de cobrada): libera reservas y libera la mesa.
pub async fn cancel(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let row: Option<(String, Option<Uuid>)> =
        sqlx::query_as("select status::text, table_id from orders where id = $1 for update")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (status, table_id) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let cancellable = matches!(
        status.as_str(),
        "creada" | "en_preparacion" | "lista" | "entregada"
    );
    if !cancellable {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    // Libera reservas activas de la orden.
    let released: Vec<(Uuid, Uuid, Decimal)> = sqlx::query_as(
        "update reservations set status = 'liberada', updated_at = now()
          where order_id = $1 and status = 'activa'
          returning location_id, product_id, qty",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;
    for (location_id, product_id, qty) in &released {
        sqlx::query(
            "update stock set reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    }

    if let Some(table) = table_id {
        sqlx::query("update tables set status = 'libre' where id = $1")
            .bind(table)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("update orders set status = 'cancelada', cancelled_at = now() where id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

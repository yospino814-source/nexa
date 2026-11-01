//! Venta de mostrador en un solo paso (`POST /v1/sales`).
//!
//! Todo ocurre en una transaccion: crear la orden, fijar precios e impuestos
//! del servidor, descontar stock, registrar pagos, asignar el consecutivo del
//! comprobante y encolar el documento fiscal. Si algo falla, no queda nada.
//!
//! **Idempotencia:** el cliente genera `id` (UUID) y lo reusa en cada
//! reintento. Si la venta ya existe con el mismo cuerpo se devuelve tal cual
//! (sin cobrar dos veces); si el cuerpo cambio, se responde 409.

use axum::http::StatusCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, state, validation};
use crate::orders::{self, ITEM_COLS, Order, OrderItem};
use crate::tenancy::{self, Action, Scope};
use crate::{cash, inventory};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethod {
    Efectivo,
    Datafono,
    Qr,
}

impl PaymentMethod {
    fn as_db(self) -> &'static str {
        match self {
            PaymentMethod::Efectivo => "EFECTIVO",
            PaymentMethod::Datafono => "DATAFONO",
            PaymentMethod::Qr => "QR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleItem {
    pub product_id: Uuid,
    pub qty: Decimal,
    #[serde(default)]
    pub unit_price_override: Option<Decimal>,
    #[serde(default)]
    pub discount_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalePayment {
    pub method: PaymentMethod,
    /// En efectivo: lo que entrega el cliente (el vuelto lo calcula el
    /// servidor). En los demas metodos: el monto cobrado.
    pub amount: Decimal,
    /// Numero de aprobacion del datafono, referencia de Nequi, etc.
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleRequest {
    /// Clave de idempotencia generada por el cliente; es tambien el id de la orden.
    pub id: Uuid,
    pub branch_id: Uuid,
    pub items: Vec<SaleItem>,
    pub payments: Vec<SalePayment>,
    #[serde(default)]
    pub tip_amount: Option<Decimal>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub customer_alias: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub method: String,
    pub amount: Decimal,
    pub tendered: Option<Decimal>,
    pub reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaleResponse {
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub payments: Vec<Payment>,
    /// Consecutivo del comprobante (p. ej. `POS-000123`).
    pub receipt_number: String,
    pub change_due: Decimal,
    /// Estado del documento equivalente electronico (`PENDIENTE` al vender).
    pub fiscal_status: String,
}

/// Resultado de un checkout: venta nueva o reintento de una ya registrada.
#[derive(Debug)]
pub enum Checkout {
    Created(SaleResponse),
    Replayed(SaleResponse),
}

/// Validaciones que no necesitan la BD.
fn validate(req: &SaleRequest) -> Result<(), ApiError> {
    if req.items.is_empty() {
        return Err(validation("sale has no items"));
    }
    if req.items.iter().any(|i| i.qty <= Decimal::ZERO) {
        return Err(validation("qty must be > 0"));
    }
    if req.payments.is_empty() {
        return Err(validation("sale has no payments"));
    }
    if req.payments.iter().any(|p| p.amount <= Decimal::ZERO) {
        return Err(validation("payment amount must be > 0"));
    }
    let cash_lines = req
        .payments
        .iter()
        .filter(|p| p.method == PaymentMethod::Efectivo)
        .count();
    if cash_lines > 1 {
        return Err(validation("at most one cash payment per sale"));
    }
    if req.tip_amount.is_some_and(|t| t < Decimal::ZERO) {
        return Err(validation("tip_amount must be >= 0"));
    }
    for item in &req.items {
        if item.unit_price_override.is_some_and(|p| p < Decimal::ZERO) {
            return Err(validation("unit_price_override must be >= 0"));
        }
        if item
            .discount_pct
            .is_some_and(|d| d < Decimal::ZERO || d > Decimal::new(100, 0))
        {
            return Err(validation("discount_pct must be between 0 and 100"));
        }
    }
    Ok(())
}

/// Reparto de los pagos sobre el total: cuanto aplica cada metodo y el vuelto.
#[derive(Debug, PartialEq)]
struct Settlement {
    /// Efectivo aplicado a la orden (lo recibido menos el vuelto).
    cash_applied: Decimal,
    change_due: Decimal,
    paid: Decimal,
}

/// Los pagos no-efectivo se aplican primero y no pueden superar el total (no
/// hay vuelto de una tarjeta); el efectivo cubre el resto y genera el vuelto.
fn settle(total: Decimal, payments: &[SalePayment]) -> Result<Settlement, ApiError> {
    let non_cash: Decimal = payments
        .iter()
        .filter(|p| p.method != PaymentMethod::Efectivo)
        .map(|p| p.amount)
        .sum();
    if non_cash > total {
        return Err(validation("non-cash payments exceed the total"));
    }
    let due = total - non_cash;
    let tendered = payments
        .iter()
        .find(|p| p.method == PaymentMethod::Efectivo)
        .map(|p| p.amount);

    match tendered {
        None if due == Decimal::ZERO => Ok(Settlement {
            cash_applied: Decimal::ZERO,
            change_due: Decimal::ZERO,
            paid: non_cash,
        }),
        None => Err(validation("payments do not cover the total")),
        Some(_) if due == Decimal::ZERO => Err(validation("cash payment is not needed")),
        Some(t) if t < due => Err(validation("payments do not cover the total")),
        Some(t) => Ok(Settlement {
            cash_applied: due,
            change_due: t - due,
            paid: non_cash + t,
        }),
    }
}

/// Registra una venta de mostrador cobrada.
pub async fn checkout(
    pool: &PgPool,
    claims: &Claims,
    req: SaleRequest,
) -> Result<Checkout, ApiError> {
    validate(&req)?;
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require(Action::Charge)?;
    let result = checkout_tx(&mut tx, &scope, &req).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn checkout_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: &SaleRequest,
) -> Result<Checkout, ApiError> {
    let body = serde_json::to_value(req).map_err(anyhow::Error::from)?;
    let branch = req.branch_id;

    // 1. Reclamar el id. Si otro request con el mismo id esta en curso, este
    //    insert espera a que termine y luego no inserta nada.
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "insert into orders (id, tenant_id, branch_id, sale_type_id, status_id, client_request)
         values ($1, $2, $3, public.tipo_id('VENTA', 'MOSTRADOR'), public.tipo_id('ORD', 'CREADA'), $4)
         on conflict (id) do nothing
         returning id",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(branch)
    .bind(&body)
    .fetch_optional(&mut **tx)
    .await?;

    if claimed.is_none() {
        let stored: Option<Option<serde_json::Value>> =
            sqlx::query_scalar("select client_request from orders where id = $1")
                .bind(req.id)
                .fetch_optional(&mut **tx)
                .await?;
        return match stored {
            Some(Some(previous)) if previous == body => {
                Ok(Checkout::Replayed(load_response(tx, req.id).await?))
            }
            _ => Err(state("sale id already used with a different request")),
        };
    }

    // Cargar ajustes de la sucursal para validar reglas de venta y caja
    let settings = crate::settings::load_tx(tx, branch).await?;

    // Validar cliente y alias (F1.4)
    let customer_id = req.customer_id;
    let customer_alias = req.customer_alias.as_deref().map(|a| a.trim());

    // 1. Vienen los dos: 422 SALE_CUSTOMER_AND_ALIAS
    if customer_id.is_some() && req.customer_alias.is_some() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_CUSTOMER_AND_ALIAS",
            "No se puede especificar cliente y alias al mismo tiempo",
            None,
        ));
    }

    // 2. customer_alias sin espacios a los lados debe tener entre 1 y 40 caracteres. Si no: 422 SALE_ALIAS_INVALID
    if let Some(raw_alias) = &req.customer_alias {
        let trimmed = raw_alias.trim();
        if trimmed.is_empty() || trimmed.len() > 40 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "SALE_ALIAS_INVALID",
                "El alias debe tener entre 1 y 40 caracteres",
                None,
            ));
        }
    }

    // 3. Viene alias y settings.pos.allow_alias_sales es falso: 422 SALE_ALIAS_NOT_ALLOWED
    if customer_alias.is_some() && !settings.pos.allow_alias_sales {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_ALIAS_NOT_ALLOWED",
            "Las ventas con alias no están permitidas en esta sucursal",
            None,
        ));
    }

    // 4. No viene ninguno y settings.pos.allow_anonymous_sales es falso: 422 SALE_CUSTOMER_REQUIRED
    if customer_id.is_none() && customer_alias.is_none() && !settings.pos.allow_anonymous_sales {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_CUSTOMER_REQUIRED",
            "Esta sucursal requiere identificar al cliente",
            None,
        ));
    }

    // 5. Viene customer_id y no existe o está archivado: 422 SALE_CUSTOMER_INVALID
    if let Some(cid) = customer_id {
        let exists: Option<bool> = sqlx::query_scalar(
            "select true from customers where id = $1 and tenant_id = $2 and archived_at is null",
        )
        .bind(cid)
        .bind(scope.tenant)
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "SALE_CUSTOMER_INVALID",
                "El cliente seleccionado no existe o está archivado",
                None,
            ));
        }
    }

    for item in &req.items {
        if item.unit_price_override.is_some() {
            if !settings.pos.allow_price_override {
                return Err(validation("price override is not allowed in this branch"));
            }
            scope.require_permission("pos:price_override")?;
        }
        if let Some(disc) = item.discount_pct {
            if disc > settings.pos.max_discount_pct {
                return Err(validation("discount exceeds maximum allowed"));
            }
            scope.require_permission("pos:discount")?;
        }
    }

    // 2. Turno de caja del cajero (si la sucursal lo exige)
    let session = if settings.cash.require_session {
        let s = cash::open_session_tx(tx, scope, branch)
            .await?
            .ok_or_else(|| state("no open cash session for this cashier"))?;
        Some(s)
    } else {
        cash::open_session_tx(tx, scope, branch).await?
    };

    // 3. Lineas con precio e impuesto del servidor (con posibles overrides y descuentos).
    for item in &req.items {
        let (catalog_price, tax_rate) = orders::sellable_price(tx, branch, item.product_id).await?;
        let base_price = item.unit_price_override.unwrap_or(catalog_price);
        let final_price = match item.discount_pct {
            Some(disc) if disc > Decimal::ZERO => {
                let factor = Decimal::ONE - (disc / Decimal::new(100, 0));
                (base_price * factor).round_dp(2)
            }
            _ => base_price.round_dp(2),
        };
        sqlx::query(
            "insert into order_items
               (tenant_id, branch_id, order_id, product_id, qty, unit_price, tax_rate)
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(scope.tenant)
        .bind(branch)
        .bind(req.id)
        .bind(item.product_id)
        .bind(item.qty)
        .bind(final_price)
        .bind(tax_rate)
        .execute(&mut **tx)
        .await?;
    }

    // 4. Stock: descuento directo, sin reserva (se cobra en el acto).
    let lines: Vec<(Uuid, Decimal)> = req.items.iter().map(|i| (i.product_id, i.qty)).collect();
    inventory::sell_tx(tx, scope, branch, req.id, &lines).await?;

    // 5. Totales (impuesto sumado sobre el precio) y propina.
    sqlx::query("update orders set tip_amount = $2 where id = $1")
        .bind(req.id)
        .bind(req.tip_amount.unwrap_or(Decimal::ZERO))
        .execute(&mut **tx)
        .await?;
    orders::recalc_totals(tx, req.id).await?;
    let total: Decimal = sqlx::query_scalar("select total_amount from orders where id = $1")
        .bind(req.id)
        .fetch_one(&mut **tx)
        .await?;

    // 6. Pagos.
    let settlement = settle(total, &req.payments)?;
    for p in &req.payments {
        let (amount, tendered) = if p.method == PaymentMethod::Efectivo {
            (settlement.cash_applied, Some(p.amount))
        } else {
            (p.amount, None)
        };
        sqlx::query(
            "insert into payments
               (tenant_id, branch_id, order_id, cash_session_id, method_id, amount, tendered, reference, created_by)
             values ($1, $2, $3, $4, public.tipo_id('PAGO', $5), $6, $7, $8, $9)",
        )
        .bind(scope.tenant)
        .bind(branch)
        .bind(req.id)
        .bind(session)
        .bind(p.method.as_db())
        .bind(amount)
        .bind(tendered)
        .bind(&p.reference)
        .bind(scope.user)
        .execute(&mut **tx)
        .await?;
    }

    // 7. Consecutivo sin huecos + documento fiscal en la outbox.
    let (prefix, number) = crate::sequences::next_tx(tx, scope, branch, "POS").await?;
    sqlx::query(
        "insert into fiscal_documents (tenant_id, branch_id, order_id, doc_type, prefix, number, status_id)
         values ($1, $2, $3, 'POS', $4, $5, public.tipo_id('DOCF', 'PENDIENTE'))",
    )
    .bind(scope.tenant)
    .bind(branch)
    .bind(req.id)
    .bind(&prefix)
    .bind(number)
    .execute(&mut **tx)
    .await?;

    // 8. Cierre de la venta.
    sqlx::query(
        "update orders
            set status_id = public.tipo_id('ORD', 'COBRADA'),
                cash_session_id = $2, receipt_prefix = $3, receipt_number = $4,
                paid_amount = $5, change_due = $6, waiter_id = null,
                customer_id = $7, customer_alias = $8
          where id = $1",
    )
    .bind(req.id)
    .bind(session)
    .bind(&prefix)
    .bind(number)
    .bind(settlement.paid)
    .bind(settlement.change_due)
    .bind(customer_id)
    .bind(customer_alias)
    .execute(&mut **tx)
    .await?;

    Ok(Checkout::Created(load_response(tx, req.id).await?))
}

fn format_receipt(prefix: &str, number: i64) -> String {
    format!("{prefix}-{number:06}")
}

/// Arma la respuesta desde la BD (la misma para la venta y sus reintentos).
async fn load_response(tx: &mut Tx<'_>, order_id: Uuid) -> Result<SaleResponse, ApiError> {
    let order = orders::order_tx(tx, order_id).await?;
    let items = sqlx::query_as::<_, OrderItem>(&format!(
        "select {ITEM_COLS} from order_items where order_id = $1 order by created_at, id"
    ))
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    let payments = sqlx::query_as::<_, Payment>(
        "select p.id, t.tima_codigo as method, p.amount, p.tendered, p.reference
           from payments p
           join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.order_id = $1
          order by p.created_at, p.id",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    let (prefix, number, change_due, fiscal_status): (String, i64, Decimal, String) =
        sqlx::query_as(
            "select o.receipt_prefix, o.receipt_number, o.change_due, t.tima_codigo
               from orders o
               join fiscal_documents f on f.order_id = o.id and f.doc_type = 'POS'
               join tbl_tipos_maestra t on t.tima_id = f.status_id
              where o.id = $1",
        )
        .bind(order_id)
        .fetch_one(&mut **tx)
        .await?;

    Ok(SaleResponse {
        order,
        items,
        payments,
        receipt_number: format_receipt(&prefix, number),
        change_due,
        fiscal_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pay(method: PaymentMethod, amount: i64) -> SalePayment {
        SalePayment {
            method,
            amount: Decimal::from(amount),
            reference: None,
        }
    }

    #[test]
    fn settle_cash_with_change() {
        let s = settle(Decimal::from(8950), &[pay(PaymentMethod::Efectivo, 10000)]).unwrap();
        assert_eq!(s.cash_applied, Decimal::from(8950));
        assert_eq!(s.change_due, Decimal::from(1050));
        assert_eq!(s.paid, Decimal::from(10000));
    }

    #[test]
    fn settle_mixed_card_and_cash() {
        let s = settle(
            Decimal::from(8950),
            &[
                pay(PaymentMethod::Datafono, 5000),
                pay(PaymentMethod::Efectivo, 5000),
            ],
        )
        .unwrap();
        assert_eq!(s.cash_applied, Decimal::from(3950));
        assert_eq!(s.change_due, Decimal::from(1050));
    }

    #[test]
    fn settle_rejects_bad_combinations() {
        let total = Decimal::from(1000);
        // Tarjeta por encima del total: no hay vuelto de tarjeta.
        assert!(settle(total, &[pay(PaymentMethod::Datafono, 1500)]).is_err());
        // No alcanza.
        assert!(settle(total, &[pay(PaymentMethod::Qr, 500)]).is_err());
        assert!(settle(total, &[pay(PaymentMethod::Efectivo, 500)]).is_err());
        // Efectivo sobrante cuando la tarjeta ya cubrio todo.
        assert!(
            settle(
                total,
                &[
                    pay(PaymentMethod::Datafono, 1000),
                    pay(PaymentMethod::Efectivo, 100)
                ]
            )
            .is_err()
        );
        // Exacto con QR.
        assert!(settle(total, &[pay(PaymentMethod::Qr, 1000)]).is_ok());
    }

    #[test]
    fn validate_rejects_empty_and_duplicated_cash() {
        let base = SaleRequest {
            id: Uuid::new_v4(),
            branch_id: Uuid::new_v4(),
            items: vec![SaleItem {
                product_id: Uuid::new_v4(),
                qty: Decimal::ONE,
                unit_price_override: None,
                discount_pct: None,
            }],
            payments: vec![pay(PaymentMethod::Efectivo, 1000)],
            tip_amount: None,
            customer_id: None,
            customer_alias: None,
        };
        assert!(validate(&base).is_ok());

        let mut no_items = base.clone();
        no_items.items.clear();
        assert!(validate(&no_items).is_err());

        let mut two_cash = base.clone();
        two_cash.payments.push(pay(PaymentMethod::Efectivo, 1));
        assert!(validate(&two_cash).is_err());

        let mut zero_qty = base;
        zero_qty.items[0].qty = Decimal::ZERO;
        assert!(validate(&zero_qty).is_err());
    }

    #[test]
    fn receipt_format_pads_number() {
        assert_eq!(format_receipt("POS", 123), "POS-000123");
    }
}

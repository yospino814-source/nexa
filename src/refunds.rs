//! Devoluciones de ventas (F1.11).
//!
//! Una devolución se crea `PENDIENTE` o `APROBADA` según el ajuste
//! `pos.refund_requires_approval` y el permiso `pos:refund` de quien la pide.
//! Los efectos (stock, caja, saldo devuelto de la venta y nota crédito) se
//! aplican en la misma transacción en que se aprueba. No hay "deshacer".

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::cash::{self, CashMovementKind};
use crate::error::{self, ApiError, DomainError};
use crate::money::money;
use crate::tenancy::{self, Scope};
use crate::{audit, inventory, sequences};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundMethod {
    Efectivo,
    Original,
}

impl RefundMethod {
    fn as_db(self) -> &'static str {
        match self {
            RefundMethod::Efectivo => "EFECTIVO",
            RefundMethod::Original => "ORIGINAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Disposition {
    Reintegrar,
    Merma,
    SinDevolucion,
}

impl Disposition {
    fn as_db(self) -> &'static str {
        match self {
            Disposition::Reintegrar => "REINTEGRAR",
            Disposition::Merma => "MERMA",
            Disposition::SinDevolucion => "SIN_DEVOLUCION",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundItemInput {
    pub order_item_id: Uuid,
    pub qty: Decimal,
    pub disposition: Disposition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundRequest {
    /// Generado por el cliente: clave de idempotencia.
    pub id: Uuid,
    pub order_id: Uuid,
    pub method: RefundMethod,
    pub reason: String,
    #[serde(default)]
    pub reversal_reference: Option<String>,
    pub items: Vec<RefundItemInput>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RefundItem {
    pub order_item_id: Uuid,
    pub name: String,
    pub qty: Decimal,
    pub disposition: String,
    pub line_total: Decimal,
    pub tax_amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct Refund {
    pub id: Uuid,
    pub order_id: Uuid,
    pub receipt_number: Option<String>,
    pub number: String,
    pub status: String,
    pub method: String,
    pub original_method: Option<String>,
    pub reversal_reference: Option<String>,
    pub reason: String,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub items: Vec<RefundItem>,
    pub requested_by_name: Option<String>,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub reject_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Resultado de crear: devolución nueva (201) o reintento idéntico (200).
#[derive(Debug)]
pub enum Created {
    New(Refund),
    Replayed(Refund),
}

fn invalid(code: &'static str, message: &str) -> ApiError {
    error::coded(StatusCode::UNPROCESSABLE_ENTITY, code, message, None)
}

fn conflict(code: &'static str, message: &str) -> ApiError {
    error::coded(StatusCode::CONFLICT, code, message, None)
}

/// Texto sin espacios a los lados con longitud (en caracteres) entre `min` y `max`.
fn text_between(value: &str, min: usize, max: usize) -> Option<String> {
    let t = value.trim();
    let n = t.chars().count();
    (n >= min && n <= max).then(|| t.to_string())
}

// ------------------------------------------------------------------- crear

pub async fn create(
    pool: &PgPool,
    claims: &Claims,
    req: RefundRequest,
) -> Result<Created, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:sell")?;
    let result = create_tx(&mut tx, &scope, &req).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn create_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: &RefundRequest,
) -> Result<Created, ApiError> {
    let body = serde_json::to_value(req).map_err(anyhow::Error::from)?;

    // 1. Idempotencia.
    let stored: Option<serde_json::Value> =
        sqlx::query_scalar("select client_request from refunds where id = $1")
            .bind(req.id)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(previous) = stored {
        return if previous == body {
            Ok(Created::Replayed(load_tx(tx, req.id).await?))
        } else {
            Err(conflict(
                "IDEMPOTENCY_CONFLICT",
                "refund id already used with a different request",
            ))
        };
    }

    // 2. Orden bloqueada, cobrada y de la sucursal del usuario.
    let order: Option<(Uuid, String)> = sqlx::query_as(
        "select o.branch_id, t.tima_codigo
           from orders o join tbl_tipos_maestra t on t.tima_id = o.status_id
          where o.id = $1
          for update of o",
    )
    .bind(req.order_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((branch, status)) = order else {
        return Err(conflict("REFUND_ORDER_INVALID", "order not found"));
    };
    if status != "COBRADA" && status != "CERRADA" {
        return Err(conflict("REFUND_ORDER_INVALID", "order is not charged"));
    }
    if tenancy::ensure_branch_tx(tx, scope, branch).await.is_err() {
        return Err(conflict(
            "REFUND_ORDER_INVALID",
            "order belongs to another branch",
        ));
    }

    // 3. Ítems no vacíos ni repetidos.
    if req.items.is_empty() {
        return Err(invalid("REFUND_INVALID", "refund has no items"));
    }
    let mut seen = std::collections::HashSet::new();
    for item in &req.items {
        if !seen.insert(item.order_item_id) || item.qty <= Decimal::ZERO {
            return Err(invalid("REFUND_INVALID", "repeated item or qty <= 0"));
        }
    }
    let reason = text_between(&req.reason, 3, 200)
        .ok_or_else(|| invalid("REFUND_INVALID", "reason must have 3 to 200 characters"))?;

    // 4. Techo devolvible y montos por línea.
    let mut lines: Vec<(Uuid, Decimal, Disposition, Decimal, Decimal)> = Vec::new();
    for item in &req.items {
        let row: Option<(Option<Uuid>, Decimal, Decimal, Decimal)> = sqlx::query_as(
            "select product_id, qty, line_total, tax_amount
               from order_items
              where id = $1 and order_id = $2",
        )
        .bind(item.order_item_id)
        .bind(req.order_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((product_id, sold_qty, line_total, line_tax)) = row else {
            return Err(invalid(
                "REFUND_INVALID",
                "item does not belong to the order",
            ));
        };
        let (prev_qty, prev_total, prev_tax): (Decimal, Decimal, Decimal) = sqlx::query_as(
            "select coalesce(sum(ri.qty), 0), coalesce(sum(ri.line_total), 0), coalesce(sum(ri.tax_amount), 0)
               from refund_items ri
               join refunds r on r.id = ri.refund_id
              where ri.order_item_id = $1 and r.status in ('PENDIENTE', 'APROBADA')",
        )
        .bind(item.order_item_id)
        .fetch_one(&mut **tx)
        .await?;
        let refundable = sold_qty - prev_qty;
        if item.qty > refundable {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "REFUND_QTY_EXCEEDED",
                "qty exceeds what is left to refund",
                Some(serde_json::json!({
                    "order_item_id": item.order_item_id,
                    "refundable": refundable,
                })),
            ));
        }
        if product_id.is_none() && item.disposition != Disposition::SinDevolucion {
            return Err(invalid(
                "REFUND_INVALID",
                "a custom item only admits SIN_DEVOLUCION",
            ));
        }
        // Lo último que queda se devuelve exacto: sin diferencias de centavos.
        let (total, tax) = if item.qty == refundable {
            (line_total - prev_total, line_tax - prev_tax)
        } else {
            (
                money(line_total * item.qty / sold_qty),
                money(line_tax * item.qty / sold_qty),
            )
        };
        lines.push((item.order_item_id, item.qty, item.disposition, total, tax));
    }
    let subtotal: Decimal = lines.iter().map(|l| l.3).sum();
    let tax_amount: Decimal = lines.iter().map(|l| l.4).sum();
    let total = subtotal + tax_amount;
    if total <= Decimal::ZERO {
        return Err(invalid("REFUND_INVALID", "refund total must be > 0"));
    }

    // 5. Método.
    let mut effective_method = req.method.as_db();
    let mut original_method: Option<String> = None;
    let mut reversal_reference: Option<String> = None;

    let methods: Vec<String> = sqlx::query_scalar(
        "select t.tima_codigo from payments p
           join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.order_id = $1",
    )
    .bind(req.order_id)
    .fetch_all(&mut **tx)
    .await?;

    if methods.len() == 1 && methods[0] == "CREDITO" {
        // F3.2: si CREDITO es el único pago, el método se fuerza a ORIGINAL con original_method = CREDITO
        effective_method = "ORIGINAL";
        original_method = Some("CREDITO".to_string());
    } else if req.method == RefundMethod::Original {
        if methods.len() != 1 {
            return Err(invalid(
                "REFUND_ORIGINAL_MIXED",
                "the sale has several payments: refund in cash",
            ));
        }
        let m = methods[0].clone();
        if m == "DATAFONO" || m == "QR" {
            reversal_reference = Some(
                req.reversal_reference
                    .as_deref()
                    .and_then(|r| text_between(r, 3, 60))
                    .ok_or_else(|| {
                        invalid(
                            "REFUND_REFERENCE_REQUIRED",
                            "reversal reference is required",
                        )
                    })?,
            );
        }
        original_method = Some(m);
    }

    // 6. Número.
    let (prefix, number) = sequences::next_tx(tx, scope, branch, "DEVOLUCION").await?;

    // 7. Estado inicial.
    let settings = crate::settings::load_tx(tx, branch).await?;
    let approve_now = !settings.pos.refund_requires_approval || scope.has_permission("pos:refund");

    sqlx::query(
        "insert into refunds
           (id, tenant_id, branch_id, order_id, prefix, number, status, method, original_method,
            reversal_reference, reason, subtotal, tax_amount, total, requested_by, client_request)
         values ($1, $2, $3, $4, $5, $6, 'PENDIENTE', $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(branch)
    .bind(req.order_id)
    .bind(&prefix)
    .bind(number)
    .bind(effective_method)
    .bind(&original_method)
    .bind(&reversal_reference)
    .bind(&reason)
    .bind(subtotal)
    .bind(tax_amount)
    .bind(total)
    .bind(scope.user)
    .bind(&body)
    .execute(&mut **tx)
    .await?;

    for (order_item_id, qty, disposition, line_total, tax) in &lines {
        sqlx::query(
            "insert into refund_items (tenant_id, refund_id, order_item_id, qty, disposition, line_total, tax_amount)
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(scope.tenant)
        .bind(req.id)
        .bind(order_item_id)
        .bind(qty)
        .bind(disposition.as_db())
        .bind(line_total)
        .bind(tax)
        .execute(&mut **tx)
        .await?;
    }

    // 8. Auditoría (y efectos si se aprueba al crearla).
    audit::record_tx(
        tx,
        scope,
        "DEVOLUCION_SOLICITADA",
        "refunds",
        req.id,
        None,
        Some(serde_json::json!({ "order_id": req.order_id, "total": total, "method": req.method.as_db() })),
        None,
    )
    .await?;
    if approve_now {
        apply_effects_tx(tx, scope, req.id).await?;
        audit::record_tx(
            tx,
            scope,
            "DEVOLUCION_APROBADA",
            "refunds",
            req.id,
            None,
            Some(serde_json::json!({ "total": total })),
            None,
        )
        .await?;
    }

    Ok(Created::New(load_tx(tx, req.id).await?))
}

// ------------------------------------------------------------------ efectos

/// Reduce la cuenta por cobrar asociada a una orden.
/// Devuelve el monto aplicado.
pub async fn reduce_receivable_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    order_id: Uuid,
    amount: Decimal,
    strict: bool,
) -> Result<Decimal, ApiError> {
    // 1. select id, principal, amount_total, amount_paid from receivables where order_id = $1 and tenant_id = $2 for update. Si no existe: devuelve 0.
    let row: Option<(Uuid, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "select id, principal, amount_total, amount_paid
           from receivables
          where order_id = $1 and tenant_id = $2
          for update",
    )
    .bind(order_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((rec_id, principal, amount_total, amount_paid)) = row else {
        return Ok(Decimal::ZERO);
    };

    // 2. saldo = amount_total − amount_paid
    let saldo = amount_total - amount_paid;

    // 3. Si strict y amount > saldo: 409 REFUND_EXCEEDS_RECEIVABLE
    if strict && amount > saldo {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "REFUND_EXCEEDS_RECEIVABLE",
            "El monto a devolver supera el saldo de la cuenta por cobrar",
            None,
        ));
    }

    // 4. aplicado = min(amount, saldo). Si es 0, devuelve 0.
    let aplicado = amount.min(saldo);
    if aplicado <= Decimal::ZERO {
        return Ok(Decimal::ZERO);
    }

    // 5. amount_total −= aplicado y principal −= aplicado. Si queda amount_total = amount_paid, status = 'PAGADA'.
    let new_total = amount_total - aplicado;
    let new_principal = (principal - aplicado).max(Decimal::ZERO);
    sqlx::query(
        "update receivables
            set amount_total = $2,
                principal = $3,
                status = case when $2 = amount_paid then 'PAGADA' else status end
          where id = $1 and tenant_id = $4",
    )
    .bind(rec_id)
    .bind(new_total)
    .bind(new_principal)
    .bind(scope.tenant)
    .execute(&mut **tx)
    .await?;

    // 6. Cuotas, de la última a la primera (order by n desc, con for update), hasta agotar aplicado.
    let installments: Vec<(i32, Decimal, Decimal)> = sqlx::query_as(
        "select n, amount, amount_paid
           from receivable_installments
          where receivable_id = $1 and tenant_id = $2
          order by n desc
          for update",
    )
    .bind(rec_id)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    let mut restante = aplicado;
    for (n, inst_amount, inst_paid) in installments {
        if restante <= Decimal::ZERO {
            break;
        }
        let reducible = inst_amount - inst_paid;
        if reducible > restante {
            let new_amount = inst_amount - restante;
            sqlx::query(
                "update receivable_installments
                    set amount = $3
                  where receivable_id = $1 and n = $2 and tenant_id = $4",
            )
            .bind(rec_id)
            .bind(n)
            .bind(new_amount)
            .bind(scope.tenant)
            .execute(&mut **tx)
            .await?;
            break;
        } else {
            if inst_paid == Decimal::ZERO {
                sqlx::query(
                    "delete from receivable_installments
                      where receivable_id = $1 and n = $2 and tenant_id = $3",
                )
                .bind(rec_id)
                .bind(n)
                .bind(scope.tenant)
                .execute(&mut **tx)
                .await?;
            } else {
                sqlx::query(
                    "update receivable_installments
                        set amount = amount_paid
                      where receivable_id = $1 and n = $2 and tenant_id = $3",
                )
                .bind(rec_id)
                .bind(n)
                .bind(scope.tenant)
                .execute(&mut **tx)
                .await?;
            }
            restante -= reducible;
        }
    }

    // 7. due_date = select min(due_date) from receivable_installments where receivable_id = $1 and amount_paid < amount, leído con fetch_one en Option<chrono::NaiveDate>. Si es Some, actualiza receivables.due_date; si es None, no lo toca.
    let next_due: Option<chrono::NaiveDate> = sqlx::query_scalar(
        "select min(due_date) from receivable_installments
          where receivable_id = $1 and amount_paid < amount",
    )
    .bind(rec_id)
    .fetch_one(&mut **tx)
    .await?;

    if let Some(due) = next_due {
        sqlx::query("update receivables set due_date = $2 where id = $1 and tenant_id = $3")
            .bind(rec_id)
            .bind(due)
            .bind(scope.tenant)
            .execute(&mut **tx)
            .await?;
    }

    // 8. Devuelve aplicado.
    Ok(aplicado)
}

/// Aplica stock, caja, saldo devuelto y nota crédito, y deja la devolución
/// `APROBADA`. La devolución debe estar bloqueada o recién creada en `tx`.
async fn apply_effects_tx(tx: &mut Tx<'_>, scope: &Scope, refund_id: Uuid) -> Result<(), ApiError> {
    let (branch, order_id, prefix, number, method, original_method, total): (
        Uuid,
        Uuid,
        String,
        i64,
        String,
        Option<String>,
        Decimal,
    ) = sqlx::query_as(
        "select branch_id, order_id, prefix, number, method, original_method, total
           from refunds where id = $1",
    )
    .bind(refund_id)
    .fetch_one(&mut **tx)
    .await?;

    // 1. Stock: solo REINTEGRAR de líneas con producto.
    let returns: Vec<(Uuid, Decimal, Decimal, Uuid)> = sqlx::query_as(
        "select oi.product_id, ri.qty, oi.units_per_package, ri.order_item_id
           from refund_items ri
           join order_items oi on oi.id = ri.order_item_id
          where ri.refund_id = $1 and ri.disposition = 'REINTEGRAR' and oi.product_id is not null",
    )
    .bind(refund_id)
    .fetch_all(&mut **tx)
    .await?;
    let label = format!("Devolución {prefix}-{number:06}");
    for (product_id, qty, units, order_item_id) in returns {
        let lot_id: Option<Uuid> = sqlx::query_scalar(
            "select lot_id from order_item_lots where order_item_id = $1 order by qty desc limit 1",
        )
        .bind(order_item_id)
        .fetch_optional(&mut **tx)
        .await?;

        inventory::return_tx(
            tx,
            scope,
            branch,
            product_id,
            qty * units,
            &label,
            ("REFUND", refund_id),
            lot_id,
        )
        .await?;
    }

    // 2. Pagos de la orden para saber si hubo CREDITO
    let methods: Vec<String> = sqlx::query_scalar(
        "select t.tima_codigo from payments p
           join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.order_id = $1",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;

    let has_credit = methods.iter().any(|m| m == "CREDITO");
    let mut session: Option<Uuid> = None;

    if original_method.as_deref() == Some("CREDITO") || (methods.len() == 1 && has_credit) {
        // Un solo pago CREDITO
        let aplicado = reduce_receivable_tx(tx, scope, order_id, total, true).await?;
        sqlx::query("update refunds set receivable_applied = $2 where id = $1")
            .bind(refund_id)
            .bind(aplicado)
            .execute(&mut **tx)
            .await?;
        // No hay movimiento de caja: cash_out ya es falso con original_method = CREDITO
    } else if has_credit {
        // Pago mixto con CREDITO:
        // 1. aplicado = reduce_receivable_tx(..., total, strict = false)
        let aplicado = reduce_receivable_tx(tx, scope, order_id, total, false).await?;
        // 2. receivable_applied = aplicado
        sqlx::query("update refunds set receivable_applied = $2 where id = $1")
            .bind(refund_id)
            .bind(aplicado)
            .execute(&mut **tx)
            .await?;
        // 3. movimiento DEVOLUCION por total - aplicado
        let cash_refund = total - aplicado;
        // 4. si total - aplicado = 0, no se llama a record_movement_tx y no se exige turno abierto
        if cash_refund > Decimal::ZERO {
            let open = cash::open_session_tx(tx, scope, branch).await?;
            let Some(session_id) = open else {
                return Err(conflict(
                    "REFUND_NEEDS_OPEN_SESSION",
                    "open a cash session to refund cash",
                ));
            };
            cash::record_movement_tx(
                tx,
                scope,
                session_id,
                CashMovementKind::Devolucion,
                cash_refund,
                &label,
                None,
                Some("REFUND"),
                Some(refund_id),
            )
            .await?;
            session = Some(session_id);
        }
    } else {
        // Sin pago CREDITO: como hoy
        let cash_out = method == "EFECTIVO" || original_method.as_deref() == Some("EFECTIVO");
        if cash_out {
            let open = cash::open_session_tx(tx, scope, branch).await?;
            let Some(session_id) = open else {
                return Err(conflict(
                    "REFUND_NEEDS_OPEN_SESSION",
                    "open a cash session to refund cash",
                ));
            };
            cash::record_movement_tx(
                tx,
                scope,
                session_id,
                CashMovementKind::Devolucion,
                total,
                &label,
                None,
                Some("REFUND"),
                Some(refund_id),
            )
            .await?;
            session = Some(session_id);
        }
    }

    // 3. Lo devuelto de la venta.
    sqlx::query("update orders set refunded_amount = refunded_amount + $2 where id = $1")
        .bind(order_id)
        .bind(total)
        .execute(&mut **tx)
        .await?;

    // 4. Nota crédito si la venta tiene documento POS.
    sqlx::query(
        "insert into fiscal_documents (tenant_id, branch_id, order_id, doc_type, prefix, number, status_id, refund_id)
         select f.tenant_id, f.branch_id, f.order_id, 'NOTA_CREDITO', $2, $3, public.tipo_id('DOCF', 'PENDIENTE'), $4
           from fiscal_documents f
          where f.order_id = $1 and f.doc_type = 'POS' and f.refund_id is null",
    )
    .bind(order_id)
    .bind(&prefix)
    .bind(number)
    .bind(refund_id)
    .execute(&mut **tx)
    .await?;

    // 5. Decisión.
    sqlx::query(
        "update refunds
            set status = 'APROBADA', decided_by = $2, decided_at = now(), cash_session_id = $3
          where id = $1",
    )
    .bind(refund_id)
    .bind(scope.user)
    .bind(session)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Bloquea una devolución pendiente de la sucursal del usuario.
async fn lock_pending_tx(tx: &mut Tx<'_>, scope: &Scope, refund_id: Uuid) -> Result<(), ApiError> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("select branch_id, status from refunds where id = $1 for update")
            .bind(refund_id)
            .fetch_optional(&mut **tx)
            .await?;
    let (branch, status) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    tenancy::ensure_branch_tx(tx, scope, branch).await?;
    if status != "PENDIENTE" {
        return Err(conflict("REFUND_NOT_PENDING", "refund was already decided"));
    }
    Ok(())
}

pub async fn approve(pool: &PgPool, claims: &Claims, refund_id: Uuid) -> Result<Refund, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:refund")?;
    lock_pending_tx(&mut tx, &scope, refund_id).await?;
    apply_effects_tx(&mut tx, &scope, refund_id).await?;
    audit::record_tx(
        &mut tx,
        &scope,
        "DEVOLUCION_APROBADA",
        "refunds",
        refund_id,
        None,
        None,
        None,
    )
    .await?;
    let refund = load_tx(&mut tx, refund_id).await?;
    tx.commit().await?;
    Ok(refund)
}

pub async fn reject(
    pool: &PgPool,
    claims: &Claims,
    refund_id: Uuid,
    req: RejectRequest,
) -> Result<Refund, ApiError> {
    let reason = text_between(&req.reason, 3, 200)
        .ok_or_else(|| invalid("REFUND_INVALID", "reason must have 3 to 200 characters"))?;
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:refund")?;
    lock_pending_tx(&mut tx, &scope, refund_id).await?;
    sqlx::query(
        "update refunds
            set status = 'RECHAZADA', reject_reason = $2, decided_by = $3, decided_at = now()
          where id = $1",
    )
    .bind(refund_id)
    .bind(&reason)
    .bind(scope.user)
    .execute(&mut *tx)
    .await?;
    audit::record_tx(
        &mut tx,
        &scope,
        "DEVOLUCION_RECHAZADA",
        "refunds",
        refund_id,
        None,
        Some(serde_json::json!({ "reason": reason })),
        None,
    )
    .await?;
    let refund = load_tx(&mut tx, refund_id).await?;
    tx.commit().await?;
    Ok(refund)
}

// -------------------------------------------------------------------- lectura

#[derive(sqlx::FromRow)]
struct RefundRow {
    id: Uuid,
    order_id: Uuid,
    receipt_prefix: Option<String>,
    receipt_number: Option<i64>,
    prefix: String,
    number: i64,
    status: String,
    method: String,
    original_method: Option<String>,
    reversal_reference: Option<String>,
    reason: String,
    subtotal: Decimal,
    tax_amount: Decimal,
    total: Decimal,
    requested_by_name: Option<String>,
    decided_by_name: Option<String>,
    decided_at: Option<DateTime<Utc>>,
    reject_reason: Option<String>,
    created_at: DateTime<Utc>,
}

const REFUND_SELECT: &str =
    "select r.id, r.order_id, o.receipt_prefix, o.receipt_number, r.prefix, r.number,
        r.status, r.method, r.original_method, r.reversal_reference, r.reason, r.subtotal,
        r.tax_amount, r.total, ur.full_name as requested_by_name, ud.full_name as decided_by_name,
        r.decided_at, r.reject_reason, r.created_at
   from refunds r
   join orders o on o.id = r.order_id
   left join users ur on ur.id = r.requested_by
   left join users ud on ud.id = r.decided_by";

async fn hydrate(tx: &mut Tx<'_>, row: RefundRow) -> Result<Refund, ApiError> {
    let items = sqlx::query_as::<_, RefundItem>(
        "select ri.order_item_id, coalesce(p.name, oi.custom_name, '') as name, ri.qty,
                ri.disposition, ri.line_total, ri.tax_amount
           from refund_items ri
           join order_items oi on oi.id = ri.order_item_id
           left join products p on p.id = oi.product_id
          where ri.refund_id = $1
          order by oi.created_at, oi.id",
    )
    .bind(row.id)
    .fetch_all(&mut **tx)
    .await?;
    let receipt_number = match (row.receipt_prefix, row.receipt_number) {
        (Some(p), Some(n)) => Some(format!("{p}-{n:06}")),
        _ => None,
    };
    Ok(Refund {
        id: row.id,
        order_id: row.order_id,
        receipt_number,
        number: format!("{}-{:06}", row.prefix, row.number),
        status: row.status,
        method: row.method,
        original_method: row.original_method,
        reversal_reference: row.reversal_reference,
        reason: row.reason,
        subtotal: row.subtotal,
        tax_amount: row.tax_amount,
        total: row.total,
        items,
        requested_by_name: row.requested_by_name,
        decided_by_name: row.decided_by_name,
        decided_at: row.decided_at,
        reject_reason: row.reject_reason,
        created_at: row.created_at,
    })
}

pub async fn load_tx(tx: &mut Tx<'_>, refund_id: Uuid) -> Result<Refund, ApiError> {
    let row = sqlx::query_as::<_, RefundRow>(&format!("{REFUND_SELECT} where r.id = $1"))
        .bind(refund_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(ApiError::Domain(DomainError::NotFound))?;
    hydrate(tx, row).await
}

async fn load_many(tx: &mut Tx<'_>, rows: Vec<RefundRow>) -> Result<Vec<Refund>, ApiError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(hydrate(tx, row).await?);
    }
    Ok(out)
}

/// `GET /v1/refunds?branch_id=&status=` (pos:refund).
pub async fn list(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    status: Option<String>,
) -> Result<Vec<Refund>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("pos:refund")?;
    let rows = sqlx::query_as::<_, RefundRow>(&format!(
        "{REFUND_SELECT} where r.branch_id = $1 and ($2::text is null or r.status = $2)
          order by r.created_at desc limit 200"
    ))
    .bind(branch_id)
    .bind(status)
    .fetch_all(&mut *tx)
    .await?;
    let out = load_many(&mut tx, rows).await?;
    tx.commit().await?;
    Ok(out)
}

/// Devoluciones de una venta (para el detalle del historial). Sin control de
/// permiso: lo hacen quienes llaman.
pub async fn for_order_tx(tx: &mut Tx<'_>, order_id: Uuid) -> Result<Vec<Refund>, ApiError> {
    let rows = sqlx::query_as::<_, RefundRow>(&format!(
        "{REFUND_SELECT} where r.order_id = $1 order by r.created_at"
    ))
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    load_many(tx, rows).await
}

/// `GET /v1/orders/{id}/refunds` (pos:sell).
pub async fn for_order(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
) -> Result<Vec<Refund>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:sell")?;
    let out = for_order_tx(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok(out)
}

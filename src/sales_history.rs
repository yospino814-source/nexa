//! Historial de ventas (F1.13, pantalla 04 `/ordenes`).
//!
//! Exige `cash:history`. Quien no tiene `reports:read` solo ve las ventas que
//! cobró él (`payments.created_by`), aunque pida otro cajero.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::customers::Customer;
use crate::error::{ApiError, DomainError, validation};
use crate::refunds::{self, Refund};
use crate::sales::{self, SaleResponse};
use crate::tenancy;

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub branch_id: Uuid,
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    #[serde(default)]
    pub cashier_id: Option<Uuid>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SaleRow {
    pub id: Uuid,
    pub receipt_number: String,
    pub created_at: DateTime<Utc>,
    pub cashier_id: Option<Uuid>,
    pub cashier_name: Option<String>,
    pub customer_label: String,
    pub total: Decimal,
    pub refunded_amount: Decimal,
    pub methods: Vec<String>,
    pub status: String,
    pub fiscal_status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Totals {
    pub count: i64,
    pub amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct HistoryPage {
    pub items: Vec<SaleRow>,
    pub total_count: i64,
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct SaleDetail {
    #[serde(flatten)]
    pub sale: SaleResponse,
    pub refunds: Vec<Refund>,
    pub customer: Option<Customer>,
    pub cashier_name: Option<String>,
    pub register_name: Option<String>,
    pub waiter_name: Option<String>,
}

/// Filtro común de la lista: sucursal, fechas locales, estado, método, cajero,
/// número y visibilidad por cajero. Los parámetros van de $1 a $9.
const FILTER: &str = "
   from orders o
   join branches b on b.id = o.branch_id
   join tbl_tipos_maestra st on st.tima_id = o.status_id
  where o.branch_id = $1
    and o.receipt_number is not null
    and st.tima_codigo in ('COBRADA', 'CERRADA', 'CANCELADA')
    and (o.created_at at time zone b.timezone)::date
        between coalesce($2::date, (now() at time zone b.timezone)::date)
            and coalesce($3::date, (now() at time zone b.timezone)::date)
    and ($4::text is null
         or ($4 = 'COBRADA' and st.tima_codigo in ('COBRADA', 'CERRADA'))
         or ($4 = 'CANCELADA' and st.tima_codigo = 'CANCELADA'))
    and ($5::text is null or exists (
          select 1 from payments p join tbl_tipos_maestra pm on pm.tima_id = p.method_id
           where p.order_id = o.id and pm.tima_codigo = $5))
    and ($6::uuid is null or exists (
          select 1 from payments p where p.order_id = o.id and p.created_by = $6))
    and ($7::bigint is null or o.receipt_number = $7)
    and ($8::uuid is null or exists (
          select 1 from payments p where p.order_id = o.id and p.created_by = $8))";

/// `POS-000123` o `123` → 123. Sin dígitos → `None`.
fn parse_receipt(q: &str) -> Option<i64> {
    let digits: String = q
        .rsplit('-')
        .next()
        .unwrap_or(q)
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

pub async fn list(
    pool: &PgPool,
    claims: &Claims,
    q: HistoryQuery,
) -> Result<HistoryPage, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, q.branch_id).await?;
    scope.require_permission("cash:history")?;

    if let Some(m) = &q.method
        && !matches!(m.as_str(), "EFECTIVO" | "DATAFONO" | "QR")
    {
        return Err(validation("method must be EFECTIVO, DATAFONO or QR"));
    }
    if let Some(s) = &q.status
        && !matches!(s.as_str(), "COBRADA" | "CANCELADA")
    {
        return Err(validation("status must be COBRADA or CANCELADA"));
    }
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 100);
    let receipt = match q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(text) => {
            Some(parse_receipt(text).ok_or_else(|| validation("q must be a receipt number"))?)
        }
        None => None,
    };
    // Sin reports:read solo se ven las ventas propias y se ignora cashier_id.
    let all = scope.has_permission("reports:read");
    let cashier = if all { q.cashier_id } else { None };
    let own = if all { None } else { Some(scope.user) };

    let (count, amount): (i64, Decimal) = sqlx::query_as(&format!(
        "select count(*), coalesce(sum(o.total_amount), 0) {FILTER}"
    ))
    .bind(q.branch_id)
    .bind(q.from)
    .bind(q.to)
    .bind(&q.status)
    .bind(&q.method)
    .bind(cashier)
    .bind(receipt)
    .bind(own)
    .fetch_one(&mut *tx)
    .await?;

    let items = sqlx::query_as::<_, SaleRow>(&format!(
        "select o.id,
                o.receipt_prefix || '-' || lpad(o.receipt_number::text, 6, '0') as receipt_number,
                o.created_at,
                (select p.created_by from payments p
                  where p.order_id = o.id order by p.created_at, p.id limit 1) as cashier_id,
                (select u.full_name from payments p join users u on u.id = p.created_by
                  where p.order_id = o.id order by p.created_at, p.id limit 1) as cashier_name,
                coalesce((select c.name from customers c where c.id = o.customer_id),
                         o.customer_alias, 'Consumidor final') as customer_label,
                o.total_amount as total,
                o.refunded_amount,
                coalesce((select array_agg(distinct pm.tima_codigo order by pm.tima_codigo)
                            from payments p join tbl_tipos_maestra pm on pm.tima_id = p.method_id
                           where p.order_id = o.id), '{{}}') as methods,
                st.tima_codigo as status,
                (select ft.tima_codigo from fiscal_documents f
                   join tbl_tipos_maestra ft on ft.tima_id = f.status_id
                  where f.order_id = o.id and f.doc_type = 'POS' and f.refund_id is null) as fiscal_status
         {FILTER}
         order by o.created_at desc
         limit $9 offset $10"
    ))
    .bind(q.branch_id)
    .bind(q.from)
    .bind(q.to)
    .bind(&q.status)
    .bind(&q.method)
    .bind(cashier)
    .bind(receipt)
    .bind(own)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(HistoryPage {
        items,
        total_count: count,
        totals: Totals { count, amount },
    })
}

pub async fn detail(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
) -> Result<SaleDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("cash:history")?;

    let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "select branch_id, customer_id from orders where id = $1 and receipt_number is not null",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (branch, customer_id) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    tenancy::ensure_branch_tx(&mut tx, &scope, branch).await?;

    if !scope.has_permission("reports:read") {
        let own: bool = sqlx::query_scalar(
            "select exists (select 1 from payments where order_id = $1 and created_by = $2)",
        )
        .bind(order_id)
        .bind(scope.user)
        .fetch_one(&mut *tx)
        .await?;
        if !own {
            return Err(ApiError::Domain(DomainError::Forbidden));
        }
    }

    let sale = sales::load_response(&mut tx, order_id).await?;
    let refunds = refunds::for_order_tx(&mut tx, order_id).await?;
    let customer = match customer_id {
        Some(id) => sqlx::query_as::<_, Customer>(
            "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id from customers where id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?,
        None => None,
    };
    let cashier_name: Option<String> = sqlx::query_scalar(
        "select u.full_name from payments p join users u on u.id = p.created_by
          where p.order_id = $1 order by p.created_at, p.id limit 1",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (register_name, waiter_name): (Option<String>, Option<String>) = sqlx::query_as(
        "select r.name, w.full_name
           from orders o
           left join cash_sessions s on s.id = o.cash_session_id
           left join cash_registers r on r.id = s.register_id
           left join users w on w.id = o.waiter_id
          where o.id = $1",
    )
    .bind(order_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(SaleDetail {
        sale,
        refunds,
        customer,
        cashier_name,
        register_name,
        waiter_name,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_receipt;

    #[test]
    fn parse_receipt_accepts_both_forms() {
        assert_eq!(parse_receipt("POS-000123"), Some(123));
        assert_eq!(parse_receipt("123"), Some(123));
        assert_eq!(parse_receipt("abc"), None);
    }
}

use axum::http::StatusCode;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit;
use crate::auth::Claims;
use crate::cash::{self, CashMovementKind};
use crate::error::{self, ApiError};
use crate::money::money;
use crate::tenancy::{self, Scope};

pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateReceivablePaymentRequest {
    pub id: Uuid,
    pub amount: Decimal,
    pub method: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceivablesFilter {
    pub customer_id: Option<Uuid>,
    pub status: Option<String>,
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationItem {
    pub n: i32,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextDue {
    pub n: i32,
    pub due_date: NaiveDate,
    pub amount_pending: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivablePaymentResponse {
    pub id: Uuid,
    pub receivable_id: Uuid,
    pub amount: Decimal,
    pub method: String,
    pub reference: Option<String>,
    pub allocation: Vec<AllocationItem>,
    pub created_at: DateTime<Utc>,
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    pub next_due: Option<NextDue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReceivableRow {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub branch_name: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub source: String,
    pub order_id: Option<Uuid>,
    pub receipt_number: Option<String>,
    pub principal: Decimal,
    pub amount_total: Decimal,
    pub amount_paid: Decimal,
    pub balance: Decimal,
    pub status: String,
    pub due_date: NaiveDate,
    pub days_overdue: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivableInstallmentItem {
    pub n: i32,
    pub due_date: NaiveDate,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub paid: bool,
    pub days_overdue: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivablePaymentItem {
    pub id: Uuid,
    pub amount: Decimal,
    pub method: String,
    pub reference: Option<String>,
    pub allocation: Vec<AllocationItem>,
    pub created_at: DateTime<Utc>,
    pub created_by_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivableDetail {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub branch_name: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub source: String,
    pub order_id: Option<Uuid>,
    pub receipt_number: Option<String>,
    pub principal: Decimal,
    pub amount_total: Decimal,
    pub amount_paid: Decimal,
    pub balance: Decimal,
    pub status: String,
    pub due_date: NaiveDate,
    pub days_overdue: i32,
    pub created_at: DateTime<Utc>,
    pub installments: Vec<ReceivableInstallmentItem>,
    pub payments: Vec<ReceivablePaymentItem>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
struct ReceivableLockRow {
    id: Uuid,
    branch_id: Uuid,
    customer_id: Uuid,
    principal: Decimal,
    interest_total: Decimal,
    amount_total: Decimal,
    amount_paid: Decimal,
    status: String,
    due_date: NaiveDate,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
struct InstallmentLockRow {
    n: i32,
    due_date: NaiveDate,
    amount: Decimal,
    amount_paid: Decimal,
}

/// Registra un abono a una cuenta por cobrar.
pub async fn record_payment(
    pool: &PgPool,
    claims: &Claims,
    receivable_id: Uuid,
    payload: CreateReceivablePaymentRequest,
) -> Result<(StatusCode, ReceivablePaymentResponse), ApiError> {
    // 1. Abre UNA transacción y lee branch_id de la cuenta. Si no existe o RLS bloquea: 404.
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;

    let branch_row: Option<Uuid> =
        sqlx::query_scalar("select branch_id from receivables where id = $1 and tenant_id = $2")
            .bind(receivable_id)
            .bind(scope.tenant)
            .fetch_optional(&mut *tx)
            .await?;

    let branch_id = match branch_row {
        Some(bid) => bid,
        None => {
            return Err(error::coded(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Cuenta por cobrar no encontrada",
                None,
            ));
        }
    };

    tenancy::ensure_branch_tx(&mut tx, &scope, branch_id).await?;

    // 2. Permiso credit:collect
    scope.require_permission("credit:collect")?;

    // 3. Idempotencia por id
    let client_request_json = serde_json::to_value(&payload).map_err(anyhow::Error::from)?;

    #[derive(sqlx::FromRow)]
    struct ExistingPaymentRow {
        id: Uuid,
        amount: Decimal,
        method: String,
        reference: Option<String>,
        allocation: serde_json::Value,
        created_at: DateTime<Utc>,
        client_request: serde_json::Value,
    }

    let existing: Option<ExistingPaymentRow> = sqlx::query_as(
        "select id, amount, method, reference, allocation, created_at, client_request
           from receivable_payments
          where id = $1 and tenant_id = $2",
    )
    .bind(payload.id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(prev) = existing {
        if prev.client_request == client_request_json {
            let rec_opt = sqlx::query_as::<_, ReceivableLockRow>(
                "select id, branch_id, customer_id, principal, interest_total, amount_total, amount_paid, status, due_date
                   from receivables
                  where id = $1 and tenant_id = $2",
            )
            .bind(receivable_id)
            .bind(scope.tenant)
            .fetch_optional(&mut *tx)
            .await?;

            let rec = match rec_opt {
                Some(r) => r,
                None => {
                    return Err(error::coded(
                        StatusCode::NOT_FOUND,
                        "NOT_FOUND",
                        "Cuenta por cobrar no encontrada",
                        None,
                    ));
                }
            };

            let balance_after = rec.amount_total - rec.amount_paid;
            let balance_before = balance_after + prev.amount;

            let next_due_row: Option<(i32, NaiveDate, Decimal, Decimal)> = sqlx::query_as(
                "select n, due_date, amount, amount_paid
                   from receivable_installments
                  where receivable_id = $1 and tenant_id = $2 and amount_paid < amount
                  order by n asc
                  limit 1",
            )
            .bind(receivable_id)
            .bind(scope.tenant)
            .fetch_optional(&mut *tx)
            .await?;

            let next_due = next_due_row.map(|(n, due_date, amount, amount_paid)| NextDue {
                n,
                due_date,
                amount_pending: amount - amount_paid,
            });

            let allocation: Vec<AllocationItem> =
                serde_json::from_value(prev.allocation).unwrap_or_default();

            tx.commit().await?;

            return Ok((
                StatusCode::OK,
                ReceivablePaymentResponse {
                    id: prev.id,
                    receivable_id,
                    amount: prev.amount,
                    method: prev.method,
                    reference: prev.reference,
                    allocation,
                    created_at: prev.created_at,
                    balance_before,
                    balance_after,
                    next_due,
                },
            ));
        } else {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "El id del abono ya fue utilizado con una solicitud diferente",
                None,
            ));
        }
    }

    // 4. method = "SALDO": 422 PAYMENT_METHOD_NOT_AVAILABLE
    let method = payload.method.trim().to_uppercase();
    if method == "SALDO" {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_METHOD_NOT_AVAILABLE",
            "Ese método no está disponible para abonos.",
            None,
        ));
    }

    // 5. Bloquea la cuenta: select ... from receivables where id = $1 and tenant_id = $2 for update
    let rec_opt = sqlx::query_as::<_, ReceivableLockRow>(
        "select id, branch_id, customer_id, principal, interest_total, amount_total, amount_paid, status, due_date
           from receivables
          where id = $1 and tenant_id = $2
          for update",
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let rec = match rec_opt {
        Some(r) => r,
        None => {
            return Err(error::coded(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Cuenta por cobrar no encontrada",
                None,
            ));
        }
    };

    if rec.status != "ABIERTA" && rec.status != "PARCIAL" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "RECEIVABLE_CLOSED",
            "La cuenta ya está cerrada.",
            None,
        ));
    }

    // 6. amount = money(amount). Si es <= 0, o > saldo: 422 PAYMENT_EXCEEDS_BALANCE con data: {balance}
    let amount = money(payload.amount);
    let balance = rec.amount_total - rec.amount_paid;
    if amount <= Decimal::ZERO || amount > balance {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_EXCEEDS_BALANCE",
            "El abono supera el saldo de la cuenta.",
            Some(serde_json::json!({ "balance": balance })),
        ));
    }

    // 7. DATAFONO, QR y TRANSFERENCIA exigen reference (3 a 60 chars)
    let ref_trimmed = payload.reference.as_deref().unwrap_or("").trim();
    if matches!(method.as_str(), "DATAFONO" | "QR" | "TRANSFERENCIA")
        && (ref_trimmed.len() < 3 || ref_trimmed.len() > 60)
    {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_REFERENCE_REQUIRED",
            "Se requiere una referencia válida (de 3 a 60 caracteres).",
            None,
        ));
    }
    let reference_clean = if ref_trimmed.is_empty() {
        None
    } else {
        Some(ref_trimmed.to_string())
    };

    // 8. Aplicación a cuotas: orden n asc, con for update
    let installments = sqlx::query_as::<_, InstallmentLockRow>(
        "select n, due_date, amount, amount_paid
           from receivable_installments
          where receivable_id = $1 and tenant_id = $2
          order by n asc
          for update",
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    let mut allocation: Vec<AllocationItem> = Vec::new();
    let mut remaining = amount;

    for inst in installments {
        if remaining <= Decimal::ZERO {
            break;
        }
        let needed = inst.amount - inst.amount_paid;
        if needed > Decimal::ZERO {
            let applied = remaining.min(needed);
            sqlx::query(
                "update receivable_installments
                    set amount_paid = amount_paid + $1
                  where receivable_id = $2 and n = $3 and tenant_id = $4",
            )
            .bind(applied)
            .bind(receivable_id)
            .bind(inst.n)
            .bind(scope.tenant)
            .execute(&mut *tx)
            .await?;

            allocation.push(AllocationItem {
                n: inst.n,
                amount: applied,
            });
            remaining -= applied;
        }
    }

    // 9. amount_paid += amount. Estado: PAGADA si saldo == 0, sino PARCIAL
    let new_amount_paid = rec.amount_paid + amount;
    let new_status = if new_amount_paid >= rec.amount_total {
        "PAGADA"
    } else {
        "PARCIAL"
    };

    // 10. due_date pasa a ser el vencimiento de la primera cuota que queda con saldo
    let next_due_row: Option<(i32, NaiveDate, Decimal, Decimal)> = sqlx::query_as(
        "select n, due_date, amount, amount_paid
           from receivable_installments
          where receivable_id = $1 and tenant_id = $2 and amount_paid < amount
          order by n asc
          limit 1",
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let next_due = next_due_row.map(|(n, due_date, amount, amount_paid)| NextDue {
        n,
        due_date,
        amount_pending: amount - amount_paid,
    });

    if let Some(ref nd) = next_due {
        sqlx::query(
            "update receivables
                set amount_paid = $1, status = $2, due_date = $3
              where id = $4 and tenant_id = $5",
        )
        .bind(new_amount_paid)
        .bind(new_status)
        .bind(nd.due_date)
        .bind(receivable_id)
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "update receivables
                set amount_paid = $1, status = $2
              where id = $3 and tenant_id = $4",
        )
        .bind(new_amount_paid)
        .bind(new_status)
        .bind(receivable_id)
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;
    }

    // 11. Efectivo: turno abierto, movimiento ABONO_CARTERA y cash_session_id
    let cash_session_id = if method == "EFECTIVO" {
        let open_session = cash::open_session_tx(&mut tx, &scope, branch_id).await?;
        let sid = match open_session {
            Some(s) => s,
            None => {
                return Err(error::coded(
                    StatusCode::CONFLICT,
                    "PAYMENT_NEEDS_OPEN_SESSION",
                    "Para recibir un abono en efectivo, abrí la caja.",
                    None,
                ));
            }
        };

        let cust_name_opt: Option<String> =
            sqlx::query_scalar("select name from customers where id = $1 and tenant_id = $2")
                .bind(rec.customer_id)
                .bind(scope.tenant)
                .fetch_optional(&mut *tx)
                .await?;
        let customer_name = cust_name_opt.unwrap_or_else(|| "Cliente".into());

        cash::record_movement_tx(
            &mut tx,
            &scope,
            sid,
            CashMovementKind::AbonoCartera,
            amount,
            &format!("Abono {}", customer_name),
            None,
            Some("RECEIVABLE"),
            Some(receivable_id),
        )
        .await?;

        Some(sid)
    } else {
        None
    };

    // 12. Inserta receivable_payments
    let allocation_json = serde_json::to_value(&allocation).map_err(anyhow::Error::from)?;

    let created_at: DateTime<Utc> = sqlx::query_scalar(
        "insert into receivable_payments
           (id, tenant_id, branch_id, receivable_id, amount, method, reference, cash_session_id, allocation, created_by, client_request)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         returning created_at",
    )
    .bind(payload.id)
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(receivable_id)
    .bind(amount)
    .bind(&method)
    .bind(&reference_clean)
    .bind(cash_session_id)
    .bind(allocation_json)
    .bind(scope.user)
    .bind(client_request_json)
    .fetch_one(&mut *tx)
    .await?;

    // 13. Auditoría ABONO_CARTERA
    audit::record_tx(
        &mut tx,
        &scope,
        "ABONO_CARTERA",
        "receivables",
        receivable_id,
        None,
        Some(serde_json::json!({
            "payment_id": payload.id,
            "amount": amount,
            "method": method,
        })),
        None,
    )
    .await?;

    tx.commit().await?;

    let balance_before = balance;
    let balance_after = balance_before - amount;

    // 14. Respuesta 201
    Ok((
        StatusCode::CREATED,
        ReceivablePaymentResponse {
            id: payload.id,
            receivable_id,
            amount,
            method,
            reference: reference_clean,
            allocation,
            created_at,
            balance_before,
            balance_after,
            next_due,
        },
    ))
}

/// Lista cuentas por cobrar según filtros.
pub async fn list_receivables_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    filter: ReceivablesFilter,
) -> Result<Vec<ReceivableRow>, ApiError> {
    scope.require_permission("customers:read")?;

    let status_filter = filter.status.as_deref().map(|s| s.trim().to_uppercase());

    let rows = sqlx::query_as::<_, ReceivableRow>(
        r#"
        with q as (
            select
                r.id,
                r.branch_id,
                b.name as branch_name,
                r.customer_id,
                c.name as customer_name,
                r.source,
                r.order_id,
                case
                    when r.prefix is not null and r.number is not null then r.prefix || '-' || lpad(r.number::text, 6, '0')
                    when o.receipt_prefix is not null and o.receipt_number is not null then o.receipt_prefix || '-' || lpad(o.receipt_number::text, 6, '0')
                    when r.number is not null then r.number::text
                    when o.receipt_number is not null then o.receipt_number::text
                    else null
                end as receipt_number,
                r.principal,
                r.amount_total,
                r.amount_paid,
                (r.amount_total - r.amount_paid) as balance,
                r.status,
                r.due_date,
                case
                    when (r.amount_total - r.amount_paid) <= 0 then 0
                    else greatest(0, ((now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)))
                end as days_overdue,
                r.created_at
            from receivables r
            join branches b on b.id = r.branch_id
            join customers c on c.id = r.customer_id
            left join orders o on o.id = r.order_id
            left join lateral (
                select min(ri.due_date) as first_due
                  from receivable_installments ri
                 where ri.receivable_id = r.id and ri.amount_paid < ri.amount
            ) i on true
            where r.tenant_id = $1
              and ($2::uuid is null or r.customer_id = $2)
              and (
                  case
                      when $3::text is not null then r.status = $3
                      else r.status in ('ABIERTA', 'PARCIAL')
                  end
              )
        )
        select * from q
        where ($4::boolean is null or not $4 or days_overdue > 0)
        order by days_overdue desc, created_at asc
        "#,
    )
    .bind(scope.tenant)
    .bind(filter.customer_id)
    .bind(status_filter)
    .bind(filter.overdue)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows)
}

/// Detalle de una cuenta por cobrar con cuotas y abonos.
pub async fn get_receivable_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    receivable_id: Uuid,
) -> Result<ReceivableDetail, ApiError> {
    scope.require_permission("customers:read")?;

    let row_opt: Option<ReceivableRow> = sqlx::query_as(
        r#"
        select
            r.id,
            r.branch_id,
            b.name as branch_name,
            r.customer_id,
            c.name as customer_name,
            r.source,
            r.order_id,
            case
                when r.prefix is not null and r.number is not null then r.prefix || '-' || lpad(r.number::text, 6, '0')
                when o.receipt_prefix is not null and o.receipt_number is not null then o.receipt_prefix || '-' || lpad(o.receipt_number::text, 6, '0')
                when r.number is not null then r.number::text
                when o.receipt_number is not null then o.receipt_number::text
                else null
            end as receipt_number,
            r.principal,
            r.amount_total,
            r.amount_paid,
            (r.amount_total - r.amount_paid) as balance,
            r.status,
            r.due_date,
            case
                when (r.amount_total - r.amount_paid) <= 0 then 0
                else greatest(0, ((now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)))
            end as days_overdue,
            r.created_at
        from receivables r
        join branches b on b.id = r.branch_id
        join customers c on c.id = r.customer_id
        left join orders o on o.id = r.order_id
        left join lateral (
            select min(ri.due_date) as first_due
              from receivable_installments ri
             where ri.receivable_id = r.id and ri.amount_paid < ri.amount
        ) i on true
        where r.id = $1 and r.tenant_id = $2
        "#,
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let row = match row_opt {
        Some(r) => r,
        None => {
            return Err(error::coded(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Cuenta por cobrar no encontrada",
                None,
            ));
        }
    };

    #[derive(sqlx::FromRow)]
    struct RawInstallment {
        n: i32,
        due_date: NaiveDate,
        amount: Decimal,
        amount_paid: Decimal,
        paid: bool,
        days_overdue: i32,
    }

    let raw_installments: Vec<RawInstallment> = sqlx::query_as(
        r#"
        select
            ri.n,
            ri.due_date,
            ri.amount,
            ri.amount_paid,
            (ri.amount_paid >= ri.amount) as paid,
            case
                when (ri.amount - ri.amount_paid) <= 0 then 0
                else greatest(0, ((now() at time zone b.timezone)::date - ri.due_date))
            end as days_overdue
        from receivable_installments ri
        join receivables r on r.id = ri.receivable_id
        join branches b on b.id = r.branch_id
        where ri.receivable_id = $1 and ri.tenant_id = $2
        order by ri.n asc
        "#,
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    let installments = raw_installments
        .into_iter()
        .map(|ri| ReceivableInstallmentItem {
            n: ri.n,
            due_date: ri.due_date,
            amount: ri.amount,
            amount_paid: ri.amount_paid,
            paid: ri.paid,
            days_overdue: ri.days_overdue,
        })
        .collect();

    #[derive(sqlx::FromRow)]
    struct RawPayment {
        id: Uuid,
        amount: Decimal,
        method: String,
        reference: Option<String>,
        allocation: serde_json::Value,
        created_at: DateTime<Utc>,
        created_by_name: String,
    }

    let raw_payments: Vec<RawPayment> = sqlx::query_as(
        r#"
        select
            p.id,
            p.amount,
            p.method,
            p.reference,
            p.allocation,
            p.created_at,
            coalesce(u.full_name, 'Usuario') as created_by_name
        from receivable_payments p
        left join users u on u.id = p.created_by
        where p.receivable_id = $1 and p.tenant_id = $2
        order by p.created_at asc
        "#,
    )
    .bind(receivable_id)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    let payments = raw_payments
        .into_iter()
        .map(|rp| ReceivablePaymentItem {
            id: rp.id,
            amount: rp.amount,
            method: rp.method,
            reference: rp.reference,
            allocation: serde_json::from_value(rp.allocation).unwrap_or_default(),
            created_at: rp.created_at,
            created_by_name: rp.created_by_name,
        })
        .collect();

    Ok(ReceivableDetail {
        id: row.id,
        branch_id: row.branch_id,
        branch_name: row.branch_name,
        customer_id: row.customer_id,
        customer_name: row.customer_name,
        source: row.source,
        order_id: row.order_id,
        receipt_number: row.receipt_number,
        principal: row.principal,
        amount_total: row.amount_total,
        amount_paid: row.amount_paid,
        balance: row.balance,
        status: row.status,
        due_date: row.due_date,
        days_overdue: row.days_overdue,
        created_at: row.created_at,
        installments,
        payments,
    })
}

use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use chrono::Duration;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::audit;
use crate::auth::Claims;
use crate::cash;
use crate::error::{self, ApiError, DomainError};
use crate::money;
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayableRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
    pub supplier_id: Option<Uuid>,
    pub supplier_name: Option<String>,
    pub source_type: String,
    pub source_id: Uuid,
    pub document_number: Option<String>,
    pub amount_total: Decimal,
    pub amount_paid: Decimal,
    pub balance: Decimal,
    pub status: String,
    pub due_date: chrono::NaiveDate,
    pub overdue: bool,
    pub days_overdue: i32,
    pub written_off_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayablePaymentRow {
    pub id: Uuid,
    pub payable_id: Uuid,
    pub amount: Decimal,
    pub method: String,
    pub reference: Option<String>,
    pub cash_session_id: Option<Uuid>,
    pub paid_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayableDetail {
    #[serde(flatten)]
    pub payable: PayableRow,
    pub payments: Vec<PayablePaymentRow>,
}

#[derive(Debug, Deserialize)]
pub struct PayablesFilter {
    pub branch_id: Option<Uuid>,
    pub status: Option<String>,
    pub supplier_id: Option<Uuid>,
    pub overdue: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AgingFilter {
    pub branch_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PayablesAging {
    pub al_dia: Decimal,
    pub d1_30: Decimal,
    pub d31_60: Decimal,
    pub d61_90: Decimal,
    pub d91_120: Decimal,
    pub d120_mas: Decimal,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePayablePaymentRequest {
    pub id: Uuid,
    pub amount: Decimal,
    pub method: String,
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteOffPayableRequest {
    pub reason: String,
}

/// Actualiza o inserta la cuenta por pagar asociada a una orden de compra recibida.
pub async fn upsert_from_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    purchase_id: Uuid,
) -> Result<(), ApiError> {
    // 1. Encabezado de la compra
    #[derive(sqlx::FromRow)]
    struct PurchaseHeader {
        branch_id: Uuid,
        supplier_id: Uuid,
        payment_terms: String,
        credit_days: i32,
        freight_mode: String,
        received_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let pr = sqlx::query_as::<_, PurchaseHeader>(
        r#"
        select branch_id, supplier_id, payment_terms, credit_days, freight_mode, received_at
          from purchase_receipts
         where id = $1 and tenant_id = $2
        "#,
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    // 2. Líneas y su prorrateo
    #[derive(sqlx::FromRow)]
    struct LineCalc {
        qty: Decimal,
        qty_received: Decimal,
        discount_alloc: Decimal,
        freight_alloc: Decimal,
        tax_amount: Decimal,
        unit_cost: Decimal,
    }

    let items = sqlx::query_as::<_, LineCalc>(
        r#"
        select qty, qty_received, discount_alloc, freight_alloc, tax_amount, unit_cost
          from purchase_receipt_items
         where purchase_receipt_id = $1 and tenant_id = $2
        "#,
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    let mut amount_total = Decimal::ZERO;
    for it in items {
        if it.qty > Decimal::ZERO && it.qty_received > Decimal::ZERO {
            let gross = money::money(it.qty * it.unit_cost);
            let net = (gross - it.discount_alloc).max(Decimal::ZERO);
            let freight = if pr.freight_mode == "GASTO" {
                Decimal::ZERO
            } else {
                it.freight_alloc
            };
            let line_cost = net + freight + it.tax_amount;
            let ratio = it.qty_received / it.qty;
            amount_total += money::money(ratio * line_cost);
        }
    }

    if amount_total <= Decimal::ZERO {
        return Ok(());
    }

    let first_recv_date = pr
        .received_at
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let due_date = if pr.payment_terms == "CREDITO" {
        first_recv_date + Duration::days(pr.credit_days.max(0) as i64)
    } else {
        first_recv_date
    };

    #[derive(sqlx::FromRow)]
    struct ExistingPayable {
        id: Uuid,
        amount_paid: Decimal,
    }

    let existing = sqlx::query_as::<_, ExistingPayable>(
        "select id, amount_paid from payables where source_type = 'COMPRA' and source_id = $1 and tenant_id = $2 for update",
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(p) = existing {
        let new_status = if p.amount_paid >= amount_total {
            "PAGADA"
        } else if p.amount_paid > Decimal::ZERO {
            "PARCIAL"
        } else {
            "ABIERTA"
        };
        sqlx::query(
            "update payables set amount_total = $1, status = $2, due_date = $3 where id = $4",
        )
        .bind(amount_total)
        .bind(new_status)
        .bind(due_date)
        .bind(p.id)
        .execute(&mut **tx)
        .await?;
    } else {
        let new_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into payables (
                id, tenant_id, branch_id, supplier_id, source_type, source_id,
                amount_total, amount_paid, status, due_date
            ) values (
                $1, $2, $3, $4, 'COMPRA', $5,
                $6, 0, 'ABIERTA', $7
            )
            "#,
        )
        .bind(new_id)
        .bind(scope.tenant)
        .bind(pr.branch_id)
        .bind(pr.supplier_id)
        .bind(purchase_id)
        .bind(amount_total)
        .bind(due_date)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn list_payables_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    filter: PayablesFilter,
) -> Result<Vec<PayableRow>, ApiError> {
    if !scope.has_permission("purchases:pay") && !scope.has_permission("reports:read") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let rows = sqlx::query_as::<_, PayableRow>(
        r#"
        select
            p.id,
            p.tenant_id,
            p.branch_id,
            p.supplier_id,
            s.name as supplier_name,
            p.source_type,
            p.source_id,
            case
                when p.source_type = 'COMPRA' then pr.document_number
                when p.source_type = 'GASTO' then e.prefix || '-' || e.number
                else null
            end as document_number,
            p.amount_total,
            p.amount_paid,
            (p.amount_total - p.amount_paid) as balance,
            p.status,
            p.due_date,
            (p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL')) as overdue,
            case
                when p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL')
                    then (current_date - p.due_date)
                else 0
            end as days_overdue,
            p.written_off_reason,
            p.created_at
        from payables p
        left join suppliers s on s.id = p.supplier_id
        left join purchase_receipts pr on pr.id = p.source_id and p.source_type = 'COMPRA'
        left join expenses e on e.id = p.source_id and p.source_type = 'GASTO'
        where p.tenant_id = $1
          and ($2::uuid is null or p.branch_id = $2)
          and ($3::text is null or p.status = $3)
          and ($4::uuid is null or p.supplier_id = $4)
          and (
              $5::boolean is null
              or ($5 = true and p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL'))
              or ($5 = false and not (p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL')))
          )
        order by p.due_date asc, p.created_at desc
        "#,
    )
    .bind(scope.tenant)
    .bind(filter.branch_id)
    .bind(filter.status)
    .bind(filter.supplier_id)
    .bind(filter.overdue)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows)
}

pub async fn get_payable_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    payable_id: Uuid,
) -> Result<PayableDetail, ApiError> {
    if !scope.has_permission("purchases:pay") && !scope.has_permission("reports:read") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let payable = sqlx::query_as::<_, PayableRow>(
        r#"
        select
            p.id,
            p.tenant_id,
            p.branch_id,
            p.supplier_id,
            s.name as supplier_name,
            p.source_type,
            p.source_id,
            case
                when p.source_type = 'COMPRA' then pr.document_number
                when p.source_type = 'GASTO' then e.prefix || '-' || e.number
                else null
            end as document_number,
            p.amount_total,
            p.amount_paid,
            (p.amount_total - p.amount_paid) as balance,
            p.status,
            p.due_date,
            (p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL')) as overdue,
            case
                when p.due_date < current_date and p.amount_paid < p.amount_total and p.status in ('ABIERTA','PARCIAL')
                    then (current_date - p.due_date)
                else 0
            end as days_overdue,
            p.written_off_reason,
            p.created_at
        from payables p
        left join suppliers s on s.id = p.supplier_id
        left join purchase_receipts pr on pr.id = p.source_id and p.source_type = 'COMPRA'
        left join expenses e on e.id = p.source_id and p.source_type = 'GASTO'
        where p.id = $1 and p.tenant_id = $2
        "#,
    )
    .bind(payable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let payments = sqlx::query_as::<_, PayablePaymentRow>(
        r#"
        select
            pp.id,
            pp.payable_id,
            pp.amount,
            pp.method,
            pp.reference,
            pp.cash_session_id,
            pp.paid_at,
            pp.created_by,
            u.full_name as created_by_name
        from payable_payments pp
        left join users u on u.id = pp.created_by
        where pp.payable_id = $1 and pp.tenant_id = $2
        order by pp.paid_at desc
        "#,
    )
    .bind(payable_id)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    Ok(PayableDetail { payable, payments })
}

pub async fn aging_summary_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Option<Uuid>,
) -> Result<PayablesAging, ApiError> {
    if !scope.has_permission("purchases:pay") && !scope.has_permission("reports:read") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let row = sqlx::query_as::<_, PayablesAging>(
        r#"
        select
            coalesce(sum(case when p.due_date >= current_date then (p.amount_total - p.amount_paid) else 0 end), 0) as al_dia,
            coalesce(sum(case when current_date - p.due_date between 1 and 30 then (p.amount_total - p.amount_paid) else 0 end), 0) as d1_30,
            coalesce(sum(case when current_date - p.due_date between 31 and 60 then (p.amount_total - p.amount_paid) else 0 end), 0) as d31_60,
            coalesce(sum(case when current_date - p.due_date between 61 and 90 then (p.amount_total - p.amount_paid) else 0 end), 0) as d61_90,
            coalesce(sum(case when current_date - p.due_date between 91 and 120 then (p.amount_total - p.amount_paid) else 0 end), 0) as d91_120,
            coalesce(sum(case when current_date - p.due_date > 120 then (p.amount_total - p.amount_paid) else 0 end), 0) as d120_mas
        from payables p
        where p.tenant_id = $1
          and ($2::uuid is null or p.branch_id = $2)
          and p.status in ('ABIERTA', 'PARCIAL')
        "#,
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row)
}

pub async fn create_payment_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    payable_id: Uuid,
    payload: CreatePayablePaymentRequest,
) -> Result<PayableDetail, ApiError> {
    scope.require_permission("purchases:pay")?;

    let current_body = serde_json::to_value(&payload).map_err(|e| ApiError::Internal(e.into()))?;

    // 1. Idempotencia por id
    #[derive(sqlx::FromRow)]
    struct ExistingPayment {
        client_request: serde_json::Value,
    }

    let existing = sqlx::query_as::<_, ExistingPayment>(
        "select client_request from payable_payments where id = $1 and tenant_id = $2",
    )
    .bind(payload.id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(r) = existing {
        if r.client_request == current_body {
            return get_payable_detail_tx(tx, scope, payable_id).await;
        } else {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "El id del pago ya fue utilizado con un cuerpo diferente",
                None,
            ));
        }
    }

    // 2. Bloquea la cuenta con for update
    #[derive(sqlx::FromRow)]
    struct PayableHeader {
        id: Uuid,
        branch_id: Uuid,
        source_type: String,
        source_id: Uuid,
        amount_total: Decimal,
        amount_paid: Decimal,
        status: String,
    }

    let payable = sqlx::query_as::<_, PayableHeader>(
        "select id, branch_id, source_type, source_id, amount_total, amount_paid, status
           from payables
          where id = $1 and tenant_id = $2
          for update",
    )
    .bind(payable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if payable.status == "PAGADA" || payable.status == "CASTIGADA" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "PAYABLE_CLOSED",
            "La cuenta por pagar se encuentra cerrada (pagada o castigada)",
            Some(serde_json::json!({ "status": payable.status })),
        ));
    }

    // 3. Monto y saldo
    let amount = money::money(payload.amount);
    if amount <= Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_AMOUNT_INVALID",
            "El monto del pago debe ser mayor a cero",
            None,
        ));
    }

    let balance = money::money(payable.amount_total - payable.amount_paid);
    if amount > balance {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_EXCEEDS_BALANCE",
            "El monto del pago supera el saldo pendiente de la cuenta",
            Some(serde_json::json!({ "balance": balance })),
        ));
    }

    // 4. Método y referencia
    let method = payload.method.trim().to_uppercase();
    if !matches!(method.as_str(), "EFECTIVO" | "TRANSFERENCIA" | "CHEQUE") {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_METHOD_INVALID",
            "Método de pago no válido (debe ser EFECTIVO, TRANSFERENCIA o CHEQUE)",
            None,
        ));
    }

    let ref_trimmed = payload.reference.as_deref().unwrap_or("").trim();
    if matches!(method.as_str(), "TRANSFERENCIA" | "CHEQUE")
        && (ref_trimmed.len() < 3 || ref_trimmed.len() > 60)
    {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYMENT_REFERENCE_REQUIRED",
            "Se requiere una referencia válida (de 3 a 60 caracteres) para pagos por transferencia o cheque",
            None,
        ));
    }

    // 5. En EFECTIVO: turno abierto y movimiento en caja
    let session_id = if method == "EFECTIVO" {
        let open_session = cash::open_session_tx(tx, scope, payable.branch_id).await?;
        let sid = match open_session {
            Some(s) => s,
            None => {
                return Err(error::coded(
                    StatusCode::CONFLICT,
                    "PAYMENT_NEEDS_OPEN_SESSION",
                    "El pago en efectivo requiere que el usuario tenga un turno de caja abierto en la sucursal",
                    None,
                ));
            }
        };

        let movement_kind = if payable.source_type == "GASTO" {
            cash::CashMovementKind::Gasto
        } else {
            cash::CashMovementKind::PagoProveedor
        };

        let reason_desc = format!("Pago a proveedor CxP {}", payable.id);
        cash::record_movement_tx(
            tx,
            scope,
            sid,
            movement_kind,
            amount,
            &reason_desc,
            None,
            Some("PAYABLE"),
            Some(payable.id),
        )
        .await?;

        Some(sid)
    } else {
        None
    };

    // 6. Inserta payable_payments
    sqlx::query(
        r#"
        insert into payable_payments (
            id, tenant_id, payable_id, amount, method, reference,
            cash_session_id, paid_at, created_by, client_request
        ) values (
            $1, $2, $3, $4, $5, $6,
            $7, now(), $8, $9
        )
        "#,
    )
    .bind(payload.id)
    .bind(scope.tenant)
    .bind(payable.id)
    .bind(amount)
    .bind(&method)
    .bind(if ref_trimmed.is_empty() {
        None
    } else {
        Some(ref_trimmed)
    })
    .bind(session_id)
    .bind(scope.user)
    .bind(current_body)
    .execute(&mut **tx)
    .await?;

    // 7. Actualiza payables
    let new_paid = money::money(payable.amount_paid + amount);
    let new_status = if new_paid >= payable.amount_total {
        "PAGADA"
    } else {
        "PARCIAL"
    };

    sqlx::query("update payables set amount_paid = $1, status = $2 where id = $3")
        .bind(new_paid)
        .bind(new_status)
        .bind(payable.id)
        .execute(&mut **tx)
        .await?;

    // Si la fuente es GASTO y la cuenta queda PAGADA, el gasto pasa a PAGADO
    if payable.source_type == "GASTO" && new_status == "PAGADA" {
        sqlx::query("update expenses set status = 'PAGADO' where id = $1 and tenant_id = $2")
            .bind(payable.source_id)
            .bind(scope.tenant)
            .execute(&mut **tx)
            .await?;
    }

    // 8. Auditoría PAGO_PROVEEDOR
    audit::record_tx(
        tx,
        scope,
        "PAGO_PROVEEDOR",
        "payables",
        payable.id,
        Some(serde_json::json!({
            "amount_paid": payable.amount_paid,
            "status": payable.status,
        })),
        Some(serde_json::json!({
            "amount_paid": new_paid,
            "status": new_status,
            "payment_id": payload.id,
            "method": method,
            "amount": amount,
        })),
        None,
    )
    .await?;

    get_payable_detail_tx(tx, scope, payable_id).await
}

pub async fn write_off_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    payable_id: Uuid,
    payload: WriteOffPayableRequest,
) -> Result<PayableDetail, ApiError> {
    scope.require_permission("purchases:pay")?;

    if scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let reason = payload.reason.trim();
    if reason.len() < 3 || reason.len() > 200 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PAYABLE_INVALID_REASON",
            "El motivo de castigo debe tener entre 3 y 200 caracteres",
            None,
        ));
    }

    #[derive(sqlx::FromRow)]
    struct PayableHeader {
        id: Uuid,
        status: String,
        amount_paid: Decimal,
        amount_total: Decimal,
    }

    let payable = sqlx::query_as::<_, PayableHeader>(
        "select id, status, amount_paid, amount_total from payables where id = $1 and tenant_id = $2 for update",
    )
    .bind(payable_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if payable.status == "PAGADA" || payable.status == "CASTIGADA" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "PAYABLE_CLOSED",
            "La cuenta por pagar ya se encuentra cerrada (pagada o castigada)",
            Some(serde_json::json!({ "status": payable.status })),
        ));
    }

    sqlx::query("update payables set status = 'CASTIGADA', written_off_reason = $1 where id = $2")
        .bind(reason)
        .bind(payable.id)
        .execute(&mut **tx)
        .await?;

    audit::record_tx(
        tx,
        scope,
        "CXP_CASTIGADA",
        "payables",
        payable.id,
        Some(serde_json::json!({
            "status": payable.status,
            "amount_paid": payable.amount_paid,
            "amount_total": payable.amount_total,
        })),
        Some(serde_json::json!({
            "status": "CASTIGADA",
            "written_off_reason": reason,
        })),
        None,
    )
    .await?;

    get_payable_detail_tx(tx, scope, payable_id).await
}

// ------------------------------------------------------------------- handlers HTTP

pub async fn payables_list(
    pool: axum::extract::Extension<PgPool>,
    claims: Claims,
    Query(filter): Query<PayablesFilter>,
) -> Result<Json<Vec<PayableRow>>, ApiError> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&pool, &claims, bid).await?
    } else {
        tenancy::begin(&pool, &claims).await?
    };

    let rows = list_payables_tx(&mut tx, &scope, filter).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

pub async fn payables_aging(
    pool: axum::extract::Extension<PgPool>,
    claims: Claims,
    Query(filter): Query<AgingFilter>,
) -> Result<Json<PayablesAging>, ApiError> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&pool, &claims, bid).await?
    } else {
        tenancy::begin(&pool, &claims).await?
    };

    let row = aging_summary_tx(&mut tx, &scope, filter.branch_id).await?;
    tx.commit().await?;
    Ok(Json(row))
}

pub async fn payables_get(
    pool: axum::extract::Extension<PgPool>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<PayableDetail>, ApiError> {
    let (scope, mut tx) = tenancy::begin(&pool, &claims).await?;
    let detail = get_payable_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

pub async fn payables_create_payment(
    pool: axum::extract::Extension<PgPool>,
    claims: Claims,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreatePayablePaymentRequest>,
) -> Result<Json<PayableDetail>, ApiError> {
    let (scope, mut tx) = tenancy::begin(&pool, &claims).await?;
    let detail = create_payment_tx(&mut tx, &scope, id, payload).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

pub async fn payables_write_off(
    pool: axum::extract::Extension<PgPool>,
    claims: Claims,
    Path(id): Path<Uuid>,
    Json(payload): Json<WriteOffPayableRequest>,
) -> Result<Json<PayableDetail>, ApiError> {
    let (scope, mut tx) = tenancy::begin(&pool, &claims).await?;
    let detail = write_off_tx(&mut tx, &scope, id, payload).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

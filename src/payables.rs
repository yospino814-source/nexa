use chrono::Duration;
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::{ApiError, DomainError};
use crate::money;
use crate::tenancy::Scope;

type Tx<'a> = Transaction<'a, Postgres>;

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

use axum::http::StatusCode;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::audit;
use crate::error::{self, ApiError, DomainError};
use crate::sequences;
use crate::tenancy::Scope;

type Tx<'a> = Transaction<'a, Postgres>;

fn is_valid_hex_color(c: &str) -> bool {
    if c.len() != 7 || !c.starts_with('#') {
        return false;
    }
    c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseCategory {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpenseCategoryRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpenseCategoryRequest {
    pub name: String,
    pub color: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
    pub prefix: String,
    pub number: i64,
    pub document_number: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub category_color: String,
    pub supplier_id: Option<Uuid>,
    pub supplier_name: Option<String>,
    pub description: String,
    pub amount: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub expense_date: NaiveDate,
    pub status: String,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub reject_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub payable_id: Option<Uuid>,
    pub payable_amount_paid: Option<Decimal>,
    pub payable_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExpenseRequest {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub category_id: Uuid,
    pub supplier_id: Option<Uuid>,
    pub description: String,
    pub amount: Decimal,
    pub tax_amount: Option<Decimal>,
    pub expense_date: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct RejectExpenseRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ExpensesFilter {
    pub branch_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub supplier_id: Option<Uuid>,
    pub status: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseSummaryFilter {
    pub branch_id: Option<Uuid>,
    pub month: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpenseSummary {
    pub total_approved_month: Decimal,
    pub total_pending_month: Decimal,
    pub total_paid_month: Decimal,
    pub count_pending: i64,
    pub count_approved: i64,
}

#[derive(Debug, Deserialize)]
pub struct CategoriesFilter {
    pub include_inactive: Option<bool>,
}

// ---------------------------------------------------------------------------
// CATEGORIES
// ---------------------------------------------------------------------------

pub async fn list_categories_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    include_inactive: bool,
) -> Result<Vec<ExpenseCategory>, ApiError> {
    if !scope.has_permission("expenses:create") && !scope.has_permission("expenses:approve") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let rows = sqlx::query_as::<_, ExpenseCategory>(
        r#"
        select id, name, color, active
          from expense_categories
         where tenant_id = $1
           and ($2 = true or active = true)
         order by name asc
        "#,
    )
    .bind(scope.tenant)
    .bind(include_inactive)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows)
}

pub async fn create_category_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateExpenseCategoryRequest,
) -> Result<ExpenseCategory, ApiError> {
    scope.require_permission("expenses:approve")?;

    let name = req.name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(error::validation(
            "El nombre debe tener entre 1 y 40 caracteres",
        ));
    }

    let color = req.color.unwrap_or_else(|| "#64748B".into());
    if !is_valid_hex_color(&color) {
        return Err(error::validation(
            "El color debe tener formato hexadecimal #RRGGBB",
        ));
    }

    let cat = sqlx::query_as::<_, ExpenseCategory>(
        r#"
        insert into expense_categories (tenant_id, name, color, active)
        values ($1, $2, $3, true)
        returning id, name, color, active
        "#,
    )
    .bind(scope.tenant)
    .bind(name)
    .bind(&color)
    .fetch_one(&mut **tx)
    .await?;

    Ok(cat)
}

pub async fn update_category_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    req: UpdateExpenseCategoryRequest,
) -> Result<ExpenseCategory, ApiError> {
    scope.require_permission("expenses:approve")?;

    let name = req.name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(error::validation(
            "El nombre debe tener entre 1 y 40 caracteres",
        ));
    }

    if !is_valid_hex_color(&req.color) {
        return Err(error::validation(
            "El color debe tener formato hexadecimal #RRGGBB",
        ));
    }

    let cat = sqlx::query_as::<_, ExpenseCategory>(
        r#"
        update expense_categories
           set name = $1,
               color = $2,
               active = $3
         where tenant_id = $4 and id = $5
        returning id, name, color, active
        "#,
    )
    .bind(name)
    .bind(&req.color)
    .bind(req.active)
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    match cat {
        Some(c) => Ok(c),
        None => Err(ApiError::Domain(DomainError::NotFound)),
    }
}

// ---------------------------------------------------------------------------
// EXPENSES
// ---------------------------------------------------------------------------

pub async fn create_expense_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateExpenseRequest,
) -> Result<ExpenseRow, ApiError> {
    scope.require_permission("expenses:create")?;

    // Idempotencia por id y client_request
    let req_val = serde_json::to_value(&req).unwrap_or_default();
    let existing: Option<(serde_json::Value,)> =
        sqlx::query_as("select client_request from expenses where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(req.id)
            .fetch_optional(&mut **tx)
            .await?;

    if let Some((stored_req,)) = existing {
        if stored_req == req_val {
            return get_expense_detail_tx(tx, scope, req.id).await;
        } else {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "Esta operación ya se registró con otros datos.",
                None,
            ));
        }
    }

    let desc = req.description.trim();
    if desc.len() < 3 || desc.len() > 200 {
        return Err(error::validation(
            "La descripción debe tener entre 3 y 200 caracteres",
        ));
    }

    if req.amount <= Decimal::ZERO {
        return Err(error::validation("El monto debe ser mayor a cero"));
    }

    let tax_amount = req.tax_amount.unwrap_or(Decimal::ZERO);
    if tax_amount < Decimal::ZERO {
        return Err(error::validation("El impuesto no puede ser negativo"));
    }

    // Verificar categoría activa
    let cat_active: Option<(bool,)> =
        sqlx::query_as("select active from expense_categories where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(req.category_id)
            .fetch_optional(&mut **tx)
            .await?;

    match cat_active {
        None => return Err(error::validation("La categoría no existe")),
        Some((false,)) => return Err(error::validation("La categoría no está activa")),
        Some((true,)) => {}
    }

    // Verificar proveedor si se indicó
    if let Some(supp_id) = req.supplier_id {
        let supp_exists: bool = sqlx::query_scalar(
            "select exists(select 1 from suppliers where tenant_id = $1 and id = $2)",
        )
        .bind(scope.tenant)
        .bind(supp_id)
        .fetch_one(&mut **tx)
        .await?;
        if !supp_exists {
            return Err(error::validation("El proveedor no existe"));
        }
    }

    let (prefix, number) = sequences::next_tx(tx, scope, req.branch_id, "GASTO").await?;

    sqlx::query(
        r#"
        insert into expenses (
            id, tenant_id, branch_id, prefix, number, category_id, supplier_id,
            description, amount, tax_amount, expense_date, status, created_by, client_request
        ) values (
            $1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, 'PENDIENTE', $12, $13
        )
        "#,
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(&prefix)
    .bind(number)
    .bind(req.category_id)
    .bind(req.supplier_id)
    .bind(desc)
    .bind(req.amount)
    .bind(tax_amount)
    .bind(req.expense_date)
    .bind(scope.user)
    .bind(&req_val)
    .execute(&mut **tx)
    .await?;

    audit::record_tx(
        tx,
        scope,
        "GASTO_CREADO",
        "expenses",
        req.id,
        None,
        Some(serde_json::json!({
            "prefix": prefix,
            "number": number,
            "amount": req.amount,
            "tax_amount": tax_amount,
            "total": req.amount + tax_amount,
        })),
        None,
    )
    .await?;

    get_expense_detail_tx(tx, scope, req.id).await
}

pub async fn approve_expense_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<ExpenseRow, ApiError> {
    scope.require_permission("expenses:approve")?;

    #[derive(sqlx::FromRow)]
    struct ExpLock {
        id: Uuid,
        branch_id: Uuid,
        supplier_id: Option<Uuid>,
        total: Option<Decimal>,
        amount: Decimal,
        tax_amount: Decimal,
        expense_date: NaiveDate,
        status: String,
    }

    let exp = sqlx::query_as::<_, ExpLock>(
        r#"
        select id, branch_id, supplier_id, total, amount, tax_amount, expense_date, status
          from expenses
         where tenant_id = $1 and id = $2
         for update
        "#,
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    let exp = match exp {
        Some(e) => e,
        None => return Err(ApiError::Domain(DomainError::NotFound)),
    };

    if exp.status != "PENDIENTE" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "EXPENSE_INVALID_STATE",
            "El gasto no admite esa acción en su estado actual.",
            None,
        ));
    }

    sqlx::query(
        r#"
        update expenses
           set status = 'APROBADO',
               decided_by = $1,
               decided_at = now()
         where tenant_id = $2 and id = $3
        "#,
    )
    .bind(scope.user)
    .bind(scope.tenant)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    let exp_total = exp.total.unwrap_or(exp.amount + exp.tax_amount);
    let payable_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into payables (
            id, tenant_id, branch_id, supplier_id, source_type, source_id,
            amount_total, amount_paid, status, due_date
        ) values (
            $1, $2, $3, $4, 'GASTO', $5,
            $6, 0, 'ABIERTA', $7
        )
        on conflict (source_type, source_id) do update set
            amount_total = excluded.amount_total,
            due_date = excluded.due_date
        "#,
    )
    .bind(payable_id)
    .bind(scope.tenant)
    .bind(exp.branch_id)
    .bind(exp.supplier_id)
    .bind(exp.id)
    .bind(exp_total)
    .bind(exp.expense_date)
    .execute(&mut **tx)
    .await?;

    audit::record_tx(
        tx,
        scope,
        "GASTO_APROBADO",
        "expenses",
        id,
        None,
        Some(serde_json::json!({
            "payable_id": payable_id,
            "amount_total": exp_total,
        })),
        None,
    )
    .await?;

    get_expense_detail_tx(tx, scope, id).await
}

pub async fn reject_expense_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    req: RejectExpenseRequest,
) -> Result<ExpenseRow, ApiError> {
    scope.require_permission("expenses:approve")?;

    let reason = req.reason.trim();
    if reason.len() < 3 || reason.len() > 200 {
        return Err(error::validation(
            "El motivo debe tener entre 3 y 200 caracteres",
        ));
    }

    let status: Option<String> = sqlx::query_scalar(
        "select status from expenses where tenant_id = $1 and id = $2 for update",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    let status = match status {
        Some(s) => s,
        None => return Err(ApiError::Domain(DomainError::NotFound)),
    };

    if status != "PENDIENTE" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "EXPENSE_INVALID_STATE",
            "El gasto no admite esa acción en su estado actual.",
            None,
        ));
    }

    sqlx::query(
        r#"
        update expenses
           set status = 'RECHAZADO',
               decided_by = $1,
               decided_at = now(),
               reject_reason = $2
         where tenant_id = $3 and id = $4
        "#,
    )
    .bind(scope.user)
    .bind(reason)
    .bind(scope.tenant)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    audit::record_tx(
        tx,
        scope,
        "GASTO_RECHAZADO",
        "expenses",
        id,
        None,
        Some(serde_json::json!({ "reason": reason })),
        None,
    )
    .await?;

    get_expense_detail_tx(tx, scope, id).await
}

pub async fn cancel_expense_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<ExpenseRow, ApiError> {
    let exp: Option<(String, Uuid)> = sqlx::query_as(
        "select status, created_by from expenses where tenant_id = $1 and id = $2 for update",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    let (status, created_by) = match exp {
        Some(e) => e,
        None => return Err(ApiError::Domain(DomainError::NotFound)),
    };

    let can_approve = scope.has_permission("expenses:approve");
    let is_creator_with_perm = scope.has_permission("expenses:create") && created_by == scope.user;

    if !can_approve && !is_creator_with_perm {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    if status != "PENDIENTE" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "EXPENSE_INVALID_STATE",
            "El gasto no admite esa acción en su estado actual.",
            None,
        ));
    }

    sqlx::query(
        r#"
        update expenses
           set status = 'CANCELADO',
               decided_by = $1,
               decided_at = now()
         where tenant_id = $2 and id = $3
        "#,
    )
    .bind(scope.user)
    .bind(scope.tenant)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    audit::record_tx(
        tx,
        scope,
        "GASTO_CANCELADO",
        "expenses",
        id,
        None,
        None,
        None,
    )
    .await?;

    get_expense_detail_tx(tx, scope, id).await
}

pub async fn get_expense_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<ExpenseRow, ApiError> {
    if !scope.has_permission("expenses:create") && !scope.has_permission("expenses:approve") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let row = sqlx::query_as::<_, ExpenseRow>(
        r#"
        select
          e.id, e.tenant_id, e.branch_id, e.prefix, e.number,
          (e.prefix || '-' || e.number) as document_number,
          e.category_id, c.name as category_name, c.color as category_color,
          e.supplier_id, s.name as supplier_name,
          e.description, e.amount, e.tax_amount, coalesce(e.total, e.amount + e.tax_amount) as total,
          e.expense_date, e.status, e.created_by, u.full_name as created_by_name,
          e.decided_by, du.full_name as decided_by_name, e.decided_at,
          e.reject_reason, e.created_at,
          p.id as payable_id, p.amount_paid as payable_amount_paid, p.status as payable_status
        from expenses e
        inner join expense_categories c on c.id = e.category_id
        left join suppliers s on s.id = e.supplier_id
        left join users u on u.id = e.created_by
        left join users du on du.id = e.decided_by
        left join payables p on p.source_type = 'GASTO' and p.source_id = e.id
        where e.tenant_id = $1 and e.id = $2
        "#,
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        Some(r) => Ok(r),
        None => Err(ApiError::Domain(DomainError::NotFound)),
    }
}

pub async fn list_expenses_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    filter: ExpensesFilter,
) -> Result<Vec<ExpenseRow>, ApiError> {
    if !scope.has_permission("expenses:create") && !scope.has_permission("expenses:approve") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let rows = sqlx::query_as::<_, ExpenseRow>(
        r#"
        select
          e.id, e.tenant_id, e.branch_id, e.prefix, e.number,
          (e.prefix || '-' || e.number) as document_number,
          e.category_id, c.name as category_name, c.color as category_color,
          e.supplier_id, s.name as supplier_name,
          e.description, e.amount, e.tax_amount, coalesce(e.total, e.amount + e.tax_amount) as total,
          e.expense_date, e.status, e.created_by, u.full_name as created_by_name,
          e.decided_by, du.full_name as decided_by_name, e.decided_at,
          e.reject_reason, e.created_at,
          p.id as payable_id, p.amount_paid as payable_amount_paid, p.status as payable_status
        from expenses e
        inner join expense_categories c on c.id = e.category_id
        left join suppliers s on s.id = e.supplier_id
        left join users u on u.id = e.created_by
        left join users du on du.id = e.decided_by
        left join payables p on p.source_type = 'GASTO' and p.source_id = e.id
        where e.tenant_id = $1
          and ($2::uuid is null or e.branch_id = $2)
          and ($3::uuid is null or e.category_id = $3)
          and ($4::uuid is null or e.supplier_id = $4)
          and ($5::text is null or e.status = $5)
          and ($6::date is null or e.expense_date >= $6)
          and ($7::date is null or e.expense_date <= $7)
        order by e.expense_date desc, e.created_at desc
        "#,
    )
    .bind(scope.tenant)
    .bind(filter.branch_id)
    .bind(filter.category_id)
    .bind(filter.supplier_id)
    .bind(filter.status)
    .bind(filter.from)
    .bind(filter.to)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows)
}

pub async fn expenses_summary_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    filter: ExpenseSummaryFilter,
) -> Result<ExpenseSummary, ApiError> {
    if !scope.has_permission("expenses:create") && !scope.has_permission("expenses:approve") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let summary = sqlx::query_as::<_, ExpenseSummary>(
        r#"
        select
          coalesce(sum(case when status in ('APROBADO', 'PAGADO') then coalesce(total, amount + tax_amount) else 0 end), 0) as total_approved_month,
          coalesce(sum(case when status = 'PENDIENTE' then coalesce(total, amount + tax_amount) else 0 end), 0) as total_pending_month,
          coalesce(sum(case when status = 'PAGADO' then coalesce(total, amount + tax_amount) else 0 end), 0) as total_paid_month,
          coalesce(count(case when status = 'PENDIENTE' then 1 end), 0)::bigint as count_pending,
          coalesce(count(case when status in ('APROBADO', 'PAGADO') then 1 end), 0)::bigint as count_approved
        from expenses
        where tenant_id = $1
          and ($2::uuid is null or branch_id = $2)
          and (
            $3::text is null
            or to_char(expense_date, 'YYYY-MM') = $3
          )
        "#,
    )
    .bind(scope.tenant)
    .bind(filter.branch_id)
    .bind(filter.month)
    .fetch_one(&mut **tx)
    .await?;

    Ok(summary)
}

// ---------------------------------------------------------------------------
// FILTERS
// ---------------------------------------------------------------------------

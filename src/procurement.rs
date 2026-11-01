//! Compras y proveedores (modulo procurement).
//!
//! - Proveedores: a nivel de negocio (`tenant_id`).
//! - Compras: estado inicial `PENDIENTE` (idempotente por `id`).
//!
//! Al recibir (`/v1/purchases/{id}/receive`):
//! - Suma stock por sucursal en `inventory::replenish_ref_tx`.
//! - Registra movimiento `REPOSICION` con referencia `("COMPRA", purchase_id)` y causa "Compra <document_number>".
//! - Actualiza `cost_price` en `products` al costo unitario recibido.
//! - Pasa a `RECIBIDA` y fija `received_at = now()`.
//!
//! Al anular (`/v1/purchases/{id}/cancel`):
//! - Solo desde `PENDIENTE` (desde `RECIBIDA` da 409).
//! - Pasa a `ANULADA`.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state, validation};
use crate::tenancy::{self, Action, Scope};
use crate::{catalog, inventory};

type Tx<'a> = Transaction<'a, Postgres>;

fn clean_opt(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

// ============================================================
// Proveedores
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Supplier {
    pub id: Uuid,
    pub name: String,
    pub tax_id: Option<String>,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSupplierRequest {
    pub name: String,
    #[serde(default)]
    pub tax_id: Option<String>,
    #[serde(default)]
    pub contact_name: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSupplierRequest {
    pub name: String,
    #[serde(default)]
    pub tax_id: Option<String>,
    #[serde(default)]
    pub contact_name: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

pub async fn list_suppliers(
    pool: &PgPool,
    claims: &Claims,
    q: Option<String>,
    include_inactive: bool,
) -> Result<Vec<Supplier>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let q = clean_opt(q);
    let pattern = q.map(|s| format!("%{s}%"));
    let rows = sqlx::query_as::<_, Supplier>(
        "select id, name, tax_id, contact_name, phone, email, address, notes, active, created_at
           from suppliers
          where tenant_id = $1
            and ($2 or active)
            and ($3::text is null or name ilike $3 or tax_id ilike $3 or contact_name ilike $3)
          order by name",
    )
    .bind(scope.tenant)
    .bind(include_inactive)
    .bind(pattern)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn create_supplier(
    pool: &PgPool,
    claims: &Claims,
    req: CreateSupplierRequest,
) -> Result<Supplier, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let supplier = create_supplier_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(supplier)
}

pub async fn create_supplier_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateSupplierRequest,
) -> Result<Supplier, ApiError> {
    scope.require(Action::ManageInventory)?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(validation("name must not be empty"));
    }
    let tax_id = clean_opt(req.tax_id);

    if let Some(ref nit) = tax_id {
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from suppliers where tenant_id = $1 and tax_id = $2)",
        )
        .bind(scope.tenant)
        .bind(nit)
        .fetch_one(&mut **tx)
        .await?;
        if exists {
            return Err(ApiError::Domain(DomainError::Conflict));
        }
    }

    let supplier = sqlx::query_as::<_, Supplier>(
        "insert into suppliers (tenant_id, name, tax_id, contact_name, phone, email, address, notes, active)
         values ($1, $2, $3, $4, $5, $6, $7, $8, true)
         returning id, name, tax_id, contact_name, phone, email, address, notes, active, created_at",
    )
    .bind(scope.tenant)
    .bind(name)
    .bind(tax_id)
    .bind(clean_opt(req.contact_name))
    .bind(clean_opt(req.phone))
    .bind(clean_opt(req.email))
    .bind(clean_opt(req.address))
    .bind(clean_opt(req.notes))
    .fetch_one(&mut **tx)
    .await?;

    Ok(supplier)
}

pub async fn update_supplier(
    pool: &PgPool,
    claims: &Claims,
    supplier_id: Uuid,
    req: UpdateSupplierRequest,
) -> Result<Supplier, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let supplier = update_supplier_tx(&mut tx, &scope, supplier_id, req).await?;
    tx.commit().await?;
    Ok(supplier)
}

pub async fn update_supplier_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    supplier_id: Uuid,
    req: UpdateSupplierRequest,
) -> Result<Supplier, ApiError> {
    scope.require(Action::ManageInventory)?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(validation("name must not be empty"));
    }
    let tax_id = clean_opt(req.tax_id);

    if let Some(ref nit) = tax_id {
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from suppliers where tenant_id = $1 and tax_id = $2 and id != $3)",
        )
        .bind(scope.tenant)
        .bind(nit)
        .bind(supplier_id)
        .fetch_one(&mut **tx)
        .await?;
        if exists {
            return Err(ApiError::Domain(DomainError::Conflict));
        }
    }

    let supplier = sqlx::query_as::<_, Supplier>(
        "update suppliers
            set name = $2,
                tax_id = $3,
                contact_name = $4,
                phone = $5,
                email = $6,
                address = $7,
                notes = $8,
                active = coalesce($9, active)
          where id = $1 and tenant_id = $10
          returning id, name, tax_id, contact_name, phone, email, address, notes, active, created_at",
    )
    .bind(supplier_id)
    .bind(name)
    .bind(tax_id)
    .bind(clean_opt(req.contact_name))
    .bind(clean_opt(req.phone))
    .bind(clean_opt(req.email))
    .bind(clean_opt(req.address))
    .bind(clean_opt(req.notes))
    .bind(req.active)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    Ok(supplier)
}

// ============================================================
// Compras
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseItemRequest {
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_cost: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePurchaseRequest {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    #[serde(default)]
    pub document_number: Option<String>,
    #[serde(default)]
    pub supplier_invoice_date: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<PurchaseItemRequest>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PurchaseRow {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    pub supplier_name: String,
    pub document_number: Option<String>,
    pub supplier_invoice_date: Option<NaiveDate>,
    pub status: String,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    pub items_count: i64,
    pub received_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PurchaseItemDetail {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    pub line_total: Decimal,
    pub tax_code: Option<String>,
    pub tax_rate: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseDetail {
    #[serde(flatten)]
    pub purchase: PurchaseRow,
    pub items: Vec<PurchaseItemDetail>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseSummary {
    pub total_purchased_month: Decimal,
    pub active_suppliers: i64,
}

#[derive(Debug, Deserialize, Default)]
pub struct PurchaseFilter {
    pub branch_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub supplier_id: Option<Uuid>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

const PURCHASE_SELECT: &str = r#"
select pr.id, pr.branch_id, pr.supplier_id, s.name as supplier_name,
       pr.document_number, pr.supplier_invoice_date,
       tm.tima_codigo as status,
       pr.subtotal, pr.tax_amount, pr.total_amount,
       count(pri.id) as items_count,
       pr.received_at, pr.notes, pr.created_at
  from purchase_receipts pr
  join suppliers s on s.id = pr.supplier_id
  join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
  left join purchase_receipt_items pri on pri.purchase_receipt_id = pr.id
"#;

pub async fn get_purchase_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let sql = format!(
        "{PURCHASE_SELECT} where pr.id = $1 and pr.tenant_id = $2 group by pr.id, s.name, tm.tima_codigo"
    );
    let purchase = sqlx::query_as::<_, PurchaseRow>(&sql)
        .bind(id)
        .bind(scope.tenant)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let items = sqlx::query_as::<_, PurchaseItemDetail>(
        r#"
        select pri.id, pri.product_id, p.name as product_name, p.sku,
               pri.qty, pri.unit_cost, pri.line_total,
               tx.tima_codigo as tax_code,
               null::numeric as tax_rate,
               pri.notes
          from purchase_receipt_items pri
          join products p on p.id = pri.product_id
          left join tbl_tipos_maestra tx on tx.tima_id = pri.tax_type_id
         where pri.purchase_receipt_id = $1
         order by p.name
        "#,
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;

    let items = items
        .into_iter()
        .map(|mut it| {
            it.tax_rate = catalog::tax_rate_for(it.tax_code.as_deref());
            it
        })
        .collect();

    Ok(PurchaseDetail { purchase, items })
}

pub async fn get_purchase(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let detail = get_purchase_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_purchase(
    pool: &PgPool,
    claims: &Claims,
    req: CreatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require(Action::ManageInventory)?;
    let detail = create_purchase_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    scope.require(Action::ManageInventory)?;
    tenancy::ensure_branch_tx(tx, scope, req.branch_id).await?;

    if req.items.is_empty() {
        return Err(validation("purchase has no items"));
    }
    for item in &req.items {
        if item.qty <= Decimal::ZERO {
            return Err(validation("qty must be > 0"));
        }
        if item.unit_cost < Decimal::ZERO {
            return Err(validation("unit_cost must be >= 0"));
        }
    }

    let supplier_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from suppliers where id = $1 and tenant_id = $2)",
    )
    .bind(req.supplier_id)
    .bind(scope.tenant)
    .fetch_one(&mut **tx)
    .await?;
    if !supplier_exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    let product_ids: Vec<Uuid> = req.items.iter().map(|i| i.product_id).collect();
    #[derive(sqlx::FromRow)]
    struct ProdInfo {
        id: Uuid,
        is_composed: bool,
        tax_code: Option<String>,
        tax_type_id: Option<i32>,
    }
    let prods = sqlx::query_as::<_, ProdInfo>(
        "select p.id, p.is_composed, tx.tima_codigo as tax_code, p.tax_type_id
           from products p
           left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
          where p.id = any($1) and p.tenant_id = $2",
    )
    .bind(&product_ids)
    .bind(scope.tenant)
    .fetch_all(&mut **tx)
    .await?;

    if prods.len() != product_ids.len() {
        return Err(validation(
            "one or more products do not exist in this tenant",
        ));
    }
    for p in &prods {
        if p.is_composed {
            return Err(validation(format!(
                "composed product {} cannot be purchased directly",
                p.id
            )));
        }
    }

    let body = serde_json::to_value(&req).map_err(anyhow::Error::from)?;
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "insert into purchase_receipts (
           id, tenant_id, branch_id, supplier_id, status_id,
           document_number, supplier_invoice_date, notes, client_request,
           subtotal, tax_amount, total_amount, created_by
         ) values (
           $1, $2, $3, $4, public.tipo_id('PUR', 'PENDIENTE'),
           $5, $6, $7, $8,
           0, 0, 0, $9
         )
         on conflict (id) do nothing
         returning id",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(req.supplier_id)
    .bind(clean_opt(req.document_number.clone()))
    .bind(req.supplier_invoice_date)
    .bind(clean_opt(req.notes.clone()))
    .bind(&body)
    .bind(scope.user)
    .fetch_optional(&mut **tx)
    .await?;

    if claimed.is_none() {
        let stored: Option<Option<serde_json::Value>> =
            sqlx::query_scalar("select client_request from purchase_receipts where id = $1")
                .bind(req.id)
                .fetch_optional(&mut **tx)
                .await?;
        return match stored {
            Some(Some(previous)) if previous == body => {
                get_purchase_detail_tx(tx, scope, req.id).await
            }
            _ => Err(state("purchase id already used with a different request")),
        };
    }

    let mut subtotal = Decimal::ZERO;
    let mut tax_amount = Decimal::ZERO;

    for item in &req.items {
        let p_info = prods.iter().find(|p| p.id == item.product_id).unwrap();
        let line_subtotal = item.qty * item.unit_cost;
        let tax_rate = catalog::tax_rate_for(p_info.tax_code.as_deref()).unwrap_or(Decimal::ZERO);
        let line_tax = line_subtotal * tax_rate;
        subtotal += line_subtotal;
        tax_amount += line_tax;

        sqlx::query(
            "insert into purchase_receipt_items
               (tenant_id, branch_id, purchase_receipt_id, product_id, qty, unit_cost, tax_type_id)
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(scope.tenant)
        .bind(req.branch_id)
        .bind(req.id)
        .bind(item.product_id)
        .bind(item.qty)
        .bind(item.unit_cost)
        .bind(p_info.tax_type_id)
        .execute(&mut **tx)
        .await?;
    }

    let total_amount = subtotal + tax_amount;
    sqlx::query(
        "update purchase_receipts
            set subtotal = $2,
                tax_amount = $3,
                total_amount = $4
          where id = $1",
    )
    .bind(req.id)
    .bind(subtotal)
    .bind(tax_amount)
    .bind(total_amount)
    .execute(&mut **tx)
    .await?;

    get_purchase_detail_tx(tx, scope, req.id).await
}

pub async fn receive_purchase(
    pool: &PgPool,
    claims: &Claims,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let result = receive_purchase_tx(&mut tx, &scope, purchase_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn receive_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    scope.require(Action::ManageInventory)?;

    let row: Option<(Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        "select pr.id, pr.branch_id, tm.tima_codigo as status, pr.document_number
           from purchase_receipts pr
           join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
          where pr.id = $1 and pr.tenant_id = $2
            for update of pr",
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let (_id, branch_id, status, doc_num) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    if status == "RECIBIDA" {
        return Err(state("purchase is already received"));
    }
    if status == "ANULADA" {
        return Err(state("cannot receive a cancelled purchase"));
    }
    if status != "PENDIENTE" {
        return Err(state(format!("invalid status {status} for receive")));
    }

    #[derive(sqlx::FromRow)]
    struct ItemToReceive {
        product_id: Uuid,
        qty: Decimal,
        unit_cost: Decimal,
    }
    let items = sqlx::query_as::<_, ItemToReceive>(
        "select product_id, qty, unit_cost
           from purchase_receipt_items
          where purchase_receipt_id = $1
          order by product_id
            for update",
    )
    .bind(purchase_id)
    .fetch_all(&mut **tx)
    .await?;

    let cause = match doc_num.as_deref() {
        Some(doc) if !doc.trim().is_empty() => format!("Compra {}", doc.trim()),
        _ => "Compra".to_string(),
    };

    for item in items {
        inventory::replenish_ref_tx(
            tx,
            scope,
            branch_id,
            item.product_id,
            item.qty,
            &cause,
            Some(("COMPRA", purchase_id)),
            Some(item.unit_cost),
        )
        .await?;

        sqlx::query(
            "update products
                set cost_price = $1
              where id = $2 and tenant_id = $3",
        )
        .bind(item.unit_cost)
        .bind(item.product_id)
        .bind(scope.tenant)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query(
        "update purchase_receipts
            set status_id = public.tipo_id('PUR', 'RECIBIDA'),
                received_at = now(),
                updated_at = now()
          where id = $1",
    )
    .bind(purchase_id)
    .execute(&mut **tx)
    .await?;

    get_purchase_detail_tx(tx, scope, purchase_id).await
}

pub async fn cancel_purchase(
    pool: &PgPool,
    claims: &Claims,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let result = cancel_purchase_tx(&mut tx, &scope, purchase_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn cancel_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    scope.require(Action::ManageInventory)?;

    let row: Option<(Uuid, String)> = sqlx::query_as(
        "select pr.id, tm.tima_codigo as status
           from purchase_receipts pr
           join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
          where pr.id = $1 and pr.tenant_id = $2
            for update of pr",
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let (_id, status) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    if status == "RECIBIDA" {
        return Err(state("cannot cancel an already received purchase"));
    }
    if status == "ANULADA" {
        return Err(state("purchase is already cancelled"));
    }
    if status != "PENDIENTE" {
        return Err(state(format!("invalid status {status} for cancel")));
    }

    sqlx::query(
        "update purchase_receipts
            set status_id = public.tipo_id('PUR', 'ANULADA'),
                updated_at = now()
          where id = $1",
    )
    .bind(purchase_id)
    .execute(&mut **tx)
    .await?;

    get_purchase_detail_tx(tx, scope, purchase_id).await
}

pub async fn list_purchases(
    pool: &PgPool,
    claims: &Claims,
    filter: PurchaseFilter,
) -> Result<Vec<PurchaseRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;

    let limit = filter.limit.unwrap_or(50).clamp(1, 200);
    let status_code = clean_opt(filter.status);

    let sql = format!(
        "{PURCHASE_SELECT}
         where pr.tenant_id = $1
           and ($2::uuid is null or pr.branch_id = $2)
           and ($3::timestamptz is null or pr.created_at >= $3)
           and ($4::timestamptz is null or pr.created_at <= $4)
           and ($5::uuid is null or pr.supplier_id = $5)
           and ($6::text is null or tm.tima_codigo = $6)
         group by pr.id, s.name, tm.tima_codigo
         order by pr.created_at desc
         limit $7"
    );

    let rows = sqlx::query_as::<_, PurchaseRow>(&sql)
        .bind(scope.tenant)
        .bind(filter.branch_id)
        .bind(filter.from)
        .bind(filter.to)
        .bind(filter.supplier_id)
        .bind(status_code)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(rows)
}

pub async fn purchase_summary(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Option<Uuid>,
    month: Option<String>,
) -> Result<PurchaseSummary, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;

    let month_str = clean_opt(month).unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());

    let total: Option<Decimal> = sqlx::query_scalar(
        "select coalesce(sum(pr.total_amount), 0)
           from purchase_receipts pr
           join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
          where pr.tenant_id = $1
            and ($2::uuid is null or pr.branch_id = $2)
            and tm.tima_codigo = 'RECIBIDA'
            and to_char(coalesce(pr.received_at, pr.created_at), 'YYYY-MM') = $3",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(month_str)
    .fetch_one(&mut *tx)
    .await?;

    let count: i64 =
        sqlx::query_scalar("select count(*) from suppliers where tenant_id = $1 and active = true")
            .bind(scope.tenant)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(PurchaseSummary {
        total_purchased_month: total.unwrap_or(Decimal::ZERO),
        active_suppliers: count,
    })
}

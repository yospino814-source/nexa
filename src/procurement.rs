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

/// Dígito de verificación del NIT (algoritmo DIAN).
pub fn nit_dv(nit: &str) -> u8 {
    let digits: Vec<u64> = nit
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| d as u64))
        .collect();

    let weights: [u64; 15] = [3, 7, 13, 17, 19, 23, 29, 37, 41, 43, 47, 53, 59, 67, 71];

    let mut sum = 0u64;
    for (i, &d) in digits.iter().rev().enumerate() {
        if i < weights.len() {
            sum += d * weights[i];
        }
    }

    let r = (sum % 11) as u8;
    if r == 0 || r == 1 { r } else { 11 - r }
}

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
    pub doc_type: String,
    pub dv: Option<String>,
    pub person_type: String,
    pub iva_responsible: bool,
    pub self_withholding: bool,
    pub payment_terms: String,
    pub credit_days: i32,
    pub delivery_days: Option<i32>,
    pub category: String,
    pub bank_name: Option<String>,
    pub bank_account_type: Option<String>,
    pub bank_account_number: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateSupplierRequest {
    pub name: String,
    #[serde(default)]
    pub tax_id: Option<String>,
    #[serde(default)]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub dv: Option<String>,
    #[serde(default)]
    pub person_type: Option<String>,
    #[serde(default)]
    pub iva_responsible: Option<bool>,
    #[serde(default)]
    pub self_withholding: Option<bool>,
    #[serde(default)]
    pub payment_terms: Option<String>,
    #[serde(default)]
    pub credit_days: Option<i32>,
    #[serde(default)]
    pub delivery_days: Option<i32>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub bank_account_type: Option<String>,
    #[serde(default)]
    pub bank_account_number: Option<String>,
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateSupplierRequest {
    pub name: String,
    #[serde(default)]
    pub tax_id: Option<String>,
    #[serde(default)]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub dv: Option<String>,
    #[serde(default)]
    pub person_type: Option<String>,
    #[serde(default)]
    pub iva_responsible: Option<bool>,
    #[serde(default)]
    pub self_withholding: Option<bool>,
    #[serde(default)]
    pub payment_terms: Option<String>,
    #[serde(default)]
    pub credit_days: Option<i32>,
    #[serde(default)]
    pub delivery_days: Option<i32>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub bank_account_type: Option<String>,
    #[serde(default)]
    pub bank_account_number: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SupplierProductItemInput {
    pub product_id: Uuid,
    #[serde(default)]
    pub supplier_code: Option<String>,
    #[serde(default)]
    pub min_order_qty: Option<Decimal>,
    #[serde(default)]
    pub preferred: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SupplierProductDetail {
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub supplier_code: Option<String>,
    pub last_cost: Option<Decimal>,
    pub min_order_qty: Option<Decimal>,
    pub preferred: bool,
}

#[derive(Debug, Serialize)]
pub struct SupplierDetailResponse {
    pub supplier: Supplier,
    pub products: Vec<SupplierProductDetail>,
    pub open_orders: i64,
    pub balance: Decimal,
}

pub async fn list_suppliers(
    pool: &PgPool,
    claims: &Claims,
    q: Option<String>,
    include_inactive: bool,
) -> Result<Vec<Supplier>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:read")?;
    let q = clean_opt(q);
    let pattern = q.map(|s| format!("%{s}%"));
    let rows = sqlx::query_as::<_, Supplier>(
        "select id, name, tax_id, contact_name, phone, email, address, notes, active, created_at,
                doc_type, dv, person_type, iva_responsible, self_withholding, payment_terms,
                credit_days, delivery_days, category, bank_name, bank_account_type, bank_account_number,
                archived_at
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

pub async fn get_supplier(
    pool: &PgPool,
    claims: &Claims,
    supplier_id: Uuid,
) -> Result<SupplierDetailResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:read")?;
    let res = get_supplier_tx(&mut tx, &scope, supplier_id).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn get_supplier_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    supplier_id: Uuid,
) -> Result<SupplierDetailResponse, ApiError> {
    scope.require_permission("inventory:read")?;

    let supplier = sqlx::query_as::<_, Supplier>(
        "select id, name, tax_id, contact_name, phone, email, address, notes, active, created_at,
                doc_type, dv, person_type, iva_responsible, self_withholding, payment_terms,
                credit_days, delivery_days, category, bank_name, bank_account_type, bank_account_number,
                archived_at
           from suppliers
          where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let products = sqlx::query_as::<_, SupplierProductDetail>(
        "select sp.product_id, p.name as product_name, p.sku, sp.supplier_code, sp.last_cost, sp.min_order_qty, sp.preferred
           from supplier_products sp
           join products p on p.id = sp.product_id
          where sp.tenant_id = $1 and sp.supplier_id = $2
          order by p.name",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_all(&mut **tx)
    .await?;

    let open_orders: i64 = sqlx::query_scalar(
        "select count(*)::bigint
           from purchase_receipts pr
           join tbl_tipos_maestra t on t.tima_id = pr.status_id
          where pr.tenant_id = $1 and pr.supplier_id = $2
            and t.tima_codigo in ('PENDIENTE', 'APROBADA', 'PARCIAL')",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_one(&mut **tx)
    .await?;

    let balance: Decimal = sqlx::query_scalar(
        "select coalesce(sum(amount_total - amount_paid), 0)
           from payables
          where tenant_id = $1 and supplier_id = $2
            and status not in ('PAGADA', 'CASTIGADA')",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(SupplierDetailResponse {
        supplier,
        products,
        open_orders,
        balance,
    })
}

pub async fn create_supplier(
    pool: &PgPool,
    claims: &Claims,
    req: CreateSupplierRequest,
) -> Result<Supplier, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:create")?;
    let supplier = create_supplier_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(supplier)
}

pub async fn create_supplier_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateSupplierRequest,
) -> Result<Supplier, ApiError> {
    scope.require_permission("purchases:create")?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(validation("name must not be empty"));
    }
    let tax_id = clean_opt(req.tax_id);

    let doc_type = req
        .doc_type
        .map(|d| d.trim().to_uppercase())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "NIT".to_string());
    if !matches!(doc_type.as_str(), "NIT" | "CC" | "CE") {
        return Err(validation("doc_type must be NIT, CC, or CE"));
    }

    let dv = match clean_opt(req.dv) {
        Some(dv_val) => {
            if let (true, Some(nit)) = (doc_type == "NIT", tax_id.as_deref()) {
                let expected = nit_dv(nit).to_string();
                if dv_val != expected {
                    return Err(crate::error::coded(
                        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                        "NIT_DV_INVALID",
                        "El dígito de verificación no corresponde al NIT.",
                        None,
                    ));
                }
            }
            Some(dv_val)
        }
        None => {
            if doc_type == "NIT" {
                tax_id.as_deref().map(|nit| nit_dv(nit).to_string())
            } else {
                None
            }
        }
    };

    let person_type = req
        .person_type
        .map(|p| p.trim().to_uppercase())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "JURIDICA".to_string());
    if !matches!(person_type.as_str(), "NATURAL" | "JURIDICA") {
        return Err(validation("person_type must be NATURAL or JURIDICA"));
    }

    let iva_responsible = req.iva_responsible.unwrap_or(true);
    let self_withholding = req.self_withholding.unwrap_or(false);

    let payment_terms = req
        .payment_terms
        .map(|pt| pt.trim().to_uppercase())
        .filter(|pt| !pt.is_empty())
        .unwrap_or_else(|| "CONTADO".to_string());
    if !matches!(payment_terms.as_str(), "CONTADO" | "CREDITO") {
        return Err(validation("payment_terms must be CONTADO or CREDITO"));
    }

    let credit_days = req.credit_days.unwrap_or(0);
    if !(0..=180).contains(&credit_days) {
        return Err(validation("credit_days must be between 0 and 180"));
    }
    if payment_terms != "CREDITO" && credit_days != 0 {
        return Err(validation(
            "credit_days must be 0 when payment_terms is CONTADO",
        ));
    }

    if matches!(req.delivery_days, Some(del_days) if !(0..=60).contains(&del_days)) {
        return Err(validation("delivery_days must be between 0 and 60"));
    }

    let category = req
        .category
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "MERCANCIA".to_string());
    if !matches!(category.as_str(), "MERCANCIA" | "TRANSPORTE" | "SERVICIOS") {
        return Err(validation(
            "category must be MERCANCIA, TRANSPORTE, or SERVICIOS",
        ));
    }

    if let Some(ref bat) = req.bank_account_type {
        let bat_upper = bat.trim().to_uppercase();
        if !matches!(bat_upper.as_str(), "AHORROS" | "CORRIENTE") {
            return Err(validation("bank_account_type must be AHORROS or CORRIENTE"));
        }
    }

    if let Some(ref ban) = req.bank_account_number {
        let ban_trim = ban.trim();
        if ban_trim.len() < 6
            || ban_trim.len() > 20
            || !ban_trim.chars().all(|c| c.is_ascii_digit())
        {
            return Err(validation("bank_account_number must be 6 to 20 digits"));
        }
    }

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
        "insert into suppliers (
           tenant_id, name, tax_id, contact_name, phone, email, address, notes, active,
           doc_type, dv, person_type, iva_responsible, self_withholding, payment_terms,
           credit_days, delivery_days, category, bank_name, bank_account_type, bank_account_number
         ) values (
           $1, $2, $3, $4, $5, $6, $7, $8, true,
           $9, $10, $11, $12, $13, $14,
           $15, $16, $17, $18, $19, $20
         )
         returning id, name, tax_id, contact_name, phone, email, address, notes, active, created_at,
                   doc_type, dv, person_type, iva_responsible, self_withholding, payment_terms,
                   credit_days, delivery_days, category, bank_name, bank_account_type, bank_account_number,
                   archived_at",
    )
    .bind(scope.tenant)
    .bind(name)
    .bind(tax_id)
    .bind(clean_opt(req.contact_name))
    .bind(clean_opt(req.phone))
    .bind(clean_opt(req.email))
    .bind(clean_opt(req.address))
    .bind(clean_opt(req.notes))
    .bind(doc_type)
    .bind(dv)
    .bind(person_type)
    .bind(iva_responsible)
    .bind(self_withholding)
    .bind(payment_terms)
    .bind(credit_days)
    .bind(req.delivery_days)
    .bind(category)
    .bind(clean_opt(req.bank_name))
    .bind(req.bank_account_type.map(|s| s.trim().to_uppercase()))
    .bind(clean_opt(req.bank_account_number))
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
    scope.require_permission("purchases:create")?;
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
    scope.require_permission("purchases:create")?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(validation("name must not be empty"));
    }
    let tax_id = clean_opt(req.tax_id);

    let doc_type = req
        .doc_type
        .map(|d| d.trim().to_uppercase())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "NIT".to_string());
    if !matches!(doc_type.as_str(), "NIT" | "CC" | "CE") {
        return Err(validation("doc_type must be NIT, CC, or CE"));
    }

    let dv = match clean_opt(req.dv) {
        Some(dv_val) => {
            if let (true, Some(nit)) = (doc_type == "NIT", tax_id.as_deref()) {
                let expected = nit_dv(nit).to_string();
                if dv_val != expected {
                    return Err(crate::error::coded(
                        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                        "NIT_DV_INVALID",
                        "El dígito de verificación no corresponde al NIT.",
                        None,
                    ));
                }
            }
            Some(dv_val)
        }
        None => {
            if doc_type == "NIT" {
                tax_id.as_deref().map(|nit| nit_dv(nit).to_string())
            } else {
                None
            }
        }
    };

    let person_type = req
        .person_type
        .map(|p| p.trim().to_uppercase())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "JURIDICA".to_string());
    if !matches!(person_type.as_str(), "NATURAL" | "JURIDICA") {
        return Err(validation("person_type must be NATURAL or JURIDICA"));
    }

    let iva_responsible = req.iva_responsible.unwrap_or(true);
    let self_withholding = req.self_withholding.unwrap_or(false);

    let payment_terms = req
        .payment_terms
        .map(|pt| pt.trim().to_uppercase())
        .filter(|pt| !pt.is_empty())
        .unwrap_or_else(|| "CONTADO".to_string());
    if !matches!(payment_terms.as_str(), "CONTADO" | "CREDITO") {
        return Err(validation("payment_terms must be CONTADO or CREDITO"));
    }

    let credit_days = req.credit_days.unwrap_or(0);
    if !(0..=180).contains(&credit_days) {
        return Err(validation("credit_days must be between 0 and 180"));
    }
    if payment_terms != "CREDITO" && credit_days != 0 {
        return Err(validation(
            "credit_days must be 0 when payment_terms is CONTADO",
        ));
    }

    if matches!(req.delivery_days, Some(del_days) if !(0..=60).contains(&del_days)) {
        return Err(validation("delivery_days must be between 0 and 60"));
    }

    let category = req
        .category
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "MERCANCIA".to_string());
    if !matches!(category.as_str(), "MERCANCIA" | "TRANSPORTE" | "SERVICIOS") {
        return Err(validation(
            "category must be MERCANCIA, TRANSPORTE, or SERVICIOS",
        ));
    }

    if let Some(ref bat) = req.bank_account_type {
        let bat_upper = bat.trim().to_uppercase();
        if !matches!(bat_upper.as_str(), "AHORROS" | "CORRIENTE") {
            return Err(validation("bank_account_type must be AHORROS or CORRIENTE"));
        }
    }

    if let Some(ref ban) = req.bank_account_number {
        let ban_trim = ban.trim();
        if ban_trim.len() < 6
            || ban_trim.len() > 20
            || !ban_trim.chars().all(|c| c.is_ascii_digit())
        {
            return Err(validation("bank_account_number must be 6 to 20 digits"));
        }
    }

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
                active = coalesce($9, active),
                doc_type = $10,
                dv = $11,
                person_type = $12,
                iva_responsible = $13,
                self_withholding = $14,
                payment_terms = $15,
                credit_days = $16,
                delivery_days = $17,
                category = $18,
                bank_name = $19,
                bank_account_type = $20,
                bank_account_number = $21
          where id = $1 and tenant_id = $22
          returning id, name, tax_id, contact_name, phone, email, address, notes, active, created_at,
                    doc_type, dv, person_type, iva_responsible, self_withholding, payment_terms,
                    credit_days, delivery_days, category, bank_name, bank_account_type, bank_account_number,
                    archived_at",
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
    .bind(doc_type)
    .bind(dv)
    .bind(person_type)
    .bind(iva_responsible)
    .bind(self_withholding)
    .bind(payment_terms)
    .bind(credit_days)
    .bind(req.delivery_days)
    .bind(category)
    .bind(clean_opt(req.bank_name))
    .bind(req.bank_account_type.map(|s| s.trim().to_uppercase()))
    .bind(clean_opt(req.bank_account_number))
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    Ok(supplier)
}

pub async fn archive_supplier(
    pool: &PgPool,
    claims: &Claims,
    supplier_id: Uuid,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:create")?;
    archive_supplier_tx(&mut tx, &scope, supplier_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn archive_supplier_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    supplier_id: Uuid,
) -> Result<(), ApiError> {
    scope.require_permission("purchases:create")?;

    let has_open_orders: bool = sqlx::query_scalar(
        "select exists(
           select 1 from purchase_receipts pr
            join tbl_tipos_maestra t on t.tima_id = pr.status_id
           where pr.tenant_id = $1 and pr.supplier_id = $2
             and t.tima_codigo in ('PENDIENTE', 'APROBADA', 'PARCIAL')
        )",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_one(&mut **tx)
    .await?;

    let has_open_payables: bool = sqlx::query_scalar(
        "select exists(
           select 1 from payables
            where tenant_id = $1 and supplier_id = $2
              and status in ('ABIERTA', 'PARCIAL')
              and (amount_total - amount_paid) > 0
        )",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_one(&mut **tx)
    .await?;

    if has_open_orders || has_open_payables {
        return Err(crate::error::coded(
            axum::http::StatusCode::CONFLICT,
            "SUPPLIER_HAS_OPEN_DOCS",
            "El proveedor tiene órdenes o deudas abiertas.",
            None,
        ));
    }

    let affected = sqlx::query(
        "update suppliers set archived_at = now(), active = false where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .execute(&mut **tx)
    .await?;

    if affected.rows_affected() == 0 {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    Ok(())
}

pub async fn set_supplier_products(
    pool: &PgPool,
    claims: &Claims,
    supplier_id: Uuid,
    items: Vec<SupplierProductItemInput>,
) -> Result<Vec<SupplierProductDetail>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:create")?;
    let res = set_supplier_products_tx(&mut tx, &scope, supplier_id, items).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn set_supplier_products_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    supplier_id: Uuid,
    items: Vec<SupplierProductItemInput>,
) -> Result<Vec<SupplierProductDetail>, ApiError> {
    scope.require_permission("purchases:create")?;

    let supplier_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from suppliers where tenant_id = $1 and id = $2)",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_one(&mut **tx)
    .await?;

    if !supplier_exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    for item in &items {
        if matches!(item.min_order_qty, Some(moq) if moq <= Decimal::ZERO) {
            return Err(validation("min_order_qty must be > 0"));
        }
        if item.preferred {
            sqlx::query(
                "update supplier_products
                    set preferred = false
                  where tenant_id = $1 and product_id = $2 and supplier_id != $3",
            )
            .bind(scope.tenant)
            .bind(item.product_id)
            .bind(supplier_id)
            .execute(&mut **tx)
            .await?;
        }
    }

    sqlx::query("delete from supplier_products where tenant_id = $1 and supplier_id = $2")
        .bind(scope.tenant)
        .bind(supplier_id)
        .execute(&mut **tx)
        .await?;

    for item in items {
        sqlx::query(
            "insert into supplier_products
               (tenant_id, supplier_id, product_id, supplier_code, min_order_qty, preferred)
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(scope.tenant)
        .bind(supplier_id)
        .bind(item.product_id)
        .bind(clean_opt(item.supplier_code))
        .bind(item.min_order_qty)
        .bind(item.preferred)
        .execute(&mut **tx)
        .await?;
    }

    let list = sqlx::query_as::<_, SupplierProductDetail>(
        "select sp.product_id, p.name as product_name, p.sku, sp.supplier_code, sp.last_cost, sp.min_order_qty, sp.preferred
           from supplier_products sp
           join products p on p.id = sp.product_id
          where sp.tenant_id = $1 and sp.supplier_id = $2
          order by p.name",
    )
    .bind(scope.tenant)
    .bind(supplier_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(list)
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

    let loc_id = crate::locations::default_location_id_tx(tx, req.branch_id).await?;
    let body = serde_json::to_value(&req).map_err(anyhow::Error::from)?;
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "insert into purchase_receipts (
           id, tenant_id, branch_id, location_id, supplier_id, status_id,
           document_number, supplier_invoice_date, notes, client_request,
           subtotal, tax_amount, total_amount, created_by
         ) values (
           $1, $2, $3, $4, $5, public.tipo_id('PUR', 'PENDIENTE'),
           $6, $7, $8, $9,
           0, 0, 0, $10
         )
         on conflict (id) do nothing
         returning id",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(loc_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nit_dv_known_values() {
        assert_eq!(nit_dv("900123456"), 8);
        assert_eq!(nit_dv("800197268"), 4);
        assert_eq!(nit_dv("860001022"), 7);
    }
}

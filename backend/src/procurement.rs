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

use axum::http::StatusCode;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state, validation};
use crate::tenancy::{self, Scope};
use crate::{audit, catalog, inventory, money, payables, pricing, sequences};

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
        .unwrap_or_else(|| {
            if doc_type.as_str() == "NIT" {
                tax_id
                    .as_deref()
                    .map(crate::customers::default_person_type)
                    .unwrap_or("JURIDICA")
                    .to_string()
            } else {
                "NATURAL".to_string()
            }
        });
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
        .unwrap_or_else(|| {
            if doc_type.as_str() == "NIT" {
                tax_id
                    .as_deref()
                    .map(crate::customers::default_person_type)
                    .unwrap_or("JURIDICA")
                    .to_string()
            } else {
                "NATURAL".to_string()
            }
        });
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
// COMPRAS V2 (F2.12)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PurchaseItemRequest {
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    #[serde(default)]
    pub new_sale_price: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePurchaseRequest {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    #[serde(default)]
    pub location_id: Option<Uuid>,
    #[serde(default)]
    pub expected_at: Option<NaiveDate>,
    #[serde(default)]
    pub document_number: Option<String>,
    #[serde(default)]
    pub supplier_invoice_date: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub discount_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_mode: Option<String>,
    #[serde(default)]
    pub payment_terms: Option<String>,
    #[serde(default)]
    pub credit_days: Option<i32>,
    #[serde(default)]
    pub iva_to_cost: Option<bool>,
    pub items: Vec<PurchaseItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePurchaseRequest {
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    #[serde(default)]
    pub location_id: Option<Uuid>,
    #[serde(default)]
    pub expected_at: Option<NaiveDate>,
    #[serde(default)]
    pub document_number: Option<String>,
    #[serde(default)]
    pub supplier_invoice_date: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub discount_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_mode: Option<String>,
    #[serde(default)]
    pub payment_terms: Option<String>,
    #[serde(default)]
    pub credit_days: Option<i32>,
    #[serde(default)]
    pub iva_to_cost: Option<bool>,
    pub items: Vec<PurchaseItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveItemInput {
    pub purchase_item_id: Uuid,
    pub qty: Decimal,
    #[serde(default)]
    pub lot_code: Option<String>,
    #[serde(default)]
    pub expires_at: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivePurchaseRequest {
    pub id: Uuid,
    pub items: Vec<ReceiveItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostPreviewRequest {
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    #[serde(default)]
    pub discount_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_amount: Option<Decimal>,
    #[serde(default)]
    pub freight_mode: Option<String>,
    #[serde(default)]
    pub iva_to_cost: Option<bool>,
    pub items: Vec<PurchaseItemRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostLineResponse {
    pub product_id: Uuid,
    pub product_name: String,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    pub gross: Decimal,
    pub discount_alloc: Decimal,
    pub net: Decimal,
    pub freight_alloc: Decimal,
    pub tax_amount: Decimal,
    pub landed_unit_cost: Decimal,
    pub current_sale_price: Option<Decimal>,
    pub new_sale_price: Option<Decimal>,
    pub current_margin: Option<Decimal>,
    pub new_margin: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostPreviewResponse {
    pub subtotal: Decimal,
    pub discount_amount: Decimal,
    pub freight_amount: Decimal,
    pub freight_mode: String,
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    pub iva_to_cost: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub lines: Vec<CostLineResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PurchaseRow {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub supplier_id: Uuid,
    pub supplier_name: String,
    pub location_id: Uuid,
    pub expected_at: Option<NaiveDate>,
    pub document_number: Option<String>,
    pub supplier_invoice_date: Option<NaiveDate>,
    pub status: String,
    pub subtotal: Decimal,
    pub discount_amount: Decimal,
    pub freight_amount: Decimal,
    pub freight_mode: String,
    pub payment_terms: String,
    pub credit_days: i32,
    pub iva_to_cost: bool,
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    pub items_count: i64,
    pub received_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PurchaseItemDetail {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub purchase_unit: Option<String>,
    pub purchase_factor: Decimal,
    pub qty: Decimal,
    pub qty_received: Decimal,
    pub unit_cost: Decimal,
    pub line_total: Decimal,
    pub discount_alloc: Decimal,
    pub freight_alloc: Decimal,
    pub tax_code: Option<String>,
    pub tax_rate: Option<Decimal>,
    pub tax_amount: Decimal,
    pub landed_unit_cost: Option<Decimal>,
    pub new_sale_price: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseDetail {
    #[serde(flatten)]
    pub purchase: PurchaseRow,
    pub items: Vec<PurchaseItemDetail>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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

#[derive(Debug, Clone)]
pub struct CostLineInput {
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    pub tax_rate: Decimal,
    pub is_inc: bool,
    pub purchase_factor: Decimal,
    pub new_sale_price: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct CostLine {
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    pub gross: Decimal,
    pub discount_alloc: Decimal,
    pub net: Decimal,
    pub freight_alloc: Decimal,
    pub tax_amount: Decimal,
    pub landed_unit_cost: Decimal,
    pub new_sale_price: Option<Decimal>,
}

/// Función pura de costeo de la orden (F2.12).
pub fn cost_lines(
    lines: &[CostLineInput],
    discount: Decimal,
    freight: Decimal,
    freight_mode: &str,
    iva_to_cost: bool,
) -> Vec<CostLine> {
    if lines.is_empty() {
        return Vec::new();
    }

    // 1. gross_i = money(qty_i * unit_cost_i)
    let gross_vec: Vec<Decimal> = lines
        .iter()
        .map(|l| money::money(l.qty * l.unit_cost))
        .collect();

    // 2. Descuento global: repartido con pricing::allocate(discount, pesos = gross_i).
    // Las líneas con unit_cost = 0 (obsequios) tienen peso 0.
    let disc_weights: Vec<Decimal> = lines
        .iter()
        .zip(gross_vec.iter())
        .map(|(l, &g)| {
            if l.unit_cost <= Decimal::ZERO {
                Decimal::ZERO
            } else {
                g
            }
        })
        .collect();
    let disc_allocs = pricing::allocate(discount, &disc_weights);
    let net_vec: Vec<Decimal> = gross_vec
        .iter()
        .zip(disc_allocs.iter())
        .map(|(&g, &d)| (g - d).max(Decimal::ZERO))
        .collect();

    // 3. Flete:
    // con PRORRATEAR: allocate(freight, pesos = net_i); si Σ net_i == 0, los pesos son qty_i.
    // con GASTO: freight_i = 0.
    let freight_allocs = if freight_mode == "PRORRATEAR" {
        let net_sum: Decimal = net_vec.iter().copied().sum();
        let freight_weights: Vec<Decimal> = if net_sum > Decimal::ZERO {
            net_vec.clone()
        } else {
            lines.iter().map(|l| l.qty).collect()
        };
        pricing::allocate(freight, &freight_weights)
    } else {
        vec![Decimal::ZERO; lines.len()]
    };

    // 4. Impuesto: tax_i = money(net_i * rate_i).
    // 5. Al costo: cost_base_i = net_i + freight_i + (tax_i si inc_i o iva_to_cost; si no, 0).
    // 6. landed_unit_cost_i = cost_base_i / (qty_i * purchase_factor_i), redondeado a 4 decimales.
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let gross = gross_vec[i];
            let disc = disc_allocs[i];
            let net = net_vec[i];
            let f_alloc = freight_allocs[i];
            let tax = money::money(net * l.tax_rate);
            let tax_to_cost = if l.is_inc || iva_to_cost {
                tax
            } else {
                Decimal::ZERO
            };
            let cost_base = net + f_alloc + tax_to_cost;
            let total_inv_units = l.qty * l.purchase_factor;
            let landed_unit_cost = if total_inv_units > Decimal::ZERO {
                (cost_base / total_inv_units).round_dp(4)
            } else {
                Decimal::ZERO
            };

            CostLine {
                product_id: l.product_id,
                qty: l.qty,
                unit_cost: l.unit_cost,
                gross,
                discount_alloc: disc,
                net,
                freight_alloc: f_alloc,
                tax_amount: tax,
                landed_unit_cost,
                new_sale_price: l.new_sale_price,
            }
        })
        .collect()
}

const PURCHASE_SELECT: &str = r#"
select pr.id, pr.branch_id, pr.supplier_id, s.name as supplier_name,
       pr.location_id, pr.expected_at,
       pr.document_number, pr.supplier_invoice_date,
       tm.tima_codigo as status,
       pr.subtotal, pr.discount_amount, pr.freight_amount, pr.freight_mode,
       pr.payment_terms, pr.credit_days, pr.iva_to_cost,
       pr.tax_amount, pr.total_amount,
       count(pri.id) as items_count,
       pr.received_at, pr.approved_by, pr.approved_at, pr.notes, pr.created_at
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
        select pri.id, pri.product_id, p.name as product_name, p.sku, p.barcode,
               puom.tima_codigo as purchase_unit, p.purchase_factor,
               pri.qty, pri.qty_received, pri.unit_cost, pri.line_total,
               pri.discount_alloc, pri.freight_alloc,
               tx.tima_codigo as tax_code,
               null::numeric as tax_rate,
               pri.tax_amount,
               pri.landed_unit_cost,
               pri.new_sale_price,
               pri.notes
          from purchase_receipt_items pri
          join products p on p.id = pri.product_id
          left join tbl_tipos_maestra tx on tx.tima_id = pri.tax_type_id
          left join tbl_tipos_maestra puom on puom.tima_id = p.purchase_uom_id
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

    Ok(PurchaseDetail {
        purchase,
        items,
        warnings: vec![],
    })
}

pub async fn get_purchase(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_any(&["purchases:create", "purchases:approve", "purchases:receive"])?;
    let detail = get_purchase_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(detail)
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ValidatedProduct {
    id: Uuid,
    name: String,
    usage: String,
    is_composed: bool,
    purchase_factor: Decimal,
    track_lots: bool,
    tax_code: Option<String>,
    tax_type_id: Option<i32>,
    variants_count: i64,
}

async fn validate_products_for_purchase(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    product_ids: &[Uuid],
) -> Result<Vec<ValidatedProduct>, ApiError> {
    let prods = sqlx::query_as::<_, ValidatedProduct>(
        r#"
        select p.id, p.name, p.usage, p.is_composed, p.purchase_factor, p.track_lots,
               tx.tima_codigo as tax_code, p.tax_type_id,
               (select count(*) from products var where var.parent_id = p.id) as variants_count
          from products p
          left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
         where p.id = any($1) and p.tenant_id = $2
        "#,
    )
    .bind(product_ids)
    .bind(tenant_id)
    .fetch_all(&mut **tx)
    .await?;

    for pid in product_ids {
        if !prods.iter().any(|p| p.id == *pid) {
            return Err(validation(
                "PURCHASE_INVALID: Uno o más productos no existen en el tenant.",
            ));
        }
    }

    for p in &prods {
        if p.usage != "VENTA" && p.usage != "INSUMO" {
            return Err(validation(format!(
                "PURCHASE_INVALID: El producto {} tiene uso {}, solo se permite VENTA o INSUMO.",
                p.name, p.usage
            )));
        }
        if p.variants_count > 0 {
            return Err(validation(format!(
                "PURCHASE_INVALID: El producto {} es un producto padre con variantes.",
                p.name
            )));
        }
        if p.is_composed {
            return Err(validation(format!(
                "PURCHASE_INVALID: El producto {} es compuesto y no se puede comprar directamente.",
                p.name
            )));
        }
    }

    Ok(prods)
}

pub async fn cost_preview(
    pool: &PgPool,
    claims: &Claims,
    req: CostPreviewRequest,
) -> Result<CostPreviewResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_any(&["purchases:create", "purchases:approve", "purchases:receive"])?;
    let resp = cost_preview_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(resp)
}

pub async fn cost_preview_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CostPreviewRequest,
) -> Result<CostPreviewResponse, ApiError> {
    if req.items.is_empty() {
        return Err(validation("items cannot be empty"));
    }
    for item in &req.items {
        if item.qty <= Decimal::ZERO {
            return Err(validation("qty must be > 0"));
        }
        if item.unit_cost < Decimal::ZERO {
            return Err(validation("unit_cost must be >= 0"));
        }
    }

    let product_ids: Vec<Uuid> = req.items.iter().map(|i| i.product_id).collect();
    let prods = validate_products_for_purchase(tx, scope.tenant, &product_ids).await?;

    let (iva_to_cost, warnings) = match req.iva_to_cost {
        Some(v) => (v, vec![]),
        None => {
            let resp: Option<Option<bool>> =
                sqlx::query_scalar("select iva_responsible from tenants where id = $1")
                    .bind(scope.tenant)
                    .fetch_optional(&mut **tx)
                    .await?;
            match resp.flatten() {
                Some(true) => (false, vec![]),
                Some(false) => (true, vec![]),
                None => (true, vec!["IVA_REGIME_UNKNOWN".to_string()]),
            }
        }
    };

    let discount = req
        .discount_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight = req
        .freight_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight_mode = req
        .freight_mode
        .as_ref()
        .map(|m| m.trim().to_uppercase())
        .filter(|m| m == "GASTO" || m == "PRORRATEAR")
        .unwrap_or_else(|| "PRORRATEAR".to_string());

    let cost_inputs: Vec<CostLineInput> = req
        .items
        .iter()
        .map(|it| {
            let p = prods.iter().find(|p| p.id == it.product_id).unwrap();
            let tax_rate = catalog::tax_rate_for(p.tax_code.as_deref()).unwrap_or(Decimal::ZERO);
            let is_inc = p.tax_code.as_deref() == Some("INC8");
            CostLineInput {
                product_id: it.product_id,
                qty: it.qty,
                unit_cost: it.unit_cost,
                tax_rate,
                is_inc,
                purchase_factor: p.purchase_factor,
                new_sale_price: it.new_sale_price,
            }
        })
        .collect();

    let calculated = cost_lines(&cost_inputs, discount, freight, &freight_mode, iva_to_cost);

    #[derive(sqlx::FromRow)]
    struct BranchPrice {
        product_id: Uuid,
        price: Decimal,
    }
    let prices = sqlx::query_as::<_, BranchPrice>(
        "select product_id, price from branch_products where tenant_id = $1 and branch_id = $2 and product_id = any($3)",
    )
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(&product_ids)
    .fetch_all(&mut **tx)
    .await?;

    let lines: Vec<CostLineResponse> = calculated
        .into_iter()
        .map(|cl| {
            let p = prods.iter().find(|p| p.id == cl.product_id).unwrap();
            let current_price = prices
                .iter()
                .find(|pr| pr.product_id == cl.product_id)
                .map(|pr| pr.price);
            let current_margin = current_price
                .filter(|&cp| cp > Decimal::ZERO)
                .map(|cp| ((cp - cl.landed_unit_cost) / cp * Decimal::from(100)).round_dp(2));
            let new_margin = cl
                .new_sale_price
                .filter(|&np| np > Decimal::ZERO)
                .map(|np| ((np - cl.landed_unit_cost) / np * Decimal::from(100)).round_dp(2));

            CostLineResponse {
                product_id: cl.product_id,
                product_name: p.name.clone(),
                qty: cl.qty,
                unit_cost: cl.unit_cost,
                gross: cl.gross,
                discount_alloc: cl.discount_alloc,
                net: cl.net,
                freight_alloc: cl.freight_alloc,
                tax_amount: cl.tax_amount,
                landed_unit_cost: cl.landed_unit_cost,
                current_sale_price: current_price,
                new_sale_price: cl.new_sale_price,
                current_margin,
                new_margin,
            }
        })
        .collect();

    let subtotal: Decimal = lines.iter().map(|l| l.net).sum();
    let tax_amount: Decimal = lines.iter().map(|l| l.tax_amount).sum();
    let total_amount = subtotal + tax_amount + freight;

    Ok(CostPreviewResponse {
        subtotal,
        discount_amount: discount,
        freight_amount: freight,
        freight_mode,
        tax_amount,
        total_amount,
        iva_to_cost,
        warnings,
        lines,
    })
}

pub async fn create_purchase(
    pool: &PgPool,
    claims: &Claims,
    req: CreatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("purchases:create")?;
    let detail = create_purchase_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    scope.require_permission("purchases:create")?;
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

    #[derive(sqlx::FromRow)]
    struct SupplierTerms {
        payment_terms: String,
        credit_days: i32,
    }
    let sup: Option<SupplierTerms> = sqlx::query_as(
        "select payment_terms, credit_days from suppliers where id = $1 and tenant_id = $2",
    )
    .bind(req.supplier_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;
    let sup = sup.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let product_ids: Vec<Uuid> = req.items.iter().map(|i| i.product_id).collect();
    let prods = validate_products_for_purchase(tx, scope.tenant, &product_ids).await?;

    let location_id = match req.location_id {
        Some(lid) => lid,
        None => crate::locations::default_location_id_tx(tx, req.branch_id).await?,
    };

    let payment_terms = req
        .payment_terms
        .as_ref()
        .map(|t| t.trim().to_uppercase())
        .filter(|t| t == "CONTADO" || t == "CREDITO")
        .unwrap_or(sup.payment_terms);

    let credit_days = req.credit_days.unwrap_or(sup.credit_days).max(0);

    let (iva_to_cost, warnings) = match req.iva_to_cost {
        Some(v) => (v, vec![]),
        None => {
            let resp: Option<Option<bool>> =
                sqlx::query_scalar("select iva_responsible from tenants where id = $1")
                    .bind(scope.tenant)
                    .fetch_optional(&mut **tx)
                    .await?;
            match resp.flatten() {
                Some(true) => (false, vec![]),
                Some(false) => (true, vec![]),
                None => (true, vec!["IVA_REGIME_UNKNOWN".to_string()]),
            }
        }
    };

    let discount = req
        .discount_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight = req
        .freight_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight_mode = req
        .freight_mode
        .as_ref()
        .map(|m| m.trim().to_uppercase())
        .filter(|m| m == "GASTO" || m == "PRORRATEAR")
        .unwrap_or_else(|| "PRORRATEAR".to_string());

    let body = serde_json::to_value(&req).map_err(anyhow::Error::from)?;
    let claimed: Option<Uuid> = sqlx::query_scalar(
        r#"
        insert into purchase_receipts (
           id, tenant_id, branch_id, location_id, supplier_id, status_id,
           expected_at, document_number, supplier_invoice_date, notes, client_request,
           discount_amount, freight_amount, freight_mode, payment_terms, credit_days, iva_to_cost,
           subtotal, tax_amount, total_amount, created_by
        ) values (
           $1, $2, $3, $4, $5, public.tipo_id('PUR', 'PENDIENTE'),
           $6, $7, $8, $9, $10,
           $11, $12, $13, $14, $15, $16,
           0, 0, 0, $17
        )
        on conflict (id) do nothing
        returning id
        "#,
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(location_id)
    .bind(req.supplier_id)
    .bind(req.expected_at)
    .bind(clean_opt(req.document_number.clone()))
    .bind(req.supplier_invoice_date)
    .bind(clean_opt(req.notes.clone()))
    .bind(&body)
    .bind(discount)
    .bind(freight)
    .bind(&freight_mode)
    .bind(&payment_terms)
    .bind(credit_days)
    .bind(iva_to_cost)
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
            _ => Err(crate::error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "purchase id already used with a different request",
                None,
            )),
        };
    }

    let cost_inputs: Vec<CostLineInput> = req
        .items
        .iter()
        .map(|it| {
            let p = prods.iter().find(|p| p.id == it.product_id).unwrap();
            let tax_rate = catalog::tax_rate_for(p.tax_code.as_deref()).unwrap_or(Decimal::ZERO);
            let is_inc = p.tax_code.as_deref() == Some("INC8");
            CostLineInput {
                product_id: it.product_id,
                qty: it.qty,
                unit_cost: it.unit_cost,
                tax_rate,
                is_inc,
                purchase_factor: p.purchase_factor,
                new_sale_price: it.new_sale_price,
            }
        })
        .collect();

    let calculated = cost_lines(&cost_inputs, discount, freight, &freight_mode, iva_to_cost);

    for cl in &calculated {
        let p_info = prods.iter().find(|p| p.id == cl.product_id).unwrap();
        sqlx::query(
            r#"
            insert into purchase_receipt_items
               (tenant_id, branch_id, purchase_receipt_id, product_id, qty, unit_cost,
                discount_alloc, freight_alloc, tax_amount, landed_unit_cost, new_sale_price, tax_type_id)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(scope.tenant)
        .bind(req.branch_id)
        .bind(req.id)
        .bind(cl.product_id)
        .bind(cl.qty)
        .bind(cl.unit_cost)
        .bind(cl.discount_alloc)
        .bind(cl.freight_alloc)
        .bind(cl.tax_amount)
        .bind(cl.landed_unit_cost)
        .bind(cl.new_sale_price)
        .bind(p_info.tax_type_id)
        .execute(&mut **tx)
        .await?;
    }

    let subtotal: Decimal = calculated.iter().map(|l| l.net).sum();
    let tax_amount: Decimal = calculated.iter().map(|l| l.tax_amount).sum();
    let total_amount = subtotal + tax_amount + freight;

    sqlx::query(
        r#"
        update purchase_receipts
           set subtotal = $2,
               tax_amount = $3,
               total_amount = $4
         where id = $1
        "#,
    )
    .bind(req.id)
    .bind(subtotal)
    .bind(tax_amount)
    .bind(total_amount)
    .execute(&mut **tx)
    .await?;

    let mut detail = get_purchase_detail_tx(tx, scope, req.id).await?;
    detail.warnings = warnings;
    Ok(detail)
}

pub async fn update_purchase(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    req: UpdatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("purchases:create")?;
    let detail = update_purchase_tx(&mut tx, &scope, id, req).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn update_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    req: UpdatePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    scope.require_permission("purchases:create")?;

    let current: Option<(Uuid, String)> = sqlx::query_as(
        "select pr.id, tm.tima_codigo as status
           from purchase_receipts pr
           join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
          where pr.id = $1 and pr.tenant_id = $2
            for update of pr",
    )
    .bind(id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let (_id, status) = current.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if status != "PENDIENTE" {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "PURCHASE_INVALID_STATE",
            format!("Solo se pueden editar compras en estado PENDIENTE (actual: {status})."),
            Some(serde_json::json!({ "status": status })),
        ));
    }

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

    #[derive(sqlx::FromRow)]
    struct SupplierTerms {
        payment_terms: String,
        credit_days: i32,
    }
    let sup: Option<SupplierTerms> = sqlx::query_as(
        "select payment_terms, credit_days from suppliers where id = $1 and tenant_id = $2",
    )
    .bind(req.supplier_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;
    let sup = sup.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let product_ids: Vec<Uuid> = req.items.iter().map(|i| i.product_id).collect();
    let prods = validate_products_for_purchase(tx, scope.tenant, &product_ids).await?;

    let location_id = match req.location_id {
        Some(lid) => lid,
        None => crate::locations::default_location_id_tx(tx, req.branch_id).await?,
    };

    let payment_terms = req
        .payment_terms
        .as_ref()
        .map(|t| t.trim().to_uppercase())
        .filter(|t| t == "CONTADO" || t == "CREDITO")
        .unwrap_or(sup.payment_terms);

    let credit_days = req.credit_days.unwrap_or(sup.credit_days).max(0);

    let (iva_to_cost, warnings) = match req.iva_to_cost {
        Some(v) => (v, vec![]),
        None => {
            let resp: Option<Option<bool>> =
                sqlx::query_scalar("select iva_responsible from tenants where id = $1")
                    .bind(scope.tenant)
                    .fetch_optional(&mut **tx)
                    .await?;
            match resp.flatten() {
                Some(true) => (false, vec![]),
                Some(false) => (true, vec![]),
                None => (true, vec!["IVA_REGIME_UNKNOWN".to_string()]),
            }
        }
    };

    let discount = req
        .discount_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight = req
        .freight_amount
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let freight_mode = req
        .freight_mode
        .as_ref()
        .map(|m| m.trim().to_uppercase())
        .filter(|m| m == "GASTO" || m == "PRORRATEAR")
        .unwrap_or_else(|| "PRORRATEAR".to_string());

    // Eliminar líneas viejas
    sqlx::query(
        "delete from purchase_receipt_items where purchase_receipt_id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(scope.tenant)
    .execute(&mut **tx)
    .await?;

    let cost_inputs: Vec<CostLineInput> = req
        .items
        .iter()
        .map(|it| {
            let p = prods.iter().find(|p| p.id == it.product_id).unwrap();
            let tax_rate = catalog::tax_rate_for(p.tax_code.as_deref()).unwrap_or(Decimal::ZERO);
            let is_inc = p.tax_code.as_deref() == Some("INC8");
            CostLineInput {
                product_id: it.product_id,
                qty: it.qty,
                unit_cost: it.unit_cost,
                tax_rate,
                is_inc,
                purchase_factor: p.purchase_factor,
                new_sale_price: it.new_sale_price,
            }
        })
        .collect();

    let calculated = cost_lines(&cost_inputs, discount, freight, &freight_mode, iva_to_cost);

    for cl in &calculated {
        let p_info = prods.iter().find(|p| p.id == cl.product_id).unwrap();
        sqlx::query(
            r#"
            insert into purchase_receipt_items
               (tenant_id, branch_id, purchase_receipt_id, product_id, qty, unit_cost,
                discount_alloc, freight_alloc, tax_amount, landed_unit_cost, new_sale_price, tax_type_id)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(scope.tenant)
        .bind(req.branch_id)
        .bind(id)
        .bind(cl.product_id)
        .bind(cl.qty)
        .bind(cl.unit_cost)
        .bind(cl.discount_alloc)
        .bind(cl.freight_alloc)
        .bind(cl.tax_amount)
        .bind(cl.landed_unit_cost)
        .bind(cl.new_sale_price)
        .bind(p_info.tax_type_id)
        .execute(&mut **tx)
        .await?;
    }

    let subtotal: Decimal = calculated.iter().map(|l| l.net).sum();
    let tax_amount: Decimal = calculated.iter().map(|l| l.tax_amount).sum();
    let total_amount = subtotal + tax_amount + freight;

    sqlx::query(
        r#"
        update purchase_receipts
           set branch_id = $2,
               supplier_id = $3,
               location_id = $4,
               expected_at = $5,
               document_number = $6,
               supplier_invoice_date = $7,
               notes = $8,
               discount_amount = $9,
               freight_amount = $10,
               freight_mode = $11,
               payment_terms = $12,
               credit_days = $13,
               iva_to_cost = $14,
               subtotal = $15,
               tax_amount = $16,
               total_amount = $17,
               updated_at = now()
         where id = $1
        "#,
    )
    .bind(id)
    .bind(req.branch_id)
    .bind(req.supplier_id)
    .bind(location_id)
    .bind(req.expected_at)
    .bind(clean_opt(req.document_number))
    .bind(req.supplier_invoice_date)
    .bind(clean_opt(req.notes))
    .bind(discount)
    .bind(freight)
    .bind(&freight_mode)
    .bind(&payment_terms)
    .bind(credit_days)
    .bind(iva_to_cost)
    .bind(subtotal)
    .bind(tax_amount)
    .bind(total_amount)
    .execute(&mut **tx)
    .await?;

    let mut detail = get_purchase_detail_tx(tx, scope, id).await?;
    detail.warnings = warnings;
    Ok(detail)
}

pub async fn approve_purchase(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:approve")?;
    let detail = approve_purchase_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn approve_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    scope.require_permission("purchases:approve")?;

    let current: Option<(Uuid, String)> = sqlx::query_as(
        "select pr.id, tm.tima_codigo as status
           from purchase_receipts pr
           join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
          where pr.id = $1 and pr.tenant_id = $2
            for update of pr",
    )
    .bind(id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let (_id, status) = current.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if status != "PENDIENTE" {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "PURCHASE_INVALID_STATE",
            format!("No se puede aprobar una orden en estado {status}."),
            Some(serde_json::json!({ "status": status })),
        ));
    }

    sqlx::query(
        r#"
        update purchase_receipts
           set status_id = public.tipo_id('PUR', 'APROBADA'),
               approved_by = $2,
               approved_at = now(),
               updated_at = now()
         where id = $1
        "#,
    )
    .bind(id)
    .bind(scope.user)
    .execute(&mut **tx)
    .await?;

    audit::record_tx(
        tx,
        scope,
        "COMPRA_APROBADA",
        "purchase_receipts",
        id,
        None,
        None,
        None,
    )
    .await?;

    get_purchase_detail_tx(tx, scope, id).await
}

pub async fn receive_purchase(
    pool: &PgPool,
    claims: &Claims,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:receive")?;

    let status: String = sqlx::query_scalar(
        "select tm.tima_codigo from purchase_receipts pr join tbl_tipos_maestra tm on tm.tima_id = pr.status_id where pr.id = $1 and pr.tenant_id = $2"
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if status == "RECIBIDA" {
        return Err(state("purchase is already received"));
    }
    if status == "ANULADA" {
        return Err(state("cannot receive a cancelled purchase"));
    }

    // Compatibilidad Fase 1: si está PENDIENTE, auto-aprobar
    if status == "PENDIENTE" {
        approve_purchase_tx(&mut tx, &scope, purchase_id).await?;
    }

    #[derive(sqlx::FromRow)]
    struct PendingLine {
        id: Uuid,
        pending: Decimal,
    }
    let pendings = sqlx::query_as::<_, PendingLine>(
        "select id, (qty - qty_received) as pending from purchase_receipt_items where purchase_receipt_id = $1 and qty > qty_received"
    )
    .bind(purchase_id)
    .fetch_all(&mut *tx)
    .await?;

    let req = ReceivePurchaseRequest {
        id: Uuid::new_v4(),
        items: pendings
            .into_iter()
            .map(|p| ReceiveItemInput {
                purchase_item_id: p.id,
                qty: p.pending,
                lot_code: None,
                expires_at: None,
            })
            .collect(),
    };

    let res = receive_purchase_tx(&mut tx, &scope, purchase_id, req).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn receive_purchase_with_req(
    pool: &PgPool,
    claims: &Claims,
    purchase_id: Uuid,
    req: ReceivePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:receive")?;
    let res = receive_purchase_tx(&mut tx, &scope, purchase_id, req).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn receive_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    purchase_id: Uuid,
    req: ReceivePurchaseRequest,
) -> Result<PurchaseDetail, ApiError> {
    scope.require_permission("purchases:receive")?;

    // 1. Idempotencia por purchase_receptions.id
    let client_req = serde_json::to_value(&req).map_err(anyhow::Error::from)?;
    let existing_reception: Option<serde_json::Value> = sqlx::query_scalar(
        "select client_request from purchase_receptions where id = $1 and tenant_id = $2",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(stored) = existing_reception {
        if stored != client_req {
            return Err(crate::error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "Idempotency key already used with different payload.",
                None,
            ));
        }
        return get_purchase_detail_tx(tx, scope, purchase_id).await;
    }

    // 2. Lock purchase
    #[derive(sqlx::FromRow)]
    struct PurchaseLock {
        branch_id: Uuid,
        location_id: Uuid,
        supplier_id: Uuid,
        status: String,
        document_number: Option<String>,
        freight_mode: String,
        freight_amount: Decimal,
        received_at: Option<DateTime<Utc>>,
    }

    let pr = sqlx::query_as::<_, PurchaseLock>(
        r#"
        select pr.branch_id, pr.location_id, pr.supplier_id, tm.tima_codigo as status,
               pr.document_number, pr.freight_mode, pr.freight_amount, pr.received_at
          from purchase_receipts pr
          join tbl_tipos_maestra tm on tm.tima_id = pr.status_id
         where pr.id = $1 and pr.tenant_id = $2
           for update of pr
        "#,
    )
    .bind(purchase_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if pr.status != "APROBADA" && pr.status != "PARCIAL" {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "PURCHASE_INVALID_STATE",
            format!(
                "Solo se pueden recibir órdenes en estado APROBADA o PARCIAL (actual: {}).",
                pr.status
            ),
            Some(serde_json::json!({ "status": pr.status })),
        ));
    }

    if req.items.is_empty() {
        return Err(validation("Debe indicar al menos un ítem a recibir"));
    }

    sqlx::query(
        r#"
        insert into purchase_receptions (id, tenant_id, branch_id, purchase_id, received_by, received_at, client_request)
        values ($1, $2, $3, $4, $5, now(), $6)
        "#,
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(pr.branch_id)
    .bind(purchase_id)
    .bind(scope.user)
    .bind(&client_req)
    .execute(&mut **tx)
    .await?;

    let is_first_reception = pr.received_at.is_none();
    let cause = match pr.document_number.as_deref() {
        Some(d) if !d.trim().is_empty() => format!("Compra {}", d.trim()),
        _ => format!("Compra {}", &purchase_id.to_string()[..8]),
    };

    for item in req.items {
        if item.qty <= Decimal::ZERO {
            return Err(validation("qty must be > 0"));
        }

        #[derive(sqlx::FromRow)]
        #[allow(dead_code)]
        struct LineItem {
            id: Uuid,
            product_id: Uuid,
            qty: Decimal,
            qty_received: Decimal,
            unit_cost: Decimal,
            landed_unit_cost: Option<Decimal>,
            new_sale_price: Option<Decimal>,
            purchase_factor: Decimal,
            track_lots: bool,
        }

        let pri = sqlx::query_as::<_, LineItem>(
            r#"
            select pri.id, pri.product_id, pri.qty, pri.qty_received, pri.unit_cost, pri.landed_unit_cost, pri.new_sale_price,
                   p.purchase_factor, p.track_lots
              from purchase_receipt_items pri
              join products p on p.id = pri.product_id
             where pri.id = $1 and pri.purchase_receipt_id = $2 and pri.tenant_id = $3
               for update of pri
            "#,
        )
        .bind(item.purchase_item_id)
        .bind(purchase_id)
        .bind(scope.tenant)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| validation("purchase_item_id not found in this purchase"))?;

        let pending = pri.qty - pri.qty_received;
        if item.qty > pending {
            return Err(crate::error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "RECEIVE_EXCEEDS_PENDING",
                format!(
                    "La cantidad a recibir ({}) supera lo pendiente ({})",
                    item.qty, pending
                ),
                Some(serde_json::json!({
                    "purchase_item_id": item.purchase_item_id,
                    "pending": pending
                })),
            ));
        }

        // 4. Lotes
        let mut lot_id = None;
        if pri.track_lots {
            let code = item
                .lot_code
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty());
            let exp = item.expires_at;
            match (code, exp) {
                (Some(c), Some(e)) if (1..=30).contains(&c.len()) => {
                    let existing_lot: Option<(Uuid, chrono::NaiveDate)> = sqlx::query_as(
                        "select id, expires_at from lots where location_id = $1 and product_id = $2 and code = $3",
                    )
                    .bind(pr.location_id)
                    .bind(pri.product_id)
                    .bind(c)
                    .fetch_optional(&mut **tx)
                    .await?;

                    let lid = match existing_lot {
                        Some((id, existing_exp)) => {
                            if existing_exp != e {
                                return Err(crate::error::coded(
                                    StatusCode::UNPROCESSABLE_ENTITY,
                                    "LOT_EXPIRY_MISMATCH",
                                    "Ese lote ya existe con otra fecha de vencimiento.",
                                    None,
                                ));
                            }
                            id
                        }
                        None => {
                            sqlx::query_scalar(
                                r#"
                                insert into lots (tenant_id, branch_id, location_id, product_id, code, expires_at, qty)
                                values ($1, $2, $3, $4, $5, $6, 0)
                                returning id
                                "#,
                            )
                            .bind(scope.tenant)
                            .bind(pr.branch_id)
                            .bind(pr.location_id)
                            .bind(pri.product_id)
                            .bind(c)
                            .bind(e)
                            .fetch_one(&mut **tx)
                            .await?
                        }
                    };
                    lot_id = Some(lid);
                }
                _ => {
                    return Err(crate::error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "LOT_REQUIRED",
                        "El producto requiere código de lote (1-30 caracteres) y fecha de vencimiento.",
                        None,
                    ));
                }
            }
        }

        // 5. Stock: apply_delta_tx
        let landed_cost = pri.landed_unit_cost.unwrap_or(Decimal::ZERO);
        let stock_delta = item.qty * pri.purchase_factor;
        inventory::apply_delta_tx(
            tx,
            scope,
            inventory::StockDelta {
                branch_id: pr.branch_id,
                location_id: pr.location_id,
                product_id: pri.product_id,
                delta: stock_delta,
                movement: "REPOSICION",
                unit_cost: Some(landed_cost),
                lot_id,
                ref_type: "COMPRA",
                ref_id: purchase_id,
                cause: Some(cause.clone()),
            },
        )
        .await?;

        // 6. Update qty_received and supplier_products.last_cost
        sqlx::query(
            "update purchase_receipt_items set qty_received = qty_received + $1 where id = $2",
        )
        .bind(item.qty)
        .bind(item.purchase_item_id)
        .execute(&mut **tx)
        .await?;

        let supplier_unit_cost = landed_cost * pri.purchase_factor;
        sqlx::query(
            r#"
            insert into supplier_products (tenant_id, supplier_id, product_id, last_cost)
            values ($1, $2, $3, $4)
            on conflict (supplier_id, product_id) do update
            set last_cost = excluded.last_cost
            "#,
        )
        .bind(scope.tenant)
        .bind(pr.supplier_id)
        .bind(pri.product_id)
        .bind(supplier_unit_cost)
        .execute(&mut **tx)
        .await?;

        // Actualizar cost_price en products con unit_cost (compatibilidad F1 y respaldo)
        sqlx::query("update products set cost_price = $1 where id = $2 and tenant_id = $3")
            .bind(pri.unit_cost)
            .bind(pri.product_id)
            .bind(scope.tenant)
            .execute(&mut **tx)
            .await?;

        // Insert purchase_reception_items
        sqlx::query(
            r#"
            insert into purchase_reception_items (tenant_id, reception_id, purchase_item_id, qty, lot_id)
            values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(scope.tenant)
        .bind(req.id)
        .bind(item.purchase_item_id)
        .bind(item.qty)
        .bind(lot_id)
        .execute(&mut **tx)
        .await?;

        // 7. new_sale_price
        if let Some(nsp) = pri.new_sale_price {
            sqlx::query(
                r#"
                insert into branch_products (tenant_id, branch_id, product_id, price)
                values ($1, $2, $3, $4)
                on conflict (branch_id, product_id) do update
                set price = excluded.price
                "#,
            )
            .bind(scope.tenant)
            .bind(pr.branch_id)
            .bind(pri.product_id)
            .bind(nsp)
            .execute(&mut **tx)
            .await?;

            audit::record_tx(
                tx,
                scope,
                "PRECIO_EDITADO",
                "branch_products",
                pri.product_id,
                None,
                Some(serde_json::json!({
                    "branch_id": pr.branch_id,
                    "price": nsp
                })),
                None,
            )
            .await?;
        }
    }

    // 8. Estado final de la orden
    let pending_count: i64 = sqlx::query_scalar(
        "select count(*) from purchase_receipt_items where purchase_receipt_id = $1 and qty_received < qty",
    )
    .bind(purchase_id)
    .fetch_one(&mut **tx)
    .await?;

    let final_status = if pending_count == 0 {
        "RECIBIDA"
    } else {
        "PARCIAL"
    };
    sqlx::query(
        r#"
        update purchase_receipts
           set status_id = public.tipo_id('PUR', $1),
               received_at = coalesce(received_at, now()),
               updated_at = now()
         where id = $2
        "#,
    )
    .bind(final_status)
    .bind(purchase_id)
    .execute(&mut **tx)
    .await?;

    // 9. Cuenta por pagar (F2.13)
    payables::upsert_from_purchase_tx(tx, scope, purchase_id).await?;

    // 10. Gasto por flete si freight_mode = GASTO en la primera recepcion
    if pr.freight_mode == "GASTO" && pr.freight_amount > Decimal::ZERO && is_first_reception {
        let cat_id: Uuid = match sqlx::query_scalar(
            "select id from expense_categories where tenant_id = $1 and lower(name) = 'fletes'",
        )
        .bind(scope.tenant)
        .fetch_optional(&mut **tx)
        .await?
        {
            Some(cid) => cid,
            None => {
                let cid = Uuid::new_v4();
                sqlx::query(
                    "insert into expense_categories (id, tenant_id, name, active) values ($1, $2, 'Fletes', true)",
                )
                .bind(cid)
                .bind(scope.tenant)
                .execute(&mut **tx)
                .await?;
                cid
            }
        };

        let (prefix, num) = sequences::next_tx(tx, scope, pr.branch_id, "GASTO")
            .await
            .unwrap_or_else(|_| ("GAS".into(), 1));
        let exp_id = Uuid::new_v4();
        let desc = format!(
            "Flete compra {}",
            pr.document_number.as_deref().unwrap_or("")
        );
        let today = chrono::Utc::now().date_naive();
        sqlx::query(
            r#"
            insert into expenses (
                id, tenant_id, branch_id, prefix, number, category_id, supplier_id,
                description, amount, tax_amount, expense_date, status, created_by, decided_by, decided_at, client_request
            ) values (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, 0, $10, 'APROBADO', $11, $11, now(), '{}'::jsonb
            )
            "#,
        )
        .bind(exp_id)
        .bind(scope.tenant)
        .bind(pr.branch_id)
        .bind(&prefix)
        .bind(num)
        .bind(cat_id)
        .bind(pr.supplier_id)
        .bind(desc.trim())
        .bind(pr.freight_amount)
        .bind(today)
        .bind(scope.user)
        .execute(&mut **tx)
        .await?;

        let pay_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into payables (
                id, tenant_id, branch_id, supplier_id, source_type, source_id,
                amount_total, amount_paid, status, due_date
            ) values (
                $1, $2, $3, $4, 'GASTO', $5,
                $6, 0, 'ABIERTA', $7
            )
            "#,
        )
        .bind(pay_id)
        .bind(scope.tenant)
        .bind(pr.branch_id)
        .bind(pr.supplier_id)
        .bind(exp_id)
        .bind(pr.freight_amount)
        .bind(today)
        .execute(&mut **tx)
        .await?;
    }

    // 11. Auditoria
    audit::record_tx(
        tx,
        scope,
        "COMPRA_RECIBIDA",
        "purchase_receipts",
        purchase_id,
        None,
        Some(serde_json::json!({
            "reception_id": req.id,
            "status": final_status
        })),
        None,
    )
    .await?;

    get_purchase_detail_tx(tx, scope, purchase_id).await
}

pub async fn cancel_purchase(
    pool: &PgPool,
    claims: &Claims,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("purchases:create")?;
    let result = cancel_purchase_tx(&mut tx, &scope, purchase_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn cancel_purchase_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    purchase_id: Uuid,
) -> Result<PurchaseDetail, ApiError> {
    scope.require_permission("purchases:create")?;

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
    if status != "PENDIENTE" && status != "APROBADA" {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "PURCHASE_INVALID_STATE",
            format!("No se puede anular una orden en estado {status}."),
            Some(serde_json::json!({ "status": status })),
        ));
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

    audit::record_tx(
        tx,
        scope,
        "COMPRA_ANULADA",
        "purchase_receipts",
        purchase_id,
        None,
        None,
        None,
    )
    .await?;

    get_purchase_detail_tx(tx, scope, purchase_id).await
}

pub async fn list_purchases(
    pool: &PgPool,
    claims: &Claims,
    filter: PurchaseFilter,
) -> Result<Vec<PurchaseRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_any(&["purchases:create", "purchases:approve", "purchases:receive"])?;

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
    scope.require_any(&["purchases:create", "purchases:approve", "purchases:receive"])?;

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

// ============================================================
// Sugerencias de compra (F2.17)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierHeader {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseSuggestionItem {
    pub product_id: Uuid,
    pub name: String,
    pub on_hand: Decimal,
    pub min_qty: Decimal,
    pub suggested_qty: Decimal,
    pub purchase_uom: Option<String>,
    pub last_cost: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseSuggestionGroup {
    pub supplier: Option<SupplierHeader>,
    pub items: Vec<PurchaseSuggestionItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateFromSuggestionsRequest {
    pub branch_id: Uuid,
    pub supplier_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SuggestionsQuery {
    pub branch_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct SuggestionDbRow {
    product_id: Uuid,
    name: String,
    on_hand: Decimal,
    min_qty: Decimal,
    reorder_qty: Option<Decimal>,
    max_qty: Option<Decimal>,
    purchase_factor: Decimal,
    cost_price: Option<Decimal>,
    purchase_uom: Option<String>,
    supplier_id: Option<Uuid>,
    supplier_name: Option<String>,
    last_cost: Option<Decimal>,
    min_order_qty: Option<Decimal>,
}

pub async fn get_purchase_suggestions_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
) -> Result<Vec<PurchaseSuggestionGroup>, ApiError> {
    scope.require_permission("purchases:create")?;
    tenancy::ensure_branch_tx(tx, scope, branch_id).await?;

    let rows = sqlx::query_as::<_, SuggestionDbRow>(
        r#"
        select p.id as product_id,
               p.name,
               st.on_hand,
               st.min_qty,
               p.reorder_qty,
               p.max_qty,
               p.purchase_factor,
               p.cost_price,
               puom.tima_codigo as purchase_uom,
               s.id as supplier_id,
               s.name as supplier_name,
               sp.last_cost,
               sp.min_order_qty
          from products p
          join (
            select product_id,
                   coalesce(sum(on_hand), 0) as on_hand,
                   coalesce(sum(min_qty), 0) as min_qty
              from stock
             where tenant_id = $1 and branch_id = $2
             group by product_id
            having coalesce(sum(min_qty), 0) > 0
               and coalesce(sum(on_hand), 0) <= coalesce(sum(min_qty), 0)
          ) st on st.product_id = p.id
          left join tbl_tipos_maestra puom on puom.tima_id = p.purchase_uom_id
          left join supplier_products sp on sp.product_id = p.id and sp.tenant_id = $1 and sp.preferred = true
          left join suppliers s on s.id = sp.supplier_id and s.tenant_id = $1 and s.active = true and s.archived_at is null
         where p.tenant_id = $1
           and p.usage in ('VENTA', 'INSUMO')
           and p.archived_at is null
           and p.has_variants = false
           and not exists (select 1 from products var where var.parent_id = p.id)
         order by s.name nulls last, p.name asc
        "#,
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut groups_map: std::collections::BTreeMap<
        Option<(String, Uuid)>,
        Vec<PurchaseSuggestionItem>,
    > = std::collections::BTreeMap::new();

    for row in rows {
        let sugerida_inv = if let Some(rq) = row.reorder_qty {
            if rq > Decimal::ZERO {
                rq
            } else if let Some(mq) = row.max_qty {
                (mq - row.on_hand).max(Decimal::ONE)
            } else {
                (Decimal::from(2) * row.min_qty - row.on_hand).max(Decimal::ONE)
            }
        } else if let Some(mq) = row.max_qty {
            (mq - row.on_hand).max(Decimal::ONE)
        } else {
            (Decimal::from(2) * row.min_qty - row.on_hand).max(Decimal::ONE)
        };

        let factor = if row.purchase_factor > Decimal::ZERO {
            row.purchase_factor
        } else {
            Decimal::ONE
        };

        let mut suggested_purchase = (sugerida_inv / factor).ceil();
        if suggested_purchase < Decimal::ONE {
            suggested_purchase = Decimal::ONE;
        }

        if row.supplier_id.is_some()
            && row
                .min_order_qty
                .is_some_and(|m| m > Decimal::ZERO && suggested_purchase < m)
        {
            suggested_purchase = row.min_order_qty.unwrap();
        }

        let last_cost = if let Some(lc) = row.last_cost {
            if lc > Decimal::ZERO {
                Some(lc)
            } else {
                row.cost_price.map(|cp| money::money(cp * factor))
            }
        } else {
            row.cost_price.map(|cp| money::money(cp * factor))
        };

        let item = PurchaseSuggestionItem {
            product_id: row.product_id,
            name: row.name,
            on_hand: row.on_hand,
            min_qty: row.min_qty,
            suggested_qty: suggested_purchase,
            purchase_uom: row.purchase_uom,
            last_cost,
        };

        let key = match (row.supplier_name, row.supplier_id) {
            (Some(name), Some(id)) => Some((name, id)),
            _ => None,
        };

        groups_map.entry(key).or_default().push(item);
    }

    let mut result: Vec<PurchaseSuggestionGroup> = Vec::new();
    let mut null_group_items = None;

    for (k, items) in groups_map {
        match k {
            Some((name, id)) => {
                result.push(PurchaseSuggestionGroup {
                    supplier: Some(SupplierHeader { id, name }),
                    items,
                });
            }
            None => {
                null_group_items = Some(items);
            }
        }
    }

    if let Some(items) = null_group_items {
        result.push(PurchaseSuggestionGroup {
            supplier: None,
            items,
        });
    }

    Ok(result)
}

pub async fn get_purchase_suggestions(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<PurchaseSuggestionGroup>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("purchases:create")?;
    let res = get_purchase_suggestions_tx(&mut tx, &scope, branch_id).await?;
    tx.commit().await?;
    Ok(res)
}

pub async fn create_purchases_from_suggestions_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateFromSuggestionsRequest,
) -> Result<Vec<Uuid>, ApiError> {
    scope.require_permission("purchases:create")?;
    tenancy::ensure_branch_tx(tx, scope, req.branch_id).await?;

    if req.supplier_ids.is_empty() {
        return Ok(Vec::new());
    }

    let groups = get_purchase_suggestions_tx(tx, scope, req.branch_id).await?;
    let mut order_ids = Vec::new();

    for sup_id in req.supplier_ids {
        if let Some(group) = groups
            .iter()
            .find(|g| g.supplier.as_ref().is_some_and(|s| s.id == sup_id))
        {
            if group.items.is_empty() {
                continue;
            }

            let order_id = Uuid::new_v4();
            let items: Vec<PurchaseItemRequest> = group
                .items
                .iter()
                .map(|it| PurchaseItemRequest {
                    product_id: it.product_id,
                    qty: it.suggested_qty,
                    unit_cost: it.last_cost.unwrap_or(Decimal::ZERO),
                    new_sale_price: None,
                })
                .collect();

            let purchase_req = CreatePurchaseRequest {
                id: order_id,
                branch_id: req.branch_id,
                supplier_id: sup_id,
                location_id: None,
                expected_at: None,
                document_number: None,
                supplier_invoice_date: None,
                notes: Some("Sugerencia de compra automática".to_string()),
                discount_amount: None,
                freight_amount: None,
                freight_mode: Some("PRORRATEAR".to_string()),
                payment_terms: None,
                credit_days: None,
                iva_to_cost: None,
                items,
            };

            create_purchase_tx(tx, scope, purchase_req).await?;
            order_ids.push(order_id);
        }
    }

    Ok(order_ids)
}

pub async fn create_purchases_from_suggestions(
    pool: &PgPool,
    claims: &Claims,
    req: CreateFromSuggestionsRequest,
) -> Result<Vec<Uuid>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("purchases:create")?;
    let ids = create_purchases_from_suggestions_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(ids)
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

    #[test]
    fn cost_lines_allocation_and_gift_test() {
        use std::str::FromStr;
        let lines = vec![
            CostLineInput {
                product_id: Uuid::new_v4(),
                qty: Decimal::from(10),
                unit_cost: Decimal::from(100),
                tax_rate: Decimal::from_str("0.19").unwrap(),
                is_inc: false,
                purchase_factor: Decimal::ONE,
                new_sale_price: None,
            },
            CostLineInput {
                product_id: Uuid::new_v4(),
                qty: Decimal::from(5),
                unit_cost: Decimal::from(200),
                tax_rate: Decimal::from_str("0.08").unwrap(),
                is_inc: true,
                purchase_factor: Decimal::ONE,
                new_sale_price: None,
            },
            CostLineInput {
                product_id: Uuid::new_v4(),
                qty: Decimal::from(2),
                unit_cost: Decimal::ZERO,
                tax_rate: Decimal::ZERO,
                is_inc: false,
                purchase_factor: Decimal::ONE,
                new_sale_price: None,
            },
        ];

        let discount = Decimal::from(150);
        let freight = Decimal::from(60);

        let res = cost_lines(&lines, discount, freight, "PRORRATEAR", false);
        assert_eq!(res.len(), 3);
        assert_eq!(res[2].discount_alloc, Decimal::ZERO);
        assert_eq!(res[2].net, Decimal::ZERO);

        let sum_disc: Decimal = res.iter().map(|l| l.discount_alloc).sum();
        assert_eq!(sum_disc, discount);

        let sum_freight: Decimal = res.iter().map(|l| l.freight_alloc).sum();
        assert_eq!(sum_freight, freight);

        assert_eq!(
            res[0].landed_unit_cost,
            Decimal::from_str("95.5000").unwrap()
        );
        assert_eq!(
            res[1].landed_unit_cost,
            Decimal::from_str("205.8000").unwrap()
        );

        let res_iva_cost = cost_lines(&lines, discount, freight, "PRORRATEAR", true);
        assert_eq!(
            res_iva_cost[0].landed_unit_cost,
            Decimal::from_str("113.0750").unwrap()
        );
    }
}

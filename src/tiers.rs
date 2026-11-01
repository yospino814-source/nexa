#![allow(clippy::collapsible_if)]

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PriceTier {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub kind: String,
    pub adjustment: String,
    pub value: Decimal,
    pub units_per_package: Option<Decimal>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePriceTierRequest {
    pub name: String,
    pub kind: String,
    pub adjustment: String,
    #[serde(default)]
    pub value: Decimal,
    #[serde(default)]
    pub units_per_package: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePriceTierRequest {
    pub name: String,
    pub kind: String,
    pub adjustment: String,
    pub value: Decimal,
    #[serde(default)]
    pub units_per_package: Option<Decimal>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProductTierPriceRow {
    pub tier_id: Uuid,
    pub price: Option<Decimal>,
    pub units_per_package: Option<Decimal>,
    pub barcode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetProductTierPriceInput {
    pub tier_id: Uuid,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub units_per_package: Option<Decimal>,
    #[serde(default)]
    pub barcode: Option<String>,
}

fn validate_tier(
    name: &str,
    kind: &str,
    adjustment: &str,
    value: Decimal,
    units_per_package: Option<Decimal>,
) -> Result<(), ApiError> {
    let name_trimmed = name.trim();
    if name_trimmed.is_empty() || name_trimmed.len() > 40 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "El nombre debe tener entre 1 y 40 caracteres",
            None,
        ));
    }
    if kind != "CLIENTE" && kind != "PRESENTACION" {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "El tipo de tarifa debe ser CLIENTE o PRESENTACION",
            None,
        ));
    }
    if !matches!(
        adjustment,
        "NINGUNO"
            | "DESCUENTO_PCT"
            | "RECARGO_PCT"
            | "DESCUENTO_FIJO"
            | "RECARGO_FIJO"
            | "PRECIO_FIJO"
    ) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "Ajuste no válido",
            None,
        ));
    }
    if value < Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "El valor no puede ser negativo",
            None,
        ));
    }
    if adjustment == "DESCUENTO_PCT" && value > Decimal::from(100) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "El descuento porcentual no puede superar el 100%",
            None,
        ));
    }
    if adjustment == "RECARGO_PCT" && value > Decimal::from(1000) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "El recargo porcentual no puede superar el 1000%",
            None,
        ));
    }
    if kind == "CLIENTE" && units_per_package.is_some() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_INVALID",
            "Las tarifas de cliente no llevan unidades por empaque",
            None,
        ));
    }
    if let Some(u) = units_per_package {
        if u <= Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "TIER_INVALID",
                "Las unidades por empaque deben ser mayores a cero",
                None,
            ));
        }
    }
    Ok(())
}

pub async fn list_tiers(pool: &PgPool, claims: &Claims) -> Result<Vec<PriceTier>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:sell")?;

    let rows = sqlx::query_as::<_, PriceTier>(
        "select id, tenant_id, name, kind, adjustment, value, units_per_package, active, created_at
           from price_tiers
          where tenant_id = $1
          order by kind, name",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

pub async fn create_tier(
    pool: &PgPool,
    claims: &Claims,
    req: CreatePriceTierRequest,
) -> Result<PriceTier, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    validate_tier(
        &req.name,
        &req.kind,
        &req.adjustment,
        req.value,
        req.units_per_package,
    )?;

    let row = sqlx::query_as::<_, PriceTier>(
        "insert into price_tiers (tenant_id, name, kind, adjustment, value, units_per_package)
         values ($1, $2, $3, $4, $5, $6)
         returning id, tenant_id, name, kind, adjustment, value, units_per_package, active, created_at",
    )
    .bind(scope.tenant)
    .bind(req.name.trim())
    .bind(&req.kind)
    .bind(&req.adjustment)
    .bind(req.value)
    .bind(req.units_per_package)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

pub async fn update_tier(
    pool: &PgPool,
    claims: &Claims,
    tier_id: Uuid,
    req: UpdatePriceTierRequest,
) -> Result<PriceTier, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let existing: Option<String> =
        sqlx::query_scalar("select kind from price_tiers where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(tier_id)
            .fetch_optional(&mut *tx)
            .await?;

    let current_kind = existing.ok_or(DomainError::NotFound)?;
    if current_kind != req.kind {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "TIER_KIND_IMMUTABLE",
            "No se puede cambiar el tipo de una tarifa",
            None,
        ));
    }

    validate_tier(
        &req.name,
        &req.kind,
        &req.adjustment,
        req.value,
        req.units_per_package,
    )?;

    let row = sqlx::query_as::<_, PriceTier>(
        "update price_tiers
            set name = $3,
                adjustment = $4,
                value = $5,
                units_per_package = $6,
                active = $7
          where tenant_id = $1 and id = $2
         returning id, tenant_id, name, kind, adjustment, value, units_per_package, active, created_at",
    )
    .bind(scope.tenant)
    .bind(tier_id)
    .bind(req.name.trim())
    .bind(&req.adjustment)
    .bind(req.value)
    .bind(req.units_per_package)
    .bind(req.active)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

pub async fn get_product_tier_prices(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
) -> Result<Vec<ProductTierPriceRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:sell")?;

    let rows = sqlx::query_as::<_, ProductTierPriceRow>(
        "select tier_id, price, units_per_package, barcode
           from product_tier_prices
          where tenant_id = $1 and product_id = $2",
    )
    .bind(scope.tenant)
    .bind(product_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

pub async fn set_product_tier_prices(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
    items: Vec<SetProductTierPriceInput>,
) -> Result<Vec<ProductTierPriceRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let row_meta: Option<(bool, Option<Uuid>)> = sqlx::query_as(
        "select has_variants, parent_id from products where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((has_variants, parent_id)) = row_meta {
        if has_variants || parent_id.is_some() {
            for it in &items {
                let kind: Option<String> = sqlx::query_scalar(
                    "select kind from price_tiers where tenant_id = $1 and id = $2",
                )
                .bind(scope.tenant)
                .bind(it.tier_id)
                .fetch_optional(&mut *tx)
                .await?;
                if kind.as_deref() == Some("PRESENTACION") {
                    return Err(error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "VARIANTS_AND_PRESENTATIONS",
                        "No se pueden asignar presentaciones a un producto con variantes ni a una variante",
                        None,
                    ));
                }
            }
        }
    }

    // Validar unicidad de códigos de barras
    let mut seen_barcodes = std::collections::HashSet::new();
    for it in &items {
        if let Some(b) = &it.barcode {
            let b_trimmed = b.trim();
            if !b_trimmed.is_empty() {
                if !seen_barcodes.insert(b_trimmed.to_string()) {
                    return Err(error::coded(
                        StatusCode::CONFLICT,
                        "BARCODE_TAKEN",
                        "El código de barras está duplicado en la solicitud",
                        None,
                    ));
                }

                // Verificar si choca con products.barcode de otro producto
                let exists_product: Option<bool> = sqlx::query_scalar(
                    "select true from products where tenant_id = $1 and barcode = $2 and id != $3",
                )
                .bind(scope.tenant)
                .bind(b_trimmed)
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                if exists_product.unwrap_or(false) {
                    return Err(error::coded(
                        StatusCode::CONFLICT,
                        "BARCODE_TAKEN",
                        "El código de barras ya pertenece a un producto",
                        None,
                    ));
                }

                // Verificar si choca con product_tier_prices de otro producto
                let exists_tier: Option<bool> = sqlx::query_scalar(
                    "select true from product_tier_prices where tenant_id = $1 and barcode = $2 and product_id != $3",
                )
                .bind(scope.tenant)
                .bind(b_trimmed)
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                if exists_tier.unwrap_or(false) {
                    return Err(error::coded(
                        StatusCode::CONFLICT,
                        "BARCODE_TAKEN",
                        "El código de barras ya está asignado a otra presentación",
                        None,
                    ));
                }
            }
        }
    }

    // Reemplazar filas del producto
    sqlx::query("delete from product_tier_prices where tenant_id = $1 and product_id = $2")
        .bind(scope.tenant)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    for it in &items {
        let clean_barcode = it
            .barcode
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        sqlx::query(
            "insert into product_tier_prices (tenant_id, product_id, tier_id, price, units_per_package, barcode)
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(scope.tenant)
        .bind(product_id)
        .bind(it.tier_id)
        .bind(it.price)
        .bind(it.units_per_package)
        .bind(clean_barcode)
        .execute(&mut *tx)
        .await?;
    }

    let rows = sqlx::query_as::<_, ProductTierPriceRow>(
        "select tier_id, price, units_per_package, barcode
           from product_tier_prices
          where tenant_id = $1 and product_id = $2",
    )
    .bind(scope.tenant)
    .bind(product_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

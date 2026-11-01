#![allow(clippy::collapsible_if)]

use axum::http::StatusCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

fn default_kind() -> String {
    "FISICO".to_string()
}

fn default_usage() -> String {
    "VENTA".to_string()
}

fn default_purchase_factor() -> Decimal {
    Decimal::ONE
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInput {
    pub name: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_usage")]
    pub usage: String,
    pub unit_measure_code: String,
    pub purchase_uom_code: Option<String>,
    #[serde(default = "default_purchase_factor")]
    pub purchase_factor: Decimal,
    pub tax_code: String, // "IVA19" | "IVA5" | "INC8" | "EXENTO"
    pub cost_price: Option<Decimal>,
    pub base_price: Option<Decimal>,
    pub reorder_qty: Option<Decimal>,
    pub max_qty: Option<Decimal>,
    #[serde(default)]
    pub track_lots: bool,
    #[serde(default = "default_true")]
    pub allow_price_override: bool,
    #[serde(default)]
    pub has_variants: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub kind: String,
    pub usage: String,
    pub price: Option<Decimal>,
    pub cost_price: Option<Decimal>,
    pub tax_code: String,
    pub on_hand: Decimal,
    pub category_name: Option<String>,
    pub has_variants: bool,
    pub parent_id: Option<Uuid>,
    pub archived: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct ProductDetailDbRow {
    pub id: Uuid,
    pub name: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub kind: String,
    pub usage: String,
    pub price: Option<Decimal>,
    pub cost_price: Option<Decimal>,
    pub tax_code: Option<String>,
    pub on_hand: Decimal,
    pub category_name: Option<String>,
    pub has_variants: bool,
    pub parent_id: Option<Uuid>,
    pub archived: bool,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub unit_measure_code: String,
    pub purchase_uom_code: Option<String>,
    pub purchase_factor: Decimal,
    pub reorder_qty: Option<Decimal>,
    pub max_qty: Option<Decimal>,
    pub track_lots: bool,
    pub allow_price_override: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LocationStockRow {
    pub location_id: Uuid,
    pub location_name: String,
    pub on_hand: Decimal,
    pub reserved: Decimal,
    pub min_qty: Option<Decimal>,
    pub avg_cost: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetail {
    #[serde(flatten)]
    pub row: ProductRow,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub unit_measure_code: String,
    pub purchase_uom_code: Option<String>,
    pub purchase_factor: Decimal,
    pub reorder_qty: Option<Decimal>,
    pub max_qty: Option<Decimal>,
    pub track_lots: bool,
    pub allow_price_override: bool,
    pub description: Option<String>,
    pub variants: Vec<ProductRow>,
    pub tier_prices: Vec<crate::tiers::ProductTierPriceRow>,
    pub option_groups: Vec<serde_json::Value>,
    pub stock_by_location: Vec<LocationStockRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductListParams {
    pub branch_id: Option<Uuid>,
    pub q: Option<String>,
    pub category_id: Option<Uuid>,
    pub usage: Option<String>,
    #[serde(default)]
    pub archived: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductListResponse {
    pub items: Vec<ProductRow>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateVariantInput {
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub attrs: BTreeMap<String, String>,
    pub price: Decimal,
    pub cost_price: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateBranchProductInput {
    pub branch_id: Uuid,
    pub price: Decimal,
    pub category_id: Option<Uuid>,
    #[serde(default = "default_true")]
    pub is_available: bool,
}

pub fn validate_product_input(input: &ProductInput) -> Result<(), ApiError> {
    let name_trimmed = input.name.trim();
    if name_trimmed.is_empty() || name_trimmed.chars().count() > 120 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "El nombre debe tener entre 1 y 120 caracteres",
            None,
        ));
    }

    if let Some(sku) = &input.sku {
        let sku_trimmed = sku.trim();
        if !sku_trimmed.is_empty() {
            if sku_trimmed.chars().count() > 40
                || !sku_trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PRODUCT_INVALID",
                    "El SKU debe tener entre 1 y 40 caracteres válidos",
                    None,
                ));
            }
        }
    }

    if let Some(barcode) = &input.barcode {
        let b_trimmed = barcode.trim();
        if !b_trimmed.is_empty() {
            if b_trimmed.contains("E+") || b_trimmed.contains("e+") {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "BARCODE_SCIENTIFIC",
                    "El código de barras contiene notación científica de Excel",
                    None,
                ));
            }
            if b_trimmed.chars().count() < 6
                || b_trimmed.chars().count() > 14
                || !b_trimmed.chars().all(|c| c.is_ascii_digit())
            {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PRODUCT_INVALID",
                    "El código de barras debe tener entre 6 y 14 dígitos",
                    None,
                ));
            }
        }
    }

    if let Some(bp) = input.base_price {
        if bp < Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "El precio base no puede ser negativo",
                None,
            ));
        }
    }
    if let Some(cp) = input.cost_price {
        if cp < Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "El precio de costo no puede ser negativo",
                None,
            ));
        }
    }

    if input.usage == "VENTA" && input.base_price.is_none() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "Los productos para venta requieren precio base",
            None,
        ));
    }

    let valid_tax_codes = ["IVA19", "IVA5", "INC8", "EXENTO"];
    if !valid_tax_codes.contains(&input.tax_code.as_str()) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "Impuesto no válido (debe ser IVA19, IVA5, INC8 o EXENTO)",
            None,
        ));
    }

    let valid_kinds = ["FISICO", "SERVICIO", "PREPARADO"];
    if !valid_kinds.contains(&input.kind.as_str()) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "Tipo de producto no válido",
            None,
        ));
    }

    let valid_usages = ["VENTA", "INSUMO", "INTERNO"];
    if !valid_usages.contains(&input.usage.as_str()) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "Uso de producto no válido",
            None,
        ));
    }

    Ok(())
}

async fn check_sku_unique(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    sku: &str,
    exclude_product_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from products where tenant_id = $1 and lower(sku) = lower($2) and ($3::uuid is null or id != $3))",
    )
    .bind(tenant_id)
    .bind(sku)
    .bind(exclude_product_id)
    .fetch_one(&mut **tx)
    .await?;

    if exists {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "SKU_TAKEN",
            "El SKU ya está en uso por otro producto",
            None,
        ));
    }
    Ok(())
}

async fn check_barcode_unique(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    barcode: &str,
    exclude_product_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let exists_prod: bool = sqlx::query_scalar(
        "select exists(select 1 from products where tenant_id = $1 and barcode = $2 and ($3::uuid is null or id != $3))",
    )
    .bind(tenant_id)
    .bind(barcode)
    .bind(exclude_product_id)
    .fetch_one(&mut **tx)
    .await?;

    if exists_prod {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "BARCODE_TAKEN",
            "El código de barras ya está en uso por otro producto",
            None,
        ));
    }

    let exists_tier: bool = sqlx::query_scalar(
        "select exists(select 1 from product_tier_prices where tenant_id = $1 and barcode = $2 and ($3::uuid is null or product_id != $3))",
    )
    .bind(tenant_id)
    .bind(barcode)
    .bind(exclude_product_id)
    .fetch_one(&mut **tx)
    .await?;

    if exists_tier {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "BARCODE_TAKEN",
            "El código de barras ya está en uso por una presentación",
            None,
        ));
    }

    Ok(())
}

async fn resolve_uom_id(tx: &mut Tx<'_>, code: &str) -> Result<i32, ApiError> {
    let id_opt: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra where tima_codigo = $1 and padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM')",
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;

    id_opt.ok_or_else(|| {
        error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            format!("Unidad de medida no encontrada: {code}"),
            None,
        )
    })
}

async fn resolve_tax_type_id(tx: &mut Tx<'_>, code: &str) -> Result<Option<i32>, ApiError> {
    if code == "EXENTO" {
        return Ok(None);
    }
    let id_opt: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra where tima_codigo = $1 and padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'IMP')",
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;

    let id = id_opt.ok_or_else(|| {
        error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            format!("Tipo de impuesto no encontrado: {code}"),
            None,
        )
    })?;
    Ok(Some(id))
}

pub async fn list_products(
    pool: &PgPool,
    claims: &Claims,
    params: ProductListParams,
) -> Result<ProductListResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:read")?;

    let branch_id = params.branch_id.or(scope.branch);
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    let archived_filter = params.archived.unwrap_or(false);
    let q_pattern = params
        .q
        .as_deref()
        .map(|s| format!("%{}%", s.trim().to_lowercase()));

    let total_count: i64 = sqlx::query_scalar(
        "select count(*)
           from products p
          where p.tenant_id = $1
            and ($2::boolean = true or p.archived_at is null)
            and ($2::boolean = false or p.archived_at is not null)
            and ($3::text is null or lower(p.name) like $3 or lower(coalesce(p.sku, '')) like $3 or lower(coalesce(p.barcode, '')) like $3)
            and ($4::text is null or p.usage = $4)
            and ($5::uuid is null or exists(select 1 from branch_products bp where bp.product_id = p.id and bp.category_id = $5))",
    )
    .bind(scope.tenant)
    .bind(archived_filter)
    .bind(&q_pattern)
    .bind(&params.usage)
    .bind(params.category_id)
    .fetch_one(&mut *tx)
    .await?;

    let items: Vec<ProductRow> = sqlx::query_as(
        "select
           p.id,
           p.name,
           p.sku,
           p.barcode,
           p.kind,
           p.usage,
           coalesce(bp.price, p.base_price) as price,
           p.cost_price,
           coalesce(tx.tima_codigo, 'EXENTO') as tax_code,
           coalesce((select sum(s.on_hand) from stock s where s.product_id = p.id and ($2::uuid is null or s.branch_id = $2)), 0) as on_hand,
           c.name as category_name,
           p.has_variants,
           p.parent_id,
           (p.archived_at is not null) as archived
         from products p
         left join branch_products bp on bp.product_id = p.id and ($2::uuid is null or bp.branch_id = $2)
         left join product_categories c on c.id = bp.category_id
         left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
        where p.tenant_id = $1
          and ($3::boolean = true or p.archived_at is null)
          and ($3::boolean = false or p.archived_at is not null)
          and ($4::text is null or lower(p.name) like $4 or lower(coalesce(p.sku, '')) like $4 or lower(coalesce(p.barcode, '')) like $4)
          and ($5::text is null or p.usage = $5)
          and ($6::uuid is null or bp.category_id = $6)
        order by p.name
        limit $7 offset $8",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(archived_filter)
    .bind(&q_pattern)
    .bind(&params.usage)
    .bind(params.category_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;

    Ok(ProductListResponse { items, total_count })
}

pub async fn get_product_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    branch_id: Option<Uuid>,
) -> Result<ProductDetail, ApiError> {
    let row: ProductDetailDbRow = sqlx::query_as(
        "select
           p.id,
           p.name,
           p.sku,
           p.barcode,
           p.kind,
           p.usage,
           coalesce(bp.price, p.base_price) as price,
           p.cost_price,
           tx.tima_codigo as tax_code,
           coalesce((select sum(s.on_hand) from stock s where s.product_id = p.id and ($3::uuid is null or s.branch_id = $3)), 0) as on_hand,
           c.name as category_name,
           p.has_variants,
           p.parent_id,
           (p.archived_at is not null) as archived,
           p.archived_at,
           uom.tima_codigo as unit_measure_code,
           puom.tima_codigo as purchase_uom_code,
           p.purchase_factor,
           p.reorder_qty,
           p.max_qty,
           p.track_lots,
           p.allow_price_override,
           p.description
         from products p
         left join branch_products bp on bp.product_id = p.id and ($3::uuid is null or bp.branch_id = $3)
         left join product_categories c on c.id = bp.category_id
         left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
         join tbl_tipos_maestra uom on uom.tima_id = p.unit_measure_id
         left join tbl_tipos_maestra puom on puom.tima_id = p.purchase_uom_id
        where p.tenant_id = $1 and p.id = $2",
    )
    .bind(scope.tenant)
    .bind(id)
    .bind(branch_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(crate::error::DomainError::NotFound))?;

    let variants: Vec<ProductRow> = sqlx::query_as(
        "select
           p.id,
           p.name,
           p.sku,
           p.barcode,
           p.kind,
           p.usage,
           coalesce(bp.price, p.base_price) as price,
           p.cost_price,
           coalesce(tx.tima_codigo, 'EXENTO') as tax_code,
           coalesce((select sum(s.on_hand) from stock s where s.product_id = p.id and ($2::uuid is null or s.branch_id = $2)), 0) as on_hand,
           c.name as category_name,
           p.has_variants,
           p.parent_id,
           (p.archived_at is not null) as archived
         from products p
         left join branch_products bp on bp.product_id = p.id and ($2::uuid is null or bp.branch_id = $2)
         left join product_categories c on c.id = bp.category_id
         left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
        where p.tenant_id = $1 and p.parent_id = $3
        order by p.name",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;

    // Consultar tier_prices
    let tier_prices = crate::tiers::get_product_tier_prices_tx(tx, scope, id).await?;

    // Consultar option_groups
    let option_groups = crate::options::get_product_option_groups_tx(tx, scope, id).await?;
    let option_groups_json: Vec<serde_json::Value> = option_groups
        .into_iter()
        .map(|og| serde_json::to_value(og).unwrap_or_default())
        .collect();

    // Consultar stock_by_location (o stock de la sucursal si no hay locations aún)
    let stock_by_location: Vec<LocationStockRow> = sqlx::query_as(
        "select s.branch_id as location_id,
                coalesce(b.name, 'Principal') as location_name,
                s.on_hand,
                s.reserved,
                null::numeric as min_qty,
                0::numeric as avg_cost
           from stock s
           join branches b on b.id = s.branch_id
          where s.tenant_id = $1 and s.product_id = $2 and ($3::uuid is null or s.branch_id = $3)",
    )
    .bind(scope.tenant)
    .bind(id)
    .bind(branch_id)
    .fetch_all(&mut **tx)
    .await
    .unwrap_or_default();

    Ok(ProductDetail {
        row: ProductRow {
            id: row.id,
            name: row.name,
            sku: row.sku,
            barcode: row.barcode,
            kind: row.kind,
            usage: row.usage,
            price: row.price,
            cost_price: row.cost_price,
            tax_code: row.tax_code.unwrap_or_else(|| "EXENTO".to_string()),
            on_hand: row.on_hand,
            category_name: row.category_name,
            has_variants: row.has_variants,
            parent_id: row.parent_id,
            archived: row.archived,
        },
        archived_at: row.archived_at,
        unit_measure_code: row.unit_measure_code,
        purchase_uom_code: row.purchase_uom_code,
        purchase_factor: row.purchase_factor,
        reorder_qty: row.reorder_qty,
        max_qty: row.max_qty,
        track_lots: row.track_lots,
        allow_price_override: row.allow_price_override,
        description: row.description,
        variants,
        tier_prices,
        option_groups: option_groups_json,
        stock_by_location,
    })
}

pub async fn get_product(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    branch_id: Option<Uuid>,
) -> Result<ProductDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:read")?;
    let detail = get_product_detail_tx(&mut tx, &scope, id, branch_id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_product_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    input: ProductInput,
) -> Result<Uuid, ApiError> {
    validate_product_input(&input)?;
    let name = input.name.trim().to_string();
    let sku = input
        .sku
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let barcode = input
        .barcode
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(s) = sku {
        check_sku_unique(tx, scope.tenant, s, None).await?;
    }
    if let Some(b) = barcode {
        check_barcode_unique(tx, scope.tenant, b, None).await?;
    }

    let uom_id = resolve_uom_id(tx, &input.unit_measure_code).await?;
    let puom_id = match &input.purchase_uom_code {
        Some(code) => Some(resolve_uom_id(tx, code).await?),
        None => None,
    };
    let tax_id = resolve_tax_type_id(tx, &input.tax_code).await?;

    let id: Uuid = sqlx::query_scalar(
        "insert into products (
           tenant_id, name, sku, barcode, kind, usage, unit_measure_id,
           purchase_uom_id, purchase_factor, tax_type_id, cost_price, base_price,
           reorder_qty, max_qty, track_lots, allow_price_override, has_variants,
           description, active
         ) values (
           $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, true
         ) returning id",
    )
    .bind(scope.tenant)
    .bind(&name)
    .bind(sku)
    .bind(barcode)
    .bind(&input.kind)
    .bind(&input.usage)
    .bind(uom_id)
    .bind(puom_id)
    .bind(input.purchase_factor)
    .bind(tax_id)
    .bind(input.cost_price)
    .bind(input.base_price)
    .bind(input.reorder_qty)
    .bind(input.max_qty)
    .bind(input.track_lots)
    .bind(input.allow_price_override)
    .bind(input.has_variants)
    .bind(&input.description)
    .fetch_one(&mut **tx)
    .await?;

    // Crear branch_products si el scope tiene branch o para todas las sucursales del negocio
    let branches: Vec<Uuid> = sqlx::query_scalar("select id from branches where tenant_id = $1")
        .bind(scope.tenant)
        .fetch_all(&mut **tx)
        .await?;

    for bid in &branches {
        sqlx::query(
            "insert into branch_products (tenant_id, branch_id, product_id, price, is_available)
             values ($1, $2, $3, $4, true)
             on conflict (branch_id, product_id) do nothing",
        )
        .bind(scope.tenant)
        .bind(bid)
        .bind(id)
        .bind(input.base_price)
        .execute(&mut **tx)
        .await?;
    }

    crate::audit::record_tx(
        tx,
        scope,
        "PRODUCTO_CREADO",
        "products",
        id,
        None,
        Some(serde_json::to_value(&input).unwrap_or_default()),
        None,
    )
    .await?;

    Ok(id)
}

pub async fn create_product(
    pool: &PgPool,
    claims: &Claims,
    input: ProductInput,
) -> Result<ProductDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let id = create_product_tx(&mut tx, &scope, input).await?;
    let detail = get_product_detail_tx(&mut tx, &scope, id, scope.branch).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn update_product_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    input: ProductInput,
) -> Result<(), ApiError> {
    validate_product_input(&input)?;

    let name = input.name.trim().to_string();
    let sku = input
        .sku
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let barcode = input
        .barcode
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(s) = sku {
        check_sku_unique(tx, scope.tenant, s, Some(id)).await?;
    }
    if let Some(b) = barcode {
        check_barcode_unique(tx, scope.tenant, b, Some(id)).await?;
    }

    let existing_uom_id: i32 =
        sqlx::query_scalar("select unit_measure_id from products where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(ApiError::Domain(crate::error::DomainError::NotFound))?;

    let uom_id = resolve_uom_id(tx, &input.unit_measure_code).await?;
    if uom_id != existing_uom_id {
        let has_movements: bool = sqlx::query_scalar(
            "select exists(select 1 from inventory_movements where tenant_id = $1 and product_id = $2)",
        )
        .bind(scope.tenant)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;

        if has_movements {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "PRODUCT_UOM_LOCKED",
                "No se puede cambiar la unidad de medida de un producto con movimientos de inventario",
                None,
            ));
        }
    }

    if input.has_variants {
        let has_pres: bool = sqlx::query_scalar(
            "select exists(
                select 1 from product_tier_prices ptp
                join price_tiers pt on pt.id = ptp.tier_id
                where ptp.tenant_id = $1 and ptp.product_id = $2 and pt.kind = 'PRESENTACION'
            )",
        )
        .bind(scope.tenant)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;

        if has_pres {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "VARIANTS_AND_PRESENTATIONS",
                "Un producto con presentaciones no puede tener variantes",
                None,
            ));
        }
    }

    let puom_id = match &input.purchase_uom_code {
        Some(code) => Some(resolve_uom_id(tx, code).await?),
        None => None,
    };
    let tax_id = resolve_tax_type_id(tx, &input.tax_code).await?;

    let before_detail = get_product_detail_tx(tx, scope, id, scope.branch).await?;

    sqlx::query(
        "update products set
           name = $3,
           sku = $4,
           barcode = $5,
           kind = $6,
           usage = $7,
           unit_measure_id = $8,
           purchase_uom_id = $9,
           purchase_factor = $10,
           tax_type_id = $11,
           cost_price = $12,
           base_price = $13,
           reorder_qty = $14,
           max_qty = $15,
           track_lots = $16,
           allow_price_override = $17,
           has_variants = $18,
           description = $19
         where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(id)
    .bind(&name)
    .bind(sku)
    .bind(barcode)
    .bind(&input.kind)
    .bind(&input.usage)
    .bind(uom_id)
    .bind(puom_id)
    .bind(input.purchase_factor)
    .bind(tax_id)
    .bind(input.cost_price)
    .bind(input.base_price)
    .bind(input.reorder_qty)
    .bind(input.max_qty)
    .bind(input.track_lots)
    .bind(input.allow_price_override)
    .bind(input.has_variants)
    .bind(&input.description)
    .execute(&mut **tx)
    .await?;

    // Copiar campos heredados y recalcular nombres de variantes hijas si es un padre
    let children: Vec<(Uuid, Option<serde_json::Value>)> = sqlx::query_as(
        "select id, variant_attrs from products where tenant_id = $1 and parent_id = $2",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;

    for (child_id, attrs_opt) in children {
        let child_name = if let Some(serde_json::Value::Object(map)) = attrs_opt {
            let values: Vec<String> = map
                .values()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            format!("{} - {}", name, values.join(" / "))
        } else {
            name.clone()
        };

        sqlx::query(
            "update products set
               name = $3,
               kind = $4,
               usage = $5,
               unit_measure_id = $6,
               purchase_uom_id = $7,
               purchase_factor = $8,
               tax_type_id = $9,
               track_lots = $10
             where tenant_id = $1 and id = $2",
        )
        .bind(scope.tenant)
        .bind(child_id)
        .bind(&child_name)
        .bind(&input.kind)
        .bind(&input.usage)
        .bind(uom_id)
        .bind(puom_id)
        .bind(input.purchase_factor)
        .bind(tax_id)
        .bind(input.track_lots)
        .execute(&mut **tx)
        .await?;
    }

    crate::audit::record_tx(
        tx,
        scope,
        "PRODUCTO_EDITADO",
        "products",
        id,
        Some(serde_json::to_value(&before_detail).unwrap_or_default()),
        Some(serde_json::to_value(&input).unwrap_or_default()),
        None,
    )
    .await?;

    Ok(())
}

pub async fn update_product(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    input: ProductInput,
) -> Result<ProductDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    update_product_tx(&mut tx, &scope, id, input).await?;
    let detail = get_product_detail_tx(&mut tx, &scope, id, scope.branch).await?;
    tx.commit().await?;
    Ok(detail)
}

#[derive(Debug, sqlx::FromRow)]
struct ParentProductRow {
    name: String,
    has_variants: bool,
    kind: String,
    usage: String,
    unit_measure_id: i32,
    purchase_uom_id: Option<i32>,
    purchase_factor: Decimal,
    tax_type_id: Option<i32>,
    track_lots: bool,
}

pub async fn create_variant_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    parent_id: Uuid,
    input: CreateVariantInput,
) -> Result<Uuid, ApiError> {
    let p: ParentProductRow = sqlx::query_as(
        "select name, has_variants, kind, usage, unit_measure_id, purchase_uom_id, purchase_factor, tax_type_id, track_lots
           from products where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(parent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(crate::error::DomainError::NotFound))?;

    if !p.has_variants {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "El producto padre no tiene habilitadas las variantes",
            None,
        ));
    }

    // Verificar que el padre no tenga presentaciones
    let has_pres: bool = sqlx::query_scalar(
        "select exists(
            select 1 from product_tier_prices ptp
            join price_tiers pt on pt.id = ptp.tier_id
            where ptp.tenant_id = $1 and ptp.product_id = $2 and pt.kind = 'PRESENTACION'
        )",
    )
    .bind(scope.tenant)
    .bind(parent_id)
    .fetch_one(&mut **tx)
    .await?;

    if has_pres {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VARIANTS_AND_PRESENTATIONS",
            "No se pueden crear variantes para un producto con presentaciones",
            None,
        ));
    }

    // Validar attrs
    if input.attrs.is_empty() || input.attrs.len() > 3 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "Los atributos de variante deben tener entre 1 y 3 claves",
            None,
        ));
    }
    for (k, v) in &input.attrs {
        let k_trim = k.trim();
        let v_trim = v.trim();
        if k_trim.is_empty()
            || k_trim.chars().count() > 20
            || v_trim.is_empty()
            || v_trim.chars().count() > 30
        {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "Las claves de atributos deben tener 1-20 caracteres y los valores 1-30 caracteres",
                None,
            ));
        }
    }

    let sku = input
        .sku
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let barcode = input
        .barcode
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(s) = sku {
        if s.chars().count() > 40
            || !s
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "El SKU debe tener entre 1 y 40 caracteres válidos",
                None,
            ));
        }
        check_sku_unique(tx, scope.tenant, s, None).await?;
    }

    if let Some(b) = barcode {
        if b.contains("E+") || b.contains("e+") {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "BARCODE_SCIENTIFIC",
                "El código de barras contiene notación científica",
                None,
            ));
        }
        if b.chars().count() < 6 || b.chars().count() > 14 || !b.chars().all(|c| c.is_ascii_digit())
        {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "El código de barras debe tener entre 6 y 14 dígitos",
                None,
            ));
        }
        check_barcode_unique(tx, scope.tenant, b, None).await?;
    }

    if input.price < Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_INVALID",
            "El precio de la variante no puede ser negativo",
            None,
        ));
    }
    if let Some(cp) = input.cost_price {
        if cp < Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_INVALID",
                "El costo de la variante no puede ser negativo",
                None,
            ));
        }
    }

    let values: Vec<&str> = input.attrs.values().map(|v| v.as_str()).collect();
    let variant_name = format!("{} - {}", p.name, values.join(" / "));
    let attrs_json = serde_json::to_value(&input.attrs).unwrap_or_default();

    let id: Uuid = sqlx::query_scalar(
        "insert into products (
           tenant_id, name, sku, barcode, kind, usage, unit_measure_id,
           purchase_uom_id, purchase_factor, tax_type_id, cost_price, base_price,
           track_lots, parent_id, has_variants, variant_attrs, active
         ) values (
           $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, false, $15, true
         ) returning id",
    )
    .bind(scope.tenant)
    .bind(&variant_name)
    .bind(sku)
    .bind(barcode)
    .bind(&p.kind)
    .bind(&p.usage)
    .bind(p.unit_measure_id)
    .bind(p.purchase_uom_id)
    .bind(p.purchase_factor)
    .bind(p.tax_type_id)
    .bind(input.cost_price)
    .bind(input.price)
    .bind(p.track_lots)
    .bind(parent_id)
    .bind(attrs_json)
    .fetch_one(&mut **tx)
    .await?;

    let branches: Vec<Uuid> = sqlx::query_scalar("select id from branches where tenant_id = $1")
        .bind(scope.tenant)
        .fetch_all(&mut **tx)
        .await?;

    for bid in &branches {
        sqlx::query(
            "insert into branch_products (tenant_id, branch_id, product_id, price, is_available)
             values ($1, $2, $3, $4, true)
             on conflict (branch_id, product_id) do nothing",
        )
        .bind(scope.tenant)
        .bind(bid)
        .bind(id)
        .bind(input.price)
        .execute(&mut **tx)
        .await?;
    }

    crate::audit::record_tx(
        tx,
        scope,
        "PRODUCTO_CREADO",
        "products",
        id,
        None,
        Some(serde_json::to_value(&variant_name).unwrap_or_default()),
        None,
    )
    .await?;

    Ok(id)
}

pub async fn create_variant(
    pool: &PgPool,
    claims: &Claims,
    parent_id: Uuid,
    input: CreateVariantInput,
) -> Result<ProductDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let id = create_variant_tx(&mut tx, &scope, parent_id, input).await?;
    let detail = get_product_detail_tx(&mut tx, &scope, id, scope.branch).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn update_branch_product(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
    input: UpdateBranchProductInput,
) -> Result<serde_json::Value, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let before_price: Option<Option<Decimal>> = sqlx::query_scalar(
        "select price from branch_products where tenant_id = $1 and branch_id = $2 and product_id = $3",
    )
    .bind(scope.tenant)
    .bind(input.branch_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;

    let bp_id: Uuid = sqlx::query_scalar(
        "insert into branch_products (tenant_id, branch_id, product_id, price, category_id, is_available)
         values ($1, $2, $3, $4, $5, $6)
         on conflict (branch_id, product_id) do update set
           price = excluded.price,
           category_id = excluded.category_id,
           is_available = excluded.is_available
         returning id",
    )
    .bind(scope.tenant)
    .bind(input.branch_id)
    .bind(product_id)
    .bind(input.price)
    .bind(input.category_id)
    .bind(input.is_available)
    .fetch_one(&mut *tx)
    .await?;

    if before_price != Some(Some(input.price)) {
        crate::audit::record_tx(
            &mut tx,
            &scope,
            "PRECIO_EDITADO",
            "branch_products",
            bp_id,
            Some(serde_json::json!({ "price": before_price.flatten() })),
            Some(serde_json::json!({ "price": input.price })),
            None,
        )
        .await?;
    }

    tx.commit().await?;
    Ok(serde_json::json!({ "status": "ok", "id": bp_id }))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchiveProductInput {
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivePlanItem {
    pub location_id: Uuid,
    pub location_name: String,
    pub on_hand: Decimal,
    pub avg_cost: Decimal,
    pub value: Decimal,
}

pub async fn archive_product(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    confirm: bool,
) -> Result<serde_json::Value, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let prod_row: Option<(Uuid, bool)> = sqlx::query_as(
        "select id, has_variants
           from products
          where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let (prod_id, has_variants) = prod_row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let mut product_ids = vec![prod_id];
    if has_variants {
        let variant_ids: Vec<Uuid> = sqlx::query_scalar(
            "select id from products where tenant_id = $1 and parent_id = $2 and archived_at is null",
        )
        .bind(scope.tenant)
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        product_ids.extend(variant_ids);
    }

    // 1. Si el producto tiene reservas ACTIVA en cualquier ubicación: 409 PRODUCT_HAS_RESERVATIONS
    let has_res: bool = sqlx::query_scalar(
        "select exists (
           select 1 from stock_reservations
            where tenant_id = $1
              and product_id = any($2)
              and status_id = public.tipo_id('RES', 'ACTIVA')
         )",
    )
    .bind(scope.tenant)
    .bind(&product_ids)
    .fetch_one(&mut *tx)
    .await?;

    if has_res {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "PRODUCT_HAS_RESERVATIONS",
            "El producto o sus variantes tienen reservas activas",
            None,
        ));
    }

    // 2. Calcula el plan: [{location_id, location_name, on_hand, avg_cost, value = money(on_hand × avg_cost)}], solo de las ubicaciones con on_hand != 0.
    let stock_rows: Vec<(Uuid, String, Uuid, Decimal, Decimal, Option<Decimal>)> = sqlx::query_as(
        "select s.branch_id, b.name, s.product_id, s.on_hand, s.reserved, coalesce(p.cost_price, 0)
           from stock s
           join branches b on b.id = s.branch_id
           join products p on p.id = s.product_id
          where s.tenant_id = $1
            and s.product_id = any($2)
            and s.on_hand != 0",
    )
    .bind(scope.tenant)
    .bind(&product_ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut plan = Vec::new();
    let mut total_value = Decimal::ZERO;

    for (branch_id, branch_name, _pid, on_hand, _reserved, cost_price) in &stock_rows {
        let avg_cost = cost_price.unwrap_or(Decimal::ZERO);
        let value = crate::money::money(*on_hand * avg_cost);
        total_value += value;
        plan.push(ArchivePlanItem {
            location_id: *branch_id,
            location_name: branch_name.clone(),
            on_hand: *on_hand,
            avg_cost,
            value,
        });
    }

    // 3. Si el plan no está vacío y confirm = false: 409 ARCHIVE_NEEDS_CONFIRM, con data: {plan, total_value}.
    if !plan.is_empty() && !confirm {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "ARCHIVE_NEEDS_CONFIRM",
            "El producto tiene existencias que deben darse de baja para archivar",
            Some(serde_json::json!({
                "plan": plan,
                "total_value": total_value,
            })),
        ));
    }

    // 4. Con confirm = true (o con el plan vacío), en la misma transacción:
    // 4.1 por cada fila del plan, registra un ajuste de salida con causa "Baja por archivo" mediante apply_delta_tx,
    // con movement = AJUSTE_SALIDA, ref_type = 'ARCHIVE' y ref_id = product_id
    for (branch_id, _bname, pid, on_hand, reserved, _cost) in stock_rows {
        if on_hand > Decimal::ZERO {
            let delta = -on_hand;
            crate::inventory::apply_delta_ref_tx(
                &mut tx,
                &scope,
                branch_id,
                pid,
                on_hand,
                reserved,
                delta,
                "Baja por archivo",
                Some(("ARCHIVE", pid)),
            )
            .await?;
        }
    }

    // 4.2 archived_at = now() y branch_products.is_available = false en todas las sucursales;
    // 4.3 si es un padre, archiva también todas sus variantes con el mismo proceso;
    sqlx::query(
        "update products
            set archived_at = now()
          where tenant_id = $1 and id = any($2)",
    )
    .bind(scope.tenant)
    .bind(&product_ids)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "update branch_products
            set is_available = false
          where product_id = any($1)",
    )
    .bind(&product_ids)
    .execute(&mut *tx)
    .await?;

    // 4.4 auditoría PRODUCTO_ARCHIVADO, con el plan en after.
    crate::audit::record_tx(
        &mut tx,
        &scope,
        "PRODUCTO_ARCHIVADO",
        "products",
        id,
        None,
        Some(serde_json::json!({
            "plan": plan,
            "total_value": total_value,
        })),
        None,
    )
    .await?;

    tx.commit().await?;

    Ok(serde_json::json!({
        "status": "ok",
        "archived_at": chrono::Utc::now(),
        "archived_ids": product_ids,
    }))
}

pub async fn unarchive_product(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
) -> Result<serde_json::Value, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let prod_row: Option<(Uuid, bool)> =
        sqlx::query_as("select id, has_variants from products where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

    let (prod_id, has_variants) = prod_row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let mut product_ids = vec![prod_id];
    if has_variants {
        let variant_ids: Vec<Uuid> =
            sqlx::query_scalar("select id from products where tenant_id = $1 and parent_id = $2")
                .bind(scope.tenant)
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;
        product_ids.extend(variant_ids);
    }

    sqlx::query(
        "update products
            set archived_at = null
          where tenant_id = $1 and id = any($2)",
    )
    .bind(scope.tenant)
    .bind(&product_ids)
    .execute(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "PRODUCTO_DESARCHIVADO",
        "products",
        id,
        None,
        None,
        None,
    )
    .await?;

    tx.commit().await?;

    Ok(serde_json::json!({
        "status": "ok",
        "unarchived_ids": product_ids,
    }))
}

pub fn ean13_check(digits: &str) -> u8 {
    let mut sum = 0u32;
    for (i, ch) in digits.chars().take(12).enumerate() {
        let d = ch.to_digit(10).unwrap_or(0);
        let weight = if i % 2 == 1 { 3 } else { 1 };
        sum += d * weight;
    }
    ((10 - (sum % 10)) % 10) as u8
}

pub async fn generate_barcode(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
) -> Result<String, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let existing: Option<Option<String>> =
        sqlx::query_scalar("select barcode from products where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?;

    let existing_barcode = existing.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if let Some(bc) = existing_barcode {
        if !bc.trim().is_empty() {
            return Err(error::state("BARCODE_EXISTS"));
        }
    }

    let barcode = loop {
        let next_val: i64 = sqlx::query_scalar(
            "insert into tenant_counters (tenant_id, name, next_value)
             values ($1, 'EAN_INTERNO', 2)
             on conflict (tenant_id, name)
             do update set next_value = tenant_counters.next_value + 1
             returning tenant_counters.next_value - 1",
        )
        .bind(scope.tenant)
        .fetch_one(&mut *tx)
        .await?;

        let base12 = format!("20{:010}", next_val);
        let check = ean13_check(&base12);
        let candidate = format!("{}{}", base12, check);

        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from products where tenant_id = $1 and barcode = $2)",
        )
        .bind(scope.tenant)
        .bind(&candidate)
        .fetch_one(&mut *tx)
        .await?;

        if !exists {
            break candidate;
        }
    };

    sqlx::query("update products set barcode = $1 where tenant_id = $2 and id = $3")
        .bind(&barcode)
        .bind(scope.tenant)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "PRODUCTO_CODIGO_GENERADO",
        "products",
        product_id,
        None,
        Some(serde_json::json!({ "barcode": barcode })),
        None,
    )
    .await?;

    tx.commit().await?;

    Ok(barcode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ean13_check_digit() {
        assert_eq!(ean13_check("200000000001"), 5);
    }
}

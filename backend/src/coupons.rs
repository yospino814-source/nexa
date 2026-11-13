use std::collections::HashSet;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::pricing::{CouponError, CouponRule, PromoKind, PromoScope};
use crate::tenancy;

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouponDto {
    pub id: Uuid,
    pub branch_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub value: Decimal,
    pub scope: String,
    pub min_qty: Option<Decimal>,
    pub min_subtotal: Option<Decimal>,
    pub max_discount: Option<Decimal>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub status: String,
    pub usage_limit: Option<i32>,
    pub usage_count: i32,
    pub per_customer_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub product_ids: Vec<Uuid>,
    pub category_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCouponInput {
    pub branch_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub value: Decimal,
    pub scope: String,
    pub min_qty: Option<Decimal>,
    pub min_subtotal: Option<Decimal>,
    pub max_discount: Option<Decimal>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub product_ids: Option<Vec<Uuid>>,
    pub category_ids: Option<Vec<Uuid>>,
}

fn validate_coupon_input(input: &CreateCouponInput) -> Result<String, ApiError> {
    let code = input.code.trim().to_uppercase();
    if code.len() < 4
        || code.len() > 20
        || !code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "El código del cupón debe tener entre 4 y 20 caracteres alfanuméricos en mayúsculas",
            None,
        ));
    }

    let name = input.name.trim();
    if name.is_empty() || name.len() > 60 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "El nombre debe tener entre 1 y 60 caracteres",
            None,
        ));
    }

    if input.kind != "PORCENTAJE" && input.kind != "FIJO" {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "El tipo de cupón debe ser PORCENTAJE o FIJO",
            None,
        ));
    }

    if input.value <= Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "El valor del cupón debe ser mayor a 0",
            None,
        ));
    }

    if input.kind == "PORCENTAJE" && input.value > Decimal::from(100) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "El porcentaje de descuento no puede superar el 100%",
            None,
        ));
    }

    if input.ends_at.is_some_and(|ends| ends <= input.starts_at) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "COUPON_INVALID",
            "La fecha de fin debe ser posterior a la de inicio",
            None,
        ));
    }

    let p_ids = input.product_ids.as_deref().unwrap_or(&[]);
    let c_ids = input.category_ids.as_deref().unwrap_or(&[]);

    match input.scope.as_str() {
        "PEDIDO" => {
            if !p_ids.is_empty() || !c_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Un cupón de pedido no puede tener productos ni categorías objetivo",
                    None,
                ));
            }
            if input.min_qty.is_some() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Un cupón de pedido no admite cantidad mínima de unidades",
                    None,
                ));
            }
        }
        "PRODUCTO" => {
            if p_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Debe seleccionar al menos un producto para el cupón",
                    None,
                ));
            }
            if !c_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Un cupón de producto no puede incluir categorías",
                    None,
                ));
            }
        }
        "CATEGORIA" => {
            if c_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Debe seleccionar al menos una categoría para el cupón",
                    None,
                ));
            }
            if !p_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "COUPON_INVALID",
                    "Un cupón de categoría no puede incluir productos específicos",
                    None,
                ));
            }
        }
        _ => {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "COUPON_INVALID",
                "Alcance de cupón desconocido",
                None,
            ));
        }
    }

    Ok(code)
}

pub async fn list(pool: &PgPool, claims: &Claims) -> Result<Vec<CouponDto>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;
    let tenant = scope.tenant;

    let rows = sqlx::query(
        "select id, branch_id, code, name, kind, value, scope, min_qty, min_subtotal, max_discount,
                starts_at, ends_at, status, usage_limit, usage_count, per_customer_limit, created_at
         from coupons
         where tenant_id = $1
         order by created_at desc",
    )
    .bind(tenant)
    .fetch_all(&mut *tx)
    .await?;

    let mut dtos = Vec::with_capacity(rows.len());

    for row in rows {
        let id: Uuid = row.get("id");

        let target_rows =
            sqlx::query("select product_id, category_id from coupon_targets where coupon_id = $1")
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;

        let mut product_ids = Vec::new();
        let mut category_ids = Vec::new();
        for tr in target_rows {
            let pid: Option<Uuid> = tr.get("product_id");
            let cid: Option<Uuid> = tr.get("category_id");
            if let Some(p) = pid {
                product_ids.push(p);
            }
            if let Some(c) = cid {
                category_ids.push(c);
            }
        }

        dtos.push(CouponDto {
            id,
            branch_id: row.get("branch_id"),
            code: row.get("code"),
            name: row.get("name"),
            kind: row.get("kind"),
            value: row.get("value"),
            scope: row.get("scope"),
            min_qty: row.get("min_qty"),
            min_subtotal: row.get("min_subtotal"),
            max_discount: row.get("max_discount"),
            starts_at: row.get("starts_at"),
            ends_at: row.get("ends_at"),
            status: row.get("status"),
            usage_limit: row.get("usage_limit"),
            usage_count: row.get("usage_count"),
            per_customer_limit: row.get("per_customer_limit"),
            created_at: row.get("created_at"),
            product_ids,
            category_ids,
        });
    }

    tx.commit().await?;
    Ok(dtos)
}

pub async fn create(
    pool: &PgPool,
    claims: &Claims,
    input: CreateCouponInput,
) -> Result<CouponDto, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;
    let code = validate_coupon_input(&input)?;

    let tenant = scope.tenant;
    let name = input.name.trim();
    let now = Utc::now();

    let existing: Option<Uuid> =
        sqlx::query_scalar("select id from coupons where tenant_id = $1 and code = $2")
            .bind(tenant)
            .bind(&code)
            .fetch_optional(&mut *tx)
            .await?;

    if existing.is_some() {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "COUPON_CODE_TAKEN",
            "Ya existe un cupón con ese código",
            None,
        ));
    }

    let id: Uuid = sqlx::query_scalar(
        "insert into coupons (
            tenant_id, branch_id, code, name, kind, value, scope, min_qty, min_subtotal, max_discount,
            starts_at, ends_at, status, usage_limit, per_customer_limit
         )
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'ACTIVO', $13, $14)
         returning id",
    )
    .bind(tenant)
    .bind(input.branch_id)
    .bind(&code)
    .bind(name)
    .bind(&input.kind)
    .bind(input.value)
    .bind(&input.scope)
    .bind(input.min_qty)
    .bind(input.min_subtotal)
    .bind(input.max_discount)
    .bind(input.starts_at)
    .bind(input.ends_at)
    .bind(input.usage_limit)
    .bind(input.per_customer_limit)
    .fetch_one(&mut *tx)
    .await?;

    let p_ids = input.product_ids.unwrap_or_default();
    let c_ids = input.category_ids.unwrap_or_default();

    for pid in &p_ids {
        sqlx::query(
            "insert into coupon_targets (tenant_id, coupon_id, product_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }

    for cid in &c_ids {
        sqlx::query(
            "insert into coupon_targets (tenant_id, coupon_id, category_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(CouponDto {
        id,
        branch_id: input.branch_id,
        code,
        name: name.to_string(),
        kind: input.kind,
        value: input.value,
        scope: input.scope,
        min_qty: input.min_qty,
        min_subtotal: input.min_subtotal,
        max_discount: input.max_discount,
        starts_at: input.starts_at,
        ends_at: input.ends_at,
        status: "ACTIVO".to_string(),
        usage_limit: input.usage_limit,
        usage_count: 0,
        per_customer_limit: input.per_customer_limit,
        created_at: now,
        product_ids: p_ids,
        category_ids: c_ids,
    })
}

pub async fn update(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    input: CreateCouponInput,
) -> Result<CouponDto, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;
    let code = validate_coupon_input(&input)?;

    let tenant = scope.tenant;

    let cur: Option<(String, i32, DateTime<Utc>)> = sqlx::query_as(
        "select status, usage_count, created_at from coupons where id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let (status, usage_count, created_at) = cur.ok_or(DomainError::NotFound)?;

    let dup: Option<Uuid> = sqlx::query_scalar(
        "select id from coupons where tenant_id = $1 and code = $2 and id != $3",
    )
    .bind(tenant)
    .bind(&code)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    if dup.is_some() {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "COUPON_CODE_TAKEN",
            "Ya existe un cupón con ese código",
            None,
        ));
    }

    let name = input.name.trim();

    sqlx::query(
        "update coupons
         set branch_id = $3,
             code = $4,
             name = $5,
             kind = $6,
             value = $7,
             scope = $8,
             min_qty = $9,
             min_subtotal = $10,
             max_discount = $11,
             starts_at = $12,
             ends_at = $13,
             usage_limit = $14,
             per_customer_limit = $15
         where id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(tenant)
    .bind(input.branch_id)
    .bind(&code)
    .bind(name)
    .bind(&input.kind)
    .bind(input.value)
    .bind(&input.scope)
    .bind(input.min_qty)
    .bind(input.min_subtotal)
    .bind(input.max_discount)
    .bind(input.starts_at)
    .bind(input.ends_at)
    .bind(input.usage_limit)
    .bind(input.per_customer_limit)
    .execute(&mut *tx)
    .await?;

    sqlx::query("delete from coupon_targets where coupon_id = $1 and tenant_id = $2")
        .bind(id)
        .bind(tenant)
        .execute(&mut *tx)
        .await?;

    let p_ids = input.product_ids.unwrap_or_default();
    let c_ids = input.category_ids.unwrap_or_default();

    for pid in &p_ids {
        sqlx::query(
            "insert into coupon_targets (tenant_id, coupon_id, product_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }

    for cid in &c_ids {
        sqlx::query(
            "insert into coupon_targets (tenant_id, coupon_id, category_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(CouponDto {
        id,
        branch_id: input.branch_id,
        code,
        name: name.to_string(),
        kind: input.kind,
        value: input.value,
        scope: input.scope,
        min_qty: input.min_qty,
        min_subtotal: input.min_subtotal,
        max_discount: input.max_discount,
        starts_at: input.starts_at,
        ends_at: input.ends_at,
        status,
        usage_limit: input.usage_limit,
        usage_count,
        per_customer_limit: input.per_customer_limit,
        created_at,
        product_ids: p_ids,
        category_ids: c_ids,
    })
}

pub async fn toggle(pool: &PgPool, claims: &Claims, id: Uuid) -> Result<CouponDto, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let tenant = scope.tenant;
    let cur_status: Option<String> =
        sqlx::query_scalar("select status from coupons where id = $1 and tenant_id = $2")
            .bind(id)
            .bind(tenant)
            .fetch_optional(&mut *tx)
            .await?;

    let current = cur_status.ok_or(DomainError::NotFound)?;
    let next = if current == "ACTIVO" {
        "INACTIVO"
    } else {
        "ACTIVO"
    };

    sqlx::query("update coupons set status = $3 where id = $1 and tenant_id = $2")
        .bind(id)
        .bind(tenant)
        .bind(next)
        .execute(&mut *tx)
        .await?;

    let row = sqlx::query(
        "select id, branch_id, code, name, kind, value, scope, min_qty, min_subtotal, max_discount,
                starts_at, ends_at, status, usage_limit, usage_count, per_customer_limit, created_at
         from coupons
         where id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(tenant)
    .fetch_one(&mut *tx)
    .await?;

    let target_rows =
        sqlx::query("select product_id, category_id from coupon_targets where coupon_id = $1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    let mut product_ids = Vec::new();
    let mut category_ids = Vec::new();
    for tr in target_rows {
        let pid: Option<Uuid> = tr.get("product_id");
        let cid: Option<Uuid> = tr.get("category_id");
        if let Some(p) = pid {
            product_ids.push(p);
        }
        if let Some(c) = cid {
            category_ids.push(c);
        }
    }

    tx.commit().await?;

    Ok(CouponDto {
        id,
        branch_id: row.get("branch_id"),
        code: row.get("code"),
        name: row.get("name"),
        kind: row.get("kind"),
        value: row.get("value"),
        scope: row.get("scope"),
        min_qty: row.get("min_qty"),
        min_subtotal: row.get("min_subtotal"),
        max_discount: row.get("max_discount"),
        starts_at: row.get("starts_at"),
        ends_at: row.get("ends_at"),
        status: row.get("status"),
        usage_limit: row.get("usage_limit"),
        usage_count: row.get("usage_count"),
        per_customer_limit: row.get("per_customer_limit"),
        created_at: row.get("created_at"),
        product_ids,
        category_ids,
    })
}

/// Valida y resuelve un cupón por código para una venta/cotización.
/// Retorna `CouponError` con el código exacto de la tabla si no es válido.
pub async fn resolve_coupon_tx(
    tx: &mut Tx<'_>,
    tenant: Uuid,
    branch: Uuid,
    customer_id: Option<Uuid>,
    raw_code: &str,
    now: DateTime<Utc>,
) -> Result<CouponRule, CouponError> {
    let code = raw_code.trim().to_uppercase();

    let row = sqlx::query(
        "select id, branch_id, code, name, kind, value, scope, min_qty, min_subtotal, max_discount,
                starts_at, ends_at, status, usage_limit, usage_count, per_customer_limit
         from coupons
         where tenant_id = $1 and code = $2",
    )
    .bind(tenant)
    .bind(&code)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| CouponError {
        code: "COUPON_NOT_FOUND".into(),
        message: format!("Error en base de datos al buscar cupón: {e}"),
    })?;

    // 1. No existe para el negocio: COUPON_NOT_FOUND
    let r = match row {
        Some(row) => row,
        None => {
            return Err(CouponError {
                code: "COUPON_NOT_FOUND".into(),
                message: "El cupón no existe".into(),
            });
        }
    };

    // 2. status = INACTIVO o branch_id distinto: COUPON_INACTIVE
    let status: String = r.get("status");
    let c_branch: Option<Uuid> = r.get("branch_id");
    if status != "ACTIVO" || (c_branch.is_some() && c_branch != Some(branch)) {
        return Err(CouponError {
            code: "COUPON_INACTIVE".into(),
            message: "El cupón no está activo en esta sucursal".into(),
        });
    }

    // 3. now < starts_at: COUPON_NOT_STARTED
    let starts_at: DateTime<Utc> = r.get("starts_at");
    if now < starts_at {
        return Err(CouponError {
            code: "COUPON_NOT_STARTED".into(),
            message: "El cupón aún no inicia su vigencia".into(),
        });
    }

    // 4. ends_at <= now: COUPON_EXPIRED
    let ends_at: Option<DateTime<Utc>> = r.get("ends_at");
    if ends_at.is_some_and(|ends| ends <= now) {
        return Err(CouponError {
            code: "COUPON_EXPIRED".into(),
            message: "El cupón ha vencido".into(),
        });
    }

    // 5. usage_limit alcanzado: COUPON_EXHAUSTED
    let usage_limit: Option<i32> = r.get("usage_limit");
    let usage_count: i32 = r.get("usage_count");
    if usage_limit.is_some_and(|limit| usage_count >= limit) {
        return Err(CouponError {
            code: "COUPON_EXHAUSTED".into(),
            message: "El cupón ya agotó su límite de usos".into(),
        });
    }

    // 6. Tiene per_customer_limit y la venta no tiene cliente: COUPON_REQUIRES_CUSTOMER
    let per_customer_limit: Option<i32> = r.get("per_customer_limit");
    if let Some(pcl) = per_customer_limit {
        let cid = match customer_id {
            Some(id) => id,
            None => {
                return Err(CouponError {
                    code: "COUPON_REQUIRES_CUSTOMER".into(),
                    message: "Este cupón requiere asignar un cliente a la venta".into(),
                });
            }
        };

        // 7. El cliente ya alcanzó per_customer_limit: COUPON_CUSTOMER_LIMIT
        let coupon_id: Uuid = r.get("id");
        let count: i64 = sqlx::query_scalar(
            "select count(*) from coupon_redemptions where coupon_id = $1 and customer_id = $2",
        )
        .bind(coupon_id)
        .bind(cid)
        .fetch_one(&mut **tx)
        .await
        .unwrap_or(0);

        if count >= pcl as i64 {
            return Err(CouponError {
                code: "COUPON_CUSTOMER_LIMIT".into(),
                message: "El cliente ya alcanzó el límite de usos para este cupón".into(),
            });
        }
    }

    let coupon_id: Uuid = r.get("id");
    let target_rows =
        sqlx::query("select product_id, category_id from coupon_targets where coupon_id = $1")
            .bind(coupon_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| CouponError {
                code: "COUPON_NOT_FOUND".into(),
                message: format!("Error al cargar objetivos del cupón: {e}"),
            })?;

    let mut product_ids = HashSet::new();
    let mut category_ids = HashSet::new();
    for tr in target_rows {
        let pid: Option<Uuid> = tr.get("product_id");
        let cid: Option<Uuid> = tr.get("category_id");
        if let Some(p) = pid {
            product_ids.insert(p);
        }
        if let Some(c) = cid {
            category_ids.insert(c);
        }
    }

    let kind_str: String = r.get("kind");
    let kind = match kind_str.as_str() {
        "PORCENTAJE" => PromoKind::Porcentaje,
        _ => PromoKind::Fijo,
    };

    let scope_str: String = r.get("scope");
    let scope = match scope_str.as_str() {
        "PRODUCTO" => PromoScope::Producto,
        "CATEGORIA" => PromoScope::Categoria,
        _ => PromoScope::Pedido,
    };

    Ok(CouponRule {
        id: coupon_id,
        code,
        name: r.get("name"),
        kind,
        value: r.get("value"),
        scope,
        product_ids,
        category_ids,
        min_qty: r.get("min_qty"),
        min_subtotal: r.get("min_subtotal"),
        max_discount: r.get("max_discount"),
    })
}

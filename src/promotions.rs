use std::collections::HashSet;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::pricing::{PromoKind, PromoRule, PromoScope};
use crate::tenancy;

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

pub fn effective_status(
    status: &str,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> &'static str {
    match status {
        "BORRADOR" => "BORRADOR",
        "PAUSADA" => "PAUSADA",
        "ACTIVA" => {
            if now < starts_at {
                "PROGRAMADA"
            } else if let Some(ends) = ends_at {
                if ends <= now { "VENCIDA" } else { "ACTIVA" }
            } else {
                "ACTIVA"
            }
        }
        _ => "BORRADOR",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionDto {
    pub id: Uuid,
    pub branch_id: Option<Uuid>,
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
    pub effective_status: String,
    pub priority: i32,
    pub usage_limit: Option<i32>,
    pub usage_count: i32,
    pub per_customer_limit: Option<i32>,
    pub skip_tier_lines: bool,
    pub created_at: DateTime<Utc>,
    pub product_ids: Vec<Uuid>,
    pub category_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePromotionInput {
    pub branch_id: Option<Uuid>,
    pub name: String,
    pub kind: String,
    pub value: Decimal,
    pub scope: String,
    pub min_qty: Option<Decimal>,
    pub min_subtotal: Option<Decimal>,
    pub max_discount: Option<Decimal>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub priority: Option<i32>,
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub skip_tier_lines: Option<bool>,
    pub product_ids: Option<Vec<Uuid>>,
    pub category_ids: Option<Vec<Uuid>>,
}

fn validate_promotion_input(input: &CreatePromotionInput) -> Result<(), ApiError> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 60 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROMOTION_INVALID",
            "El nombre debe tener entre 1 y 60 caracteres",
            None,
        ));
    }

    if input.kind != "PORCENTAJE" && input.kind != "FIJO" {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROMOTION_INVALID",
            "El tipo de promoción debe ser PORCENTAJE o FIJO",
            None,
        ));
    }

    if input.value <= Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROMOTION_INVALID",
            "El valor del descuento debe ser mayor a 0",
            None,
        ));
    }

    if input.kind == "PORCENTAJE" && input.value > Decimal::from(100) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROMOTION_INVALID",
            "El porcentaje de descuento no puede superar el 100%",
            None,
        ));
    }

    if input.ends_at.is_some_and(|ends| ends <= input.starts_at) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROMOTION_INVALID",
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
                    "PROMOTION_INVALID",
                    "Una promoción de pedido no puede tener productos ni categorías objetivo",
                    None,
                ));
            }
            if input.min_qty.is_some() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PROMOTION_INVALID",
                    "Una promoción de pedido no admite cantidad mínima de unidades",
                    None,
                ));
            }
        }
        "PRODUCTO" => {
            if p_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PROMOTION_INVALID",
                    "Debe seleccionar al menos un producto para la promoción",
                    None,
                ));
            }
            if !c_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PROMOTION_INVALID",
                    "Una promoción de producto no puede incluir categorías",
                    None,
                ));
            }
        }
        "CATEGORIA" => {
            if c_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PROMOTION_INVALID",
                    "Debe seleccionar al menos una categoría para la promoción",
                    None,
                ));
            }
            if !p_ids.is_empty() {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PROMOTION_INVALID",
                    "Una promoción de categoría no puede incluir productos específicos",
                    None,
                ));
            }
        }
        _ => {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PROMOTION_INVALID",
                "Alcance de promoción desconocido",
                None,
            ));
        }
    }

    Ok(())
}

pub async fn list(
    pool: &PgPool,
    claims: &Claims,
    status_filter: Option<String>,
) -> Result<Vec<PromotionDto>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("pos:sell")?;
    let tenant = scope.tenant;
    let now = Utc::now();

    let rows = sqlx::query(
        "select id, branch_id, name, kind, value, scope, min_qty, min_subtotal, max_discount,
                starts_at, ends_at, status, priority, usage_limit, usage_count,
                per_customer_limit, skip_tier_lines, created_at
         from promotions
         where tenant_id = $1
         order by created_at desc",
    )
    .bind(tenant)
    .fetch_all(&mut *tx)
    .await?;

    let mut dtos = Vec::with_capacity(rows.len());

    for row in rows {
        let id: Uuid = row.get("id");
        let status: String = row.get("status");
        let starts_at: DateTime<Utc> = row.get("starts_at");
        let ends_at: Option<DateTime<Utc>> = row.get("ends_at");
        let eff = effective_status(&status, starts_at, ends_at, now);

        if status_filter.as_deref().is_some_and(|filter| eff != filter) {
            continue;
        }

        let target_rows = sqlx::query(
            "select product_id, category_id from promotion_targets where promotion_id = $1",
        )
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

        dtos.push(PromotionDto {
            id,
            branch_id: row.get("branch_id"),
            name: row.get("name"),
            kind: row.get("kind"),
            value: row.get("value"),
            scope: row.get("scope"),
            min_qty: row.get("min_qty"),
            min_subtotal: row.get("min_subtotal"),
            max_discount: row.get("max_discount"),
            starts_at,
            ends_at,
            status,
            effective_status: eff.to_string(),
            priority: row.get("priority"),
            usage_limit: row.get("usage_limit"),
            usage_count: row.get("usage_count"),
            per_customer_limit: row.get("per_customer_limit"),
            skip_tier_lines: row.get("skip_tier_lines"),
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
    input: CreatePromotionInput,
) -> Result<PromotionDto, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;
    validate_promotion_input(&input)?;

    let tenant = scope.tenant;
    let name = input.name.trim();
    let priority = input.priority.unwrap_or(0);
    let skip_tier_lines = input.skip_tier_lines.unwrap_or(true);
    let now = Utc::now();

    let id: Uuid = sqlx::query_scalar(
        "insert into promotions (
            tenant_id, branch_id, name, kind, value, scope, min_qty, min_subtotal, max_discount,
            starts_at, ends_at, status, priority, usage_limit, per_customer_limit, skip_tier_lines
         )
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'BORRADOR', $12, $13, $14, $15)
         returning id",
    )
    .bind(tenant)
    .bind(input.branch_id)
    .bind(name)
    .bind(&input.kind)
    .bind(input.value)
    .bind(&input.scope)
    .bind(input.min_qty)
    .bind(input.min_subtotal)
    .bind(input.max_discount)
    .bind(input.starts_at)
    .bind(input.ends_at)
    .bind(priority)
    .bind(input.usage_limit)
    .bind(input.per_customer_limit)
    .bind(skip_tier_lines)
    .fetch_one(&mut *tx)
    .await?;

    let p_ids = input.product_ids.unwrap_or_default();
    let c_ids = input.category_ids.unwrap_or_default();

    for pid in &p_ids {
        sqlx::query(
            "insert into promotion_targets (tenant_id, promotion_id, product_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }

    for cid in &c_ids {
        sqlx::query(
            "insert into promotion_targets (tenant_id, promotion_id, category_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    }

    let eff = effective_status("BORRADOR", input.starts_at, input.ends_at, now);

    tx.commit().await?;

    Ok(PromotionDto {
        id,
        branch_id: input.branch_id,
        name: name.to_string(),
        kind: input.kind,
        value: input.value,
        scope: input.scope,
        min_qty: input.min_qty,
        min_subtotal: input.min_subtotal,
        max_discount: input.max_discount,
        starts_at: input.starts_at,
        ends_at: input.ends_at,
        status: "BORRADOR".to_string(),
        effective_status: eff.to_string(),
        priority,
        usage_limit: input.usage_limit,
        usage_count: 0,
        per_customer_limit: input.per_customer_limit,
        skip_tier_lines,
        created_at: now,
        product_ids: p_ids,
        category_ids: c_ids,
    })
}

pub async fn update(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    input: CreatePromotionInput,
) -> Result<PromotionDto, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;
    validate_promotion_input(&input)?;

    let tenant = scope.tenant;
    let now = Utc::now();

    let cur: Option<(String, i32, DateTime<Utc>)> = sqlx::query_as(
        "select status, usage_count, created_at from promotions where id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let (status, usage_count, created_at) = cur.ok_or(DomainError::NotFound)?;

    if status != "BORRADOR" && status != "PAUSADA" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "PROMOTION_NOT_EDITABLE",
            "Solo se pueden editar promociones en estado BORRADOR o PAUSADA",
            None,
        ));
    }

    let name = input.name.trim();
    let priority = input.priority.unwrap_or(0);
    let skip_tier_lines = input.skip_tier_lines.unwrap_or(true);

    sqlx::query(
        "update promotions
         set branch_id = $3,
             name = $4,
             kind = $5,
             value = $6,
             scope = $7,
             min_qty = $8,
             min_subtotal = $9,
             max_discount = $10,
             starts_at = $11,
             ends_at = $12,
             priority = $13,
             usage_limit = $14,
             per_customer_limit = $15,
             skip_tier_lines = $16
         where id = $1 and tenant_id = $2",
    )
    .bind(id)
    .bind(tenant)
    .bind(input.branch_id)
    .bind(name)
    .bind(&input.kind)
    .bind(input.value)
    .bind(&input.scope)
    .bind(input.min_qty)
    .bind(input.min_subtotal)
    .bind(input.max_discount)
    .bind(input.starts_at)
    .bind(input.ends_at)
    .bind(priority)
    .bind(input.usage_limit)
    .bind(input.per_customer_limit)
    .bind(skip_tier_lines)
    .execute(&mut *tx)
    .await?;

    sqlx::query("delete from promotion_targets where promotion_id = $1 and tenant_id = $2")
        .bind(id)
        .bind(tenant)
        .execute(&mut *tx)
        .await?;

    let p_ids = input.product_ids.unwrap_or_default();
    let c_ids = input.category_ids.unwrap_or_default();

    for pid in &p_ids {
        sqlx::query(
            "insert into promotion_targets (tenant_id, promotion_id, product_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }

    for cid in &c_ids {
        sqlx::query(
            "insert into promotion_targets (tenant_id, promotion_id, category_id) values ($1, $2, $3)",
        )
        .bind(tenant)
        .bind(id)
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    }

    let eff = effective_status(&status, input.starts_at, input.ends_at, now);

    tx.commit().await?;

    Ok(PromotionDto {
        id,
        branch_id: input.branch_id,
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
        effective_status: eff.to_string(),
        priority,
        usage_limit: input.usage_limit,
        usage_count,
        per_customer_limit: input.per_customer_limit,
        skip_tier_lines,
        created_at,
        product_ids: p_ids,
        category_ids: c_ids,
    })
}

pub async fn activate(pool: &PgPool, claims: &Claims, id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let updated = sqlx::query(
        "update promotions
         set status = 'ACTIVA'
         where id = $1 and tenant_id = $2 and status in ('BORRADOR', 'PAUSADA')",
    )
    .bind(id)
    .bind(scope.tenant)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(error::state(
            "promotion cannot be activated or does not exist",
        ));
    }

    tx.commit().await?;
    Ok(())
}

pub async fn pause(pool: &PgPool, claims: &Claims, id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let updated = sqlx::query(
        "update promotions
         set status = 'PAUSADA'
         where id = $1 and tenant_id = $2 and status = 'ACTIVA'",
    )
    .bind(id)
    .bind(scope.tenant)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(error::state("promotion is not active or does not exist"));
    }

    tx.commit().await?;
    Ok(())
}

/// Carga las promociones aplicables para la cotización o venta en curso.
pub async fn load_active_rules_tx(
    tx: &mut Tx<'_>,
    tenant: Uuid,
    branch: Uuid,
    customer_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<Vec<PromoRule>, ApiError> {
    let rows = sqlx::query(
        "select id, name, kind, value, scope, min_qty, min_subtotal, max_discount,
                priority, starts_at, ends_at, usage_limit, usage_count,
                per_customer_limit, skip_tier_lines, created_at
         from promotions
         where tenant_id = $1
           and (branch_id is null or branch_id = $2)
           and status = 'ACTIVA'
           and starts_at <= $3
           and (ends_at is null or ends_at > $3)
           and (usage_limit is null or usage_count < usage_limit)",
    )
    .bind(tenant)
    .bind(branch)
    .bind(now)
    .fetch_all(&mut **tx)
    .await?;

    let mut rules = Vec::new();

    for r in rows {
        let promo_id: Uuid = r.get("id");
        let per_customer_limit: Option<i32> = r.get("per_customer_limit");

        if let Some(pcl) = per_customer_limit {
            let cid = match customer_id {
                Some(id) => id,
                None => continue, // Venta sin cliente no entra si tiene per_customer_limit
            };
            let count: i64 = sqlx::query_scalar(
                "select count(*) from promotion_redemptions where promotion_id = $1 and customer_id = $2",
            )
            .bind(promo_id)
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;

            if count >= pcl as i64 {
                continue;
            }
        }

        let target_rows = sqlx::query(
            "select product_id, category_id from promotion_targets where promotion_id = $1",
        )
        .bind(promo_id)
        .fetch_all(&mut **tx)
        .await?;

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

        rules.push(PromoRule {
            id: promo_id,
            name: r.get("name"),
            kind,
            value: r.get("value"),
            scope,
            product_ids,
            category_ids,
            min_qty: r.get("min_qty"),
            min_subtotal: r.get("min_subtotal"),
            max_discount: r.get("max_discount"),
            priority: r.get("priority"),
            created_at: r.get("created_at"),
            skip_tier_lines: r.get("skip_tier_lines"),
        });
    }

    Ok(rules)
}

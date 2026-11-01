#![allow(clippy::collapsible_if)]

use axum::http::StatusCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OptionRow {
    pub id: Uuid,
    pub group_id: Uuid,
    pub name: String,
    pub price_delta: Decimal,
    pub input_product_id: Option<Uuid>,
    pub input_qty: Option<Decimal>,
    pub sort_order: i32,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OptionGroupRow {
    pub id: Uuid,
    pub name: String,
    pub min_select: i32,
    pub max_select: i32,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionGroupDetail {
    pub id: Uuid,
    pub name: String,
    pub min_select: i32,
    pub max_select: i32,
    pub active: bool,
    pub options: Vec<OptionRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionInput {
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub price_delta: Decimal,
    pub input_product_id: Option<Uuid>,
    pub input_qty: Option<Decimal>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}

fn default_one() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateOptionGroupInput {
    pub name: String,
    #[serde(default)]
    pub min_select: i32,
    #[serde(default = "default_one")]
    pub max_select: i32,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub options: Vec<OptionInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProductOptionGroupInput {
    pub group_id: Uuid,
    #[serde(default)]
    pub sort_order: i32,
}

fn validate_group_input(input: &CreateOptionGroupInput) -> Result<(), ApiError> {
    let name_trimmed = input.name.trim();
    if name_trimmed.is_empty() || name_trimmed.chars().count() > 40 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OPTION_INVALID",
            "El nombre del grupo debe tener entre 1 y 40 caracteres",
            None,
        ));
    }
    if input.min_select < 0 || input.max_select < 1 || input.min_select > input.max_select {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OPTION_INVALID",
            "Selección mínima y máxima inválidas",
            None,
        ));
    }
    for opt in &input.options {
        let opt_name = opt.name.trim();
        if opt_name.is_empty() || opt_name.chars().count() > 40 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "OPTION_INVALID",
                "El nombre de la opción debe tener entre 1 y 40 caracteres",
                None,
            ));
        }
        if opt.price_delta < Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "OPTION_INVALID",
                "El precio extra no puede ser negativo",
                None,
            ));
        }
        if opt.input_product_id.is_some() != opt.input_qty.is_some() {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "OPTION_INVALID",
                "Insumo y cantidad deben especificarse juntos",
                None,
            ));
        }
        if let Some(qty) = opt.input_qty {
            if qty <= Decimal::ZERO {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "OPTION_INVALID",
                    "La cantidad de insumo debe ser mayor a 0",
                    None,
                ));
            }
        }
    }
    Ok(())
}

pub async fn list_option_groups(
    pool: &PgPool,
    claims: &Claims,
) -> Result<Vec<OptionGroupDetail>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let groups: Vec<OptionGroupRow> = sqlx::query_as(
        "select id, name, min_select, max_select, active
           from option_groups
          where tenant_id = $1
          order by name",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    let all_options: Vec<OptionRow> = sqlx::query_as(
        "select id, group_id, name, price_delta, input_product_id, input_qty, sort_order, active
           from options
          where tenant_id = $1
          order by sort_order, name",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut result = Vec::with_capacity(groups.len());
    for g in groups {
        let opts: Vec<OptionRow> = all_options
            .iter()
            .filter(|o| o.group_id == g.id)
            .cloned()
            .collect();
        result.push(OptionGroupDetail {
            id: g.id,
            name: g.name,
            min_select: g.min_select,
            max_select: g.max_select,
            active: g.active,
            options: opts,
        });
    }

    Ok(result)
}

pub async fn create_option_group(
    pool: &PgPool,
    claims: &Claims,
    input: CreateOptionGroupInput,
) -> Result<OptionGroupDetail, ApiError> {
    validate_group_input(&input)?;
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    let group_id: Uuid = sqlx::query_scalar(
        "insert into option_groups (tenant_id, name, min_select, max_select, active)
         values ($1, $2, $3, $4, $5)
         returning id",
    )
    .bind(scope.tenant)
    .bind(input.name.trim())
    .bind(input.min_select)
    .bind(input.max_select)
    .bind(input.active)
    .fetch_one(&mut *tx)
    .await?;

    let mut created_options = Vec::with_capacity(input.options.len());
    for opt in input.options {
        let opt_id: Uuid = sqlx::query_scalar(
            "insert into options (tenant_id, group_id, name, price_delta, input_product_id, input_qty, sort_order, active)
             values ($1, $2, $3, $4, $5, $6, $7, $8)
             returning id",
        )
        .bind(scope.tenant)
        .bind(group_id)
        .bind(opt.name.trim())
        .bind(opt.price_delta)
        .bind(opt.input_product_id)
        .bind(opt.input_qty)
        .bind(opt.sort_order)
        .bind(opt.active)
        .fetch_one(&mut *tx)
        .await?;

        created_options.push(OptionRow {
            id: opt_id,
            group_id,
            name: opt.name.trim().to_string(),
            price_delta: opt.price_delta,
            input_product_id: opt.input_product_id,
            input_qty: opt.input_qty,
            sort_order: opt.sort_order,
            active: opt.active,
        });
    }

    tx.commit().await?;

    Ok(OptionGroupDetail {
        id: group_id,
        name: input.name.trim().to_string(),
        min_select: input.min_select,
        max_select: input.max_select,
        active: input.active,
        options: created_options,
    })
}

pub async fn update_option_group(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    input: CreateOptionGroupInput,
) -> Result<OptionGroupDetail, ApiError> {
    validate_group_input(&input)?;
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    sqlx::query(
        "update option_groups
            set name = $3, min_select = $4, max_select = $5, active = $6
          where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(id)
    .bind(input.name.trim())
    .bind(input.min_select)
    .bind(input.max_select)
    .bind(input.active)
    .execute(&mut *tx)
    .await?;

    // Desactivar opciones existentes que no vienen en el input
    let present_ids: Vec<Uuid> = input.options.iter().filter_map(|o| o.id).collect();
    sqlx::query(
        "update options
            set active = false
          where tenant_id = $1 and group_id = $2 and not (id = any($3))",
    )
    .bind(scope.tenant)
    .bind(id)
    .bind(&present_ids)
    .execute(&mut *tx)
    .await?;

    // Actualizar o insertar opciones
    let mut final_options = Vec::with_capacity(input.options.len());
    for opt in input.options {
        if let Some(opt_id) = opt.id {
            sqlx::query(
                "update options
                    set name = $4, price_delta = $5, input_product_id = $6, input_qty = $7, sort_order = $8, active = $9
                  where tenant_id = $1 and group_id = $2 and id = $3",
            )
            .bind(scope.tenant)
            .bind(id)
            .bind(opt_id)
            .bind(opt.name.trim())
            .bind(opt.price_delta)
            .bind(opt.input_product_id)
            .bind(opt.input_qty)
            .bind(opt.sort_order)
            .bind(opt.active)
            .execute(&mut *tx)
            .await?;

            final_options.push(OptionRow {
                id: opt_id,
                group_id: id,
                name: opt.name.trim().to_string(),
                price_delta: opt.price_delta,
                input_product_id: opt.input_product_id,
                input_qty: opt.input_qty,
                sort_order: opt.sort_order,
                active: opt.active,
            });
        } else {
            let new_id: Uuid = sqlx::query_scalar(
                "insert into options (tenant_id, group_id, name, price_delta, input_product_id, input_qty, sort_order, active)
                 values ($1, $2, $3, $4, $5, $6, $7, $8)
                 returning id",
            )
            .bind(scope.tenant)
            .bind(id)
            .bind(opt.name.trim())
            .bind(opt.price_delta)
            .bind(opt.input_product_id)
            .bind(opt.input_qty)
            .bind(opt.sort_order)
            .bind(opt.active)
            .fetch_one(&mut *tx)
            .await?;

            final_options.push(OptionRow {
                id: new_id,
                group_id: id,
                name: opt.name.trim().to_string(),
                price_delta: opt.price_delta,
                input_product_id: opt.input_product_id,
                input_qty: opt.input_qty,
                sort_order: opt.sort_order,
                active: opt.active,
            });
        }
    }

    tx.commit().await?;

    Ok(OptionGroupDetail {
        id,
        name: input.name.trim().to_string(),
        min_select: input.min_select,
        max_select: input.max_select,
        active: input.active,
        options: final_options,
    })
}

pub async fn set_product_option_groups(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
    groups: Vec<UpdateProductOptionGroupInput>,
) -> Result<serde_json::Value, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:adjust")?;

    sqlx::query("delete from product_option_groups where tenant_id = $1 and product_id = $2")
        .bind(scope.tenant)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    for g in groups {
        sqlx::query(
            "insert into product_option_groups (tenant_id, product_id, group_id, sort_order)
             values ($1, $2, $3, $4)",
        )
        .bind(scope.tenant)
        .bind(product_id)
        .bind(g.group_id)
        .bind(g.sort_order)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(serde_json::json!({ "status": "ok" }))
}

pub async fn get_product_option_groups_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    product_id: Uuid,
) -> Result<Vec<OptionGroupDetail>, ApiError> {
    let groups: Vec<OptionGroupRow> = sqlx::query_as(
        "select og.id, og.name, og.min_select, og.max_select, og.active
           from product_option_groups pog
           join option_groups og on og.id = pog.group_id
          where pog.tenant_id = $1 and pog.product_id = $2 and og.active
          order by pog.sort_order",
    )
    .bind(scope.tenant)
    .bind(product_id)
    .fetch_all(&mut **tx)
    .await?;

    if groups.is_empty() {
        return Ok(vec![]);
    }

    let group_ids: Vec<Uuid> = groups.iter().map(|g| g.id).collect();
    let all_options: Vec<OptionRow> = sqlx::query_as(
        "select id, group_id, name, price_delta, input_product_id, input_qty, sort_order, active
           from options
          where tenant_id = $1 and group_id = any($2) and active
          order by sort_order, name",
    )
    .bind(scope.tenant)
    .bind(&group_ids)
    .fetch_all(&mut **tx)
    .await?;

    let mut result = Vec::with_capacity(groups.len());
    for g in groups {
        let opts: Vec<OptionRow> = all_options
            .iter()
            .filter(|o| o.group_id == g.id)
            .cloned()
            .collect();
        result.push(OptionGroupDetail {
            id: g.id,
            name: g.name,
            min_select: g.min_select,
            max_select: g.max_select,
            active: g.active,
            options: opts,
        });
    }

    Ok(result)
}

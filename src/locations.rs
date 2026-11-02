//! Gestión de bodegas y ubicaciones por sucursal (F2.8).

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy;

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LocationRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
    pub name: String,
    pub kind: String,
    pub sellable: bool,
    pub is_default: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLocationInput {
    pub branch_id: Uuid,
    pub name: String,
    pub kind: String,
    pub sellable: bool,
    #[serde(default)]
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLocationInput {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub sellable: Option<bool>,
    pub is_default: Option<bool>,
    pub active: Option<bool>,
}

fn validate_kind(kind: &str) -> bool {
    matches!(
        kind,
        "PISO_VENTA"
            | "BODEGA"
            | "PRODUCCION"
            | "RECEPCION"
            | "DESPACHO"
            | "CUARENTENA"
            | "AVERIADOS"
    )
}

pub async fn list_locations(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<LocationRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    let rows = sqlx::query_as::<_, LocationRow>(
        "select id, tenant_id, branch_id, name, kind, sellable, is_default, active, created_at
         from locations
         where tenant_id = $1 and branch_id = $2
         order by is_default desc, name asc",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

pub async fn create_location(
    pool: &PgPool,
    claims: &Claims,
    input: CreateLocationInput,
) -> Result<LocationRow, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, input.branch_id).await?;
    scope.require_permission("settings:write")?;

    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "El nombre debe tener entre 1 y 40 caracteres",
            None,
        ));
    }

    if !validate_kind(&input.kind) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "Tipo de bodega inválido",
            None,
        ));
    }

    if (input.kind == "CUARENTENA" || input.kind == "AVERIADOS") && input.sellable {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "Una bodega en cuarentena o averiados no puede ser vendible",
            None,
        ));
    }

    let is_default = input.is_default.unwrap_or(false);
    if is_default && !input.sellable {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "La bodega por defecto debe ser vendible",
            None,
        ));
    }

    if is_default {
        sqlx::query(
            "update locations set is_default = false where branch_id = $1 and is_default = true",
        )
        .bind(input.branch_id)
        .execute(&mut *tx)
        .await?;
    }

    let row = sqlx::query_as::<_, LocationRow>(
        "insert into locations (tenant_id, branch_id, name, kind, sellable, is_default, active)
         values ($1, $2, $3, $4, $5, $6, true)
         returning id, tenant_id, branch_id, name, kind, sellable, is_default, active, created_at",
    )
    .bind(scope.tenant)
    .bind(input.branch_id)
    .bind(name)
    .bind(&input.kind)
    .bind(input.sellable)
    .bind(is_default)
    .fetch_one(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "BODEGA_CREADA",
        "locations",
        row.id,
        None,
        Some(serde_json::to_value(&row).unwrap_or_default()),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

pub async fn update_location(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    input: UpdateLocationInput,
) -> Result<LocationRow, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let existing: Option<LocationRow> = sqlx::query_as(
        "select id, tenant_id, branch_id, name, kind, sellable, is_default, active, created_at
         from locations
         where tenant_id = $1 and id = $2
         for update",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let current = existing.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let name = input.name.as_deref().unwrap_or(&current.name).trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "El nombre debe tener entre 1 y 40 caracteres",
            None,
        ));
    }

    let kind = input.kind.as_deref().unwrap_or(&current.kind);
    if !validate_kind(kind) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "Tipo de bodega inválido",
            None,
        ));
    }

    let sellable = input.sellable.unwrap_or(current.sellable);
    if (kind == "CUARENTENA" || kind == "AVERIADOS") && sellable {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "Una bodega en cuarentena o averiados no puede ser vendible",
            None,
        ));
    }

    let is_default = input.is_default.unwrap_or(current.is_default);
    let active = input.active.unwrap_or(current.active);

    if is_default && (!sellable || !active) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOCATION_INVALID",
            "La bodega por defecto debe ser vendible y estar activa",
            None,
        ));
    }

    // Desactivar con stock != 0 da 409 LOCATION_HAS_STOCK
    if current.active && !active {
        let has_stock: bool = sqlx::query_scalar(
            "select exists(select 1 from stock where location_id = $1 and on_hand <> 0)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        if has_stock {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "LOCATION_HAS_STOCK",
                "No se puede desactivar una bodega que tiene existencias",
                None,
            ));
        }
    }

    if is_default && !current.is_default {
        sqlx::query("update locations set is_default = false where branch_id = $1 and id <> $2 and is_default = true")
            .bind(current.branch_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    let updated = sqlx::query_as::<_, LocationRow>(
        "update locations
            set name = $1, kind = $2, sellable = $3, is_default = $4, active = $5
          where tenant_id = $6 and id = $7
          returning id, tenant_id, branch_id, name, kind, sellable, is_default, active, created_at",
    )
    .bind(name)
    .bind(kind)
    .bind(sellable)
    .bind(is_default)
    .bind(active)
    .bind(scope.tenant)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "BODEGA_ACTUALIZADA",
        "locations",
        id,
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(updated)
}

pub async fn default_location_id_tx(tx: &mut Tx<'_>, branch_id: Uuid) -> Result<Uuid, ApiError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("select id from locations where branch_id = $1 and is_default = true")
            .bind(branch_id)
            .fetch_optional(&mut **tx)
            .await?;

    id.ok_or_else(|| {
        error::coded(
            StatusCode::NOT_FOUND,
            "DEFAULT_LOCATION_NOT_FOUND",
            "No se encontró bodega por defecto para la sucursal",
            None,
        )
    })
}

pub async fn default_location_id(pool: &PgPool, branch_id: Uuid) -> Result<Uuid, ApiError> {
    let mut tx = pool.begin().await?;
    let id = default_location_id_tx(&mut tx, branch_id).await?;
    tx.commit().await?;
    Ok(id)
}

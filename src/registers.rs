//! Gestión de cajas registradoras por sucursal.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct OpenSessionBrief {
    pub id: Uuid,
    pub opened_by: Uuid,
    pub opened_by_name: Option<String>,
    pub opened_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Register {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub name: String,
    pub code: String,
    pub default_opening: Decimal,
    pub active: bool,
    pub open_session: Option<OpenSessionBrief>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRegisterRequest {
    pub branch_id: Uuid,
    pub name: String,
    pub code: String,
    pub default_opening: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRegisterRequest {
    pub name: String,
    pub code: String,
    pub default_opening: Decimal,
    pub active: bool,
}

fn validate_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 10
        && code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
}

/// Lista de cajas de la sucursal ordenada por código.
/// Permiso: cash:open o settings:write.
pub async fn list(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<Register>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    if !scope.has_permission("cash:open") && !scope.has_permission("settings:write") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        branch_id: Uuid,
        name: String,
        code: String,
        default_opening: Decimal,
        active: bool,
        session_id: Option<Uuid>,
        opened_by: Option<Uuid>,
        opened_by_name: Option<String>,
        opened_at: Option<DateTime<Utc>>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "select r.id, r.branch_id, r.name, r.code, r.default_opening, r.active,
                s.id as session_id, s.opened_by, u.full_name as opened_by_name, s.opened_at
         from cash_registers r
         left join cash_sessions s on s.register_id = r.id and s.closed_at is null
         left join users u on u.id = s.opened_by
         where r.tenant_id = $1 and r.branch_id = $2
         order by r.code",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_all(&mut *tx)
    .await?;

    let registers = rows
        .into_iter()
        .map(|r| Register {
            id: r.id,
            branch_id: r.branch_id,
            name: r.name,
            code: r.code,
            default_opening: r.default_opening,
            active: r.active,
            open_session: r.session_id.map(|sid| OpenSessionBrief {
                id: sid,
                opened_by: r.opened_by.unwrap_or_default(),
                opened_by_name: r.opened_by_name,
                opened_at: r.opened_at.unwrap_or_else(Utc::now),
            }),
        })
        .collect();

    tx.commit().await?;
    Ok(registers)
}

/// Crea una nueva caja registradora.
/// Permiso: settings:write.
pub async fn create(
    pool: &PgPool,
    claims: &Claims,
    req: CreateRegisterRequest,
) -> Result<Register, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("settings:write")?;

    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "El nombre debe tener entre 1 y 60 caracteres",
            None,
        ));
    }

    let code = req.code.trim().to_uppercase();
    if !validate_code(&code) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "El código debe cumplir ^[A-Z0-9-]{1,10}$",
            None,
        ));
    }

    if req.default_opening < Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "La base sugerida no puede ser negativa",
            None,
        ));
    }

    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from cash_registers where branch_id = $1 and code = $2)",
    )
    .bind(req.branch_id)
    .bind(&code)
    .fetch_one(&mut *tx)
    .await?;

    if exists {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "REGISTER_CODE_TAKEN",
            "Ya existe una caja con ese código en la sucursal",
            None,
        ));
    }

    let id: Uuid = sqlx::query_scalar(
        "insert into cash_registers (tenant_id, branch_id, name, code, default_opening, active)
         values ($1, $2, $3, $4, $5, true)
         returning id",
    )
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(name)
    .bind(&code)
    .bind(req.default_opening)
    .fetch_one(&mut *tx)
    .await?;

    let register = Register {
        id,
        branch_id: req.branch_id,
        name: name.to_string(),
        code: code.clone(),
        default_opening: req.default_opening,
        active: true,
        open_session: None,
    };

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "CAJA_CREADA",
        "cash_registers",
        id,
        None,
        Some(serde_json::to_value(&register).unwrap_or_default()),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(register)
}

/// Actualiza una caja registradora.
/// Permiso: settings:write.
pub async fn update(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    req: UpdateRegisterRequest,
) -> Result<Register, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("settings:write")?;

    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "El nombre debe tener entre 1 y 60 caracteres",
            None,
        ));
    }

    let code = req.code.trim().to_uppercase();
    if !validate_code(&code) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "El código debe cumplir ^[A-Z0-9-]{1,10}$",
            None,
        ));
    }

    if req.default_opening < Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REGISTER_INVALID",
            "La base sugerida no puede ser negativa",
            None,
        ));
    }

    #[derive(sqlx::FromRow)]
    struct CurrentRow {
        branch_id: Uuid,
        name: String,
        code: String,
        default_opening: Decimal,
        active: bool,
    }

    let current = sqlx::query_as::<_, CurrentRow>(
        "select branch_id, name, code, default_opening, active
         from cash_registers
         where id = $1 for update",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let current = current.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let code_taken: bool = sqlx::query_scalar(
        "select exists(select 1 from cash_registers where branch_id = $1 and code = $2 and id != $3)",
    )
    .bind(current.branch_id)
    .bind(&code)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if code_taken {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "REGISTER_CODE_TAKEN",
            "Ya existe una caja con ese código en la sucursal",
            None,
        ));
    }

    if !req.active && current.active {
        let has_open: bool = sqlx::query_scalar(
            "select exists(select 1 from cash_sessions where register_id = $1 and closed_at is null)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        if has_open {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "REGISTER_HAS_OPEN_SESSION",
                "No se puede desactivar una caja con un turno abierto",
                None,
            ));
        }
    }

    let before = serde_json::json!({
        "id": id,
        "branch_id": current.branch_id,
        "name": current.name,
        "code": current.code,
        "default_opening": current.default_opening,
        "active": current.active,
    });

    sqlx::query(
        "update cash_registers
         set name = $1, code = $2, default_opening = $3, active = $4
         where id = $5",
    )
    .bind(name)
    .bind(&code)
    .bind(req.default_opening)
    .bind(req.active)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let after = serde_json::json!({
        "id": id,
        "branch_id": current.branch_id,
        "name": name,
        "code": code,
        "default_opening": req.default_opening,
        "active": req.active,
    });

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "CAJA_EDITADA",
        "cash_registers",
        id,
        Some(before),
        Some(after),
        None,
    )
    .await?;

    // Obtener sesión abierta si existe
    let session = sqlx::query_as::<_, OpenSessionBrief>(
        "select s.id, s.opened_by, u.full_name as opened_by_name, s.opened_at
         from cash_sessions s
         left join users u on u.id = s.opened_by
         where s.register_id = $1 and s.closed_at is null",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let updated = Register {
        id,
        branch_id: current.branch_id,
        name: name.to_string(),
        code: code.to_string(),
        default_opening: req.default_opening,
        active: req.active,
        open_session: session,
    };

    tx.commit().await?;
    Ok(updated)
}

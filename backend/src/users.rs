//! Gestión de usuarios, membresías e invitaciones (F0.2).

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state, validation};
use crate::tenancy;

pub const ASSIGNABLE_ROLES: &[&str] = &["ADMIN_CAJA", "SUPERVISOR", "CAJERO", "BODEGA"];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TenantUser {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub email: Option<String>,
    pub branch_id: Option<Uuid>,
    pub branch_name: Option<String>,
    pub role: String,
    pub role_name: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Invitation {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub branch_id: Option<Uuid>,
    pub branch_name: Option<String>,
    pub role: String,
    pub role_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersListResponse {
    pub users: Vec<TenantUser>,
    pub invitations: Vec<Invitation>,
}

#[derive(Debug, Deserialize)]
pub struct InviteUserRequest {
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub branch_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserStatusRequest {
    pub active: bool,
}

/// Lista usuarios activos y pendientes del tenant.
pub async fn list_users(pool: &PgPool, claims: &Claims) -> Result<UsersListResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    let users = sqlx::query_as::<_, TenantUser>(
        "select m.id as membership_id, m.user_id, u.full_name, coalesce(u.email, au.email) as email,
                m.branch_id, b.name as branch_name, t.tima_codigo as role, t.tima_nombre as role_name,
                m.active, m.created_at
           from memberships m
           join users u on u.id = m.user_id
           left join auth.users au on au.id = m.user_id
           left join branches b on b.id = m.branch_id
           join tbl_tipos_maestra t on t.tima_id = m.role_id
          where m.tenant_id = $1
          order by m.created_at",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    let invitations = sqlx::query_as::<_, Invitation>(
        "select i.id, i.email, i.full_name, i.branch_id, b.name as branch_name,
                t.tima_codigo as role, t.tima_nombre as role_name, i.status, i.created_at
           from invitations i
           left join branches b on b.id = i.branch_id
           join tbl_tipos_maestra t on t.tima_id = i.role_id
          where i.tenant_id = $1 and i.status = 'PENDIENTE'
          order by i.created_at desc",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(UsersListResponse { users, invitations })
}

/// Invita a un usuario al tenant por email.
pub async fn invite_user(
    pool: &PgPool,
    claims: &Claims,
    req: InviteUserRequest,
) -> Result<Invitation, ApiError> {
    let email = req.email.trim().to_lowercase();
    let name = req.full_name.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(validation("invalid email"));
    }
    let name_len = name.chars().count();
    if !(2..=80).contains(&name_len) {
        return Err(validation("full_name must be between 2 and 80 characters"));
    }

    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    if req.role == "DUENO" && scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    if req.role != "DUENO" && !ASSIGNABLE_ROLES.contains(&req.role.as_str()) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ROLE_NOT_AVAILABLE",
            "Ese rol todavía no está disponible.",
            None,
        ));
    }

    let already_member: bool = sqlx::query_scalar(
        "select exists (
           select 1 from memberships m
           join users u on u.id = m.user_id
           where m.tenant_id = $1 and m.active and lower(u.email) = $2
         )",
    )
    .bind(scope.tenant)
    .bind(&email)
    .fetch_one(&mut *tx)
    .await?;

    if already_member {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "USER_ALREADY_MEMBER",
            "Esa persona ya trabaja en tu negocio.",
            None,
        ));
    }

    let role_id: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra
          where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
            and tima_codigo = $1",
    )
    .bind(&req.role)
    .fetch_optional(&mut *tx)
    .await?;

    let role_id = role_id.ok_or_else(|| validation("invalid role"))?;

    if let Some(b) = req.branch_id {
        tenancy::ensure_branch_tx(&mut tx, &scope, b).await?;
    }

    let inv_id: Uuid = sqlx::query_scalar(
        "insert into invitations (tenant_id, branch_id, email, full_name, role_id, status, invited_by, created_at)
         values ($1, $2, $3, $4, $5, 'PENDIENTE', $6, now())
         on conflict (tenant_id, email)
         do update set branch_id = excluded.branch_id, full_name = excluded.full_name,
                       role_id = excluded.role_id, status = 'PENDIENTE', created_at = now()
         returning id",
    )
    .bind(scope.tenant)
    .bind(req.branch_id)
    .bind(&email)
    .bind(name)
    .bind(role_id)
    .bind(scope.user)
    .fetch_one(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "USUARIO_INVITADO",
        "invitations",
        inv_id,
        None,
        Some(serde_json::json!({
            "email": email,
            "role": req.role,
            "branch_id": req.branch_id,
        })),
        None,
    )
    .await?;

    let inv = sqlx::query_as::<_, Invitation>(
        "select i.id, i.email, i.full_name, i.branch_id, b.name as branch_name,
                t.tima_codigo as role, t.tima_nombre as role_name, i.status, i.created_at
           from invitations i
           left join branches b on b.id = i.branch_id
           join tbl_tipos_maestra t on t.tima_id = i.role_id
          where i.id = $1",
    )
    .bind(inv_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(inv)
}

/// Cambia el rol de un usuario.
/// [P] Regla: El dueño no puede quitarse su propio rol.
pub async fn update_user_role(
    pool: &PgPool,
    claims: &Claims,
    membership_id: Uuid,
    req: UpdateUserRoleRequest,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    if req.role == "DUENO" && scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    if req.role != "DUENO" && !ASSIGNABLE_ROLES.contains(&req.role.as_str()) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ROLE_NOT_AVAILABLE",
            "Ese rol todavía no está disponible.",
            None,
        ));
    }

    let target: Option<(Uuid, String)> = sqlx::query_as(
        "select m.user_id, t.tima_codigo
           from memberships m
           join tbl_tipos_maestra t on t.tima_id = m.role_id
          where m.id = $1 and m.tenant_id = $2",
    )
    .bind(membership_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let (target_user_id, current_role) =
        target.ok_or_else(|| ApiError::Domain(DomainError::NotFound))?;

    // [P] Solo el DUENO puede asignar o quitar el rol DUENO, o cambiarle el rol a un dueño.
    // ADMIN_CAJA no puede subir a nadie a dueño ni tocar a un dueño.
    if scope.role != "DUENO" && (req.role == "DUENO" || current_role == "DUENO") {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    // [P] El dueño no puede quitarse su propio rol
    if target_user_id == scope.user && current_role == "DUENO" && req.role != "DUENO" {
        return Err(state("an owner cannot remove their own DUENO role"));
    }

    let new_role_id: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra
          where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
            and tima_codigo = $1",
    )
    .bind(&req.role)
    .fetch_optional(&mut *tx)
    .await?;

    let new_role_id = new_role_id.ok_or_else(|| validation("invalid role"))?;

    sqlx::query("update memberships set role_id = $1 where id = $2 and tenant_id = $3")
        .bind(new_role_id)
        .bind(membership_id)
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;

    // Registrar en auditoría
    crate::audit::record_tx(
        &mut tx,
        &scope,
        "CAMBIAR_ROL",
        "memberships",
        membership_id,
        Some(serde_json::json!({ "role": current_role })),
        Some(serde_json::json!({ "role": req.role })),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Activa o desactiva la membresía de un usuario.
pub async fn update_user_status(
    pool: &PgPool,
    claims: &Claims,
    membership_id: Uuid,
    req: UpdateUserStatusRequest,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    let target: Option<(Uuid, String)> = sqlx::query_as(
        "select m.user_id, t.tima_codigo
           from memberships m
           join tbl_tipos_maestra t on t.tima_id = m.role_id
          where m.id = $1 and m.tenant_id = $2",
    )
    .bind(membership_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let (target_user_id, role) = target.ok_or_else(|| ApiError::Domain(DomainError::NotFound))?;

    // [P] Solo el DUENO puede desactivar a un dueño. ADMIN_CAJA no puede tocar a un dueño.
    if role == "DUENO" && scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    // No permitir desactivarse a sí mismo si es el dueño
    if target_user_id == scope.user && role == "DUENO" && !req.active {
        return Err(state("an owner cannot deactivate their own membership"));
    }

    sqlx::query("update memberships set active = $1 where id = $2 and tenant_id = $3")
        .bind(req.active)
        .bind(membership_id)
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Mesero activo de una sucursal (selector de propina del POS, F1.10).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Waiter {
    pub user_id: Uuid,
    pub full_name: String,
}

/// `GET /v1/waiters?branch_id=`: usuarios con rol MESERO y membresía activa en
/// la sucursal (o en todo el negocio). Exige `pos:sell`.
pub async fn list_waiters(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<Waiter>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("pos:sell")?;
    let rows = sqlx::query_as::<_, Waiter>(
        "select distinct u.id as user_id, u.full_name
           from memberships m
           join users u on u.id = m.user_id
           join tbl_tipos_maestra t on t.tima_id = m.role_id
          where m.tenant_id = $1 and m.active
            and (m.branch_id is null or m.branch_id = $2)
            and t.tima_codigo = 'MESERO'
          order by u.full_name",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// Revoca una invitación pendiente.
pub async fn revoke_invitation(
    pool: &PgPool,
    claims: &Claims,
    invitation_id: Uuid,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    let row: Option<(String, String)> = sqlx::query_as(
        "select i.status, t.tima_codigo as role_code
           from invitations i
           join tbl_tipos_maestra t on t.tima_id = i.role_id
          where i.id = $1 and i.tenant_id = $2
          for update of i",
    )
    .bind(invitation_id)
    .bind(scope.tenant)
    .fetch_optional(&mut *tx)
    .await?;

    let (status, role_code) = row.ok_or_else(|| ApiError::Domain(DomainError::NotFound))?;

    if role_code == "DUENO" && scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    if status != "PENDIENTE" {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "INVITATION_NOT_PENDING",
            "Esa invitación ya no está vigente.",
            None,
        ));
    }

    sqlx::query("update invitations set status = 'REVOCADA' where id = $1")
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "INVITACION_REVOCADA",
        "invitations",
        invitation_id,
        None,
        None,
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

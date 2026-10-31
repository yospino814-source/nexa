//! Gestión de usuarios, membresías e invitaciones (F0.2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state, validation};
use crate::tenancy;

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
    if name.is_empty() {
        return Err(validation("full_name cannot be empty"));
    }

    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

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

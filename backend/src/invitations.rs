//! Gestión de invitaciones desde la perspectiva del usuario invitado (S2).

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::ApiError;
use crate::tenancy::Scope;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MyInvitation {
    pub id: Uuid,
    pub tenant_name: String,
    pub branch_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptInvitationResponse {
    pub tenant_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub role: String,
}

/// GET /v1/invitations/mine
pub async fn my_invitations(pool: &PgPool, claims: &Claims) -> Result<Vec<MyInvitation>, ApiError> {
    let mut tx = crate::db::begin_scope_tx(pool, claims, None, None).await?;
    let rows = sqlx::query_as::<_, MyInvitation>(
        "select id, tenant_name, branch_name, role_code, role_name, created_at from public.my_invitations()",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// POST /v1/invitations/{id}/accept
pub async fn accept_invitation(
    pool: &PgPool,
    claims: &Claims,
    invitation_id: Uuid,
) -> Result<AcceptInvitationResponse, ApiError> {
    let mut tx = crate::db::begin_scope_tx(pool, claims, None, None).await?;

    let row: (Uuid, Option<Uuid>, String) = sqlx::query_as(
        "select out_tenant_id, out_branch_id, out_role_code from public.accept_invitation($1)",
    )
    .bind(invitation_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| match err {
        sqlx::Error::Database(e) => {
            let msg = e.message();
            if msg.contains("INVITATION_NOT_FOUND") {
                crate::error::coded(
                    StatusCode::NOT_FOUND,
                    "INVITATION_NOT_FOUND",
                    "No encontramos esa invitación para tu correo.",
                    None,
                )
            } else if msg.contains("INVITATION_NOT_PENDING") {
                crate::error::coded(
                    StatusCode::CONFLICT,
                    "INVITATION_NOT_PENDING",
                    "Esa invitación ya no está vigente.",
                    None,
                )
            } else if msg.contains("ONBOARDING_NO_SESSION") {
                ApiError::Unauthorized
            } else {
                ApiError::Database(sqlx::Error::Database(e))
            }
        }
        other => ApiError::from(other),
    })?;

    let (tenant, branch, role) = row;

    crate::db::set_scope(&mut tx, tenant, branch).await?;

    let permissions =
        crate::permissions::load_effective_permissions_tx(&mut tx, tenant, &role).await?;

    let scope = Scope {
        user: claims.sub,
        tenant,
        branch,
        role: role.clone(),
        permissions,
    };

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "INVITACION_ACEPTADA",
        "invitations",
        invitation_id,
        None,
        None,
        None,
    )
    .await?;

    tx.commit().await?;

    Ok(AcceptInvitationResponse {
        tenant_id: tenant,
        branch_id: branch,
        role,
    })
}

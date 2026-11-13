use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::ApiError;
use crate::tenancy::Scope;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateBusinessRequest {
    pub business_name: String,
    pub branch_name: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingResponse {
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
}

pub async fn create_business(
    pool: &PgPool,
    claims: &Claims,
    req: CreateBusinessRequest,
) -> Result<OnboardingResponse, ApiError> {
    let business_name = req.business_name.trim();
    let branch_name = req.branch_name.trim();
    let full_name = req.full_name.trim();

    let bn_len = business_name.chars().count();
    if !(2..=80).contains(&bn_len) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ONBOARDING_INVALID",
            "Revisá los datos del negocio.",
            Some(serde_json::json!({ "field": "business_name" })),
        ));
    }

    let br_len = branch_name.chars().count();
    if !(2..=60).contains(&br_len) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ONBOARDING_INVALID",
            "Revisá los datos del negocio.",
            Some(serde_json::json!({ "field": "branch_name" })),
        ));
    }

    let fn_len = full_name.chars().count();
    if !(2..=80).contains(&fn_len) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ONBOARDING_INVALID",
            "Revisá los datos del negocio.",
            Some(serde_json::json!({ "field": "full_name" })),
        ));
    }

    let mut tx = crate::db::begin_scope_tx(pool, claims, None, None).await?;

    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        "select tenant_id, branch_id from public.create_business($1, $2, $3)",
    )
    .bind(business_name)
    .bind(branch_name)
    .bind(full_name)
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| match err {
        sqlx::Error::Database(e) => {
            let msg = e.message();
            if msg.contains("ONBOARDING_ALREADY_MEMBER") {
                crate::error::coded(
                    StatusCode::CONFLICT,
                    "ONBOARDING_ALREADY_MEMBER",
                    "Tu usuario ya tiene un negocio.",
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

    let (tenant, branch) = row;

    crate::db::set_scope(&mut tx, tenant, Some(branch)).await?;

    let effective_permissions =
        crate::permissions::load_effective_permissions_tx(&mut tx, tenant, "DUENO").await?;

    let scope = Scope {
        user: claims.sub,
        tenant,
        branch: Some(branch),
        role: "DUENO".into(),
        permissions: effective_permissions,
    };

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "NEGOCIO_CREADO",
        "tenants",
        tenant,
        None,
        Some(serde_json::json!({
            "business_name": business_name,
            "branch_name": branch_name,
        })),
        None,
    )
    .await?;

    tx.commit().await?;

    Ok(OnboardingResponse {
        tenant_id: tenant,
        branch_id: branch,
    })
}

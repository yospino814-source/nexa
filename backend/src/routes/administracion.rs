//! Endpoints de permisos, ajustes, auditoría, usuarios y seguridad del login.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Permisos y roles
        .route("/v1/permissions", get(list_permissions))
        .route(
            "/v1/roles/{code}/permissions",
            get(get_role_permissions).put(update_role_permissions),
        )
        // Ajustes
        .route("/v1/settings", get(get_settings).put(update_settings))
        // Auditoria
        .route("/v1/audit", get(list_audit))
        // Usuarios y roles
        .route("/v1/onboarding", post(onboarding_create_business))
        .route("/v1/users", get(list_users))
        .route("/v1/waiters", get(list_waiters))
        .route("/v1/business", get(business_info).put(update_business))
        .route("/v1/users/invite", post(invite_user))
        .route(
            "/v1/users/invitations/{id}/revoke",
            post(revoke_user_invitation),
        )
        .route("/v1/invitations/mine", get(get_my_invitations))
        .route("/v1/invitations/{id}/accept", post(accept_user_invitation))
        .route("/v1/users/{id}/role", put(update_user_role))
        .route("/v1/users/{id}/status", put(update_user_status))
        // Auth / seguridad
        .route(
            "/v1/auth/login-attempt",
            post(auth_login_attempt).layer(
                tower_governor::GovernorLayer::new(crate::security::login_limiter())
                    .error_handler(crate::security::too_many),
            ),
        )
}

async fn list_permissions(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<permissions::Permission>>> {
    let perms = permissions::list_permissions(&state.pool, &user.claims).await?;
    Ok(Json(perms))
}

async fn get_role_permissions(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> ApiResult<Json<permissions::RolePermissionsResponse>> {
    let resp = permissions::get_role_permissions(&state.pool, &user.claims, &code).await?;
    Ok(Json(resp))
}

async fn update_role_permissions(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
    Body(req): Body<permissions::UpdateRolePermissionsRequest>,
) -> ApiResult<Json<permissions::RolePermissionsResponse>> {
    let resp = permissions::update_role_permissions(&state.pool, &user.claims, &code, req).await?;
    Ok(Json(resp))
}

async fn get_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<settings::BranchSettings>> {
    let s = settings::get_settings(&state.pool, &user.claims, q.branch_id).await?;
    Ok(Json(s))
}

async fn update_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<settings::UpdateSettingsRequest>,
) -> ApiResult<Json<settings::BranchSettings>> {
    let s = settings::update_settings(&state.pool, &user.claims, req).await?;
    Ok(Json(s))
}

async fn list_audit(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<audit::AuditFilter>,
) -> ApiResult<Json<Vec<audit::AuditEntry>>> {
    let entries = audit::list_audit_entries(&state.pool, &user.claims, filter).await?;
    Ok(Json(entries))
}

async fn list_users(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<users::UsersListResponse>> {
    let resp = users::list_users(&state.pool, &user.claims).await?;
    Ok(Json(resp))
}

async fn list_waiters(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Vec<users::Waiter>>> {
    Ok(Json(
        users::list_waiters(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn invite_user(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<users::InviteUserRequest>,
) -> ApiResult<(StatusCode, Json<users::Invitation>)> {
    let inv = users::invite_user(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(inv)))
}

async fn update_user_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<users::UpdateUserRoleRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    users::update_user_role(&state.pool, &user.claims, id, req).await?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn update_user_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<users::UpdateUserStatusRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    users::update_user_status(&state.pool, &user.claims, id, req).await?;
    Ok(Json(json!({ "status": "ok" })))
}

#[derive(Debug, Deserialize)]
pub struct LoginAttemptRequest {
    pub email: String,
    pub success: Option<bool>,
    #[serde(alias = "failure_reason")]
    pub reason: Option<String>,
}

async fn auth_login_attempt(
    State(state): State<AppState>,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: axum::http::HeaderMap,
    Body(req): Body<LoginAttemptRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let ip = crate::security::request_ip(&headers, peer.map(|c| (c.0).0)).map(|ip| ip.to_string());
    let ip = ip.as_deref();
    let user_agent = headers.get("user-agent").and_then(|h| h.to_str().ok());

    let count = crate::auth::check_and_record_login_attempt(
        &state.pool,
        &req.email,
        ip,
        user_agent,
        req.success,
        req.reason.as_deref(),
    )
    .await?;

    Ok(Json(
        json!({ "status": "ok", "failed_attempts_in_window": count }),
    ))
}

async fn business_info(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<settings::BusinessInfo>> {
    Ok(Json(
        settings::business_info(&state.pool, &user.claims).await?,
    ))
}

async fn update_business(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<settings::UpdateBusinessRequest>,
) -> ApiResult<Json<settings::BusinessInfo>> {
    let item = settings::update_business(&state.pool, &user.claims, req).await?;
    Ok(Json(item))
}

async fn onboarding_create_business(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<onboarding::CreateBusinessRequest>,
) -> ApiResult<(StatusCode, Json<onboarding::OnboardingResponse>)> {
    let res = onboarding::create_business(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(res)))
}

async fn revoke_user_invitation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    users::revoke_invitation(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_my_invitations(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<invitations::MyInvitation>>> {
    let res = invitations::my_invitations(&state.pool, &user.claims).await?;
    Ok(Json(res))
}

async fn accept_user_invitation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<invitations::AcceptInvitationResponse>> {
    let res = invitations::accept_invitation(&state.pool, &user.claims, id).await?;
    Ok(Json(res))
}

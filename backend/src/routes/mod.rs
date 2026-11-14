//! Endpoints HTTP. Cada área tiene su archivo con su `router()`; este reúne
//! los extractores comunes, la salud y el contexto.
//! Los handlers solo traducen HTTP <-> dominio; las reglas de
//! negocio, permisos y transacciones viven en cada modulo.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{self, ApiError, DomainError};
use crate::orders::OrderStatus;
use crate::sales::{Checkout, SaleRequest};
use crate::{
    AppState, audit, cash, catalog, customers, expenses, inventory, invitations, locations,
    onboarding, options, orders, payables, permissions, procurement, products, receivables,
    refunds, registers, reports, sales, sales_history, settings, tenancy, users,
};

mod administracion;
mod catalogo;
mod clientes;
mod compras;
mod inventario;
mod ordenes;
mod reportes;
mod ventas;

// Extractores que rechazan entradas invalidas con el mismo formato
// `{ "error": .. }` que el resto de la API (los de axum responden texto plano).

/// Cuerpo JSON.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Body<T>(pub T);

/// Query string.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

/// Parametros de la ruta.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

type ApiResult<T> = Result<T, ApiError>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/health/db", get(health_db))
        .route("/v1/context", get(context))
        .route("/v1/branches", get(branches).post(create_branch))
        .route("/v1/branches/{id}", put(update_branch))
        .merge(catalogo::router())
        .merge(clientes::router())
        .merge(ventas::router())
        .merge(ordenes::router())
        .merge(inventario::router())
        .merge(compras::router())
        .merge(administracion::router())
        .merge(reportes::router())
        .merge(crate::whatsapp::router())
}

#[derive(Deserialize)]
struct BranchQuery {
    branch_id: Uuid,
}

// -------------------------------------------------------------------- salud

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn health_db(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let role: String = sqlx::query_scalar("select current_user::text")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({ "db": "ok", "role": role })))
}

/// Identidad y alcance resueltos por membresia (tenant, sucursal y rol), tal
/// como los aplica RLS. 403 si el usuario no pertenece a ningun negocio.
pub async fn context(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let (uid, tenant, branch): (Option<Uuid>, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("select auth.uid(), public.current_tenant_id(), public.current_branch_id()")
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    let mut perms: Vec<String> = scope.permissions.into_iter().collect();
    perms.sort();
    Ok(Json(json!({
        "uid": uid,
        "tenant": tenant,
        "branch": branch,
        "role": scope.role,
        "permissions": perms,
    })))
}

/// Sucursales a las que el usuario tiene acceso, con su rol en cada una.
async fn branches(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<tenancy::BranchAccess>>> {
    Ok(Json(tenancy::branches(&state.pool, &user.claims).await?))
}

async fn create_branch(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<tenancy::CreateBranchRequest>,
) -> ApiResult<(StatusCode, Json<tenancy::BranchItem>)> {
    let item = tenancy::create_branch(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn update_branch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<tenancy::UpdateBranchRequest>,
) -> ApiResult<Json<tenancy::BranchItem>> {
    let item = tenancy::update_branch(&state.pool, &user.claims, id, req).await?;
    Ok(Json(item))
}

// ----------------------------------------------------------------- catalogo

mod auth;
mod catalog;
mod db;
mod error;
mod tenancy;

#[allow(dead_code)]
mod inventory;

#[cfg(test)]
mod db_tests;
#[cfg(test)]
mod test_support;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::net::SocketAddr;
use uuid::Uuid;

use crate::auth::{AuthUser, JwtValidator};
use crate::error::ApiError;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    jwt: JwtValidator,
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/health/db", get(health_db))
        .route("/v1/context", get(context))
        .route("/v1/catalog", get(catalog_handler))
        .route("/v1/products/{id}/recipe", get(recipe_handler))
        .with_state(state)
}

#[derive(Deserialize)]
struct CatalogQuery {
    location_id: Uuid,
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn health_db(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let role: String = sqlx::query_scalar("select current_user::text")
        .fetch_one(&state.pool)
        .await?;
    sqlx::query("select 1").execute(&state.pool).await?;
    Ok(Json(json!({ "db": "ok", "role": role })))
}

/// Devuelve la identidad y el tenant resueltos por RLS/GUC para el token recibido.
async fn context(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = db::begin_tenant_tx(&state.pool, &user.claims).await?;
    let uid: Option<Uuid> = sqlx::query_scalar("select auth.uid()")
        .fetch_one(&mut *tx)
        .await?;
    let tenant: Option<Uuid> = sqlx::query_scalar("select public.current_tenant_id()")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Json(json!({ "uid": uid, "tenant": tenant })))
}

/// Catalogo activo de una sucursal con precio, impuesto y disponibilidad.
async fn catalog_handler(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<Vec<catalog::CatalogItem>>, ApiError> {
    let items = catalog::catalog(&state.pool, &user.claims, query.location_id).await?;
    Ok(Json(items))
}

/// Insumos y cantidades de la receta de un producto.
async fn recipe_handler(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<catalog::RecipeItem>>, ApiError> {
    let items = catalog::recipe(&state.pool, &user.claims, id).await?;
    Ok(Json(items))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    auth::init_crypto();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexa_backend=info,tower_http=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let supabase_url = std::env::var("SUPABASE_URL")?;

    let pool = db::connect(&database_url).await?;
    let jwt = JwtValidator::new(&supabase_url);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, app(AppState { pool, jwt })).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_router() -> Router {
        auth::init_crypto();
        // Pool perezoso: no abre conexion, solo sirve para construir el router.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost:5432/db")
            .expect("lazy pool");
        let jwt = JwtValidator::new("http://localhost:54321");
        app(AppState { pool, jwt })
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn context_without_token_is_unauthorized() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/v1/context")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn context_with_malformed_token_is_unauthorized() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/v1/context")
                    .header("authorization", "Bearer not.a.jwt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

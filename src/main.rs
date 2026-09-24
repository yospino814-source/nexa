mod db;
mod error;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use sqlx::PgPool;
use std::net::SocketAddr;

use crate::error::ApiError;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/health/db", get(health_db))
        .with_state(state)
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexa_backend=info,tower_http=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = db::connect(&database_url).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, app(AppState { pool })).await?;
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
        // Pool perezoso: no abre conexion, solo sirve para construir el router.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost:5432/db")
            .expect("lazy pool");
        app(AppState { pool })
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
}

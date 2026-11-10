mod audit;
pub mod auth;
mod cash;
mod catalog;
pub mod coupons;
pub mod credit;
pub mod csv_import;
pub mod customers;
mod db;
mod error;
pub mod expenses;
mod inventory;
pub mod invitations;
pub mod locations;
pub mod money;
pub mod onboarding;
pub mod options;
mod orders;
pub mod outbox;
pub mod payables;
pub mod permissions;
pub mod pricing;
mod procurement;
pub mod products;
pub mod promotions;
pub mod receivables;
pub mod refunds;
pub mod registers;
pub mod routes;
mod sales;
pub mod sales_history;
pub mod sequences;
pub mod settings;
mod tenancy;
pub mod tiers;
pub mod users;

#[cfg(test)]
mod db_tests;
#[cfg(test)]
mod test_support;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use sqlx::PgPool;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::auth::JwtValidator;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt: JwtValidator,
}

/// Tiempo maximo de un request antes de responder 408.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

fn app(state: AppState) -> Router {
    routes::router()
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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
    let port: u16 = match std::env::var("PORT") {
        Ok(p) => p.parse()?,
        Err(_) => 8080,
    };
    let max_connections: u32 = match std::env::var("DB_MAX_CONNECTIONS") {
        Ok(n) => n.parse()?,
        Err(_) => 5,
    };

    let pool = db::connect(&database_url, max_connections).await?;
    let jwt = JwtValidator::new(&supabase_url);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, app(AppState { pool, jwt }))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Termina de atender los requests en curso al recibir Ctrl+C / SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
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

    #[tokio::test]
    async fn login_attempt_without_token_is_not_unauthorized() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login-attempt")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"email":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn protected_routes_require_token_and_answer_json() {
        for (method, uri) in [
            ("POST", "/v1/sales"),
            ("POST", "/v1/cash-sessions"),
            (
                "GET",
                "/v1/cash-sessions/current?branch_id=00000000-0000-0000-0000-000000000000",
            ),
            ("POST", "/v1/orders"),
            (
                "GET",
                "/v1/inventory/movements?branch_id=00000000-0000-0000-0000-000000000000",
            ),
            ("GET", "/v1/suppliers"),
            ("POST", "/v1/suppliers"),
            ("GET", "/v1/purchases"),
            ("POST", "/v1/purchases"),
            ("GET", "/v1/permissions"),
            (
                "GET",
                "/v1/settings?branch_id=00000000-0000-0000-0000-000000000000",
            ),
            ("GET", "/v1/audit"),
            ("GET", "/v1/users"),
        ] {
            let response = test_router()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {uri}"
            );
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"], "unauthorized");
        }
    }
}

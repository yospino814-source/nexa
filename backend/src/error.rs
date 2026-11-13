use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Errores de dominio que mapean a codigos HTTP concretos.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("forbidden")]
    Forbidden,
    #[error("validation: {0}")]
    Validation(String),
    /// Regla de negocio que impide la operacion en el estado actual
    /// (p. ej. vender sin turno de caja abierto). Se responde 409 con el motivo.
    #[error("{0}")]
    State(String),
    #[error("too many requests: {0}")]
    TooManyRequests(String),
    #[error("{message}")]
    Coded {
        status: StatusCode,
        code: &'static str,
        message: String,
        data: Option<serde_json::Value>,
    },
}

impl DomainError {
    pub fn status(&self) -> StatusCode {
        match self {
            DomainError::NotFound => StatusCode::NOT_FOUND,
            DomainError::Conflict | DomainError::State(_) => StatusCode::CONFLICT,
            DomainError::Forbidden => StatusCode::FORBIDDEN,
            DomainError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            DomainError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            DomainError::Coded { status, .. } => *status,
        }
    }
}

/// Atajos para construir errores de dominio.
pub fn validation(msg: impl Into<String>) -> ApiError {
    ApiError::Domain(DomainError::Validation(msg.into()))
}

pub fn state(msg: impl Into<String>) -> ApiError {
    ApiError::Domain(DomainError::State(msg.into()))
}

pub fn too_many_requests(msg: impl Into<String>) -> ApiError {
    ApiError::Domain(DomainError::TooManyRequests(msg.into()))
}

#[allow(dead_code)]
pub fn coded(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
    data: Option<serde_json::Value>,
) -> ApiError {
    ApiError::Domain(DomainError::Coded {
        status,
        code,
        message: message.into(),
        data,
    })
}

/// Error de la capa API.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Clasificacion de errores de Postgres que son culpa de la peticion, no del servidor.
fn db_status(err: &sqlx::Error) -> Option<StatusCode> {
    let code = err.as_database_error()?.code()?;
    match code.as_ref() {
        // unique_violation
        "23505" => Some(StatusCode::CONFLICT),
        // check_violation, foreign_key_violation, not_null_violation, datos invalidos
        "23514" | "23503" | "23502" | "22P02" | "22003" => Some(StatusCode::UNPROCESSABLE_ENTITY),
        _ => None,
    }
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Domain(e) => e.status(),
            ApiError::Database(e) => db_status(e).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> String {
        match self {
            ApiError::Unauthorized => "unauthorized".to_string(),
            ApiError::Domain(e) => e.to_string(),
            ApiError::Database(e) => match db_status(e) {
                Some(StatusCode::CONFLICT) => "conflict: duplicated record".to_string(),
                Some(_) => "validation: constraint violated".to_string(),
                None => "internal error".to_string(),
            },
            ApiError::Internal(_) => "internal error".to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            // El cliente solo ve "internal error"; el detalle queda en el log.
            tracing::error!(error = ?self, "request failed");
        }
        if let ApiError::Domain(DomainError::Coded {
            code,
            message,
            data,
            ..
        }) = self
        {
            let mut body = json!({
                "error": message,
                "code": code,
            });
            if let Some(d) = data {
                body["data"] = d;
            }
            return (status, Json(body)).into_response();
        }
        (status, Json(json!({ "error": self.message() }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(err: ApiError) -> StatusCode {
        err.status()
    }

    #[tokio::test]
    async fn coded_error_body_has_code() {
        use http_body_util::BodyExt;
        let err = coded(StatusCode::CONFLICT, "X", "m", None);
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], "X");
        assert_eq!(body["error"], "m");
        assert!(body.get("data").is_none());
    }

    #[test]
    fn domain_errors_map_to_expected_status() {
        assert_eq!(
            status_of(DomainError::NotFound.into()),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_of(DomainError::Conflict.into()),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_of(DomainError::Forbidden.into()),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_of(DomainError::Validation("missing".into()).into()),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status_of(state("no open session")), StatusCode::CONFLICT);
        assert_eq!(
            status_of(too_many_requests("rate limited")),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(status_of(ApiError::Unauthorized), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn non_database_sqlx_errors_are_internal() {
        assert_eq!(
            status_of(ApiError::Database(sqlx::Error::RowNotFound)),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
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
}

impl DomainError {
    pub fn status(&self) -> StatusCode {
        match self {
            DomainError::NotFound => StatusCode::NOT_FOUND,
            DomainError::Conflict => StatusCode::CONFLICT,
            DomainError::Forbidden => StatusCode::FORBIDDEN,
            DomainError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
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

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Domain(e) => e.status(),
            ApiError::Database(_) | ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> String {
        match self {
            ApiError::Unauthorized => "unauthorized".to_string(),
            ApiError::Domain(e) => e.to_string(),
            ApiError::Database(_) | ApiError::Internal(_) => "internal error".to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        (status, Json(json!({ "error": self.message() }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(err: ApiError) -> StatusCode {
        err.status()
    }

    #[test]
    fn domain_errors_map_to_expected_status() {
        assert_eq!(status_of(DomainError::NotFound.into()), StatusCode::NOT_FOUND);
        assert_eq!(status_of(DomainError::Conflict.into()), StatusCode::CONFLICT);
        assert_eq!(status_of(DomainError::Forbidden.into()), StatusCode::FORBIDDEN);
        assert_eq!(
            status_of(DomainError::Validation("missing".into()).into()),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status_of(ApiError::Unauthorized), StatusCode::UNAUTHORIZED);
    }
}

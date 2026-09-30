use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;
use std::sync::{Arc, Once};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::ApiError;
use crate::AppState;

/// Claims relevantes del token de Supabase Auth.
#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    #[serde(default)]
    pub app_metadata: serde_json::Value,
}

impl Claims {
    pub fn tenant_id(&self) -> Option<Uuid> {
        self.meta_uuid("tenant_id")
    }

    pub fn branch_id(&self) -> Option<Uuid> {
        self.meta_uuid("branch_id")
    }

    fn meta_uuid(&self, key: &str) -> Option<Uuid> {
        self.app_metadata
            .get(key)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    }
}

/// Instala una vez el proveedor criptografico de jsonwebtoken.
pub fn init_crypto() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER
            .install_default()
            .expect("install jsonwebtoken crypto provider");
    });
}

/// Valida tokens de Supabase Auth contra su JWKS.
#[derive(Clone)]
pub struct JwtValidator {
    jwks_url: String,
    client: reqwest::Client,
    keys: Arc<RwLock<Option<JwkSet>>>,
}

impl JwtValidator {
    pub fn new(supabase_url: &str) -> Self {
        let jwks_url = format!(
            "{}/auth/v1/.well-known/jwks.json",
            supabase_url.trim_end_matches('/')
        );
        Self {
            jwks_url,
            client: reqwest::Client::new(),
            keys: Arc::new(RwLock::new(None)),
        }
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, ApiError> {
        let set: JwkSet = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|_| ApiError::Unauthorized)?
            .json()
            .await
            .map_err(|_| ApiError::Unauthorized)?;
        *self.keys.write().await = Some(set.clone());
        Ok(set)
    }

    pub async fn validate(&self, token: &str) -> Result<Claims, ApiError> {
        let header = decode_header(token).map_err(|_| ApiError::Unauthorized)?;
        let kid = header.kid.clone().ok_or(ApiError::Unauthorized)?;

        let cached = {
            let guard = self.keys.read().await;
            guard.as_ref().and_then(|set| set.find(&kid).cloned())
        };
        let jwk = match cached {
            Some(jwk) => jwk,
            None => {
                let set = self.fetch_jwks().await?;
                set.find(&kid).cloned().ok_or(ApiError::Unauthorized)?
            }
        };

        let key = DecodingKey::from_jwk(&jwk).map_err(|_| ApiError::Unauthorized)?;
        let mut validation = Validation::new(header.alg);
        validation.set_audience(&["authenticated"]);
        let data =
            decode::<Claims>(token, &key, &validation).map_err(|_| ApiError::Unauthorized)?;
        Ok(data.claims)
    }
}

/// Extractor que exige un token valido y expone los claims.
pub struct AuthUser {
    pub claims: Claims,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let claims = state.jwt.validate(token).await?;
        Ok(AuthUser { claims })
    }
}

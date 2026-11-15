use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;

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

/// Minimo entre descargas del JWKS: un `kid` desconocido no puede forzar una
/// descarga por request (amplificacion contra Supabase).
const JWKS_REFETCH_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Default)]
struct JwksCache {
    set: Option<JwkSet>,
    fetched_at: Option<Instant>,
}

/// Valida tokens de Supabase Auth contra su JWKS.
#[derive(Clone)]
pub struct JwtValidator {
    jwks_url: String,
    issuer: String,
    client: reqwest::Client,
    cache: Arc<RwLock<JwksCache>>,
}

impl JwtValidator {
    pub fn new(supabase_url: &str) -> Self {
        let base = supabase_url.trim_end_matches('/');
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("build http client");
        Self {
            jwks_url: format!("{base}/auth/v1/.well-known/jwks.json"),
            issuer: format!("{base}/auth/v1"),
            client,
            cache: Arc::new(RwLock::new(JwksCache::default())),
        }
    }

    /// Descarga el JWKS si no se descargo hace poco. Devuelve el conjunto vigente.
    async fn refresh_jwks(&self) -> Result<Option<JwkSet>, ApiError> {
        let mut cache = self.cache.write().await;
        // Otro request pudo haberlo refrescado mientras esperabamos el lock.
        if cache
            .fetched_at
            .is_some_and(|at| at.elapsed() < JWKS_REFETCH_COOLDOWN)
        {
            return Ok(cache.set.clone());
        }
        cache.fetched_at = Some(Instant::now());

        let response = self.client.get(&self.jwks_url).send().await;
        match response {
            Ok(r) => match r.json::<JwkSet>().await {
                Ok(set) => cache.set = Some(set),
                Err(err) => tracing::warn!(%err, "invalid JWKS response"),
            },
            Err(err) => tracing::warn!(%err, "JWKS fetch failed"),
        }
        Ok(cache.set.clone())
    }

    pub async fn validate(&self, token: &str) -> Result<Claims, ApiError> {
        let header = decode_header(token).map_err(|_| ApiError::Unauthorized)?;
        // Solo claves asimetricas del JWKS: un HS* no tiene sentido aqui.
        if matches!(
            header.alg,
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512
        ) {
            return Err(ApiError::Unauthorized);
        }
        let kid = header.kid.clone().ok_or(ApiError::Unauthorized)?;

        let cached = {
            let guard = self.cache.read().await;
            guard.set.as_ref().and_then(|set| set.find(&kid).cloned())
        };
        let jwk = match cached {
            Some(jwk) => jwk,
            None => self
                .refresh_jwks()
                .await?
                .and_then(|set| set.find(&kid).cloned())
                .ok_or(ApiError::Unauthorized)?,
        };

        let key = DecodingKey::from_jwk(&jwk).map_err(|_| ApiError::Unauthorized)?;
        let mut validation = Validation::new(header.alg);
        validation.set_audience(&["authenticated"]);
        validation.set_issuer(&[&self.issuer]);
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

/// Registra o comprueba un intento de login para un correo.
/// Si en los últimos 15 minutos hay 5 o más fallos para ese correo, responde error 429 TooManyRequests.
pub async fn check_and_record_login_attempt(
    pool: &sqlx::PgPool,
    email: &str,
    ip: Option<&str>,
    user_agent: Option<&str>,
    success: Option<bool>,
    reason: Option<&str>,
) -> Result<i64, ApiError> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Err(crate::error::validation("email is required"));
    }

    // La tabla no admite escrituras directas del rol de la API: se usa la
    // función security definer de la migración 20261102000300.
    let count: i32 = sqlx::query_scalar("select private.record_login_attempt($1, $2, $3, $4, $5)")
        .bind(&email)
        .bind(ip)
        .bind(user_agent)
        .bind(success)
        .bind(reason)
        .fetch_one(pool)
        .await?;
    let count = i64::from(count);

    if count >= 5 {
        return Err(crate::error::too_many_requests(
            "demasiados intentos fallidos en los últimos 15 minutos",
        ));
    }

    Ok(count)
}

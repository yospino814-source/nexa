//! Controles transversales: IP real del cliente, límite de peticiones y
//! encabezados de seguridad en las respuestas.
//!
//! El navegador no llama directo al backend: pasa por el proxy de Next
//! (`/api/*`), así que la IP de la conexión es la del servidor de Next. Solo
//! con `TRUST_PROXY=1` se usa `X-Forwarded-For`, y de ahí la **última**
//! entrada, que es la que agregó nuestro proxy (las anteriores las puede
//! inventar el cliente).

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, header};
use axum::response::IntoResponse;
use serde_json::json;
use sha2::{Digest, Sha256};
use tower_governor::GovernorError;
use tower_governor::governor::{GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::KeyExtractor;
use tower_http::set_header::SetResponseHeaderLayer;

fn trust_proxy() -> bool {
    static T: OnceLock<bool> = OnceLock::new();
    *T.get_or_init(|| matches!(std::env::var("TRUST_PROXY").as_deref(), Ok("1" | "true")))
}

/// IP del cliente: la que agregó nuestro proxy si se confía en él, si no la
/// de la conexión.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, trust: bool) -> Option<IpAddr> {
    if trust {
        let from_proxy = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .filter_map(|s| s.trim().parse::<IpAddr>().ok())
            .next_back();
        if from_proxy.is_some() {
            return from_proxy;
        }
    }
    peer
}

/// IP del cliente para un request de axum.
pub fn request_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> Option<IpAddr> {
    client_ip(headers, peer.map(|p| p.ip()), trust_proxy())
}

/// Clave del límite: el usuario (por su token) si viene autenticado; si no,
/// la IP. Detrás del proxy de Next todos comparten IP, por eso se usa el token.
#[derive(Clone, Copy, Debug)]
pub struct ClientKey;

impl KeyExtractor for ClientKey {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(token) = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            // Solo un resumen del token: nunca se guarda el token.
            let digest = Sha256::digest(token.as_bytes());
            let hex: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
            return Ok(format!("t:{hex}"));
        }
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        Ok(match request_ip(req.headers(), peer) {
            Some(ip) => format!("ip:{ip}"),
            None => "ip:desconocida".to_string(),
        })
    }
}

pub type Limiter = Arc<GovernorConfig<ClientKey, governor::middleware::NoOpMiddleware>>;

fn limiter(period: Duration, burst: u32) -> Limiter {
    let config = GovernorConfigBuilder::default()
        .period(period)
        .burst_size(burst)
        .key_extractor(ClientKey)
        .finish()
        .expect("límite de peticiones válido");
    let config = Arc::new(config);
    // Limpia las claves viejas para que la memoria no crezca sin límite.
    let limiter = config.limiter().clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            limiter.retain_recent();
        }
    });
    config
}

/// API en general: ráfagas de 60 y luego 10 por segundo por usuario o IP.
/// Un POS con escáner y búsqueda no llega a eso; un script que barre sí.
pub fn api_limiter() -> Limiter {
    limiter(Duration::from_millis(100), 60)
}

/// Endpoints sin sesión que sirven para adivinar (login): 5 seguidos y luego
/// uno cada 12 segundos por IP.
pub fn login_limiter() -> Limiter {
    limiter(Duration::from_secs(12), 5)
}

/// Respuesta 429 con el mismo formato `{ "error": .. }` del resto de la API.
pub fn too_many(err: GovernorError) -> Response<Body> {
    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": format!("demasiadas peticiones; intentá de nuevo en {wait_time} s") })),
            )
                .into_response();
            if let Some(h) = headers {
                resp.headers_mut().extend(h);
            }
            if let Ok(v) = HeaderValue::from_str(&wait_time.to_string()) {
                resp.headers_mut().insert(header::RETRY_AFTER, v);
            }
            resp
        }
        GovernorError::UnableToExtractKey => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "request inválido" })),
        )
            .into_response(),
        GovernorError::Other { code, msg, .. } => {
            (code, Json(json!({ "error": msg.unwrap_or_default() }))).into_response()
        }
    }
}

/// Encabezados para toda respuesta de la API: datos de negocio, nunca cacheados
/// ni interpretados como otro tipo de contenido.
pub fn header_layers() -> (
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
) {
    (
        SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ),
        SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(xff: &[&str]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for v in xff {
            m.append("x-forwarded-for", HeaderValue::from_str(v).unwrap());
        }
        m
    }

    #[test]
    fn ignores_forwarded_for_unless_trusted() {
        let peer: IpAddr = "10.0.0.5".parse().unwrap();
        let spoof = h(&["1.2.3.4"]);
        assert_eq!(client_ip(&spoof, Some(peer), false), Some(peer));
    }

    #[test]
    fn trusted_proxy_uses_last_hop() {
        let peer: IpAddr = "10.0.0.5".parse().unwrap();
        // El cliente inventa 1.2.3.4; nuestro proxy agrega la real al final.
        let m = h(&["1.2.3.4, 181.50.20.7"]);
        assert_eq!(
            client_ip(&m, Some(peer), true),
            Some("181.50.20.7".parse().unwrap())
        );
        let m = h(&["1.2.3.4", "181.50.20.7"]);
        assert_eq!(
            client_ip(&m, Some(peer), true),
            Some("181.50.20.7".parse().unwrap())
        );
        assert_eq!(client_ip(&h(&["basura"]), Some(peer), true), Some(peer));
    }

    #[test]
    fn key_uses_token_digest_not_token() {
        let req = Request::builder()
            .header("authorization", "Bearer abc.def.ghi")
            .body(())
            .unwrap();
        let k = ClientKey.extract(&req).unwrap();
        assert!(k.starts_with("t:") && !k.contains("abc"));
        let req = Request::builder().body(()).unwrap();
        assert_eq!(ClientKey.extract(&req).unwrap(), "ip:desconocida");
    }
}

#[cfg(test)]
mod db_tests {
    use crate::test_support::*;

    /// Los fallos de un atacante desde su IP no bloquean al dueño en la suya.
    #[tokio::test]
    async fn login_lockout_is_per_email_and_ip() {
        let Some(app) = app_pool().await else {
            eprintln!("sin DATABASE_URL: se salta");
            return;
        };
        let email = format!("{}@bloqueo.test", uuid::Uuid::new_v4());
        for _ in 0..4 {
            crate::auth::check_and_record_login_attempt(
                &app,
                &email,
                Some("203.0.113.9"),
                None,
                Some(false),
                Some("x"),
            )
            .await
            .unwrap();
        }
        let blocked = crate::auth::check_and_record_login_attempt(
            &app,
            &email,
            Some("203.0.113.9"),
            None,
            Some(false),
            Some("x"),
        )
        .await;
        assert!(blocked.is_err(), "el atacante queda bloqueado en su IP");
        let owner = crate::auth::check_and_record_login_attempt(
            &app,
            &email,
            Some("181.50.20.7"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(owner.unwrap(), 0, "el dueño entra desde la suya");
    }
}

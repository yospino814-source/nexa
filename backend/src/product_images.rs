//! Foto de los productos.
//!
//! El navegador la reduce y comprime antes de subirla; acá se valida el tipo
//! por sus primeros bytes (no por lo que diga el cliente) y el tamaño, y se
//! guarda en `product_images`. La lectura es pública por id para que la use
//! una etiqueta `<img>`, con caché larga: la URL lleva `?v=` con la fecha de
//! la última foto, así un cambio de foto es otra URL.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{self, ApiError, DomainError};
use crate::routes::{Body, Path};
use crate::tenancy;

/// Máximo guardado (la columna tiene el mismo límite).
pub const MAX_BYTES: usize = 512 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/products/{id}/image",
            put(put_image).delete(delete_image),
        )
        .route("/v1/public/product-images/{id}", get(public_image))
}

/// Tipo real de la imagen según su firma.
pub fn sniff(data: &[u8]) -> Option<&'static str> {
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

/// Decodifica y valida la foto que manda el navegador.
pub fn decode(data_base64: &str) -> Result<(Vec<u8>, &'static str), ApiError> {
    // Acepta también un data URI completo ("data:image/webp;base64,...").
    let raw = data_base64
        .split_once(";base64,")
        .map(|(_, b)| b)
        .unwrap_or(data_base64)
        .trim();
    if raw.len() > MAX_BYTES * 4 / 3 + 8 {
        return Err(error::validation("la foto pesa más de 512 KB"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|_| error::validation("la foto no se pudo leer"))?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(error::validation("la foto pesa más de 512 KB"));
    }
    let mime =
        sniff(&bytes).ok_or_else(|| error::validation("la foto debe ser WebP, JPEG o PNG"))?;
    Ok((bytes, mime))
}

#[derive(Deserialize)]
struct ImageBody {
    /// Imagen en base64 (o data URI).
    data: String,
}

async fn put_image(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(b): Body<ImageBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (bytes, mime) = decode(&b.data)?;
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    scope.require_permission("inventory:adjust")?;
    let exists: bool = sqlx::query_scalar("select exists(select 1 from products where id = $1)")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }
    let version: i64 = sqlx::query_scalar(
        "insert into product_images (product_id, tenant_id, mime, data) values ($1, $2, $3, $4)
         on conflict (product_id) do update set mime = excluded.mime, data = excluded.data, updated_at = now()
         returning (extract(epoch from updated_at) * 1000)::bigint",
    )
    .bind(id)
    .bind(scope.tenant)
    .bind(mime)
    .bind(&bytes)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "image_version": version })))
}

async fn delete_image(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    scope.require_permission("inventory:adjust")?;
    sqlx::query("delete from product_images where product_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn public_image(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let row: Result<Option<(String, Vec<u8>)>, sqlx::Error> =
        sqlx::query_as("select mime, data from private.product_image($1)")
            .bind(id)
            .fetch_optional(&state.pool)
            .await;
    match row {
        Ok(Some((mime, data))) => {
            let mut res = data.into_response();
            let h = res.headers_mut();
            if let Ok(v) = HeaderValue::from_str(&mime) {
                h.insert(header::CONTENT_TYPE, v);
            }
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            h.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            res
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "no se pudo leer la foto del producto");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_reconoce_los_tipos() {
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            Some("image/png")
        );
        assert_eq!(sniff(b"<svg onload=alert(1)>"), None);
        assert_eq!(sniff(b"GIF89a"), None);
    }

    #[test]
    fn decode_acepta_data_uri_y_rechaza_lo_demas() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        assert_eq!(decode(&b64).unwrap().1, "image/png");
        assert_eq!(
            decode(&format!("data:image/png;base64,{b64}")).unwrap().1,
            "image/png"
        );
        assert!(decode("no es base64!!").is_err());
        let svg = base64::engine::general_purpose::STANDARD.encode(b"<svg></svg>");
        assert!(decode(&svg).is_err());
        let big = base64::engine::general_purpose::STANDARD.encode(vec![0xFFu8; MAX_BYTES + 1]);
        assert!(decode(&big).is_err());
    }
}

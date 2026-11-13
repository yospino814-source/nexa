//! Outbox transaccional para efectos externos (F0.7).
//!
//! Encola eventos (correo, WhatsApp, push, DIAN) dentro de la misma transacción
//! para que un worker asíncrono los procese con reintentos.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::ApiError;

type Tx<'a> = Transaction<'a, Postgres>;

/// Encola un efecto externo para ser enviado por el worker de outbox.
pub async fn enqueue_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    kind: &str,
    payload: serde_json::Value,
) -> Result<Uuid, ApiError> {
    let id: Uuid = sqlx::query_scalar(
        "insert into outbox (tenant_id, kind, payload, status, attempts, next_attempt_at, created_at)
         values ($1, $2, $3, 'PENDIENTE', 0, now(), now())
         returning id",
    )
    .bind(tenant_id)
    .bind(kind)
    .bind(payload)
    .fetch_one(&mut **tx)
    .await?;

    Ok(id)
}

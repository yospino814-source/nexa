//! Numeración atómica unificada de documentos por sucursal (F0.6).
//!
//! Garantiza asignación secuencial sin huecos ni colisiones concurrentes
//! mediante `UPDATE ... RETURNING next_number`.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::{ApiError, state};
use crate::tenancy::Scope;

type Tx<'a> = Transaction<'a, Postgres>;

/// Prefijo por defecto para cada tipo de documento soportado.
pub fn default_prefix_for(doc_type: &str) -> &'static str {
    match doc_type {
        "POS" | "VENTA" => "POS",
        "DEVOLUCION" => "DEV",
        "COMPRA" => "COM",
        "SEPARADO" => "SEP",
        "COTIZACION" => "COT",
        "ACUERDO_PAGO" => "ACP",
        "TRASLADO" => "TRA",
        "GASTO" => "GAS",
        "RUTA" => "RUT",
        _ => "DOC",
    }
}

/// Obtiene el siguiente consecutivo atómico para el tipo de documento en la sucursal.
///
/// Si no existe fila en `document_sequences`, la inicializa con el prefijo por defecto
/// y `next_number = 2`, retornando `(prefix, 1)`.
pub async fn next_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    doc_type: &str,
) -> Result<(String, i64), ApiError> {
    let default_prefix = default_prefix_for(doc_type);

    // Asegurar que exista la fila de la secuencia
    sqlx::query(
        "insert into document_sequences (tenant_id, branch_id, doc_type, prefix, next_number)
         values ($1, $2, $3, $4, 1)
         on conflict (branch_id, doc_type) do nothing",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(doc_type)
    .bind(default_prefix)
    .execute(&mut **tx)
    .await?;

    // Bloquear fila y avanzar el contador
    let row: Option<(String, i64, Option<i64>, bool)> = sqlx::query_as(
        "update document_sequences
            set next_number = next_number + 1
          where branch_id = $1 and doc_type = $2
          returning prefix, next_number - 1, range_to, coalesce(valid_until < current_date, false)",
    )
    .bind(branch_id)
    .bind(doc_type)
    .fetch_optional(&mut **tx)
    .await?;

    let (prefix, number, range_to, expired) =
        row.ok_or_else(|| state(format!("sequence for {doc_type} not found")))?;

    if expired {
        return Err(state("receipt numbering resolution has expired"));
    }

    if let Some(limit) = range_to
        && number > limit
    {
        return Err(state(format!(
            "authorized numbering range exceeded for {doc_type} (limit: {limit})"
        )));
    }

    Ok((prefix, number))
}

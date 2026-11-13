use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;

/// Crea el pool de conexiones forzando cada conexion a ejecutar como
/// `authenticated`, de modo que RLS aplique (nunca como dueño, que la salta).
pub async fn connect(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("set role authenticated")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Abre una transaccion con el contexto de tenant/sucursal/usuario para RLS.
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn begin_tenant_tx<'a>(
    pool: &'a PgPool,
    claims: &Claims,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    begin_tenant_tx_for(pool, claims, claims.tenant_id()).await
}

/// Como `begin_tenant_tx` pero con un tenant explicito (el resuelto por membresia).
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn begin_tenant_tx_for<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    tenant: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    begin_scope_tx(pool, claims, tenant, claims.branch_id()).await
}

/// Abre una transaccion fijando rol, claims JWT y el contexto tenant+sucursal.
pub async fn begin_scope_tx<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    tenant: Option<Uuid>,
    branch: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("set local role authenticated")
        .execute(&mut *tx)
        .await?;

    let jwt_claims = serde_json::json!({
        "sub": claims.sub,
        "app_metadata": claims.app_metadata,
    })
    .to_string();
    // Un solo viaje a la BD; '' equivale a "sin contexto" (fail-closed en RLS).
    sqlx::query(
        "select set_config('request.jwt.claims', $1, true),
                set_config('app.current_tenant_id', $2, true),
                set_config('app.current_branch_id', $3, true)",
    )
    .bind(jwt_claims)
    .bind(tenant.map(|t| t.to_string()).unwrap_or_default())
    .bind(branch.map(|b| b.to_string()).unwrap_or_default())
    .execute(&mut *tx)
    .await?;

    Ok(tx)
}

/// Fija tenant y sucursal de una transaccion ya abierta (tras resolver la membresia).
/// `branch = None` deja ver todas las sucursales del tenant.
pub async fn set_scope(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    branch: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "select set_config('app.current_tenant_id', $1, true),
                set_config('app.current_branch_id', $2, true)",
    )
    .bind(tenant.to_string())
    .bind(branch.map(|b| b.to_string()).unwrap_or_default())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

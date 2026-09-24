use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;

/// Crea el pool de conexiones forzando cada conexion a ejecutar como
/// `authenticated`, de modo que RLS aplique (nunca como dueño, que la salta).
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
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

/// Abre una transaccion con el contexto de tenant/usuario fijado para RLS.
pub async fn begin_tenant_tx<'a>(
    pool: &'a PgPool,
    claims: &Claims,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    begin_tenant_tx_for(pool, claims, claims.tenant_id()).await
}

/// Como `begin_tenant_tx` pero con un tenant explicito (el resuelto por membresia).
pub async fn begin_tenant_tx_for<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    tenant: Option<Uuid>,
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
    sqlx::query("select set_config('request.jwt.claims', $1, true)")
        .bind(jwt_claims)
        .execute(&mut *tx)
        .await?;

    if let Some(tenant) = tenant {
        sqlx::query("select set_config('app.current_tenant_id', $1, true)")
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await?;
    }

    Ok(tx)
}

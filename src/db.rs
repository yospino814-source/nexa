use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};

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

    if let Some(tenant) = claims.tenant_id() {
        sqlx::query("select set_config('app.current_tenant_id', $1, true)")
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await?;
    }

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn claims(uid: Uuid, tenant: Uuid) -> Claims {
        Claims {
            sub: uid,
            app_metadata: serde_json::json!({ "tenant_id": tenant.to_string() }),
        }
    }

    /// Verifica contra la BD real que el contexto fija auth.uid() y el tenant.
    /// Se omite si no hay DATABASE_URL.
    #[tokio::test]
    async fn tenant_context_sets_uid_and_tenant() {
        dotenvy::dotenv().ok();
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };

        let pool = connect(&url).await.expect("connect");
        let uid = Uuid::new_v4();
        let tenant = Uuid::new_v4();

        let mut tx = begin_tenant_tx(&pool, &claims(uid, tenant))
            .await
            .expect("begin tx");

        let got_uid: Option<Uuid> = sqlx::query_scalar("select auth.uid()")
            .fetch_one(&mut *tx)
            .await
            .expect("auth.uid()");
        let got_tenant: Option<Uuid> = sqlx::query_scalar("select public.current_tenant_id()")
            .fetch_one(&mut *tx)
            .await
            .expect("current_tenant_id()");
        tx.rollback().await.expect("rollback");

        assert_eq!(got_uid, Some(uid));
        assert_eq!(got_tenant, Some(tenant));
    }
}

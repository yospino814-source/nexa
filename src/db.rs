use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Crea el pool de conexiones y fuerza cada conexion a ejecutar como
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

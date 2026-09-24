//! Tests de aislamiento por tenant (RLS) y de integridad referencial.

use crate::test_support::*;

fn database_url() -> Option<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL").ok()
}

async fn count_categories(pool: &sqlx::PgPool, claims: &crate::auth::Claims) -> i64 {
    let mut tx = crate::db::begin_tenant_tx(pool, claims).await.unwrap();
    let n: i64 = sqlx::query_scalar("select count(*) from categories")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    n
}

#[tokio::test]
async fn rls_isolates_tenants_and_fails_closed() {
    let Some(admin) = admin_pool().await else {
        return;
    };
    let url = database_url().unwrap();
    let app = crate::db::connect(&url).await.unwrap();

    let tenant_a = seed_tenant(&admin).await;
    let tenant_b = seed_tenant(&admin).await;
    seed_category(&admin, tenant_a).await;
    let cat_b = seed_category(&admin, tenant_b).await;

    let user = seed_user(&admin).await;
    seed_membership(&admin, tenant_a, user, "admin_caja").await;

    // Con contexto de A solo se ve lo de A.
    let ctx_a = claims(user, Some(tenant_a));
    assert_eq!(count_categories(&app, &ctx_a).await, 1);

    // Sin tenant -> fail-closed.
    let ctx_none = claims(user, None);
    assert_eq!(count_categories(&app, &ctx_none).await, 0);

    // No puede modificar filas de otro tenant.
    let mut tx = crate::db::begin_tenant_tx(&app, &ctx_a).await.unwrap();
    let affected = sqlx::query("update categories set name = 'hacked' where id = $1")
        .bind(cat_b)
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    tx.commit().await.unwrap();
    assert_eq!(affected, 0);

    // La fila de B sigue intacta (vista por el admin).
    let name: String = sqlx::query_scalar("select name from categories where id = $1")
        .bind(cat_b)
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(name, "cat");

    cleanup(&admin, tenant_a, &[user]).await;
    cleanup(&admin, tenant_b, &[]).await;
}

#[tokio::test]
async fn composite_fk_blocks_cross_tenant_reference() {
    let Some(admin) = admin_pool().await else {
        return;
    };

    let tenant_a = seed_tenant(&admin).await;
    let tenant_b = seed_tenant(&admin).await;
    let cat_b = seed_category(&admin, tenant_b).await;

    // El admin salta RLS, asi que solo puede rechazarlo la FK compuesta por tenant.
    let result = sqlx::query(
        "insert into products (tenant_id, category_id, name) values ($1, $2, 'cross')",
    )
    .bind(tenant_a)
    .bind(cat_b)
    .execute(&admin)
    .await;

    assert!(result.is_err(), "cross-tenant FK should be rejected");

    cleanup(&admin, tenant_a, &[]).await;
    cleanup(&admin, tenant_b, &[]).await;
}

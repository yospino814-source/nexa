//! Utilidades de apoyo para los tests de integracion contra la BD real.
//! Solo se compila en tests.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;

/// Pool con rol administrador (dueño): salta RLS. Solo para sembrar y limpiar.
pub async fn admin_pool() -> Option<PgPool> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").ok()?;
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .ok()
}

pub async fn seed_tenant(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("insert into tenants (name) values ('itest') returning id")
        .fetch_one(pool)
        .await
        .expect("seed tenant")
}

pub async fn seed_location(pool: &PgPool, tenant: Uuid) -> Uuid {
    sqlx::query_scalar("insert into locations (tenant_id, name) values ($1, 'loc') returning id")
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("seed location")
}

pub async fn seed_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("insert into auth.users (id, email) values ($1, $2)")
        .bind(id)
        .bind(format!("{id}@itest.local"))
        .execute(pool)
        .await
        .expect("seed user");
    id
}

pub async fn seed_membership(pool: &PgPool, tenant: Uuid, user: Uuid, role: &str) {
    sqlx::query("insert into memberships (tenant_id, user_id, role) values ($1, $2, $3)")
        .bind(tenant)
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .expect("seed membership");
}

pub async fn seed_category(pool: &PgPool, tenant: Uuid) -> Uuid {
    sqlx::query_scalar("insert into categories (tenant_id, name) values ($1, 'cat') returning id")
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("seed category")
}

pub async fn cleanup(pool: &PgPool, tenant: Uuid, users: &[Uuid]) {
    let _ = sqlx::query("delete from tenants where id = $1")
        .bind(tenant)
        .execute(pool)
        .await;
    for user in users {
        let _ = sqlx::query("delete from auth.users where id = $1")
            .bind(user)
            .execute(pool)
            .await;
    }
}

pub fn claims(user: Uuid, tenant: Option<Uuid>) -> Claims {
    Claims {
        sub: user,
        app_metadata: serde_json::json!({
            "tenant_id": tenant.map(|t| t.to_string()),
        }),
    }
}

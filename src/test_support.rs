//! Utilidades de apoyo para los tests de integracion contra la BD real.
//! Solo se compila en tests.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;

fn database_url() -> Option<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL").ok()
}

/// Pool con rol administrador (dueño): salta RLS. Solo para sembrar y limpiar.
pub async fn admin_pool() -> Option<PgPool> {
    let url = database_url()?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .ok()
}

/// Pool de la aplicacion (rol `authenticated`).
pub async fn app_pool() -> Option<PgPool> {
    let url = database_url()?;
    crate::db::connect(&url).await.ok()
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

pub async fn seed_tax(pool: &PgPool, code: &str, rate: &str) -> Uuid {
    sqlx::query_scalar(
        "insert into taxes (code, name, rate) values ($1, $1, $2::numeric) returning id",
    )
    .bind(code)
    .bind(rate)
    .fetch_one(pool)
    .await
    .expect("seed tax")
}

pub async fn seed_product(
    pool: &PgPool,
    tenant: Uuid,
    name: &str,
    category: Option<Uuid>,
    product_type: &str,
    tax: Option<Uuid>,
    tracks_inventory: bool,
) -> Uuid {
    sqlx::query_scalar(
        "insert into products (tenant_id, name, category_id, product_type, tax_id, tracks_inventory)
         values ($1, $2, $3, $4::product_type, $5, $6) returning id",
    )
    .bind(tenant)
    .bind(name)
    .bind(category)
    .bind(product_type)
    .bind(tax)
    .bind(tracks_inventory)
    .fetch_one(pool)
    .await
    .expect("seed product")
}

pub async fn seed_product_price(
    pool: &PgPool,
    tenant: Uuid,
    product: Uuid,
    location: Option<Uuid>,
    price: &str,
) {
    sqlx::query(
        "insert into product_prices (tenant_id, product_id, location_id, price)
         values ($1, $2, $3, $4::numeric)",
    )
    .bind(tenant)
    .bind(product)
    .bind(location)
    .bind(price)
    .execute(pool)
    .await
    .expect("seed product price");
}

pub async fn seed_stock(
    pool: &PgPool,
    tenant: Uuid,
    location: Uuid,
    product: Uuid,
    on_hand: &str,
    reserved: &str,
) {
    sqlx::query(
        "insert into stock (tenant_id, location_id, product_id, on_hand, reserved)
         values ($1, $2, $3, $4::numeric, $5::numeric)",
    )
    .bind(tenant)
    .bind(location)
    .bind(product)
    .bind(on_hand)
    .bind(reserved)
    .execute(pool)
    .await
    .expect("seed stock");
}

pub async fn seed_recipe(pool: &PgPool, tenant: Uuid, product: Uuid) -> Uuid {
    sqlx::query_scalar("insert into recipes (tenant_id, product_id) values ($1, $2) returning id")
        .bind(tenant)
        .bind(product)
        .fetch_one(pool)
        .await
        .expect("seed recipe")
}

pub async fn seed_recipe_item(pool: &PgPool, tenant: Uuid, recipe: Uuid, input: Uuid, qty: &str) {
    sqlx::query(
        "insert into recipe_items (tenant_id, recipe_id, input_product_id, qty)
         values ($1, $2, $3, $4::numeric)",
    )
    .bind(tenant)
    .bind(recipe)
    .bind(input)
    .bind(qty)
    .execute(pool)
    .await
    .expect("seed recipe item");
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

pub async fn cleanup_tax(pool: &PgPool, tax: Uuid) {
    let _ = sqlx::query("delete from taxes where id = $1")
        .bind(tax)
        .execute(pool)
        .await;
}

pub fn claims(user: Uuid, tenant: Option<Uuid>) -> Claims {
    Claims {
        sub: user,
        app_metadata: serde_json::json!({
            "tenant_id": tenant.map(|t| t.to_string()),
        }),
    }
}

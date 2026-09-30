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

pub async fn seed_branch(pool: &PgPool, tenant: Uuid) -> Uuid {
    sqlx::query_scalar("insert into branches (tenant_id, name) values ($1, 'suc') returning id")
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("seed branch")
}

pub async fn seed_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("insert into auth.users (id, email) values ($1, $2)")
        .bind(id)
        .bind(format!("{id}@itest.local"))
        .execute(pool)
        .await
        .expect("seed auth user");
    sqlx::query("insert into users (id, full_name) values ($1, 'Test')")
        .bind(id)
        .execute(pool)
        .await
        .expect("seed user profile");
    id
}

pub async fn seed_membership(pool: &PgPool, tenant: Uuid, branch: Option<Uuid>, user: Uuid, role: &str) {
    sqlx::query(
        "insert into memberships (tenant_id, branch_id, user_id, role_id)
         values ($1, $2, $3, public.tipo_id('ROL', $4))",
    )
    .bind(tenant)
    .bind(branch)
    .bind(user)
    .bind(role)
    .execute(pool)
    .await
    .expect("seed membership");
}

pub async fn seed_table(pool: &PgPool, tenant: Uuid, branch: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "insert into restaurant_tables (tenant_id, branch_id, code, status_id)
         values ($1, $2, $3, public.tipo_id('MESA', 'LIBRE')) returning id",
    )
    .bind(tenant)
    .bind(branch)
    .bind(code)
    .fetch_one(pool)
    .await
    .expect("seed table")
}

pub async fn seed_category(pool: &PgPool, tenant: Uuid, branch: Uuid) -> Uuid {
    sqlx::query_scalar(
        "insert into product_categories (tenant_id, branch_id, name) values ($1, $2, 'cat') returning id",
    )
    .bind(tenant)
    .bind(branch)
    .fetch_one(pool)
    .await
    .expect("seed category")
}

pub async fn seed_product(
    pool: &PgPool,
    tenant: Uuid,
    name: &str,
    uom: &str,
    tax_code: Option<&str>,
    is_composed: bool,
) -> Uuid {
    sqlx::query_scalar(
        "insert into products (tenant_id, name, unit_measure_id, tax_type_id, is_composed)
         values ($1, $2, public.tipo_id('UOM', $3), public.tipo_id('IMP', $4), $5) returning id",
    )
    .bind(tenant)
    .bind(name)
    .bind(uom)
    .bind(tax_code)
    .bind(is_composed)
    .fetch_one(pool)
    .await
    .expect("seed product")
}

pub async fn set_base_price(pool: &PgPool, product: Uuid, price: &str) {
    sqlx::query("update products set base_price = $2::numeric where id = $1")
        .bind(product)
        .bind(price)
        .execute(pool)
        .await
        .expect("set base price");
}

pub async fn seed_branch_product(
    pool: &PgPool,
    tenant: Uuid,
    branch: Uuid,
    product: Uuid,
    category: Option<Uuid>,
    price: Option<&str>,
) {
    sqlx::query(
        "insert into branch_products (tenant_id, branch_id, product_id, category_id, price)
         values ($1, $2, $3, $4, $5::numeric)",
    )
    .bind(tenant)
    .bind(branch)
    .bind(product)
    .bind(category)
    .bind(price)
    .execute(pool)
    .await
    .expect("seed branch product");
}

pub async fn seed_stock(
    pool: &PgPool,
    tenant: Uuid,
    branch: Uuid,
    product: Uuid,
    on_hand: &str,
    reserved: &str,
) {
    sqlx::query(
        "insert into stock (tenant_id, branch_id, product_id, on_hand, reserved)
         values ($1, $2, $3, $4::numeric, $5::numeric)",
    )
    .bind(tenant)
    .bind(branch)
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

pub fn claims(user: Uuid, tenant: Option<Uuid>, branch: Option<Uuid>) -> Claims {
    Claims {
        sub: user,
        app_metadata: serde_json::json!({
            "tenant_id": tenant.map(|t| t.to_string()),
            "branch_id": branch.map(|b| b.to_string()),
        }),
    }
}

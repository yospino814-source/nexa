//! Pruebas de integracion contra la BD real (RLS, tenancy, catalogo).
//!
//! Se ejecutan en un unico test para compartir un runtime y los pools de
//! conexion (Supabase limita clientes en modo sesion).

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::orders::OrderStatus;
use crate::test_support::*;
use crate::{catalog, inventory, orders, tenancy};

fn dec(n: i64) -> Decimal {
    Decimal::from(n)
}

async fn count_categories(pool: &PgPool, claims: &crate::auth::Claims) -> i64 {
    let mut tx = crate::db::begin_tenant_tx(pool, claims).await.unwrap();
    let n: i64 = sqlx::query_scalar("select count(*) from categories")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    n
}

async fn context_sets_uid_and_tenant(app: &PgPool) {
    let uid = Uuid::new_v4();
    let tenant = Uuid::new_v4();
    let c = claims(uid, Some(tenant));

    let mut tx = crate::db::begin_tenant_tx(app, &c).await.unwrap();
    let got_uid: Option<Uuid> = sqlx::query_scalar("select auth.uid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let got_tenant: Option<Uuid> = sqlx::query_scalar("select public.current_tenant_id()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(got_uid, Some(uid));
    assert_eq!(got_tenant, Some(tenant));
}

async fn resolve_membership_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let other = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "mesero").await;

    let c = claims(user, Some(tenant));
    assert_eq!(
        tenancy::resolve_membership(app, &c, None).await.unwrap(),
        (tenant, "mesero".to_string())
    );

    let c_other = claims(user, Some(other));
    assert!(tenancy::resolve_membership(app, &c_other, None).await.is_err());

    let c_none = claims(user, None);
    assert_eq!(
        tenancy::resolve_membership(app, &c_none, None).await.unwrap(),
        (tenant, "mesero".to_string())
    );

    cleanup(admin, tenant, &[user]).await;
    cleanup(admin, other, &[]).await;
}

async fn ensure_location_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let other = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "admin_caja").await;

    let loc = seed_location(admin, tenant).await;
    let foreign = seed_location(admin, other).await;

    let c = claims(user, Some(tenant));
    assert!(tenancy::ensure_location_in_tenant(app, &c, tenant, loc)
        .await
        .is_ok());
    assert!(
        tenancy::ensure_location_in_tenant(app, &c, tenant, foreign)
            .await
            .is_err()
    );

    cleanup(admin, tenant, &[user]).await;
    cleanup(admin, other, &[]).await;
}

async fn rls_isolation_checks(app: &PgPool, admin: &PgPool) {
    let tenant_a = seed_tenant(admin).await;
    let tenant_b = seed_tenant(admin).await;
    seed_category(admin, tenant_a).await;
    let cat_b = seed_category(admin, tenant_b).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant_a, user, "admin_caja").await;

    // Con contexto de A solo se ve lo de A.
    let ctx_a = claims(user, Some(tenant_a));
    assert_eq!(count_categories(app, &ctx_a).await, 1);

    // Sin tenant -> fail-closed.
    let ctx_none = claims(user, None);
    assert_eq!(count_categories(app, &ctx_none).await, 0);

    // No puede modificar filas de otro tenant.
    let mut tx = crate::db::begin_tenant_tx(app, &ctx_a).await.unwrap();
    let affected = sqlx::query("update categories set name = 'hacked' where id = $1")
        .bind(cat_b)
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    tx.commit().await.unwrap();
    assert_eq!(affected, 0);

    let name: String = sqlx::query_scalar("select name from categories where id = $1")
        .bind(cat_b)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(name, "cat");

    cleanup(admin, tenant_a, &[user]).await;
    cleanup(admin, tenant_b, &[]).await;
}

async fn composite_fk_check(admin: &PgPool) {
    let tenant_a = seed_tenant(admin).await;
    let tenant_b = seed_tenant(admin).await;
    let cat_b = seed_category(admin, tenant_b).await;

    // El admin salta RLS, asi que solo puede rechazarlo la FK compuesta por tenant.
    let result = sqlx::query(
        "insert into products (tenant_id, category_id, name) values ($1, $2, 'cross')",
    )
    .bind(tenant_a)
    .bind(cat_b)
    .execute(admin)
    .await;
    assert!(result.is_err(), "cross-tenant FK should be rejected");

    cleanup(admin, tenant_a, &[]).await;
    cleanup(admin, tenant_b, &[]).await;
}

async fn catalog_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let loc_a = seed_location(admin, tenant).await;
    let loc_b = seed_location(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "admin_caja").await;

    let code = format!("INC{}", &Uuid::new_v4().simple().to_string()[..8]);
    let tax = seed_tax(admin, &code, "0.08").await;
    let cat = seed_category(admin, tenant).await;

    let active =
        seed_product(admin, tenant, "Limonada", Some(cat), "sellable", Some(tax), true).await;
    let inactive = seed_product(admin, tenant, "Viejo", None, "sellable", None, true).await;
    sqlx::query("update products set active = false where id = $1")
        .bind(inactive)
        .execute(admin)
        .await
        .unwrap();
    let supply = seed_product(admin, tenant, "Azucar", None, "supply", None, true).await;

    seed_product_price(admin, tenant, active, None, "10000").await;
    seed_product_price(admin, tenant, active, Some(loc_a), "12000").await;
    seed_stock(admin, tenant, loc_a, active, "5", "0").await;

    // Producto compuesto: arroz compuesto por insumo, 0.2 por unidad.
    let dish = seed_product(admin, tenant, "Arroz", Some(cat), "sellable", None, false).await;
    let insumo = seed_product(admin, tenant, "Arroz grano", None, "supply", None, true).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "0.2").await;
    seed_product_price(admin, tenant, dish, None, "5000").await;
    seed_stock(admin, tenant, loc_a, insumo, "3", "0").await;

    let c = claims(user, Some(tenant));

    // Sucursal A: override de precio, impuesto, disponibilidad propia y derivada.
    let items_a = catalog::catalog(app, &c, loc_a).await.unwrap();
    let limonada = items_a.iter().find(|i| i.product_id == active).unwrap();
    assert_eq!(limonada.price.unwrap().normalize(), Decimal::from(12000));
    assert_eq!(limonada.tax_code.as_deref(), Some(code.as_str()));
    assert_eq!(limonada.tax_rate.unwrap().normalize(), Decimal::new(8, 2));
    assert_eq!(limonada.available.unwrap().normalize(), Decimal::from(5));
    assert_eq!(limonada.category.as_deref(), Some("cat"));

    let arroz = items_a.iter().find(|i| i.product_id == dish).unwrap();
    assert_eq!(arroz.available.unwrap().normalize(), Decimal::from(15)); // floor(3 / 0.2)

    assert!(items_a.iter().all(|i| i.product_id != inactive));
    assert!(items_a.iter().all(|i| i.product_id != supply));
    assert_eq!(items_a.iter().filter(|i| i.product_id == active).count(), 1);

    // Sucursal B: precio default y sin stock -> 0 disponible.
    let items_b = catalog::catalog(app, &c, loc_b).await.unwrap();
    let limonada_b = items_b.iter().find(|i| i.product_id == active).unwrap();
    assert_eq!(limonada_b.price.unwrap().normalize(), Decimal::from(10000));
    assert_eq!(limonada_b.available.unwrap().normalize(), Decimal::ZERO);
    let arroz_b = items_b.iter().find(|i| i.product_id == dish).unwrap();
    assert_eq!(arroz_b.available.unwrap().normalize(), Decimal::ZERO);

    cleanup(admin, tenant, &[user]).await;
    cleanup_tax(admin, tax).await;
}

async fn recipe_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "admin_caja").await;

    let dish = seed_product(admin, tenant, "Plato", None, "sellable", None, false).await;
    let insumo = seed_product(admin, tenant, "Insumo", None, "supply", None, true).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "0.25").await;

    let c = claims(user, Some(tenant));
    let items = catalog::recipe(app, &c, dish).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].input_product_id, insumo);
    assert_eq!(items[0].qty.normalize(), Decimal::new(25, 2));

    // Producto visible pero sin receta -> lista vacia.
    let plain = seed_product(admin, tenant, "Suelto", None, "sellable", None, true).await;
    assert!(catalog::recipe(app, &c, plain).await.unwrap().is_empty());

    cleanup(admin, tenant, &[user]).await;
}

async fn inventory_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let loc = seed_location(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "admin_caja").await;
    let c = claims(user, Some(tenant));

    let p = seed_product(admin, tenant, "Pan", None, "sellable", None, true).await;
    seed_stock(admin, tenant, loc, p, "5", "0").await;

    // available = on_hand - reserved
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(5)
    );

    // Reserva con disponibilidad suficiente.
    let ids = inventory::reserve(app, &c, loc, None, None, p, dec(3), None)
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    // Reserva sin disponibilidad suficiente (solo quedan 2).
    assert!(
        inventory::reserve(app, &c, loc, None, None, p, dec(3), None)
            .await
            .is_err()
    );

    // Sobreventa bajo reservas concurrentes: exactamente una gana.
    let p2 = seed_product(admin, tenant, "Agua", None, "sellable", None, true).await;
    seed_stock(admin, tenant, loc, p2, "1", "0").await;
    let (r1, r2) = tokio::join!(
        inventory::reserve(app, &c, loc, None, None, p2, dec(1), None),
        inventory::reserve(app, &c, loc, None, None, p2, dec(1), None),
    );
    assert!(r1.is_ok() ^ r2.is_ok(), "exactly one concurrent reserve wins");

    // Consumacion: descuento definitivo + movimiento.
    assert_eq!(inventory::consume(app, &c, &ids).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );
    let mv = inventory::movements(app, &c, loc).await.unwrap();
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "descuento" && m.qty.normalize() == dec(3)));

    // Liberacion explicita.
    let r = inventory::reserve(app, &c, loc, None, None, p, dec(1), None)
        .await
        .unwrap();
    assert_eq!(inventory::release(app, &c, &r).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    // Expiracion por barrido.
    let past = Utc::now() - Duration::minutes(5);
    inventory::reserve(app, &c, loc, None, None, p, dec(1), Some(past))
        .await
        .unwrap();
    assert!(inventory::expire_due(app, &c).await.unwrap() >= 1);
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    // Reposicion y ajuste con bitacora.
    inventory::replenish(app, &c, loc, p, dec(10), "compra")
        .await
        .unwrap();
    inventory::adjust(app, &c, loc, p, dec(-2), "merma")
        .await
        .unwrap();
    let mv = inventory::movements(app, &c, loc).await.unwrap();
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "reposicion" && m.qty.normalize() == dec(10)));
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "ajuste" && m.qty.normalize() == dec(2)));
    // on_hand: 5 - 3 (consumo) = 2; +10 = 12; -2 = 10.
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(10)
    );

    // Reserva por insumo (explosion de receta).
    let dish = seed_product(admin, tenant, "Torta", None, "sellable", None, false).await;
    let insumo = seed_product(admin, tenant, "Harina", None, "supply", None, true).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "2").await;
    seed_stock(admin, tenant, loc, insumo, "4", "0").await;

    let ids2 = inventory::reserve(app, &c, loc, None, None, dish, dec(1), None)
        .await
        .unwrap();
    assert_eq!(ids2.len(), 1);
    // Insumo disponible 2, 2 por unidad -> 1.
    assert_eq!(
        inventory::available(app, &c, loc, dish).await.unwrap().unwrap().normalize(),
        dec(1)
    );

    cleanup(admin, tenant, &[user]).await;
}

async fn order_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let loc = seed_location(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, user, "mesero").await;
    let c = claims(user, Some(tenant));

    let table = seed_table(admin, tenant, loc, "T1").await;
    let p = seed_product(admin, tenant, "Plato", None, "sellable", None, true).await;
    seed_product_price(admin, tenant, p, None, "100").await;
    seed_stock(admin, tenant, loc, p, "10", "0").await;

    // 5.2 dine_in con mesa (se ocupa).
    let o1 = orders::create_order(app, &c, loc, "dine_in", Some(table), None)
        .await
        .unwrap();
    assert_eq!(o1.status, "creada");
    let t_status: String =
        sqlx::query_scalar("select status::text from tables where id = $1")
            .bind(table)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(t_status, "ocupada");
    assert!(orders::create_order(app, &c, loc, "dine_in", Some(table), None)
        .await
        .is_err());

    // 5.2 pos_counter sin mesa; dine_in sin mesa rechazado.
    let o2 = orders::create_order(app, &c, loc, "pos_counter", None, None)
        .await
        .unwrap();
    assert_eq!(o2.status, "creada");
    assert!(orders::create_order(app, &c, loc, "dine_in", None, None)
        .await
        .is_err());

    // 5.6 una orden sin lineas no se confirma.
    assert!(orders::confirm(app, &c, o2.id).await.is_err());

    // 5.3 snapshot de unit_price.
    let line = orders::add_line(app, &c, o2.id, p, dec(2), None)
        .await
        .unwrap();
    assert_eq!(line.unit_price.normalize(), dec(100));
    sqlx::query("update product_prices set price = 999 where product_id = $1")
        .bind(p)
        .execute(admin)
        .await
        .unwrap();
    let reloaded = orders::line(app, &c, line.id).await.unwrap();
    assert_eq!(reloaded.unit_price.normalize(), dec(100));
    assert_eq!(reloaded.line_total.normalize(), dec(200));

    // Confirmar reserva inventario y pasa a en_preparacion.
    orders::confirm(app, &c, o2.id).await.unwrap();
    assert_eq!(
        orders::order(app, &c, o2.id).await.unwrap().status,
        "en_preparacion"
    );
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(8)
    );

    // 5.4 maquina de estados + transicion invalida.
    assert!(orders::transition(app, &c, o1.id, OrderStatus::Lista)
        .await
        .is_err());
    orders::transition(app, &c, o2.id, OrderStatus::Lista).await.unwrap();
    orders::transition(app, &c, o2.id, OrderStatus::Entregada).await.unwrap();
    orders::transition(app, &c, o2.id, OrderStatus::Cobrada).await.unwrap();
    orders::transition(app, &c, o2.id, OrderStatus::Cerrada).await.unwrap();
    assert_eq!(orders::order(app, &c, o2.id).await.unwrap().status, "cerrada");
    assert!(orders::transition(app, &c, o2.id, OrderStatus::EnPreparacion)
        .await
        .is_err());

    // 5.5 cancelacion libera reservas y mesa.
    orders::add_line(app, &c, o1.id, p, dec(1), None).await.unwrap();
    orders::confirm(app, &c, o1.id).await.unwrap();
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(7)
    );
    orders::cancel(app, &c, o1.id).await.unwrap();
    assert_eq!(
        orders::order(app, &c, o1.id).await.unwrap().status,
        "cancelada"
    );
    assert_eq!(
        inventory::available(app, &c, loc, p).await.unwrap().unwrap().normalize(),
        dec(8)
    );
    let t_status: String =
        sqlx::query_scalar("select status::text from tables where id = $1")
            .bind(table)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(t_status, "libre");

    cleanup(admin, tenant, &[user]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_integration() {
    let Some(admin) = admin_pool().await else {
        return;
    };
    let Some(app) = app_pool().await else {
        return;
    };

    context_sets_uid_and_tenant(&app).await;
    resolve_membership_checks(&app, &admin).await;
    ensure_location_checks(&app, &admin).await;
    rls_isolation_checks(&app, &admin).await;
    composite_fk_check(&admin).await;
    catalog_checks(&app, &admin).await;
    recipe_checks(&app, &admin).await;
    inventory_checks(&app, &admin).await;
    order_checks(&app, &admin).await;
}

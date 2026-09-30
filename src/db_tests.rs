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
    let n: i64 = sqlx::query_scalar("select count(*) from product_categories")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    n
}

async fn context_sets_uid_tenant_branch(app: &PgPool) {
    let uid = Uuid::new_v4();
    let tenant = Uuid::new_v4();
    let branch = Uuid::new_v4();
    let c = claims(uid, Some(tenant), Some(branch));

    let mut tx = crate::db::begin_tenant_tx(app, &c).await.unwrap();
    let got_uid: Option<Uuid> = sqlx::query_scalar("select auth.uid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let got_tenant: Option<Uuid> = sqlx::query_scalar("select public.current_tenant_id()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let got_branch: Option<Uuid> = sqlx::query_scalar("select public.current_branch_id()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(got_uid, Some(uid));
    assert_eq!(got_tenant, Some(tenant));
    assert_eq!(got_branch, Some(branch));
}

async fn resolve_membership_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let other = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "MESERO").await;

    let c = claims(user, Some(tenant), None);
    assert_eq!(
        tenancy::resolve_membership(app, &c, None).await.unwrap(),
        (tenant, "MESERO".to_string())
    );

    let c_other = claims(user, Some(other), None);
    assert!(tenancy::resolve_membership(app, &c_other, None).await.is_err());

    let c_none = claims(user, None, None);
    assert_eq!(
        tenancy::resolve_membership(app, &c_none, None).await.unwrap(),
        (tenant, "MESERO".to_string())
    );

    cleanup(admin, tenant, &[user]).await;
    cleanup(admin, other, &[]).await;
}

async fn ensure_branch_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let other = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;

    let branch = seed_branch(admin, tenant).await;
    let foreign = seed_branch(admin, other).await;

    let c = claims(user, Some(tenant), None);
    assert!(tenancy::ensure_branch_in_tenant(app, &c, tenant, branch)
        .await
        .is_ok());
    assert!(
        tenancy::ensure_branch_in_tenant(app, &c, tenant, foreign)
            .await
            .is_err()
    );

    cleanup(admin, tenant, &[user]).await;
    cleanup(admin, other, &[]).await;
}

async fn rls_isolation_checks(app: &PgPool, admin: &PgPool) {
    let tenant_a = seed_tenant(admin).await;
    let tenant_b = seed_tenant(admin).await;
    let branch_a = seed_branch(admin, tenant_a).await;
    let branch_b = seed_branch(admin, tenant_b).await;
    seed_category(admin, tenant_a, branch_a).await;
    let cat_b = seed_category(admin, tenant_b, branch_b).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant_a, None, user, "ADMIN_CAJA").await;

    // Con contexto de A solo se ve lo de A.
    let ctx_a = claims(user, Some(tenant_a), None);
    assert_eq!(count_categories(app, &ctx_a).await, 1);

    // Sin tenant -> fail-closed.
    let ctx_none = claims(user, None, None);
    assert_eq!(count_categories(app, &ctx_none).await, 0);

    // No puede modificar filas de otro tenant.
    let mut tx = crate::db::begin_tenant_tx(app, &ctx_a).await.unwrap();
    let affected = sqlx::query("update product_categories set name = 'hacked' where id = $1")
        .bind(cat_b)
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    tx.commit().await.unwrap();
    assert_eq!(affected, 0);

    let name: String = sqlx::query_scalar("select name from product_categories where id = $1")
        .bind(cat_b)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(name, "cat");

    cleanup(admin, tenant_a, &[user]).await;
    cleanup(admin, tenant_b, &[]).await;
}

async fn catalog_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch_a = seed_branch(admin, tenant).await;
    let branch_b = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;

    let cat = seed_category(admin, tenant, branch_a).await;

    // Vendible con impuesto INC8 y precio base 10000, override 12000 en A.
    let limonada = seed_product(admin, tenant, "Limonada", "UND", Some("INC8"), false).await;
    set_base_price(admin, limonada, "10000").await;
    seed_branch_product(admin, tenant, branch_a, limonada, Some(cat), Some("12000")).await;
    seed_branch_product(admin, tenant, branch_b, limonada, None, None).await;
    seed_stock(admin, tenant, branch_a, limonada, "5", "0").await;

    // Inactivo: en el surtido pero inactivo.
    let inactive = seed_product(admin, tenant, "Viejo", "UND", None, false).await;
    sqlx::query("update products set active = false where id = $1")
        .bind(inactive)
        .execute(admin)
        .await
        .unwrap();
    seed_branch_product(admin, tenant, branch_a, inactive, None, Some("1")).await;

    // Insumo: producto sin fila en el surtido -> fuera del catalogo.
    let supply = seed_product(admin, tenant, "Azucar", "G", None, false).await;

    // Compuesto: arroz por insumo, 0.2 por unidad.
    let dish = seed_product(admin, tenant, "Arroz", "UND", None, true).await;
    let insumo = seed_product(admin, tenant, "Arroz grano", "G", None, false).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "0.2").await;
    seed_branch_product(admin, tenant, branch_a, dish, Some(cat), Some("5000")).await;
    seed_branch_product(admin, tenant, branch_b, dish, None, None).await;
    seed_stock(admin, tenant, branch_a, insumo, "3", "0").await;

    let c = claims(user, Some(tenant), None);

    // Sucursal A: override de precio, impuesto, disponibilidad propia y derivada.
    let items_a = catalog::catalog(app, &c, branch_a).await.unwrap();
    let lim = items_a.iter().find(|i| i.product_id == limonada).unwrap();
    assert_eq!(lim.price.unwrap().normalize(), Decimal::from(12000));
    assert_eq!(lim.tax_code.as_deref(), Some("INC8"));
    assert_eq!(lim.tax_rate.unwrap().normalize(), Decimal::new(8, 2));
    assert_eq!(lim.available.unwrap().normalize(), Decimal::from(5));
    assert_eq!(lim.category.as_deref(), Some("cat"));

    let arroz = items_a.iter().find(|i| i.product_id == dish).unwrap();
    assert_eq!(arroz.available.unwrap().normalize(), Decimal::from(15)); // floor(3 / 0.2)

    assert!(items_a.iter().all(|i| i.product_id != inactive));
    assert!(items_a.iter().all(|i| i.product_id != supply));
    assert_eq!(items_a.iter().filter(|i| i.product_id == limonada).count(), 1);

    // Sucursal B: precio base y sin stock -> 0 disponible.
    let items_b = catalog::catalog(app, &c, branch_b).await.unwrap();
    let lim_b = items_b.iter().find(|i| i.product_id == limonada).unwrap();
    assert_eq!(lim_b.price.unwrap().normalize(), Decimal::from(10000));
    assert_eq!(lim_b.available.unwrap().normalize(), Decimal::ZERO);
    let arroz_b = items_b.iter().find(|i| i.product_id == dish).unwrap();
    assert_eq!(arroz_b.available.unwrap().normalize(), Decimal::ZERO);

    cleanup(admin, tenant, &[user]).await;
}

async fn recipe_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;

    let dish = seed_product(admin, tenant, "Plato", "UND", None, true).await;
    let insumo = seed_product(admin, tenant, "Insumo", "G", None, false).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "0.25").await;

    let c = claims(user, Some(tenant), None);
    let items = catalog::recipe(app, &c, dish).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].input_product_id, insumo);
    assert_eq!(items[0].qty.normalize(), Decimal::new(25, 2));

    // Producto visible pero sin receta -> lista vacia.
    let plain = seed_product(admin, tenant, "Suelto", "UND", None, false).await;
    assert!(catalog::recipe(app, &c, plain).await.unwrap().is_empty());

    cleanup(admin, tenant, &[user]).await;
}

async fn inventory_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;
    let c = claims(user, Some(tenant), None);

    let p = seed_product(admin, tenant, "Pan", "UND", None, false).await;
    seed_stock(admin, tenant, branch, p, "5", "0").await;

    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(5)
    );

    let ids = inventory::reserve(app, &c, branch, None, None, p, dec(3), None)
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    assert!(
        inventory::reserve(app, &c, branch, None, None, p, dec(3), None)
            .await
            .is_err()
    );

    // Sobreventa bajo reservas concurrentes: exactamente una gana.
    let p2 = seed_product(admin, tenant, "Agua", "UND", None, false).await;
    seed_stock(admin, tenant, branch, p2, "1", "0").await;
    let (r1, r2) = tokio::join!(
        inventory::reserve(app, &c, branch, None, None, p2, dec(1), None),
        inventory::reserve(app, &c, branch, None, None, p2, dec(1), None),
    );
    assert!(r1.is_ok() ^ r2.is_ok(), "exactly one concurrent reserve wins");

    // Consumacion: descuento definitivo + movimiento.
    assert_eq!(inventory::consume(app, &c, &ids).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );
    let mv = inventory::movements(app, &c, branch).await.unwrap();
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "DESCUENTO" && m.qty.normalize() == dec(3)));

    // Liberacion explicita.
    let r = inventory::reserve(app, &c, branch, None, None, p, dec(1), None)
        .await
        .unwrap();
    assert_eq!(inventory::release(app, &c, &r).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    // Expiracion por barrido.
    let past = Utc::now() - Duration::minutes(5);
    inventory::reserve(app, &c, branch, None, None, p, dec(1), Some(past))
        .await
        .unwrap();
    assert!(inventory::expire_due(app, &c).await.unwrap() >= 1);
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(2)
    );

    // Reposicion y ajuste con bitacora.
    inventory::replenish(app, &c, branch, p, dec(10), "compra")
        .await
        .unwrap();
    inventory::adjust(app, &c, branch, p, dec(-2), "merma")
        .await
        .unwrap();
    let mv = inventory::movements(app, &c, branch).await.unwrap();
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "REPOSICION" && m.qty.normalize() == dec(10)));
    assert!(mv
        .iter()
        .any(|m| m.movement_type == "AJUSTE" && m.qty.normalize() == dec(2)));
    // on_hand: 5 - 3 (consumo) = 2; +10 = 12; -2 = 10.
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(10)
    );

    // Reserva por insumo (explosion de receta).
    let dish = seed_product(admin, tenant, "Torta", "UND", None, true).await;
    let insumo = seed_product(admin, tenant, "Harina", "G", None, false).await;
    let rec = seed_recipe(admin, tenant, dish).await;
    seed_recipe_item(admin, tenant, rec, insumo, "2").await;
    seed_stock(admin, tenant, branch, insumo, "4", "0").await;

    let ids2 = inventory::reserve(app, &c, branch, None, None, dish, dec(1), None)
        .await
        .unwrap();
    assert_eq!(ids2.len(), 1);
    // Insumo disponible 4, 2 por unidad -> 2; tras reservar 1 -> 1.
    assert_eq!(
        inventory::available(app, &c, branch, dish).await.unwrap().unwrap().normalize(),
        dec(1)
    );

    cleanup(admin, tenant, &[user]).await;
}

async fn table_status(pool: &PgPool, table: Uuid) -> String {
    sqlx::query_scalar(
        "select t.tima_codigo from restaurant_tables rt
           join tbl_tipos_maestra t on t.tima_id = rt.status_id
          where rt.id = $1",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn order_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "MESERO").await;
    let c = claims(user, Some(tenant), None);

    let table = seed_table(admin, tenant, branch, "T1").await;
    let p = seed_product(admin, tenant, "Plato", "UND", None, false).await;
    set_base_price(admin, p, "100").await;
    seed_branch_product(admin, tenant, branch, p, None, None).await;
    seed_stock(admin, tenant, branch, p, "10", "0").await;

    // 5.2 MESA con mesa (se ocupa).
    let o1 = orders::create_order(app, &c, branch, "MESA", Some(table), None, None)
        .await
        .unwrap();
    assert_eq!(o1.status, "CREADA");
    assert_eq!(table_status(admin, table).await, "OCUPADA");
    assert!(orders::create_order(app, &c, branch, "MESA", Some(table), None, None)
        .await
        .is_err());

    // 5.2 MOSTRADOR sin mesa; MESA sin mesa rechazado.
    let o2 = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    assert_eq!(o2.status, "CREADA");
    assert!(orders::create_order(app, &c, branch, "MESA", None, None, None)
        .await
        .is_err());

    // 5.6 una orden sin lineas no se confirma.
    assert!(orders::confirm(app, &c, o2.id).await.is_err());

    // 5.3 snapshot de unit_price.
    let line = orders::add_line(app, &c, o2.id, p, dec(2), None)
        .await
        .unwrap();
    assert_eq!(line.unit_price.normalize(), dec(100));
    set_base_price(admin, p, "999").await;
    let reloaded = orders::line(app, &c, line.id).await.unwrap();
    assert_eq!(reloaded.unit_price.normalize(), dec(100));
    assert_eq!(reloaded.line_total.normalize(), dec(200));
    assert!(reloaded.station_id.is_none());

    // Confirmar reserva inventario y pasa a EN_PREPARACION.
    orders::confirm(app, &c, o2.id).await.unwrap();
    assert_eq!(
        orders::order(app, &c, o2.id).await.unwrap().status,
        "EN_PREPARACION"
    );
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
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
    assert_eq!(orders::order(app, &c, o2.id).await.unwrap().status, "CERRADA");
    assert!(orders::transition(app, &c, o2.id, OrderStatus::EnPreparacion)
        .await
        .is_err());

    // 5.5 cancelacion libera reservas y mesa.
    orders::add_line(app, &c, o1.id, p, dec(1), None).await.unwrap();
    orders::confirm(app, &c, o1.id).await.unwrap();
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(7)
    );
    orders::cancel(app, &c, o1.id).await.unwrap();
    assert_eq!(orders::order(app, &c, o1.id).await.unwrap().status, "CANCELADA");
    assert_eq!(
        inventory::available(app, &c, branch, p).await.unwrap().unwrap().normalize(),
        dec(8)
    );
    assert_eq!(table_status(admin, table).await, "LIBRE");

    cleanup(admin, tenant, &[user]).await;
}

async fn hook_checks(admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    sqlx::query("insert into user_active_context (user_id, tenant_id, branch_id) values ($1, $2, $3)")
        .bind(user)
        .bind(tenant)
        .bind(branch)
        .execute(admin)
        .await
        .unwrap();

    let event = serde_json::json!({
        "user_id": user.to_string(),
        "claims": { "sub": user.to_string(), "app_metadata": {} }
    });
    let out: serde_json::Value =
        sqlx::query_scalar("select public.custom_access_token_hook($1::jsonb)")
            .bind(event)
            .fetch_one(admin)
            .await
            .unwrap();

    assert_eq!(
        out["claims"]["app_metadata"]["tenant_id"],
        serde_json::json!(tenant.to_string())
    );
    assert_eq!(
        out["claims"]["app_metadata"]["branch_id"],
        serde_json::json!(branch.to_string())
    );

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

    context_sets_uid_tenant_branch(&app).await;
    resolve_membership_checks(&app, &admin).await;
    ensure_branch_checks(&app, &admin).await;
    hook_checks(&admin).await;
    rls_isolation_checks(&app, &admin).await;
    catalog_checks(&app, &admin).await;
    recipe_checks(&app, &admin).await;
    inventory_checks(&app, &admin).await;
    order_checks(&app, &admin).await;
}

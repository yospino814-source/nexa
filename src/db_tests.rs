//! Pruebas de integracion contra la BD real (RLS, tenancy, catalogo).
//!
//! Se ejecutan en un unico test para compartir un runtime y los pools de
//! conexion (Supabase limita clientes en modo sesion).

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiError, DomainError};
use crate::orders::OrderStatus;
use crate::sales::{Checkout, PaymentMethod, QuoteRequest, SaleItem, SalePayment, SaleRequest};
use crate::test_support::*;
use crate::{
    AppState, cash, catalog, customers, inventory, orders, outbox, permissions, procurement,
    registers, routes, sales, sequences, settings, tenancy, users,
};

async fn default_register(pool: &PgPool, branch_id: Uuid) -> Uuid {
    sqlx::query_scalar("select id from cash_registers where branch_id = $1 and code = 'CAJA-1'")
        .bind(branch_id)
        .fetch_one(pool)
        .await
        .expect("branch default register CAJA-1")
}

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
    assert!(
        tenancy::resolve_membership(app, &c_other, None)
            .await
            .is_err()
    );

    let c_none = claims(user, None, None);
    assert_eq!(
        tenancy::resolve_membership(app, &c_none, None)
            .await
            .unwrap(),
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
    assert!(
        tenancy::ensure_branch_in_tenant(app, &c, tenant, branch)
            .await
            .is_ok()
    );
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
    assert_eq!(
        items_a.iter().filter(|i| i.product_id == limonada).count(),
        1
    );

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
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(5)
    );

    let ids = inventory::reserve(app, &c, branch, None, None, p, dec(3), None)
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
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
    assert!(
        r1.is_ok() ^ r2.is_ok(),
        "exactly one concurrent reserve wins"
    );

    // Consumacion: descuento definitivo + movimiento.
    assert_eq!(inventory::consume(app, &c, &ids).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(2)
    );
    let mv = inventory::movements(app, &c, branch).await.unwrap();
    assert!(
        mv.iter()
            .any(|m| m.movement_type == "DESCUENTO" && m.qty.normalize() == dec(3))
    );

    // Liberacion explicita.
    let r = inventory::reserve(app, &c, branch, None, None, p, dec(1), None)
        .await
        .unwrap();
    assert_eq!(inventory::release(app, &c, &r).await.unwrap(), 1);
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(2)
    );

    // Expiracion por barrido.
    let past = Utc::now() - Duration::minutes(5);
    inventory::reserve(app, &c, branch, None, None, p, dec(1), Some(past))
        .await
        .unwrap();
    assert!(inventory::expire_due(app, &c).await.unwrap() >= 1);
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
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
    assert!(
        mv.iter()
            .any(|m| m.movement_type == "REPOSICION" && m.qty.normalize() == dec(10))
    );
    // El ajuste negativo queda como salida, con delta -2 en el kardex.
    assert!(mv.iter().any(|m| m.movement_type == "AJUSTE_SALIDA"
        && m.qty.normalize() == dec(2)
        && m.delta.map(|d| d.normalize()) == Some(dec(-2))));
    assert!(
        mv.iter()
            .any(|m| m.movement_type == "DESCUENTO"
                && m.delta.map(|d| d.normalize()) == Some(dec(-3)))
    );
    assert!(mv.iter().any(
        |m| m.movement_type == "REPOSICION" && m.delta.map(|d| d.normalize()) == Some(dec(10))
    ));
    // on_hand: 5 - 3 (consumo) = 2; +10 = 12; -2 = 10.
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(10)
    );

    // Ajuste positivo -> entrada con delta +1.
    inventory::adjust(app, &c, branch, p, dec(1), "conteo")
        .await
        .unwrap();
    let mv = inventory::movements(app, &c, branch).await.unwrap();
    assert!(
        mv.iter().any(|m| m.movement_type == "AJUSTE_ENTRADA"
            && m.delta.map(|d| d.normalize()) == Some(dec(1)))
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
        inventory::available(app, &c, branch, dish)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
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
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;
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
    assert!(
        orders::create_order(app, &c, branch, "MESA", Some(table), None, None)
            .await
            .is_err()
    );

    // 5.2 MOSTRADOR sin mesa; MESA sin mesa rechazado.
    let o2 = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    assert_eq!(o2.status, "CREADA");
    assert!(
        orders::create_order(app, &c, branch, "MESA", None, None, None)
            .await
            .is_err()
    );

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
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(8)
    );

    // 5.4 maquina de estados + transicion invalida.
    assert!(
        orders::transition(app, &c, o1.id, OrderStatus::Lista)
            .await
            .is_err()
    );
    orders::transition(app, &c, o2.id, OrderStatus::Lista)
        .await
        .unwrap();
    orders::transition(app, &c, o2.id, OrderStatus::Entregada)
        .await
        .unwrap();
    // Antes de cobrar, las 2 unidades estan reservadas pero no descontadas.
    assert_eq!(stock_row(admin, branch, p).await, (dec(10), dec(2)));
    orders::transition(app, &c, o2.id, OrderStatus::Cobrada)
        .await
        .unwrap();
    // Cobrar consume la reserva: el stock fisico baja de verdad.
    assert_eq!(stock_row(admin, branch, p).await, (dec(8), dec(0)));
    let mv = inventory::movements(app, &c, branch).await.unwrap();
    assert!(
        mv.iter()
            .any(|m| m.movement_type == "DESCUENTO" && m.qty.normalize() == dec(2))
    );
    orders::transition(app, &c, o2.id, OrderStatus::Cerrada)
        .await
        .unwrap();
    assert_eq!(
        orders::order(app, &c, o2.id).await.unwrap().status,
        "CERRADA"
    );
    assert!(
        orders::transition(app, &c, o2.id, OrderStatus::EnPreparacion)
            .await
            .is_err()
    );

    // 5.5 cancelacion libera reservas y mesa.
    orders::add_line(app, &c, o1.id, p, dec(1), None)
        .await
        .unwrap();
    orders::confirm(app, &c, o1.id).await.unwrap();
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(7)
    );
    orders::cancel(app, &c, o1.id).await.unwrap();
    assert_eq!(
        orders::order(app, &c, o1.id).await.unwrap().status,
        "CANCELADA"
    );
    assert_eq!(
        inventory::available(app, &c, branch, p)
            .await
            .unwrap()
            .unwrap()
            .normalize(),
        dec(8)
    );
    assert_eq!(table_status(admin, table).await, "LIBRE");

    // Cerrar una orden de MESA libera la mesa.
    let o3 = orders::create_order(app, &c, branch, "MESA", Some(table), None, None)
        .await
        .unwrap();
    assert_eq!(table_status(admin, table).await, "OCUPADA");
    orders::add_line(app, &c, o3.id, p, dec(1), None)
        .await
        .unwrap();
    orders::confirm(app, &c, o3.id).await.unwrap();
    for to in [
        OrderStatus::Lista,
        OrderStatus::Entregada,
        OrderStatus::Cobrada,
        OrderStatus::Cerrada,
    ] {
        orders::transition(app, &c, o3.id, to).await.unwrap();
    }
    assert_eq!(table_status(admin, table).await, "LIBRE");
    assert_eq!(stock_row(admin, branch, p).await, (dec(7), dec(0)));

    // Pasar a EN_PREPARACION por `transition` tambien reserva (no se salta confirm).
    let o4 = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    orders::add_line(app, &c, o4.id, p, dec(1), None)
        .await
        .unwrap();
    orders::transition(app, &c, o4.id, OrderStatus::EnPreparacion)
        .await
        .unwrap();
    assert_eq!(stock_row(admin, branch, p).await, (dec(7), dec(1)));
    orders::cancel(app, &c, o4.id).await.unwrap();
    assert_eq!(stock_row(admin, branch, p).await, (dec(7), dec(0)));

    cleanup(admin, tenant, &[user]).await;
}

async fn stock_row(pool: &PgPool, branch: Uuid, product: Uuid) -> (Decimal, Decimal) {
    let (on_hand, reserved): (Decimal, Decimal) = sqlx::query_as(
        "select on_hand, reserved from stock where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(product)
    .fetch_one(pool)
    .await
    .unwrap();
    (on_hand.normalize(), reserved.normalize())
}

/// Totales con impuesto y confirmacion atomica.
async fn order_totals_and_atomic_confirm_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    seed_membership(admin, tenant, None, user, "ADMIN_CAJA").await;
    let c = claims(user, Some(tenant), None);

    // Gaseosa IVA 19% a 2500; pan exento a 1000 con poco stock.
    let soda = seed_product(admin, tenant, "Gaseosa", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "10", "0").await;
    let bread = seed_product(admin, tenant, "Pan", "UND", None, false).await;
    set_base_price(admin, bread, "1000").await;
    seed_branch_product(admin, tenant, branch, bread, None, None).await;
    seed_stock(admin, tenant, branch, bread, "1", "0").await;

    // Totales: 2 x 2500 = 5000 + IVA 950; 3 x 1000 = 3000 exento.
    let o = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    let l1 = orders::add_line(app, &c, o.id, soda, dec(2), None)
        .await
        .unwrap();
    assert_eq!(l1.tax_rate.normalize(), Decimal::new(19, 2));
    assert_eq!(l1.tax_amount.normalize(), dec(950));
    orders::add_line(app, &c, o.id, bread, dec(3), None)
        .await
        .unwrap();
    let got = orders::order(app, &c, o.id).await.unwrap();
    assert_eq!(got.subtotal.normalize(), dec(8000));
    assert_eq!(got.inc_amount.normalize(), dec(950));
    assert_eq!(got.total_amount.normalize(), dec(8950));

    // Confirmacion atomica: el pan no alcanza (3 > 1), asi que tampoco queda
    // reservada la gaseosa y la orden sigue CREADA.
    assert!(orders::confirm(app, &c, o.id).await.is_err());
    assert_eq!(orders::order(app, &c, o.id).await.unwrap().status, "CREADA");
    assert_eq!(stock_row(admin, branch, soda).await, (dec(10), dec(0)));
    assert_eq!(stock_row(admin, branch, bread).await, (dec(1), dec(0)));
    let reservations: i64 =
        sqlx::query_scalar("select count(*) from stock_reservations where order_id = $1")
            .bind(o.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(reservations, 0);

    // Confirmaciones concurrentes de la misma orden: solo una gana y se
    // reserva una sola vez.
    let o2 = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    orders::add_line(app, &c, o2.id, soda, dec(1), None)
        .await
        .unwrap();
    let (r1, r2) = tokio::join!(
        orders::confirm(app, &c, o2.id),
        orders::confirm(app, &c, o2.id),
    );
    assert!(
        r1.is_ok() ^ r2.is_ok(),
        "exactly one concurrent confirm wins"
    );
    assert_eq!(stock_row(admin, branch, soda).await, (dec(10), dec(1)));

    // No se pueden agregar lineas a una orden ya confirmada.
    assert!(
        orders::add_line(app, &c, o2.id, soda, dec(1), None)
            .await
            .is_err()
    );

    // Un producto inactivo no se puede vender.
    let o3 = orders::create_order(app, &c, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    sqlx::query("update products set active = false where id = $1")
        .bind(bread)
        .execute(admin)
        .await
        .unwrap();
    assert!(
        orders::add_line(app, &c, o3.id, bread, dec(1), None)
            .await
            .is_err()
    );

    cleanup(admin, tenant, &[user]).await;
}

async fn hook_checks(admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let user = seed_user(admin).await;
    sqlx::query(
        "insert into user_active_context (user_id, tenant_id, branch_id) values ($1, $2, $3)",
    )
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
    order_totals_and_atomic_confirm_checks(&app, &admin).await;
    branch_access_checks(&app, &admin).await;
    role_checks(&app, &admin).await;
    sales_and_cash_checks(&app, &admin).await;
    stock_levels_and_transfer_checks(&app, &admin).await;
    supplier_checks(&app, &admin).await;
    purchase_checks(&app, &admin).await;
    platform_checks(&app, &admin).await;
    context_returns_permissions(&app, &admin).await;
    f1_registers(&app, &admin).await;
    f1_cash_kinds(&app, &admin).await;
    f1_cash_state(&app, &admin).await;
    f1_sale_customer(&app, &admin).await;
    f1_pricing_db(&app, &admin).await;
}

/// Existencias con punto de reorden, conteo fisico, kardex con saldo y traslados.
async fn stock_levels_and_transfer_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let main = seed_branch(admin, tenant).await;
    let annex = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let clerk = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    // Administra solo la sede principal.
    seed_membership(admin, tenant, Some(main), clerk, "ADMIN_CAJA").await;
    let c = claims(owner, Some(tenant), None);

    let rice = seed_product(admin, tenant, "Arroz", "UND", None, false).await;
    seed_branch_product(admin, tenant, main, rice, None, None).await;
    seed_stock(admin, tenant, main, rice, "10", "0").await;
    // En surtido pero nunca tuvo stock: aparece agotado.
    let oil = seed_product(admin, tenant, "Aceite", "UND", None, false).await;
    seed_branch_product(admin, tenant, main, oil, None, None).await;

    let levels = inventory::stock_levels(app, &c, main).await.unwrap();
    let row = |levels: &[inventory::StockLevel], p: Uuid| {
        let r = levels.iter().find(|l| l.product_id == p).unwrap();
        (r.available.normalize(), r.status.clone())
    };
    assert_eq!(levels.len(), 2);
    assert_eq!(row(&levels, rice), (dec(10), "OK".to_string()));
    assert_eq!(row(&levels, oil), (dec(0), "AGOTADO".to_string()));

    // Punto de reorden 10 -> el arroz queda en stock bajo.
    inventory::set_min_qty(app, &c, main, rice, dec(10))
        .await
        .unwrap();
    let levels = inventory::stock_levels(app, &c, main).await.unwrap();
    assert_eq!(row(&levels, rice).1, "BAJO");

    // Conteo fisico: 7 contadas -> salida de 3; mismo conteo no registra nada.
    assert_eq!(
        inventory::count(app, &c, main, rice, dec(7), "conteo")
            .await
            .unwrap()
            .normalize(),
        dec(-3)
    );
    assert_eq!(
        inventory::count(app, &c, main, rice, dec(7), "conteo")
            .await
            .unwrap()
            .normalize(),
        dec(0)
    );
    // Conteo de un producto sin fila de stock: la crea y registra la entrada.
    inventory::count(app, &c, main, oil, dec(4), "llego")
        .await
        .unwrap();
    assert_eq!(stock_row(admin, main, oil).await, (dec(4), dec(0)));

    // Traslado de 5 a la sede anexa (sin fila previa alla).
    inventory::transfer(app, &c, main, annex, rice, dec(5), "surtir anexo")
        .await
        .unwrap();
    assert_eq!(stock_row(admin, main, rice).await, (dec(2), dec(0)));
    assert_eq!(stock_row(admin, annex, rice).await, (dec(5), dec(0)));
    // No se traslada mas de lo disponible, ni a la misma sede.
    assert!(is_state(
        &inventory::transfer(app, &c, main, annex, rice, dec(3), "x").await
    ));
    assert!(matches!(
        inventory::transfer(app, &c, main, main, rice, dec(1), "x").await,
        Err(ApiError::Domain(DomainError::Validation(_)))
    ));
    // Quien solo administra la principal no puede mandar a la anexa.
    let k = claims(clerk, Some(tenant), Some(main));
    assert!(is_forbidden(
        &inventory::transfer(app, &k, main, annex, rice, dec(1), "x").await
    ));

    // Kardex del arroz en la principal: mas reciente primero y saldo correcto.
    let kx = inventory::kardex(app, &c, main, Some(rice), 50)
        .await
        .unwrap();
    let seq: Vec<(String, Decimal)> = kx
        .iter()
        .map(|m| {
            (
                m.movement_type.clone(),
                m.balance_after.unwrap().normalize(),
            )
        })
        .collect();
    assert_eq!(
        seq,
        vec![
            ("TRASLADO_SALIDA".to_string(), dec(2)),
            ("AJUSTE_SALIDA".to_string(), dec(7)),
        ]
    );
    assert_eq!(kx[0].product_name, "Arroz");
    let annex_kx = inventory::kardex(app, &c, annex, None, 50).await.unwrap();
    assert_eq!(annex_kx.len(), 1);
    assert_eq!(annex_kx[0].movement_type, "TRASLADO_ENTRADA");
    assert_eq!(annex_kx[0].ref_id, kx[0].ref_id);

    cleanup(admin, tenant, &[owner, clerk]).await;
}

fn is_forbidden<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    matches!(r, Err(ApiError::Domain(DomainError::Forbidden)))
}

fn is_state<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    matches!(r, Err(ApiError::Domain(DomainError::State(_))))
}

fn is_conflict<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    match r {
        Err(e) => e.status() == axum::http::StatusCode::CONFLICT,
        Ok(_) => false,
    }
}

fn is_validation<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    matches!(r, Err(ApiError::Domain(DomainError::Validation(_))))
}

fn is_too_many_requests<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    matches!(r, Err(ApiError::Domain(DomainError::TooManyRequests(_))))
}

/// Una membresia de sucursal solo ve y opera su sucursal, aunque el JWT pida otra.
async fn branch_access_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch_a = seed_branch(admin, tenant).await;
    let branch_b = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch_a), cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;

    let p = seed_product(admin, tenant, "Agua", "UND", None, false).await;
    set_base_price(admin, p, "1000").await;
    seed_branch_product(admin, tenant, branch_a, p, None, None).await;
    seed_branch_product(admin, tenant, branch_b, p, None, None).await;

    let c = claims(cashier, Some(tenant), None);
    assert_eq!(catalog::catalog(app, &c, branch_a).await.unwrap().len(), 1);
    assert!(is_forbidden(&catalog::catalog(app, &c, branch_b).await));

    // Aunque el JWT diga sucursal B, la membresia es de A.
    let c_jwt_b = claims(cashier, Some(tenant), Some(branch_b));
    assert!(is_forbidden(
        &catalog::catalog(app, &c_jwt_b, branch_b).await
    ));

    // Sin sucursal pedida, RLS queda limitado a la de la membresia.
    let (scope, tx) = tenancy::begin(app, &c).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(scope.branch, Some(branch_a));

    // El dueño (todo el tenant) ve ambas; una orden de B es invisible para el cajero de A.
    let o = owner_claims(owner, tenant);
    assert_eq!(catalog::catalog(app, &o, branch_b).await.unwrap().len(), 1);
    let order_b = orders::create_order(app, &o, branch_b, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    assert!(matches!(
        orders::order(app, &c, order_b.id).await,
        Err(ApiError::Domain(DomainError::NotFound))
    ));
    assert!(orders::order(app, &o, order_b.id).await.is_ok());

    // Sucursales visibles con el rol en cada una.
    let mine = tenancy::branches(app, &c).await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].id, mine[0].role.as_str()), (branch_a, "CAJERO"));
    let all = tenancy::branches(app, &o).await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|b| b.role == "DUENO"));

    // Rol de todo el negocio + rol especifico en B: en B gana el especifico.
    let mixed = seed_user(admin).await;
    seed_membership(admin, tenant, None, mixed, "MESERO").await;
    seed_membership(admin, tenant, Some(branch_b), mixed, "ADMIN_CAJA").await;
    let m = claims(mixed, Some(tenant), None);
    let roles: Vec<(Uuid, String)> = tenancy::branches(app, &m)
        .await
        .unwrap()
        .into_iter()
        .map(|b| (b.id, b.role))
        .collect();
    assert!(roles.contains(&(branch_a, "MESERO".to_string())));
    assert!(roles.contains(&(branch_b, "ADMIN_CAJA".to_string())));
    // Y las operaciones en B usan ese rol.
    let (scope_b, tx) = tenancy::begin_in_branch(app, &m, branch_b).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(scope_b.role, "ADMIN_CAJA");

    cleanup(admin, tenant, &[cashier, owner, mixed]).await;
}

fn owner_claims(user: Uuid, tenant: Uuid) -> crate::auth::Claims {
    claims(user, Some(tenant), None)
}

/// Matriz de permisos aplicada en las operaciones.
async fn role_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let waiter = seed_user(admin).await;
    let cook = seed_user(admin).await;
    seed_membership(admin, tenant, None, waiter, "MESERO").await;
    seed_membership(admin, tenant, None, cook, "COCINA").await;
    let w = claims(waiter, Some(tenant), None);
    let k = claims(cook, Some(tenant), None);

    let p = seed_product(admin, tenant, "Plato", "UND", None, false).await;
    set_base_price(admin, p, "100").await;
    seed_branch_product(admin, tenant, branch, p, None, None).await;
    seed_stock(admin, tenant, branch, p, "10", "0").await;

    // El mesero toma y confirma la orden; la cocina no puede crearla.
    assert!(is_forbidden(
        &orders::create_order(app, &k, branch, "MOSTRADOR", None, None, None).await
    ));
    let order = orders::create_order(app, &w, branch, "MOSTRADOR", None, None, None)
        .await
        .unwrap();
    orders::add_line(app, &w, order.id, p, dec(1), None)
        .await
        .unwrap();
    orders::confirm(app, &w, order.id).await.unwrap();

    // Solo cocina (o admin) marca LISTA; el mesero entrega pero no cobra.
    assert!(is_forbidden(
        &orders::transition(app, &w, order.id, OrderStatus::Lista).await
    ));
    orders::transition(app, &k, order.id, OrderStatus::Lista)
        .await
        .unwrap();
    orders::transition(app, &w, order.id, OrderStatus::Entregada)
        .await
        .unwrap();
    assert!(is_forbidden(
        &orders::transition(app, &w, order.id, OrderStatus::Cobrada).await
    ));
    assert!(is_forbidden(&orders::cancel(app, &w, order.id).await));

    // Inventario y caja quedan fuera del mesero.
    assert!(is_forbidden(
        &inventory::replenish(app, &w, branch, p, dec(1), "x").await
    ));
    assert!(is_forbidden(&inventory::movements(app, &w, branch).await));
    let def_reg = default_register(admin, branch).await;
    assert!(is_forbidden(
        &cash::open(app, &w, branch, def_reg, dec(0)).await
    ));

    cleanup(admin, tenant, &[waiter, cook]).await;
}

fn sale(
    id: Uuid,
    branch: Uuid,
    items: &[(Uuid, i64)],
    payments: &[(PaymentMethod, i64)],
    quoted_total: Decimal,
) -> SaleRequest {
    SaleRequest {
        id,
        branch_id: branch,
        items: items
            .iter()
            .map(|(product_id, qty)| SaleItem {
                product_id: Some(*product_id),
                custom_name: None,
                qty: dec(*qty),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
            })
            .collect(),
        payments: payments
            .iter()
            .map(|(method, amount)| SalePayment {
                method: *method,
                amount: dec(*amount),
                reference: None,
            })
            .collect(),
        quoted_total,
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    }
}

fn created(r: Checkout) -> sales::SaleResponse {
    match r {
        Checkout::Created(s) => s,
        Checkout::Replayed(_) => panic!("expected a new sale, got a replay"),
    }
}

/// Venta de mostrador: turno obligatorio, totales, stock, pagos, consecutivo,
/// outbox fiscal, idempotencia y arqueo.
async fn sales_and_cash_checks(app: &PgPool, admin: &PgPool) {
    use PaymentMethod::*;

    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let waiter = seed_user(admin).await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, waiter, "MESERO").await;
    let c = claims(cashier, Some(tenant), None);

    // Gaseosa 2500 + IVA 19%; pan exento 1000 con 1 unidad.
    let soda = seed_product(admin, tenant, "Gaseosa", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "10", "0").await;
    let bread = seed_product(admin, tenant, "Pan", "UND", None, false).await;
    set_base_price(admin, bread, "1000").await;
    seed_branch_product(admin, tenant, branch, bread, None, None).await;
    seed_stock(admin, tenant, branch, bread, "1", "0").await;

    let first = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 2), (bread, 1)],
        &[(Efectivo, 10000)],
        dec(6950),
    );

    // Sin turno abierto no se vende; el mesero no puede vender.
    assert!(is_state(&sales::checkout(app, &c, first.clone()).await));
    let w = claims(waiter, Some(tenant), None);
    assert!(is_forbidden(&sales::checkout(app, &w, first.clone()).await));

    let def_reg = default_register(admin, branch).await;
    // Apertura de turno (uno por cajero y sucursal).
    let session = cash::open(app, &c, branch, def_reg, dec(50000))
        .await
        .unwrap();
    assert!(is_conflict(
        &cash::open(app, &c, branch, def_reg, dec(0)).await
    ));

    // Venta 1: 2 x 2500 + 950 IVA + 1000 = 6950; paga 10000 -> vuelto 3050.
    let s1 = created(sales::checkout(app, &c, first.clone()).await.unwrap());
    assert_eq!(s1.order.status, "COBRADA");
    assert_eq!(s1.order.subtotal.normalize(), dec(6000));
    assert_eq!(s1.order.inc_amount.normalize(), dec(950));
    assert_eq!(s1.order.total_amount.normalize(), dec(6950));
    assert_eq!(s1.change_due.normalize(), dec(3050));
    assert_eq!(s1.receipt_number, "POS-000001");
    assert_eq!(s1.fiscal_status, "PENDIENTE");
    assert_eq!(s1.items.len(), 2);
    assert_eq!(s1.payments.len(), 1);
    assert_eq!(s1.payments[0].amount.normalize(), dec(6950));
    assert_eq!(
        s1.payments[0].tendered.map(|t| t.normalize()),
        Some(dec(10000))
    );
    // Descuento directo, sin reservas colgadas.
    assert_eq!(stock_row(admin, branch, soda).await, (dec(8), dec(0)));
    assert_eq!(stock_row(admin, branch, bread).await, (dec(0), dec(0)));

    // Reintento con el mismo cuerpo: misma venta, nada se cobra dos veces.
    match sales::checkout(app, &c, first.clone()).await.unwrap() {
        Checkout::Replayed(r) => {
            assert_eq!(r.order.id, s1.order.id);
            assert_eq!(r.receipt_number, "POS-000001");
        }
        Checkout::Created(_) => panic!("replay must not create a new sale"),
    }
    assert_eq!(stock_row(admin, branch, soda).await, (dec(8), dec(0)));
    let payments: i64 = sqlx::query_scalar("select count(*) from payments where order_id = $1")
        .bind(s1.order.id)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(payments, 1);

    // Mismo id con otro cuerpo -> 409.
    let mut tampered = first.clone();
    tampered.items[0].qty = dec(3);
    assert!(is_state(&sales::checkout(app, &c, tampered).await));

    // Doble clic: dos requests identicos a la vez -> una venta, un descuento.
    let twin = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(Qr, 2975)],
        dec(2975),
    );
    let (r1, r2) = tokio::join!(
        sales::checkout(app, &c, twin.clone()),
        sales::checkout(app, &c, twin.clone()),
    );
    let outcomes = [r1.unwrap(), r2.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Checkout::Created(_)))
            .count(),
        1
    );
    assert_eq!(stock_row(admin, branch, soda).await, (dec(7), dec(0)));

    // Sin stock: la venta se revierte entera y no consume consecutivo.
    let no_stock = sale(
        Uuid::new_v4(),
        branch,
        &[(bread, 1)],
        &[(Efectivo, 1000)],
        dec(1000),
    );
    assert!(is_state(&sales::checkout(app, &c, no_stock).await));

    // Pago mixto: 2975 = 1000 datafono + 2000 efectivo -> vuelto 25.
    let mixed = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(Datafono, 1000), (Efectivo, 2000)],
        dec(2975),
    );
    let s3 = created(sales::checkout(app, &c, mixed).await.unwrap());
    assert_eq!(s3.receipt_number, "POS-000003");
    assert_eq!(s3.change_due.normalize(), dec(25));

    // Pagos que no cubren el total.
    let short = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(Datafono, 100)],
        dec(2975),
    );
    assert!(matches!(
        sales::checkout(app, &c, short).await,
        Err(ApiError::Domain(DomainError::Validation(_)))
    ));

    // Rango DIAN agotado: la venta se rechaza.
    sqlx::query("update document_sequences set range_to = 3 where branch_id = $1")
        .bind(branch)
        .execute(admin)
        .await
        .unwrap();
    let over = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(Qr, 2975)],
        dec(2975),
    );
    assert!(is_state(&sales::checkout(app, &c, over).await));
    sqlx::query("update document_sequences set range_to = null where branch_id = $1")
        .bind(branch)
        .execute(admin)
        .await
        .unwrap();
    // Sin huecos: las ventas revertidas no consumieron numero.
    let next: i64 =
        sqlx::query_scalar("select next_number from document_sequences where branch_id = $1")
            .bind(branch)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(next, 4);

    // Resumen del turno: 3 ventas; efectivo esperado = 50000 + 6950 + 1975.
    let summary = cash::current(app, &c, branch).await.unwrap().unwrap();
    assert_eq!(summary.session.id, session.id);
    assert_eq!(summary.sales_count, 3);
    assert_eq!(summary.sales_total.normalize(), dec(12900));
    assert_eq!(summary.expected_cash.normalize(), dec(58925));
    let by = |m: &str| {
        summary
            .by_method
            .iter()
            .find(|t| t.method == m)
            .map(|t| t.amount.normalize())
    };
    assert_eq!(by("EFECTIVO"), Some(dec(8925)));
    assert_eq!(by("DATAFONO"), Some(dec(1000)));
    assert_eq!(by("QR"), Some(dec(2975)));

    // Ingreso y retiro de efectivo: mueven el esperado, no las ventas.
    use cash::CashMovementKind::{Ingreso, Retiro};
    let after_in = cash::add_movement(
        app,
        &c,
        session.id,
        Ingreso,
        dec(10000),
        Some("sencillo"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(after_in.cash_in.normalize(), dec(10000));
    assert_eq!(after_in.expected_cash.normalize(), dec(68925));
    // No se puede retirar mas de lo que hay en la gaveta, ni sin motivo.
    assert!(is_conflict(
        &cash::add_movement(
            app,
            &c,
            session.id,
            Retiro,
            dec(70000),
            Some("banco"),
            Some("CONSIGNACION")
        )
        .await
    ));
    assert!(matches!(
        cash::add_movement(app, &c, session.id, Retiro, dec(1), Some("  "), None).await,
        Err(ApiError::Domain(DomainError::Validation(_)))
    ));
    let after_out = cash::add_movement(
        app,
        &c,
        session.id,
        Retiro,
        dec(5000),
        Some("hielo"),
        Some("GASTO_MENOR"),
    )
    .await
    .unwrap();
    assert_eq!(after_out.cash_out.normalize(), dec(5000));
    assert_eq!(after_out.sales_count, 3);
    assert_eq!(after_out.expected_cash.normalize(), dec(63925));
    // El mesero no maneja caja.
    assert!(is_forbidden(
        &cash::add_movement(app, &w, session.id, Ingreso, dec(1), Some("x"), None).await
    ));

    // Bitacora: apertura, 3 ventas, ingreso y retiro, la mas reciente primero.
    let log = cash::activity(app, &c, session.id).await.unwrap();
    let kinds = |k: &str| log.iter().filter(|e| e.kind == k).count();
    assert_eq!(kinds("APERTURA"), 1);
    assert_eq!(kinds("VENTA"), 3);
    assert_eq!(kinds("INGRESO"), 1);
    assert_eq!(kinds("RETIRO"), 1);
    assert_eq!(log.last().unwrap().kind, "APERTURA");
    assert!(log.iter().any(|e| e.kind == "VENTA"
        && e.detail.as_deref() == Some("POS-000001")
        && e.methods.as_deref() == Some("EFECTIVO")));

    // Cierre con arqueo: faltan 25.
    let cur_session = cash::current(app, &c, branch).await.unwrap().unwrap();
    let closed = cash::close(
        app,
        &c,
        session.id,
        dec(63900),
        Some("faltante".into()),
        &cur_session.state_token,
    )
    .await
    .unwrap();
    assert_eq!(
        closed.session.difference.map(|d| d.normalize()),
        Some(dec(-25))
    );
    assert!(is_state(
        &cash::close(app, &c, session.id, dec(0), None, &cur_session.state_token).await
    ));
    assert!(cash::current(app, &c, branch).await.unwrap().is_none());

    // Con el turno cerrado ya no se vende ni se mueve efectivo.
    let late = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(Qr, 2975)],
        dec(2975),
    );
    assert!(is_state(&sales::checkout(app, &c, late).await));
    assert!(is_state(
        &cash::add_movement(app, &c, session.id, Ingreso, dec(1), Some("tarde"), None).await
    ));

    // Historial: el turno cerrado con su arqueo y ventas; la bitacora incluye el cierre.
    let history = cash::history(app, &c, branch, 10).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].sales_count, 3);
    assert_eq!(history[0].difference.map(|d| d.normalize()), Some(dec(-25)));
    let log = cash::activity(app, &c, session.id).await.unwrap();
    assert_eq!(log.first().unwrap().kind, "CIERRE");

    cleanup(admin, tenant, &[cashier, waiter]).await;
}

/// Proveedores: crear, NIT duplicado (409), RLS por tenant y permisos (cajero 403).
async fn supplier_checks(app: &PgPool, admin: &PgPool) {
    let tenant_a = seed_tenant(admin).await;
    let tenant_b = seed_tenant(admin).await;
    let owner_a = seed_user(admin).await;
    let cashier_a = seed_user(admin).await;
    let owner_b = seed_user(admin).await;

    seed_membership(admin, tenant_a, None, owner_a, "DUENO").await;
    seed_membership(admin, tenant_a, None, cashier_a, "CAJERO").await;
    seed_membership(admin, tenant_b, None, owner_b, "DUENO").await;

    let c_owner_a = claims(owner_a, Some(tenant_a), None);
    let c_cashier_a = claims(cashier_a, Some(tenant_a), None);
    let c_owner_b = claims(owner_b, Some(tenant_b), None);

    // 1. Cajero no puede crear proveedor (403)
    let req = procurement::CreateSupplierRequest {
        name: "Distribuidora Los Alpes".into(),
        tax_id: Some("900123456-1".into()),
        contact_name: Some("Carlos".into()),
        phone: Some("3001234567".into()),
        email: Some("alpes@test.com".into()),
        address: Some("Calle 10 # 20-30".into()),
        notes: None,
    };
    assert!(is_forbidden(
        &procurement::create_supplier(app, &c_cashier_a, req.clone()).await
    ));

    // 2. Dueño crea proveedor exitosamente
    let sup1 = procurement::create_supplier(app, &c_owner_a, req.clone())
        .await
        .unwrap();
    assert_eq!(sup1.name, "Distribuidora Los Alpes");
    assert_eq!(sup1.tax_id.as_deref(), Some("900123456-1"));
    assert!(sup1.active);

    // 3. NIT duplicado en el mismo tenant da 409
    let req_dup = procurement::CreateSupplierRequest {
        name: "Otro Alpes".into(),
        tax_id: Some("900123456-1".into()),
        contact_name: None,
        phone: None,
        email: None,
        address: None,
        notes: None,
    };
    assert!(is_conflict(
        &procurement::create_supplier(app, &c_owner_a, req_dup).await
    ));

    // 4. Mismo NIT en otro tenant (tenant_b) se permite (aislamiento por tenant)
    let sup_b = procurement::create_supplier(app, &c_owner_b, req.clone())
        .await
        .unwrap();
    assert_eq!(sup_b.tax_id.as_deref(), Some("900123456-1"));

    // 5. Tenant B no ve los proveedores de Tenant A (aislamiento RLS)
    let list_b = procurement::list_suppliers(app, &c_owner_b, None, false)
        .await
        .unwrap();
    assert_eq!(list_b.len(), 1);
    assert_eq!(list_b[0].id, sup_b.id);

    let list_a = procurement::list_suppliers(app, &c_owner_a, None, false)
        .await
        .unwrap();
    assert_eq!(list_a.len(), 1);
    assert_eq!(list_a[0].id, sup1.id);

    // 6. Editar proveedor (incluido active)
    let update_req = procurement::UpdateSupplierRequest {
        name: "Distribuidora Los Alpes SAS".into(),
        tax_id: Some("900123456-1".into()),
        contact_name: Some("Carlos Gomez".into()),
        phone: Some("3111111111".into()),
        email: Some("alpes_sas@test.com".into()),
        address: Some("Cra 15 # 45-67".into()),
        notes: Some("Excelente proveedor".into()),
        active: Some(false),
    };
    let updated = procurement::update_supplier(app, &c_owner_a, sup1.id, update_req)
        .await
        .unwrap();
    assert_eq!(updated.name, "Distribuidora Los Alpes SAS");
    assert!(!updated.active);

    let list_active = procurement::list_suppliers(app, &c_owner_a, None, false)
        .await
        .unwrap();
    assert!(list_active.is_empty());
    let list_all = procurement::list_suppliers(app, &c_owner_a, None, true)
        .await
        .unwrap();
    assert_eq!(list_all.len(), 1);

    cleanup(admin, tenant_a, &[owner_a, cashier_a]).await;
    cleanup(admin, tenant_b, &[owner_b]).await;
}

/// Compras: creacion, impuestos, kardex, regla de ultimo costo, idempotencia y estados.
async fn purchase_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;

    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let sup = procurement::create_supplier(
        app,
        &c_owner,
        procurement::CreateSupplierRequest {
            name: "Distribuidora Central".into(),
            tax_id: Some("800999888-1".into()),
            contact_name: None,
            phone: None,
            email: None,
            address: None,
            notes: None,
        },
    )
    .await
    .unwrap();

    let prod_iva = seed_product(admin, tenant, "Gaseosa 350ml", "UND", Some("IVA19"), false).await;
    let prod_exento = seed_product(admin, tenant, "Pan tajado", "UND", None, false).await;
    let prod_compuesto = seed_product(admin, tenant, "Combo Hamburguesa", "UND", None, true).await;

    // 1. Cajero no puede crear compra (403)
    let purchase_id = Uuid::new_v4();
    let req_cajero = procurement::CreatePurchaseRequest {
        id: purchase_id,
        branch_id: branch,
        supplier_id: sup.id,
        document_number: Some("FAC-001".into()),
        supplier_invoice_date: None,
        notes: None,
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_iva,
            qty: dec(10),
            unit_cost: dec(2000),
        }],
    };
    assert!(is_forbidden(
        &procurement::create_purchase(app, &c_cashier, req_cajero).await
    ));

    // 2. Producto compuesto es rechazado con 422
    let req_compuesto = procurement::CreatePurchaseRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        supplier_id: sup.id,
        document_number: Some("FAC-002".into()),
        supplier_invoice_date: None,
        notes: None,
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_compuesto,
            qty: dec(1),
            unit_cost: dec(5000),
        }],
    };
    assert!(matches!(
        procurement::create_purchase(app, &c_owner, req_compuesto).await,
        Err(ApiError::Domain(DomainError::Validation(_)))
    ));

    // 3. Crear compra con IVA 19% y exento
    let req_valida = procurement::CreatePurchaseRequest {
        id: purchase_id,
        branch_id: branch,
        supplier_id: sup.id,
        document_number: Some("FAC-100".into()),
        supplier_invoice_date: None,
        notes: Some("Compra semanal".into()),
        items: vec![
            procurement::PurchaseItemRequest {
                product_id: prod_iva,
                qty: dec(10),
                unit_cost: dec(2000),
            },
            procurement::PurchaseItemRequest {
                product_id: prod_exento,
                qty: dec(5),
                unit_cost: dec(4000),
            },
        ],
    };
    let created = procurement::create_purchase(app, &c_owner, req_valida.clone())
        .await
        .unwrap();
    assert_eq!(created.purchase.status, "PENDIENTE");
    assert_eq!(created.purchase.subtotal.normalize(), dec(40000));
    assert_eq!(created.purchase.tax_amount.normalize(), dec(3800));
    assert_eq!(created.purchase.total_amount.normalize(), dec(43800));
    assert_eq!(created.purchase.items_count, 2);

    // 4. Idempotencia: mismo ID y cuerpo devuelve 200 con la compra existente
    let replayed = procurement::create_purchase(app, &c_owner, req_valida.clone())
        .await
        .unwrap();
    assert_eq!(replayed.purchase.id, purchase_id);
    assert_eq!(replayed.purchase.total_amount.normalize(), dec(43800));

    // Mismo ID pero cuerpo distinto da 409
    let mut diff_req = req_valida.clone();
    diff_req.notes = Some("Nota distinta".into());
    assert!(is_conflict(
        &procurement::create_purchase(app, &c_owner, diff_req).await
    ));

    // 5. Cajero no puede recibir compra (403)
    assert!(is_forbidden(
        &procurement::receive_purchase(app, &c_cashier, purchase_id).await
    ));

    // 6. Recibir compra pasa a RECIBIDA, suma stock, kàrdex y actualiza cost_price
    let received = procurement::receive_purchase(app, &c_owner, purchase_id)
        .await
        .unwrap();
    assert_eq!(received.purchase.status, "RECIBIDA");
    assert!(received.purchase.received_at.is_some());

    let stock_iva = inventory::available(app, &c_owner, branch, prod_iva)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_iva.normalize(), dec(10));
    let stock_exento = inventory::available(app, &c_owner, branch, prod_exento)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_exento.normalize(), dec(5));

    let kardex = inventory::movements(app, &c_owner, branch).await.unwrap();
    let compra_moves: Vec<_> = kardex
        .iter()
        .filter(|m| m.ref_type.as_deref() == Some("COMPRA") && m.ref_id == Some(purchase_id))
        .collect();
    assert_eq!(compra_moves.len(), 2);
    assert!(compra_moves.iter().all(|m| m.movement_type == "REPOSICION"));
    assert!(
        compra_moves
            .iter()
            .all(|m| m.cause.as_deref() == Some("Compra FAC-100"))
    );

    let cost_iva: Option<Decimal> =
        sqlx::query_scalar("select cost_price from products where id = $1")
            .bind(prod_iva)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(cost_iva.map(|d| d.normalize()), Some(dec(2000)));

    let cost_exento: Option<Decimal> =
        sqlx::query_scalar("select cost_price from products where id = $1")
            .bind(prod_exento)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(cost_exento.map(|d| d.normalize()), Some(dec(4000)));

    // 7. Recibir dos veces da 409
    assert!(is_conflict(
        &procurement::receive_purchase(app, &c_owner, purchase_id).await
    ));

    // 8. No se puede anular una compra ya recibida (409)
    assert!(is_conflict(
        &procurement::cancel_purchase(app, &c_owner, purchase_id).await
    ));

    // 9. Anular una compra desde PENDIENTE
    let cancel_id = Uuid::new_v4();
    let req_cancel = procurement::CreatePurchaseRequest {
        id: cancel_id,
        branch_id: branch,
        supplier_id: sup.id,
        document_number: Some("FAC-CANCEL".into()),
        supplier_invoice_date: None,
        notes: None,
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_iva,
            qty: dec(1),
            unit_cost: dec(2000),
        }],
    };
    procurement::create_purchase(app, &c_owner, req_cancel)
        .await
        .unwrap();
    let cancelled = procurement::cancel_purchase(app, &c_owner, cancel_id)
        .await
        .unwrap();
    assert_eq!(cancelled.purchase.status, "ANULADA");

    // 10. Summary de compras
    let summary = procurement::purchase_summary(app, &c_owner, Some(branch), None)
        .await
        .unwrap();
    assert_eq!(summary.total_purchased_month.normalize(), dec(43800));
    assert_eq!(summary.active_suppliers, 1);

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn platform_checks(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // 1. Permisos granulares
    let all_perms = permissions::list_permissions(app, &c_owner).await.unwrap();
    assert!(all_perms.iter().any(|p| p.code == "pos:sell"));
    assert!(all_perms.iter().any(|p| p.code == "pos:price_override"));

    // [P] PUT /v1/roles/{code}/permissions: solo DUENO, y nunca sobre DUENO
    let cashier_try_override = permissions::update_role_permissions(
        app,
        &c_cashier,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:discount".into(),
                allowed: false,
            }],
        },
    )
    .await;
    assert!(is_forbidden(&cashier_try_override));

    let owner_try_override_owner = permissions::update_role_permissions(
        app,
        &c_owner,
        "DUENO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:sell".into(),
                allowed: false,
            }],
        },
    )
    .await;
    assert!(is_conflict(&owner_try_override_owner) || is_state(&owner_try_override_owner));

    // Dueño desactiva pos:discount para CAJERO
    let updated_cajero = permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:discount".into(),
                allowed: false,
            }],
        },
    )
    .await
    .unwrap();
    let disc_perm = updated_cajero
        .permissions
        .iter()
        .find(|p| p.code == "pos:discount")
        .unwrap();
    assert!(!disc_perm.allowed);
    assert!(disc_perm.is_override);

    // 2. Usuarios y roles
    // [P] El dueño no puede quitarse su propio rol
    let owner_membership_id: Uuid =
        sqlx::query_scalar("select id from memberships where tenant_id = $1 and user_id = $2")
            .bind(tenant)
            .bind(owner)
            .fetch_one(admin)
            .await
            .unwrap();

    let try_remove_owner_role = users::update_user_role(
        app,
        &c_owner,
        owner_membership_id,
        users::UpdateUserRoleRequest {
            role: "CAJERO".into(),
        },
    )
    .await;
    assert!(is_conflict(&try_remove_owner_role) || is_state(&try_remove_owner_role));

    // Invitar usuario nuevo
    let invited = users::invite_user(
        app,
        &c_owner,
        users::InviteUserRequest {
            email: "nuevo.empleado@ejemplo.com".into(),
            full_name: "Nuevo Empleado".into(),
            role: "MESERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .unwrap();
    assert_eq!(invited.email, "nuevo.empleado@ejemplo.com");
    assert_eq!(invited.status, "PENDIENTE");

    // [P] Solo el DUENO puede asignar o quitar el rol DUENO, cambiarle el rol a un dueño o desactivarlo.
    // ADMIN_CAJA no puede subir a nadie a dueño ni tocar a un dueño.
    let admin_user = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), admin_user, "ADMIN_CAJA").await;
    let c_admin = claims(admin_user, Some(tenant), Some(branch));

    let cashier_membership_id: Uuid =
        sqlx::query_scalar("select id from memberships where tenant_id = $1 and user_id = $2")
            .bind(tenant)
            .bind(cashier)
            .fetch_one(admin)
            .await
            .unwrap();

    // 1. ADMIN_CAJA intenta invitar a alguien como DUENO -> 403
    let admin_invite_owner = users::invite_user(
        app,
        &c_admin,
        users::InviteUserRequest {
            email: "otro.dueno@ejemplo.com".into(),
            full_name: "Otro Dueno".into(),
            role: "DUENO".into(),
            branch_id: None,
        },
    )
    .await;
    assert!(is_forbidden(&admin_invite_owner));

    // 2. ADMIN_CAJA intenta subir a un cajero a DUENO -> 403
    let admin_promote_owner = users::update_user_role(
        app,
        &c_admin,
        cashier_membership_id,
        users::UpdateUserRoleRequest {
            role: "DUENO".into(),
        },
    )
    .await;
    assert!(is_forbidden(&admin_promote_owner));

    // 3. ADMIN_CAJA intenta cambiarle el rol a un DUENO -> 403
    let admin_demote_owner = users::update_user_role(
        app,
        &c_admin,
        owner_membership_id,
        users::UpdateUserRoleRequest {
            role: "CAJERO".into(),
        },
    )
    .await;
    assert!(is_forbidden(&admin_demote_owner));

    // 4. ADMIN_CAJA intenta desactivar a un DUENO -> 403
    let admin_deactivate_owner = users::update_user_status(
        app,
        &c_admin,
        owner_membership_id,
        users::UpdateUserStatusRequest { active: false },
    )
    .await;
    assert!(is_forbidden(&admin_deactivate_owner));

    // [P] membership_tx debe ignorar las membresías con active = false: un usuario desactivado recibe 403.
    users::update_user_status(
        app,
        &c_owner,
        cashier_membership_id,
        users::UpdateUserStatusRequest { active: false },
    )
    .await
    .unwrap();

    let deactivated_cashier_op = tenancy::begin_in_branch(app, &c_cashier, branch).await;
    assert!(is_forbidden(&deactivated_cashier_op));

    // Reactivar cajero para continuar con las pruebas
    users::update_user_status(
        app,
        &c_owner,
        cashier_membership_id,
        users::UpdateUserStatusRequest { active: true },
    )
    .await
    .unwrap();

    // [P] Permisos base a SUPERVISOR, BODEGA y REPARTIDOR
    assert!(tenancy::base_allows("SUPERVISOR", "pos:sell"));
    assert!(tenancy::base_allows("SUPERVISOR", "pos:refund"));
    assert!(tenancy::base_allows("SUPERVISOR", "inventory:adjust"));
    assert!(tenancy::base_allows("SUPERVISOR", "reports:read"));

    assert!(tenancy::base_allows("BODEGA", "inventory:read"));
    assert!(tenancy::base_allows("BODEGA", "inventory:adjust"));
    assert!(tenancy::base_allows("BODEGA", "inventory:adjust_approve"));
    assert!(tenancy::base_allows("BODEGA", "inventory:transfer"));
    assert!(tenancy::base_allows("BODEGA", "purchases:receive"));
    assert!(!tenancy::base_allows("BODEGA", "pos:sell"));

    assert!(tenancy::base_allows("REPARTIDOR", "pos:sell"));
    assert!(tenancy::base_allows("REPARTIDOR", "inventory:read"));
    assert!(!tenancy::base_allows("REPARTIDOR", "pos:refund"));

    // 3. Ajustes por sucursal
    // [P] pos.allow_price_override (false por defecto) y pos.max_discount_pct (15)
    let prod = seed_product(admin, tenant, "Gaseosa Ajustes", "UND", None, false).await;
    seed_branch_product(admin, tenant, branch, prod, None, Some("5000")).await;
    seed_stock(admin, tenant, branch, prod, "50", "0").await;

    let def_reg = default_register(admin, branch).await;
    // Abrir turno de caja
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    // [P] un usuario no puede abrir segundo turno en la sucursal: da conflicto
    let second_session = cash::open(app, &c_cashier, branch, def_reg, dec(20000)).await;
    assert!(is_conflict(&second_session) || is_state(&second_session));

    // [P] pos.allow_price_override: venta con price override rechazada porque está en false
    let sale_override_req = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(prod),
            custom_name: None,
            qty: dec(1),
            unit_price_override: Some(dec(4500)),
            discount_pct: None,
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4500),
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let try_override_sale = sales::checkout(app, &c_cashier, sale_override_req.clone()).await;
    assert!(is_validation(&try_override_sale));

    // [P] pos.max_discount_pct (15): venta con 20% de descuento rechazada
    let sale_disc_req = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(prod),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: Some(dec(20)),
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4000),
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let try_high_disc = sales::checkout(app, &c_cashier, sale_disc_req).await;
    assert!(is_validation(&try_high_disc));

    // Venta con 10% de descuento es aceptada (10 <= 15)
    let valid_sale_id = Uuid::new_v4();
    let valid_sale_req = SaleRequest {
        id: valid_sale_id,
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(prod),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: Some(dec(10)),
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4500),
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let ok_sale = match sales::checkout(app, &c_cashier, valid_sale_req.clone())
        .await
        .unwrap()
    {
        Checkout::Created(r) => r,
        Checkout::Replayed(r) => r,
    };
    assert!(ok_sale.receipt_number.starts_with("POS-"));

    // [P] Override que le quita pos:sell al cajero lo bloquea (403)
    permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:sell".into(),
                allowed: false,
            }],
        },
    )
    .await
    .unwrap();

    let try_blocked_sale = sales::checkout(app, &c_cashier, valid_sale_req.clone()).await;
    assert!(is_forbidden(&try_blocked_sale));

    // Restaurar pos:sell para el cajero
    permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:sell".into(),
                allowed: true,
            }],
        },
    )
    .await
    .unwrap();

    // [P] Cambio de precio exige pos:price_override además del ajuste de la sucursal
    let mut allow_override_settings = settings::BranchSettings::default();
    allow_override_settings.pos.allow_price_override = true;
    settings::update_settings(
        app,
        &c_owner,
        settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: allow_override_settings,
        },
    )
    .await
    .unwrap();

    let try_cashier_override_without_perm =
        sales::checkout(app, &c_cashier, sale_override_req.clone()).await;
    assert!(is_forbidden(&try_cashier_override_without_perm));

    // [P] Descuento exige pos:discount: quitamos pos:discount por override -> 403 Forbidden
    permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:discount".into(),
                allowed: false,
            }],
        },
    )
    .await
    .unwrap();

    let try_disc_without_perm = sales::checkout(app, &c_cashier, valid_sale_req.clone()).await;
    assert!(is_forbidden(&try_disc_without_perm));

    // Restaurar pos:discount para el cajero
    permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:discount".into(),
                allowed: true,
            }],
        },
    )
    .await
    .unwrap();

    // [P] pos.refund_requires_approval (true): cajero no puede cancelar sin aprobacion admin
    let unbilled_order =
        orders::create_order(app, &c_cashier, branch, "MOSTRADOR", None, None, None)
            .await
            .unwrap();
    let try_cashier_cancel_unbilled = orders::cancel(app, &c_cashier, unbilled_order.id).await;
    assert!(is_conflict(&try_cashier_cancel_unbilled) || is_state(&try_cashier_cancel_unbilled));

    // [P] cash.close_tolerance: cerrar caja con diferencia sin nota obligatoria si supera tolerancia
    // Configuramos close_tolerance = 1000
    let mut custom_settings = settings::BranchSettings::default();
    custom_settings.cash.close_tolerance = dec(1000);
    settings::update_settings(
        app,
        &c_owner,
        settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: custom_settings,
        },
    )
    .await
    .unwrap();

    let cur_session = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    // Diferencia esperada: contado 40000 vs esperado (50000 base + 4500 venta = 54500)
    // Diferencia ~14500 > 1000 de tolerancia, sin notas -> validation error
    let try_close_no_notes = cash::close(
        app,
        &c_cashier,
        cur_session.session.id,
        dec(40000),
        None,
        &cur_session.state_token,
    )
    .await;
    assert!(is_validation(&try_close_no_notes));

    // Cerrando con nota -> éxito
    let ok_close = cash::close(
        app,
        &c_cashier,
        cur_session.session.id,
        dec(40000),
        Some("Faltante justificado por pago de flete menor".into()),
        &cur_session.state_token,
    )
    .await
    .unwrap();
    assert_eq!(ok_close.session.counted_cash, Some(dec(40000)));

    // [P] inventory.allow_negative_stock (false): ajuste negativo que deja existencia < 0 es rechazado
    let try_neg_adjust =
        inventory::adjust(app, &c_owner, branch, prod, dec(-100), "Ajuste excesivo").await;
    assert!(is_validation(&try_neg_adjust));

    // [P] F0.5 Auditoría: solo insert y select. UPDATE o DELETE fallan por trigger
    let update_audit_try =
        sqlx::query("update audit_log set action = 'MODIFICADO' where tenant_id = $1")
            .bind(tenant)
            .execute(admin)
            .await;
    assert!(update_audit_try.is_err());

    let delete_audit_try = sqlx::query("delete from audit_log where tenant_id = $1")
        .bind(tenant)
        .execute(admin)
        .await;
    assert!(delete_audit_try.is_err());

    // [P] F0.6 Numeración unificada: 20 llamadas concurrentes a next_tx no repiten números
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let app_clone = app.clone();
        let c_clone = c_owner.clone();
        tasks.push(tokio::spawn(async move {
            let (scope, mut tx) = tenancy::begin_in_branch(&app_clone, &c_clone, branch)
                .await
                .unwrap();
            let (prefix, num) = sequences::next_tx(&mut tx, &scope, branch, "COTIZACION")
                .await
                .unwrap();
            tx.commit().await.unwrap();
            (prefix, num)
        }));
    }

    let mut numbers = Vec::new();
    for t in tasks {
        let (prefix, num) = t.await.unwrap();
        assert_eq!(prefix, "COT");
        numbers.push(num);
    }
    numbers.sort();
    assert_eq!(numbers.len(), 20);
    for (i, num) in numbers.iter().enumerate() {
        assert_eq!(*num, (i + 1) as i64);
    }

    // F0.7 Outbox
    let mut outbox_tx = crate::db::begin_tenant_tx(app, &c_owner).await.unwrap();
    let outbox_id = outbox::enqueue_tx(
        &mut outbox_tx,
        tenant,
        "CORREO",
        serde_json::json!({ "to": "cliente@test.com", "subject": "Factura" }),
    )
    .await
    .unwrap();
    outbox_tx.commit().await.unwrap();

    let outbox_status: String = sqlx::query_scalar("select status from outbox where id = $1")
        .bind(outbox_id)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(outbox_status, "PENDIENTE");

    // [P] F0.3: después de 5 intentos fallidos en 15 minutos, responde 429
    let test_rate_email = "bloqueo.test@ejemplo.com";
    for i in 1..=4 {
        let res = crate::auth::check_and_record_login_attempt(
            app,
            test_rate_email,
            Some("192.168.1.50"),
            Some("TestAgent/1.0"),
            Some(false),
            Some("invalid password"),
        )
        .await;
        assert!(res.is_ok(), "intento {i} fallido dentro del límite");
    }

    // Pre-check antes del 5to intento todavía pasa (4 fallos < 5)
    let pre_check_ok =
        crate::auth::check_and_record_login_attempt(app, test_rate_email, None, None, None, None)
            .await;
    assert!(pre_check_ok.is_ok());

    // 5to intento fallido responde 429 TooManyRequests
    let fifth_attempt = crate::auth::check_and_record_login_attempt(
        app,
        test_rate_email,
        Some("192.168.1.50"),
        Some("TestAgent/1.0"),
        Some(false),
        Some("invalid password"),
    )
    .await;
    assert!(is_too_many_requests(&fifth_attempt));

    // Pre-check posterior responde 429 bloqueando el login
    let blocked_pre_check =
        crate::auth::check_and_record_login_attempt(app, test_rate_email, None, None, None, None)
            .await;
    assert!(is_too_many_requests(&blocked_pre_check));

    cleanup(admin, tenant, &[owner, cashier, admin_user]).await;
}

/// F1.0: Permisos en el contexto
async fn context_returns_permissions(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let state = AppState {
        pool: app.clone(),
        jwt: crate::auth::JwtValidator::new("http://localhost:54321"),
    };

    // [P] El dueño recibe todos los permisos de la tabla permissions
    let owner_resp = routes::context(
        axum::extract::State(state.clone()),
        crate::auth::AuthUser { claims: c_owner },
    )
    .await
    .unwrap();
    let owner_json = owner_resp.0;
    let owner_perms: Vec<String> =
        serde_json::from_value(owner_json["permissions"].clone()).unwrap();

    let all_perms: Vec<String> = sqlx::query_scalar("select code from permissions order by code")
        .fetch_all(admin)
        .await
        .unwrap();
    assert_eq!(owner_perms, all_perms);

    // [P] Un cajero recibe exactamente los permisos base de CAJERO
    let cashier_resp = routes::context(
        axum::extract::State(state),
        crate::auth::AuthUser { claims: c_cashier },
    )
    .await
    .unwrap();
    let cashier_json = cashier_resp.0;
    let cashier_perms: Vec<String> =
        serde_json::from_value(cashier_json["permissions"].clone()).unwrap();

    let role_id: i32 = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL') and tima_codigo = 'CAJERO'"
    )
    .fetch_one(admin)
    .await
    .unwrap();
    let mut base_cajero_perms: Vec<String> =
        sqlx::query_scalar("select permission_code from role_permissions where role_id = $1")
            .bind(role_id)
            .fetch_all(admin)
            .await
            .unwrap();
    base_cajero_perms.sort();

    assert_eq!(cashier_perms, base_cajero_perms);

    cleanup(admin, tenant, &[owner, cashier]).await;
}

/// F1.1 Cajas registradoras
async fn f1_registers(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;

    // Una sucursal recién insertada tiene CAJA-1
    let reg1_id: Uuid = sqlx::query_scalar(
        "select id from cash_registers where branch_id = $1 and code = 'CAJA-1'",
    )
    .bind(branch)
    .fetch_one(admin)
    .await
    .expect("branch must automatically have CAJA-1");

    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let cashier1 = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier1, "CAJERO").await;
    let c_cashier1 = claims(cashier1, Some(tenant), Some(branch));

    let cashier2 = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier2, "CAJERO").await;
    let c_cashier2 = claims(cashier2, Some(tenant), Some(branch));

    // [P] Un cajero no puede crear cajas: 403
    let cashier_create_try = registers::create(
        app,
        &c_cashier1,
        registers::CreateRegisterRequest {
            branch_id: branch,
            name: "Caja 2".to_string(),
            code: "CAJA-2".to_string(),
            default_opening: dec(100000),
        },
    )
    .await;
    assert!(is_forbidden(&cashier_create_try));

    // Dueño crea Caja 2
    let reg2 = registers::create(
        app,
        &c_owner,
        registers::CreateRegisterRequest {
            branch_id: branch,
            name: "Caja 2".to_string(),
            code: "CAJA-2".to_string(),
            default_opening: dec(100000),
        },
    )
    .await
    .expect("owner can create register");
    let reg2_id = reg2.id;

    // Cajero 1 abre turno en Caja 1
    let s1 = cash::open(app, &c_cashier1, branch, reg1_id, dec(50000))
        .await
        .expect("cashier 1 can open Caja 1");
    assert_eq!(s1.register_id, reg1_id);
    assert_eq!(s1.register_name, "Caja 1");
    assert_eq!(s1.register_code, "CAJA-1");

    // [P] Dos usuarios no pueden abrir turno en la misma caja: el segundo recibe 409 REGISTER_BUSY
    let try_same_reg = cash::open(app, &c_cashier2, branch, reg1_id, dec(20000)).await;
    match try_same_reg {
        Err(ApiError::Domain(DomainError::Coded {
            status, code, data, ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "REGISTER_BUSY");
            assert!(data.is_some());
        }
        other => panic!("Expected Coded REGISTER_BUSY, got {:?}", other),
    }

    // [P] Un usuario no puede abrir dos turnos en la misma sucursal, ni siquiera en cajas distintas: 409 USER_HAS_OPEN_SESSION
    let try_second_session = cash::open(app, &c_cashier1, branch, reg2_id, dec(20000)).await;
    match try_second_session {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "USER_HAS_OPEN_SESSION");
        }
        other => panic!("Expected Coded USER_HAS_OPEN_SESSION, got {:?}", other),
    }

    // [P] No se puede desactivar una caja con un turno abierto: 409 REGISTER_HAS_OPEN_SESSION
    let try_deactivate = registers::update(
        app,
        &c_owner,
        reg1_id,
        registers::UpdateRegisterRequest {
            name: "Caja 1".to_string(),
            code: "CAJA-1".to_string(),
            default_opening: dec(50000),
            active: false,
        },
    )
    .await;
    match try_deactivate {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "REGISTER_HAS_OPEN_SESSION");
        }
        other => panic!("Expected Coded REGISTER_HAS_OPEN_SESSION, got {:?}", other),
    }

    // Cerrar el turno de Caja 1
    let cur_s1 = cash::current(app, &c_cashier1, branch)
        .await
        .unwrap()
        .unwrap();
    cash::close(
        app,
        &c_cashier1,
        s1.id,
        dec(50000),
        None,
        &cur_s1.state_token,
    )
    .await
    .expect("close session");

    // Ahora sí se puede desactivar Caja 1
    let deactivated = registers::update(
        app,
        &c_owner,
        reg1_id,
        registers::UpdateRegisterRequest {
            name: "Caja 1".to_string(),
            code: "CAJA-1".to_string(),
            default_opening: dec(50000),
            active: false,
        },
    )
    .await
    .expect("deactivating register succeeds when closed");
    assert!(!deactivated.active);

    cleanup(admin, tenant, &[owner, cashier1, cashier2]).await;
}

/// F1.2 Tipos de movimiento de caja y esperado
async fn f1_cash_kinds(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let reg_id = default_register(admin, branch).await;
    let opening = dec(500000);
    let s = cash::open(app, &c_owner, branch, reg_id, opening)
        .await
        .unwrap();

    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();

    // [P] El esperado suma y resta los 9 tipos
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::Ingreso,
        dec(10000),
        "Ingreso test",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::AbonoCartera,
        dec(20000),
        "Abono cartera",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::AbonoSeparado,
        dec(30000),
        "Abono separado",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::RecargaSaldo,
        dec(40000),
        "Recarga saldo",
        None,
        None,
        None,
    )
    .await
    .unwrap();

    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::Retiro,
        dec(15000),
        "Retiro banco",
        Some("CONSIGNACION"),
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::Devolucion,
        dec(5000),
        "Devolucion cliente",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::PropinaPagada,
        dec(8000),
        "Mesero Juan",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::PagoProveedor,
        dec(12000),
        "Pago proveedor verduras",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::Gasto,
        dec(10000),
        "Compra bombillos",
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let summary = cash::summary_tx(&mut tx, s.id).await.unwrap();
    assert_eq!(summary.expected_cash, dec(550000));
    assert_eq!(summary.cash_in, dec(100000));
    assert_eq!(summary.cash_out, dec(50000));
    assert_eq!(summary.movements_by_kind.len(), 9);

    // [P] Un retiro mayor al esperado: 409 CASH_INSUFFICIENT
    let try_huge_withdraw = cash::record_movement_tx(
        &mut tx,
        &scope,
        s.id,
        cash::CashMovementKind::Retiro,
        dec(600000),
        "Retiro excesivo",
        Some("CONSIGNACION"),
        None,
        None,
    )
    .await;
    match try_huge_withdraw {
        Err(ApiError::Domain(DomainError::Coded {
            status, code, data, ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CASH_INSUFFICIENT");
            assert_eq!(data, Some(serde_json::json!({ "available": dec(550000) })));
        }
        other => panic!("Expected Coded CASH_INSUFFICIENT, got {:?}", other),
    }

    tx.commit().await.unwrap();

    // [P] El endpoint público rechaza kind = DEVOLUCION: 422 CASH_KIND_NOT_ALLOWED
    let try_pub_dev = cash::add_movement(
        app,
        &c_owner,
        s.id,
        cash::CashMovementKind::Devolucion,
        dec(5000),
        Some("Devolucion"),
        None,
    )
    .await;
    match try_pub_dev {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CASH_KIND_NOT_ALLOWED");
        }
        other => panic!("Expected Coded CASH_KIND_NOT_ALLOWED, got {:?}", other),
    }

    // [P] Un RETIRO sin reason_code: 422
    let try_no_rc = cash::add_movement(
        app,
        &c_owner,
        s.id,
        cash::CashMovementKind::Retiro,
        dec(1000),
        Some("Sin motivo"),
        None,
    )
    .await;
    assert!(is_validation(&try_no_rc));

    cleanup(admin, tenant, &[owner]).await;
}

async fn f1_cash_state(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let reg = registers::create(
        app,
        &c_owner,
        registers::CreateRegisterRequest {
            branch_id: branch,
            name: "Caja State".into(),
            code: "CAJA-ST".into(),
            default_opening: dec(50000),
        },
    )
    .await
    .unwrap();

    let session = cash::open(app, &c_cashier, branch, reg.id, dec(50000))
        .await
        .unwrap();

    let soda = seed_product(admin, tenant, "Soda State", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "100", "0").await;

    // 1. Lee el token inicial del turno abierto
    let summary1 = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    let old_token = summary1.state_token.clone();
    assert!(!old_token.is_empty());

    // 2. Registra una venta en el turno mientras se "arquea"
    let sale_id = Uuid::new_v4();
    let s = sale(
        sale_id,
        branch,
        &[(soda, 1)],
        &[(PaymentMethod::Efectivo, 2975)],
        dec(2975),
    );
    sales::checkout(app, &c_cashier, s).await.unwrap();

    // 3. Intenta cerrar con el token viejo: 409 CASH_STATE_CHANGED sin data
    let try_old = cash::close(app, &c_cashier, session.id, dec(52975), None, &old_token).await;

    match try_old {
        Err(ApiError::Domain(DomainError::Coded {
            status, code, data, ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CASH_STATE_CHANGED");
            assert!(data.is_none());
        }
        other => panic!("Expected Coded CASH_STATE_CHANGED, got {:?}", other),
    }

    // 4. Con el token nuevo el cierre funciona
    let summary2 = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(summary2.state_token, old_token);

    let closed = cash::close(
        app,
        &c_cashier,
        session.id,
        dec(52975),
        None,
        &summary2.state_token,
    )
    .await
    .expect("close with updated token should succeed");

    assert!(closed.session.closed_at.is_some());

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn f1_sale_customer(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;

    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    let soda = seed_product(admin, tenant, "Soda Cust", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "100", "0").await;

    // Crear un cliente
    let customer = customers::create(
        app,
        &c_cashier,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("12345678".into()),
            dv: None,
            name: "Cliente Prueba".into(),
            phone: Some("3001234567".into()),
            email: Some("cliente@prueba.com".into()),
        },
    )
    .await
    .unwrap();

    // 1. [P] Cliente y alias juntos: 422 SALE_CUSTOMER_AND_ALIAS
    let mut sale_both = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(PaymentMethod::Efectivo, 2975)],
        dec(2975),
    );
    sale_both.customer_id = Some(customer.id);
    sale_both.customer_alias = Some("Mesa 4".into());
    let try_both = sales::checkout(app, &c_cashier, sale_both).await;
    match try_both {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "SALE_CUSTOMER_AND_ALIAS");
        }
        other => panic!("Expected Coded SALE_CUSTOMER_AND_ALIAS, got {:?}", other),
    }

    // 2. [P] Con allow_alias_sales = false, una venta con alias: 422 SALE_ALIAS_NOT_ALLOWED
    sqlx::query(
        "update branch_settings set settings = jsonb_set(settings, '{pos,allow_alias_sales}', 'false') where branch_id = $1"
    )
    .bind(branch)
    .execute(admin)
    .await
    .unwrap();

    let mut sale_alias = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(PaymentMethod::Efectivo, 2975)],
        dec(2975),
    );
    sale_alias.customer_alias = Some("Mesa 4".into());
    let try_alias = sales::checkout(app, &c_cashier, sale_alias).await;
    match try_alias {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "SALE_ALIAS_NOT_ALLOWED");
        }
        other => panic!("Expected Coded SALE_ALIAS_NOT_ALLOWED, got {:?}", other),
    }

    // 3. [P] Con allow_anonymous_sales = false, una venta sin cliente: 422 SALE_CUSTOMER_REQUIRED
    sqlx::query(
        "update branch_settings set settings = jsonb_set(settings, '{pos,allow_anonymous_sales}', 'false') where branch_id = $1"
    )
    .bind(branch)
    .execute(admin)
    .await
    .unwrap();

    let sale_anon = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(PaymentMethod::Efectivo, 2975)],
        dec(2975),
    );
    let try_anon = sales::checkout(app, &c_cashier, sale_anon).await;
    match try_anon {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "SALE_CUSTOMER_REQUIRED");
        }
        other => panic!("Expected Coded SALE_CUSTOMER_REQUIRED, got {:?}", other),
    }

    // Con cliente sí funciona
    let mut sale_with_customer = sale(
        Uuid::new_v4(),
        branch,
        &[(soda, 1)],
        &[(PaymentMethod::Efectivo, 2975)],
        dec(2975),
    );
    sale_with_customer.customer_id = Some(customer.id);
    let ok_sale = sales::checkout(app, &c_cashier, sale_with_customer)
        .await
        .expect("sale with customer should succeed");
    match ok_sale {
        sales::Checkout::Created(resp) => {
            assert_eq!(resp.order.customer_id, Some(customer.id));
            assert_eq!(resp.order.customer_name.as_deref(), Some("Cliente Prueba"));
        }
        _ => panic!("Expected Created"),
    }

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn f1_pricing_db(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));
    let c_owner = claims(owner, Some(tenant), None);

    let soda = seed_product(admin, tenant, "Gaseosa F15", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "10", "0").await;

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    // 1. Cotización: POST /v1/sales/quote
    let quote = sales::quote(
        app,
        &c_cashier,
        QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
            }],
            tip_amount: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(quote.subtotal.normalize(), dec(5000));
    assert_eq!(quote.tax_total.normalize(), dec(950));
    assert_eq!(quote.total.normalize(), dec(5950));
    assert_eq!(quote.lines.len(), 1);
    assert_eq!(quote.lines[0].line_total.normalize(), dec(5000));

    // 2. Venta con quoted_total incorrecto da 409 PRICE_CHANGED
    let bad_quote_sale = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(2),
            unit_price_override: None,
            discount_pct: None,
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5950),
            reference: None,
        }],
        quoted_total: dec(5000), // Incorrecto, real es 5950
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let err = sales::checkout(app, &c_cashier, bad_quote_sale).await;
    match err {
        Err(ApiError::Domain(DomainError::Coded {
            status, code, data, ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "PRICE_CHANGED");
            let data_val = data.expect("should have data payload");
            let total_val: Decimal = serde_json::from_value(data_val["total"].clone()).unwrap();
            assert_eq!(total_val.normalize(), dec(5950));
        }
        other => panic!("expected 409 PRICE_CHANGED, got {:?}", other),
    }

    // 3. Venta con descuento manual del 10% guarda discount_manual, line_total y orders.total_amount
    // Precio 2 * 2500 = 5000. Descuento 10% = 500. Subtotal neto = 4500. IVA 19% = 855. Total = 5355.
    let sale_id = Uuid::new_v4();
    let disc_sale = SaleRequest {
        id: sale_id,
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(2),
            unit_price_override: None,
            discount_pct: Some(dec(10)),
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5355),
            reference: None,
        }],
        quoted_total: dec(5355),
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let res = sales::checkout(app, &c_cashier, disc_sale).await.unwrap();
    match res {
        Checkout::Created(s) => {
            assert_eq!(s.order.subtotal.normalize(), dec(4500));
            assert_eq!(s.order.discount_total.normalize(), dec(500));
            assert_eq!(s.order.inc_amount.normalize(), dec(855));
            assert_eq!(s.order.total_amount.normalize(), dec(5355));
        }
        Checkout::Replayed(_) => panic!("unexpected replay"),
    }

    let (disc_m, line_d, line_t, tax_a): (Decimal, Decimal, Decimal, Decimal) = sqlx::query_as(
        "select discount_manual, line_discount, line_total, tax_amount from order_items where order_id = $1",
    )
    .bind(sale_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(disc_m.normalize(), dec(500));
    assert_eq!(line_d.normalize(), dec(500));
    assert_eq!(line_t.normalize(), dec(4500));
    assert_eq!(tax_a.normalize(), dec(855));

    // 4. Cajero sin pos:discount mandando descuento da 403
    permissions::update_role_permissions(
        app,
        &c_owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: "pos:discount".into(),
                allowed: false,
            }],
        },
    )
    .await
    .unwrap();

    let try_forbidden_disc = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: Some(dec(10)),
            tier_id: None,
            price_override_reason: None,
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(2678),
        tip_amount: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
    };
    let disc_err = sales::checkout(app, &c_cashier, try_forbidden_disc).await;
    assert!(is_forbidden(&disc_err));

    cleanup(admin, tenant, &[cashier, owner]).await;
}

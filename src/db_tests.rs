//! Pruebas de integracion contra la BD real (RLS, tenancy, catalogo).
//!
//! Se ejecutan en un unico test para compartir un runtime y los pools de
//! conexion (Supabase limita clientes en modo sesion).

#![allow(clippy::needless_update)]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiError, DomainError};
use crate::orders::OrderStatus;
use crate::sales::{Checkout, PaymentMethod, QuoteRequest, SaleItem, SalePayment, SaleRequest};
use crate::test_support::*;
use crate::{
    AppState, cash, catalog, coupons, customers, expenses, inventory, options, orders, outbox,
    payables, permissions, procurement, products, refunds, registers, routes, sales, sales_history,
    sequences, settings, tenancy, users,
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
    // Sin DATABASE_URL se omite (p. ej. cargo test sin Docker). Con ella, no
    // conectar es un error: antes se omitia en silencio y parecia que pasaba.
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("database_integration omitido: falta DATABASE_URL");
        return;
    }
    let admin = admin_pool()
        .await
        .expect("no se pudo conectar como administrador");
    let app = app_pool()
        .await
        .expect("no se pudo conectar como authenticated (¿faltan las migraciones?)");

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
    f1_tiers(&app, &admin).await;
    f1_promotions(&app, &admin).await;
    f1_coupons(&app, &admin).await;
    f1_override(&app, &admin).await;
    f1_tips(&app, &admin).await;
    f1_refunds(&app, &admin).await;
    f1_void(&app, &admin).await;
    f1_sales_history(&app, &admin).await;
    f2_products(&app, &admin).await;
    f2_options(&app, &admin).await;
    f2_archive(&app, &admin).await;
    f2_import(&app, &admin).await;
    f2_locations(&app, &admin).await;
    f2_pipeline(&app, &admin).await;
    f2_costing(&app, &admin).await;
    f2_suppliers(&app, &admin).await;
    f2_purchases(&app, &admin).await;
    f2_payables(&app, &admin).await;
    f2_expenses(&app, &admin).await;
    f2_lots(&app, &admin).await;
    f2_adjustments(&app, &admin).await;
    f2_suggestions(&app, &admin).await;
    f3_customers(&app, &admin).await;
    f3_credit_sale(&app, &admin).await;
    f3_payments(&app, &admin).await;
    f3_aging(&app, &admin).await;
    s1_onboarding(&app, &admin).await;
    s2_invitations(&app, &admin).await;
    s3_business(&app, &admin).await;
    s4_permissions(&app, &admin).await;
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

    let levels = inventory::stock_levels(app, &c, main, None).await.unwrap();
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
    let levels = inventory::stock_levels(app, &c, main, None).await.unwrap();
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
    let kx = inventory::kardex(app, &c, main, Some(rice), None, 50)
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
    let annex_kx = inventory::kardex(app, &c, annex, None, None, 50)
        .await
        .unwrap();
    assert_eq!(annex_kx.len(), 1);
    assert_eq!(annex_kx[0].movement_type, "TRASLADO_ENTRADA");
    assert_eq!(annex_kx[0].ref_id, kx[0].ref_id);

    cleanup(admin, tenant, &[owner, clerk]).await;
}

fn is_forbidden<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    matches!(r, Err(ApiError::Domain(DomainError::Forbidden)))
}

/// 409 por regla de estado: `state(..)` o un error con código (F1.0) de estado 409.
fn is_state<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    match r {
        Err(ApiError::Domain(DomainError::State(_))) => true,
        Err(e @ ApiError::Domain(DomainError::Coded { .. })) => {
            e.status() == axum::http::StatusCode::CONFLICT
        }
        _ => false,
    }
}

fn is_conflict<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    match r {
        Err(e) => e.status() == axum::http::StatusCode::CONFLICT,
        Ok(_) => false,
    }
}

/// 422: `validation(..)` o un error con código (F1.0) de estado 422.
fn is_validation<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> bool {
    match r {
        Err(ApiError::Domain(DomainError::Validation(_))) => true,
        Err(e @ ApiError::Domain(DomainError::Coded { .. })) => {
            e.status() == axum::http::StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => false,
    }
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
                custom_tax: None,
                option_ids: vec![],
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
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
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
        &cash::add_movement(
            app,
            &w,
            session.id,
            Ingreso,
            dec(1),
            Some("Base adicional"),
            None
        )
        .await
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
            ..Default::default()
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
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_iva,
            qty: dec(10),
            unit_cost: dec(2000),
            new_sale_price: None,
        }],
        ..Default::default()
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
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_compuesto,
            qty: dec(1),
            unit_cost: dec(5000),
            new_sale_price: None,
        }],
        ..Default::default()
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
        notes: Some("Compra semanal".into()),
        items: vec![
            procurement::PurchaseItemRequest {
                product_id: prod_iva,
                qty: dec(10),
                unit_cost: dec(2000),
                new_sale_price: None,
            },
            procurement::PurchaseItemRequest {
                product_id: prod_exento,
                qty: dec(5),
                unit_cost: dec(4000),
                new_sale_price: None,
            },
        ],
        ..Default::default()
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
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod_iva,
            qty: dec(1),
            unit_cost: dec(2000),
            new_sale_price: None,
        }],
        ..Default::default()
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
            role: "SUPERVISOR".into(),
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4500),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4000),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    let try_high_disc = sales::checkout(app, &c_cashier, sale_disc_req).await;
    assert!(is_validation(&try_high_disc));

    // Venta con 10% de descuento es aceptada (10 <= 15)
    let valid_sale_id = Uuid::new_v4();
    // Más arriba se le quitó pos:discount al cajero; se le devuelve para esta venta.
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(4500),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
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

    // Id nuevo: con el mismo id la venta ya registrada se devolvería sin validar nada.
    let mut disc_again = valid_sale_req.clone();
    disc_again.id = Uuid::new_v4();
    let try_disc_without_perm = sales::checkout(app, &c_cashier, disc_again).await;
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
    // El cajero no tiene pos:refund: se le niega antes de llegar a la regla del ajuste.
    assert!(
        is_forbidden(&try_cashier_cancel_unbilled),
        "{try_cashier_cancel_unbilled:?}"
    );

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
            location_id: None,
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
            location_id: None,
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
            location_id: None,
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
            location_id: None,
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
            let available: Decimal =
                serde_json::from_value(data.expect("data")["available"].clone()).unwrap();
            assert_eq!(available.normalize(), dec(550000));
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
            location_id: None,
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
            price_tier_id: None,
            ..Default::default()
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
        "insert into branch_settings (branch_id, tenant_id, data)
         select id, tenant_id, jsonb_build_object('pos', jsonb_build_object('allow_alias_sales', false))
           from branches where id = $1
         on conflict (branch_id) do update
           set data = jsonb_set(coalesce(branch_settings.data, '{}'), '{pos,allow_alias_sales}', 'false', true)"
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
        "insert into branch_settings (branch_id, tenant_id, data)
         select id, tenant_id, jsonb_build_object('pos', jsonb_build_object('allow_anonymous_sales', false))
           from branches where id = $1
         on conflict (branch_id) do update
           set data = jsonb_set(coalesce(branch_settings.data, '{}'), '{pos,allow_anonymous_sales}', 'false', true)"
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
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5950),
            reference: None,
        }],
        quoted_total: dec(5000), // Incorrecto, real es 5950
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5355),
            reference: None,
        }],
        quoted_total: dec(5355),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
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
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(5000),
            reference: None,
        }],
        quoted_total: dec(2678),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    let disc_err = sales::checkout(app, &c_cashier, try_forbidden_disc).await;
    assert!(is_forbidden(&disc_err));

    cleanup(admin, tenant, &[cashier, owner]).await;
}

async fn f1_tiers(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));
    let c_owner = claims(owner, Some(tenant), None);

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    // 1. Crear tarifas: "Paca x12" (PRESENTACION) y "Mayorista" (CLIENTE)
    let paca = crate::tiers::create_tier(
        app,
        &c_owner,
        crate::tiers::CreatePriceTierRequest {
            name: "Paca x12".into(),
            kind: "PRESENTACION".into(),
            adjustment: "DESCUENTO_PCT".into(),
            value: dec(5),
            units_per_package: Some(dec(12)),
        },
    )
    .await
    .unwrap();
    assert_eq!(paca.name, "Paca x12");

    let mayorista = crate::tiers::create_tier(
        app,
        &c_owner,
        crate::tiers::CreatePriceTierRequest {
            name: "Mayorista".into(),
            kind: "CLIENTE".into(),
            adjustment: "DESCUENTO_PCT".into(),
            value: dec(8),
            units_per_package: None,
        },
    )
    .await
    .unwrap();

    // 2. [P] TIER_KIND_IMMUTABLE: No se puede cambiar kind en PUT
    let try_change_kind = crate::tiers::update_tier(
        app,
        &c_owner,
        paca.id,
        crate::tiers::UpdatePriceTierRequest {
            name: "Paca x12 Cambiada".into(),
            kind: "CLIENTE".into(),
            adjustment: "DESCUENTO_PCT".into(),
            value: dec(5),
            units_per_package: None,
            active: true,
        },
    )
    .await;
    match try_change_kind {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "TIER_KIND_IMMUTABLE");
        }
        other => panic!("expected TIER_KIND_IMMUTABLE, got {:?}", other),
    }

    // 3. Crear productos
    let soda = seed_product(admin, tenant, "Gaseosa Tiers", "UND", None, false).await;
    set_base_price(admin, soda, "1000").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "50", "0").await;

    let bread = seed_product(admin, tenant, "Pan Barcode", "UND", None, false).await;
    sqlx::query("update products set barcode = '7701234567890' where id = $1")
        .bind(bread)
        .execute(admin)
        .await
        .unwrap();

    // 4. [P] Código de barras de presentación igual al de otro producto: 409 BARCODE_TAKEN
    let try_dup_barcode = crate::tiers::set_product_tier_prices(
        app,
        &c_owner,
        soda,
        vec![crate::tiers::SetProductTierPriceInput {
            tier_id: paca.id,
            price: None,
            units_per_package: None,
            barcode: Some("7701234567890".into()),
        }],
    )
    .await;
    match try_dup_barcode {
        Err(ApiError::Domain(DomainError::Coded { status, code, .. })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "BARCODE_TAKEN");
        }
        other => panic!("expected BARCODE_TAKEN, got {:?}", other),
    }

    // Configurar precio fijo por producto para la paca: $10.000 por paca
    crate::tiers::set_product_tier_prices(
        app,
        &c_owner,
        soda,
        vec![crate::tiers::SetProductTierPriceInput {
            tier_id: paca.id,
            price: Some(dec(10000)),
            units_per_package: None,
            barcode: Some("7709999999999".into()),
        }],
    )
    .await
    .unwrap();

    // 5. [P] Una venta de 2 "Paca x12" descuenta 24 unidades del stock
    let sale_paca = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(2),
            unit_price_override: None,
            discount_pct: None,
            tier_id: Some(paca.id),
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(20000),
            reference: None,
        }],
        quoted_total: dec(20000),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    sales::checkout(app, &c_cashier, sale_paca).await.unwrap();

    // Stock inicial: 50. 2 pacas x 12 = 24. Saldo = 26.
    assert_eq!(stock_row(admin, branch, soda).await.0, dec(26));

    // 6. [P] Un cajero sin pos:discount que usa la tarifa "Mayorista" con un cliente que no la tiene: 403
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

    let customer_regular = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("70000001".into()),
            dv: None,
            name: "Cliente Sin Tarifa".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let sale_forbidden_tier = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: None,
            tier_id: Some(mayorista.id),
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(1000),
            reference: None,
        }],
        quoted_total: dec(920),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: Some(customer_regular.id),
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    let try_forbidden = sales::checkout(app, &c_cashier, sale_forbidden_tier).await;
    assert!(is_forbidden(&try_forbidden));

    // Con cliente que SÍ tiene la tarifa asignada, se permite aunque el cajero no tenga pos:discount
    let customer_mayorista = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("70000002".into()),
            dv: None,
            name: "Cliente Mayorista".into(),
            phone: None,
            email: None,
            price_tier_id: Some(mayorista.id),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let sale_allowed_tier = SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: None,
            tier_id: Some(mayorista.id),
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(920),
            reference: None,
        }],
        quoted_total: dec(920),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: Some(customer_mayorista.id),
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    let allowed_res = sales::checkout(app, &c_cashier, sale_allowed_tier).await;
    assert!(allowed_res.is_ok());

    cleanup(admin, tenant, &[cashier, owner]).await;
}

async fn f1_promotions(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));
    let c_owner = claims(owner, Some(tenant), None);

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    let soda = seed_product(admin, tenant, "Gaseosa Promo", "UND", None, false).await;
    set_base_price(admin, soda, "10000").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "100", "0").await;

    let now = chrono::Utc::now();

    // 1. [P] PAUSADA, PROGRAMADA y VENCIDA no se aplican
    let p_pausada = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Pausada".into(),
            kind: "PORCENTAJE".into(),
            value: dec(50),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(2),
            ends_at: None,
            priority: Some(10),
            usage_limit: None,
            per_customer_limit: None,
            skip_tier_lines: Some(true),
            product_ids: Some(vec![soda]),
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_pausada.id)
        .await
        .unwrap();
    crate::promotions::pause(app, &c_owner, p_pausada.id)
        .await
        .unwrap();

    let p_programada = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Programada".into(),
            kind: "PORCENTAJE".into(),
            value: dec(50),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now + chrono::Duration::days(2),
            ends_at: None,
            priority: Some(10),
            usage_limit: None,
            per_customer_limit: None,
            skip_tier_lines: Some(true),
            product_ids: Some(vec![soda]),
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_programada.id)
        .await
        .unwrap();

    let p_vencida = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Vencida".into(),
            kind: "PORCENTAJE".into(),
            value: dec(50),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::days(5),
            ends_at: Some(now - chrono::Duration::days(1)),
            priority: Some(10),
            usage_limit: None,
            per_customer_limit: None,
            skip_tier_lines: Some(true),
            product_ids: Some(vec![soda]),
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_vencida.id)
        .await
        .unwrap();

    let q_res = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q_res.total, dec(10000));
    assert!(q_res.promotions.is_empty());

    // 2. [P] Con usage_limit = 1, la segunda venta no recibe el descuento
    let p_limit = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Promo Limit 1".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            priority: Some(1),
            usage_limit: Some(1),
            per_customer_limit: None,
            skip_tier_lines: Some(true),
            product_ids: Some(vec![soda]),
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_limit.id)
        .await
        .unwrap();

    let q1 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q1.total, dec(9000));
    assert_eq!(q1.promotions.len(), 1);

    let sale1 = sales::SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: None,
            tier_id: None,
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(9000),
            reference: None,
        }],
        quoted_total: dec(9000),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    sales::checkout(app, &c_cashier, sale1).await.unwrap();

    // Segunda cotización: ya no tiene la promo
    let q2 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q2.total, dec(10000));
    assert!(q2.promotions.is_empty());

    // 3. [P] per_customer_limit = 1 y venta sin cliente
    let p_cust = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Promo Per Customer".into(),
            kind: "PORCENTAJE".into(),
            value: dec(20),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            priority: Some(5),
            usage_limit: None,
            per_customer_limit: Some(1),
            skip_tier_lines: Some(true),
            product_ids: Some(vec![soda]),
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_cust.id)
        .await
        .unwrap();

    // [P] Venta sin cliente no recibe promo con per_customer_limit
    let q_no_cust = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q_no_cust.total, dec(10000));

    let cust1 = customers::create(
        app,
        &c_cashier,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("555111".into()),
            dv: None,
            name: "Cliente Uno".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let cust2 = customers::create(
        app,
        &c_cashier,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("555222".into()),
            dv: None,
            name: "Cliente Dos".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Cliente 1 cotiza: recibe 20% (8000)
    let q_c1 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust1.id),
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q_c1.total, dec(8000));

    // Cliente 1 compra
    let sale_c1 = sales::SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(soda),
            custom_name: None,
            qty: dec(1),
            unit_price_override: None,
            discount_pct: None,
            tier_id: None,
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(8000),
            reference: None,
        }],
        quoted_total: dec(8000),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: Some(cust1.id),
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    };
    sales::checkout(app, &c_cashier, sale_c1).await.unwrap();

    // Cliente 1 cotiza por segunda vez: NO recibe descuento (10000)
    let q_c1_again = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust1.id),
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q_c1_again.total, dec(10000));

    // [P] Cliente 2 cotiza: SÍ recibe descuento (8000)
    let q_c2 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust2.id),
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(q_c2.total, dec(8000));

    cleanup(admin, tenant, &[cashier, owner]).await;
}

async fn f1_coupons(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let cashier = seed_user(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));
    let c_owner = claims(owner, Some(tenant), None);

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    let soda = seed_product(admin, tenant, "Gaseosa Cupón", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "10000").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "100", "0").await;

    let snack = seed_product(admin, tenant, "Papas Cupón", "UND", Some("IVA19"), false).await;
    set_base_price(admin, snack, "5000").await;
    seed_branch_product(admin, tenant, branch, snack, None, None).await;
    seed_stock(admin, tenant, branch, snack, "100", "0").await;

    let now = chrono::Utc::now();

    // 1. [P] COUPON_NOT_FOUND
    let q_not_found = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("NOEXISTE123".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_not_found.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_NOT_FOUND")
    );

    // 2. [P] COUPON_INACTIVE
    let c_inact = coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "INACTIVO10".into(),
            name: "Inactivo".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();
    coupons::toggle(app, &c_owner, c_inact.id).await.unwrap();

    let q_inact = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("INACTIVO10".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_inact.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_INACTIVE")
    );

    // 3. [P] COUPON_NOT_STARTED
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "FUTURO10".into(),
            name: "Futuro".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now + chrono::Duration::days(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();
    let q_future = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("FUTURO10".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_future.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_NOT_STARTED")
    );

    // 4. [P] COUPON_EXPIRED
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "VENCIDO10".into(),
            name: "Vencido".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::days(2),
            ends_at: Some(now - chrono::Duration::hours(1)),
            usage_limit: None,
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();
    let q_exp = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("VENCIDO10".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_exp.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_EXPIRED")
    );

    // 5. [P] COUPON_EXHAUSTED
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "LIMITADO1".into(),
            name: "Un Solo Uso".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: Some(1),
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let q_lim1 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("LIMITADO1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(q_lim1.coupon_error.is_none());
    assert!(q_lim1.coupon.is_some());

    sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![SalePayment {
                method: PaymentMethod::Efectivo,
                amount: q_lim1.total,
                reference: None,
            }],
            quoted_total: q_lim1.total,
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("LIMITADO1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let q_lim2 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("LIMITADO1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_lim2.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_EXHAUSTED")
    );

    // 6. [P] COUPON_REQUIRES_CUSTOMER
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "CLIENTE10".into(),
            name: "Solo Clientes".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: Some(1),
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let q_req_cust = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("CLIENTE10".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_req_cust.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_REQUIRES_CUSTOMER")
    );

    // 7. [P] COUPON_CUSTOMER_LIMIT
    let cust = customers::create(
        app,
        &c_cashier,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("777888".into()),
            dv: None,
            name: "Cliente Fidelizado".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let q_cust1 = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.id),
            customer_alias: None,
            coupon_code: Some("CLIENTE10".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(q_cust1.coupon_error.is_none());

    sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![SalePayment {
                method: PaymentMethod::Efectivo,
                amount: q_cust1.total,
                reference: None,
            }],
            quoted_total: q_cust1.total,
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.id),
            customer_alias: None,
            coupon_code: Some("CLIENTE10".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let q_cust_limit = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.id),
            customer_alias: None,
            coupon_code: Some("CLIENTE10".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_cust_limit.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_CUSTOMER_LIMIT")
    );

    // 8. [P] COUPON_MIN_SUBTOTAL
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "MINIMO50K".into(),
            name: "Mínimo 50k".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: Some(dec(50000)),
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let q_min_sub = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("MINIMO50K".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_min_sub.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_MIN_SUBTOTAL")
    );

    // 9. [P] COUPON_NOT_APPLICABLE
    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "SOLOPAPAS".into(),
            name: "Solo Papas".into(),
            kind: "PORCENTAJE".into(),
            value: dec(15),
            scope: "PRODUCTO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: None,
            product_ids: Some(vec![snack]),
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let q_not_app = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("SOLOPAPAS".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q_not_app.coupon_error.as_ref().map(|e| e.code.as_str()),
        Some("COUPON_NOT_APPLICABLE")
    );

    // [P] Promoción de pedido del 10% + Cupón del 10%
    let p_order10 = crate::promotions::create(
        app,
        &c_owner,
        crate::promotions::CreatePromotionInput {
            branch_id: None,
            name: "Promo Pedido 10%".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            priority: Some(10),
            usage_limit: None,
            per_customer_limit: None,
            skip_tier_lines: Some(true),
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();
    crate::promotions::activate(app, &c_owner, p_order10.id)
        .await
        .unwrap();

    coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "COMBOPROMO".into(),
            name: "Cupón 10% Pedido".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: now - chrono::Duration::hours(1),
            ends_at: None,
            usage_limit: None,
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let q_combo = sales::quote(
        app,
        &c_cashier,
        sales::QuoteRequest {
            branch_id: branch,
            items: vec![SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: Some("COMBOPROMO".into()),
        },
    )
    .await
    .unwrap();

    assert!(q_combo.coupon_error.is_none());
    assert_eq!(q_combo.subtotal_gross, dec(10000));
    let line = &q_combo.lines[0];
    assert_eq!(line.gross, dec(10000));
    // La promoción es de alcance PEDIDO: va en discount.order, no en discount.promotion.
    assert_eq!(line.discount.promotion, dec(0));
    assert_eq!(line.discount.order, dec(1000));
    assert_eq!(line.discount.coupon, dec(900));
    assert_eq!(line.discount.total, dec(1900));
    assert_eq!(line.line_total, dec(8100));
    assert!(q_combo.discount_total <= q_combo.subtotal_gross);

    // [P] IVA se calcula sobre la base ya descontada: tax_amount == money(line_total * tasa)
    let expected_tax = crate::money::money(line.line_total * line.tax_rate);
    assert_eq!(line.tax_amount, expected_tax);
    assert_eq!(line.tax_amount, dec(1539));
    assert_eq!(q_combo.total, dec(8100) + dec(1539));

    cleanup(admin, tenant, &[cashier, owner]).await;
}

/// Código de un error `Coded`, o `None` si es otro tipo de error.
fn err_code<T: std::fmt::Debug>(r: &Result<T, ApiError>) -> Option<&'static str> {
    match r {
        Err(ApiError::Domain(DomainError::Coded { code, .. })) => Some(code),
        _ => None,
    }
}

fn line_item(
    product_id: Option<Uuid>,
    custom_name: Option<&str>,
    price: Option<i64>,
    reason: Option<&str>,
    custom_tax: Option<&str>,
    discount_pct: Option<i64>,
) -> sales::SaleItem {
    sales::SaleItem {
        product_id,
        custom_name: custom_name.map(str::to_string),
        qty: dec(1),
        unit_price_override: price.map(dec),
        discount_pct: discount_pct.map(dec),
        tier_id: None,
        price_override_reason: reason.map(str::to_string),
        custom_tax: custom_tax.map(str::to_string),
        option_ids: vec![],
    }
}

fn cash_sale(branch: Uuid, item: sales::SaleItem, total: i64) -> sales::SaleRequest {
    sales::SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![item],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(total),
            reference: None,
        }],
        quoted_total: dec(total),
        tip_amount: None,
        tip_waiter_id: None,
        customer_id: None,
        customer_alias: None,
        coupon_code: None,
        ..Default::default()
    }
}

async fn set_cashier_override(
    app: &PgPool,
    owner: &crate::auth::Claims,
    code: &str,
    allowed: bool,
) {
    permissions::update_role_permissions(
        app,
        owner,
        "CAJERO",
        permissions::UpdateRolePermissionsRequest {
            overrides: vec![permissions::RolePermissionOverrideInput {
                permission_code: code.into(),
                allowed,
            }],
        },
    )
    .await
    .unwrap();
}

/// F1.9: cambio de precio e ítem libre.
async fn f1_override(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // Arroz con IVA 5 %: precio 3.000; con cambio a 2.500 el total es 2.625.
    let prod = seed_product(admin, tenant, "Arroz 500g", "UND", Some("IVA5"), false).await;
    set_base_price(admin, prod, "3000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "50", "0").await;

    let reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, reg, dec(0))
        .await
        .unwrap();

    let override_sale = |reason: Option<&str>| {
        cash_sale(
            branch,
            line_item(Some(prod), None, Some(2500), reason, None, None),
            2625,
        )
    };

    // [P] Con el ajuste de la sucursal apagado (por defecto): 422 PRICE_OVERRIDE_DISABLED
    let r = sales::checkout(app, &c_cashier, override_sale(Some("cliente fiel"))).await;
    assert_eq!(err_code(&r), Some("PRICE_OVERRIDE_DISABLED"), "{r:?}");

    // Un precio igual al de catálogo (diferencia < 0,01) se ignora y no exige nada.
    let same = cash_sale(
        branch,
        line_item(Some(prod), None, Some(3000), None, None, None),
        3150,
    );
    assert!(sales::checkout(app, &c_cashier, same).await.is_ok());

    let mut s = settings::BranchSettings::default();
    s.pos.allow_price_override = true;
    settings::update_settings(
        app,
        &c_owner,
        settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: s,
        },
    )
    .await
    .unwrap();

    // [P] Con el ajuste encendido pero sin pos:price_override: 403
    let r = sales::checkout(app, &c_cashier, override_sale(Some("cliente fiel"))).await;
    assert!(is_forbidden(&r), "{r:?}");

    set_cashier_override(app, &c_owner, "pos:price_override", true).await;

    // [P] Producto que no permite cambio de precio: 422 PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT
    sqlx::query("update products set allow_price_override = false where id = $1")
        .bind(prod)
        .execute(admin)
        .await
        .unwrap();
    let r = sales::checkout(app, &c_cashier, override_sale(Some("cliente fiel"))).await;
    assert_eq!(
        err_code(&r),
        Some("PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT"),
        "{r:?}"
    );
    sqlx::query("update products set allow_price_override = true where id = $1")
        .bind(prod)
        .execute(admin)
        .await
        .unwrap();

    // [P] Sin motivo, o con menos de 3 caracteres: 422 PRICE_OVERRIDE_REASON_REQUIRED
    let r = sales::checkout(app, &c_cashier, override_sale(None)).await;
    assert_eq!(
        err_code(&r),
        Some("PRICE_OVERRIDE_REASON_REQUIRED"),
        "{r:?}"
    );
    let r = sales::checkout(app, &c_cashier, override_sale(Some("ok"))).await;
    assert_eq!(
        err_code(&r),
        Some("PRICE_OVERRIDE_REASON_REQUIRED"),
        "{r:?}"
    );

    // Cambio válido: se cobra 2.500 más IVA 5 %.
    let resp = created(
        sales::checkout(app, &c_cashier, override_sale(Some("cliente frecuente")))
            .await
            .unwrap(),
    );
    assert_eq!(resp.order.total_amount.normalize(), dec(2625));
    let line_id = resp.items[0].id;

    // [P] El cambio de precio queda en audit_log (precio resuelto antes, nuevo después)
    let (before, after): (serde_json::Value, serde_json::Value) = sqlx::query_as(
        "select before, after from audit_log
          where action = 'CAMBIO_PRECIO' and entity = 'order_items' and entity_id = $1",
    )
    .bind(line_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let before_price: Decimal = serde_json::from_value(before["price"].clone()).unwrap();
    let after_price: Decimal = serde_json::from_value(after["price"].clone()).unwrap();
    assert_eq!(before_price.normalize(), dec(3000));
    assert_eq!(after_price.normalize(), dec(2500));
    assert_eq!(after["reason"], "cliente frecuente");

    // --- Ítem libre ---
    let free = |tax: Option<&str>, discount: Option<i64>, total: i64| {
        cash_sale(
            branch,
            line_item(
                None,
                Some("Servicio especial"),
                Some(5000),
                None,
                tax,
                discount,
            ),
            total,
        )
    };

    // [P] Sin pos:custom_item: 403 (el cajero lo trae por defecto; se le quita)
    set_cashier_override(app, &c_owner, "pos:custom_item", false).await;
    let r = sales::checkout(app, &c_cashier, free(Some("IVA19"), None, 5950)).await;
    assert!(is_forbidden(&r), "{r:?}");
    set_cashier_override(app, &c_owner, "pos:custom_item", true).await;

    // [P] Impuesto inválido o ausente: 422 CUSTOM_ITEM_INVALID
    let r = sales::checkout(app, &c_cashier, free(Some("IVA99"), None, 5000)).await;
    assert_eq!(err_code(&r), Some("CUSTOM_ITEM_INVALID"), "{r:?}");
    let r = sales::checkout(app, &c_cashier, free(None, None, 5000)).await;
    assert_eq!(err_code(&r), Some("CUSTOM_ITEM_INVALID"), "{r:?}");

    // [P] Ítem libre con descuento: 422 CUSTOM_ITEM_INVALID
    let r = sales::checkout(app, &c_cashier, free(Some("EXENTO"), Some(10), 4500)).await;
    assert_eq!(err_code(&r), Some("CUSTOM_ITEM_INVALID"), "{r:?}");

    // [P] Producto e ítem libre en la misma línea: 422 CUSTOM_ITEM_INVALID
    let both = cash_sale(
        branch,
        line_item(
            Some(prod),
            Some("Doble"),
            Some(5000),
            None,
            Some("EXENTO"),
            None,
        ),
        5000,
    );
    let r = sales::checkout(app, &c_cashier, both).await;
    assert_eq!(err_code(&r), Some("CUSTOM_ITEM_INVALID"), "{r:?}");

    // [P] El ítem libre no mueve stock y calcula bien su impuesto (5.000 + IVA 19 % = 5.950)
    let (on_hand_before, _) = stock_row(admin, branch, prod).await;
    let resp = created(
        sales::checkout(app, &c_cashier, free(Some("IVA19"), None, 5950))
            .await
            .unwrap(),
    );
    assert_eq!(resp.items.len(), 1);
    assert!(resp.items[0].product_id.is_none());
    assert_eq!(resp.items[0].tax_amount.normalize(), dec(950));
    assert_eq!(resp.order.total_amount.normalize(), dec(5950));
    let (on_hand_after, _) = stock_row(admin, branch, prod).await;
    assert_eq!(on_hand_before, on_hand_after);

    cleanup(admin, tenant, &[owner, cashier]).await;
}

/// F1.10: propinas.
async fn f1_tips(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    let waiter = seed_user(admin).await;
    let stranger = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    seed_membership(admin, tenant, Some(branch), waiter, "MESERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // Gaseosa sin impuesto especial: IVA 19 %, 2 x 2.500 = 5.000 + 950 de IVA.
    let soda = seed_product(admin, tenant, "Gaseosa", "UND", Some("IVA19"), false).await;
    set_base_price(admin, soda, "2500").await;
    seed_branch_product(admin, tenant, branch, soda, None, None).await;
    seed_stock(admin, tenant, branch, soda, "20", "0").await;
    let reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, reg, dec(0))
        .await
        .unwrap();

    let tip_sale = |tip: Option<i64>, waiter_id: Option<Uuid>, total: i64| {
        let mut req = cash_sale(
            branch,
            sales::SaleItem {
                product_id: Some(soda),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            },
            total,
        );
        req.tip_amount = tip.map(dec);
        req.tip_waiter_id = waiter_id;
        req
    };

    // [P] Con las propinas apagadas (por defecto): 422 TIPS_DISABLED
    let r = sales::checkout(app, &c_cashier, tip_sale(Some(1000), None, 6950)).await;
    assert_eq!(err_code(&r), Some("TIPS_DISABLED"), "{r:?}");

    let mut s = settings::BranchSettings::default();
    s.tips.enabled = true;
    settings::update_settings(
        app,
        &c_owner,
        settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: s,
        },
    )
    .await
    .unwrap();

    // Mesero sin propina, o mesero sin membresía en la sucursal: 422 TIP_INVALID
    let r = sales::checkout(app, &c_cashier, tip_sale(None, Some(waiter), 5950)).await;
    assert_eq!(err_code(&r), Some("TIP_INVALID"), "{r:?}");
    let r = sales::checkout(app, &c_cashier, tip_sale(Some(1000), Some(stranger), 6950)).await;
    assert_eq!(err_code(&r), Some("TIP_INVALID"), "{r:?}");

    // [P] La propina entra en total_amount, no en subtotal ni en inc_amount
    let resp = created(
        sales::checkout(app, &c_cashier, tip_sale(Some(1000), Some(waiter), 6950))
            .await
            .unwrap(),
    );
    assert_eq!(resp.order.subtotal.normalize(), dec(5000));
    assert_eq!(resp.order.inc_amount.normalize(), dec(950));
    assert_eq!(resp.order.tip_amount.normalize(), dec(1000));
    assert_eq!(resp.order.total_amount.normalize(), dec(6950));
    let saved_waiter: Option<Uuid> =
        sqlx::query_scalar("select waiter_id from orders where id = $1")
            .bind(resp.order.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(saved_waiter, Some(waiter));

    // El selector de meseros solo lista meseros activos de la sucursal.
    let waiters = users::list_waiters(app, &c_cashier, branch).await.unwrap();
    assert_eq!(waiters.len(), 1);
    assert_eq!(waiters[0].user_id, waiter);

    // [P] La propina aparece en tips_total del turno
    let summary = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .expect("turno abierto");
    assert_eq!(summary.tips_total.normalize(), dec(1000));
    assert_eq!(summary.tips_pending.normalize(), dec(1000));

    cleanup(admin, tenant, &[owner, cashier, waiter, stranger]).await;
}

fn d(s: &str) -> Decimal {
    s.parse().expect("decimal")
}

fn refund_req(
    id: Uuid,
    order_id: Uuid,
    order_item_id: Uuid,
    qty: i64,
    disposition: refunds::Disposition,
    method: refunds::RefundMethod,
) -> refunds::RefundRequest {
    refunds::RefundRequest {
        id,
        order_id,
        method,
        reason: "Producto defectuoso".into(),
        reversal_reference: None,
        items: vec![refunds::RefundItemInput {
            order_item_id,
            qty: dec(qty),
            disposition,
        }],
    }
}

fn new_refund(r: refunds::Created) -> refunds::Refund {
    match r {
        refunds::Created::New(x) => x,
        refunds::Created::Replayed(_) => panic!("expected a new refund"),
    }
}

fn cash_payment(amount: &str) -> Vec<SalePayment> {
    vec![SalePayment {
        method: PaymentMethod::Efectivo,
        amount: d(amount),
        reference: None,
    }]
}

/// F1.11: devoluciones.
async fn f1_refunds(app: &PgPool, admin: &PgPool) {
    use refunds::{Disposition::*, RefundMethod::*};

    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // Precio con el que el IVA de 3 unidades no se divide exacto entre 3:
    // 3 x 1.001,01 = 3.003,03; IVA 19 % = 570,58 (190,19 por unidad, sobra 0,01).
    let prod = seed_product(admin, tenant, "Licuadora", "UND", Some("IVA19"), false).await;
    set_base_price(admin, prod, "1001.01").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "10", "0").await;

    let reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, reg, dec(0))
        .await
        .unwrap();

    let mut s1 = sale(Uuid::new_v4(), branch, &[(prod, 3)], &[], d("3573.61"));
    s1.payments = cash_payment("3573.61");
    let s1 = created(sales::checkout(app, &c_cashier, s1).await.unwrap());
    let order_id = s1.order.id;
    let item = &s1.items[0];
    let item_id = item.id;
    let (stock_after_sale, _) = stock_row(admin, branch, prod).await;

    // [P] Con aprobación obligatoria y un cajero sin pos:refund queda PENDIENTE, sin mover stock ni caja
    let r1 = new_refund(
        refunds::create(
            app,
            &c_cashier,
            refund_req(Uuid::new_v4(), order_id, item_id, 1, Reintegrar, Efectivo),
        )
        .await
        .unwrap(),
    );
    assert_eq!(r1.status, "PENDIENTE");
    assert_eq!(r1.number, "DEV-000001");
    assert_eq!(stock_row(admin, branch, prod).await.0, stock_after_sale);
    let before = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.expected_cash.normalize(), d("3573.61"));

    // [P] No se puede devolver más de lo vendido menos lo ya devuelto (contando las pendientes)
    let r = refunds::create(
        app,
        &c_cashier,
        refund_req(Uuid::new_v4(), order_id, item_id, 3, Reintegrar, Efectivo),
    )
    .await;
    assert_eq!(err_code(&r), Some("REFUND_QTY_EXCEEDED"), "{:?}", r.err());

    // S4.2: como el cajero cerró su turno tras pedir la devolución y el dueño tampoco tiene turno:
    // [P] Sin turno abierto de quien pide ni de quien aprueba: 409 REFUND_NEEDS_OPEN_SESSION
    cash::close(
        app,
        &c_cashier,
        before.session.id,
        d("3573.61"),
        None,
        &before.state_token,
    )
    .await
    .unwrap();
    let r = refunds::approve(app, &c_owner, r1.id).await;
    assert_eq!(
        err_code(&r),
        Some("REFUND_NEEDS_OPEN_SESSION"),
        "{:?}",
        r.err()
    );

    // El dueño abre turno en una segunda caja y aprueba.
    let reg2: Uuid = sqlx::query_scalar(
        "insert into cash_registers (tenant_id, branch_id, name, code)
         values ($1, $2, 'Caja 2', 'CAJA-2') returning id",
    )
    .bind(tenant)
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();
    cash::open(app, &c_owner, branch, reg2, dec(10000))
        .await
        .unwrap();
    let r1 = refunds::approve(app, &c_owner, r1.id).await.unwrap();
    assert_eq!(r1.status, "APROBADA");
    assert_eq!(r1.total.normalize(), d("1191.20"));

    // [P] REINTEGRAR sube el stock
    assert_eq!(
        stock_row(admin, branch, prod).await.0,
        stock_after_sale + dec(1)
    );

    // [P] La devolución en efectivo crea un movimiento DEVOLUCION y baja el esperado del turno
    let owner_summary = cash::current(app, &c_owner, branch).await.unwrap().unwrap();
    assert_eq!(owner_summary.expected_cash.normalize(), d("8808.80"));
    let movements: i64 = sqlx::query_scalar(
        "select count(*) from cash_movements m join tbl_tipos_maestra t on t.tima_id = m.kind_id
          where t.tima_codigo = 'DEVOLUCION' and m.ref_id = $1",
    )
    .bind(r1.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(movements, 1);

    // [P] Se crea un fiscal_documents NOTA_CREDITO con refund_id
    let notes: i64 = sqlx::query_scalar(
        "select count(*) from fiscal_documents where doc_type = 'NOTA_CREDITO' and refund_id = $1",
    )
    .bind(r1.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(notes, 1);

    // [P] MERMA no cambia el stock; las devoluciones parciales suman exacto lo vendido
    let r2_req = refund_req(Uuid::new_v4(), order_id, item_id, 2, Merma, Efectivo);
    let r2 = new_refund(
        refunds::create(app, &c_cashier, r2_req.clone())
            .await
            .unwrap(),
    );
    let r2 = refunds::approve(app, &c_owner, r2.id).await.unwrap();
    assert_eq!(
        stock_row(admin, branch, prod).await.0,
        stock_after_sale + dec(1)
    );
    assert_eq!(
        (r1.subtotal + r2.subtotal).normalize(),
        item.line_total.normalize()
    );
    assert_eq!(
        (r1.tax_amount + r2.tax_amount).normalize(),
        item.tax_amount.normalize()
    );
    let refunded: Decimal = sqlx::query_scalar("select refunded_amount from orders where id = $1")
        .bind(order_id)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(refunded.normalize(), d("3573.61"));

    // [P] Reintento idéntico devuelve la misma devolución; con otro cuerpo, IDEMPOTENCY_CONFLICT
    match refunds::create(app, &c_cashier, r2_req.clone())
        .await
        .unwrap()
    {
        refunds::Created::Replayed(x) => assert_eq!(x.id, r2.id),
        refunds::Created::New(_) => panic!("expected a replay"),
    }
    let mut changed = r2_req.clone();
    changed.reason = "Otro motivo distinto".into();
    let r = refunds::create(app, &c_cashier, changed).await;
    assert_eq!(err_code(&r), Some("IDEMPOTENCY_CONFLICT"), "{:?}", r.err());

    // Rechazo: una pendiente rechazada ya no se puede aprobar (el cajero reabre su turno para vender).
    cash::open(app, &c_cashier, branch, reg, dec(0))
        .await
        .unwrap();
    let mut s3 = sale(Uuid::new_v4(), branch, &[(prod, 1)], &[], d("1191.20"));
    s3.payments = cash_payment("1191.20");
    let s3 = created(sales::checkout(app, &c_cashier, s3).await.unwrap());
    let r3 = new_refund(
        refunds::create(
            app,
            &c_cashier,
            refund_req(
                Uuid::new_v4(),
                s3.order.id,
                s3.items[0].id,
                1,
                Reintegrar,
                Efectivo,
            ),
        )
        .await
        .unwrap(),
    );
    let r3 = refunds::reject(
        app,
        &c_owner,
        r3.id,
        refunds::RejectRequest {
            reason: "No aplica".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(r3.status, "RECHAZADA");
    let r = refunds::approve(app, &c_owner, r3.id).await;
    assert_eq!(err_code(&r), Some("REFUND_NOT_PENDING"), "{:?}", r.err());

    // [P] ORIGINAL en una venta con pagos mixtos: 422 REFUND_ORIGINAL_MIXED
    let mut s4 = sale(Uuid::new_v4(), branch, &[(prod, 1)], &[], d("1191.20"));
    s4.payments = vec![
        SalePayment {
            method: PaymentMethod::Datafono,
            amount: dec(1000),
            reference: Some("123456".into()),
        },
        SalePayment {
            method: PaymentMethod::Efectivo,
            amount: d("191.20"),
            reference: None,
        },
    ];
    let s4 = created(sales::checkout(app, &c_cashier, s4).await.unwrap());
    let r = refunds::create(
        app,
        &c_cashier,
        refund_req(
            Uuid::new_v4(),
            s4.order.id,
            s4.items[0].id,
            1,
            Reintegrar,
            Original,
        ),
    )
    .await;
    assert_eq!(err_code(&r), Some("REFUND_ORIGINAL_MIXED"), "{:?}", r.err());

    cleanup(admin, tenant, &[owner, cashier]).await;
}

/// F1.12: anulación de ventas.
async fn f1_void(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let prod = seed_product(admin, tenant, "Cuaderno", "UND", None, false).await;
    set_base_price(admin, prod, "1000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "20", "0").await;

    // Cupón de un solo uso: 10 % al pedido.
    let coupon = coupons::create(
        app,
        &c_owner,
        coupons::CreateCouponInput {
            branch_id: None,
            code: "ANULA10".into(),
            name: "Anula".into(),
            kind: "PORCENTAJE".into(),
            value: dec(10),
            scope: "PEDIDO".into(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            starts_at: Utc::now() - Duration::days(1),
            ends_at: None,
            usage_limit: Some(1),
            per_customer_limit: None,
            product_ids: None,
            category_ids: None,
        },
    )
    .await
    .unwrap();

    let reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, reg, dec(0))
        .await
        .unwrap();
    let (stock_before, _) = stock_row(admin, branch, prod).await;

    let cash_sale_of = |qty: i64, total: &str, coupon_code: Option<&str>| {
        let mut req = sale(Uuid::new_v4(), branch, &[(prod, qty)], &[], d(total));
        req.payments = cash_payment(total);
        req.coupon_code = coupon_code.map(str::to_string);
        req
    };

    // Venta con cupón: 2.000 − 10 % = 1.800.
    let s1 = created(
        sales::checkout(app, &c_cashier, cash_sale_of(2, "1800", Some("ANULA10")))
            .await
            .unwrap(),
    );
    let usage = |id: Uuid| async move {
        sqlx::query_scalar::<_, i32>("select usage_count from coupons where id = $1")
            .bind(id)
            .fetch_one(admin)
            .await
            .unwrap()
    };
    assert_eq!(usage(coupon.id).await, 1);

    let reason = || sales::VoidRequest {
        reason: "Cliente se arrepintió".into(),
    };

    // Un cajero sin pos:refund no anula.
    let r = sales::void(app, &c_cashier, s1.order.id, reason()).await;
    assert!(is_forbidden(&r), "{r:?}");

    // Motivo de menos de 3 caracteres: 422 VOID_REASON_REQUIRED
    let r = sales::void(
        app,
        &c_owner,
        s1.order.id,
        sales::VoidRequest {
            reason: "no".into(),
        },
    )
    .await;
    assert_eq!(err_code(&r), Some("VOID_REASON_REQUIRED"), "{r:?}");

    // [P] Anular devuelve el stock, saca el efectivo del esperado y libera el cupón
    let voided = sales::void(app, &c_owner, s1.order.id, reason())
        .await
        .unwrap();
    assert_eq!(voided.status, "CANCELADA");
    assert_eq!(stock_row(admin, branch, prod).await.0, stock_before);
    let summary = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.expected_cash.normalize(), dec(0));
    assert_eq!(usage(coupon.id).await, 0);
    let fiscal: String = sqlx::query_scalar(
        "select t.tima_codigo from fiscal_documents f join tbl_tipos_maestra t on t.tima_id = f.status_id
          where f.order_id = $1 and f.doc_type = 'POS'",
    )
    .bind(s1.order.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(fiscal, "ANULADO");
    // El cupón liberado se puede volver a usar.
    created(
        sales::checkout(app, &c_cashier, cash_sale_of(2, "1800", Some("ANULA10")))
            .await
            .unwrap(),
    );

    // Una venta ya anulada no se anula otra vez.
    let r = sales::void(app, &c_owner, s1.order.id, reason()).await;
    assert_eq!(err_code(&r), Some("VOID_NOT_ALLOWED"), "{r:?}");

    // [P] Una venta con devolución no se anula: HAS_REFUNDS
    let s2 = created(
        sales::checkout(app, &c_cashier, cash_sale_of(1, "1000", None))
            .await
            .unwrap(),
    );
    refunds::create(
        app,
        &c_cashier,
        refund_req(
            Uuid::new_v4(),
            s2.order.id,
            s2.items[0].id,
            1,
            refunds::Disposition::Reintegrar,
            refunds::RefundMethod::Efectivo,
        ),
    )
    .await
    .unwrap();
    let r = sales::void(app, &c_owner, s2.order.id, reason()).await;
    assert_void_why(&r, "HAS_REFUNDS");

    // [P] Una venta con el documento fiscal ACEPTADO no se anula: FISCAL_ACCEPTED
    let s3 = created(
        sales::checkout(app, &c_cashier, cash_sale_of(1, "1000", None))
            .await
            .unwrap(),
    );
    sqlx::query(
        "update fiscal_documents set status_id = public.tipo_id('DOCF', 'ACEPTADO')
          where order_id = $1 and doc_type = 'POS'",
    )
    .bind(s3.order.id)
    .execute(admin)
    .await
    .unwrap();
    let r = sales::void(app, &c_owner, s3.order.id, reason()).await;
    assert_void_why(&r, "FISCAL_ACCEPTED");

    // Una venta de otro día no se anula: NOT_TODAY
    let s4 = created(
        sales::checkout(app, &c_cashier, cash_sale_of(1, "1000", None))
            .await
            .unwrap(),
    );
    sqlx::query("update orders set created_at = now() - interval '2 days' where id = $1")
        .bind(s4.order.id)
        .execute(admin)
        .await
        .unwrap();
    let r = sales::void(app, &c_owner, s4.order.id, reason()).await;
    assert_void_why(&r, "NOT_TODAY");

    // [P] Una venta de un turno cerrado no se anula: SESSION_CLOSED
    let s5 = created(
        sales::checkout(app, &c_cashier, cash_sale_of(1, "1000", None))
            .await
            .unwrap(),
    );
    sqlx::query(
        "update cash_sessions set closed_at = now(), counted_cash = 0
          where id = (select cash_session_id from orders where id = $1)",
    )
    .bind(s5.order.id)
    .execute(admin)
    .await
    .unwrap();
    let r = sales::void(app, &c_owner, s5.order.id, reason()).await;
    assert_void_why(&r, "SESSION_CLOSED");

    cleanup(admin, tenant, &[owner, cashier]).await;
}

fn assert_void_why<T: std::fmt::Debug>(r: &Result<T, ApiError>, why: &str) {
    match r {
        Err(ApiError::Domain(DomainError::Coded { code, data, .. })) => {
            assert_eq!(*code, "VOID_NOT_ALLOWED");
            assert_eq!(data.as_ref().and_then(|d| d["why"].as_str()), Some(why));
        }
        other => panic!("expected VOID_NOT_ALLOWED/{why}, got {other:?}"),
    }
}

fn history_query(branch: Uuid) -> sales_history::HistoryQuery {
    sales_history::HistoryQuery {
        branch_id: branch,
        from: None,
        to: None,
        cashier_id: None,
        method: None,
        status: None,
        q: None,
        page: None,
        page_size: None,
    }
}

/// F1.13: historial de ventas.
async fn f1_sales_history(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier_a = seed_user(admin).await;
    let cashier_b = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier_a, "CAJERO").await;
    seed_membership(admin, tenant, None, cashier_b, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_a = claims(cashier_a, Some(tenant), Some(branch));
    let c_b = claims(cashier_b, Some(tenant), Some(branch));

    let prod = seed_product(admin, tenant, "Lapicero", "UND", None, false).await;
    set_base_price(admin, prod, "1500").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "20", "0").await;

    let reg_a = default_register(admin, branch).await;
    let reg_b: Uuid = sqlx::query_scalar(
        "insert into cash_registers (tenant_id, branch_id, name, code)
         values ($1, $2, 'Caja 2', 'CAJA-2') returning id",
    )
    .bind(tenant)
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();
    cash::open(app, &c_a, branch, reg_a, dec(0)).await.unwrap();
    cash::open(app, &c_b, branch, reg_b, dec(0)).await.unwrap();

    let one = || {
        let mut req = sale(Uuid::new_v4(), branch, &[(prod, 1)], &[], dec(1500));
        req.payments = cash_payment("1500");
        req
    };
    let sale_a = created(sales::checkout(app, &c_a, one()).await.unwrap());
    created(sales::checkout(app, &c_b, one()).await.unwrap());
    created(sales::checkout(app, &c_b, one()).await.unwrap());

    // El dueño ve las tres ventas del día.
    let all = sales_history::list(app, &c_owner, history_query(branch))
        .await
        .unwrap();
    assert_eq!(all.total_count, 3);
    assert_eq!(all.totals.amount.normalize(), dec(4500));

    // [P] Un cajero sin reports:read solo ve sus ventas, aunque mande cashier_id de otro
    let mut q = history_query(branch);
    q.cashier_id = Some(cashier_b);
    let mine = sales_history::list(app, &c_a, q).await.unwrap();
    assert_eq!(mine.total_count, 1);
    assert_eq!(mine.items[0].id, sale_a.order.id);
    assert_eq!(mine.items[0].customer_label, "Consumidor final");
    assert_eq!(mine.items[0].methods, vec!["EFECTIVO".to_string()]);

    // Ni tampoco el detalle de una venta ajena.
    let other = all
        .items
        .iter()
        .find(|s| s.id != sale_a.order.id)
        .unwrap()
        .id;
    let r = sales_history::detail(app, &c_a, other).await;
    assert!(is_forbidden(&r), "{:?}", r.err());

    // Búsqueda por número, en las dos formas.
    let mut q = history_query(branch);
    q.q = Some(sale_a.receipt_number.clone());
    assert_eq!(
        sales_history::list(app, &c_owner, q)
            .await
            .unwrap()
            .total_count,
        1
    );
    let detail = sales_history::detail(app, &c_a, sale_a.order.id)
        .await
        .unwrap();
    assert_eq!(detail.sale.order.id, sale_a.order.id);
    assert!(detail.refunds.is_empty());
    assert_eq!(detail.register_name.as_deref(), Some("Caja 1"));
    assert_eq!(mine.items[0].cashier_id, Some(cashier_a));

    // Datos del negocio para la tirilla: cualquier miembro los lee.
    let biz = settings::business_info(app, &c_a).await.unwrap();
    assert_eq!(biz.name, "itest");

    cleanup(admin, tenant, &[owner, cashier_a, cashier_b]).await;
}

/// F2.3 CRUD de productos y variantes
async fn f2_products(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // 1. [P] Un SKU repetido entre un producto y una variante de otro: 409 SKU_TAKEN.
    let prod_a = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Producto A".into(),
            sku: Some("SKU-DUP-1".into()),
            barcode: Some("770123456789".into()),
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "IVA19".into(),
            cost_price: Some(dec(5000)),
            base_price: Some(dec(10000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: true,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    let prod_b = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Camiseta Padre".into(),
            sku: Some("SKU-PADRE".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "IVA19".into(),
            cost_price: Some(dec(8000)),
            base_price: Some(dec(20000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: true,
            has_variants: true,
            description: None,
        },
    )
    .await
    .unwrap();

    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("Talla".into(), "M".into());
    let try_dup_variant_sku = products::create_variant(
        app,
        &c_owner,
        prod_b.row.id,
        products::CreateVariantInput {
            sku: Some("SKU-DUP-1".into()),
            barcode: None,
            attrs: attrs.clone(),
            price: dec(20000),
            cost_price: Some(dec(8000)),
        },
    )
    .await;
    assert!(
        matches!(&try_dup_variant_sku, Err(ApiError::Domain(DomainError::Coded { code, .. })) if *code == "SKU_TAKEN"),
        "expected SKU_TAKEN, got {:?}",
        try_dup_variant_sku
    );

    // 2. [P] Un insumo no aparece en /v1/catalog y venderlo da 422 PRODUCT_NOT_SELLABLE.
    let insumo = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Harina Insumo".into(),
            sku: Some("INS-HARINA".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "INSUMO".into(),
            unit_measure_code: "KG".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(3000)),
            base_price: None,
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    let cat_items = catalog::catalog(app, &c_cashier, branch).await.unwrap();
    assert!(
        !cat_items.iter().any(|i| i.product_id == insumo.row.id),
        "insumo must not be in catalog"
    );

    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(0))
        .await
        .unwrap();

    let try_sell_insumo = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(insumo.row.id),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(3000),
                reference: None,
            }],
            quoted_total: dec(3000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await;
    assert!(
        matches!(&try_sell_insumo, Err(ApiError::Domain(DomainError::Coded { code, .. })) if *code == "PRODUCT_NOT_SELLABLE"),
        "expected PRODUCT_NOT_SELLABLE, got {:?}",
        try_sell_insumo
    );

    // 3. [P] Vender un padre con variantes: 422 PRODUCT_IS_VARIANT_PARENT.
    let try_sell_parent = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod_b.row.id),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(20000),
                reference: None,
            }],
            quoted_total: dec(20000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await;
    assert!(
        matches!(&try_sell_parent, Err(ApiError::Domain(DomainError::Coded { code, .. })) if *code == "PRODUCT_IS_VARIANT_PARENT"),
        "expected PRODUCT_IS_VARIANT_PARENT, got {:?}",
        try_sell_parent
    );

    // 4. [P] Variantes más presentaciones: 422 VARIANTS_AND_PRESENTATIONS.
    let pres_tier = crate::tiers::create_tier(
        app,
        &c_owner,
        crate::tiers::CreatePriceTierRequest {
            name: "Paca x6".into(),
            kind: "PRESENTACION".into(),
            adjustment: "PRECIO_FIJO".into(),
            value: dec(50000),
            units_per_package: Some(dec(6)),
        },
    )
    .await
    .unwrap();

    let try_tier_on_parent = crate::tiers::set_product_tier_prices(
        app,
        &c_owner,
        prod_b.row.id,
        vec![crate::tiers::SetProductTierPriceInput {
            tier_id: pres_tier.id,
            price: Some(dec(50000)),
            units_per_package: Some(dec(6)),
            barcode: None,
        }],
    )
    .await;
    assert!(
        matches!(&try_tier_on_parent, Err(ApiError::Domain(DomainError::Coded { code, .. })) if *code == "VARIANTS_AND_PRESENTATIONS"),
        "expected VARIANTS_AND_PRESENTATIONS, got {:?}",
        try_tier_on_parent
    );

    // 5. [P] Cambiar la unidad de un producto con movimientos: 409 PRODUCT_UOM_LOCKED.
    inventory::replenish(
        app,
        &c_owner,
        branch,
        prod_a.row.id,
        dec(10),
        "entrada inicial",
    )
    .await
    .unwrap();

    let update_input = products::ProductInput {
        name: prod_a.row.name.clone(),
        sku: prod_a.row.sku.clone(),
        barcode: prod_a.row.barcode.clone(),
        kind: prod_a.row.kind.clone(),
        usage: prod_a.row.usage.clone(),
        unit_measure_code: "KG".into(), // Cambiando de UND a KG
        purchase_uom_code: None,
        purchase_factor: dec(1),
        tax_code: "IVA19".into(),
        cost_price: prod_a.row.cost_price,
        base_price: prod_a.row.price,
        reorder_qty: None,
        max_qty: None,
        track_lots: false,
        allow_price_override: true,
        has_variants: false,
        description: None,
    };

    let try_change_uom =
        products::update_product(app, &c_owner, prod_a.row.id, update_input.clone()).await;
    assert!(
        matches!(&try_change_uom, Err(ApiError::Domain(DomainError::Coded { code, .. })) if *code == "PRODUCT_UOM_LOCKED"),
        "expected PRODUCT_UOM_LOCKED, got {:?}",
        try_change_uom
    );

    // 6. [P] Un servicio vendido no mueve el stock.
    let servicio = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Servicio Corte".into(),
            sku: Some("SRV-CORTE".into()),
            barcode: None,
            kind: "SERVICIO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: None,
            base_price: Some(dec(15000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    let _srv_sale = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(servicio.row.id),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(15000),
                reference: None,
            }],
            quoted_total: dec(15000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let mov_count: i64 = sqlx::query_scalar(
        "select count(*) from inventory_movements where tenant_id = $1 and product_id = $2",
    )
    .bind(tenant)
    .bind(servicio.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        mov_count, 0,
        "Servicio no debe generar movimientos de stock"
    );

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn f2_options(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // 1. Insumo Queso Mozzarella
    let queso = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Queso Mozzarella".into(),
            sku: Some("INS-QUESO".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "INSUMO".into(),
            unit_measure_code: "G".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(50)),
            base_price: None,
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // 1000g de queso inicial
    inventory::replenish(
        app,
        &c_owner,
        branch,
        queso.row.id,
        dec(1000),
        "stock inicial queso",
    )
    .await
    .unwrap();

    // 2. Hamburguesa Clásica
    let burger = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Hamburguesa Clásica".into(),
            sku: Some("HAMB-01".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(8000)),
            base_price: Some(dec(15000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // 10 hamburguesas iniciales
    inventory::replenish(
        app,
        &c_owner,
        branch,
        burger.row.id,
        dec(10),
        "stock hamburguesa",
    )
    .await
    .unwrap();

    // 3. Crear grupo opcional "Toppings" con "Extra queso"
    let group_toppings = options::create_option_group(
        app,
        &c_owner,
        options::CreateOptionGroupInput {
            name: "Toppings".into(),
            min_select: 0,
            max_select: 3,
            active: true,
            options: vec![options::OptionInput {
                id: None,
                name: "Extra queso".into(),
                price_delta: dec(2000),
                input_product_id: Some(queso.row.id),
                input_qty: Some(dec(30)),
                sort_order: 1,
                active: true,
            }],
        },
    )
    .await
    .unwrap();
    let opt_extra_queso = group_toppings.options[0].id;

    // 4. Crear grupo obligatorio "Término de la carne" (min 1, max 1)
    let group_termino = options::create_option_group(
        app,
        &c_owner,
        options::CreateOptionGroupInput {
            name: "Término de la carne".into(),
            min_select: 1,
            max_select: 1,
            active: true,
            options: vec![
                options::OptionInput {
                    id: None,
                    name: "Bien cocido".into(),
                    price_delta: dec(0),
                    input_product_id: None,
                    input_qty: None,
                    sort_order: 1,
                    active: true,
                },
                options::OptionInput {
                    id: None,
                    name: "Tres cuartos".into(),
                    price_delta: dec(0),
                    input_product_id: None,
                    input_qty: None,
                    sort_order: 2,
                    active: true,
                },
            ],
        },
    )
    .await
    .unwrap();
    let opt_bien_cocido = group_termino.options[0].id;

    // Asignar ambos grupos a la hamburguesa
    options::set_product_option_groups(
        app,
        &c_owner,
        burger.row.id,
        vec![
            options::UpdateProductOptionGroupInput {
                group_id: group_termino.id,
                sort_order: 1,
            },
            options::UpdateProductOptionGroupInput {
                group_id: group_toppings.id,
                sort_order: 2,
            },
        ],
    )
    .await
    .unwrap();

    // Abrir caja
    let def_reg = default_register(admin, branch).await;
    cash::open(app, &c_cashier, branch, def_reg, dec(0))
        .await
        .unwrap();

    // [P] No elegir el mínimo de un grupo obligatorio: 422 OPTION_SELECTION_INVALID con data {group_id}
    let try_sale_no_mandatory = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(burger.row.id),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(15000),
                reference: None,
            }],
            quoted_total: dec(15000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await;

    match try_sale_no_mandatory {
        Err(ApiError::Domain(DomainError::Coded { code, data, .. })) => {
            assert_eq!(code, "OPTION_SELECTION_INVALID");
            let data_obj = data.expect("must contain data");
            assert_eq!(
                data_obj.get("group_id").and_then(|v| v.as_str()),
                Some(group_termino.id.to_string().as_str())
            );
        }
        other => panic!("expected OPTION_SELECTION_INVALID, got {:?}", other),
    }

    // [P] "Extra queso" con un delta de $2.000 y 30 g de queso: el precio sube $2.000 por unidad y el queso baja 30 g por unidad.
    // Compramos 2 unidades de hamburguesa con extra queso y término bien cocido
    // Precio unitario: 15.000 + 2.000 = 17.000. Total por 2 = 34.000.
    let sale_id = Uuid::new_v4();
    let ok_sale = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(burger.row.id),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![opt_bien_cocido, opt_extra_queso],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(34000),
                reference: None,
            }],
            quoted_total: dec(34000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let created_sale = match ok_sale {
        sales::Checkout::Created(s) => s,
        sales::Checkout::Replayed(s) => s,
    };
    assert_eq!(created_sale.order.total_amount, dec(34000));

    // Verificar stock de hamburguesa: 10 - 2 = 8
    let burger_stock: Decimal = sqlx::query_scalar(
        "select on_hand from stock where tenant_id = $1 and branch_id = $2 and product_id = $3",
    )
    .bind(tenant)
    .bind(branch)
    .bind(burger.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(burger_stock, dec(8));

    // Verificar stock de queso: 1000 - (2 * 30) = 940
    let queso_stock: Decimal = sqlx::query_scalar(
        "select on_hand from stock where tenant_id = $1 and branch_id = $2 and product_id = $3",
    )
    .bind(tenant)
    .bind(branch)
    .bind(queso.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(queso_stock, dec(940));

    // Verificar registro en order_item_options
    let order_options_count: i64 =
        sqlx::query_scalar("select count(*) from order_item_options where tenant_id = $1")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        order_options_count, 2,
        "deben guardarse 2 opciones en order_item_options"
    );

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn f2_archive(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch_a = seed_branch(admin, tenant).await;
    let branch_b = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner_a = claims(owner, Some(tenant), Some(branch_a));
    let c_owner_b = claims(owner, Some(tenant), Some(branch_b));
    let c_owner_tenant = claims(owner, Some(tenant), None);

    // 1. Crear producto con costo $5.000 y precio $10.000
    let prod = products::create_product(
        app,
        &c_owner_tenant,
        products::ProductInput {
            name: "Producto Archivable".into(),
            sku: Some("ARCH-01".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(5000)),
            base_price: Some(dec(10000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // 2. Reponer stock: 10 en sucursal A y 5 en sucursal B
    inventory::replenish(
        app,
        &c_owner_a,
        branch_a,
        prod.row.id,
        dec(10),
        "stock branch A",
    )
    .await
    .unwrap();
    inventory::replenish(
        app,
        &c_owner_b,
        branch_b,
        prod.row.id,
        dec(5),
        "stock branch B",
    )
    .await
    .unwrap();

    // 3. [P] Sin confirmar responde 409 ARCHIVE_NEEDS_CONFIRM con el plan y total_value
    let try_archive_no_confirm =
        products::archive_product(app, &c_owner_tenant, prod.row.id, false).await;

    match try_archive_no_confirm {
        Err(ApiError::Domain(DomainError::Coded { code, data, .. })) => {
            assert_eq!(code, "ARCHIVE_NEEDS_CONFIRM");
            let data_obj = data.expect("must contain data");
            let plan = data_obj
                .get("plan")
                .and_then(|v| v.as_array())
                .expect("must have plan array");
            assert_eq!(plan.len(), 2, "debe tener 2 ubicaciones con stock");
            let total_val = data_obj
                .get("total_value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // 15 unidades * 5000 = 75000.00
            assert!(
                total_val.starts_with("75000"),
                "total_value debe ser 75000, got {}",
                total_val
            );
        }
        other => panic!("expected ARCHIVE_NEEDS_CONFIRM, got {:?}", other),
    }

    // 4. [P] Al confirmar, el stock queda en 0 con un movimiento por ubicación
    let archive_confirmed = products::archive_product(app, &c_owner_tenant, prod.row.id, true)
        .await
        .unwrap();
    assert_eq!(
        archive_confirmed.get("status").and_then(|v| v.as_str()),
        Some("ok")
    );

    // Verificar que stock en ambas sucursales quedó en 0
    let stock_a: Decimal = sqlx::query_scalar(
        "select on_hand from stock where tenant_id = $1 and branch_id = $2 and product_id = $3",
    )
    .bind(tenant)
    .bind(branch_a)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(stock_a, Decimal::ZERO, "stock en sucursal A debe ser 0");

    let stock_b: Decimal = sqlx::query_scalar(
        "select on_hand from stock where tenant_id = $1 and branch_id = $2 and product_id = $3",
    )
    .bind(tenant)
    .bind(branch_b)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(stock_b, Decimal::ZERO, "stock en sucursal B debe ser 0");

    // Verificar movimientos registrados con ref_type = 'ARCHIVE' y cause = 'Baja por archivo'
    let movs_count: i64 = sqlx::query_scalar(
        "select count(*) from inventory_movements
          where tenant_id = $1 and product_id = $2 and ref_type = 'ARCHIVE' and ref_id = $2",
    )
    .bind(tenant)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        movs_count, 2,
        "debe haber 2 movimientos de baja por archivo"
    );

    // Verificar que el producto quedó archivado
    let is_archived: bool = sqlx::query_scalar(
        "select archived_at is not null from products where tenant_id = $1 and id = $2",
    )
    .bind(tenant)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(is_archived, "producto debe estar archivado");

    // 5. [P] Reactivar sin stock mediante unarchive
    let unarchive_res = products::unarchive_product(app, &c_owner_tenant, prod.row.id)
        .await
        .unwrap();
    assert_eq!(
        unarchive_res.get("status").and_then(|v| v.as_str()),
        Some("ok")
    );

    let is_still_archived: bool = sqlx::query_scalar(
        "select archived_at is not null from products where tenant_id = $1 and id = $2",
    )
    .bind(tenant)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(
        !is_still_archived,
        "producto no debe estar archivado tras unarchive"
    );

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_import(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    // 1. [P] Un código 7.70123E+12 da ERROR
    let csv_with_sci_error = "sku,codigo_barras,nombre,categoria,unidad,impuesto,costo,precio,cantidad_inicial,sku_padre,atributos\n\
        ERR-01,7.70123E+12,Producto Malo,General,UND,IVA19,1000,2000,0,,\n";

    let resp_err =
        crate::csv_import::analyze_csv(app, &c_owner, branch, csv_with_sci_error.as_bytes())
            .await
            .unwrap();

    assert_eq!(resp_err.summary.errors, 1);
    assert_eq!(resp_err.rows[0].status, "ERROR");
    assert!(
        resp_err.rows[0]
            .messages
            .iter()
            .any(|m| m.contains("código en notación científica"))
    );

    // 2. [P] Un CSV con errores no se confirma (responde 409 IMPORT_HAS_ERRORS)
    let commit_err_result = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_err.session_id,
            offset: 0,
            limit: 200,
        },
    )
    .await;

    match commit_err_result {
        Err(ApiError::Domain(DomainError::Coded { code, .. })) => {
            assert_eq!(code, "IMPORT_HAS_ERRORS");
        }
        other => panic!("expected IMPORT_HAS_ERRORS, got {:?}", other),
    }

    // 3. [P] Un CSV válido crea los productos y las variantes, y un reintento del mismo rango no duplica nada
    let valid_csv = "sku;codigo_barras;nombre;categoria;unidad;impuesto;costo;precio;cantidad_inicial;sku_padre;atributos\n\
        PADRE-01;;Camiseta Base;Ropa;UND;IVA19;10000;20000;0;;\n\
        HIJO-01;7701234567890;Camiseta Roja M;Ropa;UND;IVA19;10000;22000;5;PADRE-01;Talla=M|Color=Rojo\n\
        PROD-SOLO;7701234567891;Gorra Negra;Accesorios;UND;IVA19;5000;12000;10;;\n";

    let resp_valid = crate::csv_import::analyze_csv(app, &c_owner, branch, valid_csv.as_bytes())
        .await
        .unwrap();

    assert_eq!(resp_valid.summary.errors, 0);
    assert_eq!(resp_valid.summary.ok + resp_valid.summary.warnings, 3);

    // Commit primer bloque (filas 0 y 1: padre y variante)
    let commit_batch1 = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_valid.session_id,
            offset: 0,
            limit: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(commit_batch1.committed_rows, 2);
    assert!(!commit_batch1.done);

    // Reintento del mismo rango (idempotente)
    let commit_retry = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_valid.session_id,
            offset: 0,
            limit: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(commit_retry.committed_rows, 2);
    assert!(!commit_retry.done);

    // Commit segundo bloque (fila 2: gorra negra)
    let commit_batch2 = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_valid.session_id,
            offset: 2,
            limit: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(commit_batch2.committed_rows, 3);
    assert!(commit_batch2.done);

    // Verificar en base de datos que se crearon los productos
    let total_products: i64 =
        sqlx::query_scalar("select count(*) from products where tenant_id = $1")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        total_products, 3,
        "deben haberse creado 3 productos en total"
    );

    // Verificar que la variante HIJO-01 tiene parent_id = PADRE-01
    let parent_id: Uuid =
        sqlx::query_scalar("select id from products where tenant_id = $1 and sku = 'PADRE-01'")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap();

    let variant_parent: Option<Uuid> = sqlx::query_scalar(
        "select parent_id from products where tenant_id = $1 and sku = 'HIJO-01'",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        variant_parent,
        Some(parent_id),
        "variante debe referenciar al padre"
    );

    // Verificar stock inicial cargado (5 en HIJO-01 y 10 en PROD-SOLO)
    let stock_variant: Decimal = sqlx::query_scalar(
        "select on_hand from stock where branch_id = $1 and product_id = (select id from products where sku = 'HIJO-01' and tenant_id = $2)",
    )
    .bind(branch)
    .bind(tenant)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        stock_variant,
        dec(5),
        "variante debe tener 5 de stock inicial"
    );

    let stock_solo: Decimal = sqlx::query_scalar(
        "select on_hand from stock where branch_id = $1 and product_id = (select id from products where sku = 'PROD-SOLO' and tenant_id = $2)",
    )
    .bind(branch)
    .bind(tenant)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(stock_solo, dec(10), "gorra debe tener 10 de stock inicial");

    // 4. [P] Importar un producto con cantidad inicial 10 y costo 2.500 deja on_hand = 10, avg_cost = 2500 y un movimiento AJUSTE_ENTRADA con ref_type = 'IMPORT' y cause = 'Carga inicial'
    let csv_with_cost = "sku,codigo_barras,nombre,categoria,unidad,impuesto,costo,precio,cantidad_inicial,sku_padre,atributos\n\
        PROD-COST,7701234567892,Producto Con Costo,General,UND,IVA19,2500,5000,10,,\n";

    let resp_cost = crate::csv_import::analyze_csv(app, &c_owner, branch, csv_with_cost.as_bytes())
        .await
        .unwrap();
    assert_eq!(resp_cost.summary.errors, 0);

    let commit_cost = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_cost.session_id,
            offset: 0,
            limit: 10,
        },
    )
    .await
    .unwrap();
    assert!(commit_cost.done);

    let prod_cost_id: Uuid =
        sqlx::query_scalar("select id from products where tenant_id = $1 and sku = 'PROD-COST'")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap();

    let (cost_on_hand, cost_avg): (Decimal, Decimal) = sqlx::query_as(
        "select on_hand, avg_cost from stock where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(prod_cost_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(cost_on_hand, dec(10));
    assert_eq!(cost_avg, dec(2500));

    let mov_cost: (String, String) = sqlx::query_as(
        "select ref_type, cause from inventory_movements where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(prod_cost_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_cost.0, "IMPORT");
    assert_eq!(mov_cost.1, "Carga inicial");

    // 5. [P] Importar un producto nuevo con cantidad inicial 5 y sin costo deja on_hand = 5, avg_cost = 0, products.cost_price sin cambio (null) y un movimiento AJUSTE_ENTRADA con unit_cost = 0
    let csv_no_cost = "sku,codigo_barras,nombre,categoria,unidad,impuesto,costo,precio,cantidad_inicial,sku_padre,atributos\n\
        PROD-NO-COST,7701234567893,Producto Sin Costo,General,UND,IVA19,,5000,5,,\n";

    let resp_no_cost =
        crate::csv_import::analyze_csv(app, &c_owner, branch, csv_no_cost.as_bytes())
            .await
            .unwrap();
    assert_eq!(resp_no_cost.summary.errors, 0);

    let commit_no_cost = crate::csv_import::commit_import(
        app,
        &c_owner,
        crate::csv_import::CommitRequest {
            session_id: resp_no_cost.session_id,
            offset: 0,
            limit: 10,
        },
    )
    .await
    .unwrap();
    assert!(commit_no_cost.done);

    let prod_no_cost_id: Uuid =
        sqlx::query_scalar("select id from products where tenant_id = $1 and sku = 'PROD-NO-COST'")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap();

    let (no_cost_on_hand, no_cost_avg): (Decimal, Decimal) = sqlx::query_as(
        "select on_hand, avg_cost from stock where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(prod_no_cost_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(no_cost_on_hand, dec(5));
    assert_eq!(no_cost_avg, dec(0));

    let prod_cost_price: Option<Decimal> =
        sqlx::query_scalar("select cost_price from products where id = $1")
            .bind(prod_no_cost_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert!(prod_cost_price.is_none());

    let mov_unit_cost: Decimal = sqlx::query_scalar(
        "select unit_cost from inventory_movements where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(prod_no_cost_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_unit_cost, dec(0));

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_locations(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    // 1. [P] Las ubicaciones AVERIADOS y CUARENTENA no se pueden marcar vendibles
    let err_averiados = crate::locations::create_location(
        app,
        &c_owner,
        crate::locations::CreateLocationInput {
            branch_id: branch,
            name: "Averiados Vendible".into(),
            kind: "AVERIADOS".into(),
            sellable: true,
            is_default: Some(false),
        },
    )
    .await;
    assert!(is_validation(&err_averiados));

    let err_cuarentena = crate::locations::create_location(
        app,
        &c_owner,
        crate::locations::CreateLocationInput {
            branch_id: branch,
            name: "Cuarentena Vendible".into(),
            kind: "CUARENTENA".into(),
            sellable: true,
            is_default: Some(false),
        },
    )
    .await;
    assert!(is_validation(&err_cuarentena));

    // Crear AVERIADOS no vendible es válido
    let loc_averiados = crate::locations::create_location(
        app,
        &c_owner,
        crate::locations::CreateLocationInput {
            branch_id: branch,
            name: "Merma y Daños".into(),
            kind: "AVERIADOS".into(),
            sellable: false,
            is_default: Some(false),
        },
    )
    .await
    .unwrap();
    assert!(!loc_averiados.sellable);

    // Intentar cambiarla a vendible mediante update da 422 LOCATION_INVALID
    let err_update = crate::locations::update_location(
        app,
        &c_owner,
        loc_averiados.id,
        crate::locations::UpdateLocationInput {
            name: None,
            kind: None,
            sellable: Some(true),
            is_default: None,
            active: None,
        },
    )
    .await;
    assert!(is_validation(&err_update));

    // 2. [P] Venta descuenta de la ubicación de la caja
    // Obtener ubicación Principal creada automáticamente
    let principal_id: Uuid = sqlx::query_scalar(
        "select id from locations where branch_id = $1 and is_default = true limit 1",
    )
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();

    // Crear "Bodega Auxiliar" vendible
    let bodega_aux = crate::locations::create_location(
        app,
        &c_owner,
        crate::locations::CreateLocationInput {
            branch_id: branch,
            name: "Bodega Auxiliar".into(),
            kind: "BODEGA".into(),
            sellable: true,
            is_default: Some(false),
        },
    )
    .await
    .unwrap();

    // Producto de prueba
    let prod = seed_product(admin, tenant, "Arroz Diana 1kg", "UND", None, false).await;
    set_base_price(admin, prod, "5000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;

    // Stock: 10 en Principal, 20 en Bodega Auxiliar
    seed_stock_location(admin, tenant, branch, principal_id, prod, "10", "0").await;
    seed_stock_location(admin, tenant, branch, bodega_aux.id, prod, "20", "0").await;

    // Crear caja vinculada a Bodega Auxiliar
    let reg_aux = crate::registers::create(
        app,
        &c_owner,
        crate::registers::CreateRegisterRequest {
            branch_id: branch,
            code: "CAJA-AUX".into(),
            name: "Caja Auxiliar".into(),
            default_opening: dec(10000),
            location_id: Some(bodega_aux.id),
        },
    )
    .await
    .unwrap();

    // Abrir turno en reg_aux
    let _session = crate::cash::open(app, &c_owner, branch, reg_aux.id, dec(10000))
        .await
        .unwrap();

    // Vender 2 unidades
    let sale_req = sale(
        Uuid::new_v4(),
        branch,
        &[(prod, 2)],
        &[(crate::sales::PaymentMethod::Efectivo, 10000)],
        dec(10000),
    );
    let s_resp = created(
        crate::sales::checkout(app, &c_owner, sale_req)
            .await
            .unwrap(),
    );
    assert_eq!(s_resp.order.status, "COBRADA");

    // Verificar existencias:
    // Bodega Auxiliar debe haber bajado de 20 a 18
    let stock_aux: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(bodega_aux.id)
            .bind(prod)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        stock_aux,
        dec(18),
        "Bodega Auxiliar debe tener 18 tras la venta"
    );

    // Principal debe mantenerse intacta en 10
    let stock_princ: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(principal_id)
            .bind(prod)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(stock_princ, dec(10), "Principal debe permanecer en 10");

    // 3. [P] Un traslado interno mueve el stock entre dos ubicaciones de la misma sucursal y deja dos movimientos
    let transfer_res = crate::inventory::transfer_locations(
        app,
        &c_owner,
        crate::inventory::TransferArgs {
            from_branch_id: None,
            to_branch_id: None,
            from_location_id: Some(principal_id),
            to_location_id: Some(bodega_aux.id),
            product_id: prod,
            qty: dec(3),
            cause: "Traslado interno a bodega auxiliar".into(),
            lot_id: None,
        },
    )
    .await
    .unwrap();

    // Existencias post-traslado:
    // Principal: 10 - 3 = 7
    let stock_princ_after: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(principal_id)
            .bind(prod)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        stock_princ_after,
        dec(7),
        "Principal debe tener 7 tras traslado"
    );

    // Bodega Auxiliar: 18 + 3 = 21
    let stock_aux_after: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(bodega_aux.id)
            .bind(prod)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        stock_aux_after,
        dec(21),
        "Bodega Auxiliar debe tener 21 tras traslado"
    );

    // Verificar que se generaron dos movimientos de inventario con el mismo ref_id
    let movements: Vec<(String, Uuid, Decimal)> = sqlx::query_as(
        "select t.tima_codigo, im.location_id, im.qty
         from inventory_movements im
         join tbl_tipos_maestra t on t.tima_id = im.movement_type_id
         where im.ref_id = $1
         order by im.created_at asc",
    )
    .bind(transfer_res)
    .fetch_all(admin)
    .await
    .unwrap();

    assert_eq!(
        movements.len(),
        2,
        "deben existir dos movimientos por traslado"
    );
    assert_eq!(movements[0].0, "TRASLADO_SALIDA");
    assert_eq!(movements[0].1, principal_id);
    assert_eq!(movements[0].2, dec(3));

    assert_eq!(movements[1].0, "TRASLADO_ENTRADA");
    assert_eq!(movements[1].1, bodega_aux.id);
    assert_eq!(movements[1].2, dec(3));

    // Intentar desactivar bodega con stock: 409 LOCATION_HAS_STOCK
    let err_deact = crate::locations::update_location(
        app,
        &c_owner,
        bodega_aux.id,
        crate::locations::UpdateLocationInput {
            name: None,
            kind: None,
            sellable: None,
            is_default: None,
            active: Some(false),
        },
    )
    .await;
    assert!(is_conflict(&err_deact));

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_pipeline(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let principal_id: Uuid = sqlx::query_scalar(
        "select id from locations where branch_id = $1 and is_default = true limit 1",
    )
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();

    let reg = crate::registers::create(
        app,
        &c_owner,
        crate::registers::CreateRegisterRequest {
            branch_id: branch,
            code: "CAJA-PIPE".to_string(),
            name: "Caja Pipeline".to_string(),
            default_opening: dec(100000),
            location_id: Some(principal_id),
        },
    )
    .await
    .unwrap();

    // Abrir turno de caja
    let _session = crate::cash::open(app, &c_owner, branch, reg.id, dec(100000))
        .await
        .unwrap();

    // Crear producto estándar
    let prod_id = seed_product(admin, tenant, "Producto Pipeline", "UND", None, false).await;
    set_base_price(admin, prod_id, "1000").await;
    seed_branch_product(admin, tenant, branch, prod_id, None, None).await;

    // Fijar punto de reorden en 5
    crate::inventory::set_min_qty(app, &c_owner, branch, prod_id, dec(5))
        .await
        .unwrap();

    // 1. Reponer 1 unidad
    crate::inventory::replenish(app, &c_owner, branch, prod_id, dec(1), "stock inicial")
        .await
        .unwrap();

    let (oh, res) = stock_row(admin, branch, prod_id).await;
    assert_eq!(oh, dec(1));
    assert_eq!(res, dec(0));

    // [P] Dos ventas concurrentes por la última unidad: solo una pasa y la otra recibe 409 STOCK_INSUFFICIENT
    let sale_req_1 = sale(
        Uuid::new_v4(),
        branch,
        &[(prod_id, 1)],
        &[(crate::sales::PaymentMethod::Efectivo, 1000)],
        dec(1000),
    );
    let sale_req_2 = sale(
        Uuid::new_v4(),
        branch,
        &[(prod_id, 1)],
        &[(crate::sales::PaymentMethod::Efectivo, 1000)],
        dec(1000),
    );

    let (r1, r2) = tokio::join!(
        crate::sales::checkout(app, &c_owner, sale_req_1),
        crate::sales::checkout(app, &c_owner, sale_req_2),
    );

    let (winner, loser) = match (r1, r2) {
        (Ok(w), Err(l)) => (w, l),
        (Err(l), Ok(w)) => (w, l),
        (Ok(_), Ok(_)) => panic!("Both concurrent sales succeeded! Exactly one must win"),
        (Err(e1), Err(e2)) => panic!("Both concurrent sales failed: {:?}, {:?}", e1, e2),
    };

    assert_eq!(created(winner).order.status, "COBRADA");
    assert_eq!(loser.status(), axum::http::StatusCode::CONFLICT);

    let (oh_after, _) = stock_row(admin, branch, prod_id).await;
    assert_eq!(oh_after, dec(0));

    // [P] Bajar de min_qty crea alerta BAJO, y reponer la resuelve
    let alerts_low = crate::inventory::list_alerts(app, &c_owner, branch)
        .await
        .unwrap();
    assert!(
        alerts_low
            .iter()
            .any(|a| a.product_id == prod_id && a.kind == "BAJO" && a.resolved_at.is_none()),
        "debe existir una alerta BAJO abierta para el producto"
    );

    // Reponer 10 unidades (> min_qty de 5)
    crate::inventory::replenish(
        app,
        &c_owner,
        branch,
        prod_id,
        dec(10),
        "reponer sobre min_qty",
    )
    .await
    .unwrap();

    let alerts_after_rep = crate::inventory::list_alerts(app, &c_owner, branch)
        .await
        .unwrap();
    assert!(
        !alerts_after_rep
            .iter()
            .any(|a| a.product_id == prod_id && a.kind == "BAJO"),
        "la alerta BAJO debe estar resuelta tras reponer sobre min_qty"
    );

    // [P] allow_negative_stock = true: stock queda negativo y se crea una alerta NEGATIVO. Nunca se recorta a 0.
    let mut current_settings = crate::settings::get_settings(app, &c_owner, branch)
        .await
        .unwrap();
    current_settings.inventory.allow_negative_stock = true;
    crate::settings::update_settings(
        app,
        &c_owner,
        crate::settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: current_settings,
        },
    )
    .await
    .unwrap();

    crate::inventory::adjust(app, &c_owner, branch, prod_id, dec(-12), "ajuste negativo")
        .await
        .unwrap();

    let (oh_neg, _) = stock_row(admin, branch, prod_id).await;
    assert_eq!(
        oh_neg,
        dec(-2),
        "stock debe quedar en -2 y nunca recortarse a 0"
    );

    let alerts_neg = crate::inventory::list_alerts(app, &c_owner, branch)
        .await
        .unwrap();
    assert!(
        alerts_neg
            .iter()
            .any(|a| a.product_id == prod_id && a.kind == "NEGATIVO" && a.resolved_at.is_none()),
        "debe existir una alerta NEGATIVO abierta"
    );

    cleanup(admin, tenant, &[owner]).await;
}

/// Costeo promedio y PEPS (F2.10).
async fn f2_costing(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let annex = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), None);

    let (_scope, mut tx) = tenancy::begin(app, &c_owner).await.unwrap();
    let def_loc = crate::locations::default_location_id_tx(&mut tx, branch)
        .await
        .unwrap();
    let annex_loc = crate::locations::default_location_id_tx(&mut tx, annex)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let reg = crate::registers::create(
        app,
        &c_owner,
        crate::registers::CreateRegisterRequest {
            branch_id: branch,
            code: "CAJA-COST".to_string(),
            name: "Caja Costeo".to_string(),
            default_opening: dec(100000),
            location_id: Some(def_loc),
        },
    )
    .await
    .unwrap();

    let _session = crate::cash::open(app, &c_owner, branch, reg.id, dec(100000))
        .await
        .unwrap();

    // 1. [P] Promedio: entran 10 a $100 y 10 a $200, y el promedio queda en $150.
    // Luego salen 5 con un costo de venta de $750.
    let p_prom = seed_product(admin, tenant, "Prod Promedio", "UND", None, false).await;
    set_base_price(admin, p_prom, "1000").await;
    seed_branch_product(admin, tenant, branch, p_prom, None, None).await;

    // Entrada 1: 10 @ 100
    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    crate::inventory::apply_delta_tx(
        &mut tx,
        &scope,
        crate::inventory::StockDelta {
            branch_id: branch,
            location_id: def_loc,
            product_id: p_prom,
            delta: dec(10),
            movement: "REPOSICION",
            unit_cost: Some(dec(100)),
            lot_id: None,
            ref_type: "test",
            ref_id: Uuid::new_v4(),
            cause: Some("Entrada 1".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Entrada 2: 10 @ 200
    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    crate::inventory::apply_delta_tx(
        &mut tx,
        &scope,
        crate::inventory::StockDelta {
            branch_id: branch,
            location_id: def_loc,
            product_id: p_prom,
            delta: dec(10),
            movement: "REPOSICION",
            unit_cost: Some(dec(200)),
            lot_id: None,
            ref_type: "test",
            ref_id: Uuid::new_v4(),
            cause: Some("Entrada 2".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Verificar avg_cost en stock
    let avg: Decimal =
        sqlx::query_scalar("select avg_cost from stock where location_id = $1 and product_id = $2")
            .bind(def_loc)
            .bind(p_prom)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(avg, dec(150), "promedio ponderado debe ser 150");

    // Salen 5 unidades mediante checkout
    let sale_prom = sale(
        Uuid::new_v4(),
        branch,
        &[(p_prom, 5)],
        &[(crate::sales::PaymentMethod::Efectivo, 5000)],
        dec(5000),
    );
    let s_resp = created(
        crate::sales::checkout(app, &c_owner, sale_prom)
            .await
            .unwrap(),
    );

    // Verificar order_items
    let (cost_tot, cost_u, cost_miss): (Decimal, Decimal, bool) = sqlx::query_as(
        "select cost_total, unit_cost, cost_missing from order_items where order_id = $1",
    )
    .bind(s_resp.order.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        cost_tot,
        dec(750),
        "costo de venta en promedio de 5 @ 150 debe ser 750"
    );
    assert_eq!(cost_u, dec(150), "costo unitario debe ser 150");
    assert!(!cost_miss, "cost_missing debe ser falso");

    // 2. [P] PEPS con las mismas entradas: salen 15 y el costo es 10 × 100 + 5 × 200 = $2.000.
    let mut st = crate::settings::get_settings(app, &c_owner, branch)
        .await
        .unwrap();
    st.inventory.costing = "PEPS".to_string();
    crate::settings::update_settings(
        app,
        &c_owner,
        crate::settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: st,
        },
    )
    .await
    .unwrap();

    let p_peps = seed_product(admin, tenant, "Prod PEPS", "UND", None, false).await;
    set_base_price(admin, p_peps, "1000").await;
    seed_branch_product(admin, tenant, branch, p_peps, None, None).await;

    // Entrada 1: 10 @ 100
    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    crate::inventory::apply_delta_tx(
        &mut tx,
        &scope,
        crate::inventory::StockDelta {
            branch_id: branch,
            location_id: def_loc,
            product_id: p_peps,
            delta: dec(10),
            movement: "REPOSICION",
            unit_cost: Some(dec(100)),
            lot_id: None,
            ref_type: "test",
            ref_id: Uuid::new_v4(),
            cause: Some("Entrada 1 PEPS".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Entrada 2: 10 @ 200
    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    crate::inventory::apply_delta_tx(
        &mut tx,
        &scope,
        crate::inventory::StockDelta {
            branch_id: branch,
            location_id: def_loc,
            product_id: p_peps,
            delta: dec(10),
            movement: "REPOSICION",
            unit_cost: Some(dec(200)),
            lot_id: None,
            ref_type: "test",
            ref_id: Uuid::new_v4(),
            cause: Some("Entrada 2 PEPS".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Salen 15 unidades en venta
    let sale_peps = sale(
        Uuid::new_v4(),
        branch,
        &[(p_peps, 15)],
        &[(crate::sales::PaymentMethod::Efectivo, 15000)],
        dec(15000),
    );
    let s_peps_resp = created(
        crate::sales::checkout(app, &c_owner, sale_peps)
            .await
            .unwrap(),
    );
    let (c_tot_peps, _, c_miss_peps): (Decimal, Decimal, bool) = sqlx::query_as(
        "select cost_total, unit_cost, cost_missing from order_items where order_id = $1",
    )
    .bind(s_peps_resp.order.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(
        c_tot_peps,
        dec(2000),
        "costo PEPS para 15 unidades debe ser 10*100 + 5*200 = 2000"
    );
    assert!(!c_miss_peps, "cost_missing debe ser falso");

    // 3. [P] Respaldo: un producto sin capas, sin promedio y con cost_price = 80 cuesta 80 por unidad.
    // Si cost_price es nulo, el costo es 0 y cost_missing = true.
    let mut st = crate::settings::get_settings(app, &c_owner, branch)
        .await
        .unwrap();
    st.inventory.allow_negative_stock = true;
    crate::settings::update_settings(
        app,
        &c_owner,
        crate::settings::UpdateSettingsRequest {
            branch_id: branch,
            settings: st,
        },
    )
    .await
    .unwrap();

    let p_respaldo_80 = seed_product(admin, tenant, "Prod Respaldo 80", "UND", None, false).await;
    set_base_price(admin, p_respaldo_80, "1000").await;
    seed_branch_product(admin, tenant, branch, p_respaldo_80, None, None).await;
    sqlx::query("update products set cost_price = 80 where id = $1")
        .bind(p_respaldo_80)
        .execute(admin)
        .await
        .unwrap();

    let sale_respaldo = sale(
        Uuid::new_v4(),
        branch,
        &[(p_respaldo_80, 2)],
        &[(crate::sales::PaymentMethod::Efectivo, 2000)],
        dec(2000),
    );
    let s_resp_80 = created(
        crate::sales::checkout(app, &c_owner, sale_respaldo)
            .await
            .unwrap(),
    );
    let (cost_tot_80, cost_u_80, cost_miss_80): (Decimal, Decimal, bool) = sqlx::query_as(
        "select cost_total, unit_cost, cost_missing from order_items where order_id = $1",
    )
    .bind(s_resp_80.order.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(cost_tot_80, dec(160), "2 * 80 = 160");
    assert_eq!(cost_u_80, dec(80));
    assert!(!cost_miss_80);

    let p_respaldo_null =
        seed_product(admin, tenant, "Prod Respaldo Null", "UND", None, false).await;
    set_base_price(admin, p_respaldo_null, "1000").await;
    seed_branch_product(admin, tenant, branch, p_respaldo_null, None, None).await;
    sqlx::query("update products set cost_price = null where id = $1")
        .bind(p_respaldo_null)
        .execute(admin)
        .await
        .unwrap();

    let sale_null = sale(
        Uuid::new_v4(),
        branch,
        &[(p_respaldo_null, 2)],
        &[(crate::sales::PaymentMethod::Efectivo, 2000)],
        dec(2000),
    );
    let s_resp_null = created(
        crate::sales::checkout(app, &c_owner, sale_null)
            .await
            .unwrap(),
    );
    let (cost_tot_null, cost_u_null, cost_miss_null): (Decimal, Decimal, bool) = sqlx::query_as(
        "select cost_total, unit_cost, cost_missing from order_items where order_id = $1",
    )
    .bind(s_resp_null.order.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(cost_tot_null, dec(0));
    assert_eq!(cost_u_null, dec(0));
    assert!(
        cost_miss_null,
        "cost_missing debe ser verdadero cuando cost_price es null"
    );

    // 4. [P] Un traslado conserva el costo de origen.
    let p_trans = seed_product(admin, tenant, "Prod Traslado", "UND", None, false).await;
    set_base_price(admin, p_trans, "2000").await;
    seed_branch_product(admin, tenant, branch, p_trans, None, None).await;
    seed_branch_product(admin, tenant, annex, p_trans, None, None).await;

    // Entrada en branch de 10 @ 120
    let (scope, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    crate::inventory::apply_delta_tx(
        &mut tx,
        &scope,
        crate::inventory::StockDelta {
            branch_id: branch,
            location_id: def_loc,
            product_id: p_trans,
            delta: dec(10),
            movement: "REPOSICION",
            unit_cost: Some(dec(120)),
            lot_id: None,
            ref_type: "test",
            ref_id: Uuid::new_v4(),
            cause: Some("Entrada origen".into()),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Trasladar 4 unidades de branch a annex
    crate::inventory::transfer_locations(
        app,
        &c_owner,
        crate::inventory::TransferArgs {
            from_branch_id: Some(branch),
            to_branch_id: Some(annex),
            from_location_id: Some(def_loc),
            to_location_id: Some(annex_loc),
            product_id: p_trans,
            qty: dec(4),
            cause: "Traslado a anexo".into(),
            lot_id: None,
        },
    )
    .await
    .unwrap();

    let avg_annex: Decimal =
        sqlx::query_scalar("select avg_cost from stock where location_id = $1 and product_id = $2")
            .bind(annex_loc)
            .bind(p_trans)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        avg_annex,
        dec(120),
        "destino debe conservar costo unitario del origen (120)"
    );

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_suppliers(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), None);

    let branch = seed_branch(admin, tenant).await;
    let (_scope, mut tx) = tenancy::begin(app, &c_owner).await.unwrap();
    let def_loc = crate::locations::default_location_id_tx(&mut tx, branch)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let prod = seed_product(admin, tenant, "F2 Suppliers Product", "UND", None, false).await;

    // 1. [P] Un DV incorrecto: 422 NIT_DV_INVALID.
    // Para NIT "900123456", el DV real es 8. Si enviamos 5 debe fallar con NIT_DV_INVALID.
    let err_dv = crate::procurement::create_supplier(
        app,
        &c_owner,
        crate::procurement::CreateSupplierRequest {
            name: "Proveedor DV Malo".into(),
            tax_id: Some("900123456".into()),
            doc_type: Some("NIT".into()),
            dv: Some("5".into()),
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            delivery_days: Some(5),
            category: Some("MERCANCIA".into()),
            bank_name: None,
            bank_account_type: None,
            bank_account_number: None,
            contact_name: None,
            phone: None,
            email: None,
            address: None,
            notes: None,
        },
    )
    .await;

    match err_dv {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "NIT_DV_INVALID");
        }
        other => panic!("esperaba 422 NIT_DV_INVALID pero obtuve {:?}", other),
    }

    // Creación exitosa: sin DV (debe calcular 8 automáticamente)
    let s_ok = crate::procurement::create_supplier(
        app,
        &c_owner,
        crate::procurement::CreateSupplierRequest {
            name: "Proveedor Exitoso".into(),
            tax_id: Some("900123456".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            delivery_days: Some(3),
            category: Some("MERCANCIA".into()),
            bank_name: Some("Bancolombia".into()),
            bank_account_type: Some("AHORROS".into()),
            bank_account_number: Some("123456789".into()),
            contact_name: Some("Juan Perez".into()),
            phone: None,
            email: None,
            address: None,
            notes: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(s_ok.dv.as_deref(), Some("8"));

    // 2. [P] Archivar con una orden abierta: 409 SUPPLIER_HAS_OPEN_DOCS.
    let status_pending: i32 = sqlx::query_scalar("select public.tipo_id('PUR', 'PENDIENTE')")
        .fetch_one(admin)
        .await
        .unwrap();

    let purchase_id = Uuid::new_v4();
    sqlx::query(
        "insert into purchase_receipts (id, tenant_id, branch_id, supplier_id, document_number, total_amount, status_id, location_id)
         values ($1, $2, $3, $4, 'OC-001', 50000, $5, $6)"
    )
    .bind(purchase_id)
    .bind(tenant)
    .bind(branch)
    .bind(s_ok.id)
    .bind(status_pending)
    .bind(def_loc)
    .execute(admin)
    .await
    .unwrap();

    let err_arch = crate::procurement::archive_supplier(app, &c_owner, s_ok.id).await;
    match err_arch {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "SUPPLIER_HAS_OPEN_DOCS");
        }
        other => panic!(
            "esperaba 409 SUPPLIER_HAS_OPEN_DOCS pero obtuve {:?}",
            other
        ),
    }

    // Cancelar la orden para permitir archivar
    let status_cancel: i32 = sqlx::query_scalar("select public.tipo_id('PUR', 'ANULADA')")
        .fetch_one(admin)
        .await
        .unwrap();

    sqlx::query("update purchase_receipts set status_id = $1 where id = $2")
        .bind(status_cancel)
        .bind(purchase_id)
        .execute(admin)
        .await
        .unwrap();

    crate::procurement::archive_supplier(app, &c_owner, s_ok.id)
        .await
        .expect("debe poder archivar sin órdenes abiertas");

    // Verificar productos por proveedor y switch de preferred
    let s2 = crate::procurement::create_supplier(
        app,
        &c_owner,
        crate::procurement::CreateSupplierRequest {
            name: "Proveedor Secundario".into(),
            tax_id: Some("800197268".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            delivery_days: None,
            category: Some("MERCANCIA".into()),
            bank_name: None,
            bank_account_type: None,
            bank_account_number: None,
            contact_name: None,
            phone: None,
            email: None,
            address: None,
            notes: None,
        },
    )
    .await
    .unwrap();

    crate::procurement::set_supplier_products(
        app,
        &c_owner,
        s2.id,
        vec![crate::procurement::SupplierProductItemInput {
            product_id: prod,
            supplier_code: Some("PROD-S2".into()),
            min_order_qty: Some(dec(5)),
            preferred: true,
        }],
    )
    .await
    .unwrap();

    let det2 = crate::procurement::get_supplier(app, &c_owner, s2.id)
        .await
        .unwrap();
    assert_eq!(det2.products.len(), 1);
    assert!(det2.products[0].preferred);

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_purchases(app: &PgPool, admin: &PgPool) {
    use std::str::FromStr;
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let branch = seed_branch(admin, tenant).await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let (_scope, mut tx) = tenancy::begin(app, &c_owner).await.unwrap();
    let def_loc = crate::locations::default_location_id_tx(&mut tx, branch)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let sup = crate::procurement::create_supplier(
        app,
        &c_owner,
        crate::procurement::CreateSupplierRequest {
            name: "Proveedor F2 Compras".into(),
            tax_id: Some("900123456".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CREDITO".into()),
            credit_days: Some(30),
            delivery_days: Some(3),
            category: Some("MERCANCIA".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let p_iva = seed_product(admin, tenant, "Prod IVA19", "UND", Some("IVA19"), false).await;
    let p_inc = seed_product(admin, tenant, "Prod INC8", "UND", Some("INC8"), false).await;
    let p_gift = seed_product(admin, tenant, "Prod Obsequio", "UND", None, false).await;

    // 1. [P] Con iva_responsible = true, el IVA no entra al costo; con false, sí. El INC entra siempre.
    sqlx::query("update tenants set iva_responsible = true where id = $1")
        .bind(tenant)
        .execute(admin)
        .await
        .unwrap();

    let p_order_id = Uuid::new_v4();
    let req = crate::procurement::CreatePurchaseRequest {
        id: p_order_id,
        branch_id: branch,
        supplier_id: sup.id,
        location_id: Some(def_loc),
        expected_at: None,
        document_number: Some("FAC-F2-01".into()),
        supplier_invoice_date: None,
        notes: None,
        discount_amount: Some(dec(150)),
        freight_amount: Some(dec(60)),
        freight_mode: Some("PRORRATEAR".into()),
        payment_terms: None,
        credit_days: None,
        iva_to_cost: None,
        items: vec![
            crate::procurement::PurchaseItemRequest {
                product_id: p_iva,
                qty: dec(10),
                unit_cost: dec(100),
                new_sale_price: Some(dec(150)),
            },
            crate::procurement::PurchaseItemRequest {
                product_id: p_inc,
                qty: dec(5),
                unit_cost: dec(200),
                new_sale_price: Some(dec(300)),
            },
            crate::procurement::PurchaseItemRequest {
                product_id: p_gift,
                qty: dec(2),
                unit_cost: Decimal::ZERO,
                new_sale_price: None,
            },
        ],
    };

    let created = crate::procurement::create_purchase(app, &c_owner, req)
        .await
        .unwrap();
    assert_eq!(created.purchase.status, "PENDIENTE");
    assert_eq!(created.purchase.payment_terms, "CREDITO");
    assert_eq!(created.purchase.credit_days, 30);
    assert!(!created.purchase.iva_to_cost);

    // [P] El reparto del descuento y flete suma exacto y las líneas de obsequio no reciben descuento
    let it_iva = created
        .items
        .iter()
        .find(|i| i.product_id == p_iva)
        .unwrap();
    let it_inc = created
        .items
        .iter()
        .find(|i| i.product_id == p_inc)
        .unwrap();
    let it_gift = created
        .items
        .iter()
        .find(|i| i.product_id == p_gift)
        .unwrap();

    assert_eq!(it_gift.discount_alloc, Decimal::ZERO);
    assert_eq!(
        it_iva.discount_alloc + it_inc.discount_alloc + it_gift.discount_alloc,
        dec(150)
    );
    assert_eq!(
        it_iva.freight_alloc + it_inc.freight_alloc + it_gift.freight_alloc,
        dec(60)
    );

    // IVA19 no entra al costo cuando iva_to_cost es false
    assert_eq!(
        it_iva.landed_unit_cost,
        Some(Decimal::from_str("95.5000").unwrap())
    );
    // INC8 entra siempre al costo
    assert_eq!(
        it_inc.landed_unit_cost,
        Some(Decimal::from_str("205.8000").unwrap())
    );

    // 2. Transición de estado y recepción
    // Intentar recibir estando PENDIENTE vía receive_purchase_with_req da 409 PURCHASE_INVALID_STATE
    let err_rec = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        p_order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: it_iva.id,
                qty: dec(4),
                lot_code: None,
                expires_at: None,
            }],
        },
    )
    .await;
    match err_rec {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "PURCHASE_INVALID_STATE");
        }
        other => panic!(
            "esperaba 409 PURCHASE_INVALID_STATE pero obtuve {:?}",
            other
        ),
    }

    // Aprobar orden
    let approved = crate::procurement::approve_purchase(app, &c_owner, p_order_id)
        .await
        .unwrap();
    assert_eq!(approved.purchase.status, "APROBADA");

    // 3. [P] Recibir más de lo pendiente: 422 RECEIVE_EXCEEDS_PENDING
    let err_exceed = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        p_order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: it_iva.id,
                qty: dec(11),
                lot_code: None,
                expires_at: None,
            }],
        },
    )
    .await;
    match err_exceed {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "RECEIVE_EXCEEDS_PENDING");
        }
        other => panic!(
            "esperaba 422 RECEIVE_EXCEEDS_PENDING pero obtuve {:?}",
            other
        ),
    }

    // 4. [P] Una recepción parcial y luego la total: el estado pasa por PARCIAL y llega a RECIBIDA
    let rec1_id = Uuid::new_v4();
    let partial = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        p_order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: rec1_id,
            items: vec![
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_iva.id,
                    qty: dec(4),
                    lot_code: None,
                    expires_at: None,
                },
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_inc.id,
                    qty: dec(2),
                    lot_code: None,
                    expires_at: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(partial.purchase.status, "PARCIAL");

    // Idempotencia: enviar misma recepción devuelve el detalle sin duplicar
    let idem = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        p_order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: rec1_id,
            items: vec![
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_iva.id,
                    qty: dec(4),
                    lot_code: None,
                    expires_at: None,
                },
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_inc.id,
                    qty: dec(2),
                    lot_code: None,
                    expires_at: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(idem.purchase.status, "PARCIAL");

    let stock_iva = crate::inventory::available(app, &c_owner, branch, p_iva)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_iva.normalize(), dec(4));

    // Segunda recepción con el remanente (6 de p_iva, 3 de p_inc, 2 de p_gift)
    let rec2_id = Uuid::new_v4();
    let total_rec = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        p_order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: rec2_id,
            items: vec![
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_iva.id,
                    qty: dec(6),
                    lot_code: None,
                    expires_at: None,
                },
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_inc.id,
                    qty: dec(3),
                    lot_code: None,
                    expires_at: None,
                },
                crate::procurement::ReceiveItemInput {
                    purchase_item_id: it_gift.id,
                    qty: dec(2),
                    lot_code: None,
                    expires_at: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(total_rec.purchase.status, "RECIBIDA");

    let stock_iva_full = crate::inventory::available(app, &c_owner, branch, p_iva)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_iva_full.normalize(), dec(10));
    let stock_inc_full = crate::inventory::available(app, &c_owner, branch, p_inc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_inc_full.normalize(), dec(5));
    let stock_gift_full = crate::inventory::available(app, &c_owner, branch, p_gift)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stock_gift_full.normalize(), dec(2));

    let price_iva: Option<Decimal> = sqlx::query_scalar(
        "select price from branch_products where branch_id = $1 and product_id = $2",
    )
    .bind(branch)
    .bind(p_iva)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(price_iva.map(|p| p.normalize()), Some(dec(150)));

    // 5. Test con iva_responsible = false en otro tenant: el IVA sí entra al costo
    let tenant_no_iva = seed_tenant(admin).await;
    let owner_no_iva = seed_user(admin).await;
    seed_membership(admin, tenant_no_iva, None, owner_no_iva, "DUENO").await;
    let branch_no_iva = seed_branch(admin, tenant_no_iva).await;
    let c_no_iva = claims(owner_no_iva, Some(tenant_no_iva), Some(branch_no_iva));

    sqlx::query("update tenants set iva_responsible = false where id = $1")
        .bind(tenant_no_iva)
        .execute(admin)
        .await
        .unwrap();

    let sup_no_iva = crate::procurement::create_supplier(
        app,
        &c_no_iva,
        crate::procurement::CreateSupplierRequest {
            name: "Sup No IVA".into(),
            tax_id: Some("800197268".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let p_iva_2 = seed_product(
        admin,
        tenant_no_iva,
        "Prod IVA 2",
        "UND",
        Some("IVA19"),
        false,
    )
    .await;
    let p_order_2 = Uuid::new_v4();
    let created_no_iva = crate::procurement::create_purchase(
        app,
        &c_no_iva,
        crate::procurement::CreatePurchaseRequest {
            id: p_order_2,
            branch_id: branch_no_iva,
            supplier_id: sup_no_iva.id,
            location_id: None,
            expected_at: None,
            document_number: Some("FAC-NO-IVA".into()),
            supplier_invoice_date: None,
            notes: None,
            discount_amount: Some(Decimal::ZERO),
            freight_amount: Some(Decimal::ZERO),
            freight_mode: Some("PRORRATEAR".into()),
            payment_terms: None,
            credit_days: None,
            iva_to_cost: None,
            items: vec![crate::procurement::PurchaseItemRequest {
                product_id: p_iva_2,
                qty: dec(10),
                unit_cost: dec(100),
                new_sale_price: None,
            }],
        },
    )
    .await
    .unwrap();

    assert!(created_no_iva.purchase.iva_to_cost);
    assert_eq!(
        created_no_iva.items[0].landed_unit_cost,
        Some(Decimal::from_str("119.0000").unwrap())
    );

    cleanup(admin, tenant, &[owner]).await;
    cleanup(admin, tenant_no_iva, &[owner_no_iva]).await;
}

/// F2.13 Cuentas por pagar y aging
async fn f2_payables(app: &PgPool, admin: &PgPool) {
    use crate::payables;
    use chrono::Duration;
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let cashier = seed_user(admin).await;
    let branch = seed_branch(admin, tenant).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let (_scope, mut tx) = tenancy::begin(app, &c_owner).await.unwrap();
    let loc_id = crate::locations::default_location_id_tx(&mut tx, branch)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let reg_id = default_register(admin, branch).await;

    // Crear proveedor y producto
    let supp = procurement::create_supplier(
        app,
        &c_owner,
        procurement::CreateSupplierRequest {
            name: "Proveedor CxP Test".into(),
            tax_id: Some("900999888".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            delivery_days: None,
            category: Some("MERCANCIA".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let p1 = seed_product(admin, tenant, "Prod CxP 1", "UND", None, false).await;

    // [P] Dos recepciones parciales hacen crecer una sola cuenta
    let po_id = Uuid::new_v4();
    let po = procurement::create_purchase(
        app,
        &c_owner,
        procurement::CreatePurchaseRequest {
            id: po_id,
            branch_id: branch,
            supplier_id: supp.id,
            location_id: Some(loc_id),
            document_number: Some("FAC-CXP-001".into()),
            expected_at: None,
            supplier_invoice_date: None,
            notes: None,
            discount_amount: Some(Decimal::ZERO),
            freight_amount: Some(Decimal::ZERO),
            freight_mode: Some("PRORRATEAR".into()),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            iva_to_cost: Some(false),
            items: vec![procurement::PurchaseItemRequest {
                product_id: p1,
                qty: dec(10),
                unit_cost: dec(100),
                new_sale_price: None,
            }],
        },
    )
    .await
    .unwrap();

    procurement::approve_purchase(app, &c_owner, po.purchase.id)
        .await
        .unwrap();

    // 1ra recepción parcial: 4 unidades
    let r1_id = Uuid::new_v4();
    procurement::receive_purchase_with_req(
        app,
        &c_owner,
        po.purchase.id,
        procurement::ReceivePurchaseRequest {
            id: r1_id,
            items: vec![procurement::ReceiveItemInput {
                purchase_item_id: po.items[0].id,
                qty: dec(4),
                lot_code: None,
                expires_at: None,
            }],
        },
    )
    .await
    .unwrap();

    // Verificar que existe exactamente 1 cuenta por pagar con amount_total = 400
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let payables_1 = payables::list_payables_tx(
        &mut tx,
        &scope_owner,
        payables::PayablesFilter {
            branch_id: Some(branch),
            status: None,
            supplier_id: Some(supp.id),
            overdue: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(payables_1.len(), 1);
    assert_eq!(payables_1[0].amount_total, dec(400));
    assert_eq!(payables_1[0].amount_paid, dec(0));
    assert_eq!(payables_1[0].balance, dec(400));
    assert_eq!(payables_1[0].status, "ABIERTA");
    let payable_id = payables_1[0].id;

    // 2da recepción parcial: 6 unidades restantes
    let r2_id = Uuid::new_v4();
    procurement::receive_purchase_with_req(
        app,
        &c_owner,
        po.purchase.id,
        procurement::ReceivePurchaseRequest {
            id: r2_id,
            items: vec![procurement::ReceiveItemInput {
                purchase_item_id: po.items[0].id,
                qty: dec(6),
                lot_code: None,
                expires_at: None,
            }],
        },
    )
    .await
    .unwrap();

    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let payables_2 = payables::list_payables_tx(
        &mut tx,
        &scope_owner,
        payables::PayablesFilter {
            branch_id: Some(branch),
            status: None,
            supplier_id: Some(supp.id),
            overdue: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Sigue siendo 1 sola cuenta pero ahora amount_total = 1000
    assert_eq!(payables_2.len(), 1);
    assert_eq!(payables_2[0].id, payable_id);
    assert_eq!(payables_2[0].amount_total, dec(1000));
    assert_eq!(payables_2[0].amount_paid, dec(0));
    assert_eq!(payables_2[0].balance, dec(1000));

    // [P] Un pago mayor al saldo: 422
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let pay_err = payables::create_payment_tx(
        &mut tx,
        &scope_owner,
        payable_id,
        payables::CreatePayablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(1200),
            method: "TRANSFERENCIA".into(),
            reference: Some("REF-123456".into()),
        },
    )
    .await
    .unwrap_err();
    tx.rollback().await.unwrap();

    match pay_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "PAYMENT_EXCEEDS_BALANCE");
        }
        other => panic!(
            "esperaba 422 PAYMENT_EXCEEDS_BALANCE pero obtuve {:?}",
            other
        ),
    }

    // [P] Un pago en efectivo crea un movimiento PAGO_PROVEEDOR y baja el esperado del turno
    // Primero, si intentamos pagar en efectivo sin turno abierto: 409 PAYMENT_NEEDS_OPEN_SESSION
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let no_session_err = payables::create_payment_tx(
        &mut tx,
        &scope_owner,
        payable_id,
        payables::CreatePayablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(300),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await
    .unwrap_err();
    tx.rollback().await.unwrap();
    match no_session_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "PAYMENT_NEEDS_OPEN_SESSION");
        }
        other => panic!(
            "esperaba 409 PAYMENT_NEEDS_OPEN_SESSION pero obtuve {:?}",
            other
        ),
    }

    // Abrimos turno de caja con 500.000
    let session = cash::open(app, &c_owner, branch, reg_id, dec(500000))
        .await
        .unwrap();

    // Pagamos 300 en efectivo
    let pay1_id = Uuid::new_v4();
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let paid_detail = payables::create_payment_tx(
        &mut tx,
        &scope_owner,
        payable_id,
        payables::CreatePayablePaymentRequest {
            id: pay1_id,
            amount: dec(300),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(paid_detail.payable.amount_paid, dec(300));
    assert_eq!(paid_detail.payable.balance, dec(700));
    assert_eq!(paid_detail.payable.status, "PARCIAL");
    assert_eq!(paid_detail.payments.len(), 1);
    assert_eq!(paid_detail.payments[0].method, "EFECTIVO");
    assert_eq!(paid_detail.payments[0].amount, dec(300));

    // Verificar que el esperado del turno bajó de 500.000 a 499.700
    let (_scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let cash_summary = cash::summary_tx(&mut tx, session.id).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(cash_summary.expected_cash, dec(499700));

    // [P] El vencimiento usa los días de crédito del proveedor
    let supp_cred = procurement::create_supplier(
        app,
        &c_owner,
        procurement::CreateSupplierRequest {
            name: "Proveedor Credito 30d".into(),
            tax_id: Some("900111222".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            self_withholding: Some(false),
            payment_terms: Some("CREDITO".into()),
            credit_days: Some(30),
            delivery_days: None,
            category: Some("MERCANCIA".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let po_cred_id = Uuid::new_v4();
    let po_cred = procurement::create_purchase(
        app,
        &c_owner,
        procurement::CreatePurchaseRequest {
            id: po_cred_id,
            branch_id: branch,
            supplier_id: supp_cred.id,
            location_id: Some(loc_id),
            document_number: Some("FAC-CRED-001".into()),
            expected_at: None,
            supplier_invoice_date: None,
            notes: None,
            discount_amount: Some(Decimal::ZERO),
            freight_amount: Some(Decimal::ZERO),
            freight_mode: Some("PRORRATEAR".into()),
            payment_terms: Some("CREDITO".into()),
            credit_days: Some(30),
            iva_to_cost: Some(false),
            items: vec![procurement::PurchaseItemRequest {
                product_id: p1,
                qty: dec(5),
                unit_cost: dec(200),
                new_sale_price: None,
            }],
        },
    )
    .await
    .unwrap();

    procurement::approve_purchase(app, &c_owner, po_cred_id)
        .await
        .unwrap();

    let r_cred_id = Uuid::new_v4();
    procurement::receive_purchase_with_req(
        app,
        &c_owner,
        po_cred_id,
        procurement::ReceivePurchaseRequest {
            id: r_cred_id,
            items: vec![procurement::ReceiveItemInput {
                purchase_item_id: po_cred.items[0].id,
                qty: dec(5),
                lot_code: None,
                expires_at: None,
            }],
        },
    )
    .await
    .unwrap();

    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let pay_cred_row_id = sqlx::query_scalar::<_, Uuid>(
        "select id from payables where source_type = 'COMPRA' and source_id = $1",
    )
    .bind(po_cred_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let pay_cred = payables::get_payable_detail_tx(&mut tx, &scope_owner, pay_cred_row_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let expected_due = chrono::Utc::now().date_naive() + Duration::days(30);
    assert_eq!(pay_cred.payable.due_date, expected_due);
    assert_eq!(pay_cred.payable.amount_total, dec(1000));
    assert_eq!(pay_cred.payable.status, "ABIERTA");

    // [P] Los tramos de antigüedad suman el saldo total
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let aging = payables::aging_summary_tx(&mut tx, &scope_owner, Some(branch))
        .await
        .unwrap();
    let all_open = payables::list_payables_tx(
        &mut tx,
        &scope_owner,
        payables::PayablesFilter {
            branch_id: Some(branch),
            status: None,
            supplier_id: None,
            overdue: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let total_open_balance: Decimal = all_open
        .iter()
        .filter(|p| p.status == "ABIERTA" || p.status == "PARCIAL")
        .map(|p| p.balance)
        .sum();

    let aging_sum =
        aging.al_dia + aging.d1_30 + aging.d31_60 + aging.d61_90 + aging.d91_120 + aging.d120_mas;
    assert_eq!(aging_sum, total_open_balance);

    // Test adicional: Castigar cuenta (write-off)
    // Cajero intenta castigar -> 403
    let (scope_cashier, mut tx) = tenancy::begin_in_branch(app, &c_cashier, branch)
        .await
        .unwrap();
    let cashier_err = payables::write_off_tx(
        &mut tx,
        &scope_cashier,
        pay_cred.payable.id,
        payables::WriteOffPayableRequest {
            reason: "Empresa en liquidación".into(),
        },
    )
    .await
    .unwrap_err();
    tx.rollback().await.unwrap();
    assert!(matches!(
        cashier_err,
        ApiError::Domain(crate::error::DomainError::Forbidden)
    ));

    // Dueño castiga la cuenta
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let written_off = payables::write_off_tx(
        &mut tx,
        &scope_owner,
        pay_cred.payable.id,
        payables::WriteOffPayableRequest {
            reason: "Empresa en liquidación judicial".into(),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(written_off.payable.status, "CASTIGADA");
    assert_eq!(
        written_off.payable.written_off_reason,
        Some("Empresa en liquidación judicial".into())
    );

    // Intentar pagar cuenta castigada -> 409 PAYABLE_CLOSED
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let closed_err = payables::create_payment_tx(
        &mut tx,
        &scope_owner,
        pay_cred.payable.id,
        payables::CreatePayablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(100),
            method: "TRANSFERENCIA".into(),
            reference: Some("REF-CLOSED".into()),
        },
    )
    .await
    .unwrap_err();
    tx.rollback().await.unwrap();
    match closed_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "PAYABLE_CLOSED");
        }
        other => panic!("esperaba 409 PAYABLE_CLOSED pero obtuve {:?}", other),
    }

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn f2_expenses(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let reg = default_register(admin, branch).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));

    // Turno de caja abierto para el dueño con $500,000 en efectivo
    let session = cash::open(app, &c_owner, branch, reg, dec(500000))
        .await
        .unwrap();

    // Obtener categoría activa "Arriendo" o crear una
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let categories = expenses::list_categories_tx(&mut tx, &scope_owner, false)
        .await
        .unwrap();
    let cat_id = categories
        .iter()
        .find(|c| c.name.to_lowercase() == "arriendo")
        .map(|c| c.id)
        .unwrap_or_else(|| {
            categories
                .first()
                .map(|c| c.id)
                .expect("at least one category")
        });
    tx.commit().await.unwrap();

    // 1. Crear gasto
    let exp_id = Uuid::new_v4();
    let today = Utc::now().date_naive();
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let exp_created = expenses::create_expense_tx(
        &mut tx,
        &scope_owner,
        expenses::CreateExpenseRequest {
            id: exp_id,
            branch_id: branch,
            category_id: cat_id,
            supplier_id: None,
            description: "Pago arriendo local noviembre".into(),
            amount: dec(150000),
            tax_amount: Some(dec(0)),
            expense_date: today,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(exp_created.status, "PENDIENTE");
    assert_eq!(exp_created.amount, dec(150000));
    assert_eq!(exp_created.total, dec(150000));
    assert!(exp_created.document_number.starts_with("GAS-"));

    // [P] Aprobar crea una cuenta por pagar
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let exp_approved = expenses::approve_expense_tx(&mut tx, &scope_owner, exp_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(exp_approved.status, "APROBADO");
    assert!(exp_approved.payable_id.is_some());

    // Verificar que la cuenta por pagar existe y tiene los datos esperados
    let pay_id = exp_approved.payable_id.unwrap();
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let payable_detail = payables::get_payable_detail_tx(&mut tx, &scope_owner, pay_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(payable_detail.payable.source_type, "GASTO");
    assert_eq!(payable_detail.payable.source_id, exp_id);
    assert_eq!(payable_detail.payable.amount_total, dec(150000));
    assert_eq!(payable_detail.payable.amount_paid, dec(0));
    assert_eq!(payable_detail.payable.balance, dec(150000));
    assert_eq!(payable_detail.payable.status, "ABIERTA");

    // [P] Pagar en efectivo crea un movimiento GASTO en la caja y deja el gasto PAGADO
    let pay_payment_id = Uuid::new_v4();
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let pay_res = payables::create_payment_tx(
        &mut tx,
        &scope_owner,
        pay_id,
        payables::CreatePayablePaymentRequest {
            id: pay_payment_id,
            amount: dec(150000),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(pay_res.payable.status, "PAGADA");
    assert_eq!(pay_res.payable.amount_paid, dec(150000));
    assert_eq!(pay_res.payable.balance, dec(0));

    // El gasto pasa automáticamente a PAGADO
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let exp_paid = expenses::get_expense_detail_tx(&mut tx, &scope_owner, exp_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(exp_paid.status, "PAGADO");

    // Movimiento en caja con kind = GASTO
    let (_scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    let cash_summary = cash::summary_tx(&mut tx, session.id).await.unwrap();
    tx.commit().await.unwrap();

    // 500,000 inicial - 150,000 gasto = 350,000 esperado
    assert_eq!(cash_summary.expected_cash, dec(350000));

    // Verificar en la BD que el movimiento tiene kind = GASTO
    let mov_kind: String = sqlx::query_scalar(
        r#"
        select tm.tima_codigo
          from cash_movements cm
          join tbl_tipos_maestra tm on tm.tima_id = cm.kind_id
         where cm.cash_session_id = $1 and cm.ref_id = $2
        "#,
    )
    .bind(session.id)
    .bind(pay_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_kind, "GASTO");

    // [P] Aprobar un gasto rechazado: 409 EXPENSE_INVALID_STATE
    let exp2_id = Uuid::new_v4();
    let (scope_owner, mut tx) = tenancy::begin_in_branch(app, &c_owner, branch)
        .await
        .unwrap();
    expenses::create_expense_tx(
        &mut tx,
        &scope_owner,
        expenses::CreateExpenseRequest {
            id: exp2_id,
            branch_id: branch,
            category_id: cat_id,
            supplier_id: None,
            description: "Gasto con presupuesto dudoso".into(),
            amount: dec(80000),
            tax_amount: None,
            expense_date: today,
        },
    )
    .await
    .unwrap();
    expenses::reject_expense_tx(
        &mut tx,
        &scope_owner,
        exp2_id,
        expenses::RejectExpenseRequest {
            reason: "Monto no corresponde a cotización".into(),
        },
    )
    .await
    .unwrap();

    // Intentar aprobar gasto rechazado
    let reject_err = expenses::approve_expense_tx(&mut tx, &scope_owner, exp2_id)
        .await
        .unwrap_err();
    tx.rollback().await.unwrap();

    match reject_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "EXPENSE_INVALID_STATE");
        }
        other => panic!("esperaba 409 EXPENSE_INVALID_STATE pero obtuve {:?}", other),
    }

    cleanup(admin, tenant, &[owner]).await;
}

/// F2.15: Lotes, vencimientos y FEFO.
async fn f2_lots(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let def_loc: Uuid = sqlx::query_scalar(
        "select id from locations where branch_id = $1 and is_default = true limit 1",
    )
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();

    let reg = crate::registers::create(
        app,
        &c_owner,
        crate::registers::CreateRegisterRequest {
            branch_id: branch,
            code: "CAJA-LOTES".into(),
            name: "Caja Lotes".into(),
            default_opening: dec(10000),
            location_id: Some(def_loc),
        },
    )
    .await
    .unwrap();

    let _session = crate::cash::open(app, &c_owner, branch, reg.id, dec(10000))
        .await
        .unwrap();

    let sup = crate::procurement::create_supplier(
        app,
        &c_owner,
        crate::procurement::CreateSupplierRequest {
            name: "Proveedor Lotes".into(),
            tax_id: Some("900111222".into()),
            doc_type: Some("NIT".into()),
            dv: None,
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(false),
            self_withholding: Some(false),
            payment_terms: Some("CONTADO".into()),
            credit_days: Some(0),
            delivery_days: Some(1),
            category: Some("MERCANCIA".into()),
            bank_name: None,
            bank_account_type: None,
            bank_account_number: None,
            contact_name: None,
            phone: None,
            email: None,
            address: None,
            notes: None,
        },
    )
    .await
    .unwrap();

    let prod = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Yogurt con Lote".into(),
            sku: Some("YOG-LOT-01".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(2000)),
            base_price: Some(dec(3500)),
            reorder_qty: None,
            max_qty: None,
            track_lots: true,
            allow_price_override: true,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    let today = chrono::Utc::now().date_naive();

    // 1. [P] Recibir un lote existente con otro vencimiento: 422 LOT_EXPIRY_MISMATCH.
    let order_id = Uuid::new_v4();
    let create_order = crate::procurement::create_purchase(
        app,
        &c_owner,
        crate::procurement::CreatePurchaseRequest {
            id: order_id,
            branch_id: branch,
            supplier_id: sup.id,
            location_id: Some(def_loc),
            expected_at: None,
            document_number: Some("OC-LOT-01".into()),
            supplier_invoice_date: None,
            notes: None,
            discount_amount: None,
            freight_amount: None,
            freight_mode: None,
            payment_terms: None,
            credit_days: None,
            iva_to_cost: None,
            items: vec![
                crate::procurement::PurchaseItemRequest {
                    product_id: prod.row.id,
                    qty: dec(10),
                    unit_cost: dec(2000),
                    new_sale_price: None,
                },
                crate::procurement::PurchaseItemRequest {
                    product_id: prod.row.id,
                    qty: dec(10),
                    unit_cost: dec(2000),
                    new_sale_price: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    crate::procurement::approve_purchase(app, &c_owner, order_id)
        .await
        .unwrap();

    let item_1 = create_order.items[0].id;
    let item_2 = create_order.items[1].id;

    // Recepción 1 con LOTE-MISMATCH y vencimiento hoy + 10 días
    crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: item_1,
                qty: dec(10),
                lot_code: Some("LOTE-MISMATCH".into()),
                expires_at: Some(today + chrono::Duration::days(10)),
            }],
        },
    )
    .await
    .unwrap();

    // Recepción 2 con el MISMO lote pero vencimiento diferente (hoy + 20 días): debe fallar con 422
    let mismatch_err = crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        order_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: item_2,
                qty: dec(10),
                lot_code: Some("LOTE-MISMATCH".into()),
                expires_at: Some(today + chrono::Duration::days(20)),
            }],
        },
    )
    .await
    .unwrap_err();

    match mismatch_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "LOT_EXPIRY_MISMATCH");
        }
        other => panic!("esperaba 422 LOT_EXPIRY_MISMATCH pero obtuve {:?}", other),
    }

    // 2. [P] Una venta toma primero el lote que vence antes (FEFO).
    // Creamos dos lotes para el producto:
    // LOTE-LATER (vence en 30 días, qty 5)
    // LOTE-SOONER (vence en 5 días, qty 5)
    let order2_id = Uuid::new_v4();
    let create_order2 = crate::procurement::create_purchase(
        app,
        &c_owner,
        crate::procurement::CreatePurchaseRequest {
            id: order2_id,
            branch_id: branch,
            supplier_id: sup.id,
            location_id: Some(def_loc),
            expected_at: None,
            document_number: Some("OC-LOT-02".into()),
            supplier_invoice_date: None,
            notes: None,
            discount_amount: None,
            freight_amount: None,
            freight_mode: None,
            payment_terms: None,
            credit_days: None,
            iva_to_cost: None,
            items: vec![crate::procurement::PurchaseItemRequest {
                product_id: prod.row.id,
                qty: dec(10),
                unit_cost: dec(2000),
                new_sale_price: None,
            }],
        },
    )
    .await
    .unwrap();

    crate::procurement::approve_purchase(app, &c_owner, order2_id)
        .await
        .unwrap();

    let p2_item = create_order2.items[0].id;

    // Recibir 5 con LOTE-LATER (30 días)
    crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        order2_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: p2_item,
                qty: dec(5),
                lot_code: Some("LOTE-LATER".into()),
                expires_at: Some(today + chrono::Duration::days(30)),
            }],
        },
    )
    .await
    .unwrap();

    // Recibir 5 con LOTE-SOONER (5 días)
    crate::procurement::receive_purchase_with_req(
        app,
        &c_owner,
        order2_id,
        crate::procurement::ReceivePurchaseRequest {
            id: Uuid::new_v4(),
            items: vec![crate::procurement::ReceiveItemInput {
                purchase_item_id: p2_item,
                qty: dec(5),
                lot_code: Some("LOTE-SOONER".into()),
                expires_at: Some(today + chrono::Duration::days(5)),
            }],
        },
    )
    .await
    .unwrap();

    // Vender 3 unidades: debe consumirse de LOTE-SOONER
    let sale_req = sale(
        Uuid::new_v4(),
        branch,
        &[(prod.row.id, 3)],
        &[(crate::sales::PaymentMethod::Efectivo, 10500)],
        dec(10500),
    );
    let s_resp = created(
        crate::sales::checkout(app, &c_owner, sale_req)
            .await
            .unwrap(),
    );

    // Verificar existencias de los lotes en la base de datos
    let qty_sooner: Decimal = sqlx::query_scalar(
        "select qty from lots where location_id = $1 and product_id = $2 and code = 'LOTE-SOONER'",
    )
    .bind(def_loc)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();

    let qty_later: Decimal = sqlx::query_scalar(
        "select qty from lots where location_id = $1 and product_id = $2 and code = 'LOTE-LATER'",
    )
    .bind(def_loc)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();

    assert_eq!(qty_sooner, dec(2), "LOTE-SOONER debió bajar de 5 a 2");
    assert_eq!(
        qty_later,
        dec(5),
        "LOTE-LATER debió permanecer intacto en 5"
    );

    // Verificar en order_item_lots
    let lot_sooner_id: Uuid = sqlx::query_scalar(
        "select id from lots where location_id = $1 and product_id = $2 and code = 'LOTE-SOONER'",
    )
    .bind(def_loc)
    .bind(prod.row.id)
    .fetch_one(admin)
    .await
    .unwrap();

    let oil_qty: Decimal = sqlx::query_scalar(
        "select oil.qty from order_item_lots oil
           join order_items oi on oi.id = oil.order_item_id
          where oi.order_id = $1 and oil.lot_id = $2",
    )
    .bind(s_resp.order.id)
    .bind(lot_sooner_id)
    .fetch_one(admin)
    .await
    .unwrap();

    assert_eq!(
        oil_qty,
        dec(3),
        "order_item_lots debió registrar 3 unidades consumidas de LOTE-SOONER"
    );

    // 3. [P] Con solo lotes vencidos: 409 LOT_ONLY_EXPIRED.
    let prod_expired = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Queso Vencido".into(),
            sku: Some("QSO-VENCIDO".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(5000)),
            base_price: Some(dec(8000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: true,
            allow_price_override: true,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // Insertar lote ya vencido con cantidad 10 y stock en 10
    sqlx::query(
        r#"
        insert into lots (tenant_id, branch_id, location_id, product_id, code, expires_at, qty)
        values ($1, $2, $3, $4, 'LOTE-VENCIDO', $5, 10)
        "#,
    )
    .bind(tenant)
    .bind(branch)
    .bind(def_loc)
    .bind(prod_expired.row.id)
    .bind(today - chrono::Duration::days(3))
    .execute(admin)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved, min_qty, avg_cost)
        values ($1, $2, $3, $4, 10, 0, 0, 5000)
        on conflict (location_id, product_id) do update set on_hand = 10
        "#,
    )
    .bind(tenant)
    .bind(branch)
    .bind(def_loc)
    .bind(prod_expired.row.id)
    .execute(admin)
    .await
    .unwrap();

    // Intentar vender 1 unidad de producto con solo lotes vencidos
    let sale_expired_req = sale(
        Uuid::new_v4(),
        branch,
        &[(prod_expired.row.id, 1)],
        &[(crate::sales::PaymentMethod::Efectivo, 8000)],
        dec(8000),
    );
    let expired_err = crate::sales::checkout(app, &c_owner, sale_expired_req)
        .await
        .unwrap_err();

    match expired_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "LOT_ONLY_EXPIRED");
        }
        other => panic!("esperaba 409 LOT_ONLY_EXPIRED pero obtuve {:?}", other),
    }

    // Comprobar consulta de expiring lots
    let expiring = inventory::list_expiring_lots(app, &c_owner, Some(branch), Some(30))
        .await
        .unwrap();
    assert!(
        !expiring.is_empty(),
        "debe retornar lotes próximos a vencer o vencidos"
    );
    assert!(
        expiring
            .iter()
            .any(|l| l.code == "LOTE-VENCIDO" && l.is_expired)
    );
    assert!(
        expiring
            .iter()
            .any(|l| l.code == "LOTE-SOONER" && !l.is_expired)
    );

    cleanup(admin, tenant, &[owner]).await;
}

async fn f2_adjustments(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let def_loc = crate::locations::default_location_id(admin, branch)
        .await
        .unwrap();

    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let supervisor = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), supervisor, "SUPERVISOR").await;
    let c_super = claims(supervisor, Some(tenant), Some(branch));

    // Producto de prueba con costo unitario 100,000
    let prod = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Cámara Pro".into(),
            sku: Some("CAM-PRO-1".into()),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: None,
            purchase_factor: dec(1),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(100_000)),
            base_price: Some(dec(150_000)),
            reorder_qty: None,
            max_qty: None,
            track_lots: false,
            allow_price_override: true,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // Establecer stock inicial en 5 unidades a 100,000
    sqlx::query(
        r#"
        insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved, min_qty, avg_cost)
        values ($1, $2, $3, $4, 5, 0, 0, 100000)
        on conflict (location_id, product_id) do update set on_hand = 5, avg_cost = 100000
        "#,
    )
    .bind(tenant)
    .bind(branch)
    .bind(def_loc)
    .bind(prod.row.id)
    .execute(admin)
    .await
    .unwrap();

    // 1. [P] Un ajuste por encima del umbral (200,000) pedido por un usuario sin permiso de aprobación (SUPERVISOR)
    // queda PENDIENTE y el stock no cambia; al aprobarlo, cambia.
    // -3 unidades a 100,000 = valor 300,000 > umbral (200,000).
    let adj_id = Uuid::new_v4();
    let adj_req = inventory::CreateAdjustmentRequest {
        id: adj_id,
        branch_id: branch,
        location_id: Some(def_loc),
        product_id: prod.row.id,
        lot_id: None,
        qty_delta: dec(-3),
        reason_code: "PERDIDA".into(),
        notes: Some("Pérdida en almacén".into()),
        unit_cost: None,
    };

    let adj_res = inventory::create_adjustment(app, &c_super, adj_req)
        .await
        .unwrap();
    assert_eq!(
        adj_res.status, "PENDIENTE",
        "Ajuste > umbral por supervisor debe quedar PENDIENTE"
    );
    assert_eq!(adj_res.value, dec(300_000));

    // Verificar que el stock NO ha cambiado (sigue en 5)
    let stock_on_hand: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(def_loc)
            .bind(prod.row.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        stock_on_hand,
        dec(5),
        "El stock no debe cambiar mientras esté PENDIENTE"
    );

    // Supervisor intenta aprobarlo y debe recibir 403 Forbidden
    let super_approve_err = inventory::approve_adjustment(app, &c_super, adj_id)
        .await
        .unwrap_err();
    match super_approve_err {
        ApiError::Domain(DomainError::Forbidden) => {}
        other => panic!(
            "esperaba 403 Forbidden para supervisor aprobando pero obtuve {:?}",
            other
        ),
    }

    // Owner (con permiso inventory:adjust_approve) lo aprueba
    let approved_adj = inventory::approve_adjustment(app, &c_owner, adj_id)
        .await
        .unwrap();
    assert_eq!(approved_adj.status, "APLICADO");
    assert_eq!(approved_adj.decided_by, Some(owner));

    // Ahora el stock debe haber cambiado a 2 (5 - 3)
    let stock_on_hand_after: Decimal =
        sqlx::query_scalar("select on_hand from stock where location_id = $1 and product_id = $2")
            .bind(def_loc)
            .bind(prod.row.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(
        stock_on_hand_after,
        dec(2),
        "El stock debió actualizarse a 2 tras aprobarse el ajuste"
    );

    // 2. [P] CARGA_INICIAL con stock existente: 422 ADJUST_INVALID.
    let initial_req = inventory::CreateAdjustmentRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        location_id: Some(def_loc),
        product_id: prod.row.id,
        lot_id: None,
        qty_delta: dec(10),
        reason_code: "CARGA_INICIAL".into(),
        notes: Some("Intento inválido de carga inicial".into()),
        unit_cost: Some(dec(100_000)),
    };
    let initial_err = inventory::create_adjustment(app, &c_owner, initial_req)
        .await
        .unwrap_err();
    match initial_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "ADJUST_INVALID");
        }
        other => panic!(
            "esperaba 422 ADJUST_INVALID para CARGA_INICIAL con stock pero obtuve {:?}",
            other
        ),
    }

    // 3. [P] ROBO con una cantidad positiva: 422 ADJUST_INVALID.
    let robo_req = inventory::CreateAdjustmentRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        location_id: Some(def_loc),
        product_id: prod.row.id,
        lot_id: None,
        qty_delta: dec(5),
        reason_code: "ROBO".into(),
        notes: Some("Robo positivo inválido".into()),
        unit_cost: None,
    };
    let robo_err = inventory::create_adjustment(app, &c_owner, robo_req)
        .await
        .unwrap_err();
    match robo_err {
        ApiError::Domain(DomainError::Coded { code, status, .. }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "ADJUST_INVALID");
        }
        other => panic!(
            "esperaba 422 ADJUST_INVALID para ROBO con cantidad positiva pero obtuve {:?}",
            other
        ),
    }

    // 4. Rechazar un ajuste pendiente
    let reject_adj_id = Uuid::new_v4();
    let reject_req = inventory::CreateAdjustmentRequest {
        id: reject_adj_id,
        branch_id: branch,
        location_id: Some(def_loc),
        product_id: prod.row.id,
        lot_id: None,
        qty_delta: dec(-4),
        reason_code: "PERDIDA".into(),
        notes: Some("Ajuste a rechazar".into()),
        unit_cost: None,
    };
    inventory::create_adjustment(app, &c_super, reject_req)
        .await
        .unwrap();
    let rejected =
        inventory::reject_adjustment(app, &c_owner, reject_adj_id, "Rechazado por inconsistencia")
            .await
            .unwrap();
    assert_eq!(rejected.status, "RECHAZADO");
    assert_eq!(
        rejected.reject_reason.as_deref(),
        Some("Rechazado por inconsistencia")
    );

    cleanup(admin, tenant, &[owner, supervisor]).await;
}

async fn f2_suggestions(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let def_loc = crate::locations::default_location_id(admin, branch)
        .await
        .unwrap();

    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), owner, "DUENO").await;
    let c_owner = claims(owner, Some(tenant), Some(branch));

    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    // 1. Crear un proveedor
    let sup = procurement::create_supplier(
        app,
        &c_owner,
        procurement::CreateSupplierRequest {
            name: "Distribuidora Central Sugerencias SAS".into(),
            tax_id: Some("900123456".into()),
            doc_type: Some("NIT".into()),
            person_type: Some("JURIDICA".into()),
            iva_responsible: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 2. Crear producto con reorder_qty = 20, purchase_factor = 6, UOM de compra CAJA
    let prod = products::create_product(
        app,
        &c_owner,
        products::ProductInput {
            name: "Arroz Extra 500g Sugerencias".into(),
            sku: Some(format!("SUGG-{}", Uuid::new_v4().simple())),
            barcode: None,
            kind: "FISICO".into(),
            usage: "VENTA".into(),
            unit_measure_code: "UND".into(),
            purchase_uom_code: Some("CAJA".into()),
            purchase_factor: dec(6),
            tax_code: "EXENTO".into(),
            cost_price: Some(dec(2000)),
            base_price: Some(dec(3000)),
            reorder_qty: Some(dec(20)),
            max_qty: None,
            track_lots: false,
            allow_price_override: false,
            has_variants: false,
            description: None,
        },
    )
    .await
    .unwrap();

    // 3. Vincular producto al proveedor como preferido, con min_order_qty = 2 y last_cost = 12000
    procurement::set_supplier_products(
        app,
        &c_owner,
        sup.id,
        vec![procurement::SupplierProductItemInput {
            product_id: prod.row.id,
            supplier_code: Some("SUP-ARR-01".into()),
            min_order_qty: Some(dec(2)),
            preferred: true,
        }],
    )
    .await
    .unwrap();

    sqlx::query(
        "update supplier_products set last_cost = $1 where supplier_id = $2 and product_id = $3",
    )
    .bind(dec(12000))
    .bind(sup.id)
    .bind(prod.row.id)
    .execute(admin)
    .await
    .unwrap();

    // 4. Sembrar stock por debajo del mínimo: on_hand = 3, min_qty = 5
    sqlx::query(
        r#"
        insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved, min_qty, avg_cost)
        values ($1, $2, $3, $4, 3, 0, 5, 2000)
        on conflict (location_id, product_id) do update set on_hand = 3, min_qty = 5, avg_cost = 2000
        "#,
    )
    .bind(tenant)
    .bind(branch)
    .bind(def_loc)
    .bind(prod.row.id)
    .execute(admin)
    .await
    .unwrap();

    // 5. Cajero sin purchases:create intenta consultar sugerencias: 403 Forbidden
    let cashier_err = procurement::get_purchase_suggestions(app, &c_cashier, branch)
        .await
        .unwrap_err();
    match cashier_err {
        ApiError::Domain(DomainError::Forbidden) => {}
        other => panic!(
            "esperaba 403 Forbidden para cajero consultando sugerencias pero obtuve {:?}",
            other
        ),
    }

    // 6. [P] Dueño consulta sugerencias: un producto bajo el mínimo con un preferido
    // sugiere reorder_qty convertida a unidad de compra: ceil(20 / 6) = 4 >= min_order_qty 2
    let suggestions = procurement::get_purchase_suggestions(app, &c_owner, branch)
        .await
        .unwrap();

    assert_eq!(suggestions.len(), 1, "Debe haber 1 grupo de proveedor");
    let group = &suggestions[0];
    assert_eq!(
        group.supplier.as_ref().map(|s| s.id),
        Some(sup.id),
        "El proveedor debe ser el preferido"
    );
    assert_eq!(group.items.len(), 1, "Debe haber 1 item sugerido");
    let item = &group.items[0];
    assert_eq!(item.product_id, prod.row.id);
    assert_eq!(item.on_hand, dec(3));
    assert_eq!(item.min_qty, dec(5));
    assert_eq!(
        item.suggested_qty,
        dec(4),
        "suggested_qty en unidad de compra debe ser ceil(20/6) = 4"
    );
    assert_eq!(item.last_cost, Some(dec(12000)));
    assert_eq!(item.purchase_uom.as_deref(), Some("CAJA"));

    // 7. Crear orden de compra desde las sugerencias
    let created_order_ids = procurement::create_purchases_from_suggestions(
        app,
        &c_owner,
        procurement::CreateFromSuggestionsRequest {
            branch_id: branch,
            supplier_ids: vec![sup.id],
        },
    )
    .await
    .unwrap();

    assert_eq!(created_order_ids.len(), 1);
    let order_id = created_order_ids[0];

    // Verificar orden creada en estado PENDIENTE con los datos sugeridos
    let order_detail = procurement::get_purchase(app, &c_owner, order_id)
        .await
        .unwrap();
    assert_eq!(order_detail.purchase.status, "PENDIENTE");
    assert_eq!(order_detail.purchase.supplier_id, sup.id);
    assert_eq!(order_detail.items.len(), 1);
    assert_eq!(order_detail.items[0].product_id, prod.row.id);
    assert_eq!(order_detail.items[0].qty, dec(4));
    assert_eq!(order_detail.items[0].unit_cost, dec(12000));

    cleanup(admin, tenant, &[owner, cashier]).await;
}

/// F3.1 Clientes completos: normalización, DV, upsert, límites de crédito y archivo.
async fn f3_customers(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), None);
    let c_cashier = claims(cashier, Some(tenant), None);
    let branch = seed_branch(admin, tenant).await;

    // 1. [P] Un NIT con un DV incorrecto: 422 NIT_DV_INVALID
    // Para NIT "900123456", el DV real es 8. Si enviamos 5 debe fallar.
    let err_dv = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: Some("5".into()),
            name: "Cliente DV Malo SAS".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await;

    match err_dv {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "NIT_DV_INVALID");
        }
        other => panic!("esperaba 422 NIT_DV_INVALID pero obtuve {:?}", other),
    }

    // Crear cliente inicial con NIT válido: "900.123.456-8", teléfono "3001112233"
    let c1 = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900.123.456-8".into()),
            dv: None,
            name: "Empresa Original SAS".into(),
            phone: Some("3001112233".into()),
            email: Some("contacto@empresaoriginal.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: Some("RESPONSABLE_IVA".into()),
            fiscal_responsibilities: Some(vec!["O-13".into()]),
            ciiu: Some("6201".into()),
            withholding_agent: Some(false),
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: Some("Cliente VIP".into()),
            credit_limit: Some(dec(0)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    assert_eq!(c1.customer.dv.as_deref(), Some("8"));
    assert_eq!(c1.customer.person_type, "JURIDICA");
    assert_eq!(c1.customer.phone.as_deref(), Some("3001112233"));

    // 2. [P] `upsert` con un cliente existente solo llena los campos vacíos y no cambia el teléfono que ya tenía
    let upsert_res = customers::upsert(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: Some("8".into()),
            name: "Empresa Original SAS Renombrada".into(),
            phone: Some("3229998877".into()), // Nuevo teléfono enviado (no debe sobrescribir)
            email: Some("contacto@empresaoriginal.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: Some("05001".into()), // Campo vacío antes -> debe llenarse
            billing_address: Some("Calle 10 # 20-30".into()), // Campo vacío antes -> debe llenarse
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(upsert_res.result, "UPDATED");
    assert_eq!(
        upsert_res.customer.customer.phone.as_deref(),
        Some("3001112233"),
        "upsert no debe cambiar el teléfono que ya tenía el cliente"
    );
    assert_eq!(
        upsert_res.customer.customer.city_code.as_deref(),
        Some("05001"),
        "upsert debe llenar campos que estaban vacíos"
    );
    assert_eq!(
        upsert_res.customer.customer.billing_address.as_deref(),
        Some("Calle 10 # 20-30"),
        "upsert debe llenar dirección de facturación que estaba vacía"
    );

    // 3. [P] Un usuario sin `credit:override` que cambia el `credit_limit`: 403
    let err_credit_limit = customers::update(
        app,
        &c_cashier,
        c1.customer.id,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: Some("8".into()),
            name: "Empresa Original SAS".into(),
            phone: Some("3001112233".into()),
            email: Some("contacto@empresaoriginal.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: Some("RESPONSABLE_IVA".into()),
            fiscal_responsibilities: Some(vec!["O-13".into()]),
            ciiu: Some("6201".into()),
            withholding_agent: Some(false),
            city_code: Some("05001".into()),
            billing_address: Some("Calle 10 # 20-30".into()),
            shipping_address: None,
            pinned_note: Some("Cliente VIP".into()),
            credit_limit: Some(dec(500000)), // Cajero intenta cambiar cupo a 500.000 sin credit:override
            credit_block_days: Some(30),
        },
    )
    .await;

    match err_credit_limit {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Forbidden)) => {}
        other => panic!("esperaba 403 Forbidden pero obtuve {:?}", other),
    }

    // El dueño (con credit:override) sí puede cambiar el cupo
    let updated_by_owner = customers::update(
        app,
        &c_owner,
        c1.customer.id,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: Some("8".into()),
            name: "Empresa Original SAS".into(),
            phone: Some("3001112233".into()),
            email: Some("contacto@empresaoriginal.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: Some("RESPONSABLE_IVA".into()),
            fiscal_responsibilities: Some(vec!["O-13".into()]),
            ciiu: Some("6201".into()),
            withholding_agent: Some(false),
            city_code: Some("05001".into()),
            billing_address: Some("Calle 10 # 20-30".into()),
            shipping_address: None,
            pinned_note: Some("Cliente VIP".into()),
            credit_limit: Some(dec(500000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated_by_owner.customer.credit_limit, dec(500000));
    assert_eq!(updated_by_owner.credit_available, dec(500000));

    // [P] Un cajero que crea un cliente con credit_limit = 100000: 403 Forbidden
    let err_create_cashier = customers::create(
        app,
        &c_cashier,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("11223344".into()),
            dv: None,
            name: "Cliente Cupo Cajero".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(100000)),
            credit_block_days: Some(30),
        },
    )
    .await;
    match err_create_cashier {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Forbidden)) => {}
        other => panic!(
            "esperaba 403 Forbidden al crear cliente con cupo sin permiso pero obtuve {:?}",
            other
        ),
    }

    // [P] Un PUT del dueño sin credit_limit conserva el cupo guardado
    let put_no_credit = customers::update(
        app,
        &c_owner,
        c1.customer.id,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: Some("8".into()),
            name: "Empresa Original SAS Modificada".into(),
            phone: Some("3001112233".into()),
            email: Some("contacto@empresaoriginal.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: Some("RESPONSABLE_IVA".into()),
            fiscal_responsibilities: Some(vec!["O-13".into()]),
            ciiu: Some("6201".into()),
            withholding_agent: Some(false),
            city_code: Some("05001".into()),
            billing_address: Some("Calle 10 # 20-30".into()),
            shipping_address: None,
            pinned_note: Some("Cliente VIP".into()),
            credit_limit: None, // None conserva el valor anterior
            credit_block_days: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(put_no_credit.customer.credit_limit, dec(500000));

    // [P] Con una deuda de 100.000 en la sucursal A, un cajero atado a la sucursal B ve balance = 100000 en el detalle del cliente, y archivarlo da 409 CUSTOMER_HAS_BALANCE
    let branch_b = seed_branch(admin, tenant).await;
    let cashier_b = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch_b), cashier_b, "CAJERO").await;
    let c_cashier_b = claims(cashier_b, Some(tenant), Some(branch_b));

    let rec_id = Uuid::new_v4();
    sqlx::query(
        "insert into receivables (
            id, tenant_id, branch_id, customer_id, source, principal, amount_total,
            amount_paid, status, due_date, created_by
         ) values ($1, $2, $3, $4, 'ACUERDO', 100000, 100000, 0, 'ABIERTA', current_date + 30, $5)",
    )
    .bind(rec_id)
    .bind(tenant)
    .bind(branch)
    .bind(c1.customer.id)
    .bind(owner)
    .execute(admin)
    .await
    .unwrap();

    let detail_from_b = customers::get(app, &c_cashier_b, c1.customer.id)
        .await
        .unwrap();
    assert_eq!(detail_from_b.balance, dec(100000));

    // 4. [P] Archivar a un cliente con saldo: 409 CUSTOMER_HAS_BALANCE
    let err_archive = customers::archive(app, &c_owner, c1.customer.id).await;
    match err_archive {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CUSTOMER_HAS_BALANCE");
        }
        other => panic!("esperaba 409 CUSTOMER_HAS_BALANCE pero obtuve {:?}", other),
    }

    // Pagamos la cuenta por cobrar (status = PAGADA, amount_paid = 100000)
    sqlx::query("update receivables set status = 'PAGADA', amount_paid = 100000 where id = $1")
        .bind(rec_id)
        .execute(admin)
        .await
        .unwrap();

    // Ahora archivar debe tener éxito
    customers::archive(app, &c_owner, c1.customer.id)
        .await
        .unwrap();

    let detail_archived = customers::get(app, &c_owner, c1.customer.id).await.unwrap();
    assert!(detail_archived.customer.archived_at.is_some());

    // Desarchivar
    customers::unarchive(app, &c_owner, c1.customer.id)
        .await
        .unwrap();

    let detail_unarchived = customers::get(app, &c_owner, c1.customer.id).await.unwrap();
    assert!(detail_unarchived.customer.archived_at.is_none());

    // Pruebas adicionales en nuevo tenant para upsert por correo/doc, filtros, compras y conflicto de unarchive
    let t_up = seed_tenant(admin).await;
    let u_up = seed_user(admin).await;
    seed_membership(admin, t_up, None, u_up, "DUENO").await;
    let c_up = claims(u_up, Some(t_up), None);

    // [P] upsert que encuentra por correo a un cliente sin documento y recibe el NIT 900123456: queda con doc_type = NIT, DV 8 y person_type = JURIDICA
    let c_nodoc = customers::create(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: None,
            doc_number: None,
            dv: None,
            name: "Cliente Sin Doc Inicial".into(),
            phone: None,
            email: Some("nodoc@nexa.co".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();
    assert!(c_nodoc.customer.doc_type.is_none());

    let up_res = customers::upsert(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: None,
            name: "Cliente Con NIT Ya".into(),
            phone: None,
            email: Some("nodoc@nexa.co".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(up_res.result, "UPDATED");
    assert_eq!(up_res.customer.customer.id, c_nodoc.customer.id);
    assert_eq!(up_res.customer.customer.doc_type.as_deref(), Some("NIT"));
    assert_eq!(
        up_res.customer.customer.doc_number.as_deref(),
        Some("900123456")
    );
    assert_eq!(up_res.customer.customer.dv.as_deref(), Some("8"));
    assert_eq!(up_res.customer.customer.person_type, "JURIDICA");

    // [P] upsert que trae el documento de otro cliente activo: 409 CUSTOMER_DUPLICATED, con data.id de ese cliente
    let c_second = customers::create(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: None,
            doc_number: None,
            dv: None,
            name: "Segundo Cliente".into(),
            phone: None,
            email: Some("segundo@nexa.co".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();

    let err_up_dup = customers::upsert(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456".into()),
            dv: None,
            name: "Intento Duplicado".into(),
            phone: None,
            email: Some("segundo@nexa.co".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await;

    match err_up_dup {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            data,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CUSTOMER_DUPLICATED");
            let id_in_data =
                data.and_then(|d| d.get("id").and_then(|v| v.as_str().map(String::from)));
            assert_eq!(id_in_data, Some(c_nodoc.customer.id.to_string()));
        }
        other => panic!(
            "esperaba 409 CUSTOMER_DUPLICATED con data.id pero obtuve {:?}",
            other
        ),
    }

    // [P] Desarchivar a un cliente cuyo documento ya usa otro cliente activo: 409 CUSTOMER_DUPLICATED
    let cust_x = customers::create(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("88776655".into()),
            dv: None,
            name: "Cliente X Para Archivar".into(),
            phone: None,
            email: Some("x@test.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();

    customers::archive(app, &c_up, cust_x.customer.id)
        .await
        .unwrap();

    let cust_y = customers::create(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("88776655".into()),
            dv: None,
            name: "Cliente Y Con Mismo Doc".into(),
            phone: None,
            email: Some("y@test.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();

    let err_unarch = customers::unarchive(app, &c_up, cust_x.customer.id).await;
    match err_unarch {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            data,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CUSTOMER_DUPLICATED");
            let id_in_data =
                data.and_then(|d| d.get("id").and_then(|v| v.as_str().map(String::from)));
            assert_eq!(id_in_data, Some(cust_y.customer.id.to_string()));
        }
        other => panic!(
            "esperaba 409 CUSTOMER_DUPLICATED con data.id al desarchivar pero obtuve {:?}",
            other
        ),
    }

    // [P] Filtros de GET /v1/customers: un cliente con mora sale en EN_MORA y no en AL_DIA; uno archivado solo sale en ARCHIVADO (A17)
    let b_up = seed_branch(admin, t_up).await;
    let rec_mora = Uuid::new_v4();
    sqlx::query(
        "insert into receivables (
            id, tenant_id, branch_id, customer_id, source, principal, amount_total,
            amount_paid, status, due_date, created_by
         ) values ($1, $2, $3, $4, 'ACUERDO', 50000, 50000, 0, 'ABIERTA', current_date - 15, $5)",
    )
    .bind(rec_mora)
    .bind(t_up)
    .bind(b_up)
    .bind(c_second.customer.id)
    .bind(u_up)
    .execute(admin)
    .await
    .unwrap();

    let list_mora = customers::list(
        app,
        &c_up,
        customers::CustomerQuery {
            q: None,
            status: Some("EN_MORA".into()),
            page: None,
            page_size: None,
            limit: None,
        },
    )
    .await
    .unwrap();
    assert!(list_mora.iter().any(|c| c.id == c_second.customer.id));
    assert!(!list_mora.iter().any(|c| c.id == c_nodoc.customer.id));
    assert!(!list_mora.iter().any(|c| c.id == cust_x.customer.id));

    let list_aldia = customers::list(
        app,
        &c_up,
        customers::CustomerQuery {
            q: None,
            status: Some("AL_DIA".into()),
            page: None,
            page_size: None,
            limit: None,
        },
    )
    .await
    .unwrap();
    assert!(list_aldia.iter().any(|c| c.id == c_nodoc.customer.id));
    assert!(!list_aldia.iter().any(|c| c.id == c_second.customer.id));
    assert!(!list_aldia.iter().any(|c| c.id == cust_x.customer.id));

    let list_arch = customers::list(
        app,
        &c_up,
        customers::CustomerQuery {
            q: None,
            status: Some("ARCHIVADO".into()),
            page: None,
            page_size: None,
            limit: None,
        },
    )
    .await
    .unwrap();
    assert!(list_arch.iter().any(|c| c.id == cust_x.customer.id));
    assert!(!list_arch.iter().any(|c| c.id == c_nodoc.customer.id));
    assert!(!list_arch.iter().any(|c| c.id == c_second.customer.id));

    // [P] upsert que busca por documento (sin correo) encuentra al cliente existente (A17)
    let up_by_doc = customers::upsert(
        app,
        &c_up,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("88776655".into()),
            dv: None,
            name: "Cliente Y Con Mismo Doc".into(),
            phone: Some("3101234567".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(up_by_doc.customer.customer.id, cust_y.customer.id);
    assert_eq!(
        up_by_doc.customer.customer.phone.as_deref(),
        Some("3101234567")
    );

    // [P] GET /v1/customers/{id}/purchases devuelve las ventas del cliente con el formato de /v1/sales (A17)
    let prod_up = seed_product(admin, t_up, "Café Especial", "UND", None, false).await;
    set_base_price(admin, prod_up, "15000").await;
    seed_branch_product(admin, t_up, b_up, prod_up, None, None).await;
    seed_stock(admin, t_up, b_up, prod_up, "10", "0").await;

    let def_reg_up = default_register(admin, b_up).await;
    let _sess = cash::open(app, &c_up, b_up, def_reg_up, dec(50000))
        .await
        .unwrap();

    let _sale_res = sales::checkout(
        app,
        &c_up,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: b_up,
            items: vec![sales::SaleItem {
                product_id: Some(prod_up),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(15000),
                reference: None,
            }],
            quoted_total: dec(15000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_y.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let purchases = customers::list_purchases(app, &c_up, cust_y.customer.id, None, None)
        .await
        .unwrap();

    assert_eq!(purchases.total_count, 1);
    assert_eq!(purchases.totals.amount, dec(15000));
    assert_eq!(purchases.items.len(), 1);
    assert_eq!(purchases.items[0].total, dec(15000));
    assert_eq!(purchases.items[0].customer_label, cust_y.customer.name);
    assert_eq!(purchases.items[0].status, "COBRADA");

    cleanup(admin, t_up, &[u_up]).await;

    cleanup(admin, tenant, &[owner, cashier]).await;
}

/// F3.2 Venta a crédito y cuentas por cobrar: validaciones, cupo, mora, caja y cuotas.
async fn f3_credit_sale(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), None);
    let c_cashier = claims(cashier, Some(tenant), None);
    let branch = seed_branch(admin, tenant).await;

    // Crear producto para la venta
    let prod = seed_product(admin, tenant, "Zapatos de Cuero", "UND", None, false).await;
    set_base_price(admin, prod, "50000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "10", "0").await;

    // Abrir turno de caja con base de $100.000
    let def_reg = default_register(admin, branch).await;
    let _session = cash::open(app, &c_cashier, branch, def_reg, dec(100000))
        .await
        .unwrap();

    // 1. [P] Un pago CREDITO sin cliente: 422 CREDIT_REQUIRES_CUSTOMER
    let sale_no_cust = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await;

    match sale_no_cust {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CREDIT_REQUIRES_CUSTOMER");
        }
        other => panic!(
            "esperaba 422 CREDIT_REQUIRES_CUSTOMER pero obtuve {:?}",
            other
        ),
    }

    // 2. Crear cliente con cupo de $40.000 y 30 días de mora
    let cust = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("12345678".into()),
            dv: None,
            name: "Juan Pérez".into(),
            phone: Some("3001234567".into()),
            email: Some("juan@ejemplo.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(40000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    // [P] Superar el cupo: 409 CREDIT_LIMIT_EXCEEDED (venta de $50.000 con cupo de $40.000)
    let sale_exceed = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await;

    match sale_exceed {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            data,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CREDIT_LIMIT_EXCEEDED");
            assert!(
                data.is_some(),
                "data con cupo disponible debe estar presente"
            );
        }
        other => panic!("esperaba 409 CREDIT_LIMIT_EXCEEDED pero obtuve {:?}", other),
    }

    // Cajero intenta autorizar con override pero no tiene permiso credit:override -> 403
    let sale_unauthorized_override = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: Some("Autorizado por supervisor".into()),
        },
    )
    .await;

    match sale_unauthorized_override {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Forbidden)) => {}
        other => panic!("esperaba 403 Forbidden pero obtuve {:?}", other),
    }

    // Supervisor/dueño autoriza con credit:override y motivo -> la venta pasa y queda en auditoría
    sqlx::query(
        "update memberships set role_id = public.tipo_id('ROL', 'SUPERVISOR')
         where tenant_id = $1 and user_id = $2",
    )
    .bind(tenant)
    .bind(cashier)
    .execute(admin)
    .await
    .unwrap();

    let sale_authorized_id = Uuid::new_v4();
    let sale_authorized = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_authorized_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: Some("Autorizado por gerencia general".into()),
        },
    )
    .await
    .unwrap();

    match sale_authorized {
        sales::Checkout::Created(resp) => {
            assert_eq!(resp.payments.len(), 1);
            assert_eq!(resp.payments[0].method, "CREDITO");
            assert_eq!(resp.payments[0].amount, dec(50000));
        }
        other => panic!("esperaba Checkout::Created pero obtuve {:?}", other),
    }

    // [P] Superar el cupo: 409 CREDIT_LIMIT_EXCEEDED. Con credit:override y motivo, la venta pasa y existe una fila en audit_log con action = 'CREDITO_AUTORIZADO' y entity_id del cliente
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where tenant_id = $1 and action = 'CREDITO_AUTORIZADO' and entity_id = $2",
    )
    .bind(tenant)
    .bind(cust.customer.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(audit_count >= 1);

    // 3. [P] El crédito no suma al efectivo esperado del turno
    let summary = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    // La base era $100.000, la venta de $50.000 a crédito no debe sumar al efectivo
    assert_eq!(summary.expected_cash, dec(100000));
    let credit_method = summary.by_method.iter().find(|m| m.method == "CREDITO");
    assert!(
        credit_method.is_some(),
        "CREDITO debe aparecer en by_method"
    );
    assert_eq!(credit_method.unwrap().amount, dec(50000));

    // 4. [P] Un cliente con 31 días de mora y credit_block_days = 30: 409 CREDIT_BLOCKED_OVERDUE
    // Creamos cliente con cupo amplio pero una deuda con 31 días de vencimiento
    let cust_mora = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("87654321".into()),
            dv: None,
            name: "Cliente En Mora".into(),
            phone: Some("3119998877".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(200000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    let overdue_rec_id = Uuid::new_v4();
    sqlx::query(
        "insert into receivables (
            id, tenant_id, branch_id, customer_id, source, principal, amount_total,
            amount_paid, status, due_date, created_by
         ) values ($1, $2, $3, $4, 'ACUERDO', 10000, 10000, 0, 'ABIERTA', current_date - 35, $5)",
    )
    .bind(overdue_rec_id)
    .bind(tenant)
    .bind(branch)
    .bind(cust_mora.customer.id)
    .bind(owner)
    .execute(admin)
    .await
    .unwrap();

    let sale_overdue = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_mora.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await;

    match sale_overdue {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            data,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CREDIT_BLOCKED_OVERDUE");
            assert!(data.is_some(), "data con días de mora debe estar presente");
        }
        other => panic!(
            "esperaba 409 CREDIT_BLOCKED_OVERDUE pero obtuve {:?}",
            other
        ),
    }

    // 5. [P] Una venta con cuota inicial en efectivo más crédito crea una cuenta solo por la parte a crédito
    // Cliente con cupo suficiente
    let cust_mixed = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("55667788".into()),
            dv: None,
            name: "Cliente Pago Mixto".into(),
            phone: Some("3154443322".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(200000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    let mixed_sale_id = Uuid::new_v4();
    // Venta de 2 unidades = $100.000: $30.000 en efectivo + $70.000 a crédito en 2 cuotas mensuales
    let sale_mixed = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: mixed_sale_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![
                sales::SalePayment {
                    method: sales::PaymentMethod::Efectivo,
                    amount: dec(30000),
                    reference: None,
                },
                sales::SalePayment {
                    method: sales::PaymentMethod::Credito,
                    amount: dec(70000),
                    reference: None,
                },
            ],
            quoted_total: dec(100000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_mixed.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 2,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
                interest_rate_monthly: Decimal::ZERO,
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    match sale_mixed {
        sales::Checkout::Created(resp) => {
            assert_eq!(resp.payments.len(), 2);
        }
        other => panic!("esperaba Checkout::Created pero obtuve {:?}", other),
    }

    // Verificar que en receivables la cuenta se creó solo por la parte a crédito ($70.000)
    let rec_mixed: (Decimal, Decimal, Decimal, String) = sqlx::query_as(
        "select principal, amount_total, amount_paid, status from receivables where order_id = $1",
    )
    .bind(mixed_sale_id)
    .fetch_one(admin)
    .await
    .unwrap();

    assert_eq!(rec_mixed.0, dec(70000));
    assert_eq!(rec_mixed.1, dec(70000));
    assert_eq!(rec_mixed.2, dec(0));
    assert_eq!(rec_mixed.3, "ABIERTA");

    // Verificar cuotas: 2 cuotas de $35.000 cada una
    let inst_count: i64 = sqlx::query_scalar(
        "select count(*) from receivable_installments where receivable_id = (select id from receivables where order_id = $1)",
    )
    .bind(mixed_sale_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(inst_count, 2);

    let inst_amounts: Vec<Decimal> = sqlx::query_scalar(
        "select amount from receivable_installments where receivable_id = (select id from receivables where order_id = $1) order by n asc",
    )
    .bind(mixed_sale_id)
    .fetch_all(admin)
    .await
    .unwrap();
    assert_eq!(inst_amounts, vec![dec(35000), dec(35000)]);

    // Efectivo en caja debe haber aumentado solo por los $30.000 de la cuota inicial (base 100.000 + 30.000 = 130.000)
    let summary_mixed = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary_mixed.expected_cash, dec(130000));

    // [P] Cupo de todo el negocio: cliente con cupo de 100.000 y una deuda de 100.000 en la sucursal A. Un cajero atado a la sucursal B que le vende 1.000 a crédito recibe 409 CREDIT_LIMIT_EXCEEDED.
    let branch_b = seed_branch(admin, tenant).await;
    let cashier_b = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch_b), cashier_b, "CAJERO").await;
    let c_cashier_b = claims(cashier_b, Some(tenant), Some(branch_b));
    seed_branch_product(admin, tenant, branch_b, prod, None, None).await;
    seed_stock(admin, tenant, branch_b, prod, "10", "0").await;

    let cust_global = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("99887766".into()),
            dv: None,
            name: "Cliente Global".into(),
            phone: Some("3201112233".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(100000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    let rec_branch_a = Uuid::new_v4();
    sqlx::query(
        "insert into receivables (
            id, tenant_id, branch_id, customer_id, source, principal, amount_total,
            amount_paid, status, due_date, created_by
         ) values ($1, $2, $3, $4, 'ACUERDO', 100000, 100000, 0, 'ABIERTA', current_date + 30, $5)",
    )
    .bind(rec_branch_a)
    .bind(tenant)
    .bind(branch)
    .bind(cust_global.customer.id)
    .bind(owner)
    .execute(admin)
    .await
    .unwrap();

    let def_reg_b = default_register(admin, branch_b).await;
    let _session_b = cash::open(app, &c_cashier_b, branch_b, def_reg_b, dec(50000))
        .await
        .unwrap();

    let sale_cross_branch = sales::checkout(
        app,
        &c_cashier_b,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch_b,
            items: vec![sales::SaleItem {
                product_id: None,
                custom_name: Some("Item Libre 1000".into()),
                qty: dec(1),
                unit_price_override: Some(dec(1000)),
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: Some("EXENTO".into()),
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(1000),
                reference: None,
            }],
            quoted_total: dec(1000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_global.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await;

    match sale_cross_branch {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "CREDIT_LIMIT_EXCEEDED");
        }
        other => panic!("esperaba 409 CREDIT_LIMIT_EXCEEDED pero obtuve {:?}", other),
    }

    // [P] credit_plan con interest_rate_monthly = 2: 422 CREDIT_PLAN_INVALID
    let sale_plan_interest = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_mixed.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 3,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
                interest_rate_monthly: dec(2),
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await;

    match sale_plan_interest {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CREDIT_PLAN_INVALID");
        }
        other => panic!("esperaba 422 CREDIT_PLAN_INVALID pero obtuve {:?}", other),
    }

    // [P] credit_plan con first_due_date de ayer: 422 CREDIT_PLAN_INVALID
    let yesterday = chrono::Utc::now().date_naive() - chrono::Duration::days(5);
    let sale_plan_past = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_mixed.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 3,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: yesterday,
                interest_rate_monthly: Decimal::ZERO,
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await;

    match sale_plan_past {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CREDIT_PLAN_INVALID");
        }
        other => panic!("esperaba 422 CREDIT_PLAN_INVALID pero obtuve {:?}", other),
    }

    // [P] credit_plan sin pago CREDITO: 422 CREDIT_PLAN_INVALID
    let sale_plan_no_credit = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: Uuid::new_v4(),
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Efectivo,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_mixed.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 3,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
                interest_rate_monthly: Decimal::ZERO,
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await;

    match sale_plan_no_credit {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CREDIT_PLAN_INVALID");
        }
        other => panic!("esperaba 422 CREDIT_PLAN_INVALID pero obtuve {:?}", other),
    }

    // [P] Devolver toda una venta a crédito sin abonos deja la cuenta PAGADA, con saldo 0 y sin error
    let cust_refund = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("44332211".into()),
            dv: None,
            name: "Cliente Devolucion".into(),
            phone: Some("3185556677".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(200000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    let sale_to_refund_id = Uuid::new_v4();
    let sale_to_refund = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_to_refund_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_refund.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let order_item_id = match sale_to_refund {
        sales::Checkout::Created(ref resp) => resp.items[0].id,
        _ => unreachable!(),
    };

    let refund_res = refunds::create(
        app,
        &c_owner,
        refunds::RefundRequest {
            id: Uuid::new_v4(),
            order_id: sale_to_refund_id,
            method: refunds::RefundMethod::Original,
            reason: "Devolucion total de credito".into(),
            reversal_reference: None,
            items: vec![refunds::RefundItemInput {
                order_item_id,
                qty: dec(1),
                disposition: refunds::Disposition::Reintegrar,
            }],
        },
    )
    .await
    .unwrap();

    match refund_res {
        refunds::Created::New(r) => {
            assert_eq!(r.status, "APROBADA");
            assert_eq!(r.original_method.as_deref(), Some("CREDITO"));
        }
        _ => panic!("esperaba Created::New"),
    }

    let rec_after_refund: (Decimal, Decimal, Decimal, String) = sqlx::query_as(
        "select principal, amount_total, amount_paid, status from receivables where order_id = $1",
    )
    .bind(sale_to_refund_id)
    .fetch_one(admin)
    .await
    .unwrap();

    assert_eq!(rec_after_refund.0, dec(0)); // principal
    assert_eq!(rec_after_refund.1, dec(0)); // amount_total
    assert_eq!(rec_after_refund.2, dec(0)); // amount_paid
    assert_eq!(rec_after_refund.3, "PAGADA"); // status

    // [P] Devolver toda una venta a crédito en 3 cuotas sin abonos deja la cuenta PAGADA, sin cuotas y sin error
    let sale_3cuotas_id = Uuid::new_v4();
    let sale_3cuotas = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_3cuotas_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_refund.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 3,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
                interest_rate_monthly: Decimal::ZERO,
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let item_3cuotas_id = match sale_3cuotas {
        sales::Checkout::Created(ref resp) => resp.items[0].id,
        _ => unreachable!(),
    };

    let cuotas_before: i64 = sqlx::query_scalar(
        "select count(*) from receivable_installments where receivable_id = (select id from receivables where order_id = $1)",
    )
    .bind(sale_3cuotas_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(cuotas_before, 3);

    let refund_3cuotas = refunds::create(
        app,
        &c_owner,
        refunds::RefundRequest {
            id: Uuid::new_v4(),
            order_id: sale_3cuotas_id,
            method: refunds::RefundMethod::Original,
            reason: "Devolucion total de 3 cuotas".into(),
            reversal_reference: None,
            items: vec![refunds::RefundItemInput {
                order_item_id: item_3cuotas_id,
                qty: dec(1),
                disposition: refunds::Disposition::Reintegrar,
            }],
        },
    )
    .await
    .unwrap();

    match refund_3cuotas {
        refunds::Created::New(r) => {
            assert_eq!(r.status, "APROBADA");
        }
        _ => panic!("esperaba Created::New"),
    }

    let rec_3cuotas_after: (Decimal, Decimal, String) = sqlx::query_as(
        "select amount_total, amount_paid, status from receivables where order_id = $1",
    )
    .bind(sale_3cuotas_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(rec_3cuotas_after.0, dec(0));
    assert_eq!(rec_3cuotas_after.2, "PAGADA");

    let cuotas_after: i64 = sqlx::query_scalar(
        "select count(*) from receivable_installments where receivable_id = (select id from receivables where order_id = $1)",
    )
    .bind(sale_3cuotas_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(cuotas_after, 0);

    // [P] Venta de 100.000 (20.000 en efectivo y 80.000 a crédito) devuelta completa: la cuenta baja 80.000, receivable_applied = 80.000 y sale un movimiento DEVOLUCION de 20.000 de la caja.
    let sale_mixed_refund_id = Uuid::new_v4();
    let sale_mixed_refund = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_mixed_refund_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![
                sales::SalePayment {
                    method: sales::PaymentMethod::Efectivo,
                    amount: dec(20000),
                    reference: None,
                },
                sales::SalePayment {
                    method: sales::PaymentMethod::Credito,
                    amount: dec(80000),
                    reference: None,
                },
            ],
            quoted_total: dec(100000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_refund.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let mixed_item_id = match sale_mixed_refund {
        sales::Checkout::Created(ref resp) => resp.items[0].id,
        _ => unreachable!(),
    };

    let refund_mixed_id = Uuid::new_v4();
    let refund_mixed = refunds::create(
        app,
        &c_cashier,
        refunds::RefundRequest {
            id: refund_mixed_id,
            order_id: sale_mixed_refund_id,
            method: refunds::RefundMethod::Efectivo,
            reason: "Devolucion venta mixta".into(),
            reversal_reference: None,
            items: vec![refunds::RefundItemInput {
                order_item_id: mixed_item_id,
                qty: dec(2),
                disposition: refunds::Disposition::Reintegrar,
            }],
        },
    )
    .await
    .unwrap();

    match refund_mixed {
        refunds::Created::New(r) => {
            assert_eq!(r.status, "APROBADA");
        }
        _ => panic!("esperaba Created::New"),
    }

    let rec_mixed_after: (Decimal, Decimal, String) = sqlx::query_as(
        "select amount_total, amount_paid, status from receivables where order_id = $1",
    )
    .bind(sale_mixed_refund_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(rec_mixed_after.0, dec(0));
    assert_eq!(rec_mixed_after.2, "PAGADA");

    let rec_applied: Decimal =
        sqlx::query_scalar("select receivable_applied from refunds where id = $1")
            .bind(refund_mixed_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(rec_applied, dec(80000));

    let mov_amount: Decimal = sqlx::query_scalar(
        "select amount from cash_movements where ref_type = 'REFUND' and ref_id = $1",
    )
    .bind(refund_mixed_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_amount, dec(20000));

    // [P] Anular una venta a crédito con un abono: 409 VOID_NOT_ALLOWED con why = "HAS_PAYMENTS"
    let sale_to_void_id = Uuid::new_v4();
    let _sale_to_void = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_to_void_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(50000),
                reference: None,
            }],
            quoted_total: dec(50000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust_refund.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    // Simular un abono con el pool de administrador
    sqlx::query(
        "update receivables set amount_paid = 1000, status = 'PARCIAL' where order_id = $1",
    )
    .bind(sale_to_void_id)
    .execute(admin)
    .await
    .unwrap();

    let void_res = sales::void(
        app,
        &c_owner,
        sale_to_void_id,
        sales::VoidRequest {
            reason: "Anulacion venta con abono".into(),
        },
    )
    .await;
    assert_void_why(&void_res, "HAS_PAYMENTS");

    cleanup(admin, tenant, &[owner, cashier, cashier_b]).await;
}

/// F3.3 Abonos y cuotas: validaciones, aplicación a cuotas, caja, idempotencia y estados.
async fn f3_payments(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, None, cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), None);
    let c_cashier = claims(cashier, Some(tenant), None);
    let branch = seed_branch(admin, tenant).await;

    // Crear producto para la venta
    let prod = seed_product(admin, tenant, "Televisor 55", "UND", None, false).await;
    set_base_price(admin, prod, "100000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "10", "0").await;

    // Abrir turno de caja para cashier con base de $100.000
    let def_reg = default_register(admin, branch).await;
    let _session = cash::open(app, &c_cashier, branch, def_reg, dec(100000))
        .await
        .unwrap();

    // Crear cliente con cupo amplio
    let cust = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("10203040".into()),
            dv: None,
            name: "Carlos Abonos".into(),
            phone: Some("3001234567".into()),
            email: None,
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(500000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    let today = chrono::Utc::now().date_naive();
    let sale_id = Uuid::new_v4();
    let _sale = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(prod),
                custom_name: None,
                qty: dec(1),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(100000),
                reference: None,
            }],
            quoted_total: dec(100000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: Some(crate::credit::CreditPlan {
                installments: 3,
                frequency: crate::credit::Frequency::Mensual,
                first_due_date: today + chrono::Duration::days(30),
                interest_rate_monthly: Decimal::ZERO,
                interest_type: crate::credit::InterestType::Simple,
            }),
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let rec_id: Uuid = sqlx::query_scalar("select id from receivables where order_id = $1")
        .bind(sale_id)
        .fetch_one(admin)
        .await
        .unwrap();

    // 1. [P] Un abono de 50.000 sobre cuotas de 33.333,33, 33.333,33 y 33.333,34 paga completa la cuota 1 y abona 16.666,67 a la cuota 2. allocation = [{n:1, amount: 33333.33}, {n:2, amount: 16666.67}], y el due_date de la cuenta pasa al de la cuota 2.
    let pay1_id = Uuid::new_v4();
    let (status1, resp1) = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: pay1_id,
            amount: dec(50000),
            method: "TRANSFERENCIA".into(),
            reference: Some("TRF-12345".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(status1, axum::http::StatusCode::CREATED);
    assert_eq!(resp1.amount, dec(50000));
    assert_eq!(resp1.allocation.len(), 2);
    assert_eq!(resp1.allocation[0].n, 1);
    assert_eq!(resp1.allocation[0].amount, Decimal::new(3333333, 2));
    assert_eq!(resp1.allocation[1].n, 2);
    assert_eq!(resp1.allocation[1].amount, Decimal::new(1666667, 2));

    let inst2_due: chrono::NaiveDate = sqlx::query_scalar(
        "select due_date from receivable_installments where receivable_id = $1 and n = 2",
    )
    .bind(rec_id)
    .fetch_one(admin)
    .await
    .unwrap();

    let rec_row1: (Decimal, Decimal, String, chrono::NaiveDate) = sqlx::query_as(
        "select amount_total, amount_paid, status, due_date from receivables where id = $1",
    )
    .bind(rec_id)
    .fetch_one(admin)
    .await
    .unwrap();

    assert_eq!(rec_row1.1, dec(50000));
    assert_eq!(rec_row1.2, "PARCIAL");
    assert_eq!(rec_row1.3, inst2_due);

    // 2. [P] Un abono mayor que el saldo: 422 PAYMENT_EXCEEDS_BALANCE
    let err_excess = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(60000),
            method: "TRANSFERENCIA".into(),
            reference: Some("TRF-EXCESS".into()),
        },
    )
    .await;

    match err_excess {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            data,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "PAYMENT_EXCEEDS_BALANCE");
            assert_eq!(
                data.as_ref().and_then(|d| d.get("balance")),
                Some(&serde_json::json!("50000.00"))
            );
        }
        other => panic!(
            "esperaba 422 PAYMENT_EXCEEDS_BALANCE pero obtuve {:?}",
            other
        ),
    }

    // 3. [P] Un abono en efectivo crea un movimiento ABONO_CARTERA y sube el efectivo esperado del turno en el monto del abono
    let pay_cash_id = Uuid::new_v4();
    let (status_cash, resp_cash) = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: pay_cash_id,
            amount: dec(10000),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(status_cash, axum::http::StatusCode::CREATED);
    assert_eq!(resp_cash.balance_after, dec(40000));

    // Turno de caja: base de 100.000 + abono en efectivo de 10.000 = 110.000
    let summary_cash = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary_cash.expected_cash, dec(110000));

    let mov_exists: (Decimal, String) = sqlx::query_as(
        "select cm.amount, tm.tima_codigo
           from cash_movements cm
           join tbl_tipos_maestra tm on tm.tima_id = cm.kind_id
          where cm.ref_type = 'RECEIVABLE' and cm.ref_id = $1",
    )
    .bind(rec_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_exists.0, dec(10000));
    assert_eq!(mov_exists.1, "ABONO_CARTERA");

    // 4. [P] Un abono en efectivo sin turno abierto: 409 PAYMENT_NEEDS_OPEN_SESSION, y la cuenta no cambia
    let err_no_session = crate::receivables::record_payment(
        app,
        &c_owner, // El dueño no abrió turno de caja en la sucursal
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(10000),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await;

    match err_no_session {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "PAYMENT_NEEDS_OPEN_SESSION");
        }
        other => panic!(
            "esperaba 409 PAYMENT_NEEDS_OPEN_SESSION pero obtuve {:?}",
            other
        ),
    }

    // La cuenta sigue en 60.000 pagados (50.000 + 10.000)
    let paid_after_fail: Decimal =
        sqlx::query_scalar("select amount_paid from receivables where id = $1")
            .bind(rec_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(paid_after_fail, dec(60000));

    // 5. [P] Un reintento idéntico (mismo id y mismo cuerpo) no abona dos veces. Con el mismo id y otro monto: 409 IDEMPOTENCY_CONFLICT
    let (status_replay, resp_replay) = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: pay_cash_id,
            amount: dec(10000),
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(status_replay, axum::http::StatusCode::OK);
    assert_eq!(resp_replay.id, pay_cash_id);
    assert_eq!(resp_replay.amount, dec(10000));

    // No debe haber abonado dos veces
    let paid_after_replay: Decimal =
        sqlx::query_scalar("select amount_paid from receivables where id = $1")
            .bind(rec_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(paid_after_replay, dec(60000));

    let err_conflict = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: pay_cash_id,
            amount: dec(5000), // Monto diferente con el mismo id
            method: "EFECTIVO".into(),
            reference: None,
        },
    )
    .await;
    assert_eq!(err_code(&err_conflict), Some("IDEMPOTENCY_CONFLICT"));

    // 6. [P] method = "SALDO": 422 PAYMENT_METHOD_NOT_AVAILABLE
    let err_saldo = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(5000),
            method: "SALDO".into(),
            reference: None,
        },
    )
    .await;
    assert_eq!(err_code(&err_saldo), Some("PAYMENT_METHOD_NOT_AVAILABLE"));

    // 7. [P] method = "DATAFONO" sin referencia: 422 PAYMENT_REFERENCE_REQUIRED
    let err_ref = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(5000),
            method: "DATAFONO".into(),
            reference: None,
        },
    )
    .await;
    assert_eq!(err_code(&err_ref), Some("PAYMENT_REFERENCE_REQUIRED"));

    // 8. [P] Abonar el saldo completo deja la cuenta PAGADA. Un abono más: 409 RECEIVABLE_CLOSED
    let pay_full_id = Uuid::new_v4();
    let (status_full, resp_full) = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: pay_full_id,
            amount: dec(40000),
            method: "DATAFONO".into(),
            reference: Some("AUTH-45678".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(status_full, axum::http::StatusCode::CREATED);
    assert_eq!(resp_full.balance_after, dec(0));
    assert!(resp_full.next_due.is_none());

    let rec_final: (Decimal, Decimal, String) =
        sqlx::query_as("select amount_total, amount_paid, status from receivables where id = $1")
            .bind(rec_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(rec_final.1, dec(100000));
    assert_eq!(rec_final.2, "PAGADA");

    // Intento de abono a cuenta ya cerrada
    let err_closed = crate::receivables::record_payment(
        app,
        &c_cashier,
        rec_id,
        crate::receivables::CreateReceivablePaymentRequest {
            id: Uuid::new_v4(),
            amount: dec(1000),
            method: "TRANSFERENCIA".into(),
            reference: Some("TRF-EXTRA".into()),
        },
    )
    .await;
    assert_eq!(err_code(&err_closed), Some("RECEIVABLE_CLOSED"));

    // Verificar list y get_detail
    let (scope, mut tx) = tenancy::begin(app, &c_owner).await.unwrap();
    let detail = crate::receivables::get_receivable_detail_tx(&mut tx, &scope, rec_id)
        .await
        .unwrap();
    assert_eq!(detail.status, "PAGADA");
    assert_eq!(detail.installments.len(), 3);
    assert!(detail.installments.iter().all(|i| i.paid));
    assert_eq!(detail.payments.len(), 3);

    let list_all = crate::receivables::list_receivables_tx(
        &mut tx,
        &scope,
        crate::receivables::ReceivablesFilter {
            customer_id: Some(cust.customer.id),
            status: Some("PAGADA".into()),
            overdue: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(list_all.len(), 1);
    assert_eq!(list_all[0].id, rec_id);

    cleanup(admin, tenant, &[owner, cashier]).await;
}

#[test]
fn whatsapp_phone_normalization() {
    assert_eq!(
        crate::receivables::normalize_colombian_cell("300 123 4567"),
        Ok("3001234567".into())
    );
    assert_eq!(
        crate::receivables::normalize_colombian_cell("+57 300 123 4567"),
        Ok("3001234567".into())
    );
    assert!(crate::receivables::normalize_colombian_cell("6012345678").is_err());
    assert!(crate::receivables::normalize_colombian_cell("12345").is_err());
}

async fn f3_aging(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    let cashier = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;
    let c_owner = claims(owner, Some(tenant), None);
    let c_cashier = claims(cashier, Some(tenant), None);

    // Crear producto para la venta
    let product_id = seed_product(admin, tenant, "Zapatos de Cuero", "UND", None, false).await;
    set_base_price(admin, product_id, "50000").await;
    seed_branch_product(admin, tenant, branch, product_id, None, None).await;
    seed_stock(admin, tenant, branch, product_id, "10", "0").await;

    // Abrir turno de caja
    let def_reg = default_register(admin, branch).await;
    let _session = cash::open(app, &c_cashier, branch, def_reg, dec(50000))
        .await
        .unwrap();

    // Crear cliente con crédito y teléfono válido
    let cust = customers::create(
        app,
        &c_owner,
        customers::CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("1234567890".into()),
            dv: None,
            name: "Cliente Mora Cartera".into(),
            phone: Some("573001234567".into()),
            email: Some("cartera@ejemplo.com".into()),
            price_tier_id: None,
            person_type: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: Some(dec(1000000)),
            credit_block_days: Some(30),
        },
    )
    .await
    .unwrap();

    // Venta a crédito: $100.000 (2 unidades)
    let sale_id = Uuid::new_v4();
    let sale_resp = sales::checkout(
        app,
        &c_cashier,
        sales::SaleRequest {
            id: sale_id,
            branch_id: branch,
            items: vec![sales::SaleItem {
                product_id: Some(product_id),
                custom_name: None,
                qty: dec(2),
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
                custom_tax: None,
                option_ids: vec![],
            }],
            payments: vec![sales::SalePayment {
                method: sales::PaymentMethod::Credito,
                amount: dec(100000),
                reference: None,
            }],
            quoted_total: dec(100000),
            tip_amount: None,
            tip_waiter_id: None,
            customer_id: Some(cust.customer.id),
            customer_alias: None,
            coupon_code: None,
            credit_plan: None,
            credit_override_reason: None,
        },
    )
    .await
    .unwrap();

    let order_item_id = match sale_resp {
        sales::Checkout::Created(ref resp) => resp.items[0].id,
        _ => unreachable!(),
    };

    let rec_id: Uuid = sqlx::query_scalar("select id from receivables where order_id = $1")
        .bind(sale_id)
        .fetch_one(admin)
        .await
        .unwrap();

    // 1. [P] Los tramos de antigüedad suman total_balance, y una cuenta con 45 días de mora cae en d31_60
    // Modificar due_date para que tenga 45 días de mora
    sqlx::query(
        "update receivables
            set due_date = (now() at time zone 'America/Bogota')::date - 45
          where id = $1",
    )
    .bind(rec_id)
    .execute(admin)
    .await
    .unwrap();

    let summary = crate::receivables::get_summary(app, &c_owner, None)
        .await
        .unwrap();
    let sum_aging = summary.aging.al_dia
        + summary.aging.d1_30
        + summary.aging.d31_60
        + summary.aging.d61_90
        + summary.aging.d91_120
        + summary.aging.d120_mas;
    assert_eq!(summary.total_balance, sum_aging);
    assert_eq!(summary.aging.d31_60, dec(100000));
    assert_eq!(summary.overdue_balance, dec(100000));

    // 4. [P] El texto de whatsapp-link de una cuenta vencida contiene "vencido desde el"
    // 3. [P] whatsapp-link con el teléfono 573001234567 devuelve una url que empieza por https://wa.me/573001234567?text=. Con el teléfono 6012345678 (fijo): 422 CUSTOMER_NO_PHONE.
    let wa_link = crate::receivables::get_whatsapp_link(app, &c_owner, rec_id)
        .await
        .unwrap();
    assert!(wa_link.url.starts_with("https://wa.me/573001234567?text="));
    assert!(wa_link.url.contains("vencido%20desde%20el"));

    // Cambiar teléfono a número fijo
    sqlx::query("update customers set phone = '6012345678' where id = $1")
        .bind(cust.customer.id)
        .execute(admin)
        .await
        .unwrap();

    let err_fixed_phone = crate::receivables::get_whatsapp_link(app, &c_owner, rec_id).await;
    match err_fixed_phone {
        Err(crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            code,
            status,
            ..
        })) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "CUSTOMER_NO_PHONE");
        }
        other => panic!("esperaba 422 CUSTOMER_NO_PHONE pero obtuve {:?}", other),
    }

    // 2. [P] Después de una devolución parcial de una venta a crédito, closing_balance del estado de cuenta es igual al saldo del cliente, y hay una fila DEVOLUCION con el receivable_applied
    // Devolución parcial de 1 unidad ($50.000)
    let refund_res = refunds::create(
        app,
        &c_owner,
        refunds::RefundRequest {
            id: Uuid::new_v4(),
            order_id: sale_id,
            method: refunds::RefundMethod::Original,
            reason: "Devolucion parcial a credito".into(),
            reversal_reference: None,
            items: vec![refunds::RefundItemInput {
                order_item_id,
                qty: dec(1),
                disposition: refunds::Disposition::Reintegrar,
            }],
        },
    )
    .await
    .unwrap();

    match refund_res {
        refunds::Created::New(r) => {
            assert_eq!(r.status, "APROBADA");
            let applied: Decimal =
                sqlx::query_scalar("select receivable_applied from refunds where id = $1")
                    .bind(r.id)
                    .fetch_one(admin)
                    .await
                    .unwrap();
            assert_eq!(applied, dec(50000));
        }
        _ => panic!("esperaba Created::New"),
    }

    let cust_detail = customers::get(app, &c_owner, cust.customer.id)
        .await
        .unwrap();
    let customer_debt = cust_detail.balance;
    assert_eq!(customer_debt, dec(50000));

    let from_date = chrono::Utc::now().date_naive() - chrono::Duration::days(60);
    let to_date = chrono::Utc::now().date_naive();
    let statement = crate::receivables::get_customer_statement(
        app,
        &c_owner,
        cust.customer.id,
        from_date,
        to_date,
    )
    .await
    .unwrap();

    assert_eq!(statement.closing_balance, customer_debt);
    let dev_entry = statement
        .entries
        .iter()
        .find(|e| e.entry_type == "DEVOLUCION");
    assert!(dev_entry.is_some(), "debe existir una fila DEVOLUCION");
    let dev = dev_entry.unwrap();
    assert_eq!(dev.credit, dec(50000));

    cleanup(admin, tenant, &[owner, cashier]).await;
}

async fn s1_onboarding(app: &PgPool, admin: &PgPool) {
    // [P] Un usuario sin membresía crea su negocio. Quedan un tenant, una branch, una membresía DUENO con branch_id nulo y active = true, una location "Principal" con is_default = true, una cash_register CAJA-1 y la fila NEGOCIO_CREADO en audit_log.
    let user1 = seed_user(admin).await;
    let u1_claims = claims(user1, None, None);
    let resp = crate::onboarding::create_business(
        app,
        &u1_claims,
        crate::onboarding::CreateBusinessRequest {
            business_name: "Licorera Central".into(),
            branch_name: "Sede Centro".into(),
            full_name: "Pedro Perez".into(),
        },
    )
    .await
    .expect("crear negocio");

    // Verificar tenant
    let tenant_name: String = sqlx::query_scalar("select name from tenants where id = $1")
        .bind(resp.tenant_id)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(tenant_name, "Licorera Central");

    // Verificar branch
    let branch_name: String =
        sqlx::query_scalar("select name from branches where tenant_id = $1 and id = $2")
            .bind(resp.tenant_id)
            .bind(resp.branch_id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(branch_name, "Sede Centro");

    // Verificar membresía DUENO con branch_id nulo y active = true
    let (role_code, branch_id_opt, is_active): (String, Option<Uuid>, bool) = sqlx::query_as(
        "select t.tima_codigo, m.branch_id, m.active
         from memberships m
         join tbl_tipos_maestra t on t.tima_id = m.role_id
         where m.tenant_id = $1 and m.user_id = $2",
    )
    .bind(resp.tenant_id)
    .bind(user1)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(role_code, "DUENO");
    assert!(branch_id_opt.is_none());
    assert!(is_active);

    // Verificar location "Principal" con is_default = true
    let (loc_name, loc_default): (String, bool) = sqlx::query_as(
        "select name, is_default from locations where tenant_id = $1 and branch_id = $2",
    )
    .bind(resp.tenant_id)
    .bind(resp.branch_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(loc_name, "Principal");
    assert!(loc_default);

    // Verificar cash_register "Caja 1" (código "CAJA-1")
    let (reg_name, reg_code): (String, String) = sqlx::query_as(
        "select name, code from cash_registers where tenant_id = $1 and branch_id = $2",
    )
    .bind(resp.tenant_id)
    .bind(resp.branch_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(reg_name, "Caja 1");
    assert_eq!(reg_code, "CAJA-1");

    // Verificar audit_log NEGOCIO_CREADO
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where tenant_id = $1 and action = 'NEGOCIO_CREADO' and entity = 'tenants' and entity_id = $1 and user_id = $2",
    )
    .bind(resp.tenant_id)
    .bind(user1)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    // [P] Después del alta, tenancy::begin con ese usuario funciona y su rol es DUENO.
    let owner_claims = claims(user1, Some(resp.tenant_id), Some(resp.branch_id));
    let (scope, tx_begin) = crate::tenancy::begin(app, &owner_claims)
        .await
        .expect("tenancy begin debe funcionar");
    assert_eq!(scope.role, "DUENO");
    assert_eq!(scope.tenant, resp.tenant_id);
    tx_begin.rollback().await.unwrap();

    // [P] La segunda llamada del mismo usuario: 409 ONBOARDING_ALREADY_MEMBER. Sigue habiendo un solo negocio de ese usuario.
    let err_dup = crate::onboarding::create_business(
        app,
        &u1_claims,
        crate::onboarding::CreateBusinessRequest {
            business_name: "Otro Negocio".into(),
            branch_name: "Principal".into(),
            full_name: "Pedro Perez".into(),
        },
    )
    .await
    .unwrap_err();
    match err_dup {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "ONBOARDING_ALREADY_MEMBER");
        }
        other => panic!(
            "esperaba 409 ONBOARDING_ALREADY_MEMBER, obtuve: {:?}",
            other
        ),
    }
    let tenants_for_user: i64 = sqlx::query_scalar(
        "select count(distinct tenant_id) from memberships where user_id = $1 and active",
    )
    .bind(user1)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(tenants_for_user, 1);

    // [P] Un usuario con una membresía CAJERO activa en otro negocio: 409 ONBOARDING_ALREADY_MEMBER.
    let other_user = seed_user(admin).await;
    let other_tenant = seed_tenant(admin).await;
    let other_branch = seed_branch(admin, other_tenant).await;
    seed_membership(
        admin,
        other_tenant,
        Some(other_branch),
        other_user,
        "CAJERO",
    )
    .await;
    let other_claims = claims(other_user, None, None);
    let err_cashier = crate::onboarding::create_business(
        app,
        &other_claims,
        crate::onboarding::CreateBusinessRequest {
            business_name: "Negocio Dos".into(),
            branch_name: "Principal".into(),
            full_name: "Otro Usuario".into(),
        },
    )
    .await
    .unwrap_err();
    match err_cashier {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "ONBOARDING_ALREADY_MEMBER");
        }
        other => panic!(
            "esperaba 409 ONBOARDING_ALREADY_MEMBER, obtuve: {:?}",
            other
        ),
    }

    // [P] business_name = " a ": 422 ONBOARDING_INVALID, con field = "business_name".
    let user3 = seed_user(admin).await;
    let u3_claims = claims(user3, None, None);
    let err_short = crate::onboarding::create_business(
        app,
        &u3_claims,
        crate::onboarding::CreateBusinessRequest {
            business_name: " a ".into(),
            branch_name: "Principal".into(),
            full_name: "Carlos Lopez".into(),
        },
    )
    .await
    .unwrap_err();
    match err_short {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status,
            code,
            data,
            ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "ONBOARDING_INVALID");
            let field = data.and_then(|d| {
                d.get("field")
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string())
            });
            assert_eq!(field.as_deref(), Some("business_name"));
        }
        other => panic!(
            "esperaba 422 ONBOARDING_INVALID con field business_name, obtuve: {:?}",
            other
        ),
    }

    // [P] El rol anon no puede ejecutar public.create_business (error de permiso).
    let mut tx_anon = app.begin().await.unwrap();
    sqlx::query("set local role anon")
        .execute(&mut *tx_anon)
        .await
        .unwrap();
    let res_anon = sqlx::query("select * from public.create_business('X', 'Y', 'Z')")
        .execute(&mut *tx_anon)
        .await;
    assert!(
        res_anon.is_err(),
        "anon no debe poder ejecutar create_business"
    );
    tx_anon.rollback().await.unwrap();

    // [P] El negocio creado no ve datos de otro negocio: un GET /v1/customers con el dueño nuevo devuelve 0 filas aunque otro negocio tenga clientes.
    let other_owner = seed_user(admin).await;
    seed_membership(admin, other_tenant, None, other_owner, "DUENO").await;
    let other_owner_claims = claims(other_owner, Some(other_tenant), Some(other_branch));
    let _cust = crate::customers::create(
        app,
        &other_owner_claims,
        crate::customers::CreateCustomerInput {
            name: "Cliente De Otro Negocio".into(),
            doc_type: Some("CC".into()),
            doc_number: Some("123456789".into()),
            ..Default::default()
        },
    )
    .await
    .expect("crear cliente en otro negocio");

    let customers_list = crate::customers::list(
        app,
        &owner_claims,
        crate::customers::CustomerQuery {
            q: None,
            status: None,
            page: None,
            page_size: None,
            limit: None,
        },
    )
    .await
    .expect("listar clientes");
    assert_eq!(
        customers_list.len(),
        0,
        "el negocio nuevo debe ver 0 clientes"
    );

    cleanup(admin, resp.tenant_id, &[user1]).await;
    cleanup(admin, other_tenant, &[other_user, other_owner]).await;
}

async fn s2_invitations(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    sqlx::query("update auth.users set email_confirmed_at = now() where id = $1")
        .bind(owner)
        .execute(admin)
        .await
        .unwrap();
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let owner_c = claims(owner, Some(tenant), Some(branch));

    // [P] Invitar con role = "MESERO": 422 ROLE_NOT_AVAILABLE.
    let err_mesero = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: "mesero@test.com".into(),
            full_name: "Juan Mesero".into(),
            role: "MESERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .unwrap_err();
    match err_mesero {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "ROLE_NOT_AVAILABLE");
        }
        other => panic!("esperaba 422 ROLE_NOT_AVAILABLE, obtuve: {:?}", other),
    }

    // [P] Invitar a un correo que ya tiene una membresía activa en el negocio: 409 USER_ALREADY_MEMBER.
    let owner_email: String = sqlx::query_scalar("select email from users where id = $1")
        .bind(owner)
        .fetch_one(admin)
        .await
        .unwrap();
    let err_already = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: owner_email,
            full_name: "Dueño Mismo".into(),
            role: "CAJERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .unwrap_err();
    match err_already {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "USER_ALREADY_MEMBER");
        }
        other => panic!("esperaba 409 USER_ALREADY_MEMBER, obtuve: {:?}", other),
    }

    // [P] Un usuario con el correo confirmado ve la invitación en my_invitations, la acepta y queda con una membresía activa del rol y la sucursal invitados. La invitación queda ACEPTADA.
    let invitee = seed_user(admin).await;
    let invitee_email = format!("cajero_{}@itest.local", invitee);
    sqlx::query("update auth.users set email = $1, email_confirmed_at = now() where id = $2")
        .bind(&invitee_email)
        .bind(invitee)
        .execute(admin)
        .await
        .unwrap();
    sqlx::query("update users set email = $1 where id = $2")
        .bind(&invitee_email)
        .bind(invitee)
        .execute(admin)
        .await
        .unwrap();

    let inv = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: invitee_email.clone(),
            full_name: "Cajero Test".into(),
            role: "CAJERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .expect("invitar cajero");

    let invitee_c = claims(invitee, None, None);
    let my_invs = crate::invitations::my_invitations(app, &invitee_c)
        .await
        .expect("listar mis invitaciones");
    assert!(my_invs.iter().any(|i| i.id == inv.id));

    let accept_res = crate::invitations::accept_invitation(app, &invitee_c, inv.id)
        .await
        .expect("aceptar invitacion");
    assert_eq!(accept_res.tenant_id, tenant);
    assert_eq!(accept_res.branch_id, Some(branch));
    assert_eq!(accept_res.role, "CAJERO");

    // Verificar en BD que membership está activa
    let (mem_role, mem_active): (String, bool) = sqlx::query_as(
        "select t.tima_codigo, m.active
         from memberships m
         join tbl_tipos_maestra t on t.tima_id = m.role_id
         where m.tenant_id = $1 and m.user_id = $2 and m.branch_id = $3",
    )
    .bind(tenant)
    .bind(invitee)
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mem_role, "CAJERO");
    assert!(mem_active);

    // Verificar status ACEPTADA
    let inv_status: String = sqlx::query_scalar("select status from invitations where id = $1")
        .bind(inv.id)
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(inv_status, "ACEPTADA");

    // [P] Aceptar dos veces: la segunda da 409 INVITATION_NOT_PENDING.
    let err_second = crate::invitations::accept_invitation(app, &invitee_c, inv.id)
        .await
        .unwrap_err();
    match err_second {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "INVITATION_NOT_PENDING");
        }
        other => panic!("esperaba 409 INVITATION_NOT_PENDING, obtuve: {:?}", other),
    }

    // [P] Un usuario con otro correo que intenta aceptar: 404 INVITATION_NOT_FOUND, y no se crea membresía.
    let stranger = seed_user(admin).await;
    sqlx::query("update auth.users set email_confirmed_at = now() where id = $1")
        .bind(stranger)
        .execute(admin)
        .await
        .unwrap();
    let stranger_c = claims(stranger, None, None);

    let inv2 = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: format!("cajero2_{}@itest.local", Uuid::new_v4()),
            full_name: "Cajero Dos".into(),
            role: "CAJERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .expect("crear inv2");

    let err_stranger = crate::invitations::accept_invitation(app, &stranger_c, inv2.id)
        .await
        .unwrap_err();
    match err_stranger {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
            assert_eq!(code, "INVITATION_NOT_FOUND");
        }
        other => panic!("esperaba 404 INVITATION_NOT_FOUND, obtuve: {:?}", other),
    }

    let stranger_has_mem: bool = sqlx::query_scalar(
        "select exists(select 1 from memberships where user_id = $1 and tenant_id = $2)",
    )
    .bind(stranger)
    .bind(tenant)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(!stranger_has_mem);

    // [P] Un usuario con el correo sin confirmar no la ve en my_invitations, y aceptar da 404.
    let unconfirmed_user = seed_user(admin).await;
    let unconfirmed_email = format!("unconf_{}@itest.local", unconfirmed_user);
    sqlx::query("update auth.users set email = $1, email_confirmed_at = null where id = $2")
        .bind(&unconfirmed_email)
        .bind(unconfirmed_user)
        .execute(admin)
        .await
        .unwrap();

    let inv3 = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: unconfirmed_email,
            full_name: "Unconfirmed User".into(),
            role: "CAJERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .expect("crear inv3");

    let unconf_c = claims(unconfirmed_user, None, None);
    let unconf_invs = crate::invitations::my_invitations(app, &unconf_c)
        .await
        .unwrap();
    assert!(!unconf_invs.iter().any(|i| i.id == inv3.id));

    let err_unconf = crate::invitations::accept_invitation(app, &unconf_c, inv3.id)
        .await
        .unwrap_err();
    match err_unconf {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
            assert_eq!(code, "INVITATION_NOT_FOUND");
        }
        other => panic!("esperaba 404 INVITATION_NOT_FOUND, obtuve: {:?}", other),
    }

    // [P] Una invitación revocada no se puede aceptar: 409 INVITATION_NOT_PENDING.
    let user_for_revoked = seed_user(admin).await;
    let email_for_revoked = format!("rev_{}@itest.local", user_for_revoked);
    sqlx::query("update auth.users set email = $1, email_confirmed_at = now() where id = $2")
        .bind(&email_for_revoked)
        .bind(user_for_revoked)
        .execute(admin)
        .await
        .unwrap();

    let inv4 = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: email_for_revoked,
            full_name: "Revokee".into(),
            role: "CAJERO".into(),
            branch_id: Some(branch),
        },
    )
    .await
    .expect("crear inv4");

    crate::users::revoke_invitation(app, &owner_c, inv4.id)
        .await
        .expect("revocar invitacion");

    let rev_c = claims(user_for_revoked, None, None);
    let err_revoked = crate::invitations::accept_invitation(app, &rev_c, inv4.id)
        .await
        .unwrap_err();
    match err_revoked {
        crate::error::ApiError::Domain(crate::error::DomainError::Coded {
            status, code, ..
        }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "INVITATION_NOT_PENDING");
        }
        other => panic!("esperaba 409 INVITATION_NOT_PENDING, obtuve: {:?}", other),
    }

    // [P] ADMIN_CAJA no puede revocar una invitación de rol DUENO: 403.
    let admin_caja_user = seed_user(admin).await;
    seed_membership(admin, tenant, None, admin_caja_user, "ADMIN_CAJA").await;
    let admin_caja_c = claims(admin_caja_user, Some(tenant), Some(branch));

    let inv_owner = crate::users::invite_user(
        app,
        &owner_c,
        crate::users::InviteUserRequest {
            email: format!("coowner_{}@itest.local", Uuid::new_v4()),
            full_name: "Co Dueño".into(),
            role: "DUENO".into(),
            branch_id: None,
        },
    )
    .await
    .expect("invitar co-dueno");

    let err_admin_caja = crate::users::revoke_invitation(app, &admin_caja_c, inv_owner.id)
        .await
        .unwrap_err();
    match err_admin_caja {
        crate::error::ApiError::Domain(crate::error::DomainError::Forbidden) => {}
        other => panic!("esperaba 403 Forbidden, obtuve: {:?}", other),
    }

    cleanup(
        admin,
        tenant,
        &[
            owner,
            invitee,
            stranger,
            unconfirmed_user,
            user_for_revoked,
            admin_caja_user,
        ],
    )
    .await;
}

async fn s3_business(app: &PgPool, admin: &PgPool) {
    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;
    let owner = seed_user(admin).await;
    seed_membership(admin, tenant, None, owner, "DUENO").await;
    let owner_c = claims(owner, Some(tenant), Some(branch));

    let admin_caja_user = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), admin_caja_user, "ADMIN_CAJA").await;
    let admin_caja_c = claims(admin_caja_user, Some(tenant), Some(branch));

    // [P] NIT "900123456" con dv = 7: 422 NIT_DV_INVALID.
    let err_invalid_dv = settings::update_business(
        app,
        &owner_c,
        settings::UpdateBusinessRequest {
            name: "Mi Negocio".into(),
            business_name: Some("Mi Negocio SAS".into()),
            nit: Some("900123456".into()),
            dv: Some("7".into()),
            iva_responsible: Some(true),
        },
    )
    .await
    .unwrap_err();
    match err_invalid_dv {
        ApiError::Domain(DomainError::Coded { status, code, .. }) => {
            assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(code, "NIT_DV_INVALID");
        }
        other => panic!("esperaba 422 NIT_DV_INVALID, obtuve: {:?}", other),
    }

    // [P] El dueño cambia el nombre, la razón social y el NIT "900123456" sin DV: queda guardado con DV "8" y hay auditoría NEGOCIO_ACTUALIZADO.
    let updated = settings::update_business(
        app,
        &owner_c,
        settings::UpdateBusinessRequest {
            name: "Comercial Valido".into(),
            business_name: Some("Comercial SAS".into()),
            nit: Some("900123456".into()),
            dv: None,
            iva_responsible: Some(true),
        },
    )
    .await
    .expect("update business as owner");
    assert_eq!(updated.name, "Comercial Valido");
    assert_eq!(updated.business_name.as_deref(), Some("Comercial SAS"));
    assert_eq!(updated.nit.as_deref(), Some("900123456"));
    assert_eq!(updated.dv.as_deref(), Some("8"));
    assert_eq!(updated.iva_responsible, Some(true));

    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where tenant_id = $1 and action = 'NEGOCIO_ACTUALIZADO' and entity_id = $1",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(audit_count >= 1, "esperaba auditoria NEGOCIO_ACTUALIZADO");

    // [P] ADMIN_CAJA cambia solo name: 200. ADMIN_CAJA cambia nit: 403.
    let updated_name = settings::update_business(
        app,
        &admin_caja_c,
        settings::UpdateBusinessRequest {
            name: "Nombre Por Admin".into(),
            business_name: Some("Comercial SAS".into()),
            nit: Some("900123456".into()),
            dv: Some("8".into()),
            iva_responsible: Some(true),
        },
    )
    .await
    .expect("admin_caja should be allowed to change only name");
    assert_eq!(updated_name.name, "Nombre Por Admin");

    let err_admin_caja_nit = settings::update_business(
        app,
        &admin_caja_c,
        settings::UpdateBusinessRequest {
            name: "Nombre Por Admin".into(),
            business_name: Some("Comercial SAS".into()),
            nit: Some("800111222".into()),
            dv: None,
            iva_responsible: Some(true),
        },
    )
    .await
    .unwrap_err();
    match err_admin_caja_nit {
        ApiError::Domain(DomainError::Forbidden) => {}
        other => panic!(
            "esperaba 403 Forbidden al cambiar nit como ADMIN_CAJA, obtuve: {:?}",
            other
        ),
    }

    // [P] ADMIN_CAJA crea una sucursal: 403.
    let err_branch_admin_caja = tenancy::create_branch(
        app,
        &admin_caja_c,
        tenancy::CreateBranchRequest { name: "Sur".into() },
    )
    .await
    .unwrap_err();
    match err_branch_admin_caja {
        ApiError::Domain(DomainError::Forbidden) => {}
        other => panic!(
            "esperaba 403 Forbidden al crear sucursal como ADMIN_CAJA, obtuve: {:?}",
            other
        ),
    }

    // [P] El dueño crea la sucursal "Norte": existe, con su bodega "Principal" y su caja CAJA-1. Crear otra "norte": 409 BRANCH_DUPLICATED.
    let branch_norte = tenancy::create_branch(
        app,
        &owner_c,
        tenancy::CreateBranchRequest {
            name: "Norte".into(),
        },
    )
    .await
    .expect("crear sucursal Norte");
    assert_eq!(branch_norte.name, "Norte");

    let has_location: bool = sqlx::query_scalar(
        "select exists(select 1 from locations where branch_id = $1 and name = 'Principal')",
    )
    .bind(branch_norte.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(has_location, "sucursal nueva debe tener bodega Principal");

    let has_register: bool = sqlx::query_scalar(
        "select exists(select 1 from cash_registers where branch_id = $1 and code = 'CAJA-1')",
    )
    .bind(branch_norte.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(has_register, "sucursal nueva debe tener caja CAJA-1");

    let err_duplicated = tenancy::create_branch(
        app,
        &owner_c,
        tenancy::CreateBranchRequest {
            name: "norte".into(),
        },
    )
    .await
    .unwrap_err();
    match err_duplicated {
        ApiError::Domain(DomainError::Coded { status, code, .. }) => {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(code, "BRANCH_DUPLICATED");
        }
        other => panic!("esperaba 409 BRANCH_DUPLICATED, obtuve: {:?}", other),
    }

    // Actualizar nombre de sucursal (PUT /v1/branches/{id})
    let updated_branch = tenancy::update_branch(
        app,
        &owner_c,
        branch_norte.id,
        tenancy::UpdateBranchRequest {
            name: "Norte Centro".into(),
        },
    )
    .await
    .expect("update branch name");
    assert_eq!(updated_branch.name, "Norte Centro");

    let branch_audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where tenant_id = $1 and action = 'SUCURSAL_ACTUALIZADA' and entity_id = $2",
    )
    .bind(tenant)
    .bind(branch_norte.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert!(
        branch_audit_count >= 1,
        "esperaba auditoria SUCURSAL_ACTUALIZADA"
    );

    // [P] Con la membresía desactivada, select name from tenants devuelve 0 filas.
    let user_m = seed_user(admin).await;
    seed_membership(admin, tenant, Some(branch), user_m, "CAJERO").await;
    let user_m_c = claims(user_m, Some(tenant), Some(branch));

    let mut tx_active = crate::db::begin_scope_tx(app, &user_m_c, None, None)
        .await
        .unwrap();
    let rows_active: Vec<String> = sqlx::query_scalar("select name from tenants where id = $1")
        .bind(tenant)
        .fetch_all(&mut *tx_active)
        .await
        .unwrap();
    tx_active.commit().await.unwrap();
    assert_eq!(
        rows_active.len(),
        1,
        "con membresia activa debe ver el tenant"
    );

    sqlx::query("update memberships set active = false where user_id = $1 and tenant_id = $2")
        .bind(user_m)
        .bind(tenant)
        .execute(admin)
        .await
        .unwrap();

    let mut tx_inactive = crate::db::begin_scope_tx(app, &user_m_c, None, None)
        .await
        .unwrap();
    let rows_inactive: Vec<String> = sqlx::query_scalar("select name from tenants where id = $1")
        .bind(tenant)
        .fetch_all(&mut *tx_inactive)
        .await
        .unwrap();
    tx_inactive.commit().await.unwrap();
    assert!(
        rows_inactive.is_empty(),
        "con membresia desactivada select name from tenants devuelve 0 filas"
    );

    cleanup(admin, tenant, &[owner, admin_caja_user, user_m]).await;
}

async fn s4_permissions(app: &PgPool, admin: &PgPool) {
    use refunds::{Disposition::*, RefundMethod::*};

    let tenant = seed_tenant(admin).await;
    let branch = seed_branch(admin, tenant).await;

    let owner = seed_user(admin).await;
    let admin_caja = seed_user(admin).await;
    let supervisor = seed_user(admin).await;
    let bodega = seed_user(admin).await;
    let cashier = seed_user(admin).await;

    seed_membership(admin, tenant, None, owner, "DUENO").await;
    seed_membership(admin, tenant, Some(branch), admin_caja, "ADMIN_CAJA").await;
    seed_membership(admin, tenant, Some(branch), supervisor, "SUPERVISOR").await;
    seed_membership(admin, tenant, Some(branch), bodega, "BODEGA").await;
    seed_membership(admin, tenant, Some(branch), cashier, "CAJERO").await;

    let c_owner = claims(owner, Some(tenant), Some(branch));
    let c_admin = claims(admin_caja, Some(tenant), Some(branch));
    let c_supervisor = claims(supervisor, Some(tenant), Some(branch));
    let c_bodega = claims(bodega, Some(tenant), Some(branch));
    let c_cashier = claims(cashier, Some(tenant), Some(branch));

    let sup = procurement::create_supplier(
        app,
        &c_owner,
        procurement::CreateSupplierRequest {
            name: "Distribuidora S4".into(),
            tax_id: Some("900888777-1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let prod = seed_product(admin, tenant, "Cerveza S4", "UND", Some("IVA19"), false).await;
    set_base_price(admin, prod, "5000").await;
    seed_branch_product(admin, tenant, branch, prod, None, None).await;
    seed_stock(admin, tenant, branch, prod, "10", "0").await;

    let make_purchase_req = |doc: &str| procurement::CreatePurchaseRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        supplier_id: sup.id,
        document_number: Some(doc.into()),
        items: vec![procurement::PurchaseItemRequest {
            product_id: prod,
            qty: dec(5),
            unit_cost: dec(2000),
            new_sale_price: None,
        }],
        ..Default::default()
    };

    // [P] BODEGA crea una compra: 403. BODEGA recibe una compra aprobada: 200.
    assert!(
        is_forbidden(
            &procurement::create_purchase(app, &c_bodega, make_purchase_req("FAC-S4-NO")).await
        ),
        "BODEGA no debe poder crear compras"
    );
    let p_bodega = procurement::create_purchase(app, &c_owner, make_purchase_req("FAC-S4-1"))
        .await
        .unwrap();
    // Sin aprobar (PENDIENTE), BODEGA tampoco puede auto-aprobarla al recibirla.
    assert!(
        is_forbidden(&procurement::receive_purchase(app, &c_bodega, p_bodega.purchase.id).await),
        "BODEGA no debe auto-aprobar una compra pendiente"
    );
    assert!(
        is_forbidden(&procurement::approve_purchase(app, &c_bodega, p_bodega.purchase.id).await),
        "BODEGA no debe aprobar compras"
    );
    assert!(
        is_forbidden(&procurement::cancel_purchase(app, &c_bodega, p_bodega.purchase.id).await),
        "BODEGA no debe anular compras"
    );
    procurement::approve_purchase(app, &c_owner, p_bodega.purchase.id)
        .await
        .unwrap();
    let rec_bodega = procurement::receive_purchase(app, &c_bodega, p_bodega.purchase.id)
        .await
        .expect("BODEGA debe poder recibir una compra aprobada");
    assert_eq!(rec_bodega.purchase.status, "RECIBIDA");

    // [P] SUPERVISOR aprueba una compra: 403. SUPERVISOR la recibe: 200.
    assert!(
        is_forbidden(
            &procurement::create_purchase(app, &c_supervisor, make_purchase_req("FAC-S4-NO2"))
                .await
        ),
        "SUPERVISOR no debe crear compras"
    );
    let p_sup = procurement::create_purchase(app, &c_owner, make_purchase_req("FAC-S4-2"))
        .await
        .unwrap();
    assert!(
        is_forbidden(&procurement::approve_purchase(app, &c_supervisor, p_sup.purchase.id).await),
        "SUPERVISOR no debe aprobar compras"
    );
    procurement::approve_purchase(app, &c_owner, p_sup.purchase.id)
        .await
        .unwrap();
    let rec_sup = procurement::receive_purchase(app, &c_supervisor, p_sup.purchase.id)
        .await
        .expect("SUPERVISOR debe poder recibir una compra aprobada");
    assert_eq!(rec_sup.purchase.status, "RECIBIDA");

    // [P] CAJERO lista compras: 403.
    assert!(
        is_forbidden(
            &procurement::list_purchases(app, &c_cashier, procurement::PurchaseFilter::default())
                .await
        ),
        "CAJERO no debe listar compras"
    );
    assert!(
        is_forbidden(&procurement::get_purchase(app, &c_cashier, p_bodega.purchase.id).await),
        "CAJERO no debe ver detalle de compras"
    );
    assert!(
        !procurement::list_purchases(app, &c_supervisor, procurement::PurchaseFilter::default())
            .await
            .expect("SUPERVISOR si puede listar compras")
            .is_empty()
    );
    assert!(
        !procurement::list_purchases(app, &c_bodega, procurement::PurchaseFilter::default())
            .await
            .expect("BODEGA si puede listar compras")
            .is_empty()
    );

    // [P] ADMIN_CAJA crea, aprueba y recibe: 200 en las tres.
    let p_admin = procurement::create_purchase(app, &c_admin, make_purchase_req("FAC-S4-3"))
        .await
        .expect("ADMIN_CAJA crea compra");
    assert_eq!(p_admin.purchase.status, "PENDIENTE");
    let p_admin_approved = procurement::approve_purchase(app, &c_admin, p_admin.purchase.id)
        .await
        .expect("ADMIN_CAJA aprueba compra");
    assert_eq!(p_admin_approved.purchase.status, "APROBADA");
    let p_admin_received = procurement::receive_purchase(app, &c_admin, p_admin.purchase.id)
        .await
        .expect("ADMIN_CAJA recibe compra");
    assert_eq!(p_admin_received.purchase.status, "RECIBIDA");

    // [P] Un cajero con turno abierto pide una devolución en efectivo. El dueño, sin turno, la aprueba. El movimiento DEVOLUCION queda en el turno del cajero.
    let reg = default_register(admin, branch).await;
    let cashier_session = cash::open(app, &c_cashier, branch, reg, dec(10000))
        .await
        .unwrap();

    // Venta de 2 unidades (5.000 * 1.19 = 5.950 cada una -> 11.900 total).
    let mut sale_req = sale(Uuid::new_v4(), branch, &[(prod, 2)], &[], d("11900"));
    sale_req.payments = cash_payment("11900");
    let s1 = created(sales::checkout(app, &c_cashier, sale_req).await.unwrap());

    let r1 = new_refund(
        refunds::create(
            app,
            &c_cashier,
            refund_req(
                Uuid::new_v4(),
                s1.order.id,
                s1.items[0].id,
                1,
                Reintegrar,
                Efectivo,
            ),
        )
        .await
        .unwrap(),
    );
    assert_eq!(r1.status, "PENDIENTE");

    // Dueño sin turno abierto en la sucursal aprueba la devolución.
    assert!(
        cash::current(app, &c_owner, branch)
            .await
            .unwrap()
            .is_none()
    );
    let r1_approved = refunds::approve(app, &c_owner, r1.id)
        .await
        .expect("Dueño sin turno aprueba devolución pedida por cajero con turno abierto");
    assert_eq!(r1_approved.status, "APROBADA");

    let mov_session: (Uuid, String) = sqlx::query_as(
        "select m.cash_session_id, t.tima_codigo
           from cash_movements m
           join tbl_tipos_maestra t on t.tima_id = m.kind_id
          where m.ref_type = 'REFUND' and m.ref_id = $1",
    )
    .bind(r1.id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(mov_session.0, cashier_session.id);
    assert_eq!(mov_session.1, "DEVOLUCION");

    let refund_cash_session: Option<Uuid> =
        sqlx::query_scalar("select cash_session_id from refunds where id = $1")
            .bind(r1.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(refund_cash_session, Some(cashier_session.id));

    let cashier_summary = cash::current(app, &c_cashier, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cashier_summary.expected_cash.normalize(), dec(15950));

    // SUPERVISOR sin turno también puede aprobar la devolución de la segunda unidad -> queda en el turno del cajero.
    let r2 = new_refund(
        refunds::create(
            app,
            &c_cashier,
            refund_req(
                Uuid::new_v4(),
                s1.order.id,
                s1.items[0].id,
                1,
                Reintegrar,
                Efectivo,
            ),
        )
        .await
        .unwrap(),
    );
    let r2_approved = refunds::approve(app, &c_supervisor, r2.id)
        .await
        .expect("SUPERVISOR sin turno aprueba devolución pedida por cajero con turno abierto");
    assert_eq!(r2_approved.status, "APROBADA");
    let refund2_session: Option<Uuid> =
        sqlx::query_scalar("select cash_session_id from refunds where id = $1")
            .bind(r2.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(refund2_session, Some(cashier_session.id));

    // Si tanto el cajero como el dueño tienen turno abierto, la devolución sigue saliendo del turno del cajero.
    let reg2: Uuid = sqlx::query_scalar(
        "insert into cash_registers (tenant_id, branch_id, name, code)
         values ($1, $2, 'Caja S4 2', 'CAJA-S4-2') returning id",
    )
    .bind(tenant)
    .bind(branch)
    .fetch_one(admin)
    .await
    .unwrap();
    let owner_session = cash::open(app, &c_owner, branch, reg2, dec(50000))
        .await
        .unwrap();

    let mut sale2_req = sale(Uuid::new_v4(), branch, &[(prod, 1)], &[], d("5950"));
    sale2_req.payments = cash_payment("5950");
    let s2 = created(sales::checkout(app, &c_cashier, sale2_req).await.unwrap());
    let r3 = new_refund(
        refunds::create(
            app,
            &c_cashier,
            refund_req(
                Uuid::new_v4(),
                s2.order.id,
                s2.items[0].id,
                1,
                Reintegrar,
                Efectivo,
            ),
        )
        .await
        .unwrap(),
    );
    refunds::approve(app, &c_owner, r3.id).await.unwrap();
    let r3_session: Option<Uuid> =
        sqlx::query_scalar("select cash_session_id from refunds where id = $1")
            .bind(r3.id)
            .fetch_one(admin)
            .await
            .unwrap();
    assert_eq!(r3_session, Some(cashier_session.id));
    let owner_summary = cash::current(app, &c_owner, branch).await.unwrap().unwrap();
    assert_eq!(owner_summary.session.id, owner_session.id);
    assert_eq!(owner_summary.expected_cash.normalize(), dec(50000));

    cleanup(
        admin,
        tenant,
        &[owner, admin_caja, supervisor, bodega, cashier],
    )
    .await;
}

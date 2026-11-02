//! Inventario: disponibilidad, reservas, consumo y bitacora (kardex).
//!
//! Cada operacion existe en dos formas:
//! - `*_tx(tx, scope, ..)`: trabaja dentro de una transaccion abierta con
//!   [`tenancy::begin`], para componerla con otras (p. ej. confirmar una orden).
//! - `fn(pool, claims, ..)`: abre su propia transaccion y la confirma.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy::{self, Action, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

/// Movimiento de inventario (bitacora / kardex).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Movement {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub movement_type: String,
    /// Cantidad del movimiento (siempre positiva).
    pub qty: Decimal,
    /// Efecto con signo sobre `on_hand` (+ entra, - sale). `null` para el
    /// tipo heredado `AJUSTE`, que no registraba la direccion.
    pub delta: Option<Decimal>,
    /// Existencia del producto despues del movimiento (reconstruida desde la
    /// existencia actual hacia atras).
    pub balance_after: Option<Decimal>,
    pub cause: Option<String>,
    pub ref_type: Option<String>,
    pub ref_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub branch_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateAdjustmentRequest {
    pub id: Uuid,
    pub branch_id: Uuid,
    #[serde(default)]
    pub location_id: Option<Uuid>,
    pub product_id: Uuid,
    #[serde(default)]
    pub lot_id: Option<Uuid>,
    pub qty_delta: Decimal,
    pub reason_code: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub unit_cost: Option<Decimal>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RejectAdjustmentRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct InventoryAdjustment {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub location_id: Uuid,
    pub product_id: Uuid,
    pub product_name: Option<String>,
    pub sku: Option<String>,
    pub location_name: Option<String>,
    pub lot_id: Option<Uuid>,
    pub lot_code: Option<String>,
    pub qty_delta: Decimal,
    pub reason_code: String,
    pub notes: Option<String>,
    pub unit_cost: Decimal,
    pub value: Decimal,
    pub status: String,
    pub requested_by: Uuid,
    pub requester_name: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decider_name: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub reject_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Existencia de un producto en una sucursal, para la pantalla de inventario.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StockLevel {
    pub product_id: Uuid,
    pub name: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub category: Option<String>,
    pub unit: Option<String>,
    pub kind: String,
    pub usage: String,
    /// Precio de venta en la sucursal (sin impuesto).
    pub price: Option<Decimal>,
    pub cost_price: Option<Decimal>,
    pub on_hand: Decimal,
    pub reserved: Decimal,
    pub available: Decimal,
    /// Punto de reorden: en o por debajo se considera stock bajo.
    pub min_qty: Decimal,
    /// AGOTADO | BAJO | OK
    pub status: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived: bool,
}

/// Fila de reserva: (id, branch_id, location_id, product_id, qty).
type ReservationRow = (Uuid, Uuid, Uuid, Uuid, Decimal);

const AVAILABLE_SQL: &str = r#"
select case
  when p.is_composed then
    (select min(floor((coalesce(si.on_hand, 0) - coalesce(si.reserved, 0)) / nullif(ri.qty, 0)))
       from recipe_items ri
       join recipes r2 on r2.id = ri.recipe_id
       left join (
         select product_id, sum(on_hand) as on_hand, sum(reserved) as reserved
           from stock
          where branch_id = $1
          group by product_id
       ) si on si.product_id = ri.input_product_id
      where r2.product_id = p.id)
  else coalesce(s.on_hand, 0) - coalesce(s.reserved, 0)
end
from products p
left join (
  select product_id, sum(on_hand) as on_hand, sum(reserved) as reserved
    from stock
   where branch_id = $1
   group by product_id
) s on s.product_id = p.id
where p.id = $2
"#;

fn positive(qty: Decimal) -> Result<(), ApiError> {
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }
    Ok(())
}

// ------------------------------------------------------------ disponibilidad

/// Disponible de un producto en una sucursal (derivado si es compuesto).
pub async fn available(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Uuid,
) -> Result<Option<Decimal>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::Read)?;
    let value = available_tx(&mut tx, branch_id, product_id).await?;
    tx.commit().await?;
    Ok(value)
}

pub async fn available_tx(
    tx: &mut Tx<'_>,
    branch_id: Uuid,
    product_id: Uuid,
) -> Result<Option<Decimal>, ApiError> {
    expire_due_tx(tx).await?;
    let row: Option<Option<Decimal>> = sqlx::query_scalar(AVAILABLE_SQL)
        .bind(branch_id)
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?;
    row.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Explota la receta de un producto en componentes con stock (o el propio producto).
pub async fn components(
    tx: &mut Tx<'_>,
    product_id: Uuid,
    qty: Decimal,
) -> Result<Vec<(Uuid, Decimal)>, ApiError> {
    let kind: Option<String> = sqlx::query_scalar("select kind from products where id = $1")
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?;
    if kind.as_deref() == Some("SERVICIO") {
        return Ok(vec![]);
    }

    let items: Vec<(Uuid, Decimal)> = sqlx::query_as(
        "select ri.input_product_id, ri.qty
           from recipe_items ri
           join recipes r on r.id = ri.recipe_id
          where r.product_id = $1",
    )
    .bind(product_id)
    .fetch_all(&mut **tx)
    .await?;

    if !items.is_empty() {
        return Ok(items.into_iter().map(|(p, r)| (p, r * qty)).collect());
    }

    Ok(vec![(product_id, qty)])
}

/// Bloquea las filas de stock de varios productos en orden estable, para que
/// operaciones con varias lineas no se bloqueen mutuamente (deadlock).
pub async fn lock_stock_tx(
    tx: &mut Tx<'_>,
    branch_id: Uuid,
    product_ids: &[Uuid],
) -> Result<(), ApiError> {
    sqlx::query(
        "select 1 from stock
          where branch_id = $1 and product_id = any($2)
          order by product_id
          for update",
    )
    .bind(branch_id)
    .bind(product_ids)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ------------------------------------------------------------------ reservas

/// Valida disponibilidad y crea las reservas por insumo en una sola transaccion.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn reserve(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    order_id: Option<Uuid>,
    order_item_id: Option<Uuid>,
    product_id: Uuid,
    qty: Decimal,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Vec<Uuid>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;
    let ids = reserve_tx(
        &mut tx,
        &scope,
        branch_id,
        order_id,
        order_item_id,
        product_id,
        qty,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(ids)
}

#[allow(clippy::too_many_arguments)]
async fn check_not_variant_parent(tx: &mut Tx<'_>, product_id: Uuid) -> Result<(), ApiError> {
    let has_variants: Option<bool> =
        sqlx::query_scalar("select has_variants from products where id = $1")
            .bind(product_id)
            .fetch_optional(&mut **tx)
            .await?;
    if has_variants.unwrap_or(false) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_IS_VARIANT_PARENT",
            "No se puede operar stock de un producto padre de variantes",
            None,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn reserve_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    order_id: Option<Uuid>,
    order_item_id: Option<Uuid>,
    product_id: Uuid,
    qty: Decimal,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Vec<Uuid>, ApiError> {
    positive(qty)?;
    check_not_variant_parent(tx, product_id).await?;

    let exists: bool = sqlx::query_scalar("select exists(select 1 from products where id = $1)")
        .bind(product_id)
        .fetch_one(&mut **tx)
        .await?;
    if !exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    expire_due_tx(tx).await?;

    let mut comps = components(tx, product_id, qty).await?;
    comps.sort_by_key(|c| c.0);

    let mut ids = Vec::new();
    for (stock_product, need) in comps {
        let loc_id = crate::locations::default_location_id_tx(tx, branch_id).await?;
        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "select on_hand, reserved from stock
              where location_id = $1 and product_id = $2
              for update",
        )
        .bind(loc_id)
        .bind(stock_product)
        .fetch_optional(&mut **tx)
        .await?;

        let (on_hand, reserved) = row.ok_or(ApiError::Domain(DomainError::Conflict))?;
        if on_hand - reserved < need {
            return Err(ApiError::Domain(DomainError::Conflict));
        }

        sqlx::query(
            "update stock set reserved = reserved + $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(need)
        .bind(loc_id)
        .bind(stock_product)
        .execute(&mut **tx)
        .await?;

        let id: Uuid = sqlx::query_scalar(
            "insert into stock_reservations
               (tenant_id, branch_id, location_id, order_id, order_item_id, product_id, qty, status_id, expires_at)
             values ($1, $2, $3, $4, $5, $6, $7, public.tipo_id('RES', 'ACTIVA'), $8)
             returning id",
        )
        .bind(scope.tenant)
        .bind(branch_id)
        .bind(loc_id)
        .bind(order_id)
        .bind(order_item_id)
        .bind(stock_product)
        .bind(need)
        .bind(expires_at)
        .fetch_one(&mut **tx)
        .await?;

        ids.push(id);
    }

    Ok(ids)
}

// ------------------------------------------------------------------- consumo

/// Convierte reservas activas en descuento definitivo y registra el movimiento.
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn consume(
    pool: &PgPool,
    claims: &Claims,
    reservation_ids: &[Uuid],
) -> Result<u64, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let count = consume_tx(&mut tx, &scope, reservation_ids).await?;
    tx.commit().await?;
    Ok(count)
}

#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn consume_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    reservation_ids: &[Uuid],
) -> Result<u64, ApiError> {
    let rows: Vec<ReservationRow> = sqlx::query_as(
        "select id, branch_id, location_id, product_id, qty from stock_reservations
          where id = any($1) and status_id = public.tipo_id('RES', 'ACTIVA')
          order by product_id
          for update",
    )
    .bind(reservation_ids)
    .fetch_all(&mut **tx)
    .await?;
    consume_rows(tx, scope, rows).await
}

/// Consume todas las reservas activas de una orden (al cobrarla).
pub async fn consume_order_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    order_id: Uuid,
) -> Result<u64, ApiError> {
    let rows: Vec<ReservationRow> = sqlx::query_as(
        "select id, branch_id, location_id, product_id, qty from stock_reservations
          where order_id = $1 and status_id = public.tipo_id('RES', 'ACTIVA')
          order by product_id
          for update",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    consume_rows(tx, scope, rows).await
}

async fn consume_rows(
    tx: &mut Tx<'_>,
    scope: &Scope,
    rows: Vec<ReservationRow>,
) -> Result<u64, ApiError> {
    let mut count = 0u64;
    for (id, branch_id, location_id, product_id, qty) in rows {
        sqlx::query(
            "update stock
                set reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "update stock_reservations set status_id = public.tipo_id('RES', 'CONFIRMADA'), updated_at = now() where id = $1",
        )
        .bind(id)
        .execute(&mut **tx)
        .await?;

        apply_delta_tx(
            tx,
            scope,
            StockDelta {
                branch_id,
                location_id,
                product_id,
                delta: -qty,
                movement: "DESCUENTO",
                unit_cost: None,
                lot_id: None,
                ref_type: "reservation",
                ref_id: id,
                cause: Some("consumo".to_string()),
            },
        )
        .await?;

        count += 1;
    }
    Ok(count)
}

// ------------------------------------------------------------ venta directa

#[allow(clippy::too_many_arguments)]
async fn sell_component_fefo_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    location_id: Uuid,
    order_id: Uuid,
    order_item_id: Option<Uuid>,
    product_id: Uuid,
    need: Decimal,
) -> Result<(Decimal, bool), ApiError> {
    let track_lots: bool =
        sqlx::query_scalar("select coalesce(track_lots, false) from products where id = $1")
            .bind(product_id)
            .fetch_optional(&mut **tx)
            .await?
            .unwrap_or(false);

    if !track_lots {
        let applied = apply_delta_tx(
            tx,
            scope,
            StockDelta {
                branch_id,
                location_id,
                product_id,
                delta: -need,
                movement: "DESCUENTO",
                unit_cost: None,
                lot_id: None,
                ref_type: "order",
                ref_id: order_id,
                cause: Some("venta".to_string()),
            },
        )
        .await?;
        return Ok((applied.total_cost, applied.cost_missing));
    }

    #[derive(sqlx::FromRow)]
    struct LotItem {
        id: Uuid,
        #[allow(dead_code)]
        code: String,
        #[allow(dead_code)]
        expires_at: chrono::NaiveDate,
        qty: Decimal,
    }

    let lots: Vec<LotItem> = sqlx::query_as(
        r#"
        select id, code, expires_at, qty
          from lots
         where location_id = $1 and product_id = $2 and qty > 0
           and expires_at >= (now() at time zone (select timezone from branches where id = $3))::date
         order by expires_at asc, code asc
           for update
        "#,
    )
    .bind(location_id)
    .bind(product_id)
    .bind(branch_id)
    .fetch_all(&mut **tx)
    .await?;

    let available_valid: Decimal = lots.iter().map(|l| l.qty).sum();
    if available_valid < need {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "LOT_ONLY_EXPIRED",
            format!("insufficient non-expired lots for product {}", product_id),
            Some(serde_json::json!({
                "product_id": product_id,
                "available_valid": available_valid,
            })),
        ));
    }

    let mut remaining = need;
    let mut total_cost = Decimal::ZERO;
    let mut cost_missing = false;

    for lot in lots {
        if remaining <= Decimal::ZERO {
            break;
        }
        let take = remaining.min(lot.qty);
        let applied = apply_delta_tx(
            tx,
            scope,
            StockDelta {
                branch_id,
                location_id,
                product_id,
                delta: -take,
                movement: "DESCUENTO",
                unit_cost: None,
                lot_id: Some(lot.id),
                ref_type: "order",
                ref_id: order_id,
                cause: Some("venta".to_string()),
            },
        )
        .await?;
        total_cost += applied.total_cost;
        cost_missing = cost_missing || applied.cost_missing;

        if let Some(oi_id) = order_item_id {
            sqlx::query(
                r#"
                insert into order_item_lots (tenant_id, order_item_id, lot_id, qty)
                values ($1, $2, $3, $4)
                on conflict (order_item_id, lot_id) do update set qty = order_item_lots.qty + excluded.qty
                "#,
            )
            .bind(scope.tenant)
            .bind(oi_id)
            .bind(lot.id)
            .bind(take)
            .execute(&mut **tx)
            .await?;
        }

        remaining -= take;
    }

    Ok((total_cost, cost_missing))
}

/// Descuenta stock sin pasar por reserva (venta de mostrador: se cobra en el
/// acto). Explota recetas, agrega por insumo, bloquea en orden estable y falla
/// con 409 si algun insumo no alcanza. Registra un DESCUENTO por insumo con
/// referencia a la orden.
pub async fn sell_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    location_id: Uuid,
    order_id: Uuid,
    lines: &[(Uuid, Decimal)],
) -> Result<(), ApiError> {
    expire_due_tx(tx).await?;

    // Query order_items for this order to compute and save line costs
    #[derive(sqlx::FromRow)]
    struct ItemRow {
        id: Uuid,
        product_id: Option<Uuid>,
        qty: Decimal,
        units_per_package: Decimal,
    }

    let items: Vec<ItemRow> = sqlx::query_as(
        "select id, product_id, qty, units_per_package from order_items where order_id = $1",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;

    if !items.is_empty() {
        for item in items {
            let mut line_cost_total = Decimal::ZERO;
            let mut line_cost_missing = false;

            if let Some(pid) = item.product_id {
                let kind: Option<String> =
                    sqlx::query_scalar("select kind from products where id = $1")
                        .bind(pid)
                        .fetch_optional(&mut **tx)
                        .await?;
                if kind.as_deref() != Some("SERVICIO") {
                    let total_qty = item.qty * item.units_per_package;
                    for (comp, need) in components(tx, pid, total_qty).await? {
                        let (c_total, c_missing) = sell_component_fefo_tx(
                            tx,
                            scope,
                            branch_id,
                            location_id,
                            order_id,
                            Some(item.id),
                            comp,
                            need,
                        )
                        .await?;
                        line_cost_total += c_total;
                        line_cost_missing = line_cost_missing || c_missing;
                    }
                }
            }

            // Insumos de opciones
            #[derive(sqlx::FromRow)]
            struct OptInput {
                input_product_id: Uuid,
                input_qty: Decimal,
            }
            let opt_inputs: Vec<OptInput> = sqlx::query_as(
                "select o.input_product_id, o.input_qty
                   from order_item_options oio
                   join options o on o.id = oio.option_id
                  where oio.order_item_id = $1 and o.input_product_id is not null and o.input_qty is not null",
            )
            .bind(item.id)
            .fetch_all(&mut **tx)
            .await?;

            for opt in opt_inputs {
                let opt_total_qty = item.qty * opt.input_qty;
                for (comp, need) in components(tx, opt.input_product_id, opt_total_qty).await? {
                    let (c_total, c_missing) = sell_component_fefo_tx(
                        tx,
                        scope,
                        branch_id,
                        location_id,
                        order_id,
                        Some(item.id),
                        comp,
                        need,
                    )
                    .await?;
                    line_cost_total += c_total;
                    line_cost_missing = line_cost_missing || c_missing;
                }
            }

            let line_unit_cost = if item.qty > Decimal::ZERO {
                (line_cost_total / item.qty).round_dp(4)
            } else {
                Decimal::ZERO
            };

            sqlx::query(
                "update order_items set cost_total = $1, unit_cost = $2, cost_missing = $3 where id = $4",
            )
            .bind(line_cost_total)
            .bind(line_unit_cost)
            .bind(line_cost_missing)
            .bind(item.id)
            .execute(&mut **tx)
            .await?;
        }
    } else {
        // Fallback si no hay order_items (llamada directa con lines)
        let mut needs: Vec<(Uuid, Decimal)> = Vec::new();
        for (product_id, qty) in lines {
            positive(*qty)?;
            for (component, need) in components(tx, *product_id, *qty).await? {
                match needs.iter_mut().find(|(p, _)| *p == component) {
                    Some((_, total)) => *total += need,
                    None => needs.push((component, need)),
                }
            }
        }
        needs.sort_by_key(|(p, _)| *p);
        for (product_id, need) in needs {
            sell_component_fefo_tx(
                tx,
                scope,
                branch_id,
                location_id,
                order_id,
                None,
                product_id,
                need,
            )
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- liberacion

/// Libera explicitamente reservas activas devolviendo las unidades.
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn release(
    pool: &PgPool,
    claims: &Claims,
    reservation_ids: &[Uuid],
) -> Result<u64, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let count = release_tx(&mut tx, reservation_ids).await?;
    tx.commit().await?;
    Ok(count)
}

#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn release_tx(tx: &mut Tx<'_>, reservation_ids: &[Uuid]) -> Result<u64, ApiError> {
    let rows: Vec<ReservationRow> = sqlx::query_as(
        "update stock_reservations set status_id = public.tipo_id('RES', 'LIBERADA'), updated_at = now()
          where id = any($1) and status_id = public.tipo_id('RES', 'ACTIVA')
          returning id, branch_id, location_id, product_id, qty",
    )
    .bind(reservation_ids)
    .fetch_all(&mut **tx)
    .await?;
    return_reserved(tx, &rows).await
}

/// Libera todas las reservas activas de una orden (al cancelarla).
pub async fn release_order_tx(tx: &mut Tx<'_>, order_id: Uuid) -> Result<u64, ApiError> {
    let rows: Vec<ReservationRow> = sqlx::query_as(
        "update stock_reservations set status_id = public.tipo_id('RES', 'LIBERADA'), updated_at = now()
          where order_id = $1 and status_id = public.tipo_id('RES', 'ACTIVA')
          returning id, branch_id, location_id, product_id, qty",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    return_reserved(tx, &rows).await
}

/// Devuelve al disponible las unidades de reservas ya marcadas como no activas.
async fn return_reserved(tx: &mut Tx<'_>, rows: &[ReservationRow]) -> Result<u64, ApiError> {
    for (_id, _branch_id, location_id, product_id, qty) in rows {
        sqlx::query(
            "update stock set reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(rows.len() as u64)
}

async fn expire_due_tx(tx: &mut Tx<'_>) -> Result<u64, ApiError> {
    let expired: Vec<ReservationRow> = sqlx::query_as(
        "update stock_reservations set status_id = public.tipo_id('RES', 'EXPIRADA'), updated_at = now()
          where status_id = public.tipo_id('RES', 'ACTIVA') and expires_at is not null and expires_at < now()
          returning id, branch_id, location_id, product_id, qty",
    )
    .fetch_all(&mut **tx)
    .await?;
    return_reserved(tx, &expired).await
}

/// Barrido de reservas vencidas del tenant activo.
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn expire_due(pool: &PgPool, claims: &Claims) -> Result<u64, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;
    let count = expire_due_tx(&mut tx).await?;
    tx.commit().await?;
    Ok(count)
}

// ------------------------------------------------------- reposicion y ajuste

/// Repone inventario y registra el movimiento.
pub async fn replenish(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    replenish_tx(&mut tx, &scope, branch_id, product_id, qty, cause).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn replenish_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    replenish_ref_tx(tx, scope, branch_id, product_id, qty, cause, None, None).await
}

#[allow(clippy::too_many_arguments)]
pub async fn replenish_ref_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
    reference: Option<(&str, Uuid)>,
    unit_cost: Option<Decimal>,
) -> Result<(), ApiError> {
    scope.require(Action::ManageInventory)?;
    tenancy::ensure_branch_tx(tx, scope, branch_id).await?;
    positive(qty)?;
    check_not_variant_parent(tx, product_id).await?;

    let loc_id = crate::locations::default_location_id_tx(tx, branch_id).await?;
    let (ref_type, ref_id) = reference.unwrap_or(("MANUAL", Uuid::nil()));

    apply_delta_tx(
        tx,
        scope,
        StockDelta {
            branch_id,
            location_id: loc_id,
            product_id,
            delta: qty,
            movement: "REPOSICION",
            unit_cost,
            lot_id: None,
            ref_type,
            ref_id,
            cause: Some(cause.to_string()),
        },
    )
    .await?;

    Ok(())
}

/// Reingresa al inventario lo devuelto por un cliente (devolución o anulación).
///
/// No exige permiso de inventario: el permiso lo valida quien llama
/// (`pos:sell` o `pos:refund`). Un producto con receta devuelve sus insumos,
/// igual que `sell_tx` los descontó. El movimiento es de tipo `DEVOLUCION`.
#[allow(clippy::too_many_arguments)]
pub async fn return_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
    reference: (&str, Uuid),
    lot_id: Option<Uuid>,
) -> Result<(), ApiError> {
    positive(qty)?;
    let loc_id = crate::locations::default_location_id_tx(tx, branch_id).await?;
    for (component, need) in components(tx, product_id, qty).await? {
        apply_delta_tx(
            tx,
            scope,
            StockDelta {
                branch_id,
                location_id: loc_id,
                product_id: component,
                delta: need,
                movement: "DEVOLUCION",
                unit_cost: None,
                lot_id,
                ref_type: reference.0,
                ref_id: reference.1,
                cause: Some(cause.to_string()),
            },
        )
        .await?;
    }
    Ok(())
}

/// Ajusta inventario (delta con signo) y registra el movimiento.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn adjust(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Uuid,
    delta: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    adjust_tx(&mut tx, &scope, branch_id, product_id, delta, cause).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn adjust_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    delta: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    let loc_id = default_location_id_tx(tx, branch_id).await?;
    let existing_on_hand: Decimal = sqlx::query_scalar(
        "select coalesce(on_hand, 0) from stock where location_id = $1 and product_id = $2",
    )
    .bind(loc_id)
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(Decimal::ZERO);

    let settings = crate::settings::load_tx(tx, branch_id).await?;
    if delta < Decimal::ZERO
        && !settings.inventory.allow_negative_stock
        && existing_on_hand + delta < Decimal::ZERO
    {
        return Err(ApiError::Domain(DomainError::Validation(
            "adjustment violates stock".into(),
        )));
    }

    let reason_code = if delta > Decimal::ZERO {
        "CORRECCION"
    } else {
        "PERDIDA"
    };
    create_adjustment_tx(
        tx,
        scope,
        CreateAdjustmentRequest {
            id: Uuid::new_v4(),
            branch_id,
            location_id: Some(loc_id),
            product_id,
            lot_id: None,
            qty_delta: delta,
            reason_code: reason_code.to_string(),
            notes: Some(cause.to_string()),
            unit_cost: None,
        },
    )
    .await?;
    Ok(())
}

/// Obtiene un ajuste por ID con información enriquecida para el cliente.
pub async fn get_adjustment_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<InventoryAdjustment, ApiError> {
    let row = sqlx::query_as::<_, InventoryAdjustment>(
        r#"
        select
            a.id,
            a.branch_id,
            a.location_id,
            a.product_id,
            p.name as product_name,
            p.sku,
            loc.name as location_name,
            a.lot_id,
            l.code as lot_code,
            a.qty_delta,
            a.reason_code,
            a.notes,
            a.unit_cost,
            a.value,
            a.status,
            a.requested_by,
            ru.full_name as requester_name,
            a.decided_by,
            du.full_name as decider_name,
            a.decided_at,
            a.reject_reason,
            a.created_at
        from inventory_adjustments a
        join products p on p.id = a.product_id
        join locations loc on loc.id = a.location_id
        left join lots l on l.id = a.lot_id
        left join users ru on ru.id = a.requested_by
        left join users du on du.id = a.decided_by
        where a.tenant_id = $1 and a.id = $2
        "#,
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    Ok(row)
}

/// Lista ajustes de inventario con filtros por sucursal y estado.
pub async fn list_adjustments(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Option<Uuid>,
    status: Option<String>,
) -> Result<Vec<InventoryAdjustment>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Read)?;
    let rows = sqlx::query_as::<_, InventoryAdjustment>(
        r#"
        select
            a.id,
            a.branch_id,
            a.location_id,
            a.product_id,
            p.name as product_name,
            p.sku,
            loc.name as location_name,
            a.lot_id,
            l.code as lot_code,
            a.qty_delta,
            a.reason_code,
            a.notes,
            a.unit_cost,
            a.value,
            a.status,
            a.requested_by,
            ru.full_name as requester_name,
            a.decided_by,
            du.full_name as decider_name,
            a.decided_at,
            a.reject_reason,
            a.created_at
        from inventory_adjustments a
        join products p on p.id = a.product_id
        join locations loc on loc.id = a.location_id
        left join lots l on l.id = a.lot_id
        left join users ru on ru.id = a.requested_by
        left join users du on du.id = a.decided_by
        where a.tenant_id = $1
          and ($2::uuid is null or a.branch_id = $2)
          and ($3::text is null or a.status = $3)
        order by a.created_at desc
        "#,
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(status)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

/// Crea o aplica un ajuste según el umbral de aprobación.
pub async fn create_adjustment_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: CreateAdjustmentRequest,
) -> Result<InventoryAdjustment, ApiError> {
    scope.require(Action::ManageInventory)?;
    tenancy::ensure_branch_tx(tx, scope, req.branch_id).await?;

    // Idempotencia por id y client_request
    let req_val = serde_json::to_value(&req).unwrap_or_default();
    let existing: Option<(serde_json::Value,)> = sqlx::query_as(
        "select client_request from inventory_adjustments where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(req.id)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some((stored_req,)) = existing {
        if stored_req == req_val {
            return get_adjustment_tx(tx, scope, req.id).await;
        } else {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "Esta operación ya se registró con otros datos.",
                None,
            ));
        }
    }

    if req.notes.as_ref().is_some_and(|n| n.chars().count() > 300) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ADJUST_INVALID",
            "Las notas no pueden superar los 300 caracteres.",
            None,
        ));
    }

    let location_id = match req.location_id {
        Some(lid) => lid,
        None => default_location_id_tx(tx, req.branch_id).await?,
    };

    check_not_variant_parent(tx, req.product_id).await?;

    // Validar motivo
    let valid_reasons = [
        "DANO",
        "PERDIDA",
        "ROBO",
        "VENCIMIENTO",
        "CONTEO",
        "CORRECCION",
        "CARGA_INICIAL",
    ];
    if !valid_reasons.contains(&req.reason_code.as_str()) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ADJUST_INVALID",
            "Motivo de ajuste no válido.",
            None,
        ));
    }

    if req.qty_delta == Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ADJUST_INVALID",
            "La cantidad a ajustar no puede ser cero.",
            None,
        ));
    }

    // Coherencia del signo
    match req.reason_code.as_str() {
        "DANO" | "PERDIDA" | "ROBO" | "VENCIMIENTO" => {
            if req.qty_delta >= Decimal::ZERO {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ADJUST_INVALID",
                    "Este motivo exige una cantidad negativa (salida).",
                    None,
                ));
            }
        }
        "CARGA_INICIAL" => {
            if req.qty_delta <= Decimal::ZERO {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ADJUST_INVALID",
                    "CARGA_INICIAL exige una cantidad positiva.",
                    None,
                ));
            }
            let current_on_hand: Decimal = sqlx::query_scalar(
                "select coalesce(on_hand, 0) from stock where location_id = $1 and product_id = $2",
            )
            .bind(location_id)
            .bind(req.product_id)
            .fetch_optional(&mut **tx)
            .await?
            .unwrap_or(Decimal::ZERO);

            if current_on_hand != Decimal::ZERO {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ADJUST_INVALID",
                    "CARGA_INICIAL solo se permite con existencia actual en cero.",
                    None,
                ));
            }

            let prev_movs: i64 = sqlx::query_scalar(
                "select count(*) from inventory_movements where location_id = $1 and product_id = $2",
            )
            .bind(location_id)
            .bind(req.product_id)
            .fetch_one(&mut **tx)
            .await?;

            if prev_movs > 0 {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ADJUST_INVALID",
                    "CARGA_INICIAL no se permite si el producto ya tiene movimientos previos en esta bodega.",
                    None,
                ));
            }
        }
        _ => {}
    }

    // Validar track_lots
    let (track_lots, cost_price, is_composed): (bool, Option<Decimal>, bool) = sqlx::query_as(
        "select coalesce(track_lots, false), cost_price, is_composed from products where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(req.product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if is_composed {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ADJUST_INVALID",
            "No se puede ajustar directamente un producto compuesto/receta.",
            None,
        ));
    }

    if track_lots {
        let lot_id = match req.lot_id {
            Some(lid) => lid,
            None => {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "LOT_REQUIRED",
                    "Este producto exige lote y vencimiento.",
                    None,
                ));
            }
        };
        let lot_exists: Option<Uuid> = sqlx::query_scalar(
            "select id from lots where id = $1 and tenant_id = $2 and location_id = $3 and product_id = $4",
        )
        .bind(lot_id)
        .bind(scope.tenant)
        .bind(location_id)
        .bind(req.product_id)
        .fetch_optional(&mut **tx)
        .await?;

        if lot_exists.is_none() {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "ADJUST_INVALID",
                "El lote indicado no existe en esta bodega para este producto.",
                None,
            ));
        }
    }

    // Costo y valor
    let stock_avg_cost: Option<Decimal> =
        sqlx::query_scalar("select avg_cost from stock where location_id = $1 and product_id = $2")
            .bind(location_id)
            .bind(req.product_id)
            .fetch_optional(&mut **tx)
            .await?;

    let avg_cost = stock_avg_cost.unwrap_or_else(|| cost_price.unwrap_or(Decimal::ZERO));

    let eff_cost = if req.qty_delta > Decimal::ZERO {
        req.unit_cost.unwrap_or(avg_cost)
    } else {
        avg_cost
    };

    let value = (req.qty_delta.abs() * eff_cost).round_dp(2);

    let settings = crate::settings::load_tx(tx, req.branch_id).await?;
    let threshold = settings.inventory.adjust_approval_threshold;

    let needs_approval = value > threshold && !scope.has_permission("inventory:adjust_approve");

    if needs_approval {
        sqlx::query(
            r#"
            insert into inventory_adjustments (
                id, tenant_id, branch_id, location_id, product_id, lot_id,
                qty_delta, reason_code, notes, unit_cost, value, status,
                requested_by, decided_by, decided_at, reject_reason, client_request, created_at
            ) values (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, 'PENDIENTE',
                $12, null, null, null, $13, now()
            )
            "#,
        )
        .bind(req.id)
        .bind(scope.tenant)
        .bind(req.branch_id)
        .bind(location_id)
        .bind(req.product_id)
        .bind(req.lot_id)
        .bind(req.qty_delta)
        .bind(&req.reason_code)
        .bind(&req.notes)
        .bind(eff_cost)
        .bind(value)
        .bind(scope.user)
        .bind(&req_val)
        .execute(&mut **tx)
        .await?;

        crate::audit::record_tx(
            tx,
            scope,
            "AJUSTE_SOLICITADO",
            "inventory_adjustments",
            req.id,
            None,
            Some(serde_json::json!({
                "branch_id": req.branch_id,
                "product_id": req.product_id,
                "qty_delta": req.qty_delta,
                "reason_code": req.reason_code,
                "value": value,
                "threshold": threshold,
            })),
            None,
        )
        .await?;
    } else {
        let mov = adjustment_type(req.qty_delta);

        apply_delta_tx(
            tx,
            scope,
            StockDelta {
                branch_id: req.branch_id,
                location_id,
                product_id: req.product_id,
                delta: req.qty_delta,
                movement: mov,
                unit_cost: Some(eff_cost),
                lot_id: req.lot_id,
                ref_type: "ADJUST",
                ref_id: req.id,
                cause: req.notes.clone().or_else(|| Some(req.reason_code.clone())),
            },
        )
        .await?;

        sqlx::query(
            r#"
            insert into inventory_adjustments (
                id, tenant_id, branch_id, location_id, product_id, lot_id,
                qty_delta, reason_code, notes, unit_cost, value, status,
                requested_by, decided_by, decided_at, reject_reason, client_request, created_at
            ) values (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, 'APLICADO',
                $12, $12, now(), null, $13, now()
            )
            "#,
        )
        .bind(req.id)
        .bind(scope.tenant)
        .bind(req.branch_id)
        .bind(location_id)
        .bind(req.product_id)
        .bind(req.lot_id)
        .bind(req.qty_delta)
        .bind(&req.reason_code)
        .bind(&req.notes)
        .bind(eff_cost)
        .bind(value)
        .bind(scope.user)
        .bind(&req_val)
        .execute(&mut **tx)
        .await?;

        crate::audit::record_tx(
            tx,
            scope,
            "AJUSTE_APLICADO",
            "inventory_adjustments",
            req.id,
            None,
            Some(serde_json::json!({
                "branch_id": req.branch_id,
                "product_id": req.product_id,
                "qty_delta": req.qty_delta,
                "reason_code": req.reason_code,
                "value": value,
            })),
            None,
        )
        .await?;
    }

    get_adjustment_tx(tx, scope, req.id).await
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn create_adjustment(
    pool: &PgPool,
    claims: &Claims,
    req: CreateAdjustmentRequest,
) -> Result<InventoryAdjustment, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    let adj = create_adjustment_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(adj)
}

pub async fn approve_adjustment_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
) -> Result<InventoryAdjustment, ApiError> {
    scope.require_permission("inventory:adjust_approve")?;

    #[derive(sqlx::FromRow)]
    struct AdjForApprove {
        branch_id: Uuid,
        location_id: Uuid,
        product_id: Uuid,
        lot_id: Option<Uuid>,
        qty_delta: Decimal,
        reason_code: String,
        notes: Option<String>,
        unit_cost: Decimal,
        status: String,
    }

    let adj: AdjForApprove = sqlx::query_as(
        r#"
        select branch_id, location_id, product_id, lot_id, qty_delta, reason_code, notes, unit_cost, status
          from inventory_adjustments
         where tenant_id = $1 and id = $2
           for update
        "#,
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    if adj.status != "PENDIENTE" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "ADJUST_NOT_PENDING",
            "El ajuste ya fue decidido.",
            None,
        ));
    }

    let mov = adjustment_type(adj.qty_delta);

    apply_delta_tx(
        tx,
        scope,
        StockDelta {
            branch_id: adj.branch_id,
            location_id: adj.location_id,
            product_id: adj.product_id,
            delta: adj.qty_delta,
            movement: mov,
            unit_cost: Some(adj.unit_cost),
            lot_id: adj.lot_id,
            ref_type: "ADJUST",
            ref_id: id,
            cause: adj.notes.clone().or_else(|| Some(adj.reason_code.clone())),
        },
    )
    .await?;

    sqlx::query(
        "update inventory_adjustments set status = 'APLICADO', decided_by = $1, decided_at = now() where tenant_id = $2 and id = $3",
    )
    .bind(scope.user)
    .bind(scope.tenant)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    crate::audit::record_tx(
        tx,
        scope,
        "AJUSTE_APLICADO",
        "inventory_adjustments",
        id,
        None,
        Some(serde_json::json!({ "decided_by": scope.user })),
        None,
    )
    .await?;

    get_adjustment_tx(tx, scope, id).await
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn approve_adjustment(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
) -> Result<InventoryAdjustment, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    let adj = approve_adjustment_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(adj)
}

pub async fn reject_adjustment_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    id: Uuid,
    reason: &str,
) -> Result<InventoryAdjustment, ApiError> {
    scope.require_permission("inventory:adjust_approve")?;

    let reason_trim = reason.trim();
    if reason_trim.is_empty() {
        return Err(error::validation("reject reason cannot be empty"));
    }

    let status: Option<String> = sqlx::query_scalar(
        "select status from inventory_adjustments where tenant_id = $1 and id = $2 for update",
    )
    .bind(scope.tenant)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;

    let status = status.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if status != "PENDIENTE" {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "ADJUST_NOT_PENDING",
            "El ajuste ya fue decidido.",
            None,
        ));
    }

    sqlx::query(
        "update inventory_adjustments set status = 'RECHAZADO', decided_by = $1, decided_at = now(), reject_reason = $2 where tenant_id = $3 and id = $4",
    )
    .bind(scope.user)
    .bind(reason_trim)
    .bind(scope.tenant)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    crate::audit::record_tx(
        tx,
        scope,
        "AJUSTE_RECHAZADO",
        "inventory_adjustments",
        id,
        None,
        Some(serde_json::json!({ "decided_by": scope.user, "reject_reason": reason_trim })),
        None,
    )
    .await?;

    get_adjustment_tx(tx, scope, id).await
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn reject_adjustment(
    pool: &PgPool,
    claims: &Claims,
    id: Uuid,
    reason: &str,
) -> Result<InventoryAdjustment, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    let adj = reject_adjustment_tx(&mut tx, &scope, id, reason).await?;
    tx.commit().await?;
    Ok(adj)
}

/// Conteo fisico: fija la existencia en `counted` y registra la diferencia
/// como ajuste de entrada o salida. Sin diferencia no registra nada.
pub async fn count(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Uuid,
    counted: Decimal,
    cause: &str,
) -> Result<Decimal, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;
    if counted < Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "counted must be >= 0".into(),
        )));
    }
    let loc_id = default_location_id_tx(&mut tx, branch_id).await?;
    let on_hand: Decimal = sqlx::query_scalar(
        "select coalesce(on_hand, 0) from stock where location_id = $1 and product_id = $2",
    )
    .bind(loc_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(Decimal::ZERO);

    let delta = counted - on_hand;
    if delta != Decimal::ZERO {
        create_adjustment_tx(
            &mut tx,
            &scope,
            CreateAdjustmentRequest {
                id: Uuid::new_v4(),
                branch_id,
                location_id: Some(loc_id),
                product_id,
                lot_id: None,
                qty_delta: delta,
                reason_code: "CONTEO".to_string(),
                notes: Some(cause.to_string()),
                unit_cost: None,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(delta)
}

/// Bloquea la fila de stock; si el producto nunca tuvo stock en la sucursal,
/// la crea en cero (el producto debe ser del negocio).
pub async fn default_location_id_tx(tx: &mut Tx<'_>, branch_id: Uuid) -> Result<Uuid, ApiError> {
    crate::locations::default_location_id_tx(tx, branch_id).await
}

pub async fn lock_or_create_stock_location_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    location_id: Uuid,
    product_id: Uuid,
) -> Result<(Decimal, Decimal), ApiError> {
    check_not_variant_parent(tx, product_id).await?;
    sqlx::query(
        "insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved)
         select $1, $2, $3, p.id, 0, 0 from products p
          where p.id = $4 and p.tenant_id = $1 and not p.is_composed
         on conflict (location_id, product_id) do nothing",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(location_id)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;

    let row: Option<(Decimal, Decimal)> = sqlx::query_as(
        "select on_hand, reserved from stock
          where location_id = $1 and product_id = $2
          for update",
    )
    .bind(location_id)
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.ok_or(ApiError::Domain(DomainError::NotFound))
}

#[derive(Debug, Clone)]
pub struct StockDelta<'a> {
    pub branch_id: Uuid,
    pub location_id: Uuid,
    pub product_id: Uuid,
    pub delta: Decimal,
    pub movement: &'a str,
    pub unit_cost: Option<Decimal>,
    pub lot_id: Option<Uuid>,
    pub ref_type: &'a str,
    pub ref_id: Uuid,
    pub cause: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Applied {
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub cost_missing: bool,
    pub balance_after: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StockAlert {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
    pub location_id: Uuid,
    pub location_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub kind: String,
    pub qty: Decimal,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

pub async fn apply_delta_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    d: StockDelta<'_>,
) -> Result<Applied, ApiError> {
    // 1. delta == 0: 422 STOCK_DELTA_ZERO
    if d.delta == Decimal::ZERO {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "STOCK_DELTA_ZERO",
            "stock delta cannot be zero",
            None,
        ));
    }

    // 2. Parent with variants: 422 PRODUCT_IS_VARIANT_PARENT. SERVICIO: return 0 cost Applied
    let prod: Option<(bool, String, Option<Decimal>)> = sqlx::query_as(
        "select has_variants, kind, cost_price from products where id = $1 and tenant_id = $2",
    )
    .bind(d.product_id)
    .bind(scope.tenant)
    .fetch_optional(&mut **tx)
    .await?;

    let (has_variants, kind, prod_cost_price) = prod.ok_or(DomainError::NotFound)?;
    if has_variants {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PRODUCT_IS_VARIANT_PARENT",
            "cannot apply stock delta to a product with variants",
            None,
        ));
    }
    if kind == "SERVICIO" {
        return Ok(Applied {
            unit_cost: Decimal::ZERO,
            total_cost: Decimal::ZERO,
            cost_missing: false,
            balance_after: Decimal::ZERO,
        });
    }

    // 3. Lock row: select id, on_hand, reserved, min_qty, avg_cost from stock where location_id = $1 and product_id = $2 for update
    let row: Option<(Uuid, Decimal, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "select id, on_hand, reserved, min_qty, avg_cost from stock
          where location_id = $1 and product_id = $2
          for update",
    )
    .bind(d.location_id)
    .bind(d.product_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (stock_id, on_hand, reserved, min_qty, avg_cost) = match row {
        Some(r) => r,
        None => {
            sqlx::query(
                "insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved, min_qty, avg_cost)
                 values ($1, $2, $3, $4, 0, 0, 0, 0)
                 on conflict (location_id, product_id) do nothing",
            )
            .bind(scope.tenant)
            .bind(d.branch_id)
            .bind(d.location_id)
            .bind(d.product_id)
            .execute(&mut **tx)
            .await?;

            let r: Option<(Uuid, Decimal, Decimal, Decimal, Decimal)> = sqlx::query_as(
                "select id, on_hand, reserved, min_qty, avg_cost from stock
                  where location_id = $1 and product_id = $2
                  for update",
            )
            .bind(d.location_id)
            .bind(d.product_id)
            .fetch_optional(&mut **tx)
            .await?;
            r.ok_or(DomainError::NotFound)?
        }
    };

    let settings = crate::settings::load_tx(tx, d.branch_id).await?;

    // 4. Salida (delta < 0): disponible = on_hand - reserved
    if d.delta < Decimal::ZERO {
        let available = on_hand - reserved;
        let need = d.delta.abs();
        if available < need && !settings.inventory.allow_negative_stock {
            return Err(crate::error::coded(
                StatusCode::CONFLICT,
                "STOCK_INSUFFICIENT",
                format!("insufficient stock for product {}", d.product_id),
                Some(serde_json::json!({
                    "product_id": d.product_id,
                    "available": available,
                })),
            ));
        }
    }

    // 5. Costo (F2.10)
    let mut new_avg_cost = avg_cost;
    let unit_cost: Decimal;
    let total_cost: Decimal;
    let cost_missing: bool;

    if d.delta > Decimal::ZERO {
        // Entrada con unit_cost = c y cantidad q:
        let q = d.delta;
        let c = d.unit_cost.unwrap_or(avg_cost);
        let q0 = on_hand.max(Decimal::ZERO);
        let a0 = avg_cost;
        new_avg_cost = if q0 + q == Decimal::ZERO {
            c
        } else {
            ((q0 * a0 + q * c) / (q0 + q)).round_dp(4)
        };

        sqlx::query(
            "insert into cost_layers (tenant_id, branch_id, location_id, product_id, qty_remaining, unit_cost, source_type, source_id, received_at)
             values ($1, $2, $3, $4, $5, $6, $7, $8, now())",
        )
        .bind(scope.tenant)
        .bind(d.branch_id)
        .bind(d.location_id)
        .bind(d.product_id)
        .bind(q)
        .bind(c)
        .bind(d.movement)
        .bind(d.ref_id)
        .execute(&mut **tx)
        .await?;

        unit_cost = c;
        total_cost = (c * q).round_dp(2);
        cost_missing = d.unit_cost.is_none() && c == Decimal::ZERO;
    } else {
        // Salida de cantidad q:
        let mut q_rem = d.delta.abs();

        #[derive(sqlx::FromRow)]
        struct LayerRow {
            id: Uuid,
            qty_remaining: Decimal,
            unit_cost: Decimal,
        }

        let layers: Vec<LayerRow> = sqlx::query_as(
            "select id, qty_remaining, unit_cost from cost_layers
              where location_id = $1 and product_id = $2 and qty_remaining > 0
              order by received_at asc, id asc
              for update",
        )
        .bind(d.location_id)
        .bind(d.product_id)
        .fetch_all(&mut **tx)
        .await?;

        let mut peps_cost = Decimal::ZERO;
        for layer in layers {
            if q_rem <= Decimal::ZERO {
                break;
            }
            let take = q_rem.min(layer.qty_remaining);
            sqlx::query("update cost_layers set qty_remaining = qty_remaining - $1 where id = $2")
                .bind(take)
                .bind(layer.id)
                .execute(&mut **tx)
                .await?;
            peps_cost += take * layer.unit_cost;
            q_rem -= take;
        }

        let (fallback_cost, is_missing) = if avg_cost > Decimal::ZERO {
            (avg_cost, false)
        } else if let Some(cp) = prod_cost_price.filter(|&c| c > Decimal::ZERO) {
            (cp, false)
        } else {
            (Decimal::ZERO, true)
        };

        if q_rem > Decimal::ZERO {
            peps_cost += q_rem * fallback_cost;
        }

        let q = d.delta.abs();
        if settings.inventory.costing == "PEPS" {
            total_cost = peps_cost.round_dp(2);
            unit_cost = if q > Decimal::ZERO {
                (total_cost / q).round_dp(4)
            } else {
                Decimal::ZERO
            };
            cost_missing = q_rem > Decimal::ZERO && is_missing;
        } else {
            let eff_cost = if avg_cost > Decimal::ZERO {
                avg_cost
            } else {
                fallback_cost
            };
            total_cost = (q * eff_cost).round_dp(2);
            unit_cost = eff_cost;
            cost_missing = avg_cost == Decimal::ZERO && is_missing;
        }
    }

    // 6. update stock set on_hand = on_hand + delta, avg_cost = $2, updated_at = now() where id = $3
    let new_on_hand = on_hand + d.delta;
    sqlx::query(
        "update stock set on_hand = on_hand + $1, avg_cost = $2, updated_at = now() where id = $3",
    )
    .bind(d.delta)
    .bind(new_avg_cost)
    .bind(stock_id)
    .execute(&mut **tx)
    .await?;

    // 7. Insert into inventory_movements
    sqlx::query(
        "insert into inventory_movements
           (tenant_id, branch_id, location_id, product_id, movement_type_id, qty, cause, ref_type, ref_id, created_by, unit_cost, total_cost, balance_after, lot_id)
         values ($1, $2, $3, $4, public.tipo_id('MOV', $5), $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(scope.tenant)
    .bind(d.branch_id)
    .bind(d.location_id)
    .bind(d.product_id)
    .bind(d.movement)
    .bind(d.delta.abs())
    .bind(d.cause.as_deref().unwrap_or(""))
    .bind(d.ref_type)
    .bind(d.ref_id)
    .bind(scope.user)
    .bind(unit_cost)
    .bind(total_cost)
    .bind(new_on_hand)
    .bind(d.lot_id)
    .execute(&mut **tx)
    .await?;

    if let Some(lot_id) = d.lot_id {
        sqlx::query("update lots set qty = qty + $1 where tenant_id = $2 and id = $3")
            .bind(d.delta)
            .bind(scope.tenant)
            .bind(lot_id)
            .execute(&mut **tx)
            .await?;
    }

    // 8. Alertas
    if new_on_hand < Decimal::ZERO {
        sqlx::query(
            "insert into stock_alerts (tenant_id, branch_id, location_id, product_id, kind, qty)
             values ($1, $2, $3, $4, 'NEGATIVO', $5)
             on conflict (location_id, product_id, kind) where resolved_at is null do nothing",
        )
        .bind(scope.tenant)
        .bind(d.branch_id)
        .bind(d.location_id)
        .bind(d.product_id)
        .bind(new_on_hand)
        .execute(&mut **tx)
        .await?;
    }

    if new_on_hand <= min_qty && min_qty > Decimal::ZERO {
        sqlx::query(
            "insert into stock_alerts (tenant_id, branch_id, location_id, product_id, kind, qty)
             values ($1, $2, $3, $4, 'BAJO', $5)
             on conflict (location_id, product_id, kind) where resolved_at is null do nothing",
        )
        .bind(scope.tenant)
        .bind(d.branch_id)
        .bind(d.location_id)
        .bind(d.product_id)
        .bind(new_on_hand)
        .execute(&mut **tx)
        .await?;
    }

    if new_on_hand > min_qty {
        sqlx::query(
            "update stock_alerts set resolved_at = now()
              where location_id = $1 and product_id = $2 and kind = 'BAJO' and resolved_at is null",
        )
        .bind(d.location_id)
        .bind(d.product_id)
        .execute(&mut **tx)
        .await?;
    }

    if new_on_hand >= Decimal::ZERO {
        sqlx::query(
            "update stock_alerts set resolved_at = now()
              where location_id = $1 and product_id = $2 and kind = 'NEGATIVO' and resolved_at is null",
        )
        .bind(d.location_id)
        .bind(d.product_id)
        .execute(&mut **tx)
        .await?;
    }

    // 9. Return Applied
    Ok(Applied {
        unit_cost,
        total_cost,
        cost_missing,
        balance_after: new_on_hand,
    })
}

pub async fn list_alerts(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<StockAlert>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("inventory:read")?;
    let alerts = sqlx::query_as::<_, StockAlert>(
        "select sa.id, sa.tenant_id, sa.branch_id, sa.location_id, l.name as location_name,
                sa.product_id, p.name as product_name, p.sku, sa.kind, sa.qty, sa.created_at, sa.resolved_at
           from stock_alerts sa
           join locations l on l.id = sa.location_id
           join products p on p.id = sa.product_id
          where sa.branch_id = $1 and sa.resolved_at is null
          order by sa.created_at desc",
    )
    .bind(branch_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(alerts)
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpiringLot {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Uuid,
    pub location_id: Uuid,
    pub location_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub sku: Option<String>,
    pub code: String,
    pub expires_at: chrono::NaiveDate,
    pub qty: Decimal,
    pub days_until_expiry: i32,
    pub is_expired: bool,
}

pub async fn list_expiring_lots(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Option<Uuid>,
    days: Option<i64>,
) -> Result<Vec<ExpiringLot>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("inventory:read")?;
    let days_val = days.unwrap_or(30).max(0);

    let rows: Vec<ExpiringLot> = sqlx::query_as(
        r#"
        select l.id, l.tenant_id, l.branch_id, l.location_id, loc.name as location_name,
               l.product_id, p.name as product_name, p.sku,
               l.code, l.expires_at, l.qty,
               (l.expires_at - (now() at time zone b.timezone)::date)::int as days_until_expiry,
               (l.expires_at < (now() at time zone b.timezone)::date) as is_expired
          from lots l
          join locations loc on loc.id = l.location_id
          join products p on p.id = l.product_id
          join branches b on b.id = l.branch_id
         where l.tenant_id = $1
           and ($2::uuid is null or l.branch_id = $2)
           and l.qty > 0
           and l.expires_at <= ((now() at time zone b.timezone)::date + ($3 || ' days')::interval)::date
         order by l.expires_at asc, l.code asc
        "#,
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(days_val.to_string())
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

/// Tipo de movimiento segun la direccion del ajuste.
fn adjustment_type(delta: Decimal) -> &'static str {
    if delta > Decimal::ZERO {
        "AJUSTE_ENTRADA"
    } else {
        "AJUSTE_SALIDA"
    }
}

// -------------------------------------------------------------- traslados

#[derive(Debug, Clone, Deserialize)]
pub struct TransferArgs {
    pub from_branch_id: Option<Uuid>,
    pub to_branch_id: Option<Uuid>,
    pub from_location_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub product_id: Uuid,
    pub qty: Decimal,
    pub cause: String,
    #[serde(default)]
    pub lot_id: Option<Uuid>,
}

pub async fn transfer_locations(
    pool: &PgPool,
    claims: &Claims,
    args: TransferArgs,
) -> Result<Uuid, ApiError> {
    positive(args.qty)?;

    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    crate::db::set_scope(&mut tx, scope.tenant, None).await?;

    let from_loc_id = match args.from_location_id {
        Some(id) => id,
        None => {
            let bid = args.from_branch_id.ok_or_else(|| {
                error::validation("from_location_id or from_branch_id is required")
            })?;
            default_location_id_tx(&mut tx, bid).await?
        }
    };

    let to_loc_id = match args.to_location_id {
        Some(id) => id,
        None => {
            let bid = args
                .to_branch_id
                .ok_or_else(|| error::validation("to_location_id or to_branch_id is required"))?;
            default_location_id_tx(&mut tx, bid).await?
        }
    };

    if from_loc_id == to_loc_id {
        return Err(ApiError::Domain(DomainError::Validation(
            "source and destination locations must differ".into(),
        )));
    }

    #[derive(sqlx::FromRow)]
    struct LocInfo {
        #[allow(dead_code)]
        id: Uuid,
        branch_id: Uuid,
    }

    let from_info: LocInfo =
        sqlx::query_as("select id, branch_id from locations where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(from_loc_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                error::coded(
                    StatusCode::NOT_FOUND,
                    "LOCATION_NOT_FOUND",
                    "Ubicación de origen no encontrada",
                    None,
                )
            })?;

    let to_info: LocInfo =
        sqlx::query_as("select id, branch_id from locations where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(to_loc_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                error::coded(
                    StatusCode::NOT_FOUND,
                    "LOCATION_NOT_FOUND",
                    "Ubicación de destino no encontrada",
                    None,
                )
            })?;

    tenancy::require_in_branch_tx(
        &mut tx,
        &scope,
        from_info.branch_id,
        Action::ManageInventory,
    )
    .await?;
    if to_info.branch_id != from_info.branch_id {
        tenancy::require_in_branch_tx(&mut tx, &scope, to_info.branch_id, Action::ManageInventory)
            .await?;
    }

    let track_lots: bool =
        sqlx::query_scalar("select coalesce(track_lots, false) from products where id = $1")
            .bind(args.product_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);

    if track_lots && args.lot_id.is_none() {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LOT_REQUIRED",
            "Este producto exige lote y vencimiento.",
            None,
        ));
    }

    let (from_lot_id, to_lot_id) = if let Some(lid) = args.lot_id {
        #[derive(sqlx::FromRow)]
        struct LotInfo {
            code: String,
            expires_at: chrono::NaiveDate,
        }
        let src_lot: Option<LotInfo> = sqlx::query_as(
            "select code, expires_at from lots where id = $1 and location_id = $2 and tenant_id = $3",
        )
        .bind(lid)
        .bind(from_loc_id)
        .bind(scope.tenant)
        .fetch_optional(&mut *tx)
        .await?;

        let src_lot = src_lot.ok_or_else(|| {
            crate::error::coded(
                StatusCode::NOT_FOUND,
                "LOT_NOT_FOUND",
                "Lote de origen no encontrado",
                None,
            )
        })?;

        let dest_lot: Option<(Uuid, chrono::NaiveDate)> = sqlx::query_as(
            "select id, expires_at from lots where location_id = $1 and product_id = $2 and code = $3",
        )
        .bind(to_loc_id)
        .bind(args.product_id)
        .bind(&src_lot.code)
        .fetch_optional(&mut *tx)
        .await?;

        let dlid = match dest_lot {
            Some((did, dexp)) => {
                if dexp != src_lot.expires_at {
                    return Err(crate::error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "LOT_EXPIRY_MISMATCH",
                        "Ese lote ya existe con otra fecha de vencimiento en destino.",
                        None,
                    ));
                }
                did
            }
            None => {
                sqlx::query_scalar(
                    r#"
                    insert into lots (tenant_id, branch_id, location_id, product_id, code, expires_at, qty)
                    values ($1, $2, $3, $4, $5, $6, 0)
                    returning id
                    "#,
                )
                .bind(scope.tenant)
                .bind(to_info.branch_id)
                .bind(to_loc_id)
                .bind(args.product_id)
                .bind(&src_lot.code)
                .bind(src_lot.expires_at)
                .fetch_one(&mut *tx)
                .await?
            }
        };
        (Some(lid), Some(dlid))
    } else {
        (None, None)
    };

    let wide = Scope {
        branch: None,
        ..scope.clone()
    };

    let reference = Uuid::new_v4();
    let from_applied = apply_delta_tx(
        &mut tx,
        &wide,
        StockDelta {
            branch_id: from_info.branch_id,
            location_id: from_loc_id,
            product_id: args.product_id,
            delta: -args.qty,
            movement: "TRASLADO_SALIDA",
            unit_cost: None,
            lot_id: from_lot_id,
            ref_type: "transfer",
            ref_id: reference,
            cause: Some(args.cause.clone()),
        },
    )
    .await?;

    apply_delta_tx(
        &mut tx,
        &wide,
        StockDelta {
            branch_id: to_info.branch_id,
            location_id: to_loc_id,
            product_id: args.product_id,
            delta: args.qty,
            movement: "TRASLADO_ENTRADA",
            unit_cost: Some(from_applied.unit_cost),
            lot_id: to_lot_id,
            ref_type: "transfer",
            ref_id: reference,
            cause: Some(args.cause),
        },
    )
    .await?;

    tx.commit().await?;
    Ok(reference)
}

/// Traslada unidades de una sucursal a otra (o entre bodegas) en una sola transaccion.
#[allow(dead_code)]
pub async fn transfer(
    pool: &PgPool,
    claims: &Claims,
    from: Uuid,
    to: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
) -> Result<Uuid, ApiError> {
    transfer_locations(
        pool,
        claims,
        TransferArgs {
            from_branch_id: Some(from),
            to_branch_id: Some(to),
            from_location_id: None,
            to_location_id: None,
            product_id,
            qty,
            cause: cause.to_string(),
            lot_id: None,
        },
    )
    .await
}

// ------------------------------------------------------------- existencias

/// Existencias de la sucursal: productos del surtido o con stock, con su
/// estado frente al punto de reorden. Los compuestos (recetas) no llevan stock.
pub async fn stock_levels(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    location_id: Option<Uuid>,
) -> Result<Vec<StockLevel>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;
    let rows = sqlx::query_as::<_, StockLevel>(
        "select p.id as product_id, p.name, p.sku, p.barcode::text as barcode, c.name as category,
                u.tima_nombre as unit, p.kind, p.usage, bp.price, p.cost_price,
                coalesce(s.on_hand, 0) as on_hand,
                coalesce(s.reserved, 0) as reserved,
                coalesce(s.on_hand, 0) - coalesce(s.reserved, 0) as available,
                coalesce(s.min_qty, 0) as min_qty,
                case
                  when coalesce(s.on_hand, 0) - coalesce(s.reserved, 0) <= 0 then 'AGOTADO'
                  when coalesce(s.on_hand, 0) - coalesce(s.reserved, 0) <= coalesce(s.min_qty, 0) then 'BAJO'
                  else 'OK'
                end as status,
                s.updated_at,
                (p.archived_at is not null) as archived
           from products p
           left join branch_products bp on bp.product_id = p.id and bp.branch_id = $1
           left join (
             select product_id, sum(on_hand) as on_hand, sum(reserved) as reserved,
                    max(min_qty) as min_qty, max(updated_at) as updated_at
               from stock
              where branch_id = $1 and ($2::uuid is null or location_id = $2)
              group by product_id
           ) s on s.product_id = p.id
           left join product_categories c on c.id = bp.category_id
           left join tbl_tipos_maestra u on u.tima_id = p.unit_measure_id
          where p.active and not p.is_composed
            and (bp.id is not null or s.product_id is not null)
          order by p.name",
    )
    .bind(branch_id)
    .bind(location_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// Fija el punto de reorden de un producto en la sucursal.
pub async fn set_min_qty(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Uuid,
    min_qty: Decimal,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;
    if min_qty < Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "min_qty must be >= 0".into(),
        )));
    }
    let loc_id = default_location_id_tx(&mut tx, branch_id).await?;
    lock_or_create_stock_location_tx(&mut tx, &scope, branch_id, loc_id, product_id).await?;
    sqlx::query("update stock set min_qty = $3 where location_id = $1 and product_id = $2")
        .bind(loc_id)
        .bind(product_id)
        .bind(min_qty)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ------------------------------------------------------------------- kardex

/// Bitacora de movimientos de una sucursal (todos los productos).
#[cfg_attr(not(test), allow(dead_code))]
pub async fn movements(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<Movement>, ApiError> {
    kardex(pool, claims, branch_id, None, None, 500).await
}

/// Kardex: movimientos mas recientes primero, opcionalmente de un producto,
/// con la existencia resultante despues de cada uno.
pub async fn kardex(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    product_id: Option<Uuid>,
    location_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Movement>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;
    let rows = sqlx::query_as::<_, Movement>(
        "with m as (
           select im.id, im.product_id, t.tima_codigo as movement_type, im.qty,
                  case
                    when t.tima_codigo in ('REPOSICION', 'AJUSTE_ENTRADA', 'TRASLADO_ENTRADA', 'DEVOLUCION') then im.qty
                    when t.tima_codigo in ('DESCUENTO', 'AJUSTE_SALIDA', 'TRASLADO_SALIDA') then -im.qty
                  end as delta,
                  im.cause, im.ref_type, im.ref_id, im.created_by, im.branch_id, im.created_at
             from inventory_movements im
             join tbl_tipos_maestra t on t.tima_id = im.movement_type_id
            where im.branch_id = $1
              and ($2::uuid is null or im.product_id = $2)
              and ($3::uuid is null or im.location_id = $3)
         )
         select m.id, m.product_id, p.name as product_name, p.sku, m.movement_type, m.qty, m.delta,
                coalesce(s.on_hand, 0) - coalesce(sum(coalesce(m.delta, 0)) over (
                  partition by m.product_id order by m.created_at desc, m.id desc
                  rows between unbounded preceding and 1 preceding), 0) as balance_after,
                m.cause, m.ref_type, m.ref_id, u.full_name as user_name, m.branch_id, m.created_at
           from m
           join products p on p.id = m.product_id
           left join (
             select product_id, sum(on_hand) as on_hand
               from stock
              where branch_id = $1 and ($3::uuid is null or location_id = $3)
              group by product_id
           ) s on s.product_id = m.product_id
           left join users u on u.id = m.created_by
          order by m.created_at desc, m.id desc
          limit $4",
    )
    .bind(branch_id)
    .bind(product_id)
    .bind(location_id)
    .bind(limit.clamp(1, 1000))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjustment_type_follows_sign() {
        assert_eq!(adjustment_type(Decimal::from(3)), "AJUSTE_ENTRADA");
        assert_eq!(adjustment_type(Decimal::from(-2)), "AJUSTE_SALIDA");
    }
}

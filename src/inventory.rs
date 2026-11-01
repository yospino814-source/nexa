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
                set on_hand = on_hand - $1, reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "insert into inventory_movements
               (tenant_id, branch_id, location_id, product_id, movement_type_id, qty, cause, ref_type, ref_id, created_by)
             values ($1, $2, $3, $4, public.tipo_id('MOV', 'DESCUENTO'), $5, 'consumo', 'reservation', $6, $7)",
        )
        .bind(scope.tenant)
        .bind(branch_id)
        .bind(location_id)
        .bind(product_id)
        .bind(qty)
        .bind(id)
        .bind(scope.user)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "update stock_reservations set status_id = public.tipo_id('RES', 'CONFIRMADA'), updated_at = now() where id = $1",
        )
        .bind(id)
        .execute(&mut **tx)
        .await?;
        count += 1;
    }
    Ok(count)
}

// ------------------------------------------------------------ venta directa

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

    expire_due_tx(tx).await?;

    for (product_id, need) in needs {
        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "select on_hand, reserved from stock
              where location_id = $1 and product_id = $2
              for update",
        )
        .bind(location_id)
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?;
        let (on_hand, reserved) = row.unwrap_or((Decimal::ZERO, Decimal::ZERO));
        if on_hand - reserved < need {
            return Err(crate::error::state(format!(
                "insufficient stock for product {product_id}"
            )));
        }

        sqlx::query(
            "update stock set on_hand = on_hand - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(need)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "insert into inventory_movements
               (tenant_id, branch_id, location_id, product_id, movement_type_id, qty, cause, ref_type, ref_id, created_by)
             values ($1, $2, $3, $4, public.tipo_id('MOV', 'DESCUENTO'), $5, 'venta', 'order', $6, $7)",
        )
        .bind(scope.tenant)
        .bind(branch_id)
        .bind(location_id)
        .bind(product_id)
        .bind(need)
        .bind(order_id)
        .bind(scope.user)
        .execute(&mut **tx)
        .await?;
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
    replenish_ref_tx(tx, scope, branch_id, product_id, qty, cause, None).await
}

pub async fn replenish_ref_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
    reference: Option<(&str, Uuid)>,
) -> Result<(), ApiError> {
    scope.require(Action::ManageInventory)?;
    tenancy::ensure_branch_tx(tx, scope, branch_id).await?;
    positive(qty)?;
    check_not_variant_parent(tx, product_id).await?;

    let loc_id = crate::locations::default_location_id_tx(tx, branch_id).await?;

    sqlx::query(
        "insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved)
         values ($1, $2, $3, $4, $5, 0)
         on conflict (location_id, product_id)
         do update set on_hand = stock.on_hand + excluded.on_hand, updated_at = now()",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(loc_id)
    .bind(product_id)
    .bind(qty)
    .execute(&mut **tx)
    .await?;

    record_movement_ref(
        tx,
        scope,
        branch_id,
        loc_id,
        product_id,
        "REPOSICION",
        qty,
        cause,
        reference,
    )
    .await
}

/// Reingresa al inventario lo devuelto por un cliente (devolución o anulación).
///
/// No exige permiso de inventario: el permiso lo valida quien llama
/// (`pos:sell` o `pos:refund`). Un producto con receta devuelve sus insumos,
/// igual que `sell_tx` los descontó. El movimiento es de tipo `DEVOLUCION`.
pub async fn return_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
    reference: (&str, Uuid),
) -> Result<(), ApiError> {
    positive(qty)?;
    let loc_id = crate::locations::default_location_id_tx(tx, branch_id).await?;
    for (component, need) in components(tx, product_id, qty).await? {
        sqlx::query(
            "insert into stock (tenant_id, branch_id, location_id, product_id, on_hand, reserved)
             values ($1, $2, $3, $4, $5, 0)
             on conflict (location_id, product_id)
             do update set on_hand = stock.on_hand + excluded.on_hand, updated_at = now()",
        )
        .bind(scope.tenant)
        .bind(branch_id)
        .bind(loc_id)
        .bind(component)
        .bind(need)
        .execute(&mut **tx)
        .await?;
        record_movement_ref(
            tx,
            scope,
            branch_id,
            loc_id,
            component,
            "DEVOLUCION",
            need,
            cause,
            Some(reference),
        )
        .await?;
    }
    Ok(())
}

/// Ajusta inventario (delta con signo) y registra el movimiento.
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

pub async fn adjust_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    delta: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    scope.require(Action::ManageInventory)?;
    tenancy::ensure_branch_tx(tx, scope, branch_id).await?;
    if delta == Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "delta must be non-zero".into(),
        )));
    }
    check_not_variant_parent(tx, product_id).await?;

    let (on_hand, reserved) = lock_or_create_stock_tx(tx, scope, branch_id, product_id).await?;
    apply_delta_tx(
        tx, scope, branch_id, product_id, on_hand, reserved, delta, cause,
    )
    .await?;

    crate::audit::record_tx(
        tx,
        scope,
        "AJUSTE_INVENTARIO",
        "stock",
        product_id,
        Some(serde_json::json!({ "on_hand": on_hand })),
        Some(serde_json::json!({ "delta": delta, "new_on_hand": on_hand + delta, "cause": cause })),
        None,
    )
    .await
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
    let (on_hand, reserved) =
        lock_or_create_stock_tx(&mut tx, &scope, branch_id, product_id).await?;
    let delta = counted - on_hand;
    if delta != Decimal::ZERO {
        apply_delta_tx(
            &mut tx, &scope, branch_id, product_id, on_hand, reserved, delta, cause,
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

async fn lock_or_create_stock_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
) -> Result<(Decimal, Decimal), ApiError> {
    let loc_id = default_location_id_tx(tx, branch_id).await?;
    lock_or_create_stock_location_tx(tx, scope, branch_id, loc_id, product_id).await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_delta_ref_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    on_hand: Decimal,
    reserved: Decimal,
    delta: Decimal,
    cause: &str,
    reference: Option<(&str, Uuid)>,
) -> Result<(), ApiError> {
    let loc_id = default_location_id_tx(tx, branch_id).await?;
    apply_delta_ref_location_tx(
        tx, scope, branch_id, loc_id, product_id, on_hand, reserved, delta, cause, reference,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_delta_ref_location_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    location_id: Uuid,
    product_id: Uuid,
    on_hand: Decimal,
    reserved: Decimal,
    delta: Decimal,
    cause: &str,
    reference: Option<(&str, Uuid)>,
) -> Result<(), ApiError> {
    let settings = crate::settings::load_tx(tx, branch_id).await?;
    let new_on_hand = on_hand + delta;
    if (!settings.inventory.allow_negative_stock && new_on_hand < Decimal::ZERO)
        || new_on_hand < reserved
    {
        return Err(ApiError::Domain(DomainError::Validation(
            "adjustment violates stock".into(),
        )));
    }

    sqlx::query(
        "update stock set on_hand = $1, updated_at = now()
          where location_id = $2 and product_id = $3",
    )
    .bind(new_on_hand)
    .bind(location_id)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;

    let kind = adjustment_type(delta);
    record_movement_ref(
        tx,
        scope,
        branch_id,
        location_id,
        product_id,
        kind,
        delta.abs(),
        cause,
        reference,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn apply_delta_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    on_hand: Decimal,
    reserved: Decimal,
    delta: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    apply_delta_ref_tx(
        tx, scope, branch_id, product_id, on_hand, reserved, delta, cause, None,
    )
    .await
}

/// Tipo de movimiento segun la direccion del ajuste.
fn adjustment_type(delta: Decimal) -> &'static str {
    if delta > Decimal::ZERO {
        "AJUSTE_ENTRADA"
    } else {
        "AJUSTE_SALIDA"
    }
}

#[allow(clippy::too_many_arguments)]
async fn record_movement_ref(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    location_id: Uuid,
    product_id: Uuid,
    kind: &str,
    qty: Decimal,
    cause: &str,
    reference: Option<(&str, Uuid)>,
) -> Result<(), ApiError> {
    sqlx::query(
        "insert into inventory_movements
           (tenant_id, branch_id, location_id, product_id, movement_type_id, qty, cause, ref_type, ref_id, created_by)
         values ($1, $2, $3, $4, public.tipo_id('MOV', $5), $6, $7, $8, $9, $10)",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(location_id)
    .bind(product_id)
    .bind(kind)
    .bind(qty)
    .bind(cause)
    .bind(reference.map(|(t, _)| t))
    .bind(reference.map(|(_, id)| id))
    .bind(scope.user)
    .execute(&mut **tx)
    .await?;
    Ok(())
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

    crate::db::set_scope(&mut tx, scope.tenant, None).await?;
    let wide = Scope {
        branch: None,
        ..scope.clone()
    };

    let (first_loc, second_loc, first_bid, second_bid) = if from_loc_id < to_loc_id {
        (
            from_loc_id,
            to_loc_id,
            from_info.branch_id,
            to_info.branch_id,
        )
    } else {
        (
            to_loc_id,
            from_loc_id,
            to_info.branch_id,
            from_info.branch_id,
        )
    };

    let a = lock_or_create_stock_location_tx(&mut tx, &wide, first_bid, first_loc, args.product_id)
        .await?;
    let b =
        lock_or_create_stock_location_tx(&mut tx, &wide, second_bid, second_loc, args.product_id)
            .await?;
    let (on_hand, reserved) = if first_loc == from_loc_id { a } else { b };

    if on_hand - reserved < args.qty {
        return Err(crate::error::state("insufficient stock for transfer"));
    }

    sqlx::query(
        "update stock set on_hand = on_hand - $1, updated_at = now()
         where location_id = $2 and product_id = $3",
    )
    .bind(args.qty)
    .bind(from_loc_id)
    .bind(args.product_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "update stock set on_hand = on_hand + $1, updated_at = now()
         where location_id = $2 and product_id = $3",
    )
    .bind(args.qty)
    .bind(to_loc_id)
    .bind(args.product_id)
    .execute(&mut *tx)
    .await?;

    let reference = Uuid::new_v4();
    let r = Some(("TRASLADO", reference));
    record_movement_ref(
        &mut tx,
        &wide,
        from_info.branch_id,
        from_loc_id,
        args.product_id,
        "TRASLADO_SALIDA",
        args.qty,
        &args.cause,
        r,
    )
    .await?;

    record_movement_ref(
        &mut tx,
        &wide,
        to_info.branch_id,
        to_loc_id,
        args.product_id,
        "TRASLADO_ENTRADA",
        args.qty,
        &args.cause,
        r,
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
    if from == to {
        return Err(ApiError::Domain(DomainError::Validation(
            "source and destination branches must differ".into(),
        )));
    }
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

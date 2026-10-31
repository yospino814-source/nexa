//! Ordenes: creacion, lineas, maquina de estados y cancelacion.
//!
//! Igual que en `inventory`, cada operacion tiene su forma `*_tx` (componible
//! dentro de una transaccion abierta con [`tenancy::begin`]) y una envoltura
//! que abre y confirma su propia transaccion.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError};
use crate::tenancy::{self, Action, Scope};
use crate::{catalog, inventory};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Creada,
    EnPreparacion,
    Lista,
    Entregada,
    Cobrada,
    Cerrada,
    Cancelada,
}

impl OrderStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            OrderStatus::Creada => "CREADA",
            OrderStatus::EnPreparacion => "EN_PREPARACION",
            OrderStatus::Lista => "LISTA",
            OrderStatus::Entregada => "ENTREGADA",
            OrderStatus::Cobrada => "COBRADA",
            OrderStatus::Cerrada => "CERRADA",
            OrderStatus::Cancelada => "CANCELADA",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        Some(match value {
            "CREADA" => OrderStatus::Creada,
            "EN_PREPARACION" => OrderStatus::EnPreparacion,
            "LISTA" => OrderStatus::Lista,
            "ENTREGADA" => OrderStatus::Entregada,
            "COBRADA" => OrderStatus::Cobrada,
            "CERRADA" => OrderStatus::Cerrada,
            "CANCELADA" => OrderStatus::Cancelada,
            _ => return None,
        })
    }
}

fn can_transition(from: OrderStatus, to: OrderStatus) -> bool {
    use OrderStatus::*;
    matches!(
        (from, to),
        (Creada, EnPreparacion)
            | (EnPreparacion, Lista)
            | (Lista, Entregada)
            | (Entregada, Cobrada)
            | (Cobrada, Cerrada)
    ) || (to == Cancelada && matches!(from, Creada | EnPreparacion | Lista | Entregada))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub sale_type: String,
    pub status: String,
    pub table_id: Option<Uuid>,
    pub branch_id: Uuid,
    pub waiter_id: Option<Uuid>,
    /// Suma de `line_total` (precio sin impuesto).
    pub subtotal: Decimal,
    /// Suma del impuesto de las lineas (IVA o INC segun el producto).
    pub inc_amount: Decimal,
    pub tip_amount: Decimal,
    /// `subtotal + inc_amount + tip_amount`.
    pub total_amount: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub line_total: Decimal,
    /// Tasa congelada al agregar la linea (0.19 = 19%).
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub station_id: Option<i32>,
    pub status_id: Option<i32>,
}

const ORDER_COLS: &str = "o.id, st.tima_codigo as sale_type, os.tima_codigo as status, \
     o.table_id, o.branch_id, o.waiter_id, o.subtotal, o.inc_amount, o.tip_amount, o.total_amount";
const ORDER_FROM: &str = "orders o \
     join tbl_tipos_maestra st on st.tima_id = o.sale_type_id \
     join tbl_tipos_maestra os on os.tima_id = o.status_id";
pub(crate) const ITEM_COLS: &str =
    "id, product_id, qty, unit_price, line_total, tax_rate, tax_amount, station_id, status_id";

/// Estado, sucursal y mesa de una orden, bloqueando su fila hasta el commit.
/// Serializa las operaciones concurrentes sobre la misma orden.
async fn lock_order(
    tx: &mut Tx<'_>,
    order_id: Uuid,
) -> Result<(OrderStatus, Uuid, Option<Uuid>), ApiError> {
    let row: Option<(String, Uuid, Option<Uuid>)> = sqlx::query_as(
        "select os.tima_codigo, o.branch_id, o.table_id
           from orders o
           join tbl_tipos_maestra os on os.tima_id = o.status_id
          where o.id = $1
          for update of o",
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (status, branch_id, table_id) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    let status = OrderStatus::from_db(&status)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("unknown order status {status}")))?;
    Ok((status, branch_id, table_id))
}

async fn set_status(tx: &mut Tx<'_>, order_id: Uuid, to: OrderStatus) -> Result<(), ApiError> {
    sqlx::query(
        "update orders set
            status_id = public.tipo_id('ORD', $2::text),
            closed_at = case when $2::text = 'CERRADA' then now() else closed_at end,
            cancelled_at = case when $2::text = 'CANCELADA' then now() else cancelled_at end
          where id = $1",
    )
    .bind(order_id)
    .bind(to.as_db())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn free_table(tx: &mut Tx<'_>, table_id: Option<Uuid>) -> Result<(), ApiError> {
    if let Some(table) = table_id {
        sqlx::query(
            "update restaurant_tables set status_id = public.tipo_id('MESA', 'LIBRE') where id = $1",
        )
        .bind(table)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Recalcula subtotal, impuesto y total de la orden a partir de sus lineas.
pub(crate) async fn recalc_totals(tx: &mut Tx<'_>, order_id: Uuid) -> Result<(), ApiError> {
    sqlx::query(
        "update orders o
            set subtotal = s.subtotal,
                inc_amount = s.tax,
                total_amount = s.subtotal + s.tax + o.tip_amount
           from (select coalesce(sum(line_total), 0) as subtotal,
                        coalesce(sum(tax_amount), 0) as tax
                   from order_items where order_id = $1) s
          where o.id = $1",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Precio vigente (sin impuesto) y tasa de impuesto de un producto vendible
/// en la sucursal. Falla con 422 si no esta en el surtido o esta inactivo.
pub(crate) async fn sellable_price(
    tx: &mut Tx<'_>,
    branch_id: Uuid,
    product_id: Uuid,
) -> Result<(Decimal, Decimal), ApiError> {
    let row: Option<(Option<Decimal>, Option<String>)> = sqlx::query_as(
        "select coalesce(bp.price, p.base_price), tx.tima_codigo
           from branch_products bp
           join products p on p.id = bp.product_id
           left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
          where bp.branch_id = $1 and bp.product_id = $2
            and bp.is_available and p.active",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?;
    let not_sellable = || {
        ApiError::Domain(DomainError::Validation(format!(
            "product {product_id} is not in the branch assortment"
        )))
    };
    let (price, tax_code) = row.ok_or_else(not_sellable)?;
    let price = price.ok_or_else(not_sellable)?;
    let tax_rate = catalog::tax_rate_for(tax_code.as_deref()).unwrap_or(Decimal::ZERO);
    Ok((price, tax_rate))
}

// ------------------------------------------------------------------ creacion

/// Crea una orden. El tipo de venta `MESA` exige mesa libre (se ocupa en la misma transaccion).
pub async fn create_order(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    sale_type: &str,
    table_id: Option<Uuid>,
    waiter_id: Option<Uuid>,
    notes: Option<String>,
) -> Result<Order, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    let order = create_order_tx(
        &mut tx, &scope, branch_id, sale_type, table_id, waiter_id, notes,
    )
    .await?;
    tx.commit().await?;
    Ok(order)
}

pub async fn create_order_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    sale_type: &str,
    table_id: Option<Uuid>,
    waiter_id: Option<Uuid>,
    notes: Option<String>,
) -> Result<Order, ApiError> {
    scope.require(Action::TakeOrder)?;
    tenancy::ensure_branch_tx(tx, scope, branch_id).await?;

    let sale_type_id: Option<i32> = sqlx::query_scalar("select public.tipo_id('VENTA', $1)")
        .bind(sale_type)
        .fetch_one(&mut **tx)
        .await?;
    let sale_type_id = sale_type_id.ok_or(ApiError::Domain(DomainError::Validation(
        "invalid sale type".into(),
    )))?;

    match (sale_type, table_id) {
        ("MESA", Some(table)) => {
            let free: Option<bool> = sqlx::query_scalar(
                "select (status_id = public.tipo_id('MESA', 'LIBRE'))
                   from restaurant_tables
                  where id = $1 and branch_id = $2
                  for update",
            )
            .bind(table)
            .bind(branch_id)
            .fetch_optional(&mut **tx)
            .await?;
            match free {
                Some(true) => {
                    sqlx::query(
                        "update restaurant_tables set status_id = public.tipo_id('MESA', 'OCUPADA') where id = $1",
                    )
                    .bind(table)
                    .execute(&mut **tx)
                    .await?;
                }
                Some(false) => return Err(ApiError::Domain(DomainError::Conflict)),
                None => return Err(ApiError::Domain(DomainError::NotFound)),
            }
        }
        ("MESA", None) => {
            return Err(ApiError::Domain(DomainError::Validation(
                "MESA requires a table".into(),
            )));
        }
        (_, Some(_)) => {
            return Err(ApiError::Domain(DomainError::Validation(
                "table is only allowed for MESA".into(),
            )));
        }
        (_, None) => {}
    }

    let order = sqlx::query_as::<_, Order>(
        "insert into orders
           (tenant_id, branch_id, table_id, sale_type_id, status_id, waiter_id, notes)
         values ($1, $2, $3, $4, public.tipo_id('ORD', 'CREADA'), $5, $6)
         returning id,
           (select tima_codigo from tbl_tipos_maestra where tima_id = sale_type_id) as sale_type,
           (select tima_codigo from tbl_tipos_maestra where tima_id = status_id) as status,
           table_id, branch_id, waiter_id, subtotal, inc_amount, tip_amount, total_amount",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(table_id)
    .bind(sale_type_id)
    .bind(waiter_id)
    .bind(notes)
    .fetch_one(&mut **tx)
    .await?;

    Ok(order)
}

// -------------------------------------------------------------------- lineas

/// Agrega una linea con precio e impuesto vigentes (snapshot) y recalcula totales.
pub async fn add_line(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    notes: Option<String>,
) -> Result<OrderItem, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    let line = add_line_tx(&mut tx, &scope, order_id, product_id, qty, notes).await?;
    tx.commit().await?;
    Ok(line)
}

pub async fn add_line_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    order_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    notes: Option<String>,
) -> Result<OrderItem, ApiError> {
    scope.require(Action::TakeOrder)?;
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }

    let (status, branch_id, _table) = lock_order(tx, order_id).await?;
    if status != OrderStatus::Creada {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let (price, tax_rate) = sellable_price(tx, branch_id, product_id).await?;

    let line = sqlx::query_as::<_, OrderItem>(&format!(
        "insert into order_items
           (tenant_id, branch_id, order_id, product_id, qty, unit_price, tax_rate, notes)
         values ($1, $2, $3, $4, $5, $6, $7, $8)
         returning {ITEM_COLS}"
    ))
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(order_id)
    .bind(product_id)
    .bind(qty)
    .bind(price)
    .bind(tax_rate)
    .bind(notes)
    .fetch_one(&mut **tx)
    .await?;

    recalc_totals(tx, order_id).await?;
    Ok(line)
}

// ------------------------------------------------------------------ lecturas

/// Lee una orden del tenant activo.
pub async fn order(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<Order, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Read)?;
    let found = order_tx(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok(found)
}

pub async fn order_tx(tx: &mut Tx<'_>, order_id: Uuid) -> Result<Order, ApiError> {
    let found = sqlx::query_as::<_, Order>(&format!(
        "select {ORDER_COLS} from {ORDER_FROM} where o.id = $1"
    ))
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Lee una linea del tenant activo.
#[cfg_attr(not(test), allow(dead_code))] // API usada hoy solo por tests.
pub async fn line(pool: &PgPool, claims: &Claims, line_id: Uuid) -> Result<OrderItem, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Read)?;
    let found = sqlx::query_as::<_, OrderItem>(&format!(
        "select {ITEM_COLS} from order_items where id = $1"
    ))
    .bind(line_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

// -------------------------------------------------------------- confirmacion

/// Confirma la orden: exige lineas, reserva inventario y pasa a `EN_PREPARACION`.
///
/// Todo ocurre en una transaccion: si una linea no tiene stock, no queda
/// ninguna reserva y la orden sigue `CREADA`.
pub async fn confirm(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    confirm_tx(&mut tx, &scope, order_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn confirm_tx(tx: &mut Tx<'_>, scope: &Scope, order_id: Uuid) -> Result<(), ApiError> {
    scope.require(Action::TakeOrder)?;
    // El bloqueo de la orden impide confirmaciones dobles y lineas agregadas a mitad.
    let (status, branch_id, _table) = lock_order(tx, order_id).await?;
    if status != OrderStatus::Creada {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let lines: Vec<(Uuid, Uuid, Decimal)> =
        sqlx::query_as("select id, product_id, qty from order_items where order_id = $1")
            .bind(order_id)
            .fetch_all(&mut **tx)
            .await?;
    if lines.is_empty() {
        return Err(ApiError::Domain(DomainError::Validation(
            "order has no lines".into(),
        )));
    }

    // Bloquea de una vez, en orden estable, todo el stock que tocara la orden.
    let mut stock_products = Vec::new();
    for (_line, product_id, qty) in &lines {
        for (component, _need) in inventory::components(tx, *product_id, *qty).await? {
            stock_products.push(component);
        }
    }
    stock_products.sort();
    stock_products.dedup();
    inventory::lock_stock_tx(tx, branch_id, &stock_products).await?;

    for (line_id, product_id, qty) in lines {
        inventory::reserve_tx(
            tx,
            scope,
            branch_id,
            Some(order_id),
            Some(line_id),
            product_id,
            qty,
            None,
        )
        .await?;
    }

    set_status(tx, order_id, OrderStatus::EnPreparacion).await
}

// ---------------------------------------------------------- estados y cierre

/// Avanza el estado de la orden segun la maquina de estados.
///
/// - `EN_PREPARACION` equivale a [`confirm`] (reserva inventario).
/// - `COBRADA` consume las reservas: el stock se descuenta definitivamente.
/// - `CERRADA` libera la mesa.
pub async fn transition(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    to: OrderStatus,
) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    transition_tx(&mut tx, &scope, order_id, to).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn transition_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    order_id: Uuid,
    to: OrderStatus,
) -> Result<(), ApiError> {
    match to {
        OrderStatus::Cancelada => {
            return Err(ApiError::Domain(DomainError::Validation(
                "use cancel for cancellation".into(),
            )));
        }
        OrderStatus::EnPreparacion => return confirm_tx(tx, scope, order_id).await,
        OrderStatus::Lista => scope.require(Action::Kitchen)?,
        OrderStatus::Entregada => scope.require(Action::TakeOrder)?,
        OrderStatus::Cobrada | OrderStatus::Cerrada => scope.require(Action::Charge)?,
        OrderStatus::Creada => {}
    }

    let (from, _branch, table_id) = lock_order(tx, order_id).await?;
    if !can_transition(from, to) {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    match to {
        OrderStatus::Cobrada => {
            inventory::consume_order_tx(tx, scope, order_id).await?;
        }
        OrderStatus::Cerrada => free_table(tx, table_id).await?,
        _ => {}
    }

    set_status(tx, order_id, to).await
}

/// Cancela la orden (antes de cobrada): libera reservas y libera la mesa.
pub async fn cancel(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    cancel_tx(&mut tx, &scope, order_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn cancel_tx(tx: &mut Tx<'_>, scope: &Scope, order_id: Uuid) -> Result<(), ApiError> {
    scope.require(Action::CancelOrder)?;
    let (status, _branch, table_id) = lock_order(tx, order_id).await?;
    if !can_transition(status, OrderStatus::Cancelada) {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    inventory::release_order_tx(tx, order_id).await?;
    free_table(tx, table_id).await?;
    set_status(tx, order_id, OrderStatus::Cancelada).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_allows_only_forward_steps() {
        use OrderStatus::*;
        assert!(can_transition(Creada, EnPreparacion));
        assert!(can_transition(Entregada, Cobrada));
        assert!(can_transition(Cobrada, Cerrada));
        assert!(!can_transition(Creada, Lista));
        assert!(!can_transition(Cerrada, EnPreparacion));
        assert!(can_transition(Lista, Cancelada));
        assert!(!can_transition(Cobrada, Cancelada));
    }
}

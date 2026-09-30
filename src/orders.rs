use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::db;
use crate::error::{ApiError, DomainError};
use crate::{inventory, tenancy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub subtotal: Decimal,
    pub inc_amount: Decimal,
    pub tip_amount: Decimal,
    pub total_amount: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub product_id: Uuid,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub line_total: Decimal,
    pub station_id: Option<i32>,
    pub status_id: Option<i32>,
}

const ORDER_COLS: &str = "o.id, st.tima_codigo as sale_type, os.tima_codigo as status, \
     o.table_id, o.branch_id, o.waiter_id, o.subtotal, o.inc_amount, o.tip_amount, o.total_amount";
const ORDER_FROM: &str = "orders o \
     join tbl_tipos_maestra st on st.tima_id = o.sale_type_id \
     join tbl_tipos_maestra os on os.tima_id = o.status_id";

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
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    tenancy::ensure_branch_in_tenant(pool, claims, tenant, branch_id).await?;

    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let sale_type_id: Option<i32> = sqlx::query_scalar("select public.tipo_id('VENTA', $1)")
        .bind(sale_type)
        .fetch_one(&mut *tx)
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
            .fetch_optional(&mut *tx)
            .await?;
            match free {
                Some(true) => {
                    sqlx::query(
                        "update restaurant_tables set status_id = public.tipo_id('MESA', 'OCUPADA') where id = $1",
                    )
                    .bind(table)
                    .execute(&mut *tx)
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

    let order = sqlx::query_as::<_, Order>(&format!(
        "insert into orders
           (tenant_id, branch_id, table_id, sale_type_id, status_id, waiter_id, notes)
         values ($1, $2, $3, $4, public.tipo_id('ORD', 'CREADA'), $5, $6)
         returning id,
           (select tima_codigo from tbl_tipos_maestra where tima_id = sale_type_id) as sale_type,
           (select tima_codigo from tbl_tipos_maestra where tima_id = status_id) as status,
           table_id, branch_id, waiter_id, subtotal, inc_amount, tip_amount, total_amount"
    ))
    .bind(tenant)
    .bind(branch_id)
    .bind(table_id)
    .bind(sale_type_id)
    .bind(waiter_id)
    .bind(notes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(order)
}

/// Agrega una linea con el precio vigente (snapshot) de la sucursal de la orden.
pub async fn add_line(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    notes: Option<String>,
) -> Result<OrderItem, ApiError> {
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }

    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let row: Option<(String, Uuid, Uuid)> = sqlx::query_as(
        "select os.tima_codigo, o.branch_id, o.tenant_id
           from orders o
           join tbl_tipos_maestra os on os.tima_id = o.status_id
          where o.id = $1
          for update of o",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, branch_id, _o_tenant) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if status != "CREADA" {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let price: Option<Option<Decimal>> = sqlx::query_scalar(
        "select coalesce(bp.price, p.base_price)
           from branch_products bp
           join products p on p.id = bp.product_id
          where bp.branch_id = $1 and bp.product_id = $2",
    )
    .bind(branch_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;
    let price = price.flatten().ok_or(ApiError::Domain(DomainError::Validation(
        "product is not in the branch assortment".into(),
    )))?;

    let line = sqlx::query_as::<_, OrderItem>(
        "insert into order_items (tenant_id, branch_id, order_id, product_id, qty, unit_price, notes)
         values ($1, $2, $3, $4, $5, $6, $7)
         returning id, product_id, qty, unit_price, line_total, station_id, status_id",
    )
    .bind(tenant)
    .bind(branch_id)
    .bind(order_id)
    .bind(product_id)
    .bind(qty)
    .bind(price)
    .bind(notes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(line)
}

/// Lee una orden del tenant activo.
pub async fn order(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<Order, ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
    let found = sqlx::query_as::<_, Order>(&format!(
        "select {ORDER_COLS} from {ORDER_FROM} where o.id = $1"
    ))
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Lee una linea del tenant activo.
pub async fn line(pool: &PgPool, claims: &Claims, line_id: Uuid) -> Result<OrderItem, ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
    let found = sqlx::query_as::<_, OrderItem>(
        "select id, product_id, qty, unit_price, line_total, station_id, status_id
           from order_items where id = $1",
    )
    .bind(line_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Confirma la orden: exige lineas, reserva inventario y pasa a `EN_PREPARACION`.
pub async fn confirm(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;

    let (status, branch_id): (String, Uuid) = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let row: Option<(String, Uuid)> = sqlx::query_as(
            "select os.tima_codigo, o.branch_id
               from orders o
               join tbl_tipos_maestra os on os.tima_id = o.status_id
              where o.id = $1",
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.ok_or(ApiError::Domain(DomainError::NotFound))?
    };
    if status != "CREADA" {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    let lines: Vec<(Uuid, Uuid, Decimal)> = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let rows = sqlx::query_as("select id, product_id, qty from order_items where order_id = $1")
            .bind(order_id)
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        rows
    };
    if lines.is_empty() {
        return Err(ApiError::Domain(DomainError::Validation(
            "order has no lines".into(),
        )));
    }

    let mut reservation_ids = Vec::new();
    for (line_id, product_id, qty) in lines {
        match inventory::reserve(
            pool,
            claims,
            branch_id,
            Some(order_id),
            Some(line_id),
            product_id,
            qty,
            None,
        )
        .await
        {
            Ok(ids) => reservation_ids.extend(ids),
            Err(err) => {
                let _ = inventory::release(pool, claims, &reservation_ids).await;
                return Err(err);
            }
        }
    }

    let updated = {
        let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
        let affected = sqlx::query(
            "update orders set status_id = public.tipo_id('ORD', 'EN_PREPARACION')
              where id = $1 and status_id = public.tipo_id('ORD', 'CREADA')",
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        affected
    };

    if updated == 0 {
        let _ = inventory::release(pool, claims, &reservation_ids).await;
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    Ok(())
}

/// Avanza el estado de la orden segun la maquina de estados.
pub async fn transition(
    pool: &PgPool,
    claims: &Claims,
    order_id: Uuid,
    to: OrderStatus,
) -> Result<(), ApiError> {
    if to == OrderStatus::Cancelada {
        return Err(ApiError::Domain(DomainError::Validation(
            "use cancel for cancellation".into(),
        )));
    }

    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let current: Option<String> = sqlx::query_scalar(
        "select os.tima_codigo
           from orders o
           join tbl_tipos_maestra os on os.tima_id = o.status_id
          where o.id = $1
          for update of o",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let current = current.ok_or(ApiError::Domain(DomainError::NotFound))?;
    let from = OrderStatus::from_db(&current)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("unknown order status")))?;

    if !can_transition(from, to) {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    if to == OrderStatus::Cerrada {
        sqlx::query(
            "update orders set status_id = public.tipo_id('ORD', 'CERRADA'), closed_at = now() where id = $1",
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query("update orders set status_id = public.tipo_id('ORD', $2) where id = $1")
            .bind(order_id)
            .bind(to.as_db())
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Cancela la orden (antes de cobrada): libera reservas y libera la mesa.
pub async fn cancel(pool: &PgPool, claims: &Claims, order_id: Uuid) -> Result<(), ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let mut tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;

    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "select os.tima_codigo, o.table_id
           from orders o
           join tbl_tipos_maestra os on os.tima_id = o.status_id
          where o.id = $1
          for update of o",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, table_id) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let cancellable = matches!(
        status.as_str(),
        "CREADA" | "EN_PREPARACION" | "LISTA" | "ENTREGADA"
    );
    if !cancellable {
        return Err(ApiError::Domain(DomainError::Conflict));
    }

    // Libera reservas activas de la orden.
    let released: Vec<(Uuid, Uuid, Decimal)> = sqlx::query_as(
        "update stock_reservations set status_id = public.tipo_id('RES', 'LIBERADA'), updated_at = now()
          where order_id = $1 and status_id = public.tipo_id('RES', 'ACTIVA')
          returning branch_id, product_id, qty",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;
    for (branch_id, product_id, qty) in &released {
        sqlx::query(
            "update stock set reserved = reserved - $1, updated_at = now()
              where branch_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(branch_id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    }

    if let Some(table) = table_id {
        sqlx::query(
            "update restaurant_tables set status_id = public.tipo_id('MESA', 'LIBRE') where id = $1",
        )
        .bind(table)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "update orders set status_id = public.tipo_id('ORD', 'CANCELADA'), cancelled_at = now() where id = $1",
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db;
use crate::error::{ApiError, DomainError};
use crate::tenancy;

/// Movimiento de inventario (bitacora).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Movement {
    pub product_id: Uuid,
    pub movement_type: String,
    pub qty: Decimal,
    pub cause: Option<String>,
    pub location_id: Uuid,
    pub created_at: DateTime<Utc>,
}

const AVAILABLE_SQL: &str = r#"
select case
  when exists (select 1 from recipes rr where rr.product_id = p.id) then
    (select min(floor((coalesce(si.on_hand, 0) - coalesce(si.reserved, 0)) / nullif(ri.qty, 0)))
       from recipe_items ri
       join recipes r2 on r2.id = ri.recipe_id
       left join stock si
         on si.product_id = ri.input_product_id and si.location_id = $1
      where r2.product_id = p.id)
  when p.tracks_inventory then coalesce(s.on_hand, 0) - coalesce(s.reserved, 0)
  else null
end
from products p
left join stock s on s.product_id = p.id and s.location_id = $1
where p.id = $2
"#;

async fn tenant_tx<'a>(
    pool: &'a PgPool,
    claims: &Claims,
) -> Result<(Uuid, Transaction<'a, Postgres>), ApiError> {
    let (tenant, _role) = tenancy::resolve_membership(pool, claims, None).await?;
    let tx = db::begin_tenant_tx_for(pool, claims, Some(tenant)).await?;
    Ok((tenant, tx))
}

/// Disponible de un producto en una sucursal (derivado si tiene receta).
/// `None` si el producto no trackea inventario.
pub async fn available(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
    product_id: Uuid,
) -> Result<Option<Decimal>, ApiError> {
    let (_tenant, mut tx) = tenant_tx(pool, claims).await?;
    expire_due_tx(&mut tx).await?;
    let row: Option<Option<Decimal>> = sqlx::query_scalar(AVAILABLE_SQL)
        .bind(location_id)
        .bind(product_id)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    row.ok_or(ApiError::Domain(DomainError::NotFound))
}

/// Explota la receta de un producto en componentes con stock (o el propio producto).
async fn components(
    tx: &mut Transaction<'_, Postgres>,
    product_id: Uuid,
    qty: Decimal,
) -> Result<Vec<(Uuid, Decimal)>, ApiError> {
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

    let tracks: Option<bool> =
        sqlx::query_scalar("select tracks_inventory from products where id = $1")
            .bind(product_id)
            .fetch_optional(&mut **tx)
            .await?;
    match tracks {
        Some(true) => Ok(vec![(product_id, qty)]),
        _ => Ok(vec![]),
    }
}

/// Valida disponibilidad y crea las reservas por insumo en una sola transaccion.
pub async fn reserve(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
    order_id: Option<Uuid>,
    order_line_id: Option<Uuid>,
    product_id: Uuid,
    qty: Decimal,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Vec<Uuid>, ApiError> {
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }

    let (tenant, mut tx) = tenant_tx(pool, claims).await?;

    let exists: bool =
        sqlx::query_scalar("select exists(select 1 from products where id = $1)")
            .bind(product_id)
            .fetch_one(&mut *tx)
            .await?;
    if !exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    expire_due_tx(&mut tx).await?;

    let mut comps = components(&mut tx, product_id, qty).await?;
    comps.sort_by(|a, b| a.0.cmp(&b.0));

    let mut ids = Vec::new();
    for (stock_product, need) in comps {
        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "select on_hand, reserved from stock
              where location_id = $1 and product_id = $2
              for update",
        )
        .bind(location_id)
        .bind(stock_product)
        .fetch_optional(&mut *tx)
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
        .bind(location_id)
        .bind(stock_product)
        .execute(&mut *tx)
        .await?;

        let id: Uuid = sqlx::query_scalar(
            "insert into reservations
               (tenant_id, location_id, order_id, order_line_id, product_id, qty, status, expires_at)
             values ($1, $2, $3, $4, $5, $6, 'activa', $7)
             returning id",
        )
        .bind(tenant)
        .bind(location_id)
        .bind(order_id)
        .bind(order_line_id)
        .bind(stock_product)
        .bind(need)
        .bind(expires_at)
        .fetch_one(&mut *tx)
        .await?;

        ids.push(id);
    }

    tx.commit().await?;
    Ok(ids)
}

/// Convierte reservas activas en descuento definitivo y registra el movimiento.
pub async fn consume(
    pool: &PgPool,
    claims: &Claims,
    reservation_ids: &[Uuid],
) -> Result<u64, ApiError> {
    let (tenant, mut tx) = tenant_tx(pool, claims).await?;

    let rows: Vec<(Uuid, Uuid, Uuid, Decimal)> = sqlx::query_as(
        "select id, location_id, product_id, qty from reservations
          where id = any($1) and status = 'activa'
          for update",
    )
    .bind(reservation_ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut count = 0u64;
    for (id, location_id, product_id, qty) in rows {
        sqlx::query(
            "update stock
                set on_hand = on_hand - $1, reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "insert into inventory_movements
               (tenant_id, location_id, product_id, type, qty, cause, ref_type, ref_id, created_by)
             values ($1, $2, $3, 'descuento', $4, 'consumo', 'reservation', $5, $6)",
        )
        .bind(tenant)
        .bind(location_id)
        .bind(product_id)
        .bind(qty)
        .bind(id)
        .bind(claims.sub)
        .execute(&mut *tx)
        .await?;

        sqlx::query("update reservations set status = 'confirmada', updated_at = now() where id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// Libera explicitamente reservas activas devolviendo las unidades.
pub async fn release(
    pool: &PgPool,
    claims: &Claims,
    reservation_ids: &[Uuid],
) -> Result<u64, ApiError> {
    let (_tenant, mut tx) = tenant_tx(pool, claims).await?;

    let rows: Vec<(Uuid, Uuid, Uuid, Decimal)> = sqlx::query_as(
        "select id, location_id, product_id, qty from reservations
          where id = any($1) and status = 'activa'
          for update",
    )
    .bind(reservation_ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut count = 0u64;
    for (id, location_id, product_id, qty) in rows {
        sqlx::query(
            "update stock set reserved = reserved - $1, updated_at = now()
              where location_id = $2 and product_id = $3",
        )
        .bind(qty)
        .bind(location_id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("update reservations set status = 'liberada', updated_at = now() where id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

async fn expire_due_tx(tx: &mut Transaction<'_, Postgres>) -> Result<u64, ApiError> {
    let expired: Vec<(Uuid, Uuid, Uuid, Decimal)> = sqlx::query_as(
        "update reservations set status = 'expirada', updated_at = now()
          where status = 'activa' and expires_at is not null and expires_at < now()
          returning id, location_id, product_id, qty",
    )
    .fetch_all(&mut **tx)
    .await?;

    for (_id, location_id, product_id, qty) in &expired {
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

    Ok(expired.len() as u64)
}

/// Barrido de reservas vencidas del tenant activo.
pub async fn expire_due(pool: &PgPool, claims: &Claims) -> Result<u64, ApiError> {
    let (_tenant, mut tx) = tenant_tx(pool, claims).await?;
    let count = expire_due_tx(&mut tx).await?;
    tx.commit().await?;
    Ok(count)
}

/// Repone inventario y registra el movimiento.
pub async fn replenish(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    if qty <= Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "qty must be > 0".into(),
        )));
    }

    let (tenant, mut tx) = tenant_tx(pool, claims).await?;

    sqlx::query(
        "insert into stock (tenant_id, location_id, product_id, on_hand, reserved)
         values ($1, $2, $3, $4, 0)
         on conflict (location_id, product_id)
         do update set on_hand = stock.on_hand + excluded.on_hand, updated_at = now()",
    )
    .bind(tenant)
    .bind(location_id)
    .bind(product_id)
    .bind(qty)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "insert into inventory_movements
           (tenant_id, location_id, product_id, type, qty, cause, created_by)
         values ($1, $2, $3, 'reposicion', $4, $5, $6)",
    )
    .bind(tenant)
    .bind(location_id)
    .bind(product_id)
    .bind(qty)
    .bind(cause)
    .bind(claims.sub)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Ajusta inventario (delta con signo) y registra el movimiento.
pub async fn adjust(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
    product_id: Uuid,
    delta: Decimal,
    cause: &str,
) -> Result<(), ApiError> {
    if delta == Decimal::ZERO {
        return Err(ApiError::Domain(DomainError::Validation(
            "delta must be non-zero".into(),
        )));
    }

    let (tenant, mut tx) = tenant_tx(pool, claims).await?;

    let row: Option<(Decimal, Decimal)> = sqlx::query_as(
        "select on_hand, reserved from stock
          where location_id = $1 and product_id = $2
          for update",
    )
    .bind(location_id)
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (on_hand, reserved) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;

    let new_on_hand = on_hand + delta;
    if new_on_hand < reserved || new_on_hand < Decimal::ZERO {
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
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "insert into inventory_movements
           (tenant_id, location_id, product_id, type, qty, cause, created_by)
         values ($1, $2, $3, 'ajuste', $4, $5, $6)",
    )
    .bind(tenant)
    .bind(location_id)
    .bind(product_id)
    .bind(delta.abs())
    .bind(cause)
    .bind(claims.sub)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Bitacora de movimientos de una sucursal.
pub async fn movements(
    pool: &PgPool,
    claims: &Claims,
    location_id: Uuid,
) -> Result<Vec<Movement>, ApiError> {
    let (_tenant, mut tx) = tenant_tx(pool, claims).await?;
    let rows = sqlx::query_as::<_, Movement>(
        "select product_id, \"type\"::text as movement_type, qty, cause, location_id, created_at
           from inventory_movements
          where location_id = $1
          order by created_at",
    )
    .bind(location_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

//! Turnos de caja: apertura con base, ingresos y retiros de efectivo, resumen
//! esperado por metodo de pago y cierre con arqueo (diferencia entre lo
//! contado y lo esperado).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state, validation};
use crate::tenancy::{self, Action, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CashSession {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub opened_by: Uuid,
    pub opening_amount: Decimal,
    pub opened_at: DateTime<Utc>,
    pub closed_by: Option<Uuid>,
    pub closed_at: Option<DateTime<Utc>>,
    pub expected_cash: Option<Decimal>,
    pub counted_cash: Option<Decimal>,
    pub difference: Option<Decimal>,
    pub notes: Option<String>,
}

/// Total cobrado en el turno con un metodo de pago.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MethodTotal {
    pub method: String,
    pub amount: Decimal,
    pub count: i64,
}

/// Estado del turno para la pantalla de caja y el arqueo.
#[derive(Debug, Serialize)]
pub struct CashSummary {
    pub session: CashSession,
    pub sales_count: i64,
    pub sales_total: Decimal,
    pub by_method: Vec<MethodTotal>,
    /// Ingresos de efectivo que no son ventas (cambio que trae el dueno...).
    pub cash_in: Decimal,
    /// Retiros de efectivo de la gaveta (gastos menores, envio a banco...).
    pub cash_out: Decimal,
    /// Base inicial + efectivo cobrado + ingresos - retiros.
    pub expected_cash: Decimal,
}

/// Tipo de movimiento manual de efectivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CashMovementKind {
    Ingreso,
    Retiro,
}

impl CashMovementKind {
    fn code(self) -> &'static str {
        match self {
            CashMovementKind::Ingreso => "INGRESO",
            CashMovementKind::Retiro => "RETIRO",
        }
    }
}

/// Una linea de la bitacora del turno: apertura, venta, ingreso, retiro o cierre.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ActivityEntry {
    pub at: DateTime<Utc>,
    /// APERTURA | VENTA | INGRESO | RETIRO | CIERRE
    pub kind: String,
    pub detail: Option<String>,
    /// Metodos de pago de la venta (`EFECTIVO`, `DATAFONO+QR`...).
    pub methods: Option<String>,
    pub amount: Decimal,
    pub user_name: Option<String>,
}

/// Turno en el historial de la sucursal.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub opened_by: Uuid,
    pub opened_by_name: Option<String>,
    pub opening_amount: Decimal,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub expected_cash: Option<Decimal>,
    pub counted_cash: Option<Decimal>,
    pub difference: Option<Decimal>,
    pub sales_count: i64,
    pub sales_total: Decimal,
}

const SESSION_COLS: &str = "id, branch_id, opened_by, opening_amount, opened_at, closed_by, \
     closed_at, expected_cash, counted_cash, difference, notes";

// ------------------------------------------------------------------ apertura

/// Abre un turno para el usuario en la sucursal. Un cajero no puede tener
/// dos turnos abiertos en la misma sucursal (409).
pub async fn open(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    opening_amount: Decimal,
) -> Result<CashSession, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageCash)?;
    if opening_amount < Decimal::ZERO {
        return Err(validation("opening_amount must be >= 0"));
    }
    let settings = crate::settings::load_tx(&mut tx, branch_id).await?;
    if settings.cash.one_session_per_user
        && open_session_tx(&mut tx, &scope, branch_id).await?.is_some()
    {
        return Err(state("there is already an open cash session"));
    }

    let session = sqlx::query_as::<_, CashSession>(&format!(
        "insert into cash_sessions (tenant_id, branch_id, opened_by, opening_amount)
         values ($1, $2, $3, $4)
         returning {SESSION_COLS}"
    ))
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(scope.user)
    .bind(opening_amount)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(session)
}

/// Turno abierto del usuario en la sucursal. Toma un bloqueo compartido: una
/// venta en curso impide que el turno se cierre a mitad.
pub async fn open_session_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let id = sqlx::query_scalar(
        "select id from cash_sessions
          where branch_id = $1 and opened_by = $2 and closed_at is null
          for share",
    )
    .bind(branch_id)
    .bind(scope.user)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(id)
}

// ------------------------------------------------------------------- resumen

/// Resumen del turno abierto del usuario en la sucursal (`None` si no hay).
pub async fn current(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Option<CashSummary>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageCash)?;
    let summary = match open_session_tx(&mut tx, &scope, branch_id).await? {
        Some(id) => Some(summary_tx(&mut tx, id).await?),
        None => None,
    };
    tx.commit().await?;
    Ok(summary)
}

pub async fn summary_tx(tx: &mut Tx<'_>, session_id: Uuid) -> Result<CashSummary, ApiError> {
    let session = sqlx::query_as::<_, CashSession>(&format!(
        "select {SESSION_COLS} from cash_sessions where id = $1"
    ))
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let (sales_count, sales_total): (i64, Decimal) = sqlx::query_as(
        "select count(*), coalesce(sum(total_amount), 0)
           from orders
          where cash_session_id = $1
            and status_id in (public.tipo_id('ORD', 'COBRADA'), public.tipo_id('ORD', 'CERRADA'))",
    )
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?;

    let by_method = sqlx::query_as::<_, MethodTotal>(
        "select t.tima_codigo as method, sum(p.amount) as amount, count(*) as count
           from payments p
           join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.cash_session_id = $1
          group by t.tima_codigo
          order by t.tima_codigo",
    )
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;

    let cash_sales: Decimal = by_method
        .iter()
        .filter(|m| m.method == "EFECTIVO")
        .map(|m| m.amount)
        .sum();

    let (cash_in, cash_out): (Decimal, Decimal) = sqlx::query_as(
        "select coalesce(sum(m.amount) filter (where t.tima_codigo = 'INGRESO'), 0),
                coalesce(sum(m.amount) filter (where t.tima_codigo = 'RETIRO'), 0)
           from cash_movements m
           join tbl_tipos_maestra t on t.tima_id = m.kind_id
          where m.cash_session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?;

    let expected_cash = session.opening_amount + cash_sales + cash_in - cash_out;

    Ok(CashSummary {
        session,
        sales_count,
        sales_total,
        by_method,
        cash_in,
        cash_out,
        expected_cash,
    })
}

/// Bloquea el turno y valida que siga abierto y que el usuario pueda operarlo
/// (su dueno o un administrador). Devuelve la sucursal del turno.
async fn lock_open_session_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    session_id: Uuid,
) -> Result<Uuid, ApiError> {
    let row: Option<(Uuid, Uuid, bool)> = sqlx::query_as(
        "select branch_id, opened_by, closed_at is not null
           from cash_sessions where id = $1 for update",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (branch_id, opened_by, closed) = row.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if opened_by != scope.user && !scope.is_admin() {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    if closed {
        return Err(state("cash session is already closed"));
    }
    Ok(branch_id)
}

// --------------------------------------------------- ingresos y retiros

/// Registra un ingreso o retiro de efectivo en un turno abierto. Un retiro no
/// puede dejar la gaveta en negativo.
pub async fn add_movement(
    pool: &PgPool,
    claims: &Claims,
    session_id: Uuid,
    kind: CashMovementKind,
    amount: Decimal,
    reason: &str,
) -> Result<CashSummary, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageCash)?;
    if amount <= Decimal::ZERO {
        return Err(validation("amount must be > 0"));
    }
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(validation("reason is required"));
    }

    let branch_id = lock_open_session_tx(&mut tx, &scope, session_id).await?;
    if kind == CashMovementKind::Retiro {
        let expected = summary_tx(&mut tx, session_id).await?.expected_cash;
        if amount > expected {
            return Err(state("withdrawal exceeds the cash in the drawer"));
        }
    }

    sqlx::query(
        "insert into cash_movements
           (tenant_id, branch_id, cash_session_id, kind_id, amount, reason, created_by)
         values ($1, $2, $3, public.tipo_id('CAJA_MOV', $4), $5, $6, $7)",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(session_id)
    .bind(kind.code())
    .bind(amount)
    .bind(reason)
    .bind(scope.user)
    .execute(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        match kind {
            CashMovementKind::Ingreso => "INGRESO_CAJA",
            CashMovementKind::Retiro => "RETIRO_CAJA",
        },
        "cash_movements",
        session_id,
        None,
        Some(serde_json::json!({
            "amount": amount,
            "reason": reason
        })),
        None,
    )
    .await?;

    let summary = summary_tx(&mut tx, session_id).await?;
    tx.commit().await?;
    Ok(summary)
}

// ------------------------------------------------------------------ bitacora

/// Bitacora del turno, mas reciente primero: apertura, ventas, ingresos,
/// retiros y cierre. Un cajero solo ve sus propios turnos.
pub async fn activity(
    pool: &PgPool,
    claims: &Claims,
    session_id: Uuid,
) -> Result<Vec<ActivityEntry>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageCash)?;

    let owner: Option<Uuid> =
        sqlx::query_scalar("select opened_by from cash_sessions where id = $1")
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?;
    let owner = owner.ok_or(ApiError::Domain(DomainError::NotFound))?;
    if owner != scope.user && !scope.is_admin() {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let rows = sqlx::query_as::<_, ActivityEntry>(
        "select * from (
           select s.opened_at as at, 'APERTURA' as kind, 'Base inicial' as detail,
                  'EFECTIVO' as methods, s.opening_amount as amount, u.full_name as user_name
             from cash_sessions s left join users u on u.id = s.opened_by
            where s.id = $1
           union all
           select o.created_at, 'VENTA',
                  o.receipt_prefix || '-' || lpad(o.receipt_number::text, 6, '0'),
                  (select string_agg(t.tima_codigo, '+' order by t.tima_codigo)
                     from payments p join tbl_tipos_maestra t on t.tima_id = p.method_id
                    where p.order_id = o.id),
                  o.total_amount, u.full_name
             from orders o
             join cash_sessions s on s.id = o.cash_session_id
             left join users u on u.id = s.opened_by
            where o.cash_session_id = $1
              and o.status_id in (public.tipo_id('ORD', 'COBRADA'), public.tipo_id('ORD', 'CERRADA'))
           union all
           select m.created_at, t.tima_codigo, m.reason, 'EFECTIVO', m.amount, u.full_name
             from cash_movements m
             join tbl_tipos_maestra t on t.tima_id = m.kind_id
             left join users u on u.id = m.created_by
            where m.cash_session_id = $1
           union all
           select s.closed_at, 'CIERRE', s.notes, 'EFECTIVO', s.counted_cash, u.full_name
             from cash_sessions s left join users u on u.id = s.closed_by
            where s.id = $1 and s.closed_at is not null
         ) a
         order by at desc",
    )
    .bind(session_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

// ------------------------------------------------------------------ historial

/// Turnos de la sucursal, mas reciente primero. Un cajero solo ve los suyos.
pub async fn history(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    limit: i64,
) -> Result<Vec<SessionRow>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageCash)?;
    let only_user = (!scope.is_admin()).then_some(scope.user);

    let rows = sqlx::query_as::<_, SessionRow>(
        "select s.id, s.opened_by, u.full_name as opened_by_name, s.opening_amount, s.opened_at,
                s.closed_at, s.expected_cash, s.counted_cash, s.difference,
                coalesce(o.n, 0) as sales_count, coalesce(o.total, 0) as sales_total
           from cash_sessions s
           left join users u on u.id = s.opened_by
           left join lateral (
             select count(*) as n, sum(total_amount) as total
               from orders
              where cash_session_id = s.id
                and status_id in (public.tipo_id('ORD', 'COBRADA'), public.tipo_id('ORD', 'CERRADA'))
           ) o on true
          where s.branch_id = $1 and ($2::uuid is null or s.opened_by = $2)
          order by s.opened_at desc
          limit $3",
    )
    .bind(branch_id)
    .bind(only_user)
    .bind(limit.clamp(1, 200))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

// -------------------------------------------------------------------- cierre

/// Cierra el turno con el efectivo contado. Solo el dueno del turno o un
/// administrador puede cerrarlo.
pub async fn close(
    pool: &PgPool,
    claims: &Claims,
    session_id: Uuid,
    counted_cash: Decimal,
    notes: Option<String>,
) -> Result<CashSummary, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageCash)?;
    if counted_cash < Decimal::ZERO {
        return Err(validation("counted_cash must be >= 0"));
    }

    // Bloqueo exclusivo: espera a que terminen las ventas en curso del turno.
    let branch_id = lock_open_session_tx(&mut tx, &scope, session_id).await?;
    let settings = crate::settings::load_tx(&mut tx, branch_id).await?;

    let expected = summary_tx(&mut tx, session_id).await?.expected_cash;
    let diff = (counted_cash - expected).abs();
    if diff > settings.cash.close_tolerance && notes.as_ref().map_or(true, |n| n.trim().is_empty())
    {
        return Err(validation(
            "notes are required when cash difference exceeds tolerance",
        ));
    }

    sqlx::query(
        "update cash_sessions
            set closed_at = now(), closed_by = $2, expected_cash = $3,
                counted_cash = $4, difference = $4 - $3, notes = $5
          where id = $1",
    )
    .bind(session_id)
    .bind(scope.user)
    .bind(expected)
    .bind(counted_cash)
    .bind(&notes)
    .execute(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "CIERRE_CAJA",
        "cash_sessions",
        session_id,
        None,
        Some(serde_json::json!({
            "expected_cash": expected,
            "counted_cash": counted_cash,
            "difference": counted_cash - expected
        })),
        None,
    )
    .await?;

    let summary = summary_tx(&mut tx, session_id).await?;
    tx.commit().await?;
    Ok(summary)
}

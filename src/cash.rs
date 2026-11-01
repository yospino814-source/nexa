//! Turnos de caja: apertura con base, ingresos y retiros de efectivo, resumen
//! esperado por metodo de pago y cierre con arqueo (diferencia entre lo
//! contado y lo esperado).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError, state, validation};
use crate::tenancy::{self, Action, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct CashSession {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub register_id: Uuid,
    pub register_name: String,
    pub register_code: String,
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
#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct MethodTotal {
    pub method: String,
    pub amount: Decimal,
    pub count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MovementByKind {
    pub kind: String,
    pub label: String,
    pub direction: String,
    pub amount: Decimal,
    pub count: i64,
}

/// Estado del turno para la pantalla de caja y el arqueo.
#[derive(Debug, Serialize, Clone)]
pub struct CashSummary {
    pub session: CashSession,
    pub sales_count: i64,
    pub sales_total: Decimal,
    pub by_method: Vec<MethodTotal>,
    pub movements_by_kind: Vec<MovementByKind>,
    pub cash_in: Decimal,
    pub cash_out: Decimal,
    pub expected_cash: Decimal,
    pub tips_total: Decimal,
    pub tips_paid: Decimal,
    pub tips_pending: Decimal,
}

/// Tipo de movimiento de efectivo del turno (9 tipos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CashMovementKind {
    Ingreso,
    AbonoCartera,
    AbonoSeparado,
    RecargaSaldo,
    Retiro,
    Devolucion,
    PropinaPagada,
    PagoProveedor,
    Gasto,
}

impl CashMovementKind {
    pub fn code(self) -> &'static str {
        match self {
            CashMovementKind::Ingreso => "INGRESO",
            CashMovementKind::AbonoCartera => "ABONO_CARTERA",
            CashMovementKind::AbonoSeparado => "ABONO_SEPARADO",
            CashMovementKind::RecargaSaldo => "RECARGA_SALDO",
            CashMovementKind::Retiro => "RETIRO",
            CashMovementKind::Devolucion => "DEVOLUCION",
            CashMovementKind::PropinaPagada => "PROPINA_PAGADA",
            CashMovementKind::PagoProveedor => "PAGO_PROVEEDOR",
            CashMovementKind::Gasto => "GASTO",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CashMovementKind::Ingreso => "Ingreso de efectivo",
            CashMovementKind::AbonoCartera => "Abono de cartera",
            CashMovementKind::AbonoSeparado => "Abono de separado",
            CashMovementKind::RecargaSaldo => "Recarga de saldo",
            CashMovementKind::Retiro => "Retiro de efectivo",
            CashMovementKind::Devolucion => "Devolución",
            CashMovementKind::PropinaPagada => "Propina pagada",
            CashMovementKind::PagoProveedor => "Pago a proveedor",
            CashMovementKind::Gasto => "Gasto",
        }
    }

    pub fn is_inflow(self) -> bool {
        matches!(
            self,
            CashMovementKind::Ingreso
                | CashMovementKind::AbonoCartera
                | CashMovementKind::AbonoSeparado
                | CashMovementKind::RecargaSaldo
        )
    }

    pub const ALL: [CashMovementKind; 9] = [
        CashMovementKind::Ingreso,
        CashMovementKind::AbonoCartera,
        CashMovementKind::AbonoSeparado,
        CashMovementKind::RecargaSaldo,
        CashMovementKind::Retiro,
        CashMovementKind::Devolucion,
        CashMovementKind::PropinaPagada,
        CashMovementKind::PagoProveedor,
        CashMovementKind::Gasto,
    ];
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

const SESSION_SELECT: &str = "select s.id, s.branch_id, s.register_id, r.name as register_name, r.code as register_code, \
     s.opened_by, s.opening_amount, s.opened_at, s.closed_by, s.closed_at, \
     s.expected_cash, s.counted_cash, s.difference, s.notes \
     from cash_sessions s join cash_registers r on r.id = s.register_id";

// ------------------------------------------------------------------ apertura

/// Abre un turno para el usuario en la sucursal y caja.
pub async fn open(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    register_id: Uuid,
    opening_amount: Decimal,
) -> Result<CashSession, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("cash:open")?;
    if opening_amount < Decimal::ZERO {
        return Err(validation("opening_amount must be >= 0"));
    }

    // 1. La caja existe, pertenece a la sucursal del cuerpo y está activa. Si no: 422 REGISTER_INVALID.
    let register_active: Option<bool> = sqlx::query_scalar(
        "select active from cash_registers where id = $1 and branch_id = $2 for share",
    )
    .bind(register_id)
    .bind(branch_id)
    .fetch_optional(&mut *tx)
    .await?;

    match register_active {
        Some(true) => {}
        _ => {
            return Err(error::coded(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "REGISTER_INVALID",
                "La caja no existe o no está activa",
                None,
            ));
        }
    }

    // 2. La caja no tiene turno abierto. Si lo tiene: 409 REGISTER_BUSY, con data: {opened_by_name}.
    let busy_row: Option<(Option<String>,)> = sqlx::query_as(
        "select u.full_name
         from cash_sessions s
         left join users u on u.id = s.opened_by
         where s.register_id = $1 and s.closed_at is null
         for share",
    )
    .bind(register_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((opened_by_name,)) = busy_row {
        let name = opened_by_name.unwrap_or_else(|| "Otro usuario".to_string());
        return Err(error::coded(
            axum::http::StatusCode::CONFLICT,
            "REGISTER_BUSY",
            "La caja ya tiene un turno abierto",
            Some(serde_json::json!({ "opened_by_name": name })),
        ));
    }

    // 3. El usuario no tiene otro turno abierto en la sucursal. Si lo tiene: 409 USER_HAS_OPEN_SESSION.
    if open_session_tx(&mut tx, &scope, branch_id).await?.is_some() {
        return Err(error::coded(
            axum::http::StatusCode::CONFLICT,
            "USER_HAS_OPEN_SESSION",
            "El usuario ya tiene un turno abierto en la sucursal",
            None,
        ));
    }

    let session_id: Uuid = sqlx::query_scalar(
        "insert into cash_sessions (tenant_id, branch_id, register_id, opened_by, opening_amount)
         values ($1, $2, $3, $4, $5)
         returning id",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(register_id)
    .bind(scope.user)
    .bind(opening_amount)
    .fetch_one(&mut *tx)
    .await?;

    let session = sqlx::query_as::<_, CashSession>(&format!("{SESSION_SELECT} where s.id = $1"))
        .bind(session_id)
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
    let session = sqlx::query_as::<_, CashSession>(&format!("{SESSION_SELECT} where s.id = $1"))
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
           join orders o on o.id = p.order_id
          where p.cash_session_id = $1
            and o.status_id != public.tipo_id('ORD', 'CANCELADA')
          group by t.tima_codigo
          order by case when t.tima_codigo = 'EFECTIVO' then 0 else 1 end, t.tima_codigo",
    )
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;

    let cash_sales: Decimal = by_method
        .iter()
        .filter(|m| m.method == "EFECTIVO")
        .map(|m| m.amount)
        .sum();

    let tips_total: Decimal = sqlx::query_scalar(
        "select coalesce(sum(tip_amount), 0)
           from orders
          where cash_session_id = $1
            and status_id != public.tipo_id('ORD', 'CANCELADA')",
    )
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?;

    #[derive(sqlx::FromRow)]
    struct MovAggRow {
        kind: String,
        amount: Decimal,
        count: i64,
    }

    let mov_rows = sqlx::query_as::<_, MovAggRow>(
        "select t.tima_codigo as kind, coalesce(sum(m.amount), 0) as amount, count(m.id) as count
           from tbl_tipos_maestra t
           left join cash_movements m on m.kind_id = t.tima_id and m.cash_session_id = $1
          where t.padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'CAJA_MOV')
          group by t.tima_codigo",
    )
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut movements_by_kind = Vec::with_capacity(CashMovementKind::ALL.len());
    let mut cash_in = Decimal::ZERO;
    let mut cash_out = Decimal::ZERO;
    let mut tips_paid = Decimal::ZERO;

    for kind in CashMovementKind::ALL {
        let (amount, count) = mov_rows
            .iter()
            .find(|r| r.kind == kind.code())
            .map(|r| (r.amount, r.count))
            .unwrap_or((Decimal::ZERO, 0));

        let direction = if kind.is_inflow() { "IN" } else { "OUT" };
        if kind.is_inflow() {
            cash_in += amount;
        } else {
            cash_out += amount;
        }
        if kind == CashMovementKind::PropinaPagada {
            tips_paid += amount;
        }

        movements_by_kind.push(MovementByKind {
            kind: kind.code().to_string(),
            label: kind.label().to_string(),
            direction: direction.to_string(),
            amount,
            count,
        });
    }

    let tips_pending = tips_total - tips_paid;
    let expected_cash =
        crate::money::money(session.opening_amount + cash_sales + cash_in - cash_out);

    Ok(CashSummary {
        session,
        sales_count,
        sales_total,
        by_method,
        movements_by_kind,
        cash_in,
        cash_out,
        expected_cash,
        tips_total,
        tips_paid,
        tips_pending,
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

/// Función interna única para registrar movimientos de caja.
#[allow(clippy::too_many_arguments)]
pub async fn record_movement_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    session_id: Uuid,
    kind: CashMovementKind,
    amount: Decimal,
    reason: &str,
    reason_code: Option<&str>,
    ref_type: Option<&str>,
    ref_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    let amount = crate::money::money(amount);
    if amount <= Decimal::ZERO {
        return Err(error::coded(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "CASH_AMOUNT_INVALID",
            "El monto debe ser mayor a cero",
            None,
        ));
    }

    let branch_id = lock_open_session_tx(tx, scope, session_id).await?;

    if !kind.is_inflow() {
        let available = summary_tx(tx, session_id).await?.expected_cash;
        if amount > available {
            return Err(error::coded(
                axum::http::StatusCode::CONFLICT,
                "CASH_INSUFFICIENT",
                "Efectivo insuficiente en gaveta",
                Some(serde_json::json!({ "available": available })),
            ));
        }
    }

    let mov_id: Uuid = sqlx::query_scalar(
        "insert into cash_movements
           (tenant_id, branch_id, cash_session_id, kind_id, amount, reason, reason_code, ref_type, ref_id, created_by)
         values ($1, $2, $3, public.tipo_id('CAJA_MOV', $4), $5, $6, $7, $8, $9, $10)
         returning id",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(session_id)
    .bind(kind.code())
    .bind(amount)
    .bind(reason)
    .bind(reason_code)
    .bind(ref_type)
    .bind(ref_id)
    .bind(scope.user)
    .fetch_one(&mut **tx)
    .await?;

    crate::audit::record_tx(
        tx,
        scope,
        "MOVIMIENTO_CAJA",
        "cash_movements",
        session_id,
        None,
        Some(serde_json::json!({
            "kind": kind.code(),
            "amount": amount,
            "reason": reason,
            "reason_code": reason_code,
            "ref_type": ref_type,
            "ref_id": ref_id,
        })),
        None,
    )
    .await?;

    Ok(mov_id)
}

/// Endpoint público para registrar movimientos de caja (permiso cash:movement).
pub async fn add_movement(
    pool: &PgPool,
    claims: &Claims,
    session_id: Uuid,
    kind: CashMovementKind,
    amount: Decimal,
    reason: Option<&str>,
    reason_code: Option<&str>,
) -> Result<CashSummary, ApiError> {
    let (final_reason, final_reason_code) = match kind {
        CashMovementKind::Ingreso => {
            if reason_code.is_some() {
                return Err(validation("reason_code must be null for INGRESO"));
            }
            let r = reason.unwrap_or("").trim();
            if r.chars().count() < 3 || r.chars().count() > 200 {
                return Err(validation(
                    "reason must be between 3 and 200 characters for INGRESO",
                ));
            }
            (r.to_string(), None)
        }
        CashMovementKind::Retiro => {
            let rc = match reason_code {
                Some(c) => c.trim(),
                None => return Err(validation("reason_code is required for RETIRO")),
            };
            if !matches!(rc, "CONSIGNACION" | "GASTO_MENOR" | "PRESTAMO" | "OTRO") {
                return Err(validation(
                    "reason_code must be CONSIGNACION, GASTO_MENOR, PRESTAMO or OTRO",
                ));
            }
            let label = match rc {
                "CONSIGNACION" => "Consignación a banco",
                "GASTO_MENOR" => "Gasto menor",
                "PRESTAMO" => "Préstamo",
                _ => "Otro",
            };
            let r = reason.unwrap_or("").trim();
            if rc == "OTRO" {
                if r.chars().count() < 3 || r.chars().count() > 200 {
                    return Err(validation(
                        "reason must be between 3 and 200 characters when reason_code is OTRO",
                    ));
                }
                (r.to_string(), Some(rc.to_string()))
            } else {
                let actual_reason = if r.is_empty() { label } else { r };
                (actual_reason.to_string(), Some(rc.to_string()))
            }
        }
        CashMovementKind::PropinaPagada => {
            let r = reason.unwrap_or("").trim();
            if r.is_empty() || r.chars().count() > 200 {
                return Err(validation(
                    "reason (waiter name) is required for PROPINA_PAGADA",
                ));
            }
            (r.to_string(), None)
        }
        _ => {
            return Err(error::coded(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "CASH_KIND_NOT_ALLOWED",
                "Tipo de movimiento no permitido en este endpoint",
                None,
            ));
        }
    };

    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("cash:movement")?;

    record_movement_tx(
        &mut tx,
        &scope,
        session_id,
        kind,
        amount,
        &final_reason,
        final_reason_code.as_deref(),
        None,
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
    if diff > settings.cash.close_tolerance && notes.as_ref().is_none_or(|n| n.trim().is_empty()) {
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

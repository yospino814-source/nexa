//! Panel principal con datos reales (S6 / F7.2).
//!
//! Expone `GET /v1/reports/dashboard?branch_id=&from=&to=` (permiso `reports:read`).
//! Reutiliza las fórmulas únicas de `crate::reports::metrics` sobre `public.v_sales`.

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::money::money;
use crate::reports::metrics;
use crate::tenancy;

type Tx<'a> = Transaction<'a, Postgres>;

const MAX_RANGE_DAYS: i64 = 3660;

fn empty_string_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) => s.parse::<T>().map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct DashboardQuery {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub branch_id: Option<Uuid>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub from: Option<NaiveDate>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardKpis {
    pub gross_sale: Decimal,
    pub net_sale: Decimal,
    pub sales_count: i64,
    pub avg_ticket: Decimal,
    pub gross_margin: Decimal,
    pub margin_pct: Decimal,
    pub refunds: Decimal,
    pub tips: Decimal,
    pub cost_missing_lines: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HourlySales {
    pub hour: i32,
    pub gross_sale: Decimal,
    pub sales_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailySales {
    pub date: NaiveDate,
    pub gross_sale: Decimal,
    pub sales_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct TopProduct {
    pub product_id: Option<Uuid>,
    pub name: String,
    pub qty: Decimal,
    pub line_total: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PaymentMethodSummary {
    pub method: String,
    pub amount: Decimal,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LowStockItem {
    pub product_id: Uuid,
    pub name: String,
    pub location_name: String,
    pub on_hand: Decimal,
    pub min_qty: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverdueReceivablesSummary {
    pub count: i64,
    pub amount: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub period: DateRange,
    pub previous: DateRange,
    pub kpis: DashboardKpis,
    pub kpis_previous: DashboardKpis,
    pub by_hour: Vec<HourlySales>,
    pub by_day: Vec<DailySales>,
    pub top_products: Vec<TopProduct>,
    pub payment_methods: Vec<PaymentMethodSummary>,
    pub low_stock: Vec<LowStockItem>,
    pub overdue_receivables: OverdueReceivablesSummary,
}

/// Calcula el periodo anterior con la misma duración exacta inmediatamente antes de `from`.
pub fn previous_period(from: NaiveDate, to: NaiveDate) -> Option<DateRange> {
    if from > to {
        return None;
    }
    let span_days = (to - from).num_days().checked_add(1)?;
    let prev_to = from.checked_sub_signed(Duration::days(1))?;
    let prev_from = prev_to.checked_sub_signed(Duration::days(span_days.checked_sub(1)?))?;
    Some(DateRange {
        from: prev_from,
        to: prev_to,
    })
}

/// Completa las 24 horas del día (`0..=23`) en orden ascendente.
pub fn fill_hours(rows: &[(i32, Decimal, i64)]) -> Vec<HourlySales> {
    let mut hours: Vec<HourlySales> = (0..24)
        .map(|h| HourlySales {
            hour: h,
            gross_sale: Decimal::ZERO,
            sales_count: 0,
        })
        .collect();
    for &(hour, gross_sale, sales_count) in rows {
        if let Ok(idx) = usize::try_from(hour)
            && let Some(slot) = hours.get_mut(idx)
        {
            slot.gross_sale = money(gross_sale);
            slot.sales_count = sales_count;
        }
    }
    hours
}

/// Completa todos los días del rango cerrado `from..=to` en orden cronológico.
pub fn fill_days(
    from: NaiveDate,
    to: NaiveDate,
    rows: &[(NaiveDate, Decimal, i64)],
) -> Vec<DailySales> {
    let mut by_date = std::collections::HashMap::with_capacity(rows.len());
    for &(date, gross_sale, sales_count) in rows {
        by_date.insert(date, (money(gross_sale), sales_count));
    }

    let mut days = Vec::new();
    let mut cursor = from;
    while cursor <= to {
        let (gross_sale, sales_count) = by_date.get(&cursor).copied().unwrap_or((Decimal::ZERO, 0));
        days.push(DailySales {
            date: cursor,
            gross_sale,
            sales_count,
        });
        match cursor.succ_opt() {
            Some(next) => cursor = next,
            None => break,
        }
    }
    days
}

async fn local_today_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    branch_filter: Option<Uuid>,
) -> Result<NaiveDate, ApiError> {
    let today: NaiveDate = sqlx::query_scalar(
        "select (now() at time zone coalesce(
            (select b.timezone
               from public.branches b
              where b.tenant_id = $1
                and ($2::uuid is null or b.id = $2)
              order by b.created_at asc
              limit 1),
            'America/Bogota'
         ))::date",
    )
    .bind(tenant_id)
    .bind(branch_filter)
    .fetch_one(&mut **tx)
    .await?;
    Ok(today)
}

async fn load_kpis_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    branch_filter: Option<Uuid>,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<DashboardKpis, ApiError> {
    let (gross_sale, sales_count, tips): (Decimal, i64, Decimal) = sqlx::query_as(
        "select coalesce(sum(v.gross_sale), 0),
                count(*)::bigint,
                coalesce(sum(v.tip_amount), 0)
           from public.v_sales v
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4",
    )
    .bind(tenant_id)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_one(&mut **tx)
    .await?;

    let refunds: Decimal = sqlx::query_scalar(
        "select coalesce(sum(r.total), 0)
           from public.refunds r
           join public.branches b on b.id = r.branch_id
          where r.tenant_id = $1
            and ($2::uuid is null or r.branch_id = $2)
            and r.status = 'APROBADA'
            and r.decided_at is not null
            and (r.decided_at at time zone b.timezone)::date between $3 and $4",
    )
    .bind(tenant_id)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_one(&mut **tx)
    .await?;

    let (line_total_sum, cost_of_sales, cost_missing_lines): (Decimal, Decimal, i64) =
        sqlx::query_as(
            "select coalesce(sum(oi.line_total), 0),
                    coalesce(sum(coalesce(oi.cost_total, 0)), 0),
                    count(*) filter (where oi.cost_missing)::bigint
               from public.order_items oi
               join public.v_sales v on v.id = oi.order_id
              where v.tenant_id = $1
                and ($2::uuid is null or v.branch_id = $2)
                and v.local_date between $3 and $4",
        )
        .bind(tenant_id)
        .bind(branch_filter)
        .bind(from)
        .bind(to)
        .fetch_one(&mut **tx)
        .await?;

    let m = metrics::from_totals(
        gross_sale,
        refunds,
        sales_count,
        line_total_sum,
        cost_of_sales,
        cost_missing_lines,
    );

    Ok(DashboardKpis {
        gross_sale: m.gross_sale,
        net_sale: m.net_sale,
        sales_count: m.sales_count,
        avg_ticket: m.avg_ticket,
        gross_margin: m.gross_margin,
        margin_pct: m.margin_pct,
        refunds: m.refunds,
        tips: money(tips),
        cost_missing_lines: m.cost_missing_lines,
    })
}

/// Carga todas las métricas, gráficas y alertas operativas del panel principal.
pub async fn get(
    pool: &PgPool,
    claims: &Claims,
    q: DashboardQuery,
) -> Result<DashboardResponse, ApiError> {
    let (scope, mut tx) = if let Some(bid) = q.branch_id {
        tenancy::begin_in_branch(pool, claims, bid).await?
    } else {
        tenancy::begin(pool, claims).await?
    };
    scope.require_permission("reports:read")?;

    let branch_filter = q.branch_id.or(scope.branch);
    let (from, to) = match (q.from, q.to) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            let today = local_today_tx(&mut tx, scope.tenant, branch_filter).await?;
            let to = q.to.unwrap_or(today);
            let from = q.from.unwrap_or(to);
            (from, to)
        }
    };

    if from > to {
        return Err(error::validation("from must be <= to"));
    }
    if (to - from).num_days() > MAX_RANGE_DAYS {
        return Err(error::validation("date range too large"));
    }

    let previous =
        previous_period(from, to).ok_or_else(|| error::validation("invalid date range"))?;

    let kpis = load_kpis_tx(&mut tx, scope.tenant, branch_filter, from, to).await?;
    let kpis_previous = load_kpis_tx(
        &mut tx,
        scope.tenant,
        branch_filter,
        previous.from,
        previous.to,
    )
    .await?;

    let hour_rows: Vec<(i32, Decimal, i64)> = sqlx::query_as(
        "select v.local_hour,
                coalesce(sum(v.gross_sale), 0),
                count(*)::bigint
           from public.v_sales v
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by v.local_hour
          order by v.local_hour",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    let by_hour = fill_hours(&hour_rows);

    let day_rows: Vec<(NaiveDate, Decimal, i64)> = sqlx::query_as(
        "select v.local_date,
                coalesce(sum(v.gross_sale), 0),
                count(*)::bigint
           from public.v_sales v
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by v.local_date
          order by v.local_date",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    let by_day = fill_days(from, to, &day_rows);

    let mut top_products = sqlx::query_as::<_, TopProduct>(
        "select oi.product_id,
                coalesce(p.name, oi.custom_name, 'Ítem libre') as name,
                coalesce(sum(oi.qty), 0) as qty,
                coalesce(sum(oi.line_total), 0) as line_total
           from public.order_items oi
           join public.v_sales v on v.id = oi.order_id
           left join public.products p on p.id = oi.product_id
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by oi.product_id, coalesce(p.name, oi.custom_name, 'Ítem libre')
          order by line_total desc, qty desc, name asc
          limit 10",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    for item in &mut top_products {
        item.line_total = money(item.line_total);
    }

    let mut payment_methods = sqlx::query_as::<_, PaymentMethodSummary>(
        "select tm.tima_codigo as method,
                coalesce(sum(pay.amount), 0) as amount,
                count(distinct v.id)::bigint as count
           from public.v_sales v
           join public.payments pay on pay.order_id = v.id
           join public.tbl_tipos_maestra tm on tm.tima_id = pay.method_id
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by tm.tima_codigo
          order by amount desc, tm.tima_codigo asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    for item in &mut payment_methods {
        item.amount = money(item.amount);
    }

    let low_stock = sqlx::query_as::<_, LowStockItem>(
        "select sa.product_id,
                p.name,
                l.name as location_name,
                coalesce(s.on_hand, sa.qty) as on_hand,
                coalesce(s.min_qty, 0) as min_qty
           from public.stock_alerts sa
           join public.products p on p.id = sa.product_id
           join public.locations l on l.id = sa.location_id
           left join public.stock s
             on s.location_id = sa.location_id
            and s.product_id = sa.product_id
          where sa.tenant_id = $1
            and ($2::uuid is null or sa.branch_id = $2)
            and sa.kind = 'BAJO'
            and sa.resolved_at is null
          order by (coalesce(s.on_hand, sa.qty) - coalesce(s.min_qty, 0)) asc,
                   sa.created_at desc,
                   p.name asc
          limit 10",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .fetch_all(&mut *tx)
    .await?;

    let (overdue_count, overdue_amount): (i64, Decimal) = sqlx::query_as(
        "with acct as (
            select (r.amount_total - r.amount_paid) as balance,
                   case
                       when (r.amount_total - r.amount_paid) <= 0 then 0
                       else greatest(0, ((now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)))
                   end as days_overdue
              from public.receivables r
              join public.branches b on b.id = r.branch_id
              left join lateral (
                  select min(ri.due_date) as first_due
                    from public.receivable_installments ri
                   where ri.receivable_id = r.id and ri.amount_paid < ri.amount
              ) i on true
             where r.tenant_id = $1
               and ($2::uuid is null or r.branch_id = $2)
               and r.status in ('ABIERTA', 'PARCIAL')
         )
         select count(*)::bigint,
                coalesce(sum(balance), 0)
           from acct
          where days_overdue > 0",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(DashboardResponse {
        period: DateRange { from, to },
        previous,
        kpis,
        kpis_previous,
        by_hour,
        by_day,
        top_products,
        payment_methods,
        low_stock,
        overdue_receivables: OverdueReceivablesSummary {
            count: overdue_count,
            amount: money(overdue_amount),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;
    use axum::http::Uri;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal literal")
    }

    fn day(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
    }

    #[test]
    fn previous_period_matches_duration() {
        assert_eq!(
            previous_period(day(2026, 11, 10), day(2026, 11, 10)),
            Some(DateRange {
                from: day(2026, 11, 9),
                to: day(2026, 11, 9),
            })
        );
        assert_eq!(
            previous_period(day(2026, 11, 1), day(2026, 11, 7)),
            Some(DateRange {
                from: day(2026, 10, 25),
                to: day(2026, 10, 31),
            })
        );
        assert_eq!(previous_period(day(2026, 11, 8), day(2026, 11, 7)), None);
    }

    #[test]
    fn fill_hours_and_days_zero_fill_gaps() {
        let hours = fill_hours(&[(9, d("12500.5"), 2), (23, d("4000"), 1)]);
        assert_eq!(hours.len(), 24);
        assert_eq!(hours[0].hour, 0);
        assert_eq!(hours[0].gross_sale, Decimal::ZERO);
        assert_eq!(hours[9].gross_sale, d("12500.50"));
        assert_eq!(hours[9].sales_count, 2);
        assert_eq!(hours[23].gross_sale, d("4000.00"));

        let days = fill_days(
            day(2026, 11, 8),
            day(2026, 11, 10),
            &[(day(2026, 11, 9), d("30000"), 3)],
        );
        assert_eq!(days.len(), 3);
        assert_eq!(days[0].date, day(2026, 11, 8));
        assert_eq!(days[0].sales_count, 0);
        assert_eq!(days[1].date, day(2026, 11, 9));
        assert_eq!(days[1].gross_sale, d("30000.00"));
        assert_eq!(days[2].date, day(2026, 11, 10));
        assert_eq!(days[2].gross_sale, Decimal::ZERO);
    }

    #[test]
    fn query_treats_empty_strings_as_none() {
        let uri: Uri = "/v1/reports/dashboard?branch_id=&from=&to="
            .parse()
            .unwrap();
        let Query(q) = Query::<DashboardQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(q.branch_id, None);
        assert_eq!(q.from, None);
        assert_eq!(q.to, None);

        let uri_set: Uri = "/v1/reports/dashboard?branch_id=11111111-1111-1111-1111-111111111111&from=2026-11-01&to=2026-11-10"
            .parse()
            .unwrap();
        let Query(q2) = Query::<DashboardQuery>::try_from_uri(&uri_set).unwrap();
        assert_eq!(
            q2.branch_id,
            Some(Uuid::from_str("11111111-1111-1111-1111-111111111111").unwrap())
        );
        assert_eq!(q2.from, Some(day(2026, 11, 1)));
        assert_eq!(q2.to, Some(day(2026, 11, 10)));
    }
}

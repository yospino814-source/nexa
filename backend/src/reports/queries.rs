//! Endpoints de reportes analíticos y exportación CSV (S7 / F7.3).
//!
//! Todos los reportes exigen permiso `reports:read`, respetan RLS con `branch_id`
//! opcional (`tenancy::begin_in_branch` vs `tenancy::begin`) y calculan sus
//! métricas únicamente con `crate::reports::metrics` sobre `public.v_sales`.

use axum::Json;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::money::money;
use crate::reports::csv;
use crate::reports::dashboard::{self, DateRange, MAX_RANGE_DAYS, empty_string_as_none};
use crate::reports::metrics;
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Default, Deserialize)]
pub struct ReportQuery {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub branch_id: Option<Uuid>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub from: Option<NaiveDate>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub to: Option<NaiveDate>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Json,
    Csv,
}

impl ReportFormat {
    pub fn parse(raw: Option<&str>, allow_csv: bool) -> Result<Self, ApiError> {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            None => Ok(Self::Json),
            Some(s) if s.eq_ignore_ascii_case("json") => Ok(Self::Json),
            Some(s) if s.eq_ignore_ascii_case("csv") => {
                if allow_csv {
                    Ok(Self::Csv)
                } else {
                    Err(error::validation(
                        "format=csv is not supported for this report",
                    ))
                }
            }
            Some(_) => Err(error::validation("format must be json or csv")),
        }
    }
}

/// Resultado tipado de un reporte con soporte opcional de exportación CSV.
pub enum ReportOutput<T> {
    Json(T),
    Csv {
        slug: &'static str,
        period: DateRange,
        body: String,
    },
}

impl<T: Serialize> IntoResponse for ReportOutput<T> {
    fn into_response(self) -> Response {
        match self {
            Self::Json(data) => Json(data).into_response(),
            Self::Csv { slug, period, body } => {
                csv::csv_response(slug, period.from, period.to, body)
            }
        }
    }
}

pub fn method_label(code: &str) -> &str {
    match code {
        "EFECTIVO" => "Efectivo",
        "DATAFONO" => "Datáfono",
        "QR" => "QR",
        "CREDITO" => "Crédito",
        other => other,
    }
}

/// Calcula días de inventario = `on_hand / (vendidas_30d / 30)` sin perder precisión intermedia.
/// Devuelve `None` si `sold_30d <= 0` ("vacío si no vendió").
pub fn inventory_days(on_hand: Decimal, sold_30d: Decimal) -> Option<Decimal> {
    if sold_30d <= Decimal::ZERO {
        None
    } else {
        Some(money((on_hand * Decimal::from(30)) / sold_30d))
    }
}

async fn begin_report_tx<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    branch_id: Option<Uuid>,
) -> Result<(Scope, Tx<'a>), ApiError> {
    let (scope, tx) = if let Some(bid) = branch_id {
        tenancy::begin_in_branch(pool, claims, bid).await?
    } else {
        tenancy::begin(pool, claims).await?
    };
    scope.require_permission("reports:read")?;
    Ok((scope, tx))
}

async fn resolve_range_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    branch_filter: Option<Uuid>,
    from_q: Option<NaiveDate>,
    to_q: Option<NaiveDate>,
) -> Result<DateRange, ApiError> {
    let (from, to) = match (from_q, to_q) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            let today = dashboard::local_today_tx(tx, tenant_id, branch_filter).await?;
            let to = to_q.unwrap_or(today);
            let from = from_q.unwrap_or(to);
            (from, to)
        }
    };
    if from > to {
        return Err(error::validation("from must be <= to"));
    }
    if (to - from).num_days() > MAX_RANGE_DAYS {
        return Err(error::validation("date range too large"));
    }
    Ok(DateRange { from, to })
}

// ============================================================
// 1. /v1/reports/sales-by-day
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByDayRow {
    pub fecha: NaiveDate,
    pub ventas: i64,
    pub venta_bruta: Decimal,
    pub devoluciones: Decimal,
    pub venta_neta: Decimal,
    pub ticket_promedio: Decimal,
    pub margen: Decimal,
    pub cost_missing_lines: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByDayResponse {
    pub period: DateRange,
    pub cost_missing_lines: i64,
    pub rows: Vec<SalesByDayRow>,
}

impl SalesByDayResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.fecha.to_string(),
                    r.ventas.to_string(),
                    csv::format_decimal_2(r.venta_bruta),
                    csv::format_decimal_2(r.devoluciones),
                    csv::format_decimal_2(r.venta_neta),
                    csv::format_decimal_2(r.ticket_promedio),
                    csv::format_decimal_2(r.margen),
                ]
            })
            .collect();
        csv::build_csv(
            &[
                "fecha",
                "ventas",
                "venta_bruta",
                "devoluciones",
                "venta_neta",
                "ticket_promedio",
                "margen",
            ],
            &rows,
        )
    }
}

pub async fn sales_by_day_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesByDayResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct DayAgg {
        fecha: NaiveDate,
        ventas: i64,
        venta_bruta: Decimal,
        devoluciones: Decimal,
        line_total_sum: Decimal,
        cost_of_sales: Decimal,
        cost_missing_lines: i64,
    }

    let raw = sqlx::query_as::<_, DayAgg>(
        "with daily_sales as (
             select v.local_date as fecha,
                    count(*)::bigint as ventas,
                    coalesce(sum(v.gross_sale), 0) as venta_bruta
               from public.v_sales v
              where v.tenant_id = $1
                and ($2::uuid is null or v.branch_id = $2)
                and v.local_date between $3 and $4
              group by v.local_date
         ),
         daily_refunds as (
             select (r.decided_at at time zone b.timezone)::date as fecha,
                    coalesce(sum(r.total), 0) as devoluciones
               from public.refunds r
               join public.branches b on b.id = r.branch_id
              where r.tenant_id = $1
                and ($2::uuid is null or r.branch_id = $2)
                and r.status = 'APROBADA'
                and r.decided_at is not null
                and (r.decided_at at time zone b.timezone)::date between $3 and $4
              group by (r.decided_at at time zone b.timezone)::date
         ),
         daily_items as (
             select v.local_date as fecha,
                    coalesce(sum(oi.line_total), 0) as line_total_sum,
                    coalesce(sum(coalesce(oi.cost_total, 0)), 0) as cost_of_sales,
                    count(*) filter (where oi.cost_missing)::bigint as cost_missing_lines
               from public.order_items oi
               join public.v_sales v on v.id = oi.order_id
              where v.tenant_id = $1
                and ($2::uuid is null or v.branch_id = $2)
                and v.local_date between $3 and $4
              group by v.local_date
         )
         select ds.fecha,
                ds.ventas,
                ds.venta_bruta,
                coalesce(dr.devoluciones, 0) as devoluciones,
                coalesce(di.line_total_sum, 0) as line_total_sum,
                coalesce(di.cost_of_sales, 0) as cost_of_sales,
                coalesce(di.cost_missing_lines, 0) as cost_missing_lines
           from daily_sales ds
           left join daily_refunds dr on dr.fecha = ds.fecha
           left join daily_items di on di.fecha = ds.fecha
          order by ds.fecha asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut total_cost_missing = 0i64;
    let rows = raw
        .into_iter()
        .map(|r| {
            total_cost_missing += r.cost_missing_lines;
            let m = metrics::from_totals(
                r.venta_bruta,
                r.devoluciones,
                r.ventas,
                r.line_total_sum,
                r.cost_of_sales,
                r.cost_missing_lines,
            );
            SalesByDayRow {
                fecha: r.fecha,
                ventas: m.sales_count,
                venta_bruta: m.gross_sale,
                devoluciones: m.refunds,
                venta_neta: m.net_sale,
                ticket_promedio: m.avg_ticket,
                margen: m.gross_margin,
                cost_missing_lines: m.cost_missing_lines,
            }
        })
        .collect();

    Ok(SalesByDayResponse {
        period,
        cost_missing_lines: total_cost_missing,
        rows,
    })
}

pub async fn sales_by_day(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<SalesByDayResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = sales_by_day_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "sales-by-day",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 2. /v1/reports/sales-by-product
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByProductRow {
    pub product_id: Option<Uuid>,
    pub sku: Option<String>,
    pub producto: String,
    pub categoria: String,
    pub cantidad: Decimal,
    pub venta_sin_impuesto: Decimal,
    pub costo: Decimal,
    pub margen: Decimal,
    pub pct_margen: Decimal,
    pub cost_missing_lines: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByProductResponse {
    pub period: DateRange,
    pub cost_missing_lines: i64,
    pub rows: Vec<SalesByProductRow>,
}

impl SalesByProductResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.sku.clone().unwrap_or_default(),
                    r.producto.clone(),
                    r.categoria.clone(),
                    csv::format_qty(r.cantidad),
                    csv::format_decimal_2(r.venta_sin_impuesto),
                    csv::format_decimal_2(r.costo),
                    csv::format_decimal_2(r.margen),
                    csv::format_decimal_2(r.pct_margen),
                ]
            })
            .collect();
        csv::build_csv(
            &[
                "sku",
                "producto",
                "categoría",
                "cantidad",
                "venta_sin_impuesto",
                "costo",
                "margen",
                "pct_margen",
            ],
            &rows,
        )
    }
}

pub async fn sales_by_product_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesByProductResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct ProdAgg {
        product_id: Option<Uuid>,
        sku: Option<String>,
        producto: String,
        categoria: String,
        cantidad: Decimal,
        venta_sin_impuesto: Decimal,
        costo: Decimal,
        cost_missing_lines: i64,
    }

    let raw = sqlx::query_as::<_, ProdAgg>(
        "select oi.product_id,
                p.sku,
                coalesce(p.name, oi.custom_name, 'Ítem libre') as producto,
                coalesce(min(pc.name), 'Sin categoría') as categoria,
                coalesce(sum(oi.qty), 0) as cantidad,
                coalesce(sum(oi.line_total), 0) as venta_sin_impuesto,
                coalesce(sum(coalesce(oi.cost_total, 0)), 0) as costo,
                count(*) filter (where oi.cost_missing)::bigint as cost_missing_lines
           from public.order_items oi
           join public.v_sales v on v.id = oi.order_id
           left join public.products p on p.id = oi.product_id
           left join public.branch_products bp
             on bp.branch_id = oi.branch_id
            and bp.product_id = oi.product_id
           left join public.product_categories pc on pc.id = bp.category_id
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by oi.product_id, p.sku, coalesce(p.name, oi.custom_name, 'Ítem libre')
          order by venta_sin_impuesto desc, cantidad desc, producto asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut total_cost_missing = 0i64;
    let rows = raw
        .into_iter()
        .map(|r| {
            total_cost_missing += r.cost_missing_lines;
            let venta = money(r.venta_sin_impuesto);
            let costo = money(r.costo);
            let margen = metrics::gross_margin(venta, costo);
            let pct_margen = metrics::margin_pct(margen, venta);
            SalesByProductRow {
                product_id: r.product_id,
                sku: r.sku,
                producto: r.producto,
                categoria: r.categoria,
                cantidad: r.cantidad,
                venta_sin_impuesto: venta,
                costo,
                margen,
                pct_margen,
                cost_missing_lines: r.cost_missing_lines,
            }
        })
        .collect();

    Ok(SalesByProductResponse {
        period,
        cost_missing_lines: total_cost_missing,
        rows,
    })
}

pub async fn sales_by_product(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<SalesByProductResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = sales_by_product_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "sales-by-product",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 3. /v1/reports/sales-by-category
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByCategoryRow {
    pub categoria: String,
    pub cantidad: Decimal,
    pub venta_sin_impuesto: Decimal,
    pub costo: Decimal,
    pub margen: Decimal,
    pub pct_margen: Decimal,
    pub cost_missing_lines: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByCategoryResponse {
    pub period: DateRange,
    pub cost_missing_lines: i64,
    pub rows: Vec<SalesByCategoryRow>,
}

impl SalesByCategoryResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.categoria.clone(),
                    csv::format_qty(r.cantidad),
                    csv::format_decimal_2(r.venta_sin_impuesto),
                    csv::format_decimal_2(r.margen),
                ]
            })
            .collect();
        csv::build_csv(
            &["categoría", "cantidad", "venta_sin_impuesto", "margen"],
            &rows,
        )
    }
}

pub async fn sales_by_category_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesByCategoryResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct CatAgg {
        categoria: String,
        cantidad: Decimal,
        venta_sin_impuesto: Decimal,
        costo: Decimal,
        cost_missing_lines: i64,
    }

    let raw = sqlx::query_as::<_, CatAgg>(
        "select coalesce(pc.name, 'Sin categoría') as categoria,
                coalesce(sum(oi.qty), 0) as cantidad,
                coalesce(sum(oi.line_total), 0) as venta_sin_impuesto,
                coalesce(sum(coalesce(oi.cost_total, 0)), 0) as costo,
                count(*) filter (where oi.cost_missing)::bigint as cost_missing_lines
           from public.order_items oi
           join public.v_sales v on v.id = oi.order_id
           left join public.branch_products bp
             on bp.branch_id = oi.branch_id
            and bp.product_id = oi.product_id
           left join public.product_categories pc on pc.id = bp.category_id
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by coalesce(pc.name, 'Sin categoría')
          order by venta_sin_impuesto desc, categoria asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut total_cost_missing = 0i64;
    let rows = raw
        .into_iter()
        .map(|r| {
            total_cost_missing += r.cost_missing_lines;
            let venta = money(r.venta_sin_impuesto);
            let costo = money(r.costo);
            let margen = metrics::gross_margin(venta, costo);
            let pct_margen = metrics::margin_pct(margen, venta);
            SalesByCategoryRow {
                categoria: r.categoria,
                cantidad: r.cantidad,
                venta_sin_impuesto: venta,
                costo,
                margen,
                pct_margen,
                cost_missing_lines: r.cost_missing_lines,
            }
        })
        .collect();

    Ok(SalesByCategoryResponse {
        period,
        cost_missing_lines: total_cost_missing,
        rows,
    })
}

pub async fn sales_by_category(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<SalesByCategoryResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = sales_by_category_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "sales-by-category",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 4. /v1/reports/sales-by-cashier
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByCashierRow {
    pub cashier_id: Option<Uuid>,
    pub cajero: String,
    pub ventas: i64,
    pub venta_bruta: Decimal,
    pub ticket_promedio: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByCashierResponse {
    pub period: DateRange,
    pub rows: Vec<SalesByCashierRow>,
}

impl SalesByCashierResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.cajero.clone(),
                    r.ventas.to_string(),
                    csv::format_decimal_2(r.venta_bruta),
                    csv::format_decimal_2(r.ticket_promedio),
                ]
            })
            .collect();
        csv::build_csv(
            &["cajero", "ventas", "venta_bruta", "ticket_promedio"],
            &rows,
        )
    }
}

pub async fn sales_by_cashier_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesByCashierResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct CashierAgg {
        cashier_id: Option<Uuid>,
        cajero: String,
        ventas: i64,
        venta_bruta: Decimal,
    }

    let raw = sqlx::query_as::<_, CashierAgg>(
        "select fp.created_by as cashier_id,
                coalesce(u.full_name, u.email, 'Sin cajero') as cajero,
                count(*)::bigint as ventas,
                coalesce(sum(v.gross_sale), 0) as venta_bruta
           from public.v_sales v
           left join lateral (
               select p.created_by
                 from public.payments p
                where p.order_id = v.id
                order by p.created_at asc, p.id asc
                limit 1
           ) fp on true
           left join public.users u on u.id = fp.created_by
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by fp.created_by, coalesce(u.full_name, u.email, 'Sin cajero')
          order by venta_bruta desc, ventas desc, cajero asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let rows = raw
        .into_iter()
        .map(|r| {
            let venta_bruta = money(r.venta_bruta);
            let ticket_promedio = metrics::avg_ticket(venta_bruta, r.ventas);
            SalesByCashierRow {
                cashier_id: r.cashier_id,
                cajero: r.cajero,
                ventas: r.ventas,
                venta_bruta,
                ticket_promedio,
            }
        })
        .collect();

    Ok(SalesByCashierResponse { period, rows })
}

pub async fn sales_by_cashier(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<SalesByCashierResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = sales_by_cashier_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "sales-by-cashier",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 5. /v1/reports/sales-by-method
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByMethodRow {
    pub metodo: String,
    pub etiqueta: String,
    pub pagos: i64,
    pub monto: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesByMethodResponse {
    pub period: DateRange,
    pub rows: Vec<SalesByMethodRow>,
}

impl SalesByMethodResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.etiqueta.clone(),
                    r.pagos.to_string(),
                    csv::format_decimal_2(r.monto),
                ]
            })
            .collect();
        csv::build_csv(&["método", "pagos", "monto"], &rows)
    }
}

pub async fn sales_by_method_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesByMethodResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct MethodAgg {
        metodo: String,
        pagos: i64,
        monto: Decimal,
    }

    let raw = sqlx::query_as::<_, MethodAgg>(
        "select tm.tima_codigo as metodo,
                count(pay.id)::bigint as pagos,
                coalesce(sum(pay.amount), 0) as monto
           from public.v_sales v
           join public.payments pay on pay.order_id = v.id
           join public.tbl_tipos_maestra tm on tm.tima_id = pay.method_id
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by tm.tima_codigo
          order by monto desc, tm.tima_codigo asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let rows = raw
        .into_iter()
        .map(|r| SalesByMethodRow {
            etiqueta: method_label(&r.metodo).to_string(),
            metodo: r.metodo,
            pagos: r.pagos,
            monto: money(r.monto),
        })
        .collect();

    Ok(SalesByMethodResponse { period, rows })
}

pub async fn sales_by_method(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<SalesByMethodResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = sales_by_method_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "sales-by-method",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 6. /v1/reports/sales-heatmap (sin CSV)
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesHeatmapRow {
    pub dia: i32,
    pub hora: i32,
    pub ventas: i64,
    pub venta_bruta: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesHeatmapResponse {
    pub period: DateRange,
    pub rows: Vec<SalesHeatmapRow>,
}

/// Rellena la matriz completa de 7 días ISO (`1..=7`) × 24 horas (`0..=23`) en orden ascendente.
pub fn fill_heatmap(entries: &[(i32, i32, i64, Decimal)]) -> Vec<SalesHeatmapRow> {
    let mut by_slot = HashMap::with_capacity(entries.len());
    for &(dia, hora, ventas, venta_bruta) in entries {
        by_slot.insert((dia, hora), (ventas, money(venta_bruta)));
    }
    let mut out = Vec::with_capacity(7 * 24);
    for dia in 1..=7 {
        for hora in 0..24 {
            let (ventas, venta_bruta) = by_slot
                .get(&(dia, hora))
                .copied()
                .unwrap_or((0, Decimal::ZERO));
            out.push(SalesHeatmapRow {
                dia,
                hora,
                ventas,
                venta_bruta,
            });
        }
    }
    out
}

pub async fn sales_heatmap(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<SalesHeatmapResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), false)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    let raw: Vec<(i32, i32, i64, Decimal)> = sqlx::query_as(
        "select v.local_isodow as dia,
                v.local_hour as hora,
                count(*)::bigint as ventas,
                coalesce(sum(v.gross_sale), 0) as venta_bruta
           from public.v_sales v
          where v.tenant_id = $1
            and ($2::uuid is null or v.branch_id = $2)
            and v.local_date between $3 and $4
          group by v.local_isodow, v.local_hour
          order by v.local_isodow asc, v.local_hour asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(SalesHeatmapResponse {
        period,
        rows: fill_heatmap(&raw),
    })
}

// ============================================================
// 7. /v1/reports/inventory-valuation
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryValuationRow {
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub sucursal: String,
    pub producto: String,
    pub ubicacion: String,
    pub existencias: Decimal,
    pub costo_promedio: Decimal,
    pub valor: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryValuationResponse {
    pub period: DateRange,
    pub total_valor: Decimal,
    pub rows: Vec<InventoryValuationRow>,
}

impl InventoryValuationResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.producto.clone(),
                    r.ubicacion.clone(),
                    csv::format_qty(r.existencias),
                    csv::format_decimal_2(r.costo_promedio),
                    csv::format_decimal_2(r.valor),
                ]
            })
            .collect();
        csv::build_csv(
            &[
                "producto",
                "ubicación",
                "existencias",
                "costo_promedio",
                "valor",
            ],
            &rows,
        )
    }
}

pub async fn inventory_valuation_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<InventoryValuationResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct ValuationAgg {
        product_id: Uuid,
        location_id: Uuid,
        sucursal: String,
        producto: String,
        ubicacion: String,
        existencias: Decimal,
        costo_promedio: Decimal,
    }

    let raw = sqlx::query_as::<_, ValuationAgg>(
        "select s.product_id,
                s.location_id,
                b.name as sucursal,
                p.name as producto,
                l.name as ubicacion,
                s.on_hand as existencias,
                s.avg_cost as costo_promedio
           from public.stock s
           join public.products p on p.id = s.product_id
           join public.locations l on l.id = s.location_id
           join public.branches b on b.id = s.branch_id
          where s.tenant_id = $1
            and ($2::uuid is null or s.branch_id = $2)
            and s.on_hand <> 0
          order by (s.on_hand * s.avg_cost) desc, p.name asc, l.name asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut total_valor = Decimal::ZERO;
    let rows = raw
        .into_iter()
        .map(|r| {
            let valor = money(r.existencias * r.costo_promedio);
            total_valor = money(total_valor + valor);
            InventoryValuationRow {
                product_id: r.product_id,
                location_id: r.location_id,
                sucursal: r.sucursal,
                producto: r.producto,
                ubicacion: r.ubicacion,
                existencias: r.existencias,
                costo_promedio: money(r.costo_promedio),
                valor,
            }
        })
        .collect();

    Ok(InventoryValuationResponse {
        period,
        total_valor,
        rows,
    })
}

pub async fn inventory_valuation(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<InventoryValuationResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = inventory_valuation_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "inventory-valuation",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 8. /v1/reports/inventory-rotation
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryRotationRow {
    pub product_id: Uuid,
    pub producto: String,
    pub vendidas_30d: Decimal,
    pub existencias: Decimal,
    pub dias_inventario: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryRotationResponse {
    pub period: DateRange,
    pub rows: Vec<InventoryRotationRow>,
}

impl InventoryRotationResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.producto.clone(),
                    csv::format_qty(r.vendidas_30d),
                    csv::format_qty(r.existencias),
                    r.dias_inventario
                        .map(csv::format_decimal_2)
                        .unwrap_or_default(),
                ]
            })
            .collect();
        csv::build_csv(
            &["producto", "vendidas_30d", "existencias", "dias_inventario"],
            &rows,
        )
    }
}

pub async fn inventory_rotation_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<InventoryRotationResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);

    // Ignora `from` y toma siempre los últimos 30 días cerrados hasta `to`.
    let to = match q.to {
        Some(t) => t,
        None => dashboard::local_today_tx(&mut tx, scope.tenant, branch_filter).await?,
    };
    let window_from = to - Duration::days(29);
    let period = DateRange {
        from: window_from,
        to,
    };

    #[derive(sqlx::FromRow)]
    struct RotationAgg {
        product_id: Uuid,
        producto: String,
        vendidas_30d: Decimal,
        existencias: Decimal,
    }

    let raw = sqlx::query_as::<_, RotationAgg>(
        "with stock_by_prod as (
             select s.product_id,
                    coalesce(sum(s.on_hand), 0) as existencias
               from public.stock s
              where s.tenant_id = $1
                and ($2::uuid is null or s.branch_id = $2)
              group by s.product_id
         ),
         sold_by_prod as (
             select oi.product_id,
                    coalesce(sum(oi.qty * oi.units_per_package), 0) as vendidas_30d
               from public.order_items oi
               join public.v_sales v on v.id = oi.order_id
              where v.tenant_id = $1
                and ($2::uuid is null or v.branch_id = $2)
                and oi.product_id is not null
                and v.local_date between $3 and $4
              group by oi.product_id
         )
         select p.id as product_id,
                p.name as producto,
                coalesce(so.vendidas_30d, 0) as vendidas_30d,
                coalesce(st.existencias, 0) as existencias
           from public.products p
           left join stock_by_prod st on st.product_id = p.id
           left join sold_by_prod so on so.product_id = p.id
          where p.tenant_id = $1
            and not p.has_variants
            and not p.is_composed
            and p.kind <> 'SERVICIO'
            and (
                st.product_id is not null
                or so.product_id is not null
                or (
                    p.active
                    and exists (
                        select 1
                          from public.branch_products bp
                         where bp.tenant_id = $1
                           and bp.product_id = p.id
                           and ($2::uuid is null or bp.branch_id = $2)
                    )
                )
            )
          order by coalesce(so.vendidas_30d, 0) desc, p.name asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let rows = raw
        .into_iter()
        .map(|r| {
            let dias_inventario = inventory_days(r.existencias, r.vendidas_30d);
            InventoryRotationRow {
                product_id: r.product_id,
                producto: r.producto,
                vendidas_30d: r.vendidas_30d,
                existencias: r.existencias,
                dias_inventario,
            }
        })
        .collect();

    Ok(InventoryRotationResponse { period, rows })
}

pub async fn inventory_rotation(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<InventoryRotationResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = inventory_rotation_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "inventory-rotation",
            period: resp.period,
            body: resp.to_csv(),
        },
    })
}

// ============================================================
// 9. /v1/reports/purchases-by-supplier
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchasesBySupplierRow {
    pub supplier_id: Uuid,
    pub proveedor: String,
    pub ordenes: i64,
    pub recibido: Decimal,
    pub pagado: Decimal,
    pub saldo: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchasesBySupplierResponse {
    pub period: DateRange,
    pub rows: Vec<PurchasesBySupplierRow>,
}

impl PurchasesBySupplierResponse {
    pub fn to_csv(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.proveedor.clone(),
                    r.ordenes.to_string(),
                    csv::format_decimal_2(r.recibido),
                    csv::format_decimal_2(r.pagado),
                    csv::format_decimal_2(r.saldo),
                ]
            })
            .collect();
        csv::build_csv(
            &["proveedor", "órdenes", "recibido", "pagado", "saldo"],
            &rows,
        )
    }
}

pub async fn purchases_by_supplier_data(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<PurchasesBySupplierResponse, ApiError> {
    let _ = ReportFormat::parse(q.format.as_deref(), true)?;
    let (scope, mut tx) = begin_report_tx(pool, claims, q.branch_id).await?;
    let branch_filter = q.branch_id.or(scope.branch);
    let period = resolve_range_tx(&mut tx, scope.tenant, branch_filter, q.from, q.to).await?;

    #[derive(sqlx::FromRow)]
    struct SupplierAgg {
        supplier_id: Uuid,
        proveedor: String,
        ordenes: i64,
        recibido: Decimal,
        pagado: Decimal,
        saldo: Decimal,
    }

    let raw = sqlx::query_as::<_, SupplierAgg>(
        "select sup.id as supplier_id,
                sup.name as proveedor,
                count(pr.id)::bigint as ordenes,
                coalesce(sum(coalesce(pay.amount_total, 0)), 0) as recibido,
                coalesce(sum(coalesce(pay.amount_paid, 0)), 0) as pagado,
                coalesce(sum(coalesce(
                    case
                        when pay.status = 'CASTIGADA' then 0
                        else greatest(0, pay.amount_total - pay.amount_paid)
                    end,
                    0
                )), 0) as saldo
           from public.purchase_receipts pr
           join public.branches b on b.id = pr.branch_id
           join public.suppliers sup on sup.id = pr.supplier_id
           join public.tbl_tipos_maestra tm on tm.tima_id = pr.status_id
           left join public.payables pay
             on pay.source_type = 'COMPRA'
            and pay.source_id = pr.id
            and pay.tenant_id = pr.tenant_id
          where pr.tenant_id = $1
            and ($2::uuid is null or pr.branch_id = $2)
            and tm.tima_codigo <> 'ANULADA'
            and (pr.created_at at time zone b.timezone)::date between $3 and $4
          group by sup.id, sup.name
          order by recibido desc, ordenes desc, proveedor asc",
    )
    .bind(scope.tenant)
    .bind(branch_filter)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let rows = raw
        .into_iter()
        .map(|r| PurchasesBySupplierRow {
            supplier_id: r.supplier_id,
            proveedor: r.proveedor,
            ordenes: r.ordenes,
            recibido: money(r.recibido),
            pagado: money(r.pagado),
            saldo: money(r.saldo),
        })
        .collect();

    Ok(PurchasesBySupplierResponse { period, rows })
}

pub async fn purchases_by_supplier(
    pool: &PgPool,
    claims: &Claims,
    q: ReportQuery,
) -> Result<ReportOutput<PurchasesBySupplierResponse>, ApiError> {
    let fmt = ReportFormat::parse(q.format.as_deref(), true)?;
    let resp = purchases_by_supplier_data(pool, claims, q).await?;
    Ok(match fmt {
        ReportFormat::Json => ReportOutput::Json(resp),
        ReportFormat::Csv => ReportOutput::Csv {
            slug: "purchases-by-supplier",
            period: resp.period,
            body: resp.to_csv(),
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
        Decimal::from_str(s).expect("decimal literal")
    }

    #[test]
    fn report_format_validates_values_and_csv_support() {
        assert_eq!(ReportFormat::parse(None, true).unwrap(), ReportFormat::Json);
        assert_eq!(
            ReportFormat::parse(Some(""), true).unwrap(),
            ReportFormat::Json
        );
        assert_eq!(
            ReportFormat::parse(Some("JSON"), true).unwrap(),
            ReportFormat::Json
        );
        assert_eq!(
            ReportFormat::parse(Some("csv"), true).unwrap(),
            ReportFormat::Csv
        );
        assert!(ReportFormat::parse(Some("csv"), false).is_err());
        assert!(ReportFormat::parse(Some("xlsx"), true).is_err());
    }

    #[test]
    fn inventory_days_handles_zero_and_fractional_sales() {
        assert_eq!(inventory_days(d("50"), Decimal::ZERO), None);
        assert_eq!(inventory_days(d("50"), d("-1")), None);
        assert_eq!(inventory_days(d("60"), d("60")), Some(d("30.00")));
        assert_eq!(inventory_days(d("10"), d("15")), Some(d("20.00")));
        assert_eq!(inventory_days(Decimal::ZERO, d("15")), Some(d("0.00")));
        // 1 unidad vendida en 30 días, 2 en stock => 2 / (1/30) = 60.00 días exactos.
        assert_eq!(inventory_days(d("2"), d("1")), Some(d("60.00")));
    }

    #[test]
    fn method_label_translates_credit_with_accent() {
        assert_eq!(method_label("CREDITO"), "Crédito");
        assert_eq!(method_label("EFECTIVO"), "Efectivo");
        assert_eq!(method_label("DATAFONO"), "Datáfono");
        assert_eq!(method_label("QR"), "QR");
    }

    #[test]
    fn fill_heatmap_generates_7x24_grid_in_order() {
        let cells = fill_heatmap(&[(1, 8, 2, d("25000.5")), (7, 23, 1, d("12000"))]);
        assert_eq!(cells.len(), 7 * 24);
        assert_eq!(cells[0].dia, 1);
        assert_eq!(cells[0].hora, 0);
        assert_eq!(cells[0].ventas, 0);
        assert_eq!(cells[8].dia, 1);
        assert_eq!(cells[8].hora, 8);
        assert_eq!(cells[8].ventas, 2);
        assert_eq!(cells[8].venta_bruta, d("25000.50"));
        let last = cells.last().unwrap();
        assert_eq!(last.dia, 7);
        assert_eq!(last.hora, 23);
        assert_eq!(last.ventas, 1);
        assert_eq!(last.venta_bruta, d("12000.00"));
    }

    #[test]
    fn report_query_treats_empty_strings_as_none() {
        let uri: Uri = "/v1/reports/sales-by-day?branch_id=&from=&to=&format="
            .parse()
            .unwrap();
        let Query(q) = Query::<ReportQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(q.branch_id, None);
        assert_eq!(q.from, None);
        assert_eq!(q.to, None);
        assert_eq!(q.format, None);
    }

    #[test]
    fn sales_by_product_csv_has_bom_and_spanish_headers() {
        let period = DateRange {
            from: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            to: NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(),
        };
        let resp = SalesByProductResponse {
            period,
            cost_missing_lines: 0,
            rows: vec![SalesByProductRow {
                product_id: None,
                sku: Some("LIC-001".into()),
                producto: "Aguardiente Antioqueño".into(),
                categoria: "Licores".into(),
                cantidad: d("3.0000"),
                venta_sin_impuesto: d("114000"),
                costo: d("72000"),
                margen: d("42000"),
                pct_margen: d("36.84"),
                cost_missing_lines: 0,
            }],
        };
        let body = resp.to_csv();
        assert!(body.starts_with('\u{FEFF}'));
        assert!(body.contains(
            "sku;producto;categoría;cantidad;venta_sin_impuesto;costo;margen;pct_margen"
        ));
        assert!(body.contains(
            "LIC-001;Aguardiente Antioqueño;Licores;3;114000,00;72000,00;42000,00;36,84"
        ));
    }
}

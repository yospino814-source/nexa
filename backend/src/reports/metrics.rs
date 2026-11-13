//! Definiciones únicas de métricas de ventas y margen (S5 / F7.1).
//!
//! Todos los reportes usan exclusivamente estas funciones para evitar fórmulas
//! duplicadas o inconsistencias entre pantallas y exportaciones.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::money::money;

/// Venta bruta de una orden de `v_sales`: `subtotal + tax` (sin propina).
#[allow(dead_code)]
pub fn order_gross_sale(subtotal: Decimal, tax: Decimal) -> Decimal {
    money(subtotal + tax)
}

/// Venta bruta acumulada: `Σ gross_sale` (sin propina).
#[allow(dead_code)]
pub fn gross_sale_sum<I>(gross_sales: I) -> Decimal
where
    I: IntoIterator<Item = Decimal>,
{
    money(gross_sales.into_iter().sum())
}

/// Fila mínima de una orden en `v_sales` para cálculo en memoria.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaleMetricInput {
    pub local_date: NaiveDate,
    pub gross_sale: Decimal,
}

/// Fila mínima de una devolución para calcular las devoluciones del periodo.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundMetricInput {
    pub status: String,
    pub local_decided_date: Option<NaiveDate>,
    pub total: Decimal,
}

/// Fila mínima de `order_items` para costo de venta, margen bruto y líneas sin costo.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderItemMetricInput {
    pub cancelled: bool,
    pub line_total: Decimal,
    pub cost_total: Option<Decimal>,
    pub cost_missing: bool,
}

/// Verifica si `local_date` cae dentro del intervalo cerrado `from..=to`.
#[allow(dead_code)]
pub fn in_period(local_date: NaiveDate, from: NaiveDate, to: NaiveDate) -> bool {
    local_date >= from && local_date <= to
}

/// Venta bruta del periodo filtrada con `local_date between from and to`:
/// `Σ gross_sale` (sin propina).
#[allow(dead_code)]
pub fn period_gross_sale<'a, I>(sales: I, from: NaiveDate, to: NaiveDate) -> Decimal
where
    I: IntoIterator<Item = &'a SaleMetricInput>,
{
    money(
        sales
            .into_iter()
            .filter(|s| in_period(s.local_date, from, to))
            .map(|s| s.gross_sale)
            .sum(),
    )
}

/// Número de ventas del periodo (`count(*)` de `v_sales` con `local_date between from and to`).
#[allow(dead_code)]
pub fn period_sales_count<'a, I>(sales: I, from: NaiveDate, to: NaiveDate) -> i64
where
    I: IntoIterator<Item = &'a SaleMetricInput>,
{
    let n = sales
        .into_iter()
        .filter(|s| in_period(s.local_date, from, to))
        .count();
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Devoluciones del periodo: `Σ refunds.total` con `status = 'APROBADA'`
/// y fecha local de `decided_at` dentro de `from..=to`.
#[allow(dead_code)]
pub fn approved_refunds_total<'a, I>(refunds: I, from: NaiveDate, to: NaiveDate) -> Decimal
where
    I: IntoIterator<Item = &'a RefundMetricInput>,
{
    money(
        refunds
            .into_iter()
            .filter(|r| {
                r.status == "APROBADA"
                    && r.local_decided_date.is_some_and(|d| in_period(d, from, to))
            })
            .map(|r| r.total)
            .sum(),
    )
}

/// Venta neta: `venta bruta - devoluciones del periodo`.
#[allow(dead_code)]
pub fn net_sale(gross_sale: Decimal, refunds: Decimal) -> Decimal {
    money(gross_sale - refunds)
}

/// Ticket promedio: `venta bruta / número de ventas` (`0` si no hay ventas).
#[allow(dead_code)]
pub fn avg_ticket(gross_sale: Decimal, sales_count: i64) -> Decimal {
    if sales_count <= 0 {
        Decimal::ZERO
    } else {
        money(gross_sale / Decimal::from(sales_count))
    }
}

/// Costo de venta: `Σ order_items.cost_total` de las líneas no canceladas.
#[allow(dead_code)]
pub fn cost_of_sales<'a, I>(items: I) -> Decimal
where
    I: IntoIterator<Item = &'a OrderItemMetricInput>,
{
    money(
        items
            .into_iter()
            .filter(|i| !i.cancelled)
            .filter_map(|i| i.cost_total)
            .sum(),
    )
}

/// Suma de `line_total` de las líneas no canceladas (base del margen bruto y % de margen).
#[allow(dead_code)]
pub fn net_line_total<'a, I>(items: I) -> Decimal
where
    I: IntoIterator<Item = &'a OrderItemMetricInput>,
{
    money(
        items
            .into_iter()
            .filter(|i| !i.cancelled)
            .map(|i| i.line_total)
            .sum(),
    )
}

/// Líneas sin costo: cantidad de líneas no canceladas con `cost_missing = true`.
#[allow(dead_code)]
pub fn cost_missing_lines<'a, I>(items: I) -> i64
where
    I: IntoIterator<Item = &'a OrderItemMetricInput>,
{
    let n = items
        .into_iter()
        .filter(|i| !i.cancelled && i.cost_missing)
        .count();
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Margen bruto: `Σ line_total - costo de venta`.
#[allow(dead_code)]
pub fn gross_margin(line_total_sum: Decimal, cost_of_sales: Decimal) -> Decimal {
    money(line_total_sum - cost_of_sales)
}

/// Porcentaje de margen: `margen / Σ line_total * 100` (`0` si `line_total_sum <= 0`).
#[allow(dead_code)]
pub fn margin_pct(gross_margin: Decimal, line_total_sum: Decimal) -> Decimal {
    if line_total_sum <= Decimal::ZERO {
        Decimal::ZERO
    } else {
        money((gross_margin * Decimal::from(100)) / line_total_sum)
    }
}

/// Conjunto completo de métricas derivadas para un periodo o fila agregada.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesMetrics {
    pub gross_sale: Decimal,
    pub refunds: Decimal,
    pub net_sale: Decimal,
    pub sales_count: i64,
    pub avg_ticket: Decimal,
    pub line_total: Decimal,
    pub cost_of_sales: Decimal,
    pub gross_margin: Decimal,
    pub margin_pct: Decimal,
    pub cost_missing_lines: i64,
}

/// Calcula todas las métricas derivadas a partir de los acumulados base usando
/// las fórmulas únicas del módulo.
#[allow(dead_code)]
pub fn from_totals(
    gross_sale: Decimal,
    refunds: Decimal,
    sales_count: i64,
    line_total_sum: Decimal,
    cost_of_sales: Decimal,
    cost_missing_lines: i64,
) -> SalesMetrics {
    let gross_sale = money(gross_sale);
    let refunds = money(refunds);
    let line_total = money(line_total_sum);
    let cost_of_sales = money(cost_of_sales);
    let net = net_sale(gross_sale, refunds);
    let ticket = avg_ticket(gross_sale, sales_count);
    let margin = gross_margin(line_total, cost_of_sales);
    let pct = margin_pct(margin, line_total);

    SalesMetrics {
        gross_sale,
        refunds,
        net_sale: net,
        sales_count,
        avg_ticket: ticket,
        line_total,
        cost_of_sales,
        gross_margin: margin,
        margin_pct: pct,
        cost_missing_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal literal")
    }

    fn day(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
    }

    #[test]
    fn metrics_avg_ticket_zero_sales() {
        assert_eq!(avg_ticket(Decimal::ZERO, 0), Decimal::ZERO);
        assert_eq!(avg_ticket(d("50000"), 0), Decimal::ZERO);
        assert_eq!(avg_ticket(d("10000"), -1), Decimal::ZERO);
        assert_eq!(avg_ticket(d("100000"), 3), d("33333.33"));
        assert_eq!(avg_ticket(d("150000"), 3), d("50000.00"));
    }

    #[test]
    fn metrics_margin_pct() {
        assert_eq!(margin_pct(Decimal::ZERO, Decimal::ZERO), Decimal::ZERO);
        assert_eq!(margin_pct(d("5000"), Decimal::ZERO), Decimal::ZERO);

        let margin = gross_margin(d("100000"), d("60000"));
        assert_eq!(margin, d("40000"));
        assert_eq!(margin_pct(margin, d("100000")), d("40.00"));

        // Redondeo a 2 decimales sin truncar antes de multiplicar por 100.
        assert_eq!(margin_pct(d("1000"), d("3000")), d("33.33"));
        assert_eq!(margin_pct(d("2000"), d("3000")), d("66.67"));

        // Margen negativo cuando el costo supera el neto sin impuesto.
        assert_eq!(margin_pct(d("-2500"), d("10000")), d("-25.00"));
    }

    #[test]
    fn metrics_net_sale() {
        assert_eq!(net_sale(d("120000"), d("20000")), d("100000"));
        assert_eq!(net_sale(d("50000"), Decimal::ZERO), d("50000"));
        // Si las devoluciones del periodo superan las ventas brutas, no se recorta a 0.
        assert_eq!(net_sale(d("30000"), d("45000.50")), d("-15000.50"));
    }

    #[test]
    fn metrics_period_and_item_formulas() {
        let from = day(2026, 11, 1);
        let to = day(2026, 11, 10);

        let sales = vec![
            SaleMetricInput {
                local_date: day(2026, 10, 31),
                gross_sale: d("99999"),
            },
            SaleMetricInput {
                local_date: from,
                gross_sale: order_gross_sale(d("10000"), d("1900")),
            },
            SaleMetricInput {
                local_date: to,
                gross_sale: order_gross_sale(d("20000"), d("0")),
            },
        ];

        assert_eq!(period_sales_count(&sales, from, to), 2);
        let gross = period_gross_sale(&sales, from, to);
        assert_eq!(gross, d("31900"));

        let refunds = vec![
            RefundMetricInput {
                status: "APROBADA".into(),
                local_decided_date: Some(day(2026, 11, 5)),
                total: d("11900"),
            },
            RefundMetricInput {
                status: "PENDIENTE".into(),
                local_decided_date: Some(day(2026, 11, 5)),
                total: d("5000"),
            },
            RefundMetricInput {
                status: "APROBADA".into(),
                local_decided_date: Some(day(2026, 11, 11)),
                total: d("8000"),
            },
        ];
        let refunds_total = approved_refunds_total(&refunds, from, to);
        assert_eq!(refunds_total, d("11900"));

        let items = vec![
            OrderItemMetricInput {
                cancelled: false,
                line_total: d("10000"),
                cost_total: Some(d("6500")),
                cost_missing: false,
            },
            OrderItemMetricInput {
                cancelled: false,
                line_total: d("20000"),
                cost_total: None,
                cost_missing: true,
            },
            OrderItemMetricInput {
                cancelled: true,
                line_total: d("9000"),
                cost_total: Some(d("4000")),
                cost_missing: true,
            },
        ];
        let line_sum = net_line_total(&items);
        let cost = cost_of_sales(&items);
        let missing = cost_missing_lines(&items);
        assert_eq!(line_sum, d("30000"));
        assert_eq!(cost, d("6500"));
        assert_eq!(missing, 1);

        let summary = from_totals(
            gross,
            refunds_total,
            period_sales_count(&sales, from, to),
            line_sum,
            cost,
            missing,
        );
        assert_eq!(summary.gross_sale, d("31900"));
        assert_eq!(summary.refunds, d("11900"));
        assert_eq!(summary.net_sale, d("20000"));
        assert_eq!(summary.sales_count, 2);
        assert_eq!(summary.avg_ticket, d("15950"));
        assert_eq!(summary.gross_margin, d("23500"));
        assert_eq!(summary.margin_pct, d("78.33"));
        assert_eq!(summary.cost_missing_lines, 1);
    }
}

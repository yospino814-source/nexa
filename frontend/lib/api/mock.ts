// Datos mock con la misma forma que el JSON del backend (ver ./types.ts).

import type {
  CatalogItem,
  DashboardKpis,
  DashboardQuery,
  DashboardResponse,
  Uuid,
} from "./types";

export const MOCK_BRANCH_ID: Uuid = "00000000-0000-4000-8000-000000000001";

export const mockSession = {
  user: { name: "Carlos M.", initials: "CM", role: "Cajero" },
  owner: { name: "Admin", initials: "AD", role: "Propietario" },
  branch: { id: MOCK_BRANCH_ID, name: "Sucursal Principal" },
  terminal: "Terminal #1 - Mostrador",
  shift: "Turno Mañana",
};

function item(
  n: number,
  name: string,
  category: string,
  price: number,
  tax: "IVA19" | null,
  available: number,
): CatalogItem {
  return {
    product_id: `10000000-0000-4000-8000-${String(n).padStart(12, "0")}`,
    name,
    category,
    price: price.toFixed(2),
    tax_code: tax,
    tax_rate: tax ? "0.19" : null,
    available: available.toFixed(3),
  };
}

export const mockCatalog: CatalogItem[] = [
  item(1, "Coca-Cola 400 ml", "Bebidas", 3500, "IVA19", 42),
  item(2, "Arroz Diana 500 g", "Abarrotes", 2800, null, 18),
  item(3, "Leche 1 L", "Lácteos", 4200, null, 6),
  item(4, "Pan tajado", "Panadería", 6900, null, 0),
  item(5, "Huevos AA x30", "Abarrotes", 15900, null, 11),
  item(6, "Jabón en barra", "Aseo y Hogar", 2300, "IVA19", 54),
  item(7, "Café 250 g", "Abarrotes", 9500, "IVA19", 8),
  item(8, "Cerveza 330 ml", "Bebidas", 2900, "IVA19", 3),
  item(9, "Yogur fresa 200 g", "Lácteos", 2600, null, 22),
  item(10, "Papas fritas 45 g", "Snacks y Dulces", 2200, "IVA19", 35),
  item(11, "Chocolatina 30 g", "Snacks y Dulces", 1500, "IVA19", 4),
  item(12, "Detergente 1 kg", "Aseo y Hogar", 12900, "IVA19", 15),
  item(13, "Aceite Premier 1 L", "Abarrotes", 12500, null, 0),
  item(14, "Atún Van Camp's 160 g", "Abarrotes", 8200, null, 2),
  item(15, "Agua Cristal 600 ml", "Bebidas", 2000, null, 28),
];

const HOURLY_PROFILE: readonly { gross: number; count: number }[] = [
  { gross: 0, count: 0 },
  { gross: 0, count: 0 },
  { gross: 0, count: 0 },
  { gross: 0, count: 0 },
  { gross: 0, count: 0 },
  { gross: 0, count: 0 },
  { gross: 85_000, count: 3 },
  { gross: 145_000, count: 5 },
  { gross: 260_000, count: 9 },
  { gross: 310_000, count: 11 },
  { gross: 420_000, count: 14 },
  { gross: 495_000, count: 17 },
  { gross: 615_000, count: 21 },
  { gross: 540_000, count: 18 },
  { gross: 640_000, count: 22 },
  { gross: 460_000, count: 15 },
  { gross: 390_000, count: 13 },
  { gross: 445_000, count: 15 },
  { gross: 530_000, count: 18 },
  { gross: 410_000, count: 14 },
  { gross: 275_000, count: 9 },
  { gross: 130_000, count: 4 },
  { gross: 50_000, count: 2 },
  { gross: 0, count: 0 },
];

const moneyStr = (n: number) => (Math.round(n * 100) / 100).toFixed(2);

function todayIso(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function parseIsoDay(iso: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso.trim());
  if (!m) return null;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return Number.isNaN(d.getTime()) ? null : d;
}

function formatUtcDay(d: Date): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function addUtcDays(d: Date, delta: number): Date {
  const copy = new Date(d.getTime());
  copy.setUTCDate(copy.getUTCDate() + delta);
  return copy;
}

export function mockGetDashboard(query?: DashboardQuery): DashboardResponse {
  const fallbackToday = parseIsoDay(todayIso())!;
  const parsedTo = query?.to ? parseIsoDay(query.to) : null;
  const parsedFrom = query?.from ? parseIsoDay(query.from) : null;
  let toDate = parsedTo ?? fallbackToday;
  let fromDate = parsedFrom ?? toDate;
  if (fromDate.getTime() > toDate.getTime()) {
    fromDate = toDate;
    toDate = parsedFrom ?? toDate;
  }

  const spanDays = Math.max(1, Math.round((toDate.getTime() - fromDate.getTime()) / 86_400_000) + 1);
  const prevToDate = addUtcDays(fromDate, -1);
  const prevFromDate = addUtcDays(prevToDate, -(spanDays - 1));
  const branchFactor = query?.branchId ? 1 : 1.35;

  const by_day = Array.from({ length: spanDays }, (_, idx) => {
    const cur = addUtcDays(fromDate, idx);
    const dow = cur.getUTCDay(); // 0=Dom..6=Sab
    const dayWeight = (dow === 5 || dow === 6 ? 1.18 : dow === 0 ? 0.88 : 1.0) * branchFactor;
    const baseDayGross = HOURLY_PROFILE.reduce((sum, h) => sum + h.gross, 0);
    const baseDayCount = HOURLY_PROFILE.reduce((sum, h) => sum + h.count, 0);
    return {
      date: formatUtcDay(cur),
      gross_sale: moneyStr(Math.round(baseDayGross * dayWeight)),
      sales_count: Math.max(1, Math.round(baseDayCount * dayWeight)),
    };
  });

  const grossTotal = by_day.reduce((s, d) => s + Number(d.gross_sale), 0);
  const salesTotal = by_day.reduce((s, d) => s + d.sales_count, 0);
  const baseGrossDay = HOURLY_PROFILE.reduce((s, h) => s + h.gross, 0);
  const periodFactor = baseGrossDay > 0 ? grossTotal / baseGrossDay : 1;

  const by_hour = HOURLY_PROFILE.map((h, hour) => ({
    hour,
    gross_sale: moneyStr(Math.round(h.gross * periodFactor)),
    sales_count: Math.round(h.count * periodFactor),
  }));

  const refundsTotal = Math.round(grossTotal * 0.024);
  const netTotal = grossTotal - refundsTotal;
  const tipsTotal = Math.round(grossTotal * 0.018);
  const lineTotalSum = Math.round(grossTotal / 1.12);
  const marginPct = 32.4;
  const grossMargin = Math.round(lineTotalSum * (marginPct / 100));

  const kpis: DashboardKpis = {
    gross_sale: moneyStr(grossTotal),
    net_sale: moneyStr(netTotal),
    sales_count: salesTotal,
    avg_ticket: moneyStr(salesTotal > 0 ? grossTotal / salesTotal : 0),
    gross_margin: moneyStr(grossMargin),
    margin_pct: marginPct.toFixed(2),
    refunds: moneyStr(refundsTotal),
    tips: moneyStr(tipsTotal),
    cost_missing_lines: 0,
  };

  const prevGross = Math.round(grossTotal * 0.89);
  const prevSalesCount = Math.max(1, Math.round(salesTotal * 0.91));
  const prevRefunds = Math.round(prevGross * 0.019);
  const prevNet = prevGross - prevRefunds;
  const prevTips = Math.round(prevGross * 0.015);
  const prevLineSum = Math.round(prevGross / 1.12);
  const prevMarginPct = 34.1;
  const prevMargin = Math.round(prevLineSum * (prevMarginPct / 100));

  const kpis_previous: DashboardKpis = {
    gross_sale: moneyStr(prevGross),
    net_sale: moneyStr(prevNet),
    sales_count: prevSalesCount,
    avg_ticket: moneyStr(prevSalesCount > 0 ? prevGross / prevSalesCount : 0),
    gross_margin: moneyStr(prevMargin),
    margin_pct: prevMarginPct.toFixed(2),
    refunds: moneyStr(prevRefunds),
    tips: moneyStr(prevTips),
    cost_missing_lines: 0,
  };

  const topBaseUnits = [142, 84, 65, 52, 48, 36, 44, 38, 29, 41];
  const top_products = [0, 4, 6, 11, 1, 2, 13, 3, 5, 14]
    .map((catalogIdx, rank) => {
      const cat = mockCatalog[catalogIdx];
      const qty = Math.max(1, Math.round(topBaseUnits[rank] * Math.max(1, spanDays * 0.7) * branchFactor));
      const lineTotal = qty * Number(cat.price ?? 0);
      return {
        product_id: cat.product_id,
        name: cat.name,
        qty: qty.toFixed(3),
        line_total: moneyStr(lineTotal),
      };
    })
    .sort((a, b) => Number(b.line_total) - Number(a.line_total));

  const totalPayments = grossTotal + tipsTotal;
  const cashAmt = Math.round(totalPayments * 0.44);
  const cardAmt = Math.round(totalPayments * 0.27);
  const qrAmt = Math.round(totalPayments * 0.17);
  const creditAmt = Math.max(0, totalPayments - cashAmt - cardAmt - qrAmt);

  const payment_methods = [
    { method: "EFECTIVO", amount: moneyStr(cashAmt), count: Math.max(1, Math.round(salesTotal * 0.48)) },
    { method: "DATAFONO", amount: moneyStr(cardAmt), count: Math.max(1, Math.round(salesTotal * 0.26)) },
    { method: "QR", amount: moneyStr(qrAmt), count: Math.max(1, Math.round(salesTotal * 0.19)) },
    { method: "CREDITO", amount: moneyStr(creditAmt), count: Math.max(1, Math.round(salesTotal * 0.09)) },
  ];

  const low_stock = [
    {
      product_id: mockCatalog[7].product_id,
      name: mockCatalog[7].name,
      location_name: "Principal",
      on_hand: "3.000",
      min_qty: "12.000",
    },
    {
      product_id: mockCatalog[13].product_id,
      name: mockCatalog[13].name,
      location_name: "Principal",
      on_hand: "2.000",
      min_qty: "8.000",
    },
    {
      product_id: mockCatalog[2].product_id,
      name: mockCatalog[2].name,
      location_name: "Principal",
      on_hand: "6.000",
      min_qty: "8.000",
    },
    {
      product_id: mockCatalog[10].product_id,
      name: mockCatalog[10].name,
      location_name: "Principal",
      on_hand: "4.000",
      min_qty: "6.000",
    },
  ];

  return {
    period: { from: formatUtcDay(fromDate), to: formatUtcDay(toDate) },
    previous: { from: formatUtcDay(prevFromDate), to: formatUtcDay(prevToDate) },
    kpis,
    kpis_previous,
    by_hour,
    by_day,
    top_products,
    payment_methods,
    low_stock,
    overdue_receivables: {
      count: query?.branchId ? 1 : 2,
      amount: query?.branchId ? "150000.00" : "245000.00",
    },
  };
}

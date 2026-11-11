// Datos mock de reportes S7 / F7.3 para una licorería colombiana en COP.

import type {
  InventoryRotationResponse,
  InventoryRotationRow,
  InventoryValuationResponse,
  InventoryValuationRow,
  PurchasesBySupplierResponse,
  PurchasesBySupplierRow,
  ReportCsvSlug,
  ReportQuery,
  SalesByCashierResponse,
  SalesByCashierRow,
  SalesByCategoryResponse,
  SalesByCategoryRow,
  SalesByDayResponse,
  SalesByDayRow,
  SalesByMethodResponse,
  SalesByMethodRow,
  SalesHeatmapResponse,
  SalesHeatmapRow,
  SalesByProductResponse,
  SalesByProductRow,
} from "./types";

const money = (n: number) => (Math.round(n * 100) / 100).toFixed(2);
const qtyStr = (n: number) => (Math.round(n * 10000) / 10000).toFixed(4);

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

function resolveRange(query?: ReportQuery): {
  from: string;
  to: string;
  fromDate: Date;
  toDate: Date;
  spanDays: number;
  branchFactor: number;
} {
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
  const branchFactor = query?.branchId ? 1 : 1.45;
  return {
    from: formatUtcDay(fromDate),
    to: formatUtcDay(toDate),
    fromDate,
    toDate,
    spanDays,
    branchFactor,
  };
}

// ------------------------------------------------------------------ CSV helper (igual al backend)

const CSV_BOM = "\u{FEFF}";

function escapeCsvCell(value: string): string {
  if (value.includes(";") || value.includes('"') || value.includes("\n") || value.includes("\r")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function formatCsvDecimal(val: string): string {
  const n = Number(val);
  if (!Number.isFinite(n)) return "0,00";
  return n.toFixed(2).replace(".", ",");
}

function formatCsvQty(val: string): string {
  const n = Number(val);
  if (!Number.isFinite(n)) return "0";
  const rounded = Number(n.toFixed(4));
  const raw = Number.isInteger(rounded) ? String(rounded) : rounded.toString();
  return raw.replace(".", ",");
}

function buildCsv(headers: string[], rows: string[][]): string {
  const lines = [headers, ...rows].map((row) => row.map(escapeCsvCell).join(";"));
  return `${CSV_BOM}${lines.join("\r\n")}\r\n`;
}

// ------------------------------------------------------------------ catálogo base licorería

type LiquorBaseItem = {
  id: string;
  sku: string;
  name: string;
  category: string;
  unitNetPrice: number;
  unitCost: number;
  dailyUnits: number;
  floorStock: number;
  warehouseStock: number;
};

const LIQUOR_PRODUCTS: LiquorBaseItem[] = [
  {
    id: "lic-00000000-0000-4000-8000-000000000001",
    sku: "LIC-AGU-750",
    name: "Aguardiente Antioqueño Azul 750 ml",
    category: "Aguardiente",
    unitNetPrice: 39_500,
    unitCost: 26_200,
    dailyUnits: 14,
    floorStock: 28,
    warehouseStock: 72,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000002",
    sku: "LIC-RON-750",
    name: "Ron Viejo de Caldas 3 Años 750 ml",
    category: "Ron",
    unitNetPrice: 44_000,
    unitCost: 29_800,
    dailyUnits: 11,
    floorStock: 24,
    warehouseStock: 60,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000003",
    sku: "LIC-WHI-OP12",
    name: "Whisky Old Parr 12 Años 750 ml",
    category: "Whisky",
    unitNetPrice: 138_000,
    unitCost: 98_500,
    dailyUnits: 5,
    floorStock: 10,
    warehouseStock: 26,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000004",
    sku: "LIC-CER-CLUB6",
    name: "Cerveza Club Colombia Dorada x6",
    category: "Cervezas",
    unitNetPrice: 19_500,
    unitCost: 14_200,
    dailyUnits: 22,
    floorStock: 42,
    warehouseStock: 110,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000005",
    sku: "LIC-CER-POKER6",
    name: "Cerveza Poker Lata x6",
    category: "Cervezas",
    unitNetPrice: 15_200,
    unitCost: 13_450, // margen ~11.5% (<15% crítico)
    dailyUnits: 26,
    floorStock: 36,
    warehouseStock: 96,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000006",
    sku: "LIC-WHI-BUCH",
    name: "Whisky Buchanan's Deluxe 750 ml",
    category: "Whisky",
    unitNetPrice: 156_000,
    unitCost: 114_000,
    dailyUnits: 4,
    floorStock: 8,
    warehouseStock: 18,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000007",
    sku: "LIC-VIN-CAS",
    name: "Vino Casillero del Diablo Cabernet 750 ml",
    category: "Vinos",
    unitNetPrice: 52_000,
    unitCost: 34_500,
    dailyUnits: 6,
    floorStock: 12,
    warehouseStock: 24,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000008",
    sku: "LIC-AGU-NEC",
    name: "Aguardiente Néctar Rojo 1000 ml",
    category: "Aguardiente",
    unitNetPrice: 48_000,
    unitCost: 32_800,
    dailyUnits: 8,
    floorStock: 16,
    warehouseStock: 40,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000009",
    sku: "LIC-TEQ-JC",
    name: "Tequila José Cuervo Reposado 750 ml",
    category: "Destilados",
    unitNetPrice: 94_000,
    unitCost: 66_000,
    dailyUnits: 3,
    floorStock: 7,
    warehouseStock: 14,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000010",
    sku: "LIC-RON-MED",
    name: "Ron Medellín Añejo 750 ml",
    category: "Ron",
    unitNetPrice: 43_000,
    unitCost: 29_200,
    dailyUnits: 7,
    floorStock: 18,
    warehouseStock: 35,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000011",
    sku: "LIC-MEZ-TON4",
    name: "Agua Tónica Botánica x4",
    category: "Mezcladores",
    unitNetPrice: 17_500,
    unitCost: 10_600,
    dailyUnits: 9,
    floorStock: 20,
    warehouseStock: 48,
  },
  {
    id: "lic-00000000-0000-4000-8000-000000000012",
    sku: "LIC-HIE-3KG",
    name: "Bolsa de Hielo Cubos 3 kg",
    category: "Mezcladores",
    unitNetPrice: 4_500,
    unitCost: 1_900,
    dailyUnits: 32,
    floorStock: 45,
    warehouseStock: 0,
  },
];

// ------------------------------------------------------------------ 1. sales-by-day

export function mockSalesByDay(query?: ReportQuery): SalesByDayResponse {
  const { from, to, fromDate, spanDays, branchFactor } = resolveRange(query);

  const rows: SalesByDayRow[] = Array.from({ length: spanDays }, (_, idx) => {
    const cur = addUtcDays(fromDate, idx);
    const dow = cur.getUTCDay(); // 0=Dom..6=Sab
    const dayWeight = (dow === 5 || dow === 6 ? 1.38 : dow === 4 ? 1.15 : dow === 0 ? 0.85 : 0.92) * branchFactor;
    const ventas = Math.max(8, Math.round(42 * dayWeight));
    const ventaBruta = Math.round(3_480_000 * dayWeight);
    const devoluciones = idx % 4 === 1 ? Math.round(ventaBruta * 0.025) : 0;
    const ventaNeta = ventaBruta - devoluciones;
    const ticketPromedio = ventas > 0 ? ventaBruta / ventas : 0;
    const margen = Math.round((ventaBruta / 1.19) * 0.285);
    return {
      fecha: formatUtcDay(cur),
      ventas,
      venta_bruta: money(ventaBruta),
      devoluciones: money(devoluciones),
      venta_neta: money(ventaNeta),
      ticket_promedio: money(ticketPromedio),
      margen: money(margen),
      cost_missing_lines: 0,
    };
  });

  return {
    period: { from, to },
    cost_missing_lines: 0,
    rows,
  };
}

// ------------------------------------------------------------------ 2. sales-by-product

export function mockSalesByProduct(query?: ReportQuery): SalesByProductResponse {
  const { from, to, spanDays, branchFactor } = resolveRange(query);
  const scale = Math.max(1, spanDays * 0.85) * branchFactor;

  const rows: SalesByProductRow[] = LIQUOR_PRODUCTS.map((p) => {
    const cantidad = Math.max(1, Math.round(p.dailyUnits * scale));
    const ventaSinImpuesto = cantidad * p.unitNetPrice;
    const costo = cantidad * p.unitCost;
    const margen = ventaSinImpuesto - costo;
    const pctMargen = ventaSinImpuesto > 0 ? (margen / ventaSinImpuesto) * 100 : 0;
    return {
      product_id: p.id,
      sku: p.sku,
      producto: p.name,
      categoria: p.category,
      cantidad: qtyStr(cantidad),
      venta_sin_impuesto: money(ventaSinImpuesto),
      costo: money(costo),
      margen: money(margen),
      pct_margen: money(pctMargen),
      cost_missing_lines: 0,
    };
  }).sort((a, b) => Number(b.venta_sin_impuesto) - Number(a.venta_sin_impuesto));

  return {
    period: { from, to },
    cost_missing_lines: 0,
    rows,
  };
}

// ------------------------------------------------------------------ 3. sales-by-category

export function mockSalesByCategory(query?: ReportQuery): SalesByCategoryResponse {
  const prod = mockSalesByProduct(query);
  const byCat = new Map<string, { cantidad: number; venta: number; costo: number; margen: number }>();

  for (const r of prod.rows) {
    const prev = byCat.get(r.categoria) ?? { cantidad: 0, venta: 0, costo: 0, margen: 0 };
    prev.cantidad += Number(r.cantidad);
    prev.venta += Number(r.venta_sin_impuesto);
    prev.costo += Number(r.costo);
    prev.margen += Number(r.margen);
    byCat.set(r.categoria, prev);
  }

  const rows: SalesByCategoryRow[] = Array.from(byCat.entries())
    .map(([categoria, agg]) => ({
      categoria,
      cantidad: qtyStr(agg.cantidad),
      venta_sin_impuesto: money(agg.venta),
      costo: money(agg.costo),
      margen: money(agg.margen),
      pct_margen: money(agg.venta > 0 ? (agg.margen / agg.venta) * 100 : 0),
      cost_missing_lines: 0,
    }))
    .sort((a, b) => Number(b.venta_sin_impuesto) - Number(a.venta_sin_impuesto));

  return {
    period: prod.period,
    cost_missing_lines: 0,
    rows,
  };
}

// ------------------------------------------------------------------ 4. sales-by-cashier

export function mockSalesByCashier(query?: ReportQuery): SalesByCashierResponse {
  const daily = mockSalesByDay(query);
  const totalGross = daily.rows.reduce((s, r) => s + Number(r.venta_bruta), 0);
  const totalSales = daily.rows.reduce((s, r) => s + r.ventas, 0);

  const weights = [
    { id: "ca100000-0000-4000-8000-000000000001", name: "Mateo Restrepo", share: 0.44, salesShare: 0.42 },
    { id: "ca100000-0000-4000-8000-000000000002", name: "Valentina Gómez", share: 0.36, salesShare: 0.38 },
    { id: "ca100000-0000-4000-8000-000000000003", name: "Sebastián Rojas", share: 0.2, salesShare: 0.2 },
  ];

  let remainingGross = totalGross;
  let remainingSales = totalSales;

  const rows: SalesByCashierRow[] = weights.map((w, idx) => {
    const isLast = idx === weights.length - 1;
    const ventas = isLast ? Math.max(1, remainingSales) : Math.max(1, Math.round(totalSales * w.salesShare));
    const gross = isLast ? Math.max(0, remainingGross) : Math.round(totalGross * w.share);
    remainingSales -= ventas;
    remainingGross -= gross;
    return {
      cashier_id: w.id,
      cajero: w.name,
      ventas,
      venta_bruta: money(gross),
      ticket_promedio: money(ventas > 0 ? gross / ventas : 0),
    };
  });

  return {
    period: daily.period,
    rows,
  };
}

// ------------------------------------------------------------------ 5. sales-by-method (incluye CREDITO -> "Crédito")

export function mockSalesByMethod(query?: ReportQuery): SalesByMethodResponse {
  const daily = mockSalesByDay(query);
  const totalGross = daily.rows.reduce((s, r) => s + Number(r.venta_bruta), 0);
  const totalSales = daily.rows.reduce((s, r) => s + r.ventas, 0);

  const efectivo = Math.round(totalGross * 0.41);
  const qr = Math.round(totalGross * 0.27);
  const datafono = Math.round(totalGross * 0.21);
  const credito = Math.max(0, totalGross - efectivo - qr - datafono);

  const rows: SalesByMethodRow[] = [
    {
      metodo: "EFECTIVO",
      etiqueta: "Efectivo",
      pagos: Math.max(1, Math.round(totalSales * 0.44)),
      monto: money(efectivo),
    },
    {
      metodo: "QR",
      etiqueta: "QR",
      pagos: Math.max(1, Math.round(totalSales * 0.27)),
      monto: money(qr),
    },
    {
      metodo: "DATAFONO",
      etiqueta: "Datáfono",
      pagos: Math.max(1, Math.round(totalSales * 0.2)),
      monto: money(datafono),
    },
    {
      metodo: "CREDITO",
      etiqueta: "Crédito",
      pagos: Math.max(1, Math.round(totalSales * 0.11)),
      monto: money(credito),
    },
  ];

  return {
    period: daily.period,
    rows,
  };
}

// ------------------------------------------------------------------ 6. sales-heatmap (7x24 sin CSV)

const HOUR_CURVE = [
  0, 0, 0, 0, 0, 0, 0, 0, 0.15, 0.22, 0.35, 0.48, 0.62, 0.55, 0.6, 0.72, 0.85, 1.05, 1.35, 1.55, 1.48, 1.25, 0.82, 0.38,
];

const DAY_WEIGHT = [0.78, 0.82, 0.9, 1.15, 1.52, 1.68, 0.94]; // Lun..Dom

export function mockSalesHeatmap(query?: ReportQuery): SalesHeatmapResponse {
  const { from, to, spanDays, branchFactor } = resolveRange(query);
  const scale = Math.max(1, spanDays / 7) * branchFactor;
  const rows: SalesHeatmapRow[] = [];

  for (let dia = 1; dia <= 7; dia += 1) {
    const dw = DAY_WEIGHT[dia - 1];
    for (let hora = 0; hora < 24; hora += 1) {
      const hw = HOUR_CURVE[hora];
      const intensity = hw * dw * scale;
      const ventas = hw === 0 ? 0 : Math.max(1, Math.round(intensity * 3));
      const ventaBruta = hw === 0 ? 0 : Math.round(intensity * 245_000);
      rows.push({
        dia,
        hora,
        ventas,
        venta_bruta: money(ventaBruta),
      });
    }
  }

  return {
    period: { from, to },
    rows,
  };
}

// ------------------------------------------------------------------ 7. inventory-valuation

export function mockInventoryValuation(query?: ReportQuery): InventoryValuationResponse {
  const { from, to, branchFactor } = resolveRange(query);
  const mult = branchFactor > 1 ? 1.5 : 1;
  const branchLabel = query?.branchId ? "Sucursal Principal" : "Todas las sucursales";

  const rows: InventoryValuationRow[] = [];
  let totalValor = 0;

  for (const p of LIQUOR_PRODUCTS) {
    const floorUnits = Math.round(p.floorStock * mult);
    if (floorUnits !== 0) {
      const val = floorUnits * p.unitCost;
      totalValor += val;
      rows.push({
        product_id: p.id,
        location_id: "loc-piso-venta",
        sucursal: branchLabel,
        producto: p.name,
        ubicacion: "Piso de venta",
        existencias: qtyStr(floorUnits),
        costo_promedio: money(p.unitCost),
        valor: money(val),
      });
    }
    const bodegaUnits = Math.round(p.warehouseStock * mult);
    if (bodegaUnits !== 0) {
      const val = bodegaUnits * p.unitCost;
      totalValor += val;
      rows.push({
        product_id: p.id,
        location_id: "loc-bodega-ppal",
        sucursal: branchLabel,
        producto: p.name,
        ubicacion: "Bodega Principal",
        existencias: qtyStr(bodegaUnits),
        costo_promedio: money(p.unitCost),
        valor: money(val),
      });
    }
  }

  rows.sort((a, b) => Number(b.valor) - Number(a.valor));

  return {
    period: { from, to },
    total_valor: money(totalValor),
    rows,
  };
}

// ------------------------------------------------------------------ 8. inventory-rotation (30 días hasta `to`)

export function mockInventoryRotation(query?: ReportQuery): InventoryRotationResponse {
  const fallbackToday = parseIsoDay(todayIso())!;
  const toDate = (query?.to ? parseIsoDay(query.to) : null) ?? fallbackToday;
  const fromDate = addUtcDays(toDate, -29);
  const branchFactor = query?.branchId ? 1 : 1.45;
  const mult = branchFactor > 1 ? 1.5 : 1;

  const rows: InventoryRotationRow[] = LIQUOR_PRODUCTS.map((p) => {
    const sold30d = Math.round(p.dailyUnits * 30 * branchFactor);
    const existencias = Math.round((p.floorStock + p.warehouseStock) * mult);
    const dias = sold30d > 0 ? money((existencias * 30) / sold30d) : null;
    return {
      product_id: p.id,
      producto: p.name,
      vendidas_30d: qtyStr(sold30d),
      existencias: qtyStr(existencias),
      dias_inventario: dias,
    };
  });

  // Producto sin movimiento en los últimos 30 días para mostrar celda vacía / "—"
  rows.push({
    product_id: "lic-00000000-0000-4000-8000-000000000099",
    producto: "Ginebra Tanqueray London Dry 750 ml",
    vendidas_30d: qtyStr(0),
    existencias: qtyStr(6),
    dias_inventario: null,
  });

  rows.sort((a, b) => Number(b.vendidas_30d) - Number(a.vendidas_30d));

  return {
    period: { from: formatUtcDay(fromDate), to: formatUtcDay(toDate) },
    rows,
  };
}

// ------------------------------------------------------------------ 9. purchases-by-supplier

export function mockPurchasesBySupplier(query?: ReportQuery): PurchasesBySupplierResponse {
  const { from, to, spanDays, branchFactor } = resolveRange(query);
  const scale = Math.max(1, spanDays / 7) * branchFactor;

  const baseSuppliers = [
    {
      id: "sup-lic-0001",
      name: "Fábrica de Licores de Antioquia (FLA)",
      orders: 4,
      received: 6_840_000,
      paidRatio: 0.72,
    },
    {
      id: "sup-lic-0002",
      name: "Bavaria & Cía S.C.A.",
      orders: 6,
      received: 5_420_000,
      paidRatio: 1.0,
    },
    {
      id: "sup-lic-0003",
      name: "Diageo Colombia S.A.",
      orders: 2,
      received: 4_950_000,
      paidRatio: 0.5,
    },
    {
      id: "sup-lic-0004",
      name: "Industria Licorera de Caldas",
      orders: 3,
      received: 3_280_000,
      paidRatio: 0.85,
    },
    {
      id: "sup-lic-0005",
      name: "Dislicores S.A.S.",
      orders: 2,
      received: 1_960_000,
      paidRatio: 0.4,
    },
  ];

  const rows: PurchasesBySupplierRow[] = baseSuppliers.map((s) => {
    const ordenes = Math.max(1, Math.round(s.orders * Math.max(1, scale * 0.6)));
    const recibido = Math.round(s.received * scale);
    const pagado = Math.round(recibido * s.paidRatio);
    const saldo = Math.max(0, recibido - pagado);
    return {
      supplier_id: s.id,
      proveedor: s.name,
      ordenes,
      recibido: money(recibido),
      pagado: money(pagado),
      saldo: money(saldo),
    };
  });

  return {
    period: { from, to },
    rows,
  };
}

// ------------------------------------------------------------------ exportador CSV unificado mock

export function mockExportReportCsv(
  slug: ReportCsvSlug,
  query?: ReportQuery,
): { filename: string; body: string } {
  switch (slug) {
    case "sales-by-day": {
      const data = mockSalesByDay(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["fecha", "ventas", "venta_bruta", "devoluciones", "venta_neta", "ticket_promedio", "margen"],
          data.rows.map((r) => [
            r.fecha,
            String(r.ventas),
            formatCsvDecimal(r.venta_bruta),
            formatCsvDecimal(r.devoluciones),
            formatCsvDecimal(r.venta_neta),
            formatCsvDecimal(r.ticket_promedio),
            formatCsvDecimal(r.margen),
          ]),
        ),
      };
    }
    case "sales-by-product": {
      const data = mockSalesByProduct(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["sku", "producto", "categoría", "cantidad", "venta_sin_impuesto", "costo", "margen", "pct_margen"],
          data.rows.map((r) => [
            r.sku ?? "",
            r.producto,
            r.categoria,
            formatCsvQty(r.cantidad),
            formatCsvDecimal(r.venta_sin_impuesto),
            formatCsvDecimal(r.costo),
            formatCsvDecimal(r.margen),
            formatCsvDecimal(r.pct_margen),
          ]),
        ),
      };
    }
    case "sales-by-category": {
      const data = mockSalesByCategory(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["categoría", "cantidad", "venta_sin_impuesto", "margen"],
          data.rows.map((r) => [
            r.categoria,
            formatCsvQty(r.cantidad),
            formatCsvDecimal(r.venta_sin_impuesto),
            formatCsvDecimal(r.margen),
          ]),
        ),
      };
    }
    case "sales-by-cashier": {
      const data = mockSalesByCashier(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["cajero", "ventas", "venta_bruta", "ticket_promedio"],
          data.rows.map((r) => [
            r.cajero,
            String(r.ventas),
            formatCsvDecimal(r.venta_bruta),
            formatCsvDecimal(r.ticket_promedio),
          ]),
        ),
      };
    }
    case "sales-by-method": {
      const data = mockSalesByMethod(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["método", "pagos", "monto"],
          data.rows.map((r) => [r.etiqueta, String(r.pagos), formatCsvDecimal(r.monto)]),
        ),
      };
    }
    case "inventory-valuation": {
      const data = mockInventoryValuation(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["producto", "ubicación", "existencias", "costo_promedio", "valor"],
          data.rows.map((r) => [
            r.producto,
            r.ubicacion,
            formatCsvQty(r.existencias),
            formatCsvDecimal(r.costo_promedio),
            formatCsvDecimal(r.valor),
          ]),
        ),
      };
    }
    case "inventory-rotation": {
      const data = mockInventoryRotation(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["producto", "vendidas_30d", "existencias", "dias_inventario"],
          data.rows.map((r) => [
            r.producto,
            formatCsvQty(r.vendidas_30d),
            formatCsvQty(r.existencias),
            r.dias_inventario != null ? formatCsvDecimal(r.dias_inventario) : "",
          ]),
        ),
      };
    }
    case "purchases-by-supplier": {
      const data = mockPurchasesBySupplier(query);
      return {
        filename: `${slug}_${data.period.from}_${data.period.to}.csv`,
        body: buildCsv(
          ["proveedor", "órdenes", "recibido", "pagado", "saldo"],
          data.rows.map((r) => [
            r.proveedor,
            String(r.ordenes),
            formatCsvDecimal(r.recibido),
            formatCsvDecimal(r.pagado),
            formatCsvDecimal(r.saldo),
          ]),
        ),
      };
    }
  }
}


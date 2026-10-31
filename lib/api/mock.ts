// Datos mock con la misma forma que el JSON del backend (ver ./types.ts).

import type { CatalogItem, DashboardSummary, Uuid } from "./types";

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
];

const hourly = [
  [0, 120_000, 3], [4, 380_000, 9], [8, 2_150_000, 41], [10, 3_900_000, 66],
  [12, 5_200_000, 88], [14, 6_420_000, 102], [16, 4_100_000, 71],
  [18, 3_300_000, 58], [20, 1_900_000, 34], [23, 450_000, 8],
] as const;

export const mockDashboard: DashboardSummary = {
  revenue: "44820000",
  revenue_ex_tax: "37663865",
  revenue_change_pct: "12.4",
  profit: "14340000",
  profit_margin_pct: "32",
  profit_change_pct: "8.1",
  balance: "8950000",
  expenses: "92000",
  hourly: hourly.map(([hour, revenue, orders]) => ({ hour, revenue: String(revenue), orders })),
  channels: [
    { channel: "WHATSAPP", amount: "20169000" },
    { channel: "POS", amount: "15687000" },
    { channel: "TIENDA_ONLINE", amount: "8964000" },
  ],
  alerts: { low_stock: 5, out_of_stock: 2, pending_dispatch: 3 },
};

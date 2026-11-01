// Inventario simulado para el modo demo. Usa el mismo `available` que el
// catalogo mock, asi una venta en el POS se ve reflejada en /inventario y en
// el kardex. Imita las reglas de src/inventory.rs.

import { ApiError } from "./client";
import { MOCK_BRANCH_ID, mockCatalog } from "./mock";
import type { Movement, MovementType, StockLevel, StockStatus, Uuid } from "./types";

type Meta = { sku: string; barcode: string; cost: number; min: number };

/** Costo de compra y punto de reorden de cada producto del catalogo mock. */
const META: Meta[] = [
  { sku: "BEB-001", barcode: "7702004003508", cost: 2100, min: 12 },
  { sku: "ABA-002", barcode: "7702511000015", cost: 1950, min: 10 },
  { sku: "LAC-003", barcode: "7702001020119", cost: 2900, min: 8 },
  { sku: "PAN-004", barcode: "7705326010012", cost: 4800, min: 5 },
  { sku: "ABA-005", barcode: "7709990000052", cost: 11800, min: 4 },
  { sku: "ASE-006", barcode: "7702310020016", cost: 1400, min: 10 },
  { sku: "ABA-007", barcode: "7702032112013", cost: 6200, min: 6 },
  { sku: "BEB-008", barcode: "7702004110305", cost: 1850, min: 12 },
  { sku: "LAC-009", barcode: "7702001045617", cost: 1700, min: 8 },
  { sku: "SNA-010", barcode: "7702189045201", cost: 1300, min: 10 },
  { sku: "SNA-011", barcode: "7702007030129", cost: 900, min: 6 },
  { sku: "ASE-012", barcode: "7702191660317", cost: 9100, min: 5 },
];

const meta = new Map<Uuid, Meta>(mockCatalog.map((p, i) => [p.product_id, META[i]]));
const DIRECTION: Record<MovementType, 1 | -1 | 0> = {
  REPOSICION: 1,
  AJUSTE_ENTRADA: 1,
  TRASLADO_ENTRADA: 1,
  DESCUENTO: -1,
  AJUSTE_SALIDA: -1,
  TRASLADO_SALIDA: -1,
  DEVOLUCION: 1,
  AJUSTE: 0,
};

const movements: Movement[] = seedMovements();

function product(productId: Uuid) {
  const p = mockCatalog.find((item) => item.product_id === productId);
  if (!p) throw new ApiError(404, "not found");
  return p;
}

function onHand(productId: Uuid): number {
  return Number(product(productId).available ?? 0);
}

function statusOf(available: number, min: number): StockStatus {
  if (available <= 0) return "AGOTADO";
  if (available <= min) return "BAJO";
  return "OK";
}

/** Reposicion inicial de ayer para que el kardex no arranque vacio. */
function seedMovements(): Movement[] {
  const yesterday = new Date(Date.now() - 20 * 3600_000);
  return mockCatalog.map((p, i): Movement => {
    const qty = Number(p.available ?? 0);
    return {
      id: crypto.randomUUID(),
      product_id: p.product_id,
      product_name: p.name,
      sku: META[i].sku,
      movement_type: "REPOSICION",
      qty: qty.toFixed(4),
      delta: qty.toFixed(4),
      balance_after: qty.toFixed(4),
      cause: "Inventario inicial",
      ref_type: null,
      ref_id: null,
      user_name: "Usuario demo",
      branch_id: MOCK_BRANCH_ID,
      created_at: new Date(yesterday.getTime() + i * 60_000).toISOString(),
    };
  }).filter((m) => Number(m.qty) > 0).reverse();
}

/** Aplica un movimiento: cambia la existencia y lo anota en el kardex. */
export function mockRecordMovement(
  productId: Uuid,
  type: MovementType,
  qty: number,
  cause: string,
  ref?: { type: string; id: Uuid },
) {
  const p = product(productId);
  const delta = DIRECTION[type] * qty;
  const balance = onHand(productId) + delta;
  if (balance < 0) throw new ApiError(422, "validation: adjustment violates stock");
  p.available = balance.toFixed(3);
  movements.unshift({
    id: crypto.randomUUID(),
    product_id: productId,
    product_name: p.name,
    sku: meta.get(productId)?.sku ?? null,
    movement_type: type,
    qty: qty.toFixed(4),
    delta: delta.toFixed(4),
    balance_after: balance.toFixed(4),
    cause,
    ref_type: ref?.type ?? null,
    ref_id: ref?.id ?? null,
    user_name: "Usuario demo",
    branch_id: MOCK_BRANCH_ID,
    created_at: new Date().toISOString(),
  });
}

export function mockStockLevels(): StockLevel[] {
  return mockCatalog
    .map((p) => {
      const m = meta.get(p.product_id)!;
      const available = onHand(p.product_id);
      return {
        product_id: p.product_id,
        name: p.name,
        sku: m.sku,
        barcode: m.barcode,
        category: p.category,
        unit: "Unidad",
        price: p.price,
        cost_price: m.cost.toFixed(2),
        on_hand: available.toFixed(4),
        reserved: "0.0000",
        available: available.toFixed(4),
        min_qty: m.min.toFixed(4),
        status: statusOf(available, m.min),
        updated_at: movements.find((mv) => mv.product_id === p.product_id)?.created_at ?? null,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name, "es"));
}

export function mockKardex(productId?: Uuid | null): Movement[] {
  return movements.filter((m) => !productId || m.product_id === productId);
}

export function mockReplenish(productId: Uuid, qty: number, cause: string) {
  if (qty <= 0) throw new ApiError(422, "validation: qty must be > 0");
  mockRecordMovement(productId, "REPOSICION", qty, cause);
}

export function mockAdjust(productId: Uuid, delta: number, cause: string) {
  if (delta === 0) throw new ApiError(422, "validation: delta must be non-zero");
  mockRecordMovement(productId, delta > 0 ? "AJUSTE_ENTRADA" : "AJUSTE_SALIDA", Math.abs(delta), cause);
}

export function mockCount(productId: Uuid, counted: number, cause: string): number {
  if (counted < 0) throw new ApiError(422, "validation: counted must be >= 0");
  const delta = counted - onHand(productId);
  if (delta !== 0) mockAdjust(productId, delta, cause);
  return delta;
}

export function mockSetMinQty(productId: Uuid, minQty: number) {
  if (minQty < 0) throw new ApiError(422, "validation: min_qty must be >= 0");
  meta.get(productId)!.min = minQty;
}

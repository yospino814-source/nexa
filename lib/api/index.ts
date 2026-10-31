// Punto unico de acceso a datos. Las pantallas importan desde aqui y no saben
// si los datos vienen del backend Rust (via /api) o de los mocks.
//
// Los mocks imitan las reglas del backend (impuesto sumado al precio, turno de
// caja obligatorio, idempotencia por id) para que la UI se comporte igual.

import { ApiError, request, USE_MOCKS } from "./client";
import { MOCK_BRANCH_ID, mockCatalog, mockDashboard } from "./mock";
import {
  mockAdjust,
  mockCount,
  mockKardex,
  mockRecordMovement,
  mockReplenish,
  mockSetMinQty,
  mockStockLevels,
} from "./mock-inventory";
import type {
  BranchAccess,
  CashActivity,
  CashMovementKind,
  CashSession,
  CashSessionRow,
  CashSummary,
  CatalogItem,
  CheckoutRequest,
  CheckoutResponse,
  DashboardSummary,
  Decimal,
  Movement,
  OrderItem,
  Payment,
  SessionContext,
  StockLevel,
  Uuid,
} from "./types";

export * from "./types";
export { ApiError, setAccessToken, setTokenProvider, setUnauthorizedHandler, USE_MOCKS } from "./client";

const delay = (ms = 150) => new Promise((resolve) => setTimeout(resolve, ms));
const cents = (n: number) => Math.round(n * 100) / 100;
const dec = (n: number): Decimal => cents(n).toFixed(2);

// ------------------------------------------------------------ estado mock

let receiptSeq = 0;
const mockSales = new Map<Uuid, { body: string; response: CheckoutResponse; at: string }>();
/** Turno al que pertenece cada venta mock. */
const sessionOf = new Map<Uuid, Uuid>();
/** El demo arranca con la caja abierta para poder vender sin configurar nada. */
let mockCash: CashSession | null = newMockSession(100_000);
/** Ingresos y retiros de efectivo de los turnos mock. */
const mockCashMoves: { sessionId: Uuid; kind: CashMovementKind; amount: number; reason: string; at: string }[] = [];
/** Turnos cerrados (los dos primeros son de dias anteriores, para el historial). */
const mockClosed: CashSummary[] = seedClosedSessions();

function seedClosedSessions(): CashSummary[] {
  const day = 24 * 3600_000;
  return [
    { daysAgo: 1, opening: 100_000, sales: 1_284_500, cash: 642_300, counted: 742_300, count: 38 },
    { daysAgo: 2, opening: 100_000, sales: 956_800, cash: 410_900, counted: 505_900, count: 29 },
  ].map(({ daysAgo, opening, sales, cash, counted, count }) => {
    const opened = new Date(Date.now() - daysAgo * day);
    opened.setHours(7, 0, 0, 0);
    const expected = opening + cash;
    return {
      session: {
        id: crypto.randomUUID(),
        branch_id: MOCK_BRANCH_ID,
        opened_by: "00000000-0000-4000-8000-0000000000aa",
        opening_amount: dec(opening),
        opened_at: opened.toISOString(),
        closed_by: "00000000-0000-4000-8000-0000000000aa",
        closed_at: new Date(opened.getTime() + 13 * 3600_000).toISOString(),
        expected_cash: dec(expected),
        counted_cash: dec(counted),
        difference: dec(counted - expected),
        notes: counted === expected ? null : "Faltante en cambio menudo",
      },
      sales_count: count,
      sales_total: dec(sales),
      by_method: [{ method: "EFECTIVO", amount: dec(cash), count }],
      cash_in: "0.00",
      cash_out: "0.00",
      expected_cash: dec(expected),
    };
  });
}

function newMockSession(opening: number): CashSession {
  return {
    id: crypto.randomUUID(),
    branch_id: MOCK_BRANCH_ID,
    opened_by: "00000000-0000-4000-8000-0000000000aa",
    opening_amount: dec(opening),
    opened_at: new Date().toISOString(),
    closed_by: null,
    closed_at: null,
    expected_cash: null,
    counted_cash: null,
    difference: null,
    notes: null,
  };
}

function mockSummary(session: CashSession): CashSummary {
  const sales = [...mockSales.values()]
    .map((s) => s.response)
    .filter((r) => r.order.status === "COBRADA" && sessionOf.get(r.order.id) === session.id);
  const byMethod = new Map<string, { amount: number; count: number }>();
  for (const sale of sales) {
    for (const p of sale.payments) {
      const acc = byMethod.get(p.method) ?? { amount: 0, count: 0 };
      acc.amount += Number(p.amount);
      acc.count += 1;
      byMethod.set(p.method, acc);
    }
  }
  const cash = byMethod.get("EFECTIVO")?.amount ?? 0;
  const moves = mockCashMoves.filter((m) => m.sessionId === session.id);
  const sum = (kind: CashMovementKind) => moves.filter((m) => m.kind === kind).reduce((s, m) => s + m.amount, 0);
  const cashIn = sum("INGRESO");
  const cashOut = sum("RETIRO");
  return {
    session,
    sales_count: sales.length,
    sales_total: dec(sales.reduce((sum, s) => sum + Number(s.order.total_amount), 0)),
    by_method: [...byMethod.entries()].map(([method, v]) => ({
      method: method as Payment["method"],
      amount: dec(v.amount),
      count: v.count,
    })),
    cash_in: dec(cashIn),
    cash_out: dec(cashOut),
    expected_cash: dec(Number(session.opening_amount) + cash + cashIn - cashOut),
  };
}

/** Bitacora del turno mock, igual que src/cash.rs::activity. */
function mockActivity(sessionId: Uuid): CashActivity[] {
  const session =
    mockCash?.id === sessionId ? mockCash : mockClosed.find((s) => s.session.id === sessionId)?.session;
  if (!session) throw new ApiError(404, "not found");
  const entries: CashActivity[] = [
    { at: session.opened_at, kind: "APERTURA", detail: "Base inicial", methods: "EFECTIVO", amount: session.opening_amount, user_name: "Usuario demo" },
  ];
  for (const [id, sale] of mockSales) {
    if (sessionOf.get(id) !== sessionId) continue;
    entries.push({
      at: sale.at,
      kind: "VENTA",
      detail: sale.response.receipt_number,
      methods: [...new Set(sale.response.payments.map((p) => p.method))].sort().join("+"),
      amount: sale.response.order.total_amount,
      user_name: "Usuario demo",
    });
  }
  for (const m of mockCashMoves.filter((x) => x.sessionId === sessionId)) {
    entries.push({ at: m.at, kind: m.kind, detail: m.reason, methods: "EFECTIVO", amount: dec(m.amount), user_name: "Usuario demo" });
  }
  if (session.closed_at) {
    entries.push({ at: session.closed_at, kind: "CIERRE", detail: session.notes, methods: "EFECTIVO", amount: session.counted_cash ?? "0", user_name: "Usuario demo" });
  }
  return entries.sort((a, b) => b.at.localeCompare(a.at));
}


function mockCheckout(body: CheckoutRequest): CheckoutResponse {
  const key = JSON.stringify(body);
  const previous = mockSales.get(body.id);
  if (previous) {
    if (previous.body !== key) throw new ApiError(409, "sale id already used with a different request");
    return previous.response;
  }
  if (!mockCash) throw new ApiError(409, "no open cash session for this cashier");

  const products = new Map(mockCatalog.map((p) => [p.product_id, p]));
  const items: OrderItem[] = body.items.map((line) => {
    const product = products.get(line.product_id);
    if (!product?.price) throw new ApiError(422, `product ${line.product_id} is not in the branch assortment`);
    const qty = Number(line.qty);
    if (product.available != null && Number(product.available) < qty) {
      throw new ApiError(409, `insufficient stock for product ${line.product_id}`);
    }
    const base = cents(Number(product.price) * qty);
    const rate = Number(product.tax_rate ?? 0);
    return {
      id: crypto.randomUUID(),
      product_id: line.product_id,
      qty: line.qty,
      unit_price: product.price,
      line_total: dec(base),
      tax_rate: rate.toFixed(4),
      tax_amount: dec(base * rate),
      station_id: null,
      status_id: null,
    };
  });
  const subtotal = items.reduce((s, i) => s + Number(i.line_total), 0);
  const tax = items.reduce((s, i) => s + Number(i.tax_amount), 0);
  const total = cents(subtotal + tax);

  // Mismo reparto que src/sales.rs::settle.
  const nonCash = body.payments.filter((p) => p.method !== "EFECTIVO").reduce((s, p) => s + Number(p.amount), 0);
  const cashLine = body.payments.find((p) => p.method === "EFECTIVO");
  const due = cents(total - nonCash);
  if (nonCash > total) throw new ApiError(422, "validation: non-cash payments exceed the total");
  if (!cashLine && due !== 0) throw new ApiError(422, "validation: payments do not cover the total");
  if (cashLine && Number(cashLine.amount) < due) throw new ApiError(422, "validation: payments do not cover the total");
  const change = cashLine ? cents(Number(cashLine.amount) - due) : 0;

  const orderId = body.id;
  for (const line of body.items) {
    const product = products.get(line.product_id);
    if (product?.available != null) {
      mockRecordMovement(line.product_id, "DESCUENTO", Number(line.qty), "venta", { type: "venta", id: orderId });
    }
  }

  receiptSeq += 1;
  sessionOf.set(orderId, mockCash.id);
  const response: CheckoutResponse = {
    order: {
      id: orderId,
      sale_type: "MOSTRADOR",
      status: "COBRADA",
      table_id: null,
      branch_id: body.branch_id,
      waiter_id: null,
      subtotal: dec(subtotal),
      inc_amount: dec(tax),
      tip_amount: "0",
      total_amount: dec(total),
    },
    items,
    payments: body.payments.map((p) => ({
      id: crypto.randomUUID(),
      method: p.method,
      amount: p.method === "EFECTIVO" ? dec(due) : p.amount,
      tendered: p.method === "EFECTIVO" ? p.amount : null,
      reference: p.reference ?? null,
    })),
    receipt_number: `POS-${String(receiptSeq).padStart(6, "0")}`,
    change_due: dec(change),
    fiscal_status: "PENDIENTE",
  };
  mockSales.set(body.id, { body: key, response, at: new Date().toISOString() });
  return response;
}

// --------------------------------------------------------------------- API

export const api = {
  session: {
    /** GET /v1/context */
    async context(): Promise<SessionContext> {
      if (USE_MOCKS) {
        await delay();
        return { uid: null, tenant: null, branch: MOCK_BRANCH_ID, role: "CAJERO" };
      }
      return request("/v1/context");
    },

    /** GET /v1/branches — sucursales del usuario con su rol en cada una. */
    async branches(): Promise<BranchAccess[]> {
      if (USE_MOCKS) {
        await delay();
        return [{ id: MOCK_BRANCH_ID, name: "Sucursal Principal", code: null, role: "ADMIN_CAJA" }];
      }
      return request("/v1/branches");
    },
  },

  catalog: {
    /** GET /v1/catalog?branch_id= */
    async list(branchId: Uuid): Promise<CatalogItem[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockCatalog);
      }
      return request(`/v1/catalog?branch_id=${branchId}`);
    },
  },

  sales: {
    /**
     * POST /v1/sales. Reusar el mismo `body.id` en los reintentos: el backend
     * devuelve la venta ya registrada en vez de cobrar de nuevo.
     */
    async checkout(body: CheckoutRequest): Promise<CheckoutResponse> {
      if (!USE_MOCKS) {
        return request("/v1/sales", { method: "POST", body: JSON.stringify(body) });
      }
      await delay(400);
      return structuredClone(mockCheckout(body));
    },
  },

  cash: {
    /** GET /v1/cash-sessions/current?branch_id= — `null` si no hay turno abierto. */
    async current(branchId: Uuid): Promise<CashSummary | null> {
      if (USE_MOCKS) {
        await delay();
        return mockCash ? structuredClone(mockSummary(mockCash)) : null;
      }
      return request(`/v1/cash-sessions/current?branch_id=${branchId}`);
    },

    /** POST /v1/cash-sessions */
    async open(branchId: Uuid, openingAmount: Decimal): Promise<CashSession> {
      if (!USE_MOCKS) {
        return request("/v1/cash-sessions", {
          method: "POST",
          body: JSON.stringify({ branch_id: branchId, opening_amount: openingAmount }),
        });
      }
      await delay();
      if (mockCash) throw new ApiError(409, "there is already an open cash session");
      mockCash = newMockSession(Number(openingAmount));
      return structuredClone(mockCash);
    },

    /** POST /v1/cash-sessions/{id}/close */
    async close(sessionId: Uuid, countedCash: Decimal, notes?: string): Promise<CashSummary> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-sessions/${sessionId}/close`, {
          method: "POST",
          body: JSON.stringify({ counted_cash: countedCash, notes: notes ?? null }),
        });
      }
      await delay();
      if (!mockCash || mockCash.id !== sessionId) throw new ApiError(409, "cash session is already closed");
      const expected = Number(mockSummary(mockCash).expected_cash);
      const counted = Number(countedCash);
      const closed: CashSession = {
        ...mockCash,
        closed_at: new Date().toISOString(),
        closed_by: mockCash.opened_by,
        expected_cash: dec(expected),
        counted_cash: dec(counted),
        difference: dec(counted - expected),
        notes: notes ?? null,
      };
      const summary = mockSummary(closed);
      mockCash = null;
      mockClosed.unshift(summary);
      return structuredClone(summary);
    },

    /** POST /v1/cash-sessions/{id}/movements — ingreso o retiro; devuelve el resumen. */
    async addMovement(sessionId: Uuid, kind: CashMovementKind, amount: Decimal, reason: string): Promise<CashSummary> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-sessions/${sessionId}/movements`, {
          method: "POST",
          body: JSON.stringify({ kind, amount, reason }),
        });
      }
      await delay();
      if (!mockCash || mockCash.id !== sessionId) throw new ApiError(409, "cash session is already closed");
      if (Number(amount) <= 0) throw new ApiError(422, "validation: amount must be > 0");
      if (!reason.trim()) throw new ApiError(422, "validation: reason is required");
      if (kind === "RETIRO" && Number(amount) > Number(mockSummary(mockCash).expected_cash)) {
        throw new ApiError(409, "withdrawal exceeds the cash in the drawer");
      }
      mockCashMoves.push({ sessionId, kind, amount: Number(amount), reason: reason.trim(), at: new Date().toISOString() });
      return structuredClone(mockSummary(mockCash));
    },

    /** GET /v1/cash-sessions/{id}/activity — bitacora del turno, mas reciente primero. */
    async activity(sessionId: Uuid): Promise<CashActivity[]> {
      if (USE_MOCKS) {
        await delay();
        return mockActivity(sessionId);
      }
      return request(`/v1/cash-sessions/${sessionId}/activity`);
    },

    /** GET /v1/cash-sessions?branch_id= — turnos de la sucursal (cajero: solo los suyos). */
    async history(branchId: Uuid, limit = 30): Promise<CashSessionRow[]> {
      if (USE_MOCKS) {
        await delay();
        const all = mockCash ? [mockSummary(mockCash), ...mockClosed] : mockClosed;
        return all.slice(0, limit).map(({ session: s, sales_count, sales_total }) => ({
          id: s.id,
          opened_by: s.opened_by,
          opened_by_name: "Usuario demo",
          opening_amount: s.opening_amount,
          opened_at: s.opened_at,
          closed_at: s.closed_at,
          expected_cash: s.expected_cash,
          counted_cash: s.counted_cash,
          difference: s.difference,
          sales_count,
          sales_total,
        }));
      }
      return request(`/v1/cash-sessions?branch_id=${branchId}&limit=${limit}`);
    },
  },

  inventory: {
    /** GET /v1/inventory/stock?branch_id= — existencias con estado frente al punto de reorden. */
    async stock(branchId: Uuid): Promise<StockLevel[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockStockLevels());
      }
      return request(`/v1/inventory/stock?branch_id=${branchId}`);
    },

    /** GET /v1/inventory/movements — kardex, mas reciente primero. */
    async movements(branchId: Uuid, productId?: Uuid | null, limit = 200): Promise<Movement[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockKardex(productId).slice(0, limit));
      }
      const product = productId ? `&product_id=${productId}` : "";
      return request(`/v1/inventory/movements?branch_id=${branchId}${product}&limit=${limit}`);
    },

    /** POST /v1/inventory/replenish — entrada de mercancia. */
    async replenish(branchId: Uuid, productId: Uuid, qty: Decimal, cause: string): Promise<void> {
      if (USE_MOCKS) {
        await delay();
        return mockReplenish(productId, Number(qty), cause);
      }
      await request("/v1/inventory/replenish", {
        method: "POST",
        body: JSON.stringify({ branch_id: branchId, product_id: productId, qty, cause }),
      });
    },

    /** POST /v1/inventory/adjust — suma o resta unidades (delta con signo). */
    async adjust(branchId: Uuid, productId: Uuid, delta: Decimal, cause: string): Promise<void> {
      if (USE_MOCKS) {
        await delay();
        return mockAdjust(productId, Number(delta), cause);
      }
      await request("/v1/inventory/adjust", {
        method: "POST",
        body: JSON.stringify({ branch_id: branchId, product_id: productId, qty: delta, cause }),
      });
    },

    /** POST /v1/inventory/count — conteo fisico; devuelve la diferencia registrada. */
    async count(branchId: Uuid, productId: Uuid, counted: Decimal, cause: string): Promise<{ delta: Decimal }> {
      if (USE_MOCKS) {
        await delay();
        return { delta: String(mockCount(productId, Number(counted), cause)) };
      }
      return request("/v1/inventory/count", {
        method: "POST",
        body: JSON.stringify({ branch_id: branchId, product_id: productId, counted, cause }),
      });
    },

    /** POST /v1/inventory/transfer — traslado entre sucursales del negocio. */
    async transfer(fromBranchId: Uuid, toBranchId: Uuid, productId: Uuid, qty: Decimal, cause: string): Promise<{ transfer_id: Uuid }> {
      if (USE_MOCKS) {
        await delay();
        throw new ApiError(403, "forbidden");
      }
      return request("/v1/inventory/transfer", {
        method: "POST",
        body: JSON.stringify({ from_branch_id: fromBranchId, to_branch_id: toBranchId, product_id: productId, qty, cause }),
      });
    },

    /** PUT /v1/inventory/min-qty — punto de reorden. */
    async setMinQty(branchId: Uuid, productId: Uuid, minQty: Decimal): Promise<void> {
      if (USE_MOCKS) {
        await delay();
        return mockSetMinQty(productId, Number(minQty));
      }
      await request("/v1/inventory/min-qty", {
        method: "PUT",
        body: JSON.stringify({ branch_id: branchId, product_id: productId, min_qty: minQty }),
      });
    },
  },

  reports: {
    /** Backend: GET /v1/reports/dashboard (PROPUESTO, aun sin implementar). */
    async dashboard(branchId: Uuid): Promise<DashboardSummary> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockDashboard);
      }
      return request(`/v1/reports/dashboard?branch_id=${branchId}&period=today`);
    },
  },
};

/** Mensajes del backend traducidos para el cajero. */
export function friendlyError(error: unknown): string {
  if (!(error instanceof ApiError)) return "No se pudo conectar con el servidor. Intentá de nuevo.";
  const m = error.message;
  if (m.includes("no open cash session")) return "No hay un turno de caja abierto. Abrí la caja para vender.";
  if (m.includes("insufficient stock")) return "No hay stock suficiente de uno de los productos.";
  if (m.includes("do not cover the total")) return "El pago no cubre el total.";
  if (m.includes("non-cash payments exceed")) return "Los pagos con tarjeta o QR no pueden superar el total.";
  if (m.includes("already used with a different request")) return "Esta venta ya se registró con otros datos. Iniciá una venta nueva.";
  if (m.includes("numbering range is exhausted")) return "Se agotó la numeración autorizada por la DIAN.";
  if (m.includes("resolution has expired")) return "La resolución de numeración DIAN está vencida.";
  if (m.includes("already an open cash session")) return "Ya tenés un turno de caja abierto en esta sucursal.";
  if (m.includes("cash session is already closed")) return "Este turno de caja ya está cerrado.";
  if (m.includes("withdrawal exceeds")) return "No hay tanto efectivo en la gaveta para ese retiro.";
  if (m.includes("reason is required")) return "Escribí el motivo del movimiento.";
  if (m.includes("insufficient stock for transfer")) return "No hay stock disponible suficiente para trasladar.";
  if (m.includes("branches must differ")) return "Elegí una sucursal de destino distinta.";
  if (m.includes("adjustment violates stock")) return "El ajuste dejaría el stock en negativo o por debajo de lo reservado.";
  if (error.status === 401) return "Tu sesión expiró. Iniciá sesión de nuevo.";
  if (error.status === 403) return "No tenés permiso para esta operación.";
  return m;
}

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
import {
  mockPurchaseCancel,
  mockPurchaseCreate,
  mockPurchaseGet,
  mockPurchaseReceive,
  mockPurchasesList,
  mockPurchaseSummary,
  mockSupplierCreate,
  mockSuppliersList,
  mockSupplierUpdate,
} from "./mock-procurement";
import {
  mockGetAudit,
  mockGetSettings,
  mockInviteUser,
  mockPermissionsList,
  mockRolePermissions,
  mockUpdateRolePermissions,
  mockUpdateSettings,
  mockUpdateUserRole,
  mockUpdateUserStatus,
  mockUsersList,
  mockRecordLoginAttempt,
  PERMISSION_CATALOG,
} from "./mock-platform";
import type {
  AuditEntry,
  AuditFilter,
  BranchAccess,
  BranchSettings,
  CashActivity,
  CashActivityReport,
  CashMovementKind,
  CashMovementReasonCode,
  CashRegister,
  CashSession,
  CashSessionRow,
  CashSummary,
  MovementByKind,
  CatalogItem,
  CheckoutRequest,
  CheckoutResponse,
  QuoteRequest,
  QuoteResponse,
  QuoteLine,
  PriceTier,
  CreatePriceTierInput,
  UpdatePriceTierInput,
  ProductTierPrice,
  SetProductTierPriceInput,
  CreateCustomerInput,
  CreatePurchaseInput,
  CreateSupplierInput,
  Customer,
  CustomerQuery,
  DashboardSummary,
  Decimal,
  DocType,
  Invitation,
  InviteUserInput,
  LoginAttemptInput,
  Movement,
  OrderItem,
  Payment,
  Permission,
  Purchase,
  PurchaseFilter,
  PurchaseSummary,
  Role,
  RolePermissionsResponse,
  SessionContext,
  StockLevel,
  Supplier,
  TenantUser,
  UpdateRolePermissionsInput,
  UpdateSupplierInput,
  UpdateUserRoleInput,
  UpdateUserStatusInput,
  Uuid,
} from "./types";

export * from "./types";
export { ApiError, setAccessToken, setTokenProvider, setUnauthorizedHandler, USE_MOCKS } from "./client";

const delay = (ms = 150) => new Promise((resolve) => setTimeout(resolve, ms));
const cents = (n: number) => Math.round(n * 100) / 100;
const dec = (n: number): Decimal => cents(n).toFixed(2);

// ------------------------------------------------------------ estado mock


const mockTiers: PriceTier[] = [
  {
    id: "t1111111-1111-1111-1111-111111111111",
    tenant_id: "00000000-0000-0000-0000-000000000000",
    name: "Mayorista",
    kind: "CLIENTE",
    adjustment: "DESCUENTO_PCT",
    value: "8.0000",
    units_per_package: null,
    active: true,
    created_at: new Date().toISOString(),
  },
  {
    id: "t2222222-2222-2222-2222-222222222222",
    tenant_id: "00000000-0000-0000-0000-000000000000",
    name: "Paca x12",
    kind: "PRESENTACION",
    adjustment: "DESCUENTO_PCT",
    value: "5.0000",
    units_per_package: "12.0000",
    active: true,
    created_at: new Date().toISOString(),
  },
  {
    id: "t3333333-3333-3333-3333-333333333333",
    tenant_id: "00000000-0000-0000-0000-000000000000",
    name: "Bulto 50 kg",
    kind: "PRESENTACION",
    adjustment: "NINGUNO",
    value: "0.0000",
    units_per_package: "50.0000",
    active: true,
    created_at: new Date().toISOString(),
  },
];
const mockProductTierPrices = new Map<Uuid, ProductTierPrice[]>();

let receiptSeq = 0;
const mockSales = new Map<Uuid, { body: string; response: CheckoutResponse; at: string }>();
/** Turno al que pertenece cada venta mock. */
const sessionOf = new Map<Uuid, Uuid>();
/** El demo arranca con la caja abierta para poder vender sin configurar nada. */
let mockCash: CashSession | null = newMockSession(100_000);
/** Ingresos y retiros de efectivo de los turnos mock. */
const mockCashMoves: {
  sessionId: Uuid;
  kind: CashMovementKind;
  amount: number;
  reason: string;
  reason_code?: CashMovementReasonCode | null;
  at: string;
}[] = [];
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
        register_id: "00000000-0000-4000-8000-000000000c01",
        register_name: "Caja 1",
        register_code: "CAJA-1",
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
      movements_by_kind: [],
      cash_in: "0.00",
      cash_out: "0.00",
      expected_cash: dec(expected),
      tips_total: "0.00",
      tips_paid: "0.00",
      tips_pending: "0.00",
      state_token: "mock-closed-state",
    };
  });
}

const mockRegisters: CashRegister[] = [
  {
    id: "00000000-0000-4000-8000-000000000c01",
    branch_id: MOCK_BRANCH_ID,
    name: "Caja 1",
    code: "CAJA-1",
    default_opening: "200000.00",
    active: true,
    open_session: null,
  },
  {
    id: "00000000-0000-4000-8000-000000000c02",
    branch_id: MOCK_BRANCH_ID,
    name: "Caja 2",
    code: "CAJA-2",
    default_opening: "100000.00",
    active: true,
    open_session: null,
  },
];

const mockCustomers: Customer[] = [
  {
    id: "c0000000-0000-0000-0000-000000000001",
    doc_type: "NIT",
    doc_number: "900123456",
    dv: "1",
    name: "Distribuidora El Triunfo SAS",
    phone: "3104567890",
    email: "contacto@eltriunfo.com.co",
    price_tier_id: null,
  },
  {
    id: "c0000000-0000-0000-0000-000000000002",
    doc_type: "CC",
    doc_number: "1020304050",
    dv: null,
    name: "Juan Carlos Pérez",
    phone: "3001234567",
    email: "juan.perez@gmail.com",
    price_tier_id: "t0000000-0000-0000-0000-000000000001",
  },
  {
    id: "c0000000-0000-0000-0000-000000000003",
    doc_type: "CC",
    doc_number: "52987654",
    dv: null,
    name: "María Fernanda Gómez",
    phone: "3159876543",
    email: "mafe.gomez@hotmail.com",
    price_tier_id: null,
  },
  {
    id: "c0000000-0000-0000-0000-000000000004",
    doc_type: "CC",
    doc_number: "80123456",
    dv: null,
    name: "Carlos Alberto Rodríguez",
    phone: "3201122334",
    email: "carlos.rodriguez@outlook.com",
    price_tier_id: null,
  },
  {
    id: "c0000000-0000-0000-0000-000000000005",
    doc_type: "CC",
    doc_number: "1032456789",
    dv: null,
    name: "Laura Camila Restrepo",
    phone: "3118899001",
    email: "laura.restrepo@yahoo.es",
    price_tier_id: null,
  },
  {
    id: "c0000000-0000-0000-0000-000000000006",
    doc_type: "CC",
    doc_number: "79654321",
    dv: null,
    name: "Andrés Felipe Morales",
    phone: "3187766554",
    email: "andres.morales@empresa.co",
    price_tier_id: null,
  },
];

function newMockSession(
  opening: number,
  registerId?: string,
  registerName?: string,
  registerCode?: string,
): CashSession {
  return {
    id: crypto.randomUUID(),
    branch_id: MOCK_BRANCH_ID,
    register_id: registerId ?? mockRegisters[0].id,
    register_name: registerName ?? mockRegisters[0].name,
    register_code: registerCode ?? mockRegisters[0].code,
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
  const movementMeta: Record<CashMovementKind, { label: string; is_inflow: boolean }> = {
    INGRESO: { label: "Entrada general", is_inflow: true },
    ABONO_CARTERA: { label: "Abono cartera", is_inflow: true },
    ABONO_SEPARADO: { label: "Abono plan separado", is_inflow: true },
    RECARGA_SALDO: { label: "Recarga saldo a favor", is_inflow: true },
    RETIRO: { label: "Retiro general", is_inflow: false },
    DEVOLUCION: { label: "Devolución", is_inflow: false },
    PROPINA_PAGADA: { label: "Pago de propina", is_inflow: false },
    PAGO_PROVEEDOR: { label: "Pago a proveedor", is_inflow: false },
    GASTO: { label: "Gasto menor", is_inflow: false },
  };

  const byKindMap = new Map<CashMovementKind, { amount: number; count: number }>();
  let cashIn = 0;
  let cashOut = 0;
  let tipsPaid = 0;
  for (const m of moves) {
    const meta = movementMeta[m.kind];
    const acc = byKindMap.get(m.kind) ?? { amount: 0, count: 0 };
    acc.amount += m.amount;
    acc.count += 1;
    byKindMap.set(m.kind, acc);
    if (meta.is_inflow) {
      cashIn += m.amount;
    } else {
      cashOut += m.amount;
    }
    if (m.kind === "PROPINA_PAGADA") {
      tipsPaid += m.amount;
    }
  }

  const movementsByKind: MovementByKind[] = [...byKindMap.entries()].map(([k, v]) => ({
    kind: k,
    code: k,
    label: movementMeta[k].label,
    amount: dec(v.amount),
    count: v.count,
    is_inflow: movementMeta[k].is_inflow,
  }));
  const tipsTotal = 0;
  const tipsPending = Math.max(0, tipsTotal - tipsPaid);

  return {
    session,
    sales_count: sales.length,
    sales_total: dec(sales.reduce((sum, s) => sum + Number(s.order.total_amount), 0)),
    by_method: [...byMethod.entries()].map(([method, v]) => ({
      method: method as Payment["method"],
      amount: dec(v.amount),
      count: v.count,
    })),
    movements_by_kind: movementsByKind,
    cash_in: dec(cashIn),
    cash_out: dec(cashOut),
    expected_cash: dec(Number(session.opening_amount) + cash + cashIn - cashOut),
    tips_total: dec(tipsTotal),
    tips_paid: dec(tipsPaid),
    tips_pending: dec(tipsPending),
    state_token: `mock-state-${session.id.slice(0, 8)}-${moves.length}-${sales.length}`,
  };
}

/** Bitacora del turno mock, igual que src/cash.rs::activity. */
function mockActivity(sessionId: Uuid): CashActivityReport {
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
  const items = entries.sort((a, b) => b.at.localeCompare(a.at));
  const token = mockCash && mockCash.id === sessionId ? mockSummary(mockCash).state_token : "mock-state";
  return { state_token: token, items };
}



function mockQuote(req: QuoteRequest): QuoteResponse {
  const products = new Map(mockCatalog.map((p) => [p.product_id, p]));
  let subtotalGross = 0;
  let discountTotal = 0;
  let subtotal = 0;
  let taxTotal = 0;

  const lines: QuoteLine[] = req.items.map((item, index) => {
    const product = item.product_id ? products.get(item.product_id) : undefined;
    const name = item.custom_name || product?.name || "Ítem libre";
    const qty = Number(item.qty);
    const unitPrice = item.unit_price_override ? Number(item.unit_price_override) : (product?.price ? Number(product.price) : 0);
    const gross = cents(qty * unitPrice);
    const discPct = item.discount_pct ? Number(item.discount_pct) : 0;
    const manualDisc = cents(gross * (discPct / 100));
    const lineTotal = cents(gross - manualDisc);
    const rate = Number(product?.tax_rate ?? 0);
    const taxAmount = cents(lineTotal * rate);

    subtotalGross += gross;
    discountTotal += manualDisc;
    subtotal += lineTotal;
    taxTotal += taxAmount;

    return {
      index,
      product_id: item.product_id ?? null,
      name,
      qty: item.qty,
      units_per_package: "1",
      list_price: dec(unitPrice),
      unit_price: dec(unitPrice),
      gross: dec(gross),
      discount: {
        manual: dec(manualDisc),
        promotion: "0.00",
        order: "0.00",
        coupon: "0.00",
        total: dec(manualDisc),
      },
      promotion_id: null,
      line_total: dec(lineTotal),
      tax_rate: rate.toFixed(4),
      tax_amount: dec(taxAmount),
    };
  });

  const tip = req.tip_amount ? Number(req.tip_amount) : 0;
  const total = cents(subtotal + taxTotal + tip);

  return {
    lines,
    subtotal_gross: dec(subtotalGross),
    discount_total: dec(discountTotal),
    subtotal: dec(subtotal),
    tax_total: dec(taxTotal),
    tip_amount: dec(tip),
    total: dec(total),
    promotions: [],
    coupon: null,
    coupon_error: null,
  };
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
  const quote = mockQuote({
    branch_id: body.branch_id,
    items: body.items,
    tip_amount: body.tip_amount,
    customer_id: body.customer_id,
    customer_alias: body.customer_alias,
    coupon_code: body.coupon_code,
  });

  if (body.quoted_total && cents(Number(body.quoted_total)) !== Number(quote.total)) {
    throw new ApiError(409, "Los precios cambiaron. Revisá el total.", "PRICE_CHANGED", { total: quote.total });
  }

  const items: OrderItem[] = quote.lines.map((ql, idx) => {
    const input = body.items[idx];
    if (input.product_id) {
      const product = products.get(input.product_id);
      const qty = Number(input.qty);
      if (product?.available != null && Number(product.available) < qty) {
        throw new ApiError(409, `insufficient stock for product ${input.product_id}`);
      }
    }
    return {
      id: crypto.randomUUID(),
      product_id: input.product_id ?? null,
      custom_name: input.custom_name ?? null,
      name: ql.name,
      tier_id: input.tier_id ?? null,
      units_per_package: ql.units_per_package,
      qty: ql.qty,
      unit_price: ql.unit_price,
      list_price: ql.list_price,
      discount_manual: ql.discount.manual,
      discount_promotion: ql.discount.promotion,
      discount_order: ql.discount.order,
      discount_coupon: ql.discount.coupon,
      promotion_id: ql.promotion_id ?? null,
      price_override_reason: input.price_override_reason ?? null,
      line_discount: ql.discount.total,
      line_total: ql.line_total,
      tax_rate: ql.tax_rate,
      tax_amount: ql.tax_amount,
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
    if (line.product_id) {
      const product = products.get(line.product_id);
      if (product?.available != null) {
        mockRecordMovement(line.product_id, "DESCUENTO", Number(line.qty), "venta", { type: "venta", id: orderId });
      }
    }
  }

  receiptSeq += 1;
  sessionOf.set(orderId, mockCash.id);
  const cust = body.customer_id ? mockCustomers.find((c) => c.id === body.customer_id) : null;
  const response: CheckoutResponse = {
    order: {
      id: orderId,
      sale_type: "MOSTRADOR",
      status: "COBRADA",
      table_id: null,
      branch_id: body.branch_id,
      waiter_id: null,
      customer_id: body.customer_id ?? null,
      customer_alias: body.customer_alias ?? null,
      customer_name: cust ? cust.name : null,
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
        return {
          uid: null,
          tenant: null,
          branch: MOCK_BRANCH_ID,
          role: "DUENO",
          permissions: PERMISSION_CATALOG.map((p) => p.code),
        };
      }
      return request("/v1/context");
    },

    /** GET /v1/branches — sucursales del usuario con su rol en cada una. */
    async branches(): Promise<BranchAccess[]> {
      if (USE_MOCKS) {
        await delay();
        return [{ id: MOCK_BRANCH_ID, name: "Sucursal Principal", code: null, role: "DUENO" }];
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
    /** POST /v1/sales/quote */
    async quote(req: QuoteRequest): Promise<QuoteResponse> {
      if (!USE_MOCKS) {
        return request("/v1/sales/quote", { method: "POST", body: JSON.stringify(req) });
      }
      await delay(100);
      return structuredClone(mockQuote(req));
    },

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
    async open(branchId: Uuid, registerId: Uuid, openingAmount: Decimal): Promise<CashSession> {
      if (!USE_MOCKS) {
        return request("/v1/cash-sessions", {
          method: "POST",
          body: JSON.stringify({ branch_id: branchId, register_id: registerId, opening_amount: openingAmount }),
        });
      }
      await delay();
      if (mockCash) throw new ApiError(409, "there is already an open cash session", "USER_HAS_OPEN_SESSION");
      const reg = mockRegisters.find((r) => r.id === registerId);
      if (!reg || !reg.active) {
        throw new ApiError(422, "La caja no existe o no está activa", "REGISTER_INVALID");
      }
      if (reg.open_session) {
        throw new ApiError(409, "La caja ya tiene un turno abierto", "REGISTER_BUSY", {
          opened_by_name: reg.open_session.opened_by_name,
        });
      }
      mockCash = newMockSession(Number(openingAmount), registerId, reg.name, reg.code);
      reg.open_session = {
        id: mockCash.id,
        opened_by: mockCash.opened_by,
        opened_by_name: "Tú",
        opened_at: mockCash.opened_at,
      };
      return structuredClone(mockCash);
    },

    /** POST /v1/cash-sessions/{id}/close */
    async close(sessionId: Uuid, countedCash: Decimal, stateToken: string, notes?: string): Promise<CashSummary> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-sessions/${sessionId}/close`, {
          method: "POST",
          body: JSON.stringify({ counted_cash: countedCash, state_token: stateToken, notes: notes ?? null }),
        });
      }
      await delay();
      if (!mockCash || mockCash.id !== sessionId) throw new ApiError(409, "cash session is already closed");
      const curSummary = mockSummary(mockCash);
      if (curSummary.state_token !== stateToken) {
        throw new ApiError(409, "Hubo movimientos nuevos mientras contabas. Volvé a contar.", "CASH_STATE_CHANGED");
      }
      const expected = Number(curSummary.expected_cash);
      const counted = Number(countedCash);
      const reg = mockRegisters.find((r) => r.id === mockCash?.register_id);
      if (reg) reg.open_session = null;
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

    /** POST /v1/cash-sessions/{id}/movements — ingreso, retiro o pago de propina; devuelve el resumen. */
    async addMovement(
      sessionId: Uuid,
      kind: CashMovementKind,
      amount: Decimal,
      reason?: string | null,
      reasonCode?: CashMovementReasonCode | null,
    ): Promise<CashSummary> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-sessions/${sessionId}/movements`, {
          method: "POST",
          body: JSON.stringify({ kind, amount, reason, reason_code: reasonCode }),
        });
      }
      await delay();
      if (!mockCash || mockCash.id !== sessionId) throw new ApiError(409, "cash session is already closed");
      if (Number(amount) <= 0) throw new ApiError(422, "validation: amount must be > 0");
      if (!["INGRESO", "RETIRO", "PROPINA_PAGADA"].includes(kind)) {
        throw new ApiError(422, "tipo no permitido", "CASH_KIND_NOT_ALLOWED");
      }
      const trimmedReason = reason?.trim() || "";
      if (kind === "INGRESO" && (!trimmedReason || trimmedReason.length < 3 || trimmedReason.length > 200)) {
        throw new ApiError(422, "validation: reason must be between 3 and 200 chars");
      }
      if (kind === "RETIRO") {
        if (!reasonCode) {
          throw new ApiError(422, "validation: reason_code is required for withdrawal");
        }
        if (reasonCode === "OTRO" && (!trimmedReason || trimmedReason.length < 3 || trimmedReason.length > 200)) {
          throw new ApiError(422, "validation: reason must be between 3 and 200 chars when reason_code is OTRO");
        }
      }
      if (kind === "PROPINA_PAGADA" && !trimmedReason) {
        throw new ApiError(422, "validation: waiter name is required");
      }
      if (
        (kind === "RETIRO" || kind === "PROPINA_PAGADA") &&
        Number(amount) > Number(mockSummary(mockCash).expected_cash)
      ) {
        throw new ApiError(409, "withdrawal exceeds the cash in the drawer", "INSUFFICIENT_CASH");
      }
      mockCashMoves.push({
        sessionId,
        kind,
        amount: Number(amount),
        reason: trimmedReason || reasonCode || "",
        reason_code: reasonCode ?? null,
        at: new Date().toISOString(),
      });
      return structuredClone(mockSummary(mockCash));
    },

    /** GET /v1/cash-sessions/{id}/activity — bitacora del turno con token de estado. */
    async activity(sessionId: Uuid): Promise<CashActivityReport> {
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

  cashRegisters: {
    async list(branchId: Uuid): Promise<CashRegister[]> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-registers?branch_id=${branchId}`);
      }
      await delay();
      return structuredClone(mockRegisters.filter((r) => r.branch_id === branchId || branchId === MOCK_BRANCH_ID));
    },

    async create(req: { branch_id: Uuid; name: string; code: string; default_opening: number }): Promise<CashRegister> {
      if (!USE_MOCKS) {
        return request("/v1/cash-registers", {
          method: "POST",
          body: JSON.stringify(req),
        });
      }
      await delay();
      const code = req.code.trim().toUpperCase();
      if (mockRegisters.some((r) => r.code === code)) {
        throw new ApiError(409, "Ya existe una caja con ese código en la sucursal", "REGISTER_CODE_TAKEN");
      }
      const newReg: CashRegister = {
        id: crypto.randomUUID(),
        branch_id: req.branch_id,
        name: req.name.trim(),
        code,
        default_opening: dec(req.default_opening),
        active: true,
        open_session: null,
      };
      mockRegisters.push(newReg);
      return structuredClone(newReg);
    },

    async update(
      id: Uuid,
      req: { name: string; code: string; default_opening: number; active: boolean },
    ): Promise<CashRegister> {
      if (!USE_MOCKS) {
        return request(`/v1/cash-registers/${id}`, {
          method: "PUT",
          body: JSON.stringify(req),
        });
      }
      await delay();
      const idx = mockRegisters.findIndex((r) => r.id === id);
      if (idx === -1) throw new ApiError(404, "Caja no encontrada");
      const current = mockRegisters[idx];
      const code = req.code.trim().toUpperCase();
      if (mockRegisters.some((r) => r.id !== id && r.code === code)) {
        throw new ApiError(409, "Ya existe una caja con ese código en la sucursal", "REGISTER_CODE_TAKEN");
      }
      if (!req.active && current.open_session) {
        throw new ApiError(409, "No se puede desactivar una caja con un turno abierto", "REGISTER_HAS_OPEN_SESSION");
      }
      mockRegisters[idx] = {
        ...current,
        name: req.name.trim(),
        code,
        default_opening: dec(req.default_opening),
        active: req.active,
      };
      return structuredClone(mockRegisters[idx]);
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

  suppliers: {
    /** GET /v1/suppliers?q=&include_inactive= */
    async list(q?: string, includeInactive?: boolean): Promise<Supplier[]> {
      if (USE_MOCKS) {
        await delay();
        return mockSuppliersList(q, includeInactive);
      }
      const params = new URLSearchParams();
      if (q) params.set("q", q);
      if (includeInactive) params.set("include_inactive", "true");
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/suppliers${qs}`);
    },

    /** POST /v1/suppliers */
    async create(body: CreateSupplierInput): Promise<Supplier> {
      if (USE_MOCKS) {
        await delay();
        return mockSupplierCreate(body);
      }
      return request("/v1/suppliers", { method: "POST", body: JSON.stringify(body) });
    },

    /** PUT /v1/suppliers/{id} */
    async update(id: Uuid, body: UpdateSupplierInput): Promise<Supplier> {
      if (USE_MOCKS) {
        await delay();
        return mockSupplierUpdate(id, body);
      }
      return request(`/v1/suppliers/${id}`, { method: "PUT", body: JSON.stringify(body) });
    },
  },

  purchases: {
    /** GET /v1/purchases?branch_id=&from=&to=&supplier_id=&status=&limit= */
    async list(filter?: PurchaseFilter): Promise<Purchase[]> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchasesList(filter);
      }
      const params = new URLSearchParams();
      if (filter?.branch_id) params.set("branch_id", filter.branch_id);
      if (filter?.from) params.set("from", filter.from);
      if (filter?.to) params.set("to", filter.to);
      if (filter?.supplier_id) params.set("supplier_id", filter.supplier_id);
      if (filter?.status) params.set("status", filter.status);
      if (filter?.limit) params.set("limit", String(filter.limit));
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/purchases${qs}`);
    },

    /** GET /v1/purchases/{id} */
    async get(id: Uuid): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseGet(id);
      }
      return request(`/v1/purchases/${id}`);
    },

    /** POST /v1/purchases */
    async create(body: CreatePurchaseInput): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseCreate(body);
      }
      return request("/v1/purchases", { method: "POST", body: JSON.stringify(body) });
    },

    /** POST /v1/purchases/{id}/receive */
    async receive(id: Uuid): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseReceive(id);
      }
      return request(`/v1/purchases/${id}/receive`, { method: "POST" });
    },

    /** POST /v1/purchases/{id}/cancel */
    async cancel(id: Uuid): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseCancel(id);
      }
      return request(`/v1/purchases/${id}/cancel`, { method: "POST" });
    },

    /** GET /v1/purchases/summary?branch_id=&month= */
    async summary(branchId?: Uuid, month?: string): Promise<PurchaseSummary> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseSummary(branchId, month);
      }
      const params = new URLSearchParams();
      if (branchId) params.set("branch_id", branchId);
      if (month) params.set("month", month);
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/purchases/summary${qs}`);
    },
  },


  tiers: {
    async list(): Promise<PriceTier[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockTiers);
      }
      return request("/v1/price-tiers");
    },
    async create(input: CreatePriceTierInput): Promise<PriceTier> {
      if (!USE_MOCKS) {
        return request("/v1/price-tiers", { method: "POST", body: JSON.stringify(input) });
      }
      await delay();
      if (mockTiers.some((t) => t.name.toLowerCase() === input.name.trim().toLowerCase())) {
        throw new ApiError(409, "Ya existe una tarifa con ese nombre");
      }
      const tier: PriceTier = {
        id: crypto.randomUUID(),
        tenant_id: "00000000-0000-0000-0000-000000000000",
        name: input.name.trim(),
        kind: input.kind,
        adjustment: input.adjustment,
        value: input.value,
        units_per_package: input.units_per_package ?? null,
        active: true,
        created_at: new Date().toISOString(),
      };
      mockTiers.push(tier);
      return structuredClone(tier);
    },
    async update(id: Uuid, input: UpdatePriceTierInput): Promise<PriceTier> {
      if (!USE_MOCKS) {
        return request(`/v1/price-tiers/${id}`, { method: "PUT", body: JSON.stringify(input) });
      }
      await delay();
      const existing = mockTiers.find((t) => t.id === id);
      if (!existing) throw new ApiError(404, "Tarifa no encontrada");
      if (existing.kind !== input.kind) {
        throw new ApiError(422, "No se puede cambiar el tipo de una tarifa", "TIER_KIND_IMMUTABLE");
      }
      existing.name = input.name.trim();
      existing.adjustment = input.adjustment;
      existing.value = input.value;
      existing.units_per_package = input.units_per_package ?? null;
      existing.active = input.active;
      return structuredClone(existing);
    },
    async getProductPrices(productId: Uuid): Promise<ProductTierPrice[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockProductTierPrices.get(productId) ?? []);
      }
      return request(`/v1/products/${productId}/tier-prices`);
    },
    async setProductPrices(productId: Uuid, items: SetProductTierPriceInput[]): Promise<ProductTierPrice[]> {
      if (!USE_MOCKS) {
        return request(`/v1/products/${productId}/tier-prices`, { method: "PUT", body: JSON.stringify(items) });
      }
      await delay();
      mockProductTierPrices.set(productId, items);
      return structuredClone(items);
    },
  },

  users: {
    async list(): Promise<{ users: TenantUser[]; invitations: Invitation[] }> {
      if (USE_MOCKS) {
        await delay();
        return mockUsersList();
      }
      return request("/v1/users");
    },

    async invite(input: InviteUserInput): Promise<Invitation> {
      if (USE_MOCKS) {
        await delay();
        return mockInviteUser(input);
      }
      return request("/v1/users/invite", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    async updateRole(userId: Uuid, input: UpdateUserRoleInput): Promise<TenantUser> {
      if (USE_MOCKS) {
        await delay();
        return mockUpdateUserRole(userId, input.role);
      }
      return request(`/v1/users/${userId}/role`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },

    async updateStatus(userId: Uuid, input: UpdateUserStatusInput): Promise<TenantUser> {
      if (USE_MOCKS) {
        await delay();
        return mockUpdateUserStatus(userId, input.active);
      }
      return request(`/v1/users/${userId}/status`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },
  },

  permissions: {
    async list(): Promise<{ permissions: Permission[]; roles: Role[] }> {
      if (USE_MOCKS) {
        await delay();
        return mockPermissionsList();
      }
      return request("/v1/permissions");
    },

    async getRole(role: Role): Promise<RolePermissionsResponse> {
      if (USE_MOCKS) {
        await delay();
        return mockRolePermissions(role);
      }
      return request(`/v1/permissions/roles/${role}`);
    },

    async updateRole(role: Role, input: UpdateRolePermissionsInput): Promise<RolePermissionsResponse> {
      if (USE_MOCKS) {
        await delay();
        return mockUpdateRolePermissions(role, input);
      }
      return request(`/v1/permissions/roles/${role}`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },
  },

  settings: {
    async get(branchId: Uuid): Promise<BranchSettings> {
      if (USE_MOCKS) {
        await delay();
        return mockGetSettings(branchId);
      }
      return request(`/v1/settings?branch_id=${branchId}`);
    },

    async update(branchId: Uuid, settings: BranchSettings): Promise<BranchSettings> {
      if (USE_MOCKS) {
        await delay();
        return mockUpdateSettings(branchId, settings);
      }
      return request(`/v1/settings?branch_id=${branchId}`, {
        method: "PUT",
        body: JSON.stringify(settings),
      });
    },
  },

  audit: {
    async list(filter?: AuditFilter): Promise<AuditEntry[]> {
      if (USE_MOCKS) {
        await delay();
        return mockGetAudit(filter);
      }
      const params = new URLSearchParams();
      if (filter?.branch_id) params.set("branch_id", filter.branch_id);
      if (filter?.user_id) params.set("user_id", filter.user_id);
      if (filter?.action) params.set("action", filter.action);
      if (filter?.from) params.set("from", filter.from);
      if (filter?.to) params.set("to", filter.to);
      if (filter?.limit) params.set("limit", String(filter.limit));
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/audit${qs}`);
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

  auth: {
    /** POST /v1/auth/login-attempt (verificación previa antes de iniciar sesión con Supabase) */
    async checkLoginAttempt(email: string): Promise<void> {
      if (USE_MOCKS) {
        mockRecordLoginAttempt({ email });
        return;
      }
      await request("/v1/auth/login-attempt", {
        method: "POST",
        body: JSON.stringify({ email }),
      });
    },

    /** POST /v1/auth/login-attempt (registro tras éxito o fallo de autenticación) */
    async recordLoginAttempt(input: LoginAttemptInput): Promise<void> {
      if (USE_MOCKS) {
        mockRecordLoginAttempt(input);
        return;
      }
      await request("/v1/auth/login-attempt", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },
  },

  customers: {
    /** GET /v1/customers?q=&limit= */
    async search(query?: CustomerQuery): Promise<Customer[]> {
      if (USE_MOCKS) {
        await delay();
        const q = query?.q?.trim().toLowerCase() || "";
        const limit = Math.min(Math.max(query?.limit || 20, 1), 50);
        if (!q) {
          return structuredClone(mockCustomers.slice(0, limit));
        }
        const filtered = mockCustomers.filter(
          (c) =>
            c.name.toLowerCase().includes(q) ||
            (c.doc_number && c.doc_number.toLowerCase().startsWith(q)) ||
            (c.phone && c.phone.startsWith(q)),
        );
        return structuredClone(filtered.slice(0, limit));
      }
      const params = new URLSearchParams();
      if (query?.q) params.set("q", query.q);
      if (query?.limit) params.set("limit", String(query.limit));
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/customers${qs}`);
    },

    /** POST /v1/customers */
    async create(input: CreateCustomerInput): Promise<Customer> {
      if (USE_MOCKS) {
        await delay();
        const name = input.name.trim().replace(/\s+/g, " ");
        if (!name || name.length > 120) {
          throw new ApiError(422, "El nombre debe tener entre 1 y 120 caracteres", "CUSTOMER_INVALID");
        }
        let doc_number = input.doc_number ? input.doc_number.replace(/[^a-zA-Z0-9]/g, "").toUpperCase() : null;
        let doc_type = input.doc_type ? (input.doc_type.trim().toUpperCase() as DocType) : null;
        if (!doc_number) doc_number = null;
        if (!doc_type) doc_type = null;
        if ((doc_number === null) !== (doc_type === null)) {
          throw new ApiError(422, "Tipo y número de documento deben proporcionarse juntos", "CUSTOMER_INVALID");
        }
        let dv = input.dv ? input.dv.replace(/[^0-9]/g, "") : null;
        if (!dv) dv = null;
        if (dv && doc_type !== "NIT") {
          throw new ApiError(422, "El dígito de verificación (DV) solo aplica para NIT", "CUSTOMER_INVALID");
        }
        if (dv && dv.length !== 1) {
          throw new ApiError(422, "El DV debe ser un único dígito", "CUSTOMER_INVALID");
        }
        let phone = input.phone ? input.phone.replace(/[^0-9]/g, "") : null;
        if (!phone) phone = null;
        if (phone && (phone.length < 7 || phone.length > 15)) {
          throw new ApiError(422, "El teléfono debe tener entre 7 y 15 dígitos", "CUSTOMER_INVALID");
        }
        let email = input.email ? input.email.trim().toLowerCase() : null;
        if (!email) email = null;
        if (email) {
          const atCount = (email.match(/@/g) || []).length;
          if (atCount !== 1 || email.startsWith("@") || email.endsWith("@") || email.length < 3) {
            throw new ApiError(422, "El correo electrónico no es válido", "CUSTOMER_INVALID");
          }
        }
        if (doc_number && doc_type) {
          const dup = mockCustomers.find(
            (c) => c.doc_type === doc_type && c.doc_number?.toUpperCase() === doc_number?.toUpperCase(),
          );
          if (dup) {
            throw new ApiError(409, "Ya existe un cliente con ese documento.", "CUSTOMER_DUPLICATED", { id: dup.id });
          }
        }
        const newCust: Customer = {
          id: `c0000000-0000-0000-0000-${String(mockCustomers.length + 1).padStart(12, "0")}`,
          name,
          doc_type,
          doc_number,
          dv,
          phone,
          email,
          price_tier_id: null,
        };
        mockCustomers.push(newCust);
        return structuredClone(newCust);
      }
      return request("/v1/customers", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },
  },
};

export const ERROR_MESSAGES: Record<string, string> = {
  REGISTER_INVALID: "Revisá los datos de la caja.",
  REGISTER_CODE_TAKEN: "Ya existe una caja con ese código.",
  REGISTER_HAS_OPEN_SESSION: "La caja tiene un turno abierto. Cerralo primero.",
  REGISTER_BUSY: "Esa caja ya tiene un turno abierto.",
  USER_HAS_OPEN_SESSION: "Ya tenés un turno abierto en esta sucursal.",
  CASH_AMOUNT_INVALID: "El monto debe ser mayor a cero.",
  CASH_INSUFFICIENT: "No hay suficiente efectivo en la caja.",
  CASH_KIND_NOT_ALLOWED: "Ese tipo de movimiento no se registra a mano.",
  CASH_STATE_CHANGED: "Hubo movimientos nuevos mientras contabas. Volvé a contar.",
  CUSTOMER_INVALID: "Revisá los datos del cliente.",
  CUSTOMER_DUPLICATED: "Ya existe un cliente con ese documento.",
  SALE_CUSTOMER_AND_ALIAS: "Elegí cliente o alias, no los dos.",
  SALE_ALIAS_INVALID: "El alias debe tener entre 1 y 40 caracteres.",
  SALE_ALIAS_NOT_ALLOWED: "Esta sucursal no permite ventas con alias.",
  SALE_CUSTOMER_REQUIRED: "Esta sucursal exige un cliente en cada venta.",
  SALE_CUSTOMER_INVALID: "El cliente no existe o está archivado.",
  PRICE_CHANGED: "Los precios cambiaron. Revisá el total.",
  DISCOUNT_ABOVE_MAX: "El descuento supera el máximo permitido.",
  TIER_INVALID: "La tarifa no es válida para esta línea.",
  TIER_MISSING_UNITS: "La presentación no tiene unidades configuradas.",
  TIER_KIND_IMMUTABLE: "No se puede cambiar el tipo de una tarifa.",
  BARCODE_TAKEN: "Ese código de barras ya está en uso.",
  PROMOTION_INVALID: "Revisá los datos de la promoción.",
  PROMOTION_NOT_EDITABLE: "Pausá la promoción para editarla.",
  PROMOTION_EXHAUSTED: "Una promoción se agotó. Revisá el total.",
  COUPON_NOT_FOUND: "El cupón no existe.",
  COUPON_INACTIVE: "El cupón no está activo.",
  COUPON_NOT_STARTED: "El cupón todavía no está vigente.",
  COUPON_EXPIRED: "El cupón venció.",
  COUPON_EXHAUSTED: "El cupón se agotó.",
  COUPON_REQUIRES_CUSTOMER: "Este cupón necesita un cliente identificado.",
  COUPON_CUSTOMER_LIMIT: "Este cliente ya usó el cupón.",
  COUPON_MIN_SUBTOTAL: "La compra no alcanza el mínimo del cupón.",
  COUPON_NOT_APPLICABLE: "El cupón no aplica a estos productos.",
  COUPON_CODE_TAKEN: "Ya existe un cupón con ese código.",
  PRICE_OVERRIDE_DISABLED: "Esta sucursal no permite cambiar precios.",
  PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT: "Este producto no permite cambiar el precio.",
  PRICE_OVERRIDE_REASON_REQUIRED: "Escribí el motivo del cambio de precio.",
  CUSTOM_ITEM_INVALID: "Revisá el ítem libre.",
  TIPS_DISABLED: "Las propinas están desactivadas.",
  TIP_INVALID: "Revisá la propina y el mesero.",
  IDEMPOTENCY_CONFLICT: "Esta operación ya se registró con otros datos.",
  REFUND_ORDER_INVALID: "Esta venta no admite devoluciones.",
  REFUND_INVALID: "Revisá las líneas de la devolución.",
  REFUND_QTY_EXCEEDED: "La cantidad supera lo que queda por devolver.",
  REFUND_ORIGINAL_MIXED: "La venta tuvo varios medios de pago: devolvé en efectivo.",
  REFUND_REFERENCE_REQUIRED: "Escribí la referencia del reverso.",
  REFUND_NEEDS_OPEN_SESSION: "Abrí un turno de caja para devolver efectivo.",
  REFUND_NOT_PENDING: "La devolución ya fue decidida.",
  VOID_NOT_ALLOWED: "No se puede anular: la venta no es de hoy, el turno cerró, tiene devoluciones o ya fue facturada.",
  VOID_REASON_REQUIRED: "Escribí el motivo de la anulación.",
};

/** Mensajes del backend traducidos para el cajero. */
export function friendlyError(error: unknown): string {
  if (!(error instanceof ApiError)) return "No se pudo conectar con el servidor. Intentá de nuevo.";
  if (error.code && ERROR_MESSAGES[error.code]) {
    return ERROR_MESSAGES[error.code];
  }
  const m = error.message;
  if (error.status === 429 || m.includes("too many requests") || m.includes("intentos fallidos")) {
    return "Demasiados intentos fallidos. Tu cuenta está bloqueada temporalmente (15 minutos).";
  }
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
  if (m.includes("name must not be empty")) return "Escribí el nombre del proveedor.";
  if (m.includes("conflict: duplicated record")) return "Ya existe un registro con ese documento o NIT en este negocio.";
  if (m.includes("purchase has no items")) return "Agregá al menos un producto a la compra.";
  if (m.includes("purchase is already received")) return "Esta compra ya fue recibida.";
  if (m.includes("cannot cancel an already received purchase")) return "No se puede anular una compra ya recibida.";
  if (m.includes("purchase is already cancelled")) return "Esta compra ya está anulada.";
  if (m.includes("composed product")) return "Los productos compuestos (recetas) no se pueden comprar directamente.";
  if (m.includes("cannot modify owner role")) return "No podés quitarte tu propio rol de dueño.";
  if (m.includes("cannot modify DUENO permissions")) return "Los permisos del Dueño no se pueden modificar.";
  if (m.includes("price override is not allowed")) return "No está permitido cambiar el precio manualmente en esta sucursal.";
  if (m.includes("discount exceeds maximum allowed")) return "El descuento supera el máximo permitido para esta sucursal.";
  if (m.includes("notes are required when cash difference exceeds tolerance")) return "Tenés que justificar el descuadre de caja en las notas de cierre.";
  if (m.includes("one session per user")) return "Ya tenés un turno de caja abierto en esta sucursal.";
  if (m.includes("negative stock")) return "No se permite dejar el stock en negativo en esta sucursal.";
  if (m.includes("refund requires approval")) return "Se requiere autorización de un administrador para cancelar esta orden.";
  if (error.status === 401) return "Tu sesión expiró. Iniciá sesión de nuevo.";
  if (error.status === 403) return "No tenés permiso para esta operación.";
  return m;
}

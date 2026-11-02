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
  mockPurchaseApprove,
  mockPurchaseCancel,
  mockPurchaseCreate,
  mockPurchaseGet,
  mockPurchaseReceive,
  mockPurchasesCostPreview,
  mockPurchasesList,
  mockPurchaseSummary,
  mockPurchaseUpdate,
  mockPayablesList,
  mockPayablesAging,
  mockPayableGet,
  mockPayablePayment,
  mockPayableWriteOff,
  mockExpenseCategoriesList,
  mockExpenseCategoryCreate,
  mockExpenseCategoryUpdate,
  mockExpensesList,
  mockExpenseGet,
  mockExpensesSummary,
  mockExpenseCreate,
  mockExpenseApprove,
  mockExpenseReject,
  mockExpenseCancel,
  mockSupplierArchive,
  mockSupplierCreate,
  mockSupplierGet,
  mockSuppliersList,
  mockSupplierSetProducts,
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
  mockWaiters,
  mockBusinessInfo,
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
  Promotion,
  CreatePromotionInput,
  UpdatePromotionInput,
  Coupon,
  CreateCouponInput,
  UpdateCouponInput,
  CreateCustomerInput,
  CreatePurchaseInput,
  UpdatePurchaseInput,
  CostPreviewRequest,
  CostPreviewResponse,
  ReceivePurchaseRequest,
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
  Payable,
  PayableDetail,
  PayablesAging,
  PayablesFilter,
  CreatePayablePaymentInput,
  WriteOffPayableInput,
  CreateExpenseCategoryInput,
  CreateExpenseInput,
  Expense,
  ExpenseCategory,
  ExpenseSummary,
  ExpenseSummaryFilter,
  ExpensesFilter,
  RejectExpenseInput,
  UpdateExpenseCategoryInput,
  Permission,
  Purchase,
  PurchaseFilter,
  PurchaseSummary,
  Role,
  RolePermissionsResponse,
  SessionContext,
  StockAlert,
  StockLevel,
  Supplier,
  SupplierDetailResponse,
  SupplierProductDetail,
  SupplierProductItemInput,
  TenantUser,
  UpdateRolePermissionsInput,
  UpdateSupplierInput,
  UpdateUserRoleInput,
  UpdateUserStatusInput,
  Uuid,
  Waiter,
  Order,
  PaymentMethod,
  Refund,
  RefundInput,
  RefundItem,
  RefundStatus,
  SaleDetail,
  SaleHistoryPage,
  SaleHistoryQuery,
  SaleHistoryRow,
  BusinessInfo,
  CreateVariantInput,
  ProductDetail,
  ProductInput,
  ProductListParams,
  ProductListResponse,
  ProductRow,
  UpdateBranchProductInput,
  CreateOptionGroupInput,
  OptionGroupDetail,
  UpdateProductOptionGroupInput,
  AnalyzeResponse,
  CommitResponse,
  ImportRowResult,
  LocationRow,
  CreateLocationInput,
  UpdateLocationInput,
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

const mockOptionGroups: OptionGroupDetail[] = [
  {
    id: "og-11111111-1111-1111-1111-111111111111",
    name: "Toppings y Adiciones",
    min_select: 0,
    max_select: 5,
    active: true,
    options: [
      {
        id: "opt-11111111-1111-1111-1111-111111111111",
        group_id: "og-11111111-1111-1111-1111-111111111111",
        name: "Extra Queso",
        price_delta: "2000.00",
        input_product_id: null,
        input_qty: null,
        sort_order: 1,
        active: true,
      },
      {
        id: "opt-22222222-2222-2222-2222-222222222222",
        group_id: "og-11111111-1111-1111-1111-111111111111",
        name: "Tocineta",
        price_delta: "3500.00",
        input_product_id: null,
        input_qty: null,
        sort_order: 2,
        active: true,
      },
    ],
  },
  {
    id: "og-22222222-2222-2222-2222-222222222222",
    name: "Término de la carne",
    min_select: 1,
    max_select: 1,
    active: true,
    options: [
      {
        id: "opt-33333333-3333-3333-3333-333333333333",
        group_id: "og-22222222-2222-2222-2222-222222222222",
        name: "Tres cuartos (3/4)",
        price_delta: "0.00",
        input_product_id: null,
        input_qty: null,
        sort_order: 1,
        active: true,
      },
      {
        id: "opt-44444444-4444-4444-4444-444444444444",
        group_id: "og-22222222-2222-2222-2222-222222222222",
        name: "Bien cocido",
        price_delta: "0.00",
        input_product_id: null,
        input_qty: null,
        sort_order: 2,
        active: true,
      },
    ],
  },
];

const mockPromotions: Promotion[] = [
  {
    id: "p1111111-1111-1111-1111-111111111111",
    branch_id: null,
    name: "Arroz 5 %",
    kind: "PORCENTAJE",
    value: "5.0000",
    scope: "PRODUCTO",
    min_qty: null,
    min_subtotal: null,
    max_discount: null,
    starts_at: new Date(Date.now() - 86400000).toISOString(),
    ends_at: null,
    status: "ACTIVA",
    effective_status: "ACTIVA",
    priority: 1,
    usage_limit: null,
    usage_count: 14,
    per_customer_limit: null,
    skip_tier_lines: true,
    created_at: new Date(Date.now() - 86400000 * 5).toISOString(),
    product_ids: ["p1111111-1111-1111-1111-111111111111"],
    category_ids: [],
  },
  {
    id: "p2222222-2222-2222-2222-222222222222",
    branch_id: null,
    name: "Lleva 3 gaseosas",
    kind: "PORCENTAJE",
    value: "10.0000",
    scope: "CATEGORIA",
    min_qty: "3.0000",
    min_subtotal: null,
    max_discount: null,
    starts_at: new Date(Date.now() - 86400000).toISOString(),
    ends_at: null,
    status: "ACTIVA",
    effective_status: "ACTIVA",
    priority: 2,
    usage_limit: 100,
    usage_count: 28,
    per_customer_limit: 3,
    skip_tier_lines: true,
    created_at: new Date(Date.now() - 86400000 * 3).toISOString(),
    product_ids: [],
    category_ids: ["cat-bebidas"],
  },
  {
    id: "p3333333-3333-3333-3333-333333333333",
    branch_id: null,
    name: "Compras desde $100.000",
    kind: "FIJO",
    value: "5000.0000",
    scope: "PEDIDO",
    min_qty: null,
    min_subtotal: "100000.00",
    max_discount: "5000.00",
    starts_at: new Date(Date.now() - 86400000).toISOString(),
    ends_at: null,
    status: "ACTIVA",
    effective_status: "ACTIVA",
    priority: 0,
    usage_limit: null,
    usage_count: 5,
    per_customer_limit: 1,
    skip_tier_lines: true,
    created_at: new Date(Date.now() - 86400000 * 2).toISOString(),
    product_ids: [],
    category_ids: [],
  },
];

const mockCoupons: Coupon[] = [
  {
    id: "c1111111-1111-1111-1111-111111111111",
    branch_id: null,
    code: "BIENVENIDA10",
    name: "Bienvenida 10%",
    kind: "PORCENTAJE",
    value: "10.0000",
    scope: "PEDIDO",
    min_qty: null,
    min_subtotal: "30000.00",
    max_discount: "10000.00",
    starts_at: new Date(Date.now() - 86400000 * 30).toISOString(),
    ends_at: null,
    status: "ACTIVO",
    usage_limit: null,
    usage_count: 3,
    per_customer_limit: null,
    created_at: new Date(Date.now() - 86400000 * 30).toISOString(),
    product_ids: [],
    category_ids: [],
  },
];

let receiptSeq = 0;
const mockSales = new Map<Uuid, { body: string; response: CheckoutResponse; at: string }>();
/** Turno al que pertenece cada venta mock. */
const sessionOf = new Map<Uuid, Uuid>();
const mockLocations: LocationRow[] = [
  {
    id: "00000000-0000-4000-8000-000000000b01",
    tenant_id: "00000000-0000-4000-8000-000000000001",
    branch_id: MOCK_BRANCH_ID,
    name: "Principal",
    kind: "PISO_VENTA",
    sellable: true,
    is_default: true,
    active: true,
    created_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "00000000-0000-4000-8000-000000000b02",
    tenant_id: "00000000-0000-4000-8000-000000000001",
    branch_id: MOCK_BRANCH_ID,
    name: "Bodega de Reserva",
    kind: "BODEGA",
    sellable: true,
    is_default: false,
    active: true,
    created_at: "2026-01-01T00:00:00Z",
  },
];

const mockRegisters: CashRegister[] = [
  {
    id: "00000000-0000-4000-8000-000000000c01",
    branch_id: MOCK_BRANCH_ID,
    location_id: "00000000-0000-4000-8000-000000000b01",
    name: "Caja 1",
    code: "CAJA-1",
    default_opening: "200000.00",
    active: true,
    open_session: null,
  },
  {
    id: "00000000-0000-4000-8000-000000000c02",
    branch_id: MOCK_BRANCH_ID,
    location_id: "00000000-0000-4000-8000-000000000b01",
    name: "Caja 2",
    code: "CAJA-2",
    default_opening: "100000.00",
    active: true,
    open_session: null,
  },
];

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



/**
 * Precio por unidad vendida de una línea con tarifa (mismo orden que
 * src/pricing.rs::resolve_tier): precio fijo del producto, luego la regla de la
 * tarifa sobre precio base x unidades, y nunca negativo.
 */
function mockResolveTier(productId: Uuid, base: number, tierId: Uuid | null | undefined) {
  if (!tierId) return { unitPrice: base, units: 1, listPrice: base };
  const tier = mockTiers.find((t) => t.id === tierId && t.active);
  if (!tier) throw new ApiError(422, "tier is not valid for this line", "TIER_INVALID");
  const own = (mockProductTierPrices.get(productId) ?? []).find((p) => p.tier_id === tierId);
  let units = 1;
  if (tier.kind === "PRESENTACION") {
    const u = own?.units_per_package ?? tier.units_per_package;
    if (!u) throw new ApiError(422, "presentation has no units", "TIER_MISSING_UNITS");
    units = Number(u);
  }
  const listPrice = cents(base * units);
  if (own?.price != null) return { unitPrice: cents(Number(own.price)), units, listPrice };
  const v = Number(tier.value);
  const b = base * units;
  const raw =
    tier.adjustment === "DESCUENTO_PCT" ? b * (1 - v / 100)
    : tier.adjustment === "RECARGO_PCT" ? b * (1 + v / 100)
    : tier.adjustment === "DESCUENTO_FIJO" ? b - v
    : tier.adjustment === "RECARGO_FIJO" ? b + v
    : tier.adjustment === "PRECIO_FIJO" ? v
    : b;
  return { unitPrice: cents(Math.max(0, raw)), units, listPrice };
}

/** Reparto por peso con el resto en centavos (src/pricing.rs::allocate). */
function mockAllocate(amount: number, weights: number[]): number[] {
  const total = weights.reduce((s, w) => s + w, 0);
  if (total <= 0) return weights.map(() => 0);
  const shares = weights.map((w) => Math.floor(((amount * w) / total) * 100) / 100);
  let rest = Math.round((amount - shares.reduce((s, x) => s + x, 0)) * 100);
  const order = weights.map((w, i) => ({ w, i })).sort((a, b) => b.w - a.w || a.i - b.i);
  while (rest > 0) {
    let moved = false;
    for (const { i, w } of order) {
      if (rest <= 0) break;
      if (shares[i] + 0.01 <= w + 1e-9) {
        shares[i] = cents(shares[i] + 0.01);
        rest -= 1;
        moved = true;
      }
    }
    if (!moved) break;
  }
  return shares;
}

/**
 * Cotización del modo demo. Aplica tarifas, cambio de precio, descuentos
 * manuales y cupones con el mismo reparto y redondeo del servidor. Las
 * promociones del demo solo se muestran en su pantalla y NO afectan esta
 * cotización (F1.14).
 */
function mockQuote(req: QuoteRequest): QuoteResponse {
  const products = new Map(mockCatalog.map((p) => [p.product_id, p]));
  const customRates: Record<string, number> = { IVA19: 0.19, IVA5: 0.05, INC8: 0.08, EXENTO: 0 };

  const base = req.items.map((item) => {
    const product = item.product_id ? products.get(item.product_id) : undefined;
    const qty = Number(item.qty);
    const catalogPrice = product?.price ? Number(product.price) : 0;
    const tier = item.product_id
      ? mockResolveTier(item.product_id, catalogPrice, item.tier_id)
      : { unitPrice: Number(item.unit_price_override ?? 0), units: 1, listPrice: Number(item.unit_price_override ?? 0) };
    const override = item.product_id && item.unit_price_override != null ? cents(Number(item.unit_price_override)) : null;
    const overridden = override != null && Math.abs(override - tier.unitPrice) >= 0.01;
    const unitPrice = overridden ? (override as number) : tier.unitPrice;
    const gross = cents(qty * unitPrice);
    const manual = cents(gross * (Number(item.discount_pct ?? 0) / 100));
    const rate = item.product_id ? Number(product?.tax_rate ?? 0) : customRates[item.custom_tax ?? "EXENTO"] ?? 0;
    return {
      item,
      name: item.custom_name || product?.name || "Ítem libre",
      qty,
      units: tier.units,
      listPrice: tier.listPrice,
      unitPrice,
      gross,
      manual,
      rate,
      eligible: !overridden,
    };
  });

  const subtotalGross = cents(base.reduce((s, l) => s + l.gross, 0));
  let coupon = base.map(() => 0);
  let couponApplied: QuoteResponse["coupon"] = null;
  let couponError: QuoteResponse["coupon_error"] = null;

  if (req.coupon_code) {
    const code = req.coupon_code.trim().toUpperCase();
    const found = mockCoupons.find((c) => c.code === code);
    const weights = base.map((l) => (l.eligible ? l.gross - l.manual : 0));
    const eligibleBase = cents(weights.reduce((s, w) => s + w, 0));
    if (!found) {
      couponError = { code: "COUPON_NOT_FOUND", message: ERROR_MESSAGES.COUPON_NOT_FOUND };
    } else if (found.status === "INACTIVO") {
      couponError = { code: "COUPON_INACTIVE", message: ERROR_MESSAGES.COUPON_INACTIVE };
    } else if (eligibleBase <= 0) {
      couponError = { code: "COUPON_NOT_APPLICABLE", message: ERROR_MESSAGES.COUPON_NOT_APPLICABLE };
    } else if (found.min_subtotal && subtotalGross < Number(found.min_subtotal)) {
      couponError = { code: "COUPON_MIN_SUBTOTAL", message: ERROR_MESSAGES.COUPON_MIN_SUBTOTAL };
    } else {
      let amount =
        found.kind === "PORCENTAJE"
          ? cents(eligibleBase * (Number(found.value) / 100))
          : Math.min(Number(found.value), eligibleBase);
      if (found.max_discount) amount = Math.min(amount, Number(found.max_discount));
      coupon = mockAllocate(amount, weights);
      couponApplied = { id: found.id, code: found.code, amount: dec(amount) };
    }
  }

  let discountTotal = 0;
  let subtotal = 0;
  let taxTotal = 0;
  const lines: QuoteLine[] = base.map((l, index) => {
    const total = cents(l.manual + coupon[index]);
    const lineTotal = cents(l.gross - total);
    const taxAmount = cents(lineTotal * l.rate);
    discountTotal += total;
    subtotal += lineTotal;
    taxTotal += taxAmount;
    return {
      index,
      product_id: l.item.product_id ?? null,
      name: l.name,
      qty: l.item.qty,
      units_per_package: String(l.units),
      list_price: dec(l.listPrice),
      unit_price: dec(l.unitPrice),
      gross: dec(l.gross),
      discount: {
        manual: dec(l.manual),
        promotion: "0.00",
        order: "0.00",
        coupon: dec(coupon[index]),
        total: dec(total),
      },
      promotion_id: null,
      line_total: dec(lineTotal),
      tax_rate: l.rate.toFixed(4),
      tax_amount: dec(taxAmount),
    };
  });

  const tip = req.tip_amount ? Number(req.tip_amount) : 0;
  return {
    lines,
    subtotal_gross: dec(subtotalGross),
    discount_total: dec(discountTotal),
    subtotal: dec(subtotal),
    tax_total: dec(taxTotal),
    tip_amount: dec(tip),
    total: dec(subtotal + taxTotal + tip),
    promotions: [],
    coupon: couponApplied,
    coupon_error: couponError,
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
  const tip = body.tip_amount ? Number(body.tip_amount) : 0;
  // Propina sin impuesto, sumada después del IVA (F1.10).
  const total = cents(subtotal + tax + tip);

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
      waiter_id: body.tip_waiter_id ?? null,
      customer_id: body.customer_id ?? null,
      customer_alias: body.customer_alias ?? null,
      customer_name: cust ? cust.name : null,
      subtotal: dec(subtotal),
      inc_amount: dec(tax),
      tip_amount: dec(tip),
      refunded_amount: "0",
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

// ------------------------------------- historial, anulación y devoluciones (demo)

/** Devoluciones del modo demo, en orden de creación. */
const mockRefunds: Refund[] = [];
let refundSeq = 0;

function mockSaleOrThrow(orderId: Uuid): CheckoutResponse {
  const found = mockSales.get(orderId);
  if (!found) throw new ApiError(404, "not found");
  return found.response;
}

function mockHistory(q: SaleHistoryQuery): SaleHistoryPage {
  const today = new Date().toISOString().slice(0, 10);
  const from = q.from ?? today;
  const to = q.to ?? today;
  const digits = q.q ? q.q.split("-").pop()?.replace(/\D/g, "") ?? "" : "";
  const rows: SaleHistoryRow[] = [...mockSales.values()]
    .filter(({ at }) => at.slice(0, 10) >= from && at.slice(0, 10) <= to)
    .map(({ response, at }) => ({
      id: response.order.id,
      receipt_number: response.receipt_number,
      created_at: at,
      cashier_id: null,
      cashier_name: "Usuario demo",
      customer_label: response.order.customer_name ?? response.order.customer_alias ?? "Consumidor final",
      total: response.order.total_amount,
      refunded_amount: response.order.refunded_amount ?? "0",
      methods: [...new Set(response.payments.map((p) => p.method))].sort() as PaymentMethod[],
      status: response.order.status,
      fiscal_status: response.fiscal_status,
    }))
    .filter((r) => !q.method || r.methods.includes(q.method))
    .filter((r) =>
      !q.status ? true : q.status === "CANCELADA" ? r.status === "CANCELADA" : r.status !== "CANCELADA",
    )
    .filter((r) => !digits || Number(r.receipt_number.split("-").pop()) === Number(digits))
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
  const pageSize = Math.min(q.page_size ?? 50, 100);
  const page = Math.max(q.page ?? 1, 1);
  return {
    items: rows.slice((page - 1) * pageSize, page * pageSize),
    total_count: rows.length,
    totals: { count: rows.length, amount: dec(rows.reduce((s, r) => s + Number(r.total), 0)) },
  };
}

function mockDetail(orderId: Uuid): SaleDetail {
  const sale = mockSaleOrThrow(orderId);
  const customer = sale.order.customer_id
    ? mockCustomers.find((c) => c.id === sale.order.customer_id) ?? null
    : null;
  return {
    ...sale,
    refunds: mockRefunds.filter((r) => r.order_id === orderId),
    customer,
    cashier_name: "Usuario demo",
    register_name: "Caja 1",
    waiter_name: null,
  };
}

function voidError(why: string): ApiError {
  return new ApiError(409, "the sale cannot be voided", "VOID_NOT_ALLOWED", { why });
}

/** Mismas reglas que src/sales.rs::void_tx. */
function mockVoid(orderId: Uuid, reason: string): Order {
  const sale = mockSaleOrThrow(orderId);
  const at = mockSales.get(orderId)?.at ?? "";
  if (sale.order.status !== "COBRADA") throw voidError("STATUS");
  if (at.slice(0, 10) !== new Date().toISOString().slice(0, 10)) throw voidError("NOT_TODAY");
  if (!mockCash || sessionOf.get(orderId) !== mockCash.id) throw voidError("SESSION_CLOSED");
  if (mockRefunds.some((r) => r.order_id === orderId && r.status !== "RECHAZADA")) {
    throw voidError("HAS_REFUNDS");
  }
  if (sale.fiscal_status === "ACEPTADO") throw voidError("FISCAL_ACCEPTED");
  const trimmed = reason.trim();
  if (trimmed.length < 3 || trimmed.length > 200) {
    throw new ApiError(422, "reason must have 3 to 200 characters", "VOID_REASON_REQUIRED");
  }
  for (const item of sale.items) {
    if (item.product_id) {
      mockRecordMovement(item.product_id, "DEVOLUCION", Number(item.qty), "anulación", {
        type: "venta",
        id: orderId,
      });
    }
  }
  sale.order.status = "CANCELADA";
  sale.order.void_reason = trimmed;
  sale.fiscal_status = "ANULADO";
  return structuredClone(sale.order);
}

/** Cantidad ya devuelta (pendiente o aprobada) de una línea, y sus montos. */
function refundedOf(orderItemId: Uuid): { qty: number; total: number; tax: number } {
  let qty = 0;
  let total = 0;
  let tax = 0;
  for (const r of mockRefunds) {
    if (r.status === "RECHAZADA") continue;
    for (const it of r.items) {
      if (it.order_item_id !== orderItemId) continue;
      qty += Number(it.qty);
      total += Number(it.line_total);
      tax += Number(it.tax_amount);
    }
  }
  return { qty, total, tax };
}

function applyMockRefund(refund: Refund): void {
  const sale = mockSaleOrThrow(refund.order_id);
  const cashOut = refund.method === "EFECTIVO" || refund.original_method === "EFECTIVO";
  if (cashOut) {
    if (!mockCash) {
      throw new ApiError(409, "open a cash session to refund cash", "REFUND_NEEDS_OPEN_SESSION");
    }
    mockCashMoves.push({
      sessionId: mockCash.id,
      kind: "DEVOLUCION",
      amount: Number(refund.total),
      reason: `Devolución ${refund.number}`,
      at: new Date().toISOString(),
    });
  }
  for (const it of refund.items) {
    const line = sale.items.find((i) => i.id === it.order_item_id);
    if (it.disposition === "REINTEGRAR" && line?.product_id) {
      mockRecordMovement(line.product_id, "DEVOLUCION", Number(it.qty), "devolución", {
        type: "venta",
        id: refund.order_id,
      });
    }
  }
  sale.order.refunded_amount = dec(Number(sale.order.refunded_amount ?? 0) + Number(refund.total));
  refund.status = "APROBADA";
  refund.decided_by_name = "Usuario demo";
  refund.decided_at = new Date().toISOString();
}

/** Mismas reglas que src/refunds.rs::create_tx (el usuario demo tiene pos:refund). */
function mockCreateRefund(input: RefundInput): Refund {
  const existing = mockRefunds.find((r) => r.id === input.id);
  if (existing) return existing;
  const sale = mockSaleOrThrow(input.order_id);
  if (sale.order.status !== "COBRADA" && sale.order.status !== "CERRADA") {
    throw new ApiError(409, "order is not charged", "REFUND_ORDER_INVALID");
  }
  const reason = input.reason.trim();
  if (input.items.length === 0 || reason.length < 3 || reason.length > 200) {
    throw new ApiError(422, "invalid refund", "REFUND_INVALID");
  }
  const items: RefundItem[] = input.items.map((it) => {
    const line = sale.items.find((l) => l.id === it.order_item_id);
    if (!line) throw new ApiError(422, "item does not belong to the order", "REFUND_INVALID");
    const prev = refundedOf(it.order_item_id);
    const sold = Number(line.qty);
    const refundable = sold - prev.qty;
    const qty = Number(it.qty);
    if (qty > refundable) {
      throw new ApiError(422, "qty exceeds what is left to refund", "REFUND_QTY_EXCEEDED", {
        order_item_id: it.order_item_id,
        refundable: String(refundable),
      });
    }
    if (!line.product_id && it.disposition !== "SIN_DEVOLUCION") {
      throw new ApiError(422, "a custom item only admits SIN_DEVOLUCION", "REFUND_INVALID");
    }
    const full = qty === refundable;
    const lineTotal = full ? Number(line.line_total) - prev.total : cents((Number(line.line_total) * qty) / sold);
    const tax = full ? Number(line.tax_amount) - prev.tax : cents((Number(line.tax_amount) * qty) / sold);
    return {
      order_item_id: it.order_item_id,
      name: line.name ?? "",
      qty: it.qty,
      disposition: it.disposition,
      line_total: dec(lineTotal),
      tax_amount: dec(tax),
    };
  });
  let originalMethod: PaymentMethod | null = null;
  let reversal: string | null = null;
  if (input.method === "ORIGINAL") {
    if (sale.payments.length !== 1) {
      throw new ApiError(422, "the sale has several payments", "REFUND_ORIGINAL_MIXED");
    }
    originalMethod = sale.payments[0].method as PaymentMethod;
    if (originalMethod !== "EFECTIVO") {
      reversal = input.reversal_reference?.trim() ?? "";
      if (reversal.length < 3 || reversal.length > 60) {
        throw new ApiError(422, "reversal reference is required", "REFUND_REFERENCE_REQUIRED");
      }
    }
  }
  const subtotal = items.reduce((s, i) => s + Number(i.line_total), 0);
  const tax = items.reduce((s, i) => s + Number(i.tax_amount), 0);
  refundSeq += 1;
  const refund: Refund = {
    id: input.id,
    order_id: input.order_id,
    receipt_number: sale.receipt_number,
    number: `DEV-${String(refundSeq).padStart(6, "0")}`,
    status: "PENDIENTE",
    method: input.method,
    original_method: originalMethod,
    reversal_reference: reversal,
    reason,
    subtotal: dec(subtotal),
    tax_amount: dec(tax),
    total: dec(subtotal + tax),
    items,
    requested_by_name: "Usuario demo",
    decided_by_name: null,
    decided_at: null,
    reject_reason: null,
    created_at: new Date().toISOString(),
  };
  applyMockRefund(refund);
  mockRefunds.push(refund);
  return refund;
}

function mockPendingRefund(id: Uuid): Refund {
  const refund = mockRefunds.find((r) => r.id === id);
  if (!refund) throw new ApiError(404, "not found");
  if (refund.status !== "PENDIENTE") {
    throw new ApiError(409, "refund was already decided", "REFUND_NOT_PENDING");
  }
  return refund;
}

// --------------------------------------------------------------------- API

export const api = {
  business: {
    /** GET /v1/business */
    async get(): Promise<BusinessInfo> {
      if (USE_MOCKS) {
        await delay(50);
        const [nit, dv] = mockBusinessInfo.tax_id.replace(/\./g, "").split("-");
        return {
          name: mockBusinessInfo.commercial_name,
          business_name: mockBusinessInfo.legal_name,
          nit,
          dv: dv ?? null,
        };
      }
      return request("/v1/business");
    },
  },

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
    /** GET /v1/sales — historial (F1.13). */
    async history(q: SaleHistoryQuery): Promise<SaleHistoryPage> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockHistory(q));
      }
      const params = new URLSearchParams({ branch_id: q.branch_id });
      for (const key of ["from", "to", "cashier_id", "method", "status", "q"] as const) {
        const value = q[key];
        if (value) params.set(key, value);
      }
      if (q.page) params.set("page", String(q.page));
      if (q.page_size) params.set("page_size", String(q.page_size));
      return request(`/v1/sales?${params}`);
    },

    /** GET /v1/sales/{id} */
    async detail(orderId: Uuid): Promise<SaleDetail> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockDetail(orderId));
      }
      return request(`/v1/sales/${orderId}`);
    },

    /** POST /v1/sales/{id}/void (F1.12). */
    async void(orderId: Uuid, reason: string): Promise<Order> {
      if (USE_MOCKS) {
        await delay();
        return mockVoid(orderId, reason);
      }
      return request(`/v1/sales/${orderId}/void`, { method: "POST", body: JSON.stringify({ reason }) });
    },
  },

  refunds: {
    /** POST /v1/refunds — idempotente por `input.id`. */
    async create(input: RefundInput): Promise<Refund> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockCreateRefund(input));
      }
      return request("/v1/refunds", { method: "POST", body: JSON.stringify(input) });
    },

    async approve(id: Uuid): Promise<Refund> {
      if (USE_MOCKS) {
        await delay();
        const refund = mockPendingRefund(id);
        applyMockRefund(refund);
        return structuredClone(refund);
      }
      return request(`/v1/refunds/${id}/approve`, { method: "POST" });
    },

    async reject(id: Uuid, reason: string): Promise<Refund> {
      if (USE_MOCKS) {
        await delay();
        const refund = mockPendingRefund(id);
        refund.status = "RECHAZADA";
        refund.reject_reason = reason.trim();
        refund.decided_by_name = "Usuario demo";
        refund.decided_at = new Date().toISOString();
        return structuredClone(refund);
      }
      return request(`/v1/refunds/${id}/reject`, { method: "POST", body: JSON.stringify({ reason }) });
    },

    /** GET /v1/refunds?branch_id=&status= */
    async list(branchId: Uuid, status?: RefundStatus | null): Promise<Refund[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(
          mockRefunds.filter((r) => !status || r.status === status).slice().reverse(),
        );
      }
      const params = new URLSearchParams({ branch_id: branchId });
      if (status) params.set("status", status);
      return request(`/v1/refunds?${params}`);
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

    async create(req: { branch_id: Uuid; name: string; code: string; default_opening: number; location_id?: Uuid | null }): Promise<CashRegister> {
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
        location_id: req.location_id ?? null,
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
      req: { name: string; code: string; default_opening: number; active: boolean; location_id?: Uuid | null },
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
        location_id: req.location_id !== undefined ? req.location_id : current.location_id,
      };
      return structuredClone(mockRegisters[idx]);
    },
  },

  locations: {
    async list(branchId: Uuid): Promise<LocationRow[]> {
      if (!USE_MOCKS) {
        return request(`/v1/locations?branch_id=${branchId}`);
      }
      await delay();
      return structuredClone(
        mockLocations.filter((l) => l.branch_id === branchId || branchId === MOCK_BRANCH_ID),
      );
    },

    async create(input: CreateLocationInput): Promise<LocationRow> {
      if (!USE_MOCKS) {
        return request("/v1/locations", {
          method: "POST",
          body: JSON.stringify(input),
        });
      }
      await delay();
      const isDef = input.is_default ?? false;
      if (isDef) {
        for (const loc of mockLocations) {
          if (loc.branch_id === input.branch_id) {
            loc.is_default = false;
          }
        }
      }
      const newLoc: LocationRow = {
        id: crypto.randomUUID(),
        tenant_id: "00000000-0000-4000-8000-000000000001",
        branch_id: input.branch_id,
        name: input.name.trim(),
        kind: input.kind,
        sellable: input.sellable,
        is_default: isDef,
        active: true,
        created_at: new Date().toISOString(),
      };
      mockLocations.push(newLoc);
      return structuredClone(newLoc);
    },

    async update(id: Uuid, input: UpdateLocationInput): Promise<LocationRow> {
      if (!USE_MOCKS) {
        return request(`/v1/locations/${id}`, {
          method: "PUT",
          body: JSON.stringify(input),
        });
      }
      await delay();
      const idx = mockLocations.findIndex((l) => l.id === id);
      if (idx === -1) throw new ApiError(404, "Bodega no encontrada");
      const current = mockLocations[idx];
      const isDef = input.is_default ?? current.is_default;
      if (isDef && !current.is_default) {
        for (const loc of mockLocations) {
          if (loc.branch_id === current.branch_id) {
            loc.is_default = false;
          }
        }
      }
      mockLocations[idx] = {
        ...current,
        name: input.name !== undefined ? input.name.trim() : current.name,
        kind: input.kind ?? current.kind,
        sellable: input.sellable !== undefined ? input.sellable : current.sellable,
        is_default: isDef,
        active: input.active !== undefined ? input.active : current.active,
      };
      return structuredClone(mockLocations[idx]);
    },
  },

  inventory: {
    /** GET /v1/inventory/stock?branch_id= — existencias con estado frente al punto de reorden. */
    async stock(branchId: Uuid, locationId?: Uuid | null): Promise<StockLevel[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockStockLevels());
      }
      const loc = locationId ? `&location_id=${locationId}` : "";
      return request(`/v1/inventory/stock?branch_id=${branchId}${loc}`);
    },

    /** GET /v1/inventory/alerts — alertas abiertas de stock. */
    async alerts(branchId: Uuid): Promise<StockAlert[]> {
      if (USE_MOCKS) {
        await delay();
        return [];
      }
      return request(`/v1/inventory/alerts?branch_id=${branchId}`);
    },

    /** GET /v1/inventory/movements — kardex, mas reciente primero. */
    async movements(branchId: Uuid, productId?: Uuid | null, limit = 200, locationId?: Uuid | null): Promise<Movement[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockKardex(productId).slice(0, limit));
      }
      const product = productId ? `&product_id=${productId}` : "";
      const loc = locationId ? `&location_id=${locationId}` : "";
      return request(`/v1/inventory/movements?branch_id=${branchId}${product}${loc}&limit=${limit}`);
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

    /** POST /v1/inventory/transfer — traslado entre sucursales o entre bodegas del negocio. */
    async transfer(
      fromBranchOrParams:
        | Uuid
        | {
            fromBranchId?: Uuid;
            toBranchId?: Uuid;
            fromLocationId?: Uuid;
            toLocationId?: Uuid;
            productId: Uuid;
            qty: Decimal;
            cause: string;
          },
      toBranchId?: Uuid,
      productId?: Uuid,
      qty?: Decimal,
      cause?: string,
    ): Promise<{ transfer_id: Uuid }> {
      let body: Record<string, unknown>;
      if (typeof fromBranchOrParams === "object") {
        body = {
          from_branch_id: fromBranchOrParams.fromBranchId,
          to_branch_id: fromBranchOrParams.toBranchId,
          from_location_id: fromBranchOrParams.fromLocationId,
          to_location_id: fromBranchOrParams.toLocationId,
          product_id: fromBranchOrParams.productId,
          qty: fromBranchOrParams.qty,
          cause: fromBranchOrParams.cause,
        };
      } else {
        body = {
          from_branch_id: fromBranchOrParams,
          to_branch_id: toBranchId,
          product_id: productId,
          qty,
          cause,
        };
      }
      if (USE_MOCKS) {
        await delay();
        return { transfer_id: crypto.randomUUID() };
      }
      return request("/v1/inventory/transfer", {
        method: "POST",
        body: JSON.stringify(body),
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

  products: {
    async list(params: ProductListParams = {}): Promise<ProductListResponse> {
      if (USE_MOCKS) {
        await delay();
        const stock = mockStockLevels();
        const items: ProductRow[] = stock.map((s) => ({
          id: s.product_id,
          name: s.name,
          sku: s.sku,
          barcode: s.barcode,
          kind: s.kind ?? "FISICO",
          usage: s.usage ?? "VENTA",
          price: s.price,
          cost_price: s.cost_price,
          tax_code: "IVA19",
          on_hand: s.on_hand,
          category_name: s.category,
          has_variants: false,
          parent_id: null,
          archived: false,
        }));
        return { items, total_count: items.length };
      }
      const query = new URLSearchParams();
      if (params.branch_id) query.set("branch_id", params.branch_id);
      if (params.q) query.set("q", params.q);
      if (params.category_id) query.set("category_id", params.category_id);
      if (params.usage) query.set("usage", params.usage);
      if (params.archived !== undefined) query.set("archived", String(params.archived));
      if (params.page) query.set("page", String(params.page));
      if (params.page_size) query.set("page_size", String(params.page_size));
      const qs = query.toString();
      return request(`/v1/products${qs ? `?${qs}` : ""}`);
    },

    async get(id: Uuid, branchId?: Uuid): Promise<ProductDetail> {
      if (USE_MOCKS) {
        await delay();
        const stock = mockStockLevels().find((s) => s.product_id === id);
        return {
          id,
          name: stock?.name ?? "Producto Demo",
          sku: stock?.sku ?? null,
          barcode: stock?.barcode ?? null,
          kind: stock?.kind ?? "FISICO",
          usage: stock?.usage ?? "VENTA",
          price: stock?.price ?? "10000.00",
          cost_price: stock?.cost_price ?? "5000.00",
          tax_code: "IVA19",
          on_hand: stock?.on_hand ?? "10.0000",
          category_name: stock?.category ?? null,
          has_variants: false,
          parent_id: null,
          archived: false,
          unit_measure_code: "UND",
          purchase_uom_code: null,
          purchase_factor: "1.0000",
          reorder_qty: stock?.min_qty ?? null,
          max_qty: null,
          track_lots: false,
          allow_price_override: true,
          description: null,
          variants: [],
          tier_prices: [],
          option_groups: [],
          stock_by_location: [
            {
              location_id: branchId ?? MOCK_BRANCH_ID,
              location_name: "Principal",
              on_hand: stock?.on_hand ?? "10.0000",
              reserved: "0.0000",
              min_qty: stock?.min_qty ?? null,
              avg_cost: stock?.cost_price ?? "5000.00",
            },
          ],
        };
      }
      const qs = branchId ? `?branch_id=${branchId}` : "";
      return request(`/v1/products/${id}${qs}`);
    },

    async create(input: ProductInput): Promise<ProductDetail> {
      if (USE_MOCKS) {
        await delay();
        const id = crypto.randomUUID();
        return {
          id,
          name: input.name,
          sku: input.sku ?? null,
          barcode: input.barcode ?? null,
          kind: input.kind ?? "FISICO",
          usage: input.usage ?? "VENTA",
          price: input.base_price ?? null,
          cost_price: input.cost_price ?? null,
          tax_code: input.tax_code,
          on_hand: "0.0000",
          category_name: null,
          has_variants: input.has_variants ?? false,
          parent_id: null,
          archived: false,
          unit_measure_code: input.unit_measure_code,
          purchase_uom_code: input.purchase_uom_code ?? null,
          purchase_factor: input.purchase_factor ?? "1.0000",
          reorder_qty: input.reorder_qty ?? null,
          max_qty: input.max_qty ?? null,
          track_lots: input.track_lots ?? false,
          allow_price_override: input.allow_price_override ?? true,
          description: input.description ?? null,
          variants: [],
          tier_prices: [],
          option_groups: [],
          stock_by_location: [],
        };
      }
      return request(`/v1/products`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    async update(id: Uuid, input: ProductInput): Promise<ProductDetail> {
      if (USE_MOCKS) {
        await delay();
        return this.get(id);
      }
      return request(`/v1/products/${id}`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },

    async createVariant(id: Uuid, input: CreateVariantInput): Promise<ProductDetail> {
      if (USE_MOCKS) {
        await delay();
        return this.get(id);
      }
      return request(`/v1/products/${id}/variants`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    async updateBranchProduct(productId: Uuid, input: UpdateBranchProductInput): Promise<{ status: string; id: Uuid }> {
      if (USE_MOCKS) {
        await delay();
        return { status: "ok", id: crypto.randomUUID() };
      }
      return request(`/v1/branch-products/${productId}`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },

    async archive(id: Uuid, confirm = false): Promise<{ status: string }> {
      if (USE_MOCKS) {
        await delay();
        return { status: "ok" };
      }
      return request(`/v1/products/${id}/archive`, {
        method: "POST",
        body: JSON.stringify({ confirm }),
      });
    },

    async unarchive(id: Uuid): Promise<{ status: string }> {
      if (USE_MOCKS) {
        await delay();
        return { status: "ok" };
      }
      return request(`/v1/products/${id}/unarchive`, {
        method: "POST",
      });
    },

    async generateBarcode(id: Uuid): Promise<{ barcode: string }> {
      if (USE_MOCKS) {
        await delay();
        const barcode = `2000000000015`;
        return { barcode };
      }
      return request<{ barcode: string }>(`/v1/products/${id}/generate-barcode`, {
        method: "POST",
      });
    },

    async importAnalyze(branchId: Uuid, file: File): Promise<AnalyzeResponse> {
      if (USE_MOCKS) {
        await delay(300);
        const text = await file.text();
        const lines = text.split("\n").filter((l) => l.trim().length > 0);
        const hasErrors = text.includes("7.70123E+12");
        const rows: ImportRowResult[] = lines.slice(1).map((line, idx) => ({
          line: idx + 2,
          status: line.includes("7.70123E+12") ? "ERROR" : "OK",
          action: "CREAR",
          messages: line.includes("7.70123E+12") ? ["código en notación científica"] : [],
          data: {
            sku: `MOCK-${idx + 1}`,
            codigo_barras: null,
            nombre: `Producto importado ${idx + 1}`,
            categoria: "General",
            unidad: "UND",
            impuesto: "IVA19",
            costo: "5000.00",
            precio: "10000.00",
            cantidad_inicial: "10.0000",
            sku_padre: null,
            atributos: {},
          },
        }));
        return {
          session_id: crypto.randomUUID(),
          summary: {
            ok: hasErrors ? rows.length - 1 : rows.length,
            warnings: 0,
            errors: hasErrors ? 1 : 0,
            create: rows.length,
            update: 0,
          },
          rows,
        };
      }

      const formData = new FormData();
      formData.append("branch_id", branchId);
      formData.append("file", file);

      return request<AnalyzeResponse>("/v1/products/import/analyze", {
        method: "POST",
        body: formData,
      });
    },

    async importCommit(sessionId: Uuid, offset: number, limit: number): Promise<CommitResponse> {
      if (USE_MOCKS) {
        await delay(200);
        return {
          committed_rows: offset + limit,
          done: true,
        };
      }
      return request<CommitResponse>("/v1/products/import/commit", {
        method: "POST",
        body: JSON.stringify({ session_id: sessionId, offset, limit }),
      });
    },
  },

  options: {
    async listGroups(): Promise<OptionGroupDetail[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockOptionGroups);
      }
      return request("/v1/option-groups");
    },

    async createGroup(input: CreateOptionGroupInput): Promise<OptionGroupDetail> {
      if (USE_MOCKS) {
        await delay();
        const id = crypto.randomUUID();
        const group: OptionGroupDetail = {
          id,
          name: input.name,
          min_select: input.min_select ?? 0,
          max_select: input.max_select ?? 1,
          active: input.active ?? true,
          options: (input.options ?? []).map((o, idx) => ({
            id: o.id ?? crypto.randomUUID(),
            group_id: id,
            name: o.name,
            price_delta: o.price_delta ?? "0",
            input_product_id: o.input_product_id ?? null,
            input_qty: o.input_qty ?? null,
            sort_order: o.sort_order ?? idx + 1,
            active: o.active ?? true,
          })),
        };
        mockOptionGroups.push(group);
        return group;
      }
      return request("/v1/option-groups", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    async updateGroup(id: Uuid, input: CreateOptionGroupInput): Promise<OptionGroupDetail> {
      if (USE_MOCKS) {
        await delay();
        const idx = mockOptionGroups.findIndex((g) => g.id === id);
        const group: OptionGroupDetail = {
          id,
          name: input.name,
          min_select: input.min_select ?? 0,
          max_select: input.max_select ?? 1,
          active: input.active ?? true,
          options: (input.options ?? []).map((o, optIdx) => ({
            id: o.id ?? crypto.randomUUID(),
            group_id: id,
            name: o.name,
            price_delta: o.price_delta ?? "0",
            input_product_id: o.input_product_id ?? null,
            input_qty: o.input_qty ?? null,
            sort_order: o.sort_order ?? optIdx + 1,
            active: o.active ?? true,
          })),
        };
        if (idx >= 0) mockOptionGroups[idx] = group;
        else mockOptionGroups.push(group);
        return group;
      }
      return request(`/v1/option-groups/${id}`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },

    async setProductOptionGroups(productId: Uuid, groups: UpdateProductOptionGroupInput[]): Promise<{ status: string }> {
      if (USE_MOCKS) {
        await delay();
        return { status: "ok" };
      }
      return request(`/v1/products/${productId}/option-groups`, {
        method: "PUT",
        body: JSON.stringify(groups),
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

    /** GET /v1/suppliers/{id} */
    async get(id: Uuid): Promise<SupplierDetailResponse> {
      if (USE_MOCKS) {
        await delay();
        return mockSupplierGet(id);
      }
      return request(`/v1/suppliers/${id}`);
    },

    /** POST /v1/suppliers/{id}/archive */
    async archive(id: Uuid): Promise<void> {
      if (USE_MOCKS) {
        await delay();
        return mockSupplierArchive(id);
      }
      return request(`/v1/suppliers/${id}/archive`, { method: "POST" });
    },

    /** PUT /v1/suppliers/{id}/products */
    async setProducts(id: Uuid, items: SupplierProductItemInput[]): Promise<SupplierProductDetail[]> {
      if (USE_MOCKS) {
        await delay();
        return mockSupplierSetProducts(id, items);
      }
      return request(`/v1/suppliers/${id}/products`, { method: "PUT", body: JSON.stringify(items) });
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

    /** PUT /v1/purchases/{id} */
    async update(id: Uuid, body: UpdatePurchaseInput): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseUpdate(id, body);
      }
      return request(`/v1/purchases/${id}`, { method: "PUT", body: JSON.stringify(body) });
    },

    /** POST /v1/purchases/{id}/approve */
    async approve(id: Uuid): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseApprove(id);
      }
      return request(`/v1/purchases/${id}/approve`, { method: "POST" });
    },

    /** POST /v1/purchases/{id}/receive */
    async receive(id: Uuid, body?: ReceivePurchaseRequest): Promise<Purchase> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchaseReceive(id, body);
      }
      return request(`/v1/purchases/${id}/receive`, {
        method: "POST",
        body: body ? JSON.stringify(body) : undefined,
      });
    },

    /** POST /v1/purchases/cost-preview */
    async costPreview(body: CostPreviewRequest): Promise<CostPreviewResponse> {
      if (USE_MOCKS) {
        await delay();
        return mockPurchasesCostPreview(body);
      }
      return request("/v1/purchases/cost-preview", {
        method: "POST",
        body: JSON.stringify(body),
      });
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

  payables: {
    /** GET /v1/payables?branch_id=&status=&supplier_id=&overdue= */
    async list(filter?: PayablesFilter): Promise<Payable[]> {
      if (USE_MOCKS) {
        await delay();
        return mockPayablesList(filter);
      }
      const params = new URLSearchParams();
      if (filter?.branch_id) params.set("branch_id", filter.branch_id);
      if (filter?.status) params.set("status", filter.status);
      if (filter?.supplier_id) params.set("supplier_id", filter.supplier_id);
      if (filter?.overdue !== undefined) params.set("overdue", String(filter.overdue));
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/payables${qs}`);
    },

    /** GET /v1/payables/aging?branch_id= */
    async aging(branchId?: Uuid): Promise<PayablesAging> {
      if (USE_MOCKS) {
        await delay();
        return mockPayablesAging(branchId);
      }
      const qs = branchId ? `?branch_id=${branchId}` : "";
      return request(`/v1/payables/aging${qs}`);
    },

    /** GET /v1/payables/{id} */
    async get(id: Uuid): Promise<PayableDetail> {
      if (USE_MOCKS) {
        await delay();
        return mockPayableGet(id);
      }
      return request(`/v1/payables/${id}`);
    },

    /** POST /v1/payables/{id}/payments */
    async createPayment(id: Uuid, input: CreatePayablePaymentInput): Promise<PayableDetail> {
      if (USE_MOCKS) {
        await delay();
        return mockPayablePayment(id, input);
      }
      return request(`/v1/payables/${id}/payments`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    /** POST /v1/payables/{id}/write-off */
    async writeOff(id: Uuid, input: WriteOffPayableInput): Promise<PayableDetail> {
      if (USE_MOCKS) {
        await delay();
        return mockPayableWriteOff(id, input);
      }
      return request(`/v1/payables/${id}/write-off`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },
  },

  expenseCategories: {
    /** GET /v1/expense-categories?include_inactive= */
    async list(includeInactive?: boolean): Promise<ExpenseCategory[]> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseCategoriesList(includeInactive);
      }
      const qs = includeInactive ? "?include_inactive=true" : "";
      return request(`/v1/expense-categories${qs}`);
    },

    /** POST /v1/expense-categories */
    async create(input: CreateExpenseCategoryInput): Promise<ExpenseCategory> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseCategoryCreate(input);
      }
      return request("/v1/expense-categories", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    /** PUT /v1/expense-categories/{id} */
    async update(id: Uuid, input: UpdateExpenseCategoryInput): Promise<ExpenseCategory> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseCategoryUpdate(id, input);
      }
      return request(`/v1/expense-categories/${id}`, {
        method: "PUT",
        body: JSON.stringify(input),
      });
    },
  },

  expenses: {
    /** GET /v1/expenses */
    async list(filter?: ExpensesFilter): Promise<Expense[]> {
      if (USE_MOCKS) {
        await delay();
        return mockExpensesList(filter);
      }
      const params = new URLSearchParams();
      if (filter?.branch_id) params.set("branch_id", filter.branch_id);
      if (filter?.category_id) params.set("category_id", filter.category_id);
      if (filter?.supplier_id) params.set("supplier_id", filter.supplier_id);
      if (filter?.status) params.set("status", filter.status);
      if (filter?.from) params.set("from", filter.from);
      if (filter?.to) params.set("to", filter.to);
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/expenses${qs}`);
    },

    /** GET /v1/expenses/summary */
    async summary(filter?: ExpenseSummaryFilter): Promise<ExpenseSummary> {
      if (USE_MOCKS) {
        await delay();
        return mockExpensesSummary(filter?.branch_id, filter?.month);
      }
      const params = new URLSearchParams();
      if (filter?.branch_id) params.set("branch_id", filter.branch_id);
      if (filter?.month) params.set("month", filter.month);
      const qs = params.toString() ? `?${params.toString()}` : "";
      return request(`/v1/expenses/summary${qs}`);
    },

    /** GET /v1/expenses/{id} */
    async get(id: Uuid): Promise<Expense> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseGet(id);
      }
      return request(`/v1/expenses/${id}`);
    },

    /** POST /v1/expenses */
    async create(input: CreateExpenseInput): Promise<Expense> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseCreate(input);
      }
      return request("/v1/expenses", {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    /** POST /v1/expenses/{id}/approve */
    async approve(id: Uuid): Promise<Expense> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseApprove(id);
      }
      return request(`/v1/expenses/${id}/approve`, {
        method: "POST",
      });
    },

    /** POST /v1/expenses/{id}/reject */
    async reject(id: Uuid, input: RejectExpenseInput): Promise<Expense> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseReject(id, input);
      }
      return request(`/v1/expenses/${id}/reject`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },

    /** POST /v1/expenses/{id}/cancel */
    async cancel(id: Uuid): Promise<Expense> {
      if (USE_MOCKS) {
        await delay();
        return mockExpenseCancel(id);
      }
      return request(`/v1/expenses/${id}/cancel`, {
        method: "POST",
      });
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

  promotions: {
    async list(status?: string): Promise<Promotion[]> {
      if (USE_MOCKS) {
        await delay();
        if (status) return mockPromotions.filter((p) => p.effective_status === status);
        return structuredClone(mockPromotions);
      }
      const q = status ? `?status=${encodeURIComponent(status)}` : "";
      return request(`/v1/promotions${q}`);
    },
    async create(input: CreatePromotionInput): Promise<Promotion> {
      if (!USE_MOCKS) {
        return request("/v1/promotions", { method: "POST", body: JSON.stringify(input) });
      }
      await delay();
      const p: Promotion = {
        id: crypto.randomUUID(),
        branch_id: input.branch_id ?? null,
        name: input.name,
        kind: input.kind,
        value: String(input.value),
        scope: input.scope,
        min_qty: input.min_qty ? String(input.min_qty) : null,
        min_subtotal: input.min_subtotal ? String(input.min_subtotal) : null,
        max_discount: input.max_discount ? String(input.max_discount) : null,
        starts_at: input.starts_at,
        ends_at: input.ends_at ?? null,
        status: "BORRADOR",
        effective_status: "BORRADOR",
        priority: input.priority ?? 0,
        usage_limit: input.usage_limit ?? null,
        usage_count: 0,
        per_customer_limit: input.per_customer_limit ?? null,
        skip_tier_lines: input.skip_tier_lines ?? true,
        created_at: new Date().toISOString(),
        product_ids: input.product_ids ?? [],
        category_ids: input.category_ids ?? [],
      };
      mockPromotions.unshift(p);
      return p;
    },
    async update(id: string, input: UpdatePromotionInput): Promise<Promotion> {
      if (!USE_MOCKS) {
        return request(`/v1/promotions/${id}`, { method: "PUT", body: JSON.stringify(input) });
      }
      await delay();
      const idx = mockPromotions.findIndex((p) => p.id === id);
      if (idx === -1) throw new ApiError(404, "Promoción no encontrada");
      const current = mockPromotions[idx];
      const updated: Promotion = {
        ...current,
        branch_id: input.branch_id ?? null,
        name: input.name,
        kind: input.kind,
        value: String(input.value),
        scope: input.scope,
        min_qty: input.min_qty ? String(input.min_qty) : null,
        min_subtotal: input.min_subtotal ? String(input.min_subtotal) : null,
        max_discount: input.max_discount ? String(input.max_discount) : null,
        starts_at: input.starts_at,
        ends_at: input.ends_at ?? null,
        priority: input.priority ?? current.priority,
        usage_limit: input.usage_limit ?? null,
        per_customer_limit: input.per_customer_limit ?? null,
        skip_tier_lines: input.skip_tier_lines ?? current.skip_tier_lines,
        product_ids: input.product_ids ?? current.product_ids,
        category_ids: input.category_ids ?? current.category_ids,
      };
      mockPromotions[idx] = updated;
      return updated;
    },
    async activate(id: string): Promise<void> {
      if (!USE_MOCKS) {
        return request(`/v1/promotions/${id}/activate`, { method: "POST" });
      }
      await delay();
      const p = mockPromotions.find((item) => item.id === id);
      if (p) {
        p.status = "ACTIVA";
        p.effective_status = "ACTIVA";
      }
    },
    async pause(id: string): Promise<void> {
      if (!USE_MOCKS) {
        return request(`/v1/promotions/${id}/pause`, { method: "POST" });
      }
      await delay();
      const p = mockPromotions.find((item) => item.id === id);
      if (p) {
        p.status = "PAUSADA";
        p.effective_status = "PAUSADA";
      }
    },
  },

  coupons: {
    async list(): Promise<Coupon[]> {
      if (USE_MOCKS) {
        await delay();
        return structuredClone(mockCoupons);
      }
      return request("/v1/coupons");
    },
    async create(input: CreateCouponInput): Promise<Coupon> {
      if (!USE_MOCKS) {
        return request("/v1/coupons", { method: "POST", body: JSON.stringify(input) });
      }
      await delay();
      const code = input.code.trim().toUpperCase();
      if (mockCoupons.some((c) => c.code === code)) {
        throw new ApiError(409, "Ya existe un cupón con ese código", "COUPON_CODE_TAKEN");
      }
      const c: Coupon = {
        id: crypto.randomUUID(),
        branch_id: input.branch_id ?? null,
        code,
        name: input.name,
        kind: input.kind,
        value: String(input.value),
        scope: input.scope,
        min_qty: input.min_qty ? String(input.min_qty) : null,
        min_subtotal: input.min_subtotal ? String(input.min_subtotal) : null,
        max_discount: input.max_discount ? String(input.max_discount) : null,
        starts_at: input.starts_at,
        ends_at: input.ends_at ?? null,
        status: "ACTIVO",
        usage_limit: input.usage_limit ?? null,
        usage_count: 0,
        per_customer_limit: input.per_customer_limit ?? null,
        created_at: new Date().toISOString(),
        product_ids: input.product_ids ?? [],
        category_ids: input.category_ids ?? [],
      };
      mockCoupons.unshift(c);
      return c;
    },
    async update(id: string, input: UpdateCouponInput): Promise<Coupon> {
      if (!USE_MOCKS) {
        return request(`/v1/coupons/${id}`, { method: "PUT", body: JSON.stringify(input) });
      }
      await delay();
      const idx = mockCoupons.findIndex((c) => c.id === id);
      if (idx === -1) throw new ApiError(404, "Cupón no encontrado");
      const code = input.code.trim().toUpperCase();
      if (mockCoupons.some((c) => c.id !== id && c.code === code)) {
        throw new ApiError(409, "Ya existe un cupón con ese código", "COUPON_CODE_TAKEN");
      }
      const current = mockCoupons[idx];
      const updated: Coupon = {
        ...current,
        branch_id: input.branch_id ?? null,
        code,
        name: input.name,
        kind: input.kind,
        value: String(input.value),
        scope: input.scope,
        min_qty: input.min_qty ? String(input.min_qty) : null,
        min_subtotal: input.min_subtotal ? String(input.min_subtotal) : null,
        max_discount: input.max_discount ? String(input.max_discount) : null,
        starts_at: input.starts_at,
        ends_at: input.ends_at ?? null,
        usage_limit: input.usage_limit ?? null,
        per_customer_limit: input.per_customer_limit ?? null,
        product_ids: input.product_ids ?? current.product_ids,
        category_ids: input.category_ids ?? current.category_ids,
      };
      mockCoupons[idx] = updated;
      return updated;
    },
    async toggle(id: string): Promise<Coupon> {
      if (!USE_MOCKS) {
        return request(`/v1/coupons/${id}/toggle`, { method: "POST" });
      }
      await delay();
      const c = mockCoupons.find((item) => item.id === id);
      if (!c) throw new ApiError(404, "Cupón no encontrado");
      c.status = c.status === "ACTIVO" ? "INACTIVO" : "ACTIVO";
      return structuredClone(c);
    },
  },

  users: {
    /** Meseros activos de la sucursal, para la propina (pos:sell). */
    async waiters(branchId: Uuid): Promise<Waiter[]> {
      if (USE_MOCKS) {
        await delay();
        return mockWaiters();
      }
      return request(`/v1/waiters?branch_id=${branchId}`);
    },

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
      if (filter?.entity) params.set("entity", filter.entity);
      if (filter?.entity_id) params.set("entity_id", filter.entity_id);
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
  BARCODE_EXISTS: "El producto ya tiene un código de barras asignado.",
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
  PRODUCT_INVALID: "Revisá los datos del producto.",
  PRODUCT_NOT_SELLABLE: "Este producto es un insumo o de uso interno: no se vende.",
  PRODUCT_IS_VARIANT_PARENT: "Elegí una variante del producto.",
  PRODUCT_UOM_LOCKED: "No se puede cambiar la unidad: el producto ya tiene movimientos.",
  VARIANTS_AND_PRESENTATIONS: "Un producto no puede tener variantes y presentaciones a la vez.",
  SKU_TAKEN: "Ese SKU ya está en uso.",
  BARCODE_SCIENTIFIC: "El código de barras está en notación científica. Revisá el archivo.",
  OPTION_INVALID: "Revisá las opciones elegidas.",
  OPTION_SELECTION_INVALID: "Falta elegir opciones obligatorias.",
  PRODUCT_HAS_RESERVATIONS: "El producto tiene reservas activas.",
  ARCHIVE_NEEDS_CONFIRM: "El producto tiene stock. Confirmá la baja.",
  IMPORT_HAS_ERRORS: "Corregí los errores del archivo antes de importar.",
  IMPORT_EXPIRED: "La importación venció. Subí el archivo otra vez.",
  LOCATION_INVALID: "Revisá los datos de la bodega.",
  LOCATION_HAS_STOCK: "La bodega tiene stock. Trasladalo antes de desactivarla.",
  STOCK_DELTA_ZERO: "La cantidad no puede ser cero.",
  STOCK_INSUFFICIENT: "No hay stock suficiente.",
  NIT_DV_INVALID: "El dígito de verificación no corresponde al NIT.",
  SUPPLIER_HAS_OPEN_DOCS: "El proveedor tiene órdenes o deudas abiertas.",
  PURCHASE_INVALID: "Revisá la orden de compra.",
  PURCHASE_INVALID_STATE: "La orden no admite esa acción en su estado actual.",
  RECEIVE_EXCEEDS_PENDING: "No podés recibir más de lo pendiente.",
  LOT_REQUIRED: "Este producto exige lote y vencimiento.",
  LOT_EXPIRY_MISMATCH: "Ese lote ya existe con otra fecha de vencimiento.",
  LOT_ONLY_EXPIRED: "Solo quedan lotes vencidos de este producto.",
  PAYABLE_CLOSED: "La cuenta ya está pagada o castigada.",
  PAYMENT_EXCEEDS_BALANCE: "El pago supera el saldo.",
  PAYMENT_REFERENCE_REQUIRED: "Escribí la referencia del pago.",
  PAYMENT_NEEDS_OPEN_SESSION: "Abrí un turno de caja para pagar en efectivo.",
  EXPENSE_INVALID_STATE: "El gasto no admite esa acción en su estado actual.",
  ADJUST_INVALID: "Revisá el motivo y la cantidad del ajuste.",
  ADJUST_NOT_PENDING: "El ajuste ya fue decidido.",
};

export const COUPON_ERROR_MESSAGES: Record<string, string> = {
  COUPON_NOT_FOUND: ERROR_MESSAGES.COUPON_NOT_FOUND,
  COUPON_INACTIVE: ERROR_MESSAGES.COUPON_INACTIVE,
  COUPON_NOT_STARTED: ERROR_MESSAGES.COUPON_NOT_STARTED,
  COUPON_EXPIRED: ERROR_MESSAGES.COUPON_EXPIRED,
  COUPON_EXHAUSTED: ERROR_MESSAGES.COUPON_EXHAUSTED,
  COUPON_REQUIRES_CUSTOMER: ERROR_MESSAGES.COUPON_REQUIRES_CUSTOMER,
  COUPON_CUSTOMER_LIMIT: ERROR_MESSAGES.COUPON_CUSTOMER_LIMIT,
  COUPON_MIN_SUBTOTAL: ERROR_MESSAGES.COUPON_MIN_SUBTOTAL,
  COUPON_NOT_APPLICABLE: ERROR_MESSAGES.COUPON_NOT_APPLICABLE,
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

// Tipos "de cable": la forma exacta del JSON que devuelve nexa-backend.
//
// - `Decimal` de rust_decimal se serializa como string ("3500.00").
// - `Uuid` llega como string.
// - Los codigos (status, sale_type, tax_code) son `tima_codigo` de
//   tbl_tipos_maestra (ver supabase/migrations/20260924000100_core_tenancy.sql).
//
// Los tipos marcados "PROPUESTO" aun no existen en el backend: son el contrato
// que el frontend necesita y que el backend debe implementar.

export type Decimal = string;
export type Uuid = string;

export type TaxCode = "IVA19" | "IVA5" | "INC8";

/** GET /v1/catalog?branch_id= — src/catalog.rs::CatalogItem */
export type CatalogItem = {
  product_id: Uuid;
  name: string;
  category: string | null;
  barcode?: string | null;
  price: Decimal | null;
  tax_code: TaxCode | null;
  /** Fraccion: 0.19 para IVA 19%. `null` = exento. */
  tax_rate: Decimal | null;
  /** Unidades disponibles (on_hand - reserved); `null` si no aplica. */
  available: Decimal | null;
  tier_barcodes?: TierBarcode[];
};

export type OrderStatus =
  | "CREADA"
  | "EN_PREPARACION"
  | "LISTA"
  | "ENTREGADA"
  | "COBRADA"
  | "CERRADA"
  | "CANCELADA";

export type SaleType = "MESA" | "MOSTRADOR" | "DOMICILIO" | "MAYORISTA";

/** src/orders.rs::Order */
export type Order = {
  id: Uuid;
  sale_type: SaleType;
  status: OrderStatus;
  table_id: Uuid | null;
  branch_id: Uuid;
  waiter_id: Uuid | null;
  customer_id?: Uuid | null;
  customer_alias?: string | null;
  customer_name?: string | null;
  subtotal: Decimal;
  discount_total?: Decimal;
  inc_amount: Decimal;
  tip_amount: Decimal;
  total_amount: Decimal;
  /** F1.11: total devuelto de la venta. */
  refunded_amount?: Decimal;
  /** F1.12: motivo de anulación. */
  void_reason?: string | null;
};

/** src/orders.rs::OrderItem */
export type OrderItem = {
  id: Uuid;
  product_id?: Uuid | null;
  custom_name?: string | null;
  name?: string;
  tier_id?: Uuid | null;
  units_per_package?: Decimal;
  qty: Decimal;
  unit_price: Decimal;
  list_price?: Decimal;
  discount_manual?: Decimal;
  discount_promotion?: Decimal;
  discount_order?: Decimal;
  discount_coupon?: Decimal;
  promotion_id?: Uuid | null;
  price_override_reason?: string | null;
  line_discount?: Decimal;
  line_total: Decimal;
  /** Tasa congelada al agregar la linea: "0.19" = 19%. "0" = exento. */
  tax_rate: Decimal;
  tax_amount: Decimal;
  station_id: number | null;
  status_id: number | null;
};

export type Role =
  | "DUENO"
  | "ADMIN_CAJA"
  | "CAJERO"
  | "MESERO"
  | "COCINA"
  | "SUPERVISOR"
  | "BODEGA"
  | "REPARTIDOR";

/** GET /v1/context — alcance resuelto por membresia (src/routes.rs::context). */
export type SessionContext = {
  uid: Uuid | null;
  tenant: Uuid | null;
  /** `null` = membresia de todo el negocio sin sucursal fija. */
  branch: Uuid | null;
  role: Role;
  permissions?: string[];
};

/** GET /v1/branches — src/tenancy.rs::BranchAccess */
export type BranchAccess = {
  id: Uuid;
  name: string;
  code: string | null;
  /** Rol del usuario en esta sucursal. */
  role: Role;
};

// ------------------------------------------------------ venta de mostrador

export type PaymentMethod = "EFECTIVO" | "DATAFONO" | "QR";

/**
 * POST /v1/sales — src/sales.rs::SaleRequest.
 *
 * `id` lo genera el cliente y es la clave de idempotencia: un reintento con
 * el mismo id y cuerpo devuelve la misma venta (200) sin cobrar dos veces.
 */
export type QuoteItemInput = {
  product_id?: Uuid | null;
  custom_name?: string | null;
  qty: Decimal;
  unit_price_override?: Decimal | null;
  discount_pct?: Decimal | null;
  tier_id?: Uuid | null;
  price_override_reason?: string | null;
  /** Impuesto del ítem libre (F1.9). */
  custom_tax?: CustomTax | null;
};

export type CustomTax = "IVA19" | "IVA5" | "INC8" | "EXENTO";

export type QuoteRequest = {
  branch_id: Uuid;
  items: QuoteItemInput[];
  tip_amount?: Decimal | null;
  customer_id?: Uuid | null;
  customer_alias?: string | null;
  coupon_code?: string | null;
};

export type LineDiscountBreakdown = {
  manual: Decimal;
  promotion: Decimal;
  order: Decimal;
  coupon: Decimal;
  total: Decimal;
};

export type QuoteLine = {
  index: number;
  product_id?: Uuid | null;
  name: string;
  qty: Decimal;
  units_per_package: Decimal;
  list_price: Decimal;
  unit_price: Decimal;
  gross: Decimal;
  discount: LineDiscountBreakdown;
  promotion_id?: Uuid | null;
  line_total: Decimal;
  tax_rate: Decimal;
  tax_amount: Decimal;
};

export type PromoApplied = {
  id: Uuid;
  name: string;
  amount: Decimal;
};

export type CouponApplied = {
  id: Uuid;
  code: string;
  amount: Decimal;
};

export type CouponError = {
  code: string;
  message: string;
};

export type QuoteResponse = {
  lines: QuoteLine[];
  subtotal_gross: Decimal;
  discount_total: Decimal;
  subtotal: Decimal;
  tax_total: Decimal;
  tip_amount: Decimal;
  total: Decimal;
  promotions: PromoApplied[];
  coupon?: CouponApplied | null;
  coupon_error?: CouponError | null;
};

export type CheckoutRequest = {
  id: Uuid;
  branch_id: Uuid;
  items: QuoteItemInput[];
  /**
   * En EFECTIVO `amount` es lo que entrega el cliente (el vuelto lo calcula
   * el servidor). En los demas metodos, el monto cobrado. Maximo un EFECTIVO.
   */
  payments: { method: PaymentMethod; amount: Decimal; reference?: string | null }[];
  quoted_total: Decimal;
  tip_amount?: Decimal | null;
  /** F1.10: mesero que recibe la propina. */
  tip_waiter_id?: Uuid | null;
  customer_id?: Uuid | null;
  customer_alias?: string | null;
  coupon_code?: string | null;
};

/** src/sales.rs::Payment */
export type Payment = {
  id: Uuid;
  method: PaymentMethod;
  /** Monto aplicado a la orden. */
  amount: Decimal;
  /** Solo EFECTIVO: lo recibido del cliente. */
  tendered: Decimal | null;
  reference: string | null;
};

export type FiscalStatus = "PENDIENTE" | "ENVIADO" | "ACEPTADO" | "RECHAZADO" | "ANULADO";

/** Respuesta de POST /v1/sales (201 nueva, 200 reintento). */
export type CheckoutResponse = {
  order: Order;
  items: OrderItem[];
  payments: Payment[];
  /** Consecutivo del comprobante (POS-000123), sin huecos. */
  receipt_number: string;
  change_due: Decimal;
  /** Documento equivalente electronico DIAN; PENDIENTE hasta que se transmita. */
  fiscal_status: FiscalStatus;
};

// ------------------------------------------------------------------- caja

/** src/registers.rs::Register */
export type CashRegister = {
  id: Uuid;
  branch_id: Uuid;
  name: string;
  code: string;
  default_opening: Decimal;
  active: boolean;
  open_session: null | {
    id: Uuid;
    opened_by: Uuid;
    opened_by_name: string | null;
    opened_at: string;
  };
};

/** src/cash.rs::CashSession */
export type CashSession = {
  id: Uuid;
  branch_id: Uuid;
  register_id: Uuid;
  register_name: string;
  register_code: string;
  opened_by: Uuid;
  opening_amount: Decimal;
  opened_at: string;
  closed_by: Uuid | null;
  closed_at: string | null;
  expected_cash: Decimal | null;
  counted_cash: Decimal | null;
  /** counted - expected (negativo = faltante). */
  difference: Decimal | null;
  notes: string | null;
};

export type CashMovementKind =
  | "INGRESO"
  | "ABONO_CARTERA"
  | "ABONO_SEPARADO"
  | "RECARGA_SALDO"
  | "RETIRO"
  | "DEVOLUCION"
  | "PROPINA_PAGADA"
  | "PAGO_PROVEEDOR"
  | "GASTO";

export type CashMovementReasonCode = "CONSIGNACION" | "GASTO_MENOR" | "PRESTAMO" | "OTRO";

export type MovementByKind = {
  kind: CashMovementKind;
  code: string;
  label: string;
  amount: Decimal;
  count: number;
  is_inflow: boolean;
};

/** GET /v1/cash-sessions/current y POST /v1/cash-sessions/{id}/close */
export type CashSummary = {
  session: CashSession;
  sales_count: number;
  sales_total: Decimal;
  by_method: { method: PaymentMethod; amount: Decimal; count: number }[];
  movements_by_kind: MovementByKind[];
  /** Ingresos de efectivo que no son ventas. */
  cash_in: Decimal;
  /** Retiros de efectivo de la gaveta. */
  cash_out: Decimal;
  /** Base + efectivo cobrado + ingresos - retiros. */
  expected_cash: Decimal;
  tips_total: Decimal;
  tips_paid: Decimal;
  tips_pending: Decimal;
  state_token: string;
};

/** GET /v1/cash-sessions/{id}/activity — src/cash.rs::ActivityResponse */
export type CashActivityReport = {
  state_token: string;
  items: CashActivity[];
};

/** Fila individual de bitacora del turno — src/cash.rs::ActivityEntry */
export type CashActivity = {
  at: string;
  kind: "APERTURA" | "VENTA" | CashMovementKind | "CIERRE";
  /** Numero de comprobante, motivo del movimiento o nota del cierre. */
  detail: string | null;
  /** Metodos de pago de la venta: "EFECTIVO", "DATAFONO+QR"... */
  methods: string | null;
  amount: Decimal;
  user_name: string | null;
};

/** GET /v1/cash-sessions?branch_id= — src/cash.rs::SessionRow */
export type CashSessionRow = {
  id: Uuid;
  opened_by: Uuid;
  opened_by_name: string | null;
  opening_amount: Decimal;
  opened_at: string;
  closed_at: string | null;
  expected_cash: Decimal | null;
  counted_cash: Decimal | null;
  difference: Decimal | null;
  sales_count: number;
  sales_total: Decimal;
};

// ------------------------------------------------------------- inventario

export type MovementType =
  | "REPOSICION"
  | "DESCUENTO"
  | "AJUSTE_ENTRADA"
  | "AJUSTE_SALIDA"
  | "TRASLADO_ENTRADA"
  | "TRASLADO_SALIDA"
  | "DEVOLUCION"
  | "AJUSTE";

/** GET /v1/inventory/movements (kardex) — src/inventory.rs::Movement */
export type Movement = {
  id: Uuid;
  product_id: Uuid;
  product_name: string;
  sku: string | null;
  movement_type: MovementType;
  qty: Decimal;
  /** Efecto con signo sobre el stock; null en ajustes heredados sin direccion. */
  delta: Decimal | null;
  /** Existencia despues del movimiento. */
  balance_after: Decimal | null;
  cause: string | null;
  /** "venta" (ref_id = orden) o "TRASLADO" (ref_id compartido por las dos patas). */
  ref_type: string | null;
  ref_id: Uuid | null;
  user_name: string | null;
  branch_id: Uuid;
  created_at: string;
};

export type StockStatus = "AGOTADO" | "BAJO" | "OK";

export type ProductKind = "FISICO" | "SERVICIO" | "PREPARADO";
export type ProductUsage = "VENTA" | "INSUMO" | "INTERNO";

export type ProductRow = {
  id: Uuid;
  name: string;
  sku: string | null;
  barcode: string | null;
  kind: ProductKind;
  usage: ProductUsage;
  price: Decimal | null;
  cost_price: Decimal | null;
  tax_code: string;
  on_hand: Decimal;
  category_name: string | null;
  has_variants: boolean;
  parent_id: Uuid | null;
  archived: boolean;
};

export type LocationStock = {
  location_id: Uuid;
  location_name: string;
  on_hand: Decimal;
  reserved: Decimal;
  min_qty: Decimal | null;
  avg_cost: Decimal;
};

export type ProductDetail = {
  id: Uuid;
  name: string;
  sku: string | null;
  barcode: string | null;
  kind: ProductKind;
  usage: ProductUsage;
  price: Decimal | null;
  cost_price: Decimal | null;
  tax_code: string;
  on_hand: Decimal;
  category_name: string | null;
  has_variants: boolean;
  parent_id: Uuid | null;
  archived: boolean;
  unit_measure_code: string;
  purchase_uom_code: string | null;
  purchase_factor: Decimal;
  reorder_qty: Decimal | null;
  max_qty: Decimal | null;
  track_lots: boolean;
  allow_price_override: boolean;
  description: string | null;
  variants: ProductRow[];
  tier_prices: ProductTierPrice[];
  option_groups: unknown[];
  stock_by_location: LocationStock[];
};

export type ProductInput = {
  name: string;
  sku?: string | null;
  barcode?: string | null;
  kind?: ProductKind;
  usage?: ProductUsage;
  unit_measure_code: string;
  purchase_uom_code?: string | null;
  purchase_factor?: Decimal;
  tax_code: string;
  cost_price?: Decimal | null;
  base_price?: Decimal | null;
  reorder_qty?: Decimal | null;
  max_qty?: Decimal | null;
  track_lots?: boolean;
  allow_price_override?: boolean;
  has_variants?: boolean;
  description?: string | null;
};

export type CreateVariantInput = {
  sku?: string | null;
  barcode?: string | null;
  attrs: Record<string, string>;
  price: Decimal;
  cost_price?: Decimal | null;
};

export type UpdateBranchProductInput = {
  branch_id: Uuid;
  price: Decimal;
  category_id?: Uuid | null;
  is_available?: boolean;
};

export type ProductListParams = {
  branch_id?: Uuid;
  q?: string;
  category_id?: Uuid;
  usage?: string;
  archived?: boolean;
  page?: number;
  page_size?: number;
};

export type ProductListResponse = {
  items: ProductRow[];
  total_count: number;
};

/** GET /v1/inventory/stock?branch_id= — src/inventory.rs::StockLevel */
export type StockLevel = {
  product_id: Uuid;
  name: string;
  sku: string | null;
  barcode: string | null;
  category: string | null;
  unit: string | null;
  kind?: ProductKind;
  usage?: ProductUsage;
  /** Precio de venta en la sucursal, sin impuesto. */
  price: Decimal | null;
  cost_price: Decimal | null;
  on_hand: Decimal;
  reserved: Decimal;
  available: Decimal;
  /** Punto de reorden. */
  min_qty: Decimal;
  status: StockStatus;
  updated_at: string | null;
};

// ------------------------------------------------------------- compras y proveedores

export type PurchaseStatus = "PENDIENTE" | "RECIBIDA" | "ANULADA";

/** src/procurement.rs::Supplier */
export type Supplier = {
  id: Uuid;
  name: string;
  tax_id: string | null;
  contact_name: string | null;
  phone: string | null;
  email: string | null;
  address: string | null;
  notes: string | null;
  active: boolean;
  created_at: string;
};

export type CreateSupplierInput = {
  name: string;
  tax_id?: string | null;
  contact_name?: string | null;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  notes?: string | null;
};

export type UpdateSupplierInput = {
  name: string;
  tax_id?: string | null;
  contact_name?: string | null;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  notes?: string | null;
  active?: boolean;
};

/** src/procurement.rs::PurchaseItemDetail */
export type PurchaseItem = {
  id: Uuid;
  product_id: Uuid;
  product_name: string;
  sku: string | null;
  qty: Decimal;
  unit_cost: Decimal;
  line_total: Decimal;
  tax_code: TaxCode | null;
  tax_rate: Decimal | null;
  notes?: string | null;
};

/** src/procurement.rs::PurchaseRow / PurchaseDetail */
export type Purchase = {
  id: Uuid;
  branch_id: Uuid;
  supplier_id: Uuid;
  supplier_name: string;
  document_number: string | null;
  supplier_invoice_date: string | null;
  status: PurchaseStatus;
  subtotal: Decimal;
  tax_amount: Decimal;
  total_amount: Decimal;
  items_count: number;
  received_at: string | null;
  notes: string | null;
  created_at: string;
  items?: PurchaseItem[];
};

export type CreatePurchaseInput = {
  id: Uuid;
  branch_id: Uuid;
  supplier_id: Uuid;
  document_number?: string | null;
  supplier_invoice_date?: string | null;
  notes?: string | null;
  items: {
    product_id: Uuid;
    qty: Decimal;
    unit_cost: Decimal;
  }[];
};

export type PurchaseFilter = {
  branch_id?: Uuid;
  from?: string;
  to?: string;
  supplier_id?: Uuid;
  status?: PurchaseStatus;
  limit?: number;
};

/** src/procurement.rs::PurchaseSummary */
export type PurchaseSummary = {
  total_purchased_month: Decimal;
  active_suppliers: number;
};

// ---------------------------------------------------------------- PLATAFORMA (Fase 0)

export type Permission = {
  code: string;
  module: string;
  description: string;
};

export type RolePermissionItem = {
  code: string;
  module: string;
  description: string;
  allowed: boolean;
  is_override: boolean;
};

export type RolePermissionsResponse = {
  role: string;
  permissions: RolePermissionItem[];
};

export type UpdateRolePermissionsInput = {
  overrides: {
    permission_code: string;
    allowed: boolean;
  }[];
};

export type PosSettings = {
  allow_price_override: boolean;
  max_discount_pct: number;
  refund_requires_approval: boolean;
  allow_anonymous_sales?: boolean;
  auto_print?: boolean;
};

export type CashSettings = {
  require_session: boolean;
  close_tolerance: Decimal;
  blind_close?: boolean;
};

export type InventorySettings = {
  allow_negative_stock: boolean;
  track_raw_materials: boolean;
  low_stock_default?: number;
};

export type TipsSettings = {
  enabled?: boolean;
  suggested_pct: number[];
  distribute_evenly: boolean;
};

export type PrintSettings = {
  ticket_width_mm: number;
  header_text: string | null;
  footer_text: string | null;
};

export type BranchSettings = {
  pos: PosSettings;
  cash: CashSettings;
  inventory: InventorySettings;
  tips: TipsSettings;
  print: PrintSettings;
};

export type AuditEntry = {
  id: string;
  tenant_id: Uuid;
  branch_id: Uuid | null;
  user_id: Uuid | null;
  action: string;
  entity: string;
  entity_id: string | null;
  details: Record<string, unknown>;
  ip_address: string | null;
  created_at: string;
};

export type AuditFilter = {
  branch_id?: Uuid;
  user_id?: Uuid;
  action?: string;
  from?: string;
  to?: string;
  limit?: number;
};

export type TenantUser = {
  id: Uuid;
  email: string | null;
  full_name: string | null;
  role: Role;
  branches: {
    id: Uuid;
    name: string;
  }[];
  active: boolean;
  created_at: string;
};

export type Invitation = {
  id: Uuid;
  email: string;
  role: Role;
  branch_ids: Uuid[];
  status: "PENDIENTE" | "ACEPTADA" | "CANCELADA";
  created_at: string;
};

export type InviteUserInput = {
  email: string;
  role: Role;
  branch_ids: Uuid[];
};

export type UpdateUserRoleInput = {
  role: Role;
  branch_id?: Uuid | null;
};

export type UpdateUserStatusInput = {
  active: boolean;
};

export type LoginAttemptInput = {
  email: string;
  success?: boolean;
  failure_reason?: string;
};

// ---------------------------------------------------------------- clientes

export type DocType = "CC" | "NIT" | "CE" | "PP" | "TI" | "PEP" | "PPT" | "NIUP";

export type Customer = {
  id: Uuid;
  doc_type: DocType | null;
  doc_number: string | null;
  dv: string | null;
  name: string;
  phone: string | null;
  email: string | null;
  price_tier_id: Uuid | null;
};

export type CreateCustomerInput = {
  name: string;
  doc_type?: DocType | null;
  doc_number?: string | null;
  dv?: string | null;
  phone?: string | null;
  email?: string | null;
  price_tier_id?: Uuid | null;
};

export type CustomerQuery = {
  q?: string;
  limit?: number;
};

// ---------------------------------------------------------------- PROPUESTOS

/** PROPUESTO — GET /v1/reports/dashboard?branch_id=&period=today */
export type DashboardSummary = {
  revenue: Decimal;
  revenue_ex_tax: Decimal;
  revenue_change_pct: Decimal;
  profit: Decimal;
  profit_margin_pct: Decimal;
  profit_change_pct: Decimal;
  balance: Decimal;
  expenses: Decimal;
  hourly: { hour: number; revenue: Decimal; orders: number }[];
  channels: { channel: "POS" | "WHATSAPP" | "TIENDA_ONLINE"; amount: Decimal }[];
  alerts: { low_stock: number; out_of_stock: number; pending_dispatch: number };
};

/** Cuerpo de error de ApiError (src/error.rs). */
export type ApiErrorBody = {
  error: string;
  code?: string;
  data?: unknown;
};

export type TierKind = "CLIENTE" | "PRESENTACION";
export type TierAdjustment =
  | "NINGUNO"
  | "DESCUENTO_PCT"
  | "RECARGO_PCT"
  | "DESCUENTO_FIJO"
  | "RECARGO_FIJO"
  | "PRECIO_FIJO";

export type PriceTier = {
  id: Uuid;
  tenant_id: Uuid;
  name: string;
  kind: TierKind;
  adjustment: TierAdjustment;
  value: Decimal;
  units_per_package?: Decimal | null;
  active: boolean;
  created_at: string;
};

export type CreatePriceTierInput = {
  name: string;
  kind: TierKind;
  adjustment: TierAdjustment;
  value: Decimal;
  units_per_package?: Decimal | null;
};

export type UpdatePriceTierInput = {
  name: string;
  kind: TierKind;
  adjustment: TierAdjustment;
  value: Decimal;
  units_per_package?: Decimal | null;
  active: boolean;
};

export type ProductTierPrice = {
  tier_id: Uuid;
  price?: Decimal | null;
  units_per_package?: Decimal | null;
  barcode?: string | null;
};

export type SetProductTierPriceInput = {
  tier_id: Uuid;
  price?: Decimal | null;
  units_per_package?: Decimal | null;
  barcode?: string | null;
};

export type TierBarcode = {
  tier_id: Uuid;
  barcode: string;
};

// ----------------------------------------------------------------- promociones

export type PromotionEffectiveStatus = "BORRADOR" | "ACTIVA" | "PROGRAMADA" | "PAUSADA" | "VENCIDA";
export type PromotionScope = "PEDIDO" | "PRODUCTO" | "CATEGORIA";
export type PromotionKind = "PORCENTAJE" | "FIJO";

export type Promotion = {
  id: Uuid;
  branch_id: Uuid | null;
  name: string;
  kind: PromotionKind;
  value: Decimal;
  scope: PromotionScope;
  min_qty: Decimal | null;
  min_subtotal: Decimal | null;
  max_discount: Decimal | null;
  starts_at: string;
  ends_at: string | null;
  status: "BORRADOR" | "ACTIVA" | "PAUSADA";
  effective_status: PromotionEffectiveStatus;
  priority: number;
  usage_limit: number | null;
  usage_count: number;
  per_customer_limit: number | null;
  skip_tier_lines: boolean;
  created_at: string;
  product_ids: Uuid[];
  category_ids: Uuid[];
};

export type CreatePromotionInput = {
  branch_id?: Uuid | null;
  name: string;
  kind: PromotionKind;
  value: Decimal | number;
  scope: PromotionScope;
  min_qty?: Decimal | number | null;
  min_subtotal?: Decimal | number | null;
  max_discount?: Decimal | number | null;
  starts_at: string;
  ends_at?: string | null;
  priority?: number;
  usage_limit?: number | null;
  per_customer_limit?: number | null;
  skip_tier_lines?: boolean;
  product_ids?: Uuid[];
  category_ids?: Uuid[];
};

export type UpdatePromotionInput = CreatePromotionInput;

// ---------------------------------------------------------------- CUPONES (F1.8)

export type CouponStatus = "ACTIVO" | "INACTIVO";
export type CouponKind = "PORCENTAJE" | "FIJO";
export type CouponScope = "PEDIDO" | "PRODUCTO" | "CATEGORIA";

export type Coupon = {
  id: Uuid;
  branch_id: Uuid | null;
  code: string;
  name: string;
  kind: CouponKind;
  value: Decimal;
  scope: CouponScope;
  min_qty: Decimal | null;
  min_subtotal: Decimal | null;
  max_discount: Decimal | null;
  starts_at: string;
  ends_at: string | null;
  status: CouponStatus;
  usage_limit: number | null;
  usage_count: number;
  per_customer_limit: number | null;
  created_at: string;
  product_ids: Uuid[];
  category_ids: Uuid[];
};

export type CreateCouponInput = {
  branch_id?: Uuid | null;
  code: string;
  name: string;
  kind: CouponKind;
  value: Decimal | number;
  scope: CouponScope;
  min_qty?: Decimal | number | null;
  min_subtotal?: Decimal | number | null;
  max_discount?: Decimal | number | null;
  starts_at: string;
  ends_at?: string | null;
  usage_limit?: number | null;
  per_customer_limit?: number | null;
  product_ids?: Uuid[];
  category_ids?: Uuid[];
};

export type UpdateCouponInput = CreateCouponInput;


/** GET /v1/waiters (F1.10). */
export type Waiter = { user_id: Uuid; full_name: string };

// ------------------------------------------------- devoluciones (F1.11)

export type RefundDisposition = "REINTEGRAR" | "MERMA" | "SIN_DEVOLUCION";
export type RefundMethod = "EFECTIVO" | "ORIGINAL";
export type RefundStatus = "PENDIENTE" | "APROBADA" | "RECHAZADA";

export type RefundItem = {
  order_item_id: Uuid;
  name: string;
  qty: Decimal;
  disposition: RefundDisposition;
  line_total: Decimal;
  tax_amount: Decimal;
};

/** src/refunds.rs::Refund */
export type Refund = {
  id: Uuid;
  order_id: Uuid;
  receipt_number: string | null;
  number: string;
  status: RefundStatus;
  method: RefundMethod;
  original_method: PaymentMethod | null;
  reversal_reference: string | null;
  reason: string;
  subtotal: Decimal;
  tax_amount: Decimal;
  total: Decimal;
  items: RefundItem[];
  requested_by_name: string | null;
  decided_by_name: string | null;
  decided_at: string | null;
  reject_reason: string | null;
  created_at: string;
};

/** POST /v1/refunds. `id` lo genera el cliente (idempotencia). */
export type RefundInput = {
  id: Uuid;
  order_id: Uuid;
  method: RefundMethod;
  reason: string;
  reversal_reference?: string | null;
  items: { order_item_id: Uuid; qty: Decimal; disposition: RefundDisposition }[];
};

// ------------------------------------------- historial de ventas (F1.13)

export type SaleHistoryQuery = {
  branch_id: Uuid;
  from?: string | null;
  to?: string | null;
  cashier_id?: Uuid | null;
  method?: PaymentMethod | null;
  status?: "COBRADA" | "CANCELADA" | null;
  q?: string | null;
  page?: number;
  page_size?: number;
};

export type SaleHistoryRow = {
  id: Uuid;
  receipt_number: string;
  created_at: string;
  cashier_id: Uuid | null;
  cashier_name: string | null;
  customer_label: string;
  total: Decimal;
  refunded_amount: Decimal;
  methods: PaymentMethod[];
  status: OrderStatus;
  fiscal_status: FiscalStatus | null;
};

export type SaleHistoryPage = {
  items: SaleHistoryRow[];
  total_count: number;
  totals: { count: number; amount: Decimal };
};

/** GET /v1/sales/{id} */
export type SaleDetail = CheckoutResponse & {
  refunds: Refund[];
  customer: Customer | null;
  cashier_name: string | null;
  register_name: string | null;
  waiter_name: string | null;
};

/** GET /v1/business — datos del negocio para la tirilla (F1.15). */
export type BusinessInfo = {
  name: string;
  business_name: string | null;
  nit: string | null;
  dv: string | null;
};

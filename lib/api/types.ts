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
  price: Decimal | null;
  tax_code: TaxCode | null;
  /** Fraccion: 0.19 para IVA 19%. `null` = exento. */
  tax_rate: Decimal | null;
  /** Unidades disponibles (on_hand - reserved); `null` si no aplica. */
  available: Decimal | null;
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
  subtotal: Decimal;
  inc_amount: Decimal;
  tip_amount: Decimal;
  total_amount: Decimal;
};

/** src/orders.rs::OrderItem */
export type OrderItem = {
  id: Uuid;
  product_id: Uuid;
  qty: Decimal;
  unit_price: Decimal;
  line_total: Decimal;
  /** Tasa congelada al agregar la linea: "0.19" = 19%. "0" = exento. */
  tax_rate: Decimal;
  tax_amount: Decimal;
  station_id: number | null;
  status_id: number | null;
};

export type Role = "DUENO" | "ADMIN_CAJA" | "CAJERO" | "MESERO" | "COCINA";

/** GET /v1/context — alcance resuelto por membresia (src/routes.rs::context). */
export type SessionContext = {
  uid: Uuid | null;
  tenant: Uuid | null;
  /** `null` = membresia de todo el negocio sin sucursal fija. */
  branch: Uuid | null;
  role: Role;
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
export type CheckoutRequest = {
  id: Uuid;
  branch_id: Uuid;
  items: { product_id: Uuid; qty: Decimal }[];
  /**
   * En EFECTIVO `amount` es lo que entrega el cliente (el vuelto lo calcula
   * el servidor). En los demas metodos, el monto cobrado. Maximo un EFECTIVO.
   */
  payments: { method: PaymentMethod; amount: Decimal; reference?: string | null }[];
  tip_amount?: Decimal | null;
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

export type FiscalStatus = "PENDIENTE" | "ENVIADO" | "ACEPTADO" | "RECHAZADO";

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

/** src/cash.rs::CashSession */
export type CashSession = {
  id: Uuid;
  branch_id: Uuid;
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

/** GET /v1/cash-sessions/current y POST /v1/cash-sessions/{id}/close */
export type CashSummary = {
  session: CashSession;
  sales_count: number;
  sales_total: Decimal;
  by_method: { method: PaymentMethod; amount: Decimal; count: number }[];
  /** Ingresos de efectivo que no son ventas. */
  cash_in: Decimal;
  /** Retiros de efectivo de la gaveta. */
  cash_out: Decimal;
  /** Base + efectivo cobrado + ingresos - retiros. */
  expected_cash: Decimal;
};

export type CashMovementKind = "INGRESO" | "RETIRO";

/** GET /v1/cash-sessions/{id}/activity — src/cash.rs::ActivityEntry */
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

/** GET /v1/inventory/stock?branch_id= — src/inventory.rs::StockLevel */
export type StockLevel = {
  product_id: Uuid;
  name: string;
  sku: string | null;
  barcode: string | null;
  category: string | null;
  unit: string | null;
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
export type ApiErrorBody = { error: string };

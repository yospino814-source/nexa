// Proveedores y compras simuladas para el modo demo.
//
// Recibir una compra suma stock simulado y anota en el kardex mediante
// mockRecordMovement de mock-inventory.ts.

import { ApiError } from "./client";
import { MOCK_BRANCH_ID, mockCatalog } from "./mock";
import { mockRecordMovement } from "./mock-inventory";
import type {
  CostPreviewLine,
  CostPreviewRequest,
  CostPreviewResponse,
  CreateExpenseCategoryInput,
  CreateExpenseInput,
  CreateFromSuggestionsInput,
  CreatePurchaseInput,
  CreatePayablePaymentInput,
  CreateSupplierInput,
  Decimal,
  Expense,
  ExpenseCategory,
  ExpenseSummary,
  ExpensesFilter,
  Payable,
  PayableDetail,
  PayablePayment,
  PayablesAging,
  PayablesFilter,
  Purchase,
  PurchaseFilter,
  PurchaseItem,
  PurchaseStatus,
  PurchaseSuggestionGroup,
  PurchaseSummary,
  ReceivePurchaseRequest,
  RejectExpenseInput,
  Supplier,
  SupplierDetailResponse,
  SupplierProductDetail,
  SupplierProductItemInput,
  TaxCode,
  UpdateExpenseCategoryInput,
  UpdatePurchaseInput,
  UpdateSupplierInput,
  Uuid,
  WriteOffPayableInput,
} from "./types";

const cents = (n: number) => Math.round(n * 100) / 100;
const dec = (n: number): Decimal => cents(n).toFixed(2);

// ------------------------------------------------------------ proveedores

export const mockSuppliers: Supplier[] = [
  {
    id: "20000000-0000-4000-8000-000000000001",
    name: "Distribuidora Los Andes S.A.S.",
    tax_id: "900.123.456-1",
    contact_name: "Carlos Mendoza",
    phone: "3104567890",
    email: "pedidos@losandes.co",
    address: "Av. Boyacá # 72-10, Bogotá",
    notes: "Distribuidor mayorista de granos y abarrotes. Despacho los martes y jueves.",
    active: true,
    created_at: new Date(Date.now() - 60 * 24 * 3600_000).toISOString(),
  },
  {
    id: "20000000-0000-4000-8000-000000000002",
    name: "Bebidas y Refrescos de Colombia S.A.",
    tax_id: "890.900.120-3",
    contact_name: "Mariana Silva",
    phone: "3156789012",
    email: "ventas@bebidascol.com",
    address: "Zona Industrial Cazucá, Soacha",
    notes: "Línea completa de gaseosas, cervezas y aguas.",
    active: true,
    created_at: new Date(Date.now() - 55 * 24 * 3600_000).toISOString(),
  },
  {
    id: "20000000-0000-4000-8000-000000000003",
    name: "Lácteos del Campo Ltda.",
    tax_id: "860.012.345-6",
    contact_name: "Jorge Henao",
    phone: "3001234567",
    email: "contacto@lacteosdelcampo.com",
    address: "Km 4 Vía Chía - Cota",
    notes: "Productos lácteos frescos. Entrega refrigerada interdiaria.",
    active: true,
    created_at: new Date(Date.now() - 50 * 24 * 3600_000).toISOString(),
  },
  {
    id: "20000000-0000-4000-8000-000000000004",
    name: "Aseo y Limpieza La Candelaria",
    tax_id: "901.456.789-2",
    contact_name: "Patricia Castro",
    phone: "3209876543",
    email: "atencion@aseocandelaria.co",
    address: "Calle 19 # 28-40, Medellín",
    notes: "Insumos de aseo, jabones y detergentes institucionales.",
    active: true,
    created_at: new Date(Date.now() - 40 * 24 * 3600_000).toISOString(),
  },
  {
    id: "20000000-0000-4000-8000-000000000005",
    name: "Molinos y Granos del Valle",
    tax_id: "800.234.567-8",
    contact_name: "Gustavo Restrepo",
    phone: "3123456789",
    email: "ordenes@granosdelvalle.com",
    address: "Carrera 50 # 12-30, Cali",
    notes: "Arroz, harina, café y legumbres seleccionadas.",
    active: true,
    created_at: new Date(Date.now() - 35 * 24 * 3600_000).toISOString(),
  },
  {
    id: "20000000-0000-4000-8000-000000000006",
    name: "Snacks y Pasabocas La Granja",
    tax_id: "900.789.012-4",
    contact_name: "Lucía Ortiz",
    phone: "3187654321",
    email: "pedidos@lagranjasnacks.com",
    address: "Bodega 4, Parque Industrial Siberia",
    notes: "Papas fritas, platanitos y confitería.",
    active: true,
    created_at: new Date(Date.now() - 30 * 24 * 3600_000).toISOString(),
  },
];

// ------------------------------------------------------------ compras iniciales

function buildMockPurchases(): Purchase[] {
  const day = 24 * 3600_000;
  const p = (idx: number) => mockCatalog[idx];

  type Seed = {
    daysAgo: number;
    supplierIdx: number;
    doc: string;
    invDaysAgo: number;
    status: PurchaseStatus;
    items: { catIdx: number; qty: number; unitCost: number }[];
    notes?: string;
  };

  const seeds: Seed[] = [
    {
      daysAgo: 2,
      supplierIdx: 1, // Bebidas
      doc: "FAC-8821",
      invDaysAgo: 3,
      status: "PENDIENTE",
      items: [
        { catIdx: 0, qty: 48, unitCost: 2150 }, // Coca-Cola
        { catIdx: 7, qty: 24, unitCost: 1900 }, // Cerveza
      ],
      notes: "Pedido de reposición para el fin de semana.",
    },
    {
      daysAgo: 4,
      supplierIdx: 0, // Los Andes
      doc: "FAC-10492",
      invDaysAgo: 5,
      status: "PENDIENTE",
      items: [
        { catIdx: 1, qty: 50, unitCost: 1980 }, // Arroz Diana
        { catIdx: 4, qty: 15, unitCost: 12000 }, // Huevos
      ],
      notes: "Verificar fecha de vencimiento de los huevos al recibir.",
    },
    {
      daysAgo: 6,
      supplierIdx: 2, // Lácteos
      doc: "FAC-0492",
      invDaysAgo: 6,
      status: "RECIBIDA",
      items: [
        { catIdx: 2, qty: 30, unitCost: 2950 }, // Leche
        { catIdx: 8, qty: 24, unitCost: 1750 }, // Yogur
      ],
    },
    {
      daysAgo: 10,
      supplierIdx: 3, // Aseo
      doc: "FAC-3108",
      invDaysAgo: 11,
      status: "RECIBIDA",
      items: [
        { catIdx: 5, qty: 40, unitCost: 1450 }, // Jabón
        { catIdx: 11, qty: 20, unitCost: 9200 }, // Detergente
      ],
    },
    {
      daysAgo: 15,
      supplierIdx: 5, // Snacks
      doc: "FAC-5520",
      invDaysAgo: 16,
      status: "RECIBIDA",
      items: [
        { catIdx: 9, qty: 60, unitCost: 1350 }, // Papas
        { catIdx: 10, qty: 50, unitCost: 920 }, // Chocolatina
      ],
    },
    {
      daysAgo: 20,
      supplierIdx: 4, // Granos Valle
      doc: "FAC-7731",
      invDaysAgo: 21,
      status: "RECIBIDA",
      items: [
        { catIdx: 6, qty: 30, unitCost: 6300 }, // Café
        { catIdx: 1, qty: 40, unitCost: 1950 }, // Arroz
      ],
    },
    {
      daysAgo: 26,
      supplierIdx: 1, // Bebidas
      doc: "FAC-8610",
      invDaysAgo: 27,
      status: "RECIBIDA",
      items: [
        { catIdx: 0, qty: 72, unitCost: 2100 },
        { catIdx: 7, qty: 48, unitCost: 1850 },
      ],
    },
    {
      daysAgo: 32,
      supplierIdx: 0, // Los Andes
      doc: "FAC-10115",
      invDaysAgo: 33,
      status: "RECIBIDA",
      items: [
        { catIdx: 1, qty: 60, unitCost: 1950 },
        { catIdx: 4, qty: 20, unitCost: 11800 },
      ],
    },
    {
      daysAgo: 38,
      supplierIdx: 2, // Lácteos
      doc: "FAC-0411",
      invDaysAgo: 40,
      status: "ANULADA",
      items: [
        { catIdx: 2, qty: 20, unitCost: 2900 },
        { catIdx: 8, qty: 20, unitCost: 1700 },
      ],
      notes: "Anulada por error en la facturación del proveedor; se emitió nueva factura.",
    },
  ];

  return seeds.map((s, i) => {
    const created = new Date(Date.now() - s.daysAgo * day);
    const invoiceDate = new Date(Date.now() - s.invDaysAgo * day).toISOString().split("T")[0];
    const supplier = mockSuppliers[s.supplierIdx];
    const purchaseId = `30000000-0000-4000-8000-${String(i + 1).padStart(12, "0")}`;

    let subtotalNum = 0;
    let taxNum = 0;

    const items: PurchaseItem[] = s.items.map((it, itemIdx) => {
      const prod = p(it.catIdx);
      const lineSubtotal = it.qty * it.unitCost;
      const rate = prod.tax_code === "IVA19" ? 0.19 : 0;
      const lineTax = lineSubtotal * rate;

      subtotalNum += lineSubtotal;
      taxNum += lineTax;

      return {
        id: `40000000-0000-4000-8000-${String(i * 10 + itemIdx + 1).padStart(12, "0")}`,
        product_id: prod.product_id,
        product_name: prod.name,
        sku: `SKU-${prod.product_id.slice(-4)}`,
        qty: it.qty.toFixed(4),
        unit_cost: dec(it.unitCost),
        line_total: dec(lineSubtotal),
        tax_code: prod.tax_code as TaxCode | null,
        tax_rate: rate > 0 ? "0.19" : null,
        notes: null,
      };
    });

    const totalNum = subtotalNum + taxNum;

    return {
      id: purchaseId,
      branch_id: MOCK_BRANCH_ID,
      supplier_id: supplier.id,
      supplier_name: supplier.name,
      document_number: s.doc,
      supplier_invoice_date: invoiceDate,
      status: s.status,
      subtotal: dec(subtotalNum),
      tax_amount: dec(taxNum),
      total_amount: dec(totalNum),
      items_count: items.length,
      received_at: s.status === "RECIBIDA" ? created.toISOString() : null,
      notes: s.notes ?? null,
      created_at: created.toISOString(),
      items,
    };
  });
}

export const mockPurchases: Purchase[] = buildMockPurchases();

// ------------------------------------------------------------ funciones mock

export async function mockSuppliersList(q?: string, includeInactive = false): Promise<Supplier[]> {
  const query = q?.trim().toLowerCase();
  return mockSuppliers
    .filter((s) => {
      if (!includeInactive && !s.active) return false;
      if (!query) return true;
      return (
        s.name.toLowerCase().includes(query) ||
        (s.tax_id?.toLowerCase().includes(query) ?? false) ||
        (s.contact_name?.toLowerCase().includes(query) ?? false)
      );
    })
    .sort((a, b) => a.name.localeCompare(b.name, "es"));
}

export async function mockSupplierCreate(input: CreateSupplierInput): Promise<Supplier> {
  const name = input.name.trim();
  if (!name) throw new ApiError(422, "validation: name must not be empty");

  const taxId = input.tax_id?.trim() || null;
  if (taxId && mockSuppliers.some((s) => s.tax_id === taxId)) {
    throw new ApiError(409, "conflict: duplicated record");
  }

  const supplier: Supplier = {
    id: crypto.randomUUID(),
    name,
    tax_id: taxId,
    contact_name: input.contact_name?.trim() || null,
    phone: input.phone?.trim() || null,
    email: input.email?.trim() || null,
    address: input.address?.trim() || null,
    notes: input.notes?.trim() || null,
    active: true,
    created_at: new Date().toISOString(),
    doc_type: input.doc_type ?? "NIT",
    dv: input.dv?.trim() || null,
    person_type: input.person_type ?? "JURIDICA",
    iva_responsible: input.iva_responsible ?? true,
    self_withholding: input.self_withholding ?? false,
    payment_terms: input.payment_terms ?? "CONTADO",
    credit_days: input.credit_days ?? 0,
    delivery_days: input.delivery_days ?? null,
    category: input.category ?? "MERCANCIA",
    bank_name: input.bank_name?.trim() || null,
    bank_account_type: input.bank_account_type ?? null,
    bank_account_number: input.bank_account_number?.trim() || null,
  };

  mockSuppliers.unshift(supplier);
  return structuredClone(supplier);
}

export async function mockSupplierUpdate(id: Uuid, input: UpdateSupplierInput): Promise<Supplier> {
  const supplier = mockSuppliers.find((s) => s.id === id);
  if (!supplier) throw new ApiError(404, "not found");

  const name = input.name.trim();
  if (!name) throw new ApiError(422, "validation: name must not be empty");

  const taxId = input.tax_id?.trim() || null;
  if (taxId && mockSuppliers.some((s) => s.id !== id && s.tax_id === taxId)) {
    throw new ApiError(409, "conflict: duplicated record");
  }

  supplier.name = name;
  supplier.tax_id = taxId;
  supplier.contact_name = input.contact_name?.trim() || null;
  supplier.phone = input.phone?.trim() || null;
  supplier.email = input.email?.trim() || null;
  supplier.address = input.address?.trim() || null;
  supplier.notes = input.notes?.trim() || null;
  if (input.active !== undefined) {
    supplier.active = input.active;
  }
  if (input.doc_type) supplier.doc_type = input.doc_type;
  if (input.dv !== undefined) supplier.dv = input.dv?.trim() || null;
  if (input.person_type) supplier.person_type = input.person_type;
  if (input.iva_responsible !== undefined) supplier.iva_responsible = input.iva_responsible;
  if (input.self_withholding !== undefined) supplier.self_withholding = input.self_withholding;
  if (input.payment_terms) supplier.payment_terms = input.payment_terms;
  if (input.credit_days !== undefined) supplier.credit_days = input.credit_days;
  if (input.delivery_days !== undefined) supplier.delivery_days = input.delivery_days;
  if (input.category) supplier.category = input.category;
  if (input.bank_name !== undefined) supplier.bank_name = input.bank_name?.trim() || null;
  if (input.bank_account_type !== undefined) supplier.bank_account_type = input.bank_account_type;
  if (input.bank_account_number !== undefined) supplier.bank_account_number = input.bank_account_number?.trim() || null;

  return structuredClone(supplier);
}

export const mockSupplierProducts: Map<string, SupplierProductDetail[]> = new Map();

export async function mockSupplierGet(id: Uuid): Promise<SupplierDetailResponse> {
  const supplier = mockSuppliers.find((s) => s.id === id);
  if (!supplier) throw new ApiError(404, "not found");

  const products = mockSupplierProducts.get(id) ?? [];
  const open_orders = mockPurchases.filter(
    (p) => p.supplier_id === id && ["PENDIENTE", "APROBADA", "PARCIAL"].includes(p.status)
  ).length;

  return {
    supplier: structuredClone(supplier),
    products: structuredClone(products),
    open_orders,
    balance: "0.00",
  };
}

export async function mockSupplierArchive(id: Uuid): Promise<void> {
  const supplier = mockSuppliers.find((s) => s.id === id);
  if (!supplier) throw new ApiError(404, "not found");

  const hasOpenOrders = mockPurchases.some(
    (p) => p.supplier_id === id && ["PENDIENTE", "APROBADA", "PARCIAL"].includes(p.status)
  );
  if (hasOpenOrders) {
    throw new ApiError(409, "El proveedor tiene órdenes o deudas abiertas.");
  }

  supplier.active = false;
  supplier.archived_at = new Date().toISOString();
}

export async function mockSupplierSetProducts(
  id: Uuid,
  items: SupplierProductItemInput[]
): Promise<SupplierProductDetail[]> {
  const supplier = mockSuppliers.find((s) => s.id === id);
  if (!supplier) throw new ApiError(404, "not found");

  for (const it of items) {
    if (it.preferred) {
      for (const [suppId, prods] of mockSupplierProducts.entries()) {
        if (suppId !== id) {
          for (const p of prods) {
            if (p.product_id === it.product_id) {
              p.preferred = false;
            }
          }
        }
      }
    }
  }

  const details: SupplierProductDetail[] = items.map((it) => {
    const prod = mockCatalog.find((c) => c.product_id === it.product_id);
    return {
      product_id: it.product_id,
      product_name: prod?.name ?? "Producto",
      sku: prod?.barcode ?? null,
      supplier_code: it.supplier_code ?? null,
      last_cost: it.min_order_qty ?? null,
      min_order_qty: it.min_order_qty ?? null,
      preferred: it.preferred,
    };
  });

  mockSupplierProducts.set(id, details);
  return structuredClone(details);
}

export async function mockPurchasesList(filter?: PurchaseFilter): Promise<Purchase[]> {
  let list = [...mockPurchases];

  if (filter?.branch_id) {
    list = list.filter((p) => p.branch_id === filter.branch_id);
  }
  if (filter?.supplier_id) {
    list = list.filter((p) => p.supplier_id === filter.supplier_id);
  }
  if (filter?.status) {
    list = list.filter((p) => p.status === filter.status);
  }
  if (filter?.from) {
    const fromTime = new Date(filter.from).getTime();
    list = list.filter((p) => new Date(p.created_at).getTime() >= fromTime);
  }
  if (filter?.to) {
    const toTime = new Date(filter.to).getTime();
    list = list.filter((p) => new Date(p.created_at).getTime() <= toTime);
  }

  list.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());

  if (filter?.limit) {
    list = list.slice(0, filter.limit);
  }

  return structuredClone(list);
}

export async function mockPurchaseGet(id: Uuid): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");
  return structuredClone(purchase);
}

export async function mockPurchaseCreate(input: CreatePurchaseInput): Promise<Purchase> {
  if (!input.items || input.items.length === 0) {
    throw new ApiError(422, "validation: purchase has no items");
  }
  for (const it of input.items) {
    if (Number(it.qty) <= 0) throw new ApiError(422, "validation: qty must be > 0");
    if (Number(it.unit_cost) < 0) throw new ApiError(422, "validation: unit_cost must be >= 0");
  }

  const existing = mockPurchases.find((p) => p.id === input.id);
  if (existing) {
    return structuredClone(existing);
  }

  const supplier = mockSuppliers.find((s) => s.id === input.supplier_id);
  if (!supplier) throw new ApiError(404, "supplier not found");

  let subtotalNum = 0;
  let taxNum = 0;

  const items: PurchaseItem[] = input.items.map((it, idx) => {
    const prod = mockCatalog.find((c) => c.product_id === it.product_id);
    const prodName = prod?.name ?? "Producto";
    const lineSubtotal = Number(it.qty) * Number(it.unit_cost);
    const taxRate = prod?.tax_code === "IVA19" ? 0.19 : 0;
    const lineTax = lineSubtotal * taxRate;

    subtotalNum += lineSubtotal;
    taxNum += lineTax;

    return {
      id: crypto.randomUUID(),
      product_id: it.product_id,
      product_name: prodName,
      sku: `SKU-${idx + 1}`,
      qty: Number(it.qty).toFixed(4),
      unit_cost: dec(Number(it.unit_cost)),
      line_total: dec(lineSubtotal),
      tax_code: (prod?.tax_code as TaxCode | null) ?? null,
      tax_rate: taxRate > 0 ? "0.19" : null,
      notes: null,
    };
  });

  const totalNum = subtotalNum + taxNum;

  const newPurchase: Purchase = {
    id: input.id,
    branch_id: input.branch_id,
    supplier_id: supplier.id,
    supplier_name: supplier.name,
    document_number: input.document_number?.trim() || null,
    supplier_invoice_date: input.supplier_invoice_date || null,
    status: "PENDIENTE",
    subtotal: dec(subtotalNum),
    tax_amount: dec(taxNum),
    total_amount: dec(totalNum),
    items_count: items.length,
    received_at: null,
    notes: input.notes?.trim() || null,
    created_at: new Date().toISOString(),
    items,
  };

  mockPurchases.unshift(newPurchase);
  return structuredClone(newPurchase);
}

export async function mockPurchaseApprove(id: Uuid): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");
  if (purchase.status !== "PENDIENTE") {
    throw new ApiError(409, `PURCHASE_INVALID_STATE: No se puede aprobar una orden en estado ${purchase.status}`);
  }
  purchase.status = "APROBADA";
  purchase.approved_at = new Date().toISOString();
  return structuredClone(purchase);
}

export async function mockPurchaseUpdate(id: Uuid, input: UpdatePurchaseInput): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");
  if (purchase.status !== "PENDIENTE") {
    throw new ApiError(409, `PURCHASE_INVALID_STATE: Solo se pueden editar órdenes en estado PENDIENTE`);
  }
  const supplier = mockSuppliers.find((s) => s.id === input.supplier_id);
  if (!supplier) throw new ApiError(404, "supplier not found");
  purchase.branch_id = input.branch_id;
  purchase.supplier_id = supplier.id;
  purchase.supplier_name = supplier.name;
  purchase.location_id = input.location_id ?? purchase.location_id;
  purchase.expected_at = input.expected_at ?? null;
  purchase.document_number = input.document_number?.trim() || null;
  purchase.supplier_invoice_date = input.supplier_invoice_date || null;
  purchase.notes = input.notes?.trim() || null;
  purchase.discount_amount = input.discount_amount ?? dec(0);
  purchase.freight_amount = input.freight_amount ?? dec(0);
  purchase.freight_mode = input.freight_mode ?? "PRORRATEAR";
  purchase.payment_terms = input.payment_terms ?? "CONTADO";
  purchase.credit_days = input.credit_days ?? 0;
  purchase.iva_to_cost = input.iva_to_cost ?? false;

  let subtotalNum = 0;
  let taxNum = 0;
  purchase.items = input.items.map((it, idx) => {
    const prod = mockCatalog.find((c) => c.product_id === it.product_id);
    const prodName = prod?.name ?? "Producto";
    const lineSubtotal = Number(it.qty) * Number(it.unit_cost);
    const taxRate = prod?.tax_code === "IVA19" ? 0.19 : 0;
    const lineTax = lineSubtotal * taxRate;
    subtotalNum += lineSubtotal;
    taxNum += lineTax;
    return {
      id: crypto.randomUUID(),
      product_id: it.product_id,
      product_name: prodName,
      sku: `SKU-${idx + 1}`,
      qty: Number(it.qty).toFixed(4),
      qty_received: "0.0000",
      unit_cost: dec(Number(it.unit_cost)),
      line_total: dec(lineSubtotal),
      tax_code: (prod?.tax_code as TaxCode | null) ?? null,
      tax_rate: taxRate > 0 ? "0.19" : null,
      tax_amount: dec(lineTax),
      landed_unit_cost: dec(Number(it.unit_cost)),
      new_sale_price: it.new_sale_price ?? null,
      notes: null,
    };
  });
  purchase.subtotal = dec(subtotalNum);
  purchase.tax_amount = dec(taxNum);
  purchase.total_amount = dec(subtotalNum + taxNum + Number(purchase.freight_amount));
  purchase.items_count = purchase.items.length;
  return structuredClone(purchase);
}

export async function mockPurchasesCostPreview(req: CostPreviewRequest): Promise<CostPreviewResponse> {
  const discount = Number(req.discount_amount || 0);
  const freight = Number(req.freight_amount || 0);
  let grossSum = 0;
  const linesData = req.items.map((it) => {
    const prod = mockCatalog.find((c) => c.product_id === it.product_id);
    const gross = Number(it.qty) * Number(it.unit_cost);
    grossSum += gross;
    return { it, prod, gross };
  });

  let subtotal = 0;
  let taxTotal = 0;
  const lines: CostPreviewLine[] = linesData.map(({ it, prod, gross }) => {
    const discWeight = grossSum > 0 ? gross / grossSum : 0;
    const lineDisc = discount * discWeight;
    const net = Math.max(0, gross - lineDisc);
    const freightWeight = grossSum > 0 ? gross / grossSum : 0;
    const lineFreight = req.freight_mode === "GASTO" ? 0 : freight * freightWeight;
    const taxRate = prod?.tax_code === "IVA19" ? 0.19 : prod?.tax_code === "INC8" ? 0.08 : 0;
    const lineTax = net * taxRate;
    const incOrIvaToCost = prod?.tax_code === "INC8" || req.iva_to_cost === true;
    const costBase = net + lineFreight + (incOrIvaToCost ? lineTax : 0);
    const landed = Number(it.qty) > 0 ? costBase / Number(it.qty) : 0;
    subtotal += net;
    taxTotal += lineTax;

    const currentPrice = prod?.price ? Number(prod.price) : null;
    const salePrice = it.new_sale_price ? Number(it.new_sale_price) : currentPrice;
    const margin = salePrice && salePrice > 0 ? ((salePrice - landed) / salePrice) * 100 : null;

    return {
      product_id: it.product_id,
      product_name: prod?.name ?? "Producto",
      qty: dec(Number(it.qty)),
      unit_cost: dec(Number(it.unit_cost)),
      purchase_factor: "1.0000",
      discount_alloc: dec(lineDisc),
      freight_alloc: dec(lineFreight),
      tax_amount: dec(lineTax),
      landed_unit_cost: dec(landed),
      current_price: currentPrice !== null ? dec(currentPrice) : null,
      new_sale_price: it.new_sale_price ?? null,
      resulting_margin_pct: margin !== null ? dec(margin) : null,
    };
  });

  return {
    lines,
    subtotal: dec(subtotal),
    discount_total: dec(discount),
    freight_total: dec(freight),
    tax_total: dec(taxTotal),
    total: dec(subtotal + taxTotal + freight),
    warnings: [],
  };
}

export async function mockPurchaseReceive(
  id: Uuid,
  body?: ReceivePurchaseRequest,
): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");

  if (purchase.status === "RECIBIDA") {
    throw new ApiError(409, "purchase is already received");
  }
  if (purchase.status === "ANULADA") {
    throw new ApiError(409, "cannot receive a cancelled purchase");
  }

  // Compatibilidad: si está PENDIENTE, auto-aprobar
  if (purchase.status === "PENDIENTE") {
    purchase.status = "APROBADA";
    purchase.approved_at = new Date().toISOString();
  }

  const cause = purchase.document_number ? `Compra ${purchase.document_number}` : "Compra";

  if (purchase.items) {
    if (body?.items && body.items.length > 0) {
      for (const rItem of body.items) {
        const pItem = purchase.items.find((i) => i.id === rItem.purchase_item_id);
        if (!pItem) continue;
        const curRcv = Number(pItem.qty_received ?? 0);
        const toRcv = Number(rItem.qty);
        const maxPending = Number(pItem.qty) - curRcv;
        if (toRcv > maxPending) {
          throw new ApiError(422, "RECEIVE_EXCEEDS_PENDING");
        }
        pItem.qty_received = (curRcv + toRcv).toFixed(4);
        mockRecordMovement(pItem.product_id, "REPOSICION", toRcv, cause, {
          type: "COMPRA",
          id: purchase.id,
        });
      }
    } else {
      // Recibir todo lo pendiente
      for (const it of purchase.items) {
        const curRcv = Number(it.qty_received ?? 0);
        const pending = Number(it.qty) - curRcv;
        if (pending > 0) {
          it.qty_received = Number(it.qty).toFixed(4);
          mockRecordMovement(it.product_id, "REPOSICION", pending, cause, {
            type: "COMPRA",
            id: purchase.id,
          });
        }
      }
    }

    const allReceived = purchase.items.every(
      (it) => Number(it.qty_received ?? 0) >= Number(it.qty),
    );
    purchase.status = allReceived ? "RECIBIDA" : "PARCIAL";
  } else {
    purchase.status = "RECIBIDA";
  }

  if (!purchase.received_at) {
    purchase.received_at = new Date().toISOString();
  }

  return structuredClone(purchase);
}

export async function mockPurchaseCancel(id: Uuid): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");

  if (purchase.status === "RECIBIDA" || purchase.status === "PARCIAL") {
    throw new ApiError(409, `PURCHASE_INVALID_STATE: No se puede anular una orden en estado ${purchase.status}`);
  }
  if (purchase.status === "ANULADA") {
    throw new ApiError(409, "purchase is already cancelled");
  }

  purchase.status = "ANULADA";
  return structuredClone(purchase);
}

export async function mockPurchaseSummary(branchId?: Uuid, month?: string): Promise<PurchaseSummary> {
  const currentMonth = month || new Date().toISOString().slice(0, 7);
  const thisMonthPurchases = mockPurchases.filter((p) => {
    if (branchId && p.branch_id !== branchId) return false;
    const dateStr = (p.received_at || p.created_at).slice(0, 7);
    return dateStr === currentMonth;
  });

  const totalNum = thisMonthPurchases
    .filter((p) => p.status === "RECIBIDA")
    .reduce((sum, p) => sum + Number(p.total_amount), 0);

  const toReceiveNum = thisMonthPurchases
    .filter((p) => p.status === "PENDIENTE" || p.status === "APROBADA" || p.status === "PARCIAL")
    .reduce((sum, p) => sum + Number(p.total_amount), 0);

  const activeCount = mockSuppliers.filter((s) => s.active).length;

  return {
    total_purchased_month: dec(totalNum),
    to_receive_month: dec(toReceiveNum),
    to_pay_balance: dec(toReceiveNum + totalNum * 0.4),
    active_suppliers: activeCount,
  };
}

// ------------------------------------------------------------ cuentas por pagar (F2.13)

const now = new Date();
const daysAgo = (d: number) => new Date(now.getTime() - d * 86400000).toISOString().slice(0, 10);
const daysAhead = (d: number) => new Date(now.getTime() + d * 86400000).toISOString().slice(0, 10);

const mockPaymentsStore: Record<Uuid, PayablePayment[]> = {
  "payable-001": [
    {
      id: "paym-001",
      payable_id: "payable-001",
      amount: "500000.00",
      method: "TRANSFERENCIA",
      reference: "TR-987654",
      cash_session_id: null,
      paid_at: new Date(now.getTime() - 2 * 86400000).toISOString(),
      created_by: "user-admin",
      created_by_name: "Administrador General",
    },
  ],
};

const mockPayables: Payable[] = [
  {
    id: "payable-001",
    tenant_id: "tenant-001",
    branch_id: "branch-001",
    supplier_id: "sup-001",
    supplier_name: "Distribuidora Los Andes S.A.S.",
    source_type: "COMPRA",
    source_id: "pur-001",
    document_number: "FAC-90812",
    amount_total: "1500000.00",
    amount_paid: "500000.00",
    balance: "1000000.00",
    status: "PARCIAL",
    due_date: daysAhead(15),
    overdue: false,
    days_overdue: 0,
    written_off_reason: null,
    created_at: daysAgo(5),
  },
  {
    id: "payable-002",
    tenant_id: "tenant-001",
    branch_id: "branch-001",
    supplier_id: "sup-002",
    supplier_name: "Lácteos del Campo",
    source_type: "COMPRA",
    source_id: "pur-002",
    document_number: "FAC-4412",
    amount_total: "620000.00",
    amount_paid: "0.00",
    balance: "620000.00",
    status: "ABIERTA",
    due_date: daysAgo(5),
    overdue: true,
    days_overdue: 5,
    written_off_reason: null,
    created_at: daysAgo(20),
  },
  {
    id: "payable-003",
    tenant_id: "tenant-001",
    branch_id: "branch-001",
    supplier_id: "sup-003",
    supplier_name: "Empaques y Plásticos Bogotá",
    source_type: "COMPRA",
    source_id: "pur-003",
    document_number: "FAC-1102",
    amount_total: "340000.00",
    amount_paid: "0.00",
    balance: "340000.00",
    status: "ABIERTA",
    due_date: daysAgo(42),
    overdue: true,
    days_overdue: 42,
    written_off_reason: null,
    created_at: daysAgo(60),
  },
];

export async function mockPayablesList(filter?: PayablesFilter): Promise<Payable[]> {
  let list = structuredClone(mockPayables);
  if (filter?.branch_id) {
    list = list.filter((p) => p.branch_id === filter.branch_id);
  }
  if (filter?.status) {
    list = list.filter((p) => p.status === filter.status);
  }
  if (filter?.supplier_id) {
    list = list.filter((p) => p.supplier_id === filter.supplier_id);
  }
  if (filter?.overdue !== undefined) {
    list = list.filter((p) => p.overdue === filter.overdue);
  }
  return list;
}

export async function mockPayablesAging(branchId?: Uuid): Promise<PayablesAging> {
  const openPayables = mockPayables.filter((p) => {
    if (branchId && p.branch_id !== branchId) return false;
    return p.status === "ABIERTA" || p.status === "PARCIAL";
  });

  let al_dia = 0;
  let d1_30 = 0;
  let d31_60 = 0;
  let d61_90 = 0;
  let d91_120 = 0;
  let d120_mas = 0;

  for (const p of openPayables) {
    const bal = Number(p.balance);
    if (!p.overdue || p.days_overdue <= 0) {
      al_dia += bal;
    } else if (p.days_overdue <= 30) {
      d1_30 += bal;
    } else if (p.days_overdue <= 60) {
      d31_60 += bal;
    } else if (p.days_overdue <= 90) {
      d61_90 += bal;
    } else if (p.days_overdue <= 120) {
      d91_120 += bal;
    } else {
      d120_mas += bal;
    }
  }

  return {
    al_dia: dec(al_dia),
    d1_30: dec(d1_30),
    d31_60: dec(d31_60),
    d61_90: dec(d61_90),
    d91_120: dec(d91_120),
    d120_mas: dec(d120_mas),
  };
}

export async function mockPayableGet(id: Uuid): Promise<PayableDetail> {
  const p = mockPayables.find((x) => x.id === id);
  if (!p) {
    throw new ApiError(404, "Cuenta por pagar no encontrada");
  }
  const payments = mockPaymentsStore[id] || [];
  return {
    ...structuredClone(p),
    payments: structuredClone(payments),
  };
}

export async function mockPayablePayment(
  id: Uuid,
  input: CreatePayablePaymentInput,
): Promise<PayableDetail> {
  const p = mockPayables.find((x) => x.id === id);
  if (!p) {
    throw new ApiError(404, "Cuenta por pagar no encontrada");
  }
  if (p.status === "PAGADA" || p.status === "CASTIGADA") {
    throw new ApiError(409, "La cuenta por pagar está cerrada", "PAYABLE_CLOSED");
  }

  const amt = Number(input.amount);
  const bal = Number(p.balance);
  if (amt <= 0) {
    throw new ApiError(422, "El monto debe ser mayor a 0", "PAYMENT_AMOUNT_INVALID");
  }
  if (amt > bal) {
    throw new ApiError(422, "El monto supera el saldo", "PAYMENT_EXCEEDS_BALANCE");
  }

  const newPaid = Number(p.amount_paid) + amt;
  p.amount_paid = dec(newPaid);
  const newBal = Number(p.amount_total) - newPaid;
  p.balance = dec(newBal);
  p.status = newBal <= 0 ? "PAGADA" : "PARCIAL";

  const newPayment: PayablePayment = {
    id: input.id,
    payable_id: id,
    amount: dec(amt),
    method: input.method,
    reference: input.reference || null,
    cash_session_id: input.method === "EFECTIVO" ? "session-current" : null,
    paid_at: new Date().toISOString(),
    created_by: "user-current",
    created_by_name: "Usuario Actual",
  };

  if (!mockPaymentsStore[id]) {
    mockPaymentsStore[id] = [];
  }
  mockPaymentsStore[id].unshift(newPayment);

  return {
    ...structuredClone(p),
    payments: structuredClone(mockPaymentsStore[id]),
  };
}

export async function mockPayableWriteOff(
  id: Uuid,
  input: WriteOffPayableInput,
): Promise<PayableDetail> {
  const p = mockPayables.find((x) => x.id === id);
  if (!p) {
    throw new ApiError(404, "Cuenta por pagar no encontrada");
  }
  if (p.status === "PAGADA" || p.status === "CASTIGADA") {
    throw new ApiError(409, "La cuenta por pagar ya está cerrada", "PAYABLE_CLOSED");
  }

  p.status = "CASTIGADA";
  p.written_off_reason = input.reason;

  const payments = mockPaymentsStore[id] || [];
  return {
    ...structuredClone(p),
    payments: structuredClone(payments),
  };
}

// ------------------------------------------------------------ gastos (F2.14)

export const mockExpenseCategories: ExpenseCategory[] = [
  { id: "cat-1", name: "Arriendo", color: "#8B5CF6", active: true },
  { id: "cat-2", name: "Servicios", color: "#F59E0B", active: true },
  { id: "cat-3", name: "Nómina", color: "#3B82F6", active: true },
  { id: "cat-4", name: "Proveedores", color: "#0D9488", active: true },
  { id: "cat-5", name: "Mantenimiento", color: "#64748B", active: true },
  { id: "cat-6", name: "Otros", color: "#EC4899", active: true },
];

export const mockExpenses: Expense[] = [
  {
    id: "exp-001",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 1,
    document_number: "GAS-1",
    category_id: "cat-1",
    category_name: "Arriendo",
    category_color: "#8B5CF6",
    supplier_id: "20000000-0000-4000-8000-000000000001",
    supplier_name: "Inmobiliaria El Poblado S.A.",
    description: "Pago Arriendo Mes Noviembre Local 102",
    amount: "3500000.00",
    tax_amount: "0.00",
    total: "3500000.00",
    expense_date: "2026-11-01",
    payment_method: "Transferencia Bancolombia",
    attachment_name: "Contrato_Recibo.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-11-01T10:00:00Z",
    reject_reason: null,
    created_at: "2026-11-01T09:30:00Z",
    payable_id: "pay-exp-001",
    payable_amount_paid: "3500000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-002",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 2,
    document_number: "GAS-2",
    category_id: "cat-3",
    category_name: "Nómina",
    category_color: "#3B82F6",
    supplier_id: null,
    supplier_name: null,
    description: "Quincena Empleados Turno Mañana",
    amount: "3250000.00",
    tax_amount: "0.00",
    total: "3250000.00",
    expense_date: "2026-10-31",
    payment_method: "Dispersión Bancaria",
    attachment_name: "Comprobante_Nomina.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-31T17:00:00Z",
    reject_reason: null,
    created_at: "2026-10-31T12:00:00Z",
    payable_id: "pay-exp-002",
    payable_amount_paid: "3250000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-003",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 3,
    document_number: "GAS-3",
    category_id: "cat-3",
    category_name: "Nómina",
    category_color: "#3B82F6",
    supplier_id: null,
    supplier_name: null,
    description: "Quincena Empleados Turno Tarde y Logística",
    amount: "3250000.00",
    tax_amount: "0.00",
    total: "3250000.00",
    expense_date: "2026-10-31",
    payment_method: "Dispersión Bancaria",
    attachment_name: "Nomina_Tarde.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-31T17:10:00Z",
    reject_reason: null,
    created_at: "2026-10-31T12:30:00Z",
    payable_id: "pay-exp-003",
    payable_amount_paid: "3250000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-004",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 4,
    document_number: "GAS-4",
    category_id: "cat-4",
    category_name: "Proveedores",
    category_color: "#0D9488",
    supplier_id: "20000000-0000-4000-8000-000000000002",
    supplier_name: "Distribuidora Lácteos del Campo",
    description: "Insumos lácteos y quesos para cafetería",
    amount: "1450000.00",
    tax_amount: "0.00",
    total: "1450000.00",
    expense_date: "2026-10-29",
    payment_method: "Transferencia Bancaria",
    attachment_name: "Factura_FE_9823.pdf",
    status: "APROBADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-29T11:00:00Z",
    reject_reason: null,
    created_at: "2026-10-29T09:45:00Z",
    payable_id: "pay-exp-004",
    payable_amount_paid: "0.00",
    payable_status: "ABIERTA",
  },
  {
    id: "exp-005",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 5,
    document_number: "GAS-5",
    category_id: "cat-2",
    category_name: "Servicios",
    category_color: "#F59E0B",
    supplier_id: null,
    supplier_name: null,
    description: "Factura Enel Codensa Energía Eléctrica",
    amount: "820000.00",
    tax_amount: "0.00",
    total: "820000.00",
    expense_date: "2026-10-28",
    payment_method: "Débito Automático",
    attachment_name: "Factura_Enel.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-28T09:00:00Z",
    reject_reason: null,
    created_at: "2026-10-28T08:15:00Z",
    payable_id: "pay-exp-005",
    payable_amount_paid: "820000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-006",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 6,
    document_number: "GAS-6",
    category_id: "cat-4",
    category_name: "Proveedores",
    category_color: "#0D9488",
    supplier_id: null,
    supplier_name: "Empaques y Plásticos Bogotá",
    description: "Compra de Bolsas Biodegradables y Rollos POS",
    amount: "750000.00",
    tax_amount: "0.00",
    total: "750000.00",
    expense_date: "2026-10-26",
    payment_method: "Transferencia Bancolombia",
    attachment_name: "Factura_Bolsas.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-26T15:30:00Z",
    reject_reason: null,
    created_at: "2026-10-26T14:00:00Z",
    payable_id: "pay-exp-006",
    payable_amount_paid: "750000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-007",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 7,
    document_number: "GAS-7",
    category_id: "cat-2",
    category_name: "Servicios",
    category_color: "#F59E0B",
    supplier_id: null,
    supplier_name: null,
    description: "Factura Acueducto y Alcantarillado Bogotá",
    amount: "430000.00",
    tax_amount: "0.00",
    total: "430000.00",
    expense_date: "2026-10-24",
    payment_method: "Nequi Empresa",
    attachment_name: "Recibo_Agua.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-24T10:00:00Z",
    reject_reason: null,
    created_at: "2026-10-24T09:10:00Z",
    payable_id: "pay-exp-007",
    payable_amount_paid: "430000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-008",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 8,
    document_number: "GAS-8",
    category_id: "cat-5",
    category_name: "Mantenimiento",
    category_color: "#64748B",
    supplier_id: null,
    supplier_name: "Refrigeración Industrial SAS",
    description: "Mantenimiento Preventivo Congelador y Neveras",
    amount: "350000.00",
    tax_amount: "0.00",
    total: "350000.00",
    expense_date: "2026-10-22",
    payment_method: "Nequi Empresa",
    attachment_name: "Soporte_Tecnico.jpg",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-22T16:00:00Z",
    reject_reason: null,
    created_at: "2026-10-22T11:00:00Z",
    payable_id: "pay-exp-008",
    payable_amount_paid: "350000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-009",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 9,
    document_number: "GAS-9",
    category_id: "cat-2",
    category_name: "Servicios",
    category_color: "#F59E0B",
    supplier_id: null,
    supplier_name: null,
    description: "Factura Gas Natural Vanti Sede Principal",
    amount: "200000.00",
    tax_amount: "0.00",
    total: "200000.00",
    expense_date: "2026-10-20",
    payment_method: "Bancolombia",
    attachment_name: "Factura_Gas.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-20T11:00:00Z",
    reject_reason: null,
    created_at: "2026-10-20T09:30:00Z",
    payable_id: "pay-exp-009",
    payable_amount_paid: "200000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-010",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 10,
    document_number: "GAS-10",
    category_id: "cat-6",
    category_name: "Otros",
    category_color: "#EC4899",
    supplier_id: null,
    supplier_name: null,
    description: "Insumos de Aseo y Desinfección Cafetería",
    amount: "185000.00",
    tax_amount: "0.00",
    total: "185000.00",
    expense_date: "2026-10-18",
    payment_method: "Efectivo / Caja Menor",
    attachment_name: "Factura_Aseo.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-18T14:30:00Z",
    reject_reason: null,
    created_at: "2026-10-18T10:15:00Z",
    payable_id: "pay-exp-010",
    payable_amount_paid: "185000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-011",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 11,
    document_number: "GAS-11",
    category_id: "cat-2",
    category_name: "Servicios",
    category_color: "#F59E0B",
    supplier_id: null,
    supplier_name: null,
    description: "Internet Fibra Óptica y Telefonía Claro",
    amount: "175000.00",
    tax_amount: "0.00",
    total: "175000.00",
    expense_date: "2026-10-15",
    payment_method: "Débito Automático",
    attachment_name: "Factura_Internet.pdf",
    status: "PAGADO",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: "user-default",
    decided_by_name: "Admin Demo",
    decided_at: "2026-10-15T12:00:00Z",
    reject_reason: null,
    created_at: "2026-10-15T09:00:00Z",
    payable_id: "pay-exp-011",
    payable_amount_paid: "175000.00",
    payable_status: "PAGADA",
  },
  {
    id: "exp-012",
    tenant_id: "tenant-default",
    branch_id: MOCK_BRANCH_ID,
    prefix: "GAS",
    number: 12,
    document_number: "GAS-12",
    category_id: "cat-6",
    category_name: "Otros",
    category_color: "#EC4899",
    supplier_id: null,
    supplier_name: null,
    description: "Reparación Urgente Chapa y Candados Bodega",
    amount: "110000.00",
    tax_amount: "0.00",
    total: "110000.00",
    expense_date: "2026-10-12",
    payment_method: "Efectivo / Caja Menor",
    attachment_name: null,
    status: "PENDIENTE",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: null,
    decided_by_name: null,
    decided_at: null,
    reject_reason: null,
    created_at: "2026-10-12T15:20:00Z",
    payable_id: null,
    payable_amount_paid: null,
    payable_status: null,
  },
];

let nextExpenseNumber = 13;

export async function mockExpenseCategoriesList(
  includeInactive = false,
): Promise<ExpenseCategory[]> {
  const filtered = includeInactive
    ? mockExpenseCategories
    : mockExpenseCategories.filter((c) => c.active);
  return structuredClone(filtered);
}

export async function mockExpenseCategoryCreate(
  input: CreateExpenseCategoryInput,
): Promise<ExpenseCategory> {
  const newCat: ExpenseCategory = {
    id: `cat-${Date.now()}`,
    name: input.name.trim(),
    color: input.color || "#64748B",
    active: true,
  };
  mockExpenseCategories.push(newCat);
  return structuredClone(newCat);
}

export async function mockExpenseCategoryUpdate(
  id: Uuid,
  input: UpdateExpenseCategoryInput,
): Promise<ExpenseCategory> {
  const cat = mockExpenseCategories.find((c) => c.id === id);
  if (!cat) throw new ApiError(404, "Categoría no encontrada");
  cat.name = input.name.trim();
  cat.color = input.color;
  cat.active = input.active;
  return structuredClone(cat);
}

export async function mockExpensesList(
  filter?: ExpensesFilter,
): Promise<Expense[]> {
  let list = [...mockExpenses];
  if (filter?.branch_id) {
    list = list.filter((e) => e.branch_id === filter.branch_id);
  }
  if (filter?.category_id) {
    list = list.filter((e) => e.category_id === filter.category_id);
  }
  if (filter?.supplier_id) {
    list = list.filter((e) => e.supplier_id === filter.supplier_id);
  }
  if (filter?.status) {
    list = list.filter((e) => e.status === filter.status);
  }
  if (filter?.from) {
    list = list.filter((e) => e.expense_date >= filter.from!);
  }
  if (filter?.to) {
    list = list.filter((e) => e.expense_date <= filter.to!);
  }
  list.sort((a, b) => (b.expense_date > a.expense_date ? 1 : -1));
  return structuredClone(list);
}

export async function mockExpenseGet(id: Uuid): Promise<Expense> {
  const exp = mockExpenses.find((e) => e.id === id);
  if (!exp) throw new ApiError(404, "Gasto no encontrado");
  return structuredClone(exp);
}

export async function mockExpensesSummary(
  branchId?: Uuid,
  month?: string,
): Promise<ExpenseSummary> {
  let list = [...mockExpenses];
  if (branchId) {
    list = list.filter((e) => e.branch_id === branchId);
  }
  if (month) {
    list = list.filter((e) => e.expense_date.startsWith(month));
  }

  let total_approved = 0;
  let total_pending = 0;
  let total_paid = 0;
  let count_pending = 0;
  let count_approved = 0;

  for (const e of list) {
    const tot = Number(e.total);
    if (e.status === "APROBADO") {
      total_approved += tot;
      count_approved += 1;
    } else if (e.status === "PENDIENTE") {
      total_pending += tot;
      count_pending += 1;
    } else if (e.status === "PAGADO") {
      total_paid += tot;
      total_approved += tot;
      count_approved += 1;
    }
  }

  return {
    total_approved_month: dec(total_approved),
    total_pending_month: dec(total_pending),
    total_paid_month: dec(total_paid),
    count_pending,
    count_approved,
  };
}

export async function mockExpenseCreate(
  input: CreateExpenseInput,
): Promise<Expense> {
  const cat = mockExpenseCategories.find((c) => c.id === input.category_id);
  if (!cat || !cat.active) {
    throw new ApiError(422, "Categoría no válida");
  }

  const supp = input.supplier_id
    ? mockSuppliers.find((s) => s.id === input.supplier_id)
    : null;

  const num = nextExpenseNumber++;
  const amt = Number(input.amount);
  const tax = Number(input.tax_amount || 0);
  const total = amt + tax;

  const newExp: Expense = {
    id: input.id,
    tenant_id: "tenant-default",
    branch_id: input.branch_id,
    prefix: "GAS",
    number: num,
    document_number: `GAS-${num}`,
    category_id: input.category_id,
    category_name: cat.name,
    category_color: cat.color,
    supplier_id: input.supplier_id || null,
    supplier_name: supp?.name || null,
    description: input.description.trim(),
    amount: dec(amt),
    tax_amount: dec(tax),
    total: dec(total),
    expense_date: input.expense_date,
    payment_method: input.payment_method || "Efectivo / Caja Menor",
    attachment_name: input.attachment_name || null,
    status: "PENDIENTE",
    created_by: "user-default",
    created_by_name: "Admin Demo",
    decided_by: null,
    decided_by_name: null,
    decided_at: null,
    reject_reason: null,
    created_at: new Date().toISOString(),
    payable_id: null,
    payable_amount_paid: null,
    payable_status: null,
  };

  mockExpenses.unshift(newExp);
  return structuredClone(newExp);
}

export async function mockExpenseApprove(id: Uuid): Promise<Expense> {
  const exp = mockExpenses.find((e) => e.id === id);
  if (!exp) throw new ApiError(404, "Gasto no encontrado");
  if (exp.status !== "PENDIENTE") {
    throw new ApiError(
      409,
      "El gasto no admite esa acción en su estado actual",
      "EXPENSE_INVALID_STATE",
    );
  }

  exp.status = "APROBADO";
  exp.decided_by = "user-default";
  exp.decided_by_name = "Admin Demo";
  exp.decided_at = new Date().toISOString();

  const payableId = `pay-exp-${exp.id.slice(0, 8)}`;
  exp.payable_id = payableId;
  const newPayable: Payable = {
    id: payableId,
    tenant_id: exp.tenant_id,
    branch_id: exp.branch_id,
    supplier_id: exp.supplier_id,
    supplier_name: exp.supplier_name,
    source_type: "GASTO",
    source_id: exp.id,
    document_number: exp.document_number,
    amount_total: exp.total,
    amount_paid: "0.00",
    balance: exp.total,
    status: "ABIERTA",
    due_date: exp.expense_date,
    overdue: false,
    days_overdue: 0,
    written_off_reason: null,
    created_at: new Date().toISOString(),
  };
  mockPayables.unshift(newPayable);

  return structuredClone(exp);
}

export async function mockExpenseReject(
  id: Uuid,
  input: RejectExpenseInput,
): Promise<Expense> {
  const exp = mockExpenses.find((e) => e.id === id);
  if (!exp) throw new ApiError(404, "Gasto no encontrado");
  if (exp.status !== "PENDIENTE") {
    throw new ApiError(
      409,
      "El gasto no admite esa acción en su estado actual",
      "EXPENSE_INVALID_STATE",
    );
  }

  exp.status = "RECHAZADO";
  exp.decided_by = "user-default";
  exp.decided_by_name = "Admin Demo";
  exp.decided_at = new Date().toISOString();
  exp.reject_reason = input.reason.trim();

  return structuredClone(exp);
}

export async function mockExpenseCancel(id: Uuid): Promise<Expense> {
  const exp = mockExpenses.find((e) => e.id === id);
  if (!exp) throw new ApiError(404, "Gasto no encontrado");
  if (exp.status !== "PENDIENTE") {
    throw new ApiError(
      409,
      "El gasto no admite esa acción en su estado actual",
      "EXPENSE_INVALID_STATE",
    );
  }

  exp.status = "CANCELADO";
  exp.decided_by = "user-default";
  exp.decided_by_name = "Admin Demo";
  exp.decided_at = new Date().toISOString();

  return structuredClone(exp);
}

// ------------------------------------------------------------ sugerencias de compra (F2.17)

export async function mockPurchaseSuggestions(
  branchId?: Uuid,
): Promise<PurchaseSuggestionGroup[]> {
  void branchId;
  const sup1 = mockSuppliers[0];
  const sup2 = mockSuppliers[1];

  return [
    {
      supplier: sup1 ? { id: sup1.id, name: sup1.name } : null,
      items: [
        {
          product_id: "prod-001",
          name: "Café Especial 500g",
          on_hand: "4.0000",
          min_qty: "10.0000",
          suggested_qty: "12.0000",
          purchase_uom: "CAJA",
          last_cost: "24000.00",
        },
        {
          product_id: "prod-002",
          name: "Azúcar Orgánica 1kg",
          on_hand: "2.0000",
          min_qty: "8.0000",
          suggested_qty: "10.0000",
          purchase_uom: "BULTO",
          last_cost: "18500.00",
        },
      ],
    },
    {
      supplier: sup2 ? { id: sup2.id, name: sup2.name } : null,
      items: [
        {
          product_id: "prod-003",
          name: "Leche Entera 1L",
          on_hand: "5.0000",
          min_qty: "15.0000",
          suggested_qty: "20.0000",
          purchase_uom: "CANASTILLA",
          last_cost: "4200.00",
        },
      ],
    },
  ];
}

export async function mockCreateFromSuggestions(
  input: CreateFromSuggestionsInput,
): Promise<Uuid[]> {
  const suggestions = await mockPurchaseSuggestions(input.branch_id);
  const createdIds: Uuid[] = [];

  for (const supId of input.supplier_ids) {
    const group = suggestions.find((g) => g.supplier?.id === supId);
    if (!group || group.items.length === 0) continue;

    const newId = `purch-sugg-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const newPurchase: Purchase = {
      id: newId,
      branch_id: input.branch_id,
      supplier_id: supId,
      supplier_name: group.supplier?.name ?? "Proveedor",
      status: "PENDIENTE",
      document_number: `OC-${Math.floor(1000 + Math.random() * 9000)}`,
      supplier_invoice_date: null,
      subtotal: "0.00",
      tax_amount: "0.00",
      total_amount: "0.00",
      items_count: group.items.length,
      received_at: null,
      created_at: new Date().toISOString(),
      notes: "Sugerencia de compra automática",
      items: group.items.map((it, idx) => ({
        id: `pitem-${Date.now()}-${idx}`,
        product_id: it.product_id,
        product_name: it.name,
        sku: null,
        qty: it.suggested_qty,
        qty_received: "0.0000",
        unit_cost: it.last_cost ?? "0.00",
        line_total: (Number(it.suggested_qty) * Number(it.last_cost ?? 0)).toFixed(2),
        discount_alloc: "0.00",
        freight_alloc: "0.00",
        tax_code: null,
        tax_rate: null,
        tax_amount: "0.00",
        landed_unit_cost: it.last_cost ?? "0.0000",
        new_sale_price: null,
        purchase_unit: it.purchase_uom,
      })),
    };

    const subtotal = newPurchase.items!.reduce((acc, it) => acc + Number(it.line_total), 0);
    newPurchase.subtotal = subtotal.toFixed(2);
    newPurchase.total_amount = subtotal.toFixed(2);

    mockPurchases.unshift(newPurchase);
    createdIds.push(newId);
  }

  return createdIds;
}

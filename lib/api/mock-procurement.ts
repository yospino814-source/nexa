// Proveedores y compras simuladas para el modo demo.
//
// Recibir una compra suma stock simulado y anota en el kardex mediante
// mockRecordMovement de mock-inventory.ts.

import { ApiError } from "./client";
import { MOCK_BRANCH_ID, mockCatalog } from "./mock";
import { mockRecordMovement } from "./mock-inventory";
import type {
  CreatePurchaseInput,
  CreateSupplierInput,
  Decimal,
  Purchase,
  PurchaseFilter,
  PurchaseItem,
  PurchaseStatus,
  PurchaseSummary,
  Supplier,
  SupplierDetailResponse,
  SupplierProductDetail,
  SupplierProductItemInput,
  TaxCode,
  UpdateSupplierInput,
  Uuid,
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

export async function mockPurchaseReceive(id: Uuid): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");

  if (purchase.status === "RECIBIDA") {
    throw new ApiError(409, "purchase is already received");
  }
  if (purchase.status === "ANULADA") {
    throw new ApiError(409, "cannot receive a cancelled purchase");
  }

  const cause = purchase.document_number ? `Compra ${purchase.document_number}` : "Compra";

  if (purchase.items) {
    for (const it of purchase.items) {
      mockRecordMovement(it.product_id, "REPOSICION", Number(it.qty), cause, {
        type: "COMPRA",
        id: purchase.id,
      });
    }
  }

  purchase.status = "RECIBIDA";
  purchase.received_at = new Date().toISOString();

  return structuredClone(purchase);
}

export async function mockPurchaseCancel(id: Uuid): Promise<Purchase> {
  const purchase = mockPurchases.find((p) => p.id === id);
  if (!purchase) throw new ApiError(404, "not found");

  if (purchase.status === "RECIBIDA") {
    throw new ApiError(409, "cannot cancel an already received purchase");
  }
  if (purchase.status === "ANULADA") {
    throw new ApiError(409, "purchase is already cancelled");
  }

  purchase.status = "ANULADA";
  return structuredClone(purchase);
}

export async function mockPurchaseSummary(branchId?: Uuid, month?: string): Promise<PurchaseSummary> {
  const currentMonth = month || new Date().toISOString().slice(0, 7);
  const receivedThisMonth = mockPurchases.filter((p) => {
    if (p.status !== "RECIBIDA") return false;
    if (branchId && p.branch_id !== branchId) return false;
    const dateStr = (p.received_at || p.created_at).slice(0, 7);
    return dateStr === currentMonth;
  });

  const totalNum = receivedThisMonth.reduce((sum, p) => sum + Number(p.total_amount), 0);
  const activeCount = mockSuppliers.filter((s) => s.active).length;

  return {
    total_purchased_month: dec(totalNum),
    active_suppliers: activeCount,
  };
}

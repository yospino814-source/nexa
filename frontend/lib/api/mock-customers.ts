import { ApiError } from "./client";
import type {
  CreateCustomerInput,
  Customer,
  CustomerDetail,
  CustomerQuery,
  CustomerUpsertResponse,
  DocType,
  SaleHistoryPage,
  SaleHistoryRow,
} from "./types";
import { defaultPersonType } from "../customers";

/**
 * DV para NIT usando el algoritmo de la DIAN (módulo 11).
 */
export function nitDv(nit: string): string {
  const v = [41, 37, 33, 29, 23, 19, 17, 13, 7, 3];
  const digits = nit.replace(/\D/g, "");
  const len = digits.length;
  let sum = 0;
  for (let i = 0; i < len; i++) {
    const digit = parseInt(digits[len - 1 - i], 10);
    const weight = v[v.length - 1 - i] ?? 0;
    sum += digit * weight;
  }
  const rem = sum % 11;
  if (rem === 0 || rem === 1) return String(rem);
  return String(11 - rem);
}

/**
 * Calcula campos faltantes para facturación electrónica en el orden exacto:
 * 1. documento
 * 2. correo
 * 3. municipio
 * 4. dirección de facturación
 */
export function computeMissingFiscal(c: Customer): string[] {
  const missing: string[] = [];
  if (!c.doc_type || !c.doc_number || !c.doc_number.trim()) {
    missing.push("documento");
  }
  if (!c.email || !c.email.trim()) {
    missing.push("correo");
  }
  if (!c.city_code || !c.city_code.trim()) {
    missing.push("municipio");
  }
  if (!c.billing_address || !c.billing_address.trim()) {
    missing.push("dirección de facturación");
  }
  return missing;
}

let mockCanOverrideCredit = true;
export function setMockCreditOverride(allowed: boolean) {
  mockCanOverrideCredit = allowed;
}

function hasCreditOverride(): boolean {
  if (typeof window !== "undefined") {
    const override = (window as unknown as { __mockCanOverrideCredit?: boolean }).__mockCanOverrideCredit;
    if (override !== undefined) return override;
  }
  return mockCanOverrideCredit;
}

export const mockCustomers: Customer[] = [
  {
    id: "c0000000-0000-0000-0000-000000000001",
    doc_type: "NIT",
    doc_number: "900123456",
    dv: "8",
    name: "Distribuidora El Triunfo SAS",
    phone: "3104567890",
    email: "contacto@eltriunfo.com.co",
    price_tier_id: null,
    person_type: "JURIDICA",
    tax_regime: "RESPONSABLE_IVA",
    fiscal_responsibilities: ["O-13"],
    ciiu: "4711",
    withholding_agent: true,
    city_code: "05001",
    billing_address: "Cra 43A # 1-50 Of 301",
    shipping_address: "Bodega 4 Parque Industrial",
    pinned_note: "Cliente VIP, prioridad en entregas.",
    credit_limit: "500000.00",
    credit_block_days: 30,
    archived_at: null,
    balance: "150000.00",
    max_days_overdue: 10,
  },
  {
    id: "c0000000-0000-0000-0000-000000000002",
    doc_type: "CC",
    doc_number: "1020304050",
    dv: null,
    name: "Carlos Pérez",
    phone: "3001234567",
    email: "carlos.perez@gmail.com",
    price_tier_id: "t0000000-0000-0000-0000-000000000001",
    person_type: "NATURAL",
    tax_regime: "NO_RESPONSABLE",
    fiscal_responsibilities: ["R-99-PN"],
    ciiu: null,
    withholding_agent: false,
    city_code: "11001",
    billing_address: "Calle 10 # 20-30",
    shipping_address: null,
    pinned_note: null,
    credit_limit: "500000.00",
    credit_block_days: 30,
    archived_at: null,
    balance: "0.00",
    max_days_overdue: 0,
  },
  {
    id: "c0000000-0000-0000-0000-000000000003",
    doc_type: "CC",
    doc_number: "52987654",
    dv: null,
    name: "Luz Marina Gómez",
    phone: "3159876543",
    email: "luz.gomez@hotmail.com",
    price_tier_id: null,
    person_type: "NATURAL",
    tax_regime: "NO_RESPONSABLE",
    fiscal_responsibilities: ["R-99-PN"],
    ciiu: null,
    withholding_agent: false,
    city_code: null,
    billing_address: null,
    shipping_address: null,
    pinned_note: null,
    credit_limit: "200000.00",
    credit_block_days: 30,
    archived_at: null,
    balance: "180000.00",
    max_days_overdue: 0,
  },
  {
    id: "c0000000-0000-0000-0000-000000000004",
    doc_type: "CC",
    doc_number: "80123456",
    dv: null,
    name: "Jorge Iguarán",
    phone: "3201122334",
    email: "jorge.iguaran@outlook.com",
    price_tier_id: null,
    person_type: "NATURAL",
    tax_regime: "NO_RESPONSABLE",
    fiscal_responsibilities: ["R-99-PN"],
    ciiu: null,
    withholding_agent: false,
    city_code: "05001",
    billing_address: "Calle 50 # 40-10",
    shipping_address: null,
    pinned_note: null,
    credit_limit: "300000.00",
    credit_block_days: 30,
    archived_at: null,
    balance: "100000.00",
    max_days_overdue: 40,
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
    person_type: "NATURAL",
    tax_regime: "NO_RESPONSABLE",
    fiscal_responsibilities: ["R-99-PN"],
    ciiu: null,
    withholding_agent: false,
    city_code: null,
    billing_address: null,
    shipping_address: null,
    pinned_note: null,
    credit_limit: "0.00",
    credit_block_days: 30,
    archived_at: null,
    balance: "0.00",
    max_days_overdue: 0,
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
    person_type: "NATURAL",
    tax_regime: "NO_RESPONSABLE",
    fiscal_responsibilities: ["R-99-PN"],
    ciiu: null,
    withholding_agent: false,
    city_code: "76001",
    billing_address: "Av 6N # 22-10",
    shipping_address: null,
    pinned_note: null,
    credit_limit: "0.00",
    credit_block_days: 30,
    archived_at: "2026-10-15T10:00:00Z",
    balance: "0.00",
    max_days_overdue: 0,
  },
];

function generatePurchasesForCustomer(c: Customer): SaleHistoryRow[] {
  return [
    {
      id: `${c.id.slice(0, -4)}0001`,
      receipt_number: "POS-001234",
      created_at: new Date(Date.now() - 2 * 86400000).toISOString(),
      cashier_id: null,
      cashier_name: "Carlos M.",
      customer_label: c.name,
      total: "45000.00",
      refunded_amount: "0.00",
      methods: ["EFECTIVO"],
      status: "COBRADA",
      fiscal_status: "ACEPTADO",
    },
    {
      id: `${c.id.slice(0, -4)}0002`,
      receipt_number: "POS-001280",
      created_at: new Date(Date.now() - 5 * 86400000).toISOString(),
      cashier_id: null,
      cashier_name: "Carlos M.",
      customer_label: c.name,
      total: "128000.00",
      refunded_amount: "0.00",
      methods: ["DATAFONO"],
      status: "COBRADA",
      fiscal_status: "ACEPTADO",
    },
    {
      id: `${c.id.slice(0, -4)}0003`,
      receipt_number: "POS-001350",
      created_at: new Date(Date.now() - 12 * 86400000).toISOString(),
      cashier_id: null,
      cashier_name: "Carlos M.",
      customer_label: c.name,
      total: "89500.00",
      refunded_amount: "0.00",
      methods: ["QR"],
      status: "COBRADA",
      fiscal_status: null,
    },
  ];
}

export function mockListCustomers(query?: CustomerQuery): Customer[] {
  const q = query?.q?.trim().toLowerCase() || "";
  const page = Math.max(1, query?.page ?? 1);
  const pageSize = query?.page_size || query?.limit || 50;
  const offset = (page - 1) * pageSize;

  let filtered = mockCustomers;
  if (query?.status && query.status !== "TODOS") {
    if (query.status === "ARCHIVADO") {
      filtered = filtered.filter((c) => Boolean(c.archived_at));
    } else if (query.status === "EN_MORA") {
      filtered = filtered.filter((c) => !c.archived_at && (c.max_days_overdue ?? 0) > 0);
    } else if (query.status === "AL_DIA") {
      filtered = filtered.filter((c) => !c.archived_at && (c.max_days_overdue ?? 0) === 0);
    }
  } else {
    filtered = filtered.filter((c) => !c.archived_at);
  }

  if (q) {
    filtered = filtered.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        (c.doc_number && c.doc_number.toLowerCase().startsWith(q)) ||
        (c.phone && c.phone.startsWith(q)),
    );
  }

  return structuredClone(filtered.slice(offset, offset + pageSize));
}

export function mockGetCustomer(id: string): CustomerDetail {
  const found = mockCustomers.find((c) => c.id === id);
  if (!found) throw new ApiError(404, "Cliente no encontrado");

  const balance = found.balance ?? "0.00";
  const creditLimit = Number(found.credit_limit ?? 0);
  const creditAvailable = String(Math.max(0, creditLimit - Number(balance)).toFixed(2));

  const detail: CustomerDetail = {
    ...found,
    balance,
    credit_available: creditAvailable,
    wallet_balance: "0.00",
    missing_fiscal: computeMissingFiscal(found),
  };
  return structuredClone(detail);
}

export function mockCreateCustomer(input: CreateCustomerInput): CustomerDetail {
  const name = input.name.trim().replace(/\s+/g, " ");
  if (!name || name.length > 120) {
    throw new ApiError(422, "El nombre debe tener entre 1 y 120 caracteres", "CUSTOMER_INVALID");
  }

  // B3: Validación de permiso credit:override al crear con cupo o tolerancia distinta a 30
  const creditLimitNum = Number(input.credit_limit ?? 0);
  const blockDays = input.credit_block_days ?? 30;
  if (creditLimitNum !== 0 || blockDays !== 30) {
    if (!hasCreditOverride()) {
      throw new ApiError(403, "No tenés permiso para asignar cupo de crédito", "FORBIDDEN");
    }
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

  // B5: Cálculo de DV y person_type
  let person_type: "NATURAL" | "JURIDICA" = "NATURAL";
  if (doc_type === "NIT") {
    const expectedDv = nitDv(doc_number ?? "");
    if (dv && dv !== expectedDv) {
      throw new ApiError(422, "Dígito de verificación no coincide con el NIT", "NIT_DV_INVALID");
    }
    dv = expectedDv;
    if (input.person_type) {
      if (input.person_type !== "NATURAL" && input.person_type !== "JURIDICA") {
        throw new ApiError(422, "Tipo de persona inválido", "CUSTOMER_INVALID");
      }
      person_type = input.person_type;
    } else {
      person_type = defaultPersonType("NIT", doc_number);
    }
  } else {
    dv = null;
    person_type = "NATURAL";
    if (input.person_type === "JURIDICA") {
      throw new ApiError(422, "Solo un cliente con NIT puede ser persona jurídica", "CUSTOMER_INVALID");
    }
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
      (c) => !c.archived_at && c.doc_type === doc_type && c.doc_number?.toUpperCase() === doc_number?.toUpperCase(),
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
    price_tier_id: input.price_tier_id ?? null,
    person_type,
    tax_regime: input.tax_regime ?? "NO_RESPONSABLE",
    fiscal_responsibilities: input.fiscal_responsibilities ?? ["R-99-PN"],
    ciiu: input.ciiu ?? null,
    withholding_agent: input.withholding_agent ?? false,
    city_code: input.city_code ?? null,
    billing_address: input.billing_address ?? null,
    shipping_address: input.shipping_address ?? null,
    pinned_note: input.pinned_note ?? null,
    credit_limit: input.credit_limit ?? "0.00",
    credit_block_days: blockDays,
    archived_at: null,
    balance: "0.00",
    max_days_overdue: 0,
  };
  mockCustomers.push(newCust);
  return mockGetCustomer(newCust.id);
}

export function mockUpdateCustomer(id: string, input: CreateCustomerInput): CustomerDetail {
  const existing = mockCustomers.find((c) => c.id === id);
  if (!existing) throw new ApiError(404, "Cliente no encontrado");

  // B3: Validación de permiso si cambia credit_limit o credit_block_days
  if (
    (input.credit_limit !== undefined && input.credit_limit !== existing.credit_limit) ||
    (input.credit_block_days !== undefined && input.credit_block_days !== existing.credit_block_days)
  ) {
    if (!hasCreditOverride()) {
      throw new ApiError(403, "No tenés permiso para modificar el cupo de crédito", "FORBIDDEN");
    }
  }

  if (input.name !== undefined) existing.name = input.name.trim();
  if (input.phone !== undefined) existing.phone = input.phone?.trim() || null;
  if (input.email !== undefined) existing.email = input.email?.trim().toLowerCase() || null;
  if (input.tax_regime !== undefined) existing.tax_regime = input.tax_regime;
  if (input.fiscal_responsibilities !== undefined) existing.fiscal_responsibilities = input.fiscal_responsibilities;
  if (input.ciiu !== undefined) existing.ciiu = input.ciiu?.trim() || null;
  if (input.withholding_agent !== undefined) existing.withholding_agent = input.withholding_agent;
  if (input.city_code !== undefined) existing.city_code = input.city_code?.trim() || null;
  if (input.billing_address !== undefined) existing.billing_address = input.billing_address?.trim() || null;
  if (input.shipping_address !== undefined) existing.shipping_address = input.shipping_address?.trim() || null;
  if (input.pinned_note !== undefined) existing.pinned_note = input.pinned_note?.trim() || null;
  if (input.credit_limit !== undefined) existing.credit_limit = input.credit_limit;
  if (input.credit_block_days !== undefined) existing.credit_block_days = input.credit_block_days;
  if (input.person_type !== undefined && input.person_type !== null) existing.person_type = input.person_type;

  return mockGetCustomer(id);
}

export function mockUpsertCustomer(input: CreateCustomerInput): CustomerUpsertResponse {
  // Buscar primero por email (no archivado)
  const normEmail = input.email?.trim().toLowerCase();
  let existing = normEmail
    ? mockCustomers.find((c) => !c.archived_at && c.email?.toLowerCase() === normEmail)
    : undefined;

  // Si no se encuentra por email, buscar por (doc_type, doc_number)
  if (!existing && input.doc_type && input.doc_number) {
    const normDocNum = input.doc_number.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
    existing = mockCustomers.find(
      (c) => !c.archived_at && c.doc_type === input.doc_type && c.doc_number === normDocNum,
    );
  }

  if (existing) {
    let changed = false;
    const exHasDoc = Boolean(existing.doc_type && existing.doc_number);

    // B6: Si el existente no tiene documento y el input sí lo trae:
    if (!exHasDoc && input.doc_type && input.doc_number) {
      const cleanDocNum = input.doc_number.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
      const conflict = mockCustomers.find(
        (c) => c.id !== existing?.id && !c.archived_at && c.doc_type === input.doc_type && c.doc_number === cleanDocNum,
      );
      if (conflict) {
        throw new ApiError(409, "Ya existe un cliente registrado con este documento", "CUSTOMER_DUPLICATED", {
          id: conflict.id,
        });
      }

      existing.doc_type = input.doc_type;
      existing.doc_number = cleanDocNum;
      if (input.doc_type === "NIT") {
        existing.dv = input.dv ? input.dv.trim() : nitDv(cleanDocNum);
        existing.person_type = input.person_type || defaultPersonType("NIT", cleanDocNum);
      } else {
        existing.dv = null;
        existing.person_type = "NATURAL";
      }
      changed = true;
    }

    // Llenar campos que estén vacíos o nulos en existing
    if (!existing.phone && input.phone?.trim()) {
      existing.phone = input.phone.trim();
      changed = true;
    }
    if (!existing.email && input.email?.trim()) {
      existing.email = input.email.trim().toLowerCase();
      changed = true;
    }
    if (!existing.city_code && input.city_code?.trim()) {
      existing.city_code = input.city_code.trim();
      changed = true;
    }
    if (!existing.billing_address && input.billing_address?.trim()) {
      existing.billing_address = input.billing_address.trim();
      changed = true;
    }
    if (!existing.shipping_address && input.shipping_address?.trim()) {
      existing.shipping_address = input.shipping_address.trim();
      changed = true;
    }
    if (!existing.ciiu && input.ciiu?.trim()) {
      existing.ciiu = input.ciiu.trim();
      changed = true;
    }
    if (!existing.pinned_note && input.pinned_note?.trim()) {
      existing.pinned_note = input.pinned_note.trim();
      changed = true;
    }

    return {
      customer: mockGetCustomer(existing.id),
      result: changed ? "UPDATED" : "UNCHANGED",
    };
  }

  const created = mockCreateCustomer(input);
  return {
    customer: created,
    result: "CREATED",
  };
}

export function mockArchiveCustomer(id: string): void {
  const found = mockCustomers.find((c) => c.id === id);
  if (!found) throw new ApiError(404, "Cliente no encontrado");

  // Validación: si tiene saldo pendiente o saldo a favor, devolver 409 CUSTOMER_HAS_BALANCE
  if (Number(found.balance ?? 0) > 0) {
    throw new ApiError(
      409,
      "No se puede archivar un cliente con saldo pendiente",
      "CUSTOMER_HAS_BALANCE",
    );
  }

  found.archived_at = new Date().toISOString();
}

export function mockUnarchiveCustomer(id: string): void {
  const found = mockCustomers.find((c) => c.id === id);
  if (!found) throw new ApiError(404, "Cliente no encontrado");

  // B7: Validar si otro cliente activo ya usa su documento
  if (found.doc_type && found.doc_number) {
    const conflict = mockCustomers.find(
      (c) => c.id !== id && !c.archived_at && c.doc_type === found.doc_type && c.doc_number === found.doc_number,
    );
    if (conflict) {
      throw new ApiError(
        409,
        "Ya existe un cliente activo con este documento",
        "CUSTOMER_DUPLICATED",
        { id: conflict.id },
      );
    }
  }

  found.archived_at = null;
}

export function mockGetCustomerPurchases(id: string, page = 1): SaleHistoryPage {
  void page;
  const found = mockCustomers.find((c) => c.id === id);
  if (!found) throw new ApiError(404, "Cliente no encontrado");

  const items = generatePurchasesForCustomer(found);
  const totalAmount = items.reduce((acc, it) => acc + Number(it.total), 0).toFixed(2);

  return {
    items,
    total_count: items.length,
    totals: { count: items.length, amount: totalAmount },
  };
}


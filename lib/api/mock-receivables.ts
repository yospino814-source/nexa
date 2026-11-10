import { ApiError } from "./client";
import { mockCustomers } from "./mock-customers";
import type {
  CreditFrequency,
  CustomerStatementResponse,
  PayReceivableInput,
  Receivable,
  ReceivableDetail,
  ReceivablePaymentAllocation,
  ReceivablePaymentResponse,
  ReceivablesFilter,
  ReceivablesSummaryResponse,
  StatementEntry,
  TopDebtor,
} from "./types";

interface CashContext {
  isOpen: () => boolean;
  recordMovement: (amount: number, reason: string) => void;
}

let cashContext: CashContext = {
  isOpen: () => true,
  recordMovement: () => {},
};

export function setMockReceivablesCashContext(ctx: CashContext) {
  cashContext = ctx;
}

const recordedPayments = new Map<
  string,
  {
    request: PayReceivableInput;
    response: ReceivablePaymentResponse;
  }
>();

export const mockReceivables: ReceivableDetail[] = [
  {
    id: "rec-00000000-0000-0000-0000-000000000001",
    branch_id: "b0000000-0000-0000-0000-000000000001",
    branch_name: "Sede Principal",
    customer_id: "c0000000-0000-0000-0000-000000000001",
    customer_name: "Distribuidora El Triunfo SAS",
    source: "VENTA",
    order_id: "o0000000-0000-0000-0000-000000000001",
    receipt_number: "POS-000012",
    principal: "150000.00",
    amount_total: "150000.00",
    amount_paid: "0.00",
    balance: "150000.00",
    status: "ABIERTA",
    due_date: new Date(Date.now() - 10 * 86400000).toISOString().slice(0, 10),
    days_overdue: 10,
    created_at: new Date(Date.now() - 40 * 86400000).toISOString(),
    installments: [
      {
        n: 1,
        due_date: new Date(Date.now() - 10 * 86400000).toISOString().slice(0, 10),
        amount: "50000.00",
        amount_paid: "0.00",
        paid: false,
        days_overdue: 10,
      },
      {
        n: 2,
        due_date: new Date(Date.now() + 20 * 86400000).toISOString().slice(0, 10),
        amount: "50000.00",
        amount_paid: "0.00",
        paid: false,
        days_overdue: 0,
      },
      {
        n: 3,
        due_date: new Date(Date.now() + 50 * 86400000).toISOString().slice(0, 10),
        amount: "50000.00",
        amount_paid: "0.00",
        paid: false,
        days_overdue: 0,
      },
    ],
    payments: [],
  },
];

export function mockCreateReceivableFromSale(params: {
  orderId: string;
  receiptNumber: string | null;
  customerId: string;
  customerName: string;
  branchId: string;
  creditAmount: number;
  creditPlan?: {
    installments: number;
    frequency: CreditFrequency;
    first_due_date: string;
  } | null;
}): ReceivableDetail {
  const n = Math.max(1, params.creditPlan?.installments || 1);
  const totalCents = Math.round(params.creditAmount * 100);
  const eachCents = Math.floor(totalCents / n);
  let accumulatedCents = 0;
  const installments = [];

  const firstDue = params.creditPlan?.first_due_date || new Date(Date.now() + 30 * 86400000).toISOString().slice(0, 10);
  const freq = params.creditPlan?.frequency || "MENSUAL";

  for (let k = 1; k <= n; k++) {
    let amountCents: number;
    if (k < n) {
      amountCents = eachCents;
      accumulatedCents += amountCents;
    } else {
      amountCents = totalCents - accumulatedCents;
    }
    const d = new Date(firstDue);
    if (freq === "SEMANAL") d.setUTCDate(d.getUTCDate() + (k - 1) * 7);
    else if (freq === "QUINCENAL") d.setUTCDate(d.getUTCDate() + (k - 1) * 15);
    else d.setUTCMonth(d.getUTCMonth() + (k - 1));

    installments.push({
      n: k,
      due_date: d.toISOString().slice(0, 10),
      amount: (amountCents / 100).toFixed(2),
      amount_paid: "0.00",
      paid: false,
      days_overdue: 0,
    });
  }

  const rec: ReceivableDetail = {
    id: `rec-${crypto.randomUUID()}`,
    branch_id: params.branchId,
    branch_name: "Sede Principal",
    customer_id: params.customerId,
    customer_name: params.customerName,
    source: "VENTA",
    order_id: params.orderId,
    receipt_number: params.receiptNumber,
    principal: params.creditAmount.toFixed(2),
    amount_total: params.creditAmount.toFixed(2),
    amount_paid: "0.00",
    balance: params.creditAmount.toFixed(2),
    status: "ABIERTA",
    due_date: installments[0]?.due_date || firstDue,
    days_overdue: 0,
    created_at: new Date().toISOString(),
    installments,
    payments: [],
  };

  mockReceivables.unshift(rec);
  return rec;
}

export function mockReceivablesList(filter?: ReceivablesFilter): Receivable[] {
  let list = [...mockReceivables];

  if (filter?.customer_id) {
    list = list.filter((r) => r.customer_id === filter.customer_id);
  }

  if (filter?.status) {
    const s = filter.status.toUpperCase();
    list = list.filter((r) => r.status.toUpperCase() === s);
  } else {
    list = list.filter((r) => r.status === "ABIERTA" || r.status === "PARCIAL");
  }

  if (filter?.overdue) {
    list = list.filter((r) => r.days_overdue > 0);
  }

  list.sort((a, b) => {
    if (b.days_overdue !== a.days_overdue) {
      return b.days_overdue - a.days_overdue;
    }
    return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
  });

  return list.map((r) => {
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { installments, payments, ...rest } = r;
    return rest;
  });
}

export function mockReceivableGet(id: string): ReceivableDetail {
  const rec = mockReceivables.find((r) => r.id === id);
  if (!rec) {
    throw new ApiError(404, "Cuenta por cobrar no encontrada", "NOT_FOUND");
  }
  return structuredClone(rec);
}

export function mockReceivablePay(id: string, body: PayReceivableInput): ReceivablePaymentResponse {
  // 3. Idempotencia por id
  const existing = recordedPayments.get(body.id);
  if (existing) {
    if (
      existing.request.id === body.id &&
      existing.request.amount === body.amount &&
      existing.request.method === body.method &&
      existing.request.reference === body.reference
    ) {
      return existing.response;
    }
    throw new ApiError(409, "El id del abono ya fue utilizado con una solicitud diferente", "IDEMPOTENCY_CONFLICT");
  }

  // 4. method = "SALDO": 422 PAYMENT_METHOD_NOT_AVAILABLE
  const method = body.method.trim().toUpperCase();
  if (method === "SALDO") {
    throw new ApiError(422, "Ese método no está disponible para abonos.", "PAYMENT_METHOD_NOT_AVAILABLE");
  }

  // 5. Bloquea la cuenta
  const rec = mockReceivables.find((r) => r.id === id);
  if (!rec) {
    throw new ApiError(404, "Cuenta por cobrar no encontrada", "NOT_FOUND");
  }

  if (rec.status !== "ABIERTA" && rec.status !== "PARCIAL") {
    throw new ApiError(409, "La cuenta ya está cerrada.", "RECEIVABLE_CLOSED");
  }

  // 6. amount = money(amount). Si es <= 0 o > saldo: 422 PAYMENT_EXCEEDS_BALANCE
  const amount = Number(body.amount);
  const balance = Number(rec.balance);
  if (amount <= 0 || amount > balance) {
    throw new ApiError(422, "El abono supera el saldo de la cuenta.", "PAYMENT_EXCEEDS_BALANCE", {
      balance: rec.balance,
    });
  }

  // 7. DATAFONO, QR y TRANSFERENCIA exigen reference (3 a 60 caracteres)
  const refTrimmed = body.reference?.trim() || "";
  if (["DATAFONO", "QR", "TRANSFERENCIA"].includes(method)) {
    if (refTrimmed.length < 3 || refTrimmed.length > 60) {
      throw new ApiError(422, "Se requiere una referencia válida (de 3 a 60 caracteres).", "PAYMENT_REFERENCE_REQUIRED");
    }
  }

  // 11. Efectivo: exige turno abierto
  if (method === "EFECTIVO") {
    if (!cashContext.isOpen()) {
      throw new ApiError(409, "Para recibir un abono en efectivo, abrí la caja.", "PAYMENT_NEEDS_OPEN_SESSION");
    }
    cashContext.recordMovement(amount, `Abono ${rec.customer_name}`);
  }

  // 8. Aplicación a cuotas (n asc)
  let remaining = amount;
  const allocation: ReceivablePaymentAllocation[] = [];

  for (const inst of rec.installments) {
    if (remaining <= 0) break;
    const pending = Number(inst.amount) - Number(inst.amount_paid);
    if (pending > 0) {
      const applied = Math.min(remaining, pending);
      inst.amount_paid = (Number(inst.amount_paid) + applied).toFixed(2);
      inst.paid = Number(inst.amount_paid) >= Number(inst.amount);
      if (inst.paid) {
        inst.days_overdue = 0;
      }
      allocation.push({
        n: inst.n,
        amount: applied.toFixed(2),
      });
      remaining -= applied;
    }
  }

  // 9. amount_paid += amount
  const balanceBefore = balance;
  rec.amount_paid = (Number(rec.amount_paid) + amount).toFixed(2);
  const balanceAfter = Math.max(0, Number(rec.amount_total) - Number(rec.amount_paid));
  rec.balance = balanceAfter.toFixed(2);
  rec.status = balanceAfter === 0 ? "PAGADA" : "PARCIAL";

  // 10. due_date pasa a ser el vencimiento de la primera cuota que queda con saldo
  const firstPending = rec.installments.find((inst) => Number(inst.amount_paid) < Number(inst.amount));
  let nextDue: { n: number; due_date: string; amount_pending: string } | null = null;
  if (firstPending) {
    rec.due_date = firstPending.due_date;
    nextDue = {
      n: firstPending.n,
      due_date: firstPending.due_date,
      amount_pending: (Number(firstPending.amount) - Number(firstPending.amount_paid)).toFixed(2),
    };
    const today = new Date().toISOString().slice(0, 10);
    rec.days_overdue = firstPending.due_date < today
      ? Math.max(0, Math.floor((Date.now() - new Date(firstPending.due_date).getTime()) / 86400000))
      : 0;
  } else {
    nextDue = null;
    rec.days_overdue = 0;
  }

  // Descontar saldo al cliente en mockCustomers
  const cust = mockCustomers.find((c) => c.id === rec.customer_id);
  if (cust) {
    const newBal = Math.max(0, Number(cust.balance || 0) - amount);
    cust.balance = newBal.toFixed(2);
    if (newBal === 0) {
      cust.max_days_overdue = 0;
    } else {
      // Recalcular max_days_overdue de las cuentas del cliente
      const custRecs = mockReceivables.filter(
        (r) => r.customer_id === cust.id && (r.status === "ABIERTA" || r.status === "PARCIAL"),
      );
      cust.max_days_overdue = custRecs.reduce((max, r) => Math.max(max, r.days_overdue), 0);
    }
  }

  const paymentRecord = {
    id: body.id,
    amount: amount.toFixed(2),
    method,
    reference: refTrimmed || null,
    allocation,
    created_at: new Date().toISOString(),
    created_by_name: "Cajero Principal",
  };
  rec.payments.push(paymentRecord);

  const response: ReceivablePaymentResponse = {
    id: body.id,
    receivable_id: rec.id,
    amount: amount.toFixed(2),
    method,
    reference: refTrimmed || null,
    allocation,
    created_at: paymentRecord.created_at,
    balance_before: balanceBefore.toFixed(2),
    balance_after: rec.balance,
    next_due: nextDue,
  };

  recordedPayments.set(body.id, {
    request: body,
    response,
  });

  return response;
}

function formatMockSaldo(amount: number): string {
  const rounded = Math.round(amount * 100) / 100;
  const isWhole = rounded % 1 === 0;
  const intPart = Math.floor(rounded);
  const withDots = intPart.toLocaleString("es-CO");
  if (isWhole) {
    return withDots;
  }
  const cents = Math.round((rounded - intPart) * 100);
  return `${withDots},${cents.toString().padStart(2, "0")}`;
}

export async function mockReceivablesSummary(branchId?: string): Promise<ReceivablesSummaryResponse> {
  const filtered = branchId
    ? mockReceivables.filter((r) => r.branch_id === branchId)
    : mockReceivables;

  const openAccounts = filtered.filter(
    (r) => r.status === "ABIERTA" || r.status === "PARCIAL",
  );

  let total_balance = 0;
  let overdue_balance = 0;
  let al_dia = 0;
  let d1_30 = 0;
  let d31_60 = 0;
  let d61_90 = 0;
  let d91_120 = 0;
  let d120_mas = 0;

  for (const acc of openAccounts) {
    const bal = Number(acc.balance || 0);
    total_balance += bal;
    const days = acc.days_overdue || 0;
    if (days > 0) {
      overdue_balance += bal;
    }
    if (days === 0) al_dia += bal;
    else if (days <= 30) d1_30 += bal;
    else if (days <= 60) d31_60 += bal;
    else if (days <= 90) d61_90 += bal;
    else if (days <= 120) d91_120 += bal;
    else d120_mas += bal;
  }

  // due_this_week: cuotas o cuentas sin cuotas que vencen entre hoy y hoy + 7 días
  const todayStr = new Date().toISOString().slice(0, 10);
  const nextWeekDate = new Date();
  nextWeekDate.setDate(nextWeekDate.getDate() + 7);
  const nextWeekStr = nextWeekDate.toISOString().slice(0, 10);

  let due_this_week = 0;
  for (const acc of openAccounts) {
    if (acc.installments && acc.installments.length > 0) {
      for (const inst of acc.installments) {
        if (!inst.paid && inst.due_date >= todayStr && inst.due_date <= nextWeekStr) {
          due_this_week += Number(inst.amount) - Number(inst.amount_paid);
        }
      }
    } else {
      if (acc.due_date >= todayStr && acc.due_date <= nextWeekStr) {
        due_this_week += Number(acc.balance);
      }
    }
  }

  // collected_this_month: pagos en el mes actual
  const currentYearMonth = todayStr.slice(0, 7);
  let collected_this_month = 0;
  for (const rec of filtered) {
    for (const p of rec.payments) {
      if (p.created_at.slice(0, 7) === currentYearMonth) {
        collected_this_month += Number(p.amount);
      }
    }
  }

  // top_debtors: agrupado por cliente, top 10 desc
  const debtorsMap = new Map<string, { customer_id: string; name: string; balance: number; max_days_overdue: number }>();
  for (const acc of openAccounts) {
    const prev = debtorsMap.get(acc.customer_id) || {
      customer_id: acc.customer_id,
      name: acc.customer_name,
      balance: 0,
      max_days_overdue: 0,
    };
    prev.balance += Number(acc.balance);
    prev.max_days_overdue = Math.max(prev.max_days_overdue, acc.days_overdue);
    debtorsMap.set(acc.customer_id, prev);
  }

  const top_debtors: TopDebtor[] = Array.from(debtorsMap.values())
    .sort((a, b) => b.balance - a.balance)
    .slice(0, 10)
    .map((d) => ({
      customer_id: d.customer_id,
      name: d.name,
      balance: d.balance.toFixed(2),
      max_days_overdue: d.max_days_overdue,
    }));

  return {
    total_balance: total_balance.toFixed(2),
    overdue_balance: overdue_balance.toFixed(2),
    due_this_week: due_this_week.toFixed(2),
    collected_this_month: collected_this_month.toFixed(2),
    aging: {
      al_dia: al_dia.toFixed(2),
      d1_30: d1_30.toFixed(2),
      d31_60: d31_60.toFixed(2),
      d61_90: d61_90.toFixed(2),
      d91_120: d91_120.toFixed(2),
      d120_mas: d120_mas.toFixed(2),
    },
    top_debtors,
  };
}

export async function mockCustomerStatement(
  customerId: string,
  from: string,
  to: string,
): Promise<CustomerStatementResponse> {
  const customer = mockCustomers.find((c) => c.id === customerId);
  if (!customer) {
    throw new ApiError(404, "NOT_FOUND", "Cliente no encontrado");
  }

  type RawItem = {
    date: string;
    time: string;
    type: "CARGO" | "ABONO" | "DEVOLUCION";
    reference: string;
    debit: number;
    credit: number;
  };

  const rawList: RawItem[] = [];

  // 1. CARGOS desde mockReceivables
  for (const rec of mockReceivables) {
    if (rec.customer_id === customerId) {
      rawList.push({
        date: rec.created_at.slice(0, 10),
        time: rec.created_at,
        type: "CARGO",
        reference: rec.receipt_number || "Venta",
        debit: Number(rec.amount_total),
        credit: 0,
      });

      // 2. ABONOS desde rec.payments
      for (const p of rec.payments) {
        rawList.push({
          date: p.created_at.slice(0, 10),
          time: p.created_at,
          type: "ABONO",
          reference: `Abono ${p.method}`,
          debit: 0,
          credit: Number(p.amount),
        });
      }
    }
  }

  const typeWeight = (t: string) => (t === "CARGO" ? 1 : t === "ABONO" ? 2 : 3);
  rawList.sort((a, b) => {
    if (a.date !== b.date) return a.date.localeCompare(b.date);
    if (a.time !== b.time) return a.time.localeCompare(b.time);
    return typeWeight(a.type) - typeWeight(b.type);
  });

  let opening = 0;
  const entries: StatementEntry[] = [];
  let running = 0;

  for (const item of rawList) {
    if (item.date < from) {
      opening += item.debit - item.credit;
    } else if (item.date <= to) {
      if (entries.length === 0) {
        running = opening;
      }
      running += item.debit - item.credit;
      entries.push({
        date: item.date,
        type: item.type,
        reference: item.reference,
        debit: item.debit.toFixed(2),
        credit: item.credit.toFixed(2),
        balance: running.toFixed(2),
      });
    }
  }

  const closing = entries.length === 0 ? opening : running;

  return {
    opening_balance: opening.toFixed(2),
    entries,
    closing_balance: closing.toFixed(2),
  };
}

export async function mockReceivableWhatsappLink(receivableId: string): Promise<{ url: string }> {
  const rec = mockReceivables.find((r) => r.id === receivableId);
  if (!rec) {
    throw new ApiError(404, "NOT_FOUND", "Cuenta por cobrar no encontrada");
  }

  const cust = mockCustomers.find((c) => c.id === rec.customer_id);
  const rawPhone = cust?.phone || "";
  const digits = rawPhone.replace(/\D/g, "");
  const cell = digits.length === 12 && digits.startsWith("57") ? digits.slice(2) : digits;

  if (cell.length !== 10 || !cell.startsWith("3")) {
    throw new ApiError(422, "CUSTOMER_NO_PHONE", "El cliente no tiene un celular válido para WhatsApp.");
  }

  const balance = Number(rec.balance);
  const saldoStr = formatMockSaldo(balance);
  const parts = rec.due_date.slice(0, 10).split("-");
  const dueDMY = parts.length === 3 ? `${parts[2]}/${parts[1]}/${parts[0]}` : rec.due_date;
  const tenantName = "Nexa POS";
  const branchName = rec.branch_name || "Sede Principal";

  const text =
    rec.days_overdue > 0
      ? `Hola ${rec.customer_name}, te escribimos de ${tenantName}. Tenés un saldo de $${saldoStr} vencido desde el ${dueDMY}. Podés pagar en ${branchName}. ¡Gracias!`
      : `Hola ${rec.customer_name}, te escribimos de ${tenantName}. Tenés un saldo de $${saldoStr} que vence el ${dueDMY}. Podés pagar en ${branchName}. ¡Gracias!`;

  return {
    url: `https://wa.me/57${cell}?text=${encodeURIComponent(text)}`,
  };
}


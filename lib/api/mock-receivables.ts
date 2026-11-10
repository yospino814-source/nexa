import { ApiError } from "./client";
import { mockCustomers } from "./mock-customers";
import type {
  CreditFrequency,
  PayReceivableInput,
  Receivable,
  ReceivableDetail,
  ReceivablePaymentAllocation,
  ReceivablePaymentResponse,
  ReceivablesFilter,
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

"use client";

import { useEffect, useMemo, useState } from "react";
import { Banknote, Check, Loader2, PlusCircle, Printer, ShoppingBag, X } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  ApiError,
  friendlyError,
  type CheckoutRequest,
  type CheckoutResponse,
  type CreditFrequency,
  type CreditPlan,
  type CustomerDetail,
  type PaymentMethod,
  type QuoteResponse,
  type SchedulePreview,
  type Waiter,
} from "@/lib/api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP } from "@/lib/format";
import { toSaleItems, useCart, type CartLine } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

import { PAYMENT_METHODS } from "./payment-methods";
import { PrintTicket, ticketFromSale, withFiscal, type TicketMeta } from "./print-ticket";
import { FiscalStatusPanel, useOrderFiscal } from "@/components/fiscal/fiscal-status";
import { useBusinessInfo } from "@/lib/api/use-business";

/**
 * Efectivo rápido (F1.14): el total exacto y ceil(total / d) * d para
 * d = 1.000, 5.000, 10.000, 20.000 y 50.000, sin repetidos ni menores al total,
 * de menor a mayor.
 */
export function quickAmounts(total: number): number[] {
  if (total <= 0) return [0];
  const rounded = [1_000, 5_000, 10_000, 20_000, 50_000]
    .map((d) => Math.ceil(total / d) * d)
    .filter((amount) => amount > total);
  return [total, ...[...new Set(rounded)].sort((a, b) => a - b)];
}

function formatDecimalCOP(amount: number): string {
  return new Intl.NumberFormat("es-CO", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(amount);
}

type PaymentLine = {
  id: string;
  method: PaymentMethod;
  amount: number;
};

type Receipt = {
  response: CheckoutResponse;
  lines: CartLine[];
  received: number;
  method: PaymentMethod;
  creditPlan?: CreditPlan | null;
};

/** Pantalla 14: modal de cobro rápido + comprobante + crédito (F3.2). */
export function CheckoutDialog({
  open,
  onOpenChange,
  lines,
  initialMethod,
  branchId,
  tipsEnabled = false,
  suggestedTipPct = 0,
  ticketWidth = 80,
  registerName = null,
  onCompleted,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  lines: CartLine[];
  initialMethod: PaymentMethod;
  branchId: string;
  /** Ajuste `tips.enabled` de la sucursal (F1.10). */
  tipsEnabled?: boolean;
  /** Ajuste `tips.suggested_pct` (porcentaje sobre el subtotal). */
  suggestedTipPct?: number;
  /** Ajuste `print.ticket_width_mm` (F1.15). */
  ticketWidth?: number;
  /** Caja del turno abierto, para la tirilla. */
  registerName?: string | null;
  onCompleted: () => void;
}) {
  const [payments, setPayments] = useState<PaymentLine[]>([]);
  const [received, setReceived] = useState<number | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<Receipt | null>(null);

  // Clave de idempotencia: la del carrito. Se reusa en cada reintento (y al
  // retomar una venta en espera) para que nada se cobre dos veces.
  const saleId = useCart((s) => s.saleId);
  const quote = useCart((s) => s.quote);
  const setQuote = useCart((s) => s.setQuote);
  const cashierName = useSession((s) => s.name);
  const customer = useCart((s) => s.customer);
  const customerAlias = useCart((s) => s.customerAlias);
  const couponCode = useCart((s) => s.couponCode);

  const can = useSession((s) => s.can);
  const canCreditSell = can("credit:sell");
  const canCreditOverride = can("credit:override");
  const hasCustomer = Boolean(customer?.id);

  const [customerDetail, setCustomerDetail] = useState<CustomerDetail | null>(null);

  // Propina y mesero
  const [tipMode, setTipMode] = useState<"NONE" | "SUGGESTED" | "CUSTOM">("NONE");
  const [customTip, setCustomTip] = useState(0);
  const [waiterId, setWaiterId] = useState<string | null>(null);
  const [waiters, setWaiters] = useState<Waiter[]>([]);

  const defaultDueDate = useMemo(() => {
    const d = new Date();
    d.setDate(d.getDate() + 30);
    return d.toISOString().slice(0, 10);
  }, []);

  // En cuotas
  const [hasInstallments, setHasInstallments] = useState(false);
  const [installments, setInstallments] = useState(1);
  const [frequency, setFrequency] = useState<CreditFrequency>("MENSUAL");
  const [firstDueDate, setFirstDueDate] = useState(defaultDueDate);
  const [schedulePreview, setSchedulePreview] = useState<SchedulePreview | null>(null);

  // Autorización (override)
  const [creditErrorData, setCreditErrorData] = useState<{ code: string; data?: unknown } | null>(null);
  const [showOverrideForm, setShowOverrideForm] = useState(false);
  const [overrideReason, setOverrideReason] = useState("");
  const [overrideError, setOverrideError] = useState<string | null>(null);

  const todayStr = useMemo(() => new Date().toISOString().slice(0, 10), []);

  useEffect(() => {
    if (!open || !tipsEnabled) return;
    let alive = true;
    api.users
      .waiters(branchId)
      .then((list) => alive && setWaiters(list))
      .catch(() => alive && setWaiters([]));
    return () => {
      alive = false;
    };
  }, [open, tipsEnabled, branchId]);

  // Cargar cliente para datos de cupo
  useEffect(() => {
    if (!open || !customer?.id) return;
    let alive = true;
    api.customers
      .get(customer.id)
      .then((c) => {
        if (alive) setCustomerDetail(c);
      })
      .catch(() => {
        if (alive) setCustomerDetail(null);
      });
    return () => {
      alive = false;
    };
  }, [open, customer?.id]);

  // Totales de la última cotización
  const saleTotal = Number(quote?.total ?? 0);
  const subtotal = Number(quote?.subtotal ?? 0);
  const units = lines.reduce((s, l) => s + l.qty, 0);
  const suggestedTip = Math.round((subtotal * suggestedTipPct) / 100);
  const tip = !tipsEnabled
    ? 0
    : tipMode === "SUGGESTED"
      ? suggestedTip
      : tipMode === "CUSTOM"
        ? customTip
        : 0;
  const total = saleTotal + tip;

  // Al abrir el modal se reinicia con el método elegido en el carrito.
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      const allowedInit =
        initialMethod === "CREDITO" && (!hasCustomer || !canCreditSell) ? "EFECTIVO" : initialMethod;
      setPayments([{ id: crypto.randomUUID(), method: allowedInit, amount: total }]);
      setReceived(null);
      setError(null);
      setReceipt(null);
      setTipMode("NONE");
      setCustomTip(0);
      setWaiterId(null);
      setHasInstallments(false);
      setInstallments(1);
      setFrequency("MENSUAL");
      setFirstDueDate(defaultDueDate);
      setSchedulePreview(null);
      setCreditErrorData(null);
      setShowOverrideForm(false);
      setOverrideReason("");
      setOverrideError(null);
    }
  }

  // Si solo hay una línea y el total cambia por propina/cotización, sincronizar
  const [lastTotal, setLastTotal] = useState(total);
  if (total !== lastTotal) {
    setLastTotal(total);
    if (payments.length === 1 && payments[0].amount !== total) {
      setPayments([{ ...payments[0], amount: total }]);
    }
  }

  // Recalcular vista previa de cronograma (400 ms debounce)
  useEffect(() => {
    const creditLine = payments.find((p) => p.method === "CREDITO");
    if (!hasInstallments || !creditLine || creditLine.amount <= 0) {
      return;
    }

    const timer = setTimeout(() => {
      api.credit
        .schedulePreview({
          principal: String(creditLine.amount),
          installments,
          frequency,
          first_due_date: firstDueDate,
          interest_rate_monthly: "0",
          interest_type: "SIMPLE",
        })
        .then(setSchedulePreview)
        .catch(() => setSchedulePreview(null));
    }, 400);

    return () => clearTimeout(timer);
  }, [hasInstallments, installments, frequency, firstDueDate, payments]);

  const creditLine = payments.find((p) => p.method === "CREDITO");
  const activeCustomerDetail = open && customer?.id ? customerDetail : null;
  const creditLimit = Number(activeCustomerDetail?.credit_limit ?? 0);
  const creditAvailable = Number(activeCustomerDetail?.credit_available ?? 0);
  const activeSchedulePreview = hasInstallments && creditLine && creditLine.amount > 0 ? schedulePreview : null;

  // Lista de métodos disponibles
  const availablePaymentMethods = PAYMENT_METHODS.filter((m) => {
    if (m.id === "CREDITO") {
      return hasCustomer && canCreditSell;
    }
    return true;
  });

  const coveredAmount = payments.reduce((acc, p) => acc + p.amount, 0);
  const remainingToCover = total - coveredAmount;
  const cashLine = payments.find((p) => p.method === "EFECTIVO");

  const cashAmount = cashLine ? cashLine.amount : 0;
  const cashReceived = cashLine ? (received ?? cashAmount) : 0;
  const cashChange = Math.max(0, cashReceived - cashAmount);

  const canConfirm =
    !submitting &&
    total > 0 &&
    coveredAmount === total &&
    payments.every((p) => p.amount > 0) &&
    (!cashLine || (cashReceived >= cashAmount && cashAmount > 0)) &&
    (!creditLine || (creditLimit > 0 && creditLine.amount > 0));

  function selectOrAddMethod(m: PaymentMethod) {
    setError(null);
    setCreditErrorData(null);
    setShowOverrideForm(false);

    if (payments.length === 1 && payments[0].amount === total) {
      setPayments([{ ...payments[0], method: m }]);
      return;
    }

    if (m === "CREDITO" && payments.some((p) => p.method === "CREDITO")) {
      return;
    }

    const remaining = Math.max(0, remainingToCover);
    setPayments((prev) => [...prev, { id: crypto.randomUUID(), method: m, amount: remaining }]);
  }

  function removePaymentLine(id: string) {
    if (payments.length <= 1) return;
    setPayments((prev) => prev.filter((p) => p.id !== id));
  }

  function updatePaymentAmount(id: string, newAmount: number) {
    setError(null);
    setCreditErrorData(null);
    setShowOverrideForm(false);
    setPayments((prev) =>
      prev.map((p) => (p.id === id ? { ...p, amount: Math.max(0, newAmount) } : p)),
    );
  }

  async function confirm(overrideReasonParam?: string) {
    if (!overrideReasonParam && !canConfirm) return;
    setSubmitting(true);
    setError(null);
    setOverrideError(null);

    const creditPlanPayload: CreditPlan | null =
      creditLine && hasInstallments
        ? {
            installments,
            frequency,
            first_due_date: firstDueDate,
            interest_rate_monthly: "0",
            interest_type: "SIMPLE",
          }
        : null;

    const requestBody: CheckoutRequest = {
      id: saleId,
      branch_id: branchId,
      items: toSaleItems(lines),
      payments: payments.map((p) => ({
        method: p.method,
        amount: p.method === "EFECTIVO" ? String(cashReceived) : String(p.amount),
      })),
      quoted_total: String(total),
      tip_amount: tip > 0 ? String(tip) : null,
      tip_waiter_id: tip > 0 ? waiterId : null,
      customer_id: customer?.id ?? null,
      customer_alias: customerAlias ?? null,
      coupon_code: couponCode ?? null,
      credit_plan: creditPlanPayload,
      credit_override_reason: overrideReasonParam ?? null,
    };

    try {
      const response = await api.sales.checkout(requestBody);
      const primaryMethod = payments[0]?.method ?? initialMethod;
      setReceipt({
        response,
        lines,
        received: cashLine ? cashReceived : total,
        method: primaryMethod,
        creditPlan: creditPlanPayload,
      });
      setCreditErrorData(null);
      setShowOverrideForm(false);
    } catch (e) {
      if (e instanceof ApiError) {
        if (e.code === "PRICE_CHANGED" && e.data) {
          const fresh = e.data as QuoteResponse;
          setQuote({ ...fresh, total: String(Number(fresh.total) - Number(fresh.tip_amount ?? 0)) });
        }
        let msg = friendlyError(e);
        if (
          e.code === "CREDIT_LIMIT_EXCEEDED" &&
          e.data &&
          (e.data as { available?: string | number }).available != null
        ) {
          const avail = Number((e.data as { available: string | number }).available);
          msg += ` Disponible: ${formatCOP(avail)}`;
          setCreditErrorData({ code: e.code, data: e.data });
        } else if (
          e.code === "CREDIT_BLOCKED_OVERDUE" &&
          e.data &&
          (e.data as { days?: number }).days != null
        ) {
          const days = (e.data as { days: number }).days;
          msg += ` Días de mora: ${days}`;
          setCreditErrorData({ code: e.code, data: e.data });
        } else {
          setCreditErrorData(null);
          setShowOverrideForm(false);
        }
        setError(msg);
      } else {
        setError(friendlyError(e));
        setCreditErrorData(null);
        setShowOverrideForm(false);
      }
    } finally {
      setSubmitting(false);
    }
  }

  function finish() {
    onCompleted();
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={(next) => (receipt && !next ? finish() : onOpenChange(next))}>
      <DialogContent
        className="max-h-[95vh] gap-0 overflow-y-auto rounded-[20px] p-6 shadow-pop sm:max-w-[720px] sm:p-7"
        onKeyDown={(e) => {
          if (e.key === "F10") {
            e.preventDefault();
            if (!receipt) {
              setPayments([{ id: crypto.randomUUID(), method: "EFECTIVO", amount: total }]);
              setReceived(total);
            }
            return;
          }
          if (e.key !== "Enter") return;
          e.preventDefault();
          if (receipt) finish();
          else if (!showOverrideForm) confirm();
        }}
      >
        {receipt ? (
          <ReceiptView
            receipt={receipt}
            meta={{
              cashier: cashierName,
              register: registerName,
              customer: customerLabel(customer, customerAlias),
              waiter: waiters.find((w) => w.user_id === waiterId)?.full_name ?? null,
              creditPlan: receipt.creditPlan,
            }}
            widthMm={ticketWidth}
            onNewSale={finish}
          />
        ) : (
          <div className="space-y-6">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-slate-500">Total a cobrar</p>
                <DialogTitle className="text-[40px] leading-[1.15] font-bold tracking-[-0.035em] tabular-nums">
                  {formatCOP(total)}
                </DialogTitle>
              </div>
              <p className="flex items-center gap-2 rounded-xl bg-slate-50 py-2 pr-3 pl-2 text-sm font-semibold">
                <span className="flex size-8 items-center justify-center rounded-full bg-indigo-50 text-indigo-600">
                  <ShoppingBag className="size-4" aria-hidden />
                </span>
                {units} artículos
              </p>
            </div>

            <div>
              <div className="mb-2 flex items-center justify-between">
                <p className="text-[13px] font-medium text-slate-700">
                  {payments.length > 1 ? "Agregar método" : "Medio de pago"}
                </p>
                {remainingToCover > 0 && (
                  <span className="text-xs font-bold text-amber-600">
                    Faltan {formatCOP(remainingToCover)}
                  </span>
                )}
              </div>
              <div
                className={cn(
                  "grid gap-2",
                  availablePaymentMethods.length === 4 ? "grid-cols-4" : "grid-cols-3",
                )}
              >
                {availablePaymentMethods.map(({ id, label, icon: Icon }) => {
                  const hasThisCredit = id === "CREDITO" && payments.some((p) => p.method === "CREDITO");
                  const isDisabled = hasThisCredit && payments.length > 1;
                  const isSelected = payments.some((p) => p.method === id);

                  return (
                    <button
                      key={id}
                      type="button"
                      disabled={isDisabled}
                      onClick={() => selectOrAddMethod(id)}
                      className={cn(
                        "flex h-14 items-center justify-center gap-2.5 rounded-xl text-[15px] font-semibold transition-colors",
                        isSelected
                          ? "border-[1.5px] border-indigo-600 bg-indigo-50 text-indigo-600"
                          : "border border-slate-300 bg-white text-slate-900 hover:bg-slate-50",
                        isDisabled && "cursor-not-allowed border-dashed bg-slate-50 text-slate-400 hover:bg-slate-50",
                      )}
                    >
                      <Icon className="size-5" aria-hidden />
                      <span className="hidden sm:inline">{label}</span>
                      <span className="sm:hidden">{label.split(" ")[0]}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {tipsEnabled && (
              <div className="space-y-3">
                <p className="text-xs font-semibold tracking-wider text-slate-500 uppercase">Propina</p>
                <div className="grid grid-cols-3 gap-2">
                  {(
                    [
                      ["NONE", "Sin propina"],
                      ["SUGGESTED", `Sugerida (${suggestedTipPct} %)`],
                      ["CUSTOM", "Otro valor"],
                    ] as const
                  ).map(([mode, label]) => (
                    <button
                      key={mode}
                      type="button"
                      onClick={() => setTipMode(mode)}
                      className={cn(
                        "h-11 rounded-full border text-sm font-medium",
                        tipMode === mode ? "border-brand bg-teal-50 text-brand" : "border-slate-200 hover:bg-slate-50",
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                {tipMode === "CUSTOM" && (
                  <div className="flex items-center gap-2 rounded-2xl border border-slate-200 px-4 py-2">
                    <span className="font-bold text-slate-400">$</span>
                    <input
                      type="text"
                      inputMode="numeric"
                      value={customTip ? customTip.toLocaleString("es-CO") : ""}
                      placeholder="0"
                      onChange={(e) => {
                        const digits = e.target.value.replace(/\D/g, "");
                        setCustomTip(digits ? Number(digits) : 0);
                      }}
                      className="w-full font-bold tabular-nums outline-none"
                    />
                  </div>
                )}
                {tip > 0 && (
                  <div className="flex items-center justify-between gap-3 text-sm">
                    <span className="text-slate-500">Propina {formatCOP(tip)} · Mesero</span>
                    <select
                      value={waiterId ?? ""}
                      onChange={(e) => setWaiterId(e.target.value || null)}
                      className="h-10 flex-1 rounded-xl border border-slate-200 bg-white px-3"
                    >
                      <option value="">Sin mesero</option>
                      {waiters.map((w) => (
                        <option key={w.user_id} value={w.user_id}>
                          {w.full_name}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
              </div>
            )}

            {/* Líneas de pago */}
            <div className="space-y-4">
              {payments.map((p) => {
                const isCred = p.method === "CREDITO";
                const isCash = p.method === "EFECTIVO";
                const methodMeta = PAYMENT_METHODS.find((m) => m.id === p.method);

                return (
                  <div key={p.id} className="space-y-3 rounded-2xl border border-slate-200/80 bg-slate-50/60 p-4">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2 font-semibold text-slate-700">
                        {methodMeta?.icon && <methodMeta.icon className="size-4 text-brand" />}
                        <span>{methodMeta?.label ?? p.method}</span>
                      </div>
                      {payments.length > 1 && (
                        <button
                          type="button"
                          onClick={() => removePaymentLine(p.id)}
                          className="flex size-7 items-center justify-center rounded-full text-slate-400 hover:bg-slate-200 hover:text-slate-600"
                          title="Eliminar método"
                        >
                          <X className="size-4" />
                        </button>
                      )}
                    </div>

                    {isCred ? (
                      <div className="space-y-3">
                        <div className="space-y-1">
                          <label className="text-xs font-semibold text-slate-500 uppercase">Monto a crédito</label>
                          <label
                            className={cn(
                              "flex items-center gap-2 rounded-2xl border-2 bg-white px-4 py-3",
                              creditLimit === 0
                                ? "border-slate-200 opacity-60 cursor-not-allowed"
                                : "border-brand",
                            )}
                          >
                            <span className="text-2xl font-bold text-slate-400">$</span>
                            <input
                              type="text"
                              inputMode="numeric"
                              disabled={creditLimit === 0}
                              value={p.amount ? p.amount.toLocaleString("es-CO") : ""}
                              placeholder="0"
                              onChange={(e) => {
                                const digits = e.target.value.replace(/\D/g, "");
                                updatePaymentAmount(p.id, digits ? Number(digits) : 0);
                              }}
                              className="w-full bg-transparent text-3xl font-bold tabular-nums outline-none disabled:cursor-not-allowed"
                            />
                          </label>
                          <div>
                            {creditLimit === 0 ? (
                              <p className="text-xs font-medium text-rose-600">
                                Este cliente no tiene cupo de crédito.
                              </p>
                            ) : (
                              <p className="text-xs text-slate-500">
                                Cupo disponible: {formatCOP(creditAvailable)}
                              </p>
                            )}
                          </div>
                        </div>

                        {/* Interruptor En cuotas */}
                        {creditLimit > 0 && (
                          <div className="rounded-2xl border border-slate-200 bg-white p-4">
                            <div className="flex items-center justify-between">
                              <span className="text-sm font-bold text-slate-800">En cuotas</span>
                              <label className="relative inline-flex cursor-pointer items-center">
                                <input
                                  type="checkbox"
                                  checked={hasInstallments}
                                  onChange={(e) => setHasInstallments(e.target.checked)}
                                  className="peer sr-only"
                                />
                                <div className="peer h-6 w-11 rounded-full bg-slate-200 after:absolute after:top-[2px] after:left-[2px] after:h-5 after:w-5 after:rounded-full after:border after:border-slate-300 after:bg-white after:transition-all after:content-[''] peer-checked:bg-brand peer-checked:after:translate-x-full peer-checked:after:border-white" />
                              </label>
                            </div>

                            {hasInstallments && (
                              <div className="mt-4 space-y-4 border-t border-slate-100 pt-4">
                                <div className="grid grid-cols-3 gap-3">
                                  <div>
                                    <label className="text-xs font-semibold text-slate-500 uppercase">Cuotas</label>
                                    <input
                                      type="number"
                                      min={1}
                                      max={60}
                                      value={installments}
                                      onChange={(e) => {
                                        const v = parseInt(e.target.value, 10);
                                        setInstallments(Number.isNaN(v) ? 1 : Math.max(1, Math.min(60, v)));
                                      }}
                                      className="mt-1 h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm font-semibold"
                                    />
                                  </div>
                                  <div>
                                    <label className="text-xs font-semibold text-slate-500 uppercase">Frecuencia</label>
                                    <select
                                      value={frequency}
                                      onChange={(e) => setFrequency(e.target.value as CreditFrequency)}
                                      className="mt-1 h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm font-semibold"
                                    >
                                      <option value="SEMANAL">Semanal</option>
                                      <option value="QUINCENAL">Quincenal</option>
                                      <option value="MENSUAL">Mensual</option>
                                    </select>
                                  </div>
                                  <div>
                                    <label className="text-xs font-semibold text-slate-500 uppercase">
                                      Primera cuota
                                    </label>
                                    <input
                                      type="date"
                                      min={todayStr}
                                      value={firstDueDate}
                                      onChange={(e) => setFirstDueDate(e.target.value)}
                                      className="mt-1 h-11 w-full rounded-xl border border-slate-200 bg-white px-2 text-sm font-semibold"
                                    />
                                  </div>
                                </div>

                                {activeSchedulePreview && activeSchedulePreview.installments.length > 0 && (
                                  <div className="mt-2 max-h-48 overflow-y-auto rounded-xl border border-slate-200 bg-white shadow-2xs">
                                    <table className="w-full text-left text-xs">
                                      <thead className="sticky top-0 border-b border-slate-100 bg-slate-50 font-semibold text-slate-600">
                                        <tr>
                                          <th className="px-3 py-2">Cuota</th>
                                          <th className="px-3 py-2">Vence</th>
                                          <th className="px-3 py-2 text-right">Valor</th>
                                        </tr>
                                      </thead>
                                      <tbody className="divide-y divide-slate-100 font-mono">
                                        {activeSchedulePreview.installments.map((inst) => (
                                          <tr key={inst.n}>
                                            <td className="px-3 py-2 font-medium text-slate-700">Cuota {inst.n}</td>
                                            <td className="px-3 py-2 text-slate-500">{inst.due_date}</td>
                                            <td className="px-3 py-2 text-right font-bold text-slate-800">
                                              ${formatDecimalCOP(Number(inst.amount))}
                                            </td>
                                          </tr>
                                        ))}
                                      </tbody>
                                    </table>
                                  </div>
                                )}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    ) : isCash ? (
                      <div className="space-y-3">
                        {payments.length > 1 && (
                          <div className="space-y-1">
                            <label className="text-xs font-semibold text-slate-500 uppercase">Monto a pagar en efectivo</label>
                            <div className="flex items-center gap-2 rounded-2xl border border-slate-200 bg-white px-4 py-3">
                              <span className="text-2xl font-bold text-slate-400">$</span>
                              <input
                                type="text"
                                inputMode="numeric"
                                value={p.amount ? p.amount.toLocaleString("es-CO") : ""}
                                placeholder="0"
                                onChange={(e) => {
                                  const digits = e.target.value.replace(/\D/g, "");
                                  updatePaymentAmount(p.id, digits ? Number(digits) : 0);
                                }}
                                className="w-full bg-transparent text-2xl font-bold tabular-nums outline-none"
                              />
                            </div>
                          </div>
                        )}

                        <div className="space-y-1">
                          <label className="text-xs font-semibold text-slate-500 uppercase">
                            Monto recibido en efectivo
                          </label>
                          <label className="flex h-14 items-center gap-2 rounded-xl border border-slate-300 bg-white px-3 focus-within:border-indigo-600 focus-within:ring-4 focus-within:ring-indigo-600/18">
                            <span className="text-2xl font-bold text-slate-400">$</span>
                            <input
                              autoFocus
                              inputMode="numeric"
                              value={received == null ? "" : received.toLocaleString("es-CO")}
                              placeholder={p.amount.toLocaleString("es-CO")}
                              onChange={(e) => {
                                const digits = e.target.value.replace(/\D/g, "");
                                setReceived(digits ? Number(digits) : null);
                              }}
                              className="w-full bg-transparent text-3xl font-bold tabular-nums outline-none"
                            />
                          </label>
                        </div>

                        <div className="grid grid-cols-3 gap-2">
                          {quickAmounts(p.amount).map((quickAmt, i) => (
                            <button
                              key={quickAmt}
                              type="button"
                              onClick={() => setReceived(quickAmt)}
                              className={cn(
                                "h-12 rounded-xl border text-[15px] font-semibold tabular-nums shadow-xs transition-transform active:scale-[0.97]",
                                cashReceived === quickAmt
                                  ? "border-indigo-600 bg-indigo-50 text-indigo-700"
                                  : "border-slate-300 bg-white text-slate-900 hover:bg-slate-50",
                              )}
                            >
                              {i === 0 ? "Exacto" : formatCOP(quickAmt)}
                            </button>
                          ))}
                        </div>

                        <div
                          className={cn(
                            "flex items-center justify-between rounded-[14px] px-[18px] py-4",
                            cashReceived >= p.amount
                              ? "bg-emerald-50 shadow-[inset_0_0_0_1px_#BFE8CF]"
                              : "bg-rose-50 shadow-[inset_0_0_0_1px_#F7C6C6]",
                          )}
                        >
                          {cashReceived >= p.amount ? (
                            <>
                              <p className="text-[15px] font-semibold text-emerald-800">Cambio a entregar</p>
                              <p className="text-[28px] font-bold tracking-[-0.02em] text-emerald-700 tabular-nums">
                                {formatCOP(cashChange)}
                              </p>
                            </>
                          ) : (
                            <p className="text-sm font-medium text-rose-600">
                              Faltan {formatCOP(p.amount - cashReceived)}
                            </p>
                          )}
                        </div>
                      </div>
                    ) : (
                      <div className="space-y-1">
                        <label className="text-xs font-semibold text-slate-500 uppercase">Monto a pagar</label>
                        <div className="flex items-center gap-2 rounded-2xl border border-slate-200 bg-white px-4 py-3">
                          <span className="text-2xl font-bold text-slate-400">$</span>
                          <input
                            type="text"
                            inputMode="numeric"
                            value={p.amount ? p.amount.toLocaleString("es-CO") : ""}
                            placeholder="0"
                            onChange={(e) => {
                              const digits = e.target.value.replace(/\D/g, "");
                              updatePaymentAmount(p.id, digits ? Number(digits) : 0);
                            }}
                            className="w-full bg-transparent text-2xl font-bold tabular-nums outline-none"
                          />
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            {/* Error y flujo de autorización */}
            {error && (
              <div className="space-y-3 rounded-2xl bg-rose-50 p-4 text-sm text-rose-700">
                <p className="font-medium">{error}</p>

                {creditErrorData && (
                  canCreditOverride ? (
                    !showOverrideForm ? (
                      <button
                        type="button"
                        onClick={() => setShowOverrideForm(true)}
                        className="rounded-xl bg-rose-600 px-4 py-2 text-xs font-bold text-white hover:bg-rose-700"
                      >
                        Autorizar igual
                      </button>
                    ) : (
                      <div className="space-y-3 border-t border-rose-200 pt-3">
                        <label className="text-xs font-bold text-rose-800 uppercase">Motivo de la autorización</label>
                        <input
                          type="text"
                          autoFocus
                          placeholder="Motivo de la autorización"
                          value={overrideReason}
                          onChange={(e) => {
                            setOverrideReason(e.target.value);
                            setOverrideError(null);
                          }}
                          className="h-11 w-full rounded-xl border border-rose-300 bg-white px-3 text-sm font-medium text-slate-800 outline-none focus:border-rose-500"
                        />
                        {overrideError && <p className="text-xs font-semibold text-rose-600">{overrideError}</p>}
                        <button
                          type="button"
                          disabled={submitting}
                          onClick={() => {
                            const trimmed = overrideReason.trim();
                            if (trimmed.length < 3 || trimmed.length > 200) {
                              setOverrideError("El motivo debe tener entre 3 y 200 caracteres.");
                              return;
                            }
                            confirm(trimmed);
                          }}
                          className="flex h-11 w-full items-center justify-center gap-2 rounded-xl bg-rose-600 px-4 text-sm font-bold text-white hover:bg-rose-700 disabled:opacity-50"
                        >
                          {submitting ? <Loader2 className="size-4 animate-spin" /> : null}
                          Autorizar y cobrar
                        </button>
                      </div>
                    )
                  ) : (
                    <p className="text-xs font-medium text-rose-800">
                      Pedile a un supervisor que autorice la venta.
                    </p>
                  )
                )}
              </div>
            )}

            <button
              onClick={() => confirm()}
              disabled={!canConfirm}
              className="btn-cobrar flex h-14 w-full items-center justify-center gap-3 rounded-[14px] text-base font-semibold disabled:cursor-not-allowed"
            >
              {submitting ? <Loader2 className="size-5 animate-spin" /> : <Check className="size-5" />}
              Confirmar cobro (Enter)
            </button>
            <p className="flex justify-between text-[13px] text-slate-500">
              <span>
                Presioná <kbd className="rounded-md bg-slate-100 px-1.5 py-0.5 text-xs font-semibold text-slate-700">Esc</kbd> para volver
              </span>
              <span>Cajero en turno: {cashierName}</span>
            </p>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function customerLabel(
  customer: { name: string; doc_type?: string | null; doc_number?: string | null } | null,
  alias: string | null,
): string {
  if (customer) {
    return customer.doc_type && customer.doc_number
      ? `${customer.name} (${customer.doc_type} ${customer.doc_number})`
      : customer.name;
  }
  return alias ?? "Consumidor final";
}

function ReceiptView({
  receipt,
  meta,
  widthMm,
  onNewSale,
}: {
  receipt: Receipt;
  meta: TicketMeta;
  widthMm: number;
  onNewSale: () => void;
}) {
  const { response, method } = receipt;
  const branchName = useCurrentBranch().name;
  const business = useBusinessInfo();
  // Los valores del comprobante son los que cobró el servidor, no los del carrito.
  // El CUDE y el QR llegan unos segundos después: el tiquete se completa solo.
  const fiscal = useOrderFiscal(response.order.id);
  const model = withFiscal(ticketFromSale(response, meta), fiscal.doc);
  const change = Number(response.change_due);
  const waitingDian =
    fiscal.doc != null && fiscal.doc.provider !== "SIMULADO" && ["PENDIENTE", "ENVIADO", "EN_PROCESO"].includes(fiscal.doc.status);

  const creditPayment = response.payments.find((p) => p.method === "CREDITO");

  return (
    <div className="space-y-5">
      <PrintTicket model={model} business={business} branchName={branchName} widthMm={widthMm} />
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-2 rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1 text-sm font-semibold text-emerald-700">
          <span className="size-2 rounded-full bg-emerald-500" /> Aprobada #{response.receipt_number}
        </span>
        <span className="font-mono text-xs text-slate-400">
          {new Date().toLocaleTimeString("es-CO", { hour12: false })}
        </span>
      </div>

      <div className="flex flex-col items-center gap-4 text-center">
        <span className="nx-rise flex size-[76px] items-center justify-center rounded-full bg-emerald-500 text-white shadow-[0_0_0_10px_#E9F8EF]">
          <Check className="size-[38px]" strokeWidth={2.6} aria-hidden />
        </span>
        <div>
          <DialogTitle className="text-[26px] font-semibold tracking-[-0.022em]">¡Venta registrada con éxito!</DialogTitle>
          <p className="mt-1 text-sm text-slate-500">El inventario se actualizó en tiempo real</p>
        </div>
      </div>

      {method === "EFECTIVO" && change > 0 && (
        <div className="flex items-center gap-3 rounded-2xl bg-emerald-50 p-[18px] shadow-[inset_0_0_0_1px_#BFE8CF]">
          <Banknote className="size-8 text-emerald-600" aria-hidden />
          <div>
            <p className="text-sm font-semibold text-emerald-800">Devolver cambio</p>
            <p className="text-[44px] leading-[1.1] font-bold tracking-[-0.04em] text-emerald-700 tabular-nums">
              {formatCOP(change)}
            </p>
          </div>
        </div>
      )}

      <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-4 font-mono text-xs">
        <p className="text-center font-sans text-sm font-bold">{branchName}</p>
        <p className="mb-3 text-center text-slate-500">{model.title}</p>
        {model.lines.map((line, i) => (
          <div key={i} className="flex justify-between py-0.5">
            <span>
              {line.qty}x {line.name}
            </span>
            <span className="font-bold">{formatCOP(line.gross)}</span>
          </div>
        ))}
        <div className="mt-3 space-y-0.5 border-t border-dashed border-slate-300 pt-3">
          <div className="flex justify-between text-slate-500">
            <span>Subtotal</span>
            <span>{formatCOP(model.subtotal)}</span>
          </div>
          {model.discountTotal > 0 && (
            <div className="flex justify-between text-emerald-600">
              <span>Descuentos</span>
              <span>−{formatCOP(model.discountTotal)}</span>
            </div>
          )}
          {model.taxes.map((t) => (
            <div key={t.label} className="flex justify-between text-slate-500">
              <span>{t.label}</span>
              <span>{formatCOP(t.amount)}</span>
            </div>
          ))}
          {model.tip > 0 && (
            <div className="flex justify-between text-slate-500">
              <span>Propina</span>
              <span>{formatCOP(model.tip)}</span>
            </div>
          )}
          <div className="flex justify-between font-sans text-sm font-bold">
            <span>TOTAL PAGADO</span>
            <span className="text-indigo-600">{formatCOP(model.total)}</span>
          </div>
        </div>

        {creditPayment && (
          <div className="mt-3 border-t border-dashed border-slate-300 pt-2 font-sans text-xs">
            <div className="flex justify-between font-bold text-amber-800">
              <span>Crédito</span>
              <span>{formatCOP(Number(creditPayment.amount))}</span>
            </div>
            {model.payments?.find((p) => p.label === "Crédito")?.detail && (
              <p className="mt-0.5 text-slate-500">
                {model.payments.find((p) => p.label === "Crédito")?.detail}
              </p>
            )}
          </div>
        )}
      </div>

      {fiscal.doc && fiscal.doc.provider !== "SIMULADO" && (
        <FiscalStatusPanel doc={fiscal.doc} canRetry={false} onRetried={fiscal.reload} />
      )}
      <button
        onClick={() => window.print()}
        className="flex h-12 w-full items-center justify-center gap-2 rounded-xl border border-slate-300 bg-white text-[15px] font-semibold shadow-xs hover:bg-slate-50"
      >
        <Printer className="size-4" /> {waitingDian ? "Imprimir sin esperar el QR" : "Imprimir"}
      </button>
      <button
        onClick={onNewSale}
        className="flex h-14 w-full items-center justify-center gap-2 rounded-xl bg-indigo-600 text-base font-semibold text-white shadow-[0_1px_2px_rgb(20_22_28/0.12),inset_0_1px_0_rgb(255_255_255/0.14)] hover:bg-indigo-700"
      >
        <PlusCircle className="size-5" /> Nueva venta (Enter)
      </button>
    </div>
  );
}

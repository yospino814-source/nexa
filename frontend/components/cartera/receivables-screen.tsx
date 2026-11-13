"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  DollarSign,
  Eye,
  Loader2,
  MessageSquare,
  Wallet,
} from "lucide-react";
import { StatCard } from "@/components/nexa/stat-card";
import { MoneyInput } from "@/components/caja/money-input";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { PaymentTicketDialog } from "@/components/clientes/payment-ticket";
import {
  api,
  ApiError,
  friendlyError,
  type Receivable,
  type ReceivableDetail,
  type ReceivablePaymentResponse,
  type ReceivablesSummaryResponse,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { useSession } from "@/lib/stores/session";

type DebtorRow = {
  customer_id: string;
  customer_name: string;
  total_balance: number;
  max_days_overdue: number;
  accounts: Receivable[];
};

export function ReceivablesScreen() {
  const { branches, can } = useSession();
  const canCollect = can("credit:collect");
  const cashierName = useSession((s) => s.name || "Cajero");

  const [selectedBranchId, setSelectedBranchId] = useState<string>("TODAS");
  const [summary, setSummary] = useState<ReceivablesSummaryResponse | null>(null);
  const [receivables, setReceivables] = useState<Receivable[]>([]);
  const [loading, setLoading] = useState(true);
  const [whatsAppError, setWhatsAppError] = useState<string | null>(null);

  // Diálogo de abono
  const [payDialogOpen, setPayDialogOpen] = useState(false);
  const [selectedReceivable, setSelectedReceivable] = useState<ReceivableDetail | null>(null);
  const [payCustomerName, setPayCustomerName] = useState("");
  const [payId, setPayId] = useState("");
  const [payAmount, setPayAmount] = useState(0);
  const [payMethod, setPayMethod] = useState<"EFECTIVO" | "DATAFONO" | "QR" | "TRANSFERENCIA">("EFECTIVO");
  const [payReference, setPayReference] = useState("");
  const [submittingPay, setSubmittingPay] = useState(false);
  const [paymentError, setPaymentError] = useState<string | null>(null);
  const [paymentErrorCode, setPaymentErrorCode] = useState<string | null>(null);

  // Tirilla de abono
  const [ticketOpen, setTicketOpen] = useState(false);
  const [ticketPayment, setTicketPayment] = useState<ReceivablePaymentResponse | null>(null);
  const [ticketReceivable, setTicketReceivable] = useState<ReceivableDetail | null>(null);

  const loadData = useCallback(async (branchIdFilter?: string) => {
    setLoading(true);
    setWhatsAppError(null);
    try {
      const bId = branchIdFilter && branchIdFilter !== "TODAS" ? branchIdFilter : undefined;
      const [sumRes, listRes] = await Promise.all([
        api.receivables.summary(bId),
        api.receivables.list(),
      ]);
      setSummary(sumRes);
      // Filtrar la lista si se seleccionó una sucursal específica
      const filteredList = bId ? listRes.filter((r) => r.branch_id === bId) : listRes;
      setReceivables(filteredList);
    } catch (err) {
      console.error("Error cargando datos de cartera:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve().then(() => {
      if (!cancelled) {
        void loadData(selectedBranchId);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [selectedBranchId, loadData]);

  // Agrupar cuentas por cliente para la tabla de deudores
  const debtorsMap = new Map<string, DebtorRow>();
  for (const r of receivables) {
    if (r.status === "ABIERTA" || r.status === "PARCIAL") {
      const prev = debtorsMap.get(r.customer_id) || {
        customer_id: r.customer_id,
        customer_name: r.customer_name,
        total_balance: 0,
        max_days_overdue: 0,
        accounts: [],
      };
      prev.total_balance += Number(r.balance || 0);
      prev.max_days_overdue = Math.max(prev.max_days_overdue, r.days_overdue || 0);
      prev.accounts.push(r);
      debtorsMap.set(r.customer_id, prev);
    }
  }

  const debtorsList = Array.from(debtorsMap.values()).sort(
    (a, b) => b.total_balance - a.total_balance,
  );

  // Abrir abono para la cuenta de mayor mora del cliente
  const handleOpenPay = async (debtor: DebtorRow) => {
    setPaymentError(null);
    setPaymentErrorCode(null);
    setWhatsAppError(null);

    // Cuenta con mayor días de mora
    const sortedAccounts = [...debtor.accounts].sort(
      (a, b) => (b.days_overdue || 0) - (a.days_overdue || 0),
    );
    const targetAccount = sortedAccounts[0];
    if (!targetAccount) return;

    try {
      const detail = await api.receivables.get(targetAccount.id);
      setSelectedReceivable(detail);
      setPayCustomerName(debtor.customer_name);
      setPayId(crypto.randomUUID());
      setPayAmount(Number(detail.balance));
      setPayMethod("EFECTIVO");
      setPayReference("");
      setPayDialogOpen(true);
    } catch (err) {
      console.error("Error obteniendo detalle de cuenta:", err);
    }
  };

  const handlePaySubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedReceivable) return;

    if (payAmount <= 0) {
      setPaymentError("El monto a abonar debe ser mayor a cero.");
      return;
    }
    if (payAmount > Number(selectedReceivable.balance)) {
      setPaymentError("El monto a abonar no puede ser mayor al saldo de la cuenta.");
      return;
    }

    if (payMethod !== "EFECTIVO" && (!payReference || payReference.trim().length < 3)) {
      setPaymentError("La referencia es obligatoria y debe tener al menos 3 caracteres.");
      return;
    }

    setSubmittingPay(true);
    setPaymentError(null);
    setPaymentErrorCode(null);

    try {
      const res = await api.receivables.pay(selectedReceivable.id, {
        id: payId,
        amount: payAmount.toFixed(2),
        method: payMethod,
        reference: payReference.trim() || null,
      });

      setTicketPayment(res);
      setTicketReceivable(selectedReceivable);
      setTicketOpen(true);
      setPayDialogOpen(false);

      // Recargar datos
      void loadData(selectedBranchId);
    } catch (err) {
      if (err instanceof ApiError) {
        setPaymentErrorCode(err.code || null);
        setPaymentError(friendlyError(err));
      } else {
        setPaymentError("Error al registrar el abono. Verificá los datos.");
      }
    } finally {
      setSubmittingPay(false);
    }
  };

  // Cobrar por WhatsApp para la cuenta de mayor mora
  const handleWhatsApp = async (debtor: DebtorRow) => {
    setWhatsAppError(null);
    const sortedAccounts = [...debtor.accounts].sort(
      (a, b) => (b.days_overdue || 0) - (a.days_overdue || 0),
    );
    const target = sortedAccounts[0];
    if (!target) return;

    try {
      const res = await api.receivables.whatsappLink(target.id);
      window.open(res.url, "_blank", "noopener,noreferrer");
    } catch (err) {
      if (err instanceof ApiError && err.code === "CUSTOMER_NO_PHONE") {
        setWhatsAppError("El cliente no tiene un celular válido para WhatsApp.");
      } else {
        setWhatsAppError(friendlyError(err));
      }
    }
  };

  // Antigüedad (Aging) datos para barras
  const agingData = [
    { label: "Al día", value: Number(summary?.aging.al_dia || 0), color: "bg-emerald-500" },
    { label: "1 a 30 días", value: Number(summary?.aging.d1_30 || 0), color: "bg-amber-400" },
    { label: "31 a 60 días", value: Number(summary?.aging.d31_60 || 0), color: "bg-amber-500" },
    { label: "61 a 90 días", value: Number(summary?.aging.d61_90 || 0), color: "bg-orange-500" },
    { label: "91 a 120 días", value: Number(summary?.aging.d91_120 || 0), color: "bg-rose-500" },
    { label: "Más de 120 días", value: Number(summary?.aging.d120_mas || 0), color: "bg-rose-700" },
  ];
  const maxAgingVal = Math.max(...agingData.map((d) => d.value), 1);

  return (
    <div className="space-y-6 p-6">
      {/* Encabezado y selector de sucursal */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900">Cartera de clientes</h1>
          <p className="text-sm text-slate-500">
            Resumen consolidado, antigüedad de saldos, deudores y gestión de cobro.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <label htmlFor="branch-select" className="text-xs font-semibold text-slate-600">
            Sucursal:
          </label>
          <select
            id="branch-select"
            value={selectedBranchId}
            onChange={(e) => setSelectedBranchId(e.target.value)}
            className="rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 shadow-2xs outline-none focus:border-teal-600 focus:ring-1 focus:ring-teal-600"
          >
            <option value="TODAS">Todas las sucursales</option>
            {branches.map((b) => (
              <option key={b.id} value={b.id}>
                {b.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {whatsAppError && (
        <div className="rounded-xl border border-amber-200 bg-amber-50 p-4 text-xs font-semibold text-amber-800 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <AlertCircle className="size-4 shrink-0 text-amber-600" />
            <span>{whatsAppError}</span>
          </div>
          <button
            type="button"
            onClick={() => setWhatsAppError(null)}
            className="text-amber-700 hover:text-amber-900 font-bold"
          >
            ✕
          </button>
        </div>
      )}

      {/* 1. Cuatro StatCard */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title="Por cobrar"
          value={formatCOP(Number(summary?.total_balance || 0))}
          caption="Total cartera activa"
          icon={Wallet}
          tone="default"
        />
        <StatCard
          title="Vencido"
          value={formatCOP(Number(summary?.overdue_balance || 0))}
          caption="Cuentas con días de mora"
          icon={AlertCircle}
          tone={Number(summary?.overdue_balance || 0) > 0 ? "danger" : "default"}
        />
        <StatCard
          title="Vence esta semana"
          value={formatCOP(Number(summary?.due_this_week || 0))}
          caption="Vencimientos a 7 días"
          icon={Clock}
          tone="warning"
        />
        <StatCard
          title="Cobrado este mes"
          value={formatCOP(Number(summary?.collected_this_month || 0))}
          caption="Abonos recibidos en el mes"
          icon={CheckCircle2}
          tone="success"
        />
      </div>

      {/* 2. Gráfica de antigüedad y Top 10 deudores */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Antigüedad de cartera (2 columnas) */}
        <div className="lg:col-span-2 rounded-2xl border border-slate-200/60 bg-white p-6 shadow-xs space-y-4">
          <div>
            <h2 className="text-sm font-bold text-slate-800">Antigüedad de saldos</h2>
            <p className="text-xs text-slate-400">Distribución de deuda por tramos de mora.</p>
          </div>

          <div className="space-y-3 pt-2">
            {agingData.map((bar) => {
              const pct = maxAgingVal > 0 ? (bar.value / maxAgingVal) * 100 : 0;
              return (
                <div key={bar.label} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span className="font-semibold text-slate-600">{bar.label}</span>
                    <span className="font-bold text-slate-800 tabular-nums">
                      {formatCOP(bar.value)}
                    </span>
                  </div>
                  <div className="h-3 w-full rounded-full bg-slate-100 overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-all duration-500 ${bar.color}`}
                      style={{ width: `${Math.max(bar.value > 0 ? 2 : 0, pct)}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* 4. Top 10 Deudores (1 columna) */}
        <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-xs flex flex-col justify-between">
          <div>
            <h2 className="text-sm font-bold text-slate-800">Top 10 deudores</h2>
            <p className="text-xs text-slate-400">Clientes con mayor saldo pendiente.</p>
          </div>

          <div className="mt-4 flex-1 divide-y divide-slate-100 overflow-auto max-h-[300px]">
            {!summary || summary.top_debtors.length === 0 ? (
              <p className="py-8 text-center text-xs text-slate-400">No hay deudores registrados.</p>
            ) : (
              summary.top_debtors.map((d, idx) => (
                <div key={d.customer_id} className="flex items-center justify-between py-2 text-xs">
                  <div className="flex items-center gap-2 truncate pr-2">
                    <span className="flex size-5 shrink-0 items-center justify-center rounded-full bg-slate-100 text-[10px] font-bold text-slate-600">
                      {idx + 1}
                    </span>
                    <Link
                      href={`/clientes/${d.customer_id}`}
                      className="truncate font-semibold text-slate-800 hover:text-teal-700"
                    >
                      {d.name}
                    </Link>
                  </div>
                  <div className="text-right shrink-0">
                    <span className="font-bold text-slate-900 tabular-nums">
                      {formatCOP(Number(d.balance))}
                    </span>
                    {d.max_days_overdue > 0 && (
                      <p className="text-[10px] font-semibold text-rose-600">
                        {d.max_days_overdue} d mora
                      </p>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      {/* 3. Tabla de deudores */}
      <div className="rounded-2xl border border-slate-200/60 bg-white shadow-xs overflow-hidden">
        <div className="border-b border-slate-100 p-5">
          <h2 className="text-sm font-bold text-slate-800">Deudores activos</h2>
          <p className="text-xs text-slate-400">Clientes con cuentas por cobrar abiertas o parciales.</p>
        </div>

        {loading ? (
          <div className="py-16 text-center text-slate-400 text-xs">
            <Loader2 className="size-5 animate-spin mx-auto mb-2 text-teal-600" />
            Cargando deudores...
          </div>
        ) : debtorsList.length === 0 ? (
          <div className="py-16 text-center text-slate-400 text-xs">
            No hay deudores con cuentas pendientes.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="bg-slate-50 text-[11px] font-bold uppercase tracking-wider text-slate-400 border-b">
                <tr>
                  <th className="py-3 px-4">Cliente</th>
                  <th className="py-3 px-4 text-right">Saldo</th>
                  <th className="py-3 px-4 text-center">Días de mora</th>
                  <th className="py-3 px-4 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {debtorsList.map((debtor) => {
                  const hasOverdue = debtor.max_days_overdue > 0;
                  return (
                    <tr key={debtor.customer_id} className="hover:bg-slate-50/80 transition-colors">
                      <td className="py-3 px-4">
                        <Link
                          href={`/clientes/${debtor.customer_id}`}
                          className="font-bold text-slate-800 hover:text-teal-700 hover:underline"
                        >
                          {debtor.customer_name}
                        </Link>
                        <p className="text-[11px] text-slate-400 mt-0.5">
                          {debtor.accounts.length} {debtor.accounts.length === 1 ? "cuenta" : "cuentas"}
                        </p>
                      </td>
                      <td className="py-3 px-4 text-right font-bold text-slate-900 tabular-nums">
                        {formatCOP(debtor.total_balance)}
                      </td>
                      <td className="py-3 px-4 text-center">
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-bold ${
                            hasOverdue
                              ? "bg-rose-50 text-rose-700 border border-rose-200"
                              : "bg-emerald-50 text-emerald-700 border border-emerald-200"
                          }`}
                        >
                          {debtor.max_days_overdue} {debtor.max_days_overdue === 1 ? "día" : "días"}
                        </span>
                      </td>
                      <td className="py-3 px-4 text-right">
                        <div className="inline-flex items-center gap-1.5 justify-end">
                          {canCollect && (
                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              onClick={() => handleOpenPay(debtor)}
                              className="h-8 text-xs font-semibold text-teal-800 border-teal-200 hover:bg-teal-50"
                            >
                              <DollarSign className="size-3.5 mr-1" />
                              Abonar
                            </Button>
                          )}
                          <Button
                            type="button"
                            size="sm"
                            variant="outline"
                            onClick={() => handleWhatsApp(debtor)}
                            className="h-8 text-xs font-semibold text-emerald-800 border-emerald-200 hover:bg-emerald-50"
                            title="Cobrar por WhatsApp"
                          >
                            <MessageSquare className="size-3.5 mr-1" />
                            WhatsApp
                          </Button>
                          <Link
                            href={`/clientes/${debtor.customer_id}`}
                            className="inline-flex items-center h-8 px-2.5 rounded-lg text-xs font-semibold text-slate-600 hover:text-slate-900 hover:bg-slate-100 transition-colors"
                          >
                            <Eye className="size-3.5 mr-1" />
                            Ver
                          </Link>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Diálogo modal de abono */}
      <Dialog open={payDialogOpen} onOpenChange={(open) => !open && setPayDialogOpen(false)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Registrar abono</DialogTitle>
            {selectedReceivable && (
              <p className="text-xs text-slate-500 pt-1">
                Cliente: <span className="font-bold text-slate-700">{payCustomerName}</span>
                <br />
                Venta: <span className="font-semibold text-slate-700">{selectedReceivable.receipt_number || "Sin número"}</span>
                {" · "}
                Saldo pendiente:{" "}
                <span className="font-bold text-rose-600">
                  {formatCOP(Number(selectedReceivable.balance))}
                </span>
              </p>
            )}
          </DialogHeader>

          <form onSubmit={handlePaySubmit} className="space-y-4 py-2">
            <div>
              <label htmlFor="pay-amount-cartera" className="block text-xs font-semibold text-slate-700 mb-1.5">
                Monto a abonar
              </label>
              <MoneyInput
                id="pay-amount-cartera"
                value={payAmount}
                onChange={setPayAmount}
                autoFocus
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                Método de pago
              </label>
              <div className="grid grid-cols-2 gap-2">
                {(
                  [
                    { id: "EFECTIVO", label: "Efectivo" },
                    { id: "DATAFONO", label: "Datáfono" },
                    { id: "QR", label: "QR" },
                    { id: "TRANSFERENCIA", label: "Transferencia" },
                  ] as const
                ).map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => setPayMethod(m.id)}
                    className={`rounded-xl border p-2.5 text-xs font-semibold transition-all ${
                      payMethod === m.id
                        ? "border-teal-700 bg-teal-50 text-teal-900 shadow-xs ring-1 ring-teal-700"
                        : "border-slate-200 bg-white text-slate-700 hover:bg-slate-50"
                    }`}
                  >
                    {m.label}
                  </button>
                ))}
              </div>
            </div>

            {payMethod !== "EFECTIVO" && (
              <div>
                <label htmlFor="pay-reference-cartera" className="block text-xs font-semibold text-slate-700 mb-1.5">
                  Referencia <span className="text-rose-500">*</span>
                </label>
                <input
                  id="pay-reference-cartera"
                  type="text"
                  required
                  placeholder="Ej: AUT-987654"
                  value={payReference}
                  onChange={(e) => setPayReference(e.target.value)}
                  className="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm font-medium outline-none focus:border-teal-600 focus:ring-1 focus:ring-teal-600"
                />
                <p className="text-[11px] text-slate-400 mt-1">
                  Mínimo 3 caracteres, obligatorio para transferencias y tarjetas.
                </p>
              </div>
            )}

            {paymentError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700 space-y-1">
                <div className="flex items-center gap-2">
                  <AlertCircle className="size-4 shrink-0 text-rose-600" />
                  <span>{paymentError}</span>
                </div>
                {paymentErrorCode === "PAYMENT_NEEDS_OPEN_SESSION" && (
                  <div className="pl-6 pt-1">
                    <Link
                      href="/caja"
                      className="font-bold underline text-rose-800 hover:text-rose-950"
                    >
                      Abrir caja
                    </Link>
                  </div>
                )}
              </div>
            )}

            <DialogFooter className="flex sm:justify-between gap-2 pt-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => setPayDialogOpen(false)}
                disabled={submittingPay}
              >
                Cancelar
              </Button>
              <Button
                type="submit"
                disabled={submittingPay || payAmount <= 0}
                className="bg-teal-700 hover:bg-teal-800 text-white font-semibold"
              >
                {submittingPay ? (
                  <>
                    <Loader2 className="size-4 animate-spin mr-1.5" />
                    Registrando...
                  </>
                ) : (
                  "Registrar abono"
                )}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Tirilla de comprobante de abono */}
      {ticketPayment && selectedReceivable && (
        <PaymentTicketDialog
          open={ticketOpen}
          onClose={() => setTicketOpen(false)}
          payment={ticketPayment}
          customer={{ name: payCustomerName }}
          receiptNumber={ticketReceivable?.receipt_number}
          cashierName={cashierName}
        />
      )}
    </div>
  );
}

"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  AlertTriangle,
  Banknote,
  Calendar,
  CheckCircle2,
  Clock,
  DollarSign,
  FileText,
  Loader2,
  RefreshCw,
  Search,
  ShieldAlert,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { MoneyInput } from "@/components/caja/money-input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  api,
  type CreatePayablePaymentInput,
  type Payable,
  type PayablePaymentMethod,
  type PayableStatus,
  type Supplier,
  type WriteOffPayableInput,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP, formatDateTime } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUS_CONFIG: Record<
  PayableStatus,
  { label: string; className: string; dot: string }
> = {
  ABIERTA: {
    label: "Abierta",
    className: "border-blue-200 bg-blue-50 text-blue-700",
    dot: "bg-blue-500",
  },
  PARCIAL: {
    label: "Pago Parcial",
    className: "border-purple-200 bg-purple-50 text-purple-700",
    dot: "bg-purple-500",
  },
  PAGADA: {
    label: "Pagada",
    className: "border-emerald-200 bg-emerald-50 text-emerald-700",
    dot: "bg-emerald-500",
  },
  CASTIGADA: {
    label: "Castigada",
    className: "border-slate-200 bg-slate-100 text-slate-600",
    dot: "bg-slate-400",
  },
};

export function PayablesScreen() {
  const currentBranch = useCurrentBranch();
  const branchId = currentBranch?.id;
  const isOwner = useSession(
    (s) => s.branches.find((b) => b.id === s.branchId)?.role === "DUENO",
  );
  const canPay = useSession((s) => s.can("purchases:pay"));

  // Filtros
  const [search, setSearch] = useState("");
  const [selectedStatus, setSelectedStatus] = useState<PayableStatus | "ALL">("ALL");
  const [selectedSupplierId, setSelectedSupplierId] = useState<string>("ALL");
  const [onlyOverdue, setOnlyOverdue] = useState(false);

  // Modales
  const [paymentModalPayable, setPaymentModalPayable] = useState<Payable | null>(null);
  const [writeOffModalPayable, setWriteOffModalPayable] = useState<Payable | null>(null);

  // Estados de formulario de pago
  const [paymentAmount, setPaymentAmount] = useState<number>(0);
  const [paymentMethod, setPaymentMethod] = useState<PayablePaymentMethod>("EFECTIVO");
  const [paymentReference, setPaymentReference] = useState("");
  const [paymentBusy, setPaymentBusy] = useState(false);
  const [paymentError, setPaymentError] = useState<string | null>(null);

  // Estados de formulario de castigo
  const [writeOffReason, setWriteOffReason] = useState("");
  const [writeOffBusy, setWriteOffBusy] = useState(false);
  const [writeOffError, setWriteOffError] = useState<string | null>(null);

  // Notificación toast
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  };

  // Cargar datos
  const {
    data: payables,
    loading: loadingPayables,
    error: errorPayables,
    reload: reloadPayables,
  } = useApi<Payable[]>(
    () =>
      api.payables.list({
        branch_id: branchId,
        status: selectedStatus === "ALL" ? undefined : selectedStatus,
        supplier_id: selectedSupplierId === "ALL" ? undefined : selectedSupplierId,
        overdue: onlyOverdue ? true : undefined,
      }),
    `${branchId}-${selectedStatus}-${selectedSupplierId}-${onlyOverdue}`,
  );

  const {
    data: aging,
    reload: reloadAging,
  } = useApi(() => api.payables.aging(branchId), `${branchId}`);

  const { data: suppliers } = useApi<Supplier[]>(() => api.suppliers.list());

  // Filtrado local por texto
  const filteredPayables = useMemo(() => {
    if (!payables) return [];
    if (!search.trim()) return payables;
    const q = search.toLowerCase().trim();
    return payables.filter(
      (p) =>
        p.supplier_name?.toLowerCase().includes(q) ||
        p.document_number?.toLowerCase().includes(q) ||
        p.id.toLowerCase().includes(q),
    );
  }, [payables, search]);

  const openPaymentDialog = (p: Payable) => {
    setPaymentModalPayable(p);
    setPaymentAmount(Number(p.balance));
    setPaymentMethod("EFECTIVO");
    setPaymentReference("");
    setPaymentError(null);
  };

  const handleConfirmPayment = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!paymentModalPayable) return;

    if (paymentAmount <= 0) {
      setPaymentError("El monto del pago debe ser mayor a 0.");
      return;
    }
    if (paymentAmount > Number(paymentModalPayable.balance)) {
      setPaymentError("El monto no puede superar el saldo pendiente.");
      return;
    }
    if (
      (paymentMethod === "TRANSFERENCIA" || paymentMethod === "CHEQUE") &&
      (!paymentReference.trim() || paymentReference.trim().length < 3)
    ) {
      setPaymentError("Se requiere una referencia válida (mínimo 3 caracteres).");
      return;
    }

    setPaymentBusy(true);
    setPaymentError(null);

    try {
      const payload: CreatePayablePaymentInput = {
        id: crypto.randomUUID(),
        amount: paymentAmount.toFixed(2),
        method: paymentMethod,
        reference: paymentReference.trim() || null,
      };

      await api.payables.createPayment(paymentModalPayable.id, payload);
      setPaymentModalPayable(null);
      showToast("Pago registrado correctamente.");
      reloadPayables();
      reloadAging();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al registrar el pago.";
      setPaymentError(msg);
    } finally {
      setPaymentBusy(false);
    }
  };

  const openWriteOffDialog = (p: Payable) => {
    setWriteOffModalPayable(p);
    setWriteOffReason("");
    setWriteOffError(null);
  };

  const handleConfirmWriteOff = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!writeOffModalPayable) return;

    if (writeOffReason.trim().length < 3) {
      setWriteOffError("El motivo debe tener al menos 3 caracteres.");
      return;
    }

    setWriteOffBusy(true);
    setWriteOffError(null);

    try {
      const payload: WriteOffPayableInput = {
        reason: writeOffReason.trim(),
      };

      await api.payables.writeOff(writeOffModalPayable.id, payload);
      setWriteOffModalPayable(null);
      showToast("Cuenta por pagar castigada.");
      reloadPayables();
      reloadAging();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al castigar la cuenta.";
      setWriteOffError(msg);
    } finally {
      setWriteOffBusy(false);
    }
  };

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast Notification */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 flex items-center gap-2 rounded-2xl bg-slate-900 px-5 py-3 text-sm font-medium text-white shadow-xl animate-in fade-in slide-in-from-bottom-2">
          <CheckCircle2 className="size-4 text-emerald-400" />
          <span>{toast}</span>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight text-slate-900 font-headline">
            Cuentas por pagar
          </h1>
          <p className="mt-1 text-sm text-slate-500">
            Seguimiento de facturas de compras y gastos, vencimientos y antigüedad de saldos.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Link
            href="/compras"
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50 shadow-xs"
          >
            <FileText className="size-4 text-slate-500" />
            Órdenes de compra
          </Link>

          <button
            onClick={() => {
              reloadPayables();
              reloadAging();
            }}
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50 shadow-xs"
          >
            <RefreshCw className="size-4 text-slate-500" />
            Actualizar
          </button>
        </div>
      </div>

      {/* 6 Tarjetas de Antigüedad de Saldos (Aging) */}
      <div className="grid gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-6">
        <StatCard
          title="AL DÍA"
          value={formatCOP(aging ? Number(aging.al_dia) : 0)}
          caption="Sin vencer"
          icon={Calendar}
          tone="success"
        />
        <StatCard
          title="1 A 30 DÍAS"
          value={formatCOP(aging ? Number(aging.d1_30) : 0)}
          caption="Vencida 1-30d"
          icon={Clock}
          tone="warning"
        />
        <StatCard
          title="31 A 60 DÍAS"
          value={formatCOP(aging ? Number(aging.d31_60) : 0)}
          caption="Vencida 31-60d"
          icon={Clock}
          tone="warning"
        />
        <StatCard
          title="61 A 90 DÍAS"
          value={formatCOP(aging ? Number(aging.d61_90) : 0)}
          caption="Vencida 61-90d"
          icon={AlertTriangle}
          tone="danger"
        />
        <StatCard
          title="91 A 120 DÍAS"
          value={formatCOP(aging ? Number(aging.d91_120) : 0)}
          caption="Vencida 91-120d"
          icon={AlertTriangle}
          tone="danger"
        />
        <StatCard
          title="+120 DÍAS"
          value={formatCOP(aging ? Number(aging.d120_mas) : 0)}
          caption="Mora mayor a 120d"
          icon={ShieldAlert}
          tone="danger"
        />
      </div>

      {/* Tabla con Búsqueda y Filtros */}
      <div className="overflow-hidden rounded-3xl border border-slate-200/80 bg-white shadow-sm">
        {/* Barra de Filtros */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-100 p-5">
          <div className="flex flex-1 flex-wrap items-center gap-3">
            {/* Buscador */}
            <div className="relative min-w-[240px] max-w-sm flex-1">
              <Search className="absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
              <input
                type="text"
                placeholder="Buscar por documento o proveedor..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-10 w-full rounded-full border border-slate-200 bg-slate-50/50 pl-9 pr-4 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:bg-white focus:outline-none"
              />
            </div>

            {/* Selector de Proveedor */}
            <select
              value={selectedSupplierId}
              onChange={(e) => setSelectedSupplierId(e.target.value)}
              className="h-10 rounded-full border border-slate-200 bg-white px-3.5 text-xs font-medium text-slate-700 hover:bg-slate-50 focus:border-teal-700 focus:outline-none"
            >
              <option value="ALL">Todos los proveedores</option>
              {suppliers?.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>

            {/* Toggle Solo Vencidas */}
            <label className="flex cursor-pointer items-center gap-2 rounded-full border border-slate-200 bg-white px-3.5 py-2 text-xs font-medium text-slate-700 hover:bg-slate-50">
              <input
                type="checkbox"
                checked={onlyOverdue}
                onChange={(e) => setOnlyOverdue(e.target.checked)}
                className="size-4 rounded text-teal-700 focus:ring-teal-700"
              />
              <span className={onlyOverdue ? "font-bold text-rose-700" : ""}>
                Solo vencidas
              </span>
            </label>
          </div>

          {/* Filtro de Estado */}
          <div className="flex items-center gap-1.5 overflow-x-auto rounded-full bg-slate-100 p-1">
            {(
              [
                { id: "ALL", label: "Todas" },
                { id: "ABIERTA", label: "Abiertas" },
                { id: "PARCIAL", label: "Parciales" },
                { id: "PAGADA", label: "Pagadas" },
                { id: "CASTIGADA", label: "Castigadas" },
              ] as const
            ).map((tab) => (
              <button
                key={tab.id}
                onClick={() => setSelectedStatus(tab.id)}
                className={cn(
                  "rounded-full px-3.5 py-1.5 text-xs font-semibold transition-colors",
                  selectedStatus === tab.id
                    ? "bg-white text-slate-900 shadow-xs"
                    : "text-slate-600 hover:text-slate-900",
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* Tabla */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-slate-100 bg-slate-50/50 text-xs font-semibold uppercase tracking-wider text-slate-500">
                <th className="py-4 pl-6 pr-4">Documento / Ref</th>
                <th className="px-4 py-4">Proveedor</th>
                <th className="px-4 py-4">Tipo</th>
                <th className="px-4 py-4 text-right">Total</th>
                <th className="px-4 py-4 text-right">Pagado</th>
                <th className="px-4 py-4 text-right">Saldo</th>
                <th className="px-4 py-4">Vencimiento</th>
                <th className="px-4 py-4">Estado</th>
                <th className="py-4 pl-4 pr-6 text-right">Acción</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 text-sm">
              {loadingPayables ? (
                <tr>
                  <td colSpan={9} className="py-12 text-center text-slate-400">
                    <Loader2 className="mx-auto size-6 animate-spin text-teal-700" />
                    <p className="mt-2 text-xs">Cargando cuentas por pagar...</p>
                  </td>
                </tr>
              ) : errorPayables ? (
                <tr>
                  <td colSpan={9} className="py-10 text-center text-rose-500 text-xs">
                    Error al cargar cuentas por pagar: {errorPayables.message}
                  </td>
                </tr>
              ) : filteredPayables.length === 0 ? (
                <tr>
                  <td colSpan={9} className="py-12 text-center text-slate-400 text-xs">
                    No se encontraron cuentas por pagar con los filtros seleccionados.
                  </td>
                </tr>
              ) : (
                filteredPayables.map((p) => {
                  const statusConf = STATUS_CONFIG[p.status];
                  const hasBalance = Number(p.balance) > 0;
                  const canAct = canPay && (p.status === "ABIERTA" || p.status === "PARCIAL");

                  return (
                    <tr
                      key={p.id}
                      className={cn(
                        "hover:bg-slate-50/60 transition-colors",
                        p.overdue && hasBalance ? "bg-rose-50/20" : "",
                      )}
                    >
                      <td className="py-3.5 pl-6 pr-4">
                        <span className="font-semibold text-slate-800">
                          {p.document_number || p.id.slice(0, 8)}
                        </span>
                        <p className="text-xs text-slate-400">
                          {formatDateTime(p.created_at)}
                        </p>
                      </td>

                      <td className="px-4 py-3.5 font-medium text-slate-700">
                        {p.supplier_name || "—"}
                      </td>

                      <td className="px-4 py-3.5">
                        <span className="inline-flex rounded-md bg-slate-100 px-2 py-0.5 text-xs font-semibold text-slate-600">
                          {p.source_type}
                        </span>
                      </td>

                      <td className="px-4 py-3.5 text-right tabular-nums text-slate-600">
                        {formatCOP(Number(p.amount_total))}
                      </td>

                      <td className="px-4 py-3.5 text-right tabular-nums text-emerald-700 font-medium">
                        {formatCOP(Number(p.amount_paid))}
                      </td>

                      <td className="px-4 py-3.5 text-right tabular-nums font-bold text-slate-900">
                        {formatCOP(Number(p.balance))}
                      </td>

                      <td className="px-4 py-3.5">
                        <div className="flex flex-col gap-0.5">
                          <span className="text-xs text-slate-700">{p.due_date}</span>
                          {p.overdue && hasBalance ? (
                            <span className="inline-flex items-center gap-1 text-[11px] font-bold text-rose-700">
                              <AlertTriangle className="size-3 shrink-0" />
                              Vencida ({p.days_overdue}d)
                            </span>
                          ) : (
                            <span className="text-[11px] text-emerald-700 font-medium">
                              Al día
                            </span>
                          )}
                        </div>
                      </td>

                      <td className="px-4 py-3.5">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-semibold",
                            statusConf.className,
                          )}
                        >
                          <span className={cn("size-1.5 rounded-full", statusConf.dot)} />
                          {statusConf.label}
                        </span>
                      </td>

                      <td className="py-3.5 pl-4 pr-6 text-right">
                        <div className="flex items-center justify-end gap-2">
                          {canAct && (
                            <button
                              type="button"
                              onClick={() => openPaymentDialog(p)}
                              className="flex items-center gap-1 rounded-full bg-teal-700 px-3 py-1.5 text-xs font-semibold text-white shadow-xs hover:bg-teal-800 transition-colors"
                            >
                              <DollarSign className="size-3.5" />
                              Pagar
                            </button>
                          )}

                          {isOwner && canAct && (
                            <button
                              type="button"
                              onClick={() => openWriteOffDialog(p)}
                              className="flex items-center gap-1 rounded-full border border-slate-200 px-2.5 py-1.5 text-xs font-medium text-slate-600 hover:bg-rose-50 hover:text-rose-700 hover:border-rose-200 transition-colors"
                              title="Castigar cuenta incobrable"
                            >
                              Castigar
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Diálogo de Pago */}
      {paymentModalPayable && (
        <Dialog open={Boolean(paymentModalPayable)} onOpenChange={(v) => !v && setPaymentModalPayable(null)}>
          <DialogContent className="max-w-md rounded-3xl p-6">
            <DialogHeader>
              <DialogTitle className="text-xl font-bold text-slate-900">
                Registrar pago a proveedor
              </DialogTitle>
              <DialogDescription className="text-xs text-slate-500">
                Abono o liquidación total para {paymentModalPayable.supplier_name || "proveedor"}.
              </DialogDescription>
            </DialogHeader>

            {paymentError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {paymentError}
              </div>
            )}

            <form onSubmit={handleConfirmPayment} className="space-y-4 pt-2">
              <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-3 text-xs space-y-1">
                <div className="flex justify-between text-slate-500">
                  <span>Documento:</span>
                  <span className="font-semibold text-slate-800">
                    {paymentModalPayable.document_number || paymentModalPayable.id.slice(0, 8)}
                  </span>
                </div>
                <div className="flex justify-between text-slate-500">
                  <span>Total cuenta:</span>
                  <span className="tabular-nums font-semibold text-slate-800">
                    {formatCOP(Number(paymentModalPayable.amount_total))}
                  </span>
                </div>
                <div className="flex justify-between text-slate-500">
                  <span>Saldo pendiente:</span>
                  <span className="tabular-nums font-bold text-teal-800">
                    {formatCOP(Number(paymentModalPayable.balance))}
                  </span>
                </div>
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Monto a pagar (COP) *
                </label>
                <MoneyInput
                  value={paymentAmount}
                  onChange={setPaymentAmount}
                  className="mt-1 h-10 text-base font-bold text-teal-900"
                />
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Método de pago *
                </label>
                <div className="mt-1 grid grid-cols-3 gap-2">
                  {(["EFECTIVO", "TRANSFERENCIA", "CHEQUE"] as const).map((m) => (
                    <button
                      key={m}
                      type="button"
                      onClick={() => setPaymentMethod(m)}
                      className={cn(
                        "rounded-xl border py-2 text-xs font-semibold transition-all",
                        paymentMethod === m
                          ? "border-teal-700 bg-teal-50 text-teal-800 shadow-xs"
                          : "border-slate-200 bg-white text-slate-600 hover:bg-slate-50",
                      )}
                    >
                      {m}
                    </button>
                  ))}
                </div>
              </div>

              {paymentMethod === "EFECTIVO" ? (
                <div className="rounded-xl border border-teal-200 bg-teal-50/50 p-2.5 text-xs text-teal-800 flex items-center gap-2">
                  <Banknote className="size-4 shrink-0 text-teal-700" />
                  <span>Se registrará egreso de caja en el turno activo de la sucursal.</span>
                </div>
              ) : (
                <div>
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                    N.° Referencia o Comprobante *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="Ej. TR-092812 o Cheque #4412"
                    value={paymentReference}
                    onChange={(e) => setPaymentReference(e.target.value)}
                    className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                  />
                </div>
              )}

              <div className="flex justify-end gap-2 pt-2 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setPaymentModalPayable(null)}
                  className="rounded-full border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-600 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={paymentBusy}
                  className="flex items-center gap-1.5 rounded-full bg-teal-700 px-5 py-2 text-xs font-semibold text-white shadow-sm hover:bg-teal-800 transition-colors disabled:opacity-50"
                >
                  {paymentBusy && <Loader2 className="size-3.5 animate-spin" />}
                  Confirmar pago
                </button>
              </div>
            </form>
          </DialogContent>
        </Dialog>
      )}

      {/* Diálogo de Castigo de Cartera / CxP */}
      {writeOffModalPayable && (
        <Dialog open={Boolean(writeOffModalPayable)} onOpenChange={(v) => !v && setWriteOffModalPayable(null)}>
          <DialogContent className="max-w-md rounded-3xl p-6">
            <DialogHeader>
              <DialogTitle className="text-xl font-bold text-slate-900">
                Castigar cuenta por pagar
              </DialogTitle>
              <DialogDescription className="text-xs text-slate-500">
                Esta acción marca la cuenta como CASTIGADA e inactiva para pagos. Solo realizable por el dueño.
              </DialogDescription>
            </DialogHeader>

            {writeOffError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {writeOffError}
              </div>
            )}

            <form onSubmit={handleConfirmWriteOff} className="space-y-4 pt-2">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Motivo o justificación del castigo *
                </label>
                <textarea
                  required
                  rows={3}
                  placeholder="Ej. Proveedor cerró operaciones / condonación formal"
                  value={writeOffReason}
                  onChange={(e) => setWriteOffReason(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setWriteOffModalPayable(null)}
                  className="rounded-full border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-600 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={writeOffBusy}
                  className="flex items-center gap-1.5 rounded-full bg-rose-600 px-5 py-2 text-xs font-semibold text-white shadow-sm hover:bg-rose-700 transition-colors disabled:opacity-50"
                >
                  {writeOffBusy && <Loader2 className="size-3.5 animate-spin" />}
                  Confirmar castigo
                </button>
              </div>
            </form>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

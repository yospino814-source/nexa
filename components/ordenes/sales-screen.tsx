"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { ChevronLeft, ChevronRight, Loader2, Printer, RotateCcw, Search, X, XCircle } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { PrintTicket, ticketFromSale } from "@/components/pos/print-ticket";
import {
  api,
  friendlyError,
  type BranchSettings,
  type PaymentMethod,
  type SaleHistoryQuery,
  type SaleHistoryRow,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useBusinessInfo } from "@/lib/api/use-business";
import { formatCOP, formatTime } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { hasRefundable, RefundDialog } from "./refund-dialog";

const METHOD_LABELS: Record<PaymentMethod, string> = {
  EFECTIVO: "Efectivo",
  DATAFONO: "Datáfono",
  QR: "QR",
  CREDITO: "Crédito",
};

const PAGE_SIZE = 50;

/** Fecha local de hoy en formato YYYY-MM-DD. */
function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function StatusBadge({ row }: { row: Pick<SaleHistoryRow, "status" | "refunded_amount"> }) {
  if (row.status === "CANCELADA") {
    return <span className="rounded-full bg-rose-50 px-2 py-0.5 text-xs font-semibold text-rose-700">Anulada</span>;
  }
  if (Number(row.refunded_amount) > 0) {
    return (
      <span className="rounded-full bg-amber-50 px-2 py-0.5 text-xs font-semibold text-amber-700">Con devolución</span>
    );
  }
  return <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-semibold text-emerald-700">Cobrada</span>;
}

/** Pantalla 04: historial de ventas (F1.13). */
export function SalesScreen() {
  const branch = useCurrentBranch();
  const can = useSession((s) => s.can);
  const seeAll = can("reports:read");

  const [from, setFrom] = useState(today);
  const [to, setTo] = useState(today);
  const [cashierId, setCashierId] = useState("");
  const [method, setMethod] = useState<PaymentMethod | "">("");
  const [status, setStatus] = useState<"" | "COBRADA" | "CANCELADA">("");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const query: SaleHistoryQuery = {
    branch_id: branch.id,
    from,
    to,
    cashier_id: seeAll && cashierId ? cashierId : null,
    method: method || null,
    status: status || null,
    q: q.trim() || null,
    page,
    page_size: PAGE_SIZE,
  };
  const key = JSON.stringify(query);
  const { data, loading, error, reload } = useApi(() => api.sales.history(query), key);

  // Cajeros para el filtro: los que aparecen en las ventas cargadas.
  const knownCashiers = useMemo(() => {
    const map = new Map<string, string>();
    for (const row of data?.items ?? []) {
      if (row.cashier_id && row.cashier_name) map.set(row.cashier_id, row.cashier_name);
    }
    return map;
  }, [data]);

  const totalPages = Math.max(1, Math.ceil((data?.total_count ?? 0) / PAGE_SIZE));

  function resetPage<T>(setter: (v: T) => void) {
    return (v: T) => {
      setter(v);
      setPage(1);
    };
  }

  return (
    <div className="flex-1 space-y-5 overflow-y-auto p-4 lg:p-8">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Órdenes</h1>
          <p className="text-sm text-slate-500">
            {seeAll ? "Ventas de la sucursal" : "Tus ventas"} · {data?.totals.count ?? 0} ventas por{" "}
            {formatCOP(Number(data?.totals.amount ?? 0))}
          </p>
        </div>
        {can("pos:refund") && (
          <Link
            href="/ordenes/devoluciones"
            className="flex h-10 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
          >
            <RotateCcw className="size-4" /> Devoluciones
          </Link>
        )}
      </div>

      <div className="flex flex-wrap items-end gap-3 rounded-2xl border border-slate-200 bg-white p-4">
        <label className="space-y-1 text-xs font-medium text-slate-500">
          Desde
          <input
            type="date"
            value={from}
            onChange={(e) => resetPage(setFrom)(e.target.value)}
            className="block h-10 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
          />
        </label>
        <label className="space-y-1 text-xs font-medium text-slate-500">
          Hasta
          <input
            type="date"
            value={to}
            onChange={(e) => resetPage(setTo)(e.target.value)}
            className="block h-10 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
          />
        </label>
        {seeAll && (
          <label className="space-y-1 text-xs font-medium text-slate-500">
            Cajero
            <select
              value={cashierId}
              onChange={(e) => resetPage(setCashierId)(e.target.value)}
              className="block h-10 rounded-xl border border-slate-200 bg-white px-3 text-sm text-slate-900"
            >
              <option value="">Todos</option>
              {[...knownCashiers.entries()].map(([id, name]) => (
                <option key={id} value={id}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="space-y-1 text-xs font-medium text-slate-500">
          Método
          <select
            value={method}
            onChange={(e) => resetPage(setMethod)(e.target.value as PaymentMethod | "")}
            className="block h-10 rounded-xl border border-slate-200 bg-white px-3 text-sm text-slate-900"
          >
            <option value="">Todos</option>
            {(Object.keys(METHOD_LABELS) as PaymentMethod[]).map((m) => (
              <option key={m} value={m}>
                {METHOD_LABELS[m]}
              </option>
            ))}
          </select>
        </label>
        <label className="space-y-1 text-xs font-medium text-slate-500">
          Estado
          <select
            value={status}
            onChange={(e) => resetPage(setStatus)(e.target.value as "" | "COBRADA" | "CANCELADA")}
            className="block h-10 rounded-xl border border-slate-200 bg-white px-3 text-sm text-slate-900"
          >
            <option value="">Todos</option>
            <option value="COBRADA">Cobrada</option>
            <option value="CANCELADA">Anulada</option>
          </select>
        </label>
        <label className="flex-1 space-y-1 text-xs font-medium text-slate-500">
          Número
          <span className="flex h-10 items-center gap-2 rounded-xl border border-slate-200 px-3">
            <Search className="size-4 text-slate-400" />
            <input
              value={q}
              onChange={(e) => resetPage(setQ)(e.target.value)}
              placeholder="POS-000123 o 123"
              className="w-full bg-transparent text-sm text-slate-900 outline-none"
            />
          </span>
        </label>
      </div>

      {toast && (
        <p className="flex items-center justify-between rounded-xl bg-emerald-50 p-3 text-sm text-emerald-700">
          {toast}
          <button onClick={() => setToast(null)} aria-label="Cerrar aviso">
            <X className="size-4" />
          </button>
        </p>
      )}
      {error && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{friendlyError(error)}</p>}

      <div className="overflow-x-auto rounded-2xl border border-slate-200 bg-white">
        <table className="w-full text-sm">
          <thead className="bg-slate-50 text-left text-xs font-semibold text-slate-500 uppercase">
            <tr>
              <th className="px-4 py-3">Número</th>
              <th className="px-4 py-3">Hora</th>
              <th className="px-4 py-3">Cliente</th>
              <th className="px-4 py-3">Cajero</th>
              <th className="px-4 py-3">Métodos</th>
              <th className="px-4 py-3 text-right">Total</th>
              <th className="px-4 py-3 text-right">Devuelto</th>
              <th className="px-4 py-3">Estado</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {loading && !data ? (
              <tr>
                <td colSpan={8} className="px-4 py-10 text-center text-slate-400">
                  <Loader2 className="mx-auto size-5 animate-spin" />
                </td>
              </tr>
            ) : data && data.items.length === 0 ? (
              <tr>
                <td colSpan={8} className="px-4 py-10 text-center text-slate-400">
                  No hay ventas con estos filtros.
                </td>
              </tr>
            ) : (
              data?.items.map((row) => (
                <tr
                  key={row.id}
                  onClick={() => setSelected(row.id)}
                  className="cursor-pointer hover:bg-slate-50"
                >
                  <td className="px-4 py-3 font-mono font-semibold">{row.receipt_number}</td>
                  <td className="px-4 py-3 tabular-nums">{formatTime(row.created_at)}</td>
                  <td className="px-4 py-3">{row.customer_label}</td>
                  <td className="px-4 py-3">{row.cashier_name ?? "—"}</td>
                  <td className="px-4 py-3">{row.methods.map((m) => METHOD_LABELS[m] ?? m).join(", ")}</td>
                  <td className="px-4 py-3 text-right font-semibold tabular-nums">{formatCOP(Number(row.total))}</td>
                  <td className="px-4 py-3 text-right tabular-nums">
                    {Number(row.refunded_amount) > 0 ? formatCOP(Number(row.refunded_amount)) : "—"}
                  </td>
                  <td className="px-4 py-3">
                    <StatusBadge row={row} />
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {totalPages > 1 && (
        <div className="flex items-center justify-end gap-2 text-sm">
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page <= 1}
            aria-label="Página anterior"
            className="flex size-9 items-center justify-center rounded-full border border-slate-200 disabled:opacity-40"
          >
            <ChevronLeft className="size-4" />
          </button>
          <span className="tabular-nums">
            {page} / {totalPages}
          </span>
          <button
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page >= totalPages}
            aria-label="Página siguiente"
            className="flex size-9 items-center justify-center rounded-full border border-slate-200 disabled:opacity-40"
          >
            <ChevronRight className="size-4" />
          </button>
        </div>
      )}

      <SaleDetailDialog
        orderId={selected}
        onClose={() => setSelected(null)}
        onChanged={(message) => {
          setToast(message);
          reload();
        }}
      />
    </div>
  );
}

function SaleDetailDialog({
  orderId,
  onClose,
  onChanged,
}: {
  orderId: string | null;
  onClose: () => void;
  onChanged: (message: string) => void;
}) {
  return (
    <Dialog open={orderId !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-xl">
        {orderId && <SaleDetailBody key={orderId} orderId={orderId} onChanged={onChanged} />}
      </DialogContent>
    </Dialog>
  );
}

function SaleDetailBody({ orderId, onChanged }: { orderId: string; onChanged: (message: string) => void }) {
  const branch = useCurrentBranch();
  const can = useSession((s) => s.can);
  const business = useBusinessInfo();
  const { data: detail, loading, error, reload } = useApi(() => api.sales.detail(orderId), orderId);
  const { data: settings } = useApi<BranchSettings | null>(
    () => api.settings.get(branch.id).catch(() => null),
    `settings:${branch.id}`,
  );
  const [refundOpen, setRefundOpen] = useState(false);
  const [voidOpen, setVoidOpen] = useState(false);
  const [voidReason, setVoidReason] = useState("");
  const [voiding, setVoiding] = useState(false);
  const [voidError, setVoidError] = useState<string | null>(null);

  if (loading && !detail) {
    return <Loader2 className="mx-auto my-10 size-6 animate-spin text-slate-400" />;
  }
  if (error || !detail) {
    return <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{friendlyError(error)}</p>;
  }

  const cancelled = detail.order.status === "CANCELADA";
  const ticket = ticketFromSale(detail, {
    cashier: detail.cashier_name,
    register: detail.register_name,
    customer: detail.customer?.name ?? detail.order.customer_alias ?? "Consumidor final",
    waiter: detail.waiter_name,
  });

  async function confirmVoid() {
    setVoiding(true);
    setVoidError(null);
    try {
      await api.sales.void(orderId, voidReason.trim());
      setVoidOpen(false);
      onChanged(`Venta ${detail?.receipt_number} anulada.`);
      reload();
    } catch (e) {
      setVoidError(friendlyError(e));
    } finally {
      setVoiding(false);
    }
  }

  return (
    <div className="space-y-5">
      <PrintTicket
        model={ticket}
        business={business}
        branchName={branch.name}
        widthMm={settings?.print.ticket_width_mm ?? 80}
      />
      <div className="flex items-start justify-between gap-3">
        <div>
          <DialogTitle className="text-xl font-bold">Venta {detail.receipt_number}</DialogTitle>
          <p className="text-sm text-slate-500">
            {ticket.customer} · {detail.cashier_name ?? "—"}
            {detail.register_name ? ` · ${detail.register_name}` : ""}
          </p>
        </div>
        <StatusBadge row={{ status: detail.order.status, refunded_amount: detail.order.refunded_amount ?? "0" }} />
      </div>

      <ul className="divide-y divide-slate-100 rounded-2xl border border-slate-200 text-sm">
        {detail.items.map((it) => (
          <li key={it.id} className="flex items-center justify-between gap-3 px-4 py-2.5">
            <div className="min-w-0">
              <p className="truncate font-medium">{it.name ?? it.custom_name}</p>
              <p className="text-xs text-slate-500">
                {Number(it.qty)} x {formatCOP(Number(it.unit_price))}
                {Number(it.line_discount ?? 0) > 0 && ` · descuento ${formatCOP(Number(it.line_discount))}`}
              </p>
            </div>
            <span className="font-semibold tabular-nums">{formatCOP(Number(it.line_total))}</span>
          </li>
        ))}
      </ul>

      <dl className="space-y-1 text-sm">
        <div className="flex justify-between text-slate-500">
          <dt>Subtotal</dt>
          <dd className="tabular-nums">{formatCOP(ticket.subtotal)}</dd>
        </div>
        {ticket.taxes.map((t) => (
          <div key={t.label} className="flex justify-between text-slate-500">
            <dt>{t.label}</dt>
            <dd className="tabular-nums">{formatCOP(t.amount)}</dd>
          </div>
        ))}
        {ticket.tip > 0 && (
          <div className="flex justify-between text-slate-500">
            <dt>Propina</dt>
            <dd className="tabular-nums">{formatCOP(ticket.tip)}</dd>
          </div>
        )}
        <div className="flex justify-between font-bold">
          <dt>Total</dt>
          <dd className="tabular-nums">{formatCOP(ticket.total)}</dd>
        </div>
        {detail.payments.map((p) => (
          <div key={p.id} className="flex justify-between text-slate-500">
            <dt>{METHOD_LABELS[p.method] ?? p.method}</dt>
            <dd className="tabular-nums">{formatCOP(Number(p.amount))}</dd>
          </div>
        ))}
        <div className="flex justify-between text-slate-500">
          <dt>Documento electrónico</dt>
          <dd>{detail.fiscal_status}</dd>
        </div>
      </dl>

      {detail.refunds.length > 0 && (
        <div className="space-y-2">
          <p className="text-xs font-semibold tracking-wider text-slate-500 uppercase">Devoluciones</p>
          {detail.refunds.map((r) => (
            <div key={r.id} className="flex justify-between rounded-xl bg-slate-50 px-3 py-2 text-sm">
              <span>
                {r.number} · {r.status.toLowerCase()}
              </span>
              <span className="tabular-nums">{formatCOP(Number(r.total))}</span>
            </div>
          ))}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <button
          onClick={() => window.print()}
          className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
        >
          <Printer className="size-4" /> Reimprimir
        </button>
        {!cancelled && can("pos:sell") && hasRefundable(detail) && (
          <button
            onClick={() => setRefundOpen(true)}
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
          >
            <RotateCcw className="size-4" /> Devolver
          </button>
        )}
        {!cancelled && can("pos:refund") && (
          <button
            onClick={() => setVoidOpen((v) => !v)}
            className="flex h-11 items-center gap-2 rounded-full border border-rose-200 px-4 text-sm font-medium text-rose-600 hover:bg-rose-50"
          >
            <XCircle className="size-4" /> Anular
          </button>
        )}
      </div>

      {voidOpen && (
        <div className="space-y-3 rounded-2xl border border-rose-200 bg-rose-50/50 p-4">
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">Motivo de la anulación</span>
            <input
              value={voidReason}
              onChange={(e) => setVoidReason(e.target.value)}
              maxLength={200}
              className="h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm"
            />
          </label>
          {voidError && <p className="text-sm text-rose-600">{voidError}</p>}
          <button
            onClick={confirmVoid}
            disabled={voiding || voidReason.trim().length < 3}
            className={cn(
              "flex h-10 items-center gap-2 rounded-full bg-rose-600 px-4 text-sm font-semibold text-white",
              "disabled:opacity-40",
            )}
          >
            {voiding && <Loader2 className="size-4 animate-spin" />}
            Confirmar anulación
          </button>
        </div>
      )}

      <RefundDialog
        detail={detail}
        open={refundOpen}
        onOpenChange={setRefundOpen}
        onDone={(message) => {
          onChanged(message);
          reload();
        }}
      />
    </div>
  );
}

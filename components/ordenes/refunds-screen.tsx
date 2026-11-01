"use client";

import { useState } from "react";
import { Check, Loader2, X } from "lucide-react";

import { api, friendlyError, type Refund, type RefundStatus } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatDateTime } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

const TABS: { status: RefundStatus; label: string }[] = [
  { status: "PENDIENTE", label: "Pendientes" },
  { status: "APROBADA", label: "Aprobadas" },
  { status: "RECHAZADA", label: "Rechazadas" },
];

const METHOD_LABELS: Record<string, string> = {
  EFECTIVO: "Efectivo",
  ORIGINAL: "Mismo medio de pago",
};

/** Pantalla 36: devoluciones de la sucursal (F1.11, permiso pos:refund). */
export function RefundsScreen() {
  const can = useSession((s) => s.can);
  if (!can("pos:refund")) {
    return (
      <p className="m-8 rounded-2xl border border-dashed border-slate-200 p-10 text-center text-sm text-slate-500">
        No tenés permiso para ver las devoluciones.
      </p>
    );
  }
  return <RefundsList />;
}

function RefundsList() {
  const branch = useCurrentBranch();
  const [tab, setTab] = useState<RefundStatus>("PENDIENTE");
  const { data, loading, error, reload } = useApi(
    () => api.refunds.list(branch.id, tab),
    `${branch.id}:${tab}`,
  );
  const [busy, setBusy] = useState<string | null>(null);
  const [rejecting, setRejecting] = useState<string | null>(null);
  const [rejectReason, setRejectReason] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  async function approve(r: Refund) {
    setBusy(r.id);
    setActionError(null);
    try {
      const done = await api.refunds.approve(r.id);
      setToast(`Devolución ${done.number} aprobada.`);
      reload();
    } catch (e) {
      setActionError(friendlyError(e));
    } finally {
      setBusy(null);
    }
  }

  async function reject(r: Refund) {
    setBusy(r.id);
    setActionError(null);
    try {
      const done = await api.refunds.reject(r.id, rejectReason.trim());
      setToast(`Devolución ${done.number} rechazada.`);
      setRejecting(null);
      setRejectReason("");
      reload();
    } catch (e) {
      setActionError(friendlyError(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="flex-1 space-y-5 overflow-y-auto p-4 lg:p-8">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Devoluciones</h1>
        <p className="text-sm text-slate-500">Aprobá o rechazá las devoluciones de la sucursal.</p>
      </div>

      <div className="flex gap-2">
        {TABS.map((t) => (
          <button
            key={t.status}
            onClick={() => setTab(t.status)}
            className={cn(
              "rounded-full px-4 py-2 text-sm font-medium",
              tab === t.status ? "bg-brand text-white" : "border border-slate-200 hover:bg-slate-50",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {toast && (
        <p className="flex items-center justify-between rounded-xl bg-emerald-50 p-3 text-sm text-emerald-700">
          {toast}
          <button onClick={() => setToast(null)} aria-label="Cerrar aviso">
            <X className="size-4" />
          </button>
        </p>
      )}
      {(error || actionError) && (
        <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{actionError ?? friendlyError(error)}</p>
      )}

      {loading && !data ? (
        <Loader2 className="mx-auto my-10 size-6 animate-spin text-slate-400" />
      ) : data && data.length === 0 ? (
        <p className="rounded-2xl border border-dashed border-slate-200 p-10 text-center text-sm text-slate-400">
          No hay devoluciones en esta pestaña.
        </p>
      ) : (
        <div className="space-y-3">
          {data?.map((r) => (
            <div key={r.id} className="space-y-3 rounded-2xl border border-slate-200 bg-white p-4">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p className="font-semibold">
                    {r.number} · venta {r.receipt_number ?? "—"}
                  </p>
                  <p className="text-xs text-slate-500">
                    {formatDateTime(r.created_at)} · pidió {r.requested_by_name ?? "—"} ·{" "}
                    {METHOD_LABELS[r.method] ?? r.method}
                    {r.reversal_reference ? ` (ref. ${r.reversal_reference})` : ""}
                  </p>
                  <p className="mt-1 text-sm">Motivo: {r.reason}</p>
                  {r.reject_reason && <p className="text-sm text-rose-600">Rechazo: {r.reject_reason}</p>}
                </div>
                <span className="text-xl font-extrabold tabular-nums">{formatCOP(Number(r.total))}</span>
              </div>
              <ul className="text-sm text-slate-600">
                {r.items.map((it) => (
                  <li key={it.order_item_id}>
                    {Number(it.qty)} x {it.name} ·{" "}
                    {it.disposition === "REINTEGRAR"
                      ? "vuelve al inventario"
                      : it.disposition === "MERMA"
                        ? "merma"
                        : "no se recibe"}
                  </li>
                ))}
              </ul>
              {r.status === "PENDIENTE" && (
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    onClick={() => approve(r)}
                    disabled={busy === r.id}
                    className="flex h-10 items-center gap-2 rounded-full bg-brand px-4 text-sm font-semibold text-white disabled:opacity-40"
                  >
                    {busy === r.id ? <Loader2 className="size-4 animate-spin" /> : <Check className="size-4" />}
                    Aprobar
                  </button>
                  {rejecting === r.id ? (
                    <>
                      <input
                        value={rejectReason}
                        onChange={(e) => setRejectReason(e.target.value)}
                        maxLength={200}
                        placeholder="Motivo del rechazo"
                        className="h-10 flex-1 rounded-xl border border-slate-200 px-3 text-sm"
                      />
                      <button
                        onClick={() => reject(r)}
                        disabled={busy === r.id || rejectReason.trim().length < 3}
                        className="h-10 rounded-full bg-rose-600 px-4 text-sm font-semibold text-white disabled:opacity-40"
                      >
                        Confirmar rechazo
                      </button>
                    </>
                  ) : (
                    <button
                      onClick={() => setRejecting(r.id)}
                      className="flex h-10 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
                    >
                      <X className="size-4" /> Rechazar
                    </button>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

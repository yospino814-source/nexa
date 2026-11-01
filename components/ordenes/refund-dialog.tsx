"use client";

import { useMemo, useState } from "react";
import { Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type RefundDisposition,
  type RefundMethod,
  type SaleDetail,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";

const DISPOSITIONS: { value: RefundDisposition; label: string }[] = [
  { value: "REINTEGRAR", label: "Vuelve al inventario" },
  { value: "MERMA", label: "Merma" },
  { value: "SIN_DEVOLUCION", label: "No se recibe" },
];

const cents = (n: number) => Math.round(n * 100) / 100;

type LineState = { qty: number; disposition: RefundDisposition };

/** Lo ya devuelto (pendiente o aprobado) por línea, para el techo y los montos. */
function alreadyRefunded(detail: SaleDetail) {
  const map = new Map<string, { qty: number; total: number; tax: number }>();
  for (const r of detail.refunds) {
    if (r.status === "RECHAZADA") continue;
    for (const it of r.items) {
      const acc = map.get(it.order_item_id) ?? { qty: 0, total: 0, tax: 0 };
      acc.qty += Number(it.qty);
      acc.total += Number(it.line_total);
      acc.tax += Number(it.tax_amount);
      map.set(it.order_item_id, acc);
    }
  }
  return map;
}

/** Lo que queda por devolver de una venta (para mostrar u ocultar "Devolver"). */
export function hasRefundable(detail: SaleDetail): boolean {
  const prev = alreadyRefunded(detail);
  return detail.items.some((it) => Number(it.qty) - (prev.get(it.id)?.qty ?? 0) > 0);
}

/** F1.11: devolución de líneas de una venta. */
export function RefundDialog({
  detail,
  open,
  onOpenChange,
  onDone,
}: {
  detail: SaleDetail;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDone: (message: string) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-2xl">
        {open && <RefundForm detail={detail} onClose={() => onOpenChange(false)} onDone={onDone} />}
      </DialogContent>
    </Dialog>
  );
}

function RefundForm({
  detail,
  onClose,
  onDone,
}: {
  detail: SaleDetail;
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const prev = useMemo(() => alreadyRefunded(detail), [detail]);
  const singleMethod = detail.payments.length === 1 ? detail.payments[0].method : null;
  const [refundId] = useState(() => crypto.randomUUID());
  const [lines, setLines] = useState<Record<string, LineState>>(() =>
    Object.fromEntries(
      detail.items.map((it) => [
        it.id,
        { qty: 0, disposition: it.product_id ? "REINTEGRAR" : "SIN_DEVOLUCION" } as LineState,
      ]),
    ),
  );
  const [method, setMethod] = useState<RefundMethod>("EFECTIVO");
  const [reference, setReference] = useState("");
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Mismo cálculo que el backend: lo último que queda se devuelve exacto.
  const total = detail.items.reduce((sum, it) => {
    const st = lines[it.id];
    if (!st || st.qty <= 0) return sum;
    const sold = Number(it.qty);
    const p = prev.get(it.id) ?? { qty: 0, total: 0, tax: 0 };
    const refundable = sold - p.qty;
    const full = st.qty === refundable;
    const lineTotal = full ? Number(it.line_total) - p.total : cents((Number(it.line_total) * st.qty) / sold);
    const tax = full ? Number(it.tax_amount) - p.tax : cents((Number(it.tax_amount) * st.qty) / sold);
    return sum + lineTotal + tax;
  }, 0);

  const needsReference = method === "ORIGINAL" && singleMethod !== null && singleMethod !== "EFECTIVO";
  const reasonOk = reason.trim().length >= 3 && reason.trim().length <= 200;
  const referenceOk = !needsReference || (reference.trim().length >= 3 && reference.trim().length <= 60);
  const canSubmit = !saving && total > 0 && reasonOk && referenceOk;

  async function submit() {
    if (!canSubmit) return;
    setSaving(true);
    setError(null);
    try {
      const refund = await api.refunds.create({
        id: refundId,
        order_id: detail.order.id,
        method,
        reason: reason.trim(),
        reversal_reference: needsReference ? reference.trim() : null,
        items: detail.items
          .filter((it) => (lines[it.id]?.qty ?? 0) > 0)
          .map((it) => ({
            order_item_id: it.id,
            qty: String(lines[it.id].qty),
            disposition: lines[it.id].disposition,
          })),
      });
      onDone(
        refund.status === "PENDIENTE"
          ? `Devolución ${refund.number} creada. Quedó pendiente de aprobación.`
          : `Devolución ${refund.number} aprobada por ${formatCOP(Number(refund.total))}.`,
      );
      onClose();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-5">
      <div>
        <DialogTitle className="text-xl font-bold">Devolver productos</DialogTitle>
        <p className="mt-1 text-sm text-slate-500">Venta {detail.receipt_number}</p>
      </div>

      <div className="overflow-x-auto rounded-2xl border border-slate-200">
        <table className="w-full text-sm">
          <thead className="bg-slate-50 text-left text-xs font-semibold text-slate-500 uppercase">
            <tr>
              <th className="px-3 py-2">Producto</th>
              <th className="px-3 py-2 text-right">Vendido</th>
              <th className="px-3 py-2 text-right">Ya devuelto</th>
              <th className="px-3 py-2">A devolver</th>
              <th className="px-3 py-2">Mercancía</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {detail.items.map((it) => {
              const sold = Number(it.qty);
              const done = prev.get(it.id)?.qty ?? 0;
              const max = sold - done;
              const st = lines[it.id];
              return (
                <tr key={it.id}>
                  <td className="px-3 py-2">{it.name ?? it.custom_name}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{sold}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{done}</td>
                  <td className="px-3 py-2">
                    <input
                      type="number"
                      min={0}
                      max={max}
                      step="any"
                      disabled={max <= 0}
                      value={st.qty}
                      onChange={(e) => {
                        const v = Math.min(Math.max(Number(e.target.value) || 0, 0), max);
                        setLines((l) => ({ ...l, [it.id]: { ...l[it.id], qty: v } }));
                      }}
                      className="h-9 w-20 rounded-lg border border-slate-200 px-2 tabular-nums disabled:bg-slate-50"
                    />
                  </td>
                  <td className="px-3 py-2">
                    <select
                      value={st.disposition}
                      disabled={!it.product_id || max <= 0}
                      onChange={(e) =>
                        setLines((l) => ({
                          ...l,
                          [it.id]: { ...l[it.id], disposition: e.target.value as RefundDisposition },
                        }))
                      }
                      className="h-9 rounded-lg border border-slate-200 bg-white px-2"
                    >
                      {DISPOSITIONS.filter((d) => it.product_id || d.value === "SIN_DEVOLUCION").map((d) => (
                        <option key={d.value} value={d.value}>
                          {d.label}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <label className="block space-y-1.5">
          <span className="text-sm font-medium">Cómo se devuelve el dinero</span>
          <select
            value={method}
            onChange={(e) => setMethod(e.target.value as RefundMethod)}
            className="h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm"
          >
            <option value="EFECTIVO">Efectivo</option>
            {singleMethod && <option value="ORIGINAL">Mismo medio de pago</option>}
          </select>
        </label>
        {needsReference && (
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">Referencia del reverso</span>
            <input
              value={reference}
              onChange={(e) => setReference(e.target.value)}
              maxLength={60}
              className="h-11 w-full rounded-xl border border-slate-200 px-3 text-sm"
            />
          </label>
        )}
      </div>

      <label className="block space-y-1.5">
        <span className="text-sm font-medium">Motivo</span>
        <input
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          maxLength={200}
          placeholder="Ej.: producto defectuoso"
          className="h-11 w-full rounded-xl border border-slate-200 px-3 text-sm"
        />
      </label>

      <div className="flex items-center justify-between rounded-2xl bg-slate-50 p-4">
        <span className="text-sm font-semibold text-slate-600">Total a devolver</span>
        <span className="text-2xl font-extrabold tabular-nums">{formatCOP(cents(total))}</span>
      </div>

      {error && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onClose}
          className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium hover:bg-slate-50"
        >
          Cancelar
        </button>
        <button
          type="button"
          onClick={submit}
          disabled={!canSubmit}
          className="flex items-center gap-2 rounded-full bg-brand px-5 py-2 text-sm font-semibold text-white disabled:opacity-40"
        >
          {saving && <Loader2 className="size-4 animate-spin" />}
          Confirmar devolución
        </button>
      </div>
    </div>
  );
}

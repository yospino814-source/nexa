"use client";

import { useState } from "react";
import { ArrowLeftRight, Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type BranchAccess, type StockLevel } from "@/lib/api";
import { formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

/** Traslado de unidades a otra sucursal del negocio. */
export function TransferDialog({
  item,
  from,
  destinations,
  onClose,
  onSaved,
}: {
  item: StockLevel | null;
  from: BranchAccess;
  /** Sucursales donde el usuario administra inventario (sin la actual). */
  destinations: BranchAccess[];
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [to, setTo] = useState(destinations[0]?.id ?? "");
  const [qty, setQty] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [lastItem, setLastItem] = useState(item?.product_id);
  if (item?.product_id !== lastItem) {
    setLastItem(item?.product_id);
    setTo(destinations[0]?.id ?? "");
    setQty("");
    setNotes("");
    setError(null);
  }

  if (!item) return <Dialog open={false} />;
  const n = Number(qty.replace(",", "."));
  const available = Number(item.available);
  const valid = n > 0 && n <= available && !!to;
  const toName = destinations.find((d) => d.id === to)?.name ?? "";

  async function save() {
    if (!item) return;
    setBusy(true);
    setError(null);
    try {
      await api.inventory.transfer(from.id, to, item.product_id, String(n), notes.trim() || `Traslado a ${toName}`);
      onSaved(`${formatQty(n)} × ${item.name} trasladadas a ${toName}`);
      onClose();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="gap-0 rounded-3xl p-6 sm:max-w-md">
        <div className="flex items-start gap-3">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-sky-50 text-sky-600">
            <ArrowLeftRight className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-lg font-bold">Trasladar a otra sucursal</DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              {item.name} · disponible en {from.name}: <b>{formatQty(available)}</b>
            </DialogDescription>
          </div>
        </div>

        {destinations.length === 0 ? (
          <p className="mt-6 rounded-2xl bg-slate-50 p-4 text-sm text-slate-600">
            No administrás inventario en otra sucursal. Agregá una sucursal o pedile permiso al dueño.
          </p>
        ) : (
          <>
            <p className="mt-6 text-sm font-medium text-slate-700">Sucursal de destino</p>
            <div className="mt-2 grid gap-2">
              {destinations.map((d) => (
                <button
                  key={d.id}
                  onClick={() => setTo(d.id)}
                  className={cn(
                    "rounded-2xl border px-4 py-3 text-left text-sm font-medium",
                    to === d.id ? "border-brand bg-brand/10 text-brand" : "border-slate-200 hover:bg-slate-50",
                  )}
                >
                  {d.name}
                </button>
              ))}
            </div>

            <label htmlFor="tqty" className="mt-5 text-sm font-medium text-slate-700">
              Unidades a trasladar
            </label>
            <input
              id="tqty"
              autoFocus
              inputMode="decimal"
              value={qty}
              onChange={(e) => setQty(e.target.value.replace(/[^\d,.]/g, ""))}
              className="mt-2 h-12 rounded-2xl border border-slate-200 text-center text-xl font-bold tabular-nums outline-none focus:ring-2 focus:ring-brand/40"
            />
            {n > available && <p className="mt-2 text-sm text-rose-600">Supera lo disponible.</p>}

            <input
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Nota (opcional): quién lo lleva, remisión…"
              className="mt-4 h-11 rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
            />
          </>
        )}

        {error && <p className="mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

        <div className="mt-6 flex justify-end gap-3">
          <button onClick={onClose} className="h-11 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50">
            Cancelar
          </button>
          <button
            onClick={save}
            disabled={!valid || busy}
            className="flex h-11 items-center gap-2 rounded-xl bg-brand px-5 text-sm font-semibold text-white hover:bg-brand-dark disabled:opacity-50"
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            Trasladar
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

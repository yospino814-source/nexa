"use client";

import { useState } from "react";

import { MoneyInput } from "@/components/caja/money-input";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { formatCOP } from "@/lib/format";
import { useCart, type CartLine } from "@/lib/stores/cart";

/** F1.9: cambio de precio de una línea, con motivo obligatorio. */
export function PriceOverrideDialog({
  line,
  onClose,
}: {
  line: CartLine | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={line !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="rounded-3xl p-6 sm:max-w-md">
        {line && <PriceOverrideForm key={line.key} line={line} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function PriceOverrideForm({ line, onClose }: { line: CartLine; onClose: () => void }) {
  const setPriceOverride = useCart((s) => s.setPriceOverride);
  const [price, setPrice] = useState<number>(line.priceOverride ?? line.unitPrice);
  const [reason, setReason] = useState(line.overrideReason ?? "");
  const trimmed = reason.trim();
  const reasonOk = trimmed.length >= 3 && trimmed.length <= 120;
  const canSave = price > 0 && reasonOk;

  return (
    <form
      className="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();
        if (!canSave) return;
        setPriceOverride(line.key, price, trimmed);
        onClose();
      }}
    >
      <div>
        <DialogTitle className="text-xl font-bold">Cambiar precio</DialogTitle>
        <p className="mt-1 text-sm text-slate-500">
          {line.name} · precio actual {formatCOP(line.unitPrice)} sin IVA
        </p>
      </div>
      <label className="block space-y-1.5">
        <span className="text-sm font-medium">Nuevo precio sin IVA</span>
        <MoneyInput value={price} onChange={setPrice} autoFocus />
      </label>
      <label className="block space-y-1.5">
        <span className="text-sm font-medium">Motivo</span>
        <input
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          maxLength={120}
          placeholder="Ej.: producto con empaque dañado"
          className="h-11 w-full rounded-xl border border-slate-200 px-3 text-sm focus:border-brand focus:outline-none"
        />
        {!reasonOk && reason.length > 0 && (
          <span className="text-xs text-rose-600">El motivo debe tener entre 3 y 120 caracteres.</span>
        )}
      </label>
      <div className="flex justify-end gap-2">
        {line.priceOverride != null && (
          <button
            type="button"
            onClick={() => {
              setPriceOverride(line.key, null, null);
              onClose();
            }}
            className="mr-auto rounded-full px-4 py-2 text-sm font-medium text-rose-600 hover:bg-rose-50"
          >
            Quitar cambio
          </button>
        )}
        <button
          type="button"
          onClick={onClose}
          className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium hover:bg-slate-50"
        >
          Cancelar
        </button>
        <button
          type="submit"
          disabled={!canSave}
          className="rounded-full bg-brand px-5 py-2 text-sm font-semibold text-white disabled:opacity-40"
        >
          Guardar
        </button>
      </div>
    </form>
  );
}

"use client";

import { useState } from "react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { useCart, type CartLine } from "@/lib/stores/cart";

/** F1.14 (F6): descuento manual en porcentaje para la línea seleccionada. */
export function DiscountDialog({
  line,
  maxPct,
  onClose,
}: {
  line: CartLine | null;
  /** Ajuste `pos.max_discount_pct` de la sucursal. */
  maxPct: number;
  onClose: () => void;
}) {
  return (
    <Dialog open={line !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="rounded-3xl p-6 sm:max-w-sm">
        {line && <DiscountForm key={line.key} line={line} maxPct={maxPct} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function DiscountForm({ line, maxPct, onClose }: { line: CartLine; maxPct: number; onClose: () => void }) {
  const setDiscount = useCart((s) => s.setDiscount);
  const [pct, setPct] = useState<number>(line.discountPct ?? 0);
  const valid = pct >= 0 && pct <= maxPct;

  return (
    <form
      className="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        setDiscount(line.key, pct > 0 ? pct : null);
        onClose();
      }}
    >
      <div>
        <DialogTitle className="text-xl font-bold">Descuento</DialogTitle>
        <p className="mt-1 text-sm text-slate-500">
          {line.name} · máximo {maxPct} %
        </p>
      </div>
      <label className="flex items-center gap-2 rounded-2xl border border-slate-200 px-4">
        <input
          type="number"
          min={0}
          max={maxPct}
          step="any"
          autoFocus
          value={pct}
          onChange={(e) => setPct(Number(e.target.value) || 0)}
          className="h-12 w-full bg-transparent text-lg font-semibold tabular-nums outline-none"
        />
        <span className="text-slate-400">%</span>
      </label>
      {!valid && <p className="text-xs text-rose-600">El descuento supera el máximo permitido.</p>}
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onClose}
          className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium hover:bg-slate-50"
        >
          Cancelar
        </button>
        <button
          type="submit"
          disabled={!valid}
          className="rounded-full bg-brand px-5 py-2 text-sm font-semibold text-white disabled:opacity-40"
        >
          Aplicar
        </button>
      </div>
    </form>
  );
}

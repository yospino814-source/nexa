"use client";

import { useState } from "react";

import { MoneyInput } from "@/components/caja/money-input";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import type { CustomTax } from "@/lib/api";
import { useCart } from "@/lib/stores/cart";

const TAX_OPTIONS: { value: CustomTax; label: string }[] = [
  { value: "IVA19", label: "IVA 19 %" },
  { value: "IVA5", label: "IVA 5 %" },
  { value: "INC8", label: "INC 8 %" },
  { value: "EXENTO", label: "Exento" },
];

/** F1.9: agrega una línea libre (sin producto del catálogo; no mueve stock). */
export function CustomItemDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="rounded-3xl p-6 sm:max-w-md">
        {open && <CustomItemForm onClose={() => onOpenChange(false)} />}
      </DialogContent>
    </Dialog>
  );
}

function CustomItemForm({ onClose }: { onClose: () => void }) {
  const addCustom = useCart((s) => s.addCustom);
  const [name, setName] = useState("");
  const [price, setPrice] = useState(0);
  const [tax, setTax] = useState<CustomTax>("IVA19");
  const [qty, setQty] = useState(1);
  const trimmed = name.trim();
  const canSave = trimmed.length >= 1 && trimmed.length <= 80 && price > 0 && qty > 0;

  return (
    <form
      className="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();
        if (!canSave) return;
        addCustom({ name: trimmed, price, tax, qty });
        onClose();
      }}
    >
      <DialogTitle className="text-xl font-bold">Ítem libre</DialogTitle>
      <label className="block space-y-1.5">
        <span className="text-sm font-medium">Nombre</span>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={80}
          autoFocus
          placeholder="Ej.: Arreglo de bota"
          className="h-11 w-full rounded-xl border border-slate-200 px-3 text-sm focus:border-brand focus:outline-none"
        />
      </label>
      <label className="block space-y-1.5">
        <span className="text-sm font-medium">Precio sin IVA</span>
        <MoneyInput value={price} onChange={setPrice} />
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="block space-y-1.5">
          <span className="text-sm font-medium">Impuesto</span>
          <select
            value={tax}
            onChange={(e) => setTax(e.target.value as CustomTax)}
            className="h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm"
          >
            {TAX_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <label className="block space-y-1.5">
          <span className="text-sm font-medium">Cantidad</span>
          <input
            type="number"
            min={1}
            value={qty}
            onChange={(e) => setQty(Math.max(0, Number(e.target.value) || 0))}
            className="h-11 w-full rounded-xl border border-slate-200 px-3 text-sm tabular-nums"
          />
        </label>
      </div>
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
          disabled={!canSave}
          className="rounded-full bg-brand px-5 py-2 text-sm font-semibold text-white disabled:opacity-40"
        >
          Agregar
        </button>
      </div>
    </form>
  );
}

"use client";

import { useState } from "react";
import { ArrowRight, Calculator, Loader2, PackagePlus, PlusCircle, SlidersHorizontal, Target } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type StockLevel } from "@/lib/api";
import { formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

export type StockMode = "CONTEO" | "DELTA" | "ENTRADA" | "MINIMO";

const MODES: { id: StockMode; label: string; icon: typeof Calculator }[] = [
  { id: "CONTEO", label: "Conteo físico", icon: Calculator },
  { id: "DELTA", label: "Sumar / restar", icon: PlusCircle },
  { id: "ENTRADA", label: "Entrada de mercancía", icon: PackagePlus },
  { id: "MINIMO", label: "Punto de reorden", icon: Target },
];

const REASONS: Record<Exclude<StockMode, "MINIMO">, string[]> = {
  CONTEO: ["Conteo de inventario", "Auditoría", "Cierre de mes"],
  DELTA: ["Merma / vencido", "Rotura / avería", "Consumo interno", "Robo / faltante", "Devolución de cliente"],
  ENTRADA: ["Compra a proveedor", "Producción propia", "Devolución a bodega"],
};

/** Ajuste de existencias de un producto, con motivo para el kardex. */
export function StockDialog({
  item,
  initialMode,
  branchId,
  onClose,
  onSaved,
}: {
  item: StockLevel | null;
  initialMode: StockMode;
  branchId: string;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [mode, setMode] = useState<StockMode>(initialMode);
  const [value, setValue] = useState("");
  const [reason, setReason] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reinicio al abrir con otro producto o modo.
  const openKey = item ? `${item.product_id}:${initialMode}` : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    setMode(initialMode);
    setValue(initialMode === "MINIMO" && item ? formatQty(item.min_qty).replace(/\./g, "") : "");
    setReason("");
    setNotes("");
    setError(null);
  }

  if (!item) return <Dialog open={false} />;

  const current = Number(item.on_hand);
  const n = Number(value.replace(",", "."));
  const valid = value.trim() !== "" && Number.isFinite(n);
  const next =
    mode === "CONTEO" ? n : mode === "DELTA" ? current + n : mode === "ENTRADA" ? current + n : current;
  const delta = next - current;
  const cause = [reason, notes.trim()].filter(Boolean).join(" — ");
  const canSave =
    valid &&
    !busy &&
    (mode === "MINIMO"
      ? n >= 0
      : !!reason &&
        (mode === "CONTEO" ? n >= 0 && delta !== 0 : mode === "DELTA" ? n !== 0 && next >= Number(item.reserved) : n > 0));

  async function save() {
    if (!item) return;
    setBusy(true);
    setError(null);
    try {
      if (mode === "CONTEO") await api.inventory.count(branchId, item.product_id, String(n), cause);
      if (mode === "DELTA") await api.inventory.adjust(branchId, item.product_id, String(n), cause);
      if (mode === "ENTRADA") await api.inventory.replenish(branchId, item.product_id, String(n), cause);
      if (mode === "MINIMO") await api.inventory.setMinQty(branchId, item.product_id, String(n));
      onSaved(
        mode === "MINIMO"
          ? `Punto de reorden de ${item.name}: ${formatQty(n)}`
          : `${item.name}: ${formatQty(current)} → ${formatQty(next)}`,
      );
      onClose();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[95vh] gap-0 overflow-y-auto rounded-3xl p-6 sm:max-w-lg">
        <div className="flex items-start gap-3">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-brand/10 text-brand">
            <SlidersHorizontal className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-lg font-bold">Ajustar inventario</DialogTitle>
            <DialogDescription className="text-sm text-slate-500">Todo cambio queda en el kardex con su motivo.</DialogDescription>
          </div>
        </div>

        <div className="mt-5 flex items-center justify-between rounded-2xl border border-slate-200 bg-slate-50 p-4">
          <div>
            <p className="font-semibold">{item.name}</p>
            <p className="font-mono text-xs text-slate-400">{item.sku ?? "Sin SKU"}</p>
          </div>
          <p className="rounded-full border border-slate-200 bg-white px-3 py-1 text-sm">
            En sistema: <span className="font-bold text-brand">{formatQty(item.on_hand)}</span>
          </p>
        </div>

        <div className="mt-5 grid grid-cols-2 gap-2">
          {MODES.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => {
                setMode(id);
                setValue(id === "MINIMO" ? formatQty(item.min_qty).replace(/\./g, "") : "");
                setReason("");
              }}
              className={cn(
                "flex items-center justify-center gap-2 rounded-2xl border px-3 py-2.5 text-sm font-medium",
                mode === id ? "border-brand bg-brand/10 text-brand" : "border-slate-200 text-slate-600 hover:bg-slate-50",
              )}
            >
              <Icon className="size-4" /> {label}
            </button>
          ))}
        </div>

        <label htmlFor="qty" className="mt-5 text-sm font-medium text-slate-700">
          {mode === "CONTEO" && "Unidades contadas en estante"}
          {mode === "DELTA" && "Unidades a sumar (use − para restar)"}
          {mode === "ENTRADA" && "Unidades que entran"}
          {mode === "MINIMO" && "Avisar cuando queden (unidades)"}
        </label>
        <input
          id="qty"
          autoFocus
          inputMode="decimal"
          value={value}
          onChange={(e) => setValue(e.target.value.replace(/[^\d,.-]/g, ""))}
          placeholder={mode === "DELTA" ? "-3" : "0"}
          className="mt-2 h-14 rounded-2xl border border-slate-200 text-center text-2xl font-bold tabular-nums outline-none focus:ring-2 focus:ring-brand/40"
        />

        {mode !== "MINIMO" && valid && (
          <div className="mt-4 rounded-2xl border border-teal-100 bg-teal-50/50 p-4 text-sm">
            <div className="flex items-center justify-between">
              <span>
                Actual: <b>{formatQty(current)}</b>
              </span>
              <ArrowRight className="size-4 text-brand" />
              <span>
                Nuevo: <b>{formatQty(next)}</b>
              </span>
            </div>
            <p
              className={cn(
                "mt-2 border-t border-teal-100 pt-2 font-medium",
                delta < 0 ? "text-rose-600" : delta > 0 ? "text-emerald-700" : "text-slate-500",
              )}
            >
              {delta === 0
                ? "Sin diferencia: no se registra movimiento."
                : `Impacto en kardex: ${delta > 0 ? "+" : ""}${formatQty(delta)} (${mode === "ENTRADA" ? "reposición" : delta > 0 ? "ajuste de entrada" : "ajuste de salida"})`}
            </p>
          </div>
        )}

        {mode !== "MINIMO" && (
          <>
            <p className="mt-5 text-sm font-medium text-slate-700">
              Motivo <span className="text-rose-500">*</span>
            </p>
            <div className="mt-2 flex flex-wrap gap-2">
              {REASONS[mode].map((r) => (
                <button
                  key={r}
                  onClick={() => setReason(r)}
                  className={cn(
                    "rounded-full border px-3 py-1 text-xs",
                    reason === r ? "border-brand bg-brand/10 font-medium text-brand" : "border-slate-200 text-slate-600 hover:bg-slate-50",
                  )}
                >
                  {r}
                </button>
              ))}
            </div>
            <textarea
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Notas (opcional): factura, proveedor, qué pasó…"
              className="mt-3 rounded-2xl border border-slate-200 p-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
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
            disabled={!canSave}
            className="flex h-11 items-center gap-2 rounded-xl bg-brand px-5 text-sm font-semibold text-white hover:bg-brand-dark disabled:opacity-50"
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            {mode === "MINIMO" ? "Guardar punto de reorden" : "Confirmar y registrar"}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

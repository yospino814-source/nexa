"use client";

import { useState } from "react";
import { Loader2, MinusCircle, PlusCircle } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type CashMovementKind } from "@/lib/api";
import { bumpCash } from "@/lib/stores/cash";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

import { MoneyInput } from "./money-input";

const REASONS: Record<CashMovementKind, string[]> = {
  INGRESO: ["Sencillo adicional", "Aporte del dueño", "Devolución de proveedor"],
  RETIRO: ["Compra de insumos", "Pago a proveedor", "Envío a banco / caja fuerte", "Gasto menor"],
};

/** Registrar ingreso o retiro de efectivo del turno (no son ventas). */
export function CashMovementDialog({
  kind,
  sessionId,
  available,
  onClose,
  onSaved,
}: {
  kind: CashMovementKind | null;
  sessionId: string;
  /** Efectivo esperado en gaveta: tope de un retiro. */
  available: number;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [amount, setAmount] = useState(0);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Se reinicia cada vez que se abre con otro tipo.
  const [lastKind, setLastKind] = useState(kind);
  if (kind !== lastKind) {
    setLastKind(kind);
    setAmount(0);
    setReason("");
    setError(null);
  }

  const isOut = kind === "RETIRO";
  const tooMuch = isOut && amount > available;

  async function save() {
    if (!kind) return;
    setBusy(true);
    setError(null);
    try {
      await api.cash.addMovement(sessionId, kind, amount.toFixed(2), reason);
      bumpCash();
      onSaved();
      onClose();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={kind !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="gap-0 rounded-3xl p-6 sm:max-w-md">
        <div className="flex items-start gap-3">
          <span
            className={cn(
              "flex size-11 shrink-0 items-center justify-center rounded-2xl",
              isOut ? "bg-rose-50 text-rose-600" : "bg-emerald-50 text-emerald-600",
            )}
          >
            {isOut ? <MinusCircle className="size-5" /> : <PlusCircle className="size-5" />}
          </span>
          <div>
            <DialogTitle className="text-lg font-bold">{isOut ? "Registrar retiro" : "Registrar ingreso"}</DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              {isOut
                ? `Sale efectivo de la gaveta. Disponible: ${formatCOP(available)}.`
                : "Entra efectivo a la gaveta sin ser una venta."}
            </DialogDescription>
          </div>
        </div>

        <label className="mt-6 text-sm font-medium text-slate-700">Monto</label>
        <MoneyInput value={amount} onChange={setAmount} autoFocus className="mt-2" />
        {tooMuch && <p className="mt-2 text-sm text-rose-600">El retiro supera el efectivo esperado en gaveta.</p>}

        <label htmlFor="reason" className="mt-5 text-sm font-medium text-slate-700">
          Motivo <span className="text-rose-500">*</span>
        </label>
        <div className="mt-2 flex flex-wrap gap-2">
          {kind &&
            REASONS[kind].map((r) => (
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
        <input
          id="reason"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="Ej: compra de hielo para el mostrador"
          className="mt-2 h-11 rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
        />

        {error && <p className="mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

        <div className="mt-6 flex justify-end gap-3">
          <button onClick={onClose} className="h-11 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50">
            Cancelar
          </button>
          <button
            onClick={save}
            disabled={busy || amount <= 0 || !reason.trim() || tooMuch}
            className={cn(
              "flex h-11 items-center gap-2 rounded-xl px-5 text-sm font-semibold text-white disabled:opacity-50",
              isOut ? "bg-rose-600 hover:bg-rose-700" : "bg-emerald-600 hover:bg-emerald-700",
            )}
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            {isOut ? "Registrar retiro" : "Registrar ingreso"}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

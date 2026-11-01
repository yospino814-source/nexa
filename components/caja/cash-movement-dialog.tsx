"use client";

import { useState } from "react";
import { ArrowDownRight, ArrowUpRight, Coins, Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type CashMovementKind, type CashMovementReasonCode } from "@/lib/api";
import { bumpCash } from "@/lib/stores/cash";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

import { MoneyInput } from "./money-input";

type DialogKind = "INGRESO" | "RETIRO" | "PROPINA_PAGADA";

const QUICK_INFLOW_REASONS = [
  "Sencillo adicional",
  "Aporte del dueño",
  "Devolución de proveedor",
];

const WITHDRAWAL_REASONS: { code: CashMovementReasonCode; label: string }[] = [
  { code: "CONSIGNACION", label: "Consignación" },
  { code: "GASTO_MENOR", label: "Gasto menor" },
  { code: "PRESTAMO", label: "Préstamo" },
  { code: "OTRO", label: "Otro motivo" },
];

/** Registrar ingreso, retiro o pago de propina del turno. */
export function CashMovementDialog({
  kind,
  sessionId,
  available,
  tipsPending = 0,
  onClose,
  onSaved,
}: {
  kind: CashMovementKind | null;
  sessionId: string;
  /** Efectivo disponible en gaveta. */
  available: number;
  /** Propinas pendientes por pagar a meseros. */
  tipsPending?: number;
  onClose: () => void;
  onSaved: () => void;
}) {
  return (
    <Dialog open={kind !== null} onOpenChange={(open) => !open && onClose()}>
      {kind !== null && (
        <CashMovementDialogInner
          key={kind}
          initialKind={kind}
          sessionId={sessionId}
          available={available}
          tipsPending={tipsPending}
          onClose={onClose}
          onSaved={onSaved}
        />
      )}
    </Dialog>
  );
}

function CashMovementDialogInner({
  initialKind,
  sessionId,
  available,
  tipsPending,
  onClose,
  onSaved,
}: {
  initialKind: CashMovementKind;
  sessionId: string;
  available: number;
  tipsPending: number;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [tab, setTab] = useState<DialogKind>(() => {
    if (initialKind === "RETIRO" || initialKind === "PROPINA_PAGADA") return initialKind;
    return "INGRESO";
  });
  const [amount, setAmount] = useState(0);
  const [reason, setReason] = useState("");
  const [reasonCode, setReasonCode] = useState<CashMovementReasonCode>("CONSIGNACION");
  const [waiterName, setWaiterName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isOut = tab === "RETIRO" || tab === "PROPINA_PAGADA";
  const tooMuch = isOut && amount > available;

  const isValid = (() => {
    if (amount <= 0 || tooMuch) return false;
    if (tab === "INGRESO") {
      const len = reason.trim().length;
      return len >= 3 && len <= 200;
    }
    if (tab === "RETIRO") {
      if (!reasonCode) return false;
      if (reasonCode === "OTRO") {
        const len = reason.trim().length;
        return len >= 3 && len <= 200;
      }
      return true;
    }
    if (tab === "PROPINA_PAGADA") {
      return waiterName.trim().length >= 2;
    }
    return false;
  })();

  async function save() {
    if (!isValid) return;
    setBusy(true);
    setError(null);
    try {
      if (tab === "INGRESO") {
        await api.cash.addMovement(sessionId, "INGRESO", amount.toFixed(2), reason.trim(), null);
      } else if (tab === "RETIRO") {
        const detail = reasonCode === "OTRO" ? reason.trim() : WITHDRAWAL_REASONS.find((r) => r.code === reasonCode)?.label;
        await api.cash.addMovement(sessionId, "RETIRO", amount.toFixed(2), detail, reasonCode);
      } else if (tab === "PROPINA_PAGADA") {
        await api.cash.addMovement(sessionId, "PROPINA_PAGADA", amount.toFixed(2), waiterName.trim(), null);
      }
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
    <DialogContent className="gap-0 rounded-3xl p-6 sm:max-w-md">
      {/* Pestañas superiores */}
      <div className="flex rounded-2xl bg-slate-100 p-1">
        <button
          type="button"
          onClick={() => {
            setTab("INGRESO");
            setError(null);
          }}
          className={cn(
            "flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2 text-xs font-semibold transition-colors",
            tab === "INGRESO"
              ? "bg-white text-emerald-700 shadow-xs"
              : "text-slate-600 hover:text-slate-900",
          )}
        >
          <ArrowUpRight className="size-3.5" />
          Ingreso
        </button>
        <button
          type="button"
          onClick={() => {
            setTab("RETIRO");
            setError(null);
          }}
          className={cn(
            "flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2 text-xs font-semibold transition-colors",
            tab === "RETIRO"
              ? "bg-white text-rose-700 shadow-xs"
              : "text-slate-600 hover:text-slate-900",
          )}
        >
          <ArrowDownRight className="size-3.5" />
          Retiro
        </button>
        <button
          type="button"
          onClick={() => {
            setTab("PROPINA_PAGADA");
            setError(null);
          }}
          className={cn(
            "flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2 text-xs font-semibold transition-colors",
            tab === "PROPINA_PAGADA"
              ? "bg-white text-amber-700 shadow-xs"
              : "text-slate-600 hover:text-slate-900",
          )}
        >
          <Coins className="size-3.5" />
          Pagar propina
        </button>
      </div>

      {/* Título y descripción */}
      <div className="mt-5">
        <DialogTitle className="text-lg font-bold">
          {tab === "INGRESO" && "Registrar ingreso"}
          {tab === "RETIRO" && "Registrar retiro"}
          {tab === "PROPINA_PAGADA" && "Pagar propina a mesero"}
        </DialogTitle>
        <DialogDescription className="mt-1 text-sm text-slate-500">
          {tab === "INGRESO" && "Entra efectivo a la gaveta sin provenir de una venta."}
          {tab === "RETIRO" && `Sale efectivo de la gaveta. Disponible: ${formatCOP(available)}.`}
          {tab === "PROPINA_PAGADA" &&
            `Liquidación de propina recolectada. Disponible en caja: ${formatCOP(available)}.`}
        </DialogDescription>
      </div>

      {/* Sugerencia de propinas pendientes */}
      {tab === "PROPINA_PAGADA" && tipsPending > 0 && (
        <div className="mt-4 flex items-center justify-between rounded-2xl border border-amber-200 bg-amber-50 p-3 text-xs text-amber-800">
          <span>
            Propinas pendientes por pagar: <strong>{formatCOP(tipsPending)}</strong>
          </span>
          <button
            type="button"
            onClick={() => setAmount(tipsPending)}
            className="rounded-lg bg-amber-600 px-2.5 py-1 font-semibold text-white hover:bg-amber-700"
          >
            Usar monto
          </button>
        </div>
      )}

      {/* Campo Monto */}
      <label className="mt-5 text-sm font-medium text-slate-700">Monto</label>
      <MoneyInput value={amount} onChange={setAmount} autoFocus className="mt-2" />
      {tooMuch && (
        <p className="mt-2 text-xs text-rose-600 font-medium">
          El monto supera el efectivo esperado en gaveta ({formatCOP(available)}).
        </p>
      )}

      {/* Formulario específico según Tab */}
      {tab === "INGRESO" && (
        <>
          <label htmlFor="reason" className="mt-5 text-sm font-medium text-slate-700">
            Motivo <span className="text-rose-500">*</span>
          </label>
          <div className="mt-2 flex flex-wrap gap-2">
            {QUICK_INFLOW_REASONS.map((r) => (
              <button
                key={r}
                type="button"
                onClick={() => setReason(r)}
                className={cn(
                  "rounded-full border px-3 py-1 text-xs transition-colors",
                  reason === r
                    ? "border-emerald-600 bg-emerald-50 font-medium text-emerald-700"
                    : "border-slate-200 text-slate-600 hover:bg-slate-50",
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
            placeholder="Ej: Aporte inicial de sencillo para cambio"
            maxLength={200}
            className="mt-2 h-11 rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
          />
          {reason.trim().length > 0 && reason.trim().length < 3 && (
            <p className="mt-1 text-xs text-rose-500">El motivo debe tener al menos 3 caracteres.</p>
          )}
        </>
      )}

      {tab === "RETIRO" && (
        <>
          <label className="mt-5 text-sm font-medium text-slate-700">
            Tipo de retiro <span className="text-rose-500">*</span>
          </label>
          <div className="mt-2 grid grid-cols-2 gap-2">
            {WITHDRAWAL_REASONS.map((r) => (
              <button
                key={r.code}
                type="button"
                onClick={() => setReasonCode(r.code)}
                className={cn(
                  "rounded-xl border p-2.5 text-left text-xs font-medium transition-colors",
                  reasonCode === r.code
                    ? "border-rose-600 bg-rose-50 text-rose-800"
                    : "border-slate-200 text-slate-600 hover:bg-slate-50",
                )}
              >
                {r.label}
              </button>
            ))}
          </div>

          {reasonCode === "OTRO" && (
            <>
              <label htmlFor="withdrawal-reason" className="mt-4 text-sm font-medium text-slate-700">
                Especifique el motivo <span className="text-rose-500">*</span>
              </label>
              <input
                id="withdrawal-reason"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                placeholder="Ej: Pago de flete a transportador"
                maxLength={200}
                className="mt-2 h-11 rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
              />
              {reason.trim().length < 3 && (
                <p className="mt-1 text-xs text-rose-500">Mínimo 3 caracteres para otro motivo.</p>
              )}
            </>
          )}
        </>
      )}

      {tab === "PROPINA_PAGADA" && (
        <>
          <label htmlFor="waiter-name" className="mt-5 text-sm font-medium text-slate-700">
            Nombre del mesero <span className="text-rose-500">*</span>
          </label>
          <input
            id="waiter-name"
            value={waiterName}
            onChange={(e) => setWaiterName(e.target.value)}
            placeholder="Ej: Juan Pérez"
            maxLength={100}
            className="mt-2 h-11 rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
          />
        </>
      )}

      {error && <p className="mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

      <div className="mt-6 flex justify-end gap-3">
        <button
          type="button"
          onClick={onClose}
          className="h-11 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50"
        >
          Cancelar
        </button>
        <button
          type="button"
          onClick={save}
          disabled={busy || !isValid}
          className={cn(
            "flex h-11 items-center gap-2 rounded-xl px-5 text-sm font-semibold text-white transition-opacity disabled:opacity-50",
            tab === "INGRESO" && "bg-emerald-600 hover:bg-emerald-700",
            tab === "RETIRO" && "bg-rose-600 hover:bg-rose-700",
            tab === "PROPINA_PAGADA" && "bg-amber-600 hover:bg-amber-700",
          )}
        >
          {busy && <Loader2 className="size-4 animate-spin" />}
          {tab === "INGRESO" && "Registrar ingreso"}
          {tab === "RETIRO" && "Registrar retiro"}
          {tab === "PROPINA_PAGADA" && "Pagar propina"}
        </button>
      </div>
    </DialogContent>
  );
}

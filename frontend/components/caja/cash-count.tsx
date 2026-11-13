"use client";

import { useState } from "react";
import { AlertTriangle, CheckCircle2, Eye, EyeOff, Loader2, Lock } from "lucide-react";

import { api, ApiError, friendlyError, type CashSummary } from "@/lib/api";
import { bumpCash } from "@/lib/stores/cash";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

const BILLS = [100_000, 50_000, 20_000, 10_000, 5_000, 2_000];
const COINS = [1_000, 500, 200, 100, 50];

/**
 * Arqueo: el cajero cuenta billetes y monedas. El efectivo esperado queda
 * oculto por defecto (conteo ciego) para que el conteo no se "acomode".
 */
export function CashCount({
  summary,
  onClosed,
  onReload,
}: {
  summary: CashSummary;
  onClosed: (closed: CashSummary) => void;
  onReload?: () => void;
}) {
  const [qty, setQty] = useState<Record<number, number>>({});
  const [showExpected, setShowExpected] = useState(false);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Guarda el token recibido al abrirse
  const [token, setToken] = useState(summary.state_token);

  const counted = [...BILLS, ...COINS].reduce((sum, d) => sum + d * (qty[d] ?? 0), 0);
  const expected = Number(summary.expected_cash);
  const difference = counted - expected;
  const touched = Object.values(qty).some((n) => n > 0);
  const needsNote = touched && difference !== 0;

  async function close() {
    setBusy(true);
    setError(null);
    try {
      const closed = await api.cash.close(
        summary.session.id,
        counted.toFixed(2),
        token,
        notes.trim() || undefined,
      );
      bumpCash();
      onClosed(closed);
    } catch (e) {
      if (e instanceof ApiError && e.code === "CASH_STATE_CHANGED") {
        setError("Hubo movimientos nuevos mientras contabas. Volvé a contar.");
        setQty({});
        setToken(summary.state_token);
        bumpCash();
        onReload?.();
      } else {
        setError(friendlyError(e));
      }
    } finally {
      setBusy(false);
    }
  }

  const row = (d: number) => (
    <div key={d} className="flex items-center gap-3 rounded-2xl border border-slate-100 bg-slate-50/60 px-3 py-2">
      <span className="w-24 text-sm font-medium tabular-nums">{formatCOP(d)}</span>
      <div className="flex items-center gap-1">
        <button
          aria-label={`Quitar ${formatCOP(d)}`}
          onClick={() => setQty((q) => ({ ...q, [d]: Math.max(0, (q[d] ?? 0) - 1) }))}
          className="size-8 rounded-full border border-slate-200 bg-white text-slate-600 hover:bg-slate-100"
        >
          −
        </button>
        <input
          inputMode="numeric"
          aria-label={`Cantidad de ${formatCOP(d)}`}
          value={qty[d] ?? ""}
          placeholder="0"
          onChange={(e) => setQty((q) => ({ ...q, [d]: Number(e.target.value.replace(/\D/g, "")) || 0 }))}
          className="h-8 w-14 rounded-lg border border-slate-200 bg-white text-center text-sm tabular-nums outline-none focus:ring-2 focus:ring-brand/40"
        />
        <button
          aria-label={`Agregar ${formatCOP(d)}`}
          onClick={() => setQty((q) => ({ ...q, [d]: (q[d] ?? 0) + 1 }))}
          className="size-8 rounded-full border border-slate-200 bg-white text-slate-600 hover:bg-slate-100"
        >
          +
        </button>
      </div>
      <span className="ml-auto text-sm font-semibold tabular-nums">{formatCOP(d * (qty[d] ?? 0))}</span>
    </div>
  );

  return (
    <section className="rounded-3xl border border-slate-200/60 bg-white p-6 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-bold">Arqueo y cierre</h2>
          <p className="text-sm text-slate-500">Contá lo que hay físicamente en la gaveta.</p>
        </div>
        <button
          onClick={() => setShowExpected((v) => !v)}
          className="flex shrink-0 items-center gap-1.5 rounded-full border border-slate-200 px-3 py-1.5 text-xs font-medium whitespace-nowrap text-slate-600 hover:bg-slate-50"
        >
          {showExpected ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
          {showExpected ? "Ocultar esperado" : "Ver esperado"}
        </button>
      </div>

      <p className="mt-5 text-xs font-semibold tracking-wider text-slate-400 uppercase">Billetes</p>
      <div className="mt-2 space-y-2">{BILLS.map(row)}</div>
      <p className="mt-5 text-xs font-semibold tracking-wider text-slate-400 uppercase">Monedas</p>
      <div className="mt-2 space-y-2">{COINS.map(row)}</div>

      <div className="mt-6 space-y-2 border-t border-slate-100 pt-4 text-sm">
        <div className="flex justify-between text-slate-500">
          <span>Efectivo esperado</span>
          <span className="tabular-nums">{showExpected ? formatCOP(expected) : "••••••"}</span>
        </div>
        <div className="flex items-baseline justify-between">
          <span className="font-semibold">Total contado</span>
          <span className="text-2xl font-extrabold tabular-nums">{formatCOP(counted)}</span>
        </div>
      </div>

      {touched && (
        <div
          className={cn(
            "mt-4 flex items-center gap-2 rounded-2xl border p-3 text-sm font-medium",
            difference === 0 && "border-emerald-200 bg-emerald-50 text-emerald-700",
            difference < 0 && "border-amber-200 bg-amber-50 text-amber-800",
            difference > 0 && "border-sky-200 bg-sky-50 text-sky-800",
          )}
        >
          {difference === 0 ? <CheckCircle2 className="size-4" /> : <AlertTriangle className="size-4" />}
          {difference === 0
            ? "Caja cuadrada"
            : `${difference < 0 ? "Faltante" : "Sobrante"}: ${formatCOP(Math.abs(difference))}`}
        </div>
      )}

      <label htmlFor="notes" className="mt-4 block text-sm font-medium text-slate-700">
        Nota del cierre {needsNote && <span className="text-rose-500">(obligatoria si hay descuadre)</span>}
      </label>
      <textarea
        id="notes"
        rows={2}
        value={notes}
        onChange={(e) => setNotes(e.target.value)}
        placeholder="Ej: faltante por cambio redondeado en hora pico"
        className="mt-2 w-full rounded-2xl border border-slate-200 p-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
      />

      {error && <p className="mt-3 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

      <button
        onClick={close}
        disabled={busy || (!touched && expected !== 0) || (needsNote && !notes.trim())}
        className="mt-5 flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-brand text-base font-bold text-white hover:bg-brand-dark disabled:opacity-50"
      >
        {busy ? <Loader2 className="size-5 animate-spin" /> : <Lock className="size-5" />}
        Confirmar cierre de caja
      </button>
    </section>
  );
}

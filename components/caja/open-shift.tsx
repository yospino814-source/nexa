"use client";

import { useState } from "react";
import { Loader2, LockOpen, Wallet } from "lucide-react";

import { api, friendlyError } from "@/lib/api";
import { bumpCash } from "@/lib/stores/cash";
import { formatCOP } from "@/lib/format";

import { MoneyInput } from "./money-input";

const QUICK_BASES = [50_000, 100_000, 150_000, 200_000];

/** Apertura de turno: el cajero declara la base (sencillo) con la que arranca. */
export function OpenShift({ branchId, branchName, onOpened }: { branchId: string; branchName: string; onOpened: () => void }) {
  const [base, setBase] = useState(100_000);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function open() {
    setBusy(true);
    setError(null);
    try {
      await api.cash.open(branchId, base.toFixed(2));
      bumpCash();
      onOpened();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto mt-10 max-w-lg rounded-3xl border border-slate-200/60 bg-white p-8 shadow-sm">
      <span className="flex size-14 items-center justify-center rounded-2xl bg-amber-50 text-amber-600">
        <Wallet className="size-7" />
      </span>
      <h2 className="mt-5 text-2xl font-bold tracking-tight">Abrir caja</h2>
      <p className="mt-1 text-sm text-slate-500">
        {branchName} · Contá el sencillo con el que arrancás. Esa base se suma al efectivo esperado del arqueo.
      </p>

      <label htmlFor="base" className="mt-6 block text-sm font-medium text-slate-700">
        Base inicial en gaveta
      </label>
      <MoneyInput id="base" value={base} onChange={setBase} autoFocus className="mt-2" />
      <div className="mt-3 flex flex-wrap gap-2">
        {QUICK_BASES.map((amount) => (
          <button
            key={amount}
            onClick={() => setBase(amount)}
            className={
              amount === base
                ? "rounded-full border border-brand bg-brand/10 px-3 py-1 text-sm font-medium text-brand"
                : "rounded-full border border-slate-200 px-3 py-1 text-sm text-slate-600 hover:bg-slate-50"
            }
          >
            {formatCOP(amount)}
          </button>
        ))}
      </div>

      {error && <p className="mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

      <button
        onClick={open}
        disabled={busy}
        className="mt-6 flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-brand text-lg font-bold text-white hover:bg-brand-dark disabled:opacity-60"
      >
        {busy ? <Loader2 className="size-5 animate-spin" /> : <LockOpen className="size-5" />}
        Abrir turno con {formatCOP(base)}
      </button>
    </div>
  );
}

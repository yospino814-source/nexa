"use client";

import { useEffect, useState } from "react";
import { Loader2, LockOpen, Wallet } from "lucide-react";

import { api, friendlyError, type CashRegister } from "@/lib/api";
import { bumpCash } from "@/lib/stores/cash";
import { formatCOP } from "@/lib/format";

import { MoneyInput } from "./money-input";

const QUICK_BASES = [50_000, 100_000, 150_000, 200_000];

/** Apertura de turno: el cajero elige la caja y declara la base inicial. */
export function OpenShift({
  branchId,
  branchName,
  onOpened,
}: {
  branchId: string;
  branchName: string;
  onOpened: () => void;
}) {
  const [registers, setRegisters] = useState<CashRegister[]>([]);
  const [selectedRegisterId, setSelectedRegisterId] = useState<string | null>(null);
  const [base, setBase] = useState(100_000);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;
    async function load() {
      setLoading(true);
      try {
        const list = await api.cashRegisters.list(branchId);
        if (!ignore) {
          const active = list.filter((r) => r.active);
          setRegisters(active);
          // Pre-seleccionar la primera caja disponible o la primera activa
          const available = active.find((r) => !r.open_session) ?? active[0];
          if (available) {
            setSelectedRegisterId(available.id);
            setBase(Number(available.default_opening));
          }
        }
      } catch (e) {
        if (!ignore) setError(friendlyError(e));
      } finally {
        if (!ignore) setLoading(false);
      }
    }
    load();
    return () => {
      ignore = true;
    };
  }, [branchId]);

  function handleSelectRegister(regId: string) {
    setSelectedRegisterId(regId);
    const reg = registers.find((r) => r.id === regId);
    if (reg) {
      setBase(Number(reg.default_opening));
    }
  }

  async function open() {
    if (!selectedRegisterId) {
      setError("Seleccioná una caja registradora");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.cash.open(branchId, selectedRegisterId, base.toFixed(2));
      bumpCash();
      onOpened();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  const selectedReg = registers.find((r) => r.id === selectedRegisterId);

  return (
    <div className="mx-auto mt-10 max-w-lg rounded-3xl border border-slate-200/60 bg-white p-8 shadow-sm">
      <span className="flex size-14 items-center justify-center rounded-2xl bg-amber-50 text-amber-600">
        <Wallet className="size-7" />
      </span>
      <h2 className="mt-5 text-2xl font-bold tracking-tight">Abrir caja</h2>
      <p className="mt-1 text-sm text-slate-500">
        {branchName} · Contá el sencillo con el que arrancás. Esa base se suma al efectivo esperado del arqueo.
      </p>

      {loading ? (
        <div className="my-8 flex items-center justify-center gap-2 text-sm text-slate-400">
          <Loader2 className="size-5 animate-spin" />
          <span>Cargando cajas...</span>
        </div>
      ) : registers.length === 1 ? (
        <div className="mt-6 rounded-2xl border border-slate-100 bg-slate-50/70 p-4">
          <span className="text-xs font-semibold uppercase tracking-wider text-slate-400">Caja asignada</span>
          <p className="mt-0.5 text-base font-bold text-slate-900">
            {registers[0].name} <span className="font-mono text-sm text-slate-500">({registers[0].code})</span>
          </p>
        </div>
      ) : registers.length > 1 ? (
        <div className="mt-6">
          <label htmlFor="register" className="block text-sm font-medium text-slate-700">
            Caja registradora
          </label>
          <select
            id="register"
            value={selectedRegisterId ?? ""}
            onChange={(e) => handleSelectRegister(e.target.value)}
            className="mt-2 block w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm font-semibold text-slate-900 shadow-xs focus:border-brand focus:ring-1 focus:ring-brand"
          >
            {registers.map((reg) => (
              <option key={reg.id} value={reg.id} disabled={!!reg.open_session}>
                {reg.name} ({reg.code})
                {reg.open_session ? ` (en uso por ${reg.open_session.opened_by_name || "otro usuario"})` : ""}
              </option>
            ))}
          </select>
        </div>
      ) : (
        <p className="mt-4 rounded-xl bg-amber-50 p-3 text-sm text-amber-700">
          No hay cajas activas en esta sucursal. Contactá a un administrador.
        </p>
      )}

      <label htmlFor="base" className="mt-6 block text-sm font-medium text-slate-700">
        Base inicial en gaveta {selectedReg && <span className="text-xs font-normal text-slate-400">(Sugerida: {formatCOP(Number(selectedReg.default_opening))})</span>}
      </label>
      <MoneyInput id="base" value={base} onChange={setBase} autoFocus className="mt-2" />
      <div className="mt-3 flex flex-wrap gap-2">
        {QUICK_BASES.map((amount) => (
          <button
            key={amount}
            type="button"
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
        type="button"
        onClick={open}
        disabled={busy || loading || registers.length === 0 || !!selectedReg?.open_session}
        className="mt-6 flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-brand text-lg font-bold text-white hover:bg-brand-dark disabled:opacity-60"
      >
        {busy ? <Loader2 className="size-5 animate-spin" /> : <LockOpen className="size-5" />}
        Abrir turno con {formatCOP(base)}
      </button>
    </div>
  );
}

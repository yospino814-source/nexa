"use client";

import { useState } from "react";
import Link from "next/link";
import {
  Calculator,
  CheckCircle2,
  Edit2,
  Loader2,
  Plus,
  Store,
  X,
  XCircle,
} from "lucide-react";

import { api, friendlyError, type CashRegister } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { MoneyInput } from "@/components/caja/money-input";

export function RegistersScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const can = useSession((s) => s.can);

  const [selectedBranchId, setSelectedBranchId] = useState<string>(currentBranch.id);
  const [toast, setToast] = useState<string | null>(null);

  // useApi hook for loading registers
  const {
    data: registers,
    loading,
    reload,
  } = useApi(() => api.cashRegisters.list(selectedBranchId), selectedBranchId);

  const { data: locations } = useApi(
    () => api.locations.list(selectedBranchId),
    selectedBranchId
  );

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingRegister, setEditingRegister] = useState<CashRegister | null>(null);
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [locationId, setLocationId] = useState<string>("");
  const [defaultOpening, setDefaultOpening] = useState(100_000);
  const [active, setActive] = useState(true);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  function openCreate() {
    setEditingRegister(null);
    setName("");
    setCode("");
    setLocationId("");
    setDefaultOpening(100_000);
    setActive(true);
    setFormError(null);
    setDialogOpen(true);
  }

  function openEdit(reg: CashRegister) {
    setEditingRegister(reg);
    setName(reg.name);
    setCode(reg.code);
    setLocationId(reg.location_id ?? "");
    setDefaultOpening(Number(reg.default_opening));
    setActive(reg.active);
    setFormError(null);
    setDialogOpen(true);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const cleanName = name.trim();
    const cleanCode = code.trim().toUpperCase();

    if (!cleanName || cleanName.length > 60) {
      setFormError("El nombre debe tener entre 1 y 60 caracteres");
      return;
    }
    if (!/^[A-Z0-9-]{1,10}$/.test(cleanCode)) {
      setFormError("El código debe tener entre 1 y 10 caracteres alfanuméricos en mayúsculas o guiones (ej. CAJA-1)");
      return;
    }
    if (defaultOpening < 0) {
      setFormError("La base sugerida no puede ser negativa");
      return;
    }

    setSaving(true);
    setFormError(null);
    try {
      if (editingRegister) {
        await api.cashRegisters.update(editingRegister.id, {
          name: cleanName,
          code: cleanCode,
          default_opening: defaultOpening,
          active,
          location_id: locationId ? locationId : null,
        });
        setToast("Caja registradora actualizada");
      } else {
        await api.cashRegisters.create({
          branch_id: selectedBranchId,
          name: cleanName,
          code: cleanCode,
          default_opening: defaultOpening,
          location_id: locationId ? locationId : null,
        });
        setToast("Caja registradora creada");
      }
      setDialogOpen(false);
      reload();
    } catch (err) {
      setFormError(friendlyError(err));
    } finally {
      setSaving(false);
    }
  }

  const canWrite = can("settings:write");
  const list = registers ?? [];

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast notification */}
      {toast && (
        <div className="fixed top-6 right-6 z-50 rounded-2xl bg-slate-900 px-5 py-3 text-sm font-medium text-white shadow-xl transition-all">
          {toast}
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="flex items-center gap-2 text-xs font-semibold text-slate-500">
            <Link href="/configuracion/negocio" className="hover:text-slate-900">
              Configuración
            </Link>
            <span>/</span>
            <span className="text-slate-900">Cajas registradoras</span>
          </div>
          <h1 className="mt-1 text-3xl font-bold tracking-tight">Cajas registradoras</h1>
          <p className="mt-1 text-slate-500">
            Administrá los puntos de venta y gavetas de efectivo asignadas a cada sucursal.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {canWrite && (
            <button
              onClick={openCreate}
              className="flex h-11 items-center gap-2 rounded-2xl bg-brand px-5 text-sm font-bold text-white shadow-sm hover:bg-brand-dark"
            >
              <Plus className="size-4" />
              Nueva caja
            </button>
          )}
        </div>
      </div>

      {/* Branch selector if multiple */}
      {branches.length > 1 && (
        <div className="flex items-center justify-between rounded-2xl bg-slate-100/70 p-3 px-5">
          <div className="flex items-center gap-3">
            <Store className="size-5 text-slate-500" />
            <span className="text-sm font-semibold text-slate-700">Sucursal a configurar:</span>
          </div>
          <select
            value={selectedBranchId}
            onChange={(e) => setSelectedBranchId(e.target.value)}
            className="rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-medium text-slate-900 shadow-xs focus:border-slate-900 focus:ring-slate-900"
          >
            {branches.map((b) => (
              <option key={b.id} value={b.id}>
                {b.name} ({b.role})
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Registers table */}
      <div className="overflow-hidden rounded-3xl border border-slate-200/80 bg-white shadow-xs">
        {loading ? (
          <div className="flex items-center justify-center p-12 text-slate-400">
            <Loader2 className="size-6 animate-spin mr-2" />
            <span>Cargando cajas registradoras...</span>
          </div>
        ) : list.length === 0 ? (
          <div className="p-12 text-center">
            <Calculator className="mx-auto size-10 text-slate-300" />
            <h3 className="mt-3 text-base font-bold text-slate-900">No hay cajas registradoras</h3>
            <p className="mt-1 text-sm text-slate-500">Creá la primera caja para permitir la apertura de turnos.</p>
            {canWrite && (
              <button
                onClick={openCreate}
                className="mt-4 inline-flex items-center gap-2 rounded-2xl bg-brand px-4 py-2 text-sm font-semibold text-white hover:bg-brand-dark"
              >
                <Plus className="size-4" />
                Nueva caja
              </button>
            )}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm divide-y divide-slate-100">
              <thead className="bg-slate-50/80 text-xs font-semibold uppercase tracking-wider text-slate-500">
                <tr>
                  <th className="px-6 py-4">Código</th>
                  <th className="px-6 py-4">Nombre</th>
                  <th className="px-6 py-4">Bodega</th>
                  <th className="px-6 py-4">Base sugerida</th>
                  <th className="px-6 py-4">Estado</th>
                  <th className="px-6 py-4">Turno activo</th>
                  {canWrite && <th className="px-6 py-4 text-right">Acciones</th>}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 font-medium text-slate-700">
                {list.map((reg) => (
                  <tr key={reg.id} className="hover:bg-slate-50/50">
                    <td className="px-6 py-4 font-mono font-bold text-slate-900">{reg.code}</td>
                    <td className="px-6 py-4 font-bold text-slate-900">{reg.name}</td>
                    <td className="px-6 py-4">
                      <span className="inline-flex items-center rounded-lg bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-800">
                        {locations?.find((l) => l.id === reg.location_id)?.name ?? "Principal (defecto)"}
                      </span>
                    </td>
                    <td className="px-6 py-4 font-semibold text-slate-900">
                      {formatCOP(Number(reg.default_opening))}
                    </td>
                    <td className="px-6 py-4">
                      {reg.active ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-1 text-xs font-semibold text-emerald-700">
                          <CheckCircle2 className="size-3.5" />
                          Activa
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-600">
                          <XCircle className="size-3.5" />
                          Inactiva
                        </span>
                      )}
                    </td>
                    <td className="px-6 py-4">
                      {reg.open_session ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 text-xs font-semibold text-amber-700">
                          Turno abierto ({reg.open_session.opened_by_name || "Cajero"})
                        </span>
                      ) : (
                        <span className="text-slate-400">—</span>
                      )}
                    </td>
                    {canWrite && (
                      <td className="px-6 py-4 text-right">
                        <button
                          onClick={() => openEdit(reg)}
                          className="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 shadow-2xs hover:bg-slate-50"
                        >
                          <Edit2 className="size-3.5" />
                          Editar
                        </button>
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Modal Dialog */}
      {dialogOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 p-4 backdrop-blur-xs">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl sm:p-8">
            <div className="flex items-center justify-between border-b border-slate-100 pb-4">
              <h2 className="text-lg font-bold text-slate-900">
                {editingRegister ? "Editar caja registradora" : "Nueva caja registradora"}
              </h2>
              <button
                onClick={() => setDialogOpen(false)}
                className="rounded-full p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="size-5" />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="mt-6 space-y-4">
              {formError && (
                <div className="rounded-2xl bg-rose-50 p-4 text-sm font-medium text-rose-600">
                  {formError}
                </div>
              )}

              <div>
                <label className="block text-xs font-bold uppercase tracking-wider text-slate-600">
                  Nombre de la caja
                </label>
                <input
                  type="text"
                  required
                  maxLength={60}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Ej. Caja Principal, Caja Barra"
                  className="mt-1.5 block w-full rounded-2xl border border-slate-200 px-4 py-2.5 text-sm font-medium text-slate-900 focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>

              <div>
                <label className="block text-xs font-bold uppercase tracking-wider text-slate-600">
                  Código identificador
                </label>
                <input
                  type="text"
                  required
                  maxLength={10}
                  value={code}
                  onChange={(e) => setCode(e.target.value.toUpperCase())}
                  placeholder="Ej. CAJA-1"
                  className="mt-1.5 block w-full rounded-2xl border border-slate-200 px-4 py-2.5 font-mono text-sm font-bold text-slate-900 focus:border-brand focus:ring-1 focus:ring-brand uppercase"
                />
                <span className="mt-1 block text-xs text-slate-400">
                  Hasta 10 caracteres: letras mayúsculas, números o guiones.
                </span>
              </div>

              <div>
                <label className="block text-xs font-bold uppercase tracking-wider text-slate-600">
                  Bodega de la caja
                </label>
                <select
                  value={locationId}
                  onChange={(e) => setLocationId(e.target.value)}
                  className="mt-1.5 block w-full rounded-2xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-medium text-slate-900 focus:border-brand focus:ring-1 focus:ring-brand"
                >
                  <option value="">Bodega principal (por defecto)</option>
                  {(locations ?? [])
                    .filter((l) => l.active && l.sellable)
                    .map((loc) => (
                      <option key={loc.id} value={loc.id}>
                        {loc.name} {loc.is_default ? "(Principal)" : ""}
                      </option>
                    ))}
                </select>
                <span className="mt-1 block text-xs text-slate-400">
                  Las ventas cobradas en esta caja descontarán stock exclusivamente de esta bodega.
                </span>
              </div>

              <div>
                <label className="block text-xs font-bold uppercase tracking-wider text-slate-600">
                  Base sugerida de apertura
                </label>
                <div className="mt-1.5">
                  <MoneyInput
                    value={defaultOpening}
                    onChange={setDefaultOpening}
                    className="w-full text-base font-bold"
                  />
                </div>
                <span className="mt-1 block text-xs text-slate-400">
                  Monto predeterminado sugerido al abrir turno en esta caja.
                </span>
              </div>

              {editingRegister && (
                <div className="rounded-2xl border border-slate-100 bg-slate-50 p-4">
                  <label className="flex items-center justify-between cursor-pointer">
                    <div>
                      <span className="text-sm font-bold text-slate-900">Caja activa</span>
                      <span className="block text-xs text-slate-500">
                        Las cajas inactivas no aparecen disponibles para apertura de turnos.
                      </span>
                    </div>
                    <input
                      type="checkbox"
                      checked={active}
                      onChange={(e) => setActive(e.target.checked)}
                      className="size-5 rounded border-slate-300 text-brand focus:ring-brand"
                    />
                  </label>
                </div>
              )}

              <div className="mt-6 flex items-center justify-end gap-3 pt-4 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setDialogOpen(false)}
                  className="rounded-2xl px-5 py-2.5 text-sm font-bold text-slate-600 hover:bg-slate-100"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="flex items-center gap-2 rounded-2xl bg-brand px-6 py-2.5 text-sm font-bold text-white shadow-sm hover:bg-brand-dark disabled:opacity-50"
                >
                  {saving && <Loader2 className="size-4 animate-spin" />}
                  {editingRegister ? "Guardar cambios" : "Crear caja"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

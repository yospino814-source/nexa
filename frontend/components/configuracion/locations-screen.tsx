"use client";

import { useState } from "react";
import Link from "next/link";
import {
  CheckCircle2,
  Edit2,
  Loader2,
  Plus,
  Store,
  Warehouse,
  X,
  XCircle,
} from "lucide-react";

import {
  api,
  friendlyError,
  type LocationKind,
  type LocationRow,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";

const KIND_LABELS: Record<LocationKind, { label: string; color: string }> = {
  PISO_VENTA: { label: "Piso de venta", color: "bg-emerald-50 text-emerald-700 border-emerald-200" },
  BODEGA: { label: "Bodega", color: "bg-blue-50 text-blue-700 border-blue-200" },
  PRODUCCION: { label: "Producción", color: "bg-indigo-50 text-indigo-700 border-indigo-200" },
  RECEPCION: { label: "Recepción", color: "bg-cyan-50 text-cyan-700 border-cyan-200" },
  DESPACHO: { label: "Despacho", color: "bg-purple-50 text-purple-700 border-purple-200" },
  CUARENTENA: { label: "Cuarentena", color: "bg-amber-50 text-amber-700 border-amber-200" },
  AVERIADOS: { label: "Averiados / Merma", color: "bg-rose-50 text-rose-700 border-rose-200" },
};

export function LocationsScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const can = useSession((s) => s.can);

  const [selectedBranchId, setSelectedBranchId] = useState<string>(currentBranch.id);
  const [toast, setToast] = useState<string | null>(null);

  const {
    data: locations,
    loading,
    reload,
  } = useApi(() => api.locations.list(selectedBranchId), selectedBranchId);

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingLocation, setEditingLocation] = useState<LocationRow | null>(null);
  const [name, setName] = useState("");
  const [kind, setKind] = useState<LocationKind>("BODEGA");
  const [sellable, setSellable] = useState(true);
  const [isDefault, setIsDefault] = useState(false);
  const [active, setActive] = useState(true);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  function openCreate() {
    setEditingLocation(null);
    setName("");
    setKind("BODEGA");
    setSellable(true);
    setIsDefault(false);
    setActive(true);
    setFormError(null);
    setDialogOpen(true);
  }

  function openEdit(loc: LocationRow) {
    setEditingLocation(loc);
    setName(loc.name);
    setKind(loc.kind);
    setSellable(loc.sellable);
    setIsDefault(loc.is_default);
    setActive(loc.active);
    setFormError(null);
    setDialogOpen(true);
  }

  function handleKindChange(newKind: LocationKind) {
    setKind(newKind);
    if (newKind === "CUARENTENA" || newKind === "AVERIADOS") {
      setSellable(false);
      setIsDefault(false);
    }
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const cleanName = name.trim();

    if (!cleanName || cleanName.length > 40) {
      setFormError("El nombre debe tener entre 1 y 40 caracteres");
      return;
    }

    if ((kind === "CUARENTENA" || kind === "AVERIADOS") && sellable) {
      setFormError("Una bodega en cuarentena o averiados no puede ser vendible");
      return;
    }

    if (isDefault && !sellable) {
      setFormError("La bodega por defecto debe ser vendible");
      return;
    }

    setSaving(true);
    setFormError(null);
    try {
      if (editingLocation) {
        await api.locations.update(editingLocation.id, {
          name: cleanName,
          kind,
          sellable,
          is_default: isDefault,
          active,
        });
        setToast("Bodega actualizada con éxito");
      } else {
        await api.locations.create({
          branch_id: selectedBranchId,
          name: cleanName,
          kind,
          sellable,
          is_default: isDefault,
        });
        setToast("Bodega creada con éxito");
      }
      setDialogOpen(false);
      reload();
    } catch (err: unknown) {
      const errMsg = friendlyError(err);
      if (errMsg.includes("LOCATION_HAS_STOCK") || errMsg.includes("existencias")) {
        setFormError("No se puede desactivar una bodega que tiene existencias");
      } else {
        setFormError(errMsg);
      }
    } finally {
      setSaving(false);
    }
  }

  const canWrite = can("settings:write");
  const list = locations ?? [];

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
            <span className="text-slate-900">Bodegas y ubicaciones</span>
          </div>
          <h1 className="mt-1 text-2xl font-bold tracking-tight text-slate-900 sm:text-3xl">
            Bodegas
          </h1>
          <p className="mt-1 text-sm text-slate-500">
            Ubicaciones de inventario por sucursal para controlar existencias, mermas y aislamiento.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          {branches.length > 1 && (
            <div className="flex items-center gap-2 rounded-2xl border border-slate-200 bg-white px-3 py-2 shadow-sm">
              <Store className="h-4 w-4 text-slate-400" />
              <select
                value={selectedBranchId}
                onChange={(e) => setSelectedBranchId(e.target.value)}
                className="bg-transparent text-sm font-semibold text-slate-700 outline-none"
              >
                {branches.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name}
                  </option>
                ))}
              </select>
            </div>
          )}

          {canWrite && (
            <button
              onClick={openCreate}
              className="inline-flex items-center gap-2 rounded-2xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition hover:bg-indigo-500"
            >
              <Plus className="h-4 w-4" />
              Nueva bodega
            </button>
          )}
        </div>
      </div>

      {/* Locations List */}
      <div className="overflow-hidden rounded-3xl border border-slate-200/80 bg-white shadow-sm">
        <div className="border-b border-slate-100 px-6 py-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-bold uppercase tracking-wider text-slate-500">
              Bodegas de la sucursal ({list.length})
            </h2>
          </div>
        </div>

        {loading ? (
          <div className="flex items-center justify-center p-12 text-slate-400">
            <Loader2 className="h-8 w-8 animate-spin" />
          </div>
        ) : list.length === 0 ? (
          <div className="p-12 text-center">
            <Warehouse className="mx-auto h-12 w-12 text-slate-300" />
            <p className="mt-3 font-semibold text-slate-700">No hay bodegas configuradas</p>
            <p className="mt-1 text-sm text-slate-500">
              Crea bodegas o pisos de venta para organizar el inventario.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-slate-100">
            {list.map((loc) => {
              const kindCfg = KIND_LABELS[loc.kind] ?? {
                label: loc.kind,
                color: "bg-slate-50 text-slate-700 border-slate-200",
              };
              return (
                <div
                  key={loc.id}
                  className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between hover:bg-slate-50/50"
                >
                  <div className="space-y-1.5">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-bold text-slate-900">{loc.name}</span>
                      <span
                        className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold ${kindCfg.color}`}
                      >
                        {kindCfg.label}
                      </span>
                      {loc.is_default && (
                        <span className="inline-flex items-center rounded-full bg-slate-900 px-2 py-0.5 text-[11px] font-bold text-white">
                          Principal
                        </span>
                      )}
                      {loc.sellable ? (
                        <span className="inline-flex items-center gap-1 text-xs font-medium text-emerald-600">
                          <CheckCircle2 className="h-3.5 w-3.5" />
                          Vendible
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-xs font-medium text-slate-400">
                          <XCircle className="h-3.5 w-3.5" />
                          No vendible
                        </span>
                      )}
                      {!loc.active && (
                        <span className="inline-flex items-center rounded-full bg-rose-100 px-2 py-0.5 text-[11px] font-bold text-rose-700">
                          Inactiva
                        </span>
                      )}
                    </div>
                  </div>

                  {canWrite && (
                    <button
                      onClick={() => openEdit(loc)}
                      className="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 shadow-sm transition hover:bg-slate-50"
                    >
                      <Edit2 className="h-3.5 w-3.5 text-slate-400" />
                      Editar
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Dialog Create / Edit */}
      {dialogOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 pb-4">
              <h2 className="text-lg font-bold text-slate-900">
                {editingLocation ? "Editar bodega" : "Nueva bodega"}
              </h2>
              <button
                onClick={() => setDialogOpen(false)}
                className="rounded-xl p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
              >
                <X className="h-5 w-5" />
              </button>
            </div>

            {formError && (
              <div className="mt-4 rounded-2xl bg-rose-50 p-3 text-sm text-rose-700">
                {formError}
              </div>
            )}

            <form onSubmit={handleSubmit} className="mt-5 space-y-4">
              <div>
                <label className="text-xs font-bold uppercase tracking-wider text-slate-500">
                  Nombre de la bodega
                </label>
                <input
                  type="text"
                  required
                  maxLength={40}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Ej. Bodega Central, Piso de Venta"
                  className="mt-1.5 w-full rounded-2xl border border-slate-200 px-3.5 py-2.5 text-sm font-medium text-slate-900 outline-none focus:border-indigo-600 focus:ring-1 focus:ring-indigo-600"
                />
              </div>

              <div>
                <label className="text-xs font-bold uppercase tracking-wider text-slate-500">
                  Tipo de ubicación
                </label>
                <select
                  value={kind}
                  onChange={(e) => handleKindChange(e.target.value as LocationKind)}
                  className="mt-1.5 w-full rounded-2xl border border-slate-200 bg-white px-3.5 py-2.5 text-sm font-medium text-slate-900 outline-none focus:border-indigo-600 focus:ring-1 focus:ring-indigo-600"
                >
                  <option value="PISO_VENTA">Piso de venta</option>
                  <option value="BODEGA">Bodega</option>
                  <option value="PRODUCCION">Producción</option>
                  <option value="RECEPCION">Recepción</option>
                  <option value="DESPACHO">Despacho</option>
                  <option value="CUARENTENA">Cuarentena</option>
                  <option value="AVERIADOS">Averiados / Merma</option>
                </select>
              </div>

              <div className="space-y-3 pt-2">
                <label className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={sellable}
                    disabled={kind === "CUARENTENA" || kind === "AVERIADOS"}
                    onChange={(e) => setSellable(e.target.checked)}
                    className="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-600"
                  />
                  <div>
                    <span className="text-sm font-semibold text-slate-800">
                      Permitir ventas (Vendible)
                    </span>
                    {(kind === "CUARENTENA" || kind === "AVERIADOS") && (
                      <p className="text-xs text-amber-600">
                        Cuarentena y Averiados no pueden ser vendibles.
                      </p>
                    )}
                  </div>
                </label>

                <label className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={isDefault}
                    disabled={!sellable}
                    onChange={(e) => setIsDefault(e.target.checked)}
                    className="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-600"
                  />
                  <div>
                    <span className="text-sm font-semibold text-slate-800">
                      Bodega principal por defecto
                    </span>
                    <p className="text-xs text-slate-500">
                      Se usará para reservas y cajas sin bodega asignada.
                    </p>
                  </div>
                </label>

                {editingLocation && (
                  <label className="flex items-center gap-3">
                    <input
                      type="checkbox"
                      checked={active}
                      onChange={(e) => setActive(e.target.checked)}
                      className="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-600"
                    />
                    <div>
                      <span className="text-sm font-semibold text-slate-800">
                        Bodega activa
                      </span>
                      <p className="text-xs text-slate-500">
                        No se puede desactivar si tiene existencias de productos.
                      </p>
                    </div>
                  </label>
                )}
              </div>

              <div className="flex items-center justify-end gap-3 pt-4 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setDialogOpen(false)}
                  className="rounded-2xl border border-slate-200 px-4 py-2.5 text-sm font-semibold text-slate-700 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="inline-flex items-center gap-2 rounded-2xl bg-indigo-600 px-5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-indigo-500 disabled:opacity-50"
                >
                  {saving && <Loader2 className="h-4 w-4 animate-spin" />}
                  {editingLocation ? "Guardar cambios" : "Crear bodega"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}


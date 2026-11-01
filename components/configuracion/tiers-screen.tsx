"use client";

import { useState } from "react";
import {
  CheckCircle2,
  Edit2,
  Loader2,
  Package,
  Plus,
  Tag,
  Users,
  X,
  XCircle,
} from "lucide-react";

import {
  api,
  friendlyError,
  type CreatePriceTierInput,
  type PriceTier,
  type TierAdjustment,
  type TierKind,
  type UpdatePriceTierInput,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useSession } from "@/lib/stores/session";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

function formatAdjustment(adj: TierAdjustment, val: string | number): string {
  const num = Number(val);
  switch (adj) {
    case "NINGUNO":
      return "Sin ajuste";
    case "DESCUENTO_PCT":
      return `−${num}%`;
    case "RECARGO_PCT":
      return `+${num}%`;
    case "DESCUENTO_FIJO":
      return `−${formatCOP(num)}`;
    case "RECARGO_FIJO":
      return `+${formatCOP(num)}`;
    case "PRECIO_FIJO":
      return `Precio fijo ${formatCOP(num)}`;
    default:
      return String(val);
  }
}

export function TiersScreen() {
  const can = useSession((s) => s.can);
  const canWrite = can("settings:write");

  const [toast, setToast] = useState<string | null>(null);

  const { data: tiers, loading, reload } = useApi(() => api.tiers.list(), "tiers");

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingTier, setEditingTier] = useState<PriceTier | null>(null);
  const [name, setName] = useState("");
  const [kind, setKind] = useState<TierKind>("CLIENTE");
  const [adjustment, setAdjustment] = useState<TierAdjustment>("DESCUENTO_PCT");
  const [value, setValue] = useState("10");
  const [unitsPerPackage, setUnitsPerPackage] = useState("");
  const [active, setActive] = useState(true);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  function openCreate() {
    setEditingTier(null);
    setName("");
    setKind("CLIENTE");
    setAdjustment("DESCUENTO_PCT");
    setValue("10");
    setUnitsPerPackage("");
    setActive(true);
    setFormError(null);
    setDialogOpen(true);
  }

  function openEdit(tier: PriceTier) {
    setEditingTier(tier);
    setName(tier.name);
    setKind(tier.kind);
    setAdjustment(tier.adjustment);
    setValue(tier.value);
    setUnitsPerPackage(tier.units_per_package ? String(Number(tier.units_per_package)) : "");
    setActive(tier.active);
    setFormError(null);
    setDialogOpen(true);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setFormError("El nombre es obligatorio");
      return;
    }
    if (kind === "PRESENTACION" && unitsPerPackage && Number(unitsPerPackage) <= 0) {
      setFormError("Las unidades por empaque deben ser mayores a cero");
      return;
    }

    setSaving(true);
    setFormError(null);

    try {
      if (editingTier) {
        const payload: UpdatePriceTierInput = {
          name: name.trim(),
          kind: editingTier.kind,
          adjustment,
          value: value || "0",
          units_per_package: kind === "PRESENTACION" && unitsPerPackage ? unitsPerPackage : null,
          active,
        };
        await api.tiers.update(editingTier.id, payload);
        setToast("Tarifa actualizada correctamente");
      } else {
        const payload: CreatePriceTierInput = {
          name: name.trim(),
          kind,
          adjustment,
          value: value || "0",
          units_per_package: kind === "PRESENTACION" && unitsPerPackage ? unitsPerPackage : null,
        };
        await api.tiers.create(payload);
        setToast("Tarifa creada correctamente");
      }
      setDialogOpen(false);
      reload();
    } catch (err) {
      setFormError(friendlyError(err));
    } finally {
      setSaving(false);
    }
  }

  const clientTiers = tiers?.filter((t) => t.kind === "CLIENTE") ?? [];
  const presentationTiers = tiers?.filter((t) => t.kind === "PRESENTACION") ?? [];

  return (
    <div className="space-y-6">
      {/* Toast */}
      {toast && (
        <div className="flex items-center justify-between rounded-2xl bg-emerald-50 px-4 py-3 text-sm text-emerald-800 border border-emerald-200">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="size-4 text-emerald-600" />
            <span>{toast}</span>
          </div>
          <button onClick={() => setToast(null)} className="text-emerald-600 hover:text-emerald-800">
            <X className="size-4" />
          </button>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900">Tarifas y presentaciones</h1>
          <p className="text-sm text-slate-500">
            Configurá precios por tipo de cliente (Mayoristas) y presentaciones por empaque (Pacas, Bultos, Cajas).
          </p>
        </div>

        {canWrite && (
          <button
            onClick={openCreate}
            className="flex items-center justify-center gap-2 rounded-xl bg-teal-600 px-4 py-2.5 text-sm font-semibold text-white shadow-xs hover:bg-teal-700 transition-colors"
          >
            <Plus className="size-4" />
            Nueva tarifa
          </button>
        )}
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="rounded-3xl border border-slate-200/80 bg-white p-5 shadow-xs">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-2xl bg-teal-50 text-teal-600">
              <Tag className="size-5" />
            </div>
            <div>
              <p className="text-xs font-medium text-slate-500">Total tarifas</p>
              <p className="text-xl font-bold text-slate-900">{tiers?.length ?? 0}</p>
            </div>
          </div>
        </div>

        <div className="rounded-3xl border border-slate-200/80 bg-white p-5 shadow-xs">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-2xl bg-blue-50 text-blue-600">
              <Users className="size-5" />
            </div>
            <div>
              <p className="text-xs font-medium text-slate-500">Tarifas de cliente</p>
              <p className="text-xl font-bold text-slate-900">{clientTiers.length}</p>
            </div>
          </div>
        </div>

        <div className="rounded-3xl border border-slate-200/80 bg-white p-5 shadow-xs">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-2xl bg-purple-50 text-purple-600">
              <Package className="size-5" />
            </div>
            <div>
              <p className="text-xs font-medium text-slate-500">Presentaciones</p>
              <p className="text-xl font-bold text-slate-900">{presentationTiers.length}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="rounded-3xl border border-slate-200/80 bg-white shadow-xs overflow-hidden">
        {loading ? (
          <div className="flex items-center justify-center p-12 text-slate-400">
            <Loader2 className="size-6 animate-spin text-teal-600" />
            <span className="ml-2 text-sm">Cargando tarifas...</span>
          </div>
        ) : !tiers || tiers.length === 0 ? (
          <div className="p-12 text-center text-slate-500">
            <Tag className="mx-auto size-8 text-slate-300 mb-2" />
            <p className="text-base font-semibold text-slate-700">No hay tarifas registradas</p>
            <p className="text-sm text-slate-400 mt-1">
              Creá tu primera tarifa de cliente o presentación de producto.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600">
              <thead className="bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 border-b border-slate-200">
                <tr>
                  <th className="px-6 py-4">Nombre</th>
                  <th className="px-6 py-4">Tipo</th>
                  <th className="px-6 py-4">Ajuste / Regla</th>
                  <th className="px-6 py-4 text-center">Unidades</th>
                  <th className="px-6 py-4 text-center">Estado</th>
                  {canWrite && <th className="px-6 py-4 text-right">Acciones</th>}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {tiers.map((tier) => (
                  <tr key={tier.id} className="hover:bg-slate-50/60 transition-colors">
                    <td className="px-6 py-4 font-semibold text-slate-900">
                      {tier.name}
                    </td>
                    <td className="px-6 py-4">
                      {tier.kind === "CLIENTE" ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-2.5 py-1 text-xs font-semibold text-blue-700 border border-blue-200">
                          <Users className="size-3" /> Cliente
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-purple-50 px-2.5 py-1 text-xs font-semibold text-purple-700 border border-purple-200">
                          <Package className="size-3" /> Presentación
                        </span>
                      )}
                    </td>
                    <td className="px-6 py-4 font-medium text-slate-800">
                      {formatAdjustment(tier.adjustment, tier.value)}
                    </td>
                    <td className="px-6 py-4 text-center tabular-nums">
                      {tier.units_per_package ? `${Number(tier.units_per_package)} un.` : "—"}
                    </td>
                    <td className="px-6 py-4 text-center">
                      {tier.active ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2.5 py-1 text-xs font-semibold text-emerald-700 border border-emerald-200">
                          <CheckCircle2 className="size-3" /> Activa
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-500">
                          <XCircle className="size-3" /> Inactiva
                        </span>
                      )}
                    </td>
                    {canWrite && (
                      <td className="px-6 py-4 text-right">
                        <button
                          onClick={() => openEdit(tier)}
                          className="inline-flex size-8 items-center justify-center rounded-lg text-slate-400 hover:bg-slate-100 hover:text-slate-600 transition-colors"
                          title="Editar tarifa"
                        >
                          <Edit2 className="size-4" />
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

      {/* Create / Edit Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-md rounded-3xl p-6">
          <DialogTitle className="text-xl font-bold text-slate-900">
            {editingTier ? "Editar tarifa" : "Nueva tarifa"}
          </DialogTitle>

          <form onSubmit={handleSubmit} className="mt-4 space-y-4">
            {formError && (
              <div className="rounded-xl bg-rose-50 p-3 text-xs text-rose-700 border border-rose-200">
                {formError}
              </div>
            )}

            <div>
              <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                Nombre
              </label>
              <input
                type="text"
                required
                maxLength={40}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Ej. Mayorista, Paca x12, Bulto 50 kg"
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-600 focus:outline-none"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                Tipo de tarifa
              </label>
              <select
                disabled={!!editingTier}
                value={kind}
                onChange={(e) => setKind(e.target.value as TierKind)}
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-600 focus:outline-none disabled:bg-slate-100 disabled:text-slate-500"
              >
                <option value="CLIENTE">Cliente (se asigna al cliente o en la venta)</option>
                <option value="PRESENTACION">Presentación (empaque con unidades)</option>
              </select>
              {editingTier && (
                <p className="mt-1 text-xs text-slate-400">El tipo de tarifa no se puede modificar.</p>
              )}
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                  Tipo de ajuste
                </label>
                <select
                  value={adjustment}
                  onChange={(e) => setAdjustment(e.target.value as TierAdjustment)}
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-600 focus:outline-none"
                >
                  <option value="DESCUENTO_PCT">Descuento %</option>
                  <option value="RECARGO_PCT">Recargo %</option>
                  <option value="DESCUENTO_FIJO">Descuento fijo ($)</option>
                  <option value="RECARGO_FIJO">Recargo fijo ($)</option>
                  <option value="PRECIO_FIJO">Precio fijo ($)</option>
                  <option value="NINGUNO">Sin ajuste</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                  Valor
                </label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  disabled={adjustment === "NINGUNO"}
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  placeholder="0.00"
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-600 focus:outline-none disabled:bg-slate-100 disabled:text-slate-400"
                />
              </div>
            </div>

            {kind === "PRESENTACION" && (
              <div>
                <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                  Unidades por empaque (por defecto)
                </label>
                <input
                  type="number"
                  step="1"
                  min="1"
                  value={unitsPerPackage}
                  onChange={(e) => setUnitsPerPackage(e.target.value)}
                  placeholder="Ej. 12 para docena, 50 para bulto"
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-600 focus:outline-none"
                />
                <p className="mt-1 text-xs text-slate-400">
                  Podés dejarlo vacío si cada producto tiene su propia cantidad de unidades.
                </p>
              </div>
            )}

            {editingTier && (
              <div className="flex items-center gap-2 pt-1">
                <input
                  type="checkbox"
                  id="active"
                  checked={active}
                  onChange={(e) => setActive(e.target.checked)}
                  className="size-4 rounded text-teal-600 focus:ring-teal-500"
                />
                <label htmlFor="active" className="text-sm font-medium text-slate-700">
                  Tarifa activa
                </label>
              </div>
            )}

            <div className="flex justify-end gap-3 pt-3">
              <button
                type="button"
                onClick={() => setDialogOpen(false)}
                className="rounded-xl border border-slate-200 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={saving}
                className="flex items-center gap-2 rounded-xl bg-teal-600 px-4 py-2 text-sm font-semibold text-white hover:bg-teal-700 disabled:opacity-50"
              >
                {saving && <Loader2 className="size-4 animate-spin" />}
                {editingTier ? "Guardar cambios" : "Crear tarifa"}
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}

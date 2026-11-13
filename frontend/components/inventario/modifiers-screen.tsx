"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { ArrowLeft, CheckCircle2, Edit2, Plus, Sliders, Trash2, X } from "lucide-react";

import {
  api,
  type CreateOptionGroupInput,
  type OptionGroupDetail,
  type OptionInput,
  type ProductRow,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

export function ModifiersScreen() {
  const [groups, setGroups] = useState<OptionGroupDetail[]>([]);
  const [products, setProducts] = useState<ProductRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Modal de edición / creación
  const [modalOpen, setModalOpen] = useState(false);
  const [editingGroupId, setEditingGroupId] = useState<string | null>(null);
  const [groupName, setGroupName] = useState("");
  const [minSelect, setMinSelect] = useState(0);
  const [maxSelect, setMaxSelect] = useState(1);
  const [groupActive, setGroupActive] = useState(true);
  const [optionsList, setOptionsList] = useState<OptionInput[]>([]);
  const [saving, setSaving] = useState(false);

  async function loadData() {
    try {
      const [groupsData, prodsRes] = await Promise.all([
        api.options.listGroups(),
        api.products.list({ page_size: 100 }),
      ]);
      setGroups(groupsData);
      setProducts(prodsRes.items);
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : "Error al cargar modificadores");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadData();
  }, []);

  function openCreateModal() {
    setEditingGroupId(null);
    setGroupName("");
    setMinSelect(0);
    setMaxSelect(1);
    setGroupActive(true);
    setOptionsList([
      {
        name: "",
        price_delta: "0",
        input_product_id: null,
        input_qty: null,
        sort_order: 1,
        active: true,
      },
    ]);
    setErrorMsg(null);
    setModalOpen(true);
  }

  function openEditModal(group: OptionGroupDetail) {
    setEditingGroupId(group.id);
    setGroupName(group.name);
    setMinSelect(group.min_select);
    setMaxSelect(group.max_select);
    setGroupActive(group.active);
    setOptionsList(
      group.options.map((o) => ({
        id: o.id,
        name: o.name,
        price_delta: o.price_delta,
        input_product_id: o.input_product_id ?? null,
        input_qty: o.input_qty ?? null,
        sort_order: o.sort_order,
        active: o.active,
      }))
    );
    setErrorMsg(null);
    setModalOpen(true);
  }

  function addOptionRow() {
    setOptionsList([
      ...optionsList,
      {
        name: "",
        price_delta: "0",
        input_product_id: null,
        input_qty: null,
        sort_order: optionsList.length + 1,
        active: true,
      },
    ]);
  }

  function updateOptionRow(index: number, patch: Partial<OptionInput>) {
    const updated = [...optionsList];
    updated[index] = { ...updated[index], ...patch };
    setOptionsList(updated);
  }

  function removeOptionRow(index: number) {
    setOptionsList(optionsList.filter((_, idx) => idx !== index));
  }

  async function handleSaveGroup(e: React.FormEvent) {
    e.preventDefault();
    if (!groupName.trim()) {
      setErrorMsg("El nombre del grupo es obligatorio (1 a 40 caracteres)");
      return;
    }
    if (minSelect < 0) {
      setErrorMsg("El mínimo a seleccionar no puede ser negativo");
      return;
    }
    if (maxSelect < 1) {
      setErrorMsg("El máximo a seleccionar debe ser al menos 1");
      return;
    }
    if (minSelect > maxSelect) {
      setErrorMsg("El mínimo no puede superar el máximo");
      return;
    }

    for (const opt of optionsList) {
      if (!opt.name.trim()) {
        setErrorMsg("Todas las opciones deben tener un nombre");
        return;
      }
      if (opt.input_product_id && (!opt.input_qty || Number(opt.input_qty) <= 0)) {
        setErrorMsg(`La opción "${opt.name}" tiene insumo seleccionado pero falta la cantidad (debe ser > 0)`);
        return;
      }
    }

    setSaving(true);
    setErrorMsg(null);
    try {
      const payload: CreateOptionGroupInput = {
        name: groupName.trim(),
        min_select: minSelect,
        max_select: maxSelect,
        active: groupActive,
        options: optionsList.map((o, idx) => ({
          ...o,
          name: o.name.trim(),
          sort_order: o.sort_order ?? idx + 1,
          input_qty: o.input_product_id ? o.input_qty : null,
        })),
      };

      if (editingGroupId) {
        await api.options.updateGroup(editingGroupId, payload);
        setToast("Grupo de modificadores actualizado correctamente");
      } else {
        await api.options.createGroup(payload);
        setToast("Grupo de modificadores creado exitosamente");
      }
      setModalOpen(false);
      await loadData();
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : "Error al guardar grupo");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-6 p-6 lg:p-10 max-w-5xl">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <Link
            href="/inventario"
            className="flex size-10 items-center justify-center rounded-full border border-slate-200 bg-white hover:bg-slate-50 text-slate-600 transition-colors"
          >
            <ArrowLeft className="size-4" />
          </Link>
          <div>
            <h1 className="text-2xl font-bold tracking-tight">Modificadores y Opciones</h1>
            <p className="text-sm text-slate-500">
              Grupos de personalización, adiciones y combos con descuento automático de insumos
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={openCreateModal}
          className="flex h-10 items-center gap-2 rounded-full bg-emerald-600 px-4 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 transition-colors"
        >
          <Plus className="size-4" /> Nuevo grupo
        </button>
      </div>

      {toast && (
        <p className="flex items-center gap-2 rounded-2xl border border-emerald-200 bg-emerald-50 p-3 text-sm font-medium text-emerald-700">
          <CheckCircle2 className="size-4" /> {toast}
        </p>
      )}

      {errorMsg && (
        <p className="flex items-center gap-2 rounded-2xl border border-rose-200 bg-rose-50 p-3 text-sm font-medium text-rose-700">
          <X className="size-4" /> {errorMsg}
        </p>
      )}

      {/* Lista de grupos */}
      <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
        {loading ? (
          <div className="p-8 text-center text-sm text-slate-500">Cargando grupos de modificadores...</div>
        ) : groups.length === 0 ? (
          <div className="p-10 text-center">
            <Sliders className="mx-auto size-10 text-slate-300 mb-3" />
            <h3 className="text-base font-semibold text-slate-800">No hay grupos de modificadores</h3>
            <p className="text-sm text-slate-500 mt-1 max-w-sm mx-auto">
              Crea tu primer grupo (ej. Toppings, Término de carne, Salsas) para asignarlo a tus productos.
            </p>
            <button
              type="button"
              onClick={openCreateModal}
              className="mt-4 inline-flex items-center gap-2 rounded-full bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-emerald-700"
            >
              <Plus className="size-4" /> Crear grupo
            </button>
          </div>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="border-b border-slate-100 bg-slate-50/50 text-xs font-semibold text-slate-500 uppercase tracking-wider">
              <tr>
                <th className="px-6 py-3.5">Grupo</th>
                <th className="px-6 py-3.5">Regla de selección</th>
                <th className="px-6 py-3.5">Opciones configuradas</th>
                <th className="px-6 py-3.5">Estado</th>
                <th className="px-6 py-3.5 text-right">Acciones</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {groups.map((g) => (
                <tr key={g.id} className="hover:bg-slate-50/50 transition-colors">
                  <td className="px-6 py-4 font-semibold text-slate-900">{g.name}</td>
                  <td className="px-6 py-4 text-slate-600">
                    {g.min_select === 0 && g.max_select === 1 && "Opcional (máx. 1)"}
                    {g.min_select === 1 && g.max_select === 1 && "Obligatorio (exacto 1)"}
                    {g.min_select === 0 && g.max_select > 1 && `Opcional (máx. ${g.max_select})`}
                    {g.min_select > 1 && `Obligatorio (mín. ${g.min_select}, máx. ${g.max_select})`}
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex flex-wrap gap-1.5">
                      {g.options.map((opt) => (
                        <span
                          key={opt.id}
                          className={cn(
                            "inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-xs font-medium",
                            opt.active
                              ? "bg-slate-100 text-slate-700"
                              : "bg-slate-50 text-slate-400 line-through"
                          )}
                        >
                          {opt.name}
                          {Number(opt.price_delta) > 0 && (
                            <span className="text-emerald-700 font-semibold">
                              +{formatCOP(Number(opt.price_delta))}
                            </span>
                          )}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <span
                      className={cn(
                        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                        g.active
                          ? "bg-emerald-50 text-emerald-700 border border-emerald-200"
                          : "bg-slate-100 text-slate-600"
                      )}
                    >
                      {g.active ? "Activo" : "Inactivo"}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <button
                      type="button"
                      onClick={() => openEditModal(g)}
                      className="inline-flex items-center gap-1.5 rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 transition-colors"
                    >
                      <Edit2 className="size-3.5" /> Editar
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Modal Crear / Editar Grupo */}
      {modalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm">
          <div className="w-full max-w-2xl max-h-[90vh] flex flex-col rounded-2xl bg-white shadow-xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-slate-100 px-6 py-4">
              <h2 className="text-lg font-bold text-slate-900">
                {editingGroupId ? "Editar grupo de modificadores" : "Nuevo grupo de modificadores"}
              </h2>
              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="rounded-full p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600 transition-colors"
              >
                <X className="size-5" />
              </button>
            </div>

            <form onSubmit={handleSaveGroup} className="flex-1 overflow-y-auto p-6 space-y-6">
              {errorMsg && (
                <p className="flex items-center gap-2 rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs font-medium text-rose-700">
                  <X className="size-4 shrink-0" /> {errorMsg}
                </p>
              )}

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-1.5 md:col-span-2">
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    Nombre del grupo *
                  </label>
                  <input
                    type="text"
                    required
                    maxLength={40}
                    value={groupName}
                    onChange={(e) => setGroupName(e.target.value)}
                    placeholder="Ej. Toppings adicionales, Salsas, Término..."
                    className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-600 focus:outline-none"
                  />
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    Mínimo a seleccionar *
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={minSelect}
                    onChange={(e) => setMinSelect(Number(e.target.value))}
                    className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-600 focus:outline-none"
                  />
                  <p className="text-[11px] text-slate-400">
                    0 = opcional. 1 o más = obligatorio para el cajero.
                  </p>
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    Máximo a seleccionar *
                  </label>
                  <input
                    type="number"
                    min={1}
                    value={maxSelect}
                    onChange={(e) => setMaxSelect(Number(e.target.value))}
                    className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-600 focus:outline-none"
                  />
                  <p className="text-[11px] text-slate-400">
                    1 = selección única (radio). Mayor a 1 = múltiples casillas.
                  </p>
                </div>

                <div className="md:col-span-2 flex items-center gap-2 pt-1">
                  <input
                    type="checkbox"
                    id="groupActiveCheck"
                    checked={groupActive}
                    onChange={(e) => setGroupActive(e.target.checked)}
                    className="size-4 rounded border-slate-300 text-emerald-600 focus:ring-emerald-500"
                  />
                  <label htmlFor="groupActiveCheck" className="text-sm font-medium text-slate-700">
                    Grupo activo para selección en el punto de venta
                  </label>
                </div>
              </div>

              {/* Lista de opciones */}
              <div className="space-y-3 pt-2 border-t border-slate-100">
                <div className="flex items-center justify-between">
                  <h3 className="text-sm font-bold text-slate-900">Opciones del grupo</h3>
                  <button
                    type="button"
                    onClick={addOptionRow}
                    className="inline-flex items-center gap-1 text-xs font-medium text-emerald-600 hover:text-emerald-700"
                  >
                    <Plus className="size-3.5" /> Agregar opción
                  </button>
                </div>

                {optionsList.length === 0 ? (
                  <p className="text-xs text-slate-400 italic">No hay opciones. Agrega al menos una opción al grupo.</p>
                ) : (
                  <div className="space-y-3">
                    {optionsList.map((opt, idx) => (
                      <div
                        key={opt.id ?? idx}
                        className="p-3 rounded-xl border border-slate-200 bg-slate-50/50 space-y-3"
                      >
                        <div className="flex items-center gap-2">
                          <input
                            type="text"
                            required
                            maxLength={40}
                            placeholder="Nombre de la opción (ej. Extra queso)"
                            value={opt.name}
                            onChange={(e) => updateOptionRow(idx, { name: e.target.value })}
                            className="flex-1 rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm focus:border-emerald-600 focus:outline-none"
                          />
                          <div className="w-32 flex items-center gap-1">
                            <span className="text-xs text-slate-400">$+</span>
                            <input
                              type="number"
                              min={0}
                              placeholder="0"
                              value={opt.price_delta}
                              onChange={(e) => updateOptionRow(idx, { price_delta: e.target.value })}
                              className="w-full rounded-lg border border-slate-200 bg-white px-2 py-1.5 text-sm focus:border-emerald-600 focus:outline-none"
                            />
                          </div>
                          <button
                            type="button"
                            onClick={() => removeOptionRow(idx)}
                            className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors"
                          >
                            <Trash2 className="size-4" />
                          </button>
                        </div>

                        {/* Insumo opcional para descuento automático */}
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
                          <div>
                            <label className="text-[10px] uppercase font-semibold text-slate-400 block mb-0.5">
                              Insumo a descontar (opcional)
                            </label>
                            <select
                              value={opt.input_product_id ?? ""}
                              onChange={(e) =>
                                updateOptionRow(idx, {
                                  input_product_id: e.target.value ? e.target.value : null,
                                })
                              }
                              className="w-full rounded-lg border border-slate-200 bg-white px-2 py-1.5 text-xs focus:border-emerald-600 focus:outline-none"
                            >
                              <option value="">Ninguno (sin receta)</option>
                              {products.map((p) => (
                                <option key={p.id} value={p.id}>
                                  {p.name} ({p.sku || "Sin SKU"})
                                </option>
                              ))}
                            </select>
                          </div>

                          {opt.input_product_id && (
                            <div>
                              <label className="text-[10px] uppercase font-semibold text-slate-400 block mb-0.5">
                                Cantidad a descontar por unidad
                              </label>
                              <input
                                type="number"
                                step="0.0001"
                                min="0.0001"
                                placeholder="Ej. 30"
                                value={opt.input_qty ?? ""}
                                onChange={(e) => updateOptionRow(idx, { input_qty: e.target.value })}
                                className="w-full rounded-lg border border-slate-200 bg-white px-2 py-1.5 text-xs focus:border-emerald-600 focus:outline-none"
                              />
                            </div>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="flex items-center justify-end gap-3 pt-4 border-t border-slate-100">
                <button
                  type="button"
                  onClick={() => setModalOpen(false)}
                  className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 transition-colors"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="rounded-full bg-emerald-600 px-5 py-2 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 transition-colors disabled:opacity-50"
                >
                  {saving ? "Guardando..." : "Guardar grupo"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

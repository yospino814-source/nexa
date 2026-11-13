"use client";

import { useEffect, useMemo, useState } from "react";
import {
  Calendar,
  CheckCircle2,
  Edit2,
  Loader2,
  Package,
  Pause,
  Play,
  Plus,
  Search,
  Sparkles,
  Tag,
  Users,
  XCircle,
} from "lucide-react";

import {
  api,
  friendlyError,
  type CatalogItem,
  type CreatePromotionInput,
  type Promotion,
  type PromotionEffectiveStatus,
  type PromotionKind,
  type PromotionScope,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

const STATUS_TABS: { id: string; label: string }[] = [
  { id: "TODAS", label: "Todas" },
  { id: "ACTIVA", label: "Activas" },
  { id: "PROGRAMADA", label: "Programadas" },
  { id: "PAUSADA", label: "Pausadas" },
  { id: "VENCIDA", label: "Vencidas" },
  { id: "BORRADOR", label: "Borradores" },
];

function statusBadge(st: PromotionEffectiveStatus) {
  switch (st) {
    case "ACTIVA":
      return { label: "Activa", bg: "bg-emerald-50 text-emerald-700 border-emerald-200" };
    case "PROGRAMADA":
      return { label: "Programada", bg: "bg-blue-50 text-blue-700 border-blue-200" };
    case "PAUSADA":
      return { label: "Pausada", bg: "bg-amber-50 text-amber-700 border-amber-200" };
    case "VENCIDA":
      return { label: "Vencida", bg: "bg-slate-100 text-slate-600 border-slate-200" };
    case "BORRADOR":
      return { label: "Borrador", bg: "bg-purple-50 text-purple-700 border-purple-200" };
    default:
      return { label: st, bg: "bg-slate-100 text-slate-700 border-slate-200" };
  }
}

export function PromotionsScreen() {
  const branch = useCurrentBranch();
  const can = useSession((s) => s.can);
  const canWrite = can("settings:write");

  const [tab, setTab] = useState("TODAS");
  const [toast, setToast] = useState<string | null>(null);

  const { data: promotions, loading, reload } = useApi(
    () => api.promotions.list(),
    "promotions",
  );

  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  useEffect(() => {
    api.catalog.list(branch.id).then(setCatalog).catch(() => {});
  }, [branch.id]);

  // Dialog State
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingPromo, setEditingPromo] = useState<Promotion | null>(null);

  const [name, setName] = useState("");
  const [kind, setKind] = useState<PromotionKind>("PORCENTAJE");
  const [value, setValue] = useState("10");
  const [scope, setScope] = useState<PromotionScope>("PRODUCTO");
  const [minQty, setMinQty] = useState("");
  const [minSubtotal, setMinSubtotal] = useState("");
  const [maxDiscount, setMaxDiscount] = useState("");
  const [startsAt, setStartsAt] = useState(() => new Date().toISOString().slice(0, 16));
  const [endsAt, setEndsAt] = useState("");
  const [priority, setPriority] = useState("0");
  const [usageLimit, setUsageLimit] = useState("");
  const [perCustomerLimit, setPerCustomerLimit] = useState("");
  const [skipTierLines, setSkipTierLines] = useState(true);

  const [selectedProductIds, setSelectedProductIds] = useState<string[]>([]);
  const [selectedCategoryNames, setSelectedCategoryNames] = useState<string[]>([]);
  const [searchTarget, setSearchTarget] = useState("");

  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const categories = useMemo(() => {
    return Array.from(new Set(catalog.map((i) => i.category).filter((c): c is string => !!c))).sort();
  }, [catalog]);

  function openCreate() {
    setEditingPromo(null);
    setName("");
    setKind("PORCENTAJE");
    setValue("10");
    setScope("PRODUCTO");
    setMinQty("");
    setMinSubtotal("");
    setMaxDiscount("");
    setStartsAt(new Date().toISOString().slice(0, 16));
    setEndsAt("");
    setPriority("0");
    setUsageLimit("");
    setPerCustomerLimit("");
    setSkipTierLines(true);
    setSelectedProductIds([]);
    setSelectedCategoryNames([]);
    setSearchTarget("");
    setFormError(null);
    setDialogOpen(true);
  }

  function openEdit(promo: Promotion) {
    setEditingPromo(promo);
    setName(promo.name);
    setKind(promo.kind);
    setValue(String(Number(promo.value)));
    setScope(promo.scope);
    setMinQty(promo.min_qty ? String(Number(promo.min_qty)) : "");
    setMinSubtotal(promo.min_subtotal ? String(Number(promo.min_subtotal)) : "");
    setMaxDiscount(promo.max_discount ? String(Number(promo.max_discount)) : "");
    setStartsAt(new Date(promo.starts_at).toISOString().slice(0, 16));
    setEndsAt(promo.ends_at ? new Date(promo.ends_at).toISOString().slice(0, 16) : "");
    setPriority(String(promo.priority));
    setUsageLimit(promo.usage_limit ? String(promo.usage_limit) : "");
    setPerCustomerLimit(promo.per_customer_limit ? String(promo.per_customer_limit) : "");
    setSkipTierLines(promo.skip_tier_lines);
    setSelectedProductIds(promo.product_ids || []);
    setSelectedCategoryNames([]);
    setSearchTarget("");
    setFormError(null);
    setDialogOpen(true);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setFormError("El nombre de la promoción es obligatorio");
      return;
    }
    const valNum = Number(value);
    if (!valNum || valNum <= 0) {
      setFormError("El valor del descuento debe ser mayor a 0");
      return;
    }
    if (kind === "PORCENTAJE" && valNum > 100) {
      setFormError("El porcentaje no puede ser mayor a 100%");
      return;
    }
    if (scope === "PRODUCTO" && selectedProductIds.length === 0) {
      setFormError("Debés seleccionar al menos un producto");
      return;
    }
    if (scope === "CATEGORIA" && selectedCategoryNames.length === 0) {
      setFormError("Debés seleccionar al menos una categoría");
      return;
    }

    setSaving(true);
    setFormError(null);

    const payload: CreatePromotionInput = {
      name: name.trim(),
      kind,
      value: valNum,
      scope,
      min_qty: minQty ? Number(minQty) : null,
      min_subtotal: minSubtotal ? Number(minSubtotal) : null,
      max_discount: maxDiscount ? Number(maxDiscount) : null,
      starts_at: new Date(startsAt).toISOString(),
      ends_at: endsAt ? new Date(endsAt).toISOString() : null,
      priority: Number(priority) || 0,
      usage_limit: usageLimit ? Number(usageLimit) : null,
      per_customer_limit: perCustomerLimit ? Number(perCustomerLimit) : null,
      skip_tier_lines: skipTierLines,
      product_ids: scope === "PRODUCTO" ? selectedProductIds : [],
      category_ids: [],
    };

    try {
      if (editingPromo) {
        await api.promotions.update(editingPromo.id, payload);
        setToast("Promoción actualizada correctamente");
      } else {
        await api.promotions.create(payload);
        setToast("Promoción creada en borrador");
      }
      setDialogOpen(false);
      reload();
      setTimeout(() => setToast(null), 3000);
    } catch (err) {
      setFormError(friendlyError(err));
    } finally {
      setSaving(false);
    }
  }

  async function toggleStatus(promo: Promotion) {
    try {
      if (promo.status === "ACTIVA") {
        await api.promotions.pause(promo.id);
        setToast(`Promoción "${promo.name}" pausada`);
      } else {
        await api.promotions.activate(promo.id);
        setToast(`Promoción "${promo.name}" activada`);
      }
      reload();
      setTimeout(() => setToast(null), 3000);
    } catch (err) {
      setToast(friendlyError(err));
      setTimeout(() => setToast(null), 4000);
    }
  }

  const list = useMemo(() => promotions ?? [], [promotions]);
  const filtered = useMemo(() => {
    if (tab === "TODAS") return list;
    return list.filter((p) => p.effective_status === tab);
  }, [list, tab]);

  const activeCount = list.filter((p) => p.effective_status === "ACTIVA").length;
  const scheduledCount = list.filter((p) => p.effective_status === "PROGRAMADA").length;
  const totalUses = list.reduce((acc, p) => acc + (p.usage_count || 0), 0);

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 flex items-center gap-2 rounded-2xl bg-slate-900 px-4 py-3 text-sm text-white shadow-xl animate-in fade-in slide-in-from-bottom-2">
          <CheckCircle2 className="size-4 text-emerald-400" />
          <span>{toast}</span>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <span className="flex size-9 items-center justify-center rounded-xl bg-teal-50 text-brand">
              <Sparkles className="size-5" />
            </span>
            <h1 className="text-3xl font-bold tracking-tight">Promociones</h1>
          </div>
          <p className="mt-1 text-slate-500">
            Descuentos automáticos por producto, categoría o pedido mínimo en {branch.name}.
          </p>
        </div>
        {canWrite && (
          <button
            onClick={openCreate}
            className="flex h-11 items-center gap-2 rounded-full bg-brand px-5 text-sm font-semibold text-white shadow-md shadow-brand/20 hover:bg-brand-dark transition-colors"
          >
            <Plus className="size-4" /> Nueva promoción
          </button>
        )}
      </div>

      {/* Stats Cards */}
      <div className="grid gap-6 md:grid-cols-3">
        <div className="rounded-3xl border border-slate-200/80 bg-white p-6 shadow-xs">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-500">Promociones activas</p>
            <span className="flex size-9 items-center justify-center rounded-full bg-emerald-50 text-emerald-600">
              <Sparkles className="size-4" />
            </span>
          </div>
          <p className="mt-4 text-3xl font-bold tracking-tight">{activeCount}</p>
          <p className="mt-1 text-xs text-slate-400">Aplicándose ahora mismo en caja</p>
        </div>

        <div className="rounded-3xl border border-slate-200/80 bg-white p-6 shadow-xs">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-500">Usos totales</p>
            <span className="flex size-9 items-center justify-center rounded-full bg-teal-50 text-brand">
              <Users className="size-4" />
            </span>
          </div>
          <p className="mt-4 text-3xl font-bold tracking-tight">{totalUses}</p>
          <p className="mt-1 text-xs text-slate-400">Canjes registrados en ventas</p>
        </div>

        <div className="rounded-3xl border border-slate-200/80 bg-white p-6 shadow-xs">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-500">Programadas</p>
            <span className="flex size-9 items-center justify-center rounded-full bg-blue-50 text-blue-600">
              <Calendar className="size-4" />
            </span>
          </div>
          <p className="mt-4 text-3xl font-bold tracking-tight">{scheduledCount}</p>
          <p className="mt-1 text-xs text-slate-400">Inician en fechas futuras</p>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {STATUS_TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={cn(
              "h-10 rounded-full px-4 text-sm font-medium transition-colors shrink-0",
              tab === t.id
                ? "bg-slate-900 text-white"
                : "bg-white border border-slate-200 text-slate-600 hover:bg-slate-50",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Table */}
      <div className="rounded-3xl border border-slate-200/80 bg-white shadow-xs overflow-hidden">
        {loading && !promotions ? (
          <div className="flex h-64 items-center justify-center text-slate-400">
            <Loader2 className="size-8 animate-spin" />
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center text-slate-400">
            <Sparkles className="size-12 stroke-1 text-slate-300" />
            <p className="mt-3 text-base font-semibold text-slate-700">No hay promociones en esta vista</p>
            <p className="text-sm text-slate-400">Creá promociones para premiar a tus clientes o liquidar inventario.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead className="border-b border-slate-100 bg-slate-50/50 text-xs font-semibold uppercase tracking-wider text-slate-500">
                <tr>
                  <th className="px-6 py-4">Nombre</th>
                  <th className="px-4 py-4">Descuento</th>
                  <th className="px-4 py-4">Alcance</th>
                  <th className="px-4 py-4">Vigencia</th>
                  <th className="px-4 py-4 text-center">Usos</th>
                  <th className="px-4 py-4 text-center">Estado</th>
                  {canWrite && <th className="px-6 py-4 text-right">Acciones</th>}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filtered.map((p) => {
                  const badge = statusBadge(p.effective_status);
                  const discountStr =
                    p.kind === "PORCENTAJE"
                      ? `${Number(p.value)} %`
                      : `${formatCOP(Number(p.value))} c/u`;

                  const canEdit = canWrite && (p.status === "BORRADOR" || p.status === "PAUSADA");
                  const canToggle = canWrite && (p.status === "ACTIVA" || p.status === "PAUSADA" || p.status === "BORRADOR");

                  return (
                    <tr key={p.id} className="hover:bg-slate-50/50 transition-colors">
                      <td className="px-6 py-4">
                        <div className="font-semibold text-slate-900 flex items-center gap-2">
                          {p.name}
                          {p.priority > 0 && (
                            <span className="rounded bg-teal-50 px-1.5 py-0.5 text-[10px] font-bold text-brand">
                              P{p.priority}
                            </span>
                          )}
                        </div>
                        {p.min_subtotal && (
                          <p className="text-xs text-slate-400">Mínimo subtotal: {formatCOP(Number(p.min_subtotal))}</p>
                        )}
                        {p.min_qty && (
                          <p className="text-xs text-slate-400">Lleva mínimo: {Number(p.min_qty)} unidades</p>
                        )}
                      </td>
                      <td className="px-4 py-4 font-semibold text-slate-900">
                        {discountStr}
                        {p.max_discount && (
                          <p className="text-xs font-normal text-slate-400">Tope: {formatCOP(Number(p.max_discount))}</p>
                        )}
                      </td>
                      <td className="px-4 py-4 text-slate-600">
                        <span className="inline-flex items-center gap-1 rounded-lg bg-slate-100 px-2 py-0.5 text-xs font-medium">
                          {p.scope === "PRODUCTO" && <Package className="size-3" />}
                          {p.scope === "CATEGORIA" && <Tag className="size-3" />}
                          {p.scope === "PEDIDO" && <Sparkles className="size-3" />}
                          {p.scope}
                        </span>
                      </td>
                      <td className="px-4 py-4 text-xs text-slate-500">
                        <div>Desde: {new Date(p.starts_at).toLocaleDateString()}</div>
                        <div>Hasta: {p.ends_at ? new Date(p.ends_at).toLocaleDateString() : "Indefinida"}</div>
                      </td>
                      <td className="px-4 py-4 text-center tabular-nums text-slate-600">
                        <span className="font-medium text-slate-900">{p.usage_count}</span>
                        <span className="text-slate-400"> / {p.usage_limit ?? "∞"}</span>
                      </td>
                      <td className="px-4 py-4 text-center">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold",
                            badge.bg,
                          )}
                        >
                          {badge.label}
                        </span>
                      </td>
                      {canWrite && (
                        <td className="px-6 py-4 text-right">
                          <div className="flex items-center justify-end gap-1">
                            {canToggle && (
                              <button
                                onClick={() => toggleStatus(p)}
                                title={p.status === "ACTIVA" ? "Pausar" : "Activar"}
                                className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-500 hover:bg-slate-100"
                              >
                                {p.status === "ACTIVA" ? <Pause className="size-3.5 text-amber-600" /> : <Play className="size-3.5 text-emerald-600" />}
                              </button>
                            )}
                            {canEdit && (
                              <button
                                onClick={() => openEdit(p)}
                                title="Editar promoción"
                                className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-500 hover:bg-slate-100"
                              >
                                <Edit2 className="size-3.5" />
                              </button>
                            )}
                          </div>
                        </td>
                      )}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Dialog Nueva / Editar */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto rounded-3xl p-6">
          <DialogTitle className="text-xl font-bold tracking-tight">
            {editingPromo ? "Editar promoción" : "Nueva promoción"}
          </DialogTitle>
          <p className="text-xs text-slate-500">
            Configurá las condiciones y descuentos que se aplicarán automáticamente en el POS.
          </p>

          <form onSubmit={handleSubmit} className="mt-4 space-y-4">
            {formError && (
              <div className="rounded-xl bg-rose-50 p-3 text-sm text-rose-700 flex items-center gap-2">
                <XCircle className="size-4 shrink-0" />
                <span>{formError}</span>
              </div>
            )}

            {/* Nombre */}
            <div>
              <label className="text-xs font-semibold text-slate-700">Nombre de la promoción</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Ej. Arroz 5%, Lleva 3 gaseosas..."
                className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                maxLength={60}
                required
              />
            </div>

            {/* Alcance y Tipo */}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-semibold text-slate-700">Alcance</label>
                <select
                  value={scope}
                  onChange={(e) => setScope(e.target.value as PromotionScope)}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm outline-none focus:border-brand"
                >
                  <option value="PRODUCTO">Por Producto específico</option>
                  <option value="CATEGORIA">Por Categoría</option>
                  <option value="PEDIDO">Todo el Pedido</option>
                </select>
              </div>
              <div>
                <label className="text-xs font-semibold text-slate-700">Tipo de descuento</label>
                <select
                  value={kind}
                  onChange={(e) => setKind(e.target.value as PromotionKind)}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm outline-none focus:border-brand"
                >
                  <option value="PORCENTAJE">Porcentaje (%)</option>
                  <option value="FIJO">Monto fijo en COP</option>
                </select>
              </div>
            </div>

            {/* Valor y Tope */}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-semibold text-slate-700">
                  {kind === "PORCENTAJE" ? "Porcentaje de descuento (%)" : "Valor de descuento (COP)"}
                </label>
                <input
                  type="number"
                  step="any"
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  placeholder={kind === "PORCENTAJE" ? "10" : "5000"}
                  required
                />
              </div>
              <div>
                <label className="text-xs font-semibold text-slate-700">Descuento máximo (tope en COP)</label>
                <input
                  type="number"
                  value={maxDiscount}
                  onChange={(e) => setMaxDiscount(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  placeholder="Opcional (tope)"
                />
              </div>
            </div>

            {/* Selectores de Productos o Categorías */}
            {scope === "PRODUCTO" && (
              <div className="rounded-2xl border border-slate-200 p-3 bg-slate-50/50">
                <label className="text-xs font-semibold text-slate-700">Productos participantes</label>
                <div className="mt-2 flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs">
                  <Search className="size-3.5 text-slate-400" />
                  <input
                    type="text"
                    value={searchTarget}
                    onChange={(e) => setSearchTarget(e.target.value)}
                    placeholder="Buscar producto..."
                    className="flex-1 bg-transparent outline-none"
                  />
                </div>
                <div className="mt-2 max-h-36 overflow-y-auto space-y-1">
                  {catalog
                    .filter((item) => !searchTarget || item.name.toLowerCase().includes(searchTarget.toLowerCase()))
                    .map((item) => {
                      const selected = selectedProductIds.includes(item.product_id);
                      return (
                        <label
                          key={item.product_id}
                          className="flex items-center gap-2 p-1.5 rounded-lg hover:bg-slate-100 cursor-pointer text-xs"
                        >
                          <input
                            type="checkbox"
                            checked={selected}
                            onChange={(e) => {
                              if (e.target.checked) setSelectedProductIds([...selectedProductIds, item.product_id]);
                              else setSelectedProductIds(selectedProductIds.filter((id) => id !== item.product_id));
                            }}
                            className="rounded border-slate-300 text-brand"
                          />
                          <span className="font-medium text-slate-800">{item.name}</span>
                          <span className="text-slate-400">({formatCOP(Number(item.price))})</span>
                        </label>
                      );
                    })}
                </div>
              </div>
            )}

            {scope === "CATEGORIA" && (
              <div className="rounded-2xl border border-slate-200 p-3 bg-slate-50/50">
                <label className="text-xs font-semibold text-slate-700">Categorías participantes</label>
                <div className="mt-2 max-h-36 overflow-y-auto space-y-1">
                  {categories.map((cat) => {
                    const selected = selectedCategoryNames.includes(cat);
                    return (
                      <label
                        key={cat}
                        className="flex items-center gap-2 p-1.5 rounded-lg hover:bg-slate-100 cursor-pointer text-xs"
                      >
                        <input
                          type="checkbox"
                          checked={selected}
                          onChange={(e) => {
                            if (e.target.checked) setSelectedCategoryNames([...selectedCategoryNames, cat]);
                            else setSelectedCategoryNames(selectedCategoryNames.filter((c) => c !== cat));
                          }}
                          className="rounded border-slate-300 text-brand"
                        />
                        <span className="font-medium text-slate-800">{cat}</span>
                      </label>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Condiciones mínimas */}
            <div className="grid grid-cols-2 gap-3">
              {scope !== "PEDIDO" && (
                <div>
                  <label className="text-xs font-semibold text-slate-700">Cantidad mínima (unidades)</label>
                  <input
                    type="number"
                    step="any"
                    value={minQty}
                    onChange={(e) => setMinQty(e.target.value)}
                    placeholder="Ej. 3 para 3x2"
                    className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  />
                </div>
              )}
              <div>
                <label className="text-xs font-semibold text-slate-700">Subtotal mínimo (COP)</label>
                <input
                  type="number"
                  value={minSubtotal}
                  onChange={(e) => setMinSubtotal(e.target.value)}
                  placeholder="Ej. 100000"
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                />
              </div>
            </div>

            {/* Fechas de vigencia */}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-semibold text-slate-700">Fecha y hora de inicio</label>
                <input
                  type="datetime-local"
                  value={startsAt}
                  onChange={(e) => setStartsAt(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  required
                />
              </div>
              <div>
                <label className="text-xs font-semibold text-slate-700">Fecha y hora de fin (opcional)</label>
                <input
                  type="datetime-local"
                  value={endsAt}
                  onChange={(e) => setEndsAt(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                />
              </div>
            </div>

            {/* Límites y prioridad */}
            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="text-xs font-semibold text-slate-700">Prioridad</label>
                <input
                  type="number"
                  value={priority}
                  onChange={(e) => setPriority(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  placeholder="0"
                />
              </div>
              <div>
                <label className="text-xs font-semibold text-slate-700">Límite total de usos</label>
                <input
                  type="number"
                  value={usageLimit}
                  onChange={(e) => setUsageLimit(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  placeholder="Ilimitado"
                />
              </div>
              <div>
                <label className="text-xs font-semibold text-slate-700">Límite por cliente</label>
                <input
                  type="number"
                  value={perCustomerLimit}
                  onChange={(e) => setPerCustomerLimit(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm outline-none focus:border-brand"
                  placeholder="Ilimitado"
                />
              </div>
            </div>

            {/* Opciones adicionales */}
            <div className="pt-2">
              <label className="flex items-center gap-2 cursor-pointer text-xs font-medium text-slate-700">
                <input
                  type="checkbox"
                  checked={skipTierLines}
                  onChange={(e) => setSkipTierLines(e.target.checked)}
                  className="rounded border-slate-300 text-brand"
                />
                Excluir líneas con precio por tarifa o presentación especial
              </label>
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-2 pt-4 border-t border-slate-100">
              <button
                type="button"
                onClick={() => setDialogOpen(false)}
                className="h-10 rounded-full border border-slate-200 px-4 text-xs font-medium text-slate-600 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={saving}
                className="flex h-10 items-center gap-1.5 rounded-full bg-brand px-5 text-xs font-semibold text-white shadow-sm hover:bg-brand-dark disabled:opacity-50"
              >
                {saving && <Loader2 className="size-3.5 animate-spin" />}
                {editingPromo ? "Guardar cambios" : "Crear promoción"}
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}

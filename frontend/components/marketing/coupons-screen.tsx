"use client";

import { useEffect, useMemo, useState } from "react";
import {
  Calendar,
  Edit2,
  Loader2,
  Package,
  Pause,
  Play,
  Plus,
  Search,
  Tag,
  Users,
  XCircle,
} from "lucide-react";

import {
  api,
  friendlyError,
  type CatalogItem,
  type Coupon,
  type CouponKind,
  type CouponScope,
  type CouponStatus,
  type CreateCouponInput,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

const STATUS_TABS: { id: string; label: string }[] = [
  { id: "TODAS", label: "Todos" },
  { id: "ACTIVO", label: "Activos" },
  { id: "INACTIVO", label: "Inactivos" },
];

function statusBadge(st: CouponStatus) {
  if (st === "ACTIVO") {
    return { label: "Activo", bg: "bg-emerald-50 text-emerald-700 border-emerald-200" };
  }
  return { label: "Inactivo", bg: "bg-slate-100 text-slate-600 border-slate-200" };
}

export function CouponsScreen() {
  const branch = useCurrentBranch();
  const can = useSession((s) => s.can);
  const canWrite = can("settings:write");

  const [tab, setTab] = useState("TODAS");
  const [toast, setToast] = useState<string | null>(null);

  const { data: coupons, loading, reload } = useApi(
    () => api.coupons.list(),
    "coupons",
  );

  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  useEffect(() => {
    api.catalog.list(branch.id).then(setCatalog).catch(() => {});
  }, [branch.id]);

  // Dialog State
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingCoupon, setEditingCoupon] = useState<Coupon | null>(null);

  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState<CouponKind>("PORCENTAJE");
  const [value, setValue] = useState("10");
  const [scope, setScope] = useState<CouponScope>("PEDIDO");
  const [minQty, setMinQty] = useState("");
  const [minSubtotal, setMinSubtotal] = useState("");
  const [maxDiscount, setMaxDiscount] = useState("");
  const [startsAt, setStartsAt] = useState(() => new Date().toISOString().slice(0, 16));
  const [endsAt, setEndsAt] = useState("");
  const [usageLimit, setUsageLimit] = useState("");
  const [perCustomerLimit, setPerCustomerLimit] = useState("");

  const [selectedProductIds, setSelectedProductIds] = useState<string[]>([]);
  const [searchTarget, setSearchTarget] = useState("");

  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  function openCreate() {
    setEditingCoupon(null);
    setCode("");
    setName("");
    setKind("PORCENTAJE");
    setValue("10");
    setScope("PEDIDO");
    setMinQty("");
    setMinSubtotal("");
    setMaxDiscount("");
    setStartsAt(new Date().toISOString().slice(0, 16));
    setEndsAt("");
    setUsageLimit("");
    setPerCustomerLimit("");
    setSelectedProductIds([]);
    setSearchTarget("");
    setFormError(null);
    setDialogOpen(true);
  }

  function openEdit(coupon: Coupon) {
    setEditingCoupon(coupon);
    setCode(coupon.code);
    setName(coupon.name);
    setKind(coupon.kind);
    setValue(String(Number(coupon.value)));
    setScope(coupon.scope);
    setMinQty(coupon.min_qty ? String(Number(coupon.min_qty)) : "");
    setMinSubtotal(coupon.min_subtotal ? String(Number(coupon.min_subtotal)) : "");
    setMaxDiscount(coupon.max_discount ? String(Number(coupon.max_discount)) : "");
    setStartsAt(new Date(coupon.starts_at).toISOString().slice(0, 16));
    setEndsAt(coupon.ends_at ? new Date(coupon.ends_at).toISOString().slice(0, 16) : "");
    setUsageLimit(coupon.usage_limit ? String(coupon.usage_limit) : "");
    setPerCustomerLimit(coupon.per_customer_limit ? String(coupon.per_customer_limit) : "");
    setSelectedProductIds(coupon.product_ids || []);
    setSearchTarget("");
    setFormError(null);
    setDialogOpen(true);
  }

  async function handleToggle(id: string) {
    try {
      await api.coupons.toggle(id);
      setToast("Estado del cupón actualizado");
      reload();
    } catch (err) {
      setToast(friendlyError(err));
    }
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setFormError(null);

    const normCode = code.trim().toUpperCase();
    if (!/^[A-Z0-9]{4,20}$/.test(normCode)) {
      setFormError("El código debe tener entre 4 y 20 caracteres alfanuméricos (solo mayúsculas y números).");
      return;
    }

    if (!name.trim()) {
      setFormError("Ingresá un nombre descriptivo para el cupón.");
      return;
    }

    const numVal = parseFloat(value);
    if (isNaN(numVal) || numVal <= 0) {
      setFormError("El valor de descuento debe ser mayor que 0.");
      return;
    }
    if (kind === "PORCENTAJE" && numVal > 100) {
      setFormError("El porcentaje no puede superar el 100 %.");
      return;
    }

    if (endsAt && new Date(endsAt) <= new Date(startsAt)) {
      setFormError("La fecha de fin debe ser posterior a la fecha de inicio.");
      return;
    }

    if (scope === "PRODUCTO" && selectedProductIds.length === 0) {
      setFormError("Seleccioná al menos un producto.");
      return;
    }

    setSaving(true);
    try {
      const input: CreateCouponInput = {
        code: normCode,
        name: name.trim(),
        kind,
        value: numVal,
        scope,
        min_qty: minQty ? parseFloat(minQty) : null,
        min_subtotal: minSubtotal ? parseFloat(minSubtotal) : null,
        max_discount: maxDiscount ? parseFloat(maxDiscount) : null,
        starts_at: new Date(startsAt).toISOString(),
        ends_at: endsAt ? new Date(endsAt).toISOString() : null,
        usage_limit: usageLimit ? parseInt(usageLimit, 10) : null,
        per_customer_limit: perCustomerLimit ? parseInt(perCustomerLimit, 10) : null,
        product_ids: scope === "PRODUCTO" ? selectedProductIds : undefined,
        category_ids: undefined,
      };

      if (editingCoupon) {
        await api.coupons.update(editingCoupon.id, input);
        setToast("Cupón actualizado con éxito");
      } else {
        await api.coupons.create(input);
        setToast("Cupón creado con éxito");
      }
      setDialogOpen(false);
      reload();
    } catch (err) {
      setFormError(friendlyError(err));
    } finally {
      setSaving(false);
    }
  }

  const filteredCoupons = useMemo(() => {
    if (!coupons) return [];
    if (tab === "TODAS") return coupons;
    return coupons.filter((c) => c.status === tab);
  }, [coupons, tab]);

  return (
    <div className="flex-1 p-6 max-w-7xl mx-auto space-y-6">
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 bg-slate-900 text-white px-4 py-3 rounded-2xl shadow-xl flex items-center gap-3 border border-slate-700 animate-in fade-in">
          <span>{toast}</span>
          <button
            onClick={() => setToast(null)}
            className="text-slate-400 hover:text-white text-xs font-semibold"
          >
            ✕
          </button>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-slate-900 flex items-center gap-3">
            <Tag className="size-8 text-brand" />
            Cupones
          </h1>
          <p className="text-sm text-slate-500 mt-1">
            Administrá códigos promocionales aplicables en el carrito del POS
          </p>
        </div>

        {canWrite && (
          <button
            onClick={openCreate}
            className="inline-flex items-center gap-2 rounded-2xl bg-brand px-5 py-3 text-sm font-semibold text-white shadow-lg shadow-teal-900/20 hover:bg-brand-dark transition-colors"
          >
            <Plus className="size-4" />
            Nuevo cupón
          </button>
        )}
      </div>

      {/* Tabs */}
      <div className="flex border-b border-slate-200 gap-2">
        {STATUS_TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={cn(
              "px-4 py-2.5 text-sm font-medium border-b-2 -mb-px transition-colors",
              tab === t.id
                ? "border-brand text-brand font-bold"
                : "border-transparent text-slate-500 hover:text-slate-800",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Table Container */}
      <div className="bg-white rounded-3xl border border-slate-200 shadow-xs overflow-hidden">
        {loading ? (
          <div className="p-12 text-center text-slate-400 flex items-center justify-center gap-3">
            <Loader2 className="size-5 animate-spin text-brand" />
            Cargando cupones...
          </div>
        ) : filteredCoupons.length === 0 ? (
          <div className="p-12 text-center text-slate-400">
            <Tag className="size-10 mx-auto text-slate-300 mb-3" />
            <p className="font-semibold text-slate-700">No hay cupones en este estado</p>
            <p className="text-xs text-slate-400 mt-1">Creá un cupón o cambiá el filtro para ver más</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse text-sm">
              <thead>
                <tr className="border-b border-slate-100 bg-slate-50/50 text-xs font-semibold text-slate-500 uppercase tracking-wider">
                  <th className="py-3.5 px-4">Código</th>
                  <th className="py-3.5 px-4">Nombre</th>
                  <th className="py-3.5 px-4">Descuento</th>
                  <th className="py-3.5 px-4">Alcance</th>
                  <th className="py-3.5 px-4">Vigencia</th>
                  <th className="py-3.5 px-4">Usos</th>
                  <th className="py-3.5 px-4">Estado</th>
                  <th className="py-3.5 px-4 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 text-slate-700">
                {filteredCoupons.map((c) => {
                  const badge = statusBadge(c.status);
                  const isPercent = c.kind === "PORCENTAJE";
                  const discountDisplay = isPercent
                    ? `${Number(c.value)} %`
                    : formatCOP(Number(c.value));

                  return (
                    <tr key={c.id} className="hover:bg-slate-50/60 transition-colors">
                      <td className="py-4 px-4">
                        <span className="font-mono font-bold tracking-wider text-brand bg-teal-50 px-2 py-0.5 rounded-lg border border-teal-100">
                          {c.code}
                        </span>
                      </td>
                      <td className="py-4 px-4 font-semibold text-slate-900">{c.name}</td>
                      <td className="py-4 px-4">
                        <span className="font-bold text-slate-900">{discountDisplay}</span>
                        {c.max_discount && (
                          <span className="block text-xs text-slate-400">
                            Tope: {formatCOP(Number(c.max_discount))}
                          </span>
                        )}
                      </td>
                      <td className="py-4 px-4">
                        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-slate-100 text-slate-700 border border-slate-200">
                          {c.scope === "PEDIDO" && <Package className="size-3" />}
                          {c.scope === "PRODUCTO" && <Tag className="size-3" />}
                          {c.scope === "CATEGORIA" && <Users className="size-3" />}
                          {c.scope === "PEDIDO" ? "Pedido" : c.scope === "PRODUCTO" ? "Producto" : "Categoría"}
                        </span>
                      </td>
                      <td className="py-4 px-4 text-xs">
                        <div className="flex items-center gap-1 text-slate-600">
                          <Calendar className="size-3" />
                          <span>{new Date(c.starts_at).toLocaleDateString()}</span>
                        </div>
                        {c.ends_at ? (
                          <span className="text-slate-400 block mt-0.5">
                            Hasta: {new Date(c.ends_at).toLocaleDateString()}
                          </span>
                        ) : (
                          <span className="text-slate-400 block mt-0.5">Sin vencimiento</span>
                        )}
                      </td>
                      <td className="py-4 px-4">
                        <span className="font-mono text-xs font-semibold text-slate-800">
                          {c.usage_count}
                          {c.usage_limit != null ? ` / ${c.usage_limit}` : " / ∞"}
                        </span>
                        {c.per_customer_limit && (
                          <span className="block text-xs text-slate-400">
                            Max {c.per_customer_limit} por cliente
                          </span>
                        )}
                      </td>
                      <td className="py-4 px-4">
                        <span
                          className={cn(
                            "inline-flex items-center px-2.5 py-1 rounded-full text-xs font-semibold border",
                            badge.bg,
                          )}
                        >
                          {badge.label}
                        </span>
                      </td>
                      <td className="py-4 px-4 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          {canWrite && (
                            <>
                              <button
                                onClick={() => handleToggle(c.id)}
                                title={c.status === "ACTIVO" ? "Desactivar" : "Activar"}
                                aria-label={c.status === "ACTIVO" ? "Desactivar cupón" : "Activar cupón"}
                                className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-600 hover:bg-slate-100 hover:text-slate-900 transition-colors"
                              >
                                {c.status === "ACTIVO" ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
                              </button>
                              <button
                                onClick={() => openEdit(c)}
                                title="Editar"
                                aria-label="Editar cupón"
                                className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-600 hover:bg-slate-100 hover:text-slate-900 transition-colors"
                              >
                                <Edit2 className="size-3.5" />
                              </button>
                            </>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Dialog for Create/Edit */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-xl max-h-[90vh] overflow-y-auto rounded-3xl p-6 bg-white">
          <DialogTitle className="text-xl font-bold text-slate-900 flex items-center gap-2">
            <Tag className="size-5 text-brand" />
            {editingCoupon ? "Editar cupón" : "Nuevo cupón"}
          </DialogTitle>

          {formError && (
            <div className="mt-3 p-3 rounded-2xl bg-rose-50 border border-rose-200 text-rose-700 text-xs flex items-center gap-2">
              <XCircle className="size-4 shrink-0" />
              <span>{formError}</span>
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4 mt-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Código del cupón *</label>
                <input
                  type="text"
                  required
                  value={code}
                  onChange={(e) => setCode(e.target.value.toUpperCase())}
                  placeholder="EJ: BIENVENIDA10"
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm font-mono uppercase focus:border-brand focus:ring-1 focus:ring-brand"
                />
                <span className="text-[11px] text-slate-400 mt-0.5 block">4 a 20 caracteres (mayúsculas y números)</span>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Nombre descriptivo *</label>
                <input
                  type="text"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="EJ: Descuento primer pedido"
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>
            </div>

            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Tipo de descuento</label>
                <select
                  value={kind}
                  onChange={(e) => setKind(e.target.value as CouponKind)}
                  className="w-full rounded-2xl border border-slate-200 px-3 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                >
                  <option value="PORCENTAJE">Porcentaje (%)</option>
                  <option value="FIJO">Monto fijo ($)</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">
                  Valor {kind === "PORCENTAJE" ? "(%)" : "($)"} *
                </label>
                <input
                  type="number"
                  step="any"
                  required
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Alcance</label>
                <select
                  value={scope}
                  onChange={(e) => setScope(e.target.value as CouponScope)}
                  className="w-full rounded-2xl border border-slate-200 px-3 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                >
                  <option value="PEDIDO">Todo el pedido</option>
                  <option value="PRODUCTO">Productos específicos</option>
                </select>
              </div>
            </div>

            {/* Target Selectors */}
            {scope === "PRODUCTO" && (
              <div className="border border-slate-200 rounded-2xl p-3 bg-slate-50/50 space-y-2">
                <label className="block text-xs font-semibold text-slate-700">Productos objetivo</label>
                <div className="relative">
                  <Search className="size-4 absolute left-3 top-2.5 text-slate-400" />
                  <input
                    type="text"
                    placeholder="Buscar producto por nombre..."
                    value={searchTarget}
                    onChange={(e) => setSearchTarget(e.target.value)}
                    className="w-full rounded-xl border border-slate-200 pl-9 pr-3 py-1.5 text-xs bg-white"
                  />
                </div>
                <div className="max-h-36 overflow-y-auto space-y-1 divide-y divide-slate-100">
                  {catalog
                    .filter((p) => p.name.toLowerCase().includes(searchTarget.toLowerCase()))
                    .map((p) => {
                      const isSelected = selectedProductIds.includes(p.product_id);
                      return (
                        <label
                          key={p.product_id}
                          className="flex items-center gap-2 py-1.5 px-2 hover:bg-slate-100 rounded-lg text-xs cursor-pointer"
                        >
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={(e) => {
                              if (e.target.checked) {
                                setSelectedProductIds([...selectedProductIds, p.product_id]);
                              } else {
                                setSelectedProductIds(selectedProductIds.filter((id) => id !== p.product_id));
                              }
                            }}
                            className="rounded border-slate-300 text-brand focus:ring-brand"
                          />
                          <span className="font-medium text-slate-800">{p.name}</span>
                          <span className="text-slate-400 ml-auto">{formatCOP(Number(p.price))}</span>
                        </label>
                      );
                    })}
                </div>
              </div>
            )}

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Mínimo de compra ($)</label>
                <input
                  type="number"
                  placeholder="Opcional"
                  value={minSubtotal}
                  onChange={(e) => setMinSubtotal(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Tope máx. de descuento ($)</label>
                <input
                  type="number"
                  placeholder="Opcional"
                  value={maxDiscount}
                  onChange={(e) => setMaxDiscount(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Inicia el *</label>
                <input
                  type="datetime-local"
                  required
                  value={startsAt}
                  onChange={(e) => setStartsAt(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Vence el</label>
                <input
                  type="datetime-local"
                  value={endsAt}
                  onChange={(e) => setEndsAt(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Límite de usos totales</label>
                <input
                  type="number"
                  placeholder="Ilimitado si se omite"
                  value={usageLimit}
                  onChange={(e) => setUsageLimit(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 mb-1">Usos por cliente</label>
                <input
                  type="number"
                  placeholder="Ilimitado si se omite"
                  value={perCustomerLimit}
                  onChange={(e) => setPerCustomerLimit(e.target.value)}
                  className="w-full rounded-2xl border border-slate-200 px-3.5 py-2 text-sm focus:border-brand focus:ring-1 focus:ring-brand"
                />
              </div>
            </div>

            <div className="pt-4 border-t border-slate-100 flex items-center justify-end gap-3">
              <button
                type="button"
                onClick={() => setDialogOpen(false)}
                className="px-4 py-2.5 rounded-2xl border border-slate-200 text-sm font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={saving}
                className="inline-flex items-center gap-2 px-6 py-2.5 rounded-2xl bg-brand text-sm font-semibold text-white shadow-md shadow-teal-900/10 hover:bg-brand-dark transition-colors disabled:opacity-50"
              >
                {saving && <Loader2 className="size-4 animate-spin" />}
                {editingCoupon ? "Guardar cambios" : "Crear cupón"}
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}

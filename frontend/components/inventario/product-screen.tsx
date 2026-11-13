"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  AlertTriangle,
  Archive,
  ArrowLeft,
  CheckCircle2,
  History,
  Layers,
  Package,
  Plus,
  RefreshCw,
  Save,
  Sliders,
  Tag,
  Warehouse,
  X,
} from "lucide-react";

import {
  api,
  ApiError,
  type ArchiveNeedsConfirmData,
  type OptionGroupDetail,
  type ProductDetail,
  type ProductInput,
  type ProductKind,
  type ProductUsage,
} from "@/lib/api";
import { useCurrentBranch } from "@/lib/stores/session";
import { formatCOP, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

type TabId = "general" | "precios" | "bodegas" | "variantes" | "modificadores";

export function ProductScreen({ productId }: { productId: string }) {
  const isNew = productId === "nuevo";
  const branch = useCurrentBranch();
  const router = useRouter();

  const [activeTab, setActiveTab] = useState<TabId>("general");
  const [loading, setLoading] = useState(!isNew);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Form State
  const [name, setName] = useState("");
  const [sku, setSku] = useState("");
  const [barcode, setBarcode] = useState("");
  const [kind, setKind] = useState<ProductKind>("FISICO");
  const [usage, setUsage] = useState<ProductUsage>("VENTA");
  const [unitMeasureCode, setUnitMeasureCode] = useState("UND");
  const [purchaseUomCode, setPurchaseUomCode] = useState("");
  const [purchaseFactor, setPurchaseFactor] = useState("1.0000");
  const [taxCode, setTaxCode] = useState("IVA19");
  const [basePrice, setBasePrice] = useState("");
  const [costPrice, setCostPrice] = useState("");
  const [reorderQty, setReorderQty] = useState("");
  const [maxQty, setMaxQty] = useState("");
  const [trackLots, setTrackLots] = useState(false);
  const [allowPriceOverride, setAllowPriceOverride] = useState(true);
  const [hasVariants, setHasVariants] = useState(false);
  const [description, setDescription] = useState("");

  // Branch Custom Price State
  const [customBranchPriceEnabled, setCustomBranchPriceEnabled] = useState(false);
  const [branchPrice, setBranchPrice] = useState("");

  // Variant Form State
  const [showVariantForm, setShowVariantForm] = useState(false);
  const [variantSku, setVariantSku] = useState("");
  const [variantBarcode, setVariantBarcode] = useState("");
  const [variantPrice, setVariantPrice] = useState("");
  const [variantCost, setVariantCost] = useState("");
  const [variantAttrs, setVariantAttrs] = useState<Array<{ key: string; value: string }>>([
    { key: "Talla", value: "" },
  ]);

  // Full product detail if editing
  const [detail, setDetail] = useState<ProductDetail | null>(null);

  // Modificadores / Option Groups State
  const [allOptionGroups, setAllOptionGroups] = useState<OptionGroupDetail[]>([]);
  const [assignedOptionGroups, setAssignedOptionGroups] = useState<
    Array<{ group_id: string; sort_order: number }>
  >([]);
  const [savingModifiers, setSavingModifiers] = useState(false);

  // Archive State
  const [archivePlan, setArchivePlan] = useState<ArchiveNeedsConfirmData | null>(null);
  const [archiving, setArchiving] = useState(false);

  async function handleArchive(confirm: boolean) {
    if (isNew) return;
    setArchiving(true);
    setErrorMsg(null);
    try {
      await api.products.archive(productId, confirm);
      setArchivePlan(null);
      setToast("Producto archivado exitosamente");
      const refreshed = await api.products.get(productId, branch.id);
      setDetail(refreshed);
      setTimeout(() => setToast(null), 3000);
    } catch (err: unknown) {
      if (err instanceof ApiError && err.code === "ARCHIVE_NEEDS_CONFIRM" && err.data) {
        setArchivePlan(err.data as ArchiveNeedsConfirmData);
      } else {
        const msg = err instanceof Error ? err.message : "Error al archivar producto";
        setErrorMsg(msg);
      }
    } finally {
      setArchiving(false);
    }
  }

  async function handleUnarchive() {
    if (isNew) return;
    setArchiving(true);
    setErrorMsg(null);
    try {
      await api.products.unarchive(productId);
      setToast("Producto reactivado exitosamente");
      const refreshed = await api.products.get(productId, branch.id);
      setDetail(refreshed);
      setTimeout(() => setToast(null), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al reactivar producto";
      setErrorMsg(msg);
    } finally {
      setArchiving(false);
    }
  }

  useEffect(() => {
    if (isNew) return;
    let mounted = true;
    Promise.all([api.products.get(productId, branch.id), api.options.listGroups()])
      .then(([data, groups]) => {
        if (!mounted) return;
        setDetail(data);
        setAllOptionGroups(groups);
        setAssignedOptionGroups(
          (data.option_groups ?? []).map((g, idx) => ({
            group_id: g.id,
            sort_order: idx + 1,
          }))
        );
        setName(data.name);
        setSku(data.sku ?? "");
        setBarcode(data.barcode ?? "");
        setKind(data.kind);
        setUsage(data.usage);
        setUnitMeasureCode(data.unit_measure_code);
        setPurchaseUomCode(data.purchase_uom_code ?? "");
        setPurchaseFactor(data.purchase_factor);
        setTaxCode(data.tax_code);
        setBasePrice(data.price ?? "");
        setCostPrice(data.cost_price ?? "");
        setReorderQty(data.reorder_qty ?? "");
        setMaxQty(data.max_qty ?? "");
        setTrackLots(data.track_lots);
        setAllowPriceOverride(data.allow_price_override);
        setHasVariants(data.has_variants);
        setDescription(data.description ?? "");

        if (data.price) {
          setBranchPrice(data.price);
        }
      })
      .catch((err) => {
        if (!mounted) return;
        setErrorMsg(err.message || "Error al cargar el producto");
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });

    return () => {
      mounted = false;
    };
  }, [productId, isNew, branch.id]);

  async function handleSaveGeneral(e: React.FormEvent) {
    e.preventDefault();
    setErrorMsg(null);
    setSaving(true);

    const input: ProductInput = {
      name: name.trim(),
      sku: sku.trim() || null,
      barcode: barcode.trim() || null,
      kind,
      usage,
      unit_measure_code: unitMeasureCode,
      purchase_uom_code: purchaseUomCode.trim() || null,
      purchase_factor: purchaseFactor || "1.0000",
      tax_code: taxCode,
      base_price: basePrice ? basePrice : null,
      cost_price: costPrice ? costPrice : null,
      reorder_qty: reorderQty ? reorderQty : null,
      max_qty: maxQty ? maxQty : null,
      track_lots: trackLots,
      allow_price_override: allowPriceOverride,
      has_variants: hasVariants,
      description: description.trim() || null,
    };

    try {
      if (isNew) {
        const created = await api.products.create(input);
        setToast("Producto creado exitosamente");
        router.push(`/inventario/producto/${created.id}`);
      } else {
        const updated = await api.products.update(productId, input);
        setDetail(updated);
        setToast("Producto actualizado correctamente");
        setTimeout(() => setToast(null), 3000);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al guardar producto";
      setErrorMsg(msg);
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveBranchPrice() {
    if (!detail) return;
    setErrorMsg(null);
    try {
      await api.products.updateBranchProduct(detail.id, {
        branch_id: branch.id,
        price: branchPrice,
        is_available: true,
      });
      setToast("Precio de sucursal actualizado");
      setTimeout(() => setToast(null), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al actualizar precio";
      setErrorMsg(msg);
    }
  }

  async function handleCreateVariant(e: React.FormEvent) {
    e.preventDefault();
    if (!detail) return;
    setErrorMsg(null);

    const attrsMap: Record<string, string> = {};
    for (const attr of variantAttrs) {
      if (attr.key.trim() && attr.value.trim()) {
        attrsMap[attr.key.trim()] = attr.value.trim();
      }
    }

    if (Object.keys(attrsMap).length === 0) {
      setErrorMsg("Debes especificar al menos un atributo (ej. Talla: M)");
      return;
    }

    try {
      const updated = await api.products.createVariant(detail.id, {
        sku: variantSku.trim() || null,
        barcode: variantBarcode.trim() || null,
        attrs: attrsMap,
        price: variantPrice || detail.price || "0.00",
        cost_price: variantCost ? variantCost : null,
      });
      setDetail(updated);
      setShowVariantForm(false);
      setVariantSku("");
      setVariantBarcode("");
      setVariantPrice("");
      setVariantCost("");
      setVariantAttrs([{ key: "Talla", value: "" }]);
      setToast("Variante creada exitosamente");
      setTimeout(() => setToast(null), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al crear variante";
      setErrorMsg(msg);
    }
  }

  if (loading) {
    return (
      <div className="space-y-6 p-6 lg:p-10 animate-pulse">
        <div className="h-8 w-48 rounded bg-slate-200" />
        <div className="h-64 rounded-2xl bg-slate-200/60" />
      </div>
    );
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
            <h1 className="text-2xl font-bold tracking-tight">
              {isNew ? "Nuevo producto" : name || "Ficha de producto"}
            </h1>
            <p className="text-sm text-slate-500">
              {isNew
                ? "Completa la información básica y precios del catálogo"
                : `Código: ${sku || barcode || "Sin código"} • Sucursal: ${branch.name}`}
            </p>
          </div>
        </div>

        {!isNew && (
          <div className="flex items-center gap-3">
            <Link
              href={`/inventario/movimientos?producto=${productId}`}
              className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium hover:bg-slate-50 text-slate-700"
            >
              <History className="size-4" /> Kardex de producto
            </Link>
            {!detail?.archived && (
              <button
                type="button"
                onClick={() => handleArchive(false)}
                disabled={archiving}
                className="flex h-10 items-center gap-2 rounded-full border border-rose-200 bg-white px-4 text-sm font-medium text-rose-700 hover:bg-rose-50 disabled:opacity-50"
              >
                <Archive className="size-4" /> Archivar
              </button>
            )}
          </div>
        )}
      </div>

      {detail?.archived && (
        <div className="flex items-center justify-between gap-4 rounded-2xl border border-amber-300 bg-amber-50 p-4 text-amber-900 shadow-sm">
          <div className="flex items-center gap-3">
            <AlertTriangle className="size-5 text-amber-600 shrink-0" />
            <div>
              <p className="font-semibold text-sm">
                Producto archivado{detail.archived_at ? ` el ${new Date(detail.archived_at).toLocaleDateString("es-CO", { day: "numeric", month: "long", year: "numeric" })}` : ""}
              </p>
              <p className="text-xs text-amber-700">
                Este producto no está disponible para ventas ni movimientos. Sus existencias fueron dadas de baja al archivar.
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={handleUnarchive}
            disabled={archiving}
            className="flex h-9 items-center gap-2 rounded-full bg-amber-600 px-4 text-xs font-semibold text-white shadow hover:bg-amber-700 disabled:opacity-50"
          >
            <RefreshCw className={cn("size-3.5", archiving && "animate-spin")} />
            Desarchivar
          </button>
        </div>
      )}

      {archivePlan && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm">
          <div className="w-full max-w-lg rounded-3xl bg-white p-6 shadow-2xl space-y-5 animate-in fade-in-0 zoom-in-95">
            <div className="flex items-center justify-between border-b border-slate-100 pb-3">
              <h3 className="text-lg font-bold text-slate-900 flex items-center gap-2">
                <AlertTriangle className="size-5 text-amber-500" /> Confirmar baja para archivar
              </h3>
              <button
                onClick={() => setArchivePlan(null)}
                className="rounded-full p-1 text-slate-400 hover:bg-slate-100"
              >
                <X className="size-5" />
              </button>
            </div>

            <div className="rounded-2xl border border-amber-200 bg-amber-50/70 p-4">
              <p className="text-sm text-slate-800 leading-relaxed">
                Se registrará una baja de inventario por{" "}
                <strong className="text-rose-700 font-bold font-mono">
                  {formatCOP(Number(archivePlan.total_value))}
                </strong>{" "}
                en <strong>{archivePlan.plan.length}</strong> {archivePlan.plan.length === 1 ? "ubicación" : "ubicaciones"} para archivar el producto. Esta acción no se puede deshacer (las existencias no se recuperan al desarchivar). ¿Deseas continuar?
              </p>
            </div>

            <div className="max-h-60 overflow-y-auto rounded-xl border border-slate-200">
              <table className="w-full text-left text-xs">
                <thead className="bg-slate-50 text-slate-600 border-b border-slate-200">
                  <tr>
                    <th className="p-2.5 font-semibold">Ubicación</th>
                    <th className="p-2.5 font-semibold text-right">Existencias</th>
                    <th className="p-2.5 font-semibold text-right">Costo Promedio</th>
                    <th className="p-2.5 font-semibold text-right">Valor Total</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {archivePlan.plan.map((item) => (
                    <tr key={item.location_id}>
                      <td className="p-2.5 font-medium text-slate-800">{item.location_name}</td>
                      <td className="p-2.5 text-right font-mono">{formatQty(item.on_hand)}</td>
                      <td className="p-2.5 text-right font-mono">{formatCOP(Number(item.avg_cost))}</td>
                      <td className="p-2.5 text-right font-mono font-semibold text-slate-900">
                        {formatCOP(Number(item.value))}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <div className="flex justify-end gap-3 pt-2">
              <button
                type="button"
                onClick={() => setArchivePlan(null)}
                disabled={archiving}
                className="rounded-full border border-slate-200 px-5 py-2.5 text-sm font-medium hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={() => handleArchive(true)}
                disabled={archiving}
                className="rounded-full bg-rose-600 px-6 py-2.5 text-sm font-semibold text-white shadow hover:bg-rose-700 disabled:opacity-50 flex items-center gap-2"
              >
                {archiving ? "Procesando..." : "Confirmar baja y archivar"}
              </button>
            </div>
          </div>
        </div>
      )}

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

      {/* Navegación por pestañas */}
      {!isNew && (
        <div className="flex border-b border-slate-200 gap-6 text-sm font-medium">
          <button
            type="button"
            onClick={() => setActiveTab("general")}
            className={cn(
              "pb-3 border-b-2 transition-colors flex items-center gap-2",
              activeTab === "general"
                ? "border-emerald-600 text-emerald-600 font-semibold"
                : "border-transparent text-slate-500 hover:text-slate-800"
            )}
          >
            <Package className="size-4" /> General
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("precios")}
            className={cn(
              "pb-3 border-b-2 transition-colors flex items-center gap-2",
              activeTab === "precios"
                ? "border-emerald-600 text-emerald-600 font-semibold"
                : "border-transparent text-slate-500 hover:text-slate-800"
            )}
          >
            <Tag className="size-4" /> Precios y presentaciones
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("bodegas")}
            className={cn(
              "pb-3 border-b-2 transition-colors flex items-center gap-2",
              activeTab === "bodegas"
                ? "border-emerald-600 text-emerald-600 font-semibold"
                : "border-transparent text-slate-500 hover:text-slate-800"
            )}
          >
            <Warehouse className="size-4" /> Stock por bodega
          </button>
          {hasVariants && (
            <button
              type="button"
              onClick={() => setActiveTab("variantes")}
              className={cn(
                "pb-3 border-b-2 transition-colors flex items-center gap-2",
                activeTab === "variantes"
                  ? "border-emerald-600 text-emerald-600 font-semibold"
                  : "border-transparent text-slate-500 hover:text-slate-800"
              )}
            >
              <Layers className="size-4" /> Variantes ({detail?.variants.length ?? 0})
            </button>
          )}
          <button
            type="button"
            onClick={() => setActiveTab("modificadores")}
            className={cn(
              "pb-3 border-b-2 transition-colors flex items-center gap-2",
              activeTab === "modificadores"
                ? "border-emerald-600 text-emerald-600 font-semibold"
                : "border-transparent text-slate-500 hover:text-slate-800"
            )}
          >
            <Sliders className="size-4" /> Modificadores ({assignedOptionGroups.length})
          </button>
        </div>
      )}

      {/* Tab: General */}
      {activeTab === "general" && (
        <form onSubmit={handleSaveGeneral} className="space-y-6">
          <fieldset disabled={Boolean(detail?.archived)} className="space-y-6">
            <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm space-y-6">
            <h2 className="text-base font-semibold text-slate-900 border-b border-slate-100 pb-3">
              Información básica
            </h2>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="md:col-span-2">
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Nombre del producto *
                </label>
                <input
                  type="text"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Ej: Café Especial Molido 500g"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  SKU (Código interno)
                </label>
                <input
                  type="text"
                  value={sku}
                  onChange={(e) => setSku(e.target.value)}
                  placeholder="Ej: CAF-ESP-500"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Código de barras
                </label>
                <input
                  type="text"
                  value={barcode}
                  onChange={(e) => setBarcode(e.target.value)}
                  placeholder="Ej: 7701234567890"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Tipo de producto
                </label>
                <select
                  value={kind}
                  onChange={(e) => setKind(e.target.value as ProductKind)}
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none bg-white"
                >
                  <option value="FISICO">Físico (lleva inventario)</option>
                  <option value="SERVICIO">Servicio (no mueve inventario)</option>
                  <option value="PREPARADO">Preparado (compuesto por receta)</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Uso en el sistema
                </label>
                <select
                  value={usage}
                  onChange={(e) => setUsage(e.target.value as ProductUsage)}
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none bg-white"
                >
                  <option value="VENTA">Venta (visible en POS y tienda)</option>
                  <option value="INSUMO">Insumo (solo producción o compras)</option>
                  <option value="INTERNO">Uso interno (dotación / consumo)</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Unidad de medida
                </label>
                <select
                  value={unitMeasureCode}
                  onChange={(e) => setUnitMeasureCode(e.target.value)}
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none bg-white"
                >
                  <option value="UND">Unidad (UND)</option>
                  <option value="KG">Kilogramo (KG)</option>
                  <option value="LB">Libra (LB)</option>
                  <option value="L">Litro (L)</option>
                  <option value="M">Metro (M)</option>
                  <option value="CAJA">Caja (CAJA)</option>
                  <option value="PAQUETE">Paquete (PAQUETE)</option>
                  <option value="BULTO">Bulto (BULTO)</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Impuesto (IVA / INC)
                </label>
                <select
                  value={taxCode}
                  onChange={(e) => setTaxCode(e.target.value)}
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none bg-white"
                >
                  <option value="IVA19">IVA General (19%)</option>
                  <option value="IVA5">IVA Reducido (5%)</option>
                  <option value="INC8">Impoconsumo (8%)</option>
                  <option value="EXENTO">Exento / Excluido (0%)</option>
                </select>
              </div>

              <div className="md:col-span-2">
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Descripción
                </label>
                <textarea
                  rows={2}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Detalles sobre el producto, presentación, ingredientes..."
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm outline-none"
                />
              </div>
            </div>
          </div>

          <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm space-y-6">
            <h2 className="text-base font-semibold text-slate-900 border-b border-slate-100 pb-3">
              Precios y costos base
            </h2>

            <div className="grid gap-4 md:grid-cols-2">
              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Precio de venta base (sin IVA) {usage === "VENTA" && "*"}
                </label>
                <input
                  type="number"
                  step="0.01"
                  required={usage === "VENTA"}
                  value={basePrice}
                  onChange={(e) => setBasePrice(e.target.value)}
                  placeholder="0.00"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Costo de referencia (sin IVA)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={costPrice}
                  onChange={(e) => setCostPrice(e.target.value)}
                  placeholder="0.00"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Punto de reorden (Stock mínimo)
                </label>
                <input
                  type="number"
                  step="0.0001"
                  value={reorderQty}
                  onChange={(e) => setReorderQty(e.target.value)}
                  placeholder="Ej: 10"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                  Stock máximo
                </label>
                <input
                  type="number"
                  step="0.0001"
                  value={maxQty}
                  onChange={(e) => setMaxQty(e.target.value)}
                  placeholder="Ej: 100"
                  className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm outline-none font-mono"
                />
              </div>
            </div>

            <div className="pt-2 border-t border-slate-100 space-y-3">
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={hasVariants}
                  onChange={(e) => setHasVariants(e.target.checked)}
                  className="size-4 rounded text-emerald-600 focus:ring-emerald-500"
                />
                <div>
                  <span className="text-sm font-medium text-slate-800">
                    Este producto tiene variantes
                  </span>
                  <p className="text-xs text-slate-500">
                    Habilita la creación de tallas, colores o presentaciones como productos hijos
                  </p>
                </div>
              </label>

              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={allowPriceOverride}
                  onChange={(e) => setAllowPriceOverride(e.target.checked)}
                  className="size-4 rounded text-emerald-600 focus:ring-emerald-500"
                />
                <div>
                  <span className="text-sm font-medium text-slate-800">
                    Permitir cambio de precio en el POS
                  </span>
                  <p className="text-xs text-slate-500">
                    El cajero puede modificar el precio si tiene permiso pos:price_override
                  </p>
                </div>
              </label>

              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={trackLots}
                  onChange={(e) => setTrackLots(e.target.checked)}
                  className="size-4 rounded text-emerald-600 focus:ring-emerald-500"
                />
                <div>
                  <span className="text-sm font-medium text-slate-800">
                    Controlar lotes y vencimientos
                  </span>
                  <p className="text-xs text-slate-500">
                    Exigir lote y fecha de vencimiento al recibir compras
                  </p>
                </div>
              </label>
            </div>
          </div>
          </fieldset>

          <div className="flex justify-end gap-3">
            <Link
              href="/inventario"
              className="px-5 py-2.5 rounded-full border border-slate-200 text-sm font-medium hover:bg-slate-50"
            >
              {detail?.archived ? "Volver" : "Cancelar"}
            </Link>
            {!detail?.archived && (
              <button
                type="submit"
                disabled={saving}
                className="flex items-center gap-2 px-6 py-2.5 rounded-full bg-emerald-600 text-white font-semibold text-sm shadow hover:bg-emerald-700 disabled:opacity-50"
              >
                <Save className="size-4" /> {saving ? "Guardando..." : "Guardar producto"}
              </button>
            )}
          </div>
        </form>
      )}

      {/* Tab: Precios y presentaciones */}
      {activeTab === "precios" && detail && (
        <fieldset disabled={Boolean(detail?.archived)} className="space-y-6">
          <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm space-y-4">
            <h2 className="text-base font-semibold text-slate-900 border-b border-slate-100 pb-3">
              Precio específico para sucursal ({branch.name})
            </h2>

            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50">
              <div>
                <span className="text-sm font-medium text-slate-900">
                  Personalizar precio en esta sucursal
                </span>
                <p className="text-xs text-slate-500">
                  Si se desmarca, se utiliza el precio base del catálogo ({formatCOP(Number(detail.price ?? 0))})
                </p>
              </div>
              <input
                type="checkbox"
                checked={customBranchPriceEnabled}
                onChange={(e) => setCustomBranchPriceEnabled(e.target.checked)}
                className="size-5 rounded text-emerald-600"
              />
            </div>

            {customBranchPriceEnabled && (
              <div className="flex items-end gap-3 pt-2">
                <div className="flex-1 max-w-xs">
                  <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                    Precio sucursal (sin IVA)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={branchPrice}
                    onChange={(e) => setBranchPrice(e.target.value)}
                    placeholder="0.00"
                    className="w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm font-mono outline-none"
                  />
                </div>
                <button
                  type="button"
                  onClick={handleSaveBranchPrice}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-slate-900 text-white font-medium text-sm hover:bg-slate-800"
                >
                  <Save className="size-4" /> Actualizar precio
                </button>
              </div>
            )}
          </div>

          <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm space-y-4">
            <div className="flex items-center justify-between border-b border-slate-100 pb-3">
              <div>
                <h2 className="text-base font-semibold text-slate-900">
                  Presentaciones y tarifas mayoristas
                </h2>
                <p className="text-xs text-slate-500">
                  Configura precios especiales por volumen o empaque
                </p>
              </div>
            </div>

            {detail.tier_prices.length === 0 ? (
              <p className="py-6 text-center text-sm text-slate-400">
                Este producto no tiene tarifas ni presentaciones configuradas.
              </p>
            ) : (
              <div className="divide-y divide-slate-100">
                {detail.tier_prices.map((tp) => (
                  <div key={tp.tier_id} className="py-3 flex items-center justify-between text-sm">
                    <div>
                      <p className="font-semibold text-slate-800">
                        Tarifa: {tp.tier_id.slice(0, 8)}
                      </p>
                      <p className="text-xs text-slate-400">
                        {tp.units_per_package ? `Paquete x ${tp.units_per_package} und.` : "Precio fijo"}
                      </p>
                    </div>
                    <span className="font-mono font-bold text-slate-900">
                      {tp.price ? formatCOP(Number(tp.price)) : "Calculado"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </fieldset>
      )}

      {/* Tab: Stock por bodega */}
      {activeTab === "bodegas" && detail && (
        <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm space-y-4">
          <div className="border-b border-slate-100 pb-3">
            <h2 className="text-base font-semibold text-slate-900">Disponibilidad en bodegas</h2>
            <p className="text-xs text-slate-500">
              Existencias registradas por ubicación y costo promedio
            </p>
          </div>

          <table className="w-full text-sm">
            <thead className="border-b border-slate-100 text-left text-xs uppercase text-slate-500">
              <tr>
                <th className="py-3 font-semibold">Ubicación / Bodega</th>
                <th className="py-3 text-right font-semibold">Físico</th>
                <th className="py-3 text-right font-semibold">Reservado</th>
                <th className="py-3 text-right font-semibold">Disponible</th>
                <th className="py-3 text-right font-semibold">Costo prom.</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {detail.stock_by_location.map((loc) => (
                <tr key={loc.location_id}>
                  <td className="py-3 font-medium text-slate-800">{loc.location_name}</td>
                  <td className="py-3 text-right tabular-nums">{formatQty(loc.on_hand)}</td>
                  <td className="py-3 text-right tabular-nums text-slate-400">{formatQty(loc.reserved)}</td>
                  <td className="py-3 text-right tabular-nums font-bold text-emerald-600">
                    {formatQty(String(Number(loc.on_hand) - Number(loc.reserved)))}
                  </td>
                  <td className="py-3 text-right tabular-nums text-slate-600 font-mono">
                    {formatCOP(Number(loc.avg_cost))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Tab: Variantes */}
      {activeTab === "variantes" && detail && (
        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <p className="text-sm text-slate-500">
              Variantes del producto padre: comparten unidad, impuesto y control de lotes.
            </p>
            {!detail.archived && (
              <button
                type="button"
                onClick={() => setShowVariantForm(!showVariantForm)}
                className="flex items-center gap-1.5 px-4 py-2 rounded-full bg-slate-900 text-white text-sm font-semibold hover:bg-slate-800"
              >
                <Plus className="size-4" /> Nueva variante
              </button>
            )}
          </div>

          {showVariantForm && (
            <form
              onSubmit={handleCreateVariant}
              className="rounded-2xl border border-emerald-200 bg-emerald-50/40 p-6 space-y-4"
            >
              <h3 className="text-sm font-bold text-slate-900 uppercase tracking-wider">
                Crear nueva variante
              </h3>

              <div>
                <label className="block text-xs font-semibold text-slate-600 uppercase mb-2">
                  Atributos (hasta 3 atributos: ej. Talla, Color)
                </label>
                <div className="space-y-2">
                  {variantAttrs.map((attr, idx) => (
                    <div key={idx} className="flex items-center gap-2">
                      <input
                        type="text"
                        placeholder="Atributo (ej. Talla)"
                        value={attr.key}
                        onChange={(e) => {
                          const next = [...variantAttrs];
                          next[idx].key = e.target.value;
                          setVariantAttrs(next);
                        }}
                        className="w-1/3 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm"
                      />
                      <input
                        type="text"
                        placeholder="Valor (ej. M)"
                        value={attr.value}
                        onChange={(e) => {
                          const next = [...variantAttrs];
                          next[idx].value = e.target.value;
                          setVariantAttrs(next);
                        }}
                        className="flex-1 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm"
                      />
                      {variantAttrs.length > 1 && (
                        <button
                          type="button"
                          onClick={() => setVariantAttrs(variantAttrs.filter((_, i) => i !== idx))}
                          className="p-1 text-slate-400 hover:text-rose-600"
                        >
                          <X className="size-4" />
                        </button>
                      )}
                    </div>
                  ))}
                  {variantAttrs.length < 3 && (
                    <button
                      type="button"
                      onClick={() => setVariantAttrs([...variantAttrs, { key: "", value: "" }])}
                      className="text-xs font-semibold text-emerald-700 hover:underline inline-flex items-center gap-1"
                    >
                      <Plus className="size-3" /> Agregar otro atributo
                    </button>
                  )}
                </div>
              </div>

              <div className="grid gap-3 sm:grid-cols-2 md:grid-cols-4">
                <div>
                  <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                    Precio *
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    required
                    value={variantPrice}
                    onChange={(e) => setVariantPrice(e.target.value)}
                    placeholder={detail.price ?? "0.00"}
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-mono"
                  />
                </div>
                <div>
                  <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                    Costo unit.
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={variantCost}
                    onChange={(e) => setVariantCost(e.target.value)}
                    placeholder="Opcional"
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-mono"
                  />
                </div>
                <div>
                  <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                    SKU
                  </label>
                  <input
                    type="text"
                    value={variantSku}
                    onChange={(e) => setVariantSku(e.target.value)}
                    placeholder="Opcional"
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-mono"
                  />
                </div>
                <div>
                  <label className="block text-xs font-semibold text-slate-600 uppercase mb-1">
                    Código de barras
                  </label>
                  <input
                    type="text"
                    value={variantBarcode}
                    onChange={(e) => setVariantBarcode(e.target.value)}
                    placeholder="Opcional"
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-mono"
                  />
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setShowVariantForm(false)}
                  className="px-4 py-1.5 rounded-full border border-slate-200 text-sm hover:bg-slate-100"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  className="px-5 py-1.5 rounded-full bg-emerald-600 text-white font-semibold text-sm hover:bg-emerald-700 shadow"
                >
                  Guardar variante
                </button>
              </div>
            </form>
          )}

          <div className="rounded-2xl border border-slate-200/60 bg-white overflow-hidden shadow-sm">
            <table className="w-full text-sm">
              <thead className="border-b border-slate-100 text-left text-xs uppercase text-slate-500 bg-slate-50">
                <tr>
                  <th className="px-6 py-3 font-semibold">Variante</th>
                  <th className="px-4 py-3 font-semibold">SKU / Barras</th>
                  <th className="px-4 py-3 text-right font-semibold">Precio</th>
                  <th className="px-4 py-3 text-right font-semibold">Costo</th>
                  <th className="px-4 py-3 text-right font-semibold">Stock</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {detail.variants.map((v) => (
                  <tr key={v.id} className="hover:bg-slate-50/50">
                    <td className="px-6 py-3.5 font-semibold text-slate-900">{v.name}</td>
                    <td className="px-4 py-3.5 font-mono text-xs text-slate-500">
                      {v.sku ?? v.barcode ?? "—"}
                    </td>
                    <td className="px-4 py-3.5 text-right font-mono font-medium">
                      {v.price ? formatCOP(Number(v.price)) : "—"}
                    </td>
                    <td className="px-4 py-3.5 text-right font-mono text-slate-500">
                      {v.cost_price ? formatCOP(Number(v.cost_price)) : "—"}
                    </td>
                    <td className="px-4 py-3.5 text-right font-bold tabular-nums">
                      {formatQty(v.on_hand)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Pestaña Modificadores */}
      {!isNew && activeTab === "modificadores" && (
        <fieldset disabled={Boolean(detail?.archived)} className="space-y-6">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div>
              <h2 className="text-base font-bold text-slate-900">Grupos de modificadores asignados</h2>
              <p className="text-sm text-slate-500">
                Selecciona los grupos de opciones que se ofrecerán al cajero al vender este producto.
              </p>
            </div>
            {!detail?.archived && (
              <Link
                href="/inventario/modificadores"
                className="inline-flex items-center gap-1.5 rounded-full border border-slate-200 bg-white px-3.5 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 transition-colors"
              >
                <Sliders className="size-3.5" /> Gestionar modificadores
              </Link>
            )}
          </div>

          <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm divide-y divide-slate-100">
            {allOptionGroups.length === 0 ? (
              <div className="p-8 text-center text-sm text-slate-500">
                No hay grupos de modificadores registrados.{" "}
                <Link href="/inventario/modificadores" className="text-emerald-600 font-medium hover:underline">
                  Crear grupos de modificadores
                </Link>
              </div>
            ) : (
              allOptionGroups.map((group) => {
                const assigned = assignedOptionGroups.find((g) => g.group_id === group.id);
                return (
                  <div
                    key={group.id}
                    className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 hover:bg-slate-50/50 transition-colors"
                  >
                    <div className="flex items-start gap-3">
                      <input
                        type="checkbox"
                        id={`group-${group.id}`}
                        checked={!!assigned}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setAssignedOptionGroups([
                              ...assignedOptionGroups,
                              { group_id: group.id, sort_order: assignedOptionGroups.length + 1 },
                            ]);
                          } else {
                            setAssignedOptionGroups(
                              assignedOptionGroups.filter((g) => g.group_id !== group.id)
                            );
                          }
                        }}
                        className="mt-1 size-4 rounded border-slate-300 text-emerald-600 focus:ring-emerald-500"
                      />
                      <div>
                        <label
                          htmlFor={`group-${group.id}`}
                          className="text-sm font-bold text-slate-900 cursor-pointer"
                        >
                          {group.name}
                        </label>
                        <p className="text-xs text-slate-500">
                          {group.min_select === 0
                            ? "Opcional"
                            : `Obligatorio (mín. ${group.min_select})`}{" "}
                          • Máx. {group.max_select} • {group.options.length} opciones
                        </p>
                        <div className="flex flex-wrap gap-1.5 mt-2">
                          {group.options.map((opt) => (
                            <span
                              key={opt.id}
                              className="inline-flex items-center gap-1 text-[11px] bg-slate-100 text-slate-600 rounded px-1.5 py-0.5"
                            >
                              {opt.name}{" "}
                              {Number(opt.price_delta) > 0 && `(+${formatCOP(Number(opt.price_delta))})`}
                            </span>
                          ))}
                        </div>
                      </div>
                    </div>

                    {assigned && (
                      <div className="flex items-center gap-2 self-end sm:self-center">
                        <label className="text-xs text-slate-500">Orden:</label>
                        <input
                          type="number"
                          min={1}
                          value={assigned.sort_order}
                          onChange={(e) => {
                            const val = Number(e.target.value);
                            setAssignedOptionGroups(
                              assignedOptionGroups.map((g) =>
                                g.group_id === group.id ? { ...g, sort_order: val } : g
                              )
                            );
                          }}
                          className="w-16 rounded-lg border border-slate-200 px-2 py-1 text-xs text-center focus:border-emerald-600 focus:outline-none"
                        />
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>

          {!detail?.archived && (
            <div className="flex justify-end">
              <button
                type="button"
                disabled={savingModifiers}
                onClick={async () => {
                  setSavingModifiers(true);
                  setErrorMsg(null);
                  try {
                    await api.options.setProductOptionGroups(productId, assignedOptionGroups);
                    setToast("Modificadores asignados guardados exitosamente");
                  } catch (err: unknown) {
                    setErrorMsg(
                      err instanceof Error ? err.message : "Error al guardar modificadores"
                    );
                  } finally {
                    setSavingModifiers(false);
                  }
                }}
                className="flex h-10 items-center gap-2 rounded-full bg-emerald-600 px-5 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 transition-colors disabled:opacity-50"
              >
                <Save className="size-4" />{" "}
                {savingModifiers ? "Guardando..." : "Guardar modificadores asignados"}
              </button>
            </div>
          )}
        </fieldset>
      )}
    </div>
  );
}

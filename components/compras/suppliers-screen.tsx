"use client";

import { useMemo, useState, useEffect } from "react";
import Link from "next/link";
import {
  Archive,
  ArrowLeft,
  Building2,
  CheckCircle2,
  Edit2,
  ExternalLink,
  FileText,
  Loader2,
  Mail,
  MapPin,
  Package,
  Phone,
  Plus,
  Power,
  PowerOff,
  Search,
  ShoppingCart,
  Star,
  User,
  Wallet,
  X,
} from "lucide-react";

import {
  api,
  friendlyError,
  type Supplier,
  type SupplierDetailResponse,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { SupplierDialog } from "./supplier-dialog";

const ADMIN_ROLES = ["DUENO", "ADMIN_CAJA"];

export function SuppliersScreen() {
  const branch = useCurrentBranch();

  const [includeInactive, setIncludeInactive] = useState(true);
  const [query, setQuery] = useState("");
  const [dialogOpen, setDialogOpen] = useState(
    () => (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("dialog") === "new"),
  );
  const [editingSupplier, setEditingSupplier] = useState<Supplier | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [selectedSupplier, setSelectedSupplier] = useState<Supplier | null>(null);
  const [supplierDetail, setSupplierDetail] = useState<SupplierDetailResponse | null>(null);
  const [activeTab, setActiveTab] = useState<"resumen" | "productos" | "ordenes" | "por_pagar">("resumen");
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const [archiveError, setArchiveError] = useState<string | null>(null);

  function closeDrawer() {
    setSelectedSupplier(null);
    setSupplierDetail(null);
    setArchiveError(null);
  }

  useEffect(() => {
    if (!selectedSupplier) return;
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoadingDetail(true);
    api.suppliers
      .get(selectedSupplier.id)
      .then((res) => {
        if (!cancelled) setSupplierDetail(res);
      })
      .catch((err) => {
        if (!cancelled) setArchiveError(friendlyError(err));
      })
      .finally(() => {
        if (!cancelled) setLoadingDetail(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedSupplier]);

  async function handleArchiveSupplier() {
    if (!selectedSupplier) return;
    setArchiveBusy(true);
    setArchiveError(null);
    try {
      await api.suppliers.archive(selectedSupplier.id);
      showToast(`Proveedor "${selectedSupplier.name}" archivado.`);
      closeDrawer();
    } catch (err) {
      setArchiveError(friendlyError(err));
    } finally {
      setArchiveBusy(false);
    }
  }

  async function handleTogglePreferred(productId: string, currentPreferred: boolean) {
    if (!selectedSupplier || !supplierDetail) return;
    try {
      const updatedItems = supplierDetail.products.map((p) => ({
        product_id: p.product_id,
        supplier_code: p.supplier_code,
        min_order_qty: p.min_order_qty,
        preferred: p.product_id === productId ? !currentPreferred : p.preferred,
      }));
      const res = await api.suppliers.setProducts(selectedSupplier.id, updatedItems);
      setSupplierDetail((prev) => (prev ? { ...prev, products: res } : null));
      showToast("Productos del proveedor actualizados.");
    } catch (err) {
      showToast(friendlyError(err));
    }
  }

  const { data, loading, error, reload } = useApi(
    () => api.suppliers.list(undefined, includeInactive),
    includeInactive ? "inactive" : "active",
  );

  const suppliers = useMemo(() => data ?? [], [data]);

  const filteredSuppliers = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return suppliers;
    return suppliers.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        (s.tax_id?.toLowerCase().includes(q) ?? false) ||
        (s.contact_name?.toLowerCase().includes(q) ?? false) ||
        (s.phone?.toLowerCase().includes(q) ?? false) ||
        (s.email?.toLowerCase().includes(q) ?? false),
    );
  }, [suppliers, query]);

  function showToast(message: string) {
    setToast(message);
    reload();
    setTimeout(() => setToast(null), 4000);
  }

  async function toggleSupplierActive(supplier: Supplier) {
    try {
      await api.suppliers.update(supplier.id, {
        name: supplier.name,
        active: !supplier.active,
      });
      showToast(
        supplier.active
          ? `Proveedor "${supplier.name}" desactivado.`
          : `Proveedor "${supplier.name}" activado.`,
      );
    } catch {
      showToast("No se pudo cambiar el estado del proveedor.");
    }
  }

  if (!ADMIN_ROLES.includes(branch.role)) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center p-6 text-center">
        <p className="max-w-md text-sm text-slate-500">
          Solo el dueño o el administrador pueden gestionar proveedores.
        </p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center p-6 text-center">
        <p className="max-w-md text-sm text-slate-500">No se pudieron cargar los proveedores.</p>
        <button
          onClick={reload}
          className="mt-4 rounded-full border border-slate-200 px-4 py-2 text-sm font-medium hover:bg-slate-50"
        >
          Reintentar
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast Notification */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 flex items-center gap-2 rounded-2xl bg-slate-900 px-5 py-3 text-sm font-medium text-white shadow-xl animate-in fade-in slide-in-from-bottom-2">
          <CheckCircle2 className="size-4 text-emerald-400" />
          <span>{toast}</span>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <Link
            href="/compras"
            className="inline-flex items-center gap-1.5 text-xs font-semibold text-slate-500 hover:text-slate-800 transition-colors mb-2"
          >
            <ArrowLeft className="size-3.5" />
            Volver a compras
          </Link>
          <h1 className="text-3xl font-bold tracking-tight text-slate-900 font-headline">
            Proveedores
          </h1>
          <p className="mt-1 text-slate-500">
            Directorio de distribuidores comerciales para todo el negocio.
          </p>
        </div>

        <button
          onClick={() => {
            setEditingSupplier(null);
            setDialogOpen(true);
          }}
          className="flex h-11 items-center gap-2 rounded-full bg-teal-700 px-5 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 transition-colors"
        >
          <Plus className="size-4" />
          Nuevo proveedor
        </button>
      </div>

      {/* Tabla y Filtros */}
      <div className="overflow-hidden rounded-3xl border border-slate-200/80 bg-white shadow-sm">
        {/* Barra de Filtros */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-100 p-5">
          {/* Buscador */}
          <div className="relative flex-1 min-w-[240px] max-w-md">
            <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 size-4 text-slate-400" />
            <input
              type="text"
              placeholder="Buscar por nombre, NIT, contacto o teléfono..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="w-full rounded-xl border border-slate-200 bg-slate-50/60 pl-10 pr-4 py-2 text-sm text-slate-800 placeholder-slate-400 focus:bg-white focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
            />
          </div>

          {/* Toggle Inactivos */}
          <div className="flex items-center gap-2 text-sm text-slate-600">
            <input
              type="checkbox"
              id="showInactive"
              checked={includeInactive}
              onChange={(e) => setIncludeInactive(e.target.checked)}
              className="size-4 rounded border-slate-300 text-teal-700 focus:ring-teal-700"
            />
            <label htmlFor="showInactive" className="cursor-pointer select-none">
              Incluir proveedores inactivos
            </label>
          </div>
        </div>

        {/* Tabla */}
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-slate-100 bg-slate-50/60 text-xs font-semibold uppercase tracking-wider text-slate-500">
              <tr>
                <th className="px-6 py-3.5">Nombre / Razón Social</th>
                <th className="px-6 py-3.5">NIT / Doc</th>
                <th className="px-6 py-3.5">Contacto</th>
                <th className="px-6 py-3.5">Teléfono / Email</th>
                <th className="px-6 py-3.5">Dirección</th>
                <th className="px-6 py-3.5 text-center">Estado</th>
                <th className="px-6 py-3.5 text-right">Acciones</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading && !data ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-slate-400">
                    <Loader2 className="mx-auto size-6 animate-spin text-teal-700" />
                    <p className="mt-2 text-sm">Cargando proveedores...</p>
                  </td>
                </tr>
              ) : filteredSuppliers.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-slate-400">
                    No se encontraron proveedores.
                  </td>
                </tr>
              ) : (
                filteredSuppliers.map((s) => (
                  <tr
                    key={s.id}
                    onClick={() => setSelectedSupplier(s)}
                    className="hover:bg-slate-50/80 cursor-pointer transition-colors"
                  >
                    <td className="px-6 py-4">
                      <div className="flex items-center gap-2.5">
                        <span className="flex size-8 shrink-0 items-center justify-center rounded-xl bg-slate-100 text-slate-600">
                          <Building2 className="size-4" />
                        </span>
                        <div>
                          <p className="font-semibold text-slate-900">{s.name}</p>
                          {s.notes && (
                            <p className="max-w-xs truncate text-xs text-slate-400">
                              {s.notes}
                            </p>
                          )}
                        </div>
                      </div>
                    </td>
                    <td className="px-6 py-4 font-mono text-xs text-slate-600">
                      {s.doc_type ?? "NIT"} {s.tax_id ?? "—"}{s.dv ? `-${s.dv}` : ""}
                    </td>
                    <td className="px-6 py-4 text-slate-700">
                      {s.contact_name ? (
                        <span className="inline-flex items-center gap-1.5">
                          <User className="size-3.5 text-slate-400" />
                          {s.contact_name}
                        </span>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="px-6 py-4 text-xs text-slate-600 space-y-0.5">
                      {s.phone && (
                        <p className="flex items-center gap-1.5 font-mono">
                          <Phone className="size-3 text-slate-400" />
                          {s.phone}
                        </p>
                      )}
                      {s.email && (
                        <p className="flex items-center gap-1.5 text-slate-500">
                          <Mail className="size-3 text-slate-400" />
                          {s.email}
                        </p>
                      )}
                      {!s.phone && !s.email && "—"}
                    </td>
                    <td className="px-6 py-4 text-xs text-slate-600 max-w-xs">
                      {s.address ? (
                        <span className="inline-flex items-center gap-1.5">
                          <MapPin className="size-3 text-slate-400 shrink-0" />
                          <span className="truncate">{s.address}</span>
                        </span>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="px-6 py-4 text-center whitespace-nowrap">
                      {s.active ? (
                        <span className="inline-flex items-center gap-1 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-0.5 text-xs font-semibold text-emerald-700">
                          <span className="size-1.5 rounded-full bg-emerald-500" />
                          Activo
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full border border-slate-200 bg-slate-100 px-2.5 py-0.5 text-xs font-semibold text-slate-500">
                          <span className="size-1.5 rounded-full bg-slate-400" />
                          Inactivo
                        </span>
                      )}
                    </td>
                    <td className="px-6 py-4 text-right whitespace-nowrap" onClick={(e) => e.stopPropagation()}>
                      <div className="inline-flex items-center gap-1.5">
                        <button
                          type="button"
                          onClick={() => setSelectedSupplier(s)}
                          className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-600 hover:bg-slate-50"
                          title="Ver ficha del proveedor"
                        >
                          <FileText className="size-3.5" />
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setEditingSupplier(s);
                            setDialogOpen(true);
                          }}
                          className="flex size-8 items-center justify-center rounded-lg border border-slate-200 text-slate-600 hover:bg-slate-50"
                          title="Editar proveedor"
                        >
                          <Edit2 className="size-3.5" />
                        </button>
                        <button
                          type="button"
                          onClick={() => toggleSupplierActive(s)}
                          className={cn(
                            "flex size-8 items-center justify-center rounded-lg border transition-colors",
                            s.active
                              ? "border-amber-200 text-amber-600 hover:bg-amber-50"
                              : "border-emerald-200 text-emerald-600 hover:bg-emerald-50",
                          )}
                          title={s.active ? "Desactivar proveedor" : "Activar proveedor"}
                        >
                          {s.active ? (
                            <PowerOff className="size-3.5" />
                          ) : (
                            <Power className="size-3.5" />
                          )}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Panel Lateral - Ficha del Proveedor */}
      {selectedSupplier && (
        <div className="fixed inset-0 z-50 flex justify-end bg-slate-900/40 backdrop-blur-xs animate-in fade-in duration-200">
          <div className="relative flex w-full max-w-xl flex-col bg-white shadow-2xl animate-in slide-in-from-right duration-300">
            {/* Header del Panel */}
            <div className="flex items-center justify-between border-b border-slate-100 p-6">
              <div className="flex items-center gap-3">
                <span className="flex size-10 items-center justify-center rounded-2xl bg-teal-50 text-teal-700">
                  <Building2 className="size-5" />
                </span>
                <div>
                  <h2 className="text-lg font-bold text-slate-900 font-headline">
                    {selectedSupplier.name}
                  </h2>
                  <p className="font-mono text-xs text-slate-500">
                    {selectedSupplier.doc_type ?? "NIT"} {selectedSupplier.tax_id ?? "—"}
                    {selectedSupplier.dv ? `-${selectedSupplier.dv}` : ""}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={closeDrawer}
                className="flex size-8 items-center justify-center rounded-full text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="size-4" />
              </button>
            </div>

            {/* Error banner si falló archivar */}
            {archiveError && (
              <div className="mx-6 mt-4 rounded-xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-700">
                {archiveError}
              </div>
            )}

            {/* Pestañas: Resumen, Productos, Órdenes, Por pagar */}
            <div className="flex border-b border-slate-100 px-6">
              {[
                { id: "resumen", label: "Resumen", icon: FileText },
                {
                  id: "productos",
                  label: `Productos (${supplierDetail?.products?.length ?? 0})`,
                  icon: Package,
                },
                {
                  id: "ordenes",
                  label: `Órdenes (${supplierDetail?.open_orders ?? 0})`,
                  icon: ShoppingCart,
                },
                {
                  id: "por_pagar",
                  label: "Por pagar",
                  icon: Wallet,
                },
              ].map((tab) => {
                const Icon = tab.icon;
                const active = activeTab === tab.id;
                return (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveTab(tab.id as typeof activeTab)}
                    className={cn(
                      "flex items-center gap-1.5 border-b-2 py-3.5 px-3 text-xs font-semibold transition-colors",
                      active
                        ? "border-teal-700 text-teal-800"
                        : "border-transparent text-slate-500 hover:text-slate-800",
                    )}
                  >
                    <Icon className="size-3.5" />
                    <span>{tab.label}</span>
                  </button>
                );
              })}
            </div>

            {/* Contenido de la pestaña */}
            <div className="flex-1 overflow-y-auto p-6 space-y-6">
              {loadingDetail && !supplierDetail ? (
                <div className="flex flex-col items-center justify-center py-12 text-slate-400">
                  <Loader2 className="size-6 animate-spin text-teal-700" />
                  <p className="mt-2 text-sm">Cargando ficha...</p>
                </div>
              ) : activeTab === "resumen" ? (
                <div className="space-y-4 text-sm">
                  {/* Datos Comerciales y Tributarios */}
                  <div className="rounded-2xl border border-slate-100 bg-slate-50/50 p-4 space-y-3">
                    <p className="text-xs font-bold uppercase tracking-wider text-slate-500">
                      Régimen y Tributación
                    </p>
                    <div className="grid grid-cols-2 gap-3 text-xs">
                      <div>
                        <span className="text-slate-400 block">Tipo Persona</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.person_type ?? "JURIDICA"}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block">Categoría</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.category ?? "MERCANCIA"}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block">IVA</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.iva_responsible ? "Responsable" : "No responsable"}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block">Autorretenedor</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.self_withholding ? "Sí" : "No"}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Condiciones de Pago y Despacho */}
                  <div className="rounded-2xl border border-slate-100 bg-slate-50/50 p-4 space-y-3">
                    <p className="text-xs font-bold uppercase tracking-wider text-slate-500">
                      Condiciones Comerciales
                    </p>
                    <div className="grid grid-cols-2 gap-3 text-xs">
                      <div>
                        <span className="text-slate-400 block">Términos de Pago</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.payment_terms === "CREDITO"
                            ? `Crédito (${selectedSupplier.credit_days} días)`
                            : "Contado"}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block">Días de Entrega</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.delivery_days != null
                            ? `${selectedSupplier.delivery_days} días`
                            : "No especificado"}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Datos Bancarios */}
                  <div className="rounded-2xl border border-slate-100 bg-slate-50/50 p-4 space-y-3">
                    <p className="text-xs font-bold uppercase tracking-wider text-slate-500">
                      Datos Bancarios
                    </p>
                    <div className="grid grid-cols-1 gap-2 text-xs">
                      <div>
                        <span className="text-slate-400 block">Banco</span>
                        <span className="font-semibold text-slate-800">
                          {selectedSupplier.bank_name || "No registrado"}
                        </span>
                      </div>
                      {selectedSupplier.bank_account_number && (
                        <div>
                          <span className="text-slate-400 block">Cuenta</span>
                          <span className="font-mono font-semibold text-slate-800">
                            {selectedSupplier.bank_account_type} {selectedSupplier.bank_account_number}
                          </span>
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Contacto y Ubicación */}
                  <div className="rounded-2xl border border-slate-100 bg-slate-50/50 p-4 space-y-3">
                    <p className="text-xs font-bold uppercase tracking-wider text-slate-500">
                      Contacto y Ubicación
                    </p>
                    <div className="space-y-1 text-xs text-slate-700">
                      {selectedSupplier.contact_name && (
                        <p className="flex items-center gap-2">
                          <User className="size-3.5 text-slate-400" />
                          <span>{selectedSupplier.contact_name}</span>
                        </p>
                      )}
                      {selectedSupplier.phone && (
                        <p className="flex items-center gap-2 font-mono">
                          <Phone className="size-3.5 text-slate-400" />
                          <span>{selectedSupplier.phone}</span>
                        </p>
                      )}
                      {selectedSupplier.email && (
                        <p className="flex items-center gap-2">
                          <Mail className="size-3.5 text-slate-400" />
                          <span>{selectedSupplier.email}</span>
                        </p>
                      )}
                      {selectedSupplier.address && (
                        <p className="flex items-center gap-2">
                          <MapPin className="size-3.5 text-slate-400" />
                          <span>{selectedSupplier.address}</span>
                        </p>
                      )}
                      {selectedSupplier.notes && (
                        <p className="mt-2 text-xs text-slate-500 border-t border-slate-100 pt-2 italic">
                          &quot;{selectedSupplier.notes}&quot;
                        </p>
                      )}
                    </div>
                  </div>
                </div>
              ) : activeTab === "productos" ? (
                <div className="space-y-3">
                  <p className="text-xs text-slate-500">
                    Productos suministrados por este proveedor y marca de proveedor preferido.
                  </p>
                  {supplierDetail?.products && supplierDetail.products.length > 0 ? (
                    <div className="overflow-hidden rounded-2xl border border-slate-200">
                      <table className="w-full text-left text-xs">
                        <thead className="bg-slate-50 border-b border-slate-200 font-semibold text-slate-600">
                          <tr>
                            <th className="p-3">Producto</th>
                            <th className="p-3">Cód. Prov</th>
                            <th className="p-3 text-right">Mínimo</th>
                            <th className="p-3 text-center">Preferido</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-100">
                          {supplierDetail.products.map((p) => (
                            <tr key={p.product_id} className="hover:bg-slate-50">
                              <td className="p-3">
                                <p className="font-semibold text-slate-900">{p.product_name}</p>
                                {p.sku && <p className="text-[10px] text-slate-400">{p.sku}</p>}
                              </td>
                              <td className="p-3 font-mono text-slate-600">
                                {p.supplier_code || "—"}
                              </td>
                              <td className="p-3 text-right font-mono">
                                {p.min_order_qty != null ? Number(p.min_order_qty).toFixed(0) : "—"}
                              </td>
                              <td className="p-3 text-center">
                                <button
                                  type="button"
                                  onClick={() => handleTogglePreferred(p.product_id, p.preferred)}
                                  className={cn(
                                    "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-semibold border transition-colors",
                                    p.preferred
                                      ? "border-amber-200 bg-amber-50 text-amber-700"
                                      : "border-slate-200 text-slate-400 hover:text-slate-600",
                                  )}
                                  title={p.preferred ? "Quitar marca preferido" : "Marcar como proveedor preferido"}
                                >
                                  <Star
                                    className={cn(
                                      "size-3",
                                      p.preferred ? "fill-amber-400 text-amber-400" : "text-slate-300",
                                    )}
                                  />
                                  <span>{p.preferred ? "Sí" : "No"}</span>
                                </button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <div className="rounded-2xl border border-dashed border-slate-200 p-8 text-center text-xs text-slate-400">
                      No hay productos asociados a este proveedor todavía.
                    </div>
                  )}
                </div>
              ) : activeTab === "ordenes" ? (
                <div className="space-y-4">
                  <div className="rounded-2xl border border-teal-100 bg-teal-50/50 p-4">
                    <p className="text-xs font-semibold text-teal-800">Órdenes abiertas</p>
                    <p className="text-2xl font-bold text-teal-900 mt-1">
                      {supplierDetail?.open_orders ?? 0}
                    </p>
                    <p className="text-xs text-teal-700 mt-0.5">
                      En estado Borrador, Aprobada o Parcial.
                    </p>
                  </div>
                  <Link
                    href={`/compras?supplier_id=${selectedSupplier.id}`}
                    className="inline-flex items-center gap-2 text-xs font-semibold text-teal-700 hover:text-teal-900"
                  >
                    <span>Ver todas las órdenes de este proveedor</span>
                    <ExternalLink className="size-3" />
                  </Link>
                </div>
              ) : (
                <div className="space-y-4">
                  <div className="rounded-2xl border border-slate-200 bg-slate-50/50 p-4">
                    <p className="text-xs font-semibold text-slate-600">Saldo pendiente por pagar</p>
                    <p className="text-2xl font-bold text-slate-900 font-mono mt-1">
                      ${Number(supplierDetail?.balance ?? 0).toLocaleString("es-CO", {
                        minimumFractionDigits: 2,
                        maximumFractionDigits: 2,
                      })}
                    </p>
                    <p className="text-xs text-slate-500 mt-0.5">
                      Cuentas abiertas o con saldo pendiente.
                    </p>
                  </div>
                  <Link
                    href={`/compras/por-pagar?supplier_id=${selectedSupplier.id}`}
                    className="inline-flex items-center gap-2 text-xs font-semibold text-teal-700 hover:text-teal-900"
                  >
                    <span>Ver cuentas por pagar de este proveedor</span>
                    <ExternalLink className="size-3" />
                  </Link>
                </div>
              )}
            </div>

            {/* Footer de la Ficha */}
            <div className="flex items-center justify-between border-t border-slate-100 p-6 bg-slate-50/60">
              <button
                type="button"
                onClick={handleArchiveSupplier}
                disabled={archiveBusy}
                className="inline-flex items-center gap-1.5 rounded-full border border-rose-200 px-4 py-2 text-xs font-semibold text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                title="Archivar proveedor (requiere que no tenga órdenes ni cuentas abiertas)"
              >
                {archiveBusy ? <Loader2 className="size-3.5 animate-spin" /> : <Archive className="size-3.5" />}
                <span>Archivar proveedor</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  setEditingSupplier(selectedSupplier);
                  setDialogOpen(true);
                }}
                className="inline-flex items-center gap-1.5 rounded-full bg-teal-700 px-5 py-2 text-xs font-semibold text-white shadow-sm hover:bg-teal-800"
              >
                <Edit2 className="size-3.5" />
                <span>Editar proveedor</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Diálogo de Creación / Edición */}
      <SupplierDialog
        open={dialogOpen}
        supplier={editingSupplier}
        onClose={() => setDialogOpen(false)}
        onSaved={(saved) => {
          showToast(
            editingSupplier
              ? `Proveedor "${saved.name}" actualizado.`
              : `Proveedor "${saved.name}" creado exitosamente.`,
          );
        }}
      />
    </div>
  );
}

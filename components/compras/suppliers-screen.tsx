"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  ArrowLeft,
  Building2,
  CheckCircle2,
  Edit2,
  Loader2,
  Mail,
  MapPin,
  Phone,
  Plus,
  Power,
  PowerOff,
  Search,
  User,
} from "lucide-react";

import { api, type Supplier } from "@/lib/api";
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
                  <tr key={s.id} className="hover:bg-slate-50/80 transition-colors">
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
                      {s.tax_id ?? "—"}
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
                    <td className="px-6 py-4 text-right whitespace-nowrap">
                      <div className="inline-flex items-center gap-1.5">
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

"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  Building2,
  CheckCircle2,
  Loader2,
  Plus,
  Search,
  Truck,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { api, type PurchaseStatus } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch } from "@/lib/stores/session";
import { formatCOP, formatDateTime } from "@/lib/format";
import { cn } from "@/lib/utils";

import { PurchaseDetailDialog } from "./purchase-detail-dialog";
import { PurchaseForm } from "./purchase-form";

const ADMIN_ROLES = ["DUENO", "ADMIN_CAJA"];

const STATUS_CONFIG: Record<
  PurchaseStatus,
  { label: string; className: string; dot: string }
> = {
  RECIBIDA: {
    label: "Registrada",
    className: "border-emerald-200 bg-emerald-50 text-emerald-700",
    dot: "bg-emerald-500",
  },
  PENDIENTE: {
    label: "Pendiente de revisión",
    className: "border-amber-200 bg-amber-50 text-amber-700",
    dot: "bg-amber-500",
  },
  ANULADA: {
    label: "Anulada",
    className: "border-slate-200 bg-slate-100 text-slate-500",
    dot: "bg-slate-400",
  },
};

export function PurchasesScreen() {
  const branch = useCurrentBranch();

  // Load purchases list
  const {
    data: purchasesData,
    loading: loadingPurchases,
    error: errorPurchases,
    reload: reloadPurchases,
  } = useApi(() => api.purchases.list({ branch_id: branch.id }), branch.id);

  // Load summary metrics
  const {
    data: summaryData,
    reload: reloadSummary,
  } = useApi(() => api.purchases.summary(branch.id), branch.id);

  // Filter state
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<PurchaseStatus | "">("");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");

  // Dialogs
  const [selectedPurchaseId, setSelectedPurchaseId] = useState<string | null>(null);
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const purchases = useMemo(() => purchasesData ?? [], [purchasesData]);

  // Client filtering
  const filteredRows = useMemo(() => {
    return purchases.filter((p) => {
      const q = query.trim().toLowerCase();
      const matchesQuery =
        !q ||
        p.supplier_name.toLowerCase().includes(q) ||
        (p.document_number?.toLowerCase().includes(q) ?? false);

      const matchesStatus = !statusFilter || p.status === statusFilter;

      let matchesDate = true;
      if (fromDate) {
        const fromTime = new Date(fromDate).getTime();
        matchesDate = matchesDate && new Date(p.created_at).getTime() >= fromTime;
      }
      if (toDate) {
        // End of that day
        const toTime = new Date(toDate).getTime() + 24 * 3600_000 - 1;
        matchesDate = matchesDate && new Date(p.created_at).getTime() <= toTime;
      }

      return matchesQuery && matchesStatus && matchesDate;
    });
  }, [purchases, query, statusFilter, fromDate, toDate]);

  function showToast(message: string) {
    setToast(message);
    reloadPurchases();
    reloadSummary();
    setTimeout(() => setToast(null), 4000);
  }

  if (!ADMIN_ROLES.includes(branch.role)) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center p-6 text-center">
        <p className="max-w-md text-sm text-slate-500">
          Solo el dueño o el administrador pueden gestionar las compras de esta sucursal.
        </p>
      </div>
    );
  }

  if (errorPurchases) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center p-6 text-center">
        <p className="max-w-md text-sm text-slate-500">No se pudieron cargar las compras.</p>
        <button
          onClick={() => {
            reloadPurchases();
            reloadSummary();
          }}
          className="mt-4 rounded-full border border-slate-200 px-4 py-2 text-sm font-medium hover:bg-slate-50"
        >
          Reintentar
        </button>
      </div>
    );
  }

  const totalComprado = summaryData?.total_purchased_month
    ? Number(summaryData.total_purchased_month)
    : 0;
  const proveedoresActivos = summaryData?.active_suppliers ?? 0;

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
          <h1 className="text-3xl font-bold tracking-tight text-slate-900 font-headline">
            Compras y recepciones
          </h1>
          <p className="mt-1 text-slate-500">
            Ingreso de mercancía por facturas de compra en {branch.name}.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Link
            href="/compras/proveedores"
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50 shadow-xs"
          >
            <Building2 className="size-4 text-slate-500" />
            Proveedores
          </Link>

          <button
            onClick={() => setIsFormOpen(true)}
            className="flex h-11 items-center gap-2 rounded-full bg-teal-700 px-5 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 transition-colors"
          >
            <Plus className="size-4" />
            Nueva compra
          </button>
        </div>
      </div>

      {/* 2 StatCards: Total Comprado este mes & Proveedores Activos */}
      <div className="grid gap-6 md:grid-cols-2">
        <StatCard
          title="TOTAL COMPRADO ESTE MES"
          value={formatCOP(totalComprado)}
          caption="Compras recibidas e ingresadas al inventario"
          icon={Truck}
          tone="success"
        />
        <StatCard
          title="PROVEEDORES ACTIVOS"
          value={String(proveedoresActivos)}
          caption="Distribuidores comerciales registrados en el negocio"
          icon={Building2}
          tone="default"
        />
      </div>

      {/* Tabla con Búsqueda y Filtros */}
      <div className="overflow-hidden rounded-3xl border border-slate-200/80 bg-white shadow-sm">
        {/* Barra de Filtros */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-100 p-5">
          {/* Buscador */}
          <div className="relative flex-1 min-w-[240px] max-w-md">
            <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 size-4 text-slate-400" />
            <input
              type="text"
              placeholder="Buscar por proveedor o N.° factura..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="w-full rounded-xl border border-slate-200 bg-slate-50/60 pl-10 pr-4 py-2 text-sm text-slate-800 placeholder-slate-400 focus:bg-white focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
            />
          </div>

          {/* Filtros de Estado y Fecha */}
          <div className="flex flex-wrap items-center gap-3">
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value as PurchaseStatus | "")}
              className="h-10 rounded-xl border border-slate-200 bg-white px-3 text-xs font-medium text-slate-700 focus:border-teal-700 focus:outline-none"
            >
              <option value="">Todos los estados</option>
              <option value="RECIBIDA">Registrada</option>
              <option value="PENDIENTE">Pendiente de revisión</option>
              <option value="ANULADA">Anulada</option>
            </select>

            <div className="flex items-center gap-1.5 text-xs text-slate-500">
              <span className="font-medium">Desde:</span>
              <input
                type="date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
                className="h-10 rounded-xl border border-slate-200 bg-white px-2.5 py-1 text-xs text-slate-700 focus:border-teal-700 focus:outline-none"
              />
            </div>

            <div className="flex items-center gap-1.5 text-xs text-slate-500">
              <span className="font-medium">Hasta:</span>
              <input
                type="date"
                value={toDate}
                onChange={(e) => setToDate(e.target.value)}
                className="h-10 rounded-xl border border-slate-200 bg-white px-2.5 py-1 text-xs text-slate-700 focus:border-teal-700 focus:outline-none"
              />
            </div>
          </div>
        </div>

        {/* Tabla */}
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-slate-100 bg-slate-50/60 text-xs font-semibold uppercase tracking-wider text-slate-500">
              <tr>
                <th className="px-6 py-3.5">Fecha</th>
                <th className="px-6 py-3.5">Proveedor</th>
                <th className="px-6 py-3.5">N.° Factura</th>
                <th className="px-6 py-3.5 text-center">Ítems</th>
                <th className="px-6 py-3.5 text-right">Total</th>
                <th className="px-6 py-3.5 text-center">Estado</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loadingPurchases && !purchasesData ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-slate-400">
                    <Loader2 className="mx-auto size-6 animate-spin text-teal-700" />
                    <p className="mt-2 text-sm">Cargando compras...</p>
                  </td>
                </tr>
              ) : filteredRows.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-slate-400">
                    No se encontraron compras con los filtros seleccionados.
                  </td>
                </tr>
              ) : (
                filteredRows.map((p) => {
                  const statusConf = STATUS_CONFIG[p.status];
                  return (
                    <tr
                      key={p.id}
                      onClick={() => setSelectedPurchaseId(p.id)}
                      className="cursor-pointer hover:bg-slate-50/80 transition-colors"
                    >
                      <td className="px-6 py-4 whitespace-nowrap text-slate-600">
                        {formatDateTime(p.created_at)}
                      </td>
                      <td className="px-6 py-4 font-semibold text-slate-900">
                        {p.supplier_name}
                      </td>
                      <td className="px-6 py-4 font-mono text-xs text-slate-600">
                        {p.document_number ?? "—"}
                      </td>
                      <td className="px-6 py-4 text-center tabular-nums text-slate-600">
                        {p.items_count} producto{p.items_count !== 1 ? "s" : ""}
                      </td>
                      <td className="px-6 py-4 text-right font-bold tabular-nums text-slate-900">
                        {formatCOP(Number(p.total_amount))}
                      </td>
                      <td className="px-6 py-4 text-center whitespace-nowrap">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-semibold",
                            statusConf.className,
                          )}
                        >
                          <span className={cn("size-1.5 rounded-full", statusConf.dot)} />
                          {statusConf.label}
                        </span>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Diálogo de Detalle */}
      <PurchaseDetailDialog
        open={!!selectedPurchaseId}
        purchaseId={selectedPurchaseId}
        branchName={branch.name}
        onClose={() => setSelectedPurchaseId(null)}
        onActionDone={showToast}
      />

      {/* Formulario de Nueva Compra */}
      <PurchaseForm
        open={isFormOpen}
        branchId={branch.id}
        branchName={branch.name}
        onClose={() => setIsFormOpen(false)}
        onSaved={showToast}
      />
    </div>
  );
}

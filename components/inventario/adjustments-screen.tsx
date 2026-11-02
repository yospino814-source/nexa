"use client";

import { useMemo, useState } from "react";
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Clock,
  Filter,
  Loader2,
  RefreshCw,
  Search,
  SlidersHorizontal,
  XCircle,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type InventoryAdjustment,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatDateTime, formatQty } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type StatusTab = "TODOS" | "PENDIENTE" | "APLICADO" | "RECHAZADO";

const REASON_LABELS: Record<string, string> = {
  DANO: "Daño / avería",
  PERDIDA: "Pérdida / merma",
  ROBO: "Robo / faltante",
  VENCIMIENTO: "Vencimiento",
  CONTEO: "Conteo físico",
  CORRECCION: "Corrección",
  CARGA_INICIAL: "Carga inicial",
};

export function AdjustmentsScreen() {
  const currentBranch = useCurrentBranch();
  const branchId = currentBranch?.id;

  const [activeTab, setActiveTab] = useState<StatusTab>("TODOS");
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedForApprove, setSelectedForApprove] =
    useState<InventoryAdjustment | null>(null);
  const [selectedForReject, setSelectedForReject] =
    useState<InventoryAdjustment | null>(null);
  const [rejectReason, setRejectReason] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [actionSuccess, setActionSuccess] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const canApprove = useSession(
    (s) =>
      s.can("inventory:adjust_approve") ||
      ["DUENO", "ADMIN_CAJA"].includes(
        s.branches.find((b) => b.id === s.branchId)?.role ?? "",
      ),
  );

  const {
    data: adjustments = [],
    loading,
    error,
    reload,
  } = useApi(
    () => api.inventory.adjustments.list({ branch_id: branchId }),
    `adjustments-${branchId}`,
  );

  const safeAdjustments = useMemo(() => adjustments ?? [], [adjustments]);

  // Conteos y métricas
  const stats = useMemo(() => {
    let pending = 0;
    let applied = 0;
    let rejected = 0;
    let pendingValue = 0;

    for (const a of safeAdjustments) {
      if (a.status === "PENDIENTE") {
        pending++;
        pendingValue += Number(a.value);
      } else if (a.status === "APLICADO") {
        applied++;
      } else if (a.status === "RECHAZADO") {
        rejected++;
      }
    }

    return { pending, applied, rejected, pendingValue };
  }, [safeAdjustments]);

  // Filtrado de lista
  const filteredAdjustments = useMemo(() => {
    return safeAdjustments.filter((a) => {
      if (activeTab !== "TODOS" && a.status !== activeTab) return false;
      if (!searchTerm) return true;
      const term = searchTerm.toLowerCase();
      const matchProd = (a.product_name ?? "").toLowerCase().includes(term);
      const matchSku = (a.sku ?? "").toLowerCase().includes(term);
      const matchReason = (REASON_LABELS[a.reason_code] ?? a.reason_code)
        .toLowerCase()
        .includes(term);
      const matchRequester = (a.requester_name ?? "").toLowerCase().includes(term);
      return matchProd || matchSku || matchReason || matchRequester;
    });
  }, [safeAdjustments, activeTab, searchTerm]);

  // Manejador Aprobar
  async function handleApprove() {
    if (!selectedForApprove) return;
    setIsProcessing(true);
    setActionError(null);
    try {
      await api.inventory.adjustments.approve(selectedForApprove.id);
      setActionSuccess(
        `Ajuste de ${selectedForApprove.product_name} aprobado exitosamente. Stock actualizado.`,
      );
      setSelectedForApprove(null);
      await reload();
      setTimeout(() => setActionSuccess(null), 5000);
    } catch (err) {
      setActionError(friendlyError(err));
    } finally {
      setIsProcessing(false);
    }
  }

  // Manejador Rechazar
  async function handleReject() {
    if (!selectedForReject) return;
    if (!rejectReason.trim()) {
      setActionError("Debe ingresar un motivo de rechazo");
      return;
    }
    setIsProcessing(true);
    setActionError(null);
    try {
      await api.inventory.adjustments.reject(
        selectedForReject.id,
        rejectReason.trim(),
      );
      setActionSuccess(
        `Ajuste de ${selectedForReject.product_name} rechazado correctamente.`,
      );
      setSelectedForReject(null);
      setRejectReason("");
      await reload();
      setTimeout(() => setActionSuccess(null), 5000);
    } catch (err) {
      setActionError(friendlyError(err));
    } finally {
      setIsProcessing(false);
    }
  }

  return (
    <div className="space-y-6 p-6">
      {/* Encabezado */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="flex items-center gap-3">
            <span className="flex size-10 items-center justify-center rounded-2xl bg-brand/10 text-brand">
              <SlidersHorizontal className="size-5" />
            </span>
            <div>
              <h1 className="text-2xl font-bold tracking-tight text-slate-900">
                Ajustes de inventario
              </h1>
              <p className="text-sm text-slate-500">
                Control y aprobación de ajustes por merma, conteo o corrección
              </p>
            </div>
          </div>
        </div>

        <button
          onClick={() => reload()}
          disabled={loading}
          className="flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 shadow-sm transition-colors hover:bg-slate-50 disabled:opacity-50"
        >
          <RefreshCw className={cn("size-4", loading && "animate-spin")} />
          Actualizar
        </button>
      </div>

      {/* Alertas de acción */}
      {actionSuccess && (
        <div className="flex items-center gap-3 rounded-2xl border border-emerald-200 bg-emerald-50 p-4 text-sm font-medium text-emerald-800">
          <CheckCircle2 className="size-5 shrink-0 text-emerald-600" />
          <span>{actionSuccess}</span>
        </div>
      )}

      {actionError && (
        <div className="flex items-center gap-3 rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm font-medium text-rose-800">
          <AlertCircle className="size-5 shrink-0 text-rose-600" />
          <span>{actionError}</span>
        </div>
      )}

      {/* Tarjetas de métricas */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Pendientes por aprobar"
          value={stats.pending.toString()}
          caption="Superan el umbral establecido"
          icon={Clock}
          tone={stats.pending > 0 ? "warning" : "default"}
        />
        <StatCard
          title="Valor pendiente"
          value={formatCOP(stats.pendingValue)}
          caption="Monto en espera de aprobación"
          icon={AlertTriangle}
          tone={stats.pendingValue > 0 ? "warning" : "default"}
        />
        <StatCard
          title="Aplicados"
          value={stats.applied.toString()}
          caption="Impactaron el kardex"
          icon={CheckCircle2}
        />
        <StatCard
          title="Rechazados"
          value={stats.rejected.toString()}
          caption="Sin impacto en existencias"
          icon={XCircle}
        />
      </div>

      {/* Barra de pestañas y búsqueda */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex rounded-2xl border border-slate-200 bg-slate-100/80 p-1">
          {(
            [
              { id: "TODOS", label: "Todos", count: safeAdjustments.length },
              { id: "PENDIENTE", label: "Pendientes", count: stats.pending },
              { id: "APLICADO", label: "Aplicados", count: stats.applied },
              { id: "RECHAZADO", label: "Rechazados", count: stats.rejected },
            ] as const
          ).map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "flex items-center gap-2 rounded-xl px-4 py-2 text-sm font-medium transition-colors",
                activeTab === tab.id
                  ? "bg-white text-slate-900 shadow-sm"
                  : "text-slate-600 hover:text-slate-900",
              )}
            >
              <span>{tab.label}</span>
              <span
                className={cn(
                  "rounded-full px-2 py-0.5 text-xs font-semibold",
                  activeTab === tab.id
                    ? "bg-slate-100 text-slate-900"
                    : "bg-slate-200/60 text-slate-600",
                )}
              >
                {tab.count}
              </span>
            </button>
          ))}
        </div>

        <div className="relative w-full sm:w-72">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Buscar producto, SKU, motivo..."
            className="h-10 w-full rounded-xl border border-slate-200 bg-white pl-9 pr-4 text-sm text-slate-900 outline-none transition-colors placeholder:text-slate-400 focus:border-brand focus:ring-2 focus:ring-brand/20"
          />
        </div>
      </div>

      {/* Tabla de ajustes */}
      <div className="overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-sm">
        {loading ? (
          <div className="flex h-64 items-center justify-center">
            <Loader2 className="size-8 animate-spin text-brand" />
          </div>
        ) : error ? (
          <div className="flex h-64 flex-col items-center justify-center p-6 text-center">
            <AlertCircle className="size-10 text-rose-500" />
            <p className="mt-2 font-medium text-slate-900">
              Error al cargar los ajustes
            </p>
            <p className="text-sm text-slate-500">{friendlyError(error)}</p>
          </div>
        ) : filteredAdjustments.length === 0 ? (
          <div className="flex h-64 flex-col items-center justify-center p-6 text-center">
            <Filter className="size-10 text-slate-300" />
            <p className="mt-2 font-semibold text-slate-700">
              No se encontraron ajustes
            </p>
            <p className="text-sm text-slate-500">
              {searchTerm
                ? "Intenta con otro término de búsqueda"
                : "No hay ajustes registrados en este estado"}
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600">
              <thead className="border-b border-slate-200 bg-slate-50/75 text-xs font-semibold uppercase tracking-wider text-slate-500">
                <tr>
                  <th className="px-6 py-4">Fecha</th>
                  <th className="px-6 py-4">Producto</th>
                  <th className="px-6 py-4">Bodega / Lote</th>
                  <th className="px-6 py-4 text-right">Cantidad (Delta)</th>
                  <th className="px-6 py-4">Motivo</th>
                  <th className="px-6 py-4 text-right">Valor estimado</th>
                  <th className="px-6 py-4">Estado</th>
                  <th className="px-6 py-4">Solicitado por</th>
                  <th className="px-6 py-4 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filteredAdjustments.map((a) => {
                  const deltaNum = Number(a.qty_delta);
                  const isNegative = deltaNum < 0;

                  return (
                    <tr
                      key={a.id}
                      className="transition-colors hover:bg-slate-50/50"
                    >
                      <td className="whitespace-nowrap px-6 py-4 font-mono text-xs text-slate-500">
                        {formatDateTime(a.created_at)}
                      </td>
                      <td className="px-6 py-4">
                        <div className="font-semibold text-slate-900">
                          {a.product_name ?? "Sin nombre"}
                        </div>
                        <div className="font-mono text-xs text-slate-400">
                          {a.sku ?? "Sin SKU"}
                        </div>
                      </td>
                      <td className="px-6 py-4">
                        <div className="text-slate-800">
                          {a.location_name ?? "Bodega principal"}
                        </div>
                        {a.lot_code ? (
                          <span className="inline-flex items-center rounded-md bg-purple-50 px-2 py-0.5 font-mono text-xs font-medium text-purple-700">
                            Lote: {a.lot_code}
                          </span>
                        ) : (
                          <span className="text-xs text-slate-400">
                            Sin lote
                          </span>
                        )}
                      </td>
                      <td className="whitespace-nowrap px-6 py-4 text-right font-mono font-bold">
                        <span
                          className={cn(
                            "rounded-full px-2.5 py-1 text-xs",
                            isNegative
                              ? "bg-rose-50 text-rose-700"
                              : "bg-emerald-50 text-emerald-700",
                          )}
                        >
                          {deltaNum > 0 ? "+" : ""}
                          {formatQty(a.qty_delta)}
                        </span>
                      </td>
                      <td className="px-6 py-4">
                        <span className="inline-flex rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-1 text-xs font-medium text-slate-700">
                          {REASON_LABELS[a.reason_code] ?? a.reason_code}
                        </span>
                        {a.notes && (
                          <p className="mt-1 max-w-xs truncate text-xs text-slate-500" title={a.notes}>
                            {a.notes}
                          </p>
                        )}
                      </td>
                      <td className="whitespace-nowrap px-6 py-4 text-right font-mono font-medium text-slate-900">
                        {formatCOP(Number(a.value))}
                      </td>
                      <td className="whitespace-nowrap px-6 py-4">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-semibold",
                            a.status === "PENDIENTE" &&
                              "bg-amber-50 text-amber-700",
                            a.status === "APLICADO" &&
                              "bg-emerald-50 text-emerald-700",
                            a.status === "RECHAZADO" &&
                              "bg-rose-50 text-rose-700",
                          )}
                        >
                          {a.status === "PENDIENTE" && (
                            <Clock className="size-3" />
                          )}
                          {a.status === "APLICADO" && (
                            <CheckCircle2 className="size-3" />
                          )}
                          {a.status === "RECHAZADO" && (
                            <XCircle className="size-3" />
                          )}
                          {a.status}
                        </span>
                        {a.reject_reason && (
                          <p className="mt-1 max-w-xs truncate text-xs text-rose-600" title={a.reject_reason}>
                            Motivo: {a.reject_reason}
                          </p>
                        )}
                      </td>
                      <td className="px-6 py-4">
                        <div className="text-slate-800">
                          {a.requester_name ?? "Usuario"}
                        </div>
                        {a.decider_name && (
                          <div className="text-xs text-slate-400">
                            Decidió: {a.decider_name}
                          </div>
                        )}
                      </td>
                      <td className="whitespace-nowrap px-6 py-4 text-right">
                        {a.status === "PENDIENTE" && canApprove ? (
                          <div className="flex items-center justify-end gap-2">
                            <button
                              onClick={() => setSelectedForApprove(a)}
                              className="rounded-lg bg-emerald-50 px-3 py-1.5 text-xs font-semibold text-emerald-700 transition-colors hover:bg-emerald-100"
                            >
                              Aprobar
                            </button>
                            <button
                              onClick={() => {
                                setSelectedForReject(a);
                                setRejectReason("");
                              }}
                              className="rounded-lg bg-rose-50 px-3 py-1.5 text-xs font-semibold text-rose-700 transition-colors hover:bg-rose-100"
                            >
                              Rechazar
                            </button>
                          </div>
                        ) : (
                          <span className="text-xs text-slate-400">—</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Modal de Aprobación */}
      <Dialog
        open={!!selectedForApprove}
        onOpenChange={(open) => !open && setSelectedForApprove(null)}
      >
        <DialogContent className="max-w-md rounded-3xl p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-slate-900">
              Aprobar ajuste de inventario
            </DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              Al aprobar, se aplicará el movimiento en el kardex y se
              actualizará el stock disponible.
            </DialogDescription>
          </DialogHeader>

          {selectedForApprove && (
            <div className="my-4 rounded-2xl border border-slate-100 bg-slate-50 p-4 text-sm space-y-2">
              <div className="flex justify-between">
                <span className="text-slate-500">Producto:</span>
                <span className="font-semibold text-slate-900">
                  {selectedForApprove.product_name}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Delta / Cantidad:</span>
                <span className="font-mono font-bold text-slate-900">
                  {formatQty(selectedForApprove.qty_delta)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Valor estimado:</span>
                <span className="font-mono font-semibold text-slate-900">
                  {formatCOP(Number(selectedForApprove.value))}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Motivo:</span>
                <span className="text-slate-900">
                  {REASON_LABELS[selectedForApprove.reason_code] ??
                    selectedForApprove.reason_code}
                </span>
              </div>
            </div>
          )}

          <DialogFooter className="mt-4 gap-2 sm:gap-0">
            <button
              onClick={() => setSelectedForApprove(null)}
              disabled={isProcessing}
              className="h-10 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50 disabled:opacity-50"
            >
              Cancelar
            </button>
            <button
              onClick={handleApprove}
              disabled={isProcessing}
              className="flex h-10 items-center justify-center gap-2 rounded-xl bg-emerald-600 px-5 text-sm font-semibold text-white transition-colors hover:bg-emerald-700 disabled:opacity-50"
            >
              {isProcessing && <Loader2 className="size-4 animate-spin" />}
              Confirmar aprobación
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Modal de Rechazo */}
      <Dialog
        open={!!selectedForReject}
        onOpenChange={(open) => !open && setSelectedForReject(null)}
      >
        <DialogContent className="max-w-md rounded-3xl p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-slate-900">
              Rechazar ajuste de inventario
            </DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              El ajuste quedará marcado como rechazado y no afectará las
              existencias.
            </DialogDescription>
          </DialogHeader>

          <div className="my-4 space-y-3">
            <label
              htmlFor="reject-reason"
              className="block text-sm font-medium text-slate-700"
            >
              Motivo del rechazo <span className="text-rose-500">*</span>
            </label>
            <textarea
              id="reject-reason"
              rows={3}
              value={rejectReason}
              onChange={(e) => setRejectReason(e.target.value)}
              placeholder="Indica el motivo por el cual no se aprueba este ajuste..."
              className="w-full rounded-2xl border border-slate-200 p-3 text-sm outline-none focus:border-brand focus:ring-2 focus:ring-brand/20"
            />
          </div>

          <DialogFooter className="mt-4 gap-2 sm:gap-0">
            <button
              onClick={() => setSelectedForReject(null)}
              disabled={isProcessing}
              className="h-10 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50 disabled:opacity-50"
            >
              Cancelar
            </button>
            <button
              onClick={handleReject}
              disabled={isProcessing || !rejectReason.trim()}
              className="flex h-10 items-center justify-center gap-2 rounded-xl bg-rose-600 px-5 text-sm font-semibold text-white transition-colors hover:bg-rose-700 disabled:opacity-50"
            >
              {isProcessing && <Loader2 className="size-4 animate-spin" />}
              Rechazar ajuste
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

"use client";

import { useMemo, useState } from "react";
import {
  AlertTriangle,
  Calendar,
  CheckCircle2,
  Clock,
  Loader2,
  PackageX,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api, friendlyError, type ExpiringLot } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type ExpiryTab = "vencidos" | "7d" | "15d" | "30d";

export function ExpiringLotsScreen() {
  const currentBranch = useCurrentBranch();
  const branchId = currentBranch?.id;

  const [activeTab, setActiveTab] = useState<ExpiryTab>("vencidos");
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedLotForDiscard, setSelectedLotForDiscard] =
    useState<ExpiringLot | null>(null);
  const [isDiscarding, setIsDiscarding] = useState(false);
  const [discardSuccess, setDiscardSuccess] = useState<string | null>(null);
  const [discardError, setDiscardError] = useState<string | null>(null);

  // Carga de lotes por vencer hasta 30 días
  const {
    data: lots = [],
    loading,
    error,
    reload,
  } = useApi(() => api.inventory.expiring(branchId, 30), `expiring-${branchId}`);

  // Filtrado y cómputos
  const counts = useMemo(() => {
    let expired = 0;
    let in7 = 0;
    let in15 = 0;
    let in30 = 0;
    let totalRiskQty = 0;
    const safeLots = lots ?? [];

    for (const lot of safeLots) {
      const qtyNum = Number(lot.qty);
      totalRiskQty += qtyNum;
      if (lot.is_expired || lot.days_until_expiry < 0) {
        expired += 1;
      } else if (lot.days_until_expiry <= 7) {
        in7 += 1;
      } else if (lot.days_until_expiry <= 15) {
        in15 += 1;
      } else {
        in30 += 1;
      }
    }

    return {
      expired,
      in7,
      in15,
      in30: expired + in7 + in15 + in30,
      totalRiskQty,
    };
  }, [lots]);

  const filteredLots = useMemo(() => {
    const safeLots = lots ?? [];
    return safeLots.filter((lot) => {
      // Filtro de pestaña
      if (activeTab === "vencidos") {
        if (!lot.is_expired && lot.days_until_expiry >= 0) return false;
      } else if (activeTab === "7d") {
        if (lot.is_expired || lot.days_until_expiry < 0 || lot.days_until_expiry > 7)
          return false;
      } else if (activeTab === "15d") {
        if (lot.is_expired || lot.days_until_expiry < 0 || lot.days_until_expiry > 15)
          return false;
      }
      // Pestaña "30d" muestra todos

      // Filtro de búsqueda
      if (searchTerm.trim()) {
        const q = searchTerm.toLowerCase();
        const pMatch = lot.product_name.toLowerCase().includes(q);
        const skuMatch = lot.sku?.toLowerCase().includes(q) ?? false;
        const codeMatch = lot.code.toLowerCase().includes(q);
        const locMatch = lot.location_name.toLowerCase().includes(q);
        return pMatch || skuMatch || codeMatch || locMatch;
      }

      return true;
    });
  }, [lots, activeTab, searchTerm]);

  // Manejo de dar de baja
  async function handleConfirmDiscard() {
    if (!selectedLotForDiscard || !branchId) return;
    setIsDiscarding(true);
    setDiscardError(null);
    try {
      await api.inventory.adjust(
        branchId,
        selectedLotForDiscard.product_id,
        (-Number(selectedLotForDiscard.qty)).toFixed(4),
        `Baja por VENCIMIENTO de lote ${selectedLotForDiscard.code}`,
      );
      setDiscardSuccess(
        `Se dio de baja el lote ${selectedLotForDiscard.code} (${selectedLotForDiscard.qty} unidades).`,
      );
      setSelectedLotForDiscard(null);
      await reload();
      setTimeout(() => setDiscardSuccess(null), 5000);
    } catch (err) {
      setDiscardError(friendlyError(err));
    } finally {
      setIsDiscarding(false);
    }
  }

  return (
    <div className="space-y-6">
      {/* Encabezado */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900">
            Control de Vencimientos
          </h1>
          <p className="text-sm text-slate-500">
            Monitoreo y despacho FEFO de lotes con fecha de caducidad.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => reload()}
            disabled={loading}
            className="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 shadow-sm hover:bg-slate-50 disabled:opacity-50"
          >
            <RefreshCw
              className={cn("h-4 w-4 text-slate-500", loading && "animate-spin")}
            />
            Actualizar
          </button>
        </div>
      </div>

      {/* Alertas globales */}
      {discardSuccess && (
        <div className="flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 p-4 text-sm text-emerald-800">
          <CheckCircle2 className="h-5 w-5 flex-shrink-0 text-emerald-600" />
          <span>{discardSuccess}</span>
        </div>
      )}
      {discardError && (
        <div className="flex items-center gap-2 rounded-lg border border-rose-200 bg-rose-50 p-4 text-sm text-rose-800">
          <AlertTriangle className="h-5 w-5 flex-shrink-0 text-rose-600" />
          <span>{discardError}</span>
        </div>
      )}

      {/* Tarjetas de Métricas */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Lotes vencidos"
          value={counts.expired.toString()}
          caption="Exige retiro o baja inmediata"
          icon={PackageX}
        />
        <StatCard
          title="Vence en ≤ 7 días"
          value={counts.in7.toString()}
          caption="Prioridad FEFO de venta"
          icon={AlertTriangle}
        />
        <StatCard
          title="Vence en ≤ 30 días"
          value={counts.in30.toString()}
          caption="Monitoreo de rotación"
          icon={Clock}
        />
        <StatCard
          title="Unidades en riesgo"
          value={counts.totalRiskQty.toLocaleString("es-CO")}
          caption="Suma total de existencias"
          icon={Calendar}
        />
      </div>

      {/* Pestañas y Buscador */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-1 rounded-lg border border-slate-200 bg-slate-100 p-1">
          <button
            onClick={() => setActiveTab("vencidos")}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              activeTab === "vencidos"
                ? "bg-white text-rose-700 shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <span>Vencidos</span>
            {counts.expired > 0 && (
              <span className="rounded-full bg-rose-100 px-1.5 py-0.5 text-xs font-semibold text-rose-700">
                {counts.expired}
              </span>
            )}
          </button>
          <button
            onClick={() => setActiveTab("7d")}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              activeTab === "7d"
                ? "bg-white text-amber-700 shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <span>7 días</span>
            {counts.in7 > 0 && (
              <span className="rounded-full bg-amber-100 px-1.5 py-0.5 text-xs font-semibold text-amber-700">
                {counts.in7}
              </span>
            )}
          </button>
          <button
            onClick={() => setActiveTab("15d")}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              activeTab === "15d"
                ? "bg-white text-slate-900 shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <span>15 días</span>
            {counts.in15 > 0 && (
              <span className="rounded-full bg-slate-200 px-1.5 py-0.5 text-xs font-semibold text-slate-700">
                {counts.in15}
              </span>
            )}
          </button>
          <button
            onClick={() => setActiveTab("30d")}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              activeTab === "30d"
                ? "bg-white text-slate-900 shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <span>30 días</span>
            <span className="rounded-full bg-slate-200 px-1.5 py-0.5 text-xs font-semibold text-slate-700">
              {counts.in30}
            </span>
          </button>
        </div>

        {/* Buscador */}
        <div className="relative w-full sm:w-72">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            placeholder="Buscar producto, lote o bodega..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full rounded-lg border border-slate-300 bg-white py-2 pl-9 pr-4 text-sm text-slate-900 placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </div>
      </div>

      {/* Tabla de Lotes */}
      <div className="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-sm">
        {loading ? (
          <div className="flex h-64 items-center justify-center">
            <Loader2 className="h-8 w-8 animate-spin text-slate-400" />
          </div>
        ) : error ? (
          <div className="p-8 text-center text-sm text-rose-600">
            Ocurrió un error al cargar los lotes: {friendlyError(error)}
          </div>
        ) : filteredLots.length === 0 ? (
          <div className="p-12 text-center">
            <CheckCircle2 className="mx-auto h-12 w-12 text-emerald-500" />
            <h3 className="mt-4 text-base font-semibold text-slate-900">
              Sin lotes en este rango
            </h3>
            <p className="mt-1 text-sm text-slate-500">
              No hay productos con existencias en riesgo para el filtro seleccionado.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600">
              <thead className="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500">
                <tr>
                  <th className="px-6 py-3">Producto</th>
                  <th className="px-6 py-3">Lote</th>
                  <th className="px-6 py-3">Bodega</th>
                  <th className="px-6 py-3 text-right">Cantidad</th>
                  <th className="px-6 py-3">Vence</th>
                  <th className="px-6 py-3">Estado</th>
                  <th className="px-6 py-3 text-right">Acción</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200">
                {filteredLots.map((lot) => {
                  const isExpired = lot.is_expired || lot.days_until_expiry < 0;
                  const isCritical =
                    !isExpired && lot.days_until_expiry <= 7;

                  return (
                    <tr key={lot.id} className="hover:bg-slate-50/70">
                      <td className="px-6 py-4">
                        <div className="font-medium text-slate-900">
                          {lot.product_name}
                        </div>
                        {lot.sku && (
                          <div className="text-xs text-slate-400">
                            SKU: {lot.sku}
                          </div>
                        )}
                      </td>
                      <td className="px-6 py-4 font-mono text-xs font-semibold text-slate-800">
                        {lot.code}
                      </td>
                      <td className="px-6 py-4 text-slate-600">
                        {lot.location_name}
                      </td>
                      <td className="px-6 py-4 text-right font-medium text-slate-900">
                        {Number(lot.qty).toLocaleString("es-CO")}
                      </td>
                      <td className="px-6 py-4 text-slate-700">
                        {lot.expires_at}
                      </td>
                      <td className="px-6 py-4">
                        {isExpired ? (
                          <span className="inline-flex items-center gap-1 rounded-full bg-rose-100 px-2.5 py-0.5 text-xs font-medium text-rose-700">
                            Vencido ({Math.abs(lot.days_until_expiry)} d)
                          </span>
                        ) : isCritical ? (
                          <span className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-2.5 py-0.5 text-xs font-medium text-amber-700">
                            En {lot.days_until_expiry} días
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-700">
                            En {lot.days_until_expiry} días
                          </span>
                        )}
                      </td>
                      <td className="px-6 py-4 text-right">
                        <button
                          onClick={() => setSelectedLotForDiscard(lot)}
                          className="inline-flex items-center gap-1 rounded-md border border-rose-200 bg-white px-2.5 py-1 text-xs font-medium text-rose-700 shadow-sm hover:bg-rose-50"
                        >
                          <Trash2 className="h-3.5 w-3.5 text-rose-600" />
                          Dar de baja
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Diálogo de Confirmación: Dar de baja */}
      <Dialog
        open={!!selectedLotForDiscard}
        onOpenChange={(open) => !open && setSelectedLotForDiscard(null)}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-rose-600">
              <AlertTriangle className="h-5 w-5" />
              Dar de baja lote vencido
            </DialogTitle>
            <DialogDescription>
              Esta acción registrará un ajuste de inventario por motivo de{" "}
              <strong>VENCIMIENTO</strong> descontando la totalidad del lote.
            </DialogDescription>
          </DialogHeader>

          {selectedLotForDiscard && (
            <div className="space-y-4 py-2">
              <div className="rounded-lg bg-slate-50 p-3 text-sm space-y-1 border border-slate-200">
                <div className="text-slate-500">
                  Producto:{" "}
                  <span className="font-medium text-slate-900">
                    {selectedLotForDiscard.product_name}
                  </span>
                </div>
                <div className="text-slate-500">
                  Código de lote:{" "}
                  <span className="font-mono font-medium text-slate-900">
                    {selectedLotForDiscard.code}
                  </span>
                </div>
                <div className="text-slate-500">
                  Bodega:{" "}
                  <span className="font-medium text-slate-900">
                    {selectedLotForDiscard.location_name}
                  </span>
                </div>
                <div className="text-slate-500">
                  Cantidad a dar de baja:{" "}
                  <span className="font-semibold text-rose-600">
                    {selectedLotForDiscard.qty} unidades
                  </span>
                </div>
                <div className="text-slate-500">
                  Fecha de vencimiento:{" "}
                  <span className="font-medium text-slate-900">
                    {selectedLotForDiscard.expires_at}
                  </span>
                </div>
              </div>

              {discardError && (
                <div className="rounded-md bg-rose-50 p-3 text-xs text-rose-700">
                  {discardError}
                </div>
              )}

              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setSelectedLotForDiscard(null)}
                  disabled={isDiscarding}
                  className="rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  onClick={handleConfirmDiscard}
                  disabled={isDiscarding}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-rose-600 px-4 py-2 text-sm font-medium text-white hover:bg-rose-700 disabled:opacity-50"
                >
                  {isDiscarding && (
                    <Loader2 className="h-4 w-4 animate-spin text-white" />
                  )}
                  Confirmar baja
                </button>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

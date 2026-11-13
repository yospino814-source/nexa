"use client";

import { useMemo, useState } from "react";
import { RefreshCw, Search } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { ExportCsvButton, formatIsoDisplay, localTodayIso } from "@/components/ventas/report-controls";
import { api, friendlyError, type ReportQuery } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatQty } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { Boxes, Clock3, Warehouse } from "lucide-react";

/** Pestaña "Reportes" dentro de /inventario: valorización y rotación (F7.3 / S7). */
export function InventoryReportsTab() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);

  const [today] = useState(localTodayIso);
  const [toDate, setToDate] = useState(today);
  const [allBranches, setAllBranches] = useState(false);
  const [search, setSearch] = useState("");

  const query = useMemo<ReportQuery>(
    () => ({
      branchId: allBranches && branches.length > 1 ? null : currentBranch.id,
      to: toDate || today,
    }),
    [allBranches, branches.length, currentBranch.id, toDate, today],
  );

  const queryKey = JSON.stringify(query);
  const { data, loading, error, reload } = useApi(async () => {
    const [valuation, rotation] = await Promise.all([
      api.reports.inventoryValuation(query),
      api.reports.inventoryRotation(query),
    ]);
    return { valuation, rotation };
  }, queryKey);

  const valuationRows = useMemo(() => {
    const q = search.trim().toLowerCase();
    const rows = data?.valuation.rows ?? [];
    if (!q) return rows;
    return rows.filter(
      (r) =>
        r.producto.toLowerCase().includes(q) ||
        r.ubicacion.toLowerCase().includes(q) ||
        r.sucursal.toLowerCase().includes(q),
    );
  }, [data, search]);

  const rotationRows = useMemo(() => {
    const q = search.trim().toLowerCase();
    const rows = data?.rotation.rows ?? [];
    if (!q) return rows;
    return rows.filter((r) => r.producto.toLowerCase().includes(q));
  }, [data, search]);

  if (error && !data) {
    return (
      <div className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700">
        <p>{friendlyError(error)}</p>
        <button
          type="button"
          onClick={reload}
          className="mt-2 inline-flex items-center gap-1.5 font-semibold underline"
        >
          <RefreshCw className="size-3.5" /> Reintentar
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm">
        <div className="flex flex-wrap items-center gap-3">
          <label className="flex h-10 min-w-[220px] items-center gap-2 rounded-full border border-slate-200 px-3.5">
            <Search className="size-4 text-slate-400" />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filtrar por producto o ubicación…"
              className="w-full bg-transparent text-sm outline-none"
            />
          </label>

          {branches.length > 1 && (
            <select
              aria-label="Alcance de sucursales"
              value={allBranches ? "ALL" : "CURRENT"}
              onChange={(e) => setAllBranches(e.target.value === "ALL")}
              className="h-10 rounded-full border border-slate-200 bg-white px-4 text-xs font-medium text-slate-700"
            >
              <option value="CURRENT">Sucursal actual ({currentBranch.name})</option>
              <option value="ALL">Todas las sucursales</option>
            </select>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-2 text-xs font-medium text-slate-500">
            <span>Corte rotación (hasta):</span>
            <input
              type="date"
              value={toDate}
              onChange={(e) => setToDate(e.target.value)}
              className="h-10 rounded-xl border border-slate-200 bg-white px-3 text-xs text-slate-800"
            />
          </label>
          {loading && <span className="text-xs text-slate-400">Actualizando…</span>}
        </div>
      </div>

      {data && (
        <div className="grid gap-5 md:grid-cols-3">
          <StatCard
            title="Valorización total"
            value={formatCOP(Number(data.valuation.total_valor))}
            caption={`${data.valuation.rows.length} registros con existencia distinta de cero`}
            icon={Warehouse}
            tone="success"
          />
          <StatCard
            title="Productos evaluados (30 d)"
            value={`${data.rotation.rows.length}`}
            caption={`Ventana del ${formatIsoDisplay(data.rotation.period.from)} al ${formatIsoDisplay(data.rotation.period.to)}`}
            icon={Boxes}
          />
          <StatCard
            title="Sin ventas últimos 30 d"
            value={`${data.rotation.rows.filter((r) => Number(r.vendidas_30d) <= 0).length}`}
            caption="Días de inventario sin rotación"
            icon={Clock3}
            tone="warning"
          />
        </div>
      )}

      {/* 1. Valorización de inventario */}
      <section className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
          <div>
            <h2 className="text-lg font-semibold text-slate-900">Valorización de inventario</h2>
            <p className="text-xs text-slate-500">
              Existencias por producto y ubicación multiplicadas por su costo promedio ponderado
            </p>
          </div>
          <ExportCsvButton
            slug="inventory-valuation"
            query={query}
            disabled={!data || data.valuation.rows.length === 0}
          />
        </div>

        {!data && loading ? (
          <div className="p-6">
            <div className="h-44 animate-pulse rounded-xl bg-slate-100" />
          </div>
        ) : valuationRows.length === 0 ? (
          <p className="py-10 text-center text-sm text-slate-400">
            No hay ubicaciones con existencias distintas de cero para mostrar.
          </p>
        ) : (
          <div className="overflow-x-auto p-4 sm:p-6">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                  {allBranches && branches.length > 1 && <th className="py-2.5 pr-4">Sucursal</th>}
                  <th className="py-2.5 pr-4">Producto</th>
                  <th className="py-2.5 pr-4">Ubicación</th>
                  <th className="py-2.5 pr-4 text-right">Existencias</th>
                  <th className="py-2.5 pr-4 text-right">Costo promedio</th>
                  <th className="py-2.5 text-right">Valor</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {valuationRows.map((r) => (
                  <tr key={`${r.product_id}-${r.location_id}`} className="hover:bg-slate-50/70">
                    {allBranches && branches.length > 1 && (
                      <td className="py-3 pr-4 text-xs text-slate-500">{r.sucursal}</td>
                    )}
                    <td className="py-3 pr-4 font-semibold text-slate-900">{r.producto}</td>
                    <td className="py-3 pr-4 text-slate-600">{r.ubicacion}</td>
                    <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{formatQty(r.existencias)}</td>
                    <td className="py-3 pr-4 text-right text-slate-600 tabular-nums">
                      {formatCOP(Number(r.costo_promedio))}
                    </td>
                    <td className="py-3 text-right font-semibold text-slate-900 tabular-nums">
                      {formatCOP(Number(r.valor))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* 2. Rotación de inventario */}
      <section className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
          <div>
            <h2 className="text-lg font-semibold text-slate-900">Rotación de inventario (30 días)</h2>
            <p className="text-xs text-slate-500">
              {data
                ? `Ventana de los últimos 30 días (${formatIsoDisplay(data.rotation.period.from)} al ${formatIsoDisplay(data.rotation.period.to)}) · Días = existencias / (vendidas 30d / 30)`
                : "Últimos 30 días hasta la fecha de corte"}
            </p>
          </div>
          <ExportCsvButton
            slug="inventory-rotation"
            query={query}
            disabled={!data || data.rotation.rows.length === 0}
          />
        </div>

        {!data && loading ? (
          <div className="p-6">
            <div className="h-44 animate-pulse rounded-xl bg-slate-100" />
          </div>
        ) : rotationRows.length === 0 ? (
          <p className="py-10 text-center text-sm text-slate-400">No hay productos para mostrar en rotación.</p>
        ) : (
          <div className="overflow-x-auto p-4 sm:p-6">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                  <th className="py-2.5 pr-4">Producto</th>
                  <th className="py-2.5 pr-4 text-right">Vendidas (30 d)</th>
                  <th className="py-2.5 pr-4 text-right">Existencias</th>
                  <th className="py-2.5 text-right">Días de inventario</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {rotationRows.map((r) => (
                  <tr key={r.product_id} className="hover:bg-slate-50/70">
                    <td className="py-3 pr-4 font-semibold text-slate-900">{r.producto}</td>
                    <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{formatQty(r.vendidas_30d)}</td>
                    <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{formatQty(r.existencias)}</td>
                    <td className="py-3 text-right font-semibold text-slate-900 tabular-nums">
                      {r.dias_inventario != null ? `${Number(r.dias_inventario).toFixed(1)} días` : "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}


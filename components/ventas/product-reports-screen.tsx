"use client";

import { useMemo, useState } from "react";
import { AlertTriangle, ArrowDown, ArrowUp, ArrowUpDown, Award, Banknote, Package, RefreshCw, Search } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import {
  CostMissingWarning,
  ExportCsvButton,
  localTodayIso,
  rangeForReportPreset,
  ReportFilterBar,
  type ReportPeriodPreset,
  SalesReportNav,
} from "@/components/ventas/report-controls";
import { api, friendlyError, type ReportQuery } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatPercent, formatQty } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type SortColumn = "producto" | "categoria" | "cantidad" | "venta_sin_impuesto" | "costo" | "margen" | "pct_margen";
type SortDirection = "asc" | "desc";

function marginClass(pct: number): string {
  if (pct < 15) return "text-rose-600";
  if (pct < 30) return "text-amber-600";
  return "text-emerald-600";
}

function SortIcon({ active, dir }: { active: boolean; dir: SortDirection }) {
  if (!active) return <ArrowUpDown className="size-3.5 text-slate-300" />;
  return dir === "asc" ? <ArrowUp className="size-3.5 text-teal-700" /> : <ArrowDown className="size-3.5 text-teal-700" />;
}

/** Pantalla 03: ventas por producto y por categoría con exportación CSV. */
export function ProductReportsScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);

  const [today] = useState(localTodayIso);
  const [preset, setPreset] = useState<ReportPeriodPreset>("SIETE_DIAS");
  const [customFrom, setCustomFrom] = useState(today);
  const [customTo, setCustomTo] = useState(today);
  const [allBranches, setAllBranches] = useState(false);

  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("");
  const [sortCol, setSortCol] = useState<SortColumn>("venta_sin_impuesto");
  const [sortDir, setSortDir] = useState<SortDirection>("desc");

  const query = useMemo<ReportQuery>(() => {
    const branchId = allBranches && branches.length > 1 ? null : currentBranch.id;
    if (preset === "PERSONALIZADO") {
      const valid = customFrom && customTo && customFrom <= customTo;
      return {
        branchId,
        from: valid ? customFrom : today,
        to: valid ? customTo : today,
      };
    }
    const r = rangeForReportPreset(preset, today);
    return { branchId, from: r.from, to: r.to };
  }, [allBranches, branches.length, currentBranch.id, customFrom, customTo, preset, today]);

  const queryKey = JSON.stringify(query);
  const { data, loading, error, reload } = useApi(async () => {
    const [byProduct, byCategory] = await Promise.all([
      api.reports.salesByProduct(query),
      api.reports.salesByCategory(query),
    ]);
    return { byProduct, byCategory };
  }, queryKey);

  const categories = useMemo(() => {
    const rows = data?.byProduct.rows ?? [];
    return [...new Set(rows.map((r) => r.categoria))].sort((a, b) => a.localeCompare(b, "es"));
  }, [data]);

  const filteredProducts = useMemo(() => {
    const rows = data?.byProduct.rows ?? [];
    const q = search.trim().toLowerCase();
    const list = rows.filter((r) => {
      const matchSearch =
        !q || r.producto.toLowerCase().includes(q) || (r.sku?.toLowerCase().includes(q) ?? false);
      const matchCat = !categoryFilter || r.categoria === categoryFilter;
      return matchSearch && matchCat;
    });

    const factor = sortDir === "asc" ? 1 : -1;
    return [...list].sort((a, b) => {
      if (sortCol === "producto") return a.producto.localeCompare(b.producto, "es") * factor;
      if (sortCol === "categoria") return a.categoria.localeCompare(b.categoria, "es") * factor;
      return (Number(a[sortCol]) - Number(b[sortCol])) * factor;
    });
  }, [data, search, categoryFilter, sortCol, sortDir]);

  const highlights = useMemo(() => {
    const rows = data?.byProduct.rows ?? [];
    if (rows.length === 0) return null;
    const leader = [...rows].sort((a, b) => Number(b.venta_sin_impuesto) - Number(a.venta_sin_impuesto))[0];
    const lowestMargin = [...rows].sort((a, b) => Number(a.pct_margen) - Number(b.pct_margen))[0];
    const totalNet = rows.reduce((s, r) => s + Number(r.venta_sin_impuesto), 0);
    const totalCost = rows.reduce((s, r) => s + Number(r.costo), 0);
    const totalMargin = rows.reduce((s, r) => s + Number(r.margen), 0);
    const totalQty = rows.reduce((s, r) => s + Number(r.cantidad), 0);
    const avgMarginPct = totalNet > 0 ? (totalMargin / totalNet) * 100 : 0;
    return { leader, lowestMargin, totalNet, totalCost, totalMargin, totalQty, avgMarginPct };
  }, [data]);

  const categoryTotalNet = useMemo(
    () => (data?.byCategory.rows ?? []).reduce((s, r) => s + Number(r.venta_sin_impuesto), 0),
    [data],
  );

  function toggleSort(col: SortColumn) {
    if (sortCol === col) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortCol(col);
      setSortDir(col === "producto" || col === "categoria" ? "asc" : "desc");
    }
  }

  if (error && !data) {
    return (
      <div className="p-6 lg:p-10">
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
      </div>
    );
  }

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight text-slate-900">Ventas por producto</h1>
          <p className="mt-1 text-sm text-slate-500">
            Rentabilidad por producto y resumen agrupado por categoría sin incluir impuestos.
          </p>
        </div>
        <SalesReportNav active="productos" />
      </div>

      <ReportFilterBar
        preset={preset}
        onPresetChange={setPreset}
        customFrom={customFrom}
        onCustomFromChange={setCustomFrom}
        customTo={customTo}
        onCustomToChange={setCustomTo}
        allBranches={allBranches}
        onAllBranchesChange={setAllBranches}
        period={data?.byProduct.period}
        loading={loading}
      />

      {error && (
        <div className="flex items-center justify-between gap-3 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          <span>{friendlyError(error)}</span>
          <button type="button" onClick={reload} className="font-semibold underline">
            Reintentar
          </button>
        </div>
      )}

      {data && <CostMissingWarning count={data.byProduct.cost_missing_lines} />}

      {loading && !data ? (
        <div className="space-y-4">
          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-36 animate-pulse rounded-2xl bg-slate-200/60" />
            ))}
          </div>
          <div className="h-96 animate-pulse rounded-2xl bg-slate-200/60" />
        </div>
      ) : (
        data && (
          <>
            {highlights && (
              <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
                <StatCard
                  title="Producto líder"
                  value={formatCOP(Number(highlights.leader.venta_sin_impuesto))}
                  caption={`${highlights.leader.producto} · ${formatQty(highlights.leader.cantidad)} uds.`}
                  icon={Award}
                  tone="success"
                />
                <StatCard
                  title="Menor rentabilidad"
                  value={formatPercent(Number(highlights.lowestMargin.pct_margen))}
                  caption={highlights.lowestMargin.producto}
                  icon={AlertTriangle}
                  tone={Number(highlights.lowestMargin.pct_margen) < 15 ? "danger" : "warning"}
                  badge={
                    Number(highlights.lowestMargin.pct_margen) < 15
                      ? { label: "Margen crítico", tone: "down" }
                      : undefined
                  }
                />
                <StatCard
                  title="Venta sin impuesto"
                  value={formatCOP(highlights.totalNet)}
                  caption={`${formatQty(highlights.totalQty)} unidades vendidas`}
                  icon={Package}
                />
                <StatCard
                  title="Margen bruto total"
                  value={formatCOP(highlights.totalMargin)}
                  caption={`${formatPercent(highlights.avgMarginPct)} sobre venta sin impuesto`}
                  icon={Banknote}
                  tone="success"
                />
              </div>
            )}

            {/* Tabla principal: Ventas por producto */}
            <section className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
                <div>
                  <h2 className="text-lg font-semibold text-slate-900">Detalle por producto</h2>
                  <p className="text-xs text-slate-500">
                    Hacé clic en un encabezado para ordenar · Costo calculado sobre líneas vendidas
                  </p>
                </div>
                <ExportCsvButton
                  slug="sales-by-product"
                  query={query}
                  disabled={data.byProduct.rows.length === 0}
                />
              </div>

              <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 px-4 py-3 sm:px-6">
                <label className="flex h-10 min-w-[220px] flex-1 items-center gap-2 rounded-full border border-slate-200 px-3.5">
                  <Search className="size-4 text-slate-400" />
                  <input
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Buscar producto o SKU…"
                    className="w-full bg-transparent text-sm outline-none"
                  />
                </label>

                <select
                  aria-label="Filtrar por categoría"
                  value={categoryFilter}
                  onChange={(e) => setCategoryFilter(e.target.value)}
                  className="h-10 rounded-full border border-slate-200 bg-white px-4 text-xs font-medium text-slate-700"
                >
                  <option value="">Todas las categorías</option>
                  {categories.map((cat) => (
                    <option key={cat} value={cat}>
                      {cat}
                    </option>
                  ))}
                </select>
              </div>

              {filteredProducts.length === 0 ? (
                <p className="py-12 text-center text-sm text-slate-400">
                  {data.byProduct.rows.length
                    ? "Ningún producto coincide con los filtros actuales."
                    : "No hay ventas por producto en este periodo."}
                </p>
              ) : (
                <div className="overflow-x-auto p-4 sm:p-6">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                        <th className="py-2.5 pr-3">SKU</th>
                        <th className="py-2.5 pr-4">
                          <button
                            type="button"
                            onClick={() => toggleSort("producto")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Producto <SortIcon active={sortCol === "producto"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 pr-4">
                          <button
                            type="button"
                            onClick={() => toggleSort("categoria")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Categoría <SortIcon active={sortCol === "categoria"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 pr-4 text-right">
                          <button
                            type="button"
                            onClick={() => toggleSort("cantidad")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Cantidad <SortIcon active={sortCol === "cantidad"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 pr-4 text-right">
                          <button
                            type="button"
                            onClick={() => toggleSort("venta_sin_impuesto")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Venta sin imp. <SortIcon active={sortCol === "venta_sin_impuesto"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 pr-4 text-right">
                          <button
                            type="button"
                            onClick={() => toggleSort("costo")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Costo <SortIcon active={sortCol === "costo"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 pr-4 text-right">
                          <button
                            type="button"
                            onClick={() => toggleSort("margen")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            Margen <SortIcon active={sortCol === "margen"} dir={sortDir} />
                          </button>
                        </th>
                        <th className="py-2.5 text-right">
                          <button
                            type="button"
                            onClick={() => toggleSort("pct_margen")}
                            className="inline-flex items-center gap-1 hover:text-slate-700"
                          >
                            % Margen <SortIcon active={sortCol === "pct_margen"} dir={sortDir} />
                          </button>
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {filteredProducts.map((r, idx) => {
                        const pct = Number(r.pct_margen);
                        return (
                          <tr key={`${r.product_id ?? r.producto}-${idx}`} className="hover:bg-slate-50/70">
                            <td className="py-3 pr-3 font-mono text-xs text-slate-400">{r.sku ?? "—"}</td>
                            <td className="py-3 pr-4 font-semibold text-slate-900">{r.producto}</td>
                            <td className="py-3 pr-4">
                              <span className="rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600">
                                {r.categoria}
                              </span>
                            </td>
                            <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{formatQty(r.cantidad)}</td>
                            <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                              {formatCOP(Number(r.venta_sin_impuesto))}
                            </td>
                            <td className="py-3 pr-4 text-right text-slate-600 tabular-nums">
                              {formatCOP(Number(r.costo))}
                            </td>
                            <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                              {formatCOP(Number(r.margen))}
                            </td>
                            <td className={cn("py-3 text-right font-bold tabular-nums", marginClass(pct))}>
                              {formatPercent(pct)}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </section>

            {/* Tabla secundaria: Ventas por categoría */}
            <section className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
                <div>
                  <h2 className="text-lg font-semibold text-slate-900">Ventas por categoría</h2>
                  <p className="text-xs text-slate-500">Agrupación consolidada sin impuesto por línea</p>
                </div>
                <ExportCsvButton
                  slug="sales-by-category"
                  query={query}
                  disabled={data.byCategory.rows.length === 0}
                />
              </div>

              {data.byCategory.rows.length === 0 ? (
                <p className="py-10 text-center text-sm text-slate-400">
                  No hay ventas por categoría en este periodo.
                </p>
              ) : (
                <div className="overflow-x-auto p-4 sm:p-6">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                        <th className="py-2.5 pr-4">Categoría</th>
                        <th className="py-2.5 pr-4 text-right">Cantidad</th>
                        <th className="py-2.5 pr-4 text-right">Venta sin imp.</th>
                        <th className="py-2.5 pr-4 text-right">Participación</th>
                        <th className="py-2.5 pr-4 text-right">Margen</th>
                        <th className="py-2.5 text-right">% Margen</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {data.byCategory.rows.map((r) => {
                        const net = Number(r.venta_sin_impuesto);
                        const pct = Number(r.pct_margen);
                        const share = categoryTotalNet > 0 ? (net / categoryTotalNet) * 100 : 0;
                        return (
                          <tr key={r.categoria} className="hover:bg-slate-50/70">
                            <td className="py-3 pr-4 font-semibold text-slate-900">{r.categoria}</td>
                            <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{formatQty(r.cantidad)}</td>
                            <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                              {formatCOP(net)}
                            </td>
                            <td className="py-3 pr-4">
                              <div className="flex items-center justify-end gap-2">
                                <div className="h-2 w-24 overflow-hidden rounded-full bg-slate-100">
                                  <div
                                    className="h-full rounded-full bg-teal-600"
                                    style={{ width: `${Math.max(Math.round(share), net > 0 ? 4 : 0)}%` }}
                                  />
                                </div>
                                <span className="w-12 text-right text-xs text-slate-500 tabular-nums">
                                  {formatPercent(share)}
                                </span>
                              </div>
                            </td>
                            <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                              {formatCOP(Number(r.margen))}
                            </td>
                            <td className={cn("py-3 text-right font-bold tabular-nums", marginClass(pct))}>
                              {formatPercent(pct)}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </section>
          </>
        )
      )}
    </div>
  );
}


"use client";

import { useMemo, useState } from "react";
import { Banknote, Receipt, RefreshCw, RotateCcw, ShoppingBag, TrendingUp } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import {
  CostMissingWarning,
  ExportCsvButton,
  formatIsoDisplay,
  localTodayIso,
  rangeForReportPreset,
  ReportFilterBar,
  type ReportPeriodPreset,
  SalesReportNav,
} from "@/components/ventas/report-controls";
import { api, friendlyError, type ReportQuery, type SalesByDayRow } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatCOPShort, formatPercent } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type SalesTab = "DIA" | "CAJERO" | "METODO" | "HEATMAP";

const DAY_NAMES = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

const METHOD_STYLES: Record<string, { dot: string; bar: string }> = {
  EFECTIVO: { dot: "bg-emerald-500", bar: "bg-emerald-500" },
  QR: { dot: "bg-teal-600", bar: "bg-teal-600" },
  DATAFONO: { dot: "bg-slate-700", bar: "bg-slate-700" },
  CREDITO: { dot: "bg-amber-500", bar: "bg-amber-500" },
};

function shortDay(iso: string): string {
  const parts = iso.split("-");
  if (parts.length !== 3) return iso;
  return `${parts[2]}/${parts[1]}`;
}

function DailyBarChart({ rows }: { rows: SalesByDayRow[] }) {
  const maxGross = Math.max(...rows.map((r) => Number(r.venta_bruta)), 0);
  if (rows.length === 0 || maxGross <= 0) {
    return (
      <div className="flex h-48 items-center justify-center rounded-2xl border border-dashed border-slate-200 bg-slate-50/60 text-sm text-slate-400">
        Sin ventas registradas por día en este periodo.
      </div>
    );
  }

  const labelStep = rows.length > 20 ? 3 : rows.length > 12 ? 2 : 1;
  const needsScroll = rows.length > 16;

  return (
    <div className="overflow-x-auto pb-1">
      <div className="relative pt-4" style={needsScroll ? { minWidth: `${rows.length * 36}px` } : undefined}>
        <div className="pointer-events-none absolute inset-x-0 top-4 bottom-6 flex flex-col justify-between">
          {[1, 0.75, 0.5, 0.25].map((ratio) => (
            <div key={ratio} className="flex items-center border-b border-dashed border-slate-200/70">
              <span className="-translate-y-2 text-[10px] text-slate-400 tabular-nums">
                {formatCOPShort(maxGross * ratio)}
              </span>
            </div>
          ))}
        </div>

        <div className="relative z-10 flex h-44 items-end gap-2 pl-12" role="img" aria-label="Venta bruta por día">
          {rows.map((row, idx) => {
            const gross = Number(row.venta_bruta);
            const pct = maxGross > 0 ? Math.max(Math.round((gross / maxGross) * 100), gross > 0 ? 5 : 2) : 2;
            const align = idx === 0 ? "left-0" : idx >= rows.length - 1 ? "right-0" : "left-1/2 -translate-x-1/2";
            return (
              <div
                key={row.fecha}
                className={cn(
                  "group relative flex h-full flex-col items-center justify-end",
                  rows.length === 1 ? "w-24" : "min-w-[20px] flex-1",
                )}
              >
                <div
                  className={cn(
                    "pointer-events-none absolute -top-9 z-20 hidden rounded-lg bg-slate-900 px-2.5 py-1 text-[11px] font-medium whitespace-nowrap text-white shadow-lg group-hover:block",
                    align,
                  )}
                >
                  {formatIsoDisplay(row.fecha)} · {formatCOP(gross)} ({row.ventas})
                </div>
                <div
                  className="w-full rounded-t-md bg-teal-600/80 transition-colors group-hover:bg-teal-700"
                  style={{ height: `${pct}%` }}
                />
              </div>
            );
          })}
        </div>

        <div className="mt-2 flex items-center gap-2 pl-12">
          {rows.map((row, idx) => {
            const show = idx % labelStep === 0 || idx === rows.length - 1;
            return (
              <span
                key={row.fecha}
                className={cn(
                  "text-center text-[10px] text-slate-400 tabular-nums",
                  rows.length === 1 ? "w-24" : "min-w-[20px] flex-1",
                )}
              >
                {show ? shortDay(row.fecha) : ""}
              </span>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** Pantalla 02: resumen de ventas, por día, cajero, método y mapa de calor. */
export function SalesReportsScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);

  const [today] = useState(localTodayIso);
  const [preset, setPreset] = useState<ReportPeriodPreset>("SIETE_DIAS");
  const [customFrom, setCustomFrom] = useState(today);
  const [customTo, setCustomTo] = useState(today);
  const [allBranches, setAllBranches] = useState(false);
  const [tab, setTab] = useState<SalesTab>("DIA");

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
    const [byDay, byCashier, byMethod, heatmap] = await Promise.all([
      api.reports.salesByDay(query),
      api.reports.salesByCashier(query),
      api.reports.salesByMethod(query),
      api.reports.salesHeatmap(query),
    ]);
    return { byDay, byCashier, byMethod, heatmap };
  }, queryKey);

  const dayTotals = useMemo(() => {
    const rows = data?.byDay.rows ?? [];
    const ventas = rows.reduce((s, r) => s + r.ventas, 0);
    const bruta = rows.reduce((s, r) => s + Number(r.venta_bruta), 0);
    const devoluciones = rows.reduce((s, r) => s + Number(r.devoluciones), 0);
    const neta = rows.reduce((s, r) => s + Number(r.venta_neta), 0);
    const margen = rows.reduce((s, r) => s + Number(r.margen), 0);
    const ticket = ventas > 0 ? bruta / ventas : 0;
    return { ventas, bruta, devoluciones, neta, margen, ticket };
  }, [data]);

  const cashierTotals = useMemo(() => {
    const rows = data?.byCashier.rows ?? [];
    return rows.reduce((s, r) => s + Number(r.venta_bruta), 0);
  }, [data]);

  const methodTotals = useMemo(() => {
    const rows = data?.byMethod.rows ?? [];
    return rows.reduce((s, r) => s + Number(r.monto), 0);
  }, [data]);

  const heatmapGrid = useMemo(() => {
    const rows = data?.heatmap.rows ?? [];
    const lookup = new Map<string, { ventas: number; bruta: number }>();
    let maxGross = 0;
    for (const r of rows) {
      const bruta = Number(r.venta_bruta);
      if (bruta > maxGross) maxGross = bruta;
      lookup.set(`${r.dia}-${r.hora}`, { ventas: r.ventas, bruta });
    }
    return { lookup, maxGross };
  }, [data]);

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
          <h1 className="text-3xl font-bold tracking-tight text-slate-900">Reportes de ventas</h1>
          <p className="mt-1 text-sm text-slate-500">
            Desglose diario, por cajero, método de pago y mapa horario en {allBranches ? "todo el negocio" : currentBranch.name}.
          </p>
        </div>
        <SalesReportNav active="resumen" />
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
        period={data?.byDay.period}
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

      {data && <CostMissingWarning count={data.byDay.cost_missing_lines} />}

      {loading && !data ? (
        <div className="space-y-4">
          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-36 animate-pulse rounded-2xl bg-slate-200/60" />
            ))}
          </div>
          <div className="h-80 animate-pulse rounded-2xl bg-slate-200/60" />
        </div>
      ) : (
        data && (
          <>
            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
              <StatCard
                title="Venta bruta"
                value={formatCOP(dayTotals.bruta)}
                caption={`${dayTotals.ventas} ${dayTotals.ventas === 1 ? "venta" : "ventas"} en el periodo`}
                icon={Receipt}
              />
              <StatCard
                title="Venta neta"
                value={formatCOP(dayTotals.neta)}
                caption={`Descontando ${formatCOP(dayTotals.devoluciones)} en devoluciones`}
                icon={TrendingUp}
                tone="success"
              />
              <StatCard
                title="Ticket promedio"
                value={formatCOP(dayTotals.ticket)}
                caption="Promedio por orden cobrada"
                icon={ShoppingBag}
              />
              <StatCard
                title="Margen bruto"
                value={formatCOP(dayTotals.margen)}
                caption={dayTotals.devoluciones > 0 ? "Incluye impacto de devoluciones" : "Sobre venta sin impuesto"}
                icon={dayTotals.devoluciones > 0 ? RotateCcw : Banknote}
                tone={dayTotals.devoluciones > 0 ? "warning" : "success"}
              />
            </div>

            <div className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
                <div className="flex flex-wrap gap-1 rounded-xl bg-slate-100 p-1" role="tablist">
                  {[
                    { id: "DIA" as const, label: "Por día" },
                    { id: "CAJERO" as const, label: "Por cajero" },
                    { id: "METODO" as const, label: "Por método" },
                    { id: "HEATMAP" as const, label: "Mapa de calor" },
                  ].map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      role="tab"
                      aria-selected={tab === t.id}
                      onClick={() => setTab(t.id)}
                      className={cn(
                        "rounded-lg px-3.5 py-1.5 text-xs font-semibold transition-colors",
                        tab === t.id ? "bg-white text-slate-900 shadow-xs" : "text-slate-600 hover:text-slate-900",
                      )}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>

                {tab === "DIA" && <ExportCsvButton slug="sales-by-day" query={query} disabled={data.byDay.rows.length === 0} />}
                {tab === "CAJERO" && (
                  <ExportCsvButton slug="sales-by-cashier" query={query} disabled={data.byCashier.rows.length === 0} />
                )}
                {tab === "METODO" && (
                  <ExportCsvButton slug="sales-by-method" query={query} disabled={data.byMethod.rows.length === 0} />
                )}
                {tab === "HEATMAP" && (
                  <span className="text-xs text-slate-400">Matriz horaria 7 × 24 (solo consulta visual)</span>
                )}
              </div>

              <div className="p-4 sm:p-6">
                {tab === "DIA" && (
                  <div className="space-y-6">
                    <DailyBarChart rows={data.byDay.rows} />
                    {data.byDay.rows.length === 0 ? (
                      <p className="py-6 text-center text-sm text-slate-400">No hay días con ventas en este rango.</p>
                    ) : (
                      <div className="overflow-x-auto">
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                              <th className="py-2.5 pr-4">Fecha</th>
                              <th className="py-2.5 pr-4 text-right">Ventas</th>
                              <th className="py-2.5 pr-4 text-right">Venta bruta</th>
                              <th className="py-2.5 pr-4 text-right">Devoluciones</th>
                              <th className="py-2.5 pr-4 text-right">Venta neta</th>
                              <th className="py-2.5 pr-4 text-right">Ticket prom.</th>
                              <th className="py-2.5 text-right">Margen</th>
                            </tr>
                          </thead>
                          <tbody className="divide-y divide-slate-100">
                            {data.byDay.rows.map((r) => (
                              <tr key={r.fecha} className="hover:bg-slate-50/70">
                                <td className="py-3 pr-4 font-medium text-slate-900 tabular-nums">
                                  {formatIsoDisplay(r.fecha)}
                                </td>
                                <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{r.ventas}</td>
                                <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                                  {formatCOP(Number(r.venta_bruta))}
                                </td>
                                <td className="py-3 pr-4 text-right text-rose-600 tabular-nums">
                                  {Number(r.devoluciones) > 0 ? formatCOP(Number(r.devoluciones)) : "—"}
                                </td>
                                <td className="py-3 pr-4 text-right font-semibold text-emerald-700 tabular-nums">
                                  {formatCOP(Number(r.venta_neta))}
                                </td>
                                <td className="py-3 pr-4 text-right text-slate-600 tabular-nums">
                                  {formatCOP(Number(r.ticket_promedio))}
                                </td>
                                <td className="py-3 text-right font-semibold text-slate-900 tabular-nums">
                                  {formatCOP(Number(r.margen))}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                          <tfoot>
                            <tr className="border-t border-slate-200 bg-slate-50/70 font-semibold text-slate-900">
                              <td className="py-3 pr-4">Total</td>
                              <td className="py-3 pr-4 text-right tabular-nums">{dayTotals.ventas}</td>
                              <td className="py-3 pr-4 text-right tabular-nums">{formatCOP(dayTotals.bruta)}</td>
                              <td className="py-3 pr-4 text-right text-rose-600 tabular-nums">
                                {formatCOP(dayTotals.devoluciones)}
                              </td>
                              <td className="py-3 pr-4 text-right text-emerald-700 tabular-nums">
                                {formatCOP(dayTotals.neta)}
                              </td>
                              <td className="py-3 pr-4 text-right tabular-nums">{formatCOP(dayTotals.ticket)}</td>
                              <td className="py-3 text-right tabular-nums">{formatCOP(dayTotals.margen)}</td>
                            </tr>
                          </tfoot>
                        </table>
                      </div>
                    )}
                  </div>
                )}

                {tab === "CAJERO" &&
                  (data.byCashier.rows.length === 0 ? (
                    <p className="py-10 text-center text-sm text-slate-400">
                      No hay ventas registradas por cajero en este periodo.
                    </p>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                            <th className="py-2.5 pr-4">Cajero</th>
                            <th className="py-2.5 pr-4 text-right">Ventas</th>
                            <th className="py-2.5 pr-4 text-right">Venta bruta</th>
                            <th className="py-2.5 pr-4 text-right">Participación</th>
                            <th className="py-2.5 text-right">Ticket promedio</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-100">
                          {data.byCashier.rows.map((r) => {
                            const gross = Number(r.venta_bruta);
                            const share = cashierTotals > 0 ? (gross / cashierTotals) * 100 : 0;
                            return (
                              <tr key={r.cashier_id ?? r.cajero} className="hover:bg-slate-50/70">
                                <td className="py-3 pr-4 font-semibold text-slate-900">{r.cajero}</td>
                                <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{r.ventas}</td>
                                <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                                  {formatCOP(gross)}
                                </td>
                                <td className="py-3 pr-4">
                                  <div className="flex items-center justify-end gap-2">
                                    <div className="h-2 w-24 overflow-hidden rounded-full bg-slate-100">
                                      <div
                                        className="h-full rounded-full bg-teal-600"
                                        style={{ width: `${Math.max(Math.round(share), gross > 0 ? 4 : 0)}%` }}
                                      />
                                    </div>
                                    <span className="w-12 text-right text-xs text-slate-500 tabular-nums">
                                      {formatPercent(share)}
                                    </span>
                                  </div>
                                </td>
                                <td className="py-3 text-right text-slate-700 tabular-nums">
                                  {formatCOP(Number(r.ticket_promedio))}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  ))}

                {tab === "METODO" &&
                  (data.byMethod.rows.length === 0 ? (
                    <p className="py-10 text-center text-sm text-slate-400">
                      No hay pagos registrados en este periodo.
                    </p>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                            <th className="py-2.5 pr-4">Método</th>
                            <th className="py-2.5 pr-4 text-right">Pagos</th>
                            <th className="py-2.5 pr-4 text-right">Monto</th>
                            <th className="py-2.5 text-right">Participación</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-100">
                          {data.byMethod.rows.map((r) => {
                            const amount = Number(r.monto);
                            const share = methodTotals > 0 ? (amount / methodTotals) * 100 : 0;
                            const style = METHOD_STYLES[r.metodo] ?? { dot: "bg-slate-500", bar: "bg-slate-500" };
                            const label = r.metodo === "CREDITO" ? "Crédito" : r.etiqueta || r.metodo;
                            return (
                              <tr key={r.metodo} className="hover:bg-slate-50/70">
                                <td className="py-3 pr-4">
                                  <span className="inline-flex items-center gap-2 font-semibold text-slate-900">
                                    <span className={cn("size-2.5 rounded-full", style.dot)} />
                                    {label}
                                  </span>
                                </td>
                                <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{r.pagos}</td>
                                <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                                  {formatCOP(amount)}
                                </td>
                                <td className="py-3">
                                  <div className="flex items-center justify-end gap-2">
                                    <div className="h-2 w-28 overflow-hidden rounded-full bg-slate-100">
                                      <div
                                        className={cn("h-full rounded-full", style.bar)}
                                        style={{ width: `${Math.max(Math.round(share), amount > 0 ? 4 : 0)}%` }}
                                      />
                                    </div>
                                    <span className="w-12 text-right text-xs text-slate-500 tabular-nums">
                                      {formatPercent(share)}
                                    </span>
                                  </div>
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  ))}

                {tab === "HEATMAP" && (
                  <div className="space-y-4 overflow-x-auto">
                    <div className="min-w-[720px] space-y-1.5">
                      <div className="grid grid-cols-[64px_repeat(24,minmax(0,1fr))] items-center gap-1 text-center text-[10px] font-medium text-slate-400">
                        <span />
                        {Array.from({ length: 24 }).map((_, h) => (
                          <span key={h}>{String(h).padStart(2, "0")}</span>
                        ))}
                      </div>
                      {DAY_NAMES.map((dayName, dayIdx) => {
                        const dia = dayIdx + 1;
                        return (
                          <div key={dia} className="grid grid-cols-[64px_repeat(24,minmax(0,1fr))] items-center gap-1">
                            <span className="text-xs font-semibold text-slate-600">{dayName}</span>
                            {Array.from({ length: 24 }).map((_, hora) => {
                              const cell = heatmapGrid.lookup.get(`${dia}-${hora}`) ?? { ventas: 0, bruta: 0 };
                              const ratio = heatmapGrid.maxGross > 0 ? cell.bruta / heatmapGrid.maxGross : 0;
                              const colorClass =
                                cell.ventas === 0
                                  ? "bg-slate-100 text-slate-300"
                                  : ratio >= 0.75
                                    ? "bg-teal-700 text-white"
                                    : ratio >= 0.5
                                      ? "bg-teal-500 text-white"
                                      : ratio >= 0.25
                                        ? "bg-teal-300 text-teal-950"
                                        : "bg-teal-100 text-teal-900";
                              return (
                                <div
                                  key={hora}
                                  title={`${dayName} ${String(hora).padStart(2, "0")}:00 · ${cell.ventas} ${cell.ventas === 1 ? "venta" : "ventas"} · ${formatCOP(cell.bruta)}`}
                                  className={cn(
                                    "flex h-7 items-center justify-center rounded-md text-[10px] font-semibold tabular-nums transition-transform hover:scale-105",
                                    colorClass,
                                  )}
                                >
                                  {cell.ventas > 0 ? cell.ventas : ""}
                                </div>
                              );
                            })}
                          </div>
                        );
                      })}
                    </div>
                    <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-slate-500">
                      <span>Pasá el cursor por cada celda para ver el total en COP de esa franja horaria.</span>
                      <div className="flex items-center gap-1.5">
                        <span>Menor</span>
                        <span className="size-3 rounded-xs bg-slate-100" />
                        <span className="size-3 rounded-xs bg-teal-100" />
                        <span className="size-3 rounded-xs bg-teal-300" />
                        <span className="size-3 rounded-xs bg-teal-500" />
                        <span className="size-3 rounded-xs bg-teal-700" />
                        <span>Mayor</span>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </>
        )
      )}
    </div>
  );
}


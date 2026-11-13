"use client";

import { useState } from "react";
import Link from "next/link";
import { AlertTriangle, Download, Loader2 } from "lucide-react";

import { api, friendlyError, type ReportCsvSlug, type ReportQuery } from "@/lib/api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

export type ReportPeriodPreset =
  | "HOY"
  | "AYER"
  | "SIETE_DIAS"
  | "ESTE_MES"
  | "MES_ANTERIOR"
  | "PERSONALIZADO";

export const REPORT_PRESETS: { id: ReportPeriodPreset; label: string }[] = [
  { id: "HOY", label: "Hoy" },
  { id: "AYER", label: "Ayer" },
  { id: "SIETE_DIAS", label: "7 días" },
  { id: "ESTE_MES", label: "Este mes" },
  { id: "MES_ANTERIOR", label: "Mes anterior" },
  { id: "PERSONALIZADO", label: "Personalizado" },
];

export function localTodayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function addDaysIso(baseIso: string, delta: number): string {
  const [y, m, d] = baseIso.split("-").map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d + delta));
  return `${dt.getUTCFullYear()}-${String(dt.getUTCMonth() + 1).padStart(2, "0")}-${String(dt.getUTCDate()).padStart(2, "0")}`;
}

export function rangeForReportPreset(
  preset: Exclude<ReportPeriodPreset, "PERSONALIZADO">,
  todayIso: string,
): { from: string; to: string } {
  if (preset === "HOY") {
    return { from: todayIso, to: todayIso };
  }
  if (preset === "AYER") {
    const yesterday = addDaysIso(todayIso, -1);
    return { from: yesterday, to: yesterday };
  }
  if (preset === "SIETE_DIAS") {
    return { from: addDaysIso(todayIso, -6), to: todayIso };
  }
  if (preset === "ESTE_MES") {
    const [y, m] = todayIso.split("-");
    return { from: `${y}-${m}-01`, to: todayIso };
  }
  const [y, m] = todayIso.split("-").map(Number);
  const prevLast = new Date(Date.UTC(y, m - 1, 0));
  const prevY = prevLast.getUTCFullYear();
  const prevM = String(prevLast.getUTCMonth() + 1).padStart(2, "0");
  const prevD = String(prevLast.getUTCDate()).padStart(2, "0");
  return { from: `${prevY}-${prevM}-01`, to: `${prevY}-${prevM}-${prevD}` };
}

export function formatIsoDisplay(iso: string): string {
  const parts = iso.split("-");
  if (parts.length !== 3) return iso;
  return `${parts[2]}/${parts[1]}/${parts[0]}`;
}

export function SalesReportNav({ active }: { active: "resumen" | "productos" }) {
  return (
    <div className="inline-flex rounded-2xl border border-slate-200/80 bg-white p-1 shadow-xs">
      <Link
        href="/ventas"
        className={cn(
          "rounded-xl px-4 py-2 text-sm font-medium transition-colors",
          active === "resumen" ? "bg-teal-700 text-white shadow-xs" : "text-slate-600 hover:text-slate-900",
        )}
      >
        Resumen de ventas
      </Link>
      <Link
        href="/ventas/productos"
        className={cn(
          "rounded-xl px-4 py-2 text-sm font-medium transition-colors",
          active === "productos" ? "bg-teal-700 text-white shadow-xs" : "text-slate-600 hover:text-slate-900",
        )}
      >
        Ventas por producto y categoría
      </Link>
    </div>
  );
}

export function ReportFilterBar({
  preset,
  onPresetChange,
  customFrom,
  onCustomFromChange,
  customTo,
  onCustomToChange,
  allBranches,
  onAllBranchesChange,
  period,
  loading,
}: {
  preset: ReportPeriodPreset;
  onPresetChange: (preset: ReportPeriodPreset) => void;
  customFrom: string;
  onCustomFromChange: (value: string) => void;
  customTo: string;
  onCustomToChange: (value: string) => void;
  allBranches: boolean;
  onAllBranchesChange: (value: boolean) => void;
  period?: { from: string; to: string } | null;
  loading?: boolean;
}) {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const customRangeError =
    preset === "PERSONALIZADO" && (!customFrom || !customTo || customFrom > customTo)
      ? "La fecha inicial debe ser menor o igual que la fecha final."
      : null;

  const periodLabel = period
    ? period.from === period.to
      ? formatIsoDisplay(period.from)
      : `${formatIsoDisplay(period.from)} al ${formatIsoDisplay(period.to)}`
    : null;

  return (
    <div className="space-y-3">
      <div className="flex flex-col gap-4 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm lg:flex-row lg:items-center lg:justify-between">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-base font-semibold text-slate-900">
              {allBranches && branches.length > 1 ? "Todas tus sucursales" : currentBranch.name}
            </h2>
            {loading && <span className="text-xs text-slate-400">Actualizando…</span>}
          </div>
          {periodLabel && <p className="text-xs text-slate-500">Periodo: {periodLabel}</p>}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {branches.length > 1 && (
            <select
              aria-label="Alcance de sucursales"
              value={allBranches ? "ALL" : "CURRENT"}
              onChange={(e) => onAllBranchesChange(e.target.value === "ALL")}
              className="h-9 rounded-xl border border-slate-200 bg-white px-3 text-xs font-medium text-slate-700"
            >
              <option value="CURRENT">Sucursal actual ({currentBranch.name})</option>
              <option value="ALL">Todas las sucursales</option>
            </select>
          )}

          <div
            className="flex flex-wrap gap-1 rounded-xl bg-slate-100 p-1"
            role="group"
            aria-label="Selector de periodo"
          >
            {REPORT_PRESETS.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => onPresetChange(item.id)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                  preset === item.id
                    ? "bg-white text-slate-900 shadow-xs"
                    : "text-slate-600 hover:text-slate-900",
                )}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {preset === "PERSONALIZADO" && (
        <div className="flex flex-wrap items-end gap-3 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm">
          <label className="space-y-1 text-xs font-medium text-slate-500">
            Desde
            <input
              type="date"
              value={customFrom}
              onChange={(e) => onCustomFromChange(e.target.value)}
              className="block h-9 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
            />
          </label>
          <label className="space-y-1 text-xs font-medium text-slate-500">
            Hasta
            <input
              type="date"
              value={customTo}
              onChange={(e) => onCustomToChange(e.target.value)}
              className="block h-9 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
            />
          </label>
          {customRangeError && <p className="text-xs font-medium text-rose-600">{customRangeError}</p>}
        </div>
      )}
    </div>
  );
}

export function CostMissingWarning({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <div className="flex items-start gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
      <AlertTriangle className="mt-0.5 size-4 shrink-0 text-amber-600" />
      <p>
        Hay <span className="font-semibold tabular-nums">{count}</span>{" "}
        {count === 1 ? "línea vendida" : "líneas vendidas"} sin costo registrado en este periodo. El margen
        calculado puede verse más alto que el real.
      </p>
    </div>
  );
}

export function ExportCsvButton({
  slug,
  query,
  disabled,
  label = "Exportar CSV",
}: {
  slug: ReportCsvSlug;
  query: ReportQuery;
  disabled?: boolean;
  label?: string;
}) {
  const [downloading, setDownloading] = useState(false);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  async function handleExport() {
    setDownloading(true);
    setDownloadError(null);
    try {
      await api.reports.exportCsv(slug, query);
    } catch (e) {
      setDownloadError(friendlyError(e));
    } finally {
      setDownloading(false);
    }
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        type="button"
        onClick={handleExport}
        disabled={disabled || downloading}
        className="inline-flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs transition-colors hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {downloading ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
        {label}
      </button>
      {downloadError && <span className="text-xs font-medium text-rose-600">{downloadError}</span>}
    </div>
  );
}


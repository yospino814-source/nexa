"use client";

import { useMemo, useState } from "react";
import { Building2, CheckCircle2, RefreshCw, Truck, Wallet } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import {
  ExportCsvButton,
  localTodayIso,
  rangeForReportPreset,
  ReportFilterBar,
  type ReportPeriodPreset,
} from "@/components/ventas/report-controls";
import { api, friendlyError, type ReportQuery } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";

/** Pestaña "Reportes" dentro de /compras: compras por proveedor (F7.3 / S7). */
export function PurchasesReportsTab() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);

  const [today] = useState(localTodayIso);
  const [preset, setPreset] = useState<ReportPeriodPreset>("ESTE_MES");
  const [customFrom, setCustomFrom] = useState(today);
  const [customTo, setCustomTo] = useState(today);
  const [allBranches, setAllBranches] = useState(false);

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
  const { data, loading, error, reload } = useApi(() => api.reports.purchasesBySupplier(query), queryKey);

  const totals = useMemo(() => {
    const rows = data?.rows ?? [];
    return {
      ordenes: rows.reduce((s, r) => s + r.ordenes, 0),
      recibido: rows.reduce((s, r) => s + Number(r.recibido), 0),
      pagado: rows.reduce((s, r) => s + Number(r.pagado), 0),
      saldo: rows.reduce((s, r) => s + Number(r.saldo), 0),
    };
  }, [data]);

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
      <ReportFilterBar
        preset={preset}
        onPresetChange={setPreset}
        customFrom={customFrom}
        onCustomFromChange={setCustomFrom}
        customTo={customTo}
        onCustomToChange={setCustomTo}
        allBranches={allBranches}
        onAllBranchesChange={setAllBranches}
        period={data?.period}
        loading={loading}
      />

      {data && (
        <div className="grid gap-5 md:grid-cols-3">
          <StatCard
            title="Total recibido"
            value={formatCOP(totals.recibido)}
            caption={`${totals.ordenes} ${totals.ordenes === 1 ? "orden" : "órdenes"} en ${data.rows.length} proveedores`}
            icon={Truck}
          />
          <StatCard
            title="Total pagado"
            value={formatCOP(totals.pagado)}
            caption="Abonos aplicados en cuentas por pagar"
            icon={CheckCircle2}
            tone="success"
          />
          <StatCard
            title="Saldo pendiente"
            value={formatCOP(totals.saldo)}
            caption="Saldo abierto con proveedores"
            icon={Wallet}
            tone={totals.saldo > 0 ? "danger" : "default"}
          />
        </div>
      )}

      <section className="rounded-2xl border border-slate-200/60 bg-white shadow-sm">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 p-4 sm:px-6">
          <div className="flex items-center gap-2.5">
            <span className="flex size-9 items-center justify-center rounded-xl bg-slate-100 text-slate-600">
              <Building2 className="size-4" />
            </span>
            <div>
              <h2 className="text-lg font-semibold text-slate-900">Compras por proveedor</h2>
              <p className="text-xs text-slate-500">Órdenes no anuladas agrupadas por proveedor en el periodo</p>
            </div>
          </div>

          <ExportCsvButton
            slug="purchases-by-supplier"
            query={query}
            disabled={!data || data.rows.length === 0}
          />
        </div>

        {!data && loading ? (
          <div className="p-6">
            <div className="h-44 animate-pulse rounded-xl bg-slate-100" />
          </div>
        ) : !data || data.rows.length === 0 ? (
          <p className="py-12 text-center text-sm text-slate-400">
            No hay órdenes de compra registradas para proveedores en este periodo.
          </p>
        ) : (
          <div className="overflow-x-auto p-4 sm:p-6">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                  <th className="py-2.5 pr-4">Proveedor</th>
                  <th className="py-2.5 pr-4 text-right">Órdenes</th>
                  <th className="py-2.5 pr-4 text-right">Recibido</th>
                  <th className="py-2.5 pr-4 text-right">Pagado</th>
                  <th className="py-2.5 text-right">Saldo</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {data.rows.map((r) => (
                  <tr key={r.supplier_id} className="hover:bg-slate-50/70">
                    <td className="py-3 pr-4 font-semibold text-slate-900">{r.proveedor}</td>
                    <td className="py-3 pr-4 text-right text-slate-700 tabular-nums">{r.ordenes}</td>
                    <td className="py-3 pr-4 text-right font-semibold text-slate-900 tabular-nums">
                      {formatCOP(Number(r.recibido))}
                    </td>
                    <td className="py-3 pr-4 text-right text-emerald-700 tabular-nums">
                      {formatCOP(Number(r.pagado))}
                    </td>
                    <td className="py-3 text-right font-semibold text-rose-600 tabular-nums">
                      {formatCOP(Number(r.saldo))}
                    </td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr className="border-t border-slate-200 bg-slate-50/70 font-semibold text-slate-900">
                  <td className="py-3 pr-4">Total</td>
                  <td className="py-3 pr-4 text-right tabular-nums">{totals.ordenes}</td>
                  <td className="py-3 pr-4 text-right tabular-nums">{formatCOP(totals.recibido)}</td>
                  <td className="py-3 pr-4 text-right text-emerald-700 tabular-nums">{formatCOP(totals.pagado)}</td>
                  <td className="py-3 text-right text-rose-600 tabular-nums">{formatCOP(totals.saldo)}</td>
                </tr>
              </tfoot>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}


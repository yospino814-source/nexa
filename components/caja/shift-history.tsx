"use client";

import { History } from "lucide-react";

import { api } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCashVersion } from "@/lib/stores/cash";
import { formatCOP, formatDateTime, formatTime } from "@/lib/format";
import { cn } from "@/lib/utils";

/** Turnos anteriores de la sucursal con su arqueo (un cajero ve solo los suyos). */
export function ShiftHistory({ branchId }: { branchId: string }) {
  const version = useCashVersion();
  const { data, loading } = useApi(() => api.cash.history(branchId, 15), `${branchId}:${version}`);

  return (
    <section className="rounded-3xl border border-slate-200/60 bg-white p-6 shadow-sm">
      <h2 className="flex items-center gap-2 text-lg font-bold">
        <History className="size-5 text-slate-400" /> Turnos anteriores
      </h2>
      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-xs tracking-wider text-slate-400 uppercase">
            <tr>
              <th className="py-2 pr-4 font-semibold">Turno</th>
              <th className="py-2 pr-4 font-semibold">Cajero</th>
              <th className="py-2 pr-4 text-right font-semibold">Ventas</th>
              <th className="py-2 pr-4 text-right font-semibold">Esperado</th>
              <th className="py-2 pr-4 text-right font-semibold">Contado</th>
              <th className="py-2 text-right font-semibold">Diferencia</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {loading && !data && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-slate-400">
                  Cargando…
                </td>
              </tr>
            )}
            {data?.map((s) => {
              const diff = s.difference == null ? null : Number(s.difference);
              return (
                <tr key={s.id}>
                  <td className="py-3 pr-4 whitespace-nowrap">
                    {formatDateTime(s.opened_at)}
                    <span className="text-slate-400"> → {s.closed_at ? formatTime(s.closed_at) : "abierto"}</span>
                  </td>
                  <td className="py-3 pr-4">{s.opened_by_name ?? "—"}</td>
                  <td className="py-3 pr-4 text-right tabular-nums">
                    {formatCOP(Number(s.sales_total))}
                    <span className="block text-xs text-slate-400">{s.sales_count} ventas</span>
                  </td>
                  <td className="py-3 pr-4 text-right tabular-nums">{s.expected_cash ? formatCOP(Number(s.expected_cash)) : "—"}</td>
                  <td className="py-3 pr-4 text-right tabular-nums">{s.counted_cash ? formatCOP(Number(s.counted_cash)) : "—"}</td>
                  <td className="py-3 text-right">
                    {diff == null ? (
                      <span className="rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-semibold text-emerald-700">En curso</span>
                    ) : (
                      <span
                        className={cn(
                          "rounded-full px-2.5 py-0.5 text-xs font-semibold tabular-nums",
                          diff === 0 && "bg-emerald-50 text-emerald-700",
                          diff < 0 && "bg-rose-50 text-rose-600",
                          diff > 0 && "bg-sky-50 text-sky-700",
                        )}
                      >
                        {diff === 0 ? "Cuadrada" : formatCOP(diff)}
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
            {data?.length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-slate-400">
                  Todavía no hay turnos en esta sucursal.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

"use client";

import { useState } from "react";
import { Search } from "lucide-react";

import { PAYMENT_METHODS } from "@/components/pos/payment-methods";
import type { CashActivity } from "@/lib/api";
import { formatCOP, formatTime } from "@/lib/format";
import { cn } from "@/lib/utils";

export const ACTIVITY_KIND_LABELS: Record<CashActivity["kind"], string> = {
  APERTURA: "Apertura de caja",
  VENTA: "Venta",
  INGRESO: "Entrada general",
  ABONO_CARTERA: "Abono cartera",
  ABONO_SEPARADO: "Abono plan separado",
  RECARGA_SALDO: "Recarga saldo",
  RETIRO: "Retiro general",
  DEVOLUCION: "Devolución",
  PROPINA_PAGADA: "Pago de propina",
  PAGO_PROVEEDOR: "Pago a proveedor",
  GASTO: "Gasto menor",
  CIERRE: "Cierre de caja",
};

export const IS_INFLOW_KIND: Record<CashActivity["kind"], boolean | null> = {
  APERTURA: true,
  VENTA: true,
  INGRESO: true,
  ABONO_CARTERA: true,
  ABONO_SEPARADO: true,
  RECARGA_SALDO: true,
  RETIRO: false,
  DEVOLUCION: false,
  PROPINA_PAGADA: false,
  PAGO_PROVEEDOR: false,
  GASTO: false,
  CIERRE: null,
};

const FILTERS = [
  { id: "ALL", label: "Todos" },
  { id: "VENTA", label: "Ventas" },
  { id: "INGRESO", label: "Ingresos" },
  { id: "RETIRO", label: "Retiros" },
  { id: "PROPINA_PAGADA", label: "Propinas" },
] as const;

function methodsLabel(methods: string | null) {
  if (!methods) return "—";
  return methods
    .split("+")
    .map((m) => PAYMENT_METHODS.find((p) => p.id === m)?.label ?? m)
    .join(" + ");
}

export function ShiftActivity({ activity }: { activity: CashActivity[] | null }) {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("ALL");
  const [query, setQuery] = useState("");

  const count = (id: string) => {
    if (!activity) return 0;
    if (id === "ALL") return activity.length;
    return activity.filter((a) => a.kind === id).length;
  };

  const rows = (activity ?? []).filter((a) => {
    if (filter !== "ALL" && a.kind !== filter) return false;
    if (!query) return true;
    const label = ACTIVITY_KIND_LABELS[a.kind] ?? a.kind;
    return `${label} ${a.detail ?? ""}`.toLowerCase().includes(query.toLowerCase());
  });

  return (
    <section className="rounded-3xl border border-slate-200/60 bg-white p-6 shadow-sm">
      <h2 className="text-lg font-bold">Movimientos del turno</h2>
      <p className="text-sm text-slate-500">Apertura, ventas, ingresos, retiros y pagos, en tiempo real.</p>

      <div className="mt-4 flex flex-wrap items-center gap-2">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            className={cn(
              "rounded-full px-3 py-1.5 text-sm transition-colors",
              filter === f.id
                ? "bg-brand font-medium text-white"
                : "bg-slate-100 text-slate-600 hover:bg-slate-200",
            )}
          >
            {f.label} ({count(f.id)})
          </button>
        ))}
        <label className="ml-auto flex h-9 items-center gap-2 rounded-full border border-slate-200 px-3 text-sm">
          <Search className="size-4 text-slate-400" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar movimiento…"
            className="w-40 bg-transparent outline-none"
          />
        </label>
      </div>

      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-xs tracking-wider text-slate-400 uppercase">
            <tr>
              <th className="py-2 pr-4 font-semibold">Hora</th>
              <th className="py-2 pr-4 font-semibold">Detalle</th>
              <th className="py-2 pr-4 font-semibold">Medio de pago</th>
              <th className="py-2 pr-4 font-semibold">Usuario</th>
              <th className="py-2 text-right font-semibold">Monto</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {rows.map((a, i) => {
              const inflow = IS_INFLOW_KIND[a.kind];
              const isOut = inflow === false;
              const isIn = inflow === true;
              return (
                <tr key={`${a.at}-${i}`}>
                  <td className="py-3 pr-4 whitespace-nowrap text-slate-500 tabular-nums">
                    {formatTime(a.at)}
                  </td>
                  <td className="py-3 pr-4">
                    <span className="flex items-center gap-2">
                      <span
                        className={cn(
                          "size-2 shrink-0 rounded-full",
                          isOut ? "bg-rose-500" : isIn ? "bg-emerald-500" : "bg-slate-400",
                        )}
                      />
                      <span className={cn(isOut && "text-rose-600 font-medium", isIn && "text-slate-800")}>
                        {ACTIVITY_KIND_LABELS[a.kind] ?? a.kind}
                        {a.detail && <span className="text-slate-400 font-normal"> · {a.detail}</span>}
                      </span>
                    </span>
                  </td>
                  <td className="py-3 pr-4">
                    <span className="rounded-md bg-slate-100 px-2 py-0.5 text-xs text-slate-600">
                      {methodsLabel(a.methods)}
                    </span>
                  </td>
                  <td className="py-3 pr-4 text-slate-500">{a.user_name ?? "—"}</td>
                  <td
                    className={cn(
                      "py-3 text-right font-semibold whitespace-nowrap tabular-nums",
                      isOut ? "text-rose-600" : isIn ? "text-emerald-600" : "text-slate-700",
                    )}
                  >
                    {isOut ? "−" : isIn ? "+" : ""}
                    {formatCOP(Number(a.amount))}
                  </td>
                </tr>
              );
            })}
            {activity && rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-8 text-center text-slate-400">
                  Sin movimientos para este filtro.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}


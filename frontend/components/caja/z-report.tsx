"use client";

import { CheckCircle2, Printer, Wallet } from "lucide-react";

import { PrintArea } from "@/components/nexa/print-area";
import { PAYMENT_METHODS } from "@/components/pos/payment-methods";
import type { CashSummary } from "@/lib/api";
import { formatCOP, formatDateTime } from "@/lib/format";
import { cn } from "@/lib/utils";

const methodLabel = (m: string) => PAYMENT_METHODS.find((p) => p.id === m)?.label ?? m;

/** Líneas del reporte Z, compartidas por la pantalla y la tirilla. */
function zLines(summary: CashSummary) {
  const s = summary.session;
  const lines: [string, number][] = [
    ["Base inicial", Number(s.opening_amount)],
    ...summary.by_method.map((m) => [`Ventas ${methodLabel(m.method)} (${m.count})`, Number(m.amount)] as [string, number]),
  ];

  if (summary.movements_by_kind && summary.movements_by_kind.length > 0) {
    for (const m of summary.movements_by_kind) {
      const amt = Number(m.amount);
      if (amt !== 0) {
        lines.push([`${m.label} (${m.count})`, m.is_inflow ? amt : -amt]);
      }
    }
  } else {
    if (Number(summary.cash_in) > 0) {
      lines.push(["Ingresos de efectivo", Number(summary.cash_in)]);
    }
    if (Number(summary.cash_out) > 0) {
      lines.push(["Retiros de efectivo", -Number(summary.cash_out)]);
    }
  }

  return lines;
}

/** Resultado del cierre: reporte Z en pantalla y en tirilla. */
export function ZReport({
  summary,
  branchName,
  cashierName,
  onNewShift,
}: {
  summary: CashSummary;
  branchName: string;
  cashierName: string | null;
  onNewShift: () => void;
}) {
  const s = summary.session;
  const expected = Number(s.expected_cash ?? summary.expected_cash);
  const counted = Number(s.counted_cash ?? 0);
  const difference = Number(s.difference ?? 0);

  const tipsTotal = Number(summary.tips_total ?? 0);
  const tipsPaid = Number(summary.tips_paid ?? 0);
  const tipsPending = Number(summary.tips_pending ?? 0);
  const hasTips = tipsTotal > 0 || tipsPaid > 0;

  return (
    <div className="mx-auto mt-8 max-w-lg rounded-3xl border border-slate-200/60 bg-white p-8 shadow-sm">
      <PrintArea>
        <p className="t-center t-bold t-big">{branchName}</p>
        <p className="t-center t-bold">CIERRE DE CAJA (Z)</p>
        <hr />
        <p className="t-row"><span>Cajero</span><span>{cashierName ?? "-"}</span></p>
        <p className="t-row"><span>Apertura</span><span>{formatDateTime(s.opened_at)}</span></p>
        <p className="t-row"><span>Cierre</span><span>{s.closed_at ? formatDateTime(s.closed_at) : "-"}</span></p>
        <hr />
        <p className="t-row"><span>Ventas ({summary.sales_count})</span><span>{formatCOP(Number(summary.sales_total))}</span></p>
        {zLines(summary).map(([label, amount]) => (
          <p key={label} className="t-row"><span>{label}</span><span>{formatCOP(amount)}</span></p>
        ))}
        {hasTips && (
          <>
            <hr />
            <p className="t-row"><span>Propinas recibidas</span><span>{formatCOP(tipsTotal)}</span></p>
            <p className="t-row"><span>Propinas pagadas</span><span>{formatCOP(tipsPaid)}</span></p>
            <p className="t-row t-bold"><span>Propinas por pagar</span><span>{formatCOP(tipsPending)}</span></p>
          </>
        )}
        <hr />
        <p className="t-row t-bold"><span>Efectivo esperado</span><span>{formatCOP(expected)}</span></p>
        <p className="t-row t-bold"><span>Efectivo contado</span><span>{formatCOP(counted)}</span></p>
        <p className="t-row t-bold t-big"><span>Diferencia</span><span>{formatCOP(difference)}</span></p>
        {s.notes && <p className="t-small">Nota: {s.notes}</p>}
        <hr />
        <p className="t-center t-small">Firma cajero ____________________</p>
      </PrintArea>

      <CheckCircle2 className="mx-auto size-16 rounded-full bg-emerald-50 p-3 text-emerald-600" />
      <h2 className="mt-3 text-center text-2xl font-extrabold tracking-tight">Caja cerrada</h2>
      <p className="text-center text-sm text-slate-500">
        {summary.sales_count} ventas por {formatCOP(Number(summary.sales_total))}
      </p>

      <div className="mt-6 space-y-1.5 rounded-2xl bg-slate-50 p-4 text-sm">
        {zLines(summary).map(([label, amount]) => (
          <div key={label} className="flex justify-between text-slate-600">
            <span>{label}</span>
            <span className="tabular-nums">{formatCOP(amount)}</span>
          </div>
        ))}
        {hasTips && (
          <div className="border-t border-slate-200 pt-2 text-xs text-slate-600">
            <div className="flex justify-between">
              <span>Propinas recibidas</span>
              <span className="tabular-nums font-medium">{formatCOP(tipsTotal)}</span>
            </div>
            <div className="flex justify-between mt-0.5">
              <span>Propinas pagadas a meseros</span>
              <span className="tabular-nums font-medium text-amber-700">{formatCOP(tipsPaid)}</span>
            </div>
            <div className="flex justify-between mt-0.5 font-semibold text-slate-800">
              <span>Propinas pendientes</span>
              <span className="tabular-nums">{formatCOP(tipsPending)}</span>
            </div>
          </div>
        )}
        <div className="flex justify-between border-t border-slate-200 pt-2 font-medium">
          <span>Efectivo esperado</span>
          <span className="tabular-nums">{formatCOP(expected)}</span>
        </div>
        <div className="flex justify-between font-medium">
          <span>Efectivo contado</span>
          <span className="tabular-nums">{formatCOP(counted)}</span>
        </div>
        <div
          className={cn(
            "flex justify-between font-bold",
            difference === 0 ? "text-emerald-600" : difference < 0 ? "text-rose-600" : "text-sky-700",
          )}
        >
          <span>{difference === 0 ? "Cuadrada" : difference < 0 ? "Faltante" : "Sobrante"}</span>
          <span className="tabular-nums">{formatCOP(difference)}</span>
        </div>
      </div>

      <div className="mt-6 grid grid-cols-2 gap-3">
        <button
          onClick={() => window.print()}
          className="flex h-12 items-center justify-center gap-2 rounded-2xl border border-slate-200 font-medium hover:bg-slate-50"
        >
          <Printer className="size-4" /> Imprimir Z
        </button>
        <button
          onClick={onNewShift}
          className="flex h-12 items-center justify-center gap-2 rounded-2xl bg-brand font-semibold text-white hover:bg-brand-dark"
        >
          <Wallet className="size-4" /> Abrir nuevo turno
        </button>
      </div>
    </div>
  );
}

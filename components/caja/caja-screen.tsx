"use client";

import { useState } from "react";
import Link from "next/link";
import { ArrowDownCircle, ArrowLeft, Coins, MinusCircle, PlusCircle, TrendingUp, Wallet } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { PAYMENT_METHODS } from "@/components/pos/payment-methods";
import { api, type CashMovementKind, type CashSummary } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCashVersion } from "@/lib/stores/cash";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP, formatTime } from "@/lib/format";

import { CashCount } from "./cash-count";
import { CashMovementDialog } from "./cash-movement-dialog";
import { OpenShift } from "./open-shift";
import { ShiftActivity } from "./shift-activity";
import { ShiftHistory } from "./shift-history";
import { ZReport } from "./z-report";

const ADMIN_ROLES = ["DUENO", "ADMIN_CAJA"];

/** Pantalla 17: caja del turno (apertura, movimientos, arqueo y cierre Z). */
export function CajaScreen() {
  const branch = useCurrentBranch();
  const cashierName = useSession((s) => s.name);
  const version = useCashVersion();
  const { data: summary, loading, error, reload } = useApi(() => api.cash.current(branch.id), `${branch.id}:${version}`);
  const [closed, setClosed] = useState<CashSummary | null>(null);
  const isAdmin = ADMIN_ROLES.includes(branch.role);

  if (!["DUENO", "ADMIN_CAJA", "CAJERO"].includes(branch.role)) {
    return <Message text="Tu rol no maneja caja en esta sucursal." />;
  }
  if (error) return <Message text="No se pudo cargar la caja." onRetry={reload} />;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-7xl space-y-6 p-4 lg:p-8">
        <Link href="/pos" className="inline-flex items-center gap-1.5 text-sm font-medium text-slate-500 hover:text-slate-800">
          <ArrowLeft className="size-4" /> Volver al POS
        </Link>

        {closed ? (
          <ZReport summary={closed} branchName={branch.name} cashierName={cashierName} onNewShift={() => setClosed(null)} />
        ) : loading && !summary ? (
          <div className="h-64 animate-pulse rounded-3xl bg-slate-200/60" />
        ) : summary ? (
          <OpenShiftView summary={summary} cashierName={cashierName} onChanged={reload} onClosed={setClosed} />
        ) : (
          <OpenShift branchId={branch.id} branchName={branch.name} onOpened={reload} />
        )}

        {isAdmin || !summary ? <ShiftHistory branchId={branch.id} /> : null}
      </div>
    </div>
  );
}

function OpenShiftView({
  summary,
  cashierName,
  onChanged,
  onClosed,
}: {
  summary: CashSummary;
  cashierName: string | null;
  onChanged: () => void;
  onClosed: (s: CashSummary) => void;
}) {
  const [dialog, setDialog] = useState<CashMovementKind | null>(null);
  const version = useCashVersion();
  const { data: activity } = useApi(() => api.cash.activity(summary.session.id), `${summary.session.id}:${version}`);

  const s = summary.session;
  const expected = Number(summary.expected_cash);
  const cashSales = Number(summary.by_method.find((m) => m.method === "EFECTIVO")?.amount ?? 0);
  const cashIn = Number(summary.cash_in);
  const cashOut = Number(summary.cash_out);
  const moves = activity?.filter((a) => a.kind === "INGRESO" || a.kind === "RETIRO").length ?? 0;

  return (
    <>
      <section className="flex flex-col gap-5 rounded-3xl border border-slate-200/60 bg-white p-6 shadow-sm lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Caja abierta</h1>
          <p className="mt-2 inline-flex items-center gap-2 rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1 text-sm font-medium text-emerald-700">
            <span className="size-2 rounded-full bg-emerald-500" />
            Desde las {formatTime(s.opened_at)} · Cajero: {cashierName ?? "—"}
          </p>
        </div>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={() => setDialog("INGRESO")}
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
          >
            <PlusCircle className="size-4 text-emerald-600" /> Registrar ingreso
          </button>
          <button
            onClick={() => setDialog("RETIRO")}
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
          >
            <MinusCircle className="size-4 text-rose-600" /> Registrar retiro
          </button>
          <button
            onClick={() => setDialog("PROPINA_PAGADA")}
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
          >
            <Coins className="size-4 text-amber-600" /> Pagar propina
          </button>
        </div>
      </section>

      <div className="grid gap-6 md:grid-cols-3">
        <StatCard
          title="Efectivo esperado en gaveta"
          value={formatCOP(expected)}
          caption={`Base ${formatCOP(Number(s.opening_amount))} + ventas ${formatCOP(cashSales)}${cashIn ? ` + ingresos ${formatCOP(cashIn)}` : ""}${cashOut ? ` − retiros ${formatCOP(cashOut)}` : ""}`}
          icon={Wallet}
          tone="success"
        />
        <StatCard
          title="Ventas del turno"
          value={formatCOP(Number(summary.sales_total))}
          caption={
            summary.by_method.length
              ? summary.by_method
                  .map((m) => `${PAYMENT_METHODS.find((p) => p.id === m.method)?.label ?? m.method} ${formatCOP(Number(m.amount))}`)
                  .join(" · ")
              : "Todavía no hay ventas"
          }
          icon={TrendingUp}
          badge={{ label: `${summary.sales_count} ventas`, tone: "neutral" }}
        />
        <StatCard
          title="Retiros de caja"
          value={formatCOP(cashOut)}
          caption={`${moves} movimiento${moves === 1 ? "" : "s"} manual${moves === 1 ? "" : "es"}${cashIn ? ` · ingresos ${formatCOP(cashIn)}` : ""}`}
          icon={ArrowDownCircle}
          tone={cashOut ? "danger" : "default"}
        />
      </div>

      <div className="grid items-start gap-6 xl:grid-cols-[1fr_420px]">
        <ShiftActivity activity={activity ?? null} />
        <CashCount summary={summary} onClosed={onClosed} />
      </div>

      <CashMovementDialog
        kind={dialog}
        sessionId={s.id}
        available={expected}
        tipsPending={Number(summary.tips_pending ?? 0)}
        onClose={() => setDialog(null)}
        onSaved={onChanged}
      />
    </>
  );
}

function Message({ text, onRetry }: { text: string; onRetry?: () => void }) {
  return (
    <div className="p-10">
      <p className="rounded-xl bg-rose-50 p-4 text-sm text-rose-600">
        {text}{" "}
        {onRetry && (
          <button className="font-semibold underline" onClick={onRetry}>
            Reintentar
          </button>
        )}
      </p>
    </div>
  );
}

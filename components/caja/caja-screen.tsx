"use client";

import { useState } from "react";
import Link from "next/link";
import { ArrowDownCircle, ArrowLeft, MinusCircle, PlusCircle, Search, TrendingUp, Wallet } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { PAYMENT_METHODS } from "@/components/pos/payment-methods";
import { api, type CashActivity, type CashMovementKind, type CashSummary } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCashVersion } from "@/lib/stores/cash";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP, formatTime } from "@/lib/format";
import { cn } from "@/lib/utils";

import { CashCount } from "./cash-count";
import { CashMovementDialog } from "./cash-movement-dialog";
import { OpenShift } from "./open-shift";
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
        <ActivityTable activity={activity} />
        <CashCount summary={summary} onClosed={onClosed} />
      </div>

      <CashMovementDialog
        kind={dialog}
        sessionId={s.id}
        available={expected}
        onClose={() => setDialog(null)}
        onSaved={onChanged}
      />
    </>
  );
}

const FILTERS = [
  { id: "ALL", label: "Todos" },
  { id: "VENTA", label: "Ventas" },
  { id: "INGRESO", label: "Ingresos" },
  { id: "RETIRO", label: "Retiros" },
] as const;

const KIND_LABEL: Record<CashActivity["kind"], string> = {
  APERTURA: "Apertura de caja",
  VENTA: "Venta",
  INGRESO: "Ingreso",
  RETIRO: "Retiro",
  CIERRE: "Cierre",
};

function methodsLabel(methods: string | null) {
  if (!methods) return "—";
  return methods
    .split("+")
    .map((m) => PAYMENT_METHODS.find((p) => p.id === m)?.label ?? m)
    .join(" + ");
}

function ActivityTable({ activity }: { activity: CashActivity[] | null }) {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("ALL");
  const [query, setQuery] = useState("");
  const count = (id: string) => (id === "ALL" ? activity?.length ?? 0 : activity?.filter((a) => a.kind === id).length ?? 0);
  const rows = (activity ?? []).filter(
    (a) =>
      (filter === "ALL" || a.kind === filter) &&
      (!query || `${KIND_LABEL[a.kind]} ${a.detail ?? ""}`.toLowerCase().includes(query.toLowerCase())),
  );

  return (
    <section className="rounded-3xl border border-slate-200/60 bg-white p-6 shadow-sm">
      <h2 className="text-lg font-bold">Movimientos del turno</h2>
      <p className="text-sm text-slate-500">Apertura, ventas, ingresos y retiros, en tiempo real.</p>

      <div className="mt-4 flex flex-wrap items-center gap-2">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            className={cn(
              "rounded-full px-3 py-1.5 text-sm",
              filter === f.id ? "bg-brand font-medium text-white" : "bg-slate-100 text-slate-600 hover:bg-slate-200",
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
              const out = a.kind === "RETIRO";
              return (
                <tr key={`${a.at}-${i}`}>
                  <td className="py-3 pr-4 whitespace-nowrap text-slate-500 tabular-nums">{formatTime(a.at)}</td>
                  <td className="py-3 pr-4">
                    <span className="flex items-center gap-2">
                      <span className={cn("size-2 shrink-0 rounded-full", out ? "bg-rose-500" : a.kind === "INGRESO" ? "bg-sky-500" : "bg-emerald-500")} />
                      <span className={cn(out && "text-rose-600")}>
                        {KIND_LABEL[a.kind]}
                        {a.detail && <span className="text-slate-400"> · {a.detail}</span>}
                      </span>
                    </span>
                  </td>
                  <td className="py-3 pr-4">
                    <span className="rounded-md bg-slate-100 px-2 py-0.5 text-xs text-slate-600">{methodsLabel(a.methods)}</span>
                  </td>
                  <td className="py-3 pr-4 text-slate-500">{a.user_name ?? "—"}</td>
                  <td className={cn("py-3 text-right font-semibold whitespace-nowrap tabular-nums", out ? "text-rose-600" : "text-emerald-600")}>
                    {out ? "−" : "+"}
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

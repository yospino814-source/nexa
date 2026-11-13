"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  AlertTriangle,
  ArrowRight,
  Banknote,
  Calculator,
  CreditCard,
  HandCoins,
  Package,
  Receipt,
  RefreshCw,
  RotateCcw,
  ShoppingBag,
  ShoppingCart,
  TrendingUp,
  Wallet,
  Warehouse,
  type LucideIcon,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { api, friendlyError, type DashboardQuery } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatPercent, formatQty } from "@/lib/format";
import { canOpen } from "@/lib/route-permissions";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { DailySalesChart, SalesTrendChart } from "./sales-trend-chart";

type PeriodPreset = "HOY" | "AYER" | "SIETE_DIAS" | "ESTE_MES" | "MES_ANTERIOR" | "PERSONALIZADO";

const PRESETS: { id: PeriodPreset; label: string }[] = [
  { id: "HOY", label: "Hoy" },
  { id: "AYER", label: "Ayer" },
  { id: "SIETE_DIAS", label: "7 días" },
  { id: "ESTE_MES", label: "Este mes" },
  { id: "MES_ANTERIOR", label: "Mes anterior" },
  { id: "PERSONALIZADO", label: "Personalizado" },
];

const PAYMENT_METHODS: Record<string, { label: string; dotClass: string; barClass: string }> = {
  EFECTIVO: { label: "Efectivo", dotClass: "bg-emerald-500", barClass: "bg-emerald-500" },
  DATAFONO: { label: "Datáfono", dotClass: "bg-slate-700", barClass: "bg-slate-700" },
  QR: { label: "QR / Nequi", dotClass: "bg-brand", barClass: "bg-brand" },
  CREDITO: { label: "Crédito", dotClass: "bg-amber-500", barClass: "bg-amber-500" },
};

const QUICK_LINKS: { href: string; title: string; caption: string; icon: LucideIcon }[] = [
  { href: "/pos", title: "POS mostrador", caption: "Cobrar ventas", icon: Calculator },
  { href: "/ordenes", title: "Órdenes", caption: "Historial de ventas", icon: ShoppingBag },
  { href: "/inventario", title: "Stock info", caption: "Existencias y alertas", icon: Warehouse },
  { href: "/cartera", title: "Cartera", caption: "Cuentas por cobrar", icon: Wallet },
  { href: "/compras", title: "Compras", caption: "Pedidos a proveedores", icon: ShoppingCart },
  { href: "/gastos", title: "Gastos", caption: "Salidas de caja", icon: CreditCard },
];

function localTodayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function addDaysIso(baseIso: string, delta: number): string {
  const [y, m, d] = baseIso.split("-").map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d + delta));
  return `${dt.getUTCFullYear()}-${String(dt.getUTCMonth() + 1).padStart(2, "0")}-${String(dt.getUTCDate()).padStart(2, "0")}`;
}

function rangeForPreset(preset: Exclude<PeriodPreset, "PERSONALIZADO">, todayIso: string): { from?: string; to?: string } {
  if (preset === "HOY") {
    return {};
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

function formatIsoDisplay(iso: string): string {
  const parts = iso.split("-");
  if (parts.length !== 3) return iso;
  return `${parts[2]}/${parts[1]}/${parts[0]}`;
}

type DeltaDirection = "up" | "down" | "flat";

function computeVariation(current: number, previous: number): { label: string; direction: DeltaDirection } {
  if (!Number.isFinite(current) || !Number.isFinite(previous)) {
    return { label: "0.0%", direction: "flat" };
  }
  if (previous === 0) {
    if (current === 0) return { label: "0.0%", direction: "flat" };
    return { label: current > 0 ? "Nuevo" : "Bajó", direction: current > 0 ? "up" : "down" };
  }
  const pct = ((current - previous) / Math.abs(previous)) * 100;
  if (Math.abs(pct) < 0.05) {
    return { label: "0.0%", direction: "flat" };
  }
  return {
    label: `${pct > 0 ? "+" : ""}${pct.toFixed(1)}%`,
    direction: pct > 0 ? "up" : "down",
  };
}

function toneFromDirection(direction: DeltaDirection): "up" | "down" | "neutral" {
  if (direction === "up") return "up";
  if (direction === "down") return "down";
  return "neutral";
}

/** Pantalla 01: panel principal con datos reales de S6 / F7.2. */
export function PanelScreen() {
  const currentBranch = useCurrentBranch();
  const { branches, can } = useSession();

  const [today] = useState(localTodayIso);
  const [preset, setPreset] = useState<PeriodPreset>("HOY");
  const [customFrom, setCustomFrom] = useState(today);
  const [customTo, setCustomTo] = useState(today);
  const [allBranches, setAllBranches] = useState(false);

  const customRangeError =
    preset === "PERSONALIZADO" && (!customFrom || !customTo || customFrom > customTo)
      ? "La fecha inicial debe ser menor o igual que la fecha final."
      : null;

  const query = useMemo<DashboardQuery>(() => {
    const branchId = allBranches && branches.length > 1 ? null : currentBranch.id;
    if (preset === "PERSONALIZADO") {
      return {
        branchId,
        from: customFrom <= customTo ? customFrom : today,
        to: customFrom <= customTo ? customTo : today,
      };
    }
    const r = rangeForPreset(preset, today);
    return { branchId, from: r.from ?? null, to: r.to ?? null };
  }, [allBranches, branches.length, currentBranch.id, customFrom, customTo, preset, today]);

  const queryKey = JSON.stringify(query);
  const { data, loading, error, reload } = useApi(() => api.reports.dashboard(query), queryKey);

  const visibleQuickLinks = QUICK_LINKS.filter((q) => canOpen(q.href, can));

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

  if (!data && loading) {
    return (
      <div className="space-y-6 p-6 lg:p-10">
        <div className="h-14 animate-pulse rounded-2xl bg-slate-200/60" />
        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="h-40 animate-pulse rounded-2xl bg-slate-200/60" />
          ))}
        </div>
        <div className="grid gap-6 xl:grid-cols-2">
          <div className="h-72 animate-pulse rounded-2xl bg-slate-200/60" />
          <div className="h-72 animate-pulse rounded-2xl bg-slate-200/60" />
        </div>
      </div>
    );
  }

  if (!data) return null;

  const k = data.kpis;
  const prev = data.kpis_previous;

  const grossSale = Number(k.gross_sale);
  const prevGrossSale = Number(prev.gross_sale);
  const netSale = Number(k.net_sale);
  const prevNetSale = Number(prev.net_sale);
  const avgTicket = Number(k.avg_ticket);
  const prevAvgTicket = Number(prev.avg_ticket);
  const grossMargin = Number(k.gross_margin);
  const prevGrossMargin = Number(prev.gross_margin);
  const marginPct = Number(k.margin_pct);
  const prevMarginPct = Number(prev.margin_pct);
  const refunds = Number(k.refunds);
  const prevRefunds = Number(prev.refunds);
  const tips = Number(k.tips);
  const prevTips = Number(prev.tips);

  const netDelta = computeVariation(netSale, prevNetSale);
  const grossDelta = computeVariation(grossSale, prevGrossSale);
  const ticketDelta = computeVariation(avgTicket, prevAvgTicket);
  const marginDelta = computeVariation(grossMargin, prevGrossMargin);
  const refundsDelta = computeVariation(refunds, prevRefunds);
  const tipsDelta = computeVariation(tips, prevTips);

  // En el margen, el color se invierte si el costo sube (menor % de margen respecto al periodo anterior).
  const costShareRose =
    k.sales_count > 0 && prev.sales_count > 0 && marginPct < prevMarginPct - 0.005;
  const baseMarginTone = toneFromDirection(marginDelta.direction);
  const marginTone = costShareRose && baseMarginTone === "up" ? "down" : baseMarginTone;

  const paymentsTotal = data.payment_methods.reduce((sum, pm) => sum + Number(pm.amount), 0);
  const periodLabel =
    data.period.from === data.period.to
      ? formatIsoDisplay(data.period.from)
      : `${formatIsoDisplay(data.period.from)} al ${formatIsoDisplay(data.period.to)}`;
  const previousLabel =
    data.previous.from === data.previous.to
      ? formatIsoDisplay(data.previous.from)
      : `${formatIsoDisplay(data.previous.from)} al ${formatIsoDisplay(data.previous.to)}`;

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="flex flex-col gap-4 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm lg:flex-row lg:items-center lg:justify-between">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-base font-semibold text-slate-900">
              {allBranches && branches.length > 1 ? "Todas tus sucursales" : currentBranch.name}
            </h2>
            {loading && <span className="text-xs text-slate-400">Actualizando…</span>}
          </div>
          <p className="text-xs text-slate-500">
            Periodo: {periodLabel} · Comparado vs. {previousLabel}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {branches.length > 1 && (
            <select
              aria-label="Alcance de sucursales"
              value={allBranches ? "ALL" : "CURRENT"}
              onChange={(e) => setAllBranches(e.target.value === "ALL")}
              className="h-9 rounded-xl border border-slate-200 bg-white px-3 text-xs font-medium text-slate-700"
            >
              <option value="CURRENT">Sucursal actual ({currentBranch.name})</option>
              <option value="ALL">Todas las sucursales</option>
            </select>
          )}

          <div className="flex flex-wrap gap-1 rounded-xl bg-slate-100 p-1" role="group" aria-label="Selector de periodo">
            {PRESETS.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setPreset(item.id)}
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
              onChange={(e) => setCustomFrom(e.target.value)}
              className="block h-9 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
            />
          </label>
          <label className="space-y-1 text-xs font-medium text-slate-500">
            Hasta
            <input
              type="date"
              value={customTo}
              onChange={(e) => setCustomTo(e.target.value)}
              className="block h-9 rounded-xl border border-slate-200 px-3 text-sm text-slate-900"
            />
          </label>
          {customRangeError && <p className="text-xs font-medium text-rose-600">{customRangeError}</p>}
        </div>
      )}

      {error && (
        <div className="flex items-center justify-between gap-3 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          <span>{friendlyError(error)}</span>
          <button type="button" onClick={reload} className="font-semibold underline">
            Reintentar
          </button>
        </div>
      )}

      {k.cost_missing_lines > 0 && (
        <div className="flex items-start gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
          <AlertTriangle className="mt-0.5 size-4 shrink-0 text-amber-600" />
          <p>
            Hay <span className="font-semibold tabular-nums">{k.cost_missing_lines}</span>{" "}
            {k.cost_missing_lines === 1 ? "línea vendida" : "líneas vendidas"} sin costo registrado en este periodo.
            El margen bruto puede verse más alto que el real.
          </p>
        </div>
      )}

      <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
        <StatCard
          title="Venta neta"
          value={formatCOP(netSale)}
          caption={`${k.sales_count} ${k.sales_count === 1 ? "venta" : "ventas"} · Anterior: ${formatCOP(prevNetSale)}`}
          icon={TrendingUp}
          tone="default"
          badge={{ label: netDelta.label, tone: toneFromDirection(netDelta.direction) }}
        />
        <StatCard
          title="Venta bruta"
          value={formatCOP(grossSale)}
          caption={`Incluye impuestos · Anterior: ${formatCOP(prevGrossSale)}`}
          icon={Receipt}
          tone="default"
          badge={{ label: grossDelta.label, tone: toneFromDirection(grossDelta.direction) }}
        />
        <StatCard
          title="Margen bruto"
          value={formatCOP(grossMargin)}
          caption={`${formatPercent(marginPct)} de margen${costShareRose ? " · costo subió" : ""}`}
          icon={Banknote}
          tone={costShareRose ? "danger" : "success"}
          badge={{ label: marginDelta.label, tone: marginTone }}
        />
        <StatCard
          title="Ticket promedio"
          value={formatCOP(avgTicket)}
          caption={`Anterior: ${formatCOP(prevAvgTicket)}`}
          icon={ShoppingBag}
          tone="default"
          badge={{ label: ticketDelta.label, tone: toneFromDirection(ticketDelta.direction) }}
        />
        <StatCard
          title="Devoluciones"
          value={formatCOP(refunds)}
          caption={`Anterior: ${formatCOP(prevRefunds)}`}
          icon={RotateCcw}
          tone={refunds > 0 ? "warning" : "default"}
          badge={{ label: refundsDelta.label, tone: toneFromDirection(refundsDelta.direction) }}
        />
        <StatCard
          title="Propinas"
          value={formatCOP(tips)}
          caption={`Anterior: ${formatCOP(prevTips)}`}
          icon={HandCoins}
          tone="default"
          badge={{ label: tipsDelta.label, tone: toneFromDirection(tipsDelta.direction) }}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-2">
        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4">
            <h2 className="text-lg font-semibold">Ventas por hora</h2>
            <p className="text-sm text-slate-500">Distribución horaria de la venta bruta (00:00 a 23:00)</p>
          </div>
          <SalesTrendChart byHour={data.by_hour} />
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4">
            <h2 className="text-lg font-semibold">Ventas por día</h2>
            <p className="text-sm text-slate-500">Evolución diaria de la venta bruta en el periodo</p>
          </div>
          <DailySalesChart byDay={data.by_day} />
        </section>
      </div>

      <div className="grid gap-6 xl:grid-cols-3">
        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm xl:col-span-2">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold">Top 10 productos</h2>
              <p className="text-sm text-slate-500">Ordenados por total vendido sin impuesto</p>
            </div>
            <Package className="size-5 text-slate-400" />
          </div>

          {data.top_products.length === 0 ? (
            <p className="py-8 text-center text-sm text-slate-400">No hay productos vendidos en este periodo.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-100 text-left text-xs font-semibold text-slate-400 uppercase">
                    <th className="py-2 pr-3">#</th>
                    <th className="py-2 pr-3">Producto</th>
                    <th className="py-2 pr-3 text-right">Cantidad</th>
                    <th className="py-2 text-right">Total</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {data.top_products.map((item, idx) => (
                    <tr key={`${item.product_id ?? "free"}-${idx}`}>
                      <td className="py-2.5 pr-3 text-xs font-semibold text-slate-400 tabular-nums">{idx + 1}</td>
                      <td className="py-2.5 pr-3 font-medium text-slate-900">{item.name}</td>
                      <td className="py-2.5 pr-3 text-right text-slate-600 tabular-nums">{formatQty(item.qty)}</td>
                      <td className="py-2.5 text-right font-semibold text-slate-900 tabular-nums">
                        {formatCOP(Number(item.line_total))}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold">Métodos de pago</h2>
              <p className="text-sm text-slate-500">Pagos de órdenes cobradas</p>
            </div>
            <span className="text-xs font-semibold text-slate-500 tabular-nums">{formatCOP(paymentsTotal)}</span>
          </div>

          {data.payment_methods.length === 0 ? (
            <p className="py-8 text-center text-sm text-slate-400">Sin pagos registrados en este periodo.</p>
          ) : (
            <ul className="space-y-4">
              {data.payment_methods.map((pm) => {
                const amount = Number(pm.amount);
                const share = paymentsTotal > 0 ? Math.round((amount / paymentsTotal) * 100) : 0;
                const meta = PAYMENT_METHODS[pm.method] ?? {
                  label: pm.method,
                  dotClass: "bg-slate-500",
                  barClass: "bg-slate-500",
                };
                return (
                  <li key={pm.method} className="space-y-1.5">
                    <div className="flex items-center justify-between text-sm">
                      <span className="flex items-center gap-2 font-medium text-slate-800">
                        <span className={cn("size-2.5 rounded-full", meta.dotClass)} />
                        {meta.label}
                      </span>
                      <span className="font-semibold text-slate-900 tabular-nums">{formatCOP(amount)}</span>
                    </div>
                    <div className="h-2 w-full overflow-hidden rounded-full bg-slate-100">
                      <div
                        className={cn("h-full rounded-full transition-all", meta.barClass)}
                        style={{ width: `${Math.max(share, amount > 0 ? 3 : 0)}%` }}
                      />
                    </div>
                    <div className="flex justify-between text-xs text-slate-400 tabular-nums">
                      <span>
                        {pm.count} {pm.count === 1 ? "pago" : "pagos"}
                      </span>
                      <span>{share}%</span>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </div>

      <div className="grid gap-6 xl:grid-cols-3">
        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4 flex items-center justify-between gap-2">
            <div>
              <h2 className="text-lg font-semibold">Stock bajo</h2>
              <p className="text-sm text-slate-500">Alertas abiertas en bodega</p>
            </div>
            {data.low_stock.length > 0 && (
              <span className="rounded-full bg-amber-50 px-2.5 py-0.5 text-xs font-semibold text-amber-700 tabular-nums">
                {data.low_stock.length} {data.low_stock.length === 1 ? "alerta" : "alertas"}
              </span>
            )}
          </div>

          {data.low_stock.length === 0 ? (
            <p className="py-6 text-sm text-slate-400">No hay productos con alerta de stock bajo.</p>
          ) : (
            <ul className="space-y-2.5">
              {data.low_stock.map((item) => (
                <li
                  key={`${item.product_id}-${item.location_name}`}
                  className="flex items-center justify-between gap-3 rounded-xl border border-slate-100 px-3 py-2.5 text-sm"
                >
                  <div className="min-w-0">
                    <p className="truncate font-medium text-slate-900">{item.name}</p>
                    <p className="text-xs text-slate-400">{item.location_name}</p>
                  </div>
                  <div className="shrink-0 text-right tabular-nums">
                    <p className="font-semibold text-amber-700">{formatQty(item.on_hand)}</p>
                    <p className="text-[11px] text-slate-400">Mín. {formatQty(item.min_qty)}</p>
                  </div>
                </li>
              ))}
            </ul>
          )}

          {canOpen("/inventario", can) && (
            <Link
              href="/inventario"
              className="mt-4 inline-flex items-center gap-1 text-sm font-semibold text-brand hover:underline"
            >
              Ver inventario <ArrowRight className="size-4" />
            </Link>
          )}
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4 flex items-center justify-between gap-2">
            <div>
              <h2 className="text-lg font-semibold">Cartera vencida</h2>
              <p className="text-sm text-slate-500">Cuentas abiertas con cuotas vencidas</p>
            </div>
            <span
              className={cn(
                "flex size-10 items-center justify-center rounded-full",
                data.overdue_receivables.count > 0 ? "bg-rose-50 text-rose-600" : "bg-emerald-50 text-emerald-600",
              )}
            >
              <Wallet className="size-5" />
            </span>
          </div>

          <div className="rounded-2xl border border-slate-100 bg-slate-50/70 p-4">
            <p className="text-xs font-medium tracking-wider text-slate-400 uppercase">Saldo vencido</p>
            <p className="mt-1 text-2xl font-bold text-slate-900 tabular-nums">
              {formatCOP(Number(data.overdue_receivables.amount))}
            </p>
            <p className="mt-1 text-sm text-slate-500 tabular-nums">
              {data.overdue_receivables.count}{" "}
              {data.overdue_receivables.count === 1 ? "cuenta vencida" : "cuentas vencidas"}
            </p>
          </div>

          {canOpen("/cartera", can) && (
            <Link
              href="/cartera"
              className="mt-4 inline-flex items-center gap-1 text-sm font-semibold text-brand hover:underline"
            >
              Ir a cartera <ArrowRight className="size-4" />
            </Link>
          )}
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <h2 className="text-lg font-semibold">Accesos rápidos</h2>
          <p className="mb-4 text-sm text-slate-500">Módulos disponibles para tu rol</p>
          {visibleQuickLinks.length === 0 ? (
            <p className="text-sm text-slate-400">No tenés accesos directos adicionales.</p>
          ) : (
            <div className="grid grid-cols-2 gap-3">
              {visibleQuickLinks.map((q) => (
                <Link
                  key={q.href}
                  href={q.href}
                  className="rounded-2xl border border-slate-200/60 p-3.5 transition-colors hover:border-brand/40 hover:bg-teal-50/40"
                >
                  <span className="flex size-9 items-center justify-center rounded-full bg-slate-100 text-slate-700">
                    <q.icon className="size-4" />
                  </span>
                  <p className="mt-2.5 text-sm font-medium">{q.title}</p>
                  <p className="text-xs text-slate-400">{q.caption}</p>
                </Link>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

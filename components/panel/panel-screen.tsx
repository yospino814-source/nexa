"use client";

import Link from "next/link";
import {
  AlertTriangle,
  ArrowRight,
  BarChart3,
  Banknote,
  CreditCard,
  Receipt,
  ShoppingBag,
  ShoppingCart,
  Tag,
  TrendingUp,
  Truck,
  Wallet,
  Warehouse,
  XCircle,
  type LucideIcon,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { api } from "@/lib/api";
import { useCurrentBranch } from "@/lib/stores/session";
import { useApi } from "@/lib/api/use-api";
import { formatCOP, formatCOPShort } from "@/lib/format";

import { SalesTrendChart } from "./sales-trend-chart";

const CHANNELS = {
  WHATSAPP: { label: "WhatsApp", color: "#10b981" },
  POS: { label: "Punto de Venta", color: "#334155" },
  TIENDA_ONLINE: { label: "Tienda Online", color: "var(--brand)" },
} as const;

const QUICK_LINKS: { href: string; title: string; caption: string; icon: LucideIcon }[] = [
  { href: "/ventas", title: "Resumen ventas", caption: "Totales e IVA", icon: BarChart3 },
  { href: "/ventas/productos", title: "Por producto", caption: "Ranking SKUs", icon: Tag },
  { href: "/ordenes", title: "Órdenes", caption: "Historial del día", icon: ShoppingBag },
  { href: "/inventario", title: "Stock info", caption: "Kardex rápido", icon: Warehouse },
  { href: "/gastos", title: "Gastos", caption: "Salidas de caja", icon: CreditCard },
  { href: "/compras", title: "Compras", caption: "Recepción de productos", icon: ShoppingCart },
];

const pct = (v: string) => `${Number(v) >= 0 ? "+" : ""}${Number(v).toFixed(1)}%`;

/** Pantalla 01: panel principal. */
export function PanelScreen() {
  const branchId = useCurrentBranch().id;
  const { data, loading, error, reload } = useApi(() => api.reports.dashboard(branchId), branchId);

  if (error) {
    return (
      <div className="p-10">
        <p className="rounded-xl bg-rose-50 p-4 text-sm text-rose-600">
          No se pudo cargar el panel.{" "}
          <button className="font-semibold underline" onClick={reload}>
            Reintentar
          </button>
        </p>
      </div>
    );
  }

  if (loading || !data) {
    return (
      <div className="grid gap-6 p-6 lg:p-10 xl:grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="h-44 animate-pulse rounded-2xl bg-slate-200/60" />
        ))}
        <div className="h-96 animate-pulse rounded-2xl bg-slate-200/60 xl:col-span-4" />
      </div>
    );
  }

  const channelTotal = data.channels.reduce((sum, c) => sum + Number(c.amount), 0);
  const alerts = [
    { count: data.alerts.low_stock, title: "productos con stock bajo", caption: "Requieren reorden pronto", icon: AlertTriangle, tone: "bg-amber-50 text-amber-600", href: "/inventario" },
    { count: data.alerts.out_of_stock, title: "productos agotados", caption: "Afectan las ventas de hoy", icon: XCircle, tone: "bg-rose-50 text-rose-600", href: "/inventario" },
    { count: data.alerts.pending_dispatch, title: "despachos pendientes de entrega", caption: "Rutas asignadas", icon: Truck, tone: "bg-sky-50 text-sky-600", href: "/rutas" },
  ].filter((a) => a.count > 0);

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          title="Ingresos"
          value={formatCOP(Number(data.revenue))}
          caption={`Sin IVA: ${formatCOP(Number(data.revenue_ex_tax))}`}
          icon={TrendingUp}
          badge={{ label: pct(data.revenue_change_pct), tone: Number(data.revenue_change_pct) >= 0 ? "up" : "down" }}
        />
        <StatCard
          title="Ganancias"
          value={formatCOP(Number(data.profit))}
          caption={`${Number(data.profit_margin_pct)}% de margen`}
          icon={Banknote}
          tone="success"
          badge={{ label: pct(data.profit_change_pct), tone: Number(data.profit_change_pct) >= 0 ? "up" : "down" }}
        />
        <StatCard
          title="Balance"
          value={formatCOP(Number(data.balance))}
          caption="Caja acumulada"
          icon={Wallet}
          badge={{ label: "En línea", tone: "neutral" }}
        />
        <StatCard
          title="Gastos"
          value={formatCOP(Number(data.expenses))}
          caption="Sin base de comparación"
          icon={Receipt}
          tone="warning"
          badge={{ label: "Turno actual", tone: "warning" }}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-3">
        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm xl:col-span-2">
          <h2 className="text-lg font-semibold">Tendencia de ventas</h2>
          <p className="mb-6 text-sm text-slate-500">Ingresos acumulados vs. volumen de órdenes por hora</p>
          <SalesTrendChart hourly={data.hourly} />
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <h2 className="text-lg font-semibold">Ventas por canal</h2>
          <p className="text-sm text-slate-500">Distribución porcentual de hoy</p>
          <ChannelDonut channels={data.channels} total={channelTotal} />
          <ul className="space-y-3 border-t border-slate-100 pt-4 text-sm">
            {data.channels.map((c) => (
              <li key={c.channel} className="flex items-center justify-between">
                <span className="flex items-center gap-2">
                  <span className="size-2.5 rounded-full" style={{ background: CHANNELS[c.channel].color }} />
                  {CHANNELS[c.channel].label} ({Math.round((Number(c.amount) / channelTotal) * 100)}%)
                </span>
                <span className="font-semibold tabular-nums">{formatCOP(Number(c.amount))}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>

      <div className="grid gap-6 xl:grid-cols-2">
        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <div className="mb-4 flex items-center gap-3">
            <h2 className="text-lg font-semibold">Alertas operativas</h2>
            {alerts.length > 0 && (
              <span className="rounded-full bg-rose-50 px-2.5 py-0.5 text-xs font-semibold text-rose-600">
                {alerts.length} pendientes
              </span>
            )}
          </div>
          <ul className="space-y-3">
            {alerts.map((a) => (
              <li key={a.title} className="flex items-center gap-4 rounded-2xl border border-slate-100 p-4">
                <span className={`flex size-11 items-center justify-center rounded-full ${a.tone}`}>
                  <a.icon className="size-5" />
                </span>
                <div className="flex-1">
                  <p className="font-medium">
                    {a.count} {a.title}
                  </p>
                  <p className="text-sm text-slate-500">{a.caption}</p>
                </div>
                <Link href={a.href} className="flex items-center gap-1 text-sm font-semibold text-brand hover:underline">
                  Ver detalle <ArrowRight className="size-4" />
                </Link>
              </li>
            ))}
            {alerts.length === 0 && <p className="text-sm text-slate-500">Todo en orden por ahora.</p>}
          </ul>
        </section>

        <section className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
          <h2 className="text-lg font-semibold">Accesos rápidos</h2>
          <p className="mb-4 text-sm text-slate-500">Acciones frecuentes del turno</p>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {QUICK_LINKS.map((q) => (
              <Link
                key={q.href}
                href={q.href}
                className="rounded-2xl border border-slate-200/60 p-4 transition-colors hover:border-brand/40 hover:bg-teal-50/40"
              >
                <span className="flex size-10 items-center justify-center rounded-full bg-slate-100 text-slate-700">
                  <q.icon className="size-5" />
                </span>
                <p className="mt-3 font-medium">{q.title}</p>
                <p className="text-xs text-slate-400">{q.caption}</p>
              </Link>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

function ChannelDonut({ channels, total }: { channels: { channel: keyof typeof CHANNELS; amount: string }[]; total: number }) {
  const r = 70;
  const circumference = 2 * Math.PI * r;
  const lengths = channels.map((c) => (Number(c.amount) / total) * circumference);
  const offsets = lengths.map((_, i) => lengths.slice(0, i).reduce((a, b) => a + b, 0));

  return (
    <div className="relative mx-auto my-6 size-52">
      <svg viewBox="0 0 180 180" className="-rotate-90">
        {channels.map((c, i) => (
          <circle
            key={c.channel}
            cx="90"
            cy="90"
            r={r}
            fill="none"
            stroke={CHANNELS[c.channel].color}
            strokeWidth="18"
            strokeDasharray={`${Math.max(lengths[i] - 3, 0)} ${circumference}`}
            strokeDashoffset={-offsets[i]}
          />
        ))}
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-xs tracking-wider text-slate-400 uppercase">Total</span>
        <span className="text-2xl font-bold">{formatCOPShort(total)}</span>
      </div>
    </div>
  );
}

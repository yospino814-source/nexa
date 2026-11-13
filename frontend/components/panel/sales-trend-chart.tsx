import type { DashboardDailyPoint, DashboardHourlyPoint } from "@/lib/api";
import { formatCOP, formatCOPShort } from "@/lib/format";
import { cn } from "@/lib/utils";

function shortDayLabel(iso: string): string {
  const parts = iso.split("-");
  if (parts.length !== 3) return iso;
  return `${parts[2]}/${parts[1]}`;
}

function fullDayLabel(iso: string): string {
  const parts = iso.split("-");
  if (parts.length !== 3) return iso;
  return `${parts[2]}/${parts[1]}/${parts[0]}`;
}

/** Gráfica de barras por hora (00..23) construida únicamente con div y CSS. */
export function SalesTrendChart({ byHour }: { byHour: DashboardHourlyPoint[] }) {
  const values = byHour.map((h) => Number(h.gross_sale));
  const maxGross = Math.max(...values, 0);
  const hasSales = values.some((v) => v > 0);
  const peakIdx = hasSales ? values.indexOf(maxGross) : -1;
  const peak = peakIdx >= 0 ? byHour[peakIdx] : null;

  if (!hasSales || byHour.length === 0) {
    return (
      <div className="flex h-56 items-center justify-center rounded-2xl border border-dashed border-slate-200 bg-slate-50/60 text-sm text-slate-400">
        Sin ventas registradas por hora en este periodo.
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {peak && (
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-slate-500">
          <span>Hora pico: {String(peak.hour).padStart(2, "0")}:00</span>
          <span className="rounded-full bg-brand/10 px-2.5 py-0.5 font-semibold text-brand tabular-nums">
            {formatCOP(Number(peak.gross_sale))} · {peak.sales_count}{" "}
            {peak.sales_count === 1 ? "venta" : "ventas"}
          </span>
        </div>
      )}

      <div className="relative pt-4">
        <div className="pointer-events-none absolute inset-x-0 top-4 bottom-0 flex flex-col justify-between">
          {[1, 0.75, 0.5, 0.25].map((ratio) => (
            <div key={ratio} className="flex items-center gap-2 border-b border-dashed border-slate-200/70">
              <span className="-translate-y-2 text-[10px] text-slate-400 tabular-nums">
                {formatCOPShort(maxGross * ratio)}
              </span>
            </div>
          ))}
        </div>

        <div className="relative z-10 flex h-48 items-end gap-1 pl-10 sm:gap-1.5" role="img" aria-label="Ventas por hora">
          {byHour.map((h, idx) => {
            const gross = Number(h.gross_sale);
            const pct = maxGross > 0 ? Math.round((gross / maxGross) * 100) : 0;
            const heightPct = gross > 0 ? Math.max(pct, 5) : 2;
            const isPeak = idx === peakIdx;
            const tooltipAlign =
              idx <= 3 ? "left-0" : idx >= byHour.length - 4 ? "right-0" : "left-1/2 -translate-x-1/2";

            return (
              <div key={h.hour} className="group relative flex h-full flex-1 flex-col items-center justify-end">
                <div
                  className={cn(
                    "pointer-events-none absolute -top-9 z-20 hidden rounded-lg bg-slate-900 px-2.5 py-1 text-[11px] font-medium whitespace-nowrap text-white shadow-lg group-hover:block",
                    tooltipAlign,
                  )}
                >
                  {String(h.hour).padStart(2, "0")}:00 · {formatCOP(gross)} ({h.sales_count})
                </div>
                <div
                  className={cn(
                    "w-full rounded-t-md transition-all",
                    gross === 0 && "bg-slate-200/70",
                    gross > 0 && !isPeak && "bg-brand/45 group-hover:bg-brand/80",
                    isPeak && "bg-brand",
                  )}
                  style={{ height: `${heightPct}%` }}
                />
              </div>
            );
          })}
        </div>

        <div className="mt-2 flex items-center gap-1 pl-10 sm:gap-1.5">
          {byHour.map((h, idx) => {
            const showLabel = h.hour % 3 === 0 || h.hour === 23;
            const isPeak = idx === peakIdx;
            return (
              <span
                key={h.hour}
                className={cn(
                  "flex-1 text-center text-[10px] tabular-nums",
                  isPeak ? "font-semibold text-brand" : "text-slate-400",
                )}
              >
                {showLabel ? String(h.hour).padStart(2, "0") : ""}
              </span>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** Gráfica de barras por día (from..to) construida únicamente con div y CSS. */
export function DailySalesChart({ byDay }: { byDay: DashboardDailyPoint[] }) {
  const values = byDay.map((d) => Number(d.gross_sale));
  const maxGross = Math.max(...values, 0);
  const hasSales = values.some((v) => v > 0);
  const peakIdx = hasSales ? values.indexOf(maxGross) : -1;
  const peak = peakIdx >= 0 ? byDay[peakIdx] : null;

  if (!hasSales || byDay.length === 0) {
    return (
      <div className="flex h-56 items-center justify-center rounded-2xl border border-dashed border-slate-200 bg-slate-50/60 text-sm text-slate-400">
        Sin ventas registradas por día en este periodo.
      </div>
    );
  }

  const labelStep = byDay.length > 20 ? 3 : byDay.length > 10 ? 2 : 1;
  const needsScroll = byDay.length > 14;

  return (
    <div className="space-y-3">
      {peak && (
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-slate-500">
          <span>Mejor día: {fullDayLabel(peak.date)}</span>
          <span className="rounded-full bg-emerald-50 px-2.5 py-0.5 font-semibold text-emerald-700 tabular-nums">
            {formatCOP(Number(peak.gross_sale))} · {peak.sales_count}{" "}
            {peak.sales_count === 1 ? "venta" : "ventas"}
          </span>
        </div>
      )}

      <div className="overflow-x-auto pb-1">
        <div
          className="relative pt-4"
          style={needsScroll ? { minWidth: `${byDay.length * 32}px` } : undefined}
        >
          <div className="pointer-events-none absolute inset-x-0 top-4 bottom-0 flex flex-col justify-between">
            {[1, 0.75, 0.5, 0.25].map((ratio) => (
              <div key={ratio} className="flex items-center gap-2 border-b border-dashed border-slate-200/70">
                <span className="-translate-y-2 text-[10px] text-slate-400 tabular-nums">
                  {formatCOPShort(maxGross * ratio)}
                </span>
              </div>
            ))}
          </div>

          <div
            className={cn(
              "relative z-10 flex h-48 items-end gap-2 pl-10",
              byDay.length === 1 && "justify-center",
            )}
            role="img"
            aria-label="Ventas por día"
          >
            {byDay.map((d, idx) => {
              const gross = Number(d.gross_sale);
              const pct = maxGross > 0 ? Math.round((gross / maxGross) * 100) : 0;
              const heightPct = gross > 0 ? Math.max(pct, 5) : 2;
              const isPeak = idx === peakIdx;
              const tooltipAlign =
                idx === 0
                  ? "left-0"
                  : idx >= byDay.length - 1
                    ? "right-0"
                    : "left-1/2 -translate-x-1/2";

              return (
                <div
                  key={d.date}
                  className={cn(
                    "group relative flex h-full flex-col items-center justify-end",
                    byDay.length === 1 ? "w-20" : "flex-1 min-w-[20px]",
                  )}
                >
                  <div
                    className={cn(
                      "pointer-events-none absolute -top-9 z-20 hidden rounded-lg bg-slate-900 px-2.5 py-1 text-[11px] font-medium whitespace-nowrap text-white shadow-lg group-hover:block",
                      tooltipAlign,
                    )}
                  >
                    {fullDayLabel(d.date)} · {formatCOP(gross)} ({d.sales_count})
                  </div>
                  <div
                    className={cn(
                      "w-full rounded-t-md transition-all",
                      gross === 0 && "bg-slate-200/70",
                      gross > 0 && !isPeak && "bg-emerald-500/45 group-hover:bg-emerald-600/80",
                      isPeak && "bg-emerald-600",
                    )}
                    style={{ height: `${heightPct}%` }}
                  />
                </div>
              );
            })}
          </div>

          <div
            className={cn(
              "mt-2 flex items-center gap-2 pl-10",
              byDay.length === 1 && "justify-center",
            )}
          >
            {byDay.map((d, idx) => {
              const showLabel = idx % labelStep === 0 || idx === byDay.length - 1;
              const isPeak = idx === peakIdx;
              return (
                <span
                  key={d.date}
                  className={cn(
                    "text-center text-[10px] tabular-nums",
                    byDay.length === 1 ? "w-20" : "flex-1 min-w-[20px]",
                    isPeak ? "font-semibold text-emerald-700" : "text-slate-400",
                  )}
                >
                  {showLabel ? shortDayLabel(d.date) : ""}
                </span>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

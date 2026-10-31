import type { DashboardSummary } from "@/lib/api";
import { formatCOP } from "@/lib/format";

const W = 600;
const H = 200;

/** Curva de ingresos por hora (area) + barras de volumen de ordenes. */
export function SalesTrendChart({ hourly }: { hourly: DashboardSummary["hourly"] }) {
  const revenue = hourly.map((h) => Number(h.revenue));
  const maxRevenue = Math.max(...revenue, 1);
  const maxOrders = Math.max(...hourly.map((h) => h.orders), 1);
  const peak = revenue.indexOf(maxRevenue);
  const step = W / hourly.length;
  const x = (i: number) => step * i + step / 2;
  const y = (v: number) => H - (v / maxRevenue) * (H - 30);

  const points = revenue.map((v, i) => [x(i), y(v)] as const);
  // Curva suave con segmentos cubicos horizontales.
  const line = points.reduce(
    (d, [px, py], i) => {
      if (i === 0) return `M ${px} ${py}`;
      const [prevX, prevY] = points[i - 1];
      const mid = (prevX + px) / 2;
      return `${d} C ${mid} ${prevY}, ${mid} ${py}, ${px} ${py}`;
    },
    "",
  );
  const area = `${line} L ${points.at(-1)![0]} ${H} L ${points[0][0]} ${H} Z`;

  return (
    <div>
      <div className="relative">
        <svg viewBox={`0 0 ${W} ${H}`} className="h-56 w-full" preserveAspectRatio="none" role="img" aria-label="Tendencia de ventas por hora">
          <defs>
            <linearGradient id="trend-fill" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor="var(--brand)" stopOpacity="0.25" />
              <stop offset="100%" stopColor="var(--brand)" stopOpacity="0" />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75, 1].map((f) => (
            <line key={f} x1="0" x2={W} y1={H * f} y2={H * f} stroke="#e2e8f0" strokeWidth="1" />
          ))}
          {hourly.map((h, i) => {
            const barH = (h.orders / maxOrders) * (H - 60);
            return (
              <rect
                key={h.hour}
                x={x(i) - step * 0.18}
                y={H - barH}
                width={step * 0.36}
                height={barH}
                rx="3"
                fill={i === peak ? "var(--brand)" : "#94a3b8"}
                opacity={i === peak ? 0.35 : 0.25}
              />
            );
          })}
          <path d={area} fill="url(#trend-fill)" />
          <path d={line} fill="none" stroke="var(--brand)" strokeWidth="2.5" vectorEffect="non-scaling-stroke" />
        </svg>
        <div
          className="pointer-events-none absolute -translate-x-1/2 rounded-full bg-slate-900 px-3 py-1 text-xs font-medium whitespace-nowrap text-white shadow-lg"
          style={{ left: `${(x(peak) / W) * 100}%`, top: 0 }}
        >
          Pico: <span className="text-teal-300">{formatCOP(maxRevenue)}</span> a las {String(hourly[peak].hour).padStart(2, "0")}:00
        </div>
      </div>
      <div className="mt-2 flex justify-around text-xs text-slate-400">
        {hourly.map((h, i) => (
          <span key={h.hour} className={i === peak ? "font-semibold text-brand" : undefined}>
            {String(h.hour).padStart(2, "0")}:00
          </span>
        ))}
      </div>
    </div>
  );
}

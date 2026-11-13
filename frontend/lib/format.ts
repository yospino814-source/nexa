const copNumber = new Intl.NumberFormat("es-CO", { maximumFractionDigits: 0 });

/** Pesos colombianos sin decimales: $1.250.000 */
export function formatCOP(value: number): string {
  const sign = value < 0 ? "-" : "";
  return `${sign}$${copNumber.format(Math.abs(Math.round(value)))}`;
}

/** Version corta para graficos: $44.8M, $850K */
export function formatCOPShort(value: number): string {
  if (Math.abs(value) >= 1_000_000) return `$${(value / 1_000_000).toFixed(1)}M`;
  if (Math.abs(value) >= 1_000) return `$${Math.round(value / 1_000)}K`;
  return formatCOP(value);
}

export function formatPercent(value: number): string {
  return `${value.toFixed(1)}%`;
}

/** Decimal del backend (string) a numero. `null` se mantiene. */
export function toNumber(value: string | null | undefined): number | null {
  if (value == null) return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

/** Etiqueta del impuesto a partir de la fraccion (0.19 -> "IVA 19%"). */
export function taxLabel(rate: number | null): string {
  return rate ? `IVA ${Math.round(rate * 100)}%` : "Exento";
}

/** Lee un monto escrito por el usuario ("50.000", "$ 1.200") como pesos enteros. */
export function parseCOP(text: string): number {
  const digits = text.replace(/[^\d]/g, "");
  return digits ? Number(digits) : 0;
}

/** Cantidad sin ceros sobrantes: "12.0000" -> "12", "1.5000" -> "1,5". */
export function formatQty(value: string | number | null | undefined): string {
  const n = Number(value ?? 0);
  return new Intl.NumberFormat("es-CO", { maximumFractionDigits: 3 }).format(n);
}

/** Fecha y hora cortas en Colombia: "31/10 13:05". */
export function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString("es-CO", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString("es-CO", { hour: "2-digit", minute: "2-digit", hour12: false });
}

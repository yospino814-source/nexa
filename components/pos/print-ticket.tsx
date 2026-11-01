"use client";

import { PrintArea } from "@/components/nexa/print-area";

import type { BusinessInfo, CheckoutResponse, PaymentMethod, QuoteResponse } from "@/lib/api";
import { formatCOP } from "@/lib/format";

const METHOD_LABELS: Record<PaymentMethod, string> = {
  EFECTIVO: "Efectivo",
  DATAFONO: "Datáfono",
  QR: "QR / Transferencia",
};

const DISCOUNT_LABELS = {
  manual: "Descuento",
  promotion: "Promoción",
  order: "Promoción del pedido",
  coupon: "Cupón",
} as const;

type TicketLine = {
  name: string;
  qty: number;
  unitPrice: number;
  gross: number;
  discounts: { label: string; amount: number }[];
};

/** Todo lo que imprime la tirilla, ya calculado (F1.15). */
export type TicketModel = {
  /** "Comprobante POS-000123", "Factura…" o "PRECUENTA - NO VÁLIDA COMO FACTURA". */
  title: string;
  date: Date;
  cashier: string | null;
  register: string | null;
  customer: string;
  waiter: string | null;
  lines: TicketLine[];
  subtotal: number;
  discountTotal: number;
  taxes: { label: string; amount: number }[];
  tip: number;
  total: number;
  /** null en la precuenta: sin pagos ni bloque fiscal. */
  payments: { label: string; amount: number; tendered: number | null }[] | null;
  change: number;
  fiscalAccepted: boolean;
};

/** Metadatos que no vienen en la venta: quién, dónde y para quién. */
export type TicketMeta = {
  cashier: string | null;
  register: string | null;
  customer: string;
  waiter: string | null;
};

function taxLabelFor(rate: number): string {
  if (Math.abs(rate - 0.08) < 1e-9) return "INC 8 %";
  return `IVA ${Math.round(rate * 100)} %`;
}

/** Impuesto agrupado por tasa, en orden de tasa descendente, sin las de 0. */
function taxesByRate(items: { rate: number; amount: number }[]): { label: string; amount: number }[] {
  const byRate = new Map<number, number>();
  for (const { rate, amount } of items) {
    if (rate <= 0 || amount === 0) continue;
    byRate.set(rate, (byRate.get(rate) ?? 0) + amount);
  }
  return [...byRate.entries()]
    .sort((a, b) => b[0] - a[0])
    .map(([rate, amount]) => ({ label: taxLabelFor(rate), amount }));
}

function nameWithPackage(name: string, units: number): string {
  return units > 1 ? `${name} x${units}` : name;
}

function discountsOf(d: { manual?: string; promotion?: string; order?: string; coupon?: string }) {
  return (Object.keys(DISCOUNT_LABELS) as (keyof typeof DISCOUNT_LABELS)[])
    .map((k) => ({ label: DISCOUNT_LABELS[k], amount: Number(d[k] ?? 0) }))
    .filter((x) => x.amount > 0);
}

/** Tirilla de una venta registrada: todos los valores salen del servidor. */
export function ticketFromSale(sale: CheckoutResponse, meta: TicketMeta, date = new Date()): TicketModel {
  const lines: TicketLine[] = sale.items.map((it) => {
    const qty = Number(it.qty);
    const unitPrice = Number(it.unit_price);
    return {
      name: nameWithPackage(it.name ?? it.custom_name ?? "", Number(it.units_per_package ?? 1)),
      qty,
      unitPrice,
      gross: Math.round(qty * unitPrice * 100) / 100,
      discounts: discountsOf({
        manual: it.discount_manual,
        promotion: it.discount_promotion,
        order: it.discount_order,
        coupon: it.discount_coupon,
      }),
    };
  });
  const title = sale.receipt_number.startsWith("FE")
    ? `Factura electrónica de venta ${sale.receipt_number}`
    : `Comprobante ${sale.receipt_number}`;
  return {
    title,
    date,
    ...meta,
    lines,
    subtotal: Number(sale.order.subtotal),
    discountTotal: Number(sale.order.discount_total ?? 0),
    taxes: taxesByRate(sale.items.map((it) => ({ rate: Number(it.tax_rate), amount: Number(it.tax_amount) }))),
    tip: Number(sale.order.tip_amount),
    total: Number(sale.order.total_amount),
    payments: sale.payments.map((p) => ({
      label: METHOD_LABELS[p.method] ?? p.method,
      amount: Number(p.amount),
      tendered: p.tendered != null ? Number(p.tendered) : null,
    })),
    change: Number(sale.change_due),
    fiscalAccepted: sale.fiscal_status === "ACEPTADO",
  };
}

/** Precuenta: misma plantilla con la última cotización, sin número ni pagos. */
export function ticketFromQuote(quote: QuoteResponse, meta: TicketMeta, date = new Date()): TicketModel {
  return {
    title: "PRECUENTA - NO VÁLIDA COMO FACTURA",
    date,
    ...meta,
    lines: quote.lines.map((l) => ({
      name: nameWithPackage(l.name, Number(l.units_per_package)),
      qty: Number(l.qty),
      unitPrice: Number(l.unit_price),
      gross: Number(l.gross),
      discounts: discountsOf(l.discount),
    })),
    subtotal: Number(quote.subtotal),
    discountTotal: Number(quote.discount_total),
    taxes: taxesByRate(quote.lines.map((l) => ({ rate: Number(l.tax_rate), amount: Number(l.tax_amount) }))),
    tip: Number(quote.tip_amount),
    total: Number(quote.total),
    payments: null,
    change: 0,
    fiscalAccepted: false,
  };
}

/** DD/MM/YYYY HH:mm en la zona de la sucursal. */
function formatTicketDate(date: Date, timeZone: string): string {
  const parts = new Intl.DateTimeFormat("es-CO", {
    timeZone,
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).formatToParts(date);
  const get = (t: string) => parts.find((p) => p.type === t)?.value ?? "";
  return `${get("day")}/${get("month")}/${get("year")} ${get("hour")}:${get("minute")}`;
}

/**
 * Tirilla para impresora térmica. Solo existe en papel: en pantalla está
 * oculta y al imprimir es lo único visible (ver `.print-ticket` en globals.css).
 */
export function PrintTicket({
  model,
  business,
  branchName,
  widthMm = 80,
  timeZone = "America/Bogota",
}: {
  model: TicketModel;
  business: BusinessInfo | null;
  branchName: string;
  widthMm?: number;
  timeZone?: string;
}) {
  const nit = business?.nit ? `NIT ${business.nit}${business.dv ? `-${business.dv}` : ""}` : null;
  return (
    <PrintArea width={widthMm === 58 ? 58 : 80}>
      <p className="t-center t-bold t-big">{business?.business_name ?? business?.name ?? branchName}</p>
      {nit && <p className="t-center">{nit}</p>}
      <p className="t-center">{branchName}</p>
      <hr />
      <p className="t-center t-bold">{model.title}</p>
      <p className="t-row">
        <span>Fecha</span>
        <span>{formatTicketDate(model.date, timeZone)}</span>
      </p>
      {model.cashier && (
        <p className="t-row">
          <span>Cajero</span>
          <span>{model.cashier}</span>
        </p>
      )}
      {model.register && (
        <p className="t-row">
          <span>Caja</span>
          <span>{model.register}</span>
        </p>
      )}
      <p className="t-row">
        <span>Cliente</span>
        <span>{model.customer}</span>
      </p>
      {model.waiter && (
        <p className="t-row">
          <span>Mesero</span>
          <span>{model.waiter}</span>
        </p>
      )}
      <hr />
      {model.lines.map((line, i) => (
        <div key={i} className="t-item">
          <p>{line.name}</p>
          <p className="t-row">
            <span>
              {line.qty} x {formatCOP(line.unitPrice)}
            </span>
            <span>{formatCOP(line.gross)}</span>
          </p>
          {line.discounts.map((d) => (
            <p key={d.label} className="t-row">
              <span>  {d.label}</span>
              <span>−{formatCOP(d.amount)}</span>
            </p>
          ))}
        </div>
      ))}
      <hr />
      <p className="t-row">
        <span>Subtotal</span>
        <span>{formatCOP(model.subtotal)}</span>
      </p>
      {model.discountTotal > 0 && (
        <p className="t-row">
          <span>Descuentos</span>
          <span>−{formatCOP(model.discountTotal)}</span>
        </p>
      )}
      {model.taxes.map((t) => (
        <p key={t.label} className="t-row">
          <span>{t.label}</span>
          <span>{formatCOP(t.amount)}</span>
        </p>
      ))}
      {model.tip > 0 && (
        <p className="t-row">
          <span>Propina</span>
          <span>{formatCOP(model.tip)}</span>
        </p>
      )}
      <p className="t-row t-bold t-big">
        <span>TOTAL</span>
        <span>{formatCOP(model.total)}</span>
      </p>
      {model.payments && (
        <>
          <hr />
          {model.payments.map((p, i) => (
            <div key={i}>
              <p className="t-row">
                <span>{p.label}</span>
                <span>{formatCOP(p.amount)}</span>
              </p>
              {p.tendered != null && (
                <>
                  <p className="t-row">
                    <span>  Recibido</span>
                    <span>{formatCOP(p.tendered)}</span>
                  </p>
                  <p className="t-row">
                    <span>  Vuelto</span>
                    <span>{formatCOP(model.change)}</span>
                  </p>
                </>
              )}
            </div>
          ))}
        </>
      )}
      {model.payments && model.fiscalAccepted && (
        <>
          <hr />
          {/* F4.7 agrega aquí el CUFE/CUDE y el QR cuando el documento es ACEPTADO. */}
          <p className="t-center t-small">Documento electrónico validado por la DIAN</p>
        </>
      )}
      <hr />
      <p className="t-center">Gracias por tu compra</p>
    </PrintArea>
  );
}

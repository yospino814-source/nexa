"use client";

import { PrintArea } from "@/components/nexa/print-area";

import type { CheckoutResponse, PaymentMethod } from "@/lib/api";
import { linePrice, type CartLine } from "@/lib/stores/cart";
import { formatCOP } from "@/lib/format";

const METHOD_LABELS: Record<PaymentMethod, string> = {
  EFECTIVO: "Efectivo",
  DATAFONO: "Datáfono",
  QR: "QR / Transferencia",
};

/**
 * Tirilla para impresora termica. Solo existe en papel: en pantalla esta
 * oculta y al imprimir es lo unico visible (ver `.print-ticket` en
 * globals.css).
 */
export function PrintTicket({
  response,
  lines,
  received,
  method,
  branchName,
  cashierName,
}: {
  response: CheckoutResponse;
  lines: CartLine[];
  received: number;
  method: PaymentMethod;
  branchName: string;
  cashierName: string | null;
}) {
  const subtotal = Number(response.order.subtotal);
  const tax = Number(response.order.inc_amount);
  const total = Number(response.order.total_amount);
  const change = Number(response.change_due);
  const now = new Date().toLocaleString("es-CO", { dateStyle: "short", timeStyle: "short", hour12: false });

  return (
    <PrintArea>
      <p className="t-center t-bold t-big">{branchName}</p>
      <p className="t-center">Nexa POS</p>
      <hr />
      <p className="t-row">
        <span>Ticket</span>
        <span className="t-bold">{response.receipt_number}</span>
      </p>
      <p className="t-row">
        <span>Fecha</span>
        <span>{now}</span>
      </p>
      {cashierName && (
        <p className="t-row">
          <span>Cajero</span>
          <span>{cashierName}</span>
        </p>
      )}
      <hr />
      {lines.map((line) => (
        <div key={line.key} className="t-item">
          <p>{line.name}</p>
          <p className="t-row">
            <span>
              {line.qty} x {formatCOP(linePrice(line))}
            </span>
            <span>{formatCOP(linePrice(line) * line.qty)}</span>
          </p>
        </div>
      ))}
      <hr />
      <p className="t-row">
        <span>Subtotal</span>
        <span>{formatCOP(subtotal)}</span>
      </p>
      <p className="t-row">
        <span>IVA</span>
        <span>{formatCOP(tax)}</span>
      </p>
      <p className="t-row t-bold t-big">
        <span>TOTAL</span>
        <span>{formatCOP(total)}</span>
      </p>
      <hr />
      <p className="t-row">
        <span>{METHOD_LABELS[method]}</span>
        <span>{formatCOP(method === "EFECTIVO" ? received : total)}</span>
      </p>
      {method === "EFECTIVO" && (
        <p className="t-row">
          <span>Cambio</span>
          <span>{formatCOP(change)}</span>
        </p>
      )}
      <hr />
      <p className="t-center t-small">
        {response.fiscal_status === "ACEPTADO"
          ? "Documento equivalente electrónico validado por la DIAN"
          : "Documento equivalente electrónico en trámite ante la DIAN"}
      </p>
      <p className="t-center">¡Gracias por su compra!</p>
    </PrintArea>
  );
}

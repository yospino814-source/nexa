"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { PrintArea } from "@/components/nexa/print-area";
import type { BusinessInfo, ReceivablePaymentResponse } from "@/lib/api";
import { api } from "@/lib/api";
import { formatCOP } from "@/lib/format";

const METHOD_LABELS: Record<string, string> = {
  EFECTIVO: "Efectivo",
  DATAFONO: "Datáfono",
  QR: "QR",
  TRANSFERENCIA: "Transferencia",
  SALDO: "Saldo a favor",
};

function formatDateDMY(d: string): string {
  const parts = d.slice(0, 10).split("-");
  if (parts.length === 3) {
    return `${parts[2].padStart(2, "0")}/${parts[1].padStart(2, "0")}/${parts[0]}`;
  }
  return d;
}

function formatTicketDate(date: Date, timeZone: string): string {
  try {
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
  } catch {
    const d = date.toISOString().slice(0, 10);
    const t = date.toTimeString().slice(0, 5);
    return `${formatDateDMY(d)} ${t}`;
  }
}

export type PaymentTicketProps = {
  open: boolean;
  onClose: () => void;
  payment: ReceivablePaymentResponse;
  customer: {
    name: string;
    doc_type?: string | null;
    doc_number?: string | null;
    dv?: string | null;
  };
  receiptNumber?: string | null;
  cashierName?: string | null;
  branchName?: string;
  widthMm?: 58 | 80;
  timeZone?: string;
};

export function PaymentTicketDialog({
  open,
  onClose,
  payment,
  customer,
  receiptNumber,
  cashierName = "Cajero",
  branchName = "Sede Principal",
  widthMm = 80,
  timeZone = "America/Bogota",
}: PaymentTicketProps) {
  const [business, setBusiness] = useState<BusinessInfo | null>(null);

  useEffect(() => {
    let active = true;
    api.business.get().then((info) => {
      if (active) setBusiness(info);
    }).catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const nit = business?.nit ? `NIT ${business.nit}${business.dv ? `-${business.dv}` : ""}` : null;
  const docStr = customer.doc_type && customer.doc_number
    ? `${customer.doc_type} ${customer.doc_number}${customer.dv ? `-${customer.dv}` : ""}`
    : null;

  const paymentDate = new Date(payment.created_at || 0);
  const formattedDateTime = formatTicketDate(paymentDate, timeZone);
  const methodLabel = METHOD_LABELS[payment.method] ?? payment.method;

  const isPaidInFull = Number(payment.balance_after) === 0;

  return (
    <>
      <Dialog open={open} onOpenChange={(val) => !val && onClose()}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Comprobante de abono</DialogTitle>
          </DialogHeader>

          {/* Vista previa en pantalla */}
          <div className="rounded-lg border bg-card p-5 text-sm space-y-3 font-mono shadow-inner">
            <div className="text-center space-y-0.5 border-b pb-2">
              <p className="font-bold text-base">{business?.business_name ?? business?.name ?? branchName}</p>
              {nit && <p className="text-xs text-muted-foreground">{nit}</p>}
              <p className="text-xs text-muted-foreground">{branchName}</p>
            </div>

            <div className="text-center font-bold text-base py-1">ABONO</div>

            <div className="space-y-1 text-xs border-b pb-2">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Fecha y hora</span>
                <span>{formattedDateTime}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Cliente</span>
                <span className="text-right font-medium">{customer.name}</span>
              </div>
              {docStr && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Documento</span>
                  <span>{docStr}</span>
                </div>
              )}
              <div className="flex justify-between">
                <span className="text-muted-foreground">Venta</span>
                <span>{receiptNumber || "Sin número"}</span>
              </div>
            </div>

            <div className="space-y-1 text-xs border-b pb-2">
              <div className="flex justify-between font-semibold">
                <span>Monto abonado</span>
                <span>{formatCOP(Number(payment.amount))}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Método</span>
                <span>{methodLabel}</span>
              </div>
              {payment.reference && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Referencia</span>
                  <span>{payment.reference}</span>
                </div>
              )}
            </div>

            <div className="space-y-1 text-xs border-b pb-2">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Saldo anterior</span>
                <span>{formatCOP(Number(payment.balance_before))}</span>
              </div>
              <div className="flex justify-between font-semibold">
                <span>Saldo nuevo</span>
                <span>{formatCOP(Number(payment.balance_after))}</span>
              </div>
            </div>

            <div className="text-xs border-b pb-2 text-center">
              {isPaidInFull ? (
                <p className="font-bold text-emerald-600">Cuenta pagada</p>
              ) : payment.next_due ? (
                <p>
                  Próxima cuota: Cuota {payment.next_due.n} · {formatDateDMY(payment.next_due.due_date)} · {formatCOP(Number(payment.next_due.amount_pending))}
                </p>
              ) : null}
            </div>

            <div className="text-xs text-muted-foreground text-center">
              Atendido por {cashierName ?? "Usuario"}
            </div>
          </div>

          <DialogFooter className="flex sm:justify-between gap-2">
            <Button variant="outline" onClick={onClose}>
              Cerrar
            </Button>
            <Button onClick={() => window.print()}>
              Imprimir
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Impresión térmica via PrintArea */}
      {open && (
        <PrintArea width={widthMm === 58 ? 58 : 80}>
          {/* 1. Encabezado del negocio */}
          <p className="t-center t-bold t-big">{business?.business_name ?? business?.name ?? branchName}</p>
          {nit && <p className="t-center">{nit}</p>}
          <p className="t-center">{branchName}</p>
          <hr />

          {/* 2. Título ABONO */}
          <p className="t-center t-bold t-big">ABONO</p>

          {/* 3. Fecha y hora */}
          <p className="t-row">
            <span>Fecha</span>
            <span>{formattedDateTime}</span>
          </p>

          {/* 4. Cliente (nombre y documento) */}
          <p className="t-row">
            <span>Cliente</span>
            <span>{customer.name}</span>
          </p>
          {docStr && (
            <p className="t-row">
              <span>Doc</span>
              <span>{docStr}</span>
            </p>
          )}

          {/* 5. Venta <número> */}
          <p className="t-row">
            <span>Venta</span>
            <span>{receiptNumber || "Sin número"}</span>
          </p>
          <hr />

          {/* 6. Monto abonado, Método (y referencia) */}
          <p className="t-row t-bold">
            <span>Monto abonado</span>
            <span>{formatCOP(Number(payment.amount))}</span>
          </p>
          <p className="t-row">
            <span>Método</span>
            <span>{methodLabel}</span>
          </p>
          {payment.reference && (
            <p className="t-row">
              <span>Ref</span>
              <span>{payment.reference}</span>
            </p>
          )}
          <hr />

          {/* 7. Saldo anterior y Saldo nuevo */}
          <p className="t-row">
            <span>Saldo anterior</span>
            <span>{formatCOP(Number(payment.balance_before))}</span>
          </p>
          <p className="t-row t-bold">
            <span>Saldo nuevo</span>
            <span>{formatCOP(Number(payment.balance_after))}</span>
          </p>
          <hr />

          {/* 8. Próxima cuota o Cuenta pagada */}
          {isPaidInFull ? (
            <p className="t-center t-bold">Cuenta pagada</p>
          ) : payment.next_due ? (
            <p className="t-center">
              Próxima cuota: {payment.next_due.n} · {formatDateDMY(payment.next_due.due_date)} · {formatCOP(Number(payment.next_due.amount_pending))}
            </p>
          ) : null}
          <hr />

          {/* 9. Atendido por <nombre del usuario> */}
          <p className="t-center">Atendido por {cashierName ?? "Usuario"}</p>
        </PrintArea>
      )}
    </>
  );
}

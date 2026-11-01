"use client";

import { useState } from "react";
import { Banknote, Check, CheckCircle2, Loader2, MessageSquare, PlusCircle, Printer, ShoppingBag } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type CheckoutResponse, type PaymentMethod } from "@/lib/api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP } from "@/lib/format";
import { cartTotals, useCart, type CartLine } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

import { PAYMENT_METHODS } from "./payment-methods";
import { PrintTicket } from "./print-ticket";

/** Billetes sugeridos por encima del total (exacto + 3 redondeos). */
function quickAmounts(total: number): number[] {
  const options = [10_000, 20_000, 50_000, 100_000]
    .map((step) => Math.ceil(total / step) * step)
    .filter((amount) => amount > total);
  return [total, ...new Set(options)].slice(0, 4);
}

type Receipt = { response: CheckoutResponse; lines: CartLine[]; received: number; method: PaymentMethod };

/** Pantalla 14: modal de cobro rapido + comprobante. */
export function CheckoutDialog({
  open,
  onOpenChange,
  lines,
  initialMethod,
  branchId,
  onCompleted,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  lines: CartLine[];
  initialMethod: PaymentMethod;
  branchId: string;
  onCompleted: () => void;
}) {
  const [method, setMethod] = useState(initialMethod);
  const [received, setReceived] = useState<number | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  // Clave de idempotencia de esta venta: se reusa en cada reintento para que
  // un doble clic o un corte de red no cobren dos veces.
  const [saleId, setSaleId] = useState(() => crypto.randomUUID());
  const cashierName = useSession((s) => s.name);
  const customer = useCart((s) => s.customer);
  const customerAlias = useCart((s) => s.customerAlias);

  // Al abrir el modal se reinicia con el metodo elegido en el carrito.
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      setMethod(initialMethod);
      setReceived(null);
      setError(null);
      setReceipt(null);
      setSaleId(crypto.randomUUID());
    }
  }

  const { total, units } = cartTotals(lines);
  const isCash = method === "EFECTIVO";
  const paid = isCash ? (received ?? total) : total;
  const change = Math.max(paid - total, 0);
  const canConfirm = !submitting && paid >= total && total > 0;

  async function confirm() {
    if (!canConfirm) return;
    setSubmitting(true);
    setError(null);
    try {
      const response = await api.sales.checkout({
        id: saleId,
        branch_id: branchId,
        items: lines.map((line) => ({ product_id: line.productId, qty: String(line.qty) })),
        payments: [{ method, amount: String(paid) }],
        quoted_total: String(total),
        customer_id: customer?.id ?? null,
        customer_alias: customerAlias ?? null,
      });
      setReceipt({ response, lines, received: paid, method });
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setSubmitting(false);
    }
  }

  function finish() {
    onCompleted();
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={(next) => (receipt && !next ? finish() : onOpenChange(next))}>
      <DialogContent
        className="max-h-[95vh] gap-0 overflow-y-auto rounded-3xl p-6 sm:max-w-lg sm:p-8"
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          if (receipt) finish();
          else confirm();
        }}
      >
        {receipt ? (
          <ReceiptView receipt={receipt} onNewSale={finish} />
        ) : (
          <div className="space-y-6">
            <div className="text-center">
              <p className="flex items-center justify-center gap-2 text-xs font-semibold tracking-widest text-slate-500 uppercase">
                <span className="size-2 rounded-full bg-emerald-500" /> Total a cobrar
              </p>
              <DialogTitle className="mt-2 text-5xl font-extrabold tracking-tight">
                {formatCOP(total)} <span className="text-2xl font-bold text-slate-400">COP</span>
              </DialogTitle>
              <p className="mx-auto mt-3 flex w-fit items-center gap-2 rounded-full bg-slate-100 px-3 py-1 text-sm text-slate-600">
                <ShoppingBag className="size-4" /> {units} artículos
              </p>
            </div>

            <div>
              <p className="mb-2 text-xs font-semibold tracking-wider text-slate-500 uppercase">Método de pago</p>
              <div className="grid grid-cols-3 gap-2">
                {PAYMENT_METHODS.map(({ id, label, icon: Icon }) => (
                  <button
                    key={id}
                    onClick={() => setMethod(id)}
                    className={cn(
                      "flex flex-col items-center gap-1.5 rounded-2xl border p-3 text-sm font-medium",
                      method === id ? "border-2 border-brand bg-teal-50 text-brand" : "border-slate-200 hover:bg-slate-50",
                    )}
                  >
                    <Icon className="size-6" />
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {isCash && (
              <div className="space-y-3 rounded-2xl border border-slate-200/60 bg-slate-50 p-4">
                <p className="text-xs font-semibold tracking-wider text-slate-500 uppercase">Monto recibido</p>
                <label className="flex items-center gap-2 rounded-2xl border-2 border-brand bg-white px-4 py-3">
                  <span className="text-2xl font-bold text-slate-400">$</span>
                  <input
                    autoFocus
                    inputMode="numeric"
                    value={received == null ? "" : received.toLocaleString("es-CO")}
                    placeholder={total.toLocaleString("es-CO")}
                    onChange={(e) => {
                      const digits = e.target.value.replace(/\D/g, "");
                      setReceived(digits ? Number(digits) : null);
                    }}
                    className="w-full bg-transparent text-3xl font-bold tabular-nums outline-none"
                  />
                </label>
                <div className="grid grid-cols-4 gap-2">
                  {quickAmounts(total).map((amount, i) => (
                    <button
                      key={amount}
                      onClick={() => setReceived(amount)}
                      className={cn(
                        "h-11 rounded-full border text-sm font-semibold",
                        paid === amount ? "border-brand bg-brand text-white" : "border-slate-200 bg-white hover:bg-slate-100",
                      )}
                    >
                      {i === 0 ? "Exacto" : formatCOP(amount)}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {isCash && (
              <div
                className={cn(
                  "rounded-2xl border p-4 text-center",
                  paid >= total ? "border-emerald-200 bg-emerald-50" : "border-rose-200 bg-rose-50",
                )}
              >
                {paid >= total ? (
                  <>
                    <p className="text-xs font-semibold tracking-wider text-emerald-700 uppercase">Cambio a entregar</p>
                    <p className="text-4xl font-extrabold text-emerald-600 tabular-nums">{formatCOP(change)}</p>
                  </>
                ) : (
                  <p className="text-sm font-medium text-rose-600">Faltan {formatCOP(total - paid)}</p>
                )}
              </div>
            )}

            {error && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

            <button
              onClick={confirm}
              disabled={!canConfirm}
              className="flex h-16 w-full items-center justify-center gap-3 rounded-2xl bg-brand text-xl font-bold text-white shadow-lg shadow-teal-900/20 hover:bg-brand-dark disabled:bg-slate-300 disabled:shadow-none"
            >
              {submitting ? <Loader2 className="size-6 animate-spin" /> : <Check className="size-6" />}
              Confirmar pago (Enter)
            </button>
            <p className="flex justify-between text-xs text-slate-400">
              <span>
                Presioná <kbd className="rounded border bg-slate-50 px-1">Esc</kbd> para cancelar
              </span>
              <span>Cajero en turno: {cashierName}</span>
            </p>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function ReceiptView({ receipt, onNewSale }: { receipt: Receipt; onNewSale: () => void }) {
  const { response, lines, received, method } = receipt;
  const branchName = useCurrentBranch().name;
  const cashierName = useSession((s) => s.name);
  // Los totales del comprobante son los que cobro el servidor, no los del carrito.
  const subtotal = Number(response.order.subtotal);
  const tax = Number(response.order.inc_amount);
  const total = Number(response.order.total_amount);
  const change = Number(response.change_due);

  return (
    <div className="space-y-5">
      <PrintTicket
        response={response}
        lines={lines}
        received={received}
        method={method}
        branchName={branchName}
        cashierName={cashierName}
      />
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-2 rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1 text-sm font-semibold text-emerald-700">
          <span className="size-2 rounded-full bg-emerald-500" /> Aprobada #{response.receipt_number}
        </span>
        <span className="font-mono text-xs text-slate-400">
          {new Date().toLocaleTimeString("es-CO", { hour12: false })}
        </span>
      </div>

      <div className="text-center">
        <CheckCircle2 className="mx-auto size-16 rounded-full bg-emerald-50 p-3 text-emerald-600" />
        <DialogTitle className="mt-3 text-2xl font-extrabold tracking-tight">¡Venta registrada con éxito!</DialogTitle>
        <p className="text-sm text-slate-500">El inventario se actualizó en tiempo real</p>
      </div>

      {method === "EFECTIVO" && change > 0 && (
        <div className="flex items-center gap-3 rounded-2xl bg-gradient-to-r from-emerald-500 to-teal-600 p-4 text-white">
          <Banknote className="size-8 rounded-full bg-white/20 p-1.5" />
          <div>
            <p className="text-xs font-semibold tracking-wider uppercase opacity-80">Devolver cambio</p>
            <p className="text-2xl font-extrabold">{formatCOP(change)} COP</p>
          </div>
        </div>
      )}

      <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-4 font-mono text-xs">
        <p className="text-center font-sans text-sm font-bold">{branchName}</p>
        <p className="text-center text-slate-500">Ticket POS: {response.receipt_number}</p>
        <p className="mb-3 text-center text-[10px] text-slate-400">
          {response.fiscal_status === "ACEPTADO"
            ? "Documento equivalente electrónico validado por la DIAN"
            : "Documento equivalente electrónico en trámite ante la DIAN"}
        </p>
        {lines.map((line) => (
          <div key={line.productId} className="flex justify-between py-0.5">
            <span>
              {line.qty}x {line.name}
            </span>
            <span className="font-bold">{formatCOP(line.unitPrice * line.qty)}</span>
          </div>
        ))}
        <div className="mt-3 space-y-0.5 border-t border-dashed border-slate-300 pt-3">
          <div className="flex justify-between text-slate-500">
            <span>Subtotal</span>
            <span>{formatCOP(subtotal)}</span>
          </div>
          <div className="flex justify-between text-slate-500">
            <span>IVA</span>
            <span>{formatCOP(tax)}</span>
          </div>
          <div className="flex justify-between font-sans text-sm font-bold">
            <span>TOTAL PAGADO</span>
            <span className="text-brand">{formatCOP(total)}</span>
          </div>
          {method === "EFECTIVO" && (
            <>
              <div className="flex justify-between text-slate-500">
                <span>Efectivo recibido</span>
                <span>{formatCOP(received)}</span>
              </div>
              <div className="flex justify-between text-emerald-600">
                <span>Cambio</span>
                <span>{formatCOP(change)}</span>
              </div>
            </>
          )}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <button
          onClick={() => window.print()}
          className="flex h-12 items-center justify-center gap-2 rounded-2xl border border-slate-200 font-medium hover:bg-slate-50"
        >
          <Printer className="size-4" /> Imprimir
        </button>
        <button className="flex h-12 items-center justify-center gap-2 rounded-2xl bg-emerald-600 font-medium text-white hover:bg-emerald-700">
          <MessageSquare className="size-4" /> WhatsApp
        </button>
      </div>
      <button
        onClick={onNewSale}
        className="flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-brand text-lg font-bold text-white hover:bg-brand-dark"
      >
        <PlusCircle className="size-5" /> Nueva venta (Enter)
      </button>
    </div>
  );
}

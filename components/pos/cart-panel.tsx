"use client";

import { useEffect, useState } from "react";
import { ArrowRight, Minus, Pencil, Percent, Plus, Printer, ShoppingCart, Tag, Trash2, User, X } from "lucide-react";

import { api, friendlyError, type PaymentMethod } from "@/lib/api";
import { useBusinessInfo } from "@/lib/api/use-business";
import { formatCOP, taxLabel } from "@/lib/format";
import { linePrice, toSaleItems, useCart, type CartLine } from "@/lib/stores/cart";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { CustomerDialog } from "./customer-dialog";
import { DiscountDialog } from "./discount-dialog";
import { HeldSales } from "./held-sales";
import { PAYMENT_METHODS } from "./payment-methods";
import { PriceOverrideDialog } from "./price-override-dialog";
import { PrintTicket, ticketFromQuote, type TicketModel } from "./print-ticket";

/** Atajos del POS (F1.14), en el orden de la especificación. */
const SHORTCUTS: [string, string][] = [
  ["F2", "Buscar"],
  ["F4", "Cliente"],
  ["F6", "Descuento"],
  ["F8", "En espera"],
  ["F9", "Cobrar"],
  ["Esc", "Cerrar"],
];

/** Componente compartido "Carrito": líneas + totales de la cotización + acción principal. */
export function CartPanel({
  method,
  onMethodChange,
  onCharge,
  customerDialogOpen: controlledDialogOpen,
  onCustomerDialogOpenChange: setControlledDialogOpen,
  discountDialogOpen,
  onDiscountDialogOpenChange,
  allowPriceOverride = false,
  maxDiscountPct = 0,
  ticketWidth = 80,
}: {
  method: PaymentMethod;
  onMethodChange: (method: PaymentMethod) => void;
  onCharge: () => void;
  customerDialogOpen?: boolean;
  onCustomerDialogOpenChange?: (open: boolean) => void;
  /** Diálogo de descuento de la línea seleccionada (F6). */
  discountDialogOpen: boolean;
  onDiscountDialogOpenChange: (open: boolean) => void;
  /** Ajuste `pos.allow_price_override` de la sucursal. */
  allowPriceOverride?: boolean;
  /** Ajuste `pos.max_discount_pct` de la sucursal. */
  maxDiscountPct?: number;
  /** Ajuste `print.ticket_width_mm` (F1.15). */
  ticketWidth?: number;
}) {
  const {
    lines,
    customer,
    customerAlias,
    setCustomer,
    setCustomerAlias,
    setQty,
    remove,
    clear,
    select,
    selectedKey,
    couponCode,
    couponError,
    setCoupon,
    setCouponError,
    quote,
    quoteLoading,
    quoteError,
  } = useCart();
  const branch = useCurrentBranch();
  const can = useSession((s) => s.can);
  const canOverride = allowPriceOverride && can("pos:price_override");
  const [overrideLine, setOverrideLine] = useState<CartLine | null>(null);
  const business = useBusinessInfo();
  const cashierName = useSession((s) => s.name);
  const [preBill, setPreBill] = useState<TicketModel | null>(null);
  const [preBillError, setPreBillError] = useState<string | null>(null);
  const [internalDialogOpen, setInternalDialogOpen] = useState(false);
  const isDialogOpen = controlledDialogOpen !== undefined ? controlledDialogOpen : internalDialogOpen;
  const setDialogOpen = setControlledDialogOpen !== undefined ? setControlledDialogOpen : setInternalDialogOpen;
  const [couponInput, setCouponInput] = useState("");

  const empty = lines.length === 0;
  const units = lines.reduce((s, l) => s + l.qty, 0);
  const total = Number(quote?.total ?? 0);
  const promoNames = new Map((quote?.promotions ?? []).map((p) => [p.id, p.name]));
  const selectedLine = lines.find((l) => l.key === selectedKey) ?? null;

  // F1.15: la precuenta se imprime apenas se monta su tirilla y se quita al terminar.
  useEffect(() => {
    if (!preBill) return;
    const clearPreBill = () => setPreBill(null);
    window.addEventListener("afterprint", clearPreBill);
    window.print();
    return () => window.removeEventListener("afterprint", clearPreBill);
  }, [preBill]);

  /** Precuenta con la última cotización; si todavía no hay, se pide una. */
  async function printPreBill() {
    setPreBillError(null);
    try {
      const q =
        quote ??
        (await api.sales.quote({
          branch_id: branch.id,
          items: toSaleItems(lines),
          customer_id: customer?.id ?? null,
          customer_alias: customerAlias ?? null,
          coupon_code: couponCode ?? null,
        }));
      setPreBill(
        ticketFromQuote(q, {
          cashier: cashierName,
          register: null,
          customer: customer?.name ?? customerAlias ?? "Consumidor final",
          waiter: null,
        }),
      );
    } catch (err) {
      setPreBillError(friendlyError(err));
    }
  }

  function handleApplyCoupon(e: React.FormEvent) {
    e.preventDefault();
    const raw = couponInput.trim().toUpperCase();
    if (!raw) return;
    // La cotización en vivo valida el cupón; si no aplica, muestra el motivo y lo quita.
    setCouponError(null);
    setCoupon(raw);
    setCouponInput("");
  }

  function handleRemoveCoupon() {
    setCoupon(null);
    setCouponError(null);
    setCouponInput("");
  }

  return (
    <div className="flex h-full flex-col bg-white">
      <div className="space-y-4 border-b border-slate-100 p-5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <h2 className="text-xl font-bold tracking-tight">Venta actual</h2>
            {!empty && (
              <span className="rounded-full border border-teal-100 bg-teal-50 px-2.5 py-0.5 text-xs font-semibold text-brand">
                {lines.length} productos ({units} ítems)
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <HeldSales branchId={branch.id} />
            <button
              onClick={clear}
              disabled={empty}
              aria-label="Vaciar venta"
              className="flex size-10 items-center justify-center rounded-full border border-slate-200 text-slate-500 hover:bg-slate-50 disabled:opacity-40"
            >
              <Trash2 className="size-4" />
            </button>
          </div>
        </div>
        <div className="flex items-center gap-3 rounded-2xl border border-slate-200/60 bg-slate-50/60 p-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-full bg-teal-50 text-brand">
            {customerAlias ? <Tag className="size-5" /> : <User className="size-5" />}
          </span>
          <div className="min-w-0 flex-1 leading-tight">
            {customer ? (
              <>
                <p className="truncate text-sm font-semibold">{customer.name}</p>
                <p className="text-xs text-slate-500">
                  {customer.doc_type && customer.doc_number
                    ? `${customer.doc_type} ${customer.doc_number}${customer.dv ? `-${customer.dv}` : ""}`
                    : "Sin documento"}
                </p>
              </>
            ) : customerAlias ? (
              <>
                <p className="truncate text-sm font-semibold">Alias: {customerAlias}</p>
                <p className="text-xs text-slate-500">Venta identificada</p>
              </>
            ) : (
              <>
                <p className="text-sm font-semibold">Consumidor Final</p>
                <p className="text-xs text-slate-500">Sin cliente asignado</p>
              </>
            )}
          </div>
          {customer || customerAlias ? (
            <button
              onClick={() => {
                setCustomer(null);
                setCustomerAlias(null);
              }}
              title="Quitar cliente o alias"
              aria-label="Quitar cliente o alias"
              className="flex size-8 items-center justify-center rounded-full border border-slate-200 bg-white text-slate-400 hover:bg-slate-50 hover:text-rose-600"
            >
              <X className="size-4" />
            </button>
          ) : null}
          <button
            onClick={() => setDialogOpen(true)}
            className="flex items-center gap-1.5 rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-slate-50"
          >
            <Pencil className="size-3" /> {customer || customerAlias ? "Cambiar" : "Cliente (F4)"}
          </button>
          {can("pos:discount") && (
            <button
              onClick={() => selectedLine && onDiscountDialogOpenChange(true)}
              disabled={!selectedLine || selectedLine.productId === null}
              aria-label="Descuento a la línea seleccionada (F6)"
              title="Descuento a la línea seleccionada (F6)"
              className="flex size-8 items-center justify-center rounded-full border border-slate-200 bg-white hover:bg-slate-50 disabled:opacity-40"
            >
              <Percent className="size-3.5" />
            </button>
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5">
        {empty ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 py-12 text-slate-400">
            <ShoppingCart className="size-12 stroke-1" />
            <p className="text-sm">Escaneá o tocá un producto</p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-100">
            {lines.map((line, index) => {
              const ql = quote?.lines[index];
              const promoAmount = Number(ql?.discount.promotion ?? 0) + Number(ql?.discount.order ?? 0);
              const promoName = ql?.promotion_id ? promoNames.get(ql.promotion_id) : null;
              const manual = Number(ql?.discount.manual ?? 0);
              return (
                <li
                  key={line.key}
                  onClick={() => select(line.key)}
                  className={cn(
                    "-mx-2 flex cursor-pointer items-center gap-3 rounded-xl px-2 py-4",
                    selectedKey === line.key && "bg-teal-50/60",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <p className="truncate font-medium">{line.name}</p>
                      {line.tierName && (
                        <span className="shrink-0 rounded bg-teal-50 px-1.5 py-0.5 text-[10px] font-semibold text-brand">
                          {line.tierName}
                        </span>
                      )}
                    </div>
                    {line.optionNames && line.optionNames.length > 0 && (
                      <p className="text-[11px] text-emerald-700 font-medium truncate">
                        + {line.optionNames.join(", ")}
                      </p>
                    )}
                    <p className="text-xs text-slate-500">
                      {line.priceOverride != null && (
                        <span className="mr-1 line-through">{formatCOP(line.unitPrice)}</span>
                      )}
                      {formatCOP(ql ? Number(ql.unit_price) : linePrice(line))} c/u · {taxLabel(line.taxRate)}
                      {line.unitsPerPackage && line.unitsPerPackage > 1 ? ` · x${line.unitsPerPackage} un.` : ""}
                    </p>
                    {line.productId === null && (
                      <span className="mt-1 inline-block rounded bg-slate-100 px-1.5 py-0.5 text-[11px] font-semibold text-slate-600">
                        Ítem libre
                      </span>
                    )}
                    {canOverride && line.productId !== null && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setOverrideLine(line);
                        }}
                        className="mt-1 ml-1 inline-block text-[11px] font-semibold text-brand hover:underline"
                      >
                        Cambiar precio
                      </button>
                    )}
                    {manual > 0 && (
                      <span className="mt-1 ml-1 inline-block rounded bg-amber-50 px-1.5 py-0.5 text-[11px] font-semibold text-amber-700">
                        Descuento {line.discountPct} % −{formatCOP(manual)}
                      </span>
                    )}
                    {promoAmount > 0 && (
                      <span className="mt-1 ml-1 inline-block rounded bg-emerald-50 px-1.5 py-0.5 text-[11px] font-semibold text-emerald-700">
                        {promoName ?? "Promoción"} −{formatCOP(promoAmount)}
                      </span>
                    )}
                  </div>
                  <div
                    className="flex items-center gap-1 rounded-full border border-slate-200 bg-slate-50 p-1"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      onClick={() => setQty(line.key, line.qty - 1)}
                      aria-label="Quitar uno"
                      className="flex size-9 items-center justify-center rounded-full bg-white shadow-xs hover:bg-slate-100"
                    >
                      <Minus className="size-4" />
                    </button>
                    <span className="w-8 text-center font-semibold tabular-nums">{line.qty}</span>
                    <button
                      onClick={() => setQty(line.key, line.qty + 1)}
                      disabled={line.maxQty != null && line.qty >= line.maxQty}
                      aria-label="Agregar uno"
                      className="flex size-9 items-center justify-center rounded-full bg-white shadow-xs hover:bg-slate-100 disabled:opacity-40"
                    >
                      <Plus className="size-4" />
                    </button>
                  </div>
                  <span className="w-20 text-right font-bold tabular-nums">
                    {formatCOP(ql ? Number(ql.line_total) : linePrice(line) * line.qty)}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      remove(line.key);
                    }}
                    aria-label={`Eliminar ${line.name}`}
                    className="text-slate-300 hover:text-rose-500"
                  >
                    <X className="size-4" />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="space-y-4 border-t border-slate-100 p-5">
        <div className="space-y-1.5 border-b border-slate-100 pb-3">
          {couponCode ? (
            <div className="flex items-center justify-between rounded-xl border border-teal-200 bg-teal-50 px-3 py-2 text-xs font-semibold text-teal-800">
              <span className="flex items-center gap-1.5 font-mono">
                <Tag className="size-3.5 text-brand" />
                {couponCode}
                {quote?.coupon && (
                  <span className="font-bold text-brand">−{formatCOP(Number(quote.coupon.amount))}</span>
                )}
              </span>
              <button
                type="button"
                onClick={handleRemoveCoupon}
                aria-label="Quitar cupón"
                className="ml-2 text-teal-600 hover:text-rose-600"
              >
                <X className="size-4" />
              </button>
            </div>
          ) : (
            <form onSubmit={handleApplyCoupon} className="flex gap-2">
              <input
                type="text"
                placeholder="Cupón"
                value={couponInput}
                onChange={(e) => setCouponInput(e.target.value.toUpperCase())}
                disabled={empty}
                className="flex-1 rounded-xl border border-slate-200 px-3 py-1.5 font-mono text-xs uppercase focus:border-brand focus:ring-1 focus:ring-brand disabled:opacity-40"
              />
              <button
                type="submit"
                disabled={empty || !couponInput.trim()}
                className="rounded-xl bg-slate-900 px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-slate-800 disabled:opacity-40"
              >
                Aplicar
              </button>
            </form>
          )}
          {couponError && <p className="text-xs font-medium text-rose-600">{couponError}</p>}
        </div>

        {quoteError && <p className="rounded-xl bg-rose-50 p-2 text-xs font-medium text-rose-600">{quoteError}</p>}

        <div className={cn("space-y-3 transition-opacity", quoteLoading && "opacity-50")}>
          <dl className="space-y-1 text-sm text-slate-500">
            <div className="flex justify-between">
              <dt>Subtotal</dt>
              <dd className="tabular-nums">{formatCOP(Number(quote?.subtotal_gross ?? 0))}</dd>
            </div>
            {Number(quote?.discount_total ?? 0) > 0 && (
              <div className="flex justify-between font-medium text-emerald-600">
                <dt>Descuentos</dt>
                <dd className="tabular-nums">−{formatCOP(Number(quote?.discount_total))}</dd>
              </div>
            )}
            <div className="flex justify-between">
              <dt>Impuestos</dt>
              <dd className="tabular-nums">{formatCOP(Number(quote?.tax_total ?? 0))}</dd>
            </div>
          </dl>
          <div className="flex items-baseline justify-between border-t border-slate-100 pt-3">
            <span className="text-sm font-bold tracking-wide uppercase">Total a pagar</span>
            <span className="text-4xl font-extrabold tracking-tight tabular-nums">{formatCOP(total)}</span>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {PAYMENT_METHODS.map(({ id, label, icon: Icon, tone }) => (
            <button
              key={id}
              onClick={() => onMethodChange(id)}
              className={cn(
                "flex h-12 items-center justify-center gap-2 rounded-full border text-sm font-medium transition-colors",
                method === id ? "border-brand bg-teal-50 text-brand ring-1 ring-brand" : "border-slate-200 hover:bg-slate-50",
              )}
            >
              <Icon className={cn("size-4", method !== id && tone)} />
              {label}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={printPreBill}
          disabled={empty}
          className="flex h-11 w-full items-center justify-center gap-2 rounded-2xl border border-slate-200 text-sm font-medium hover:bg-slate-50 disabled:opacity-40"
        >
          <Printer className="size-4" /> Precuenta
        </button>
        {preBillError && <p className="text-xs font-medium text-rose-600">{preBillError}</p>}
        {preBill && (
          <PrintTicket model={preBill} business={business} branchName={branch.name} widthMm={ticketWidth} />
        )}
        <button
          onClick={onCharge}
          disabled={empty || !quote || quoteLoading}
          className="flex h-16 w-full items-center justify-between rounded-2xl bg-brand px-6 text-xl font-bold text-white shadow-lg shadow-teal-900/20 transition-colors hover:bg-brand-dark disabled:cursor-not-allowed disabled:bg-slate-300 disabled:shadow-none"
        >
          <span>Cobrar {formatCOP(total)}</span>
          <span className="flex items-center gap-2">
            <kbd className="rounded-md bg-white/15 px-2 py-0.5 text-xs font-medium">F9</kbd>
            <ArrowRight className="size-5" />
          </span>
        </button>
        <div className="hidden flex-wrap justify-between gap-y-1 text-xs text-slate-400 lg:flex">
          {SHORTCUTS.map(([key, label]) => (
            <span key={key} className="flex items-center gap-1.5">
              <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5">{key}</kbd>
              {label}
            </span>
          ))}
        </div>
      </div>

      <PriceOverrideDialog line={overrideLine} onClose={() => setOverrideLine(null)} />
      <DiscountDialog
        line={discountDialogOpen ? selectedLine : null}
        maxPct={maxDiscountPct}
        onClose={() => onDiscountDialogOpenChange(false)}
      />
      <CustomerDialog
        open={isDialogOpen}
        onOpenChange={setDialogOpen}
        currentCustomer={customer}
        currentAlias={customerAlias}
        onSelectCustomer={setCustomer}
        onSelectAlias={setCustomerAlias}
      />
    </div>
  );
}

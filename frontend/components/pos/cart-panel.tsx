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
import { ProductArt } from "./product-art";

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
  const canCreditSell = can("credit:sell");
  const hasCustomer = Boolean(customer?.id);
  const availableMethods = PAYMENT_METHODS.filter((m) => m.id !== "CREDITO" || (hasCustomer && canCreditSell));

  useEffect(() => {
    if (method === "CREDITO" && (!hasCustomer || !canCreditSell)) {
      onMethodChange("EFECTIVO");
    }
  }, [method, hasCustomer, canCreditSell, onMethodChange]);

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
      <div className="space-y-3 border-b border-slate-100 px-5 pt-[18px] pb-3.5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-baseline gap-2.5">
            <h2 className="text-lg font-semibold tracking-[-0.01em]">Venta actual</h2>
            {!empty && (
              <span className="text-[13px] text-slate-500 tabular-nums">
                {units} {units === 1 ? "producto" : "productos"}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <HeldSales branchId={branch.id} />
            <button
              onClick={clear}
              disabled={empty}
              aria-label="Vaciar venta"
              className="flex size-9 items-center justify-center rounded-[10px] text-slate-500 hover:bg-rose-50 hover:text-rose-600 disabled:opacity-40"
            >
              <Trash2 className="size-4" />
            </button>
          </div>
        </div>
        <div className="flex items-center gap-2.5 rounded-xl border border-slate-200 bg-slate-50 py-2 pr-1.5 pl-2.5">
          <span className="flex size-[34px] shrink-0 items-center justify-center rounded-full bg-indigo-50 text-indigo-600">
            {customerAlias ? <Tag className="size-4" /> : <User className="size-4" />}
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
              className="flex size-8 items-center justify-center rounded-lg text-slate-500 hover:bg-slate-100 hover:text-rose-600"
            >
              <X className="size-4" />
            </button>
          ) : null}
          <button
            onClick={() => setDialogOpen(true)}
            className="flex h-8 items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 text-[13px] font-semibold shadow-xs hover:bg-slate-50"
          >
            <Pencil className="size-3" /> {customer || customerAlias ? "Cambiar" : "Cliente (F4)"}
          </button>
          {can("pos:discount") && (
            <button
              onClick={() => selectedLine && onDiscountDialogOpenChange(true)}
              disabled={!selectedLine || selectedLine.productId === null}
              aria-label="Descuento a la línea seleccionada (F6)"
              title="Descuento a la línea seleccionada (F6)"
              className="flex size-8 items-center justify-center rounded-lg border border-slate-300 bg-white shadow-xs hover:bg-slate-50 disabled:opacity-40"
            >
              <Percent className="size-3.5" />
            </button>
          )}
        </div>
      </div>

      <div data-cart-target className="min-h-[180px] flex-1 overflow-y-auto px-2.5 py-2">
        {empty ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 py-10 text-slate-400">
            <ShoppingCart className="size-10" strokeWidth={1.5} />
            <p className="text-sm text-slate-500">Escaneá o tocá un producto</p>
          </div>
        ) : (
          <ul className="flex flex-col gap-1">
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
                    "nx-rise flex cursor-pointer items-center gap-3 rounded-xl py-2.5 pr-2.5 pl-3 [animation-duration:.24s]",
                    selectedKey === line.key ? "bg-[#F7F8FF] shadow-[inset_0_0_0_1.5px_#3D4FE0]" : "bg-white",
                  )}
                >
                  <span className="relative grid size-12 shrink-0 place-items-center overflow-hidden rounded-xl bg-[radial-gradient(circle_at_50%_30%,#fff,#EEF0F4)]">
                    {line.imageUrl ? (
                      // eslint-disable-next-line @next/next/no-img-element
                      <img src={line.imageUrl} alt="" className="absolute inset-0 size-full object-contain p-1 mix-blend-multiply" />
                    ) : (
                      <ProductArt name={line.name} category={null} height={44} bare />
                    )}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <p className="truncate text-sm font-medium">{line.name}</p>
                      {line.tierName && (
                        <span className="shrink-0 rounded-full bg-indigo-50 px-2 py-0.5 text-[11px] font-semibold text-indigo-600">
                          {line.tierName}
                        </span>
                      )}
                    </div>
                    {line.optionNames && line.optionNames.length > 0 && (
                      <p className="text-[11px] text-emerald-700 font-medium truncate">
                        + {line.optionNames.join(", ")}
                      </p>
                    )}
                    <p className="mt-0.5 text-xs text-slate-500 tabular-nums">
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
                    className="flex shrink-0 items-center overflow-hidden rounded-[10px] border border-slate-200 bg-white"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      onClick={() => setQty(line.key, line.qty - 1)}
                      aria-label="Quitar uno"
                      className="flex size-10 items-center justify-center text-slate-700 transition-transform hover:bg-indigo-50 active:scale-[0.86]"
                    >
                      <Minus className="size-4" strokeWidth={2.2} />
                    </button>
                    <span key={line.qty} className="nx-pop w-8 text-center text-[15px] font-semibold tabular-nums">
                      {line.qty}
                    </span>
                    <button
                      onClick={() => setQty(line.key, line.qty + 1)}
                      disabled={line.maxQty != null && line.qty >= line.maxQty}
                      aria-label="Agregar uno"
                      className="flex size-10 items-center justify-center text-slate-700 transition-transform hover:bg-indigo-50 active:scale-[0.86] disabled:opacity-40"
                    >
                      <Plus className="size-4" strokeWidth={2.2} />
                    </button>
                  </div>
                  <span className="w-[84px] shrink-0 text-right text-[15px] font-semibold tabular-nums">
                    {formatCOP(ql ? Number(ql.line_total) : linePrice(line) * line.qty)}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      remove(line.key);
                    }}
                    aria-label={`Eliminar ${line.name}`}
                    className="text-slate-300 hover:text-rose-600"
                  >
                    <X className="size-4" />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="space-y-3.5 border-t border-slate-200 bg-[#FBFBFD] px-5 pt-3.5 pb-5">
        <div className="space-y-1.5 border-b border-slate-100 pb-3">
          {couponCode ? (
            <div className="flex items-center justify-between rounded-xl bg-indigo-50 px-3 py-2 text-xs font-semibold text-indigo-700 ring-1 ring-indigo-200 ring-inset">
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
                className="ml-2 text-indigo-500 hover:text-rose-600"
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
                className="h-9 flex-1 rounded-[10px] border border-slate-300 bg-white px-3 font-mono text-xs uppercase outline-none focus:border-indigo-600 focus:ring-4 focus:ring-indigo-600/18 disabled:opacity-40"
              />
              <button
                type="submit"
                disabled={empty || !couponInput.trim()}
                className="h-9 rounded-[10px] border border-slate-300 bg-white px-3 text-[13px] font-semibold text-slate-900 shadow-xs hover:bg-slate-50 disabled:opacity-40"
              >
                Aplicar
              </button>
            </form>
          )}
          {couponError && <p className="text-xs font-medium text-rose-600">{couponError}</p>}
        </div>

        {quoteError && <p className="rounded-xl bg-rose-50 p-2 text-xs font-medium text-rose-600">{quoteError}</p>}

        <div className={cn("space-y-3 transition-opacity", quoteLoading && "opacity-50")}>
          <dl className="space-y-1.5 text-[13px] text-slate-500 tabular-nums">
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
          <div className="flex items-baseline justify-between">
            <span className="text-[15px] font-semibold">Total a pagar</span>
            <span key={total} className="nx-total text-[32px] font-bold tracking-[-0.03em] tabular-nums">
              {formatCOP(total)}
            </span>
          </div>
        </div>
        <div className={cn("grid gap-2", availableMethods.length === 4 ? "grid-cols-4" : "grid-cols-3")}>
          {availableMethods.map(({ id, label, icon: Icon, tone }) => (
            <button
              key={id}
              onClick={() => onMethodChange(id)}
              className={cn(
                "flex h-11 items-center justify-center gap-2 rounded-[10px] border text-[13px] font-semibold transition-colors",
                method === id
                  ? "border-indigo-600 bg-indigo-50 text-indigo-700 shadow-[0_0_0_3px_rgb(61_79_224/0.14)]"
                  : "border-slate-200 bg-white text-slate-700 hover:bg-slate-50",
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
          className="flex h-10 w-full items-center justify-center gap-2 rounded-[10px] text-[13px] font-semibold text-slate-700 hover:bg-slate-100 disabled:opacity-40"
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
          className="btn-cobrar flex h-16 w-full items-center justify-between rounded-[14px] px-6 text-[19px] font-bold tabular-nums disabled:cursor-not-allowed"
        >
          <span>Cobrar {formatCOP(total)}</span>
          <span className="flex items-center gap-2">
            <kbd className="rounded-md bg-white/18 px-2 py-0.5 text-xs font-semibold">F9</kbd>
            <ArrowRight className="size-5" />
          </span>
        </button>
        <div className="hidden flex-wrap justify-between gap-y-1 text-[11px] text-slate-500 lg:flex">
          {SHORTCUTS.map(([key, label]) => (
            <span key={key} className="flex items-center gap-1.5">
              <kbd className="rounded-md bg-slate-100 px-1.5 py-0.5 font-semibold text-slate-600">{key}</kbd>
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

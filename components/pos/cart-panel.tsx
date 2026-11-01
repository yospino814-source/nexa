"use client";

import { useState } from "react";
import { ArrowRight, Minus, Pencil, Percent, Plus, ShoppingCart, Tag, Trash2, User, X } from "lucide-react";

import { api, COUPON_ERROR_MESSAGES, friendlyError, type PaymentMethod } from "@/lib/api";
import { formatCOP, taxLabel } from "@/lib/format";
import { cartTotals, useCart } from "@/lib/stores/cart";
import { useCurrentBranch } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { CustomerDialog } from "./customer-dialog";
import { PAYMENT_METHODS } from "./payment-methods";

/** Componente compartido "Carrito": lineas + totales + accion principal. */
export function CartPanel({
  method,
  onMethodChange,
  onCharge,
  customerDialogOpen: controlledDialogOpen,
  onCustomerDialogOpenChange: setControlledDialogOpen,
}: {
  method: PaymentMethod;
  onMethodChange: (method: PaymentMethod) => void;
  onCharge: () => void;
  customerDialogOpen?: boolean;
  onCustomerDialogOpenChange?: (open: boolean) => void;
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
    couponCode,
    couponDiscount,
    couponError,
    setCoupon,
    setCouponDiscount,
    setCouponError,
  } = useCart();
  const branch = useCurrentBranch();
  const [internalDialogOpen, setInternalDialogOpen] = useState(false);
  const isDialogOpen = controlledDialogOpen !== undefined ? controlledDialogOpen : internalDialogOpen;
  const setDialogOpen = setControlledDialogOpen !== undefined ? setControlledDialogOpen : setInternalDialogOpen;

  const [couponInput, setCouponInput] = useState("");
  const [applyingCoupon, setApplyingCoupon] = useState(false);

  const totals = cartTotals(lines, couponDiscount);
  const empty = lines.length === 0;

  async function handleApplyCoupon(e: React.FormEvent) {
    e.preventDefault();
    const raw = couponInput.trim().toUpperCase();
    if (!raw) return;
    setApplyingCoupon(true);
    setCouponError(null);
    try {
      const res = await api.sales.quote({
        branch_id: branch.id,
        items: lines.map((l) => ({
          product_id: l.productId,
          qty: String(l.qty),
          tier_id: l.tierId ?? null,
        })),
        customer_id: customer?.id ?? null,
        customer_alias: customerAlias ?? null,
        coupon_code: raw,
      });

      if (res.coupon_error) {
        const msg = COUPON_ERROR_MESSAGES[res.coupon_error.code] ?? res.coupon_error.message;
        setCouponError(msg);
        setCoupon(null);
        setCouponDiscount(0);
      } else if (res.coupon) {
        setCoupon(res.coupon.code);
        setCouponDiscount(Number(res.coupon.amount));
        setCouponError(null);
        setCouponInput("");
      }
    } catch (err) {
      setCouponError(friendlyError(err));
    } finally {
      setApplyingCoupon(false);
    }
  }

  function handleRemoveCoupon() {
    setCoupon(null);
    setCouponDiscount(0);
    setCouponError(null);
    setCouponInput("");
  }

  return (
    <div className="flex h-full flex-col bg-white">
      <div className="space-y-4 border-b border-slate-100 p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h2 className="text-xl font-bold tracking-tight">Venta actual</h2>
            {!empty && (
              <span className="rounded-full border border-teal-100 bg-teal-50 px-2.5 py-0.5 text-xs font-semibold text-brand">
                {lines.length} productos ({totals.units} ítems)
              </span>
            )}
          </div>
          <button
            onClick={clear}
            disabled={empty}
            aria-label="Vaciar venta"
            className="flex size-10 items-center justify-center rounded-full border border-slate-200 text-slate-500 hover:bg-slate-50 disabled:opacity-40"
          >
            <Trash2 className="size-4" />
          </button>
        </div>
        <div className="flex items-center gap-3 rounded-2xl border border-slate-200/60 bg-slate-50/60 p-3">
          <span className="flex size-10 items-center justify-center rounded-full bg-teal-50 text-brand shrink-0">
            {customerAlias ? <Tag className="size-5" /> : <User className="size-5" />}
          </span>
          <div className="flex-1 min-w-0 leading-tight">
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
              className="flex size-8 items-center justify-center rounded-full border border-slate-200 bg-white text-slate-400 hover:text-rose-600 hover:bg-slate-50"
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
          <button aria-label="Descuento" className="flex size-8 items-center justify-center rounded-full border border-slate-200 bg-white hover:bg-slate-50">
            <Percent className="size-3.5" />
          </button>
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
            {lines.map((line) => (
              <li key={line.productId} className="flex items-center gap-3 py-4">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <p className="truncate font-medium">{line.name}</p>
                    {line.tierName && (
                      <span className="shrink-0 rounded bg-teal-50 px-1.5 py-0.5 text-[10px] font-semibold text-brand">
                        {line.tierName}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-slate-500">
                    {formatCOP(line.unitPrice)} c/u · {taxLabel(line.taxRate)}
                    {line.unitsPerPackage && line.unitsPerPackage > 1 ? ` · x${line.unitsPerPackage} un.` : ""}
                  </p>
                  {line.promotionName && (
                    <span className="inline-block mt-1 rounded bg-emerald-50 px-1.5 py-0.5 text-[11px] font-semibold text-emerald-700">
                      {line.promotionName} {line.promotionDiscount ? `−${formatCOP(line.promotionDiscount * line.qty)}` : ""}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1 rounded-full border border-slate-200 bg-slate-50 p-1">
                  <button
                    onClick={() => setQty(line.productId, line.qty - 1)}
                    aria-label="Quitar uno"
                    className="flex size-9 items-center justify-center rounded-full bg-white shadow-xs hover:bg-slate-100"
                  >
                    <Minus className="size-4" />
                  </button>
                  <span className="w-8 text-center font-semibold tabular-nums">{line.qty}</span>
                  <button
                    onClick={() => setQty(line.productId, line.qty + 1)}
                    disabled={line.maxQty != null && line.qty >= line.maxQty}
                    aria-label="Agregar uno"
                    className="flex size-9 items-center justify-center rounded-full bg-white shadow-xs hover:bg-slate-100 disabled:opacity-40"
                  >
                    <Plus className="size-4" />
                  </button>
                </div>
                <span className="w-20 text-right font-bold tabular-nums">{formatCOP(line.unitPrice * line.qty)}</span>
                <button
                  onClick={() => remove(line.productId)}
                  aria-label={`Eliminar ${line.name}`}
                  className="text-slate-300 hover:text-rose-500"
                >
                  <X className="size-4" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="space-y-4 border-t border-slate-100 p-5">
        {/* Cupón */}
        <div className="space-y-1.5 pb-3 border-b border-slate-100">
          {couponCode ? (
            <div className="flex items-center justify-between rounded-xl bg-teal-50 border border-teal-200 px-3 py-2 text-xs font-semibold text-teal-800">
              <span className="flex items-center gap-1.5 font-mono">
                <Tag className="size-3.5 text-brand" />
                {couponCode}
                <span className="text-brand font-bold">−{formatCOP(couponDiscount)}</span>
              </span>
              <button
                type="button"
                onClick={handleRemoveCoupon}
                aria-label="Quitar cupón"
                className="text-teal-600 hover:text-rose-600 ml-2"
              >
                <X className="size-4" />
              </button>
            </div>
          ) : (
            <form onSubmit={handleApplyCoupon} className="flex gap-2">
              <input
                type="text"
                placeholder="Código de cupón"
                value={couponInput}
                onChange={(e) => setCouponInput(e.target.value.toUpperCase())}
                disabled={empty || applyingCoupon}
                className="flex-1 rounded-xl border border-slate-200 px-3 py-1.5 text-xs font-mono uppercase focus:border-brand focus:ring-1 focus:ring-brand disabled:opacity-40"
              />
              <button
                type="submit"
                disabled={empty || applyingCoupon || !couponInput.trim()}
                className="rounded-xl bg-slate-900 px-3 py-1.5 text-xs font-semibold text-white hover:bg-slate-800 disabled:opacity-40 transition-colors"
              >
                {applyingCoupon ? "..." : "Aplicar"}
              </button>
            </form>
          )}
          {couponError && (
            <p className="text-xs text-rose-600 font-medium">{couponError}</p>
          )}
        </div>

        <dl className="space-y-1 text-sm text-slate-500">
          <div className="flex justify-between">
            <dt>Subtotal</dt>
            <dd className="tabular-nums">{formatCOP(totals.subtotal)}</dd>
          </div>
          {totals.discountTotal > 0 && (
            <div className="flex justify-between text-emerald-600 font-medium">
              <dt>Descuentos</dt>
              <dd className="tabular-nums">−{formatCOP(totals.discountTotal)}</dd>
            </div>
          )}
          <div className="flex justify-between">
            <dt>Impuestos (IVA)</dt>
            <dd className="tabular-nums">{formatCOP(totals.tax)}</dd>
          </div>
        </dl>
        <div className="flex items-baseline justify-between border-t border-slate-100 pt-3">
          <span className="text-sm font-bold uppercase tracking-wide">Total a pagar</span>
          <span className="text-4xl font-extrabold tracking-tight tabular-nums">{formatCOP(totals.total)}</span>
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
          onClick={onCharge}
          disabled={empty}
          className="flex h-16 w-full items-center justify-between rounded-2xl bg-brand px-6 text-xl font-bold text-white shadow-lg shadow-teal-900/20 transition-colors hover:bg-brand-dark disabled:cursor-not-allowed disabled:bg-slate-300 disabled:shadow-none"
        >
          <span>Cobrar {formatCOP(totals.total)}</span>
          <span className="flex items-center gap-2">
            <kbd className="rounded-md bg-white/15 px-2 py-0.5 text-xs font-medium">Espacio</kbd>
            <ArrowRight className="size-5" />
          </span>
        </button>
        <div className="hidden justify-between text-xs text-slate-400 lg:flex">
          {[["F2", "Buscar"], ["F4", "Cliente"], ["Espacio", "Cobrar"], ["Esc", "Vaciar"]].map(([key, label]) => (
            <span key={key} className="flex items-center gap-1.5">
              <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5">{key}</kbd>
              {label}
            </span>
          ))}
        </div>
      </div>

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

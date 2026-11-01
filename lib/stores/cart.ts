"use client";

import { create } from "zustand";

import type { CatalogItem, Customer, CustomTax, QuoteItemInput } from "@/lib/api";
import { toNumber } from "@/lib/format";

/**
 * Línea del carrito. `key` la identifica (puede haber dos líneas del mismo
 * producto, p. ej. una con precio cambiado, y los ítems libres no tienen producto).
 */
export type CartLine = {
  key: string;
  /** null en un ítem libre. */
  productId: string | null;
  name: string;
  unitPrice: number;
  basePrice?: number;
  /** Fraccion (0.19) o null si es exento. */
  taxRate: number | null;
  qty: number;
  /** Tope de cantidad segun el stock disponible (null = sin control). */
  maxQty: number | null;
  tierId?: string | null;
  tierName?: string | null;
  unitsPerPackage?: number | null;
  promotionName?: string | null;
  promotionDiscount?: number | null;
  /** F1.9: precio cambiado a mano (por unidad vendida) y su motivo. */
  priceOverride?: number | null;
  overrideReason?: string | null;
  /** F1.9: ítem libre. */
  customName?: string | null;
  customTax?: CustomTax | null;
  /** Descuento manual en porcentaje (F1.5). */
  discountPct?: number | null;
};

export type AddItemTier = {
  tierId: string;
  tierName: string;
  price?: number | null;
  unitsPerPackage?: number | null;
};

export type CustomItemInput = {
  name: string;
  price: number;
  tax: CustomTax;
  qty: number;
};

const CUSTOM_TAX_RATES: Record<CustomTax, number | null> = {
  IVA19: 0.19,
  IVA5: 0.05,
  INC8: 0.08,
  EXENTO: null,
};

type CartState = {
  lines: CartLine[];
  customer: Customer | null;
  customerAlias: string | null;
  couponCode: string | null;
  couponDiscount: number;
  couponError: string | null;
  add: (item: CatalogItem, tier?: AddItemTier) => void;
  addCustom: (item: CustomItemInput) => void;
  setQty: (key: string, qty: number) => void;
  setLineTier: (
    key: string,
    tier: { tierId: string | null; tierName?: string | null; price?: number | null; unitsPerPackage?: number | null },
  ) => void;
  setPriceOverride: (key: string, price: number | null, reason: string | null) => void;
  remove: (key: string) => void;
  setCustomer: (customer: Customer | null) => void;
  setCustomerAlias: (alias: string | null) => void;
  setCoupon: (code: string | null) => void;
  setCouponDiscount: (discount: number) => void;
  setCouponError: (error: string | null) => void;
  clear: () => void;
};

function newKey(): string {
  return crypto.randomUUID();
}

export const useCart = create<CartState>((set) => ({
  lines: [],
  customer: null,
  customerAlias: null,
  couponCode: null,
  couponDiscount: 0,
  couponError: null,
  setCoupon: (couponCode) => set({ couponCode }),
  setCouponDiscount: (couponDiscount) => set({ couponDiscount }),
  setCouponError: (couponError) => set({ couponError }),
  setCustomer: (customer) =>
    set((state) => {
      const oldTierId = state.customer?.price_tier_id;
      const newTierId = customer?.price_tier_id;
      const updatedLines = state.lines.map((line) => {
        if (line.productId === null) return line;
        // El cliente con tarifa la pone en las lineas que no tienen.
        if (newTierId && !line.tierId) {
          return { ...line, tierId: newTierId };
        }
        // Al quitar o cambiar el cliente, se quita su tarifa de las lineas.
        if (!newTierId && oldTierId && line.tierId === oldTierId) {
          return {
            ...line,
            tierId: null,
            tierName: null,
            unitPrice: line.basePrice ?? line.unitPrice,
            unitsPerPackage: 1,
          };
        }
        return line;
      });
      return { customer, customerAlias: null, lines: updatedLines };
    }),
  setCustomerAlias: (customerAlias) => set({ customerAlias, customer: null }),
  add: (item, tier) =>
    set((state) => {
      const maxQty = toNumber(item.available);
      // Se suma a una linea igual (mismo producto y tarifa, sin precio cambiado).
      const existing = state.lines.find(
        (line) =>
          line.productId === item.product_id &&
          (line.tierId ?? null) === (tier?.tierId ?? state.customer?.price_tier_id ?? null) &&
          line.priceOverride == null,
      );
      if (existing) {
        if (existing.maxQty != null && existing.qty >= existing.maxQty) return state;
        return {
          lines: state.lines.map((line) =>
            line === existing ? { ...line, qty: line.qty + 1 } : line,
          ),
        };
      }
      if (maxQty != null && maxQty <= 0) return state;
      const basePrice = toNumber(item.price) ?? 0;
      const unitPrice = tier?.price != null ? Number(tier.price) : basePrice;
      const tierId = tier?.tierId ?? state.customer?.price_tier_id ?? null;
      const tierName = tier?.tierName ?? null;
      const unitsPerPackage = tier?.unitsPerPackage ?? 1;

      return {
        lines: [
          ...state.lines,
          {
            key: newKey(),
            productId: item.product_id,
            name: item.name,
            basePrice,
            unitPrice,
            taxRate: toNumber(item.tax_rate),
            qty: 1,
            maxQty,
            tierId,
            tierName,
            unitsPerPackage,
          },
        ],
      };
    }),
  addCustom: (item) =>
    set((state) => ({
      lines: [
        ...state.lines,
        {
          key: newKey(),
          productId: null,
          name: item.name,
          unitPrice: item.price,
          basePrice: item.price,
          taxRate: CUSTOM_TAX_RATES[item.tax],
          qty: item.qty,
          maxQty: null,
          customName: item.name,
          customTax: item.tax,
        },
      ],
    })),
  setLineTier: (key, tier) =>
    set((state) => ({
      lines: state.lines.map((line) => {
        if (line.key !== key) return line;
        const unitPrice = tier.price != null ? Number(tier.price) : (line.basePrice ?? line.unitPrice);
        return {
          ...line,
          tierId: tier.tierId,
          tierName: tier.tierName ?? null,
          unitPrice,
          unitsPerPackage: tier.unitsPerPackage ?? 1,
          priceOverride: null,
          overrideReason: null,
        };
      }),
    })),
  setPriceOverride: (key, price, reason) =>
    set((state) => ({
      lines: state.lines.map((line) =>
        line.key === key ? { ...line, priceOverride: price, overrideReason: reason } : line,
      ),
    })),
  setQty: (key, qty) =>
    set((state) => ({
      lines: state.lines
        .map((line) =>
          line.key === key
            ? { ...line, qty: line.maxQty != null ? Math.min(qty, line.maxQty) : qty }
            : line,
        )
        .filter((line) => line.qty > 0),
    })),
  remove: (key) =>
    set((state) => ({ lines: state.lines.filter((line) => line.key !== key) })),
  clear: () =>
    set({
      lines: [],
      customer: null,
      customerAlias: null,
      couponCode: null,
      couponDiscount: 0,
      couponError: null,
    }),
}));

/** Precio efectivo por unidad vendida: el cambiado a mano o el resuelto. */
export function linePrice(line: CartLine): number {
  return line.priceOverride ?? line.unitPrice;
}

/** Ítems para la API (cotización y venta), en el formato de `SaleItem`. */
export function toSaleItems(lines: CartLine[]): QuoteItemInput[] {
  return lines.map((line) =>
    line.productId === null
      ? {
          product_id: null,
          custom_name: line.customName ?? line.name,
          custom_tax: line.customTax ?? "EXENTO",
          qty: String(line.qty),
          unit_price_override: String(line.unitPrice),
        }
      : {
          product_id: line.productId,
          qty: String(line.qty),
          tier_id: line.tierId ?? null,
          unit_price_override: line.priceOverride != null ? String(line.priceOverride) : null,
          price_override_reason: line.priceOverride != null ? line.overrideReason ?? null : null,
          discount_pct: line.discountPct != null ? String(line.discountPct) : null,
        },
  );
}

export type CartTotals = { subtotal: number; tax: number; total: number; units: number; discountTotal: number };

/** Redondeo a centavos como `numeric(14,2)` de Postgres (mitad hacia afuera). */
function roundCents(value: number): number {
  return (Math.sign(value) * Math.round(Math.abs(value) * 100)) / 100;
}

/**
 * Totales con precios SIN impuesto (regla del negocio): el IVA se suma encima.
 * Replica el calculo del backend (order_items.tax_amount, por linea) para que
 * lo que ve el cajero coincida con lo que cobra el servidor.
 */
export function cartTotals(lines: CartLine[], couponDiscount: number = 0): CartTotals {
  let subtotal = 0;
  let tax = 0;
  let units = 0;
  let discountTotal = 0;
  for (const line of lines) {
    const gross = roundCents(linePrice(line) * line.qty);
    // Una linea con precio cambiado no recibe promociones (F1.5).
    const promoDisc =
      line.priceOverride != null ? 0 : roundCents((line.promotionDiscount ?? 0) * line.qty);
    const base = Math.max(0, gross - promoDisc);
    subtotal += base;
    discountTotal += promoDisc;
    tax += line.taxRate ? roundCents(base * line.taxRate) : 0;
    units += line.qty;
  }
  const couponD = roundCents(couponDiscount);
  discountTotal = roundCents(discountTotal + couponD);
  subtotal = roundCents(Math.max(0, subtotal - couponD));
  tax = roundCents(tax);
  return { subtotal, tax, total: roundCents(subtotal + tax), units, discountTotal };
}

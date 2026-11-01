"use client";

import { create } from "zustand";

import type { CatalogItem, Customer } from "@/lib/api";
import { toNumber } from "@/lib/format";

export type CartLine = {
  productId: string;
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
};

export type AddItemTier = {
  tierId: string;
  tierName: string;
  price?: number | null;
  unitsPerPackage?: number | null;
};

type CartState = {
  lines: CartLine[];
  customer: Customer | null;
  customerAlias: string | null;
  add: (item: CatalogItem, tier?: AddItemTier) => void;
  setQty: (productId: string, qty: number) => void;
  setLineTier: (
    productId: string,
    tier: { tierId: string | null; tierName?: string | null; price?: number | null; unitsPerPackage?: number | null },
  ) => void;
  remove: (productId: string) => void;
  setCustomer: (customer: Customer | null) => void;
  setCustomerAlias: (alias: string | null) => void;
  clear: () => void;
};

export const useCart = create<CartState>((set) => ({
  lines: [],
  customer: null,
  customerAlias: null,
  setCustomer: (customer) =>
    set((state) => {
      const oldTierId = state.customer?.price_tier_id;
      const newTierId = customer?.price_tier_id;
      const updatedLines = state.lines.map((line) => {
        // If customer has a tier and line has no tier, assign customer tier
        if (newTierId && !line.tierId) {
          return { ...line, tierId: newTierId };
        }
        // If customer was removed or changed, and line had the old customer tier, clear it
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
      const existing = state.lines.find((line) => line.productId === item.product_id && line.tierId === (tier?.tierId ?? null));
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
  setLineTier: (productId, tier) =>
    set((state) => ({
      lines: state.lines.map((line) => {
        if (line.productId !== productId) return line;
        const unitPrice = tier.price != null ? Number(tier.price) : (line.basePrice ?? line.unitPrice);
        return {
          ...line,
          tierId: tier.tierId,
          tierName: tier.tierName ?? null,
          unitPrice,
          unitsPerPackage: tier.unitsPerPackage ?? 1,
        };
      }),
    })),
  setQty: (productId, qty) =>
    set((state) => ({
      lines: state.lines
        .map((line) =>
          line.productId === productId
            ? { ...line, qty: line.maxQty != null ? Math.min(qty, line.maxQty) : qty }
            : line,
        )
        .filter((line) => line.qty > 0),
    })),
  remove: (productId) =>
    set((state) => ({ lines: state.lines.filter((line) => line.productId !== productId) })),
  clear: () => set({ lines: [], customer: null, customerAlias: null }),
}));

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
export function cartTotals(lines: CartLine[]): CartTotals {
  let subtotal = 0;
  let tax = 0;
  let units = 0;
  let discountTotal = 0;
  for (const line of lines) {
    const gross = roundCents(line.unitPrice * line.qty);
    const promoDisc = roundCents((line.promotionDiscount ?? 0) * line.qty);
    const base = Math.max(0, gross - promoDisc);
    subtotal += base;
    discountTotal += promoDisc;
    tax += line.taxRate ? roundCents(base * line.taxRate) : 0;
    units += line.qty;
  }
  subtotal = roundCents(subtotal);
  discountTotal = roundCents(discountTotal);
  tax = roundCents(tax);
  return { subtotal, tax, total: roundCents(subtotal + tax), units, discountTotal };
}

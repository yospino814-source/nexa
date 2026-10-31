"use client";

import { create } from "zustand";

import type { CatalogItem } from "@/lib/api";
import { toNumber } from "@/lib/format";

export type CartLine = {
  productId: string;
  name: string;
  unitPrice: number;
  /** Fraccion (0.19) o null si es exento. */
  taxRate: number | null;
  qty: number;
  /** Tope de cantidad segun el stock disponible (null = sin control). */
  maxQty: number | null;
};

type CartState = {
  lines: CartLine[];
  add: (item: CatalogItem) => void;
  setQty: (productId: string, qty: number) => void;
  remove: (productId: string) => void;
  clear: () => void;
};

export const useCart = create<CartState>((set) => ({
  lines: [],
  add: (item) =>
    set((state) => {
      const maxQty = toNumber(item.available);
      const existing = state.lines.find((line) => line.productId === item.product_id);
      if (existing) {
        if (existing.maxQty != null && existing.qty >= existing.maxQty) return state;
        return {
          lines: state.lines.map((line) =>
            line === existing ? { ...line, qty: line.qty + 1 } : line,
          ),
        };
      }
      if (maxQty != null && maxQty <= 0) return state;
      return {
        lines: [
          ...state.lines,
          {
            productId: item.product_id,
            name: item.name,
            unitPrice: toNumber(item.price) ?? 0,
            taxRate: toNumber(item.tax_rate),
            qty: 1,
            maxQty,
          },
        ],
      };
    }),
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
  clear: () => set({ lines: [] }),
}));

export type CartTotals = { subtotal: number; tax: number; total: number; units: number };

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
  for (const line of lines) {
    const base = roundCents(line.unitPrice * line.qty);
    subtotal += base;
    tax += line.taxRate ? roundCents(base * line.taxRate) : 0;
    units += line.qty;
  }
  subtotal = roundCents(subtotal);
  tax = roundCents(tax);
  return { subtotal, tax, total: roundCents(subtotal + tax), units };
}

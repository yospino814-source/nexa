"use client";

import { useEffect, useState } from "react";
import { PauseCircle } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { formatTime } from "@/lib/format";
import { useCart, type HeldCart } from "@/lib/stores/cart";

/** Máximo de ventas en espera por sucursal (F1.14). */
export const MAX_HELD = 10;

const storageKey = (branchId: string) => `nexa.pos.held.${branchId}`;

export function readHeld(branchId: string): HeldCart[] {
  try {
    const raw = localStorage.getItem(storageKey(branchId));
    return raw ? (JSON.parse(raw) as HeldCart[]) : [];
  } catch {
    return [];
  }
}

function writeHeld(branchId: string, held: HeldCart[]) {
  localStorage.setItem(storageKey(branchId), JSON.stringify(held));
  window.dispatchEvent(new Event("nexa-held-changed"));
}

/**
 * Pone la venta actual en espera y vacía el carrito. Devuelve un mensaje de
 * error si ya hay 10 en espera, o `null` si se guardó.
 */
export function holdCurrentSale(branchId: string): string | null {
  const state = useCart.getState();
  if (state.lines.length === 0) return null;
  const held = readHeld(branchId);
  if (held.length >= MAX_HELD) return "Tenés 10 ventas en espera. Retomá o descartá una.";
  const now = new Date();
  const label =
    state.customer?.name ??
    state.customerAlias ??
    `Venta ${now.toLocaleTimeString("es-CO", { hour: "2-digit", minute: "2-digit", hour12: false })}`;
  held.push({
    id: state.saleId,
    label,
    created_at: now.toISOString(),
    cart: {
      lines: state.lines,
      customer: state.customer,
      customerAlias: state.customerAlias,
      couponCode: state.couponCode,
    },
  });
  writeHeld(branchId, held);
  state.clear();
  return null;
}

/** Botón "En espera (N)" con la lista para retomar o descartar (F1.14). */
export function HeldSales({ branchId }: { branchId: string }) {
  const [held, setHeld] = useState<HeldCart[]>([]);
  const [open, setOpen] = useState(false);
  const restore = useCart((s) => s.restore);
  const hasLines = useCart((s) => s.lines.length > 0);

  useEffect(() => {
    const refresh = () => setHeld(readHeld(branchId));
    refresh();
    window.addEventListener("nexa-held-changed", refresh);
    window.addEventListener("storage", refresh);
    return () => {
      window.removeEventListener("nexa-held-changed", refresh);
      window.removeEventListener("storage", refresh);
    };
  }, [branchId]);

  function resume(h: HeldCart) {
    // La venta actual, si tiene líneas, pasa a espera antes de retomar la otra.
    if (hasLines && holdCurrentSale(branchId)) return;
    const rest = readHeld(branchId).filter((x) => x.id !== h.id);
    writeHeld(branchId, rest);
    restore(h);
    setOpen(false);
  }

  function discard(h: HeldCart) {
    writeHeld(
      branchId,
      readHeld(branchId).filter((x) => x.id !== h.id),
    );
  }

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        disabled={held.length === 0}
        className="flex h-10 items-center gap-1.5 rounded-full border border-slate-200 px-3 text-xs font-medium hover:bg-slate-50 disabled:opacity-40"
      >
        <PauseCircle className="size-4" /> En espera ({held.length})
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="rounded-3xl p-6 sm:max-w-md">
          <DialogTitle className="text-xl font-bold">Ventas en espera</DialogTitle>
          <ul className="mt-4 divide-y divide-slate-100">
            {held.map((h) => (
              <li key={h.id} className="flex items-center justify-between gap-3 py-3">
                <div className="min-w-0">
                  <p className="truncate font-medium">{h.label}</p>
                  <p className="text-xs text-slate-500">
                    {formatTime(h.created_at)} · {h.cart.lines.length} productos
                  </p>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => resume(h)}
                    className="rounded-full bg-brand px-3 py-1.5 text-xs font-semibold text-white"
                  >
                    Retomar
                  </button>
                  <button
                    onClick={() => discard(h)}
                    className="rounded-full border border-slate-200 px-3 py-1.5 text-xs font-medium hover:bg-slate-50"
                  >
                    Descartar
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </DialogContent>
      </Dialog>
    </>
  );
}

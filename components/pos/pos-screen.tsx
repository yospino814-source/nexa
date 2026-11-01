"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronUp, X } from "lucide-react";

import { api, type BranchSettings, type CatalogItem, type PaymentMethod } from "@/lib/api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP } from "@/lib/format";
import { cartTotals, useCart } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

import { CartPanel } from "./cart-panel";
import { CatalogPanel } from "./catalog-panel";
import { CheckoutDialog } from "./checkout-dialog";
import { CustomItemDialog } from "./custom-item-dialog";

/** Pantalla 13: POS mostrador (catalogo 60% / carrito 40%). */
export function PosScreen() {
  const branchId = useCurrentBranch().id;
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [method, setMethod] = useState<PaymentMethod>("EFECTIVO");
  const [checkoutOpen, setCheckoutOpen] = useState(false);
  const [cartOpen, setCartOpen] = useState(false);
  const [customerDialogOpen, setCustomerDialogOpen] = useState(false);
  const [customItemOpen, setCustomItemOpen] = useState(false);
  const [settings, setSettings] = useState<BranchSettings | null>(null);
  const can = useSession((s) => s.can);
  const searchRef = useRef<HTMLInputElement>(null);

  const lines = useCart((state) => state.lines);
  const add = useCart((state) => state.add);
  const clear = useCart((state) => state.clear);
  const totals = cartTotals(lines);

  const loadCatalog = useCallback(async () => {
    try {
      setCatalog(await api.catalog.list(branchId));
      setLoadError(null);
    } catch {
      setLoadError("No se pudo cargar el catálogo.");
    } finally {
      setLoading(false);
    }
  }, [branchId]);

  useEffect(() => {
    // Ajustes de la sucursal: cambio de precio (F1.9) y propinas (F1.10).
    let alive = true;
    api.settings
      .get(branchId)
      .then((s) => alive && setSettings(s))
      .catch(() => alive && setSettings(null));
    return () => {
      alive = false;
    };
  }, [branchId]);

  useEffect(() => {
    // Carga inicial del catalogo de la sucursal.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadCatalog();
  }, [loadCatalog]);

  const openCheckout = useCallback(() => {
    if (useCart.getState().lines.length > 0) setCheckoutOpen(true);
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (checkoutOpen || customerDialogOpen || customItemOpen) return;
      if (e.key === "F2") {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === "F4") {
        e.preventDefault();
        setCustomerDialogOpen(true);
      } else if (e.key === "F12" || (e.code === "Space" && e.target === document.body)) {
        e.preventDefault();
        openCheckout();
      } else if (e.key === "Escape") {
        clear();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [checkoutOpen, customerDialogOpen, customItemOpen, clear, openCheckout]);

  const cart = (
    <CartPanel
      method={method}
      onMethodChange={setMethod}
      onCharge={openCheckout}
      customerDialogOpen={customerDialogOpen}
      onCustomerDialogOpenChange={setCustomerDialogOpen}
      allowPriceOverride={settings?.pos.allow_price_override ?? false}
    />
  );

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col lg:w-3/5 lg:flex-none">
        {loadError && (
          <p className="mx-6 mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">
            {loadError}{" "}
            <button className="font-semibold underline" onClick={loadCatalog}>
              Reintentar
            </button>
          </p>
        )}
        <CatalogPanel
          items={catalog}
          loading={loading}
          onAdd={add}
          searchRef={searchRef}
          onCustomItem={can("pos:custom_item") ? () => setCustomItemOpen(true) : undefined}
        />
      </div>

      {/* Escritorio / tablet horizontal: carrito fijo a la derecha */}
      <aside className="hidden w-2/5 border-l border-slate-200 lg:block">{cart}</aside>

      {/* Celular / tablet vertical: barra inferior que se expande como hoja */}
      <button
        onClick={() => setCartOpen(true)}
        className="fixed inset-x-3 bottom-3 z-20 flex h-14 items-center justify-between rounded-2xl bg-brand px-5 font-semibold text-white shadow-xl lg:hidden"
      >
        <span>
          {totals.units} productos · {formatCOP(totals.total)}
        </span>
        <span className="flex items-center gap-1">
          Ver carrito <ChevronUp className="size-4" />
        </span>
      </button>
      <div
        className={cn(
          "fixed inset-0 z-40 bg-slate-900/40 transition-opacity lg:hidden",
          cartOpen ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onClick={() => setCartOpen(false)}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          className={cn(
            "absolute inset-x-0 bottom-0 h-[88vh] overflow-hidden rounded-t-3xl transition-transform",
            cartOpen ? "translate-y-0" : "translate-y-full",
          )}
        >
          <button
            onClick={() => setCartOpen(false)}
            aria-label="Cerrar carrito"
            className="absolute top-3 right-3 z-10 flex size-8 items-center justify-center rounded-full bg-slate-100"
          >
            <X className="size-4" />
          </button>
          {cart}
        </div>
      </div>

      <CustomItemDialog open={customItemOpen} onOpenChange={setCustomItemOpen} />

      <CheckoutDialog
        open={checkoutOpen}
        onOpenChange={setCheckoutOpen}
        lines={lines}
        initialMethod={method}
        branchId={branchId}
        tipsEnabled={settings?.tips.enabled ?? false}
        suggestedTipPct={Number(settings?.tips.suggested_pct ?? 0)}
        onCompleted={() => {
          clear();
          setCartOpen(false);
          loadCatalog();
          searchRef.current?.focus();
        }}
      />
    </div>
  );
}

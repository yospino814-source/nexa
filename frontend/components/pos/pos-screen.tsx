"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronUp, X } from "lucide-react";

import { api, type BranchSettings, type CatalogItem, type OptionGroupDetail, type PaymentMethod } from "@/lib/api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP } from "@/lib/format";
import { useCart, type AddItemTier } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

import { CartPanel } from "./cart-panel";
import { CatalogPanel } from "./catalog-panel";
import { CheckoutDialog } from "./checkout-dialog";
import { CustomItemDialog } from "./custom-item-dialog";
import { CustomizeDialog } from "./customize-dialog";
import { holdCurrentSale } from "./held-sales";
import { useLiveQuote } from "./use-live-quote";

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
  const [discountOpen, setDiscountOpen] = useState(false);
  const [customizeItem, setCustomizeItem] = useState<{
    item: CatalogItem;
    tier?: AddItemTier;
    groups: OptionGroupDetail[];
  } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [settings, setSettings] = useState<BranchSettings | null>(null);
  const [registerName, setRegisterName] = useState<string | null>(null);
  const can = useSession((s) => s.can);
  const searchRef = useRef<HTMLInputElement>(null);

  const lines = useCart((state) => state.lines);
  const add = useCart((state) => state.add);
  const clear = useCart((state) => state.clear);
  const quote = useCart((state) => state.quote);
  const units = lines.reduce((s, l) => s + l.qty, 0);

  // F1.14: cotización en vivo; los totales del POS salen solo de aquí.
  useLiveQuote(branchId);

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
    // Caja del turno abierto, para la tirilla (F1.15).
    api.cash
      .current(branchId)
      .then((c) => alive && setRegisterName(c?.session.register_name ?? null))
      .catch(() => alive && setRegisterName(null));
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

  const handleCatalogAdd = useCallback(
    async (item: CatalogItem, tier?: AddItemTier) => {
      try {
        const prod = await api.products.get(item.product_id, branchId);
        if (prod.option_groups && prod.option_groups.length > 0) {
          setCustomizeItem({ item, tier, groups: prod.option_groups });
          return;
        }
      } catch {
        // En caso de error consultando opciones, se añade directamente
      }
      add(item, tier);
    },
    [add, branchId],
  );

  useEffect(() => {
    /** Atajos de F1.14; solo con el POS activo y sin otro diálogo abierto (salvo Esc). */
    function onKey(e: KeyboardEvent) {
      const fKey = /^F([1-9]|1[0-2])$/.test(e.key);
      const dialogOpen = checkoutOpen || customerDialogOpen || customItemOpen || discountOpen;
      if (fKey) e.preventDefault(); // El navegador no actúa con F1–F12.
      if (dialogOpen) return; // Esc lo maneja cada diálogo; F10 lo maneja el cobro.

      const target = e.target as HTMLElement | null;
      const typing =
        !!target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT");

      if (e.key === "F2") {
        searchRef.current?.focus();
      } else if (e.key === "F4") {
        setCustomerDialogOpen(true);
      } else if (e.key === "F6") {
        const { selectedKey, lines: current } = useCart.getState();
        const selected = current.find((l) => l.key === selectedKey);
        if (can("pos:discount") && selected && selected.productId !== null) setDiscountOpen(true);
      } else if (e.key === "F8") {
        setNotice(holdCurrentSale(branchId));
      } else if (e.key === "F9") {
        openCheckout();
      } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[\p{L}\p{N}]$/u.test(e.key)) {
        // Escribir para buscar: la tecla pasa al buscador ya escrita.
        e.preventDefault();
        const input = searchRef.current;
        if (input) {
          input.focus();
          const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
          setter?.call(input, input.value + e.key);
          input.dispatchEvent(new Event("input", { bubbles: true }));
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [checkoutOpen, customerDialogOpen, customItemOpen, discountOpen, can, branchId, openCheckout]);

  const cart = (
    <CartPanel
      method={method}
      onMethodChange={setMethod}
      onCharge={openCheckout}
      customerDialogOpen={customerDialogOpen}
      onCustomerDialogOpenChange={setCustomerDialogOpen}
      discountDialogOpen={discountOpen}
      onDiscountDialogOpenChange={setDiscountOpen}
      allowPriceOverride={settings?.pos.allow_price_override ?? false}
      maxDiscountPct={Number(settings?.pos.max_discount_pct ?? 0)}
      ticketWidth={settings?.print.ticket_width_mm ?? 80}
    />
  );

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col lg:w-3/5 lg:flex-none">
        {notice && (
          <p className="mx-6 mt-4 flex items-center justify-between rounded-xl bg-amber-50 p-3 text-sm text-amber-700">
            {notice}
            <button onClick={() => setNotice(null)} aria-label="Cerrar aviso">
              <X className="size-4" />
            </button>
          </p>
        )}
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
          onAdd={handleCatalogAdd}
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
          {units} productos · {formatCOP(Number(quote?.total ?? 0))}
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
        ticketWidth={settings?.print.ticket_width_mm ?? 80}
        registerName={registerName}
        onCompleted={() => {
          clear();
          setCartOpen(false);
          loadCatalog();
          searchRef.current?.focus();
        }}
      />

      {customizeItem && (
        <CustomizeDialog
          open={!!customizeItem}
          item={customizeItem.item}
          optionGroups={customizeItem.groups}
          onClose={() => setCustomizeItem(null)}
          onConfirm={(optionIds, optionNames, priceDelta) => {
            add(customizeItem.item, customizeItem.tier, { optionIds, optionNames, priceDelta });
          }}
        />
      )}
    </div>
  );
}

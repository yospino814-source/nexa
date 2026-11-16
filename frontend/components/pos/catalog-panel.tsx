"use client";

import { useMemo, useRef, useState, type RefObject } from "react";
import { Plus, Search } from "lucide-react";

import type { CatalogItem } from "@/lib/api";
import { formatCOP, toNumber } from "@/lib/format";
import { useCart, type AddItemTier } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

import { flyToCart } from "@/lib/fly-to-cart";
import { productImageUrl } from "@/lib/product-image";

import { ProductArt } from "./product-art";

const LOW_STOCK = 10;

/** Componente compartido "Catálogo": buscador + categorias + grilla. */
export function CatalogPanel({
  items,
  loading,
  onAdd,
  searchRef,
  onCustomItem,
}: {
  items: CatalogItem[];
  loading: boolean;
  onAdd: (item: CatalogItem, tier?: AddItemTier) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  /** F1.9: abre el diálogo de ítem libre. Si no se pasa, el botón no aparece. */
  onCustomItem?: () => void;
}) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [scanMessage, setScanMessage] = useState<string | null>(null);
  // F1.14: marcas de tiempo de las teclas, para distinguir el lector del tecleo humano.
  const keyTimes = useRef<number[]>([]);

  const categories = useMemo(
    () => [...new Set(items.map((item) => item.category).filter((c): c is string => !!c))],
    [items],
  );

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter(
      (item) =>
        (!category || item.category === category) &&
        (!q ||
          item.name.toLowerCase().includes(q) ||
          item.product_id === q ||
          item.barcode?.toLowerCase().includes(q) ||
          item.tier_barcodes?.some((tb) => tb.barcode.toLowerCase().includes(q))),
    );
  }, [items, query, category]);

  /** Lector: 6 o más caracteres con menos de 50 ms entre teclas, terminados en Enter. */
  function isScannerBurst(code: string): boolean {
    // La primera tecla puede llegar por "escribir para buscar" (fuera del
    // buscador), así que se aceptan code.length − 1 marcas de tiempo.
    const times = keyTimes.current;
    const n = Math.min(times.length, code.length);
    if (code.length < 6 || n < code.length - 1) return false;
    const last = times.slice(-n);
    return last.every((t, i) => i === 0 || t - last[i - 1] < 50);
  }

  /** Busca el código exacto en productos y presentaciones y agrega una unidad. */
  function addByBarcode(code: string) {
    for (const item of items) {
      if (item.barcode === code) {
        onAdd(item);
        return true;
      }
      const tb = item.tier_barcodes?.find((b) => b.barcode === code);
      if (tb) {
        onAdd(item, { tierId: tb.tier_id, tierName: "Presentación" });
        return true;
      }
    }
    return false;
  }

  // Enter: código del lector, o el primer resultado disponible al teclear a mano.
  function handleEnter() {
    const code = query.trim();
    keyTimes.current = [];
    if (isScannerBurst(code)) {
      setQuery("");
      setScanMessage(addByBarcode(code) ? null : `Código no encontrado: ${code}`);
      return;
    }
    const q = code.toLowerCase();
    for (const item of items) {
      const tb = item.tier_barcodes?.find((b) => b.barcode.toLowerCase() === q);
      if (tb && (toNumber(item.available) ?? 1) > 0) {
        onAdd(item, { tierId: tb.tier_id, tierName: "Presentación" });
        setQuery("");
        return;
      }
    }
    const match = visible.find((item) => (toNumber(item.available) ?? 1) > 0);
    if (match) {
      onAdd(match);
      setQuery("");
    }
  }

  return (
    <section aria-label="Catálogo" className="flex min-h-0 flex-1 flex-col gap-4 px-4 pt-5 lg:px-6">
      <div className="flex flex-wrap items-center gap-2.5">
        <label className="flex h-14 min-w-0 flex-[1_1_320px] items-center gap-2 rounded-[14px] border-2 border-indigo-600 bg-white px-4 text-indigo-600 shadow-[0_0_0_4px_rgb(61_79_224/0.14)]">
          <Search className="size-[22px] shrink-0" strokeWidth={2} aria-hidden />
          <span className="sr-only">Buscar producto</span>
          <input
            ref={searchRef}
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleEnter();
              else if (e.key.length === 1) keyTimes.current = [...keyTimes.current.slice(-40), performance.now()];
            }}
            placeholder="Buscar o escanear código de barras (F2)"
            className="h-full min-w-0 flex-1 bg-transparent text-base text-slate-900 outline-none placeholder:text-slate-400"
          />
          <kbd className="hidden rounded-md bg-slate-100 px-1.5 py-0.5 text-[11px] font-semibold text-slate-500 sm:block">F2</kbd>
        </label>
        <span className="hidden h-8 items-center gap-1.5 rounded-full bg-emerald-50 px-3 text-xs font-semibold text-emerald-700 sm:inline-flex">
          <span className="size-[7px] rounded-full bg-emerald-500" />
          Lector activo
        </span>
        {onCustomItem && (
          <button
            type="button"
            onClick={onCustomItem}
            className="h-12 shrink-0 rounded-xl border border-slate-300 bg-white px-5 text-[15px] font-semibold text-slate-900 shadow-xs transition-transform hover:bg-slate-50 active:scale-[0.97]"
          >
            Ítem libre
          </button>
        )}
      </div>

      {scanMessage && (
        <p className="flex items-center justify-between rounded-xl bg-amber-50 px-3 py-2 text-sm text-amber-700">
          {scanMessage}
          <button onClick={() => setScanMessage(null)} className="text-xs font-semibold underline">
            Cerrar
          </button>
        </p>
      )}

      <div role="group" aria-label="Categorías" className="-mx-1 flex gap-2 overflow-x-auto px-1 py-0.5 [scrollbar-width:none]">
        {[null, ...categories].map((name) => (
          <button
            key={name ?? "all"}
            onClick={() => setCategory(name)}
            aria-pressed={category === name}
            className={cn(
              "h-10 shrink-0 rounded-full border px-4 text-[13px] whitespace-nowrap transition-colors",
              category === name
                ? "border-carbon bg-carbon font-semibold text-white"
                : "border-slate-200 bg-white font-medium text-slate-700 hover:bg-slate-50",
            )}
          >
            {name ?? "Todas"}
          </button>
        ))}
      </div>

      <div className="grid min-h-0 flex-1 auto-rows-min grid-cols-[repeat(auto-fill,minmax(172px,1fr))] gap-3 overflow-y-auto pb-24 lg:pb-4">
        {loading &&
          Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="h-[236px] animate-pulse rounded-[22px] bg-slate-200/60" />
          ))}
        {!loading && visible.length === 0 && (
          <p className="col-span-full py-12 text-center text-sm text-slate-500">
            No hay productos que coincidan con “{query}”.
          </p>
        )}
        {!loading && visible.map((item) => <ProductCard key={item.product_id} item={item} onAdd={onAdd} />)}
      </div>
    </section>
  );
}

function ProductCard({ item, onAdd }: { item: CatalogItem; onAdd: (item: CatalogItem) => void }) {
  const available = toNumber(item.available);
  const soldOut = available != null && available <= 0;
  const low = available != null && available > 0 && available <= LOW_STOCK;
  const photo = productImageUrl(item.product_id, item.image_version);
  const artRef = useRef<HTMLSpanElement>(null);
  const inCart = useCart((st) =>
    st.lines.reduce((n, l) => n + (l.productId === item.product_id ? l.qty : 0), 0),
  );
  const [pressed, setPressed] = useState(0);

  return (
    <button
      type="button"
      disabled={soldOut}
      onClick={() => {
        onAdd(item);
        flyToCart(artRef.current);
        setPressed((n) => n + 1);
      }}
      className={cn(
        "group relative flex flex-col gap-2 rounded-[22px] bg-white p-2 pb-3 text-left shadow-[0_1px_2px_rgb(20_22_28/0.04),0_14px_32px_-22px_rgb(20_22_28/0.35)] ring-1 ring-slate-200/70 transition-[transform,box-shadow] duration-200 ease-[cubic-bezier(.2,.7,.2,1)]",
        inCart > 0 && "ring-2 ring-indigo-600",
        soldOut
          ? "cursor-not-allowed opacity-55"
          : "hover:-translate-y-1 hover:shadow-[0_2px_4px_rgb(20_22_28/0.05),0_24px_40px_-20px_rgb(61_79_224/0.35)] active:scale-[0.97]",
      )}
    >
      <span ref={artRef} className="relative grid h-[168px] place-items-center overflow-hidden rounded-[18px] bg-[radial-gradient(circle_at_50%_32%,#FFFFFF_0%,#F3F4F8_62%,#E9EBF1_100%)]">
        {photo ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={photo}
            alt=""
            loading="lazy"
            decoding="async"
            className={cn(
              "absolute inset-0 size-full object-contain px-3 pt-2.5 pb-4 mix-blend-multiply transition-transform duration-300 group-hover:scale-[1.06]",
              soldOut && "grayscale",
            )}
          />
        ) : (
          <span className="transition-transform duration-300 group-hover:scale-[1.06]">
            <ProductArt name={item.name} category={item.category} muted={soldOut} height={130} bare />
          </span>
        )}
        <span aria-hidden data-fly-skip className="absolute inset-x-14 bottom-3 h-2 rounded-full bg-slate-900/10 blur-sm" />
        {inCart > 0 && (
          <span
            key={inCart}
            data-fly-skip
            className="nx-pop absolute top-2 right-2 grid h-7 min-w-7 place-items-center rounded-full bg-indigo-600 px-2 text-[13px] font-bold text-white tabular-nums shadow-[0_6px_14px_-4px_rgb(61_79_224/0.6)]"
          >
            ×{inCart}
          </span>
        )}
        {(soldOut || low) && (
          <span
            data-fly-skip
            className={cn(
              "absolute top-2 left-2 inline-flex h-[22px] items-center rounded-full px-2 text-[11px] font-semibold whitespace-nowrap backdrop-blur",
              soldOut ? "bg-rose-50/90 text-rose-700" : "bg-amber-50/90 text-amber-700",
            )}
          >
            {soldOut ? "Sin existencias" : `Quedan ${available}`}
          </span>
        )}
      </span>
      <span className="line-clamp-2 min-h-[38px] px-1.5 text-sm leading-[1.35] font-medium text-slate-900">{item.name}</span>
      <span className="mt-auto flex items-center justify-between gap-2 px-1.5">
        <span
          className={cn(
            "text-lg font-bold tracking-[-0.02em] tabular-nums",
            soldOut ? "text-slate-400 line-through" : "text-slate-900",
          )}
        >
          {formatCOP(toNumber(item.price) ?? 0)}
        </span>
        {!soldOut && (
          <span
            key={pressed}
            aria-hidden
            className={cn(
              "grid size-8 shrink-0 place-items-center rounded-full bg-carbon text-white transition-transform group-hover:scale-110 group-active:scale-95",
              pressed > 0 && "nx-pop",
            )}
          >
            <Plus className="size-4" strokeWidth={2.4} />
          </span>
        )}
      </span>
    </button>
  );
}

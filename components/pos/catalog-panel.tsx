"use client";

import { useMemo, useState, type RefObject } from "react";
import {
  AlertTriangle,
  Apple,
  Candy,
  Coffee,
  Croissant,
  CupSoda,
  Milk,
  Package,
  ScanBarcode,
  ShoppingBasket,
  SprayCan,
  type LucideIcon,
} from "lucide-react";

import type { CatalogItem } from "@/lib/api";
import { formatCOP, taxLabel, toNumber } from "@/lib/format";
import type { AddItemTier } from "@/lib/stores/cart";
import { cn } from "@/lib/utils";

const LOW_STOCK = 10;

const CATEGORY_ICONS: Record<string, { icon: LucideIcon; tone: string }> = {
  Bebidas: { icon: CupSoda, tone: "bg-cyan-50 text-cyan-700" },
  Abarrotes: { icon: ShoppingBasket, tone: "bg-amber-50 text-amber-700" },
  Lácteos: { icon: Milk, tone: "bg-sky-50 text-sky-700" },
  Panadería: { icon: Croissant, tone: "bg-orange-50 text-orange-700" },
  "Aseo y Hogar": { icon: SprayCan, tone: "bg-indigo-50 text-indigo-700" },
  "Snacks y Dulces": { icon: Candy, tone: "bg-pink-50 text-pink-700" },
  Cafetería: { icon: Coffee, tone: "bg-amber-50 text-amber-800" },
  Frutas: { icon: Apple, tone: "bg-emerald-50 text-emerald-700" },
};

function categoryIcon(category: string | null) {
  return (category && CATEGORY_ICONS[category]) || { icon: Package, tone: "bg-slate-100 text-slate-600" };
}

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

  // Enter agrega el primer resultado disponible (flujo de lector de codigo de barras).
  function handleEnter() {
    const q = query.trim().toLowerCase();
    for (const item of items) {
      const tb = item.tier_barcodes?.find((b) => b.barcode.toLowerCase() === q);
      if (tb && (toNumber(item.available) ?? 1) > 0) {
        onAdd(item, {
          tierId: tb.tier_id,
          tierName: "Tarifa",
        });
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
    <section className="flex min-h-0 flex-1 flex-col gap-4 p-4 lg:p-6">
      <div className="flex items-center gap-3 rounded-2xl border border-slate-200 bg-white px-4 py-3 shadow-sm focus-within:border-brand focus-within:ring-3 focus-within:ring-brand/15">
        <ScanBarcode className="size-6 text-slate-700" />
        <input
          ref={searchRef}
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleEnter()}
          placeholder="Buscar o escanear código de barras (F2)"
          className="h-9 flex-1 bg-transparent text-base outline-none placeholder:text-slate-400"
        />
        <span className="hidden items-center gap-1.5 rounded-lg border border-teal-200 bg-teal-50 px-2 py-1 text-xs font-medium text-brand sm:flex">
          <span className="size-1.5 rounded-full bg-brand" />
          Lector activo
        </span>
        <kbd className="hidden rounded-md border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs text-slate-500 sm:block">
          F2
        </kbd>
        {onCustomItem && (
          <button
            type="button"
            onClick={onCustomItem}
            className="shrink-0 rounded-full border border-slate-200 px-3 py-1.5 text-xs font-semibold hover:bg-slate-50"
          >
            Ítem libre
          </button>
        )}
      </div>

      <div className="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1">
        {[null, ...categories].map((name) => (
          <button
            key={name ?? "all"}
            onClick={() => setCategory(name)}
            className={cn(
              "h-11 shrink-0 rounded-full border px-5 text-sm font-medium transition-colors",
              category === name
                ? "border-brand bg-brand text-white"
                : "border-slate-200 bg-white text-slate-700 hover:bg-slate-50",
            )}
          >
            {name ?? "Todas las categorías"}
          </button>
        ))}
      </div>

      <div className="grid min-h-0 flex-1 auto-rows-min grid-cols-2 gap-4 overflow-y-auto pb-24 sm:grid-cols-3 xl:grid-cols-4 lg:pb-4">
        {loading &&
          Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="h-44 animate-pulse rounded-2xl bg-slate-200/60" />
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
  const { icon: Icon, tone } = categoryIcon(item.category);

  return (
    <button
      disabled={soldOut}
      onClick={() => onAdd(item)}
      className={cn(
        "flex flex-col rounded-2xl border bg-white p-4 text-left transition-all",
        soldOut
          ? "cursor-not-allowed border-dashed border-slate-200 opacity-60"
          : "border-slate-200/60 shadow-sm hover:-translate-y-0.5 hover:border-brand/40 hover:shadow-md active:translate-y-0",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <span className={cn("flex size-10 items-center justify-center rounded-full", soldOut ? "bg-slate-100 text-slate-400" : tone)}>
          <Icon className="size-5" />
        </span>
        {soldOut ? (
          <span className="rounded-md border border-rose-200 bg-rose-50 px-2 py-0.5 text-[11px] font-semibold text-rose-600">
            Sin existencias
          </span>
        ) : low ? (
          <span className="flex items-center gap-1 rounded-md border border-amber-200 bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-700">
            <AlertTriangle className="size-3" />
            Quedan {available}
          </span>
        ) : (
          <span className="rounded-md border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[11px] font-semibold text-emerald-700">
            Disponible
          </span>
        )}
      </div>
      <p className="mt-3 line-clamp-2 font-medium text-slate-900">{item.name}</p>
      <p className="text-xs text-slate-500">{taxLabel(toNumber(item.tax_rate))}</p>
      <div className="mt-auto flex items-baseline justify-between border-t border-slate-100 pt-3">
        <span className="text-xs text-slate-400">COP</span>
        <span className={cn("text-xl font-bold", soldOut ? "text-slate-400 line-through" : "text-brand")}>
          {formatCOP(toNumber(item.price) ?? 0)}
        </span>
      </div>
    </button>
  );
}

"use client";

import { useMemo, useRef, useState } from "react";
import Link from "next/link";
import { ArrowLeft, Check, Loader2, Minus, Plus, ScanBarcode, SkipForward } from "lucide-react";

import { Nexi } from "@/components/nexa/nexi";
import { ProductArt } from "@/components/pos/product-art";
import { api, friendlyError, type StockLevel } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type Step = "pick" | "count" | "summary" | "done";

/**
 * Conteo de inventario con el celular (lienzo 24): se elige una categoría,
 * se cuenta producto por producto "a ciegas" (sin ver cuántas dice el
 * sistema) y al final se revisan las diferencias antes de ajustar.
 */
export function CountScreen() {
  const branch = useCurrentBranch();
  const { data: stock, loading, error } = useApi(() => api.inventory.stock(branch.id), branch.id);
  const [step, setStep] = useState<Step>("pick");
  const [category, setCategory] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [qty, setQty] = useState("");
  const [counted, setCounted] = useState<Record<string, number>>({});
  const [scan, setScan] = useState("");
  const [scanMsg, setScanMsg] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const [result, setResult] = useState<{ adjusted: number; value: number } | null>(null);
  const qtyRef = useRef<HTMLInputElement>(null);

  const countable = useMemo(
    () => (stock ?? []).filter((s) => !s.archived && (s.kind ?? "FISICO") === "FISICO"),
    [stock],
  );
  const categories = useMemo(() => {
    const map = new Map<string, number>();
    for (const s of countable) map.set(s.category ?? "Sin categoría", (map.get(s.category ?? "Sin categoría") ?? 0) + 1);
    return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0], "es"));
  }, [countable]);
  const list = useMemo(
    () =>
      countable
        .filter((s) => category === null || (s.category ?? "Sin categoría") === category)
        .sort((a, b) => a.name.localeCompare(b.name, "es")),
    [countable, category],
  );

  const current: StockLevel | undefined = list[index];
  const doneIds = list.filter((s) => counted[s.product_id] !== undefined);
  const progress = list.length ? (doneIds.length / list.length) * 100 : 0;

  const diffs = list
    .filter((s) => counted[s.product_id] !== undefined)
    .map((s) => {
      const system = Number(s.on_hand);
      const c = counted[s.product_id];
      const unitCost = Number(s.cost_price ?? s.price ?? 0);
      return { s, system, c, delta: c - system, value: (c - system) * unitCost };
    });
  const withDiff = diffs.filter((d) => d.delta !== 0);
  const moneyDiff = withDiff.reduce((t, d) => t + d.value, 0);

  function start(cat: string | null) {
    setCategory(cat);
    setIndex(0);
    setCounted({});
    setQty("");
    setStep("count");
  }

  function goTo(i: number) {
    const next = list[i];
    setIndex(i);
    setQty(next && counted[next.product_id] !== undefined ? String(counted[next.product_id]) : "");
    setTimeout(() => qtyRef.current?.focus(), 0);
  }

  function saveAndNext() {
    if (!current || qty.trim() === "") return;
    const n = Math.max(0, Math.round(Number(qty)));
    const next = { ...counted, [current.product_id]: n };
    setCounted(next);
    const pending = list.findIndex((s, i) => i > index && next[s.product_id] === undefined);
    const firstPending = list.findIndex((s) => next[s.product_id] === undefined);
    const target = pending >= 0 ? pending : firstPending;
    if (target < 0) setStep("summary");
    else goTo(target);
  }

  function findByCode(e: React.FormEvent) {
    e.preventDefault();
    const code = scan.trim().toLowerCase();
    if (!code) return;
    const i = list.findIndex(
      (s) => s.barcode?.toLowerCase() === code || s.sku?.toLowerCase() === code || s.name.toLowerCase().includes(code),
    );
    if (i < 0) {
      setScanMsg(`No está en esta categoría: ${scan.trim()}`);
      return;
    }
    setScanMsg(null);
    setScan("");
    goTo(i);
  }

  async function send() {
    setSending(true);
    setSendError(null);
    try {
      for (const d of withDiff) {
        await api.inventory.count(branch.id, d.s.product_id, String(d.c), `Conteo físico${category ? ` · ${category}` : ""}`);
      }
      setResult({ adjusted: withDiff.length, value: moneyDiff });
      setStep("done");
    } catch (e) {
      setSendError(friendlyError(e));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-[560px] flex-col bg-[#F3F4F8]">
      <header className="relative overflow-hidden bg-carbon px-[18px] pt-[18px] pb-16 text-white">
        <span aria-hidden className="absolute -top-[120px] -right-[90px] size-[260px] rounded-full bg-[radial-gradient(circle,rgba(108,62,245,.55),transparent_70%)]" />
        <span aria-hidden className="absolute -bottom-[120px] -left-20 size-[200px] rounded-full bg-[radial-gradient(circle,rgba(30,200,240,.35),transparent_70%)]" />
        <div className="relative flex items-center gap-3">
          <Link href="/inventario" aria-label="Salir del conteo" className="grid size-10 place-items-center rounded-[10px] bg-white/8">
            <ArrowLeft className="size-5" />
          </Link>
          <div className="min-w-0 flex-1">
            <p className="text-xs text-slate-400">Conteo físico · {branch.name}</p>
            <p className="truncate text-[17px] font-bold tracking-[-0.02em]">
              {step === "pick" ? "¿Qué vas a contar?" : (category ?? "Todo el inventario")}
            </p>
          </div>
          {step !== "pick" && (
            <span className="inline-flex h-7 items-center rounded-full bg-white/10 px-2.5 text-xs font-semibold tabular-nums">
              {doneIds.length} de {list.length}
            </span>
          )}
        </div>
        {step !== "pick" && (
          <div className="relative mt-4 h-1.5 overflow-hidden rounded-full bg-white/12">
            <div
              className="h-full rounded-full bg-linear-to-r from-grad-pink to-grad-violet transition-[width] duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        )}
      </header>

      <div className="relative -mt-12 flex flex-1 flex-col gap-3.5 px-4 pb-32">
        {loading && !stock ? (
          <div className="grid h-48 place-items-center rounded-[20px] bg-white shadow-card">
            <Loader2 className="size-6 animate-spin text-slate-400" aria-label="Cargando" />
          </div>
        ) : error ? (
          <p className="rounded-[20px] bg-white p-5 text-sm text-rose-700 shadow-card">{friendlyError(error)}</p>
        ) : step === "pick" ? (
          <>
            <section className="flex flex-col gap-2 rounded-[20px] bg-white p-4 shadow-card">
              <p className="px-1 text-sm text-slate-500">Elegí una parte. Es mejor contar por nevera o estante que todo de una.</p>
              {categories.map(([cat, n]) => (
                <button
                  key={cat}
                  type="button"
                  onClick={() => start(cat)}
                  className="flex h-14 items-center justify-between rounded-xl border border-slate-200 px-4 text-left text-[15px] font-semibold hover:border-indigo-200 hover:bg-indigo-50/40 active:scale-[0.98]"
                >
                  {cat}
                  <span className="text-[13px] font-medium text-slate-500 tabular-nums">{n} {n === 1 ? "producto" : "productos"}</span>
                </button>
              ))}
              <button
                type="button"
                onClick={() => start(null)}
                className="mt-1 h-12 rounded-xl text-sm font-semibold text-indigo-600 hover:bg-indigo-50"
              >
                Contar todo el inventario ({countable.length})
              </button>
            </section>
            <NexiNote expr="happy">
              Contá sin mirar el sistema: no te mostramos cuántas debería haber, así el número es el real.
            </NexiNote>
          </>
        ) : step === "count" && current ? (
          <>
            <section className="flex flex-col items-center gap-4 rounded-[20px] bg-white p-[18px] text-center shadow-card">
              <div className="w-[120px]">
                <ProductArt name={current.name} category={current.category} height={120} />
              </div>
              <div>
                <h1 className="text-xl font-semibold tracking-[-0.022em]">{current.name}</h1>
                <p className="mt-1 text-[13px] text-slate-500 tabular-nums">
                  {[current.barcode && `Código ${current.barcode}`, current.unit && `se cuenta en ${current.unit.toLowerCase()}`]
                    .filter(Boolean)
                    .join(" · ") || "Contá las unidades"}
                </p>
              </div>
              <div className="flex items-center gap-3.5">
                <button
                  type="button"
                  aria-label="Quitar una"
                  onClick={() => setQty(String(Math.max(0, Number(qty || 0) - 1)))}
                  className="grid size-14 place-items-center rounded-2xl border border-slate-300 bg-white shadow-xs active:scale-90"
                >
                  <Minus className="size-6" strokeWidth={2.2} />
                </button>
                <label>
                  <span className="sr-only">Cantidad contada</span>
                  <input
                    ref={qtyRef}
                    autoFocus
                    inputMode="numeric"
                    value={qty}
                    onChange={(e) => setQty(e.target.value.replace(/\D/g, "").slice(0, 6))}
                    onKeyDown={(e) => e.key === "Enter" && saveAndNext()}
                    placeholder="0"
                    className="h-[72px] w-[120px] rounded-[18px] border-2 border-indigo-600 text-center text-[44px] font-bold tracking-[-0.03em] tabular-nums shadow-[0_0_0_4px_rgb(61_79_224/0.14)] outline-none placeholder:text-slate-300"
                  />
                </label>
                <button
                  type="button"
                  aria-label="Sumar una"
                  onClick={() => setQty(String(Number(qty || 0) + 1))}
                  className="grid size-14 place-items-center rounded-2xl border border-slate-300 bg-white shadow-xs active:scale-90"
                >
                  <Plus className="size-6" strokeWidth={2.2} />
                </button>
              </div>
              <div className="grid w-full grid-cols-3 gap-2">
                {[
                  ["+6", () => setQty(String(Number(qty || 0) + 6))],
                  ["+24", () => setQty(String(Number(qty || 0) + 24))],
                  ["No hay", () => setQty("0")],
                ].map(([label, fn]) => (
                  <button
                    key={label as string}
                    type="button"
                    onClick={fn as () => void}
                    className="h-12 rounded-xl border border-slate-300 bg-white text-[15px] font-semibold shadow-xs active:scale-[0.97]"
                  >
                    {label as string}
                  </button>
                ))}
              </div>
              <button
                type="button"
                onClick={() => goTo(index + 1 < list.length ? index + 1 : 0)}
                className="flex items-center gap-1.5 text-[13px] font-semibold text-slate-500"
              >
                <SkipForward className="size-4" aria-hidden /> Saltar por ahora
              </button>
            </section>

            {doneIds.length > 0 && (
              <section className="overflow-hidden rounded-[20px] bg-white shadow-card">
                <div className="flex justify-between px-4 pt-3.5 pb-1.5">
                  <h2 className="text-[15px] font-semibold">Ya contados</h2>
                  <span className="text-[13px] text-slate-500 tabular-nums">{doneIds.length}</span>
                </div>
                {doneIds
                  .slice(-6)
                  .reverse()
                  .map((s) => (
                    <button
                      key={s.product_id}
                      type="button"
                      onClick={() => goTo(list.indexOf(s))}
                      className="flex w-full items-center gap-3 border-t border-slate-100 px-4 py-3 text-left"
                    >
                      <span className="grid size-[22px] shrink-0 place-items-center rounded-full bg-emerald-50 text-emerald-600">
                        <Check className="size-3.5" strokeWidth={3} />
                      </span>
                      <span className="min-w-0 flex-1 truncate text-sm">{s.name}</span>
                      <span className="font-semibold tabular-nums">{counted[s.product_id]}</span>
                    </button>
                  ))}
              </section>
            )}

            <div className="fixed inset-x-0 bottom-0 z-10 mx-auto flex max-w-[560px] flex-col gap-2 border-t border-slate-200 bg-[rgba(243,244,248,.94)] px-4 pt-3 pb-5 backdrop-blur-md">
              <form onSubmit={findByCode} className="flex gap-2">
                <label className="flex h-12 min-w-0 flex-1 items-center gap-2 rounded-xl border border-slate-300 bg-white px-3 focus-within:border-indigo-600 focus-within:ring-4 focus-within:ring-indigo-600/18">
                  <ScanBarcode className="size-5 shrink-0 text-slate-500" aria-hidden />
                  <span className="sr-only">Buscar o escanear otro producto</span>
                  <input
                    value={scan}
                    onChange={(e) => setScan(e.target.value)}
                    placeholder="Escanear u otro producto"
                    className="min-w-0 flex-1 bg-transparent text-sm outline-none"
                  />
                </label>
                <button
                  type="button"
                  onClick={saveAndNext}
                  disabled={qty.trim() === ""}
                  className="h-12 rounded-xl bg-indigo-600 px-5 text-[15px] font-semibold text-white disabled:opacity-45"
                >
                  Guardar y siguiente
                </button>
              </form>
              {scanMsg && <p className="text-xs text-amber-700">{scanMsg}</p>}
              {doneIds.length > 0 && (
                <button type="button" onClick={() => setStep("summary")} className="text-[13px] font-semibold text-indigo-600">
                  Terminar y ver diferencias
                </button>
              )}
            </div>
          </>
        ) : step === "summary" ? (
          <>
            <section className="flex flex-col gap-1.5 rounded-[20px] bg-white p-[18px] shadow-card">
              <span className="text-[13px] text-slate-500">Diferencia en plata (a costo)</span>
              <span
                className={cn(
                  "text-[32px] font-bold tracking-[-0.03em] tabular-nums",
                  moneyDiff < 0 ? "text-rose-700" : moneyDiff > 0 ? "text-emerald-700" : "text-slate-900",
                )}
              >
                {moneyDiff < 0 ? "−" : moneyDiff > 0 ? "+" : ""}
                {formatCOP(Math.abs(moneyDiff))}
              </span>
              <span className="text-[13px] text-slate-500 tabular-nums">
                {diffs.length} contados · {diffs.length - withDiff.length} cuadran · {withDiff.length} con diferencia
              </span>
            </section>
            {withDiff.length > 0 && (
              <section className="overflow-hidden rounded-[20px] bg-white shadow-card">
                <h2 className="px-4 pt-3.5 pb-1.5 text-[15px] font-semibold">Diferencias</h2>
                {withDiff.map((d) => (
                  <button
                    key={d.s.product_id}
                    type="button"
                    onClick={() => {
                      setStep("count");
                      goTo(list.indexOf(d.s));
                    }}
                    className="flex w-full items-center gap-3 border-t border-slate-100 px-4 py-3 text-left"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-semibold">{d.s.name}</span>
                      <span className="text-xs text-slate-500 tabular-nums">
                        Sistema {d.system} · contado {d.c}
                      </span>
                    </span>
                    <span className="flex flex-col items-end">
                      <span
                        className={cn(
                          "inline-flex h-6 items-center rounded-full px-2.5 text-xs font-semibold tabular-nums",
                          d.delta < 0 ? "bg-rose-50 text-rose-700" : "bg-emerald-50 text-emerald-700",
                        )}
                      >
                        {d.delta > 0 ? "+" : ""}
                        {d.delta} und
                      </span>
                      <span className="mt-1 text-xs text-slate-500 tabular-nums">
                        {d.value < 0 ? "−" : "+"}
                        {formatCOP(Math.abs(d.value))}
                      </span>
                    </span>
                  </button>
                ))}
              </section>
            )}
            <NexiNote expr={withDiff.length ? "think" : "happy"}>
              {withDiff.length
                ? "Antes de ajustar, volvé a contar los que más faltan: casi siempre aparecen en la bodega de atrás o en otra nevera."
                : "Todo cuadra. No hay nada que ajustar."}
            </NexiNote>
            {sendError && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-700">{sendError}</p>}
            <div className="fixed inset-x-0 bottom-0 z-10 mx-auto flex max-w-[560px] flex-col gap-2 border-t border-slate-200 bg-[rgba(243,244,248,.94)] px-4 pt-3 pb-5 backdrop-blur-md">
              <button
                type="button"
                onClick={withDiff.length ? send : () => setStep("done")}
                disabled={sending}
                className="flex h-12 items-center justify-center gap-2 rounded-xl bg-indigo-600 text-[15px] font-semibold text-white disabled:opacity-50"
              >
                {sending && <Loader2 className="size-4 animate-spin" />}
                {withDiff.length ? `Ajustar ${withDiff.length} productos` : "Terminar"}
              </button>
              <button type="button" onClick={() => setStep("count")} className="h-10 text-[13px] font-semibold text-slate-600">
                Seguir contando
              </button>
            </div>
          </>
        ) : (
          <section className="flex flex-col items-center gap-3 rounded-[20px] bg-white p-6 text-center shadow-card">
            <Nexi expr="love" size={88} />
            <h1 className="text-xl font-semibold">Conteo guardado</h1>
            <p className="text-sm text-slate-500">
              {result && result.adjusted > 0
                ? `Se ajustaron ${result.adjusted} productos. Quedó en el kardex con el motivo "Conteo físico".`
                : "No hubo diferencias."}
            </p>
            <div className="mt-2 flex w-full gap-2">
              <button
                type="button"
                onClick={() => {
                  setStep("pick");
                  setResult(null);
                }}
                className="h-12 flex-1 rounded-xl border border-slate-300 text-[15px] font-semibold"
              >
                Contar otra parte
              </button>
              <Link href="/inventario" className="grid h-12 flex-1 place-items-center rounded-xl bg-indigo-600 text-[15px] font-semibold text-white">
                Ir a inventario
              </Link>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function NexiNote({ expr, children }: { expr: "happy" | "think"; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3 rounded-2xl bg-linear-to-r from-[#F3EFFF] to-[#EEF0FF] px-3.5 py-3">
      <Nexi expr={expr} size={40} label="" />
      <p className="text-[13px] leading-[1.45] text-slate-700">
        <strong className="text-grad-violet">Nexi · </strong>
        {children}
      </p>
    </div>
  );
}

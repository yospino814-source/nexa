"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { ArrowDownRight, ArrowLeft, ArrowUpRight, Search, X } from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { api, type Movement, type MovementType } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch } from "@/lib/stores/session";
import { formatDateTime, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

const TYPES: Record<MovementType, { label: string; className: string }> = {
  REPOSICION: { label: "Reposición", className: "border-emerald-200 bg-emerald-50 text-emerald-700" },
  DESCUENTO: { label: "Venta", className: "border-slate-200 bg-slate-50 text-slate-700" },
  AJUSTE_ENTRADA: { label: "Ajuste +", className: "border-sky-200 bg-sky-50 text-sky-700" },
  AJUSTE_SALIDA: { label: "Ajuste −", className: "border-amber-200 bg-amber-50 text-amber-700" },
  TRASLADO_ENTRADA: { label: "Traslado entra", className: "border-violet-200 bg-violet-50 text-violet-700" },
  TRASLADO_SALIDA: { label: "Traslado sale", className: "border-violet-200 bg-violet-50 text-violet-700" },
  DEVOLUCION: { label: "Devolución", className: "border-rose-200 bg-rose-50 text-rose-700" },
  AJUSTE: { label: "Ajuste", className: "border-slate-200 bg-slate-50 text-slate-600" },
};

const GROUPS = [
  { id: "", label: "Todos" },
  { id: "VENTAS", label: "Ventas", types: ["DESCUENTO"] },
  { id: "ENTRADAS", label: "Entradas", types: ["REPOSICION", "AJUSTE_ENTRADA", "TRASLADO_ENTRADA", "DEVOLUCION"] },
  { id: "AJUSTES", label: "Ajustes", types: ["AJUSTE_ENTRADA", "AJUSTE_SALIDA", "AJUSTE"] },
  { id: "TRASLADOS", label: "Traslados", types: ["TRASLADO_ENTRADA", "TRASLADO_SALIDA"] },
] as const;

/** Pantalla 16: kardex con trazabilidad de cada unidad (quien, cuando, por que). */
export function KardexScreen() {
  const branch = useCurrentBranch();
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const productId = params.get("producto");
  const { data, loading, error, reload } = useApi(
    () => api.inventory.movements(branch.id, productId, 300),
    `${branch.id}:${productId ?? ""}`,
  );
  const [group, setGroup] = useState<(typeof GROUPS)[number]["id"]>("");
  const [query, setQuery] = useState("");

  const rows = useMemo(() => {
    const types = GROUPS.find((g) => g.id === group);
    const q = query.trim().toLowerCase();
    return (data ?? []).filter(
      (m) =>
        (!types || !("types" in types) || (types.types as readonly string[]).includes(m.movement_type)) &&
        (!q || [m.product_name, m.sku, m.cause, m.user_name].some((v) => v?.toLowerCase().includes(q))),
    );
  }, [data, group, query]);

  if (!["DUENO", "ADMIN_CAJA"].includes(branch.role)) {
    return <p className="m-10 rounded-xl bg-amber-50 p-4 text-sm text-amber-800">Solo el dueño o el administrador pueden ver el kardex.</p>;
  }

  const sum = (pred: (m: Movement) => boolean) => (data ?? []).filter(pred).reduce((s, m) => s + Math.abs(Number(m.delta ?? 0)), 0);
  const inQty = sum((m) => Number(m.delta) > 0);
  const outQty = sum((m) => Number(m.delta) < 0);
  const productName = productId ? data?.[0]?.product_name : null;

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <Link href="/inventario" className="inline-flex items-center gap-1.5 text-sm font-medium text-slate-500 hover:text-slate-800">
        <ArrowLeft className="size-4" /> Existencias
      </Link>
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Kardex y trazabilidad</h1>
        <p className="mt-1 text-slate-500">Cada entrada y salida de {branch.name}, con quién la hizo y por qué.</p>
      </div>

      {productId && (
        <p className="inline-flex items-center gap-2 rounded-full border border-brand/30 bg-brand/10 px-3 py-1.5 text-sm font-medium text-brand">
          Producto: {productName ?? "…"}
          <button aria-label="Quitar filtro de producto" onClick={() => router.replace(pathname)}>
            <X className="size-4" />
          </button>
        </p>
      )}

      <div className="grid gap-6 md:grid-cols-3">
        <StatCard title="Entradas" value={`${inQty ? "+" : ""}${formatQty(inQty)} und`} caption="Reposiciones, ajustes y traslados recibidos" icon={ArrowUpRight} tone="success" />
        <StatCard title="Salidas" value={`${outQty ? "−" : ""}${formatQty(outQty)} und`} caption="Ventas, mermas y traslados enviados" icon={ArrowDownRight} tone="danger" />
        <StatCard title="Movimientos" value={String(data?.length ?? 0)} caption="Últimos 300 registros" icon={Search} />
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm">
        {GROUPS.map((g) => (
          <button
            key={g.id}
            onClick={() => setGroup(g.id)}
            className={cn(
              "rounded-full px-3 py-1.5 text-sm",
              group === g.id ? "bg-brand font-medium text-white" : "bg-slate-100 text-slate-600 hover:bg-slate-200",
            )}
          >
            {g.label}
          </button>
        ))}
        <label className="ml-auto flex h-10 min-w-56 items-center gap-2 rounded-full border border-slate-200 px-3 text-sm">
          <Search className="size-4 text-slate-400" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Producto, motivo o usuario…" className="w-full bg-transparent outline-none" />
        </label>
      </div>

      {error ? (
        <p className="rounded-xl bg-rose-50 p-4 text-sm text-rose-600">
          No se pudo cargar el kardex.{" "}
          <button className="font-semibold underline" onClick={reload}>
            Reintentar
          </button>
        </p>
      ) : (
        <div className="overflow-x-auto rounded-2xl border border-slate-200/60 bg-white shadow-sm">
          <table className="w-full text-sm">
            <thead className="border-b border-slate-100 text-left text-xs tracking-wider text-slate-500 uppercase">
              <tr>
                <th className="px-6 py-4 font-semibold">Fecha</th>
                <th className="px-4 py-4 font-semibold">Producto</th>
                <th className="px-4 py-4 font-semibold">Tipo</th>
                <th className="px-4 py-4 text-right font-semibold">Cantidad</th>
                <th className="px-4 py-4 text-right font-semibold">Saldo</th>
                <th className="px-4 py-4 font-semibold">Motivo</th>
                <th className="px-6 py-4 font-semibold">Usuario</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading && !data && (
                <tr>
                  <td colSpan={7} className="py-10 text-center text-slate-400">
                    Cargando…
                  </td>
                </tr>
              )}
              {rows.map((m) => {
                const t = TYPES[m.movement_type] ?? TYPES.AJUSTE;
                const delta = m.delta == null ? null : Number(m.delta);
                return (
                  <tr key={m.id}>
                    <td className="px-6 py-4 whitespace-nowrap text-slate-500 tabular-nums">{formatDateTime(m.created_at)}</td>
                    <td className="px-4 py-4">
                      <Link href={`/inventario/movimientos?producto=${m.product_id}`} className="font-semibold hover:underline">
                        {m.product_name}
                      </Link>
                      <p className="font-mono text-xs text-slate-400">{m.sku ?? "—"}</p>
                    </td>
                    <td className="px-4 py-4">
                      <span className={cn("rounded-full border px-2.5 py-1 text-xs font-semibold whitespace-nowrap", t.className)}>{t.label}</span>
                    </td>
                    <td
                      className={cn(
                        "px-4 py-4 text-right font-bold tabular-nums",
                        delta != null && delta > 0 && "text-emerald-600",
                        delta != null && delta < 0 && "text-rose-600",
                      )}
                    >
                      {delta == null ? formatQty(m.qty) : `${delta > 0 ? "+" : "−"}${formatQty(Math.abs(delta))}`}
                    </td>
                    <td className="px-4 py-4 text-right font-medium tabular-nums">{m.balance_after == null ? "—" : formatQty(m.balance_after)}</td>
                    <td className="max-w-64 px-4 py-4 text-slate-600">{m.cause ?? "—"}</td>
                    <td className="px-6 py-4 text-slate-500">{m.user_name ?? "—"}</td>
                  </tr>
                );
              })}
              {data && rows.length === 0 && (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-slate-400">
                    Sin movimientos para este filtro.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

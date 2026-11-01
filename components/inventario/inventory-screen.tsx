"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  AlertTriangle,
  ArrowLeftRight,
  CheckCircle2,
  ClipboardList,
  Download,
  History,
  PackagePlus,
  Pencil,
  Search,
  Tag,
  XCircle,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
import { api, type StockLevel, type StockStatus } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

import { StockDialog, type StockMode } from "./stock-dialog";
import { TransferDialog } from "./transfer-dialog";
import { TierPricesDialog } from "./tier-prices-dialog";

const ADMIN_ROLES = ["DUENO", "ADMIN_CAJA"];

const STATUS: Record<StockStatus, { label: string; className: string; dot: string }> = {
  OK: { label: "OK", className: "border-emerald-200 bg-emerald-50 text-emerald-700", dot: "bg-emerald-500" },
  BAJO: { label: "Stock bajo", className: "border-amber-200 bg-amber-50 text-amber-700", dot: "bg-amber-500" },
  AGOTADO: { label: "Agotado", className: "border-rose-200 bg-rose-50 text-rose-600", dot: "bg-rose-500" },
};

/** Pantalla 05: existencias, alertas de reorden, valorizacion y ajustes. */
export function InventoryScreen() {
  const branch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const { data, loading, error, reload } = useApi(() => api.inventory.stock(branch.id), branch.id);

  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");
  const [status, setStatus] = useState<StockStatus | "">("");
  const [editing, setEditing] = useState<{ item: StockLevel; mode: StockMode } | null>(null);
  const [moving, setMoving] = useState<StockLevel | null>(null);
  const [tierPricingItem, setTierPricingItem] = useState<StockLevel | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const items = useMemo(() => data ?? [], [data]);
  const categories = useMemo(
    () => [...new Set(items.map((i) => i.category).filter((c): c is string => !!c))].sort(),
    [items],
  );
  const rows = items.filter((i) => {
    const q = query.trim().toLowerCase();
    const matches = !q || [i.name, i.sku, i.barcode].some((v) => v?.toLowerCase().includes(q));
    return matches && (!category || i.category === category) && (!status || i.status === status);
  });

  if (!ADMIN_ROLES.includes(branch.role)) {
    return <Notice text="Solo el dueño o el administrador pueden ver el inventario de esta sucursal." />;
  }
  if (error) return <Notice text="No se pudo cargar el inventario." onRetry={reload} />;

  const valued = items.reduce((s, i) => s + Number(i.on_hand) * Number(i.cost_price ?? 0), 0);
  const low = items.filter((i) => i.status === "BAJO");
  const out = items.filter((i) => i.status === "AGOTADO");
  const destinations = branches.filter((b) => b.id !== branch.id && ADMIN_ROLES.includes(b.role));

  function saved(message: string) {
    setToast(message);
    reload();
    setTimeout(() => setToast(null), 4000);
  }

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Existencias e inventario</h1>
          <p className="mt-1 text-slate-500">Disponibilidad, alertas de reorden y valor del inventario en {branch.name}.</p>
        </div>
        <Link
          href="/inventario/movimientos"
          className="flex h-11 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium hover:bg-slate-50"
        >
          <History className="size-4" /> Kardex de movimientos
        </Link>
      </div>

      {loading && !data ? (
        <div className="grid gap-6 md:grid-cols-3">
          {[0, 1, 2].map((i) => (
            <div key={i} className="h-40 animate-pulse rounded-2xl bg-slate-200/60" />
          ))}
        </div>
      ) : (
        <div className="grid gap-6 md:grid-cols-3">
          <StatCard
            title="Productos activos"
            value={`${items.length} productos`}
            caption={`Valorizado a costo en ${formatCOP(valued)}`}
            icon={ClipboardList}
          />
          <StatCard
            title="Con stock bajo"
            value={`${low.length} en alerta`}
            caption={low.length ? "Requieren pedido al proveedor" : "Todo por encima del mínimo"}
            icon={AlertTriangle}
            tone="warning"
            badge={low.length ? { label: "Revisión", tone: "warning" } : undefined}
          />
          <StatCard
            title="Agotados"
            value={`${out.length} sin stock`}
            caption={out.length ? "No se pueden vender hasta reponerlos" : "Nada agotado"}
            icon={XCircle}
            tone="danger"
            badge={out.length ? { label: "Crítico", tone: "down" } : undefined}
          />
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm">
        <label className="flex h-11 min-w-64 flex-1 items-center gap-2 rounded-full border border-slate-200 px-4">
          <Search className="size-4 text-slate-400" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar por nombre, SKU o código de barras…"
            className="w-full bg-transparent text-sm outline-none"
          />
        </label>
        <Select label="Categoría" value={category} onChange={setCategory} options={categories.map((c) => [c, c])} />
        <Select
          label="Estado"
          value={status}
          onChange={(v) => setStatus(v as StockStatus | "")}
          options={[
            ["OK", "OK"],
            ["BAJO", "Stock bajo"],
            ["AGOTADO", "Agotado"],
          ]}
        />
        <button
          onClick={() => exportCsv(rows, branch.name)}
          disabled={!rows.length}
          className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50 disabled:opacity-50"
        >
          <Download className="size-4" /> Exportar Excel
        </button>
      </div>

      {toast && (
        <p className="flex items-center gap-2 rounded-2xl border border-emerald-200 bg-emerald-50 p-3 text-sm font-medium text-emerald-700">
          <CheckCircle2 className="size-4" /> {toast}
        </p>
      )}

      <div className="overflow-x-auto rounded-2xl border border-slate-200/60 bg-white shadow-sm">
        <table className="w-full text-sm">
          <thead className="border-b border-slate-100 text-left text-xs tracking-wider text-slate-500 uppercase">
            <tr>
              <th className="px-6 py-4 font-semibold">Producto</th>
              <th className="px-4 py-4 font-semibold">Categoría</th>
              <th className="px-4 py-4 text-right font-semibold">Precio venta</th>
              <th className="px-4 py-4 text-right font-semibold">Costo unit.</th>
              <th className="px-4 py-4 text-right font-semibold">Disponible</th>
              <th className="px-4 py-4 text-right font-semibold">Mínimo</th>
              <th className="px-4 py-4 text-center font-semibold">Estado</th>
              <th className="px-6 py-4 text-right font-semibold">Acciones</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {rows.map((i) => {
              const st = STATUS[i.status];
              return (
                <tr key={i.product_id} className={cn(i.status === "AGOTADO" && "bg-rose-50/30")}>
                  <td className="px-6 py-4">
                    <p className="font-semibold">{i.name}</p>
                    <p className="font-mono text-xs text-slate-400">{i.sku ?? i.barcode ?? "—"}</p>
                  </td>
                  <td className="px-4 py-4">
                    {i.category ? <span className="rounded-full bg-slate-100 px-2.5 py-1 text-xs text-slate-600">{i.category}</span> : "—"}
                  </td>
                  <td className="px-4 py-4 text-right font-semibold tabular-nums">{i.price ? formatCOP(Number(i.price)) : "—"}</td>
                  <td className="px-4 py-4 text-right text-slate-500 tabular-nums">{i.cost_price ? formatCOP(Number(i.cost_price)) : "—"}</td>
                  <td
                    className={cn(
                      "px-4 py-4 text-right text-base font-bold tabular-nums",
                      i.status === "AGOTADO" && "text-rose-600",
                      i.status === "BAJO" && "text-amber-600",
                    )}
                  >
                    {formatQty(i.available)}
                    {Number(i.reserved) > 0 && <span className="block text-xs font-normal text-slate-400">{formatQty(i.reserved)} reservadas</span>}
                  </td>
                  <td className="px-4 py-4 text-right">
                    <button
                      onClick={() => setEditing({ item: i, mode: "MINIMO" })}
                      className="rounded-lg px-2 py-1 text-slate-500 tabular-nums hover:bg-slate-100"
                      title="Cambiar punto de reorden"
                    >
                      {formatQty(i.min_qty)}
                    </button>
                  </td>
                  <td className="px-4 py-4 text-center">
                    <span className={cn("inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold", st.className)}>
                      <span className={cn("size-1.5 rounded-full", st.dot)} /> {st.label}
                    </span>
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center justify-end gap-1">
                      <IconButton label="Ajustar existencias" onClick={() => setEditing({ item: i, mode: "CONTEO" })}>
                        <Pencil className="size-4" />
                      </IconButton>
                      <IconButton label="Precios por tarifa y presentación" onClick={() => setTierPricingItem(i)}>
                        <Tag className="size-4" />
                      </IconButton>
                      <IconButton
                        label="Entrada de mercancía"
                        onClick={() => setEditing({ item: i, mode: "ENTRADA" })}
                        className={i.status !== "OK" ? "text-rose-600" : undefined}
                      >
                        <PackagePlus className="size-4" />
                      </IconButton>
                      <IconButton label="Trasladar a otra sucursal" onClick={() => setMoving(i)} disabled={Number(i.available) <= 0}>
                        <ArrowLeftRight className="size-4" />
                      </IconButton>
                      <Link
                        href={`/inventario/movimientos?producto=${i.product_id}`}
                        aria-label="Ver kardex"
                        title="Ver kardex"
                        className="flex size-9 items-center justify-center rounded-lg text-slate-500 hover:bg-slate-100"
                      >
                        <History className="size-4" />
                      </Link>
                    </div>
                  </td>
                </tr>
              );
            })}
            {data && rows.length === 0 && (
              <tr>
                <td colSpan={8} className="py-12 text-center text-slate-400">
                  {items.length ? "Ningún producto coincide con los filtros." : "Esta sucursal todavía no tiene productos con inventario."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
        {data && (
          <p className="border-t border-slate-100 px-6 py-4 text-sm text-slate-500">
            Mostrando <b>{rows.length}</b> de <b>{items.length}</b> productos
          </p>
        )}
      </div>

      <StockDialog
        item={editing?.item ?? null}
        initialMode={editing?.mode ?? "CONTEO"}
        branchId={branch.id}
        onClose={() => setEditing(null)}
        onSaved={saved}
      />
      <TransferDialog item={moving} from={branch} destinations={destinations} onClose={() => setMoving(null)} onSaved={saved} />
      <TierPricesDialog item={tierPricingItem} onClose={() => setTierPricingItem(null)} onSaved={saved} />
    </div>
  );
}

function Select({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: [string, string][];
}) {
  return (
    <label className="flex h-11 items-center gap-1 rounded-full border border-slate-200 px-4 text-sm">
      <span className="text-slate-400">{label}:</span>
      <select value={value} onChange={(e) => onChange(e.target.value)} className="bg-transparent font-medium outline-none">
        <option value="">Todos</option>
        {options.map(([v, l]) => (
          <option key={v} value={v}>
            {l}
          </option>
        ))}
      </select>
    </label>
  );
}

function IconButton({
  label,
  onClick,
  disabled,
  className,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      disabled={disabled}
      className={cn("flex size-9 items-center justify-center rounded-lg text-slate-500 hover:bg-slate-100 disabled:opacity-30", className)}
    >
      {children}
    </button>
  );
}

function Notice({ text, onRetry }: { text: string; onRetry?: () => void }) {
  return (
    <div className="p-10">
      <p className="rounded-xl bg-amber-50 p-4 text-sm text-amber-800">
        {text}{" "}
        {onRetry && (
          <button className="font-semibold underline" onClick={onRetry}>
            Reintentar
          </button>
        )}
      </p>
    </div>
  );
}

/** CSV con BOM y punto y coma: Excel en español lo abre con columnas y tildes bien. */
function exportCsv(rows: StockLevel[], branchName: string) {
  const header = ["Producto", "SKU", "Código de barras", "Categoría", "Precio venta", "Costo", "Existencia", "Reservado", "Disponible", "Mínimo", "Estado", "Valor a costo"];
  const esc = (v: string | number | null) => `"${String(v ?? "").replace(/"/g, '""')}"`;
  const num = (v: string | number | null) => String(Number(v ?? 0)).replace(".", ",");
  const lines = rows.map((r) =>
    [
      esc(r.name),
      esc(r.sku),
      esc(r.barcode),
      esc(r.category),
      num(r.price),
      num(r.cost_price),
      num(r.on_hand),
      num(r.reserved),
      num(r.available),
      num(r.min_qty),
      esc(STATUS[r.status].label),
      num(Number(r.on_hand) * Number(r.cost_price ?? 0)),
    ].join(";"),
  );
  const blob = new Blob(["﻿" + [header.map(esc).join(";"), ...lines].join("\r\n")], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `inventario-${branchName.toLowerCase().replace(/\s+/g, "-")}-${new Date().toISOString().slice(0, 10)}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

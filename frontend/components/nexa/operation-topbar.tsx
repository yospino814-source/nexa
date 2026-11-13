"use client";

import Link from "next/link";
import { LayoutDashboard, Store } from "lucide-react";

import { BranchSwitcher, UserMenu } from "@/components/auth/user-menu";
import { BrandMark } from "@/components/nexa/brand";
import { api } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCashVersion } from "@/lib/stores/cash";
import { useCurrentBranch, useSession } from "@/lib/stores/session";

/** Barra superior del layout de operacion (56px, sin sidebar). */
export function OperationTopbar() {
  const branch = useCurrentBranch();
  const { can } = useSession();
  const cashVersion = useCashVersion();
  const { data: cash, loading } = useApi(() => api.cash.current(branch.id), `${branch.id}:${cashVersion}`);
  const canManageCash = ["DUENO", "ADMIN_CAJA", "CAJERO"].includes(branch.role);

  return (
    <header className="z-30 flex h-14 shrink-0 items-center justify-between border-b border-slate-200/80 bg-white px-4">
      <div className="flex items-center gap-3">
        <BrandMark withName />
        <div className="hidden h-4 w-px bg-slate-200 sm:block" />
        <div className="hidden items-center gap-1.5 rounded-lg border border-slate-200/60 bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-700 sm:flex">
          <Store className="size-3.5 text-slate-500" />
          Mostrador
        </div>
      </div>

      <div className="flex items-center gap-2">
        <div className="hidden md:flex">
          <BranchSwitcher />
        </div>
        {canManageCash && !loading && (
          <Link
            href="/caja"
            className={
              cash
                ? "flex items-center gap-2 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-xs font-medium text-emerald-700 md:px-3 md:py-1.5 md:text-sm"
                : "flex items-center gap-2 rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700 md:px-3 md:py-1.5 md:text-sm"
            }
          >
            <span className={`size-2 shrink-0 rounded-full ${cash ? "bg-emerald-500" : "bg-amber-500"}`} />
            <span className="md:hidden">{cash ? "Caja abierta" : "Caja cerrada"}</span>
            <span className="hidden md:inline">
              {cash
                ? `Caja abierta desde ${new Date(cash.session.opened_at).toLocaleTimeString("es-CO", { hour: "2-digit", minute: "2-digit" })}`
                : "Caja cerrada · abrir turno"}
            </span>
          </Link>
        )}
      </div>

      <div className="flex items-center gap-3">
        <UserMenu />
        {can("reports:read") ? (
          <Link
            href="/panel"
            className="flex items-center gap-2 rounded-full bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800"
          >
            <LayoutDashboard className="size-4" />
            <span className="hidden sm:inline">Panel</span>
          </Link>
        ) : can("cash:history") ? (
          <Link
            href="/ordenes"
            className="flex items-center gap-2 rounded-full bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800"
          >
            <LayoutDashboard className="size-4" />
            <span className="hidden sm:inline">Ventas</span>
          </Link>
        ) : null}
      </div>
    </header>
  );
}

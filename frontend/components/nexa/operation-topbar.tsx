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
    <header className="z-30 flex h-14 shrink-0 items-center gap-3.5 border-b border-slate-200 bg-white px-4">
      <div className="flex items-center gap-3.5">
        <Link href={can("reports:read") ? "/panel" : "/pos"} className="flex items-center gap-2 text-carbon" aria-label="Nexa POS">
          <BrandMark size={28} />
          <span aria-hidden className="text-[17px] font-extrabold tracking-[0.06em]">
            NE
            <span className="bg-linear-to-br from-grad-pink to-grad-cyan bg-clip-text text-transparent">X</span>A
          </span>
        </Link>
        <span aria-hidden className="hidden h-6 w-px bg-slate-200 sm:block" />
        <span className="hidden h-8 items-center gap-1.5 rounded-full bg-slate-100 px-3 text-[13px] font-semibold text-slate-700 sm:flex">
          <Store className="size-[15px]" aria-hidden />
          Mostrador
        </span>
      </div>

      <div className="flex flex-1 items-center justify-center gap-2.5">
        <div className="hidden md:flex">
          <BranchSwitcher />
        </div>
        {canManageCash && !loading && (
          <Link
            href="/caja"
            className={
              cash
                ? "flex h-8 items-center gap-2 rounded-full bg-emerald-50 px-3 text-xs font-semibold text-emerald-700 ring-1 ring-emerald-200 ring-inset md:text-[13px]"
                : "flex h-8 items-center gap-2 rounded-full bg-amber-50 px-3 text-xs font-semibold text-amber-700 ring-1 ring-amber-200 ring-inset md:text-[13px]"
            }
          >
            <span className={`size-[7px] shrink-0 rounded-full ${cash ? "bg-emerald-500" : "bg-amber-500"}`} />
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
            className="flex h-9 items-center gap-2 rounded-full bg-carbon px-3.5 text-[13px] font-semibold text-white hover:bg-carbon-soft"
          >
            <LayoutDashboard className="size-4" />
            <span className="hidden sm:inline">Panel</span>
          </Link>
        ) : can("cash:history") ? (
          <Link
            href="/ordenes"
            className="flex h-9 items-center gap-2 rounded-full bg-carbon px-3.5 text-[13px] font-semibold text-white hover:bg-carbon-soft"
          >
            <LayoutDashboard className="size-4" />
            <span className="hidden sm:inline">Ventas</span>
          </Link>
        ) : null}
      </div>
    </header>
  );
}

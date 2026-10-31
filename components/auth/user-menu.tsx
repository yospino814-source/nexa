"use client";

import { useRouter } from "next/navigation";
import { Check, ChevronDown, LogOut, Store } from "lucide-react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { AUTH_ENABLED } from "@/lib/supabase/config";
import { initials, ROLE_LABELS, useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

/** Selector de sucursal (solo si el usuario trabaja en mas de una). */
export function BranchSwitcher({ className }: { className?: string }) {
  const { branches, selectBranch } = useSession();
  const current = useCurrentBranch();

  const pill = (
    <>
      <Store className="size-4 text-slate-500" />
      {current.name}
      {branches.length > 1 && <ChevronDown className="size-4 text-slate-400" />}
    </>
  );
  const pillClass = cn(
    "flex items-center gap-2 rounded-full border border-slate-200 px-3 py-1.5 text-sm font-medium text-slate-700",
    branches.length > 1 && "hover:bg-slate-50",
    className,
  );

  if (branches.length <= 1) return <span className={pillClass}>{pill}</span>;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger className={pillClass}>{pill}</DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Cambiar de sucursal</DropdownMenuLabel>
          {branches.map((b) => (
            <DropdownMenuItem key={b.id} onClick={() => selectBranch(b.id)}>
              <span className="flex-1">{b.name}</span>
              <span className="text-xs text-slate-400">{ROLE_LABELS[b.role] ?? b.role}</span>
              {b.id === current.id && <Check className="size-4 text-brand" />}
            </DropdownMenuItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Avatar con nombre y rol; abre el menu con "Cerrar sesion". */
export function UserMenu({ dark = false }: { dark?: boolean }) {
  const router = useRouter();
  const { name, email, signOut } = useSession();
  const branch = useCurrentBranch();

  async function logout() {
    await signOut();
    router.replace("/login");
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="flex items-center gap-2 rounded-full text-left outline-none focus-visible:ring-2 focus-visible:ring-brand">
        <span
          className={cn(
            "flex size-9 items-center justify-center rounded-full text-xs font-semibold",
            dark ? "bg-slate-900 text-white" : "bg-teal-50 text-brand ring-1 ring-teal-100",
          )}
        >
          {initials(name)}
        </span>
        <span className="hidden leading-tight sm:block">
          <span className="block text-sm font-semibold">{name}</span>
          <span className="block text-xs text-brand">{ROLE_LABELS[branch.role] ?? branch.role}</span>
        </span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56">
        <DropdownMenuGroup>
          <DropdownMenuLabel>
            <span className="block font-semibold text-slate-900">{name}</span>
            {email && <span className="block text-xs font-normal text-slate-500">{email}</span>}
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={logout} disabled={!AUTH_ENABLED}>
          <LogOut className="size-4" /> {AUTH_ENABLED ? "Cerrar sesión" : "Cerrar sesión (modo demo)"}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

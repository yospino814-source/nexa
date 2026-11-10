"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Check, ChevronDown, Loader2, LogOut, Mail, Store, X } from "lucide-react";

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
  const { name, email, signOut, invitations, acceptInvitation } = useSession();
  const [invitationsOpen, setInvitationsOpen] = useState(false);
  const [acceptingId, setAcceptingId] = useState<string | null>(null);
  const branch = useCurrentBranch();

  async function logout() {
    await signOut();
    router.replace("/login");
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger className="flex items-center gap-2 rounded-full text-left outline-none focus-visible:ring-2 focus-visible:ring-brand">
          <span
            className={cn(
              "flex size-9 items-center justify-center rounded-full text-xs font-semibold relative",
              dark ? "bg-slate-900 text-white" : "bg-teal-50 text-brand ring-1 ring-teal-100",
            )}
          >
            {initials(name)}
            {invitations.length > 0 && (
              <span className="absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full bg-amber-500 text-[10px] font-bold text-white ring-2 ring-white">
                {invitations.length}
              </span>
            )}
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
          {invitations.length > 0 && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuGroup>
                <DropdownMenuItem onClick={() => setInvitationsOpen(true)}>
                  <Mail className="size-4 text-amber-600" />
                  <span className="flex-1 font-medium">Invitaciones</span>
                  <span className="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-bold text-amber-700">
                    {invitations.length}
                  </span>
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={logout} disabled={!AUTH_ENABLED}>
            <LogOut className="size-4" /> {AUTH_ENABLED ? "Cerrar sesión" : "Cerrar sesión (modo demo)"}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {invitationsOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/50 p-4 backdrop-blur-xs">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 pb-3">
              <h3 className="text-lg font-bold text-slate-900">Invitaciones pendientes ({invitations.length})</h3>
              <button
                onClick={() => setInvitationsOpen(false)}
                className="rounded-full p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="size-5" />
              </button>
            </div>
            <div className="mt-4 space-y-3">
              {invitations.map((inv) => (
                <div
                  key={inv.id}
                  className="flex items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-slate-50/50 p-3.5"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-semibold text-slate-900">{inv.tenant_name}</p>
                    <p className="text-xs text-slate-500">
                      {inv.branch_name ?? "Todas las sucursales"} · <span className="font-medium text-slate-700">{inv.role_name}</span>
                    </p>
                  </div>
                  <button
                    disabled={acceptingId !== null}
                    onClick={async () => {
                      setAcceptingId(inv.id);
                      try {
                        await acceptInvitation(inv.id);
                        setInvitationsOpen(false);
                      } finally {
                        setAcceptingId(null);
                      }
                    }}
                    className="flex h-8 shrink-0 items-center gap-1.5 rounded-full bg-brand px-3.5 text-xs font-semibold text-white shadow-xs hover:bg-brand-dark disabled:opacity-50"
                  >
                    {acceptingId === inv.id && <Loader2 className="size-3.5 animate-spin" />}
                    Aceptar
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </>
  );
}

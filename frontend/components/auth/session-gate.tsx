"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { AlertTriangle, Building2, Loader2, LogOut } from "lucide-react";

import { useSession } from "@/lib/stores/session";

import { AuthShell } from "./auth-shell";
import { BranchPicker } from "./branch-picker";

/**
 * Protege las pantallas privadas: arranca la sesion y solo muestra el
 * contenido cuando hay usuario, negocio y sucursal elegida. proxy.ts ya
 * redirige sin cookie; esto cubre sesiones vencidas y el 401 del backend.
 */
export function SessionGate({ children }: { children: React.ReactNode }) {
  const { status, error, email, branches, branchId, boot, signOut, invitations, acceptInvitation } = useSession();
  const [acceptingId, setAcceptingId] = useState<string | null>(null);
  const router = useRouter();
  const pathname = usePathname();

  useEffect(() => {
    if (status === "loading") boot();
  }, [status, boot]);

  useEffect(() => {
    if (status === "signed-out") router.replace(`/login?next=${encodeURIComponent(pathname)}`);
  }, [status, router, pathname]);

  if (status === "ready") {
    const role = branches.find((b) => b.id === branchId)?.role;
    if (role === "MESERO" || role === "COCINA" || role === "REPARTIDOR") {
      return (
        <AuthShell title="Tu rol todavía no tiene pantallas">
          <div className="space-y-5 text-center">
            <p className="text-slate-600">Pedile al dueño que te asigne otro rol.</p>
            <button
              type="button"
              onClick={signOut}
              className="flex h-12 w-full items-center justify-center gap-2 rounded-2xl border border-slate-200 font-semibold hover:bg-slate-50"
            >
              <LogOut className="size-4" /> Salir
            </button>
          </div>
        </AuthShell>
      );
    }
    return <>{children}</>;
  }

  if (status === "choose-branch") {
    return (
      <AuthShell title="¿Dónde vas a trabajar hoy?" subtitle="Elegí la sucursal para esta sesión">
        <BranchPicker />
      </AuthShell>
    );
  }

  if (status === "no-business") {
    if (invitations.length > 0) {
      return (
        <AuthShell title="Te invitaron a trabajar en Nexa">
          <div className="space-y-4">
            <p className="text-center text-sm text-slate-600">
              Tenés {invitations.length} {invitations.length === 1 ? "invitación pendiente" : "invitaciones pendientes"}. Aceptala para empezar a trabajar.
            </p>
            <div className="space-y-3">
              {invitations.map((inv) => (
                <div
                  key={inv.id}
                  className="flex items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-4 shadow-xs"
                >
                  <div className="min-w-0">
                    <p className="truncate font-semibold text-slate-900">{inv.tenant_name}</p>
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
                      } finally {
                        setAcceptingId(null);
                      }
                    }}
                    className="flex h-9 shrink-0 items-center gap-1.5 rounded-full bg-brand px-4 text-xs font-semibold text-white shadow-xs hover:bg-brand-dark disabled:opacity-50"
                  >
                    {acceptingId === inv.id && <Loader2 className="size-3.5 animate-spin" />}
                    Aceptar
                  </button>
                </div>
              ))}
            </div>

            <div className="pt-2 text-center text-xs text-slate-600">
              ¿Querés crear tu propio negocio?{" "}
              <Link href="/onboarding" className="font-semibold text-brand underline hover:text-brand-dark">
                Creá tu negocio acá
              </Link>
            </div>

            <button
              onClick={signOut}
              className="flex h-11 w-full items-center justify-center gap-2 rounded-2xl border border-slate-200 font-semibold text-sm hover:bg-slate-50"
            >
              <LogOut className="size-4" /> Salir
            </button>
          </div>
        </AuthShell>
      );
    }

    return (
      <AuthShell title="Tu usuario no tiene un negocio asignado">
        <div className="space-y-5 text-center">
          <Building2 className="mx-auto size-12 rounded-2xl bg-slate-100 p-2.5 text-slate-500" />
          <p className="text-slate-600">
            Entraste como <strong>{email}</strong>, pero todavía nadie te dio acceso a un negocio. Pedile al dueño
            que te invite o creá tu negocio.
          </p>
          <button
            onClick={() => router.push("/onboarding")}
            className="flex h-12 w-full items-center justify-center rounded-2xl bg-brand font-semibold text-white hover:bg-brand-dark"
          >
            Crear mi negocio
          </button>
          <button
            onClick={signOut}
            className="flex h-12 w-full items-center justify-center gap-2 rounded-2xl border border-slate-200 font-semibold hover:bg-slate-50"
          >
            <LogOut className="size-4" /> Salir
          </button>
        </div>
      </AuthShell>
    );
  }

  if (status === "error") {
    return (
      <AuthShell title="No pudimos iniciar">
        <div className="space-y-5 text-center">
          <AlertTriangle className="mx-auto size-12 rounded-2xl bg-amber-50 p-2.5 text-amber-600" />
          <p className="text-slate-600">{error}</p>
          <button
            onClick={boot}
            className="flex h-12 w-full items-center justify-center rounded-2xl bg-brand font-semibold text-white hover:bg-brand-dark"
          >
            Reintentar
          </button>
        </div>
      </AuthShell>
    );
  }

  return (
    <div className="flex min-h-screen flex-1 items-center justify-center bg-slate-50" aria-busy="true">
      <Loader2 className="size-8 animate-spin text-brand" aria-label="Cargando sesión" />
    </div>
  );
}

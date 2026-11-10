"use client";

import { useEffect } from "react";
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
  const { status, error, email, boot, signOut } = useSession();
  const router = useRouter();
  const pathname = usePathname();

  useEffect(() => {
    if (status === "loading") boot();
  }, [status, boot]);

  useEffect(() => {
    if (status === "signed-out") router.replace(`/login?next=${encodeURIComponent(pathname)}`);
  }, [status, router, pathname]);

  if (status === "ready") return <>{children}</>;

  if (status === "choose-branch") {
    return (
      <AuthShell title="¿Dónde vas a trabajar hoy?" subtitle="Elegí la sucursal para esta sesión">
        <BranchPicker />
      </AuthShell>
    );
  }

  if (status === "no-business") {
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

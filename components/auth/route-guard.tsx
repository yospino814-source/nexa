"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { canOpen } from "@/lib/route-permissions";
import { homeFor, useSession } from "@/lib/stores/session";

import { AuthShell, primaryButtonClass } from "./auth-shell";

/**
 * Guardia de ruta por permisos granulares. Debe usarse dentro de `<SessionGate>`.
 */
export function RouteGuard({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { branches, branchId, can } = useSession();
  const role = branches.find((b) => b.id === branchId)?.role ?? null;

  if (!canOpen(pathname, can)) {
    return (
      <AuthShell title="No tenés permiso para ver esta pantalla">
        <div className="space-y-5 text-center">
          <p className="text-slate-600">Pedile al dueño que te dé acceso.</p>
          <Link href={homeFor(role)} className={primaryButtonClass}>
            Ir a mi inicio
          </Link>
        </div>
      </AuthShell>
    );
  }

  return <>{children}</>;
}

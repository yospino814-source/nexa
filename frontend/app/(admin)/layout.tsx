import { Suspense } from "react";

import { RouteGuard } from "@/components/auth/route-guard";
import { SessionGate } from "@/components/auth/session-gate";
import { Sidebar } from "@/components/nexa/sidebar";

/** Layout de administración: menú de 84 px + contenido (cada página pone su Topbar). */
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <Suspense fallback={null}>
      <SessionGate>
        <RouteGuard>
          <div className="flex min-h-screen w-full bg-slate-50 text-slate-900">
            <Sidebar />
            <main className="nx-admin min-w-0 flex-1">{children}</main>
          </div>
        </RouteGuard>
      </SessionGate>
    </Suspense>
  );
}

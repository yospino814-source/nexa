import { Suspense } from "react";

import { RouteGuard } from "@/components/auth/route-guard";
import { SessionGate } from "@/components/auth/session-gate";

/** Depende de la sesión del navegador: ver app/(admin)/layout.tsx. */
export const instant = false;

/** Pantallas de bodega para el celular: sin menú ni barra, a pantalla completa. */
export default function MobileLayout({ children }: { children: React.ReactNode }) {
  return (
    <Suspense fallback={null}>
      <SessionGate>
        <RouteGuard>{children}</RouteGuard>
      </SessionGate>
    </Suspense>
  );
}

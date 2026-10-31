import { Suspense } from "react";
import type { Metadata } from "next";

import { KardexScreen } from "@/components/inventario/kardex-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Kardex · Nexa POS" };

export default function MovimientosPage() {
  return (
    <>
      <Topbar title="Kardex de movimientos" />
      {/* useSearchParams (filtro ?producto=) necesita un limite de Suspense. */}
      <Suspense>
        <KardexScreen />
      </Suspense>
    </>
  );
}

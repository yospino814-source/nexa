import type { Metadata } from "next";

import { FiscalScreen } from "@/components/configuracion/fiscal-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Facturación electrónica · Configuración · Nexa POS" };

export default function FiscalPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <FiscalScreen />
    </>
  );
}

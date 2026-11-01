import type { Metadata } from "next";

import { TiersScreen } from "@/components/configuracion/tiers-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Tarifas y presentaciones · Configuración · Nexa POS" };

export default function TarifasPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <TiersScreen />
    </>
  );
}

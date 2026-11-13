import type { Metadata } from "next";

import { RegistersScreen } from "@/components/configuracion/registers-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Cajas registradoras · Configuración · Nexa POS" };

export default function CajasPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <RegistersScreen />
    </>
  );
}


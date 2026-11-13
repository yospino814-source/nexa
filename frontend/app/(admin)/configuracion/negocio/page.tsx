import type { Metadata } from "next";

import { SettingsScreen } from "@/components/configuracion/settings-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Configuración · Negocio y sucursales · Nexa POS" };

export default function NegocioPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <SettingsScreen />
    </>
  );
}


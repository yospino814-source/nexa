import type { Metadata } from "next";

import { UsersScreen } from "@/components/configuracion/users-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Usuarios y roles · Nexa POS" };

export default function UsuariosPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <UsersScreen />
    </>
  );
}


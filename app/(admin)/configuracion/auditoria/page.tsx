import type { Metadata } from "next";

import { AuditScreen } from "@/components/configuracion/audit-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Auditoría · Nexa POS" };

export default function AuditoriaPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <AuditScreen />
    </>
  );
}


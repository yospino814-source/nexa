import type { Metadata } from "next";

import { RefundsScreen } from "@/components/ordenes/refunds-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Devoluciones · Nexa POS" };

export default function DevolucionesPage() {
  return (
    <>
      <Topbar title="Devoluciones" />
      <RefundsScreen />
    </>
  );
}

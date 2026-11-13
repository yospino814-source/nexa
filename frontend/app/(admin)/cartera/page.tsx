import type { Metadata } from "next";

import { ReceivablesScreen } from "@/components/cartera/receivables-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Cartera · Nexa POS" };

export default function CarteraPage() {
  return (
    <>
      <Topbar title="Cartera de Clientes" />
      <ReceivablesScreen />
    </>
  );
}

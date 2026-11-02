import type { Metadata } from "next";

import { PayablesScreen } from "@/components/compras/payables-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Cuentas por pagar · Nexa POS" };

export default function PorPagarPage() {
  return (
    <>
      <Topbar title="Cuentas por pagar" />
      <PayablesScreen />
    </>
  );
}


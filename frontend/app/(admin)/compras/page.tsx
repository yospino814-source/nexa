import type { Metadata } from "next";

import { PurchasesScreen } from "@/components/compras/purchases-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Compras · Nexa POS" };

export default function ComprasPage() {
  return (
    <>
      <Topbar title="Compras" />
      <PurchasesScreen />
    </>
  );
}

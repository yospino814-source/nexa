import type { Metadata } from "next";

import { SalesScreen } from "@/components/ordenes/sales-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Órdenes · Nexa POS" };

export default function OrdenesPage() {
  return (
    <>
      <Topbar title="Órdenes" />
      <SalesScreen />
    </>
  );
}

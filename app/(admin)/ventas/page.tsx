import type { Metadata } from "next";

import { Topbar } from "@/components/nexa/topbar";
import { SalesReportsScreen } from "@/components/ventas/sales-reports-screen";

export const metadata: Metadata = { title: "Resumen de ventas · Nexa POS" };

export default function VentasPage() {
  return (
    <>
      <Topbar title="Resumen de ventas" />
      <SalesReportsScreen />
    </>
  );
}


import type { Metadata } from "next";

import { Topbar } from "@/components/nexa/topbar";
import { ProductReportsScreen } from "@/components/ventas/product-reports-screen";

export const metadata: Metadata = { title: "Ventas por producto · Nexa POS" };

export default function VentasProductosPage() {
  return (
    <>
      <Topbar title="Ventas por producto" />
      <ProductReportsScreen />
    </>
  );
}


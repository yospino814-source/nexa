import type { Metadata } from "next";

import { ProductScreen } from "@/components/inventario/product-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Nuevo producto · Nexa POS" };

export default function NuevoProductoPage() {
  return (
    <>
      <Topbar title="Nuevo producto" />
      <ProductScreen productId="nuevo" />
    </>
  );
}

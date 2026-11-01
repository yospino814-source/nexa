import type { Metadata } from "next";

import { ProductScreen } from "@/components/inventario/product-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Ficha de producto · Nexa POS" };

export default async function ProductoDetallePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return (
    <>
      <Topbar title="Ficha de producto" />
      <ProductScreen productId={id} />
    </>
  );
}

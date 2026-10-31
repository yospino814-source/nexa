import type { Metadata } from "next";

import { InventoryScreen } from "@/components/inventario/inventory-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Inventario · Nexa POS" };

export default function InventarioPage() {
  return (
    <>
      <Topbar title="Inventario" />
      <InventoryScreen />
    </>
  );
}

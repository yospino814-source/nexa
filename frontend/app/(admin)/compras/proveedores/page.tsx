import type { Metadata } from "next";

import { SuppliersScreen } from "@/components/compras/suppliers-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Proveedores · Nexa POS" };

export default function ProveedoresPage() {
  return (
    <>
      <Topbar title="Proveedores" />
      <SuppliersScreen />
    </>
  );
}

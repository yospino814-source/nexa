import type { Metadata } from "next";

import { OrdenesViewWrapper } from "@/components/ordenes/ordenes-view-wrapper";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Órdenes · Nexa POS" };

export default function OrdenesPage() {
  return (
    <>
      <Topbar title="Órdenes" />
      <OrdenesViewWrapper />
    </>
  );
}

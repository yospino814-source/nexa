import type { Metadata } from "next";

import { CountScreen } from "@/components/inventario/count-screen";

export const metadata: Metadata = { title: "Conteo de inventario · Nexa POS" };

export default function ConteoPage() {
  return <CountScreen />;
}

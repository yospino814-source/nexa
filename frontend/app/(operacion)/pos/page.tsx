import type { Metadata } from "next";

import { PosScreen } from "@/components/pos/pos-screen";

export const metadata: Metadata = { title: "POS Mostrador · Nexa POS" };

export default function PosPage() {
  return <PosScreen />;
}

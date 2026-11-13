import type { Metadata } from "next";

import { CajaScreen } from "@/components/caja/caja-screen";

export const metadata: Metadata = { title: "Caja y arqueo · Nexa POS" };

export default function CajaPage() {
  return <CajaScreen />;
}

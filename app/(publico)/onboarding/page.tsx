import { Suspense } from "react";
import type { Metadata } from "next";

import { OnboardingScreen } from "@/components/auth/onboarding-screen";

export const metadata: Metadata = { title: "Configurá tu negocio · Nexa POS" };

/** Pantalla 12: Onboarding de nuevo negocio y sucursal. */
export default function OnboardingPage() {
  return (
    <Suspense>
      <OnboardingScreen />
    </Suspense>
  );
}


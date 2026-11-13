import { Suspense } from "react";
import type { Metadata } from "next";

import { RegisterScreen } from "@/components/auth/register-screen";

export const metadata: Metadata = { title: "Creá tu cuenta · Nexa POS" };

/** Pantalla 09: Registro de usuario inicial. */
export default function RegistroPage() {
  return (
    <Suspense>
      <RegisterScreen />
    </Suspense>
  );
}


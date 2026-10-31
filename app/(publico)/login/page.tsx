import { Suspense } from "react";
import type { Metadata } from "next";

import { LoginScreen } from "@/components/auth/login-screen";

export const metadata: Metadata = { title: "Iniciar sesión · Nexa POS" };

/** Pantalla 11. Suspense: LoginScreen lee `?next=` de la URL. */
export default function LoginPage() {
  return (
    <Suspense>
      <LoginScreen />
    </Suspense>
  );
}

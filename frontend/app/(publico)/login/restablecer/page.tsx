import type { Metadata } from "next";

import { ResetPasswordScreen } from "@/components/auth/reset-password-screen";

export const metadata: Metadata = { title: "Nueva contraseña · Nexa POS" };

export default function ResetPasswordPage() {
  return <ResetPasswordScreen />;
}

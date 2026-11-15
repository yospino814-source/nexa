import type { Metadata } from "next";

import { WhatsappQrScreen } from "@/components/configuracion/whatsapp-qr-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "QR de WhatsApp · Configuración · Nexa POS" };

export default function WhatsappQrPage() {
  return (
    <>
      <Topbar title="Configuración" />
      <WhatsappQrScreen />
    </>
  );
}

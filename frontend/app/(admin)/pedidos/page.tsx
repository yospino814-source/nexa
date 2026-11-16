import type { Metadata } from "next";

import { WhatsappOrdersScreen } from "@/components/pedidos/whatsapp-orders-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Pedidos por WhatsApp · Nexa POS" };

export default function PedidosPage() {
  return (
    <>
      <Topbar title="Pedidos por WhatsApp" />
      <WhatsappOrdersScreen />
    </>
  );
}

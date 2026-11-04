import type { Metadata } from "next";

import { CustomerScreen } from "@/components/clientes/customer-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Ficha de cliente · Nexa POS" };

export default async function ClienteDetallePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return (
    <>
      <Topbar title="Ficha de Cliente" />
      <CustomerScreen customerId={id} />
    </>
  );
}

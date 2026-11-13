import type { Metadata } from "next";

import { CustomersScreen } from "@/components/clientes/customers-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Clientes · Nexa POS" };

export default function ClientesPage() {
  return (
    <>
      <Topbar title="Directorio de Clientes" />
      <CustomersScreen />
    </>
  );
}

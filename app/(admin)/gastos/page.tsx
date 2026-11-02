import type { Metadata } from "next";

import { ExpensesScreen } from "@/components/gastos/expenses-screen";
import { Topbar } from "@/components/nexa/topbar";

export const metadata: Metadata = { title: "Gastos · Nexa POS" };

export default function GastosPage() {
  return (
    <>
      <Topbar title="Gastos Operativos" />
      <ExpensesScreen />
    </>
  );
}

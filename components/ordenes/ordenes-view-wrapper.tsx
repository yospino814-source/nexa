"use client";

import { useState } from "react";
import { LayoutGrid, Table } from "lucide-react";

import { OrdersScreen } from "./orders-screen";
import { SalesScreen } from "./sales-screen";
import { USE_MOCKS } from "@/lib/api";
import { cn } from "@/lib/utils";

export function OrdenesViewWrapper() {
  const [activeView, setActiveView] = useState<"pedidos" | "ventas">("pedidos");

  if (!USE_MOCKS) {
    return <SalesScreen />;
  }

  return (
    <div className="flex flex-1 flex-col">
      {/* Switcher de Vistas */}
      <div className="border-b border-slate-200/80 bg-white px-4 py-2.5 lg:px-8">
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setActiveView("pedidos")}
            className={cn(
              "flex items-center gap-2 rounded-xl px-3.5 py-1.5 text-xs font-semibold transition-all",
              activeView === "pedidos"
                ? "bg-teal-700 text-white shadow-xs"
                : "text-slate-600 hover:bg-slate-100 hover:text-slate-900",
            )}
          >
            <LayoutGrid className="size-3.5" />
            <span>Gestión de Pedidos (Omnicanal)</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveView("ventas")}
            className={cn(
              "flex items-center gap-2 rounded-xl px-3.5 py-1.5 text-xs font-semibold transition-all",
              activeView === "ventas"
                ? "bg-teal-700 text-white shadow-xs"
                : "text-slate-600 hover:bg-slate-100 hover:text-slate-900",
            )}
          >
            <Table className="size-3.5" />
            <span>Historial de Ventas (Caja POS)</span>
          </button>
        </div>
      </div>

      {/* Contenido de la Vista Activa */}
      {activeView === "pedidos" ? <OrdersScreen /> : <SalesScreen />}
    </div>
  );
}


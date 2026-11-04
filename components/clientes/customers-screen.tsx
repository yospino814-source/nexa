"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { AlertTriangle, ChevronRight, Loader2, Plus, Search, Tag, Users } from "lucide-react";

import { CustomerFormDialog } from "./customer-form-dialog";
import { api, friendlyError, type Customer, type CustomerDetail } from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { useSession } from "@/lib/stores/session";

type CustomerTab = "TODOS" | "AL_DIA" | "EN_MORA" | "ARCHIVADO";

export function CustomersScreen() {
  const canWrite = useSession((s) => s.can("customers:write"));

  const [tab, setTab] = useState<CustomerTab>("TODOS");
  const [query, setQuery] = useState("");
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [dialogOpen, setDialogOpen] = useState(false);

  const fetchCustomers = async (statusTab: CustomerTab, qStr: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.customers.list({
        status: statusTab,
        q: qStr.trim() || undefined,
        page_size: 50,
      });
      setCustomers(res);
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const timer = setTimeout(() => {
      fetchCustomers(tab, query);
    }, 250);
    return () => clearTimeout(timer);
  }, [tab, query]);

  const handleCustomerCreated = (newCust: CustomerDetail) => {
    setCustomers((prev) => [newCust, ...prev]);
  };

  return (
    <div className="flex flex-col gap-6 p-6">
      {/* Barra de cabecera con acciones */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-black tracking-tight text-slate-900">Directorio de Clientes</h1>
          <p className="text-xs text-slate-500 mt-0.5">
            Gestión de clientes, cupos de crédito, estados de cuenta y facturación electrónica.
          </p>
        </div>

        {canWrite && (
          <button
            type="button"
            onClick={() => setDialogOpen(true)}
            className="flex items-center justify-center gap-2 rounded-xl bg-teal-700 px-4 py-2.5 text-xs font-semibold text-white shadow-xs hover:bg-teal-800 transition-colors"
          >
            <Plus className="size-4" />
            Nuevo cliente
          </button>
        )}
      </div>

      {/* Filtros: Buscador y Pestañas */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 size-4 text-slate-400" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar por nombre, documento o teléfono..."
            className="w-full rounded-xl border border-slate-200 bg-white pl-10 pr-4 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
          />
        </div>

        {/* Pestañas: Todos, Al día, En mora, Archivados */}
        <div className="flex rounded-xl bg-slate-100 p-1 text-xs font-semibold">
          <button
            type="button"
            onClick={() => setTab("TODOS")}
            className={`rounded-lg px-3.5 py-1.5 transition-all ${
              tab === "TODOS" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
            }`}
          >
            Todos
          </button>
          <button
            type="button"
            onClick={() => setTab("AL_DIA")}
            className={`rounded-lg px-3.5 py-1.5 transition-all ${
              tab === "AL_DIA" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
            }`}
          >
            Al día
          </button>
          <button
            type="button"
            onClick={() => setTab("EN_MORA")}
            className={`rounded-lg px-3.5 py-1.5 transition-all ${
              tab === "EN_MORA" ? "bg-white text-rose-700 shadow-xs" : "text-slate-600 hover:text-slate-900"
            }`}
          >
            En mora
          </button>
          <button
            type="button"
            onClick={() => setTab("ARCHIVADO")}
            className={`rounded-lg px-3.5 py-1.5 transition-all ${
              tab === "ARCHIVADO" ? "bg-white text-slate-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
            }`}
          >
            Archivados
          </button>
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-xl bg-rose-50 p-3 text-xs text-rose-700">
          <AlertTriangle className="size-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Tabla de Clientes */}
      <div className="rounded-2xl border border-slate-200 bg-white overflow-hidden shadow-xs">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b border-slate-100 bg-slate-50/75 text-slate-500 font-semibold">
                <th className="py-3 px-4">Nombre / Razón Social</th>
                <th className="py-3 px-4">Documento</th>
                <th className="py-3 px-4">Teléfono</th>
                <th className="py-3 px-4 text-right">Saldo Cartera</th>
                <th className="py-3 px-4 text-center">Días de Mora</th>
                <th className="py-3 px-4">Tarifa Asignada</th>
                <th className="py-3 px-4 text-center">Acción</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {loading ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-slate-400">
                    <Loader2 className="size-6 animate-spin mx-auto mb-2 text-teal-600" />
                    Cargando directorio de clientes...
                  </td>
                </tr>
              ) : customers.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-slate-400">
                    <Users className="size-8 mx-auto mb-2 opacity-30" />
                    No se encontraron clientes para este filtro.
                  </td>
                </tr>
              ) : (
                customers.map((c) => {
                  const balanceNum = Number(c.balance ?? 0);
                  const daysOverdue = c.max_days_overdue ?? 0;
                  const isOverdue = daysOverdue > 0 && balanceNum > 0;

                  return (
                    <tr
                      key={c.id}
                      className="hover:bg-slate-50/75 transition-colors group cursor-pointer"
                    >
                      <td className="py-3 px-4">
                        <Link href={`/clientes/${c.id}`} className="block">
                          <span className="font-bold text-slate-900 group-hover:text-teal-700 transition-colors">
                            {c.name}
                          </span>
                          {c.email && (
                            <span className="block text-[11px] text-slate-400">{c.email}</span>
                          )}
                        </Link>
                      </td>

                      <td className="py-3 px-4 text-slate-700 font-medium">
                        {c.doc_type && c.doc_number ? (
                          <span>
                            {c.doc_type} {c.doc_number}
                            {c.dv ? `-${c.dv}` : ""}
                          </span>
                        ) : (
                          <span className="text-slate-400 italic">Sin documento</span>
                        )}
                      </td>

                      <td className="py-3 px-4 text-slate-600">
                        {c.phone || <span className="text-slate-400 italic">—</span>}
                      </td>

                      <td className="py-3 px-4 text-right font-bold text-slate-900">
                        {balanceNum > 0 ? (
                          formatCOP(balanceNum)
                        ) : (
                          <span className="text-slate-400 font-normal">$ 0</span>
                        )}
                      </td>

                      <td className="py-3 px-4 text-center">
                        {isOverdue ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-bold bg-rose-50 text-rose-700 border border-rose-200">
                            {daysOverdue} días
                          </span>
                        ) : (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-emerald-50 text-emerald-700 border border-emerald-200">
                            Al día
                          </span>
                        )}
                      </td>

                      <td className="py-3 px-4 text-slate-600">
                        {c.price_tier_id ? (
                          <span className="inline-flex items-center gap-1 text-[11px] font-medium text-slate-700 bg-slate-100 px-2 py-0.5 rounded-md">
                            <Tag className="size-3 text-slate-400" />
                            Tarifa especial
                          </span>
                        ) : (
                          <span className="text-slate-400 italic">General</span>
                        )}
                      </td>

                      <td className="py-3 px-4 text-center">
                        <Link
                          href={`/clientes/${c.id}`}
                          className="inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-semibold text-teal-700 hover:bg-teal-50 transition-colors"
                        >
                          Ver ficha
                          <ChevronRight className="size-3.5" />
                        </Link>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Diálogo de nuevo cliente */}
      <CustomerFormDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSuccess={handleCustomerCreated}
      />
    </div>
  );
}

"use client";

import { useMemo, useState } from "react";
import {
  History,
  Lock,
  Search,
  ShieldCheck,
} from "lucide-react";

import { api } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { cn } from "@/lib/utils";

const ACTION_LABELS: Record<string, { label: string; color: string }> = {
  CIERRE_CAJA: { label: "Cierre de caja", color: "bg-emerald-50 text-emerald-700 border-emerald-200" },
  APERTURA_CAJA: { label: "Apertura de caja", color: "bg-emerald-50 text-emerald-700 border-emerald-200" },
  INGRESO_CAJA: { label: "Ingreso de efectivo", color: "bg-blue-50 text-blue-700 border-blue-200" },
  RETIRO_CAJA: { label: "Retiro de efectivo", color: "bg-amber-50 text-amber-700 border-amber-200" },
  CAMBIO_AJUSTES: { label: "Cambio de ajustes", color: "bg-purple-50 text-purple-700 border-purple-200" },
  CAMBIO_ROL: { label: "Cambio de rol", color: "bg-indigo-50 text-indigo-700 border-indigo-200" },
  CAMBIO_PERMISOS_ROL: { label: "Permisos de rol", color: "bg-indigo-50 text-indigo-700 border-indigo-200" },
  INVITACION_USUARIO: { label: "Invitación enviada", color: "bg-sky-50 text-sky-700 border-sky-200" },
  ACTIVAR_USUARIO: { label: "Usuario activado", color: "bg-teal-50 text-teal-700 border-teal-200" },
  DESACTIVAR_USUARIO: { label: "Usuario desactivado", color: "bg-rose-50 text-rose-700 border-rose-200" },
  AJUSTE_INVENTARIO: { label: "Ajuste de inventario", color: "bg-amber-50 text-amber-700 border-amber-200" },
  CANCELAR_ORDEN: { label: "Orden cancelada", color: "bg-rose-50 text-rose-700 border-rose-200" },
};

function formatDateTime(iso: string) {
  try {
    const d = new Date(iso);
    return d.toLocaleString("es-CO", {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function AuditScreen() {
  const [actionFilter, setActionFilter] = useState<string>("");
  const [userQuery, setUserQuery] = useState<string>("");

  const { data, reload } = useApi(() => api.audit.list(), "audit-list");
  const entries = useMemo(() => data ?? [], [data]);

  const filtered = useMemo(() => {
    return entries.filter((e) => {
      const matchAction = !actionFilter || e.action === actionFilter;
      const matchUser = !userQuery || JSON.stringify(e).toLowerCase().includes(userQuery.toLowerCase());
      return matchAction && matchUser;
    });
  }, [entries, actionFilter, userQuery]);

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Registro de auditoría</h1>
          <p className="mt-1 text-slate-500">
            Bitácora inmutable de eventos sensibles: aperturas, cierres de caja, ajustes y cambios de permisos.
          </p>
        </div>

        {/* Security badge */}
        <div className="flex items-center gap-2 rounded-2xl border border-emerald-200 bg-emerald-50 px-4 py-2 text-xs font-semibold text-emerald-800">
          <ShieldCheck className="size-4 text-emerald-600" />
          <span>Registro inmutable protegido contra modificación</span>
        </div>
      </div>

      {/* Notice */}
      <div className="flex items-start gap-3 rounded-2xl border border-slate-200 bg-slate-50 p-4 text-xs text-slate-600">
        <Lock className="size-4 shrink-0 text-slate-500 mt-0.5" />
        <div>
          <span className="font-semibold text-slate-900 block">Integridad criptográfica y append-only</span>
          Cada evento queda registrado de forma secuencial y definitiva en la base de datos. Ningún usuario (incluido el
          dueño) puede alterar o eliminar este historial.
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <div className="relative w-64 md:w-80">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              placeholder="Buscar en eventos y detalles..."
              value={userQuery}
              onChange={(e) => setUserQuery(e.target.value)}
              className="h-10 w-full rounded-full border border-slate-200 bg-white pr-4 pl-9 text-sm outline-none transition focus:border-slate-900"
            />
          </div>

          <select
            value={actionFilter}
            onChange={(e) => setActionFilter(e.target.value)}
            className="h-10 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 outline-none hover:bg-slate-50 focus:border-slate-900"
          >
            <option value="">Todas las acciones</option>
            {Object.entries(ACTION_LABELS).map(([action, meta]) => (
              <option key={action} value={action}>
                {meta.label}
              </option>
            ))}
          </select>
        </div>

        <button
          onClick={() => reload()}
          className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
        >
          <History className="size-3.5" />
          Actualizar bitácora
        </button>
      </div>

      {/* Table */}
      <div className="overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-sm">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-slate-100 bg-slate-50 text-xs font-semibold text-slate-500 uppercase">
            <tr>
              <th className="py-3.5 pr-4 pl-6">Fecha y hora</th>
              <th className="px-4 py-3.5">Acción</th>
              <th className="px-4 py-3.5">Entidad / Ref</th>
              <th className="px-4 py-3.5">Detalles del evento</th>
              <th className="py-3.5 pr-6 pl-4 text-right">Dirección IP</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {filtered.map((item) => {
              const meta = ACTION_LABELS[item.action] ?? {
                label: item.action,
                color: "bg-slate-100 text-slate-800 border-slate-200",
              };
              return (
                <tr key={item.id} className="transition hover:bg-slate-50/50">
                  <td className="py-4 pr-4 pl-6 font-medium text-slate-700 whitespace-nowrap">
                    {formatDateTime(item.created_at)}
                  </td>
                  <td className="px-4 py-4 whitespace-nowrap">
                    <span
                      className={cn(
                        "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold",
                        meta.color,
                      )}
                    >
                      {meta.label}
                    </span>
                  </td>
                  <td className="px-4 py-4 font-mono text-xs text-slate-600 whitespace-nowrap">
                    <span className="font-semibold text-slate-900">{item.entity}</span>
                    {item.entity_id && <span className="text-slate-400 ml-1">#{item.entity_id.slice(0, 10)}</span>}
                  </td>
                  <td className="px-4 py-4 text-xs text-slate-600">
                    <div className="max-w-md truncate font-mono bg-slate-50 p-1.5 rounded-lg border border-slate-100 text-[11px]">
                      {JSON.stringify(item.details)}
                    </div>
                  </td>
                  <td className="py-4 pr-6 pl-4 text-right font-mono text-xs text-slate-400 whitespace-nowrap">
                    {item.ip_address ?? "127.0.0.1"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}


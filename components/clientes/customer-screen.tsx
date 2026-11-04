"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  AlertCircle,
  AlertTriangle,
  Archive,
  ArrowLeft,
  Check,
  CreditCard,
  Edit2,
  Loader2,
  Package,
  Pin,
  ShoppingBag,
  User,
  Wallet,
} from "lucide-react";

import { CustomerFormDialog } from "./customer-form-dialog";
import {
  api,
  friendlyError,
  type CustomerDetail,
  type SaleHistoryRow,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { useSession } from "@/lib/stores/session";

type TabKey = "compras" | "cartera" | "separados" | "saldo" | "datos";

export function CustomerScreen({ customerId }: { customerId: string }) {
  const canWrite = useSession((s) => s.can("customers:write"));

  const [customer, setCustomer] = useState<CustomerDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [activeTab, setActiveTab] = useState<TabKey>("datos");

  // Edición rápida de la nota fijada
  const [editingNote, setEditingNote] = useState(false);
  const [noteValue, setNoteValue] = useState("");
  const [savingNote, setSavingNote] = useState(false);

  // Diálogo de edición completa
  const [editDialogOpen, setEditDialogOpen] = useState(false);

  // Historial de compras
  const [purchases, setPurchases] = useState<SaleHistoryRow[]>([]);
  const [loadingPurchases, setLoadingPurchases] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.customers
      .get(customerId)
      .then((data) => {
        if (!cancelled) {
          setCustomer(data);
          setNoteValue(data.pinned_note || "");
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(friendlyError(err));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [customerId]);

  // Cargar compras cuando se activa la pestaña compras
  useEffect(() => {
    if (activeTab !== "compras" || !customerId) return;
    let cancelled = false;
    api.customers
      .getPurchases(customerId)
      .then((page) => {
        if (!cancelled) {
          setPurchases(page.items);
          setLoadingPurchases(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          console.error("Error cargando compras:", err);
          setLoadingPurchases(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab, customerId]);

  const handleSaveNote = async () => {
    if (!customer) return;
    setSavingNote(true);
    try {
      const updated = await api.customers.update(customer.id, {
        name: customer.name,
        doc_type: customer.doc_type,
        doc_number: customer.doc_number,
        dv: customer.dv,
        phone: customer.phone,
        email: customer.email,
        price_tier_id: customer.price_tier_id,
        tax_regime: customer.tax_regime,
        fiscal_responsibilities: customer.fiscal_responsibilities,
        ciiu: customer.ciiu,
        withholding_agent: customer.withholding_agent,
        city_code: customer.city_code,
        billing_address: customer.billing_address,
        shipping_address: customer.shipping_address,
        pinned_note: noteValue.trim() || null,
        credit_limit: customer.credit_limit,
        credit_block_days: customer.credit_block_days,
      });
      setCustomer(updated);
      setEditingNote(false);
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setSavingNote(false);
    }
  };

  const handleArchiveToggle = async () => {
    if (!customer) return;
    const isArchived = Boolean(customer.archived_at);
    if (!window.confirm(isArchived ? "¿Deseas desarchivar este cliente?" : "¿Deseas archivar este cliente?")) {
      return;
    }
    try {
      if (isArchived) {
        await api.customers.unarchive(customer.id);
      } else {
        await api.customers.archive(customer.id);
      }
      const fresh = await api.customers.get(customer.id);
      setCustomer(fresh);
    } catch (err) {
      setError(friendlyError(err));
    }
  };

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center p-24 text-slate-400">
        <Loader2 className="size-8 animate-spin text-teal-600 mb-3" />
        <p className="text-sm font-medium">Cargando información del cliente...</p>
      </div>
    );
  }

  if (error || !customer) {
    return (
      <div className="p-6 max-w-4xl mx-auto">
        <div className="flex items-center gap-2 rounded-2xl bg-rose-50 p-4 text-xs text-rose-700">
          <AlertCircle className="size-5 shrink-0" />
          <span>{error || "No se encontró el cliente solicitado."}</span>
        </div>
        <Link
          href="/clientes"
          className="mt-4 inline-flex items-center gap-1.5 text-xs font-semibold text-teal-700 hover:underline"
        >
          <ArrowLeft className="size-3.5" />
          Volver al directorio
        </Link>
      </div>
    );
  }

  const isArchived = Boolean(customer.archived_at);

  return (
    <div className="flex flex-col gap-6 p-6 max-w-6xl mx-auto">
      {/* Botón Volver */}
      <div className="flex items-center justify-between">
        <Link
          href="/clientes"
          className="inline-flex items-center gap-1.5 text-xs font-semibold text-slate-600 hover:text-slate-900 transition-colors"
        >
          <ArrowLeft className="size-4" />
          Volver al directorio de clientes
        </Link>

        <div className="flex items-center gap-2">
          {canWrite && (
            <>
              <button
                type="button"
                onClick={() => setEditDialogOpen(true)}
                className="flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3.5 py-1.5 text-xs font-semibold text-slate-700 hover:bg-slate-50 transition-colors shadow-xs"
              >
                <Edit2 className="size-3.5" />
                Editar ficha
              </button>

              <button
                type="button"
                onClick={handleArchiveToggle}
                className={`flex items-center gap-1.5 rounded-xl border px-3.5 py-1.5 text-xs font-semibold transition-colors shadow-xs ${
                  isArchived
                    ? "border-emerald-200 bg-emerald-50 text-emerald-800 hover:bg-emerald-100"
                    : "border-slate-200 bg-white text-slate-600 hover:bg-slate-50"
                }`}
              >
                <Archive className="size-3.5" />
                {isArchived ? "Desarchivar" : "Archivar"}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Encabezado principal */}
      <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-xs flex flex-col gap-4">
        <div className="flex flex-col md:flex-row md:items-start justify-between gap-4">
          <div className="flex items-start gap-3.5">
            <span className="flex size-14 shrink-0 items-center justify-center rounded-2xl bg-teal-50 text-teal-700 text-xl font-bold">
              {customer.name.slice(0, 2).toUpperCase()}
            </span>
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <h1 className="text-2xl font-black text-slate-900">{customer.name}</h1>
                {isArchived && (
                  <span className="rounded-full bg-slate-100 px-2.5 py-0.5 text-[11px] font-bold text-slate-600 border border-slate-200">
                    Archivado
                  </span>
                )}
                {customer.person_type === "JURIDICA" ? (
                  <span className="rounded-full bg-blue-50 px-2.5 py-0.5 text-[11px] font-semibold text-blue-700 border border-blue-200">
                    Persona Jurídica
                  </span>
                ) : (
                  <span className="rounded-full bg-slate-100 px-2.5 py-0.5 text-[11px] font-medium text-slate-600">
                    Persona Natural
                  </span>
                )}
              </div>

              <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-600">
                {customer.doc_type && customer.doc_number ? (
                  <span className="font-semibold text-slate-800">
                    {customer.doc_type}: {customer.doc_number}
                    {customer.dv ? `-${customer.dv}` : ""}
                  </span>
                ) : (
                  <span className="italic text-slate-400">Sin documento</span>
                )}
                {customer.phone && <span>Tel: {customer.phone}</span>}
                {customer.email && <span>{customer.email}</span>}
                {customer.city_code && <span>Mun: {customer.city_code}</span>}
              </div>
            </div>
          </div>

          {/* Tarjetas rápidas de saldo y cupo */}
          <div className="flex items-center gap-3">
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-3 text-right min-w-[120px]">
              <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
                Saldo Cartera
              </span>
              <span
                className={`text-base font-black ${
                  Number(customer.balance) > 0 ? "text-rose-600" : "text-slate-800"
                }`}
              >
                {formatCOP(Number(customer.balance || 0))}
              </span>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-3 text-right min-w-[120px]">
              <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
                Cupo Disponible
              </span>
              <span className="text-base font-black text-teal-700">
                {formatCOP(Number(customer.credit_available || 0))}
              </span>
            </div>
          </div>
        </div>

        {/* Avisos amarillos por campos de missing_fiscal */}
        {customer.missing_fiscal && customer.missing_fiscal.length > 0 && (
          <div className="rounded-2xl border border-amber-200 bg-amber-50/70 p-3.5 text-xs text-amber-900 flex flex-col gap-1.5">
            <div className="flex items-center gap-2 font-bold">
              <AlertTriangle className="size-4 text-amber-700 shrink-0" />
              <span>Faltan datos para emitir Factura Electrónica:</span>
            </div>
            <ul className="list-disc list-inside pl-1 text-[11px] text-amber-800">
              {customer.missing_fiscal.map((field) => (
                <li key={field}>
                  Campo pendiente: <span className="font-semibold">{field}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Nota fijada editable */}
        <div className="rounded-2xl border border-slate-200 bg-amber-50/30 p-3.5 text-xs text-slate-700">
          <div className="flex items-center justify-between mb-1.5">
            <div className="flex items-center gap-1.5 font-bold text-amber-900 text-xs">
              <Pin className="size-3.5 text-amber-700" />
              <span>Nota fijada (visible en cobro y caja)</span>
            </div>
            {canWrite && !editingNote && (
              <button
                type="button"
                onClick={() => setEditingNote(true)}
                className="text-[11px] font-semibold text-teal-700 hover:underline flex items-center gap-1"
              >
                <Edit2 className="size-3" />
                Editar nota
              </button>
            )}
          </div>

          {editingNote ? (
            <div className="flex flex-col gap-2 mt-2">
              <textarea
                maxLength={300}
                rows={2}
                value={noteValue}
                onChange={(e) => setNoteValue(e.target.value)}
                placeholder="Escribe instrucciones o recordatorios sobre este cliente..."
                className="w-full rounded-xl border border-slate-200 bg-white p-2.5 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
              />
              <div className="flex items-center justify-end gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setEditingNote(false);
                    setNoteValue(customer.pinned_note || "");
                  }}
                  className="rounded-lg px-2.5 py-1 text-xs text-slate-600 hover:bg-slate-100"
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  disabled={savingNote}
                  onClick={handleSaveNote}
                  className="flex items-center gap-1.5 rounded-lg bg-teal-700 px-3 py-1 text-xs font-semibold text-white hover:bg-teal-800 disabled:opacity-50"
                >
                  {savingNote ? <Loader2 className="size-3 animate-spin" /> : <Check className="size-3" />}
                  Guardar
                </button>
              </div>
            </div>
          ) : (
            <p className="text-slate-700 italic">
              {customer.pinned_note || "Sin nota fijada. Haz clic en 'Editar nota' para agregar una."}
            </p>
          )}
        </div>
      </div>

      {/* Pestañas de detalle */}
      <div className="flex rounded-2xl bg-slate-100 p-1 text-xs font-semibold self-start">
        <button
          type="button"
          onClick={() => setActiveTab("datos")}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
            activeTab === "datos" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
          }`}
        >
          <User className="size-3.5" />
          Datos Generales
        </button>

        <button
          type="button"
          onClick={() => setActiveTab("compras")}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
            activeTab === "compras" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
          }`}
        >
          <ShoppingBag className="size-3.5" />
          Historial de Compras
        </button>

        <button
          type="button"
          onClick={() => setActiveTab("cartera")}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
            activeTab === "cartera" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
          }`}
        >
          <CreditCard className="size-3.5" />
          Cartera (Fiado)
        </button>

        <button
          type="button"
          onClick={() => setActiveTab("separados")}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
            activeTab === "separados" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
          }`}
        >
          <Package className="size-3.5" />
          Separados
        </button>

        <button
          type="button"
          onClick={() => setActiveTab("saldo")}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
            activeTab === "saldo" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
          }`}
        >
          <Wallet className="size-3.5" />
          Saldo a Favor
        </button>
      </div>

      {/* Contenido de la pestaña */}
      <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-xs">
        {/* Pestaña: Datos Generales */}
        {activeTab === "datos" && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="flex flex-col gap-4">
              <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
                Información Fiscal y Tributaria
              </h3>
              <div className="divide-y divide-slate-100 text-xs">
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Régimen tributario:</span>
                  <span className="font-semibold text-slate-800">
                    {customer.tax_regime === "RESPONSABLE_IVA"
                      ? "Responsable de IVA"
                      : "No responsable de IVA"}
                  </span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Código CIIU:</span>
                  <span className="font-semibold text-slate-800">{customer.ciiu || "—"}</span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Municipio DANE:</span>
                  <span className="font-semibold text-slate-800">{customer.city_code || "—"}</span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Autorretenedor:</span>
                  <span className="font-semibold text-slate-800">
                    {customer.withholding_agent ? "Sí" : "No"}
                  </span>
                </div>
                <div className="py-2.5 flex flex-col gap-1">
                  <span className="text-slate-500">Responsabilidades fiscales:</span>
                  <div className="flex flex-wrap gap-1.5 mt-1">
                    {customer.fiscal_responsibilities?.map((r) => (
                      <span
                        key={r}
                        className="rounded-md bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-700"
                      >
                        {r}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>

            <div className="flex flex-col gap-4">
              <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
                Condiciones Comerciales y Direcciones
              </h3>
              <div className="divide-y divide-slate-100 text-xs">
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Cupo total de crédito:</span>
                  <span className="font-bold text-teal-700">
                    {formatCOP(Number(customer.credit_limit || 0))}
                  </span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Tolerancia días de mora:</span>
                  <span className="font-semibold text-slate-800">
                    {customer.credit_block_days} días
                  </span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Dirección de facturación:</span>
                  <span className="font-semibold text-slate-800 text-right max-w-xs">
                    {customer.billing_address || "—"}
                  </span>
                </div>
                <div className="py-2.5 flex justify-between">
                  <span className="text-slate-500">Dirección de despacho:</span>
                  <span className="font-semibold text-slate-800 text-right max-w-xs">
                    {customer.shipping_address || "—"}
                  </span>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Pestaña: Historial de Compras */}
        {activeTab === "compras" && (
          <div>
            <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider mb-4">
              Ventas realizadas a este cliente
            </h3>
            {loadingPurchases ? (
              <div className="py-12 text-center text-slate-400">
                <Loader2 className="size-5 animate-spin mx-auto mb-2 text-teal-600" />
                Cargando historial de compras...
              </div>
            ) : purchases.length === 0 ? (
              <div className="py-12 text-center text-slate-400 text-xs">
                Este cliente aún no registra compras facturadas.
              </div>
            ) : (
              <table className="w-full text-left text-xs">
                <thead>
                  <tr className="border-b border-slate-100 text-slate-500 font-semibold">
                    <th className="py-2.5 px-3">Recibo / Factura</th>
                    <th className="py-2.5 px-3">Fecha</th>
                    <th className="py-2.5 px-3">Cajero</th>
                    <th className="py-2.5 px-3 text-right">Total</th>
                    <th className="py-2.5 px-3 text-center">Estado</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {purchases.map((p) => (
                    <tr key={p.id} className="hover:bg-slate-50">
                      <td className="py-2.5 px-3 font-bold text-slate-800">{p.receipt_number}</td>
                      <td className="py-2.5 px-3 text-slate-600">
                        {new Date(p.created_at).toLocaleDateString("es-CO")}
                      </td>
                      <td className="py-2.5 px-3 text-slate-600">{p.cashier_name || "—"}</td>
                      <td className="py-2.5 px-3 text-right font-bold text-slate-900">
                        {formatCOP(Number(p.total))}
                      </td>
                      <td className="py-2.5 px-3 text-center">
                        <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                          {p.status}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}

        {/* Pestaña: Cartera */}
        {activeTab === "cartera" && (
          <div className="flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
                Estado de Cuentas por Cobrar
              </h3>
              <span className="text-xs font-medium text-slate-500">
                Saldo pendiente: <strong className="text-rose-600">{formatCOP(Number(customer.balance || 0))}</strong>
              </span>
            </div>
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-8 text-center text-slate-500 text-xs">
              <CreditCard className="size-8 mx-auto mb-2 opacity-30 text-teal-700" />
              <p className="font-medium text-slate-700">Módulo de gestión de cuentas por cobrar (F3.2 - F3.5)</p>
              <p className="text-[11px] text-slate-400 mt-1">
                Los abonos, cuotas, acuerdos y cobro por WhatsApp estarán disponibles en las siguientes tareas.
              </p>
            </div>
          </div>
        )}

        {/* Pestaña: Separados */}
        {activeTab === "separados" && (
          <div className="flex flex-col gap-4">
            <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
              Separados Activos y Vencidos
            </h3>
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-8 text-center text-slate-500 text-xs">
              <Package className="size-8 mx-auto mb-2 opacity-30 text-teal-700" />
              <p className="font-medium text-slate-700">Módulo de separados (F3.7)</p>
              <p className="text-[11px] text-slate-400 mt-1">
                La creación de órdenes de separado y registro de cuotas iniciales se habilitará en F3.7.
              </p>
            </div>
          </div>
        )}

        {/* Pestaña: Saldo a favor */}
        {activeTab === "saldo" && (
          <div className="flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
                Billetera de Saldo a Favor
              </h3>
              <span className="text-xs font-medium text-slate-500">
                Saldo disponible:{" "}
                <strong className="text-teal-700 font-black">
                  {formatCOP(Number(customer.wallet_balance || 0))}
                </strong>
              </span>
            </div>
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-8 text-center text-slate-500 text-xs">
              <Wallet className="size-8 mx-auto mb-2 opacity-30 text-teal-700" />
              <p className="font-medium text-slate-700">Módulo de saldo a favor (F3.6)</p>
              <p className="text-[11px] text-slate-400 mt-1">
                Las recargas, reembolsos a billetera y movimientos se habilitarán en F3.6.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Diálogo de edición completa */}
      <CustomerFormDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        customer={customer}
        onSuccess={(updated) => setCustomer(updated)}
      />
    </div>
  );
}

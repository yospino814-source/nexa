"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  AlertCircle,
  AlertTriangle,
  Archive,
  ArrowLeft,
  Check,
  CreditCard,
  DollarSign,
  Edit2,
  FileText,
  Loader2,
  MessageSquare,
  Pin,
  ShoppingBag,
  User,
} from "lucide-react";

import { CustomerFormDialog } from "./customer-form-dialog";
import { PaymentTicketDialog } from "./payment-ticket";
import { StatementDialog } from "@/components/cartera/statement-dialog";
import { MoneyInput } from "@/components/caja/money-input";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  api,
  ApiError,
  friendlyError,
  type CustomerDetail,
  type ReceivableDetail,
  type ReceivablePaymentResponse,
  type SaleHistoryRow,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { useSession } from "@/lib/stores/session";

type TabKey = "cartera" | "compras" | "datos";

function formatDateDMY(d: string): string {
  const parts = d.slice(0, 10).split("-");
  if (parts.length === 3) {
    return `${parts[2].padStart(2, "0")}/${parts[1].padStart(2, "0")}/${parts[0]}`;
  }
  return d;
}

export function CustomerScreen({ customerId }: { customerId: string }) {
  const canWrite = useSession((s) => s.can("customers:write"));
  const canRead = useSession((s) => s.can("customers:read"));
  const canCollect = useSession((s) => s.can("credit:collect"));
  const cashierName = useSession((s) => s.name || "Cajero");

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

  // Cartera y cuentas por cobrar
  const [receivables, setReceivables] = useState<ReceivableDetail[]>([]);
  const [loadingReceivables, setLoadingReceivables] = useState(false);

  // Diálogo de abono
  const [payDialogOpen, setPayDialogOpen] = useState(false);
  const [selectedReceivable, setSelectedReceivable] = useState<ReceivableDetail | null>(null);
  const [payId, setPayId] = useState("");
  const [payAmount, setPayAmount] = useState(0);
  const [payMethod, setPayMethod] = useState<"EFECTIVO" | "DATAFONO" | "QR" | "TRANSFERENCIA">("EFECTIVO");
  const [payReference, setPayReference] = useState("");
  const [submittingPay, setSubmittingPay] = useState(false);
  const [paymentError, setPaymentError] = useState<string | null>(null);
  const [paymentErrorCode, setPaymentErrorCode] = useState<string | null>(null);

  // Tirilla de abono
  const [ticketOpen, setTicketOpen] = useState(false);
  const [ticketPayment, setTicketPayment] = useState<ReceivablePaymentResponse | null>(null);
  const [ticketReceivable, setTicketReceivable] = useState<ReceivableDetail | null>(null);

  // Estado de cuenta y WhatsApp (F3.5)
  const [statementDialogOpen, setStatementDialogOpen] = useState(false);
  const [whatsAppError, setWhatsAppError] = useState<string | null>(null);

  const reloadCustomer = useCallback(async () => {
    if (!customerId) return;
    try {
      const data = await api.customers.get(customerId);
      setCustomer(data);
      setNoteValue(data.pinned_note || "");
    } catch (err) {
      console.error("Error recargando cliente:", err);
    }
  }, [customerId]);

  const loadReceivables = useCallback(async () => {
    if (!customerId) return;
    setLoadingReceivables(true);
    try {
      const list = await api.receivables.list({ customer_id: customerId });
      const details = await Promise.all(
        list.map((r) => api.receivables.get(r.id).catch(() => null)),
      );
      setReceivables(details.filter((d): d is ReceivableDetail => d !== null));
    } catch (err) {
      console.error("Error cargando cartera:", err);
    } finally {
      setLoadingReceivables(false);
    }
  }, [customerId]);

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

  // Cargar cartera cuando se activa la pestaña cartera
  useEffect(() => {
    if (activeTab !== "cartera" || !customerId) return;
    let cancelled = false;
    Promise.resolve().then(() => {
      if (!cancelled) {
        void loadReceivables();
      }
    });
    return () => {
      cancelled = true;
    };
  }, [activeTab, customerId, loadReceivables]);

  const handleWhatsApp = async (recId: string) => {
    setWhatsAppError(null);
    try {
      const res = await api.receivables.whatsappLink(recId);
      window.open(res.url, "_blank", "noopener,noreferrer");
    } catch (err) {
      if (err instanceof ApiError && err.code === "CUSTOMER_NO_PHONE") {
        setWhatsAppError("El cliente no tiene un celular válido para WhatsApp.");
      } else {
        setWhatsAppError(friendlyError(err));
      }
    }
  };

  const handleOpenPay = (rec: ReceivableDetail) => {
    setSelectedReceivable(rec);
    setPayId(crypto.randomUUID());
    setPayAmount(Number(rec.balance));
    setPayMethod("EFECTIVO");
    setPayReference("");
    setPaymentError(null);
    setPaymentErrorCode(null);
    setPayDialogOpen(true);
  };

  const handlePaySubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedReceivable) return;

    if (payAmount <= 0) {
      setPaymentError("El abono debe ser mayor a cero.");
      return;
    }
    if (payAmount > Number(selectedReceivable.balance)) {
      setPaymentError("El abono supera el saldo de la cuenta.");
      return;
    }
    if (payMethod !== "EFECTIVO" && (!payReference.trim() || payReference.trim().length < 3 || payReference.trim().length > 60)) {
      setPaymentError("Se requiere una referencia válida (de 3 a 60 caracteres).");
      return;
    }

    setSubmittingPay(true);
    setPaymentError(null);
    setPaymentErrorCode(null);
    try {
      const resp = await api.receivables.pay(selectedReceivable.id, {
        id: payId,
        amount: payAmount.toFixed(2),
        method: payMethod,
        reference: payMethod === "EFECTIVO" ? undefined : payReference.trim(),
      });
      setPayDialogOpen(false);
      await Promise.all([loadReceivables(), reloadCustomer()]);
      setTicketPayment(resp);
      setTicketReceivable(selectedReceivable);
      setTicketOpen(true);
    } catch (err: unknown) {
      if (err instanceof ApiError) {
        setPaymentErrorCode(err.code || null);
        setPaymentError(friendlyError(err));
      } else {
        setPaymentError("Ocurrió un error inesperado al registrar el abono.");
      }
    } finally {
      setSubmittingPay(false);
    }
  };

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
                placeholder="Escribí instrucciones o recordatorios sobre este cliente..."
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
              {customer.pinned_note || "Sin nota fijada. Hacé clic en 'Editar nota' para agregar una."}
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

        {canRead && (
          <button
            type="button"
            onClick={() => setActiveTab("cartera")}
            className={`flex items-center gap-2 rounded-xl px-4 py-2 transition-all ${
              activeTab === "cartera" ? "bg-white text-teal-800 shadow-xs" : "text-slate-600 hover:text-slate-900"
            }`}
          >
            <CreditCard className="size-3.5" />
            Cartera
          </button>
        )}
      </div>

      {/* Contenido de la pestaña */}
      <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-xs">
        {/* Pestaña: Cartera */}
        {activeTab === "cartera" && (
          <div className="space-y-6">
            {whatsAppError && (
              <div className="rounded-xl border border-amber-200 bg-amber-50 p-4 text-xs font-semibold text-amber-800 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <AlertCircle className="size-4 shrink-0 text-amber-600" />
                  <span>{whatsAppError}</span>
                </div>
                <button
                  type="button"
                  onClick={() => setWhatsAppError(null)}
                  className="text-amber-700 hover:text-amber-900 font-bold"
                >
                  ✕
                </button>
              </div>
            )}

            {/* Encabezado: Tarjetas Saldo, Cupo disponible y Días de mora */}
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
                <span className="text-xs font-bold text-slate-400 uppercase tracking-wider block mb-1">
                  Saldo
                </span>
                <span className="text-xl font-black text-slate-900">
                  {formatCOP(Number(customer.balance))}
                </span>
              </div>

              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
                <span className="text-xs font-bold text-slate-400 uppercase tracking-wider block mb-1">
                  Cupo disponible
                </span>
                <span className="text-xl font-black text-teal-700">
                  {formatCOP(Math.max(0, Number(customer.credit_limit || 0) - Number(customer.balance || 0)))}
                </span>
              </div>

              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
                <span className="text-xs font-bold text-slate-400 uppercase tracking-wider block mb-1">
                  Días de mora
                </span>
                <span
                  className={`text-xl font-black ${
                    (customer.max_days_overdue ?? 0) > 0 ? "text-rose-600" : "text-slate-900"
                  }`}
                >
                  {customer.max_days_overdue ?? 0} días
                </span>
              </div>
            </div>

            {/* Listado de cuentas por cobrar */}
            <div>
              <div className="flex flex-wrap items-center justify-between gap-2 mb-4">
                <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider">
                  Cuentas por cobrar pendientes
                </h3>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setStatementDialogOpen(true)}
                  className="text-xs font-semibold text-teal-800 border-teal-200 hover:bg-teal-50 gap-1.5"
                >
                  <FileText className="size-3.5" />
                  Estado de cuenta
                </Button>
              </div>

              {loadingReceivables ? (
                <div className="py-12 text-center text-slate-400">
                  <Loader2 className="size-5 animate-spin mx-auto mb-2 text-teal-600" />
                  Cargando cartera...
                </div>
              ) : receivables.filter((r) => r.status === "ABIERTA" || r.status === "PARCIAL").length === 0 ? (
                <div className="py-12 text-center text-slate-400 text-xs">
                  Este cliente no tiene cuentas por cobrar pendientes.
                </div>
              ) : (
                <div className="space-y-4">
                  {receivables
                    .filter((r) => r.status === "ABIERTA" || r.status === "PARCIAL")
                    .map((rec) => (
                      <div
                        key={rec.id}
                        className="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs space-y-4"
                      >
                        {/* Cabecera de la cuenta */}
                        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-100 pb-3">
                          <div>
                            <div className="flex items-center gap-2">
                              <span className="font-bold text-slate-900 text-sm">
                                Venta {rec.receipt_number || "Sin número"}
                              </span>
                              <span
                                className={`rounded-full px-2 py-0.5 text-[10px] font-bold ${
                                  rec.status === "ABIERTA"
                                    ? "bg-amber-50 text-amber-700 border border-amber-200"
                                    : "bg-teal-50 text-teal-700 border border-teal-200"
                                }`}
                              >
                                {rec.status === "ABIERTA" ? "Abierta" : "Parcial"}
                              </span>
                            </div>
                            <span className="text-xs text-slate-500">
                              Emitida el {formatDateDMY(rec.created_at)}
                            </span>
                          </div>

                          <div className="flex items-center gap-2">
                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              onClick={() => handleWhatsApp(rec.id)}
                              className="border-emerald-200 text-emerald-800 hover:bg-emerald-50 font-semibold text-xs"
                              title="Cobrar por WhatsApp"
                            >
                              <MessageSquare className="size-3.5 mr-1" />
                              Cobrar por WhatsApp
                            </Button>
                            {canCollect && (
                              <Button
                                size="sm"
                                onClick={() => handleOpenPay(rec)}
                                className="bg-teal-700 hover:bg-teal-800 text-white font-semibold text-xs"
                              >
                                <DollarSign className="size-3.5 mr-1" />
                                Abonar
                              </Button>
                            )}
                          </div>
                        </div>

                        {/* Métricas de la cuenta */}
                        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs">
                          <div>
                            <span className="text-slate-400 block">Total</span>
                            <span className="font-semibold text-slate-800">{formatCOP(Number(rec.amount_total))}</span>
                          </div>
                          <div>
                            <span className="text-slate-400 block">Pagado</span>
                            <span className="font-semibold text-slate-800">{formatCOP(Number(rec.amount_paid))}</span>
                          </div>
                          <div>
                            <span className="text-slate-400 block">Saldo</span>
                            <span className="font-bold text-rose-600">{formatCOP(Number(rec.balance))}</span>
                          </div>
                          <div>
                            <span className="text-slate-400 block">Vencimiento</span>
                            <span className="font-semibold text-slate-800">{formatDateDMY(rec.due_date)}</span>
                          </div>
                        </div>

                        {/* Tabla de cuotas */}
                        {rec.installments && rec.installments.length > 0 && (
                          <div className="rounded-xl border border-slate-100 overflow-hidden">
                            <table className="w-full text-left text-xs">
                              <thead>
                                <tr className="bg-slate-50 text-slate-500 font-semibold border-b border-slate-100">
                                  <th className="py-2 px-3">Cuota</th>
                                  <th className="py-2 px-3">Vence</th>
                                  <th className="py-2 px-3 text-right">Valor</th>
                                  <th className="py-2 px-3 text-right">Pagado</th>
                                  <th className="py-2 px-3 text-center">Estado</th>
                                </tr>
                              </thead>
                              <tbody className="divide-y divide-slate-100">
                                {rec.installments.map((inst) => (
                                  <tr key={inst.n} className="hover:bg-slate-50/50">
                                    <td className="py-2 px-3 font-medium text-slate-800">
                                      Cuota {inst.n}
                                    </td>
                                    <td className="py-2 px-3 text-slate-600">
                                      {formatDateDMY(inst.due_date)}
                                    </td>
                                    <td className="py-2 px-3 text-right font-medium text-slate-800">
                                      {formatCOP(Number(inst.amount))}
                                    </td>
                                    <td className="py-2 px-3 text-right text-slate-600">
                                      {formatCOP(Number(inst.amount_paid))}
                                    </td>
                                    <td className="py-2 px-3 text-center">
                                      {inst.paid ? (
                                        <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                                          Pagada
                                        </span>
                                      ) : inst.days_overdue > 0 ? (
                                        <span className="rounded-full bg-rose-50 px-2 py-0.5 text-[10px] font-bold text-rose-600">
                                          Vencida ({inst.days_overdue} días)
                                        </span>
                                      ) : (
                                        <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-medium text-slate-600">
                                          Pendiente
                                        </span>
                                      )}
                                    </td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </div>
                        )}
                      </div>
                    ))}
                </div>
              )}
            </div>
          </div>
        )}


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
      </div>

      {/* Diálogo de edición completa */}
      <CustomerFormDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        customer={customer}
        onSuccess={(updated) => setCustomer(updated)}
      />

      {/* Diálogo de Registrar Abono */}
      <Dialog open={payDialogOpen} onOpenChange={setPayDialogOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Registrar abono</DialogTitle>
            {selectedReceivable && (
              <p className="text-xs text-muted-foreground mt-1">
                Venta {selectedReceivable.receipt_number || "Sin número"} · Saldo pendiente:{" "}
                <span className="font-bold text-rose-600">{formatCOP(Number(selectedReceivable.balance))}</span>
              </p>
            )}
          </DialogHeader>

          <form onSubmit={handlePaySubmit} className="space-y-4 py-2">
            <div>
              <label htmlFor="pay-amount" className="block text-xs font-semibold text-slate-700 mb-1.5">
                Monto a abonar
              </label>
              <MoneyInput
                id="pay-amount"
                value={payAmount}
                onChange={setPayAmount}
                autoFocus
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                Método de pago
              </label>
              <div className="grid grid-cols-2 gap-2">
                {(
                  [
                    { id: "EFECTIVO", label: "Efectivo" },
                    { id: "DATAFONO", label: "Datáfono" },
                    { id: "QR", label: "QR" },
                    { id: "TRANSFERENCIA", label: "Transferencia" },
                  ] as const
                ).map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => setPayMethod(m.id)}
                    className={`rounded-xl border p-2.5 text-xs font-semibold transition-all ${
                      payMethod === m.id
                        ? "border-teal-700 bg-teal-50 text-teal-900 shadow-xs ring-1 ring-teal-700"
                        : "border-slate-200 bg-white text-slate-700 hover:bg-slate-50"
                    }`}
                  >
                    {m.label}
                  </button>
                ))}
              </div>
            </div>

            {payMethod !== "EFECTIVO" && (
              <div>
                <label htmlFor="pay-reference" className="block text-xs font-semibold text-slate-700 mb-1.5">
                  Referencia <span className="text-rose-500">*</span>
                </label>
                <input
                  id="pay-reference"
                  type="text"
                  required
                  placeholder="Ej: AUT-987654"
                  value={payReference}
                  onChange={(e) => setPayReference(e.target.value)}
                  className="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm font-medium outline-none focus:border-teal-600 focus:ring-1 focus:ring-teal-600"
                />
                <p className="text-[11px] text-slate-400 mt-1">
                  Mínimo 3 caracteres, obligatorio para transferencias y tarjetas.
                </p>
              </div>
            )}

            {paymentError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700 space-y-1">
                <div className="flex items-center gap-2">
                  <AlertCircle className="size-4 shrink-0 text-rose-600" />
                  <span>{paymentError}</span>
                </div>
                {paymentErrorCode === "PAYMENT_NEEDS_OPEN_SESSION" && (
                  <div className="pl-6 pt-1">
                    <Link
                      href="/caja"
                      className="font-bold underline text-rose-800 hover:text-rose-950"
                    >
                      Abrir caja
                    </Link>
                  </div>
                )}
              </div>
            )}

            <DialogFooter className="flex sm:justify-between gap-2 pt-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => setPayDialogOpen(false)}
                disabled={submittingPay}
              >
                Cancelar
              </Button>
              <Button
                type="submit"
                disabled={submittingPay || payAmount <= 0}
                className="bg-teal-700 hover:bg-teal-800 text-white font-semibold"
              >
                {submittingPay ? (
                  <>
                    <Loader2 className="size-4 animate-spin mr-1.5" />
                    Registrando...
                  </>
                ) : (
                  "Registrar abono"
                )}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Tirilla de comprobante de abono */}
      {ticketPayment && customer && (
        <PaymentTicketDialog
          open={ticketOpen}
          onClose={() => setTicketOpen(false)}
          payment={ticketPayment}
          customer={customer}
          receiptNumber={ticketReceivable?.receipt_number}
          cashierName={cashierName}
        />
      )}

      {/* Estado de cuenta */}
      {customer && (
        <StatementDialog
          open={statementDialogOpen}
          onClose={() => setStatementDialogOpen(false)}
          customerId={customer.id}
          customerName={customer.name}
          customerDoc={customer.doc_number ? `${customer.doc_type || ""} ${customer.doc_number}` : null}
        />
      )}
    </div>
  );
}

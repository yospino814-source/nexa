"use client";

import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Building2,
  Calendar,
  CheckCircle2,
  Clock,
  FileText,
  History,
  Loader2,
  PackageCheck,
  XCircle,
} from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type AuditEntry,
  type Purchase,
  type PurchaseStatus,
} from "@/lib/api";
import { formatCOP, formatDateTime, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUS_BADGES: Record<
  PurchaseStatus,
  { label: string; className: string; icon: typeof CheckCircle2 }
> = {
  PENDIENTE: {
    label: "Borrador",
    className: "border-amber-200 bg-amber-50 text-amber-700",
    icon: AlertTriangle,
  },
  APROBADA: {
    label: "Aprobada",
    className: "border-blue-200 bg-blue-50 text-blue-700",
    icon: CheckCircle2,
  },
  PARCIAL: {
    label: "Parcial",
    className: "border-purple-200 bg-purple-50 text-purple-700",
    icon: AlertTriangle,
  },
  RECIBIDA: {
    label: "Recibida",
    className: "border-emerald-200 bg-emerald-50 text-emerald-700",
    icon: CheckCircle2,
  },
  ANULADA: {
    label: "Anulada",
    className: "border-slate-200 bg-slate-100 text-slate-500",
    icon: XCircle,
  },
};

type ReceiveLineState = {
  purchaseItemId: string;
  productName: string;
  qtyOrdered: number;
  qtyReceived: number;
  qtyPending: number;
  qtyToReceive: number;
  trackLots: boolean;
  lotCode: string;
  expiresAt: string;
};

export function PurchaseDetailDialog({
  purchaseId,
  branchName,
  open,
  onClose,
  onActionDone,
}: {
  purchaseId: string | null;
  branchName: string;
  open: boolean;
  onClose: () => void;
  onActionDone: (message: string) => void;
}) {
  const [purchase, setPurchase] = useState<Purchase | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Audit timeline
  const [timeline, setTimeline] = useState<AuditEntry[]>([]);
  const [loadingTimeline, setLoadingTimeline] = useState(false);

  // Modes
  const [confirmingCancel, setConfirmingCancel] = useState(false);
  const [isReceivingModal, setIsReceivingModal] = useState(false);
  const [receiveLines, setReceiveLines] = useState<ReceiveLineState[]>([]);

  const openKey = open && purchaseId ? purchaseId : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    setPurchase(null);
    setLoading(Boolean(openKey));
    setError(null);
    setConfirmingCancel(false);
    setIsReceivingModal(false);
    setTimeline([]);
    setLoadingTimeline(Boolean(openKey));
  }

  useEffect(() => {
    if (!open || !purchaseId) return;
    let ignore = false;

    api.purchases
      .get(purchaseId)
      .then((p) => {
        if (!ignore) {
          setPurchase(p);
          // Preload receive lines
          if (p.items) {
            setReceiveLines(
              p.items.map((it) => {
                const ordered = Number(it.qty);
                const received = Number(it.qty_received ?? 0);
                const pending = Math.max(0, ordered - received);
                return {
                  purchaseItemId: it.id,
                  productName: it.product_name,
                  qtyOrdered: ordered,
                  qtyReceived: received,
                  qtyPending: pending,
                  qtyToReceive: pending,
                  trackLots: Boolean(it.track_lots),
                  lotCode: "",
                  expiresAt: "",
                };
              }),
            );
          }
        }
      })
      .catch((err) => {
        if (!ignore) setError(friendlyError(err));
      })
      .finally(() => {
        if (!ignore) setLoading(false);
      });

    // Load audit timeline
    api.audit
      .list({ entity: "purchase_receipts", entity_id: purchaseId })
      .then((entries) => {
        if (!ignore) setTimeline(entries);
      })
      .catch(() => {
        // ignore audit failure
      })
      .finally(() => {
        if (!ignore) setLoadingTimeline(false);
      });

    return () => {
      ignore = true;
    };
  }, [open, purchaseId]);

  if (!open) return null;

  async function handleApprove() {
    if (!purchase) return;
    setBusy(true);
    setError(null);
    try {
      await api.purchases.approve(purchase.id);
      onActionDone("Orden de compra aprobada correctamente.");
      onClose();
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleReceiveSubmit() {
    if (!purchase) return;
    setBusy(true);
    setError(null);

    const itemsToSubmit = receiveLines
      .filter((l) => l.qtyToReceive > 0)
      .map((l) => {
        if (l.trackLots && (!l.lotCode.trim() || !l.expiresAt)) {
          throw new Error(`El producto "${l.productName}" requiere número de lote y fecha de vencimiento.`);
        }
        return {
          purchase_item_id: l.purchaseItemId,
          qty: l.qtyToReceive.toFixed(4),
          lot_code: l.lotCode.trim() || null,
          expires_at: l.expiresAt || null,
        };
      });

    if (itemsToSubmit.length === 0) {
      setError("Tenés que ingresar una cantidad a recibir mayor a cero.");
      setBusy(false);
      return;
    }

    try {
      await api.purchases.receive(purchase.id, {
        id: crypto.randomUUID(),
        items: itemsToSubmit,
      });
      onActionDone(`Recepción registrada en inventario de ${branchName}.`);
      onClose();
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCancel() {
    if (!purchase) return;
    setBusy(true);
    setError(null);
    try {
      await api.purchases.cancel(purchase.id);
      onActionDone("La compra fue anulada.");
      onClose();
    } catch (err) {
      setError(friendlyError(err));
      setConfirmingCancel(false);
    } finally {
      setBusy(false);
    }
  }

  const badge = purchase ? STATUS_BADGES[purchase.status] : null;

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-3xl">
        {loading || !purchase ? (
          <div className="flex flex-col items-center justify-center py-12">
            <Loader2 className="size-8 animate-spin text-teal-700" />
            <p className="mt-3 text-sm text-slate-500">Cargando orden de compra...</p>
          </div>
        ) : (
          <div className="space-y-6">
            {/* Cabecera */}
            <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-100 pb-4">
              <div>
                <div className="flex items-center gap-2">
                  <span className="flex size-9 items-center justify-center rounded-xl bg-teal-50 text-teal-700">
                    <FileText className="size-5" />
                  </span>
                  <DialogTitle className="text-xl font-bold tracking-tight text-slate-900">
                    {purchase.document_number
                      ? `Factura / Remisión N.° ${purchase.document_number}`
                      : "Orden de compra sin factura"}
                  </DialogTitle>
                </div>
                <DialogDescription className="mt-1 flex flex-wrap items-center gap-2 text-xs text-slate-500">
                  <span className="flex items-center gap-1">
                    <Calendar className="size-3.5" />
                    Creada: {formatDateTime(purchase.created_at)}
                  </span>
                  {purchase.supplier_invoice_date && (
                    <span>• Factura: {purchase.supplier_invoice_date}</span>
                  )}
                  {purchase.expected_at && (
                    <span>• Entrega esperada: {purchase.expected_at}</span>
                  )}
                  {purchase.payment_terms && (
                    <span className="font-semibold text-slate-700">
                      • {purchase.payment_terms}
                      {purchase.payment_terms === "CREDITO" && purchase.credit_days
                        ? ` (${purchase.credit_days} días)`
                        : ""}
                    </span>
                  )}
                </DialogDescription>
              </div>

              {badge && (
                <span
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-semibold",
                    badge.className,
                  )}
                >
                  <badge.icon className="size-3.5" />
                  {badge.label}
                </span>
              )}
            </div>

            {error && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-700">
                {error}
              </div>
            )}

            {/* Datos del proveedor y notas */}
            <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-slate-500">
                  <Building2 className="size-4" />
                  <span className="text-xs font-semibold uppercase tracking-wider">Proveedor</span>
                </div>
                {purchase.freight_mode && (
                  <span className="text-xs text-slate-500">
                    Flete: <strong>{purchase.freight_mode}</strong> ({formatCOP(Number(purchase.freight_amount || 0))})
                  </span>
                )}
              </div>
              <p className="mt-1 text-base font-semibold text-slate-900">{purchase.supplier_name}</p>
              {purchase.notes && (
                <p className="mt-2 text-xs text-slate-500 border-t border-slate-200/60 pt-2">
                  <span className="font-semibold text-slate-700">Notas:</span> {purchase.notes}
                </p>
              )}
            </div>

            {/* Modal / Sección de Recepción */}
            {isReceivingModal ? (
              <div className="rounded-2xl border border-emerald-200 bg-emerald-50/40 p-4 space-y-4">
                <div className="flex items-center justify-between border-b border-emerald-100 pb-2">
                  <h3 className="text-sm font-bold text-emerald-900 flex items-center gap-2">
                    <PackageCheck className="size-4 text-emerald-700" />
                    Recepción de mercancía
                  </h3>
                  <button
                    type="button"
                    onClick={() => setIsReceivingModal(false)}
                    className="text-xs font-medium text-slate-600 hover:text-slate-900"
                  >
                    Volver a detalle
                  </button>
                </div>
                <p className="text-xs text-slate-600">
                  Ingresá las cantidades a recibir para cada línea. Para productos con control de lotes, completá el lote y vencimiento.
                </p>

                <div className="space-y-3">
                  {receiveLines.map((line, idx) => {
                    if (line.qtyPending <= 0) return null;
                    return (
                      <div
                        key={line.purchaseItemId}
                        className="rounded-xl border border-emerald-200/80 bg-white p-3 space-y-2"
                      >
                        <div className="flex items-center justify-between">
                          <div>
                            <p className="text-sm font-semibold text-slate-900">{line.productName}</p>
                            <p className="text-xs text-slate-500">
                              Pedido: {formatQty(line.qtyOrdered)} • Recibido: {formatQty(line.qtyReceived)} • Pendiente: <strong>{formatQty(line.qtyPending)}</strong>
                            </p>
                          </div>
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-slate-600 font-medium">Recibir:</span>
                            <input
                              type="number"
                              min="0"
                              max={line.qtyPending}
                              step="any"
                              value={line.qtyToReceive || ""}
                              onChange={(e) => {
                                const val = parseFloat(e.target.value) || 0;
                                setReceiveLines((prev) =>
                                  prev.map((l, i) => (i === idx ? { ...l, qtyToReceive: val } : l)),
                                );
                              }}
                              className="w-24 rounded-lg border border-slate-200 px-2 py-1 text-right text-sm font-semibold focus:border-teal-700 focus:outline-none"
                            />
                          </div>
                        </div>

                        {line.trackLots && (
                          <div className="grid grid-cols-2 gap-3 pt-2 border-t border-slate-100">
                            <div>
                              <label className="text-[11px] font-semibold text-slate-600">N.° de Lote *</label>
                              <input
                                type="text"
                                placeholder="Ej. L-202611"
                                value={line.lotCode}
                                onChange={(e) => {
                                  const val = e.target.value;
                                  setReceiveLines((prev) =>
                                    prev.map((l, i) => (i === idx ? { ...l, lotCode: val } : l)),
                                  );
                                }}
                                className="mt-0.5 w-full rounded-lg border border-slate-200 px-2 py-1 text-xs focus:border-teal-700 focus:outline-none"
                              />
                            </div>
                            <div>
                              <label className="text-[11px] font-semibold text-slate-600">Fecha Vencimiento *</label>
                              <input
                                type="date"
                                value={line.expiresAt}
                                onChange={(e) => {
                                  const val = e.target.value;
                                  setReceiveLines((prev) =>
                                    prev.map((l, i) => (i === idx ? { ...l, expiresAt: val } : l)),
                                  );
                                }}
                                className="mt-0.5 w-full rounded-lg border border-slate-200 px-2 py-1 text-xs focus:border-teal-700 focus:outline-none"
                              />
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>

                <div className="flex justify-end gap-2 pt-2 border-t border-emerald-100">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => setIsReceivingModal(false)}
                    className="h-9 rounded-full border border-slate-200 bg-white px-4 text-xs font-medium text-slate-600 hover:bg-slate-50"
                  >
                    Cancelar
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={handleReceiveSubmit}
                    className="flex h-9 items-center gap-1.5 rounded-full bg-emerald-600 px-5 text-xs font-semibold text-white shadow-xs hover:bg-emerald-700 disabled:opacity-50"
                  >
                    {busy && <Loader2 className="size-3.5 animate-spin" />}
                    Confirmar recepción
                  </button>
                </div>
              </div>
            ) : (
              /* Tabla de productos (Pedido vs Recibido vs Pendiente) */
              <div>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Líneas de la compra ({purchase.items?.length ?? 0})
                </h3>
                <div className="mt-2 overflow-x-auto rounded-2xl border border-slate-200 bg-white">
                  <table className="w-full text-left text-sm">
                    <thead className="border-b border-slate-100 bg-slate-50 text-xs font-semibold uppercase text-slate-500">
                      <tr>
                        <th className="px-3 py-3">Producto</th>
                        <th className="px-2 py-3 text-right">Pedido</th>
                        <th className="px-2 py-3 text-right">Recibido</th>
                        <th className="px-2 py-3 text-right">Pendiente</th>
                        <th className="px-3 py-3 text-right">Costo Unit.</th>
                        <th className="px-3 py-3 text-right">Landed Unit.</th>
                        <th className="px-3 py-3 text-right">Subtotal</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {purchase.items?.map((item) => {
                        const ordered = Number(item.qty);
                        const received = Number(item.qty_received ?? 0);
                        const pending = Math.max(0, ordered - received);
                        const lineSubtotal = Number(item.line_total);
                        return (
                          <tr key={item.id} className="hover:bg-slate-50/50">
                            <td className="px-3 py-3">
                              <p className="font-medium text-slate-900">{item.product_name}</p>
                              <p className="text-xs text-slate-400">
                                {item.sku ? `SKU: ${item.sku}` : ""}
                                {item.purchase_unit ? ` • ${item.purchase_unit}` : ""}
                              </p>
                            </td>
                            <td className="px-2 py-3 text-right tabular-nums font-semibold text-slate-800">
                              {formatQty(ordered)}
                            </td>
                            <td className="px-2 py-3 text-right tabular-nums text-emerald-700 font-semibold">
                              {formatQty(received)}
                            </td>
                            <td className="px-2 py-3 text-right tabular-nums text-amber-700 font-semibold">
                              {formatQty(pending)}
                            </td>
                            <td className="px-3 py-3 text-right tabular-nums text-slate-600">
                              {formatCOP(Number(item.unit_cost))}
                            </td>
                            <td className="px-3 py-3 text-right tabular-nums text-teal-800 font-semibold">
                              {item.landed_unit_cost ? formatCOP(Number(item.landed_unit_cost)) : "—"}
                            </td>
                            <td className="px-3 py-3 text-right tabular-nums font-semibold text-slate-900">
                              {formatCOP(lineSubtotal)}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* Totales */}
            <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4 space-y-2 text-sm">
              <div className="flex justify-between text-slate-600">
                <span>Subtotal neto:</span>
                <span className="font-medium tabular-nums">{formatCOP(Number(purchase.subtotal))}</span>
              </div>
              {Number(purchase.discount_amount || 0) > 0 && (
                <div className="flex justify-between text-slate-600">
                  <span>Descuento global:</span>
                  <span className="font-medium tabular-nums text-emerald-700">
                    -{formatCOP(Number(purchase.discount_amount))}
                  </span>
                </div>
              )}
              {Number(purchase.freight_amount || 0) > 0 && (
                <div className="flex justify-between text-slate-600">
                  <span>Flete:</span>
                  <span className="font-medium tabular-nums">+{formatCOP(Number(purchase.freight_amount))}</span>
                </div>
              )}
              <div className="flex justify-between text-slate-600">
                <span>IVA / Impuestos:</span>
                <span className="font-medium tabular-nums">{formatCOP(Number(purchase.tax_amount))}</span>
              </div>
              <div className="flex justify-between border-t border-slate-200 pt-2 text-base font-bold text-slate-900">
                <span>Total orden:</span>
                <span className="tabular-nums text-teal-700">{formatCOP(Number(purchase.total_amount))} COP</span>
              </div>
            </div>

            {/* Historial de Auditoría (Timeline) */}
            <div>
              <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-500 flex items-center gap-1.5 mb-2">
                <History className="size-3.5" />
                Historial de eventos (Línea de tiempo)
              </h3>
              {loadingTimeline ? (
                <div className="flex items-center gap-2 py-2 text-xs text-slate-400">
                  <Loader2 className="size-3 animate-spin" />
                  Cargando bitácora...
                </div>
              ) : timeline.length === 0 ? (
                <p className="text-xs text-slate-400 py-1">No hay eventos registrados aún.</p>
              ) : (
                <div className="relative pl-4 border-l border-slate-200 space-y-3">
                  {timeline.map((entry) => (
                    <div key={entry.id} className="relative text-xs">
                      <div className="absolute -left-[21px] top-1 size-2.5 rounded-full bg-teal-600 border-2 border-white" />
                      <p className="font-semibold text-slate-800">{entry.action}</p>
                      <p className="text-[11px] text-slate-400 flex items-center gap-1">
                        <Clock className="size-3" />
                        {formatDateTime(entry.created_at)}
                      </p>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Confirmación para Anular */}
            {confirmingCancel && (
              <div className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-900">
                <p className="font-semibold flex items-center gap-1.5">
                  <AlertTriangle className="size-4 text-rose-600" />
                  ¿Anular compra?
                </p>
                <p className="mt-1">La orden pasará a estado ANULADA y no podrá ser recibida.</p>
                <div className="mt-3 flex gap-2">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={handleCancel}
                    className="flex h-9 items-center gap-1.5 rounded-full bg-rose-600 px-4 text-xs font-semibold text-white hover:bg-rose-700 disabled:opacity-50"
                  >
                    {busy && <Loader2 className="size-3.5 animate-spin" />}
                    Confirmar anulación
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => setConfirmingCancel(false)}
                    className="h-9 rounded-full border border-rose-300 px-3 text-xs font-medium text-rose-800 hover:bg-rose-100/60"
                  >
                    Regresar
                  </button>
                </div>
              </div>
            )}

            {/* Botones de acción del diálogo */}
            <div className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-4">
              <button
                type="button"
                onClick={onClose}
                className="h-11 rounded-full border border-slate-200 px-5 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cerrar
              </button>

              <div className="flex items-center gap-2">
                {/* Anular permitido en PENDIENTE o APROBADA */}
                {(purchase.status === "PENDIENTE" || purchase.status === "APROBADA") &&
                  !confirmingCancel &&
                  !isReceivingModal && (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => setConfirmingCancel(true)}
                      className="h-11 rounded-full border border-rose-200 px-4 text-sm font-medium text-rose-600 hover:bg-rose-50"
                    >
                      Anular orden
                    </button>
                  )}

                {/* Aprobar en PENDIENTE */}
                {purchase.status === "PENDIENTE" && !isReceivingModal && (
                  <button
                    type="button"
                    disabled={busy}
                    onClick={handleApprove}
                    className="h-11 rounded-full bg-blue-600 px-5 text-sm font-semibold text-white hover:bg-blue-700"
                  >
                    {busy && <Loader2 className="mr-1.5 inline size-4 animate-spin" />}
                    Aprobar orden
                  </button>
                )}

                {/* Recibir en APROBADA o PARCIAL */}
                {(purchase.status === "APROBADA" || purchase.status === "PARCIAL") &&
                  !isReceivingModal && (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => setIsReceivingModal(true)}
                      className="flex h-11 items-center gap-2 rounded-full bg-emerald-600 px-5 text-sm font-semibold text-white shadow-sm hover:bg-emerald-700"
                    >
                      <PackageCheck className="size-4" />
                      Recibir mercancía
                    </button>
                  )}
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

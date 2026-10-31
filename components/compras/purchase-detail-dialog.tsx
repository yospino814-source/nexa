"use client";

import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Building2,
  Calendar,
  CheckCircle2,
  FileText,
  Loader2,
  PackageCheck,
  XCircle,
} from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type Purchase, type PurchaseStatus } from "@/lib/api";
import { formatCOP, formatDateTime, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUS_BADGES: Record<
  PurchaseStatus,
  { label: string; className: string; icon: typeof CheckCircle2 }
> = {
  RECIBIDA: {
    label: "Registrada",
    className: "border-emerald-200 bg-emerald-50 text-emerald-700",
    icon: CheckCircle2,
  },
  PENDIENTE: {
    label: "Pendiente de revisión",
    className: "border-amber-200 bg-amber-50 text-amber-700",
    icon: AlertTriangle,
  },
  ANULADA: {
    label: "Anulada",
    className: "border-slate-200 bg-slate-100 text-slate-500",
    icon: XCircle,
  },
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
  const [confirmingReceive, setConfirmingReceive] = useState(false);
  const [confirmingCancel, setConfirmingCancel] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openKey = open && purchaseId ? purchaseId : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    setPurchase(null);
    setLoading(Boolean(openKey));
    setError(null);
    setConfirmingReceive(false);
    setConfirmingCancel(false);
  }

  useEffect(() => {
    if (!open || !purchaseId) return;
    let ignore = false;
    api.purchases
      .get(purchaseId)
      .then((p) => {
        if (!ignore) setPurchase(p);
      })
      .catch((err) => {
        if (!ignore) setError(friendlyError(err));
      })
      .finally(() => {
        if (!ignore) setLoading(false);
      });

    return () => {
      ignore = true;
    };
  }, [open, purchaseId]);

  if (!open) return null;

  const totalUnits =
    purchase?.items?.reduce((sum, it) => sum + Number(it.qty), 0) ?? 0;

  async function handleReceive() {
    if (!purchase) return;
    setBusy(true);
    setError(null);
    try {
      await api.purchases.receive(purchase.id);
      onActionDone(
        `Compra recibida e ingresada al inventario de ${branchName}.`,
      );
      onClose();
    } catch (err) {
      setError(friendlyError(err));
      setConfirmingReceive(false);
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
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-2xl">
        {loading || !purchase ? (
          <div className="flex flex-col items-center justify-center py-12">
            <Loader2 className="size-8 animate-spin text-teal-700" />
            <p className="mt-3 text-sm text-slate-500">Cargando compra...</p>
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
                      ? `Factura N.° ${purchase.document_number}`
                      : "Compra sin factura"}
                  </DialogTitle>
                </div>
                <DialogDescription className="mt-1 flex items-center gap-2 text-xs text-slate-500">
                  <Calendar className="size-3.5" />
                  Fecha de registro: {formatDateTime(purchase.created_at)}
                  {purchase.supplier_invoice_date && (
                    <span>• Factura fecha: {purchase.supplier_invoice_date}</span>
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

            {/* Datos del proveedor */}
            <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
              <div className="flex items-center gap-2 text-slate-500">
                <Building2 className="size-4" />
                <span className="text-xs font-semibold uppercase tracking-wider">
                  Proveedor
                </span>
              </div>
              <p className="mt-1 text-base font-semibold text-slate-900">
                {purchase.supplier_name}
              </p>
              {purchase.notes && (
                <p className="mt-2 text-xs text-slate-500 border-t border-slate-200/60 pt-2">
                  <span className="font-semibold text-slate-700">Nota:</span>{" "}
                  {purchase.notes}
                </p>
              )}
            </div>

            {/* Tabla de productos */}
            <div>
              <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Líneas de la compra ({purchase.items?.length ?? 0})
              </h3>
              <div className="mt-2 overflow-hidden rounded-2xl border border-slate-200 bg-white">
                <table className="w-full text-left text-sm">
                  <thead className="border-b border-slate-100 bg-slate-50 text-xs font-semibold uppercase text-slate-500">
                    <tr>
                      <th className="px-4 py-3">Producto</th>
                      <th className="px-4 py-3 text-right">Cant.</th>
                      <th className="px-4 py-3 text-right">Costo unitario</th>
                      <th className="px-4 py-3 text-right">Subtotal</th>
                      <th className="px-4 py-3 text-right">Total c/IVA</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {purchase.items?.map((item) => {
                      const lineSubtotal = Number(item.line_total);
                      const rate = item.tax_rate ? Number(item.tax_rate) : 0;
                      const lineTotal = lineSubtotal * (1 + rate);
                      return (
                        <tr key={item.id} className="hover:bg-slate-50/50">
                          <td className="px-4 py-3">
                            <p className="font-medium text-slate-900">
                              {item.product_name}
                            </p>
                            {item.sku && (
                              <p className="font-mono text-xs text-slate-400">
                                {item.sku}
                              </p>
                            )}
                          </td>
                          <td className="px-4 py-3 text-right tabular-nums font-semibold text-slate-800">
                            {formatQty(item.qty)}
                          </td>
                          <td className="px-4 py-3 text-right tabular-nums text-slate-600">
                            {formatCOP(Number(item.unit_cost))}
                          </td>
                          <td className="px-4 py-3 text-right tabular-nums text-slate-600">
                            {formatCOP(lineSubtotal)}
                          </td>
                          <td className="px-4 py-3 text-right tabular-nums font-semibold text-slate-900">
                            {formatCOP(lineTotal)}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>

            {/* Totales */}
            <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4 space-y-2 text-sm">
              <div className="flex justify-between text-slate-600">
                <span>Subtotal (sin IVA):</span>
                <span className="font-medium tabular-nums">
                  {formatCOP(Number(purchase.subtotal))}
                </span>
              </div>
              <div className="flex justify-between text-slate-600">
                <span>IVA acumulado:</span>
                <span className="font-medium tabular-nums">
                  {formatCOP(Number(purchase.tax_amount))}
                </span>
              </div>
              <div className="flex justify-between border-t border-slate-200 pt-2 text-base font-bold text-slate-900">
                <span>Total de la factura:</span>
                <span className="tabular-nums text-teal-700">
                  {formatCOP(Number(purchase.total_amount))} COP
                </span>
              </div>
            </div>

            {/* Confirmación para Recibir Mercancía */}
            {confirmingReceive && (
              <div className="rounded-2xl border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
                <p className="font-semibold flex items-center gap-1.5">
                  <AlertTriangle className="size-4 text-amber-600" />
                  Confirmar ingreso a inventario
                </p>
                <p className="mt-1">
                  Esto suma {formatQty(totalUnits)} unidades al inventario de{" "}
                  <strong>{branchName}</strong> y actualiza el último costo de los
                  productos.
                </p>
                <div className="mt-3 flex gap-2">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={handleReceive}
                    className="flex h-9 items-center gap-1.5 rounded-full bg-emerald-600 px-4 text-xs font-semibold text-white hover:bg-emerald-700 disabled:opacity-50"
                  >
                    {busy && <Loader2 className="size-3.5 animate-spin" />}
                    Confirmar y recibir
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => setConfirmingReceive(false)}
                    className="h-9 rounded-full border border-amber-300 px-3 text-xs font-medium text-amber-800 hover:bg-amber-100/60"
                  >
                    Regresar
                  </button>
                </div>
              </div>
            )}

            {/* Confirmación para Anular */}
            {confirmingCancel && (
              <div className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-900">
                <p className="font-semibold flex items-center gap-1.5">
                  <AlertTriangle className="size-4 text-rose-600" />
                  ¿Anular compra pendiente?
                </p>
                <p className="mt-1">
                  La compra quedará registrada como anulada y no podrá ser recibida
                  posteriormente.
                </p>
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

              {purchase.status === "PENDIENTE" &&
                !confirmingReceive &&
                !confirmingCancel && (
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => setConfirmingCancel(true)}
                      className="h-11 rounded-full border border-rose-200 px-4 text-sm font-medium text-rose-600 hover:bg-rose-50"
                    >
                      Anular compra
                    </button>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => setConfirmingReceive(true)}
                      className="flex h-11 items-center gap-2 rounded-full bg-emerald-600 px-5 text-sm font-semibold text-white shadow-sm hover:bg-emerald-700"
                    >
                      <PackageCheck className="size-4" />
                      Recibir mercancía
                    </button>
                  </div>
                )}
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

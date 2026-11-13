"use client";

import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  AlertTriangle,
  Building2,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Loader2,
  Package,
  Sparkles,
} from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type PurchaseSuggestionGroup,
  type Uuid,
} from "@/lib/api";
import { formatCOP, formatQty } from "@/lib/format";
import { useCurrentBranch } from "@/lib/stores/session";

interface PurchaseSuggestionsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOrdersCreated: (count: number) => void;
}

export function PurchaseSuggestionsDialog({
  open,
  onOpenChange,
  onOrdersCreated,
}: PurchaseSuggestionsDialogProps) {
  const branch = useCurrentBranch();

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [groups, setGroups] = useState<PurchaseSuggestionGroup[]>([]);
  const [selectedSupplierIds, setSelectedSupplierIds] = useState<Set<Uuid>>(new Set());
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const openKey = open ? branch.id : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    setGroups([]);
    setSelectedSupplierIds(new Set());
    setCollapsedGroups(new Set());
    setLoading(Boolean(openKey));
    setError(null);
    setSubmitError(null);
  }

  useEffect(() => {
    if (!open) return;

    let active = true;

    api.purchases
      .suggestions(branch.id)
      .then((data) => {
        if (!active) return;
        setGroups(data);
        // By default, select all suppliers that are not null
        const allIds = new Set<Uuid>();
        for (const g of data) {
          if (g.supplier?.id) {
            allIds.add(g.supplier.id);
          }
        }
        setSelectedSupplierIds(allIds);
      })
      .catch((err) => {
        if (!active) return;
        setError(friendlyError(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [open, branch.id]);

  function toggleSupplier(supplierId: Uuid) {
    setSelectedSupplierIds((prev) => {
      const next = new Set(prev);
      if (next.has(supplierId)) {
        next.delete(supplierId);
      } else {
        next.add(supplierId);
      }
      return next;
    });
  }

  function toggleSelectAll() {
    const validSupplierIds = groups
      .map((g) => g.supplier?.id)
      .filter((id): id is Uuid => Boolean(id));

    if (selectedSupplierIds.size === validSupplierIds.length) {
      setSelectedSupplierIds(new Set());
    } else {
      setSelectedSupplierIds(new Set(validSupplierIds));
    }
  }

  function toggleCollapse(groupId: string) {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }

  // Calculate totals
  const totalItemsCount = useMemo(() => {
    return groups.reduce((acc, g) => acc + g.items.length, 0);
  }, [groups]);

  const selectedStats = useMemo(() => {
    let orderCount = 0;
    let itemsCount = 0;
    let totalEstimated = 0;

    for (const g of groups) {
      if (g.supplier?.id && selectedSupplierIds.has(g.supplier.id)) {
        orderCount += 1;
        itemsCount += g.items.length;
        for (const item of g.items) {
          const cost = Number(item.last_cost ?? 0);
          const qty = Number(item.suggested_qty);
          totalEstimated += cost * qty;
        }
      }
    }

    return { orderCount, itemsCount, totalEstimated };
  }, [groups, selectedSupplierIds]);

  async function handleCreateOrders() {
    if (selectedSupplierIds.size === 0) return;
    setSubmitting(true);
    setSubmitError(null);

    try {
      const ids = await api.purchases.createFromSuggestions({
        branch_id: branch.id,
        supplier_ids: Array.from(selectedSupplierIds),
      });
      onOrdersCreated(ids.length);
      onOpenChange(false);
    } catch (err) {
      setSubmitError(friendlyError(err));
    } finally {
      setSubmitting(false);
    }
  }

  const validSuppliers = groups.filter((g) => g.supplier !== null);
  const unassignedGroup = groups.find((g) => g.supplier === null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col p-0 gap-0 overflow-hidden sm:rounded-2xl">
        {/* Header */}
        <div className="border-b border-slate-200 px-6 py-5 bg-gradient-to-r from-teal-50/50 to-white">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-xl bg-teal-100 text-teal-700">
              <Sparkles className="size-5" />
            </div>
            <div>
              <DialogTitle className="text-xl font-bold text-slate-900 font-headline">
                Sugerencias de reabastecimiento
              </DialogTitle>
              <DialogDescription className="text-sm text-slate-500">
                Productos que han alcanzado su punto de reorden o stock mínimo en {branch.name}.
              </DialogDescription>
            </div>
          </div>
        </div>

        {/* Content Body */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {loading && (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <Loader2 className="size-8 animate-spin text-teal-600 mb-3" />
              <p className="text-sm font-medium text-slate-700">Analizando existencias y proveedores...</p>
              <p className="text-xs text-slate-400 mt-1">Calculando punto de reorden y factores de compra</p>
            </div>
          )}

          {error && !loading && (
            <div className="rounded-xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700 flex items-start gap-3">
              <AlertCircle className="size-5 shrink-0 text-rose-500 mt-0.5" />
              <div>
                <p className="font-semibold">No se pudieron cargar las sugerencias</p>
                <p className="mt-1 text-xs text-rose-600">{error}</p>
              </div>
            </div>
          )}

          {!loading && !error && groups.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <div className="flex size-12 items-center justify-center rounded-full bg-emerald-100 text-emerald-600 mb-4">
                <CheckCircle2 className="size-6" />
              </div>
              <h3 className="text-base font-semibold text-slate-900">¡Inventario en orden!</h3>
              <p className="mt-1 max-w-sm text-sm text-slate-500">
                No hay productos por debajo del stock mínimo ni con necesidad de reorden en esta sucursal.
              </p>
            </div>
          )}

          {!loading && !error && groups.length > 0 && (
            <>
              {/* Summary Toolbar */}
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-slate-50 p-4 border border-slate-200">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-slate-700">
                    {totalItemsCount} {totalItemsCount === 1 ? "producto requiere" : "productos requieren"} reposición
                  </span>
                  <span className="text-slate-300">•</span>
                  <span className="text-xs text-slate-500">
                    {validSuppliers.length} {validSuppliers.length === 1 ? "proveedor sugerido" : "proveedores sugeridos"}
                  </span>
                </div>

                {validSuppliers.length > 0 && (
                  <button
                    type="button"
                    onClick={toggleSelectAll}
                    className="text-xs font-semibold text-teal-700 hover:text-teal-800"
                  >
                    {selectedSupplierIds.size === validSuppliers.length
                      ? "Deseleccionar todos"
                      : "Seleccionar todos"}
                  </button>
                )}
              </div>

              {/* Suppliers Groups */}
              <div className="space-y-4">
                {validSuppliers.map((g) => {
                  const sup = g.supplier!;
                  const isSelected = selectedSupplierIds.has(sup.id);
                  const isCollapsed = collapsedGroups.has(sup.id);

                  const groupSubtotal = g.items.reduce(
                    (acc, it) => acc + Number(it.suggested_qty) * Number(it.last_cost ?? 0),
                    0,
                  );

                  return (
                    <div
                      key={sup.id}
                      className={`rounded-2xl border transition-all ${
                        isSelected
                          ? "border-teal-300 bg-white shadow-xs"
                          : "border-slate-200 bg-slate-50/50 opacity-75"
                      }`}
                    >
                      {/* Supplier Card Header */}
                      <div className="flex items-center justify-between p-4 gap-3">
                        <div className="flex items-center gap-3">
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => toggleSupplier(sup.id)}
                            className="size-4.5 rounded border-slate-300 text-teal-600 focus:ring-teal-500 cursor-pointer"
                          />
                          <div className="flex items-center gap-2">
                            <Building2 className="size-4 text-slate-400" />
                            <span className="text-sm font-bold text-slate-900">{sup.name}</span>
                            <span className="rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600">
                              {g.items.length} {g.items.length === 1 ? "ítem" : "ítems"}
                            </span>
                          </div>
                        </div>

                        <div className="flex items-center gap-4">
                          <div className="text-right">
                            <span className="text-xs text-slate-400 block">Subtotal est.</span>
                            <span className="text-sm font-semibold text-slate-900">
                              {formatCOP(groupSubtotal)}
                            </span>
                          </div>

                          <button
                            type="button"
                            onClick={() => toggleCollapse(sup.id)}
                            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
                            title={isCollapsed ? "Ver detalles" : "Ocultar detalles"}
                          >
                            {isCollapsed ? (
                              <ChevronRight className="size-4.5" />
                            ) : (
                              <ChevronDown className="size-4.5" />
                            )}
                          </button>
                        </div>
                      </div>

                      {/* Items Table */}
                      {!isCollapsed && (
                        <div className="border-t border-slate-100 overflow-x-auto">
                          <table className="w-full text-left text-xs">
                            <thead className="bg-slate-50/70 text-slate-500 uppercase tracking-wider font-semibold">
                              <tr>
                                <th className="px-4 py-2.5">Producto</th>
                                <th className="px-3 py-2.5 text-right">Existencia</th>
                                <th className="px-3 py-2.5 text-right">Mínimo</th>
                                <th className="px-3 py-2.5 text-right">Sugerido</th>
                                <th className="px-3 py-2.5 text-right">Último costo</th>
                                <th className="px-4 py-2.5 text-right">Subtotal</th>
                              </tr>
                            </thead>
                            <tbody className="divide-y divide-slate-100">
                              {g.items.map((it) => {
                                const cost = Number(it.last_cost ?? 0);
                                const qty = Number(it.suggested_qty);
                                const lineTotal = cost * qty;

                                return (
                                  <tr key={it.product_id} className="hover:bg-slate-50/50">
                                    <td className="px-4 py-2.5 font-medium text-slate-800">
                                      {it.name}
                                    </td>
                                    <td className="px-3 py-2.5 text-right text-slate-600 font-mono">
                                      {formatQty(it.on_hand)}
                                    </td>
                                    <td className="px-3 py-2.5 text-right text-amber-700 font-mono font-medium">
                                      {formatQty(it.min_qty)}
                                    </td>
                                    <td className="px-3 py-2.5 text-right font-mono font-bold text-teal-800">
                                      {formatQty(it.suggested_qty)}{" "}
                                      <span className="text-[10px] text-teal-600 font-sans font-normal uppercase">
                                        {it.purchase_uom ?? "UND"}
                                      </span>
                                    </td>
                                    <td className="px-3 py-2.5 text-right text-slate-600 font-mono">
                                      {it.last_cost ? formatCOP(cost) : "—"}
                                    </td>
                                    <td className="px-4 py-2.5 text-right font-medium text-slate-900 font-mono">
                                      {formatCOP(lineTotal)}
                                    </td>
                                  </tr>
                                );
                              })}
                            </tbody>
                          </table>
                        </div>
                      )}
                    </div>
                  );
                })}

                {/* Items without preferred supplier */}
                {unassignedGroup && unassignedGroup.items.length > 0 && (
                  <div className="rounded-2xl border border-amber-200 bg-amber-50/30 p-4 space-y-3">
                    <div className="flex items-center gap-2 text-amber-800 font-semibold text-sm">
                      <AlertTriangle className="size-4.5 text-amber-600" />
                      <span>Sin proveedor preferido asignado ({unassignedGroup.items.length})</span>
                    </div>
                    <p className="text-xs text-slate-600">
                      Los siguientes productos están bajo el mínimo pero no tienen un proveedor preferido activo.
                      Asigna un proveedor en la ficha del producto o catálogo de proveedores para incluirlos en órdenes automáticas.
                    </p>

                    <div className="rounded-xl border border-amber-200/60 bg-white overflow-hidden">
                      <table className="w-full text-left text-xs">
                        <thead className="bg-amber-50/50 text-slate-500 font-semibold uppercase">
                          <tr>
                            <th className="px-3 py-2">Producto</th>
                            <th className="px-3 py-2 text-right">Existencia</th>
                            <th className="px-3 py-2 text-right">Mínimo</th>
                            <th className="px-3 py-2 text-right">Sugerido</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-100">
                          {unassignedGroup.items.map((it) => (
                            <tr key={it.product_id}>
                              <td className="px-3 py-2 font-medium text-slate-800">{it.name}</td>
                              <td className="px-3 py-2 text-right font-mono text-slate-600">
                                {formatQty(it.on_hand)}
                              </td>
                              <td className="px-3 py-2 text-right font-mono text-amber-700">
                                {formatQty(it.min_qty)}
                              </td>
                              <td className="px-3 py-2 text-right font-mono font-semibold text-slate-900">
                                {formatQty(it.suggested_qty)}{" "}
                                <span className="text-[10px] text-slate-500 font-sans font-normal uppercase">
                                  {it.purchase_uom ?? "UND"}
                                </span>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}
              </div>
            </>
          )}

          {submitError && (
            <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700 flex items-center gap-2">
              <AlertCircle className="size-4 text-rose-500 shrink-0" />
              <span>{submitError}</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-slate-200 bg-slate-50 px-6 py-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-4">
            <div className="text-xs text-slate-600">
              <span className="font-semibold text-slate-900">{selectedStats.orderCount}</span>{" "}
              {selectedStats.orderCount === 1 ? "orden a generar" : "órdenes a generar"} (
              <span className="font-semibold text-slate-900">{selectedStats.itemsCount}</span> ítems)
            </div>
            {selectedStats.orderCount > 0 && (
              <div className="text-xs text-slate-600">
                Total estimado:{" "}
                <span className="font-bold text-slate-900 font-mono text-sm">
                  {formatCOP(selectedStats.totalEstimated)}
                </span>
              </div>
            )}
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              disabled={submitting}
              className="rounded-full border border-slate-200 px-4 py-2 text-xs font-medium text-slate-600 hover:bg-white transition-colors"
            >
              Cerrar
            </button>

            <button
              type="button"
              onClick={handleCreateOrders}
              disabled={submitting || selectedStats.orderCount === 0}
              className="flex items-center gap-2 rounded-full bg-teal-700 px-5 py-2 text-xs font-semibold text-white shadow-sm hover:bg-teal-800 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {submitting ? (
                <>
                  <Loader2 className="size-3.5 animate-spin" />
                  <span>Creando órdenes...</span>
                </>
              ) : (
                <>
                  <Package className="size-3.5" />
                  <span>
                    Crear {selectedStats.orderCount}{" "}
                    {selectedStats.orderCount === 1 ? "orden" : "órdenes"}
                  </span>
                </>
              )}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

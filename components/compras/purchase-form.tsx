"use client";

import { useEffect, useMemo, useState } from "react";
import {
  FilePlus2,
  Loader2,
  PackageCheck,
  Search,
  Trash2,
} from "lucide-react";

import { MoneyInput } from "@/components/caja/money-input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type CatalogItem,
  type StockLevel,
  type Supplier,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";

type FormLine = {
  productId: string;
  name: string;
  sku: string | null;
  taxCode: string | null;
  taxRate: number;
  qty: number;
  unitCost: number;
};

export function PurchaseForm({
  branchId,
  branchName,
  open,
  onClose,
  onSaved,
}: {
  branchId: string;
  branchName: string;
  open: boolean;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  // Idempotency UUID fixed per form opening
  const [purchaseId, setPurchaseId] = useState("");

  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [catalogItems, setCatalogItems] = useState<CatalogItem[]>([]);
  const [stockLevels, setStockLevels] = useState<StockLevel[]>([]);
  const [loadingData, setLoadingData] = useState(false);

  // Form fields
  const [selectedSupplierId, setSelectedSupplierId] = useState("");
  const [documentNumber, setDocumentNumber] = useState("");
  const [invoiceDate, setInvoiceDate] = useState("");
  const [notes, setNotes] = useState("");
  const [lines, setLines] = useState<FormLine[]>([]);

  // Product search state
  const [productSearch, setProductSearch] = useState("");
  const [showProductDropdown, setShowProductDropdown] = useState(false);

  // Mini inline new supplier form
  const [showNewSupplierInline, setShowNewSupplierInline] = useState(false);
  const [newSupplierName, setNewSupplierName] = useState("");
  const [newSupplierTaxId, setNewSupplierTaxId] = useState("");
  const [newSupplierPhone, setNewSupplierPhone] = useState("");
  const [savingSupplier, setSavingSupplier] = useState(false);

  // Submit states
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Initialize or reset on open
  const openKey = open ? `open:${branchId}` : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    if (open) {
      setPurchaseId(crypto.randomUUID());
      setSelectedSupplierId("");
      setDocumentNumber("");
      setInvoiceDate(new Date().toISOString().split("T")[0]);
      setNotes("");
      setLines([]);
      setProductSearch("");
      setShowNewSupplierInline(false);
      setNewSupplierName("");
      setNewSupplierTaxId("");
      setNewSupplierPhone("");
      setError(null);
      setLoadingData(true);
    }
  }

  useEffect(() => {
    if (!open) return;
    let ignore = false;
    Promise.all([
      api.suppliers.list(undefined, false),
      api.catalog.list(branchId),
      api.inventory.stock(branchId).catch(() => []),
    ])
      .then(([supList, catList, stkList]) => {
        if (!ignore) {
          setSuppliers(supList);
          setCatalogItems(catList);
          setStockLevels(stkList);

          const isSample = typeof window !== "undefined" && new URLSearchParams(window.location.search).has("sample");
          if (isSample && supList.length > 0 && catList.length > 1) {
            setSelectedSupplierId(supList[0].id);
            setDocumentNumber("FAC-10492");
            setLines([
              {
                productId: catList[0].product_id,
                name: catList[0].name,
                sku: null,
                taxCode: catList[0].tax_code,
                taxRate: catList[0].tax_code === "IVA19" ? 0.19 : 0,
                qty: 24,
                unitCost: 2150,
              },
              {
                productId: catList[1].product_id,
                name: catList[1].name,
                sku: null,
                taxCode: catList[1].tax_code,
                taxRate: catList[1].tax_code === "IVA19" ? 0.19 : 0,
                qty: 50,
                unitCost: 1980,
              },
            ]);
          }
        }
      })
      .catch((err) => {
        if (!ignore) setError(friendlyError(err));
      })
      .finally(() => {
        if (!ignore) setLoadingData(false);
      });

    return () => {
      ignore = true;
    };
  }, [open, branchId]);

  // Map product id to cost_price from stockLevels
  const costMap = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of stockLevels) {
      if (item.cost_price) {
        map.set(item.product_id, Number(item.cost_price));
      }
    }
    return map;
  }, [stockLevels]);

  // Filter catalog items for product search
  const filteredProducts = useMemo(() => {
    const q = productSearch.trim().toLowerCase();
    if (!q) return [];
    return catalogItems
      .filter((p) => {
        const matchesName = p.name.toLowerCase().includes(q);
        const stockItem = stockLevels.find((s) => s.product_id === p.product_id);
        const matchesSku = stockItem?.sku?.toLowerCase().includes(q) ?? false;
        const matchesBarcode = stockItem?.barcode?.toLowerCase().includes(q) ?? false;
        return matchesName || matchesSku || matchesBarcode;
      })
      .slice(0, 8);
  }, [catalogItems, stockLevels, productSearch]);

  if (!open) return null;

  function addProductToLines(p: CatalogItem) {
    if (lines.some((l) => l.productId === p.product_id)) {
      setProductSearch("");
      setShowProductDropdown(false);
      return;
    }

    const stockItem = stockLevels.find((s) => s.product_id === p.product_id);
    const lastCost = costMap.get(p.product_id) ?? 0;
    const rate = p.tax_code === "IVA19" ? 0.19 : 0;

    setLines((prev) => [
      ...prev,
      {
        productId: p.product_id,
        name: p.name,
        sku: stockItem?.sku ?? null,
        taxCode: p.tax_code,
        taxRate: rate,
        qty: 1,
        unitCost: lastCost,
      },
    ]);

    setProductSearch("");
    setShowProductDropdown(false);
  }

  function updateLineQty(index: number, qty: number) {
    setLines((prev) =>
      prev.map((l, i) => (i === index ? { ...l, qty: Math.max(0, qty) } : l)),
    );
  }

  function updateLineCost(index: number, unitCost: number) {
    setLines((prev) =>
      prev.map((l, i) => (i === index ? { ...l, unitCost: Math.max(0, unitCost) } : l)),
    );
  }

  function removeLine(index: number) {
    setLines((prev) => prev.filter((_, i) => i !== index));
  }

  // Quick inline create supplier
  async function handleCreateSupplierInline() {
    if (!newSupplierName.trim()) {
      setError("Escribí el nombre del nuevo proveedor.");
      return;
    }
    setSavingSupplier(true);
    setError(null);
    try {
      const created = await api.suppliers.create({
        name: newSupplierName.trim(),
        tax_id: newSupplierTaxId.trim() || null,
        phone: newSupplierPhone.trim() || null,
      });
      setSuppliers((prev) => [created, ...prev]);
      setSelectedSupplierId(created.id);
      setShowNewSupplierInline(false);
      setNewSupplierName("");
      setNewSupplierTaxId("");
      setNewSupplierPhone("");
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setSavingSupplier(false);
    }
  }

  // Totals
  const subtotal = lines.reduce((acc, l) => acc + l.qty * l.unitCost, 0);
  const taxAmount = lines.reduce(
    (acc, l) => acc + l.qty * l.unitCost * l.taxRate,
    0,
  );
  const total = subtotal + taxAmount;

  async function handleSave(receiveImmediately = false) {
    if (!selectedSupplierId) {
      setError("Elegí un proveedor para la compra.");
      return;
    }
    if (lines.length === 0) {
      setError("Agregá al menos un producto a la compra.");
      return;
    }
    for (const l of lines) {
      if (l.qty <= 0) {
        setError(`La cantidad de "${l.name}" debe ser mayor a 0.`);
        return;
      }
      if (l.unitCost < 0) {
        setError(`El costo unitario de "${l.name}" no puede ser negativo.`);
        return;
      }
    }

    setBusy(true);
    setError(null);

    const body = {
      id: purchaseId,
      branch_id: branchId,
      supplier_id: selectedSupplierId,
      document_number: documentNumber.trim() || null,
      supplier_invoice_date: invoiceDate || null,
      notes: notes.trim() || null,
      items: lines.map((l) => ({
        product_id: l.productId,
        qty: l.qty.toFixed(4),
        unit_cost: l.unitCost.toFixed(2),
      })),
    };

    try {
      await api.purchases.create(body);
      if (receiveImmediately) {
        await api.purchases.receive(purchaseId);
        onSaved(`Compra registrada e ingresada al inventario de ${branchName}.`);
      } else {
        onSaved("Compra guardada como pendiente de revisión.");
      }
      onClose();
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-h-[94vh] overflow-y-auto rounded-3xl p-6 sm:max-w-3xl">
        {/* Header */}
        <div className="flex items-start gap-3 border-b border-slate-100 pb-4">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-teal-50 text-teal-700">
            <FilePlus2 className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-xl font-bold tracking-tight text-slate-900">
              Registrar compra
            </DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              Ingresá la factura del proveedor con sus cantidades y costos de adquisición.
            </DialogDescription>
          </div>
        </div>

        {error && (
          <div className="mt-4 rounded-xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-700">
            {error}
          </div>
        )}

        {loadingData ? (
          <div className="flex flex-col items-center justify-center py-10">
            <Loader2 className="size-7 animate-spin text-teal-700" />
            <p className="mt-2 text-sm text-slate-500">Cargando catálogo y proveedores...</p>
          </div>
        ) : (
          <div className="mt-4 space-y-5">
            {/* Proveedor y Datos de Factura */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              {/* Selector de Proveedor */}
              <div className="sm:col-span-1">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    Proveedor *
                  </label>
                  {!showNewSupplierInline && (
                    <button
                      type="button"
                      onClick={() => setShowNewSupplierInline(true)}
                      className="text-xs font-semibold text-teal-700 hover:text-teal-800"
                    >
                      + Nuevo
                    </button>
                  )}
                </div>

                {showNewSupplierInline ? (
                  <div className="mt-2 space-y-2 rounded-xl border border-teal-200 bg-teal-50/50 p-3">
                    <p className="text-xs font-bold text-teal-800">Crear proveedor rápido</p>
                    <input
                      type="text"
                      placeholder="Nombre o empresa *"
                      value={newSupplierName}
                      onChange={(e) => setNewSupplierName(e.target.value)}
                      className="w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs focus:outline-none focus:border-teal-700"
                    />
                    <input
                      type="text"
                      placeholder="NIT (opcional)"
                      value={newSupplierTaxId}
                      onChange={(e) => setNewSupplierTaxId(e.target.value)}
                      className="w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs focus:outline-none focus:border-teal-700"
                    />
                    <input
                      type="text"
                      placeholder="Teléfono (opcional)"
                      value={newSupplierPhone}
                      onChange={(e) => setNewSupplierPhone(e.target.value)}
                      className="w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs focus:outline-none focus:border-teal-700"
                    />
                    <div className="flex gap-2 pt-1">
                      <button
                        type="button"
                        disabled={savingSupplier}
                        onClick={handleCreateSupplierInline}
                        className="rounded-full bg-teal-700 px-3 py-1 text-[11px] font-semibold text-white hover:bg-teal-800 disabled:opacity-50"
                      >
                        {savingSupplier ? "Guardando..." : "Guardar"}
                      </button>
                      <button
                        type="button"
                        onClick={() => setShowNewSupplierInline(false)}
                        className="rounded-full border border-slate-200 bg-white px-2.5 py-1 text-[11px] font-medium text-slate-600 hover:bg-slate-50"
                      >
                        Cancelar
                      </button>
                    </div>
                  </div>
                ) : (
                  <select
                    value={selectedSupplierId}
                    onChange={(e) => setSelectedSupplierId(e.target.value)}
                    className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3.5 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                  >
                    <option value="">Elegí un proveedor...</option>
                    {suppliers.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name} {s.tax_id ? `(${s.tax_id})` : ""}
                      </option>
                    ))}
                  </select>
                )}
              </div>

              {/* N.° de Factura */}
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  N.° de Factura o Remisión
                </label>
                <input
                  type="text"
                  placeholder="Ej. FAC-10492"
                  value={documentNumber}
                  onChange={(e) => setDocumentNumber(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>

              {/* Fecha de Factura */}
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Fecha de factura
                </label>
                <input
                  type="date"
                  value={invoiceDate}
                  onChange={(e) => setInvoiceDate(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>
            </div>

            {/* Buscador de productos para agregar líneas */}
            <div className="relative">
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Agregar productos a la compra
              </label>
              <div className="relative mt-1">
                <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 size-4 text-slate-400" />
                <input
                  type="text"
                  placeholder="Buscá por nombre, código de barras o SKU para agregar..."
                  value={productSearch}
                  onFocus={() => setShowProductDropdown(true)}
                  onChange={(e) => {
                    setProductSearch(e.target.value);
                    setShowProductDropdown(true);
                  }}
                  className="w-full rounded-xl border border-slate-200 bg-slate-50/60 pl-10 pr-4 py-2 text-sm text-slate-800 placeholder-slate-400 focus:bg-white focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>

              {showProductDropdown && filteredProducts.length > 0 && (
                <div className="absolute left-0 right-0 z-20 mt-1 max-h-56 overflow-y-auto rounded-2xl border border-slate-200 bg-white p-1.5 shadow-lg">
                  {filteredProducts.map((p) => {
                    const stockItem = stockLevels.find((s) => s.product_id === p.product_id);
                    const lastCost = costMap.get(p.product_id);
                    return (
                      <button
                        key={p.product_id}
                        type="button"
                        onClick={() => addProductToLines(p)}
                        className="flex w-full items-center justify-between rounded-xl px-3 py-2 text-left text-sm hover:bg-slate-50 transition-colors"
                      >
                        <div>
                          <p className="font-medium text-slate-900">{p.name}</p>
                          <p className="text-xs text-slate-400">
                            {stockItem?.sku ? `SKU: ${stockItem.sku}` : p.category ?? "Sin categoría"}
                            {p.tax_code === "IVA19" ? " • IVA 19%" : " • Exento"}
                          </p>
                        </div>
                        <div className="text-right">
                          <span className="text-xs text-slate-500">Último costo:</span>{" "}
                          <span className="font-semibold text-slate-800">
                            {lastCost != null ? formatCOP(lastCost) : "Sin costo"}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Tabla de Líneas */}
            <div>
              <div className="flex items-center justify-between pb-1">
                <span className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Líneas de la compra ({lines.length})
                </span>
                {lines.length > 0 && (
                  <span className="text-xs text-slate-400">
                    Precios y costos van sin IVA (el impuesto se calcula por línea)
                  </span>
                )}
              </div>

              {lines.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-slate-200 bg-slate-50/50 py-8 text-center text-sm text-slate-400">
                  Buscá y seleccioná productos arriba para agregarlos a la compra.
                </div>
              ) : (
                <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-xs">
                  <table className="w-full text-left text-sm">
                    <thead className="border-b border-slate-100 bg-slate-50 text-xs font-semibold uppercase text-slate-500">
                      <tr>
                        <th className="px-4 py-2.5">Producto</th>
                        <th className="px-3 py-2.5 text-right w-28">Cantidad</th>
                        <th className="px-4 py-2.5 text-right w-44">Costo unitario</th>
                        <th className="px-4 py-2.5 text-right w-36">Subtotal</th>
                        <th className="px-3 py-2.5 text-center w-12"></th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {lines.map((line, idx) => {
                        const lineSub = line.qty * line.unitCost;
                        const lineTax = lineSub * line.taxRate;
                        return (
                          <tr key={line.productId} className="hover:bg-slate-50/50">
                            <td className="px-4 py-3">
                              <p className="font-medium text-slate-900">{line.name}</p>
                              <p className="text-xs text-slate-400">
                                {line.sku ? `SKU: ${line.sku}` : ""}
                                {line.taxRate > 0 ? " • IVA 19%" : " • Exento"}
                              </p>
                            </td>
                            <td className="px-3 py-3 text-right">
                              <input
                                type="number"
                                min="0.001"
                                step="any"
                                value={line.qty || ""}
                                onChange={(e) =>
                                  updateLineQty(idx, parseFloat(e.target.value) || 0)
                                }
                                className="w-20 rounded-xl border border-slate-200 px-2.5 py-1.5 text-right text-sm font-semibold text-slate-800 focus:border-teal-700 focus:outline-none"
                              />
                            </td>
                            <td className="px-4 py-3 text-right">
                              <MoneyInput
                                value={line.unitCost}
                                onChange={(val) => updateLineCost(idx, val)}
                                className="h-10 text-sm"
                              />
                            </td>
                            <td className="px-4 py-3 text-right tabular-nums">
                              <p className="font-semibold text-slate-900">{formatCOP(lineSub)}</p>
                              {line.taxRate > 0 && (
                                <p className="text-[11px] text-slate-400">+ IVA: {formatCOP(lineTax)}</p>
                              )}
                            </td>
                            <td className="px-3 py-3 text-center">
                              <button
                                type="button"
                                onClick={() => removeLine(idx)}
                                className="text-slate-400 hover:text-rose-500 transition-colors p-1"
                              >
                                <Trash2 className="size-4" />
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            {/* Notas y Totales en vivo */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Notas de la compra (opcional)
                </label>
                <textarea
                  rows={3}
                  placeholder="Detalles de entrega, lote de compra o notas para revisión..."
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>

              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4 space-y-2 text-sm self-end">
                <div className="flex justify-between text-slate-600">
                  <span>Subtotal:</span>
                  <span className="font-medium tabular-nums">{formatCOP(subtotal)}</span>
                </div>
                <div className="flex justify-between text-slate-600">
                  <span>IVA estimado:</span>
                  <span className="font-medium tabular-nums">{formatCOP(taxAmount)}</span>
                </div>
                <div className="flex justify-between border-t border-slate-200 pt-2 text-base font-bold text-slate-900">
                  <span>Total estimado:</span>
                  <span className="tabular-nums text-teal-700">{formatCOP(total)} COP</span>
                </div>
              </div>
            </div>

            {/* Botones de acción */}
            <div className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-4">
              <button
                type="button"
                onClick={onClose}
                disabled={busy}
                className="h-11 rounded-full border border-slate-200 px-5 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50"
              >
                Cancelar
              </button>

              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  disabled={busy || lines.length === 0}
                  onClick={() => handleSave(false)}
                  className="h-11 rounded-full border border-teal-700/30 bg-teal-50/50 px-5 text-sm font-semibold text-teal-800 hover:bg-teal-100/60 disabled:opacity-50"
                >
                  {busy && <Loader2 className="mr-1.5 inline size-4 animate-spin" />}
                  Guardar como pendiente
                </button>
                <button
                  type="button"
                  disabled={busy || lines.length === 0}
                  onClick={() => handleSave(true)}
                  className="flex h-11 items-center gap-2 rounded-full bg-teal-700 px-6 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:opacity-50"
                >
                  {busy ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <PackageCheck className="size-4" />
                  )}
                  Guardar y recibir
                </button>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

"use client";

import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Building2,
  FilePlus2,
  Loader2,
  PackageCheck,
  Search,
  Trash2,
  Warehouse,
} from "lucide-react";

import { MoneyInput } from "@/components/caja/money-input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type CatalogItem,
  type CostPreviewResponse,
  type LocationRow,
  type StockLevel,
  type Supplier,
} from "@/lib/api";
import { formatCOP } from "@/lib/format";

type FormLine = {
  productId: string;
  name: string;
  sku: string | null;
  purchaseUnit: string | null;
  taxCode: string | null;
  taxRate: number;
  qty: number;
  unitCost: number;
  newSalePrice: number | null;
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
  const [purchaseId, setPurchaseId] = useState("");

  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [locations, setLocations] = useState<LocationRow[]>([]);
  const [catalogItems, setCatalogItems] = useState<CatalogItem[]>([]);
  const [stockLevels, setStockLevels] = useState<StockLevel[]>([]);
  const [loadingData, setLoadingData] = useState(false);

  // Form fields
  const [selectedSupplierId, setSelectedSupplierId] = useState("");
  const [locationId, setLocationId] = useState("");
  const [documentNumber, setDocumentNumber] = useState("");
  const [invoiceDate, setInvoiceDate] = useState("");
  const [expectedAt, setExpectedAt] = useState("");
  const [notes, setNotes] = useState("");
  const [paymentTerms, setPaymentTerms] = useState<"CONTADO" | "CREDITO">("CONTADO");
  const [creditDays, setCreditDays] = useState(0);
  const [discountAmount, setDiscountAmount] = useState(0);
  const [freightAmount, setFreightAmount] = useState(0);
  const [freightMode, setFreightMode] = useState<"PRORRATEAR" | "GASTO">("PRORRATEAR");
  const [ivaToCost, setIvaToCost] = useState(false);

  const [lines, setLines] = useState<FormLine[]>([]);

  // Cost preview
  const [costPreview, setCostPreview] = useState<CostPreviewResponse | null>(null);
  const [loadingPreview, setLoadingPreview] = useState(false);

  // Product search state
  const [productSearch, setProductSearch] = useState("");
  const [showProductDropdown, setShowProductDropdown] = useState(false);

  // Inline supplier creation
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
      setLocationId("");
      setDocumentNumber("");
      setInvoiceDate(new Date().toISOString().split("T")[0]);
      setExpectedAt("");
      setNotes("");
      setPaymentTerms("CONTADO");
      setCreditDays(0);
      setDiscountAmount(0);
      setFreightAmount(0);
      setFreightMode("PRORRATEAR");
      setIvaToCost(false);
      setLines([]);
      setCostPreview(null);
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
      api.locations.list(branchId).catch(() => []),
      api.catalog.list(branchId),
      api.inventory.stock(branchId).catch(() => []),
    ])
      .then(([supList, locList, catList, stkList]) => {
        if (!ignore) {
          setSuppliers(supList);
          setLocations(locList);
          setCatalogItems(catList);
          setStockLevels(stkList);

          const defaultLoc = locList.find((l) => l.is_default) || locList[0];
          if (defaultLoc) {
            setLocationId(defaultLoc.id);
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

  // When supplier changes, preload payment terms
  function handleSupplierChange(id: string) {
    setSelectedSupplierId(id);
    const sup = suppliers.find((s) => s.id === id);
    if (sup) {
      if (sup.payment_terms === "CREDITO") {
        setPaymentTerms("CREDITO");
        setCreditDays(sup.credit_days ?? 30);
      } else {
        setPaymentTerms("CONTADO");
        setCreditDays(0);
      }
    }
  }

  // Cost map for preloaded unit costs
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

  // Debounced cost-preview (400ms)
  useEffect(() => {
    if (!open || lines.length === 0 || !selectedSupplierId) {
      return;
    }

    const timer = setTimeout(() => {
      setLoadingPreview(true);
      api.purchases
        .costPreview({
          supplier_id: selectedSupplierId,
          discount_amount: discountAmount > 0 ? discountAmount.toFixed(2) : undefined,
          freight_amount: freightAmount > 0 ? freightAmount.toFixed(2) : undefined,
          freight_mode: freightMode,
          iva_to_cost: ivaToCost,
          items: lines.map((l) => ({
            product_id: l.productId,
            qty: l.qty.toFixed(4),
            unit_cost: l.unitCost.toFixed(2),
            new_sale_price: l.newSalePrice !== null ? l.newSalePrice.toFixed(2) : null,
          })),
        })
        .then((res) => {
          setCostPreview(res);
        })
        .catch(() => {
          // ignore preview error silently
        })
        .finally(() => {
          setLoadingPreview(false);
        });
    }, 400);

    return () => clearTimeout(timer);
  }, [open, lines, selectedSupplierId, discountAmount, freightAmount, freightMode, ivaToCost]);

  const activeCostPreview = lines.length > 0 && selectedSupplierId ? costPreview : null;

  if (!open) return null;

  function addProductToLines(p: CatalogItem) {
    if (lines.some((l) => l.productId === p.product_id)) {
      setProductSearch("");
      setShowProductDropdown(false);
      return;
    }
    const defaultCost = costMap.get(p.product_id) ?? 0;
    const stockItem = stockLevels.find((s) => s.product_id === p.product_id);

    setLines((prev) => [
      ...prev,
      {
        productId: p.product_id,
        name: p.name,
        sku: stockItem?.sku ?? null,
        purchaseUnit: null,
        taxCode: p.tax_code,
        taxRate: p.tax_code === "IVA19" ? 0.19 : (p.tax_code === "INC8" ? 0.08 : 0),
        qty: 1,
        unitCost: defaultCost,
        newSalePrice: null,
      },
    ]);
    setProductSearch("");
    setShowProductDropdown(false);
  }

  function updateLineQty(index: number, qty: number) {
    setLines((prev) =>
      prev.map((l, i) => (i === index ? { ...l, qty: Math.max(0.0001, qty) } : l)),
    );
  }

  function updateLineCost(index: number, unitCost: number) {
    setLines((prev) =>
      prev.map((l, i) => (i === index ? { ...l, unitCost: Math.max(0, unitCost) } : l)),
    );
  }

  function updateLineNewPrice(index: number, newSalePrice: number | null) {
    setLines((prev) =>
      prev.map((l, i) => (i === index ? { ...l, newSalePrice } : l)),
    );
  }

  function removeLine(index: number) {
    setLines((prev) => prev.filter((_, i) => i !== index));
  }

  async function handleCreateSupplierInline() {
    if (!newSupplierName.trim()) return;
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

  async function handleSave(mode: "borrador" | "aprobar" | "recibir_todo") {
    if (!selectedSupplierId) {
      setError("Tenés que seleccionar un proveedor para la compra.");
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
        setError(`El costo de "${l.name}" no puede ser negativo.`);
        return;
      }
    }

    setBusy(true);
    setError(null);

    try {
      const payload = {
        id: purchaseId,
        branch_id: branchId,
        location_id: locationId || undefined,
        supplier_id: selectedSupplierId,
        document_number: documentNumber.trim() || null,
        supplier_invoice_date: invoiceDate || null,
        expected_at: expectedAt || null,
        notes: notes.trim() || null,
        discount_amount: discountAmount > 0 ? discountAmount.toFixed(2) : undefined,
        freight_amount: freightAmount > 0 ? freightAmount.toFixed(2) : undefined,
        freight_mode: freightMode,
        payment_terms: paymentTerms,
        credit_days: paymentTerms === "CREDITO" ? creditDays : 0,
        iva_to_cost: ivaToCost,
        items: lines.map((l) => ({
          product_id: l.productId,
          qty: l.qty.toFixed(4),
          unit_cost: l.unitCost.toFixed(2),
          new_sale_price: l.newSalePrice !== null ? l.newSalePrice.toFixed(2) : null,
        })),
      };

      await api.purchases.create(payload);

      if (mode === "aprobar") {
        await api.purchases.approve(purchaseId);
        onSaved("Compra creada y aprobada con éxito.");
      } else if (mode === "recibir_todo") {
        await api.purchases.approve(purchaseId);
        await api.purchases.receive(purchaseId);
        onSaved("Compra creada, aprobada y recibida en inventario.");
      } else {
        onSaved("Borrador de compra guardado correctamente.");
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
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-4xl">
        {/* Header */}
        <div className="flex items-center gap-3 border-b border-slate-100 pb-4">
          <span className="flex size-10 items-center justify-center rounded-2xl bg-teal-50 text-teal-700">
            <FilePlus2 className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-xl font-bold tracking-tight text-slate-900">
              Registrar orden de compra
            </DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              Ingresá la compra a proveedor con costos de adquisición, prorrateo y bodega destino para {branchName}.
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
            <p className="mt-2 text-sm text-slate-500">Cargando catálogo, bodegas y proveedores...</p>
          </div>
        ) : (
          <div className="mt-4 space-y-6">
            {/* Fila 1: Proveedor, Bodega Destino y N.° Factura */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              {/* Proveedor */}
              <div>
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500 flex items-center gap-1.5">
                    <Building2 className="size-3.5" />
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
                  <div className="mt-1 space-y-2 rounded-xl border border-teal-200 bg-teal-50/50 p-2.5">
                    <input
                      type="text"
                      placeholder="Nombre *"
                      value={newSupplierName}
                      onChange={(e) => setNewSupplierName(e.target.value)}
                      className="w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs"
                    />
                    <input
                      type="text"
                      placeholder="NIT (opcional)"
                      value={newSupplierTaxId}
                      onChange={(e) => setNewSupplierTaxId(e.target.value)}
                      className="w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs"
                    />
                    <div className="flex gap-2">
                      <button
                        type="button"
                        disabled={savingSupplier}
                        onClick={handleCreateSupplierInline}
                        className="rounded-full bg-teal-700 px-3 py-1 text-xs font-semibold text-white"
                      >
                        Guardar
                      </button>
                      <button
                        type="button"
                        onClick={() => setShowNewSupplierInline(false)}
                        className="rounded-full border px-2.5 py-1 text-xs text-slate-600"
                      >
                        Cancelar
                      </button>
                    </div>
                  </div>
                ) : (
                  <select
                    value={selectedSupplierId}
                    onChange={(e) => handleSupplierChange(e.target.value)}
                    className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
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

              {/* Bodega destino */}
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500 flex items-center gap-1.5">
                  <Warehouse className="size-3.5" />
                  Bodega Destino *
                </label>
                <select
                  value={locationId}
                  onChange={(e) => setLocationId(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                >
                  {locations.map((loc) => (
                    <option key={loc.id} value={loc.id}>
                      {loc.name} {loc.is_default ? "(Principal)" : ""}
                    </option>
                  ))}
                </select>
              </div>

              {/* N.° de Factura */}
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  N.° Factura / Documento
                </label>
                <input
                  type="text"
                  placeholder="Ej. FAC-10492"
                  value={documentNumber}
                  onChange={(e) => setDocumentNumber(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>
            </div>

            {/* Fila 2: Fechas y Condiciones de pago */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Fecha Factura
                </label>
                <input
                  type="date"
                  value={invoiceDate}
                  onChange={(e) => setInvoiceDate(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Fecha Estimada Entrega
                </label>
                <input
                  type="date"
                  value={expectedAt}
                  onChange={(e) => setExpectedAt(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Condición de Pago
                </label>
                <select
                  value={paymentTerms}
                  onChange={(e) => setPaymentTerms(e.target.value as "CONTADO" | "CREDITO")}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                >
                  <option value="CONTADO">Contado</option>
                  <option value="CREDITO">Crédito</option>
                </select>
              </div>

              {paymentTerms === "CREDITO" && (
                <div>
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    Días de Crédito
                  </label>
                  <input
                    type="number"
                    min="1"
                    max="180"
                    value={creditDays || ""}
                    onChange={(e) => setCreditDays(parseInt(e.target.value, 10) || 0)}
                    className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                  />
                </div>
              )}
            </div>

            {/* Fila 3: Descuento, Flete y Modo Flete */}
            <div className="grid grid-cols-1 gap-4 rounded-2xl border border-slate-200/80 bg-slate-50/60 p-4 sm:grid-cols-4 items-center">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Descuento global
                </label>
                <MoneyInput
                  value={discountAmount}
                  onChange={setDiscountAmount}
                  className="mt-1 h-9 text-sm"
                />
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Valor del Flete
                </label>
                <MoneyInput
                  value={freightAmount}
                  onChange={setFreightAmount}
                  className="mt-1 h-9 text-sm"
                />
              </div>

              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-600">
                  Tratamiento del Flete
                </label>
                <select
                  value={freightMode}
                  onChange={(e) => setFreightMode(e.target.value as "PRORRATEAR" | "GASTO")}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs focus:border-teal-700 focus:outline-none"
                >
                  <option value="PRORRATEAR">Prorratear al costo</option>
                  <option value="GASTO">Registrar como gasto</option>
                </select>
              </div>

              <div className="flex items-center gap-2 pt-4">
                <input
                  type="checkbox"
                  id="ivaToCost"
                  checked={ivaToCost}
                  onChange={(e) => setIvaToCost(e.target.checked)}
                  className="size-4 rounded text-teal-700 focus:ring-teal-700"
                />
                <label htmlFor="ivaToCost" className="text-xs font-medium text-slate-700">
                  Llevar IVA al costo
                </label>
              </div>
            </div>

            {/* Buscador de productos */}
            <div className="relative">
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Buscar productos para agregar
              </label>
              <div className="relative mt-1">
                <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 size-4 text-slate-400" />
                <input
                  type="text"
                  placeholder="Escribí el nombre, código o SKU..."
                  value={productSearch}
                  onFocus={() => setShowProductDropdown(true)}
                  onChange={(e) => {
                    setProductSearch(e.target.value);
                    setShowProductDropdown(true);
                  }}
                  className="w-full rounded-xl border border-slate-200 bg-white pl-10 pr-4 py-2.5 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>

              {showProductDropdown && filteredProducts.length > 0 && (
                <div className="absolute z-20 mt-1 max-h-56 w-full overflow-y-auto rounded-2xl border border-slate-200 bg-white p-2 shadow-lg">
                  {filteredProducts.map((p) => (
                    <button
                      key={p.product_id}
                      type="button"
                      onClick={() => addProductToLines(p)}
                      className="flex w-full items-center justify-between rounded-xl px-3 py-2 text-left text-sm hover:bg-slate-50"
                    >
                      <div>
                        <p className="font-semibold text-slate-800">{p.name}</p>
                        <p className="text-xs text-slate-400">
                          {p.category || "General"} • {p.tax_code || "Exento"}
                        </p>
                      </div>
                      <span className="text-xs font-semibold text-teal-700">+ Agregar</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Tabla de líneas */}
            <div>
              <div className="flex items-center justify-between pb-1">
                <span className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Líneas de la compra ({lines.length})
                </span>
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
                        <th className="px-3 py-2.5 text-right w-24">Cant. Compra</th>
                        <th className="px-3 py-2.5 text-right w-32">Costo Unitario</th>
                        <th className="px-3 py-2.5 text-right w-32">Nuevo Precio Venta</th>
                        <th className="px-3 py-2.5 text-right w-28">Subtotal</th>
                        <th className="px-2 py-2.5 text-center w-10"></th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {lines.map((line, idx) => {
                        const lineSub = line.qty * line.unitCost;
                        return (
                          <tr key={line.productId} className="hover:bg-slate-50/50">
                            <td className="px-4 py-3">
                              <p className="font-medium text-slate-900">{line.name}</p>
                              <p className="text-xs text-slate-400">
                                {line.purchaseUnit ? `Unidad: ${line.purchaseUnit}` : ""}
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
                                className="w-20 rounded-xl border border-slate-200 px-2.5 py-1 text-right text-sm font-semibold focus:border-teal-700 focus:outline-none"
                              />
                            </td>
                            <td className="px-3 py-3 text-right">
                              <MoneyInput
                                value={line.unitCost}
                                onChange={(val) => updateLineCost(idx, val)}
                                className="h-9 text-sm"
                              />
                            </td>
                            <td className="px-3 py-3 text-right">
                              <MoneyInput
                                value={line.newSalePrice ?? 0}
                                onChange={(val) => updateLineNewPrice(idx, val > 0 ? val : null)}
                                className="h-9 text-sm"
                              />
                            </td>
                            <td className="px-3 py-3 text-right tabular-nums font-semibold text-slate-800">
                              {formatCOP(lineSub)}
                            </td>
                            <td className="px-2 py-3 text-center">
                              <button
                                type="button"
                                onClick={() => removeLine(idx)}
                                className="text-slate-400 hover:text-rose-500"
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

            {/* Panel de Vista Previa de Costos */}
            {activeCostPreview && activeCostPreview.lines.length > 0 && (
              <div className="rounded-2xl border border-teal-200 bg-teal-50/40 p-4">
                <div className="flex items-center justify-between pb-2 border-b border-teal-100">
                  <h4 className="text-xs font-bold uppercase tracking-wider text-teal-900">
                    Vista previa de costos y márgenes proyectados
                  </h4>
                  {loadingPreview && <Loader2 className="size-3.5 animate-spin text-teal-700" />}
                </div>

                <div className="mt-3 overflow-x-auto">
                  <table className="w-full text-left text-xs">
                    <thead>
                      <tr className="text-teal-800 font-semibold border-b border-teal-100/80">
                        <th className="pb-2">Producto</th>
                        <th className="pb-2 text-right">Costo Llegada (Landed)</th>
                        <th className="pb-2 text-right">Precio Actual</th>
                        <th className="pb-2 text-right">Nuevo Precio</th>
                        <th className="pb-2 text-right">Margen Proyectado</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-teal-100/50">
                      {activeCostPreview.lines.map((cl) => {
                        const marginNum = cl.resulting_margin_pct ? Number(cl.resulting_margin_pct) : null;
                        return (
                          <tr key={cl.product_id}>
                            <td className="py-2 text-slate-800 font-medium">{cl.product_name}</td>
                            <td className="py-2 text-right tabular-nums font-bold text-teal-900">
                              {formatCOP(Number(cl.landed_unit_cost))}
                            </td>
                            <td className="py-2 text-right tabular-nums text-slate-600">
                              {cl.current_price ? formatCOP(Number(cl.current_price)) : "—"}
                            </td>
                            <td className="py-2 text-right tabular-nums text-slate-700 font-semibold">
                              {cl.new_sale_price ? formatCOP(Number(cl.new_sale_price)) : "—"}
                            </td>
                            <td className="py-2 text-right tabular-nums font-bold">
                              {marginNum !== null ? (
                                <span className={marginNum >= 20 ? "text-emerald-700" : "text-amber-700"}>
                                  {marginNum.toFixed(1)}%
                                </span>
                              ) : (
                                "—"
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                {activeCostPreview.warnings.length > 0 && (
                  <div className="mt-3 flex items-center gap-2 text-xs text-amber-800">
                    <AlertCircle className="size-4 shrink-0" />
                    <span>Advertencia: {activeCostPreview.warnings.join(", ")}</span>
                  </div>
                )}
              </div>
            )}

            {/* Notas y Totales */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Notas de la compra (opcional)
                </label>
                <textarea
                  rows={3}
                  placeholder="Detalles de entrega o notas para revisión..."
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-teal-700 focus:outline-none"
                />
              </div>

              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4 space-y-2 text-sm self-end">
                <div className="flex justify-between text-slate-600">
                  <span>Subtotal neto:</span>
                  <span className="font-medium tabular-nums">
                    {formatCOP(activeCostPreview ? Number(activeCostPreview.subtotal) : lines.reduce((s, l) => s + l.qty * l.unitCost, 0))}
                  </span>
                </div>
                {discountAmount > 0 && (
                  <div className="flex justify-between text-slate-600">
                    <span>Descuento global:</span>
                    <span className="font-medium tabular-nums text-emerald-700">
                      -{formatCOP(discountAmount)}
                    </span>
                  </div>
                )}
                <div className="flex justify-between text-slate-600">
                  <span>Flete ({freightMode}):</span>
                  <span className="font-medium tabular-nums">+{formatCOP(freightAmount)}</span>
                </div>
                <div className="flex justify-between text-slate-600">
                  <span>Impuestos (IVA / INC):</span>
                  <span className="font-medium tabular-nums">
                    {formatCOP(activeCostPreview ? Number(activeCostPreview.tax_total) : 0)}
                  </span>
                </div>
                <div className="flex justify-between border-t border-slate-200 pt-2 text-base font-bold text-slate-900">
                  <span>Total compra:</span>
                  <span className="tabular-nums text-teal-700">
                    {formatCOP(activeCostPreview ? Number(activeCostPreview.total) : 0)} COP
                  </span>
                </div>
              </div>
            </div>

            {/* 3 Botones de acción según F2.12 */}
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
                  onClick={() => handleSave("borrador")}
                  className="h-11 rounded-full border border-slate-300 bg-white px-4 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:opacity-50"
                >
                  {busy && <Loader2 className="mr-1.5 inline size-3.5 animate-spin" />}
                  Guardar borrador
                </button>
                <button
                  type="button"
                  disabled={busy || lines.length === 0}
                  onClick={() => handleSave("aprobar")}
                  className="h-11 rounded-full border border-blue-600/30 bg-blue-50 px-4 text-xs font-semibold text-blue-800 hover:bg-blue-100 disabled:opacity-50"
                >
                  {busy && <Loader2 className="mr-1.5 inline size-3.5 animate-spin" />}
                  Aprobar
                </button>
                <button
                  type="button"
                  disabled={busy || lines.length === 0}
                  onClick={() => handleSave("recibir_todo")}
                  className="flex h-11 items-center gap-2 rounded-full bg-teal-700 px-5 text-xs font-semibold text-white shadow-sm hover:bg-teal-800 disabled:opacity-50"
                >
                  {busy ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <PackageCheck className="size-4" />
                  )}
                  Aprobar y recibir todo
                </button>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

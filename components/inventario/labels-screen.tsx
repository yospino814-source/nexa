"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import JsBarcode from "jsbarcode";
import {
  ArrowLeft,
  Barcode,
  Minus,
  Plus,
  Printer,
  Search,
  Sparkles,
  Trash2,
} from "lucide-react";

import { api, type ProductRow } from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

interface LabelItem {
  product: ProductRow;
  quantity: number;
}

type LabelFormat = "ROLL_50_25" | "A4_3_8";

const TAX_RATES: Record<string, number> = {
  IVA19: 0.19,
  IVA5: 0.05,
  INC8: 0.08,
  EXENTO: 0,
};

function getTaxRate(taxCode?: string): number {
  if (!taxCode) return 0;
  return TAX_RATES[taxCode.toUpperCase()] ?? 0;
}

function calculatePriceWithTax(priceStr?: string | null, taxCode?: string): number {
  if (!priceStr) return 0;
  const basePrice = parseFloat(priceStr) || 0;
  const rate = getTaxRate(taxCode);
  return Math.round(basePrice * (1 + rate));
}

function BarcodeSvg({ value }: { value: string }) {
  const svgRef = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    if (!svgRef.current || !value) return;
    try {
      const isEan13 = value.length === 13 && /^\d{13}$/.test(value);
      JsBarcode(svgRef.current, value, {
        format: isEan13 ? "EAN13" : "CODE128",
        width: 1.3,
        height: 36,
        displayValue: true,
        fontSize: 10,
        margin: 1,
        textMargin: 1,
      });
    } catch {
      try {
        if (svgRef.current) {
          JsBarcode(svgRef.current, value, {
            format: "CODE128",
            width: 1.1,
            height: 36,
            displayValue: true,
            fontSize: 10,
            margin: 1,
          });
        }
      } catch (err) {
        console.error("Barcode error:", err);
      }
    }
  }, [value]);

  return <svg ref={svgRef} className="max-w-full h-auto" />;
}

export function LabelsScreen() {
  const [format, setFormat] = useState<LabelFormat>("ROLL_50_25");
  const [items, setItems] = useState<LabelItem[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<ProductRow[]>([]);
  const [searching, setSearching] = useState(false);
  const [generatingId, setGeneratingId] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Search products
  useEffect(() => {
    const q = searchQuery.trim();
    const timer = setTimeout(async () => {
      if (!q) {
        setSearchResults([]);
        return;
      }
      setSearching(true);
      try {
        const res = await api.products.list({ q, page_size: 8 });
        setSearchResults(res.items);
      } catch (err) {
        console.error(err);
      } finally {
        setSearching(false);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  function handleAddProduct(product: ProductRow) {
    setItems((prev) => {
      const existing = prev.find((i) => i.product.id === product.id);
      if (existing) {
        return prev.map((i) =>
          i.product.id === product.id ? { ...i, quantity: i.quantity + 1 } : i,
        );
      }
      return [...prev, { product, quantity: 1 }];
    });
    setSearchQuery("");
    setSearchResults([]);
  }

  function handleQuantityChange(id: string, qty: number) {
    if (qty <= 0) {
      handleRemoveItem(id);
      return;
    }
    setItems((prev) =>
      prev.map((i) => (i.product.id === id ? { ...i, quantity: qty } : i)),
    );
  }

  function handleRemoveItem(id: string) {
    setItems((prev) => prev.filter((i) => i.product.id !== id));
  }

  async function handleGenerateBarcode(product: ProductRow) {
    setGeneratingId(product.id);
    setErrorMsg(null);
    try {
      const res = await api.products.generateBarcode(product.id);
      const updatedProduct = { ...product, barcode: res.barcode };
      setItems((prev) =>
        prev.map((i) =>
          i.product.id === product.id ? { ...i, product: updatedProduct } : i,
        ),
      );
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al generar código";
      setErrorMsg(msg);
    } finally {
      setGeneratingId(null);
    }
  }

  function handlePrint() {
    window.print();
  }

  // Flattened labels list for printing
  const flattenedLabels: ProductRow[] = [];
  items.forEach((item) => {
    for (let k = 0; k < item.quantity; k++) {
      flattenedLabels.push(item.product);
    }
  });

  return (
    <div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-4 md:p-8">
      {/* Print CSS styles */}
      <style jsx global>{`
        @media print {
          body * {
            visibility: hidden;
          }
          #print-area,
          #print-area * {
            visibility: visible;
          }
          #print-area {
            position: absolute;
            left: 0;
            top: 0;
            width: 100%;
            margin: 0;
            padding: 0;
          }
          @page {
            size: ${format === "ROLL_50_25" ? "50mm 25mm" : "A4 portrait"};
            margin: ${format === "ROLL_50_25" ? "0" : "10mm"};
          }
        }
      `}</style>

      {/* Screen view (hidden during print) */}
      <div className="max-w-5xl mx-auto space-y-6 print:hidden">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <Link
              href="/inventario"
              className="p-2 text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 rounded-lg border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-900 transition"
              title="Volver a inventario"
            >
              <ArrowLeft className="w-5 h-5" />
            </Link>
            <div>
              <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-100 flex items-center gap-2">
                <Barcode className="w-7 h-7 text-emerald-600" />
                Impresión de Etiquetas
              </h1>
              <p className="text-sm text-slate-500">
                Diseña e imprime etiquetas de góndola y productos con código de barras
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={handlePrint}
              disabled={flattenedLabels.length === 0}
              className="flex items-center gap-2 px-4 py-2.5 bg-emerald-600 hover:bg-emerald-700 disabled:bg-slate-300 dark:disabled:bg-slate-800 text-white font-medium rounded-xl shadow-sm transition disabled:cursor-not-allowed"
            >
              <Printer className="w-4 h-4" />
              Imprimir ({flattenedLabels.length})
            </button>
          </div>
        </div>

        {errorMsg && (
          <div className="p-3 bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-400 rounded-xl text-sm">
            {errorMsg}
          </div>
        )}

        {/* Controls Grid */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          {/* Format selector & Search */}
          <div className="md:col-span-1 space-y-5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 p-5 rounded-2xl shadow-sm">
            <div>
              <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 mb-2">
                Formato de etiqueta
              </label>
              <div className="space-y-2">
                <label
                  className={cn(
                    "flex items-center gap-3 p-3 rounded-xl border cursor-pointer transition",
                    format === "ROLL_50_25"
                      ? "border-emerald-600 bg-emerald-50/50 dark:bg-emerald-950/20 text-emerald-950 dark:text-emerald-100 font-medium"
                      : "border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50",
                  )}
                >
                  <input
                    type="radio"
                    name="format"
                    value="ROLL_50_25"
                    checked={format === "ROLL_50_25"}
                    onChange={() => setFormat("ROLL_50_25")}
                    className="accent-emerald-600"
                  />
                  <div>
                    <div>Rollo 50 × 25 mm</div>
                    <div className="text-xs text-slate-500 font-normal">
                      Impresora térmica de etiquetas
                    </div>
                  </div>
                </label>

                <label
                  className={cn(
                    "flex items-center gap-3 p-3 rounded-xl border cursor-pointer transition",
                    format === "A4_3_8"
                      ? "border-emerald-600 bg-emerald-50/50 dark:bg-emerald-950/20 text-emerald-950 dark:text-emerald-100 font-medium"
                      : "border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50",
                  )}
                >
                  <input
                    type="radio"
                    name="format"
                    value="A4_3_8"
                    checked={format === "A4_3_8"}
                    onChange={() => setFormat("A4_3_8")}
                    className="accent-emerald-600"
                  />
                  <div>
                    <div>Hoja A4 (3 × 8)</div>
                    <div className="text-xs text-slate-500 font-normal">
                      24 etiquetas por página
                    </div>
                  </div>
                </label>
              </div>
            </div>

            {/* Search products */}
            <div className="relative">
              <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 mb-2">
                Buscar productos para agregar
              </label>
              <div className="relative">
                <Search className="w-4 h-4 absolute left-3 top-3 text-slate-400" />
                <input
                  type="text"
                  placeholder="Nombre o SKU..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full pl-9 pr-4 py-2 border border-slate-200 dark:border-slate-800 rounded-xl bg-slate-50 dark:bg-slate-950 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500"
                />
              </div>

              {/* Autocomplete dropdown */}
              {searchResults.length > 0 && (
                <div className="absolute z-10 mt-1 w-full bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-lg max-h-60 overflow-y-auto divide-y divide-slate-100 dark:divide-slate-800">
                  {searchResults.map((prod) => (
                    <button
                      key={prod.id}
                      onClick={() => handleAddProduct(prod)}
                      className="w-full px-4 py-2.5 text-left hover:bg-slate-50 dark:hover:bg-slate-800 transition flex items-center justify-between gap-2"
                    >
                      <div className="truncate">
                        <div className="text-sm font-medium text-slate-900 dark:text-slate-100 truncate">
                          {prod.name}
                        </div>
                        <div className="text-xs text-slate-500">
                          {prod.sku || "Sin SKU"} • {formatCOP(calculatePriceWithTax(prod.price, prod.tax_code))}
                        </div>
                      </div>
                      <Plus className="w-4 h-4 text-emerald-600 shrink-0" />
                    </button>
                  ))}
                </div>
              )}
              {searching && (
                <div className="text-xs text-slate-400 mt-1">Buscando productos...</div>
              )}
            </div>
          </div>

          {/* Selected products list */}
          <div className="md:col-span-2 space-y-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 p-5 rounded-2xl shadow-sm">
            <h2 className="text-sm font-semibold text-slate-700 dark:text-slate-300">
              Productos seleccionados ({items.length})
            </h2>

            {items.length === 0 ? (
              <div className="py-12 text-center text-slate-400">
                <Barcode className="w-12 h-12 mx-auto mb-2 opacity-30" />
                <p className="text-sm">No has seleccionado ningún producto.</p>
                <p className="text-xs text-slate-500 mt-1">
                  Usa el buscador para agregar productos a la lista de etiquetas.
                </p>
              </div>
            ) : (
              <div className="divide-y divide-slate-100 dark:divide-slate-800">
                {items.map(({ product, quantity }) => {
                  const priceWithTax = calculatePriceWithTax(
                    product.price,
                    product.tax_code,
                  );
                  return (
                    <div
                      key={product.id}
                      className="py-3 flex flex-col sm:flex-row sm:items-center justify-between gap-3"
                    >
                      <div className="flex-1 min-w-0">
                        <div className="font-medium text-sm text-slate-900 dark:text-slate-100 truncate">
                          {product.name}
                        </div>
                        <div className="text-xs text-slate-500 flex items-center gap-2 flex-wrap">
                          <span>{formatCOP(priceWithTax)} (con IVA)</span>
                          <span>•</span>
                          {product.barcode ? (
                            <span className="font-mono bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded text-[11px]">
                              {product.barcode}
                            </span>
                          ) : (
                            <button
                              onClick={() => handleGenerateBarcode(product)}
                              disabled={generatingId === product.id}
                              className="text-emerald-600 hover:text-emerald-700 text-xs font-medium inline-flex items-center gap-1 disabled:opacity-50"
                            >
                              <Sparkles className="w-3.5 h-3.5" />
                              {generatingId === product.id
                                ? "Generando..."
                                : "Generar código"}
                            </button>
                          )}
                        </div>
                      </div>

                      {/* Quantity stepper and delete */}
                      <div className="flex items-center gap-3 shrink-0">
                        <div className="flex items-center border border-slate-200 dark:border-slate-800 rounded-lg overflow-hidden">
                          <button
                            onClick={() =>
                              handleQuantityChange(product.id, quantity - 1)
                            }
                            className="p-1.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                            title="Disminuir"
                          >
                            <Minus className="w-3.5 h-3.5" />
                          </button>
                          <input
                            type="number"
                            min="1"
                            value={quantity}
                            onChange={(e) =>
                              handleQuantityChange(
                                product.id,
                                parseInt(e.target.value) || 1,
                              )
                            }
                            className="w-12 text-center text-sm py-1 bg-transparent focus:outline-none font-medium"
                          />
                          <button
                            onClick={() =>
                              handleQuantityChange(product.id, quantity + 1)
                            }
                            className="p-1.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                            title="Aumentar"
                          >
                            <Plus className="w-3.5 h-3.5" />
                          </button>
                        </div>

                        <button
                          onClick={() => handleRemoveItem(product.id)}
                          className="p-1.5 text-slate-400 hover:text-red-600 dark:hover:text-red-400 transition"
                          title="Eliminar de la lista"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Preview Section */}
        {flattenedLabels.length > 0 && (
          <div className="space-y-4">
            <h2 className="text-sm font-semibold text-slate-700 dark:text-slate-300">
              Vista previa ({flattenedLabels.length} etiquetas)
            </h2>
            <div className="p-6 bg-slate-200/60 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl overflow-x-auto">
              <div
                className={cn(
                  "gap-4",
                  format === "ROLL_50_25"
                    ? "flex flex-wrap"
                    : "grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 max-w-4xl mx-auto",
                )}
              >
                {flattenedLabels.slice(0, 12).map((prod, idx) => {
                  const priceWithTax = calculatePriceWithTax(
                    prod.price,
                    prod.tax_code,
                  );
                  return (
                    <div
                      key={`${prod.id}-${idx}`}
                      className="bg-white text-slate-900 border border-slate-300 rounded shadow-sm p-2 flex flex-col justify-between"
                      style={{
                        width: "188px",
                        height: "94px",
                      }}
                    >
                      <div className="text-[11px] font-semibold leading-tight line-clamp-2">
                        {prod.name}
                      </div>
                      <div className="flex items-end justify-between gap-1 mt-1">
                        <div className="flex-1 overflow-hidden">
                          {prod.barcode ? (
                            <BarcodeSvg value={prod.barcode} />
                          ) : (
                            <span className="text-[10px] text-amber-600 font-mono">
                              [Sin código]
                            </span>
                          )}
                        </div>
                        <div className="text-[13px] font-bold text-slate-900 shrink-0">
                          {formatCOP(priceWithTax)}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
              {flattenedLabels.length > 12 && (
                <p className="text-xs text-center text-slate-500 mt-4">
                  Mostrando primeras 12 de {flattenedLabels.length} etiquetas
                </p>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Printable Area (visible only during window.print) */}
      <div id="print-area" className="hidden print:block">
        {format === "ROLL_50_25" ? (
          <div>
            {flattenedLabels.map((prod, idx) => {
              const priceWithTax = calculatePriceWithTax(
                prod.price,
                prod.tax_code,
              );
              return (
                <div
                  key={`print-roll-${prod.id}-${idx}`}
                  className="box-border flex flex-col justify-between overflow-hidden"
                  style={{
                    width: "50mm",
                    height: "25mm",
                    padding: "1.5mm",
                    pageBreakInside: "avoid",
                    breakInside: "avoid",
                    pageBreakAfter: "always",
                    breakAfter: "page",
                  }}
                >
                  <div
                    style={{
                      fontSize: "9pt",
                      fontWeight: 600,
                      lineHeight: "1.1",
                      maxHeight: "2.4em",
                      overflow: "hidden",
                    }}
                  >
                    {prod.name}
                  </div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "flex-end",
                      justifyContent: "space-between",
                      gap: "2mm",
                    }}
                  >
                    <div style={{ flex: 1, overflow: "hidden" }}>
                      {prod.barcode ? (
                        <BarcodeSvg value={prod.barcode} />
                      ) : (
                        <span style={{ fontSize: "8pt" }}>[Sin código]</span>
                      )}
                    </div>
                    <div
                      style={{
                        fontSize: "11pt",
                        fontWeight: 700,
                        whiteSpace: "nowrap",
                      }}
                    >
                      {formatCOP(priceWithTax)}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(3, 1fr)",
              gap: "2mm",
              width: "100%",
            }}
          >
            {flattenedLabels.map((prod, idx) => {
              const priceWithTax = calculatePriceWithTax(
                prod.price,
                prod.tax_code,
              );
              return (
                <div
                  key={`print-a4-${prod.id}-${idx}`}
                  style={{
                    border: "0.5pt dashed #ccc",
                    boxSizing: "border-box",
                    padding: "2mm",
                    height: "32mm",
                    display: "flex",
                    flexDirection: "column",
                    justifyContent: "space-between",
                    pageBreakInside: "avoid",
                    breakInside: "avoid",
                  }}
                >
                  <div
                    style={{
                      fontSize: "9pt",
                      fontWeight: 600,
                      lineHeight: "1.1",
                      maxHeight: "2.4em",
                      overflow: "hidden",
                    }}
                  >
                    {prod.name}
                  </div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "flex-end",
                      justifyContent: "space-between",
                      gap: "2mm",
                    }}
                  >
                    <div style={{ flex: 1, overflow: "hidden" }}>
                      {prod.barcode ? (
                        <BarcodeSvg value={prod.barcode} />
                      ) : (
                        <span style={{ fontSize: "8pt" }}>[Sin código]</span>
                      )}
                    </div>
                    <div
                      style={{
                        fontSize: "11pt",
                        fontWeight: 700,
                        whiteSpace: "nowrap",
                      }}
                    >
                      {formatCOP(priceWithTax)}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

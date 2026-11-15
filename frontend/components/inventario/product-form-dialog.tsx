"use client";

import { useEffect, useState } from "react";
import { Barcode, Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  USE_MOCKS,
  type ProductInput,
  type StockLevel,
} from "@/lib/api";

interface ProductFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  product?: StockLevel | null;
  branchId: string;
  onSaved: (message: string) => void;
}

const CATEGORIES = [
  "Abarrotes",
  "Bebidas",
  "Lácteos",
  "Panadería",
  "Aseo y Hogar",
  "Snacks y Dulces",
  "Licores y Cervezas",
  "Comidas Rápidas",
];

function ProductFormContent({
  product,
  branchId,
  onSaved,
  onCancel,
}: {
  product?: StockLevel | null;
  branchId: string;
  onSaved: (message: string) => void;
  onCancel: () => void;
}) {
  const isNew = !product;

  const [name, setName] = useState(() => product?.name || "");
  const [category, setCategory] = useState(() => product?.category || "Abarrotes");
  const [price, setPrice] = useState(() =>
    product?.price ? String(Math.round(Number(product.price))) : "",
  );
  const [costPrice, setCostPrice] = useState(() =>
    product?.cost_price ? String(Math.round(Number(product.cost_price))) : "",
  );
  const [barcode, setBarcode] = useState(() => product?.barcode || product?.sku || "");
  const [taxCode, setTaxCode] = useState("IVA19");
  const [alcoholDegrees, setAlcoholDegrees] = useState("");
  const [volumeMl, setVolumeMl] = useState("750");
  const [unitMeasureCode, setUnitMeasureCode] = useState("UND");
  const [available, setAvailable] = useState(() =>
    product?.available ? String(Number(product.available)) : "10",
  );
  const [minQty, setMinQty] = useState(() =>
    product?.min_qty ? String(Number(product.min_qty)) : "5",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (product?.product_id) {
      api.products
        .get(product.product_id, branchId)
        .then((detail) => {
          if (detail.tax_code) setTaxCode(detail.tax_code);
          if (detail.alcohol_degrees) setAlcoholDegrees(String(detail.alcohol_degrees));
          if (detail.volume_ml) setVolumeMl(String(detail.volume_ml));
          if (detail.unit_measure_code) setUnitMeasureCode(detail.unit_measure_code);
        })
        .catch(() => {});
    }
  }, [product?.product_id, branchId]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setError("El nombre del producto es obligatorio.");
      return;
    }
    if (!price || Number(price) <= 0) {
      setError("El precio de venta debe ser mayor a 0.");
      return;
    }
    if (taxCode === "ICL") {
      const deg = Number(alcoholDegrees);
      const vol = Number(volumeMl);
      if (!alcoholDegrees || isNaN(deg) || deg <= 0 || deg > 100) {
        setError("Los productos con ICL requieren grados de alcohol válidos (0.1 a 100°).");
        return;
      }
      if (!volumeMl || isNaN(vol) || vol <= 0) {
        setError("Los productos con ICL requieren volumen en ml válido (> 0).");
        return;
      }
    }

    setBusy(true);
    setError(null);

    try {
      const basePriceFormatted = Number(price).toFixed(2);
      const costPriceNum = costPrice ? Number(costPrice) : null;
      const costPriceFormatted =
        costPriceNum !== null && costPriceNum > 0 ? costPriceNum.toFixed(2) : null;

      const input: ProductInput = {
        name: name.trim(),
        barcode: barcode.trim() || null,
        base_price: basePriceFormatted,
        cost_price: costPriceFormatted,
        tax_code: taxCode,
        alcohol_degrees: taxCode === "ICL" && alcoholDegrees ? alcoholDegrees.trim() : null,
        volume_ml: taxCode === "ICL" && volumeMl ? volumeMl.trim() : null,
        unit_measure_code: unitMeasureCode,
      };

      if (USE_MOCKS) {
        if (category) {
          (input as Record<string, unknown>).category = category.trim();
        }
        if (minQty) {
          (input as Record<string, unknown>).reorder_qty = minQty;
        }
      }

      if (isNew) {
        const created = await api.products.create(input);
        let successMsg = "Producto creado exitosamente.";
        const availNum = Number(available);
        if (availNum > 0) {
          const adj = await api.inventory.adjust({
            branch_id: branchId,
            product_id: created.id,
            qty_delta: String(availNum),
            reason_code: "CARGA_INICIAL",
            notes: "Carga inicial",
            unit_cost: costPriceFormatted,
          });
          if (adj.status === "PENDIENTE") {
            successMsg =
              "Producto creado. El stock inicial quedó pendiente de aprobación por superar el umbral.";
          }
        }
        onSaved(successMsg);
      } else {
        await api.products.update(product.product_id, input);
        onSaved("Producto actualizado exitosamente.");
      }
    } catch (err: unknown) {
      setError(friendlyError(err));
    } finally {
      setBusy(false);
    }
  }

  function generateRandomBarcode() {
    const randomSuffix = Math.floor(100000000 + Math.random() * 900000000);
    setBarcode(`770${randomSuffix}`);
  }

  return (
    <>
      <div className="flex items-center justify-between border-b border-slate-100 pb-4">
        <div>
          <DialogTitle className="font-headline text-lg font-bold text-slate-900">
            {isNew ? "Crear nuevo producto" : "Editar producto"}
          </DialogTitle>
          <p className="text-xs text-slate-500">
            {isNew
              ? "Registrá un nuevo artículo en la base central de inventario"
              : `Actualizá los datos de ${product?.name}`}
          </p>
        </div>
      </div>

      {error && (
        <div className="mt-3 rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs font-semibold text-rose-700">
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="mt-4 space-y-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {/* Nombre del producto */}
          <div className="col-span-1 sm:col-span-2">
            <label className="mb-1.5 block text-xs font-semibold text-slate-700">
              Nombre del producto *
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Ej. Café Juan Valdez 500 g Molido"
              className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
            />
          </div>

          {/* Categoría: solo en modo demo */}
          {USE_MOCKS && (
            <div>
              <label className="mb-1.5 block text-xs font-semibold text-slate-700">
                Categoría *
              </label>
              <select
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              >
                {CATEGORIES.map((cat) => (
                  <option key={cat} value={cat}>
                    {cat}
                  </option>
                ))}
              </select>
            </div>
          )}

          {/* Código de barras / SKU */}
          <div className={!USE_MOCKS ? "col-span-1 sm:col-span-2" : ""}>
            <label className="mb-1.5 block text-xs font-semibold text-slate-700">
              Código de barras / SKU
            </label>
            <div className="flex items-center gap-1.5">
              <input
                type="text"
                value={barcode}
                onChange={(e) => setBarcode(e.target.value)}
                placeholder="770210098..."
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              />
              <button
                type="button"
                onClick={generateRandomBarcode}
                title="Generar código"
                className="flex size-9 shrink-0 items-center justify-center rounded-xl border border-slate-200 bg-slate-100 text-slate-700 hover:bg-slate-200"
              >
                <Barcode className="size-4" />
              </button>
            </div>
          </div>

          {/* Precio de venta (COP) */}
          <div>
            <label className="mb-1.5 block text-xs font-semibold text-slate-700">
              Precio de venta (COP) *
            </label>
            <div className="relative">
              <span className="absolute top-1/2 left-3.5 -translate-y-1/2 text-xs font-bold text-slate-400">
                $
              </span>
              <input
                type="number"
                value={price}
                onChange={(e) => setPrice(e.target.value)}
                placeholder="12000"
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 pr-3.5 pl-8 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              />
            </div>
          </div>

          {/* Costo unitario (COP) */}
          <div>
            <label className="mb-1.5 block text-xs font-semibold text-slate-700">
              Costo unitario (COP)
            </label>
            <div className="relative">
              <span className="absolute top-1/2 left-3.5 -translate-y-1/2 text-xs font-bold text-slate-400">
                $
              </span>
              <input
                type="number"
                value={costPrice}
                onChange={(e) => setCostPrice(e.target.value)}
                placeholder="8500"
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 pr-3.5 pl-8 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              />
            </div>
          </div>

          {/* Impuesto (IVA / INC / ICL) */}
          <div className="col-span-1 sm:col-span-2">
            <label className="mb-1.5 block text-xs font-semibold text-slate-700">
              Impuesto
            </label>
            <select
              value={taxCode}
              onChange={(e) => setTaxCode(e.target.value)}
              className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
            >
              <option value="IVA19">IVA General (19%)</option>
              <option value="IVA5">IVA Reducido (5%)</option>
              <option value="INC8">Impoconsumo (8%)</option>
              <option value="ICL">Impuesto al Consumo de Licores (ICL)</option>
              <option value="EXENTO">Exento / Excluido (0%)</option>
            </select>
          </div>

          {/* Datos específicos de licores para ICL */}
          {taxCode === "ICL" && (
            <div className="col-span-1 sm:col-span-2 grid grid-cols-2 gap-4 rounded-xl border border-indigo-100 bg-indigo-50/40 p-3.5">
              <div>
                <label className="mb-1.5 block text-xs font-semibold text-slate-700">
                  Grados de alcohol (°) *
                </label>
                <input
                  type="number"
                  step="0.1"
                  min="0.1"
                  max="100"
                  value={alcoholDegrees}
                  onChange={(e) => setAlcoholDegrees(e.target.value)}
                  placeholder="Ej. 29"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-indigo-600 focus:outline-none"
                />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-semibold text-slate-700">
                  Volumen (ml) *
                </label>
                <input
                  type="number"
                  step="1"
                  min="1"
                  value={volumeMl}
                  onChange={(e) => setVolumeMl(e.target.value)}
                  placeholder="Ej. 750"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-indigo-600 focus:outline-none"
                />
              </div>
              <p className="col-span-2 text-[11px] text-slate-500">
                La DIAN exige reportar el ICL nominal según grados alcoholimétricos por cada 750 ml (Ley 1816 de 2016).
              </p>
            </div>
          )}

          {/* Stock inicial: solo al crear */}
          {isNew && (
            <div>
              <label className="mb-1.5 block text-xs font-semibold text-slate-700">
                Stock inicial
              </label>
              <input
                type="number"
                value={available}
                onChange={(e) => setAvailable(e.target.value)}
                placeholder="0"
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              />
            </div>
          )}

          {/* Alerta stock mínimo: solo en modo demo */}
          {USE_MOCKS && (
            <div>
              <label className="mb-1.5 block text-xs font-semibold text-slate-700">
                Alerta stock mínimo
              </label>
              <input
                type="number"
                value={minQty}
                onChange={(e) => setMinQty(e.target.value)}
                placeholder="5"
                className="w-full rounded-xl border border-slate-200 bg-slate-50/50 px-3.5 py-2.5 text-xs text-slate-800 transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-2 focus:ring-teal-700/20 focus:outline-none"
              />
            </div>
          )}
        </div>

        {/* Modal Footer Actions */}
        <div className="flex items-center justify-end gap-3 border-t border-slate-100 pt-4">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="rounded-xl border border-slate-200 px-4 py-2.5 text-xs font-semibold text-slate-600 transition-colors hover:bg-slate-50 hover:text-slate-800"
          >
            Cancelar
          </button>
          <button
            type="submit"
            disabled={busy}
            className="inline-flex items-center gap-2 rounded-xl bg-teal-700 px-5 py-2.5 text-xs font-semibold text-white shadow-sm transition-all hover:bg-teal-800 disabled:opacity-50 active:scale-98"
          >
            {busy && <Loader2 className="size-3.5 animate-spin" />}
            Guardar producto
          </button>
        </div>
      </form>
    </>
  );
}

export function ProductFormDialog({
  open,
  onOpenChange,
  product,
  branchId,
  onSaved,
}: ProductFormDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92vh] w-full max-w-xl overflow-y-auto rounded-3xl p-6 sm:p-8">
        {open && (
          <ProductFormContent
            key={product ? product.product_id : "new-product"}
            product={product}
            branchId={branchId}
            onSaved={(msg) => {
              onSaved(msg);
              onOpenChange(false);
            }}
            onCancel={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

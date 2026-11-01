"use client";

import { useEffect, useState } from "react";
import { Barcode, Loader2, Package, Tag, Users } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  type PriceTier,
  type SetProductTierPriceInput,
  type StockLevel,
} from "@/lib/api";

type TierRowState = {
  tier: PriceTier;
  price: string;
  units: string;
  barcode: string;
};

export function TierPricesDialog({
  item,
  onClose,
  onSaved,
}: {
  item: StockLevel | null;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rows, setRows] = useState<TierRowState[]>([]);

  useEffect(() => {
    if (!item) return;
    let active = true;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    setError(null);

    Promise.all([
      api.tiers.list(),
      api.tiers.getProductPrices(item.product_id),
    ])
      .then(([allTiers, productPrices]) => {
        if (!active) return;
        const activeTiers = allTiers.filter((t) => t.active);
        const priceMap = new Map(productPrices.map((p) => [p.tier_id, p]));

        const initialRows: TierRowState[] = activeTiers.map((tier) => {
          const existing = priceMap.get(tier.id);
          return {
            tier,
            price: existing?.price ? String(Number(existing.price)) : "",
            units: existing?.units_per_package
              ? String(Number(existing.units_per_package))
              : tier.units_per_package
              ? String(Number(tier.units_per_package))
              : "",
            barcode: existing?.barcode ?? "",
          };
        });
        setRows(initialRows);
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
  }, [item]);

  if (!item) return <Dialog open={false} />;

  function updateRow(tierId: string, field: "price" | "units" | "barcode", value: string) {
    setRows((prev) =>
      prev.map((r) => (r.tier.id === tierId ? { ...r, [field]: value } : r))
    );
  }

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    if (!item) return;

    setSaving(true);
    setError(null);

    try {
      const payload: SetProductTierPriceInput[] = rows
        .filter((r) => r.price.trim() || r.units.trim() || r.barcode.trim())
        .map((r) => ({
          tier_id: r.tier.id,
          price: r.price.trim() ? r.price.trim() : null,
          units_per_package: r.units.trim() ? r.units.trim() : null,
          barcode: r.barcode.trim() ? r.barcode.trim() : null,
        }));

      await api.tiers.setProductPrices(item.product_id, payload);
      onSaved(`Precios por tarifa guardados para ${item.name}`);
      onClose();
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={true} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-2xl rounded-3xl p-6">
        <DialogTitle className="flex items-center gap-2 text-xl font-bold text-slate-900">
          <Tag className="size-5 text-teal-600" />
          Precios por tarifa y presentación
        </DialogTitle>
        <DialogDescription className="text-sm text-slate-500">
          Configurá precios específicos o códigos de barras por presentación para{" "}
          <span className="font-semibold text-slate-700">{item.name}</span>.
        </DialogDescription>

        {error && (
          <div className="mt-2 rounded-xl bg-rose-50 p-3 text-xs text-rose-700 border border-rose-200">
            {error}
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center p-8 text-slate-400">
            <Loader2 className="size-6 animate-spin text-teal-600" />
            <span className="ml-2 text-sm">Cargando tarifas...</span>
          </div>
        ) : rows.length === 0 ? (
          <div className="p-8 text-center text-slate-500">
            <Tag className="mx-auto size-8 text-slate-300 mb-2" />
            <p className="text-sm">No hay tarifas activas configuradas en el negocio.</p>
          </div>
        ) : (
          <form onSubmit={handleSave} className="mt-4 space-y-4">
            <div className="max-h-96 overflow-y-auto rounded-2xl border border-slate-200 divide-y divide-slate-100">
              {rows.map((r) => (
                <div key={r.tier.id} className="p-4 space-y-3 bg-white hover:bg-slate-50/50">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-sm text-slate-900">{r.tier.name}</span>
                      {r.tier.kind === "CLIENTE" ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-2 py-0.5 text-xs font-semibold text-blue-700 border border-blue-200">
                          <Users className="size-3" /> Cliente
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-purple-50 px-2 py-0.5 text-xs font-semibold text-purple-700 border border-purple-200">
                          <Package className="size-3" /> Presentación
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                    <div>
                      <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                        Precio fijo ($)
                      </label>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        value={r.price}
                        onChange={(e) => updateRow(r.tier.id, "price", e.target.value)}
                        placeholder="Opcional"
                        className="w-full rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-800 focus:border-teal-600 focus:outline-none"
                      />
                    </div>

                    <div>
                      <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                        Unidades por empaque
                      </label>
                      <input
                        type="number"
                        step="1"
                        min="1"
                        disabled={r.tier.kind === "CLIENTE"}
                        value={r.units}
                        onChange={(e) => updateRow(r.tier.id, "units", e.target.value)}
                        placeholder={r.tier.kind === "CLIENTE" ? "1" : "Ej. 12"}
                        className="w-full rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-800 focus:border-teal-600 focus:outline-none disabled:bg-slate-100 disabled:text-slate-400"
                      />
                    </div>

                    <div>
                      <label className="block text-xs font-semibold uppercase text-slate-500 mb-1">
                        Código de barras
                      </label>
                      <div className="relative">
                        <Barcode className="absolute left-2.5 top-2.5 size-4 text-slate-400" />
                        <input
                          type="text"
                          value={r.barcode}
                          onChange={(e) => updateRow(r.tier.id, "barcode", e.target.value)}
                          placeholder="EAN / UPC"
                          className="w-full rounded-xl border border-slate-200 pl-8 pr-3 py-1.5 text-sm text-slate-800 focus:border-teal-600 focus:outline-none"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>

            <div className="flex justify-end gap-3 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded-xl border border-slate-200 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={saving}
                className="flex items-center gap-2 rounded-xl bg-teal-600 px-4 py-2 text-sm font-semibold text-white hover:bg-teal-700 disabled:opacity-50"
              >
                {saving && <Loader2 className="size-4 animate-spin" />}
                Guardar precios por tarifa
              </button>
            </div>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}

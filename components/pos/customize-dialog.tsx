"use client";

import { useMemo, useState } from "react";
import { AlertCircle, Check, Sliders, X } from "lucide-react";

import type { CatalogItem, OptionGroupDetail } from "@/lib/api";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

export function CustomizeDialog({
  open,
  item,
  optionGroups,
  onClose,
  onConfirm,
}: {
  open: boolean;
  item: CatalogItem | null;
  optionGroups: OptionGroupDetail[];
  onClose: () => void;
  onConfirm: (optionIds: string[], optionNames: string[], priceDelta: number) => void;
}) {
  const [selectedIds, setSelectedIds] = useState<string[]>([]);

  // Agrupación activa y seleccionada
  const activeGroups = useMemo(
    () => optionGroups.filter((g) => g.active && g.options.some((o) => o.active)),
    [optionGroups]
  );

  // Validación de requerimientos
  const validationErrors = useMemo(() => {
    const errors: Record<string, string> = {};
    for (const group of activeGroups) {
      const activeOptions = group.options.filter((o) => o.active);
      const chosenCount = activeOptions.filter((o) => selectedIds.includes(o.id)).length;

      if (chosenCount < group.min_select) {
        errors[group.id] = `Debes elegir al menos ${group.min_select} ${
          group.min_select === 1 ? "opción" : "opciones"
        }`;
      } else if (chosenCount > group.max_select) {
        errors[group.id] = `Máximo permitido: ${group.max_select} ${
          group.max_select === 1 ? "opción" : "opciones"
        }`;
      }
    }
    return errors;
  }, [activeGroups, selectedIds]);

  const canConfirm = Object.keys(validationErrors).length === 0;

  // Cálculo de delta total
  const { totalDelta, chosenOptionNames } = useMemo(() => {
    let delta = 0;
    const names: string[] = [];
    for (const group of activeGroups) {
      for (const opt of group.options) {
        if (selectedIds.includes(opt.id)) {
          delta += Number(opt.price_delta || 0);
          names.push(opt.name);
        }
      }
    }
    return { totalDelta: delta, chosenOptionNames: names };
  }, [activeGroups, selectedIds]);

  if (!open || !item) return null;

  function toggleOption(groupId: string, optionId: string, maxSelect: number) {
    const group = activeGroups.find((g) => g.id === groupId);
    if (!group) return;

    if (maxSelect === 1) {
      // Selección única (estilo radio button)
      const groupOptionIds = group.options.map((o) => o.id);
      const filtered = selectedIds.filter((id) => !groupOptionIds.includes(id));
      setSelectedIds([...filtered, optionId]);
    } else {
      // Selección múltiple (estilo checkbox)
      if (selectedIds.includes(optionId)) {
        setSelectedIds(selectedIds.filter((id) => id !== optionId));
      } else {
        const groupOptionIds = group.options.map((o) => o.id);
        const currentInGroup = selectedIds.filter((id) => groupOptionIds.includes(id)).length;
        if (currentInGroup < maxSelect) {
          setSelectedIds([...selectedIds, optionId]);
        }
      }
    }
  }

  function handleConfirm() {
    if (!canConfirm) return;
    onConfirm(selectedIds, chosenOptionNames, totalDelta);
    onClose();
  }

  const basePrice = Number(item.price || 0);
  const finalPrice = basePrice + totalDelta;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm">
      <div className="w-full max-w-lg max-h-[85vh] flex flex-col rounded-2xl bg-white shadow-xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between border-b border-slate-100 px-6 py-4 bg-slate-50/50">
          <div>
            <h2 className="text-base font-bold text-slate-900 flex items-center gap-2">
              <Sliders className="size-4 text-emerald-600" /> Personalizar {item.name}
            </h2>
            <p className="text-xs text-slate-500 mt-0.5">
              Precio base: {formatCOP(basePrice)}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600 transition-colors"
          >
            <X className="size-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {activeGroups.map((group) => {
            const isSingle = group.max_select === 1;
            const error = validationErrors[group.id];

            return (
              <div key={group.id} className="space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-sm font-bold text-slate-900">{group.name}</h3>
                    <p className="text-[11px] text-slate-500">
                      {group.min_select > 0 ? (
                        <span className="text-amber-600 font-semibold">Obligatorio</span>
                      ) : (
                        "Opcional"
                      )}
                      {" • "}
                      {isSingle
                        ? "Selecciona 1 opción"
                        : `Elige hasta ${group.max_select} opciones`}
                    </p>
                  </div>
                  {error && (
                    <span className="text-xs font-medium text-rose-600 flex items-center gap-1">
                      <AlertCircle className="size-3.5" /> {error}
                    </span>
                  )}
                </div>

                <div className="grid grid-cols-1 gap-2">
                  {group.options
                    .filter((o) => o.active)
                    .map((opt) => {
                      const isSelected = selectedIds.includes(opt.id);
                      const delta = Number(opt.price_delta || 0);

                      return (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => toggleOption(group.id, opt.id, group.max_select)}
                          className={cn(
                            "flex items-center justify-between p-3 rounded-xl border text-left text-sm transition-all",
                            isSelected
                              ? "border-emerald-600 bg-emerald-50/50 text-slate-900 shadow-sm"
                              : "border-slate-200 bg-white text-slate-700 hover:border-slate-300 hover:bg-slate-50/50"
                          )}
                        >
                          <div className="flex items-center gap-3">
                            <div
                              className={cn(
                                "flex size-5 items-center justify-center rounded-md border text-white transition-colors",
                                isSingle ? "rounded-full" : "rounded-md",
                                isSelected
                                  ? "border-emerald-600 bg-emerald-600"
                                  : "border-slate-300 bg-white"
                              )}
                            >
                              {isSelected && <Check className="size-3.5 stroke-[3]" />}
                            </div>
                            <span className="font-medium">{opt.name}</span>
                          </div>

                          <span
                            className={cn(
                              "text-xs font-semibold tabular-nums",
                              delta > 0 ? "text-emerald-700" : "text-slate-400"
                            )}
                          >
                            {delta > 0 ? `+${formatCOP(delta)}` : "Sin costo"}
                          </span>
                        </button>
                      );
                    })}
                </div>
              </div>
            );
          })}
        </div>

        <div className="flex items-center justify-between border-t border-slate-100 p-4 bg-slate-50/50">
          <div>
            <span className="text-xs text-slate-500">Total producto:</span>
            <div className="text-lg font-bold text-slate-900 tabular-nums">
              {formatCOP(finalPrice)}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="button"
              disabled={!canConfirm}
              onClick={handleConfirm}
              className="rounded-full bg-emerald-600 px-5 py-2 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 disabled:opacity-50 transition-colors"
            >
              Confirmar y agregar
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

"use client";

import { useState } from "react";
import { Check, Store } from "lucide-react";

import { ROLE_LABELS, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

import { primaryButtonClass } from "./auth-shell";

/** Paso 2 del login: elegir sucursal cuando el usuario trabaja en varias. */
export function BranchPicker({ onChosen }: { onChosen?: (branchId: string) => void }) {
  const { branches, branchId, selectBranch } = useSession();
  const [selected, setSelected] = useState<string | null>(branchId);

  function confirm() {
    if (!selected) return;
    selectBranch(selected);
    onChosen?.(selected);
  }

  return (
    <div className="space-y-5">
      <ul className="space-y-2" role="listbox" aria-label="Sucursales">
        {branches.map((branch) => {
          const active = branch.id === selected;
          return (
            <li key={branch.id}>
              <button
                type="button"
                role="option"
                aria-selected={active}
                onClick={() => setSelected(branch.id)}
                onDoubleClick={() => {
                  setSelected(branch.id);
                  selectBranch(branch.id);
                  onChosen?.(branch.id);
                }}
                className={cn(
                  "flex w-full items-center gap-3 rounded-2xl border p-4 text-left transition",
                  active ? "border-2 border-brand bg-teal-50" : "border-slate-200 hover:bg-slate-50",
                )}
              >
                <span className="flex size-10 items-center justify-center rounded-xl bg-slate-100 text-slate-500">
                  <Store className="size-5" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-semibold">{branch.name}</span>
                  {branch.code && <span className="block text-sm text-slate-500">{branch.code}</span>}
                </span>
                <span className="rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-600">
                  {ROLE_LABELS[branch.role] ?? branch.role}
                </span>
                {active && <Check className="size-5 text-brand" />}
              </button>
            </li>
          );
        })}
      </ul>
      <button type="button" onClick={confirm} disabled={!selected} className={primaryButtonClass}>
        Continuar
      </button>
    </div>
  );
}

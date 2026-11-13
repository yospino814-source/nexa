"use client";

import { cn } from "@/lib/utils";
import { parseCOP } from "@/lib/format";

const digits = new Intl.NumberFormat("es-CO", { maximumFractionDigits: 0 });

/** Campo de pesos: muestra "50.000" mientras se escribe y entrega el numero. */
export function MoneyInput({
  value,
  onChange,
  className,
  autoFocus,
  id,
  placeholder = "0",
}: {
  value: number;
  onChange: (value: number) => void;
  className?: string;
  autoFocus?: boolean;
  id?: string;
  placeholder?: string;
}) {
  return (
    <div className={cn("flex items-center rounded-2xl border border-slate-200 bg-white px-4 focus-within:ring-2 focus-within:ring-brand/40", className)}>
      <span className="text-slate-400">$</span>
      <input
        id={id}
        inputMode="numeric"
        autoFocus={autoFocus}
        placeholder={placeholder}
        value={value ? digits.format(value) : ""}
        onChange={(e) => onChange(parseCOP(e.target.value))}
        className="h-12 w-full bg-transparent px-2 text-lg font-semibold tabular-nums outline-none"
      />
      <span className="text-sm text-slate-400">COP</span>
    </div>
  );
}

import { cn } from "@/lib/utils";

/** Monograma "N" + nombre de la marca. */
export function BrandMark({ withName = false, className }: { withName?: boolean; className?: string }) {
  return (
    <div className={cn("flex items-center gap-2", className)}>
      <div className="flex size-9 items-center justify-center rounded-xl bg-brand text-lg font-bold text-white shadow-sm shadow-teal-900/20">
        N
      </div>
      {withName && <span className="text-lg font-bold tracking-tight text-brand">Nexa POS</span>}
    </div>
  );
}

import { cn } from "@/lib/utils";

/**
 * Símbolo de Nexa (la N con la bolsa) sobre carbón, recortado del logo.
 * `size` en px; con `withName` agrega "Nexa POS" al lado.
 */
export function BrandMark({
  withName = false,
  size = 36,
  dark = false,
  className,
}: {
  withName?: boolean;
  size?: number;
  /** Nombre en blanco, para fondos oscuros. */
  dark?: boolean;
  className?: string;
}) {
  return (
    <div className={cn("flex items-center gap-2.5", className)}>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src="/brand/nexa-mark.webp"
        alt={withName ? "" : "Nexa POS"}
        width={size}
        height={size}
        className="block shrink-0 bg-carbon object-cover"
        style={{ width: size, height: size, borderRadius: Math.round(size * 0.28) }}
      />
      {withName && (
        <span className={cn("text-[17px] font-semibold tracking-tight", dark ? "text-white" : "text-slate-900")}>
          Nexa <span className={dark ? "text-slate-400" : "text-slate-500"}>POS</span>
        </span>
      )}
    </div>
  );
}

"use client";

import { createPortal } from "react-dom";

/**
 * Contenido que solo existe en papel (tirilla termica de 80mm). En pantalla
 * esta oculto; al imprimir es lo unico visible (ver `.print-ticket` en
 * globals.css). Se monta en <body> para quedar fuera de modales y layouts.
 */
export function PrintArea({
  children,
  width = 80,
  className,
}: {
  children: React.ReactNode;
  /** Ancho del rollo en mm (ajuste `print.ticket_width_mm`): 80 o 58. */
  width?: 80 | 58;
  className?: string;
}) {
  const cls = [
    "print-ticket",
    width === 58 ? "w58" : "",
    className || "",
  ].filter(Boolean).join(" ");

  return createPortal(
    <div className={cls} aria-hidden>
      {children}
    </div>,
    document.body,
  );
}

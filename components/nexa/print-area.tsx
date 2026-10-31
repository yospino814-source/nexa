"use client";

import { createPortal } from "react-dom";

/**
 * Contenido que solo existe en papel (tirilla termica de 80mm). En pantalla
 * esta oculto; al imprimir es lo unico visible (ver `.print-ticket` en
 * globals.css). Se monta en <body> para quedar fuera de modales y layouts.
 */
export function PrintArea({ children }: { children: React.ReactNode }) {
  return createPortal(
    <div className="print-ticket" aria-hidden>
      {children}
    </div>,
    document.body,
  );
}

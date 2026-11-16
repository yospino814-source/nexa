"use client";

import { useState } from "react";
import Link from "next/link";

import { Nexi, type NexiExpression } from "@/components/nexa/nexi";

/**
 * Aviso de Nexi en la franja oscura de cada pantalla: uno solo, corto, con
 * un botón de acción. Se calcula con los datos de la pantalla (nada inventado)
 * y se puede cerrar con "Ahora no" hasta recargar.
 */
export function NexiStrip({
  children,
  action,
  href,
  onAction,
  expr = "happy",
}: {
  children: React.ReactNode;
  action: string;
  href?: string;
  onAction?: () => void;
  expr?: NexiExpression;
}) {
  const [hidden, setHidden] = useState(false);
  if (hidden) return null;
  const btn = "inline-flex h-8 shrink-0 items-center rounded-lg bg-white px-3 text-[13px] font-semibold text-slate-900 hover:bg-slate-100";
  return (
    <div className="group mt-5 flex max-w-[820px] flex-wrap items-center gap-3.5 rounded-[18px] bg-white/6 py-2.5 pr-3 pl-2.5 shadow-[inset_0_0_0_1px_rgba(255,255,255,.12)]">
      <span className="transition-transform duration-200 group-hover:-translate-y-1 group-hover:-rotate-3">
        <Nexi expr={expr} size={44} label="" />
      </span>
      <span className="min-w-0 flex-[1_1_320px] text-sm leading-[1.45] !text-[#E5E7EB]">
        <strong className="font-semibold text-[#C9B8FF]">Nexi · </strong>
        {children}
      </span>
      {href ? (
        <Link href={href} className={btn}>
          {action}
        </Link>
      ) : (
        <button type="button" onClick={onAction} className={btn}>
          {action}
        </button>
      )}
      <button
        type="button"
        onClick={() => setHidden(true)}
        className="h-8 rounded-lg px-2.5 text-[13px] font-semibold text-[#A9AFBB] hover:bg-white/8"
      >
        Ahora no
      </button>
    </div>
  );
}

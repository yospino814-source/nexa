import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

const TONES = {
  default: "bg-indigo-50 text-indigo-600",
  success: "bg-emerald-50 text-emerald-600",
  warning: "bg-amber-50 text-amber-600",
  danger: "bg-rose-50 text-rose-600",
} as const;

/** Tarjeta de indicador: 1 dato grande, 1 subtexto chico y gris. */
export function StatCard({
  title,
  value,
  caption,
  icon: Icon,
  tone = "default",
  badge,
}: {
  title: string;
  value: string;
  caption: string;
  icon: LucideIcon;
  tone?: keyof typeof TONES;
  badge?: { label: string; tone: "up" | "down" | "neutral" | "warning" };
}) {
  return (
    <div className="flex min-w-0 flex-col gap-2 rounded-2xl border border-slate-200 bg-white px-[22px] py-5 shadow-card">
      <div className="flex items-center justify-between gap-3">
        <p className="text-[13px] font-medium text-slate-500">{title}</p>
        <span className={cn("flex size-10 items-center justify-center rounded-xl", TONES[tone])}>
          <Icon className="size-5" aria-hidden />
        </span>
      </div>
      <p className="text-[28px] font-semibold tracking-[-0.025em] tabular-nums">{value}</p>
      <div className="flex items-center justify-between gap-2">
        <p className="text-[13px] text-slate-500">{caption}</p>
        {badge && (
          <span
            className={cn(
              "inline-flex h-6 shrink-0 items-center rounded-full px-2.5 text-xs font-semibold",
              badge.tone === "up" && "bg-emerald-50 text-emerald-600",
              badge.tone === "down" && "bg-rose-50 text-rose-600",
              badge.tone === "neutral" && "bg-slate-100 text-slate-600",
              badge.tone === "warning" && "bg-amber-50 text-amber-700",
            )}
          >
            {badge.label}
          </span>
        )}
      </div>
    </div>
  );
}

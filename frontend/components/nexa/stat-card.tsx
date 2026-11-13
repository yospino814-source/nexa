import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

const TONES = {
  default: "bg-slate-100 text-slate-600",
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
    <div className="rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm">
      <div className="flex items-center justify-between">
        <p className="text-sm text-slate-500">{title}</p>
        <span className={cn("flex size-10 items-center justify-center rounded-full", TONES[tone])}>
          <Icon className="size-5" />
        </span>
      </div>
      <p className="mt-4 text-3xl font-bold tracking-tight tabular-nums">{value}</p>
      <div className="mt-3 flex items-center justify-between gap-2">
        <p className="text-sm text-slate-400">{caption}</p>
        {badge && (
          <span
            className={cn(
              "shrink-0 rounded-full px-2.5 py-0.5 text-xs font-semibold",
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

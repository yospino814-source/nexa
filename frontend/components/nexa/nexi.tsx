import { useId } from "react";

import { cn } from "@/lib/utils";

export type NexiExpression = "idle" | "happy" | "think" | "alert" | "love" | "wink" | "sad" | "sleep";

type Face = {
  /** Ojos rellenos */ ef: string;
  /** Ojos de trazo */ es: string;
  ec: string;
  /** Boca rellena */ mf: string;
  /** Boca de trazo */ ms: string;
  /** Detalle alrededor (chispas, corazones, globo) */ xf: string;
  xc: string;
  xs: string;
};

const FACES: Record<NexiExpression, Face> = {
  idle: { ef: "M59.5 71a4.5 4.5 0 0 1 9 0v8a4.5 4.5 0 0 1 -9 0zM91.5 71a4.5 4.5 0 0 1 9 0v8a4.5 4.5 0 0 1 -9 0z", es: "", ec: "#3EE0F5", mf: "", ms: "M72 89q8 6 16 0", xf: "", xc: "#3EE0F5", xs: "" },
  happy: { ef: "", es: "M57 79q7-11 14 0M89 79q7-11 14 0", ec: "#3EE0F5", mf: "M70 87q10 12 20 0z", ms: "", xf: "M140 27l1.96 5.04 5.04 1.96 -5.04 1.96 -1.96 5.04 -1.96 -5.04 -5.04 -1.96 5.04 -1.96zM26 39l1.4 3.6 3.6 1.4 -3.6 1.4 -1.4 3.6 -1.4 -3.6 -3.6 -1.4 3.6 -1.4z", xc: "#3EE0F5", xs: "" },
  think: { ef: "M64.5 67a4.5 4.5 0 0 1 9 0v6a4.5 4.5 0 0 1 -9 0zM96.5 67a4.5 4.5 0 0 1 9 0v6a4.5 4.5 0 0 1 -9 0z", es: "", ec: "#3EE0F5", mf: "", ms: "M74 91h12", xf: "M136 36a4 4 0 1 0 .1 0zM146 24a5 5 0 1 0 .1 0zM151 9a6 6 0 1 0 .1 0z", xc: "#3EE0F5", xs: "" },
  alert: { ef: "M64 66.5a7.5 7.5 0 1 0 .1 0zM96 66.5a7.5 7.5 0 1 0 .1 0z", es: "", ec: "#3EE0F5", mf: "M80 87.5a3.5 3.5 0 1 0 .1 0z", ms: "", xf: "M128 14h20a6 6 0 0 1 6 6v14a6 6 0 0 1-6 6h-20a6 6 0 0 1-6-6v-14a6 6 0 0 1 6-6z", xc: "#F5A524", xs: "M138 20v8M138 34v.5" },
  love: { ef: "M64 80.05C52.45 71.8 58.5 63 64 70.15C69.5 63 75.55 71.8 64 80.05ZM96 80.05C84.45 71.8 90.5 63 96 70.15C101.5 63 107.55 71.8 96 80.05Z", es: "", ec: "#FF8FB8", mf: "", ms: "M71 88q9 9 18 0", xf: "M142 36.4C133.6 30.4 138 24 142 29.2C146 24 150.4 30.4 142 36.4ZM24 51.3C17.7 46.8 21 42 24 45.9C27 42 30.3 46.8 24 51.3Z", xc: "#FF8FB8", xs: "" },
  wink: { ef: "M59.5 71a4.5 4.5 0 0 1 9 0v8a4.5 4.5 0 0 1 -9 0z", es: "M89 78q7-9 14 0", ec: "#3EE0F5", mf: "", ms: "M71 88q9 8 18 0", xf: "M140 24l1.68 4.32 4.32 1.68 -4.32 1.68 -1.68 4.32 -1.68 -4.32 -4.32 -1.68 4.32 -1.68z", xc: "#3EE0F5", xs: "" },
  sad: { ef: "", es: "M57 72q7 8 14 0M89 72q7 8 14 0", ec: "#3EE0F5", mf: "", ms: "M72 94q8-6 16 0", xf: "", xc: "#3EE0F5", xs: "" },
  sleep: { ef: "", es: "M57 77q7 4 14 0M89 77q7 4 14 0", ec: "#A9A6B8", mf: "", ms: "M75 91h10", xf: "", xc: "#A9A6B8", xs: "M132 26h9l-9 9h9M144 12h6l-6 6h6" },
};

const BODY = "M80 30C118 30 140 50 140 78C140 106 118 124 80 124H62L40 146L46 120C30 112 20 96 20 78C20 50 42 30 80 30Z";

/**
 * Nexi 2.0, la asistente de Nexa: una burbuja de chat violeta con visor oscuro
 * y ojos cian. Solo cambian ojos, boca y un detalle alrededor según la
 * expresión. Desde 32 px va completa; más chica usá `simple`.
 */
export function Nexi({
  expr = "idle",
  size = 96,
  wave = false,
  simple = false,
  animated = false,
  label = "Nexi, la asistente de Nexa",
  className,
}: {
  expr?: NexiExpression;
  size?: number;
  wave?: boolean;
  simple?: boolean;
  /** Respira y parpadea (se apaga con movimiento reducido). */
  animated?: boolean;
  /** Vacío: decorativa, oculta a lectores de pantalla. */
  label?: string;
  className?: string;
}) {
  const id = useId().replace(/:/g, "");
  const a11y = label ? { role: "img", "aria-label": label } : { "aria-hidden": true };

  if (simple) {
    return (
      <svg {...a11y} width={size} height={size} viewBox="14 24 132 128" className={cn("block shrink-0", className)}>
        <path d={BODY} fill="#6C4DF6" />
        <rect x="38" y="50" width="84" height="54" rx="26" fill="#1C1A3A" />
        <path d="M58 72a6 6 0 0 1 12 0v10a6 6 0 0 1 -12 0zM90 72a6 6 0 0 1 12 0v10a6 6 0 0 1 -12 0z" fill="#3EE0F5" />
      </svg>
    );
  }

  const f = FACES[expr];
  const off = expr === "sleep";
  const top = off ? "#6B6880" : "#8B6CFF";
  const deep = off ? "#4B4860" : "#3D3BD8";
  const spark = off ? "#A9A6B8" : "#3EE0F5";
  const mouth = off ? "#A9A6B8" : "#3EE0F5";
  const cheeks = off ? 0 : 0.55;

  return (
    <svg
      {...a11y}
      width={size}
      height={Math.round((size * 170) / 160)}
      viewBox="0 0 160 170"
      className={cn("block shrink-0 overflow-visible", className)}
    >
      <defs>
        <linearGradient id={`nxg${id}`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={top} />
          <stop offset="1" stopColor={deep} />
        </linearGradient>
        <clipPath id={`nxc${id}`}>
          <path d={BODY} />
        </clipPath>
      </defs>
      <ellipse cx="80" cy="162" rx="44" ry="6" fill="#141629" opacity=".14" />
      <g className={animated && !off ? "nx-body" : undefined}>
        <ellipse cx="22" cy="92" rx="9" ry="13" fill={deep} transform="rotate(18 22 92)" />
        <ellipse
          cx={wave ? 140 : 138}
          cy={wave ? 62 : 92}
          rx="9"
          ry={wave ? 14 : 13}
          fill={deep}
          transform={wave ? "rotate(-28 140 62)" : "rotate(-18 138 92)"}
        />
        <path d="M80 31V17" stroke={deep} strokeWidth="4" strokeLinecap="round" />
        <circle cx="80" cy="11" r="10" fill={spark} opacity={off ? 0 : 0.28} />
        <path d="M80 4l1.96 5.04 5.04 1.96 -5.04 1.96 -1.96 5.04 -1.96 -5.04 -5.04 -1.96 5.04 -1.96z" fill={spark} />
        <path d={BODY} fill={`url(#nxg${id})`} />
        <g clipPath={`url(#nxc${id})`}>
          <ellipse cx="96" cy="128" rx="58" ry="22" fill={deep} opacity=".55" />
        </g>
        <path d="M36 64C40 48 54 39 70 37" fill="none" stroke="#ffffff" strokeOpacity=".38" strokeWidth="5" strokeLinecap="round" />
        <rect x="40" y="52" width="80" height="50" rx="24" fill="#1C1A3A" />
        <rect x="41.5" y="53.5" width="77" height="47" rx="22.5" fill="none" stroke="#ffffff" strokeOpacity=".08" strokeWidth="1.5" />
        <ellipse cx="40" cy="111" rx="7" ry="4" fill="#FF8FB8" opacity={cheeks} />
        <ellipse cx="120" cy="111" rx="7" ry="4" fill="#FF8FB8" opacity={cheeks} />
        <g className={animated && f.ef ? "nx-eyes" : undefined}>
          {f.ef && <path d={f.ef} fill={f.ec} />}
          {f.es && <path d={f.es} fill="none" stroke={f.ec} strokeWidth="4.5" strokeLinecap="round" />}
        </g>
        {f.mf && <path d={f.mf} fill={mouth} />}
        {f.ms && <path d={f.ms} fill="none" stroke={mouth} strokeWidth="3.6" strokeLinecap="round" />}
        {f.xf && <path d={f.xf} fill={f.xc} />}
        {f.xs && (
          <path
            d={f.xs}
            fill="none"
            stroke={expr === "alert" ? "#1A1D24" : "#A9A6B8"}
            strokeWidth={expr === "alert" ? 3.6 : 2.6}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )}
      </g>
    </svg>
  );
}

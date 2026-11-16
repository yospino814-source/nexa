import { useId } from "react";

/**
 * Ilustración del producto en el POS cuando no tiene foto: la silueta del
 * envase (botella, lata, bolsa…) sobre un fondo suave del color de su tipo.
 * Se elige por palabras del nombre y, si no hay, por la categoría.
 */

type Shape = "botella" | "vino" | "lata" | "six" | "pet" | "bolsa" | "caja";

const SHAPES: Record<Shape, [body: string, label: string]> = {
  botella: ["M26 6h12v16c0 4 8 7 8 16v46a6 6 0 0 1-6 6H24a6 6 0 0 1-6-6V38c0-9 8-12 8-16z", "M18 50h28v20H18z"],
  vino: ["M28 4h8v22c0 5 10 8 10 18v42a6 6 0 0 1-6 6H24a6 6 0 0 1-6-6V44c0-10 10-13 10-18z", "M18 58h28v16H18z"],
  lata: ["M20 18h24l3 5v54l-3 5H20l-3-5V23z", "M17 38h30v24H17z"],
  six: [
    "M4 24h16l2 4v50l-2 4H4l-2-4V28zM24 24h16l2 4v50l-2 4H24l-2-4V28zM44 24h16l2 4v50l-2 4H44l-2-4V28z",
    "M2 48h60v14H2z",
  ],
  pet: [
    "M27 6h10v8c0 3 9 6 9 14v8c-2 2-2 4 0 6v6c-2 2-2 4 0 6v24a6 6 0 0 1-6 6H24a6 6 0 0 1-6-6V54c2-2 2-4 0-6v-6c2-2 2-4 0-6v-8c0-8 9-11 9-14z",
    "M18 56h28v14H18z",
  ],
  bolsa: ["M12 16q20-8 40 0l-2 66q-18 8-36 0z", "M16 40h32v20H16z"],
  caja: ["M16 20h32v66H16z", "M16 20h32v18H16z"],
};

/** Tonos de la marca: [silueta, fondo]. */
const TONES = {
  naranja: ["#FF7A3D", "#FFF3EC"],
  rosa: ["#FF3D8B", "#FFEFF5"],
  violeta: ["#6C3EF5", "#F3EFFF"],
  cian: ["#1EC8F0", "#EAF9FD"],
  indigo: ["#3D4FE0", "#EEF0FF"],
} as const;
type Tone = keyof typeof TONES;

const RULES: [RegExp, Shape, Tone][] = [
  [/six ?pack|x ?6\b/i, "six", "cian"],
  [/caja x|canasta|x ?24\b/i, "caja", "cian"],
  [/aguardiente|guaro/i, "botella", "naranja"],
  [/\bron\b/i, "botella", "rosa"],
  [/whisk|vodka|tequila|ginebra|brandy/i, "botella", "violeta"],
  [/vino|champa|espumoso/i, "vino", "rosa"],
  [/cerveza|lata|\b330\b|águila|aguila|club colombia|poker|costeña/i, "lata", "cian"],
  [/coca|gaseosa|postob|pony|agua|jugo|soda|leche|yogur|\b1[,.]5 ?l\b|\b\d+ ?ml\b/i, "pet", "indigo"],
  [/hielo|papas|snack|chito|detod|bolsa|arroz|azúcar|azucar|café|cafe|frijol|lenteja/i, "bolsa", "naranja"],
  [/cigarr|lucky|marlboro|boston/i, "caja", "violeta"],
];

const CATEGORY_TONE: Record<string, Tone> = {
  Bebidas: "indigo",
  Licores: "violeta",
  Cervezas: "cian",
  Abarrotes: "naranja",
  Lácteos: "indigo",
  Panadería: "naranja",
  "Aseo y Hogar": "cian",
  "Snacks y Dulces": "rosa",
};

export function productArt(name: string, category: string | null): { shape: Shape; tone: Tone } {
  for (const [re, shape, tone] of RULES) {
    if (re.test(name)) return { shape, tone };
  }
  return { shape: "caja", tone: (category && CATEGORY_TONE[category]) || "indigo" };
}

/** Mezcla un color con blanco (t > 0) o negro (t < 0). */
function tint(hex: string, t: number): string {
  const n = parseInt(hex.slice(1), 16);
  const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) =>
    Math.round(t >= 0 ? v + (255 - v) * t : v * (1 + t)),
  );
  return `#${c.map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

/** Tapas, cuellos y bordes que le dan el "envase real" a cada forma. */
const CAPS: Partial<Record<Shape, { d: string; metal?: boolean }>> = {
  botella: { d: "M25 1.5h14v12.5H25z" },
  vino: { d: "M27.2 0h9.6v25H27.2z", metal: true },
  pet: { d: "M26.5 1.5h11v8h-11z" },
  lata: { d: "M20 18h24l3 5H17z", metal: true },
  bolsa: { d: "M12 16q20-8 40 0l-.2 6q-19.8-8-39.6 0z" },
};

/**
 * Envase dibujado con volumen: vidrio o lata con sombra en los bordes, brillo
 * vertical, tapa, etiqueta y, sin fondo, su reflejo en el piso.
 */
export function ProductArt({
  name,
  category,
  muted = false,
  height = 104,
  bare = false,
}: {
  name: string;
  category: string | null;
  muted?: boolean;
  height?: number;
  /** Sin fondo de color: el envase solo con su reflejo, para tarjetas con fondo propio. */
  bare?: boolean;
}) {
  const id = useId().replace(/:/g, "");
  const { shape, tone } = productArt(name, category);
  const [base, tileBg] = muted ? ["#A9AEBA", "#EEF0F4"] : TONES[tone];
  const [body, label] = SHAPES[shape];
  const cap = CAPS[shape];
  const vbH = bare ? 124 : 96;
  const iconH = Math.round(height * (bare ? 0.98 : 0.85));
  const glass = shape === "botella" || shape === "vino" || shape === "pet";
  return (
    <span
      aria-hidden
      className="grid place-items-center overflow-hidden rounded-xl"
      style={{ height, background: bare ? "transparent" : tileBg }}
    >
      <svg width={Math.round((iconH * 64) / vbH)} height={iconH} viewBox={`0 0 64 ${vbH}`}>
        <defs>
          <linearGradient id={`b${id}`} x1="0" x2="1">
            <stop offset="0" stopColor={tint(base, -0.55)} />
            <stop offset=".16" stopColor={tint(base, -0.12)} />
            <stop offset=".38" stopColor={tint(base, 0.32)} />
            <stop offset=".52" stopColor={base} />
            <stop offset=".82" stopColor={tint(base, -0.28)} />
            <stop offset="1" stopColor={tint(base, -0.6)} />
          </linearGradient>
          <linearGradient id={`l${id}`} x1="0" x2="1">
            <stop offset="0" stopColor="#D9D2C2" />
            <stop offset=".35" stopColor="#FFFDF7" />
            <stop offset=".7" stopColor="#F4EFE4" />
            <stop offset="1" stopColor="#CFC7B5" />
          </linearGradient>
          <linearGradient id={`c${id}`} x1="0" x2="1">
            <stop offset="0" stopColor={cap?.metal ? "#7D8494" : "#0E1013"} />
            <stop offset=".4" stopColor={cap?.metal ? "#F2F4F8" : "#4A505C"} />
            <stop offset="1" stopColor={cap?.metal ? "#8A91A0" : "#0E1013"} />
          </linearGradient>
          <linearGradient id={`f${id}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#fff" stopOpacity=".55" />
            <stop offset=".6" stopColor="#fff" stopOpacity="0" />
          </linearGradient>
          <filter id={`s${id}`} x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation=".7" />
          </filter>
          {bare && (
            <linearGradient id={`r${id}`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor="#fff" stopOpacity=".25" />
              <stop offset=".5" stopColor="#fff" stopOpacity="0" />
            </linearGradient>
          )}
          {bare && (
            <mask id={`m${id}`}>
              <rect x="0" y="96" width="64" height="28" fill={`url(#r${id})`} />
            </mask>
          )}
        </defs>
        {bare && (
          <g mask={`url(#m${id})`} transform="translate(0 192) scale(1 -1)">
            <path d={body} fill={`url(#b${id})`} />
          </g>
        )}
        <path d={body} fill={`url(#b${id})`} />
        {glass && <path d={body} fill={`url(#f${id})`} opacity=".35" />}
        <path d={label} fill={`url(#l${id})`} />
        <path d={label} fill="none" stroke={tint(base, -0.35)} strokeOpacity=".35" strokeWidth=".6" />
        <LabelMarks label={label} color={muted ? "#8A90A0" : tint(base, -0.35)} />
        {cap && <path d={cap.d} fill={`url(#c${id})`} />}
        <path
          d={shape === "lata" || shape === "six" || shape === "caja" ? "M22 26v50" : "M23 40q-1 22 0 42"}
          stroke="#fff"
          strokeOpacity={glass ? 0.7 : 0.45}
          strokeWidth="2.6"
          strokeLinecap="round"
          fill="none"
          filter={`url(#s${id})`}
        />
      </svg>
    </span>
  );
}

/** Dos rayitas que simulan el texto de la etiqueta, centradas en ella. */
function LabelMarks({ label, color }: { label: string; color: string }) {
  // Todas las etiquetas son rectángulos "Mx yhWvH…".
  const m = /^M(\d+) (\d+)h(\d+)v(\d+)/.exec(label);
  if (!m) return null;
  const [x, y, w, h] = m.slice(1).map(Number);
  const cx = x + w / 2;
  const cy = y + h / 2;
  return (
    <g fill={color}>
      <rect x={cx - w * 0.3} y={cy - 3.2} width={w * 0.6} height="2.4" rx="1.2" />
      <rect x={cx - w * 0.2} y={cy + 1.6} width={w * 0.4} height="1.4" rx=".7" opacity=".6" />
    </g>
  );
}

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
  /** Sin fondo de color: la silueta sola, para tarjetas con fondo propio. */
  bare?: boolean;
}) {
  const { shape, tone } = productArt(name, category);
  const [fill, tileBg] = muted ? ["#B8BCC8", "#EEF0F4"] : TONES[tone];
  const bg = bare ? "transparent" : tileBg;
  const [body, label] = SHAPES[shape];
  const iconH = Math.round(height * 0.85);
  return (
    <span
      aria-hidden
      className="grid place-items-center overflow-hidden rounded-xl"
      style={{ height, background: bg }}
    >
      <svg
        width={Math.round((iconH * 64) / 96)}
        height={iconH}
        viewBox="0 0 64 96"
        style={{ filter: "drop-shadow(0 8px 10px rgba(20,22,28,.16))" }}
      >
        <path d={body} fill={fill} />
        <path d={label} fill="#FFFFFF" fillOpacity=".75" />
      </svg>
    </span>
  );
}

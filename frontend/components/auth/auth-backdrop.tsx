"use client";

import { useState } from "react";
import { Check, Pause, Play, Truck } from "lucide-react";

const SHAPES: Record<string, [string, string]> = {
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

const PRODS: [string, string, string][] = [
  ["botella", "#FF7A3D", "$62.000"],
  ["lata", "#1EC8F0", "$3.200"],
  ["botella", "#FF3D8B", "$58.000"],
  ["six", "#1EC8F0", "$18.500"],
  ["botella", "#6C3EF5", "$165.000"],
  ["pet", "#3D4FE0", "$6.500"],
  ["vino", "#FF3D8B", "$32.000"],
  ["bolsa", "#1EC8F0", "$4.500"],
  ["lata", "#FF7A3D", "$3.800"],
  ["bolsa", "#FF7A3D", "$5.200"],
  ["caja", "#6C3EF5", "$11.000"],
  ["pet", "#1EC8F0", "$2.000"],
];
const SALES = ["$150.500", "$62.000", "$37.000", "$124.000", "$18.500", "$9.000"];
const BUYS = ["+48 und", "+24 und", "+12 und", "+96 und"];

type Tile = { kind: "prod" | "sale" | "buy"; shape?: string; fill?: string; text: string };

const ROWS: { cls: string; items: Tile[] }[] = [0, 1, 2, 3, 4, 5, 6].map((ri) => {
  const one: Tile[] = [];
  for (let i = 0; i < 12; i++) {
    const k = (i * 5 + ri * 3) % 12;
    const kind = (i + ri) % 6 === 2 ? "sale" : (i + ri * 2) % 9 === 5 ? "buy" : "prod";
    if (kind === "sale") one.push({ kind, text: SALES[(i * 7 + ri * 5) % SALES.length] });
    else if (kind === "buy") one.push({ kind, text: BUYS[(i + ri) % BUYS.length] });
    else one.push({ kind, shape: PRODS[k][0], fill: PRODS[k][1], text: PRODS[k][2] });
  }
  const cls = ["auth-track", ri % 2 ? "auth-rev" : "", ri % 3 === 1 ? "auth-s2" : ri % 3 === 2 ? "auth-s3" : ""].join(" ");
  return { cls, items: one.concat(one) };
});

/**
 * Fondo de las pantallas de acceso: manchas de color de la marca y una pared
 * de productos, ventas y compras que se desliza. Se puede pausar y respeta
 * "reducir movimiento".
 */
export function AuthBackdrop() {
  const [paused, setPaused] = useState(false);
  return (
    <>
      <div aria-hidden className={`pointer-events-none absolute inset-0 overflow-hidden ${paused ? "auth-paused" : ""}`}>
        <div className="auth-blob" style={{ width: 620, height: 620, left: -160, top: -200, background: "#FF7A3D", opacity: 0.32 }} />
        <div
          className="auth-blob"
          style={{ width: 700, height: 700, right: -200, top: -120, background: "#6C3EF5", opacity: 0.4, animationDuration: "26s" }}
        />
        <div
          className="auth-blob"
          style={{ width: 600, height: 600, left: "30%", bottom: -320, background: "#1EC8F0", opacity: 0.22, animationDuration: "23s" }}
        />
        <div className="auth-wall absolute top-1/2 left-1/2 -mt-[760px] -ml-[1300px] w-[2600px]">
          <div className="flex flex-col gap-[18px]" style={{ transform: "rotate(-12deg) skewX(8deg)" }}>
            {ROWS.map((row, ri) => (
              <div key={ri} className={row.cls}>
                {row.items.map((t, i) => (
                  <WallTile key={i} tile={t} />
                ))}
              </div>
            ))}
          </div>
        </div>
        <div className="absolute inset-0 bg-[radial-gradient(46%_62%_at_50%_50%,rgba(14,15,20,.92)_0%,rgba(14,15,20,.72)_45%,rgba(14,15,20,.15)_100%)]" />
      </div>
      <button
        type="button"
        onClick={() => setPaused((p) => !p)}
        aria-pressed={paused}
        aria-label={paused ? "Reanudar animación de fondo" : "Pausar animación de fondo"}
        className="absolute bottom-5 left-5 z-10 grid size-11 place-items-center rounded-full bg-white/8 text-[#E6E4EE] ring-1 ring-white/14"
      >
        {paused ? <Play className="size-4 fill-current" /> : <Pause className="size-4 fill-current" />}
      </button>
    </>
  );
}

function WallTile({ tile }: { tile: Tile }) {
  const base = "flex h-[168px] w-[150px] flex-none flex-col items-center justify-center gap-3 rounded-3xl";
  if (tile.kind === "sale") {
    return (
      <div className={`${base} bg-white shadow-[0_20px_40px_-18px_rgba(0,0,0,.6)]`}>
        <span className="grid size-[34px] place-items-center rounded-full bg-emerald-500">
          <Check className="size-[18px] text-white" strokeWidth={3} />
        </span>
        <span className="auth-display text-[17px] font-semibold tracking-[-0.02em] text-[#15131C] tabular-nums">{tile.text}</span>
      </div>
    );
  }
  if (tile.kind === "buy") {
    return (
      <div className={`${base} bg-[rgba(30,200,240,.08)] shadow-[inset_0_0_0_1px_rgba(30,200,240,.28)]`}>
        <span className="grid size-[34px] place-items-center rounded-[10px] bg-[rgba(30,200,240,.18)] text-[#8BE4F8]">
          <Truck className="size-5" strokeWidth={1.9} />
        </span>
        <span className="text-[15px] font-semibold text-[#8BE4F8] tabular-nums">{tile.text}</span>
      </div>
    );
  }
  const [body, label] = SHAPES[tile.shape ?? "caja"];
  return (
    <div className={`${base} bg-white/[.045] shadow-[inset_0_0_0_1px_rgba(255,255,255,.08)]`}>
      <span className="grid h-[84px] w-[76px] place-items-center rounded-[18px] bg-white/5">
        <svg width="44" height="66" viewBox="0 0 64 96">
          <path d={body} fill={tile.fill} />
          <path d={label} fill="#FFFFFF" fillOpacity=".7" />
        </svg>
      </span>
      <span className="text-[15px] font-semibold text-white/85 tabular-nums">{tile.text}</span>
    </div>
  );
}

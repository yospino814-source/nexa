import type { Metadata } from "next";
import Link from "next/link";

import { Topbar } from "@/components/nexa/topbar";
import { modules, screens, type ModuleId } from "@/lib/screens";
import { cn } from "@/lib/utils";

export const metadata: Metadata = { title: "Mapa de pantallas · Nexa POS" };

const LAYOUT_LABEL = { admin: "Administración", operacion: "Operación", publico: "Público" } as const;

/** Indice de las 30 pantallas: estado de migracion y acceso directo a cada ruta. */
export default function ModulesPage() {
  const migrated = screens.filter((s) => s.status === "react").length;

  return (
    <>
      <Topbar title="Mapa de pantallas" />
      <div className="space-y-10 p-6 lg:p-10">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">Módulos de Nexa POS</h2>
            <p className="text-sm text-slate-500">
              Las pantallas “Diseño” muestran la referencia de Stitch hasta que se migren a React.
            </p>
          </div>
          <div className="w-64">
            <div className="mb-1 flex justify-between text-xs text-slate-500">
              <span>Migradas a React</span>
              <span className="font-semibold text-slate-700">
                {migrated} / {screens.length}
              </span>
            </div>
            <div className="h-2 rounded-full bg-slate-200">
              <div className="h-2 rounded-full bg-brand" style={{ width: `${(migrated / screens.length) * 100}%` }} />
            </div>
          </div>
        </div>

        {(Object.keys(modules) as ModuleId[]).map((moduleId) => (
          <section key={moduleId}>
            <h3 className="mb-4 text-sm font-semibold tracking-wider text-slate-500 uppercase">
              {modules[moduleId].title}
            </h3>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
              {screens
                .filter((s) => s.module === moduleId)
                .map((screen) => (
                  <Link
                    key={screen.id}
                    href={screen.route}
                    className="group overflow-hidden rounded-2xl border border-slate-200/60 bg-white shadow-sm transition-shadow hover:shadow-md"
                  >
                    <div className="aspect-[5/4] overflow-hidden border-b border-slate-100 bg-slate-100">
                      {/* eslint-disable-next-line @next/next/no-img-element */}
                      <img
                        src={`/design/${screen.design}.png`}
                        alt=""
                        loading="lazy"
                        className="h-full w-full object-cover object-top transition-transform group-hover:scale-[1.02]"
                      />
                    </div>
                    <div className="space-y-1 p-4">
                      <div className="flex items-center justify-between gap-2">
                        <p className="font-medium">
                          <span className="text-slate-400">{screen.id}</span> {screen.title}
                        </p>
                        <span
                          className={cn(
                            "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-semibold",
                            screen.status === "react" ? "bg-emerald-50 text-emerald-700" : "bg-slate-100 text-slate-500",
                          )}
                        >
                          {screen.status === "react" ? "React" : "Diseño"}
                        </span>
                      </div>
                      <p className="text-xs text-slate-400">
                        {screen.route} · {LAYOUT_LABEL[screen.layout]}
                        {screen.note && ` · ${screen.note}`}
                      </p>
                    </div>
                  </Link>
                ))}
            </div>
          </section>
        ))}
      </div>
    </>
  );
}

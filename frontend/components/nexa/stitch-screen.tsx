import Link from "next/link";
import { LayoutGrid } from "lucide-react";

import type { Screen } from "@/lib/screens";

/** Pantalla de referencia: el HTML de Stitch a pantalla completa. */
export function StitchScreen({ screen }: { screen: Screen }) {
  return (
    <div className="relative h-screen w-full">
      <iframe
        src={`/design/${screen.design}.html`}
        title={screen.title}
        className="h-full w-full border-0"
      />
      <Link
        href="/modulos"
        className="fixed bottom-4 left-1/2 z-50 flex -translate-x-1/2 items-center gap-2 rounded-full bg-slate-900/90 px-4 py-2 text-xs font-medium text-white shadow-lg backdrop-blur hover:bg-slate-900"
      >
        <LayoutGrid className="size-3.5" />
        {screen.id} · {screen.title} — diseño de referencia, pendiente de migrar
      </Link>
    </div>
  );
}

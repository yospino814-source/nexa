# M16A Corrección: `npm run build` falla y la app no se puede desplegar

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorio | Solo `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Archivos | `app/(admin)/layout.tsx` y `app/(operacion)/layout.tsx` |
| Commit | `fix(fase-s): build de produccion con Suspense en los layouts` |
| Reporte | Sección "M16A" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M16A de Nexa POS: una corrección chica. Hacé solo esto.

1. Leé completo antigravity/modulos/M16A-correccion-build.md.
2. Trabajá solo en nexa-frontend, en la rama feat/fase-s-salida.
3. Primero, descartá los dos cambios sin commit que quedaron de M16: son solo un salto de línea al final de components/auth/route-guard.tsx y de lib/route-permissions.ts. Revisá con git diff que sea solo eso y corré:
   git checkout -- components/auth/route-guard.tsx lib/route-permissions.ts
   No uses reset, rebase ni stash.
4. Hacé exactamente el cambio de "Qué hacer". No toques ningún otro archivo.
5. Corré la verificación completa. Las tres órdenes deben terminar sin errores.
6. Commit con el mensaje exacto del módulo.
7. Escribí la sección "M16A" en antigravity/REPORTE-FASE-S.md.
8. Detenete. Escribime "M16A terminado". Después sigue M17.
```

## Problema (comprobado por Claude el 2026-11-10)

`npx next build` termina con error:

```
Error: Route "/clientes/[id]": Next.js encountered URL data `usePathname()` in a Client Component outside of `<Suspense>`.
    at components\auth\session-gate.tsx:18:31
Export encountered an error on /(admin)/inventario/producto/[id]/page
```

`next.config.ts` tiene `cacheComponents: true`. Con esa opción, un componente cliente que usa `usePathname()` fuera de `<Suspense>` no deja prerenderizar las rutas dinámicas. `SessionGate`, `RouteGuard` y `Sidebar` lo usan, y los layouts los montan sin `<Suspense>`.

El problema viene de antes de la fase S, porque `SessionGate` usa `usePathname()` desde `f300d2d`. Sin el build, la app no se puede desplegar en el VPS.

Claude probó este mismo cambio en su copia: con él, el build pasa entero.

## Qué hacer

En **los dos** layouts, envolvé `<SessionGate>` en `<Suspense fallback={null}>`, importado de `react`. No cambies nada más.

`app/(admin)/layout.tsx` queda así:

```tsx
import { Suspense } from "react";

import { RouteGuard } from "@/components/auth/route-guard";
import { SessionGate } from "@/components/auth/session-gate";
import { Sidebar } from "@/components/nexa/sidebar";

/** (conservá el comentario que ya tiene el archivo) */
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <Suspense fallback={null}>
      <SessionGate>
        <RouteGuard>
          <div className="flex min-h-screen w-full bg-slate-50 text-slate-900">
            <Sidebar />
            <main className="min-w-0 flex-1">{children}</main>
          </div>
        </RouteGuard>
      </SessionGate>
    </Suspense>
  );
}
```

`app/(operacion)/layout.tsx` lleva el mismo cambio, alrededor de su `<SessionGate>` actual (con `OperationTopbar` adentro).

No agregues `export const instant = false`. No toques `next.config.ts` ni `session-gate.tsx`.

## Verificación

```sh
cd nexa-frontend && npx tsc --noEmit && npm run lint && npx next build
```

El build debe terminar con la tabla de rutas. En ella, `/clientes/[id]` y `/inventario/producto/[id]` aparecen como "◐ Partial Prerender". **No mates los procesos de los puertos 3000 ni 3100**: el build no los necesita.

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M16A`:
1. El hash del commit.
2. La salida de `tsc` y de `lint`.
3. Las últimas 25 líneas de `npx next build`.

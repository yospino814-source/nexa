# nexa-frontend

Frontend de Nexa POS: Next.js 16 (App Router), Tailwind y shadcn/ui.

## Correr

```sh
cp .env.example .env.local   # opcional: sin variables corre en modo demo
npm install
npm run dev                  # http://localhost:3000
```

| Modo | Variables | Qué pasa |
|---|---|---|
| Demo | ninguna | Sin login, con datos mock. |
| Login + mocks | `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY` | Login real con Supabase, datos mock. |
| Completo | las anteriores + `BACKEND_URL`, `NEXT_PUBLIC_USE_BACKEND=true` | Login real y backend Rust en `/api/*`, que Next reenvía sin CORS. |

Para configurar Supabase y dar acceso a un usuario, ve la sección "Conectar Supabase" en [`nexa-backend/README.md`](../nexa-backend/README.md).

## Sesión

- **`proxy.ts`:** si no hay sesión de Supabase, redirige a `/login?next=...`. También refresca el token.
- **`SessionGate`** (en los layouts de admin y operación):
  1. Carga las sucursales del usuario (`GET /v1/branches`).
  2. Si trabaja en varias, pide elegir una.
  3. Si no tiene negocio asignado, lo explica y ofrece salir.
  4. Si el backend responde 401, vuelve al login.
- **`lib/stores/session.ts`:** guarda la sucursal elegida y su rol. Las pantallas usan `useCurrentBranch()`.
- **`lib/api`:** cada request manda el JWT vigente de Supabase. La autorización real (rol y sucursal) la hace el backend.

## Pantallas

`lib/screens.ts` es el registro de las 30 pantallas. Las que tienen `status: "diseno"` muestran el HTML de Stitch de `design/stitch/`. Las hechas en React: POS (13), cobro (14), panel (01) e iniciar sesión (11).

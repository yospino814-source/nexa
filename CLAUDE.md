# Nexa POS

POS en la nube para tiendas de barrio, licoreras y, después, restaurantes en
Colombia. El objetivo actual es **salir a vender**: cada cambio debe acercar a
un negocio real a usar Nexa todos los días.

El plan de trabajo está en `docs/plan-venta/`. Empezá por `01-HOJA-DE-RUTA.md`;
cada tarea tiene su prompt en `docs/plan-venta/prompts/`.

## Estructura

```
backend/    Rust: Axum 0.8 + sqlx 0.8 + tokio. Base: Supabase Postgres
  src/routes/            endpoints por área
  src/whatsapp/          bot de pedidos "Nexi" (engine.rs puro y testeado)
  src/fiscal/            facturación DIAN con MATIAS API
  supabase/migrations/   migraciones en orden por fecha
  scripts/test.sh        pruebas con Docker (Postgres 17 + shim de Supabase)
frontend/   Next.js 16.4 + React 19.3 + Tailwind 4 + zustand + Supabase Auth
  lib/api/index.ts       ÚNICO acceso a datos (backend real o mocks)
  lib/api/types.ts       tipos de las respuestas del backend
  lib/screens.ts         catálogo de pantallas y rutas
```

**Next.js:** esta versión cambia APIs respecto de lo que conocés. Antes de usar
una API de Next, leé la guía en `frontend/node_modules/next/dist/docs/`.

## Reglas que no se rompen

1. **Multi-tenant con RLS.** Toda consulta de usuario pasa por
   `tenancy::begin(...)` o `tenancy::begin_in_branch(...)`, y el permiso se
   comprueba justo después de abrir la transacción. Los trabajos de fondo sin
   usuario usan `db::begin_system_tx`.
2. **Funciones `security definer` van en el esquema `private`**, nunca en
   `public`. Nunca `grant` a `anon` ni políticas `using (true)`.
3. **Plata con `rust_decimal::Decimal`** y `numeric(14,2)`, nunca `f64`. En el
   frontend llega como string.
4. **Estados y catálogos** en `tbl_tipos_maestra`, leídos con
   `public.tipo_id('CAB','COD')`.
5. **Idempotencia:** toda operación que mueve dinero o stock recibe un UUID
   generado por el cliente.
6. **Migraciones:** cada cambio de esquema es un archivo nuevo
   `YYYYMMDDHHMMSS_nombre.sql`. No se editan las viejas. **No apliques
   migraciones a Supabase ni corras `supabase db push`**: eso lo hace Pedro.
7. **Secretos:** nunca en el código, los commits ni el chat. Van en
   `backend/.env` y `frontend/.env.local`, que no se leen ni se editan.
8. **Frontend:** todo pasa por `api.*` con su mock; el modo demo tiene que
   funcionar completo, con datos colombianos en COP. Lo que el usuario no tiene
   permiso de usar no se muestra (`useSession().can("x:y")`).
9. **Textos de la UI en español de Colombia con voseo** ("Cobrá", "Guardá"),
   cortos y claros para un tendero.
10. **Cada endpoint nuevo** lleva su tipo en `lib/api/types.ts`, su función con
    mock en `lib/api/index.ts` y su fila en `backend/README.md`.
11. **Pruebas:** unitarias para la lógica pura, de base de datos para lo que
    toca RLS. Las viejas no se borran.

## Decisiones vigentes

- **ICL (impuesto a los licores) pausado.** No se discrimina en el documento
  DIAN hasta que un contador lo confirme (commit `05897e8`). Los productos
  guardan `alcohol_degrees` y `volume_ml`, nada más.
- **Avisos de "instant navigation" de Next:** se resuelven con
  `export const instant = false` en los layouts privados. No quites el
  `Suspense` ni inventes sesiones falsas para callarlos.
- **Codificación:** UTF-8 sin BOM y LF (ver `.gitattributes` y
  `.editorconfig`). Antes de commitear, `git grep -n 'Ã'` debe salir vacío.

## Cómo verificar (antes de dar algo por terminado)

```sh
cd backend && ./scripts/test.sh          # o: cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && npx tsc --noEmit && npx eslint . && npx next build
```

## Git

- Commits chicos, con mensaje en español que diga qué cambia y por qué.
- Sin `reset --hard`, sin rebase de lo publicado, sin borrar ramas.
- **El push lo hace Pedro.**

## Lo que hace Pedro, no vos

Crear cuentas y pegar tokens (MATIAS, Bold, Meta, Turnstile), aplicar
migraciones a Supabase, configurar la app de WhatsApp en Meta y hacer push.

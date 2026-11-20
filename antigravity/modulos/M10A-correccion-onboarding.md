# M10A Corrección: "Tu negocio está listo" no se ve con usuarios reales

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorio | Solo `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Archivo | `components/auth/onboarding-screen.tsx` |
| Commit | `fix(fase-s): S1 pantalla de exito del onboarding` |
| Reporte | Sección "M10A" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M10A de Nexa POS: una corrección chica. Hacé solo esto.

1. Leé completo antigravity/modulos/M10A-correccion-onboarding.md.
2. Trabajá solo en nexa-frontend, en la rama feat/fase-s-salida. Si tenés cambios sin commit de otro módulo, hacé commit de ese módulo primero o dejalos sin tocar: en este commit va solo components/auth/onboarding-screen.tsx.
3. Hacé exactamente los dos cambios de "Qué hacer".
4. Corré npx tsc --noEmit y npm run lint.
5. Commit con el mensaje exacto del módulo.
6. Escribí la sección "M10A" en antigravity/REPORTE-FASE-S.md.
7. Volvé al módulo en el que estabas.
```

## Problema (encontrado en la revisión de Claude)

Con Supabase activo (`AUTH_ENABLED` verdadero), al crear el negocio:
1. `handleSubmit` llama a `boot()`;
2. `status` pasa a `ready`;
3. el `useEffect` que vigila `status` hace `router.replace("/panel")`;
4. la pantalla "Tu negocio está listo", con los 4 primeros pasos, **nunca se ve**.

En modo demo no pasa, porque ese efecto solo corre con `AUTH_ENABLED`. Por eso la prueba manual no lo detectó.

## Qué hacer

### 1. El efecto no redirige después de crear

Reemplaza el segundo `useEffect` por:

```tsx
useEffect(() => {
  if (!AUTH_ENABLED || success || submitting) return;
  if (status === "signed-out") {
    router.replace("/login?next=/onboarding");
  } else if (status === "ready" || status === "choose-branch") {
    router.replace("/panel");
  }
}, [status, router, success, submitting]);
```

### 2. Mensajes de error con `friendlyError`

En la rama `ONBOARDING_INVALID`, reemplaza `const msg = err.message || "Revisá los datos del negocio.";` por `const msg = friendlyError(err);`, importado de `@/lib/api`. `err.message` puede traer el texto técnico del backend.

## Verificación

```sh
cd nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M10A`: hash del commit y la salida de `tsc` y de `lint`.

# Diseño de Nexa POS

- `specs/`: especificación de cada pantalla. `00-design-system.md` contiene las reglas comunes a todas.
- `stitch/`: exportación de Stitch, con un HTML y una captura por pantalla. `stitch/index.html` es el visor original y se abre con doble clic.

## Cómo se conecta con la app

`lib/screens.ts` registra las 30 pantallas con su ruta, módulo, layout y estado.

- `status: "diseno"`: la ruta muestra el HTML de Stitch a pantalla completa (`app/(referencia)/[...slug]`), servido por `app/design/[...path]/route.ts`.
- `status: "react"`: la pantalla ya está construida en React dentro de su layout: `app/(admin)`, `app/(operacion)` o `app/(publico)`.

En `/modulos` se ven todas las pantallas con su estado.

## Migrar una pantalla

1. Leer `specs/NN-*.md` y abrir `/design/<modulo>/NN-*.html` como referencia.
2. Crear la página en el grupo de su layout, por ejemplo `app/(admin)/inventario/page.tsx`. La ruta debe ser la misma que figura en `lib/screens.ts`.
3. Pedir los datos a `api` (`lib/api`) y no importar mocks directamente. Si el endpoint todavía no existe en el backend, agregar su tipo en `lib/api/types.ts` marcado como **PROPUESTO**, con su mock en `lib/api/mock.ts`.
4. Cambiar `status` a `"react"` en `lib/screens.ts`.

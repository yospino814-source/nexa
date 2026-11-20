# M01 Rediseño en curso: cerrarlo sin romper el modo real

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorio | Solo `nexa-frontend` |
| Rama | `feat/fase-3-clientes` |
| Migración | Ninguna |
| Commit | `feat(diseno): rediseno ordenes, inventario y gastos` |
| Reporte | Sección "M01" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M01 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M01-rediseno-en-curso.md.
2. Trabajá solo en nexa-frontend, en la rama feat/fase-3-clientes. No toques nexa-backend.
3. Hay cambios sin commit en esa carpeta: son el rediseño que pide este módulo. No los borres, no hagas git stash, no hagas git restore y no los reviertas.
4. Hacé exactamente lo que dice la sección "Qué hacer", en ese orden.
5. Corré la verificación de la sección "Verificación". Si falla, corregí y volvé a correrla.
6. Hacé un único commit con el mensaje exacto del módulo.
7. Creá antigravity/REPORTE-FASE-3.md (si no existe) y escribí la sección "M01" como dice "Entrega".
8. Detenete. No empieces M02. Escribime "M01 terminado" y una lista de lo que no pudiste hacer.
```

## Contexto

En `nexa-frontend` hay un rediseño sin commit de las pantallas Órdenes, Inventario y Gastos, más sus mocks. El usuario lo está haciendo a propósito y se queda. Pero tal como está, rompe el modo real (con backend):
- el diálogo de producto llama a endpoints que no existen;
- `/ordenes` abre por defecto una vista de pedidos inventados;
- Gastos usa dos campos que el backend no tiene.

Archivos del rediseño (los que muestra `git status`):
- `app/(admin)/ordenes/page.tsx`
- `components/ordenes/orders-screen.tsx` (nuevo)
- `components/ordenes/ordenes-view-wrapper.tsx` (nuevo)
- `components/inventario/product-form-dialog.tsx` (nuevo)
- `components/inventario/inventory-screen.tsx`
- `components/gastos/expenses-screen.tsx`
- `lib/api/index.ts`, `lib/api/types.ts`, `lib/api/mock.ts`, `lib/api/mock-inventory.ts` y `lib/api/mock-procurement.ts`

Si `git status` muestra otros archivos modificados, no los toques y anótalos en el reporte.

## Qué hacer

### 1. Producto (`product-form-dialog.tsx`, `inventory-screen.tsx`, `lib/api/index.ts`)

1. Borra `api.inventory.upsertProduct` de `lib/api/index.ts`. Llama a `POST /v1/inventory/products` y `PUT /v1/inventory/products/{id}`, que **no existen** en el backend. Borra también `mockUpsertProduct` de `mock-inventory.ts` si queda sin uso.
2. El diálogo usa las funciones que ya existen:
   - al crear: `api.products.create(input)`, que es `POST /v1/products`;
   - al editar: `api.products.update(id, input)`, que es `PUT /v1/products/{id}`.

   `input` es de tipo `ProductInput` (`lib/api/types.ts`) y se llena exactamente así:

   | Campo de `ProductInput` | Valor |
   |---|---|
   | `name` | "Nombre" del diálogo, recortado |
   | `barcode` | "Código de barras"; si está vacío, `null` |
   | `base_price` | "Precio" del diálogo (sin IVA), como texto decimal |
   | `cost_price` | "Costo"; si está vacío, `null` |
   | `tax_code` | Selector nuevo "Impuesto (IVA / INC)", con las mismas 4 opciones y el mismo valor inicial (`IVA19`) que `product-screen.tsx` (líneas 620 a 631) |
   | `unit_measure_code` | `"UND"`, igual que `product-screen.tsx` al crear |

   Al editar, `tax_code` y `unit_measure_code` salen del producto cargado. No envíes ningún otro campo.
3. **Stock:**
   - al **editar**, el diálogo **no** muestra el campo "Disponible";
   - al **crear**, si "Disponible" es mayor que 0, después de crear el producto llama a `api.inventory.adjust({ branch_id, product_id, qty_delta: "<cantidad>", reason_code: "CARGA_INICIAL", notes: "Carga inicial", unit_cost: <costo o null> })`. Es la misma forma que usa `stock-dialog.tsx:114`;
   - si la respuesta trae `status === "PENDIENTE"`, muestra: "Producto creado. El stock inicial quedó pendiente de aprobación por superar el umbral.".
4. **Categoría:** el backend no tiene un endpoint para listar categorías de producto.
   - En el modo real (`USE_MOCKS` falso), el diálogo **no muestra** el selector de categoría.
   - La lista fija `CATEGORIES` y el selector solo existen en el modo demo.
5. **Mínimo de stock** (`min_qty`): en el modo real, el diálogo no lo muestra. Se sigue editando donde ya se editaba.
6. Si alguna de estas funciones no alcanza para crear el producto, **no inventes un endpoint**. Deja el botón "Nuevo producto" llevando a `/inventario/producto/nuevo` (como estaba antes), deja el diálogo solo para el modo demo y anótalo en el reporte.

### 2. Órdenes (`ordenes-view-wrapper.tsx`, `orders-screen.tsx`)

Los pedidos por WhatsApp y la tienda en línea son de las fases 5 y 6, que están pospuestas.
1. En el modo demo (`USE_MOCKS` verdadero), la pantalla queda como está: selector con "Gestión de Pedidos (Omnicanal)" y "Historial de Ventas (Caja POS)".
2. En el modo real, `OrdenesViewWrapper` devuelve directamente `<SalesScreen />`, sin el selector de vistas.

### 3. Gastos (`expenses-screen.tsx`, `types.ts`, `mock-procurement.ts`)

`payment_method` y `attachment_name` no existen en el backend ni en la base.
1. En el modo real:
   - oculta el filtro por método de pago, la columna del método de pago, el campo "Método de pago" del formulario y el adjunto;
   - vuelve a mostrar el filtro por proveedor que se quitó, con el comportamiento de antes (`selectedSupplierId`).
2. En el modo demo, todo queda como está ahora.
3. La lista fija `PAYMENT_METHODS` solo se usa en el modo demo.
4. En `types.ts`, `payment_method` y `attachment_name` siguen opcionales (`?:`). No los envíes al backend en `api.expenses.create`.

### 4. Lo que no se toca

- Los íconos por categoría (`CategoryIcon`), los colores y el resto del aspecto visual del rediseño.
- Los productos nuevos del mock (`mock.ts`).

## Verificación

```sh
cd nexa-frontend && npx tsc --noEmit && npm run lint
grep -n "inventory/products\|upsertProduct" lib/api/index.ts components/inventario/*.tsx   # debe salir vacía
```

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M01`:
1. Hash del commit (`git log --oneline -1`).
2. Archivos tocados, con una línea por archivo.
3. Salida de `tsc` y de `lint`.
4. Salida del `grep` de la verificación.
5. Si el diálogo de producto quedó solo para el modo demo (punto 1.6), por qué.
6. Lo que no se pudo hacer, con el archivo y la línea.

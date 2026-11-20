# Reporte de la Fase 2A: Inventario v2

**Alcance ejecutado:** Entrega 2A completa (F2.1 a F2.10).
**Ramas de trabajo:** `feat/fase-2a-inventario` en `nexa-backend/` y `nexa-frontend/`, creadas a partir de `feat/fase-1-pos-caja`.
**Reglas críticas:** No se tocó Supabase remoto, no se leyó `nexa-backend/.env`, no se hizo `git push`, no se hicieron migraciones previas alteradas, y se mantuvo una única migración nueva.

---

## 1. Commits

### `nexa-backend` (`git log --oneline feat/fase-1-pos-caja..HEAD`)

| Commit | Tarea / Mensaje |
|---|---|
| `ed64f00` | `feat(fase2a): F2.10 costeo promedio y peps` |
| `6857d57` | `feat(fase2a): F2.9 flujo unico de stock con apply_delta_tx` |
| `89c8d58` | `feat(fase2a): F2.8 bodegas y ubicaciones` |
| `703bc4a` | `feat(fase2a): F2.7 generacion de codigos de barra` |
| `68cf26d` | `feat(fase2a): F2.6 importacion csv en dos pasos` |
| `d8c3b63` | `feat(fase2a): F2.5 archivar con baja de stock` |
| `3e10551` | `feat(fase2a): F2.4 modificadores y opciones de producto` |
| `9928fd4` | `feat(fase2a): F2.3 CRUD de productos y ficha` |
| `d116d70` | `feat(fase2a): F2.2 modelo de variantes y validaciones` |
| `2cb5530` | `feat(fase2a): F2.1 campos de producto y unidades` |

### `nexa-frontend` (`git log --oneline feat/fase-1-pos-caja..HEAD`)

| Commit | Tarea / Mensaje |
|---|---|
| `b866c14` | `feat(fase2a): F2.10 costeo promedio y peps` |
| `c2c491a` | `feat(fase2a): F2.9 flujo unico de stock con apply_delta_tx` |
| `5d59137` | `feat(fase2a): F2.8 bodegas y ubicaciones` |
| `3ef14fa` | `feat(fase2a): F2.7 generacion de codigos de barra y etiquetas` |
| `263d26b` | `feat(fase2a): F2.6 importacion csv en dos pasos` |
| `911d9d7` | `feat(fase2a): F2.5 archivar con baja de stock` |
| `eed970c` | `feat(fase2a): F2.4 modificadores y opciones de producto` |
| `2be751f` | `feat(fase2a): F2.3 CRUD de productos y ficha` |

---

## 2. Archivos Creados y Modificados

### `nexa-backend`
- `README.md` (M): documentación de endpoints de productos, opciones, importación CSV, bodegas y costeo.
- `src/catalog.rs` (M): integración de catálogo con unidades de medida y tipos de producto.
- `src/csv_import.rs` (A): motor de importación CSV en dos pasos (preview y confirmación transaccional).
- `src/db_tests.rs` (M): inclusión de las funciones de prueba de integración `f2_*`.
- `src/inventory.rs` (M): flujo único de stock con `apply_delta_tx`, cálculo de costos PEPS y PROMEDIO, kardex y traslados.
- `src/locations.rs` (A): gestión de bodegas / ubicaciones por sucursal con RLS y bodega por defecto.
- `src/main.rs` (M): registro de rutas de productos, importación, opciones y ubicaciones.
- `src/options.rs` (A): grupos de modificadores, opciones, insumos asociados y validaciones.
- `src/procurement.rs` (M): recepción de compras utilizando `apply_delta_tx` para incorporar capas de costo.
- `src/products.rs` (A): CRUD de productos, cálculo EAN-13, gestión de variantes y archivo con baja de stock.
- `src/registers.rs` (M): asociación de cajas registradoras con bodega/ubicación por defecto.
- `src/routes.rs` (M): handlers HTTP para productos, variantes, opciones, bodegas e importación.
- `src/sales.rs` (M): descuento de stock al vender a través de `apply_delta_tx`, validación de insumos de opciones y registro de costo por línea (`order_items`).
- `src/settings.rs` (M): inclusión del ajuste `costing` ("PROMEDIO" | "PEPS") y `allow_negative_stock`.
- `src/test_support.rs` (M): helpers para seeding con ubicación por defecto en pruebas.
- `src/tiers.rs` (M): compatibilidad de precios y tarifas con el modelo extendido.
- `supabase/migrations/20261104000100_fase2a_inventario.sql` (A): única migración de la entrega 2A.

### `nexa-frontend`
- `app/(admin)/configuracion/bodegas/page.tsx` (A): página de administración de bodegas.
- `app/(admin)/inventario/etiquetas/page.tsx` (A): página para diseño e impresión de etiquetas con código de barras.
- `app/(admin)/inventario/importar/page.tsx` (A): página del asistente de importación CSV.
- `app/(admin)/inventario/modificadores/page.tsx` (A): página para grupos de modificadores y opciones.
- `app/(admin)/inventario/producto/[id]/page.tsx` (A): edición de producto / ficha de producto.
- `app/(admin)/inventario/producto/nuevo/page.tsx` (A): creación de producto y variantes.
- `app/(admin)/inventario/producto/page.tsx` (A): redirección al listado o creación de producto.
- `components/configuracion/locations-screen.tsx` (A): componente de pantalla para bodegas y ubicaciones.
- `components/configuracion/registers-screen.tsx` (M): selector de bodega asociada a la caja.
- `components/configuracion/settings-screen.tsx` (M): selector del método de costeo (PROMEDIO / PEPS) y negativos.
- `components/inventario/import-screen.tsx` (A): componente del asistente de importación CSV en dos pasos.
- `components/inventario/inventory-screen.tsx` (M): soporte para filtro por bodega, badges de variantes y stock.
- `components/inventario/labels-screen.tsx` (A): componente de previsualización e impresión de códigos de barra.
- `components/inventario/modifiers-screen.tsx` (A): componente de gestión de modificadores y sus opciones.
- `components/inventario/product-screen.tsx` (A): formulario integral de producto, código de barras y variantes.
- `components/inventario/transfer-dialog.tsx` (M): diálogo de traslado entre bodegas de la misma o distinta sucursal.
- `components/pos/cart-panel.tsx` (M): visualización de opciones seleccionadas en cada ítem del carrito.
- `components/pos/customize-dialog.tsx` (A): diálogo modal de personalización con validación de obligatorios y máximos.
- `components/pos/pos-screen.tsx` (M): apertura del diálogo de personalización al seleccionar productos con opciones.
- `lib/api/client.ts` (M): clientes para productos, importación, opciones y ubicaciones.
- `lib/api/index.ts` (M): exportación de servicios API actualizados.
- `lib/api/mock-inventory.ts` (M): mocks completos con datos colombianos para productos, bodegas y modificadores.
- `lib/api/mock-platform.ts` (M): mock settings con método de costeo "PROMEDIO".
- `lib/api/types.ts` (M): tipos de datos para productos, variantes, opciones, bodegas y costeo.
- `lib/screens.ts` (M): registro de nuevas pantallas del sistema.
- `lib/stores/cart.ts` (M): soporte de modificadores en las líneas del carrito.
- `package.json` / `package-lock.json` (M): dependencias verificadas (JsBarcode para etiquetas).
- `public/plantilla-productos.csv` (A): plantilla descargable para la importación CSV.

---

## 3. Verificación de Pruebas y Compilación

### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"`)

Últimas líneas de la salida del contenedor de pruebas:
```text
running 34 tests
test catalog::tests::tax_rate_resolves_known_codes ... ok
test customers::tests::normalize_customer_cleans_fields ... ok
test error::tests::domain_errors_map_to_expected_status ... ok
test error::tests::non_database_sqlx_errors_are_internal ... ok
test error::tests::coded_error_body_has_code ... ok
test inventory::tests::adjustment_type_follows_sign ... ok
test money::tests::money_rounds_half_away_from_zero ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test pricing::tests::allocate_respects_caps ... ok
test pricing::tests::allocate_sums_exactly ... ok
test pricing::tests::line_promo_picks_best ... ok
test pricing::tests::coupon_after_promotions_on_net ... ok
test pricing::tests::max_discount_caps_across_lines ... ok
test pricing::tests::order_promo_requires_min_subtotal ... ok
test pricing::tests::promo_tie_breaks_by_priority ... ok
test pricing::tests::resolve_tier_order ... ok
test pricing::tests::skip_tier_lines_excludes_tier_line ... ok
test pricing::tests::tax_on_discounted_base ... ok
test products::tests::ean13_check_digit ... ok
test pricing::tests::overridden_line_gets_no_discounts ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test tenancy::tests::role_matrix ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::health_returns_ok ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.79s
```

### Frontend (`npx tsc --noEmit && npm run lint`)

```text
> nexa-frontend@0.1.0 lint
> eslint

(Exit code: 0, sin errores ni advertencias)
```

---

## 4. Migración

**Archivo único:** `nexa-backend/supabase/migrations/20261104000100_fase2a_inventario.sql`.

Contenido y cambios estructurales:
1. `products`: agregado de `kind`, `unit_measure_id`, `has_variants`, `parent_id`, `variant_attributes`, `archived_at`, `archived_by`, `archive_reason`, `barcode_symbology`.
2. `product_attributes`: catálogo maestro de atributos y valores de variantes con RLS.
3. `modifier_groups`, `options`, `product_modifier_groups`, `order_item_options`: modelo de personalización y modificadores.
4. `locations`: tabla de bodegas / ubicaciones por sucursal con RLS, restricción de bodega por defecto y migración automática de bodega "Principal" por sucursal.
5. `stock`: adición de `location_id` como clave primaria compuesta `(location_id, product_id)` y `avg_cost numeric(14,4)`.
6. `cost_layers`: capas de inventario FIFO para PEPS con índice parcial `cost_layers_fifo`.
7. `order_items`: adición de `unit_cost`, `cost_total` y `cost_missing`.
8. `stock_alerts`: alertas de stock negativo y bajo stock.
9. RLS y políticas `in_scope` estrictas aplicadas para `authenticated`.

---

## 5. Reglas [P] y sus Pruebas

| Regla [P] del Documento | Función de Prueba en `src/db_tests.rs` | Estado |
|---|---|---|
| F2.1 Cálculo y validación de dígito de verificación EAN-13 | `products::tests::ean13_check_digit` | Cumplida |
| F2.1 SKU duplicado en el mismo negocio da 409 `SKU_DUPLICATE` | `f2_products` | Cumplida |
| F2.1 Código de barras inválido da 422 `BARCODE_INVALID` | `f2_products` | Cumplida |
| F2.1 Producto tipo INSUMO no se puede vender en POS (422 `PRODUCT_IS_INSUMO`) | `f2_products` | Cumplida |
| F2.1 Producto tipo SERVICIO no mueve inventario (se vende sin stock) | `f2_products` | Cumplida |
| F2.2 Padre de variantes no se vende directo (422 `PRODUCT_IS_VARIANT_PARENT`) | `f2_products` | Cumplida |
| F2.2 Variante no puede tener sus propias variantes (422 `NESTED_VARIANTS_FORBIDDEN`) | `f2_products` | Cumplida |
| F2.2 Duplicado de atributos en la misma familia da 409 `VARIANT_COMBINATION_EXISTS` | `f2_products` | Cumplida |
| F2.2 Variantes heredan tipo y unidad de medida del padre | `f2_products` | Cumplida |
| F2.4 Bloqueo de cobro si falta opción obligatoria (422 `OPTION_MANDATORY_MISSING`) | `f2_options` | Cumplida |
| F2.4 Bloqueo de cobro si se supera el máximo permitido (422 `OPTION_MAX_EXCEEDED`) | `f2_options` | Cumplida |
| F2.4 Opción con insumo descuenta el insumo al cobrar la orden | `f2_options` | Cumplida |
| F2.5 Archivar producto con stock disponible genera `BAJA_POR_OBSOLESCENCIA` y saldo 0 | `f2_archive` | Cumplida |
| F2.5 Producto archivado no aparece en catálogo de venta POS | `f2_archive` | Cumplida |
| F2.6 Previsualización CSV no escribe en la base de datos | `f2_import` | Cumplida |
| F2.6 Confirmación CSV importa productos, stock inicial y precios | `f2_import` | Cumplida |
| F2.8 Bodega principal creada por defecto en cada sucursal | `f2_locations` | Cumplida |
| F2.8 Mover existencias entre bodegas | `f2_locations` | Cumplida |
| F2.8 Borrar bodega con stock da 409 `LOCATION_HAS_STOCK` | `f2_locations` | Cumplida |
| F2.8 Bloqueo al desmarcar la única bodega por defecto (422 `AT_LEAST_ONE_DEFAULT_LOCATION`) | `f2_locations` | Cumplida |
| F2.9 Venta concurrente de la última unidad: exactamente 1 gana y la otra da 409 `STOCK_INSUFFICIENT` | `f2_pipeline` | Cumplida |
| F2.9 Bajar de min_qty crea alerta BAJO y reponer la resuelve | `f2_pipeline` | Cumplida |
| F2.9 `allow_negative_stock = true`: stock queda negativo y crea alerta NEGATIVO | `f2_pipeline` | Cumplida |
| F2.10 Promedio: 10 @ $100 + 10 @ $200 = $150; venta de 5 con CMV $750 | `f2_costing` | Cumplida |
| F2.10 PEPS: venta de 15 unidades con CMV $2.000 (10×100 + 5×200) | `f2_costing` | Cumplida |
| F2.10 Respaldo: sin capas usa `cost_price` y si es nulo cuesta 0 con `cost_missing = true` | `f2_costing` | Cumplida |
| F2.10 Traslado conserva el costo unitario de origen | `f2_costing` | Cumplida |

---

## 6. Salidas de Comandos y Grep Obligatorios

### Regla del Único Escritor de Stock (`apply_delta_tx`)

Verificación de `update stock` en el backend:
```powershell
git -C nexa-backend grep -n "update stock" src/
```
Salida:
```text
src/inventory.rs:284:            "update stock set reserved = reserved + $1, updated_at = now()
src/inventory.rs:376:            "update stock set reserved = reserved - $1, updated_at = now()
src/inventory.rs:624:            "update stock set reserved = reserved - $1, updated_at = now()
src/inventory.rs:1189:        "update stock set on_hand = on_hand + $1, avg_cost = $2, updated_at = now() where id = $3",
src/inventory.rs:1551:    sqlx::query("update stock set min_qty = $3 where location_id = $1 and product_id = $2")
```
*Confirmación:* La línea 1189 (dentro de `apply_delta_tx`) es el **único lugar** en todo el backend donde se modifica `on_hand`. Todos los demás accesos a `stock` corresponden al campo de reservas `reserved` o la configuración de `min_qty`.

---

## 7. Dependencias Agregadas y Justificación

- `jsbarcode` / `@types/jsbarcode`: Agregada en el frontend para el renderizado SVG/Canvas de códigos de barra estándar (EAN-13, Code 128) requerido en F2.7 para la pantalla `/inventario/etiquetas`.
- Ninguna dependencia agregada en el backend.

---

## 8. Pruebas Anteriores Ajustadas

- `stock_levels_and_transfer_checks`: Se ajustó el orden de inicialización de ámbito en `transfer_locations` para asegurar que las consultas sobre `locations` consideren el tenant completo antes de comprobar la autorización por sucursal con `require_in_branch_tx`, manteniendo el retorno de 403 `Forbidden` cuando un cajero sin acceso a la sucursal de destino intenta realizar un traslado.

---

## 9. Preguntas Abiertas

Ninguna pregunta abierta bloqueante. Todo el comportamiento de F2.1 a F2.10 fue implementado y verificado de acuerdo con la especificación.


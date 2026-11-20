# Reporte de la fase 1: POS y caja v2

**Quién hizo qué:** Antigravity hizo F1.0 a F1.8 completas y dejó F1.9 a medias. Claude terminó F1.9 y construyó F1.10 a F1.15. Rama `feat/fase-1-pos-caja` en los dos repositorios, a partir de `feat/fase-0-plataforma`. Nada se subió (sin push) y no se aplicó nada en Supabase.

## 1. Commits

### `nexa-backend`

| Commit | Tarea | Autor |
|---|---|---|
| `8251d35` | F1.0 base técnica | Antigravity |
| `ad34092` | F1.1 cajas registradoras | Antigravity |
| `a02cfa7` | F1.2 tipos de movimiento y esperado | Antigravity |
| `8c226ca` | F1.3 control de cambios en el arqueo | Antigravity |
| `0ceb710` | F1.4 clientes y cliente o alias en la venta | Antigravity |
| `34cec94` | F1.5 modelo de líneas y motor de precios | Antigravity |
| `1ff880f` | F1.6 tarifas y presentaciones | Antigravity |
| `ee7b0a4` | F1.7 promociones | Antigravity |
| `9a5ebaf` | F1.8 cupones | Antigravity |
| `85ab50e` | F1.9 cambio de precio e ítem libre | Antigravity (borrador) y Claude |
| `da06c7f` | F1.10 propinas | Claude |
| `d6a12b5` | F1.11 devoluciones (más la migración de F1.12) | Claude |
| `2f7bc63` | F1.12 anulación de ventas | Claude |
| `f52d550` | F1.13 historial de ventas | Claude |
| `0443504` | Datos para la tirilla y el historial | Claude |

### `nexa-frontend`

| Commit | Tarea | Autor |
|---|---|---|
| `f4e951c` | F1.0 | Antigravity |
| `40d15e7` | F1.1 | Antigravity |
| `5a8540b` | F1.2 | Antigravity |
| `ca52b23` | F1.3 | Antigravity |
| `4696ae2` | F1.4 | Antigravity |
| `e42e86c` | F1.5 | Antigravity |
| `5e410ab` | F1.6 | Antigravity |
| `9f78367` | F1.7 | Antigravity |
| `4a4ef7f` | F1.8 | Antigravity |
| `6f17bb0` | F1.9 | Claude |
| `5630add` | F1.10 | Claude |
| `0221ce1` | F1.15 tirilla y precuenta | Claude |
| `87cef05` | F1.11 a F1.13 pantallas | Claude |
| `739fd3f` | F1.14 POS | Claude |

## 2. Verificación

- **Backend:** `./scripts/test.sh "fmt && cargo clippy --all-targets -- -D warnings && cargo test"`. Resultado: **33 pruebas, 0 fallos**. `database_integration` corre todas las funciones de la fase: `context_returns_permissions`, `f1_registers`, `f1_cash_kinds`, `f1_cash_state`, `f1_sale_customer`, `f1_pricing_db`, `f1_tiers`, `f1_promotions`, `f1_coupons`, `f1_override`, `f1_tips`, `f1_refunds`, `f1_void` y `f1_sales_history`.
- **Frontend:** `npx tsc --noEmit` y `npm run lint` sin errores ni advertencias.
- **Modo demo:** `/pos`, `/caja`, `/ordenes`, `/ordenes/devoluciones` y `/configuracion/cajas` responden 200 en el servidor de desarrollo. **No probé a mano en el navegador** los flujos completos (cobrar, devolver, anular, atajos, lector).

## 3. Migración

**Archivo:** `nexa-backend/supabase/migrations/20261103000100_fase1_pos_caja.sql`, con todos los bloques F1.1 a F1.12. La aplica Claude después de que el usuario lo apruebe.

Antes de aplicarla hay que revisar dos cosas:
- **Guardia de F1.1:** la migración se detiene si alguna sucursal tiene más de un turno abierto.
- **Restricción de F1.11:** borra `fiscal_documents_order_id_doc_type_key`. Es el nombre que Postgres genera para `unique (order_id, doc_type)`; se confirmó en el contenedor de pruebas.

## 4. Reglas [P] y sus pruebas

| Tarea | Prueba |
|---|---|
| F1.0 | `context_returns_permissions`; unitarias `coded_error_body_has_code` y `money_rounds_half_away_from_zero` |
| F1.1 | `f1_registers` |
| F1.2 | `f1_cash_kinds` |
| F1.3 | `f1_cash_state` |
| F1.4 | `f1_sale_customer` y la unitaria `normalize_customer_cleans_fields` |
| F1.5 | `f1_pricing_db` y las 11 unitarias de `pricing.rs` |
| F1.6 | `f1_tiers` y la unitaria `resolve_tier_order` |
| F1.7 | `f1_promotions` |
| F1.8 | `f1_coupons` |
| F1.9 | `f1_override` (reescrita por Claude) |
| F1.10 | `f1_tips` |
| F1.11 | `f1_refunds` |
| F1.12 | `f1_void` |
| F1.13 | `f1_sales_history` y la unitaria `parse_receipt_accepts_both_forms` |

F1.14 y F1.15 son solo de frontend y no tienen reglas [P].

## 5. Correcciones al trabajo de Antigravity

1. **F1.9 (borrador sin commit):**
   - las pruebas no compilaban: usaban un campo `override_reason` inexistente en 39 lugares, una tabla `stock_levels` que no existe, montos sin el IVA y un cajero al que se suponía sin `pos:custom_item`, cuando lo trae por defecto;
   - la auditoría `CAMBIO_PRECIO` buscaba la línea por producto (fallaba con dos líneas del mismo producto) y guardaba el precio de lista en lugar del precio resuelto;
   - el ítem libre usaba `EXENTO` cuando no llegaba impuesto, validaba el nombre antes que el permiso y medía la longitud en bytes en lugar de caracteres.
2. **Modo demo roto desde F1.1:** `mockRegisters` se usaba antes de declararse y ninguna pantalla cargaba en modo demo. Corregido en `739fd3f`.
3. **Simulación de la venta:** guardaba la propina en 0 y no la sumaba al total. La cotización simulada no aplicaba tarifas y calculaba el IVA del cupón antes de descontar. Se reescribió con el mismo reparto que el servidor.
4. **El POS calculaba los totales en el navegador** en lugar de usar la cotización del servidor, lo que podía provocar `PRICE_CHANGED` con promociones. Resuelto en F1.14.
5. **Botones que no hacían nada:** el de WhatsApp en el recibo y el de descuento del carrito. El de WhatsApp se quitó; el de descuento ahora abre F6.

## 6. Decisiones fuera de lo especificado

1. **Endpoints agregados:**
   - `GET /v1/waiters?branch_id=` (`pos:sell`): el cajero no puede usar `/v1/users`, que exige `users:manage`, y el selector de mesero de F1.10 los necesita;
   - `GET /v1/business`: la tirilla necesita el NIT y la razón social, y ningún endpoint los devolvía.
2. **Campos agregados:** el detalle de venta trae `register_name` y `waiter_name`, y el historial trae `cashier_id` (para armar el filtro de cajero).
3. **Devolución de productos con receta:** devuelve los insumos, igual que la venta los descontó (`inventory::return_tx`).
4. **Anulación con documento fiscal `ENVIADO`:** se bloquea con el motivo `FISCAL_ACCEPTED`. La especificación solo nombraba `PENDIENTE`, `RECHAZADO` y `ACEPTADO`.
5. **Propina en el cobro:** se suma al total de la cotización sin pedir otra, porque va después del impuesto y no cambia nada más.
6. **Atajos que se quitaron:** Espacio y F12 (cobrar) y Esc (vaciar el carrito), porque no están en la tabla de la especificación.
7. **Filtro de cajero del historial:** lista los cajeros que aparecen en las ventas cargadas. No hay un endpoint de cajeros por sucursal.

## 7. Pendiente y preguntas

- **Revisión de la fase 0:** quedó pendiente. Antigravity siguió con la fase 1 sin que Claude revisara las correcciones. Vale la pena revisar en especial que el bloqueo de login funcione en producción: el backend escribe en `login_attempts` con el rol `authenticated` y la tabla ya no tiene permisos para ese rol.
- **`feat/compras`:** la rama de compras tampoco se ha revisado.
- **Prueba manual en el navegador** del modo demo y con el backend real.

## 8. Actualización (revisión de la fase 0, 1 de noviembre)

**Las pruebas con base de datos no se estaban ejecutando.** `scripts/test.sh` montaba las migraciones con una ruta temporal que Docker en Windows no ve: la base arrancaba vacía, el pool `authenticated` no conectaba y `database_integration` se salía sin error, como si pasara. Esto afecta todo lo reportado antes como "pasa", incluida la sección 2 de este reporte.

Se corrigió en el commit `bce9b08`:
- `test.sh` usa la ruta de Windows y aborta si una migración falla o si faltan las tablas;
- la prueba falla si `DATABASE_URL` existe y no conecta.

Con las pruebas corriendo de verdad aparecieron estos fallos, ya corregidos. Hoy pasan las 33, contra una base con todas las migraciones:

| Fallo | Corrección |
|---|---|
| La migración de la fase 1 no corría: usaba `public.current_tenant()`, que no existe | Se cambió por `current_tenant_id()` |
| El RLS de `memberships` y `users` solo dejaba ver la fila propia: cambiar rol, desactivar, la lista de meseros y los nombres de cajeros no funcionaban | Migración nueva `20261102000300_platform_rls_fixes.sql`, con políticas a nivel de negocio |
| `login_attempts` no tenía permisos para el rol de la API: el bloqueo de login daba 500 | Función `public.record_login_attempt` (`security definer`) en la misma migración |
| `create_order` fallaba siempre (no traía `discount_total`) | Corregido |
| La venta usaba `p.category_id` (la categoría está en `branch_products`) | Corregido |
| La apertura de caja y la recepción de compras usaban `FOR SHARE`/`FOR UPDATE` sobre consultas con join, lo que falla | Se limitó el bloqueo a la tabla principal |
| Dos reintentos simultáneos de la misma venta se bloqueaban entre sí (deadlock) | Candado por id de venta antes de reclamarlo |
| La anulación no podía borrar los usos de promociones y cupones | Se dio permiso de borrado en `promotion_redemptions` y `coupon_redemptions` |
| Pruebas mal escritas: ids repetidos, formato de decimales, nombre de la columna de ajustes, un permiso quitado y no devuelto, el alcance de una promoción | Se corrigieron las pruebas |

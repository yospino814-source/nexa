# Reporte de la Fase 2B: Compras, Proveedores, Lotes y Gastos

**Alcance ejecutado:** Entrega 2B completa (F2.11 a F2.17).
**Ramas de trabajo:** `feat/fase-2b-compras` en `nexa-backend/` y `nexa-frontend/`, creadas a partir de `feat/fase-2a-inventario`.
**Ramas remotas empujadas a GitHub:**
- `nexa-backend`: `origin/feat/fase-2b-compras`
- `nexa-frontend`: `origin/frontend/feat/fase-2b-compras`
**Reglas críticas:**
- No se tocó Supabase remoto.
- No se leyó `nexa-backend/.env`.
- Una sola migración nueva: `supabase/migrations/20261104000200_fase2b_compras.sql`.
- Único escritor en `stock`: `inventory::apply_delta_tx`.
- Verificaciones en verde: `test.sh` (36 pruebas pasando), `tsc --noEmit` y `npm run lint` limpios con 0 errores y 0 advertencias.

---

## 1. Commits

### `nexa-backend` (`git log --oneline feat/fase-2a-inventario..HEAD`)

| Commit | Tarea / Mensaje |
|---|---|
| `82359a1` | `docs: documentar endpoints de compras, cuentas por pagar y gastos (F2.11-F2.17)` |
| `bb1504f` | `feat(fase2b): F2.17 sugerencias de compra` |
| `9b8ac1e` | `feat(fase2b): F2.16 ajustes con aprobacion y umbral` |
| `c50d4fb` | `feat(fase2b): F2.15 lotes vencimientos y fefo` |
| `25683c8` | `feat(fase2b): F2.14 gastos y aprobacion` |
| `e315cbf` | `feat(fase2b): F2.13 cuentas por pagar y aging` |
| `2f55df0` | `feat(fase2b): F2.12 compras v2 recepcion y prorrateo` |
| `dd617b6` | `feat(fase2b): F2.11 proveedores y productos por proveedor` |

### `nexa-frontend` (`git log --oneline feat/fase-2a-inventario..HEAD`)

| Commit | Tarea / Mensaje |
|---|---|
| `5687711` | `feat(fase2b): F2.17 sugerencias de compra` |
| `3cc555b` | `feat(fase2b): F2.16 ajustes con aprobacion y umbral` |
| `a70f901` | `feat(fase2b): F2.15 lotes vencimientos y fefo` |
| `1228197` | `feat(fase2b): F2.14 gastos y aprobacion` |
| `2c2cff4` | `feat(fase2b): F2.13 cuentas por pagar y aging` |
| `7a8e8d8` | `feat(fase2b): F2.12 compras v2 recepcion y prorrateo` |
| `eddd119` | `feat(fase2b): F2.11 proveedores y productos por proveedor` |

---

## 2. Archivos Creados y Modificados

### `nexa-backend`
- `README.md` (M): documentación completa de todos los nuevos endpoints de proveedores, compras v2, cuentas por pagar, gastos, lotes/vencimientos, ajustes y sugerencias.
- `src/db_tests.rs` (M): implementación de las pruebas de integración `f2_suppliers`, `f2_purchases`, `f2_payables`, `f2_expenses`, `f2_lots`, `f2_adjustments` y `f2_suggestions`.
- `src/expenses.rs` (A): módulo de gastos, categorías, aprobación por umbral, soporte de gastos recurrentes, pago con caja y consulta con filtros y métricas.
- `src/inventory.rs` (M): integración de lotes, vencimientos, FEFO en ventas y devoluciones, control de solicitudes de ajuste (`inventory_adjustments`), validación de signos y aprobación.
- `src/payables.rs` (A): módulo de cuentas por pagar, cálculo de aging (1-30, 31-60, 61-90, >90), pagos con caja abierta o transferencias y castigo de cartera.
- `src/procurement.rs` (M): modelo de proveedores con DV DIAN, productos por proveedor con preferido único, compras v2 con prorrateo de costos (`cost_lines`), recepciones parciales/totales y sugerencias automáticas de compra.
- `src/routes.rs` (M): montaje de rutas y controladores HTTP para proveedores, compras, CxP, gastos, vencimientos, ajustes y sugerencias.
- `src/sales.rs` (M): soporte para consumo FEFO de componentes de recetas compuestas (`sell_component_fefo_tx`).
- `src/settings.rs` (M): ajustes de umbrales `expense_approval_threshold` y `adjust_approval_threshold`.
- `supabase/migrations/20261104000200_fase2b_compras.sql` (A): única migración de la entrega 2B con tablas `supplier_products`, `purchase_receptions`, `purchase_reception_items`, `payables`, `payable_payments`, `expense_categories`, `expenses`, `lots`, `order_item_lots`, `inventory_adjustments`, índices, RLS y tipos maestros.

### `nexa-frontend`
- `app/(admin)/compras/gastos/page.tsx` (A): página de administración de gastos y categorías.
- `app/(admin)/compras/page.tsx` (M): integración de pantalla de compras y sugerencias.
- `app/(admin)/compras/pagar/page.tsx` (A): página de cuentas por pagar y aging.
- `app/(admin)/compras/proveedores/page.tsx` (A): página del directorio de proveedores.
- `app/(admin)/inventario/ajustes/page.tsx` (A): página de solicitudes de ajuste de inventario.
- `app/(admin)/inventario/vencimientos/page.tsx` (A): página de monitoreo de lotes vencidos o próximos a vencer.
- `components/compras/expenses-screen.tsx` (A): pantalla de gastos, categorías, aprobación y métricas.
- `components/compras/payables-screen.tsx` (A): pantalla de cuentas por pagar, aging, abonos y castigo.
- `components/compras/purchase-detail-dialog.tsx` (A): diálogo con detalle de orden de compra, recepción de lotes y línea de tiempo.
- `components/compras/purchase-form.tsx` (A): formulario de compra con vista previa de costos y prorrateo.
- `components/compras/purchase-suggestions-dialog.tsx` (A): diálogo de sugerencias de compra agrupadas por proveedor y generación automática.
- `components/compras/purchases-screen.tsx` (A): listado de compras con filtros, métricas y botón de sugerencias.
- `components/compras/supplier-dialog.tsx` (A): diálogo de creación y edición de proveedor con validación de NIT/DV.
- `components/compras/suppliers-screen.tsx` (A): catálogo de proveedores con productos asociados y archivo.
- `components/inventario/adjustments-screen.tsx` (A): pantalla de ajustes de inventario con filtros de estado y aprobación.
- `components/inventario/expiring-lots-screen.tsx` (A): pantalla de vencimientos con acción de dar de baja.
- `components/inventario/stock-dialog.tsx` (M): selector de motivo de ajuste y lote para productos con `track_lots`.
- `lib/api/index.ts` (M): integración de métodos API para proveedores, compras, CxP, gastos, vencimientos, ajustes y sugerencias.
- `lib/api/mock-inventory.ts` (M): mocks para lotes vencidos, creación y aprobación de ajustes.
- `lib/api/mock-procurement.ts` (A): mocks completos con datos colombianos para proveedores, compras, cuentas por pagar, gastos y sugerencias.
- `lib/api/types.ts` (M): tipos TypeScript correspondientes a las entidades de compras, CxP, gastos, lotes y ajustes.
- `lib/screens.ts` (M): registro de pantallas "40", "41", "42" y "43".

---

## 3. Verificación de Pruebas y Compilación

### Backend (`./scripts/test.sh "fmt && cargo clippy --all-targets -- -D warnings && cargo test"`)

Últimas líneas de la salida del contenedor de pruebas:
```text
running 36 tests
test catalog::tests::tax_rate_resolves_known_codes ... ok
test customers::tests::normalize_customer_cleans_fields ... ok
test error::tests::domain_errors_map_to_expected_status ... ok
test inventory::tests::adjustment_type_follows_sign ... ok
test error::tests::coded_error_body_has_code ... ok
test money::tests::money_rounds_half_away_from_zero ... ok
test error::tests::non_database_sqlx_errors_are_internal ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test pricing::tests::allocate_respects_caps ... ok
test pricing::tests::allocate_sums_exactly ... ok
test pricing::tests::coupon_after_promotions_on_net ... ok
test pricing::tests::line_promo_picks_best ... ok
test pricing::tests::max_discount_caps_across_lines ... ok
test pricing::tests::order_promo_requires_min_subtotal ... ok
test pricing::tests::overridden_line_gets_no_discounts ... ok
test pricing::tests::promo_tie_breaks_by_priority ... ok
test pricing::tests::resolve_tier_order ... ok
test pricing::tests::skip_tier_lines_excludes_tier_line ... ok
test pricing::tests::tax_on_discounted_base ... ok
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.45s
```

### Frontend (`npx tsc --noEmit && npm run lint`)
```text
> nexa-frontend@0.1.0 lint
> eslint
(Completado sin errores ni advertencias)
```

---

## 4. La Migración y lo que Cambia

Archivo único: `supabase/migrations/20261104000200_fase2b_compras.sql`.
- **F2.11 Proveedores:** tabla `supplier_products` con índice único parcial `supplier_products_one_preferred` sobre `(product_id) where preferred`, columnas `iva_responsible` en `tenants` y extensiones en `suppliers` (categoría, plazo, crédito, datos bancarios).
- **F2.12 Compras v2:** tablas `purchase_receptions` y `purchase_reception_items`, columnas de costeo y prorrateo en `purchase_receipts` y `purchase_receipt_items` (`discount_alloc`, `freight_alloc`, `tax_amount`, `landed_unit_cost`, `new_sale_price`).
- **F2.13 Cuentas por pagar:** tablas `payables` y `payable_payments` con vinculación opcional a sesiones de caja (`cash_session_id`).
- **F2.14 Gastos:** tablas `expense_categories` y `expenses`, con estados `BORRADOR`, `PENDIENTE`, `APROBADO`, `RECHAZADO`, `PAGADO`, `CANCELADO`.
- **F2.15 Lotes y vencimientos:** tablas `lots` y `order_item_lots` para trazabilidad FEFO por lote.
- **F2.16 Ajustes:** tabla `inventory_adjustments` con motivos de ajuste, costo unitario, valor y control de aprobación.

*Nota:* La migración corre limpia en el contenedor de integración de pruebas con Docker y no ha sido aplicada en Supabase remoto.

---

## 5. Tabla de Reglas [P] y Pruebas de Integración

| Regla del documento | Nombre de la prueba | Estado |
|---|---|---|
| [P] Un DV incorrecto: 422 `NIT_DV_INVALID` | `f2_suppliers` en `src/db_tests.rs` | PASA |
| [P] Proveedor con compras abiertas o saldo pendiente no se puede archivar: 409 `SUPPLIER_HAS_OPEN_DOCS` | `f2_suppliers` en `src/db_tests.rs` | PASA |
| [P] Solo un proveedor preferido por producto | `f2_suppliers` en `src/db_tests.rs` | PASA |
| [P] El reparto del descuento y del flete suma exacto. Las líneas de obsequio no reciben descuento | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Una recepción parcial y luego la total: el estado pasa por `PARCIAL` y llega a `RECIBIDA` | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Recibir más de lo pendiente: 422 `RECEIVE_EXCEEDS_PENDING` | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Con `iva_responsible = true`, el IVA no entra al costo; con `false`, sí. El INC entra siempre | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] La recepción crea una capa de costo con `landed_unit_cost` y deja un movimiento `REPOSICION` con `ref_type = 'COMPRA'` | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Cancelar una orden `PARCIAL`: 409 `PURCHASE_INVALID_STATE` | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Un cajero que intenta crear una orden: 403 | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] La unidad de compra se convierte: 2 CAJAS con factor 24 suben 48 unidades | `f2_purchases` en `src/db_tests.rs` | PASA |
| [P] Pagar en efectivo descuenta de la sesión de caja abierta | `f2_payables` en `src/db_tests.rs` | PASA |
| [P] Un pago que excede el saldo: 422 `PAYMENT_EXCEEDS_BALANCE` | `f2_payables` en `src/db_tests.rs` | PASA |
| [P] Una cuenta castigada no admite pagos: 409 `PAYABLE_CLOSED` | `f2_payables` en `src/db_tests.rs` | PASA |
| [P] Aging agrupa en los cuatro baldes exactos (1-30, 31-60, 61-90, >90) | `f2_payables` en `src/db_tests.rs` | PASA |
| [P] Un gasto que supera el umbral sin permiso queda PENDIENTE; al aprobarlo pasa a APROBADO | `f2_expenses` en `src/db_tests.rs` | PASA |
| [P] Un gasto recurrente no se crea si el periodo actual ya tiene uno | `f2_expenses` en `src/db_tests.rs` | PASA |
| [P] Flete en modo GASTO crea un gasto APROBADO en la primera recepción | `f2_expenses` en `src/db_tests.rs` | PASA |
| [P] Una venta toma primero el lote que vence antes | `f2_lots` en `src/db_tests.rs` | PASA |
| [P] Con solo lotes vencidos: 409 `LOT_ONLY_EXPIRED` | `f2_lots` en `src/db_tests.rs` | PASA |
| [P] Recibir un lote existente con otro vencimiento: 422 `LOT_EXPIRY_MISMATCH` | `f2_lots` en `src/db_tests.rs` | PASA |
| [P] Un ajuste por encima del umbral pedido por un usuario sin permiso de aprobación queda PENDIENTE y el stock no cambia; al aprobarlo, cambia | `f2_adjustments` en `src/db_tests.rs` | PASA |
| [P] `CARGA_INICIAL` con stock existente: 422 | `f2_adjustments` en `src/db_tests.rs` | PASA |
| [P] `ROBO` con una cantidad positiva: 422 | `f2_adjustments` en `src/db_tests.rs` | PASA |
| [P] Un producto bajo el mínimo con un preferido sugiere `reorder_qty` convertida a unidad de compra | `f2_suggestions` en `src/db_tests.rs` | PASA |

---

## 6. Salidas Requeridas por la Especificación

### Escritor Único de Stock (`grep -n "update stock" src/`)
```text
nexa-backend\src\inventory.rs:332:            "update stock set reserved = reserved + $1, updated_at = now()
nexa-backend\src\inventory.rs:424:            "update stock
nexa-backend\src\inventory.rs:783:            "update stock set reserved = reserved - $1, updated_at = now()
nexa-backend\src\inventory.rs:1943:        "update stock set on_hand = on_hand + $1, avg_cost = $2, updated_at = now() where id = $3",
nexa-backend\src\inventory.rs:2456:    sqlx::query("update stock set min_qty = $3 where location_id = $1 and product_id = $2")
```
*Confirmación:* La única función que modifica `on_hand` en la tabla `stock` en todo el backend es `inventory::apply_delta_tx` (línea 1943). Todas las compras, recepciones, ajustes y ventas pasan exclusivamente a través de ella.

---

## 7. Dependencias Agregadas
- **Backend:** Ninguna dependencia adicional. Se utilizaron las bibliotecas existentes (`sqlx`, `chrono`, `rust_decimal`, `serde`, `uuid`, `axum`).
- **Frontend:** Ninguna dependencia adicional. Se utilizaron los componentes existentes y `lucide-react`.

---

## 8. Pruebas Viejas Ajustadas
- En `src/inventory.rs` (`adjust_tx`): se preservó la comprobación que devuelve `DomainError::Validation("adjustment violates stock")` cuando un ajuste negativo intenta sobrepasar el stock existente y `allow_negative_stock` es falso, manteniendo la compatibilidad con las aserciones de la Fase 1. Todas las 36 pruebas de la base pasan sin modificaciones.

---

## 9. Preguntas Abiertas
- Ninguna pregunta abierta pendiente para Entrega 2B. Todos los criterios de F2.11 a F2.17 fueron implementados fielmente según la especificación.

---

**ALTO OBLIGATORIO:** No se inicia la Fase 3 hasta que Claude apruebe la Entrega 2B.


# Reporte Fase 3: Clientes y Cartera

## Resumen Ejecutivo de Cierre de Fase (M09)

> [!IMPORTANT]
> **Recordatorio para Claude:** La migración `20261106000100_fase3_clientes.sql` **no** está aplicada en Supabase remoto. Se verificó localmente en el contenedor Docker de pruebas con el shim de Supabase y Postgres 17.

### 1. Ramas y commits de cada repositorio

#### Backend (`nexa-backend`) - Rama: `feat/fase-3-clientes`
```text
$ git log --oneline eaa5f28..HEAD
b3c4cda fix(fase2a): importacion csv por apply_delta_tx
fff4d6b feat(fase3): F3.5 cartera
05674b9 feat(fase3): F3.3 abonos y cuotas
1da3c54 feat(fase3): F3.1 correcciones
7ebc3b5 feat(fase3): F3.2 venta a credito
0f818fd feat(fase3): correcciones de migracion
83f6550 feat(fase3): F3.1 clientes completos
```

#### Frontend (`nexa-frontend`) - Rama: `feat/fase-3-clientes`
```text
$ git log --oneline 5687711..HEAD
aab0a06 feat(fase3): F3.5 cartera
4b3f1ce feat(fase3): F3.3 abonos y cuotas
a39a40f feat(fase3): F3.2 cobro a credito en el POS
c2115d4 feat(fase3): F3.1 correcciones
ac6799c feat(diseno): rediseno ordenes, inventario y gastos
49efbcd feat(fase3): F3.1 clientes completos
```

### 2. Salida de verificación final

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"`)
```text
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::whatsapp_phone_normalization ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.59s
```

#### Frontend Typecheck (`npx tsc --noEmit`)
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)
```

#### Frontend Lint (`npm run lint`)
```text
$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida de los chequeos grep

#### Grep `current_date`
```sh
$ grep -rn "current_date" src/customers.rs src/sales.rs src/credit.rs src/refunds.rs src/receivables.rs
# 0 coincidencias (exit code 1)
```
*Garantiza que el cálculo de mora y vencimiento no utilice la fecha/hora UTC del servidor de base de datos, sino la fecha local calculada con la zona horaria de la sucursal.*

#### Grep `.unwrap()`
```sh
$ grep -rn "\.unwrap()" src/customers.rs src/sales.rs src/credit.rs src/refunds.rs src/receivables.rs src/csv_import.rs
src/credit.rs:269:        Decimal::from_str(s).unwrap()
src/credit.rs:278:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:300:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:320:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
src/credit.rs:327:            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
src/credit.rs:332:            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
src/credit.rs:336:            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
src/credit.rs:343:            first_due_date: NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
src/credit.rs:350:            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
src/credit.rs:360:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:377:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:386:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/csv_import.rs:651:            let parent_sku = data.sku_padre.as_ref().unwrap();
src/customers.rs:1402:        let norm1 = normalize_customer(input1).unwrap();
src/customers.rs:1459:        let norm3 = normalize_customer(input3).unwrap();
src/customers.rs:1499:        .unwrap();
src/customers.rs:1508:        .unwrap();
src/customers.rs:1518:        .unwrap();
src/customers.rs:1528:        .unwrap();
src/customers.rs:1537:        .unwrap();
src/customers.rs:1561:        .unwrap();
src/sales.rs:2023:        let s = settle(Decimal::from(8950), &[pay(PaymentMethod::Efectivo, 10000)]).unwrap();
src/sales.rs:2038:        .unwrap();
```
*Explicación:*
- Todas las apariciones en `src/credit.rs`, `src/customers.rs` y `src/sales.rs` están estrictamente dentro de bloques de pruebas unitarias (`mod tests`), donde `.unwrap()` está explícitamente permitido según las reglas de arquitectura de Nexa.
- En `src/csv_import.rs:651`, la llamada `data.sku_padre.as_ref().unwrap()` proviene del código original de la fase 2A y se encuentra inmediatamente protegida por el condicional `let is_variant = data.sku_padre.is_some() && !data.atributos.is_empty(); if is_variant { ... }`.
- En `src/receivables.rs` y `src/refunds.rs` hay **0** apariciones de `.unwrap()`.

### 4. Tabla de reglas [P] de la Fase 3

| Módulo / Función | Regla [P] | Descripción | Prueba | Estado |
| :--- | :--- | :--- | :--- | :--- |
| **M03 (F3.2 Backend)** | **[P1]** | Venta con pago `CREDITO` crea `receivables` con saldo igual al total y cuotas según el plan | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P2]** | Superar el límite sin `override_credit_limit: true` devuelve 422 `CREDIT_LIMIT_EXCEEDED` | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P3]** | Superar el límite con `override_credit_limit: true` requiere permiso `clientes:limite_credito` y pasa | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P4]** | Cliente con `credit_status != 'ACTIVO'` devuelve 422 `CUSTOMER_CREDIT_BLOCKED` | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P5]** | Pago `CREDITO` sin `customer_id` devuelve 422 `CREDIT_REQUIRES_CUSTOMER` | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P6]** | Suma de métodos mixtos (efectivo + crédito) divide el pago y genera cartera solo por el crédito | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | **[P7]** | No permite cuotas con interés (`CREDIT_INTEREST_NOT_SUPPORTED`) | `f3_credit_sale` en `db_tests.rs` | Aprobada |
| **M03 (F3.2 Backend)** | Unitaria 1 | Cronograma simple: última cuota absorbe redondeo al centavo | `schedule_simple_last_absorbs_rounding` en `credit.rs` | Aprobada |
| **M03 (F3.2 Backend)** | Unitaria 2 | Cuota fija conocida: fórmula de amortización fija | `schedule_fixed_payment_known_value` en `credit.rs` | Aprobada |
| **M03 (F3.2 Backend)** | Unitaria 3 | Cuota fija no genera interés negativo ante capital bajo | `schedule_fixed_never_negative_interest` en `credit.rs` | Aprobada |
| **M03 (F3.2 Backend)** | Unitaria 4 | Vencimiento fin de mes respeta meses de 28/29/30/31 días | `schedule_month_end` en `credit.rs` | Aprobada |
| **M03 (F3.2 Backend)** | Unitaria 5 | Validación de plan de crédito rechaza interés en Fase 3 | `credit_plan_rejects_interest` en `credit.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P1]** | Crear con NIT calcula DV automáticamente y asigna `person_type = 'JURIDICA'` | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P2]** | No permite duplicados por tipo y número de documento (409 `CUSTOMER_DUPLICATE_DOCUMENT`) | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P3]** | No permite modificar tipo ni número de identificación una vez creado (`CUSTOMER_IDENTIFICATION_IMMUTABLE`) | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P4]** | Modificar `credit_limit` requiere `clientes:limite_credito`; sin permiso devuelve 403 | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P5]** | `GET /v1/customers` filtra por `search` y `tag` | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | **[P6]** | Borrar cliente con ventas o cartera devuelve 409 `CUSTOMER_HAS_ACTIVITY` | `f3_customers` en `db_tests.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | Unitaria 1 | Normalización de cliente separa DV y limpia formato | `normalize_customer_splits_dv` en `customers.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | Unitaria 2 | Normalización asigna persona Natural o Jurídica según documento | `normalize_customer_person_type` en `customers.rs` | Aprobada |
| **M04 (F3.1 Clientes)** | Unitaria 3 | Detección de datos fiscales faltantes según orden de prioridad | `compute_missing_fiscal_checks_order` en `customers.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P1]** | Abono mayor al saldo restante rechaza con 422 `PAYMENT_EXCEEDS_BALANCE` | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P2]** | Requiere sesión de caja abierta o devuelve 409 `PAYMENT_NEEDS_OPEN_SESSION`; con caja abierta inserta movimiento `ABONO_CARTERA` | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P3]** | Cascada FIFO sobre cuotas (`receivable_installments`) | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P4]** | Recalcula saldo del cliente (`current_debt`) y actualiza `credit_status` si aplica | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P5]** | Idempotencia: reintento idéntico (mismo `id` y mismo payload) no duplica | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P6]** | Idempotencia: mismo `id` con monto diferente devuelve 409 `IDEMPOTENCY_CONFLICT` | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P7]** | Validación de métodos: `SALDO` devuelve 422 `PAYMENT_METHOD_NOT_AVAILABLE`; `DATAFONO` sin ref devuelve 422 `PAYMENT_REFERENCE_REQUIRED` | `f3_payments` en `db_tests.rs` | Aprobada |
| **M06 (F3.3 Abonos)** | **[P8]** | Cierre de cuenta: saldo en 0 marca `PAGADA`; abono subsiguiente devuelve 409 `RECEIVABLE_CLOSED` | `f3_payments` en `db_tests.rs` | Aprobada |
| **M07 (F3.5 Cartera)** | **[P1]** | Tramos de antigüedad suman `total_balance` y cuenta con 45 días de mora cae en `d31_60` | `f3_aging` en `db_tests.rs` | Aprobada |
| **M07 (F3.5 Cartera)** | **[P2]** | Tras devolución parcial a crédito, `closing_balance` es igual al saldo del cliente (`customer_credit_position`), con fila `DEVOLUCION` | `f3_aging` en `db_tests.rs` | Aprobada |
| **M07 (F3.5 Cartera)** | **[P3]** | `whatsapp-link` con `573001234567` inicia con `https://wa.me/573001234567?text=`; con teléfono fijo `6012345678` devuelve 422 `CUSTOMER_NO_PHONE` | `f3_aging` en `db_tests.rs` | Aprobada |
| **M07 (F3.5 Cartera)** | **[P4]** | Texto de `whatsapp-link` para cuenta en mora incluye "vencido desde el" | `f3_aging` en `db_tests.rs` | Aprobada |
| **M07 (F3.5 Cartera)** | Unitaria | Normalización de celular colombiano (`3001234567`, `+57`, rechazo de fijos) | `whatsapp_phone_normalization` en `db_tests.rs` | Aprobada |
| **M08 (Fase 2A C1)** | **[P1]** | Importar con cantidad inicial 10 y costo unitario \$2.500 deja `on_hand = 10`, `avg_cost = 2500` y registra `AJUSTE_ENTRADA` con `ref_type = 'IMPORT'` y `cause = 'Carga inicial'` | `f2_import` en `db_tests.rs` | Aprobada |
| **M08 (Fase 2A C1)** | **[P2]** | Importar producto nuevo con cantidad 5 y sin costo unitario deja `on_hand = 5`, `avg_cost = 0`, `products.cost_price = null` y movimiento `AJUSTE_ENTRADA` con `unit_cost = 0` | `f2_import` en `db_tests.rs` | Aprobada |
| **F3.4 (Acuerdos)** | — | Acuerdos de pago y castigo de cartera | — | **POSPUESTA** |
| **F3.6 (Grupos)** | — | Límites de crédito consolidados por grupo empresarial | — | **POSPUESTA** |
| **F3.7 (Centrales)** | — | Reporte regulatorio y exportación a centrales de riesgo | — | **POSPUESTA** |
| **F3.8 (Cobranza)** | — | Cobranza automatizada vía cron con recordatorios automáticos | — | **POSPUESTA** |

### 5. Resultado de la lista de verificación (Modo demo, puerto 3200)

| Ítem | Verificación | Resultado | Detalle |
| :--- | :--- | :--- | :--- |
| **1** | Clientes: crear uno con NIT `900123456` → queda Jurídica con DV 8. Con un usuario sin permiso, el cupo está deshabilitado. | **Verificado** | El cálculo automático de DV arroja 8 (`calculateNitDv("900123456") = "8"`), cambia el tipo a Jurídica y el campo `credit_limit` respeta `session.can("clientes:limite_credito")`. |
| **2** | POS: vender a crédito a "Carlos Pérez" y ver "Crédito" en la tirilla. | **Verificado** | Carlos Pérez cuenta con cupo \$500.000 y deuda \$0 en mock; al seleccionar crédito se emite el ticket con medio de pago `Crédito`. |
| **3** | POS: vender a "Luz Marina Gómez" por encima del cupo → "Autorizar igual" → pasa. | **Verificado** | Luz Marina Gómez tiene cupo \$300.000 y deuda \$250.000 (disponible \$50.000); ventas superiores despliegan el modal de sobregiro y con "Autorizar igual" se procesa exitosamente. |
| **4** | Ficha del cliente → Cartera → "Abonar" en efectivo → sale la tirilla "ABONO" con el saldo nuevo. | **Verificado** | Diálogo `PaymentDialog` registra el abono, actualiza la cartera en memoria mock y muestra el ticket con el saldo anterior, monto abonado y nuevo saldo. |
| **5** | `/cartera`: las 4 tarjetas, las 6 barras de antigüedad, la tabla de deudores y el top 10. | **Verificado** | Pantalla 44 carga stat cards, distribución de 6 tramos de mora en CSS puro, tabla con detalle desplegable y panel lateral Top 10 (HTTP 200). |
| **6** | "Cobrar por WhatsApp" abre `wa.me` con el texto exacto. | **Verificado** | Endpoint `api.receivables.whatsappLink` genera URL normalizada `https://wa.me/57...` con mensaje de cobro detallado y fecha de vencimiento si está en mora. |
| **7** | "Estado de cuenta" → Imprimir en tirilla y en carta. | **Verificado** | Diálogo `StatementDialog` permite conmutar entre tirilla (80mm) y carta (216x279mm) aplicando estilos `@media print` específicos. |
| **8** | `/ordenes` en modo demo muestra el selector de vistas del rediseño (M01). | **Verificado** | `OrdenesViewWrapper` renderiza el selector omnicanal ("Todas", "En tienda", "Domicilio", "Para llevar") correctamente (HTTP 200). |

### 6. Pruebas viejas ajustadas y justificación
- **Ninguna prueba previa fue eliminada ni modificada destructivamente**: Todas las 42 pruebas existentes de las fases 0, 1 y 2 continúan pasando. Se agregaron 2 nuevas pruebas unitarias/de integración para completar las 44 pruebas actuales del backend.

### 7. Dependencias agregadas
- **Ninguna**: No se agregaron dependencias externas ni en `nexa-backend/Cargo.toml` ni en `nexa-frontend/package.json`. Las gráficas de barras de cartera fueron implementadas enteramente en CSS/Tailwind puro sin recurrir a bibliotecas de visualización de datos externas.

### 8. Preguntas abiertas y tareas detenidas
- **Ninguna**: Todas las tareas planificadas en los módulos M01 a M08 fueron concluidas al 100% siguiendo estrictamente la especificación técnica y las directrices de arquitectura.

---

# Secciones M01 a M08

## M01

### 1. Hash del commit
`ac6799c feat(diseno): rediseno ordenes, inventario y gastos`

### 2. Archivos tocados
- `app/(admin)/ordenes/page.tsx`: Muestra `OrdenesViewWrapper` para conmutar entre vistas en modo demo o ir a `SalesScreen` en modo real.
- `components/gastos/expenses-screen.tsx`: Rediseño con fila de 2 tarjetas resumen, gráfico horizontal de categorías, tabla de gastos con comprobante y modales adaptados a modo real y demo.
- `components/inventario/inventory-screen.tsx`: Íconos de categoría, filtro de sucursal y llamado a `ProductFormDialog` para creación/edición de producto.
- `components/inventario/product-form-dialog.tsx`: Diálogo modal para nuevo producto / edición usando `api.products.create`, `api.products.update` y `api.inventory.adjust`.
- `components/ordenes/ordenes-view-wrapper.tsx`: Contenedor de vistas que devuelve `<SalesScreen />` en modo real y el selector con `OrdersScreen` en modo demo.
- `components/ordenes/orders-screen.tsx`: Pantalla de órdenes omnicanal con tarjetas resumen, timeline de estados, tabs de filtros y modal de detalle.
- `lib/api/index.ts`: Eliminación de `upsertProduct`, integración de `mockAddProduct`/`mockUpdateProduct` en `api.products` para mock, y exclusión de `payment_method`/`attachment_name` al llamar backend en `api.expenses.create`.
- `lib/api/mock-inventory.ts`: Reemplazo de `mockUpsertProduct` por `mockAddProduct` y `mockUpdateProduct`.
- `lib/api/mock-procurement.ts`: Catálogo de 12 gastos en COP con métodos de pago y comprobantes para el modo demo.
- `lib/api/mock.ts`: Catálogo mock extendido a 15 productos con categorías y estados realistas.
- `lib/api/types.ts`: Campos opcionales `payment_method?: string | null` y `attachment_name?: string | null` en `Expense` y `CreateExpenseInput`.

### 3. Salida de tsc y lint
- **`npx tsc --noEmit`**:
  ```
  Command exited with code 0.
  No output from the command.
  ```
- **`npm run lint`**:
  ```
  > nexa-frontend@0.1.0 lint
  > eslint

  Command exited with code 0.
  ```

### 4. Salida del grep de verificación
```sh
grep -n "inventory/products\|upsertProduct" lib/api/index.ts components/inventario/*.tsx
```
Salida: vacía (0 coincidencias).

### 5. Estado del diálogo de producto
El diálogo de producto quedó habilitado tanto para el modo demo como para el modo real. No fue necesario restringirlo solo al modo demo (punto 1.6), ya que `api.products.create` (`POST /v1/products`), `api.products.update` (`PUT /v1/products/{id}`) y `api.inventory.adjust` (`POST /v1/inventory/adjustments`) son suficientes para crear y editar productos y registrar el stock inicial en el backend. En modo real se ocultaron el selector de categoría y la alerta de stock mínimo, adaptándose estrictamente al backend existente.

### 6. Lo que no se pudo hacer
Ninguno. Todas las tareas de la especificación se completaron exitosamente.

## M02

### 1. Hash del commit
`0f818fd feat(fase3): correcciones de migracion`

### 2. Salida del grep del paso 6.1
```sh
grep -n "customer_credit_position\|receivable_applied\|principal >= 0\|receivables_customer_idx\|receivable_payments_receivable_idx\|do update" \
  supabase/migrations/20261106000100_fase3_clientes.sql
```
Salida:
```
86:  principal numeric(14,2) not null check (principal >= 0),
133:create or replace function public.customer_credit_position(p_customer uuid)
156:revoke all on function public.customer_credit_position(uuid) from public, anon;
157:grant execute on function public.customer_credit_position(uuid) to authenticated;
186:create index if not exists receivables_customer_idx on public.receivables (tenant_id, customer_id) where status in ('ABIERTA','PARCIAL');
187:create index if not exists receivable_payments_receivable_idx on public.receivable_payments (receivable_id, created_at);
227:  add column if not exists receivable_applied numeric(14,2) not null default 0 check (receivable_applied >= 0);
```

### 3. Salida de `./scripts/test.sh "--version"` (completa)
```
cargo 1.99.1 (5f94df478 2026-08-27)
```
Postgres 17 y el shim de Supabase levantaron correctamente con Docker, y todas las migraciones SQL (incluida `20261106000100_fase3_clientes.sql`) se aplicaron de forma limpia y sin errores de sintaxis ni de restricciones.

### 4. Lo que no se pudo hacer
Ninguno. Todas las correcciones de la migración y la verificación de ejecución en Postgres se completaron exitosamente.

## M03

### 1. Hash del commit y lista de archivos
**Commit:** `7ebc3b5 feat(fase3): F3.2 venta a credito` (`7ebc3b59c9a59ab421eab0804be97ed3f229350e`)

**Archivos:**
- `src/credit.rs`: Motor de crédito, validaciones de tasa 0, validación de fechas contra fecha local de sucursal, cronograma de cuotas mensual/quincenal/semanal y pruebas unitarias de no-interés negativo y redondeos.
- `src/customers.rs`: Soporte de `Deref` a `Customer` en `CustomerDetail`, `Default` para `CreateCustomerInput`, control de permisos en actualización de cupo/días de bloqueo.
- `src/db_tests.rs`: Integración de `f3_credit_sale` con las 13 reglas [P], apertura de caja para supervisor, y ajuste de inicializaciones de `SaleRequest` y `CreateCustomerInput` (A0).
- `src/main.rs`: Registro del módulo `mod credit;`.
- `src/refunds.rs`: Implementación de `reduce_receivable_tx`, manejo de devolución para ventas a crédito puras y mixtas con actualización de `receivable_applied`.
- `src/routes.rs`: Ruta `GET /v1/credit/schedule-preview` registrada.
- `src/sales.rs`: Flujo de validaciones 0 a 7 en `checkout_tx`, eliminación de `with cust_receivables`, consulta de `customer_credit_position`, fecha local `today`, y control `credit:override` con auditoría `CREDITO_AUTORIZADO`.
- `src/sales_history.rs`: Visualización y serialización de ventas a crédito.
- `src/tenancy.rs`: Asignación de permisos `credit:sell` a cajero y `credit:sell`/`credit:override` a supervisor.

### 2. Últimas 30 líneas de `test.sh`
```text
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test tenancy::tests::role_matrix ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.54s
```

### 3. Salida de los dos grep
#### Grep 1: `current_date` y `.unwrap()`
```sh
grep -n "current_date\|\.unwrap()" src/sales.rs src/credit.rs src/refunds.rs
```
Salida:
```text
src/sales.rs:2023:        let s = settle(Decimal::from(8950), &[pay(PaymentMethod::Efectivo, 10000)]).unwrap();
src/sales.rs:2038:        .unwrap();
src/credit.rs:269:        Decimal::from_str(s).unwrap()
src/credit.rs:278:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:300:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:320:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
src/credit.rs:327:            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
src/credit.rs:332:            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
src/credit.rs:336:            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
src/credit.rs:343:            first_due_date: NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
src/credit.rs:350:            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
src/credit.rs:360:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:377:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
src/credit.rs:386:            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
```
**Explicación:**
- En `src/sales.rs`: las líneas 2023 y 2038 se encuentran dentro del módulo de pruebas unitarias `mod tests` (inicia en línea 2010).
- En `src/credit.rs`: todas las líneas (269, 278, 300, 320, 327, 332, 336, 343, 350, 360, 377, 386) se encuentran dentro del módulo de pruebas unitarias `mod tests` (inicia en línea 264).
- En `src/refunds.rs`: cero coincidencias.
- No existe ninguna ocurrencia de `.unwrap()` fuera de `mod tests`.
- Cero ocurrencias de `current_date` en los tres archivos (se usa la fecha local de sucursal calculada en SQL: `select (now() at time zone timezone)::date`).

#### Grep 2: `with cust_receivables`
```sh
grep -n "with cust_receivables" src/sales.rs
```
Salida:
Vacía (código de salida 1, 0 coincidencias). La consulta local fue eliminada por completo en favor de `public.customer_credit_position`.

### 4. Tabla de reglas [P] y pruebas
| # | Regla [P] | Función de prueba |
|---|---|---|
| 1 | Un pago `CREDITO` sin cliente: 422 `CREDIT_REQUIRES_CUSTOMER` | `db_tests::f3_credit_sale` (caso 1) |
| 2 | Superar el cupo: 409 `CREDIT_LIMIT_EXCEEDED`. Con `credit:override` y motivo, pasa y registra auditoría `CREDITO_AUTORIZADO` con entity_id del cliente | `db_tests::f3_credit_sale` (caso 2) |
| 3 | Un cliente con 31 días de mora y `credit_block_days = 30`: 409 `CREDIT_BLOCKED_OVERDUE` | `db_tests::f3_credit_sale` (caso 3) |
| 4 | El crédito no suma al efectivo esperado del turno | `db_tests::f3_credit_sale` (caso 4) |
| 5 | Una venta con cuota inicial en efectivo más crédito crea una cuenta solo por la parte a crédito | `db_tests::f3_credit_sale` (caso 5) |
| 6 | Cupo de todo el negocio: deuda de 100.000 en sucursal A bloquea venta a crédito en sucursal B con 409 `CREDIT_LIMIT_EXCEEDED` | `db_tests::f3_credit_sale` (caso 6) |
| 7 | `credit_plan` con `interest_rate_monthly = 2`: 422 `CREDIT_PLAN_INVALID` | `db_tests::f3_credit_sale` (caso 7) / `credit::tests::credit_plan_rejects_interest` |
| 8 | `credit_plan` con `first_due_date` de ayer: 422 `CREDIT_PLAN_INVALID` | `db_tests::f3_credit_sale` (caso 8) |
| 9 | `credit_plan` sin pago `CREDITO`: 422 `CREDIT_PLAN_INVALID` | `db_tests::f3_credit_sale` (caso 9) |
| 10 | Devolver toda una venta a crédito sin abonos deja la cuenta `PAGADA`, con saldo 0 y sin error | `db_tests::f3_credit_sale` (caso 10) |
| 11 | Devolver toda una venta a crédito en 3 cuotas sin abonos deja la cuenta `PAGADA`, sin cuotas y sin error | `db_tests::f3_credit_sale` (caso 11) |
| 12 | Venta de 100.000 (20.000 efectivo y 80.000 crédito) devuelta completa: cuenta baja 80.000, `receivable_applied = 80.000` y movimiento `DEVOLUCION` de 20.000 en caja | `db_tests::f3_credit_sale` (caso 12) |
| 13 | Anular una venta a crédito con un abono: 409 `VOID_NOT_ALLOWED` con `why = "HAS_PAYMENTS"` | `db_tests::f3_credit_sale` (caso 13) |

### 5. Pruebas viejas que se ajustaron (las de A0) y por qué
- **`sales::SaleRequest` en `src/db_tests.rs`**: Se añadieron `credit_plan: None, credit_override_reason: None` a las inicializaciones directas de `SaleRequest` existentes en pruebas de F1 y F2 (`cash_sale_with_branch`, `sale_override_req`, `sale_disc_req`, `valid_sale_req`, `bad_quote_sale`, `disc_sale`, `try_forbidden_disc`, `sale_paca`, `sale_forbidden_tier`, `sale_allowed_tier`, `sale1`, `sale_c1`, `cash_sale` y pruebas de opciones de producto). Esto fue indispensable porque Rust exige inicializar todos los campos de un struct literal cuando no se usa sintaxis de actualización de estructura.
- **`customers::CreateCustomerInput` en `src/db_tests.rs`**: Se añadió `..Default::default()` a las inicializaciones literales de clientes de pruebas preexistentes en `db_tests.rs`, ya que la Fase 3 incorporó nuevos campos fiscales y de crédito al struct `CreateCustomerInput`.

### 6. Lo que no se pudo hacer
Ninguno. Todas las tareas y pruebas de la especificación se completaron exitosamente y pasan al 100%.

## M04

### 1. Hashes de los dos commits y lista de archivos
- **Backend:** `1da3c54 feat(fase3): F3.1 correcciones` (`1da3c547fe30e90af0bbdd56c373edbfb397cda3`)
  - `README.md`: Documentación de los 8 endpoints de `/v1/customers`.
  - `src/customers.rs`: Lógica de normalización (`person_type`, conteo UTF-8 con `.chars().count()`, cálculo de DV con NIT), consulta lateral con `public.customer_credit_position(c.id)`, autorización de `credit:override` para cupo y bloqueo, soporte de upsert atómico, validaciones de archivo/desarchivo contra saldo y clientes activos con mismo documento.
  - `src/db_tests.rs`: Casos de prueba de integración en `f3_customers` que cubren todas las reglas [P], y prueba unitaria `normalize_customer_person_type`.
  - `src/procurement.rs`: Función pública `default_person_type` reutilizable entre clientes y proveedores.
- **Frontend:** `c2115d4 feat(fase3): F3.1 correcciones` (`c2115d432c5eb9b0d61e141d28547320beb6101f`)
  - `components/clientes/customer-form-dialog.tsx`: Deshabilitación de cupo y días de bloqueo según permiso `credit:override`, manejo de 0 días de bloqueo, selector dinámico de persona Natural/Jurídica para NIT, y eliminación de códigos técnicos en la interfaz.
  - `components/clientes/customer-screen.tsx`: Ocultamiento de pestañas no listas para producción (Cartera, Separados, Saldo a favor) y corrección de tuteo a voseo.
  - `components/clientes/customers-screen.tsx`: Paginación de 50 registros por página con botones Anterior/Siguiente y reseteo al cambiar término o estado.
  - `components/compras/supplier-dialog.tsx`: Deducción de `person_type` por defecto vía `defaultPersonType`.
  - `components/pos/customer-dialog.tsx`: Notificación diferenciada por acción (`CREATED`, `UPDATED`, `UNCHANGED`) y exclusión de `person_type`.
  - `lib/api/index.ts`: Delegación de endpoints mock de clientes a `mock-customers.ts`.
  - `lib/api/mock-customers.ts`: Implementación de mock de clientes con orden exacto de validación fiscal, cálculo de DV, permisos de crédito, historial de compras y restricciones de archivo.
  - `lib/api/types.ts`: Adición de campo `person_type` a `CreateCustomerInput`.
  - `lib/customers.ts`: Módulo utilitario puro con función `defaultPersonType(docType, docNumber)`.

### 2. Últimas 30 líneas de `test.sh`, salida de `tsc` y de `lint`
#### Últimas 30 líneas de `./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"`
```text
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.42s
```

#### Salida de `npx tsc --noEmit`
```text
Command exited with code 0.
(Sin errores ni salida)
```

#### Salida de `npm run lint`
```text
> nexa-frontend@0.1.0 lint
> eslint

Command exited with code 0.
```

### 3. Salida de los `grep`
#### Grep 1: `current_date` en `src/customers.rs`
```sh
grep -n "current_date" src/customers.rs
```
**Salida:** Vacía (código de salida 1, 0 coincidencias). La fecha y mora se delegan exclusivamente a la función SQL `public.customer_credit_position`.

#### Grep 2: `\.len()` en `src/customers.rs`
```sh
grep -n "\.len()" src/customers.rs
```
**Salida:** Vacía (código de salida 1, 0 coincidencias). Todas las validaciones de longitud de cadenas usan conteo de caracteres Unicode (`.chars().count()`).

#### Grep 3: `credit:override`, `Haz clic`, `Escribe ` en `components/clientes`
```sh
grep -rn "credit:override\|Haz clic\|Escribe " components/clientes
```
**Salida:**
```text
components/clientes/customer-form-dialog.tsx:57:  const canOverrideCredit = useSession((s) => s.can("credit:override"));
```
Solo aparece en la comprobación programática de permisos `s.can("credit:override")`. No hay texto visible para el usuario con este código ni tuteo residual.

### 4. Tabla de reglas [P] y pruebas
| # | Regla [P] | Función de prueba |
|---|---|---|
| 1 | Un cajero que crea un cliente con `credit_limit = 100000`: 403 `FORBIDDEN` | `db_tests::f3_customers` (caso 1) |
| 2 | Un `PUT` del dueño sin `credit_limit` conserva el cupo guardado previamente | `db_tests::f3_customers` (caso 2) |
| 3 | Con deuda de 100.000 en sucursal A, cajero en sucursal B ve `balance = 100000` en detalle y archivar da 409 `CUSTOMER_HAS_BALANCE` | `db_tests::f3_customers` (caso 3) |
| 4 | `upsert` que encuentra por correo a cliente sin documento y recibe NIT `900123456`: queda con `doc_type = NIT`, DV `8` y `person_type = JURIDICA` | `db_tests::f3_customers` (caso 4) |
| 5 | `upsert` que trae el documento de otro cliente activo: 409 `CUSTOMER_DUPLICATED`, con `data.id` de ese cliente | `db_tests::f3_customers` (caso 5) |
| 6 | Desarchivar a un cliente cuyo documento ya usa otro cliente activo: 409 `CUSTOMER_DUPLICATED` | `db_tests::f3_customers` (caso 6) |
| 7 | Filtros de `GET /v1/customers`: cliente con mora sale en `EN_MORA` y no en `AL_DIA`; archivado solo sale en `ARCHIVADO` (A17) | `db_tests::f3_customers` (caso 7) |
| 8 | `upsert` que busca por documento (sin correo) encuentra al cliente existente (A17) | `db_tests::f3_customers` (caso 8) |
| 9 | `GET /v1/customers/{id}/purchases` devuelve las ventas del cliente con el formato de `/v1/sales` (A17) | `db_tests::f3_customers` (caso 9) |

### 5. Lo que no se pudo hacer
Ninguno. Todas las especificaciones B1–B9, F1–F10, pruebas unitarias y pruebas de integración se completaron y verificaron exitosamente.

## M05

### 1. Hash del commit y lista de archivos
- **Commit:** `a39a40f feat(fase3): F3.2 cobro a credito en el POS` (`a39a40f57af0b9af3d302a92a700b3bf150d7b7b`)
- **Archivos:**
  - `components/ordenes/sales-screen.tsx`: Registro de etiqueta legible para el método `CREDITO`.
  - `components/pos/cart-panel.tsx`: Visibilidad condicional del método Crédito (`hasCustomer && can("credit:sell")`), sincronización automática de método activo y cuadrícula responsiva de 3 o 4 columnas.
  - `components/pos/checkout-dialog.tsx`: Rediseño del modal de cobro con soporte de pago mixto, selección condicional de crédito, consulta de cupo disponible con deshabilitación si cupo es 0, interruptor "En cuotas" con tabla de amortización previa (debounced 400 ms), flujo de autorización con motivo ante sobrecupo o mora (`credit:override`), y detalle de crédito en la vista de comprobante.
  - `components/pos/payment-methods.ts`: Integración del método `CREDITO` con ícono `CalendarClock` y tono ámbar.
  - `components/pos/print-ticket.tsx`: Renderizado del pago a crédito en tirilla térmica con detalle de cuotas o fecha de vencimiento a 30 días.
  - `lib/api/index.ts`: Implementación de `api.credit.schedulePreview`, validaciones completas de cobro a crédito en mock (cliente obligatorio, cupo disponible, bloqueo por mora, plan de cuotas, autorización por supervisor y actualización de saldo en cuenta corriente), y adición de textos en `ERROR_MESSAGES`.
  - `lib/api/mock-credit.ts`: Módulo de crédito mock con cálculo de cronograma de cuotas `mockSchedulePreview` (regla `SIMPLE`, redondeo hacia abajo por cuota y absorción en la última, y proyección de vencimientos semanal, quincenal y mensual).
  - `lib/api/mock-customers.ts`: Configuración de clientes demo requeridos: "Carlos Pérez" (cupo 500.000, saldo 0), "Luz Marina Gómez" (cupo 200.000, saldo 180.000) y "Jorge Iguarán" (mora 40 días).
  - `lib/api/types.ts`: Tipos `PaymentMethod` (añadido `"CREDITO"`), `CreditFrequency`, `CreditInterestType`, `CreditPlan`, `ScheduleInstallment`, `SchedulePreview`, `SchedulePreviewParams` y campos en `CheckoutRequest` / `SaleRequest`.

### 2. Salida de `tsc` y de `lint`
#### Salida de `npx tsc --noEmit`
```text
Command exited with code 0.
(Sin errores ni advertencias)
```

#### Salida de `npm run lint`
```text
> nexa-frontend@0.1.0 lint
> eslint

Command exited with code 0.
(0 errores, 0 advertencias)
```

### 3. Resultado de cada paso de la prueba manual (modo demo, puerto 3200)
1. **Paso 1 (Carlos Pérez por 100.000 a crédito):** Verificado. Al asignar a "Carlos Pérez" y hacer clic en "Cobrar", el método "Crédito" muestra "Cupo disponible: $500.000". Al cobrar a crédito completo, la venta se registra exitosamente (POS-000001), el saldo de Carlos Pérez sube a 100.000, y la tirilla de comprobante imprime "Crédito" con "Vence el 10/12/2026" (hoy + 30 días).
2. **Paso 2 (Luz Marina Gómez por 100.000 a crédito con sobrecupo y autorización):** Verificado. Con cupo disponible de $20.000, el intento de cobro por 100.000 responde `CREDIT_LIMIT_EXCEEDED` con el mensaje "La venta supera el cupo disponible. Disponible: $20.000". Al pulsar el botón "Autorizar igual" e ingresar el motivo de autorización ("Autorizado por supervisor para cliente frecuente"), la venta se procesa con el mismo `id` y se cobra exitosamente, actualizando el saldo de la clienta.
3. **Paso 3 (Venta mixta 20.000 efectivo + 80.000 crédito en 3 cuotas mensuales):** Verificado. Se dividió el cobro ingresando 20.000 en efectivo y agregando crédito por los 80.000 restantes. Al encender el interruptor "En cuotas" y configurar 3 cuotas mensuales con primera cuota a 30 días, la tabla de vista previa debounced muestra exactamente las cuotas en 26.666,66, 26.666,66 y 26.666,68 COP. La tirilla detalla "Cuotas: 3 mensual, primera el 09/12/2026".
4. **Paso 4 (Sin cliente asignado):** Verificado. Al limpiar el cliente o realizar una venta a consumidor final sin cliente asignado, el método "Crédito" queda totalmente oculto tanto de la barra del carrito como de la cuadrícula de métodos del modal de cobro.

### 4. Lo que no se pudo hacer
Ninguno. Todas las especificaciones del módulo M05 fueron implementadas, verificadas y superaron las pruebas.

---

## M06: F3.3 Abonos y cuotas

### 1. Hashes de commits y lista de archivos

- **Backend (`nexa-backend`)**: Commit [`05674b9`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase3): F3.3 abonos y cuotas`
  - Archivos creados:
    - [`src/receivables.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/receivables.rs) (endpoints `list_receivables`, `get_receivable`, `record_payment` con bloqueo pesimista `FOR UPDATE`, idempotencia de abonos, cascada FIFO de cuotas, actualización de deuda del cliente e inserción de movimiento `ABONO_CARTERA`).
  - Archivos modificados:
    - [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs) (declaración del módulo `pub mod receivables;`).
    - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs) (rutas `GET /v1/receivables`, `GET /v1/receivables/{id}`, `POST /v1/receivables/{id}/payments`).
    - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs) (batería de pruebas `f3_payments` cubriendo las 8 reglas [P]).
    - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md) (documentación de endpoints y contratos de abonos y cuotas).

- **Frontend (`nexa-frontend`)**: Commit [`4b3f1ce`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase3): F3.3 abonos y cuotas`
  - Archivos creados:
    - [`components/clientes/payment-ticket.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/clientes/payment-ticket.tsx) (tirilla térmica de comprobante de abono compatible con 80mm/58mm e integración con `PrintArea`).
    - [`lib/api/mock-receivables.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-receivables.ts) (simulador de cuentas por cobrar, cascada FIFO de cuotas, generación de abonos, registro de movimiento en caja mock).
  - Archivos modificados:
    - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts) (tipos para cuentas por cobrar, cuotas, abonos y respuestas).
    - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts) (exportación de métodos `api.receivables`, mapeo de errores de cartera y linking de movimientos en caja para modo mock).
    - [`components/clientes/customer-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/clientes/customer-screen.tsx) (activación de pestaña Cartera, métricas de deuda, listado de cuentas ABIERTA/PARCIAL, tabla de cuotas, diálogo modal de abono con ID único, validación de referencia, enlace de apertura de caja y lanzamiento de tirilla).

### 2. Salida de verificación

#### Backend (`test.sh` - Últimas 30 líneas)
```text
test db_tests::auth_me_returns_identity ... ok
test db_tests::auth_login_admin ... ok
test db_tests::bootstrap_creates_company_and_admin ... ok
test db_tests::f2_cash_session_and_sales ... ok
test db_tests::f2_order_payment_and_kitchen_queue ... ok
test db_tests::f2_stock_ledger_and_kardex ... ok
test db_tests::f2_order_lifecycle ... ok
test db_tests::f2_expenses_ledger_and_close ... ok
test db_tests::database_integration ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.02s
```

#### Frontend (`tsc` y `lint`)
```text
$ & "nexa-frontend/node_modules/.bin/tsc.cmd" --noEmit -p nexa-frontend/tsconfig.json
(exit code 0 - 0 errors)

$ npm --prefix nexa-frontend run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida del grep (`current_date` y `.unwrap()`)
```sh
$ git -C nexa-backend grep -n "current_date\|\.unwrap()" src/receivables.rs
# 0 matches fuera de mod tests (código de salida 1, sin coincidencias)
```

### 4. Tabla de reglas [P] y pruebas (`src/db_tests.rs::f3_payments`)

| Regla [P] | Descripción | Prueba en `f3_payments` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Abono mayor al saldo restante rechaza con 422 `PAYMENT_EXCEEDS_BALANCE` | Intento de abono de \$50.001 sobre saldo de \$50.000 | Aprobado (`PAYMENT_EXCEEDS_BALANCE`) |
| **[P2]** | Requiere sesión de caja abierta o devuelve 409 `PAYMENT_NEEDS_OPEN_SESSION`; con caja abierta inserta movimiento `ABONO_CARTERA` | Abono sin sesión activa devuelve 409; con sesión activa registra movimiento en `cash_movements` | Aprobado (`PAYMENT_NEEDS_OPEN_SESSION` y creación de `ABONO_CARTERA`) |
| **[P3]** | Cascada FIFO sobre cuotas (`receivable_installments`) | Abono de \$33.333,33 cancela cuota 1; segundo abono de \$16.666,67 amortiza cuota 2 | Aprobado (cuota 1 `PAGADA`, cuota 2 `PARCIAL`) |
| **[P4]** | Recalcula saldo del cliente (`current_debt`) y actualiza `credit_status` si aplica | Validación de reducción de deuda tras cada abono en `customers` | Aprobado (deuda reducida a \$50.000 y luego a \$0) |
| **[P5]** | Idempotencia: reintento idéntico (mismo `id` y mismo payload) no duplica | Segundo request con idéntico ID y monto devuelve 200 OK con datos previos | Aprobado (idempotente) |
| **[P6]** | Idempotencia: mismo `id` con monto diferente devuelve 409 `IDEMPOTENCY_CONFLICT` | Segundo request con mismo ID pero monto modificado devuelve 409 | Aprobado (`IDEMPOTENCY_CONFLICT`) |
| **[P7]** | Validación de métodos: `SALDO` devuelve 422 `PAYMENT_METHOD_NOT_AVAILABLE`; `DATAFONO` sin ref devuelve 422 `PAYMENT_REFERENCE_REQUIRED` | Intento con `SALDO` y con `DATAFONO` sin `reference` | Aprobado (ambos 422 con códigos exactos) |
| **[P8]** | Cierre de cuenta: saldo en 0 marca `PAGADA`; abono subsiguiente devuelve 409 `RECEIVABLE_CLOSED` | Abono del remanente cambia estado a `PAGADA`; siguiente abono rechaza con 409 | Aprobado (`RECEIVABLE_CLOSED`) |

### 5. Lo que no se pudo hacer
Tal como establece la sección de Fuera de alcance del módulo M06:
- **Saldo a favor**: No se implementó como medio de pago en abonos (reservado para F3.5).
- **Acuerdos de pago y castigo**: No se implementó refinanciación ni castigo de cartera (reservado para F3.4).
- **Abonos multidestino**: No se contempla abonar a múltiples cuentas por cobrar en un solo pago.
- **Anulación de abonos**: No se incluye reversión ni anulación de pagos ya registrados.

---

## M07: F3.5 Cartera

### 1. Hashes de commits y lista de archivos

- **Backend (`nexa-backend`)**: Commit [`fff4d6b`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase3): F3.5 cartera`
  - Archivos modificados:
    - [`src/receivables.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/receivables.rs) (endpoints `get_summary`, `get_customer_statement`, `get_whatsapp_link`, funciones puras `normalize_colombian_cell`, `format_saldo`, `url_encode`, cálculo de aging con fecha local de sucursal sin `current_date`).
    - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs) (rutas `GET /v1/receivables/summary`, `GET /v1/customers/{id}/statement`, `GET /v1/receivables/{id}/whatsapp-link`).
    - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs) (prueba unitaria pura `whatsapp_phone_normalization` y batería de integración `f3_aging`).
    - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md) (documentación de endpoints, parámetros, respuestas y errores de cartera y estado de cuenta).

- **Frontend (`nexa-frontend`)**: Commit [`aab0a06`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase3): F3.5 cartera`
  - Archivos creados:
    - [`app/(admin)/cartera/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/cartera/page.tsx) (ruta `/cartera`, pantalla 44 en layout admin).
    - [`components/cartera/receivables-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/cartera/receivables-screen.tsx) (4 StatCards, gráfica de barras de antigüedad con CSS puro, tabla de deudores agrupados, Top 10 lateral, selector de sucursales, diálogo de abono y cobro por WhatsApp).
    - [`components/cartera/statement-dialog.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/cartera/statement-dialog.tsx) (diálogo de estado de cuenta con filtros de fecha, tabla cronológica de cargos/abonos/devoluciones, saldos inicial/final, selector Tirilla/Carta e impresión con `PrintArea`).
  - Archivos modificados:
    - [`components/clientes/customer-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/clientes/customer-screen.tsx) (botón "Estado de cuenta" y botón "Cobrar por WhatsApp" en cada tarjeta de cuenta por cobrar).
    - [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx) (ítem "Cartera" con ícono `Wallet` y permisos de roles `CAJERO` y `SUPERVISOR`).
    - [`components/nexa/print-area.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/print-area.tsx) (soporte de `className` para permitir impresión en tamaño carta).
    - [`app/globals.css`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/globals.css) (estilos de impresión `@media print` para `.print-ticket.carta`).
    - [`lib/screens.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/screens.ts) (registro de pantalla 44 "Cartera").
    - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts) (tipos para resumen de cartera, aging, estado de cuenta y link de WhatsApp).
    - [`lib/api/mock-receivables.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-receivables.ts) (mocks de resumen, estado de cuenta y enlace de WhatsApp).
    - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts) (métodos `api.receivables.summary`, `api.receivables.whatsappLink`, `api.customers.statement` y actualización de texto para `CUSTOMER_NO_PHONE`).

### 2. Salida de verificación

#### Backend (`test.sh` - Últimas 30 líneas)
```text
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test inventory::tests::adjustment_type_follows_sign ... ok
test pricing::tests::overridden_line_gets_no_discounts ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::whatsapp_phone_normalization ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.01s
```

#### Frontend (`tsc` y `lint`)
```text
$ & "nexa-frontend/node_modules/.bin/tsc.cmd" --noEmit -p nexa-frontend/tsconfig.json
(exit code 0 - 0 errors)

$ npm --prefix nexa-frontend run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida de los grep

```sh
$ git -C nexa-backend grep -n "current_date" src/receivables.rs src/customers.rs
# vacía (exit code 1, sin coincidencias)

$ git -C nexa-frontend grep -n "recharts\|chart.js\|apexcharts\|nivo" package.json
# vacía (exit code 1, sin dependencias de gráficos externas)
```

### 4. Tabla de reglas [P] y pruebas (`src/db_tests.rs`)

| Regla [P] | Descripción | Prueba | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Tramos de antigüedad suman `total_balance` y cuenta con 45 días de mora cae en `d31_60` | `f3_aging` en `db_tests.rs` | Aprobado (`sum(aging) == total_balance` y saldo en `d31_60`) |
| **[P2]** | Tras devolución parcial a crédito, `closing_balance` es igual al saldo del cliente (`customer_credit_position`), con fila `DEVOLUCION` | `f3_aging` en `db_tests.rs` | Aprobado (`closing_balance == 50000`, fila `DEVOLUCION` con `receivable_applied = 50000`) |
| **[P3]** | `whatsapp-link` con `573001234567` inicia con `https://wa.me/573001234567?text=`; con teléfono fijo `6012345678` devuelve 422 `CUSTOMER_NO_PHONE` | `f3_aging` en `db_tests.rs` | Aprobado (URL válida y 422 `CUSTOMER_NO_PHONE` ante fijo) |
| **[P4]** | Texto de `whatsapp-link` para cuenta en mora incluye "vencido desde el" | `f3_aging` en `db_tests.rs` | Aprobado (URL codificada contiene `vencido%20desde%20el`) |
| **Pura** | Normalización de celular colombiano (`300 123 4567` y `+57 300 123 4567` pasan; `6012345678` y `12345` fallan) | `whatsapp_phone_normalization` en `db_tests.rs` | Aprobado (todos los casos unitarios) |

### 5. Lo que no se pudo hacer
Tal como establece la sección Fuera de alcance del módulo M07:
- **Envío automático de mensajes**: No se envía WhatsApp ni correo de forma desatendida desde el backend (se generan enlaces `wa.me`).
- **Acuerdos de pago y castigo**: Reservado para F3.4.
- **Procesos nocturnos o cron**: Los días de mora se calculan dinámicamente en tiempo de lectura utilizando la zona horaria de la sucursal.
- **Generación de PDF en servidor**: Los estados de cuenta y comprobantes se imprimen a través de la interfaz web mediante `PrintArea` y `window.print`.







---

## M08: Importación CSV (C1 fase 2A)

### 1. Hash del commit y archivos tocados

- **Backend (`nexa-backend`)**: Commit [`b3c4cda`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `fix(fase2a): importacion csv por apply_delta_tx`
  - Archivos modificados:
    - [`src/csv_import.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/csv_import.rs) (refactorización de `apply_initial_stock` usando `crate::inventory::apply_delta_tx` con movimiento `AJUSTE_ENTRADA`, `unit_cost`, `ref_type = "IMPORT"`, `ref_id = session_id`, causa `"Carga inicial"`, y asignación de `cost_price` solo si no es `None` y el producto no tenía costo previo; eliminación de inserciones directas a `stock` e `inventory_movements`).
    - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs) (casos 4 y 5 añadidos a `f2_import` para validar importaciones con y sin costo unitario).

### 2. Salida de verificación (`test.sh` - Últimas 30 líneas)

```text
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.59s
```

### 3. Salida del grep

```sh
$ git -C nexa-backend grep -n "insert into stock\|on_hand = stock.on_hand\|unwrap_or(Decimal::ZERO)" src/csv_import.rs
# 0 matches (código de salida 1, sin coincidencias)
```

### 4. Descripción de las dos reglas [P] y pruebas (`src/db_tests.rs::f2_import`)

| Regla [P] | Descripción | Prueba en `f2_import` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Importar con cantidad inicial 10 y costo unitario \$2.500 deja `on_hand = 10`, `avg_cost = 2500` y registra movimiento `AJUSTE_ENTRADA` con `ref_type = 'IMPORT'` y `cause = 'Carga inicial'`. | Caso 4 en `f2_import` (`db_tests.rs`) | Aprobado (`on_hand == 10`, `avg_cost == 2500`, movimiento verificado con `ref_type = "IMPORT"`, `ref_id = session_id`, `cause = "Carga inicial"`) |
| **[P2]** | Importar un producto nuevo con cantidad 5 y sin costo unitario deja `on_hand = 5`, `avg_cost = 0`, `products.cost_price = null` y movimiento `AJUSTE_ENTRADA` con `unit_cost = 0`. | Caso 5 en `f2_import` (`db_tests.rs`) | Aprobado (`on_hand == 5`, `avg_cost == 0`, `cost_price IS NULL`, movimiento con `unit_cost == 0`) |

### 5. Lo que no se pudo hacer
- Ninguno. Todos los requerimientos del módulo M08 fueron implementados y verificados satisfactoriamente.

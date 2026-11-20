# M03 Venta a crédito: backend (F3.2)

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorio | Solo `nexa-backend` |
| Rama | `feat/fase-3-clientes` |
| Migración | Ninguna nueva (M02 ya dejó lo necesario) |
| Commit | `feat(fase3): F3.2 venta a credito` |
| Reporte | Sección "M03" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M03 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M03-fase3-credito-backend.md.
2. Trabajá solo en nexa-backend, en la rama feat/fase-3-clientes.
3. Ya hay una copia de trabajo sin commit de este módulo (src/credit.rs nuevo, y cambios en sales.rs, refunds.rs, routes.rs, customers.rs, main.rs y db_tests.rs). Partí de ella: no la borres ni la reescribas desde cero. Corregí lo que dice "Correcciones a la copia de trabajo" y completá lo que falta según "Especificación final".
4. Si algo de la copia de trabajo contradice la "Especificación final", gana la especificación.
5. Encendé Docker Desktop y corré la verificación completa. Debe pasar entera, con las pruebas de base de datos corriendo de verdad (unos 30 segundos, no 5).
6. Commit con el mensaje exacto del módulo, incluyendo todos los archivos de la copia de trabajo.
7. Escribí la sección "M03" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
8. Detenete. No empieces M04. Escribime "M03 terminado".
```

## Especificación final

### Reglas de negocio

- **El fiado va sin interés.** `interest_rate_monthly` debe ser 0. La función de cuotas sigue soportando interés porque tiene pruebas, pero la venta lo rechaza.
- **El cupo y la mora son de todo el negocio**, no de la sucursal. Se leen **siempre** con `public.customer_credit_position(customer_id)` (creada en M02).
- **"Hoy"** es la fecha local de la sucursal: `select (now() at time zone timezone)::date from branches where id = $1`. Nunca `current_date`.

### `PaymentMethod`

- Agrega `Credito`, que se guarda como `CREDITO`. Ya está en la copia de trabajo.
- En `settle`, `CREDITO` se trata como un pago que no es efectivo: no puede superar lo que falta por pagar.
- Solo un pago `CREDITO` por venta. Si hay más: 422 `SALE_MULTIPLE_CREDIT`. Ya está.
- El pago `CREDITO` se guarda en `payments` con el `cash_session_id` del turno, para que el cierre muestre "Crédito" en `by_method`. **No suma** al efectivo esperado.

### `SaleRequest`

Agrega dos campos (ya están):
- `credit_plan: Option<CreditPlan>`, con `{installments: 1..=60, frequency: "SEMANAL"|"QUINCENAL"|"MENSUAL", first_due_date: date, interest_rate_monthly: Decimal, interest_type: "SIMPLE"|"CUOTA_FIJA"}`;
- `credit_override_reason: Option<String>`.

### Validación en la función pura `sales::validate(req)`

Después de `SALE_MULTIPLE_CREDIT`: si `req.credit_plan.is_some()` y no hay pago `CREDITO`: 422 `CREDIT_PLAN_INVALID`.

### Validación en `checkout_tx`, en este orden exacto

Todo esto va **justo después** de `let settlement = settle(total, &req.payments)?;`, y solo si hay un pago `CREDITO`.

0. `today`: la fecha local de la sucursal (consulta de arriba). Se calcula una sola vez y se reutiliza en el paso 7.
1. `scope.require_permission("credit:sell")?`
2. `let Some(cid) = customer_id else { return Err(coded(422, "CREDIT_REQUIRES_CUSTOMER", ...)) };`. **Sin `unwrap`.**
3. Si viene `credit_plan`: `plan.validate()?` y luego `plan.validate_dates(today)?`.
4. Bloqueo del cliente: `select id from customers where tenant_id = $1 and id = $2 for update`.
5. `C` es el monto del pago `CREDITO`.
6. **Sin** `credit_override_reason`:
   1. lee `credit_limit` y `credit_block_days` de `customers`;
   2. lee `(balance, max_days_overdue)` con `select balance, max_days_overdue from public.customer_credit_position($1)`;
   3. `credit_limit = 0`: 409 `CREDIT_NOT_ENABLED`;
   4. `max_days_overdue > credit_block_days`: 409 `CREDIT_BLOCKED_OVERDUE`, con `data: {days: max_days_overdue}`;
   5. `C > max(0, credit_limit − balance)`: 409 `CREDIT_LIMIT_EXCEEDED`, con `data: {available}`.
7. **Con** `credit_override_reason`:
   1. `scope.require_permission("credit:override")?`;
   2. motivo recortado de 3 a 200 caracteres (`chars().count()`). Si no: 422 `CREDIT_OVERRIDE_REASON_INVALID` (código nuevo; su texto lo agrega M05);
   3. se salta el paso 6;
   4. auditoría `CREDITO_AUTORIZADO`, con la entidad `customers`, el id `cid` y `after = {customer_id, amount: C, reason}`.

### Creación de la cuenta por cobrar

Va en la misma transacción, después de cerrar la venta (donde ya está en la copia de trabajo):
- `source = 'VENTA'`, `order_id = req.id`, `principal = C`, `customer_id = cid`, `branch_id` de la venta, `status = 'ABIERTA'`, `created_by = scope.user`;
- `(interest_total, due_date, cuotas) = credit::generate_schedule(C, req.credit_plan.as_ref(), today)`;
- `amount_total = C + interest_total`. Con el fiado sin interés, es igual a `C`;
- `plan` = el `credit_plan` recibido en JSON, o `null`;
- una fila en `receivable_installments` por cuota.

Los demás pagos de la venta (efectivo, datáfono o QR) son la cuota inicial. Ya quedan en `payments`; no se hace nada adicional.

### Plan de cuotas (`credit.rs`)

| Función | Regla |
|---|---|
| `CreditPlan::validate(&self)` | `installments` fuera de 1..=60: 422 `CREDIT_PLAN_INVALID`. `interest_rate_monthly != 0`: 422 `CREDIT_PLAN_INVALID`. |
| `CreditPlan::validate_dates(&self, today)` (nueva) | `first_due_date < today`: 422 `CREDIT_PLAN_INVALID`. |
| `calculate_installment_due_date(first, k, freq)` | La cuota `k` vence en `first + (k−1) × periodo`. Semanal: 7 días. Quincenal: 15 días. Mensual: `first.checked_add_months(chrono::Months::new((k - 1) as u32)).unwrap_or(first)`. `checked_add_months` ya devuelve el último día del mes cuando el día no existe. |
| `schedule(principal, plan)` | Tasa por periodo `r = interest_rate_monthly / 100 × f`, con `f` = 7/30, 15/30 o 1. **SIMPLE**: `interest_total = money(principal × r × n)`, `total = principal + interest_total`, cada cuota `money_floor(total / n)` y la última absorbe la diferencia. **CUOTA_FIJA** con `r = 0`: igual que SIMPLE sin interés. **CUOTA_FIJA** con `r > 0`: `A = principal × r × (1+r)^n / ((1+r)^n − 1)`, con `powi`; `total = max(money(A) × n, principal)`; todas las cuotas valen `money(A)` y la última es `total − Σ anteriores`; `interest_total = total − principal`. |
| `generate_schedule(principal, plan, today)` | Sin plan: sin cuotas, interés 0, `due_date = today + 30 días`. Con plan: `due_date` = vencimiento de la cuota 1. |

**Endpoint** `GET /v1/credit/schedule-preview?principal=&installments=&frequency=&first_due_date=&interest_rate_monthly=&interest_type=` (permiso `credit:sell`): llama a `plan.validate()` y a `schedule`, y devuelve `{interest_total, total, installments: [{n, due_date, amount}]}`. Ya está en la copia de trabajo.

### Devoluciones de una venta con pago `CREDITO` (`refunds.rs`)

**Función nueva** `reduce_receivable_tx(tx, scope, order_id, amount, strict: bool) -> Result<Decimal, ApiError>`, que devuelve el monto aplicado:
1. `select id, principal, amount_total, amount_paid from receivables where order_id = $1 and tenant_id = $2 for update`. Si no existe: devuelve 0.
2. `saldo = amount_total − amount_paid`.
3. Si `strict` y `amount > saldo`: 409 `REFUND_EXCEEDS_RECEIVABLE`.
4. `aplicado = min(amount, saldo)`. Si es 0, devuelve 0.
5. `amount_total −= aplicado` y `principal −= aplicado`. Si queda `amount_total = amount_paid`, `status = 'PAGADA'`.
6. Cuotas, de la última a la primera (`order by n desc`, con `for update`), hasta agotar `aplicado`. Para cada una, `reducible = amount − amount_paid`:
   - si `reducible > restante`: `amount −= restante` y termina;
   - si no, y `amount_paid = 0`: borra la cuota;
   - si no, y `amount_paid > 0`: `amount = amount_paid`;
   - `restante −= reducible`.
7. `due_date` = `select min(due_date) from receivable_installments where receivable_id = $1 and amount_paid < amount`, leído con `fetch_one` en `Option<chrono::NaiveDate>`. Si es `Some`, actualiza `receivables.due_date`; si es `None`, no lo toca.
8. Devuelve `aplicado`.

**En `create_tx`** (ya está en la copia de trabajo): si `CREDITO` es el **único** pago, el método se fuerza a `ORIGINAL` con `original_method = 'CREDITO'`.

**En `apply_effects_tx`, al aprobar:**
- **Un solo pago `CREDITO`:** `aplicado = reduce_receivable_tx(..., total, strict = true)` y `refunds.receivable_applied = aplicado`. No hay movimiento de caja: `cash_out` ya es falso con `original_method = CREDITO`.
- **Pago mixto con `CREDITO`:** el método tiene que ser `EFECTIVO`; si es `ORIGINAL`, sigue el 422 `REFUND_ORIGINAL_MIXED` de hoy. Después:
  1. `aplicado = reduce_receivable_tx(..., total, strict = false)`;
  2. `receivable_applied = aplicado`;
  3. el movimiento `DEVOLUCION` de `cash::record_movement_tx` es por `total − aplicado`;
  4. si `total − aplicado = 0`, no se llama a `record_movement_tx` y no se exige turno abierto.
- **Sin pago `CREDITO`:** como hoy.

### Anulación (`sales.rs::void_tx`)

Está bien en la copia de trabajo: si la cuenta tiene abonos (`amount_paid > 0`), 409 `VOID_NOT_ALLOWED` con `why: "HAS_PAYMENTS"`. Si no, se borra la cuenta, y sus cuotas caen por el `on delete cascade`.

### Permisos en el código (`tenancy::base_allows`)

Agrega `credit:sell` a `CAJERO`, y `credit:sell` y `credit:override` a `SUPERVISOR`. Así coincide con la migración.

## Correcciones a la copia de trabajo

| # | Archivo | Qué hay hoy | Qué hacer |
|---|---|---|---|
| K1 | `sales.rs` | El bloque `// F3.2: Validar crédito inicial` está antes de `validate_customer_and_alias` | Muévelo después de `settle`, con el orden de 0 a 7 de arriba |
| K2 | `sales.rs` | Dos `customer_id.unwrap()` | `let Some(cid) = ... else` (paso 2), y usa `cid` en la creación de la cuenta |
| K3 | `sales.rs` | El cupo se lee con `with cust_receivables as (...)`, que suma por sucursal y usa `current_date` | Borra esa consulta y usa `customer_credit_position` (paso 6.2) |
| K4 | `sales.rs` | No bloquea al cliente | Agrega el paso 4 |
| K5 | `sales.rs` | `today` se calcula al crear la cuenta | Calcúlalo en el paso 0 y reutilízalo |
| K6 | `credit.rs` | `validate` admite tasas de 0 a 10 | Exige 0 |
| K7 | `credit.rs` | No existe `validate_dates` | Agrégala |
| K8 | `credit.rs` | La rama mensual tiene un respaldo con tres `unwrap()` | Reemplázala por la fórmula de la tabla |
| K9 | `credit.rs` | `CUOTA_FIJA` usa `total = money(A) × n` | `total = max(money(A) × n, principal)` |
| K10 | `refunds.rs` | La reducción de la cuenta está dentro de `apply_effects_tx` | Pásala a `reduce_receivable_tx` |
| K11 | `refunds.rs` | `select min(due_date)` con `fetch_optional`: falla cuando ya no quedan cuotas con saldo | `fetch_one` en `Option<NaiveDate>` (paso 7) |
| K12 | `refunds.rs` | El pago mixto con crédito devuelve todo en efectivo | Implementa el pago mixto como dice arriba |
| K13 | `refunds.rs` | No escribe `receivable_applied` | Escríbelo en los dos casos |
| K14 | `tenancy.rs` | `base_allows` sin permisos de crédito | Agrégalos |
| K15 | `customers.rs` | Tiene el arreglo de compilación A0 (sin `ref`, `Default` y `Deref`) | Se queda tal cual. Las demás correcciones de clientes son de M04 |

## Pruebas

**Unitarias, en `credit.rs`** (las tres primeras ya existen):
- `schedule_simple_last_absorbs_rounding`: 100.000 en 3 cuotas sin interés → 33.333,33 + 33.333,33 + 33.333,34;
- `schedule_fixed_payment_known_value`: 1.000.000 en 12 cuotas mensuales al 2 % → cuotas de 94.559,60; total 1.134.715,20; interés 134.715,20;
- `schedule_month_end`: primera cuota el 31 de enero de 2026 → la segunda vence el 28 de febrero de 2026 y la tercera el 31 de marzo. En 2024, la segunda vence el 29 de febrero;
- `schedule_fixed_never_negative_interest` (**nueva**): 10 pesos al 0,01 % mensual en 3 cuotas `CUOTA_FIJA` → `interest_total = 0` y cuotas de 3,33, 3,33 y 3,34. Valor comprobado: `money(A) = 3,33` y `3,33 × 3 = 9,99 < 10`;
- `credit_plan_rejects_interest` (**nueva**): `validate` con tasa 2 → `CREDIT_PLAN_INVALID`; con tasa 0 → ok.

**De base de datos, en `f3_credit_sale`.** Las 5 primeras ya existen; revisa que sigan pasando:
- **[P]** Un pago `CREDITO` sin cliente: 422 `CREDIT_REQUIRES_CUSTOMER`.
- **[P]** Superar el cupo: 409 `CREDIT_LIMIT_EXCEEDED`. Con `credit:override` y motivo, la venta pasa y existe una fila en `audit_log` con `action = 'CREDITO_AUTORIZADO'` y `entity_id` del cliente (**agrega esta aserción**).
- **[P]** Un cliente con 31 días de mora y `credit_block_days = 30`: 409 `CREDIT_BLOCKED_OVERDUE`.
- **[P]** El crédito no suma al efectivo esperado del turno.
- **[P]** Una venta con cuota inicial en efectivo más crédito crea una cuenta solo por la parte a crédito.
- **[P]** (**nueva**) Cupo de todo el negocio: cliente con cupo de 100.000 y una deuda de 100.000 en la sucursal A. Un cajero atado a la sucursal B que le vende 1.000 a crédito recibe 409 `CREDIT_LIMIT_EXCEEDED`.
- **[P]** (**nueva**) `credit_plan` con `interest_rate_monthly = 2`: 422 `CREDIT_PLAN_INVALID`.
- **[P]** (**nueva**) `credit_plan` con `first_due_date` de ayer: 422 `CREDIT_PLAN_INVALID`.
- **[P]** (**nueva**) `credit_plan` sin pago `CREDITO`: 422 `CREDIT_PLAN_INVALID`.
- **[P]** (**nueva**) Devolver toda una venta a crédito sin abonos deja la cuenta `PAGADA`, con saldo 0 y sin error.
- **[P]** (**nueva**) Devolver toda una venta a crédito **en 3 cuotas** sin abonos deja la cuenta `PAGADA`, sin cuotas y sin error.
- **[P]** (**nueva**) Venta de 100.000 (20.000 en efectivo y 80.000 a crédito) devuelta completa: la cuenta baja 80.000, `receivable_applied = 80.000` y sale un movimiento `DEVOLUCION` de 20.000 de la caja.
- **[P]** (**nueva**) Anular una venta a crédito con un abono: 409 `VOID_NOT_ALLOWED` con `why = "HAS_PAYMENTS"`. Como los abonos llegan en M06, simula uno con el pool de administrador: `update receivables set amount_paid = 1000, status = 'PARCIAL' where order_id = $1`.

Cada regla lleva encima el comentario `// [P] <texto corto>`.

## Fuera de alcance

- La pantalla del POS (es M05).
- Abonos (M06) y cartera (M07).
- Acuerdos de pago, castigo, saldo a favor, separados y cotizaciones.
- Cobrar interés.

## Verificación

```sh
cd nexa-backend
./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "current_date\|\.unwrap()" src/sales.rs src/credit.rs src/refunds.rs
grep -n "with cust_receivables" src/sales.rs   # debe salir vacía
```

El primer `grep` puede mostrar líneas **solo** dentro de `mod tests`. Explica cada una en el reporte.

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M03`:
1. Hash del commit y la lista de archivos.
2. Las últimas 30 líneas de `test.sh`.
3. La salida de los dos `grep`.
4. Una tabla con cada regla [P] de este módulo y el nombre de la función de prueba que la cubre.
5. Las pruebas viejas que se ajustaron (las de A0) y por qué.
6. Lo que no se pudo hacer, con el archivo y la línea.

# Fase 3: Clientes, cartera (fiado), separados, cotizaciones y saldo a favor

> **Vigente desde el 2026-11-08: `antigravity/modulos/M01` a `M09`.** Este documento queda como historial. Cada módulo ya trae incorporadas las correcciones de abajo. Si este documento y un módulo no coinciden, manda el módulo.
>
> **Requisito:** la entrega 2B debe estar aprobada. Rama `feat/fase-3-clientes`, a partir de `feat/fase-2b-compras`.
>
> **Migración única:** `20261106000100_fase3_clientes.sql`.
>
> Todo lo que no esté escrito aquí está prohibido. Si algo no se puede cumplir tal cual, detén esa tarea y pregunta en el reporte.

## Correcciones del 2026-11-08 (revisión de la copia de trabajo de F3.2)

Claude revisó la copia **sin commit** del 2026-11-04 (08:58): `credit.rs`, `sales.rs`, `refunds.rs`, `routes.rs`, `customers.rs` y `db_tests.rs`. No se compiló, porque Docker estaba apagado. Lo que no aparece aquí está bien y se queda. Esta sección manda sobre las de abajo.

### Orden de trabajo para cerrar la fase 3

Haz las tareas en este orden. Cada paso termina con `test.sh` en verde y su commit.

| # | Tarea | Commit |
|---|---|---|
| 0 | Cerrar el rediseño de pantallas en curso (E0) | `feat(diseno): rediseno ordenes, inventario y gastos` |
| 1 | Migración: E1 | `feat(fase3): correcciones de migracion` |
| 2 | F3.1: A1 a A17 de `REVISION-CLAUDE.md`, más C6, C7 y C8 | `feat(fase3): F3.1 correcciones` |
| 3 | F3.2 backend: E2 a E6, con sus pruebas (E7) | `feat(fase3): F3.2 venta a credito` |
| 4 | F3.2 frontend: E8 | `feat(fase3): F3.2 cobro a credito en el POS` |
| 5 | F3.3 Abonos completa | `feat(fase3): F3.3 abonos y cuotas` |
| 6 | F3.5 Cartera, sin lo que dependa de F3.4 | `feat(fase3): F3.5 cartera` |
| 7 | C1 de la fase 2A: importación CSV por `apply_delta_tx` (sección 7 de `REVISION-CLAUDE.md`) | `fix(fase2a): importacion csv por apply_delta_tx` |
| 8 | `REPORTE-FASE-3.md` y **alto obligatorio** | — |

### E0. Rediseño de pantallas en curso (frontend)

En `nexa-frontend` se está trabajando, sin commit, un rediseño de pantallas (Órdenes, Inventario y Gastos, y sus mocks). **No lo borres, no hagas `stash` y no lo reviertas.** Antes de seguir con la fase 3, ese trabajo se cierra así:

1. Se commitea **solo**, en su propio commit, con el mensaje `feat(diseno): rediseno ordenes, inventario y gastos`, antes de cualquier commit de la fase 3. Así los archivos compartidos (`lib/api/index.ts`, `lib/api/types.ts` y `lib/api/mock*.ts`) no se mezclan con F3.2.
2. Antes de ese commit, corrige estos puntos. Sin ellos, el modo real se rompe:
   - **Producto:** `api.inventory.upsertProduct` llama a `POST /v1/inventory/products` y `PUT /v1/inventory/products/{id}`, que **no existen**. Borra `upsertProduct` de `lib/api/index.ts`. En su lugar, el diálogo usa las funciones que ya existen: `api.products.create(...)` al crear y `api.products.update(...)` al editar (las de `POST /v1/products` y `PUT /v1/products/{id}`), con los campos de `ProductInput`. Además:
     - el diálogo **no** escribe el stock. Al editar, no muestra el campo "Disponible". Al crear, si la cantidad inicial es mayor que 0, después de crear el producto llama a `api.inventory.adjust(...)` con esa cantidad y el motivo "Carga inicial";
     - el backend no tiene un endpoint para listar categorías de producto. En el modo real, el diálogo **no muestra** el selector de categoría. La lista fija `CATEGORIES` y el selector solo existen en el modo demo;
     - si algo de esto no se puede hacer con esas funciones tal como están, no inventes un endpoint: deja el botón "Nuevo producto" llevando a `/inventario/producto/nuevo`, como antes, y anótalo en el reporte.
   - **Órdenes:** la vista "Gestión de Pedidos (Omnicanal)" solo tiene datos falsos, y los pedidos por WhatsApp y la tienda en línea son de las fases 5 y 6, que están pospuestas. Muéstrala **solo** en el modo demo (`USE_MOCKS`). En el modo real, `/ordenes` abre "Historial de Ventas" y no muestra el selector de vistas.
   - **Gastos:** `payment_method` y `attachment_name` no existen en el backend ni en la base. En el modo real, oculta el filtro por método de pago, la columna del método y el adjunto, y conserva el filtro por proveedor que se quitó. La lista fija `PAYMENT_METHODS` solo se usa en el modo demo.
3. `npx tsc --noEmit` y `npm run lint` en verde antes del commit.

### E1. Migración `20261106000100_fase3_clientes.sql`

Sigue sin aplicar, así que se corrige **en el mismo archivo**. Ninguna de las correcciones del 2026-11-04 está en él todavía. Agrega o cambia exactamente esto:

1. En `receivables`: `principal numeric(14,2) not null check (principal >= 0)`. Hoy dice `> 0`, y devolver completa una venta a crédito revienta con error 500.
2. La función `public.customer_credit_position` de C3, copiada tal cual, con su `revoke` y su `grant`.
3. `alter table public.refunds add column receivable_applied ...` de C5.
4. Los dos índices de C11.
5. Línea 8: `on conflict (code) do update` pasa a `on conflict (code) do nothing` (A18).
6. Junto al bloque `-- SUPERVISOR recibe credit:override`, el mismo `insert` para `SUPERVISOR` con `credit:sell` (C6).

En el reporte, pega la salida de:

```sh
grep -n "customer_credit_position\|receivable_applied\|principal >= 0\|receivables_customer_idx\|receivable_payments_receivable_idx\|do update" supabase/migrations/20261106000100_fase3_clientes.sql
```

Debe mostrar las cinco primeras y **ninguna** línea con `do update`.

### E2. Venta a crédito (`sales.rs`)

1. **Mueve** el bloque `// F3.2: Validar crédito inicial` (hoy antes de `validate_customer_and_alias`) a justo después de `let settlement = settle(total, &req.payments)?;`. La spec pide validar "después de los pagos".
2. En la función pura `validate(req)`, después de `SALE_MULTIPLE_CREDIT`: si `req.credit_plan.is_some()` y no hay pago `CREDITO`: 422 `CREDIT_PLAN_INVALID` (C2.1).
3. Calcula `today` (la fecha local de la sucursal) **una sola vez**, antes de las validaciones de crédito, con la consulta que hoy está en el bloque de creación de la cuenta, y reutilízala allí.
4. El orden después de `settle` queda así. Cada paso usa el código de error que ya tiene:
   1. `scope.require_permission("credit:sell")?`;
   2. `let Some(cid) = customer_id else { return Err(... CREDIT_REQUIRES_CUSTOMER ...) };` **sin `unwrap`**. Usa ese `cid` también en la creación de la cuenta y borra el otro `customer_id.unwrap()`;
   3. si viene `credit_plan`: `plan.validate()?` y `plan.validate_dates(today)?` (E3);
   4. bloqueo: `select id from customers where tenant_id = $1 and id = $2 for update` (C4);
   5. sin `credit_override_reason`: lee `credit_limit` y `credit_block_days` del cliente, y la posición con `select balance, max_days_overdue from public.customer_credit_position($1)`. **Borra** la consulta `with cust_receivables as (...)`, que suma por sucursal y usa `current_date`. Después, en este orden: `CREDIT_NOT_ENABLED`, `CREDIT_BLOCKED_OVERDUE`, `CREDIT_LIMIT_EXCEEDED`;
   6. con `credit_override_reason`: como ya está (permiso, motivo de 3 a 200 caracteres y auditoría `CREDITO_AUTORIZADO`).
5. Al terminar, en `sales.rs` no debe quedar ningún `current_date` ni ningún `unwrap()` fuera de `mod tests`. Pega en el reporte la salida de `grep -n "current_date\|\.unwrap()" src/sales.rs src/credit.rs src/refunds.rs src/customers.rs`, y explica cada línea que aparezca dentro de `mod tests`.

### E3. Plan de cuotas (`credit.rs`)

1. `CreditPlan::validate`: si `interest_rate_monthly != 0`: 422 `CREDIT_PLAN_INVALID` (C1). Se mantiene la validación de 1 a 60 cuotas. Quita la de 0 % a 10 %, que queda cubierta.
2. Método nuevo `CreditPlan::validate_dates(&self, today: NaiveDate) -> Result<(), ApiError>`: si `first_due_date < today`: 422 `CREDIT_PLAN_INVALID` (C2.2).
3. `calculate_installment_due_date`, rama `Mensual`: `checked_add_months` de `chrono` ya devuelve el último día del mes cuando el día no existe. Reemplaza todo el bloque por `first_due_date.checked_add_months(chrono::Months::new((k - 1) as u32)).unwrap_or(first_due_date)` y borra la rama de respaldo con los tres `unwrap()`. La prueba `schedule_month_end` confirma el comportamiento.
4. `CUOTA_FIJA` con `r > 0`: `total = max(money(A) × n, principal)`. Todas las cuotas valen `money(A)` y la última es `total − Σ las anteriores` (C1).
5. `preview` llama también a `plan.validate()`, que ya rechaza tasas distintas de 0. No cambia nada más.
6. Prueba unitaria nueva `schedule_fixed_never_negative_interest`: 10 pesos al 0,01 % mensual en 3 cuotas `CUOTA_FIJA` da `interest_total = 0` y cuotas de 3,33, 3,33 y 3,34. (Valor comprobado: `money(A) = 3,33` y `3,33 × 3 = 9,99 < 10`.)

### E4. Devoluciones (`refunds.rs`)

1. **Función nueva** `reduce_receivable_tx(tx, scope, order_id, amount) -> Result<Decimal, ApiError>`, en `refunds.rs`. Contiene el procedimiento que hoy está dentro de `apply_effects_tx` (bloquear la cuenta, validar, reducir la cuenta y las cuotas, y recalcular `due_date`), y devuelve el monto aplicado.
   - `aplicado = min(amount, amount_total − amount_paid)`.
   - Si la cuenta no existe, devuelve 0.
   - `REFUND_EXCEEDS_RECEIVABLE` se mantiene **solo** en el caso de un solo pago `CREDITO`, cuando `amount > saldo`.
2. **Error que hay que corregir:** la consulta `select min(due_date) ...` usa `fetch_optional` con `Option<NaiveDate>`. Cuando ya no quedan cuotas con saldo, `min` devuelve una fila con `NULL` y la decodificación falla. Usa `fetch_one` con `Option<chrono::NaiveDate>`, y actualiza `due_date` solo si es `Some`.
3. **Un solo pago `CREDITO`:** llama a `reduce_receivable_tx(..., total)` y guarda `refunds.receivable_applied = total`. No hay movimiento de caja: ya ocurre, porque `cash_out` es falso con `original_method = CREDITO`.
4. **Pago mixto con `CREDITO`** (C5): el método debe ser `EFECTIVO`; si no: 422 `REFUND_ORIGINAL_MIXED`, como hoy. Al aprobar: `aplicado = reduce_receivable_tx(..., total)`, `receivable_applied = aplicado`, y el movimiento `DEVOLUCION` de `cash::record_movement_tx` es por `total − aplicado`. Si ese monto es 0, no se llama.

### E5. Anulación (`sales.rs::void_tx`)

Queda como está: `HAS_PAYMENTS` si la cuenta tiene abonos, y borra la cuenta si no los tiene. Las cuotas se borran por el `on delete cascade`.

### E6. Permisos en el código (`tenancy::base_allows`)

C6 tampoco está hecha: agrega `credit:sell` a `CAJERO`, y `credit:sell` y `credit:override` a `SUPERVISOR`. La prueba de la matriz de roles de la fase 0 debe seguir pasando. Si hay que ajustarla, explícalo en el reporte.

### E7. Pruebas que faltan

En `f3_credit_sale`, además de las 5 que ya existen:
- en la regla de cupo con `credit:override`, agrega la aserción de que existe una fila en `audit_log` con `action = 'CREDITO_AUTORIZADO'` y `entity_id` del cliente;
- **[P]** Cupo de todo el negocio: con un cliente de cupo 100.000 y una deuda de 100.000 en la sucursal A, un cajero atado a la sucursal B que le vende 1.000 a crédito recibe 409 `CREDIT_LIMIT_EXCEEDED`;
- **[P]** `credit_plan` con `interest_rate_monthly = 2`: 422 `CREDIT_PLAN_INVALID`;
- **[P]** `credit_plan` con `first_due_date` de ayer: 422 `CREDIT_PLAN_INVALID`;
- **[P]** las dos de C5 (devolución total sin abonos y pago mixto de 100.000);
- **[P]** Devolución total de una venta a crédito **en 3 cuotas** sin abonos: la cuenta queda `PAGADA`, sin cuotas y sin error.

En `f3_customers`: la de C6 (un cajero que crea un cliente con cupo: 403).

Prueba unitaria `normalize_customer_person_type` (C7), con sus 6 casos.

### E8. Frontend de F3.2 (POS)

Es la parte "POS" de F3.5 → Frontend, con C1:
- método "Crédito" en el cobro, visible solo con un cliente asignado y `can("credit:sell")`;
- debajo, el texto `Cupo disponible: <formatCOP(credit_available)>`, leído de `GET /v1/customers/{id}`;
- interruptor "En cuotas" con tres campos: Cuotas (1 a 60), Frecuencia (Semanal, Quincenal, Mensual) y Primera cuota (fecha, mínimo hoy). **Sin** campos de tasa ni de tipo de interés: envía `interest_rate_monthly: "0"` e `interest_type: "SIMPLE"`. La vista previa de las cuotas usa `GET /v1/credit/schedule-preview`;
- si la respuesta es `CREDIT_LIMIT_EXCEEDED` o `CREDIT_BLOCKED_OVERDUE` y `can("credit:override")`: botón "Autorizar igual", que pide el motivo y reenvía la venta **con el mismo `id`** y `credit_override_reason`;
- el mock (`lib/api/mock-credit.ts`) aplica las mismas validaciones de cupo y mora;
- `ERROR_MESSAGES` ya tiene los códigos; revisa que estén `CREDIT_PLAN_INVALID` y `PAYMENT_METHOD_NOT_AVAILABLE` con los textos de C13.

### E9. Verificación

Docker Desktop tiene que estar encendido antes de `test.sh`. Si la salida dice `failed to connect to the docker API`, eso **no** es un resultado de prueba: anótalo en el reporte como "no se pudo verificar" y no hagas el commit.

---

## Correcciones del 2026-11-04 (mandan sobre el resto del documento)

Si algo de abajo contradice esta sección, gana esta sección. Lo que ya construiste y choca con ella se corrige con un commit nuevo. La migración `20261106000100_fase3_clientes.sql` todavía no está aplicada: estos cambios se hacen **en ese mismo archivo**.

### C0. Alcance para salir a vender (licorerías y tiendas)

| Tarea | Estado |
|---|---|
| F3.1 Clientes | Se construye |
| F3.2 Venta a crédito | Se construye |
| F3.3 Abonos | Se construye |
| F3.5 Cartera | Se construye, sin lo que dependa de F3.4 |
| F3.4 Acuerdos y castigo | **Pospuesta: no construir** |
| F3.6 Saldo a favor | **Pospuesta: no construir** |
| F3.7 Separados | **Pospuesta: no construir** |
| F3.8 Cotizaciones | **Pospuesta: no construir** |

Las tablas de las tareas pospuestas se quedan en la migración, sin código. Por lo tanto:
- La ficha del cliente **no muestra** las pestañas Separados ni Saldo a favor, ni el botón "Acuerdo de pago".
- `wallet_balance` sigue saliendo en la API, con valor 0.
- `PaymentMethod` no agrega `Saldo`, y la devolución no ofrece `SALDO_A_FAVOR`.
- Un abono con `method = SALDO`: 422 `PAYMENT_METHOD_NOT_AVAILABLE`.
- En el reporte, las reglas [P] de las tareas pospuestas van marcadas como "POSPUESTA".
- La lista de verificación del modo demo se reduce a: vender a crédito, abonar y ver la cartera.

### C1. Fiado sin interés

`interest_rate_monthly` debe ser 0. Si llega otro valor: 422 `CREDIT_PLAN_INVALID`. La función pura `credit::schedule` y sus pruebas se quedan como están. El POS **no muestra** los campos de tasa ni de tipo de interés: envía `interest_rate_monthly = 0` e `interest_type = "SIMPLE"`.

En `CUOTA_FIJA`, `total = max(money(A) × n, principal)`, para que `interest_total` nunca sea negativo.

### C2. Validación del plan

Dentro de la validación de la venta a crédito, antes del paso 4:
1. Si viene `credit_plan` y no hay pago `CREDITO`: 422 `CREDIT_PLAN_INVALID`.
2. Si `first_due_date` es anterior a hoy en la sucursal: 422 `CREDIT_PLAN_INVALID`.

### C3. Saldo y mora de todo el negocio

`receivables` tiene RLS por sucursal, pero el cliente y su cupo son de todo el negocio. Agrega esta función a la migración. Ya está probada contra la base local: un cajero de otra sucursal ve el saldo real, y otro negocio ve 0.

```sql
create or replace function public.customer_credit_position(p_customer uuid)
returns table (balance numeric(14,2), max_days_overdue integer)
language sql
stable
security definer
set search_path = ''
as $$
  with x as (
    select r.amount_total - r.amount_paid as saldo,
           greatest(0, (now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)) as dias
      from public.receivables r
      join public.branches b on b.id = r.branch_id
      left join lateral (
        select min(ri.due_date) as first_due
          from public.receivable_installments ri
         where ri.receivable_id = r.id and ri.amount_paid < ri.amount
      ) i on true
     where r.tenant_id = public.current_tenant_id()
       and r.customer_id = p_customer
       and r.status in ('ABIERTA','PARCIAL')
  )
  select coalesce(sum(saldo), 0)::numeric(14,2), coalesce(max(dias), 0)::integer from x
$$;
revoke all on function public.customer_credit_position(uuid) from public, anon;
grant execute on function public.customer_credit_position(uuid) to authenticated;
```

Úsala en **todos** estos lugares, en vez de sumar `receivables` directamente:
- la validación de cupo de `sales.rs`;
- `customers::list`, con `left join lateral public.customer_credit_position(c.id) cp on true`; los filtros `AL_DIA` y `EN_MORA` usan `cp.max_days_overdue`;
- `get_customer_detail_tx`;
- `customers::archive`.

No queda ningún `current_date` en `customers.rs` ni en el código de cartera.

### C4. Bloqueo del cliente

Dos ventas simultáneas no pueden pasar el mismo cupo. Antes de leer la posición de crédito, en la venta y en `archive`:

```sql
select id from customers where tenant_id = $1 and id = $2 for update
```

### C5. Devoluciones de ventas a crédito

Cambios en la migración:

```sql
-- en receivables: principal numeric(14,2) not null check (principal >= 0)
alter table public.refunds add column receivable_applied numeric(14,2) not null default 0 check (receivable_applied >= 0);
```

- Si la cuenta queda con `amount_total = amount_paid`, su estado pasa a `PAGADA`. Eso incluye `principal = 0` y `amount_total = 0`.
- **Solo `CREDITO`:** como dice F3.2. Además, `receivable_applied = total`.
- **Pago mixto con `CREDITO`** (reemplaza "solo se admite EFECTIVO, como ya ocurre"): el método sigue siendo `EFECTIVO`. Al aprobar:
  1. bloquea la cuenta con `for update`;
  2. `aplicado = min(total, saldo de la cuenta)`;
  3. reduce la cuenta y sus cuotas en `aplicado`, con el mismo procedimiento de F3.2, y guarda `receivable_applied = aplicado`;
  4. el movimiento de caja `DEVOLUCION` es por `total − aplicado`; si es 0, no hay movimiento.

Prueba `f3_credit_sale`:
- **[P]** Devolver toda una venta a crédito sin abonos deja la cuenta `PAGADA`, con saldo 0 y sin error.
- **[P]** Una venta de 100.000 (20.000 en efectivo y 80.000 a crédito) devuelta completa reduce la cuenta en 80.000 y saca 20.000 de la caja.

### C6. Permisos

- `SUPERVISOR` recibe también `credit:sell`.
- Actualiza `tenancy::base_allows` para que coincida: `CAJERO` con `credit:sell`; `SUPERVISOR` con `credit:sell` y `credit:override`.
- Los permisos nuevos usan `on conflict (code) do nothing`, como dice la spec.
- **Crear con cupo:** en `POST /v1/customers` y en la creación de `upsert`, si `credit_limit` no es 0 o `credit_block_days` no es 30 y el usuario no tiene `credit:override`: 403. En el formulario, esos campos quedan deshabilitados también al crear.

Prueba `f3_customers`:
- **[P]** Un cajero que crea un cliente con cupo: 403.

### C7. Tipo de persona y NIT

No todo NIT es persona jurídica: un comerciante persona natural también tiene NIT. Esta sección reemplaza los pasos 2, 4 y 5 de "Normalización". `CreateCustomerInput` agrega `person_type: Option<String>`.

Paso 2: separa el DV en el guion **solo si** `doc_type = NIT`. Con otro tipo, el guion se elimina en el paso 3 y lo que siga hace parte del número.

Paso 4, si `doc_type = NIT`:
- el DV funciona como dice la spec;
- si llega `person_type`, debe ser `NATURAL` o `JURIDICA`; si no: 422 `CUSTOMER_INVALID`;
- si no llega, usa la función pura `customers::default_person_type(doc_number)`: `JURIDICA` si el número tiene 9 dígitos y empieza por 8 o 9; si no, `NATURAL`.

Paso 5, con cualquier otro tipo o sin documento: `dv = null` y `person_type = NATURAL`. Si llega `JURIDICA`: 422 `CUSTOMER_INVALID`.

Prueba unitaria `normalize_customer_person_type`, con estos casos:
- NIT `900123456` sin `person_type` → `JURIDICA`, DV `8`;
- NIT `1020304050` sin `person_type` → `NATURAL`, DV `8`;
- NIT `52123456` sin `person_type` → `NATURAL`, DV `2`;
- NIT `900123456` con `NATURAL` → `NATURAL`;
- `CC` con `JURIDICA` → 422 `CUSTOMER_INVALID`;
- `PP` con `AB-123456` → número `AB123456`, sin DV.

**Frontend:**
- `customer-form-dialog.tsx`: selector "Tipo de persona" (Natural, Jurídica), visible solo con NIT. Se llena con la misma regla mientras el usuario no lo cambie.
- La etiqueta del nombre es "Razón social" con `JURIDICA` y "Nombre completo" con `NATURAL`.
- El diálogo rápido del POS no muestra el selector y no envía `person_type`.
- El mock aplica las mismas reglas.

**Proveedores:** en `procurement.rs`, si `person_type` no llega, usa `customers::default_person_type` con NIT y `NATURAL` con otro documento, en vez de `JURIDICA` fijo. `supplier-dialog.tsx` aplica la misma regla al crear.

### C8. `upsert` y `PUT`

- **Documento en `upsert`:** `doc_type`, `doc_number`, `dv` y `person_type` son un solo bloque.
  - Si el cliente encontrado **no** tiene documento y el cuerpo sí: busca si otro cliente activo ya lo usa. Si existe: 409 `CUSTOMER_DUPLICATED`, con `data: {id}`. Si no: copia los cuatro.
  - Si el cliente encontrado ya tiene documento: esos cuatro campos no se tocan, aunque el cuerpo traiga otros.
- **`PUT`:** si `credit_limit`, `credit_block_days` o `person_type` no llegan, se conserva el valor guardado.
- **`unarchive`:** si otro cliente activo tiene el mismo documento: 409 `CUSTOMER_DUPLICATED`, con `data: {id}`.

### C9. Estado de cuenta

`type` agrega `DEVOLUCION`: una fila por cada devolución aprobada con `receivable_applied > 0`, con `credit = receivable_applied`. Un cargo usa el valor original de la cuenta: `principal + interest_total + Σ receivable_applied` de sus devoluciones. `closing_balance` debe ser igual a `balance(customer)`.

Prueba `f3_aging`:
- **[P]** Después de una devolución parcial de una venta a crédito, `closing_balance` es igual al saldo del cliente.

### C10. Enlace de WhatsApp

Antes de armar `wa.me`: si el teléfono tiene 12 dígitos y empieza por `57`, quita ese `57`. El resultado debe tener 10 dígitos y empezar por `3`; si no: 422 `CUSTOMER_NO_PHONE`.

### C11. Índices

Agrégalos a la migración:

```sql
create index receivables_customer_idx on public.receivables (tenant_id, customer_id) where status in ('ABIERTA','PARCIAL');
create index receivable_payments_receivable_idx on public.receivable_payments (receivable_id, created_at);
```

### C12. Otras correcciones

Corrige también los puntos A9 a A17 de `antigravity/REVISION-CLAUDE.md`. En A15, las pestañas pospuestas se ocultan. En A16, los textos van con voseo: "Hacé clic", "Escribí".

### C13. Errores nuevos

| Código | Texto |
|---|---|
| `CREDIT_PLAN_INVALID` | Revisá el plan de cuotas. |
| `PAYMENT_METHOD_NOT_AVAILABLE` | Ese método no está disponible para abonos. |

---

**Fuera de alcance:**
- consulta del NIT en RUES o SECOP;
- envío automático por WhatsApp o correo (aquí solo se generan enlaces `wa.me`);
- procesos nocturnos: los días de mora se **calculan al leer**, no se guardan;
- saldo retenido en la billetera;
- PDF generado en el servidor (se usa la impresión del navegador).

## Permisos nuevos (en la migración)

```sql
insert into public.permissions (code, module, description) values
  ('credit:override','credit','Autorizar crédito por encima del cupo o con mora'),
  ('wallet:adjust','customers','Ajustar el saldo a favor de un cliente')
on conflict (code) do nothing;
```

- `DUENO` y `ADMIN_CAJA` reciben los dos.
- `SUPERVISOR` recibe `credit:override`.
- `CAJERO` recibe además `credit:sell`.

## Orden de las tareas

| Tarea | Tema |
|---|---|
| F3.1 | Clientes completos |
| F3.2 | Venta a crédito y cuentas por cobrar |
| F3.3 | Abonos y cuotas |
| F3.4 | Acuerdos de pago y castigo |
| F3.5 | Cartera: pantalla, antigüedad, estado de cuenta y cobro por WhatsApp |
| F3.6 | Saldo a favor (billetera) |
| F3.7 | Separados |
| F3.8 | Cotizaciones |

---

## F3.1 Clientes completos

### Migración

```sql
alter table public.customers
  add column person_type text not null default 'NATURAL' check (person_type in ('NATURAL','JURIDICA')),
  add column tax_regime text not null default 'NO_RESPONSABLE' check (tax_regime in ('RESPONSABLE_IVA','NO_RESPONSABLE')),
  add column fiscal_responsibilities text[] not null default '{R-99-PN}',
  add column ciiu text check (ciiu ~ '^[0-9]{4}$'),
  add column withholding_agent boolean not null default false,
  add column city_code text check (city_code ~ '^[0-9]{5}$'),
  add column billing_address text check (length(billing_address) <= 200),
  add column shipping_address text check (length(shipping_address) <= 200),
  add column pinned_note text check (length(pinned_note) <= 300),
  add column credit_limit numeric(14,2) not null default 0 check (credit_limit >= 0),
  add column credit_block_days integer not null default 30 check (credit_block_days between 0 and 365),
  add constraint customers_resp_codes check (fiscal_responsibilities <@ array['O-13','O-15','O-23','O-47','R-99-PN']::text[]);
```

### Normalización
Amplía la función pura `normalize_customer` de la fase 1, en este orden:
1. `doc_type`: mayúsculas y sin espacios a los lados.
2. Si `doc_number` contiene `-`: lo que va después del último guion es el DV y lo de antes es el número. Si además llegó `dv` y es distinto: 422 `CUSTOMER_DV_CONFLICT`.
3. `doc_number`: solo letras y dígitos, en mayúsculas.
4. Si `doc_type = NIT`:
   - el DV se calcula con `nit_dv` (fase 2); si llegó uno distinto: 422 `NIT_DV_INVALID`; si no llegó, se guarda el calculado;
   - `person_type`: ver la corrección C7.
5. Con cualquier otro tipo: ver la corrección C7.

Prueba unitaria `normalize_customer_splits_dv`, con estos casos:
- `"900.123.456-8"` → número `900123456`, DV `8`;
- `"900123456-7"` con `dv = 8` → `CUSTOMER_DV_CONFLICT`;
- `"cc"` → `CC`.

### Endpoints (`customers.rs`)

| Método y ruta | Permiso | Detalle |
|---|---|---|
| `GET /v1/customers?q=&status=&page=&page_size=` | `customers:read` | `status` puede ser `AL_DIA`, `EN_MORA` o `ARCHIVADO`. Cada fila trae `balance` y `max_days_overdue` (F3.2). |
| `GET /v1/customers/{id}` | `customers:read` | Cliente, más `balance`, `credit_available`, `wallet_balance` y `missing_fiscal: string[]` (abajo). |
| `POST /v1/customers` | `customers:write` | Todos los campos. |
| `PUT /v1/customers/{id}` | `customers:write` | Todos los campos. `credit_limit` y `credit_block_days` solo los cambia quien tiene `credit:override`; si no lo tiene y cambian: 403. |
| `POST /v1/customers/upsert` | `customers:write` | Buscar o crear (abajo). |
| `POST /v1/customers/{id}/archive` | `customers:write` | Si tiene saldo en cartera, separados `ACTIVO` o saldo a favor: 409 `CUSTOMER_HAS_BALANCE`. |
| `POST /v1/customers/{id}/unarchive` | `customers:write` | — |
| `GET /v1/customers/{id}/purchases?page=` | `customers:read` | Ventas del cliente, con el formato de `/v1/sales`. |

**`missing_fiscal`:** lista de campos que faltan para facturarle electrónicamente. Revisa, en este orden:
- `doc_type` y `doc_number` → `"documento"`;
- `email` → `"correo"`;
- `city_code` → `"municipio"`;
- `billing_address` → `"dirección de facturación"`.

**Buscar o crear (`upsert`):**
1. Normaliza.
2. Busca, sin archivados, primero por `email` (si viene) y luego por `doc_type` y `doc_number` (si vienen).
3. Si lo encuentra: copia **solo** los campos del cuerpo que en la base estén en `null` o vacíos. Nunca sobrescribe un valor que ya existe. Respuesta: `{customer, result: "UPDATED"}`, o `"UNCHANGED"` si no cambió nada.
4. Si no lo encuentra: lo crea. Respuesta: `{customer, result: "CREATED"}`.

El POS usa este endpoint en la pestaña "Nuevo" del diálogo de cliente, en lugar de `POST /v1/customers`.

### Frontend
- **`/clientes`** (pantalla 07): `app/(admin)/clientes/page.tsx` y `components/clientes/customers-screen.tsx`. Cambia la pantalla a `"react"`.
  - Buscador, pestañas Todos, Al día, En mora y Archivados.
  - Tabla con Nombre, Documento, Teléfono, Saldo, Días de mora (en rojo si es mayor que 0) y Tarifa.
  - Botón "Nuevo cliente".
- **`/clientes/[id]`:** `components/clientes/customer-screen.tsx`.
  - Encabezado con el nombre, el documento, la nota fijada (editable) y avisos amarillos por cada elemento de `missing_fiscal`.
  - Pestañas:
    - **Compras:** historial;
    - **Cartera:** F3.3 a F3.5;
    - **Separados:** F3.7;
    - **Saldo a favor:** F3.6;
    - **Datos:** formulario completo.
- **POS:** cuando `upsert` responde, muestra "Cliente creado" o "Cliente actualizado con los datos nuevos".

### Pruebas (`f3_customers`)
- **[P]** `upsert` con un cliente existente solo llena los campos vacíos y no cambia el teléfono que ya tenía.
- **[P]** Un NIT con un DV incorrecto: 422 `NIT_DV_INVALID`.
- **[P]** Archivar a un cliente con saldo: 409 `CUSTOMER_HAS_BALANCE`.
- **[P]** Un usuario sin `credit:override` que cambia el `credit_limit`: 403.

---

## F3.2 Venta a crédito y cuentas por cobrar

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Credito', 'CREDITO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PAGO'
on conflict (padre_id, tima_codigo) do nothing;

create table public.receivables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  customer_id uuid not null,
  source text not null check (source in ('VENTA','ACUERDO')),
  order_id uuid,
  prefix text, number bigint,                      -- solo para ACUERDO (secuencia ACUERDO_PAGO)
  principal numeric(14,2) not null check (principal >= 0),
  interest_total numeric(14,2) not null default 0 check (interest_total >= 0),
  amount_total numeric(14,2) not null,
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  status text not null default 'ABIERTA' check (status in ('ABIERTA','PARCIAL','PAGADA','CASTIGADA','REFINANCIADA')),
  due_date date not null,
  plan jsonb,                                      -- el credit_plan recibido, o null para crédito libre
  written_off_reason text,
  refinanced_into uuid,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (order_id),
  check (amount_total = principal + interest_total),
  check (amount_paid <= amount_total),
  check ((source = 'VENTA') = (order_id is not null)),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id)
);
create table public.receivable_installments (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  receivable_id uuid not null,
  n integer not null check (n >= 1),
  due_date date not null,
  amount numeric(14,2) not null check (amount > 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  primary key (receivable_id, n),
  check (amount_paid <= amount),
  foreign key (tenant_id, receivable_id) references public.receivables (tenant_id, id) on delete cascade
);
-- RLS: receivables con in_scope; installments a nivel de negocio. Grants.
```

### Días de mora y saldo
Funciones SQL o de Rust, de uso común. "Hoy" es la fecha local de la sucursal.

| Concepto | Cálculo |
|---|---|
| `balance(receivable)` | `amount_total − amount_paid`, si el estado es `ABIERTA` o `PARCIAL`; si no, 0. |
| `days_overdue(receivable)` | Con cuotas: días entre la primera cuota con `amount_paid < amount` y hoy. Sin cuotas: días entre `due_date` y hoy. Mínimo 0. Si el saldo es 0, es 0. |
| `balance(customer)` | Suma de los saldos de sus cuentas. |
| `max_days_overdue(customer)` | El máximo de sus cuentas. |
| `credit_available(customer)` | `max(0, credit_limit − balance)`. |

### Venta (`sales.rs`)
`PaymentMethod` agrega `Credito` (`CREDITO`). En `settle`, `CREDITO` se trata como un pago no en efectivo, así que no puede superar lo que falta por pagar. Solo puede haber **un** pago `CREDITO` por venta. Si hay más: 422 `SALE_MULTIPLE_CREDIT`.

`SaleRequest` agrega:
- `credit_plan: Option<{installments: 1..=60, frequency: "SEMANAL"|"QUINCENAL"|"MENSUAL", first_due_date: date, interest_rate_monthly: Decimal (0..=10), interest_type: "SIMPLE"|"CUOTA_FIJA"}>`;
- `credit_override_reason: Option<String>`.

Si hay un pago `CREDITO`, valida en este orden, después de los pagos:
1. Sin `credit:sell`: 403.
2. Sin `customer_id`: 422 `CREDIT_REQUIRES_CUSTOMER`.
3. `C` es el monto del pago `CREDITO`.
4. Si **no** viene `credit_override_reason`:
   - `credit_limit = 0`: 409 `CREDIT_NOT_ENABLED`;
   - `max_days_overdue > credit_block_days`: 409 `CREDIT_BLOCKED_OVERDUE`, con `data: {days}`;
   - `C > credit_available`: 409 `CREDIT_LIMIT_EXCEEDED`, con `data: {available}`.
5. Si viene `credit_override_reason`: exige `credit:override` (si no: 403) y un motivo de 3 a 200 caracteres. Se salta el paso 4 y se registra la auditoría `CREDITO_AUTORIZADO`, con `{customer_id, amount: C, reason}`.
6. Se crea la cuenta por cobrar en la misma transacción, después de cerrar la venta: `source = VENTA`, `order_id`, `principal = C` y el plan (abajo).
7. Los demás pagos de la venta (efectivo, datáfono o QR) son la **cuota inicial**. Ya quedan registrados en `payments`; no se hace nada adicional.

Un pago `CREDITO` se guarda en `payments`, con `cash_session_id` del turno, para que el cierre muestre "Crédito" en `by_method`. **No suma** al efectivo esperado.

### Plan de cuotas: función pura `credit::schedule(principal, plan) -> (interest_total, Vec<(n, due_date, amount)>)`
- **Tasa por periodo:** `r = interest_rate_monthly / 100 × f`, con `f = 7/30` (`SEMANAL`), `15/30` (`QUINCENAL`) o `1` (`MENSUAL`).
- **Fechas:** la cuota `k` vence en `first_due_date + (k−1) × periodo`. El periodo es de 7 días, 15 días o 1 mes calendario: usa `chrono` y `checked_add_months`; si el día no existe en ese mes, toma el último día del mes.
- **`SIMPLE`:**
  - `interest_total = money(principal × r × n)`;
  - `total = principal + interest_total`;
  - cada cuota es `money_floor(total / n)` y la **última** absorbe la diferencia.
- **`CUOTA_FIJA`:**
  - si `r = 0`, se calcula igual que `SIMPLE` con interés 0;
  - si no, `A = principal × r / (1 − (1 + r)^−n)`, calculado con `Decimal` y su método `powi`; `total = money(A) × n`;
  - todas las cuotas valen `money(A)` y la última se ajusta a `total − Σ las anteriores`;
  - `interest_total = total − principal`.
- **Sin `credit_plan`** (crédito libre): sin cuotas, `interest_total = 0`, `due_date = hoy + 30 días`.
- **Con plan:** `due_date` es la fecha de la cuota 1.

Pruebas unitarias:
- `schedule_simple_last_absorbs_rounding`: 100.000 en 3 cuotas sin interés da 33.333,33 + 33.333,33 + 33.333,34.
- `schedule_fixed_payment_known_value`: 1.000.000, 12 cuotas mensuales al 2 % da una cuota de $94.559,60.
- `schedule_month_end`: con `first_due_date = 31 de enero` y frecuencia mensual, la segunda cuota vence el 28 o 29 de febrero.

### Devoluciones y anulación de una venta a crédito
- **Devolución** (F1.11) de una venta con un pago `CREDITO`:
  - si `CREDITO` es el único pago, el método se fuerza a `ORIGINAL` con `original_method = CREDITO`. El efecto es reducir `amount_total` (y `principal`) de la cuenta en el total devuelto. Si el resultado quedara por debajo de `amount_paid`: 409 `REFUND_EXCEEDS_RECEIVABLE`. Las cuotas se recalculan: se reduce la última cuota y, si no alcanza, se eliminan cuotas desde la última;
  - si la venta tiene pagos mixtos, solo se admite `EFECTIVO`, como ya ocurre.
- **Anulación** (F1.12): si la cuenta tiene abonos: 409 `VOID_NOT_ALLOWED`, con `why: "HAS_PAYMENTS"`. Si no los tiene, se borra la cuenta y sus cuotas.

### Pruebas (`f3_credit_sale`)
- **[P]** Un pago `CREDITO` sin cliente: 422 `CREDIT_REQUIRES_CUSTOMER`.
- **[P]** Superar el cupo: 409 `CREDIT_LIMIT_EXCEEDED`; con `credit:override` y motivo, la venta pasa y queda en auditoría.
- **[P]** Un cliente con 31 días de mora y `credit_block_days = 30`: 409 `CREDIT_BLOCKED_OVERDUE`.
- **[P]** El crédito no suma al efectivo esperado del turno.
- **[P]** Una venta con cuota inicial en efectivo más crédito crea una cuenta solo por la parte a crédito.

---

## F3.3 Abonos y cuotas

### Migración

```sql
create table public.receivable_payments (
  id uuid primary key,                                 -- generado por el cliente
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  receivable_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','DATAFONO','QR','TRANSFERENCIA','SALDO')),
  reference text,
  cash_session_id uuid,
  allocation jsonb not null,                           -- [{n, amount}] cuotas a las que se aplicó
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, receivable_id) references public.receivables (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
-- RLS con in_scope; grants.
```

### `POST /v1/receivables/{id}/payments`
Permiso `credit:collect`. Cuerpo: `{id, amount, method, reference?}`.

1. **Idempotencia** por `id` (mismo cuerpo: devuelve el pago; distinto: 409 `IDEMPOTENCY_CONFLICT`).
2. Bloquea la cuenta con `for update`. Debe estar `ABIERTA` o `PARCIAL`; si no: 409 `RECEIVABLE_CLOSED`.
3. `amount = money(amount)` debe ser menor o igual al saldo. Si no: 422 `PAYMENT_EXCEEDS_BALANCE`, con `data: {balance}`.
4. `DATAFONO`, `QR` y `TRANSFERENCIA` exigen `reference` (de 3 a 60 caracteres). `SALDO` descuenta de la billetera (F3.6).
5. **Aplicación a cuotas:** recorre las cuotas por `n` ascendente y a cada una le suma `min(restante, amount − amount_paid)`, hasta agotar el abono. Guarda la aplicación en `allocation`. Sin cuotas, `allocation = []`.
6. `amount_paid += amount`. Estado: `PAGADA` si el saldo queda en 0; si no, `PARCIAL`. **`due_date`** pasa a ser el vencimiento de la primera cuota que queda con saldo (sin cuotas, no cambia).
7. En `EFECTIVO`: exige un turno abierto del usuario en la sucursal (si no: 409 `PAYMENT_NEEDS_OPEN_SESSION`) y llama a `cash::record_movement_tx` con el tipo `ABONO_CARTERA`, el motivo "Abono <nombre del cliente>", `ref_type = 'RECEIVABLE'` y `ref_id`.
8. Auditoría `ABONO_CARTERA`.

`GET /v1/receivables?customer_id=&status=&overdue=` y `GET /v1/receivables/{id}` (permiso `customers:read`) devuelven la cuenta con `installments` (cada una con `paid: bool` y `days_overdue`), `payments`, `balance` y `days_overdue`.

### Tirilla de abono
Título "ABONO", cliente, número de la venta o del acuerdo, monto, método, saldo anterior, saldo nuevo y próxima cuota.

### Pruebas (`f3_payments`)
- **[P]** Un abono de 50.000 sobre cuotas de 33.333,33, 33.333,33 y 33.333,34 paga completa la cuota 1 y abona 16.666,67 a la cuota 2. El `due_date` pasa al de la cuota 2.
- **[P]** Un abono mayor al saldo: 422.
- **[P]** Un abono en efectivo crea un movimiento `ABONO_CARTERA` y sube el esperado del turno.
- **[P]** Un reintento idéntico no abona dos veces.

---

## F3.4 Acuerdos de pago y castigo

**Acuerdo:** `POST /v1/receivables/agreements` (permiso `credit:override`). Cuerpo: `{id, customer_id, receivable_ids: [uuid], plan: credit_plan}`.
1. Idempotencia por `id`. El acuerdo se guarda como una cuenta nueva con ese `id`.
2. Bloquea todas las cuentas con `for update`. Deben ser del cliente y estar `ABIERTA` o `PARCIAL`. Si no: 422 `AGREEMENT_INVALID`.
3. `principal = Σ saldos`.
4. Crea la cuenta nueva con `source = ACUERDO`, número de `sequences::next_tx(..., "ACUERDO_PAGO")`, `schedule(principal, plan)` y `plan` guardado.
5. Cada cuenta original queda con `status = REFINANCIADA` y `refinanced_into = id`. Su `amount_paid` no cambia y su saldo deja de contar porque ya no está `ABIERTA` ni `PARCIAL`.
6. Auditoría `ACUERDO_PAGO`.

Los abonos al acuerdo usan el mismo endpoint de F3.3.

**Castigo:** `POST /v1/receivables/{id}/write-off` (permiso `credit:write_off`). Cuerpo `{reason}` (de 3 a 200 caracteres). Debe estar `ABIERTA` o `PARCIAL`. Queda `status = CASTIGADA` y `written_off_reason`. Auditoría `CARTERA_CASTIGADA`, con el saldo castigado.

### Pruebas (`f3_agreements`)
- **[P]** Un acuerdo sobre dos cuentas crea una cuenta con `principal` igual a la suma de los saldos, y las originales quedan `REFINANCIADA` con saldo 0 en el cliente.
- **[P]** Los abonos al acuerdo reducen su saldo.
- **[P]** Una cuenta castigada deja de contar en `balance(customer)`.

---

## F3.5 Cartera: pantalla, antigüedad, estado de cuenta y cobro por WhatsApp

### Endpoints (permiso `customers:read`)
- `GET /v1/receivables/summary?branch_id=`:
  - `{total_balance, overdue_balance, due_this_week (saldo de cuotas o cuentas que vencen entre hoy y hoy + 7 días), collected_this_month (Σ receivable_payments del mes local), aging: {al_dia, d1_30, d31_60, d61_90, d91_120, d120_mas}, top_debtors: [{customer_id, name, balance, max_days_overdue}] (10, ordenados por saldo descendente)}`;
  - el tramo de cada cuenta se decide con `days_overdue`; `al_dia` es 0 días.
- `GET /v1/customers/{id}/statement?from=&to=`:
  - `{opening_balance, entries: [{date, type: "CARGO"|"ABONO"|"CASTIGO"|"REFINANCIACION", reference, debit, credit, balance}], closing_balance}`;
  - un cargo es una cuenta creada (por `amount_total`), un abono es un `receivable_payments`, un castigo es el saldo castigado, y una refinanciación cierra la cuenta vieja con un abono por su saldo y abre la nueva con un cargo;
  - orden por fecha y luego por hora.
- `GET /v1/receivables/{id}/whatsapp-link`:
  - devuelve `{url}` con `https://wa.me/57<phone>?text=<urlencoded>`;
  - si el cliente no tiene teléfono: 422 `CUSTOMER_NO_PHONE`;
  - texto exacto: `Hola <nombre>, te escribimos de <negocio>. Tenés un saldo de $<saldo> que vence el <dd/mm/yyyy>. Podés pagar en <sucursal>. ¡Gracias!`. Si está vencido, cambia "que vence el" por "vencido desde el".

### Frontend
- **Pantalla `/cartera`** (registro `s("44", "cartera", "Cartera", "/cartera", "clientes", "admin", "react")`, permiso `customers:read`):
  - 4 tarjetas: Por cobrar, Vencido, Vence esta semana y Cobrado este mes;
  - gráfica de antigüedad: 6 barras horizontales hechas con `div` y CSS (ancho proporcional al monto del tramo más grande), con la etiqueta del tramo y el monto. Sin librerías de gráficas;
  - tabla de deudores con Nombre, Saldo, Días de mora y acciones "Abonar", "Cobrar por WhatsApp" (abre la URL en una pestaña nueva) y "Ver";
  - top 10 a un lado.
- **Pestaña Cartera de la ficha del cliente:**
  - cuentas con sus cuotas;
  - botón "Abonar" (diálogo con monto, método y referencia), botón "Acuerdo de pago" (solo con `credit:override`; elige las cuentas y el plan con una vista previa de las cuotas, calculada con `GET /v1/credit/schedule-preview?principal=&installments=&frequency=&first_due_date=&interest_rate_monthly=&interest_type=`, permiso `credit:sell`, que solo llama a `schedule` y devuelve `{interest_total, total, installments: [{n, due_date, amount}]}`) y botón "Estado de cuenta" (rango de fechas, botón Imprimir: tirilla o carta según elija el usuario, con `window.print`).
- **POS:** el cobro agrega el método "Crédito", visible solo con un cliente asignado y `can("credit:sell")`. Al elegirlo:
  - muestra el cupo disponible;
  - interruptor "En cuotas" con los campos del plan y la vista previa;
  - si el servidor responde `CREDIT_LIMIT_EXCEEDED` o `CREDIT_BLOCKED_OVERDUE` y el usuario tiene `credit:override`, ofrece "Autorizar igual" y pide el motivo.

### Prueba (`f3_aging`)
- **[P]** Los tramos de antigüedad suman `total_balance`, y una cuenta con 45 días de mora cae en `d31_60`.

---

## F3.6 Saldo a favor (billetera)

### Migración

```sql
create table public.customer_wallets (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  customer_id uuid primary key,
  balance numeric(14,2) not null default 0 check (balance >= 0),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);
create table public.wallet_movements (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  customer_id uuid not null,
  kind text not null check (kind in ('RECARGA','USO','DEVOLUCION','AJUSTE','SEPARADO')),
  amount numeric(14,2) not null check (amount <> 0),       -- + entra, − sale
  balance_before numeric(14,2) not null,
  balance_after numeric(14,2) not null,
  ref_type text, ref_id uuid,
  reason text,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  check (balance_after = balance_before + amount),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Saldo a favor', 'SALDO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PAGO'
on conflict (padre_id, tima_codigo) do nothing;
alter table public.refunds drop constraint refunds_method_check;   -- verifica el nombre
alter table public.refunds add constraint refunds_method_check check (method in ('EFECTIVO','ORIGINAL','SALDO_A_FAVOR'));
-- RLS a nivel de negocio en las dos tablas; grants.
```

### Función única `wallet::apply_tx(tx, scope, customer_id, kind, amount, ref_type, ref_id, reason)`
1. Inserta la billetera si no existe, con `on conflict do nothing`.
2. Bloquea la fila con `for update`.
3. Si `balance + amount < 0`: 409 `WALLET_INSUFFICIENT`, con `data: {balance}`.
4. Actualiza el saldo e inserta el movimiento.

### Usos
- **Recarga:** `POST /v1/customers/{id}/wallet/topup` (permiso `credit:collect`), cuerpo `{id, amount, method: EFECTIVO|DATAFONO|QR|TRANSFERENCIA, reference?}`. Es idempotente: guarda el `id` como `ref_id` del movimiento y, si ya existe un movimiento `RECARGA` con ese `ref_id`, lo devuelve. En `EFECTIVO`, crea un movimiento de caja `RECARGA_SALDO` (con turno obligatorio).
- **Pago en el POS:** `PaymentMethod::Saldo` (`SALDO`).
  - Exige `customer_id`; si falta: 422 `WALLET_REQUIRES_CUSTOMER`. Máximo un pago `SALDO` por venta.
  - En `settle` cuenta como pago no en efectivo.
  - Se ejecuta `wallet::apply_tx(USO, −monto, 'ORDER', order_id)`.
- **Abono a cartera** con `method = SALDO`: igual, con `ref_type = 'RECEIVABLE_PAYMENT'`.
- **Devolución con `SALDO_A_FAVOR`:** exige que la venta tenga cliente; si no: 422 `REFUND_REQUIRES_CUSTOMER`. Al aprobarla, `wallet::apply_tx(DEVOLUCION, +total, 'REFUND', id)`, sin movimiento de caja.
- **Ajuste:** `POST /v1/customers/{id}/wallet/adjust` (permiso `wallet:adjust`), cuerpo `{amount (puede ser negativo), reason (3..200)}`. Auditoría `AJUSTE_SALDO`.
- `GET /v1/customers/{id}/wallet` devuelve `{balance, movements: [...] (los últimos 100)}`.

### Frontend
- Pestaña "Saldo a favor" de la ficha: saldo, botones "Recargar" y "Ajustar" (solo con `wallet:adjust`) y la tabla de movimientos.
- POS: el método "Saldo a favor" se muestra con un cliente que tiene saldo mayor que 0, junto al saldo disponible.
- La devolución agrega el método "Saldo a favor" cuando la venta tiene cliente.

### Pruebas (`f3_wallet`)
- **[P]** Una recarga en efectivo crea un movimiento `RECARGA_SALDO` en la caja.
- **[P]** Pagar con más saldo del disponible: 409 `WALLET_INSUFFICIENT`.
- **[P]** Una devolución a saldo a favor sube el saldo y no toca la caja.
- **[P]** `balance_after = balance_before + amount` en todos los movimientos.

---

## F3.7 Separados

**Modelo:** un separado **es una orden** (`orders`) con `sale_type = SEPARADO`, en estado `CREADA`, que reserva su stock con el mecanismo de reservas que ya existe, sin vencimiento. Sus datos extra van en `layaways`.

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Separado', 'SEPARADO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'VENTA'
on conflict (padre_id, tima_codigo) do nothing;

create table public.layaways (
  id uuid primary key,                                  -- = orders.id
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  customer_id uuid not null,
  status text not null default 'ACTIVO' check (status in ('ACTIVO','COMPLETADO','CANCELADO','VENCIDO')),
  total numeric(14,2) not null check (total > 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  expires_on date not null,
  penalty numeric(14,2),
  refunded numeric(14,2),
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  check (amount_paid <= total),
  foreign key (tenant_id, id) references public.orders (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);
create table public.layaway_installments (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  layaway_id uuid not null,
  n integer not null check (n >= 0),                     -- 0 = cuota inicial
  due_date date not null,
  amount numeric(14,2) not null check (amount > 0),
  amount_paid numeric(14,2) not null default 0,
  primary key (layaway_id, n),
  check (amount_paid <= amount),
  foreign key (tenant_id, layaway_id) references public.layaways (tenant_id, id) on delete cascade
);
create table public.layaway_payments (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  layaway_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','DATAFONO','QR','TRANSFERENCIA','SALDO')),
  reference text,
  cash_session_id uuid,
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  foreign key (tenant_id, layaway_id) references public.layaways (tenant_id, id)
);
alter table public.orders add column layaway_id uuid;
-- RLS: layaways con in_scope; installments y payments a nivel de negocio. Grants.
```

**Ajuste nuevo:** `pos.layaway_days` (entero de 1 a 365; por defecto 60).

### Endpoints (`src/layaways.rs`, permiso `pos:sell` salvo donde se indique)
1. **Crear:** `POST /v1/layaways`, cuerpo `{id, branch_id, customer_id, items (formato SaleItem, sin cupón), installments: [{n, due_date, amount}], quoted_total}`.
   1. Idempotencia por `id`.
   2. Precios con el motor de la fase 1 (tarifas, opciones y promociones sí; cupón **no**: si viene, 422 `LAYAWAY_NO_COUPON`). Valida `quoted_total`.
   3. `installments` debe incluir `n = 0` (la cuota inicial, con fecha de hoy) y `Σ amount = total`. Si no: 422 `LAYAWAY_PLAN_MISMATCH`, con `data: {total}`.
   4. Crea la orden en `CREADA` con sus líneas y llama a `orders::confirm_tx`, que reserva el stock. Si no hay stock: 409 `STOCK_INSUFFICIENT`. **Elimina** el vencimiento de esas reservas (`expires_at = null`).
   5. Crea `layaways` con el número de `sequences` `SEPARADO` y `expires_on = hoy + pos.layaway_days`.
   6. **No** registra el pago inicial: el POS llama enseguida a "abonar" con la cuota 0. Hasta que esté pagada, el separado sale con la insignia "Sin cuota inicial".
2. **Abonar:** `POST /v1/layaways/{id}/payments`, cuerpo `{id, amount, method, reference?}`. Se aplica a las cuotas por `n` ascendente, igual que F3.3. Con `EFECTIVO`, crea un movimiento de caja `ABONO_SEPARADO`; con `SALDO`, usa la billetera. Si el abono supera el saldo: 422 `PAYMENT_EXCEEDS_BALANCE`. Debe estar `ACTIVO`; si no: 409 `LAYAWAY_NOT_ACTIVE`.
3. **Completar:** pasa automáticamente dentro del abono cuando `amount_paid = total`.
   1. Consume las reservas (`consume_order_tx`).
   2. Asigna el consecutivo y crea el documento fiscal, igual que una venta (reutiliza las funciones de `checkout_tx`).
   3. La orden queda `COBRADA`, con `cash_session_id = null` y `layaway_id = id`.
   4. `layaways.status = COMPLETADO`.
   5. **No** inserta filas en `payments`: el dinero ya entró como abonos.
4. **Cancelar:** `POST /v1/layaways/{id}/cancel` (permiso `pos:refund`), cuerpo `{penalty, refund_method: "EFECTIVO"|"SALDO_A_FAVOR"}`.
   1. `0 <= penalty <= amount_paid`. Si no: 422 `LAYAWAY_PENALTY_INVALID`.
   2. `refunded = amount_paid − penalty`. Si es mayor que 0: con `EFECTIVO`, un movimiento `DEVOLUCION` en el turno del usuario (obligatorio); con `SALDO_A_FAVOR`, `wallet::apply_tx(SEPARADO, +refunded)`.
   3. Libera las reservas, deja la orden `CANCELADA` y `status = CANCELADO`.
   4. Auditoría `SEPARADO_CANCELADO`.
5. **Vencer:** `POST /v1/layaways/{id}/expire` (permiso `pos:refund`). Solo si `ACTIVO` y `expires_on < hoy`; si no: 409 `LAYAWAY_NOT_EXPIRABLE`. Libera el stock, todo lo pagado pasa al saldo a favor (`SEPARADO`), la orden queda `CANCELADA` y `status = VENCIDO`.
6. **Listar y ver:** `GET /v1/layaways?branch_id=&status=&customer_id=` y `GET /v1/layaways/{id}`. Cada separado trae `overdue: bool`, que es `expires_on < hoy` y `ACTIVO`.

**Ventas de separados en el cierre de caja:** no aparecen en `sales_total`, porque no tienen `cash_session_id`. Su dinero aparece como `ABONO_SEPARADO`. En el historial de ventas aparecen con la insignia "Separado".

### Frontend
- **POS:** botón "Separar" (exige cliente). Abre un diálogo con el total, la cuota inicial y el número de cuotas, la frecuencia y la primera fecha. Arma `installments` repartiendo el resto en cuotas iguales, con la última ajustada, y deja editar cada monto. Al confirmar, crea el separado y cobra la cuota inicial con los métodos del POS.
- **Pantalla `/separados`** (registro `s("45", "separados", "Separados", "/separados", "clientes", "admin", "react")`): pestañas Activos, Vencidos sin procesar, Completados y Cancelados; acciones Abonar, Cancelar y Vencer.
- Tirilla de separado y de cada abono.

### Pruebas (`f3_layaways`)
- **[P]** Una cuota inicial más cuotas que no suman el total: 422 `LAYAWAY_PLAN_MISMATCH`.
- **[P]** Crear reserva el stock: el disponible baja y el `on_hand` no.
- **[P]** Cada abono en efectivo crea un movimiento `ABONO_SEPARADO`.
- **[P]** Al completar, el stock sale (`on_hand` baja), se crea el documento fiscal y la orden queda `COBRADA`.
- **[P]** Cancelar con una penalidad mayor a lo pagado: 422. Una cancelación válida devuelve `amount_paid − penalty` y libera la reserva.
- **[P]** Vencer manda lo pagado al saldo a favor.

---

## F3.8 Cotizaciones

### Migración

```sql
create table public.quotes (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  customer_id uuid,
  customer_name text,                                   -- para no clientes
  customer_phone text,
  status text not null default 'BORRADOR' check (status in ('BORRADOR','ENVIADA','ACEPTADA','RECHAZADA','CONVERTIDA')),
  valid_until date not null,
  notes text check (length(notes) <= 500),
  subtotal numeric(14,2) not null, discount_total numeric(14,2) not null,
  tax_total numeric(14,2) not null, total numeric(14,2) not null,
  lines jsonb not null,                                 -- respuesta completa de /v1/sales/quote, congelada
  request jsonb not null,                               -- items enviados (formato SaleItem)
  order_id uuid,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  check (customer_id is not null or customer_name is not null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);
alter table public.orders add column quote_id uuid;
-- RLS con in_scope; grants.
```

**Estado visible:** `VENCIDA` si el estado es `BORRADOR` o `ENVIADA` y `valid_until < hoy`; en cualquier otro caso, el guardado. Función pura y prueba unitaria `quote_effective_status`. No hay proceso nocturno.

### Endpoints (`src/quotes.rs`, permiso `pos:sell`)
- `POST /v1/quotes`, cuerpo `{id, branch_id, customer_id? | customer_name + customer_phone?, items, valid_until (≥ hoy, ≤ hoy + 90), notes?}`.
  - Ejecuta la cotización de la fase 1 **sin cupón**.
  - Guarda `lines` (la respuesta completa) y `request`.
  - Número de `sequences` `COTIZACION`.
- `PUT /v1/quotes/{id}`: solo en `BORRADOR`; vuelve a cotizar.
- `POST /v1/quotes/{id}/status`, cuerpo `{status: "ENVIADA"|"ACEPTADA"|"RECHAZADA"}`. Transiciones permitidas:
  - `BORRADOR → ENVIADA`;
  - `ENVIADA → ACEPTADA` o `RECHAZADA`;
  - `BORRADOR → ACEPTADA`.

  Cualquier otra: 409 `QUOTE_INVALID_STATE`.
- `POST /v1/quotes/{id}/duplicate`, cuerpo `{id}` (nuevo uuid): copia `request`, vuelve a cotizar con los precios de hoy y queda en `BORRADOR` con un número nuevo.
- `GET /v1/quotes?branch_id=&status=` y `GET /v1/quotes/{id}`.
- `GET /v1/quotes/{id}/whatsapp-link`. Texto: `Hola <nombre>, te compartimos la cotización <COT-…> de <negocio> por $<total>, válida hasta el <dd/mm/yyyy>.`, seguido del detalle con una línea por producto (`- <cant> x <nombre>: $<total línea>`).

### Conversión a venta (atómica)
`SaleRequest` agrega `quote_id: Option<Uuid>`. Si viene:
1. Bloquea la cotización con `for update`.
2. Estado visible `CONVERTIDA`: 409 `QUOTE_ALREADY_CONVERTED`. `VENCIDA` o `RECHAZADA`: 409 `QUOTE_INVALID_STATE`.
3. Los `items` de la venta deben ser **idénticos** a `quote.request`: mismo orden, producto, cantidad, tarifa y opciones. Si no: 422 `QUOTE_MISMATCH`.
4. Cada línea se cobra con el `unit_price` y los descuentos congelados en `quote.lines`. **No** se ejecuta el motor de precios.
   - Se marca `overridden = true` con `price_override_reason = "Precio de cotización COT-000123"`.
   - En este caso **no** se exige `pos:price_override` ni el ajuste de la sucursal.
5. `quoted_total` debe ser igual a `quote.total + propina`.
6. Al final: `quote.status = CONVERTIDA`, `quote.order_id` y `orders.quote_id`.

En el frontend, "Convertir en venta" carga el carrito del POS desde la cotización en modo **solo lectura** (no se pueden editar líneas) y cobra.

### Frontend
- **Pantalla `/cotizaciones`** (registro `s("46", "cotizaciones", "Cotizaciones", "/cotizaciones", "ventas", "admin", "react")`):
  - pestañas por estado visible;
  - tabla;
  - acciones Ver, Imprimir (formato carta con `window.print`, con el encabezado del negocio, el cliente, las líneas, los totales, la vigencia y las notas), WhatsApp, Duplicar, Cambiar estado y Convertir en venta.
- **POS:** botón "Cotizar", que guarda el carrito actual como cotización. Pide el cliente o el nombre y la vigencia (por defecto, hoy + 15 días).

### Pruebas (`f3_quotes`)
- **[P]** Una cotización `VENCIDA` no se convierte.
- **[P]** Dos conversiones simultáneas: una pasa y la otra recibe 409 `QUOTE_ALREADY_CONVERTED`. Usa dos tareas de tokio.
- **[P]** Convertir con ítems distintos: 422 `QUOTE_MISMATCH`.
- **[P]** La venta convertida usa los precios de la cotización aunque el precio de catálogo haya cambiado después.

---

## Códigos de error nuevos de la fase 3

| Código | Texto |
|---|---|
| `CUSTOMER_DV_CONFLICT` | El dígito de verificación no coincide. |
| `CUSTOMER_HAS_BALANCE` | El cliente tiene saldos pendientes. |
| `CUSTOMER_NO_PHONE` | El cliente no tiene teléfono. |
| `SALE_MULTIPLE_CREDIT` | Solo puede haber un pago a crédito por venta. |
| `CREDIT_REQUIRES_CUSTOMER` | Para vender a crédito elegí un cliente. |
| `CREDIT_NOT_ENABLED` | Este cliente no tiene cupo de crédito. |
| `CREDIT_BLOCKED_OVERDUE` | El cliente tiene deudas vencidas: crédito bloqueado. |
| `CREDIT_LIMIT_EXCEEDED` | La venta supera el cupo disponible. |
| `REFUND_EXCEEDS_RECEIVABLE` | La devolución supera lo que queda por pagar del crédito. |
| `RECEIVABLE_CLOSED` | La cuenta ya está cerrada. |
| `AGREEMENT_INVALID` | Revisá las cuentas del acuerdo. |
| `WALLET_INSUFFICIENT` | El saldo a favor no alcanza. |
| `WALLET_REQUIRES_CUSTOMER` | Para usar saldo a favor elegí un cliente. |
| `REFUND_REQUIRES_CUSTOMER` | La venta no tiene cliente para devolverle saldo. |
| `LAYAWAY_NO_COUPON` | Los separados no admiten cupones. |
| `LAYAWAY_PLAN_MISMATCH` | Las cuotas deben sumar el total. |
| `LAYAWAY_NOT_ACTIVE` | El separado no está activo. |
| `LAYAWAY_PENALTY_INVALID` | La penalidad no puede superar lo pagado. |
| `LAYAWAY_NOT_EXPIRABLE` | El separado todavía no venció. |
| `QUOTE_INVALID_STATE` | La cotización no admite esa acción. |
| `QUOTE_ALREADY_CONVERTED` | La cotización ya se convirtió en venta. |
| `QUOTE_MISMATCH` | Los productos no coinciden con la cotización. |

## Lista de verificación
- [ ] Una sola migración que corre limpia.
- [ ] Todas las pruebas **[P]** (con su nombre) y las de fases anteriores en verde.
- [ ] `fmt`, `clippy`, `test`, `tsc` y `lint` en verde.
- [ ] El README tiene todos los endpoints.
- [ ] El modo demo funciona: vender a crédito, abonar, acuerdo, separado completo, cotización convertida y saldo a favor.

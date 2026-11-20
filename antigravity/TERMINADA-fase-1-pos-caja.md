# Fase 1: POS y caja v2

> **TERMINADA.** Fase 1 completa el 2026-11-01 (ver `TERMINADA-REPORTE-FASE-1.md`). No se construye de nuevo: este documento sirve para consultar cómo funcionan el POS y la caja. Los cambios posteriores a esta fase están en los módulos de `antigravity/modulos/` (por ejemplo, la devolución aprobada a distancia en M13).

> **Requisito:** la fase 0 debe estar aprobada por Claude. Crea la rama `feat/fase-1-pos-caja` en `nexa-backend` y en `nexa-frontend`, a partir de `feat/fase-0-plataforma`.
>
> **Cómo leer esto:** todo lo que no esté escrito aquí está **prohibido**. No agregues campos, endpoints, pantallas, botones ni reglas que no aparezcan. Si una instrucción no se puede cumplir tal cual, detén **esa** tarea, escribe la duda en el reporte y sigue con la siguiente.

## Orden de las tareas

Hazlas en este orden, con un commit por tarea (`feat(fase1): F1.N <resumen>`):

| Tarea | Tema |
|---|---|
| F1.0 | Base técnica: errores con código, redondeo, permisos en `/v1/context` |
| F1.1 | Cajas registradoras |
| F1.2 | Tipos de movimiento de caja y nuevo cálculo del esperado |
| F1.3 | Control de cambios durante el arqueo |
| F1.4 | Clientes mínimos y cliente/alias en la venta |
| F1.5 | Nuevo modelo de líneas y motor de precios (`pricing.rs`) |
| F1.6 | Tarifas y presentaciones |
| F1.7 | Promociones |
| F1.8 | Cupones |
| F1.9 | Cambio de precio e ítem libre |
| F1.10 | Propinas |
| F1.11 | Devoluciones |
| F1.12 | Anulación de ventas |
| F1.13 | Historial de ventas (pantalla 04) |
| F1.14 | POS: cotización en vivo, atajos, ventas en espera, efectivo rápido, lector |
| F1.15 | Tirilla y precuenta |

**Toda la migración de la fase va en un solo archivo:** `nexa-backend/supabase/migrations/20261103000100_fase1_pos_caja.sql`. Agrega cada bloque en el orden de las tareas, con un comentario `-- F1.N` encima.

**Fuera de alcance en esta fase** (no lo hagas aunque parezca útil):
- balanza por Web Serial;
- modo sin conexión e IndexedDB;
- clave de supervisor para superar el descuento máximo;
- pedidos pendientes al cerrar caja (pasa a la fase 5, con mesas);
- saldo a favor como medio de devolución (fase 3);
- envío por WhatsApp.

---

## F1.0 Base técnica

### Backend

**1. Errores con código.** En `src/error.rs`:
- agrega la variante `DomainError::Coded { status: StatusCode, code: &'static str, message: String, data: Option<serde_json::Value> }`;
- `status()` devuelve `status`;
- agrega el helper `pub fn coded(status: StatusCode, code: &'static str, message: impl Into<String>, data: Option<serde_json::Value>) -> ApiError`;
- en `into_response`, si el error es `Coded`, el cuerpo es `{"error": message, "code": code, "data": data}` (omite `data` si es `None`). Los demás errores no cambian.
- Prueba unitaria `coded_error_body_has_code` en `error.rs`: un `coded(StatusCode::CONFLICT, "X", "m", None)` responde 409 y su cuerpo JSON tiene `code = "X"`.

Todos los códigos de error nuevos de esta fase usan `coded`. La lista completa está al final del documento. **No inventes códigos.**

**2. Redondeo.** Crea `src/money.rs` con:

```rust
pub fn money(x: Decimal) -> Decimal { x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero) }
pub fn money_floor(x: Decimal) -> Decimal { x.round_dp_with_strategy(2, RoundingStrategy::ToZero) }
```

Regístralo en `main.rs`. Todo valor monetario calculado en Rust pasa por `money` antes de guardarse o compararse. Agrega la prueba unitaria `money_rounds_half_away_from_zero` con estos casos: `0.005 → 0.01`, `-0.005 → -0.01`, `1.234 → 1.23`.

**3. Permisos en el contexto.** `GET /v1/context` agrega el campo `permissions: string[]`, con los permisos efectivos del usuario en la sucursal activa, ordenados alfabéticamente. Para `DUENO` van todos los códigos de la tabla `permissions`.

### Frontend
- En `lib/stores/session.ts` agrega `permissions: string[]` y la función `can(code: string): boolean`.
- El mock de contexto devuelve todos los permisos para el usuario demo dueño.
- **Regla para toda la fase:** un botón o acción que exige un permiso **no se muestra** si `can(...)` es falso. No lo muestres deshabilitado: ocúltalo.
- En `lib/api/index.ts`, `friendlyError` lee `code` del cuerpo y lo traduce con un mapa `ERROR_MESSAGES: Record<string, string>`. Los textos están en la tabla de códigos al final.

### Pruebas
- `context_returns_permissions` en `db_tests.rs`: un cajero recibe exactamente los permisos base de `CAJERO`, y el dueño recibe todos.

---

## F1.1 Cajas registradoras

### Migración

```sql
create table public.cash_registers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 60),
  code text not null check (code ~ '^[A-Z0-9-]{1,10}$'),
  default_opening numeric(14,2) not null default 0 check (default_opening >= 0),
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (branch_id, code),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
-- RLS: política cash_registers_isolation con public.in_scope(tenant_id, branch_id) y grants a authenticated (plantilla de 00-LEEME).

-- Una caja "Caja 1" por cada sucursal existente
insert into public.cash_registers (tenant_id, branch_id, name, code)
select tenant_id, id, 'Caja 1', 'CAJA-1' from public.branches;

alter table public.cash_sessions add column register_id uuid;
update public.cash_sessions s set register_id = r.id
  from public.cash_registers r where r.branch_id = s.branch_id and r.code = 'CAJA-1';

-- Guardia: si hay dos turnos abiertos en la misma sucursal, la migración se detiene
do $$ begin
  if exists (select 1 from public.cash_sessions where closed_at is null
             group by branch_id having count(*) > 1) then
    raise exception 'Hay sucursales con mas de un turno abierto: cerrarlos antes de migrar';
  end if;
end $$;

alter table public.cash_sessions alter column register_id set not null;
alter table public.cash_sessions add constraint cash_sessions_register_fkey
  foreign key (tenant_id, register_id) references public.cash_registers (tenant_id, id);
create unique index cash_sessions_one_open_per_register
  on public.cash_sessions (register_id) where closed_at is null;
```

El índice que ya existe, `cash_sessions_one_open_idx` (un turno abierto por usuario y sucursal), **se queda**.

Además, cada sucursal nueva necesita su caja. En `branches` no hay endpoint de creación todavía, así que agrega este trigger:

```sql
create function public.branch_default_register() returns trigger language plpgsql as $$
begin
  insert into public.cash_registers (tenant_id, branch_id, name, code)
  values (new.tenant_id, new.id, 'Caja 1', 'CAJA-1');
  return new;
end $$;
create trigger branches_default_register after insert on public.branches
  for each row execute function public.branch_default_register();
```

### Ajuste que se elimina
Quita `cash.one_session_per_user`:
- de `CashSettings` en `settings.rs`;
- de su uso en `cash.rs`;
- del tipo `BranchSettings` del frontend;
- de la pestaña Caja de `/configuracion/negocio`;
- de su prueba.

La regla queda fija por el índice: **un usuario tiene como máximo un turno abierto por sucursal**.

### Backend: archivo nuevo `src/registers.rs`

| Método y ruta | Permiso | Cuerpo | Respuesta |
|---|---|---|---|
| `GET /v1/cash-registers?branch_id=` | `cash:open` o `settings:write` | — | `200 [Register]` |
| `POST /v1/cash-registers` | `settings:write` | `{branch_id, name, code, default_opening}` | `201 Register` |
| `PUT /v1/cash-registers/{id}` | `settings:write` | `{name, code, default_opening, active}` | `200 Register` |

`Register = {id, branch_id, name, code, default_opening, active, open_session: null | {id, opened_by, opened_by_name, opened_at}}`

La lista va ordenada por `code`.

**Validaciones, en este orden:**
1. `name` sin espacios a los lados, entre 1 y 60 caracteres. Si no: 422 `REGISTER_INVALID`.
2. `code` en mayúsculas y sin espacios a los lados; debe cumplir `^[A-Z0-9-]{1,10}$`. Si no: 422 `REGISTER_INVALID`.
3. `default_opening >= 0`. Si no: 422 `REGISTER_INVALID`.
4. Código repetido en la sucursal: 409 `REGISTER_CODE_TAKEN`. Revísalo antes del insert.
5. `PUT` con `active = false` cuando la caja tiene un turno abierto: 409 `REGISTER_HAS_OPEN_SESSION`.

No hay endpoint para borrar cajas.

**Auditoría:** `CAJA_CREADA` y `CAJA_EDITADA` sobre la entidad `cash_registers`, con `before` y `after`.

### Backend: abrir turno
`POST /v1/cash-sessions` agrega `register_id` (obligatorio) al cuerpo. Antes del insert valida, en este orden:
1. La caja existe, pertenece a la sucursal del cuerpo y está activa. Si no: 422 `REGISTER_INVALID`.
2. La caja no tiene turno abierto. Si lo tiene: 409 `REGISTER_BUSY`, con `data: {opened_by_name}`.
3. El usuario no tiene otro turno abierto en la sucursal. Si lo tiene: 409 `USER_HAS_OPEN_SESSION`.

`CashSession` agrega `register_id`, `register_name` y `register_code`.

### Frontend
- **Pantalla nueva** `/configuracion/cajas`:
  - componente `components/configuracion/registers-screen.tsx`;
  - página `app/(admin)/configuracion/cajas/page.tsx`;
  - regístrala en `lib/screens.ts` como `s("32", "cajas", "Cajas registradoras", "/configuracion/cajas", "configuracion", "admin", "react")`;
  - en el menú lateral va bajo Configuración y solo se ve con `settings:write`.
- **Contenido:**
  - tabla con las columnas Código, Nombre, Base sugerida, Estado (Activa/Inactiva) y Turno (nombre del cajero o "—");
  - botón "Nueva caja";
  - diálogo con los campos Nombre, Código y Base sugerida (`MoneyInput`);
  - en cada fila, acción "Editar" (mismo diálogo, más el interruptor "Activa").
- **Abrir turno** (`components/caja/open-shift.tsx`):
  - si la sucursal tiene **una** caja activa, se muestra su nombre sin selector;
  - si tiene **más de una**, aparece un selector "Caja" con las cajas activas. Las que tienen turno abierto salen deshabilitadas con el texto "(en uso por <nombre>)";
  - el monto de apertura se precarga con `default_opening` de la caja elegida.
- **Mock:** dos cajas, "Caja 1" (`CAJA-1`, base $200.000) y "Caja 2" (`CAJA-2`, base $100.000).

### Pruebas (`db_tests.rs`, función `f1_registers`)
- **[P]** Dos usuarios no pueden abrir turno en la misma caja: el segundo recibe 409 `REGISTER_BUSY`.
- **[P]** Un usuario no puede abrir dos turnos en la misma sucursal, ni siquiera en cajas distintas: 409 `USER_HAS_OPEN_SESSION`.
- **[P]** No se puede desactivar una caja con un turno abierto: 409 `REGISTER_HAS_OPEN_SESSION`.
- **[P]** Un cajero no puede crear cajas: 403.
- Una sucursal recién insertada tiene `CAJA-1`.

---

## F1.2 Tipos de movimiento de caja y esperado

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Abono de cartera','ABONO_CARTERA'), ('Abono de separado','ABONO_SEPARADO'),
             ('Recarga de saldo','RECARGA_SALDO'), ('Devolucion','DEVOLUCION'),
             ('Propina pagada','PROPINA_PAGADA'), ('Pago a proveedor','PAGO_PROVEEDOR'),
             ('Gasto','GASTO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'CAJA_MOV') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.cash_movements
  add column reason_code text check (reason_code in ('CONSIGNACION','GASTO_MENOR','PRESTAMO','OTRO')),
  add column ref_type text,
  add column ref_id uuid;
```

### Backend (`cash.rs`)

**1. Enum de tipos.** `CashMovementKind` pasa a tener 9 variantes, cada una con su dirección:

| Variante | Código | Dirección |
|---|---|---|
| `Ingreso` | `INGRESO` | entrada |
| `AbonoCartera` | `ABONO_CARTERA` | entrada |
| `AbonoSeparado` | `ABONO_SEPARADO` | entrada |
| `RecargaSaldo` | `RECARGA_SALDO` | entrada |
| `Retiro` | `RETIRO` | salida |
| `Devolucion` | `DEVOLUCION` | salida |
| `PropinaPagada` | `PROPINA_PAGADA` | salida |
| `PagoProveedor` | `PAGO_PROVEEDOR` | salida |
| `Gasto` | `GASTO` | salida |

Agrega `fn is_inflow(self) -> bool`.

**2. Función interna única** que usan todos los módulos:

```rust
pub async fn record_movement_tx(tx, scope, session_id: Uuid, kind: CashMovementKind, amount: Decimal,
    reason: &str, reason_code: Option<&str>, ref_type: Option<&str>, ref_id: Option<Uuid>) -> Result<Uuid, ApiError>
```

Pasos, en orden:
1. `amount = money(amount)`; si `amount <= 0`: 422 `CASH_AMOUNT_INVALID`.
2. Bloquea el turno con `lock_open_session_tx` (ya existe).
3. Si el tipo es de salida y `amount > summary_tx(...).expected_cash`: 409 `CASH_INSUFFICIENT`, con `data: {available}`.
4. Inserta el movimiento.
5. Registra la auditoría con la acción `MOVIMIENTO_CAJA` y `after = {kind, amount, reason, ref_type, ref_id}`.

**3. Endpoint público** `POST /v1/cash-sessions/{id}/movements` (permiso `cash:movement`). Su cuerpo es `{kind, amount, reason, reason_code?}`, y `kind` solo admite:
- `INGRESO`: `reason` obligatorio (de 3 a 200 caracteres); `reason_code` debe venir nulo.
- `RETIRO`: `reason_code` obligatorio. Si es `OTRO`, `reason` es obligatorio (de 3 a 200 caracteres). Con cualquier otro código, `reason` es opcional y, si viene vacío, se guarda la etiqueta del código.
- `PROPINA_PAGADA`: `reason` obligatorio, con el nombre del mesero.

Cualquier otro `kind`: 422 `CASH_KIND_NOT_ALLOWED`.

Etiquetas de `reason_code`:

| Código | Etiqueta |
|---|---|
| `CONSIGNACION` | Consignación a banco |
| `GASTO_MENOR` | Gasto menor |
| `PRESTAMO` | Préstamo |
| `OTRO` | Otro |

**4. Esperado.** En `summary_tx`:

```
esperado = apertura
         + efectivo de ventas
         + suma de movimientos de entrada
         − suma de movimientos de salida
```

- "Efectivo de ventas" es la suma de `payments.amount` con método `EFECTIVO` en el turno, **solo** de órdenes cuyo estado no es `CANCELADA` (ver F1.12).
- `by_method` también excluye los pagos de órdenes `CANCELADA`.
- Orden de `by_method`: `EFECTIVO` primero y el resto alfabético.

`CashSummary` agrega:
- `movements_by_kind: [{kind, label, direction: "IN"|"OUT", amount, count}]`, con los 9 tipos siempre presentes (en 0 si no hubo), en el orden de la tabla de arriba;
- `cash_in` y `cash_out`, que pasan a ser la suma de todas las entradas y de todas las salidas;
- `tips_total`: suma de `orders.tip_amount` del turno, sin las órdenes `CANCELADA`;
- `tips_paid`: suma de `PROPINA_PAGADA`;
- `tips_pending = tips_total − tips_paid`.

### Frontend
- **Diálogo de movimiento** (`components/caja/cash-movement-dialog.tsx`), con tres pestañas:
  - **Ingreso**: monto y motivo;
  - **Retiro**: monto, selector de motivo con los 4 códigos, y texto obligatorio solo si el motivo es "Otro";
  - **Pagar propina**: monto (sugiere `tips_pending`) y mesero (texto).
- **Bitácora:** muestra cada tipo con su etiqueta, verde si es de entrada y rojo si es de salida.
- **Cierre Z** (`z-report.tsx`):
  - tabla de los 9 tipos;
  - fila de propinas con Total, Pagadas y Por pagar.

### Pruebas (`f1_cash_kinds`)
- **[P]** El esperado suma y resta los 9 tipos. Inserta uno de cada tipo con `record_movement_tx` y compara el total con la fórmula.
- **[P]** Un retiro mayor al esperado: 409 `CASH_INSUFFICIENT`.
- **[P]** El endpoint público rechaza `kind = DEVOLUCION`: 422 `CASH_KIND_NOT_ALLOWED`.
- **[P]** Un `RETIRO` sin `reason_code`: 422.

---

## F1.3 Control de cambios durante el arqueo

El cierre sigue siendo **ciego**: el cajero cuenta sin ver el esperado.

### Backend
- `GET /v1/cash-sessions/current` y `GET /v1/cash-sessions/{id}/activity` agregan `state_token: string`, calculado así:
  1. junta los `id` de todos los `payments` y `cash_movements` del turno;
  2. ordénalos como texto;
  3. únelos con `,`;
  4. el token es el SHA-256 en hexadecimal de esa cadena.

  Usa el crate `sha2`. Si no está en `Cargo.toml`, agrégalo: es la única dependencia nueva permitida en esta fase.
- `POST /v1/cash-sessions/{id}/close` agrega `state_token` (obligatorio) al cuerpo. Después de bloquear el turno, recalcula el token. Si es distinto al recibido: 409 `CASH_STATE_CHANGED`, sin `data`.

### Frontend
- La pantalla de arqueo guarda el `state_token` que recibió al abrirse y lo envía al cerrar.
- Ante `CASH_STATE_CHANGED`:
  1. muestra el mensaje "Hubo movimientos nuevos mientras contabas. Volvé a contar.";
  2. recarga el turno;
  3. borra el conteo por denominación.

### Prueba (`f1_cash_state`)
- **[P]** Lee el token, registra una venta en el turno y cierra con el token viejo: 409 `CASH_STATE_CHANGED`. Con el token nuevo el cierre funciona.

---

## F1.4 Clientes mínimos y cliente en la venta

### Migración

```sql
create table public.customers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  doc_type text check (doc_type in ('CC','NIT','CE','PP','TI','PEP','PPT','NIUP')),
  doc_number text check (doc_number ~ '^[A-Z0-9]{3,20}$'),
  dv char(1) check (dv ~ '^[0-9]$'),
  name text not null check (length(trim(name)) between 1 and 120),
  phone text check (phone ~ '^[0-9]{7,15}$'),
  email text,
  price_tier_id uuid,
  archived_at timestamptz,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  check ((doc_type is null) = (doc_number is null)),
  check (dv is null or doc_type = 'NIT')
);
create unique index customers_doc_idx on public.customers (tenant_id, doc_type, doc_number)
  where doc_number is not null and archived_at is null;
create index customers_name_idx on public.customers (tenant_id, lower(name));
-- RLS a nivel de negocio: customers_isolation con tenant_id = public.current_tenant_id(); grants a authenticated.

alter table public.orders
  add column customer_id uuid,
  add column customer_alias text check (customer_alias is null or length(customer_alias) between 1 and 40),
  add constraint orders_customer_fkey foreign key (tenant_id, customer_id) references public.customers (tenant_id, id),
  add constraint orders_customer_xor check (customer_id is null or customer_alias is null);
```

La FK de `price_tier_id` se agrega en F1.6.

### Backend: archivo nuevo `src/customers.rs`

| Método y ruta | Permiso | Detalle |
|---|---|---|
| `GET /v1/customers?q=&limit=` | `customers:read` | Busca en `name` (ILIKE `%q%`), `doc_number` (prefijo) y `phone` (prefijo); excluye archivados; `limit` por defecto 20 y máximo 50; orden por `name`. |
| `POST /v1/customers` | `customers:write` | Cuerpo `{doc_type?, doc_number?, dv?, name, phone?, email?}`. |

`Customer = {id, doc_type, doc_number, dv, name, phone, email, price_tier_id}`

**Normalización antes de validar** (función pura `normalize_customer`, con prueba unitaria):
- `doc_number`: mayúsculas y sin ningún carácter que no sea letra o dígito. Si queda vacío, se toma como `None`.
- `name`: sin espacios a los lados y con los espacios internos repetidos reducidos a uno.
- `phone`: solo dígitos; si queda vacío, `None`.
- `email`: minúsculas y sin espacios a los lados; si queda vacío, `None`. Si no contiene exactamente una `@`: 422 `CUSTOMER_INVALID`.

Documento repetido (mismo tipo y número, sin archivar): 409 `CUSTOMER_DUPLICATED`, con `data: {id}` del cliente existente.

### Backend: venta
`SaleRequest` agrega `customer_id: Option<Uuid>` y `customer_alias: Option<String>`. En `checkout_tx`, después de cargar los ajustes, valida en este orden:
1. Vienen los dos: 422 `SALE_CUSTOMER_AND_ALIAS`.
2. `customer_alias` sin espacios a los lados debe tener entre 1 y 40 caracteres. Si no: 422 `SALE_ALIAS_INVALID`.
3. Viene alias y `settings.pos.allow_alias_sales` es falso: 422 `SALE_ALIAS_NOT_ALLOWED`.
4. No viene ninguno y `settings.pos.allow_anonymous_sales` es falso: 422 `SALE_CUSTOMER_REQUIRED`.
5. Viene `customer_id` y no existe o está archivado: 422 `SALE_CUSTOMER_INVALID`.

Se guardan en `orders.customer_id` y `orders.customer_alias`. `Order` agrega `customer_id`, `customer_alias` y `customer_name` (obtenido con un join).

### Frontend
- **Componente nuevo** `components/pos/customer-dialog.tsx`. Se abre con F4 o con el botón "Cliente" del carrito, y tiene tres pestañas:
  1. **Buscar**: campo de búsqueda (llama a `api.customers.search` con 300 ms de espera) y lista de resultados; al hacer clic, se asigna el cliente.
  2. **Nuevo**: Nombre (obligatorio), Tipo de documento (selector de los 8 tipos más "Sin documento"), Número, DV (solo visible si el tipo es NIT), Teléfono y Correo. Al guardar, crea el cliente y lo asigna. Ante `CUSTOMER_DUPLICATED`, ofrece el botón "Usar el existente".
  3. **Alias**: un campo y el botón "Usar alias". Solo aparece si `allow_alias_sales` está activo.
- El carrito muestra el cliente o alias asignado, con una "x" para quitarlo.
- **Mock:** 6 clientes colombianos. Uno con NIT y DV, uno con tarifa "Mayorista" (F1.6) y los demás con cédula.

### Pruebas (`f1_sale_customer`)
- **[P]** Con `allow_anonymous_sales = false`, una venta sin cliente: 422 `SALE_CUSTOMER_REQUIRED`.
- **[P]** Con `allow_alias_sales = false`, una venta con alias: 422 `SALE_ALIAS_NOT_ALLOWED`.
- **[P]** Cliente y alias juntos: 422 `SALE_CUSTOMER_AND_ALIAS`.
- Prueba unitaria `normalize_customer_cleans_fields`.

---

## F1.5 Nuevo modelo de líneas y motor de precios

### Migración

```sql
alter table public.order_items drop column tax_amount;
alter table public.order_items drop column line_total;
alter table public.order_items
  alter column product_id drop not null,
  add column custom_name text check (custom_name is null or length(trim(custom_name)) between 1 and 80),
  add column tier_id uuid,
  add column units_per_package numeric(14,4) not null default 1 check (units_per_package > 0),
  add column list_price numeric(14,2),
  add column discount_manual numeric(14,2) not null default 0 check (discount_manual >= 0),
  add column discount_promotion numeric(14,2) not null default 0 check (discount_promotion >= 0),
  add column discount_order numeric(14,2) not null default 0 check (discount_order >= 0),
  add column discount_coupon numeric(14,2) not null default 0 check (discount_coupon >= 0),
  add column promotion_id uuid,
  add column price_override_reason text,
  add column line_discount numeric(14,2) generated always as
    (discount_manual + discount_promotion + discount_order + discount_coupon) stored,
  add column line_total numeric(14,2) generated always as
    (round(qty * unit_price, 2) - (discount_manual + discount_promotion + discount_order + discount_coupon)) stored,
  add column tax_amount numeric(14,2) generated always as
    (round((round(qty * unit_price, 2) - (discount_manual + discount_promotion + discount_order + discount_coupon)) * tax_rate, 2)) stored,
  add constraint order_items_product_xor_custom check ((product_id is null) = (custom_name is not null)),
  add constraint order_items_discount_le_gross check
    ((discount_manual + discount_promotion + discount_order + discount_coupon) <= round(qty * unit_price, 2));

alter table public.orders add column discount_total numeric(14,2) not null default 0;
```

### Significado de cada columna

| Columna | Qué guarda |
|---|---|
| `unit_price` | Precio **por unidad vendida**, sin impuesto, ya con tarifa o cambio de precio y antes de descuentos. Si la línea es una presentación (paca de 12), es el precio de la paca. |
| `list_price` | Precio de catálogo antes de tarifa o cambio de precio, por unidad vendida: precio base × `units_per_package`. Para un ítem libre es igual a `unit_price`. |
| `units_per_package` | Unidades de inventario que descuenta cada unidad vendida. Es 1 si no hay presentación. |
| `discount_*` | Montos totales de la línea, ya redondeados. |
| `line_total` | Base sin impuesto, ya descontada. |
| `tax_amount` | Impuesto sobre `line_total`. |

### Backend
- **`orders::recalc_totals`** calcula:
  - `subtotal = Σ line_total`;
  - `inc_amount = Σ tax_amount`;
  - `discount_total = Σ line_discount`;
  - `total = subtotal + inc_amount + tip_amount`.
- **`OrderItem` y `ITEM_COLS`** agregan todas las columnas nuevas. `product_id` pasa a `Option<Uuid>`. Agrega `name`, que es `products.name`, o `custom_name` si la línea es libre. Corrige todos los usos que no compilen. `orders.rs` (mesas) sigue creando líneas solo con producto.
- **Módulo nuevo `src/pricing.rs`** con una **función pura** sin acceso a la BD:

```rust
pub struct PriceLine { pub index: usize, pub product_id: Option<Uuid>, pub category_id: Option<Uuid>,
  pub qty: Decimal, pub unit_price: Decimal, pub tier_id: Option<Uuid>, pub overridden: bool,
  pub custom: bool, pub manual_pct: Option<Decimal>, pub tax_rate: Decimal }
pub struct PromoRule { pub id: Uuid, pub name: String, pub kind: Kind /*Porcentaje|Fijo*/, pub value: Decimal,
  pub scope: Scope /*Pedido|Producto|Categoria*/, pub product_ids: HashSet<Uuid>, pub category_ids: HashSet<Uuid>,
  pub min_qty: Option<Decimal>, pub min_subtotal: Option<Decimal>, pub max_discount: Option<Decimal>,
  pub priority: i32, pub created_at: DateTime<Utc>, pub skip_tier_lines: bool }
pub struct CouponRule { /* mismos campos que PromoRule menos priority/created_at/skip_tier_lines, más code */ }
pub fn price_cart(lines: &[PriceLine], promos: &[PromoRule], coupon: Option<&CouponRule>, tip: Decimal) -> Result<PricedCart, CouponError>
```

### Algoritmo de `price_cart` (implementa exactamente esto)

Notación: `money` redondea a 2 decimales (mitad hacia afuera) y `money_floor` trunca a 2 decimales.

1. **Bruto por línea:** `gross_i = money(qty_i × unit_price_i)`. Subtotal bruto: `G = Σ gross_i`.
2. **Descuento manual:** `manual_i = money(gross_i × manual_pct_i / 100)`, o 0 si no hay porcentaje.
3. **Promoción por línea.** Una línea es elegible si no es libre (`custom = false`) y no tiene cambio de precio (`overridden = false`). Para cada promoción `p` de alcance `Producto` o `Categoria`:
   - `p` aplica a la línea `i` si se cumple todo esto:
     - la línea es elegible;
     - el producto está en `p.product_ids` (alcance `Producto`) o la categoría en `p.category_ids` (alcance `Categoria`);
     - `qty_i >= p.min_qty`, si `min_qty` existe;
     - `G >= p.min_subtotal`, si existe;
     - no se da el caso de que `p.skip_tier_lines` sea verdadero y la línea tenga `tier_id`.
   - Monto: `Porcentaje` → `money((gross_i − manual_i) × value / 100)`; `Fijo` → `money(value × qty_i)`. Se limita a `gross_i − manual_i`.
   - Recorre las líneas por `index` ascendente. Para cada una, elige **una sola** promoción, la de mayor monto. Empates, en orden: mayor `priority`, luego `created_at` más antiguo, luego `id` menor.
   - `max_discount` es por promoción y por venta: lleva un acumulado y recorta el monto para no pasarse. Si el monto recortado queda en 0, la línea no recibe esa promoción y se evalúa la siguiente mejor.
   - `promo_i` es el monto elegido; `promotion_id_i` es su id.
4. **Neto 1:** `net1_i = gross_i − manual_i − promo_i`.
5. **Promoción de pedido.** Líneas elegibles: las no cambiadas de precio, quitando las que tienen `tier_id` si `skip_tier_lines` está activo. `N1 = Σ net1_i` de esas líneas.
   - Una promoción de alcance `Pedido` aplica si `N1 > 0` y `G >= min_subtotal` (si existe).
   - Monto: `Porcentaje` → `money(N1 × value / 100)`; `Fijo` → `min(value, N1)`; recortado por `max_discount`.
   - Elige una sola, con las mismas reglas de desempate del paso 3.
   - Reparte el monto entre las líneas elegibles con el algoritmo de **reparto** de abajo, usando `net1_i` como peso. El resultado es `order_i`.
6. **Neto 2:** `net2_i = net1_i − order_i`.
7. **Cupón**, si viene. Líneas elegibles: las no cambiadas de precio y, según el alcance, todas (`Pedido`) o solo las del producto o categoría.
   - `B = Σ net2_i` de las elegibles. Si `B = 0`: error `COUPON_NOT_APPLICABLE`.
   - Si `G < min_subtotal`: error `COUPON_MIN_SUBTOTAL`.
   - Monto: `Porcentaje` → `money(B × value / 100)`; `Fijo` → `min(value, B)`; recortado por `max_discount`.
   - Se reparte con `net2_i` como peso. El resultado es `coupon_i`.
8. **Resultado por línea:**
   - `line_total_i = gross_i − manual_i − promo_i − order_i − coupon_i`;
   - `tax_i = money(line_total_i × tax_rate_i)`.
9. **Totales:**
   - `subtotal = Σ line_total`;
   - `tax_total = Σ tax`;
   - `discount_total = Σ (manual + promo + order + coupon)`;
   - `total = subtotal + tax_total + tip`.

**Algoritmo de reparto** (función pura `allocate(amount, weights) -> Vec<Decimal>`):
1. `W = Σ weights`. Si `W = 0`, todo es 0.
2. A cada línea le toca `share_i = money_floor(amount × w_i / W)`.
3. Calcula `resto = amount − Σ share_i`, en centavos.
4. Recorre las líneas de mayor a menor peso (empate: menor índice) sumando 0,01 a cada una, dando vueltas, hasta repartir todo el resto. Nunca pases el tope de una línea: `share_i <= w_i`.

### Pruebas unitarias en `pricing.rs` (todas obligatorias)
- `allocate_sums_exactly`: reparte 100 entre pesos 1, 1 y 1. Debe dar 33,34 + 33,33 + 33,33.
- `allocate_respects_caps`.
- `line_promo_picks_best`: dos promociones que aplican a la misma línea; gana la de mayor monto.
- `promo_tie_breaks_by_priority`.
- `max_discount_caps_across_lines`.
- `skip_tier_lines_excludes_tier_line`.
- `overridden_line_gets_no_discounts`.
- `order_promo_requires_min_subtotal`.
- `coupon_after_promotions_on_net`.
- `discount_never_exceeds_gross` (prueba con muchos datos aleatorios fijos: 200 casos, generados con una semilla fija sin crates nuevos).
- `tax_on_discounted_base`: una línea de $10.000 con IVA del 19 % y 10 % de descuento da un impuesto de $1.710.

### Endpoint de cotización
`POST /v1/sales/quote` (permiso `pos:sell`).
- Cuerpo `QuoteRequest`: el mismo `SaleRequest` sin `id`, `payments` ni `quoted_total`.
- **No escribe nada.** Hace las mismas validaciones y cálculos que la venta (F1.4 a F1.10), pero sin validar el turno de caja y sin tocar el stock.

Respuesta:

```json
{
  "lines": [{"index":0,"product_id":"…","name":"Arroz Diana 500 g","qty":"2","units_per_package":"1",
             "list_price":"2500.00","unit_price":"2500.00","gross":"5000.00",
             "discount":{"manual":"0.00","promotion":"250.00","order":"0.00","coupon":"0.00","total":"250.00"},
             "promotion_id":"…|null","line_total":"4750.00","tax_rate":"0.0500","tax_amount":"237.50"}],
  "subtotal_gross":"5000.00","discount_total":"250.00","subtotal":"4750.00","tax_total":"237.50",
  "tip_amount":"0.00","total":"4987.50",
  "promotions":[{"id":"…","name":"Arroz 5 %","amount":"250.00"}],
  "coupon": null,
  "coupon_error": null
}
```

Diferencias con la venta:
- Si el cupón falla, la cotización responde **200** con `coupon_error: {code, message}` y calcula sin cupón.
- En la venta, el mismo fallo es un 422 con ese código.

### Venta (`checkout_tx`)
`SaleRequest` agrega `quoted_total: Decimal` (obligatorio). El nuevo orden de `checkout_tx`:
1. Reclama el `id` (igual que hoy).
2. Carga los ajustes.
3. Valida cliente o alias (F1.4).
4. Toma el turno (igual que hoy).
5. Carga el contexto de precios con `pricing::load_context_tx`: precios, categorías, tarifas, promociones vigentes y cupón.
6. Revisa los permisos de cada línea (F1.6, F1.9).
7. Ejecuta `price_cart`.
8. Si `money(quoted_total) != total`: 409 `PRICE_CHANGED`, con `data` = la respuesta de cotización completa.
9. Inserta las líneas con todas las columnas.
10. Descuenta el stock con `inventory::sell_tx`, solo con las líneas de producto y con `qty × units_per_package`.
11. Guarda cliente, alias, propina y mesero, y llama a `recalc_totals`.
12. Registra los usos de promociones y cupón (F1.7, F1.8).
13. Registra los pagos (igual que hoy).
14. Asigna el consecutivo y encola el documento fiscal (igual que hoy).
15. Marca la orden `COBRADA` (igual que hoy).
16. Registra la auditoría de los cambios de precio (F1.9).

`discount_pct` (que agregó la fase 0) se queda como `manual_pct`:
- exige el permiso `pos:discount`; si falta: 403;
- debe estar entre 0 y `settings.pos.max_discount_pct`; si no: 422 `DISCOUNT_ABOVE_MAX`, con `data: {max}`.

### Pruebas (`f1_pricing_db`)
- **[P]** Una venta con `quoted_total` distinto al calculado: 409 `PRICE_CHANGED`. El `data.total` trae el total correcto.
- **[P]** Una venta con descuento manual del 10 % guarda `discount_manual` y `line_total` correctos, y `orders.total_amount = subtotal + impuesto + propina`.
- **[P]** Un cajero sin `pos:discount` que manda un descuento: 403.
- Actualiza las pruebas de venta que ya existen para que manden `quoted_total`, sin cambiar lo que comprueban.

---

## F1.6 Tarifas y presentaciones

### Migración

```sql
create table public.price_tiers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  kind text not null check (kind in ('CLIENTE','PRESENTACION')),
  adjustment text not null check (adjustment in ('NINGUNO','DESCUENTO_PCT','RECARGO_PCT','DESCUENTO_FIJO','RECARGO_FIJO','PRECIO_FIJO')),
  value numeric(14,4) not null default 0 check (value >= 0),
  units_per_package numeric(14,4) check (units_per_package > 0),
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, name),
  check (kind = 'PRESENTACION' or units_per_package is null),
  check (adjustment <> 'DESCUENTO_PCT' or value <= 100),
  check (adjustment <> 'RECARGO_PCT' or value <= 1000)
);
create table public.product_tier_prices (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  tier_id uuid not null,
  price numeric(14,2) check (price >= 0),
  units_per_package numeric(14,4) check (units_per_package > 0),
  barcode varchar(50),
  primary key (product_id, tier_id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade,
  foreign key (tenant_id, tier_id) references public.price_tiers (tenant_id, id)
);
create unique index product_tier_prices_barcode_idx on public.product_tier_prices (tenant_id, barcode) where barcode is not null;
-- RLS a nivel de negocio en las dos tablas, más grants.

alter table public.customers add constraint customers_tier_fkey
  foreign key (tenant_id, price_tier_id) references public.price_tiers (tenant_id, id);
alter table public.order_items add constraint order_items_tier_fkey
  foreign key (tenant_id, tier_id) references public.price_tiers (tenant_id, id);
```

### Resolución del precio de una línea con tarifa `T`, producto `P` y precio base `B` de la sucursal
(`B` es el `sellable_price` actual.) Implementa la función pura `pricing::resolve_tier(B, &tier, override: Option<&ProductTierPrice>) -> (unit_price, units)`:

1. **Unidades `U`:**
   - si `T.kind = CLIENTE`, `U = 1`;
   - si `T.kind = PRESENTACION`, `U` es `ptp.units_per_package`; si no existe, `T.units_per_package`; y si tampoco, la línea se rechaza con 422 `TIER_MISSING_UNITS`.
2. Si `ptp.price` existe: `unit_price = ptp.price` (precio de la presentación completa).
3. Si no, `base = B × U` y se aplica `T.adjustment`:

   | Ajuste | Precio |
   |---|---|
   | `NINGUNO` | `base` |
   | `DESCUENTO_PCT` | `base × (1 − v/100)` |
   | `RECARGO_PCT` | `base × (1 + v/100)` |
   | `DESCUENTO_FIJO` | `base − v` |
   | `RECARGO_FIJO` | `base + v` |
   | `PRECIO_FIJO` | `v` |

4. `unit_price = money(max(0, resultado))`.
5. `list_price = money(B × U)`.

Si la línea no tiene tarifa: `unit_price = list_price = B` y `U = 1`.

### Reglas en la venta
`SaleItem` agrega `tier_id: Option<Uuid>`. Valida en este orden:
1. La tarifa no existe o está inactiva: 422 `TIER_INVALID`.
2. Tarifa de tipo `CLIENTE`: se permite si `tier_id == customer.price_tier_id`, o si el usuario tiene `pos:discount`. Si no se cumple ninguna: 403.
3. Tarifa de tipo `PRESENTACION`: siempre se permite.
4. Una línea libre con tarifa: 422 `TIER_INVALID`.

**El servidor no aplica tarifas por su cuenta.** Es el POS el que pone `tier_id` en las líneas (ver el frontend).

### Endpoints (archivo nuevo `src/tiers.rs`)

| Método y ruta | Permiso | Detalle |
|---|---|---|
| `GET /v1/price-tiers` | `pos:sell` | Activas e inactivas, ordenadas por `kind` y `name`. |
| `POST /v1/price-tiers` | `settings:write` | `{name, kind, adjustment, value, units_per_package?}` |
| `PUT /v1/price-tiers/{id}` | `settings:write` | Mismo cuerpo más `active`. No se puede cambiar `kind`: 422 `TIER_KIND_IMMUTABLE`. |
| `GET /v1/products/{id}/tier-prices` | `pos:sell` | `[{tier_id, price, units_per_package, barcode}]` |
| `PUT /v1/products/{id}/tier-prices` | `settings:write` | Cuerpo: lista completa `[{tier_id, price?, units_per_package?, barcode?}]`. Reemplaza todas las filas del producto. |

- Código de barras repetido, o igual al `products.barcode` de otro producto: 409 `BARCODE_TAKEN`.
- No hay `DELETE`: una tarifa se desactiva con `active = false`.

**Lector de código de barras.** `GET /v1/catalog` agrega a cada producto `tier_barcodes: [{tier_id, barcode}]`, para que el POS reconozca presentaciones.

### Frontend
- **Pantalla nueva** `/configuracion/tarifas`:
  - registro `s("33", "tarifas", "Tarifas y presentaciones", "/configuracion/tarifas", "configuracion", "admin", "react")`;
  - componente `components/configuracion/tiers-screen.tsx`;
  - permiso `settings:write`;
  - tabla con Nombre, Tipo, Ajuste (texto legible, por ejemplo "−10 %" o "Precio fijo $45.000"), Unidades y Estado;
  - diálogo de creación y edición.
- **Precios por tarifa de un producto.** La ficha de producto no existe todavía (fase 2). Por ahora agrega en `/inventario`, en el menú de cada fila, la acción "Precios por tarifa", que abre el diálogo `components/inventario/tier-prices-dialog.tsx`: una fila por tarifa activa, con los campos precio, unidades y código de barras.
- **POS:**
  - al asignar un cliente con `price_tier_id`, todas las líneas sin tarifa reciben esa tarifa. Al quitar el cliente, se les quita esa tarifa;
  - cada línea tiene el menú "Presentación", con las tarifas `PRESENTACION` que aplican al producto, es decir, las que tienen `units_per_package` definido en la tarifa o en el producto;
  - un código de barras que coincide con `tier_barcodes` agrega la línea con esa tarifa.
- **Mock:** las tarifas "Mayorista" (CLIENTE, −8 %), "Paca x12" (PRESENTACION, 12 unidades, −5 %) y "Bulto 50 kg" (PRESENTACION, 50 unidades, sin ajuste).

### Pruebas
- Unitaria `resolve_tier_order`: el precio fijo por producto gana sobre la regla; la regla se aplica sobre `B × U`; el precio nunca queda negativo.
- **[P]** (`f1_tiers`) Una venta de 2 "Paca x12" descuenta 24 unidades del stock.
- **[P]** Un cajero sin `pos:discount` que usa la tarifa "Mayorista" con un cliente que no la tiene: 403.
- **[P]** Un código de barras de presentación igual al de otro producto: 409 `BARCODE_TAKEN`.

---

## F1.7 Promociones

### Migración

```sql
create table public.promotions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,                                   -- null = todas las sucursales
  name text not null check (length(trim(name)) between 1 and 60),
  kind text not null check (kind in ('PORCENTAJE','FIJO')),
  value numeric(14,4) not null check (value > 0),
  scope text not null check (scope in ('PEDIDO','PRODUCTO','CATEGORIA')),
  min_qty numeric(14,4) check (min_qty > 0),
  min_subtotal numeric(14,2) check (min_subtotal > 0),
  max_discount numeric(14,2) check (max_discount > 0),
  starts_at timestamptz not null,
  ends_at timestamptz,
  status text not null default 'BORRADOR' check (status in ('BORRADOR','ACTIVA','PAUSADA')),
  priority integer not null default 0,
  usage_limit integer check (usage_limit > 0),
  usage_count integer not null default 0,
  per_customer_limit integer check (per_customer_limit > 0),
  skip_tier_lines boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  check (ends_at is null or ends_at > starts_at),
  check (kind <> 'PORCENTAJE' or value <= 100),
  check (scope <> 'PEDIDO' or min_qty is null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.promotion_targets (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  promotion_id uuid not null,
  product_id uuid,
  category_id uuid references public.product_categories(id),
  foreign key (tenant_id, promotion_id) references public.promotions (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  check ((product_id is null) <> (category_id is null))
);
create table public.promotion_redemptions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  promotion_id uuid not null,
  order_id uuid not null,
  customer_id uuid,
  amount numeric(14,2) not null check (amount > 0),
  created_at timestamptz not null default now(),
  unique (promotion_id, order_id),
  foreign key (tenant_id, promotion_id) references public.promotions (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade
);
-- RLS a nivel de negocio en las 3 tablas, más grants.
```

### Estado visible
Se calcula y no se guarda. Función pura `effective_status(status, starts_at, ends_at, now)`:

| Condición | Estado visible |
|---|---|
| `status` es `BORRADOR` o `PAUSADA` | el mismo |
| `ACTIVA` y `now < starts_at` | `PROGRAMADA` |
| `ACTIVA` y `ends_at <= now` | `VENCIDA` |
| `ACTIVA` en cualquier otro caso | `ACTIVA` |

### Cuáles entran al motor
`load_context_tx` carga las promociones que cumplen todo esto:
- estado visible `ACTIVA`;
- `branch_id` nulo o igual a la sucursal de la venta;
- `usage_limit` nulo o `usage_count < usage_limit`;
- si tiene `per_customer_limit`: la venta tiene cliente y ese cliente tiene menos de `per_customer_limit` usos en `promotion_redemptions`. Si la venta no tiene cliente, **no entra**.

Todas las promociones son automáticas. No hay promociones manuales.

### Registro de uso (paso 12 de la venta)
Para cada promoción aplicada, con su monto total en la venta:
1. Ejecuta `update promotions set usage_count = usage_count + 1 where id = $1 and (usage_limit is null or usage_count < usage_limit) returning id`.
2. Si no devuelve fila: 409 `PROMOTION_EXHAUSTED`, con `data: {promotion_id}`.
3. Inserta en `promotion_redemptions`.

### Endpoints (archivo nuevo `src/promotions.rs`, permiso `settings:write`, salvo la lista)

| Método y ruta | Detalle |
|---|---|
| `GET /v1/promotions?status=` | Permiso `pos:sell`. Cada promoción incluye `effective_status`, `targets` y `usage_count`. |
| `POST /v1/promotions` | `{branch_id?, name, kind, value, scope, min_qty?, min_subtotal?, max_discount?, starts_at, ends_at?, priority, usage_limit?, per_customer_limit?, skip_tier_lines, product_ids: [], category_ids: []}`. Se crea en `BORRADOR`. |
| `PUT /v1/promotions/{id}` | Mismo cuerpo. Solo en `BORRADOR` o `PAUSADA`; si no: 409 `PROMOTION_NOT_EDITABLE`. |
| `POST /v1/promotions/{id}/activate` | De `BORRADOR` o `PAUSADA` a `ACTIVA`. |
| `POST /v1/promotions/{id}/pause` | De `ACTIVA` a `PAUSADA`. |

- Alcance `PRODUCTO` sin `product_ids`, o `CATEGORIA` sin `category_ids`: 422 `PROMOTION_INVALID`.
- Alcance `PEDIDO` con objetivos: 422 `PROMOTION_INVALID`.
- No hay `DELETE`.

### Frontend
- **Pantalla** `/marketing/promociones`:
  - registro `s("34", "promociones", "Promociones", "/marketing/promociones", "marketing", "admin", "react")`;
  - grupo nuevo del menú, "Marketing";
  - componente `components/marketing/promotions-screen.tsx`.
- **Contenido:**
  - pestañas por estado visible;
  - tabla con Nombre, Descuento (por ejemplo "10 %" o "$2.000 c/u"), Alcance, Vigencia, Usos (`usage_count/usage_limit`) y Estado;
  - formulario en un panel lateral con todos los campos del `POST`. Los selectores de productos y categorías tienen búsqueda.
- **POS:** debajo de cada línea con promoción, la etiqueta verde "<nombre> −$X". El total muestra la fila "Descuentos".
- **Mock:** "Arroz 5 %" (PRODUCTO), "Lleva 3 gaseosas" (CATEGORIA, `min_qty` 3, 10 %) y "Compras desde $100.000" (PEDIDO, $5.000 fijo).

### Pruebas (`f1_promotions`)
- **[P]** Una promoción `PAUSADA`, una `PROGRAMADA` (futura) y una `VENCIDA` no se aplican.
- **[P]** Con `usage_limit = 1`, la segunda venta ya no recibe el descuento. El `quoted_total` de la cotización ya lo refleja.
- **[P]** `per_customer_limit = 1`: el mismo cliente no la recibe dos veces y otro cliente sí.
- **[P]** Una venta sin cliente no recibe una promoción con `per_customer_limit`.

---

## F1.8 Cupones

### Migración
- `coupons`: mismas columnas que `promotions`, con estos cambios:
  - **sin** `priority` ni `skip_tier_lines`;
  - **con** `code text not null check (code ~ '^[A-Z0-9]{4,20}$')` y `unique (tenant_id, code)`;
  - `status` admite solo `'ACTIVO','INACTIVO'`.
- `coupon_targets`: igual que `promotion_targets`.
- `coupon_redemptions`: igual que `promotion_redemptions`, más `unique (order_id)` (un cupón por venta).
- RLS a nivel de negocio y grants.

### Venta
`SaleRequest` y `QuoteRequest` agregan `coupon_code: Option<String>`, que se normaliza a mayúsculas sin espacios a los lados. Validaciones antes del motor, en este orden. El primer error que ocurra es el que se devuelve, siempre como 422:

| Condición | Código |
|---|---|
| No existe para el negocio | `COUPON_NOT_FOUND` |
| `status = INACTIVO` o `branch_id` distinto | `COUPON_INACTIVE` |
| `now < starts_at` | `COUPON_NOT_STARTED` |
| `ends_at <= now` | `COUPON_EXPIRED` |
| `usage_limit` alcanzado | `COUPON_EXHAUSTED` |
| Tiene `per_customer_limit` y la venta no tiene cliente | `COUPON_REQUIRES_CUSTOMER` |
| El cliente ya alcanzó `per_customer_limit` | `COUPON_CUSTOMER_LIMIT` |
| Lo que devuelve el motor | `COUPON_MIN_SUBTOTAL` o `COUPON_NOT_APPLICABLE` |

El registro de uso funciona igual que en las promociones. Si alguien se adelanta y el cupón se agota: 409 `COUPON_EXHAUSTED`.

### Endpoints (archivo nuevo `src/coupons.rs`)

| Método y ruta | Permiso |
|---|---|
| `GET /v1/coupons` | `settings:write` |
| `POST /v1/coupons` | `settings:write` |
| `PUT /v1/coupons/{id}` | `settings:write` |
| `POST /v1/coupons/{id}/toggle` | `settings:write` |

Código repetido: 409 `COUPON_CODE_TAKEN`.

### Frontend
- **Pantalla** `/marketing/cupones`:
  - registro `s("35", "cupones", "Cupones", "/marketing/cupones", "marketing", "admin", "react")`;
  - componente `components/marketing/coupons-screen.tsx`;
  - el mismo patrón de tabla y formulario que las promociones.
- **POS:** en el carrito, el campo "Cupón" con el botón "Aplicar".
  - Si la cotización devuelve `coupon_error`, muestra en rojo el texto de su código, tomado de la tabla de abajo.
  - Si el cupón aplica, muestra el chip "CODIGO −$X" con una "x" para quitarlo.
- **Mock:** `BIENVENIDA10` (10 % al pedido, mínimo $30.000, tope $10.000).

### Pruebas (`f1_coupons`)
- **[P]** Cada uno de los 9 códigos de rechazo aparece al menos una vez.
- **[P]** Con una promoción de pedido del 10 % y un cupón del 10 %, el descuento total nunca supera el subtotal bruto, y el cupón se calcula sobre el neto después de la promoción.
- **[P]** El IVA se calcula sobre la base ya descontada: compara `tax_amount` con `money(line_total × tasa)`.

---

## F1.9 Cambio de precio e ítem libre

### Migración

```sql
alter table public.products add column allow_price_override boolean not null default true;
```

### Venta
`SaleItem` agrega `override_reason: Option<String>`, `custom_name: Option<String>` y `custom_tax: Option<String>`. Los valores permitidos de `custom_tax` son `IVA19`, `IVA5`, `INC8` y `EXENTO`.

**Cambio de precio.** `unit_price_override` es el precio por unidad vendida, por presentación si hay tarifa. Sea `resolved` el precio que resulta de F1.6.
1. Si `|unit_price_override − resolved| < 0,01`, se ignora el cambio.
2. Si no:
   1. `settings.pos.allow_price_override` es falso: 422 `PRICE_OVERRIDE_DISABLED`.
   2. El usuario no tiene `pos:price_override`: 403.
   3. `products.allow_price_override` es falso: 422 `PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT`.
   4. `override_reason` sin espacios a los lados debe tener entre 3 y 120 caracteres. Si no: 422 `PRICE_OVERRIDE_REASON_REQUIRED`.
   5. `unit_price = money(unit_price_override)`, `overridden = true` y se guarda `price_override_reason`.
   6. Auditoría con la acción `CAMBIO_PRECIO`, la entidad `order_items` y el id de la línea, `before: {price: resolved}`, `after: {price, reason}`.

**Ítem libre:** `product_id` nulo y `custom_name` presente.
1. Sin `pos:custom_item`: 403.
2. `custom_name` debe tener entre 1 y 80 caracteres; `unit_price_override` debe existir y ser mayor que 0; `custom_tax` debe estar en la lista. Si falla cualquiera: 422 `CUSTOM_ITEM_INVALID`.
3. Con `tier_id` o `discount_pct`: 422 `CUSTOM_ITEM_INVALID`.
4. `unit_price = list_price = unit_price_override`; la tasa sale de `catalog::tax_rate_for(custom_tax)`; `EXENTO` vale 0; no toca el stock; `custom = true`.

Una línea que no trae ni `product_id` ni `custom_name`, o que trae los dos: 422 `CUSTOM_ITEM_INVALID`.

### Frontend
- Menú de cada línea → "Cambiar precio". Solo se ve si están `can("pos:price_override")` y el ajuste de la sucursal.
- El diálogo pide el nuevo precio (`MoneyInput`) y el motivo (obligatorio).
- La línea cambiada muestra el precio original tachado.
- Botón "Ítem libre" junto al buscador (solo con `pos:custom_item`). Abre un diálogo con Nombre, Precio sin IVA, Impuesto (selector de 4 opciones) y Cantidad.

### Pruebas (`f1_override`)
- **[P]** Con el ajuste apagado: 422 `PRICE_OVERRIDE_DISABLED`.
- **[P]** Con el ajuste encendido y sin el permiso: 403.
- **[P]** Con el producto bloqueado: 422 `PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT`.
- **[P]** Sin motivo: 422 `PRICE_OVERRIDE_REASON_REQUIRED`.
- **[P]** El cambio de precio queda en `audit_log`.
- **[P]** El ítem libre no mueve el stock y calcula bien su impuesto.

---

## F1.10 Propinas

### Venta
`SaleRequest` agrega `tip_waiter_id: Option<Uuid>`.
1. `tip_amount > 0` y `settings.tips.enabled` es falso: 422 `TIPS_DISABLED`.
2. `tip_waiter_id` sin `tip_amount > 0`: 422 `TIP_INVALID`.
3. `tip_waiter_id` sin membresía activa en la sucursal: 422 `TIP_INVALID`.
4. Se guarda en `orders.tip_amount` y `orders.waiter_id`. **Corrige** el `waiter_id = null` que hoy pone el paso final de la venta: ahora guarda `tip_waiter_id`.

La propina no lleva impuesto: ya se suma después del impuesto en `recalc_totals`. No cambies eso.

### Frontend
En el diálogo de cobro, solo si `tips.enabled`:
- tres botones: "Sin propina", "Sugerida (X %)" con `tips.suggested_pct` sobre el `subtotal`, y "Otro valor";
- selector de mesero, con los usuarios de la sucursal de rol `MESERO`.

La propina vuelve a cotizarse con la API.

### Pruebas (`f1_tips`)
- **[P]** Con las propinas apagadas: 422 `TIPS_DISABLED`.
- **[P]** La propina entra en `total_amount`, no en `subtotal` ni en `inc_amount`.
- **[P]** La propina aparece en `tips_total` del turno.

---

## F1.11 Devoluciones

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Devolucion de cliente', 'DEVOLUCION', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'MOV'
on conflict (padre_id, tima_codigo) do nothing;

create table public.refunds (
  id uuid primary key,                               -- generado por el cliente (idempotencia)
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid not null,
  prefix text not null,
  number bigint not null,
  status text not null check (status in ('PENDIENTE','APROBADA','RECHAZADA')),
  method text not null check (method in ('EFECTIVO','ORIGINAL')),
  original_method text,                              -- EFECTIVO|DATAFONO|QR cuando method = ORIGINAL
  reversal_reference text,
  reason text not null check (length(trim(reason)) between 3 and 200),
  subtotal numeric(14,2) not null,
  tax_amount numeric(14,2) not null,
  total numeric(14,2) not null check (total > 0),
  cash_session_id uuid,
  requested_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (branch_id, prefix, number),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
create table public.refund_items (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  refund_id uuid not null,
  order_item_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  disposition text not null check (disposition in ('REINTEGRAR','MERMA','SIN_DEVOLUCION')),
  line_total numeric(14,2) not null,
  tax_amount numeric(14,2) not null,
  foreign key (tenant_id, refund_id) references public.refunds (tenant_id, id) on delete cascade,
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id)
);
alter table public.orders add column refunded_amount numeric(14,2) not null default 0;

alter table public.fiscal_documents add column refund_id uuid;
alter table public.fiscal_documents drop constraint fiscal_documents_order_id_doc_type_key;
create unique index fiscal_documents_order_doc_idx on public.fiscal_documents (order_id, doc_type) where refund_id is null;
create unique index fiscal_documents_refund_idx on public.fiscal_documents (refund_id) where refund_id is not null;
-- RLS: refunds con in_scope(tenant_id, branch_id); refund_items a nivel de negocio. Grants.
```

Antes de escribir el `drop constraint`, comprueba el nombre real de la restricción con `\d fiscal_documents` dentro del contenedor de pruebas. Si es otro, usa el real y anótalo en el reporte.

### Endpoints (archivo nuevo `src/refunds.rs`)

| Método y ruta | Permiso | Cuerpo y respuesta |
|---|---|---|
| `POST /v1/refunds` | `pos:sell` | `{id, order_id, method, reason, reversal_reference?, items: [{order_item_id, qty, disposition}]}` → `201 Refund` (o `200` con la misma respuesta si es un reintento idéntico) |
| `POST /v1/refunds/{id}/approve` | `pos:refund` | — → `200 Refund` |
| `POST /v1/refunds/{id}/reject` | `pos:refund` | `{reason}` → `200 Refund` |
| `GET /v1/refunds?branch_id=&status=` | `pos:refund` | `[Refund]` |
| `GET /v1/orders/{id}/refunds` | `pos:sell` | `[Refund]` |

`Refund = {id, order_id, receipt_number (de la venta), number: "DEV-000001", status, method, original_method, reversal_reference, reason, subtotal, tax_amount, total, items:[{order_item_id, name, qty, disposition, line_total, tax_amount}], requested_by_name, decided_by_name, decided_at, reject_reason, created_at}`

### Creación (`create_refund_tx`), en este orden
1. **Idempotencia:** si el `id` ya existe y su `client_request` es igual, devuelve la devolución existente. Si es distinto: 409 `IDEMPOTENCY_CONFLICT`.
2. Bloquea la orden con `select … from orders where id = $1 for update`. Debe estar en estado `COBRADA` o `CERRADA` y pertenecer a la sucursal del usuario. Si no: 409 `REFUND_ORDER_INVALID`.
3. `items` no puede venir vacío ni repetir `order_item_id`. Si no: 422 `REFUND_INVALID`.
4. Para cada ítem:
   - `devolvible = order_items.qty − Σ qty` de `refund_items` de esa línea en devoluciones `PENDIENTE` o `APROBADA`. Si `qty > devolvible`: 422 `REFUND_QTY_EXCEEDED`, con `data: {order_item_id, refundable}`.
   - Una línea libre solo admite `disposition = SIN_DEVOLUCION`. Si no: 422 `REFUND_INVALID`.
   - Monto: si `qty == devolvible` (devuelve todo lo que queda), `line_total = oi.line_total − Σ line_total ya devuelto` y lo mismo para `tax_amount`. Si no, `line_total = money(oi.line_total × qty / oi.qty)` y `tax_amount = money(oi.tax_amount × qty / oi.qty)`.
5. **Método:**
   - `EFECTIVO`: siempre se permite.
   - `ORIGINAL`: la venta debe tener **un solo** registro en `payments`. Si tiene más: 422 `REFUND_ORIGINAL_MIXED`. Se guarda `original_method` con el método de ese pago. Si es `EFECTIVO`, la devolución se trata como `EFECTIVO`. Si es `DATAFONO` o `QR`, `reversal_reference` es obligatorio (de 3 a 60 caracteres); si falta: 422 `REFUND_REFERENCE_REQUIRED`.
6. El número se toma de `sequences::next_tx(tx, scope, branch, "DEVOLUCION")`.
7. **Estado inicial:**
   - si `settings.pos.refund_requires_approval` es falso, o el usuario tiene `pos:refund`: inserta con estado `APROBADA` y ejecuta los efectos (abajo) en la misma transacción;
   - si no: `PENDIENTE`, sin efectos.
8. Auditoría: `DEVOLUCION_SOLICITADA`, y además `DEVOLUCION_APROBADA` si se aprobó al crearla.

### Efectos al aprobar (`apply_refund_effects_tx`), en la misma transacción
1. **Stock:** cada ítem `REINTEGRAR` de una línea con producto entra al inventario como `qty × units_per_package`. Usa el tipo de movimiento `DEVOLUCION` con `ref_type = 'REFUND'` y `ref_id = refund.id`. Agrega en `inventory.rs` la función `return_tx(tx, scope, branch, product_id, qty, refund_id)`, que reutiliza `replenish_ref_tx` con el tipo `DEVOLUCION`. `MERMA` y `SIN_DEVOLUCION` no tocan el stock.
2. **Caja:** si el dinero sale en efectivo (`method = EFECTIVO`, o `ORIGINAL` con `original_method = EFECTIVO`):
   - busca el turno abierto **del usuario que aprueba** en esa sucursal; si no tiene: 409 `REFUND_NEEDS_OPEN_SESSION`;
   - ejecuta `cash::record_movement_tx(..., Devolucion, total, "Devolución DEV-…", None, Some("REFUND"), Some(id))`. Si no hay efectivo suficiente, se propaga el 409 `CASH_INSUFFICIENT`;
   - guarda `cash_session_id`.
3. `orders.refunded_amount += total`.
4. **Fiscal:** si la venta tiene un `fiscal_documents` con `doc_type = 'POS'`, inserta una fila con `doc_type = 'NOTA_CREDITO'`, `refund_id`, `prefix` y `number` de la devolución, y estado `PENDIENTE`.
5. `decided_by`, `decided_at = now()` y `status = APROBADA`.

### Aprobar y rechazar
- **Aprobar:** bloquea la devolución con `for update`. Debe estar `PENDIENTE`; si no: 409 `REFUND_NOT_PENDING`. Ejecuta los efectos. Auditoría `DEVOLUCION_APROBADA`.
- **Rechazar:** debe estar `PENDIENTE`; `reason` debe tener entre 3 y 200 caracteres. Guarda `reject_reason`, `decided_by` y `decided_at`. Auditoría `DEVOLUCION_RECHAZADA`. No tiene efectos.

**Las devoluciones no tienen "deshacer".**

### Frontend
- En el detalle de la venta (F1.13), el botón "Devolver" se muestra si `can("pos:sell")` y queda algo por devolver. Abre `components/ordenes/refund-dialog.tsx` con:
  - una tabla de líneas con las columnas Producto, Vendido, Ya devuelto, A devolver (campo numérico de 0 al máximo) y Destino de la mercancía (selector: "Vuelve al inventario", "Merma", "No se recibe");
  - el método ("Efectivo" o "Mismo medio de pago"). Si la venta tiene pagos mixtos, solo aparece "Efectivo". Si el medio es datáfono o QR, aparece el campo "Referencia del reverso";
  - el motivo (obligatorio) y el total a devolver, calculado igual que el backend (redondeo con `Math.round` sobre centavos).
  - Al confirmar, si la devolución queda `PENDIENTE`, se muestra "Quedó pendiente de aprobación".
- **Pantalla** `/ordenes/devoluciones`:
  - registro `s("36", "devoluciones", "Devoluciones", "/ordenes/devoluciones", "ventas", "admin", "react")`;
  - permiso `pos:refund`;
  - pestañas Pendientes, Aprobadas y Rechazadas;
  - en Pendientes, botones "Aprobar" y "Rechazar" (este último pide el motivo).
- **Tirilla de devolución:** la misma `PrintArea`, con el título "DEVOLUCIÓN DEV-000001", la venta original, las líneas, el total y el método.

### Pruebas (`f1_refunds`)
- **[P]** No se puede devolver más de lo vendido menos lo ya devuelto (contando las pendientes): 422 `REFUND_QTY_EXCEEDED`.
- **[P]** Dos devoluciones parciales que suman todo dejan `Σ line_total = oi.line_total` exacto, sin diferencias de centavos. Prueba con `qty = 3` devolviendo 1 y luego 2, con un precio que no se divide exacto.
- **[P]** Con `refund_requires_approval = true` y un cajero sin `pos:refund`, la devolución queda `PENDIENTE`, sin mover stock ni caja. Al aprobarla, se mueven.
- **[P]** `REINTEGRAR` sube el stock; `MERMA` no lo cambia.
- **[P]** La devolución en efectivo crea un movimiento `DEVOLUCION` y baja el esperado del turno.
- **[P]** Sin turno abierto del aprobador: 409 `REFUND_NEEDS_OPEN_SESSION`.
- **[P]** Se crea un `fiscal_documents` de tipo `NOTA_CREDITO` con `refund_id`.
- **[P]** Un reintento idéntico devuelve la misma devolución; con un cuerpo distinto: 409 `IDEMPOTENCY_CONFLICT`.
- **[P]** `ORIGINAL` en una venta con pagos mixtos: 422 `REFUND_ORIGINAL_MIXED`.

---

## F1.12 Anulación de ventas

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Anulado', 'ANULADO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'DOCF'
on conflict (padre_id, tima_codigo) do nothing;
alter table public.orders add column void_reason text;
```

### Endpoint
`POST /v1/sales/{id}/void` (permiso `pos:refund`), con cuerpo `{reason}`. Pasos, en orden:
1. Bloquea la orden con `for update`. Debe estar `COBRADA`; si no: 409 `VOID_NOT_ALLOWED`, con `data: {why: "STATUS"}`.
2. La fecha local de `orders.created_at`, en la zona de `branches.timezone`, debe ser la de hoy en esa zona. Si no: 409 `VOID_NOT_ALLOWED`, con `data: {why: "NOT_TODAY"}`.
3. El turno de la venta debe seguir abierto. Si no: 409 `VOID_NOT_ALLOWED`, con `data: {why: "SESSION_CLOSED"}`.
4. La venta no puede tener devoluciones `PENDIENTE` ni `APROBADA`. Si las tiene: 409 `VOID_NOT_ALLOWED`, con `data: {why: "HAS_REFUNDS"}`.
5. Documento fiscal `POS`:
   - si está `ACEPTADO`: 409 `VOID_NOT_ALLOWED`, con `data: {why: "FISCAL_ACCEPTED"}` (hay que hacer una devolución);
   - si está `PENDIENTE` o `RECHAZADO`: pasa a `ANULADO`.
6. `reason` debe tener entre 3 y 200 caracteres. Si no: 422 `VOID_REASON_REQUIRED`.
7. **Stock:** cada línea con producto vuelve con `return_tx` (`qty × units_per_package`), con `ref_type = 'VOID'`.
8. **Promociones y cupón:**
   - `update promotions set usage_count = usage_count − 1` por cada uso de esta venta, y borra sus `promotion_redemptions`;
   - lo mismo con el cupón.
9. La orden queda en `CANCELADA`, con `cancelled_at = now()` y `void_reason`.
10. **Caja:** no se crea ningún movimiento. El esperado ya excluye los pagos de órdenes `CANCELADA` (F1.2).
11. Auditoría `VENTA_ANULADA`.

### Pruebas (`f1_void`)
- **[P]** Anular devuelve el stock, saca el efectivo del esperado y libera el uso del cupón.
- **[P]** Una venta de un turno cerrado no se anula: `SESSION_CLOSED`.
- **[P]** Una venta con devolución no se anula: `HAS_REFUNDS`.
- **[P]** Una venta con el documento fiscal `ACEPTADO` no se anula: `FISCAL_ACCEPTED`.

---

## F1.13 Historial de ventas (pantalla 04 `/ordenes`)

### Endpoints
`GET /v1/sales` (permiso `cash:history`). Parámetros:

| Parámetro | Detalle |
|---|---|
| `branch_id` | Obligatorio. |
| `from`, `to` | Fechas `YYYY-MM-DD` en la zona de la sucursal; ambas inclusivas. Por defecto, hoy. |
| `cashier_id` | Opcional. |
| `method` | `EFECTIVO`, `DATAFONO` o `QR`. |
| `status` | `COBRADA` o `CANCELADA`. |
| `q` | Número de comprobante. Acepta `POS-000123` o `123`. |
| `page` | Desde 1. |
| `page_size` | Por defecto 50 y máximo 100. |

- Si el usuario **no** tiene `reports:read`, solo ve las ventas donde `payments.created_by = él`. El parámetro `cashier_id` se ignora.
- Respuesta: `{items: [{id, receipt_number, created_at, cashier_name, customer_label (nombre del cliente, alias o "Consumidor final"), total, refunded_amount, methods: ["EFECTIVO", …], status, fiscal_status}], total_count, totals: {count, amount}}`.
- Orden: `created_at` descendente.

`GET /v1/sales/{id}` (permiso `cash:history`, con la misma restricción de dueño de la venta) devuelve `SaleResponse`, más `refunds: [Refund]`, `customer` y `cashier_name`.

### Frontend
- Pantalla 04 en `app/(admin)/ordenes/page.tsx`, componente `components/ordenes/sales-screen.tsx`. Sigue `design/specs/04-ordenes.md` en lo visual. Si la especificación pide algo que no está en este documento (por ejemplo, exportar), **no lo hagas**.
- Filtros arriba: rango de fechas, cajero (solo con `reports:read`), método, estado y número.
- Tabla paginada con las columnas Número, Hora, Cliente, Cajero, Métodos, Total, Devuelto y Estado (insignias "Cobrada", "Anulada" y "Con devolución").
- Al hacer clic se abre un panel lateral con:
  - las líneas (nombre, cantidad, precio, descuento y total);
  - los pagos, los impuestos y la propina;
  - el estado fiscal;
  - las devoluciones;
  - los botones "Reimprimir", "Devolver" (F1.11) y "Anular". "Anular" solo se ve con `pos:refund` y pide el motivo.
- Cambia el estado de la pantalla 04 a `"react"` en `screens.ts`.

### Prueba (`f1_sales_history`)
- **[P]** Un cajero sin `reports:read` solo ve sus propias ventas, aunque mande `cashier_id` de otro.

---

## F1.14 POS: cotización en vivo, atajos, espera, efectivo rápido y lector

Todo en `components/pos/`.

1. **Cotización en vivo.** Cada cambio del carrito (líneas, cantidades, tarifa, cliente, cupón, descuento o propina) llama a `api.sales.quote` con 300 ms de espera, cancelando la llamada anterior. El panel de totales muestra **solo** lo que devuelve la cotización. Mientras carga, se ve el total anterior con opacidad del 50 %.
2. **Cobro.** La venta envía `quoted_total = quote.total`. Ante `PRICE_CHANGED`, actualiza la cotización con `data`, muestra el aviso "Los precios cambiaron. Revisá el total." y deja el diálogo de cobro abierto.
3. **Atajos de teclado.** Solo funcionan con la pantalla del POS activa y sin otro diálogo abierto, salvo `Esc`:

   | Tecla | Acción |
   |---|---|
   | F2 | Pone el foco en el buscador |
   | F4 | Abre el diálogo de cliente |
   | F6 | Descuento a la línea seleccionada (con `pos:discount`) |
   | F8 | Pone la venta en espera |
   | F9 | Abre el cobro |
   | F10 | Dentro del cobro: efectivo exacto |
   | Esc | Cierra el diálogo abierto |

   Llama a `preventDefault` en F1–F12 para que el navegador no actúe.
4. **Escribir para buscar.** Si el foco no está en un campo de texto y se presiona una letra o un número, el foco pasa al buscador con esa tecla ya escrita.
5. **Lector de código de barras.** Se detecta una secuencia de 6 o más caracteres con menos de 50 ms entre teclas, terminada en `Enter`. Se busca el código en `products.barcode` y en `tier_barcodes`:
   - si lo encuentra, agrega una unidad;
   - si no, muestra "Código no encontrado: X".
6. **Ventas en espera.**
   - Se guardan en `localStorage` con la clave `nexa.pos.held.<branch_id>`, como un arreglo de `{id, label, created_at, cart}`. `id` es el UUID que se usará como id de la venta.
   - Máximo 10. Si se intenta guardar una más: "Tenés 10 ventas en espera. Retomá o descartá una."
   - El botón "En espera (N)" abre una lista con Retomar y Descartar.
   - `label` es el nombre del cliente o el alias, o "Venta HH:MM".
7. **Efectivo rápido.** En el cobro, botones con el total exacto y los valores `ceil(total / d) × d` para `d` = 1.000, 5.000, 10.000, 20.000 y 50.000. Se quitan los repetidos y los menores al total, y se ordenan de menor a mayor.

**Mock:** la cotización del modo demo aplica solo los descuentos manuales, las tarifas y el cupón `BIENVENIDA10`. Las promociones del mock solo se muestran en su pantalla y no afectan la cotización. Escríbelo en un comentario del código.

---

## F1.15 Tirilla y precuenta

Edita `components/pos/print-ticket.tsx`. El ancho sale de `print.ticket_width_mm` (58 u 80 mm).

**Tirilla de venta**, en este orden:
1. Nombre del negocio, NIT con DV y nombre de la sucursal.
2. "Comprobante POS-000123", fecha y hora en la zona de la sucursal (formato `DD/MM/YYYY HH:mm`), cajero y caja.
3. Cliente: nombre y documento, o el alias, o "Consumidor final". Mesero, si lo hay.
4. Líneas: nombre (con "x12" si es presentación), cantidad × precio y total. Debajo, cada descuento con su nombre.
5. Subtotal, Descuentos y desglose de impuestos por tasa ("IVA 19 %: $X", "IVA 5 %: $X", "INC 8 %: $X"; solo las tasas que aparecen).
6. Propina, si la hay.
7. **Total**.
8. Pagos en el orden en que se registraron; con efectivo, "Recibido" y "Vuelto".
9. El CUFE y el QR **solo** si `fiscal_status = ACEPTADO` (en esta fase nunca pasa; deja el bloque condicional listo).
10. "Gracias por tu compra".

**Precuenta:** el botón "Precuenta" del carrito imprime, con la misma plantilla:
- título "PRECUENTA - NO VÁLIDA COMO FACTURA";
- sin número, sin pagos y sin bloque fiscal;
- usa los datos de la última cotización.

---

## Códigos de error de la fase 1 y su texto en pantalla

Agrega esta tabla a `ERROR_MESSAGES` en el frontend:

| Código | Texto (voseo) |
|---|---|
| `REGISTER_INVALID` | Revisá los datos de la caja. |
| `REGISTER_CODE_TAKEN` | Ya existe una caja con ese código. |
| `REGISTER_HAS_OPEN_SESSION` | La caja tiene un turno abierto. Cerralo primero. |
| `REGISTER_BUSY` | Esa caja ya tiene un turno abierto. |
| `USER_HAS_OPEN_SESSION` | Ya tenés un turno abierto en esta sucursal. |
| `CASH_AMOUNT_INVALID` | El monto debe ser mayor a cero. |
| `CASH_INSUFFICIENT` | No hay suficiente efectivo en la caja. |
| `CASH_KIND_NOT_ALLOWED` | Ese tipo de movimiento no se registra a mano. |
| `CASH_STATE_CHANGED` | Hubo movimientos nuevos mientras contabas. Volvé a contar. |
| `CUSTOMER_INVALID` | Revisá los datos del cliente. |
| `CUSTOMER_DUPLICATED` | Ya existe un cliente con ese documento. |
| `SALE_CUSTOMER_AND_ALIAS` | Elegí cliente o alias, no los dos. |
| `SALE_ALIAS_INVALID` | El alias debe tener entre 1 y 40 caracteres. |
| `SALE_ALIAS_NOT_ALLOWED` | Esta sucursal no permite ventas con alias. |
| `SALE_CUSTOMER_REQUIRED` | Esta sucursal exige un cliente en cada venta. |
| `SALE_CUSTOMER_INVALID` | El cliente no existe o está archivado. |
| `PRICE_CHANGED` | Los precios cambiaron. Revisá el total. |
| `DISCOUNT_ABOVE_MAX` | El descuento supera el máximo permitido. |
| `TIER_INVALID` | La tarifa no es válida para esta línea. |
| `TIER_MISSING_UNITS` | La presentación no tiene unidades configuradas. |
| `TIER_KIND_IMMUTABLE` | No se puede cambiar el tipo de una tarifa. |
| `BARCODE_TAKEN` | Ese código de barras ya está en uso. |
| `PROMOTION_INVALID` | Revisá los datos de la promoción. |
| `PROMOTION_NOT_EDITABLE` | Pausá la promoción para editarla. |
| `PROMOTION_EXHAUSTED` | Una promoción se agotó. Revisá el total. |
| `COUPON_NOT_FOUND` | El cupón no existe. |
| `COUPON_INACTIVE` | El cupón no está activo. |
| `COUPON_NOT_STARTED` | El cupón todavía no está vigente. |
| `COUPON_EXPIRED` | El cupón venció. |
| `COUPON_EXHAUSTED` | El cupón se agotó. |
| `COUPON_REQUIRES_CUSTOMER` | Este cupón necesita un cliente identificado. |
| `COUPON_CUSTOMER_LIMIT` | Este cliente ya usó el cupón. |
| `COUPON_MIN_SUBTOTAL` | La compra no alcanza el mínimo del cupón. |
| `COUPON_NOT_APPLICABLE` | El cupón no aplica a estos productos. |
| `COUPON_CODE_TAKEN` | Ya existe un cupón con ese código. |
| `PRICE_OVERRIDE_DISABLED` | Esta sucursal no permite cambiar precios. |
| `PRICE_OVERRIDE_NOT_ALLOWED_PRODUCT` | Este producto no permite cambiar el precio. |
| `PRICE_OVERRIDE_REASON_REQUIRED` | Escribí el motivo del cambio de precio. |
| `CUSTOM_ITEM_INVALID` | Revisá el ítem libre. |
| `TIPS_DISABLED` | Las propinas están desactivadas. |
| `TIP_INVALID` | Revisá la propina y el mesero. |
| `IDEMPOTENCY_CONFLICT` | Esta operación ya se registró con otros datos. |
| `REFUND_ORDER_INVALID` | Esta venta no admite devoluciones. |
| `REFUND_INVALID` | Revisá las líneas de la devolución. |
| `REFUND_QTY_EXCEEDED` | La cantidad supera lo que queda por devolver. |
| `REFUND_ORIGINAL_MIXED` | La venta tuvo varios medios de pago: devolvé en efectivo. |
| `REFUND_REFERENCE_REQUIRED` | Escribí la referencia del reverso. |
| `REFUND_NEEDS_OPEN_SESSION` | Abrí un turno de caja para devolver efectivo. |
| `REFUND_NOT_PENDING` | La devolución ya fue decidida. |
| `VOID_NOT_ALLOWED` | No se puede anular: la venta no es de hoy, el turno cerró, tiene devoluciones o ya fue facturada. |
| `VOID_REASON_REQUIRED` | Escribí el motivo de la anulación. |

---

## Lista de verificación antes de escribir `TERMINADA-REPORTE-FASE-1.md`

- [ ] Una sola migración, `20261103000100_fase1_pos_caja.sql`, que corre limpia con `test.sh`.
- [ ] Todas las pruebas **[P]** de este documento existen con el nombre de función indicado y pasan.
- [ ] Las pruebas que ya existían siguen pasando, solo con los ajustes de `quoted_total` y `register_id`.
- [ ] `fmt`, `clippy` con `-D warnings`, `test`, `tsc` y `lint` en verde.
- [ ] `nexa-backend/README.md` documenta todos los endpoints nuevos o cambiados.
- [ ] El modo demo funciona: abrir turno eligiendo caja, vender con cliente, tarifa y cupón, devolver, anular y ver el historial.
- [ ] Ningún botón sin permiso queda visible.
- [ ] Nada de lo que está "fuera de alcance" quedó hecho.

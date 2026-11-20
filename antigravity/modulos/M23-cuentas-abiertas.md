# M23 Cuentas abiertas por mesa y por nombre, cobro compartido y cuenta no pagada

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorio | `nexa` (repo único: `backend/` y `frontend/`) |
| Rama | `feat/roles-permisos` (la misma de M21 y M22) |
| Depende de | M22 aprobado por Claude |
| Migración | Archivo **nuevo** `backend/supabase/migrations/20261120000300_cuentas_abiertas.sql` (texto exacto en C1) |
| Commits | Uno por subagente, con el mensaje de la tabla "Reparto en subagentes" |
| Reporte | Sección "M23" de `antigravity/REPORTE-ROLES.md` |
| Fuente de la decisión | `nexa/docs/roles-y-permisos.md`, reglas 13 y 16. Si difieren, **manda este módulo**. |

## Prompt para Antigravity

```text
Vas a hacer el módulo M23 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

Contexto de rutas: todo vive en un repositorio único, `nexa/`, y las rutas de este módulo son relativas a su raíz: `backend/`, `frontend/`, `antigravity/`, `despliegue/`, `docs/`. Trabajá siempre con la terminal abierta en esa raíz.

Condición previa: M22 debe estar aprobado por Claude. Seguí en la rama feat/roles-permisos. Si git status no está limpio, detenete y avisame.

1. Leé completos antigravity/00-LEEME.md (reglas generales), antigravity/INFORME-SALIDA.md (§4), CLAUDE.md y antigravity/modulos/M23-cuentas-abiertas.md.
2. BLOQUE 1: abrí 5 subagentes (M23.1 a M23.5) y lanzalos TODOS AL MISMO TIEMPO. A cada uno dale: su tarea tal como está escrita, su lista de "Archivos que le pertenecen" (sección "Reparto en subagentes") y la regla 0 del LEEME. Ningún subagente edita un archivo que no está en su lista; si lo necesita, se detiene y te avisa.
3. Cuando el bloque 1 termine, uní el trabajo, resolvé los choques y hacé un commit por subagente con el mensaje de la tabla.
4. BLOQUE 2: lanzá el subagente M23.6 (pruebas), con el código del bloque 1 ya unido. Commit con el mensaje de la tabla.
5. Corré VOS la verificación del coordinador, con Docker Desktop encendido, después del último cambio.
6. Escribí la sección "M23" en antigravity/REPORTE-ROLES.md como dice "Reporte".
7. NO hagas push, NO apliques migraciones a Supabase y NO leas ni edites .env. El push lo hace Pedro.
8. Detenete. Escribime "M23 listo para revisión".
```

## Reparto en subagentes

Lo que no está escrito en este módulo está prohibido. Ningún subagente edita un archivo ajeno.

### Bloque 1 (los cinco a la vez)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M23.1 | C1 migración, C2 permisos y `uses_tables`, C3 invitar Mesero | `backend/supabase/migrations/20261120000300_cuentas_abiertas.sql` (nuevo), `backend/src/tenancy.rs`, `backend/src/settings.rs`, `backend/src/invitations.rs`, `backend/src/users.rs` | `feat(cuentas): tablas, permisos y mesero invitable` |
| M23.2 | C4 backend de cuentas y mesas, C5 cambio en `sales.rs` | `backend/src/accounts.rs` (nuevo), `backend/src/routes/cuentas.rs` (nuevo), `backend/src/routes/mod.rs`, `backend/src/main.rs`, `backend/src/sales.rs` | `feat(cuentas): cuentas abiertas, cobro compartido y no pagó` |
| M23.3 | C6 tipos, API, mocks, inicio y guardia del mesero | `frontend/lib/api/types.ts`, `frontend/lib/api/index.ts`, `frontend/lib/api/mock*.ts`, `frontend/lib/stores/session.ts`, `frontend/components/auth/session-gate.tsx`, `frontend/lib/route-permissions.ts` | `feat(frontend): api y mocks de cuentas abiertas` |
| M23.4 | C7 pantalla de cuentas | `frontend/app/(operacion)/cuentas/**` (nuevo), `frontend/components/cuentas/**` (nuevo), `frontend/components/nexa/operation-topbar.tsx` | `feat(frontend): pantalla de cuentas abiertas` |
| M23.5 | C8 ajustes de mesas, Mesero en usuarios, aviso en caja | `frontend/components/configuracion/settings-screen.tsx`, `frontend/components/configuracion/users-screen.tsx`, `frontend/components/caja/caja-screen.tsx` | `feat(frontend): mesas en ajustes y aviso de cuentas abiertas` |

Dependencias: M23.2 usa las tablas que C1 define aunque todavía no existan en su máquina; M23.1 es la única dueña de la migración. M23.4 usa las funciones de `api` con los nombres exactos de C6, que declara M23.3. Como arrancan juntos, `tsc` y `cargo` pueden fallar hasta que 3.8 una todo: es esperado, y lo verifica 3.8, no el subagente.

### Bloque 2 (después de unir el bloque 1)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M23.6 | Todas las pruebas [P] de este módulo y filas nuevas del README | `backend/src/db_tests.rs`, `backend/README.md` | `test(cuentas): pruebas de cuentas abiertas` |

## Decisiones que este módulo implementa

1. **Una cuenta abierta** es una lista de productos que se va llenando y se cobra al final. Sin fiado. Puede ser **de una mesa** o **de un nombre** (cliente en el mostrador).
2. **Todos los que toman pedidos** (`pos:order`) ven todas las cuentas de la sucursal.
3. **Cobra quien tenga `pos:collect_open`**: Cajero, Mesero, Supervisor, Admin y Dueño. Cobra cualquier cuenta, aunque no la haya abierto.
4. **Cada cuenta guarda** quién la abrió, quién agregó cada producto, quién lo quitó y quién la cobró, con hora.
5. **Cuenta que el cliente no pagó:** quien la atendió la reporta; el Admin (o el Dueño) decide: dejarla abierta para cobrar después, perdonarla (cortesía) o cerrarla como pérdida. Quien reporta no decide, salvo el Dueño.
6. **El cobro reutiliza `checkout_tx`**: precios, promociones, stock, comprobante y fila fiscal salen del mismo camino que una venta de mostrador. No se duplica ninguna regla de venta.
7. **Mesas opcionales** con el interruptor `features.uses_tables` (apagado por defecto), disponible en cualquier tipo de negocio.
8. **Versión 1, sin esto:** descuento manual en la cuenta, ítems libres, reserva de stock mientras la cuenta está abierta, comandas a cocina, división de cuenta, propina al mesero y mover una cuenta de mesa.

## C1. Migración

Archivo nuevo `backend/supabase/migrations/20261120000300_cuentas_abiertas.sql`. No edites migraciones viejas:

```sql
-- M23 Cuentas abiertas

insert into public.permissions (code, module, description) values
  ('pos:collect_open', 'pos', 'Cobrar cuentas abiertas de mesas o de clientes'),
  ('pos:resolve_unpaid', 'pos', 'Decidir qué hacer con una cuenta que el cliente no pagó')
on conflict (code) do update set description = excluded.description, module = excluded.module;

with rol as (
  select tima_id, tima_codigo from public.tbl_tipos_maestra
   where padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
)
insert into public.role_permissions (role_id, permission_code)
select rol.tima_id, v.code
  from rol
  join (values
    ('ADMIN_CAJA', 'pos:collect_open'),
    ('ADMIN_CAJA', 'pos:resolve_unpaid'),
    ('SUPERVISOR', 'pos:collect_open'),
    ('CAJERO',     'pos:collect_open'),
    ('MESERO',     'pos:collect_open')
  ) v(rol, code) on v.rol = rol.tima_codigo
on conflict do nothing;

create table public.open_accounts (
  id uuid primary key,  -- lo genera el cliente; es también el id de la venta al cobrar
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  table_id uuid,
  account_name text check (account_name is null or length(trim(account_name)) between 1 and 60),
  customer_id uuid references public.customers(id),
  status text not null default 'ABIERTA'
    check (status in ('ABIERTA','NO_PAGO','COBRADA','CORTESIA','PERDIDA','ANULADA')),
  opened_by uuid not null references public.users(id),
  opened_at timestamptz not null default now(),
  collected_by uuid references public.users(id),
  collected_at timestamptz,
  reported_by uuid references public.users(id),
  reported_at timestamptz,
  unpaid_reason text,
  resolved_by uuid references public.users(id),
  resolved_at timestamptz,
  resolution_note text,
  loss_amount numeric(14,2),
  closed_at timestamptz,
  unique (tenant_id, id),
  check (table_id is not null or account_name is not null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, table_id) references public.restaurant_tables (tenant_id, id)
);
create index open_accounts_branch_idx on public.open_accounts (tenant_id, branch_id, status);
-- Una sola cuenta viva por mesa.
create unique index open_accounts_table_live_idx on public.open_accounts (tenant_id, table_id)
  where table_id is not null and status in ('ABIERTA','NO_PAGO');

create table public.open_account_lines (
  id uuid primary key,  -- lo genera el cliente: reintentar no duplica
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  account_id uuid not null,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  notes text,
  added_by uuid not null references public.users(id),
  added_at timestamptz not null default now(),
  removed_by uuid references public.users(id),
  removed_at timestamptz,
  foreign key (tenant_id, account_id) references public.open_accounts (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index open_account_lines_account_idx on public.open_account_lines (account_id);

alter table public.open_accounts enable row level security;
alter table public.open_account_lines enable row level security;

create policy open_accounts_scope on public.open_accounts for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
create policy open_account_lines_scope on public.open_account_lines for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
```

Los permisos de la tabla (`grant ... to authenticated`) van **igual que en las tablas recientes**: copiá el patrón de la migración `20261103000100_fase1_pos_caja.sql` (tabla `promotions`). Nunca `grant` a `anon` ni `using (true)`.

## C2. Permisos y ajuste de mesas

1. `tenancy::base_allows`: agregá `pos:collect_open` a `CAJERO`, `MESERO` y `SUPERVISOR`. `ADMIN_CAJA` ya los recibe por la regla `!matches!`. `pos:resolve_unpaid` solo para `ADMIN_CAJA` (por la misma regla) y `DUENO`.
2. `settings.rs`: dentro de `FeaturesSettings` (creada en M21), agregá `#[serde(default)] pub uses_tables: bool` con valor por defecto `false`. Sin eso la sucursal no usa mesas.

## C3. Mesero invitable

Hoy invitar con rol `MESERO` se rechaza con `ROLE_NOT_AVAILABLE`. Buscalo con `grep -rn ROLE_NOT_AVAILABLE backend/src`.

1. `MESERO` pasa a permitirse. `COCINA` y `REPARTIDOR` siguen rechazados.
2. La prueba vieja que espera el rechazo de `MESERO` se **cambia** para que verifique el nuevo comportamiento. No la borres; anotala en el reporte.

## C4. Backend (`accounts.rs` y `routes/cuentas.rs`)

Todas las funciones públicas abren la transacción con `tenancy::begin` o `begin_in_branch`, y el permiso se comprueba **justo después**. Las funciones que reciben solo el id de la cuenta cargan la cuenta (RLS la filtra) y llaman a `tenancy::ensure_branch_tx` con su sucursal. Estados: `ABIERTA`, `NO_PAGO`, `COBRADA`, `CORTESIA`, `PERDIDA`, `ANULADA`. Cada acción escribe en `audit_log` con `audit::record_tx` (acciones `ACCOUNT_OPENED`, `ACCOUNT_LINE_ADDED`, `ACCOUNT_LINE_REMOVED`, `ACCOUNT_PAID`, `ACCOUNT_REPORTED_UNPAID`, `ACCOUNT_RESOLVED`, `ACCOUNT_CANCELLED`).

| Método y ruta | Permiso | Cuerpo |
|---|---|---|
| `GET /v1/tables?branch_id=` | `pos:order` | (lista de mesas con su estado y la cuenta viva, si hay) |
| `POST /v1/tables` | `settings:write` | `{branch_id, code}` |
| `GET /v1/accounts?branch_id=&status=` | `pos:order` | |
| `POST /v1/accounts` | `pos:order` | `{id, branch_id, table_id?, account_name?, customer_id?}` |
| `GET /v1/accounts/{id}` | `pos:order` | |
| `POST /v1/accounts/{id}/lines` | `pos:order` | `{id, product_id, qty, notes?}` |
| `DELETE /v1/accounts/{id}/lines/{line_id}` | `pos:order` | |
| `POST /v1/accounts/{id}/pay` | `pos:collect_open` | `{payments, tip_amount?, coupon_code?}` |
| `POST /v1/accounts/{id}/report-unpaid` | `pos:order` | `{reason}` |
| `POST /v1/accounts/{id}/resolve` | `pos:resolve_unpaid` | `{decision, note}` |
| `POST /v1/accounts/{id}/cancel` | `pos:order` | |

`GET /v1/accounts` sin `status` devuelve las vivas (`ABIERTA` y `NO_PAGO`). `GET /v1/accounts/{id}` devuelve la cuenta, sus líneas activas (nombre del producto, cantidad, `added_by` con el nombre de quien agregó, hora) y el total cotizado con `sales::quote_tx` (la fuente de verdad del precio, igual que el POS).

Respuesta de cuenta (campos, todos siempre presentes): `id`, `branch_id`, `table_id`, `table_code`, `account_name`, `status`, `opened_by`, `opened_by_name`, `opened_at`, `collected_by_name`, `reported_by_name`, `unpaid_reason`, `total` (`string`), `lines`.

### Abrir (`POST /v1/accounts`), en este orden

1. `pos:order`, si no: 403.
2. Sin `table_id` y sin `account_name`: 422 `ACCOUNT_NEEDS_TARGET`, "Poné el número de mesa o un nombre para la cuenta.".
3. `table_id` con `settings.features.uses_tables = false`: 422 `TABLES_DISABLED`, "Esta sucursal no usa mesas.".
4. `account_name` ya recortado de más de 60 caracteres: 422 `ACCOUNT_NAME_INVALID`, "El nombre tiene hasta 60 letras.".
5. Si el `id` ya existe: mismos datos → 200 con la cuenta existente; datos distintos → 409 `ACCOUNT_ID_REUSED`, "Ese id ya se usó con otros datos.". Antes de esto, `pg_advisory_xact_lock(hashtextextended(id::text, 0))`, como hace `checkout_tx`.
6. La mesa debe existir en la sucursal (404) y estar `LIBRE`: si no, 409 `TABLE_BUSY`, "Esa mesa ya tiene una cuenta abierta.". Se marca `OCUPADA` con `public.tipo_id('MESA','OCUPADA')`, como hace `orders.rs`.
7. Inserta con `opened_by = scope.user`.

### Agregar y quitar productos

- **Agregar:** `pos:order`. Cuenta `ABIERTA`, si no: 409 `ACCOUNT_NOT_OPEN`, "Esta cuenta ya no está abierta.". `qty > 0` (422 con `validation`). El producto debe ser vendible en la sucursal: usá `orders::sellable_price` (error de validación si no). El `id` de la línea es del cliente; si ya existe con los mismos datos, devuelve la existente. `added_by = scope.user`.
- **Quitar:** `pos:order`. Cuenta `ABIERTA`. La línea debe estar activa (404 si no). No se borra: se llenan `removed_by` y `removed_at`. Queda en `audit_log` con la línea en `before`. Sin autorización extra en esta versión porque todavía no hay comandas a cocina.

### Cobrar (`POST /v1/accounts/{id}/pay`), en este orden

1. `pos:collect_open`, si no: 403. **No** se exige `pos:sell`.
2. Bloqueá la cuenta con `for update`.
3. Si ya está `COBRADA`: devolvé 200 con `sales::load_response(tx, id)` (reintento seguro) y no hagas nada más.
4. Si no está `ABIERTA`: 409 `ACCOUNT_NOT_OPEN`.
5. Sin líneas activas: 422 `ACCOUNT_EMPTY`, "La cuenta no tiene productos.".
6. **Turno de caja** (de ahí sale el dinero):
   1. el turno abierto de quien cobra, con `cash::open_session_tx`;
   2. si no tiene, y hay **exactamente un** turno abierto en la sucursal, ese (consulta `select id from cash_sessions where branch_id = $1 and closed_at is null for share`);
   3. si no: 409 `ACCOUNT_NEEDS_CASH_SESSION`, "Para cobrar tiene que haber una caja abierta. Pedile al cajero que abra su turno.".
7. Cotizá con `sales::quote_tx` (productos y cantidades de las líneas activas) y armá un `SaleRequest`: `id` = el de la cuenta, `quoted_total` = el total cotizado, `items` = una `SaleItem` por línea (solo `product_id` y `qty`; los demás campos vacíos), `payments`, `tip_amount` y `coupon_code` del cuerpo, `customer_id` de la cuenta. Llamá a `sales::validate` (hacela `pub(crate)`) antes de seguir.
8. Llamá a `sales::checkout_with_session_tx(tx, &scope, &req, Some(turno))` (ver C5).
9. Después, en la misma transacción: `update orders set table_id = <mesa>, waiter_id = <opened_by>, sale_type_id = public.tipo_id('VENTA','MESA')` (el tipo `MESA` solo si hay mesa) donde `id` = el de la cuenta.
10. Marcá la cuenta `COBRADA` con `collected_by = scope.user`, `collected_at` y `closed_at`. Liberá la mesa (`LIBRE`).

El comprobante, el consecutivo y la fila fiscal `PENDIENTE` salen de `checkout_tx`; no los toques. El dinero queda con `created_by = scope.user`: quien cobró.

### Reportar que no pagó (`POST /v1/accounts/{id}/report-unpaid`)

1. `pos:order`.
2. Cuenta `ABIERTA`, si no: 409 `ACCOUNT_NOT_OPEN`.
3. `reason` de 3 a 200 caracteres, si no: 422 `ACCOUNT_REASON_REQUIRED`, "Escribí qué pasó (mínimo 3 letras).".
4. Pasa a `NO_PAGO` con `reported_by`, `reported_at` y `unpaid_reason`. La mesa sigue `OCUPADA`.

### Decidir (`POST /v1/accounts/{id}/resolve`)

`decision` es `REABRIR`, `CORTESIA` o `PERDIDA`. Orden:

1. `pos:resolve_unpaid`.
2. Cuenta `NO_PAGO`, si no: 409 `ACCOUNT_NOT_REPORTED`, "Esta cuenta no está reportada como no pagada.".
3. `decision` inválida: 422 `ACCOUNT_DECISION_INVALID`.
4. `note` de 3 a 200 caracteres: 422 `ACCOUNT_NOTE_REQUIRED`, "Escribí el motivo de tu decisión.".
5. Si `reported_by == scope.user` y el rol no es `DUENO`: 403 `ACCOUNT_SELF_RESOLVE`, "Quien reportó la cuenta no puede decidir sobre ella.".
6. `REABRIR`: vuelve a `ABIERTA` (la mesa sigue `OCUPADA`).
7. `CORTESIA` o `PERDIDA`: la mercancía ya salió. Cotizá la cuenta (`quote_tx`) y guardá el total en `loss_amount`. Por cada producto (sumando cantidades de líneas iguales) llamá a `inventory::apply_delta_tx` con `delta` negativo, `movement: "AJUSTE_SALIDA"`, `ref_type: "CUENTA_ABIERTA"`, `ref_id` = el de la cuenta, `cause: Some(note)`, `location_id` = `locations::default_location_id_tx(tx, branch)` y `unit_cost: None`. Si falta stock y la sucursal no permite stock negativo, el error `STOCK_INSUFFICIENT` sale tal cual: no lo ocultes. Después `closed_at`, `resolved_by`, `resolved_at`, `resolution_note`, y la mesa pasa a `LIBRE`.

Límite a comprobar y anotar en el reporte: `apply_delta_tx` sobre un producto compuesto (con receta) puede no descontar sus insumos. Escribí "no lo probé" si no lo verificaste.

### Anular (`POST /v1/accounts/{id}/cancel`)

1. `pos:order`. Cuenta `ABIERTA`, si no: 409 `ACCOUNT_NOT_OPEN`.
2. Si tiene líneas activas, además `pos:refund`, si no: 403.
3. Pasa a `ANULADA`, con `closed_at`. Libera la mesa.

### Mesas

- `GET /v1/tables`: `id`, `code`, `status` (`LIBRE` u `OCUPADA`), `account_id` (la cuenta viva o `null`).
- `POST /v1/tables`: `code` recortado de 1 a 20 caracteres, 422 si no. Si ya existe en la sucursal: 409 con `state("Esa mesa ya existe.")`. Se crea `LIBRE`. Hoy no existe ningún endpoint de mesas: solo las pruebas las crean.

## C5. Cambio en `sales.rs`

Cambio mínimo, sin tocar la lógica de venta:

1. Renombrá la función `checkout_tx` actual a `checkout_with_session_tx(tx, scope, req, session_override: Option<Uuid>)`.
2. En ella, el bloque que hoy calcula `let session = ...` (alrededor de la línea 1038) pasa a: si `session_override` es `Some(s)`, `let session = Some(s);` y se saltea la búsqueda del turno de quien vende. Si es `None`, queda **igual que hoy**.
3. Creá `checkout_tx(tx, scope, req)` como envoltorio que llama a `checkout_with_session_tx(tx, scope, req, None)`. Todos los llamadores actuales siguen funcionando sin cambios.
4. Hacé `pub(crate)` a `validate`.

## C6. Frontend: tipos, API y mocks

Funciones en `lib/api/index.ts`, con estos nombres exactos (M23.4 las usa tal cual):

```ts
api.tables.list(branchId: string): Promise<TableInfo[]>
api.tables.create(branchId: string, code: string): Promise<TableInfo>
api.accounts.list(branchId: string, status?: AccountStatus): Promise<AccountSummary[]>
api.accounts.get(id: string): Promise<AccountDetail>
api.accounts.open(body: { id: string; branch_id: string; table_id?: string | null; account_name?: string | null; customer_id?: string | null }): Promise<AccountDetail>
api.accounts.addLine(accountId: string, body: { id: string; product_id: string; qty: string; notes?: string | null }): Promise<AccountDetail>
api.accounts.removeLine(accountId: string, lineId: string): Promise<AccountDetail>
api.accounts.pay(accountId: string, body: { payments: SalePaymentInput[]; tip_amount?: string | null; coupon_code?: string | null }): Promise<CheckoutResponse>
api.accounts.reportUnpaid(accountId: string, reason: string): Promise<AccountDetail>
api.accounts.resolve(accountId: string, body: { decision: "REABRIR" | "CORTESIA" | "PERDIDA"; note: string }): Promise<AccountDetail>
api.accounts.cancel(accountId: string): Promise<AccountDetail>
```

Tipos nuevos en `types.ts`: `AccountStatus`, `TableInfo`, `AccountSummary`, `AccountDetail` y `AccountLine`, con los campos de C4. Los montos son `string`. Reutilizá `SalePaymentInput` y `CheckoutResponse`; si los nombres reales son otros, usá los del archivo.

Mock (modo demo): 4 mesas (`1` a `4`), `features.uses_tables = true` en la sucursal demo, y 2 cuentas de ejemplo (una de mesa 2 y una a nombre de "Don Pedro") con productos de licorería. Las cuentas viven en memoria y cambian de verdad al abrir, agregar, cobrar, reportar y decidir. Agregá un usuario demo con rol `MESERO` al mock, como los que ya existen para otros roles.

Además, en el frontend:
- `session.ts`: `homeFor("MESERO")` devuelve `"/cuentas"`.
- `session-gate.tsx`: `MESERO` deja de estar en la lista de roles sin pantallas (`COCINA` y `REPARTIDOR` quedan).
- `route-permissions.ts`: fila nueva `/cuentas` → `["pos:order"]`.

## C7. Frontend: pantalla `/cuentas`

Ruta nueva `app/(operacion)/cuentas/page.tsx`, dentro del layout de operación (sin menú lateral). Componentes en `components/cuentas/`. Textos exactos:

1. **Lista.** Tarjetas de las cuentas vivas: "Mesa 2" o el nombre, total, "Abrió: {nombre}" y el tiempo abierta. Botón primario "Nueva cuenta". Las `NO_PAGO` van marcadas con la insignia ámbar "No pagó".
2. **Nueva cuenta.** Diálogo "Nueva cuenta". Con `uses_tables`, selector de mesa (solo libres) o "Sin mesa, a nombre de:" con un campo de texto. Sin `uses_tables`, solo el nombre. Botón "Abrir cuenta".
3. **Detalle.** Productos con "Agregó: {nombre}" y la hora, botón de quitar, buscador de productos (usá la misma función de `api` que usa `components/pos/catalog-panel.tsx`), total y botones "Cobrar", "No pagó" y "Anular cuenta".
4. **Cobrar.** Diálogo con los medios de pago de `components/pos/payment-methods.ts`, monto y referencia. Si la respuesta es `ACCOUNT_NEEDS_CASH_SESSION`, muestra el mensaje del backend. Al terminar, muestra el comprobante con el componente que ya usa el POS.
5. **No pagó.** Diálogo "¿Qué pasó?" con el campo del motivo y botón "Reportar".
6. **Por decidir.** Para quien tenga `pos:resolve_unpaid`, un aviso arriba "Hay N cuentas por decidir" que abre una lista con cada cuenta, quién la reportó y el motivo, y tres botones: "Cobrar después" (`REABRIR`), "Perdonar" (`CORTESIA`) y "Cerrar como pérdida" (`PERDIDA`), cada uno con el campo de nota. Si `ACCOUNT_SELF_RESOLVE`, muestra el mensaje del backend.
7. **Mostrar solo lo permitido:** "Cobrar" con `can("pos:collect_open")`; "Anular cuenta" con `can("pos:order")`; el aviso de "por decidir" con `can("pos:resolve_unpaid")`.
8. **Barra de operación** (`operation-topbar.tsx`): enlace "Cuentas" visible con `can("pos:order")`. El indicador de caja ("Caja abierta…") se muestra **solo** con `can("cash:open")`; un mesero no debe llamar a los endpoints de caja (darían 403).

## C8. Frontend: ajustes, usuarios y caja

1. **Ajustes** (`settings-screen.tsx`): interruptor "Usar mesas" (`features.uses_tables`) y, cuando está encendido, una tarjeta "Mesas" con la lista y un campo "Agregar mesa" ("Mesa 1", "Terraza 2"…), con `api.tables`.
2. **Usuarios** (`users-screen.tsx`): el rol Mesero aparece bajo "Más roles", con la descripción "Abre cuentas, toma pedidos y cobra", **solo** si la sucursal tiene `uses_tables`.
3. **Caja** (`caja-screen.tsx`): si hay cuentas vivas en la sucursal, un aviso informativo al cerrar turno: "Hay N cuentas abiertas. Cobralas o avisale al encargado.". No bloquea el cierre.

## Pruebas (`m23_cuentas`)

- **[P]** `MESERO` abre una cuenta en una mesa libre con `uses_tables`: 200 y la mesa queda `OCUPADA`. Otra cuenta en la misma mesa: 409 `TABLE_BUSY`.
- **[P]** `table_id` con `uses_tables = false`: 422 `TABLES_DISABLED`. Sin mesa ni nombre: 422 `ACCOUNT_NEEDS_TARGET`.
- **[P]** Reintentar abrir con el mismo `id` y mismos datos: 200 con la misma cuenta. Con datos distintos: 409 `ACCOUNT_ID_REUSED`.
- **[P]** `CAJERO` abre una cuenta por nombre, `MESERO` le agrega un producto: la línea guarda `added_by` = el mesero y `opened_by` = el cajero. Ambos la ven en la lista.
- **[P]** Reintentar `addLine` con el mismo `id`: no duplica la línea.
- **[P]** Quitar una línea: el total baja, la fila sigue con `removed_by`, y `audit_log` tiene `ACCOUNT_LINE_REMOVED`.
- **[P]** `MESERO` cobra una cuenta abierta por el `CAJERO`, con exactamente un turno abierto en la sucursal: 200; el pago queda con `created_by` = mesero, la cuenta con `collected_by` = mesero y `opened_by` = cajero; el stock baja; la mesa queda `LIBRE`; `orders.waiter_id` = quien abrió.
- **[P]** `MESERO` cobra sin ningún turno abierto: 409 `ACCOUNT_NEEDS_CASH_SESSION`. Con dos turnos abiertos y sin turno propio: igual 409.
- **[P]** `MESERO` con turno propio cobra: el pago va a su turno, no a otro.
- **[P]** Cobrar dos veces el mismo `id`: la segunda devuelve 200 con la misma venta y hay una sola fila de `payments`.
- **[P]** `BODEGA` cobra: 403. Cobrar una cuenta vacía: 422 `ACCOUNT_EMPTY`. Cobrar una cuenta `NO_PAGO`: 409 `ACCOUNT_NOT_OPEN`.
- **[P]** `MESERO` reporta que no pagó: la cuenta queda `NO_PAGO` y la mesa `OCUPADA`. Motivo corto: 422 `ACCOUNT_REASON_REQUIRED`. Reportar dos veces: 409.
- **[P]** `resolve`: `MESERO` 403. `ADMIN_CAJA` que no reportó: 200. El mismo `ADMIN_CAJA` que reportó: 403 `ACCOUNT_SELF_RESOLVE`. `DUENO` que reportó: 200. Sin nota: 422 `ACCOUNT_NOTE_REQUIRED`.
- **[P]** `PERDIDA`: queda un movimiento de stock `AJUSTE_SALIDA` por cada producto, `loss_amount` = el total cotizado y la mesa `LIBRE`. `CORTESIA`: igual con estado `CORTESIA`. `REABRIR`: vuelve a `ABIERTA` y la mesa sigue `OCUPADA`.
- **[P]** `cancel`: `MESERO` anula una cuenta vacía: 200. Con productos: 403; `SUPERVISOR`: 200.
- **[P]** RLS: un usuario de otro negocio no ve cuentas ni líneas; uno de otra sucursal tampoco.
- **[P]** Invitar con rol `MESERO`: 200. Con `COCINA`: sigue `ROLE_NOT_AVAILABLE`.
- **[P]** Matriz: `base_allows` da `pos:collect_open` a `CAJERO`, `MESERO` y `SUPERVISOR`, y no a `BODEGA`; y coincide con `role_permissions`.
- **[P]** Las pruebas de venta de mostrador existentes siguen pasando (el cambio en `sales.rs` no puede alterarlas).

## Lo que está prohibido en este módulo

- Aplicar migraciones a Supabase.
- Tocar la lógica de precios, promociones, stock o comprobantes de `sales.rs`. El único cambio permitido es el de C5.
- Descuentos manuales, ítems libres, propina al mesero, comandas, pantalla de cocina, mapa visual de mesas, dividir o mover cuentas.
- Guardar cuentas en `localStorage`. Las cuentas viven en la base.
- Cambiar los permisos de M21 y M22.

## Verificación del coordinador

Esto lo corre el modelo 3.8, no los subagentes, **después de unir todo y del último cambio**. Si algo falla, la corrección vuelve al subagente dueño del archivo:

```sh
cd backend && ./scripts/test.sh
cd ../frontend && npx tsc --noEmit && npx eslint . && npx next build
cd .. && git grep -n 'Ã'    # vacía
cd backend && grep -n "fn checkout_tx" src/sales.rs    # una sola, el envoltorio
```

Luego, en el modo demo (`node frontend/scripts/dev-demo.mjs`, puerto 3200), comprobá y anotá "pasa" o "falla" con lo que viste:
1. Entrar como mesero lleva a `/cuentas` y no muestra el mensaje "Tu rol todavía no tiene pantallas".
2. Abrir una cuenta en la mesa 3 y agregar dos productos.
3. Entrar como cajero: ve la misma cuenta, con "Agregó" del mesero.
4. Cobrar la cuenta como cajero con efectivo: sale el comprobante y la mesa queda libre.
5. Reportar que no pagó una cuenta como mesero.
6. Entrar como Admin: ve "Hay 1 cuenta por decidir" y decide "Cerrar como pérdida".
7. El mesero no ve el indicador de caja ni recibe errores en la consola.
8. Con `uses_tables` apagado en Ajustes, desaparece el selector de mesa.

## Reporte

En `antigravity/REPORTE-ROLES.md`, sección `## M23`:
1. Una tabla con cada subagente: tarea, archivos que tocó, commit y las pruebas que corrió.
2. Los choques que tuviste que resolver al unir, y cómo.
3. La salida completa de tu verificación final y el resultado de los 8 puntos del demo.
4. La tabla de reglas [P] con el nombre de su prueba. Las que falten, "FALTA".
5. Si `apply_delta_tx` descuenta los insumos de un producto compuesto. Escribí "no lo probé" antes que "funciona".
6. Las pruebas viejas que hubo que ajustar (la de `MESERO` rechazado), con archivo y línea.
7. Preguntas abiertas, con archivo y línea.

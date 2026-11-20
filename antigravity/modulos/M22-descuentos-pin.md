# M22 Descuentos y promociones aprobados por el Dueño (PIN y fecha de fin)

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorio | `nexa` (repo único) |
| Rama | `feat/roles-permisos` |
| Depende de | M21 aprobado por Claude |
| Migración | Archivo **nuevo** `backend/supabase/migrations/20261120000200_descuentos_pin.sql` (texto exacto en D1) |
| Dependencia nueva | `argon2 = "0.5"` en `backend/Cargo.toml` (hoy no hay ninguna librería de hash de contraseñas; solo `sha2` y `hmac`) |
| Commits | Uno por subagente, con el mensaje de la tabla "Reparto en subagentes" |
| Reporte | Sección "M22" de `antigravity/REPORTE-ROLES.md` |
| Fuente de la decisión | `nexa/docs/roles-y-permisos.md`, reglas 3 y 14. Si difieren, **manda este módulo**. |

## Prompt para Antigravity

```text
Vas a hacer el módulo M22 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

Contexto de rutas: todo vive en un repositorio único, `nexa/`, y las rutas de este módulo son relativas a su raíz: `backend/`, `frontend/`, `antigravity/`, `despliegue/`, `docs/`. Trabajá siempre con la terminal abierta en esa raíz.

Condición previa: M21 debe estar aprobado por Claude. Seguí en la rama feat/roles-permisos. Si git status no está limpio, detenete y avisame.

1. Leé completos antigravity/00-LEEME.md (reglas generales), antigravity/INFORME-SALIDA.md (§4), CLAUDE.md y antigravity/modulos/M22-descuentos-pin.md.
2. BLOQUE 1: abrí 3 subagentes (M22.1 a M22.3) y lanzalos TODOS AL MISMO TIEMPO. A cada uno dale: su tarea tal como está escrita, su lista de "Archivos que le pertenecen" (sección "Reparto en subagentes") y la regla 0 del LEEME. Ningún subagente edita un archivo que no está en su lista; si lo necesita, se detiene y te avisa.
3. Cuando el bloque 1 termine, uní el trabajo, resolvé los choques y hacé un commit por subagente con el mensaje de la tabla.
4. BLOQUE 2: lanzá el subagente M22.4 (pruebas), con el código del bloque 1 ya unido. Commit con el mensaje de la tabla.
5. Corré VOS la verificación del coordinador, con Docker Desktop encendido, después del último cambio.
6. Escribí la sección "M22" en antigravity/REPORTE-ROLES.md como dice "Reporte".
7. NO hagas push, NO apliques migraciones a Supabase y NO leas ni edites .env. El PIN nunca se escribe en logs, reportes ni commits.
8. Detenete. Escribime "M22 listo para revisión".
```

## Reparto en subagentes

Lo que no está escrito en este módulo está prohibido. Ningún subagente edita un archivo ajeno.

### Bloque 1 (los tres a la vez)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M22.1 | D1 migración completa, `base_allows`, D2 promociones | `backend/supabase/migrations/20261120000200_descuentos_pin.sql` (nuevo), `backend/src/tenancy.rs`, `backend/src/promotions.rs` | `feat(promos): proponer y aprobar con fecha de fin` |
| M22.2 | D3 PIN: guardar, verificar y usarlo en la venta | `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/src/pin.rs` (nuevo), `backend/src/main.rs`, `backend/src/sales.rs`, `backend/src/routes/pin.rs` (nuevo), `backend/src/routes/mod.rs` | `feat(pin): PIN del dueño para descuentos sueltos` |
| M22.3 | D4 pantallas, tipos, API y mocks | `frontend/lib/api/types.ts`, `frontend/lib/api/index.ts`, `frontend/lib/api/mock*.ts`, `frontend/lib/route-permissions.ts`, `frontend/components/pos/**`, `frontend/components/configuracion/audit-screen.tsx`, `frontend/components/configuracion/settings-screen.tsx`, `frontend/app/(admin)/marketing/**`, `frontend/components/marketing/**` | `feat(frontend): descuentos con PIN y promociones por aprobar` |

La migración la escribe solo M22.1, completa, con el texto de D1. M22.2 no crea migraciones: usa las tablas que D1 define (`user_pins`, `pin_failures`, `pin_authorizations`) aunque todavía no existan en su máquina; las pruebas las verifica M22.4.

### Bloque 2 (después de unir el bloque 1)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M22.4 | Todas las pruebas [P] de M22 y filas nuevas del README | `backend/src/db_tests.rs`, `backend/README.md` | `test(pin): pruebas de promociones y PIN` |

## Decisiones que este módulo implementa

1. **Promociones y descuentos sueltos los aprueba solo el Dueño.** Supervisor y Admin **proponen**.
2. **Toda promoción activa tiene fecha de fin.** No hay promociones sin vencimiento.
3. **Una promoción vigente se aplica sola en caja**, sin pedir permiso en cada venta (ya funciona así: `promotions::load_active_rules_tx`).
4. **Un descuento suelto** sobre una línea de una venta lo puede aplicar un empleado **con el PIN del Dueño**. Queda registrado quién vendió, cuánto y con qué autorización.
5. **No hay descuentos preaprobados sin vencimiento.**
6. **Fuera de este módulo (decisión pendiente):** la notificación al celular del Dueño cada vez que se usa su PIN. No hay hoy infraestructura de notificaciones. Este módulo deja el registro y una lista en pantalla; la notificación no se hace.

## D1. Migración

Archivo nuevo `backend/supabase/migrations/20261120000200_descuentos_pin.sql`:

```sql
-- M22 Descuentos y PIN del dueño

insert into public.permissions (code, module, description) values
  ('promotions:propose', 'pos', 'Crear y editar promociones en borrador'),
  ('promotions:approve', 'pos', 'Activar y pausar promociones')
on conflict (code) do update set description = excluded.description, module = excluded.module;

with rol as (
  select tima_id, tima_codigo from public.tbl_tipos_maestra
   where padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
)
insert into public.role_permissions (role_id, permission_code)
select rol.tima_id, v.code
  from rol
  join (values
    ('ADMIN_CAJA', 'promotions:propose'),
    ('SUPERVISOR', 'promotions:propose')
  ) v(rol, code) on v.rol = rol.tima_codigo
on conflict do nothing;

-- El Admin recibió todos los permisos en la fase 0; este no se le deja.
with rol as (
  select tima_id from public.tbl_tipos_maestra
   where padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
     and tima_codigo = 'ADMIN_CAJA'
)
delete from public.role_permissions rp using rol
 where rp.role_id = rol.tima_id and rp.permission_code = 'promotions:approve';

create table public.user_pins (
  user_id uuid primary key references public.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  pin_hash text not null,
  updated_at timestamptz not null default now()
);

create table public.pin_failures (
  user_id uuid primary key references public.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  failed integer not null default 0,
  locked_until timestamptz
);

alter table public.user_pins enable row level security;
alter table public.pin_failures enable row level security;

-- Nadie lee el hash desde el cliente: ninguna política para authenticated.
-- El backend los usa con db::begin_system_tx.

create table public.pin_authorizations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid,
  requested_by uuid not null references public.users(id),
  authorized_by uuid not null references public.users(id),
  kind text not null check (kind in ('DESCUENTO')),
  amount numeric(14,2) not null check (amount >= 0),
  detail jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index pin_authorizations_tenant_idx on public.pin_authorizations (tenant_id, created_at desc);

alter table public.pin_authorizations enable row level security;
create policy pin_authorizations_read on public.pin_authorizations
  for select to authenticated
  using (tenant_id = public.current_tenant_id());
-- Sin política de insert/update/delete: solo el backend escribe (begin_system_tx).
```

`promotions:approve` no se le da a ningún rol: `DUENO` lo tiene siempre por `has_permission`. Ya verificado: `public.users` (`core_tenancy.sql`, línea 116) y `public.current_tenant_id()` (línea 146) existen con esos nombres.

`tenancy::base_allows`: `ADMIN_CAJA` pasa a `!matches!(code, "costs:read" | "margins:read" | "pos:discount" | "promotions:approve")`. `SUPERVISOR` y `ADMIN_CAJA` ganan `promotions:propose` (en `ADMIN_CAJA` ya está incluido por la regla `!matches!`; en `SUPERVISOR` agregalo a la lista).

## D2. Promociones: proponer y aprobar (`promotions.rs`)

| Función | Permiso hoy | Permiso nuevo |
|---|---|---|
| `list` | `pos:sell` | sin cambio |
| `create`, `update` | `settings:write` | `promotions:propose` |
| `activate`, `pause` | `settings:write` | `promotions:approve` |

Reglas:

1. `create` guarda siempre `status = 'BORRADOR'`, aunque el cuerpo traiga otro valor.
2. `update` de una promoción `ACTIVA` por alguien **sin** `promotions:approve` la devuelve a `BORRADOR`. Por alguien con `promotions:approve`, la deja como está.
3. `activate`: antes de cambiar el estado, en este orden:
   1. si `ends_at` es `NULL`: 422 `PROMO_END_REQUIRED`, "Poné la fecha en que termina la promoción antes de activarla.";
   2. si `ends_at <= now()`: 422 `PROMO_END_IN_PAST`, "La fecha de fin ya pasó. Elegí una fecha futura.".
4. `activate` guarda `approved_by` y `approved_at` (columnas nuevas, ver abajo) y escribe en `audit_log` con `audit::record_tx` la acción `PROMOTION_APPROVED`, con `before` y `after`.
5. `pause` escribe `PROMOTION_PAUSED`.
6. Las promociones `ACTIVA` que ya existan sin `ends_at` siguen funcionando; solo se exige para activar.

Agregá al final de la migración D1:

```sql
alter table public.promotions
  add column approved_by uuid references public.users(id),
  add column approved_at timestamptz;
```

## D3. PIN del Dueño para un descuento suelto

### D3.1 Guardar el PIN

- `PUT /v1/me/pin`, cuerpo `{ "pin": "1234", "current_pin": "0000" }`. Solo `scope.role == "DUENO"`; otro rol: 403.
- `pin`: de 4 a 6 dígitos (`^[0-9]{4,6}$`). Si no: 422 `PIN_INVALID_FORMAT`, "El PIN tiene entre 4 y 6 números.".
- Si ya existe un PIN, `current_pin` es obligatorio y debe coincidir; si no: 403 `PIN_WRONG`, "El PIN actual no es correcto.".
- Se guarda con Argon2id (`argon2` crate, parámetros por defecto, sal aleatoria) en `user_pins.pin_hash`. Nunca se devuelve ni se registra.
- `audit_log`: acción `PIN_CHANGED`, sin el valor.
- Filas nuevas en `backend/README.md` y tipos en `lib/api/types.ts`.

### D3.2 Verificar el PIN

Función nueva en `backend/src/pin.rs`:

```rust
pub async fn verify_owner_pin(
    pool: &PgPool,
    scope: &Scope,
    requested_by: Uuid,
    pin: &str,
) -> Result<Uuid /* user_id del dueño que autorizó */, ApiError>
```

Usa `db::begin_system_tx` (no la transacción del usuario). Orden exacto:

1. Si `pin_failures.locked_until > now()` para `requested_by`: 429 `PIN_LOCKED`, "Demasiados intentos. Probá de nuevo en 15 minutos.".
2. Busca los PIN de todos los usuarios con rol `DUENO` en `scope.tenant_id`. Si no hay ninguno: 409 `PIN_NOT_SET`, "El dueño todavía no creó su PIN.".
3. Si el PIN coincide con alguno, borra `pin_failures` de `requested_by` y devuelve el `user_id` de ese dueño.
4. Si no coincide: suma 1 a `pin_failures.failed`. Al llegar a 5, pone `locked_until = now() + interval '15 minutes'` y `failed = 0`. Responde 403 `PIN_WRONG`, "El PIN no es correcto.".

El incremento del paso 4 **se confirma (`commit`) antes de devolver el error**; por eso va en su propia transacción de sistema. Si se hiciera en la transacción del usuario, el `rollback` lo borraría.

### D3.3 Descuento en la venta (`sales.rs`)

1. `SaleRequest` y `QuoteRequest` ganan `#[serde(default)] pub discount_owner_pin: Option<String>`.
2. En la función compartida que valida cada línea (donde hoy está `scope.require_permission("pos:discount")?`, alrededor de la línea 473), reemplazá esa llamada por:

```rust
if disc > Decimal::ZERO && !discount_authorized {
    return Err(ApiError::Domain(DomainError::Forbidden));
}
```

   `discount_authorized: bool` entra como parámetro de la función, y lo calcula el llamador:
   - `checkout_tx`: es `true` si `scope.has_permission("pos:discount")`; si no, y hay algún `discount_pct > 0` y `discount_owner_pin` viene con valor, llama a `pin::verify_owner_pin` y es `true` si devuelve `Ok`. Si no hay PIN: 403 como hoy.
   - `quote_tx`: es siempre `true` (cotizar no mueve dinero ni stock).
   - La misma variable gobierna la comprobación de tarifa manual de `sales.rs` (la que hoy llama a `has_permission("pos:discount")`, cerca de la línea 812): con PIN válido, el cajero puede elegir una tarifa que no es la de su cliente.
3. La verificación del PIN va **antes** de cualquier otra validación de las líneas, para no dar pistas con otros errores.
4. Cuando el PIN autoriza, por cada venta (no por línea) se inserta una fila en `pin_authorizations` con `kind = 'DESCUENTO'`, `requested_by` el vendedor, `authorized_by` el dueño, `amount` la suma de los descuentos de línea en pesos y `detail = {"lines": [{"product_id": ..., "discount_pct": ...}]}`; y `audit::record_tx` con la acción `DISCOUNT_PIN_AUTH`.
5. `settings.pos.max_discount_pct` sigue valiendo, también con PIN: `DISCOUNT_ABOVE_MAX` no se salta.
6. `GET /v1/pin-authorizations?branch_id=&from=&to=` (solo `reports:read`): lista las filas con el nombre de quien vendió y de quien autorizó. Lo usa la pantalla D4.2.

## D4. Pantallas

1. **POS:** si el usuario no tiene `pos:discount` y toca el botón de descuento de una línea, se abre el modal "Autorización del dueño": título "Descuento", texto "Pedile al dueño que digite su PIN para aplicar este descuento.", campo numérico enmascarado, botones "Aplicar" y "Cancelar". Los errores `PIN_WRONG`, `PIN_LOCKED` y `PIN_NOT_SET` se muestran con el texto del backend. El PIN se envía en `discount_owner_pin` y **no se guarda** en el estado del carrito después de la venta.
2. **Descuentos autorizados** (`/configuracion/auditoria`, pestaña nueva "Descuentos con PIN"): tabla con fecha, quién vendió, quién autorizó, monto. Visible con `reports:read`.
3. **Promociones** (`/marketing`):
   - Con `promotions:propose` sin `promotions:approve`: el botón "Activar" se reemplaza por "Pedir aprobación", que solo deja la promoción en `BORRADOR`; se muestra la insignia "Esperando al dueño".
   - Con `promotions:approve`: botones "Activar" y "Pausar". "Activar" exige llenar "Termina el" (fecha y hora) en el mismo diálogo.
   - **Ruta:** en `lib/route-permissions.ts`, `/marketing` pasa de `["settings:write"]` a `["promotions:propose", "promotions:approve", "settings:write"]`. Sin este cambio el Supervisor, que no tiene `settings:write`, no puede abrir la pantalla.
4. **Mi PIN** (`/configuracion`, tarjeta visible solo para el Dueño): "Crear PIN" o "Cambiar PIN", con los textos de D3.1.
5. Modo demo: el mock acepta el PIN `1234` como el del dueño y rechaza cualquier otro con `PIN_WRONG`; tras 5 fallos devuelve `PIN_LOCKED`.

## Pruebas (`m22_descuentos`)

- **[P]** `SUPERVISOR` y `ADMIN_CAJA` crean una promoción: 200 y queda `BORRADOR`. `CAJERO`: 403.
- **[P]** `ADMIN_CAJA` activa una promoción: 403. `DUENO` la activa con `ends_at` futuro: 200 y `approved_by` guardado.
- **[P]** `DUENO` activa sin `ends_at`: 422 `PROMO_END_REQUIRED`. Con `ends_at` pasado: 422 `PROMO_END_IN_PAST`.
- **[P]** `SUPERVISOR` edita una promoción `ACTIVA`: queda `BORRADOR`. `DUENO` la edita: sigue `ACTIVA`.
- **[P]** Promoción `ACTIVA` y vigente: la venta del `CAJERO` la aplica sin PIN.
- **[P]** Promoción cuyo `ends_at` ya pasó: no se aplica (ya lo cubre `load_active_rules_tx`; la prueba lo fija).
- **[P]** `PUT /v1/me/pin` con `CAJERO`: 403. Con `DUENO`: 200; `user_pins.pin_hash` no contiene el PIN en texto plano. PIN de 3 dígitos: 422 `PIN_INVALID_FORMAT`.
- **[P]** Venta del `CAJERO` con `discount_pct` y sin PIN: 403. Con PIN correcto: 200 y una fila en `pin_authorizations` con `requested_by` = cajero y `authorized_by` = dueño. Con PIN incorrecto: 403 `PIN_WRONG`.
- **[P]** 5 PIN incorrectos seguidos: el sexto intento, aunque sea correcto, da 429 `PIN_LOCKED`. El contador **persiste** aunque cada intento falle (prueba que usa transacción de sistema).
- **[P]** Descuento con PIN correcto pero sobre `max_discount_pct`: 422 `DISCOUNT_ABOVE_MAX`.
- **[P]** `quote` con `discount_pct` por un `CAJERO` sin PIN: 200.
- **[P]** Dueño sin PIN creado y cajero manda un PIN: 409 `PIN_NOT_SET`.
- **[P]** RLS: un usuario de otro negocio no ve filas de `pin_authorizations` ni puede leer `user_pins`.

## Lo que está prohibido en este módulo

- Notificaciones al celular (WhatsApp, push o correo). Decisión pendiente.
- Descuentos preaprobados o promociones sin `ends_at` al activar.
- Guardar el PIN en texto plano, en logs o en `audit_log`.
- Cuentas abiertas por mesa o por nombre (M23).

## Verificación del coordinador

Esto lo corre el modelo 3.8, no los subagentes, **después de unir todo y del último cambio**. Si algo falla, la corrección vuelve al subagente dueño del archivo.

```sh
cd backend && ./scripts/test.sh
cd ../frontend && npx tsc --noEmit && npx eslint . && npx next build
cd .. && git grep -n 'Ã'    # vacía
git grep -n "discount_owner_pin" backend/src | grep -v "db_tests\|sales.rs"    # vacía
```

## Reporte

En `antigravity/REPORTE-ROLES.md`, sección `## M22`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc`, `eslint` y `next build`.
3. La tabla de reglas [P] con el nombre de su prueba. Las que falten, "FALTA".
4. Los nombres reales de tablas o funciones que difirieron de los de este módulo.
5. Lo que no se pudo hacer, con archivo y línea. Escribí "no lo probé" antes que "funciona".

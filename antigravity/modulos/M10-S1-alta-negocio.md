# M10 Alta de negocio: registro y onboarding (S1)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Crea el archivo `supabase/migrations/20261109000100_salida.sql` con el bloque `-- S1 Alta de negocio` y la función `create_business`. |
| Commit | `feat(fase-s): S1 alta de negocio` |
| Reporte | Sección "M10" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M10 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M10-S1-alta-negocio.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Antes de tocar código, creá la rama de la fase en los dos repositorios, a partir de feat/fase-3-clientes:
   git checkout feat/fase-3-clientes && git checkout -b feat/fase-s-salida
   (en nexa-backend y en nexa-frontend). Si la rama ya existe, solo hacé checkout.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M10" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M11. Escribime "M10 terminado".
```

## Especificación

### Flujo

1. La persona entra a `/registro`, escribe su nombre, correo y contraseña, y recibe un correo de confirmación.
2. El enlace del correo la lleva a `/onboarding`, ya con sesión.
3. En `/onboarding` escribe el nombre del negocio, el de la sucursal y su nombre. El backend crea el negocio, la sucursal y su membresía de `DUENO`.
4. Los triggers que ya existen crean la bodega "Principal", la "Caja 1" y las categorías de gasto. **No los dupliques.**
5. Ve la pantalla "Tu negocio está listo" con los primeros pasos.

### Migración

```sql
create or replace function public.create_business(
  p_business_name text,
  p_branch_name text,
  p_full_name text
)
returns table (tenant_id uuid, branch_id uuid)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_tenant uuid;
  v_branch uuid;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  -- Dos llamadas simultáneas del mismo usuario no crean dos negocios.
  perform pg_advisory_xact_lock(hashtext('create_business:' || v_uid::text));

  if exists (select 1 from public.memberships m where m.user_id = v_uid and m.active) then
    raise exception 'ONBOARDING_ALREADY_MEMBER';
  end if;

  select u.email into v_email from auth.users u where u.id = v_uid;

  insert into public.users (id, full_name, email)
  values (v_uid, p_full_name, v_email)
  on conflict (id) do update set full_name = excluded.full_name;

  insert into public.tenants (name) values (p_business_name)
  returning id into v_tenant;

  insert into public.branches (tenant_id, name) values (v_tenant, p_branch_name)
  returning id into v_branch;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_tenant, null, public.tipo_id('ROL', 'DUENO'), true);

  return query select v_tenant, v_branch;
end;
$$;
revoke all on function public.create_business(text, text, text) from public, anon;
grant execute on function public.create_business(text, text, text) to authenticated;
```

La función no valida longitudes: eso lo hace el backend antes de llamarla (abajo). `#variable_conflict use_column` evita que los nombres de salida (`tenant_id`, `branch_id`) choquen con las columnas dentro de la función. Claude no probó esta función contra una base: las pruebas de S1 la prueban.

### Endpoint `POST /v1/onboarding` (`src/onboarding.rs`)

Sin permiso: el usuario todavía no tiene negocio. Requiere sesión (`AuthUser`). Cuerpo: `{business_name, branch_name, full_name}`.

1. Recorta los espacios de los tres campos. Valida, en este orden, contando caracteres con `chars().count()`:
   - `business_name` de 2 a 80: si no, 422 `ONBOARDING_INVALID`, con `data: {field: "business_name"}`;
   - `branch_name` de 2 a 60: si no, 422 `ONBOARDING_INVALID`, con `data: {field: "branch_name"}`;
   - `full_name` de 2 a 80: si no, 422 `ONBOARDING_INVALID`, con `data: {field: "full_name"}`.
2. Abre la transacción con `db::begin_scope_tx(pool, claims, None, None)`. **No** uses `tenancy::begin`: falla sin membresía.
3. `select tenant_id, branch_id from public.create_business($1, $2, $3)`.
   - Si el error de la base trae el mensaje `ONBOARDING_ALREADY_MEMBER`: 409 `ONBOARDING_ALREADY_MEMBER`.
   - Si trae `ONBOARDING_NO_SESSION`: 401.
   - Lee el mensaje con `sqlx::Error::Database(e)` y `e.message()`. Cualquier otro error sigue el camino normal.
4. En la misma transacción:
   - `db::set_scope(&mut tx, tenant, Some(branch)).await?`;
   - arma `Scope { user: claims.sub, tenant, branch: Some(branch), role: "DUENO".into(), permissions: permissions::load_effective_permissions_tx(&mut tx, tenant, "DUENO").await? }`;
   - registra la auditoría `NEGOCIO_CREADO`, con la entidad `tenants`, el id `tenant`, `before = None` y `after = {business_name, branch_name}`.
5. `commit`. Respuesta 201: `{tenant_id, branch_id}`.

Registra la ruta en `routes.rs` y documéntala en `README.md`. En la sección "Conectar Supabase", paso 4, agrega antes del SQL: "Desde la fase S, el dueño crea su negocio en `/registro`. Este SQL queda solo para pruebas."

### Frontend

**`/registro`** (pantalla 09): `app/(publico)/registro/page.tsx` y `components/auth/register-screen.tsx`, con `AuthShell`, `FieldLabel`, `inputClass` y `primaryButtonClass` de `auth-shell.tsx`. En `lib/screens.ts`, la pantalla 09 pasa a `"react"`.
- Título "Creá tu cuenta" y subtítulo "Después configurás tu negocio en un minuto".
- Campos, en este orden: "Tu nombre", "Correo", "Contraseña" y "Repetí la contraseña". Los dos de contraseña tienen el botón de mostrar, igual que el login.
- Validación local, en este orden. Cada error se muestra debajo de su campo:
  1. nombre de 2 a 80 caracteres: "Escribí tu nombre";
  2. correo con `@`: "Revisá el correo";
  3. contraseña de 8 caracteres o más: "La contraseña debe tener al menos 8 caracteres";
  4. las dos contraseñas iguales: "Las contraseñas no coinciden".
- Botón "Crear cuenta". Llama a `getSupabase().auth.signUp({ email, password, options: { emailRedirectTo: `${window.location.origin}/onboarding`, data: { full_name } } })`.
- Si Supabase responde un error con el código `user_already_exists`, o con el texto `User already registered`: "Ese correo ya tiene cuenta." con el enlace "Iniciá sesión" a `/login`. Cualquier otro error: "No pudimos crear la cuenta. Intentá de nuevo en unos minutos."
- Si funciona: reemplaza el formulario por el ícono `MailCheck` y el texto "Te enviamos un correo a <correo>. Abrí el enlace para seguir. Si no llega, revisá la carpeta de spam."
- Abajo: "¿Ya tenés cuenta? Iniciá sesión", con enlace a `/login`.
- **Modo demo** (`AUTH_ENABLED` falso): el botón lleva directo a `/onboarding`, sin llamar a Supabase.

**`/onboarding`** (pantalla 12): `app/(publico)/onboarding/page.tsx` y `components/auth/onboarding-screen.tsx`. En `lib/screens.ts`, la pantalla 12 pasa a `"react"`. `/onboarding` ya está en `PUBLIC_PREFIXES`, así que la pantalla controla la sesión:
- Al montar, llama a `useSession().boot()` y espera el estado:
  - `signed-out`: `router.replace("/login?next=/onboarding")`;
  - `ready` o `choose-branch`: `router.replace("/panel")` (ya tiene negocio);
  - `no-business`: muestra el formulario;
  - `error`: muestra el error con el botón "Reintentar", como `SessionGate`.
- Título "Configurá tu negocio". Campos, en este orden:
  - "Nombre del negocio" (ayuda: "Así aparece en la tirilla. Ej.: Licorera La 15");
  - "Nombre de la sucursal" (vale "Principal" al abrir);
  - "Tu nombre" (vale `useSession().name` al abrir).
- Botón "Crear negocio". Llama a `api.onboarding.create(...)`.
  - Con 422 `ONBOARDING_INVALID`, el mensaje va debajo del campo de `data.field`.
  - Con 409 `ONBOARDING_ALREADY_MEMBER`: `router.replace("/panel")`.
- Si funciona: `await useSession().boot()` y muestra la pantalla "Tu negocio está listo", con 4 pasos. Cada uno es un enlace que se abre en la misma pestaña:
  1. "Completá los datos del negocio" → `/configuracion/negocio`;
  2. "Cargá tus productos" → `/inventario/importar`;
  3. "Invitá a tus cajeros" → `/configuracion/usuarios`;
  4. "Abrí la caja y vendé" → `/pos`.

  Y el botón "Ir al panel" → `/panel`.

**`SessionGate`**, estado `no-business`: agrega, antes del botón "Salir", un botón principal "Crear mi negocio" que lleva a `/onboarding`.

**API y mock:**
- `api.onboarding.create({business_name, branch_name, full_name}): Promise<{tenant_id, branch_id}>`, en `lib/api/index.ts`;
- el mock va en `lib/api/mock-onboarding.ts`: aplica las mismas validaciones de longitud y devuelve dos `crypto.randomUUID()`.

**Errores** (`ERROR_MESSAGES`):

| Código | Texto |
|---|---|
| `ONBOARDING_INVALID` | Revisá los datos del negocio. |
| `ONBOARDING_ALREADY_MEMBER` | Tu usuario ya tiene un negocio. |

### Pruebas (`s1_onboarding`)

Usa `test_support::seed_user` para crear usuarios sin membresía. Ese helper ya crea la fila en `public.users`, así que también prueba el `on conflict` de la función.

- **[P]** Un usuario sin membresía crea su negocio. Quedan un `tenant`, una `branch`, una membresía `DUENO` con `branch_id` nulo y `active = true`, una `location` "Principal" con `is_default = true`, una `cash_register` `CAJA-1` y la fila `NEGOCIO_CREADO` en `audit_log`.
- **[P]** Después del alta, `tenancy::begin` con ese usuario funciona y su rol es `DUENO`.
- **[P]** La segunda llamada del mismo usuario: 409 `ONBOARDING_ALREADY_MEMBER`. Sigue habiendo un solo negocio de ese usuario.
- **[P]** Un usuario con una membresía `CAJERO` activa en otro negocio: 409 `ONBOARDING_ALREADY_MEMBER`.
- **[P]** `business_name = " a "`: 422 `ONBOARDING_INVALID`, con `field = "business_name"`.
- **[P]** El rol `anon` no puede ejecutar `public.create_business` (error de permiso).
- **[P]** El negocio creado no ve datos de otro negocio: un `GET /v1/customers` con el dueño nuevo devuelve 0 filas aunque otro negocio tenga clientes.

### Lo que hace Claude (no Antigravity)

Configurar en Supabase la *Site URL* y las *Redirect URLs* con el dominio del VPS, el correo SMTP propio (el de Supabase solo envía unos pocos correos por hora) y la protección de contraseñas filtradas. No lo intentes ni lo dejes "preparado".

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M10`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

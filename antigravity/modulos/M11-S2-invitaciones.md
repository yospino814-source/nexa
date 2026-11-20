# M11 Invitaciones que funcionan (S2)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Agrega **al final** de `20261109000100_salida.sql` el bloque `-- S2 Invitaciones`, con el SQL de este módulo. |
| Commit | `feat(fase-s): S2 invitaciones` |
| Reporte | Sección "M11" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M11 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M11-S2-invitaciones.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M11" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M12. Escribime "M11 terminado".
```

## Especificación

### Problema (comprobado en el código)

- `POST /v1/users/invite` solo guarda una fila en `invitations`. No se envía ningún correo y **nada convierte la invitación en membresía**: un cajero invitado nunca puede entrar.
- El frontend envía `{email, role, branch_ids: [...]}` (`users-screen.tsx:118`), y el backend espera `{email, full_name, role, branch_id}` (`users.rs:46`). Con datos reales, invitar falla siempre por falta de `full_name`.
- Se pueden invitar los roles `MESERO`, `COCINA` y `REPARTIDOR`, cuyas pantallas son de las fases 5 y 6 (pospuestas). Ese usuario entraría a una pantalla que no existe.

### Migración

```sql
alter table public.invitations
  add constraint invitations_status_check check (status in ('PENDIENTE','ACEPTADA','REVOCADA'));

create or replace function public.my_invitations()
returns table (id uuid, tenant_name text, branch_name text, role_code text, role_name text, created_at timestamptz)
language sql
stable
security definer
set search_path = ''
as $$
  select i.id, t.name, b.name, r.tima_codigo, r.tima_nombre, i.created_at
    from public.invitations i
    join public.tenants t on t.id = i.tenant_id
    left join public.branches b on b.id = i.branch_id
    join public.tbl_tipos_maestra r on r.tima_id = i.role_id
    join auth.users u on u.id = auth.uid()
   where i.status = 'PENDIENTE'
     and u.email_confirmed_at is not null
     and lower(i.email) = lower(u.email)
   order by i.created_at
$$;
revoke all on function public.my_invitations() from public, anon;
grant execute on function public.my_invitations() to authenticated;

create or replace function public.accept_invitation(p_id uuid)
returns table (out_tenant_id uuid, out_branch_id uuid, out_role_code text)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_confirmed timestamptz;
  v_inv public.invitations%rowtype;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  select u.email, u.email_confirmed_at into v_email, v_confirmed
    from auth.users u where u.id = v_uid;

  select * into v_inv from public.invitations i where i.id = p_id for update;

  -- El mismo error si no existe o es de otro correo: no se revela que existe.
  if not found or v_confirmed is null or lower(v_inv.email) <> lower(v_email) then
    raise exception 'INVITATION_NOT_FOUND';
  end if;
  if v_inv.status <> 'PENDIENTE' then
    raise exception 'INVITATION_NOT_PENDING';
  end if;

  insert into public.users (id, full_name, email)
  values (v_uid, v_inv.full_name, v_email)
  on conflict (id) do nothing;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_inv.tenant_id, v_inv.branch_id, v_inv.role_id, true)
  on conflict (user_id, tenant_id, branch_id)
  do update set role_id = excluded.role_id, active = true;

  update public.invitations set status = 'ACEPTADA', accepted_at = now() where id = p_id;

  return query
    select v_inv.tenant_id, v_inv.branch_id, r.tima_codigo
      from public.tbl_tipos_maestra r where r.tima_id = v_inv.role_id;
end;
$$;
revoke all on function public.accept_invitation(uuid) from public, anon;
grant execute on function public.accept_invitation(uuid) to authenticated;
```

### Roles disponibles para la salida

Lista cerrada, en la constante `users::ASSIGNABLE_ROLES = ["ADMIN_CAJA", "SUPERVISOR", "CAJERO", "BODEGA"]`. `DUENO` solo lo asigna otro `DUENO`, como hoy.

### Backend

**`POST /v1/users/invite`** (`users.rs::invite_user`). El cuerpo **no cambia**: `{email, full_name, role, branch_id}`. Validaciones en este orden:
1. Las de hoy (correo con `@`, `full_name` no vacío). Además, `full_name` debe tener de 2 a 80 caracteres: si no, el mismo error de validación de hoy.
2. Permiso `users:manage` (como hoy).
3. Si `role` es `DUENO` y quien invita no es dueño: 403, como hoy. Si `role` no está en `ASSIGNABLE_ROLES` y no es `DUENO`: 422 `ROLE_NOT_AVAILABLE`.
4. Si ese correo ya tiene una membresía **activa** en el negocio (`memberships` unida con `users` por `users.email`, en minúsculas): 409 `USER_ALREADY_MEMBER`.
5. `branch_id`, si viene, debe ser del negocio (como hoy).
6. El `insert ... on conflict` de hoy.
7. Auditoría `USUARIO_INVITADO`, con `{email, role, branch_id}`.

**`PUT /v1/users/{id}/role`:** agrega la misma regla 3, antes de las validaciones que ya tiene.

**`GET /v1/users`:** `invitations` devuelve solo las `PENDIENTE`.

**`POST /v1/users/invitations/{id}/revoke`** (permiso `users:manage`, sin cuerpo):
1. La busca con `for update` en el negocio del usuario. Si no existe: 404.
2. Si es de rol `DUENO` y quien revoca no es dueño: 403.
3. Si no está `PENDIENTE`: 409 `INVITATION_NOT_PENDING`.
4. `status = 'REVOCADA'`. Auditoría `INVITACION_REVOCADA`. Respuesta 204.

**`GET /v1/invitations/mine`** (`src/invitations.rs`, sin permiso, con sesión): abre la transacción con `db::begin_scope_tx(pool, claims, None, None)`, devuelve `select * from public.my_invitations()` como `[{id, tenant_name, branch_name, role_code, role_name, created_at}]` y hace `commit`.

**`POST /v1/invitations/{id}/accept`** (sin permiso, con sesión, sin cuerpo):
1. `db::begin_scope_tx(pool, claims, None, None)`.
2. `select out_tenant_id, out_branch_id, out_role_code from public.accept_invitation($1)`. Traduce el mensaje del error de la base, igual que en S1: `INVITATION_NOT_FOUND` → 404 `INVITATION_NOT_FOUND`; `INVITATION_NOT_PENDING` → 409 `INVITATION_NOT_PENDING`; `ONBOARDING_NO_SESSION` → 401.
3. `db::set_scope(&mut tx, tenant, branch).await?`. Arma el `Scope` con `role = out_role_code` y `permissions::load_effective_permissions_tx(&mut tx, tenant, &role)`.
4. Auditoría `INVITACION_ACEPTADA`, con la entidad `invitations` y el id de la invitación.
5. `commit`. Respuesta 200: `{tenant_id, branch_id, role}`.

Documenta en `README.md` las rutas nuevas y las que cambian.

### Frontend

**Modal "Invitar usuario"** (`users-screen.tsx`):
- Campos, en este orden: "Nombre" (obligatorio), "Correo", "Rol" y "Sucursal".
- "Rol" muestra solo `ASSIGNABLE_ROLES` (constante igual en `lib/stores/session.ts`), más `DUENO` si el usuario actual es dueño. Cada opción usa `ROLE_LABELS` y `ROLE_DESCRIPTIONS`, que ya existen.
- "Sucursal" tiene "Todas las sucursales" (envía `branch_id: null`) y una opción por sucursal (envía su id).
- `api.users.invite` envía exactamente `{email, full_name, role, branch_id}`. Corrige `InviteUserInput` en `types.ts` y el mock.
- Los errores se muestran con `friendlyError`, no con `err.message`.
- Si funciona, el modal cambia a "Invitación creada", con:
  - el texto del mensaje (abajo), en un recuadro de solo lectura;
  - el botón "Copiar mensaje" (`navigator.clipboard.writeText`; el botón pasa a decir "Copiado");
  - el botón "Enviar por WhatsApp", que abre en una pestaña nueva `https://wa.me/?text=` más el mensaje codificado con `encodeURIComponent`;
  - el botón "Listo", que cierra.
- **Texto exacto del mensaje:** `Hola <full_name>, te invité a <nombre del negocio> en Nexa POS como <ROLE_LABELS[role]>. Para entrar, creá tu cuenta con este correo: <email>, en <origin>/registro?correo=<email codificado>. Si ya tenés cuenta, solo iniciá sesión.` El nombre del negocio sale de `GET /v1/business` y `<origin>` de `window.location.origin`.

**Lista de invitaciones pendientes** (la que ya muestra `users-screen.tsx`): cada fila agrega "Copiar mensaje" (el mismo texto) y "Revocar". "Revocar" pide confirmación ("¿Revocar la invitación de <email>?") y llama a `api.users.revokeInvitation(id)`.

**`/registro`:** si la URL trae `?correo=`, el campo "Correo" se llena con ese valor.

**Sesión** (`lib/stores/session.ts`):
- el estado agrega `invitations: MyInvitation[]`, vacío al empezar;
- en `boot()`, con un usuario de Supabase y **antes** de pedir las sucursales, llama a `api.invitations.mine()`. Si falla, deja `[]` y sigue;
- acción nueva `acceptInvitation(id)`: llama a `api.invitations.accept(id)` y luego a `boot()`.

**`SessionGate`, estado `no-business`:**
- con `invitations.length > 0`: título "Te invitaron a trabajar en Nexa". Una tarjeta por invitación, con `<tenant_name>`, `<role_name>`, `<branch_name ?? "Todas las sucursales">` y el botón "Aceptar". Debajo: "¿Querés crear tu propio negocio?", con enlace a `/onboarding`, y el botón "Salir";
- sin invitaciones: lo de S1 ("Crear mi negocio" y "Salir").

**Usuario con negocio e invitaciones nuevas:** `UserMenu` agrega el ítem "Invitaciones (<n>)" solo si `invitations.length > 0`. Abre un diálogo con las mismas tarjetas.

**`/onboarding`:** si `invitations.length > 0`, muestra arriba un aviso amarillo: "Tenés <n> invitación(es) pendiente(s). Si vas a trabajar en un negocio que ya existe, aceptala en vez de crear uno nuevo." y el botón "Ver invitaciones", que lleva a `/panel` (ahí `SessionGate` las muestra).

**API y mock:** `api.invitations.mine()`, `api.invitations.accept(id)` y `api.users.revokeInvitation(id)`. El mock va en `lib/api/mock-invitations.ts`, con una invitación de ejemplo: "Licorera La 15", rol "Cajero", sucursal "Principal".

### Pruebas (`s2_invitations`)

El helper `test_support::seed_user` no llena `email_confirmed_at`. En estas pruebas, actualízalo con el pool de administrador: `update auth.users set email_confirmed_at = now() where id = $1`.

- **[P]** Invitar con `role = "MESERO"`: 422 `ROLE_NOT_AVAILABLE`.
- **[P]** Invitar a un correo que ya tiene una membresía activa en el negocio: 409 `USER_ALREADY_MEMBER`.
- **[P]** Un usuario con el correo confirmado ve la invitación en `my_invitations`, la acepta y queda con una membresía activa del rol y la sucursal invitados. La invitación queda `ACEPTADA`.
- **[P]** Un usuario con **otro** correo que intenta aceptar: 404 `INVITATION_NOT_FOUND`, y no se crea membresía.
- **[P]** Un usuario con el correo **sin confirmar** no la ve en `my_invitations`, y aceptar da 404.
- **[P]** Aceptar dos veces: la segunda da 409 `INVITATION_NOT_PENDING`.
- **[P]** Una invitación revocada no se puede aceptar: 409 `INVITATION_NOT_PENDING`.
- **[P]** `ADMIN_CAJA` no puede revocar una invitación de rol `DUENO`: 403.

## Errores de este módulo

Agrégalos al backend (`error::coded`) y a `ERROR_MESSAGES` del frontend, con estos textos exactos:

| Código | HTTP | Texto en pantalla |
|---|---|---|
| `ROLE_NOT_AVAILABLE` | 422 | Ese rol todavía no está disponible. |
| `USER_ALREADY_MEMBER` | 409 | Esa persona ya trabaja en tu negocio. |
| `INVITATION_NOT_FOUND` | 404 | No encontramos esa invitación para tu correo. |
| `INVITATION_NOT_PENDING` | 409 | Esa invitación ya no está vigente. |

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M11`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

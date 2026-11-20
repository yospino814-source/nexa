# M11A Corrección: invitaciones (revisión de Claude del 2026-11-10)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` (solo descartar cambios) y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Commit | Frontend: `fix(fase-s): S2 roles y mensaje de invitacion`. Backend: ninguno |
| Reporte | Sección "M11A" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M11A de Nexa POS: correcciones de M11. Hacé solo esto.

1. Leé completo antigravity/modulos/M11A-correccion-invitaciones.md.
2. Hacé las correcciones en el orden de "Qué hacer".
3. Corré la verificación.
4. Commit del frontend con el mensaje exacto del módulo. En el backend no hay commit.
5. Escribí la sección "M11A" en antigravity/REPORTE-FASE-S.md.
6. Si ya habías hecho M10A, seguí con M12. Si no, hacé M10A primero.
```

## Resultado de la revisión de M11

Claude corrió la verificación completa sobre el commit `a24f401`: `fmt`, `clippy` y **44 de 44 pruebas con la base de datos real** pasan.

Está bien:
- la migración (copiada tal cual);
- `invite_user` con las validaciones en orden;
- `revoke_invitation`, `my_invitations` y `accept_invitation`;
- las 8 reglas [P] de `s2_invitations`;
- el modal "Invitar usuario", con el mensaje exacto y los botones "Copiar mensaje" y "Enviar por WhatsApp";
- `/registro` con `?correo=` y envuelto en `Suspense`;
- la sesión, `SessionGate`, `UserMenu` y el aviso en `/onboarding`.

El cambio en `00_supabase_shim.sql` (columna `email_confirmed_at`) y el de `seed_user` son necesarios y se aceptan: Supabase real ya tiene esa columna.

## Qué hacer

### 1. Backend: descarta el cambio sin commit que rompe la compilación

En `nexa-backend` quedaron, sin commit:
- `src/main.rs`, que declara `pub mod invitations;` **por segunda vez** (líneas 12 y 14). Con eso, el backend **no compila**;
- `src/invitations.rs`, que solo agrega una línea vacía al final.

Descártalos (son tuyos, no hay nada que conservar):

```sh
cd nexa-backend && git restore src/main.rs src/invitations.rs && git status --short
```

`git status --short` debe salir vacío.

### 2. Frontend: "Cambiar rol" solo ofrece los roles disponibles

`components/configuracion/users-screen.tsx`, línea 814. El diálogo "Seleccionar nuevo rol" recorre `Object.keys(ROLE_LABELS)`, así que ofrece Mesero, Cocina y Repartidor (el backend responde `ROLE_NOT_AVAILABLE`), y a un `ADMIN_CAJA` también le ofrece Dueño (el backend responde 403).

Reemplaza `(Object.keys(ROLE_LABELS) as Role[])` por `availableRoles` (definida en la línea 58). Así quedan `ASSIGNABLE_ROLES`, más `DUENO` solo si el usuario es dueño.

### 3. Frontend: pestaña "Roles y permisos" sin roles pospuestos

Misma pantalla, línea 507. La grilla de roles recorre `Object.keys(ROLE_LABELS)` y muestra Mesero, Cocina y Repartidor, que no tienen pantallas (regla 6). Reemplaza esa lista por `ASSIGNABLE_ROLES`. El dueño no va en la grilla: sus permisos son siempre todos y no se editan.

### 4. Frontend: el mensaje copiado desde la lista trae el nombre

Misma pantalla, línea 469. El mensaje de "Copiar mensaje" en la lista de invitaciones pendientes empieza con "Hola, te invité…", sin el nombre. Tiene que ser el mismo texto exacto del modal:

```ts
const msg = `Hola ${inv.full_name ?? ""}, te invité a ${businessName} en Nexa POS como ${roleLabel}. Para entrar, creá tu cuenta con este correo: ${inv.email}, en ${origin}/registro?correo=${encodeURIComponent(inv.email)}. Si ya tenés cuenta, solo iniciá sesión.`;
```

Si `inv.full_name` está vacío, el texto queda "Hola , te invité…". Para evitarlo, usa `` `Hola${inv.full_name ? " " + inv.full_name : ""}, te invité…` ``.

### 5. Frontend: sucursal de la invitación sin `branch_ids`

Misma pantalla, líneas 450 a 453. Todavía usan `inv.branch_ids`, que es el formato viejo que el backend nunca devolvió. La sucursal se muestra así: `inv.branch_name ?? "Todas las sucursales"`. Borra `branch_ids` del tipo `Invitation` en `lib/api/types.ts` y de los mocks que lo usen.

## Verificación

```sh
cd nexa-backend && git status --short           # vacío
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
grep -n "Object.keys(ROLE_LABELS)" components/configuracion/users-screen.tsx   # vacío
grep -rn "branch_ids" lib components                                           # vacío
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M11A`:
1. Hash del commit del frontend.
2. La salida de los comandos de verificación.

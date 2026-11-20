# M13 Permisos y navegación por rol (S4)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Agrega **al final** de `20261109000100_salida.sql` el bloque `-- S4 Permisos`, con el SQL de S4.1. |
| Commit | `feat(fase-s): S4 permisos y navegacion por rol` |
| Reporte | Sección "M13" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M13 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M13-S4-permisos-navegacion.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M13" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M14. Escribime "M13 terminado".
```

## Especificación

Sale de simular cada rol (ver `antigravity/flujos-por-rol.md`). Corrige los casos en que un rol ve pantallas que no puede usar, o no ve las que necesita.

### S4.1 Compras con sus propios permisos (backend)

Hoy todo el flujo de compras se valida con `Action::ManageInventory` (`inventory:adjust`). Por eso `BODEGA` y `SUPERVISOR` pueden crear **y aprobar** compras, y `purchases:approve` no se usa en ningún lado.

1. Agrega a `tenancy::Scope` el método `require_any(&self, codes: &[&str]) -> Result<(), ApiError>`. Pasa si `has_permission` es verdadero para al menos uno; si no, `Forbidden`.
2. En `procurement.rs`, reemplaza `scope.require(Action::ManageInventory)?` así:

| Funciones | Permiso |
|---|---|
| `list_purchases`, `get_purchase`, `purchase_summary`, `cost_preview` | `scope.require_any(&["purchases:create", "purchases:approve", "purchases:receive"])?` |
| `create_purchase`, `update_purchase`, `cancel_purchase` | `purchases:create` |
| `approve_purchase` | `purchases:approve` |
| `receive_purchase`, `receive_purchase_with_req` | `purchases:receive` |

   Cambia la función pública de cada fila y también su `_tx`, si la `_tx` tiene su propio `require`. En `procurement.rs` no debe quedar ningún `Action::ManageInventory`.
3. Migración:

```sql
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, 'purchases:receive'
  from public.tbl_tipos_maestra r
 where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
   and r.tima_codigo = 'SUPERVISOR'
on conflict do nothing;
```

4. `tenancy::base_allows`: agrega `purchases:receive` a `SUPERVISOR`.

### S4.2 Devolución aprobada a distancia (backend)

Hoy, al aprobar una devolución en efectivo, `refunds.rs::apply_effects_tx` saca el dinero del turno de **quien aprueba** (`cash::open_session_tx(tx, scope, branch)`). En una tienda, el cajero pide la devolución y el dueño la aprueba desde el celular, sin caja abierta: falla con `REFUND_NEEDS_OPEN_SESSION`. Si el dueño sí tiene caja abierta, el dinero se descuenta de su turno y no del cajón del cajero.

1. Función nueva `cash::open_session_of_tx(tx, branch_id, user_id) -> Result<Option<Uuid>, ApiError>`: la misma consulta de `open_session_tx`, pero con `opened_by = user_id`.
2. En `apply_effects_tx`, cuando `cash_out` es verdadero, el turno se elige en este orden:
   1. el turno abierto de `refunds.requested_by` en la sucursal;
   2. si no hay, el turno abierto de quien aprueba;
   3. si tampoco hay: 409 `REFUND_NEEDS_OPEN_SESSION`, como hoy.
3. `refunds.cash_session_id` guarda el turno usado.

### S4.3 Permiso por pantalla (frontend)

Archivo nuevo `lib/route-permissions.ts`, con esta tabla exacta. Gana el prefijo más largo, y el usuario necesita **al menos uno** de los permisos de la fila:

| Prefijo | Permisos |
|---|---|
| `/panel` | `reports:read` |
| `/ventas` | `reports:read` |
| `/pos` | `pos:sell` |
| `/caja` | `cash:movement` |
| `/inventario` | `inventory:adjust` |
| `/inventario/vencimientos` | `inventory:read` |
| `/compras` | `purchases:create`, `purchases:approve`, `purchases:receive` |
| `/compras/proveedores` | `purchases:create` |
| `/compras/por-pagar` | `purchases:pay` |
| `/ordenes` | `cash:history` |
| `/ordenes/devoluciones` | `pos:refund` |
| `/gastos` | `expenses:create`, `expenses:approve` |
| `/clientes` | `customers:read` |
| `/cartera` | `customers:read` |
| `/marketing` | `settings:write` |
| `/configuracion` | `settings:write` |
| `/configuracion/usuarios` | `users:manage` |
| `/configuracion/auditoria` | `reports:read` |

Exporta `canOpen(pathname: string, can: (code: string) => boolean): boolean`. Una ruta sin fila en la tabla se puede abrir.

Componente nuevo `components/auth/route-guard.tsx`, usado **dentro** de `SessionGate` en `app/(admin)/layout.tsx` y en `app/(operacion)/layout.tsx`. Si `!canOpen(pathname, can)`, en vez de la pantalla muestra `AuthShell` con el título "No tenés permiso para ver esta pantalla", el texto "Pedile al dueño que te dé acceso." y el botón "Ir a mi inicio", que lleva a `homeFor(role)`.

### S4.4 Menú lateral (`components/nexa/sidebar.tsx`)

1. **Borra** `isAllowedForRole`. El menú se filtra **solo** con `canOpen(item.href, can)`.
2. **Borra** los ítems "Restaurante" (`/mesas`) y "Rutas y mayorista" (`/rutas`): son de las fases 5 y 6.
3. Agrega, después de "Clientes", el ítem "Cartera" (`/cartera`, ícono `Wallet` de `lucide-react`).
4. `const role = currentBranch?.role ?? null`. Si es `null`, el menú no muestra ítems.
5. "Cerrar sesión" deja de ser un `Link` a `/login`, que hoy **no cierra la sesión**. Pasa a ser un `button` que llama a `await signOut()` de `useSession()` y luego a `router.replace("/login")`.
6. El logo lleva a `homeFor(role)`, no a `/panel` fijo.

Resultado esperado del menú (lo revisa Claude con usuarios reales):

| Rol | Ítems |
|---|---|
| `DUENO`, `ADMIN_CAJA` | Panel, POS, Inventario, Compras, Ventas y órdenes, Clientes, Cartera, Marketing, Configuración |
| `SUPERVISOR` | Panel, POS, Inventario, Compras, Ventas y órdenes, Clientes, Cartera |
| `CAJERO` | POS, Ventas y órdenes, Clientes, Cartera |
| `BODEGA` | Inventario, Compras |

### S4.5 Inicio por rol (`lib/stores/session.ts`)

```ts
export function homeFor(role: Role | null): string {
  if (role === "CAJERO") return "/pos";
  if (role === "BODEGA") return "/inventario";
  return "/panel";
}
```

En `SessionGate`, con `status === "ready"` y un rol `MESERO`, `COCINA` o `REPARTIDOR`, muestra `AuthShell` con el título "Tu rol todavía no tiene pantallas", el texto "Pedile al dueño que te asigne otro rol." y el botón "Salir". No muestra la app.

### S4.6 Barra de operación (`components/nexa/operation-topbar.tsx`)

1. **Borra** el botón "Bloquear terminal": no hace nada (regla 6). Para cambiar de cajero se usa "Cerrar sesión" del `UserMenu`.
2. El enlace "Panel Admin":
   - con `can("reports:read")`: texto "Panel" y `href="/panel"`;
   - si no, con `can("cash:history")`: texto "Ventas" y `href="/ordenes"`;
   - si no, no se muestra.
3. El indicador de caja ("Caja abierta desde…" o "Caja cerrada · abrir turno") se ve también en pantallas pequeñas. Por debajo de `md`, el texto es "Caja abierta" o "Caja cerrada". El `BranchSwitcher` sigue oculto por debajo de `md`.

### S4.7 Pantallas de diseño en producción

En `app/(referencia)/[...slug]/page.tsx`: si `process.env.NODE_ENV === "production"`, llama a `notFound()` antes de todo. Así, las pantallas que todavía son diseño no se ven en el VPS.

### Pruebas (`s4_permissions`)

- **[P]** `BODEGA` crea una compra: 403. `BODEGA` recibe una compra aprobada: 200.
- **[P]** `SUPERVISOR` aprueba una compra: 403. `SUPERVISOR` la recibe: 200.
- **[P]** `CAJERO` lista compras: 403.
- **[P]** `ADMIN_CAJA` crea, aprueba y recibe: 200 en las tres.
- **[P]** Un cajero con turno abierto pide una devolución en efectivo. El dueño, sin turno, la aprueba. El movimiento `DEVOLUCION` queda en el turno del cajero.
- La prueba de la matriz de roles de la fase 0 debe seguir pasando. Si cambia por S4.1, explícalo en el reporte.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-backend
grep -n "Action::ManageInventory" src/procurement.rs                                   # vacía
grep -n "isAllowedForRole\|/mesas\|/rutas" ../nexa-frontend/components/nexa/sidebar.tsx   # vacía
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M13`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

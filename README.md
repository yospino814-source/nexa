# nexa-backend

API de Nexa POS en Rust (Axum + sqlx) sobre Supabase/PostgreSQL, multi-tenant con Row-Level Security.

## Correr

```sh
cp .env.example .env          # DATABASE_URL y SUPABASE_URL
supabase db push              # aplica supabase/migrations
cargo run                     # escucha en :8080 (PORT para cambiarlo)
```

| Variable | Uso |
|---|---|
| `DATABASE_URL` | Conexión a Postgres. El pool ejecuta como rol `authenticated`, así que RLS siempre aplica. |
| `SUPABASE_URL` | Base del proyecto Supabase: de ahí salen el JWKS y el emisor (`iss`) que se validan. |
| `PORT` | Puerto HTTP (por defecto 8080). |
| `DB_MAX_CONNECTIONS` | Tamaño del pool (por defecto 5). |

## Conectar Supabase

1. **Claves de firma asimétricas.** En *Project Settings → JWT Keys*, el proyecto debe usar *JWT Signing Keys* (ES256/RS256). El backend valida los tokens contra el JWKS y rechaza `HS256`. Un proyecto que firma con el *JWT secret* legacy recibe 401 en todo.
2. **Backend:** en `nexa-backend/.env` van `SUPABASE_URL` (`https://<ref>.supabase.co`) y `DATABASE_URL` (*Connect → Session pooler*).
3. **Frontend:** en `nexa-frontend/.env.local` van `NEXT_PUBLIC_SUPABASE_URL` y `NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY`. Solo la clave pública, nunca la `service_role`.
4. **Usuario:** créalo en *Authentication → Users* y dale acceso a un negocio. Sin membresía, el login muestra "Tu usuario no tiene un negocio asignado". Ejecuta esto en el SQL editor:

```sql
-- Reemplaza el email. Crea negocio y sucursal de prueba y da rol DUENO.
with u as (select id from auth.users where email = 'tu@email.com'),
     p as (insert into public.users (id, full_name) select id, 'Dueño' from u
           on conflict (id) do nothing returning id),
     t as (insert into public.tenants (name) values ('Mi negocio') returning id),
     b as (insert into public.branches (tenant_id, name) select id, 'Principal' from t returning id, tenant_id)
insert into public.memberships (user_id, tenant_id, branch_id, role_id)
select u.id, b.tenant_id, null, public.tipo_id('ROL', 'DUENO') from u, b;
```

   Para un cajero de una sola sucursal, usa `branch_id` = id de la sucursal y el rol `CAJERO`.
5. **Opcional:** activa el hook `public.custom_access_token_hook` en *Authentication → Hooks*. Agrega al JWT el tenant y la sucursal activos. El backend funciona sin él, porque resuelve todo por membresía.

## Tests

```sh
./scripts/test.sh             # Postgres 17 + shim de Supabase + migraciones, todo en Docker
./scripts/test.sh clippy --all-targets
```

No hace falta tener Rust instalado, solo Docker. Sin `DATABASE_URL`, `cargo test` se salta las pruebas de integración.

## Cómo está organizado

- **Alcance (`tenancy.rs`).** Cada operación abre **una** transacción con `tenancy::begin` / `begin_in_branch`. Ahí se resuelve la membresía (tenant, sucursal y rol), se fija el contexto de RLS y se devuelve un `Scope`.
  - La sucursal sale de la membresía, nunca solo del JWT.
  - Una membresía con `branch_id` nulo cubre todo el negocio.
- **Funciones de dominio.** Cada operación existe como `*_tx(tx, scope, ..)`, que se puede componer con otras, y como envoltura que abre su propia transacción.
- **Roles.** `tenancy::role_allows` define la matriz de permisos:

  | Rol | Puede |
  |---|---|
  | `DUENO`, `ADMIN_CAJA` | Todo |
  | `CAJERO` | Leer, tomar órdenes, cobrar, caja |
  | `MESERO` | Leer, tomar órdenes |
  | `COCINA` | Leer, marcar órdenes como listas |

- **Precios sin IVA.** El impuesto se suma encima y se congela por línea (`order_items.tax_rate` y `tax_amount`).

## Endpoints

Todos, salvo `health`, exigen `Authorization: Bearer <JWT de Supabase>`. Los errores responden `{ "error": "..." }`:

| Código | Significado |
|---|---|
| 401 | Token faltante o inválido |
| 403 | Sin permiso o sucursal fuera de alcance |
| 404 | No existe |
| 409 | Estado inválido: sin turno abierto, sin stock, id reutilizado |
| 422 | Datos inválidos |

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/v1/health`, `/v1/health/db` | Salud del servicio y de la BD |
| GET | `/v1/context` | uid, tenant, sucursal y rol resueltos |
| GET | `/v1/branches` | Sucursales del usuario con su rol en cada una |
| GET | `/v1/catalog?branch_id=` | Catálogo con precio, impuesto y disponible |
| GET | `/v1/products/{id}/recipe` | Insumos de un producto compuesto |
| POST | `/v1/sales` | Venta de mostrador idempotente (ver abajo) |
| GET | `/v1/cash-registers?branch_id=` | Listar cajas registradoras de la sucursal |
| POST | `/v1/cash-registers` | Crear caja `{branch_id, name, code, default_opening}` |
| PUT | `/v1/cash-registers/{id}` | Editar caja `{name, code, default_opening, active}` |
| POST | `/v1/cash-sessions` | Abrir turno `{branch_id, register_id, opening_amount}` |
| GET | `/v1/cash-sessions/current?branch_id=` | Turno abierto con lo esperado por método de pago, o `null` |
| POST | `/v1/cash-sessions/{id}/close` | Cerrar con arqueo `{counted_cash, notes?}` |
| POST | `/v1/cash-sessions/{id}/movements` | Ingreso o retiro de efectivo `{kind: "INGRESO" \| "RETIRO", amount, reason}`; un retiro no puede superar lo esperado en gaveta |
| GET | `/v1/cash-sessions/{id}/activity` | Bitácora del turno: apertura, ventas, ingresos, retiros y cierre |
| GET | `/v1/cash-sessions?branch_id=&limit=` | Historial de turnos con arqueo (un cajero ve solo los suyos) |
| POST | `/v1/orders` | Orden de mesa, domicilio o mayorista |
| GET | `/v1/orders/{id}` | Leer orden |
| POST | `/v1/orders/{id}/lines` | Agregar línea `{product_id, qty}` |
| POST | `/v1/orders/{id}/confirm` | Reservar stock (pasa a `EN_PREPARACION`) |
| POST | `/v1/orders/{id}/transition` | `{to: "LISTA" \| "ENTREGADA" \| "COBRADA" \| "CERRADA"}` |
| POST | `/v1/orders/{id}/cancel` | Cancelar y liberar reservas y mesa |
| GET | `/v1/inventory/available?branch_id=&product_id=` | Disponible de un producto |
| GET | `/v1/inventory/stock?branch_id=` | Existencias con punto de reorden y estado `OK` / `BAJO` / `AGOTADO` |
| GET | `/v1/inventory/movements?branch_id=&product_id=&limit=` | Kardex, más reciente primero, con `delta` y saldo resultante |
| POST | `/v1/inventory/replenish`, `/v1/inventory/adjust` | `{branch_id, product_id, qty, cause}` (el ajuste crea la fila de stock si no existía) |
| POST | `/v1/inventory/count` | Conteo físico `{branch_id, product_id, counted, cause}`; registra la diferencia |
| POST | `/v1/inventory/transfer` | Traslado entre sucursales `{from_branch_id, to_branch_id, product_id, qty, cause}`; exige permiso en ambas |
| PUT | `/v1/inventory/min-qty` | Punto de reorden `{branch_id, product_id, min_qty}` |
| GET | `/v1/suppliers?q=&include_inactive=` | Lista de proveedores del negocio |
| POST | `/v1/suppliers` | Crear proveedor `{name, tax_id?, contact_name?, phone?, email?, address?, notes?}` |
| PUT | `/v1/suppliers/{id}` | Editar proveedor, incluido `active` |
| GET | `/v1/purchases?branch_id=&from=&to=&supplier_id=&status=&limit=` | Historial de compras con número de ítems y total |
| POST | `/v1/purchases` | Crear compra en estado `PENDIENTE` (idempotente) |
| GET | `/v1/purchases/summary?branch_id=&month=YYYY-MM` | Total comprado en el mes (solo `RECIBIDA`) y proveedores activos |
| GET | `/v1/purchases/{id}` | Detalle de compra con ítems, producto y SKU |
| POST | `/v1/purchases/{id}/receive` | Pasa compra a `RECIBIDA`, suma stock, registra kardex y actualiza `cost_price` |
| POST | `/v1/purchases/{id}/cancel` | Anula compra (solo desde `PENDIENTE`) |
| GET | `/v1/permissions` | Catálogo de permisos del sistema y roles |
| GET | `/v1/permissions/roles/{role_id}` | Permisos efectivos de un rol (base + overrides de tenant) |
| PUT | `/v1/permissions/roles/{role_id}` | Actualizar overrides de permisos de un rol (solo `DUENO`) |
| GET | `/v1/settings?branch_id=` | Ajustes operativos de sucursal (POS, caja, inventario, propinas, impresión) |
| PUT | `/v1/settings?branch_id=` | Modificar ajustes operativos de sucursal |
| GET | `/v1/audit?from=&to=&user_id=&action=&limit=` | Registro de auditoría inmutable |
| GET | `/v1/users` | Lista de usuarios y membresías del negocio |
| POST | `/v1/users/invite` | Invitar usuario por email con rol y sucursales asignadas |
| PUT | `/v1/users/{id}/role` | Modificar rol de usuario (protege al dueño de auto-degradarse) |
| PUT | `/v1/users/{id}/status` | Activar o desactivar acceso de un usuario |
| POST | `/v1/auth/login-attempt` | Registrar o verificar intento de login; tras 5 fallos en 15 min responde 429 |

### `POST /v1/sales`

Todo ocurre en una transacción:

1. Exige un turno de caja abierto.
2. Toma precio e impuesto del servidor; el cliente solo manda `product_id` y `qty`.
3. Descuenta el stock directo, sin reserva.
4. Registra los pagos:
   - Puede haber varios, con un máximo de un pago en efectivo.
   - Los pagos que no son en efectivo no pueden superar el total.
   - El efectivo genera el vuelto.
5. Asigna el consecutivo del comprobante. No deja huecos: si la venta falla, el número no se consume.
6. Encola el documento equivalente electrónico en `fiscal_documents` como `PENDIENTE`.

**Idempotencia.** El cliente genera `id` (UUID) y lo reusa en cada reintento:
- Mismo cuerpo: 200 con la venta ya registrada.
- Otro cuerpo: 409.
- Venta nueva: 201.

## Pendiente

- **Worker de facturación DIAN:** lee `fiscal_documents` en `PENDIENTE`, transmite al proveedor tecnológico y guarda el CUDE. Falta elegir proveedor.
- **Configurar numeración:** pantalla para cargar prefijo, rango y resolución en `document_sequences`. Hoy se crea una numeración `POS` sin resolución.
- **`GET /v1/reports/dashboard`:** el panel del frontend todavía usa mocks.

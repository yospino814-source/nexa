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
| GET | `/v1/sales?branch_id=&from=&to=&cashier_id=&method=&status=&q=&page=&page_size=` | Historial de ventas; sin `reports:read` solo las propias (`cash:history`) |
| GET | `/v1/sales/{id}` | Detalle de una venta con devoluciones, cliente y cajero (`cash:history`) |
| POST | `/v1/sales/{id}/void` | Anular una venta de hoy con el turno abierto, sin devoluciones ni documento fiscal aceptado (`pos:refund`) |
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
| GET | `/v1/orders/{id}/refunds` | Devoluciones de una venta (`pos:sell`) |
| POST | `/v1/refunds` | Crear devolución idempotente por `id`; queda `PENDIENTE` o `APROBADA` según el ajuste y el permiso (`pos:sell`) |
| POST | `/v1/refunds/{id}/approve` | Aprobar: stock, caja, saldo devuelto y nota crédito (`pos:refund`) |
| POST | `/v1/refunds/{id}/reject` | Rechazar con motivo (`pos:refund`) |
| GET | `/v1/refunds?branch_id=&status=` | Devoluciones de la sucursal (`pos:refund`) |
| GET | `/v1/inventory/available?branch_id=&product_id=` | Disponible de un producto |
| GET | `/v1/inventory/stock?branch_id=` | Existencias con punto de reorden y estado `OK` / `BAJO` / `AGOTADO` |
| GET | `/v1/inventory/movements?branch_id=&product_id=&limit=` | Kardex, más reciente primero, con `delta` y saldo resultante |
| POST | `/v1/inventory/replenish`, `/v1/inventory/adjust` | `{branch_id, product_id, qty, cause}` (el ajuste crea la fila de stock si no existía) |
| POST | `/v1/inventory/count` | Conteo físico `{branch_id, product_id, counted, cause}`; registra la diferencia |
| POST | `/v1/inventory/transfer` | Traslado entre sucursales `{from_branch_id, to_branch_id, product_id, qty, cause}`; exige permiso en ambas |
| PUT | `/v1/inventory/min-qty` | Punto de reorden `{branch_id, product_id, min_qty}` |
| GET | `/v1/suppliers?q=&include_inactive=` | Lista de proveedores del negocio |
| POST | `/v1/suppliers` | Crear proveedor `{name, tax_id?, doc_type?, dv?, person_type?, iva_responsible?, self_withholding?, payment_terms?, credit_days?, delivery_days?, category?, bank_name?, bank_account_type?, bank_account_number?, contact_name?, phone?, email?, address?, notes?}` |
| PUT | `/v1/suppliers/{id}` | Editar proveedor |
| POST | `/v1/suppliers/{id}/archive` | Archivar proveedor (valida que no tenga compras o deudas abiertas) |
| PUT | `/v1/suppliers/{id}/products` | Configurar catálogo del proveedor `{products: [{product_id, supplier_code?, min_order_qty?, preferred}]}` |
| GET | `/v1/purchases?branch_id=&from=&to=&supplier_id=&status=&limit=` | Historial de compras con número de ítems y total |
| POST | `/v1/purchases` | Crear orden de compra en estado `PENDIENTE` (idempotente) |
| GET | `/v1/purchases/summary?branch_id=&month=YYYY-MM` | Total comprado en el mes y proveedores activos |
| POST | `/v1/purchases/cost-preview` | Vista previa de prorrateo de costos y márgenes de ganancia |
| GET | `/v1/purchases/suggestions?branch_id=` | Sugerencias de compra por stock mínimo agrupadas por proveedor |
| POST | `/v1/purchases/suggestions/create` | Generar órdenes en borrador desde sugerencias seleccionadas |
| GET | `/v1/purchases/{id}` | Detalle de compra con ítems, producto, SKU, unidad de compra y costos |
| PUT | `/v1/purchases/{id}` | Actualizar orden de compra en estado `PENDIENTE` |
| POST | `/v1/purchases/{id}/approve` | Aprobar orden de compra (`purchases:approve`) |
| POST | `/v1/purchases/{id}/receive` | Recepción parcial o total con lotes, vencimientos y fletes (`purchases:receive`) |
| POST | `/v1/purchases/{id}/cancel` | Anular orden de compra (`purchases:approve`) |
| GET | `/v1/payables?branch_id=&status=&supplier_id=&overdue=` | Cuentas por pagar con saldo y estado (`payables:read`) |
| GET | `/v1/payables/aging?branch_id=` | Reporte de cartera y antigüedad de saldos (`payables:read`) |
| GET | `/v1/payables/{id}` | Detalle de cuenta por pagar e historial de pagos (`payables:read`) |
| POST | `/v1/payables/{id}/payments` | Registrar abono o pago a cuenta por pagar (`payables:pay`) |
| POST | `/v1/payables/{id}/write-off` | Castigar saldo pendiente de cuenta por pagar (`payables:pay`) |
| GET | `/v1/expense-categories` | Listado de categorías de gasto activas (`expenses:read`) |
| POST | `/v1/expense-categories` | Crear categoría de gasto (`expenses:create`) |
| PUT | `/v1/expense-categories/{id}` | Actualizar categoría de gasto (`expenses:create`) |
| GET | `/v1/expenses?branch_id=&category_id=&status=&from=&to=&limit=` | Historial de gastos (`expenses:read`) |
| GET | `/v1/expenses/summary?branch_id=&month=` | Métricas y total de gastos del mes (`expenses:read`) |
| GET | `/v1/expenses/{id}` | Detalle de gasto (`expenses:read`) |
| POST | `/v1/expenses` | Registrar gasto, pasa a `APROBADO` o `PENDIENTE` según umbral (`expenses:create`) |
| POST | `/v1/expenses/{id}/approve` | Aprobar gasto pendiente (`expenses:approve`) |
| POST | `/v1/expenses/{id}/reject` | Rechazar gasto pendiente con motivo (`expenses:approve`) |
| POST | `/v1/expenses/{id}/cancel` | Cancelar gasto no pagado (`expenses:create`) |
| GET | `/v1/inventory/expiring?branch_id=&days=30` | Lotes vencidos o próximos a vencer (`inventory:read`) |
| POST | `/v1/inventory/adjust` | Registrar ajuste de inventario con motivo, validación de signo y umbral (`inventory:adjust`) |
| GET | `/v1/inventory/adjustments?branch_id=&status=` | Historial de solicitudes de ajuste (`inventory:read`) |
| POST | `/v1/inventory/adjustments/{id}/approve` | Aprobar ajuste y aplicar delta al stock (`inventory:adjust_approve`) |
| POST | `/v1/inventory/adjustments/{id}/reject` | Rechazar solicitud de ajuste (`inventory:adjust_approve`) |
| GET | `/v1/permissions` | Catálogo de permisos del sistema y roles |
| GET | `/v1/permissions/roles/{role_id}` | Permisos efectivos de un rol (base + overrides de tenant) |
| PUT | `/v1/permissions/roles/{role_id}` | Actualizar overrides de permisos de un rol (solo `DUENO`) |
| GET | `/v1/settings?branch_id=` | Ajustes operativos de sucursal (POS, caja, inventario, propinas, impresión) |
| PUT | `/v1/settings?branch_id=` | Modificar ajustes operativos de sucursal |
| GET | `/v1/audit?from=&to=&user_id=&action=&limit=` | Registro de auditoría inmutable |
| GET | `/v1/users` | Lista de usuarios y membresías del negocio |
| GET | `/v1/business` | Nombre, razón social, NIT y DV del negocio, para la tirilla (cualquier miembro) |
| GET | `/v1/waiters?branch_id=` | Meseros activos de la sucursal, para la propina del POS (`pos:sell`) |
| POST | `/v1/users/invite` | Invitar usuario por email con rol y sucursales asignadas |
| PUT | `/v1/users/{id}/role` | Modificar rol de usuario (protege al dueño de auto-degradarse) |
| PUT | `/v1/users/{id}/status` | Activar o desactivar acceso de un usuario |
| POST | `/v1/auth/login-attempt` | Registrar o verificar intento de login; tras 5 fallos en 15 min responde 429 |
| GET | `/v1/products?branch_id=&q=&category_id=&usage=&archived=false&page=&page_size=` | Listado de productos con paginación y filtros (`inventory:read`) |
| GET | `/v1/products/{id}?branch_id=` | Detalle completo de producto (`inventory:read`) |
| POST | `/v1/products` | Crear nuevo producto (`inventory:adjust`) |
| PUT | `/v1/products/{id}` | Actualizar producto (`inventory:adjust`) |
| POST | `/v1/products/{id}/variants` | Crear variante de producto (`inventory:adjust`) |
| PUT | `/v1/branch-products/{product_id}` | Actualizar precio y disponibilidad por sucursal (`inventory:adjust`) |

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

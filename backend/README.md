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
4. **Usuario:** créalo en *Authentication → Users* y dale acceso a un negocio. Desde la fase S, el dueño crea su negocio en `/registro`. Este SQL queda solo para pruebas. Sin membresía, el login muestra "Tu usuario no tiene un negocio asignado". Ejecuta esto en el SQL editor:

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
- **Endpoints por área (`src/routes/`).** Cada archivo tiene su `router()` y sus handlers; `routes/mod.rs` los une y guarda los extractores comunes (`Body`, `Query`, `Path`), la salud, el contexto y las sucursales. Para un endpoint nuevo, agrégalo en el archivo de su área.

  | Archivo | Área |
  |---|---|
  | `catalogo.rs` | Productos, modificadores, tarifas, promociones, cupones, importación CSV |
  | `clientes.rs` | Clientes y cuentas por cobrar |
  | `ventas.rs` | Venta de mostrador, caja, devoluciones, anulaciones, historial |
  | `ordenes.rs` | Órdenes de mesa, domicilio y mayorista |
  | `inventario.rs` | Existencias, kardex, traslados, bodegas |
  | `compras.rs` | Proveedores, compras, cuentas por pagar, gastos |
  | `administracion.rs` | Permisos, ajustes, auditoría, usuarios, intentos de login |
  | `reportes.rs` | Reportes y tablero |

  Los pedidos por WhatsApp viven aparte, en `src/whatsapp/`, con su propio `router()`.

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
| POST | `/v1/branches` | Crear sucursal nueva con bodega Principal y Caja 1 (solo `DUENO`) |
| PUT | `/v1/branches/{id}` | Actualizar nombre de sucursal (`settings:write`) |
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
| GET | `/v1/purchases?branch_id=&from=&to=&supplier_id=&status=&limit=` | Historial de compras con número de ítems y total (`purchases:create`, `purchases:approve` o `purchases:receive`) |
| POST | `/v1/purchases` | Crear orden de compra en estado `PENDIENTE` (idempotente, `purchases:create`) |
| GET | `/v1/purchases/summary?branch_id=&month=YYYY-MM` | Total comprado en el mes y proveedores activos (`purchases:create`, `purchases:approve` o `purchases:receive`) |
| POST | `/v1/purchases/cost-preview` | Vista previa de prorrateo de costos y márgenes de ganancia (`purchases:create`, `purchases:approve` o `purchases:receive`) |
| GET | `/v1/purchases/suggestions?branch_id=` | Sugerencias de compra por stock mínimo agrupadas por proveedor |
| POST | `/v1/purchases/suggestions/create` | Generar órdenes en borrador desde sugerencias seleccionadas |
| GET | `/v1/purchases/{id}` | Detalle de compra con ítems, producto, SKU, unidad de compra y costos (`purchases:create`, `purchases:approve` o `purchases:receive`) |
| PUT | `/v1/purchases/{id}` | Actualizar orden de compra en estado `PENDIENTE` (`purchases:create`) |
| POST | `/v1/purchases/{id}/approve` | Aprobar orden de compra (`purchases:approve`) |
| POST | `/v1/purchases/{id}/receive` | Recepción parcial o total con lotes, vencimientos y fletes (`purchases:receive`) |
| POST | `/v1/purchases/{id}/cancel` | Anular orden de compra (`purchases:create`) |
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
| GET | `/v1/reports/dashboard?branch_id=&from=&to=` | Métricas, ventas por hora/día, top 10, métodos de pago, stock bajo y cartera vencida del panel (`reports:read`; `branch_id` opcional) |
| GET | `/v1/reports/sales-by-day?branch_id=&from=&to=&format=` | Ventas por día: cantidad, venta bruta, devoluciones, venta neta, ticket promedio y margen (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/sales-by-product?branch_id=&from=&to=&format=` | Ventas por producto: SKU, categoría, cantidad, venta sin impuesto, costo, margen y % margen (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/sales-by-category?branch_id=&from=&to=&format=` | Ventas por categoría: cantidad, venta sin impuesto y margen (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/sales-by-cashier?branch_id=&from=&to=&format=` | Ventas por cajero (creador del primer pago): ventas, venta bruta y ticket promedio (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/sales-by-method?branch_id=&from=&to=&format=` | Pagos por método incluyendo `CREDITO` ("Crédito"): pagos y monto (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/sales-heatmap?branch_id=&from=&to=` | Mapa de calor por día ISO (1..7) y hora (0..23): ventas y venta bruta (`reports:read`; solo JSON) |
| GET | `/v1/reports/inventory-valuation?branch_id=&from=&to=&format=` | Valorización de inventario por producto y ubicación con `on_hand != 0` (`reports:read`; `format=json\|csv`) |
| GET | `/v1/reports/inventory-rotation?branch_id=&to=&format=` | Rotación de inventario últimos 30 días hasta `to` (`reports:read`; ignora `from`; `format=json\|csv`) |
| GET | `/v1/reports/purchases-by-supplier?branch_id=&from=&to=&format=` | Compras por proveedor: órdenes no anuladas, recibido, pagado y saldo (`reports:read`; `format=json\|csv`) |
| POST | `/v1/onboarding` | Alta de negocio y sucursal principal con rol DUENO (requiere sesión autenticada) |
| GET | `/v1/users` | Lista de usuarios y membresías del negocio |
| GET | `/v1/business` | Nombre, razón social, NIT, DV e iva_responsible del negocio (cualquier miembro) |
| PUT | `/v1/business` | Actualizar datos del negocio (`settings:write`, campos fiscales solo `DUENO`) |
| GET | `/v1/waiters?branch_id=` | Meseros activos de la sucursal, para la propina del POS (`pos:sell`) |
| POST | `/v1/users/invite` | Invitar usuario por email con rol y sucursales asignadas |
| POST | `/v1/users/invitations/{id}/revoke` | Revocar invitación pendiente (`users:manage`) |
| GET | `/v1/invitations/mine` | Invitaciones pendientes del usuario autenticado |
| POST | `/v1/invitations/{id}/accept` | Aceptar invitación y crear membresía activa |
| PUT | `/v1/users/{id}/role` | Modificar rol de usuario (protege al dueño de auto-degradarse) |
| PUT | `/v1/users/{id}/status` | Activar o desactivar acceso de un usuario |
| POST | `/v1/auth/login-attempt` | Registrar o verificar intento de login; tras 5 fallos en 15 min responde 429 |
| GET | `/v1/products?branch_id=&q=&category_id=&usage=&archived=false&page=&page_size=` | Listado de productos con paginación y filtros (`inventory:read`) |
| GET | `/v1/products/{id}?branch_id=` | Detalle completo de producto (`inventory:read`) |
| POST | `/v1/products` | Crear nuevo producto `{name, sku?, barcode?, kind, usage, unit_measure_code, purchase_uom_code?, purchase_factor?, tax_code, cost_price?, base_price?, reorder_qty?, max_qty?, track_lots?, allow_price_override?, has_variants?, description?, alcohol_degrees?, volume_ml?}` (`inventory:adjust`) |
| PUT | `/v1/products/{id}` | Actualizar producto (`inventory:adjust`) |
| PUT / DELETE | `/v1/products/{id}/image` | Foto del producto `{data}` en base64 o data URI; WebP, JPEG o PNG de hasta 512 KB, validado por su firma (`inventory:adjust`) |
| GET | `/v1/public/product-images/{id}?v=` | Foto pública para `<img>`, con caché de un año (la `v` cambia con cada foto). Sin token |
| POST | `/v1/products/{id}/variants` | Crear variante de producto (`inventory:adjust`) |
| PUT | `/v1/branch-products/{product_id}` | Actualizar precio y disponibilidad por sucursal (`inventory:adjust`) |
| GET | `/v1/customers?q=&status=&page=&page_size=` | Listado de clientes con saldo consolidado y mora (`customers:read`) |
| GET | `/v1/customers/{id}` | Detalle de cliente con cupo, saldo y mora (`customers:read`) |
| POST | `/v1/customers` | Crear cliente (`customers:write`; cupo != 0 exige `credit:override`) |
| PUT | `/v1/customers/{id}` | Actualizar cliente (`customers:write`; cambio de cupo exige `credit:override`) |
| POST | `/v1/customers/upsert` | Crear o completar cliente existente por email/documento (`customers:write`) |
| DELETE | `/v1/customers/{id}` | Archivar cliente sin saldo ni apartados pendientes (`customers:write`) |
| POST | `/v1/customers/{id}/unarchive` | Desarchivar cliente (`customers:write`) |
| GET | `/v1/customers/{id}/purchases?page=&page_size=` | Historial de compras del cliente con formato de ventas (`customers:read`) |
| GET | `/v1/customers/{id}/statement?from=&to=` | Estado de cuenta cronológico con cargos, abonos y devoluciones (`customers:read`) |
| GET | `/v1/receivables?customer_id=&status=&overdue=` | Listado de cuentas por cobrar con días de mora (`customers:read`) |
| GET | `/v1/receivables/summary?branch_id=` | Resumen de cartera: saldos, vencido, semana, mes, antigüedad y top deudores (`customers:read`) |
| GET | `/v1/receivables/{id}` | Detalle de cuenta por cobrar con cuotas y abonos (`customers:read`) |
| POST | `/v1/receivables/{id}/payments` | Registrar abono con aplicación a cuotas y caja (`credit:collect`) |
| GET | `/v1/receivables/{id}/whatsapp-link` | Enlace wa.me con mensaje de cobro formateado (`customers:read`) |

### Onboarding (`/v1/onboarding`)

Alta inicial de negocio y sucursal para nuevos usuarios registrados:

| Método | Ruta | Permisos | Parámetros / Cuerpo | Códigos de error |
|---|---|---|---|---|
| `POST` | `/v1/onboarding` | Ninguno (solo sesión `AuthUser`) | Body JSON: `{ business_name, branch_name, full_name }`. Retorna `{ tenant_id, branch_id }` con status 201 Created. | 400, 401, 409 (`ONBOARDING_ALREADY_MEMBER`), 422 (`ONBOARDING_INVALID`) |

### Clientes (`/v1/customers`)

Módulo de gestión de clientes y posición crediticia consolidada por negocio (`tenancy`):

| Método | Ruta | Permisos | Parámetros / Cuerpo | Códigos de error |
|---|---|---|---|---|
| `GET` | `/v1/customers` | `customers:read` | Query: `q` (búsqueda), `status` (`TODOS`, `AL_DIA`, `EN_MORA`, `ARCHIVADO`), `page`, `page_size`, `limit` | 401, 403 |
| `GET` | `/v1/customers/{id}` | `customers:read` | Path: `id` (UUID) | 401, 403, 404 |
| `POST` | `/v1/customers` | `customers:write`<br>*(+ `credit:override` si `credit_limit != 0` o `credit_block_days != 30`)* | Body JSON: `name` (obligatorio), `doc_type`, `doc_number`, `dv`, `person_type`, `phone`, `email`, `tax_regime`, `fiscal_responsibilities`, `ciiu`, `withholding_agent`, `city_code`, `billing_address`, `shipping_address`, `pinned_note`, `credit_limit`, `credit_block_days` | 400, 401, 403, 409 (`CUSTOMER_DUPLICATED`), 422 (`CUSTOMER_INVALID`, `NIT_DV_INVALID`, `CUSTOMER_DV_CONFLICT`) |
| `PUT` | `/v1/customers/{id}` | `customers:write`<br>*(+ `credit:override` si cambia `credit_limit` o `credit_block_days`)* | Path: `id` (UUID). Body JSON: campos a modificar (campos omitidos conservan su valor guardado) | 400, 401, 403, 404, 409 (`CUSTOMER_DUPLICATED`), 422 |
| `POST` | `/v1/customers/upsert` | `customers:write`<br>*(+ `credit:override` si `credit_limit != 0` o `credit_block_days != 30`)* | Body JSON: igual a `POST /v1/customers`. Busca por `email` y luego por `(doc_type, doc_number)`. Si existe, solo llena campos vacíos y copia el documento como bloque atómico. Respuesta: `{ customer, result: "CREATED" \| "UPDATED" \| "UNCHANGED" }` | 400, 401, 403, 409 (`CUSTOMER_DUPLICATED`), 422 |
| `DELETE` | `/v1/customers/{id}` | `customers:write` | Path: `id` (UUID). Valida `customer_credit_position.balance == 0`, sin apartados activos ni saldo a favor en billetera. | 401, 403, 404, 409 (`CUSTOMER_HAS_BALANCE`) |
| `POST` | `/v1/customers/{id}/unarchive` | `customers:write` | Path: `id` (UUID). Verifica que ningún otro cliente activo tenga el mismo `(doc_type, doc_number)`. | 401, 403, 404, 409 (`CUSTOMER_DUPLICATED`) |
| `GET` | `/v1/customers/{id}/purchases` | `customers:read` | Path: `id` (UUID). Query: `page`, `page_size`. Devuelve historial de órdenes con formato estándar de ventas (`HistoryPage`). | 401, 403, 404 |
| `GET` | `/v1/customers/{id}/statement` | `customers:read` | Path: `id` (UUID). Query: `from`, `to` (fechas `YYYY-MM-DD`). Retorna `opening_balance`, movimientos cronológicos (`CARGO`, `ABONO`, `DEVOLUCION` con debit/credit/balance acumulado) y `closing_balance`. | 401, 403, 404 |

### Cuentas por cobrar y abonos (`/v1/receivables`)

Módulo de cartera de clientes, cuotas y aplicación de abonos:

| Método | Ruta | Permisos | Parámetros / Cuerpo | Códigos de error |
|---|---|---|---|---|
| `GET` | `/v1/receivables` | `customers:read` | Query: `customer_id` (UUID), `status` (`ABIERTA`, `PARCIAL`, `PAGADA`; por defecto abiertas y parciales), `overdue` (`true` filtra solo mora > 0) | 401, 403 |
| `GET` | `/v1/receivables/summary` | `customers:read` | Query: `branch_id` (UUID, opcional). Retorna `total_balance`, `overdue_balance`, `due_this_week`, `collected_this_month`, `aging` (`al_dia`, `d1_30`, `d31_60`, `d61_90`, `d91_120`, `d120_mas`) y `top_debtors` (top 10 por saldo). | 401, 403 |
| `GET` | `/v1/receivables/{id}` | `customers:read` | Path: `id` (UUID). Retorna datos de la cuenta, tabla de cuotas (`installments`) y pagos recibidos (`payments`). | 401, 403, 404 |
| `POST` | `/v1/receivables/{id}/payments` | `credit:collect` | Path: `id` (UUID de la cuenta). Body JSON: `{ id, amount, method, reference }`. Aplica cuotas en cascada, valida límites, registra movimiento de caja en EFECTIVO (`ABONO_CARTERA`) y emite auditoría. | 400, 401, 403, 404, 409 (`IDEMPOTENCY_CONFLICT`, `RECEIVABLE_CLOSED`, `PAYMENT_NEEDS_OPEN_SESSION`), 422 (`PAYMENT_EXCEEDS_BALANCE`, `PAYMENT_METHOD_NOT_AVAILABLE`, `PAYMENT_REFERENCE_REQUIRED`) |
| `GET` | `/v1/receivables/{id}/whatsapp-link` | `customers:read` | Path: `id` (UUID). Genera enlace `https://wa.me/57<telefono>?text=...` con saludo personalizado, saldo y fecha de vencimiento/mora. | 401, 403, 404, 422 (`CUSTOMER_NO_PHONE`) |

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

## Pedidos por WhatsApp

Nexi, el bot, toma pedidos por WhatsApp con el catálogo, los precios con impuesto y las existencias de la sede. El equipo los atiende desde la bandeja.

**Conversación.** El cliente escribe ("1 old parr y 2 hielos") o toca botones:

1. Arma el carrito. Si algo no alcanza, pone lo que hay; si está agotado, ofrece uno parecido de la misma categoría.
2. Confirma el pedido. Con licor o cigarrillos pide confirmar la mayoría de edad.
3. Pide la dirección (u ofrece la del último domicilio, o recoger en tienda). Calcula el domicilio por zona y revisa el pedido mínimo.
4. Pide el medio de pago: link (si está activo), efectivo o datáfono al recibir. Registra el pedido `W-0001`.

También reconoce al cliente por su número y ofrece "lo de siempre", responde fuera de horario y pasa a una persona si escribe "asesor". El motor está en `src/whatsapp/engine.rs` y no toca la base: se prueba con `cargo test whatsapp`.

**Conectar Meta.**

1. En *Meta for Developers*, crea la app de WhatsApp y un número de WhatsApp Business.
2. Configura el webhook en `https://<tu-backend>/v1/whatsapp/webhook`, con el mismo `WHATSAPP_VERIFY_TOKEN`, y suscríbelo a `messages`.
3. En el `.env`: `WHATSAPP_APP_SECRET` (firma de cada mensaje) y `WHATSAPP_ACCESS_TOKEN` (token de sistema para responder).
4. Con `PUT /v1/whatsapp/bot` asocia el `phone_number_id` a la sede y carga horario, zonas y opciones.

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/v1/whatsapp/webhook` | Verificación de Meta (`hub.challenge`). Sin JWT. |
| POST | `/v1/whatsapp/webhook` | Mensajes entrantes. Sin JWT: exige `X-Hub-Signature-256`. Un reintento con el mismo id no se responde dos veces. |
| GET | `/v1/whatsapp/orders?branch_id=&status=` | Bandeja: pedidos abiertos y los cerrados del último día. |
| GET | `/v1/whatsapp/orders/{id}` | Pedido y su conversación. |
| POST | `/v1/whatsapp/orders/{id}/status` | `{status, courier_name?, sale_id?}`. `NUEVO → PREPARANDO → EN_CAMINO → ENTREGADO`, o `CANCELADO` antes de entregar. Avisa al cliente. Un pedido con link no sale sin pagar. |
| POST | `/v1/whatsapp/orders/{id}/paid` | `{reference}`: marca pagado un pedido con link y avisa. |
| GET / PUT | `/v1/whatsapp/bot` | Configuración por sede: número, horario, pedido mínimo, opciones y zonas. PUT solo dueño o administrador. |
| GET | `/v1/whatsapp/bot/qr?branch_id=&text=` | QR de la tienda: enlace `wa.me` al número del bot con el mensaje ya escrito, y el QR en SVG para imprimir. |

## Facturación electrónica

Cada venta deja su **documento equivalente POS electrónico** en `fiscal_documents` (PENDIENTE), y cada devolución aprobada su **nota crédito POS**. Un proceso de fondo (`src/fiscal/`) los envía cada 10 s al proveedor de la sede y guarda el CUDE, el enlace del QR de la DIAN y el PDF.

- **Proveedores:** `SIMULADO` (no sale a internet; inventa un CUDE para desarrollo y demos) y `MATIAS` (MATIAS API).
- **Estados:** `PENDIENTE` → `ACEPTADO`, `RECHAZADO` (con el motivo de la DIAN en `last_error`) o `EN_PROCESO` (la DIAN aún no responde; se vuelve a consultar).
- **Reintentos:** si el proveedor no responde, se reintenta con espera creciente, de 30 s hasta 1 h, como máximo 12 veces. Si la respuesta dice que pudo haberse enviado, primero se consulta el estado y no se reenvía a ciegas.
- **Notas crédito:** salen solo cuando el documento POS original ya fue aceptado, porque lo referencian por su CUDE.

| Método | Ruta | Descripción |
|---|---|---|
| GET / PUT | `/v1/fiscal/settings?branch_id=` | Proveedor, activado, sub-cuenta (Casa de Software), dirección del punto de venta, y la numeración DIAN de POS y de notas crédito (prefijo, resolución, rango, fechas). PUT solo dueño o administrador. |
| GET | `/v1/fiscal/documents?branch_id=&status=` | Documentos con estado, CUDE, QR, PDF y último error. |
| POST | `/v1/fiscal/documents/{id}/retry` | Vuelve a enviar un documento rechazado o detenido (dueño o administrador). |
| GET | `/v1/fiscal/orders/{order_id}` | Documento POS de una venta: estado, número, CUDE, PDF y `qr_svg` (solo ACEPTADO) para el tiquete. `null` si no hay. |
| POST | `/v1/fiscal/test?branch_id=` | Prueba la conexión con el proveedor de la sede (`GET /tokens` en MATIAS). |

Al cobrar o aprobar una devolución el documento se envía al instante (`fiscal::kick`); el ciclo de 10 s queda para reintentos.

**Probar con el sandbox de MATIAS:**

1. Crea la cuenta en https://sandbox-auth.matias-api.com/ y verifica el correo.
2. En ese portal, configura la empresa y la numeración POS (y la de notas crédito). El número de resolución que pongas en Nexa debe ser el mismo.
3. Crea un token personal (`POST /tokens`, o desde el portal) y ponlo en `MATIAS_API_TOKEN`.
4. En Nexa: **Configuración → Facturación DIAN** (`/configuracion/facturacion`): elegí MATIAS, cargá prefijo y resolución, prendé el envío y tocá *Probar conexión*.

Impuestos en el documento electrónico: IVA (tax_id 1) e INC (tax_id 4). El ICL de licores no se discrimina: se cobra en la cadena (productor o importador) y viene incluido en el precio de compra. Los productos guardan grados y ml por si un contador indica lo contrario.

## Seguridad

Controles del backend:

- **Límite de peticiones** (`src/security.rs`, crate `tower_governor`): 60 seguidas y luego 10 por segundo por usuario (se identifica por un resumen del token, nunca el token) o por IP si no hay sesión. `POST /v1/auth/login-attempt` tiene uno más estricto: 5 seguidas y luego 1 cada 12 s por IP. Responde 429 con `Retry-After`.
- **IP real del cliente:** solo con `TRUST_PROXY=1`, y se toma la última entrada de `X-Forwarded-For` (la que agrega nuestro proxy).
- **Intentos de login:** se cuentan por correo **e IP**, así nadie puede bloquear la cuenta de otra persona desde su propia IP.
- **Encabezados:** toda respuesta lleva `nosniff`, `Cache-Control: no-store`, `X-Frame-Options: DENY` y `Referrer-Policy: no-referrer`.
- **Base de datos:** las funciones que corren sin contexto de negocio (`record_login_attempt`, `whatsapp_bot_route`) viven en el esquema `private`, que Supabase no publica. El negocio activo del JWT exige membresía activa. El perfil propio solo permite cambiar el nombre.
- **Dependencias:** `cargo audit` limpio. Los avisos aceptados, con su razón, están en `.cargo/audit.toml`.

Configuración que se hace en el panel de Supabase (no está en el código):

1. **Authentication → Bot and Abuse Protection:** activar CAPTCHA con **Cloudflare Turnstile** y pegar la clave secreta. La clave de sitio va en el frontend (`NEXT_PUBLIC_TURNSTILE_SITE_KEY`).
2. **Authentication → Rate Limits:** revisar los límites de inicio de sesión y de correos.
3. **Project Settings → Data API:** el frontend no consulta tablas por la API de Supabase, solo usa el login. Desactivar la Data API o quitar `public` de los esquemas expuestos.

## Pendiente

- **ICL de licores:** confirmar con un contador si la tienda debe discriminarlo en el documento POS. Hoy no se envía.
- **Link de pago de los pedidos por WhatsApp:** el bot encola `WHATSAPP_PAYMENT_LINK` en el outbox. Falta el worker que crea el link en Bold, lo envía y llama `/paid` cuando se confirma el pago.
- **Pedido → venta del POS:** al despachar, el frontend registra la venta con `POST /v1/sales` y manda su `sale_id` al cambiar el estado. Los combos y promociones del POS no se aplican en el precio que cotiza el bot.

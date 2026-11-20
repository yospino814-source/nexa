# Fase 6: Tienda en línea y pedidos por WhatsApp

> **POSPUESTA.** No se construye para la salida a licorerías y tiendas. Ver `antigravity/modulos/00-INDICE.md`.

> **Requisito:** la entrega 5B debe estar aprobada. Rama `feat/fase-6-tienda`, a partir de `feat/fase-5b-domicilios`.
>
> **Migración única:** `20261110000100_fase6_tienda.sql`.
>
> **Llaves de Wompi:** **no las busques, no las pidas en el chat y no escribas valores de ejemplo reales.** El código las lee de variables de entorno, y Claude las configura. Sin llaves, el pago en línea **no se muestra**.
>
> Todo lo que no esté escrito aquí está prohibido.

**Fuera de alcance:**
- cuentas de cliente y carrito guardado en el servidor: la compra es **solo como invitado**, con el carrito en el navegador;
- zonas de domicilio por mapa: la zona se elige de una lista;
- subir comprobantes de pago;
- dominio propio por negocio y PWA;
- envío automático por WhatsApp: solo se generan enlaces `wa.me`.

## Orden de las tareas

| Tarea | Tema |
|---|---|
| F6.1 | Configuración de la tienda, zonas y franjas |
| F6.2 | Catálogo público |
| F6.3 | Checkout con idempotencia y todas sus validaciones |
| F6.4 | Pago en línea con Wompi (más un proveedor simulado) |
| F6.5 | Finalizar por WhatsApp |
| F6.6 | Seguimiento público del pedido |
| F6.7 | Pedidos entrantes en el POS y la cocina |
| F6.8 | Cancelación automática de los pedidos sin pagar |

---

## F6.1 Configuración de la tienda

### Migración

```sql
create table public.store_settings (
  branch_id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  enabled boolean not null default false,
  slug text not null unique check (slug ~ '^[a-z0-9](?:[a-z0-9-]{1,38})[a-z0-9]$'),
  display_name text not null check (length(display_name) between 1 and 60),
  whatsapp_phone text check (whatsapp_phone ~ '^57[0-9]{10}$'),
  min_order numeric(14,2) not null default 0 check (min_order >= 0),
  pickup_enabled boolean not null default true,
  delivery_enabled boolean not null default false,
  cash_on_pickup boolean not null default true,
  cash_on_delivery boolean not null default false,
  online_payment_enabled boolean not null default false,
  whatsapp_checkout_enabled boolean not null default false,
  prep_minutes integer not null default 30 check (prep_minutes between 5 and 240),
  hours jsonb not null default '[]',          -- [{weekday 0..6 (0=domingo), open "HH:MM", close "HH:MM"}]
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.delivery_zones (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(name) between 1 and 40),
  fee numeric(14,2) not null check (fee >= 0),
  free_from numeric(14,2) check (free_from > 0),
  active boolean not null default true,
  unique (tenant_id, id), unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.store_slots (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  weekday integer not null check (weekday between 0 and 6),
  start_time time not null, end_time time not null,
  capacity integer not null check (capacity between 1 and 500),
  active boolean not null default true,
  unique (tenant_id, id),
  check (end_time > start_time),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.store_closed_days (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  day date not null,
  primary key (branch_id, day),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
alter table public.products add column online_visible boolean not null default true;
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('En linea','ONLINE'),('WhatsApp','WHATSAPP')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'VENTA') h
on conflict (padre_id, tima_codigo) do nothing;
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Pago en linea', 'ONLINE', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PAGO'
on conflict (padre_id, tima_codigo) do nothing;
create function public.resolve_store_slug(p_slug text)
returns table (tenant_id uuid, branch_id uuid)
language sql stable security definer set search_path = '' as $$
  select s.tenant_id, s.branch_id from public.store_settings s where s.slug = p_slug and s.enabled
$$;
revoke execute on function public.resolve_store_slug(text) from public, anon;
grant execute on function public.resolve_store_slug(text) to authenticated;
-- RLS con in_scope en todas las tablas nuevas; grants.
```

### Endpoints (permiso `settings:write`)
- `GET` y `PUT /v1/store/settings?branch_id=`.
  - El `slug` se normaliza a minúsculas. Si ya está tomado: 409 `STORE_SLUG_TAKEN`.
  - `hours`: cada entrada debe tener un `weekday` válido y `open < close`, sin solaparse con otra del mismo día. Si no: 422 `STORE_HOURS_INVALID`.
  - `online_payment_enabled = true` cuando el servidor no tiene las llaves de Wompi (F6.4): 422 `PAYMENTS_NOT_CONFIGURED`.
- `GET`, `POST` y `PUT /v1/store/zones`.
- `GET`, `POST` y `PUT /v1/store/slots`.
- `GET`, `POST` y `DELETE /v1/store/closed-days`.

### Frontend
Pantalla `/configuracion/tienda` (registro `s("48", "tienda-config", "Tienda en línea", "/configuracion/tienda", "configuracion", "admin", "react")`), con las pestañas General (los campos de `store_settings`, el enlace público con un botón para copiarlo y su QR), Horario, Zonas de domicilio, Franjas y Días cerrados.

---

## F6.2 Catálogo público

**Acceso sin sesión:** todos los endpoints `/v1/public/store/{slug}/…` resuelven el slug con `public.resolve_store_slug` y abren la transacción con `tenancy::begin_public` (fase 5). Si el slug no existe o la tienda está apagada: 404. Usan el mismo limitador de 60 peticiones por minuto por IP de la fase 5.

`GET /v1/public/store/{slug}` devuelve:

```json
{ "display_name": "...", "whatsapp_enabled": true, "pickup_enabled": true, "delivery_enabled": true,
  "open_now": true, "next_open": "2026-11-10T08:00:00-05:00|null", "min_order": "20000.00",
  "zones": [{"id":"…","name":"Centro","fee":"3000.00","free_from":"50000.00|null"}],
  "payment_methods": ["ONLINE","EFECTIVO_RECOGIDA","EFECTIVO_DOMICILIO","WHATSAPP"],
  "categories": [{"id":"…","name":"Bebidas","products":[{"id":"…","name":"…","description":"…",
     "price_with_tax":"3570.00","available": true,"option_groups":[…]}]}] }
```

- **Productos:** `usage = VENTA`, `online_visible`, disponibles en la sucursal, sin archivar y sin ser padres de variantes (las variantes se listan como productos aparte).
- `available` es falso si el disponible en la ubicación por defecto es menor o igual a 0 (los servicios siempre están disponibles).
- **`payment_methods`:**

  | Método | Aparece si |
  |---|---|
  | `ONLINE` | `online_payment_enabled` y hay llaves |
  | `EFECTIVO_RECOGIDA` | `pickup_enabled` y `cash_on_pickup` |
  | `EFECTIVO_DOMICILIO` | `delivery_enabled` y `cash_on_delivery` |
  | `WHATSAPP` | `whatsapp_checkout_enabled` y `whatsapp_phone` |

- `open_now` sale de `hours` con la hora local de la sucursal y de `store_closed_days`.

`GET /v1/public/store/{slug}/slots?date=YYYY-MM-DD` devuelve `[{slot_id, start, end, remaining}]`. `remaining = capacity − número de órdenes no canceladas con ese slot y esa fecha`. Un día cerrado devuelve `[]`.

### Frontend
Pantalla 29, `/tienda/[slug]`: página pública `app/(publico)/tienda/[slug]/page.tsx`, pensada primero para celular.
- Encabezado con el nombre y el estado ("Abierto" o "Cerrado, abre <día hora>").
- Pestañas de categorías, buscador y tarjetas de producto con el botón "Agregar" (abre las opciones si las tiene).
- Barra inferior fija con "Ver carrito (N) $total".
- El carrito se guarda en `localStorage`, con la clave `nexa.store.cart.<slug>`, como `[{product_id, qty, option_ids}]`.

---

## F6.3 Checkout

### Migración

```sql
create table public.idempotency_keys (
  key uuid primary key,
  branch_id uuid not null,
  request_hash text not null,
  status text not null check (status in ('PROCESANDO','HECHO','FALLIDO')),
  response jsonb,
  created_at timestamptz not null default now()
);
alter table public.idempotency_keys enable row level security;   -- sin políticas: solo se usa con el pool del worker o desde funciones security definer
alter table public.orders
  add column fulfillment text check (fulfillment in ('RECOGIDA','DOMICILIO')),
  add column slot_id uuid, add column slot_date date,
  add column payment_status text check (payment_status in ('PENDIENTE_PAGO','PAGADO','CONTRAENTREGA','POR_CONFIRMAR','RECHAZADO')),
  add column tracking_token text unique,
  add column buyer_name text, add column buyer_email text,
  add column delivery_zone_id uuid;
```

**`idempotency_keys`** se maneja **solo** con dos funciones `security definer`, sin otro acceso:
- `public.idem_begin(key, branch, hash) returns jsonb`:
  - si la clave no existe, la inserta en `PROCESANDO` y devuelve `null`;
  - si existe con el mismo `hash`: en `HECHO`, devuelve `{"replay": response}`; en `PROCESANDO`, devuelve `{"busy": true}`; en `FALLIDO`, la pasa a `PROCESANDO` y devuelve `null`;
  - si existe con otro `hash`, devuelve `{"conflict": true}`;
  - las claves de más de 24 horas se tratan como inexistentes y se sobrescriben.
- `public.idem_finish(key, status, response)`.

Ejecuta las dos en **transacciones separadas** de la del pedido. Así, si el pedido falla, se marca `FALLIDO`.

### `POST /v1/public/store/{slug}/checkout`
Encabezado `Idempotency-Key: <uuid>`, obligatorio; si falta: 400. Cuerpo:

```json
{ "items": [{"product_id":"…","qty":"2","option_ids":[]}],
  "buyer": {"name":"…","phone":"3001234567","email":"…|null","doc_type":"CC|null","doc_number":"…|null"},
  "fulfillment": "RECOGIDA|DOMICILIO",
  "address": "…|null", "zone_id": "…|null", "notes": "…|null",
  "slot_id": "…|null", "slot_date": "YYYY-MM-DD|null",
  "payment_method": "ONLINE|EFECTIVO_RECOGIDA|EFECTIVO_DOMICILIO|WHATSAPP",
  "coupon_code": "…|null", "expected_total": "…" }
```

**Idempotencia:** `idem_begin`. Si devuelve `busy`: 409 `REQUEST_IN_PROGRESS`. Si devuelve `conflict`: 409 `IDEMPOTENCY_CONFLICT`. Si devuelve `replay`: responde lo guardado con 200.

**Validaciones, en este orden exacto.** El primer error que ocurra es la respuesta, siempre 422 salvo que se indique otra cosa:
1. `buyer.name` de 2 a 80 caracteres y `buyer.phone` de 10 dígitos que empiece por 3. Si no: `BUYER_INVALID`.
2. La tienda está abierta (`open_now`), **o** se eligió una franja futura. Si no: `STORE_CLOSED`.
3. **Franja:** si la sucursal tiene franjas activas, `slot_id` y `slot_date` son obligatorios; la franja debe corresponder al día de la semana y no ser un día cerrado; su inicio debe ser al menos `prep_minutes` posterior a ahora; y su `remaining` debe ser mayor que 0, contado **dentro de la transacción** con `for update` sobre la fila de `store_slots`. Si falla: `SLOT_UNAVAILABLE`. Si no hay franjas, no se permite enviar una.
4. **Método de pago:** debe estar en `payment_methods`, y además:
   - `EFECTIVO_RECOGIDA` exige `RECOGIDA`;
   - `EFECTIVO_DOMICILIO` exige `DOMICILIO`;
   - `DOMICILIO` exige `delivery_enabled`.

   Si falla: `PAYMENT_METHOD_INVALID`.
5. **Productos:** cada uno debe estar visible y disponible (si no: `PRODUCT_UNAVAILABLE`, con `data: {product_id}`). Las opciones se validan como en el POS. El stock se **reserva** en la ubicación por defecto; si no alcanza: 409 `STOCK_INSUFFICIENT`.
6. **Domicilio:** si `fulfillment = DOMICILIO`, `address` (de 5 a 200 caracteres) y `zone_id` activo son obligatorios (si no: `DELIVERY_INVALID`). `delivery_fee = zone.fee`, salvo que `free_from` exista y `subtotal + impuesto ≥ free_from`, en cuyo caso es 0. **Siempre lo calcula el servidor.**
7. **Precios:** el motor de la fase 1 calcula promociones y cupón. Cupón inválido: el código del cupón correspondiente.
8. **Mínimo:** `subtotal + impuesto ≥ min_order`. Si no: `BELOW_MIN_ORDER`, con `data: {min_order}`.
9. **Fiscal:** `fiscal::requires_buyer` con `buyer_identified = (doc_type y doc_number presentes)`. Si devuelve verdadero: `FISCAL_BUYER_REQUIRED`.
10. **Total:** si `money(expected_total)` es distinto del total calculado (con domicilio y sin propina): 409 `PRICE_CHANGED`, con `data` = el desglose.

### Creación, si todo pasa
1. **Cliente:** si viene `doc_number` o `email`, usa `customers::upsert` y pone `customer_id`. Si no, `customer_alias = buyer.name` (recortado a 40 caracteres).
2. **Orden:**
   - `sale_type` es `WHATSAPP` si `payment_method = WHATSAPP`; si no, `ONLINE`;
   - estado `CREADA`, con `fulfillment`, `slot_id`, `slot_date`, `delivery_*`, `buyer_name`, `buyer_email`, `delivery_phone = buyer.phone` y `tracking_token` (32 caracteres hexadecimales aleatorios);
   - líneas con los descuentos ya calculados.
3. **Reserva:** si el método es `ONLINE` o `WHATSAPP`, la reserva vence en `now() + 2 horas`; si es en efectivo, no vence.
4. **`payment_status` y siguiente paso:**

   | Método | `payment_status` | Qué sigue |
   |---|---|---|
   | `ONLINE` | `PENDIENTE_PAGO` | Se crea la transacción de Wompi (F6.4). La respuesta incluye `payment` con los datos del widget. |
   | `EFECTIVO_RECOGIDA` o `EFECTIVO_DOMICILIO` | `CONTRAENTREGA` | La orden se **envía a cocina** de inmediato con la lógica de `fire` de la fase 5 (si tiene productos de cocina). Si es `DOMICILIO`, `delivery_status = PENDIENTE` para armar la ruta. |
   | `WHATSAPP` | `POR_CONFIRMAR` | No se envía a cocina. La respuesta incluye `whatsapp_url` (F6.5). |

5. Crea las notificaciones (fase 7) solo si ya existe el módulo. En esta fase, el POS se entera por *polling* (F6.7).
6. Respuesta (201): `{order_id, tracking_url, total, payment_status, payment?: {...}, whatsapp_url?}`. Se guarda con `idem_finish(HECHO)`.

### Frontend
Pantalla 30, `/tienda/[slug]/carrito`: lista del carrito con sus cantidades; formulario con nombre, teléfono, correo y la opción "¿Necesitás factura?" (si se marca, aparecen el tipo y el número de documento); recogida o domicilio (dirección y zona); franja (si hay); método de pago; cupón; y un resumen calculado con `POST /v1/public/store/{slug}/quote`. Ese endpoint tiene el mismo cuerpo, **sin** idempotencia, no escribe nada y devuelve el desglose o los errores de las validaciones 1 a 9.

El botón "Hacer pedido" genera la `Idempotency-Key` **una sola vez por intento** y la reutiliza en los reintentos de red. Ante `PRICE_CHANGED`, actualiza el resumen y pide confirmar de nuevo.

### Pruebas (`f6_checkout`)
- **[P]** La misma clave con el mismo cuerpo devuelve la misma orden; con otro cuerpo: 409 `IDEMPOTENCY_CONFLICT`. Una clave en `PROCESANDO`: 409 `REQUEST_IN_PROGRESS`.
- **[P]** Una franja llena: `SLOT_UNAVAILABLE`. Dos pedidos simultáneos por el último cupo: uno pasa y el otro falla.
- **[P]** `EFECTIVO_RECOGIDA` con `DOMICILIO`: `PAYMENT_METHOD_INVALID`.
- **[P]** El costo del domicilio es 0 cuando se supera `free_from`, aunque el cliente envíe otro valor (no puede enviarlo).
- **[P]** Un pedido de efectivo con productos de cocina crea un ticket de cocina al instante.
- **[P]** La regla de 5 UVT aplica igual que en el POS.

---

## F6.4 Pago en línea con Wompi

### Variables de entorno del backend
Se leen al iniciar. **Nunca** las escribas en archivos del repositorio:
- `WOMPI_PUBLIC_KEY`
- `WOMPI_INTEGRITY_SECRET`
- `WOMPI_EVENTS_SECRET`
- `WOMPI_ENV` (`sandbox` o `production`)

Si falta cualquiera, "hay llaves" es falso.

### Código (`src/payments/`)
- Trait `PaymentProvider` con:
  - `fn checkout_params(&self, reference: &str, amount_in_cents: i64, redirect_url: &str) -> serde_json::Value`;
  - `fn verify_event(&self, body: &serde_json::Value) -> Result<WompiEvent, ApiError>`.
- **`WompiProvider`:**
  - `checkout_params` devuelve `{public_key, currency: "COP", amount_in_cents, reference, signature_integrity, redirect_url}`, con `signature_integrity = sha256_hex(reference + amount_in_cents + "COP" + integrity_secret)`;
  - `verify_event`:
    1. toma `signature.properties` (por ejemplo `["transaction.id","transaction.status","transaction.amount_in_cents"]`);
    2. concatena los valores de esas rutas dentro de `data`, en ese orden;
    3. agrega `timestamp` (el del cuerpo) y luego el `events_secret`;
    4. calcula el SHA-256 en hexadecimal y lo compara, sin distinguir mayúsculas, con `signature.checksum`;
    5. si no coincide: 401.
- **`MockPaymentProvider`:** solo se activa con `NEXA_PAYMENTS_MOCK=on`. `checkout_params` devuelve `{mock: true, reference}`, y existe el endpoint `POST /v1/public/payments/mock/{reference}/approve`, que solo funciona con el mock activo.

### Referencia
`reference = "<order_id>-<intento>"`. El intento empieza en 1 y se guarda en una columna nueva, `orders.payment_attempt integer not null default 0`.

### Webhook: `POST /v1/public/payments/wompi/webhook` (sin autenticación)
1. `verify_event`.
2. Solo procesa `event = "transaction.updated"`.
3. De `reference` saca el `order_id` y el intento.
4. Si el intento no es el actual, lo ignora y responde 200.
5. Según `transaction.status`:
   - **`APPROVED`:**
     - si `amount_in_cents` es distinto de `total × 100`: registra el error en el log y la auditoría `PAGO_MONTO_INVALIDO`, sin cobrar;
     - si no, cobra la orden reutilizando `/charge` de la fase 5, con un pago de método `ONLINE` por el total, `reference = transaction.id` y `cash_session_id = null`. Hace `payment_status = PAGADO`, quita el vencimiento de la reserva y envía a cocina.
   - **`DECLINED`, `ERROR` o `VOIDED`:** `payment_status = RECHAZADO`. La reserva sigue con su vencimiento de 2 horas.
6. **Idempotente:** si la orden ya está `COBRADA`, responde 200 sin hacer nada.
7. Responde siempre 200 cuando la firma es válida.

**Reintento de pago:** `POST /v1/public/track/{token}/retry-payment`. Solo si `payment_status` es `PENDIENTE_PAGO` o `RECHAZADO` y la orden no está cancelada. Incrementa `payment_attempt`, vuelve a `PENDIENTE_PAGO` y devuelve nuevos `checkout_params`.

### Frontend
Con `payment.mock`, el modo demo muestra el botón "Simular pago aprobado". Si no, inserta el botón del widget de Wompi: un `<form>` con un `<script src="https://checkout.wompi.co/widget.js">` que lleve los atributos `data-render="button"`, `data-public-key`, `data-currency="COP"`, `data-amount-in-cents`, `data-reference`, `data-signature:integrity` y `data-redirect-url` (la URL de seguimiento), tomados de `checkout_params`. No uses ningún otro modo del widget.

### Pruebas (`f6_payments`)
- **[P]** Un webhook con la firma inválida: 401, y la orden no cambia.
- **[P]** Un webhook `APPROVED` válido (firmado en la prueba con un secreto de prueba) cobra la orden; repetido, no cobra dos veces.
- **[P]** Un `APPROVED` con un monto distinto no cobra.
- Unitaria `integrity_signature_known_value`: con `reference = "abc-1"`, `amount_in_cents = 250000` y secreto `"test_integrity"`, la firma debe ser exactamente `6c9f8e65dd78d7a43ea13cf29ca8aaa1a53f9667facf876bd76694d32db2261f` (el SHA-256 de `"abc-1250000COPtest_integrity"`).

---

## F6.5 Finalizar por WhatsApp

`whatsapp_url = "https://wa.me/<whatsapp_phone>?text=" + urlencode(texto)`. Texto exacto, una línea por renglón:

```
Hola, quiero hacer este pedido (#<últimos 6 del order_id>):
- <qty> x <nombre> (<opciones>) $<total línea>
...
Subtotal: $<…>
Domicilio: $<…>          (solo si es DOMICILIO)
Total: $<…>
<Recoger en tienda | Domicilio a: <dirección> (<zona>)>
Nombre: <nombre> - Tel: <teléfono>
Seguimiento: <tracking_url>
```

En el POS, los pedidos `POR_CONFIRMAR` tienen dos botones:
- **"Confirmar":** `POST /v1/orders/{id}/confirm-online` (permiso `pos:sell`). Pasa `payment_status` a `CONTRAENTREGA`, quita el vencimiento de la reserva y envía a cocina.
- **"Rechazar":** `POST /v1/orders/{id}/reject-online`, con `{reason}`. Cancela la orden y libera la reserva.

Si el pedido no se confirma en 2 horas, se cancela solo (F6.8).

**Prueba [P]** (`f6_whatsapp`): un pedido por WhatsApp no crea un ticket de cocina hasta que se confirma; el texto generado contiene el total y el enlace de seguimiento.

---

## F6.6 Seguimiento público

`GET /v1/public/track/{token}` (sin autenticación; con un token inexistente, 404) devuelve:

```
{ display_name, order_short, created_at, fulfillment, payment_status, can_retry_payment,
  steps: [{key, label, done, at}], items: [{qty, name}], total, delivery_fee }
```

Los pasos, en orden:

| `key` | Etiqueta | Se completa cuando |
|---|---|---|
| `RECIBIDO` | Pedido recibido | Siempre |
| `CONFIRMADO` | Confirmado | `payment_status` es `PAGADO` o `CONTRAENTREGA` |
| `PREPARANDO` | En preparación | Algún ticket de cocina está en `EN_PREPARACION` o después |
| `LISTO` | Listo | Todos los tickets están `LISTO` o después, o no hay tickets y está confirmado |
| `EN_CAMINO` | En camino | `delivery_status = EN_RUTA` (solo en domicilio) |
| `ENTREGADO` | Entregado / Recogido | `delivery_status = ENTREGADO`, o la orden está `COBRADA` o `CERRADA` en recogida |

Si la orden está cancelada, devuelve `cancelled: true` con el motivo.

Pantalla 31, `/tienda/seguimiento/[token]`: línea de pasos, resumen del pedido, refresco cada 20 segundos y, si `can_retry_payment`, el botón "Pagar".

---

## F6.7 Pedidos entrantes en el POS

`GET /v1/orders/incoming?branch_id=&since=<iso>` (permiso `pos:sell`) devuelve las órdenes `ONLINE` y `WHATSAPP` no canceladas, de las últimas 24 horas y posteriores a `since`, con `{id, short, sale_type, payment_status, fulfillment, buyer_name, total, created_at, slot}`.

En el POS:
- el botón "Pedidos en línea (N)", en la barra superior, consulta este endpoint cada 10 segundos;
- cuando llega un pedido nuevo, suena un pitido (el mismo oscilador de la fase 5, a 660 Hz) y aparece un aviso;
- el panel tiene las pestañas Por confirmar, Por cobrar (`CONTRAENTREGA` de recogida), En preparación y Listos;
- acción "Cobrar" (recogida en efectivo): abre el cobro de la orden con `/charge`;
- acción "Enviar a ruta" (domicilio): lleva a `/rutas` con el pedido preseleccionado.

---

## F6.8 Cancelación automática

Tarea de `tokio` en `main.rs`, activa con `NEXA_STORE_WORKER=on` (apagada por defecto), que se ejecuta cada 5 minutos y usa el pool del worker de la fase 4. Cancela las órdenes que cumplen `sale_type in (ONLINE, WHATSAPP)`, `status = CREADA`, `payment_status in (PENDIENTE_PAGO, RECHAZADO, POR_CONFIRMAR)` y `created_at < now() − 2 horas`:
- `status = CANCELADA`;
- libera las reservas;
- `void_reason = "Sin pago o sin confirmar en 2 horas"`.

Cada orden va en su propia transacción, con el contexto de su negocio.

**Prueba [P]** (`f6_expiry`): llamando a la función directamente, una orden `PENDIENTE_PAGO` de hace 3 horas se cancela y su stock queda libre; una `PAGADO` no se toca.

---

## Códigos de error nuevos de la fase 6

| Código | Texto |
|---|---|
| `STORE_SLUG_TAKEN` | Esa dirección de tienda ya está en uso. |
| `STORE_HOURS_INVALID` | Revisá el horario. |
| `PAYMENTS_NOT_CONFIGURED` | El pago en línea todavía no está configurado. |
| `REQUEST_IN_PROGRESS` | Tu pedido se está procesando. Esperá un momento. |
| `BUYER_INVALID` | Revisá tu nombre y tu teléfono. |
| `STORE_CLOSED` | La tienda está cerrada en este momento. |
| `SLOT_UNAVAILABLE` | Ese horario ya no está disponible. |
| `PAYMENT_METHOD_INVALID` | Ese medio de pago no está disponible para tu pedido. |
| `PRODUCT_UNAVAILABLE` | Un producto ya no está disponible. |
| `BELOW_MIN_ORDER` | El pedido no alcanza el mínimo. |

## Lista de verificación
- [ ] Una migración que corre limpia.
- [ ] Todas las pruebas **[P]** y las de fases anteriores en verde.
- [ ] Ninguna llave, secreto ni URL de producción en el código o en el reporte.
- [ ] Los workers están apagados por defecto.
- [ ] El modo demo funciona de punta a punta: catálogo → carrito → pedido en efectivo → aparece en el POS y en la cocina → seguimiento. También el pago simulado con el mock.

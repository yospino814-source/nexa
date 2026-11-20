# Fase 5: Mesas, comandas, cocina (KDS), domicilios y rutas

> **POSPUESTA.** No se construye para la salida a licorerías y tiendas. Ver `antigravity/modulos/00-INDICE.md`.

> **Requisito:** la fase 4 debe estar aprobada.
>
> Esta fase tiene **dos entregas, cada una con su alto obligatorio**:
> - **5A**: mesas, comandas, cobro de órdenes abiertas y cocina. Rama `feat/fase-5a-restaurante`, a partir de `feat/fase-4-facturacion`. Al terminar, escribe `REPORTE-FASE-5A.md` y **detente**.
> - **5B**: domicilios, rutas y app del repartidor. Rama `feat/fase-5b-domicilios`, a partir de 5A ya aprobada. Al terminar, escribe `REPORTE-FASE-5B.md`.
>
> **Migraciones:**
> - 5A: `20261108000100_fase5a_restaurante.sql`
> - 5B: `20261109000100_fase5b_domicilios.sql`
>
> Todo lo que no esté escrito aquí está prohibido.

**Fuera de alcance:**
- bloqueo de estación de cocina por usuario;
- pedido desde la mesa por QR (en esta fase el QR solo muestra la carta);
- reservas de mesa con fecha y hora: solo existe el estado manual `RESERVADA`;
- Supabase Realtime (se usa *polling*);
- recetas con merma o subrecetas, y órdenes de producción;
- optimización de rutas;
- lectura de planillas con IA.

## Permisos nuevos

```sql
insert into public.permissions (code, module, description) values
  ('tables:operate','restaurant','Abrir mesas, tomar pedidos y enviarlos a cocina'),
  ('kitchen:operate','restaurant','Operar la pantalla de cocina'),
  ('deliveries:manage','deliveries','Crear y despachar rutas de domicilio'),
  ('deliveries:drive','deliveries','Operar mis entregas como repartidor')
on conflict (code) do nothing;
```

`role_permissions` (va en la migración de 5A y en la de 5B, cada permiso en la suya):

| Rol | Permisos |
|---|---|
| `DUENO`, `ADMIN_CAJA` | Todos. |
| `SUPERVISOR` | `tables:operate`, `kitchen:operate` y `deliveries:manage`. |
| `MESERO` | `tables:operate`. |
| `COCINA` | `kitchen:operate`. |
| `CAJERO` | `tables:operate`. |
| `REPARTIDOR` | `deliveries:drive`. |

---

# ENTREGA 5A

## F5.1 Nuevo ciclo de la orden abierta

Hoy una orden de mesa pasa por `CREADA → EN_PREPARACION → LISTA → ENTREGADA → COBRADA → CERRADA`, y solo admite líneas en `CREADA`. **Eso cambia:**

- La orden queda en **`CREADA`** ("abierta") mientras se le agregan rondas. La cocina se sigue **por línea y por ticket**, no por orden.
- Estados de la orden que se usan desde ahora: `CREADA` → `COBRADA` → `CERRADA`, más `CANCELADA`.
- `EN_PREPARACION`, `LISTA` y `ENTREGADA` **dejan de usarse** para órdenes nuevas.
- `POST /v1/orders/{id}/transition` responde 410 `ENDPOINT_RETIRED`. `POST /v1/orders/{id}/confirm` pasa a ser un alias de `/fire` (F5.4).
- Ajusta las pruebas viejas del flujo de órdenes a este ciclo y explica en el reporte cada prueba que cambiaste.

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Entregado','ENTREGADO'),('Cancelado','CANCELADO')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ITEM') h
on conflict (padre_id, tima_codigo) do nothing;
alter table public.order_items
  add column fired_at timestamptz,
  add column cancel_reason text,
  add column added_by uuid references public.users(id);
update public.order_items set status_id = public.tipo_id('ITEM','PENDIENTE') where status_id is null;
alter table public.order_items alter column status_id set not null;
alter table public.order_items alter column status_id set default public.tipo_id('ITEM','PENDIENTE');
```

### Estados de una línea

| Estado | Significado |
|---|---|
| `PENDIENTE` con `fired_at` nulo | Sin enviar. |
| `EN_PREPARACION` | Enviada a cocina. |
| `LISTO` | La cocina la terminó. |
| `ENTREGADO` | Llevada a la mesa. |
| `CANCELADO` | Cancelada. |

Las líneas `CANCELADO` **no cuentan** en `recalc_totals` (agrega `where status_id <> tipo_id('ITEM','CANCELADO')`), ni en el cobro, ni en el documento fiscal.

### Endpoints (`orders.rs`, permiso `tables:operate`)
- **`POST /v1/orders/{id}/lines`**: cuerpo `{product_id, qty, tier_id?, option_ids?, unit_price_override?, override_reason?, notes? (≤140)}`. Solo si la orden está en `CREADA`; si no: 409 `ORDER_NOT_OPEN`. El precio se resuelve con las **mismas** funciones de la venta (tarifa, opciones y cambio de precio, con sus permisos). Se guarda `added_by`. Nunca se fusiona con otra línea igual.
- **`POST /v1/orders/{id}/lines/{line_id}/cancel`**: cuerpo `{reason (3..140)}`.
  - Línea sin enviar: se **borra**.
  - Línea enviada: exige `pos:refund` (si no lo tiene: 403). Libera su reserva, pasa a `CANCELADO` con `cancel_reason` y marca su ítem en el ticket de cocina como cancelado (F5.4). Auditoría `LINEA_CANCELADA`.
- **`POST /v1/orders/{id}/lines/{line_id}/deliver`**: de `LISTO` a `ENTREGADO`. Desde `EN_PREPARACION` también se permite cuando el producto no va a cocina. Desde cualquier otro estado: 409 `LINE_INVALID_STATE`.
- **`PUT /v1/orders/{id}/lines/{line_id}/discount`**: cuerpo `{discount_pct}`. Mismas reglas que el descuento manual de la venta (`pos:discount` y el máximo de la sucursal). Se guarda en una columna nueva, `order_items.manual_pct numeric(5,2)`, que el cobro usa como `manual_pct`.

## F5.2 Mesas y sesiones

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('En limpieza','LIMPIEZA'),('Reservada','RESERVADA')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'MESA') h
on conflict (padre_id, tima_codigo) do nothing;
alter table public.restaurant_tables
  add column zone text not null default 'Salón' check (length(zone) between 1 and 30),
  add column capacity integer not null default 4 check (capacity between 1 and 30),
  add column pos_x integer not null default 0 check (pos_x between 0 and 100),
  add column pos_y integer not null default 0 check (pos_y between 0 and 100),
  add column shape text not null default 'CUADRADA' check (shape in ('CUADRADA','REDONDA','RECTANGULAR')),
  add column active boolean not null default true;
create table public.table_sessions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  table_id uuid not null,
  guests integer not null default 1 check (guests between 1 and 50),
  waiter_id uuid references public.users(id),
  opened_by uuid not null references public.users(id),
  opened_at timestamptz not null default now(),
  closed_at timestamptz,
  unique (tenant_id, id),
  foreign key (tenant_id, table_id) references public.restaurant_tables (tenant_id, id)
);
create unique index table_sessions_one_open on public.table_sessions (table_id) where closed_at is null;
alter table public.orders add column table_session_id uuid,
  add column parent_order_id uuid,
  add constraint orders_table_session_fkey foreign key (tenant_id, table_session_id) references public.table_sessions (tenant_id, id);
-- RLS de table_sessions con in_scope; grants.
create function public.resolve_table_token(p_token text)
returns table (tenant_id uuid, branch_id uuid, table_id uuid, table_code text)
language sql stable security definer set search_path = '' as $$
  select t.tenant_id, t.branch_id, t.id, t.code from public.restaurant_tables t
   where t.qr_token = p_token and t.active
$$;
revoke execute on function public.resolve_table_token(text) from public, anon;
grant execute on function public.resolve_table_token(text) to authenticated;
```

### Estados de la mesa

| Desde | Hacia | Cuándo |
|---|---|---|
| `LIBRE` | `OCUPADA` | Al abrir la mesa. |
| `OCUPADA` | `LIMPIEZA` | Cuando todas las órdenes de la sesión quedan `COBRADA` o `CANCELADA`; en ese momento se cierra la sesión (`closed_at`) y las órdenes `COBRADA` pasan a `CERRADA`. |
| `LIMPIEZA` | `LIBRE` | `POST /v1/tables/{id}/clean`. |
| `LIBRE` | `RESERVADA` | `POST /v1/tables/{id}/reserve`, de forma manual. |
| `RESERVADA` | `LIBRE` | `POST /v1/tables/{id}/release`, de forma manual. |
| `RESERVADA` | `OCUPADA` | Al abrir la mesa. |

### Endpoints (`src/tables.rs`)

| Método y ruta | Permiso | Detalle |
|---|---|---|
| `GET /v1/tables?branch_id=` | `tables:operate` | `[{id, code, zone, capacity, pos_x, pos_y, shape, status, session: null \| {id, guests, waiter_name, opened_at, orders: [{id, total, lines_count, unsent_count, ready_count}]}}]` |
| `POST /v1/tables` y `PUT /v1/tables/{id}` | `settings:write` | `{code, zone, capacity, pos_x, pos_y, shape, active}` |
| `POST /v1/tables/{id}/open` | `tables:operate` | `{guests, waiter_id?}`. En una transacción: bloquea la mesa (`for update`); su estado debe ser `LIBRE` o `RESERVADA` (si no: 409 `TABLE_BUSY`); crea la sesión, crea la orden (`sale_type = MESA`, `CREADA`, `table_session_id`, `waiter_id`) y deja la mesa `OCUPADA`. Si el índice único falla por una carrera (23505), responde 409 `TABLE_BUSY`. |
| `POST /v1/tables/{id}/move` | `tables:operate` | `{to_table_id}`. La mesa destino debe estar `LIBRE`. Mueve la sesión y sus órdenes (`table_id`); el origen queda `LIBRE` y el destino `OCUPADA`. |
| `POST /v1/tables/{id}/merge` | `tables:operate` | `{from_table_id}`. Mueve todas las líneas no canceladas de las órdenes `CREADA` de la sesión de origen a la orden principal (la más antigua `CREADA`) de esta mesa. Las órdenes de origen quedan `CANCELADA`, con la nota "Unida a la mesa X"; su sesión se cierra y la mesa de origen queda `LIBRE`. Las reservas de stock se reasignan a la orden de destino (`update stock_reservations set order_id`). |
| `POST /v1/tables/{id}/clean`, `/reserve` y `/release` | `tables:operate` | Validan el estado de origen. Si no corresponde: 409 `TABLE_INVALID_STATE`. |

### Pruebas (`f5_tables`)
- **[P]** Dos aperturas simultáneas de la misma mesa: una pasa y la otra recibe 409 `TABLE_BUSY`, nunca un 500.
- **[P]** Cobrar la única orden deja la mesa en `LIMPIEZA` y cierra la sesión.
- **[P]** Mover a una mesa ocupada: 409.
- **[P]** Unir mesas conserva todas las líneas y sus reservas.

## F5.3 Cobro de una orden abierta

**Refactor obligatorio:** separa `checkout_tx` en funciones reutilizables (pon los nombres exactos en el reporte):
- `price_lines_tx`: ejecuta el motor sobre líneas ya guardadas;
- `apply_customer_rules`;
- `register_payments_tx`;
- `finalize_sale_tx`: número, documento fiscal y estado `COBRADA`.

`POST /v1/sales` las usa sin cambiar su comportamiento. **Todas las pruebas de venta siguen pasando.**

### `POST /v1/orders/{id}/charge`
Permiso `pos:sell`. Cuerpo: `{payments, quoted_total, customer_id?, customer_alias?, coupon_code?, tip_amount?, tip_waiter_id?, credit_plan?, credit_override_reason?}`.

1. Bloquea la orden. Si está `COBRADA` y su `client_request` es igual a este cuerpo, devuelve la venta. Si el cuerpo es distinto: 409 `IDEMPOTENCY_CONFLICT`. Si no está `CREADA`: 409 `ORDER_NOT_OPEN`.
2. Si hay líneas no canceladas sin enviar: 409 `ORDER_HAS_UNSENT_LINES`.
3. Valida el turno y el cliente, igual que la venta.
4. Ejecuta el motor de precios (`price_lines_tx`) sobre las líneas no canceladas. `unit_price`, `tier_id` y el cambio de precio ya están fijos. Recalcula promociones, cupón y el `manual_pct` de cada línea, y actualiza las columnas de descuento.
5. `quoted_total` (si no coincide: 409 `PRICE_CHANGED`), la regla de 5 UVT, los pagos, la venta a crédito y el saldo a favor: todo igual que en la venta.
6. Stock: `inventory::consume_order_tx`, que ya debe pasar por `apply_delta_tx` desde la fase 2, consume las reservas de las líneas.
7. `finalize_sale_tx`, y guarda `client_request`.
8. Si la orden tiene mesa: aplica la regla de la mesa (F5.2).

`GET /v1/orders/{id}/quote` (permiso `pos:sell`) devuelve la cotización de la orden abierta, con el mismo formato que `/v1/sales/quote` y parámetros `coupon_code`, `customer_id` y `tip_amount` opcionales.

### Dividir la cuenta por ítems
`POST /v1/orders/{id}/split` (permiso `tables:operate`), cuerpo `{groups: [[{order_item_id, qty}]]}`.
1. La orden debe estar `CREADA` y todas sus líneas enviadas (si no: 409 `ORDER_HAS_UNSENT_LINES`).
2. Cada grupo se convierte en una orden nueva: `CREADA`, misma mesa y sesión, `parent_order_id = id`.
   - Si `qty` es menor que la de la línea, la línea se parte en dos filas: una se queda y otra se mueve, con los mismos `unit_price`, `tier_id`, opciones, notas y `fired_at`. La reserva se parte igual: actualiza la cantidad de la reserva original e inserta una nueva para la parte movida.
   - Si `qty` es igual a la de la línea, la línea se mueve completa (`update order_items set order_id`).
3. Si una cantidad supera lo disponible en la línea, o si la orden original quedaría vacía: 422 `SPLIT_INVALID`.
4. Recalcula los totales de todas las órdenes.
5. Respuesta: `{orders: [Order]}`.

**Dividir por montos** no es un endpoint: en el diálogo de cobro, el botón "Dividir en N" reparte el total en N pagos (hasta 5), que se pueden editar uno por uno.

### Pruebas (`f5_charge`)
- **[P]** Cobrar una orden con una línea sin enviar: 409.
- **[P]** Una línea cancelada no cuenta en el total ni en el documento fiscal.
- **[P]** Dividir 3 cervezas en 1 y 2 crea dos órdenes con los totales correctos; cobrar las dos libera la mesa.
- **[P]** Un segundo cobro con el mismo cuerpo devuelve la misma venta.

## F5.4 Cocina (KDS)

### Migración

```sql
alter table public.kds_stations
  add column is_default boolean not null default false,
  add column prep_minutes integer not null default 10 check (prep_minutes between 1 and 120),
  add column print_mode boolean not null default false;
create unique index kds_stations_one_default on public.kds_stations (branch_id) where is_default;
insert into public.kds_stations (tenant_id, branch_id, name, is_default)
select b.tenant_id, b.id, 'Cocina', true from public.branches b
where not exists (select 1 from public.kds_stations k where k.branch_id = b.id);
update public.kds_stations k set is_default = true
 where not exists (select 1 from public.kds_stations k2 where k2.branch_id = k.branch_id and k2.is_default)
   and k.id = (select min(id) from public.kds_stations k3 where k3.branch_id = k.branch_id);
alter table public.products
  add column goes_to_kitchen boolean not null default false,
  add column kds_station_id integer references public.kds_stations(id);
update public.products set goes_to_kitchen = true where kind = 'PREPARADO';

create table public.kds_daily_counters (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  station_id integer not null references public.kds_stations(id) on delete cascade,
  day date not null,
  next_number integer not null default 1,
  primary key (station_id, day),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.kitchen_tickets (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  station_id integer not null references public.kds_stations(id),
  order_id uuid not null,
  day date not null,
  daily_number integer not null,
  status text not null default 'PENDIENTE' check (status in ('PENDIENTE','EN_PREPARACION','LISTO','ENTREGADO','CANCELADO')),
  table_code text, waiter_name text,
  created_at timestamptz not null default now(),
  started_at timestamptz, ready_at timestamptz, delivered_at timestamptz,
  unique (tenant_id, id),
  unique (station_id, day, daily_number),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id)
);
create table public.kitchen_ticket_items (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  ticket_id uuid not null,
  order_item_id uuid not null,
  name text not null, qty numeric(14,4) not null, notes text, options text,
  cancelled boolean not null default false,
  primary key (ticket_id, order_item_id),
  foreign key (tenant_id, ticket_id) references public.kitchen_tickets (tenant_id, id) on delete cascade
);
-- RLS: kitchen_tickets y kds_daily_counters con in_scope(tenant_id, branch_id); kitchen_ticket_items a nivel de negocio. Grants.
```

### Enviar a cocina: `POST /v1/orders/{id}/fire` (permiso `tables:operate`)
1. Bloquea la orden. Debe estar `CREADA`; si no: 409 `ORDER_NOT_OPEN`.
2. Toma las líneas `PENDIENTE` con `fired_at` nulo. Si no hay: 409 `NOTHING_TO_FIRE`.
3. Reserva su stock con la lógica del `confirm_tx` actual, solo para esas líneas e incluyendo los insumos de opciones y recetas.
4. Agrupa las líneas con `goes_to_kitchen = true` por estación: la del producto, o la estación por defecto de la sucursal. Por cada estación:
   1. Toma el número del día (fecha en la zona de la sucursal) con esta sentencia exacta: `insert into kds_daily_counters (tenant_id, branch_id, station_id, day, next_number) values ($1, $2, $3, $4, 2) on conflict (station_id, day) do update set next_number = kds_daily_counters.next_number + 1 returning next_number - 1`. El primer ticket del día recibe el 1.
   2. Crea el ticket con `table_code` y `waiter_name`.
   3. Crea un ítem por línea, con su nombre, cantidad, nota y opciones en texto (`"Extra queso, Sin cebolla"`). **Las líneas nunca se fusionan:** dos líneas iguales con notas distintas son dos ítems.
5. Las líneas que van a cocina quedan `EN_PREPARACION`. Las que no van quedan `LISTO`. A todas se les pone `fired_at = now()`.
6. Respuesta: `{tickets: [{id, station_id, station_name, daily_number, print_mode}]}`.

### Endpoints de cocina (permiso `kitchen:operate`)
- `GET /v1/kitchen/tickets?station_id=&scope=active|done`:
  - `active` = `PENDIENTE`, `EN_PREPARACION` y `LISTO`, ordenados por `created_at`;
  - `done` = los últimos 50 `ENTREGADO` o `CANCELADO` del día;
  - cada ticket trae `items` y `elapsed_seconds`.
- `POST /v1/kitchen/tickets/{id}/status`, cuerpo `{status}`. Transiciones:
  - `PENDIENTE → EN_PREPARACION`, que fija `started_at`;
  - `EN_PREPARACION → LISTO`, que fija `ready_at` y pasa sus líneas no canceladas a `LISTO`;
  - `LISTO → ENTREGADO`, que fija `delivered_at` y pasa sus líneas a `ENTREGADO`;
  - cualquier estado no final `→ CANCELADO`, que exige `pos:refund` y cancela sus líneas como en F5.1.

  Cualquier otra transición: 409 `TICKET_INVALID_STATE`.
- **Cocina:** `GET`, `POST` y `PUT /v1/kds-stations` (permiso `settings:write`).

### Frontend
- **Pantalla 23 `/cocina`:** `app/(operacion)/cocina/page.tsx` y `components/cocina/kds-screen.tsx`.
  - Selector de estación y una columna por estado activo.
  - Cada tarjeta muestra el número del día en grande, la mesa, el mesero, el tiempo transcurrido (mm:ss), los ítems con su nota y opciones, y un botón para avanzar.
  - El borde de la tarjeta es normal mientras `elapsed < prep_minutes`, **amarillo** desde `prep_minutes` y **rojo** desde `prep_minutes + 5`.
  - *Polling* cada 3 segundos. Cuando aparece un ticket nuevo (un `id` que antes no estaba), suena un pitido generado con Web Audio (un oscilador de 880 Hz durante 200 ms). No uses archivos de audio.
- **Modo impreso:** si la estación tiene `print_mode`, la pantalla del mesero, al recibir la respuesta de `fire`, imprime una comanda por ticket con `PrintArea`: estación, número, mesa, mesero, hora e ítems con notas.

### Pruebas (`f5_kitchen`)
- **[P]** La numeración diaria es independiente por estación: cocina y bar tienen cada una su ticket #1 el mismo día.
- **[P]** Dos líneas del mismo producto, una "sin papas", generan dos ítems en el ticket.
- **[P]** `fire` reserva el stock de los insumos de la receta y no reserva nada para las líneas ya enviadas.
- **[P]** Un ticket `LISTO` pasa sus líneas a `LISTO`.
- **[P]** Un usuario sin `kitchen:operate` que avanza un ticket: 403.

## F5.5 Pantallas de mesas y comanda

- **Pantalla 21 `/mesas`:** `components/mesas/tables-screen.tsx`.
  - El plano dibuja las mesas en `pos_x` y `pos_y` (porcentaje del contenedor), con su forma, una pestaña por zona y un color por estado: libre en gris, ocupada en azul, limpieza en amarillo y reservada en morado.
  - Una mesa ocupada muestra los comensales, el tiempo abierta, el total y la insignia "N listos".
  - Al hacer clic en una mesa libre, se abre el diálogo "Abrir mesa" (comensales y mesero). En una ocupada, se abre la comanda.
  - El botón "Editar plano" (solo con `settings:write`) permite arrastrar mesas, que se guardan con `PUT`, y crear mesas nuevas.
- **Pantalla 22 `/comanda/[orderId]`:** `components/mesas/order-screen.tsx`.
  - Catálogo a la izquierda (el mismo del POS).
  - Lista de líneas a la derecha, agrupadas en "Sin enviar" y "Enviadas", con su estado.
  - Acciones por línea: nota, cancelar y entregar.
  - Botones: "Enviar a cocina", "Precuenta", "Dividir", "Mover", "Unir" y "Cobrar". "Cobrar" abre el diálogo de cobro del POS, alimentado por `GET /v1/orders/{id}/quote` y `POST /charge`.
- **Pantalla 24 (cuenta de mesa):** es el diálogo de cobro y el de dividir. Cambia su ruta en `screens.ts` a `/comanda` y márcala `"react"`.

## F5.6 Carta pública por QR

- **Backend:** `GET /v1/public/menu/{token}`, **sin autenticación**.
  1. Llama a `public.resolve_table_token` con una conexión del pool normal, dentro de una transacción con `set local role authenticated` y `request.jwt.claims` vacío.
  2. Con el `tenant_id` y el `branch_id` obtenidos, abre la transacción con un helper nuevo, `tenancy::begin_public(pool, tenant, branch)`, que fija el contexto **sin usuario**.
  3. Devuelve `{business_name, branch_name, table_code, categories: [{name, products: [{name, description, price_with_tax, options_summary}]}]}`, solo con los productos `VENTA`, disponibles y sin archivar.
  4. Si el token no existe: 404.
  5. Límite: 60 peticiones por minuto por IP. Hazlo en memoria con `HashMap` y `Mutex`, sin crates nuevos.
- **Frontend:** página pública `app/(publico)/m/[token]/page.tsx`. Es solo lectura, pensada para celular: logo, nombre, mesa, buscador y la carta por categorías con su precio. **No tiene** carrito ni botón de pedir.
- **En `/mesas`**, la acción "Código QR" de cada mesa muestra e imprime el QR de `<origen>/m/<qr_token>`.

**Prueba [P]** (`f5_public_menu`): un token válido devuelve la carta sin los insumos; un token inválido da 404; y la respuesta no incluye costos.

### Fin de la entrega 5A
Escribe `REPORTE-FASE-5A.md` y **detente**.

---

# ENTREGA 5B

## F5.7 Domicilios

### Migración

```sql
alter table public.orders
  add column delivery_address text check (length(delivery_address) between 5 and 200),
  add column delivery_phone text check (delivery_phone ~ '^[0-9]{7,15}$'),
  add column delivery_notes text check (length(delivery_notes) <= 200),
  add column delivery_fee numeric(14,2) not null default 0 check (delivery_fee >= 0),
  add column delivery_status text check (delivery_status in ('PENDIENTE','EN_RUTA','ENTREGADO','PARCIAL','RECHAZADO'));
```

`recalc_totals` pasa a calcular `total = subtotal + impuesto + propina + delivery_fee`. **El domicilio no lleva impuesto y no entra al documento fiscal**, igual que la propina.

### `POST /v1/deliveries` (permiso `pos:sell`)
Cuerpo: `{id, branch_id, customer_id? | customer_alias?, address, phone, notes?, delivery_fee, items (SaleItem), pay_now: bool, payments?, quoted_total, coupon_code?}`.

- **Con `pay_now = true`:** es una venta normal (`checkout_tx`) con `sale_type = DOMICILIO`, los campos de domicilio y `delivery_status = PENDIENTE`. El repartidor solo entrega.
- **Con `pay_now = false`** (contra entrega):
  1. Crea la orden `CREADA` con `sale_type = DOMICILIO`, sus líneas y sus precios. El precio queda fijado con el motor en este momento: guarda las columnas de descuento y `quoted_total` debe coincidir.
  2. Reserva el stock como en `fire`, y si hay productos de cocina, crea los tickets.
  3. `delivery_status = PENDIENTE`.
  4. No hay pagos todavía.

Otras reglas:
- `address` y `phone` son obligatorios. Si el cliente los tiene, el POS los precarga.
- Si `pay_now = false`, `payments` no puede venir: 422 `DELIVERY_INVALID`.

### Frontend (POS)
Interruptor "Domicilio" en el carrito, que muestra los campos dirección, teléfono, notas, costo del domicilio y "¿Paga ahora?". El total incluye el domicilio. Con "Paga ahora" desactivado, el botón dice "Crear pedido" en lugar de "Cobrar".

## F5.8 Vehículos, rutas y liquidación

### Migración

```sql
create table public.vehicles (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  plate text not null check (plate ~ '^[A-Z0-9]{5,7}$'),
  kind text not null check (kind in ('MOTO','CARRO','BICICLETA','OTRO')),
  driver_id uuid references public.users(id),
  active boolean not null default true,
  unique (tenant_id, id), unique (tenant_id, plate),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.routes (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  vehicle_id uuid, driver_id uuid not null references public.users(id),
  status text not null default 'BORRADOR' check (status in ('BORRADOR','DESPACHADA','EN_TRANSITO','CERRADA')),
  dispatched_at timestamptz, closed_at timestamptz,
  cash_expected numeric(14,2), cash_declared numeric(14,2), cash_difference numeric(14,2),
  closed_by uuid references public.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  foreign key (tenant_id, vehicle_id) references public.vehicles (tenant_id, id)
);
create table public.route_stops (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  route_id uuid not null,
  order_id uuid not null,
  seq integer not null check (seq >= 1),
  status text not null default 'PENDIENTE' check (status in ('PENDIENTE','ENTREGADO','PARCIAL','RECHAZADO','LIBERADO')),
  collected_amount numeric(14,2) not null default 0 check (collected_amount >= 0),
  collected_method text check (collected_method in ('EFECTIVO','DATAFONO','QR','CREDITO')),
  reject_reason text,
  lat numeric(9,6), lng numeric(9,6),
  settled_at timestamptz,
  unique (tenant_id, id), unique (route_id, seq),
  foreign key (tenant_id, route_id) references public.routes (tenant_id, id) on delete cascade,
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id)
);
create unique index route_stops_one_active_per_order on public.route_stops (order_id) where status = 'PENDIENTE';
create table public.delivery_proofs (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  stop_id uuid not null,
  kind text not null check (kind in ('FOTO','FIRMA')),
  mime text not null check (mime in ('image/jpeg','image/png')),
  data bytea not null check (octet_length(data) <= 512000),
  created_at timestamptz not null default now(),
  foreign key (tenant_id, stop_id) references public.route_stops (tenant_id, id) on delete cascade
);
create table public.route_shortages (
  route_id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  driver_id uuid not null references public.users(id),
  amount numeric(14,2) not null check (amount > 0),
  settled boolean not null default false,
  foreign key (tenant_id, route_id) references public.routes (tenant_id, id)
);
-- RLS: vehicles y routes con in_scope; las demás a nivel de negocio. Grants.
```

### Rutas (`src/routes_delivery.rs`; no lo llames `routes.rs`, que ya existe)

| Acción | Permiso | Reglas |
|---|---|---|
| `POST /v1/delivery-routes` `{branch_id, driver_id, vehicle_id?, order_ids: [uuid]}` | `deliveries:manage` | Número de `sequences` `RUTA`. Cada orden debe ser `DOMICILIO` con `delivery_status = PENDIENTE` y sin una parada `PENDIENTE` en otra ruta; si no: 422 `ROUTE_INVALID`. Crea las paradas con `seq` en el orden recibido. El conductor debe tener membresía activa en la sucursal. |
| `PUT /v1/delivery-routes/{id}` `{order_ids, driver_id, vehicle_id?}` | `deliveries:manage` | Solo en `BORRADOR`. Reemplaza las paradas. |
| `POST /v1/delivery-routes/{id}/dispatch` | `deliveries:manage` | `BORRADOR → DESPACHADA`. Exige al menos una parada. Sus órdenes pasan a `delivery_status = EN_RUTA`. |
| `POST /v1/delivery-routes/{id}/start` | `deliveries:drive` (solo el conductor de la ruta) | `DESPACHADA → EN_TRANSITO`. |
| `POST /v1/delivery-stops/{id}/settle` | `deliveries:drive` (solo el conductor) o `deliveries:manage` | Ver abajo. |
| `POST /v1/delivery-stops/{id}/proof` | `deliveries:drive` | `multipart` con `kind` y `file` (JPEG o PNG de 500 KB como máximo). Máximo 1 foto y 1 firma por parada; una nueva reemplaza la anterior. |
| `POST /v1/delivery-routes/{id}/close` `{cash_declared}` | `deliveries:manage` | Ver abajo. |
| `GET /v1/delivery-routes?branch_id=&status=` y `GET /v1/delivery-routes/{id}` | `deliveries:manage`, o `deliveries:drive` (solo sus rutas) | La ruta con sus paradas, la orden, la dirección, el teléfono, el saldo por cobrar y si tiene prueba de entrega. |
| `GET /v1/delivery-proofs/{id}` | igual | Devuelve la imagen con su `Content-Type`. |

Cualquier acción desde un estado no permitido: 409 `ROUTE_INVALID_STATE`.

### Liquidar una parada
Cuerpo: `{result: "ENTREGADO"|"PARCIAL"|"RECHAZADO"|"LIBERADO", collected_amount, collected_method?, delivered_items?: [{order_item_id, qty}], reject_reason?, lat?, lng?}`. La ruta debe estar `EN_TRANSITO` y la parada `PENDIENTE`.

`saldo` = lo que falta cobrar de la orden: `total` si la orden está `CREADA`, o 0 si ya está `COBRADA` (pagó al crearla).

- **`ENTREGADO`:**
  - Si `saldo = 0`: `collected_amount` debe ser 0.
  - Si `saldo > 0` y `collected_method` es distinto de `CREDITO`: `collected_amount == saldo`, comparado en centavos. Si no: 422 `STOP_AMOUNT_MISMATCH`, con `data: {due: saldo}`.
  - Si `collected_method = CREDITO`: `0 <= collected_amount <= saldo` y la orden tiene un cliente con cupo. Se cobra con dos pagos: `collected_amount` en `EFECTIVO` (si es mayor que 0) y el resto en `CREDITO`, con la cuenta por cobrar de la fase 3.
  - Si la orden está `CREADA`, se cobra **reutilizando `/charge`** (F5.3) con `cash_session_id = null`. Los pagos quedan sin turno, porque el efectivo entra a la caja al cerrar la ruta.
  - `delivery_status = ENTREGADO`.
- **`PARCIAL`:** exige `delivered_items`, con cantidades menores o iguales a las de cada línea y al menos una menor. Antes de cobrar:
  - las líneas no entregadas se cancelan (con la misma lógica de F5.1 y el motivo "No entregado");
  - las entregadas a medias se parten en dos: se cancela la parte no entregada y se libera su reserva.

  Luego se cobra como `ENTREGADO`, sobre el nuevo total. `delivery_status = PARCIAL`.
- **`RECHAZADO`:** exige `reject_reason`. Si la orden está `CREADA`, se cancela y libera su stock. Si ya está `COBRADA`, se registra una devolución total con `REINTEGRAR` y método `ORIGINAL`, creada y aprobada por el sistema con el usuario que liquida. `delivery_status = RECHAZADO`.
- **`LIBERADO`:** la parada queda `LIBERADO` y la orden vuelve a `delivery_status = PENDIENTE` para otra ruta.

En todos los casos se guardan `settled_at`, `lat` y `lng`.

### Cerrar la ruta
1. Todas las paradas deben estar en un estado distinto de `PENDIENTE`. Si no: 409 `ROUTE_HAS_PENDING_STOPS`.
2. `cash_expected = Σ collected_amount` de las paradas con método `EFECTIVO`.
3. `cash_difference = cash_declared − cash_expected`.
4. **Ingreso a la caja:** si `cash_declared > 0`, el usuario que cierra necesita un turno abierto (si no: 409 `ROUTE_NEEDS_OPEN_SESSION`). Se registra `cash::record_movement_tx` con el tipo `INGRESO`, el motivo "Liquidación ruta RUT-000123", `ref_type = 'ROUTE'` y `ref_id`, por `cash_declared`.
5. Si `cash_difference < 0`: inserta en `route_shortages` el faltante (valor positivo) a cargo del conductor.
6. `status = CERRADA`, `closed_at` y `closed_by`. Auditoría `RUTA_CERRADA`.

### Pruebas (`f5_routes`)
- **[P]** Liquidar como entregado un contra entrega con un monto distinto al saldo: 422 `STOP_AMOUNT_MISMATCH`. Con el monto exacto, la orden queda `COBRADA` y su stock sale.
- **[P]** Una entrega parcial cancela lo no entregado, libera su reserva y cobra solo lo entregado.
- **[P]** Un contra entrega con un cliente a crédito puede cobrar menos; el resto queda en cartera.
- **[P]** Cerrar con una parada pendiente: 409.
- **[P]** Cerrar declarando menos de lo cobrado crea un `route_shortages` y un `INGRESO` en la caja por lo declarado.
- **[P]** Un repartidor no puede liquidar paradas de una ruta de otro conductor: 403.
- **[P]** Una foto de más de 500 KB: 422.

## F5.9 Pantallas de domicilios

- **Pantalla 26 `/rutas`:** `components/rutas/routes-screen.tsx`, permiso `deliveries:manage`.
  - A la izquierda, los pedidos con `delivery_status = PENDIENTE`, con sus casillas.
  - Botón "Crear ruta": elige conductor y vehículo; el orden de las paradas se cambia arrastrando.
  - Lista de rutas por estado.
  - Detalle de la ruta: paradas con su resultado, la prueba de entrega (miniaturas) y el botón "Cerrar ruta" (pide el efectivo declarado y muestra la diferencia en vivo).
  - La pestaña "Vehículos" es el CRUD de vehículos.
- **Pantalla 28 `/repartidor`:** página `app/(operacion)/repartidor/page.tsx`, diseñada **para celular**, permiso `deliveries:drive`.
  - Lista "Mis rutas" (`DESPACHADA` y `EN_TRANSITO`) y el botón "Iniciar ruta".
  - Por parada: dirección, botón "Navegar" (abre `https://www.google.com/maps/dir/?api=1&destination=<dirección codificada>`), botón "Llamar" (`tel:`), el saldo a cobrar y el botón "Entregar".
  - "Entregar" lleva a una pantalla con:
    - resultado (cuatro botones);
    - en `PARCIAL`, la lista de líneas con cantidades;
    - monto cobrado (precargado con el saldo) y método;
    - foto con `<input type="file" accept="image/*" capture="environment">`, comprimida en el navegador a JPEG de calidad 0,7 y lado máximo de 1280 px antes de subirla;
    - firma: un `canvas` táctil que se exporta a PNG;
    - ubicación: `navigator.geolocation.getCurrentPosition`, opcional (si el usuario la niega, se envía sin `lat` ni `lng`).
- Cambia las pantallas 26 y 28 a `"react"`.

## Códigos de error nuevos de la fase 5

| Código | Texto |
|---|---|
| `ENDPOINT_RETIRED` | Esta acción ya no existe. Actualizá la aplicación. |
| `ORDER_NOT_OPEN` | La orden ya no está abierta. |
| `ORDER_HAS_UNSENT_LINES` | Hay productos sin enviar a cocina. |
| `LINE_INVALID_STATE` | La línea no admite esa acción. |
| `TABLE_BUSY` | La mesa ya está ocupada. |
| `TABLE_INVALID_STATE` | La mesa no admite esa acción. |
| `SPLIT_INVALID` | Revisá cómo dividiste la cuenta. |
| `NOTHING_TO_FIRE` | No hay productos nuevos para enviar. |
| `TICKET_INVALID_STATE` | El ticket no admite esa acción. |
| `DELIVERY_INVALID` | Revisá los datos del domicilio. |
| `ROUTE_INVALID` | Revisá los pedidos de la ruta. |
| `ROUTE_INVALID_STATE` | La ruta no admite esa acción. |
| `STOP_AMOUNT_MISMATCH` | El monto cobrado debe ser exactamente el saldo. |
| `ROUTE_HAS_PENDING_STOPS` | Hay paradas sin liquidar. |
| `ROUTE_NEEDS_OPEN_SESSION` | Abrí un turno de caja para recibir el efectivo de la ruta. |

## Lista de verificación de cada entrega
- [ ] Una migración por entrega, que corre limpia.
- [ ] Todas las pruebas **[P]** de la entrega y las de fases anteriores en verde. El reporte explica cada prueba vieja que se ajustó.
- [ ] `fmt`, `clippy`, `test`, `tsc` y `lint` en verde. El README está actualizado.
- [ ] El modo demo funciona: abrir mesa, enviar a cocina, ver el ticket en `/cocina`, dividir y cobrar (5A); crear un domicilio contra entrega, armar la ruta, liquidar desde `/repartidor` y cerrar (5B).

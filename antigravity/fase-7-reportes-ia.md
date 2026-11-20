# Fase 7: Reportes, notificaciones, correo, resumen semanal y asistente IA

> **Requisito:** la fase 6 debe estar aprobada. Rama `feat/fase-7-reportes`, a partir de `feat/fase-6-tienda`.
>
> **Cambio del 2026-11-08:** F7.1, F7.2 y F7.3 se construyen **antes**, dentro de `fase-S-salida.md` (tareas S5 a S7), con los cambios que dice ese documento. Aquí solo quedan F7.4 a F7.7, pospuestas.
>
> **Migración única:** `20261111000100_fase7_reportes.sql`.
>
> **Llaves:** la de correo (Resend) y la de la API de Claude **no se buscan ni se piden en el chat**. El código las lee de variables de entorno y Claude las configura. Sin llave, cada función usa su modo simulado, que se describe abajo.
>
> Todo lo que no esté escrito aquí está prohibido.

**Fuera de alcance:**
- envío por WhatsApp desde el servidor (necesita plantillas aprobadas por Meta; se sigue usando `wa.me`);
- push web;
- SMS;
- escrituras desde el asistente IA (solo lee);
- lectura de facturas con IA;
- historial de conversaciones guardado en el servidor;
- respuesta del asistente en *streaming*: se responde con un JSON completo.

## Orden de las tareas

| Tarea | Tema |
|---|---|
| F7.1 | Vista única de "venta completada" |
| F7.2 | Panel principal (pantalla 01) |
| F7.3 | Reportes y exportación CSV |
| F7.4 | Notificaciones dentro de la app |
| F7.5 | Worker del *outbox* y correo |
| F7.6 | Tareas programadas diarias y resumen semanal |
| F7.7 | Asistente IA de solo lectura |

---

## F7.1 Vista de venta completada

```sql
create view public.v_sales with (security_invoker = true) as
select o.id, o.tenant_id, o.branch_id, o.created_at,
       (o.created_at at time zone b.timezone)::date as local_date,
       extract(hour from o.created_at at time zone b.timezone)::int as local_hour,
       extract(isodow from o.created_at at time zone b.timezone)::int as local_isodow,
       o.subtotal, o.inc_amount as tax, o.discount_total, o.tip_amount, o.delivery_fee,
       (o.subtotal + o.inc_amount) as gross_sale,
       o.refunded_amount, o.customer_id, o.sale_type_id, o.cash_session_id
  from public.orders o
  join public.branches b on b.id = o.branch_id
 where o.status_id in (public.tipo_id('ORD','COBRADA'), public.tipo_id('ORD','CERRADA'));
grant select on public.v_sales to authenticated;
```

**Definiciones únicas. Todos los reportes las usan; no hay otra fórmula en ningún lado:**

| Concepto | Fórmula |
|---|---|
| Venta bruta | `Σ gross_sale` (sin propina ni domicilio). |
| Devoluciones del periodo | `Σ refunds.total` con `status = APROBADA` y `decided_at` local dentro del periodo. |
| Venta neta | Venta bruta − devoluciones del periodo. |
| Número de ventas | `count(*)` de `v_sales`. |
| Ticket promedio | Venta bruta / número de ventas (0 si no hay ventas). |
| Costo de venta | `Σ order_items.cost_total` de las líneas no canceladas de las órdenes de `v_sales`. |
| Margen bruto | `Σ line_total` de esas líneas − costo de venta. |
| % de margen | Margen / `Σ line_total` × 100. |
| Líneas sin costo | Número de líneas con `cost_missing`. Se muestra como advertencia, nunca se esconde. |

Cada periodo se filtra con `local_date between from and to`.

Escribe estas fórmulas una sola vez, como funciones de Rust en `src/reports/metrics.rs`, con pruebas.

---

## F7.2 Panel principal (pantalla 01)

`GET /v1/reports/dashboard?branch_id=&from=&to=` (permiso `reports:read`). `branch_id` es opcional: sin él, suma todas las sucursales a las que el usuario tiene acceso.

```
{ period: {from, to}, previous: {from, to},
  kpis: { gross_sale, net_sale, sales_count, avg_ticket, gross_margin, margin_pct, refunds,
          tips, cost_missing_lines },
  kpis_previous: { …mismos campos… },
  by_hour: [{hour 0..23, gross_sale, sales_count}],
  by_day: [{date, gross_sale, sales_count}],
  top_products: [{product_id, name, qty, line_total}] (10, por line_total desc),
  payment_methods: [{method, amount, count}] (Σ payments de las órdenes de v_sales),
  low_stock: [{product_id, name, location_name, on_hand, min_qty}] (alertas BAJO abiertas, máx. 10),
  overdue_receivables: {count, amount} }
```

El periodo anterior es el de la misma duración, inmediatamente antes de `from`.

**Frontend:** el panel `/panel` deja de usar mocks en el modo real (en el modo demo los conserva).
- Tarjetas con la variación porcentual frente al periodo anterior: verde si sube y rojo si baja. En el margen, el color se invierte si el costo sube.
- Gráfica por hora y por día.
- Top 10, métodos de pago, stock bajo y cartera vencida.
- Selector de periodo: Hoy, Ayer, 7 días, Este mes, Mes anterior y Personalizado.

Para las gráficas, usa barras con `div` y CSS o SVG a mano. **No agregues librerías de gráficas.**

**Prueba [P]** (`f7_dashboard`): con datos conocidos (2 ventas, 1 devolución y 1 venta cancelada), cada KPI da el valor exacto esperado. La venta cancelada no cuenta.

---

## F7.3 Reportes y exportación

Todos con permiso `reports:read` y los parámetros `branch_id`, `from` y `to`. Con `format=csv`:
- responde `text/csv; charset=utf-8`;
- con BOM (`\u{FEFF}`) al inicio, separador `;` y decimales con coma;
- encabezados en español;
- el nombre del archivo va en `Content-Disposition` y se arma como `<reporte>_<from>_<to>.csv`.

| Endpoint | Filas | Columnas |
|---|---|---|
| `/v1/reports/sales-by-day` | Una por día con ventas | fecha, ventas, venta_bruta, devoluciones, venta_neta, ticket_promedio, margen |
| `/v1/reports/sales-by-product` | Una por producto | sku, producto, categoría, cantidad, venta_sin_impuesto, costo, margen, pct_margen |
| `/v1/reports/sales-by-category` | Una por categoría | categoría, cantidad, venta_sin_impuesto, margen |
| `/v1/reports/sales-by-cashier` | Una por cajero (el `created_by` del primer pago) | cajero, ventas, venta_bruta, ticket_promedio |
| `/v1/reports/sales-by-method` | Una por método | método, pagos, monto |
| `/v1/reports/sales-heatmap` | Día ISO 1..7 × hora 0..23 | dia, hora, ventas, venta_bruta (sin CSV) |
| `/v1/reports/inventory-valuation` | Una por producto y ubicación con `on_hand != 0` | producto, ubicación, existencias, costo_promedio, valor = `money(on_hand × avg_cost)` |
| `/v1/reports/inventory-rotation` | Una por producto (últimos 30 días hasta `to`; ignora `from`) | producto, vendidas_30d, existencias, dias_inventario = `on_hand / (vendidas_30d / 30)` (vacío si no vendió) |
| `/v1/reports/purchases-by-supplier` | Una por proveedor | proveedor, órdenes, recibido, pagado, saldo |

**Frontend:**
- Pantalla 02 `/ventas`: el reporte por día más los de cajero y método, en pestañas.
- Pantalla 03 `/ventas/productos`: por producto y por categoría.
- Pestaña "Reportes" en `/inventario`: valorización y rotación.
- Pestaña "Reportes" en `/compras`: por proveedor.
- Cada tabla tiene el botón "Exportar CSV", que descarga el mismo endpoint con `format=csv`.
- Cambia las pantallas 02 y 03 a `"react"`.

**Pruebas [P]** (`f7_reports`):
- la suma de `sales-by-product.venta_sin_impuesto` es igual a `Σ line_total` del periodo;
- el CSV empieza con el BOM y usa `;`.

---

## F7.4 Notificaciones dentro de la app

### Migración

```sql
create table public.notifications (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,
  user_id uuid not null references public.users(id) on delete cascade,
  kind text not null,
  title text not null check (length(title) <= 80),
  body text not null check (length(body) <= 300),
  link text,
  dedupe_key text,
  created_at timestamptz not null default now(),
  read_at timestamptz
);
create index notifications_user_idx on public.notifications (user_id, created_at desc);
create unique index notifications_dedupe on public.notifications (user_id, dedupe_key) where dedupe_key is not null;
create table public.notification_preferences (
  user_id uuid not null references public.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  kind text not null,
  in_app boolean not null default true,
  email boolean not null default false,
  sound boolean not null default false,
  primary key (user_id, tenant_id, kind)
);
-- RLS: notifications con "tenant_id = current_tenant_id() and user_id = auth.uid()" para select/update;
-- insert solo desde el backend (dentro de la transacción del evento). notification_preferences: user_id = auth.uid().
```

### Eventos (lista cerrada)

| `kind` | Cuándo se crea | Quién la recibe (con membresía activa en la sucursal) | Título |
|---|---|---|---|
| `STOCK_BAJO` | Se crea una alerta `BAJO` | Usuarios con `inventory:adjust` | "Stock bajo: <producto>" |
| `STOCK_NEGATIVO` | Se crea una alerta `NEGATIVO` | Usuarios con `inventory:adjust` | "Stock negativo: <producto>" |
| `PEDIDO_ONLINE` | Checkout en línea creado (fase 6) | `pos:sell` | "Pedido en línea #<short>" |
| `PAGO_ONLINE` | Webhook `APPROVED` | `pos:sell` | "Pago recibido #<short>" |
| `DEVOLUCION_PENDIENTE` | Devolución creada en `PENDIENTE` | `pos:refund` | "Devolución por aprobar" |
| `AJUSTE_PENDIENTE` | Ajuste creado en `PENDIENTE` | `inventory:adjust_approve` | "Ajuste por aprobar" |
| `GASTO_PENDIENTE` | Gasto creado | `expenses:approve` | "Gasto por aprobar" |
| `TICKET_LISTO` | Un ticket pasa a `LISTO` | El `waiter_id` de la orden (si existe) | "Listo: mesa <code>" |
| `FISCAL_FALLO` | Un documento pasa a `RECHAZADO` o `CONTINGENCIA` | `fiscal:manage` | "Factura con problema" |
| `CXC_VENCIDA` | Tarea diaria (F7.6) | `credit:collect` | "Cartera vencida hoy" |
| `CXP_POR_VENCER` | Tarea diaria (F7.6) | `purchases:pay` | "Pagos a proveedores por vencer" |
| `LOTE_POR_VENCER` | Tarea diaria (F7.6) | `inventory:adjust` | "Lotes por vencer" |
| `RESOLUCION_POR_AGOTARSE` | Tarea diaria (F7.6) | `fiscal:manage` | "Resolución de facturación por agotarse" |
| `RESUMEN_SEMANAL` | Tarea semanal (F7.6) | Usuarios `DUENO` | "Tu resumen de la semana" |

**Función única** `notifications::notify_tx(tx, tenant, branch, recipients: Recipients, kind, title, body, link, dedupe_key)`, donde `Recipients` es `Permission(&str)`, `User(Uuid)` o `Role(&str)`:
- calcula los destinatarios con los permisos efectivos de la fase 0;
- respeta `in_app = false` en las preferencias;
- inserta con `on conflict do nothing` usando `dedupe_key`;
- si el usuario tiene `email = true` para ese `kind`, encola también un correo en el *outbox* (F7.5).

Llámala dentro de la **misma transacción** del evento, en cada uno de los lugares de la tabla. `dedupe_key`: para las alertas, `"<kind>:<location>:<product>:<fecha>"`; para las tareas diarias, `"<kind>:<branch>:<fecha>"`; para las demás, `"<kind>:<id de la entidad>"`.

### Endpoints
- `GET /v1/notifications?unread_only=&limit=30` devuelve `{items, unread_count}`.
- `POST /v1/notifications/{id}/read` y `POST /v1/notifications/read-all`.
- `GET` y `PUT /v1/notifications/preferences`, con `[{kind, in_app, email, sound}]` para todos los `kind`. Valores por defecto: `in_app` verdadero en todos; `email` verdadero solo en `RESUMEN_SEMANAL`; `sound` verdadero en `PEDIDO_ONLINE` y `TICKET_LISTO`.

### Frontend
- La campana de la barra superior muestra `unread_count`. **Quita el 3 fijo del mock.** Consulta cada 30 segundos.
- Al abrirla: lista con título, texto, hace cuánto (por ejemplo, "hace 5 min") y enlace; botón "Marcar todo como leído".
- Si llega una notificación nueva con `sound`, suena el pitido de la fase 5.
- Pantalla `/configuracion/notificaciones` (personal y visible para todos, registro `s("49", "notificaciones", "Notificaciones", "/configuracion/notificaciones", "configuracion", "admin", "react")`): una tabla de `kind` con tres interruptores.

**Pruebas [P]** (`f7_notifications`):
- una alerta `BAJO` notifica solo a usuarios con `inventory:adjust`, y no duplica la notificación en el mismo día;
- un usuario con `in_app = false` no la recibe;
- un usuario no puede leer las notificaciones de otro.

---

## F7.5 Worker del *outbox* y correo

### Migración

```sql
alter table public.outbox
  add column branch_id uuid,
  add column dedupe_key text,
  add column sent_at timestamptz;
create unique index outbox_dedupe on public.outbox (dedupe_key) where dedupe_key is not null;
```

**Tipos (`kind`) que procesa esta fase:** solo `EMAIL`. Payload: `{to, subject, html, text}`. Cualquier otro `kind` se ignora: se queda en `PENDIENTE`.

### Worker (`src/outbox_worker.rs`)
Tarea de `tokio`, activa con `NEXA_OUTBOX_WORKER=on` (apagada por defecto). Usa el pool del worker de la fase 4 y se ejecuta cada 30 segundos:
1. Toma hasta 20 filas `PENDIENTE` con `next_attempt_at <= now()`, con `for update skip locked`, y las marca `ENVIANDO`.
2. **Con `RESEND_API_KEY` y `NEXA_EMAIL_FROM` configuradas:** `POST https://api.resend.com/emails`, con el encabezado `Authorization: Bearer <key>` y el cuerpo `{from, to: [to], subject, html, text}`, usando `reqwest`, que ya está en las dependencias, con un tiempo máximo de 15 segundos.
   - Respuesta 2xx: `ENVIADO`, con `sent_at`.
   - Si no, o si hay error de red: `attempts += 1`, `last_error` y vuelve a `PENDIENTE`, con `next_attempt_at = now() + [1, 5, 15, 60, 240][attempts − 1]` minutos. Al llegar a `attempts = 5`: `FALLIDO`.
3. **Sin llaves:** marca `SIMULADO`, con `sent_at`, y escribe en el log `tracing::info!` el destinatario y el asunto, **nunca** el contenido.
4. Al arrancar, recupera las filas `ENVIANDO` de hace más de 10 minutos y las devuelve a `PENDIENTE`.

`outbox::enqueue_tx` (que creó la fase 0) gana los parámetros `branch_id` y `dedupe_key`.

**Pruebas [P]** (`f7_outbox`), llamando las funciones directamente:
- sin llave, la fila queda `SIMULADO`;
- con un cliente HTTP simulado (crea el trait `EmailSender` con una implementación falsa para las pruebas) que falla 5 veces, la fila queda `FALLIDO` con `attempts = 5`;
- dos *workers* concurrentes no toman la misma fila.

---

## F7.6 Tareas programadas y resumen semanal

### Migración

```sql
create table public.weekly_reports (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  week_start date not null,            -- lunes
  data jsonb not null,
  created_at timestamptz not null default now(),
  primary key (branch_id, week_start),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create table public.job_runs (
  job text not null,
  branch_id uuid not null,
  day date not null,
  ran_at timestamptz not null default now(),
  primary key (job, branch_id, day)
);
-- RLS: weekly_reports con in_scope; job_runs sin políticas (solo el pool del worker).
```

### Programador
Tarea de `tokio`, activa con `NEXA_SCHEDULER=on` (apagada por defecto), que se ejecuta cada 10 minutos con el pool del worker. Para cada sucursal calcula la fecha y la hora locales. Cada tarea se ejecuta **una vez por sucursal y día**, garantizado por `job_runs` (`insert … on conflict do nothing returning`: si no devuelve fila, ya se ejecutó). Cada sucursal y tarea va en su propia transacción, con el contexto de su negocio.

| Tarea (`job`) | Desde la hora local | Qué hace |
|---|---|---|
| `DIARIO_CXC` | 07:00 | Si hay cuentas por cobrar que vencieron **ayer** (su `days_overdue` pasó a 1 hoy), crea `CXC_VENCIDA`: "N cuentas por $X vencieron ayer". |
| `DIARIO_CXP` | 07:00 | Cuentas por pagar con `due_date` entre hoy y hoy + 3 días y saldo mayor que 0 → `CXP_POR_VENCER`. |
| `DIARIO_LOTES` | 07:00 | Lotes con `qty > 0` que vencen en exactamente 30, 15 o 7 días → `LOTE_POR_VENCER`. |
| `DIARIO_RESOLUCION` | 07:00 | Resoluciones con `warning` → `RESOLUCION_POR_AGOTARSE`. |
| `SEMANAL_RESUMEN` | Lunes 06:00 | Ver abajo. |

**Resumen semanal**, para la semana de lunes a domingo anterior:
1. Si `weekly_reports` ya tiene esa semana, no hace nada.
2. Si la sucursal tiene **menos de 7 días distintos** con ventas en toda su historia (`count(distinct local_date)` en `v_sales`), no hace nada. Así no se manda un resumen vacío.
3. Calcula `data`:
   - los KPIs de F7.2 para la semana y la semana anterior;
   - el top 5 de productos;
   - el mejor día;
   - la mejor hora;
   - la cartera vencida;
   - el stock bajo (número de alertas);
   - las cuentas por pagar que vencen esta semana.
4. Guarda la fila, crea la notificación `RESUMEN_SEMANAL` para los dueños y, si su preferencia de correo está activa, encola un `EMAIL`.
   - **Asunto:** "Tu semana en <sucursal>: $<venta neta> (<+/-X %>)".
   - **HTML:** sencillo, con estilos en línea, sin imágenes externas, armado en Rust con `format!` o con una plantilla en `src/templates/weekly.html` leída con `include_str!`. No agregues motores de plantillas.

**Pantalla:** `/panel` gana la pestaña "Resúmenes semanales", con la lista de semanas y su detalle (`GET /v1/reports/weekly?branch_id=`, permiso `reports:read`).

**Pruebas [P]** (`f7_scheduler`), llamando a la función de cada tarea con una hora simulada:
- el resumen no se crea con menos de 7 días de ventas;
- con 7 días o más se crea una vez, y una segunda ejecución no lo duplica;
- `DIARIO_CXP` no se ejecuta dos veces el mismo día.

---

## F7.7 Asistente IA (solo lectura)

### Migración

```sql
create table public.ai_usage (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  month text not null check (month ~ '^[0-9]{4}-[0-9]{2}$'),
  input_tokens bigint not null default 0,
  output_tokens bigint not null default 0,
  messages integer not null default 0,
  primary key (tenant_id, month)
);
alter table public.tenants add column ai_monthly_token_limit bigint not null default 2000000;
insert into public.permissions (code, module, description) values
  ('assistant:use','assistant','Usar el asistente IA') on conflict (code) do nothing;
-- role_permissions: DUENO, ADMIN_CAJA y SUPERVISOR. RLS de ai_usage a nivel de negocio.
```

### `POST /v1/assistant/chat` (permiso `assistant:use`)
Cuerpo: `{branch_id, messages: [{role: "user"|"assistant", content: string}]}`.
- Máximo 20 mensajes; cada uno de 2.000 caracteres como máximo. Si no: 422 `ASSISTANT_INVALID`.
- El último mensaje debe ser de `user`.

Respuesta: `{reply: string, tools_used: [{name, ok: bool}], usage: {input_tokens, output_tokens}}`.

1. **Cupo:** si `input_tokens + output_tokens` del mes local es mayor o igual a `ai_monthly_token_limit`: 429 `AI_QUOTA_EXCEEDED`.
2. **Sin `ANTHROPIC_API_KEY`:** 503 `ASSISTANT_NOT_CONFIGURED`. En la pantalla, el panel del asistente no se muestra si `GET /v1/assistant/status` responde `{configured: false}`.
3. **Llamada a la API de Claude** (Messages API) con `reqwest`:
   - `POST https://api.anthropic.com/v1/messages`;
   - encabezados `x-api-key: <key>`, `anthropic-version: 2023-06-01` y `content-type: application/json`;
   - cuerpo `{model: "claude-sonnet-5-5", max_tokens: 1024, system: <prompt de sistema>, tools: [...], messages: [...]}`, **sin** `stream`;
   - tiempo máximo de 60 segundos.
4. **Bucle de herramientas:**
   - si `stop_reason = "tool_use"`, por cada bloque `tool_use` de la respuesta ejecuta la herramienta;
   - agrega a `messages` el mensaje `assistant` con el `content` recibido tal cual, y un mensaje `user` con los bloques `{type: "tool_result", tool_use_id, content: <JSON en texto>}`;
   - vuelve a llamar;
   - **máximo 5 vueltas**; si se pasa, responde con lo que haya y la frase "No pude terminar la consulta.".
5. `reply` es la concatenación de los bloques `text` de la última respuesta.
6. Suma `usage.input_tokens` y `usage.output_tokens` de **todas** las vueltas en `ai_usage` (`insert … on conflict do update`) y `messages += 1`.

### Herramientas (lista cerrada)
Cada una llama a las funciones `_tx` existentes con el `scope` **del usuario** y exige el mismo permiso que su endpoint. Sin permiso, la herramienta devuelve `{"error": "sin permiso para ver esto"}` y **no falla** la petición.

| Nombre | Entrada (JSON Schema) | Llama a | Permiso |
|---|---|---|---|
| `ventas_resumen` | `{from: date, to: date}` | Los KPIs de F7.2 | `reports:read` |
| `productos_mas_vendidos` | `{from, to, limit: 1..20}` | `sales-by-product`, ordenado | `reports:read` |
| `stock_bajo` | `{}` | Las alertas `BAJO` y `NEGATIVO` abiertas | `inventory:read` |
| `buscar_producto` | `{q: string}` | `GET /v1/products` (10 resultados, con precio y existencias) | `inventory:read` |
| `cartera_resumen` | `{}` | `/v1/receivables/summary` | `customers:read` |
| `deudores` | `{limit: 1..20}` | `top_debtors` | `customers:read` |
| `caja_turno_actual` | `{}` | El resumen del turno abierto del usuario | `cash:history` |
| `cuentas_por_pagar` | `{}` | Aging de cuentas por pagar | `purchases:pay` |

### Prompt de sistema (texto exacto)

```
Sos el asistente de Nexa para <nombre del negocio>, sucursal <sucursal>. Hoy es <fecha local, dd/mm/yyyy>.
Respondé en español de Colombia con voseo, corto y claro, como le hablarías al dueño de una tienda de barrio.
Usá las herramientas para cualquier número: nunca inventes cifras. Si una herramienta dice que no hay permiso, decí que esa información no está disponible para su usuario.
Los precios son en pesos colombianos; escribilos como $12.500. No uses jerga contable.
Solo podés consultar información; si te piden cambiar algo, explicá cómo hacerlo en la aplicación.
```

### Código para pruebas
Crea el trait `LlmClient` con dos implementaciones:
- `AnthropicClient` (real);
- `ScriptedLlm` (para las pruebas), que devuelve respuestas predefinidas en orden.

**Las pruebas nunca llaman a la red.**

### Frontend
- Botón flotante "Asistente" en el layout de administración, visible solo con `can("assistant:use")` y `configured`.
- Abre un panel lateral con el chat: mensajes, campo de texto e indicador "Pensando…".
- El historial vive en el estado del componente. Se pierde al recargar y se envían los últimos 20 mensajes.
- Debajo de cada respuesta, en gris, las herramientas usadas (por ejemplo, "Consultó: ventas_resumen").
- Ante 429, muestra: "Se acabó el cupo del asistente este mes."

### Pruebas (`f7_assistant`, con `ScriptedLlm`)
- **[P]** Un cajero sin `reports:read`, cuando el modelo pide `ventas_resumen`, recibe `{"error": "sin permiso…"}` en el `tool_result`, y la respuesta es 200.
- **[P]** Con el cupo agotado: 429 `AI_QUOTA_EXCEEDED`, sin llamar al modelo.
- **[P]** Se suman los tokens de todas las vueltas en `ai_usage`.
- **[P]** Después de 5 vueltas de herramientas, el bucle se corta.

---

## Códigos de error nuevos de la fase 7

| Código | Texto |
|---|---|
| `ASSISTANT_INVALID` | El mensaje es demasiado largo. |
| `ASSISTANT_NOT_CONFIGURED` | El asistente todavía no está configurado. |
| `AI_QUOTA_EXCEEDED` | Se acabó el cupo del asistente este mes. |

## Lista de verificación
- [ ] Una migración que corre limpia.
- [ ] Todas las pruebas **[P]** y las de fases anteriores en verde.
- [ ] Ninguna llave ni contenido de correo en los logs.
- [ ] Los tres workers (`NEXA_OUTBOX_WORKER`, `NEXA_SCHEDULER` y los de fases anteriores) están apagados por defecto y documentados en el README.
- [ ] El modo demo funciona: panel, reportes con exportación, campana y asistente simulado. En el modo demo, el asistente responde con un texto fijo que dice que es una demostración.

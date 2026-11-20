# Fase 4: Facturación electrónica (por proveedor tecnológico) e impuestos

> **Bloqueada:** espera la decisión D5 (proveedor de facturación). Claude la convertirá en módulos cuando esté decidida.
>
> **Requisito:** la fase S debe estar aprobada y el proveedor de facturación elegido (D5 en `REVISION-CLAUDE.md`). **No empieces esta fase hasta que Claude actualice este documento con el proveedor.** Rama `feat/fase-4-facturacion`, a partir de `feat/fase-s-salida`.
>
> **Migración única:** el nombre se fija cuando Claude actualice el documento. Debe ser posterior a `20261109000100_salida.sql`.
>
> **Decisión tomada (no la cambies):** Nexa **no** se conecta directo a la DIAN. Usa un proveedor tecnológico autorizado. En esta fase se construye la interfaz del proveedor con una implementación **simulada** (`MockProvider`). **No elijas ni programes un proveedor real**: Claude te dirá cuál, y sus llaves no pasan por ti.
>
> Todo lo que no esté escrito aquí está prohibido.

**Fuera de alcance:**
- XML UBL y firma digital: los hace el proveedor;
- set de pruebas de habilitación;
- documentos recibidos y eventos RADIAN;
- documento soporte;
- notas débito;
- envío del documento por correo o WhatsApp, que se hace desde el proveedor en una fase posterior. Aquí solo existe el botón "Reintentar".

## Orden de las tareas

| Tarea | Tema |
|---|---|
| F4.1 | Configuración fiscal del negocio y UVT |
| F4.2 | Resoluciones de numeración |
| F4.3 | Tipo de documento por venta y regla de 5 UVT |
| F4.4 | Interfaz del proveedor y `MockProvider` |
| F4.5 | Worker de transmisión |
| F4.6 | Notas crédito |
| F4.7 | Pantalla de documentos y tirilla con CUFE y QR |
| F4.8 | Reglas de impuestos por régimen |

---

## F4.1 Configuración fiscal y UVT

### Migración

```sql
alter table public.tenants
  add column fiscal_status text not null default 'INACTIVO' check (fiscal_status in ('INACTIVO','CONFIGURANDO','ACTIVO','BLOQUEADO')),
  add column fiscal_provider text check (fiscal_provider in ('MOCK')),
  add column fiscal_environment text not null default 'PRUEBAS' check (fiscal_environment in ('PRUEBAS','PRODUCCION')),
  add column fiscal_email text,
  add column person_type text not null default 'NATURAL' check (person_type in ('NATURAL','JURIDICA')),
  add column city_code text check (city_code ~ '^[0-9]{5}$'),
  add column address text;
create table public.uvt_values (
  year integer primary key check (year between 2020 and 2100),
  value numeric(14,2) not null check (value > 0)
);
alter table public.uvt_values enable row level security;
create policy uvt_values_read on public.uvt_values for select using (true);
grant select on public.uvt_values to authenticated;
insert into public.permissions (code, module, description) values
  ('fiscal:manage','fiscal','Configurar facturación electrónica y reintentar documentos')
on conflict (code) do nothing;
-- role_permissions: solo DUENO (que ya tiene todo) y ADMIN_CAJA.
```

**No siembres ningún valor de UVT.** Lo carga el dueño desde la pantalla; el valor oficial cambia cada año.

### Endpoints (`src/fiscal.rs`)

| Método y ruta | Permiso | Detalle |
|---|---|---|
| `GET /v1/fiscal/config` | `fiscal:manage` | `{fiscal_status, fiscal_provider, fiscal_environment, fiscal_email, person_type, city_code, address, nit, dv, business_name, iva_responsible, uvt: [{year, value}]}` |
| `PUT /v1/fiscal/config` | `fiscal:manage` | Mismos campos, salvo `uvt`. |
| `PUT /v1/fiscal/uvt` | `fiscal:manage` | `{year, value}`. Escribe con una función `security definer` `public.set_uvt(year, value)`, que verifica `fiscal:manage` dentro del backend antes de llamarla. La tabla es global y solo se lee. |

**Para pasar `fiscal_status` a `ACTIVO`**, el negocio necesita, en este orden:
1. `nit` y `dv` válidos;
2. `business_name`;
3. `city_code`;
4. `address`;
5. `fiscal_email`;
6. `iva_responsible` distinto de `null`;
7. `fiscal_provider = MOCK`;
8. al menos una resolución vigente de tipo `POS` en alguna sucursal (F4.2).

Si falta algo: 422 `FISCAL_NOT_READY`, con `data: {missing: [...]}` en ese orden.

**Interruptor:** con `fiscal_status` distinto de `ACTIVO`:
- las ventas **siguen** creando su `fiscal_documents` en `PENDIENTE` (como hoy);
- el worker **no los transmite**;
- la regla de 5 UVT **no se aplica**.

---

## F4.2 Resoluciones de numeración

### Migración

```sql
alter table public.document_sequences drop constraint document_sequences_doc_type_check;
alter table public.document_sequences add constraint document_sequences_doc_type_check
  check (doc_type in ('POS','FACTURA','NOTA_CREDITO','VENTA','DEVOLUCION','COMPRA','SEPARADO','COTIZACION','ACUERDO_PAGO','TRASLADO','GASTO','RUTA'));
alter table public.document_sequences
  add column range_from bigint check (range_from > 0),
  add column valid_from date,
  add column technical_key text,
  add column is_fiscal boolean not null default false;
```

Una fila **fiscal** (`doc_type` `POS`, `FACTURA` o `NOTA_CREDITO` con `is_fiscal = true`) exige `resolution_number`, `resolution_date`, `range_from`, `range_to`, `valid_from`, `valid_until` y, salvo en `NOTA_CREDITO`, `technical_key`.

### Endpoints (permiso `fiscal:manage`)
- `GET /v1/fiscal/resolutions?branch_id=` devuelve las filas fiscales con `remaining = range_to − next_number + 1`, `remaining_pct`, `days_left = valid_until − hoy` y `warning`. `warning` es `"RANGO"` si `remaining_pct < 10`, `"VIGENCIA"` si `days_left < 30` y `"AMBOS"` si se cumplen las dos.
- `POST /v1/fiscal/resolutions`, cuerpo `{branch_id, doc_type, prefix (1..4 letras mayúsculas), resolution_number, resolution_date, range_from, range_to, valid_from, valid_until, technical_key?}`.
  - Si ya existe una fila de ese `doc_type` en la sucursal, **se reemplaza**: `next_number = range_from` y los demás campos.
  - El reemplazo falla si el rango nuevo se solapa con números ya emitidos con el mismo prefijo, es decir, si `range_from <= max(number)` en `fiscal_documents` de esa sucursal, tipo y prefijo: 409 `RESOLUTION_OVERLAP`.
  - `range_from > range_to` o `valid_from > valid_until`: 422 `RESOLUTION_INVALID`.
  - Auditoría `RESOLUCION_CARGADA`.

`sequences::next_tx` ya respeta `range_to`. Agrega también:
- si `is_fiscal` y hoy es menor que `valid_from`: 409 `RESOLUTION_NOT_STARTED`;
- si hoy es mayor que `valid_until`: 409 `RESOLUTION_EXPIRED`.

---

## F4.3 Tipo de documento por venta y regla de 5 UVT

### Qué documento genera cada venta (en `checkout_tx`, en el paso de documento fiscal)

| Caso | `doc_type` |
|---|---|
| La venta tiene `customer_id` con `doc_type` y `doc_number` | `FACTURA` |
| Cualquier otro caso | `POS` |

- El **número del comprobante** de la venta (`orders.receipt_prefix` y `receipt_number`) sale de `sequences::next_tx(tx, scope, branch, doc_type)`, con el tipo elegido. Es el mismo número del documento fiscal.
- Si no existe la secuencia `FACTURA` en la sucursal, se crea con el prefijo `FE` y sin resolución, como ya pasa con `POS`.

### Regla de 5 UVT
Función pura `fiscal::requires_buyer(total, uvt: Option<Decimal>, fiscal_active: bool, buyer_identified: bool) -> bool`:
- devuelve `false` si `!fiscal_active`, si `uvt` es `None` o si `buyer_identified`;
- si no, devuelve `total > 5 × uvt`.

`uvt` es el valor de `uvt_values` para el año local de la sucursal.

En `checkout_tx`, después de calcular el total y antes de los pagos: si `requires_buyer` es verdadero, responde 422 `FISCAL_BUYER_REQUIRED`, con `data: {limit: 5 × uvt}`.

La misma función se usará en la fase 6 (tienda en línea). Ponla en `fiscal.rs`, no en `sales.rs`.

### Pruebas
- Unitaria `requires_buyer_cases`: los tres casos que la dejan pasar y el caso que bloquea.
- **[P]** (`f4_uvt_rule`) Con el fiscal `ACTIVO` y UVT 2026 = 50.000, una venta anónima de $250.001: 422 `FISCAL_BUYER_REQUIRED`. Con un cliente identificado pasa y genera `doc_type = FACTURA`. Con el fiscal `INACTIVO` pasa anónima.

---

## F4.4 Interfaz del proveedor y `MockProvider`

### Código (`src/fiscal/provider.rs`; convierte `fiscal.rs` en el módulo `fiscal/mod.rs`)

```rust
pub enum EmitResult {
    Accepted { cude: String, qr_url: String, provider_id: String, raw: serde_json::Value },
    Rejected { reason: String, raw: serde_json::Value },
    Pending  { provider_id: String, raw: serde_json::Value },
}
pub enum ProviderError { Timeout, Unavailable(String), Invalid(String) }

#[async_trait::async_trait]   // si async_trait no está en Cargo.toml, usa una función que devuelve Pin<Box<dyn Future>>; no agregues el crate sin justificar
pub trait FiscalProvider: Send + Sync {
    async fn emit(&self, doc: &FiscalPayload) -> Result<EmitResult, ProviderError>;
    async fn status(&self, provider_id: &str) -> Result<EmitResult, ProviderError>;
}
```

**`FiscalPayload`** se arma desde la base y contiene:
- `doc_type`, `prefix`, `number` y la fecha y hora local;
- el emisor: tipo de persona, NIT, DV, razón social, dirección, municipio, correo, ambiente y clave técnica;
- el adquirente: tipo de persona, documento, DV, nombre, correo, dirección y municipio, o "222222222222 / Consumidor final" en el caso `POS` anónimo;
- las líneas: nombre, cantidad, precio unitario, descuento, base, tasa y código de impuesto, e impuesto;
- los totales: base, cada impuesto por tasa, descuentos y total sin propina. **La propina no entra al documento fiscal.**
- los pagos: método y monto;
- en `NOTA_CREDITO`: referencia al documento original (prefijo, número, CUDE y fecha) y motivo.

**`MockProvider`:**
- configurable con `MockMode::{Accept, Reject, Error}`;
- `Accept`: devuelve `cude = "MOCK-" + uuid sin guiones`, `qr_url = "https://catalogo-vpfe-hab.dian.gov.co/document/searchqr?documentkey=" + cude` y `provider_id = uuid`;
- `Reject`: devuelve `Rejected { reason: "Rechazo simulado" }`;
- `Error`: devuelve `Err(Unavailable)`.

`fiscal::provider_for(tenant) -> Box<dyn FiscalProvider>` devuelve `MockProvider(Accept)` si `fiscal_provider = MOCK`. El modo se puede cambiar con la variable de entorno `NEXA_FISCAL_MOCK_MODE` (`accept`, `reject` o `error`; por defecto `accept`).

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Contingencia', 'CONTINGENCIA', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'DOCF'
on conflict (padre_id, tima_codigo) do nothing;
alter table public.fiscal_documents
  add column provider_id text,
  add column qr_url text,
  add column sent_at timestamptz,
  add column accepted_at timestamptz,
  add column payload_hash text;
```

---

## F4.5 Worker de transmisión

### Conexión
`db::connect` fuerza `set role authenticated`, así que el worker no ve documentos de otros negocios. Crea `db::connect_worker(url)`: un pool aparte, de **2 conexiones**, **sin** `after_connect`. Este pool solo lo usa el worker.

**Comprueba en el contenedor de pruebas** que ese pool lee filas de dos negocios distintos. Si no puede, **detente** y pregunta en el reporte. No desactives el RLS ni crees roles sin preguntar.

### Ciclo (`src/fiscal/worker.rs`)
- Corre como tarea de `tokio` iniciada en `main.rs`, solo si `NEXA_FISCAL_WORKER=on` (por defecto apagado).
- Repite cada 30 segundos: `recover_stuck`, `claim_batch(20)` y el procesamiento.

1. **`recover_stuck`:** `update fiscal_documents set status = PENDIENTE where status = ENVIADO and sent_at < now() − interval '15 minutes'`.
2. **`claim_batch`:** en una transacción, selecciona los documentos `PENDIENTE` con `next_attempt_at <= now()` cuyo negocio tiene `fiscal_status = 'ACTIVO'`. **Excluye** las `NOTA_CREDITO` cuyo documento original todavía no está `ACEPTADO`. Usa `order by created_at limit 20 for update skip locked`, márcalos `ENVIADO` con `sent_at = now()` y haz commit.
3. **Procesamiento**, de a uno:
   1. Arma el `FiscalPayload` y guarda `payload_hash`, el SHA-256 de su JSON.
   2. Llama a `emit` con un tiempo máximo de 20 segundos.
   3. Según el resultado:

   | Resultado | Efecto |
   |---|---|
   | `Accepted` | `ACEPTADO`, con `cude`, `qr_url`, `provider_id`, `accepted_at = now()` y `provider_response = raw`. |
   | `Rejected` | `RECHAZADO`, con `last_error = reason` y `provider_response`. **No se reintenta solo.** |
   | `Pending` | Se queda `ENVIADO`, con `provider_id` y `next_attempt_at = now() + 2 min`. El siguiente ciclo llama a `status` en lugar de `emit` para los `ENVIADO` con `provider_id` y `next_attempt_at` vencido. |
   | `Err(_)` | `attempts += 1` y `last_error`. Si `attempts < 5`: vuelve a `PENDIENTE` con `next_attempt_at = now() + 2 min`. Si `attempts >= 5`: `CONTINGENCIA`. |

**Contingencia:** un documento en `CONTINGENCIA` se reintenta cada 30 minutos (mismo flujo, `next_attempt_at = now() + 30 min`) hasta 48 horas después de su `created_at`. Después queda en `CONTINGENCIA` sin más reintentos y aparece con alerta en la pantalla.

**El POS nunca espera al worker:** la venta responde con `fiscal_status = PENDIENTE`.

### Reintento manual
`POST /v1/fiscal/documents/{id}/retry` (permiso `fiscal:manage`). Solo desde `RECHAZADO` o `CONTINGENCIA`; si no: 409 `FISCAL_NOT_RETRYABLE`. Deja `status = PENDIENTE`, `attempts = 0` y `next_attempt_at = now()`. Auditoría `REINTENTO_FISCAL`.

### Pruebas (`f4_worker`)
Llama a las funciones del worker directamente, sin esperar el temporizador:
- **[P]** `Accept`: el documento queda `ACEPTADO` con un CUDE que empieza por `MOCK-`.
- **[P]** `Error` cinco veces: el documento queda `CONTINGENCIA` con `attempts = 5`.
- **[P]** Un documento `ENVIADO` con `sent_at` de hace 16 minutos vuelve a `PENDIENTE` con `recover_stuck`.
- **[P]** Dos llamadas concurrentes a `claim_batch` no toman el mismo documento.
- **[P]** Con el negocio en `INACTIVO`, `claim_batch` no toma nada.
- **[P]** Una nota crédito no se toma mientras su original no esté `ACEPTADO`.

---

## F4.6 Notas crédito

La fase 1 ya crea un `fiscal_documents` de tipo `NOTA_CREDITO` al aprobar una devolución. En esta fase:
- su numeración sale de `sequences` `NOTA_CREDITO`, no de `DEVOLUCION`. Cambia `apply_refund_effects_tx` para que tome `next_tx(..., "NOTA_CREDITO")` y guarde ese prefijo y número en el documento fiscal. El número interno de la devolución sigue saliendo de `DEVOLUCION`;
- el payload de la nota referencia el documento original (prefijo, número y CUDE) y sus líneas son las de `refund_items`;
- la nota **solo** se crea si el documento original no está `ANULADO`.

**Prueba [P]** (`f4_credit_note`): una devolución sobre una venta con su documento `ACEPTADO` crea una nota con número de `NOTA_CREDITO`. El worker la acepta con el mock, y el payload incluye el CUDE del original.

---

## F4.7 Pantallas

1. **Pantalla 20, `/configuracion/facturacion`:** página `app/(admin)/configuracion/facturacion/page.tsx` y componente `components/configuracion/fiscal-screen.tsx`. Cambia la pantalla 20 a `"react"`. Permiso `fiscal:manage`. Sigue `design/specs/20-pagos-impuestos-y-facturacion.md` en lo visual, **solo** con lo que está aquí. Bloques:
   - **Datos del emisor:** los campos de `PUT /v1/fiscal/config`. El NIT y el DV se muestran sin editar si ya existen.
   - **Proveedor:** selector con una sola opción, "Simulado (pruebas)", y el ambiente.
   - **UVT:** tabla de año y valor, y el botón "Agregar año".
   - **Resoluciones por sucursal:** tabla con Tipo, Prefijo, Rango, Usados, Restantes, Vigencia y Aviso (amarillo si hay `warning`), y el botón "Cargar resolución".
   - **Estado:** la insignia de `fiscal_status` y el botón "Activar facturación". Ante `FISCAL_NOT_READY`, muestra la lista de lo que falta.
2. **Pantalla `/facturacion`** (registro `s("47", "documentos-fiscales", "Documentos electrónicos", "/facturacion", "ventas", "admin", "react")`, permiso `fiscal:manage`):
   - endpoint `GET /v1/fiscal/documents?branch_id=&status=&doc_type=&from=&to=&page=` → `{items: [{id, doc_type, number, order_receipt, customer_label, total, status, attempts, last_error, created_at, accepted_at, cude}], total_count, counts: {PENDIENTE, ENVIADO, ACEPTADO, RECHAZADO, CONTINGENCIA, ANULADO}}`;
   - tarjetas por estado y tabla con filtros;
   - en las filas `RECHAZADO` y `CONTINGENCIA`, el botón "Reintentar". En `CONTINGENCIA` con más de 48 horas, la insignia roja "Fuera de plazo".
3. **Tirilla:** el bloque de la fase 1 se activa con `fiscal_status = ACEPTADO`. Muestra "CUFE/CUDE: <cude>" (partido en líneas de 32 caracteres) y un QR de `qr_url`. Se permite agregar el paquete npm `qrcode`; justifícalo. Con un documento todavía `PENDIENTE`, la tirilla dice "Documento electrónico en proceso". En una venta con `doc_type = FACTURA`, el título es "Factura electrónica de venta" en lugar de "Comprobante POS".
4. **POS:** ante `FISCAL_BUYER_REQUIRED`, abre el diálogo de cliente en la pestaña "Nuevo", con el mensaje "Las ventas mayores a $X necesitan los datos del comprador."

---

## F4.8 Reglas de impuestos por régimen

- **IVA e INC son excluyentes:** ya lo garantiza el modelo, porque cada producto tiene un solo `tax_type_id`. Agrega la prueba **[P]** `f4_tax_exclusive`, que confirma que no hay forma de asignar dos impuestos (por ejemplo, que `ProductInput.tax_code` es un único valor).
- **Negocio no responsable de IVA** (`tenants.iva_responsible = false`):
  - crear o editar un producto con `IVA19` o `IVA5`: 422 `TAX_NOT_ALLOWED_REGIME`. `INC8` y `EXENTO` sí se permiten;
  - en la venta, si por datos viejos un producto tiene IVA, se cobra con tasa 0. `sellable_price` devuelve 0 cuando el impuesto es IVA y el negocio no es responsable;
  - al pasar `iva_responsible` a `false` en `PUT /v1/fiscal/config`: si existen productos activos con IVA, responde 409 `TAX_PRODUCTS_WITH_IVA`, con `data: {count}`. El dueño debe cambiarlos primero.
- **Pruebas [P]** (`f4_tax_regime`): un producto con IVA en un negocio no responsable da 422, y la venta de un producto viejo con IVA cobra 0 de impuesto.

---

## Códigos de error nuevos de la fase 4

| Código | Texto |
|---|---|
| `FISCAL_NOT_READY` | Faltan datos para activar la facturación. |
| `RESOLUTION_INVALID` | Revisá el rango y las fechas de la resolución. |
| `RESOLUTION_OVERLAP` | El rango se cruza con números ya usados. |
| `RESOLUTION_NOT_STARTED` | La resolución todavía no está vigente. |
| `RESOLUTION_EXPIRED` | La resolución de numeración venció. |
| `FISCAL_BUYER_REQUIRED` | Esta venta necesita los datos del comprador. |
| `FISCAL_NOT_RETRYABLE` | Ese documento no se puede reintentar. |
| `TAX_NOT_ALLOWED_REGIME` | Un negocio no responsable de IVA no puede cobrar IVA. |
| `TAX_PRODUCTS_WITH_IVA` | Hay productos con IVA: cambialos antes. |

## Lista de verificación
- [ ] Una sola migración que corre limpia.
- [ ] Todas las pruebas **[P]** (con su nombre) y las de fases anteriores en verde.
- [ ] El worker está apagado por defecto y se probó llamando sus funciones directamente.
- [ ] Ningún proveedor real, URL de producción ni llave en el código.
- [ ] `fmt`, `clippy`, `test`, `tsc` y `lint` en verde. El README está actualizado.

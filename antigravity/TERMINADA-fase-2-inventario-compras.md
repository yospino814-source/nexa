# Fase 2: Inventario v2, compras, proveedores, cuentas por pagar y gastos

> **TERMINADA.** Entregas 2A y 2B terminadas el 2026-11-02 y unidas a `main` por el PR #1. No se construye de nuevo. Su único defecto pendiente (la importación CSV escribe el stock sin `apply_delta_tx`) se corrige en `antigravity/modulos/M08-fase2-importacion-csv.md`, y los permisos de compras se corrigen en M13.

> **Requisito:** la fase 1 debe estar aprobada.
>
> Esta fase tiene **dos entregas, cada una con su alto obligatorio**:
> - **2A**: catálogo, bodegas, flujo único de stock y costeo. Rama `feat/fase-2a-inventario`, a partir de `feat/fase-1-pos-caja`. Al terminar, escribe `antigravity/TERMINADA-REPORTE-FASE-2A.md` y **detente**.
> - **2B**: proveedores, compras, cuentas por pagar, gastos, lotes y ajustes con aprobación. Rama `feat/fase-2b-compras`, a partir de 2A **ya aprobada**. Al terminar, escribe `TERMINADA-REPORTE-FASE-2B.md`.
>
> Todo lo que no esté escrito aquí está prohibido. Si algo no se puede cumplir tal cual, detén **esa** tarea y pregunta en el reporte.

**Migraciones:**
- 2A: `20261104000100_fase2a_inventario.sql`
- 2B: `20261105000100_fase2b_compras.sql`

**Fuera de alcance en toda la fase:**
- números de serie;
- importación desde XLSX (solo CSV);
- conteo masivo con Excel;
- lectura de facturas con IA;
- crear productos desde una orden de compra;
- cruce de tres vías;
- avisos diarios de vencimiento (son de la fase 7);
- exportación al banco;
- recetas y producción (fase 5).

---

# ENTREGA 2A

## F2.1 Campos de producto y unidades

### Migración

```sql
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Kilogramo','KG'),('Litro','L'),('Metro','M'),
  ('Caja','CAJA'),('Paquete','PAQUETE'),('Bulto','BULTO'),('Libra','LB')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.products
  add column kind text not null default 'FISICO' check (kind in ('FISICO','SERVICIO','PREPARADO')),
  add column usage text not null default 'VENTA' check (usage in ('VENTA','INSUMO','INTERNO')),
  add column purchase_uom_id integer references public.tbl_tipos_maestra(tima_id),
  add column purchase_factor numeric(14,4) not null default 1 check (purchase_factor > 0),
  add column reorder_qty numeric(14,4) check (reorder_qty > 0),
  add column max_qty numeric(14,4) check (max_qty > 0),
  add column track_lots boolean not null default false,
  add column parent_id uuid,
  add column has_variants boolean not null default false,
  add column variant_attrs jsonb,
  add column archived_at timestamptz,
  add column description text check (length(description) <= 500),
  add constraint products_parent_fkey foreign key (tenant_id, parent_id) references public.products (tenant_id, id),
  add constraint products_variant_shape check (
    (parent_id is null) or (has_variants = false and variant_attrs is not null)),
  add constraint products_service_no_lots check (kind <> 'SERVICIO' or track_lots = false);
```

La columna `unit_measure_id`, que ya existe, es la **unidad de inventario**.

### Significado de cada valor

| Valor | Significado |
|---|---|
| `kind = SERVICIO` | Nunca toca el stock: `sell_tx` lo salta. |
| `kind = PREPARADO` | Tiene receta y descuenta sus insumos (es lo que hoy hace `is_composed`; se mantienen las dos columnas). |
| `kind = FISICO` | Descuenta su propio stock. |
| `usage = INSUMO` o `INTERNO` | **No se vende**: no aparece en `/v1/catalog`, y venderlo responde 422 `PRODUCT_NOT_SELLABLE`. |

### Reglas
- `unit_measure_id` no se puede cambiar si el producto tiene filas en `inventory_movements`: 409 `PRODUCT_UOM_LOCKED`.
- `purchase_factor` es cuántas unidades de inventario trae una unidad de compra. Ejemplo: se compra por `CAJA` con factor 24 y se inventaría en `UND`.

## F2.2 Variantes (como productos hijos)

**Modelo:** una variante es una fila de `products` con `parent_id` apuntando al producto padre. El padre tiene `has_variants = true`, **no tiene stock y no se vende**. Cada variante tiene su `sku`, su `barcode`, su precio en `branch_products` y su stock propio. Así, todo el código de stock funciona igual.

- `variant_attrs` es un objeto plano de texto a texto, por ejemplo `{"Talla":"M","Color":"Rojo"}`. Máximo 3 claves. Cada clave tiene de 1 a 20 caracteres y cada valor de 1 a 30.
- **Nombre visible** de una variante: `"<nombre del padre> - <valores separados por ' / '>"`, por ejemplo "Camiseta polo - M / Rojo". El campo `name` de la variante guarda ese texto y se recalcula cada vez que se edita el padre o la variante.
- Un padre con `has_variants = true` no puede tener variantes de presentación: si tiene filas en `product_tier_prices` con tarifas `PRESENTACION`, responde 422 `VARIANTS_AND_PRESENTATIONS`. Al revés, igual: no se pueden asignar presentaciones a un padre con variantes ni a sus variantes.
- **Unicidad:** `sku` y `barcode` son únicos por negocio entre todas las filas de `products`, incluidas las variantes. Ya lo garantiza `unique (tenant_id, sku)`. Los archivados **siguen ocupando** el código. Esto es una decisión: no lo cambies.
- Las variantes heredan del padre `kind`, `usage`, `tax_type_id`, `unit_measure_id`, `purchase_uom_id`, `purchase_factor` y `track_lots`. Al editar el padre, esos campos se copian a sus variantes en la misma transacción.
- No se puede vender, reservar ni mover stock de un padre con variantes: 422 `PRODUCT_IS_VARIANT_PARENT`.

## F2.3 CRUD de productos y ficha (pantalla 15)

### Backend: archivo nuevo `src/products.rs`

| Método y ruta | Permiso | Cuerpo y respuesta |
|---|---|---|
| `GET /v1/products?branch_id=&q=&category_id=&usage=&archived=false&page=&page_size=` | `inventory:read` | `{items:[ProductRow], total_count}` |
| `GET /v1/products/{id}?branch_id=` | `inventory:read` | `ProductDetail` |
| `POST /v1/products` | `inventory:adjust` | `ProductInput` → `201 ProductDetail` |
| `PUT /v1/products/{id}` | `inventory:adjust` | `ProductInput` → `200 ProductDetail` |
| `POST /v1/products/{id}/variants` | `inventory:adjust` | `{sku?, barcode?, attrs, price, cost_price?}` → `201 ProductDetail` (de la variante) |
| `PUT /v1/branch-products/{product_id}` | `inventory:adjust` | `{branch_id, price, category_id?, is_available}` |

- `ProductInput = {name, sku?, barcode?, kind, usage, unit_measure_code, purchase_uom_code?, purchase_factor, tax_code ("IVA19"|"IVA5"|"INC8"|"EXENTO"), cost_price?, base_price?, reorder_qty?, max_qty?, track_lots, allow_price_override, has_variants, description?}`.
  - `EXENTO` guarda `tax_type_id = null`.
- `ProductRow = {id, name, sku, barcode, kind, usage, price (de la sucursal), cost_price, tax_code, on_hand (suma de las ubicaciones de la sucursal), category_name, has_variants, parent_id, archived}`.
- `ProductDetail = ProductRow + {unit_measure_code, purchase_uom_code, purchase_factor, reorder_qty, max_qty, track_lots, allow_price_override, description, variants: [ProductRow], tier_prices, option_groups (F2.4), stock_by_location: [{location_id, location_name, on_hand, reserved, min_qty, avg_cost}]}`.

**Validaciones, en este orden:**
1. `name` entre 1 y 120 caracteres.
2. `sku` de 1 a 40 caracteres, de `[A-Za-z0-9-_.]`.
3. `barcode` solo de dígitos, de 6 a 14.
4. Un `barcode` que contiene `E+` o `e+` (notación científica de Excel): 422 `BARCODE_SCIENTIFIC`.
5. `base_price >= 0` y `cost_price >= 0`.
6. `usage = VENTA` exige `base_price`.

Lo que no cumpla las reglas 1, 2, 3, 5 y 6 responde 422 `PRODUCT_INVALID`. Un `sku` o `barcode` repetido responde 409 `SKU_TAKEN` o `BARCODE_TAKEN` (revísalo antes del insert, también contra `product_tier_prices.barcode`).

**Auditoría:** `PRODUCTO_CREADO`, `PRODUCTO_EDITADO` y `PRECIO_EDITADO` (esta última cuando cambia el precio de `branch_products`).

### Frontend
- **Ficha de producto:** página `app/(admin)/inventario/producto/[id]/page.tsx` y componente `components/inventario/product-screen.tsx`. La ruta `/inventario/producto/nuevo` crea un producto. Cambia la pantalla 15 a `"react"` en `screens.ts`.
- **Pestañas:**
  1. **General:** todos los campos de `ProductInput` y el precio de la sucursal.
  2. **Precios y tarifas:** mueve aquí el diálogo de F1.6 y elimina la acción del menú de `/inventario`.
  3. **Variantes:** solo si `has_variants`. Tabla de variantes y botón "Agregar variante", con los atributos, SKU, código, precio y costo.
  4. **Modificadores** (F2.4).
  5. **Stock por bodega** (F2.8).
  6. **Kardex:** el componente actual de movimientos, filtrado por el producto.
- En `/inventario`: botón "Nuevo producto", y cada fila abre la ficha.

### Pruebas (`f2_products`)
- **[P]** Un SKU repetido entre un producto y una variante de otro: 409 `SKU_TAKEN`.
- **[P]** Un insumo no aparece en `/v1/catalog` y venderlo: 422 `PRODUCT_NOT_SELLABLE`.
- **[P]** Vender un padre con variantes: 422 `PRODUCT_IS_VARIANT_PARENT`.
- **[P]** Variantes más presentaciones: 422 `VARIANTS_AND_PRESENTATIONS`.
- **[P]** Cambiar la unidad de un producto con movimientos: 409 `PRODUCT_UOM_LOCKED`.
- **[P]** Un servicio vendido no mueve el stock.

## F2.4 Modificadores

### Migración

```sql
create table public.option_groups (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  min_select integer not null default 0 check (min_select >= 0),
  max_select integer not null default 1 check (max_select >= 1),
  active boolean not null default true,
  unique (tenant_id, id), unique (tenant_id, name),
  check (min_select <= max_select)
);
create table public.options (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  group_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 40),
  price_delta numeric(14,2) not null default 0 check (price_delta >= 0),
  input_product_id uuid,
  input_qty numeric(14,4) check (input_qty > 0),
  sort_order integer not null default 0,
  active boolean not null default true,
  unique (tenant_id, id),
  foreign key (tenant_id, group_id) references public.option_groups (tenant_id, id) on delete cascade,
  foreign key (tenant_id, input_product_id) references public.products (tenant_id, id),
  check ((input_product_id is null) = (input_qty is null))
);
create table public.product_option_groups (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  group_id uuid not null,
  sort_order integer not null default 0,
  primary key (product_id, group_id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade,
  foreign key (tenant_id, group_id) references public.option_groups (tenant_id, id)
);
create table public.order_item_options (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  order_item_id uuid not null,
  option_id uuid not null,
  name text not null,
  price_delta numeric(14,2) not null,
  primary key (order_item_id, option_id),
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id) on delete cascade,
  foreign key (tenant_id, option_id) references public.options (tenant_id, id)
);
-- RLS a nivel de negocio en las 4 tablas, más grants.
```

### Venta y cotización
`SaleItem` agrega `option_ids: Vec<Uuid>` (por defecto vacío). Valida en este orden:
1. Cada opción existe, está activa y su grupo está asignado al producto. Si no: 422 `OPTION_INVALID`.
2. Por cada grupo asignado, el número de opciones elegidas debe estar entre `min_select` y `max_select`. Si no: 422 `OPTION_SELECTION_INVALID`, con `data: {group_id}`.
3. Una línea con opciones y tarifa `PRESENTACION`: 422 `OPTION_INVALID`.
4. **Precio:** al `unit_price` y al `list_price` resueltos se les suma `Σ price_delta`, antes del motor de descuentos y antes de evaluar el cambio de precio.
5. **Stock:** por cada opción con insumo, se descuenta `qty_línea × input_qty` de `input_product_id`, en la misma llamada a `sell_tx`. Agrégalo como líneas extra de componentes.
6. Se guardan en `order_item_options`, con `name` y `price_delta` congelados.

### Endpoints (archivo nuevo `src/options.rs`, permiso `inventory:adjust`)
- `GET` y `POST /v1/option-groups`, y `PUT /v1/option-groups/{id}`. El cuerpo incluye `options: [{id?, name, price_delta, input_product_id?, input_qty?, sort_order, active}]`, que **reemplaza** la lista completa. Las opciones que ya no vienen se desactivan; no se borran.
- `PUT /v1/products/{id}/option-groups`, con cuerpo `[{group_id, sort_order}]`.

### Frontend
- Pestaña "Modificadores" de la ficha: asignar grupos.
- Pantalla `/inventario/modificadores` (registro `s("37", "modificadores", "Modificadores", "/inventario/modificadores", "inventario", "admin", "react")`) para crear grupos y opciones.
- **POS:** al agregar un producto con grupos asignados, se abre el diálogo "Personalizar", con un grupo por sección, casillas o radios según `max_select` y el precio extra de cada opción. No se puede confirmar si falta un grupo obligatorio.

### Pruebas (`f2_options`)
- **[P]** "Extra queso" con un delta de $2.000 y 30 g de queso: el precio sube $2.000 por unidad y el queso baja 30 g por unidad.
- **[P]** No elegir el mínimo de un grupo obligatorio: 422 `OPTION_SELECTION_INVALID`.

## F2.5 Archivar con baja de stock

`POST /v1/products/{id}/archive` (permiso `inventory:adjust`), con cuerpo `{confirm: bool}`.

1. Si el producto tiene reservas `ACTIVA` en cualquier ubicación: 409 `PRODUCT_HAS_RESERVATIONS`.
2. Calcula el plan: `[{location_id, location_name, on_hand, avg_cost, value = money(on_hand × avg_cost)}]`, solo de las ubicaciones con `on_hand != 0`.
3. Si el plan no está vacío y `confirm = false`: 409 `ARCHIVE_NEEDS_CONFIRM`, con `data: {plan, total_value}`.
4. Con `confirm = true` (o con el plan vacío), en la misma transacción:
   1. por cada fila del plan, registra un ajuste de salida con causa "Baja por archivo" mediante `apply_delta_tx`, con `movement = AJUSTE_SALIDA`, `ref_type = 'ARCHIVE'` y `ref_id = product_id`;
   2. `archived_at = now()` y `branch_products.is_available = false` en todas las sucursales;
   3. si es un padre, archiva también todas sus variantes con el mismo proceso;
   4. auditoría `PRODUCTO_ARCHIVADO`, con el plan en `after`.

`POST /v1/products/{id}/unarchive` lo reactiva sin stock (`archived_at = null`). No cambia `is_available`.

**Prueba [P]** (`f2_archive`): sin confirmar responde 409 con el plan; al confirmar, el stock queda en 0 con un movimiento por ubicación.

## F2.6 Importación CSV en dos pasos

### Migración

```sql
create table public.import_sessions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  created_by uuid not null references public.users(id),
  rows jsonb not null,
  committed_rows integer not null default 0,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '24 hours',
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
-- RLS con in_scope; grants.
```

### Formato del CSV
- UTF-8, separador `;` o `,` (se detecta con la primera línea) y primera fila de encabezados.
- Encabezados exactos, sin distinguir mayúsculas: `sku, codigo_barras, nombre, categoria, unidad, impuesto, costo, precio, cantidad_inicial, sku_padre, atributos`.
- `atributos` usa el formato `Talla=M|Color=Rojo`.
- Máximo 5.000 filas y 2 MB.

Se permite agregar el crate `csv`; justifícalo en el reporte.

### `POST /v1/products/import/analyze`
- Permiso `inventory:adjust`.
- `multipart/form-data` con los campos `file` y `branch_id`.
- Valida cada fila y la marca como `OK`, `ADVERTENCIA` o `ERROR`. Reglas, en este orden:

| Condición | Resultado |
|---|---|
| `nombre` vacío | ERROR "nombre vacío" |
| `codigo_barras` en notación científica | ERROR "código en notación científica" |
| `impuesto` fuera de `IVA19`, `IVA5`, `INC8`, `EXENTO` | ERROR |
| `unidad` no existe en `UOM` | ERROR |
| `precio` o `costo` no numérico o negativo | ERROR |
| `sku` existe en el negocio | `OK` con acción `ACTUALIZAR` |
| `sku` no existe | acción `CREAR` |
| `categoria` no existe en la sucursal | ADVERTENCIA "se creará la categoría" |
| `sku_padre` con `atributos` | la fila es una variante del padre con ese SKU; si el padre no existe ni está en el archivo: ERROR |
| `cantidad_inicial > 0` y el producto ya tiene stock | ADVERTENCIA "se ignora la cantidad inicial" |

- Respuesta: `{session_id, summary: {ok, warnings, errors, create, update}, rows: [{line, status, action, messages: [], data}]}`.
- Con al menos un `ERROR`, la sesión se guarda igual, pero `commit` responde 409 `IMPORT_HAS_ERRORS`.

### `POST /v1/products/import/commit`
- Cuerpo `{session_id, offset, limit}`. `limit` máximo 200.
- Aplica las filas de `offset` a `offset + limit − 1`, en una transacción por llamada.
- Crea y actualiza productos con la misma lógica de F2.3. La cantidad inicial se carga con `apply_delta_tx` usando `movement = AJUSTE_ENTRADA`, causa "Carga inicial" y el costo de la fila.
- Respuesta: `{committed_rows, done: bool}`.
- Reintentar un rango ya aplicado no hace nada: compara con `committed_rows`.
- Sesión vencida: 410 `IMPORT_EXPIRED`.

### Frontend
Pantalla `/inventario/importar` (registro `s("38", "importar", "Importar productos", "/inventario/importar", "inventario", "admin", "react")`) con tres pasos:
1. Subir el archivo y descargar la plantilla. La plantilla es un CSV estático en `public/plantilla-productos.csv`, con 3 filas de ejemplo.
2. Tabla de resultados con filtros por estado.
3. Botón "Importar N productos". Llama a `commit` en páginas de 200 y muestra una barra de progreso.

### Prueba (`f2_import`)
- **[P]** Un código `7.70123E+12` da ERROR; un CSV con errores no se confirma; un CSV válido crea los productos y las variantes, y un reintento del mismo rango no duplica nada.

## F2.7 Etiquetas con código de barras

### Migración

```sql
create table public.tenant_counters (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null,
  next_value bigint not null default 1,
  primary key (tenant_id, name)
);
-- RLS a nivel de negocio.
```

### Backend
`POST /v1/products/{id}/generate-barcode` (permiso `inventory:adjust`):
1. Si el producto ya tiene `barcode`: 409 `BARCODE_EXISTS`.
2. Toma el siguiente número de `tenant_counters` con `name = 'EAN_INTERNO'`, con `update … returning` atómico.
3. Arma 12 dígitos: `"2"` + el dígito `0` + el número con ceros a la izquierda hasta 10 dígitos.
4. Calcula el dígito de control EAN-13 y lo agrega como dígito 13. Función pura `ean13_check(&str) -> u8`, con prueba unitaria `ean13_check_digit`. Con la entrada `200000000001`, el dígito de control es **5**.
5. Si el código existe (por ejemplo, porque se cargó a mano), toma el siguiente número.

### Frontend
Pantalla `/inventario/etiquetas` (registro `s("39", "etiquetas", "Etiquetas", "/inventario/etiquetas", "inventario", "admin", "react")`):
- buscador para agregar productos, cada uno con su cantidad de etiquetas;
- selector de formato: "Rollo 50×25 mm" o "Hoja A4 3×8";
- botón "Imprimir".

Cada etiqueta lleva el nombre (máximo 2 líneas), el precio **con impuesto** (`price × (1 + tasa)`, redondeado a pesos) y el código de barras en EAN-13 si tiene 13 dígitos, o en CODE128 si no. Se permite agregar el paquete npm `jsbarcode`; justifícalo en el reporte. Los productos sin código muestran el botón "Generar código".

## F2.8 Bodegas (ubicaciones)

### Migración

```sql
create table public.locations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 40),
  kind text not null check (kind in ('PISO_VENTA','BODEGA','PRODUCCION','RECEPCION','DESPACHO','CUARENTENA','AVERIADOS')),
  sellable boolean not null,
  is_default boolean not null default false,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, branch_id, id),
  unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  check (not (kind in ('CUARENTENA','AVERIADOS') and sellable)),
  check (not is_default or (sellable and active))
);
create unique index locations_one_default on public.locations (branch_id) where is_default;
-- RLS con in_scope; grants.

insert into public.locations (tenant_id, branch_id, name, kind, sellable, is_default)
select tenant_id, id, 'Principal', 'PISO_VENTA', true, true from public.branches;
```

Agrega `insert into locations …` al trigger `branch_default_register` de la fase 1, para que toda sucursal nueva tenga su "Principal".

```sql
alter table public.stock add column location_id uuid;
update public.stock s set location_id = l.id from public.locations l where l.branch_id = s.branch_id and l.is_default;
alter table public.stock alter column location_id set not null;
alter table public.stock add constraint stock_location_fkey
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);
alter table public.stock drop constraint stock_branch_id_product_id_key;   -- verifica el nombre real
alter table public.stock add constraint stock_location_product_key unique (location_id, product_id);
-- Lo mismo para inventory_movements.location_id y stock_reservations.location_id (backfill con la ubicación por defecto, not null y FK).
alter table public.cash_registers add column location_id uuid,
  add constraint cash_registers_location_fkey foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);
```

### Reglas
- **Ventas:** descuentan de la ubicación de la caja del turno (`cash_registers.location_id`); si es nula, de la ubicación por defecto de la sucursal. **Nunca se reparte** entre ubicaciones.
- **Reservas:** usan la ubicación por defecto.
- **Recepción de compras:** usa la ubicación destino de la orden.
- **Traslados:** el endpoint actual `POST /v1/inventory/transfer` acepta `from_location_id` y `to_location_id` en lugar de las sucursales. Si las dos son de la misma sucursal, es un traslado interno; si son de sucursales distintas, es el traslado que ya existe. Son los mismos tipos de movimiento (`TRASLADO_SALIDA` y `TRASLADO_ENTRADA`) y la misma función. Mantén los parámetros viejos (`from_branch_id`, `to_branch_id`): se traducen a las ubicaciones por defecto.
- **Endpoints de stock y kardex:** reciben `location_id` opcional; sin él, suman todas las ubicaciones de la sucursal.

### Endpoints (permiso `settings:write`)
- `GET /v1/locations?branch_id=`, `POST /v1/locations` y `PUT /v1/locations/{id}`.
- Cambiar la ubicación por defecto quita la marca de la anterior en la misma transacción.
- Desactivar una ubicación con stock distinto de 0: 409 `LOCATION_HAS_STOCK`.
- Una ubicación `CUARENTENA` o `AVERIADOS` con `sellable = true`: 422 `LOCATION_INVALID`.

### Frontend
- Pantalla `/configuracion/bodegas` (registro `s("40", "bodegas", "Bodegas", "/configuracion/bodegas", "configuracion", "admin", "react")`).
- El diálogo de traslado elige ubicaciones, agrupadas por sucursal.
- Pestaña "Stock por bodega" en la ficha de producto.
- En `/configuracion/cajas`, selector "Bodega de la caja".

### Pruebas (`f2_locations`)
- **[P]** Una venta descuenta de la ubicación de la caja.
- **[P]** Un traslado interno mueve el stock entre dos ubicaciones de la misma sucursal y deja dos movimientos.
- **[P]** Las ubicaciones `AVERIADOS` no se pueden marcar vendibles.

## F2.9 Flujo único de stock

### Migración

```sql
alter table public.stock drop constraint stock_on_hand_check;    -- verifica los nombres reales
alter table public.stock drop constraint stock_check;            -- (on_hand >= reserved)
alter table public.inventory_movements
  add column unit_cost numeric(14,4),
  add column total_cost numeric(14,2),
  add column balance_after numeric(14,4),
  add column lot_id uuid;
create table public.stock_alerts (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  kind text not null check (kind in ('BAJO','NEGATIVO')),
  qty numeric(14,4) not null,
  created_at timestamptz not null default now(),
  resolved_at timestamptz,
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create unique index stock_alerts_open on public.stock_alerts (location_id, product_id, kind) where resolved_at is null;
-- RLS con in_scope; grants.
```

### Backend: la **única** función que escribe en `stock` (`inventory.rs`)

```rust
pub struct StockDelta<'a> {
    pub branch_id: Uuid, pub location_id: Uuid, pub product_id: Uuid,
    pub delta: Decimal,                 // + entra, − sale; nunca 0
    pub movement: &'a str,              // código MOV: REPOSICION, DESCUENTO, AJUSTE_ENTRADA, AJUSTE_SALIDA, TRASLADO_SALIDA, TRASLADO_ENTRADA, DEVOLUCION
    pub unit_cost: Option<Decimal>,     // obligatorio en entradas con costo conocido
    pub lot_id: Option<Uuid>,
    pub ref_type: &'a str, pub ref_id: Uuid, pub cause: Option<String>,
}
pub struct Applied { pub unit_cost: Decimal, pub total_cost: Decimal, pub cost_missing: bool, pub balance_after: Decimal }
pub async fn apply_delta_tx(tx: &mut Tx<'_>, scope: &Scope, d: StockDelta<'_>) -> Result<Applied, ApiError>
```

Pasos, **en este orden**:
1. `delta == 0`: 422 `STOCK_DELTA_ZERO`.
2. El producto es un padre con variantes: 422 `PRODUCT_IS_VARIANT_PARENT`. Es un servicio: no hace nada y devuelve un costo de 0.
3. Bloquea la fila con `select … from stock where location_id = $1 and product_id = $2 for update`. Si no existe, la inserta con `on_hand = 0` y la vuelve a bloquear.
4. **Salida** (`delta < 0`):
   - `disponible = on_hand − reserved`;
   - si `disponible < |delta|` y `settings.inventory.allow_negative_stock` es falso: 409 `STOCK_INSUFFICIENT`, con `data: {product_id, available}`;
   - si se permite el negativo, sigue igual.
5. **Costo** (F2.10): en una entrada crea la capa y actualiza `avg_cost`; en una salida consume capas y calcula el costo.
6. `update stock set on_hand = on_hand + delta, updated_at = now() where id = $1`.
7. Inserta en `inventory_movements`, con `qty = |delta|`, `unit_cost`, `total_cost`, `balance_after` (el nuevo `on_hand`), `lot_id`, `location_id`, `ref_type` y `ref_id`.
8. **Alertas:**
   - si `on_hand < 0`, inserta una alerta `NEGATIVO` (`on conflict do nothing`);
   - si `on_hand <= min_qty` y `min_qty > 0`, inserta una alerta `BAJO` (`on conflict do nothing`);
   - si `on_hand > min_qty`, resuelve las alertas `BAJO` abiertas, y si `on_hand >= 0`, las `NEGATIVO` abiertas (`resolved_at = now()`).
9. Devuelve `Applied`.

**Refactor obligatorio:** `sell_tx`, `consume_*`, `replenish_*`, `adjust_tx`, `count`, `transfer`, `return_tx` (de la fase 1) y la recepción de compras llaman a `apply_delta_tx`. Busca con `grep -n "update stock" src/`: al terminar, el único resultado debe estar dentro de `apply_delta_tx`. Pega esa salida en el reporte. Las reservas (`reserved`) siguen con sus funciones actuales, que solo tocan la columna `reserved`.

`GET /v1/inventory/alerts?branch_id=` (permiso `inventory:read`) devuelve las alertas abiertas. En `/inventario`, la insignia "N alertas" abre la lista.

### Pruebas (`f2_pipeline`)
- **[P]** Dos ventas concurrentes por la última unidad: solo una pasa y la otra recibe 409 `STOCK_INSUFFICIENT`. Usa dos transacciones en tareas de tokio, igual que la prueba de secuencias.
- **[P]** Con `allow_negative_stock = true`, el stock queda negativo y se crea una alerta `NEGATIVO`. **Nunca** se recorta a 0.
- **[P]** Bajar de `min_qty` crea una alerta `BAJO`, y reponer la resuelve.
- **Todas las pruebas que ya existían siguen pasando.**

## F2.10 Costeo

### Migración

```sql
alter table public.stock add column avg_cost numeric(14,4) not null default 0 check (avg_cost >= 0);
update public.stock s set avg_cost = coalesce(p.cost_price, 0) from public.products p where p.id = s.product_id;
create table public.cost_layers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  qty_remaining numeric(14,4) not null check (qty_remaining >= 0),
  unit_cost numeric(14,4) not null check (unit_cost >= 0),
  source_type text not null,
  source_id uuid,
  received_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create index cost_layers_fifo on public.cost_layers (location_id, product_id, received_at, id) where qty_remaining > 0;
insert into public.cost_layers (tenant_id, branch_id, location_id, product_id, qty_remaining, unit_cost, source_type)
select tenant_id, branch_id, location_id, product_id, on_hand, avg_cost, 'MIGRACION' from public.stock where on_hand > 0;
alter table public.order_items
  add column unit_cost numeric(14,4),
  add column cost_total numeric(14,2),
  add column cost_missing boolean not null default false;
-- RLS de cost_layers con in_scope; grants.
```

**Ajuste nuevo:** `inventory.costing`, con valores `PROMEDIO` (por defecto) o `PEPS`. Va en `InventorySettings` y en la pestaña Inventario de `/configuracion/negocio`.

### Reglas dentro de `apply_delta_tx`

**Entrada** con `unit_cost = c` y cantidad `q`:
1. Antes de sumar: `q0 = max(on_hand, 0)` y `a0 = avg_cost`.
2. `avg_cost = (q0 × a0 + q × c) / (q0 + q)`, redondeado a 4 decimales. Si `q0 + q = 0`, `avg_cost = c`.
3. Inserta una capa con `qty_remaining = q` y `unit_cost = c`.
4. Una entrada **sin** `unit_cost` (traslados de entrada y devoluciones sin costo) usa `c = avg_cost` actual.

**Salida** de cantidad `q`:
1. Consume capas de la más vieja a la más nueva (`received_at`, `id`), con `for update`, restando de `qty_remaining`. Esto pasa con cualquier método.
2. Costo total:
   - `PROMEDIO`: `money(q × avg_cost)`.
   - `PEPS`: la suma de `cantidad tomada × unit_cost` de cada capa. Si las capas no alcanzan, el faltante se valora con el costo de respaldo.
3. **Respaldo** (en este orden): `avg_cost` si es mayor que 0; si no, `products.cost_price` si es mayor que 0; si no, 0 con `cost_missing = true`.
4. `avg_cost` no cambia en las salidas.

**Traslado:** la salida calcula el costo en el origen, y la entrada recibe ese `unit_cost` en el destino.

**Venta:** `sell_tx` guarda en cada línea:
- `cost_total`: la suma de los costos de todas sus salidas (incluidos los insumos de recetas y opciones);
- `unit_cost = cost_total / qty`;
- `cost_missing`: verdadero si alguna de sus salidas lo tuvo.

### Pruebas (`f2_costing`)
- **[P]** Promedio: entran 10 a $100 y 10 a $200, y el promedio queda en $150. Luego salen 5 con un costo de venta de $750.
- **[P]** PEPS con las mismas entradas: salen 15 y el costo es 10 × 100 + 5 × 200 = $2.000.
- **[P]** Respaldo: un producto sin capas, sin promedio y con `cost_price = 80` cuesta 80 por unidad. Si `cost_price` es nulo, el costo es 0 y `cost_missing = true`.
- **[P]** Un traslado conserva el costo de origen.

### Fin de la entrega 2A
Escribe `TERMINADA-REPORTE-FASE-2A.md` y **detente**.

---

# ENTREGA 2B

## F2.11 Proveedores

### Migración

```sql
alter table public.suppliers
  add column doc_type text not null default 'NIT' check (doc_type in ('NIT','CC','CE')),
  add column dv char(1) check (dv ~ '^[0-9]$'),
  add column person_type text not null default 'JURIDICA' check (person_type in ('NATURAL','JURIDICA')),
  add column iva_responsible boolean not null default true,
  add column self_withholding boolean not null default false,
  add column payment_terms text not null default 'CONTADO' check (payment_terms in ('CONTADO','CREDITO')),
  add column credit_days integer not null default 0 check (credit_days between 0 and 180),
  add column delivery_days integer check (delivery_days between 0 and 60),
  add column category text not null default 'MERCANCIA' check (category in ('MERCANCIA','TRANSPORTE','SERVICIOS')),
  add column bank_name text, add column bank_account_type text check (bank_account_type in ('AHORROS','CORRIENTE')),
  add column bank_account_number text check (bank_account_number ~ '^[0-9]{6,20}$'),
  add column archived_at timestamptz,
  add constraint suppliers_credit check (payment_terms = 'CREDITO' or credit_days = 0);
create table public.supplier_products (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  supplier_id uuid not null,
  product_id uuid not null,
  supplier_code text,
  last_cost numeric(14,4),
  min_order_qty numeric(14,4) check (min_order_qty > 0),
  preferred boolean not null default false,
  primary key (supplier_id, product_id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create unique index supplier_products_one_preferred on public.supplier_products (product_id) where preferred;
alter table public.tenants add column iva_responsible boolean;   -- null = desconocido
-- RLS a nivel de negocio en supplier_products; grants.
```

### Dígito de verificación del NIT
Función pura `nit_dv(nit: &str) -> u8`:
1. Toma los dígitos de derecha a izquierda y multiplícalos por los pesos `3, 7, 13, 17, 19, 23, 29, 37, 41, 43, 47, 53, 59, 67, 71`, en ese orden.
2. Suma los productos y calcula `r = suma mod 11`.
3. Si `r` es 0 o 1, el DV es `r`. Si no, es `11 − r`.

Prueba unitaria `nit_dv_known_values`: `900123456 → 8`, `800197268 → 4`, `860001022 → 7`.

Si `doc_type = NIT` y llega un `dv`, debe coincidir con `nit_dv`. Si no: 422 `NIT_DV_INVALID`. Si no llega `dv`, se calcula y se guarda.

### Endpoints (amplía los que ya existen en `procurement.rs`; permiso `purchases:create` para escribir e `inventory:read` para leer)
- `POST` y `PUT /v1/suppliers` aceptan todos los campos nuevos.
- `POST /v1/suppliers/{id}/archive`: si tiene órdenes en `PENDIENTE`, `APROBADA` o `PARCIAL`, o cuentas por pagar con saldo: 409 `SUPPLIER_HAS_OPEN_DOCS`.
- `GET /v1/suppliers/{id}`: `{supplier, products: [...], open_orders: N, balance: $}`.
- `PUT /v1/suppliers/{id}/products`: reemplaza la lista `[{product_id, supplier_code?, min_order_qty?, preferred}]`. Marcar `preferred` quita la marca del proveedor anterior de ese producto, en la misma transacción.

### Frontend
Pantalla `/compras/proveedores` (ya existe; amplíala). La ficha es un panel lateral con las pestañas Resumen, Productos, Órdenes y Por pagar.

### Prueba (`f2_suppliers`)
- **[P]** Un DV incorrecto: 422 `NIT_DV_INVALID`.
- **[P]** Archivar con una orden abierta: 409 `SUPPLIER_HAS_OPEN_DOCS`.

## F2.12 Compras v2

**Decisión:** la tabla `purchase_receipts` **es** la orden de compra. No se renombra.

### Migración

```sql
update public.tbl_tipos_maestra set tima_nombre = 'Borrador'
 where tima_codigo = 'PENDIENTE' and padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PUR');
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Aprobada','APROBADA'),('Parcial','PARCIAL')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PUR') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.purchase_receipts
  add column location_id uuid,
  add column expected_at date,
  add column discount_amount numeric(14,2) not null default 0 check (discount_amount >= 0),
  add column freight_amount numeric(14,2) not null default 0 check (freight_amount >= 0),
  add column freight_mode text not null default 'PRORRATEAR' check (freight_mode in ('PRORRATEAR','GASTO')),
  add column payment_terms text not null default 'CONTADO' check (payment_terms in ('CONTADO','CREDITO')),
  add column credit_days integer not null default 0,
  add column iva_to_cost boolean not null default false,
  add column approved_by uuid references public.users(id),
  add column approved_at timestamptz,
  add constraint purchase_receipts_location_fkey foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);
update public.purchase_receipts p set location_id = l.id from public.locations l where l.branch_id = p.branch_id and l.is_default;
alter table public.purchase_receipts alter column location_id set not null;

alter table public.purchase_receipt_items
  add column qty_received numeric(14,4) not null default 0 check (qty_received >= 0),
  add column discount_alloc numeric(14,2) not null default 0,
  add column freight_alloc numeric(14,2) not null default 0,
  add column tax_amount numeric(14,2) not null default 0,
  add column landed_unit_cost numeric(14,4),          -- por unidad de INVENTARIO
  add column new_sale_price numeric(14,2) check (new_sale_price >= 0),
  add constraint pri_received_le_qty check (qty_received <= qty);

create table public.purchase_receptions (
  id uuid primary key,                                 -- generado por el cliente
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  purchase_id uuid not null,
  received_by uuid not null references public.users(id),
  received_at timestamptz not null default now(),
  client_request jsonb not null,
  unique (tenant_id, id),
  foreign key (tenant_id, purchase_id) references public.purchase_receipts (tenant_id, id)
);
create table public.purchase_reception_items (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  reception_id uuid not null,
  purchase_item_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),           -- en unidad de COMPRA
  lot_id uuid,
  foreign key (tenant_id, reception_id) references public.purchase_receptions (tenant_id, id) on delete cascade,
  foreign key (tenant_id, purchase_item_id) references public.purchase_receipt_items (tenant_id, id)
);
-- RLS: purchase_receptions con in_scope; purchase_reception_items a nivel de negocio. Grants.
```

**Unidades:** `purchase_receipt_items.qty` y `unit_cost` están en **unidad de compra**. Stock que entra = `qty × products.purchase_factor`.

### Costeo de la orden: función pura `procurement::cost_lines(lines, discount, freight, freight_mode, iva_to_cost) -> Vec<CostLine>`

Para cada línea `i`, con `rate_i` del impuesto del producto y `inc_i = (impuesto es INC8)`:
1. `gross_i = money(qty_i × unit_cost_i)`.
2. **Descuento global:** se reparte con `pricing::allocate(discount, pesos = gross_i)`. Las líneas con `unit_cost = 0` (obsequios) tienen peso 0. El resultado es `disc_i`, y `net_i = gross_i − disc_i`.
3. **Flete:**
   - con `PRORRATEAR`: se reparte con `allocate(freight, pesos = net_i)`; si `Σ net_i = 0`, los pesos son `qty_i`. El resultado es `freight_i`;
   - con `GASTO`: `freight_i = 0`. El flete se registra como gasto en F2.14, cuando se recibe la primera vez.
4. **Impuesto:** `tax_i = money(net_i × rate_i)`.
5. **Al costo:** `cost_base_i = net_i + freight_i + (tax_i si inc_i o iva_to_cost; si no, 0)`.
6. `landed_unit_cost_i = cost_base_i / (qty_i × purchase_factor_i)`, redondeado a 4 decimales.
7. Totales de la orden: `subtotal = Σ net_i`, `tax_amount = Σ tax_i` y `total = subtotal + tax_amount + freight` (el flete se suma en los dos modos).

Para fijar `iva_to_cost` al crear la orden:

| `tenants.iva_responsible` | `iva_to_cost` | Advertencia |
|---|---|---|
| `true` | `false` | — |
| `false` | `true` | — |
| `null` | `true` | La respuesta incluye `warnings: ["IVA_REGIME_UNKNOWN"]` |

### Estados y endpoints (en `procurement.rs`)

| Acción | Desde | Hacia | Permiso |
|---|---|---|---|
| `POST /v1/purchases` (crear, idempotente por `id`) | — | `PENDIENTE` | `purchases:create` |
| `PUT /v1/purchases/{id}` (reemplaza el encabezado y las líneas) | `PENDIENTE` | `PENDIENTE` | `purchases:create` |
| `POST /v1/purchases/{id}/approve` | `PENDIENTE` | `APROBADA` | `purchases:approve` |
| `POST /v1/purchases/{id}/receive` | `APROBADA`, `PARCIAL` | `PARCIAL` o `RECIBIDA` | `purchases:receive` |
| `POST /v1/purchases/{id}/cancel` | `PENDIENTE`, `APROBADA` | `ANULADA` | `purchases:approve` |
| `POST /v1/purchases/cost-preview` (no escribe) | — | — | `purchases:create` |

- Cualquier acción desde un estado no permitido: 409 `PURCHASE_INVALID_STATE`, con `data: {status}`.
- El cuerpo de crear y editar es el actual más `location_id, expected_at?, discount_amount, freight_amount, freight_mode, payment_terms, credit_days, items: [{product_id, qty, unit_cost, new_sale_price?}]`.
- Los productos deben ser `usage` `VENTA` o `INSUMO`, sin variantes en el padre. Si no: 422 `PURCHASE_INVALID`.
- `payment_terms` y `credit_days` se precargan desde el proveedor si no vienen.
- Al crear, editar y en la vista previa se ejecuta `cost_lines`, y se guardan `discount_alloc`, `freight_alloc`, `tax_amount` y `landed_unit_cost`.
- El endpoint viejo `receive`, que recibía todo sin cuerpo, se reemplaza.

### Recepción (`receive_tx`)
Cuerpo: `{id (uuid de la recepción), items: [{purchase_item_id, qty, lot_code?, expires_at?}]}`. Pasos:
1. **Idempotencia** por `purchase_receptions.id` y `client_request`. Mismo cuerpo: devuelve el detalle actual. Cuerpo distinto: 409 `IDEMPOTENCY_CONFLICT`.
2. Bloquea la orden con `for update` y valida el estado.
3. Para cada ítem: `pendiente = qty − qty_received`. Si `qty > pendiente`: 422 `RECEIVE_EXCEEDS_PENDING`, con `data: {purchase_item_id, pending}`.
4. **Lotes:** si el producto tiene `track_lots`, exige `lot_code` (de 1 a 30 caracteres) y `expires_at`. Si faltan: 422 `LOT_REQUIRED`. Ver F2.15.
5. **Stock:** `apply_delta_tx` con `movement = REPOSICION`, `delta = qty × purchase_factor`, `unit_cost = landed_unit_cost`, `ref_type = 'COMPRA'`, `ref_id = purchase_id`, la causa "Compra <document_number o id corto>" y `lot_id`.
6. `qty_received += qty`. Actualiza `supplier_products.last_cost` con el `landed_unit_cost` multiplicado por el factor, insertando la fila si no existe.
7. Si `new_sale_price` está presente, `branch_products.price = new_sale_price` en la sucursal de la orden, con auditoría `PRECIO_EDITADO`.
8. **Estado:** `RECIBIDA` si todas las líneas tienen `qty_received = qty`; si no, `PARCIAL`. `received_at = now()` en la primera recepción.
9. **Cuenta por pagar** (F2.13): llama a `payables::upsert_from_purchase_tx`.
10. Si `freight_mode = GASTO` y esta es la primera recepción, crea un gasto `APROBADO` con la categoría "Fletes" (se crea si no existe), el monto del flete y el proveedor (F2.14).
11. Auditoría `COMPRA_RECIBIDA`.

### Frontend
- `/compras`: tarjetas del mes (comprado, por recibir, por pagar), filtros por estado, proveedor y fechas, y la tabla.
- `/compras/nueva` y `/compras/[id]/editar`:
  - proveedor y bodega destino;
  - líneas con buscador, cantidad (con la unidad de compra visible, por ejemplo "CAJA x24"), costo unitario y nuevo precio de venta (opcional);
  - descuento global, flete y modo de flete, y condiciones de pago;
  - el panel "Vista previa de costos" llama a `cost-preview` con 400 ms de espera y muestra, por línea, el costo de llegada por unidad de inventario y el margen que resultaría con el precio actual o el nuevo;
  - botones "Guardar borrador", "Aprobar" y "Aprobar y recibir todo". Este último llama a `approve` y luego a `receive` con todas las cantidades pendientes.
- `/compras/[id]`:
  - encabezado y líneas con lo pedido, lo recibido y lo pendiente;
  - el botón "Recibir" abre un diálogo con las cantidades (precargadas con lo pendiente) y, solo en los productos con `track_lots`, los campos de lote y vencimiento;
  - la línea de tiempo sale de `audit_log` con `entity = 'purchase_receipts'` y `entity_id = id`.

### Pruebas (`f2_purchases`)
- **[P]** El reparto del descuento y del flete suma exacto. Las líneas de obsequio no reciben descuento.
- **[P]** Una recepción parcial y luego la total: el estado pasa por `PARCIAL` y llega a `RECIBIDA`.
- **[P]** Recibir más de lo pendiente: 422 `RECEIVE_EXCEEDS_PENDING`.
- **[P]** Con `iva_responsible = true`, el IVA no entra al costo; con `false`, sí. El INC entra siempre.
- **[P]** La recepción crea una capa de costo con `landed_unit_cost` y deja un movimiento `REPOSICION` con `ref_type = 'COMPRA'`.
- **[P]** Cancelar una orden `PARCIAL`: 409 `PURCHASE_INVALID_STATE`.
- **[P]** Un cajero que intenta crear una orden: 403.
- **[P]** La unidad de compra se convierte: 2 CAJAS con factor 24 suben 48 unidades.

## F2.13 Cuentas por pagar

### Migración

```sql
create table public.payables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  supplier_id uuid,
  source_type text not null check (source_type in ('COMPRA','GASTO')),
  source_id uuid not null,
  amount_total numeric(14,2) not null check (amount_total >= 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  status text not null default 'ABIERTA' check (status in ('ABIERTA','PARCIAL','PAGADA','CASTIGADA')),
  due_date date not null,
  written_off_reason text,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (source_type, source_id),
  check (amount_paid <= amount_total),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id)
);
create table public.payable_payments (
  id uuid primary key,                                  -- generado por el cliente
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  payable_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','TRANSFERENCIA','CHEQUE')),
  reference text,
  cash_session_id uuid,
  paid_at timestamptz not null default now(),
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  foreign key (tenant_id, payable_id) references public.payables (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
-- RLS: payables con in_scope; payable_payments a nivel de negocio. Grants.
```

### Reglas (`src/payables.rs`)
- **`upsert_from_purchase_tx(purchase_id)`:**
  - `amount_total` = valor recibido acumulado = `Σ por línea money(qty_received / qty × (net_i + freight_i + tax_i))`; con `freight_mode = GASTO`, sin flete;
  - `due_date` = fecha de la primera recepción + `credit_days` (0 si es de contado);
  - inserta, o actualiza `amount_total` si ya existe;
  - estado: `PAGADA` si `amount_paid = amount_total`; `PARCIAL` si `0 < amount_paid < total`; si no, `ABIERTA`.
- **"Vencida" no es un estado:** se calcula como `due_date < hoy y saldo > 0`, y la API lo devuelve en `overdue: bool` y `days_overdue`.

**`POST /v1/payables/{id}/payments`** (permiso `purchases:pay`), con cuerpo `{id, amount, method, reference?}`:
1. Idempotencia por `id`, igual que en las devoluciones.
2. Bloquea la cuenta con `for update`. Si está `PAGADA` o `CASTIGADA`: 409 `PAYABLE_CLOSED`.
3. `amount = money(amount)` debe ser menor o igual al saldo. Si no: 422 `PAYMENT_EXCEEDS_BALANCE`, con `data: {balance}`.
4. `TRANSFERENCIA` y `CHEQUE` exigen `reference` (de 3 a 60 caracteres). Si falta: 422 `PAYMENT_REFERENCE_REQUIRED`.
5. En `EFECTIVO`: turno abierto del usuario en la sucursal de la cuenta (si no tiene: 409 `PAYMENT_NEEDS_OPEN_SESSION`), y `cash::record_movement_tx` con el tipo `PAGO_PROVEEDOR` (si la fuente es `COMPRA`) o `GASTO` (si la fuente es `GASTO`), `ref_type = 'PAYABLE'` y `ref_id`.
6. Actualiza `amount_paid` y el estado. Si la fuente es `GASTO` y la cuenta queda `PAGADA`, el gasto pasa a `PAGADO`.
7. Auditoría `PAGO_PROVEEDOR`.

**`POST /v1/payables/{id}/write-off`** (permiso `purchases:pay`, y el usuario debe ser `DUENO`; si no: 403), con cuerpo `{reason}` (de 3 a 200 caracteres): `status = CASTIGADA`. Auditoría `CXP_CASTIGADA`.

**Consultas:**
- `GET /v1/payables?branch_id=&status=&supplier_id=&overdue=` (permiso `purchases:pay` o `reports:read`).
- `GET /v1/payables/aging?branch_id=` devuelve los montos por tramo, sobre el saldo de las cuentas no pagadas ni castigadas: `{al_dia, d1_30, d31_60, d61_90, d91_120, d120_mas}`.

### Frontend
Pantalla `/compras/por-pagar` (registro `s("41", "por-pagar", "Cuentas por pagar", "/compras/por-pagar", "inventario", "admin", "react")`):
- tarjetas de los 6 tramos;
- tabla con filtros;
- botón "Pagar", que abre un diálogo con monto, método y referencia.

### Pruebas (`f2_payables`)
- **[P]** Dos recepciones parciales hacen crecer una sola cuenta.
- **[P]** Un pago mayor al saldo: 422.
- **[P]** Un pago en efectivo crea un movimiento `PAGO_PROVEEDOR` y baja el esperado del turno.
- **[P]** El vencimiento usa los días de crédito del proveedor.
- **[P]** Los tramos de antigüedad suman el saldo total.

## F2.14 Gastos (pantalla 06 `/gastos`)

### Migración

```sql
create table public.expense_categories (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  color text not null default '#64748B' check (color ~ '^#[0-9A-Fa-f]{6}$'),
  active boolean not null default true,
  unique (tenant_id, id), unique (tenant_id, name)
);
create table public.expenses (
  id uuid primary key,                                  -- generado por el cliente
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  category_id uuid not null,
  supplier_id uuid,
  description text not null check (length(trim(description)) between 3 and 200),
  amount numeric(14,2) not null check (amount > 0),
  tax_amount numeric(14,2) not null default 0 check (tax_amount >= 0),
  total numeric(14,2) generated always as (amount + tax_amount) stored,
  expense_date date not null,
  status text not null default 'PENDIENTE' check (status in ('PENDIENTE','APROBADO','RECHAZADO','CANCELADO','PAGADO')),
  created_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, category_id) references public.expense_categories (tenant_id, id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id)
);
-- RLS: categorías a nivel de negocio; expenses con in_scope. Grants.
insert into public.permissions (code, module, description) values
  ('expenses:create','expenses','Registrar gastos'), ('expenses:approve','expenses','Aprobar gastos')
on conflict (code) do nothing;
-- role_permissions: DUENO y ADMIN_CAJA tienen los dos; SUPERVISOR tiene expenses:create.
```

### Flujo (`src/expenses.rs`)

| Acción | Desde | Hacia | Permiso | Efecto |
|---|---|---|---|---|
| Crear (`POST /v1/expenses`, idempotente) | — | `PENDIENTE` | `expenses:create` | Número de `sequences` `GASTO` |
| Aprobar | `PENDIENTE` | `APROBADO` | `expenses:approve` | Crea una cuenta por pagar con `source_type = GASTO`, `due_date = expense_date` y el proveedor si lo hay |
| Rechazar (`{reason}`) | `PENDIENTE` | `RECHAZADO` | `expenses:approve` | — |
| Cancelar | `PENDIENTE` | `CANCELADO` | `expenses:create` (solo el creador) o `expenses:approve` | — |
| Pagar | `APROBADO` | `PAGADO` | `purchases:pay` | Se hace con el pago de su cuenta por pagar (F2.13) |

- Cualquier transición no permitida: 409 `EXPENSE_INVALID_STATE`.
- Categorías: `GET`, `POST` y `PUT /v1/expense-categories` (permiso `expenses:approve`). La migración crea estas categorías para cada negocio: "Arriendo", "Servicios públicos", "Nómina", "Fletes", "Mantenimiento" y "Otros".

### Frontend
Pantalla 06 según `design/specs/06-gastos.md`, **sin** botones de IA y sin nada que la especificación pida y este documento no. Componentes:
- tarjetas del mes por estado;
- tabla;
- diálogo "Nuevo gasto";
- en cada fila, acciones según el estado y los permisos.

Pagar abre el diálogo de pago de la cuenta por pagar.

### Pruebas (`f2_expenses`)
- **[P]** Aprobar crea una cuenta por pagar.
- **[P]** Pagar en efectivo crea un movimiento `GASTO` en la caja y deja el gasto `PAGADO`.
- **[P]** Aprobar un gasto rechazado: 409.

## F2.15 Lotes y vencimientos

### Migración

```sql
create table public.lots (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  code text not null check (length(trim(code)) between 1 and 30),
  expires_at date not null,
  qty numeric(14,4) not null default 0,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (location_id, product_id, code),
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create table public.order_item_lots (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  order_item_id uuid not null,
  lot_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  primary key (order_item_id, lot_id),
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id) on delete cascade,
  foreign key (tenant_id, lot_id) references public.lots (tenant_id, id)
);
-- RLS: lots con in_scope; order_item_lots a nivel de negocio. Grants.
```

### Reglas para productos con `track_lots`
- **Entradas:** exigen un lote. La recepción crea el lote o le suma cantidad si ya existe con el mismo código en esa ubicación. Si el lote existe con otro `expires_at`: 422 `LOT_EXPIRY_MISMATCH`. `apply_delta_tx` recibe `lot_id` y actualiza `lots.qty`.
- **Ventas (FEFO):**
  1. Toma los lotes de la ubicación con `qty > 0` y `expires_at >= hoy` (en la zona de la sucursal), ordenados por `expires_at` y luego por `code`, con `for update`.
  2. Consume en ese orden, con un movimiento por lote, y guarda `order_item_lots`.
  3. Si los lotes vigentes no alcanzan, aunque haya lotes vencidos: 409 `LOT_ONLY_EXPIRED`, con `data: {product_id, available_valid}`.
  4. Esta regla **no** se salta con `allow_negative_stock`.
- **Ajustes y traslados:** de un producto con lotes exigen `lot_id`. Si falta: 422 `LOT_REQUIRED`.
- **Devoluciones** (`REINTEGRAR`): vuelven al primer lote de `order_item_lots` de esa línea.
- `GET /v1/inventory/expiring?branch_id=&days=30` (permiso `inventory:read`) devuelve los lotes con `qty > 0` que vencen dentro de `days`, más los ya vencidos con `qty > 0`.

### Frontend
Pantalla `/inventario/vencimientos` (registro `s("42", "vencimientos", "Vencimientos", "/inventario/vencimientos", "inventario", "admin", "react")`):
- pestañas Vencidos, 7 días, 15 días y 30 días;
- tabla con Producto, Lote, Bodega, Cantidad y Vence;
- acción "Dar de baja" (un ajuste `VENCIMIENTO` por la cantidad del lote).

### Pruebas (`f2_lots`)
- **[P]** Una venta toma primero el lote que vence antes.
- **[P]** Con solo lotes vencidos: 409 `LOT_ONLY_EXPIRED`.
- **[P]** Recibir un lote existente con otro vencimiento: 422 `LOT_EXPIRY_MISMATCH`.

## F2.16 Ajustes con aprobación real

### Migración

```sql
create table public.inventory_adjustments (
  id uuid primary key,                                  -- generado por el cliente
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  lot_id uuid,
  qty_delta numeric(14,4) not null check (qty_delta <> 0),
  reason_code text not null check (reason_code in ('DANO','PERDIDA','ROBO','VENCIMIENTO','CONTEO','CORRECCION','CARGA_INICIAL')),
  notes text check (length(notes) <= 300),
  unit_cost numeric(14,4) not null,
  value numeric(14,2) not null,
  status text not null check (status in ('PENDIENTE','APLICADO','RECHAZADO')),
  requested_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
-- RLS con in_scope; grants.
```

**Ajuste nuevo:** `inventory.adjust_approval_threshold` (Decimal), con valor por defecto `200000`.

### Reglas
`POST /v1/inventory/adjust` (permiso `inventory:adjust`) cambia su cuerpo a `{id, branch_id, location_id?, product_id, lot_id?, qty_delta, reason_code, notes?, unit_cost?}`.
1. Idempotencia por `id`.
2. **Coherencia del signo:**
   - `DANO`, `PERDIDA`, `ROBO` y `VENCIMIENTO` exigen `qty_delta < 0`;
   - `CARGA_INICIAL` exige `qty_delta > 0`, `on_hand = 0` en esa ubicación y ningún movimiento previo del producto en esa ubicación;
   - `CONTEO` y `CORRECCION` admiten cualquier signo.

   Si no se cumple: 422 `ADJUST_INVALID`.
3. `unit_cost`: en una entrada, el enviado; si no llega, `avg_cost`. En una salida, siempre `avg_cost`. `value = money(|qty_delta| × unit_cost)`.
4. Si `value > threshold` y el usuario no tiene `inventory:adjust_approve`: se guarda `PENDIENTE` **sin mover stock**. Si no: `APLICADO`, con `apply_delta_tx` en la misma transacción (`AJUSTE_ENTRADA` o `AJUSTE_SALIDA`, `ref_type = 'ADJUST'`).
5. Aprobar: `POST /v1/inventory/adjustments/{id}/approve` (permiso `inventory:adjust_approve`). Debe estar `PENDIENTE`; si no: 409 `ADJUST_NOT_PENDING`. Aplica el delta en ese momento y puede fallar con `STOCK_INSUFFICIENT`.
6. Rechazar: `POST /v1/inventory/adjustments/{id}/reject {reason}`.
7. `GET /v1/inventory/adjustments?branch_id=&status=`.
8. Auditoría: `AJUSTE_SOLICITADO`, `AJUSTE_APLICADO` y `AJUSTE_RECHAZADO`.

El endpoint `count` (conteo físico) que ya existe crea ajustes `CONTEO` con estas mismas reglas.

### Frontend
- El diálogo de ajuste actual agrega el selector de motivo y, solo en los productos con `track_lots`, el selector de lote.
- Pantalla `/inventario/ajustes` (registro `s("43", "ajustes", "Ajustes pendientes", "/inventario/ajustes", "inventario", "admin", "react")`), con pestañas por estado y botones "Aprobar" y "Rechazar" (solo con `inventory:adjust_approve`).

### Pruebas (`f2_adjustments`)
- **[P]** Un ajuste por encima del umbral pedido por un usuario sin permiso de aprobación queda `PENDIENTE` y el stock no cambia; al aprobarlo, cambia.
- **[P]** `CARGA_INICIAL` con stock existente: 422.
- **[P]** `ROBO` con una cantidad positiva: 422.

## F2.17 Sugerencia de compra

`GET /v1/purchases/suggestions?branch_id=` (permiso `purchases:create`):
1. Productos con `usage` `VENTA` o `INSUMO`, sin variantes en el padre, sin archivar, con `min_qty > 0` en alguna ubicación de la sucursal y con `on_hand` sumado de la sucursal `<= Σ min_qty`.
2. **Cantidad sugerida** en unidad de inventario:
   - si hay `reorder_qty`, esa;
   - si no, y hay `max_qty`, `max_qty − on_hand`;
   - si no, `2 × Σ min_qty − on_hand`.

   Luego se convierte a unidad de compra: `ceil(sugerida / purchase_factor)`, y se ajusta al `min_order_qty` del proveedor preferido si es mayor.
3. Se agrupan por proveedor preferido. Los productos sin preferido van en el grupo `supplier: null`.
4. Respuesta: `[{supplier: {id, name} | null, items: [{product_id, name, on_hand, min_qty, suggested_qty (en unidad de compra), purchase_uom, last_cost}]}]`.

`POST /v1/purchases/suggestions/create` (permiso `purchases:create`), con cuerpo `{branch_id, supplier_ids: [uuid]}`: crea una orden `PENDIENTE` por proveedor con sus sugerencias, al `last_cost`, o al `cost_price × factor` si no hay `last_cost`. Los ids de las órdenes son nuevos (`gen_random_uuid`). Devuelve los ids.

**Frontend:** en `/compras`, el botón "Sugerencias" abre una lista agrupada con casillas por proveedor y el botón "Crear órdenes".

**Prueba [P]** (`f2_suggestions`): un producto bajo el mínimo con un preferido sugiere `reorder_qty` convertida a unidad de compra.

---

## Códigos de error nuevos de la fase 2 (agrégalos a `ERROR_MESSAGES`)

| Código | Texto |
|---|---|
| `PRODUCT_INVALID` | Revisá los datos del producto. |
| `PRODUCT_NOT_SELLABLE` | Este producto es un insumo o de uso interno: no se vende. |
| `PRODUCT_IS_VARIANT_PARENT` | Elegí una variante del producto. |
| `PRODUCT_UOM_LOCKED` | No se puede cambiar la unidad: el producto ya tiene movimientos. |
| `VARIANTS_AND_PRESENTATIONS` | Un producto no puede tener variantes y presentaciones a la vez. |
| `SKU_TAKEN` | Ese SKU ya está en uso. |
| `BARCODE_SCIENTIFIC` | El código de barras está en notación científica. Revisá el archivo. |
| `BARCODE_EXISTS` | El producto ya tiene código de barras. |
| `OPTION_INVALID` | Revisá las opciones elegidas. |
| `OPTION_SELECTION_INVALID` | Falta elegir opciones obligatorias. |
| `PRODUCT_HAS_RESERVATIONS` | El producto tiene reservas activas. |
| `ARCHIVE_NEEDS_CONFIRM` | El producto tiene stock. Confirmá la baja. |
| `IMPORT_HAS_ERRORS` | Corregí los errores del archivo antes de importar. |
| `IMPORT_EXPIRED` | La importación venció. Subí el archivo otra vez. |
| `LOCATION_INVALID` | Revisá los datos de la bodega. |
| `LOCATION_HAS_STOCK` | La bodega tiene stock. Trasladalo antes de desactivarla. |
| `STOCK_DELTA_ZERO` | La cantidad no puede ser cero. |
| `STOCK_INSUFFICIENT` | No hay stock suficiente. |
| `NIT_DV_INVALID` | El dígito de verificación no corresponde al NIT. |
| `SUPPLIER_HAS_OPEN_DOCS` | El proveedor tiene órdenes o deudas abiertas. |
| `PURCHASE_INVALID` | Revisá la orden de compra. |
| `PURCHASE_INVALID_STATE` | La orden no admite esa acción en su estado actual. |
| `RECEIVE_EXCEEDS_PENDING` | No podés recibir más de lo pendiente. |
| `LOT_REQUIRED` | Este producto exige lote y vencimiento. |
| `LOT_EXPIRY_MISMATCH` | Ese lote ya existe con otra fecha de vencimiento. |
| `LOT_ONLY_EXPIRED` | Solo quedan lotes vencidos de este producto. |
| `PAYABLE_CLOSED` | La cuenta ya está pagada o castigada. |
| `PAYMENT_EXCEEDS_BALANCE` | El pago supera el saldo. |
| `PAYMENT_REFERENCE_REQUIRED` | Escribí la referencia del pago. |
| `PAYMENT_NEEDS_OPEN_SESSION` | Abrí un turno de caja para pagar en efectivo. |
| `EXPENSE_INVALID_STATE` | El gasto no admite esa acción en su estado actual. |
| `ADJUST_INVALID` | Revisá el motivo y la cantidad del ajuste. |
| `ADJUST_NOT_PENDING` | El ajuste ya fue decidido. |

## Lista de verificación de cada entrega
- [ ] Una sola migración por entrega, que corre limpia con `test.sh`.
- [ ] Todas las pruebas **[P]** de la entrega existen con el nombre de función indicado y pasan, más todas las anteriores.
- [ ] La salida de `grep -n "update stock" src/` (2A) está pegada en el reporte.
- [ ] `fmt`, `clippy` con `-D warnings`, `test`, `tsc` y `lint` en verde.
- [ ] El README tiene todos los endpoints.
- [ ] El modo demo funciona en todas las pantallas nuevas.
- [ ] No hay botones sin permiso ni funciones a medias.

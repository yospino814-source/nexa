# M21 Roles y permisos: matriz base, cierre ciego y costos ocultos

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorio | `nexa` (repo único: `backend/` y `frontend/`) |
| Rama | `feat/roles-permisos` |
| Depende de | M17 |
| Migración | Archivo **nuevo** `backend/supabase/migrations/20261120000100_roles_permisos.sql` (texto exacto en R1) |
| Commits | Uno por subagente, con el mensaje de la tabla "Reparto en subagentes" |
| Reporte | Sección "M21" de `antigravity/REPORTE-ROLES.md` (crealo) |
| Fuente de la decisión | `nexa/docs/roles-y-permisos.md`. Si ese documento y este módulo difieren, **manda este módulo**. |

## Prompt para Antigravity

```text
Vas a hacer el módulo M21 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

Contexto de rutas: todo vive en un repositorio único, `nexa/`, y las rutas de este módulo son relativas a su raíz: `backend/`, `frontend/`, `antigravity/`, `despliegue/`, `docs/`. Trabajá siempre con la terminal abierta en esa raíz.

Antes de empezar: git status en nexa/ debe estar limpio. Si no lo está, detenete y avisame; no commitees cambios que no sean tuyos. Creá la rama feat/roles-permisos desde la rama actual.

1. Leé completos antigravity/00-LEEME.md (reglas generales), antigravity/INFORME-SALIDA.md (§4), CLAUDE.md y antigravity/modulos/M21-roles-permisos.md.
2. BLOQUE 1: abrí 5 subagentes (M21.1 a M21.5) y lanzalos TODOS AL MISMO TIEMPO. No esperés a que uno termine para lanzar el siguiente. A cada uno dale: su tarea tal como está escrita en el módulo, su lista de "Archivos que le pertenecen" (sección "Reparto en subagentes") y la regla 0 del LEEME. Ningún subagente edita un archivo que no está en su lista; si lo necesita, se detiene y te avisa.
3. Cuando el bloque 1 termine, uní el trabajo, resolvé los choques y hacé un commit por subagente con el mensaje de la tabla.
4. BLOQUE 2: lanzá el subagente M21.6 (pruebas). Recién ahora, porque necesita el código del bloque 1 ya unido. Commit con el mensaje de la tabla.
5. Corré VOS la verificación del coordinador (sección "Verificación del coordinador"), con Docker Desktop encendido, después del último cambio.
6. Escribí la sección "M21" en antigravity/REPORTE-ROLES.md (crealo si no existe) como dice "Reporte".
7. NO hagas push, NO apliques migraciones a Supabase y NO leas ni edites .env. El push lo hace Pedro.
8. Detenete. No empieces M22. Escribime "M21 listo para revisión".
```

## Reparto en subagentes

Lo que no está escrito en este módulo está prohibido. Ningún subagente edita un archivo ajeno.

### Bloque 1 (los cinco a la vez)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M21.1 | R1 migración, R2 `base_allows`, ajuste `features.uses_credit` (backend) | `backend/supabase/migrations/20261120000100_roles_permisos.sql` (nuevo), `backend/src/tenancy.rs`, `backend/src/settings.rs` | `feat(roles): matriz de permisos base` |
| M21.2 | R3.1 cierre ciego completo (incluye la fuga de `CASH_INSUFFICIENT`) | `backend/src/cash.rs`, `backend/src/routes/ventas.rs` | `feat(caja): cierre ciego aplicado` |
| M21.3 | R3.2 costos y márgenes ocultos, escritura sin borrar el costo, CSV, importación | `backend/src/reports/**`, `backend/src/routes/reportes.rs`, `backend/src/sales_history.rs`, `backend/src/products.rs`, `backend/src/catalog.rs`, `backend/src/inventory.rs`, `backend/src/procurement.rs`, `backend/src/csv_import.rs`, `backend/src/routes/catalogo.rs`, `backend/src/routes/compras.rs`, `backend/src/routes/inventario.rs` | `feat(reportes): costos y margenes solo con permiso` |
| M21.4 | R4 tipos, API y mocks; pantallas que muestran costos, márgenes o el cierre de caja (R5.1 y R5.2) | `frontend/lib/api/types.ts`, `frontend/lib/api/index.ts`, `frontend/lib/api/mock*.ts`, `frontend/components/caja/**`, `frontend/components/ventas/**`, `frontend/components/panel/**`, `frontend/components/compras/**`, `frontend/components/inventario/**`, `frontend/components/gastos/**` | `feat(frontend): cierre ciego y costos ocultos` |
| M21.5 | R5.3 usuarios y roles, R5.4 fiado oculto | `frontend/components/configuracion/users-screen.tsx`, `frontend/components/configuracion/settings-screen.tsx`, `frontend/components/pos/checkout-dialog.tsx`, `frontend/components/pos/payment-methods.ts`, `frontend/components/pos/cart-panel.tsx`, `frontend/components/nexa/sidebar.tsx`, `frontend/lib/route-permissions.ts` | `feat(usuarios): roles visibles segun el negocio` |

Dependencias: M21.4 es dueño de `types.ts`, `index.ts` y los mocks. M21.5 usa `features.uses_credit`, que M21.4 declara exactamente como dice R5.4. Como arrancan juntos, `tsc` puede fallar hasta que 3.8 una ambos: es esperado, y lo verifica 3.8, no el subagente.

### Bloque 2 (después de unir el bloque 1)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M21.6 | Todas las pruebas [P] de R3 y arreglo de las pruebas viejas | `backend/src/db_tests.rs`, `backend/README.md` | `test(roles): pruebas de la matriz, cierre ciego y costos` |

`db_tests.rs` tiene unas 15.000 líneas y es de un solo dueño a propósito: dos subagentes editándolo a la vez lo dañarían.

## Decisiones que este módulo implementa

1. **El permiso de cada rol sale de la base** (`role_permissions` más `tenant_role_overrides`), leída por `permissions::load_effective_permissions_tx`. `tenancy::base_allows` es una copia que usan las pruebas: hay que mantener las dos iguales.
2. **Fiado fuera por ahora.** No se tocan permisos ni backend de crédito. Solo se oculta en el frontend (R5.4).
3. **Recibir mercancía** lo hace también el `CAJERO`.
4. **Costos y márgenes:** solo el Dueño ve el costo de ventas, el costo del producto (`cost_price`, `avg_cost`) y los márgenes. El costo de las líneas de una **compra** lo ve quien crea o aprueba compras (el Admin), porque es quien lo escribe. El Admin ve las ventas sin costos.
5. **Cierre ciego:** el cajero no ve el efectivo esperado ni la diferencia.
6. **Descuentos:** nadie aplica un descuento suelto con su propio permiso, salvo el Dueño. El resto de la historia (PIN y promociones) está en M22. En M21 solo se retira `pos:discount` a los demás roles.

## R1. Migración

Archivo nuevo `backend/supabase/migrations/20261120000100_roles_permisos.sql`. No edites migraciones viejas. Contenido exacto:

```sql
-- M21 Roles y permisos

insert into public.permissions (code, module, description) values
  ('costs:read', 'reports', 'Ver costos de compra y costo de ventas'),
  ('margins:read', 'reports', 'Ver márgenes y utilidad'),
  ('cash:see_expected', 'cash', 'Ver el efectivo esperado y la diferencia al cerrar caja')
on conflict (code) do update set description = excluded.description, module = excluded.module;

-- Permisos que se agregan
with rol as (
  select tima_id, tima_codigo from public.tbl_tipos_maestra
   where padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
)
insert into public.role_permissions (role_id, permission_code)
select rol.tima_id, v.code
  from rol
  join (values
    ('CAJERO',     'purchases:receive'),
    ('SUPERVISOR', 'purchases:receive'),
    ('SUPERVISOR', 'inventory:adjust_approve'),
    ('SUPERVISOR', 'cash:see_expected'),
    ('ADMIN_CAJA', 'cash:see_expected')
  ) v(rol, code) on v.rol = rol.tima_codigo
on conflict do nothing;

-- Permisos que se retiran
with rol as (
  select tima_id, tima_codigo from public.tbl_tipos_maestra
   where padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
)
delete from public.role_permissions rp
 using rol, (values
    ('CAJERO',     'pos:discount'),
    ('CAJERO',     'pos:custom_item'),
    ('SUPERVISOR', 'pos:discount'),
    ('ADMIN_CAJA', 'pos:discount'),
    ('ADMIN_CAJA', 'costs:read'),
    ('ADMIN_CAJA', 'margins:read'),
    ('BODEGA',     'inventory:adjust_approve')
 ) v(rol, code)
 where rp.role_id = rol.tima_id and rol.tima_codigo = v.rol and rp.permission_code = v.code;
```

`DUENO` no tiene filas: `Scope::has_permission` le da todo. No agregues filas para `DUENO`.

## R2. `tenancy.rs`

Reemplaza `base_allows` entera por esta. El cambio frente a hoy está en las líneas marcadas con `// M21`:

```rust
pub fn base_allows(role: &str, code: &str) -> bool {
    match role {
        "DUENO" => true,
        // M21: el Admin tiene todo menos costos, márgenes y descuento suelto.
        "ADMIN_CAJA" => !matches!(code, "costs:read" | "margins:read" | "pos:discount"),
        "CAJERO" => matches!(
            code,
            "inventory:read"
                | "pos:order"
                | "pos:sell"
                | "cash:movement"
                | "cash:open"
                | "cash:close"
                | "cash:history"
                | "customers:read"
                | "customers:write"
                | "credit:collect"
                | "credit:sell"
                | "purchases:receive" // M21
        ),
        "MESERO" => matches!(code, "inventory:read" | "pos:order" | "customers:read"),
        "COCINA" => matches!(code, "inventory:read" | "pos:kitchen"),
        "SUPERVISOR" => matches!(
            code,
            "inventory:read"
                | "pos:order"
                | "pos:sell"
                | "pos:custom_item"
                | "cash:movement"
                | "cash:open"
                | "cash:close"
                | "cash:history"
                | "cash:see_expected" // M21
                | "customers:read"
                | "customers:write"
                | "credit:collect"
                | "credit:sell"
                | "credit:override"
                | "pos:refund"
                | "inventory:adjust"
                | "inventory:adjust_approve" // M21
                | "reports:read"
                | "purchases:receive"
        ),
        "BODEGA" => matches!(
            code,
            "inventory:read" | "inventory:adjust" | "inventory:transfer" | "purchases:receive"
        ), // M21: sin inventory:adjust_approve
        "REPARTIDOR" => matches!(code, "inventory:read" | "pos:sell"),
        _ => false,
    }
}
```

`is_admin` no cambia. `ADMIN_CAJA` conserva `cash:see_expected`, que sale de la regla `!matches!`.

## R3. Backend

### R3.1 Cierre ciego (`cash.rs`)

Hoy `settings.cash.blind_close` (por defecto `true`) existe pero **ningún código lo aplica**.

1. Agrega a `CashSummary` el campo `pub expected_hidden: bool`.
2. Función nueva, sin tocar `summary_tx` (que sigue devolviendo todo, porque `record_movement_tx` y `close` lo usan por dentro):

```rust
/// Oculta al cajero lo que le permitiría ver el efectivo esperado.
pub fn mask_blind(mut s: CashSummary, scope: &Scope, blind_close: bool) -> CashSummary {
    if !blind_close || scope.has_permission("cash:see_expected") {
        s.expected_hidden = false;
        return s;
    }
    s.expected_hidden = true;
    s.expected_cash = Decimal::ZERO;
    s.cash_in = Decimal::ZERO;
    s.cash_out = Decimal::ZERO;
    s.by_method.retain(|m| m.method != "EFECTIVO");
    s.movements_by_kind.clear();
    s.session.expected_cash = None;
    s.session.difference = None;
    s
}
```

   Verificado en el código: el campo se llama `method` y el valor del efectivo es `"EFECTIVO"` (`cash.rs`, struct `MethodTotal`). `CashSession.expected_cash` y `difference` ya son `Option`.
3. Aplicalo, cargando `settings::load_tx(&mut tx, branch_id)` antes, en: `cash::current`, la respuesta de `cash::close` y toda ruta de `routes/ventas.rs` que devuelva un `CashSummary`.
4. En `cash::history` y `cash::activity`, cuando `!has_permission("cash:see_expected")` y `blind_close`: `expected_cash` y `difference` salen `null`. `counted_cash` se deja.
5. `cash::close` **no cambia su lógica**: sigue calculando contra el esperado real y exigiendo nota si la diferencia supera la tolerancia. El error de nota ya existente se reemplaza por uno con código:

   `error::coded(StatusCode::UNPROCESSABLE_ENTITY, "CASH_NOTES_REQUIRED", "Hay una diferencia en la caja. Escribí una nota y volvé a cerrar.", None)`

   Ese mensaje no dice cuánto es la diferencia.

6. **Fuga a cerrar:** `cash::record_movement_tx` devuelve 409 `CASH_INSUFFICIENT` con `data: {"available": <efectivo esperado>}`. Eso le muestra el esperado al cajero aunque el cierre sea ciego. Cuando el usuario no tiene `cash:see_expected` y `blind_close` está activo, el error se devuelve con `data: None`; el mensaje "Efectivo insuficiente en gaveta" se queda igual. Con `cash:see_expected` o con `blind_close = false`, no cambia.

Límite conocido, para el reporte: esto evita que el cajero lea el número del sistema; no impide que lleve su propia cuenta de las ventas, ni que averigüe el esperado probando egresos hasta que falle `CASH_INSUFFICIENT`.

### R3.2 Costos y márgenes ocultos

Regla: **costos y márgenes se devuelven solo con `costs:read` / `margins:read`**. Sin el permiso, los campos salen `null` y el CSV no lleva esas columnas.

1. Ubicalos: `grep -rnE "cost_of_sales|costo|margen|pct_margen|gross_margin|cost_total|unit_cost|avg_cost|cost_missing" src --include=*.rs | grep -v db_tests`. Cubrí al menos:
   - `reports/queries.rs`: ventas por día (`cost_of_sales`, `margen`), por producto, por categoría y por cajero (`costo`, `margen`, `pct_margen`, `cost_missing_lines`).
   - `reports` del dashboard (`get_dashboard`): utilidad y margen.
   - `sales_history.rs`: costo por línea de una venta.
   - respuestas de productos e inventario que traigan costo promedio o último costo (`products.rs`, `catalog.rs`, `inventory.rs`).
2. Campos de costo: requieren `costs:read`. Campos de margen, utilidad y porcentaje de margen: requieren `margins:read`. Pasan a `Option<Decimal>` y se serializan `null` cuando falta el permiso. `cost_missing_lines` se oculta junto con el costo.
3. Compras: `procurement.rs::get_purchase` y `list_purchases` devuelven los costos de las líneas a quien tenga `purchases:create` o `purchases:approve` (porque ellos los escriben). Quien solo tenga `purchases:receive` ve las líneas con cantidades y `unit_cost` en `null`.
4. `recibir` (`receive_purchase`) no pide costos al cajero: usa los de la orden.
5. Cada archivo CSV de reportes omite las columnas ocultas.
6. **Escritura sin borrar el costo.** Hoy `products.rs` (crear y editar), `inventory.rs` y las líneas de compra reciben `cost_price` / `unit_cost` en el cuerpo. Si el usuario no tiene `costs:read`, esos campos del cuerpo se **ignoran**: al editar, el costo guardado no cambia (nunca se escribe `null` encima); al crear, queda `null`. Sin esta regla, un Admin que edita un producto que no puede ver le borraría el costo.
7. **Importación CSV** (`csv_import.rs`): una columna de costo en el archivo solo se aplica si el usuario tiene `costs:read`; si no, se ignora y se avisa en el resumen de la importación.

### R3.3 Tarifa manual y `pos:discount`

`sales.rs` (alrededor de la línea 812) exige `pos:discount` para elegir una tarifa (lista de precios) que no es la del cliente de la venta. Al quitarle `pos:discount` al `CAJERO`, el cajero solo podrá usar la tarifa del cliente. Es lo que se quiere. No cambies ese código en M21; M22 lo cubre con el PIN del Dueño. Dejá una prueba que fije el comportamiento.

### Pruebas de R3 (`m21_roles`)

- **[P]** Matriz: `base_allows` para cada rol da exactamente lo de R2, y coincide con las filas de `role_permissions` tras la migración (una prueba de base de datos que compara los dos, para `ADMIN_CAJA`, `CAJERO`, `SUPERVISOR`, `BODEGA`).
- **[P]** `CAJERO` recibe una compra aprobada: 200. `CAJERO` crea una compra: 403.
- **[P]** `CAJERO` vende con `discount_pct` > 0 sin override: 403. Con override `pos:discount = true`: 200.
- **[P]** `CAJERO` vende un ítem libre: 403. Con override `pos:custom_item = true`: 200.
- **[P]** `BODEGA` aprueba un ajuste de inventario: 403. `SUPERVISOR` lo aprueba: 200.
- **[P]** `ADMIN_CAJA`: `has_permission("costs:read")` es falso. `DUENO`: verdadero.
- **[P]** Ventas por día con `ADMIN_CAJA`: `ventas` con valor, `costo` y `margen` en `null`. Con `DUENO`: los tres con valor.
- **[P]** Ventas por producto en CSV con `ADMIN_CAJA`: sin las columnas de costo y margen.
- **[P]** `get_purchase` con `CAJERO` (solo recibe): `unit_cost` `null`. Con `ADMIN_CAJA`: con valor.
- **[P]** Cierre ciego con `blind_close = true`: `CAJERO` en `current` ve `expected_hidden = true`, `expected_cash = 0` y sin la fila de efectivo en `by_method`; `SUPERVISOR` ve todo.
- **[P]** Cierre ciego con `blind_close = false`: `CAJERO` ve todo.
- **[P]** `CAJERO` con cierre ciego registra un egreso mayor al efectivo: 409 `CASH_INSUFFICIENT` y el cuerpo no trae `available`. `SUPERVISOR`: sí lo trae.
- **[P]** `ADMIN_CAJA` edita un producto con `cost_price` en el cuerpo: el costo guardado no cambia. `DUENO`: sí cambia.
- **[P]** `CAJERO` vende con una tarifa que no es la de su cliente: 403.
- **[P]** La prueba de coincidencia entre `base_allows` y `role_permissions` falla por un permiso que no es de M21: no cambies el rol; corregí `base_allows` para que copie la base y anotalo en el reporte.
- **[P]** `CAJERO` cierra con una diferencia mayor a la tolerancia y sin nota: 422 `CASH_NOTES_REQUIRED`. Con nota: 200 y `session.difference = null` en la respuesta.
- Las pruebas viejas que dependían de que `CAJERO` aplicara descuentos o ítems libres sin override se arreglan dándoles el override con `set_cashier_override`. Listalas en el reporte. **No borres ninguna.**

## R4. API y mock del frontend

1. `lib/api/types.ts`: `CashSummary` gana `expected_hidden: boolean`. Los campos de costo y margen de los reportes, productos y compras pasan a `string | null`.
2. `lib/api/index.ts` y los mocks: el mock del cajero devuelve `expected_hidden: true` y sin efectivo en `by_method`; el del dueño, todo. El mock del Admin devuelve costos y márgenes `null`.
3. Fila nueva en `backend/README.md` para `cash:see_expected`, `costs:read` y `margins:read`, en la tabla de permisos.

## R5. Pantallas

1. **Cierre de caja** (`app/(operacion)/caja`; ubicá la pantalla con `grep -rn "expected_cash" frontend --include=*.tsx`): si `expected_hidden`, no muestra el esperado ni la diferencia. Muestra "Contá el efectivo de la caja y escribí el total." y, al cerrar con `CASH_NOTES_REQUIRED`, el mensaje del error con un campo de nota.
2. **Reportes y panel:** una columna o tarjeta con valor `null` no se muestra (no se pinta "0" ni "—"). Si no queda ninguna columna de costo, desaparece también su encabezado.
3. **Usuarios y roles** (`components/configuracion/users-screen.tsx`, modal "Invitar usuario"):
   - Lista visible, con estas descripciones exactas:
     - Dueño: "Ve todo y configura el negocio". Solo lo puede asignar un Dueño.
     - Administrador de caja: "Encargado: abre y cierra caja, ve las ventas de su sucursal"
     - Cajero: "Vende, cobra y recibe mercancía"
     - Supervisor: "Anula ventas, aprueba ajustes y propone promociones"
   - Bajo un botón "Más roles" (cerrado por defecto): Bodega, "Recibe compras y ajusta inventario, no maneja dinero".
   - **No se muestran** Mesero, Cocina ni Repartidor (todavía no tienen pantallas; ver `session-gate.tsx`).
   - Las tarjetas de la pestaña "Roles" de solo lectura se actualizan a la matriz de R2. Para `CAJERO`: "Vender en mostrador", "Cobrar", "Recibir mercancía", "Abrir y cerrar su caja".
4. **Fiado oculto:** agrega a los ajustes de sucursal `features.uses_credit: boolean` (por defecto `false`) en `backend/src/settings.rs` (`FeaturesSettings`, dentro de `BranchSettings`, con `#[serde(default)]`) y en `lib/api/types.ts`. Con `false`, el frontend oculta la opción de pago a crédito del POS, el ítem "Cartera" del menú y la ruta `/cartera` (entra en `route-permissions.ts` con una condición extra, no con un permiso). El backend **no** valida la bandera. En la pantalla de ajustes, un interruptor "Vender fiado" apagado por defecto.

## Lo que está prohibido en este módulo

- Aplicar migraciones a Supabase.
- Quitar `credit:*` del código o de la base.
- Mesas, cocina, domicilios, cuentas abiertas: son fases 5 y 6 o M23.
- PIN, aprobación de descuentos y fecha de fin de promociones: son M22.
- Cambiar la lógica de `close` salvo lo escrito en R3.1.

## Verificación del coordinador

Esto lo corre el modelo 3.8, no los subagentes, **después de unir todo y del último cambio**. Si algo falla, la corrección vuelve al subagente dueño del archivo.

```sh
cd backend && ./scripts/test.sh
cd ../frontend && npx tsc --noEmit && npx eslint . && npx next build
cd .. && git grep -n 'Ã'    # vacía
cd backend && grep -n '"DUENO" | "ADMIN_CAJA" => true' src/tenancy.rs    # vacía
```

## Reporte

En `antigravity/REPORTE-ROLES.md`, sección `## M21`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc`, `eslint` y `next build`.
3. La salida de los `grep` de la verificación.
4. Una tabla con cada regla [P] y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. La lista de pruebas viejas que hubo que ajustar, con el archivo y la línea.
6. Los lugares donde encontraste costos o márgenes que este módulo no cubre. Escribí "no lo probé" antes que "funciona".

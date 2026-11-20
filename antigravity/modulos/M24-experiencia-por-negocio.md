# M24 Experiencia por negocio: tipo de negocio, funciones que se encienden y apagan, pantallas que se encuentran y menú en el celular

| Dato | Valor |
|---|---|
| Fase | Roles |
| Repositorio | `nexa` (repo único: `backend/` y `frontend/`) |
| Rama | `feat/ux-por-negocio`, creada desde la rama donde quedó aprobado M23 (`feat/roles-permisos`) |
| Depende de | **M21, M22 y M23 aprobados por Claude.** M21 crea `FeaturesSettings` con `uses_credit`, y M23 le agrega `uses_tables`. Entre los dos tocan `sidebar.tsx`, `route-permissions.ts`, `session.ts`, `types.ts`, `settings.rs`, `settings-screen.tsx` y `users-screen.tsx`, que este módulo también edita: si M23 no está unido, **detenete**. |
| Migración | **Ninguna.** Los ajustes viven en `branch_settings.data` (jsonb). No crees archivos en `supabase/migrations/`. |
| Commits | Uno por subagente, con el mensaje de la tabla "Reparto en subagentes" |
| Reporte | Sección "M24" de `antigravity/REPORTE-ROLES.md` |
| Fuente de la decisión | `nexa/docs/roles-y-permisos.md` ("interruptores del negocio, no un tipo rígido") y la exploración del 2026-11-20 |

## Por qué existe

Hoy un dueño de tienda, de licorera o de bodega ve la misma app. El "tipo de negocio" no existe en ningún lado, el onboarding solo pide el nombre, y varias pantallas útiles (Ajustes de inventario, Tarifas, Bodegas, Cupones) no tienen un solo botón que lleve a ellas. En el celular el menú lateral desaparece y no hay reemplazo.

**Decisión de diseño:** el tipo de negocio elegido en el alta solo **preselecciona** unos interruptores. Lo que manda son los interruptores, que el dueño puede cambiar cuando quiera. El tipo no bloquea nada.

## Prompt para Antigravity

```text
Vas a hacer el módulo M24 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

Contexto de rutas: todo vive en un repositorio único, `nexa/`, y las rutas de este módulo son relativas a su raíz: `backend/`, `frontend/`, `antigravity/`, `despliegue/`, `docs/`. Trabajá siempre con la terminal abierta en esa raíz.

Antes de empezar:
- M21, M22 y M23 tienen que estar aprobados y unidos en la rama actual. Comprobalo: en backend/src/settings.rs debe existir la struct FeaturesSettings con los campos uses_credit y uses_tables. Si falta alguno, detenete y avisame.
- git status en nexa/ debe estar limpio. Si no lo está, detenete y avisame; no commitees cambios que no sean tuyos.
- git grep -n 'Ã' en nexa/ solo puede devolver CLAUDE.md y archivos .png. Si devuelve algo en frontend/lib/api/index.ts o frontend/lib/api/mock-platform.ts, detenete y avisame: tienen la codificación dañada.
- Creá la rama feat/ux-por-negocio desde la rama actual.

1. Leé completos antigravity/00-LEEME.md (reglas generales), CLAUDE.md, frontend/AGENTS.md y antigravity/modulos/M24-experiencia-por-negocio.md. Antes de usar cualquier API de Next, leé la guía en frontend/node_modules/next/dist/docs/.
2. BLOQUE 1: abrí 5 subagentes (M24.1 a M24.5) y lanzalos TODOS AL MISMO TIEMPO. No esperés a que uno termine para lanzar el siguiente. A cada uno dale: su tarea tal como está escrita en el módulo, su lista de "Archivos que le pertenecen" (sección "Reparto en subagentes") y la regla 0 del LEEME. Ningún subagente edita un archivo que no está en su lista; si lo necesita, se detiene y te avisa.
3. Cuando el bloque 1 termine, uní el trabajo, resolvé los choques y hacé un commit por subagente con el mensaje de la tabla.
4. BLOQUE 2: lanzá el subagente M24.6 (pruebas). Recién ahora, porque necesita el código del bloque 1 ya unido. Commit con el mensaje de la tabla.
5. Corré VOS la verificación del coordinador (sección "Verificación del coordinador"), con Docker Desktop encendido, después del último cambio.
6. Escribí la sección "M24" en antigravity/REPORTE-ROLES.md como dice "Reporte".
7. NO hagas push, NO apliques migraciones a Supabase (este módulo no tiene ninguna) y NO leas ni edites .env. El push lo hace Pedro.
8. Detenete. No empieces M23. Escribime "M24 listo para revisión".
```

## Reparto en subagentes

Lo que no está escrito en este módulo está prohibido. Ningún subagente edita un archivo ajeno.

### Bloque 1 (los cinco a la vez)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M24.1 | B1 a B4: tipo de negocio, interruptores y alta (backend) | `backend/src/settings.rs`, `backend/src/onboarding.rs`, `backend/README.md` | `feat(negocio): tipo de negocio e interruptores en el backend` |
| M24.2 | F1 tipos, API y mocks; F2 sesión; F3 guardia de rutas | `frontend/lib/api/types.ts`, `frontend/lib/api/index.ts`, `frontend/lib/api/mock-onboarding.ts`, `frontend/lib/api/mock-platform.ts`, `frontend/lib/stores/session.ts`, `frontend/lib/route-permissions.ts` | `feat(frontend): interruptores del negocio en la sesión` |
| M24.3 | F4 alta con tipo de negocio; F5 interruptores en Ajustes y enlaces a Tarifas y Bodegas; F6 campos de licor | `frontend/components/auth/onboarding-screen.tsx`, `frontend/components/configuracion/settings-screen.tsx`, `frontend/components/inventario/product-form-dialog.tsx` | `feat(frontend): elegir el tipo de negocio y encender funciones` |
| M24.4 | F7 enlaces a pantallas escondidas; F8 tarjeta "Pendientes" del panel | `frontend/components/inventario/inventory-screen.tsx`, `frontend/components/marketing/promotions-screen.tsx`, `frontend/components/marketing/coupons-screen.tsx`, `frontend/components/panel/panel-screen.tsx`, `frontend/components/panel/pending-card.tsx` (nuevo) | `feat(frontend): enlaces a pantallas escondidas y pendientes del día` |
| M24.5 | F9 menú inferior en el celular | `frontend/lib/nav-items.ts` (nuevo), `frontend/components/nexa/bottom-nav.tsx` (nuevo), `frontend/components/nexa/sidebar.tsx`, `frontend/app/(admin)/layout.tsx` | `feat(frontend): menú inferior para el celular` |

Dependencias: M24.2 es dueño de `types.ts`, `index.ts`, los mocks y `session.ts`, y declara **exactamente** lo que dicen F1 y F2. M24.3, M24.4 y M24.5 lo usan. Como arrancan juntos, `tsc` puede fallar hasta que 3.8 una todo: es esperado, y lo verifica 3.8, no el subagente.

### Bloque 2 (después de unir el bloque 1)

| Subagente | Tarea | Archivos que le pertenecen | Commit |
|---|---|---|---|
| M24.6 | Todas las pruebas [P] de B y de las funciones puras | `backend/src/db_tests.rs`, y las pruebas unitarias dentro de `backend/src/settings.rs` (solo el bloque `#[cfg(test)]`) | `test(negocio): pruebas del tipo de negocio y los interruptores` |

`db_tests.rs` es de un solo dueño a propósito.

## Decisiones que este módulo implementa

1. **Sin migración.** El tipo de negocio y los interruptores se guardan dentro de `BranchSettings.features` (jsonb en `branch_settings.data`), junto a `uses_credit` de M21.
2. **Valores por defecto que no esconden nada a los negocios que ya existen.** Un negocio sin `features` guardado queda con `uses_warehouses = true` y `uses_alcohol = true` (se sigue viendo todo lo que se veía) y `uses_credit = false` (decisión de M21). Solo los negocios **nuevos** reciben la preselección del tipo.
3. **El tipo no bloquea nada.** Cambiar un interruptor no cambia el tipo, y el tipo solo se lee en el alta.
4. **Los interruptores son por sucursal**, igual que `uses_credit` en M21. Una sucursal creada después **no hereda** los interruptores de la primera: arranca con los valores por defecto del punto 2. Es un límite conocido; anotalo en el reporte, no lo resuelvas.
5. **`uses_tables` es de M23 y no se toca.** M24 lo conserva tal como M23 lo dejó (apagado por defecto, en cualquier tipo de negocio) y **no lo preselecciona según el tipo**: ni siquiera Restaurante lo enciende solo, porque el alta no configura mesas. Cocina no tiene interruptor: no hay pantallas (fases 5 y 6 pospuestas), y un interruptor que no enciende nada está prohibido (regla 6 del LEEME).
6. **Restaurante existe como tipo, sin pantallas propias.** Se puede elegir en el alta; se comporta como "Otro" y el alta lo avisa con un texto (F4).

## B. Backend

### B1. Tipos y valores

En `backend/src/settings.rs`, junto a `FeaturesSettings` (creada por M21; M23 le agregó `uses_tables`). Reemplazá la struct por esta, **conservando `uses_credit` y `uses_tables` exactamente como las dejaron M21 y M23** (si sus atributos `serde` difieren de los de abajo, mandan los de ellos):

```rust
pub const BUSINESS_TYPES: [&str; 5] = ["TIENDA", "LICORERA", "BODEGA", "RESTAURANTE", "OTRO"];

fn default_business_type() -> String {
    "OTRO".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeaturesSettings {
    #[serde(default = "default_false")]
    pub uses_credit: bool,
    #[serde(default = "default_false")]
    pub uses_tables: bool,
    #[serde(default = "default_business_type")]
    pub business_type: String,
    #[serde(default = "default_true")]
    pub uses_warehouses: bool,
    #[serde(default = "default_true")]
    pub uses_alcohol: bool,
}

impl Default for FeaturesSettings {
    fn default() -> Self {
        Self {
            uses_credit: false,
            uses_tables: false,
            business_type: default_business_type(),
            uses_warehouses: true,
            uses_alcohol: true,
        }
    }
}

/// Interruptores que se preseleccionan al crear un negocio nuevo, según su tipo.
/// Es lo único que hace el tipo: después manda cada interruptor.
pub fn features_for(business_type: &str) -> FeaturesSettings {
    let (uses_warehouses, uses_alcohol) = match business_type {
        "TIENDA" | "LICORERA" => (false, true),
        "BODEGA" => (true, false),
        _ => (false, false), // RESTAURANTE y OTRO
    };
    FeaturesSettings {
        uses_credit: false,
        uses_tables: false,
        business_type: business_type.to_string(),
        uses_warehouses,
        uses_alcohol,
    }
}
```

Si M21 declaró `FeaturesSettings` con `#[derive(Default)]`, quitá ese derive: el `Default` manual de arriba es obligatorio, porque dos campos valen `true` por defecto. `BranchSettings` ya tiene `features` con `#[serde(default)]` desde M21; no lo cambies.

### B2. Validación al guardar ajustes

En `update_settings` (`settings.rs`), con las demás validaciones y **antes** de abrir la transacción, agregá:

```rust
if !BUSINESS_TYPES.contains(&req.settings.features.business_type.as_str()) {
    return Err(validation("features.business_type is not valid"));
}
```

### B3. Alta del negocio (`onboarding.rs`)

1. `CreateBusinessRequest` gana `#[serde(default)] pub business_type: Option<String>`. Un cliente viejo que no lo manda queda como `"OTRO"`.
2. Validaciones, **en este orden**: `business_name`, `branch_name`, `full_name` (como hoy) y después `business_type`. Si viene y no está en `BUSINESS_TYPES` (se compara después de `trim` y sin cambiar mayúsculas): 422 `ONBOARDING_INVALID` con `data: {"field": "business_type"}` y el mismo texto de siempre ("Revisá los datos del negocio.").
3. Justo después de `crate::db::set_scope(&mut tx, tenant, Some(branch))` y **antes** de calcular `effective_permissions`, guardá los interruptores del tipo:

```rust
let features = crate::settings::features_for(&business_type);
let initial = crate::settings::BranchSettings {
    features,
    ..Default::default()
};
let initial_json = serde_json::to_value(&initial).map_err(anyhow::Error::from)?;
sqlx::query(
    "insert into branch_settings (branch_id, tenant_id, data, updated_by, updated_at)
     values ($1, $2, $3, $4, now())",
)
.bind(branch)
.bind(tenant)
.bind(&initial_json)
.bind(claims.sub)
.execute(&mut *tx)
.await?;
```

   Es una sola transacción: si el insert falla, el negocio no se crea. Comprobá que la política RLS de `branch_settings` (`branch_settings_isolation`, migración `20261102000200_platform_foundations.sql`) deja insertar al Dueño recién creado; si no, **detenete** y anotalo en "Preguntas abiertas": no cambies la política.
4. La auditoría `NEGOCIO_CREADO` agrega `"business_type": business_type` al `after`.
5. **Sin migración y sin tocar `public.create_business`.**

### B4. README

`backend/README.md`: en la fila de `POST /v1/onboarding`, agregá el campo `business_type` (opcional; valores `TIENDA`, `LICORERA`, `BODEGA`, `RESTAURANTE`, `OTRO`; por defecto `OTRO`). En la fila de `PUT /v1/settings`, anotá `features.business_type`, `features.uses_warehouses` y `features.uses_alcohol`.

## F. Frontend

### F1. Tipos, API y mocks (M24.2)

1. `lib/api/types.ts`:
   - `export type BusinessType = "TIENDA" | "LICORERA" | "BODEGA" | "RESTAURANTE" | "OTRO";`
   - El tipo de `features` dentro de `BranchSettings` (declarado por M21 con `uses_credit` y ampliado por M23 con `uses_tables`) conserva esos campos y gana tres: `business_type: BusinessType; uses_warehouses: boolean; uses_alcohol: boolean`. Sigue siendo obligatorio.
   - `CreateBusinessInput` gana `business_type?: BusinessType;`.
2. `lib/api/mock-platform.ts`: el ajuste mock de la sucursal Centro (`mockBranchSettings[BRANCH_CENTRO]`) conserva el `features` que dejaron M21 y M23 (`uses_credit`, `uses_tables`) y le suma `business_type: "TIENDA", uses_warehouses: true, uses_alcohol: true`. En el demo se ve todo.
3. `lib/api/mock-onboarding.ts`: `mockCreateBusiness` valida `business_type` igual que el backend (si viene y no es uno de los cinco: `ApiError(422, "Revisá los datos del negocio.", "ONBOARDING_INVALID", { field: "business_type" })`), después de las tres validaciones que ya tiene. No guarda nada más.
4. `lib/api/index.ts`: no hay endpoints nuevos. Solo agregá el tipo importado si hace falta.

### F2. Sesión (`lib/stores/session.ts`, M24.2)

1. Exportá:

```ts
export type FeatureCode = "uses_credit" | "uses_warehouses" | "uses_alcohol";

/** Lo que se asume mientras cargan los ajustes o si fallan: no esconde nada, salvo el fiado. */
export const DEFAULT_FEATURES = {
  uses_credit: false,
  uses_warehouses: true,
  uses_alcohol: true,
} as const;
```

2. `SessionState` gana `features: Record<FeatureCode, boolean>`, `feature: (code: FeatureCode) => boolean` y `loadFeatures: () => Promise<void>`. Valor inicial de `features`: `{ ...DEFAULT_FEATURES }`.
3. `feature(code)` devuelve `get().features[code]`.
4. `loadFeatures()`: si no hay `branchId`, no hace nada. Si hay, llama `api.settings.get(branchId)` y guarda `{ uses_credit, uses_warehouses, uses_alcohol }` de `settings.features`. Si la llamada falla, deja `DEFAULT_FEATURES` (no muestra error: es un ajuste de comodidad).
5. Llamá `loadFeatures()` en los dos lugares donde hoy se hace `api.session.context()` para cargar permisos: dentro de `selectBranch` y dentro de `bootOnce` (cuando hay `branchId`). Cada una sin `await` bloqueante del estado `ready`: la sesión queda lista igual y los interruptores llegan un instante después.
6. `signOut` restablece `features` a `{ ...DEFAULT_FEATURES }`.
7. `api.settings.get` exige el permiso `inventory:read`, que tienen todos los roles. No agregues otra llamada.

### F3. Guardia de rutas (`lib/route-permissions.ts`, M24.2)

M21 (R5.4) ya agregó una condición extra en `canOpen` para `/cartera` según `uses_credit`. **Usá el mismo mecanismo** y agregá una regla igual para `/configuracion/bodegas`, que se cierra cuando `uses_warehouses` es `false`. No cambies la firma de `canOpen` si M21 no lo hizo. Si M21 resolvió la condición leyendo `useSession.getState().features`, hacé lo mismo con `features.uses_warehouses`. Si no encontrás ese mecanismo, **detenete** y anotalo.

### F4. Alta de negocio (`components/auth/onboarding-screen.tsx`, M24.3)

1. Estado nuevo: `businessType` (tipo `BusinessType`), valor inicial `"TIENDA"`; y `businessTypeError` (`string | null`).
2. Entre "Nombre del negocio" y "Nombre de la sucursal", un campo "¿Qué tipo de negocio es?" con cinco tarjetas seleccionables (botones con `role="radio"` dentro de un contenedor `role="radiogroup"` y `aria-label="Tipo de negocio"`), en una grilla de 2 columnas en pantallas chicas y 3 desde `sm`. Cada tarjeta lleva ícono de `lucide-react`, título y una línea:

| Valor | Ícono | Título | Línea |
|---|---|---|---|
| `TIENDA` | `Store` | Tienda | Productos de uso diario |
| `LICORERA` | `Wine` | Licorera | Licores, cervezas y cigarrillos |
| `BODEGA` | `Warehouse` | Bodega | Stock en varios lugares |
| `RESTAURANTE` | `UtensilsCrossed` | Restaurante | Comida y bebidas |
| `OTRO` | `Boxes` | Otro | Otro tipo de negocio |

3. La tarjeta elegida lleva borde `border-brand`, fondo `bg-brand-soft` y `aria-checked="true"`; las demás, `border-slate-200`. Usá las clases de color que ya existen en el proyecto; no definas colores nuevos.
4. Si `businessType === "RESTAURANTE"`, debajo de la grilla un texto `text-xs text-slate-500`: "Las mesas y la cocina llegan más adelante. Por ahora vendés en mostrador."
5. Debajo de la grilla, siempre: "Podés cambiar esto cuando quieras en Ajustes."
6. `handleSubmit`: agrega `business_type: businessType` a `api.onboarding.create`, y limpia `businessTypeError` junto a los otros errores. En el manejo de `ONBOARDING_INVALID`, si `field === "business_type"`, `setBusinessTypeError(msg)` y se muestra bajo la grilla, con la clase de los otros errores de campo.
7. No cambies la pantalla de éxito ("Tu negocio está listo") ni sus 4 enlaces.

### F5. Ajustes (`components/configuracion/settings-screen.tsx`, M24.3)

1. **Interruptores.** M21 (R5.4) agregó "Vender fiado" y M23 (C8) agregó "Usar mesas" con su tarjeta "Mesas". Agregá los dos nuevos **justo debajo del último interruptor de esos dos** (antes de la tarjeta "Mesas" si M23 la puso suelta, no dentro de ella), con el mismo markup (tarjeta `rounded-2xl border border-slate-100 bg-slate-50 p-5`, `label` con título y descripción, `checkbox` a la derecha), dos interruptores nuevos, que escriben en `formSettings.features`:

| Interruptor | Campo | Título | Descripción |
|---|---|---|---|
| 1 | `uses_warehouses` | Usar varias bodegas | Muestra las bodegas y sus traslados. Apagado: tu negocio tiene un solo lugar de stock. |
| 2 | `uses_alcohol` | Vender licores | Muestra los grados de alcohol y el volumen en los productos. |

   Al guardar, si la llamada a `api.settings.update` sale bien, llamá `useSession.getState().loadFeatures()` para que el menú y las pantallas cambien sin recargar. Si `branchId` de la sucursal que se está editando no es la activa de la sesión, no recargues.
2. **Enlaces de la cabecera.** En el bloque "Shortcuts" (la fila de botones tipo píldora), agregá, con las mismas clases que los demás y **después** de "Cajas registradoras":
   - `Tarifas y presentaciones` → `/configuracion/tarifas`, ícono `Tags`, visible si `can("settings:write")`.
   - `Bodegas` → `/configuracion/bodegas`, ícono `Warehouse`, visible si `can("settings:write")` **y** `feature("uses_warehouses")`.
3. No toques las pestañas ni el resto.

### F6. Campos de licor (`components/inventario/product-form-dialog.tsx`, M24.3)

Ubicá el bloque de grados de alcohol y volumen: `grep -n "alcohol_degrees\|setAlcoholDegrees\|setVolumeMl" components/inventario/product-form-dialog.tsx`. El bloque de **campos del formulario** (los `<input>` y sus etiquetas, hacia las líneas 399 a 431) se muestra solo si `feature("uses_alcohol")` **o** el producto que se edita ya trae `alcohol_degrees`. Así, apagar el interruptor no esconde el dato de un producto que ya lo tiene. No cambies la lógica que arma el cuerpo del envío (líneas 138 a 140): con el interruptor apagado y sin grados, ya manda `null`.

### F7. Enlaces a pantallas escondidas (M24.4)

Todos usan las clases del botón tipo píldora que ya tiene cada pantalla.

1. `components/inventario/inventory-screen.tsx`: entre "Kardex de movimientos" y "Importar CSV" (líneas ~197 a 208), un `Link` "Ajustes de inventario" → `/inventario/ajustes`, ícono `ClipboardCheck`, visible si `can("inventory:adjust")`.
2. `components/marketing/promotions-screen.tsx`: en la cabecera, junto al título, un `Link` "Cupones" → `/marketing/cupones`, ícono `Ticket`.
3. `components/marketing/coupons-screen.tsx`: en la cabecera, un `Link` "Promociones" → `/marketing/promociones`, ícono `Sparkles`.
4. No tocar `sales-screen.tsx`: su enlace a devoluciones (línea 113) se queda. El panel agrega otra entrada en F8.

### F8. Tarjeta "Pendientes" del panel (M24.4)

1. Archivo nuevo `components/panel/pending-card.tsx`, componente cliente `PendingCard({ lowStockCount }: { lowStockCount: number })`.
2. Filas (cada una es un `Link` con un círculo con el número a la izquierda, el texto y `ChevronRight`):

| Fila | Se muestra si | Cuenta | Texto | Destino |
|---|---|---|---|---|
| Devoluciones | `can("pos:refund")` | `api.refunds.list(branchId, "PENDIENTE")`, su `length` | "Devoluciones por aprobar" | `/ordenes/devoluciones` |
| Ajustes | `can("inventory:adjust_approve")` | La lista de ajustes con `status: "PENDIENTE"` y `branch_id` de la sucursal activa, su `length`. Usá la **misma llamada** que ya usa la pantalla `/inventario/ajustes` (buscala con `grep -rn "adjustments.list" components`) | "Ajustes de inventario por aprobar" | `/inventario/ajustes` |
| Stock | `lowStockCount > 0` | `lowStockCount` | "Productos con stock bajo" | `/inventario` |

3. Cada llamada se hace con `useApi` y su `key` incluye `branchId`. Una fila no se muestra si su conteo es 0, si la llamada falla o mientras carga (sin parpadeos de "0").
4. Si ninguna fila se muestra y ninguna llamada sigue cargando: la tarjeta muestra "Todo al día." con `text-sm text-slate-500`. Mientras carga la primera vez, no se dibuja la tarjeta.
5. Aspecto: la misma tarjeta que "Accesos rápidos" (`rounded-2xl border border-slate-200/60 bg-white p-6 shadow-sm`), con el título "Pendientes de hoy" (`text-lg font-semibold`).
6. `components/panel/panel-screen.tsx`: agregá `<PendingCard lowStockCount={data?.low_stock.length ?? 0} />` **justo antes** de la `<section>` "Accesos rápidos" (hacia la línea 570). No quites "Accesos rápidos" ni nada más del panel.

### F9. Menú inferior en el celular (M24.5)

1. **`lib/nav-items.ts` (nuevo).** Mové ahí, **tal como están hoy en `sidebar.tsx`**, el tipo `NavItem` y el arreglo `navItems` (con sus íconos importados), y exportalos. Si M21 cambió el arreglo (por ejemplo, por Cartera), movés el arreglo vigente, no el de este documento. `sidebar.tsx` los importa de ahí y no cambia nada más (el `NavLink`, el filtro `canOpen`, el aspecto y la lógica de "Salir" se quedan igual).
2. **`components/nexa/bottom-nav.tsx` (nuevo), componente cliente `BottomNav`:**
   - Visible solo por debajo de `md`: `md:hidden`.
   - Calcula `visible = role === null ? [] : navItems.filter((item) => canOpen(item.href, can))`, con el mismo `role` que usa `Sidebar` (el de la sucursal activa).
   - Barra fija: `fixed inset-x-0 bottom-0 z-30 flex h-16 border-t border-slate-200 bg-white`. Contiene hasta **4** ítems de `visible` (los primeros) y un quinto botón "Más".
   - Cada ítem: `Link` de columna (ícono `size-5` + etiqueta `text-[10px]`), `flex-1`, activo con `text-brand` y `aria-current="page"`; inactivo `text-slate-500`. La lógica de "activo" es la misma de `Sidebar` (`match` o `href`).
   - "Más" es un `button` con ícono `Menu`, etiqueta "Más", `aria-expanded` y `aria-label="Más opciones"`. Abre un panel `fixed inset-x-0 bottom-16 z-30 max-h-[70vh] overflow-y-auto rounded-t-2xl border-t border-slate-200 bg-white p-4 shadow-lg`, con un fondo tocable `fixed inset-0 z-20 bg-slate-900/30` que lo cierra.
   - El panel muestra, en este orden: los ítems de `visible` que no entraron en la barra (como filas con ícono y nombre), el `BranchSwitcher` (importado de `@/components/auth/user-menu`, sin `className` que lo oculte), el nombre del usuario y el botón "Salir", que hace lo mismo que el de `Sidebar` (`await signOut(); router.replace("/login")`).
   - Se cierra solo cuando cambia `pathname` (un `useEffect` sobre `pathname`).
   - **No uses el componente `Dialog`:** es un panel simple con `useState`.
   - Si `visible` está vacío, solo se muestra el botón "Más".
3. **`app/(admin)/layout.tsx`:** renderizá `<BottomNav />` después del `<main>`, y agregá `pb-16 md:pb-0` a la clase del `<main>` para que el menú no tape el contenido. No toques `SessionGate`, `RouteGuard` ni `export const instant = false`.
4. El layout de operación (`/pos`, `/caja`) **no se toca**.

## Lo que está prohibido en este módulo

- Crear migraciones o tocar `public.create_business`.
- Cambiar `uses_tables` o las pantallas de cuentas de M23. Interruptores de cocina, domicilios o tienda en línea: no hay pantallas que encender.
- Cambiar permisos, `base_allows`, roles o `role_permissions`.
- Partir `checkout-dialog.tsx`, unir Clientes con Cartera, renombrar ítems del menú, el buscador global o reorganizar `/configuracion` en tarjetas. Quedan para módulos aparte.
- Heredar los interruptores a sucursales nuevas.
- Cambiar los colores de marca o agregar dependencias.
- Aplicar migraciones a Supabase.

## Pruebas (M24.6)

### Unitarias (`settings.rs`, bloque `#[cfg(test)]`)

- **[P]** `features_for("TIENDA")` da `uses_warehouses = false`, `uses_alcohol = true`, `uses_credit = false`.
- **[P]** `features_for("LICORERA")` igual que `TIENDA`.
- **[P]** `features_for("BODEGA")` da `uses_warehouses = true`, `uses_alcohol = false`.
- **[P]** `features_for("RESTAURANTE")` y `features_for("OTRO")` dan los dos en `false`.
- **[P]** `features_for(x).business_type == x` para los cinco valores.
- **[P]** `FeaturesSettings::default()` da `uses_warehouses = true`, `uses_alcohol = true`, `uses_credit = false`, `uses_tables = false`, `business_type = "OTRO"`.
- **[P]** `features_for(x).uses_tables` es `false` para los cinco tipos (el tipo no enciende mesas).
- **[P]** Un `BranchSettings` leído de `{}` o de un JSON sin `features` da esos mismos valores por defecto (`serde_json::from_value`).

### Base de datos (`m24_negocio`, llamada al final de `database_integration`)

- **[P]** `create_business` con `business_type = "LICORERA"` deja una fila en `branch_settings` para la sucursal nueva, con `features.business_type = "LICORERA"`, `uses_alcohol = true` y `uses_warehouses = false`; `settings::get_settings` de esa sucursal lo devuelve igual.
- **[P]** `create_business` con `business_type = "BODEGA"` deja `uses_warehouses = true` y `uses_alcohol = false`.
- **[P]** `create_business` sin `business_type` deja `features.business_type = "OTRO"` con los dos interruptores en `false`.
- **[P]** `create_business` con `business_type = "SUPERMERCADO"`: 422 `ONBOARDING_INVALID` con `data.field = "business_type"`, y no se crea ninguna fila en `tenants` (comprobar el conteo antes y después).
- **[P]** Con `business_name` inválido **y** `business_type` inválido a la vez, el error trae `field = "business_name"` (el orden de validación).
- **[P]** La auditoría `NEGOCIO_CREADO` trae `business_type` en `after`.
- **[P]** `update_settings` con `features.business_type = "XYZ"`: error de validación. Con `"RESTAURANTE"`: 200.
- **[P]** `update_settings` con `uses_warehouses = false` y `uses_alcohol = false`: `get_settings` los devuelve así.
- **[P]** Un negocio que ya existía, sin fila en `branch_settings`: `get_settings` devuelve `uses_warehouses = true`, `uses_alcohol = true`, `uses_credit = false` (no se les esconde nada).
- **[P]** `CAJERO` puede leer los interruptores con `get_settings` (permiso `inventory:read`) y **no** puede guardarlos (403 por `settings:write`).
- Las pruebas viejas de onboarding (`db_tests.rs`, alrededor de la línea 12045 en adelante) **no se borran**: si alguna rompe porque ahora se crea una fila de ajustes, ajustala y listala en el reporte.

## Verificación del coordinador

Esto lo corre el modelo 3.8, no los subagentes, **después de unir todo y del último cambio**. Si algo falla, la corrección vuelve al subagente dueño del archivo.

```sh
cd backend && ./scripts/test.sh
cd ../frontend && npx tsc --noEmit && npx eslint . && npx next build
cd .. && git grep -n 'Ã' -- . ':!*.png'    # solo puede salir CLAUDE.md
ls backend/supabase/migrations | grep -c 20261120   # tiene que dar lo mismo que antes de M24: M24 no agrega ninguna
```

### Lista de verificación en modo demo (puerto 3200)

```sh
cd frontend && NEXT_PUBLIC_SUPABASE_URL= NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY= NEXT_PUBLIC_USE_BACKEND=false npx next dev -p 3200
```

1. `/onboarding`: se ven las 5 tarjetas, "Tienda" viene elegida, "Restaurante" muestra el texto de que las mesas llegan después, y "Crear negocio" sigue llevando a "Tu negocio está listo" con sus 4 enlaces.
2. `/configuracion/negocio`: bajo "Vender fiado" están "Usar varias bodegas" y "Vender licores". Apagar "Usar varias bodegas" y guardar hace desaparecer el botón "Bodegas" de la cabecera, sin recargar. `/configuracion/bodegas` por URL muestra "No tenés permiso para ver esta pantalla".
3. `/inventario`: aparece "Ajustes de inventario" y lleva a `/inventario/ajustes`.
4. `/marketing/promociones` tiene "Cupones" y `/marketing/cupones` tiene "Promociones".
5. `/panel`: hay una tarjeta "Pendientes de hoy" antes de "Accesos rápidos"; si no hay pendientes, dice "Todo al día."
6. Con el ancho de la ventana por debajo de 768 px: el menú lateral no se ve, aparece la barra inferior con 4 ítems y "Más"; "Más" muestra el resto, el selector de sucursal y "Salir". A 768 px o más, la barra inferior no se ve.
7. El formulario de producto muestra grados y volumen con "Vender licores" encendido, y los esconde al apagarlo (en un producto sin grados).

La barra inferior por rol y los interruptores contra la base real no se pueden probar en el demo (entra como dueño). Claude los revisa con usuarios reales.

## Reporte

En `antigravity/REPORTE-ROLES.md`, sección `## M24`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc`, `eslint` y `next build`.
3. La salida de los comandos de la verificación.
4. Una tabla con cada regla [P] y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. La lista de pruebas viejas que hubo que ajustar, con el archivo y la línea.
6. Los límites conocidos: los interruptores no se heredan a sucursales nuevas, y el tipo `RESTAURANTE` no tiene pantallas propias.
7. Qué hizo la política RLS de `branch_settings` al insertar en el alta (B3.3) y cómo está resuelto el mecanismo de `canOpen` (F3).
8. Todo lo que no probaste, escrito como "no lo probé".

# Reporte Fase S: Alta de negocio, usuarios, permisos por rol y reportes para salir a vender

## Cierre y reporte final de la fase S (M17)

> [!IMPORTANT]
> **Recordatorio para Claude:** La migración única [`20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql) **no** está aplicada en Supabase remoto (`supabase db push` prohibido para Antigravity). Todas las pruebas de integración se ejecutaron en PostgreSQL 17 local con Docker mediante `./scripts/test.sh`.

### 1. Ramas, commits y archivos creados/modificados (`feat/fase-3-clientes..HEAD`)

#### Backend (`nexa-backend`)
- **Rama local:** `feat/fase-s-salida` → **Rama remota:** `origin/feat/fase-s-salida`

```text
$ git -C nexa-backend log --oneline feat/fase-3-clientes..HEAD
12ac0f8 fix(fase-s): quitar rutas duplicadas de reportes
7f80936 feat(fase-s): S7 reportes y csv
2bbf7a2 feat(fase-s): S6 panel con datos reales
df33de3 feat(fase-s): S5 vista de ventas y metricas
0d66e98 feat(fase-s): S4 permisos y navegacion por rol
bea1f8c feat(fase-s): S3 datos del negocio y sucursales
a24f401 feat(fase-s): S2 invitaciones
dace84a feat(fase-s): S1 alta de negocio
```

**Archivos tocados en `nexa-backend` (20 archivos):**
- [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación de endpoints S1–S7 (`/v1/onboarding`, invitaciones, `/v1/business`, `/v1/branches*`, permisos de `/v1/purchases*`, `/v1/reports/*`) y nota de `/registro`.
- [`scripts/test-db/00_supabase_shim.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/scripts/test-db/00_supabase_shim.sql): Columna `email_confirmed_at timestamptz` en el shim `auth.users` para pruebas locales.
- [`src/cash.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/cash.rs): Helper `open_session_of_tx` y soporte en `record_movement_tx` / `lock_open_session_inner_tx` para movimientos `DEVOLUCION` con permiso `pos:refund`.
- [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Nuevas baterías `s1_onboarding`, `s2_invitations`, `s3_business`, `s4_permissions`, `s5_sales_view`, `f7_dashboard`, `f7_reports` y ajustes documentados en `platform_checks` y `f1_refunds`.
- [`src/invitations.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/invitations.rs): Módulo con `my_invitations` y `accept_invitation` sobre los RPCs de Postgres y mapeo de errores `INVITATION_*`.
- [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs): Declaración de `onboarding`, `invitations`, `reports` y cobertura de rutas protegidas en `protected_routes_require_token_and_answer_json`.
- [`src/onboarding.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/onboarding.rs): Endpoint de alta inicial (`POST /v1/onboarding`) con validaciones en orden, `create_business`, scope RLS y auditoría `NEGOCIO_CREADO`.
- [`src/procurement.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/procurement.rs): Sustitución de `Action::ManageInventory` por permisos granulares `purchases:create`, `purchases:approve`, `purchases:receive` y `require_any`.
- [`src/refunds.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/refunds.rs): Selección de turno (`select_cash_session_tx`) priorizando el turno abierto del solicitante, luego el aprobador o `409 REFUND_NEEDS_OPEN_SESSION`.
- [`src/reports/mod.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/mod.rs): Declaración pública de los submódulos `metrics`, `dashboard`, `queries` y `csv`.
- [`src/reports/metrics.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/metrics.rs): Fórmulas únicas S5/F7.1 en `Decimal` + `money()` y 4 pruebas unitarias.
- [`src/reports/dashboard.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/dashboard.rs): Consulta `GET /v1/reports/dashboard` con KPIs, series horaria/diaria, top 10, métodos con `CREDITO`, alertas `stock_alerts` `BAJO` y cartera vencida, más 3 unitarias.
- [`src/reports/queries.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/queries.rs): Implementación de los 9 reportes (`sales-by-*`, `inventory-*`, `purchases-by-supplier`), alcance opcional por sucursal bajo RLS y 6 unitarias.
- [`src/reports/csv.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/csv.rs): Exportación CSV con BOM UTF-8, separador `;`, decimales con coma, escape RFC 4180 y 3 unitarias.
- [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs): Registro de rutas de S1–S7 (y corrección `12ac0f8` retirando las rutas `/v1/reports/*` duplicadas).
- [`src/settings.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/settings.rs): Campo `iva_responsible`, `PUT /v1/business` con validaciones DIAN de NIT/DV y auditoría `NEGOCIO_ACTUALIZADO`.
- [`src/tenancy.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/tenancy.rs): `"purchases:receive"` para `SUPERVISOR` en `base_allows`, `Scope::require_any`, `create_branch` y `update_branch` con auditoría.
- [`src/test_support.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/test_support.rs): `seed_user` asigna `email_confirmed_at = now()` en `auth.users` y puebla `users.email`.
- [`src/users.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/users.rs): Roles asignables de salida (`ROLE_NOT_AVAILABLE`), chequeo `USER_ALREADY_MEMBER`, persistencia de `full_name` y `revoke_invitation`.
- [`supabase/migrations/20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql): Migración única de la fase S con bloques `-- S1` a `-- S5`.

#### Frontend (`nexa-frontend`)
- **Rama local:** `feat/fase-s-salida` → **Rama remota:** `origin/frontend/feat/fase-s-salida`

```text
$ git -C nexa-frontend log --oneline feat/fase-3-clientes..HEAD
a8e8797 feat(fase-s): S7 reportes y csv
8506cc4 feat(fase-s): S6 panel con datos reales
3493011 feat(fase-s): S4 permisos y navegacion por rol
8465a4a feat(fase-s): S3 datos del negocio y sucursales
bdc69d0 fix(fase-s): S2 roles y mensaje de invitacion
2aff069 fix(fase-s): S1 pantalla de exito del onboarding
b8ebbd1 feat(fase-s): S2 invitaciones
8e85d55 feat(fase-s): S1 alta de negocio
```

**Archivos tocados en `nexa-frontend` (37 archivos):**
- [`app/(admin)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/layout.tsx): Protección `<RouteGuard>` anidada dentro de `<SessionGate>`.
- [`app/(admin)/ventas/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/ventas/page.tsx): Ruta `/ventas` para `SalesReportsScreen`.
- [`app/(admin)/ventas/productos/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/ventas/productos/page.tsx): Ruta `/ventas/productos` para `ProductReportsScreen`.
- [`app/(operacion)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(operacion)/layout.tsx): Protección `<RouteGuard>` anidada dentro de `<SessionGate>`.
- [`app/(publico)/onboarding/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(publico)/onboarding/page.tsx): Ruta `/onboarding` (pantalla 12).
- [`app/(publico)/registro/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(publico)/registro/page.tsx): Ruta `/registro` (pantalla 09).
- [`app/(referencia)/[...slug]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(referencia)/[...slug]/page.tsx): Retorno temprano `notFound()` en producción (`NODE_ENV === "production"`).
- [`components/auth/onboarding-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/onboarding-screen.tsx): Formulario de alta inicial, aviso de invitaciones pendientes y pantalla `"Tu negocio está listo"` con sus 4 enlaces.
- [`components/auth/register-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/register-screen.tsx): Registro con `signUp` de Supabase, `?correo=` prellenado y validación de contraseña.
- [`components/auth/route-guard.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/route-guard.tsx): Guardia cliente de rutas por permisos con enlace `"Ir a mi inicio"` a `homeFor(role)`.
- [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx): Botón `"Crear mi negocio"` y tarjetas de invitaciones en `no-business`; bloqueo de roles `MESERO`/`COCINA`/`REPARTIDOR`.
- [`components/auth/user-menu.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/user-menu.tsx): Opción `"Invitaciones (<n>)"` con diálogo de aceptación cuando `invitations.length > 0`.
- [`components/compras/purchases-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/compras/purchases-reports-tab.tsx): Pestaña `"Reportes"` en `/compras` con `purchases-by-supplier` y descarga CSV.
- [`components/compras/purchases-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/compras/purchases-screen.tsx): Selector `"Órdenes de compra"` / `"Reportes"` sujeto a `can("reports:read")`.
- [`components/configuracion/settings-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/configuracion/settings-screen.tsx): Edición de negocio (NIT + DV DIAN en vivo, `iva_responsible`) y gestión de sucursales.
- [`components/configuracion/users-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/configuracion/users-screen.tsx): Invitación con roles asignables de salida, pantalla `"Invitación creada"` (`Copiar mensaje` / WhatsApp) y revocación.
- [`components/inventario/inventory-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/inventario/inventory-reports-tab.tsx): Pestaña `"Reportes"` en `/inventario` (`inventory-valuation` e `inventory-rotation` + CSV).
- [`components/inventario/inventory-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/inventario/inventory-screen.tsx): Selector `"Existencias"` / `"Reportes"` sujeto a `can("reports:read")`.
- [`components/nexa/operation-topbar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/operation-topbar.tsx): Retiro de `"Bloquear terminal"`, enlace `"Panel"` / `"Ventas"` por permiso e indicador de caja en móviles.
- [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx): Retiro de `isAllowedForRole`, `/mesas` y `/rutas`; filtro `canOpen`; `"Reportes de ventas"` y `"Cartera"`; `"Cerrar sesión"` activo con `signOut()`.
- [`components/panel/panel-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/panel-screen.tsx): Panel real `/panel` con selector de periodo y sucursal, KPIs comparativos, gráficas, top 10, métodos con `"Crédito"`, stock bajo y cartera vencida.
- [`components/panel/sales-trend-chart.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/sales-trend-chart.tsx): Gráficas de barras por hora (`SalesTrendChart`) y día (`DailySalesChart`) en `div` + CSS puro.
- [`components/ventas/product-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/product-reports-screen.tsx): Pantalla 03 `/ventas/productos` (`sales-by-product`, `sales-by-category`, alerta de margen `< 15%`, ordenamiento y CSV).
- [`components/ventas/report-controls.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/report-controls.tsx): Controles compartidos `ReportFilterBar`, `SalesReportNav`, `CostMissingWarning` y `ExportCsvButton`.
- [`components/ventas/sales-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/sales-reports-screen.tsx): Pantalla 02 `/ventas` (`sales-by-day`, `sales-by-cashier`, `sales-by-method`, `sales-heatmap` 7×24 sin CSV y exportación CSV).
- [`lib/api/client.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/client.ts): Helpers `saveBlob`, `filenameFromDisposition` y `downloadBlob` con `Bearer` token y `URL.createObjectURL`.
- [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts): Métodos `api.onboarding`, `api.invitations`, `api.business`, `api.branches` y `api.reports` más los 9 códigos en `ERROR_MESSAGES`.
- [`lib/api/mock-invitations.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-invitations.ts): Mock de `"Licorera La 15"` para `mine`, `accept` y `revoke`.
- [`lib/api/mock-onboarding.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-onboarding.ts): Mock de creación de negocio en modo demo con validaciones de longitud.
- [`lib/api/mock-platform.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-platform.ts): Mocks actualizados de invitaciones, `mockUpdateBusiness`, `mockCreateBranch` y `mockUpdateBranch`.
- [`lib/api/mock-reports.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-reports.ts): Mocks en COP de licorería colombiana para los 9 reportes y `mockExportReportCsv` con BOM + `;` + `,`.
- [`lib/api/mock.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock.ts): Implementación `mockGetDashboard` con datos COP coherentes para `/panel`.
- [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts): Contratos TypeScript de S1–S7 (`Onboarding*`, `MyInvitation`, `BusinessInfo`, `BranchItem`, `DashboardResponse`, `Report*`).
- [`lib/nit.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/nit.ts): Función pura `nitDv(nit)` módulo 11 DIAN idéntica al backend.
- [`lib/route-permissions.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/route-permissions.ts): Mapa `ROUTE_PERMISSIONS` y `canOpen(pathname, can)` con coincidencia por prefijo más largo.
- [`lib/screens.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/screens.ts): Registro en estado `"react"` de las pantallas `02`, `03`, `09` y `12`.
- [`lib/stores/session.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/stores/session.ts): `ASSIGNABLE_ROLES`, `invitations` en sesión, `acceptInvitation` y `homeFor(role)`.

---

### 2. Verificación final de ambos repositorios

> [!NOTE]
> La primera corrida de verificación en M17 detectó que el commit [`7f80936`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) había dejado las 9 rutas `/v1/reports/*` duplicadas en [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs#L240-L260), provocando pánico de Axum (`Overlapping method route`) en las 5 pruebas de enrutador. Se corrigió en el nuevo commit [`12ac0f8`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) (`fix(fase-s): quitar rutas duplicadas de reportes`) sin tocar ninguna otra funcionalidad y se volvió a correr toda la batería desde cero.

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"`, últimas 30 líneas)
```text
test reports::csv::tests::csv_response_sets_headers_and_filename ... ok
test reports::dashboard::tests::fill_hours_and_days_zero_fill_gaps ... ok
test reports::dashboard::tests::previous_period_matches_duration ... ok
test reports::dashboard::tests::query_treats_empty_strings_as_none ... ok
test reports::metrics::tests::metrics_avg_ticket_zero_sales ... ok
test reports::metrics::tests::metrics_margin_pct ... ok
test reports::metrics::tests::metrics_net_sale ... ok
test reports::metrics::tests::metrics_period_and_item_formulas ... ok
test reports::queries::tests::fill_heatmap_generates_7x24_grid_in_order ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test reports::queries::tests::inventory_days_handles_zero_and_fractional_sales ... ok
test reports::queries::tests::method_label_translates_credit_with_accent ... ok
test reports::queries::tests::report_format_validates_values_and_csv_support ... ok
test reports::queries::tests::report_query_treats_empty_strings_as_none ... ok
test reports::queries::tests::sales_by_product_csv_has_bom_and_spanish_headers ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 60 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.90s
```

#### Frontend (`npx tsc --noEmit` y `npm run lint`)
```text
$ cd nexa-frontend && npx tsc --noEmit
TSC_EXIT=0

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
LINT_EXIT=0
```

---

### 3. Salida de los tres chequeos `grep`

```sh
$ grep -n "delivery_fee" nexa-backend/supabase/migrations/20261109000100_salida.sql
(vacío - 0 coincidencias, exit code 1)

$ grep -n "Action::ManageInventory" nexa-backend/src/procurement.rs
(vacío - 0 coincidencias, exit code 1)

$ grep -n "isAllowedForRole\|/mesas\|/rutas" nexa-frontend/components/nexa/sidebar.tsx
(vacío - 0 coincidencias, exit code 1)
```

---

### 4. Migración única `20261109000100_salida.sql` completa y bloques

> **Recordatorio:** Esta migración la aplica **Claude** manualmente sobre Supabase remoto después de revisar el PR.

- **`-- S1 Alta de negocio` (líneas 1–49):** Crea `public.create_business(p_business_name, p_branch_name, p_full_name)` como `security definer` (`set search_path = ''`, `#variable_conflict use_column`) con `pg_advisory_xact_lock`, verifica sesión y ausencia de membresía activa (`ONBOARDING_ALREADY_MEMBER`), inserta usuario, tenant, sucursal y membresía `DUENO` global (`branch_id = null`), revocando `public`/`anon` y otorgando `execute` solo a `authenticated`.
- **`-- S2 Invitaciones` (líneas 52–124):** Agrega constraint `invitations_status_check` (`PENDIENTE`, `ACEPTADA`, `REVOCADA`) y las funciones `public.my_invitations()` y `public.accept_invitation(p_id)` (`security definer`, `set search_path = ''`), validando correo confirmado (`email_confirmed_at is not null`) y estado `PENDIENTE` bajo `for update`, solo para `authenticated`.
- **`-- S3 Negocio y sucursales` (líneas 126–133):** Reemplaza la política RLS `tenants_member_read` exigiendo `m.active = true` y crea `tenants_member_update` restringida a `public.current_tenant_id()`.
- **`-- S4 Permisos` (líneas 135–141):** Concede el permiso `'purchases:receive'` al rol `'SUPERVISOR'` en `public.role_permissions` (`on conflict do nothing`).
- **`-- S5 Vista de ventas` (líneas 143–155):** Crea `public.v_sales` con `(security_invoker = true)` sin `delivery_fee`, calcula `local_date`, `local_hour`, `local_isodow`, `tax = o.inc_amount` y `gross_sale = o.subtotal + o.inc_amount` filtrando órdenes `COBRADA`/`CERRADA`, con `grant select` exclusivamente para `authenticated`.

```sql
-- S1 Alta de negocio
create or replace function public.create_business(
  p_business_name text,
  p_branch_name text,
  p_full_name text
)
returns table (tenant_id uuid, branch_id uuid)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_tenant uuid;
  v_branch uuid;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  -- Dos llamadas simultáneas del mismo usuario no crean dos negocios.
  perform pg_advisory_xact_lock(hashtext('create_business:' || v_uid::text));

  if exists (select 1 from public.memberships m where m.user_id = v_uid and m.active) then
    raise exception 'ONBOARDING_ALREADY_MEMBER';
  end if;

  select u.email into v_email from auth.users u where u.id = v_uid;

  insert into public.users (id, full_name, email)
  values (v_uid, p_full_name, v_email)
  on conflict (id) do update set full_name = excluded.full_name;

  insert into public.tenants (name) values (p_business_name)
  returning id into v_tenant;

  insert into public.branches (tenant_id, name) values (v_tenant, p_branch_name)
  returning id into v_branch;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_tenant, null, public.tipo_id('ROL', 'DUENO'), true);

  return query select v_tenant, v_branch;
end;
$$;
revoke all on function public.create_business(text, text, text) from public, anon;
grant execute on function public.create_business(text, text, text) to authenticated;


-- S2 Invitaciones
alter table public.invitations
  add constraint invitations_status_check check (status in ('PENDIENTE','ACEPTADA','REVOCADA'));

create or replace function public.my_invitations()
returns table (id uuid, tenant_name text, branch_name text, role_code text, role_name text, created_at timestamptz)
language sql
stable
security definer
set search_path = ''
as $$
  select i.id, t.name, b.name, r.tima_codigo, r.tima_nombre, i.created_at
    from public.invitations i
    join public.tenants t on t.id = i.tenant_id
    left join public.branches b on b.id = i.branch_id
    join public.tbl_tipos_maestra r on r.tima_id = i.role_id
    join auth.users u on u.id = auth.uid()
   where i.status = 'PENDIENTE'
     and u.email_confirmed_at is not null
     and lower(i.email) = lower(u.email)
   order by i.created_at
$$;
revoke all on function public.my_invitations() from public, anon;
grant execute on function public.my_invitations() to authenticated;

create or replace function public.accept_invitation(p_id uuid)
returns table (out_tenant_id uuid, out_branch_id uuid, out_role_code text)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_confirmed timestamptz;
  v_inv public.invitations%rowtype;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  select u.email, u.email_confirmed_at into v_email, v_confirmed
    from auth.users u where u.id = v_uid;

  select * into v_inv from public.invitations i where i.id = p_id for update;

  -- El mismo error si no existe o es de otro correo: no se revela que existe.
  if not found or v_confirmed is null or lower(v_inv.email) <> lower(v_email) then
    raise exception 'INVITATION_NOT_FOUND';
  end if;
  if v_inv.status <> 'PENDIENTE' then
    raise exception 'INVITATION_NOT_PENDING';
  end if;

  insert into public.users (id, full_name, email)
  values (v_uid, v_inv.full_name, v_email)
  on conflict (id) do nothing;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_inv.tenant_id, v_inv.branch_id, v_inv.role_id, true)
  on conflict (user_id, tenant_id, branch_id)
  do update set role_id = excluded.role_id, active = true;

  update public.invitations set status = 'ACEPTADA', accepted_at = now() where id = p_id;

  return query
    select v_inv.tenant_id, v_inv.branch_id, r.tima_codigo::text
      from public.tbl_tipos_maestra r where r.tima_id = v_inv.role_id;
end;
$$;
revoke all on function public.accept_invitation(uuid) from public, anon;
grant execute on function public.accept_invitation(uuid) to authenticated;

-- S3 Negocio y sucursales
drop policy tenants_member_read on public.tenants;
create policy tenants_member_read on public.tenants for select using (
  id in (select m.tenant_id from public.memberships m where m.user_id = auth.uid() and m.active)
);
create policy tenants_member_update on public.tenants for update
  using (id = public.current_tenant_id())
  with check (id = public.current_tenant_id());

-- S4 Permisos
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, 'purchases:receive'
  from public.tbl_tipos_maestra r
 where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
   and r.tima_codigo = 'SUPERVISOR'
on conflict do nothing;

-- S5 Vista de ventas
create view public.v_sales with (security_invoker = true) as
select o.id, o.tenant_id, o.branch_id, o.created_at,
       (o.created_at at time zone b.timezone)::date as local_date,
       extract(hour from o.created_at at time zone b.timezone)::int as local_hour,
       extract(isodow from o.created_at at time zone b.timezone)::int as local_isodow,
       o.subtotal, o.inc_amount as tax, o.discount_total, o.tip_amount,
       (o.subtotal + o.inc_amount) as gross_sale,
       o.refunded_amount, o.customer_id, o.sale_type_id, o.cash_session_id
  from public.orders o
  join public.branches b on b.id = o.branch_id
 where o.status_id in (public.tipo_id('ORD','COBRADA'), public.tipo_id('ORD','CERRADA'));
grant select on public.v_sales to authenticated;
```

---

### 5. Tabla consolidada de reglas `[P]` de M10 a M16

| Módulo | Regla `[P]` | Descripción | Prueba | Estado |
| :--- | :--- | :--- | :--- | :--- |
| **M10 (S1)** | **[P1]** | Usuario sin membresía crea su negocio: `tenant`, `branch`, membresía `DUENO` global activa, bodega `"Principal"`, `CAJA-1` y auditoría `NEGOCIO_CREADO`. | `s1_onboarding` ([`db_tests.rs#L11984`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L11984)) | Aprobada |
| **M10 (S1)** | **[P2]** | Tras el alta, `tenancy::begin` funciona con ese usuario y su rol es `DUENO`. | `s1_onboarding` ([`db_tests.rs#L12068`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12068)) | Aprobada |
| **M10 (S1)** | **[P3]** | Segunda llamada del mismo usuario devuelve 409 `ONBOARDING_ALREADY_MEMBER` manteniendo 1 solo negocio. | `s1_onboarding` ([`db_tests.rs#L12077`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12077)) | Aprobada |
| **M10 (S1)** | **[P4]** | Usuario con membresía `CAJERO` activa en otro negocio devuelve 409 `ONBOARDING_ALREADY_MEMBER`. | `s1_onboarding` ([`db_tests.rs#L12110`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12110)) | Aprobada |
| **M10 (S1)** | **[P5]** | `business_name = " a "` devuelve 422 `ONBOARDING_INVALID` con `field = "business_name"`. | `s1_onboarding` ([`db_tests.rs#L12147`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12147)) | Aprobada |
| **M10 (S1)** | **[P6]** | Rol `anon` no puede ejecutar `public.create_business`. | `s1_onboarding` ([`db_tests.rs#L12183`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12183)) | Aprobada |
| **M10 (S1)** | **[P7]** | Negocio nuevo no ve datos de otro negocio (`GET /v1/customers` devuelve 0 filas). | `s1_onboarding` ([`db_tests.rs#L12198`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12198)) | Aprobada |
| **M11 (S2)** | **[P1]** | Invitar con `role = "MESERO"` devuelve 422 `ROLE_NOT_AVAILABLE`. | `s2_invitations` ([`db_tests.rs#L12250`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12250)) | Aprobada |
| **M11 (S2)** | **[P2]** | Invitar correo con membresía activa en el negocio devuelve 409 `USER_ALREADY_MEMBER`. | `s2_invitations` ([`db_tests.rs#L12273`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12273)) | Aprobada |
| **M11 (S2)** | **[P3]** | Usuario con correo confirmado ve su invitación en `my_invitations`, la acepta, queda con membresía activa del rol/sucursal invitados y la invitación pasa a `ACEPTADA`. | `s2_invitations` ([`db_tests.rs#L12301`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12301)) | Aprobada |
| **M11 (S2)** | **[P4]** | Usuario con otro correo que intenta aceptar recibe 404 `INVITATION_NOT_FOUND` y no se crea membresía. | `s2_invitations` ([`db_tests.rs#L12381`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12381)) | Aprobada |
| **M11 (S2)** | **[P5]** | Usuario con correo sin confirmar no la ve en `my_invitations` y aceptar devuelve 404 `INVITATION_NOT_FOUND`. | `s2_invitations` ([`db_tests.rs#L12426`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12426)) | Aprobada |
| **M11 (S2)** | **[P6]** | Aceptar dos veces devuelve 409 `INVITATION_NOT_PENDING` en el segundo intento. | `s2_invitations` ([`db_tests.rs#L12367`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12367)) | Aprobada |
| **M11 (S2)** | **[P7]** | Invitación revocada no se puede aceptar: 409 `INVITATION_NOT_PENDING`. | `s2_invitations` ([`db_tests.rs#L12468`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12468)) | Aprobada |
| **M11 (S2)** | **[P8]** | `ADMIN_CAJA` no puede revocar una invitación de rol `DUENO`: 403 Forbidden. | `s2_invitations` ([`db_tests.rs#L12509`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12509)) | Aprobada |
| **M12 (S3)** | **[P1]** | Dueño actualiza nombre, razón social y NIT `900123456` sin DV: guarda DV `8` y auditoría `NEGOCIO_ACTUALIZADO`. | `s3_business` ([`db_tests.rs#L12583`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12583)) | Aprobada |
| **M12 (S3)** | **[P2]** | NIT `900123456` con `dv = 7` devuelve 422 `NIT_DV_INVALID`. | `s3_business` ([`db_tests.rs#L12561`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12561)) | Aprobada |
| **M12 (S3)** | **[P3]** | `ADMIN_CAJA` cambia solo `name` (200); `ADMIN_CAJA` cambia `nit` (403 Forbidden). | `s3_business` ([`db_tests.rs#L12612`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12612)) | Aprobada |
| **M12 (S3)** | **[P4]** | Dueño crea sucursal `"Norte"` con bodega `"Principal"` y `CAJA-1`; crear `"norte"` da 409 `BRANCH_DUPLICATED`. | `s3_business` ([`db_tests.rs#L12665`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12665)) | Aprobada |
| **M12 (S3)** | **[P5]** | `ADMIN_CAJA` intenta crear sucursal: 403 Forbidden. | `s3_business` ([`db_tests.rs#L12649`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12649)) | Aprobada |
| **M12 (S3)** | **[P6]** | Con membresía desactivada, `select name from tenants` devuelve 0 filas por RLS. | `s3_business` ([`db_tests.rs#L12738`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12738)) | Aprobada |
| **M13 (S4)** | **[P1]** | `BODEGA` crea compra: 403; `BODEGA` recibe compra aprobada: 200. | `s4_permissions` ([`db_tests.rs#L12837`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12837)) | Aprobada |
| **M13 (S4)** | **[P2]** | `SUPERVISOR` aprueba compra: 403; `SUPERVISOR` la recibe: 200. | `s4_permissions` ([`db_tests.rs#L12868`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12868)) | Aprobada |
| **M13 (S4)** | **[P3]** | `CAJERO` lista compras: 403. | `s4_permissions` ([`db_tests.rs#L12891`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12891)) | Aprobada |
| **M13 (S4)** | **[P4]** | `ADMIN_CAJA` crea, aprueba y recibe compra: 200 en las tres operaciones. | `s4_permissions` ([`db_tests.rs#L12916`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12916)) | Aprobada |
| **M13 (S4)** | **[P5]** | Cajero con turno abierto pide devolución en efectivo; dueño sin turno la aprueba; movimiento `DEVOLUCION` y `cash_session_id` quedan en el turno del cajero. | `s4_permissions` ([`db_tests.rs#L12930`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12930)) | Aprobada |
| **M14 (S5)** | *(Sin `[P]` numerado)* | Fórmulas puras de S5 (`metrics_avg_ticket_zero_sales`, `metrics_margin_pct`, `metrics_net_sale`, `metrics_period_and_item_formulas`) + vista `v_sales` (`local_*`, exclusión propina/`CANCELADA`/`CREADA`, venta `CREDITO` en su día vs abono posterior, `security_invoker`, sin `anon`). | [`metrics.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/metrics.rs#L220-L368) y `s5_sales_view` ([`db_tests.rs#L13082-L13485`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L13082-L13485)) | Aprobada |
| **M15 (S6)** | **[P1]** | Con datos conocidos (ventas válidas, 1 devolución aprobada y 1 venta cancelada), cada KPI da el valor exacto y la venta cancelada no cuenta. | `f7_dashboard` ([`db_tests.rs#L13870`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L13870)) | Aprobada |
| **M15 (S6)** | **[P2]** | Venta mixta de 50.000 (20.000 efectivo + 30.000 crédito): `gross_sale` suma los 50.000 completos y `payment_methods` muestra `EFECTIVO` 20.000 y `CREDITO` 30.000. | `f7_dashboard` ([`db_tests.rs#L13871`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L13871)) | Aprobada |
| **M16 (S7)** | **[P1]** | Suma de `sales-by-product.venta_sin_impuesto` igual a `Σ line_total` del periodo (por sucursal y consolidado). | `f7_reports` ([`db_tests.rs#L14559-L14628`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L14559-L14628)) | Aprobada |
| **M16 (S7)** | **[P2]** | El CSV empieza con el BOM UTF-8 (`0xEF 0xBB 0xBF`) y usa `;` como separador. | `f7_reports` ([`db_tests.rs#L14629-L14658`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L14629-L14658)) y unitarias en [`csv.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/csv.rs#L80-L140) / [`queries.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/queries.rs#L1433-L1463) | Aprobada |

---

### 6. Lista de verificación (modo demo, `http://localhost:3200`)

El servidor demo en el puerto `3200` (`NEXT_PUBLIC_SUPABASE_URL="" NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY="" NEXT_PUBLIC_USE_BACKEND="false" BACKEND_URL=""`) arrancó correctamente (`✓ Ready in 11.8s`), **pero no lo probé en Chrome** porque Chrome DevTools MCP falló al intentar conectar (`DevToolsActivePort` no encontrado / respuesta HTTP `403`, y la configuración `--slim` del servidor MCP tampoco expone `navigate_page`, `new_page`, `take_snapshot`, `click` ni `fill`).

| # | Punto de la lista de verificación | Resultado |
| :--- | :--- | :--- |
| **1** | `/login` → "Crear cuenta" → `/registro` → "Crear cuenta" lleva a `/onboarding`. | **No lo probé** (bloqueo Chrome DevTools MCP) |
| **2** | `/onboarding` con datos válidos muestra "Tu negocio está listo" y sus 4 enlaces abren pantallas que existen. | **No lo probé** (bloqueo Chrome DevTools MCP) |
| **3** | `/panel` muestra los KPI, la gráfica por hora, el top 10, los métodos de pago con "Crédito", el stock bajo y la cartera vencida. | **No lo probé** (bloqueo Chrome DevTools MCP) |
| **4** | `/ventas` y `/ventas/productos` cargan, y "Exportar CSV" descarga un archivo que Excel abre con tildes y columnas separadas. | **No lo probé** (bloqueo Chrome DevTools MCP) |
| **5** | `/configuracion/usuarios` → "Invitar usuario" (Cajero / Todas las sucursales) → "Invitación creada" con Copiar/WhatsApp y sin Mesero/Cocina/Repartidor. | **No lo probé** (bloqueo Chrome DevTools MCP) |
| **6** | `/configuracion/negocio` → cambiar nombre comercial, guardar y recargar mantiene el nombre nuevo; "Nueva sucursal" agrega al selector. | **No lo probé** (bloqueo Chrome DevTools MCP; ver nota en §9 sobre estado en memoria de `mock-platform.ts`) |
| **7** | Menú lateral sin "Restaurante" ni "Rutas y mayorista", y "Cerrar sesión" deja la sesión cerrada pidiendo entrar al volver a `/panel`. | **No lo probé** (bloqueo Chrome DevTools MCP; ver nota en §9 sobre recarga en modo demo) |
| **8** | La barra del POS no tiene el botón del candado. | **No lo probé** (bloqueo Chrome DevTools MCP) |

---

### 7. Pruebas viejas ajustadas y por qué

- **[`platform_checks`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L1856):** Se cambió el `role` de la invitación de prueba de `"MESERO"` a `"SUPERVISOR"` porque M11 (S2) bloquea la invitación de roles pospuestos (`MESERO`, `COCINA`, `REPARTIDOR`) con `422 ROLE_NOT_AVAILABLE`.
- **[`f1_refunds`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L5097-L5108,L5203-L5206):** En M13 (S4.2), [`refunds::apply_effects_tx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/refunds.rs) prioriza el turno abierto de quien pide la devolución sobre el de quien la aprueba. Por eso se cierra explícitamente el turno del cajero antes de verificar `409 REFUND_NEEDS_OPEN_SESSION` y la caída al turno del aprobador, y se reabre antes de `s3`. Todas las aserciones `[P]` originales se conservaron.
- **Shim local y [`seed_user`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/test_support.rs#L48-L57):** [`00_supabase_shim.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/scripts/test-db/00_supabase_shim.sql#L8) agregó `email_confirmed_at` a `auth.users`, y `seed_user` lo puebla junto con `users.email` para soportar las consultas de S1/S2 (`my_invitations`, `accept_invitation`, `USER_ALREADY_MEMBER`).
- **[`tenancy::tests::role_matrix`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/tenancy.rs#L521-L544):** No se modificó y sigue pasando sin cambios. Evalúa únicamente las variantes heredadas del enum `Action` de la fase 0, mientras S4.1 trasladó compras en [`src/procurement.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/procurement.rs) a permisos granulares `purchases:*` (probados en `s4_permissions`) y añadió `"purchases:receive"` para `"SUPERVISOR"` en [`base_allows`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/tenancy.rs#L105).

---

### 8. Dependencias agregadas

- **Ninguna.** Tanto [`Cargo.toml`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/Cargo.toml) / `Cargo.lock` como [`package.json`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/package.json) / `package-lock.json` no tienen modificaciones frente a `feat/fase-3-clientes`.

---

### 9. Preguntas abiertas y tareas detenidas

- **Detención en verificación de navegador (`:3200` modo demo):** La ejecución automatizada de los 8 puntos de la lista de verificación en Chrome falló por indisponibilidad del puerto de depuración de Chrome DevTools MCP (`DevToolsActivePort` inexistente y error HTTP `403`, además de perfil `--slim` sin herramientas `navigate_page`/`click`/`fill`/`take_snapshot`). La comprobación visual/interactiva completa queda pendiente de revisión manual en `http://localhost:3200` y de la validación de Claude con usuarios reales (incluyendo menú por rol S4.4 y guardia de rutas S4.3).
- **Nota estática sobre recarga de página en punto #6 (modo demo):** En [`nexa-frontend/lib/api/mock-platform.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-platform.ts#L28-L41,L526-L591), `mockBusinessInfo` y `mockBranchesList` viven únicamente en memoria de módulo (no en `localStorage`). En modo demo sin backend (`USE_MOCKS`), navegar tras guardar conserva el nuevo nombre, pero recargar el documento completo con `F5` reiniciará el estado mock en memoria. No se modificó en M17 por la regla *"No programes nada nuevo"*.
- **Nota estática sobre cierre de sesión en punto #7 (modo demo):** [`nexa-frontend/lib/stores/session.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/stores/session.ts#L125-L129,L161-L164) limpia el store a `status: "signed-out"` al hacer clic en `"Cerrar sesión"` (con lo cual [`SessionGate`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx#L28-L30) redirige a `/login?next=...` en navegación de Next.js), aunque si se recarga por completo con `!AUTH_ENABLED` vuelve a iniciarse `"Usuario demo"`. Sin verificación en vivo por el bloqueo de Chrome MCP.
- **Restricción preexistente en `npm run build` (M13 / S4.7):** Tal como quedó registrado en [`## M13`](#m13-s4-permisos-y-navegaci%C3%B3n-por-rol), [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx#L18-L22), `RouteGuard` y `Sidebar` invocan `usePathname()` sin `<Suspense>` envolvente en los layouts, lo que detiene `npm run build` (`CLIENT_HOOK_DYNAMIC`) en rutas dinámicas preexistentes (`/inventario/producto/[id]`, `/clientes/[id]`). Por alcance de fase S no se alteraron los layouts, de modo que [`app/(referencia)/[...slug]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(referencia)/%5B...slug%5D/page.tsx#L21) compilado en producción no fue probado en runtime.
- **Aclaración sobre tabla histórica de `## M11`:** Se mantuvo intacta la sección `## M11` inferior como exige M17. Cuatro filas extras de su tabla original (`DUENO` por `ADMIN_CAJA`, longitud `full_name`, `revoke` 404 y permiso `anon` sobre `my_invitations`/`accept_invitation`) no poseen aserción explícita en `s2_invitations` (aunque [`20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql#L74-L75,L123-L124) sí revoca `public, anon`); las **8 reglas `[P]` oficiales** definidas en [`M11-S2-invitaciones.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/antigravity/modulos/M11-S2-invitaciones.md#L187-L194) están todas aprobadas en [`s2_invitations`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L12250-L12533).
- **Cambios previos sin commitear en `nexa-frontend`:** [`components/auth/route-guard.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/route-guard.tsx#L34) y [`lib/route-permissions.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/route-permissions.ts#L39) tenían 1 línea en blanco final previa sin staged en `git status`; se dejaron intactos sin incluirlos en los commits.

---

## M10: S1 Alta de negocio

### 1. Hash de cada commit y archivos tocados

#### Backend (`nexa-backend`)
- **Commit:** [`dace84a`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S1 alta de negocio`
- **Archivos tocados:**
  - [`supabase/migrations/20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql): Creación de función `public.create_business` con bloqueo de concurrencia y asignación de membresía `DUENO`.
  - [`src/onboarding.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/onboarding.rs): Módulo con validación de longitudes en orden, ejecución de `create_business`, configuración de RLS, permisos y auditoría `NEGOCIO_CREADO`.
  - [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs): Declaración del módulo `pub mod onboarding;`.
  - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs): Registro del endpoint `POST /v1/onboarding` y su handler.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Batería completa de pruebas `s1_onboarding` cubriendo todas las reglas [P].
  - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación del endpoint `/v1/onboarding` y nota sobre `/registro` en sección "Conectar Supabase".

#### Frontend (`nexa-frontend`)
- **Commit:** [`8e85d55`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase-s): S1 alta de negocio`
- **Archivos tocados:**
  - [`app/(publico)/registro/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(publico)/registro/page.tsx): Ruta `/registro` (pantalla 09).
  - [`components/auth/register-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/register-screen.tsx): Formulario de registro con validaciones en orden, toggle de contraseña y signUp de Supabase / redirección demo.
  - [`app/(publico)/onboarding/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(publico)/onboarding/page.tsx): Ruta `/onboarding` (pantalla 12).
  - [`components/auth/onboarding-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/onboarding-screen.tsx): Pantalla de onboarding con campos en orden, pantalla de éxito "Tu negocio está listo" con 4 enlaces y control de sesión.
  - [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx): Botón principal "Crear mi negocio" en estado `no-business`.
  - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts): Interfaces `CreateBusinessInput` y `OnboardingResult`.
  - [`lib/api/mock-onboarding.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-onboarding.ts): Mock de onboarding con validación de longitudes y UUIDs aleatorios.
  - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts): Integración `api.onboarding.create` y mensajes de error `ONBOARDING_INVALID` y `ONBOARDING_ALREADY_MEMBER`.
  - [`lib/screens.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/screens.ts): Pantallas 09 y 12 marcadas como `"react"`.

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::whatsapp_phone_normalization ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.40s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida de los chequeos grep

```sh
$ git -C nexa-backend grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
# 0 coincidencias (exit code 1)
```

### 4. Tabla de reglas [P] de M10

| Regla [P] | Descripción | Prueba en `s1_onboarding` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Usuario sin membresía crea su negocio: quedan un `tenant`, una `branch`, membresía `DUENO` con `branch_id = null` y `active = true`, bodega `Principal` con `is_default = true`, caja `CAJA-1` y fila `NEGOCIO_CREADO` en `audit_log`. | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P2]** | Tras el alta, `tenancy::begin` con ese usuario funciona y su rol es `DUENO`. | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P3]** | Segunda llamada del mismo usuario devuelve 409 `ONBOARDING_ALREADY_MEMBER`; se mantiene un solo negocio para el usuario. | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P4]** | Usuario con membresía `CAJERO` activa en otro negocio devuelve 409 `ONBOARDING_ALREADY_MEMBER`. | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P5]** | `business_name = " a "` (longitud 1 tras trim) devuelve 422 `ONBOARDING_INVALID` con `field = "business_name"`. | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P6]** | Rol `anon` no puede ejecutar `public.create_business` (error de permisos de PostgreSQL). | `s1_onboarding` en `db_tests.rs` | Aprobado |
| **[P7]** | Negocio creado no ve datos de otro negocio: `GET /v1/customers` con el nuevo dueño devuelve 0 filas aunque otro tenant tenga clientes. | `s1_onboarding` en `db_tests.rs` | Aprobado |

### 5. Lo que no se pudo hacer
- **Ninguno**: Todas las funcionalidades del módulo M10 fueron completadas y verificadas tanto en backend como en frontend.

## M10A: Corrección pantalla de éxito del onboarding

### 1. Commit realizado
- **Frontend (`nexa-frontend`)**: `2aff069` `fix(fase-s): S1 pantalla de exito del onboarding`

### 2. Archivo modificado
- `components/auth/onboarding-screen.tsx`:
  - `useEffect` actualizado con guardas `!AUTH_ENABLED || success || submitting` para no redirigir automáticamente a `/panel` tras la creación del negocio, permitiendo mostrar la pantalla "Tu negocio está listo" con los 4 primeros pasos.
  - Uso de `friendlyError(err)` en la rama `ONBOARDING_INVALID`.

### 3. Salida de pruebas y linters
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

## M11: S2 Invitaciones

### 1. Commits realizados
- **Backend (`nexa-backend`)**: `a24f401` `feat(fase-s): S2 invitaciones`
- **Frontend (`nexa-frontend`)**: `b8ebbd1` `feat(fase-s): S2 invitaciones`

### 2. Archivos modificados o creados

#### Backend
- `supabase/migrations/20261109000100_salida.sql` (bloque `-- S2 Invitaciones`, funciones `my_invitations`, `accept_invitation`, check constraint)
- `scripts/test-db/00_supabase_shim.sql` (columna `email_confirmed_at` añadida a `auth.users`)
- `src/invitations.rs` (nuevo módulo con `my_invitations` y `accept_invitation`)
- `src/main.rs` (registro del módulo `invitations`)
- `src/routes.rs` (rutas `/v1/users/invitations/{id}/revoke`, `/v1/invitations/mine`, `/v1/invitations/{id}/accept`)
- `src/users.rs` (`ASSIGNABLE_ROLES`, validación `ROLE_NOT_AVAILABLE`, `USER_ALREADY_MEMBER`, auditoría, `revoke_invitation`)
- `src/test_support.rs` (`seed_user` con email y email_confirmed_at)
- `src/db_tests.rs` (batería de pruebas `s2_invitations` cubriendo todas las reglas [P])
- `README.md` (documentación de endpoints S2)

#### Frontend
- `lib/api/types.ts` (`InviteUserInput`, `Invitation`, `MyInvitation`, `AcceptInvitationResult`)
- `lib/api/mock-invitations.ts` (nuevo mock para `mine`, `accept`, `revoke`)
- `lib/api/mock-platform.ts` (actualización de `mockInviteUser`)
- `lib/api/index.ts` (`api.invitations.mine()`, `api.invitations.accept()`, `api.users.revokeInvitation()`, mapeo de errores)
- `lib/stores/session.ts` (`ASSIGNABLE_ROLES`, `invitations: MyInvitation[]`, `boot()` con invitaciones, `acceptInvitation`)
- `components/configuracion/users-screen.tsx` (modal invitar con Nombre 2-80, Correo, Rol, Sucursal; pantalla invitación creada con Copiar/WhatsApp/Listo; tabla con Copiar mensaje y Revocar)
- `components/auth/register-screen.tsx` (soporte de query param `?correo=`)
- `components/auth/session-gate.tsx` (pantalla `no-business` con tarjetas de invitaciones y botón aceptar)
- `components/auth/user-menu.tsx` (ítem en menú "Invitaciones (<n>)" y diálogo para aceptarlas)
- `components/auth/onboarding-screen.tsx` (banner amarillo superior de alerta si existen invitaciones pendientes)

### 3. Salida de pruebas y linters

#### Backend (`test.sh`)
```text
running 44 tests
test catalog::tests::tax_rate_resolves_known_codes ... ok
test credit::tests::credit_plan_rejects_interest ... ok
test credit::tests::schedule_fixed_payment_known_value ... ok
test credit::tests::schedule_fixed_never_negative_interest ... ok
test credit::tests::schedule_month_end ... ok
test credit::tests::schedule_simple_last_absorbs_rounding ... ok
test customers::tests::compute_missing_fiscal_checks_order ... ok
test customers::tests::normalize_customer_splits_dv ... ok
test customers::tests::normalize_customer_person_type ... ok
test db_tests::whatsapp_phone_normalization ... ok
test error::tests::domain_errors_map_to_expected_status ... ok
test error::tests::non_database_sqlx_errors_are_internal ... ok
test inventory::tests::adjustment_type_follows_sign ... ok
test error::tests::coded_error_body_has_code ... ok
test money::tests::money_rounds_half_away_from_zero ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test pricing::tests::allocate_respects_caps ... ok
test pricing::tests::allocate_sums_exactly ... ok
test pricing::tests::line_promo_picks_best ... ok
test pricing::tests::coupon_after_promotions_on_net ... ok
test pricing::tests::order_promo_requires_min_subtotal ... ok
test pricing::tests::max_discount_caps_across_lines ... ok
test pricing::tests::overridden_line_gets_no_discounts ... ok
test pricing::tests::promo_tie_breaks_by_priority ... ok
test pricing::tests::resolve_tier_order ... ok
test pricing::tests::skip_tier_lines_excludes_tier_line ... ok
test pricing::tests::tax_on_discounted_base ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test procurement::tests::nit_dv_known_values ... ok
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.74s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 4. Tabla de reglas [P] de M11

| Regla [P] | Descripción | Prueba en `s2_invitations` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Invitar con `role = "MESERO"` (no disponible para la salida): 422 `ROLE_NOT_AVAILABLE`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P2]** | Invitar con rol `DUENO` por usuario `ADMIN_CAJA`: 403 Forbidden. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P3]** | Invitar a un correo que ya tiene una membresía activa en el negocio: 409 `USER_ALREADY_MEMBER`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P4]** | Invitar con `full_name` menor a 2 caracteres o mayor a 80: 422 de validación. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P5]** | Usuario invitado con sesión y correo confirmado ve su invitación en `my_invitations`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P6]** | Usuario con correo distinto o sin confirmar no la ve en `my_invitations`, y aceptar da 404 `INVITATION_NOT_FOUND`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P7]** | Aceptar invitación con sesión válida: crea usuario si no existía, membresía activa con el rol asignado, actualiza status a `ACEPTADA`, timestamp `accepted_at` y audit `USUARIO_ACEPTO_INVITACION`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P8]** | Revocar invitación por el dueño pasa status a `REVOCADA` y registra audit `INVITACION_REVOCADA`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P9]** | Aceptar una invitación `REVOCADA` devuelve 409 `INVITATION_NOT_PENDING`. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P10]** | `ADMIN_CAJA` no puede revocar una invitación de rol `DUENO`: devuelve 403 Forbidden. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P11]** | Revocar una invitación inexistente o de otro negocio devuelve 404 Not Found. | `s2_invitations` en `db_tests.rs` | Aprobado |
| **[P12]** | Rol `anon` no tiene permiso de ejecución sobre `public.my_invitations` ni `public.accept_invitation`. | `s2_invitations` en `db_tests.rs` | Aprobado |

### 5. Lo que no se pudo hacer
- **Ninguno**: Todas las funcionalidades del módulo M11 fueron implementadas, testeadas e integradas de acuerdo con la especificación.

## M11A: Corrección roles y mensaje de invitación

### 1. Commit realizado
- **Frontend (`nexa-frontend`)**: `bdc69d0` `fix(fase-s): S2 roles y mensaje de invitacion`
- **Backend (`nexa-backend`)**: Ninguno (árbol de trabajo limpio y verificado).

### 2. Archivos modificados
- `components/configuracion/users-screen.tsx`:
  - Diálogo "Seleccionar nuevo rol": restringido a `availableRoles` (solo roles asignables + dueño si el usuario actual es dueño).
  - Pestaña "Roles y permisos": grilla restringida a `ASSIGNABLE_ROLES`.
  - Mensaje "Copiar mensaje" en la tabla de invitaciones pendientes: ahora incluye el nombre del usuario (`Hola${inv.full_name ? " " + inv.full_name : ""}, te invité a...`).
  - Columna sucursal: simplificada a `inv.branch_name ?? "Todas las sucursales"`.
- `lib/api/types.ts`:
  - Se eliminó el campo en desuso `branch_ids` de `Invitation`.
- `lib/api/mock-platform.ts`:
  - Se adaptaron los mocks para usar `branch_id`, `branch_name` y `full_name`, eliminando `branch_ids`.

### 3. Salida de los comandos de verificación

```sh
$ cd nexa-backend && git status --short
(vacío - exit code 0)

$ cd ../nexa-frontend && npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)

$ grep -n "Object.keys(ROLE_LABELS)" components/configuracion/users-screen.tsx
(vacío - 0 coincidencias)

$ grep -rn "branch_ids" lib components
(vacío - 0 coincidencias)
```

## M12: S3 Datos del negocio y sucursales

### 1. Hash de cada commit y lista de archivos tocados

#### Backend (`nexa-backend`)
- **Commit:** [`bea1f8c`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S3 datos del negocio y sucursales`
- **Archivos tocados:**
  - [`supabase/migrations/20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql): Actualización de políticas RLS `tenants_member_read` (con filtro `m.active`) y creación de `tenants_member_update`.
  - [`src/settings.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/settings.rs): Campo `iva_responsible` en `BusinessInfo`, validaciones en orden y auditoría `NEGOCIO_ACTUALIZADO` en `update_business`.
  - [`src/tenancy.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/tenancy.rs): Implementación de `create_branch` (con reseteo de RLS `db::set_scope(&mut tx, scope.tenant, None)` y auditoría `SUCURSAL_CREADA`) y `update_branch` (con auditoría `SUCURSAL_ACTUALIZADA`).
  - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs): Rutas montadas `POST /v1/branches`, `PUT /v1/branches/{id}` y `PUT /v1/business` con sus handlers.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Batería completa de pruebas de integración `s3_business`.
  - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación de `POST /v1/branches`, `PUT /v1/branches/{id}` y `PUT /v1/business`.

#### Frontend (`nexa-frontend`)
- **Commit:** [`8465a4a`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase-s): S3 datos del negocio y sucursales`
- **Archivos tocados:**
  - [`lib/nit.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/nit.ts): Función pura `nitDv(nit: string): number | null` con el algoritmo módulo 11 de la DIAN idéntico a `procurement::nit_dv`.
  - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts): Actualización de `BusinessInfo` (`iva_responsible`), y nuevos tipos `UpdateBusinessInput`, `CreateBranchInput`, `UpdateBranchInput` y `BranchItem`.
  - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts): Métodos `api.business.update`, `api.branches.create`, `api.branches.update`, y errores `BUSINESS_INVALID`, `BRANCH_INVALID`, `BRANCH_DUPLICATED`.
  - [`lib/api/mock-platform.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-platform.ts): Implementación mock de `mockUpdateBusiness`, `mockCreateBranch`, `mockUpdateBranch` y soporte para `iva_responsible`.
  - [`components/configuracion/settings-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/configuracion/settings-screen.tsx): Pestañas "Datos del Negocio" (NIT, DV en tiempo real, `iva_responsible`, campos deshabilitados con mensaje para no dueños, guardado con `friendlyError`) y "Sucursales" (tarjetas con "Cambiar nombre" y "Nueva sucursal" para dueño, con invocación a `useSession().boot()`).

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test tenancy::tests::role_matrix ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test pricing::tests::allocate_respects_caps ... ok
test pricing::tests::allocate_sums_exactly ... ok
test pricing::tests::promo_tie_breaks_by_priority ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.99s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida de los chequeos grep

```sh
$ git -C nexa-backend grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
(vacío - 0 coincidencias)

$ git -C nexa-frontend grep -n "nitDv" lib/nit.ts
3:export function nitDv(nit: string): number | null {
```

### 4. Tabla de reglas [P] de M12

| Regla [P] | Descripción | Prueba en `s3_business` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | El dueño cambia el nombre, la razón social y el NIT `900123456` sin DV: queda guardado con DV `8` y hay auditoría `NEGOCIO_ACTUALIZADO`. | `s3_business` en `db_tests.rs` | Aprobado |
| **[P2]** | NIT `900123456` con `dv = 7`: 422 `NIT_DV_INVALID`. | `s3_business` en `db_tests.rs` | Aprobado |
| **[P3]** | `ADMIN_CAJA` cambia solo `name`: 200. `ADMIN_CAJA` cambia `nit`: 403 Forbidden. | `s3_business` en `db_tests.rs` | Aprobado |
| **[P4]** | El dueño crea la sucursal "Norte": existe, con su bodega "Principal" y su caja `CAJA-1`. Crear otra "norte": 409 `BRANCH_DUPLICATED`. | `s3_business` en `db_tests.rs` | Aprobado |
| **[P5]** | `ADMIN_CAJA` crea una sucursal: 403 Forbidden. | `s3_business` en `db_tests.rs` | Aprobado |
| **[P6]** | Con la membresía desactivada, `select name from tenants` devuelve 0 filas por la política RLS actualizada `tenants_member_read`. | `s3_business` en `db_tests.rs` | Aprobado |

### 5. Lo que no se pudo hacer
- **Ninguno**: Todos los requisitos y reglas del módulo M12 fueron completados, testeados e integrados sin incidencias.

## M13: S4 Permisos y navegación por rol

### 1. Ramas, commits y lista de archivos tocados

- **Rama en ambos repositorios:** `feat/fase-s-salida`

#### Backend (`nexa-backend`)
- **Commit:** [`0d66e98`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S4 permisos y navegacion por rol`
- **Archivos tocados:**
  - [`supabase/migrations/20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql): Bloque `-- S4 Permisos` al final asignando `purchases:receive` al rol `SUPERVISOR`.
  - [`src/tenancy.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/tenancy.rs): Método `Scope::require_any(&self, codes: &[&str])` y adición de `"purchases:receive"` en `base_allows` para `"SUPERVISOR"`.
  - [`src/procurement.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/procurement.rs): Reemplazo de todos los `Action::ManageInventory` en funciones públicas y `*_tx` por permisos granulares `purchases:create`, `purchases:approve`, `purchases:receive` o `require_any`.
  - [`src/cash.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/cash.rs): Nueva función `open_session_of_tx` para consultar el turno abierto de un usuario concreto y ajuste en `record_movement_tx` / `lock_open_session_inner_tx` para permitir registrar movimientos `DEVOLUCION` con `pos:refund` sobre el turno del cajero.
  - [`src/refunds.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/refunds.rs): Selección de turno en `apply_effects_tx` (`select_cash_session_tx`) priorizando `requested_by`, luego aprobador y si no hay ninguno `409 REFUND_NEEDS_OPEN_SESSION`, guardando `cash_session_id`.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Nueva batería `s4_permissions` con las reglas `[P]` de S4 y ajuste en `f1_refunds` acorde con S4.2.
  - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación de permisos reales en `/v1/purchases*`.

#### Frontend (`nexa-frontend`)
- **Commit:** [`3493011`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase-s): S4 permisos y navegacion por rol`
- **Archivos creados y modificados:**
  - [`lib/route-permissions.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/route-permissions.ts): Tabla exacta de permisos por prefijo (`ROUTE_PERMISSIONS`) y función `canOpen(pathname, can)` con coincidencia del prefijo más largo.
  - [`components/auth/route-guard.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/route-guard.tsx): Componente `RouteGuard` con vista de acceso denegado (`AuthShell` + enlace `"Ir a mi inicio"` a `homeFor(role)`).
  - [`app/(admin)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/layout.tsx): Envuelve el contenido dentro de `<SessionGate><RouteGuard>`.
  - [`app/(operacion)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(operacion)/layout.tsx): Envuelve el contenido dentro de `<SessionGate><RouteGuard>`.
  - [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx): Bloqueo de pantalla en `status === "ready"` para roles `MESERO`, `COCINA` y `REPARTIDOR` con botón `"Salir"`.
  - [`lib/stores/session.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/stores/session.ts): `homeFor(role: Role | null)` actualizado para `CAJERO` (`/pos`), `BODEGA` (`/inventario`) y resto (`/panel`).
  - [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx): Eliminados `isAllowedForRole`, `/mesas` y `/rutas`; añadido ítem "Cartera" (`/cartera`); filtrado exclusivo con `canOpen(item.href, can)`; menú vacío si `role === null`; logo hacia `homeFor(role)`; `"Cerrar sesión"` convertido en `<button>` con `signOut()` + `router.replace("/login")`.
  - [`components/nexa/operation-topbar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/operation-topbar.tsx): Eliminado botón `"Bloquear terminal"`; enlace contextual `"Panel"` (`/panel`, si `reports:read`) o `"Ventas"` (`/ordenes`, si `cash:history`); indicador de caja visible en pantallas `< md` como `"Caja abierta"` / `"Caja cerrada"` manteniendo `BranchSwitcher` oculto bajo `md`.
  - [`app/(referencia)/[...slug]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(referencia)/[...slug]/page.tsx): Llamada temprana a `notFound()` cuando `process.env.NODE_ENV === "production"`.

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test pricing::tests::allocate_sums_exactly ... ok
test pricing::tests::allocate_respects_caps ... ok
test pricing::tests::line_promo_picks_best ... ok
test pricing::tests::overridden_line_gets_no_discounts ... ok
test pricing::tests::coupon_after_promotions_on_net ... ok
test pricing::tests::resolve_tier_order ... ok
test pricing::tests::skip_tier_lines_excludes_tier_line ... ok
test pricing::tests::tax_on_discounted_base ... ok
test pricing::tests::promo_tie_breaks_by_priority ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test pricing::tests::max_discount_caps_across_lines ... ok
test pricing::tests::order_promo_requires_min_subtotal ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.27s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
(exit code 0 - 0 errors)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Migración y salidas de los chequeos `grep`

- **Migración (`supabase/migrations/20261109000100_salida.sql`):** Se añadió al final el bloque `-- S4 Permisos` que inserta `'purchases:receive'` en `public.role_permissions` para el rol `'SUPERVISOR'` (con `on conflict do nothing`). **No se aplicó a Supabase remoto.**

```sh
$ git -C nexa-backend grep -n "Action::ManageInventory" src/procurement.rs
(vacío - exit code 1, 0 coincidencias)

$ git -C nexa-frontend grep -n -E "isAllowedForRole|/mesas|/rutas" components/nexa/sidebar.tsx
(vacío - exit code 1, 0 coincidencias)

$ git -C nexa-backend grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
(vacío - exit code 1, 0 coincidencias)
```

### 4. Tabla de reglas [P] de M13 y prueba `role_matrix`

| Regla [P] | Descripción | Prueba en `s4_permissions` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | `BODEGA` crea una compra: 403. `BODEGA` recibe una compra aprobada: 200. | `s4_permissions` en `db_tests.rs` | Aprobado |
| **[P2]** | `SUPERVISOR` aprueba una compra: 403. `SUPERVISOR` la recibe: 200. | `s4_permissions` en `db_tests.rs` | Aprobado |
| **[P3]** | `CAJERO` lista compras: 403. | `s4_permissions` en `db_tests.rs` | Aprobado |
| **[P4]** | `ADMIN_CAJA` crea, aprueba y recibe: 200 en las tres. | `s4_permissions` en `db_tests.rs` | Aprobado |
| **[P5]** | Cajero con turno abierto pide una devolución en efectivo; dueño sin turno la aprueba; movimiento `DEVOLUCION` y `refunds.cash_session_id` quedan en el turno del cajero. | `s4_permissions` en `db_tests.rs` | Aprobado |

- **`tenancy::tests::role_matrix` (fase 0):** Sigue pasando sin modificaciones (`ok`), ya que solo evalúa las 7 variantes del enum `Action` (`Read`, `TakeOrder`, `Kitchen`, `Charge`, `ManageCash`, `CancelOrder`, `ManageInventory`), mientras que S4.1 trasladó compras de `Action::ManageInventory` al modelo granular `purchases:*` (`Scope::require_permission` / `Scope::require_any`).

### 5. Dependencias agregadas
- **Ninguna.**

### 6. Pruebas viejas ajustadas
- [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L5094-L5203) (`f1_refunds`): Antes de S4.2, el cajero mantenía abierto su turno tras crear `r1`, por lo que aprobar como `owner` sin turno propio devolvía `409 REFUND_NEEDS_OPEN_SESSION` y luego descontaba de `owner_session` (`expected_cash == 8808.80`). Con S4.2, `apply_effects_tx` prioriza el turno abierto de quien pide la devolución (`requested_by`); por ello se cierra explícitamente el turno del cajero antes de validar la falta total de turno abierto (`REFUND_NEEDS_OPEN_SESSION`) y la caída al turno del aprobador, y se reabre antes de `s3`. Todas las aserciones `[P]` originales de `f1_refunds` se mantienen intactas.

### 7. Lo que no se pudo hacer / no probado
- **Navegación, guardia y UI en navegador:** Las pantallas y flujos de [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx#L72-L104), [`components/auth/route-guard.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/route-guard.tsx#L14-L33), [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx#L32-L51) y [`components/nexa/operation-topbar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/operation-topbar.tsx#L36-L74) **no los probé** en navegador con usuarios reales por rol, en modo demo (`:3200`) ni en producción; solo se verificaron con `npx tsc --noEmit` y `npm run lint`.
- **Build de producción (`npm run build`) y S4.7 en runtime:** Al ejecutar `npm run build` como comprobación adicional, Next 16 (`cacheComponents: true`) falló pre-renderizando las rutas dinámicas preexistentes [`app/(admin)/inventario/producto/[id]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/inventario/producto/[id]/page.tsx) y [`app/(admin)/clientes/[id]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/clientes/[id]/page.tsx) con `CLIENT_HOOK_DYNAMIC` porque [`components/auth/session-gate.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/session-gate.tsx#L18-L22) usa `usePathname()` fuera de `<Suspense>` (misma situación en `RouteGuard` línea 15 y `Sidebar` línea 63). Por la regla de no tocar fuera de alcance, no se alteraron los layouts para envolver en `<Suspense>`, así que [`app/(referencia)/[...slug]/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(referencia)/[...slug]/page.tsx#L21) en modo producción (`next build && next start`) **no lo probé**.

---

## M14

### 1. Ramas, commits y lista de archivos tocados

- **Repositorio:** Solo `nexa-backend` (no se tocó `nexa-frontend`).
- **Rama:** `feat/fase-s-salida`
- **Commit (`nexa-backend`):** [`df33de3`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S5 vista de ventas y metricas`
- **Archivos creados y modificados:**
  - [`supabase/migrations/20261109000100_salida.sql`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql): Bloque `-- S5 Vista de ventas` al final creando la vista `public.v_sales` con `security_invoker = true` (sin `delivery_fee`), con `local_date`, `local_hour`, `local_isodow`, `tax` y `gross_sale = subtotal + inc_amount`, filtrada a órdenes `COBRADA`/`CERRADA`, y `grant select` únicamente a `authenticated`.
  - [`src/reports/mod.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/mod.rs): Declaración pública del submódulo `metrics`.
  - [`src/reports/metrics.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/metrics.rs): Fórmulas únicas de S5/F7.1 en `Decimal` + `money()` (`order_gross_sale`, `gross_sale_sum`, `in_period`, `period_gross_sale`, `period_sales_count`, `approved_refunds_total`, `net_sale`, `avg_ticket`, `cost_of_sales`, `net_line_total`, `cost_missing_lines`, `gross_margin`, `margin_pct`, `from_totals`) y pruebas unitarias (`metrics_avg_ticket_zero_sales`, `metrics_margin_pct`, `metrics_net_sale`, `metrics_period_and_item_formulas`).
  - [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs): Registro de `pub mod reports;`.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Nueva batería de integración `s5_sales_view` llamada desde `database_integration`.

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test products::tests::ean13_check_digit ... ok
test reports::metrics::tests::metrics_avg_ticket_zero_sales ... ok
test reports::metrics::tests::metrics_margin_pct ... ok
test reports::metrics::tests::metrics_net_sale ... ok
test sales::tests::receipt_format_pads_number ... ok
test reports::metrics::tests::metrics_period_and_item_formulas ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test tenancy::tests::role_matrix ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.98s
```

### 3. Migración y salida de `grep`

- **Migración (`supabase/migrations/20261109000100_salida.sql`):** Se añadió al final el bloque `-- S5 Vista de ventas` (`lines 143-155`). **No se aplicó a Supabase remoto.**

```sh
$ git -C nexa-backend grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
(vacío - exit code 1, 0 coincidencias)
```

### 4. Tabla de reglas [P] / pruebas exigidas por M14

M14 no define reglas `[P]` numeradas adicionales aparte de las pruebas unitarias exigidas en el cambio 3 y el comportamiento del cambio 4 y la vista `v_sales`:

| Requisito / Regla | Descripción | Prueba | Resultado |
| :--- | :--- | :--- | :--- |
| Cambio 3 (unitaria) | Ticket promedio con 0 ventas y división normal | `reports::metrics::tests::metrics_avg_ticket_zero_sales` | Aprobado |
| Cambio 3 (unitaria) | Porcentaje de margen (incl. base 0, redondeo y negativos) | `reports::metrics::tests::metrics_margin_pct` | Aprobado |
| Cambio 3 (unitaria) | Venta neta (`gross_sale - refunds`) | `reports::metrics::tests::metrics_net_sale` | Aprobado |
| Tabla de fórmulas S5 | Venta bruta, devoluciones `APROBADA` en rango local, costo, margen y `cost_missing` | `reports::metrics::tests::metrics_period_and_item_formulas` | Aprobado |
| Vista `v_sales` + Cambio 4 + RLS | `COBRADA`/`CERRADA`, exclusión de propina/`CANCELADA`/`CREADA`, zona horaria local, `CREDITO` el día de venta y exclusión de abonos posteriores, `security_invoker` y sin `anon` | `s5_sales_view` en `db_tests.rs` | Aprobado |

### 5. Dependencias agregadas
- **Ninguna.**

### 6. Pruebas viejas ajustadas
- **Ninguna.**

### 7. Lo que no se pudo hacer / no probado
- **Supabase remoto:** La vista [`public.v_sales`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/supabase/migrations/20261109000100_salida.sql#L143-L155) **no la probé** contra la instancia remota de Supabase (prohibido aplicar migraciones remotas); solo quedó verificada contra PostgreSQL 17 en Docker mediante `./scripts/test.sh`.
- **Consumidores HTTP en endpoints reales:** Las funciones puras de [`src/reports/metrics.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/metrics.rs#L11-L216) aún no tienen rutas HTTP (llegan en M15/M16), por lo que conservan `#[allow(dead_code)]` y solo se ejercitaron con pruebas unitarias y `s5_sales_view`.

---

## M15

### 1. Hash de cada commit y lista de archivos tocados

- **Rama en ambos repositorios:** `feat/fase-s-salida`

#### Backend (`nexa-backend`)
- **Commit:** [`2bbf7a2`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S6 panel con datos reales`
- **Archivos creados y modificados:**
  - [`src/reports/dashboard.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/dashboard.rs): Endpoint `GET /v1/reports/dashboard?branch_id=&from=&to=` con `DashboardQuery` (cadenas vacías tratadas como `None`), alcance opcional de sucursal (`tenancy::begin_in_branch` vs `tenancy::begin` con RLS), fecha local de sucursal sin `current_date`, KPIs y periodo anterior vía `metrics::from_totals`, series `by_hour` (`0..23`) y `by_day` (`from..=to`), `top_products`, `payment_methods` con `CREDITO`, `low_stock` desde `stock_alerts` (`kind = 'BAJO'` y `resolved_at is null`), `overdue_receivables` con mora en hora local, más pruebas unitarias (`previous_period_matches_duration`, `fill_hours_and_days_zero_fill_gaps`, `query_treats_empty_strings_as_none`).
  - [`src/reports/mod.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/mod.rs): Declaración pública `pub mod dashboard;`.
  - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs): Ruta `/v1/reports/dashboard` y handler `get_dashboard`.
  - [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs): Cobertura de `/v1/reports/dashboard` en `protected_routes_require_token_and_answer_json`.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Prueba de integración `f7_dashboard` invocada desde `database_integration`.
  - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación de `GET /v1/reports/dashboard?branch_id=&from=&to=` y retiro de la nota pendiente del panel.

#### Frontend (`nexa-frontend`)
- **Commit:** [`8506cc4`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase-s): S6 panel con datos reales`
- **Archivos modificados:**
  - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts): Reemplazo de `DashboardSummary` (`PROPUESTO`) por el contrato real `DashboardDateRange`, `DashboardKpis`, `DashboardHourlyPoint`, `DashboardDailyPoint`, `DashboardTopProduct`, `DashboardPaymentMethod`, `DashboardLowStockItem`, `DashboardOverdueReceivables`, `DashboardResponse` y `DashboardQuery`.
  - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts): Actualización de `api.reports.dashboard(query?)` para consumir `/v1/reports/dashboard` en modo real (omitiendo `branch_id` cuando es `null`/opcional) y `mockGetDashboard` en modo demo.
  - [`lib/api/mock.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock.ts): Reemplazo de `mockDashboard` por `mockGetDashboard(query?)` con datos colombianos en COP coherentes con `MockCatalog`, `by_hour`, `by_day`, `CREDITO`, `low_stock` y `overdue_receivables`.
  - [`components/panel/sales-trend-chart.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/sales-trend-chart.tsx): Gráficas de barras por hora (`SalesTrendChart`) y por día (`DailySalesChart`) usando únicamente `div` y CSS, sin librerías de gráficas ni SVG.
  - [`components/panel/panel-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/panel-screen.tsx): Selector de periodo (`Hoy`, `Ayer`, `7 días`, `Este mes`, `Mes anterior`, `Personalizado`), selector opcional de alcance multi-sucursal, aviso de líneas sin costo (`cost_missing_lines`), tarjetas KPI con variación vs periodo anterior (inversión de color en `Margen bruto` si el costo sube), gráficas por hora y día, top 10 productos, métodos de pago con `"Crédito"`, alertas de stock bajo, cartera vencida y accesos rápidos filtrados con `canOpen`.

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test pricing::tests::promo_tie_breaks_by_priority ... ok
test pricing::tests::resolve_tier_order ... ok
test pricing::tests::skip_tier_lines_excludes_tier_line ... ok
test pricing::tests::tax_on_discounted_base ... ok
test procurement::tests::cost_lines_allocation_and_gift_test ... ok
test procurement::tests::nit_dv_known_values ... ok
test products::tests::ean13_check_digit ... ok
test reports::dashboard::tests::fill_hours_and_days_zero_fill_gaps ... ok
test reports::dashboard::tests::previous_period_matches_duration ... ok
test reports::metrics::tests::metrics_avg_ticket_zero_sales ... ok
test reports::dashboard::tests::query_treats_empty_strings_as_none ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test reports::metrics::tests::metrics_net_sale ... ok
test reports::metrics::tests::metrics_margin_pct ... ok
test reports::metrics::tests::metrics_period_and_item_formulas ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.51s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
TSC_OK (exit code 0)

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
(exit code 0 - 0 errors, 0 warnings)
```

### 3. Salida de los chequeos `grep`

```sh
$ git -C nexa-backend grep -n "delivery_fee" -- supabase/migrations/20261109000100_salida.sql
(vacío - exit code 1)

$ git -C nexa-backend grep -n "Action::ManageInventory" -- src/procurement.rs
(vacío - exit code 1)

$ git -C nexa-frontend grep -n -e "isAllowedForRole" -e "/mesas" -e "/rutas" -- components/nexa/sidebar.tsx
(vacío - exit code 1)

$ git -C nexa-backend grep -n "current_date" -- src/reports/dashboard.rs
(vacío - exit code 1)
```

### 4. Tabla de reglas [P] de M15

| Regla [P] | Descripción | Prueba en `f7_dashboard` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | Con datos conocidos (ventas válidas, 1 devolución aprobada y 1 venta cancelada), cada KPI da el valor exacto esperado y la venta cancelada no cuenta. | `f7_dashboard` en [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L13866-L13904) | Aprobado |
| **[P2]** | Venta de 50.000 pagada con 20.000 en efectivo y 30.000 a crédito: `gross_sale` la cuenta completa y `payment_methods` muestra `EFECTIVO` 20.000 y `CREDITO` 30.000. | `f7_dashboard` en [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L13867-L13915) | Aprobado |

### 5. Lo que no se pudo hacer / no probado
- **Supabase remoto:** No se aplicaron ni ejecutaron consultas contra Supabase remoto.
- **UI del panel en navegador (`:3200` ni modo real conectado):** [`components/panel/panel-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/panel-screen.tsx) y [`components/panel/sales-trend-chart.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/panel/sales-trend-chart.tsx) **no los probé** interactivamente en navegador ni contra el servidor en vivo; solo quedaron verificados por `npx tsc --noEmit` y `npm run lint`.

---

## M16

### 1. Hash de cada commit y lista de archivos tocados

- **Rama en ambos repositorios:** `feat/fase-s-salida`
- **Migración:** Ninguna.

#### Backend (`nexa-backend`)
- **Commit:** [`7f80936`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend) `feat(fase-s): S7 reportes y csv`
- **Archivos creados y modificados:**
  - [`src/reports/csv.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/csv.rs): Generación CSV con BOM UTF-8 (`\u{FEFF}`), delimitador `;`, decimales con `,`, escape RFC 4180, respuesta `text/csv; charset=utf-8` con `Content-Disposition: attachment; filename="<reporte>_<from>_<to>.csv"` y 3 pruebas unitarias.
  - [`src/reports/queries.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/queries.rs): Implementación de los 9 reportes (`sales-by-day`, `sales-by-product`, `sales-by-category`, `sales-by-cashier`, `sales-by-method`, `sales-heatmap`, `inventory-valuation`, `inventory-rotation`, `purchases-by-supplier`) con permiso `reports:read`, alcance opcional por `branch_id` bajo RLS (`tenancy::begin_in_branch` vs `tenancy::begin`), métricas calculadas vía `metrics` y `money()`, `CREDITO` etiquetado `"Crédito"`, matriz 7×24 en `sales-heatmap` rechazando `format=csv` con 422, valorización excluyendo `on_hand == 0`, rotación de 30 días ignorando `from` y compras excluyendo `ANULADA`, más 6 pruebas unitarias.
  - [`src/reports/dashboard.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/dashboard.rs): Exposición `pub(crate)` de `MAX_RANGE_DAYS`, `empty_string_as_none` y `local_today_tx`.
  - [`src/reports/mod.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/mod.rs): Declaración pública de `csv` y `queries`.
  - [`src/routes.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/routes.rs): Registro de las 9 rutas `GET /v1/reports/*` y sus handlers.
  - [`src/main.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/main.rs): Cobertura de las 9 rutas `/v1/reports/*` en `protected_routes_require_token_and_answer_json`.
  - [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs): Prueba de integración `f7_reports` invocada en `database_integration` verificando `[P1]`, `[P2]` y los 9 reportes con alcance por sucursal y consolidado.
  - [`README.md`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/README.md): Documentación de los 9 endpoints `GET /v1/reports/*` y exportación `format=csv`.

#### Frontend (`nexa-frontend`)
- **Commit:** [`a8e8797`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `feat(fase-s): S7 reportes y csv`
- **Archivos creados y modificados:**
  - [`lib/api/client.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/client.ts): Helpers `saveBlob`, `filenameFromDisposition` y `downloadBlob` para descarga autenticada por `fetch` con token `Bearer` y `URL.createObjectURL`.
  - [`lib/api/types.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/types.ts): Tipos `ReportQuery`, `ReportCsvSlug` y contratos `*Row` / `*Response` para los 9 reportes.
  - [`lib/api/mock-reports.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/mock-reports.ts): Mocks en COP de licorería colombiana ("Licorería La Reserva") para los 9 reportes (con `CREDITO` = `"Crédito"`) y exportación `mockExportReportCsv` con BOM UTF-8, `;` y `,` decimal.
  - [`lib/api/index.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/index.ts): Endpoints tipados bajo `api.reports` (los 9 JSON y `exportCsv`).
  - [`components/ventas/report-controls.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/report-controls.tsx): Controles `ReportFilterBar`, `SalesReportNav`, `CostMissingWarning` y `ExportCsvButton`.
  - [`components/ventas/sales-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/sales-reports-screen.tsx): Pantalla 02 (`/ventas`) con KPIs, gráfica de barras CSS/HTML, pestañas `"Por día"`, `"Por cajero"`, `"Por método"` (con `"Crédito"`), `"Mapa de calor"` (sin CSV) y `"Exportar CSV"`.
  - [`app/(admin)/ventas/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/ventas/page.tsx): Ruta `/ventas` montando `SalesReportsScreen`.
  - [`components/ventas/product-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/product-reports-screen.tsx): Pantalla 03 (`/ventas/productos`) con tarjetas destacadas (incluyendo alerta de margen `< 15%`), búsqueda, filtro por categoría, tabla ordenable de `sales-by-product`, tabla `sales-by-category` y `"Exportar CSV"` en ambas.
  - [`app/(admin)/ventas/productos/page.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/ventas/productos/page.tsx): Ruta `/ventas/productos` montando `ProductReportsScreen`.
  - [`components/inventario/inventory-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/inventario/inventory-reports-tab.tsx): Pestaña `"Reportes"` en `/inventario` con tablas `inventory-valuation` e `inventory-rotation` y exportación CSV.
  - [`components/inventario/inventory-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/inventario/inventory-screen.tsx): Selector `"Existencias"` | `"Reportes"` condicionado a `can("reports:read")`.
  - [`components/compras/purchases-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/compras/purchases-reports-tab.tsx): Pestaña `"Reportes"` en `/compras` con `purchases-by-supplier` y exportación CSV.
  - [`components/compras/purchases-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/compras/purchases-screen.tsx): Selector `"Órdenes de compra"` | `"Reportes"` condicionado a `can("reports:read")`.
  - [`lib/screens.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/screens.ts): Pantallas `"02"` (`/ventas`) y `"03"` (`/ventas/productos`) actualizadas a `"react"`.
  - [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx): Ítem `"Reportes de ventas"` (`/ventas`, filtrado por `canOpen` → `reports:read`) y ajuste de `match` en `/ordenes` a `["/ordenes", "/gastos"]`.

### 2. Salida de verificación

#### Backend (`./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"` - Últimas 30 líneas)
```text
test reports::csv::tests::formats_numbers_with_comma_decimal ... ok
test pricing::tests::discount_never_exceeds_gross ... ok
test reports::dashboard::tests::fill_hours_and_days_zero_fill_gaps ... ok
test reports::dashboard::tests::previous_period_matches_duration ... ok
test reports::metrics::tests::metrics_avg_ticket_zero_sales ... ok
test reports::dashboard::tests::query_treats_empty_strings_as_none ... ok
test reports::metrics::tests::metrics_margin_pct ... ok
test reports::metrics::tests::metrics_net_sale ... ok
test reports::metrics::tests::metrics_period_and_item_formulas ... ok
test reports::queries::tests::fill_heatmap_generates_7x24_grid_in_order ... ok
test reports::queries::tests::inventory_days_handles_zero_and_fractional_sales ... ok
test reports::queries::tests::method_label_translates_credit_with_accent ... ok
test reports::queries::tests::report_format_validates_values_and_csv_support ... ok
test reports::queries::tests::report_query_treats_empty_strings_as_none ... ok
test reports::queries::tests::sales_by_product_csv_has_bom_and_spanish_headers ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test sales_history::tests::parse_receipt_accepts_both_forms ... ok
test tenancy::tests::role_matrix ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::health_returns_ok ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 60 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.79s
```

#### Frontend (`tsc` y `lint`)
```text
$ npx tsc --noEmit
TSC_EXIT=0

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
LINT_EXIT=0
```

### 3. Salida de los chequeos `grep`

```sh
$ git -C nexa-backend grep -n "delivery_fee" -- supabase/migrations/20261109000100_salida.sql
(vacío - exit code 1)

$ git -C nexa-backend grep -n "Action::ManageInventory" -- src/procurement.rs
(vacío - exit code 1)

$ git -C nexa-frontend grep -n -e "isAllowedForRole" -e "/mesas" -e "/rutas" -- components/nexa/sidebar.tsx
(vacío - exit code 1)

$ git -C nexa-backend grep -n "f64" -- src/reports/
(vacío - exit code 1)

$ grep -rn "current_date" nexa-backend/src/reports/
(vacío - exit code 1)

$ grep -rn "format=csv" nexa-frontend/components/ nexa-frontend/app/
(vacío - exit code 1)
```

### 4. Tabla de reglas [P] de M16

| Regla [P] | Descripción | Prueba en `f7_reports` | Resultado |
| :--- | :--- | :--- | :--- |
| **[P1]** | La suma de `sales-by-product.venta_sin_impuesto` es igual a `Σ line_total` del periodo (probado por sucursal y consolidado multi-sucursal). | `f7_reports` en [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L14559-L14628) | Aprobado |
| **[P2]** | El CSV empieza con el BOM UTF-8 (`0xEF 0xBB 0xBF`) y usa `;` como separador (verificado sobre respuesta HTTP real y unitarias de CSV). | `f7_reports` en [`src/db_tests.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/db_tests.rs#L14629-L14658), [`reports::csv::tests::*`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/csv.rs#L80-L140) y [`reports::queries::tests::sales_by_product_csv_has_bom_and_spanish_headers`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/queries.rs#L1433-L1463) | Aprobado |

### 5. Lo que no se pudo hacer / no probado
- **Supabase remoto:** No se aplicaron ni ejecutaron consultas contra Supabase remoto; las consultas y RLS de [`src/reports/queries.rs`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-backend/src/reports/queries.rs#L1-L1464) solo se probaron contra PostgreSQL 17 en Docker mediante `./scripts/test.sh`.
- **Navegador (`:3200` modo demo, sesión real y apertura del CSV en Excel):**
  - [`components/ventas/sales-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/sales-reports-screen.tsx#L118-L557) (`/ventas`): **no lo probé** en navegador; verificado estáticamente con `tsc` y `eslint`.
  - [`components/ventas/product-reports-screen.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/product-reports-screen.tsx#L37-L456) (`/ventas/productos`): **no lo probé** en navegador; verificado estáticamente con `tsc` y `eslint`.
  - [`components/inventario/inventory-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/inventario/inventory-reports-tab.tsx#L15-L255) y [`components/compras/purchases-reports-tab.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/compras/purchases-reports-tab.tsx#L20-L184) (pestañas `"Reportes"`): **no los probé** en navegador; verificados con `tsc` y `eslint`.
  - [`lib/api/client.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/api/client.ts#L65-L107) (`downloadBlob` / `saveBlob`) y [`components/ventas/report-controls.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/ventas/report-controls.tsx#L223-L263) (`ExportCsvButton`): la descarga real en navegador y apertura en Excel **no la probé**.
  - [`components/nexa/sidebar.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/nexa/sidebar.tsx#L28-L39) y [`lib/screens.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/screens.ts#L92-L93): navegación en vivo por roles y estado `"react"` de 02 y 03 **no los probé** en navegador.
- **Estado de trabajo previo:** En `nexa-frontend`, [`components/auth/route-guard.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/components/auth/route-guard.tsx) y [`lib/route-permissions.ts`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/lib/route-permissions.ts) tenían cambios previos sin commitear antes de iniciar M16; se mantuvieron intactos sin incluirlos en el commit `a8e8797`.

---

## M16A: Corrección build de producción con Suspense en los layouts

### 1. Commit y archivos tocados
- **Frontend (`nexa-frontend`, rama `feat/fase-s-salida`):** [`c3d924a`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend) `fix(fase-s): build de produccion con Suspense en los layouts`
- **Descarte previo sin commit:** Con `git checkout -- components/auth/route-guard.tsx lib/route-permissions.ts` se limpiaron los dos saltos de línea finales previos tras verificar con `git diff`.
- **Archivos modificados:**
  - [`app/(admin)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(admin)/layout.tsx): `<SessionGate>` envuelto en `<Suspense fallback={null}>` (importado de `react`), conservando su comentario original.
  - [`app/(operacion)/layout.tsx`](file:///c:/Users/pedro/Documents/universidad/desarrolloconjunto/nexa-frontend/app/(operacion)/layout.tsx): `<SessionGate>` envuelto en `<Suspense fallback={null}>` (importado de `react`), conservando su comentario original.

### 2. Salida de `tsc` y `lint`

```text
$ npx tsc --noEmit
TSC_EXIT=0

$ npm run lint
> nexa-frontend@0.1.0 lint
> eslint
LINT_EXIT=0
```

### 3. Últimas 25 líneas de `npx next build`

*(Además de `/inventario/producto/[id]` en las últimas 25 líneas, la tabla completa también mostró `◐ /clientes/[id]` bajo `/clientes/[id]` con salida `BUILD_EXIT=0`.)*

```text
├   /inventario/producto/[id]
│ └ ◐ /inventario/producto/[id]
├ ○ /inventario/producto/nuevo
├ ○ /inventario/vencimientos
├ ○ /login
├ ○ /login/restablecer
├ ○ /marketing/cupones
├ ○ /marketing/promociones
├ ○ /modulos
├ ○ /onboarding
├ ○ /ordenes
├ ○ /ordenes/devoluciones
├ ○ /panel
├ ○ /pos
├ ○ /registro
├ ○ /ventas
└ ○ /ventas/productos


ƒ Proxy (Middleware)

○  (Static)             prerendered as static content
◐  (Partial Prerender)  prerendered as static HTML with dynamic server-streamed content
ƒ  (Dynamic)            server-rendered on demand
```



---

## M18

### 1. Cambios realizados

| Tarea | Archivos | Commit |
|---|---|---|
| M18.1 Crear cuenta en el login demo | `frontend/components/auth/login-screen.tsx` | `526db17` |
| M18.2 Sucursales del demo desde la lista mock | `frontend/lib/api/index.ts` (session.branches) | `6019d0d` |
| M18.3 Roles de salida solamente | `frontend/lib/api/mock-platform.ts`, `mock.ts` | `8fabdc8` |
| M18.4 Demo de licorería | `frontend/lib/api/mock-customers.ts`, `mock-inventory.ts`, `mock-procurement.ts`, `components/ordenes/orders-screen.tsx`, `components/configuracion/users-screen.tsx` | `a11919f` |

### 2. Problema de codificación (mojibake) y resolución

Las ediciones iniciales de M18 usaron herramientas de PowerShell 5.1 (`Get-Content`/`Set-Content`) que corrompieron la codificación UTF-8. Ejemplo: "¿No tenés cuenta?" quedó como "Â¿No tenÃ©s cuenta?". Los saltos de línea también cambiaron de LF a CRLF.

Resolución: se guardó el diff en `../respaldos/m18-con-mojibake.patch`, se restauró cada archivo con `git checkout HEAD -- <archivo>`, y se reaplicaron los cambios de M18 usando la herramienta `edit_file` del agente (UTF-8 sin BOM, LF). Se configuró `git config core.autocrlf false`.

Verificación post-corrección:
- `grep -rn "Ã|Â" lib components app` → sin resultados ✓
- `npx tsc --noEmit` → exit 0 ✓
- `npm run lint` → exit 0 ✓
- `npx next build` → exit 0, 57 páginas ✓

### 3. Cambios fuera de alcance revertidos

- `app/(operacion)/layout.tsx`: se restauró `export const instant = false;` que el agente había eliminado (commit `70f095d` del usuario).
- `app/(admin)/modulos/page.tsx`: se revirtió `notFound()` que M18 no pedía.

### 4. Verificación final

- Grep de residuos de demo anterior: sin resultados en archivos de M18 ✓
- "Medellín" aparece solo como nombre de producto "Ron Medellín" ✓
- Demo en puerto 3200: /configuracion/usuarios no muestra Mesero, Cocina ni Repartidor ✓

### 5. Preguntas abiertas

- `app/(admin)/modulos/page.tsx` (~línea 20): el agente propuso agregar `notFound()` para módulos desactivados en producción. No se aplicó; podría ser parte de un módulo de permisos futuro.

---

## M19

### 1. Tabla por tarea

| Tarea | Archivos | Commit | Pruebas |
|---|---|---|---|
| M19.1 Despliegue dentro del repositorio | `.gitignore` (añade `despliegue/.env`) | `1a2651c` | `git status` no muestra `.env`; grep de secretos: vacío ✓ |
| M19.2 Respaldos de la base | `despliegue/respaldos/respaldar.sh`, `restaurar.sh`, `.gitignore` | `920264c` | `bash -n` ✓; sin `DATABASE_URL` → exit 1 con mensaje ✓; sin args → muestra uso ✓ |
| M19.3 Verificación posterior al despliegue | `despliegue/verificar.sh` | `e47a90a` | `bash -n` ✓; sin args → exit 1 ✓; con `no-existe.invalid` → 7 fallos, ~6 s, exit 1 ✓ |
| M19.4 Guía de despliegue | `despliegue/LEEME.md` | `f0d8ee6` | Sin `RAMA_FRONTEND` ni `TU-PROYECTO` ✓; sin secretos en comandos ✓ |

### 2. LEEME.md: orden final de secciones

El LEEME.md original mencionaba "aprobar fase 3" y "copiar despliegue/ a mano". Se reescribió completo en M19.4. Orden final: encabezado (estado/advertencia DIAN) → cómo queda armado → lo que necesitás antes (incluye Supabase Auth URLs) → estructura de disco → primer despliegue en 7 pasos → actualizaciones y rollback → respaldos → verificar → disco y memoria → problemas comunes → si cambian variables públicas → nunca en chat ni git.

No hubo choques de merge porque M19.4 reescribió el archivo completo de forma coordinada con las secciones de M19.2 y M19.3.

### 3. Verificación del coordinador (salida completa)

```
git status --short despliegue/
# → M despliegue/LEEME.md (antes del commit), limpio después

grep -rniE "sb_secret|postgres://..." despliegue/
# → "sin secretos" (vacío)

bash -n despliegue/respaldos/respaldar.sh  → ok
bash -n despliegue/respaldos/restaurar.sh  → ok
bash -n despliegue/verificar.sh            → ok

despliegue/respaldos/respaldar.sh (sin DATABASE_URL)
# ERROR: la variable DATABASE_URL no está definida.
# exit 1 ✓

despliegue/respaldos/restaurar.sh (sin args)
# Uso: DATABASE_URL=<url> ./restaurar.sh <ruta-al-archivo.dump>
# exit 1 ✓

despliegue/verificar.sh (sin args)
# Uso: ./verificar.sh <dominio>
# exit 1 ✓

despliegue/verificar.sh no-existe.invalid
# ✗ 1. /api/v1/health respondió 000 (esperado 200)
# ✗ 2. /api/v1/health/db respondió 000
# ✗ 3. /login respondió 000 (esperado 200)
# ✗ 4. No se pudo leer el certificado HTTPS
# ✗ 5. http://no-existe.invalid no redirigió a https://
# ✗ 6. /api/v1/context sin token devolvió 000 (esperado 401)
# ✗ 7. docker compose ps: algún servicio no está running/healthy
# 7 punto(s) fallaron. exit 1 ✓ (completó en ~6 s)

docker compose -f despliegue/docker-compose.yml config
# → falla con "required variable DATABASE_URL is missing"
# Esperado: el compose está bien escrito; falta .env local con valores reales.
```

### 4. Confirmación

- No se conectó ningún servidor por SSH.
- No se corrió `docker build` ni `docker compose up`.
- No se aplicaron migraciones.
- No se escribió ninguna clave, token, contraseña ni URL con credenciales en ningún archivo ni commit. Donde hace falta un secreto, va el nombre de la variable de entorno.

### 5. Preguntas abiertas

- `despliegue/docker-compose.yml`: `docker compose config` no pudo validarse localmente por falta de `.env` real. La validación completa se hará en el VPS.
- `despliegue/verificar.sh` punto 7: usa `python3` para parsear JSON de `docker compose ps`. Si el VPS no tiene Python 3, el punto fallará. Alternativa: `jq`. Decisión queda para el despliegue real.

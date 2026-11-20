# Reporte de Fin de Fase 0: Plataforma (Permisos, Ajustes, Auditoría, Secuencias y Outbox) — Revisión y Correcciones Aplicadas

Este reporte documenta la implementación de la Fase 0 y las correcciones solicitadas tras la revisión.

---

## 1. Ramas y commits de cada repositorio

### Backend (`nexa-backend/`)
- **Rama:** `feat/fase-0-plataforma` (creada a partir de `feat/compras`)
- **Commits (`feat/compras..HEAD`):**
  - `d2403d1` `fix(plataforma): corregir reglas de dueno, membership activa, login attempts y secuencias`
  - `141cc2f` `Revert "fix(plataforma): resolver warnings de clippy y tipos en db_tests"`
  - `4833fc4` `fix(plataforma): resolver warnings de clippy y tipos en db_tests`
  - `ba4ca5c` `feat(plataforma): backend de permisos, ajustes, auditoria, secuencias y outbox`
  - `2cd62bb` `feat(plataforma): migracion de permisos, ajustes, auditoria, secuencias y outbox`

### Frontend (`nexa-frontend/`)
- **Rama:** `feat/fase-0-plataforma` (creada a partir de `feat/compras`)
- **Commits (`feat/compras..HEAD`):**
  - `2eaf8b8` `fix(plataforma): llamadas a login-attempt antes y despues de auth y permisos base`
  - `8322bf7` `feat(plataforma): pantallas de usuarios, ajustes de negocio, auditoria y menu por rol`

---

## 2. Correcciones específicas aplicadas (Fase 0)

1. **Jerarquía y protección del rol DUENO [P]:**
   - En `src/users.rs`, se garantiza que solo `DUENO` pueda:
     - Asignar el rol `DUENO` en invitaciones (`invite_user`). Un `ADMIN_CAJA` intentándolo recibe 403 Forbidden.
     - Asignar el rol `DUENO` al cambiar roles (`update_user_role`). Un `ADMIN_CAJA` intentando promover a alguien a `DUENO` recibe 403 Forbidden.
     - Cambiarle el rol a un usuario que actualmente es `DUENO` (`update_user_role`). Un `ADMIN_CAJA` recibe 403 Forbidden.
     - Desactivar a un usuario que actualmente es `DUENO` (`update_user_status`). Un `ADMIN_CAJA` recibe 403 Forbidden.
   - En `src/db_tests.rs::platform_checks`, se añadió una prueba **[P]** donde un `ADMIN_CAJA` intenta cada uno de estos 4 casos y recibe 403 Forbidden en todos.

2. **Filtro de membresías activas en `membership_tx` [P]:**
   - En `src/tenancy.rs`, `membership_tx` y `branches` ahora filtran explícitamente con `and m.active`.
   - Si la membresía de un usuario está desactivada (`active = false`), `membership_tx` no retorna registros, resultando en `ApiError::Domain(DomainError::Forbidden)` (HTTP 403).
   - Se agregó prueba **[P]** en `src/db_tests.rs::platform_checks`: un usuario desactivado intenta iniciar transacciones en su sucursal y recibe 403 Forbidden; al reactivarse puede operar con normalidad.

3. **Seguridad y RLS de `login_attempts`:**
   - En `supabase/migrations/20261102000200_platform_foundations.sql`, se eliminó la política permisiva `login_attempts_all` (`using (true)`) y se removieron los grants para el rol `authenticated`.
   - Con RLS habilitado y sin políticas públicas, ningún cliente autenticado ni anónimo vía cliente Supabase puede leer o escribir directamente en `login_attempts`. Solo el backend tiene acceso con permisos de servicio/superusuario.

4. **Acciones mapeadas a códigos de permiso y overrides [P]:**
   - En `src/tenancy.rs`:
     - Se añadió `Action::permission_code(&self)` mapeando cada acción: `Read -> inventory:read`, `TakeOrder -> pos:order`, `Kitchen -> pos:kitchen`, `Charge -> pos:sell`, `ManageCash -> cash:movement`, `CancelOrder -> pos:refund`, `ManageInventory -> inventory:adjust`.
     - `role_allows` evalúa los permisos base (`base_allows`) y sigue pasando al 100% todas las aserciones de la matriz original (`tenancy::tests::role_matrix`).
     - `Scope::require` evalúa `self.require_permission(action.permission_code())`, aplicando efectivamente los overrides cargados para el tenant.
   - Se añadió prueba **[P]** en `src/db_tests.rs::platform_checks`: se crea un override que le quita el permiso `pos:sell` al rol `CAJERO`; al intentar realizar una venta (`sales::checkout`), el cajero es bloqueado inmediatamente con 403 Forbidden.

5. **Exigencia de permisos en descuentos y cambios de precio [P]:**
   - En `src/sales.rs`:
     - Al enviar `unit_price_override`, se valida tanto el ajuste de sucursal `pos.allow_price_override` como el permiso `pos:price_override` del usuario. Si falta el permiso, responde 403 Forbidden.
     - Al enviar `discount_pct`, se valida tanto el tope de sucursal `pos.max_discount_pct` como el permiso `pos:discount` del usuario. Si falta el permiso, responde 403 Forbidden.
     - El precio final por ítem (`final_price`) se redondea explícitamente a 2 decimales con `.round_dp(2)`.
   - Se agregaron pruebas **[P]** en `src/db_tests.rs::platform_checks` verificando ambos requerimientos.

6. **Permisos base para SUPERVISOR, BODEGA y REPARTIDOR:**
   - En `supabase/migrations/20261102000200_platform_foundations.sql`, `src/tenancy.rs` y `nexa-frontend/lib/api/mock-platform.ts`:
     - `SUPERVISOR`: permisos de cajero + `pos:refund`, `inventory:adjust`, `reports:read`.
     - `BODEGA`: `inventory:*` (`inventory:read`, `inventory:adjust`, `inventory:adjust_approve`, `inventory:transfer`) y `purchases:receive`.
     - `REPARTIDOR`: `pos:sell`, `inventory:read`.
   - Verificado con aserciones en `src/db_tests.rs::platform_checks`.

7. **Numeración de ventas con `sequences::next_tx`:**
   - En `src/sales.rs`, se eliminó la lógica duplicada `next_receipt_number` y se conectó directamente a `crate::sequences::next_tx(tx, scope, branch, "POS").await?`.
   - En `src/sequences.rs`, `next_tx` bloquea la fila y valida que la resolución de numeración no haya vencido (`valid_until < current_date`).

8. **F0.3: Rate limiting y registro en `POST /v1/auth/login-attempt`:**
   - Backend:
     - En `src/error.rs`, se agregó `DomainError::TooManyRequests` que mapea a HTTP 429 (`StatusCode::TOO_MANY_REQUESTS`).
     - En `src/auth.rs`, se implementó `check_and_record_login_attempt(pool, email, ip, user_agent, success, reason)`. Cuenta los fallos del correo en los últimos 15 minutos (`at >= now() - interval '15 minutes'`). Si alcanza 5 o más fallos, devuelve error 429.
     - En `src/routes.rs`, se expuso la ruta pública `POST /v1/auth/login-attempt`.
     - En `src/main.rs`, se agregó la prueba `login_attempt_without_token_is_not_unauthorized`.
     - En `src/db_tests.rs::platform_checks`, se añadió prueba **[P]**: tras 4 intentos fallidos pasa; al 5to intento fallido responde 429; y el pre-check subsiguiente responde 429 bloqueando el acceso.
   - Frontend:
     - En `lib/api/index.ts`, se añadieron `api.auth.checkLoginAttempt` y `api.auth.recordLoginAttempt`.
     - En `lib/api/mock-platform.ts`, se añadió seguimiento de intentos fallidos en memoria para modo demo (bloqueo a los 5 intentos en 15 min).
     - En `components/auth/login-screen.tsx`, el formulario llama a `api.auth.checkLoginAttempt(email)` antes de consultar Supabase Auth, y registra el intento (`success: false` o `success: true`) inmediatamente después con `api.auth.recordLoginAttempt`. Si la cuenta está bloqueada, muestra el mensaje amigable de bloqueo temporal de 15 minutos.

---

## 3. Salida de las verificaciones

### Backend (`test.sh` con `fmt --check`, `clippy` y `cargo test`)
```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.21s
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.50s
     Running unittests src/main.rs (/target/debug/deps/nexa_backend-d2fee284b7751c0d)

running 17 tests
test catalog::tests::tax_rate_resolves_known_codes ... ok
test error::tests::non_database_sqlx_errors_are_internal ... ok
test error::tests::domain_errors_map_to_expected_status ... ok
test orders::tests::state_machine_allows_only_forward_steps ... ok
test inventory::tests::adjustment_type_follows_sign ... ok
test sales::tests::receipt_format_pads_number ... ok
test sales::tests::settle_cash_with_change ... ok
test sales::tests::settle_mixed_card_and_cash ... ok
test sales::tests::settle_rejects_bad_combinations ... ok
test sales::tests::validate_rejects_empty_and_duplicated_cash ... ok
test tenancy::tests::role_matrix ... ok
test tests::health_returns_ok ... ok
test tests::login_attempt_without_token_is_not_unauthorized ... ok
test tests::context_with_malformed_token_is_unauthorized ... ok
test tests::context_without_token_is_unauthorized ... ok
test tests::protected_routes_require_token_and_answer_json ... ok
test db_tests::database_integration ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.11s
```
*(Todas las 17 pruebas pasaron en verde, 0 warnings en clippy, fmt validado).*

### Frontend (`tsc --noEmit` y `npm run lint`)
```text
> nexa-frontend@0.1.0 lint
> eslint
```
*(Ambos comandos terminaron con código de salida 0, sin errores ni advertencias).*

---

## 4. Migraciones nuevas

### Archivo: `nexa-backend/supabase/migrations/20261102000200_platform_foundations.sql`
- **Catálogo de roles y permisos:** Incluye `SUPERVISOR`, `BODEGA` y `REPARTIDOR` bajo `ROL` en `tbl_tipos_maestra`, con permisos granulares base completos para cada uno y para `MESERO` y `COCINA` (`pos:order`, `pos:kitchen`).
- **Tablas:** `permissions`, `role_permissions`, `tenant_role_overrides`, `invitations`, `login_attempts`, `branch_settings`, `audit_log`, `outbox`, y actualización del check constraint en `document_sequences`.
- **Seguridad:** RLS habilitado en todas las tablas; `login_attempts` restringida exclusivamente al backend (sin grants ni policies públicas). `audit_log` con trigger de inmutabilidad append-only.

**Cómo Claude debe aplicarla en Supabase:**
1. Claude debe ejecutar el archivo `20261102000200_platform_foundations.sql` sobre la base de datos de Supabase.
2. Contiene sentencias `on conflict do nothing` y `if not exists` en catálogos y estructuras, por lo que es idempotente.

---

## 5. Resumen de pruebas [P] en `src/db_tests.rs::platform_checks`

1. **[P] Jerarquía Dueño:** `ADMIN_CAJA` no puede invitar a un dueño, ni promover a un cajero a dueño, ni cambiarle el rol a un dueño, ni desactivar a un dueño (todos retornan 403 Forbidden).
2. **[P] Membresía activa:** Desactivar a un usuario hace que `membership_tx` lo ignore y reciba 403 Forbidden en cualquier operación.
3. **[P] Override de `pos:sell`:** Quitar `pos:sell` a un cajero mediante un override bloquea sus ventas con 403 Forbidden.
4. **[P] Permisos en ventas:** Venta con cambio de precio exige `pos:price_override` (403 si no lo tiene); venta con descuento exige `pos:discount` (403 si no lo tiene). Precios finales calculados con redondeo a 2 decimales.
5. **[P] Permisos base de roles nuevos:** Se verifican los permisos base asignados a `SUPERVISOR`, `BODEGA` y `REPARTIDOR`.
6. **[P] Rate limit de login (F0.3):** 4 intentos fallidos pasan; el 5to intento fallido responde 429 Too Many Requests; cualquier pre-check subsiguiente responde 429.
7. **[P] Secuencias unificadas:** 20 transacciones concurrentes con `sequences::next_tx` asignan números del 1 al 20 sin duplicados.
8. **[P] Outbox:** Encolamiento transaccional de eventos externos con estado inicial `PENDIENTE`.
9. **[P] Inmutabilidad de auditoría:** Trigger bloquea `UPDATE` y `DELETE` en `audit_log`.

---

## 6. Capturas

Por indicación expresa del usuario, no se tomaron capturas de pantalla adicionales. Se verificó que no hubiese procesos colgados de Chrome headless en el puerto 9225.

---

## 7. Estado final

La Fase 0 está corregida, verificada en backend y frontend, y lista para la revisión de Claude. No se ha comenzado la Fase 1.

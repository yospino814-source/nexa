# 01: Plataforma (fase 0)

> **TERMINADA.** Fase 0 aprobada por Claude el 2026-11-01. No se construye de nuevo: este documento sirve para consultar cómo funciona la plataforma. Lo pendiente está en `antigravity/modulos/00-INDICE.md`.

Esta fase prepara la base que necesitan todos los módulos siguientes. No agrega pantallas de negocio nuevas, salvo la de Usuarios y roles y la de Ajustes.

## Qué hace Vendix (resumen funcional)

- **Jerarquía.** Una organización (la persona jurídica, con NIT, DV, régimen, CIIU) tiene una o varias tiendas.
  - Cada tienda tiene un tipo (física, online, híbrida), municipio DANE, zona horaria y bodega por defecto.
  - Un **alcance operativo** define si inventario y contabilidad se manejan por tienda o centralizados en la organización.
  - El modo se elige en el onboarding: "una tienda" u "organización con varias tiendas".
- **Login.**
  - El mismo correo puede pertenecer a varias organizaciones. Si es ambiguo, el sistema muestra un selector de cuenta.
  - Tras 5 intentos fallidos, la cuenta queda bloqueada 30 minutos, y se registra cada intento con IP, navegador y motivo.
  - Las sesiones se pueden listar y revocar.
- **Roles y permisos.**
  - Unos 700 permisos con nombre `dominio:recurso:accion`, agrupados por módulo.
  - Roles de sistema asignables: admin, supervisor fiscal, gerente, supervisor, empleado, cajero, repartidor, mesero, cocina, recepcionista, camarera.
  - Cada rol tiene una lista explícita de permisos; ninguno hereda de otro.
  - "Dueño" no se puede asignar.
  - Si un rol de sistema nuevo no está en la lista blanca, no se puede asignar.
- **Visibilidad del menú.** Es aparte de la seguridad: por usuario se pueden ocultar módulos del menú. El sistema explica en español por qué un módulo no aparece: no es de tu industria, no está activado o no tienes permiso.
- **Onboarding guiado.** Pasos: verificar correo → elegir modo (una tienda o varias) → datos del usuario → datos fiscales del negocio → primera tienda → apariencia e industria → listo. Cada paso se guarda como borrador. Al terminar se activan los métodos de pago básicos.
- **Planes**, con funciones bloqueadas según el plan:

  | Plan | Precio | Tiendas | Usuarios | Además |
  |---|---|---|---|---|
  | Básico | 49.000/mes | 1 | 3 | — |
  | Pro | 119.000/mes | 3 | 10 | Inventario avanzado, IA con cupo |
  | Empresarial | 1.290.000/año | Ilimitadas | 50 | — |

  - Una sola prueba gratis por organización, de 14 días.
  - Estados de la suscripción: prueba, activa, gracia (aviso), suspendida, cancelada.
  - Suspendida: se bloquean las escrituras (todo lo que no sea GET) y la lectura sigue funcionando.
- **Auditoría.** Cada petición que modifica datos queda registrada con usuario, sucursal, IP y navegador. El administrador ve la bitácora.
- **Ajustes por tienda.** Un documento de configuración con bloques (pos, caja, inventario, checkout, impresión, propinas…), con valores por defecto que se mezclan con lo guardado.

## Mejora Nexa

1. **El aislamiento queda en la base de datos.** Ya tenemos RLS; se mantiene en cada tabla nueva, sin excepciones.
2. **Cada ajuste declarado se aplica.** Vendix declara ajustes que no usa (descuento máximo, devolución sin aprobación, cierre con conteo, una sesión por usuario). En Nexa, cada ajuste de la tabla de abajo tiene código que lo aplica y una prueba **[P]**.
3. **Numeración atómica.** Vendix calcula "último + 1" y choca con dos ventas simultáneas. En Nexa todo consecutivo sale de `document_sequences` con `update … returning`, igual que ya hace `sales.rs::next_receipt_number`.
4. **Permisos por nombre.** No se atan a rutas HTTP: renombrar una ruta no debe romper los permisos.

## Tareas

### F0.1 Permisos granulares
- **Migración:**
  - Tabla `permissions(code text pk, module, description)`.
  - Tabla `role_permissions(role_id → tbl_tipos_maestra, permission_code)`.
  - Tabla `tenant_role_overrides(tenant_id, role_id, permission_code, allowed bool)`, para que el dueño ajuste un rol en su negocio.
  - Semilla con los permisos de los módulos actuales y futuros. Mínimo:
    - `pos:sell`, `pos:price_override`, `pos:discount`, `pos:custom_item`, `pos:refund`
    - `cash:open`, `cash:close`, `cash:movement`, `cash:history`
    - `inventory:read`, `inventory:adjust`, `inventory:adjust_approve`, `inventory:transfer`
    - `purchases:create`, `purchases:approve`, `purchases:receive`, `purchases:pay`
    - `customers:read`, `customers:write`, `credit:sell`, `credit:collect`, `credit:write_off`
    - `reports:read`, `settings:write`, `users:manage`
- **Backend:** `tenancy::require(scope, "pos:refund")` resuelve según el rol más los overrides. **La matriz actual de `role_allows` debe seguir dando el mismo resultado [P]**: migra los `Action` existentes a códigos.
- **Endpoints:**
  - `GET /v1/permissions`
  - `GET /v1/roles/{code}/permissions`
  - `PUT /v1/roles/{code}/permissions` (solo DUENO, y nunca sobre DUENO **[P]**)

### F0.2 Usuarios y roles (pantalla 19, `/configuracion/usuarios`)
- Invitar usuario por correo, con rol y sucursal (membresía).
- Cambiar rol, desactivar.
- Ver la matriz de permisos del rol con interruptores por módulo.
- El dueño no puede quitarse su propio rol **[P]**.

### F0.3 Seguridad de login
- Tabla `login_attempts(email, ip, user_agent, success, reason, at)`.
- Bloqueo a los 5 fallos, durante 30 minutos. Como el login es de Supabase, esto vive en una ruta del frontend o en un hook, y el backend solo lo consulta.
- **Pregunta abierta:** si esto choca con Supabase Auth, anótalo y no lo implementes.

### F0.4 Ajustes por sucursal
- **Migración:** `branch_settings(branch_id pk, data jsonb, updated_by, updated_at)`.
- **Backend:** un struct `Settings` con `serde(default)` para cada bloque y `settings::load_tx(tx, branch)`.
- Bloques mínimos, cada uno con quién lo aplica:

  | Ajuste | Lo aplica |
  |---|---|
  | `pos.allow_anonymous_sales` (true) | `sales.rs` |
  | `pos.allow_alias_sales` (true) | `sales.rs` |
  | `pos.max_discount_pct` (15) | descuento en POS **[P]** |
  | `pos.allow_price_override` (false) | `sales.rs` **[P]** |
  | `pos.refund_requires_approval` (true) | devoluciones **[P]** |
  | `pos.auto_print` (true) | frontend |
  | `cash.require_session` (true) | `sales.rs` (ya existe; muévelo aquí) |
  | `cash.one_session_per_user` (true) | `cash.rs` **[P]** |
  | `cash.blind_close` (true) | frontend y backend |
  | `cash.close_tolerance` (0) | si la diferencia la supera, la nota es obligatoria **[P]** |
  | `inventory.allow_negative_stock` (false) | `inventory.rs` **[P]** |
  | `inventory.low_stock_default` (5) | existencias |
  | `tips.enabled` (false) | POS |
  | `tips.suggested_pct` (10) | POS |
  | `print.ticket_width_mm` (80 o 58) | CSS de la tirilla |

- **Endpoints:**
  - `GET /v1/settings?branch_id=`
  - `PUT /v1/settings` (permiso `settings:write`; valida tipos y rangos)
- **Pantalla `/configuracion/negocio`:** pestañas POS, Caja, Inventario, Impresión.

### F0.5 Auditoría
- **Migración:** `audit_log(id, tenant_id, branch_id, user_id, action text, entity text, entity_id, before jsonb, after jsonb, ip, at)`. Solo admite insert y select: sin update ni delete, ni siquiera para el dueño **[P]**.
- **Backend:** `audit::record_tx(tx, scope, action, entity, id, before, after)`. Llámala desde:
  - cierres de caja
  - retiros
  - ajustes de inventario
  - cambios de precio
  - devoluciones
  - anulaciones
  - cambios de rol
  - cambios de ajustes
- **Pantalla `/configuracion/auditoria`:** filtros por usuario, fecha y acción.

### F0.6 Numeración unificada
- Generaliza `document_sequences` para estos documentos:
  - `VENTA` (ya existe)
  - `DEVOLUCION`, `COMPRA`, `SEPARADO`, `COTIZACION`, `ACUERDO_PAGO`, `TRASLADO`, `GASTO`, `RUTA`
- **API interna:** `sequences::next_tx(tx, scope, kind) -> (prefix, number)`.
- Prueba **[P]**: 20 llamadas concurrentes no repiten ningún número.

### F0.7 Outbox de efectos externos
- **Migración:** `outbox(id, tenant_id, kind, payload jsonb, status PENDIENTE/ENVIADO/ERROR, attempts, next_attempt_at, last_error)`. `fiscal_documents` queda tal cual.
- Los efectos que salen del sistema se encolan en la misma transacción que los origina:
  - correo
  - WhatsApp
  - notificación push
  - DIAN
- El *worker* se hace en la fase 7. Por ahora solo se encolan.

### F0.8 Menú según rol
El menú lateral oculta lo que el rol no puede usar. El enlace "Mapa de pantallas" (`/modulos`) solo aparece con `NODE_ENV !== "production"`.

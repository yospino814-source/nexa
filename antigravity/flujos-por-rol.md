# Flujos por tipo de usuario: simulación, necesidades y cómo se atienden

> **Qué es este documento:** Claude simuló, leyendo el código, cómo usa Nexa cada tipo de usuario para vender en una licorería o una tienda. Para cada uno se listan lo que necesita, el flujo paso a paso, qué pasa hoy y en qué tarea se corrige.
>
> **Fecha:** 2026-11-08. **Código revisado:** backend `83f6550` y frontend `49efbcd`, más el trabajo sin commit de esa fecha (F3.2 a medias y el rediseño de Órdenes, Inventario y Gastos).
>
> **Cómo se hizo:** se leyeron las migraciones (roles y permisos reales), los permisos de cada endpoint, el menú, las pantallas de inicio, el guardia de sesión, las invitaciones, la caja y las devoluciones. **No se ejecutó la app**: Docker estaba apagado. Lo marcado "No revisado" se debe probar en el piloto.
>
> **Equivalencia con los módulos** (`antigravity/modulos/`): E0 = M01 · E1 = M02 · E2 a E7 y C1 a C5 = M03 · F3.1, C6 a C8 y A1 a A17 = M04 · E8 = M05 · F3.3 = M06 · F3.5 = M07 · C1 de la fase 2A = M08 · S1 = M10 · S2 = M11 · S3 = M12 · S4 = M13 · S5 = M14 · S6 = M15 · S7 = M16.
>
> **Para Antigravity:** este documento **no** se construye por sí solo. Cada problema apunta a una tarea con su especificación exacta: `fase-3-clientes-cartera.md` (E0 a E9) o `fase-S-salida.md` (S1 a S7). Si una tarea y este documento no coinciden, manda la tarea.

## 1. Tipos de usuario

| # | Usuario | Rol en Nexa | ¿Entra en la salida? |
|---|---|---|---|
| U1 | Persona que se registra para crear su negocio | Sin rol, hasta terminar el alta | Sí |
| U2 | Dueño de la licorería o la tienda | `DUENO` | Sí |
| U3 | Administrador o encargado (de confianza, con caja) | `ADMIN_CAJA` | Sí |
| U4 | Supervisor de turno | `SUPERVISOR` | Sí |
| U5 | Cajero | `CAJERO` | Sí |
| U6 | Bodeguero (recibe mercancía) | `BODEGA` | Sí |
| U7 | Empleado invitado que todavía no tiene cuenta | Sin rol, hasta aceptar | Sí |
| U8 | Cliente final del negocio (compra de contado o fiado) | No entra a Nexa | Sí: recibe la tirilla y los mensajes de cobro |
| U9 | Contador del negocio | No hay rol | Recibe los CSV del dueño |
| U10 | Operador de Nexa (vos, que vendés el sistema) | No hay rol en la app | Trabaja con procedimientos manuales (sección 6) |
| U11 | Mesero, cocina y repartidor | `MESERO`, `COCINA` y `REPARTIDOR` | **No**: fases 5 y 6 pospuestas |

## 2. Permisos reales por rol

Salen de las semillas de `role_permissions` en las migraciones, no de `tenancy::base_allows`. El backend lee la base (`permissions::load_effective_permissions_tx`). `DUENO` tiene siempre todos los permisos. `ADMIN_CAJA` recibe todos los que existían en la fase 0, más los que cada fase le da después.

**Hoy (incluye la migración de la fase 3, todavía sin aplicar):**

| Permiso | DUENO | ADMIN_CAJA | SUPERVISOR | CAJERO | BODEGA |
|---|---|---|---|---|---|
| `pos:sell`, `pos:order`, `pos:discount`, `pos:custom_item` | ✓ | ✓ | ✓ | ✓ | — |
| `pos:price_override` | ✓ | ✓ | — | — | — |
| `pos:refund` (aprobar devoluciones, anular) | ✓ | ✓ | ✓ | — | — |
| `cash:open`, `cash:close`, `cash:movement`, `cash:history` | ✓ | ✓ | ✓ | ✓ | — |
| `inventory:read` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `inventory:adjust` | ✓ | ✓ | ✓ | — | ✓ |
| `inventory:adjust_approve`, `inventory:transfer` | ✓ | ✓ | — | — | ✓ |
| `purchases:create`, `purchases:approve`, `purchases:pay` | ✓ | ✓ | — | — | — |
| `purchases:receive` | ✓ | ✓ | — (S4.1 se lo da) | — | ✓ |
| `customers:read`, `customers:write`, `credit:collect` | ✓ | ✓ | ✓ | ✓ | — |
| `credit:sell` | ✓ | ✓ | — (E1 se lo da) | ✓ | — |
| `credit:override` | ✓ | ✓ | ✓ | — | — |
| `credit:write_off`, `wallet:adjust` | ✓ | ✓ | — | — | — |
| `expenses:create` | ✓ | ✓ | ✓ | — | — |
| `expenses:approve` | ✓ | ✓ | — | — | — |
| `reports:read` | ✓ | ✓ | ✓ | — | — |
| `settings:write`, `users:manage` | ✓ | ✓ | — | — | — |

**Diferencia entre lo que dice el permiso y lo que hace el código:** todo el flujo de compras se valida con `inventory:adjust`, no con `purchases:*`. Por eso hoy `BODEGA` y `SUPERVISOR` **pueden crear y aprobar compras**. Se corrige en S4.1.

## 3. Simulación por usuario

Leyenda: ✅ funciona según el código · ❌ falla o falta · ⚠️ funciona con un defecto · 🔎 no revisado.

### U1. Persona que se registra para crear su negocio

**Necesita:** crear su cuenta sola, sin llamar a nadie, y quedar lista para vender en menos de 10 minutos.

| Paso | Hoy | Se atiende en |
|---|---|---|
| 1. Entra a `/login` y toca "Crear cuenta" | ❌ `/registro` muestra un diseño de referencia sin formulario | S1 |
| 2. Llena nombre, correo y contraseña | ❌ No existe | S1 |
| 3. Confirma el correo | ❌ No existe. Además, el correo de Supabase por defecto envía muy pocos correos por hora | S1 (Claude configura el SMTP) |
| 4. Crea el negocio y la primera sucursal | ❌ Solo con SQL a mano (`README.md`, "Conectar Supabase") | S1 |
| 5. Llena los datos del negocio (nombre, NIT) | ❌ El botón Guardar no guarda nada | S3 |
| 6. Carga productos con el CSV | ⚠️ La importación deja el stock sin costo promedio | C1 de la fase 2A (paso 7 de la fase 3) |
| 7. Invita al cajero | ❌ La invitación nunca se convierte en acceso | S2 |
| 8. Abre caja y vende | ✅ | — |

### U2. Dueño (`DUENO`)

**Necesita:**
- ver cuánto vendió y cuánto ganó, hoy y en el mes;
- saber quién le debe y cobrar;
- controlar a los cajeros (descuadres, devoluciones y anulaciones);
- reponer mercancía a tiempo;
- todo esto también desde el celular, sin estar en la tienda.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Entrar y ver el panel | ⚠️ Entra a `/panel`, pero con datos falsos en el modo real | S6 |
| Ver ventas por día, producto y cajero, y exportar a Excel | ❌ Las pantallas 02 y 03 son diseño | S7 |
| Aprobar desde el celular la devolución que pidió el cajero | ❌ Sin caja abierta, falla con `REFUND_NEEDS_OPEN_SESSION`. Con caja abierta, el dinero sale de **su** turno y no del cajón del cajero | S4.2 |
| Revisar el cierre de caja de cada cajero | ✅ `/caja` (historial y reporte Z) | — |
| Ver la cartera y cobrar por WhatsApp | ❌ Falta construir | F3.5 |
| Autorizar un fiado por encima del cupo | ❌ Falta construir | E2 y E8 |
| Crear compras, aprobarlas y pagarle al proveedor | ✅ | — |
| Editar el nombre y el NIT del negocio | ❌ No guarda | S3 |
| Abrir una segunda tienda | ❌ No hay forma de crear sucursales | S3 |
| Invitar, cambiar de rol y desactivar empleados | ❌ Invitar falla siempre en el modo real (el cuerpo no coincide) y nunca da acceso | S2 |
| Cerrar sesión desde el menú lateral | ❌ "Cerrar sesión" es un enlace a `/login` y **no cierra la sesión** | S4.4 |
| Ver la bitácora de auditoría | ✅ `/configuracion/auditoria` | — |
| Ver Restaurante y Rutas en el menú | ❌ Aparecen y llevan a diseños sin función (regla 6) | S4.4 |

### U3. Administrador o encargado (`ADMIN_CAJA`)

**Necesita:** lo mismo que el dueño en el día a día, sin poder quitarle el negocio al dueño.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Todo lo del dueño, salvo hacerse dueño | ✅ Comprobado en la fase 0 | — |
| Cambiar el NIT o la razón social | Se decide que **no**: solo el dueño | S3 (paso 4 de `PUT /v1/business`) |
| Crear sucursales | Se decide que **no**: solo el dueño | S3 |
| Revocar la invitación de un dueño | Se decide que **no** | S2 |

### U4. Supervisor de turno (`SUPERVISOR`)

**Necesita:**
- vender y manejar caja;
- aprobar devoluciones y anulaciones;
- autorizar fiados por encima del cupo;
- recibir mercancía;
- ver las ventas del día.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Ver "Ventas y órdenes" en el menú | ❌ El menú le permite `/ventas`, que no es un ítem, y no `/ordenes`: no ve el ítem | S4.4 |
| Aprobar devoluciones (`/ordenes/devoluciones`) | ❌ No llega desde el menú | S4.4 |
| Vender a crédito | ❌ Tiene `credit:override` pero no `credit:sell` | E1 y E6 |
| Recibir una compra | ⚠️ Hoy puede, pero solo porque compras usa `inventory:adjust`, y por eso también puede **aprobarla** | S4.1 (le da `purchases:receive` y le quita aprobar) |
| Ver "Restaurante" en el menú | ❌ Lo ve | S4.4 |
| Ver el panel | ✅ Tiene `reports:read` | — |

### U5. Cajero (`CAJERO`)

**Necesita:**
- empezar rápido en una terminal compartida, abrir caja y vender con el lector;
- cobrar con efectivo, datáfono, QR o mixto, y fiar a clientes conocidos;
- recibir abonos;
- reimprimir una tirilla;
- pedir una devolución;
- cerrar su turno y dejar la terminal lista para el siguiente.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Entrar: llega a `/pos` | ✅ | — |
| Abrir caja con la base | ✅ `/caja` | — |
| Vender con el lector y cobro mixto | ✅ (F1.14) 🔎 lector no probado con hardware | Piloto |
| Fiar a un cliente con cupo | ❌ No existe el método "Crédito" en el POS | E8 |
| Recibir un abono | ❌ Falta construir | F3.3 |
| Reimprimir o buscar una venta, o pedir una devolución | ❌ Tiene `cash:history`, pero el menú solo le deja `/pos`, `/panel` y `/clientes`: no llega a `/ordenes` | S4.4 |
| Saber si el dueño aprobó la devolución | ⚠️ No puede listar devoluciones (`pos:refund`). Abre de nuevo la venta en `/ordenes`, que muestra sus devoluciones (`for_order`, permiso `pos:sell`) | Así queda. Ver D9 |
| Ver "Panel principal" en el menú | ❌ Lo ve, pero no tiene `reports:read`: con datos reales daría error | S4.4 (se quita) |
| Escribir a mano `/configuracion/usuarios` | ⚠️ El backend lo rechaza, pero la pantalla carga y muestra errores | S4.3 (guardia de rutas) |
| Botón del candado "Bloquear terminal" | ❌ No hace nada | S4.6 (se quita) |
| Cerrar turno y cerrar sesión para el siguiente cajero | ⚠️ El cierre funciona. "Cerrar sesión" del `UserMenu` sí cierra la sesión; el del menú lateral no | S4.4 |
| Usar el POS en una tableta | ⚠️ Por debajo de 768 px no se ve si la caja está abierta | S4.6 |
| Seguir vendiendo cuando se cae el internet | ❌ No hay modo sin internet | D7 |

### U6. Bodeguero (`BODEGA`)

**Necesita:** recibir pedidos contra la orden de compra, ajustar el inventario con motivo y ver vencimientos.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Entrar: llega a `/inventario` | ✅ | — |
| Recibir una compra | ✅ | — |
| Crear y aprobar compras | ❌ Hoy **puede**, y no debería | S4.1 |
| Aprobar ajustes de inventario | ✅ Tiene `inventory:adjust_approve` (decisión de la fase 0) | — |
| Ver productos y vencimientos | ✅ | — |

### U7. Empleado invitado

**Necesita:** que el dueño le avise y entrar con su propio correo y contraseña.

| Paso | Hoy | Se atiende en |
|---|---|---|
| 1. El dueño lo invita | ❌ Falla con datos reales (el frontend no envía `full_name`) | S2 |
| 2. Se entera de la invitación | ❌ No se envía nada | S2 (mensaje para copiar o enviar por WhatsApp) |
| 3. Crea su cuenta con ese correo | ❌ No existe `/registro` | S1 |
| 4. Acepta y entra con su rol | ❌ Nada convierte la invitación en membresía; ve "Tu usuario no tiene un negocio asignado" | S2 |
| 5. Invitado como mesero, cocina o repartidor | ❌ Entraría a una pantalla que no existe | S2 (no se pueden invitar) y S4.5 |

### U8. Cliente final del negocio

**Necesita:**
- una tirilla clara;
- saber cuánto debe y hasta cuándo;
- recibir el cobro sin sentirse acosado;
- un comprobante de cada abono.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Tirilla de 58 u 80 mm con el nombre del negocio | ✅ 🔎 no probado con impresora | Piloto |
| Comprobante de abono | ❌ Falta construir | F3.3 (tirilla "ABONO") |
| Mensaje de cobro por WhatsApp con su saldo | ❌ Falta construir | F3.5 y C10 |
| Factura electrónica o documento POS de la DIAN | ❌ Espera la decisión del proveedor | Fase 4 (D5) |

### U9. Contador

**Necesita:** ventas, compras, gastos y la valorización del inventario por periodo, en Excel.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Recibir CSV de ventas, compras e inventario | ❌ No hay exportación | S7 (el dueño los descarga y se los envía) |
| Entrar con su propio usuario de solo lectura | No hay rol de contador | Fuera de la salida |

### U10. Operador de Nexa (vos)

**Necesita:** ver qué negocios hay, ayudarles sin conocer sus contraseñas y cortar el servicio a quien no pague.

| Flujo | Hoy | Se atiende en |
|---|---|---|
| Ver los negocios y sus usuarios | Solo con SQL | Sección 6 (consulta lista) |
| Ayudar a un negocio | Nunca entrar con la cuenta del cliente. El dueño invita a un correo de soporte con rol `ADMIN_CAJA` y lo desactiva al terminar | Sección 6 |
| Cobrar la suscripción y suspender | No existe | D8 |

## 4. Flujos que cruzan varios roles

### 4.1 Cambio de cajero en una terminal compartida

1. El cajero A cierra su turno en `/caja` (conteo ciego, como manda `cash.blind_close = true` por defecto). ✅
2. El cajero A toca "Cerrar sesión" en el `UserMenu`. ✅ El del menú lateral no cierra la sesión: S4.4.
3. El cajero B entra con su correo y abre su turno. ✅
4. Si el cajero A no cierra sesión, B vende a nombre de A. Por eso no hay botón de "bloquear" que deje la sesión abierta (S4.6).

### 4.2 Devolución con aprobación (`pos.refund_requires_approval = true` por defecto)

1. El cajero busca la venta en `/ordenes` y toca "Devolver". La devolución queda `PENDIENTE`. Hoy el cajero no llega a `/ordenes`: S4.4.
2. El dueño o el supervisor la aprueba en `/ordenes/devoluciones`, en la tienda o desde el celular.
3. El efectivo sale del turno del cajero que la pidió. Hoy sale del turno de quien aprueba, o falla: S4.2.
4. El cajero abre de nuevo la venta, ve "Aprobada" y entrega el dinero.

### 4.3 Fiado completo

1. Un dueño o administrador crea al cliente con cupo. El cajero **no** puede dar cupo (C6).
2. El cajero vende con "Crédito", con cupo de todo el negocio y bloqueo por mora (E2 y E8).
3. Si se pasa del cupo, el supervisor o el dueño pone "Autorizar igual" con un motivo, y queda en la auditoría (E2 y E8).
4. El cliente abona en caja; el efectivo entra al turno (F3.3).
5. El dueño ve la cartera, la antigüedad y el estado de cuenta, y cobra por WhatsApp (F3.5).
6. Una devolución de una venta fiada reduce primero la deuda (C5 y E4).

### 4.4 Internet caído

Hoy no se puede vender sin conexión. Ver D7.

## 5. Problemas encontrados, por prioridad

| # | Prioridad | Problema | Tarea |
|---|---|---|---|
| H1 | Bloquea la salida | Un negocio nuevo no se puede crear sin SQL | S1 |
| H2 | Bloquea la salida | Un empleado invitado nunca puede entrar, e invitar falla con datos reales | S2 |
| H3 | Bloquea la salida | El fiado no está terminado (backend a medias, sin POS, sin abonos ni cartera) | E0 a E9, F3.3 y F3.5 |
| H4 | Alta | "Cerrar sesión" del menú lateral no cierra la sesión: en una terminal compartida, el siguiente usuario queda con la sesión del anterior | S4.4 |
| H5 | Alta | Bodega y supervisor pueden crear y aprobar compras | S4.1 |
| H6 | Alta | Una devolución aprobada a distancia falla o descuenta el dinero del turno equivocado | S4.2 |
| H7 | Alta | Los datos del negocio no se guardan, aunque la pantalla dice que sí | S3 |
| H8 | Alta | El panel y los reportes muestran datos falsos en el modo real | S5 a S7 |
| H9 | Media | El cajero no llega a "Ventas y órdenes" y el supervisor tampoco | S4.4 |
| H10 | Media | El menú muestra Restaurante y Rutas, que no funcionan | S4.4 |
| H11 | Media | No hay guardia por pantalla: escribir la URL abre pantallas sin permiso | S4.3 |
| H12 | Media | Se pueden invitar roles sin pantallas | S2 y S4.5 |
| H13 | Media | No se pueden crear sucursales | S3 |
| H14 | Media | El rediseño en curso rompe "Nuevo producto" y muestra pedidos falsos en `/ordenes` con datos reales | E0 |
| H15 | Baja | Botón del candado sin función | S4.6 |
| H16 | Baja | En una tableta no se ve el estado de la caja | S4.6 |
| H17 | Baja | Las pantallas de diseño se ven en producción | S4.7 |
| H18 | Baja | Una membresía desactivada sigue viendo el nombre del negocio | S3 |

## 6. Procedimientos manuales del operador (los ejecuta Claude con permiso del usuario)

Hasta que exista un panel de operador, estas tareas se hacen con SQL en Supabase. **Nunca** se pegan contraseñas ni llaves en el chat.

**Listar negocios, con su dueño y su actividad:**

```sql
select t.id, t.name, t.created_at,
       (select u.email from public.memberships m join public.users u on u.id = m.user_id
         where m.tenant_id = t.id and m.role_id = public.tipo_id('ROL','DUENO') and m.active
         order by m.created_at limit 1) as dueno,
       (select count(*) from public.memberships m where m.tenant_id = t.id and m.active) as usuarios_activos,
       (select max(o.created_at) from public.orders o where o.tenant_id = t.id) as ultima_venta
  from public.tenants t
 order by t.created_at desc;
```

**Soporte a un negocio:** el dueño invita, desde `/configuracion/usuarios`, al correo de soporte de Nexa con rol `ADMIN_CAJA`. Al terminar, el dueño lo desactiva. Ni Claude ni el operador entran con la cuenta del cliente.

**Suspender un negocio que no paga:** pendiente de D8. No se hace mientras no esté decidido.

## 7. Decisiones que tomás vos

| # | Tema | Recomendación de Claude | Estado |
|---|---|---|---|
| D5 | Proveedor de facturación electrónica | MATIAS API (ver `REVISION-CLAUDE.md`, sección 12) | Por decidir |
| D6 | Dominio y VPS | Comprar el dominio ya: S1 y S2 lo necesitan para los enlaces de confirmación | Falta el dominio |
| D7 | Vender sin internet | No construirlo para la salida. Plan B en la tienda: datos del celular como respaldo. Si el piloto lo pide, se especifica una fase aparte, porque cambia la caja, el stock y la numeración | Por decidir |
| D8 | Cobro de la suscripción de Nexa y suspensión | Cobrar por fuera (transferencia o Nequi) mientras haya pocos clientes. Después se especifica: estado del negocio, días de gracia y bloqueo de escrituras | Por decidir |
| D9 | Devoluciones en tiendas sin supervisor | Dejar `refund_requires_approval = true`: el dueño aprueba desde el celular (S4.2). Quien quiera que el cajero devuelva solo, lo apaga en Configuración | Recomendación |
| D10 | Rol de contador de solo lectura | No para la salida. El dueño le envía los CSV (S7) | Recomendación |

# Fase S: alta de negocio, usuarios, permisos por rol y reportes para salir a vender

> **Requisito:** la fase 3 (reducida) debe estar aprobada. Rama `feat/fase-s-salida`, a partir de `feat/fase-3-clientes`. (Histórico: hoy el repositorio es único, `nexa/`.)
>
> **Migración única:** `20261109000100_salida.sql`.
>
> **Por qué existe esta fase:** sale de simular el uso de Nexa con cada tipo de usuario (`antigravity/flujos-por-rol.md`). Hoy un negocio nuevo solo se puede crear pegando SQL en Supabase, un empleado invitado nunca puede entrar, los datos del negocio no se guardan, cada rol ve pantallas que no puede usar, y el panel y los reportes del modo real usan datos falsos. Sin esto no se puede vender Nexa a una licorería o una tienda.
>
> Todo lo que no esté escrito aquí está prohibido. Si algo no se puede cumplir tal cual, detén esa tarea y pregunta en el reporte.

**Fuera de alcance:**
- planes, precios, pruebas gratis y cobro de la suscripción de Nexa (el cobro se hace por fuera mientras tanto);
- elegir industria, modo "varias tiendas", logo o colores en el alta;
- datos fiscales en el alta: se llenan después en `/configuracion/negocio`, que ya existe;
- invitar usuarios en el alta: se hace en `/configuracion/usuarios` (S2);
- enviar correos desde Nexa: la invitación se comparte con un mensaje copiable o por WhatsApp (S2);
- teléfono y correo de facturación del negocio (llegan con la fase 4);
- borrar o archivar sucursales;
- pantallas para los roles `MESERO`, `COCINA` y `REPARTIDOR`;
- modo sin internet;
- iniciar sesión con Google u otro proveedor;
- borrar un negocio;
- F7.4 a F7.7 (notificaciones, correo, resumen semanal y asistente IA);
- librerías de gráficas.

## Reglas comunes de la fase S

- Una sola migración, `20261109000100_salida.sql`. La crea M10, y M11 a M14 le agregan su bloque **al final**, en ese orden.
- Las funciones SQL nuevas son `security definer`, con `set search_path = ''`, `revoke ... from public, anon` y `grant execute ... to authenticated`.
- Los errores de la base que el backend traduce se leen con `sqlx::Error::Database(e)` y `e.message()`.
- Textos con voseo, como el resto de la app.

## Orden de las tareas

| Tarea | Tema |
|---|---|
| S1 | Alta de negocio: registro y configuración inicial |
| S2 | Invitaciones que funcionan |
| S3 | Datos del negocio y sucursales |
| S4 | Permisos y navegación por rol |
| S5 | Vista de ventas y métricas (F7.1, con cambios) |
| S6 | Panel principal con datos reales (F7.2, con cambios) |
| S7 | Reportes y exportación CSV (F7.3, con cambios) |

Un commit por tarea: `feat(fase-s): S<n> <resumen>`.

**La especificación de cada tarea está en su módulo:** S1 → `modulos/M10-S1-alta-negocio.md`, S2 → `M11`, S3 → `M12`, S4 → `M13`, S5 → `M14`, S6 → `M15`, S7 → `M16`, y el cierre en `M17`. Este documento solo guarda el contexto, las reglas comunes, el alcance y la lista de errores de la fase.

---

## Códigos de error nuevos

| Código | HTTP | Texto en pantalla |
|---|---|---|
| `ONBOARDING_INVALID` | 422 | Revisá los datos del negocio. |
| `ONBOARDING_ALREADY_MEMBER` | 409 | Tu usuario ya tiene un negocio. |
| `ROLE_NOT_AVAILABLE` | 422 | Ese rol todavía no está disponible. |
| `USER_ALREADY_MEMBER` | 409 | Esa persona ya trabaja en tu negocio. |
| `INVITATION_NOT_FOUND` | 404 | No encontramos esa invitación para tu correo. |
| `INVITATION_NOT_PENDING` | 409 | Esa invitación ya no está vigente. |
| `BUSINESS_INVALID` | 422 | Revisá los datos del negocio. |
| `BRANCH_INVALID` | 422 | El nombre de la sucursal debe tener entre 2 y 60 caracteres. |
| `BRANCH_DUPLICATED` | 409 | Ya existe una sucursal con ese nombre. |

`NIT_DV_INVALID` y `REFUND_NEEDS_OPEN_SESSION` ya existen: no los dupliques en `ERROR_MESSAGES`.

## Lista de verificación (modo demo, puerto 3200)

1. `/login` → "Crear cuenta" → `/registro` → "Crear cuenta" lleva a `/onboarding`.
2. `/onboarding` con datos válidos muestra "Tu negocio está listo" y sus 4 enlaces abren pantallas que existen.
3. `/panel` muestra los KPI, la gráfica por hora, el top 10, los métodos de pago con "Crédito", el stock bajo y la cartera vencida.
4. `/ventas` y `/ventas/productos` cargan, y "Exportar CSV" descarga un archivo que Excel abre con tildes y columnas separadas.
5. `/configuracion/usuarios` → "Invitar usuario" con nombre, correo, rol "Cajero" y "Todas las sucursales" → "Invitación creada", con "Copiar mensaje" y "Enviar por WhatsApp". El selector de rol no ofrece Mesero, Cocina ni Repartidor.
6. `/configuracion/negocio` → cambiar el nombre comercial y guardar → recargar la página: el nombre nuevo sigue ahí. "Nueva sucursal" agrega una sucursal al selector.
7. El menú lateral no muestra "Restaurante" ni "Rutas y mayorista", y "Cerrar sesión" deja la sesión cerrada: al volver a `/panel`, pide entrar de nuevo.
8. La barra del POS no tiene el botón del candado.

El menú por rol (S4.4) y el guardia de rutas (S4.3) no se pueden probar en modo demo, porque el demo entra como dueño. Claude los revisa con usuarios reales.

## Reporte

`antigravity/REPORTE-FASE-S.md`, con el formato de la sección 6 del LEEME. Agrega la salida de estos comandos. Las tres deben salir vacías:

```sh
grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
grep -n "Action::ManageInventory" src/procurement.rs
grep -n "isAllowedForRole\|/mesas\|/rutas" frontend/components/nexa/sidebar.tsx
```

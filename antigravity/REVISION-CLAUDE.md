# Revisión de Claude: fases 0 a 3

Archivo de trabajo de Claude. Aquí queda lo que está bien y lo que hay que cambiar. Se actualiza en cada revisión.

## 00. Revisión del 2026-11-10: M01 a M10

**Revisado:** backend `b3c4cda` (fin de la fase 3) y `dace84a` (M10); frontend `aab0a06` y `8e85d55`.

**Verificación propia de Claude** (en una copia aparte del commit `dace84a`, sin tocar el trabajo en curso):
- `fmt --check`, `clippy -D warnings` y `cargo test` pasan: **44 de 44, con la base de datos real** (12,9 s).
- La primera corrida falló con "no se pudo conectar como administrador". Fue un choque con el `test.sh` que Antigravity corría al mismo tiempo: los dos usan el contenedor `nexa-test-pg`. Al repetirla, pasó. **Regla:** nunca correr dos `test.sh` a la vez.
- `tsc` y `lint` del frontend pasan (corridos sobre la carpeta real, con M11).

**Fase 3: aprobada.** Revisado en el código:
- migración con las 6 correcciones;
- crédito con el orden de validación de la spec, `customer_credit_position`, bloqueo del cliente, fecha local y sin `unwrap`;
- devoluciones con `reduce_receivable_tx`, el pago mixto y `fetch_one`;
- clientes con la posición de todo el negocio, el bloqueo al archivar y la regla de cupo;
- abonos idempotentes, con bloqueo, turno de caja y aplicación a cuotas;
- cartera con fecha local, los tramos y el enlace de WhatsApp;
- CSV por `apply_delta_tx`;
- frontend del rediseño protegido en el modo real, y crédito en el POS;
- las 13 reglas [P] de crédito, 8 de abonos, 4 de cartera y 7 del alta.

Observaciones menores, sin bloqueo:
- `REPORTE-FASE-3.md` describe mal las tareas pospuestas: dice "grupos", "centrales de riesgo" y "cobranza", cuando F3.6 es saldo a favor, F3.7 separados y F3.8 cotizaciones;
- `customers::archive` lee separados y saldo a favor sin la función de todo el negocio. Hoy esas tablas están vacías; corregirlo cuando vuelvan F3.6 y F3.7.

**M10 (S1 alta de negocio): bien, con un error.** Con usuarios reales, la pantalla "Tu negocio está listo" nunca se ve: tras crear, `boot()` deja el estado en `ready` y el efecto redirige a `/panel`. En modo demo no pasa. Es un hueco de la spec. Corrección: `modulos/M10A-correccion-onboarding.md`.

**Proceso:** Antigravity empezó M10 sin esperar la aprobación de la fase 3 (M09 pedía alto), y ya commiteó M11 (`a24f401`, sin revisar).

**M11 (S2 invitaciones), revisado el mismo día.** Commit `a24f401` y `b8ebbd1`. Verificación de Claude: `fmt`, `clippy` y 44 de 44 con la base de datos real pasan. El backend, la migración, las 8 reglas [P] y el flujo del invitado están bien. Errores, corregidos en `modulos/M11A-correccion-invitaciones.md`:
1. Sin commit, `src/main.rs` declara `pub mod invitations;` dos veces: el backend no compila. Hay que descartarlo.
2. "Cambiar rol" ofrece todos los roles (Mesero, Cocina, Repartidor y, a un administrador, Dueño).
3. La pestaña "Roles y permisos" muestra los roles pospuestos.
4. El mensaje copiado desde la lista de pendientes no trae el nombre.
5. Quedó código con `branch_ids`, un formato que el backend nunca devolvió.

**Pendiente de Claude, con permiso del usuario:** aplicar a Supabase las migraciones de las fases 0 a 3 y la de la fase S cuando termine. Nada de esto está en producción.

## 0. Estado al 2026-11-08 (lo más nuevo; manda sobre las secciones de abajo)

**Nada nuevo en git desde el 2026-11-03.** Las dos ramas siguen en `83f6550` (backend) y `49efbcd` (frontend), sin subir.

**Backend, sin commit (2026-11-04, 08:58):** F3.2 empezado. Hay unas 870 líneas en `credit.rs` (nuevo), `sales.rs`, `refunds.rs`, `routes.rs`, `customers.rs` y `db_tests.rs`. Ya tiene la función de cuotas con sus 3 pruebas, el pago `CREDITO`, las validaciones de cupo y mora, la creación de la cuenta, la devolución con solo crédito, la anulación y 5 pruebas [P]. **No se compiló**: Docker estaba apagado. Sus fallas están en `fase-3-clientes-cartera.md`, "Correcciones del 2026-11-08" (E1 a E9). Las más graves:
- la migración no tiene **ninguna** de las correcciones del 2026-11-04. Con `principal > 0`, devolver completa una venta a crédito da error 500;
- el cupo se suma por sucursal y con `current_date` (B1 y A6 siguen abiertos), y no bloquea la fila del cliente (B2);
- el pago mixto con crédito sigue devolviendo todo en efectivo (B4);
- `select min(due_date)` con `fetch_optional` falla cuando ya no quedan cuotas con saldo;
- no hay frontend de F3.2, y de F3.1 siguen pendientes A1 a A17.

**Frontend, sin commit (2026-11-08, hasta las 21:40):** cambios que ningún documento pide. No se sabe si son del usuario o de Antigravity:
- una vista "Gestión de Pedidos (Omnicanal)" solo con datos falsos, que queda por defecto en `/ordenes`;
- un diálogo de producto que llama a `/v1/inventory/products`, que **no existe**. Reemplaza el botón que lleva a la ficha real, así que en el modo real "Nuevo producto" deja de funcionar.

Confirmado por el usuario: es un rediseño que se está haciendo a propósito (a las 21:56 también cambió Gastos, con los campos `payment_method` y `attachment_name`, que no existen en el backend). E0 pide commitearlo aparte y corregir lo que rompe el modo real; no se guarda en `stash`.

**Hueco nuevo encontrado:** no existe el alta de negocio. `/registro` y `/onboarding` siguen como diseño, el login ya enlaza a `/registro` (lleva a una página de referencia), y un negocio solo se crea pegando SQL en Supabase. Se especificó en `fase-S-salida.md`, tarea S1.

**Paso 7 de la ruta (revisado):** F7.1 a F7.3 no dependen de las fases pospuestas, salvo una cosa: `v_sales` usa `orders.delivery_fee`, que llega en la fase 5B. La fase S lo quita.

### Ruta de salida actualizada (reemplaza la sección 10)

| # | Paso | Quién | Documento |
|---|---|---|---|
| 1 | Cerrar la fase 3 | Antigravity | `modulos/M01` a `M09` |
| 2 | Revisar y aprobar la fase 3 | Claude | — |
| 3 | Fase S: alta de negocio, invitaciones, datos del negocio y sucursales, permisos y menú por rol, panel y reportes reales | Antigravity | `modulos/M10` a `M17` |
| 4 | Revisar y aprobar la fase S | Claude | — |
| 5 | Aplicar a Supabase las 8 migraciones pendientes (las 7 de la sección 8 más `20261109000100`), con permiso del usuario. Configurar *Site URL*, *Redirect URLs*, el SMTP propio y la protección de contraseñas filtradas | Claude | — |
| 6 | Comprar el dominio y publicar en el VPS (D6). Build en el VPS, no en el PC | Claude y usuario | `despliegue/` |
| 7 | Primer negocio piloto **no obligado a facturar**: lector, impresora, caja, fiado y reportes | Usuario y Claude | — |
| 8 | Elegir el proveedor DIAN (D5) y confirmar con un contador | Usuario | Sección 12 |
| 9 | Reescribir la fase 4 con el proveedor real y construirla | Claude y Antigravity | `fase-4-facturacion.md` |

Los pasos 6 y 8 no dependen de los anteriores y pueden empezar ya.

### Simulación por rol (2026-11-08)

Se simuló, leyendo el código, cómo usa Nexa cada tipo de usuario: persona que se registra, dueño, administrador, supervisor, cajero, bodega, empleado invitado, cliente final, contador y operador de Nexa. El detalle está en `antigravity/flujos-por-rol.md`. Hallazgos nuevos, todos especificados en `fase-S-salida.md`:
- **Invitaciones (S2):** nada convierte una invitación en membresía, así que un empleado invitado nunca entra. Además, el frontend manda un cuerpo distinto del que espera el backend, y con datos reales invitar falla siempre.
- **Datos del negocio (S3):** "Guardar" en Datos de la empresa solo muestra un aviso, `tenants` no tiene política de `update`, y no se pueden crear sucursales.
- **Permisos (S4.1):** compras se valida con `inventory:adjust`, así que bodega y supervisor pueden crear y aprobar compras.
- **Devoluciones (S4.2):** al aprobar a distancia, el efectivo sale del turno de quien aprueba, o falla si no tiene caja abierta.
- **Menú y sesión (S4.3 a S4.7):**
  - "Cerrar sesión" del menú lateral no cierra la sesión;
  - el cajero y el supervisor no llegan a "Ventas y órdenes";
  - se ven Restaurante y Rutas;
  - no hay guardia por pantalla;
  - el candado no hace nada;
  - las pantallas de diseño se ven en producción.

Decisiones nuevas del usuario: D7 (vender sin internet), D8 (cobro y suspensión de la suscripción), D9 (devoluciones sin supervisor) y D10 (rol de contador). Están en la sección 7 de `flujos-por-rol.md`.

- **Fecha:** 2026-11-04
- **Revisado:** `feat/fase-3-clientes` en los dos repositorios (backend `83f6550`, frontend `49efbcd`)
- **Comparado contra:** `antigravity/00-LEEME.md`, `TERMINADA-fase-2-inventario-compras.md` y `fase-3-clientes-cartera.md`

## 1. Veredicto

**La fase 3 no está terminada y no se aprueba.** El backend de la rama **no compila**, así que F3.1 se entregó sin ejecutarse nunca. De las 8 tareas solo existe F3.1 (clientes). La migración sí trae las tablas de toda la fase, pero F3.2 a F3.8 no tienen código, ni en local ni en GitHub, ni en stash. Tampoco existe `REPORTE-FASE-3.md`.

**Trabajo en curso sin revisar:** Antigravity está construyendo F3.2 sin commit (`credit.rs`, `sales.rs`, `refunds.rs`, `db_tests.rs`). Ya corrigió la compilación de `customers.rs` en su copia de trabajo. Esta revisión corresponde al commit `83f6550`; ese trabajo se revisa en la entrega.

No empezar la fase 4 todavía. Antes hay que corregir F3.1 (sección 5). La spec de la fase 3 ya se corrigió el 2026-11-04 (sección 6) y quedó reducida para salir a vender.

| Fase | Estado | Nota |
|---|---|---|
| 0 Plataforma | Aprobada el 2026-11-01 | No se revisó de nuevo hoy |
| 1 POS y caja | Completa | No se revisó de nuevo hoy, salvo anulación y devoluciones |
| 2A Inventario | Con un defecto | Importación CSV (sección 7, C1) |
| 2B Compras | Sin aprobación registrada | Revisión parcial hoy; ya está en `main` por el PR #1 |
| 3 Clientes y cartera | **1 de 8 tareas** | F3.1 con correcciones pendientes |

### Avance de la fase 3

| Tarea | Migración | Backend | Frontend | Pruebas |
|---|---|---|---|---|
| F3.1 Clientes | Sí | No compila; con defectos | Sí, con defectos | 4 de 4 [P] escritas; pasan solo en la copia corregida |
| F3.2 Venta a crédito | Sí | No | No | 0 de 5 |
| F3.3 Abonos | Sí | No | No | 0 de 4 |
| F3.4 Acuerdos y castigo | Sí | No | No | 0 de 3 |
| F3.5 Cartera | No aplica | No | No | 0 de 1 |
| F3.6 Saldo a favor | Sí | No | No | 0 de 4 |
| F3.7 Separados | Sí | No | No | 0 de 6 |
| F3.8 Cotizaciones | Sí | No | No | 0 de 4 |

## 2. Resultado de las verificaciones

| Verificación | Resultado |
|---|---|
| `cargo fmt --check` | **Falla:** 20 diferencias (13 en `customers.rs`, 3 en `routes.rs`, 2 en `db_tests.rs`, 2 en `sales_history.rs`) |
| `cargo clippy` | **Falla:** no compila (2 errores en `customers.rs`) |
| `cargo test` | **Falla:** no compila (20 errores) |
| `tsc --noEmit` y `npm run lint` | Pasan |
| `cargo test` en una copia desechable con arreglos mínimos | **37 de 37**, con BD real y todas las migraciones |

La copia desechable solo arregló la compilación (A0). Sirve para saber que, una vez compile, F3.1 y las pruebas de las fases 0 a 2 pasan, y que la migración de la fase 3 corre limpia. El repositorio no se tocó. Clippy no se corrió sobre la copia.

Sobre esa copia se agregaron sondas temporales. Reprodujeron A1, A2, A3, A4 y B11. También comprobaron que el filtro `EN_MORA` devuelve saldo y días de mora correctos (50.000 y 10 días).

## 3. Qué está bien

**F3.1 backend**
- `normalize_customer` sigue el orden de la spec. El DV de NIT, `CUSTOMER_DV_CONFLICT` y `NIT_DV_INVALID` están bien.
- `person_type` nunca se acepta del cliente.
- `missing_fiscal` tiene el orden y los textos exactos.
- `upsert` no sobrescribe valores existentes y tiene prueba.
- `PUT` devuelve 403 si cambia el cupo sin `credit:override`.
- `archive` revisa cartera, separados y saldo a favor.
- Las 4 reglas [P] de `f3_customers` están escritas y pasan en la copia corregida.

**Migración `20261106000100_fase3_clientes.sql`**
- Es una sola, cubre toda la fase y usa claves foráneas compuestas.
- Las 9 tablas nuevas tienen RLS y ningún `grant` a `anon`.
- `refunds_method_check` es el nombre correcto: el check original es de columna, y Postgres le da ese nombre.

**F3.1 frontend**
- `/clientes` y `/clientes/[id]` existen, y la pantalla 07 quedó en `"react"`.
- El POS ya usa `upsert`.
- Los 22 códigos de error de la fase ya están en `ERROR_MESSAGES`.

**Base lista para F3.2 a F3.8 (comprobado en el código)**
- `money_floor`, la función `powi` de `rust_decimal` (feature `maths`) y las secuencias `SEPARADO`, `COTIZACION` y `ACUERDO_PAGO`.
- Los movimientos de caja `ABONO_CARTERA`, `ABONO_SEPARADO` y `RECARGA_SALDO`.
- Las cabeceras maestras `PAGO` y `VENTA`, y las funciones `confirm_tx`, `consume_order_tx` y `record_movement_tx`.
- El valor de la prueba `schedule_fixed_payment_known_value` es correcto: 94.559,5966 redondea a $94.559,60.

**Fases 2A y 2B**
- Sin `f64` para dinero y sin políticas abiertas en tablas de negocio.
- `update stock` sobre `on_hand` solo aparece en `apply_delta_tx`.
- Las reglas [P] marcadas en total coinciden con la spec (44 pedidas, 45 marcadas).
- Las pruebas de BD ya no se omiten en silencio si `DATABASE_URL` está definida.

## 4. Cómo leer las prioridades

- **Alta:** pierde dinero, salta un permiso o rompe un flujo real.
- **Media:** dato incorrecto o regla del LEEME incumplida.
- **Baja:** limpieza.

## 5. Qué cambiar en F3.1

| # | Prioridad | Dónde | Problema | Corrección |
|---|---|---|---|---|
| A0 | **Bloqueante** | `customers.rs:788`, `customers.rs:884`, `db_tests.rs` (18 errores entre las líneas 2865 y 4375) | La rama no compila. `Some(ref dt)` no se admite en edición 2024. Además, F3.1 cambió `CreateCustomerInput` y el retorno de `create` sin ajustar las pruebas viejas. | Quitar `ref`. En las pruebas, usar `cust.customer.id` y derivar `Default` en `CreateCustomerInput` con `..Default::default()`. Correr `fmt`. Explicar el ajuste en el reporte. |
| A1 | Alta | `customers.rs:687`, `customers.rs:995`, `customer-form-dialog.tsx:440` y `:456` | **Reproducido:** un cajero creó un cliente con cupo de 9.999.999. Puede **crear** con cualquier cupo porque `POST` y `upsert` guardan `credit_limit` sin pedir `credit:override`, y el formulario le habilita el campo al crear. El permiso solo se revisa en `PUT`. Es un hueco de mi spec. | Si `credit_limit` no es 0 o `credit_block_days` no es 30 y no tiene `credit:override`: 403. Deshabilitar los campos también al crear. Agregar prueba. |
| A2 | Alta | `customers.rs:1051`, `customers.rs:634` y `customers.rs:530` | **Reproducido:** con una deuda de 100.000 en otra sucursal, un cajero atado a su sucursal ve saldo 0 y logra archivar al cliente. RLS le oculta esa cartera. | Ver B1. |
| A3 | Media | `customers.rs:963` | **Reproducido:** `upsert` que llena un NIT en un cliente sin documento deja `person_type = NATURAL`, porque el `update` no incluye esa columna. | Recalcular `person_type` cuando se llena el documento. |
| A4 | Media | `customers.rs:925` y `customers.rs:917` | **Reproducido:** `upsert` que encuentra por correo a un cliente con CC y recibe un NIT copia el `dv` y rompe el check `dv is null or doc_type = 'NIT'`: error genérico. Si el documento es de otro cliente, sale un error genérico de `customers_doc_idx` y no `CUSTOMER_DUPLICATED`. | Copiar el documento como bloque (tipo, número y DV). Antes de llenarlo, buscar duplicado y responder `CUSTOMER_DUPLICATED`. |
| A5 | Media | `customers.rs:468` y `customers.rs:478` | Un `PUT` sin `credit_limit` lo deja en 0. Si lo hace el dueño, borra el cupo sin avisar. | En `PUT`, si no viene, conservar el valor guardado. |
| A6 | Media | `customers.rs:537` y `customers.rs:539` | Los días de mora usan `current_date` (UTC). En Colombia, desde las 7 p. m. cuenta un día de más. La spec pide la fecha local de la sucursal. | Usar `(now() at time zone b.timezone)::date`, como `sales_history.rs`. |
| A7 | Media | `customers.rs:1042` | `archive` valida saldos sin bloquear con `for update`. Una venta a crédito simultánea se cuela. | Bloquear la fila del cliente al inicio. |
| A8 | Media | `customers.rs:1125` | Desarchivar con un documento ya usado por otro cliente activo da un 409 genérico. | Validar y responder `CUSTOMER_DUPLICATED`. |
| A9 | Media | `customer-form-dialog.tsx:156` | `parseInt(...) \|\| 30` convierte 0 en 30, y 0 es un valor válido. | Usar `Number.isNaN` y conservar el 0. |
| A10 | Media | `lib/api/index.ts:3132`, `:3186`, `:3022` | El modo demo está incompleto: `upsert` nunca da `UNCHANGED` ni llena campos, `missing_fiscal` siempre sale vacío, las compras salen vacías y `archive` no valida saldo. Además, los mocks van en `index.ts`, no en `lib/api/mock-customers.ts`. | Completar y mover al archivo de mocks. |
| A11 | Media | `customers-screen.tsx` | La tabla pide 50 clientes y no tiene paginación: del 51 en adelante solo se llega buscando. | Agregar paginación con `page`. |
| A12 | Media | `README.md` | Ningún endpoint de clientes está documentado. | Documentar los 8. |
| A13 | Baja | `customer-dialog.tsx:134` | Con `UNCHANGED` el POS dice "Cliente actualizado con los datos nuevos". | Sin cambios: seleccionar al cliente sin mensaje. |
| A14 | Baja | `customers.rs:211`, `:428`, `:443`, `:458` | `len()` cuenta bytes, y la BD cuenta caracteres. Un texto con tildes se rechaza antes de tiempo. | Usar `chars().count()`. |
| A15 | Baja | `customer-screen.tsx:555` a `:610` | Las pestañas Cartera, Separados y Saldo muestran textos como "Módulo (F3.6)". La regla 6 prohíbe funciones a medias visibles. | Se reemplazan al construir cada tarea. Si la fase se pausa, ocultarlas. |
| A16 | Baja | `customer-form-dialog.tsx:427`, `customer-screen.tsx:331` y `:358` | El usuario ve el código `credit:override`, y hay textos en tuteo ("Haz clic", "Escribe"). | Texto sin código de permiso. Dejar voseo: "Hacé clic", "Escribí". |
| A17 | Baja | `db_tests.rs:9342` | Faltan pruebas de los filtros de estado, de la búsqueda por documento en `upsert` y de `/purchases`. | Agregarlas a `f3_customers`. |
| A18 | Baja | Migración, línea 8 | Usa `do update` y la spec pide `do nothing`. | Ajustar. La migración aún se puede tocar porque es de esta misma entrega. |

## 6. Qué cambiar en la spec de la fase 3 (trabajo de Claude)

Estos errores eran de `fase-3-clientes-cartera.md`, no de quien programa. **Corregidos en la spec el 2026-11-04**, en "Correcciones del 2026-11-04": B1 a B4, B8 y B10 a B15. **Pendientes para cuando vuelvan F3.4, F3.6 y F3.7:** B5, B6, B7 y B9.

| # | Prioridad | Tarea | Problema | Corrección para la spec |
|---|---|---|---|---|
| B1 | Alta | F3.1, F3.2 | `receivables` usa RLS por sucursal, pero el cliente y su cupo son de todo el negocio. Quien debe el cupo completo en una sucursal puede volver a gastarlo en otra. | Crear una función `security definer` que devuelva `balance` y `max_days_overdue` de todo el negocio, y usarla en cupo, lista, detalle y archivo. |
| B2 | Alta | F3.2 | No se indica qué fila bloquear al validar el cupo. Dos ventas simultáneas pueden pasar el mismo cupo. | Bloquear la fila del cliente con `for update` antes de calcular. |
| B3 | Alta | F3.2 | Devolver toda una venta a crédito deja `principal = 0` y viola `check (principal > 0)`. Además, el interés queda cobrándose sobre mercancía devuelta. | Cambiar a `principal >= 0`, definir el estado final de la cuenta y reducir `interest_total` en proporción. |
| B4 | Alta | F3.2 | Con pago mixto solo se admite devolver en efectivo. En una venta de 100.000 (20.000 en efectivo y 80.000 a crédito), la caja entregaría 100.000 y la deuda seguiría en 80.000. | La devolución reduce primero el saldo de la cuenta. El efectivo solo cubre el resto, hasta lo realmente recibido. |
| B5 | Alta | F3.6 | Anular o devolver una venta pagada con `SALDO` no devuelve el saldo. `void_tx` no toca la billetera, y `ORIGINAL` no tiene efecto definido para `SALDO`. | Anulación: `wallet::apply_tx(DEVOLUCION, +monto, 'VOID', order_id)`. `ORIGINAL` con `SALDO` actúa igual que `SALDO_A_FAVOR`. |
| B6 | Alta | F3.7 | La spec dice que la orden del separado queda `CREADA`, pero `confirm_tx` la pasa a `EN_PREPARACION` (`orders.rs:484`), y `can_transition` no tiene camino directo a `COBRADA`. En la fase 5 entraría a cocina. | Función nueva que reserve sin cambiar el estado, y paso explícito a `COBRADA` al completar. Quitar el paso "elimina el vencimiento": `confirm_tx` ya reserva sin vencimiento. |
| B7 | Alta | F3.6 | La recarga no es idempotente de verdad: sin índice único, dos reintentos simultáneos acreditan dos veces, y no hay `client_request` para detectar otro cuerpo. `wallet/adjust` no recibe `id`. | Índice único parcial sobre `ref_id` cuando `kind = 'RECARGA'`, guardar `client_request` y agregar `id` a `adjust`. |
| B8 | Media | Permisos | `SUPERVISOR` recibe `credit:override` pero no `credit:sell`: no puede vender a crédito ni ver la vista previa de un acuerdo. | Darle también `credit:sell`. |
| B9 | Media | F3.4, F3.6 | `POST /agreements` y `wallet/topup` no reciben `branch_id`, pero necesitan sucursal para el consecutivo y el turno. | Agregar `branch_id` y usar `begin_in_branch`. |
| B10 | Media | F3.5 | El estado de cuenta no tiene tipo para devoluciones, así que el saldo final no cuadraría con la cuenta. | Agregar el tipo `DEVOLUCION`. |
| B11 | Media | F3.1 | **Reproducido:** el paso 2 de normalización corta en el guion con cualquier documento. El pasaporte `ABCDEF-123456` se guarda como `ABCDEF`, y `AB-123456` se rechaza con un error genérico. | Separar el DV solo con `doc_type = NIT`. |
| B12 | Media | F3.5, F3.8 | `wa.me/57<phone>` duplica el indicativo si el teléfono ya trae 57, y un fijo no tiene WhatsApp. | Quitar el 57 inicial y exigir celular de 10 dígitos que empiece por 3. Si no: `CUSTOMER_NO_PHONE`. |
| B13 | Media | F3.2 | Faltan validaciones del plan: `first_due_date` en el pasado, o `credit_plan` sin pago `CREDITO`. | 422 `CREDIT_PLAN_INVALID` en los dos casos. |
| B14 | Media | Migración | Ninguna tabla nueva tiene índice por `customer_id`, y `GET /v1/customers` agrega la cartera en cada búsqueda. | Índices en `receivables`, `layaways`, `wallet_movements` y `receivable_payments`. |
| B15 | Baja | F3.2 | En `CUOTA_FIJA` con monto y tasa muy bajos, `money(A) × n` puede dar menos que el capital: 10 pesos al 0,01 % en 3 cuotas da interés de −0,01 y viola el check. | `total = max(money(A) × n, principal)`. |

## 7. Qué cambiar en las fases 2A y 2B

| # | Prioridad | Dónde | Problema | Corrección |
|---|---|---|---|---|
| C1 | Alta | `csv_import.rs:834` | La importación escribe `on_hand` directo, sin `apply_delta_tx`. La spec (línea 277) lo exige. El stock importado queda sin `avg_cost`, sin capa de costo y sin alertas. Los reportes 2A y 2B dicen que `apply_delta_tx` es el único escritor, pero su `grep` buscaba `update stock` y este es un `insert ... do update`. | Pasar por `apply_delta_tx` con `AJUSTE_ENTRADA`, causa "Carga inicial" y costo de la fila. Agregar prueba de `avg_cost`. |
| C2 | Media | `payables.rs:266` a `:394` | Vencimientos y antigüedad usan `current_date` (UTC), igual que A6. | Usar la fecha de la sucursal. |
| C3 | Media | `procurement.rs:1376`, `:1410`, `:1614`, `:1632`, `:1812`, `:1830`, `:2785`; `csv_import.rs:651`; `sales.rs:1131`, `:1466` | Hay 10 `unwrap` fuera de pruebas, y el LEEME los prohíbe. | Devolver error con `?` o `ok_or`. |
| C4 | Media | `README.md` | 29 de 107 rutas no aparecen. Incluye cupones, promociones, tarifas, bodegas y opciones. Sale de una búsqueda literal de cada ruta. | Documentarlas. |
| C5 | Baja | `db_tests.rs` | El reporte 2B asigna 9 reglas a `f2_purchases` y hay 4 comentarios `[P]`; en `f2_suppliers`, 3 y 2. Los códigos de error sí se prueban, pero no revisé regla por regla. | Marcar cada regla con su comentario `[P]`. |

## 8. Proceso y repositorios

- **Repositorio único:** todo se sube a `DNeir/nexa-backend`, con el frontend en `frontend/feat/*`. Es la forma de trabajo acordada, no un error. El LEEME ya lo dice.
- **Aprobación:** el reporte 2B termina en "alto obligatorio" y la fase 3 empezó sin una aprobación registrada.
- **Verificación:** el commit de F3.1 se hizo sin correr `test.sh`, porque el código no compila. El LEEME lo exige antes de cada commit. El reporte 2B, además, corrió `fmt`, que reformatea, en vez de `fmt --check`.
- **Sin subir:** `feat/fase-3-clientes` solo existe en local, en los dos repositorios.
- **Supabase de producción:** tiene 8 de las 15 migraciones. Faltan `20261102000100`, `20261102000200`, `20261102000300`, `20261103000100`, `20261104000100`, `20261104000200` y `20261106000100`: nada de las fases 0 a 3 está en producción. El asesor de seguridad solo avisa que la protección de contraseñas filtradas está apagada.

## 9. Decisiones

| # | Tema | Estado |
|---|---|---|
| D1 | Tasa de interés | **Por confirmar.** Mientras tanto, el fiado va sin interés (C1 de la spec). La spec permitía 10 % mensual, unos 214 % efectivo anual, muy por encima de la tasa de usura. Si después se quiere cobrar interés, confirmar la tasa vigente con un contador. |
| D2 | NIT y tipo de persona | **Decidido:** no todo NIT es jurídica. Clientes y proveedores: C7 de la spec. Negocio emisor y documento fiscal: `fase-4-facturacion.md`. |
| D3 | Voseo | **Decidido:** se deja en toda la app, incluidos los mensajes de WhatsApp. |
| D4 | Repositorio | **Decidido:** un solo repositorio. |
| D5 | Proveedor de facturación electrónica | **Por decidir.** Recomendación: MATIAS API, Casa de Software (ver sección 12). Bloquea la salida para quien esté obligado a facturar. |
| D6 | Dónde se publica la app | **Decidido:** en el VPS del usuario (8 GB de RAM, 4 vCPU). Falta comprar el dominio y confirmar el sistema operativo. La configuración está en `despliegue/`, sin build completo probado. Supabase sigue siendo la base de datos. |

## 10. Ruta de salida para licorerías y tiendas

**Objetivo:** vender en mostrador con lector, cobro mixto, caja, inventario, compras y fiado, con datos reales y publicado en internet.

**Queda fuera de la salida:** F3.4, F3.6, F3.7, F3.8, las fases 5 y 6, y de la fase 7 lo que no sea panel y reportes.

| # | Paso | Quién |
|---|---|---|
| 1 | Terminar F3.1, F3.2, F3.3 y F3.5 con las correcciones, y escribir `REPORTE-FASE-3.md` | Antigravity |
| 2 | Corregir C1 (importación CSV): una licorería carga su catálogo así | Antigravity |
| 3 | Revisar la entrega y aprobarla | Claude |
| 4 | Aplicar las 7 migraciones a Supabase, con permiso del usuario | Claude |
| 5 | Elegir proveedor DIAN (D5) y confirmar con un contador qué clientes piloto están obligados a facturar y cómo tratar los impuestos de los licores | Usuario |
| 6 | Fase 4 con el proveedor real | Claude escribe la spec; Antigravity construye |
| 7 | F7.1 a F7.3: panel y reportes con datos reales. Falta revisar qué dependencias tienen de las fases pospuestas | Claude y Antigravity |
| 8 | Publicar backend y frontend en el VPS (D6) | Claude y usuario |
| 9 | Probar el flujo completo en un negocio real: lector, impresora, caja y fiado | Usuario y Claude |

Los pasos 5 y 8 no dependen de los anteriores y pueden empezar ya. C2 a C5 se corrigen cuando no retrasen estos pasos.

## 11. Lo que no se revisó

- Las pantallas en el navegador. Se leyó el código, no se probó la interfaz.
- Las fases 0 y 1 a fondo, y 2A y 2B regla por regla.
- B1 (gastar el cupo en otra sucursal) no se pudo probar porque F3.2 no existe. Se deduce de A2, que sí se reprodujo.
- Los demás puntos de las secciones 5 a 7 salen de leer el código; solo A0 a A4 y B11 se ejecutaron.

## 12. Proveedores de facturación electrónica

Revisado el 2026-11-04 en las páginas de cada proveedor. Hay que confirmar precios antes de contratar.

**Supuesto del cálculo:** 10 tiendas con 60 ventas al día, y un documento por venta (21.600 al año por tienda; 216.000 en total).

| Proveedor | Modalidad | Plan para el supuesto | Costo por tienda al mes |
|---|---|---|---|
| MATIAS API, Casa de Software | Software propio | Bolsa de 300.000 ($3.900.000 al año) + 10 certificados de $104.000 | **$41.167** |
| Facturalatam, nube | Software propio | Distribuidor ($349.900 al mes + IVA) | $41.638 + certificado (precio normal no publicado) |
| Facturalatam, licencia en el VPS | Software propio | USD 200 una vez | Solo certificado, pero el mantenimiento es propio |
| Plemsi | Proveedor autorizado | 24.000 documentos por NIT ($1.242.000 al año) | $103.500; bolsas multiempresa sin precio publicado |
| Bilidox | Proveedor autorizado | 30.000 documentos ($3.750.000 al año) | $312.500; sin plan multiempresa publicado |
| Factus | No lo confirma su página | Sin precio publicado | Cotizar |

**Recomendación:** MATIAS API, Casa de Software. Bolsa compartida entre clientes, sandbox gratis, y POS electrónico incluido. Antes de contratar, pedir precio de bolsa a Plemsi, porque es proveedor autorizado y puede exigir menos trámite por cliente.

**Condiciones de MATIAS:** mínimo 3 clientes terceros habilitados en el año para mantener la tarifa; cada cliente se habilita como software propio; precios vigentes hasta el 2026-12-31; el bono de un certificado gratis aplica desde 10.000 documentos.

**Pendiente de confirmar con el proveedor y un contador:**
- cuánto trámite exige habilitar cada cliente como software propio;
- qué clientes piloto caben en el artículo 8 de la Resolución 165 de 2023 (no obligados, como personas naturales que cumplen las condiciones de no responsables de IVA). Ahí el costo de facturación es 0, pero si optan por facturar quedan obligados;
- la regla de 5 UVT de F4.3: la DIAN dice que el POS electrónico se expide sin importar el valor y que hay que identificar al comprador. Leer el artículo 16 completo antes de construirla; la regla puede seguir siendo válida en parte.

**Opciones gratis:** el facturador de la DIAN no menciona API ni POS. Facturalatam Community no incluye POS. El ejecutable gratis de Caja General se instala en el VPS y lista POS, pero no aclara si la versión gratis lo incluye; falta probarlo.

**Ruta sugerida:** salir primero con negocios no obligados, sin proveedor, y sumar MATIAS para los obligados.

**Efecto en la spec de la fase 4:** `fiscal_provider` solo admite `MOCK`. Falta agregar el proveedor elegido.

**Base de datos:** Supabase Free pausa el proyecto tras una semana sin actividad y no incluye backups. Para producción se necesita Pro, desde USD 25 al mes, compartido entre todos los clientes.


### 12.1 Actualización del 2026-11-10 (manda sobre la tabla de arriba)

Revisado hoy en la página de cada proveedor. Los precios son los publicados, en COP; hay que confirmarlos por escrito.

**Supuestos:**
- **Piloto:** 3 tiendas con 40 ventas al día, unos 43.000 documentos al año.
- **Crecimiento:** 10 tiendas con 60 ventas al día, 216.000 documentos al año.

| Proveedor | Modalidad | POS electrónico | Costo en el piloto | Costo con 10 tiendas | Notas |
|---|---|---|---|---|---|
| **MATIAS API, Casa de Software** | Software propio, con varias empresas en una bolsa | Incluido (confirmado en su página) | Bolsa de 50.000 por $850.000 al año + 3 certificados de $104.000 = **$32.300 por tienda al mes** | Bolsa de 300.000 por $3.900.000 + 10 certificados = **$41.200 por tienda al mes** | Sandbox gratis que no gasta documentos. MATIAS corre el set de pruebas de la DIAN. Exige 3 clientes terceros habilitados en el año. La propuesta vale 30 días. Con una bolsa de 10.000 o más, el primer certificado es gratis. |
| **Facturalatam, nube** | Software propio, marca blanca | Incluido en Distribuidor; en Revendedor la página no lo dice | Revendedor: $99.900 + IVA al mes, hasta 10 empresas y 5.000 documentos al mes = **$39.600 por tienda + certificado** | Distribuidor: $349.900 + IVA, hasta 50 empresas y 30.000 documentos al mes = **$41.600 por tienda + certificado** | Pago mensual, sin contrato anual. 15 días de prueba. El certificado cuesta $59.000 en promoción hasta el 30 de noviembre; el precio normal no está publicado. |
| Facturalatam Enterprise, en el VPS | Software propio, instalado en el servidor propio | Incluido | USD 200 una sola vez + certificados | Igual | El servidor, las actualizaciones DIAN después del primer año y el soporte quedan a nuestro cargo. |
| Caja General API | Software propio, instalado en el servidor propio | Lista el documento equivalente POS; no aclara si viene en la versión gratis | Gratis, o $400.000 una vez por el código fuente | Igual | Hay que probarlo. Todo el mantenimiento es propio. |
| Plemsi | Proveedor tecnológico autorizado | Incluido | 1.200 documentos al mes por $72.000 = **$72.000 por tienda** (un paquete por NIT) | 24.000 al año por NIT, $1.242.000 = **$103.500 por tienda** | Para varias empresas tiene "bolsas de documentos", sin precio publicado: cotizar. |
| Bilidox | Proveedor tecnológico autorizado, ISO 27001 | Incluido | Planes chicos (15, 50 y 100 documentos); los grandes, por cotización | Cotizar | Sin plan publicado para varias empresas. |
| Factus, Dataico, The Factory HKA | Proveedores autorizados | — | Sin precio publicado | — | Cotizar solo si fallan las de arriba. |

**Recomendación:**
1. **Principal: MATIAS API, Casa de Software.** Es la más barata por documento, la bolsa se comparte entre todos los clientes, el POS está incluido, el sandbox es gratis y automatiza el set de pruebas.
2. **Para el piloto, sin compromiso anual: Facturalatam Revendedor.** Se paga mes a mes y tiene 15 días de prueba. Sirve si al principio no hay 3 clientes seguros para cumplir la condición de MATIAS.
3. **Cotizar la bolsa de Plemsi**, porque es proveedor autorizado. Con un proveedor autorizado, el cliente no se habilita como software propio, y eso puede significar menos trámite para cada tienda.

**Qué implica "software propio":** cada tienda se registra ante la DIAN como facturadora con software propio y necesita su propio certificado digital. Nexa (o el proveedor) le hace el set de pruebas. Hay que confirmar con un contador quién responde ante la DIAN si una transmisión falla.

**Efecto en el diseño:** no cambia nada. La venta guarda la fila en `fiscal_documents` (el outbox) y un worker la transmite fuera de la transacción. El adaptador del proveedor se escribe en la fase 4, cuando el usuario elija (D5).

**Antes de elegir:** pedirle a MATIAS y a Facturalatam el sandbox y la propuesta por escrito, pedirle a Plemsi el precio de la bolsa multiempresa, y confirmar con un contador qué clientes piloto están obligados a facturar.


**Opciones gratis (revisado el 2026-11-10):**
- **Facturador gratuito de la DIAN:** no sirve para el POS. Hace factura electrónica de venta, pero no el documento equivalente POS. Así lo dicen Actualícese y Alegra; no hay un comunicado oficial de 2026 que lo contradiga.
- **Caja General, ejecutable gratis:** se instala en el VPS, no tiene tope de documentos y lista el "Documento Equivalente POS" como soportado. Falta confirmar que la versión gratis lo incluya; si no, el código cuesta $400.000 una sola vez. Es la única opción con $0 por documento. Hay que probarlo en el sandbox de la DIAN.
- **Facturalatam Community, gratis:** no incluye el POS. La versión Enterprise sí, por USD 200 una sola vez.
- **Conexión directa a la DIAN con desarrollo propio:** $0 por documento, pero hay que construir el XML UBL 2.1, la firma XAdES, el CUDE y el set de pruebas. Son semanas de trabajo, y cada cambio de la DIAN queda a nuestro cargo. No se recomienda para salir rápido.
- **Costo que no se evita:** el certificado digital de cada tienda. Cuesta $104.000 al año con MATIAS y $59.000 con la promoción de Facturalatam, que vence el 30 de noviembre.
- **Tiendas no obligadas** (artículo 8 de la Resolución 165 de 2023): no pagan nada de facturación. Si deciden facturar, quedan obligadas.


## Revisión de M10A y M11A (2026-11-10)

**Aprobados.** Frontend `2aff069` (M10A) y `bdc69d0` (M11A). El backend sigue en `a24f401` y su copia de trabajo está limpia.

- M10A: el efecto quedó exactamente como pedía la spec, y `ONBOARDING_INVALID` usa `friendlyError`.
- M11A: se corrigieron los 5 puntos. "Cambiar rol" usa `availableRoles`. La grilla de roles usa `ASSIGNABLE_ROLES`. El mensaje copiado trae el nombre. La sucursal se muestra con `branch_name ?? "Todas las sucursales"`. `branch_ids` ya no está en el tipo ni en los mocks.
- Verificación de Claude: `tsc` y `lint` sin errores, y los dos `grep` vacíos.
- Detalle menor, que no bloquea: `mockInviteUser` pone `branch_name: "Sede Centro"` para cualquier sucursal. Solo afecta el modo demo.

Siguiente: M12.

## Revisión de M12 a M16 (2026-11-10)

**Aprobados, con una corrección pendiente: M16A.**
- Backend: `bea1f8c`, `0d66e98`, `df33de3`, `2bbf7a2`, `7f80936` y `12ac0f8`.
- Frontend: `8465a4a`, `3493011`, `8506cc4` y `a8e8797`.

**Verificación de Claude:**
- `test.sh` con fmt, clippy y test: 60 pruebas pasan, en `12ac0f8`.
- `tsc` y `lint` sin errores.
- `next build` **falla**.

**Lo que se revisó en el código:**
- M12: el orden de las validaciones de `PUT /v1/business` es el de la spec, y el 403 aparece solo cuando un no dueño cambia los campos restringidos. `POST /v1/branches` exige `DUENO` y usa `set_scope(..., None)`. Las dos políticas RLS de `tenants` quedaron bien.
- M13: la tabla de `route-permissions.ts` es idéntica a la spec. El menú se filtra solo con `canOpen` y "Cerrar sesión" llama a `signOut()`. La devolución usa primero el turno de quien la pide.
- M14 y M15: `v_sales` no tiene `delivery_fee`. La mora se calcula con la hora local, sin `current_date`. Los métodos de pago suman `payments.amount`, que es lo aplicado a la orden y no lo que entregó el cliente.
- M16: los 9 reportes. El CSV se descarga con `fetch` y el token.

**Hallazgos:**
1. **`next build` falla** en `/clientes/[id]` y `/inventario/producto/[id]`, con `CLIENT_HOOK_DYNAMIC`, porque `usePathname()` queda fuera de `<Suspense>` con `cacheComponents`. Viene de antes de la fase S, pero bloquea el despliegue. Claude probó la corrección: con `<Suspense>` en los dos layouts, el build pasa. Va en **M16A**.
2. **La salida del reporte de M16 no era la del commit.** `7f80936` registraba dos veces las 9 rutas de reportes, y Axum no arranca así. Antigravity lo corrigió en `12ac0f8`, que el reporte no menciona. M17 ahora exige correr la verificación después del último cambio.
3. Menor: M16 agregó al menú el ítem "Reportes de ventas" (`/ventas`), aparte de "Ventas y órdenes". El dueño ve 10 ítems, y la spec del menú (S4.4) decía 9. Se acepta por ahora; el diseño nuevo de Stitch los junta en "Ventas", con pestañas.
4. Menor: el mock de reportes usa "Licorería La Reserva". Solo afecta el modo demo.
5. Quedan dos cambios sin commit, que son solo saltos de línea en `route-guard.tsx` y `route-permissions.ts`. M16A los descarta.

**Pendiente de Claude:**
- Aplicar `20261109000100_salida.sql` a Supabase, con permiso del usuario.
- Probar el menú y el guardia de rutas con usuarios reales de cada rol. El modo demo no lo permite.

Siguiente: M16A y, después, M17.

## Revisión de M16A y M17: cierre de la fase S (2026-11-11)

**Fase S aprobada en código.** Frontend `c3d924a` (M16A). El backend no cambió desde `12ac0f8`.

**Verificación de Claude:**
- `tsc` y `lint` sin errores.
- `npx next build` termina bien: `/clientes/[id]` y `/inventario/producto/[id]` salen como "◐ Partial Prerender".
- Backend: 60 pruebas pasan, en la corrida de la revisión anterior, sobre el mismo commit.
- Las dos copias de trabajo están limpias.

**M17:**
- El reporte está completo: commits, migración entera, la tabla [P] de M10 a M16 sin "FALTA", las pruebas viejas ajustadas con su motivo y ninguna dependencia nueva.
- Se escribió antes que M16A, así que todavía nombra el build como pendiente y no lista `c3d924a`.
- Antigravity **no hizo los 8 puntos de la lista de verificación en el navegador**: Chrome DevTools MCP no conectó. Quedan para una prueba manual.
- Nota del modo demo: los datos del negocio y de las sucursales viven solo en memoria, así que se pierden con F5. No afecta el modo real.

**Antes de la salida:**
1. Prueba manual de los 8 puntos de M17 en el modo demo.
2. Aplicar `20261109000100_salida.sql` a Supabase. La aplica Claude, con permiso del usuario.
3. Probar con un usuario real de cada rol: Dueño, Administrador de caja, Supervisor, Cajero y Bodega. Hay que revisar el menú (S4.4), el guardia de rutas (S4.3) y la devolución aprobada a distancia.
4. Escribir el módulo de despliegue: VPS, dominio, variables de entorno y respaldos.

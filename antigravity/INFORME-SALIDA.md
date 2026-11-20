# Informe de salida de Nexa POS: qué falta y cómo se construye

**Fecha:** 2026-11-18. **Escrito por:** Claude. **Para:** el usuario y Antigravity.

Este informe reemplaza la ruta de salida de `REVISION-CLAUDE.md` sección 10 en lo que falta. Lo que ya está hecho no se vuelve a tocar.

---

## 1. Dónde estamos

La fase S cerró. El módulo para licorerías y tiendas está completo en código: venta en mostrador, caja y arqueo, inventario, compras, fiado y cartera, alta de negocio, invitaciones, permisos por rol, panel y reportes.

| Repositorio | Rama | Commit | Subido |
|---|---|---|---|
| `backend` | `feat/fase-s-salida` | `12ac0f8` | Sí |
| `frontend` | `feat/fase-s-salida` → `frontend/feat/fase-s-salida` | `c3d924a` | Sí, el 2026-11-18 |

**Verificado por Claude:** `fmt`, `clippy` y 60 pruebas del backend pasan; `tsc`, `lint` y `npx next build` del frontend pasan.

**Pospuesto por decisión del usuario:** facturación electrónica DIAN (fase 4), F3.4, F3.6, F3.7, F3.8, fases 5 y 6. La salida va solo con negocios no obligados a facturar electrónicamente.

---

## 2. Prueba en el navegador: resultado

Claude corrió el modo demo en el puerto 3200, con las variables de Supabase vacías para no tocar la base de producción. El lanzador quedó en `frontend/scripts/dev-demo.mjs`.

| # | Punto de M17 | Resultado |
|---|---|---|
| 1 | `/login` → "Crear cuenta" → `/registro` → `/onboarding` | **Falla en demo.** `/login` en modo demo solo muestra "Entrar al demo": no hay enlace a `/registro`. Desde `/registro` el flujo completo sí funciona. |
| 2 | `/onboarding` muestra "Tu negocio está listo" y sus 4 enlaces abren | Pasa. Los 4 responden 200. |
| 3 | `/panel` con KPI, gráfica por hora, top 10, métodos de pago con "Crédito", stock bajo y cartera vencida | Pasa. |
| 4 | `/ventas` y `/ventas/productos` cargan y "Exportar CSV" abre bien en Excel | Pasa. El CSV sale con BOM UTF-8, separador `;` y coma decimal. |
| 5 | Invitar usuario con rol "Cajero"; el selector no ofrece Mesero, Cocina ni Repartidor | Pasa en el formulario de invitación. **Pero** el filtro "Todos los roles" de la lista sí ofrece Mesero, Cocina y Repartidor, y la tabla muestra usuarios con esos roles. |
| 6 | Cambiar el nombre del negocio y que siga ahí al recargar; "Nueva sucursal" agrega al selector | **Falla en demo.** El nombre se pierde al recargar (ya documentado: el demo guarda en memoria). "Nueva sucursal" dice "Sucursal creada" pero no aparece en el selector. |
| 7 | El menú no muestra "Restaurante" ni "Rutas y mayorista"; "Cerrar sesión" cierra la sesión | Pasa. Cerrar sesión lleva a `/login`. **Nota:** en demo volver a `/panel` entra de nuevo, porque el demo no exige login; en modo real el guardia de rutas sí aplica. |
| 8 | La barra del POS no tiene el botón del candado | Pasa. |

### Defectos encontrados

| Código | Defecto | Dónde | Gravedad |
|---|---|---|---|
| **D1** | `/login` en modo demo no ofrece "Crear cuenta" | `frontend/components/auth/login-screen.tsx:135-150` | Baja. Solo demo. |
| **D2** | "Nueva sucursal" no aparece en el selector | `frontend/lib/api/index.ts:1409-1416`: en mocks, `session.branches()` devuelve una sola sucursal fija y no lee `mockBranchesList`, que es donde `mockCreateBranch` la agrega. El backend sí la crea bien (`src/tenancy.rs:380`, con pruebas) y la membresía del dueño es de todo el negocio, así que en modo real aparece. | Media. Solo demo, pero hace dudar de una función que sí sirve. |
| **D3** | El filtro de roles y los datos de prueba incluyen Mesero, Cocina y Repartidor, que son de las fases 5 y 6, pospuestas | `frontend/components/configuracion/users-screen.tsx:323` y `lib/api/mock-platform.ts` | Media. Muestra funciones que no existen; el LEEME prohíbe eso. |
| **D4** | El panel muestra la tarjeta "Propinas" y la sección "Accesos rápidos" | `frontend/components/panel/panel-screen.tsx:372` y `:573`. El campo `tips` sí está en la spec de M15; "Accesos rápidos" no está en M15 y el prompt de diseño lo prohíbe. | Baja a media. Decisión del usuario: ver §6, pregunta A. |
| **D5** | Los datos del demo son de un supermercado y un asadero de Medellín ("Supermercado El Vecino", "Asadero El Vecino") | `lib/api/mock-platform.ts:20-40` | Baja. Pero al mostrar el demo a una licorería de Riohacha, se ve ajeno. |
| **D6** | El menú muestra "Mapa de pantallas" | `components/nexa/sidebar.tsx:75` lo oculta con `NODE_ENV !== "production"`, así que en el VPS no se verá. | Ninguna. Solo se anota para que no se "arregle". |

Ninguno bloquea la salida. D2 y D3 sí conviene corregirlos antes de mostrar el demo a un cliente.

---

## 3. Lo que falta para salir

| # | Trabajo | Módulo | Quién construye | Quién verifica |
|---|---|---|---|---|
| 1 | Corregir D1 a D5 y preparar datos de demo de licorería | **M18** | Antigravity | Claude |
| 2 | Aplicar las migraciones a Supabase de producción | — | **Claude**, con permiso del usuario | Claude |
| 3 | Crear un usuario por rol y probar menú, guardia de rutas y devolución aprobada a distancia | **M20** | Antigravity escribe el guion y las pruebas; Claude ejecuta contra la base real | Claude |
| 4 | Despliegue en el VPS: dominio, variables, respaldos, migraciones y guía | **M19** | Antigravity | Claude y el usuario |
| 5 | Probar en un negocio real: lector, impresora, caja y fiado | — | Usuario | Usuario y Claude |

**Orden:** M18 y M19 son independientes y pueden ir al mismo tiempo. El paso 2 lo hago yo en cuanto me des permiso y no depende de los módulos. M20 va después del paso 2, porque necesita la base migrada.

---

## 4. Cómo se construye: 3.8 coordina, los subagentes trabajan en paralelo

Antigravity tiene subagentes. Hasta ahora se usó un solo hilo lineal, módulo por módulo. De aquí en adelante cada módulo se reparte.

**El rol del modelo 3.8 es coordinar, unir y probar. No programa las tareas él mismo.** Concretamente:

1. **Reparte.** Lee el módulo completo, abre un subagente por tarea y le pasa solo lo que esa tarea necesita.
2. **Mantiene los carriles separados.** Dos subagentes nunca editan el mismo archivo. Cada tarea del módulo dice qué archivos le pertenecen.
3. **No espera en fila.** Todas las tareas del mismo bloque arrancan a la vez. Solo espera cuando una tarea declara una dependencia.
4. **Une.** Recibe el trabajo, resuelve los choques, deja un commit por tarea con el mensaje que el módulo indica.
5. **Prueba por encima de los subagentes.** Cada subagente corre sus propias pruebas. Además, 3.8 corre la verificación completa del módulo **después de unir todo**, que es cuando aparecen los errores que ningún subagente ve solo (rutas duplicadas, tipos que no cuadran entre archivos, el build de producción).
6. **Escribe el reporte y se detiene.** Nunca aplica migraciones ni despliega.

### Plantilla del prompt (así se escriben todos los módulos de aquí en adelante)

```text
Vas a hacer el módulo MXX de Nexa POS. Vos coordinás; el trabajo lo hacen subagentes.

1. Leé completos antigravity/00-LEEME.md y antigravity/modulos/MXX-....md.
2. Abrí un subagente por cada tarea del bloque 1 y lanzalos TODOS AL MISMO TIEMPO.
   No esperés a que uno termine para lanzar el siguiente.
   A cada subagente dale: la tarea tal como está escrita, la lista de archivos que
   le pertenecen y la regla 0 del LEEME ("lo que no está escrito está prohibido").
   Ningún subagente toca un archivo que no esté en su lista. Si necesita uno ajeno,
   se detiene y te lo reporta; vos decidís.
3. Cuando el bloque 1 termine, uní el trabajo, resolvé los choques y commiteá cada
   tarea por separado con el mensaje que indica el módulo.
4. Recién entonces lanzá el bloque 2 igual que el 1, si el módulo tiene uno.
5. Corré vos mismo la verificación completa del módulo, después del último cambio.
   Esto es tu trabajo, no el de los subagentes: ellos ya probaron su parte, vos
   probás que todo junto funciona. Si algo falla, mandá la corrección al subagente
   dueño de ese archivo.
6. Escribí tu sección del reporte: qué hizo cada subagente, qué pruebas corrió cada
   uno, la salida de TU verificación final y las preguntas abiertas con archivo y línea.
7. Detenete. Alto obligatorio: esperá la revisión de Claude.

No apliques migraciones, no corras SQL remoto, no despliegues y no toques .env.
```

### Lo que yo hago

No programo en estos módulos. Mi trabajo es:

- **Antes:** escribir cada módulo con sus tareas, sus archivos, sus pruebas y sus textos exactos, para que ningún subagente tenga que decidir.
- **Después:** correr `fmt`, `clippy`, las pruebas del backend contra una base real, `tsc`, `lint` y `next build`; revisar el diff tarea por tarea; probar la interfaz en el navegador; y aprobar o devolver con correcciones numeradas.
- **Lo que solo hago yo:** aplicar las migraciones a Supabase, probar con usuarios reales de cada rol, y revisar que el VPS quede bien.

---

## 5. Los módulos

Están escritos en archivos aparte, cada uno con su prompt listo para copiar:

| Módulo | Qué hace | Tareas en paralelo |
|---|---|---|
| [M18](modulos/M18-correcciones-demo.md) | Corrige D1 a D5 y cambia los datos del demo a una licorería de Riohacha | 4 |
| [M19](modulos/M19-despliegue.md) | Despliegue en el VPS: respaldos, verificación de salud, guía y migraciones | 4 |
| [M20](modulos/M20-prueba-por-rol.md) | Guion y pruebas automáticas de los 5 roles contra la base real | 3 |

---

## 6. Lo que necesito de vos

**Permiso para:**
1. Aplicar las migraciones a Supabase de producción. Son 16 archivos; en la base hay 8. Antes hago un respaldo.

**Decisiones:**
- **A. El panel.** ¿Saco la tarjeta "Propinas" y la sección "Accesos rápidos"? Las propinas no se usan en una licorería y "Accesos rápidos" no está en la especificación. Mi recomendación: saco las dos, y las propinas vuelven si abrís un restaurante.
- **B. El dominio.** Sin dominio no hay HTTPS y no se puede publicar. Decime cuál vas a usar y lo apuntamos al VPS.
- **C. El archivo nuevo `backend/docs/notas_proyecto/05_Registro_Datos_de_Prueba.md`**, del 17 de noviembre, con 4 usuarios de prueba. ¿Lo dejaste vos o Antigravity? M20 lo usa como base; si no es tuyo, confirmame que puedo reescribirlo.

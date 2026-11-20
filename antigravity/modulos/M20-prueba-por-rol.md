# M20 Prueba por rol contra la base real

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorios | `backend` (pruebas) y `frontend` (nada de código: solo el guion) |
| Rama | `feat/fase-s-salida` |
| Commits | Uno por tarea: `test(salida): M20.N <resumen>` |
| Migración | Ninguna |
| Depende de | Que Claude ya haya aplicado `20261109000100_salida.sql` a Supabase |
| Reporte | Sección "M20" al final de `antigravity/REPORTE-FASE-S.md` |

**Qué queda sin probar hoy:** el menú por rol (S4.4), el guardia de rutas (S4.3) y la devolución aprobada a distancia (S4.2) no se pueden ver en modo demo, porque el demo entra siempre como dueño. Este módulo escribe las pruebas automáticas y el guion manual. **Antigravity no ejecuta nada contra Supabase: eso lo hace Claude.**

## Prompt para Antigravity

```text
Vas a hacer el módulo M20 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

1. Leé completos antigravity/00-LEEME.md, antigravity/INFORME-SALIDA.md (§4),
   antigravity/modulos/M20-prueba-por-rol.md, antigravity/flujos-por-rol.md (§4.2)
   y antigravity/modulos/M13-S4-permisos-navegacion.md.
2. Abrí 3 subagentes (M20.1, M20.2 y M20.3) y lanzalos TODOS AL MISMO TIEMPO, con su
   tarea, su lista de archivos y la regla 0 del LEEME.
3. Cuando terminen, uní el trabajo y hacé un commit por tarea.
4. Corré VOS la verificación final: las pruebas del backend con base local (test.sh),
   tsc, lint y next build.
5. Agregá la sección "M20" al final de antigravity/REPORTE-FASE-S.md.
6. Subí: git push origin <la rama actual de nexa/>.
7. Detenete. Escribime "M20 listo para revisión".

PROHIBIDO: conectarte a Supabase, correr SQL remoto, crear usuarios reales, leer
.env o .env.local, y escribir cualquier correo, contraseña o clave real en un
archivo o en el reporte. Las contraseñas de prueba las pone Claude al ejecutar.
```

---

## M20.1 Pruebas del menú y del guardia de rutas por rol

**Archivos que le pertenecen:** `frontend/lib/stores/session.ts` solo para leer, y un archivo de pruebas nuevo. Si el proyecto no tiene corredor de pruebas en el frontend, **no instales ninguno**: en lugar de pruebas, escribí `frontend/design/pruebas-por-rol.md` con una tabla de lo esperado, y decilo así en el reporte.

**Commit:** `test(salida): M20.1 menú y rutas por rol`

Lo esperado está en M13 (S4.4 y S4.5) y no se cambia:

| Rol | Ítems del menú | Inicio |
|---|---|---|
| `DUENO`, `ADMIN_CAJA` | Panel, POS, Inventario, Compras, Ventas y órdenes, Clientes, Cartera, Marketing, Configuración | `/panel` |
| `SUPERVISOR` | Panel, POS, Inventario, Compras, Ventas y órdenes, Clientes, Cartera | `/panel` |
| `CAJERO` | POS, Ventas y órdenes, Clientes, Cartera | `/pos` |
| `BODEGA` | Inventario, Compras | `/inventario` |
| `MESERO`, `COCINA`, `REPARTIDOR` | Ninguno; se ve "Tu rol todavía no tiene pantallas" | — |

Las pruebas deben cubrir, para cada uno de los 7 roles: los ítems visibles (exactamente esa lista, ni uno más), el resultado de `homeFor(role)`, y que una ruta prohibida para ese rol no se renderice. Las pruebas son puras: no abren un navegador ni llaman a la API.

**Prueba del subagente:** sus propias pruebas pasan, y `tsc --noEmit` y `lint` siguen limpios.

## M20.2 Prueba de la devolución aprobada por otro usuario

**Archivos que le pertenecen:** `backend/src/db_tests.rs` solamente.

**Commit:** `test(salida): M20.2 devolución aprobada a distancia`

Agregá una prueba nueva, `s4_refund_remote_approval`, siguiendo el estilo de `s4_permissions` (alrededor de la línea 12782): sembrá un negocio, una sucursal, un dueño con membresía de todo el negocio y un cajero con membresía de la sucursal.

Pasos y lo que debe cumplirse:

1. El cajero abre su turno de caja y hace una venta en efectivo.
2. Con `pos.refund_requires_approval = true`, el cajero pide la devolución: queda `PENDIENTE`.
3. El dueño la aprueba **sin tener un turno de caja abierto**, que es el caso de aprobar desde el celular.
4. **La regla que se prueba [P]:** el efectivo sale del turno del **cajero que pidió** la devolución, no del de quien aprueba. Comprobá el saldo del turno del cajero antes y después, y que la aprobación no haya creado ningún movimiento de caja a nombre del dueño.
5. La aprobación queda en la auditoría con el id del dueño como quien aprueba.

Si al escribirla descubrís que el código **no** se comporta así, **no lo arregles**: la prueba queda marcada con `#[ignore]` y un comentario que explique qué devuelve hoy, y lo anotás como pregunta abierta con archivo y línea. Ese era el defecto S4.2 y hay que confirmar si quedó corregido.

**Prueba del subagente:** `./scripts/test.sh "cargo test s4_refund_remote_approval"` pasa (o queda ignorada con su motivo). Las otras pruebas siguen pasando.

## M20.3 Guion manual de los 5 roles

**Archivos que le pertenecen:** `antigravity/guion-prueba-por-rol.md` (nuevo) y `backend/docs/notas_proyecto/05_Registro_Datos_de_Prueba.md`.

**Commit:** `test(salida): M20.3 guion de prueba por rol`

El archivo `05_Registro_Datos_de_Prueba.md` ya existe en el disco, sin commitear, con 4 usuarios (`dueno@`, `cajero@`, `caja@` y `bodega@nexa.app`) marcados "Pendiente en DB". Completalo sin borrar lo que hay: agregá el rol `SUPERVISOR` que falta, y una columna que diga qué tabla hay que limpiar para borrar cada dato.

`guion-prueba-por-rol.md` es la lista que Claude va a ejecutar a mano contra la base real. Escribila como una tabla de pasos, donde cada paso dice: **quién** (rol), **qué hace** (con los textos exactos de los botones), **qué debe pasar** y **qué NO debe pasar**. Tiene que cubrir:

1. **Dueño:** entra, ve el menú completo, crea el negocio, invita a los otros 4, carga 5 productos, da cupo a un cliente.
2. **Administrador de caja:** entra con la invitación, abre caja, ve el panel. No ve "Configuración" si el rol no lo permite: comprobalo contra la tabla de M13.
3. **Cajero:** inicia en `/pos`, vende en efectivo, vende a crédito, pide una devolución. **No** llega a `/panel` ni a `/configuracion` escribiendo la URL a mano: debe quedar fuera.
4. **Supervisor:** aprueba la devolución del cajero desde otra sesión, y autoriza una venta que se pasa del cupo con un motivo.
5. **Bodega:** inicia en `/inventario`, recibe una compra, hace un ajuste. **No** llega al POS ni a Ventas.
6. **Cierre:** el cajero cierra su turno y el arqueo cuadra, contando la devolución aprobada por el supervisor.

Al final, una sección "Cómo limpiar" que diga en qué orden se borran los datos de prueba, respetando las claves foráneas.

**Prueba del subagente:** cada paso del guion nombra una pantalla que existe y un texto de botón que está en el código. Verificá cada texto con `grep` sobre `frontend/components` y anotá cuál verificaste; si un texto no existe, corregí el guion, no el código.

---

## Verificación del coordinador

Corré **vos**, después del último cambio:

```sh
cd backend && ./scripts/test.sh "cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../frontend && npx tsc --noEmit && npm run lint && npx next build
```

Todo tiene que pasar. `test.sh` levanta una base local en Docker: esa sí se puede usar. Si Docker Desktop está apagado, `test.sh` falla con un error del demonio, que **no es un resultado de pruebas**: prendelo y repetí.

Dos corridas de `test.sh` al mismo tiempo chocan por el contenedor `nexa-test-pg`. Corré una sola.

## Reporte

Sección "M20" al final de `antigravity/REPORTE-FASE-S.md`:
1. Tabla por subagente: tarea, archivos, commit, pruebas.
2. Si M20.1 terminó en pruebas o en documento, y por qué.
3. Si `s4_refund_remote_approval` pasa o quedó ignorada, con la salida de la prueba.
4. La salida completa de tu verificación final.
5. Preguntas abiertas con archivo y línea.

## Fuera de alcance

- Crear usuarios en Supabase y ejecutar el guion: lo hace Claude.
- Instalar un corredor de pruebas en el frontend.
- Arreglar cualquier defecto que la prueba descubra: eso va en un módulo nuevo que escribe Claude.

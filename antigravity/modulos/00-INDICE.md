# Módulos pendientes de Nexa POS (índice)

Cada archivo de esta carpeta es **un módulo**: una unidad de trabajo con su prompt para Antigravity y su especificación completa. Las correcciones de las revisiones de Claude ya están incorporadas en el texto de cada módulo.

## Cómo se usa

1. El usuario copia el bloque **"Prompt para Antigravity"** del módulo que sigue y lo pega en Antigravity.
2. Antigravity lee `antigravity/00-LEEME.md` y ese módulo, hace **solo** ese módulo, corre la verificación, hace el commit, anota su sección en el reporte de la fase y **se detiene**.
3. El usuario pega el prompt del módulo siguiente. No se salta el orden.
4. Al final de cada fase (M09 y M17), Claude revisa todo y aprueba o pide correcciones.

**Si un módulo y un documento de fase dicen cosas distintas, manda el módulo.** Los documentos de fase quedan como historial y referencia.

**Por qué el crédito (M03) va antes que las correcciones de clientes (M04):** la copia de trabajo de F3.2 ya trae el arreglo que hace compilar la rama (A0), mezclado en `customers.rs` y `db_tests.rs`. Así se commitea completa, sin separar partes de un mismo archivo.

## Orden y estado

| # | Módulo | Fase | Depende de | Estado |
|---|---|---|---|---|
| M01 | [Rediseño en curso: cerrarlo sin romper el modo real](M01-rediseno-en-curso.md) | 3 | — | Hecho |
| M02 | [Migración de la fase 3 corregida](M02-fase3-migracion.md) | 3 | M01 | Hecho |
| M03 | [Venta a crédito: backend (F3.2)](M03-fase3-credito-backend.md) | 3 | M02 | Hecho |
| M04 | [Clientes: correcciones de F3.1](M04-fase3-clientes.md) | 3 | M03 | Hecho |
| M05 | [Venta a crédito: POS (F3.2)](M05-fase3-credito-pos.md) | 3 | M04 | Hecho |
| M06 | [Abonos y cuotas (F3.3)](M06-fase3-abonos.md) | 3 | M05 | Hecho |
| M07 | [Cartera (F3.5)](M07-fase3-cartera.md) | 3 | M06 | Hecho |
| M08 | [Importación CSV por `apply_delta_tx` (C1 de la fase 2A)](M08-fase2-importacion-csv.md) | 3 | M07 | Hecho |
| M09 | [Cierre y reporte de la fase 3](M09-fase3-cierre.md) | 3 | M08 | Hecho. **Fase 3 aprobada por Claude el 2026-11-10** |
| M10 | [Alta de negocio: registro y onboarding (S1)](M10-S1-alta-negocio.md) | S | M09 aprobado | Hecho y aprobado (con M10A) |
| M10A | [Corrección: pantalla de éxito del onboarding](M10A-correccion-onboarding.md) | S | M10 | Hecho y aprobado (`2aff069`) |
| M11 | [Invitaciones que funcionan (S2)](M11-S2-invitaciones.md) | S | M10 | Hecho y aprobado (con M11A) |
| M11A | [Corrección: invitaciones](M11A-correccion-invitaciones.md) | S | M11 | Hecho y aprobado (`bdc69d0`) |
| M12 | [Datos del negocio y sucursales (S3)](M12-S3-negocio-sucursales.md) | S | M11 | Hecho y aprobado (`bea1f8c` y `8465a4a`) |
| M13 | [Permisos y navegación por rol (S4)](M13-S4-permisos-navegacion.md) | S | M12 | Hecho y aprobado (`0d66e98` y `3493011`) |
| M14 | [Vista de ventas y métricas (S5)](M14-S5-vista-ventas.md) | S | M13 | Hecho y aprobado (`df33de3`) |
| M15 | [Panel con datos reales (S6)](M15-S6-panel.md) | S | M14 | Hecho y aprobado (`2bbf7a2` y `8506cc4`) |
| M16 | [Reportes y CSV (S7)](M16-S7-reportes.md) | S | M15 | Hecho y aprobado (`7f80936` + `12ac0f8`, y `a8e8797`) |
| M16A | [Corrección: `next build` falla](M16A-correccion-build.md) | S | M16 | Hecho y aprobado (`c3d924a`) |
| M17 | [Cierre y reporte de la fase S](M17-faseS-cierre.md) | S | M16A | Hecho. **Fase S aprobada en código por Claude el 2026-11-11**; faltan la prueba en navegador y la migración remota |
| M18 | [Correcciones del demo y datos de licorería](M18-correcciones-demo.md) | Salida | M17 | Pendiente |
| M19 | [Despliegue en el VPS](M19-despliegue.md) | Salida | M17 | Pendiente |
| M20 | [Prueba por rol contra la base real](M20-prueba-por-rol.md) | Salida | Migración aplicada por Claude | Pendiente |

| M21 | [Roles y permisos: matriz base, cierre ciego y costos ocultos](M21-roles-permisos.md) | Roles | M17 | Pendiente |
| M22 | [Descuentos y promociones aprobados por el dueño (PIN y fecha de fin)](M22-descuentos-pin.md) | Roles | M21 aprobado | Pendiente |
| M23 | [Cuentas abiertas por mesa y por nombre, cobro compartido y cuenta no pagada](M23-cuentas-abiertas.md) | Roles | M22 aprobado | Pendiente |
| M24 | [Experiencia por negocio: tipo de negocio, interruptores, pantallas encontrables y menú del celular](M24-experiencia-por-negocio.md) | Roles | M23 aprobado | Pendiente. Sin migración |

La fuente de M21 y M22 es `nexa/docs/roles-y-permisos.md`. Su reporte va en `antigravity/REPORTE-ROLES.md`.

**M18, M19 y M20 se construyen con subagentes en paralelo**, no en un hilo lineal: el modelo 3.8 coordina, reparte las tareas, une el trabajo y corre la verificación final. Está explicado en `antigravity/INFORME-SALIDA.md` §4, con la plantilla del prompt. M18 y M19 pueden ir al mismo tiempo; M20 va después de que Claude aplique la migración.

**Pospuesta por decisión del usuario (2026-11-10):** la fase 4, facturación electrónica. La salida va sin ella, solo con negocios que no están obligados a facturar electrónicamente. El proveedor (D5) se elige cuando se retome; la comparación de precios está en la sección 12.1 de `REVISION-CLAUDE.md`. Mientras tanto, cada venta sigue dejando su fila `PENDIENTE` en `fiscal_documents` y no hay que cambiar nada. Cuando se retome, el worker solo puede transmitir los documentos creados después de activar la facturación en cada negocio. Las filas viejas no se envían, porque el POS electrónico se transmite el mismo día de la venta.

**Pospuestas, sin módulos:** F3.4, F3.6, F3.7, F3.8, las fases 5 y 6, F7.4 a F7.7 y la fase 8.

## Ramas y migraciones

| Fase | Rama histórica backend | Rama histórica frontend | Se sube como | Migración única |
|---|---|---|---|---|
| 3 | `feat/fase-3-clientes` | `feat/fase-3-clientes` | `feat/fase-3-clientes` y `frontend/feat/fase-3-clientes` | `20261106000100_fase3_clientes.sql` (existe; se corrige en M02) |
| S | `feat/fase-s-salida` | `feat/fase-s-salida` | `feat/fase-s-salida` y `frontend/feat/fase-s-salida` | `20261109000100_salida.sql` (la crea M10; M11 a M14 le agregan su bloque al final) |

## Fases terminadas

Los documentos de las fases terminadas tienen el prefijo `TERMINADA-`. No se construyen de nuevo; sirven para consultar cómo funciona lo que ya existe:
- `TERMINADA-fase-0-plataforma.md` y `TERMINADA-REPORTE-FASE-0.md`;
- `TERMINADA-fase-1-pos-caja.md` y `TERMINADA-REPORTE-FASE-1.md`;
- `TERMINADA-fase-2-inventario-compras.md`, `TERMINADA-REPORTE-FASE-2A.md` y `TERMINADA-REPORTE-FASE-2B.md`. El único defecto pendiente de la fase 2 (la importación CSV) se corrige en M08.

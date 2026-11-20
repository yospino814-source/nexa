# M17 Cierre y reporte de la fase S

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` (solo verificación) |
| Rama | `feat/fase-s-salida` |
| Commit | Ninguno de código. Solo el reporte |
| Reporte | `antigravity/REPORTE-FASE-S.md` completo |

## Prompt para Antigravity

```text
Vas a hacer el módulo M17 de Nexa POS: el cierre de la fase S. No programes nada nuevo.

1. Leé completos: antigravity/00-LEEME.md (sobre todo la sección 6) y antigravity/modulos/M17-faseS-cierre.md.
2. Corré la verificación completa de los dos repositorios.
3. Probá en modo demo la lista de verificación de este módulo.
4. Completá antigravity/REPORTE-FASE-S.md como dice "Reporte final", sin borrar las secciones M10 a M16.
5. Subí las ramas: en nexa-backend, git push origin feat/fase-s-salida; en nexa-frontend, git push origin feat/fase-s-salida:frontend/feat/fase-s-salida.
6. Detenete. Alto obligatorio: espera la revisión de Claude. Escribime "Fase S lista para revisión".
```

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql
grep -n "Action::ManageInventory" src/procurement.rs
grep -n "isAllowedForRole\|/mesas\|/rutas" ../nexa-frontend/components/nexa/sidebar.tsx
cd ../nexa-frontend && npx tsc --noEmit && npm run lint && npx next build
```

Los tres `grep` deben salir vacíos y `next build` debe terminar con la tabla de rutas (agregado por M16A).

Corré la verificación **después del último cambio** y copiá esa salida. En M16, la salida del reporte no era la del commit `7f80936`: ese commit registraba dos veces las 9 rutas de reportes, y Axum no arranca con rutas duplicadas.

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

## Reporte final

Al principio de `antigravity/REPORTE-FASE-S.md`, antes de las secciones M10 a M16, agrega:
1. Ramas y commits: `git log --oneline feat/fase-3-clientes..HEAD` en cada repositorio.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los tres `grep`.
4. La migración `20261109000100_salida.sql` completa, con lo que agrega cada bloque. Recuerda que la aplica Claude.
5. Una tabla con **cada regla [P]** de M10 a M16 y el nombre de su prueba. Las que falten, marcadas "FALTA".
6. El resultado de cada punto de la lista de verificación.
7. Las pruebas viejas que se ajustaron (por ejemplo, la matriz de roles por S4.1) y por qué.
8. Las dependencias agregadas (debería ser ninguna).
9. Las preguntas abiertas y las tareas detenidas.

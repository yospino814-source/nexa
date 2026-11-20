# M09 Cierre y reporte de la fase 3

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorios | `nexa-backend` y `nexa-frontend` (solo lectura y verificación) |
| Rama | `feat/fase-3-clientes` |
| Commit | Ninguno de código. Solo el reporte |
| Reporte | `antigravity/REPORTE-FASE-3.md` completo |

## Prompt para Antigravity

```text
Vas a hacer el módulo M09 de Nexa POS: el cierre de la fase 3. No programes nada nuevo.

1. Leé completos, en este orden: antigravity/00-LEEME.md (sobre todo la sección 6) y antigravity/modulos/M09-fase3-cierre.md.
2. Corré la verificación completa de los dos repositorios.
3. Probá en modo demo la lista de verificación de este módulo.
4. Completá antigravity/REPORTE-FASE-3.md como dice "Reporte final", sin borrar las secciones M01 a M08.
5. Subí las ramas: en nexa-backend, git push origin feat/fase-3-clientes; en nexa-frontend, git push origin feat/fase-3-clientes:frontend/feat/fase-3-clientes.
6. Detenete. Alto obligatorio: no empieces M10 hasta que Claude apruebe la fase 3. Escribime "Fase 3 lista para revisión".
```

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -rn "current_date" src/customers.rs src/sales.rs src/credit.rs src/refunds.rs src/receivables.rs
grep -rn "\.unwrap()" src/customers.rs src/sales.rs src/credit.rs src/refunds.rs src/receivables.rs src/csv_import.rs
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Lista de verificación (modo demo, puerto 3200)

1. Clientes: crear uno con NIT `900123456` → queda Jurídica con DV 8. Con un usuario sin permiso, el cupo está deshabilitado.
2. POS: vender a crédito a "Carlos Pérez" y ver "Crédito" en la tirilla.
3. POS: vender a "Luz Marina Gómez" por encima del cupo → "Autorizar igual" → pasa.
4. Ficha del cliente → Cartera → "Abonar" en efectivo → sale la tirilla "ABONO" con el saldo nuevo.
5. `/cartera`: las 4 tarjetas, las 6 barras de antigüedad, la tabla de deudores y el top 10.
6. "Cobrar por WhatsApp" abre `wa.me` con el texto exacto.
7. "Estado de cuenta" → Imprimir en tirilla y en carta.
8. `/ordenes` en modo demo muestra el selector de vistas del rediseño (M01).

## Reporte final

Al principio de `antigravity/REPORTE-FASE-3.md`, antes de las secciones M01 a M08, agrega:
1. Ramas y commits de cada repositorio: `git log --oneline eaa5f28..HEAD` en el backend y `git log --oneline 5687711..HEAD` en el frontend.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint` de esta verificación final.
3. La salida de los dos `grep`. Si sale alguna línea, debe estar dentro de `mod tests`; explícala.
4. **Una tabla con cada regla [P] de M03, M04, M06, M07 y M08** y el nombre de su prueba. Las que falten, marcadas "FALTA". Las reglas [P] de F3.4, F3.6, F3.7 y F3.8, marcadas "POSPUESTA".
5. El resultado de cada punto de la lista de verificación. Escribe "no lo probé" si no lo hiciste.
6. Las pruebas viejas que se ajustaron y por qué.
7. Las dependencias agregadas (debería ser ninguna).
8. Las preguntas abiertas y las tareas detenidas, con el archivo y la línea.
9. Recordatorio para Claude: la migración `20261106000100_fase3_clientes.sql` **no** está aplicada en Supabase.

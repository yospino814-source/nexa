# M16 Reportes y exportación CSV (S7)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Ninguna |
| Commit | `feat(fase-s): S7 reportes y csv` |
| Reporte | Sección "M16" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M16 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M16-S7-reportes.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M16" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M17. Escribime "M16 terminado".
```

## Especificación

Construye el **texto base** de abajo (copiado de F7.3 de `fase-7-reportes-ia.md`) con estos **cambios**. Si el texto base y un cambio no coinciden, manda el cambio.

### Cambios

1. `sales-by-method` incluye `CREDITO`, con la etiqueta "Crédito".
2. Las pantallas 02 (`/ventas`) y 03 (`/ventas/productos`) pasan a `"react"` en `lib/screens.ts`, con el permiso `reports:read` en el menú.
3. En el modo demo, cada reporte tiene su mock en `lib/api/mock-reports.ts`, con datos de una licorería colombiana en COP.
4. El CSV se descarga con un `fetch` que lleva el token, y luego con `URL.createObjectURL`. Un enlace `<a href>` directo no lleva la sesión y respondería 401.

**Pruebas [P]** (`f7_reports`): las dos de F7.3.

### Texto base

Todos con permiso `reports:read` y los parámetros `branch_id`, `from` y `to`. Con `format=csv`:
- responde `text/csv; charset=utf-8`;
- con BOM (`\u{FEFF}`) al inicio, separador `;` y decimales con coma;
- encabezados en español;
- el nombre del archivo va en `Content-Disposition` y se arma como `<reporte>_<from>_<to>.csv`.

| Endpoint | Filas | Columnas |
|---|---|---|
| `/v1/reports/sales-by-day` | Una por día con ventas | fecha, ventas, venta_bruta, devoluciones, venta_neta, ticket_promedio, margen |
| `/v1/reports/sales-by-product` | Una por producto | sku, producto, categoría, cantidad, venta_sin_impuesto, costo, margen, pct_margen |
| `/v1/reports/sales-by-category` | Una por categoría | categoría, cantidad, venta_sin_impuesto, margen |
| `/v1/reports/sales-by-cashier` | Una por cajero (el `created_by` del primer pago) | cajero, ventas, venta_bruta, ticket_promedio |
| `/v1/reports/sales-by-method` | Una por método | método, pagos, monto |
| `/v1/reports/sales-heatmap` | Día ISO 1..7 × hora 0..23 | dia, hora, ventas, venta_bruta (sin CSV) |
| `/v1/reports/inventory-valuation` | Una por producto y ubicación con `on_hand != 0` | producto, ubicación, existencias, costo_promedio, valor = `money(on_hand × avg_cost)` |
| `/v1/reports/inventory-rotation` | Una por producto (últimos 30 días hasta `to`; ignora `from`) | producto, vendidas_30d, existencias, dias_inventario = `on_hand / (vendidas_30d / 30)` (vacío si no vendió) |
| `/v1/reports/purchases-by-supplier` | Una por proveedor | proveedor, órdenes, recibido, pagado, saldo |

**Frontend:**
- Pantalla 02 `/ventas`: el reporte por día más los de cajero y método, en pestañas.
- Pantalla 03 `/ventas/productos`: por producto y por categoría.
- Pestaña "Reportes" en `/inventario`: valorización y rotación.
- Pestaña "Reportes" en `/compras`: por proveedor.
- Cada tabla tiene el botón "Exportar CSV", que descarga el mismo endpoint con `format=csv`.
- Cambia las pantallas 02 y 03 a `"react"`.

**Pruebas [P]** (`f7_reports`):
- la suma de `sales-by-product.venta_sin_impuesto` es igual a `Σ line_total` del periodo;
- el CSV empieza con el BOM y usa `;`.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M16`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

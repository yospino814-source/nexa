# M15 Panel principal con datos reales (S6)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Ninguna |
| Commit | `feat(fase-s): S6 panel con datos reales` |
| Reporte | Sección "M15" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M15 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M15-S6-panel.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M15" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M16. Escribime "M15 terminado".
```

## Especificación

Construye el **texto base** de abajo (copiado de F7.2 de `fase-7-reportes-ia.md`) con estos **cambios**. Si el texto base y un cambio no coinciden, manda el cambio.

### Cambios

1. `payment_methods` incluye `CREDITO`. En la pantalla, su etiqueta es "Crédito".
2. `overdue_receivables`: cuentas `ABIERTA` o `PARCIAL` de las sucursales del alcance, con `days_overdue > 0` según F3.2. `count` es el número de cuentas y `amount` la suma de sus saldos. "Hoy" es la fecha local de cada sucursal; no uses `current_date`.
3. `low_stock` lee `public.stock_alerts`, con `kind = 'BAJO'` y la alerta abierta, según las columnas de la fase 2A.
4. `branch_id` opcional: sin él, usa `tenancy::begin` y deja que RLS limite a las sucursales del usuario. Con él, usa `begin_in_branch`.

**Prueba [P]** (`f7_dashboard`): la de F7.2, más una venta de 50.000 pagada con 20.000 en efectivo y 30.000 a crédito. `gross_sale` la cuenta completa, y `payment_methods` muestra `EFECTIVO` 20.000 y `CREDITO` 30.000.

### Texto base

`GET /v1/reports/dashboard?branch_id=&from=&to=` (permiso `reports:read`). `branch_id` es opcional: sin él, suma todas las sucursales a las que el usuario tiene acceso.

```
{ period: {from, to}, previous: {from, to},
  kpis: { gross_sale, net_sale, sales_count, avg_ticket, gross_margin, margin_pct, refunds,
          tips, cost_missing_lines },
  kpis_previous: { …mismos campos… },
  by_hour: [{hour 0..23, gross_sale, sales_count}],
  by_day: [{date, gross_sale, sales_count}],
  top_products: [{product_id, name, qty, line_total}] (10, por line_total desc),
  payment_methods: [{method, amount, count}] (Σ payments de las órdenes de v_sales),
  low_stock: [{product_id, name, location_name, on_hand, min_qty}] (alertas BAJO abiertas, máx. 10),
  overdue_receivables: {count, amount} }
```

El periodo anterior es el de la misma duración, inmediatamente antes de `from`.

**Frontend:** el panel `/panel` deja de usar mocks en el modo real (en el modo demo los conserva).
- Tarjetas con la variación porcentual frente al periodo anterior: verde si sube y rojo si baja. En el margen, el color se invierte si el costo sube.
- Gráfica por hora y por día.
- Top 10, métodos de pago, stock bajo y cartera vencida.
- Selector de periodo: Hoy, Ayer, 7 días, Este mes, Mes anterior y Personalizado.

Para las gráficas, usa barras con `div` y CSS o SVG a mano. **No agregues librerías de gráficas.**

**Prueba [P]** (`f7_dashboard`): con datos conocidos (2 ventas, 1 devolución y 1 venta cancelada), cada KPI da el valor exacto esperado. La venta cancelada no cuenta.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M15`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

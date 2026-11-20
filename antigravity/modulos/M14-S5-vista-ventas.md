# M14 Vista de ventas y métricas (S5)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | Solo `nexa-backend` |
| Rama | `feat/fase-s-salida` |
| Migración | Agrega **al final** de `20261109000100_salida.sql` el bloque `-- S5 Vista de ventas`, con la vista `v_sales` (sin `delivery_fee`). |
| Commit | `feat(fase-s): S5 vista de ventas y metricas` |
| Reporte | Sección "M14" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M14 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M14-S5-vista-ventas.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit, con el mensaje exacto del módulo.
8. Escribí la sección "M14" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M15. Escribime "M14 terminado".
```

## Especificación

Construye el **texto base** de abajo (copiado de F7.1 de `fase-7-reportes-ia.md`) con estos **cambios**. Si el texto base y un cambio no coinciden, manda el cambio.

### Cambios

1. La vista `v_sales` **no** tiene `o.delivery_fee`. Esa columna llega en la fase 5B, que está pospuesta, y la vista fallaría. Quita esa línea y nada más.
2. La vista va en `20261109000100_salida.sql`, no en `20261111000100_fase7_reportes.sql`.
3. Las fórmulas van en `src/reports/metrics.rs`, con pruebas unitarias: `metrics_avg_ticket_zero_sales`, `metrics_margin_pct` y `metrics_net_sale`.
4. Una venta con pago `CREDITO` **cuenta** como venta en el día en que se hizo. El abono posterior no es venta.

### Texto base

```sql
create view public.v_sales with (security_invoker = true) as
select o.id, o.tenant_id, o.branch_id, o.created_at,
       (o.created_at at time zone b.timezone)::date as local_date,
       extract(hour from o.created_at at time zone b.timezone)::int as local_hour,
       extract(isodow from o.created_at at time zone b.timezone)::int as local_isodow,
       o.subtotal, o.inc_amount as tax, o.discount_total, o.tip_amount, o.delivery_fee,
       (o.subtotal + o.inc_amount) as gross_sale,
       o.refunded_amount, o.customer_id, o.sale_type_id, o.cash_session_id
  from public.orders o
  join public.branches b on b.id = o.branch_id
 where o.status_id in (public.tipo_id('ORD','COBRADA'), public.tipo_id('ORD','CERRADA'));
grant select on public.v_sales to authenticated;
```

**Definiciones únicas. Todos los reportes las usan; no hay otra fórmula en ningún lado:**

| Concepto | Fórmula |
|---|---|
| Venta bruta | `Σ gross_sale` (sin propina ni domicilio). |
| Devoluciones del periodo | `Σ refunds.total` con `status = APROBADA` y `decided_at` local dentro del periodo. |
| Venta neta | Venta bruta − devoluciones del periodo. |
| Número de ventas | `count(*)` de `v_sales`. |
| Ticket promedio | Venta bruta / número de ventas (0 si no hay ventas). |
| Costo de venta | `Σ order_items.cost_total` de las líneas no canceladas de las órdenes de `v_sales`. |
| Margen bruto | `Σ line_total` de esas líneas − costo de venta. |
| % de margen | Margen / `Σ line_total` × 100. |
| Líneas sin costo | Número de líneas con `cost_missing`. Se muestra como advertencia, nunca se esconde. |

Cada periodo se filtra con `local_date between from and to`.

Escribe estas fórmulas una sola vez, como funciones de Rust en `src/reports/metrics.rs`, con pruebas.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "delivery_fee" supabase/migrations/20261109000100_salida.sql   # vacía
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M14`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".

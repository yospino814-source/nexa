# M08 Importación CSV por `apply_delta_tx` (C1 de la fase 2A)

| Dato | Valor |
|---|---|
| Fase | 3 (corrige un defecto de la fase 2A) |
| Repositorio | Solo `nexa-backend` |
| Rama | `feat/fase-3-clientes` |
| Migración | Ninguna |
| Commit | `fix(fase2a): importacion csv por apply_delta_tx` |
| Reporte | Sección "M08" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M08 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M08-fase2-importacion-csv.md.
2. Trabajá solo en nexa-backend, en la rama feat/fase-3-clientes. Solo se tocan src/csv_import.rs y src/db_tests.rs.
3. Hacé exactamente lo que dice "Qué hacer".
4. Corré la verificación. Debe pasar entera.
5. Commit con el mensaje exacto del módulo.
6. Escribí la sección "M08" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
7. Detenete. No empieces M09. Escribime "M08 terminado".
```

## Problema

`csv_import.rs::apply_initial_stock` (línea 834) escribe el stock directo:
- un `insert into stock ... on conflict do update set on_hand = ...`;
- un `insert into inventory_movements` a mano.

Así no pasa por `inventory::apply_delta_tx`, el único escritor de stock permitido desde la fase 2. El stock importado queda sin `avg_cost`, sin capa de costo y sin alertas. Una licorería carga su catálogo con esta función, así que el costo de lo vendido y el margen saldrían mal desde el primer día.

Además, el costo se pasa como `data.costo.unwrap_or(Decimal::ZERO)`. Pasar 0 como costo real baja el costo promedio del producto. Con `None`, `apply_delta_tx` usa el costo promedio actual y marca el caso como `cost_missing`.

## Qué hacer

1. Cambia la firma a:

```rust
async fn apply_initial_stock(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cost: Option<Decimal>,
    session_id: Uuid,
) -> Result<(), ApiError>
```

2. El cuerpo queda así, en este orden:
   1. `loc_id`: la misma consulta de hoy (`select id from locations where branch_id = $1 and is_default = true`), pero con `fetch_optional`. Si no hay: 409 `IMPORT_NO_DEFAULT_LOCATION`, con el texto "La sucursal no tiene una bodega principal".
   2. Llama a:

```rust
crate::inventory::apply_delta_tx(
    tx,
    scope,
    crate::inventory::StockDelta {
        branch_id,
        location_id: loc_id,
        product_id,
        delta: qty,
        movement: "AJUSTE_ENTRADA",
        unit_cost: cost,
        lot_id: None,
        ref_type: "IMPORT",
        ref_id: session_id,
        cause: Some("Carga inicial".to_string()),
    },
)
.await?;
```

   3. Si `cost` es `Some(c)`: `update products set cost_price = coalesce(cost_price, $1) where tenant_id = $2 and id = $3`, como hoy. Si es `None`, no se ejecuta.
   4. **Borra** el `insert into stock` y el `insert into inventory_movements` manuales.
3. En los dos lugares donde se llama (líneas 695 y 799), pasa `data.costo` (sin `unwrap_or`) y `req.session_id`.
4. Si `apply_delta_tx` rechaza el producto (por ejemplo, `PRODUCT_IS_VARIANT_PARENT`), el error sube como está. No lo atrapes.

## Pruebas (en `f2_import`, al final)

- **[P]** Importar un producto con cantidad inicial 10 y costo 2.500 deja `on_hand = 10`, `avg_cost = 2500` y un movimiento `AJUSTE_ENTRADA` con `ref_type = 'IMPORT'` y `cause = 'Carga inicial'`.
- **[P]** Importar un producto **nuevo** con cantidad inicial 5 y **sin** costo deja `on_hand = 5`, `avg_cost = 0`, `products.cost_price` sin cambio (`null`) y un movimiento `AJUSTE_ENTRADA` con `unit_cost = 0`. (`apply_delta_tx` marca ese caso como `cost_missing` en su respuesta `Applied`; la tabla de movimientos no tiene esa columna.)

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "insert into stock\|on_hand = stock.on_hand\|unwrap_or(Decimal::ZERO)" src/csv_import.rs   # vacía
```

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M08`:
1. Hash del commit.
2. Las últimas 30 líneas de `test.sh`.
3. La salida del `grep`.
4. Las dos reglas [P] y sus pruebas.

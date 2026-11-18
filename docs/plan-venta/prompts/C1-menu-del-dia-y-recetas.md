# C1. Menú del día, combos y descuento de insumos por receta

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (sección 6).

## Contexto

Es la primera tarea del restaurante mínimo, pensado para corrientazos y comidas
rápidas. **Los modificadores ya existen completos** y no hay que rehacerlos:

- `backend/src/options.rs`: grupos de opciones con mínimo y máximo, opciones con
  sobreprecio (`price_delta`) y un insumo que se descuenta del stock;
- la venta los valida, suma el sobreprecio y guarda `order_item_options`
  (`sales.rs`);
- pantalla en `/inventario/modificadores` y selector en el POS
  (`components/pos/customize-dialog.tsx`).

Lo que falta:

1. **El menú del día o combo:** sopa + seco + jugo a un precio fijo, donde el
   cliente elige cada parte.
2. **Que la venta descuente los insumos de la receta.** Hoy `recipes` y
   `recipe_items` existen, pero `catalog::recipe` solo se usa para mostrar
   insumos y calcular disponibilidad; al vender, `sales.rs` descuenta el
   producto mismo y los insumos de las opciones, **no** los de la receta. Para un
   restaurante, que el plato descuente sus ingredientes es lo esperado.

## Lo que quiero

1. **Antes de diseñar,** leé `options.rs`, la parte de `sales.rs` que descuenta
   inventario y `catalog.rs`, y decime si el combo se puede modelar con los
   grupos de opciones que ya existen (por ejemplo, un producto "Menú del día" con
   grupos "Sopa", "Seco" y "Bebida", cada uno de elegir 1, cuyas opciones apuntan
   a un insumo o a un producto). Si alcanza, preferí eso a crear tablas nuevas.
2. **Descuento por receta:** proponé cómo activarlo sin romper a las tiendas que
   hoy venden productos con receta solo como referencia (por ejemplo, un ajuste
   por producto o por negocio). Cuidá el caso de producto con receta **y**
   stock propio, y el caso de stock insuficiente de un insumo: ¿se bloquea la
   venta o se permite quedar en negativo? Proponé y justificá.
3. **Factura DIAN:** cómo se manda un combo al documento POS (una línea con el
   detalle en la descripción o líneas separadas), según lo que acepta MATIAS.
4. Pruebas: de base de datos para que un combo y un plato con receta descuenten
   el inventario correcto, y para que la venta vieja de una tienda no cambie.

**Primero el diseño** y esperá mi visto bueno.

## Fuera de alcance

Mesas y comandas (C2 y C3). Rehacer los modificadores. Sobreprecio negativo
("sin X, −$"), salvo que el diseño lo pida de verdad.

## Terminado cuando

Se vende un menú del día y un plato con receta desde el POS, el tiquete los
muestra bien, el inventario descuenta lo correcto, las ventas de tienda no
cambian, el modo demo trae un menú de corrientazo de ejemplo y todas las
verificaciones pasan.

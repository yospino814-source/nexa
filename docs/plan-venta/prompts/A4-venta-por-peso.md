# A4. Venta por peso y a granel

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (secciones 1 y 2).

## Contexto

Las tiendas y minimercados venden queso, carne, verdura y granos por peso.
Treinta no lo permite y los usuarios se quejan de eso en Google Play; para
nosotros es un diferencial barato.

Lo que ya existe en el repo:
- unidades de medida en `tbl_tipos_maestra`, grupo `UOM`, con `UND`, `G`, `ML`,
  `KG`, `L`, `M` (migraciones `20260924000100` y `20261104000100`);
- las cantidades son `numeric(14,4)`, así que admiten decimales;
- `products.rs` ya valida la unidad de medida y no deja cambiarla si el producto
  tiene movimientos de inventario.

## Lo que quiero

1. Revisá qué falta de verdad para vender 0,350 kg de queso: el POS de hoy
   asume unidades enteras en el teclado, el carrito y el tiquete. Hacé el
   inventario de lo que hay que cambiar.
2. **En el POS:** cuando el producto se vende por peso, pedir la cantidad en
   kilos o gramos, o el monto en pesos ("quiero $5.000 de queso") y calcular el
   peso. Mostrar en el carrito y el tiquete la cantidad con su unidad
   ("0,350 kg × $18.000/kg").
3. **Redondeo:** el precio de la línea se calcula con `money()`, y hay que
   definir y documentar cómo se redondea el peso (proponelo vos).
4. **Códigos de barras de balanza:** las balanzas imprimen etiquetas EAN-13 que
   llevan el peso o el precio dentro del código (los prefijos suelen ser 20–29).
   Investigá el formato, proponé cómo configurarlo por negocio y permití que el
   lector del POS lo interprete. Si resulta mucho para esta tarea, dejalo
   anotado y no lo hagas.
5. Pruebas: unitarias del cálculo peso ↔ precio y del parseo del código de
   balanza; de base de datos para una venta por peso que descuente bien el
   inventario.

**Primero el diseño** y esperá mi visto bueno.

## Fuera de alcance

Conectar la balanza por cable o por serial al computador.

## Terminado cuando

Se puede vender queso por peso y por monto en pesos, el inventario descuenta la
cantidad correcta, el tiquete lo muestra claro, el modo demo lo incluye y todas
las verificaciones pasan.

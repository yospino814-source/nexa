# C2. Cuentas abiertas por mesa, precuenta y propina según la ley

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (sección 6).
Conviene hacerlo **después de C1**. Habilitar el rol `MESERO` en invitaciones va en C3.

## Contexto

Ya existe parte de la base:
- tabla `restaurant_tables` (código, `qr_token`, estado), migración
  `20260924000300_core_sales.sql`;
- `orders.table_id`, `orders.waiter_id` y `orders.tip_amount`. Ojo:
  `orders.inc_amount` guarda **todo** el impuesto de la orden, no solo el INC;
- propina en el cobro (`components/pos/checkout-dialog.tsx`): sin propina,
  sugerida o personalizada (solo enteros), con mesero, si `tipsEnabled` está
  activo;
- el rol `MESERO` está definido pero hoy **no tiene pantallas**, y la invitación
  con ese rol se rechaza con `ROLE_NOT_AVAILABLE`.

Antes de diseñar, revisá cómo se usan hoy esas columnas y si una orden puede
quedar abierta (sin cobrar) y recibir líneas nuevas.

**Ley 1935 de 2018 (propina):**
- es voluntaria y lo sugerido no puede pasar del 10%;
- antes de emitir el documento hay que preguntarle al cliente si la incluye, la
  quita o cambia el monto;
- va en línea aparte y no entra en la base del INC;
- no es ingreso del negocio: pertenece a los trabajadores y, sin acuerdo, se
  reparte por partes iguales.

## Lo que quiero

1. **Lista de mesas** (sin plano gráfico todavía), con su estado: libre,
   ocupada o en precuenta, más el total acumulado y el tiempo abierta.
   Administración de mesas por sucursal.
2. **Cuenta abierta:** abrir una mesa, agregar productos en varias rondas,
   trasladar la cuenta a otra mesa, unir dos mesas.
3. **Precuenta impresa** con el subtotal, el INC y la propina sugerida
   claramente marcada como voluntaria.
4. **Al cobrar:** preguntar la propina (10% por defecto, editable o quitable),
   división de cuenta en partes iguales o por productos, y pagos mixtos sobre la
   caja que ya existe.
5. **Propina fuera del ingreso:** que no sume a la venta neta en los reportes,
   que se vea por mesero en el cierre de caja y que no entre en la base del INC
   del documento DIAN. Revisá cómo la tratan hoy `cash.rs`, los reportes y
   `fiscal/`, y corregí lo que no cumpla.
6. Pruebas para cada regla de la propina y para que una mesa no se pueda cobrar
   dos veces ni recibir líneas una vez cobrada.

**Primero el diseño** y esperá mi visto bueno.

## Fuera de alcance

Comandas a cocina y vista del mesero en el celular (tarea C3), plano gráfico de
mesas, menú QR.

## Terminado cuando

Se puede atender una mesa de principio a fin con dos rondas, precuenta, propina
y cuenta dividida; la propina no aparece como venta en los reportes; el modo demo
lo incluye y todas las verificaciones pasan.

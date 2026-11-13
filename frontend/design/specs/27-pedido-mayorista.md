[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Pedido mayorista"** (el vendedor de ruta toma el pedido de una tienda cliente). Layout: **administración**, usable en tablet. Es un asistente de 3 pasos con indicador arriba: 1 Cliente · 2 Productos · 3 Condiciones.

**Paso 1 — Cliente**
- Buscador por nombre del negocio, NIT o teléfono.
- Debajo, la sección "Clientes de tu ruta de hoy" con tarjetas: nombre del negocio, barrio, y una barra chica de **cupo de crédito** ("Disponible $1.200.000 de $2.000.000"; ámbar si le queda menos del 20%, roja si tiene saldo vencido con el texto "Tiene saldo vencido").
- Al elegir uno, botón "Continuar".

**Paso 2 — Productos**
Reusá los **componentes compartidos "Catálogo" y "Carrito"** del POS mostrador, con estas variantes:
- En las tarjetas del catálogo se ve el **precio mayorista** y la presentación ("Caja x 24", "Paca x 12").
- En el carrito la cantidad se escribe directo en un campo numérico (los pedidos son de muchas unidades), no solo con − +.
- Encabezado del carrito: nombre del cliente elegido y su cupo disponible.

**Paso 3 — Condiciones**
- Forma de pago: dos tarjetas "Contado" y "Crédito".
- Si es crédito: comparación clara "Total del pedido $860.000 · Cupo disponible $1.200.000" en verde. Si el total supera el cupo, en rojo: "Supera el cupo en $140.000" y el botón principal queda deshabilitado (link "Pasar a contado").
- Fecha de entrega: por defecto la próxima visita de la ruta ("Jueves 23 · Ruta Norte"), editable.
- Nota para el repartidor (opcional).
- Resumen con Subtotal, IVA y Total, y botón primario "Confirmar pedido".

**Confirmación**: check verde "Pedido #2041 creado" y una línea "Se entrega el jueves 23 en la Ruta Norte". Botones "Nuevo pedido" (primario) y "Ver pedido".

Datos mock: productos de distribuidora (gaseosas por caja, arroz por paca, aceite por caja, jabón por paca, galletas por display) con precios mayoristas en COP.

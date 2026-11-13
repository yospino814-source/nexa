[Pegar antes el contenido de 00-design-system.md]

Construí la **"App del repartidor"** (lo que ve en el celular quien entrega los pedidos de una ruta). Layout: **operación**, diseñado para **celular en vertical** (ancho ~390px), con botones grandes y texto legible al sol.

**Pantalla 1 — Mis entregas de hoy**
1. Encabezado: "Hoy · Ruta Norte" y una barra de progreso "5 de 12 entregadas".
2. **2 datos** en una fila: "A recaudar $2.340.000" y "Recaudado $980.000" (en verde).
3. Lista de paradas en orden de visita, cada una como tarjeta: número de parada grande, nombre del negocio, barrio, monto a cobrar (o insignia gris "Crédito" si no se cobra) y estado: "Pendiente" (gris), "Entregado" (verde, la tarjeta se atenúa y baja al final) o "Novedad" (ámbar).
4. Banner fijo arriba cuando no hay señal: "Sin conexión — tus entregas se guardan y se envían cuando vuelva la señal".

**Pantalla 2 — Detalle de la entrega** (al tocar una parada)
1. Nombre del negocio, dirección, y dos botones: "Cómo llegar" y "Llamar".
2. Lista de productos del pedido con un check por línea para verificar al descargar. Cada línea permite marcar unidades devueltas o faltantes (ícono de lápiz → campo de cantidad), y el total se recalcula.
3. **Cobro**:
   - Si es de contado: botón "Cobrar $186.000" que abre el **componente compartido "Panel de cobro"** (de la pantalla Cobro y comprobante), solo con Efectivo y Transferencia.
   - Si es a crédito: línea gris "Se suma a la cartera del cliente", sin cobro.
4. Botón primario grande al pie: "Confirmar entrega" (opcional: tomar foto del comprobante firmado, con un cuadro para la foto).
5. Link secundario "Reportar novedad" → hoja inferior con opciones: "Cliente cerrado", "Rechazó el pedido", "Faltó producto", "Otra" + nota.

Datos mock: 12 paradas de tiendas de barrio, 5 entregadas, 1 con novedad, montos entre $85.000 y $420.000.

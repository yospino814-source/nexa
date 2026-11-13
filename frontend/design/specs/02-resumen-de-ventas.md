[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Resumen de Ventas"**.

Contenido:
1. Barra de filtros arriba: selector de rango de fechas (hoy / 7 días / 30 días / personalizado), selector de sucursal (dropdown "Todas las sucursales"), botón "Exportar" (ícono descarga).
2. Fila de 3-4 tarjetas resumen: Total vendido, N° de ventas, Ticket promedio, Ventas devueltas/anuladas.
3. Gráfico de barras grande: ventas por día dentro del rango seleccionado, con tooltip al pasar el mouse mostrando el monto exacto.
4. Tabla debajo: historial de ventas con columnas — Fecha/Hora, N° de orden, Canal (mostrador/WhatsApp/tienda online, como badges de color), Método de pago, Total, Estado (badge verde "Cobrada" / ámbar "Pendiente" / rojo "Anulada"). Incluí paginación simple abajo de la tabla.

Datos mock: 15-20 filas de ventas variadas en COP, mezclando canales y estados.

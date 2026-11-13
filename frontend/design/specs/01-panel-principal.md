[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Panel Principal"** (dashboard de inicio) del sistema POS.

Contenido, de arriba hacia abajo:
1. Fila de 4 tarjetas de estadística: Ingresos (con subtexto "Sin IVA: $X"), Ganancias (con subtexto "Tras gastos: $X · Y% margen"), Balance (con subtexto "Caja acumulada: $X"), Gastos (con subtexto "sin base de comparación"). Cada tarjeta con un ícono circular de color (verde para ganancias, ámbar para gastos, neutro para las demás) en la esquina superior derecha.
2. Fila de 2 columnas (2/3 + 1/3):
   - Columna grande: tarjeta "Tendencia de Ventas" con un gráfico combinado — línea de ingresos por hora (00:00–23:00) sobre barras de cantidad de órdenes por hora. Incluí un selector de período ("Período ▾") arriba a la derecha de la tarjeta.
   - Columna chica: tarjeta "Ventas por Canal" con un gráfico de dona mostrando distribución entre "WhatsApp", "Punto de Venta" y "Tienda Online", con leyenda de colores abajo.
3. Fila de 2 columnas iguales:
   - "Alertas Operativas": lista de 2-3 alertas tipo fila clickeable, cada una con ícono (triángulo de advertencia para stock bajo, círculo con X para agotado, camión para despacho pendiente), texto del tipo "Tienes N productos con stock bajo", y un link "Ver detalle →" alineado a la derecha.
   - "Accesos Rápidos": grilla 2x3 de atajos a otros módulos (Resumen de Ventas, Ventas por Producto, Órdenes, Stock Info, Gastos, Compras), cada uno con ícono + texto, en formato de lista compacta clickeable.
4. Botón flotante circular abajo a la derecha (asistente IA del producto — un robot o chispa como ícono), con una sombra pronunciada para que flote sobre el contenido.

Usá datos mock realistas en COP (ingresos alrededor de $40-46M, gastos ~$90-95K).

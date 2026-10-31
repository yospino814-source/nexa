[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Ventas por Producto"**.

Contenido:
1. Barra de filtros: rango de fechas, categoría de producto (dropdown), buscador de producto.
2. Dos tarjetas destacadas lado a lado: "Producto más vendido" y "Producto menos rentable", cada una mostrando el nombre del producto, una métrica grande (unidades vendidas o % de margen) y un ícono de alerta sutil en la segunda si el margen es crítico (<15%).
3. Tabla principal con columnas: Producto, Categoría, Unidades vendidas, Ingresos generados, Costo, Margen (%, con texto en verde si >30%, ámbar si 15-30%, rojo si <15%), Tendencia (flecha arriba/abajo comparado al período anterior).
4. La tabla debe ser ordenable por columna (indicá visualmente las flechitas de orden en el header).

Datos mock: 10-12 productos de abarrotes/retail colombiano (arroz, leche, gaseosas, café, aseo, etc.) con márgenes variados, incluyendo al menos uno con margen crítico para mostrar el estado de alerta.

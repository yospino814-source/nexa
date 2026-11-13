[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Stock Info"** (inventario).

Contenido:
1. Fila de 3 tarjetas resumen arriba: Total de productos activos, Productos con stock bajo (ámbar), Productos agotados (rojo).
2. Barra de herramientas: buscador de producto, filtro por categoría, filtro por sucursal (si hay más de una), botón "+ Nuevo producto" destacado en el color primario.
3. Tabla principal con columnas: Producto (con mini-ícono o imagen placeholder), Categoría, Precio de venta, Costo, Stock disponible, Estado (badge: verde "OK" / ámbar "Stock bajo" / rojo "Agotado"), acciones (editar/ajustar stock como íconos).
4. Al hacer click en "+ Nuevo producto" o en "editar", mostrá un modal con formulario: nombre, categoría, precio, costo, código de barras, stock inicial.

Datos mock: 12-15 productos variados de retail colombiano, con al menos 3 en stock bajo y 2 agotados para mostrar los estados.

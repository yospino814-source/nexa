[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Ficha de producto"** (crear o editar un producto). Layout: **administración**. Se llega desde la lista de productos (Stock Info) con "Nuevo producto" o tocando un producto.

**Encabezado**
- Breadcrumb "Inventario / Productos / Café Sello 250 g".
- Nombre del producto grande, interruptor "Activo" al lado, y a la derecha botones "Cancelar" (secundario) y "Guardar" (primario).

**Distribución:** columna principal con pestañas (~70%) + columna derecha angosta con una sola tarjeta de resumen (~30%).

**Pestañas de la columna principal**
1. **Datos** — Nombre, Categoría (dropdown), SKU, Código de barras (con botón de ícono "Escanear" dentro del campo), Unidad de medida (Unidad / Gramo / Mililitro), interruptor "Es un producto compuesto (lleva receta)" con una línea de ayuda gris.
2. **Precio y costo** — Costo, Precio de venta base, y el margen calculado mostrado como texto grande en verde (ej. "Margen 34%"; ámbar si es menor a 15%). Debajo, tabla chica "Precio por sucursal": Sucursal · Precio (vacío = usa el precio base, mostrado en gris) · Disponible (interruptor).
3. **Impuesto** — Opciones tipo radio en tarjetas: IVA 19%, IVA 5%, INC 8%, Exento. Debajo, una línea: "Precio al público con impuesto: $11.305".
4. **Receta** — Solo visible si "Es un producto compuesto" está activo. Es el **componente compartido "Editor de receta"** (también se usa en la pantalla Recetas e insumos); diseñalo como pieza aislada. Campo "Rinde" (cantidad de porciones), lista de insumos (insumo, cantidad, unidad, quitar) y buscador "Agregar insumo". Debajo: "Con el stock actual alcanza para 14 porciones".

**Tarjeta derecha — Stock**
- Stock disponible por sucursal (máximo 3 filas: sucursal y número; en ámbar si es bajo).
- Link "Ver movimientos" que lleva a Movimientos de inventario filtrado por este producto.

Validación visual: borde rojo y mensaje corto si falta el nombre o el precio, o si el código de barras ya existe ("Este código ya está en Arroz Diana 500 g").

Datos mock: Café Sello 250 g, categoría Abarrotes, SKU CAF-250, costo $6.200, precio $9.500, IVA 19%, stock Tienda Centro 8, Tienda Norte 23.

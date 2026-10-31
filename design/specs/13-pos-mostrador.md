[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"POS mostrador"** (venta rápida en caja). Layout: **operación** (sin sidebar, barra superior mínima). Es la pantalla que un cajero usa cientos de veces al día: velocidad por encima de todo.

Esta pantalla define dos **componentes compartidos** que después usan otras pantallas (comanda de restaurante, pedido mayorista, tienda virtual). Diseñalos como piezas aisladas:
- **Catálogo**: buscador + filtros de categoría + grilla de productos.
- **Carrito**: líneas de la venta + totales + botón de acción principal.

Distribución en tablet/desktop: Catálogo a la izquierda (~60%), Carrito a la derecha (~40%), ambos ocupando todo el alto.

**Catálogo (izquierda)**
1. Buscador grande arriba, siempre enfocado, placeholder "Buscar o escanear código de barras". Al escanear y presionar Enter, el producto se agrega solo al carrito.
2. Fila de chips de categoría desplazable horizontalmente ("Todas", "Bebidas", "Abarrotes", "Lácteos", "Aseo", "Panadería").
3. Grilla de tarjetas de producto: nombre (máx. 2 líneas), precio grande en color primario, y solo si aplica una insignia: ámbar "Quedan 4" (stock bajo) o roja "Agotado" (la tarjeta queda atenuada y no se puede tocar). Nada más en la tarjeta.

**Carrito (derecha)**
1. Encabezado: "Venta actual" + botón chico "Agregar cliente" (ícono de persona) para ventas a crédito o factura con datos.
2. Lista de líneas: nombre, precio unitario en gris, control de cantidad − número +, y total de la línea a la derecha. Al deslizar o con ícono de papelera, se elimina.
3. Estado vacío: ícono de carrito tenue y el texto "Escaneá o tocá un producto".
4. Pie fijo: Subtotal e Impuestos en texto chico gris, y **Total** grande. Debajo, botón primario enorme (alto ~64px) "Cobrar $XX.XXX", deshabilitado si el carrito está vacío.

**Otros estados**
- Caja cerrada: toda la pantalla atenuada con una tarjeta centrada "Abrí la caja para empezar a vender" y botón "Abrir caja".
- En celular: el Catálogo ocupa la pantalla y el Carrito aparece como barra fija abajo ("3 productos · $12.565 · Ver carrito"), que se expande hacia arriba como hoja.
- Atajos de teclado visibles como pistas chicas: F2 buscar, F4 cobrar, Esc vaciar.

Datos mock (COP, IVA según producto): Coca-Cola 400 ml $3.500 (IVA 19%), Arroz Diana 500 g $2.800 (exento), Leche 1 L $4.200 (exento, quedan 6), Pan tajado $6.900 (agotado), Huevos AA x30 $15.900, Jabón en barra $2.300 (IVA 19%), Café 250 g $9.500 (IVA 19%, quedan 8), Cerveza 330 ml $2.900 (IVA 19%, quedan 3). Mostrá el carrito con 3 líneas cargadas.

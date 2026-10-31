[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Rutas"** (organizar a qué clientes mayoristas se visita, qué días y quién reparte). Layout: **administración**.

**Distribución:** lista de rutas a la izquierda (~35%) y detalle de la ruta seleccionada a la derecha (~65%).

**Lista de rutas (izquierda)**
- Botón primario "Nueva ruta" arriba.
- Una tarjeta por ruta: nombre ("Ruta Norte"), chips de días de visita (L M X J V S, los activos en color primario), avatar + nombre del repartidor, y una línea gris "18 clientes · 6 pedidos para hoy". La seleccionada con borde primario.

**Detalle de la ruta (derecha)**
1. Encabezado: nombre de la ruta (editable), selector de días de visita y dropdown "Repartidor".
2. **3 tarjetas de dato**: Pedidos para hoy, Valor a entregar hoy, Cartera de la ruta (lo que deben los clientes, ámbar si hay vencida).
3. **Clientes en orden de visita**: lista numerada que se puede reordenar arrastrando (ícono de agarre a la izquierda). Cada fila: número de parada, nombre del negocio, barrio en gris, saldo pendiente (si tiene) y una insignia si tiene pedido para hoy. Menú de tres puntos: ver cliente, quitar de la ruta.
4. Botón secundario "Agregar cliente a la ruta" (modal con buscador de clientes que todavía no tienen ruta).

Datos mock: 3 rutas (Norte, Sur, Centro) con días distintos; la Ruta Norte con 8 clientes tipo tiendas de barrio y minimercados ("Tienda Doña Rosa", "Minimercado El Ahorro", "Autoservicio La Esquina"…), 4 con pedido para hoy y 2 con saldo pendiente.

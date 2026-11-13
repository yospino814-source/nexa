[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Órdenes"** (gestión de pedidos).

Contenido:
1. Tabs horizontales arriba para filtrar por estado: Todas / Creada / En preparación / Lista / Despachada / Cancelada — cada tab muestra un contador numérico.
2. Barra de búsqueda por número de orden o cliente, más un filtro de canal (Mostrador / WhatsApp / Tienda Online) como chips seleccionables.
3. Listado de órdenes como tarjetas (no tabla) en una grilla de 2-3 columnas, cada tarjeta mostrando: número de orden, canal (badge con ícono — bolsa para mostrador, WhatsApp verde para ese canal, carrito para tienda online), lista corta de ítems (2-3 líneas, "+N más" si hay más), total, y un badge de estado grande con color según el estado (gris=creada, azul=en preparación, ámbar=lista, verde=despachada, rojo=cancelada).
4. Cada tarjeta clickeable para abrir un detalle (podés mostrar un modal de ejemplo con el detalle completo: ítems, cliente, dirección si es a domicilio, línea de tiempo de estados).

Datos mock: 9-12 órdenes repartidas entre los distintos estados y canales.

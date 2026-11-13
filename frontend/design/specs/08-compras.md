[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Compras"** (entradas de inventario desde proveedor).

Contenido:
1. Botón destacado arriba "+ Nueva compra" / "Registrar factura de proveedor", con un botón secundario "Cargar con IA" (ícono de cámara/documento) que simula subir una foto o PDF de factura de proveedor para autocompletar — mostralo como un drag-and-drop de archivo con un estado de ejemplo "Factura procesada: 12 productos detectados".
2. Fila de 2 tarjetas resumen: Total comprado este mes, N° de proveedores activos.
3. Tabla de historial de compras: Fecha, Proveedor, N° de factura/documento, N° de ítems, Total, Estado (badge: "Registrada" verde / "Pendiente de revisión" ámbar).
4. Al hacer click en una fila, mostrá un modal de detalle con la lista de productos de esa compra: producto, cantidad, costo unitario, subtotal — y un total al final.

Datos mock: 8-10 compras a proveedores colombianos típicos (distribuidoras de abarrotes, bebidas, aseo), con montos realistas en COP.

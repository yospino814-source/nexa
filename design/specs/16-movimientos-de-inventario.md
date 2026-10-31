[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Movimientos de inventario"** (kardex: todo lo que entró y salió del stock). Layout: **administración**.

1. **Barra de filtros**: rango de fechas, sucursal, buscador de producto y tipo de movimiento (Venta, Reposición, Ajuste, Traslado).
2. **3 tarjetas de dato** del período filtrado: Entradas (unidades, verde), Salidas (unidades), Ajustes (unidades, ámbar).
3. **Botones de acción** arriba a la derecha: "Registrar ajuste" (primario), "Reposición" y "Traslado" (secundarios).
4. **Pestañas**: "Movimientos" (por defecto) y "Reservas".

**Pestaña Movimientos** — tabla con 6 columnas:
Fecha y hora · Producto · Tipo (insignia de color: Venta gris, Reposición verde, Ajuste ámbar, Traslado azul) · Cantidad (+12 en verde, −3 en rojo) · Stock después · Referencia (ej. "Venta #1042", "Conteo físico", usuario que lo hizo, en gris). Paginación abajo.

**Pestaña Reservas** — unidades apartadas por pedidos de la tienda virtual que todavía no se entregaron: Producto · Cantidad · Pedido · Vence en (ej. "en 25 min", ámbar si falta poco). Texto de ayuda gris arriba: "Las reservas vencidas se devuelven solas al stock".

**Modales** (mostrá al menos el de ajuste abierto como variante):
- **Registrar ajuste**: producto (buscador), sucursal, dos opciones tipo pestaña "Cantidad contada" o "Sumar / restar", campo de cantidad, motivo (Conteo físico, Daño, Vencimiento, Otro) y nota opcional. Mostrá en vivo "Stock actual 18 → nuevo 15".
- **Reposición**: producto, cantidad, costo unitario opcional.
- **Traslado**: sucursal origen, sucursal destino, y lista de productos con cantidad.

Datos mock: 15 movimientos variados de los últimos 7 días en 2 sucursales, con productos de retail colombiano.

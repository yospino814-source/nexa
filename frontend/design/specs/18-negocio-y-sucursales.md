[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Configuración · Negocio y sucursales"**. Layout: **administración**. Pestañas horizontales arriba; cada pestaña es liviana (un solo bloque principal).

**Pestaña 1 — Negocio**
- Tarjeta con formulario: Logo (cuadro para subir imagen con vista previa), Nombre comercial, Razón social, NIT + DV en la misma fila, Teléfono, Email de contacto.
- Botón "Guardar cambios" abajo a la derecha, habilitado solo si hubo cambios.

**Pestaña 2 — Sucursales**
- Botón primario "Agregar sucursal" arriba a la derecha.
- Grilla de tarjetas, una por sucursal: nombre, ciudad en gris, chips con los tipos de negocio activos (Tienda, Restaurante, Mayorista, Tienda virtual) y número de usuarios. Menú de tres puntos para editar o desactivar.
- Modal "Agregar sucursal": Nombre, Código corto, Ciudad, Dirección, Zona horaria (América/Bogotá por defecto) y los 4 tipos de negocio como tarjetas seleccionables (igual que en la configuración inicial).

**Pestaña 3 — Mesas y cocina** (solo aparece si alguna sucursal tiene Restaurante)
- Selector de sucursal arriba.
- Bloque "Mesas": grilla de cuadrados chicos con el código de cada mesa (M1, M2… T1, T2), cada uno con ícono para descargar su código QR. Botón "Agregar mesa".
- Bloque "Estaciones de cocina": lista simple (Cocina caliente, Bar, Postres) con interruptor de activa y botón "Agregar estación". Una línea gris de ayuda: "Cada plato se envía a la pantalla de su estación".

**Pestaña 4 — Tienda virtual**
- Interruptor grande "Tienda virtual activa".
- Si está activa: dirección de la tienda (campo de solo lectura con botón copiar, ej. "tu-negocio" como identificador), sucursal que despacha los pedidos, horario de atención, costo de envío, pedido mínimo y WhatsApp de contacto.
- Botón secundario "Ver mi tienda".

Datos mock: negocio "Supermercado El Vecino S.A.S" (nombre inventado), NIT 901.234.567-8, sucursales "Tienda Centro" (Tienda, Tienda virtual, 4 usuarios) y "Asadero El Vecino" (Restaurante, 6 usuarios), 12 mesas.

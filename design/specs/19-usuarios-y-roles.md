[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Configuración · Usuarios y roles"**. Layout: **administración**. Dos pestañas: "Usuarios" (por defecto) y "Roles".

**Pestaña Usuarios**
1. Barra: buscador por nombre o email, filtro por rol, filtro por sucursal, y botón primario "Invitar usuario" a la derecha.
2. Tabla con 5 columnas: Usuario (avatar con iniciales, nombre y email en gris debajo) · Rol (insignia) · Sucursal (nombre, o "Todas") · Estado (verde "Activo" o ámbar "Invitación pendiente") · menú de tres puntos (cambiar rol, reenviar invitación, desactivar).

**Modal "Invitar usuario"**
- Email y Nombre.
- Rol: tarjetas tipo radio, cada una con nombre y una sola línea de descripción:
  - Dueño — "Ve todo y configura el negocio"
  - Administrador de caja — "Abre y cierra caja, ve reportes de su sucursal"
  - Cajero — "Vende y cobra en el mostrador"
  - Mesero — "Abre mesas y toma comandas"
  - Cocina — "Ve y despacha los platos en la pantalla de cocina"
  - Repartidor — "Ve sus entregas del día y cobra contra entrega"
  - Bodega — "Recibe compras y ajusta inventario"
- Sucursal: dropdown con las sucursales y la opción "Todas las sucursales".
- Botón primario "Enviar invitación".

**Pestaña Roles** (solo lectura)
- Una tarjeta por rol, en grilla: nombre del rol, número de usuarios con ese rol, y 3-4 permisos principales con ícono de check (ej. Cajero: "Vender en mostrador", "Cobrar", "Ver su caja"). Nada más.

Datos mock: 8 usuarios con nombres colombianos repartidos en roles y 2 sucursales, uno con invitación pendiente.

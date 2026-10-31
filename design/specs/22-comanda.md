[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Comanda"** (el mesero toma el pedido de una mesa y lo envía a cocina). Layout: **operación**.

Reusá los **componentes compartidos "Catálogo" y "Carrito"** de la pantalla POS mostrador (misma estructura: catálogo a la izquierda ~60%, carrito a la derecha ~40%), con las variantes que se describen abajo.

**Catálogo (izquierda), variante carta**
- Chips de categoría: Entradas, Platos fuertes, Sopas, Bebidas, Postres.
- Misma tarjeta de producto: nombre, precio y solo si aplica una insignia. Para platos con receta la insignia dice porciones: ámbar "Quedan 3" o roja "Agotado" (cuando falta un insumo).
- Al tocar un plato se abre un modal chico "Agregar Bandeja paisa": cantidad (− 1 +), chips rápidos de nota ("Sin cebolla", "Término medio", "Bien asado", "Para compartir"), campo de nota libre y botón "Agregar".

**Carrito (derecha), variante comanda**
- Encabezado: "Mesa M4 · 3 personas" y el nombre del mesero en gris.
- Dos grupos de líneas:
  - **"Enviado a cocina"** (arriba, no editable, texto un poco atenuado): cada línea con cantidad, nombre, nota en itálica y una insignia de estado: gris "Pendiente", azul "En preparación", verde "Listo".
  - **"Nuevo"** (abajo, editable): cantidad − +, nombre, nota, y una etiqueta chiquita con la estación a la que va ("Cocina", "Bar").
- Pie: total de la mesa en texto mediano y dos botones: primario grande "Enviar a cocina (3)" (cuenta solo lo nuevo, deshabilitado si no hay nada nuevo) y secundario "Ver cuenta".
- Después de enviar: aviso breve arriba "Enviado: 2 a Cocina caliente, 1 a Bar" y las líneas pasan al grupo "Enviado a cocina".

Botón "← Mesas" en la barra superior para volver al mapa.

Datos mock: carta colombiana — Bandeja paisa $32.000, Ajiaco $24.000, Sancocho de gallina $26.000, Arepa con queso $8.000, Chicharrón $18.000, Limonada de coco $9.000, Cerveza $7.000, Tres leches $10.000. Mostrá 2 líneas ya enviadas y 3 nuevas.

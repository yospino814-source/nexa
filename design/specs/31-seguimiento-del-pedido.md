[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Seguimiento del pedido"** (lo que ve el cliente después de pedir, y desde el link que le llega por WhatsApp). Layout: **público**, **mobile-first**, con la marca del comercio en el encabezado.

1. **Estado actual grande** arriba, en una tarjeta: ícono, título ("Tu pedido va en camino"), número de pedido en gris ("Pedido #3087") y hora estimada ("Llega aprox. 6:40 p. m.").
2. **Línea de tiempo vertical** con 4 pasos: Recibido → En preparación → En camino → Entregado. Cada paso hecho con check y la hora en gris; el actual resaltado en color primario y con una animación suave; los siguientes en gris claro.
   - Si eligió "Recoger en tienda", el tercer paso dice "Listo para recoger" y muestra la dirección de la tienda.
3. **Resumen del pedido** colapsado: "3 productos · $50.800 · Pago contra entrega en efectivo" con link "Ver detalle" que despliega las líneas.
4. Botón secundario ancho completo "Escribir al comercio por WhatsApp".

**Estados (mostralos como variantes)**
- Entregado: check grande verde, "¡Pedido entregado!" y un botón "Volver a pedir" que lleva al catálogo.
- Cancelado: ícono rojo, "Tu pedido fue cancelado", una línea con el motivo y el botón de WhatsApp destacado.

Datos mock: pedido #3087, recibido 6:02 p. m., en preparación 6:05 p. m., en camino 6:24 p. m.

[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Carrito y checkout"** de la tienda virtual. Layout: **público**, **mobile-first**, con la marca del comercio en el encabezado (logo chico + nombre, flecha para volver al catálogo).

Todo en una sola página con 3 secciones que se completan de arriba hacia abajo, y un **resumen fijo** abajo (en desktop, columna derecha).

**1. Tu pedido**
- Reusá el **componente compartido "Carrito"**: líneas con foto chica, nombre, nota, control − +, total de línea y quitar.
- Si el subtotal no llega al mínimo: aviso ámbar "Te faltan $4.200 para el pedido mínimo de $20.000" y link "Seguir comprando".

**2. Entrega**
- Dos tarjetas: "Domicilio" ($5.000) y "Recoger en tienda" (gratis).
- Domicilio: Nombre, Celular / WhatsApp, Dirección, Barrio, Indicaciones (ej. "Casa esquinera, portón verde").
- Recoger: solo Nombre y Celular, y una línea "Listo en unos 20 minutos en Calle 45 #12-30".

**3. Pago**
- "Contra entrega" con dos opciones: Efectivo (campo "¿Con cuánto vas a pagar?" para llevar el cambio) o Transferencia (Nequi / Daviplata al recibir).
- "Pago en línea" visible pero deshabilitado con insignia "Próximamente".

**Resumen fijo**
- Subtotal, Domicilio, **Total** grande y botón primario "Hacer pedido $50.800", deshabilitado mientras falten datos obligatorios o no se llegue al mínimo.

**Estados**
- Producto agotado mientras pedía (variante): aviso ámbar arriba "El Café 250 g se agotó y lo quitamos de tu pedido", con el total ya recalculado.
- Enviando: botón con spinner "Enviando pedido…".
- Al terminar, va a la pantalla de seguimiento del pedido.

Datos mock: 3 productos en el carrito por $45.800 + domicilio $5.000 = $50.800.

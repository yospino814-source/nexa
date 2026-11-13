[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Cocina (KDS)"**: la pantalla que ve el cocinero o el bartender en un monitor o tablet grande en horizontal. Layout: **operación**. Tiene que leerse a 2 metros de distancia: textos grandes, alto contraste, casi nada de decoración.

1. **Barra superior** (variante de la de operación): selector de estación como chips grandes (Todas, Cocina caliente, Bar, Postres), la hora actual grande al centro y contadores a la derecha ("6 pendientes · 4 en preparación").
2. **Tres columnas** de igual ancho: **Pendiente** · **En preparación** · **Listo**.
3. **Tickets**: una tarjeta por mesa y estación, con:
   - Encabezado: código de la mesa grande ("M4"), nombre del mesero en gris y un **temporizador** desde que se envió ("12:30"), con el borde superior de la tarjeta en verde si lleva menos de 10 min, ámbar entre 10 y 20, rojo más de 20.
   - Lista de platos: cantidad grande + nombre ("2 × Bandeja paisa"). Las **notas** debajo de cada plato, en negrita y con fondo ámbar suave ("SIN CEBOLLA"), porque es lo que más se le escapa a la cocina.
   - Botón grande al pie: "Empezar" (en Pendiente), "Listo" (en En preparación), "Entregado" (en Listo, cuando el mesero lo recoge).
4. Un ticket nuevo entra arriba de Pendiente con un borde en color primario que parpadea suave un par de veces.
5. Estado vacío por columna: texto gris "Sin pedidos".

Datos mock: 7 tickets repartidos en las 3 columnas, de mesas M1–M10 y Barra, con tiempos entre 2 y 24 minutos y al menos 3 notas.

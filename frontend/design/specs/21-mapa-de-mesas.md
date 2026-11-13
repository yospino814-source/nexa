[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Mapa de mesas"** (punto de partida del mesero en un restaurante o bar). Layout: **operación** (sin sidebar, barra superior mínima). Pensada para tablet en horizontal.

1. **Barra de resumen** debajo de la barra superior, en una sola línea: "8 libres · 5 ocupadas · 2 por cobrar". A la derecha, botón secundario "Pedido para llevar".
2. **Pestañas de zona**: Salón, Terraza, Barra.
3. **Grilla de mesas** (cuadrados grandes, ~120px), cada una muestra solo lo necesario según su estado:
   - **Libre**: fondo blanco, borde gris, solo el código grande ("M4").
   - **Ocupada**: fondo teal suave, código, tiempo abierta ("45 min") y total actual ("$86.000").
   - **Por cobrar** (el cliente pidió la cuenta): fondo ámbar suave, código y total.
   - Si la mesa tiene platos listos para servir, un punto verde con el número en la esquina ("2").
4. Leyenda chica abajo con los 3 colores.

**Al tocar una mesa libre** → modal chico "Abrir mesa M4": selector de comensales (− 2 +), mesero (por defecto el usuario actual) y botón primario "Abrir y tomar pedido".

**Al tocar una mesa ocupada o por cobrar** → panel lateral derecho con: código, mesero, comensales, tiempo, lista corta de lo pedido (máx. 5 líneas, "+3 más") y total. Dos botones grandes: "Agregar platos" (secundario) y "Ver cuenta" (primario). Si hay platos listos: aviso verde "2 platos listos para servir".

Datos mock: 15 mesas en Salón (M1–M10), Terraza (T1–T3) y Barra (B1–B2), mezclando los 3 estados, con totales entre $24.000 y $180.000.

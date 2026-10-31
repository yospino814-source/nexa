[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Iniciar sesión"**. Layout: **público**. Debe verse como pareja de la pantalla "Crear cuenta" (misma tarjeta centrada, mismos inputs).

Estado 1 — formulario de acceso:
1. Logo de marca + nombre del producto, centrado arriba.
2. Título: "Bienvenido de nuevo". Subtítulo gris: "Entrá para seguir vendiendo".
3. Tarjeta blanca centrada (máx. ~420px):
   - Campo "Email" (label chico en mayúsculas arriba, placeholder "usuario@email.com").
   - Campo "Contraseña" con ícono de ojo para mostrar/ocultar. A la derecha del label, link chico "¿Olvidaste tu contraseña?".
   - Checkbox "Recordarme en este equipo".
   - Botón primario ancho completo "Entrar".
   - Debajo, centrado: "¿No tenés cuenta? Crear cuenta" (link en color primario).
4. Estado de error (mostralo como variante): banner rojo suave arriba del formulario: "Email o contraseña incorrectos".
5. Estado de carga: el botón muestra un spinner y el texto "Entrando…".

Estado 2 — elegir sucursal (aparece solo si el usuario trabaja en más de una sucursal):
- Misma tarjeta centrada. Título "¿Dónde vas a trabajar hoy?".
- Lista de 2-4 sucursales como tarjetas clickeables: nombre de la sucursal, ciudad en gris y el rol del usuario en esa sucursal como insignia (ej. "Cajero", "Administrador").
- Al tocar una, queda seleccionada con borde en color primario y se habilita el botón "Continuar".

Datos mock: sucursales "Tienda Centro" (Bogotá, Administrador), "Tienda Norte" (Bogotá, Cajero), "Bodega Sur" (Soacha, Administrador).

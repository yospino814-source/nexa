[Esta pantalla NO usa el sidebar/topbar del sistema de diseño — es una pantalla pública de autenticación, antes de entrar al panel. Usá la misma paleta (azul petróleo como color primario, no verde) y la misma tipografía del resto del producto.]

Construí la pantalla pública **"Crear cuenta / Registro de organización"**.

Layout: todo centrado verticalmente en la página, fondo `slate-50` liso.

De arriba hacia abajo:
1. Logo de marca (cuadrado redondeado con una letra, color primario) + nombre del producto al lado, centrado.
2. Título grande: "Crea tu organización". Subtítulo chico debajo: "Empezá a vender en minutos".
3. Tarjeta blanca centrada (`rounded-2xl`, sombra suave, ancho máx ~480px) con el formulario:
   - Campo "Nombre de la organización" (ancho completo), placeholder "Mi Empresa S.A.S".
   - Fila de 2 columnas: "Nombre" / "Apellido".
   - Fila de 2 columnas: "Email" (placeholder "usuario@email.com") / "Teléfono" (placeholder "+57 123 456 7890", con texto chico "Opcional" debajo).
   - Campo "Contraseña" (ancho completo) con ícono de ojo a la derecha para mostrar/ocultar.
   - Botón primario ancho completo "Crear cuenta", color primario sólido, texto blanco.
   - Debajo del botón, centrado: "¿Ya tienes cuenta? Inicia sesión" (el link en color primario).
4. Debajo de la tarjeta, centrado y en gris chico: "← Volver al inicio".
5. Al final de la página, texto muy pequeño y gris: nombre de la plataforma + "Powered by [tu marca]".

Todos los inputs con label chico en mayúsculas gris arriba (no placeholder como label — label real + placeholder de ejemplo adentro). Bordes sutiles en los inputs, con el borde resaltado en el color primario cuando el campo tiene foco (mostralo así en el campo Email y Contraseña para dar sensación de estado activo).

No necesita lógica real de autenticación — solo la UI estática con validación visual básica (borde rojo + texto de error si un campo requerido queda vacío al simular submit).

# Sistema de diseño Nexa POS (pegar este bloque al INICIO de cada prompt individual)

No es una copia de ningún producto existente — es una marca y paleta propias, inspiradas solo en el nivel de pulido visual de dashboards SaaS modernos.

**Stack:** Next.js (App Router) + TypeScript + Tailwind CSS + shadcn/ui + lucide-react para íconos. Datos mock únicamente, sin backend real todavía.

**Layouts (cada prompt dice cuál usa):**

*Layout de administración* — para gestionar y revisar (panel, reportes, inventario, clientes, caja, configuración):
- Sidebar izquierdo angosto (72px), fondo oscuro (`slate-900`), solo íconos (sin texto), con tooltip al hover. Logo arriba (cuadrado redondeado con una letra), ítem activo resaltado con fondo teal suave y barra lateral.
- Barra superior blanca: breadcrumb a la izquierda ("Panel Administrativo / [Nombre de la pantalla]"); a la derecha: selector de sucursal activa (dropdown con el nombre de la sucursal), campana de notificaciones con badge numérico, avatar circular + nombre de usuario + rol, con chevron de dropdown.

*Layout de operación* — para una tarea rápida a pantalla completa (POS mostrador, comanda, cocina, repartidor):
- Sin sidebar. Barra superior mínima de 56px: logo a la izquierda, nombre de la sucursal + indicador "Caja abierta" (punto verde) al centro, usuario + botón "Volver al panel" a la derecha.
- Botones grandes (mínimo 48px de alto), pensados para dedo en tablet.

*Layout público* — acceso y tienda virtual (login, registro, configuración inicial, catálogo del comercio):
- Sin sidebar ni barra de administración. Contenido centrado sobre fondo `slate-50`.

**Superficies (todos los layouts):** fondo general `slate-50`, tarjetas blancas con `rounded-2xl`, `shadow-sm`, borde `slate-200/60`.

**Paleta (propia, NO uses verde menta como color de marca — ese no es nuestro color):**
- Primario / marca: azul petróleo — `#0F6E6E` o similar (tailwind: algo entre `teal-700` y `cyan-800`).
- Éxito / positivo: `emerald-500`.
- Advertencia: `amber-500`.
- Peligro / crítico: `rose-500`.
- Texto: `slate-900` sobre fondo claro, `slate-500` para texto secundario.

**Tipografía:** Inter o system-ui. Los montos en pesos colombianos (COP), formato `$1.250.000` (sin decimales, punto como separador de miles).

**Principio de diseño: liviano, no sobrecargar al usuario**
- Máximo 3-4 datos numéricos destacados por pantalla a simple vista (los que importan para decidir algo ahora). Todo lo demás queda detrás de un "Ver más" / "Ver detalle" / una pestaña secundaria, no todo junto en la primera vista.
- Mucho espacio en blanco entre bloques (`gap-6` o más entre secciones) — preferí menos elementos por pantalla antes que achicar todo para que entre más.
- Tablas: máximo 5-6 columnas visibles; si hay más datos, van en un detalle al hacer click en la fila, no agregando columnas.
- Nada de texto secundario en negrita ni compitiendo visualmente con el dato principal — jerarquía clara: 1 dato grande, 1 subtexto chico y gris, listo.
- En pantallas operativas (checkout, comanda, cierre de caja) priorizá que la acción principal (cobrar, enviar a cocina, etc.) sea lo más grande y visible de la pantalla — todo lo demás es secundario.
- Si una pantalla te queda con más de ~7 bloques de información distintos, dividila en tabs o en "ver más" en vez de mostrarlo todo de una.

**Reglas:**
- Esta pantalla es UNA pieza de una serie de pantallas de un mismo producto — usá exactamente el layout que indique el prompt, tal como está descrito arriba, para que todas las pantallas se vean consistentes entre sí.
- Cuando el prompt nombre un componente compartido (por ejemplo "Carrito" o "Panel de cobro"), diseñalo como una pieza aislada y reutilizable, porque otras pantallas lo van a usar igual.
- Mobile-first en las pantallas operativas (se usan en tablet/celular de mostrador); desktop-first está bien en las pantallas administrativas/reportes.
- No reproduzcas logos, nombres de marca ni el verde menta característico de ningún competidor.

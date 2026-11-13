[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Recetas e insumos"** (qué lleva cada plato y cuánto alcanza con el stock de hoy). Layout: **administración**. Dos pestañas: "Platos" (por defecto) e "Insumos".

Esta pantalla define el **componente compartido "Editor de receta"**, que también aparece en la pestaña Receta de la Ficha de producto. Diseñalo como pieza aislada.

**Pestaña Platos**
1. **3 tarjetas de dato**: Platos con receta, Platos agotados hoy (rojo), Margen promedio.
2. Buscador y filtro por categoría de la carta.
3. Tabla con 6 columnas: Plato · Insumos (número) · Costo por porción · Precio · Margen (verde > 30%, ámbar 15–30%, rojo < 15%) · Porciones disponibles hoy (ámbar si quedan menos de 5, rojo "Agotado" si es 0).
4. Al tocar un plato se abre un **panel lateral** con el **Editor de receta**:
   - Campo "Rinde" (porciones).
   - Lista de insumos: insumo, cantidad, unidad (g, ml, und), costo de esa cantidad, botón quitar. El insumo que limita las porciones se marca con una insignia ámbar "Limita".
   - Buscador "Agregar insumo".
   - Pie con Costo por porción, Precio y Margen recalculados en vivo, y botón "Guardar receta".

**Pestaña Insumos**
- Tabla con 5 columnas: Insumo · Unidad · Stock (en ámbar o rojo si está bajo) · Costo por unidad · Usado en (número de platos).
- Si un insumo está bajo o agotado, debajo de su nombre aparece en gris a qué platos afecta: "Afecta: Bandeja paisa, Chicharrón".

Datos mock: platos colombianos (Bandeja paisa $32.000, Ajiaco $24.000, Sancocho de gallina $26.000, Chicharrón $18.000, Limonada de coco $9.000, Tres leches $10.000) e insumos con unidades reales (frijol g, arroz g, carne molida g, chicharrón g, huevo und, plátano maduro und, aguacate und, pollo g, papa criolla g, leche de coco ml). Que el chicharrón esté bajo y deje la Bandeja paisa en 3 porciones.

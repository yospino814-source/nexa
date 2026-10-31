[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Cuenta de mesa"** (revisar, dividir y cobrar lo que consumió una mesa). Layout: **operación**. Se llega desde el mapa de mesas o la comanda con "Ver cuenta".

Para cobrar, reusá el **componente compartido "Panel de cobro"** de la pantalla "Cobro y comprobante" (mismo modal, mismos métodos de pago y comprobante).

**Distribución:** detalle de la cuenta a la izquierda (~55%) y opciones de cobro a la derecha (~45%).

**Detalle de la cuenta (izquierda)**
- Encabezado: "Mesa M4 · 3 personas · 1 h 20 min" y botón secundario "Imprimir pre-cuenta".
- Lista de lo consumido: cantidad, plato, total de la línea.
- Totales al pie: Subtotal, **INC 8%** (impuesto al consumo de restaurantes), **Propina voluntaria** y **Total** grande.

**Opciones de cobro (derecha)**
1. **Propina voluntaria**: 3 botones — "Sin propina", "10% sugerida" (seleccionada por defecto) y "Otro monto". Texto gris debajo: "Preguntale al cliente si desea incluirla".
2. **Dividir cuenta**: selector de 3 opciones — "Una sola cuenta" (por defecto), "Partes iguales", "Por platos".
   - **Partes iguales**: selector de número de personas (− 3 +) y debajo "$59.000 por persona".
   - **Por platos**: cada línea del detalle muestra chips de persona (P1, P2, P3) para asignarla; abajo, el subtotal de cada persona.
3. Lista de **partes** con su estado: "Persona 1 · $59.000 · Pagada ✓" / "Persona 2 · $59.000 · Cobrar". Cada "Cobrar" abre el Panel de cobro con ese monto. Progreso arriba: "1 de 3 pagadas".
4. Cuando todo está pagado: check verde "Cuenta saldada" y botón primario grande "Liberar mesa" (la mesa vuelve a Libre en el mapa).

Datos mock: 2 Bandeja paisa ($64.000), 1 Ajiaco ($24.000), 1 Sancocho de gallina ($26.000), 1 Arepa con queso ($8.000), 2 Limonada de coco ($18.000), 1 Tres leches ($10.000) → subtotal $150.000, INC 8% $12.000, propina 10% sobre el subtotal $15.000, total $177.000. En partes iguales entre 3: $59.000 por persona.

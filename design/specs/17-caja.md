[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Caja"** (apertura, movimientos y cierre del turno de un cajero). Layout: **administración**. La pantalla cambia según el estado de la caja; mostrá los 3 estados.

**Estado 1 — Caja cerrada: apertura**
- Tarjeta centrada: título "Abrir caja", una línea gris con el último cierre ("Último cierre: ayer 9:42 p. m. por Laura · diferencia $0").
- Campo grande "Base en efectivo" (dinero con el que arranca el turno), placeholder "$100.000".
- Botón primario grande "Abrir caja".

**Estado 2 — Caja abierta: movimientos del turno**
1. Encabezado: "Caja abierta desde 7:00 a. m." con un punto verde, y botón primario "Cerrar caja" a la derecha.
2. **3 tarjetas de dato**: Efectivo esperado ahora (el más grande), Ventas del turno, Retiros.
3. Botones secundarios: "Registrar ingreso" y "Registrar retiro" (modal chico: monto + motivo, ej. "Pago a proveedor de hielo").
4. Lista de movimientos del turno: hora · descripción (Venta #1042, Retiro, Ingreso) · método · monto (+ verde / − rojo).

**Estado 3 — Cierre con arqueo ciego** (2 pasos en la misma pantalla)
- **Paso 1, contar**: el cajero cuenta el efectivo sin ver cuánto debería haber. Lista de denominaciones colombianas, cada una con campo de cantidad y subtotal: billetes de $100.000, $50.000, $20.000, $10.000, $5.000, $2.000 y monedas de $1.000, $500, $200, $100, $50. Total contado grande abajo. Botón "Ver resultado".
- **Paso 2, resultado**: Contado vs Esperado lado a lado y la **Diferencia** destacada (verde si es $0, ámbar si es de hasta $5.000 en cualquier sentido, rojo si es mayor). Debajo, resumen corto por método: Efectivo, Tarjeta, Transferencia. Campo "Nota" (obligatorio si hay diferencia). Botón primario "Cerrar caja".
- Después de cerrar: check verde "Caja cerrada" y botones "Imprimir cierre" y "Volver al panel".

Datos mock: base $100.000, ventas en efectivo $1.720.000, retiro $50.000, esperado $1.770.000, contado $1.765.000 (diferencia −$5.000, ámbar).

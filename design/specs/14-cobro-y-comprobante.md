[Pegar antes el contenido de 00-design-system.md]

Construí el **"Cobro y comprobante"**: un modal grande que se abre sobre el POS mostrador al tocar "Cobrar". Layout: **operación** (modal centrado; en celular ocupa toda la pantalla).

Este modal es el **componente compartido "Panel de cobro"**: después lo reusan la cuenta de mesa del restaurante, la app del repartidor (cobro contra entrega) y la tienda virtual. Diseñalo como pieza aislada.

**Paso 1 — Cómo paga**
1. Arriba, centrado y muy grande: "Total a cobrar $62.237".
2. 4 botones grandes de método, en una fila: Efectivo, Tarjeta, Transferencia (Nequi / Daviplata), Mixto. Seleccionado = fondo teal suave y borde primario.
3. Si es **Efectivo**:
   - Campo grande "Recibido".
   - Botones rápidos de billetes: "Exacto", "$70.000", "$100.000" (sugeridos según el total).
   - Debajo, el **cambio** en verde y grande: "Cambio $37.763".
4. Si es **Mixto**: lista de pagos agregados (método + monto, con opción de quitar), botón "Agregar otro pago", y una línea que dice cuánto falta ("Falta $12.237") en ámbar hasta que llegue a $0.
5. Si es **Tarjeta** o **Transferencia**: solo un campo opcional "Referencia / últimos 4 dígitos".

**Comprobante** (debajo de los métodos, en una sola fila):
- Selector de dos opciones: "Tirilla POS" (por defecto) o "Factura electrónica".
- Si elige factura: aparece un buscador de cliente (por nombre, cédula o NIT) con link "Crear cliente rápido" (nombre, tipo y número de documento, email).

Botón primario ancho completo: "Confirmar pago". Deshabilitado mientras falte dinero.

**Paso 2 — Venta registrada** (mismo modal)
- Check grande verde, título "Venta registrada".
- Si fue en efectivo, el cambio a entregar en grande: "Entregá $37.763".
- 3 botones: "Imprimir", "Enviar por WhatsApp" (secundarios) y "Nueva venta" (primario, grande, con foco por defecto para que Enter empiece la siguiente venta).

Estado de error (variante): si la factura electrónica no se pudo emitir, banner ámbar "La venta quedó registrada. La factura se reintentará automáticamente." — nunca bloquear la venta por un error de facturación.

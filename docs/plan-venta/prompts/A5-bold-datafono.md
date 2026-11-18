# A5. Datáfono Bold integrado: el monto le llega solo

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (sección 5).
**Requiere la tarea A2** (ya habrá webhook de Bold funcionando).

## Contexto

Hoy, cuando el cliente paga con datáfono, el cajero digita el monto en el
datáfono y después lo registra a mano en Nexa. Se equivoca y no cuadra la caja.

Bold tiene API para esto: `POST /payments/app-checkout`, que recibe monto,
impuestos, propina, `terminal_model` y `terminal_serial`, más
`GET /payments/binded-terminals` para listar los datáfonos vinculados. Requiere
certificación con Bold antes de producción y funciona con los Smart y Smart Pro
con firmware reciente. Alegra y Loggro ya lo usan. Existe también un deeplink
("Applink") como alternativa.

Leé la documentación de `developers.bold.co` antes de diseñar y decime cuál de
las dos vías conviene.

## Lo que quiero

1. Diseño: cómo se vincula un datáfono a una sucursal, cómo se dispara el cobro
   desde el cobro del POS, cómo se espera la respuesta sin congelar la pantalla,
   y qué pasa si el cliente cancela, el datáfono se queda sin señal o el cajero
   cierra la ventana. **Esperá mi visto bueno.**
2. Pantalla de configuración para vincular y nombrar los datáfonos de la
   sucursal.
3. En el cobro: al elegir datáfono, mandar el monto y mostrar el estado
   ("Pasá la tarjeta", "Aprobado", "Rechazado"). El pago se registra solo cuando
   Bold confirma, con la referencia de la transacción.
4. **La propina:** si se manda a Bold, tiene que coincidir con `orders.tip_amount`
   y respetar la regla de la Ley 1935 (voluntaria, en línea aparte, fuera de la
   base del impuesto). Ver la tarea C2.
5. Pruebas unitarias del armado del cobro y del manejo de cada estado, y de base
   de datos para que un pago no se registre dos veces.

## Fuera de alcance

Otros datáfonos (Redeban, Credibanco, Mercado Pago Point).

## Terminado cuando

Con un datáfono de prueba de Bold, el cobro desde el POS llega al datáfono y el
pago aprobado queda registrado una sola vez con su referencia. Todas las
verificaciones pasan.

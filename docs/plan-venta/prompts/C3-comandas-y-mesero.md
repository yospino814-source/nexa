# C3. Comanda impresa por estación y vista de mesero en el celular

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (secciones 6 y 7).
**Requiere C1 y C2.**

## Contexto

Hoy Nexa imprime con `window.print()`, que sirve para el tiquete del cliente
pero no para mandar una comanda a la cocina sin que nadie toque nada. Un
navegador **no puede** abrir directamente el puerto 9100 de una impresora de
red. Las salidas reales son:

- impresoras con servidor HTTP propio (Epson ePOS-Print, Star WebPRNT);
- un agente local en el computador de la caja (QZ Tray, que sin certificado
  pago muestra un aviso en cada impresión, o un puente propio) que recibe la
  comanda y la manda a la impresora;
- WebUSB o Web Serial, solo en Chrome y solo con impresora por cable;
- un celular Android que haga de puente.

En los locales colombianos son comunes las Epson TM-T20 y las Xprinter de 80 mm.
La queja más repetida contra Loggro Restobar son las comandas que no se
imprimen: **la confiabilidad importa más que lo bonito**.

## Lo que quiero

1. **Investigación y propuesta de impresión.** Compará las opciones de arriba
   por costo para el negocio, cuánto hay que instalar, confiabilidad y qué
   impresoras soporta. Recomendá una, con un plan B. **Esperá mi decisión antes
   de construir.**
2. **Estaciones:** cada producto (o categoría) se asigna a una estación (cocina,
   bar, parrilla). Al enviar una ronda, cada estación recibe solo sus líneas, con
   mesa, mesero, hora y modificadores bien visibles. **Solo se imprime lo
   nuevo**, nunca la cuenta entera otra vez.
3. **Si la impresión falla,** el mesero y la caja se enteran y pueden
   reimprimir. Ninguna comanda se pierde en silencio.
4. **Vista de mesero en el celular:** pantallas del rol `MESERO` (que hoy no tiene
   ninguna: lo frena `session-gate.tsx` y la invitación devuelve
   `ROLE_NOT_AVAILABLE`; hay que habilitarlo en los dos lados), hechas para una mano y pantalla chica. Ve sus mesas, abre una,
   agrega productos con modificadores y envía la ronda a cocina. No cobra, salvo
   que tenga ese permiso.
5. Pruebas del reparto de líneas por estación y de que una ronda no se imprima
   dos veces.

## Fuera de alcance

Pantalla de cocina en tablet (KDS), Rappi y DiDi, menú QR para que pida el
cliente.

## Terminado cuando

Un mesero toma un pedido en el celular, la comanda sale en la impresora de
cocina y la de bar con sus líneas, una falla de impresión se ve y se puede
reimprimir, y todas las verificaciones pasan.

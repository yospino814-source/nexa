# A3. Fiado: recordatorio de cobro por WhatsApp con link de pago

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (secciones 2, 5 y 4).
**Requiere que la tarea A2 esté terminada** (worker de outbox y Bold).

## Contexto

Nexa ya tiene clientes, cupo de crédito, ventas a crédito, abonos y cartera
(`receivables.rs`, `credit.rs`, pantalla `/cartera`). Lo que no tiene es cerrar
el ciclo: el tendero le recuerda al cliente lo que debe, el cliente paga desde el
celular y el abono se aplica solo. Existen apps enteras solo para esto (Fiao,
KIPU, COfiado); para nosotros es el gancho de venta.

**Costos de WhatsApp:** desde el 1 de octubre de 2026 los mensajes de utilidad
se cobran (~$3 cada uno en Colombia), y si Meta considera que el texto es
promocional lo cobra como marketing, unas 15 veces más. El recordatorio debe ser
una plantilla de **utilidad** que cite un saldo concreto, sin tono comercial.

## Lo que quiero

1. **Desde la cartera:** botón "Recordar por WhatsApp" por cliente, y un envío
   en lote para los que tienen saldo vencido. Solo con permiso de cartera.
2. **El mensaje:** plantilla de utilidad con nombre del negocio, saldo y un link
   de pago Bold por el saldo (o un monto que el cliente elija, si Bold lo
   permite). Proponé el texto de la plantilla para que yo la registre en Meta.
3. **Al pagar:** el webhook de Bold registra un abono a la cuenta del cliente,
   usando la lógica de abonos que ya existe (con su idempotencia y su auditoría).
4. **Límites:** como máximo un recordatorio por cliente cada 7 días, y solo a
   clientes que aceptaron recibir mensajes (agregá el consentimiento si no
   existe). Se registra cada envío.
5. **Opcional, si es poco esfuerzo:** cuando el negocio no tiene Bold, mandar el
   mensaje sin link, con la llave Bre-B o el número de Nequi que configure el
   tendero.

**Primero el diseño** y esperá mi visto bueno.

## Fuera de alcance

Recordatorios automáticos programados (lo vemos después de probar el manual),
otras pasarelas.

## Terminado cuando

Desde `/cartera` sale un recordatorio real, el pago con el link aplica el abono
una sola vez, el modo demo tiene su mock y todas las verificaciones pasan.

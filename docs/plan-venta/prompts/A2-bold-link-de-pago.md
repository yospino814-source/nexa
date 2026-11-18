# A2. Link de pago Bold para los pedidos de WhatsApp

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (sección 5).

## Contexto

Cuando un cliente elige pagar con link, `backend/src/whatsapp/mod.rs` (cerca de
la línea 360) encola un mensaje `WHATSAPP_PAYMENT_LINK` en la tabla `outbox`,
con `order_id`, `code`, `total`, `wa_phone` y `phone_number_id`. **No existe
ningún worker que procese el outbox**: en `main.rs` solo arranca
`fiscal::spawn_worker`, que procesa `fiscal_documents` y no el outbox: hoy el
outbox solo recibe inserts. Ya existe `whatsapp::confirm_payment`, que marca un
pedido de `whatsapp_orders` como pagado; hoy se llama a mano, pide una
referencia de al menos 3 caracteres y el permiso de cobro. Va a hacer falta una
variante de sistema para que la use el webhook.

Bold tiene:
- link de pago: `POST https://integrations.api.bold.co/online/link/v1` (monto,
  expiración, `callback_url`);
- webhooks con eventos `SALE_APPROVED`, `SALE_REJECTED` y
  `CANCELLATION_*`;
- consulta de estado de la transacción.

Leé la documentación de `developers.bold.co` antes de diseñar; confirmá cómo se
firma o valida el webhook.

## Lo que quiero

1. **Un worker genérico de outbox** con el mismo patrón del worker fiscal
   (lease, reintentos con backoff, `FISCAL_WORKER=0` como modelo para poder
   apagarlo), que despache por `kind`. Por ahora solo maneja
   `WHATSAPP_PAYMENT_LINK`.
2. **Handler de `WHATSAPP_PAYMENT_LINK`:** crea el link en Bold con la llave del
   negocio, lo guarda en el pedido y se lo manda al cliente por WhatsApp.
3. **Webhook de Bold** (`/v1/payments/bold/webhook`): valida la firma, es
   idempotente, y con `SALE_APPROVED` llama a `confirm_payment`.
4. **Configuración por negocio:** dónde se guarda la llave de Bold de cada
   tienda. Proponé la opción. No la pongas en texto plano si hay una forma
   razonable de evitarlo.
5. Pruebas: unitarias para armar el request y validar la firma; de base de datos
   para el webhook (idempotencia y aislamiento entre negocios).

**Primero mandame el diseño** (tablas, rutas, cómo se guarda la llave, qué pasa
si Bold falla o el link vence) y esperá mi visto bueno.

## Fuera de alcance

Datáfono (es la tarea A5), Bre-B, otras pasarelas.

## Terminado cuando

En modo prueba de Bold, un pedido de WhatsApp con pago por link recibe el link,
el pago aprobado lo marca como pagado una sola vez, y `test.sh`, `tsc`, `eslint`
y `next build` pasan.

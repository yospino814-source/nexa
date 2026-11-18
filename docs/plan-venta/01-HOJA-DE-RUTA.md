# Hoja de ruta para salir a vender

Ordenada por lo que más acerca a un cliente que paga. Los hallazgos que la
sustentan están en `00-HALLAZGOS.md`. Cada tarea tiene su prompt en `prompts/`,
listo para pegar en Claude Code.

**Cómo usar los prompts:** abrí Claude Code en la carpeta `nexa/`, entrá en modo
plan (Shift+Tab dos veces) y pegá el prompt. Todos piden primero un diseño y
esperan tu visto bueno antes de escribir código. Hacé una tarea por sesión.

## Etapa A: lista para vender a tiendas y licoreras

Sin esto no conviene salir a vender.

| # | Tarea | Por qué | Depende de vos | Prompt |
|---|---|---|---|---|
| A1 | Probar MATIAS contra el sandbox real | La factura electrónica es obligatoria para licoreras y minimercados, y nunca se probó de verdad | Crear la cuenta de MATIAS y pegar el token de sandbox | `A1-matias-sandbox.md` |
| A2 | Link de pago Bold para pedidos de WhatsApp | El bot ya encola `WHATSAPP_PAYMENT_LINK`, pero nadie lo procesa | Cuenta Bold y llaves de API | `A2-bold-link-de-pago.md` |
| A3 | Fiado: recordatorio de cobro por WhatsApp con link de pago | Es la función gancho del tendero; cierra el ciclo fiado → recordatorio → pago → abono | Aprobar plantillas en Meta | `A3-fiado-recordatorio.md` |
| A4 | Venta por peso y a granel | Treinta no lo tiene y es queja frecuente; ya existen las unidades KG y G | — | `A4-venta-por-peso.md` |
| A5 | Datáfono Bold integrado (el monto llega solo) | Quita el registro manual y sus errores; Alegra y Loggro ya lo tienen | Certificación con Bold y un datáfono Smart | `A5-bold-datafono.md` |

## Etapa B: que no se caiga sin internet

| # | Tarea | Por qué | Prompt |
|---|---|---|---|
| B1 | POS que vende sin conexión y sincroniza al volver | Es la queja número uno del mercado. Nexa ya manda un `id` UUID generado en el navegador por operación, que es la base para hacerlo seguro | `B1-pos-sin-internet.md` |

## Etapa C: restaurante mínimo (corrientazo y comidas rápidas)

Más de la mitad ya existe: mesas, mesero y propina en la orden, INC 8%,
recetas y **modificadores completos con su pantalla**. Falta el menú del día y
la operación del salón.

| # | Tarea | Prompt |
|---|---|---|
| C1 | Menú del día, combos y descuento de insumos por receta | `C1-menu-del-dia-y-recetas.md` |
| C2 | Cuentas abiertas por mesa, precuenta y propina según la Ley 1935 | `C2-mesas-y-precuenta.md` |
| C3 | Comanda impresa por estación (cocina, bar) y vista de mesero en el celular | `C3-comandas-y-mesero.md` |

## Etapa D: después de los primeros clientes

Sin prompt todavía, porque dependen de lo que pidan los clientes reales:

- **Bre-B con QR dinámico y confirmación automática** vía Kushki, Mono o el QR
  de Wompi (1%). Mientras tanto, el QR estático de Nequi (0%) se registra como
  transferencia.
- **Recargas y pagos de servicios con Puntored**, que tiene API pública y le deja
  comisión al tendero.
- **Pantalla de cocina en tablet, plano gráfico de mesas y menú QR.**
- **Rappi y DiDi Food,** directo o vía un agregador como Deliverect u Ordatic.
- **Pedido sugerido a proveedores** a partir del inventario, como integración
  comercial con BEES, Juntos+ o Surtiapp.
- **ICL en la factura,** cuando un contador confirme la tarifa.

## Lo comercial (no es código, pero decide si se vende)

1. **Precio sin trampas.** Un plan de $29.900–39.900 con documentos POS
   ilimitados y el mismo precio al renovar. Es el ataque directo a los topes de
   Siigo, Loggro y Fudo y a las subidas de Treinta.
2. **Plan gratis sin facturación** para los no responsables de IVA. Se cobra
   cuando el negocio queda obligado a facturar.
3. **Programa de contadores** con 15–20% de comisión recurrente y un usuario
   gratis. Es el canal por el que crecieron Alegra y Siigo.
4. **Soporte humano por WhatsApp con respuesta en menos de una hora.** Es la
   queja principal contra Siigo y Loggro.
5. **Kit de hardware** (impresora + lector) en cuotas con el plan anual. Nadie
   ofrece comodato en tiendas.
6. **Costos de WhatsApp:** desde octubre de 2026 los mensajes de utilidad
   dentro de la ventana de 24 horas se cobran. Hay que meterlo en el precio del
   plan o poner un tope de mensajes.

Los planes y su cobro dentro de la app se construyen cuando estos números estén
decididos; no hay prompt para eso todavía.

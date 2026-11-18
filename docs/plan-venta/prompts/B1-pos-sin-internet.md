# B1. Que el POS venda sin internet

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (secciones 2 y 7).

## Contexto

Es la queja número uno del mercado: cuando se cae internet, el negocio no puede
vender. Alegra, Toteat y Poster funcionan sin conexión; Fudo no. En 2025 solo el
53,8% de los hogares colombianos tenía internet fijo.

Hoy el repo **no tiene nada** de esto: no hay service worker, ni manifest, ni
IndexedDB. Pero sí tiene la pieza más importante: **las ventas, devoluciones,
gastos, compras, abonos y pagos llevan un `id` UUID que genera el navegador**
(por ejemplo, `lib/stores/cart.ts`). Si llega repetido con el mismo cuerpo, el
backend devuelve el resultado guardado; si cambia el cuerpo, responde
`IDEMPOTENCY_CONFLICT` (ver `sales.rs`). Eso es justo lo que hace seguro
reenviar una cola de ventas. Ojo con la excepción: en los ajustes de inventario
el UUID se crea dentro de `lib/api/index.ts`, y cada reintento genera uno nuevo.

El patrón habitual es: PWA con service worker, IndexedDB como fuente de verdad
local, cola de operaciones pendientes y reenvío al volver la conexión.

## Lo que quiero

**Esta tarea es grande, así que quiero el diseño en detalle y lo vamos a partir
en varias entregas.** No escribas código hasta que acordemos el plan.

En el diseño respondeme:

1. **Alcance mínimo honesto.** Propongo: vender y cobrar en efectivo sin
   conexión. ¿Qué queda afuera y por qué? (crédito y fiado necesitan saber el
   cupo; el datáfono y los links necesitan red; la factura DIAN se emite
   después).
2. **Qué se guarda en el dispositivo:** catálogo con precios, stock conocido,
   clientes, turno de caja abierto. Cómo se refresca y qué tan viejo puede estar.
3. **La cola:** qué se encola, en qué orden se reenvía, qué pasa si el backend
   rechaza una venta ya cobrada al cliente (por ejemplo, porque no hay stock), y
   cómo se le avisa al tendero sin asustarlo.
4. **Numeración y tiquete:** hoy el consecutivo lo da el backend
   (`sequences::next_tx`). ¿Qué número se imprime estando sin conexión?
5. **La factura DIAN:** cómo se emite cuando vuelve la red, y qué dice el
   tiquete mientras tanto.
6. **El riesgo que más me preocupa:** que el modo sin conexión abra un hueco de
   seguridad o deje vender a quien no tiene permiso. Decime cómo se evita, y
   cuánto tiempo puede valer una sesión guardada en el dispositivo.
7. **Cómo se prueba** de verdad, no solo "funcionó en mi máquina".
8. **Partición en entregas,** de menor a mayor riesgo.

## Fuera de alcance

Que la comanda de cocina se imprima sin conexión (eso es la tarea C3), y
sincronizar entre varias cajas del mismo local sin pasar por el servidor.

## Terminado cuando

Acordamos el diseño y la primera entrega. (El criterio de cada entrega lo
definimos ahí.)

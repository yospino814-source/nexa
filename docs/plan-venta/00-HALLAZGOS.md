# Qué encontramos afuera y qué ya tenemos adentro

Investigación de noviembre de 2026 sobre el mercado colombiano de POS para
tiendas, licoreras y restaurantes, cruzada con el código de este repo.

**Cómo leer las cifras.** Todo dato de aquí sale de búsqueda web, y varias
páginas no se pudieron abrir directamente. Los precios de la competencia y las
comisiones hay que confirmarlos en la página del proveedor antes de usarlos en
una propuesta comercial o en la app. Lo que quedó sin confirmar está marcado
con «verificar».

## 1. Precios de la competencia (COP al mes)

| Producto | Entrada | Trampa principal |
|---|---|---|
| Alegra POS | 25.900 | El tope va por ingresos del negocio: al vender más de $10M al mes toca subir de plan. El 25% de descuento anual solo aplica el primer año |
| Siigo POS | 28.750 | El plan Esencial solo permite 240 documentos al año. La renovación se cobra a precio de lista |
| Treinta | 39.900 | La facturación electrónica solo viene en el plan Pro ($79.900) |
| Loggro POS Tienda | 54.990 | Tope de documentos al mes y una sola caja |
| Bold POS | ~80.250 (anual $963.000) | Esencial trae una sede y un usuario. El datáfono se compra aparte |
| Vendty | verificar (las cifras publicadas se contradicen) | — |
| Fudo (restaurante) | 62.900 | Mesas, pantalla de cocina, Rappi y facturación se cobran aparte. En la práctica un restaurante paga $200.000–260.000 |
| Loggro Restobar | 120.990 | El plan Básico trae 30 documentos. Limita mesas y zonas de comanda |
| Toteat | 139.900 | Además cobra entre 0,5% y 0,7% de la venta neta |

**Lectura.** La entrada del mercado de tiendas está entre $25.000 y $40.000.
El patrón que más molesta a los clientes es el precio anunciado que no es el
precio real: topes de documentos, módulos aparte y descuentos de primer año.

## 2. De qué se quejan los usuarios

Las quejas se repiten en Google Play, Capterra, App Store y Reddit:

- **Siigo:** interfaz anticuada, demasiados clics para facturar, soporte que no
  responde durante semanas mientras cobranza sí llama.
- **Alegra:** se pone lento con inventarios grandes y se encarece cuando el
  negocio crece.
- **Treinta:** 4,3 estrellas con ~105.000 reseñas; errores tras las
  actualizaciones, precios promocionales que luego se triplican y **no permite
  vender por peso**.
- **Loggro Restobar:** 3,4–3,7 estrellas; desconexiones, comandas que no se
  imprimen y pedidos que desaparecen.
- **Transversal:** depender de internet. Es la queja más repetida.

No encontramos encuestas independientes de restauranteros colombianos; esa
parte viene de blogs de competidores y hay que tomarla con pinzas.

## 3. El tendero colombiano

- Hay cerca de **450.000 tiendas de barrio**, que mueven alrededor del 48% de
  las ventas de consumo masivo.
- Según una encuesta de Fenaltiendas de mayo de 2026 (muestra chica, 220
  tiendas), **el 85,45% no tiene sistema POS** y el 42,55% cree que no lo
  necesita. La mayoría está en «modo supervivencia».
- **Obligación DIAN:** el calendario del documento equivalente POS electrónico
  ya venció (el último grupo entró el 1 de julio de 2024). Lo regula la
  Resolución 165 de 2023, hoy compilada en la Resolución Única 000227 del 23 de
  septiembre de 2025.
- **El documento electrónico no tiene tope de valor.** El tope de 5 UVT
  ($261.870 con la UVT de 2026) aplica al tiquete POS en papel.
- **Quién no está obligado:** los no responsables de IVA (persona natural, menos
  de 3.500 UVT de ingresos, un solo local). Los del Régimen Simple sí están
  obligados.

**Consecuencia comercial.** Buena parte de las tiendas chicas no está obligada a
facturar, pero las licoreras, los minimercados y los restaurantes del Régimen
Simple sí. Eso permite un plan gratis sin facturación como puerta de entrada, y
cobrar cuando el negocio queda obligado.

## 4. Canales de venta

- **El contador es el canal número uno.** Alegra paga 10–20% de comisión
  recurrente a su red de socios y Siigo premia a los contadores que refieren.
  Así crecieron los dos.
- Las marcas grandes ya tienen su propia app de pedidos para el tendero (BEES de
  Bavaria, Juntos+ de Coca-Cola FEMSA, Pedidos Postobón, Surtiapp de Altipal).
  Son posibles socios de integración, pero no encontramos ningún caso de que
  distribuyan un POS de terceros.
- **Nadie ofrece hardware en comodato** para tiendas; solo pasa con datáfonos.
  Ahí hay un diferencial posible.
- Puntored tiene unos 20.000 puntos de corresponsalía y API pública: le genera
  comisión al tendero.

## 5. Pagos e integraciones

- **Bold tiene API para todo lo que nos falta:** link de pago
  (`POST /online/link/v1`), webhooks con `SALE_APPROVED`, y —lo importante— una
  **API para mandarle el monto al datáfono** (`POST /payments/app-checkout`,
  requiere certificación). Alegra y Loggro ya la usan. Hoy Nexa registra el pago
  con datáfono a mano.
- **Bre-B** (pagos inmediatos del Banco de la República) opera desde octubre de
  2025 y ya pasó los 1.700 millones de transacciones; el 62% son de menos de
  $50.000. El Banco de la República no expone API a comercios: hay que entrar por
  un PSP. Kushki y Mono ofrecen QR dinámico con webhook, y Wompi cobra 1% por QR.
  El QR de Nequi Negocios cobra 0%, pero sin confirmación automática.
- **Facturación electrónica:** MATIAS tiene modelo de casa de software
  multi-NIT, con bolsa compartida de $13 a $44 por documento según volumen, más
  el certificado digital (~$100.000 por NIT al año). Plan B si falla:
  Dataico (tiene claves «ally» para actuar por cliente) o The Factory HKA.
- **WhatsApp, cambio que nos pega:** desde el **1 de octubre de 2026** los
  mensajes de utilidad dentro de la ventana de 24 horas **ya no son gratis**.
  Quedan 1.000 respuestas de servicio gratis al mes por número; después se
  cobran. En Colombia una plantilla de utilidad cuesta unos USD 0,0008 (~$3) y
  una de marketing USD 0,0125, unas quince veces más. Si los recordatorios se
  redactan con tono promocional, Meta los reclasifica como marketing.

## 6. Restaurantes: lo que piden y lo que ya tenemos

**Normativa que hay que cumplir.** La propina es voluntaria, no puede sugerirse
por encima del 10%, hay que preguntarle al cliente antes de emitir el
documento, va en línea aparte, **no entra en la base del INC** y no es ingreso
del negocio. El expendio de comidas lleva INC del 8% en vez de IVA (las
franquicias cobran 19%). En el XML del documento POS el INC va con el código de
tributo 22 (verificar contra el catálogo de MATIAS, que hoy usamos con
`tax_id` 4).

**La buena noticia: media base de restaurante ya está construida.** Revisando el
código encontramos que ya existen:

| Pieza | Dónde |
|---|---|
| Tabla de mesas con código, QR y estado | `restaurant_tables`, migración `20260924000300_core_sales.sql` |
| Orden con mesa, mesero y propina | `orders.table_id`, `orders.waiter_id`, `orders.tip_amount`. Ojo: `orders.inc_amount` guarda **todo** el impuesto de la orden, no solo el INC |
| Tipo de impuesto INC 8% | `tbl_tipos_maestra`, grupo `IMP`, código `INC8` |
| INC enviado a la DIAN | `fiscal/matias.rs`: `tax_of` manda `tax_id` 4 cuando la tarifa es 8%, sin mirar el código del impuesto |
| Propina en el cobro | `components/pos/checkout-dialog.tsx`: sin propina, sugerida o personalizada (solo enteros), con mesero, si `tipsEnabled` está activo |
| **Modificadores completos** | `options.rs`: grupos con mínimo y máximo, opciones con sobreprecio e insumo que se descuenta del stock al vender. Ya tienen pantalla (`/inventario/modificadores`) y selector en el POS (`components/pos/customize-dialog.tsx`). No admiten sobreprecio negativo |
| Recetas | tablas `recipes` y `recipe_items` y `catalog::recipe`, pero **solo se usan para mostrar insumos y calcular disponibilidad: la venta no descuenta los insumos de la receta** (`sales.rs`) |
| Variantes de producto | `products.has_variants`, `variant_attrs` |
| Roles de mesero, cocina y repartidor | definidos, pero **sin pantallas** (`session-gate.tsx` los frena). Hoy invitar a alguien como `MESERO` se rechaza con `ROLE_NOT_AVAILABLE` |

**Lo que falta de verdad:** menú del día o combo, que la venta descuente los
insumos de la receta, cuentas abiertas por mesa con precuenta, comanda impresa
por estación, vista de mesero en el celular y pantalla de cocina.

**Domicilios.** Rappi tiene API oficial, pero las credenciales las entrega un
gerente de cuenta y hay certificación. DiDi Food tiene portal de
desarrolladores. iFood se fue de Colombia en 2022. Los agregadores (Deliverect,
Otter, Ordatic) cotizan por local. Las comisiones de las apps rondan el 25%, lo
que refuerza el valor del bot de WhatsApp propio.

**Impresión desde una app web.** Un navegador no puede abrir el puerto 9100 de
una impresora de red. Las salidas reales son: impresoras con servidor HTTP
propio (Epson ePOS-Print, Star WebPRNT), un agente local tipo QZ Tray, WebUSB
por cable y solo en Chrome, o un celular Android que haga de puente. Hoy Nexa
imprime con `window.print()`, que sirve para el tiquete pero no para mandar una
comanda a la cocina.

## 7. Funcionamiento sin internet

No hay nada de esto en el repo: no hay service worker, ni manifest, ni
IndexedDB. El patrón estándar es PWA con IndexedDB como fuente de verdad local,
una cola de ventas pendientes, un UUID generado en el cliente para que el
servidor descarte duplicados, y reenvío al volver la conexión. **Nexa ya tiene
esa pieza:** ventas, devoluciones, gastos, compras, abonos y pagos llevan un
`id` UUID que genera el navegador; si llega repetido con el mismo cuerpo, el
backend devuelve lo guardado, y si cambia el cuerpo responde
`IDEMPOTENCY_CONFLICT`. La excepción son los ajustes de inventario: el UUID se
crea dentro de `lib/api/index.ts` y cada reintento genera uno nuevo.

En 2025, el 73,9% de los hogares colombianos tenía internet, pero solo el 53,8%
tenía internet fijo. No encontramos cifras de caídas de conexión en negocios.

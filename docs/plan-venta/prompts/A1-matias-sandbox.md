# A1. Probar la facturación electrónica contra el sandbox real de MATIAS

Leé `CLAUDE.md` y `docs/plan-venta/00-HALLAZGOS.md` (sección 5).

## Contexto

`backend/src/fiscal/` arma el documento equivalente POS electrónico (tipo 20) y
la nota crédito (94), y los manda a MATIAS con un worker que reintenta. **Nunca
se probó contra el sandbox real**, solo contra un servidor falso en los tests.
Para licoreras y minimercados la factura electrónica es obligatoria, así que esto
bloquea la venta.

Ya pegué `MATIAS_API_URL` (sandbox) y `MATIAS_API_TOKEN` en `backend/.env`. No
leas ese archivo; el código ya lee las variables.

## Lo que quiero

1. Leé la documentación pública de MATIAS (`docs.matias-api.com`): campos de
   `/pos`, catálogos de `tax_id`, `type_document_id`, medios de pago, y el
   formato de respuesta y de errores. Compará campo por campo con
   `fiscal/matias.rs` (`pos_json`, `classify` y la nota crédito) y hacé una
   lista de diferencias. Confirmá en especial qué `tax_id` corresponde al INC
   del 8%: hoy `tax_of` manda 4 cuando la tarifa es 8%, sin mirar el código
   `INC8` del producto, y otras fuentes hablan del código de tributo 22.
2. Proponé cómo hacer una prueba de punta a punta sin tocar la base de
   producción: por ejemplo, un comando o un test `#[ignore]` que, con el token de
   sandbox, emita un documento POS con dos líneas (una con IVA 19% y una con
   INC 8%) y una nota crédito parcial, e imprima el CUDE y el estado.
3. **Esperá mi visto bueno** antes de cambiar código.
4. Después: corregí las diferencias, corré la prueba contra el sandbox y dejá en
   `backend/README.md` cómo repetirla.

## Fuera de alcance

ICL (está pausado), pasar a producción, cambiar de proveedor.

## Terminado cuando

El sandbox acepta un POS y una nota crédito emitidos por Nexa, las pruebas
existentes siguen pasando (`./scripts/test.sh`) y quedó documentado cómo
repetirlo.

# M04 Clientes: correcciones de F3.1

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-3-clientes` en los dos |
| Migración | Ninguna nueva |
| Commits | Backend: `feat(fase3): F3.1 correcciones`. Frontend: `feat(fase3): F3.1 correcciones` |
| Reporte | Sección "M04" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M04 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M04-fase3-clientes.md.
2. Trabajá en nexa-backend y nexa-frontend, en la rama feat/fase-3-clientes de cada uno.
3. Hacé las correcciones de "Backend" y "Frontend" en el orden en que aparecen. Cada una dice el archivo, qué hay hoy y qué tiene que quedar.
4. Corré la verificación de los dos repositorios. Debe pasar entera.
5. Un commit en cada repositorio, con el mensaje exacto del módulo.
6. Escribí la sección "M04" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
7. Detenete. No empieces M05. Escribime "M04 terminado".
```

## Backend (`src/customers.rs` y `src/procurement.rs`)

### B1. Posición de crédito de todo el negocio (A2 y A6)

Hoy `customers.rs` suma `receivables` directo (líneas 530 a 539, 634 y 1051 de la versión `83f6550`). RLS solo le deja ver al cajero la cartera de su sucursal, y los días de mora usan `current_date` (UTC).

Usa `public.customer_credit_position(c.id)` (creada en M02) en estos cuatro lugares, y borra las sumas propias:
1. `list`: `left join lateral public.customer_credit_position(c.id) cp on true`. Cada fila trae `balance = cp.balance` y `max_days_overdue = cp.max_days_overdue`. Filtros: `AL_DIA` es `cp.max_days_overdue = 0`; `EN_MORA` es `cp.max_days_overdue > 0`; `ARCHIVADO` es `c.archived_at is not null`. Sin filtro de estado, solo clientes no archivados.
2. `get_customer_detail_tx`: `balance` y `max_days_overdue` de la función. `credit_available = max(0, credit_limit − balance)`. `wallet_balance = 0` (F3.6 pospuesta).
3. `archive`: el saldo de cartera sale de la función.
4. No debe quedar ningún `current_date` en `customers.rs`.

### B2. Bloqueo al archivar (A7)

Al inicio de `archive`, antes de leer saldos: `select id from customers where tenant_id = $1 and id = $2 for update`.

Si tiene saldo en cartera, separados `ACTIVO` o saldo a favor: 409 `CUSTOMER_HAS_BALANCE` (como hoy).

### B3. Crear con cupo exige `credit:override` (A1)

En `create` y en la rama que crea de `upsert`: si `credit_limit` es distinto de 0, o `credit_block_days` es distinto de 30, y el usuario no tiene `credit:override`: 403. Hoy un cajero puede crear un cliente con cualquier cupo.

### B4. `PUT` conserva lo que no llega (A5)

En `update`, `credit_limit`, `credit_block_days` y `person_type` son `Option`. Si no llegan, se conserva el valor guardado. Hoy un `PUT` sin `credit_limit` lo deja en 0.

La regla de 403 sin `credit:override` aplica solo si llegan **y** cambian.

### B5. Tipo de persona y NIT (C7 y B11)

`CreateCustomerInput` agrega `person_type: Option<String>`. La normalización (`normalize_customer`) queda en este orden:
1. `doc_type`: mayúsculas y sin espacios a los lados.
2. **Solo si `doc_type = NIT`** y `doc_number` contiene `-`: lo que va después del último guion es el DV, y lo de antes es el número. Si además llegó `dv` y es distinto: 422 `CUSTOMER_DV_CONFLICT`. Con otro tipo, el guion no se trata aparte: se elimina en el paso 3.
3. `doc_number`: solo letras y dígitos, en mayúsculas.
4. Si `doc_type = NIT`:
   - el DV se calcula con `nit_dv`. Si llegó uno distinto: 422 `NIT_DV_INVALID`. Si no llegó, se guarda el calculado;
   - si llega `person_type`, debe ser `NATURAL` o `JURIDICA`; si no: 422 `CUSTOMER_INVALID`;
   - si no llega, `customers::default_person_type(doc_number)`, una función pura nueva: `JURIDICA` si el número tiene 9 dígitos y empieza por 8 o 9; si no, `NATURAL`.
5. Con otro tipo, o sin documento: `dv = null` y `person_type = NATURAL`. Si llega `JURIDICA`: 422 `CUSTOMER_INVALID`.

**Proveedores** (`procurement.rs`): al crear un proveedor, si `person_type` no llega, usa `customers::default_person_type` con NIT y `NATURAL` con otro documento, en vez de `JURIDICA` fijo.

### B6. `upsert` copia el documento como bloque (A3, A4 y C8)

`doc_type`, `doc_number`, `dv` y `person_type` son un solo bloque.
- Si el cliente encontrado **no** tiene documento y el cuerpo sí, busca si otro cliente activo ya usa ese documento. Si existe: 409 `CUSTOMER_DUPLICATED`, con `data: {id}`. Si no: copia los cuatro, con el `person_type` calculado en B5.
- Si el cliente encontrado ya tiene documento, esos cuatro campos no se tocan, aunque el cuerpo traiga otros.
- El resto de campos sigue la regla de hoy: solo se llenan los que están vacíos.

### B7. `unarchive` con documento repetido (A8)

Si otro cliente activo tiene el mismo `doc_type` y `doc_number`: 409 `CUSTOMER_DUPLICATED`, con `data: {id}`. Hoy da un 409 genérico.

### B8. Longitudes en caracteres (A14)

Todas las validaciones de longitud de `customers.rs` usan `chars().count()`, no `len()`. Hoy rechazan antes de tiempo los textos con tildes.

### B9. README (A12)

Documenta en `nexa-backend/README.md` los 8 endpoints de clientes, con método, ruta, permiso, cuerpo y errores:
- `GET /v1/customers`;
- `GET /v1/customers/{id}`;
- `POST /v1/customers`;
- `PUT /v1/customers/{id}`;
- `POST /v1/customers/upsert`;
- `POST /v1/customers/{id}/archive`;
- `POST /v1/customers/{id}/unarchive`;
- `GET /v1/customers/{id}/purchases`.

## Frontend (`nexa-frontend`)

| # | Archivo | Qué hay hoy | Qué tiene que quedar |
|---|---|---|---|
| F1 | `customer-form-dialog.tsx` (A1) | "Cupo" y "Días de bloqueo" se pueden editar al crear sin permiso | Deshabilitados al crear y al editar si `!can("credit:override")`. Texto debajo: "Solo un administrador puede cambiar el cupo." |
| F2 | `customer-form-dialog.tsx` (A9) | `parseInt(...) \|\| 30` convierte 0 en 30 | `Number.isNaN(n) ? 30 : n`; el 0 se conserva |
| F3 | `customer-form-dialog.tsx` (C7) | No hay tipo de persona | Selector "Tipo de persona" (Natural, Jurídica), visible solo con NIT. Mientras el usuario no lo cambie, se llena con la regla de `default_person_type`. La etiqueta del nombre es "Razón social" con Jurídica y "Nombre completo" con Natural |
| F4 | `customer-dialog.tsx` (POS) (C7) | — | **No** muestra el selector y **no** envía `person_type` |
| F5 | `customer-dialog.tsx:134` (A13) | Con `UNCHANGED` dice "Cliente actualizado con los datos nuevos" | Con `UNCHANGED`, selecciona al cliente sin mensaje. Con `UPDATED`: "Cliente actualizado con los datos nuevos". Con `CREATED`: "Cliente creado" |
| F6 | `customers-screen.tsx` (A11) | Pide 50 clientes, sin paginación | Paginación con `page` y `page_size = 50`: botones "Anterior" y "Siguiente", y el texto "Página <n>" |
| F7 | `customer-screen.tsx` (A15) | Las pestañas Cartera, Separados y Saldo a favor muestran "Módulo (F3.x)" | **Oculta** Separados y Saldo a favor. Cartera se oculta hasta M06, que la construye |
| F8 | `customer-form-dialog.tsx:427`, `customer-screen.tsx:331` y `:358` (A16) | El usuario ve el código `credit:override` y hay textos en tuteo ("Haz clic", "Escribe") | Sin códigos de permiso visibles. Voseo: "Hacé clic", "Escribí" |
| F9 | `lib/api/index.ts` y `lib/api/mock-customers.ts` (A10) | Los mocks de clientes están en `index.ts`. `upsert` nunca da `UNCHANGED` ni llena campos, `missing_fiscal` siempre sale vacío, las compras salen vacías y `archive` no valida saldo | Mueve los mocks a `lib/api/mock-customers.ts`. El mock aplica: B3 (403 con cupo sin permiso, simulado con el rol del demo), B5, B6, `missing_fiscal` en el orden documento, correo, municipio, dirección de facturación, 3 compras de ejemplo por cliente, y `CUSTOMER_HAS_BALANCE` si el cliente del mock tiene saldo |
| F10 | `supplier-dialog.tsx` (C7) | Proveedor con NIT queda Jurídica fijo | Al crear, la misma regla de `default_person_type` |

Agrega en `lib/customers.ts` la función pura `defaultPersonType(docType, docNumber)`, con la misma regla de B5. La usan F3, F9 y F10.

## Pruebas (backend)

**Unitaria** `normalize_customer_person_type`, en `customers.rs`:
- NIT `900123456` sin `person_type` → `JURIDICA`, DV `8`;
- NIT `1020304050` sin `person_type` → `NATURAL`, DV `8`;
- NIT `52123456` sin `person_type` → `NATURAL`, DV `2`;
- NIT `900123456` con `NATURAL` → `NATURAL`;
- `CC` con `JURIDICA` → 422 `CUSTOMER_INVALID`;
- `PP` con `AB-123456` → número `AB123456`, sin DV.

La prueba `normalize_customer_splits_dv` sigue pasando.

**En `f3_customers`.** Las 4 que ya existen siguen pasando. Se agregan:
- **[P]** Un cajero que crea un cliente con `credit_limit = 100000`: 403.
- **[P]** Un `PUT` del dueño sin `credit_limit` conserva el cupo guardado.
- **[P]** Con una deuda de 100.000 en la sucursal A, un cajero atado a la sucursal B ve `balance = 100000` en el detalle del cliente, y archivarlo da 409 `CUSTOMER_HAS_BALANCE`.
- **[P]** `upsert` que encuentra por correo a un cliente sin documento y recibe el NIT `900123456`: queda con `doc_type = NIT`, DV `8` y `person_type = JURIDICA`.
- **[P]** `upsert` que trae el documento de **otro** cliente activo: 409 `CUSTOMER_DUPLICATED`, con `data.id` de ese cliente.
- **[P]** Desarchivar a un cliente cuyo documento ya usa otro cliente activo: 409 `CUSTOMER_DUPLICATED`.
- **[P]** Filtros de `GET /v1/customers`: un cliente con mora sale en `EN_MORA` y no en `AL_DIA`; uno archivado solo sale en `ARCHIVADO` (A17).
- **[P]** `upsert` que busca por documento (sin correo) encuentra al cliente existente (A17).
- **[P]** `GET /v1/customers/{id}/purchases` devuelve las ventas del cliente con el formato de `/v1/sales` (A17).

## Fuera de alcance

- Venta a crédito (M03 y M05), abonos (M06) y cartera (M07).
- Consulta del NIT en RUES.
- Pestañas Separados y Saldo a favor.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "current_date" src/customers.rs        # debe salir vacía
grep -n "\.len()" src/customers.rs             # explica en el reporte cada línea que quede
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
grep -rn "credit:override\|Haz clic\|Escribe " components/clientes   # solo puede salir dentro de can("credit:override")
```

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M04`:
1. Hashes de los dos commits y la lista de archivos.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep`.
4. Una tabla de las reglas [P] y sus pruebas.
5. Lo que no se pudo hacer, con el archivo y la línea.

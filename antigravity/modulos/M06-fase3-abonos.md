# M06 Abonos y cuotas (F3.3)

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-3-clientes` en los dos |
| Migración | Ninguna nueva: `receivable_payments` ya está en `20261106000100_fase3_clientes.sql` |
| Commits | `feat(fase3): F3.3 abonos y cuotas`, en cada repositorio |
| Reporte | Sección "M06" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M06 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M06-fase3-abonos.md.
2. Trabajá en nexa-backend y nexa-frontend, en la rama feat/fase-3-clientes de cada uno.
3. Orden: backend con sus pruebas → API y mock del frontend → pantallas.
4. Las validaciones van en el orden exacto en que están escritas, con los códigos de error que se indican.
5. Corré la verificación de los dos repositorios. Debe pasar entera.
6. Un commit por repositorio, con el mensaje exacto del módulo.
7. Escribí la sección "M06" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
8. Detenete. No empieces M07. Escribime "M06 terminado".
```

## Backend (`src/receivables.rs`, nuevo)

### `POST /v1/receivables/{id}/payments`

Permiso `credit:collect`. Cuerpo: `{id: uuid, amount: Decimal, method: "EFECTIVO"|"DATAFONO"|"QR"|"TRANSFERENCIA"|"SALDO", reference?: string}`. El `id` del abono lo genera el cliente.

1. Abre **una** transacción con `tenancy::begin(pool, claims)` y lee `branch_id` de la cuenta (`select branch_id from receivables where id = $1 and tenant_id = $2`). Si no existe o RLS no la deja ver: 404. Después, `tenancy::ensure_branch_tx(&mut tx, &scope, branch_id)`.
2. Permiso `credit:collect`.
3. **Idempotencia** por `id`. Si ya existe un `receivable_payments` con ese `id`: con el mismo `client_request` (el cuerpo recibido, en JSON), devuelve ese abono; con otro, 409 `IDEMPOTENCY_CONFLICT`.
4. `method = "SALDO"`: 422 `PAYMENT_METHOD_NOT_AVAILABLE`. El saldo a favor (F3.6) está pospuesto.
5. Bloquea la cuenta: `select ... from receivables where id = $1 and tenant_id = $2 for update`. Si el estado no es `ABIERTA` ni `PARCIAL`: 409 `RECEIVABLE_CLOSED`.
6. `amount = money(amount)`. Si es `<= 0`, o si es mayor que el saldo (`amount_total − amount_paid`): 422 `PAYMENT_EXCEEDS_BALANCE`, con `data: {balance}`.
7. `DATAFONO`, `QR` y `TRANSFERENCIA` exigen `reference`, recortada, de 3 a 60 caracteres. Si no: 422 `PAYMENT_REFERENCE_REQUIRED`.
8. **Aplicación a cuotas:** recorre las cuotas por `n` ascendente, con `for update`. A cada una le suma `min(restante, amount − amount_paid)`, hasta agotar el abono. Guarda la aplicación en `allocation` como `[{n, amount}]`. Si la cuenta no tiene cuotas, `allocation = []`.
9. `amount_paid += amount`. Estado: `PAGADA` si el saldo queda en 0; si no, `PARCIAL`.
10. `due_date` pasa a ser el vencimiento de la primera cuota que queda con saldo (`fetch_one` en `Option<NaiveDate>`; solo se actualiza si es `Some`). Sin cuotas, no cambia.
11. **Efectivo:**
    - exige un turno abierto del usuario en la sucursal (`cash::open_session_tx`). Si no hay: 409 `PAYMENT_NEEDS_OPEN_SESSION`;
    - llama a `cash::record_movement_tx` con el tipo `ABONO_CARTERA`, el monto, el motivo `Abono <nombre del cliente>`, `ref_type = "RECEIVABLE"` y `ref_id` = id de la cuenta;
    - guarda el turno en `cash_session_id`.
12. Inserta el `receivable_payments` con `client_request` = el cuerpo recibido.
13. Auditoría `ABONO_CARTERA`, con la entidad `receivables`, el id de la cuenta y `after = {payment_id, amount, method}`.
14. Respuesta 201: el abono, con `{id, receivable_id, amount, method, reference, allocation, created_at, balance_before, balance_after, next_due: {n, due_date, amount_pending} | null}`.

### `GET /v1/receivables?customer_id=&status=&overdue=`

Permiso `customers:read`.
- `status` es opcional: `ABIERTA`, `PARCIAL` o `PAGADA`. Sin él, devuelve `ABIERTA` y `PARCIAL`.
- `overdue=true` devuelve solo las que tienen `days_overdue > 0`.
- Cada fila: `{id, branch_id, branch_name, customer_id, customer_name, source, order_id, receipt_number, principal, amount_total, amount_paid, balance, status, due_date, days_overdue, created_at}`.
- Orden: `days_overdue` descendente y luego `created_at`.
- `days_overdue` se calcula con la regla de la cuenta: con cuotas, desde la primera cuota con `amount_paid < amount`; sin cuotas, desde `due_date`. "Hoy" es la fecha local de la sucursal de la cuenta. Mínimo 0, y 0 si el saldo es 0. **No uses `current_date`.**

### `GET /v1/receivables/{id}`

Permiso `customers:read`. La fila de arriba, más:
- `installments: [{n, due_date, amount, amount_paid, paid: bool, days_overdue}]`;
- `payments: [{id, amount, method, reference, allocation, created_at, created_by_name}]`.

Registra las 3 rutas en `routes.rs` y documéntalas en `README.md`.

## Frontend

### API y mock

- `api.receivables.list(filter)`, `api.receivables.get(id)` y `api.receivables.pay(id, body)`.
- Tipos `Receivable`, `ReceivableDetail`, `ReceivableInstallment`, `ReceivablePayment` y `PayReceivableInput`.
- Mock en `lib/api/mock-receivables.ts`. Las cuentas salen de las ventas a crédito del mock de M05. Aplica las mismas validaciones 4, 6 y 7 y la aplicación a cuotas.

### Pestaña "Cartera" de la ficha del cliente (`components/clientes/customer-screen.tsx`)

M04 la dejó oculta. Ahora se muestra si `can("customers:read")`:
- **Encabezado:** tarjetas "Saldo" (`balance`), "Cupo disponible" (`credit_available`) y "Días de mora" (`max_days_overdue`, en rojo si es mayor que 0).
- **Una tarjeta por cuenta** `ABIERTA` o `PARCIAL`: número de la venta, fecha, total, pagado, saldo, vencimiento y estado. Debajo, la tabla de cuotas ("Cuota", "Vence", "Valor", "Pagado" y "Estado"). El estado es "Pagada", "Vencida" (en rojo, con los días) o "Pendiente".
- **Botón "Abonar"** en cada cuenta, solo con `can("credit:collect")`. Abre un diálogo con:
  - "Monto" (`MoneyInput`), que vale el saldo de la cuenta al abrir;
  - "Método": Efectivo, Datáfono, QR o Transferencia (sin Saldo a favor);
  - "Referencia", visible y obligatoria salvo con Efectivo;
  - "Registrar abono", que llama a `api.receivables.pay` con `id: crypto.randomUUID()`, generado **una vez al abrir el diálogo**. Así, un doble clic no abona dos veces.
- Si funciona: cierra el diálogo, recarga la cuenta y abre la tirilla de abono.
- Con `PAYMENT_NEEDS_OPEN_SESSION`: el mensaje de `friendlyError` y un enlace "Abrir caja" a `/caja`.

### Tirilla de abono (`components/clientes/payment-ticket.tsx`, nuevo)

Usa `PrintArea`, con el mismo ancho que `print-ticket.tsx` (`print.ticket_width_mm`). Contenido, en este orden:
1. Encabezado del negocio, igual que la tirilla de venta.
2. Título **"ABONO"**.
3. Fecha y hora.
4. Cliente (nombre y documento).
5. "Venta <número>".
6. "Monto abonado", "Método" (y referencia, si hay).
7. "Saldo anterior" y "Saldo nuevo".
8. "Próxima cuota: <n> · <dd/mm/aaaa> · <monto>", o "Cuenta pagada" si el saldo nuevo es 0.
9. "Atendido por <nombre del usuario>".

Botón "Imprimir" (`window.print`) y botón "Cerrar".

### Errores (`ERROR_MESSAGES`)

| Código | Texto |
|---|---|
| `RECEIVABLE_CLOSED` | La cuenta ya está cerrada. |
| `PAYMENT_EXCEEDS_BALANCE` | El abono supera el saldo de la cuenta. |
| `PAYMENT_NEEDS_OPEN_SESSION` | Para recibir un abono en efectivo, abrí la caja. |

`PAYMENT_METHOD_NOT_AVAILABLE` ya lo agregó M05, y `PAYMENT_REFERENCE_REQUIRED` ya existe.

## Pruebas (`f3_payments`, en `db_tests.rs`, llamada desde `database_integration` después de `f3_credit_sale`)

- **[P]** Un abono de 50.000 sobre cuotas de 33.333,33, 33.333,33 y 33.333,34 paga completa la cuota 1 y abona 16.666,67 a la cuota 2. `allocation = [{n:1, amount: 33333.33}, {n:2, amount: 16666.67}]`, y el `due_date` de la cuenta pasa al de la cuota 2.
- **[P]** Un abono mayor que el saldo: 422 `PAYMENT_EXCEEDS_BALANCE`.
- **[P]** Un abono en efectivo crea un movimiento `ABONO_CARTERA` y sube el efectivo esperado del turno en el monto del abono.
- **[P]** Un abono en efectivo sin turno abierto: 409 `PAYMENT_NEEDS_OPEN_SESSION`, y la cuenta no cambia.
- **[P]** Un reintento idéntico (mismo `id` y mismo cuerpo) no abona dos veces. Con el mismo `id` y otro monto: 409 `IDEMPOTENCY_CONFLICT`.
- **[P]** `method = "SALDO"`: 422 `PAYMENT_METHOD_NOT_AVAILABLE`.
- **[P]** `method = "DATAFONO"` sin referencia: 422 `PAYMENT_REFERENCE_REQUIRED`.
- **[P]** Abonar el saldo completo deja la cuenta `PAGADA`. Un abono más: 409 `RECEIVABLE_CLOSED`.

## Fuera de alcance

- Saldo a favor como método.
- Acuerdos de pago y castigo (F3.4).
- Abonar a varias cuentas en un solo pago.
- Anular un abono.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "current_date\|\.unwrap()" src/receivables.rs   # solo dentro de mod tests
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M06`:
1. Hashes de los dos commits y la lista de archivos.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida del `grep`.
4. La tabla de las reglas [P] y sus pruebas.
5. Lo que no se pudo hacer.

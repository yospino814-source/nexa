# M07 Cartera: resumen, antigüedad, estado de cuenta y cobro por WhatsApp (F3.5)

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-3-clientes` en los dos |
| Migración | Ninguna nueva |
| Commits | `feat(fase3): F3.5 cartera`, en cada repositorio |
| Reporte | Sección "M07" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M07 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M07-fase3-cartera.md.
2. Trabajá en nexa-backend y nexa-frontend, en la rama feat/fase-3-clientes de cada uno.
3. Orden: backend con sus pruebas → API y mock del frontend → pantallas.
4. Los textos (incluido el mensaje de WhatsApp) van tal cual, con voseo.
5. Corré la verificación de los dos repositorios. Debe pasar entera.
6. Un commit por repositorio, con el mensaje exacto del módulo.
7. Escribí la sección "M07" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
8. Detenete. No empieces M08. Escribime "M07 terminado".
```

## Reglas comunes

- `balance(cuenta)`: `amount_total − amount_paid` si el estado es `ABIERTA` o `PARCIAL`; si no, 0.
- `days_overdue(cuenta)`: la misma regla de M06. "Hoy" es la fecha local de la sucursal de la cuenta. **Nunca `current_date`.**
- Todos los endpoints usan permiso `customers:read` y RLS (cada usuario ve las sucursales de su membresía).

## Backend (`src/receivables.rs`)

### `GET /v1/receivables/summary?branch_id=`

`branch_id` es opcional. Sin él, `tenancy::begin`; con él, `begin_in_branch`. Respuesta:

```
{ total_balance,          // Σ balance de cuentas ABIERTA o PARCIAL
  overdue_balance,        // Σ balance de las que tienen days_overdue > 0
  due_this_week,          // Σ saldo de cuotas (o de cuentas sin cuotas) que vencen entre hoy y hoy + 7 días, ambos incluidos
  collected_this_month,   // Σ receivable_payments.amount con created_at en el mes local actual
  aging: { al_dia, d1_30, d31_60, d61_90, d91_120, d120_mas },
  top_debtors: [{customer_id, name, balance, max_days_overdue}] }   // 10, por balance descendente
```

- El tramo de cada cuenta se decide con su `days_overdue`: `al_dia` = 0; `d1_30` = 1 a 30; `d31_60` = 31 a 60; `d61_90` = 61 a 90; `d91_120` = 91 a 120; `d120_mas` = más de 120.
- Cada tramo suma el `balance` de sus cuentas.
- `top_debtors` agrupa por cliente.

### `GET /v1/customers/{id}/statement?from=&to=`

`from` y `to` son fechas (`YYYY-MM-DD`). Respuesta:

```
{ opening_balance,
  entries: [{date, type: "CARGO"|"ABONO"|"DEVOLUCION", reference, debit, credit, balance}],
  closing_balance }
```

- **CARGO**: una cuenta creada. `date` = fecha local de `created_at`. `debit` = valor **original** de la cuenta = `principal + interest_total + Σ receivable_applied` de sus devoluciones aprobadas. `reference` = número de la venta.
- **ABONO**: un `receivable_payments`. `credit` = `amount`. `reference` = "Abono " + método.
- **DEVOLUCION**: una devolución aprobada (`refunds.status = 'APROBADA'`) con `receivable_applied > 0`. `date` = fecha local de `decided_at`. `credit` = `receivable_applied`. `reference` = número de la devolución.
- `opening_balance` = Σ débitos − Σ créditos de los movimientos anteriores a `from`.
- `balance` de cada fila = saldo acumulado.
- Orden: fecha, luego hora, luego tipo (CARGO, ABONO, DEVOLUCION).
- `closing_balance` debe ser igual al saldo del cliente (`customer_credit_position`) cuando `to` es hoy.

### `GET /v1/receivables/{id}/whatsapp-link`

1. Teléfono del cliente: solo dígitos. Si tiene 12 dígitos y empieza por `57`, quita ese `57`.
2. Si el resultado no tiene 10 dígitos o no empieza por `3`: 422 `CUSTOMER_NO_PHONE`.
3. Respuesta `{url}`, con `https://wa.me/57<telefono>?text=<texto codificado>`.
4. **Texto exacto**, sin vencer: `Hola <nombre del cliente>, te escribimos de <tenants.name>. Tenés un saldo de $<saldo> que vence el <dd/mm/yyyy>. Podés pagar en <nombre de la sucursal>. ¡Gracias!`
5. Si `days_overdue > 0`, cambia "que vence el" por "vencido desde el".
6. `<saldo>` va con separador de miles con punto y sin decimales si son 00 (ejemplo: `150.000`). Si tiene centavos, coma decimal (`150.000,50`). La fecha es el `due_date` de la cuenta.

Registra las 3 rutas y documéntalas en `README.md`.

## Frontend

### API y mock

`api.receivables.summary(branchId?)`, `api.customers.statement(id, from, to)` y `api.receivables.whatsappLink(id)`, con su mock en `lib/api/mock-receivables.ts`.

### Pantalla `/cartera` (pantalla 44)

- Registro en `lib/screens.ts`: `s("44", "cartera", "Cartera", "/cartera", "clientes", "admin", "react")`.
- Archivos: `app/(admin)/cartera/page.tsx` y `components/cartera/receivables-screen.tsx`.
- Menú lateral (`sidebar.tsx`): ítem "Cartera" después de "Clientes", con `href="/cartera"`, ícono `Wallet` de `lucide-react` y `permission: "customers:read"`. En `isAllowedForRole`, agrega `"/cartera"` a las listas de `CAJERO` y `SUPERVISOR`. M13 reemplaza esa función más adelante.

Contenido:
1. **Cuatro `StatCard`**: "Por cobrar" (`total_balance`), "Vencido" (`overdue_balance`), "Vence esta semana" (`due_this_week`) y "Cobrado este mes" (`collected_this_month`).
2. **Gráfica de antigüedad**: 6 barras horizontales hechas con `div` y CSS. El ancho es proporcional al monto del tramo más grande. Cada barra muestra su etiqueta ("Al día", "1 a 30 días", "31 a 60 días", "61 a 90 días", "91 a 120 días" y "Más de 120 días") y su monto con `formatCOP`. **Sin librerías de gráficas.**
3. **Tabla de deudores** (`api.receivables.list()` agrupado por cliente): "Cliente", "Saldo", "Días de mora" (en rojo si es mayor que 0) y "Acciones":
   - "Abonar": el diálogo de M06, solo con `can("credit:collect")`. Si el cliente tiene varias cuentas, abona a la de mayor mora;
   - "Cobrar por WhatsApp": abre en una pestaña nueva la `url` de `whatsapp-link` de esa misma cuenta. Con `CUSTOMER_NO_PHONE`, muestra el mensaje y no abre nada;
   - "Ver": lleva a `/clientes/<id>`.
4. **"Top 10"** a un lado (en pantallas pequeñas, debajo): `top_debtors`, con nombre y saldo.
5. Selector de sucursal: "Todas" (sin `branch_id`) y una opción por sucursal de `useSession().branches`.

### Pestaña "Cartera" de la ficha del cliente

Agrega a lo que hizo M06:
- botón **"Cobrar por WhatsApp"** en cada cuenta, con el mismo comportamiento de la tabla;
- botón **"Estado de cuenta"**. Abre un diálogo con:
  - "Desde", que vale el primer día del mes anterior;
  - "Hasta", que vale hoy;
  - la tabla de movimientos ("Fecha", "Tipo", "Referencia", "Cargo", "Abono" y "Saldo");
  - "Saldo inicial" y "Saldo final";
  - el selector "Formato: Tirilla / Carta" y el botón "Imprimir", con `PrintArea` y `window.print`. Tirilla usa `print.ticket_width_mm`; carta usa el ancho completo.
- **No** hay botón "Acuerdo de pago" (F3.4 está pospuesta).

### Errores

`CUSTOMER_NO_PHONE`: "El cliente no tiene un celular válido para WhatsApp.". Reemplaza el texto anterior si ya existe.

## Pruebas (`f3_aging`, llamada después de `f3_payments`)

- **[P]** Los tramos de antigüedad suman `total_balance`, y una cuenta con 45 días de mora cae en `d31_60`.
- **[P]** Después de una devolución parcial de una venta a crédito, `closing_balance` del estado de cuenta es igual al saldo del cliente, y hay una fila `DEVOLUCION` con el `receivable_applied`.
- **[P]** `whatsapp-link` con el teléfono `573001234567` devuelve una `url` que empieza por `https://wa.me/573001234567?text=`. Con el teléfono `6012345678` (fijo): 422 `CUSTOMER_NO_PHONE`.
- **[P]** El texto de `whatsapp-link` de una cuenta vencida contiene "vencido desde el".

Prueba unitaria `whatsapp_phone_normalization`, con la función pura que normaliza el teléfono:
- `"300 123 4567"` → `3001234567`;
- `"+57 300 123 4567"` → `3001234567`;
- `"6012345678"` → error;
- `"12345"` → error.

## Fuera de alcance

- Envío automático de WhatsApp o correo (solo enlaces `wa.me`).
- Acuerdos de pago, castigo y refinanciación.
- Procesos nocturnos: los días de mora se calculan al leer.
- PDF generado en el servidor.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
grep -n "current_date" src/receivables.rs src/customers.rs   # vacía
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
grep -rn "recharts\|chart.js\|apexcharts\|nivo" package.json   # vacía
```

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M07`:
1. Hashes de los dos commits y la lista de archivos.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep`.
4. La tabla de las reglas [P] y sus pruebas.
5. Lo que no se pudo hacer.

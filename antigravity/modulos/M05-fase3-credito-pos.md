# M05 Venta a crédito: POS (F3.2)

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorio | Solo `nexa-frontend` |
| Rama | `feat/fase-3-clientes` |
| Commit | `feat(fase3): F3.2 cobro a credito en el POS` |
| Reporte | Sección "M05" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M05 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M05-fase3-credito-pos.md.
2. Trabajá solo en nexa-frontend, en la rama feat/fase-3-clientes. El backend de este módulo ya está hecho (M03): no lo toques.
3. Hacé exactamente lo que dice "Qué hacer". Los textos de pantalla van tal cual, con voseo.
4. Corré la verificación y probá el flujo en modo demo como dice "Prueba manual".
5. Commit con el mensaje exacto del módulo.
6. Escribí la sección "M05" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
7. Detenete. No empieces M06. Escribime "M05 terminado".
```

## Contexto: lo que ya hace el backend (M03)

- El pago `{method: "CREDITO", amount}` en `POST /v1/sales` exige un cliente y el permiso `credit:sell`.
- La venta puede llevar `credit_plan: {installments, frequency, first_due_date, interest_rate_monthly: "0", interest_type: "SIMPLE"}`. Sin plan, es "crédito libre" y vence a 30 días.
- Errores posibles: `CREDIT_REQUIRES_CUSTOMER`, `CREDIT_NOT_ENABLED`, `CREDIT_BLOCKED_OVERDUE` (`data.days`), `CREDIT_LIMIT_EXCEEDED` (`data.available`), `CREDIT_PLAN_INVALID`, `SALE_MULTIPLE_CREDIT` y `CREDIT_OVERRIDE_REASON_INVALID`.
- Para autorizar por encima del cupo se reenvía **la misma venta con el mismo `id`** y `credit_override_reason`. Exige `credit:override`.
- `GET /v1/credit/schedule-preview?principal=&installments=&frequency=&first_due_date=&interest_rate_monthly=0&interest_type=SIMPLE` devuelve `{interest_total, total, installments: [{n, due_date, amount}]}`.
- `GET /v1/customers/{id}` trae `credit_available` y `credit_limit`.

## Qué hacer

### 1. Tipos y API (`lib/api/types.ts`, `lib/api/index.ts`)

- `PaymentMethod` agrega `"CREDITO"`.
- `SaleRequest` agrega `credit_plan?: CreditPlan | null` y `credit_override_reason?: string | null`.
- Tipo `CreditPlan = { installments: number; frequency: "SEMANAL" | "QUINCENAL" | "MENSUAL"; first_due_date: string; interest_rate_monthly: Decimal; interest_type: "SIMPLE" | "CUOTA_FIJA" }`.
- Tipo `SchedulePreview = { interest_total: Decimal; total: Decimal; installments: { n: number; due_date: string; amount: Decimal }[] }`.
- `api.credit.schedulePreview(params): Promise<SchedulePreview>`, con su rama mock.

### 2. Mock (`lib/api/mock-credit.ts`, nuevo)

- `mockSchedulePreview`: la misma regla `SIMPLE` sin interés del backend. Cada cuota es `floor(total / n)` con 2 decimales, y la última absorbe la diferencia. Fechas: semanal +7 días, quincenal +15 días, mensual +1 mes, con el último día del mes si el día no existe.
- En el mock de `sales.create`, si hay un pago `CREDITO`, en este orden:
  1. sin cliente: `CREDIT_REQUIRES_CUSTOMER`;
  2. si no viene `credit_override_reason`:
     - cupo 0: `CREDIT_NOT_ENABLED`;
     - mora del cliente del mock mayor que `credit_block_days`: `CREDIT_BLOCKED_OVERDUE`;
     - monto mayor que `credit_available`: `CREDIT_LIMIT_EXCEEDED`, con `available`.
- Al vender a crédito en el mock, baja el `credit_available` del cliente.
- Datos del mock: al menos un cliente con cupo de 500.000 y saldo 0 ("Carlos Pérez"), uno con cupo de 200.000 y saldo de 180.000 ("Luz Marina Gómez"), y uno con 40 días de mora ("Jorge Iguarán").

### 3. Cobro en el POS (`components/pos/checkout-dialog.tsx` y `components/pos/payment-methods.ts`)

1. **Método "Crédito"** en la lista de métodos de cobro. Visible **solo** si:
   - la venta tiene un cliente asignado (no basta un alias);
   - y `useSession().can("credit:sell")`.
2. Al elegir "Crédito":
   - debajo del monto se lee el texto `Cupo disponible: <formatCOP(credit_available)>`, con el cliente cargado por `api.customers.get(customer_id)`;
   - si `credit_limit = 0`, el texto es "Este cliente no tiene cupo de crédito." y el monto de crédito queda deshabilitado.
3. **Pago mixto:** el monto de "Crédito" se edita igual que el de los otros métodos. Solo puede haber una línea de crédito; si ya hay una, el botón "Crédito" queda deshabilitado.
4. **Interruptor "En cuotas"**, apagado al abrir. Encendido muestra:
   - "Cuotas": número de 1 a 60, que vale 1 al abrir;
   - "Frecuencia": selector Semanal, Quincenal y Mensual, que vale Mensual al abrir;
   - "Primera cuota": fecha, que vale hoy + 30 días al abrir, con mínimo hoy;
   - **sin** campos de tasa ni de tipo de interés;
   - una tabla de vista previa ("Cuota", "Vence" y "Valor"), que se recalcula con `api.credit.schedulePreview` 400 ms después del último cambio.
   - Envía `credit_plan` con `interest_rate_monthly: "0"` e `interest_type: "SIMPLE"`.
   - Con el interruptor apagado, no envía `credit_plan`.
5. **Rechazo por cupo o mora.** Si la venta responde `CREDIT_LIMIT_EXCEEDED` o `CREDIT_BLOCKED_OVERDUE`:
   - muestra el mensaje de `friendlyError`. Con `CREDIT_LIMIT_EXCEEDED` agrega "Disponible: <formatCOP(data.available)>"; con `CREDIT_BLOCKED_OVERDUE`, "Días de mora: <data.days>";
   - si `can("credit:override")`, muestra el botón "Autorizar igual". Abre un campo "Motivo de la autorización" (3 a 200 caracteres) y el botón "Autorizar y cobrar", que reenvía **la misma venta con el mismo `id`** y `credit_override_reason`;
   - si no tiene el permiso, el texto "Pedile a un supervisor que autorice la venta." y ningún botón.
6. **Tirilla** (`print-ticket.tsx`): la línea del pago a crédito dice "Crédito". Con plan, agrega debajo "Cuotas: <n> <frecuencia en minúscula>, primera el <dd/mm/aaaa>". Sin plan: "Vence el <dd/mm/aaaa>" (hoy + 30 días).

### 4. Errores (`ERROR_MESSAGES`)

Agrega los que falten, con estos textos exactos:

| Código | Texto |
|---|---|
| `CREDIT_PLAN_INVALID` | Revisá el plan de cuotas. |
| `CREDIT_OVERRIDE_REASON_INVALID` | El motivo debe tener entre 3 y 200 caracteres. |
| `PAYMENT_METHOD_NOT_AVAILABLE` | Ese método no está disponible para abonos. |
| `SALE_MULTIPLE_CREDIT` | Solo puede haber un pago a crédito por venta. |
| `CREDIT_REQUIRES_CUSTOMER` | Para vender a crédito elegí un cliente. |
| `CREDIT_NOT_ENABLED` | Este cliente no tiene cupo de crédito. |
| `CREDIT_BLOCKED_OVERDUE` | El cliente tiene deudas vencidas: crédito bloqueado. |
| `CREDIT_LIMIT_EXCEEDED` | La venta supera el cupo disponible. |
| `REFUND_EXCEEDS_RECEIVABLE` | La devolución supera lo que queda por pagar del crédito. |

## Fuera de alcance

- Campos de tasa de interés o tipo de interés.
- Saldo a favor como método de pago.
- Cambiar el cupo desde el POS.

## Verificación

```sh
cd nexa-frontend && npx tsc --noEmit && npm run lint
```

## Prueba manual (modo demo, puerto 3200)

1. POS → agregar productos por 100.000 → asignar a "Carlos Pérez" → Cobrar → "Crédito" muestra "Cupo disponible: $500.000" → cobrar todo a crédito → sale la tirilla con "Crédito" y "Vence el …".
2. Lo mismo con "Luz Marina Gómez" por 100.000 → `CREDIT_LIMIT_EXCEEDED` con "Disponible: $20.000" → "Autorizar igual" con un motivo → la venta pasa.
3. Venta mixta: 20.000 en efectivo y 80.000 a crédito, "En cuotas" con 3 mensuales → la vista previa muestra 26.666,66, 26.666,66 y 26.666,68.
4. Sin cliente asignado, el método "Crédito" no aparece.

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M05`:
1. Hash del commit y la lista de archivos.
2. La salida de `tsc` y de `lint`.
3. El resultado de cada paso de la prueba manual. Escribe "no lo probé" si no lo hiciste.
4. Lo que no se pudo hacer.

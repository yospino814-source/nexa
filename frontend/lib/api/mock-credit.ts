import type { CreditFrequency, ScheduleInstallment, SchedulePreview, SchedulePreviewParams } from "./types";

/**
 * Fechas: la cuota k vence en first_due_date + (k - 1) * periodo.
 * Semanal: +7 dias * (k - 1)
 * Quincenal: +15 dias * (k - 1)
 * Mensual: +1 mes calendario * (k - 1), ajustado al ultimo dia del mes si no existe.
 */
export function calculateInstallmentDueDate(
  firstDueDate: string,
  k: number,
  frequency: CreditFrequency,
): string {
  if (k <= 1) return firstDueDate;
  const [yearStr, monthStr, dayStr] = firstDueDate.split("-");
  const year = parseInt(yearStr, 10);
  const month = parseInt(monthStr, 10); // 1-12
  const day = parseInt(dayStr, 10);
  const step = k - 1;

  if (frequency === "SEMANAL") {
    const d = new Date(Date.UTC(year, month - 1, day));
    d.setUTCDate(d.getUTCDate() + step * 7);
    return d.toISOString().slice(0, 10);
  }

  if (frequency === "QUINCENAL") {
    const d = new Date(Date.UTC(year, month - 1, day));
    d.setUTCDate(d.getUTCDate() + step * 15);
    return d.toISOString().slice(0, 10);
  }

  // MENSUAL
  const targetMonthIdx = month - 1 + step;
  const targetYear = year + Math.floor(targetMonthIdx / 12);
  const targetMonth = (targetMonthIdx % 12) + 1; // 1-12
  const daysInMonth = new Date(Date.UTC(targetYear, targetMonth, 0)).getUTCDate();
  const targetDay = Math.min(day, daysInMonth);
  return `${targetYear}-${String(targetMonth).padStart(2, "0")}-${String(targetDay).padStart(2, "0")}`;
}

export function mockSchedulePreview(params: SchedulePreviewParams): SchedulePreview {
  const n = Math.max(1, Math.min(60, Math.floor(params.installments || 1)));
  const principalNum = Math.max(0, Number(params.principal || 0));
  const principalCents = Math.round(principalNum * 100);

  // Regla SIMPLE sin interes (interes 0): cada cuota floor(total / n), ultima absorbe diferencia
  const eachCents = Math.floor(principalCents / n);
  let accumulatedCents = 0;
  const installments: ScheduleInstallment[] = [];

  for (let k = 1; k <= n; k++) {
    const dueDate = calculateInstallmentDueDate(params.first_due_date, k, params.frequency);
    let amountCents: number;
    if (k < n) {
      amountCents = eachCents;
      accumulatedCents += amountCents;
    } else {
      amountCents = principalCents - accumulatedCents;
    }
    installments.push({
      n: k,
      due_date: dueDate,
      amount: (amountCents / 100).toFixed(2),
    });
  }

  return {
    interest_total: "0.00",
    total: (principalCents / 100).toFixed(2),
    installments,
  };
}


//! Motor de crédito, cálculo de cuotas y fechas de vencimiento (F3.2).

use axum::http::StatusCode;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::money::{money, money_floor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency {
    Semanal,
    Quincenal,
    Mensual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterestType {
    Simple,
    CuotaFija,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditPlan {
    pub installments: i32,
    pub frequency: Frequency,
    pub first_due_date: NaiveDate,
    pub interest_rate_monthly: Decimal,
    pub interest_type: InterestType,
}

impl CreditPlan {
    pub fn validate(&self) -> Result<(), ApiError> {
        if !(1..=60).contains(&self.installments) {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CREDIT_PLAN_INVALID",
                "El número de cuotas debe estar entre 1 y 60",
                None,
            ));
        }
        if self.interest_rate_monthly != Decimal::ZERO {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CREDIT_PLAN_INVALID",
                "El crédito no admite interés",
                None,
            ));
        }
        Ok(())
    }

    pub fn validate_dates(&self, today: NaiveDate) -> Result<(), ApiError> {
        if self.first_due_date < today {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CREDIT_PLAN_INVALID",
                "La fecha del primer vencimiento no puede ser anterior a hoy",
                None,
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledInstallment {
    pub n: i32,
    pub due_date: NaiveDate,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulePreviewResponse {
    pub interest_total: Decimal,
    pub total: Decimal,
    pub installments: Vec<ScheduledInstallment>,
}

#[derive(Debug, Deserialize)]
pub struct SchedulePreviewQuery {
    pub principal: Decimal,
    pub installments: i32,
    pub frequency: Frequency,
    pub first_due_date: NaiveDate,
    pub interest_rate_monthly: Decimal,
    pub interest_type: InterestType,
}

/// Fechas: la cuota k vence en first_due_date + (k−1) × periodo.
/// El periodo es de 7 días, 15 días o 1 mes calendario: usa chrono y checked_add_months;
/// si el día no existe en ese mes, toma el último día del mes.
pub fn calculate_installment_due_date(
    first_due_date: NaiveDate,
    k: i32,
    frequency: Frequency,
) -> NaiveDate {
    if k <= 1 {
        return first_due_date;
    }
    let step = (k - 1) as i64;
    match frequency {
        Frequency::Semanal => first_due_date + chrono::Duration::days(step * 7),
        Frequency::Quincenal => first_due_date + chrono::Duration::days(step * 15),
        Frequency::Mensual => {
            let months_to_add = (k - 1) as u32;
            first_due_date
                .checked_add_months(chrono::Months::new(months_to_add))
                .unwrap_or(first_due_date)
        }
    }
}

/// Plan de cuotas: función pura credit::schedule(principal, plan) -> (interest_total, Vec<(n, due_date, amount)>)
pub fn schedule(principal: Decimal, plan: &CreditPlan) -> (Decimal, Vec<ScheduledInstallment>) {
    let n = plan.installments;
    let n_dec = Decimal::from(n);

    // Tasa por periodo: r = interest_rate_monthly / 100 × f
    // con f = 7/30 (SEMANAL), 15/30 (QUINCENAL) o 1 (MENSUAL).
    let f = match plan.frequency {
        Frequency::Semanal => Decimal::new(7, 0) / Decimal::new(30, 0),
        Frequency::Quincenal => Decimal::new(15, 0) / Decimal::new(30, 0),
        Frequency::Mensual => Decimal::ONE,
    };
    let r = (plan.interest_rate_monthly / Decimal::new(100, 0)) * f;

    match plan.interest_type {
        InterestType::Simple => {
            let interest_total = money(principal * r * n_dec);
            let total = principal + interest_total;
            let each = money_floor(total / n_dec);
            let mut sum_prev = Decimal::ZERO;
            let mut installments = Vec::with_capacity(n as usize);

            for k in 1..=n {
                let due_date =
                    calculate_installment_due_date(plan.first_due_date, k, plan.frequency);
                let amount = if k < n {
                    sum_prev += each;
                    each
                } else {
                    total - sum_prev
                };
                installments.push(ScheduledInstallment {
                    n: k,
                    due_date,
                    amount,
                });
            }
            (interest_total, installments)
        }
        InterestType::CuotaFija => {
            if r == Decimal::ZERO {
                // Si r = 0, se calcula igual que SIMPLE con interés 0
                let total = principal;
                let each = money_floor(total / n_dec);
                let mut sum_prev = Decimal::ZERO;
                let mut installments = Vec::with_capacity(n as usize);

                for k in 1..=n {
                    let due_date =
                        calculate_installment_due_date(plan.first_due_date, k, plan.frequency);
                    let amount = if k < n {
                        sum_prev += each;
                        each
                    } else {
                        total - sum_prev
                    };
                    installments.push(ScheduledInstallment {
                        n: k,
                        due_date,
                        amount,
                    });
                }
                (Decimal::ZERO, installments)
            } else {
                // A = principal × r / (1 − (1 + r)^−n)
                let one_plus_r = Decimal::ONE + r;
                let pow = one_plus_r.powi(n as i64);
                let a = (principal * r * pow) / (pow - Decimal::ONE);
                let installment_val = money(a);
                let total = (installment_val * n_dec).max(principal);
                let interest_total = total - principal;
                let mut sum_prev = Decimal::ZERO;
                let mut installments = Vec::with_capacity(n as usize);

                for k in 1..=n {
                    let due_date =
                        calculate_installment_due_date(plan.first_due_date, k, plan.frequency);
                    let amount = if k < n {
                        sum_prev += installment_val;
                        installment_val
                    } else {
                        total - sum_prev
                    };
                    installments.push(ScheduledInstallment {
                        n: k,
                        due_date,
                        amount,
                    });
                }
                (interest_total, installments)
            }
        }
    }
}

/// Genera el plan completo: con o sin plan.
/// - Sin credit_plan (crédito libre): sin cuotas, interest_total = 0, due_date = hoy + 30 días.
/// - Con plan: due_date es la fecha de la cuota 1.
pub fn generate_schedule(
    principal: Decimal,
    plan: Option<&CreditPlan>,
    today: NaiveDate,
) -> (Decimal, NaiveDate, Vec<ScheduledInstallment>) {
    match plan {
        None => (Decimal::ZERO, today + chrono::Duration::days(30), vec![]),
        Some(p) => {
            let (interest_total, installments) = schedule(principal, p);
            let due_date = installments
                .first()
                .map(|i| i.due_date)
                .unwrap_or(p.first_due_date);
            (interest_total, due_date, installments)
        }
    }
}

/// `GET /v1/credit/schedule-preview` (credit:sell).
pub async fn preview(
    pool: &sqlx::PgPool,
    claims: &Claims,
    q: SchedulePreviewQuery,
) -> Result<SchedulePreviewResponse, ApiError> {
    let (scope, _tx) = crate::tenancy::begin(pool, claims).await?;
    scope.require_permission("credit:sell")?;

    let plan = CreditPlan {
        installments: q.installments,
        frequency: q.frequency,
        first_due_date: q.first_due_date,
        interest_rate_monthly: q.interest_rate_monthly,
        interest_type: q.interest_type,
    };
    plan.validate()?;

    let (interest_total, installments) = schedule(q.principal, &plan);
    let total = q.principal + interest_total;

    Ok(SchedulePreviewResponse {
        interest_total,
        total,
        installments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn schedule_simple_last_absorbs_rounding() {
        // 100.000 en 3 cuotas sin interés da 33.333,33 + 33.333,33 + 33.333,34
        let plan = CreditPlan {
            installments: 3,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            interest_rate_monthly: Decimal::ZERO,
            interest_type: InterestType::Simple,
        };
        let (interest, installments) = schedule(d("100000"), &plan);
        assert_eq!(interest, Decimal::ZERO);
        assert_eq!(installments.len(), 3);
        assert_eq!(installments[0].amount, d("33333.33"));
        assert_eq!(installments[1].amount, d("33333.33"));
        assert_eq!(installments[2].amount, d("33333.34"));
        assert_eq!(
            installments[0].amount + installments[1].amount + installments[2].amount,
            d("100000.00")
        );
    }

    #[test]
    fn schedule_fixed_payment_known_value() {
        // 1.000.000, 12 cuotas mensuales al 2 % da una cuota de $94.559,60
        let plan = CreditPlan {
            installments: 12,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            interest_rate_monthly: d("2.0"),
            interest_type: InterestType::CuotaFija,
        };
        let (interest, installments) = schedule(d("1000000"), &plan);
        assert_eq!(installments.len(), 12);
        for inst in &installments {
            assert_eq!(inst.amount, d("94559.60"));
        }
        let total: Decimal = installments.iter().map(|i| i.amount).sum();
        assert_eq!(total, d("1134715.20"));
        assert_eq!(interest, d("134715.20"));
    }

    #[test]
    fn schedule_month_end() {
        // con first_due_date = 31 de enero y frecuencia mensual, la segunda cuota vence el 28 o 29 de febrero
        let plan = CreditPlan {
            installments: 3,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
            interest_rate_monthly: Decimal::ZERO,
            interest_type: InterestType::Simple,
        };
        let (_, installments) = schedule(d("300000"), &plan);
        assert_eq!(
            installments[0].due_date,
            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
        );
        // 2026 no es bisiesto -> 28 de febrero
        assert_eq!(
            installments[1].due_date,
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert_eq!(
            installments[2].due_date,
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
        );

        // Probar año bisiesto 2024
        let plan_leap = CreditPlan {
            installments: 2,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
            interest_rate_monthly: Decimal::ZERO,
            interest_type: InterestType::Simple,
        };
        let (_, installments_leap) = schedule(d("200000"), &plan_leap);
        assert_eq!(
            installments_leap[1].due_date,
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn schedule_fixed_never_negative_interest() {
        // 10 pesos al 0,01 % mensual en 3 cuotas CUOTA_FIJA -> interest_total = 0 y cuotas de 3,33, 3,33 y 3,34
        let plan = CreditPlan {
            installments: 3,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            interest_rate_monthly: d("0.01"),
            interest_type: InterestType::CuotaFija,
        };
        let (interest, installments) = schedule(d("10"), &plan);
        assert_eq!(interest, Decimal::ZERO);
        assert_eq!(installments.len(), 3);
        assert_eq!(installments[0].amount, d("3.33"));
        assert_eq!(installments[1].amount, d("3.33"));
        assert_eq!(installments[2].amount, d("3.34"));
    }

    #[test]
    fn credit_plan_rejects_interest() {
        let plan_with_interest = CreditPlan {
            installments: 3,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            interest_rate_monthly: d("2.0"),
            interest_type: InterestType::Simple,
        };
        assert!(plan_with_interest.validate().is_err());

        let plan_no_interest = CreditPlan {
            installments: 3,
            frequency: Frequency::Mensual,
            first_due_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            interest_rate_monthly: Decimal::ZERO,
            interest_type: InterestType::Simple,
        };
        assert!(plan_no_interest.validate().is_ok());
    }
}

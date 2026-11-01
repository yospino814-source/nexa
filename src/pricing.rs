#![allow(clippy::collapsible_if, clippy::unnecessary_map_or)]

//! Motor de precios puro (F1.5).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::money::{money, money_floor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromoKind {
    Porcentaje,
    Fijo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromoScope {
    Pedido,
    Producto,
    Categoria,
}

#[derive(Debug, Clone)]
pub struct PriceLine {
    pub index: usize,
    pub product_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tier_id: Option<Uuid>,
    pub overridden: bool,
    pub custom: bool,
    pub manual_pct: Option<Decimal>,
    pub tax_rate: Decimal,
}

#[derive(Debug, Clone)]
pub struct PromoRule {
    pub id: Uuid,
    pub name: String,
    pub kind: PromoKind,
    pub value: Decimal,
    pub scope: PromoScope,
    pub product_ids: HashSet<Uuid>,
    pub category_ids: HashSet<Uuid>,
    pub min_qty: Option<Decimal>,
    pub min_subtotal: Option<Decimal>,
    pub max_discount: Option<Decimal>,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub skip_tier_lines: bool,
}

#[derive(Debug, Clone)]
pub struct CouponRule {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: PromoKind,
    pub value: Decimal,
    pub scope: PromoScope,
    pub product_ids: HashSet<Uuid>,
    pub category_ids: HashSet<Uuid>,
    pub min_qty: Option<Decimal>,
    pub min_subtotal: Option<Decimal>,
    pub max_discount: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LineDiscountBreakdown {
    pub manual: Decimal,
    pub promotion: Decimal,
    pub order: Decimal,
    pub coupon: Decimal,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricedLine {
    pub index: usize,
    pub gross: Decimal,
    pub discount: LineDiscountBreakdown,
    pub promotion_id: Option<Uuid>,
    pub line_total: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoApplied {
    pub id: Uuid,
    pub name: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouponApplied {
    pub id: Uuid,
    pub code: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CouponError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricedCart {
    pub lines: Vec<PricedLine>,
    pub subtotal_gross: Decimal,
    pub discount_total: Decimal,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub tip_amount: Decimal,
    pub total: Decimal,
    pub promotions: Vec<PromoApplied>,
    pub coupon: Option<CouponApplied>,
    pub coupon_error: Option<CouponError>,
}

/// Algoritmo de reparto exacto en centavos.
pub fn allocate(amount: Decimal, weights: &[Decimal]) -> Vec<Decimal> {
    if weights.is_empty() {
        return Vec::new();
    }
    let w_sum: Decimal = weights.iter().copied().sum();
    if w_sum <= Decimal::ZERO || amount <= Decimal::ZERO {
        return vec![Decimal::ZERO; weights.len()];
    }

    let is_capped = amount <= w_sum;
    let mut shares: Vec<Decimal> = weights
        .iter()
        .map(|&w| {
            let s = money_floor(amount * w / w_sum);
            if is_capped { s.min(w) } else { s }
        })
        .collect();

    let shares_sum: Decimal = shares.iter().copied().sum();
    let diff = amount - shares_sum;
    let mut cents = (diff * Decimal::from(100)).round();

    if cents > Decimal::ZERO {
        let mut indices: Vec<usize> = (0..weights.len()).collect();
        indices.sort_by(|&a, &b| weights[b].cmp(&weights[a]).then_with(|| a.cmp(&b)));

        let cent_val = Decimal::new(1, 2); // 0.01
        while cents > Decimal::ZERO {
            let mut incremented = false;
            for &idx in &indices {
                if cents <= Decimal::ZERO {
                    break;
                }
                if is_capped && shares[idx] + cent_val > weights[idx] {
                    continue;
                }
                shares[idx] += cent_val;
                cents -= Decimal::ONE;
                incremented = true;
            }
            if !incremented {
                break;
            }
        }
    }

    shares
}

/// Motor de cálculo de precios puro.
pub fn price_cart(
    lines: &[PriceLine],
    promos: &[PromoRule],
    coupon: Option<&CouponRule>,
    tip: Decimal,
) -> Result<PricedCart, CouponError> {
    let tip = money(tip);

    // 1. Bruto por línea: gross_i = money(qty_i × unit_price_i). Subtotal bruto: G = Σ gross_i.
    let gross_lines: Vec<Decimal> = lines.iter().map(|l| money(l.qty * l.unit_price)).collect();
    let g: Decimal = gross_lines.iter().copied().sum();

    // 2. Descuento manual: manual_i = money(gross_i × manual_pct_i / 100), o 0 si no hay porcentaje.
    let manual_discounts: Vec<Decimal> = lines
        .iter()
        .zip(&gross_lines)
        .map(|(line, &gross)| {
            if let Some(pct) = line.manual_pct {
                money(gross * pct / Decimal::from(100)).min(gross)
            } else {
                Decimal::ZERO
            }
        })
        .collect();

    // 3. Promoción por línea (alcance Producto o Categoria)
    let mut promo_discounts = vec![Decimal::ZERO; lines.len()];
    let mut promo_ids: Vec<Option<Uuid>> = vec![None; lines.len()];
    let mut promo_spent: HashMap<Uuid, Decimal> = HashMap::new();
    let mut promo_names: HashMap<Uuid, String> = HashMap::new();

    for (i, line) in lines.iter().enumerate() {
        let gross = gross_lines[i];
        let manual = manual_discounts[i];
        let available_line = gross - manual;

        // Una línea es elegible si no es libre (custom = false) y no tiene cambio de precio (overridden = false)
        if line.custom || line.overridden || available_line <= Decimal::ZERO {
            continue;
        }

        struct LineCandidate<'a> {
            promo: &'a PromoRule,
            amount: Decimal,
        }

        let mut candidates: Vec<LineCandidate> = Vec::new();

        for p in promos {
            if p.scope == PromoScope::Pedido {
                continue;
            }
            // Elegibilidad
            let matches_target = match p.scope {
                PromoScope::Producto => line
                    .product_id
                    .map_or(false, |pid| p.product_ids.contains(&pid)),
                PromoScope::Categoria => line
                    .category_id
                    .map_or(false, |cid| p.category_ids.contains(&cid)),
                _ => false,
            };
            if !matches_target {
                continue;
            }
            if let Some(min_qty) = p.min_qty {
                if line.qty < min_qty {
                    continue;
                }
            }
            if let Some(min_sub) = p.min_subtotal {
                if g < min_sub {
                    continue;
                }
            }
            if p.skip_tier_lines && line.tier_id.is_some() {
                continue;
            }

            // Monto
            let raw_amount = match p.kind {
                PromoKind::Porcentaje => money(available_line * p.value / Decimal::from(100)),
                PromoKind::Fijo => money(p.value * line.qty),
            };
            let capped_line = raw_amount.min(available_line);

            // max_discount por promoción y venta
            let spent = promo_spent.get(&p.id).copied().unwrap_or(Decimal::ZERO);
            let final_amount = if let Some(max) = p.max_discount {
                let remaining = (max - spent).max(Decimal::ZERO);
                capped_line.min(remaining)
            } else {
                capped_line
            };

            if final_amount > Decimal::ZERO {
                candidates.push(LineCandidate {
                    promo: p,
                    amount: final_amount,
                });
            }
        }

        // Elegir una sola: mayor monto; empate: mayor priority, luego created_at más antiguo, luego id menor
        candidates.sort_by(|a, b| {
            b.amount
                .cmp(&a.amount)
                .then_with(|| b.promo.priority.cmp(&a.promo.priority))
                .then_with(|| a.promo.created_at.cmp(&b.promo.created_at))
                .then_with(|| a.promo.id.cmp(&b.promo.id))
        });

        if let Some(best) = candidates.first() {
            promo_discounts[i] = best.amount;
            promo_ids[i] = Some(best.promo.id);
            *promo_spent.entry(best.promo.id).or_insert(Decimal::ZERO) += best.amount;
            promo_names.insert(best.promo.id, best.promo.name.clone());
        }
    }

    // 4. Neto 1: net1_i = gross_i − manual_i − promo_i
    let net1_lines: Vec<Decimal> = (0..lines.len())
        .map(|i| gross_lines[i] - manual_discounts[i] - promo_discounts[i])
        .collect();

    // 5. Promoción de pedido
    let mut order_discounts = vec![Decimal::ZERO; lines.len()];

    struct OrderCandidate<'a> {
        promo: &'a PromoRule,
        amount: Decimal,
        eligible_indices: Vec<usize>,
    }

    let mut order_candidates: Vec<OrderCandidate> = Vec::new();

    for p in promos {
        if p.scope != PromoScope::Pedido {
            continue;
        }

        // Líneas elegibles: las no cambiadas de precio, quitando las que tienen tier_id si skip_tier_lines está activo
        let eligible_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.overridden && !(p.skip_tier_lines && l.tier_id.is_some()))
            .map(|(i, _)| i)
            .collect();

        let n1: Decimal = eligible_indices.iter().map(|&i| net1_lines[i]).sum();

        if n1 <= Decimal::ZERO {
            continue;
        }
        if let Some(min_sub) = p.min_subtotal {
            if g < min_sub {
                continue;
            }
        }

        let raw_amount = match p.kind {
            PromoKind::Porcentaje => money(n1 * p.value / Decimal::from(100)),
            PromoKind::Fijo => p.value.min(n1),
        };

        let spent = promo_spent.get(&p.id).copied().unwrap_or(Decimal::ZERO);
        let final_amount = if let Some(max) = p.max_discount {
            let remaining = (max - spent).max(Decimal::ZERO);
            raw_amount.min(remaining)
        } else {
            raw_amount
        };

        if final_amount > Decimal::ZERO {
            order_candidates.push(OrderCandidate {
                promo: p,
                amount: final_amount,
                eligible_indices,
            });
        }
    }

    order_candidates.sort_by(|a, b| {
        b.amount
            .cmp(&a.amount)
            .then_with(|| b.promo.priority.cmp(&a.promo.priority))
            .then_with(|| a.promo.created_at.cmp(&b.promo.created_at))
            .then_with(|| a.promo.id.cmp(&b.promo.id))
    });

    if let Some(best) = order_candidates.first() {
        let weights: Vec<Decimal> = best
            .eligible_indices
            .iter()
            .map(|&i| net1_lines[i])
            .collect();
        let allocated = allocate(best.amount, &weights);
        for (idx_pos, &line_idx) in best.eligible_indices.iter().enumerate() {
            order_discounts[line_idx] = allocated[idx_pos];
        }
        *promo_spent.entry(best.promo.id).or_insert(Decimal::ZERO) += best.amount;
        promo_names.insert(best.promo.id, best.promo.name.clone());
    }

    // 6. Neto 2: net2_i = net1_i − order_i
    let net2_lines: Vec<Decimal> = (0..lines.len())
        .map(|i| net1_lines[i] - order_discounts[i])
        .collect();

    // 7. Cupón
    let mut coupon_discounts = vec![Decimal::ZERO; lines.len()];
    let mut coupon_applied: Option<CouponApplied> = None;

    if let Some(c) = coupon {
        let eligible_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                if l.overridden {
                    return false;
                }
                match c.scope {
                    PromoScope::Pedido => true,
                    PromoScope::Producto => l
                        .product_id
                        .map_or(false, |pid| c.product_ids.contains(&pid)),
                    PromoScope::Categoria => l
                        .category_id
                        .map_or(false, |cid| c.category_ids.contains(&cid)),
                }
            })
            .map(|(i, _)| i)
            .collect();

        let b: Decimal = eligible_indices.iter().map(|&i| net2_lines[i]).sum();
        if b <= Decimal::ZERO {
            return Err(CouponError {
                code: "COUPON_NOT_APPLICABLE".into(),
                message: "El cupón no aplica a estos productos.".into(),
            });
        }
        if let Some(min_sub) = c.min_subtotal {
            if g < min_sub {
                return Err(CouponError {
                    code: "COUPON_MIN_SUBTOTAL".into(),
                    message: "La compra no alcanza el mínimo del cupón.".into(),
                });
            }
        }

        let raw_amount = match c.kind {
            PromoKind::Porcentaje => money(b * c.value / Decimal::from(100)),
            PromoKind::Fijo => c.value.min(b),
        };
        let final_amount = if let Some(max) = c.max_discount {
            raw_amount.min(max)
        } else {
            raw_amount
        };

        let weights: Vec<Decimal> = eligible_indices.iter().map(|&i| net2_lines[i]).collect();
        let allocated = allocate(final_amount, &weights);
        for (idx_pos, &line_idx) in eligible_indices.iter().enumerate() {
            coupon_discounts[line_idx] = allocated[idx_pos];
        }

        coupon_applied = Some(CouponApplied {
            id: c.id,
            code: c.code.clone(),
            amount: final_amount,
        });
    }

    // 8. Resultado por línea
    let mut priced_lines = Vec::with_capacity(lines.len());
    for i in 0..lines.len() {
        let gross = gross_lines[i];
        let manual = manual_discounts[i];
        let promo = promo_discounts[i];
        let order = order_discounts[i];
        let coup = coupon_discounts[i];
        let line_disc = manual + promo + order + coup;
        let line_total = gross - line_disc;
        let tax = money(line_total * lines[i].tax_rate);

        priced_lines.push(PricedLine {
            index: lines[i].index,
            gross,
            discount: LineDiscountBreakdown {
                manual,
                promotion: promo,
                order,
                coupon: coup,
                total: line_disc,
            },
            promotion_id: promo_ids[i],
            line_total,
            tax_rate: lines[i].tax_rate,
            tax_amount: tax,
        });
    }

    // 9. Totales
    let subtotal: Decimal = priced_lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = priced_lines.iter().map(|l| l.tax_amount).sum();
    let discount_total: Decimal = priced_lines.iter().map(|l| l.discount.total).sum();
    let total = subtotal + tax_total + tip;

    let mut promotions_list: Vec<PromoApplied> = promo_spent
        .into_iter()
        .map(|(id, amount)| PromoApplied {
            name: promo_names.get(&id).cloned().unwrap_or_default(),
            id,
            amount,
        })
        .collect();
    promotions_list.sort_by(|a, b| b.amount.cmp(&a.amount).then_with(|| a.id.cmp(&b.id)));

    Ok(PricedCart {
        lines: priced_lines,
        subtotal_gross: g,
        discount_total,
        subtotal,
        tax_total,
        tip_amount: tip,
        total,
        promotions: promotions_list,
        coupon: coupon_applied,
        coupon_error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    macro_rules! dec {
        ($val:expr) => {
            Decimal::from_str_exact(stringify!($val)).unwrap()
        };
    }
    use std::collections::HashSet;

    fn make_line(
        index: usize,
        prod_id: Option<Uuid>,
        cat_id: Option<Uuid>,
        qty: Decimal,
        price: Decimal,
        tax_rate: Decimal,
    ) -> PriceLine {
        PriceLine {
            index,
            product_id: prod_id,
            category_id: cat_id,
            qty,
            unit_price: price,
            tier_id: None,
            overridden: false,
            custom: false,
            manual_pct: None,
            tax_rate,
        }
    }

    #[test]
    fn allocate_sums_exactly() {
        let shares = allocate(dec!(100), &[dec!(1), dec!(1), dec!(1)]);
        assert_eq!(shares, vec![dec!(33.34), dec!(33.33), dec!(33.33)]);
        assert_eq!(shares.iter().copied().sum::<Decimal>(), dec!(100));
    }

    #[test]
    fn allocate_respects_caps() {
        let shares = allocate(dec!(30), &[dec!(10), dec!(20)]);
        assert_eq!(shares, vec![dec!(10), dec!(20)]);

        let shares2 = allocate(dec!(15), &[dec!(10), dec!(20)]);
        assert_eq!(shares2, vec![dec!(5), dec!(10)]);
        assert!(shares2[0] <= dec!(10));
        assert!(shares2[1] <= dec!(20));
    }

    #[test]
    fn line_promo_picks_best() {
        let p_id = Uuid::new_v4();
        let l = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));

        let promo_low = PromoRule {
            id: Uuid::new_v4(),
            name: "5%".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(5),
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };
        let promo_high = PromoRule {
            id: Uuid::new_v4(),
            name: "10%".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let res = price_cart(&[l], &[promo_low, promo_high.clone()], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.promotion, dec!(1000));
        assert_eq!(res.lines[0].promotion_id, Some(promo_high.id));
    }

    #[test]
    fn promo_tie_breaks_by_priority() {
        let p_id = Uuid::new_v4();
        let l = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));

        let promo1 = PromoRule {
            id: Uuid::new_v4(),
            name: "P1".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 1,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };
        let promo2 = PromoRule {
            id: Uuid::new_v4(),
            name: "P2".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 2,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let res = price_cart(&[l], &[promo1, promo2.clone()], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].promotion_id, Some(promo2.id));
    }

    #[test]
    fn max_discount_caps_across_lines() {
        let p_id = Uuid::new_v4();
        let l1 = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));
        let l2 = make_line(1, Some(p_id), None, dec!(1), dec!(10000), dec!(0));

        let promo = PromoRule {
            id: Uuid::new_v4(),
            name: "Cap 1500".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10), // 1000 each line
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: Some(dec!(1500)),
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let res = price_cart(&[l1, l2], &[promo], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.promotion, dec!(1000));
        assert_eq!(res.lines[1].discount.promotion, dec!(500));
        assert_eq!(res.discount_total, dec!(1500));
    }

    #[test]
    fn skip_tier_lines_excludes_tier_line() {
        let p_id = Uuid::new_v4();
        let mut l = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));
        l.tier_id = Some(Uuid::new_v4());

        let promo = PromoRule {
            id: Uuid::new_v4(),
            name: "Skip Tier".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: true,
        };

        let res = price_cart(&[l], &[promo], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.promotion, dec!(0));
    }

    #[test]
    fn overridden_line_gets_no_discounts() {
        let p_id = Uuid::new_v4();
        let mut l = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));
        l.overridden = true;
        l.manual_pct = Some(dec!(10));

        let promo = PromoRule {
            id: Uuid::new_v4(),
            name: "Order Promo".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Pedido,
            product_ids: HashSet::new(),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let res = price_cart(&[l], &[promo], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.manual, dec!(1000)); // manual is on line
        assert_eq!(res.lines[0].discount.promotion, dec!(0));
        assert_eq!(res.lines[0].discount.order, dec!(0));
    }

    #[test]
    fn order_promo_requires_min_subtotal() {
        let l = make_line(0, Some(Uuid::new_v4()), None, dec!(1), dec!(5000), dec!(0));
        let promo = PromoRule {
            id: Uuid::new_v4(),
            name: "Order Min 10000".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10),
            scope: PromoScope::Pedido,
            product_ids: HashSet::new(),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: Some(dec!(10000)),
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let res = price_cart(&[l], &[promo], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.order, dec!(0));
    }

    #[test]
    fn coupon_after_promotions_on_net() {
        let p_id = Uuid::new_v4();
        let l = make_line(0, Some(p_id), None, dec!(1), dec!(10000), dec!(0));

        let promo = PromoRule {
            id: Uuid::new_v4(),
            name: "Line 10%".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10), // 1000
            scope: PromoScope::Producto,
            product_ids: HashSet::from([p_id]),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
            priority: 0,
            created_at: Utc::now(),
            skip_tier_lines: false,
        };

        let coupon = CouponRule {
            id: Uuid::new_v4(),
            code: "DESC10".into(),
            name: "Coupon 10%".into(),
            kind: PromoKind::Porcentaje,
            value: dec!(10), // 10% of net 9000 = 900
            scope: PromoScope::Pedido,
            product_ids: HashSet::new(),
            category_ids: HashSet::new(),
            min_qty: None,
            min_subtotal: None,
            max_discount: None,
        };

        let res = price_cart(&[l], &[promo], Some(&coupon), dec!(0)).unwrap();
        assert_eq!(res.lines[0].discount.promotion, dec!(1000));
        assert_eq!(res.lines[0].discount.coupon, dec!(900));
        assert_eq!(res.lines[0].line_total, dec!(8100));
    }

    #[test]
    fn discount_never_exceeds_gross() {
        // 200 casos aleatorios con semilla fija sin crates nuevos
        let mut state: u64 = 123456789;
        let mut rand = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 33) & 0xffff) as u32
        };

        for _ in 0..200 {
            let qty = Decimal::from((rand() % 10) + 1);
            let price = Decimal::from((rand() % 50000) + 1000);
            let p_id = Uuid::new_v4();
            let mut l = make_line(0, Some(p_id), None, qty, price, dec!(0.19));
            if rand() % 2 == 0 {
                l.manual_pct = Some(Decimal::from(rand() % 20));
            }

            let promo = PromoRule {
                id: Uuid::new_v4(),
                name: "Rand Promo".into(),
                kind: if rand() % 2 == 0 {
                    PromoKind::Porcentaje
                } else {
                    PromoKind::Fijo
                },
                value: Decimal::from((rand() % 50) + 1),
                scope: if rand() % 2 == 0 {
                    PromoScope::Producto
                } else {
                    PromoScope::Pedido
                },
                product_ids: HashSet::from([p_id]),
                category_ids: HashSet::new(),
                min_qty: None,
                min_subtotal: None,
                max_discount: Some(Decimal::from(10000)),
                priority: (rand() % 10) as i32,
                created_at: Utc::now(),
                skip_tier_lines: false,
            };

            let res = price_cart(&[l], &[promo], None, dec!(0)).unwrap();
            for pl in &res.lines {
                assert!(
                    pl.discount.total <= pl.gross,
                    "discount {} exceeded gross {}",
                    pl.discount.total,
                    pl.gross
                );
                assert!(pl.line_total >= Decimal::ZERO);
            }
        }
    }

    #[test]
    fn tax_on_discounted_base() {
        let mut l = make_line(
            0,
            Some(Uuid::new_v4()),
            None,
            dec!(1),
            dec!(10000),
            dec!(0.19),
        );
        l.manual_pct = Some(dec!(10));

        let res = price_cart(&[l], &[], None, dec!(0)).unwrap();
        assert_eq!(res.lines[0].line_total, dec!(9000));
        assert_eq!(res.lines[0].tax_amount, dec!(1710));
    }
}

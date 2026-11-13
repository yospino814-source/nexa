use rust_decimal::{Decimal, RoundingStrategy};

#[allow(dead_code)]
pub fn money(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

#[allow(dead_code)]
pub fn money_floor(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn money_rounds_half_away_from_zero() {
        assert_eq!(money(d("0.005")), d("0.01"));
        assert_eq!(money(d("-0.005")), d("-0.01"));
        assert_eq!(money(d("1.234")), d("1.23"));
    }
}

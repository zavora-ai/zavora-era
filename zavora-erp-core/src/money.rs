//! Monetary rounding policy (Requirement 5).
//!
//! All monetary values are rounded to 2 decimal places using banker's rounding
//! (round half to even) before being stored or compared. PAYE is an exception:
//! KRA requires it rounded to the nearest shilling (0 decimal places).
//!
//! Centralising these helpers ensures every posting path applies the same
//! policy, so VAT-derived fractional amounts cannot block journal posting or
//! create audit discrepancies.

use rust_decimal::{Decimal, RoundingStrategy};

/// The maximum imbalance (in base currency units) that a journal entry may carry
/// due to VAT line-level rounding accumulation before it is corrected with a
/// rounding-adjustment line. Anything larger is treated as a genuine imbalance.
pub const ROUNDING_TOLERANCE: Decimal = Decimal::from_parts(1, 0, 0, false, 2); // 0.01

/// Round a monetary value to 2 decimal places using banker's rounding
/// (round half to even).
pub fn round_money(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
}

/// Round PAYE to the nearest shilling (0 decimal places), as required by KRA.
pub fn round_paye(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven)
}

/// The result of checking whether a set of journal totals balances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundingOutcome {
    /// Debits already equal credits.
    Balanced,
    /// Within tolerance — add an adjustment line. `debit` is true when the
    /// adjustment line should be a debit (credits exceeded debits).
    Adjust { debit: bool, amount: Decimal },
    /// Out of tolerance — the entry is genuinely unbalanced and must be rejected.
    Unbalanced,
}

/// Decide how to reconcile journal totals (Req 2.6, 5.3). A residual imbalance of
/// at most [`ROUNDING_TOLERANCE`] is absorbed by an adjustment line; anything
/// larger is rejected.
pub fn rounding_outcome(total_debits: Decimal, total_credits: Decimal) -> RoundingOutcome {
    let diff = total_debits - total_credits;
    if diff.is_zero() {
        RoundingOutcome::Balanced
    } else if diff.abs() <= ROUNDING_TOLERANCE {
        // diff > 0 => too many debits => the adjustment line is a credit.
        RoundingOutcome::Adjust {
            debit: diff < Decimal::ZERO,
            amount: diff.abs(),
        }
    } else {
        RoundingOutcome::Unbalanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn rounds_to_two_places() {
        assert_eq!(round_money(dec!(1.005)), dec!(1.00)); // half to even -> down
        assert_eq!(round_money(dec!(1.015)), dec!(1.02)); // half to even -> up
        assert_eq!(round_money(dec!(2.675)), dec!(2.68));
        assert_eq!(round_money(dec!(1.234)), dec!(1.23));
        assert_eq!(round_money(dec!(1.236)), dec!(1.24));
    }

    #[test]
    fn paye_rounds_to_shilling() {
        assert_eq!(round_paye(dec!(1234.49)), dec!(1234));
        assert_eq!(round_paye(dec!(1234.50)), dec!(1234)); // half to even
        assert_eq!(round_paye(dec!(1235.50)), dec!(1236)); // half to even
        assert_eq!(round_paye(dec!(1234.51)), dec!(1235));
    }

    #[test]
    fn round_money_is_idempotent() {
        let v = round_money(dec!(99.999));
        assert_eq!(round_money(v), v);
    }

    #[test]
    fn tolerance_is_one_cent() {
        assert_eq!(ROUNDING_TOLERANCE, dec!(0.01));
    }

    #[test]
    fn rounding_outcome_classifies_correctly() {
        assert_eq!(rounding_outcome(dec!(100), dec!(100)), RoundingOutcome::Balanced);
        // 0.01 too many debits -> credit adjustment of 0.01
        assert_eq!(
            rounding_outcome(dec!(100.01), dec!(100)),
            RoundingOutcome::Adjust { debit: false, amount: dec!(0.01) }
        );
        // 0.01 too many credits -> debit adjustment of 0.01
        assert_eq!(
            rounding_outcome(dec!(100), dec!(100.01)),
            RoundingOutcome::Adjust { debit: true, amount: dec!(0.01) }
        );
        // 0.02 is beyond tolerance -> unbalanced
        assert_eq!(rounding_outcome(dec!(100.02), dec!(100)), RoundingOutcome::Unbalanced);
    }

    proptest::proptest! {
        /// Property: applying the rounding adjustment always yields a balanced
        /// entry whenever the residual is within tolerance (Req 2.6, 4.1, 5.3).
        #[test]
        fn rounding_adjustment_always_balances(
            debits_cents in 0i64..100_000_000i64,
            delta_cents in -1i64..=1i64,
        ) {
            let debits = Decimal::new(debits_cents, 2);
            let credits = debits - Decimal::new(delta_cents, 2);
            match rounding_outcome(debits, credits) {
                RoundingOutcome::Balanced => {
                    proptest::prop_assert_eq!(debits, credits);
                }
                RoundingOutcome::Adjust { debit, amount } => {
                    // Add the adjustment to the deficient side; totals must match.
                    let (d, c) = if debit {
                        (debits + amount, credits)
                    } else {
                        (debits, credits + amount)
                    };
                    proptest::prop_assert_eq!(d, c);
                }
                RoundingOutcome::Unbalanced => {
                    proptest::prop_assert!((debits - credits).abs() > ROUNDING_TOLERANCE);
                }
            }
        }
    }
}

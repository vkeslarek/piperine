//! How far a twin sits from its contract: the worst `|golden - actual| / bound` of a comparison,
//! where `bound = abstol + reltol * max(|golden|, |actual|)`. A ratio of 1 is exactly at the bound,
//! so a deviation reads as the fraction of the contract a twin uses. Never a gate: the contract's
//! own `assert` is.

use crate::contract::{Coordinate, ToleranceContract};

/// The worst ratio of one quantity of one fixture, and where it occurred.
#[derive(Clone, Debug, PartialEq)]
pub struct Deviation {
    pub fixture: String,
    pub quantity: String,
    pub worst_ratio: f64,
    pub at: Coordinate,
}

impl Deviation {
    /// The worst ratio over `(place, golden, actual)` rows. Panics on no rows: a deviation of
    /// nothing would read as a perfect match.
    pub fn of(
        fixture: &str,
        quantity: &str,
        contract: &ToleranceContract,
        rows: &[(Coordinate, f64, f64)],
    ) -> Self {
        let mut worst: Option<(f64, Coordinate)> = None;
        for &(at, golden, actual) in rows {
            let bound = contract.abstol() + contract.reltol() * golden.abs().max(actual.abs());
            let ratio = (golden - actual).abs() / bound;
            if worst.is_none_or(|(w, _)| ratio > w) {
                worst = Some((ratio, at));
            }
        }
        let (worst_ratio, at) = worst.expect("a deviation needs at least one compared row");
        Deviation { fixture: fixture.to_string(), quantity: quantity.to_string(), worst_ratio, at }
    }

    /// One aligned line per deviation.
    pub fn table(deviations: &[Deviation]) -> String {
        let fixture = deviations.iter().map(|d| d.fixture.len()).max().unwrap_or(0);
        let quantity = deviations.iter().map(|d| d.quantity.len()).max().unwrap_or(0);
        deviations
            .iter()
            .map(|d| {
                format!(
                    "{:<fixture$}  {:<quantity$}  {:>10.4}  {:?}\n",
                    d.fixture, d.quantity, d.worst_ratio, d.at
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worst_ratio_is_the_largest_difference_over_its_bound() {
        let c = ToleranceContract::DC_VOLTAGE;
        // bounds: 1e-6 + 1e-3*max; the middle row is the worst (ratio 0.5), neither first nor last.
        let rows = [
            (Coordinate::Time(0.0), 1.0, 1.0),
            (Coordinate::Time(1.0), 2.0, 2.0 + 0.5 * (1e-6 + 1e-3 * 2.0005)),
            (Coordinate::Time(2.0), 3.0, 3.0 + 0.1 * (1e-6 + 1e-3 * 3.0003)),
        ];
        let d = Deviation::of("f", "q", &c, &rows);
        assert_eq!(d.at, Coordinate::Time(1.0));
        assert!((d.worst_ratio - 0.5).abs() < 1e-3, "{}", d.worst_ratio);
    }

    #[test]
    fn a_pair_at_the_bound_has_ratio_one() {
        let c = ToleranceContract::DC_VOLTAGE;
        // golden 0 and actual = abstol + reltol*actual  =>  actual = abstol / (1 - reltol).
        let actual = 1e-6 / (1.0 - 1e-3);
        let d = Deviation::of("f", "q", &c, &[(Coordinate::OperatingPoint, 0.0, actual)]);
        assert!((d.worst_ratio - 1.0).abs() < 1e-12, "{}", d.worst_ratio);
        assert!(c.holds(0.0, actual * (1.0 - 1e-9)));
    }

    #[test]
    fn the_table_has_one_line_per_deviation() {
        let c = ToleranceContract::DC_VOLTAGE;
        let a = Deviation::of("alpha", "v(a)", &c, &[(Coordinate::OperatingPoint, 1.0, 1.0)]);
        let b = Deviation::of("b", "v(b)", &c, &[(Coordinate::OperatingPoint, 1.0, 1.0)]);
        assert_eq!(Deviation::table(&[a, b]).lines().count(), 2);
    }
}

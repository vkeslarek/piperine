//! The tolerance contracts: the only place a numeric bound of a parity comparison is written.
//!
//! A parity test calls [`ToleranceContract::assert`] and never compares a difference with a literal.
//! Widening a bound to turn a run green is the failure this module exists to prevent, so every
//! contract carries the reason for its value and a rejection prints that reason with the point.

/// Where in a quantity's domain a compared pair was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinate {
    OperatingPoint,
    Sweep(f64),
    Frequency(f64),
    Time(f64),
}

impl Coordinate {
    fn describe(&self) -> String {
        match self {
            Coordinate::OperatingPoint => "operating point".to_string(),
            Coordinate::Sweep(value) => format!("sweep value {value:e}"),
            Coordinate::Frequency(value) => format!("frequency {value:e} Hz"),
            Coordinate::Time(value) => format!("time {value:e} s"),
        }
    }
}

/// One compared pair's place: the fixture, the quantity and the coordinate, plus the first line of
/// `ngspice --version` once the reference run is known and the backend that produced the value.
#[derive(Clone, Copy, Debug)]
pub struct Point<'a> {
    pub fixture: &'a str,
    pub quantity: &'a str,
    pub at: Coordinate,
    pub ngspice: Option<&'a str>,
    pub backend: Option<&'a str>,
}

impl<'a> Point<'a> {
    pub fn new(fixture: &'a str, quantity: &'a str, at: Coordinate) -> Self {
        Point { fixture, quantity, at, ngspice: None, backend: None }
    }

    /// The same point, recording the `ngspice --version` line the golden came from.
    pub fn with_ngspice(self, version: &'a str) -> Self {
        Point { ngspice: Some(version), ..self }
    }

    /// The same point, naming the backend (`interp` or `rust`) whose value is compared.
    pub fn with_backend(self, backend: &'a str) -> Self {
        Point { backend: Some(backend), ..self }
    }
}

/// The agreement bound of one comparison domain: a pair `(golden, actual)` is accepted when
/// `|golden - actual| <= abstol + reltol * max(|golden|, |actual|)`.
#[derive(Clone, Copy, Debug)]
pub struct ToleranceContract {
    reltol: f64,
    abstol: f64,
    rationale: &'static str,
}

impl ToleranceContract {
    pub const DC_VOLTAGE: Self = Self {
        reltol: 1e-3,
        abstol: 1e-6,
        rationale: "both solvers stop Newton at reltol 1e-3 and 1 uV",
    };

    pub const DC_CURRENT: Self = Self {
        reltol: 1e-3,
        abstol: 1e-12,
        rationale: "the same reltol; the floor is both solvers' 1 pA flow tolerance, so a reverse-biased \
                    junction is not compared relatively against about zero",
    };

    pub const AC_MAGNITUDE: Self = Self {
        reltol: 1e-3,
        abstol: 1e-12,
        rationale: "a linear solve at a frozen operating point; the floor governs the deep stopband",
    };

    pub const AC_PHASE: Self = Self {
        reltol: 0.0,
        abstol: 1e-4,
        rationale: "radians, absolute; compared only where the magnitude clears its floor, never widened",
    };

    pub const TRAN_VOLTAGE: Self = Self {
        reltol: 1e-3,
        abstol: 1e-6,
        rationale: "two integrators under the same reltol; the interpolation onto ngspice's grid is kept \
                    a decade below by the harness's step cap",
    };

    pub fn reltol(&self) -> f64 {
        self.reltol
    }

    pub fn abstol(&self) -> f64 {
        self.abstol
    }

    pub fn rationale(&self) -> &'static str {
        self.rationale
    }

    /// The absolute bound: the smallest magnitude this contract distinguishes from zero.
    pub fn floor(&self) -> f64 {
        self.abstol
    }

    /// Whether a phase is compared at a point whose golden magnitude is `magnitude`: only above the
    /// `AC_MAGNITUDE` floor, since phase carries no information where the magnitude is about zero.
    pub fn phase_applies(magnitude: f64) -> bool {
        magnitude > Self::AC_MAGNITUDE.floor()
    }

    pub fn holds(&self, golden: f64, actual: f64) -> bool {
        (golden - actual).abs() <= self.abstol + self.reltol * golden.abs().max(actual.abs())
    }

    /// Accepts the pair or panics with everything needed to judge the failure: fixture, quantity,
    /// place, both values, the bound, the rationale and the ngspice version.
    pub fn assert(&self, at: &Point, golden: f64, actual: f64) {
        if self.holds(golden, actual) {
            return;
        }
        panic!(
            "parity failure: {fixture} / {quantity} at {place}\n  ngspice: {golden:e}\n  piperine: {actual:e}\n  \
             |difference| = {difference:e} > abstol {abstol:e} + reltol {reltol:e} * max(|values|) = {bound:e}\n  \
             rationale: {rationale}\n  reference: {version}\n  backend: {backend}",
            fixture = at.fixture,
            quantity = at.quantity,
            place = at.at.describe(),
            difference = (golden - actual).abs(),
            abstol = self.abstol,
            reltol = self.reltol,
            bound = self.abstol + self.reltol * golden.abs().max(actual.abs()),
            rationale = self.rationale,
            version = at.ngspice.unwrap_or("ngspice version not recorded"),
            backend = at.backend.unwrap_or("not recorded"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, ToleranceContract); 5] = [
        ("DC_VOLTAGE", ToleranceContract::DC_VOLTAGE),
        ("DC_CURRENT", ToleranceContract::DC_CURRENT),
        ("AC_MAGNITUDE", ToleranceContract::AC_MAGNITUDE),
        ("AC_PHASE", ToleranceContract::AC_PHASE),
        ("TRAN_VOLTAGE", ToleranceContract::TRAN_VOLTAGE),
    ];

    fn rejection(contract: &ToleranceContract, at: &Point, golden: f64, actual: f64) -> String {
        let result = std::panic::catch_unwind(|| contract.assert(at, golden, actual));
        let payload = result.expect_err("the pair must be rejected");
        match payload.downcast::<String>() {
            Ok(text) => *text,
            Err(other) => other.downcast::<&str>().map(|t| t.to_string()).expect("a text panic"),
        }
    }

    #[test]
    fn the_five_contracts_carry_bounds_and_rationale() {
        let expected = [(1e-3, 1e-6), (1e-3, 1e-12), (1e-3, 1e-12), (0.0, 1e-4), (1e-3, 1e-6)];
        for ((name, contract), (reltol, abstol)) in ALL.iter().zip(expected) {
            assert_eq!(contract.reltol(), reltol, "{name} reltol");
            assert_eq!(contract.abstol(), abstol, "{name} abstol");
            assert!(!contract.rationale().is_empty(), "{name} has no rationale");
        }
    }

    #[test]
    fn holds_is_abstol_plus_reltol_of_the_larger() {
        for (name, contract) in ALL {
            // The edge: the largest `actual` accepted against golden 1.0, found by bisection.
            let (mut inside, mut outside) = (1.0_f64, 2.0_f64);
            assert!(contract.holds(1.0, inside) && !contract.holds(1.0, outside), "{name}");
            while inside.next_up() < outside {
                let middle = inside + (outside - inside) / 2.0;
                if contract.holds(1.0, middle) { inside = middle } else { outside = middle }
            }
            assert_eq!(outside, inside.next_up(), "{name}: one ulp separates accepted from rejected");
            assert!(contract.holds(1.0, inside), "{name}: the last accepted value is inside");
            assert!(!contract.holds(1.0, inside.next_up()), "{name}: one ulp outside is rejected");
            // The edge is where the formula says, with the larger of the two values in the relative term.
            let formula = 1.0 + contract.abstol() + contract.reltol() * inside;
            assert!((inside - formula).abs() <= 4.0 * f64::EPSILON, "{name}: edge {inside} vs formula {formula}");
            // Symmetric in its arguments.
            assert_eq!(contract.holds(1.0, inside), contract.holds(inside, 1.0), "{name}");
        }
        assert!(ToleranceContract::AC_PHASE.holds(0.0, 1e-4), "at the absolute bound exactly");
        assert!(!ToleranceContract::AC_PHASE.holds(0.0, 1e-4_f64.next_up()));
    }

    #[test]
    fn a_rejection_names_point_values_bound_and_rationale() {
        let at = Point::new("diode_iv", "i(v1)", Coordinate::Sweep(0.35));
        let text = rejection(&ToleranceContract::DC_CURRENT, &at, 1.5, 2.5);
        for needle in ["diode_iv", "i(v1)", "sweep value 3.5e-1", "1.5e0", "2.5e0", "abstol 1e-12", "reltol 1e-3", "1 pA flow tolerance"] {
            assert!(text.contains(needle), "message lacks {needle:?}: {text}");
        }
        let at = Point::new("ac_rc", "v(out)", Coordinate::Frequency(1e5));
        assert!(rejection(&ToleranceContract::AC_MAGNITUDE, &at, 1.0, 2.0).contains("frequency 1e5 Hz"));
        let at = Point::new("tran_rc", "v(out)", Coordinate::Time(2e-6));
        assert!(rejection(&ToleranceContract::TRAN_VOLTAGE, &at, 1.0, 2.0).contains("time 2e-6 s"));
        let at = Point::new("divider", "v(mid)", Coordinate::OperatingPoint);
        assert!(rejection(&ToleranceContract::DC_VOLTAGE, &at, 1.0, 2.0).contains("operating point"));
    }

    #[test]
    fn a_rejection_names_the_ngspice_version() {
        let at = Point::new("divider", "v(mid)", Coordinate::OperatingPoint).with_ngspice("ngspice-47 : Circuit level simulation program");
        let text = rejection(&ToleranceContract::DC_VOLTAGE, &at, 7.5, 7.6);
        assert!(text.contains("ngspice-47 : Circuit level simulation program"), "{text}");
    }

    #[test]
    fn phase_is_gated_on_the_magnitude_floor() {
        let floor = ToleranceContract::AC_MAGNITUDE.floor();
        assert!(!ToleranceContract::phase_applies(floor), "at the floor the phase is not compared");
        assert!(!ToleranceContract::phase_applies(0.0));
        assert!(ToleranceContract::phase_applies(floor.next_up()));
        assert!(ToleranceContract::phase_applies(0.5));
    }

    #[test]
    fn two_zeros_are_accepted() {
        for (name, contract) in ALL {
            assert!(contract.holds(0.0, 0.0), "{name}");
            assert!(contract.holds(-0.0, 0.0), "{name}");
            contract.assert(&Point::new("f", "q", Coordinate::OperatingPoint), 0.0, 0.0);
        }
    }
}

//! UIC hold clamps (ngspice `CKTsetIC` analog): an `@initial` branch seed
//! is enforced during the t=0 solve and held through the first accepted
//! step by a large conductance across the seeded branch carrying `G·ic` —
//! the seed value becomes the *consistent* t=0 solution (the rest of the
//! circuit solves against it), not just a Newton guess overlaid on an
//! inconsistent operating point.

use crate::analog::AnalogReference;
use crate::math::linear::Stamp;

/// One seeded branch clamp: hold `V(plus) − V(minus) ≈ ic`.
#[derive(Debug, Clone)]
pub struct UicClamp {
    pub plus: AnalogReference,
    pub minus: Option<AnalogReference>,
    pub ic: f64,
}

impl UicClamp {
    /// Clamp conductance: large enough to pin the branch against any circuit
    /// admittance, small enough to keep the matrix conditioned.
    pub const G: f64 = 1.0e12;

    /// Stamp `G·(v − ic)`: conductance across the branch plus the `G·ic`
    /// offset current that pins the branch voltage to `ic`.
    pub fn stamp(&self, stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        stamps.push(Stamp::Matrix(self.plus.clone(), self.plus.clone(), Self::G));
        stamps.push(Stamp::Rhs(self.plus.clone(), Self::G * self.ic));
        if let Some(minus) = &self.minus {
            stamps.push(Stamp::Matrix(minus.clone(), minus.clone(), Self::G));
            stamps.push(Stamp::Matrix(self.plus.clone(), minus.clone(), -Self::G));
            stamps.push(Stamp::Matrix(minus.clone(), self.plus.clone(), -Self::G));
            stamps.push(Stamp::Rhs(minus.clone(), -Self::G * self.ic));
        }
    }
}

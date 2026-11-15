//! `Real`: an `f64` that is stored, compared and serialized by its bit pattern.
//!
//! `NaN` and the infinities are values of the IR (`inf` is a prelude constant);
//! JSON cannot write them as numbers, and `==` on `f64` would make `-0.0` equal
//! `0.0` and `NaN` unequal to itself. Bits make equality and the round trip exact.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Real(u64);

impl Real {
    pub fn new(value: f64) -> Self {
        Real(value.to_bits())
    }

    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// The bit pattern, as serialized.
    pub fn bits(self) -> u64 {
        self.0
    }
}

impl From<f64> for Real {
    fn from(value: f64) -> Self {
        Real::new(value)
    }
}

impl Serialize for Real {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for Real {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(Real)
    }
}

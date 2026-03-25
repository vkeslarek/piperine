use crate::devices::Component;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Dimensionless;

/// Coupled (mutual) inductor element (`K`).
///
/// Defines magnetic coupling between two inductors.
/// Coupling coefficient must be > 0 and ≤ 1.
/// See ngspice manual §3.3.12.
#[derive(Debug, Clone)]
pub struct MutualInductor {
    name: String,
    /// Name of the first inductor (e.g. "L1").
    inductor1: String,
    /// Name of the second inductor (e.g. "L2").
    inductor2: String,
    /// Coupling coefficient K (0 < K ≤ 1).
    coupling: Dynamic<Dimensionless>,
}

impl MutualInductor {
    pub const SYMBOL: &str = "K";

    /// Creates a new mutual inductor coupling between two inductors.
    ///
    /// * `name` — instance name (e.g. "12")
    /// * `inductor1` — name of first inductor (e.g. "L1")
    /// * `inductor2` — name of second inductor (e.g. "L2")
    /// * `coupling` — coupling coefficient (0 < K ≤ 1)
    pub fn new(
        name: impl Into<String>,
        inductor1: impl Into<String>,
        inductor2: impl Into<String>,
        coupling: impl Into<Dynamic<Dimensionless>>,
    ) -> Self {
        Self {
            name: name.into(),
            inductor1: inductor1.into(),
            inductor2: inductor2.into(),
            coupling: coupling.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn inductor1(&self) -> &str {
        &self.inductor1
    }
    pub fn inductor2(&self) -> &str {
        &self.inductor2
    }
    pub fn coupling(&self) -> &Dynamic<Dimensionless> {
        &self.coupling
    }
}

impl Component for MutualInductor {}

impl SpiceElement for MutualInductor {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for MutualInductor {
    fn into_spice(&self) -> String {
        format!(
            "{}{} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.inductor1(),
            self.inductor2(),
            self.coupling()
        )
    }
}

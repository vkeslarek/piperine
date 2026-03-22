use crate::circuit::netlist::{ComponentIdentifier, IntoNodeIdentifier, NodeIdentifier};
use crate::devices::{Component, Dynamic};
use crate::unit::{Ampere, Dimensionless, Henry, Kelvin};

/// Two-terminal linear inductor (`L+`, `L-`) with the standard `ind` parameter set.
#[derive(Debug, Clone)]
pub struct Inductor {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    pub params: InductorParams,
}

/// Parameter block plus canonical defaults for the linear inductor device.
#[derive(Debug, Clone)]
pub struct InductorParams {
    /// Electrical inductance (`IND_IND`).
    inductance: Dynamic<Henry>,
    /// Optional initial current (`IND_IC`).
    initial_current: Option<Dynamic<Ampere>>,
    /// Number of turns (`IND_NT`). Defaults to 1.
    number_of_turns: Dynamic<Dimensionless>,
    /// Parallel multiplier (`IND_M`). Defaults to 1.
    multiplier: Dynamic<Dimensionless>,
    /// Scale factor (`IND_SCALE`). Defaults to 1.
    scale: Dynamic<Dimensionless>,
    /// Optional absolute temperature (`IND_TEMP`).
    temp: Option<Dynamic<Kelvin>>,
    /// Optional delta temperature (`IND_DTEMP`).
    delta_temp: Option<Dynamic<Kelvin>>,
    /// Optional linear coefficient (`IND_TC1`).
    tc1: Option<Dynamic<Dimensionless>>,
    /// Optional quadratic coefficient (`IND_TC2`).
    tc2: Option<Dynamic<Dimensionless>>,
}

impl InductorParams {
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;
    pub const DEFAULT_SCALE: Dimensionless = 1.0;
    pub const DEFAULT_TURNS: Dimensionless = 1.0;
    pub const DEFAULT_INDUCTANCE: Henry = 1.0e-9;

    /// Creates a parameter block with a specific inductance literal/expression.
    pub fn new(value: impl Into<Dynamic<Henry>>) -> Self {
        Self {
            inductance: value.into(),
            ..Self::default()
        }
    }

    /// Returns the stored literal/expression for `IND_IND`.
    pub fn inductance(&self) -> &Dynamic<Henry> {
        &self.inductance
    }

    /// Returns the optional initial current (`IND_IC`).
    pub fn initial_current(&self) -> Option<&Dynamic<Ampere>> {
        self.initial_current.as_ref()
    }

    /// Returns the number of turns (`IND_NT`).
    pub fn number_of_turns(&self) -> &Dynamic<Dimensionless> {
        &self.number_of_turns
    }

    /// Returns the multiplier (`IND_M`).
    pub fn multiplier(&self) -> &Dynamic<Dimensionless> {
        &self.multiplier
    }

    /// Returns the scale factor (`IND_SCALE`).
    pub fn scale(&self) -> &Dynamic<Dimensionless> {
        &self.scale
    }

    /// Optional absolute temperature (`IND_TEMP`).
    pub fn temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.temp.as_ref()
    }

    /// Optional delta temperature (`IND_DTEMP`).
    pub fn delta_temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.delta_temp.as_ref()
    }

    /// Optional `tc1` coefficient (`IND_TC1`).
    pub fn tc1(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc1.as_ref()
    }

    /// Optional `tc2` coefficient (`IND_TC2`).
    pub fn tc2(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc2.as_ref()
    }
}

impl Default for InductorParams {
    fn default() -> Self {
        Self {
            inductance: Self::DEFAULT_INDUCTANCE.into(),
            initial_current: None,
            number_of_turns: Self::DEFAULT_TURNS.into(),
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
            scale: Self::DEFAULT_SCALE.into(),
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
        }
    }
}

impl Inductor {
    /// Creates an inductor bound to `L+`/`L-` with a required inductance.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
        inductance: impl Into<Dynamic<Henry>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            params: InductorParams::new(inductance),
        }
    }

    /// Sets the initial current (`IND_IC`).
    pub fn with_initial_current(&mut self, value: impl Into<Dynamic<Ampere>>) -> &mut Self {
        self.params.initial_current = Some(value.into());
        self
    }

    /// Sets the number of turns (`IND_NT`).
    pub fn with_number_of_turns(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.number_of_turns = value.into();
        self
    }

    /// Sets the multiplier (`IND_M`).
    pub fn with_multiplier(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.multiplier = value.into();
        self
    }

    /// Sets the scale factor (`IND_SCALE`).
    pub fn with_scale(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.scale = value.into();
        self
    }

    /// Sets the absolute temperature (`IND_TEMP`).
    pub fn with_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.params.temp = Some(value.into());
        self
    }

    /// Sets the delta temperature (`IND_DTEMP`).
    pub fn with_delta_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.params.delta_temp = Some(value.into());
        self
    }

    /// Sets the temperature coefficients (`IND_TC1`, `IND_TC2`).
    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl Into<Dynamic<Dimensionless>>,
        tc2: impl Into<Dynamic<Dimensionless>>,
    ) -> &mut Self {
        self.params.tc1 = Some(tc1.into());
        self.params.tc2 = Some(tc2.into());
        self
    }

    /// Instance name (e.g. `L1`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reference to the `L+` node identifier.
    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    /// Reference to the `L-` node identifier.
    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    /// Returns both terminal identifiers.
    pub fn nodes(&self) -> (&NodeIdentifier, &NodeIdentifier) {
        (&self.node_plus, &self.node_minus)
    }

    /// Immutable view of the parameter block.
    pub fn params(&self) -> &InductorParams {
        &self.params
    }

    /// Mutable view of the parameter block.
    pub fn params_mut(&mut self) -> &mut InductorParams {
        &mut self.params
    }

    /// Returns the inductance expression.
    pub fn inductance(&self) -> &Dynamic<Henry> {
        self.params.inductance()
    }

    /// Returns the optional initial current.
    pub fn initial_current(&self) -> Option<&Dynamic<Ampere>> {
        self.params.initial_current()
    }

    /// Returns the number of turns.
    pub fn number_of_turns(&self) -> Dynamic<Dimensionless> {
        self.params.number_of_turns.clone()
    }

    /// Returns the multiplier.
    pub fn multiplier(&self) -> Dynamic<Dimensionless> {
        self.params.multiplier.clone()
    }

    /// Returns the scale factor.
    pub fn scale(&self) -> Dynamic<Dimensionless> {
        self.params.scale.clone()
    }

    /// Returns the optional temperature.
    pub fn temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.params.temp()
    }

    /// Returns the optional delta temperature.
    pub fn delta_temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.params.delta_temp()
    }

    /// Returns the optional `tc1` coefficient.
    pub fn tc1(&self) -> Option<&Dynamic<Dimensionless>> {
        self.params.tc1()
    }

    /// Returns the optional `tc2` coefficient.
    pub fn tc2(&self) -> Option<&Dynamic<Dimensionless>> {
        self.params.tc2()
    }
}

impl Component for Inductor {
    fn name(&self) -> &str {
        self.name()
    }
}

/// Mutual inductance definition linking two linear inductors (`MUT_COEFF`).
#[derive(Debug, Clone)]
pub struct CoupledInductor {
    name: String,
    first: ComponentIdentifier,
    second: ComponentIdentifier,
    /// Coupling coefficient (`MUT_COEFF`).
    coupling: Dynamic<Dimensionless>,
}

impl CoupledInductor {
    /// Creates a coupled inductor entry `Kname first second value`.
    pub fn new(
        name: impl Into<String>,
        first: impl Into<ComponentIdentifier>,
        second: impl Into<ComponentIdentifier>,
        coefficient: impl Into<Dynamic<Dimensionless>>,
    ) -> Self {
        Self {
            name: name.into(),
            first: first.into(),
            second: second.into(),
            coupling: coefficient.into(),
        }
    }

    /// Instance name (e.g. `K1`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reference to the first inductor identifier.
    pub fn first(&self) -> &ComponentIdentifier {
        &self.first
    }

    /// Reference to the second inductor identifier.
    pub fn second(&self) -> &ComponentIdentifier {
        &self.second
    }

    /// Coupling coefficient literal/expression.
    pub fn coupling(&self) -> &Dynamic<Dimensionless> {
        &self.coupling
    }
}

impl Component for CoupledInductor {
    fn name(&self) -> &str {
        self.name()
    }
}

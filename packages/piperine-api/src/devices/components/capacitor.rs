use crate::circuit::netlist::{IntoNodeIdentifier, NodeIdentifier};
use crate::devices::{Component, Dynamic};
use crate::unit::{Dimensionless, Farad, Kelvin, Meter, Volt};

/// Two-terminal capacitor (`C+`, `C-`) exposing the standard `cap` parameter set.
///
/// Each field is stored explicitly with its canonical parameter code (e.g. `CAP_CAP`).
#[derive(Debug, Clone)]
pub struct Capacitor {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    pub params: CapacitorParams,
}

/// Parameter block plus canonical defaults for a linear capacitor device.
#[derive(Debug, Clone)]
pub struct CapacitorParams {
    /// Electrical capacitance (`CAP_CAP`).
    capacitance: Dynamic<Farad>,
    /// Optional initial voltage (`CAP_IC`).
    initial_voltage: Option<Dynamic<Volt>>,
    /// Device width (`CAP_WIDTH`). Defaults to 10 µm.
    width: Dynamic<Meter>,
    /// Device length (`CAP_LENGTH`). Defaults to 10 µm.
    length: Dynamic<Meter>,
    /// Parallel multiplier (`CAP_M`). Defaults to 1.
    multiplier: Dynamic<Dimensionless>,
    /// Geometric scale factor (`CAP_SCALE`). Defaults to 1.
    scale: Dynamic<Dimensionless>,
    /// Optional absolute temperature (`CAP_TEMP`).
    temp: Option<Dynamic<Kelvin>>,
    /// Optional delta temperature (`CAP_DTEMP`).
    delta_temp: Option<Dynamic<Kelvin>>,
    /// Optional linear coefficient (`CAP_TC1`).
    tc1: Option<Dynamic<Dimensionless>>,
    /// Optional quadratic coefficient (`CAP_TC2`).
    tc2: Option<Dynamic<Dimensionless>>,
    /// Optional breakdown voltage limit (`CAP_BV_MAX`).
    breakdown_voltage: Option<Dynamic<Volt>>,
}

impl CapacitorParams {
    pub const DEFAULT_WIDTH: Meter = 10e-6;
    pub const DEFAULT_LENGTH: Meter = 10e-6;
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;
    pub const DEFAULT_SCALE: Dimensionless = 1.0;
    pub const DEFAULT_CAPACITANCE: Farad = 1.0e-12;

    /// Creates a parameter block with a specific capacitance literal/expression.
    pub fn new(value: impl Into<Dynamic<Farad>>) -> Self {
        Self {
            capacitance: value.into(),
            ..Self::default()
        }
    }

    /// Returns the stored literal/expression for `CAP_CAP`.
    pub fn capacitance(&self) -> &Dynamic<Farad> {
        &self.capacitance
    }

    /// Returns the optional initial voltage (`CAP_IC`).
    pub fn initial_voltage(&self) -> Option<&Dynamic<Volt>> {
        self.initial_voltage.as_ref()
    }

    /// Returns the device width (`CAP_WIDTH`).
    pub fn width(&self) -> &Dynamic<Meter> {
        &self.width
    }

    /// Returns the device length (`CAP_LENGTH`).
    pub fn length(&self) -> &Dynamic<Meter> {
        &self.length
    }

    /// Returns the multiplier (`CAP_M`).
    pub fn multiplier(&self) -> &Dynamic<Dimensionless> {
        &self.multiplier
    }

    /// Returns the scale factor (`CAP_SCALE`).
    pub fn scale(&self) -> &Dynamic<Dimensionless> {
        &self.scale
    }

    /// Optional absolute temperature (`CAP_TEMP`).
    pub fn temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.temp.as_ref()
    }

    /// Optional delta temperature (`CAP_DTEMP`).
    pub fn delta_temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.delta_temp.as_ref()
    }

    /// Optional first-order coefficient (`CAP_TC1`).
    pub fn tc1(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc1.as_ref()
    }

    /// Optional second-order coefficient (`CAP_TC2`).
    pub fn tc2(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc2.as_ref()
    }

    /// Optional breakdown voltage limit (`CAP_BV_MAX`).
    pub fn breakdown_voltage(&self) -> Option<&Dynamic<Volt>> {
        self.breakdown_voltage.as_ref()
    }
}

impl Default for CapacitorParams {
    fn default() -> Self {
        Self {
            capacitance: Self::DEFAULT_CAPACITANCE.into(),
            initial_voltage: None,
            width: Self::DEFAULT_WIDTH.into(),
            length: Self::DEFAULT_LENGTH.into(),
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
            scale: Self::DEFAULT_SCALE.into(),
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            breakdown_voltage: None,
        }
    }
}

impl Capacitor {
    /// Creates a capacitor bound to `C+`/`C-` with a required capacitance literal/expression.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
        capacitance: impl Into<Dynamic<Farad>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            params: CapacitorParams::new(capacitance),
        }
    }

    /// Sets the initial voltage (`CAP_IC`).
    pub fn with_initial_voltage(&mut self, value: impl Into<Dynamic<Volt>>) -> &mut Self {
        self.params.initial_voltage = Some(value.into());
        self
    }

    /// Overrides the physical dimensions (`CAP_WIDTH`, `CAP_LENGTH`).
    pub fn with_dimensions(
        &mut self,
        width: impl Into<Dynamic<Meter>>,
        length: impl Into<Dynamic<Meter>>,
    ) -> &mut Self {
        self.params.width = width.into();
        self.params.length = length.into();
        self
    }

    /// Sets the multiplier (`CAP_M`).
    pub fn with_multiplier(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.multiplier = value.into();
        self
    }

    /// Sets the scale factor (`CAP_SCALE`).
    pub fn with_scale(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.scale = value.into();
        self
    }

    /// Sets the absolute temperature (`CAP_TEMP`).
    pub fn with_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.params.temp = Some(value.into());
        self
    }

    /// Sets the delta temperature (`CAP_DTEMP`).
    pub fn with_delta_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.params.delta_temp = Some(value.into());
        self
    }

    /// Sets the linear/quadratic coefficients (`CAP_TC1`, `CAP_TC2`).
    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl Into<Dynamic<Dimensionless>>,
        tc2: impl Into<Dynamic<Dimensionless>>,
    ) -> &mut Self {
        self.params.tc1 = Some(tc1.into());
        self.params.tc2 = Some(tc2.into());
        self
    }

    /// Sets the optional breakdown limit (`CAP_BV_MAX`).
    pub fn with_breakdown_voltage(&mut self, value: impl Into<Dynamic<Volt>>) -> &mut Self {
        self.params.breakdown_voltage = Some(value.into());
        self
    }

    /// Instance name (e.g. `C1`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reference to the `C+` terminal identifier.
    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    /// Reference to the `C-` terminal identifier.
    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    /// Returns both terminal identifiers.
    pub fn nodes(&self) -> (&NodeIdentifier, &NodeIdentifier) {
        (&self.node_plus, &self.node_minus)
    }

    /// Immutable view of the parameter block.
    pub fn params(&self) -> &CapacitorParams {
        &self.params
    }

    /// Mutable view of the parameter block.
    pub fn params_mut(&mut self) -> &mut CapacitorParams {
        &mut self.params
    }

    /// Returns the capacitance literal/expression.
    pub fn capacitance(&self) -> &Dynamic<Farad> {
        self.params.capacitance()
    }

    /// Returns the optional initial voltage.
    pub fn initial_voltage(&self) -> Option<&Dynamic<Volt>> {
        self.params.initial_voltage()
    }

    /// Returns the width.
    pub fn width(&self) -> Dynamic<Meter> {
        self.params.width.clone()
    }

    /// Returns the length.
    pub fn length(&self) -> Dynamic<Meter> {
        self.params.length.clone()
    }

    /// Returns the multiplier.
    pub fn multiplier(&self) -> Dynamic<Dimensionless> {
        self.params.multiplier.clone()
    }

    /// Returns the scale factor.
    pub fn scale(&self) -> Dynamic<Dimensionless> {
        self.params.scale.clone()
    }

    /// Returns the optional absolute temperature.
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

    /// Returns the optional breakdown voltage.
    pub fn breakdown_voltage(&self) -> Option<&Dynamic<Volt>> {
        self.params.breakdown_voltage()
    }
}

impl Component for Capacitor {
    fn name(&self) -> &str {
        self.name()
    }
}

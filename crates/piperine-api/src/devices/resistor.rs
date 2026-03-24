use crate::devices::Component;
use crate::models::resistor::ResistorModel;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Dimensionless, Kelvin, Meter, Ohm};
use std::sync::Arc;

/// Two-terminal linear resistor (`R+`, `R-`) with the standard `res` parameter set.
///
/// The struct stores strongly typed fields instead of raw key/value maps; each field is
/// annotated with the canonical parameter code (e.g. `RES_RESIST`).
#[derive(Debug, Clone)]
pub struct Resistor {
    name: String,
    node_plus: Node,
    node_minus: Node,
    model: Arc<dyn ResistorModel + Send + Sync>,
    /// Electrical resistance (`RES_RESIST`).
    resistance: Dynamic<Ohm>,
    /// Optional AC-only resistance (`RES_ACRESIST`).
    ac: Option<Dynamic<Ohm>>,
    /// Physical length (`RES_LENGTH`). Defaults to 10 µm.
    length: Dynamic<Meter>,
    /// Physical width (`RES_WIDTH`). Defaults to 10 µm.
    width: Dynamic<Meter>,
    /// Geometric scaling factor (`RES_SCALE`). Defaults to 1.
    scale: Dynamic<Dimensionless>,
    /// Instance multiplier (`RES_M`). Defaults to 1.
    multiplier: Dynamic<Dimensionless>,
    /// Optional absolute operating temperature (`RES_TEMP`).
    temp: Option<Dynamic<Kelvin>>,
    /// Optional relative temperature offset (`RES_DTEMP`).
    delta_temp: Option<Dynamic<Kelvin>>,
    /// Optional first-order temperature coefficient (`RES_TC1`).
    tc1: Option<Dynamic<Dimensionless>>,
    /// Optional second-order temperature coefficient (`RES_TC2`).
    tc2: Option<Dynamic<Dimensionless>>,
    /// Optional exponential temperature coefficient (`RES_TCE`).
    tce: Option<Dynamic<Dimensionless>>,
    /// Noise enable flag (`RES_NOISY`). Defaults to true.
    noisy: bool,
}

impl Resistor {
    pub const SYMBOL : &str = "R";
    pub const DEFAULT_WIDTH: Meter = 10e-6;
    pub const DEFAULT_LENGTH: Meter = 10e-6;
    pub const DEFAULT_SCALE: Dimensionless = 1.0;
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;
    pub const DEFAULT_NOISY: bool = true;
    pub const DEFAULT_RESISTANCE: Ohm = 1.0;

    /// Creates a new resistor bound to nodes `R+`/`R-` with a required resistance.
    ///
    /// * `name` is the instance identifier (e.g. `R1`)
    /// * `node_plus` corresponds to the first terminal (`R+`)
    /// * `node_minus` corresponds to the second terminal (`R-`)
    /// * `resistance` feeds parameter code `RES_RESIST`
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        resistance: impl Into<Dynamic<Ohm>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            model: Arc::clone(&crate::models::resistor::DEFAULT),
            resistance: resistance.into(),
            ac: None,
            length: Self::DEFAULT_LENGTH.into(),
            width: Self::DEFAULT_WIDTH.into(),
            scale: Self::DEFAULT_SCALE.into(),
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            tce: None,
            noisy: Self::DEFAULT_NOISY,
        }
    }

    /// Sets the optional AC-only resistance (code `RES_ACRESIST`).
    pub fn with_ac(&mut self, value: impl Into<Dynamic<Ohm>>) -> &mut Self {
        self.ac = Some(value.into());
        self
    }

    /// Overrides geometry keywords `w` (`RES_WIDTH`) and `l` (`RES_LENGTH`).
    pub fn with_dimensions(
        &mut self,
        width: impl Into<Dynamic<Meter>>,
        length: impl Into<Dynamic<Meter>>,
    ) -> &mut Self {
        self.width = width.into();
        self.length = length.into();
        self
    }

    /// Sets the scale factor (`RES_SCALE`).
    pub fn with_scale(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.scale = value.into();
        self
    }

    /// Sets the multiplicity (`RES_M`).
    pub fn with_multiplier(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.multiplier = value.into();
        self
    }

    /// Sets the absolute temperature (`RES_TEMP`).
    pub fn with_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.temp = Some(value.into());
        self
    }

    /// Sets the relative temperature offset (`RES_DTEMP`).
    pub fn with_delta_temp(&mut self, value: impl Into<Dynamic<Kelvin>>) -> &mut Self {
        self.delta_temp = Some(value.into());
        self
    }

    /// Sets the linear/quadratic temperature coefficients (`RES_TC1`, `RES_TC2`).
    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl Into<Dynamic<Dimensionless>>,
        tc2: impl Into<Dynamic<Dimensionless>>,
    ) -> &mut Self {
        self.tc1 = Some(tc1.into());
        self.tc2 = Some(tc2.into());
        self
    }

    /// Sets the exponential temperature coefficient (`RES_TCE`).
    pub fn with_exponential_temperature_coefficient(
        &mut self,
        value: impl Into<Dynamic<Dimensionless>>,
    ) -> &mut Self {
        self.tce = Some(value.into());
        self
    }

    /// Toggles the noise flag (`RES_NOISY`).
    pub fn with_noise(&mut self, enable: bool) -> &mut Self {
        self.noisy = enable;
        self
    }

    /// Instance name (e.g. `R1`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reference to the `R+` terminal identifier.
    pub fn node_plus(&self) -> &Node {
        &self.node_plus
    }

    /// Reference to the `R-` terminal identifier.
    pub fn node_minus(&self) -> &Node {
        &self.node_minus
    }

    /// Convenience accessor returning `(R+, R-)`.
    pub fn nodes(&self) -> (&Node, &Node) {
        (&self.node_plus, &self.node_minus)
    }

    /// Returns the literal/expression backing the `resistance` keyword.
    pub fn resistance(&self) -> &Dynamic<Ohm> {
        &self.resistance
    }

    /// Returns the optional AC-specific resistance.
    pub fn ac(&self) -> Option<&Dynamic<Ohm>> {
        self.ac.as_ref()
    }

    /// Returns the current `width`.
    pub fn width(&self) -> Dynamic<Meter> {
        self.width.clone()
    }

    /// Returns the current `length`.
    pub fn length(&self) -> Dynamic<Meter> {
        self.length.clone()
    }

    /// Returns the current `scale` factor.
    pub fn scale(&self) -> Dynamic<Dimensionless> {
        self.scale.clone()
    }

    /// Returns the current multiplicative factor `m`.
    pub fn multiplier(&self) -> Dynamic<Dimensionless> {
        self.multiplier.clone()
    }

    /// Returns the optional explicit temperature.
    pub fn temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.temp.as_ref()
    }

    /// Returns the optional delta temperature.
    pub fn delta_temp(&self) -> Option<&Dynamic<Kelvin>> {
        self.delta_temp.as_ref()
    }

    /// Returns the optional `tc1` coefficient.
    pub fn tc1(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc1.as_ref()
    }

    /// Returns the optional `tc2` coefficient.
    pub fn tc2(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tc2.as_ref()
    }

    /// Returns the optional `tce` coefficient.
    pub fn tce(&self) -> Option<&Dynamic<Dimensionless>> {
        self.tce.as_ref()
    }

    /// Returns whether the resistor generates noise.
    pub fn is_noisy(&self) -> bool {
        self.noisy
    }
}

impl Component for Resistor {}

impl SpiceElement for Resistor {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        Some(Arc::clone(&self.model) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for Resistor {
    fn into_spice(&self) -> String {
        let mut spice = format!(
            "{} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus()
        );

        // Required parameter: resistance
        spice.push_str(&format!(" RES={}", self.resistance()));

        // Optional parameters
        if let Some(ac) = self.ac() {
            spice.push_str(&format!(" ACRES={}", ac));
        }

        if let Some(temp) = self.temp() {
            spice.push_str(&format!(" TEMP={}", temp));
        }

        if let Some(delta_temp) = self.delta_temp() {
            spice.push_str(&format!(" DTEMP={}", delta_temp));
        }

        if let Some(tc1) = self.tc1() {
            spice.push_str(&format!(" TC1={}", tc1));
        }
        if let Some(tc2) = self.tc2() {
            spice.push_str(&format!(" TC2={}", tc2));
        }
        if let Some(tce) = self.tce() {
            spice.push_str(&format!(" TCE={}", tce));
        }
        if !self.noisy {
            spice.push_str(" NOISY=0");
        }

        spice
    }
}
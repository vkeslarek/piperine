use crate::devices::VoltageSwitch;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Ampere, Ohm, UnitExt, Volt};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait VoltageSwitchModel: Model<ComponentType = VoltageSwitch> + SpiceModel + Debug {}

pub static DEFAULT_SW: LazyLock<Arc<dyn VoltageSwitchModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", SwitchType::Sw)));

pub static DEFAULT_CSW: LazyLock<Arc<dyn VoltageSwitchModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", SwitchType::Csw)));

/// Switch model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchType {
    /// SW: Voltage-controlled switch.
    Sw,
    /// CSW: Current-controlled switch.
    Csw,
}

/// Switch model parameters (`.MODEL name SW/CSW`).
///
/// All parameters from ngspice manual §3.3.16, p. 91.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub switch_type: SwitchType,

    /// VT: Threshold voltage (V). Default: 0.0. (SW only)
    pub vt: Volt,
    /// IT: Threshold current (A). Default: 0.0. (CSW only)
    pub it: Ampere,
    /// VH: Hysteresis voltage (V). Default: 0.0. (SW only)
    pub vh: Volt,
    /// IH: Hysteresis current (A). Default: 0.0. (CSW only)
    pub ih: Ampere,
    /// RON: On resistance (Ω). Default: 1.0.
    pub ron: Ohm,
    /// ROFF: Off resistance (Ω). Default: 1.0e12.
    pub roff: Ohm,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, switch_type: SwitchType) -> Self {
        Self {
            name: name.into(),
            switch_type,
            vt: 0.0.V(),
            it: 0.0.A(),
            vh: 0.0.V(),
            ih: 0.0.A(),
            ron: 1.0.Ohms(),
            roff: 1.0e12.Ohms(),
        }
    }

    pub fn name(&self) -> &String { &self.name }
    pub fn switch_type(&self) -> SwitchType { self.switch_type }

    pub fn with_vt(&mut self, vt: Volt) -> &mut Self { self.vt = vt; self }
    pub fn with_it(&mut self, it: Ampere) -> &mut Self { self.it = it; self }
    pub fn with_vh(&mut self, vh: Volt) -> &mut Self { self.vh = vh; self }
    pub fn with_ih(&mut self, ih: Ampere) -> &mut Self { self.ih = ih; self }
    pub fn with_ron(&mut self, ron: Ohm) -> &mut Self { self.ron = ron; self }
    pub fn with_roff(&mut self, roff: Ohm) -> &mut Self { self.roff = roff; self }
}

impl Model for DefaultModel {
    type ComponentType = VoltageSwitch;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let spice_type = match self.switch_type {
            SwitchType::Sw => "SW",
            SwitchType::Csw => "CSW",
        };
        format!(".MODEL {} {}", self.name, spice_type)
    }
}

impl VoltageSwitchModel for DefaultModel {}

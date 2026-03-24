use crate::devices::Resistor;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Celsius, Dimensionless, Meter, Ohm, UnitExt, Volt};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait ResistorModel: Model<ComponentType = Resistor> + SpiceModel + Debug {}

/// Pre-instantiated default resistor model.
pub static DEFAULT: LazyLock<Arc<dyn ResistorModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub tnom: Celsius,
    pub tc1: Dimensionless,
    pub tc2: Dimensionless,
    pub tce: Dimensionless,
    pub sheet_res: Ohm,
    pub def_width: Meter,
    pub def_length: Meter,
    pub narrow: Meter,
    pub short: Meter,
    pub bv_max: Option<Volt>,
    pub lf: Dimensionless,
    pub wf: Dimensionless,
    pub ef: Dimensionless,
    pub kf: Dimensionless,
    pub af: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tnom: 27.0.deg_C(),
            tc1: 0.0.inv_C(),
            tc2: 0.0.inv_C2(),
            tce: 0.0,
            sheet_res: 0.0.Ohms(),
            def_width: 10.0.um(),
            def_length: 10.0.um(),
            narrow: 0.0.m(),
            short: 0.0.m(),
            bv_max: None,
            lf: 1.0,
            wf: 1.0,
            ef: 1.0,
            kf: 1.0,
            af: 1.0,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn tnom(&self) -> &Celsius {
        &self.tnom
    }

    pub fn tc1(&self) -> &Dimensionless {
        &self.tc1
    }

    pub fn tc2(&self) -> &Dimensionless {
        &self.tc2
    }

    pub fn tce(&self) -> &Dimensionless {
        &self.tce
    }

    pub fn sheet_res(&self) -> &Ohm {
        &self.sheet_res
    }

    pub fn def_width(&self) -> &Meter {
        &self.def_width
    }

    pub fn def_length(&self) -> &Meter {
        &self.def_length
    }

    pub fn narrow(&self) -> &Meter {
        &self.narrow
    }

    pub fn short(&self) -> &Meter {
        &self.short
    }

    pub fn bv_max(&self) -> Option<&Volt> {
        self.bv_max.as_ref()
    }

    pub fn lf(&self) -> &Dimensionless {
        &self.lf
    }

    pub fn wf(&self) -> &Dimensionless {
        &self.wf
    }

    pub fn ef(&self) -> &Dimensionless {
        &self.ef
    }

    pub fn kf(&self) -> &Dimensionless {
        &self.kf
    }

    pub fn af(&self) -> &Dimensionless {
        &self.af
    }

    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self {
        self.tnom = tnom;
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: Dimensionless,
        tc2: Dimensionless,
    ) -> &mut Self {
        self.tc1 = tc1;
        self.tc2 = tc2;
        self
    }

    pub fn with_exponential_temperature_coefficient(&mut self, tce: Dimensionless) -> &mut Self {
        self.tce = tce;
        self
    }

    pub fn with_sheet_resistivity(&mut self, sheet_res: Ohm) -> &mut Self {
        self.sheet_res = sheet_res;
        self
    }

    pub fn with_default_width(&mut self, def_width: Meter) -> &mut Self {
        self.def_width = def_width;
        self
    }

    pub fn with_default_length(&mut self, def_length: Meter) -> &mut Self {
        self.def_length = def_length;
        self
    }

    pub fn with_narrow(&mut self, narrow: Meter) -> &mut Self {
        self.narrow = narrow;
        self
    }

    pub fn with_short(&mut self, short: Meter) -> &mut Self {
        self.short = short;
        self
    }

    pub fn with_breakdown_voltage(&mut self, bv_max: Volt) -> &mut Self {
        self.bv_max = Some(bv_max);
        self
    }

    pub fn with_noise_parameters(
        &mut self,
        lf: Dimensionless,
        wf: Dimensionless,
        ef: Dimensionless,
    ) -> &mut Self {
        self.lf = lf;
        self.wf = wf;
        self.ef = ef;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Resistor;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        format!(".MODEL {} R", self.name)
    }
}

impl ResistorModel for DefaultModel {}

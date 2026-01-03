use crate::component::res::Resistor;
use crate::math::unit::MetersExt;
use crate::model::Model;
use crate::state::CircuitStates;

pub type ResistorModel = dyn Model<ComponentType = Resistor> + 'static;

pub struct ResistorIdealModel {
    pub name: String,
}

impl ResistorIdealModel {
    pub fn new(name: String) -> Self {
        ResistorIdealModel { name }
    }
}

impl Model for ResistorIdealModel {
    type ComponentType = Resistor;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(
        &self,
        component: &mut Self::ComponentType,
        _: &CircuitStates,
    ) -> crate::error::Result<()> {
        let mut res = component.resistance.unwrap_or(1.0.um());
        component.conductance = component.m / (res * component.scale);
        Ok(())
    }
}

pub struct ResistorCompleteModelParameters {
    pub name: String,
    // Temperature parameters
    pub tnom: Option<f64>,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub tce: Option<f64>,
    // Geometry parameters
    pub sheet_res: Option<f64>,
    pub def_width: Option<f64>,
    pub def_length: Option<f64>,
    pub narrow: Option<f64>,
    pub short: Option<f64>,
    // Noise and Limits
    pub bv_max: Option<f64>,
    pub lf: Option<f64>,
    pub wf: Option<f64>,
    pub ef: Option<f64>,
}

impl Default for ResistorCompleteModelParameters {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            tnom: None,
            tc1: None,
            tc2: None,
            tce: None,
            sheet_res: None,
            def_width: None,
            def_length: None,
            narrow: None,
            short: None,
            bv_max: None,
            lf: None,
            wf: None,
            ef: None,
        }
    }
}

pub struct ResistorCompleteModel {
    pub name: String,
    // Temperature parameters
    pub tnom: f64,
    pub tc1: f64,
    pub tc2: f64,
    pub tce: f64,
    // Geometry parameters
    pub sheet_res: f64,
    pub def_width: f64,
    pub def_length: f64,
    pub narrow: f64,
    pub short: f64,
    // Noise and Limits
    pub bv_max: f64,
    pub lf: f64,
    pub wf: f64,
    pub ef: f64,
}

impl ResistorCompleteModel {
    pub fn new(parameters: ResistorCompleteModelParameters) -> Self {
        Self {
            name: parameters.name,
            tnom: parameters.tnom.unwrap_or(300.15),
            sheet_res: parameters.sheet_res.unwrap_or(0.0),
            def_width: parameters.def_width.unwrap_or(10e-6),
            def_length: parameters.def_length.unwrap_or(10e-6),
            tc1: parameters.tc1.unwrap_or(0.0),
            tc2: parameters.tc2.unwrap_or(0.0),
            tce: parameters.tce.unwrap_or(0.0),
            narrow: parameters.narrow.unwrap_or(0.0),
            short: parameters.short.unwrap_or(0.0),
            bv_max: parameters.bv_max.unwrap_or(100.0),
            lf: parameters.lf.unwrap_or(1.0),
            wf: parameters.wf.unwrap_or(1.0),
            ef: parameters.ef.unwrap_or(1.0),
        }
    }
}

impl Model for ResistorCompleteModel {
    type ComponentType = Resistor;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(
        &self,
        component: &mut Resistor,
        _circuit_states: &CircuitStates,
    ) -> crate::error::Result<()> {
        // Translation of RESupdate_conduct logic
        component.resistance;

        // 1. Geometry-based resistance calculation
        let res = component.resistance.unwrap_or_else(|| {
            let effective_length = component.length - 2.0 * self.short;
            let effective_width = component.width - 2.0 * self.narrow;

            if effective_length > 0.0 && effective_width > 0.0 && self.sheet_res > 0.0 {
                (effective_length / effective_width) * self.sheet_res
            } else {
                // Warning: resistance too low, set to 1 mOhm
                1e-03
            }
        });

        // 2. Temperature Factor
        // In a real simulator, fetch circuit temperature from circuit_states or context
        let current_temp = component.temp.unwrap_or(300.15);
        let difference = (current_temp + component.dtemp) - self.tnom;

        // Overrides: instance parameters override model parameters
        let tc1 = component.tc1.unwrap_or(self.tc1);
        let tc2 = component.tc2.unwrap_or(self.tc2);
        let tce = component.tce.unwrap_or(self.tce);

        let factor = if tce != 0.0 {
            1.01f64.powf(tce * difference)
        } else {
            (tc2 * difference + tc1) * difference + 1.0
        };

        // 3. Final Conductance: G = m / (R * factor * scale)
        component.conductance = component.m / (res * factor * component.scale);

        Ok(())
    }
}

use crate::component::res::Resistor;
use crate::math::unit::{
    DeltaKelvin, Kelvin, Length, LinearTemperatureCoefficient, QuadraticTemperatureCoefficient,
    Ratio, SheetResistance, Temperature, TemperatureInterval, UnitExt, Voltage,
};
use crate::model::Model;

pub type ResistorModel = dyn Model<ComponentType = Resistor> + 'static;

#[derive(Debug)]
pub struct ResistorIdealModel {}

impl ResistorIdealModel {
    pub fn new() -> Self {
        ResistorIdealModel {}
    }
}

impl Model for ResistorIdealModel {
    type ComponentType = Resistor;

    fn update(&self, component: &mut Self::ComponentType) -> crate::error::Result<()> {
        let mut res = component.resistance.unwrap_or(1.0.uOhms());
        component.conductance = component.multiplier / (res * component.scale);
        Ok(())
    }
}

pub struct ResistorCompleteModelParameters {
    pub name: String,
    // Temperature parameters
    pub tnom: Option<Temperature>,
    pub tc1: Option<LinearTemperatureCoefficient>,
    pub tc2: Option<QuadraticTemperatureCoefficient>,
    pub tce: Option<Ratio>,
    // Geometry parameters
    pub sheet_res: Option<SheetResistance>,
    pub def_width: Option<Length>,
    pub def_length: Option<Length>,
    pub narrow: Option<Length>,
    pub short: Option<Length>,
    // Noise and Limits
    pub bv_max: Option<Voltage>,
    pub lf: Option<Ratio>,
    pub wf: Option<Ratio>,
    pub ef: Option<Ratio>,
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

#[derive(Debug)]
pub struct ResistorCompleteModel {
    pub name: String,
    // Temperature parameters
    pub tnom: Temperature,
    pub tc1: LinearTemperatureCoefficient,
    pub tc2: QuadraticTemperatureCoefficient,
    pub tce: Ratio,
    // Geometry parameters
    pub sheet_res: SheetResistance,
    pub def_width: Length,
    pub def_length: Length,
    pub narrow: Length,
    pub short: Length,
    // Noise and Limits
    pub bv_max: Voltage,
    pub lf: Ratio,
    pub wf: Ratio,
    pub ef: Ratio,
}

impl ResistorCompleteModel {
    pub fn new(parameters: ResistorCompleteModelParameters) -> Self {
        Self {
            name: parameters.name,
            tnom: parameters.tnom.unwrap_or(27.0.degC()),
            sheet_res: parameters.sheet_res.unwrap_or(0.0.OhmsPerMeter2()),
            def_width: parameters.def_width.unwrap_or(10.0.um()),
            def_length: parameters.def_length.unwrap_or(10.0.um()),
            tc1: parameters.tc1.unwrap_or(0.0.OhmsPerC()),
            tc2: parameters.tc2.unwrap_or(0.0.OhmsPerC2()),
            tce: parameters.tce.unwrap_or(0.0.ratio()),
            narrow: parameters.narrow.unwrap_or(0.0.m()),
            short: parameters.short.unwrap_or(0.0.m()),
            bv_max: parameters.bv_max.unwrap_or(100.0.V()),
            lf: parameters.lf.unwrap_or(1.0.ratio()),
            wf: parameters.wf.unwrap_or(1.0.ratio()),
            ef: parameters.ef.unwrap_or(1.0.ratio()),
        }
    }
}

impl Model for ResistorCompleteModel {
    type ComponentType = Resistor;

    fn update(&self, component: &mut Resistor) -> crate::error::Result<()> {
        let r_nom = match component.resistance {
            Some(r) => r,
            None => {
                let effective_length =
                    component.length.unwrap_or(self.def_length) - 2.0 * self.short;
                let effective_width = component.width.unwrap_or(self.def_width) - 2.0 * self.narrow;

                // Physics: R = Rsh * (L / W)
                if self.sheet_res > 0.0.OhmsPerMeter2() {
                    if effective_width > 0.0.m() {
                        self.sheet_res * effective_length * effective_width
                    } else {
                        1.0e-3.Ohms()
                    }
                } else {
                    1.0e3.Ohms()
                }
            }
        };

        // 2. Temperature Factor Calculation
        let current_temp: Temperature = component.temp.unwrap_or(self.tnom);
        let delta_t = TemperatureInterval::new::<DeltaKelvin>(
            (current_temp + component.dtemp.unwrap_or(0.0.delta_C())).get::<Kelvin>()
                - self.tnom.get::<Kelvin>(),
        );

        // Resolve coefficients (Priority: Component > Model > Default 0.0)
        let tc1 = component.tc1.unwrap_or(self.tc1);
        let tc2 = component.tc2.unwrap_or(self.tc2);
        let tce = component.tce.unwrap_or(self.tce);

        // factor = R(T)/R(Tnom)
        let factor: Ratio = if tce != 0.0.ratio() {
            // Exponential: 1.01^(tce * delta_t)
            // Note: tce * delta_t must be dimensionless for powf
            let exponent = tce * delta_t;
            1.01f64.powf(exponent.value).ratio()
        } else {
            // Polynomial: (tc2 * dT^2 + tc1 * dT + 1)
            // uom handles the addition of tc2*dT^2 and tc1*dT correctly!
            (tc2 * delta_t * delta_t) + (tc1 * delta_t) + 1.0.ratio()
        };

        // 3. Final Conductance: G = (multiplier) / (R_nom * factor * scale)
        // multiplier and scale are Ratios, r_nom is Resistance, factor is Ratio
        component.conductance = component.multiplier / (r_nom * factor * component.scale);

        Ok(())
    }
}

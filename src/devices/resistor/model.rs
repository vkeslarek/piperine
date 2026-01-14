use crate::devices::Model;
use crate::devices::resistor::Resistor;
use crate::math::unit::{Dimensionless, Kelvin, Meter, Ohm, UnitExt, Volt};
use crate::solver::Context;
use crate::util::AsAny;
use std::any::Any;

#[derive(Debug)]
pub struct ResistorModel {
    pub name: String,
    // Temperature parameters
    pub tnom: Kelvin,
    pub tc1: Dimensionless,
    pub tc2: Dimensionless,
    pub tce: Dimensionless,
    // Geometry parameters
    pub sheet_res: Ohm,
    pub def_width: Meter,
    pub def_length: Meter,
    pub narrow: Meter,
    pub short: Meter,
    // Noise and Limits
    pub bv_max: Option<Volt>,
    pub lf: Dimensionless,
    pub wf: Dimensionless,
    pub ef: Dimensionless,
    pub kf: Dimensionless,
    pub af: Dimensionless,
}

impl Default for ResistorModel {
    fn default() -> Self {
        Self {
            name: "DefaultResistorModel".to_string(),
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
}

impl ResistorModel {
    pub fn with_tnom(&mut self, tnom: Kelvin) -> &mut Self {
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

    pub fn update_conductance(&self, component: &mut Resistor, context: &Context) {
        let r_nom = match component.resistance {
            Some(r) => r,
            None => {
                let effective_length =
                    component.length.unwrap_or(self.def_length) - 2.0 * self.short;
                let effective_width = component.width.unwrap_or(self.def_width) - 2.0 * self.narrow;

                // Physics: R = Rsh * (L / W)
                if self.sheet_res > 0.0.Ohms() {
                    if effective_width > 0.0.m() {
                        self.sheet_res * effective_length * effective_width
                    } else {
                        context.min_res
                    }
                } else {
                    context.min_res
                }
            }
        };

        // 2. Temperature Factor Calculation
        let current_temp: Kelvin = component.temp.unwrap_or(self.tnom);
        let delta_t = current_temp + component.delta_temp.unwrap_or(0.0.K()) - self.tnom;

        // Resolve coefficients (Priority: Component > Model > Default 0.0)
        let tc1 = component.tc1.unwrap_or(self.tc1);
        let tc2 = component.tc2.unwrap_or(self.tc2);
        let tce = component.tce.unwrap_or(self.tce);

        let exp_fact = 1.01f64.powf(tce * delta_t);
        let poly_fact = (tc2 * delta_t * delta_t) + (tc1 * delta_t) + 1.0;
        let factor = exp_fact * poly_fact;

        // 3. Final Conductance: G = (multiplier) / (R_nom * factor * scale)
        // multiplier and scale are Ratios, r_nom is Resistance, factor is Ratio
        component.conductance =
            component.multiplier.unwrap_or(1.0) / (r_nom * factor * component.scale.unwrap_or(1.0));
    }
}

impl AsAny for ResistorModel {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Model for ResistorModel {
    type ComponentType = Resistor;
}

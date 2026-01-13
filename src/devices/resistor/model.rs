use crate::devices::Model;
use crate::devices::resistor::Resistor;
use crate::math::unit::{
    DeltaKelvin, Kelvin, Length, LinearTemperatureCoefficient, QuadraticTemperatureCoefficient,
    Ratio, SheetResistance, Temperature, TemperatureInterval, UnitExt, Voltage,
};
use crate::solver::Context;
use crate::util::AsAny;
use std::any::Any;

#[derive(Debug)]
pub struct ResistorModel {
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
    pub bv_max: Option<Voltage>,
    pub lf: Ratio,
    pub wf: Ratio,
    pub ef: Ratio,
    pub kf: Ratio,
    pub af: Ratio,
}

impl Default for ResistorModel {
    fn default() -> Self {
        Self {
            name: "DefaultResistorModel".to_string(),
            tnom: 27.0.deg_C(),
            tc1: 0.0.inv_C(),
            tc2: 0.0.inv_C2(),
            tce: 0.0.ratio(),
            sheet_res: 0.0.Ohms_per_meter2(),
            def_width: 10.0.um(),
            def_length: 10.0.um(),
            narrow: 0.0.m(),
            short: 0.0.m(),
            bv_max: None,
            lf: 1.0.ratio(),
            wf: 1.0.ratio(),
            ef: 1.0.ratio(),
            kf: 1.0.ratio(),
            af: 1.0.ratio(),
        }
    }
}

impl ResistorModel {
    pub fn with_tnom(&mut self, tnom: Temperature) -> &mut Self {
        self.tnom = tnom;
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: LinearTemperatureCoefficient,
        tc2: QuadraticTemperatureCoefficient,
    ) -> &mut Self {
        self.tc1 = tc1;
        self.tc2 = tc2;
        self
    }

    pub fn with_exponential_temperature_coefficient(&mut self, tce: Ratio) -> &mut Self {
        self.tce = tce;
        self
    }

    pub fn with_sheet_resistivity(&mut self, sheet_res: SheetResistance) -> &mut Self {
        self.sheet_res = sheet_res;
        self
    }

    pub fn with_default_width(&mut self, def_width: Length) -> &mut Self {
        self.def_width = def_width;
        self
    }

    pub fn with_default_length(&mut self, def_length: Length) -> &mut Self {
        self.def_length = def_length;
        self
    }

    pub fn with_narrow(&mut self, narrow: Length) -> &mut Self {
        self.narrow = narrow;
        self
    }

    pub fn with_short(&mut self, short: Length) -> &mut Self {
        self.short = short;
        self
    }

    pub fn with_breakdown_voltage(&mut self, bv_max: Voltage) -> &mut Self {
        self.bv_max = Some(bv_max);
        self
    }

    pub fn with_noise_parameters(&mut self, lf: Ratio, wf: Ratio, ef: Ratio) -> &mut Self {
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
                if self.sheet_res > 0.0.Ohms_per_meter2() {
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
        let current_temp: Temperature = component.temp.unwrap_or(self.tnom);
        let delta_t = TemperatureInterval::new::<DeltaKelvin>(
            (current_temp + component.delta_temp.unwrap_or(0.0.delta_C())).get::<Kelvin>()
                - self.tnom.get::<Kelvin>(),
        );

        // Resolve coefficients (Priority: Component > Model > Default 0.0)
        let tc1 = component.tc1.unwrap_or(self.tc1);
        let tc2 = component.tc2.unwrap_or(self.tc2);
        let tce = component.tce.unwrap_or(self.tce);

        let exp_fact = 1.01f64.powf((tce * delta_t).value).ratio();
        let poly_fact = (tc2 * delta_t * delta_t) + (tc1 * delta_t) + 1.0.ratio();
        let factor: Ratio = exp_fact * poly_fact;

        // 3. Final Conductance: G = (multiplier) / (R_nom * factor * scale)
        // multiplier and scale are Ratios, r_nom is Resistance, factor is Ratio
        component.conductance = component.multiplier.unwrap_or(1.0.ratio())
            / (r_nom * factor * component.scale.unwrap_or(1.0.ratio()));
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

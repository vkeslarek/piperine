use crate::component::dio::Diode;
use crate::model::Model;

pub type DiodeModel = dyn Model<ComponentType = Diode> + 'static;

pub struct DiodeShockleyModel {
    pub vt: f64, // Thermal voltage
}

impl DiodeShockleyModel {
    pub fn new() -> Self {
        Self { vt: 0.02585 } // ~25.85mV at 300K
    }

    pub fn calculate_current(&self, vd: f64, is: f64, n: f64) -> f64 {
        let nvt = n * self.vt;
        // Limit the exponential input to prevent floating point overflow
        let limit = 80.0;
        let arg = (vd / nvt).min(limit);
        is * (arg.exp() - 1.0)
    }
}

impl Model for DiodeShockleyModel {
    type ComponentType = Diode;

    // fn update(&self, diode: &mut Diode, states: &CircuitState<f64>) -> crate::error::Result<()> {
    //     // 1. Get the current guess voltage (lookback 0)
    //     let v_plus = states.get_guess_value(&diode.node_plus).unwrap_or(0.0);
    //     let v_minus = states.get_guess_value(&diode.node_minus).unwrap_or(0.0);
    //     let vd_guess = v_plus - v_minus;
    //
    //     // 2. Linearize based on this guess
    //     diode.linearize(vd_guess, self.vt)
    // }
}

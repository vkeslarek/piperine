use crate::component::dio::Diode;
use crate::model::Model;
use crate::numerical_method::History;
use crate::state::CircuitStates;

pub type DiodeModel = dyn Model<ComponentType = Diode> + 'static;

pub struct DiodeShockleyModel {
    pub name: String,
    pub vt: f64, // Thermal voltage
}

impl DiodeShockleyModel {
    pub fn new(name: String) -> Self {
        Self { name, vt: 0.02585 } // ~25.85mV at 300K
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

    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(&self, diode: &mut Diode, states: &CircuitStates) -> crate::error::Result<()> {
        // 1. Get the current guess voltage (lookback 0)
        let v_plus = states.get_value(&diode.node_plus, 0).unwrap_or(0.0);
        let v_minus = states.get_value(&diode.node_minus, 0).unwrap_or(0.0);
        let vd_guess = v_plus - v_minus;

        // 2. Linearize based on this guess
        diode.linearize(vd_guess, self.vt)
    }
}

use crate::devices::Model;
use crate::devices::diode::Diode;
use crate::math::unit::{Conductance, Siemens, UnitExt};
use num_complex::ComplexFloat;

pub type DiodeModelType = dyn Model<ComponentType = Diode> + 'static;

#[derive(Debug)]
pub struct DiodeShockleyModel {
    pub vt: f64, // Thermal voltage
}

impl DiodeShockleyModel {
    pub fn new() -> Self {
        Self { vt: 0.02585 } // ~25.85mV at 300K
    }
}

impl Model for DiodeShockleyModel {
    type ComponentType = Diode;

    fn update(&self, diode: &mut Diode) -> crate::error::Result<()> {
        let nvt = diode.emission_coefficient.value * self.vt;
        let is = diode.saturation_current.value; // Complex, use .re

        // 1. Retrieve the two DISTINCT voltages
        let v_new = diode.v_guess.value; // Raw 5V guess
        let v_old = diode.v_linearized.value; // Safe 0.16V from previous step

        // if !v_new.is_finite() {
        //     // If the solver blew up, do not corrupt the model state.
        //     // Stick to the old safe value or try a small step.
        //     eprintln!("Warning: Solver returned non-finite voltage for {}. Retaining v_old.", diode.name);
        //     // Returning early keeps v_linearized, g_eq, and i_eq at their last known good values.
        //     // This often allows the solver to recover in the next time step (if transient).
        //     return Ok(());
        // }

        // 2. Critical Voltage
        let vcrit = nvt * (nvt / (std::f64::consts::SQRT_2 * is)).ln();

        // 3. PnJLim: Compare v_new vs v_old
        // Since they are different variables, (v_new - v_old) is now 4.84V, NOT 0.0!
        let vd_limited = if v_new > vcrit && (v_new - v_old).abs() > (2.0 * nvt) {
            let arg = 1.0 + (v_new - v_old) / nvt;
            if arg > 0.0 {
                // This dampens the step: 5V -> ~0.3V
                v_old + nvt * arg.ln()
            } else {
                vcrit
            }
        } else if v_new < -vcrit {
            if v_new < v_old - 2.0 * nvt {
                v_old - 2.0 * nvt
            } else {
                v_new
            }
        } else {
            v_new
        };

        // 4. Physics using the LIMITED voltage
        let exp_term = (vd_limited / nvt).exp();
        let id = is * (exp_term - 1.0);
        let gd = (is / nvt) * exp_term;

        diode.g_eq = gd.S();
        diode.i_eq = (id - gd * vd_limited).A();

        // 5. COMMIT the limited voltage.
        // This saves the "safe" point (e.g., 0.3V) so the next iteration starts from there.
        diode.v_linearized = vd_limited.V();

        // println!("{:?}", diode);

        Ok(())
    }
}

use crate::devices::Model;
use crate::devices::diode::Diode;
use crate::math::unit::{Current, Temperature, UnitExt, Voltage};
use crate::solver::Context;
use crate::util::AsAny;
use std::any::Any;

pub trait DiodeModelType: Model<ComponentType = Diode> {
    fn update_linearization(
        &self,
        component: &mut Diode,
        v_now: f64,
        v_old: f64,
        context: &Context,
    );
}

#[derive(Debug)]
pub struct DiodeModel {
    pub name: String,
    pub is: Current,       // Saturation Current
    pub n: f64,            // Emission Coefficient
    pub tnom: Temperature, // Nominal Temperature
}

impl Default for DiodeModel {
    fn default() -> Self {
        Self {
            name: "DefaultDiode".to_string(),
            is: 1e-14.A(),
            n: 1.0,
            tnom: 27.0.deg_C(),
        }
    }
}

impl Model for DiodeModel {
    type ComponentType = Diode;
}

impl AsAny for DiodeModel {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl DiodeModelType for DiodeModel {
    fn update_linearization(
        &self,
        component: &mut Diode,
        v_now: f64,
        v_old: f64,
        context: &Context,
    ) {
        // 1. What the Linear Solver suggested (Raw Step)
        let v_d_proposed = v_now;

        // 2. What we used in the PREVIOUS iteration (The Guess)
        // Note: You need to make sure your state stores the values
        // from the iteration BEFORE the solver was called.
        let v_d_old = v_old;

        // 3. Simple Damping Logic (Limit step to 2*Vt ≈ 52mV)
        let vt = 0.02585; // Thermal voltage at room temp
        let max_step = 2.0 * vt;

        let v_d = if (v_d_proposed - v_d_old).abs() > max_step {
            if v_d_proposed > v_d_old {
                v_d_old + max_step // Clamp the rise
            } else {
                v_d_old - max_step // Clamp the fall
            }
        } else {
            v_d_proposed // Step is small enough, accept it
        };

        // 4. Calculate Conductance (g_d) and Current (i_d)
        let nvt = self.n * vt;
        let arg = v_d / nvt;

        // Shockley Equation with protection against float overflow
        let (i_d, g_d) = if arg > 50.0 {
            // Very high bias - use linear approximation to prevent Infinity
            let ev_limit = 50.0f64.exp();
            let g = (self.is.value / nvt) * ev_limit;
            let i = self.is.value * (ev_limit - 1.0) + g * (v_d - 50.0 * nvt);
            (i, g)
        } else if arg > -50.0 {
            let ev = arg.exp();
            let i = self.is.value * (ev - 1.0);
            let g = (self.is.value / nvt) * ev;
            (i, g)
        } else {
            (-self.is.value, context.gmin.value)
        };

        // 5. Calculate RHS Source (i_eq = i_actual - g_d * v_d)
        component.g_eq = (g_d + context.gmin.value).S();
        component.i_eq = (i_d - g_d * v_d).A();
    }
}

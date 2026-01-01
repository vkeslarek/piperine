use crate::{
    Analysis, CircuitInstance, ComponentInstance, Device, ModelInstance, NodeIdentifier,
    NodeReference, PiperineProblem, PiperineResult, RealStamper, TransientAnalysisContext,
};
use std::sync::Arc;

pub struct Resistor;

impl Device for Resistor {
    type ComponentInstance = ResistorInstance;
    const NAME: &'static str = "Resistor";
    const DESCRIPTION: &'static str = "Simple linear resistor";
    const PINS: &'static [&'static str] = &["R+", "R-"];
    const AVAILABLE_ANALYSIS: &'static [Analysis] = &[Analysis::OP];
}

pub struct ResistorInstance {
    pub model: Arc<ResistorModelInstance>,
    pub n_plus: Arc<NodeReference>,
    pub n_minus: Arc<NodeReference>,
    pub temp: f64,
    pub conduct: f64,
    pub resist: f64,
    pub ac_resist: f64,
    pub ac_conduct: f64,
    pub width: f64,
    pub length: f64,
    pub scale: f64,
    pub multiplier: usize,
    pub temp_coeff1: f64,
    pub temp_coeff2: f64,
    pub temp_coeffe: f64,
    pub bv_max: f64,
    pub noisy: bool,
    pub eff_noise_area: f64,
}

impl ResistorInstance {
    fn setup_problem(
        parameters: &<ResistorInstance as ComponentInstance>::ComponentParameters,
    ) -> PiperineProblem {
        PiperineProblem::ComponentSetupProblem {
            component_type: "Resistor".to_string(),
            component_name: parameters.name.clone(),
        }
    }
}

pub struct ResistorComponentParameters {
    pub name: String,
    pub n_plus: NodeIdentifier,
    pub n_minus: NodeIdentifier,
    pub model: Option<String>,
    pub resistance: Option<f64>,
    pub ac_resistance: Option<f64>,
    pub multiplier: Option<usize>,
    pub scale: Option<f64>,
    pub length: Option<f64>,
    pub width: Option<f64>,
    pub temp: Option<f64>,
    pub dtemp: Option<f64>,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub noisy: Option<bool>,
}

impl Default for ResistorComponentParameters {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            n_plus: NodeIdentifier::Gnd,
            n_minus: NodeIdentifier::Gnd,
            model: None,
            resistance: None,
            ac_resistance: None,
            multiplier: None,
            scale: None,
            length: None,
            width: None,
            temp: None,
            dtemp: None,
            tc1: None,
            tc2: None,
            noisy: None,
        }
    }
}

impl ComponentInstance for ResistorInstance {
    type ComponentParameters = ResistorComponentParameters;

    fn setup(
        parameters: Self::ComponentParameters,
        circuit_instance: &CircuitInstance,
    ) -> PiperineResult<Self> {
        // 1. Create the error context once
        let problem = Self::setup_problem(&parameters);

        // 2. Destructure parameters to take ownership of everything
        let ResistorComponentParameters {
            name,
            n_plus,
            n_minus,
            model,
            resistance,
            ac_resistance,
            multiplier,
            scale,
            length,
            width,
            temp,
            tc1,
            tc2,
            noisy,
            ..
        } = parameters;

        // 3. Fetch Model and Nodes using the '?' operator and the problem context
        let model = circuit_instance
            .get_model_instance::<ResistorModelInstance>(&model)
            .map_err(|err| err.wrap(problem.clone()))?;

        let n_plus = circuit_instance
            .get_node_reference(n_plus)
            .map_err(|err| err.wrap(problem.clone()))?;

        let n_minus = circuit_instance
            .get_node_reference(n_minus)
            .map_err(|err| err.wrap(problem))?;

        // 4. Resolve dimensions and physics
        let w = width.unwrap_or(model.def_width);
        let l = length.unwrap_or(model.def_length);

        // SPICE Logic: If R is not given, calculate from Sheet Resistance (Rsh)
        let resist = resistance.unwrap_or_else(|| {
            if model.sheet_res > 0.0 && (l - model.short) > 0.0 {
                model.sheet_res * (l - model.short) / (w - model.narrow)
            } else {
                model.res
            }
        });

        let ac_resist = ac_resistance.unwrap_or(resist);

        // 5. Calculate effective noise area (using the formula from your previous turn)
        let eff_noise_area = if width.is_some() || length.is_some() {
            (l - 2.0 * model.short).powf(model.lf) * (w - 2.0 * model.narrow).powf(model.wf)
        } else {
            1.0
        };

        Ok(Self {
            model: model.clone(),
            n_plus,
            n_minus,
            temp: temp.unwrap_or(circuit_instance.temp()),
            conduct: 0.0, // Calculated in temperature()
            resist,
            ac_resist,
            ac_conduct: 0.0, // Calculated in temperature()
            width: w,
            length: l,
            scale: scale.unwrap_or(1.0),
            multiplier: multiplier.unwrap_or(1),
            temp_coeff1: tc1.unwrap_or(model.temp_coeff1),
            temp_coeff2: tc2.unwrap_or(model.temp_coeff2),
            temp_coeffe: model.temp_coeffe,
            bv_max: model.bv_max,
            noisy: noisy.unwrap_or(true),
            eff_noise_area,
        })
    }

    fn temperature(&mut self) {
        // 1. Calculate the thermal delta from nominal temperature (usually 27°C)
        let difference = self.temp - self.model.tnom;

        // 2. Calculate the temperature scaling factor
        // Logic: Exponential (TCE) takes priority if it's non-zero.
        // Otherwise, use the standard Quadratic (TC1, TC2) model.
        let factor = if self.temp_coeffe != 0.0 {
            // Exponential: factor = 1.01 ^ (tce * ΔT)
            1.01f64.powf(self.temp_coeffe * difference)
        } else {
            // Quadratic: factor = 1 + tc1*ΔT + tc2*ΔT²
            // Optimized via Horner's Method: ((tc2 * ΔT) + tc1) * ΔT + 1.0
            (self.temp_coeff2 * difference + self.temp_coeff1) * difference + 1.0
        };

        // 3. Safety check: avoid division by zero
        // NgSpice defaults to 1mOhm if the resistance is missing or too low.
        let r_nom = if self.resist.abs() < 1e-12 {
            1e-3
        } else {
            self.resist
        };
        let r_ac_nom = if self.ac_resist.abs() < 1e-12 {
            r_nom
        } else {
            self.ac_resist
        };

        // 4. Final Conductance Calculation
        // Formula: G = m / (R_nom * factor * scale)
        // 'multiplier' handles parallel instances (m), 'scale' handles geometric scaling.
        let m = self.multiplier as f64;

        self.conduct = m / (r_nom * factor * self.scale);

        // AC conductance follows the same thermal factor
        self.ac_conduct = m / (r_ac_nom * factor * self.scale);
    }

    fn load_dc(
        &self,
        _: &CircuitInstance,
        _: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    ) {
        // Cache the conductance and IDs to avoid pointer chasing
        let g = self.conduct;
        let n1 = &self.n_plus;
        let n2 = &self.n_minus;

        // Use references (no cloning!)
        stamp.nodal_stamp(n1, n1, g);
        stamp.nodal_stamp(n2, n2, g);
        stamp.nodal_stamp(n1, n2, -g);
        stamp.nodal_stamp(n2, n1, -g);
    }
}

pub struct ResistorModelParameters {
    pub name: String,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub tce: Option<f64>,
    pub rsh: Option<f64>,
    pub defw: Option<f64>,
    pub defl: Option<f64>,
    pub narrow: Option<f64>,
    pub short: Option<f64>,
    pub tnom: Option<f64>,
    pub kf: Option<f64>,
    pub af: Option<f64>,
    pub wf: Option<f64>,
    pub lf: Option<f64>,
    pub ef: Option<f64>,
    pub resistance: Option<f64>,
}

pub struct ResistorModelInstance {
    pub tnom: f64,
    pub temp_coeff1: f64,
    pub temp_coeff2: f64,
    pub temp_coeffe: f64,
    pub sheet_res: f64,
    pub def_width: f64,
    pub def_length: f64,
    pub narrow: f64,
    pub short: f64,
    pub fn_coef: f64,
    pub f_nexp: f64,
    pub res: f64,
    pub bv_max: f64,
    pub lf: f64,
    pub wf: f64,
    pub ef: f64,
}

impl ModelInstance for ResistorModelInstance {
    type Parameters = ResistorModelParameters;

    fn setup(
        parameters: Self::Parameters,
        circuit_instance: &CircuitInstance,
    ) -> PiperineResult<Self> {
        // 1. Destructure to take ownership
        let ResistorModelParameters {
            tnom,
            tc1,
            tc2,
            tce,
            rsh,
            defw,
            defl,
            narrow,
            short,
            kf,
            af,
            wf,
            lf,
            ef,
            resistance,
            ..
        } = parameters;

        // 2. Resolve defaults
        // If tnom isn't provided, use the global circuit nominal temperature
        let tnom = tnom.unwrap_or(circuit_instance.temp());

        // Flicker Noise: kf is the coefficient, af is the exponent
        let fn_coef = kf.unwrap_or(0.0);
        let f_nexp = af.unwrap_or(1.0);

        // Safe Operating Area (SOA)
        // A value of 0.0 or infinity usually means "not checked"
        let bv_max = f64::INFINITY;

        Ok(Self {
            tnom,
            temp_coeff1: tc1.unwrap_or(0.0),
            temp_coeff2: tc2.unwrap_or(0.0),
            temp_coeffe: tce.unwrap_or(0.0),
            sheet_res: rsh.unwrap_or(0.0),
            def_width: defw.unwrap_or(10e-6),
            def_length: defl.unwrap_or(10e-6),
            narrow: narrow.unwrap_or(0.0),
            short: short.unwrap_or(0.0),
            fn_coef,
            f_nexp,
            res: resistance.unwrap_or(0.0),
            bv_max,
            lf: lf.unwrap_or(1.0),
            wf: wf.unwrap_or(1.0),
            ef: ef.unwrap_or(1.0),
        })
    }
}

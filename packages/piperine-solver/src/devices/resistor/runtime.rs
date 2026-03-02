use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::{Noise, NoiseSource};
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::{CircuitReference, Netlist};
use crate::devices::resistor::Resistor;
use crate::devices::soa::{SoaCheck, SoaCheckState, SoaViolation, SoaViolationSeverity};
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::constant::BOLTZMANN_CONSTANT;
use crate::math::linear::Stamp;
use crate::math::num::Scalar;
use crate::math::unit::Siemens;
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct ResistorRuntime {
    component: Arc<Resistor>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    conductance: Siemens,
}

impl Runtime for ResistorRuntime {
    type ComponentType = Resistor;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let node_plus = netlist.connect_node(component.node_plus().clone());
        let node_minus = netlist.connect_node(component.node_minus().clone());

        Self {
            component,
            node_plus,
            node_minus,
            conductance: 0.0,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, context: &Context)
    where
        Self: Sized,
    {
        self.conductance = self
            .component
            .model()
            .eval_conductance(&self.component, context);
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_noise_source(&self) -> Option<&dyn NoiseSource> {
        Some(self)
    }

    fn as_soa_check(&self) -> Option<&dyn SoaCheck> {
        Some(self)
    }
}

impl DcAnalysis for ResistorRuntime {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                self.conductance,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                self.conductance,
            ),
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -self.conductance,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -self.conductance,
            ),
        ]
    }
}

impl AcAnalysis for ResistorRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let admittance = Complex::new(self.conductance, 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), admittance),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -admittance),
        ]
    }
}

impl TransientAnalysis for ResistorRuntime {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                self.conductance,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                self.conductance,
            ),
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -self.conductance,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -self.conductance,
            ),
        ]
    }
}

impl NoiseSource for ResistorRuntime {
    fn noise_current_psd(
        &self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        if !self.component.noisy {
            return Vec::new();
        }

        let freq = ac_context.frequency;

        // 1. Temperature Setup (Base Unit: Kelvin)
        // If self.temp is None, use Model Tnom.
        // Note: .K() is removed; we assume values are stored as raw f64 Kelvin.
        let temp_val = self.component.temp.unwrap_or(self.component.model.tnom);
        let delta_temp_val = self.component.delta_temp.unwrap_or(0.0);
        let temp_kelvin = temp_val + delta_temp_val;

        // --- A. Thermal Noise (Johnson Noise) ---
        // Formula: S_th = 4 * k * T * G
        // Units: A^2/Hz (Current PSD)
        let g_val = self.conductance;
        let thermal_psd = 4.0 * BOLTZMANN_CONSTANT * temp_kelvin * g_val;

        // --- B. Flicker Noise (1/f Noise) ---
        // Formula: S_fl = (KF * I^AF) / (f^EF * L^LF * W^WF)
        let kf = self.component.model.kf;

        // Optimization: Only calc flicker if coefficient exists and frequency is non-zero
        // 1.0 pHz = 1.0e-12 Hz
        let flicker_psd = if kf > 0.0 && freq > 1.0e-12 {
            let af = self.component.model.af;
            let ef = self.component.model.ef;

            // Geometry (Base Unit: Meters)
            let def_l = self.component.model.def_length;
            let def_w = self.component.model.def_width;
            let short = self.component.model.short;
            let narrow = self.component.model.narrow;

            let l_eff = self.component.length.unwrap_or(def_l) - 2.0 * short;
            let w_eff = self.component.width.unwrap_or(def_w) - 2.0 * narrow;

            // Clamp dimensions to 1nm to avoid div-by-zero or negative geometry
            let l_val_m = l_eff.max(1.0e-9);
            let w_val_m = w_eff.max(1.0e-9);

            let lf = self.component.model.lf;
            let wf = self.component.model.wf;
            let geometry_factor = l_val_m.powf(lf) * w_val_m.powf(wf);

            // 2. Calculate DC Current (I_dc = V * G)
            let v_plus = dc_point.get(self.node_plus.clone()).unwrap_or(0.0);
            let v_minus = dc_point.get(self.node_minus.clone()).unwrap_or(0.0);

            let v_dc = v_plus - v_minus;
            let i_dc = v_dc * g_val;

            let m_ratio = self.component.multiplier.unwrap_or(1.0);

            // I_unit = I_total / M
            let i_unit_val = i_dc / m_ratio;

            // CRITICAL FIX: Use .abs() on current!
            // If i_unit_val is negative and af is fractional (e.g. 1.8), powf yields NaN.
            // Noise magnitude is independent of current direction.
            let i_noise_factor = i_unit_val.abs().powf(af);
            let f_noise_factor = freq.powf(ef);

            // S_fl = M * (Kf * |i_unit|^Af) / (f^Ef * AreaFactor)
            m_ratio * (kf * i_noise_factor) / (f_noise_factor * geometry_factor)
        } else {
            0.0
        };

        let total_psd = thermal_psd + flicker_psd;

        if total_psd > 0.0 {
            vec![Noise {
                terminals: (self.node_plus.clone(), self.node_minus.clone()),
                value: total_psd,
            }]
        } else {
            Vec::new()
        }
    }
}

impl SoaCheck for ResistorRuntime {
    fn soa_check(&self, circuit_state: &SoaCheckState, _context: &Context) -> Vec<SoaViolation> {
        let mut soa_violations = Vec::new();

        if let Some(bv_max) = self.component.model.bv_max {
            let v_plus = circuit_state
                .latest()
                .and_then(|val| val.get(self.node_plus.idx().unwrap()).cloned())
                .unwrap_or(0.0);
            let v_minus = circuit_state
                .latest()
                .and_then(|val| val.get(self.node_minus.idx().unwrap()).cloned())
                .unwrap_or(0.0);

            if (v_plus - v_minus).abs() >= bv_max {
                soa_violations.push(SoaViolation::new(
                    "BVMAX_EXCEEDED",
                    self.component.name.clone(),
                    "Maximum breakdown voltage of the Resistor reached!",
                    SoaViolationSeverity::HIGH,
                ));
            }
        }

        soa_violations
    }
}

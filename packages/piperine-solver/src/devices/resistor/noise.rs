use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{Noise, NoiseSource};
use crate::devices::resistor::Resistor;
use crate::math::constant::BOLTZMANN_CONSTANT;

impl NoiseSource for Resistor {
    fn noise_current_psd(
        &self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        if !self.noisy {
            return Vec::new();
        }

        let freq = ac_context.frequency;

        // 1. Temperature Setup (Base Unit: Kelvin)
        // If self.temp is None, use Model Tnom.
        // Note: .K() is removed; we assume values are stored as raw f64 Kelvin.
        let temp_val = self.temp.unwrap_or(self.model.tnom);
        let delta_temp_val = self.delta_temp.unwrap_or(0.0);
        let temp_kelvin = temp_val + delta_temp_val;

        // --- A. Thermal Noise (Johnson Noise) ---
        // Formula: S_th = 4 * k * T * G
        // Units: A^2/Hz (Current PSD)
        let g_val = self.conductance;
        let thermal_psd = 4.0 * BOLTZMANN_CONSTANT * temp_kelvin * g_val;

        // --- B. Flicker Noise (1/f Noise) ---
        // Formula: S_fl = (KF * I^AF) / (f^EF * L^LF * W^WF)
        let kf = self.model.kf;

        // Optimization: Only calc flicker if coefficient exists and frequency is non-zero
        // 1.0 pHz = 1.0e-12 Hz
        let flicker_psd = if kf > 0.0 && freq > 1.0e-12 {
            let af = self.model.af;
            let ef = self.model.ef;

            // Geometry (Base Unit: Meters)
            let def_l = self.model.def_length;
            let def_w = self.model.def_width;
            let short = self.model.short;
            let narrow = self.model.narrow;

            let l_eff = self.length.unwrap_or(def_l) - 2.0 * short;
            let w_eff = self.width.unwrap_or(def_w) - 2.0 * narrow;

            // Clamp dimensions to 1nm to avoid div-by-zero or negative geometry
            let l_val_m = l_eff.max(1.0e-9);
            let w_val_m = w_eff.max(1.0e-9);

            let lf = self.model.lf;
            let wf = self.model.wf;
            let geometry_factor = l_val_m.powf(lf) * w_val_m.powf(wf);

            // 2. Calculate DC Current (I_dc = V * G)
            let v_plus = dc_point.get(self.node_plus.clone()).unwrap_or(0.0);
            let v_minus = dc_point.get(self.node_minus.clone()).unwrap_or(0.0);

            let v_dc = v_plus - v_minus;
            let i_dc = v_dc * g_val;

            let m_ratio = self.multiplier.unwrap_or(1.0);

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

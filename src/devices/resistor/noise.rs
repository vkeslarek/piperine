use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{Noise, NoiseSource};
use crate::devices::resistor::Resistor;
use crate::math::constant::BOLTZMANN_CONSTANT;
use crate::math::unit::{
    Ampere, Conductance, Current, Dimensionless, Frequency, Hertz, Length, Meter, UnitExt, Volt,
    Voltage,
};

impl NoiseSource for Resistor {
    fn noise_current_psd(
        &self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        if !self.noisy {
            return Vec::new();
        }

        let mut noise_list = Vec::new();
        let freq: Frequency = ac_context.frequency;

        // 1. Temperature Setup
        let temp_val = self.temp.unwrap_or(self.model.tnom);
        let delta_temp_val = self.delta_temp.unwrap_or(0.0.delta_C());
        let temp_kelvin = temp_val + delta_temp_val;

        // --- A. Thermal Noise (Johnson Noise) ---
        // Formula: S_th = 4 * k * T * G
        let g_val: Conductance = self.conductance;

        let thermal_psd = 4.0 * BOLTZMANN_CONSTANT * temp_kelvin * g_val;

        // --- B. Flicker Noise (1/f Noise) ---
        let kf = self.model.kf;
        let af = self.model.af;
        let ef = self.model.ef;

        let flicker_psd = if kf > 0.0.ratio() && freq > 1.0.pHz() {
            let def_l = self.model.def_length;
            let def_w = self.model.def_width;
            let short = self.model.short;
            let narrow = self.model.narrow;

            let l_eff = self.length.unwrap_or(def_l) - 2.0 * short;
            let w_eff = self.width.unwrap_or(def_w) - 2.0 * narrow;

            let l_val_m = l_eff.max(1.0.nm());
            let w_val_m = w_eff.max(1.0.nm());

            let lf = self.model.lf;
            let wf = self.model.wf;
            let geometry_factor = l_val_m.get::<Meter>().powf(lf.get::<Dimensionless>())
                * w_val_m.get::<Meter>().powf(wf.get::<Dimensionless>());

            // 2. Calculate DC Current (I_dc = V * G)
            let v_plus = dc_point.get_value(&self.node_plus).unwrap_or(0.0);
            let v_minus = dc_point.get_value(&self.node_minus).unwrap_or(0.0);

            let v_dc = Voltage::new::<Volt>(v_plus - v_minus);
            let i_dc: Current = v_dc * g_val;

            let m_ratio = self.multiplier.unwrap_or(1.0.ratio());
            let m_val = m_ratio.value;

            // I_unit = I_total / M
            let i_unit_val = i_dc.get::<Ampere>() / m_val;
            let freq_val = freq.get::<Hertz>();

            // S_fl = M * (Kf * i_unit^Af) / (f^Ef * AreaFactor)
            let flicker_raw = m_val * (kf * i_unit_val.powf(af.get::<Dimensionless>()))
                / (freq_val.powf(ef.get::<Dimensionless>()) * geometry_factor);

            let ref_unit = 1.0.A() * 1.0.A() * 1.0.Sec();

            ref_unit * flicker_raw
        } else {
            // Zero must be typed correctly to match thermal_psd
            // 0.0 * (A * A * s)
            0.0.A() * 0.0.A() * 1.0.Sec()
        };

        let total_psd = thermal_psd + flicker_psd;

        if total_psd.value > 0.0 {
            noise_list.push(Noise {
                terminals: (self.node_plus.clone(), self.node_minus.clone()),
                value: total_psd,
            });
        }

        noise_list
    }
}

use crate::analysis::dc::DcAnalysis;
use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::analysis::noise::{NoiseSource, Noise};
use crate::circuit::netlist::AnalogReference;
use crate::osdi::runtime::OsdiRuntime;
use crate::math::linear::Stamp;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use crate::osdi::ffi::*;
use num_complex::Complex;

impl DcAnalysis for OsdiRuntime {
    fn load_dc(
        &self,
        state: &CircularArrayBuffer2<f64>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let prev_solve = self.build_prev_solve(state);
        let flags = ENABLE_LIM | CALC_RESIST_LIM_RHS | CALC_RESIST_RESIDUAL | CALC_RESIST_JACOBIAN | ANALYSIS_DC;
        self.eval_with_flags(flags, &prev_solve, context);

        let mut stamps = Vec::new();
        let mut rhs = [0.0f64; SCRATCH];
        let inst = self.inst_data.as_ptr() as *mut std::os::raw::c_void;
        let model = self.model_data.as_ptr() as *mut std::os::raw::c_void;
        unsafe {
            if let Some(f) = self.desc().load_spice_rhs_dc {
                f(inst, model, rhs.as_mut_ptr(), prev_solve.as_ptr());
            }
        }
        self.collect_rhs_stamps(&rhs, &mut stamps);
        self.add_resist_jac_stamps(&mut stamps);

        stamps
    }
}

impl AcAnalysis for OsdiRuntime {
    fn load_ac(
        &self,
        dc_point: &crate::analysis::dc::DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex<f64>>> {
        let prev_solve = self.build_prev_solve_from_dc(dc_point);
        let flags = CALC_RESIST_JACOBIAN | CALC_REACT_JACOBIAN | ANALYSIS_AC;
        self.eval_with_flags(flags, &prev_solve, context);

        let omega = ac_ctx.frequency * std::f64::consts::PI * 2.0;
        let mut stamps = Vec::new();
        
        let desc = self.desc();
        let num_res = desc.num_resistive_jacobian_entries as usize;
        if num_res > 0 {
            let mut jac = vec![0.0f64; num_res];
            let inst = self.inst_data.as_ptr() as *mut std::os::raw::c_void;
            let model = self.model_data.as_ptr() as *mut std::os::raw::c_void;
            unsafe {
                if let Some(f) = desc.write_jacobian_array_resist {
                    f(inst, model, jac.as_mut_ptr());
                }
            }
            for (j, (row, col)) in self.resist_jac_refs.iter().enumerate() {
                if j >= num_res { break; }
                let val = jac[j];
                if val == 0.0 { continue; }
                if let (Some(r), Some(c)) = (row, col) {
                    stamps.push(Stamp::Matrix(r.clone(), c.clone(), Complex::new(val, 0.0)));
                }
            }
        }

        let num_react = desc.num_reactive_jacobian_entries as usize;
        if num_react > 0 {
            let mut jac = vec![0.0f64; num_react];
            let inst = self.inst_data.as_ptr() as *mut std::os::raw::c_void;
            let model = self.model_data.as_ptr() as *mut std::os::raw::c_void;
            unsafe {
                if let Some(f) = desc.write_jacobian_array_react {
                    f(inst, model, jac.as_mut_ptr());
                }
            }
            for (j, (row, col)) in self.react_jac_refs.iter().enumerate() {
                if j >= num_react { break; }
                let val = jac[j] * omega;
                if val == 0.0 { continue; }
                if let (Some(r), Some(c)) = (row, col) {
                    stamps.push(Stamp::Matrix(r.clone(), c.clone(), Complex::new(0.0, val)));
                }
            }
        }

        stamps
    }
}

impl TransientAnalysis for OsdiRuntime {
    fn load_transient(
        &self,
        state: &CircularArrayBuffer2<f64>,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let prev_solve = self.build_prev_solve(state);
        self.eval_tran(tran_ctx.time, &prev_solve, context);

        let mut stamps = Vec::new();
        let alpha = 1.0 / tran_ctx.dt; // Backward Euler alpha

        // Use load_spice_rhs_tran which computes the combined SPICE-style RHS
        // (J*x - f(x)) including both resistive and reactive terms.
        let mut rhs = [0.0f64; SCRATCH];
        let inst = self.inst_data.as_ptr() as *mut std::os::raw::c_void;
        let model = self.model_data.as_ptr() as *mut std::os::raw::c_void;
        unsafe {
            if let Some(f) = self.desc().load_spice_rhs_tran {
                f(inst, model, rhs.as_mut_ptr(), prev_solve.as_ptr(), alpha);
            }
        }
        self.collect_rhs_stamps(&rhs, &mut stamps);
        self.add_resist_jac_stamps(&mut stamps);
        self.add_react_jac_stamps_scaled(&mut stamps, alpha);

        stamps
    }
}

impl NoiseSource for OsdiRuntime {
    fn noise_current_psd(
        &self,
        dc_point: &crate::analysis::dc::DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
    ) -> Vec<Noise> {
        let desc = self.desc();
        let num_noise = desc.num_noise_src as usize;

        // No noise sources or no load_noise callback — nothing to do.
        if num_noise == 0 || desc.load_noise.is_none() || desc.noise_sources.is_null() {
            return Vec::new();
        }

        // Evaluate the device at the DC operating point with CALC_NOISE flag.
        let prev_solve = self.build_prev_solve_from_dc(dc_point);
        let context = Context::default();
        let flags = CALC_NOISE | CALC_RESIST_JACOBIAN | CALC_REACT_JACOBIAN | ANALYSIS_AC;
        self.eval_with_flags(flags, &prev_solve, &context);

        // Call load_noise to fill the PSD array — one f64 per noise source.
        let mut psd_array = vec![0.0f64; num_noise];
        let inst = self.inst_data.as_ptr() as *mut std::os::raw::c_void;
        let model = self.model_data.as_ptr() as *mut std::os::raw::c_void;
        unsafe {
            (desc.load_noise.unwrap())(inst, model, ac_ctx.frequency, psd_array.as_mut_ptr());
        }

        // Map each noise source to a Noise struct with terminal references.
        let noise_sources = unsafe { std::slice::from_raw_parts(desc.noise_sources, num_noise) };
        let gnd_ref = AnalogReference::ground();
        let mut result = Vec::with_capacity(num_noise);
        for (i, ns) in noise_sources.iter().enumerate() {
            let psd = psd_array[i];
            if psd == 0.0 {
                continue;
            }

            let t1 = self.node_refs.get(ns.nodes.node_1 as usize)
                .and_then(|r| r.clone())
                .unwrap_or_else(|| gnd_ref.clone());
            let t2 = self.node_refs.get(ns.nodes.node_2 as usize)
                .and_then(|r| r.clone())
                .unwrap_or_else(|| gnd_ref.clone());

            result.push(Noise {
                terminals: (t1, t2),
                value: psd,
            });
        }

        result
    }
}

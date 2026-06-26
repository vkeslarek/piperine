use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext, TransientAnalysisState};
use crate::analysis::noise::{NoiseSource, Noise};
use crate::analog::netlist::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::collections::VecDeque;
use num_complex::Complex64;

use crate::analog::device::{AnalogDevice, SimInfo, EvalFlags, SimFlags};

pub trait AnalogRuntime: DcAnalysis + AcAnalysis + TransientAnalysis + NoiseSource + Send + Sync {
    fn device_name(&self) -> &str;
    fn limiting_active(&self) -> bool;
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context);
    fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context);
    fn bound_step_hint(&self) -> f64;
    fn read_opvars(&self) -> Vec<(String, f64)>;
    fn set_temperature(&mut self, temperature: f64);
    
    // Evaluation methods
    fn eval_dc(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) -> bool;
    fn eval_ac(&mut self, dc_op: &DcAnalysisResult, context: &Context) -> bool;
    fn eval_tran(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) -> bool;
}

pub struct DeviceRuntime<D: AnalogDevice> {
    pub device: D,
    pub model_data: D::ModelData,
    pub inst_data: D::InstanceData,
    pub device_name: String,
    
    pub node_refs: Vec<Option<AnalogReference>>,
    pub prev_state: Vec<f64>,
    pub next_state: Vec<f64>,
    pub last_time: f64,
    pub setup_temperature: f64,
    
    pub charge_history: VecDeque<Vec<f64>>,
    pub limiting_active: bool,
    
    pub str_params: Vec<(String, String)>,
    pub num_params: Vec<(String, f64)>,
}

impl<D: AnalogDevice> DeviceRuntime<D> 
where D::ModelData: Default, D::InstanceData: Default {
    pub fn new(
        device: D,
        device_name: String,
        node_refs_initial: Vec<Option<AnalogReference>>,
        params: &[(String, f64)],
        str_params: &[(String, String)],
        paras: &crate::analog::device::SimParams,
    ) -> Self {
        let mut model_data = D::ModelData::default();
        let mut inst_data = D::InstanceData::default();
        let mut node_refs = node_refs_initial;

        let mut init_info = crate::analog::device::InitInfo::new(SimFlags::empty());

        // Step 1: init model data
        device.setup_model(&mut model_data, paras, &mut init_info);

        // Step 2: pre-size inst_data and write initial node_mapping so that
        //         setup_instance sees the correct node assignments.
        //         We call setup_instance just to trigger the resize, then proceed.
        const DEFAULT_TEMP: f64 = 300.15;
        device.setup_instance(&model_data, &mut inst_data, DEFAULT_TEMP, SimFlags::empty(), paras, &mut init_info);
        device.bind_nodes(&mut inst_data, &mut node_refs);  // write node_mapping before params

        // Step 3: set ALL params (model params go into model_data; instance params go
        //         into the now-sized inst_data).  Model params must be correct before
        //         the real setup_instance call below.
        device.set_params(&mut model_data, &mut inst_data, params, str_params);

        // Step 4: run setup_instance again — this time model_data has correct params, so
        //         all cached temperature-dependent quantities are computed correctly.
        //         setup_instance also sets the collapsed[] flags for node collapsing.
        device.setup_instance(&model_data, &mut inst_data, DEFAULT_TEMP, SimFlags::empty(), paras, &mut init_info);

        // Step 5: re-apply instance params (setup_instance may have reset them to defaults).
        device.set_params(&mut model_data, &mut inst_data, params, str_params);

        // Step 6: re-apply node_mapping and apply collapsing based on the collapsed[]
        //         flags that were set by setup_instance in Step 4.
        device.bind_nodes(&mut inst_data, &mut node_refs);

        let num_states = device.num_states().max(1);
        
        Self {
            device, model_data, inst_data, device_name, node_refs,
            prev_state: vec![0.0f64; num_states],
            next_state: vec![0.0f64; num_states],
            last_time: f64::NEG_INFINITY,
            setup_temperature: DEFAULT_TEMP,
            charge_history: VecDeque::new(),
            limiting_active: false,
            str_params: str_params.to_vec(),
            num_params: params.to_vec(),
        }
    }
}

impl<D: AnalogDevice> DeviceRuntime<D> {
    fn rerun_setup_instance(&mut self, temperature: f64, context: &Context) {
        let paras = crate::analog::device::SimParams {
            ini_lim: false, gmin: context.gmin, gdev: 1e-12, tnom: context.tnom, simulator_version: 1.0, source_scale_factor: 1.0,
            epsmin: 1e-12, reltol: context.reltol, vntol: context.vntol, abstol: context.abstol,
        };
        let mut init_info = crate::analog::device::InitInfo::new(SimFlags::empty());
        self.device.setup_instance(&self.model_data, &mut self.inst_data, temperature, SimFlags::empty(), &paras, &mut init_info);
        self.device.set_params(&mut self.model_data, &mut self.inst_data, &self.num_params, &self.str_params);
        self.device.bind_nodes(&mut self.inst_data, &mut self.node_refs);
        self.setup_temperature = temperature;
    }
}

impl<D: AnalogDevice> AnalogRuntime for DeviceRuntime<D> {
    fn device_name(&self) -> &str { &self.device_name }
    fn limiting_active(&self) -> bool { self.limiting_active }
    
    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, context: &Context) {
        if (context.time - self.last_time).abs() > 1e-20 {
            self.prev_state.copy_from_slice(&self.next_state);
            self.last_time = context.time;
        }
        if (context.temperature - self.setup_temperature).abs() > 0.01 {
            self.rerun_setup_instance(context.temperature, context);
        }
    }
    
    fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _context: &Context) {
        let mut rhs = [0.0f64; crate::analog::osdi::ffi::SCRATCH];
        self.device.load_residual_react(&self.model_data, &self.inst_data, &mut rhs);
        let indices = self.device.get_rhs_indices(&self.inst_data);
        let mut charges = Vec::with_capacity(indices.len());
        for &idx in &indices {
            charges.push(if let Some(i) = idx { rhs[i] } else { 0.0 });
        }
        self.charge_history.push_front(charges);
        if self.charge_history.len() > 10 { self.charge_history.pop_back(); }
    }
    
    fn bound_step_hint(&self) -> f64 { self.device.bound_step_hint(&self.inst_data) }
    fn read_opvars(&self) -> Vec<(String, f64)> { self.device.read_opvars(&self.model_data, &self.inst_data) }
    
    fn set_temperature(&mut self, temperature: f64) {
        let ctx = Context::default();
        if (temperature - self.setup_temperature).abs() > 0.01 { self.rerun_setup_instance(temperature, &ctx); }
    }
    
    fn eval_dc(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) -> bool {
        let flags = SimFlags::ENABLE_LIM | SimFlags::ANALYSIS_DC | SimFlags::CALC_RESIST_RESIDUAL | SimFlags::CALC_RESIST_JACOBIAN | SimFlags::CALC_RESIST_LIM_RHS;
        let paras = crate::analog::device::SimParams {
            ini_lim: false, gmin: context.gmin, gdev: 1e-12, tnom: context.tnom, simulator_version: 1.0, source_scale_factor: 1.0,
            epsmin: 1e-12, reltol: context.reltol, vntol: context.vntol, abstol: context.abstol,
        };
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let mut info = SimInfo {
            params: &paras, abstime: context.time, prev_solve: &prev_solve, prev_state: &self.prev_state, next_state: &mut self.next_state, flags,
        };
        let ret = self.device.eval(&self.model_data, &mut self.inst_data, &mut info);
        self.limiting_active = ret.contains(EvalFlags::LIM);
        self.limiting_active
    }
    
    fn eval_ac(&mut self, dc_op: &DcAnalysisResult, _context: &Context) -> bool {
        let flags = SimFlags::ANALYSIS_AC | SimFlags::CALC_RESIST_JACOBIAN | SimFlags::CALC_REACT_JACOBIAN;
        let paras = crate::analog::device::SimParams {
            ini_lim: false, gmin: 0.0, gdev: 0.0, tnom: 300.15, simulator_version: 1.0, source_scale_factor: 1.0,
            epsmin: 1e-12, reltol: 1e-3, vntol: 1e-6, abstol: 1e-12,
        };
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| {
            if let Some(r) = self.node_refs.iter().flatten().find(|r| r.idx() == Some(k)) {
                dc_op.get(r.variable().clone()).unwrap_or(0.0)
            } else {
                0.0
            }
        });
        let mut info = SimInfo { params: &paras, abstime: 0.0, prev_solve: &prev_solve, prev_state: &self.prev_state, next_state: &mut self.next_state, flags };
        self.device.eval(&self.model_data, &mut self.inst_data, &mut info);
        true
    }
    
    fn eval_tran(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) -> bool {
        let flags = SimFlags::ANALYSIS_TRAN | SimFlags::CALC_RESIST_RESIDUAL | SimFlags::CALC_REACT_RESIDUAL | SimFlags::CALC_RESIST_JACOBIAN | SimFlags::CALC_REACT_JACOBIAN | SimFlags::CALC_RESIST_LIM_RHS | SimFlags::CALC_REACT_LIM_RHS | SimFlags::ENABLE_LIM;
        let paras = crate::analog::device::SimParams {
            ini_lim: false, gmin: context.gmin, gdev: 1e-12, tnom: context.tnom, simulator_version: 1.0, source_scale_factor: 1.0,
            epsmin: 1e-12, reltol: context.reltol, vntol: context.vntol, abstol: context.abstol,
        };
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let mut info = SimInfo { params: &paras, abstime: context.time, prev_solve: &prev_solve, prev_state: &self.prev_state, next_state: &mut self.next_state, flags };
        let ret = self.device.eval(&self.model_data, &mut self.inst_data, &mut info);
        self.limiting_active = ret.contains(EvalFlags::LIM);
        self.limiting_active
    }
}

impl<D: AnalogDevice> DcAnalysis for DeviceRuntime<D> {
    fn load_dc(&mut self, state: &DcAnalysisState, context: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.eval_dc(state, context);
        let mut stamps = Vec::new();

        // Use the proper SPICE-style RHS which computes J*x - f(x)
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let mut rhs = [0.0f64; crate::analog::osdi::ffi::SCRATCH];
        self.device.load_spice_rhs_dc(&self.model_data, &self.inst_data, &mut rhs, &prev_solve);
        self.device.collect_rhs_stamps(&self.inst_data, &rhs, &self.node_refs, &mut stamps);

        // Add resistive Jacobian stamps
        let mut jac = vec![0.0; self.device.num_resistive_jacobian_entries()];
        self.device.load_jacobian_resist(&self.model_data, &self.inst_data, &mut jac);
        let refs = self.device.get_resist_jac_refs(&self.node_refs);
        for (j, (r, c)) in refs.into_iter().enumerate() {
            if j < jac.len() && jac[j] != 0.0 {
                if let (Some(r), Some(c)) = (r, c) {
                    stamps.push(Stamp::Matrix(r, c, jac[j]));
                }
            }
        }
        stamps
    }
}

impl<D: AnalogDevice> AcAnalysis for DeviceRuntime<D> {
    fn load_ac(&mut self, dc_analysis_result: &DcAnalysisResult, _ac_analysis_context: &AcAnalysisContext, context: &Context) -> Vec<Stamp<AnalogReference, Complex64>> {
        self.eval_ac(dc_analysis_result, context);
        let mut stamps = Vec::new();
        let mut res_jac = vec![0.0; self.device.num_resistive_jacobian_entries()];
        self.device.load_jacobian_resist(&self.model_data, &self.inst_data, &mut res_jac);
        let res_refs = self.device.get_resist_jac_refs(&self.node_refs);
        for (j, (r, c)) in res_refs.into_iter().enumerate() {
            if j < res_jac.len() && res_jac[j] != 0.0 {
                if let (Some(r), Some(c)) = (r, c) {
                    stamps.push(Stamp::Matrix(r, c, Complex64::new(res_jac[j], 0.0)));
                }
            }
        }
        
        let mut react_jac = vec![0.0; self.device.num_reactive_jacobian_entries()];
        self.device.load_jacobian_react(&self.model_data, &self.inst_data, 1.0, &mut react_jac);
        let react_refs = self.device.get_react_jac_refs(&self.node_refs);
        for (j, (r, c)) in react_refs.into_iter().enumerate() {
            if j < react_jac.len() && react_jac[j] != 0.0 {
                if let (Some(r), Some(c)) = (r, c) {
                    stamps.push(Stamp::Matrix(r, c, Complex64::new(0.0, react_jac[j])));
                }
            }
        }
        stamps
    }
}

impl<D: AnalogDevice> TransientAnalysis for DeviceRuntime<D> {
    fn load_transient(
        &mut self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.eval_tran(circuit_states, context);
        let mut stamps = Vec::new();
        let dt: f64 = transient_analysis_context.dt.into();
        let alpha = 1.0 / dt; // Backward Euler alpha

        // Use the proper SPICE-style transient RHS which computes the combined
        // J*x - f(x) including both resistive and reactive terms with charge integration.
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| circuit_states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let mut rhs = [0.0f64; crate::analog::osdi::ffi::SCRATCH];
        self.device.load_spice_rhs_tran(&self.model_data, &self.inst_data, &mut rhs, &prev_solve, alpha);
        self.device.collect_rhs_stamps(&self.inst_data, &rhs, &self.node_refs, &mut stamps);

        // Add resistive Jacobian stamps
        let mut jac = vec![0.0; self.device.num_resistive_jacobian_entries()];
        self.device.load_jacobian_resist(&self.model_data, &self.inst_data, &mut jac);
        let refs = self.device.get_resist_jac_refs(&self.node_refs);
        for (j, (r, c)) in refs.into_iter().enumerate() {
            if j < jac.len() && jac[j] != 0.0 {
                if let (Some(r), Some(c)) = (r, c) {
                    stamps.push(Stamp::Matrix(r, c, jac[j]));
                }
            }
        }
        
        // Add reactive Jacobian stamps scaled by alpha (1/dt)
        let mut jac_react = vec![0.0; self.device.num_reactive_jacobian_entries()];
        self.device.load_jacobian_react(&self.model_data, &self.inst_data, 1.0, &mut jac_react);
        let react_refs = self.device.get_react_jac_refs(&self.node_refs);
        for (j, (r, c)) in react_refs.into_iter().enumerate() {
            if j < jac_react.len() && jac_react[j] != 0.0 {
                if let (Some(r), Some(c)) = (r, c) {
                    stamps.push(Stamp::Matrix(r, c, jac_react[j] * alpha));
                }
            }
        }
        stamps
    }
}

impl<D: AnalogDevice> NoiseSource for DeviceRuntime<D> {
    fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        let num_noise = self.device.num_noise_sources();
        if num_noise == 0 { return Vec::new(); }

        // Eval with AC + noise flags so inst_data reflects the DC operating point.
        let flags = SimFlags::ANALYSIS_AC | SimFlags::CALC_RESIST_JACOBIAN | SimFlags::CALC_REACT_JACOBIAN | SimFlags::CALC_NOISE;
        let paras = crate::analog::device::SimParams {
            ini_lim: false, gmin: 0.0, gdev: 0.0, tnom: 300.15, simulator_version: 1.0,
            source_scale_factor: 1.0, epsmin: 1e-12, reltol: 1e-3, vntol: 1e-6, abstol: 1e-12,
        };
        let prev_solve = self.device.build_prev_solve(&self.inst_data, &self.node_refs, &|k| {
            if let Some(r) = self.node_refs.iter().flatten().find(|r| r.idx() == Some(k)) {
                dc_point.get(r.variable().clone()).unwrap_or(0.0)
            } else {
                0.0
            }
        });
        let mut info = crate::analog::device::SimInfo {
            params: &paras, abstime: 0.0, prev_solve: &prev_solve,
            prev_state: &self.prev_state, next_state: &mut self.next_state, flags,
        };
        self.device.eval(&self.model_data, &mut self.inst_data, &mut info);

        let mut noise_rhs = vec![0.0f64; num_noise];
        self.device.load_noise(&self.model_data, &self.inst_data, ac_context.frequency, &mut noise_rhs);

        let node_pairs = self.device.noise_source_node_pairs();
        let mut noises = Vec::new();
        for (i, &psd) in noise_rhs.iter().enumerate() {
            if psd <= 0.0 { continue; }
            let (osdi_n1, osdi_n2) = node_pairs[i];
            let ref1 = self.node_refs.get(osdi_n1).and_then(|r| r.clone())
                .unwrap_or_else(AnalogReference::ground);
            let ref2 = self.node_refs.get(osdi_n2).and_then(|r| r.clone())
                .unwrap_or_else(AnalogReference::ground);
            noises.push(Noise { terminals: (ref1, ref2), value: psd });
        }
        noises
    }
}

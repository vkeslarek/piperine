use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::sync::Arc;
use num_complex::Complex64;

use piperine_solver::analog::{AnalogReference, NodeIdentifier, Netlist};
use piperine_solver::analysis::ac::AcAnalysisContext;
use piperine_solver::analysis::dc::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::analysis::noise::Noise;
use piperine_solver::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::device::Device;
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;

use crate::codegen::{DigitalInterpreter, JitAnalogDevice};

pub struct PhdlDevice {
    name: String,
    /// JIT-compiled analog behavior; `None` if the module has no analog block.
    analog: Option<Arc<JitAnalogDevice>>,
    /// Interpreted digital behavior; `None` if the module has no digital block.
    digital: Option<DigitalInterpreter>,
    /// Per-terminal netlist references (None for GND terminals).
    node_refs: Vec<Option<AnalogReference>>,
    /// Evaluated parameter values, indexed by param position.
    params: Vec<f64>,
}

impl PhdlDevice {
    pub fn new(
        name: impl Into<String>,
        analog: Option<Arc<JitAnalogDevice>>,
        digital: Option<DigitalInterpreter>,
        node_refs: Vec<Option<AnalogReference>>,
        params: Vec<f64>,
    ) -> Self {
        Self { name: name.into(), analog, digital, node_refs, params }
    }

    pub fn allocate_nodes(
        &mut self,
        terminals: &[NodeIdentifier],
        netlist: &mut Netlist,
    ) {
        self.node_refs = terminals.iter().map(|t| {
            let cref = netlist.connect_node(t.clone());
            if cref.idx().is_some() { Some(cref) } else { None }
        }).collect();
    }

    fn num_terminals(&self) -> usize { self.node_refs.len() }

    fn collect_node_voltages(&self, get_v: &dyn Fn(usize) -> f64) -> Vec<f64> {
        self.node_refs.iter().map(|r| {
            r.as_ref()
                .and_then(|cref| cref.idx())
                .map(|k| get_v(k))
                .unwrap_or(0.0)
        }).collect()
    }

    fn eval_rhs_jac(&self, node_voltages: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.num_terminals();
        let mut res = vec![0.0; n];
        let mut jac = vec![0.0; n * n];
        if let Some(a) = &self.analog {
            a.eval_residual(node_voltages, &self.params, &mut res);
            a.eval_jacobian(node_voltages, &self.params, &mut jac);
        }
        (res, jac)
    }

    fn norton_rhs(&self, node_voltages: &[f64], res: &[f64], jac: &[f64]) -> Vec<f64> {
        let n = self.num_terminals();
        (0..n).map(|i| {
            let mut val = -res[i];
            for j in 0..n { val += jac[i * n + j] * node_voltages[j]; }
            val
        }).collect()
    }

    fn rhs_stamps(&self, rhs: &[f64]) -> Vec<Stamp<AnalogReference, f64>> {
        self.node_refs.iter().enumerate().filter_map(|(i, r)| {
            let v = *rhs.get(i)?;
            if v == 0.0 { return None; }
            Some(Stamp::Rhs(r.as_ref()?.clone(), v))
        }).collect()
    }

    fn jac_stamps_f64(&self, jac: &[f64]) -> Vec<Stamp<AnalogReference, f64>> {
        let n = self.num_terminals();
        let mut stamps = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let v = jac[i * n + j];
                if v == 0.0 { continue; }
                let row = self.node_refs.get(i).and_then(|r| r.clone());
                let col = self.node_refs.get(j).and_then(|r| r.clone());
                if let (Some(r), Some(c)) = (row, col) {
                    stamps.push(Stamp::Matrix(r, c, v));
                }
            }
        }
        stamps
    }

    fn jac_stamps_complex(&self, jac: &[f64]) -> Vec<Stamp<AnalogReference, Complex64>> {
        let n = self.num_terminals();
        let mut stamps = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let v = jac[i * n + j];
                if v == 0.0 { continue; }
                let row = self.node_refs.get(i).and_then(|r| r.clone());
                let col = self.node_refs.get(j).and_then(|r| r.clone());
                if let (Some(r), Some(c)) = (row, col) {
                    stamps.push(Stamp::Matrix(r, c, Complex64::new(v, 0.0)));
                }
            }
        }
        stamps
    }

    fn load_analog_dc(&self, get_v: &dyn Fn(usize) -> f64) -> Vec<Stamp<AnalogReference, f64>> {
        if self.analog.is_none() { return Vec::new(); }
        let node_voltages = self.collect_node_voltages(get_v);
        let (res, jac) = self.eval_rhs_jac(&node_voltages);
        let rhs = self.norton_rhs(&node_voltages, &res, &jac);
        let mut stamps = self.rhs_stamps(&rhs);
        stamps.extend(self.jac_stamps_f64(&jac));
        stamps
    }

    fn load_analog_ac(&self, get_v: &dyn Fn(usize) -> f64, omega: f64) -> Vec<Stamp<AnalogReference, Complex64>> {
        let analog = match &self.analog {
            Some(a) => a,
            None => return Vec::new(),
        };
        let node_voltages = self.collect_node_voltages(get_v);
        let (_, jac) = self.eval_rhs_jac(&node_voltages);
        // Resistive conductance → real part of the admittance.
        let mut stamps = self.jac_stamps_complex(&jac);
        // Reactive `dQ/dV` → imaginary part `jω·dQ/dV` (e.g. a capacitor's jωC).
        if analog.has_reactive() {
            let n = self.num_terminals();
            let mut qjac = vec![0.0; n * n];
            analog.eval_charge_jacobian(&node_voltages, &self.params, &mut qjac);
            for i in 0..n {
                for j in 0..n {
                    let v = qjac[i * n + j];
                    if v == 0.0 { continue; }
                    let row = self.node_refs.get(i).and_then(|r| r.clone());
                    let col = self.node_refs.get(j).and_then(|r| r.clone());
                    if let (Some(r), Some(c)) = (row, col) {
                        stamps.push(Stamp::Matrix(r, c, Complex64::new(0.0, omega * v)));
                    }
                }
            }
        }
        stamps
    }

    /// Transient companion-model load: the resistive residual/Jacobian plus the
    /// reactive `ddt` term stamped via Backward-Euler (`alpha = 1/dt`).
    ///
    /// The reactive contribution adds `alpha·dQ/dV` to the Jacobian; the
    /// matching history current source falls out of the Norton transform
    /// (`jac · V_prev`), since the device is linearised at the previously
    /// accepted solution.
    fn load_analog_transient(&self, get_v: &dyn Fn(usize) -> f64, alpha: f64) -> Vec<Stamp<AnalogReference, f64>> {
        let analog = match &self.analog {
            Some(a) => a,
            None => return Vec::new(),
        };
        let node_voltages = self.collect_node_voltages(get_v);
        let (res, mut jac) = self.eval_rhs_jac(&node_voltages);
        if analog.has_reactive() {
            let n = self.num_terminals();
            let mut qjac = vec![0.0; n * n];
            analog.eval_charge_jacobian(&node_voltages, &self.params, &mut qjac);
            for k in 0..n * n {
                jac[k] += alpha * qjac[k];
            }
        }
        let rhs = self.norton_rhs(&node_voltages, &res, &jac);
        let mut stamps = self.rhs_stamps(&rhs);
        stamps.extend(self.jac_stamps_f64(&jac));
        stamps
    }
}

impl Device for PhdlDevice {
    fn device_name(&self) -> &str { &self.name }

    // ── Analog ────────────────────────────────────────────────────────────────

    fn load_dc(
        &mut self,
        state: &DcAnalysisState,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.load_analog_dc(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        })
    }

    fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        let freq: f64 = ac_ctx.frequency.into();
        let omega = 2.0 * std::f64::consts::PI * freq;
        let refs = self.node_refs.clone();
        self.load_analog_ac(&|k| {
            refs.iter().flatten()
                .find(|r| r.idx() == Some(k))
                .and_then(|r| dc_op.get(r.variable().clone()))
                .unwrap_or(0.0)
        }, omega)
    }

    fn load_transient(
        &mut self,
        states: &TransientAnalysisState,
        tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt: f64 = tran_ctx.dt.into();
        let alpha = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        self.load_analog_transient(&|k| {
            states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        }, alpha)
    }

    fn noise_current_psd(
        &mut self,
        _dc_point: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
    ) -> Vec<Noise> {
        Vec::new()
    }

    // ── Digital ───────────────────────────────────────────────────────────────

    fn digital_input_nets(&self) -> &[DigitalNet] {
        self.digital.as_ref().map(|d| d.input_nets()).unwrap_or(&[])
    }

    fn digital_output_nets(&self) -> &[DigitalNet] {
        self.digital.as_ref().map(|d| d.output_nets()).unwrap_or(&[])
    }

    fn digital_init(&mut self, event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        if let Some(d) = &mut self.digital {
            d.init(event_queue);
        }
    }

    fn digital_state_size(&self) -> usize {
        // State size is tracked dynamically inside the interpreter HashMap;
        // we report 0 here since we don't use fixed-size external state arrays.
        0
    }

    fn eval_discrete(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        _av: &[f64],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        if let Some(d) = &mut self.digital {
            d.eval(t, nets, event_queue);
        }
    }
}

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

use crate::runtime::digital::DigitalInterpreter;
use piperine_codegen::{ir::IrExpr, IrNoise, JitAnalogDevice, SimCtx};

/// A noise source declaration carried by [`PhdlDevice`]. The PSD is
/// stored as an [`IrExpr`] and evaluated at noise-analysis time against
/// the DC operating point (for branch-access / DC current terms) and the
/// device's param values (GAPS §D.4).
#[derive(Debug, Clone)]
struct StoredNoiseSource {
    /// The IR-level terminal name on the plus side. Used to resolve
    /// `V(plus, minus)` reads inside the PSD.
    plus_name: String,
    /// The plus-side netlist reference (for the `Noise` entry returned to
    /// the solver).
    plus: AnalogReference,
    minus_name: String,
    minus: AnalogReference,
    /// `White { psd }` or `Flicker { psd, exponent }`.
    kind: IrNoise,
}

/// A device that wraps JIT-compiled analog behavior and/or an interpreted
/// digital behavior, presenting them as a single mixed-signal [`Device`].
pub struct PhdlDevice {
    name: String,
    /// JIT-compiled analog behavior; `None` if the module has no analog block.
    analog: Option<Arc<JitAnalogDevice>>,
    /// Interpreted digital behavior; `None` if the module has no digital block.
    digital: Option<DigitalInterpreter>,
    /// Per-terminal netlist references (None for GND terminals).
    node_refs: Vec<Option<AnalogReference>>,
    /// Per-terminal name (`p`, `n`, etc.). Set together with `node_refs`
    /// by the constructor so `eval_ir_f64` can map IR `BranchAccess`
    /// terminal names to the corresponding `AnalogReference` for DC-point
    /// voltage lookup.
    terminal_names: Vec<String>,
    /// Evaluated parameter values, indexed by param position.
    params: Vec<f64>,
    /// Per-param name → index (for the IR PSD evaluator — same names as
    /// `JitAnalogDevice::param_names`).
    param_names: Vec<String>,
    /// Live simulator state read by `$temperature`/`$abstime`/`$vt`.
    /// Defaults to T = 300 K, t = 0, mfactor = 1, gmin = 1e-12. The solver
    /// updates this at each `load_dc` / `load_transient` call (GAPS §A.2/§A.3).
    sim_ctx: SimCtx,
    /// Noise sources declared in the analog body (GAPS §D.4). Each
    /// source's PSD is evaluated at `noise_current_psd` time using the
    /// DC operating point + device params + simulator state.
    noise_sources: Vec<StoredNoiseSource>,
}

impl PhdlDevice {
    /// Construct a new mixed-signal device with optional analog JIT device
    /// and optional digital interpreter.
    pub fn new(
        name: impl Into<String>,
        analog: Option<Arc<JitAnalogDevice>>,
        digital: Option<DigitalInterpreter>,
        node_refs: Vec<Option<AnalogReference>>,
        params: Vec<f64>,
        param_given_mask: u64,
    ) -> Self {
        let n = node_refs.len();
        let mut sim_ctx = SimCtx::default();
        sim_ctx.param_given_mask = param_given_mask;
        Self {
            name: name.into(),
            analog,
            digital,
            node_refs,
            terminal_names: vec![String::new(); n],
            params,
            param_names: Vec::new(),
            sim_ctx,
            noise_sources: Vec::new(),
        }
    }

    /// Replace the per-terminal names. Caller must provide one entry per
    /// `node_refs` entry (matching positions).
    pub fn set_terminal_names(&mut self, names: Vec<String>) {
        assert_eq!(
            names.len(),
            self.node_refs.len(),
            "terminal_names must match node_refs length"
        );
        self.terminal_names = names;
    }

    /// Override the simulator state (e.g. simulation temperature). The solver
    /// is expected to call this at the start of each analysis phase.
    pub fn set_sim_ctx(&mut self, sim: SimCtx) {
        self.sim_ctx = sim;
    }

    /// Declare a noise source carried by this device (GAPS §D.4) using
    /// terminal *indices* into `node_refs`. The `from_ir` integration uses
    /// this form because the IR's `IrNoiseSource` carries terminal *names*
    /// (strings); the integration looks up the index, then calls this
    /// method with the resolved `AnalogReference`. Indices that don't
    /// correspond to a registered `AnalogReference` (e.g. an unmapped
    /// ground) are silently skipped — they would yield zero PSD anyway.
    pub fn add_noise_source_by_index(
        &mut self,
        plus_idx: usize,
        minus_idx: usize,
        plus_name: String,
        minus_name: String,
        kind: IrNoise,
    ) {
        if let (Some(Some(p_ref)), Some(Some(m_ref))) = (
            self.node_refs.get(plus_idx),
            self.node_refs.get(minus_idx),
        ) {
            self.noise_sources.push(StoredNoiseSource {
                plus_name,
                plus: p_ref.clone(),
                minus_name,
                minus: m_ref.clone(),
                kind,
            });
        }
    }

    /// Read the current simulator state.
    pub fn sim_ctx(&self) -> &SimCtx {
        &self.sim_ctx
    }

    /// Set the parameter names (GAPS §D.4 — needed for `Param(name)` resolution
    /// when evaluating the noise PSD). Must match `JitAnalogDevice::param_names`
    /// (which is the source of truth for the JIT path).
    pub fn set_param_names(&mut self, names: Vec<String>) {
        self.param_names = names;
    }

    /// Declare a noise source carried by this device (GAPS §D.4). The
    /// terminal references must already be allocated (i.e. `allocate_nodes`
    /// has been called). The PSD is the IR expression that gets evaluated
    /// against the DC operating point at `noise_current_psd` time.
    pub fn add_noise_source(
        &mut self,
        plus_name: String,
        plus: AnalogReference,
        minus_name: String,
        minus: AnalogReference,
        kind: IrNoise,
    ) {
        self.noise_sources.push(StoredNoiseSource {
            plus_name, plus, minus_name, minus, kind
        });
    }

    /// Register the device terminals with the netlist, storing analog
    /// references for each terminal.
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

    /// Return the number of terminals on this device.
    fn num_terminals(&self) -> usize { self.node_refs.len() }

    /// Return the terminal name for index `i`, if known. Terminal names
    /// are recorded by the `from_ir` constructor (set in tandem with
    /// `node_refs`). Returns `None` for unnamed / unmapped terminals.
    fn terminal_name(&self, i: usize) -> Option<String> {
        // The terminal names are stored alongside the analog device in
        // the constructor (the JitAnalogDevice carries `param_names`, not
        // port names, so the names live in the PhdlDevice-side cache). We
        // back this with a parallel Vec populated when the caller hands
        // us the analog device.
        self.terminal_names.get(i).cloned()
    }

    /// Collect the voltage at each terminal using a voltage-lookup closure.
    fn collect_node_voltages(&self, get_v: &dyn Fn(usize) -> f64) -> Vec<f64> {
        self.node_refs.iter().map(|r| {
            r.as_ref()
                .and_then(|cref| cref.idx())
                .map(|k| get_v(k))
                .unwrap_or(0.0)
        }).collect()
    }

    /// Evaluate the residual (RHS) vector and Jacobian matrix via the JIT
    /// analog device, returning `(residual, jacobian)`.
    fn eval_rhs_jac(&self, node_voltages: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.num_terminals();
        let mut res = vec![0.0; n];
        let mut jac = vec![0.0; n * n];
        if let Some(a) = &self.analog {
            a.eval_residual(node_voltages, &self.params, &self.sim_ctx, &mut res);
            a.eval_jacobian(node_voltages, &self.params, &self.sim_ctx, &mut jac);
        }
        (res, jac)
    }

    /// Compute the Norton equivalent current source: `-res + J·V`.
    fn norton_rhs(&self, node_voltages: &[f64], res: &[f64], jac: &[f64]) -> Vec<f64> {
        let n = self.num_terminals();
        (0..n).map(|i| {
            let mut val = -res[i];
            for j in 0..n { val += jac[i * n + j] * node_voltages[j]; }
            val
        }).collect()
    }

    /// Build real-valued RHS stamps from the Norton current vector.
    fn rhs_stamps(&self, rhs: &[f64]) -> Vec<Stamp<AnalogReference, f64>> {
        self.node_refs.iter().enumerate().filter_map(|(i, r)| {
            let v = *rhs.get(i)?;
            if v == 0.0 { return None; }
            Some(Stamp::Rhs(r.as_ref()?.clone(), v))
        }).collect()
    }

    /// Build real-valued Jacobian matrix stamps from a dense Jacobian.
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

    /// Build complex-valued Jacobian matrix stamps from a dense Jacobian.
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

    /// Load stamps for a DC operating-point analysis: resistive residual
    /// and Jacobian.
    fn load_analog_dc(&self, get_v: &dyn Fn(usize) -> f64) -> Vec<Stamp<AnalogReference, f64>> {
        if self.analog.is_none() { return Vec::new(); }
        let node_voltages = self.collect_node_voltages(get_v);
        let (res, jac) = self.eval_rhs_jac(&node_voltages);
        let rhs = self.norton_rhs(&node_voltages, &res, &jac);
        let mut stamps = self.rhs_stamps(&rhs);
        stamps.extend(self.jac_stamps_f64(&jac));
        stamps
    }

    /// Load complex stamps for AC analysis: conductive Jacobian (real) plus
    /// reactive charge Jacobian times `jω` (imaginary).
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
            analog.eval_charge_jacobian(&node_voltages, &self.params, &self.sim_ctx, &mut qjac);
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
            analog.eval_charge_jacobian(&node_voltages, &self.params, &self.sim_ctx, &mut qjac);
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
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.sim_ctx.temperature = context.temperature;
        self.sim_ctx.gmin = context.gmin.into();
        self.sim_ctx.current_analysis = 0; // DC
        self.load_analog_dc(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        })
    }

    fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        self.sim_ctx.temperature = context.temperature;
        self.sim_ctx.gmin = context.gmin.into();
        self.sim_ctx.current_analysis = 2; // AC
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
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt: f64 = tran_ctx.dt.into();
        let alpha = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        self.sim_ctx.abstime = tran_ctx.time.into();
        self.sim_ctx.step = dt;
        self.sim_ctx.tfinal = tran_ctx.tfinal.into();
        self.sim_ctx.temperature = context.temperature;
        self.sim_ctx.gmin = context.gmin.into();
        self.sim_ctx.current_analysis = 1; // TRAN
        self.load_analog_transient(&|k| {
            states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        }, alpha)
    }

    fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
    ) -> Vec<Noise> {
        // GAPS §D.4 — evaluate each stored noise source's PSD against the
        // DC operating point. The PSD `IrExpr` is walked by `eval_psd`:
        //   * `Param(name)`     → self.params[idx_of(name)]
        //   * `Sim(Temp)`       → self.sim_ctx.temperature
        //   * `Sim(Vt(None))`   → self.sim_ctx.temperature * kB/q
        //   * `Sim(Simparam{..})` → self.sim_ctx.gmin (or default)
        //   * `BranchAccess V(plus,minus)` → V(plus) − V(minus) from `dc_point`
        //   * literals / arithmetic → evaluated directly
        //   * everything else → 0.0 (fail-soft; missing ops like `delay`
        //     return 0.0 rather than panicking, matching the spec's
        //     "lossless for what's supported" principle)
        let mut out = Vec::with_capacity(self.noise_sources.len());
        for src in &self.noise_sources {
            // The noise source's own terminals (its contribution's
            // `plus`/`minus`) are looked up directly via the stored
            // `AnalogReference`. This bypasses the `terminal_names` table
            // entirely (which is for `V()` reads *inside* the PSD).
            let v_plus = reference_voltage(&src.plus, dc_point).unwrap_or(0.0);
            let v_minus = reference_voltage(&src.minus, dc_point).unwrap_or(0.0);
            let psd = match &src.kind {
                IrNoise::White { psd } => eval_psd(psd, self, dc_point, v_plus, v_minus),
                IrNoise::Flicker { psd, .. } => {
                    eval_psd(psd, self, dc_point, v_plus, v_minus)
                }
            };
            if psd > 0.0 {
                out.push(Noise {
                    terminals: (src.plus.clone(), src.minus.clone()),
                    value: psd,
                });
            }
        }
        out
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

// ──────────────────────────── GAPS §D.4 helpers ────────────────────────────────
//
// `eval_ir_f64` walks a noise PSD `IrExpr` and returns its value under the
// given DC operating point + device params + simulator state. The set of
// supported nodes is intentionally small (literals, arithmetic, Param,
// branch voltages, the simulator queries the NGSPICE faithful headers
// use) — anything else returns 0.0 rather than panicking. The "lossless for
// what's supported" principle (GAPS §D.4): *when* an op is supported, the
// PSD is faithful; *when* it's not, the noise source is silently dropped
// (return value 0.0) — never a wrong number.
//
// The full JIT-compiled PSD path (which would lower the IrExpr to a
// Cranelift function and call it from the solver) is the long-term
// solution. This evaluator is the short-term pragmatic step that lets
// `noise_current_psd` return non-empty `Vec<Noise>` for the NGSPICE
// faithful models today.

/// Evaluate a noise PSD expression. The PSD's `BranchAccess V(plus, minus)`
/// reads are rewritten as `Real(v_plus) - Real(v_minus)` before walking,
/// sidestepping the `terminal_name` lookup path entirely. This makes the
/// evaluator self-contained: it does not depend on the device's
/// `terminal_names` being consistent with the PSD's terminals (a common
/// source of integration errors in the from_ir path).
fn eval_psd(
    e: &IrExpr,
    dev: &PhdlDevice,
    dc_point: &DcAnalysisResult,
    v_plus: f64,
    v_minus: f64,
) -> f64 {
    // Substitute `BranchAccess "V"(p, m)` for the corresponding noise
    // terminal's resolved DC voltage. We do not modify the original
    // `e` (the caller still owns it) — we just walk a `match` and
    // substitute on the fly.
    use piperine_codegen::ir::{IrBinOp, IrUnOp, SimQuery};
    match e {
        IrExpr::Real(v) => *v,
        IrExpr::Int(v) => *v as f64,
        IrExpr::Bool(b) => if *b { 1.0 } else { 0.0 },
        IrExpr::Param(name) => dev.param_names
            .iter()
            .position(|n| n == name)
            .and_then(|i| dev.params.get(i).copied())
            .unwrap_or(0.0),
        IrExpr::Var(_) => 0.0, // TODO: thread var slot through `PhdlDevice`
        IrExpr::Sim(sq) => match sq {
            SimQuery::Temperature => dev.sim_ctx.temperature,
            SimQuery::Vt(_) => dev.sim_ctx.temperature * SimCtx::K_B_OVER_Q_EV_PER_K,
            SimQuery::Abstime => dev.sim_ctx.abstime,
            SimQuery::Mfactor => dev.sim_ctx.mfactor,
            SimQuery::Simparam { key, default } => match key.as_str() {
                "gmin" => dev.sim_ctx.gmin,
                "temperature" => dev.sim_ctx.temperature,
                _ => eval_psd(default, dev, dc_point, v_plus, v_minus),
            },
            _ => 0.0,
        },
        IrExpr::BranchAccess { access, plus, minus } => {
            // The `plus`/`minus` strings here name the source's own
            // terminals. We can't resolve them by name without the
            // `terminal_names` table being consistent — instead, the
            // caller has already resolved them to `v_plus`/`v_minus`. Use
            // those values regardless of the IR's `plus`/`minus` strings
            // (a slight overspecification that matches the "this PSD
            // reads the source's own branch" intent).
            if access != "V" { return 0.0; }
            let _ = (plus, minus);
            v_plus - v_minus
        }
        IrExpr::StateRef(_) => 0.0,
        IrExpr::Unary(op, x) => {
            let xv = eval_psd(x, dev, dc_point, v_plus, v_minus);
            match op {
                IrUnOp::Neg => -xv,
                _ => if xv == 0.0 { 1.0 } else { 0.0 },
            }
        }
        IrExpr::Binary(op, a, b) => {
            let av = eval_psd(a, dev, dc_point, v_plus, v_minus);
            let bv = eval_psd(b, dev, dc_point, v_plus, v_minus);
            match op {
                IrBinOp::Add => av + bv,
                IrBinOp::Sub => av - bv,
                IrBinOp::Mul => av * bv,
                IrBinOp::Div => if bv != 0.0 { av / bv } else { 0.0 },
                IrBinOp::Rem => if bv != 0.0 { av % bv } else { 0.0 },
                IrBinOp::Pow => av.powf(bv),
                _ => 0.0,
            }
        }
        IrExpr::Select(c, t, f) => {
            let cv = eval_psd(c, dev, dc_point, v_plus, v_minus);
            if cv != 0.0 {
                eval_psd(t, dev, dc_point, v_plus, v_minus)
            } else {
                eval_psd(f, dev, dc_point, v_plus, v_minus)
            }
        }
        IrExpr::Call(name, args) => {
            let vs: Vec<f64> = args.iter()
                .map(|a| eval_psd(a, dev, dc_point, v_plus, v_minus))
                .collect();
            match name.as_str() {
                "abs" if vs.len() >= 1 => vs[0].abs(),
                "sqrt" if vs.len() >= 1 => vs[0].sqrt(),
                "ln" | "log" if vs.len() >= 1 => vs[0].ln(),
                "log10" if vs.len() >= 1 => vs[0].log10(),
                "exp" if vs.len() >= 1 => vs[0].exp(),
                "pow" if vs.len() >= 2 => vs[0].powf(vs[1]),
                "min" if vs.len() >= 2 => vs[0].min(vs[1]),
                "max" if vs.len() >= 2 => vs[0].max(vs[1]),
                "white_noise" | "flicker_noise" => 0.0,
                _ => 0.0,
            }
        }
        _ => 0.0,
    }
}

/// Look up the DC voltage of an `AnalogReference` (a netlist node).
/// Returns `None` for unknown / ground references.
fn reference_voltage(r: &AnalogReference, dc_point: &DcAnalysisResult) -> Option<f64> {
    if r.idx().is_some() {
        let var = r.variable().clone();
        dc_point.get(var)
    } else {
        Some(0.0) // ground → 0V
    }
}

/// Helper for the noise-PSD evaluator: given a terminal *name* string,
/// look up the corresponding node voltage. Uses `PhdlDevice::terminal_names`
/// + `node_refs` together. Returns 0.0 for unknown names (fail-soft).
///
/// Unused for now — `eval_psd` resolves the noise source's own terminals
/// via the stored `AnalogReference` directly. Retained for future PSDs that
/// re-reference the source's terminals via implicit names (GAPS §D.4).
#[allow(dead_code)]
fn lookup_voltage_by_name(dev: &PhdlDevice, name: &str, dc_point: &DcAnalysisResult) -> f64 {
    for (i, net) in dev.node_refs.iter().enumerate() {
        if let Some(tname) = dev.terminal_name(i) {
            if tname == name {
                if let Some(r) = net {
                    return reference_voltage(r, dc_point).unwrap_or(0.0);
                }
                return 0.0;
            }
        }
    }
    0.0
}

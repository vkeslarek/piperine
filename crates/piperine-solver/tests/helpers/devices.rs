//! Hand-written `Element`s that stand in for the Verilog-A modules the five
//! formerly codegen-driven targets (`sens`, `adjoint_dc`, `adjoint_vs_sens`,
//! `transient_reentry`, `pss`) compiled. Each element quotes the module text
//! it replaces and reproduces its constants, so a ported test keeps its
//! circuit and its assertions. Circuits are assembled with `CircuitBuilder`.

use std::collections::HashMap;

use piperine_solver::abi::{
    AnalogDevice, AnalogVariable, CircularArrayBuffer2, DcAnalysisState, DigitalDevice,
    DigitalPorts, Element, ElementCapabilities, EvalCtx, EventSink, Introspect, NodeIdentifier,
    Stamp, TrBdf2, TrBdf2Phase, TransientAnalysisContext, TransientAnalysisState,
    UnknownAllocator,
};
use piperine_solver::abi::{DigitalNet, LogicValue};
use piperine_solver::prelude::{
    AnalogReference, Bounds, CircuitInstance, Context, Invalidation, Net, ParamDescriptor,
    ParamError, ParamScope, Value, ValueKind,
};

type Stamps = Vec<Stamp<AnalogReference, f64>>;

/// The solver constant every junction module of the corpus uses.
const THERMAL_VOLTAGE: f64 = 0.02585;
/// The literal the Verilog-A sources write for pi.
const PI_LITERAL: f64 = 3.14159265358979;

// ---------------------------------------------------------------------------
// Circuit assembly helpers
// ---------------------------------------------------------------------------

/// Name -> reference for the nodes a test builds (the `info.nets` map the
/// codegen path returned).
pub struct NodeMap {
    nodes: HashMap<String, AnalogReference>,
}

impl NodeMap {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    pub fn insert(&mut self, name: &str, reference: AnalogReference) {
        self.nodes.insert(name.to_string(), reference);
    }

    pub fn get(&self, name: &str) -> &AnalogReference {
        self.nodes.get(name).unwrap_or_else(|| panic!("node `{name}` not mapped"))
    }

    /// The identifier results are keyed by.
    pub fn id(&self, name: &str) -> NodeIdentifier {
        node_id(self.get(name))
    }
}

/// The `NodeIdentifier` behind a node reference.
pub fn node_id(reference: &AnalogReference) -> NodeIdentifier {
    match reference.variable().as_ref() {
        AnalogVariable::Node(id) => id.clone(),
        other => panic!("not a node reference: {other:?}"),
    }
}

/// The solver `Net` of a mapped node.
pub fn net_for(circuit: &CircuitInstance, map: &NodeMap, name: &str) -> Net {
    let var = map.get(name).variable().clone();
    circuit
        .nets()
        .into_iter()
        .find(|n| n.analog_variable().map(|v| **v == *var).unwrap_or(false))
        .unwrap_or_else(|| panic!("net `{name}` not solved"))
}

// ---------------------------------------------------------------------------
// Parameter plumbing (plain functions, no macros)
// ---------------------------------------------------------------------------

fn real_param(name: &str, default: f64, unit: &str, invalidation: Invalidation) -> ParamDescriptor {
    ParamDescriptor {
        name: name.into(),
        kind: ValueKind::Real,
        default: Value::Real(default),
        unit: Some(unit.into()),
        bounds: Bounds::UNBOUNDED,
        scope: ParamScope::Instance,
        invalidation,
        desc: None,
    }
}

fn write_real(
    name: &str,
    value: Value,
    slot: &mut f64,
    invalidation: Invalidation,
) -> Result<Invalidation, ParamError> {
    let Some(v) = value.as_real() else {
        return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
    };
    *slot = v;
    Ok(invalidation)
}

// ---------------------------------------------------------------------------
// Stamp helpers
// ---------------------------------------------------------------------------

fn conductance(p: &AnalogReference, n: &AnalogReference, g: f64) -> Stamps {
    vec![
        Stamp::Matrix(p.clone(), p.clone(), g),
        Stamp::Matrix(n.clone(), n.clone(), g),
        Stamp::Matrix(p.clone(), n.clone(), -g),
        Stamp::Matrix(n.clone(), p.clone(), -g),
    ]
}

fn branch_force(
    p: &AnalogReference,
    n: &AnalogReference,
    branch: &AnalogReference,
    value: f64,
) -> Stamps {
    vec![
        Stamp::Matrix(p.clone(), branch.clone(), 1.0),
        Stamp::Matrix(branch.clone(), p.clone(), 1.0),
        Stamp::Matrix(n.clone(), branch.clone(), -1.0),
        Stamp::Matrix(branch.clone(), n.clone(), -1.0),
        Stamp::Rhs(branch.clone(), value),
    ]
}

fn volts(row: Option<ndarray::ArrayView1<'_, f64>>, node: &AnalogReference) -> f64 {
    node.idx().and_then(|i| row.as_ref().and_then(|r| r.get(i).copied())).unwrap_or(0.0)
}

fn drop_across(
    row: Option<ndarray::ArrayView1<'_, f64>>,
    p: &AnalogReference,
    n: &AnalogReference,
) -> f64 {
    let vp = p.idx().and_then(|i| row.as_ref().and_then(|r| r.get(i).copied())).unwrap_or(0.0);
    vp - volts(row, n)
}

// ---------------------------------------------------------------------------
// R
// ---------------------------------------------------------------------------

/// ```verilog
/// module R (inout electrical p, inout electrical n);
///     parameter real r = 1.0e3;
///     analog I(p, n) <+ V(p, n) / r;
/// endmodule
/// ```
pub struct R {
    label: String,
    r: f64,
    p: AnalogReference,
    n: AnalogReference,
    /// `V(p, n)` at the last `update`, the point `param_stamps` reads.
    vd: f64,
}

impl R {
    pub fn new(label: &str, r: f64, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), r, p, n, vd: 0.0 }
    }
}

impl AnalogDevice for R {
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, _ctx: &Context) {
        self.vd = drop_across(state.latest(), &self.p, &self.n);
    }

    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        conductance(&self.p, &self.n, 1.0 / self.r)
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        conductance(&self.p, &self.n, 1.0 / self.r)
    }

    /// `F_p = (V_p - V_n) / r`, so `dF_p/dr = -(V_p - V_n) / r^2`, and the
    /// opposite at `n`.
    fn param_stamps(&mut self, param_index: usize, sink: &mut Stamps) {
        if param_index == 0 {
            let d = -self.vd / (self.r * self.r);
            sink.push(Stamp::Rhs(self.p.clone(), d));
            sink.push(Stamp::Rhs(self.n.clone(), -d));
        }
    }
}

impl DigitalDevice for R {}

impl Introspect for R {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![real_param("r", 1.0e3, "ohm", Invalidation::Restamp)]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "r").then_some(Value::Real(self.r))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "r" => write_real(name, value, &mut self.r, Invalidation::Restamp),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for R {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_SENSITIVITY
    }
}

// ---------------------------------------------------------------------------
// ROpt
// ---------------------------------------------------------------------------

/// ```verilog
/// module ROpt (inout electrical p, inout electrical n);
///     parameter real r = 1.0e3;
///     parameter real w = 1.0e-3;
///     real g;
///     analog begin
///         g = 1.0e-3;
///         if ($param_given("w")) g = w;
///         I(p, n) <+ V(p, n) * g;
///     end
/// endmodule
/// ```
///
/// `w` is presence-queried: when it was not given at build, a write flips the
/// given-mask, which is a rebuild (`Invalidation::Rebuild`).
pub struct ROpt {
    label: String,
    r: f64,
    w: f64,
    w_given: bool,
    p: AnalogReference,
    n: AnalogReference,
}

impl ROpt {
    /// `w` is `Some` when the instance gave it, `None` when it kept the default.
    pub fn new(label: &str, w: Option<f64>, p: AnalogReference, n: AnalogReference) -> Self {
        Self {
            label: label.into(),
            r: 1.0e3,
            w: w.unwrap_or(1.0e-3),
            w_given: w.is_some(),
            p,
            n,
        }
    }

    fn g(&self) -> f64 {
        if self.w_given { self.w } else { 1.0e-3 }
    }
}

impl AnalogDevice for ROpt {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        conductance(&self.p, &self.n, self.g())
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        conductance(&self.p, &self.n, self.g())
    }
}

impl DigitalDevice for ROpt {}

impl Introspect for ROpt {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        let w_invalidation =
            if self.w_given { Invalidation::Restamp } else { Invalidation::Rebuild };
        vec![
            real_param("r", 1.0e3, "ohm", Invalidation::Restamp),
            real_param("w", 1.0e-3, "S", w_invalidation),
        ]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        match name {
            "r" => Some(Value::Real(self.r)),
            "w" => Some(Value::Real(self.w)),
            _ => None,
        }
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "r" => write_real(name, value, &mut self.r, Invalidation::Restamp),
            "w" if self.w_given => write_real(name, value, &mut self.w, Invalidation::Restamp),
            "w" => write_real(name, value, &mut self.w, Invalidation::Rebuild),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for ROpt {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
}

// ---------------------------------------------------------------------------
// C
// ---------------------------------------------------------------------------

/// ```verilog
/// module C (inout electrical p, inout electrical n);
///     parameter real c = 1.0e-6;
///     analog I(p, n) <+ c * ddt(V(p, n));
/// endmodule
/// ```
///
/// The adjoint target's variant of this module has an empty analog block; at
/// DC both contribute nothing, so one element serves both. The transient
/// companion is the TR-BDF2 stamping of `TrBdf2::stage_coeffs`.
pub struct C {
    label: String,
    c: f64,
    p: AnalogReference,
    n: AnalogReference,
}

impl C {
    pub fn new(label: &str, c: f64, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), c, p, n }
    }
}

impl AnalogDevice for C {
    fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        if t.h <= 0.0 {
            return Vec::new();
        }
        let (c0, c1, c2) = TrBdf2::stage_coeffs(t.phase, t.h, t.prev_h);
        let q_at = |lookback: usize| -> f64 {
            let row = if lookback == 0 { states.latest() } else { states.view(lookback) };
            self.c * drop_across(row, &self.p, &self.n)
        };
        let q_now = q_at(0);
        let q_prev = q_at(1);
        let q_prev2 = q_at(2);
        let mut i_c = c0 * q_now + c1 * q_prev + c2 * q_prev2;
        if matches!(t.phase, TrBdf2Phase::Trapezoidal) && t.prev_h > 0.0 {
            let (d0, d1, d2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, t.prev_h);
            i_c -= d0 * q_prev + d1 * q_prev2 + d2 * q_at(3);
        }
        let rhs = c0 * q_now - i_c;
        let mut stamps = conductance(&self.p, &self.n, c0 * self.c);
        stamps.push(Stamp::Rhs(self.p.clone(), rhs));
        stamps.push(Stamp::Rhs(self.n.clone(), -rhs));
        stamps
    }

    /// The charge term is invisible at DC: `dF/dc = 0`, so nothing is pushed.
    fn param_stamps(&mut self, _param_index: usize, _sink: &mut Stamps) {}
}

impl DigitalDevice for C {}

impl Introspect for C {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![real_param("c", 1.0e-6, "F", Invalidation::Restamp)]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "c").then_some(Value::Real(self.c))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "c" => write_real(name, value, &mut self.c, Invalidation::Restamp),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for C {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_SENSITIVITY
    }
}

// ---------------------------------------------------------------------------
// D
// ---------------------------------------------------------------------------

/// ```verilog
/// module D (inout electrical p, inout electrical n);
///     parameter real is_sat = 1.0e-14;
///     analog I(p, n) <+ is_sat * (exp(V(p, n) / 0.02585) - 1.0);
/// endmodule
/// ```
pub struct D {
    label: String,
    is_sat: f64,
    p: AnalogReference,
    n: AnalogReference,
    vd: f64,
}

impl D {
    pub fn new(label: &str, is_sat: f64, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), is_sat, p, n, vd: 0.0 }
    }

    fn norton(&self, vd: f64) -> Stamps {
        let ex = (vd / THERMAL_VOLTAGE).exp();
        let id = self.is_sat * (ex - 1.0);
        let gd = self.is_sat / THERMAL_VOLTAGE * ex;
        let ieq = id - gd * vd;
        let mut stamps = conductance(&self.p, &self.n, gd);
        stamps.push(Stamp::Rhs(self.p.clone(), -ieq));
        stamps.push(Stamp::Rhs(self.n.clone(), ieq));
        stamps
    }
}

impl AnalogDevice for D {
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, _ctx: &Context) {
        self.vd = drop_across(state.latest(), &self.p, &self.n);
    }

    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        self.norton(drop_across(s.latest(), &self.p, &self.n))
    }

    fn load_transient(
        &mut self,
        s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        self.norton(drop_across(s.latest(), &self.p, &self.n))
    }

    /// `F_p = is_sat * (exp(vd / vt) - 1)`, so `dF_p/d is_sat = exp(vd / vt) - 1`.
    fn param_stamps(&mut self, param_index: usize, sink: &mut Stamps) {
        if param_index == 0 {
            let d = (self.vd / THERMAL_VOLTAGE).exp() - 1.0;
            sink.push(Stamp::Rhs(self.p.clone(), d));
            sink.push(Stamp::Rhs(self.n.clone(), -d));
        }
    }
}

impl DigitalDevice for D {}

impl Introspect for D {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![real_param("is_sat", 1.0e-14, "A", Invalidation::Restamp)]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "is_sat").then_some(Value::Real(self.is_sat))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "is_sat" => write_real(name, value, &mut self.is_sat, Invalidation::Restamp),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for D {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_SENSITIVITY
    }
}

// ---------------------------------------------------------------------------
// Voltage sources: Vsrc, Vsine, Vramp, ClockSrc
// ---------------------------------------------------------------------------

/// The branch-current unknown every forced source owns.
struct Branch {
    reference: Option<AnalogReference>,
}

impl Branch {
    fn new() -> Self {
        Self { reference: None }
    }

    fn allocate(&mut self, label: &str, alloc: &mut UnknownAllocator<'_>) {
        self.reference = Some(alloc.branch(label, "i"));
    }

    fn get(&self) -> &AnalogReference {
        self.reference.as_ref().expect("branch allocated by CircuitBuilder::build")
    }
}

fn source_caps() -> ElementCapabilities {
    ElementCapabilities::ANALOG
        | ElementCapabilities::LOADS_DC
        | ElementCapabilities::LOADS_TRAN
        | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
}

/// ```verilog
/// module Vsrc (inout electrical p, inout electrical n);
///     parameter real dc = 10.0;
///     analog V(p, n) <+ dc;
/// endmodule
/// ```
pub struct Vsrc {
    label: String,
    dc: f64,
    p: AnalogReference,
    n: AnalogReference,
    branch: Branch,
}

impl Vsrc {
    pub fn new(label: &str, dc: f64, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), dc, p, n, branch: Branch::new() }
    }
}

impl AnalogDevice for Vsrc {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch.allocate(&self.label, alloc);
    }

    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), self.dc * s.src_scale)
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), self.dc)
    }

    /// The branch row is `V_p - V_n - dc`, so `dF/d dc = -1`.
    fn param_stamps(&mut self, param_index: usize, sink: &mut Stamps) {
        if param_index == 0 {
            sink.push(Stamp::Rhs(self.branch.get().clone(), -1.0));
        }
    }
}

impl DigitalDevice for Vsrc {}

impl Introspect for Vsrc {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![real_param("dc", 10.0, "V", Invalidation::OperatingPoint)]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "dc").then_some(Value::Real(self.dc))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "dc" => write_real(name, value, &mut self.dc, Invalidation::OperatingPoint),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for Vsrc {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        source_caps() | ElementCapabilities::HAS_SENSITIVITY
    }
}

/// ```verilog
/// module Vsine (inout electrical p, inout electrical n);
///     parameter real amp = 5.0;
///     parameter real freq = 1.0e3;
///     analog V(p, n) <+ amp * sin(2.0 * 3.14159265358979 * freq * $abstime);
/// endmodule
/// ```
pub struct Vsine {
    label: String,
    amp: f64,
    freq: f64,
    p: AnalogReference,
    n: AnalogReference,
    branch: Branch,
}

impl Vsine {
    pub fn new(label: &str, amp: f64, freq: f64, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), amp, freq, p, n, branch: Branch::new() }
    }

    fn at(&self, t: f64) -> f64 {
        self.amp * (2.0 * PI_LITERAL * self.freq * t).sin()
    }
}

impl AnalogDevice for Vsine {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch.allocate(&self.label, alloc);
    }

    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), self.at(0.0))
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), self.at(t.time))
    }
}

impl DigitalDevice for Vsine {}

impl Introspect for Vsine {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![
            real_param("amp", 5.0, "V", Invalidation::OperatingPoint),
            real_param("freq", 1.0e3, "Hz", Invalidation::OperatingPoint),
        ]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        match name {
            "amp" => Some(Value::Real(self.amp)),
            "freq" => Some(Value::Real(self.freq)),
            _ => None,
        }
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "amp" => write_real(name, value, &mut self.amp, Invalidation::OperatingPoint),
            "freq" => write_real(name, value, &mut self.freq, Invalidation::OperatingPoint),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for Vsine {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        source_caps()
    }
}

/// ```verilog
/// module Vramp (inout electrical p, inout electrical n);
///     analog V(p, n) <+ 1.0e3 * $abstime;
/// endmodule
/// ```
pub struct Vramp {
    label: String,
    p: AnalogReference,
    n: AnalogReference,
    branch: Branch,
}

impl Vramp {
    pub fn new(label: &str, p: AnalogReference, n: AnalogReference) -> Self {
        Self { label: label.into(), p, n, branch: Branch::new() }
    }
}

impl AnalogDevice for Vramp {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch.allocate(&self.label, alloc);
    }

    /// `1.0e3 * $abstime` at `t = 0`.
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), 0.0)
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        branch_force(&self.p, &self.n, self.branch.get(), 1.0e3 * t.time)
    }
}

impl DigitalDevice for Vramp {}

impl Introspect for Vramp {}

impl Element for Vramp {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        source_caps()
    }
}

/// ```verilog
/// module ClockSrc (inout electrical clk_a, inout electrical gnd);
///     parameter real period = 1.0e-3;
///     real ph;
///     analog begin
///         ph = $abstime - period * floor($abstime / period);
///         V(clk_a, gnd) <+ (ph > period * 0.25 && ph < period * 0.75) ? 1.0 : 0.0;
///     end
/// endmodule
/// ```
pub struct ClockSrc {
    label: String,
    period: f64,
    clk_a: AnalogReference,
    gnd: AnalogReference,
    branch: Branch,
}

impl ClockSrc {
    pub fn new(label: &str, period: f64, clk_a: AnalogReference, gnd: AnalogReference) -> Self {
        Self { label: label.into(), period, clk_a, gnd, branch: Branch::new() }
    }

    fn at(&self, t: f64) -> f64 {
        let ph = t - self.period * (t / self.period).floor();
        if ph > self.period * 0.25 && ph < self.period * 0.75 { 1.0 } else { 0.0 }
    }
}

impl AnalogDevice for ClockSrc {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch.allocate(&self.label, alloc);
    }

    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Stamps {
        branch_force(&self.clk_a, &self.gnd, self.branch.get(), self.at(0.0))
    }

    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Stamps {
        branch_force(&self.clk_a, &self.gnd, self.branch.get(), self.at(t.time))
    }
}

impl DigitalDevice for ClockSrc {}

impl Introspect for ClockSrc {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![real_param("period", 1.0e-3, "s", Invalidation::OperatingPoint)]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "period").then_some(Value::Real(self.period))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        match name {
            "period" => write_real(name, value, &mut self.period, Invalidation::OperatingPoint),
            _ => Err(ParamError::Unknown(name.into())),
        }
    }
}

impl Element for ClockSrc {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        source_caps()
    }
}

// ---------------------------------------------------------------------------
// Comparator
// ---------------------------------------------------------------------------

/// ```verilog
/// module Comparator (input electrical a, input electrical n, output logic y);
///     always @(*) begin
///         y <= V(a, n) > 0.5;
///     end
/// endmodule
/// ```
///
/// Stateless: every sample of the analog solution emits the level, and the
/// scheduler discards an emission that equals the net it drives.
pub struct Comparator {
    label: String,
    a: AnalogReference,
    n: AnalogReference,
    y: DigitalNet,
}

impl Comparator {
    pub fn new(label: &str, a: AnalogReference, n: AnalogReference, y: DigitalNet) -> Self {
        Self { label: label.into(), a, n, y }
    }

    fn level(&self, analog: &[f64]) -> LogicValue {
        let at = |r: &AnalogReference| r.idx().and_then(|i| analog.get(i).copied()).unwrap_or(0.0);
        if at(&self.a) - at(&self.n) > 0.5 { LogicValue::One } else { LogicValue::Zero }
    }
}

impl AnalogDevice for Comparator {}

impl DigitalDevice for Comparator {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.y) }
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        sink.emit(self.y, self.level(ctx.analog), 0.0);
    }
}

impl Introspect for Comparator {}

impl Element for Comparator {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL | ElementCapabilities::SAMPLES_ANALOG
    }

    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _t: f64,
        _nets: &[LogicValue],
        sink: &mut dyn EventSink,
    ) {
        if let Some(latest) = state.latest() {
            let analog: Vec<f64> = latest.iter().copied().collect();
            sink.emit(self.y, self.level(&analog), 0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// ToggleDff
// ---------------------------------------------------------------------------

/// ```verilog
/// module ToggleDff (input logic clk, output logic q);
///     integer st;
///     always begin
///         q <= (st % 2) > 0;
///         @(posedge clk) st = st + 1;
///     end
/// endmodule
/// ```
///
/// The hidden state is the register `st` and the edge memory of `clk`, which
/// round-trip through `digital_hidden_snapshot` / `digital_hidden_restore`.
pub struct ToggleDff {
    label: String,
    clk: DigitalNet,
    q: DigitalNet,
    st: i64,
    /// The last sampled `clk` level: 0, 1, or 2 for anything else.
    prev_clk: i64,
}

impl ToggleDff {
    pub fn new(label: &str, clk: DigitalNet, q: DigitalNet) -> Self {
        Self { label: label.into(), clk, q, st: 0, prev_clk: 2 }
    }

    fn level(&self) -> LogicValue {
        if self.st % 2 > 0 { LogicValue::One } else { LogicValue::Zero }
    }
}

fn clk_code(value: LogicValue) -> i64 {
    match value {
        LogicValue::Zero => 0,
        LogicValue::One => 1,
        _ => 2,
    }
}

impl AnalogDevice for ToggleDff {}

impl DigitalDevice for ToggleDff {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts {
            inputs: std::slice::from_ref(&self.clk),
            outputs: std::slice::from_ref(&self.q),
        }
    }

    fn init(&mut self, sink: &mut dyn EventSink) {
        sink.emit(self.q, self.level(), 0.0);
    }

    fn seq_phase(&mut self, ctx: &EvalCtx<'_>) -> bool {
        let now = clk_code(ctx.nets[self.clk.0]);
        let posedge = now == 1 && self.prev_clk != 1;
        self.prev_clk = now;
        if posedge {
            self.st += 1;
        }
        posedge
    }

    fn comb_phase(&mut self, _ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        sink.emit(self.q, self.level(), 0.0);
    }

    fn digital_hidden_snapshot(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        Some((vec![self.st, self.prev_clk], Vec::new()))
    }

    fn digital_hidden_restore(&mut self, state: &(Vec<i64>, Vec<f64>)) {
        if let [st, prev_clk] = state.0[..] {
            self.st = st;
            self.prev_clk = prev_clk;
        }
    }
}

impl Introspect for ToggleDff {}

impl Element for ToggleDff {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

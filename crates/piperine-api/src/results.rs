//! Result objects a host reads measurements through: the [`NetRef`] handle
//! produced by name resolution, and [`OpResult`] — the immutable snapshot a
//! DC operating-point analysis returns.

use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_solver::prelude::{
    BranchIdentifier, DcAnalysisResult, ModelDescriptor, NodeIdentifier, ObservableDescriptor,
    ParamDescriptor, TerminalDescriptor,
};

use crate::error::Error;

/// A resolved top-level net — the argument type `.v`/`.i` expect.
#[derive(Debug, Clone)]
pub struct NetRef {
    pub name: String,
}

impl From<&str> for NetRef {
    fn from(name: &str) -> Self {
        NetRef { name: name.to_string() }
    }
}

impl From<String> for NetRef {
    fn from(name: String) -> Self {
        NetRef { name }
    }
}

impl From<&String> for NetRef {
    fn from(name: &String) -> Self {
        NetRef { name: name.clone() }
    }
}

impl From<&NetRef> for NetRef {
    fn from(r: &NetRef) -> Self {
        r.clone()
    }
}

/// Anything `.v`/`.i` (HOST-23) can resolve into one net or a differential
/// pair — a bare name (`&str`/`String`/`NetRef`), or a `(a, b)` tuple for a
/// differential read: `op.v("out")` / `op.v(("out", "in"))`. No bare
/// `NetRef { name }` construction is needed at a call site anymore (though
/// it still works — `NetRef` itself implements `NetSelector`).
///
/// Deliberately *not* a blanket `impl<T: Into<NetRef>> NetSelector for T`
/// plus a generic `(A, Option<B>)` tuple impl: the two would structurally
/// overlap under Rust's coherence rules (a `(A, Option<B>)` value also
/// matches the blanket tuple pattern with `B' = Option<B>`), so each single-
/// value and tuple shape is implemented directly instead.
pub trait NetSelector {
    fn resolve(self) -> (NetRef, Option<NetRef>);
}

impl NetSelector for &str {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self.into(), None)
    }
}

impl NetSelector for String {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self.into(), None)
    }
}

impl NetSelector for &String {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self.into(), None)
    }
}

impl NetSelector for NetRef {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self, None)
    }
}

impl NetSelector for &NetRef {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self.clone(), None)
    }
}

impl<A: Into<NetRef>, B: Into<NetRef>> NetSelector for (A, B) {
    fn resolve(self) -> (NetRef, Option<NetRef>) {
        (self.0.into(), Some(self.1.into()))
    }
}

/// `.tf` result (HOST-03): DC small-signal transfer characteristics from
/// unit excitations on the system linearized at the operating point — a
/// typed api wrapper over the solver's existing `.tf` driver (no new solver
/// math, MD-14: voltage-source input only).
#[derive(Debug, Clone, Copy)]
pub struct TfResult {
    /// `d(output)/d(input)` — dimensionless (voltage/current gain) or an
    /// impedance/admittance, depending on the input/output kind.
    pub gain: f64,
    /// Resistance seen by the input source.
    pub z_in: f64,
    /// Thévenin/Norton equivalent resistance at the output.
    pub z_out: f64,
}

impl TfResult {
    pub fn from_solver(r: piperine_solver::prelude::TransferFunctionAnalysisResult) -> Self {
        Self { gain: r.gain, z_in: r.input_resistance, z_out: r.output_resistance }
    }
}

/// `.pz` result (HOST-04): poles (and, when an input source is given,
/// transmission zeros) of the linearized input→output transfer function, in
/// rad/s — the uniform host shape (MD-22), same field names on both hosts.
#[derive(Debug, Clone, Default)]
pub struct PzResult {
    pub poles: Vec<num_complex::Complex64>,
    pub zeros: Vec<num_complex::Complex64>,
}

impl From<piperine_solver::prelude::PoleZeroResult> for PzResult {
    fn from(r: piperine_solver::prelude::PoleZeroResult) -> Self {
        Self { poles: r.poles, zeros: r.zeros }
    }
}

/// `.disto` result (HOST-04): small-signal Volterra distortion at the DC
/// operating point. Single-tone runs report `hd2`/`hd3`; two-tone runs
/// `im2`/`im3` — the uniform host shape (MD-22).
#[derive(Debug, Clone, Default)]
pub struct DistoResult {
    pub hd2: Option<f64>,
    pub hd3: Option<f64>,
    pub im2: Option<f64>,
    pub im3: Option<f64>,
    /// Named capability diagnostics from the `.disto` pre-scan (ABI-24) —
    /// see the solver's `DistoResult::warnings` for the full contract.
    pub warnings: Vec<String>,
}

impl From<piperine_solver::prelude::DistoResult> for DistoResult {
    fn from(r: piperine_solver::prelude::DistoResult) -> Self {
        Self { hd2: r.hd2, hd3: r.hd3, im2: r.im2, im3: r.im3, warnings: r.warnings }
    }
}

/// `.sp` result (HOST-04): the N-port scattering matrix over a frequency
/// sweep — the uniform host shape (MD-22), same field names as the solver's
/// `SpResult`. `s(k, i, j)` reads `S_ij` at swept point `k` without indexing
/// the raw `ndarray` matrix by hand.
#[derive(Debug, Clone, Default)]
pub struct SParamResult {
    pub frequencies: Vec<f64>,
    /// `s[k]` is the `n_ports × n_ports` matrix at `frequencies[k]`,
    /// `s[k][[i, j]] = S_ij` (port `i` response / port `j` excitation).
    pub s: Vec<ndarray::Array2<num_complex::Complex64>>,
    /// Reference impedance of each port, indexed by port position.
    pub z0: Vec<f64>,
    pub n_ports: usize,
}

impl SParamResult {
    /// `S_ij` at swept point `k` — a named accessor over the raw matrix.
    pub fn s(&self, k: usize, i: usize, j: usize) -> num_complex::Complex64 {
        self.s[k][[i, j]]
    }
}

impl From<piperine_solver::prelude::SpResult> for SParamResult {
    fn from(r: piperine_solver::prelude::SpResult) -> Self {
        Self { frequencies: r.frequencies, s: r.s, z0: r.z0, n_ports: r.n_ports }
    }
}

/// `.sens` result: `∂V(output)/∂(param)` keyed by
/// `(output net name, "label.param")` — the uniform host shape (MD-22; the
/// Python binding exposes the same map and the same `get`).
#[derive(Debug, Clone)]
pub struct SensResult {
    pub d: HashMap<(String, String), f64>,
}

impl SensResult {
    /// The sensitivity of `output` w.r.t. `label.param`, if computed.
    pub fn get(&self, output: &str, label: &str, param: &str) -> Option<f64> {
        self.d.get(&(output.to_string(), format!("{label}.{param}"))).copied()
    }
}

/// PSS result: one converged period as a [`Trace`](crate::waveform::Trace)
/// plus the shooting diagnostics — the uniform host shape (MD-22).
pub struct PssResult {
    pub trace: crate::waveform::Trace,
    pub stats: piperine_solver::prelude::PssStats,
}

impl std::fmt::Debug for PssResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PssResult").field("stats", &self.stats).finish_non_exhaustive()
    }
}

/// A per-instance introspection view (HOST-07/09): one device's computed
/// operating-point variables AND its static reflection catalogs (model
/// identity, terminal descriptors, observable catalog), addressed by
/// instance label — `op.instance("x1")?.opvar("gm")` / `.opvars()` /
/// `.model()` / `.terminals()` / `.observables()`. Borrows the opvar
/// snapshot from [`OpResult::instance`] (taken eagerly at solve time);
/// the model/terminals/observables catalogs are cloned (they are small
/// static descriptors and this is a reflection API, not a hot path).
#[derive(Debug)]
pub struct InstanceView<'a> {
    label: &'a str,
    opvars: &'a [(String, f64)],
    model: ModelDescriptor,
    terminals: Vec<TerminalDescriptor>,
    observables: Vec<ObservableDescriptor>,
    params: Vec<ParamDescriptor>,
}

impl InstanceView<'_> {
    /// The instance label this view projects.
    pub fn label(&self) -> &str {
        self.label
    }

    /// The device's model identity (ABI-46 / HOST-09): `type_id` and
    /// `version`. An author-declared `@model(type, version)` populates both;
    /// absent `@model` falls back to the module name as `type_id` with empty
    /// version — never panics.
    pub fn model(&self) -> &ModelDescriptor {
        &self.model
    }

    /// The device's declared terminals with their kind (ABI-27 / HOST-09):
    /// name, domain, direction, and `TerminalKind` (external/internal/
    /// auxiliary). Ports default to `External`; internal wires to `Internal`;
    /// an author-declared `@kind(value)` overrides either.
    pub fn terminals(&self) -> &[TerminalDescriptor] {
        &self.terminals
    }

    /// The device's observable catalog (ABI-32 / HOST-09): each entry is
    /// `(name, kind, cost)` — what CAN be probed via `probe=["inst.name"]`.
    /// The name matches the observable name a `ProbeSelection` request uses.
    pub fn observables(&self) -> &[ObservableDescriptor] {
        &self.observables
    }

    /// The device's parameter descriptor catalog (ABI / HOST-12):
    /// `bounds`/`unit`/`scope`/`invalidation` for each declared parameter.
    pub fn params(&self) -> &[ParamDescriptor] {
        &self.params
    }

    /// The parameter descriptor for `name` (HOST-12): `bounds`, `unit`,
    /// `scope`, `invalidation`. Fails loud when the device declares no such
    /// parameter, naming the instance and listing available params.
    pub fn param(&self, name: &str) -> Result<&ParamDescriptor, Error> {
        self.params.iter().find(|p| p.name == name).ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{}` has no param `{name}`; available: {}",
                self.label,
                self.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// The device's computed operating-point variable `name` (ABI-30). Fails
    /// loud — never `None`/`NaN` — when the device declares no such opvar,
    /// naming the instance and listing the opvars it does have.
    pub fn opvar(&self, name: &str) -> Result<f64, Error> {
        self.opvars.iter().find(|(n, _)| n == name).map(|(_, v)| *v).ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{}` has no opvar `{name}`; available: {}",
                self.label,
                self.opvars.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// Every opvar this device declared, as `(name, value)` pairs.
    pub fn opvars(&self) -> Vec<(String, f64)> {
        self.opvars.to_vec()
    }
}

/// The immutable snapshot returned by an operating-point analysis: DC node
/// potentials and branch currents, read by name through [`CircuitBuildInfo`].
pub struct OpResult {
    dc: DcAnalysisResult,
    /// Digital net values at the solved point (0/1, NaN for X/Z) — read by
    /// `.v(bit_net)` so pure-digital designs need no analog readback stage.
    digital: HashMap<String, f64>,
    /// Every device's `read_opvars()` snapshot, keyed by instance label
    /// (HOST-07), taken while the circuit was still live inside the
    /// analysis call (mirrors `digital`'s eager-snapshot shape — the
    /// `CircuitInstance` does not outlive the call that produced this
    /// result).
    opvars: HashMap<String, Vec<(String, f64)>>,
    /// Every device's `model_descriptor()` snapshot, keyed by instance label
    /// (HOST-09) — static catalog, snapshotted eagerly alongside `opvars`.
    models: HashMap<String, ModelDescriptor>,
    /// Every device's `list_terminals()` snapshot, keyed by instance label
    /// (HOST-09) — terminal descriptors carrying `TerminalKind`.
    terminals: HashMap<String, Vec<TerminalDescriptor>>,
    /// Every device's `list_observables()` snapshot, keyed by instance label
    /// (HOST-09) — observable catalog for `probe=` discovery.
    observables: HashMap<String, Vec<ObservableDescriptor>>,
    /// Every device's `list_params()` snapshot, keyed by instance label
    /// (HOST-12) — parameter descriptors carrying `bounds`/`unit`/`scope`/
    /// `invalidation`.
    params: HashMap<String, Vec<ParamDescriptor>>,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for OpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpResult").finish_non_exhaustive()
    }
}

impl OpResult {
    pub fn new(
        dc: DcAnalysisResult,
        digital: HashMap<String, f64>,
        opvars: HashMap<String, Vec<(String, f64)>>,
        models: HashMap<String, ModelDescriptor>,
        terminals: HashMap<String, Vec<TerminalDescriptor>>,
        observables: HashMap<String, Vec<ObservableDescriptor>>,
        params: HashMap<String, Vec<ParamDescriptor>>,
        info: Rc<CircuitBuildInfo>,
    ) -> Self {
        Self { dc, digital, opvars, models, terminals, observables, params, info }
    }

    /// Per-analysis convergence + performance statistics.
    pub fn stats(&self) -> &piperine_solver::abi::SolverStats {
        &self.dc.stats
    }

    /// The introspection view over instance `label` (HOST-07/09):
    /// `op.instance("x1")?.opvar("gm")` / `.model()` / `.terminals()` /
    /// `.observables()`. Fails loud when no device carries that label.
    pub fn instance(&self, label: &str) -> Result<InstanceView<'_>, Error> {
        let (name, vars) = self
            .opvars
            .get_key_value(label)
            .ok_or_else(|| Error::Measurement(format!("no element labeled `{label}`")))?;
        // All four snapshots are populated from the same `all_devices()`
        // iteration, so a label found in `opvars` is guaranteed present in
        // the others. The fallbacks (`unwrap_or_default`) are defensive —
        // they never trigger on a well-formed `OpResult` and do not mask a
        // user error (the fail-loud check above already rejected unknown
        // labels).
        let model = self.models.get(name).cloned().unwrap_or_default();
        let terminals = self.terminals.get(name).cloned().unwrap_or_default();
        let observables = self.observables.get(name).cloned().unwrap_or_default();
        let params = self.params.get(name).cloned().unwrap_or_default();
        Ok(InstanceView {
            label: name.as_str(),
            opvars: vars.as_slice(),
            model,
            terminals,
            observables,
            params,
        })
    }

    /// Resolve a host-visible net name to a solver node.
    fn node_or_err(&self, name: &str) -> Result<NodeIdentifier, Error> {
        self.info
            .net_node(name)
            .ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
    }

    /// Node voltage of net `a` minus net `b` (ground-referenced when `b` is
    /// `None`). A single-ended digital `Bit`/`Logic` net reads its logic
    /// value (0/1; NaN for X/Z).
    pub fn v(&self, sel: impl NetSelector) -> Result<f64, Error> {
        let (a, b) = sel.resolve();
        let (a, b) = (&a, b.as_ref());
        if b.is_none()
            && let Some(v) = self.digital.get(&a.name)
        {
            return Ok(*v);
        }
        let node_a = self.node_or_err(&a.name)?;
        let va = if node_a == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(&node_a).unwrap_or(0.0) };
        let vb = match b {
            Some(nb) => {
                let node_b = self.node_or_err(&nb.name)?;
                if node_b == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(&node_b).unwrap_or(0.0) }
            }
            None => 0.0,
        };
        Ok(va - vb)
    }

    /// Branch current from terminal `a` to `b` (ground-referenced when `b` is
    /// `None`). Ideal sources read the exact MNA branch unknown; other
    /// two-terminal devices are recomputed from kernel + solved terminal
    /// voltages. The two-net form names the unique two-terminal instance
    /// whose ports connect exactly to `(a, b)` and errors on any ambiguity
    /// (use the instance-port form instead).
    pub fn i(&self, sel: impl NetSelector) -> Result<f64, Error> {
        let (a, b) = sel.resolve();
        let (a, b) = (&a, b.as_ref());
        let node_a = self.node_or_err(&a.name)?;
        let node_b = match b {
            Some(nb) => self.node_or_err(&nb.name)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = find_two_terminal_instance(&self.info, node_a.clone(), node_b)?;
        if instance.num_forces > 0 {
            let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
            return Ok(self.dc.get_branch(branch).unwrap_or(0.0));
        }
        let volts: Vec<f64> = instance
            .terminals
            .iter()
            .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(t).unwrap_or(0.0) })
            .collect();
        let mut residual = vec![0.0; instance.terminals.len()];
        let sim = piperine_codegen::SimCtx::default();
        instance.kernel.eval_residual(&volts, &instance.params, &[], &[], &sim, &mut residual);
        // Sign convention: positive current flows from terminal `a` into
        // the device; `residual[0]` is the current out of terminal 0.
        let current = if instance.terminals[0] == node_a { residual[0] } else { -residual[0] };
        Ok(current)
    }
}

/// Split a dotted probe/opvar path (`"x1.p_out"`) into its instance label and
/// observable/opvar name (HOST-08). Shared by `Session::tran`'s `probe`
/// wiring (label the `ProbeSelection` request needs) and `Trace::opvar`
/// (label to look up in the build info's instance catalog). Splits on the
/// *first* `.` — instance labels are flat identifiers with no `.` in them.
pub(crate) fn split_probe_path(path: &str) -> Result<(&str, &str), Error> {
    path.split_once('.').ok_or_else(|| {
        Error::Measurement(format!("probe path `{path}` must be `instance.name` (got no `.`)"))
    })
}

/// Net resolution over the built circuit — the one place host-visible net
/// names map to solver nodes. Ground-family names (`gnd`/`GND`/`vss`/`VSS`)
/// resolve to the reference node; everything else through the net map.
/// Shared by every result object and the session's noise setup.
pub(crate) trait NetLookup {
    /// Resolve a net *name*; `None` when the net is not addressable.
    fn net_node(&self, name: &str) -> Option<NodeIdentifier>;
}

impl NetLookup for CircuitBuildInfo {
    fn net_node(&self, name: &str) -> Option<NodeIdentifier> {
        if piperine_lang::pom::is_ground(name) {
            return Some(NodeIdentifier::Gnd);
        }
        self.nets.get(name).cloned()
    }
}

/// The unique two-terminal instance whose ports connect exactly to `(a, b)`
/// — the branch a two-net `.i(a, b)` names. Shared by [`OpResult::i`] (DC)
/// and `Trace::i` (over time).
pub(crate) fn find_two_terminal_instance(
    info: &CircuitBuildInfo,
    a: NodeIdentifier,
    b: NodeIdentifier,
) -> Result<&piperine_codegen::device::BuiltInstanceInfo, Error> {
    let matches: Vec<_> = info
        .instances
        .iter()
        .filter(|inst| {
            inst.terminals.len() == 2
                && ((inst.terminals[0] == a && inst.terminals[1] == b)
                    || (inst.terminals[0] == b && inst.terminals[1] == a))
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(Error::Measurement("no two-terminal instance connects those nets".into())),
        _ => Err(Error::Measurement(
            "more than one instance connects those nets — use the instance-port form".into(),
        )),
    }
}

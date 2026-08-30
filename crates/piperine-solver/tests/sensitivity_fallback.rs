//! Stamp-perturbation fallback for `∂F/∂p` (dv-gradients T6, DVG-06/07):
//! elements without `HAS_SENSITIVITY` get their parameter derivative by
//! differencing two `load_dc` calls at the **same** operating point (no
//! re-solve), coexisting with analytic elements in one circuit, and the
//! collector names every element that needed the fallback.

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, Bounds, DcAnalysisState, DigitalDevice,
    Element, ElementCapabilities, Introspect, Invalidation, Netlist, NodeIdentifier,
    ParamDescriptor, ParamError, ParamScope, Stamp, Value, ValueKind,
};
use piperine_solver::prelude::{ParamRequest, ParamStampCollector, SensitivityEngine};
use piperine_solver::prelude::{CircuitInstance, Context};

/// Read a node voltage from a DC analysis state (None → ground/0).
fn volt(state: &DcAnalysisState<'_>, ref_: &AnalogReference) -> f64 {
    ref_
        .idx()
        .and_then(|i| state.history().latest().and_then(|s| s.get(i).copied()))
        .unwrap_or(0.0)
}

/// A parametric conductor `I = g·V(n1,n2)`. With `HAS_SENSITIVITY` + an
/// analytic `param_stamps` override it takes the analytic path; without the
/// bit it takes the stamp-perturbation fallback. `analytic` selects which.
struct Conductor {
    name: &'static str,
    g: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    analytic: bool,
    /// V(n1,n2) cached from the last `load_dc` — the analytic `param_stamps`
    /// reads it (a JIT device would read its `last_volts` the same way).
    cached_v: f64,
}

impl Conductor {
    fn matrix_stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        let g = self.g;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl AnalogDevice for Conductor {
    fn load_dc(&mut self, state: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.cached_v = volt(state, &self.n1) - volt(state, &self.n2);
        self.matrix_stamps()
    }
    fn param_stamps(&mut self, _idx: usize, sink: &mut Vec<Stamp<AnalogReference, f64>>) {
        if !self.analytic {
            return;
        }
        // d(g·V)/dg = V(n1,n2) — the analytic derivative.
        sink.push(Stamp::Rhs(self.n1.clone(), self.cached_v));
        sink.push(Stamp::Rhs(self.n2.clone(), -self.cached_v));
    }
}

impl DigitalDevice for Conductor {}

impl Introspect for Conductor {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "g".into(),
            kind: ValueKind::Real,
            default: Value::Real(self.g),
            unit: None,
            bounds: Bounds::UNBOUNDED,
            scope: ParamScope::Instance,
            invalidation: Invalidation::Restamp,
            desc: None,
        }]
    }
    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "g").then_some(Value::Real(self.g))
    }
    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        if name != "g" {
            return Err(ParamError::Unknown(name.into()));
        }
        self.g = value.as_real().ok_or_else(|| ParamError::TypeMismatch {
            name: name.into(),
            expected: ValueKind::Real,
        })?;
        Ok(Invalidation::Restamp)
    }
}

impl Element for Conductor {
    fn name(&self) -> &str { self.name }
    fn capabilities(&self) -> ElementCapabilities {
        let mut c = ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC;
        if self.analytic {
            c |= ElementCapabilities::HAS_SENSITIVITY;
        }
        c
    }
}

/// Ideal 1 V source across `n1`–`n2` (a branch-current unknown, pre-allocated
/// via `connect_branch` — `from_devices_and_netlist` does not run the
/// unknown-allocation seam).
struct Vdc { v: f64, n1: AnalogReference, n2: AnalogReference, branch: AnalogReference }
impl AnalogDevice for Vdc {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let b = self.branch.clone();
        vec![
            Stamp::Matrix(self.n1.clone(), b.clone(), 1.0),
            Stamp::Matrix(b.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), b.clone(), -1.0),
            Stamp::Matrix(b.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(b, self.v),
        ]
    }
}
impl DigitalDevice for Vdc {}
impl Introspect for Vdc {}
impl Element for Vdc {
    fn name(&self) -> &str { "v" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// 1 V across two parallel conductors: an analytic one and a fallback one.
/// Each sees V(n1,n2) = 1.0 → ∂(g·V)/∂g = 1.0.
fn circuit() -> (CircuitInstance, AnalogReference, AnalogReference) {
    let mut netlist = Netlist::new();
    let n1 = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let n2 = netlist.connect_node(NodeIdentifier::Gnd);
    let vbranch = netlist.connect_branch(BranchIdentifier::from_component("v"));
    let analytic = Conductor { name: "gan", g: 1e-3, n1: n1.clone(), n2: n2.clone(), analytic: true, cached_v: 0.0 };
    let fallback = Conductor { name: "gfb", g: 1e-3, n1: n1.clone(), n2: n2.clone(), analytic: false, cached_v: 0.0 };
    let vdc = Vdc { v: 1.0, n1: n1.clone(), n2: n2.clone(), branch: vbranch };
    let circuit = CircuitInstance::from_devices_and_netlist(
        "t6",
        vec![Box::new(vdc), Box::new(analytic), Box::new(fallback)],
        netlist,
    );
    (circuit, n1, n2)
}

fn op_vector(circuit: &CircuitInstance, dc: &piperine_solver::prelude::DcAnalysisResult) -> Vec<f64> {
    let pairs: Vec<(usize, f64)> = circuit
        .netlist()
        .all_references()
        .iter()
        .filter_map(|r| Some((r.idx()?, dc.get(r.variable().clone())?)))
        .collect();
    let n = pairs.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
    let mut op = vec![0.0; n];
    for (i, v) in pairs {
        op[i] = v;
    }
    op
}

/// Sum the `Stamp::Rhs` values whose reference matches `node` (the ∂F_node/∂g).
fn rhs_at(stamps: &[Stamp<AnalogReference, f64>], node: &AnalogReference) -> f64 {
    stamps
        .iter()
        .filter_map(|s| match s { Stamp::Rhs(r, v) if r.idx() == node.idx() => Some(*v), _ => None })
        .sum()
}

// ── analytic and fallback coexist; both contribute; fallback named ───────────

#[test]
fn analytic_and_fallback_paths_coexist_and_the_fallback_is_named() {
    let (mut circuit, n1, _n2) = circuit();
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let op = op_vector(&circuit, &dc);

    let mut collector = ParamStampCollector::new(&mut circuit, Context::default());

    let an = collector.collect(&ParamRequest { label: "gan".into(), param: "g".into() }, &op).unwrap();
    let fb = collector.collect(&ParamRequest { label: "gfb".into(), param: "g".into() }, &op).unwrap();

    // Engine attribution (DVG-07).
    assert_eq!(an.engine, SensitivityEngine::Analytic, "gan declares HAS_SENSITIVITY");
    assert_eq!(fb.engine, SensitivityEngine::StampPerturbation, "gfb lacks the bit → fallback");

    // Both paths deliver ∂F/∂g = V(n1,n2) = 1.0 at node n1 (the gnd terminal is
    // ground for the fallback, so it lands only on n1 — the value is the check).
    let an_v = rhs_at(&an.stamps, &n1);
    let fb_v = rhs_at(&fb.stamps, &n1);
    assert!((an_v - 1.0).abs() < 1e-6, "analytic ∂F(n1)/∂g = V = 1.0, got {an_v}");
    assert!((fb_v - 1.0).abs() < 1e-3, "fallback ∂F(n1)/∂g = V = 1.0 (FD), got {fb_v}");

    // The collector names the fallback element (DVG-07) — never a silent swap.
    assert!(collector.fallback_users.contains("gfb"), "fallback user reported by label");
    assert!(!collector.fallback_users.contains("gan"), "the analytic element is not a fallback user");
}

// ── no re-solve: the operating point is untouched after collecting ───────────

#[test]
fn the_fallback_does_not_re_solve_the_operating_point() {
    let (mut circuit, _n1, _n2) = circuit();
    let dc_before = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let op = op_vector(&circuit, &dc_before);
    let v_before = dc_before.get_node(&NodeIdentifier::Anonymous(1)).unwrap();

    let mut collector = ParamStampCollector::new(&mut circuit, Context::default());
    let _ = collector.collect(&ParamRequest { label: "gfb".into(), param: "g".into() }, &op).unwrap();

    // The parameter was restored (no residue) — the adjoint economy survives
    // because the fallback re-stamps only, never re-solves.
    let g_after = circuit.devices.iter().find(|d| d.name() == "gfb").unwrap().get_param("g");
    assert!(matches!(g_after, Some(Value::Real(v)) if (v - 1e-3).abs() < 1e-15), "param restored after perturbation");

    // Re-solving yields the same operating point: nothing the collector did
    // perturbed the converged solution (a re-solve would have been visible as
    // a changed voltage if state had been corrupted).
    let dc_after = circuit.dc(Context::default()).unwrap().solve().expect("dc re-solve");
    let v_after = dc_after.get_node(&NodeIdentifier::Anonymous(1)).unwrap();
    assert!((v_after - v_before).abs() < 1e-9, "operating point unchanged: {v_before} vs {v_after}");
}

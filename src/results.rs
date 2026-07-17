//! Result objects a host reads measurements through: the [`NetRef`] handle
//! produced by name resolution, and [`OpResult`] — the immutable snapshot a
//! DC operating-point analysis returns.

use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_solver::prelude::{BranchIdentifier, DcAnalysisResult, NodeIdentifier};

use crate::error::Error;

/// A resolved top-level net — the argument type `.v`/`.i` expect.
#[derive(Debug, Clone)]
pub struct NetRef {
    pub name: String,
}

/// The immutable snapshot returned by an operating-point analysis: DC node
/// potentials and branch currents, read by name through [`CircuitBuildInfo`].
pub struct OpResult {
    dc: DcAnalysisResult,
    /// Digital net values at the solved point (0/1, NaN for X/Z) — read by
    /// `.v(bit_net)` so pure-digital designs need no analog readback stage.
    digital: HashMap<String, f64>,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for OpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpResult").finish_non_exhaustive()
    }
}

impl OpResult {
    pub fn new(dc: DcAnalysisResult, digital: HashMap<String, f64>, info: Rc<CircuitBuildInfo>) -> Self {
        Self { dc, digital, info }
    }

    /// Per-analysis convergence + performance statistics.
    pub fn stats(&self) -> &piperine_solver::abi::SolverStats {
        &self.dc.stats
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
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<f64, Error> {
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
    pub fn i(&self, a: &NetRef, b: Option<&NetRef>) -> Result<f64, Error> {
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

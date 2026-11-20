//! Host sources on a top's node ports (§8.1.14, §23.1 rule 1; PI-07).
//!
//! A node port of the top has no driver inside the design, so the host gives it one: a
//! [`NodeSource`], a potential source between the port and `gnd` with a DC value and an optional AC
//! stimulus. The elaborator placed the source as the instance `<top>.<port>.source` of the IR body
//! `HostSource` (`piperine_lang::elab::HostSourceParam`), so a source is the host setting that
//! instance's params before the run: no new mechanism, the `set` of §23.1 rule 4. A run that
//! starts with a node port the host never gave a source is refused (`E0810`).

use std::collections::BTreeSet;

use piperine_ir::{CarrierRole, Ir, NodeRole};
use piperine_lang::elab::HostSourceParam;

use crate::failure::Failure;
use crate::session::Session;

/// The potential the host holds on one node port of the top, against `gnd` (§23.1 rule 1).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSource {
    /// The value at the operating point and wherever no waveform says otherwise, in volts or the
    /// port's potential unit.
    pub dc: f64,
    /// The AC stimulus an `ac` analysis takes: magnitude and phase in radians (§14.3.1).
    pub ac: Option<(f64, f64)>,
}

impl NodeSource {
    /// A source of the DC value `dc` and no AC stimulus.
    pub fn dc(dc: f64) -> NodeSource {
        NodeSource { dc, ac: None }
    }

    /// The same source with the AC stimulus `magnitude` at `phase` radians.
    pub fn with_ac(self, magnitude: f64, phase: f64) -> NodeSource {
        NodeSource { ac: Some((magnitude, phase)), ..self }
    }

    /// The params of the source instance this source sets, each with its value.
    fn params(&self) -> [(HostSourceParam, f64); 4] {
        let (magnitude, phase) = self.ac.unwrap_or((0.0, 0.0));
        [(HostSourceParam::V0, self.dc), (HostSourceParam::V1, self.dc), (HostSourceParam::Mag, magnitude), (HostSourceParam::Phase, phase)]
    }

    /// `E0802` when a value of the source is not a finite number: nothing is set then.
    fn refuse_non_finite(&self, port: &str) -> Result<(), Failure> {
        match self.params().iter().find(|(_, value)| !value.is_finite()) {
            Some((param, value)) => Err(Failure::Host { code: "E0802", message: format!("the source of `{port}` gives `{}` the value {value}, which is not a finite number", param.name()) }),
            None => Ok(()),
        }
    }
}

/// The node ports of the top the host has given a source so far.
#[derive(Debug, Default)]
pub(crate) struct NodeSources {
    driven: BTreeSet<String>,
}

impl NodeSources {
    /// Records that the host gave the node port `port` (its name in the top) a source.
    fn mark(&mut self, port: &str) {
        self.driven.insert(port.to_string());
    }

    /// `E0810` naming the first node port of `ir`'s top without a source: a run needs every one
    /// (§23.1 rule 1).
    pub(crate) fn require_all(&self, ir: &Ir) -> Result<(), Failure> {
        let top = ir.top();
        let undriven = top.body().nodes().iter().filter(|node| node.role == NodeRole::Port).find(|node| !self.driven.contains(&node.name));
        match undriven {
            Some(node) => Err(Failure::Host { code: "E0810", message: format!("the node port `{}` of the top `{}` has no host source: give it one with `Session::source` before the run", node.name, top.path()) }),
            None => Ok(()),
        }
    }

    /// The name in the top of the node port `port` (as written or `<top>.<port>`): `E0804` for a
    /// discrete port, which is driven with `drive`, `E0801` for a path that names no port.
    pub(crate) fn node_port(ir: &Ir, port: &str) -> Result<String, Failure> {
        let top = ir.top();
        let short = port.strip_prefix(&format!("{}.", top.path())).unwrap_or(port);
        let body = top.body();
        if body.nodes().iter().any(|node| node.role == NodeRole::Port && node.name == short) {
            return Ok(short.to_string());
        }
        if body.carriers().iter().any(|carrier| carrier.name == short && carrier.role != CarrierRole::Reached) {
            return Err(Failure::Host { code: "E0804", message: format!("`{port}` is a discrete carrier of the top: a host source drives a node port, use `drive` for a discrete `in` port") });
        }
        Err(Failure::Host { code: "E0801", message: format!("`{port}` names no node port of the top `{}`", top.path()) })
    }
}

impl Session {
    /// Gives the node port `port` of the top (as written or relative to the top) the source
    /// `source` before the run: the DC value at the operating point and the AC stimulus an `ac`
    /// analysis uses (§23.1 rule 1). A later call replaces the earlier one. `E0801` for a path
    /// that names no node port, `E0804` for a discrete port, `E0802` for a value that is not
    /// finite; a session that has run is refused; each refusal leaves the session as it was.
    pub fn source(&mut self, port: &str, source: NodeSource) -> Result<(), Failure> {
        let name = NodeSources::node_port(self.ir(), port)?;
        source.refuse_non_finite(port)?;
        let instance = format!("{}.{name}.source", self.ir().top().path());
        for (param, value) in source.params() {
            self.set_at(&format!("{instance}.{}", param.name()), value, 0.0)?;
        }
        self.sources.mark(&name);
        Ok(())
    }
}

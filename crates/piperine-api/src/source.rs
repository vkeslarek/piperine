//! Host sources on a top's node ports (§8.1.14, §23.1 rule 1; PI-07).
//!
//! A node port of the top has no driver inside the design, so the host gives it one: a
//! [`NodeSource`], a potential source between the port and `gnd` with a DC value and an optional AC
//! stimulus. The elaborator placed the source as the instance `<top>.<port>.source` of the IR body
//! `HostSource` (`piperine_lang::elab::HostSourceParam`), so a source is the host setting that
//! instance's params before the run: no new mechanism, the `set` of §23.1 rule 4. A run that
//! starts with a node port the host never gave a source is refused (`E0810`).
//!
//! A piecewise-linear waveform is a schedule of timed sets of the segment params (`t0`, `v0`, `t1`,
//! `v1` of corner `k` and `k + 1`, design D9): each corner is a time point (§15.2) and the value
//! is exact on the segment, so a transient lands on every corner. A jump is a timed set with
//! `v0 = v1`.

use std::collections::BTreeSet;

use piperine_ir::{CarrierRole, Ir, NodeRole};
use piperine_lang::elab::HostSourceParam;

use crate::failure::Failure;
use crate::request::Analysis;
use crate::session::Session;

/// The potential the host holds on one node port of the top, against `gnd` (§23.1 rule 1).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSource {
    /// The value at the operating point and wherever no waveform says otherwise, in volts or the
    /// port's potential unit.
    pub dc: f64,
    /// The AC stimulus an `ac` analysis takes: magnitude and phase in radians (§14.3.1).
    pub ac: Option<(f64, f64)>,
    /// The corners `(time, value)` of a piecewise-linear waveform, strictly increasing in time;
    /// empty for a source that holds `dc`. Before the first corner the source holds `dc`, after
    /// the last it holds the last value.
    pub corners: Vec<(f64, f64)>,
}

/// One set of a param of a source instance, due at `at` (§23.1 rule 4).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SegmentSet {
    /// The param of the source instance that is set.
    param: HostSourceParam,
    /// The value it takes.
    value: f64,
    /// The time the set is due, in seconds.
    at: f64,
}

impl NodeSource {
    /// A source of the DC value `dc` and no AC stimulus.
    pub fn dc(dc: f64) -> NodeSource {
        NodeSource { dc, ac: None, corners: Vec::new() }
    }

    /// The same source with the AC stimulus `magnitude` at `phase` radians.
    /// Kept for a Rust host that gives a source an `ac` magnitude (`tests/host_sources.rs`).
    pub fn with_ac(self, magnitude: f64, phase: f64) -> NodeSource {
        NodeSource { ac: Some((magnitude, phase)), ..self }
    }

    /// The same source with the piecewise-linear waveform through `corners`.
    /// Kept for a Rust host that gives a source corners (`tests/host_sources.rs`).
    pub fn with_corners(self, corners: Vec<(f64, f64)>) -> NodeSource {
        NodeSource { corners, ..self }
    }

    /// The params of the source instance this source sets, each with its value.
    fn params(&self) -> [(HostSourceParam, f64); 4] {
        let (magnitude, phase) = self.ac.unwrap_or((0.0, 0.0));
        [(HostSourceParam::V0, self.dc), (HostSourceParam::V1, self.dc), (HostSourceParam::Magnitude, magnitude), (HostSourceParam::Phase, phase)]
    }

    /// `E0802` when a value of the source is not a finite number or the corners do not increase
    /// strictly in time; nothing is set then.
    fn refuse_malformed(&self, port: &str) -> Result<(), Failure> {
        let refused = |message: String| Failure::Host { code: "E0802", message };
        if let Some((param, value)) = self.params().iter().find(|(_, value)| !value.is_finite()) {
            return Err(refused(format!("the source of `{port}` gives `{}` the value {value}, which is not a finite number", param.name())));
        }
        if let Some((time, value)) = self.corners.iter().find(|(time, value)| !time.is_finite() || !value.is_finite()) {
            return Err(refused(format!("the corner ({time}, {value}) of the source of `{port}` is not a finite point")));
        }
        match self.corners.windows(2).find(|pair| pair[1].0 <= pair[0].0) {
            Some(pair) => Err(refused(format!("the corners of the source of `{port}` are not strictly increasing in time: {} s is followed by {} s", pair[0].0, pair[1].0))),
            None => Ok(()),
        }
    }

    /// The sets that turn the waveform into segments, in time order: at corner `k` the segment
    /// from corner `k` to `k + 1`, and at the last corner a hold of its value (`t1` one second
    /// on). A corner past `stop` is never reached and gives no set (§23.1 rule 4).
    fn segments(&self, stop: f64) -> Vec<SegmentSet> {
        let mut sets = Vec::new();
        for (k, (time, value)) in self.corners.iter().enumerate() {
            if *time > stop {
                break;
            }
            let (next_time, next_value) = self.corners.get(k + 1).copied().unwrap_or((time + 1.0, *value));
            for (param, value) in [(HostSourceParam::T0, *time), (HostSourceParam::V0, *value), (HostSourceParam::T1, next_time), (HostSourceParam::V1, next_value)] {
                sets.push(SegmentSet { param, value, at: *time });
            }
        }
        sets
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

    /// The sets of `source`'s corners on a run of `analysis`: `P0010` when the run has no time to
    /// land on, `E0806` for a corner before the start.
    fn schedule(source: &NodeSource, port: &str, analysis: &Analysis) -> Result<Vec<SegmentSet>, Failure> {
        if source.corners.is_empty() {
            return Ok(Vec::new());
        }
        let Analysis::Transient { stop, .. } = analysis else {
            return Err(Failure::Host { code: "P0010", message: format!("the source of `{port}` has corners: only a `transient` run has a time to land on, this run is `{}`", analysis.name()) });
        };
        if let Some((time, _)) = source.corners.iter().find(|(time, _)| *time < 0.0) {
            return Err(Failure::Host { code: "E0806", message: format!("the corner of the source of `{port}` at {time:e} s is a time already past") });
        }
        Ok(source.segments(*stop))
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
        if body.carriers().iter().any(|carrier| carrier.name == short && !matches!(carrier.role, CarrierRole::Reached | CarrierRole::Internal)) {
            return Err(Failure::Host { code: "E0804", message: format!("`{port}` is a discrete carrier of the top: a host source drives a node port, use `drive` for a discrete `in` port") });
        }
        Err(Failure::Host { code: "E0801", message: format!("`{port}` names no node port of the top `{}`", top.path()) })
    }
}

impl Session {
    /// Gives the node port `port` of the top (as written or relative to the top) the source
    /// `source` before the run: the DC value at the operating point and the AC stimulus an `ac`
    /// analysis uses (§23.1 rule 1); with corners, the piecewise-linear waveform a transient lands
    /// on at every corner. A later call replaces the earlier one. `E0801` for a path that names no
    /// node port, `E0804` for a discrete port, `E0802` for a value that is not finite or corners
    /// that do not increase strictly in time, `E0806` for a corner before the start, `P0010` for
    /// corners on a run that is not a transient; a session that has run is refused; each refusal
    /// leaves the session as it was.
    pub fn source(&mut self, port: &str, source: NodeSource) -> Result<(), Failure> {
        let name = NodeSources::node_port(self.ir(), port)?;
        source.refuse_malformed(port)?;
        let schedule = NodeSources::schedule(&source, port, &self.spec().analysis)?;
        let instance = self.source_instance(&name);
        for (param, value) in source.params() {
            self.set_at(&format!("{instance}.{}", param.name()), value, 0.0)?;
        }
        for set in schedule {
            self.set_at(&format!("{instance}.{}", set.param.name()), set.value, set.at)?;
        }
        self.sources.mark(&name);
        Ok(())
    }

    /// Changes the node port `port` to `value` at time `at`: a jump (§15.2, PI-07 AC 4), the
    /// source's segment becoming the constant `value` from `at` on. The port has a source already
    /// (`E0810` at the run otherwise); the errors are those of [`Session::set_at`] (`E0806` for
    /// a time already past or after the stop) and `Session::source`'s for the port.
    /// Kept for a Rust host that reads the source at a node (`tests/host_sources.rs`).
    pub fn source_at(&mut self, port: &str, value: f64, at: f64) -> Result<(), Failure> {
        let name = NodeSources::node_port(self.ir(), port)?;
        if !value.is_finite() {
            return Err(Failure::Host { code: "E0802", message: format!("the jump of `{port}` to {value} is not a finite number") });
        }
        let instance = self.source_instance(&name);
        for param in [HostSourceParam::V0, HostSourceParam::V1] {
            self.set_at(&format!("{instance}.{}", param.name()), value, at)?;
        }
        Ok(())
    }

    /// The path of the source instance on the node port `name` of the top.
    fn source_instance(&self, name: &str) -> String {
        format!("{}.{name}.source", self.ir().top().path())
    }
}

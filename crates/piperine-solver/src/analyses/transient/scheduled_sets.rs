//! The host's scheduled writes of a transient: live parameter sets and discrete-net writes at given times.

use piperine_analog_solver::error::{Error, SolverDomain};

use crate::core::bind::ParamValue;
use crate::core::introspect::Value;
use crate::core::result::Result;
use crate::protocol::SetOutcome;

use super::{TransientProtocol, TransientSolver};

/// One host-scheduled live parameter write: at simulation time `t`, set
/// `param` on the element labeled `label` to `value`.
#[derive(Debug, Clone)]
pub struct ScheduledSet {
    pub t: f64,
    pub label: String,
    pub param: String,
    pub value: crate::core::introspect::Value,
}

/// One host-scheduled write of a value to a discrete net at simulation time `t`.
#[derive(Debug, Clone)]
pub(super) struct ScheduledWrite {
    pub(super) t: f64,
    pub(super) net: String,
    pub(super) value: crate::core::bind::ParamValue,
}

impl TransientSolver<'_> {
    /// Schedule a live parameter write for simulation time `t` (LIVE-06):
    /// the integrator lands exactly on `t` (unified breakpoint table), the
    /// write applies there, and the new value takes effect from the next
    /// accepted step (the landing is solved on the value before the write).
    /// Several sets on the same param apply in scheduling order — last write
    /// wins.
    pub fn schedule_set(
        &mut self,
        t: f64,
        label: impl Into<String>,
        param: impl Into<String>,
        value: crate::core::introspect::Value,
    ) {
        self.sets.push(ScheduledSet { t, label: label.into(), param: param.into(), value });
    }

    /// Schedule a host write of `value` to the discrete net `net` (its name in the digital plan)
    /// for simulation time `t` (HII-12): a write at or before the start is one made before the
    /// run (`CircuitInstance::write_net`); a later one is a host commit the protocol enters at
    /// microstep 0 of `t`, so a reaction on the net's change fires at that instant.
    pub fn schedule_write(&mut self, t: f64, net: impl Into<String>, value: crate::core::bind::ParamValue) {
        self.net_writes.push(ScheduledWrite { t, net: net.into(), value });
    }

    /// The scheduled writes as the run commits them: a write at or before the start is made before
    /// the run (a param `set`, a net write); a later one is a host commit the protocol lands on, its
    /// bytes encoded in the net's layout. The schedules are emptied.
    pub(super) fn scheduled_commits(&mut self) -> Result<HostCommits> {
        let mut later = Vec::new();
        for set in std::mem::take(&mut self.sets) {
            let value = match set.value {
                Value::Real(v) => ParamValue::Real(v),
                Value::Integer(v) => ParamValue::Int(v),
                Value::Boolean(v) => ParamValue::Bool(v),
                Value::Bytes(bytes) => ParamValue::Bytes(bytes),
                Value::Text(_) => {
                    return Err(Error::simple(SolverDomain::Transient, format!("`{}.{}`: a text value has no byte layout", set.label, set.param)));
                }
            };
            if set.t <= self.options.start_time {
                self.system.circuit.set(&set.label, &set.param, value)?;
            } else {
                later.push((set.t, set.label, set.param, value));
            }
        }
        let mut later_nets = Vec::new();
        for write in std::mem::take(&mut self.net_writes) {
            if write.t <= self.options.start_time {
                self.system.circuit.write_net(&write.net, write.value)?;
            } else {
                later_nets.push(write);
            }
        }
        let bound = &mut self.system.circuit.abi;
        let mut commits = Vec::with_capacity(later.len() + later_nets.len());
        for (t, path, param, value) in &later {
            let (net, bytes) = bound.param_bytes(path, param, value)?;
            commits.push(HostCommit { time: *t, what: format!("{path}.{param}"), net: net.0, bytes });
        }
        for write in &later_nets {
            let (net, bytes) = bound.net_bytes(&write.net, &write.value)?;
            commits.push(HostCommit { time: write.t, what: write.net.clone(), net: net.0, bytes });
        }
        Ok(HostCommits(commits))
    }
}

/// One scheduled write the protocol commits during the run: at `time`, the bytes of the net `net`,
/// `what` naming the param or net the host wrote.
pub(super) struct HostCommit {
    time: f64,
    what: String,
    net: u32,
    bytes: Vec<u8>,
}

/// The scheduled writes of a run that come after its start, in scheduling order.
pub(super) struct HostCommits(Vec<HostCommit>);

impl HostCommits {
    /// Commits each write to `protocol` at its time; a refused write (or one that would need another
    /// shape) fails the run, naming it.
    pub(super) fn commit(&self, protocol: &mut TransientProtocol<'_, '_>) -> Result<()> {
        for HostCommit { time: t, what, net, bytes } in &self.0 {
            match protocol.set(*net, 0, bytes, Some(*t))? {
                SetOutcome::Committed => {}
                SetOutcome::Refused { reason } => {
                    return Err(Error::simple(SolverDomain::Transient, format!("the write of `{what}` at {t:e} s is refused: {reason}")));
                }
                SetOutcome::Reshape { instance } => {
                    return Err(Error::simple(
                        SolverDomain::Transient,
                        format!(
                            "the write of `{what}` at {t:e} s is refused: instance {instance} would need \
                             another shape inside a transient"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

//! Watches and the accept step (abi.md §4.3 watch table, A5; design
//! Watches, `accept`). At an accepted solution: a `VAR` watch's bytes become
//! readable by instance path and name; a `CROSS` watch whose value went from
//! `≤ 0` at the previous accepted solution to `> 0` at this one reports its
//! event pin as fired; the smallest `CAP` value bounds the next step. A
//! `TRACE` watch is recorded at every evaluation of its function that writes
//! it (anything but the all-zero canonical empty value of P3), stamped with
//! the Newton iteration, and is not part of a snapshot: a record survives a
//! restore.

use piperine_abi::raw::watch;

use crate::error::Error;

use super::assembly::{FnState, InstanceState};
use super::device_error;
use super::plan::{FnPlan, InstancePlan};

/// What an accepted solution reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    /// `(instance, event pin)` of each `CROSS` watch that fired here.
    pub fired: Vec<(usize, u32)>,
    /// The smallest `CAP` value evaluated here; infinite when there is none.
    pub cap: f64,
}

/// One `TRACE` record.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRecord {
    pub instance: String,
    pub watch: String,
    /// The Newton iteration of the evaluation that wrote it: the number of
    /// `end_iteration` calls since the last converged solve was recorded
    /// (`accept` or `push_stage`).
    pub iteration: u64,
    pub bytes: Vec<u8>,
}

/// What persists of one watch between accepted solutions.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Watched {
    /// A `VAR`'s bytes at the last accepted solution.
    pub var: Option<Vec<u8>>,
    /// A `CROSS`'s value at the last accepted solution.
    pub cross: Option<f64>,
}

/// Applies the accepted solution's watches of instance `index`.
pub(crate) fn accept_watches(
    index: usize,
    inst: &InstancePlan,
    st: &mut InstanceState,
    acc: &mut Accepted,
) -> Result<(), Error> {
    for (wi, w) in inst.watches.iter().enumerate() {
        let Some((fi, slot)) = w.value else { continue };
        let fs = &st.fns[fi];
        if w.kind == watch::VAR {
            st.watched[wi].var = Some(fs.out[slot].clone());
            continue;
        }
        if w.kind != watch::CROSS && w.kind != watch::CAP {
            continue;
        }
        let v = fs.f64(slot);
        if w.kind == watch::CAP {
            // docs/spec §17.2 rule 5 and its resolution: a cap that is not finite and positive.
            if !(v.is_finite() && v > 0.0) {
                return Err(device_error(format!(
                    "E0509: instance `{}`: step cap watch `{}` ({}) is {v}, not finite and positive (§17.2)",
                    inst.path, w.name, w.origin
                )));
            }
            acc.cap = acc.cap.min(v);
            continue;
        }
        if !v.is_finite() {
            return Err(device_error(format!(
                "instance `{}`: watch `{}` ({}) is not finite: {v} (abi.md A4)",
                inst.path, w.name, w.origin
            )));
        }
        let before = st.watched[wi].cross.replace(v);
        if before.is_some_and(|b| b <= 0.0) && v > 0.0 {
            acc.fired.push((index, w.event));
        }
    }
    Ok(())
}

/// Appends the `TRACE` records function `f` wrote in the call just made.
pub(crate) fn record_traces(inst: &InstancePlan, f: &FnPlan, fs: &FnState, iteration: u64, out: &mut Vec<TraceRecord>) {
    for (slot, wi) in &f.traces {
        let bytes = &fs.out[*slot];
        if bytes.iter().any(|b| *b != 0) {
            out.push(TraceRecord {
                instance: inst.path.clone(),
                watch: inst.watches[*wi].name.clone(),
                iteration,
                bytes: bytes.clone(),
            });
        }
    }
}

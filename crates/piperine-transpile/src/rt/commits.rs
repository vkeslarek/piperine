//! `Commits`: the commits and records of one reaction (MSL §13, §19).
//!
//! A reaction collects its commits and hands them to `React` in the order they were made, after
//! the body ran, so a fault makes none (design "Phase B: generated digital code"). Every
//! decision of the code is known, so the reaction has one path: the commits are those of the
//! statements it ran.

use piperine_abi::device::{Fault, Flow, React};

use super::pin::Carried;

/// One commit of a reaction.
#[derive(Clone, Debug)]
struct Entry {
    /// The statement it goes through.
    statement: u32,
    /// The bytes it commits: a pin, a record's layout, or none for an arming.
    bytes: Vec<u8>,
    /// The delay, in seconds.
    after: f64,
}

/// The commits and records of one reaction, in the order they were made.
#[derive(Debug, Default)]
pub struct Commits {
    entries: Vec<Entry>,
    /// Whether a `finish` was reached (§19 Resolution).
    finished: bool,
}

impl Commits {
    /// The commits of a reaction, none made yet.
    pub fn new() -> Commits {
        Commits::default()
    }

    /// `carrier <- value after delay` through statement `statement`.
    pub fn commit<T: Carried>(&mut self, statement: u32, value: T, after: f64) {
        self.entries.push(Entry { statement, bytes: value.pin(), after });
    }

    /// A record through statement `statement`: `bytes`, the whole layout of its stream's `RECORD`
    /// pin, due at the next microstep (abi.md C5: a record is a commit to that pin).
    pub fn record(&mut self, statement: u32, bytes: &[u8]) {
        self.entries.push(Entry { statement, bytes: bytes.to_vec(), after: 0.0 });
    }

    /// Arms a timer through statement `statement`, a `REPLACE` statement on an event pin (abi.md
    /// R1, C3), for the time `at` seen at the instant `now`: a later time is due `at - now`
    /// from now, the time of the instant itself at the next microstep (a delay of 0, C1), and a
    /// past time never (`+inf`, C1). Since the statement is `REPLACE`, each arming removes the
    /// firing an earlier one left pending, so a timer never fires for a time it no longer holds.
    pub fn arm(&mut self, statement: u32, at: f64, now: f64) {
        let after = match at {
            at if at > now => at - now,
            at if at == now => 0.0,
            _ => f64::INFINITY,
        };
        self.entries.push(Entry { statement, bytes: Vec::new(), after });
    }

    /// `finish(value)` (§19 Resolution): the analysis ends when `value` is true.
    pub fn finish(&mut self, value: bool) {
        if value {
            self.finished = true;
        }
    }

    /// What the reaction asks of the analysis.
    pub fn flow(&self) -> Flow {
        if self.finished { Flow::Finish } else { Flow::Continue }
    }

    /// Hands every commit to `cx`, in order; the first refusal is the fault.
    pub fn flush(self, cx: &mut React<'_>) -> Result<(), Fault> {
        for entry in &self.entries {
            cx.commit(entry.statement, &entry.bytes, entry.after)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use piperine_abi::device::{Flow, React};
    use piperine_abi::layout::LOW;
    use piperine_abi::raw::status;

    use super::Commits;
    use crate::rt::int::UInt;

    /// A reaction frame whose commits are recorded, as `(statement, bytes, after)`; a commit
    /// with flags is not one of them (see `flushed`).
    pub(crate) fn react_with(f: impl FnOnce(&mut React<'_>)) -> Vec<(u32, Vec<u8>, f64)> {
        let mut made = Vec::new();
        {
            let mut sink = |statement: u32, bytes: &[u8], after: f64| {
                made.push((statement, bytes.to_vec(), after));
                status::OK
            };
            let mut cx = React::new(vec![], &mut sink);
            f(&mut cx);
        }
        made
    }

    #[test]
    fn every_commit_reaches_the_host_in_the_order_made() {
        // nothing reaches `React` before `flush`; a record goes through its own statement as made
        let made = react_with(|cx| {
            let mut commits = Commits::new();
            commits.commit(2, UInt::<9>::new(300), 5e-9);
            commits.commit(0, true, 0.0);
            commits.record(4, &[7, 1, 1, 1]);
            commits.commit(0, false, 1e-9);
            commits.flush(cx).expect("every commit accepted");
        });
        assert_eq!(
            made,
            [(2, vec![44, 1], 5e-9), (0, vec![1], 0.0), (4, vec![7, 1, 1, 1], 0.0), (0, LOW.to_vec(), 1e-9)]
        );
        let made = react_with(|_| {
            let mut commits = Commits::new();
            commits.commit(0, true, 0.0);
            drop(commits);
        });
        assert_eq!(made, [], "a dropped `Commits`, as after a fault, commits nothing");
    }

    #[test]
    fn a_finish_ends_the_run_when_true() {
        // §19 Resolution: `finish(true)` ends the analysis, `finish(false)` does not
        let flow = |value: Option<bool>| {
            let mut commits = Commits::new();
            if let Some(value) = value {
                commits.finish(value);
            }
            commits.flow()
        };
        assert_eq!(flow(None), Flow::Continue);
        assert_eq!(flow(Some(true)), Flow::Finish);
        assert_eq!(flow(Some(false)), Flow::Continue);
    }

    #[test]
    fn a_timer_is_armed_for_a_later_time_only() {
        // msl-digital A3 on abi.md C1, C4: an arming fires its event pin (no bytes) `t - now`
        // later, at the next microstep for `t == now`, and never (`+inf`) for a past time
        let made = react_with(|cx| {
            let mut commits = Commits::new();
            commits.arm(0, 15e-9, 5e-9);
            commits.arm(1, 5e-9, 5e-9);
            commits.arm(2, 0.0, 5e-9);
            commits.flush(cx).expect("every arming accepted");
        });
        assert_eq!(made, [(0, vec![], 15e-9 - 5e-9), (1, vec![], 0.0), (2, vec![], f64::INFINITY)]);
    }
}

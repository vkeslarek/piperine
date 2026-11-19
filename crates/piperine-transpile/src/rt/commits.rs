//! `Commits`: the commits and records of one reaction, joined across the paths of unknown
//! control (MSL §3.3.3, §13 rule 1, §19).
//!
//! A reaction collects its commits and hands them to `React` in the order they were made, after
//! the body ran, so a fault makes none (design "Phase B: generated digital code"). A commit
//! whose delay is unknown commits `Unknown` at the next microstep (§3.3.3).
//!
//! Under unknown control the reaction does not know which path ran, so it makes the commits of
//! every path and `Commits` joins them as the merge of §3.3.3 says. A path's effects are its
//! value, its commits and its records; the join works on the commits, reduced to the last one per
//! **key** `(carrier, due instant)`:
//!
//! - a key in both paths commits `merge(a, b)` (through each statement either path used, so
//!   transport sees both), flagged `MAYBE`;
//! - a key in one path commits that value flagged `MAYBE | MERGE`: the solver merges it with the
//!   value the carrier holds when it comes due, which is "the value the carrier would hold then"
//!   of §3.3.3 and which only the solver knows;
//! - a path that panicked (§1 rule 13) makes every key of either path `Unknown`, and every record
//!   of either path unknown in every field but `maybe`.
//!
//! The flags are `piperine_abi::raw::commit_flag`; a commit with none goes through `React::commit`
//! as it always did, so a reaction without unknown control hands the host the same calls.

use piperine_abi::device::{Fault, Flow, React};
use piperine_abi::raw::commit_flag;

use super::control::{Attempt, Decision};
use super::flag::{Carried, Flagged, Merge};

/// The merge of two committed values, as bytes: `Flagged::<T>::merge_bytes` of the type the commit
/// carried.
type ByteMerge = fn(&[u8], &[u8]) -> Vec<u8>;

/// The instant a delay comes due, as abi.md C1 gives it, so that two delays the solver treats as
/// one instant are one key of a join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// `after <= 0`, or a delay that rounds to no tick: the next microstep of the same instant.
    NextMicrostep,
    /// The tick nearest `after`, counted from the instant that made the commit.
    Ticks(i64),
    /// `+inf`: never.
    Never,
    /// A delay the solver refuses (NaN, E0435; not a representable tick count, E0502). Such a
    /// commit is never joined with another, so the solver still names it.
    Refused,
}

impl Due {
    /// The instant a delay of `after` seconds comes due under a tick of `resolution` seconds
    /// (abi.md C1, `piperine_digital_solver::Resolution::due`): halves round away from zero.
    pub fn of(after: f64, resolution: f64) -> Due {
        if after.is_nan() {
            return Due::Refused;
        }
        if after == f64::INFINITY {
            return Due::Never;
        }
        if after <= 0.0 {
            return Due::NextMicrostep;
        }
        let count = (after / resolution).round();
        // `i64::MAX as f64` is 2^63, which is itself out of range
        if !count.is_finite() || count >= i64::MAX as f64 {
            return Due::Refused;
        }
        if count == 0.0 { Due::NextMicrostep } else { Due::Ticks(count as i64) }
    }
}

/// What one entry of a reaction asks of the host.
#[derive(Clone, Debug)]
enum What {
    /// A commit of a value of a carried type, merged by this function.
    Value(ByteMerge),
    /// A record: the whole layout of its stream's `RECORD` pin. `maybe_at` is the offset of the
    /// stream's `maybe` field (§19 rule 1).
    Record { maybe_at: usize },
    /// An arming of a timer through a `REPLACE` statement on an event pin (no bytes).
    Arm,
}

/// One commit of a reaction.
#[derive(Clone, Debug)]
struct Entry {
    /// The statement it goes through.
    statement: u32,
    /// The bytes it commits.
    bytes: Vec<u8>,
    /// The delay, in seconds.
    after: f64,
    /// `commit_flag` bits: 0 until a join of unknown paths gives it some.
    flags: u32,
    /// What it is.
    what: What,
}

/// The carrier each statement commits to, and the tick: what a join needs to tell two commits
/// apart.
#[derive(Clone, Copy, Debug, Default)]
struct Table {
    /// The carrier of statement `s` at index `s`; `None` for a record or an arming.
    carriers: &'static [Option<u32>],
    /// Seconds per tick.
    resolution: f64,
}

/// What two commits must share to be one effect of a merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    carrier: u32,
    due: Due,
}

impl Table {
    /// The key of a commit of a value, `None` when it cannot be joined: it has no carrier in the
    /// table, or the solver refuses its delay.
    fn key(&self, entry: &Entry) -> Option<Key> {
        if !matches!(entry.what, What::Value(_)) {
            return None;
        }
        let carrier = self.carriers.get(entry.statement as usize).copied().flatten()?;
        match Due::of(entry.after, self.resolution) {
            Due::Refused => None,
            due => Some(Key { carrier, due }),
        }
    }
}

/// The commits of one key across both paths of a merge.
struct Group {
    key: Key,
    /// Every statement either path used for the key, so transport sees each of them, each with
    /// the delay it was made with: two delays that round to one tick are one key, but a statement
    /// that is `LATER` by a bound refuses a commit through it that is any sooner than that bound
    /// (abi.md C3).
    statements: Vec<(u32, f64)>,
    /// The last commit of the key in the first and in the second path.
    last: [Option<Entry>; 2],
}

/// The commits of the two paths of one merge, as made.
struct Paths {
    /// The first path's commits, then the second's.
    entries: [Vec<Entry>; 2],
    table: Table,
}

impl Paths {
    /// The commits of the merge: the joined keys first, in the order they first appear, then
    /// what cannot be joined, in the order made.
    fn into_effects(self, panicked: bool) -> Vec<Entry> {
        let (mut groups, mut loose): (Vec<Group>, Vec<Entry>) = (Vec::new(), Vec::new());
        for (side, entries) in self.entries.into_iter().enumerate() {
            for entry in entries {
                let Some(key) = self.table.key(&entry) else {
                    loose.push(entry);
                    continue;
                };
                let at = groups.iter().position(|g| g.key == key).unwrap_or_else(|| {
                    groups.push(Group { key, statements: Vec::new(), last: [None, None] });
                    groups.len() - 1
                });
                if !groups[at].statements.iter().any(|(statement, _)| *statement == entry.statement) {
                    groups[at].statements.push((entry.statement, entry.after));
                }
                groups[at].last[side] = Some(entry);
            }
        }
        let mut effects: Vec<Entry> = groups.into_iter().flat_map(|group| group.effects(panicked)).collect();
        effects.extend(loose.into_iter().map(|entry| Self::loose_effect(entry, panicked)));
        effects
    }

    /// A commit that no key holds, on a path that may not have run (or panicked).
    fn loose_effect(mut entry: Entry, panicked: bool) -> Entry {
        match &entry.what {
            What::Record { maybe_at } => {
                // the record is delivered whatever happens: `maybe` already says it may not
                // have happened (§19 rule 1); a panicked merge keeps only that
                if panicked {
                    entry.bytes = Self::unknown_record(&entry.bytes, *maybe_at);
                }
            }
            What::Arm => {
                // an arming that may not have run: an unknown time does not fire (§13 rule 4)
                entry.flags |= commit_flag::MAYBE;
                if panicked {
                    entry.after = f64::INFINITY;
                }
            }
            What::Value(_) => {
                entry.flags |= commit_flag::MAYBE | if panicked { 0 } else { commit_flag::MERGE };
                if panicked {
                    entry.bytes = vec![0; entry.bytes.len()];
                }
            }
        }
        entry
    }

    /// A record with every field unknown (all-zero bytes, abi.md P3) except `maybe`, which is
    /// known and true (§1 rule 13, §19 rule 1).
    fn unknown_record(bytes: &[u8], maybe_at: usize) -> Vec<u8> {
        let mut unknown = vec![0; bytes.len()];
        Flagged::known(true).write(&mut unknown[maybe_at..]);
        unknown
    }
}

impl Entry {
    /// The bytes of this commit merged with those of `other`, a commit of the same key. Only a
    /// commit of a value has a merge, and only those have keys.
    fn merged_with(&self, other: &Entry) -> Vec<u8> {
        match self.what {
            What::Value(merge) => merge(&self.bytes, &other.bytes),
            What::Record { .. } | What::Arm => self.bytes.clone(),
        }
    }
}

impl Group {
    /// The commits this key makes, one through each statement used (see `Paths::into_effects`).
    fn effects(self, panicked: bool) -> Vec<Entry> {
        let [first, second] = self.last;
        let Some(template) = first.clone().or_else(|| second.clone()) else {
            return Vec::new();
        };
        let (bytes, flags) = match (panicked, first, second) {
            // §1 rule 13: every effect of a merge with a panicked path is unknown
            (true, ..) => (vec![0; template.bytes.len()], commit_flag::MAYBE),
            // both paths commit: their merge, final whichever path ran
            (false, Some(a), Some(b)) => (a.merged_with(&b), commit_flag::MAYBE | ((a.flags | b.flags) & commit_flag::MERGE)),
            // one path commits: the other leaves the carrier as it then is (§3.3.3)
            (false, ..) => (template.bytes.clone(), commit_flag::MAYBE | commit_flag::MERGE | template.flags),
        };
        self.statements.into_iter().map(|(statement, after)| Entry { statement, after, bytes: bytes.clone(), flags, ..template.clone() }).collect()
    }
}

/// The commits and records of one reaction, in the order they were made.
#[derive(Debug, Default)]
pub struct Commits {
    /// Every commit, a join of unknown paths replacing the commits of the paths it joined.
    entries: Vec<Entry>,
    /// What a join needs; empty for a reaction with no unknown decision.
    table: Table,
    /// How many paths taken under unknown control the code being run is inside of.
    maybe_depth: u32,
    /// Whether a `finish` was reached under known control (§19 Resolution).
    finished: bool,
}

impl Commits {
    /// The commits of a reaction with no unknown decision.
    pub fn new() -> Commits {
        Commits::default()
    }

    /// The commits of a reaction that decides on a value that may be unknown: `carriers[s]` is
    /// the carrier statement `s` commits to (`None` for a record or an arming), and `resolution`
    /// is the session's tick, so two delays that round to one tick are one key of a join.
    pub fn controlled(carriers: &'static [Option<u32>], resolution: f64) -> Commits {
        Commits { table: Table { carriers, resolution }, ..Commits::default() }
    }

    /// `carrier <- value after delay` through statement `statement`. An unknown delay commits
    /// `Unknown` at the next microstep (§3.3.3, RS-11 AC 6).
    pub fn commit<T: Carried>(&mut self, statement: u32, value: Flagged<T>, after: Flagged<f64>) {
        let (value, after) = if after.is_known() { (value, after.known_or(0.0)) } else { (Flagged::unknown(), 0.0) };
        self.entries.push(Entry { statement, bytes: value.encode(), after, flags: 0, what: What::Value(Flagged::<T>::merge_bytes) });
    }

    /// A record through statement `statement`: `bytes`, the whole layout of its stream's `RECORD`
    /// pin, due at the next microstep (abi.md C5: a record is a commit to that pin). `maybe_at` is
    /// the offset of the layout's `maybe` field (§19 rule 1): a merge with a panicked path keeps
    /// that field known and true and makes the others unknown (§1 rule 13).
    pub fn record_with_maybe(&mut self, statement: u32, bytes: &[u8], maybe_at: usize) {
        self.entries.push(Entry { statement, bytes: bytes.to_vec(), after: 0.0, flags: 0, what: What::Record { maybe_at } });
    }

    /// Arms a timer through statement `statement`, a `REPLACE` statement on an event pin (abi.md
    /// R1, C3), for the time `at` seen at the instant `now`: a later time is due `at - now`
    /// from now, the time of the instant itself at the next microstep (a delay of 0, C1), and an
    /// unknown or past time never (`+inf`, C1). Since the statement is `REPLACE`, each arming
    /// removes the firing an earlier one left pending, so a timer never fires for a time it no
    /// longer holds.
    pub fn arm(&mut self, statement: u32, at: Flagged<f64>, now: f64) {
        let after = match at.is_known() {
            true if at.known_or(0.0) > now => at.known_or(0.0) - now,
            true if at.known_or(0.0) == now => 0.0,
            _ => f64::INFINITY,
        };
        self.entries.push(Entry { statement, bytes: Vec::new(), after, flags: 0, what: What::Arm });
    }

    /// Whether the code being run is on a path taken under unknown control: the `maybe` of the
    /// records it makes (§19 rule 1).
    pub fn maybe(&self) -> Flagged<bool> {
        Flagged::known(self.maybe_depth > 0)
    }

    /// `if condition { then } else { otherwise }` in a reaction. A known condition runs the arm
    /// it names against these commits. An unknown one runs both, each as a path of its own under
    /// an `Attempt`, and joins what they committed (see the module); the value is the merge of
    /// the arms' values, `Unknown` when one panicked (§3.3.3, §1 rule 13).
    pub fn branch<T: Merge>(&mut self, condition: Flagged<bool>, then: impl FnOnce(&mut Commits) -> T, otherwise: impl FnOnce(&mut Commits) -> T) -> T {
        match Decision::of(condition) {
            Decision::Known(true) => then(self),
            Decision::Known(false) => otherwise(self),
            Decision::Unknown => {
                let start = self.entries.len();
                let first = self.path(then);
                let middle = self.entries.len();
                let second = self.path(otherwise);
                self.join(start, middle, first.is_none() || second.is_none());
                match (first, second) {
                    (Some(a), Some(b)) => a.merge(b),
                    _ => T::unknown(),
                }
            }
        }
    }

    /// Runs `body`, the commits of a reaction. When `definite` it runs as it is. Otherwise the
    /// reaction may not have run at all (a guard that is unknown, §13 rule 1), so its commits are
    /// one path joined with the empty path: each of them merged with not running it.
    pub fn run(&mut self, definite: bool, body: impl FnOnce(&mut Commits)) {
        if definite {
            body(self);
            return;
        }
        let start = self.entries.len();
        let ran = self.path(body);
        let middle = self.entries.len();
        self.join(start, middle, ran.is_none());
    }

    /// Runs `body` as one path taken under unknown control: `maybe` is true inside it, and a
    /// panic is caught (`None`) instead of faulting the run (§1 rule 13).
    fn path<T>(&mut self, body: impl FnOnce(&mut Commits) -> T) -> Option<T> {
        let depth = self.maybe_depth;
        self.maybe_depth = depth + 1;
        let result = Attempt::of(|| body(&mut *self));
        self.maybe_depth = depth;
        result
    }

    /// `finish(value)` (§19 Resolution): the analysis ends only when `value` is known true on a
    /// path that is not under unknown control, so a `finish` that may not have happened never
    /// ends it.
    pub fn finish(&mut self, value: Flagged<bool>) {
        if self.maybe_depth == 0 && value.high() {
            self.finished = true;
        }
    }

    /// What the reaction asks of the analysis.
    pub fn flow(&self) -> Flow {
        if self.finished { Flow::Finish } else { Flow::Continue }
    }

    /// Replaces the commits of two paths, `entries[start..middle]` and `entries[middle..]`, by
    /// the commits of their merge.
    fn join(&mut self, start: usize, middle: usize, panicked: bool) {
        let second = self.entries.split_off(middle);
        let first = self.entries.split_off(start);
        let paths = Paths { entries: [first, second], table: self.table };
        self.entries.extend(paths.into_effects(panicked));
    }

    /// Hands every commit to `cx`, in order; the first refusal is the fault. A commit with flags
    /// goes through `commit_with`.
    pub fn flush(self, cx: &mut React<'_>) -> Result<(), Fault> {
        for entry in &self.entries {
            if entry.flags == 0 {
                cx.commit(entry.statement, &entry.bytes, entry.after)?;
            } else {
                cx.commit_with(entry.statement, &entry.bytes, entry.after, entry.flags)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use piperine_abi::device::{Flow, React};
    use piperine_abi::raw::{commit_flag, status};
    use piperine_digital_solver::tick::{Due as SolverDue, Resolution, Tick};

    use super::{Commits, Due};
    use crate::rt::control::Panic;
    use crate::rt::flag::{Flagged, Merge};
    use crate::rt::int::UInt;

    type U8 = Flagged<UInt<8>>;

    /// A commit as the host receives it: `(statement, bytes, after, flags)`.
    type Made = (u32, Vec<u8>, f64, u32);

    const MAYBE: u32 = commit_flag::MAYBE;
    const MERGE: u32 = commit_flag::MERGE;
    const TICK: f64 = 1e-15;

    fn k<T: Copy + Default>(v: T) -> Flagged<T> {
        Flagged::known(v)
    }

    fn u<T: Copy + Default>() -> Flagged<T> {
        Flagged::unknown()
    }

    fn byte(v: u8) -> U8 {
        k(UInt::new(u64::from(v)))
    }

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

    /// What `commits` hands the host: every commit with its flags (0 for a plain `commit`).
    fn flushed(commits: Commits) -> Vec<Made> {
        let made = std::cell::RefCell::new(Vec::new());
        {
            let mut plain = |statement: u32, bytes: &[u8], after: f64| {
                made.borrow_mut().push((statement, bytes.to_vec(), after, 0));
                status::OK
            };
            let mut flagged = |statement: u32, bytes: &[u8], after: f64, flags: u32| {
                made.borrow_mut().push((statement, bytes.to_vec(), after, flags));
                status::OK
            };
            let mut cx = React::new(vec![], &mut plain).with_commit_with(&mut flagged);
            commits.flush(&mut cx).expect("every commit accepted");
        }
        made.into_inner()
    }

    /// Statements 0 and 1 commit to carrier 0, 2 to carrier 1, 3 to carrier 0 again, 4 is a
    /// record.
    const CARRIERS: &[Option<u32>] = &[Some(0), Some(0), Some(1), Some(0), None];

    fn controlled() -> Commits {
        Commits::controlled(CARRIERS, TICK)
    }

    #[test]
    fn a_join_merges_keys_by_carrier_and_due() {
        // §3.3.3 and design "Joining two paths": a key in both paths is the merge of the two
        // values, flagged MAYBE, through each path's own statement
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(0.0)), |c| c.commit(1, byte(6), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![0, 0], 0.0, MAYBE), (1, vec![0, 0], 0.0, MAYBE)]);
        // equal known values merge to that value, known (US-04 AC 4)
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(0.0)), |c| c.commit(1, byte(5), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, MAYBE), (1, vec![5, 1], 0.0, MAYBE)]);
        // a key in one path only is committed MAYBE | MERGE: merged with the value the carrier
        // holds then (AC 3)
        let mut c = controlled();
        c.branch(u(), |c| c.commit(2, byte(7), k(0.0)), |_| ());
        assert_eq!(flushed(c), [(2, vec![7, 1], 0.0, MAYBE | MERGE)]);
        // the key is (carrier, due): delays that round to one tick are one key (the second
        // is 1.2 ticks), delays a tick or more apart are two, and an `after <= 0` and a delay
        // below half a tick are the next microstep
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(1.0 * TICK)), |c| c.commit(1, byte(6), k(1.2 * TICK)));
        // each statement keeps the delay it was made with (1.2 ticks for statement 1)
        assert_eq!(flushed(c), [(0, vec![0, 0], TICK, MAYBE), (1, vec![0, 0], 1.2 * TICK, MAYBE)]);
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(1.0 * TICK)), |c| c.commit(1, byte(5), k(3.0 * TICK)));
        assert_eq!(flushed(c), [(0, vec![5, 1], TICK, MAYBE | MERGE), (1, vec![5, 1], 3.0 * TICK, MAYBE | MERGE)]);
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(-1.0)), |c| c.commit(1, byte(5), k(0.4 * TICK)));
        assert_eq!(flushed(c), [(0, vec![5, 1], -1.0, MAYBE), (1, vec![5, 1], 0.4 * TICK, MAYBE)]);
        // two carriers are two keys
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(0.0)), |c| c.commit(2, byte(5), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, MAYBE | MERGE), (2, vec![5, 1], 0.0, MAYBE | MERGE)]);
        // within a path the last commit of a key is its value, and every statement it used
        // still goes through the host (transport sees each)
        let mut c = controlled();
        c.branch(u(), |c| { c.commit(0, byte(1), k(0.0)); c.commit(3, byte(2), k(0.0)); }, |c| c.commit(1, byte(2), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![2, 1], 0.0, MAYBE), (3, vec![2, 1], 0.0, MAYBE), (1, vec![2, 1], 0.0, MAYBE)]);
        // a nested join keeps MERGE: the inner one-path commit is merged with the world, and the
        // outer merge with the other path's value stays merged with it
        let mut c = controlled();
        c.branch(
            u(),
            |c| c.branch(u(), |c| c.commit(0, byte(5), k(0.0)), |_| ()),
            |c| c.commit(1, byte(5), k(0.0)),
        );
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, MAYBE | MERGE), (1, vec![5, 1], 0.0, MAYBE | MERGE)]);
        // known control is no join: the commits are made as they are, with no flag (and reach
        // the host through the plain `commit`)
        let mut c = controlled();
        c.branch(k(true), |c| c.commit(0, byte(5), k(0.0)), |c| c.commit(1, byte(6), k(0.0)));
        c.branch(k(false), |c| c.commit(2, byte(5), k(0.0)), |c| c.commit(3, byte(6), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, 0), (3, vec![6, 1], 0.0, 0)]);
        // a branch yields the merge of its arms' values
        let mut c = controlled();
        assert_eq!(c.branch(u(), |_| byte(5), |_| byte(5)), byte(5));
        assert_eq!(c.branch(u(), |_| byte(5), |_| byte(6)), u());
        assert_eq!(c.branch(k(true), |_| byte(5), |_| byte(6)), byte(5));
    }

    #[test]
    fn maybe_is_true_inside_a_path_taken_under_unknown_control() {
        // §19 rule 1: a record made on such a path says `maybe`; the flag is known
        let mut c = controlled();
        assert_eq!(c.maybe(), k(false));
        c.branch(u(), |c| assert_eq!(c.maybe(), k(true)), |c| assert_eq!(c.maybe(), k(true)));
        assert_eq!(c.maybe(), k(false));
        c.run(false, |c| assert_eq!(c.maybe(), k(true)));
        c.run(true, |c| assert_eq!(c.maybe(), k(false)));
        // a record of either path is delivered, as made, through its own statement
        let mut c = controlled();
        c.branch(
            u(),
            |c| c.record_with_maybe(4, &[7, 1, 1, 1], 2),
            |c| c.record_with_maybe(4, &[8, 1, 1, 1], 2),
        );
        assert_eq!(flushed(c), [(4, vec![7, 1, 1, 1], 0.0, 0), (4, vec![8, 1, 1, 1], 0.0, 0)]);
    }

    #[test]
    fn a_reaction_that_may_not_run_merges_each_effect_with_not_running() {
        // §13 rule 1: a guard that is unknown runs the reaction as one path joined with the empty
        // path; a definite one runs it as it is
        let mut c = controlled();
        c.run(false, |c| c.commit(0, byte(5), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, MAYBE | MERGE)]);
        let mut c = controlled();
        c.run(true, |c| c.commit(0, byte(5), k(0.0)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 0.0, 0)]);
    }

    #[test]
    fn a_panicked_path_makes_every_effect_unknown() {
        // §1 rule 13: a panic on a path under unknown control is no fault, and every effect of
        // that merge is unknown: each key of either path is committed `Unknown`, a record keeps
        // only `maybe`; the value of the branch is unknown too
        let mut c = controlled();
        let value = c.branch(
            u(),
            |c| {
                c.commit(0, byte(5), k(0.0));
                c.record_with_maybe(4, &[7, 1, 1, 1], 2);
                Panic::raise("division by zero".to_string())
            },
            |c| {
                c.commit(2, byte(6), k(0.0));
                byte(9)
            },
        );
        assert_eq!(value, u());
        assert_eq!(
            flushed(c),
            [(0, vec![0, 0], 0.0, MAYBE), (2, vec![0, 0], 0.0, MAYBE), (4, vec![0, 0, 1, 1], 0.0, 0)]
        );
        // the panic is the other arm's too, and a panic of a reaction that may not have run
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(0.0)), |c| { c.commit(1, byte(5), k(0.0)); Panic::raise("x".to_string()) });
        assert_eq!(flushed(c), [(0, vec![0, 0], 0.0, MAYBE), (1, vec![0, 0], 0.0, MAYBE)]);
        let mut c = controlled();
        c.run(false, |c| { c.commit(0, byte(5), k(0.0)); Panic::raise("x".to_string()) });
        assert_eq!(flushed(c), [(0, vec![0, 0], 0.0, MAYBE)]);
        // under known control a panic is not caught: it is the run's fault
        let mut c = controlled();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.branch(k(true), |_| -> U8 { Panic::raise("x".to_string()) }, |_| byte(1)))).is_err());
    }

    #[test]
    fn a_maybe_finish_never_ends_the_run() {
        // §19 Resolution: `finish` ends the analysis only when known true and not `maybe`
        let flow = |body: &dyn Fn(&mut Commits)| {
            let mut c = controlled();
            body(&mut c);
            c.flow()
        };
        assert_eq!(flow(&|_| ()), Flow::Continue);
        assert_eq!(flow(&|c| c.finish(k(true))), Flow::Finish);
        assert_eq!(flow(&|c| c.finish(k(false))), Flow::Continue);
        assert_eq!(flow(&|c| c.finish(u())), Flow::Continue, "an unknown value does not finish");
        assert_eq!(flow(&|c| c.branch(u(), |c| c.finish(k(true)), |_| ())), Flow::Continue);
        assert_eq!(flow(&|c| c.branch(u(), |_| (), |c| c.finish(k(true)))), Flow::Continue);
        assert_eq!(flow(&|c| c.run(false, |c| c.finish(k(true)))), Flow::Continue);
        assert_eq!(flow(&|c| c.run(true, |c| c.finish(k(true)))), Flow::Finish);
        assert_eq!(flow(&|c| c.branch(k(true), |c| c.finish(k(true)), |_| ())), Flow::Finish);
        assert_eq!(flow(&|c| { c.branch(u(), |c| c.finish(k(true)), |_| ()); c.finish(k(true)) }), Flow::Finish);
    }

    #[test]
    fn a_joined_key_keeps_the_delay_of_each_statement() {
        // two delays one tick apart are one key (abi.md C1), yet each statement that made one
        // commits with its own delay: a `LATER` statement refuses any sooner than its bound
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), k(1e-9)), |c| c.commit(1, byte(5), k(1e-9 + 4e-16)));
        assert_eq!(flushed(c), [(0, vec![5, 1], 1e-9, MAYBE), (1, vec![5, 1], 1e-9 + 4e-16, MAYBE)]);
    }

    #[test]
    fn due_rounds_as_the_solver_does() {
        // abi.md C1: `after <= 0` or a delay that rounds to no tick is the next microstep,
        // otherwise the nearest tick (halves away from zero), `+inf` never, NaN refused
        let table = [
            (0.0, Due::NextMicrostep),
            (-3.0 * TICK, Due::NextMicrostep),
            (0.4 * TICK, Due::NextMicrostep),
            (0.5 * TICK, Due::Ticks(1)),
            (1.4 * TICK, Due::Ticks(1)),
            (1.6 * TICK, Due::Ticks(2)),
            (3.0 * TICK, Due::Ticks(3)),
            (f64::INFINITY, Due::Never),
            (f64::NAN, Due::Refused),
            (f64::MAX, Due::Refused),
        ];
        for (after, want) in table {
            assert_eq!(Due::of(after, TICK), want, "after = {after:e}");
        }
        // and against the solver's own rounding, across a sweep of delays
        let resolution = Resolution { seconds_per_tick: TICK };
        for n in 0..4000 {
            let after = f64::from(n) * 0.37 * TICK;
            let want = match resolution.due(Tick(0), after).expect("a representable delay") {
                SolverDue::NextMicrostep => Due::NextMicrostep,
                SolverDue::At(Tick(t)) => Due::Ticks(t),
                SolverDue::Never => Due::Never,
            };
            assert_eq!(Due::of(after, TICK), want, "after = {after:e}");
        }
        // a Refused delay is never joined with another: the solver names it (the test host
        // accepts it, so only the count and the flags are read)
        let mut c = controlled();
        c.branch(u(), |c| c.commit(0, byte(5), Flagged::known(f64::NAN)), |c| c.commit(1, byte(5), Flagged::known(f64::NAN)));
        let made = flushed(c);
        assert_eq!(made.len(), 2);
        assert!(made.iter().all(|m| m.3 == MAYBE | MERGE));
    }

    #[test]
    fn a_timer_is_armed_for_a_later_time_only() {
        // msl-digital A3 on abi.md C1, C4: an arming fires its event pin (no bytes) `t - now`
        // later, at the next microstep for `t == now`, and never (`+inf`) for a past or unknown
        // time
        let made = react_with(|cx| {
            let mut commits = Commits::new();
            commits.arm(0, k(15e-9), 5e-9);
            commits.arm(1, k(5e-9), 5e-9);
            commits.arm(2, k(0.0), 5e-9);
            commits.arm(3, u::<f64>(), 5e-9);
            commits.flush(cx).expect("every arming accepted");
        });
        assert_eq!(made, [(0, vec![], 15e-9 - 5e-9), (1, vec![], 0.0), (2, vec![], f64::INFINITY), (3, vec![], f64::INFINITY)]);
    }

    #[test]
    fn a_merge_of_nothing_is_nothing() {
        // a branch whose arms commit nothing commits nothing (spec edge case: neither arm commits)
        let mut c = controlled();
        c.branch(u(), |_| (), |_| ());
        assert_eq!(flushed(c), []);
        assert_eq!(<() as Merge>::unknown(), ());
    }
}

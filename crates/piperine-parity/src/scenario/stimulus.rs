//! [`Stimulus`] and [`Ramp`]: what a host does to a session during a run, kept as data so a
//! scenario states its whole timeline in one place beside the windows it judges.

use piperine_api::{Failure, ParamValue, Session};

/// One thing the host does to the session: a drive of a discrete `in` port of the top, or a timed
/// set of a param.
#[derive(Debug, Clone, PartialEq)]
enum Action {
    /// `Session::drive` of `port` to `value` at `at`.
    Drive { port: String, value: ParamValue, at: f64 },
    /// `Session::set_at` of `param` to `value` at `at`.
    Set { param: String, value: f64, at: f64 },
}

impl Action {
    /// The action as a failure names it.
    fn describe(&self) -> String {
        match self {
            Action::Drive { port, value, at } => format!("the drive of `{port}` to {value:?} at {at:e} s"),
            Action::Set { param, value, at } => format!("the set of `{param}` to {value} at {at:e} s"),
        }
    }

    /// Hands the action to the session.
    fn apply(&self, session: &mut Session) -> Result<(), Failure> {
        match self {
            Action::Drive { port, value, at } => session.drive(port, value.clone(), *at),
            Action::Set { param, value, at } => session.set_at(param, *value, *at),
        }
    }
}

/// An action the session refused: which one, and why.
#[derive(Debug)]
pub struct Refusal {
    /// The refused action, as text.
    pub action: String,
    /// The session's failure (an `E0801` to `E0806` of the host surface).
    pub failure: Failure,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} was refused: {}", self.action, self.failure)
    }
}

impl std::error::Error for Refusal {}

/// A staircase of a param from `from` to `to`: one set per `step` of value, equally spaced in
/// time over `duration` from `start`. SPEC `examples/buck/SPEC.md` §10.1 defines the supply as an
/// ideal source behind 20 mohm with no slew, so a ramp of it is a staircase of small sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    /// The value before the ramp.
    pub from: f64,
    /// The value at the end of the ramp.
    pub to: f64,
    /// The time the ramp starts, in seconds.
    pub start: f64,
    /// How long the ramp lasts, in seconds.
    pub duration: f64,
    /// The size of one step of value.
    pub step: f64,
}

impl Ramp {
    /// The `(time, value)` of each set: with `n = max(1, round(|to - from| / step))`, point `k` (1 to `n`)
    /// is at `start + k * duration / n` with the value `from + k * (to - from) / n`. The last
    /// point is exactly `start + duration` and exactly `to`, whatever the rounding of the others. A ramp shorter than half a step is one set, so it still
    /// reaches its target.
    pub fn points(&self) -> Vec<(f64, f64)> {
        assert!(self.step > 0.0, "a ramp needs a positive step, got {:e}", self.step);
        let count = (((self.to - self.from).abs() / self.step).round() as usize).max(1);
        (1..=count)
            .map(|k| {
                if k == count {
                    return (self.start + self.duration, self.to);
                }
                let share = k as f64 / count as f64;
                (self.start + share * self.duration, self.from + share * (self.to - self.from))
            })
            .collect()
    }
}

/// The host's schedule for one run, applied to a [`Session`] in the order it was given.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stimulus {
    /// The actions, in the order they are applied.
    actions: Vec<Action>,
}

impl Stimulus {
    /// A schedule that does nothing.
    pub fn new() -> Stimulus {
        Stimulus::default()
    }

    /// Adds the drive of the port `port` to `value` at `at`.
    pub fn drive(mut self, port: &str, value: ParamValue, at: f64) -> Stimulus {
        self.actions.push(Action::Drive { port: port.to_string(), value, at });
        self
    }

    /// Adds the set of the param `param` (`<instance>.<param>`) to `value` at `at`.
    pub fn set(mut self, param: &str, value: f64, at: f64) -> Stimulus {
        self.actions.push(Action::Set { param: param.to_string(), value, at });
        self
    }

    /// Adds one set of `param` per point of `ramp`.
    pub fn ramp(self, param: &str, ramp: Ramp) -> Stimulus {
        ramp.points().into_iter().fold(self, |stimulus, (at, value)| stimulus.set(param, value, at))
    }

    /// Hands every action to `session`, in the order given; the first one the session refuses is
    /// returned with the action named, and the rest are not applied.
    pub fn apply(&self, session: &mut Session) -> Result<(), Refusal> {
        self.actions.iter().try_for_each(|action| action.apply(session).map_err(|failure| Refusal { action: action.describe(), failure }))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use piperine_api::{Cell, Overrides, Project, Record, RecordSink};
    use piperine_transpile::Settings;

    use super::*;
    use crate::scenario::planted::PlantedPackage;

    /// A sink that keeps nothing: these tests read the table.
    struct Discard;

    impl RecordSink for Discard {
        fn record(&mut self, _: &Record) {}
    }

    /// Sessions the tests open.
    struct Sessions;

    impl Sessions {
        /// Opens the `short` run of the planted package.
        fn open(package: &PlantedPackage, cache: &tempfile::TempDir) -> Session {
            let project = Project::at(package.path()).unwrap_or_else(|f| panic!("{f}"));
            let spec = project.request(Some("short"), &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
            Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"))
        }
    }

    #[test]
    fn a_ramp_sets_once_per_equal_step_and_ends_on_its_target() {
        // 12 -> 16 V in 10 us in 0.1 V steps: 40 points 0.25 us apart, the last 16 V at 10 us
        let up = Ramp { from: 12.0, to: 16.0, start: 2e-3, duration: 10e-6, step: 0.1 }.points();
        assert_eq!(up.len(), 40);
        assert_eq!(up[0].0, 2e-3 + 0.25e-6);
        assert!((up[1].0 - up[0].0 - 0.25e-6).abs() < 1e-15 && (up[1].1 - up[0].1 - 0.1).abs() < 1e-12);
        assert_eq!(up[39], (2e-3 + 10e-6, 16.0));
        // 12 -> 7.5 V in 90 us: 45 falling points 2 us apart
        let down = Ramp { from: 12.0, to: 7.5, start: 1.25e-3, duration: 90e-6, step: 0.1 }.points();
        assert_eq!(down.len(), 45);
        assert!((down[0].0 - (1.25e-3 + 2e-6)).abs() < 1e-15 && (down[0].1 - 11.9).abs() < 1e-12);
        assert_eq!(down[44], (1.25e-3 + 90e-6, 7.5));
        assert!(down.windows(2).all(|pair| (pair[1].0 - pair[0].0 - 2e-6).abs() < 1e-15 && pair[1].1 < pair[0].1));
        // 12 -> 12.04 V is under half a step: one set, still at the end on the target
        let sliver = Ramp { from: 12.0, to: 12.04, start: 1e-3, duration: 5e-6, step: 0.1 }.points();
        assert_eq!(sliver, vec![(1e-3 + 5e-6, 12.04)]);
    }

    #[test]
    fn a_stimulus_is_applied_to_the_session_and_its_drives_and_sets_land() {
        let package = PlantedPackage::write();
        let cache = tempfile::tempdir().expect("a build cache");
        let mut session = Sessions::open(&package, &cache);
        let accepted = Stimulus::new()
            .drive("level", ParamValue::Int(5), 0.0)
            .drive("level", ParamValue::Int(9), 1e-6)
            .set("r1.r", 2000.0, 1e-6)
            .ramp("r1.r", Ramp { from: 2000.0, to: 1000.0, start: 1.2e-6, duration: 0.4e-6, step: 500.0 });
        accepted.apply(&mut session).unwrap_or_else(|refusal| panic!("{refusal}"));
        let table = session.run(Rc::new(RefCell::new(Discard))).unwrap_or_else(|f| panic!("{f}"));
        let level = table.columns.iter().position(|label| label.ends_with("level")).expect("a level column");
        let (first, last) = (&table.rows[0][level], &table.rows.last().expect("rows")[level]);
        assert_eq!((first, last), (&Cell::Int(5), &Cell::Int(9)), "the drive at 0 is the initial value, the one at 1 us lands");
    }

    #[test]
    fn a_refused_action_is_returned_naming_it() {
        let package = PlantedPackage::write();
        let cache = tempfile::tempdir().expect("a build cache");
        let mut refused = Sessions::open(&package, &cache);
        let refusal = Stimulus::new().drive("nothing", ParamValue::Bool(true), 0.0).apply(&mut refused).expect_err("no such port");
        assert!(matches!(refusal.failure, Failure::Host { code: "E0801", .. }), "{refusal}");
        assert!(refusal.action.contains("the drive of `nothing`"), "{refusal}");
    }
}

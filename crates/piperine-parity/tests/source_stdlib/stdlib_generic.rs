//! The standard library items built on generics, run from MSL source on the Rust backend (feature `generics` phase E, GN-28):
//! each model is written in `tests/stdlib_generic/`, elaborated by the front end, compiled on the transpiled backend and run
//! on the solver. Every expected value is computed here by Rust from the rule of Part VII §22 the test cites, never read back
//! from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Field, Overrides, Project as HostProject, Record, RecordSink, Session, Streams};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};
use piperine_parity::buck_fixture::BuckFixture;

/// The models of `tests/stdlib_generic/` and what they record.
struct StdModels;

impl StdModels {
    /// The project of the model file `file`.
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stdlib_generic").join(file);
        Project::new(&path)
    }

    /// Every record of the transient of `top` of `file` up to `stop` seconds at `step`, decoded as the host decodes it.
    fn records(file: &str, top: &str, stop: f64, step: f64) -> Vec<Record> {
        let built = BuckFixture::build(&Self::project(file), top);
        let streams = Streams::of(built.ir.clone());
        let seen = built.run(stop, step);
        seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect()
    }

    /// The voltage of each node in `names` of `top` of `file` at its operating point.
    fn operating_point(file: &str, top: &str, names: &[&str]) -> Vec<f64> {
        let mut running = Self::project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        names
            .iter()
            .map(|name| {
                let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
                solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
            })
            .collect()
    }

    /// The records of the stream `stream`, in time order.
    fn of_stream<'r>(records: &'r [Record], stream: &str) -> Vec<&'r Record> {
        records.iter().filter(|record| record.stream == stream).collect()
    }

    /// The real field `name` of `record`.
    fn real(record: &Record, name: &str) -> f64 {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Real(value))) => *value,
            other => panic!("{name} is a real: {other:?}"),
        }
    }

    /// The integer field `name` of `record`.
    fn integer(record: &Record, name: &str) -> i128 {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Int(value))) => (*value).into(),
            other => panic!("{name} is an integer: {other:?}"),
        }
    }

    /// The boolean field `name` of `record`.
    fn boolean(record: &Record, name: &str) -> bool {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Bool(value))) => *value,
            other => panic!("{name} is a bool: {other:?}"),
        }
    }

    /// The checks of the stream `checks` named `name`, `(time, pass)` in order.
    fn checks_named(records: &[Record], name: &str) -> Vec<(f64, bool)> {
        Self::of_stream(records, "checks")
            .into_iter()
            .filter(|record| matches!(record.fields.iter().find(|(field, _)| field == "name"), Some((_, Field::Text(text))) if text == name))
            .map(|record| (record.time, Self::boolean(record, "pass")))
            .collect()
    }
}

/// Rust's own reading of the §22 rules the tests cite.
struct Section22;

impl Section22 {
    /// §22.1.8: `since` is `now - t` of the last change recorded at or before `now`, `inf` while none is.
    fn since(changes: &[f64], now: f64) -> f64 {
        changes.iter().rev().find(|change| **change <= now).map_or(f64::INFINITY, |change| now - change)
    }

    /// §22.1.8 rule 2: a reaction on the change `k` reads the time since the change before it (`inf` for the first, rule 3).
    fn since_before(changes: &[f64], k: usize) -> f64 {
        Self::since(&changes[..k], changes[k])
    }

    /// §22.5.7: at each change of `d` the level that ends lasted the time since the change before; the check passes when
    /// that width is at least `w`.
    fn min_pulse_passes(changes: &[f64], w: f64) -> Vec<bool> {
        (0..changes.len()).map(|k| Self::since_before(changes, k) >= w).collect()
    }

    /// §22.5.8: the output of an inertial delay `tpd` over the input that holds `first` at (0, 0) and takes `value` at `time`
    /// for each of `changes`: a change is delivered at `time + tpd` when no later change comes before, and is an output
    /// change only when it differs from what the output holds. `(time, value)` in order, the first commit at (0, 0) included.
    fn inertial<V: PartialEq + Copy>(first: V, changes: &[(f64, V)], tpd: f64) -> Vec<(f64, V)> {
        let mut output = vec![(0.0, first)];
        for (k, (time, value)) in changes.iter().enumerate() {
            let delivered = time + tpd;
            let swallowed = changes.get(k + 1).is_some_and(|(next, _)| *next < delivered);
            if !swallowed && output.last().is_some_and(|(_, held)| held != value) {
                output.push((delivered, *value));
            }
        }
        output
    }

    /// The value an output given by [`Section22::inertial`] holds at `at`.
    fn held_at<V: Clone>(output: &[(f64, V)], at: f64) -> V {
        output.iter().rev().find(|(time, _)| *time <= at).map(|(_, value)| value.clone()).expect("the output starts at (0, 0)")
    }

    /// Whether two readings are the same real, `inf` included, up to the femtosecond the solver counts time in.
    pub fn same_reading(left: f64, right: f64) -> bool {
        left == right || (left - right).abs() < 1e-14
    }
}

#[test]
fn since_reads_the_time_from_the_last_change() {
    // §22.1.8: `d` commits its first value at (0, 0) (a change, §13 rule 6), rises at 3 ns and falls at 8 ns. A reaction on
    // each change reads the time since the one before (`inf` at the first, rules 2 and 3); a read at 2, 5 and 9 ns reads
    // `now - t_change`
    let changes = [0.0, 3e-9, 8e-9];
    let probes = [2e-9, 5e-9, 9e-9];
    let records = StdModels::records("since.msl", "SinceBench", 20e-9, 1e-9);
    let at_change = StdModels::of_stream(&records, "at_change");
    assert_eq!(at_change.len(), changes.len(), "one reading per change: {records:?}");
    for (k, record) in at_change.iter().enumerate() {
        assert!(Section22::same_reading(record.time, changes[k]), "the change {k} at {}", record.time);
        let (value, wanted) = (StdModels::real(record, "value"), Section22::since_before(&changes, k));
        assert!(Section22::same_reading(value, wanted), "since in the reaction of change {k}: {value} against {wanted}");
    }
    let read = StdModels::of_stream(&records, "age");
    assert_eq!(read.len(), probes.len(), "one reading per probe: {records:?}");
    for (record, probe) in read.iter().zip(probes) {
        assert!(Section22::same_reading(record.time, probe), "read at {probe}: {}", record.time);
        let (value, wanted) = (StdModels::real(record, "value"), Section22::since(&changes, probe));
        assert!(Section22::same_reading(value, wanted), "since at {probe}: {value} against {wanted}");
    }
}

#[test]
fn min_pulse_fails_exactly_for_a_narrow_pulse() {
    // §22.5.7: `MinPulse { w }` checks at each change of `d` that the level that just ended lasted at least `w` = 1.5 ns:
    // `d` commits at (0, 0) and changes at 2, 3, 10, 11, 15, 19 and 20 ns, so the levels last inf, 2, 1, 7, 1, 4, 4 and 1 ns
    let changes = [0.0, 2e-9, 3e-9, 10e-9, 11e-9, 15e-9, 19e-9, 20e-9];
    let records = StdModels::records("since.msl", "PulseBench", 30e-9, 1e-9);
    let checks = StdModels::of_stream(&records, "checks");
    let wanted = Section22::min_pulse_passes(&changes, 1.5e-9);
    assert_eq!(checks.len(), changes.len(), "one check per change: {records:?}");
    for ((record, change), pass) in checks.iter().zip(changes).zip(&wanted) {
        assert!(Section22::same_reading(record.time, change), "checked at {change}: {}", record.time);
        assert_eq!(StdModels::boolean(record, "pass"), *pass, "the check at {change}");
    }
    assert!(wanted.iter().any(|pass| !pass) && wanted.iter().any(|pass| *pass), "the table holds narrow and wide levels");
}

/// The schedule of `monitors.msl`: when `clk` is high, when `en` is high and when `d` changes, in seconds.
struct MonitorSchedule;

impl MonitorSchedule {
    /// The rising edges of `clk`.
    const RISES: [f64; 4] = [10e-9, 30e-9, 50e-9, 70e-9];
    /// The falling edges of `clk`.
    const FALLS: [f64; 4] = [20e-9, 40e-9, 60e-9, 80e-9];
    /// The interval `[from, to)` in which `en` is high.
    const ENABLED: (f64, f64) = (40e-9, 60e-9);
    /// The changes of `d` after its first commit at (0, 0).
    const CHANGES: [f64; 8] = [8e-9, 24e-9, 33e-9, 40e-9, 46e-9, 58e-9, 62e-9, 66e-9];
    /// `ts` of `Setup` and `th` of `Hold`.
    const MARGIN: f64 = 5e-9;

    /// The start of the high phase of `clk` that holds `at`, when it holds one.
    fn high_since(at: f64) -> Option<f64> {
        Self::RISES.iter().zip(Self::FALLS).find(|(rise, fall)| **rise <= at && at < *fall).map(|(rise, _)| *rise)
    }

    /// Every change of `d` the monitors see, the first commit at (0, 0) included.
    fn all_changes() -> Vec<f64> {
        std::iter::once(0.0).chain(Self::CHANGES).collect()
    }

    /// §22.5.5: at each rising edge `Setup` passes when `d` last changed at least `ts` before it (a change in the edge's own
    /// microstep is not yet recorded, rule 2).
    fn setup_passes() -> Vec<bool> {
        let changes = Self::all_changes();
        Self::RISES.iter().map(|rise| Section22::since(&changes, *rise - 1e-15) >= Self::MARGIN).collect()
    }

    /// §22.5.6: at each change of `d` while `clk` is high, `Hold` passes when the change is not in the edge's own microstep
    /// and comes at least `th` after the rise; the first commit is no check (rule 4). `(change, pass)` in order.
    fn hold_checks() -> Vec<(f64, bool)> {
        Self::CHANGES.iter().filter_map(|change| Self::high_since(*change).map(|rise| (*change, *change - rise >= Self::MARGIN))).collect()
    }

    /// §22.5.4: each change of `d` while `en` is high, in a microstep in which `en` does not change, is a failure.
    fn stable_failures() -> Vec<f64> {
        let (from, to) = Self::ENABLED;
        Self::CHANGES.iter().copied().filter(|change| *change > from && *change < to).collect()
    }
}

#[test]
fn setup_hold_and_stable_fail_exactly_on_their_violations() {
    // §22.5.4 to §22.5.6, at `T = u8` and at an enum: each monitor's checks are the ones Rust derives from the schedule
    for bench in ["ByteBench", "LevelBench"] {
        let records = StdModels::records("monitors.msl", bench, 90e-9, 1e-9);
        let setup = StdModels::checks_named(&records, "setup");
        let wanted = MonitorSchedule::setup_passes();
        assert_eq!(setup.len(), wanted.len(), "{bench}: one setup check per rising edge: {setup:?}");
        for (((time, pass), rise), expected) in setup.iter().zip(MonitorSchedule::RISES).zip(&wanted) {
            assert!(Section22::same_reading(*time, rise), "{bench}: setup checked at {rise}: {time}");
            assert_eq!(pass, expected, "{bench}: setup at {rise}");
        }
        let hold = StdModels::checks_named(&records, "hold");
        let wanted = MonitorSchedule::hold_checks();
        assert_eq!(hold.len(), wanted.len(), "{bench}: one hold check per change of d while clk is high: {hold:?}");
        for ((time, pass), (change, expected)) in hold.iter().zip(&wanted) {
            assert!(Section22::same_reading(*time, *change), "{bench}: hold checked at {change}: {time}");
            assert_eq!(pass, expected, "{bench}: hold at {change}");
        }
        let stable = StdModels::checks_named(&records, "stable");
        let wanted = MonitorSchedule::stable_failures();
        assert_eq!(stable.len(), wanted.len(), "{bench}: one failure per change of d while en is high: {stable:?}");
        for ((time, pass), change) in stable.iter().zip(&wanted) {
            assert!(Section22::same_reading(*time, *change) && !pass, "{bench}: stable fails at {change}: {time} {pass}");
        }
        // §22.5.5 rule 3: `viol` read 5 ns after each rising edge is high exactly when that edge violated
        let viol: Vec<bool> = StdModels::of_stream(&records, "viol").iter().map(|record| StdModels::boolean(record, "value")).collect();
        let violated: Vec<bool> = MonitorSchedule::setup_passes().iter().map(|pass| !pass).collect();
        assert_eq!(viol, violated, "{bench}: viol follows the setup check");
    }
    assert!(MonitorSchedule::setup_passes().contains(&false) && MonitorSchedule::setup_passes().contains(&true));
    assert!(MonitorSchedule::hold_checks().iter().any(|(_, pass)| !pass) && MonitorSchedule::hold_checks().iter().any(|(_, pass)| *pass));
}

/// What the benches of `inertial.msl` record, as the variant names Rust writes for its own values.
struct InertialLabels;

impl InertialLabels {
    /// The input table of `LevelBench`: a level held 20 ns, a 3 ns pulse, a level held 6 ns.
    const LEVELS: [(f64, &'static str); 6] = [(10e-9, "Mid"), (30e-9, "Low"), (50e-9, "High"), (53e-9, "Mid"), (70e-9, "High"), (76e-9, "Low")];
    /// The input table of `ModeBench`, the same instants.
    const MODES: [(f64, &'static str); 6] = [(10e-9, "Run"), (30e-9, "Idle"), (50e-9, "Run"), (53e-9, "Idle"), (70e-9, "Run"), (76e-9, "Idle")];

    /// The variant name the `y` record holds.
    fn recorded(record: &Record) -> String {
        match record.fields.iter().find(|(field, _)| field == "value") {
            Some((_, Field::Variant(name))) => name.clone(),
            other => panic!("a y record holds a variant: {other:?}"),
        }
    }
}

#[test]
fn inertial_delays_and_swallows_short_pulses() {
    // §22.5.8 rules 1 and 2, at two enums with a `Default`: with `tpd` = 5 ns a level held 20 ns and one held 6 ns pass 5 ns
    // late, and the 3 ns pulse does not pass: `y`, read every ns, is what Rust's filter holds at that instant
    let tpd = 5e-9;
    for (bench, first, table) in [("LevelBench", "Low", InertialLabels::LEVELS), ("ModeBench", "Idle", InertialLabels::MODES)] {
        let held = Section22::inertial(first, &table, tpd);
        assert!(held.len() < table.len() + 1, "{bench}: the 3 ns pulse is swallowed");
        let records = StdModels::records("inertial.msl", bench, 100e-9, 1e-9);
        let seen = StdModels::of_stream(&records, "y");
        assert!(seen.len() >= 99, "{bench}: one reading per ns: {}", seen.len());
        for record in seen {
            assert_eq!(InertialLabels::recorded(record), Section22::held_at(&held, record.time), "{bench}: y at {}", record.time);
        }
    }
}

#[test]
fn inertial_at_a_type_without_default_is_refused() {
    // §22.5.8 rule 7 and §3.3.4: `bool` does not implement `Default`, so `inertial` over a `Bit` is `E0187`
    let refused = StdModels::project("inertial_bool.msl").elaborate("BoolBench").expect_err("bool has no Default").to_string();
    assert!(refused.contains("E0187") && refused.contains("Default"), "{refused}");
}

#[test]
fn short_makes_one_node_against_section_eleven() {
    // §11: the two nodes a `Short` joins are one: a divider of 1k over 3k from 4 V has its joined middle at
    // 4 * 3k / (1k + 3k) = 3 V on both, and the 4 V - 3 V over 1k is the 1 mA the lower 3k carries; over `Electrical`
    // and over `Thermal`
    let (top, upper, lower) = (4.0_f64, 1e3_f64, 3e3_f64);
    let middle = top * lower / (upper + lower);
    for rig in ["ElectricalRig", "ThermalRig"] {
        let seen = StdModels::operating_point("short.msl", rig, &["top", "mid", "other"]);
        assert!((seen[1] - middle).abs() < 1e-6 && (seen[2] - middle).abs() < 1e-6, "{rig}: one node at {middle}: {seen:?}");
        let (through_upper, through_lower) = ((seen[0] - seen[1]) / upper, seen[2] / lower);
        assert!((through_upper - through_lower).abs() < 1e-9 && (through_upper - top / (upper + lower)).abs() < 1e-9, "{rig}: one current: {through_upper} {through_lower}");
    }
}


#[test]
fn the_water_model_boils_and_vaporizes() {
    // Part IV §13: the water warms from 293.15 K at `P / (mass * cl)` kelvin per second and starts to boil at 373.15 K; the
    // heat then vaporizes it for `mass * hv / P` seconds, after which it is vapor. The instants of the two phase changes
    // are the ones Rust computes from the model's constants (the 1 mW/K leak is 1e-9 of the heater and moves them less
    // than the tolerance)
    let (power, mass, cl, hv) = (1e6_f64, 1.0_f64, 4.18e3_f64, 2.26e6_f64);
    let (ambient, boiling) = (293.15_f64, 373.15_f64);
    let started = mass * cl * (boiling - ambient) / power;
    let vaporized = started + mass * hv / power;
    let records = StdModels::records("water.msl", "Boiler", 3.0, 5e-3);
    let phases: Vec<(f64, String)> = StdModels::of_stream(&records, "phase")
        .iter()
        .map(|record| match record.fields.iter().find(|(field, _)| field == "value") {
            Some((_, Field::Variant(name))) => (record.time, name.clone()),
            other => panic!("a phase record holds a variant: {other:?}"),
        })
        .collect();
    let names: Vec<&str> = phases.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, ["Liquid", "Boiling", "Vapor"], "{phases:?}");
    assert!((phases[1].0 - started).abs() < 0.01 * started, "boiling starts at {started}: {phases:?}");
    assert!((phases[2].0 - vaporized).abs() < 0.01 * vaporized, "vaporized at {vaporized}: {phases:?}");
}

/// The closed forms of `integrals.msl`, from §22.2.8 and §22.2.9.
struct Integrals;

impl Integrals {
    /// The rate of `idt`'s input, in units per second, and its initial condition.
    const RATE: f64 = 1e6;
    /// The initial condition of `idt`.
    const INITIAL: f64 = 0.25;
    /// The rate of `idtmod`'s input, in units per second.
    const WRAPPED_RATE: f64 = 0.3e6;
    /// The modulus `idtmod` wraps by.
    const MODULUS: f64 = 1.0;
    /// The offset `idtmod`'s range starts at.
    const OFFSET: f64 = -0.5;
    /// The interval of `rst`, in seconds.
    const RESET: (f64, f64) = (5e-6, 7e-6);

    /// §22.2.8: `ic` plus the integral of a constant.
    fn plain(at: f64) -> f64 {
        Self::INITIAL + Self::RATE * at
    }

    /// §22.2.8 rule on `reset`: the result holds `ic` while `reset` holds and integrates from it after.
    fn held(at: f64) -> f64 {
        let (from, to) = Self::RESET;
        if at < from { Self::plain(at) } else if at < to { Self::INITIAL } else { Self::INITIAL + Self::RATE * (at - to) }
    }

    /// §22.2.9: the integral wrapped into `[offset, offset + modulus)`.
    fn wrapped(at: f64) -> f64 {
        Self::OFFSET + (Self::WRAPPED_RATE * at - Self::OFFSET).rem_euclid(Self::MODULUS)
    }
}

#[test]
fn idt_and_idtmod_integrate_against_section_22() {
    // §22.2.8 and §22.2.9, read every us half a us past the instant: `idt` of a constant is `ic + x t` (at `State` and at
    // `Thermal`), a `reset` holds `ic` and the integral resumes from it, and `idtmod` is the same integral wrapped, across its wraps (`fix/idtmod-wrap`)
    let records = StdModels::records("integrals.msl", "IntegralBench", 12e-6, 25e-9);
    let read = StdModels::of_stream(&records, "integral");
    assert!(read.len() >= 11, "one reading per us: {}", read.len());
    for record in read {
        let at = record.time;
        let value = |name: &str| StdModels::real(record, name);
        assert!((value("plain") - Integrals::plain(at)).abs() < 1e-3, "idt at {at}: {}", value("plain"));
        assert!((value("hot") - Integrals::plain(at)).abs() < 1e-3, "idt::<Thermal> at {at}: {}", value("hot"));
        assert!((value("held") - Integrals::held(at)).abs() < 1e-3, "idt with reset at {at}: {}", value("held"));
        assert!((value("wrapped") - Integrals::wrapped(at)).abs() < 1e-3, "idtmod at {at}: {}", value("wrapped"));
    }
}

/// The step responses of `laplace.msl`, from §22.2.14: a unit step at `STEP` through `H(s)` with unit DC gain.
struct Laplace;

impl Laplace {
    /// The instant of the step, in seconds.
    const STEP: f64 = 1e-6;
    /// The time constant of the first-order sections, in seconds.
    const TAU: f64 = 1e-6;
    /// The real part `a` of the second-order section's poles, `-a +- jb`.
    const A: f64 = 1e6;
    /// The imaginary part `b` of the second-order section's poles, `-a +- jb`.
    const B: f64 = 2e6;
    /// How far a first-order reading may sit from the closed form: the solver delivers 3.5e-7 at the bench's 5 ns step, so 2e-6 holds
    /// a margin of five, and a realization a step late (5e-3 on this section) or a pole 0.1 % off (4e-4) fails it.
    const FIRST_ORDER_TOLERANCE: f64 = 2e-6;
    /// How far a second-order reading may sit from the closed form: the solver delivers 3.2e-6 at the bench's 5 ns step, so 1e-5
    /// holds a margin of three, and a realization a step late or a pole 0.1 % off fails it by orders of magnitude more.
    const SECOND_ORDER_TOLERANCE: f64 = 1e-5;

    /// The response of `1 / (1 + s tau)` at `at`.
    fn first_order(at: f64) -> f64 {
        if at < Self::STEP { 0.0 } else { 1.0 - (-(at - Self::STEP) / Self::TAU).exp() }
    }

    /// The response of `1 / ((1 + s/p)(1 + s/conj p))` for `p = -a + jb` at `at`.
    fn second_order(at: f64) -> f64 {
        if at < Self::STEP {
            return 0.0;
        }
        let t = at - Self::STEP;
        1.0 - (-Self::A * t).exp() * ((Self::B * t).cos() + Self::A / Self::B * (Self::B * t).sin())
    }
}

#[test]
fn the_laplace_family_gives_the_closed_form_step_response() {
    // §22.2.14: four spellings of one low-pass (`tau = 1 us`) and a second-order section, read every half us after a step at
    // 1 us; the readings are the closed forms of the step response
    let records = StdModels::records("laplace.msl", "LaplaceBench", 8e-6, 5e-9);
    let read = StdModels::of_stream(&records, "laplace");
    assert!(read.len() >= 14, "one reading per half us: {}", read.len());
    for record in read {
        let at = record.time;
        for name in ["nd", "np", "zd", "zp"] {
            let seen = StdModels::real(record, name);
            assert!((seen - Laplace::first_order(at)).abs() < Laplace::FIRST_ORDER_TOLERANCE, "laplace_{name} at {at}: {seen} against {}", Laplace::first_order(at));
        }
        let seen = StdModels::real(record, "second");
        assert!((seen - Laplace::second_order(at)).abs() < Laplace::SECOND_ORDER_TOLERANCE, "second order at {at}: {seen} against {}", Laplace::second_order(at));
    }
}

#[test]
fn an_improper_or_unpaired_laplace_is_refused() {
    // §22.2.14: `N > D` fails the `where` of `laplace_nd` (E0304); a complex pole without its conjugate fails
    // `conjugate_closed` (E0317)
    let improper = StdModels::project("laplace_refused.msl").elaborate("ImproperBench").expect_err("N > D").to_string();
    assert!(improper.contains("E0304"), "{improper}");
    let unpaired = StdModels::project("laplace_refused.msl").elaborate("UnpairedBench").expect_err("an unpaired pole").to_string();
    assert!(unpaired.contains("E0317"), "{unpaired}");
}

/// The difference equation of `zi.msl`, from §22.2.12: `y[n] = sum num[k] x[n-k] - sum den[k] y[n-1-k]`, the history of a steady input
/// at the operating point (rule on `settle`): `x` held at its first value and `y` at the value that input settles to.
struct Difference;

impl Difference {
    /// The input of `ZiBench` at the sample `n` (at `n` us).
    fn input(n: usize) -> f64 {
        let at = n as f64 * 1e-6;
        if at < 2.5e-6 || at > 5.5e-6 { 1.0 } else { 0.0 }
    }

    /// `y[1]`, `y[2]`, ... of a filter with `num` and `den`, over `count` samples.
    fn run(num: &[f64], den: &[f64], count: usize) -> Vec<f64> {
        let steady = num.iter().sum::<f64>() * Self::input(0) / (1.0 + den.iter().sum::<f64>());
        let x = |n: i64| if n <= 0 { Self::input(0) } else { Self::input(n as usize) };
        let mut y: Vec<f64> = vec![steady];
        let mut out = Vec::new();
        for n in 1..=count as i64 {
            let history = |k: usize| y.get((n - 1 - k as i64) as usize).copied().unwrap_or(steady);
            let value = num.iter().enumerate().map(|(k, c)| c * x(n - k as i64)).sum::<f64>() - den.iter().enumerate().map(|(k, c)| c * history(k)).sum::<f64>();
            y.push(value);
            out.push(value);
        }
        out
    }
}

#[test]
fn the_zi_family_follows_its_difference_equation() {
    // §22.2.12 and §22.2.13: each reading, a quarter us after the sample n, is y[n] of the difference equation run in Rust over
    // the same input, the history before the first sample being the steady state of the operating point
    let records = StdModels::records("zi.msl", "ZiBench", 8e-6, 25e-9);
    let read = StdModels::of_stream(&records, "zi");
    assert!(read.len() >= 7, "one reading per us: {}", read.len());
    let first_order = Difference::run(&[0.5], &[-0.5], read.len());
    let with_zero = Difference::run(&[1.0, -0.5], &[-0.5], read.len());
    for (n, record) in read.iter().enumerate() {
        for (name, expected) in [("nd", first_order[n]), ("np", first_order[n]), ("zd", with_zero[n]), ("zp", with_zero[n])] {
            let seen = StdModels::real(record, name);
            assert!((seen - expected).abs() < 1e-6, "zi_{name} sample {}: {seen} against {expected}", n + 1);
        }
    }
}

#[test]
fn an_unpaired_zi_pole_is_refused() {
    // §22.2.13: a complex pole without its conjugate fails `conjugate_closed` (E0317)
    let refused = StdModels::project("zi.msl").elaborate("ZiRefused").expect_err("an unpaired pole").to_string();
    assert!(refused.contains("E0317"), "{refused}");
}

/// A record sink that keeps nothing: the noise test reads the report.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// §22.2.15 `1C` over `table`: linear between the samples, constant outside them.
struct TableModel;

impl TableModel {
    /// The value at `x` of the table of `(x, y)` samples, in increasing `x`: linear between two samples, the end sample's value beyond them.
    fn linear_constant(table: &[(f64, f64)], x: f64) -> f64 {
        if x <= table[0].0 {
            return table[0].1;
        }
        for pair in table.windows(2) {
            if x <= pair[1].0 {
                return pair[0].1 + (pair[1].1 - pair[0].1) * (x - pair[0].0) / (pair[1].0 - pair[0].0);
            }
        }
        table[table.len() - 1].1
    }
}

#[test]
fn noise_table_gives_the_interpolated_psd() {
    // §22.2.3: the source's density is the table through `table_model(freq(), table, "1C")`: the table's value at its own
    // frequencies, linear between them and constant past the last; into 1 kohm the output density is `r sqrt(psd)`
    let table = [(1e3, 4e-18), (1e5, 1e-18), (5e5, 3e-18)];
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stdlib_generic/noise_table.msl");
    let project = HostProject::file(&path).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { top: Some("TableBench".into()), analysis: Some("noise".into()), output: Some("o".into()), from: Some(1e3), to: Some(1e6), per_decade: Some(1), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let dir = tempfile::tempdir().expect("a temp dir");
    let settings = piperine_transpile::Settings { rustc: None, cache: dir.path().to_path_buf() };
    let mut session = Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}"));
    session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    let output = session.noise_report().expect("a report").output.clone();
    let frequencies = [1e3, 1e4, 1e5, 1e6];
    assert_eq!(output.len(), frequencies.len());
    for (seen, f) in output.iter().zip(frequencies) {
        let wanted = 1e3 * TableModel::linear_constant(&table, f).sqrt();
        assert!((seen - wanted).abs() <= 1e-9 * wanted, "noise_table at {f} Hz: {seen:e}, table {wanted:e}");
    }
}

#[test]
fn inertial_over_an_option_reads_its_result_in_a_reaction() {
    // §22.5.8 over `Option<u8>` (GAPS L89, `Option` of a module result): `Some(3)` held 20 ns passes 5 ns late and back to `None`
    // 5 ns after it ends; the 3 ns `Some(4)` is swallowed. `y.is_some()` and `y.unwrap_or(0)` read what Rust's filter holds
    let table = [(10e-9, Some(3u8)), (30e-9, None), (50e-9, Some(4)), (53e-9, None)];
    let held = Section22::inertial(None, &table, 5e-9);
    let records = StdModels::records("inertial.msl", "OptionBench", 100e-9, 1e-9);
    let seen = StdModels::of_stream(&records, "y");
    assert!(seen.len() >= 99, "one reading per ns: {}", seen.len());
    for record in seen {
        let expected = Section22::held_at(&held, record.time);
        assert_eq!(StdModels::boolean(record, "some"), expected.is_some(), "y.is_some() at {}", record.time);
        assert_eq!(StdModels::integer(record, "n"), i128::from(expected.unwrap_or(0)), "y.unwrap_or(0) at {}", record.time);
    }
}

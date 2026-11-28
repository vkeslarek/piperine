//! Resolution by strength and the net modules of §22.3 and §22.4.3 run from MSL source on the Rust backend (feature
//! `resolved-nets`, RN-06, RN-07): each model is written in `tests/nets/`, over a nature the model declares
//! (`Line`, `impl Resolve<Driven>`; design D27), elaborated by the front end, compiled on the transpiled backend and run
//! on the solver. Every expected value is written from the rule it cites (§22.3.2's table, §22.3.5 to §22.3.8,
//! §22.4.3), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Failure, Field, Overrides, Project as HostProject, Record, RecordSink, Session, Streams};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::NodeIdentifier;
use piperine_transpile::Settings;

use piperine_parity::mixed;
#[allow(dead_code)]
use crate::support;

use mixed::{BuiltDesign, Seen};
use support::build;

/// What a `Line` holds: a level and a strength, in the numbers of `tests/nets/line.msl`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    /// 0 low, 1 high, 2 X, 3 Z.
    level: f64,
    /// 0 `HighZ`, 1 `Weak`, 2 `Pull`, 3 `Strong`, 4 `Supply`.
    strength: f64,
}

impl Held {
    /// Low.
    const LOW: f64 = 0.0;
    /// High.
    const HIGH: f64 = 1.0;
    /// Not determined.
    const X: f64 = 2.0;
    /// Not driven.
    const Z: f64 = 3.0;
    /// `HighZ`.
    const HIGH_Z: f64 = 0.0;
    /// `Weak`.
    const WEAK: f64 = 1.0;
    /// `Pull`.
    const PULL: f64 = 2.0;
    /// `Strong`.
    const STRONG: f64 = 3.0;
}

/// A model whose reader writes `level`, a number per change (`tests/nets/bus.msl`).
struct Levels;

impl Levels {
    /// The `level` fields of the stream `name` of `built`, in the order they were written.
    fn of(built: &BuiltDesign, seen: &Seen, name: &str, field: &str) -> Vec<f64> {
        let streams = Streams::of(built.ir.clone());
        seen.all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == name)
            .map(|r| match r.fields.iter().find(|(candidate, _)| candidate == field) {
                Some((_, Field::Real(value))) => *value,
                other => panic!("a real field `{field}`, found {other:?}"),
            })
            .collect()
    }
}

/// A sink that keeps nothing: a run is judged by what it returns.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// One MSL file of `tests/nets/`.
struct Model {
    /// The project read from that file.
    project: Project,
}

impl Model {
    /// The project of `tests/nets/<file>`.
    fn of(file: &str) -> Model {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nets").join(file);
        Model { project: Project::new(&root) }
    }

    /// The operating point of `top`, as the host runs it on the Rust backend.
    fn operating_point(file: &str, top: &str) -> Result<piperine_api::Table, Failure> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nets").join(file);
        let project = HostProject::file(&path).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some(top.into()), backend: Some("rust".into()), analysis: Some("op".into()), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}")).run(Rc::new(RefCell::new(Ignore)))
    }

    /// `(time, v(a))` of every accepted point of the transient of `top` to `stop`, and the net's history.
    fn pad(&self, top: &str, stop: f64) -> (Vec<(f64, f64)>, Vec<Held>) {
        let built = self.built(top);
        let a = NodeIdentifier::Anonymous(built.ir.node_at(&format!("{top}.a")).expect("the node a").id().index());
        let (seen, result) = built.run_with(stop, 0.1e-9);
        let node = result.iter().map(|point| (point.time(), point.get_node(&a).expect("v(a)"))).collect();
        let mut net = Model::held(&built, &seen);
        net.dedup();
        (node, net)
    }

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> BuiltDesign {
        build(&self.project, top)
    }

    /// What the stream `line` of `built` says the net held after each change, the first at `(0, 0)`.
    fn held(built: &BuiltDesign, seen: &Seen) -> Vec<Held> {
        let streams = Streams::of(built.ir.clone());
        seen.all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == "line")
            .map(|r| {
                let real = |name: &str| match r.fields.iter().find(|(field, _)| field == name) {
                    Some((_, Field::Real(value))) => *value,
                    other => panic!("a real field `{name}`, found {other:?}"),
                };
                Held { level: real("level"), strength: real("strength") }
            })
            .collect()
    }

    /// What the net of `top` held over a run of 12 ns (one clock cycle past the release), each change once: the value at `(0, 0)` and then the value after
    /// each change that moved it.
    fn history(&self, top: &str) -> Vec<Held> {
        let built = self.built(top);
        let seen = built.run(12e-9, 1e-9);
        let mut held = Model::held(&built, &seen);
        held.dedup();
        held
    }

    /// What the net of `top` holds at the end of a short run.
    fn settled(&self, top: &str) -> Held {
        let built = self.built(top);
        let seen = built.run(10e-9, 1e-9);
        Model::held(&built, &seen).last().copied().unwrap_or_else(|| panic!("{top}: the reader saw no value"))
    }
}

#[test]
fn strongest_gives_the_table_of_22_3_2() {
    // §22.3.2's table: no drive or all HighZ is Driven::Z (level Z, HighZ); one strongest drive is that drive; several of the
    // greatest strength with the same level give that level and strength; several whose levels differ give X at that strength
    let line = Model::of("line.msl");
    let expected = [
        ("NoneBench", Held { level: Held::Z, strength: Held::HIGH_Z }),
        ("AllHighZBench", Held { level: Held::Z, strength: Held::HIGH_Z }),
        ("OneStrongestBench", Held { level: Held::LOW, strength: Held::STRONG }),
        ("SameLevelBench", Held { level: Held::HIGH, strength: Held::PULL }),
        ("ConflictBench", Held { level: Held::X, strength: Held::STRONG }),
    ];
    for (top, held) in expected {
        assert_eq!(line.settled(top), held, "{top}");
    }
}

#[test]
fn strongest_of_equal_drives_is_that_drive_in_any_order() {
    // §22.3.2 rule 1: the result does not depend on the order of the drives (the order is the holders' instance paths, so
    // three benches with the same three drives under different names put a different one first), and `strongest([x, x])`
    // is `x`
    let line = Model::of("line.msl");
    let conflict = Held { level: Held::X, strength: Held::STRONG };
    for top in ["FirstOrderBench", "SecondOrderBench", "ThirdOrderBench"] {
        assert_eq!(line.settled(top), conflict, "{top}: a strong high and a strong low give X at Strong in any order");
    }
    assert_eq!(line.settled("EqualDrivesBench"), Held { level: Held::LOW, strength: Held::PULL }, "two equal drives are that drive");
}

#[test]
fn a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature() {
    // §22.3.5 over §3.8 and §4 rule 8: `Tribuf<W: Nature + Resolve<Driven>>` is a generic module whose `w` is a resolved
    // carrier of whatever nature it is placed on; on `Line`, an enabled one drives its input strongly and a disabled one
    // is `Driven::Z`
    let line = Model::of("line.msl");
    assert_eq!(line.settled("TribufOnBench"), Held { level: Held::HIGH, strength: Held::STRONG }, "oe high: the input at Strong");
    assert_eq!(line.settled("TribufOffBench"), Held { level: Held::Z, strength: Held::HIGH_Z }, "oe low: not driving");
}

#[test]
fn tribuf_drives_strong_or_high_impedance() {
    // §22.3.5: `Tribuf` drives its input at `Strong` while `oe` is high and `Driven::Z` while it is low, so a released one
    // leaves the net to the others
    let line = Model::of("line.msl");
    assert_eq!(line.settled("TribufOnBench"), Held { level: Held::HIGH, strength: Held::STRONG }, "oe high, d high");
    assert_eq!(line.settled("TribufLowBench"), Held { level: Held::LOW, strength: Held::STRONG }, "oe high, d low");
    assert_eq!(line.settled("TribufOffBench"), Held { level: Held::Z, strength: Held::HIGH_Z }, "oe low, alone: not driven");
    assert_eq!(line.settled("TribufReleasedBench"), Held { level: Held::LOW, strength: 1.0 }, "oe low beside a weak low: the weak low");
}

#[test]
fn a_pull_wins_only_while_every_stronger_drive_is_released() {
    // §22.3.5: `Pullup` and `Pulldown` drive `Pull`; a `Strong` drive beats them, a `HighZ` one or a `Weak` one does not, and
    // a pull up against a pull down is X at `Pull`
    let line = Model::of("line.msl");
    let pull = |level: f64| Held { level, strength: Held::PULL };
    assert_eq!(line.settled("PullupAloneBench"), pull(Held::HIGH), "a pullup alone");
    assert_eq!(line.settled("PullupOverriddenBench"), Held { level: Held::LOW, strength: Held::STRONG }, "a strong low beats it");
    assert_eq!(line.settled("PullupReleasedBench"), pull(Held::HIGH), "a released low is left out");
    assert_eq!(line.settled("PullupOverWeakBench"), pull(Held::HIGH), "a weak low is weaker than a pull");
    assert_eq!(line.settled("PulldownAloneBench"), pull(Held::LOW), "a pulldown alone");
    assert_eq!(line.settled("PulldownReleasedBench"), pull(Held::LOW), "a released tribuf is left out");
    assert_eq!(line.settled("PulldownOverriddenBench"), Held { level: Held::HIGH, strength: Held::STRONG }, "an enabled tribuf beats it");
    assert_eq!(line.settled("PullContendedBench"), pull(Held::X), "two pulls that disagree");
}

#[test]
fn a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it() {
    // §22.3.6: a keeper drives X at `Weak` until the others have driven a 0 or a 1, then that level at `Weak`; a contended X
    // is not a level, so it is not recorded. A clock drives the tribufs from 5 ns to 10 ns.
    let line = Model::of("line.msl");
    let weak = |level: f64| Held { level, strength: Held::WEAK };
    let strong = |level: f64| Held { level, strength: Held::STRONG };
    assert_eq!(line.history("KeeperAloneBench"), vec![weak(Held::X)], "no level seen: weak X");
    assert_eq!(line.history("KeeperHoldsHighBench"), vec![weak(Held::X), strong(Held::HIGH), weak(Held::HIGH)], "the high is held after the release");
    assert_eq!(line.history("KeeperHoldsLowBench"), vec![weak(Held::X), strong(Held::LOW), weak(Held::LOW)], "the low is held after the release");
    assert_eq!(line.history("KeeperContendedBench"), vec![weak(Held::X), strong(Held::X), weak(Held::X)], "a contended X is not recorded");
}

#[test]
fn tran_drives_each_side_with_the_others_of_the_other() {
    // §22.3.7: with `en` high `Tran` drives each side with what the other side holds without it, so a strong level on one
    // side is on the other, in either direction; with `en` low it drives `Driven::Z` and the far side is not driven
    let line = Model::of("line.msl");
    assert_eq!(line.settled("TranOnBench"), Held { level: Held::HIGH, strength: Held::STRONG }, "a to b");
    assert_eq!(line.settled("TranBackwardBench"), Held { level: Held::LOW, strength: Held::STRONG }, "b to a");
    assert_eq!(line.settled("TranOffBench"), Held { level: Held::Z, strength: Held::HIGH_Z }, "en low: b is not driven");
}

#[test]
fn tgate_joins_while_s_high_or_sn_low() {
    // §22.3.8: `Tgate` is two `Tran`s, one enabled by `s` and one by `!sn`: it conducts when `s` is high, when `sn` is low,
    // and not when `s` is low and `sn` is high
    let line = Model::of("line.msl");
    let through = Held { level: Held::HIGH, strength: Held::STRONG };
    assert_eq!(line.settled("TgateNBench"), through, "s high, sn high: the n side");
    assert_eq!(line.settled("TgatePBench"), through, "s low, sn low: the p side");
    assert_eq!(line.settled("TgateOffBench"), Held { level: Held::Z, strength: Held::HIGH_Z }, "s low, sn high: open");
}

#[test]
fn two_parallel_trans_carry_a_strong_level_and_stay_two_carriers() {
    // Appendix C.2.3, RN-05 AC 3: two `Tran(a, b, true)` between `a` and `b`, one module driving `a` strong high: `b` takes
    // that level, `a` and `b` stay two nets (each reader hears its own net change), and a ring of `Tran`s settles on the level
    let line = Model::of("line.msl");
    let built = line.built("ParallelTranBench");
    let seen = built.run(10e-9, 1e-9);
    let tags = Levels::of(&built, &seen, "tagged", "tag");
    let levels = Levels::of(&built, &seen, "tagged", "level");
    let last_of = |tag: f64| tags.iter().zip(&levels).rev().find(|(candidate, _)| **candidate == tag).map(|(_, level)| *level);
    assert_eq!(last_of(1.0), Some(Held::HIGH), "a holds the strong high");
    assert_eq!(last_of(2.0), Some(Held::HIGH), "b takes the level through the two parallel trans");
    let ring = line.built("RingBench");
    let ring_seen = ring.run(10e-9, 1e-9);
    let ring_tags = Levels::of(&ring, &ring_seen, "tagged", "tag");
    let ring_levels = Levels::of(&ring, &ring_seen, "tagged", "level");
    for tag in [1.0, 2.0, 3.0] {
        let last = ring_tags.iter().zip(&ring_levels).rev().find(|(candidate, _)| **candidate == tag).map(|(_, level)| *level);
        assert_eq!(last, Some(Held::HIGH), "net {tag} of the ring settles on the level");
    }
}

/// The rail of the pad benches and what the load takes of it (`rout` 100 ohms against 100 kohms).
struct Rail;

impl Rail {
    /// The rail's voltage.
    const VOLTS: f64 = 1.8;

    /// The node of a pad whose load is `100 kohm` while it drives `level` of the rail through `rout` = 100 ohms.
    fn node(level: f64) -> f64 {
        level * Self::VOLTS * 100e3 / (100e3 + 100.0)
    }
}

#[test]
fn bidir_drives_the_node_while_the_wire_is_driven() {
    // §22.4.3 row 1, rules 1 and 2: the net is driven high, so the pad drives the net `Driven::Z` and the node as `D2A`
    // does: the rail through `rout` into the load, a divider. The net stays the tribuf's strong high
    let (node, net) = Model::of("line.msl").pad("BidirDrivenBench", 10e-9);
    let (time, volts) = *node.last().expect("a transient");
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("bidir driven", "v(a)", Coordinate::Time(time)), Rail::node(1.0), volts);
    assert_eq!(net, vec![Held { level: Held::HIGH, strength: Held::STRONG }], "the pad adds nothing to the net");
}

#[test]
fn bidir_drives_the_wire_from_the_node_when_released() {
    // §22.4.3 row 2: nobody else drives the net, so the pad makes no contribution to the node (an ideal source holds it) and
    // drives the net `Driven::strong(seen)`: the node thresholded, low at 0 V and high at the rail
    let line = Model::of("line.msl");
    let (_, high) = line.pad("BidirReleasedHighBench", 10e-9);
    assert_eq!(high.last(), Some(&Held { level: Held::HIGH, strength: Held::STRONG }), "the node is at the rail: {high:?}");
    let (_, low) = line.pad("BidirReleasedLowBench", 10e-9);
    assert_eq!(low.last(), Some(&Held { level: Held::LOW, strength: Held::STRONG }), "the node is at 0 V: {low:?}");
}

#[test]
fn bidir_acts_as_driven_on_a_contended_net() {
    // §22.4.3 row 1: the net is X at `Strong` (a strong high against a strong low), which is not `HighZ`, so the pad is
    // driven from `others().v`, and X reads as mid-rail
    let (node, net) = Model::of("line.msl").pad("BidirContendedBench", 10e-9);
    let (time, volts) = *node.last().expect("a transient");
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("bidir contended", "v(a)", Coordinate::Time(time)), Rail::node(0.5), volts);
    assert_eq!(net.last(), Some(&Held { level: Held::X, strength: Held::STRONG }), "the contended net: {net:?}");
}

#[test]
fn a_change_of_driving_is_a_time_point_and_a_jump() {
    // §22.4.3 rule 2: the tribuf starts driving at 5 ns, so `driving` changes there: the solver has a time point at 5 ns, the
    // node is at 0 V just before it (the pad released, the load to ground) and at the rail's divider a nanosecond after
    let (node, _) = Model::of("line.msl").pad("BidirToggleBench", 8e-9);
    assert!(node.iter().any(|(time, _)| (time - 5e-9).abs() < 1e-12), "a time point at 5 ns: {:?}", node.iter().map(|(time, _)| time).collect::<Vec<_>>());
    let before = node.iter().rev().find(|(time, _)| *time < 5e-9).expect("a point before").1;
    let after = node.iter().find(|(time, _)| *time >= 6e-9).expect("a point after").1;
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("bidir toggle", "v(a) before 5 ns", Coordinate::Time(5e-9)), 0.0, before);
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("bidir toggle", "v(a) after 6 ns", Coordinate::Time(6e-9)), Rail::node(1.0), after);
}

#[test]
fn two_bidirs_alone_stop_the_run_with_e0511() {
    // §22.4.3 rules 3 and 4: two pads alone on one net each drive because the other does not, a loop with no fixed point:
    // the run stops with `E0511` naming both drives by their holders' instance paths
    match Model::operating_point("line.msl", "TwoBidirsBench") {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0511");
            assert!(message.contains("TwoBidirsBench.p") && message.contains("TwoBidirsBench.q"), "{message}");
        }
        other => panic!("E0511, found {other:?}"),
    }
}

impl Levels {
    /// The numbers of every record of the stream `stream` of the run of `top` in `edges.msl`, field `field` only.
    fn edges(top: &str, stream: &str, field: &str) -> Vec<f64> {
        let model = Model::of("edges.msl");
        let built = model.built(top);
        let seen = built.run(10e-9, 1e-9);
        Levels::of(&built, &seen, stream, field)
    }
}

#[test]
fn one_drive_still_goes_through_resolve() {
    // RN-03 AC 7: `resolve` of one drive is still `resolve`, here the sum plus 100
    assert_eq!(Levels::edges("OneDriveBench", "offsets", "value").last(), Some(&105.0), "5 + 100");
}

#[test]
fn a_high_impedance_drive_of_any_level_is_left_out() {
    // §22.3.2 rule 2: a drive at `HighZ` is left out whatever its level, a low, an X or a high alike: the weak high beside
    // them is the net
    assert_eq!(Model::of("line.msl").settled("HighZAnyLevelBench"), Held { level: Held::HIGH, strength: Held::WEAK });
}

#[test]
fn an_undriven_net_of_a_nature_with_two_impls_follows_its_rule() {
    // D21 (spec Q4): a net nobody drives takes the nature's `impl Resolve<V>` when there are several, so `Rule` gives 10 and
    // not the 20 of its `Resolve<u8, f64>`; with none of them `impl Resolve<V>` it is `E0181` naming the carrier
    assert_eq!(Levels::edges("UndrivenRuleBench", "rules", "value").last(), Some(&10.0), "the impl of X = V");
    match Model::of("edges.msl").project.elaborate("UndrivenNeitherBench") {
        Err(rejection) => {
            let text = rejection.to_string();
            assert!(text.contains("E0181") && text.contains("orphan"), "{text}");
        }
        Ok(_) => panic!("E0181 expected"),
    }
}

#[test]
fn a_panic_in_resolve_is_a_run_fault_naming_nature_and_carrier() {
    // D13: a `resolve` that divides by zero is a fault of the run that names the nature and the net it resolves
    match Model::operating_point("edges.msl", "BombBench") {
        Err(failure) => {
            let text = failure.to_string();
            assert!(text.contains("Bomb") && text.contains("fuse_net"), "{text}");
        }
        Ok(_) => panic!("a fault expected"),
    }
}

#[test]
fn the_hosts_drive_of_a_top_port_comes_first() {
    // §4 rule 2, D8: the host holds the first drive of a top's resolved port, absent until it writes: three digits 1, 2 and 3
    // give 123, and after the host drives 9 the net is 9123
    let rows = |drive: Option<f64>| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nets/edges.msl");
        let project = HostProject::file(&path).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some("HostFirstBench".into()), backend: Some("rust".into()), analysis: Some("op".into()), print: vec!["w".into()], ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let mut session = Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}"));
        if let Some(digit) = drive {
            session.drive("w", piperine_solver::prelude::ParamValue::Real(digit), 0.0).unwrap_or_else(|f| panic!("{f}"));
        }
        let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        match table.rows[0].last().expect("a cell") {
            piperine_api::Cell::Real(value) => *value,
            other => panic!("a real, found {other:?}"),
        }
    };
    assert_eq!(rows(None), 123.0, "no host drive: the three digits");
    assert_eq!(rows(Some(9.0)), 9123.0, "the host's drive first");
}

#[test]
fn a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type() {
    // §4 rule 8, RN-02 AC 9: `Bus` has `impl Resolve<bool>` (high when every drive is) and `impl Resolve<u8, bool>` (high when
    // every drive is non-zero); a carrier resolves by the type its drives have. Two flags, one low, are low; two flags high
    // are high; the counts 3 and 0 are low, 3 and 2 are high; and both kinds of net in one design each follow their own `impl`
    let bus = Model::of("bus.msl");
    let last = |top: &str| {
        let built = bus.built(top);
        let seen = built.run(10e-9, 1e-9);
        Levels::of(&built, &seen, "bus", "level").last().copied().unwrap_or_else(|| panic!("{top}: the reader saw no value"))
    };
    assert_eq!(last("OneLowFlagBench"), 0.0, "the AND of the flags true and false");
    assert_eq!(last("HighFlagsBench"), 1.0, "the AND of the flags true and true");
    assert_eq!(last("ZeroCountBench"), 0.0, "counts 3 and 0: not every drive is non-zero");
    assert_eq!(last("NonZeroCountsBench"), 1.0, "counts 3 and 2: every drive is non-zero");
    let built = bus.built("TwoNetsBench");
    let seen = built.run(10e-9, 1e-9);
    let mut by_tag: Vec<(f64, f64)> = Levels::of(&built, &seen, "bus", "tag").into_iter().zip(Levels::of(&built, &seen, "bus", "level")).collect();
    by_tag.sort_by(|left, right| left.0.total_cmp(&right.0));
    assert_eq!(by_tag, vec![(1.0, 1.0), (2.0, 0.0)], "the flags' net (tag 1) is high and the counts' net (tag 2) is low, each by its own impl");
}

#[test]
fn a_typed_bus_resolves_to_free_byte_or_contended() {
    // Appendix F.4.4, §4 rule 8: `DataBus` is `Resolve<Option<u8>, Data>`: nobody drives it, it is `Free`; one talker among
    // modules that release it, it is that `Byte`; two talkers, it is `Contended`. The reader writes -2, the byte and -1
    let bus = Model::of("bus.msl");
    let seen_by = |top: &str| {
        let built = bus.built(top);
        let seen = built.run(10e-9, 1e-9);
        Levels::of(&built, &seen, "data", "seen").last().copied().unwrap_or_else(|| panic!("{top}: the reader saw no value"))
    };
    assert_eq!(seen_by("FreeBusBench"), -2.0, "no drive is Free");
    assert_eq!(seen_by("ReleasedBusBench"), -2.0, "two drives that are only `None`s are Free too");
    assert_eq!(seen_by("ByteBusBench"), 42.0, "one talker is its byte, the released one is left out");
    assert_eq!(seen_by("ContendedBusBench"), -1.0, "two talkers are Contended");
}

#[test]
fn a_parent_drives_a_childs_pub_resolved_signal() {
    // §5.2.2, RN-03 edge case: the parent holds a drive of the child's `pub` resolved signal beside the child's own: 1 + 20 + 100
    assert_eq!(Levels::edges("ParentDrivesBench", "offsets", "value").last(), Some(&121.0), "the child's drive and the parent's");
}

#[test]
fn two_carriers_of_one_instance_on_one_net_hold_two_drives() {
    // §5.2: each written resolved carrier of an instance holds its own drive. `Twin(n, n)` writes 1 on `x` and 2 on `y`, both on
    // the net `n`: its two drives arrive in carrier order, 12; beside a drive of 3 from the instance `d`, whose path orders first, 312
    assert_eq!(Levels::edges("TwinBench", "digits", "value").last(), Some(&12.0), "the twin's two drives");
    assert_eq!(Levels::edges("TwinAndDigitBench", "digits", "value").last(), Some(&312.0), "d first, then the twin's x and y");
}

#[test]
fn a_tran_with_both_sides_on_one_net_settles_on_the_level() {
    // §22.3.7 with `a` and `b` the same net: the two carriers of the `Tran` each drive what the other's drive repeats beside the
    // strong high of `Put`, so the net holds that high
    let line = Model::of("line.msl");
    assert_eq!(line.settled("TranSelfBench"), Held { level: Held::HIGH, strength: Held::STRONG });
}

#[test]
fn a_match_over_options_and_a_resolve_walking_option_drives_run() {
    // digital#20's compiler fix: a tuple pattern whose fields are `Option`s tests each as flag and payload (`(Some(1), Some(2))` is
    // 3, `(None, Some(4))` is 104, `(Some(3), None)` is 3, `(None, None)` is 7), and a closure or a `for` loop over `Option` drives
    // binds the flag and the payload (5, 7 and a `None` add to 12)
    let options = Model::of("options.msl");
    let picks = |field: &str| {
        let built = options.built("PicksBench");
        let seen = built.run(2e-6, 1e-6);
        Levels::of(&built, &seen, "picks", field).last().copied().unwrap_or_else(|| panic!("no pick `{field}`"))
    };
    assert_eq!([picks("both"), picks("second"), picks("first"), picks("none")], [3.0, 104.0, 3.0, 7.0]);
    for top in ["FoldBench", "LoopBench"] {
        let built = options.built(top);
        let seen = built.run(10e-9, 1e-9);
        assert_eq!(Levels::of(&built, &seen, "sums", "v").last(), Some(&12.0), "{top}: 5 + 7, the `None` adds nothing");
    }
}

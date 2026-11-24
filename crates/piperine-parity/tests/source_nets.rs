//! Resolution by strength and the net modules of §22.3 and §22.4.3 run from MSL source on the Rust backend (feature
//! `resolved-nets`, RN-06, RN-07): each model is written in `tests/nets/`, over a nature the model declares
//! (`Line`, `impl Resolve<Driven>`; design D27), elaborated by the front end, compiled on the transpiled backend and run
//! on the solver. Every expected value is written from the rule it cites (§22.3.2's table, §22.3.5 to §22.3.8,
//! §22.4.3), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Streams};
use piperine_parity::project::Project;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, Seen};
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
    fn of(built: &Built, seen: &Seen, name: &str, field: &str) -> Vec<f64> {
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

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> Built {
        build(&self.project, top)
    }

    /// What the stream `line` of `built` says the net held after each change, the first at `(0, 0)`.
    fn held(built: &Built, seen: &Seen) -> Vec<Held> {
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
    assert_eq!(seen_by("ByteBusBench"), 42.0, "one talker is its byte, the released one is left out");
    assert_eq!(seen_by("ContendedBusBench"), -1.0, "two talkers are Contended");
}

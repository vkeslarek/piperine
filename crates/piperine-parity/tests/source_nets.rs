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
    /// `Pull`.
    const PULL: f64 = 2.0;
    /// `Strong`.
    const STRONG: f64 = 3.0;
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

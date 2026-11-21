//! Natures of every kind run from MSL source on the Rust backend (feature `resolved-nets`, RN-01
//! and RN-08): each model is written in `tests/natures/`, elaborated by the front end, compiled on
//! the transpiled backend and run on the solver. Every expected value is written from the rule it
//! cites (§4 rule 12, §5.2.1, §6 rule 2), with its closed form beside the assertion, never read
//! back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Streams};
use piperine_parity::project::Project;
use piperine_solver::protocol::AnalysisKind;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::Built;
use support::{build, transient};

/// One MSL file of `tests/natures/`.
struct Model {
    /// The project read from that file.
    project: Project,
}

impl Model {
    /// The project of `tests/natures/<file>`.
    fn of(file: &str) -> Model {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/natures").join(file);
        Model { project: Project::new(&root) }
    }

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> Built {
        build(&self.project, top)
    }

    /// The value of a known `Byte` as the net holds it: the value byte, then its known byte.
    fn known_byte(value: u8) -> Vec<u8> {
        vec![value, 1]
    }
}

#[test]
fn a_counter_over_a_project_byte_nature_commits_and_records() {
    // §4 rule 12: a carrier of `Byte` is built like `Bit<u8>`. The clock has a period of 10 ns and
    // rises at 5 ns, 15 ns, ...; the counter commits `count + 1` at each rise, so after the `k`-th
    // rise the net holds `k`, known.
    let built = Model::of("byte_counter.msl").built("ConnectedBench");
    let seen = built.run(60e-9, 1e-9);
    let committed: Vec<Vec<u8>> = transient(&seen, "ConnectedBench.count").into_iter().map(|(_, bytes)| bytes).collect();
    let expected: Vec<Vec<u8>> = (1..=6).map(Model::known_byte).collect();
    assert_eq!(committed, expected, "one commit per rising edge");
    // The reader records the count it sees at each rising edge: the value before that edge's
    // commit, so `0, 1, 2, ...` (§5.3, a commit lands after the reactions of its instant).
    let streams = Streams::of(built.ir.clone());
    let seen_counts: Vec<i128> = seen
        .all()
        .iter()
        .filter(|o| o.analysis == AnalysisKind::Transient)
        .filter_map(|o| streams.decode(o))
        .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
        .filter(|r| r.stream == "seen")
        .map(|r| match r.fields.iter().find(|(name, _)| name == "count") {
            Some((_, Field::Int(value))) => *value,
            other => panic!("no integer field `count`: {other:?}"),
        })
        .collect();
    assert_eq!(seen_counts, (0..6).collect::<Vec<i128>>());
}

#[test]
fn a_project_nature_carrier_connects_to_a_port_of_the_same_nature() {
    // §6.1.3, §8.1.10: the counter's `Byte` signal reaches the reader's `Byte` port and a bench
    // with no reader runs as well; the net is one net either way, named by its path.
    let alone = Model::of("byte_counter.msl").built("CounterBench").run(60e-9, 1e-9);
    let connected = Model::of("byte_counter.msl").built("ConnectedBench").run(60e-9, 1e-9);
    assert_eq!(transient(&alone, "CounterBench.count"), transient(&connected, "ConnectedBench.count"));
}

//! Unknown semantics run from MSL source on the Rust backend (feature `unknown-semantics`): one
//! test per run-time criterion, each module written in `tests/unknowns/`, elaborated by the front
//! end, compiled on the transpiled backend and run on the solver. Every expected value is written
//! from the rule it cites (part I §3.3, §1 rule 13, §19), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

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

use support::{build, series};

/// The project of the lone file `file` of `tests/unknowns/`.
fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unknowns").join(file);
    Project::new(&path)
}

/// The last value the net at `path` took in a 10 ns transient of `top` of `file`, `None` if it
/// never took one: a net no one wrote is unknown (every byte 0).
fn settled(file: &str, top: &str, path: &str) -> Option<Vec<u8>> {
    let seen = build(&project(file), top).run(10e-9, 1e-9);
    series(&seen, path).pop().map(|(_, bytes)| bytes)
}

#[test]
fn a_function_and_a_definition_merge_an_unknown_if() {
    // US-04 AC 10, §3.3.3: `d` is never driven. `pick(d)` is `if d { 1 } else { 2 }` in a
    // function: the arms differ, so the merge is unknown (committed as every byte 0, or no
    // value at all); the definition `if d { 3 } else { 3 }` has equal arms, so its merge is 3,
    // known. Neither is a fault.
    let y = settled("pure_merge.msl", "PickBench", "PickBench.y");
    assert!(y.as_ref().is_none_or(|bytes| *bytes == [0, 0]), "different arms merge to unknown: {y:?}");
    assert_eq!(settled("pure_merge.msl", "PickBench", "PickBench.z"), Some(vec![3, 1]), "equal arms merge to that value");
}

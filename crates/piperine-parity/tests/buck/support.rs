//! What the tests of phase 9 of `msl-digital` share: a project read from disk, elaborated from
//! MSL, compiled on the transpiled backend and run on the solver, and the changes of a net read
//! back by its path in ticks of one femtosecond.

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

use crate::mixed::{Built, Seen};

/// A file under `tests/buck/`.
pub fn fixture(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/buck").join(file)
}

/// The root file `root` of `tests/buck/`, with the buck package of `examples/buck` as the
/// dependency crate `buck`.
pub fn with_buck(root: &str) -> Project {
    let buck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl");
    Project::new(&fixture(root)).dependency("buck", &buck)
}

/// `top` of `project`, elaborated and compiled once.
pub fn build(project: &Project, top: &str) -> Built {
    let ir = project.elaborate(top).unwrap_or_else(|r| panic!("{r}"));
    Built::new(ir)
}

/// The changes of the net at `path` during the transient: instant in ticks and bytes, in order.
pub fn transient(seen: &Seen, path: &str) -> Vec<(i64, Vec<u8>)> {
    seen.all()
        .iter()
        .filter(|o| o.path == path && o.analysis == AnalysisKind::Transient)
        .filter_map(|o| match &o.kind {
            ObservationKind::Change { bytes } => Some(((o.time * 1e15).round() as i64, bytes.clone())),
            _ => None,
        })
        .collect()
}

/// A known true and a known false `Bit`: `{ v @0, known @1 }` (abi.md §3.1).
pub const HIGH: [u8; 2] = [1, 1];
pub const LOW: [u8; 2] = [0, 1];

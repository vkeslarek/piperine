//! What the tests of phase 9 of `msl-digital` share: a project read from disk, elaborated from
//! MSL, compiled on the transpiled backend and run on the solver, and the changes of a net read
//! back by its path in ticks of one femtosecond.

use std::path::{Path, PathBuf};

use crate::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

use crate::mixed::{BuiltDesign, Seen};

/// The shared steps of the buck and source tests: a project read, built and run, and the changes of a net read back.
pub struct BuckFixture;

impl BuckFixture {
    /// A file under `tests/buck/`.
    pub fn fixture(file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/buck").join(file)
    }

    /// The root file `root` of `tests/buck/`, with the buck package of `examples/buck` as the
    /// dependency crate `buck`.
    pub fn with_buck(root: &str) -> Project {
        let buck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl");
        Project::new(&BuckFixture::fixture(root)).dependency("buck", &buck)
    }

    /// `top` of `project`, elaborated and compiled once.
    pub fn build(project: &Project, top: &str) -> BuiltDesign {
        let ir = project.elaborate(top).unwrap_or_else(|r| panic!("{r}"));
        BuiltDesign::new(ir)
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

    /// Every change of the net at `path`, the operating point's initial value first: instant in ticks
    /// and bytes. The value at an instant is the last entry at or before it.
    pub fn series(seen: &Seen, path: &str) -> Vec<(i64, Vec<u8>)> {
        seen.series_of(path)
    }

    /// The value of a series after the instant `at`: the last change at or before it.
    pub fn value_at(series: &[(i64, Vec<u8>)], at: i64) -> &[u8] {
        &series.iter().rev().find(|(t, _)| *t <= at).expect("an initial value").1
    }

    /// A little-endian integer of `bytes` bytes, sign-extended when `signed`.
    pub fn decode(raw: &[u8], bytes: usize, signed: bool) -> i32 {
        let mut buf = [0u8; 4];
        buf[..bytes].copy_from_slice(&raw[..bytes]);
        let v = u32::from_le_bytes(buf);
        if signed && bytes < 4 && v & (1 << (bytes * 8 - 1)) != 0 { (v | (u32::MAX << (bytes * 8))) as i32 } else { v as i32 }
    }

    /// The value of the net at `path` once the first rising edge (2.5 ns) has committed: the one
    /// change the transient made at it, or, when the commit wrote the value the net started at, the
    /// value of the operating point.
    pub fn first_edge_value(seen: &Seen, path: &str) -> Vec<u8> {
        let changes = BuckFixture::transient(seen, path);
        match changes.as_slice() {
            [(at, bytes)] => {
                assert_eq!(*at, 5 * 500_000, "{path} changes at the first rising edge");
                bytes.clone()
            }
            [] => seen
                .all()
                .into_iter()
                .filter(|o| o.path == path && o.analysis != AnalysisKind::Transient)
                .find_map(|o| match o.kind {
                    ObservationKind::Change { bytes } => Some(bytes),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{path} keeps no value from the operating point")),
            _ => panic!("{path} changes once at most: {changes:?}"),
        }
    }
}

/// A true and a false `Bit`: `{ v @0 }` (abi.md §3.1).
pub const HIGH: [u8; 1] = [1];
pub const LOW: [u8; 1] = [0];


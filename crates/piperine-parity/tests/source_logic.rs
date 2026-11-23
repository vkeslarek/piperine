//! The four-state `Logic<N>` and the operator traits it is built on, run from MSL source on the Rust backend
//! (feature `explicit-unknowns` phase B, EU-07 to EU-10): one test per run-time criterion, each model written in
//! `tests/logic/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is the specification's own table (part VII §22.1) written in the test, never read back from the
//! model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

use run::SourceRun;

/// The model files of `tests/logic/`.
const RUN: SourceRun = SourceRun::in_directory("tests/logic");

#[test]
fn an_operator_on_a_library_type_runs_its_impl() {
    // H.10 rules 1 and 4 (EU-08): `a & b`, `a & 6u4`, `!a` and `a &= b` on `Quad` call the functions of the `impl`s the
    // model writes. `Quad` is `binary` then `value`, one byte each. `both` is `{3 | 4, 12 & 10}`, `masked` is
    // `{3, 12 & 6}`, `inverted` is `{3, !12 & 15}`, `compound` is `{3 | 8, 12 & 14}`
    RUN.assert_commits("quad.msl", &["m0", "m1", "m2", "m3"], &[vec![7, 8], vec![3, 4], vec![3, 3], vec![11, 12]]);
}

#[test]
fn logic_lays_out_two_planes() {
    // §22.1.1 rule 1 (EU-07 AC 1, 2): `Logic<4>` is `binary` then `value`, one byte each. `Logic::from(0b1010u4)` is
    // all-binary (`binary` 1111) over `value` 1010; the literal `{ binary: 3, value: 5 }` is itself
    RUN.assert_commits("planes.msl", &["m0", "m1"], &[vec![15, 10], vec![3, 5]]);
}

#[test]
fn at_gives_the_state_of_one_level() {
    // §22.1.1 rule 3 (EU-07 AC 4): `{ binary: 0b0011, value: 0b0101 }` has level 0 `1`, level 1 `0`, level 2 `Z`, level 3 `X`;
    // `LogicState` is `X`, `L`, `H`, `Z` in tag order 0 to 3
    RUN.assert_commits("planes.msl", &["s0", "s1", "s2", "s3"], &[vec![2], vec![1], vec![3], vec![0]]);
}

#[test]
fn logic_reads_a_four_state_pattern() {
    // §22.1.4 rules 2 and 3 (EU-07 AC 3, C25): `logic("10xz")` has levels 3 to 0 equal to 1, 0, X, Z: `binary` 1100 and
    // `value` 1001 (X is `(0, 0)`, Z is `(0, 1)`). `logic("10xz") & Logic::from(0b1111u4)` is the specification's `10xx`
    // (a Z level under `&` with a 1 is X): `binary` 1100, `value` 1000
    RUN.assert_commits("patterns.msl", &["m0", "m1"], &[vec![0b1100, 0b1001], vec![0b1100, 0b1000]]);
}

#[test]
fn logic_and_matches_bits_give_the_table_values() {
    // §22.1.4 (EU-07 AC 3): `matches_bits(s, p)` is true when each level of `s` is the state its character names, `x`
    // naming any: `10xz` against `10xz` and `1xxz` and `xxxx` holds, against `0xxz` (level 3 is 1) and `10x0` (level 0 is Z)
    // does not
    RUN.assert_commits("patterns.msl", &["t0", "t1", "t2", "t3", "t4"], &[vec![1], vec![1], vec![0], vec![0], vec![1]]);
}

# Retire the VAMS Frontend verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 763c4a24
**Verifier**: independent sub-agent (author != verifier)

The deletion itself is correct: `crates/piperine-lang` holds only `Cargo.toml`, `src/lib.rs`, `src/msl/**`, `tests/msl_*.rs` and `tests/support/spec_corpus.rs`; `git diff --diff-filter=AMR 2a5db571 763c4a24 -- crates/piperine-lang` touches only `Cargo.toml` and `src/lib.rs`, so every `src/msl` file and every `msl_*` test file is byte-identical to the parent branch tip. The 49 tests run and pass (2 + 21 + 9 + 10 + 7, matching the 49 `#[test]` attributes), the build is warning-free, and all three remaining dependencies (`miette` incl. its `fancy` feature via `GraphicalReportHandler`, `chumsky` incl. `pratt`/`stacker`, `logos`) are used in real code. The FAIL is for three loose proofs and one stale doc line, all minor.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `test ! -e` loop over the 20 paths AC 1 enumerates | yes - matches the criterion's own list; the tree (`find crates/piperine-lang -type f`) confirms nothing else VAMS-shaped remains |
| C2 | `ls tests/*.rs &#124; grep -v '^msl_'` empty | yes - the only non-`.rs` content left is `tests/support/`, which AC 2 allows |
| C3 | `cargo build -p piperine-lang --all-targets` grepped for `^(warning&#124;error)` | yes - cargo replays cached warnings, so an incremental build would still print them |
| C4 | `cargo test` on the 5 `msl_*` targets, sum of `N passed` `-ge 49` | no - ignores failures and the bound is `-ge`, not `==`: output `48 passed` + `1 passed` alongside `1 failed` sums to 49 and passes (simulated: `2 passed; 0 failed` + `48 passed; 1 failed` gives 50). Holds today only because the test files are byte-identical to 2a5db571 and the real count is exactly 49 |
| C5 | grep for `\bNAME::&#124;use NAME\b&#124;extern crate NAME` per `[dependencies]` entry | no - a mention in a comment satisfies it (scratch copy: adding `serde = "1"` fails the proof, but adding the comment `// serde::Serialize is not used here` makes it pass); it also never checks enabled features or `[dev-dependencies]`. Manually confirmed each of the three deps is used in non-comment code |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| C4's proof passes with a failing test as long as 49 others pass | proof sums `N passed` only and uses `-ge 49`; spec AC 4 says all pass with the same count | assert `== 49` and that no `test result` line reports `FAILED` (or that `cargo test` itself exits 0) | dd968d4a |
| C5's proof is satisfied by a dependency named only in a comment | scratch copy at 763c4a24 + `serde = "1"` + `// serde::Serialize ...` in `src/msl/span.rs` passes the proof | strip `//` comment lines before grepping, or use `cargo machete`/`cargo udeps` if available | dd968d4a |
| Stale doc line refers to VAMS modules that no longer exist | `crates/piperine-lang/src/msl/mod.rs:5-6` - "It shares nothing with the Verilog-AMS modules of this crate." | drop the sentence (lib.rs already records the retirement) | b100d9f1 |

Note (not a finding): `cargo doc -p piperine-lang --no-deps` prints 6 rustdoc warnings (private intra-doc links to `Handles`, `stmt`, `word`, `super::item::build`; unresolved `ast::File`). They live in `src/msl`, which this diff did not touch, so they predate the feature.

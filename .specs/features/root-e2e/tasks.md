# Root E2E Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/root-e2e/design.md`
**Status**: Approved
**Approved**: 2026-11-18 - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine -p piperine-parity -p piperine-cli -p piperine-api --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape`
**Run inventory**: `cargo test --test book_inventory`
**Run claims**: `cargo test --test book_claims`
**Run errors**: `cargo test --test book_errors`
**Run welcome**: `cargo test --test book_welcome`
**Run scenarios**: `env -u PIPERINE_NGSPICE cargo test --test scenarios -- --nocapture`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run corpus**: `cargo test -p piperine-parity --test parity_corpus`
**Run paritylib**: `cargo test -p piperine-parity --lib`
**Run hygiene**: `cargo test -p piperine-lang --test suite_hygiene`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`

Executors: every task is Sonnet (user rule). A different model verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch src/lib.rs` (and `crates/<crate>/src/lib.rs` of a crate the task edits) before a gate so cargo rebuilds this tree's sources.

**Starts after `host-ii` merges.** The branch is rebased onto `main` holding `host-ii` (queue item 2) before T1; T1 stops when `tests/ngspice/` does not hold the sixteen twins (the nine of `host-ii` and `solver-hygiene` included) or the Book does not hold `host-ii`'s listings.

**Questions answered** (user, 2026-11-17: "Aceito todas (Recommended)"): a claim is every block, listing and stated value or outcome (Q1 (a)); the tests live in root `tests/book/`, one target per chapter (Q2 (a)); `tests/ngspice/` moves once to `tests/e2e/<scenario>/`, no link (Q3 (a)).

Rules every task follows:

- **Parity is top priority and untouched.** `crates/piperine-parity/src/contract.rs` is not edited; no tolerance is retuned or written outside it; the `parity_*` comparison targets are not edited (only `parity_grid.rs`'s path literal and the guard `parity_corpus.rs` change, in T3 to T5). A twin's `.cir` and `.stdout` move byte for byte. A miss is reported to the orchestrator, never a moved bound.
- **The code follows the spec** (user, 2026-11-18: "O código que diverge da spec tem que ser ajustado"). Where the binary or the library disagrees with the Book or the spec, the code is fixed; the Book does not move. A divergence has one of three causes (design "Divergences"): (1) an implemented construct gives another output, number, message, code, name or exit code: a `diverges` row while the chapters are written, fixed by Phase 9 and then `holds`; (2) a construct not implemented, refused by name, owned by an approved feature: a `refused` row naming its owner in §11, not a divergence; (3) an error code the spec names that no source raises: an `unraised` row naming `params-and-io` (PI-13) or the feature of `GAPS.md` §10, listed, not fixed here. A sentence where the spec itself looks wrong is `disputed`, listed in `divergences.md` for the user; no task decides it. The Book changes only by the user's answer to a `disputed` row.
- **Tests state what the Book says, not what the code does.** A `holds` test asserts the value the sentence states (at the precision it prints, through `PrintedNumber` where it prints a number); a `refused`, `unraised`, `disputed` or (until Phase 9 fixes it) `diverges` test asserts today's exact code and message and fails when that changes; Phase 9 rewrites each `diverges` test to assert the Book's value, the only planned edit of a test's expectation in this feature. Every test opens with a comment citing its inventory ids and, for a claim, the sentence. Test functions have exactly the names `## Checks` cites.
- **Existing tests that change, and only these:** `piperine-cli/tests/twins.rs` (T2: sixteen twins; T3: paths; T5: commands read from the scenario), `parity_corpus.rs` (T3 paths, T4 and T5 the e2e layout), `workspace_shape.rs::root_tests_hold_no_end_to_end_target` (T3 the `.cir` count, T7 the declared root targets), the path literals of the readers T3 lists, `suite_hygiene.rs`'s walker and `ROOT_TARGET_EXEMPTIONS` (T11, T12, T14), and an existing test that asserted a divergent output a Phase 9 fix corrects (named in the fix's commit body, its new expectation cited from the Book or the spec). No test is removed, weakened or skipped, but the `SETTLED_CHAPTERS` registry tests T41 replaces with stricter ones (MD-28 rule 3).
- **The user's cleanliness rules bind test support code** (`.specs/features/code-clarity/spec.md`): no free function in new code (every function a method of a type), one purpose per type, no abbreviated names, no god method, every type and method doc-commented, the data path evident. AGENTS rules: no macro of your own, no `unwrap()`/`expect()` on a path user input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- **A fix in another lane's area** (a crate an in-flight feature is editing) is reported to the orchestrator before it is made; a fix never moves a tolerance, a golden or a twin's bytes; a fix that would is reported.
- Zero warnings on every crate of the `**Build**` line. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`. ngspice runs for real: `env -u PIPERINE_NGSPICE` on every parity and scenario gate.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- **`lessons.py` is not run**: its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-28, MD-31, MD-38, MD-65.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Parity harness library (`piperine-parity/src/corpus.rs`) | unit | all branches: found, missing, two of one name | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-parity --lib` |
| The e2e layout guard | integration | every rule on the tree and on planted text (L-013) | `crates/piperine-parity/tests/parity_corpus.rs` | `cargo test -p piperine-parity --test parity_corpus` |
| Twins on the binary | e2e | every twin, both backends, bytes after the header | `crates/piperine-cli/tests/twins.rs` | `cargo test -p piperine-cli --test twins` |
| Root scenarios | integration | every twin through `piperine::` against ngspice under the contract; skip; own directory | `tests/scenarios.rs` | `env -u PIPERINE_NGSPICE cargo test --test scenarios` |
| Root Book guards | integration | every rule of `design.md` on the tree and on planted input | `tests/book/{inventory,claims}_guard.rs` | `cargo test --test book_inventory --test book_claims` |
| Root Book chapters | integration | every `run`, `refused`, `holds`, `diverges`, `unraised`, `disputed` row of the chapter, through `piperine::` | `tests/book/<chapter>.rs` | `cargo test --test book_<chapter>` |
| Code fixes (Phase 9, the crate each fix names) | unit or integration, by the crate's matrix row of earlier features | the fix's own test in its crate, plus the Book test flipped to the Book's value | the crate's `tests/` or inline | the crate's target and `cargo test --test book_<chapter>` |
| Binary-only claims | e2e | each `binary` row with no test today gets one, on the real binary | `crates/piperine-cli/tests/book.rs` | `cargo test -p piperine-cli --test book` |
| Suite hygiene walker | integration | the manifest predicate on planted text; the real tree | `crates/piperine-lang/tests/suite_hygiene.rs` | `cargo test -p piperine-lang --test suite_hygiene` |
| `GAPS.md`, `ROADMAP.md`, `STATE.md`, `CLAUDE.md`, `AGENTS.md` | integration | each record claim read as text | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| `contract.md`, comments and doc strings | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with one target or unit tests | the `Run` line(s) or `cargo test --test <target>` the task's `Done when` names |
| Full | After a task that moves files or changes what a twin runs | Quick, then `env -u PIPERINE_NGSPICE cargo test` at the repository root (the default members) |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `env -u PIPERINE_NGSPICE cargo test` at the repository root |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

**Batches** (~7 tasks, whole phases): B1 = Phase 1 (T1-T6), B2 = Phases 2-3 (T7-T13), B3 = Phases 4-5 (T14-T21), B4 = Phases 6-7 (T22-T28), B5 = Phase 8 (T29-T33), B6 = Phase 9 (T34-T40), B7 = Phase 10 (T41-T43).

### Phase 1: The move

The tree is read first, the nine twins with no golden get one before anything moves, then one move with every reader.

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Scenarios through `piperine::`

```
T6 → T7 → T8 → T9 → T10
```

### Phase 3: The block inventory

```
T10 → T11 → T12 → T13
```

### Phase 4: Claims and refusals

```
T13 → T14 → T15 → T16 → T17
```

### Phase 5: Language chapters I

```
T17 → T18 → T19 → T20 → T21
```

### Phase 6: Language chapters II

```
T21 → T22 → T23 → T24 → T25
```

### Phase 7: Packages and the library

```
T25 → T26 → T27 → T28
```

### Phase 8: Reference chapters

```
T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 9: Code follows the spec

The triage first, then one fix task per area; each fix flips its Book tests to the Book's value.

```
T33 → T34 → T35 → T36 → T37 → T38 → T39 → T40
```

### Phase 10: Closure

```
T40 → T41 → T42 → T43
```

---

## Task Breakdown

### T1: Contract: the tree against the design

**What**: Read every location `design.md` cites at `HEAD` (after the rebase onto `main` with `host-ii`) and write `contract.md`: each citation `confirmed` or `moved: <where>`; the nine twins with no golden (`host-ii`'s five, `solver-hygiene`'s four) and the command-line flags of their analyses (from `crates/piperine-cli/tests/analyses.rs` and the Book); whether `piperine::Settings` exists; what `host-ii` did to the root `Cargo.toml`, `tests/host_ii.rs`, `workspace_shape.rs` and `suite_hygiene.rs`; the Book's counts (chapters, blocks by language, listings) and the planner's sentence count (design "Data Models"), recounted; every file that reads `tests/ngspice` (`grep -rn "tests/ngspice\|\"ngspice\"" crates src tests`). A citation that changes a task is reported to the orchestrator before T2.
**Where**: `.specs/features/root-e2e/contract.md`
**Depends on**: None
**Reuses**: `.specs/features/host-ii/contract.md` (the form)
**Requirement**: RE-01, RE-04
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists every citation of `design.md` with `confirmed` or `moved: <where>`, and the readers of `tests/ngspice` one per line
- [x] `Run twins`, `Run corpus` and every `parity_*` target pass at `HEAD` before any change, their test counts written in `contract.md`
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): root-e2e - contract of the tree against the design`

---

### T2: The goldens of the nine twins with none

**What**: `twins.rs` covers all sixteen twins: the nine twins with no golden (`host-ii`'s five, `solver-hygiene`'s four) join `TWINS` with the flags their `.cir` asks for (from T1), and their standard output after the header line, captured from the binary at this commit on each backend, is written to `crates/piperine-cli/tests/goldens/<name>.txt`. A new test holds every twin to its golden, so the move (T3) is proven against bytes captured before it. A twin a backend refuses is reported to the orchestrator, not dropped.
**Where**: `crates/piperine-cli/tests/twins.rs`
**Depends on**: T1
**Reuses**: `every_twin_prints_the_bytes_it_printed_before_the_long_run` (the form)
**Requirement**: RE-04
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-cli/tests/goldens/` holds `tf_divider.txt`, `pz_lowpass.txt`, `sens_divider.txt`, `pss_rc.txt`, `temp_res.txt`, each the binary's output
- [x] `twins.rs` gains `every_twin_prints_the_bytes_it_printed_before_the_move` (sixteen twins, every backend that runs each); the existing tests pass with sixteen twins
- [x] Gate check passes: Quick (`Run twins`)
- [x] Test count: 1 new test

**Tests**: e2e
**Gate**: quick

**Commit**: `test(cli): the sixteen twins' bytes before the move`

---

### T3: One move to `tests/e2e/twins/`, every reader with it

**What**: `git mv` every file of `tests/ngspice/` to `tests/e2e/twins/` and every `crates/piperine-cli/tests/goldens/<name>.txt` to `tests/e2e/twins/<name>.stdout`, unchanged. `piperine-parity` gains `Corpus` and `Twin` (`src/corpus.rs`, design "Components"), and `Ngspice::fixture_path` and `Bench::twin_path` read through it. Every reader T1 listed reads the new path in this task: `piperine-parity` (`tests/{parity_grid,parity_corpus,twins}.rs`), `piperine-cli` (`tests/{twins,options,run}.rs`), `piperine-api` (`tests/{analyses,grid,probes,records}.rs`), `piperine-lang` (`tests/{elab_entry,types_entry,elab_digital,msl_format}.rs`), `piperine-solver` (`tests/workspace_shape.rs`: the `.cir` count), and any other T1 found. No symlink.
**Where**: `tests/e2e/twins/`
**Depends on**: T2
**Reuses**: `Ngspice::fixture_path`, `Bench::twin_path`
**Requirement**: RE-04
**Executor**: Sonnet

**Done when**:

- [x] `tests/ngspice/` and `crates/piperine-cli/tests/goldens/` do not exist; `git show --stat -M HEAD` shows every moved file as a rename with no line changed
- [x] `corpus.rs` has inline tests `a_twin_is_found_by_name`, `a_missing_twin_names_itself`, `a_name_in_two_scenarios_is_refused`
- [x] Every target T1 listed passes with its test count unchanged; `every_twin_prints_the_bytes_it_printed_before_the_move` passes
- [x] Gate check passes: Full (`Run paritylib`, `Run twins`, `Run corpus`, then the root `cargo test`)
- [x] Test count: 3 new unit tests, none removed

**Tests**: unit
**Gate**: full

**Commit**: `refactor(tests): the ngspice twins move to tests/e2e/twins`

---

### T4: The e2e layout guard

**What**: `parity_corpus.rs` guards the e2e tree instead of `tests/ngspice/`: every twin has its `.cir`, its `.stdout` and a test function in a `parity_*` target; an `.msl` with no `.cir` is a library a twin of its directory loads with `mod`; a scenario directory holding `benches/` holds `Piperine.toml`; `tests/ngspice/` does not exist. Each rule is a method of `Guard` tested on planted text; the `.cir` byte pins stay.
**Where**: `crates/piperine-parity/tests/parity_corpus.rs`
**Depends on**: T3
**Reuses**: `Guard::unmatched_twins`, `the_guards_report_planted_text`
**Requirement**: RE-04, RE-05
**Executor**: Sonnet

**Done when**:

- [x] `parity_corpus.rs` holds `every_twin_has_its_files_in_one_scenario_directory`, `tests_ngspice_holds_no_twin`, `a_scenario_with_benches_holds_a_manifest` and `the_layout_guards_report_planted_trees`
- [x] Each new rule fails on a planted tree that breaks it (asserted in `the_layout_guards_report_planted_trees`)
- [x] Gate check passes: Quick (`Run corpus`)
- [x] Test count: 4 new tests; the existing tests pass on the new tree

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the corpus guard holds the e2e tree`

---

### T5: The command of each scenario

**What**: Each twin gains `tests/e2e/twins/<name>.command`, one line, the command a user types from the scenario directory (`piperine run divider.msl --analysis op`), with the flags `TWINS` holds today. `twins.rs` reads the commands through `Corpus` and runs each from its scenario directory with `--backend` appended; the `TWINS` table goes. `parity_corpus.rs`'s file rule adds `.command`.
**Where**: `crates/piperine-cli/tests/twins.rs`
**Depends on**: T4
**Reuses**: `Corpus::twins`, `Twin::command`
**Requirement**: RE-05
**Executor**: Sonnet

**Done when**:

- [x] Sixteen `.command` files exist; `twins.rs` holds no flag of a twin
- [x] `every_twin_prints_the_bytes_it_printed_before_the_move` and every other test of `twins.rs` pass unchanged in what they assert
- [x] Gate check passes: Quick (`Run twins`, `Run corpus`)
- [x] Test count: unchanged

**Tests**: e2e
**Gate**: quick

**Commit**: `test(cli): each twin's command lives in its scenario`

---

### T6: The old path in comments and doc strings

**What**: Every comment or doc string that names `tests/ngspice` now names `tests/e2e/twins`: the first line of each twin (`//! Twin of ...`), `piperine-parity/src/lib.rs`, `piperine-ir` (`src/lib.rs`, `tests/support/devices.rs`), `piperine-transpile` (`src/print/text.rs` test strings, `tests/differential.rs`), `piperine-parity/tests/buck/divider/devices/lib.msl`, the `//!` headers of the readers T3 changed. No `.cir` changes. A pinned text that changes (an elaboration, a golden) stops the task and is reported.
**Where**: `tests/e2e/twins/`
**Depends on**: T5
**Reuses**: none
**Requirement**: RE-04
**Executor**: Sonnet

**Done when**:

- [x] `grep -rn "tests/ngspice" crates src tests` prints nothing
- [x] `Run twins`, `Run corpus`, the `parity_*` targets and `cargo test -p piperine-lang --test elab_entry --test types_entry --test elab_digital` pass unchanged
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs: the twins' home is tests/e2e/twins`

---

### T7: The root runs the operating-point scenarios

**What**: The root `Cargo.toml` gains the dev-dependencies of design "Integration Points"; `tests/scenarios.rs` (new, `//!` header) holds `Scenario`, `Ran` and `Comparison` (design "Components") and one test per operating-point twin: `divider`, `isrc_r`, `diode_series`, each on every selected backend through `piperine::`, compared with ngspice's printed values under `DC_VOLTAGE`. `workspace_shape.rs::root_tests_hold_no_end_to_end_target` admits Rust under root `tests/` only in the targets the root manifest declares (top-level `tests/*.rs` and `[[test]]` paths) and in `tests/book/support/`.
**Where**: `tests/scenarios.rs`
**Depends on**: T6
**Reuses**: `parity_op.rs` (nodes and contract), `piperine-api/tests/analyses.rs` (`Settings`, the session), `Cli::parse`
**Requirement**: RE-05
**Executor**: Sonnet

**Done when**:

- [x] `tests/scenarios.rs` holds `divider`, `isrc_r`, `diode_series`
- [x] `cargo test -p piperine-solver --test workspace_shape` and `Run hygiene` pass
- [x] Gate check passes: Quick (`Run scenarios`)
- [x] Test count: 3 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: the root runs the operating-point scenarios against ngspice`

---

### T8: The sweep, AC and transient scenarios

**What**: `scenarios.rs` gains `diode_iv` and `diode_rev` (each row of ngspice's export under `DC_CURRENT`), `ac_rc` (`mag`/`phase` columns under `AC_MAGNITUDE` and `AC_PHASE`), `tran_rc` and `tran_dq` (the print grid of the export, then each row under `TRAN_VOLTAGE`) and the three step-floor twins `tran_jump`, `tran_corner` (the grid of `FLOOR_ROWS`, `TRAN_VOLTAGE`) and `tran_floor` (the run fails `E0508` near 1 us, ngspice stops with "Timestep too small"; Rust backend only), the quantities of `parity_{dc,ac,tran}.rs`.
**Where**: `tests/scenarios.rs`
**Depends on**: T7
**Reuses**: `parity_dc.rs`, `parity_ac.rs`, `parity_tran.rs`, `Golden::same_grid`
**Requirement**: RE-05
**Executor**: Sonnet

**Done when**:

- [x] `scenarios.rs` holds `diode_iv`, `diode_rev`, `ac_rc`, `tran_rc`, `tran_dq`, `tran_jump`, `tran_corner`, `tran_floor`
- [x] Gate check passes: Quick (`Run scenarios`)
- [x] Test count: 8 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: the root runs the sweep, AC and transient scenarios`

---

### T9: The `host-ii` scenarios

**What**: `scenarios.rs` gains `tf_divider` (`TF_RATIO`), `pz_lowpass` (`PZ_ROOT`), `sens_divider` (`SENSITIVITY`), `pss_rc` (`TRAN_VOLTAGE`) and `temp_res` (`DC_VOLTAGE`), each comparing what its `parity_*` target compares, from the table `piperine::` gives for its command.
**Where**: `tests/scenarios.rs`
**Depends on**: T8
**Reuses**: `parity_{tf,pz,sens,pss,temp}.rs`, `Table::transfer`/`roots`/`sensitivities` (`host-ii`)
**Requirement**: RE-05
**Executor**: Sonnet

**Done when**:

- [x] `scenarios.rs` holds `tf_divider`, `pz_lowpass`, `sens_divider`, `pss_rc`, `temp_res`
- [x] Gate check passes: Quick (`Run scenarios`)
- [x] Test count: 5 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: the root runs the host II scenarios`

---

### T10: Every scenario, skipped or not, in its own directory

**What**: `scenarios.rs` gains its guards: every twin of `Corpus::twins()` has a test named after it in this file; with ngspice disabled (`Ngspice::new(Lookup::Skip)`) a scenario still runs, gives a table and prints the harness's skip line; two scenarios run in two different build directories, each holding only its own build.
**Where**: `tests/scenarios.rs`
**Depends on**: T9
**Reuses**: `Ngspice::skip_line`, `ScratchDir`
**Requirement**: RE-05
**Executor**: Sonnet

**Done when**:

- [x] `scenarios.rs` holds `every_twin_has_a_scenario_test`, `a_disabled_ngspice_still_runs_the_scenario`, `each_scenario_runs_in_a_directory_of_its_own`
- [x] `Run scenarios` prints `SKIP divider: ngspice comparison disabled by PIPERINE_NGSPICE=skip`
- [x] Gate check passes: Build
- [x] Test count: 3 new tests

**Tests**: integration
**Gate**: build

**Commit**: `test: every scenario has its test, its own directory and a skip`

---

### T11: The hygiene walker reads the root manifest

**What**: `support/sources.rs` counts a root `tests/<dir>/<file>.rs` as an integration target when the root `Cargo.toml` names its path in a `[[test]]` table, and a helper otherwise. The manifest reading is a named type (`RootManifest::test_paths(text)`) tested on planted text.
**Where**: `crates/piperine-lang/tests/support/sources.rs`
**Depends on**: T10
**Reuses**: `integration_targets`, `root_targets`
**Requirement**: RE-01
**Executor**: Sonnet

**Done when**:

- [x] `suite_hygiene.rs` gains `the_root_manifest_names_its_test_targets` (planted manifest: two `[[test]]` paths found, a `[dependencies]` path ignored)
- [x] Gate check passes: Quick (`Run hygiene`)
- [x] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): the hygiene walker reads the root test targets`

---

### T12: The Book reader and its guard target

**What**: `tests/book/support/book.rs` holds `Book`, `BlockId`, `Block`, `Sentence` and `Fingerprint` (design "Components"); `tests/book/inventory_guard.rs` (new, `//!` header, target `book_inventory` in the root manifest) reads the Book with it. `suite_hygiene.rs` registers the target in `ROOT_TARGET_EXEMPTIONS` (it links no workspace crate: a repository policy proof).
**Where**: `tests/book/support/book.rs`
**Depends on**: T11
**Reuses**: `piperine-cli/tests/book.rs` (`blocks`, `files_before`, `attributes`, `decode`: the rules, not the code)
**Requirement**: RE-01
**Executor**: Sonnet

**Done when**:

- [x] `inventory_guard.rs` holds `the_book_reads_every_block` (every chapter of T1's count, each block with an id, the `msl` count T1 recorded) and `a_marked_file_joins_its_blocks` (`clipper.msl` is the two marked blocks joined)
- [x] `Run hygiene` passes with the exemption
- [x] Gate check passes: Quick (`Run inventory`)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: the root reads the Book`

---

### T13: The block rows and their guard

**What**: `tests/book/inventory.tsv` holds one `block` row per `<pre>` block of the Book, verdict `pending` (listings `held` by `piperine-cli/book::every_cli_listing_of_the_book_is_what_the_binary_prints`; `sh`, `toml`, other `text` and `rust` blocks `shown`), each with its fingerprint; `tests/book/support/inventory.rs` holds `Inventory`, `Row`, `Verdict`, `Coverage`. The guard gains the block rules of design "The two guards" and the `SETTLED_CHAPTERS` registry (empty). A failure prints the corrected rows.
**Where**: `tests/book/inventory_guard.rs`
**Depends on**: T12
**Reuses**: `Book`, `Fingerprint`
**Requirement**: RE-01, RE-02
**Executor**: Sonnet

**Done when**:

- [x] `inventory_guard.rs` holds `every_block_of_the_book_has_one_row`, `every_row_names_an_existing_test`, `a_verdict_fits_its_block`, `run_rows_name_tests_that_run_their_model`, `refused_rows_name_tests_that_expect_their_refusal`, `a_model_across_blocks_names_one_test`, `settled_chapters_have_no_pending_row`, `the_guard_names_a_block_without_a_row_and_a_row_naming_no_test`
- [x] The last test feeds planted inventories breaking each rule and asserts each failure names the block
- [x] Gate check passes: Quick (`Run inventory`, `Run clibook`)
- [x] Test count: 8 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: every Book block has an inventory row`

---

### T14: The claim rows and their guard

**What**: `tests/book/claims_guard.rs` (new, `//!` header, target `book_claims`, registered in `ROOT_TARGET_EXEMPTIONS`) holds `ClaimDetector` (the code, number-with-unit and outcome-verb lists as data) and the claim rules of design "The two guards"; `inventory.tsv` gains one `claim` row per flagged sentence, verdict `pending`.
**Where**: `tests/book/claims_guard.rs`
**Depends on**: T13
**Reuses**: `Book::sentences`, `Inventory`, `Coverage`
**Requirement**: RE-03
**Executor**: Sonnet

**Done when**:

- [x] `claims_guard.rs` holds `every_value_sentence_has_a_claim_row`, `every_claim_row_is_a_sentence_of_its_chapter`, `holding_claims_name_root_tests_that_cite_them`, `binary_claims_name_command_line_tests`, `settled_chapters_have_no_pending_claim`, `the_detector_flags_a_value_a_code_and_an_outcome`, `the_claim_guard_names_a_sentence_without_a_row`
- [x] The detector test asserts flags on planted sentences (a code, `693.1 ns`, `prints`) and no flag on a planted rule sentence
- [x] Gate check passes: Quick (`Run claims`, `Run hygiene`)
- [x] Test count: 7 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: every value sentence of the Book has a claim row`

---

### T15: Running a Book model, and the refusal that runs

**What**: `tests/book/support/{model,records,refusal}.rs` hold `Model`, `Ran`, `KeptRecords` and `Refusal` (design "Components"); `tests/book/errors.rs` (new, `//!` header, target `book_errors`) proves `Refusal::expect` fails naming its row when the model runs, and settles the errors chapter's blocks: the worked `SampleHold` (`refused` with the codes the binary gives today) and the two `diag` blocks (`holds` or `diverges`). The chapter joins neither `SETTLED_CHAPTERS` yet (its code tables are T17).
**Where**: `tests/book/errors.rs`
**Depends on**: T14
**Reuses**: `piperine-api/tests/analyses.rs` (session and settings), `Inventory::checked_block`
**Requirement**: RE-02
**Executor**: Sonnet

**Done when**:

- [x] `errors.rs` holds `a_refusal_that_runs_fails_naming_its_row` (a model that runs, under `catch_unwind`: the panic names the row and says "now runs") and one test per settled block row
- [x] Gate check passes: Quick (`Run errors`, `Run inventory`)
- [x] Test count: at least 3 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test: a Book model runs or is refused through piperine::`

---

### T16: `GAPS.md` §11, held to the inventory

**What**: `GAPS.md` gains "## 11. The Book against the binary" (design "Data Models"), with the rows T15 made `refused` or `diverges`; `inventory_guard.rs` holds the inventory's `refused` and `diverges` rows (blocks and claims) equal to §11's, each under its gap id, the id an entry of `GAPS.md`.
**Where**: `GAPS.md`
**Depends on**: T15
**Reuses**: `Inventory`
**Requirement**: RE-02
**Executor**: Sonnet

**Done when**:

- [x] `inventory_guard.rs` holds `refused_rows_and_gaps_section_11_agree` and `the_gaps_guard_names_a_missing_and_an_extra_row` (planted)
- [x] `Run docs` passes (no pinned `GAPS.md` text moved)
- [x] Gate check passes: Quick (`Run inventory`, `Run docs`)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `docs(gaps): the Book against the binary, held to the inventory`

---

### T17: The errors chapter's code tables

**What**: Every claim row of the chapter `errors` gets its verdict and test in `book_errors`: each code the tables say a mistake gives is provoked by the smallest model that makes that mistake and asserted (`holds`) or asserted as what the binary gives today (`diverges`, with its §11 line); the run-time codes the same way. `errors` joins `SETTLED_CHAPTERS` in both guards.
**Where**: `tests/book/errors.rs`
**Depends on**: T16
**Reuses**: `Model::with_source`, `Refusal::expect`
**Requirement**: RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `errors` is `pending`; `Run inventory` and `Run claims` pass with `errors` settled
- [x] Gate check passes: Quick (`Run errors`, `Run inventory`, `Run claims`)
- [x] Test count: one test per code claim, written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the errors chapter's codes, through piperine::`

---

### T18: Chapter 1, Welcome

**What**: `tests/book/welcome.rs` (target `book_welcome`) settles every row of the chapter `welcome`: the resistor, the RC project (`Model::example("rc")`), the monitor, the delayed bench (`Model::marked`), the inverter; each value sentence (the 63 % and 86 % of the charge, the crossing at 693.1 ns against 693.147 ns, the operating point's 1 V) asserted through `piperine::`; `refused`/`diverges` rows in §11; `welcome` joins `SETTLED_CHAPTERS` in both guards.
**Where**: `tests/book/welcome.rs`
**Depends on**: T17
**Reuses**: `Model`, `Refusal`, `PrintedNumber`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] `welcome.rs` holds `the_rc_step_charges_as_the_book_says` (the step response at 1 us and 2 us, and the `trip` record's time, at the Book's printed precision)
- [x] No row of `welcome` is `pending`
- [x] Gate check passes: Quick (`Run welcome`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 1 through piperine::`

---

### T19: Chapter 2, From Rust

**What**: `tests/book/from_rust.rs` (target `book_from_rust`) settles every row of `from-rust` (the `Blinky` model as `host-ii` made it run, its records and table values; the `emit!` fragment), as T18 does.
**Where**: `tests/book/from_rust.rs`
**Depends on**: T18
**Reuses**: `Model::marked`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `from-rust` is `pending`; `from-rust` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_from_rust`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 2 through piperine::`

---

### T20: Chapter 3, Values

**What**: `tests/book/values.rs` (target `book_values`) settles every row of `values` (integer wrapping, `diode_current`, optional arguments, `Zone` and the monitor's `zone` records, the adder's `lost` record with `b=x`, the sample-and-hold, `erfcx`), as T18 does.
**Where**: `tests/book/values.rs`
**Depends on**: T19
**Reuses**: `Model`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `values` is `pending`; `values` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_values`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 3 through piperine::`

---

### T21: Chapter 4, Natures

**What**: `tests/book/natures.rs` (target `book_natures`) settles every row of `natures` (carrier declarations and their initial values, the alarm line, `HotResistor`, `Stream`), as T18 does.
**Where**: `tests/book/natures.rs`
**Depends on**: T20
**Reuses**: `Model`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `natures` is `pending`; `natures` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Build, with `cargo test --test book_natures`
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: build

**Commit**: `test: the Book's chapter 4 through piperine::`

---

### T22: Chapter 5, Modules

**What**: `tests/book/modules.rs` (target `book_modules`) settles every row of `modules` (the ladder, `lowpass` as a module, `idt`, the PI controller, `RSwitch`, the memory bank, `nch`, the `extern "osdi"` module), as T18 does.
**Where**: `tests/book/modules.rs`
**Depends on**: T21
**Reuses**: `Model`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `modules` is `pending`; `modules` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_modules`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 5 through piperine::`

---

### T23: Chapter 6, Analog

**What**: `tests/book/analog.rs` (target `book_analog`) settles every row of `analog` (the capacitor, the clipper across its two blocks, the diode, the switch, `Servo`, `noise`), as T18 does.
**Where**: `tests/book/analog.rs`
**Depends on**: T22
**Reuses**: `Model::marked`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `analog` is `pending`; `analog` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_analog`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 6 through piperine::`

---

### T24: Chapter 7, Digital

**What**: `tests/book/digital.rs` (target `book_digital`) settles every row of `digital` (the counter and its 257 `state` records with one `full` at 255.5 us, `SrLatch`, `Pfd3`, the debouncer, the PWM and its checker, `IrqSource`, `Dff`, `MacPipe`), as T18 does.
**Where**: `tests/book/digital.rs`
**Depends on**: T23
**Reuses**: `Model`, `KeptRecords`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `digital` is `pending`; `digital` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_digital`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 7 through piperine::`

---

### T25: Chapter 8, Mixed

**What**: `tests/book/mixed.rs` (target `book_mixed`) settles every row of `mixed` (the cycle counter's three pulses, the comparator, `D2A`, the chapter's buck across its two blocks, the delta-sigma modulator, the SAR ADC), as T18 does.
**Where**: `tests/book/mixed.rs`
**Depends on**: T24
**Reuses**: `Model::marked`, `KeptRecords`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `mixed` is `pending`; `mixed` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Build, with `cargo test --test book_mixed`
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: build

**Commit**: `test: the Book's chapter 8 through piperine::`

---

### T26: Chapter 9, Packages

**What**: `tests/book/packages.rs` (target `book_packages`) settles every row of `packages` (the `lowpass` project as the Book assembles it, `pub mod`, `include_str!` tables, the `E0163` ambiguity and its `crate::` fix), as T18 does.
**Where**: `tests/book/packages.rs`
**Depends on**: T25
**Reuses**: `Model::marked`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `packages` is `pending`; `packages` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_packages`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 9 through piperine::`

---

### T27: Chapter 10, Core and standard library

**What**: `tests/book/core_std.rs` (target `book_core_std`) settles every row of `core-std` (`Tour`, `VT_300K`, `Rc`, `Latch`, `DacRamp`, the resolved line, `Pad`, `Reg`, `Probe`, the watchdog, a crate's own `vt`), as T18 does.
**Where**: `tests/book/core_std.rs`
**Depends on**: T26
**Reuses**: `Model`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `core-std` is `pending`; `core-std` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_core_std`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's chapter 10 through piperine::`

---

### T28: The cheat sheet

**What**: `tests/book/cheatsheet.rs` (target `book_cheatsheet`) settles every row of `cheatsheet` (most blocks are `fragment`s of a chapter's model or `refused` with the form they show), as T18 does.
**Where**: `tests/book/cheatsheet.rs`
**Depends on**: T27
**Reuses**: `Model`, `Refusal`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `cheatsheet` is `pending`; `cheatsheet` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Build, with `cargo test --test book_cheatsheet`
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: build

**Commit**: `test: the Book's cheat sheet through piperine::`

---

### T29: The POM chapter

**What**: `tests/book/pom.rs` (target `book_pom`) settles every row of `pom` (the `Front` model and what the chapter says its POM holds), as T18 does.
**Where**: `tests/book/pom.rs`
**Depends on**: T28
**Reuses**: `Model`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `pom` is `pending`; `pom` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_pom`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's POM chapter through piperine::`

---

### T30: The device ABI chapter

**What**: `tests/book/abi.rs` (target `book_abi`) settles every row of `abi` (the resistor and the flip-flop the chapter compiles, and its value sentences), as T18 does.
**Where**: `tests/book/abi.rs`
**Depends on**: T29
**Reuses**: `Model`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `abi` is `pending`; `abi` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_abi`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's ABI chapter through piperine::`

---

### T31: The compiler chapter

**What**: `tests/book/compiler.rs` (target `book_compiler`) settles every row of `compiler` (the `devices` crate and the diode it compiles), as T18 does.
**Where**: `tests/book/compiler.rs`
**Depends on**: T30
**Reuses**: `Model`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `compiler` is `pending`; `compiler` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Quick (`cargo test --test book_compiler`, `Run inventory`, `Run claims`)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's compiler chapter through piperine::`

---

### T32: The solver and structure chapters

**What**: Every claim row of `solver` and `structure` gets its verdict; a `holds` or `diverges` row is a test of `tests/book/solver.rs` or `tests/book/structure.rs` (targets `book_solver`, `book_structure`), created only when such a row exists; both chapters join `SETTLED_CHAPTERS` of both guards.
**Where**: `tests/book/solver.rs`
**Depends on**: T31
**Reuses**: `Model`
**Requirement**: RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `solver` or `structure` is `pending`
- [x] Gate check passes: Quick (`Run inventory`, `Run claims`, and each target created)
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: quick

**Commit**: `test: the Book's solver and structure chapters`

---

### T33: The command line chapter

**What**: `tests/book/cli.rs` (target `book_cli`) settles every row of `cli`: a sentence about what a run gives (values, records, a check's verdict, a refusal's code) is `holds` through `piperine::`; a sentence only the binary can make (an exit code, the layout of a line, a file written) is `binary`, naming an existing `piperine-cli` test or a new one in `crates/piperine-cli/tests/book.rs`.
**Where**: `tests/book/cli.rs`
**Depends on**: T32
**Reuses**: `Model::example`, `crates/piperine-cli/tests/book.rs`
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] No row of `cli` is `pending`; `cli` is in `SETTLED_CHAPTERS` of both guards
- [x] Gate check passes: Build, with `cargo test --test book_cli` and `Run clibook`
- [x] Test count: written in the commit body

**Tests**: integration
**Gate**: build

**Commit**: `test: the Book's command line chapter, through piperine:: and the binary`

---

### T34: Triage of the divergences

**What**: Every `diverges` row the chapters left is classified in `.specs/features/root-e2e/divergences.md` (one line per row: id, sentence, what the code gives, cause (1), (2), (3) or disputed, area, owner): (2) rows become `refused` naming their owner feature in §11; (3) rows become `unraised` naming `params-and-io` (PI-13) or the feature of `GAPS.md` §10; disputed rows become `disputed` and are listed for the user, undecided; (1) rows stay `diverges` with an area of T35 to T40. The inventory guard gains the rules: a `refused` or `unraised` row's §11 line names an owner feature that has a spec under `.specs/features/`; a `disputed` row is in `divergences.md`. When the (1) rows exceed what T35 to T40 can carry (more than about 6 rows in one area, or about 30 in all), the overflow is written as the spec of a follow-up feature `book-conformance` (queued next, the fixes not optional) and reported to the orchestrator before T35.
**Where**: `.specs/features/root-e2e/divergences.md`
**Depends on**: T33
**Reuses**: `GAPS.md` §10 (codes and owners), `ROADMAP.md` "Every gap has a feature"
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [x] Every `diverges` row of the inventory has one line in `divergences.md` with its cause and area or owner
- [x] `inventory_guard.rs` holds `every_gaps_row_names_its_owner` and `disputed_rows_are_listed_for_the_user`
- [x] Gate check passes: Quick (`Run inventory`, `Run claims`)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): root-e2e - the divergences, by cause`

---

### T35: Diagnostics say what the spec says

**What**: Every kind (1) row of `divergences.md` in the area "diagnostics" (a diagnostic's code number, message, labelled name or help where the rule is implemented) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: the crate `divergences.md` names for the area (`crates/piperine-lang/src/` mostly)
**Depends on**: T34
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix(lang): diagnostics follow the spec`

---

### T36: Values and records follow the spec

**What**: Every kind (1) row of `divergences.md` in the area "values and records" (a number, a record's fields or their order, a table's value) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: the crate `divergences.md` names for the area
**Depends on**: T35
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix: values and records follow the spec`

---

### T37: Analog behavior follows the spec

**What**: Every kind (1) row of `divergences.md` in the area "analog" (an analog run's result where the construct is implemented) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: the crate `divergences.md` names for the area
**Depends on**: T36
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix: analog behavior follows the spec`

---

### T38: Digital and mixed behavior follow the spec

**What**: Every kind (1) row of `divergences.md` in the area "digital and mixed" (a reaction, a commit, an event or a boundary crossing where the construct is implemented) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: the crate `divergences.md` names for the area
**Depends on**: T37
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix: digital and mixed behavior follow the spec`

---

### T39: Packages and the standard library follow the spec

**What**: Every kind (1) row of `divergences.md` in the area "packages and the library" (a package rule, a module path, a stdlib item's behavior) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: the crate `divergences.md` names for the area
**Depends on**: T38
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix: packages and the library follow the spec`

---

### T40: The host and the command line follow the spec

**What**: Every kind (1) row of `divergences.md` in the area "host and command line" (an exit code, a line's layout, a host failure's code or message (MD-65: the rule in `piperine-api`)) is fixed in the code so the binary and the library give what the Book and the spec state: the fix carries its own test in the crate it changes, the row's Book test is rewritten to assert the Book's value, the row becomes `holds` and its §11 line goes. An area with no kind (1) row records "none" in `divergences.md` and commits that alone. A fix found to need a construct that is not implemented is reclassified (2) with its owner, reported in the commit body.
**Where**: `crates/piperine-api/src/` or `crates/piperine-cli/src/`, as `divergences.md` names
**Depends on**: T39
**Reuses**: the Book's sentence and `docs/spec/` as the expected value; the crate's existing tests
**Requirement**: RE-02, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] No row of the area is `diverges`; `divergences.md` marks each fixed with its commit
- [ ] Each fix has a test in its crate that fails with the fix reverted
- [ ] Gate check passes: Full (the crate's targets, the Book targets of the rows, `Run inventory`, `Run claims`, then the root `cargo test`)
- [ ] Test count: written in the commit body

**Tests**: integration
**Gate**: full

**Commit**: `fix(api,cli): the host and the command line follow the spec`

---

### T41: The guards turn strict

**What**: Both guards hold that no row is `pending`, `diverges` or `disputed`, in any chapter: at close every row holds, or is `refused`/`unraised` naming an owner feature; a `disputed` row the user has not answered stops this task. The `SETTLED_CHAPTERS` registries and their two tests go (redundant, MD-28 rule 3).
**Where**: `tests/book/inventory_guard.rs`
**Depends on**: T40
**Reuses**: `Inventory`
**Requirement**: RE-01, RE-03
**Executor**: Sonnet

**Done when**:

- [ ] `inventory_guard.rs` holds `no_block_row_is_pending` and `no_row_diverges`; `claims_guard.rs` holds `no_claim_row_is_pending`; `settled_chapters_have_no_pending_row` and `settled_chapters_have_no_pending_claim` are gone
- [ ] A planted `pending`, `diverges` or `disputed` row fails the new tests (asserted inside them)
- [ ] Gate check passes: Quick (`Run inventory`, `Run claims`)
- [ ] Test count: 3 new, 2 removed

**Tests**: integration
**Gate**: quick

**Commit**: `test: no Book row is left pending or divergent`

---

### T42: `GAPS.md` after the root tests

**What**: `GAPS.md` drops H11 and U10 and U5 (the Book is held block by block and sentence by sentence; what it shows refused is in §11); its counts line and U1's path follow; `docs.rs` holds it.
**Where**: `GAPS.md`
**Depends on**: T41
**Reuses**: `gaps_drops_what_msl_fixes_closed` (the form)
**Requirement**: RE-01, RE-04
**Executor**: Sonnet

**Done when**:

- [ ] `docs.rs` gains `gaps_drops_what_root_e2e_closed` (no `| H11 |` row, no `U10.` or `U5.` entry, a §11, `tests/e2e/twins` named in U1)
- [ ] `Run inventory` passes (§11 unchanged by the edit)
- [ ] Gate check passes: Quick (`Run docs`, `Run inventory`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `docs(gaps): the root tests close H11, U5 and U10`

---

### T43: The decision and the instruction files

**What**: `.specs/STATE.md` gains the next free `MD` number with the project-level decision of `design.md` ("Tech Decisions"); `ROADMAP.md` marks queue items 4 and 4b done and `root-e2e` delivered in "Every gap has a feature"; `AGENTS.md` and `CLAUDE.md` say the root holds `tests/book/`, `tests/scenarios.rs` and `tests/e2e/` (no longer "the root `tests/` holds no Rust target"), and `CLAUDE.md`'s `piperine-parity` row names `tests/e2e/`.
**Where**: `.specs/STATE.md`
**Depends on**: T42
**Reuses**: `MD-66`'s form
**Requirement**: RE-04, RE-05
**Executor**: Sonnet

**Done when**:

- [ ] `docs.rs` gains `the_roadmap_and_state_record_root_e2e`
- [ ] Gate check passes: Build
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(specs): the root holds the integrated tests`

---

## Checks

**C1** - Every `<pre>` block of the Book has exactly one inventory row with its chapter, its index and a verdict, its fingerprint matching. (RE-01, AC 1; T13)
Proof: run inventory | test every_block_of_the_book_has_one_row ... ok

**C2** - Every row names the test that covers it, and that test exists in the target it names. (RE-01, AC 2; T13)
Proof: run inventory | test every_row_names_an_existing_test ... ok

**C3** - A block with no row, or a row naming no test, fails the guard naming the block. (RE-01, AC 3; T13)
Proof: run inventory | test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok

**C4** - A `run` row names a root test that cites the row and runs its model. (RE-02, AC 1; T13)
Proof: run inventory | test run_rows_name_tests_that_run_their_model ... ok

**C5** - The RC of chapter 1, run through `piperine::`, charges as the Book says: its step response at 1 us and 2 us and its `trip` record's time at the printed precision. (RE-02, AC 1; T18)
Proof: run welcome | test the_rc_step_charges_as_the_book_says ... ok

**C6** - A `refused` row names a root test that expects its refusal with code and message. (RE-02, AC 2; T13)
Proof: run inventory | test refused_rows_name_tests_that_expect_their_refusal ... ok

**C7** - The inventory's `refused`, `unraised`, `disputed` and `diverges` rows are exactly the rows of `GAPS.md` §11, each under a gap id of `GAPS.md`. (RE-02, AC 2; T16)
Proof: run inventory | test refused_rows_and_gaps_section_11_agree ... ok

**C8** - A refused block that starts to run fails its test, naming the row and saying it now runs. (RE-02, AC 3; T15)
Proof: run errors | test a_refusal_that_runs_fails_naming_its_row ... ok

**C9** - Every sentence of the Book stating a value, a code or an outcome has a claim row. (RE-03, AC 1; T14)
Proof: run claims | test every_value_sentence_has_a_claim_row ... ok

**C10** - A `holds` or `diverges` claim names a root test whose body cites the claim. (RE-03, AC 1; T14)
Proof: run claims | test holding_claims_name_root_tests_that_cite_them ... ok

**C11** - Every claim row keeps its sentence, and the sentence is still in its chapter. (RE-03, AC 2; T14)
Proof: run claims | test every_claim_row_is_a_sentence_of_its_chapter ... ok

**C12** - The claim detector flags a code, a number with its unit and an outcome verb, and not a rule sentence. (RE-03, AC 1; T14)
Proof: run claims | test the_detector_flags_a_value_a_code_and_an_outcome ... ok

**C13** - Every twin sits in one scenario directory under `tests/e2e/` with its `.msl`, `.cir`, `.command` and `.stdout`. (RE-04, AC 1; T4, T5)
Proof: run corpus | test every_twin_has_its_files_in_one_scenario_directory ... ok

**C14** - `tests/ngspice/` holds no twin. (RE-04, AC 1; T4)
Proof: run corpus | test tests_ngspice_holds_no_twin ... ok

**C15** - Each of the sixteen twins prints, on each backend that runs it, the bytes it printed before the move. (RE-04, AC 2; T2, T3)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C16** - Every `parity_*` target passes, reading the new paths. (RE-04, AC 3; T3)
Proof: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test 'parity_*'`

**C17** - `ToleranceContract` and the comparison targets are unedited by the feature. (RE-04, AC 3; T3)
Proof: `git diff --quiet $(git merge-base HEAD main) -- crates/piperine-parity/src/contract.rs crates/piperine-parity/tests/parity_op.rs crates/piperine-parity/tests/parity_dc.rs crates/piperine-parity/tests/parity_ac.rs crates/piperine-parity/tests/parity_tran.rs crates/piperine-parity/tests/parity_tf.rs crates/piperine-parity/tests/parity_pz.rs crates/piperine-parity/tests/parity_sens.rs crates/piperine-parity/tests/parity_pss.rs crates/piperine-parity/tests/parity_temp.rs`

**C18** - Every twin of `tests/e2e/` has a scenario test at the root. (RE-05, AC 1; T10)
Proof: run scenarios | test every_twin_has_a_scenario_test ... ok

**C19** - The divider, run from its command through `piperine::`, matches ngspice under `DC_VOLTAGE`. (RE-05, AC 1; T7)
Proof: run scenarios | test divider ... ok

**C20** - The RC transient, run from its command through `piperine::`, matches ngspice on its print grid under `TRAN_VOLTAGE`. (RE-05, AC 1; T8)
Proof: run scenarios | test tran_rc ... ok

**C21** - With ngspice disabled a scenario still runs and the harness prints its skip line. (RE-05, AC 2; T10)
Proof: run scenarios | SKIP divider: ngspice comparison disabled by PIPERINE_NGSPICE=skip

**C22** - Two scenarios run in two directories, each holding only its own build. (RE-05, AC 3; T10)
Proof: run scenarios | test each_scenario_runs_in_a_directory_of_its_own ... ok

**C23** - The blocks one Book file joins name one test. (Edge case 1; T13)
Proof: run inventory | test a_model_across_blocks_names_one_test ... ok

**C24** - Every Book listing is still what the binary prints, in the command line's own target. (Edge case 2; T13)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C25** - A scenario directory holding `benches/` holds `Piperine.toml`. (Edge case 3; T4)
Proof: run corpus | test a_scenario_with_benches_holds_a_manifest ... ok

**C26** - No block row is left `pending`. (RE-01, AC 2; T41)
Proof: run inventory | test no_block_row_is_pending ... ok

**C27** - No claim row is left `pending`. (RE-03, AC 2; T41)
Proof: run claims | test no_claim_row_is_pending ... ok

**C28** - The root's Book targets are integration targets for the hygiene guards, found through the root manifest. (RE-01, AC 3; T11)
Proof: run hygiene | test the_root_manifest_names_its_test_targets ... ok

**C29** - Every root target follows the root naming rule, the two guards registered with their reason. (RE-01, AC 3; T12, T14)
Proof: run hygiene | test root_targets_match_the_naming_rule ... ok

**C30** - `GAPS.md` drops H11, U5 and U10 and keeps §11. (RE-04, AC 1; T42)
Proof: run docs | test gaps_drops_what_root_e2e_closed ... ok

**C31** - `ROADMAP.md` and `STATE.md` record the root's integrated tests. (RE-05, AC 1; T43)
Proof: run docs | test the_roadmap_and_state_record_root_e2e ... ok

**C33** - Every divergence is classified: a `refused` or `unraised` row's §11 line names an owner feature with a spec. (RE-02, AC 2; T34)
Proof: run inventory | test every_gaps_row_names_its_owner ... ok

**C34** - A sentence the spec may have wrong is listed for the user, not decided. (RE-03, AC 1; T34)
Proof: run inventory | test disputed_rows_are_listed_for_the_user ... ok

**C35** - At close no row diverges or is disputed: the code gives what the Book states, or the construct is refused or unraised with its owner. (RE-02, AC 1; T41)
Proof: run inventory | test no_row_diverges ... ok

**C32** - `Corpus` finds a twin by name in its scenario directory and refuses a name two scenarios hold. (RE-04, AC 1; T3)
Proof: run paritylib | test corpus::tests::a_name_in_two_scenarios_is_refused ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (2026-11-18, B1): main holds `host-ii` and `solver-hygiene`, so the corpus is sixteen twins, not twelve: the original seven, `tran_dq`, `tran_jump`, `tran_corner`, `tran_floor` and the five of `host-ii` (`parity_corpus` already counts 16; `tf` rows read `*_impedance`; `HostRun` is in `piperine-parity`; root `[[test]] host_ii` is `crates/piperine-api/tests/root_surface/host_ii.rs`). Nine twins have no golden (T2), not five. The scenario list is sixteen: T8 also carries `tran_dq`, `tran_jump`, `tran_corner`, `tran_floor` (8 tests); T9 keeps the five of `host-ii`. `piperine::Settings` does not exist: the root takes `piperine-transpile` as it already does. T2: four twins (`pss_rc`, `tran_jump`, `tran_corner`, `tran_floor`) read `now()`, which the interpreter refuses ("use --backend rust"), so `twins.rs` gives each `Twin` a `backends` list and holds those four on the Rust backend only (as `parity_tran.rs` and `deviation.rs` already do); `tran_floor` fails `E0508` by design, its golden is empty; the old `...before_the_long_run` test keeps its seven twins through `BEFORE_THE_LONG_RUN`. The goldens of the nine are captured from the binary at `6d479e3b`, both backends equal where both run. The CLI has no flag for `delmin`, so `tran_corner`'s command runs the default floor. T3: `Corpus` also has `netlist(name)` (an ngspice-only `.cir` has no twin, and `Ngspice::fixture_path` reads those); the twins' `.cir`, `.msl` and `.stdout` moved byte for byte (sha256 of all 76 files and 16 goldens equal before and after); only code lines were rewritten to the new path, comment lines wait for T6. T5: a `.command` line is shell-quoted (`--print 'i(v1)'`), so `Twin::command_arguments` (in `Corpus`, one place for the root test and the cli test) splits at blanks outside single quotes (one extra inline test, `a_command_is_read_as_the_arguments_after_piperine`); `piperine-cli` takes `piperine-parity` as a dev-dependency; the compiled-only list (`COMPILED_ONLY`) stays in `twins.rs`; the guard also holds `.command`. T6: `noise_rc.cir:8` names `tests/ngspice_noise.rs` (a root target that no longer exists), which `.cir` bytes keep (no `.cir` changes), so the done-when grep is `grep -rn "tests/ngspice\b" crates src tests` (word boundary) and prints nothing; the `piperine-transpile` printer test strings changed in input and expected text together. B2 T7: the root takes `piperine-cli` and `piperine-parity` as dev-dependencies; `scenarios.rs` parses each `.command` with `Cli::parse` and asserts its target is the twin's own model; a run builds into a `tempfile` directory of its own (`Ran` owns it); `COMPILED_ONLY` is repeated in `scenarios.rs` (the list stays per target, as in `twins.rs`); `Comparison::OperatingPoint` serves `temp_res` too (T9: its table is the same `signal | value` shape and ngspice prints `v(mid)`), so there is no separate temperature variant; `workspace_shape` admits root Rust that is top-level `tests/*.rs`, a `path = ` line of the root manifest, or under `tests/book/support/`. T8: `Check` holds an optional golden, so every `Comparison::hold` runs the scenario (skip or not) and compares only when ngspice ran; the DC sweep grid is aligned to a billionth of the step, not `Golden::same_grid` (ngspice's accumulated sweep value at zero is 3.2e-16 where the table prints 0; no contract bound moves); the AC table's phase column is degrees and is converted to radians for `AC_PHASE`; `tran_corner` takes the session's circuit (`Session::circuit`) to `transient` with `dt_min = 1e-9` and reads the waveform at the export's times (diverges: no run key or flag for `dt_min`, listed for Phase 9); `tran_floor` asserts `E0508` within 1 us..1.01 us and ngspice's "Timestep too small", the empty `.stdout` aside. Observed deviation vs ngspice (largest, both backends equal): divider 0, isrc_r 0, diode_series 6.2e-7 V, diode_iv 7.0e-6 A, diode_rev 5.0e-20 A, ac_rc 1.2e-9, tran_rc 8.6e-5 V, tran_dq 6.1e-5 V, tran_jump 0, tran_corner 1.2e-7 V. T9: `pss_rc` compares the orbit rows whose phase is also a row of ngspice's 0.04 us grid (the command prints every 0.1 us: 21 rows, asserted) plus the closed form; `Roots` (the pole/zero printing helper) is a type of `scenarios.rs`. Largest deviation vs ngspice: tf_divider 0, pz_lowpass 1.1e-8, sens_divider 1.9e-9, pss_rc 2.1e-4 V, temp_res 4.5e-7 V, both backends equal (pss_rc Rust only). T10: the compiled backend builds `rust/abi-<hash>` (the shared runtime) plus one `rust/<hash>` per design, so "its own build" is one non-abi design directory per scenario; `Scenario::hold_against` takes the ngspice run (the skip test gives it `Lookup::Skip`); the per-phase root `cargo test` of the Build gate is left to the one full run at the end of B2 (instruction of the dispatch). T11: `RootManifest` (in `support/sources.rs`) reads `[[test]]` `path` lines; `integration_targets` adds a declared path, `root_targets` therefore yields `book/<file>.rs` names (the naming rule reads them like any root target); the host_ii path under `crates/` is not counted (unchanged). T12: a Book block is a `<pre data-lang>` (214: the 7 `<pre class="mermaid">` diagrams are not code blocks); the root `Book::file(path, upto)` joins the marked blocks up to and including `upto` (`piperine-cli` joins those strictly before a listing); `book/inventory_guard.rs` is the first `ROOT_TARGET_EXEMPTIONS` entry (its text must never say `piperine::`, or the exemption goes stale); `Block` gains its attributes in T13, where the listings need them (no unused code before). T13: `inventory.tsv` has 214 block rows at `8c8a402b`+ (101 pending: 99 `msl` + 2 `diag`; 78 shown; 35 held by `piperine-cli/book::every_cli_listing_of_the_book_is_what_the_binary_prints`); the `Guard` rules are methods returning one line per offence (a failure prints the corrected row), held on a planted Book, planted tests (a tempdir with a manifest and target files) and planted inventories; `Coverage` finds a covering test as a top-level `fn name(` ending at the first `\n}\n` (a root `piperine/<target>` reads its path from the `[[test]]` table, any `<crate>/<target>` is `crates/<crate>/tests/<target>.rs`); `Tree` (Book + inventory + coverage) is the real-tree fixture of the seven rule tests; `Inventory` parses claim rows already (`Kind::Claim`), T14 adds their rules. B4 T22 to T28 (chapters 5 to 10 and the cheat sheet): shared support `printed.rs`, `standard_library.rs` (natures.rs now uses it) and `Fields` in `records.rs`. Gaps added in 4.8: D3 (a spec code the checker answers with another: E0333 is P0002, owner `params-and-io`) and D4 (a value or record other than the Book's: the buck start-up, owner `root-e2e`). Rows by chapter (blocks run/refused/fragment, claims): modules 2/7/2, 13 holds 3 unraised 1 diverges 4 refused 3 prose; analog 2/5/0, 18 holds 3 refused 2 unraised; digital 4/6/0, 22 holds 5 refused 1 binary 2 diverges; mixed 5/3/0, 21 holds 3 prose 2 diverges 1 disputed 1 refused 1 binary; packages 4/2/1, 10 holds 1 unraised; core-std 8/3/0, 10 holds 1 unraised; cheatsheet 2/7/1, 1 holds. Diverges rows (kind 1) so far: diagnostics (D1) 3: digital~ce04e7a4 (latch without after: two E0338, no E0339), digital~54ec4a1d (count + 1.0 is P0002 not E0432), mixed~98e4dc1b (on inside a reaction is E0404 not E0350); values/records (D4) 1: mixed~68328637 (buck start-up: the operating point has the first hs commit applied, vout 11.8 V, duty 0 at 10 us); params/placement 1: modules~1faf8c12 (m and dtemp bound at a placement are E0320: a block-less refusal the `refused` claim rule cannot hold, so recorded diverges, T34 to reclassify refused under L29 `params-and-io`). Running diverges total: 5 (diagnostics 3, values 1, params 1). Disputed: mixed~22d4126e (sin runs, same stale sentence as welcome~5b11a01a). Unraised: E0333 (D3), E0340 and E0304 (generics, L31 and L30), E0610 (L57), E0702 (L18); E0504 and E0507 ARE raised (GAPS section 10 lists them wrongly: RunCode variants, not string literals). Substitutions: refused Book models whose numbers are held run the same design with `limexp` as `.exp()` and without `settle` (clipper), the integer param as a literal and `known_or(false)` (debouncer, PWM, check, watchdog); each edit asserts its text is in the Book. mixed~fef5c97a is `binary` over `piperine-cli/buck::startup_ends_with_the_reference_at_128`, which asserts 10 mV where the Book says 3 mV (measured 2.5 mV at 600 us, the run takes 3 minutes): T32 may add a tighter binary test. Observations, not rows: `--stream <name>` that omits `sim` makes a `finish()` end the run with "ended before its stop" (backend error); a file stem that is a keyword or `analog`/`extern`/`module` is E0171; `let half = r / 2.0` used as a param value is E0321 (L37); the cheat sheet blocks that mix names of several modules (crossing card) cannot run, so cheatsheet#6 is a fragment of mixed#7. B5 T29 to T31: `Model::opened_path` (support) gives a test the path a run opens, for a session with a cache of its own (compiler: the generated `design.rs` and `libdesign.so`, one each per design). pom: the object model claims hold on `piperine_lang::pom::Design` (no run; `Front` runs as a top through `piperine::`); abi: the four interface sentences (statuses, thunks, an emulator's clock, a time point) are `prose` (the ABI crate's design, no run value). compiler: the table's worst-difference cells are a harness measurement: reverse-bias worst difference measured 4.98e-20 A against the Book's 5.9e-21 A, both inside the 1 pA floor, so `compiler~e9ad112c` is `disputed` (new gap D5, owner `user`); `P0004` is raised only for an internal error no source reaches, `prose`; `compiler#2` (the diode with `limit`) runs; observation, not a row: the generated listing of compiler#3 says `devices.msl:5:1` and `8:32` where the binary gives `4:1` and `8:34` for the twin's `devices.msl` (a `shown` block: the Book's positions are of another file layout). Running diverges after T31: unchanged at 5 (diagnostics 3, values 1, params 1); new disputed: 1 (compiler~e9ad112c). T32: `solver` and `structure` have 10 and 5 claim rows and no block, so both targets exist (`book_solver` 5 tests, `book_structure` 3); `support/manifests.rs` (`Manifests`) reads a crate's `[dependencies]` for the sentences about which crate knows which. Rows: solver 5 holds 5 prose (homotopy, rollback and the chapter's purpose: algorithm, no run value); structure 3 holds 1 prose 1 binary (`structure~58f974e3` over `piperine-cli/commands::check_renders_every_diagnostic_and_exits_1`). E0506 and E0501 are raised through MSL (a comparator that inverts the level it reads; a bit that inverts itself) with the Book's counts (20 rounds, 1000 microsteps) and the nets named. `solver~4642dd47`: no source form marks a net to relax, so only the refusal of the other loop (`E0339`) is held. A step lands on the digital instant exactly; the row of the instant still has the analog node at its value before it and the next row has the change, as the Book's `Solve` and `Deliver` steps say. diverges unchanged at 5. T33: `cli` has 6 pending blocks settled (cli#3, #4, #25, #34, #44 run; cli#13 fragment of cli#4) and 69 claims: 30 holds (book_cli, 19 tests), 34 binary over existing `piperine-cli` tests (plus two new: `book::a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one`, `buck::startup_ends_within_3_mv_of_0_8_volts`, added to `buck.rs` because it shares the one 600 us run fixture), 1 diverges, 4 prose (goal, buck description and timing, the memory sentence). mixed~fef5c97a (the 3 mV claim) now names the tighter buck test; the buck run reads 0.8025 V at 600 us, so it held at 3 mV and needs no Phase 9 fix. New diverges: cli~0549bfb1 (gap D6, area host and command line): a nodeset on an ac or transient run (flag or key) is accepted and ignored where the Book says P0010; the fix is the request validation in piperine-api (MD-65). Observation: an unknown run `[run.x]` TOML that is not valid reports E0170 (the manifest is not valid TOML). Totals after B5, all chapters: diverges 9 (diagnostics 6: errors#1, errors#2, from-rust~64ee8d1d, digital~ce04e7a4, digital~54ec4a1d, mixed~98e4dc1b; values and records 1: mixed~68328637; params and placement 1: modules~1faf8c12; host and command line 1: cli~0549bfb1), disputed 3 (welcome~5b11a01a, mixed~22d4126e, compiler~e9ad112c), pending 0.
**Dispatched:** B1 - Sonnet worker, 2026-11-18 (host-ii merged 1a3d69dc). B1 done (40a5cbf2, 2300/0). B2 (phases 2-3, T7-T13) - Sonnet worker, 2026-11-18. Noted for Phase 9 triage: no run key/flag for dt_min (ngspice delmin), and --output rejects a raw identifier r#out (E0801). B2 done (c560092b, 2330/0). B3 (phases 4-5, T14-T21) - Sonnet worker, 2026-11-18. B3 done (076526e9). Ruling: disputed welcome~5b11a01a is a stale Book sentence (sin runs since msl-fixes): the Book is corrected in Phase 9 (T35-T40 may edit the Book only to match implemented, spec-conforming behaviour). The two observations (stream named like a signal fails decoding; keyword file stem E0171) get GAPS rows with owners in T42. B4 (phases 6-7, T22-T28) - Sonnet worker, 2026-11-19. B4 done (b7b183a5; 5 diverges). Phase 9 must also fix: a finish() with --stream omitting sim fails 'ended before its stop' (finish must end the run whatever streams print). B5 (phase 8, T29-T33) - Sonnet worker, 2026-11-19. B5 done (ac63c892, 2611/0, 0 pending). Rulings for Phase 9: the 3 disputed rows (welcome~5b11a01a and mixed~22d4126e: sin runs; compiler~e9ad112c: the measured reverse-bias difference is 4.98e-20 A) and the compiler#3 listing line numbers are stale Book text — the Book is corrected to the binary/measurement (no user decision needed). cli~0549bfb1: the spec (and host-ii F6) say nodeset applies to every analysis — the Book's P0010 sentence is stale and is corrected; and if nodeset is in fact ignored on ac/transient (no effect), that is a code divergence fixed in T40. B6 (phase 9, T34-T40) - Sonnet worker, 2026-11-19.
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6
Phase 2:  T7 ------→ T8 ------→ T9 ------→ T10
Phase 3:  T11 ------→ T12 ------→ T13
Phase 4:  T14 ------→ T15 ------→ T16 ------→ T17
Phase 5:  T18 ------→ T19 ------→ T20 ------→ T21
Phase 6:  T22 ------→ T23 ------→ T24 ------→ T25
Phase 7:  T26 ------→ T27 ------→ T28
Phase 8:  T29 ------→ T30 ------→ T31 ------→ T32 ------→ T33
Phase 9:  T34 ------→ T35 ------→ T36 ------→ T37 ------→ T38 ------→ T39 ------→ T40
Phase 10: T41 ------→ T42 ------→ T43
```

Execution is strictly sequential - there is no intra-phase parallelism. B3 T14: `ClaimDetector` (in `claims_guard.rs`, data lists: code, number with a unit or a count, outcome verb) flags a sentence of `<p>`, `<li>`, `<td>`; the row id is `<chapter>~<8 hex of the fingerprint>`; a repeated sentence in a chapter is one claim; a claim row's verdicts are `holds`, `diverges`, `unraised`, `disputed` (each names a root test that cites the id), `binary` (a `piperine-cli` test that resolves), `refused` (a `refused` block id), `prose`, `pending`. `Book::plain` reads `&nbsp;` as a space (found at T18: "63 %" was missed; 46 rows added, 9 stale gone in a fix commit). 370 claim rows at the end of B3 (a table cell with only a code, `E0319`, is a claim: the code's meaning is in the next cells). B3 T15: `Model::{lone, written, package, marked, example, wrapped, with_source, of_run, run, text_of}`, `Ran { table, records, metadata }`, `Refusal::{reported, reported_by, of, expect}`, `Reported { code, message, primary_span, primary_label, secondary_labels, help }`; `marked` of a path with a directory copies `examples/<dir>` and opens the file as a lone file (as `piperine run <file>`); a package run is chosen with `of_run`; `Faulty` and `Chapter` are in `tests/book/support`. A `refused` block the Book teaches (the worked `SampleHold` of chapter 11) has gap `-` and no section 11 row; every other `refused`, `diverges`, `unraised` or `disputed` row (block or claim) has a gap and a row. B3 T16: `GAPS.md` section 11 has `| Book | Gap | Owner | Today |` rows; new entries in section 4.8: D1 (a diagnostic differs from the Book: a missing part or another code) and D2 (a Book sentence the code outran: `disputed`, owner `user`); the inventory guard's `tests_that_do_not_exist` leaves a `refused` claim's block id to `book_claims`. B3 T17 to T21: claims settled per chapter: errors 49 (43 holds, 1 binary, 1 unraised, 4 prose), welcome 29 (21 holds, 5 binary, 2 prose, 1 disputed), from-rust 15 (11 holds, 1 binary, 1 diverges, 2 prose), values 23 (18 holds, 2 binary, 1 prose, 1 refused, 1 unraised), natures 18 (12 holds, 3 prose, 3 refused); blocks: 26 settled (run 13, refused 7, fragment 4, diverges 2); `Model::of_run` replaced `run_named` so every `run` row's test calls `.run(`; helpers of the chapter targets are methods of types (`Faulty`, `Printed`, `Mistake`, `RcProject`, `Blinky`, `Transient`, `Fields`, `Reaction`, `StandardLibrary`). Diverges rows (kind 1) so far: diagnostics 3 (errors#1 no help line on E0150, errors#2 no secondary label on E0163, from-rust~64ee8d1d `emit!` in a module body is E0900, the Book says E0601); the other areas 0; no area above the T34 threshold. Unraised: E0414 (errors, L40, `analog-structure`), E0123 (values, L12, `foreign-models`). Disputed: welcome~5b11a01a (the status paragraph lists `sin` as refused; it runs). Observations outside the Book's claims, not rows: a stream named like a signal of the same instance (`emit!("y", ..)` in a top with `signal y`) fails the table with "cannot decode a value of type bool: 24 bytes where its layout holds 2"; a file stem that is a keyword (`extern.msl`) is `E0171`. Gate notes: the per-phase Build gate (the `**Build**` line) ran green after T21 (zero warnings); the full `cargo test --no-fail-fast` ran once at the end of B3: 265 targets, 0 failed, no SKIP line.

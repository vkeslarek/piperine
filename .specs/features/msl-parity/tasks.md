# MSL Parity Harness (M1) Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-parity/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-parity || cargo build -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run lib**: `cargo test -p piperine-parity --lib`
**Run twins**: `cargo test -p piperine-parity --test twins`
**Run harness**: `cargo test -p piperine-parity --test harness`
**Run bench**: `cargo test -p piperine-parity --test bench`
**Run op**: `cargo test -p piperine-parity --test parity_op`
**Run dc**: `cargo test -p piperine-parity --test parity_dc`
**Run ac**: `cargo test -p piperine-parity --test parity_ac`
**Run tran**: `cargo test -p piperine-parity --test parity_tran`
**Run corpus**: `cargo test -p piperine-parity --test parity_corpus`

Executors: Sonnet implements every task; Opus verifies the feature. The worktree and branch are told at dispatch. Phase 1 (T1 to T4) runs in wave 1: it needs only `piperine-lang` as it is today, ngspice and the fixtures. Phase 2 (T5 to T10) runs in wave 3: it needs `msl-typeck`, `msl-ir`, `msl-elab` and `ir-interp` merged into the worktree first. ngspice must be installed where the tests run (`/usr/bin/ngspice`, ngspice-47 on the planning machine); `check_proofs.py` is run without `PIPERINE_NGSPICE` set.

Rules every task follows:

- **No `.cir` file is edited**, and none of the `.msl` twins or `devices.msl` either: they were checked against today's front end when the plan was written. A twin that seems wrong: stop and report.
- **One place for tolerances.** A numeric bound of a comparison exists only in `contract.rs` (values of the spec's assumptions table, Q3 decided). A parity test calls `ToleranceContract::assert`; it never compares a difference with a literal. A test that fails under the contract is a finding to report, never a reason to widen a bound or to move a number.
- **ngspice absent fails** (Q2, decided): `PIPERINE_NGSPICE=skip` is the only opt-out, it prints `SKIP <fixture>: ...`, and the MSL side still runs. No test mutates the process environment: the lookup is a method that takes the variable's value and the `PATH` as arguments.
- **The golden is ngspice run at test time**, in a scratch directory of its own, removed on drop. No golden number is stored in the tree.
- The harness panics with full context (fixture, quantity, point, both values, bound, rationale, ngspice version): it serves tests only.
- One test function per fixture, named after the fixture; `parity_corpus` looks for that name.
- **Fail loud.** A form the code does not handle is a diagnostic, a `Fault` or an entry of `check()`, never a silent default.
- **AGENTS.md idiom rules bind**: no loose function (every `fn` is a method of a struct or a trait), no macro of your own (`macro_rules!`, proc-macros; derives are fine), modules named after what they do, no `unwrap()`/`expect()` on a user-input path.
- Tests derive from the spec's criteria, never from the implementation. Test functions have exactly the names `## Checks` cites. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`, never set `CARGO_TARGET_DIR`.
- A numeric assertion or tolerance of an existing test is never edited. A number you cannot explain: stop and report, never retune.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the five Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `.specs/features/codegen/context.md` (S1 to S10; ngspice parity is the proof of M1).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| `ToleranceContract`, `Golden` parsers, `Ngspice` lookup, `Waveform` | unit | each AC of PAR-05 (1 to 4), PAR-03 (4, 5), PAR-04 (1 to 3), PAR-06 AC 5, edge cases 1 to 3; at, just inside and just outside every bound | unit tests in `crates/piperine-parity/src/{contract,golden,ngspice,waveform}.rs` | `cargo test -p piperine-parity --lib` |
| The corpus files | unit | each AC of PAR-01, PAR-08 AC 1 | `crates/piperine-parity/tests/twins.rs` | `cargo test -p piperine-parity --test twins` |
| Running ngspice | integration | PAR-03 (1 to 3), edge case 4, on the real binary | `crates/piperine-parity/tests/harness.rs` | `cargo test -p piperine-parity --test harness` |
| `Bench`: the chain | integration | each AC of PAR-02, PAR-04 AC 4; no ngspice involved | `crates/piperine-parity/tests/bench.rs` | `cargo test -p piperine-parity --test bench` |
| Fixture comparisons | e2e | one test per twin: every exported point compared under its contract | `crates/piperine-parity/tests/parity_{op,dc,ac,tran}.rs` | `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran` |
| Corpus guard | unit | each AC of PAR-07, PAR-05 AC 5; green on the tree, red on planted text | `crates/piperine-parity/tests/parity_corpus.rs` | `cargo test -p piperine-parity --test parity_corpus` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | `cargo test -p piperine-parity --test <the task's target>` or `--lib` |
| Full | every task | `cargo test -p piperine-parity` |
| Build | T1 and the last task of each phase | `cargo test -p piperine-parity`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Two phases. Phase 1 runs in wave 1, Phase 2 in wave 3 after the other four features are merged. Two batches, one per phase (4 and 6 tasks), both Sonnet.

### Phase 1: The harness without the chain

Executor: Sonnet.

```
T1 → T2 → T3 → T4
```

### Phase 2: The chain and the fixture tests

Executor: Sonnet.

```
T5 → T6 → T7 → T8 → T9 → T10
```

---

## Task Breakdown

## Phase 1 tasks

### T1: The crate and the corpus test

**What**: The crate `piperine-parity` (`publish = false`; dependency `piperine-lang`, dev-dependency none yet) joined to the workspace, and the target that pins the corpus: the library and the six twins exist beside their `.cir`, build with no diagnostic, name their top nodes as the `.cir` does, and the six `.cir` are byte-identical to their recorded SHA-256.
**Where**: `crates/piperine-parity/` (the new crate)
**Depends on**: None
**Reuses**: `tests/ngspice/*.msl` and `*.cir` (already in the tree); `Design::builder().root_file`; `crates/piperine-solver/tests/workspace_shape.rs` (`DEFAULT_MEMBERS`)
**Requirement**: PAR-01, PAR-08
**Executor**: Sonnet

**Done when**:

- [x] Root `Cargo.toml` lists the crate in `members` and `default-members`; `workspace_shape.rs::DEFAULT_MEMBERS` gains it
- [x] `the_cir_fixtures_are_byte_identical` pins each of the six `.cir` by its length and an FNV-1a 64-bit hash computed by a method of the test's own `Fixture` struct (no new dependency); the pinned values are taken from the files at the commit this task starts from, where `git diff origin/main -- tests/ngspice/*.cir` shows only the new `isrc_r.cir`
- [x] Top nodes: `divider` has `vin`, `mid`; `diode_iv` `vin`, `a`; `diode_series` `vin`, `a`, `b`; `ac_rc` and `tran_rc` `vin`, `out`; `isrc_r` `a`
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --test twins`: `the_corpus_holds_the_library_and_the_twins`, `every_twin_builds_without_diagnostics`, `top_nodes_carry_the_cir_names`, `the_cir_fixtures_are_byte_identical`, `the_current_source_twin_exists`
- [x] Gate check passes: `cargo test -p piperine-parity`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(parity): the piperine-parity crate and the pinned twin corpus`

---

### T2: ToleranceContract

**What**: `ToleranceContract` with the five contracts, `holds`, `floor`, `phase_applies(magnitude)`, `Point`/`Coordinate`, and `assert` panicking with fixture, quantity, point, both values, the bound, the rationale and the ngspice version line.
**Where**: `crates/piperine-parity/src/contract.rs`
**Depends on**: T1
**Reuses**: `git show 16bddaae^:tests/support/ngspice.rs` (the old contract: shape, `holds`, rationale); design "ToleranceContract" (the table of values)
**Requirement**: PAR-05, PAR-03
**Executor**: Sonnet

**Done when**:

- [x] Values: `DC_VOLTAGE` 1e-3 / 1e-6; `DC_CURRENT` 1e-3 / 1e-12; `AC_MAGNITUDE` 1e-3 / 1e-12; `AC_PHASE` 0 / 1e-4; `TRAN_VOLTAGE` 1e-3 / 1e-6; each with the rationale of the design's table
- [x] `holds` is tested at the bound, one ulp inside and one ulp outside for each contract
- [x] A rejection's panic message contains every field the criterion lists
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --lib`: `contract::tests::a_rejection_names_the_ngspice_version`, `contract::tests::the_five_contracts_carry_bounds_and_rationale`, `contract::tests::holds_is_abstol_plus_reltol_of_the_larger`, `contract::tests::a_rejection_names_point_values_bound_and_rationale`, `contract::tests::phase_is_gated_on_the_magnitude_floor`, `contract::tests::two_zeros_are_accepted`
- [x] Gate check passes: `cargo test -p piperine-parity`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(parity): the tolerance contract, the one place a bound is written`

---

### T3: Running ngspice

**What**: `Ngspice` (lookup from the variable's value and the `PATH` given as arguments; `run(fixture) -> Reference`), `ScratchDir` (a new directory per run, removed on drop), the `skip` opt-out printing one `SKIP` line, failure on a missing binary, a non-zero exit or a missing export, with ngspice's output and the fixture's name.
**Where**: `crates/piperine-parity/src/ngspice.rs`
**Depends on**: T2
**Reuses**: the old harness's `ScratchDir`; `tempfile` (workspace dependency)
**Requirement**: PAR-03, PAR-04
**Executor**: Sonnet

**Done when**:

- [x] `ngspice -b <absolute .cir>` runs with the scratch directory as its working directory; `divider.cir` yields stdout holding `v(mid) = 7.500000e+00`
- [x] Lookup: a path in the variable wins; else `ngspice` on the given `PATH`; `skip` gives `Reference::Skipped`; nothing found panics naming what was looked for and `PIPERINE_NGSPICE=skip`
- [x] Two runs started from two threads have different directories, both gone after drop
- [x] The first line of `ngspice --version` is kept on the `Golden`
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --test harness`: `ngspice_runs_in_batch_in_an_empty_scratch_directory`, `each_run_has_its_own_scratch_removed_on_drop`, `a_failed_run_or_missing_export_names_the_fixture`, `parallel_runs_do_not_share_a_scratch_directory`
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --lib`: `ngspice::tests::the_binary_comes_from_the_variable_or_the_path`, `ngspice::tests::a_missing_binary_fails_naming_the_opt_out`, `ngspice::tests::skip_passes_and_prints_one_line`
- [x] Gate check passes: `cargo test -p piperine-parity`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(parity): run ngspice in a scratch directory; absent fails, skip is explicit`

---

### T4: Golden parsers

**What**: `Golden::printed(name)`, `Golden::real(export)`, `Golden::complex(export)` and `Golden::same_grid`: strict parsing of `print` lines and `wrdata` files, the export found with or without `.data`, a row-count check, a grid comparison at 1e-9 relative.
**Where**: `crates/piperine-parity/src/golden.rs`
**Depends on**: T3
**Reuses**: the old harness's `wrdata` parser; the exports ngspice-47 writes (`x y` rows, `x re im` rows)
**Requirement**: PAR-03, PAR-06
**Executor**: Sonnet

**Done when**:

- [x] `diode_iv_sweep` parses to 37 rows, first `(-1.0, 1.01008091e-12)`; `ac_rc_ac` to 41 complex rows with `(0.5, -0.5)` at 100 kHz; `tran_rc_tran` to 100 rows (asserted in `tests/harness.rs` against the real binary)
- [x] A row `1.0 abc` is an error naming file and row; a wrong row count names both counts; a grid point off by 2e-9 relative is named
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --lib`: `golden::tests::an_unparsable_row_names_file_and_row`, `golden::tests::an_export_is_found_with_or_without_data_suffix`, `golden::tests::a_grid_mismatch_names_the_point`, `golden::tests::a_wrong_row_count_names_both_counts`
- [x] Gate check passes: `cargo test -p piperine-parity`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(parity): strict parsers of ngspice print and wrdata output`

---

## Phase 2 tasks

### T5: Bench

**What**: `Bench::load(fixture)`: `Design` from `tests/ngspice/<fixture>.msl`, `Elaborator` on its one module without ports, `Interpreter::circuit`; `node(name)`, `source(name)`, `path(instance)`; a failing stage panics with its name and every diagnostic rendered.
**Where**: `crates/piperine-parity/src/bench.rs`
**Depends on**: None (and, outside this feature: `msl-typeck`, `msl-ir`, `msl-elab` and `ir-interp` merged)
**Reuses**: `piperine_lang::pom::Design`, `piperine_lang::elab::Elaborator`, `piperine_interp::Interpreter`, `piperine_solver::prelude`
**Requirement**: PAR-02, PAR-04
**Executor**: Sonnet

**Done when**:

- [ ] `Cargo.toml` gains `piperine-interp`, `piperine-ir` and `piperine-solver`
- [ ] `Bench::load("divider")`, `dc()`, `node("mid")` reads 7.5 V within 1e-9
- [ ] `source("v1")` is the `BranchIdentifier` of the private flow pin of `<Top>.v1`
- [ ] A twin text with a type error (written in the test to a temporary directory) panics naming the stage `typeck` and the diagnostic's code
- [ ] With a `Reference::Skipped`, the comparison loop runs zero times after the solve succeeded; a failing solve still panics
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test bench`: `a_twin_loads_into_a_bound_circuit`, `a_failing_stage_is_named_with_its_diagnostics`, `nodes_and_sources_are_reached_by_cir_names`, `a_skipped_reference_still_solves_the_msl_side`
- [ ] Gate check passes: `cargo test -p piperine-parity`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: integration
**Gate**: full

**Commit**: `feat(parity): Bench loads a twin through the whole chain`

---

### T6: Operating-point parity

**What**: The target `parity_op` with the tests `divider`, `diode_series` and `isrc_r`, each comparing its printed node voltages under `DC_VOLTAGE`; the CI workflow installs ngspice.
**Where**: `crates/piperine-parity/tests/parity_op.rs`
**Depends on**: T5
**Reuses**: `Bench`, `Ngspice::run`, `Golden::printed`, `ToleranceContract::DC_VOLTAGE`
**Requirement**: PAR-06, PAR-08
**Executor**: Sonnet

**Done when**:

- [ ] `divider` compares `v(vin)` and `v(mid)`; `diode_series` `v(a)` and `v(b)`; `isrc_r` `v(a)` (ngspice: +1 V)
- [ ] `.github/workflows/ci.yml` installs ngspice before the test step (Q2, decided)
- [ ] A failure is reported, never tuned away
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_op`: `divider`, `diode_series`, `isrc_r`
- [ ] Gate check passes: `cargo test -p piperine-parity`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): operating-point twins against ngspice`

---

### T7: DC sweep parity

**What**: The target `parity_dc` with the test `diode_iv`: for each of the 37 exported sweep values, `set` `v1.dc`, solve `Dc`, compare the flow of `v1` with `i(v1)` under `DC_CURRENT`.
**Where**: `crates/piperine-parity/tests/parity_dc.rs`
**Depends on**: T6
**Reuses**: `CircuitInstance::set`, `Bench::source`, `Golden::real`
**Requirement**: PAR-06
**Executor**: Sonnet

**Done when**:

- [ ] The sign is compared unchanged: ngspice gives about +1.01e-12 A at −1 V and −4.59e-2 A at +0.8 V
- [ ] The row count is asserted (37) before the loop
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_dc`: `diode_iv`
- [ ] Gate check passes: `cargo test -p piperine-parity`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): diode I-V sweep against ngspice`

---

### T8: AC parity

**What**: The target `parity_ac` with the test `ac_rc`: an `Ac` sweep over the 41 exported frequencies, magnitude under `AC_MAGNITUDE`, phase under `AC_PHASE` where `phase_applies`.
**Where**: `crates/piperine-parity/tests/parity_ac.rs`
**Depends on**: T7
**Reuses**: `AcSweepAnalysisOptions` (`crates/piperine-solver/src/analyses/ac.rs`), `Golden::complex`, `Golden::same_grid`
**Requirement**: PAR-06
**Executor**: Sonnet

**Done when**:

- [ ] The options are built from the count and the end points of the export; `same_grid` is asserted on the solver's frequencies before any value is compared
- [ ] The row count is asserted (41)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_ac`: `ac_rc`
- [ ] Gate check passes: `cargo test -p piperine-parity`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): RC low-pass AC sweep against ngspice`

---

### T9: Transient parity and Waveform

**What**: `Waveform` (linear interpolation, `None` outside the result) and the target `parity_tran` with the test `tran_rc`: a transient to 10 µs with `dt_max` one tenth of the export's time step, `v(out)` interpolated at the 100 exported times, compared under `TRAN_VOLTAGE`.
**Where**: `crates/piperine-parity/tests/parity_tran.rs`
**Depends on**: T8
**Reuses**: `TransientAnalysisOptions` (`crates/piperine-solver/src/analyses/transient.rs`), `Golden::real`
**Requirement**: PAR-06
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-parity/src/waveform.rs` holds `Waveform` with its unit tests
- [ ] An exported time outside the result panics naming the time; nothing is extrapolated
- [ ] `the_transient_step_cap_is_a_tenth_of_the_print_step` asserts the option the test passes, derived from the export's first two times
- [ ] The row count is asserted (100)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_tran`: `tran_rc`, `the_transient_step_cap_is_a_tenth_of_the_print_step`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --lib`: `waveform::tests::a_time_outside_the_result_is_none`
- [ ] Gate check passes: `cargo test -p piperine-parity`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): RC transient against ngspice`

---

### T10: The corpus guard

**What**: The target `parity_corpus`: every `<name>.msl` of `tests/ngspice/` but `devices.msl` has a `<name>.cir` and a test function `<name>` in a `parity_*` target; no `parity_*` target compares an absolute difference with a numeric literal; both checks are plain functions returning their failures, asserted empty on the tree and non-empty on planted text.
**Where**: `crates/piperine-parity/tests/parity_corpus.rs`
**Depends on**: T9
**Reuses**: `crates/piperine-lang/tests/suite_hygiene.rs` and `std_galleries.rs` (the guard pattern)
**Requirement**: PAR-07, PAR-05
**Executor**: Sonnet

**Done when**:

- [ ] Planted: a twin name with no test, a twin with no `.cir`, a line `assert!((a - b).abs() < 1e-3)`: each reported with file and line
- [ ] Green on the tree with the six twins
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_corpus`: `no_parity_target_compares_against_a_literal`, `every_twin_has_a_cir_and_a_test`, `a_literal_tolerance_in_a_parity_target_is_reported`, `the_guards_report_planted_text`
- [ ] Gate check passes: `cargo test -p piperine-parity`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `test(parity): guard the twin corpus and the tolerance rule`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`. A test function has exactly the name its check cites.

**C1** - The system SHALL hold `tests/ngspice/devices.msl` with the modules `Resistor`, `Capacitor`, `Vsource`, `Isource` and `Diode`, and one `<name>.msl` beside `<name>.cir` for `divider`, `diode_iv`, `diode_series`, `ac_rc` and `tran_rc`. (PAR-01, AC 1; T1)
Proof: run twins | test the_corpus_holds_the_library_and_the_twins ... ok

**C2** - WHEN a twin is built by `Design::builder().root_file(..)` THEN the system SHALL give a `Design` with no diagnostic. (PAR-01, AC 2; T1)
Proof: run twins | test every_twin_builds_without_diagnostics ... ok

**C3** - The system SHALL name each node of a twin's top module as its `.cir` names it, so that `v(mid)` and `<Top>.mid` are the same quantity. (PAR-01, AC 3; T1)
Proof: run twins | test top_nodes_carry_the_cir_names ... ok

**C4** - The system SHALL leave every `.cir` file of `tests/ngspice/` byte-identical. (PAR-01, AC 4; T1)
Proof: run twins | test the_cir_fixtures_are_byte_identical ... ok

**C5** - WHEN `Bench::load` is given a twin's name THEN the system SHALL build the `Design` from `tests/ngspice/<name>.msl`, elaborate its one module that has no port, interpret the `Ir` and return the bound `CircuitInstance`. (PAR-02, AC 1; T5)
Proof: run bench | test a_twin_loads_into_a_bound_circuit ... ok

**C6** - IF any stage returns diagnostics or an error THEN the system SHALL fail with the stage's name and every diagnostic rendered with its code and span. (PAR-02, AC 2; T5)
Proof: run bench | test a_failing_stage_is_named_with_its_diagnostics ... ok

**C7** - The system SHALL give access to a node's value by the name the `.cir` uses and to a source's branch flow by the instance's name. (PAR-02, AC 3; T5)
Proof: run bench | test nodes_and_sources_are_reached_by_cir_names ... ok

**C8** - WHEN a fixture is run THEN the system SHALL execute `ngspice -b <absolute path of the .cir>` with a new, empty scratch directory as its working directory. (PAR-03, AC 1; T3)
Proof: run harness | test ngspice_runs_in_batch_in_an_empty_scratch_directory ... ok

**C9** - The system SHALL use a different scratch directory for every run and SHALL remove it when the run's result is dropped. (PAR-03, AC 2; T3)
Proof: run harness | test each_run_has_its_own_scratch_removed_on_drop ... ok

**C10** - IF ngspice exits with a non-zero status, or an expected export or printed value is missing, THEN the system SHALL fail with the fixture's name and ngspice's captured output. (PAR-03, AC 3; T3)
Proof: run harness | test a_failed_run_or_missing_export_names_the_fixture ... ok

**C11** - IF a row of a `wrdata` export or a printed `name = value` line does not parse as numbers THEN the system SHALL fail naming the file and the row. (PAR-03, AC 4; T4)
Proof: run lib | test golden::tests::an_unparsable_row_names_file_and_row ... ok

**C12** - The system SHALL read a `wrdata` export under the name the `.cir` gives it, with or without the suffix `.data`. (PAR-03, AC 5; T4)
Proof: run lib | test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok

**C13** - WHEN a comparison fails THEN the system SHALL include the first line of `ngspice --version` in the message. (PAR-03, AC 6; T2)
Proof: run lib | test contract::tests::a_rejection_names_the_ngspice_version ... ok

**C14** - The system SHALL look for the binary named by the environment variable `PIPERINE_NGSPICE` when it is set to a path, else for `ngspice` on `PATH`. (PAR-04, AC 1; T3)
Proof: run lib | test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok

**C15** - IF no binary is found THEN each parity test SHALL fail with a message that names what it looked for and the opt-out. (PAR-04, AC 2; T3)
Proof: run lib | test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok

**C16** - WHERE `PIPERINE_NGSPICE` is set to `skip` each parity test SHALL pass without running ngspice and SHALL print one line `SKIP <fixture>: ngspice comparison disabled by PIPERINE_NGSPICE=skip`. (PAR-04, AC 3; T3)
Proof: run lib | test ngspice::tests::skip_passes_and_prints_one_line ... ok

**C17** - WHERE `PIPERINE_NGSPICE` is set to `skip` the system SHALL still build, elaborate and solve the MSL side of each twin, and SHALL fail when that side fails. (PAR-04, AC 4; T5)
Proof: run bench | test a_skipped_reference_still_solves_the_msl_side ... ok

**C18** - The system SHALL define the contracts `DC_VOLTAGE`, `DC_CURRENT`, `AC_MAGNITUDE`, `AC_PHASE` and `TRAN_VOLTAGE`, each with a relative bound, an absolute bound and a rationale text. (PAR-05, AC 1; T2)
Proof: run lib | test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok

**C19** - The system SHALL accept a pair when `|golden − actual| <= abstol + reltol · max(|golden|, |actual|)` and reject it otherwise. (PAR-05, AC 2; T2)
Proof: run lib | test contract::tests::holds_is_abstol_plus_reltol_of_the_larger ... ok

**C20** - WHEN a pair is rejected THEN the system SHALL fail with the fixture, the quantity, the sweep value, time or frequency of the point, both values, the bound and the contract's rationale. (PAR-05, AC 3; T2)
Proof: run lib | test contract::tests::a_rejection_names_point_values_bound_and_rationale ... ok

**C21** - The system SHALL compare a phase only at points where the golden magnitude is above the absolute bound of `AC_MAGNITUDE`. (PAR-05, AC 4; T2)
Proof: run lib | test contract::tests::phase_is_gated_on_the_magnitude_floor ... ok

**C22** - The system SHALL hold no numeric tolerance outside the contract's definition: no parity test target compares a difference with a literal. (PAR-05, AC 5; T10)
Proof: run corpus | test no_parity_target_compares_against_a_literal ... ok

**C23** - WHEN the test `divider` runs THEN the system SHALL compare `v(vin)` and `v(mid)` of a `Dc` solve with ngspice's operating point under `DC_VOLTAGE`. (PAR-06, AC 1; T6)
Proof: run op | test divider ... ok

**C24** - WHEN the test `diode_series` runs THEN the system SHALL compare `v(a)` and `v(b)` of a `Dc` solve with ngspice's operating point under `DC_VOLTAGE`. (PAR-06, AC 2; T6)
Proof: run op | test diode_series ... ok

**C25** - WHEN the test `diode_iv` runs THEN the system SHALL set `v1.dc` to each of the 37 sweep values of the export, solve `Dc` at each, and compare the flow of `v1` with ngspice's `i(v1)` under `DC_CURRENT`. (PAR-06, AC 3; T7)
Proof: run dc | test diode_iv ... ok

**C26** - WHEN the test `ac_rc` runs THEN the system SHALL run an `Ac` sweep over the 41 frequencies of the export and compare the magnitude of `v(out)` under `AC_MAGNITUDE` and its phase under `AC_PHASE`. (PAR-06, AC 4; T8)
Proof: run ac | test ac_rc ... ok

**C27** - IF the solver's frequency or sweep grid differs from the export's by more than 1e-9 relative at any point THEN the system SHALL fail naming the point. (PAR-06, AC 5; T4)
Proof: run lib | test golden::tests::a_grid_mismatch_names_the_point ... ok

**C28** - WHEN the test `tran_rc` runs THEN the system SHALL run a transient to 10 µs, interpolate `v(out)` linearly at each of the 100 times of the export, and compare under `TRAN_VOLTAGE`. (PAR-06, AC 6; T9)
Proof: run tran | test tran_rc ... ok

**C29** - The system SHALL run the transient with a maximum step of one tenth of the export's time step, so that the interpolation error stays below a tenth of the contract's bound. (PAR-06, AC 7; T9)
Proof: run tran | test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok

**C30** - IF a `<name>.msl` under `tests/ngspice/` other than `devices.msl` has no `<name>.cir`, or no test function named `<name>` in a parity target, THEN the guard SHALL fail naming the file. (PAR-07, AC 1; T10)
Proof: run corpus | test every_twin_has_a_cir_and_a_test ... ok

**C31** - IF a parity test target contains a comparison of an absolute difference with a numeric literal THEN the guard SHALL fail naming the file and the line. (PAR-07, AC 2; T10)
Proof: run corpus | test a_literal_tolerance_in_a_parity_target_is_reported ... ok

**C32** - WHEN the guard's two checks are given text written in the test that breaks each rule THEN each SHALL report it. (PAR-07, AC 3; T10)
Proof: run corpus | test the_guards_report_planted_text ... ok

**C33** - The system SHALL hold `tests/ngspice/isrc_r.cir` (a 1 mA current source into 1 kΩ, operating point, `print v(a)`) and its twin `isrc_r.msl`. (PAR-08, AC 1; T1)
Proof: run twins | test the_current_source_twin_exists ... ok

**C34** - WHEN the test `isrc_r` runs THEN the system SHALL compare `v(a)` with ngspice's under `DC_VOLTAGE`. (PAR-08, AC 2; T6)
Proof: run op | test isrc_r ... ok

**C35** - WHEN the golden value and the actual value are both exactly 0 THEN the system SHALL accept the pair. (PAR edge case 1; T2)
Proof: run lib | test contract::tests::two_zeros_are_accepted ... ok

**C36** - IF an export has a number of rows different from the one the test expects (37, 41, 100) THEN the system SHALL fail naming the fixture and both counts. (PAR edge case 2; T4)
Proof: run lib | test golden::tests::a_wrong_row_count_names_both_counts ... ok

**C37** - IF a time of the export lies outside the solver's transient result THEN the system SHALL fail naming the time; it SHALL not extrapolate. (PAR edge case 3; T9)
Proof: run lib | test waveform::tests::a_time_outside_the_result_is_none ... ok

**C38** - WHEN two parity tests run at the same time THEN each SHALL use its own scratch directory. (PAR edge case 4; T3)
Proof: run harness | test parallel_runs_do_not_share_a_scratch_directory ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** 2026-11-15 batch 1 (T1-T4, Sonnet) in ../piperine-msl-parity
**Walked:** not applicable - the feature puts no screen in front of a user; each story's Independent Test is a test named in `## Checks`

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1 (Sonnet):  T1 → T2 → T3 → T4
Phase 2 (Sonnet):  T5 → T6 → T7 → T8 → T9 → T10
```

Batches: B1 = Phase 1 (4 tasks, Sonnet, wave 1); B2 = Phase 2 (6 tasks, Sonnet, wave 3). The Verifier (Opus) runs after B2, with ngspice installed.

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: The crate and the corpus test | 1 crate skeleton + 1 corpus target | ✅ Granular |
| T2: ToleranceContract | 1 contract type | ✅ Granular |
| T3: Running ngspice | 1 process runner | ✅ Granular |
| T4: Golden parsers | 1 parser | ✅ Granular |
| T5: Bench | 1 loader | ✅ Granular |
| T6: Operating-point parity | 1 test target | ✅ Granular |
| T7: DC sweep parity | 1 test target | ✅ Granular |
| T8: AC parity | 1 test target | ✅ Granular |
| T9: Transient parity and Waveform | 1 interpolator + 1 test target | ✅ Granular |
| T10: The corpus guard | 1 guard target | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of its phase | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | None | first of its phase | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: The crate and the corpus test | The corpus files | unit | unit | ✅ OK |
| T2: ToleranceContract | `ToleranceContract` | unit | unit | ✅ OK |
| T3: Running ngspice | Running ngspice; `Ngspice` lookup | unit | unit | ✅ OK |
| T4: Golden parsers | `Golden` parsers | unit | unit | ✅ OK |
| T5: Bench | `Bench`: the chain | integration | integration | ✅ OK |
| T6: Operating-point parity | Fixture comparisons | e2e | e2e | ✅ OK |
| T7: DC sweep parity | Fixture comparisons | e2e | e2e | ✅ OK |
| T8: AC parity | Fixture comparisons | e2e | e2e | ✅ OK |
| T9: Transient parity and Waveform | Fixture comparisons; `Waveform` | e2e | e2e | ✅ OK |
| T10: The corpus guard | Corpus guard | unit | unit | ✅ OK |

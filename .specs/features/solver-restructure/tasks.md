# Solver Restructure Tasks

**Approved**: 2026-11-14 - user: "Approve and run (Recommended)"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-restructure/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run solver**: `cargo test -p piperine-solver`
**Run analog**: `cargo test -p piperine-analog-solver`
**Run digital**: `cargo test -p piperine-digital-solver`
**Run hygiene**: `cargo test -p piperine-lang --test suite_hygiene`

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-solver-completeness`, branch `feature/solver-completeness`. Do not set `CARGO_TARGET_DIR`. Never pass `--workspace` and never run `cargo clean`: five kept crates do not compile (`piperine-project`, `piperine-cli`, `piperine-lang-server`, `piperine-api`, the root `piperine` shell) and are expected to stay that way through this feature. Do not push.

Rules every task follows:

- **No logic change.** A task moves, renames a path, or deletes. It never edits the body of a solver algorithm. The one permitted shape change per moved item is listed in `design.md` (generic scheduler, `StepLimits`, `DigitalLimits`, `DigitalError`). Anything else that seems needed: stop and report.
- **Tests are only moved, ported or removed by name.** A test function is removed only when every assertion in it is about an item this feature removes; its name goes into `.specs/features/solver-restructure/removed.txt` (one bare function name per line) in the same commit, and the commit body lists it. No assertion line is added to or changed in a test file that existed at e129b869, except in the five ported targets, where the assertions stay and only the circuit construction changes.
- **A ported test keeps its name, its asserted quantities and its tolerances.** If a hand-written element cannot meet a tolerance, the element is wrong: fix the element, never the tolerance.
- A new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Test functions have exactly the names the checks cite.
- No macro for tables or helpers: data and plain functions.
- Zero warnings on every crate a task touches that is expected green: `piperine-lang`, `piperine-solver`, and from Phase 4 `piperine-analog-solver`, `piperine-digital-solver`.
- Commit message: `refactor(solver): ...`, `chore(workspace): ...`, `test(solver): ...` or `docs: ...`, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("zero warnings is the bar"; "Where the tests are"), `AGENTS.md` ("Test placement"), `.specs/STATE.md` MD-28 (a target lives in the crate it exercises), MD-31 (an invariant lives in a gate), MD-33 (no file-scope lint suppression).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver owner crate (`core/`, `analyses/`, facade) | integration | every baseline test not announced as removed still passes; ported targets keep their 15 names and assertions | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Analog sub-solver crate | unit | every unit test of the modules it receives passes there, same assertions | `crates/piperine-analog-solver/src/**` (`#[cfg(test)]`) | `cargo test -p piperine-analog-solver` |
| Digital sub-solver crate | unit | every unit test of the modules it receives passes there, same assertions | `crates/piperine-digital-solver/src/**` (`#[cfg(test)]`) | `cargo test -p piperine-digital-solver` |
| Test element library | integration | each element checked against an analytic value; each `param_stamps` against a central difference | `crates/piperine-solver/tests/helper_devices.rs` | `cargo test -p piperine-solver --test helper_devices` |
| Structural guards (manifests, removed names, crate boundary, instruction files) | integration | one test per spec criterion of SR-02, SR-03, SR-04, SR-06 | `crates/piperine-solver/tests/{workspace_shape,removed_surface,crate_boundaries}.rs` | `cargo test -p piperine-solver --test <target>` |
| The hygiene guard | integration | its 11 tests pass from `piperine-lang`, plus one test that the walk reaches the solver crates | `crates/piperine-lang/tests/suite_hygiene.rs` | `cargo test -p piperine-lang --test suite_hygiene` |
| Workspace manifests, CI workflows, deleted files, `CLAUDE.md`, `AGENTS.md`, `.specs/STATE.md` | integration | covered by the guard test the same task adds to `workspace_shape.rs` | `crates/piperine-solver/tests/workspace_shape.rs` | `cargo test -p piperine-solver --test workspace_shape` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Phase 1 tasks before the last port (the unported targets do not build yet) | `cargo test -p piperine-solver --lib` plus `cargo test -p piperine-solver --test <each target the task names>` |
| Full | every task from T7 on | `cargo test -p piperine-solver`; from T19 also `cargo test -p piperine-analog-solver`; from T22 also `cargo test -p piperine-digital-solver` |
| Build | last task of each phase | Full, plus `cargo build --all-targets` with zero warnings on `piperine-lang` and every solver crate that exists, plus `cargo metadata --no-deps --format-version 1 > /dev/null` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Five batches, one per phase (7, 5, 6, 6, 3 tasks).

### Phase 1: The suite runs alone

```
T1 → T2 → T3 → T4 → T5 → T6 → T7
```

### Phase 2: Retired crates and the VAMS suite

```
T8 → T9 → T10 → T11 → T12
```

### Phase 3: Safe surplus

```
T13 → T14 → T15 → T16 → T17 → T18
```

### Phase 4: Three crates

```
T19 → T20 → T21 → T22 → T23 → T24
```

### Phase 5: Instructions, decision and the hygiene guard

```
T25 → T26 → T27
```

---

## Task Breakdown

### T1: Drop the in-tree dev-dependencies and record the baseline

**What**: Remove `piperine-lang` and `piperine-codegen` from the solver's `[dev-dependencies]`, remove `rayon` and `rand_distr` from `[dependencies]`, and record which tests pass.
**Where**: `crates/piperine-solver/Cargo.toml` (+ new `tests/workspace_shape.rs`, new `.specs/features/solver-restructure/baseline.txt` and empty `removed.txt`)
**Depends on**: None
**Reuses**: nothing
**Requirement**: SR-01

**Done when**:

- [x] `[dev-dependencies]` holds only `tempfile`; `rayon` and `rand_distr` are gone; `piperine-ir` stays until T8
- [x] `cargo test -p piperine-solver --lib` passes, and so does `cargo test -p piperine-solver --test <t>` for each of the 34 targets other than `sens`, `adjoint_dc`, `adjoint_vs_sens`, `transient_reentry`, `pss`
- [x] **If any of those tests fails: stop here, change nothing else, report the test and its output** (SR-01 AC 5)
- [x] `baseline.txt` holds the bare function name of every test that printed `... ok`, sorted, one per line, plus the 15 names of the five unported targets (they are baseline by contract); `removed.txt` exists and is empty
- [x] New target `workspace_shape.rs` with a `//!` header, a helper `repo_root()` (`CARGO_MANIFEST_DIR/../..`) and the test `solver_manifest_has_no_in_tree_dev_dependency` (C1), which also asserts the absence of `rayon` and `rand_distr`; from T8 on it also asserts the absence of `piperine-ir`
- [x] Gate: quick, on `--lib` and the 34 targets plus `workspace_shape`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): drop in-tree dev-dependencies and record the baseline`

---

### T2: Hand-written test element library

**What**: Analog and mixed `Element`s that reproduce, constant for constant, the Verilog-A modules of the five targets to port.
**Where**: `crates/piperine-solver/tests/helpers/devices.rs` (+ `mod devices;` in `helpers/mod.rs`, new target `tests/helper_devices.rs`)
**Depends on**: T1
**Reuses**: `tests/helpers/mod.rs` (`D2ADevice` pattern), stamping in `tests/parity_baseline.rs` and `tests/live_params.rs`
**Requirement**: SR-01

**Done when**:

- [x] Elements exist for every module in the five targets: `R`, `C`, `D`, `Vsrc`, `Vsine`, `Vramp`, `ROpt`, `ClockSrc`, `Comparator`, `ToggleDff`; each quotes the Verilog-A text it replaces in a comment and exposes the params the tests read or set (`list_params`, `get_param`, `set_param` with the right `Invalidation`)
- [x] `param_stamps` is implemented, with `HAS_SENSITIVITY`, on every element whose parameter an adjoint test differentiates analytically
- [x] Test `divider_reads_the_analytic_ratio` passes (C4)
- [x] Test `rc_step_reads_one_minus_exp` passes (C5)
- [x] Test `diode_satisfies_its_own_equation` passes (C6)
- [x] Test `param_stamps_match_a_central_difference` passes for every element that implements `param_stamps`
- [x] No existing helper is changed
- [x] Gate: quick, on `helper_devices`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): hand-written element library for the ported targets`

---

### T3: Port `sens.rs`

**What**: Build the circuits of `sens.rs` from the element library instead of compiling Verilog-A.
**Where**: `crates/piperine-solver/tests/sens.rs`
**Depends on**: T2
**Reuses**: `tests/helpers/devices.rs`
**Requirement**: SR-01

**Done when**:

- [x] No `piperine_lang` or `piperine_codegen` import remains; `headers_source_map` and the Verilog-A string are gone
- [x] The 4 tests keep their names; every `assert` line is unchanged except for the expression that names the circuit or device handle
- [x] `divider_sensitivities_match_analytic_values`, `diode_sensitivity_is_step_independent_and_signed`, `sens_error_paths_are_loud`, `sens_rebuild_class_param_fails_loud` pass
- [x] Gate: quick, on `sens`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): port sens to hand-written elements`

---

### T4: Port `adjoint_dc.rs`

**What**: Same port for the DC adjoint target.
**Where**: `crates/piperine-solver/tests/adjoint_dc.rs`
**Depends on**: T3
**Reuses**: `tests/helpers/devices.rs`, the circuit builders written in T3
**Requirement**: SR-01

**Done when**:

- [x] No frontend import remains
- [x] `one_transpose_solve_serves_every_requested_parameter`, `df_dx_is_built_for_a_scalar_output_from_the_converged_dc_solution`, `adjoint_runs_on_a_nonlinear_circuit` keep their names and assertions and pass
- [x] Gate: quick, on `adjoint_dc`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): port adjoint_dc to hand-written elements`

---

### T5: Port `adjoint_vs_sens.rs`

**What**: Same port for the adjoint-against-`.sens` oracle.
**Where**: `crates/piperine-solver/tests/adjoint_vs_sens.rs`
**Depends on**: T4
**Reuses**: `tests/helpers/devices.rs`
**Requirement**: SR-01

**Done when**:

- [x] No frontend import remains
- [x] `adjoint_matches_sens_on_the_linear_divider`, `adjoint_and_sens_agree_on_zero_for_a_dc_invisible_parameter`, `adjoint_matches_sens_on_a_nonlinear_diode_circuit`, `a_20_parameter_gradient_costs_exactly_one_extra_solve` keep their names and assertions and pass
- [x] Gate: quick, on `adjoint_vs_sens`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): port adjoint_vs_sens to hand-written elements`

---

### T6: Port `transient_reentry.rs`

**What**: Same port for the full-state re-entry target.
**Where**: `crates/piperine-solver/tests/transient_reentry.rs`
**Depends on**: T5
**Reuses**: `tests/helpers/devices.rs`
**Requirement**: SR-01

**Done when**:

- [x] No frontend import remains
- [x] `reentry_from_captured_state_matches_continuous_run` keeps its name and assertions and passes
- [x] Gate: quick, on `transient_reentry`

**Tests**: integration
**Gate**: quick
**Commit**: `test(solver): port transient_reentry to hand-written elements`

---

### T7: Port `pss.rs`; the whole suite is green

**What**: Same port for the periodic-steady-state target, mixed-signal divider included; first full green run.
**Where**: `crates/piperine-solver/tests/pss.rs`
**Depends on**: T6
**Reuses**: `tests/helpers/devices.rs` (`ClockSrc`, `Comparator`, `ToggleDff`), `tests/helpers/mod.rs`
**Requirement**: SR-01, SR-05

**Done when**:

- [x] No frontend import remains anywhere under `crates/piperine-solver/`
- [x] `sine_rc_converges_to_the_analytic_phasor`, `non_periodic_circuit_fails_loud`, `digital_divider_reports_k_times_period` keep their names and assertions and pass
- [x] `cargo test -p piperine-solver` builds every target and passes (C2); the 15 ported names all print `... ok` (C3)
- [x] Every name of `baseline.txt` prints `... ok`
- [x] `cargo build -p piperine-solver --all-targets` prints no warning
- [x] Gate: build

**Tests**: integration
**Gate**: build
**Commit**: `test(solver): port pss to hand-written elements`

---

### T8: Remove `piperine-codegen` and `piperine-ir`

**What**: Delete both crates and every manifest line that names them; the solver keeps the one function it used.
**Where**: `Cargo.toml` (root: members, dev-dependencies, the seven `[profile.dev.package.*]` Cranelift entries; + `crates/piperine-codegen/`, `crates/piperine-ir/` deleted; dependency lines in `crates/piperine-api/Cargo.toml`, `crates/piperine-cli/Cargo.toml`, `crates/piperine-solver/Cargo.toml`)
**Depends on**: None (phase order: starts after T7 closes the previous phase)
**Reuses**: `crates/piperine-ir/src/events.rs` (`CrossDir`, `Prediction`, `predict_crossing` and its two tests, copied verbatim into `crates/piperine-solver/src/digital/interface.rs` before the crate is deleted)
**Requirement**: SR-02, SR-01

**Done when**:

- [x] Both directories are gone; no `Cargo.toml` names either crate
- [x] `predict_crossing_interpolates_a_linear_bracket_exactly` and `predict_crossing_rejects_a_non_bracketing_pair` pass inside the solver; `interpolate_a2d_crossing` behaves as before (its two existing tests pass untouched)
- [x] `cargo metadata --no-deps --format-version 1 > /dev/null` succeeds (C9)
- [x] `workspace_shape.rs` gains `retired_crates_are_gone` and `no_manifest_names_a_retired_crate`, each over the list of crates removed so far (the list is one `const` the next task extends); `no_manifest_names_a_retired_crate` also asserts that the root manifest has no `[profile.dev.package.cranelift` entry
- [x] `solver_manifest_has_no_in_tree_dev_dependency` now also rejects `piperine-ir`
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `chore(workspace): remove piperine-codegen and piperine-ir`

---

### T9: Remove `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`

**What**: Delete the three crates and every manifest line that names them.
**Where**: `Cargo.toml` (root: members, dev-dependencies incl. the `pip` alias; + the three crate directories deleted; dependency lines in `crates/piperine-cli/Cargo.toml`)
**Depends on**: T8
**Reuses**: the `const` list of `workspace_shape.rs`
**Requirement**: SR-02

**Done when**:

- [x] The three directories are gone; `ls crates` shows six directories (eight after Phase 4)
- [x] `retired_crates_are_gone` (C7) and `no_manifest_names_a_retired_crate` (C8) pass over all five names
- [x] `cargo metadata --no-deps --format-version 1 > /dev/null` succeeds
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `chore(workspace): remove piperine-python, piperine-plugin and piperine-plugin-macros`

---

### T10: Delete the root package's VAMS suite

**What**: Delete every root test target except the hygiene guard, with its support code and Verilog-AMS fixtures. The guard stays in place until T27 moves it.
**Where**: `tests/` (root: every `*.rs` except `suite_hygiene.rs`; everything under `tests/support/` except `sources.rs`, which the guard includes; `tests/spice/`; every `*.vams` under `tests/`)
**Depends on**: T9
**Reuses**: nothing
**Requirement**: SR-02

**Done when**:

- [x] `tests/*.rs` is exactly `tests/suite_hygiene.rs`, untouched; `tests/support/` holds only `sources.rs`, untouched; no `.vams` file exists under `tests/`; `tests/ngspice/*.cir` are all still there
- [x] `workspace_shape.rs` gains `root_tests_hold_no_end_to_end_target` (C10): no `.vams` under `tests/`, at least one `.cir` under `tests/ngspice/`, and no `.rs` directly under `tests/` other than `suite_hygiene.rs` (T27 tightens this to none)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `chore(workspace): delete the root VAMS end-to-end suite`

---

### T11: Delete the Verilog-AMS and Python examples

**What**: Delete the Verilog-AMS example sources, their Python drivers and the plots.
**Where**: `examples/` (every `*.vams`, `*.py`, `*.png`, and `requirements.txt`)
**Depends on**: T10
**Reuses**: nothing
**Requirement**: SR-02

**Done when**:

- [x] No `.vams`, `.py` or `.png` file and no `requirements.txt` exists under `examples/`; `Piperine.toml` and `Piperine.lock` are untouched
- [x] `workspace_shape.rs` gains `examples_hold_no_vams_or_python` (C11)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `chore(workspace): delete the Verilog-AMS and Python examples`

---

### T12: `default-members` and CI

**What**: Make a bare `cargo` command at the root cover the green crates, and stop CI from building removed crates.
**Where**: `Cargo.toml` (root: `default-members`; + `.github/workflows/ci.yml`, `.github/workflows/release.yml`)
**Depends on**: T11
**Reuses**: nothing
**Requirement**: SR-02

**Done when**:

- [x] `default-members = ["crates/piperine-lang", "crates/piperine-solver"]` (T19 and T22 add the two sub-solver crates)
- [x] `ci.yml` builds and tests with no `--workspace` (it runs the default members) and installs no Python requirements; `release.yml` has no step that names `piperine-python`
- [x] `workspace_shape.rs` gains `default_members_are_the_green_crates` (C12), whose expected list is one `const`, and `workflows_name_no_retired_crate` (C13), which also rejects `--workspace` in `ci.yml`
- [x] `cargo build` and `cargo test` with no `-p`, at the root, are green
- [x] Gate: build

**Tests**: integration
**Gate**: build
**Commit**: `chore(workspace): default-members and CI cover the green crates`

---

### T13: Remove distortion

**What**: Delete the `.disto` analysis and its `Element` hooks.
**Where**: `crates/piperine-solver/src/analyses/disto.rs` (deleted; + the items of the design's removal map row "Distortion"; new target `tests/removed_surface.rs`)
**Depends on**: None (phase order: starts after T12 closes the previous phase)
**Reuses**: `repo_root()` pattern of `workspace_shape.rs`
**Requirement**: SR-03

**Done when**:

- [x] Everything in the removal map row is gone; the three capability bits are left vacant with a one-line comment, no other flag changes value
- [x] Test functions whose every assertion is about `HAS_DISTO2`, `HAS_DISTO3`, `NUMERIC_JACOBIAN` or `disto` (in `capabilities_contract.rs`, `failure_rules.rs`, `validation_gating.rs` or elsewhere) are removed and named in `removed.txt`; the 9 unit tests of `disto.rs` are named there too
- [x] New target `removed_surface.rs` with a `//!` header, a helper `names_found(names: &[&str]) -> Vec<String>` that scans every `.rs` file under `crates/piperine-*solver*/src`, and the tests `disto_is_gone` (C14) and `the_scan_catches_a_planted_name` (C20: the helper's core, run on a string that holds one of the names, reports it)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `refactor(solver): remove the distortion analysis`

---

### T14: Remove text and file output

**What**: Delete `SimOutput`, the sinks, the file tables and `Context.io`.
**Where**: `crates/piperine-solver/src/core/output.rs` (deleted; + the items of the removal map row "Text and file IO")
**Depends on**: T13
**Reuses**: `removed_surface.rs`
**Requirement**: SR-03

**Done when**:

- [x] Everything in the removal map row is gone; `Context`'s hand-written `Debug` and `Default` lose only their `io` lines
- [x] The 20 unit tests of `output.rs` and any test function wholly about `Context.io` are named in `removed.txt`
- [x] Test `text_io_is_gone` passes in `removed_surface.rs` (C15)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `refactor(solver): remove text and file output`

---

### T15: Remove the verification channels

**What**: Delete the validation, margin and coverage channels and the check postures.
**Where**: `crates/piperine-solver/src/core/validation.rs` (deleted, with `core/margin.rs`, `core/coverage.rs`; + the items of the removal map row "Verification channels"; the five test targets `check_posture`, `margin_eval_points`, `validation_findings`, `validation_gating`, `validation_postures` deleted)
**Depends on**: T14
**Reuses**: `removed_surface.rs`
**Requirement**: SR-03

**Done when**:

- [x] Everything in the removal map row is gone; the drivers lose only the calls into the removed channels, in place, with no reordering of what remains
- [x] `AnalysisKind`, `start_analysis`, `finish_analysis` and `SolverStats.limiting` are untouched
- [x] The 19 tests of the five deleted targets, and any other test function wholly about a removed item, are named in `removed.txt`
- [x] Test `verification_channels_are_gone` passes (C16)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `refactor(solver): remove the validation, margin and coverage channels`

---

### T16: Remove `specify` delays and timing checks

**What**: Delete the per-net delay and timing-check tables.
**Where**: `crates/piperine-solver/src/digital/delays.rs` (deleted; + the items of the removal map row "`specify` timing"; the target `specify_timing_check_enforcement` deleted)
**Depends on**: T15
**Reuses**: `removed_surface.rs`
**Requirement**: SR-03

**Done when**:

- [x] Everything in the removal map row is gone; `QueueSink::push` computes `time: self.base_time + delay` (the table was empty for every surviving caller, so `extra` was 0.0)
- [x] The 5 tests of the deleted target and the 2 unit tests of `delays.rs` are named in `removed.txt`
- [x] Test `specify_timing_is_gone` passes (C17)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `refactor(solver): remove specify delays and timing checks`

---

### T17: Remove stratified regions

**What**: Delete the LRM chapter 8 region primitive and the `region` key of `DigitalEvent`.
**Where**: `crates/piperine-solver/src/digital/regions.rs` (deleted; + the items of the removal map row "Regions"; the targets `scheduler_region_queue_parity`, `vams_stratified_regions` deleted; new target `tests/event_order.rs`)
**Depends on**: T16
**Reuses**: `removed_surface.rs`
**Requirement**: SR-03, SR-05

**Done when**:

- [x] Everything in the removal map row is gone; `DigitalEvent::cmp` compares `time` then `seq`; `PartialEq` drops the `region` term only
- [x] In kept test files a `region: Region::Active` field is dropped from `DigitalEvent` literals and nothing else changes; a test function wholly about region ordering is removed and named in `removed.txt`, with the 6 tests of the two deleted targets and the 8 unit tests of `regions.rs`
- [x] New target `event_order.rs` with the test `same_time_events_order_by_seq` (C29): events pushed at one time with `seq` 2, 0, 1 pop as 0, 1, 2; an earlier time pops before a lower `seq`
- [x] Test `regions_are_gone` passes (C18)
- [x] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `refactor(solver): remove stratified event regions`

---

### T18: Remove `simprobe`

**What**: Delete the `$simprobe` helper.
**Where**: `crates/piperine-solver/src/core/introspect.rs` (+ the re-exports in `prelude.rs` and `abi.rs`)
**Depends on**: T17
**Reuses**: `removed_surface.rs`
**Requirement**: SR-03

**Done when**:

- [x] `simprobe` and `SimprobeError` are gone; the unit tests of `introspect.rs` that test only them are named in `removed.txt`
- [x] Test `simprobe_is_gone` passes (C19)
- [x] `cargo test -p piperine-solver` is green; every name of `baseline.txt` not in `removed.txt` prints `... ok`
- [x] `cargo build -p piperine-solver --all-targets` prints no warning
- [x] Gate: build

**Tests**: integration
**Gate**: build
**Commit**: `refactor(solver): remove simprobe`

---

### T19: Create `piperine-analog-solver` with the error type and the linear algebra

**What**: New crate holding `Error`, `SolverDomain`, `Result` and the leaf math modules.
**Where**: `crates/piperine-analog-solver/` (new: `Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/math/{mod, num, linear, circular_array, iv, faer}.rs`; + root `Cargo.toml` members and `default-members`, the owner's `Cargo.toml` dependency)
**Depends on**: None (phase order: starts after T18 closes the previous phase)
**Reuses**: the files moved from `crates/piperine-solver/src/{error.rs, math/}` with `git mv`
**Requirement**: SR-04

**Done when**:

- [x] The six modules live in the new crate, moved with `git mv`, bodies unchanged apart from `use` paths; the `Result` alias moves beside `Error`
- [x] The owner has no `error.rs` and none of the five math files; its internal paths point at `piperine_analog_solver`; `prelude` and `abi` export the same names as before
- [x] The new crate's manifest names no workspace crate and lists only the external crates its modules use
- [x] `cargo test -p piperine-analog-solver` passes with the unit tests of `faer.rs` (4), among them `singular_system_fails_loud_through_the_existing_error_family` (C27)
- [x] The expected list of `default_members_are_the_green_crates` gains the crate
- [x] Gate: full (owner and analog)

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): piperine-analog-solver with the error type and linear algebra`

---

### T20: Move the netlist, the tolerances and the tunables

**What**: `Netlist` naming, `Tolerances`, `Policy` and `config.rs` join the analog crate.
**Where**: `crates/piperine-analog-solver/src/netlist.rs` (+ `src/tolerances.rs`, `src/config.rs`; the owner's `analog.rs`, `analyses/config.rs` and the two structs of `analyses/context.rs` leave)
**Depends on**: T19
**Reuses**: `git mv` of `crates/piperine-solver/src/analog.rs` and `analyses/config.rs`
**Requirement**: SR-04

**Done when**:

- [x] `Netlist::nets()` is a free function `analog_nets(&Netlist)` in the owner's `core/net.rs`, same body; its callers are updated; no other method of `Netlist` changes
- [x] `Tolerances` (with `has_converged`, `residual_test`, `vntol_for`) and `Policy` are byte-for-byte the same code in `tolerances.rs`; `Context`, `AnalysisKind` and `init_global` stay in the owner
- [x] The unit test of `config.rs` (`defaults_equal_the_literals_they_replace`) passes in the analog crate
- [x] `prelude` and `abi` export the same names
- [x] Gate: full (owner and analog)

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): netlist, tolerances and tunables move to the analog crate`

---

### T21: Move Newton, the convergence plan, the integration scheme and the stamp check

**What**: The rest of the numeric engine joins the analog crate; the owner has no `math` module left.
**Where**: `crates/piperine-analog-solver/src/convergence.rs` (+ `src/math/newton_raphson.rs`, `src/math/integration.rs`, `src/stamp_validation.rs`; new owner target `tests/integration_scheme_elements.rs`)
**Depends on**: T20
**Reuses**: `git mv` of the four owner files
**Requirement**: SR-04

**Done when**:

- [x] The stepper strategy takes `StepLimits { dt_min, dt_max }` in place of `&TransientAnalysisOptions`; the owner builds it from the options at each of the call sites; the arithmetic is unchanged
- [x] `PlanLimits` keeps `max_mixed_signal_iter`; its two digital fields stay available to the owner through a struct defined in the owner until T22 moves them (no default value changes: 20, 1000, 1e-12)
- [x] The unit tests of `integration.rs` that build a full `Element` (they import `core::element`) move, bodies unchanged, to the owner target `integration_scheme_elements.rs`; the other unit tests of `integration.rs`, `convergence.rs` (4) and the non-finite stamp test stay with their modules and pass in the analog crate
- [x] The owner has no `src/math/` and no `src/analog.rs`; no `pub mod math` or `pub mod analog` shim exists
- [x] `cargo build --all-targets` prints no warning on the owner and the analog crate
- [x] Gate: full (owner and analog)

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): Newton, convergence plan and integration move to the analog crate`

---

### T22: Create `piperine-digital-solver` with events, drivers and the device interface

**What**: New crate holding the digital value types, the driver table, the sink and the `DigitalDevice` trait.
**Where**: `crates/piperine-digital-solver/` (new: `Cargo.toml`, `src/lib.rs`, `src/{events, drivers, interface, limits, error}.rs`; + root `Cargo.toml` members and `default-members`, the owner's dependency)
**Depends on**: T21
**Reuses**: `git mv` of `crates/piperine-solver/src/digital/{events, drivers, interface}.rs`; `DigitalDevice` cut from `core/element.rs`
**Requirement**: SR-04

**Done when**:

- [x] `DigitalDevice` lives in `interface.rs` of the new crate with the same methods and defaults; `Element: AnalogDevice + DigitalDevice + Introspect` in the owner names it from there
- [x] `trait DigitalParticipant: DigitalDevice { fn is_digital(&self) -> bool; }` exists in the new crate; the owner has `impl DigitalParticipant for dyn Element` returning `self.capabilities().contains(ElementCapabilities::DIGITAL)`
- [x] `DigitalLimits { max_delta_cycles, time_epsilon }` with defaults 1000 and 1e-12, asserted by a unit test `digital_limits_defaults_are_the_literals`
- [x] `DigitalError` with the two variants of the design, `Display` equal to today's message texts
- [x] The mock devices of the unit tests of `events.rs` and `interface.rs` implement `DigitalDevice` (and `DigitalParticipant` where the test runs the scheduler) instead of the full `Element`; their assertions are unchanged; tests that need the scheduler stay in the owner until T23 and move then
- [x] The crate's manifest has no dependency in the tree
- [x] `cargo test -p piperine-digital-solver` passes, among its tests `external_model_emits_through_the_sink` (C28)
- [x] The expected list of `default_members_are_the_green_crates` gains the crate
- [x] Gate: full (owner, analog, digital)

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): piperine-digital-solver with events, drivers and the device interface`

---

### T23: Move the digital state, the topology and the scheduler

**What**: The rest of `digital/` joins the digital crate; the owner has no `digital` module left.
**Where**: `crates/piperine-digital-solver/src/scheduler.rs` (+ `src/state.rs`, `src/topology.rs`)
**Depends on**: T22
**Reuses**: `git mv` of the three owner files
**Requirement**: SR-04, SR-05

**Done when**:

- [x] `evaluate_until_stable`, `evaluate_dag_ordered` and `DigitalTopology::build` are generic over `D: DigitalParticipant + ?Sized` and take `&mut [Box<D>]` / `&[Box<D>]`; the capability test reads `device.is_digital()`; every other line of the three functions is unchanged
- [x] The scheduler takes `DigitalLimits` and returns `Result<(), DigitalError>`; the owner has `impl From<DigitalError> for Error` producing `Error::simple(SolverDomain::Digital, e.to_string())`; the texts asserted by `failure_rules.rs` and `mixed_signal.rs` are unchanged and those targets pass untouched
- [x] The owner has no `src/digital/` and no `pub mod digital` shim; `analyses/events.rs`, `core/net.rs`, `core/circuit.rs`, `core/result.rs` name the digital crate
- [x] The remaining unit tests of the moved modules pass in the digital crate
- [x] `cargo build --all-targets` prints no warning on the three solver crates
- [x] Gate: full (owner, analog, digital)

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): digital state, topology and scheduler move to the digital crate`

---

### T24: Boundary guards

**What**: Tests that keep the three-crate structure true.
**Where**: `crates/piperine-solver/tests/crate_boundaries.rs`
**Depends on**: T23
**Reuses**: `repo_root()` and the scan helper of `removed_surface.rs` (copied; three callers do not earn a shared module)
**Requirement**: SR-04, SR-05

**Done when**:

- [ ] Test `the_three_solver_crates_exist` passes (C21)
- [ ] Test `sub_solvers_depend_on_no_workspace_crate` passes (C22): neither manifest holds `path =` or a `piperine-` dependency in any table
- [ ] Test `owner_depends_on_both_sub_solvers_only` passes (C23)
- [ ] Test `analog_sources_name_no_digital_type` passes (C24): no `.rs` under the analog crate contains `LogicValue`, `DigitalNet`, `DigitalEvent`, `DigitalState`, `DigitalDevice`
- [ ] Test `digital_sources_name_no_analog_type` passes (C25): no `.rs` under the digital crate contains `Stamp`, `Netlist`, `AnalogReference`, `Tolerances`, `NewtonRaphsonSolver`
- [ ] Test `each_sub_solver_holds_its_modules` passes (C26): the files named in the design exist in each crate, and the owner has none of `src/math`, `src/analog.rs`, `src/digital`, `src/error.rs`
- [ ] `removed_surface.rs` still scans all three `src/` trees (its glob already matches the new crates) and passes
- [ ] `abi_surface.rs`, `prelude_surface.rs` and `parity_baseline.rs` are byte-identical to e129b869 and pass (C30, C31)
- [ ] Every name of `baseline.txt` not in `removed.txt` prints `... ok` across the three crates (C32)
- [ ] Gate: build

**Tests**: integration
**Gate**: build
**Commit**: `test(solver): guards for the three-crate boundary`

---

### T25: `CLAUDE.md` and `AGENTS.md` describe the workspace that exists

**What**: Rewrite the project summary, pipeline, build section, crate table and workspace map for the eight kept crates.
**Where**: `CLAUDE.md` (+ `AGENTS.md`)
**Depends on**: None (phase order: starts after T24 closes the previous phase)
**Reuses**: `analysis.md` section 7 of `solver-completeness` (the crate list), `docs/spec/index.md` for what MSL is
**Requirement**: SR-06

**Done when**:

- [ ] The crate table of `CLAUDE.md` and the workspace map of `AGENTS.md` list exactly `piperine-lang`, `piperine-project`, `piperine-solver`, `piperine-analog-solver`, `piperine-digital-solver`, `piperine-api`, `piperine-cli`, `piperine-lang-server`, with the owner-depends-on-both direction stated
- [ ] Both files say which crates compile (`piperine-lang` and the three solver crates) and which do not yet, and give `cargo build` / `cargo test` at the root (default members) as the commands, never `--workspace`
- [ ] The Verilog-AMS pipeline diagram, the "analog device path", the OSDI and PHDL paragraphs and the "files not to edit casually" entries for deleted paths are removed; sections about `piperine-lang` and MSL written by earlier features are left as they are
- [ ] `workspace_shape.rs` gains `instructions_list_the_kept_crates` (C34) and `instructions_have_no_entry_for_a_retired_crate` (C35), which read the crate table of `CLAUDE.md` (the rows of the table under `## Crate responsibilities`) and the fenced block under `## Workspace map` of `AGENTS.md`
- [ ] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `docs: CLAUDE.md and AGENTS.md describe the kept workspace`

---

### T26: Record the decision

**What**: One project decision in `.specs/STATE.md` for the solver's structure and the user's answers.
**Where**: `.specs/STATE.md`
**Depends on**: T25
**Reuses**: `.specs/features/solver-completeness/context.md` (the user's words, rounds 1 and 2)
**Requirement**: SR-06

**Done when**:

- [ ] A new entry `### MD-58: the solver is three crates; events are the only boundary` under "Macro Decisions", with: the three crate names and the dependency direction; the event-only boundary as the target of `solver-event-boundary`; opaque byte carriers; the kept discontinuity algorithm and tolerance mapping with the spec amendments owed; the five removed crates; the user's words quoted for each; status Locked, date 2026-11-14
- [ ] The handoff snapshot gains a short entry for `solver-restructure`
- [ ] `workspace_shape.rs` gains `state_records_the_solver_structure_decision` (C36): the heading exists and its section names the three crates
- [ ] `cargo test` on the three solver crates is green; `cargo build --all-targets` on `piperine-lang` and the three solver crates prints no warning (C33)
- [ ] Gate: full

**Tests**: integration
**Gate**: full
**Commit**: `docs(specs): MD-58 the solver is three crates`

---

### T27: Move the hygiene guard into `piperine-lang`

**What**: Move `suite_hygiene` and its walk helper into a crate that builds, rooted at the repository, enforcing over every kept source.
**Where**: `crates/piperine-lang/tests/suite_hygiene.rs` (+ `crates/piperine-lang/tests/support/sources.rs`; both moved with `git mv` from the root `tests/`)
**Depends on**: T26
**Reuses**: the guard as it is; `crates/piperine-lang/tests/support/` (existing directory)
**Requirement**: SR-07, SR-02

**Done when**:

- [ ] Both files are moved with `git mv`; no `.rs` file remains under the root `tests/`, and `tests/support/` is gone
- [ ] `repo_root()` resolves to the repository root (`CARGO_MANIFEST_DIR/../..`); the walk functions return the same repository-relative paths as before, so no detector and no message changes
- [ ] The 11 test functions exist under their names (C40); no detector function (`is_disabled_marker`, `is_ignore_attribute`, `is_file_scope_allow`, `dead_architecture_identifiers_in`, ...) is edited
- [ ] Registrations and exemptions that name a path under a removed crate or a deleted file are removed; an entry for a solver file that moved is re-pointed to its new path; nothing else in the registries changes (C41)
- [ ] A new test `the_walk_covers_the_three_solver_crates` asserts that the library-source walk returns at least one file under each of `crates/piperine-solver/src`, `crates/piperine-analog-solver/src`, `crates/piperine-digital-solver/src` and `crates/piperine-lang/src` (C42)
- [ ] `cargo test -p piperine-lang --test suite_hygiene` passes (C39). **If a test fails on a kept source for a reason that predates this feature (including the "more than 100 library sources" assertion): stop, change nothing, report the test and its output** (SR-07 AC 6)
- [ ] `root_tests_hold_no_end_to_end_target` in `workspace_shape.rs` is tightened to "no `.rs` file under the root `tests/`", and `workspace_shape.rs` gains `hygiene_guard_lives_in_piperine_lang` (C43)
- [ ] The new guard targets of this feature (`workspace_shape`, `removed_surface`, `crate_boundaries`, `event_order`, `helper_devices`, `integration_scheme_elements`) pass the guard's own rules: `//!` header, no `#[ignore]`, no file-scope `#![allow]`
- [ ] `cargo test` on `piperine-lang` and the three solver crates is green; `cargo build --all-targets` on the four prints no warning
- [ ] Gate: build

**Tests**: integration
**Gate**: build
**Commit**: `test(lang): the suite-hygiene guard moves to piperine-lang`

---

## Checks

**C1** - The solver's manifest lists no workspace crate as a dev-dependency and no `rayon`, `rand_distr` or `piperine-ir` (SR-01, AC 1)
Proof: run solver | test solver_manifest_has_no_in_tree_dev_dependency ... ok

**C2** - Every test target of `piperine-solver` builds and every test passes with no other crate of the tree (SR-01, AC 2)
Proof: `cargo test -p piperine-solver`

**C3** - The 15 test functions of the five ported targets exist under their names and pass (SR-01, AC 3)
Proof: `cargo test -p piperine-solver --test sens --test adjoint_dc --test adjoint_vs_sens --test transient_reentry --test pss 2>&1 | grep -cE '^test (divider_sensitivities_match_analytic_values|diode_sensitivity_is_step_independent_and_signed|sens_error_paths_are_loud|sens_rebuild_class_param_fails_loud|one_transpose_solve_serves_every_requested_parameter|df_dx_is_built_for_a_scalar_output_from_the_converged_dc_solution|adjoint_runs_on_a_nonlinear_circuit|adjoint_matches_sens_on_the_linear_divider|adjoint_and_sens_agree_on_zero_for_a_dc_invisible_parameter|adjoint_matches_sens_on_a_nonlinear_diode_circuit|a_20_parameter_gradient_costs_exactly_one_extra_solve|reentry_from_captured_state_matches_continuous_run|sine_rc_converges_to_the_analytic_phasor|non_periodic_circuit_fails_loud|digital_divider_reports_k_times_period) \.\.\. ok$' | grep -qx 15`

**C4** - A hand-written two-resistor divider reads `vin * r2 / (r1 + r2)` within 1e-9 V (SR-01, AC 4)
Proof: run solver | test divider_reads_the_analytic_ratio ... ok

**C5** - A hand-written RC step reads `1 - exp(-1)` at `t = rc` within 1e-3 (SR-01, AC 4)
Proof: run solver | test rc_step_reads_one_minus_exp ... ok

**C6** - A hand-written diode fed through a resistor satisfies its own current equation within 1e-9 A (SR-01, AC 4)
Proof: run solver | test diode_satisfies_its_own_equation ... ok

**C7** - The five retired crate directories do not exist (SR-02, AC 1)
Proof: run solver | test retired_crates_are_gone ... ok

**C8** - No `Cargo.toml` names a retired crate and the root manifest has no Cranelift profile entry (SR-02, AC 2)
Proof: run solver | test no_manifest_names_a_retired_crate ... ok

**C9** - The workspace manifest loads (SR-02, AC 3)
Proof: `cargo metadata --no-deps --format-version 1 > /dev/null`

**C10** - The root `tests/` holds no `.rs` file and no `.vams`, and keeps the `.cir` oracles (SR-02, AC 4)
Proof: run solver | test root_tests_hold_no_end_to_end_target ... ok

**C11** - `examples/` holds no `.vams`, `.py` or `.png` file and no `requirements.txt` (SR-02, AC 5)
Proof: run solver | test examples_hold_no_vams_or_python ... ok

**C12** - `default-members` is `piperine-lang` plus the three solver crates (SR-02, AC 6)
Proof: run solver | test default_members_are_the_green_crates ... ok

**C13** - No workflow builds or tests a retired crate, and CI does not pass `--workspace` (SR-02, AC 7)
Proof: run solver | test workflows_name_no_retired_crate ... ok

**C14** - No solver source names the distortion analysis, its hooks or its capability flags (SR-03, AC 1)
Proof: run solver | test disto_is_gone ... ok

**C15** - No solver source names the text or file output surface, and `Context` has no `io` field (SR-03, AC 2)
Proof: run solver | test text_io_is_gone ... ok

**C16** - No solver source names a validation, margin or coverage item, a check posture or one of the five removed `Element` methods (SR-03, AC 3)
Proof: run solver | test verification_channels_are_gone ... ok

**C17** - No solver source names the `specify` delay or timing-check surface (SR-03, AC 4)
Proof: run solver | test specify_timing_is_gone ... ok

**C18** - No solver source names a stratified-region item, and `DigitalEvent` has no `region` field (SR-03, AC 5)
Proof: run solver | test regions_are_gone ... ok

**C19** - No solver source names `simprobe` or `SimprobeError` (SR-03, AC 6)
Proof: run solver | test simprobe_is_gone ... ok

**C20** - The guard's scan reports a removed name when a source text holds one (SR-03, AC 7)
Proof: run solver | test the_scan_catches_a_planted_name ... ok

**C21** - The crates `piperine-solver`, `piperine-analog-solver` and `piperine-digital-solver` exist as workspace members (SR-04, AC 1)
Proof: run solver | test the_three_solver_crates_exist ... ok

**C22** - Neither sub-solver manifest lists a workspace crate in any dependency table (SR-04, AC 2)
Proof: run solver | test sub_solvers_depend_on_no_workspace_crate ... ok

**C23** - The owner's manifest depends on both sub-solver crates and on no other workspace crate (SR-04, AC 3)
Proof: run solver | test owner_depends_on_both_sub_solvers_only ... ok

**C24** - No source file of the analog crate contains `LogicValue`, `DigitalNet`, `DigitalEvent`, `DigitalState` or `DigitalDevice` (SR-04, AC 4)
Proof: run solver | test analog_sources_name_no_digital_type ... ok

**C25** - No source file of the digital crate contains `Stamp`, `Netlist`, `AnalogReference`, `Tolerances` or `NewtonRaphsonSolver` (SR-04, AC 5)
Proof: run solver | test digital_sources_name_no_analog_type ... ok

**C26** - Each sub-solver crate holds the modules the design assigns to it and the owner holds none of them (SR-04, AC 6 and AC 7)
Proof: run solver | test each_sub_solver_holds_its_modules ... ok

**C27** - The analog crate builds alone and passes with the unit tests it received, the singular-Jacobian test among them (SR-04, AC 9)
Proof: run analog | test singular_system_fails_loud_through_the_existing_error_family ... ok

**C28** - The digital crate builds alone and passes with the unit tests it received, the sink test among them (SR-04, AC 9)
Proof: run digital | test external_model_emits_through_the_sink ... ok

**C29** - Events at one time pop in `seq` order, and an earlier time pops before a lower `seq` (SR-05, AC 2)
Proof: run solver | test same_time_events_order_by_seq ... ok

**C30** - The two surface targets are byte-identical to e129b869 and pass: every surviving name still comes through `prelude` and `abi` (SR-04, AC 8)
Proof: `git diff --quiet e129b869 -- crates/piperine-solver/tests/abi_surface.rs crates/piperine-solver/tests/prelude_surface.rs && cargo test -p piperine-solver --test abi_surface --test prelude_surface`

**C31** - `parity_baseline.rs` is byte-identical to e129b869 and its tests pass (SR-05, AC 3)
Proof: `git diff --quiet e129b869 -- crates/piperine-solver/tests/parity_baseline.rs && cargo test -p piperine-solver --test parity_baseline`

**C32** - Every baseline test not announced in `removed.txt` passes in one of the three solver crates (SR-05, AC 1)
Proof: `now=$(mktemp); cargo test -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver 2>&1 | grep -E '^test .* \.\.\. ok$' | sed -E 's/^test (.*::)?([A-Za-z0-9_]+) \.\.\. ok$/\2/' | sort -u > "$now"; test -s "$now" && ! comm -23 <(sort -u .specs/features/solver-restructure/baseline.txt) "$now" | grep -vxFf .specs/features/solver-restructure/removed.txt | grep .`

**C33** - `piperine-lang` and the three solver crates build all targets with no warning (SR-05, AC 5)
Proof: `! cargo build -p piperine-lang -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver --all-targets 2>&1 | grep -E '^(warning|error)'`

**C34** - `CLAUDE.md` and `AGENTS.md` list the eight kept crates (SR-06, AC 1)
Proof: run solver | test instructions_list_the_kept_crates ... ok

**C35** - The crate table of `CLAUDE.md` and the workspace map of `AGENTS.md` hold no entry for a retired crate (SR-06, AC 2)
Proof: run solver | test instructions_have_no_entry_for_a_retired_crate ... ok

**C36** - `.specs/STATE.md` records the three-crate decision under `MD-58` (SR-06, AC 3)
Proof: run solver | test state_records_the_solver_structure_decision ... ok

**C37** - No assertion line was added to or changed in a test file that existed at e129b869, outside the five ported targets (SR-05, AC 4)
Proof: `! git diff --diff-filter=M e129b869 -- crates/piperine-solver/tests ':(exclude)crates/piperine-solver/tests/sens.rs' ':(exclude)crates/piperine-solver/tests/adjoint_dc.rs' ':(exclude)crates/piperine-solver/tests/adjoint_vs_sens.rs' ':(exclude)crates/piperine-solver/tests/transient_reentry.rs' ':(exclude)crates/piperine-solver/tests/pss.rs' ':(exclude)crates/piperine-solver/tests/helpers/mod.rs' | grep -E '^\+[^+].*assert'`

**C38** - The baseline of the first green run is recorded, so a red test at that point was visible before any structure moved (SR-01, AC 5)
Proof: `test "$(wc -l < .specs/features/solver-restructure/baseline.txt)" -ge 250`

**C39** - The moved guard passes on the kept sources (SR-07, AC 2)
Proof: `cargo test -p piperine-lang --test suite_hygiene`

**C40** - The moved guard holds the 11 test functions it had at e129b869, by name (SR-07, AC 4)
Proof: `test -z "$(comm -23 <(git show e129b869:tests/suite_hygiene.rs | grep -A1 '^#\[test\]' | grep -oE '^fn [a-z_0-9]+' | sort) <(grep -oE '^fn [a-z_0-9]+' crates/piperine-lang/tests/suite_hygiene.rs | sort))"`

**C41** - The moved guard names no path under a removed crate (SR-07, AC 5)
Proof: `! grep -nE 'crates/piperine-(codegen|ir|python|plugin|plugin-macros)/' crates/piperine-lang/tests/suite_hygiene.rs`

**C42** - The guard's walk starts at the repository root and reaches the three solver crates and `piperine-lang` (SR-07, AC 3)
Proof: run hygiene | test the_walk_covers_the_three_solver_crates ... ok

**C43** - The guard lives in `crates/piperine-lang/tests/` with its helper, and none remains under the root `tests/` (SR-07, AC 1)
Proof: run solver | test hygiene_guard_lives_in_piperine_lang ... ok

**C44** - No detector of the guard was loosened: its detector self-test is unchanged and passes (SR-07, AC 6)
Proof: run hygiene | test the_detectors_recognise_what_they_forbid ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** B1 (T1-T7, Sonnet) at b4b1e768 - returned (T1-T7 218054f7..3767b691; baseline 354 green; 358 pass after ports; helpers via #[path] mod); B2 (T8-T12, Sonnet) at 03d18454 - returned (T8-T12 1cdd920f..c8929613; guards written post-state (no red run); predict_crossing moved into solver); B3 (T13-T18, Sonnet) at f91fb21d - returned (T13-T18 aa0660ff..5bc9006d; 298 pass; removed.txt 76 names; T13 'wholly about' picks by worker; timing-check tests removed in T15 (dep on ValidationFinding)); B4 (T19-T24, Sonnet) at 50cf98eb
**Walked:** not applicable: no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2:  T8 → T9 → T10 → T11 → T12
Phase 3:  T13 → T14 → T15 → T16 → T17 → T18
Phase 4:  T19 → T20 → T21 → T22 → T23 → T24
Phase 5:  T25 → T26 → T27
```

Batches: one per phase, five in all (7, 5, 6, 6, 3 tasks). Batch 1 is the only one that can stop the feature on a pre-existing red test (T1).

## Task Granularity Check

Every task is one move, one removal or one port. The validator's "`Where` names multiple files" warning fires on 15 of them and is accepted: a crate removal or a module move touches the moved files plus the manifest and `use` lines that name them, and cannot compile in two commits. Each `Where` names the primary path first and the dependent edits in parentheses. No task holds two independent deliverables.

## What this feature deletes

| Kind | Deleted |
| --- | --- |
| Crates | `piperine-codegen`, `piperine-ir`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros` |
| Root package | every `tests/*.rs` (the hygiene guard and `support/sources.rs` are moved to `piperine-lang`, not deleted); the rest of `tests/support/`; `tests/spice/`; every `.vams` under `tests/` |
| Examples | `examples/*.vams`, `examples/*.py`, `examples/*.png`, `examples/requirements.txt` |
| Solver source | `analyses/disto.rs`, `core/output.rs`, `core/validation.rs`, `core/margin.rs`, `core/coverage.rs`, `digital/delays.rs`, `digital/regions.rs`, `simprobe`, the `Element` hooks and capability flags that served them, `Context.io`, the check postures |
| Solver test targets | `check_posture`, `margin_eval_points`, `validation_findings`, `validation_gating`, `validation_postures`, `specify_timing_check_enforcement`, `scheduler_region_queue_parity`, `vams_stratified_regions` (30 tests), plus single test functions named in `removed.txt` |
| Dependencies | `rayon`, `rand_distr`, `piperine-ir` (solver); `piperine-lang`, `piperine-codegen` (solver dev); the Cranelift profile entries |

Kept and expected not to compile at the end: `piperine-project`, `piperine-cli`, `piperine-lang-server`, `piperine-api`, the root `piperine` shell.

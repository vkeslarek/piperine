# Solver ABI Analog Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-abi-analog/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-analog-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run analog**: `cargo test -p piperine-analog-solver`

Executors: Sonnet sub-agents by default, one commit per task; verifier: Opus. The orchestrator may have Opus implement instead (a different Opus then verifies): this feature is numerically delicate and not large (10 tasks), the case the user named for it. Work in a worktree of `/home/keslarek/Git/piperine-solver-completeness` branched from the last commit of `solver-abi-core`, in parallel with `solver-abi-digital`: **no file outside `crates/piperine-analog-solver` and `.specs/` is edited**, so the two branches merge without conflict. Never pass `--workspace`, never run `cargo clean`, do not set `CARGO_TARGET_DIR`, do not push.

Rules every task follows:

- **The numeric core is not edited.** No change to `crates/piperine-analog-solver/src/math/`, `convergence.rs`, `tolerances.rs`, `config.rs`, `stamp_validation.rs`, `netlist.rs`. Only `lib.rs` (`pub mod device;`) and `Cargo.toml` (the dependency) change among existing files. If a task seems to need more, stop and report.
- **Expected stamps are computed in the test**, by hand or from `TrBdf2::stage_coeffs`/`phase_coeffs`, never by calling the code under test. Analytic circuits are solved with the real `NewtonRaphsonSolver` through a small `NonLinearSystem` in `tests/support/`.
- **Numerically sensitive tasks** (`Numerically sensitive: yes`) state in the commit body the analytic values reached and the tolerance; a tolerance looser than the one named in the task is a failed task.
- **abi.md is the contract**; rule names (A, F) are its own. **No macro** for devices, tables or helpers. **No `unsafe`**: devices are called through `piperine_abi::caller::Handle`.
- Tests derive from the spec's criteria and abi.md's rules. Test functions have exactly the names the checks cite. New tests go in integration targets under `crates/piperine-analog-solver/tests/`, each opening with a `//!` scope header, no `#[ignore]`, no file-scope `#![allow]`; the library's own unit-test count stays 24.
- Zero warnings on `piperine-analog-solver`.
- Commit message as given per task, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("zero warnings is the bar"; the numeric oracles), `.specs/STATE.md` MD-28, MD-31, MD-33, MD-60, MD-61; user: unit tests only; "a paridade com ngspice é um strenght bem alto".

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Analog assembly (`src/device/*`) | unit | each AC of the spec; stamps asserted against values computed in the test; analytic circuits solved with the real Newton engine | `crates/piperine-analog-solver/tests/{plan,assembly_*}.rs` | `cargo test -p piperine-analog-solver` |
| Fixture devices and the test `NonLinearSystem` | unit | every fixture shape passes `validate`; every function passes `verify_partials` | `crates/piperine-analog-solver/tests/support/`, `tests/plan.rs` | `cargo test -p piperine-analog-solver --test plan` |
| The numeric core | unit | its 24 unit tests, unchanged | `crates/piperine-analog-solver/src/**` (`#[cfg(test)]`) | `cargo test -p piperine-analog-solver --lib` |
| Boundary guard of the crate | unit | dependency, names, `unsafe`, untouched files | `crates/piperine-analog-solver/tests/boundary.rs` | `cargo test -p piperine-analog-solver --test boundary` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | never alone | `cargo test -p piperine-analog-solver --test <the task's target>` |
| Full | every task | `cargo test -p piperine-analog-solver` (24 old tests plus the new targets) |
| Build | last task of each phase | Full, plus `cargo test -p piperine-solver` unedited (nothing may move), plus the two `**Build**` commands, plus `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Phases are ordered and run sequentially. Two batches, one per phase (6 and 4 tasks). Sensitive tasks: T3, T4, T5, T7, T8.

### Phase 1: Rows: plan, evaluation, the three operators, limiting

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Small signal, watches and the guard

```
T7 → T8 → T9 → T10
```

---

## Task Breakdown

### T1: The analog plan and the matrix pattern

**What**: `AnalogPlan::bind`: analog functions, cell sources and sinks, slots for terms, inputs and watches, the pattern as terms × partials.
**Where**: `crates/piperine-analog-solver/src/device/plan.rs` (+ `Cargo.toml` dependency on `piperine-abi`, `src/lib.rs` `pub mod device`, target `tests/plan.rs`, fixture devices in `tests/support/devices.rs`)
**Depends on**: None
**Reuses**: design `device::plan`; `piperine_abi::{validate, caller::Handle}`; the fixtures of `crates/piperine-abi/tests/support/examples.rs` (resistor, capacitor, diode, DAC) copied
**Requirement**: SAN-01, SAN-02
**Numerically sensitive**: no

**Done when**:

- [x] `piperine-analog-solver` depends on `piperine-abi` and on nothing else in the tree; `cargo test -p piperine-solver --test crate_boundaries` passes unedited (`sub_solvers_depend_on_the_abi_crate_only`)
- [x] Test `the_pattern_is_terms_times_partials` passes: the diode's pattern holds its four node positions whatever the bias, and a partial that is zero at the first evaluation keeps its position (C13)
- [x] Test `a_discrete_function_is_not_taken` and test `a_term_to_ground_has_one_row` pass
- [x] Test `an_analog_bind_error_names_its_entry` passes for a term target bound to a discrete net and for a limit written with a term and no `OWNS_CORRECTION`
- [x] Test `every_fixture_function_passes_the_partials_verifier` and test `every_fixture_shape_is_valid` pass in `tests/plan.rs`
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): analog plan and matrix pattern from terms and partials`

---

### T2: Evaluating analog functions

**What**: `Assembly` evaluation: frames, read sources, ambient inputs, the dirty rule F3, the non-finite error A4.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs` (+ target `tests/assembly_eval.rs`)
**Depends on**: T1
**Reuses**: design `device::assembly` (Evaluation); `stamp_validation.rs` for the error's domain and wording family
**Requirement**: SAN-06, SAN-07, SAN-11, SAN-12
**Numerically sensitive**: no

**Done when**:

- [x] Test `a_clean_function_is_not_called` passes: a current source that reads only a param is called once across ten iterations; the resistor is called again when a pin moves (C11)
- [x] Test `setting_a_discrete_slot_makes_its_readers_dirty` passes (C21)
- [x] Test `a_non_finite_value_names_instance_cell_and_origin` passes for a value and for a seeded partial (C12)
- [x] Test `ambient_inputs_carry_the_solve_coordinates` passes for `TIME`, `FREQ`, `STATIC`, `GMIN`, `SOURCE_SCALE` (C20)
- [x] Partials against params are not seeded in a Newton solve (a counting device asserts the seed bytes)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): analog function evaluation with the run-on-change rule`

---

### T3: `FLOW` terms

**What**: Norton stamps of a `FLOW` term and `assemble_dc`.
**Where**: `crates/piperine-analog-solver/src/device/flow.rs` (+ target `tests/assembly_flow.rs`)
**Depends on**: T2
**Reuses**: design `FLOW`; `crates/piperine-solver/tests/helpers/devices.rs` (`D::norton`, `conductance`, `branch_force`) as the stamp shapes to reproduce
**Requirement**: SAN-02
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_flow_term_stamps_its_norton_form` asserts the four matrix entries and two right-side entries of the diode at a hand-computed bias (C2)
- [x] Test `a_potential_source_stamps_through_its_private_pin` asserts the voltage source of abi.md A1 gives the same five positions and the right side `V` that `branch_force` gives today
- [x] Test `a_divider_solves_to_its_ratio` drives `NewtonRaphsonSolver` with `assemble_dc` on a source and two resistors and reads 7.5 V within 1e-12
- [x] Test `a_diode_satisfies_its_own_equation` passes with the tolerance of `helper_devices.rs`
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): FLOW terms as Norton stamps`

---

### T4: `DDT` terms: the companion and its history

**What**: The solver-owned TR-BDF2 companion over a per-term history ring, with `push_stage`, `snapshot`, `restore`.
**Where**: `crates/piperine-analog-solver/src/device/ddt.rs` (+ target `tests/assembly_ddt.rs`)
**Depends on**: T3
**Reuses**: design `DDT`; `math/integration.rs` (`stage_coeffs`, `phase_coeffs`); `crates/piperine-solver/tests/helpers/devices.rs` `impl AnalogDevice for C` (`load_transient`) as the expression to reproduce
**Requirement**: SAN-03
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_ddt_term_is_silent_in_an_equilibrium_solve` passes (C3)
- [x] Test `a_ddt_term_stamps_the_capacitor_companion` asserts conductance and right side for both phases, with `prev_h = 0` and `prev_h > 0`, against values computed in the test from `stage_coeffs` and `phase_coeffs` on a four-row history (C4)
- [x] Test `an_rc_step_reads_one_minus_exp` runs two TR-BDF2 stages per step by hand over `assemble_tran` and meets the tolerance of `helper_devices.rs::rc_step_reads_one_minus_exp`
- [x] Test `a_restore_returns_histories_and_limit_state` passes for the `DDT` half (C5)
- [x] `h <= 0` contributes nothing, as `C::load_transient` does
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): solver-owned ddt companion and term history`

---

### T5: `LIMIT` inputs

**What**: The held iterate, `LAST` and `AGE`, the correction of abi.md A2, the veto, `OWNS_CORRECTION`, the unevaluated limit of A3.
**Where**: `crates/piperine-analog-solver/src/device/limit.rs` (+ target `tests/assembly_limit.rs`)
**Depends on**: T4
**Reuses**: design `LIMIT`; abi.md §4.2, §4.3, A2, A3, §11.2 (the diode with `pnjlim`); Appendix C.2 "limiting"
**Requirement**: SAN-04
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_limit_reader_is_linearized_at_the_held_iterate` asserts Jacobian `g·p` and the right side for a hand-computed `x`, `x̂` (C6)
- [x] Test `an_active_limit_vetoes_convergence` passes at both sides of `1e-6 + 1e-4·abs(x)` (C7)
- [x] Test `owns_correction_gets_no_correction_and_keeps_the_veto` passes (C8)
- [x] Test `a_limited_diode_reaches_the_unlimited_operating_point` solves the diode of abi.md §11.2 from a 5 V start and reads the same `vd` as the unlimited diode within 1e-9
- [x] Test `age_and_last_follow_the_table_of_the_abi` passes: 0 before any solve, 2 after an iteration, 1 after an accept; `STATIC` with `AGE = 1` re-seeds
- [x] Test `an_unevaluated_limit_answers_last` passes (A3)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): limit inputs with the held iterate, correction and veto`

---

### T6: `DELAY` terms

**What**: Delay history, lookup at `t − td`, the `max` bound.
**Where**: `crates/piperine-analog-solver/src/device/delay.rs` (+ target `tests/assembly_delay.rs`)
**Depends on**: T5
**Reuses**: design `DELAY`; abi.md §4.3
**Requirement**: SAN-05, SAN-03 (AC 5)
**Numerically sensitive**: no

**Done when**:

- [x] Test `a_delay_term_reads_its_history_at_t_minus_td` passes on a ramp, between two accepted points (C9)
- [x] Test `a_delay_term_is_its_value_in_an_equilibrium_solve` passes
- [x] Test `a_delay_beyond_its_max_fails_naming_the_term` passes (C10)
- [x] Test `a_restore_returns_the_delay_history` passes
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `feat(analog): delay terms over an accepted history`

---

### T7: Small-signal assembly and `STIM`

**What**: `assemble_ac`: complex stamps per operator and the stimulus right side.
**Where**: `crates/piperine-analog-solver/src/device/small_signal.rs` (+ target `tests/assembly_small_signal.rs`)
**Depends on**: None (phase order: starts after T6 closes the previous phase)
**Reuses**: design `assemble_ac`; abi.md §10 "Ac"
**Requirement**: SAN-08
**Numerically sensitive**: yes

**Done when**:

- [x] Test `small_signal_weights_each_operator` asserts 1, `jω` and `exp(−jω·td)` on one term of each kind (C14)
- [x] Test `a_stim_input_is_the_right_side` asserts `−g·(RE + j·IM)`
- [x] Test `an_rc_low_pass_has_its_corner` solves one frequency by hand and reads `1/√2` at `1/(2πRC)` within 1e-9
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): small-signal assembly and stimulus inputs`

---

### T8: Noise columns

**What**: `noise_sources`: one column per `NOISE` input, through every term that reads it.
**Where**: `crates/piperine-analog-solver/src/device/small_signal.rs` (+ target `tests/assembly_noise.rs`)
**Depends on**: T7
**Reuses**: design `noise_sources`; abi.md A2, §10 "Noise"
**Requirement**: SAN-09
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_noise_input_is_one_column_with_its_psd` passes for a resistor's thermal source: rows `+1` at `plus`, `−1` at `minus`, density `4kT/R`
- [x] Test `one_noise_input_on_two_terms_is_correlated` passes: a resistor split in two that shares one input gives zero density across the pair, and two inputs give the sum (C15)
- [x] Test `a_psd_that_reads_freq_is_evaluated_per_frequency` passes and is not evaluated outside a noise sweep (A4)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): noise inputs as correlated columns`

---

### T9: Watches and the accept step

**What**: `Assembly::accept`: `VAR`, `CROSS`, `CAP`, history and `LAST` advance; `TRACE` records; `samples`.
**Where**: `crates/piperine-analog-solver/src/device/watch.rs` (+ target `tests/assembly_watch.rs`)
**Depends on**: T8
**Reuses**: design Watches, `accept`; abi.md §4.3 (watch table), A5
**Requirement**: SAN-10
**Numerically sensitive**: no

**Done when**:

- [x] Test `a_var_watch_is_readable_at_an_accepted_solution` passes (C16)
- [x] Test `a_cross_watch_fires_on_a_rising_sign_change` passes: `≤ 0` to `> 0` fires, `> 0` to `> 0` and a falling change do not (C17)
- [x] Test `accept_reports_the_smallest_cap` passes (C18)
- [x] Test `a_trace_record_survives_a_rejected_step` passes: records written before a `restore` are still drained, each with its iteration number (C19)
- [x] Test `samples_are_the_continuous_pins_reactions_read` passes
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(analog): watches, traces and the accept step`

---

### T10: Boundary guard and the facade's fit

**What**: A guard target of this crate: dependency, no digital or owner item, no `unsafe`, the numeric core untouched; and one test that drives every method an `AnalogSide` adapter needs.
**Where**: `crates/piperine-analog-solver/tests/boundary.rs`
**Depends on**: T9
**Reuses**: the scan style of `crates/piperine-solver/tests/crate_boundaries.rs` (plain `std::fs`); `.specs/features/solver-abi-core/design.md` component `protocol` (`AnalogSide`)
**Requirement**: SAN-01, SAN-12
**Numerically sensitive**: no

**Done when**:

- [x] Test `the_crate_depends_on_the_abi_crate_only` reads this crate's manifest
- [x] Test `device_modules_name_no_digital_or_owner_item` scans `src/device/` for `piperine_digital_solver`, `piperine_solver`, `LogicValue`, `DigitalNet` (C1); test `device_modules_hold_no_unsafe` passes
- [x] Test `the_facade_offers_what_an_analog_side_needs` runs, on an RC with a DAC: `set_discrete`, `assemble_tran` for both stages, `end_iteration`, `push_stage`, `accept`, `snapshot`, `restore`, `samples`, `drain_traces` (C22)
- [x] `git diff <base>..HEAD --stat -- . ':!crates/piperine-analog-solver' ':!.specs'` is empty, and so is the diff of `src/math/`, `src/convergence.rs`, `src/tolerances.rs`, `src/config.rs`, where `<base>` is the commit before T1; the commit body states the base hash
- [x] `cargo test -p piperine-analog-solver --lib` reports the 24 old tests passing (C23), and `cargo test -p piperine-solver` is green unedited
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `test(analog): boundary guard and facade fit of the device assembly`

---

## Checks

**C1** - The analog crate depends on `piperine-abi` only and its new modules name no digital or owner item (SAN-01, AC 1)
Proof: run analog | test device_modules_name_no_digital_or_owner_item ... ok

**C2** - A `FLOW` term stamps `g_j` at (`plus`, `j`), `−g_j` at (`minus`, `j`) and `∓(v − Σ g_j·x_j)` on the right side (SAN-02, AC 2)
Proof: run analog | test a_flow_term_stamps_its_norton_form ... ok

**C3** - A `DDT` term contributes nothing in an equilibrium solve (SAN-03, AC 3)
Proof: run analog | test a_ddt_term_is_silent_in_an_equilibrium_solve ... ok

**C4** - A `DDT` term stamps the TR-BDF2 companion of `stage_coeffs` over its own history, with the trapezoidal correction (SAN-03, AC 4)
Proof: run analog | test a_ddt_term_stamps_the_capacitor_companion ... ok

**C5** - A restore returns term histories and limit state to their values before the step (SAN-03, AC 5)
Proof: run analog | test a_restore_returns_histories_and_limit_state ... ok

**C6** - A limit's reader is linearized at the held iterate with Jacobian `g·p` and right side `g·(x − x̂)` (SAN-04, AC 6)
Proof: run analog | test a_limit_reader_is_linearized_at_the_held_iterate ... ok

**C7** - A limit with `abs(x̂ − x) > 1e-6 + 1e-4·abs(x)` vetoes convergence (SAN-04, AC 7)
Proof: run analog | test an_active_limit_vetoes_convergence ... ok

**C8** - `OWNS_CORRECTION` gets no correction and keeps the veto (SAN-04, AC 8)
Proof: run analog | test owns_correction_gets_no_correction_and_keeps_the_veto ... ok

**C9** - A `DELAY` term reads its accepted history at `t − td` (SAN-05, AC 9)
Proof: run analog | test a_delay_term_reads_its_history_at_t_minus_td ... ok

**C10** - A `td` beyond `max` fails naming instance and term (SAN-05, AC 10)
Proof: run analog | test a_delay_beyond_its_max_fails_naming_the_term ... ok

**C11** - A function whose inputs did not change is not called (SAN-06, AC 11)
Proof: run analog | test a_clean_function_is_not_called ... ok

**C12** - A non-finite value or seeded partial fails naming instance, cell and `origin` (SAN-07, AC 12)
Proof: run analog | test a_non_finite_value_names_instance_cell_and_origin ... ok

**C13** - The pattern is the declared terms × partials in every solve (SAN-02, AC 13)
Proof: run analog | test the_pattern_is_terms_times_partials ... ok

**C14** - A small-signal sweep weights `FLOW` by 1, `DDT` by `jω`, `DELAY` by `exp(−jω·td)` (SAN-08, AC 14)
Proof: run analog | test small_signal_weights_each_operator ... ok

**C15** - One `NOISE` input on two terms is one correlated source (SAN-09, AC 15)
Proof: run analog | test one_noise_input_on_two_terms_is_correlated ... ok

**C16** - A `VAR` watch is readable at an accepted solution (SAN-10, AC 16)
Proof: run analog | test a_var_watch_is_readable_at_an_accepted_solution ... ok

**C17** - A `CROSS` watch fires when its value goes from `≤ 0` to `> 0` between accepted solutions (SAN-10, AC 17)
Proof: run analog | test a_cross_watch_fires_on_a_rising_sign_change ... ok

**C18** - `accept` reports the smallest `CAP` evaluated there (SAN-10, AC 18)
Proof: run analog | test accept_reports_the_smallest_cap ... ok

**C19** - A `TRACE` record keeps its Newton iteration and survives a restore (SAN-10, AC 19)
Proof: run analog | test a_trace_record_survives_a_rejected_step ... ok

**C20** - Ambient inputs carry time, frequency, the static flag and the continuation values (SAN-11, AC 20)
Proof: run analog | test ambient_inputs_carry_the_solve_coordinates ... ok

**C21** - Setting a discrete slot makes its readers run again (SAN-12, AC 21)
Proof: run analog | test setting_a_discrete_slot_makes_its_readers_dirty ... ok

**C22** - The module offers every method an `AnalogSide` adapter needs (SAN-12, AC 22)
Proof: run analog | test the_facade_offers_what_an_analog_side_needs ... ok

**C23** - The crate's 24 old unit tests pass unchanged (SAN-12, Goals)
Proof: `cargo test -p piperine-analog-solver --lib 2>&1 | grep -E '^test result: ok\. 24 passed'`

---

## Handoff

**Settled mid-build:** none
**Dispatched:** B1 at daa75d37 - returned (T1-T6 45d8a189..f7cde469 + merge core 1f75003a; limited-diode test at reltol 1e-10 (new test); DELAY history pushed in accept); B2 (Opus) at ecddd47c - returned (T7-T10 92462429..9401619a + core final merge b59184c6 + adapt 50dd951d); Verifier (Opus) at 836b22cb
**Walked:** not applicable - no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T7 → T8 → T9 → T10
```

Batches: Phase 1 (6 tasks), Phase 2 (4 tasks). Phase 2 starts after T6 because the small-signal weights and `accept` cover all three operators.

## Task Granularity Check

Each task is one file of `src/device/` with its own test target. T1 also adds the manifest dependency, the module root and the fixture devices because nothing else compiles without them. T7 and T8 share `small_signal.rs` with separate targets: the sweep's matrix and the noise columns are separate rules (abi.md §10 "Ac" and "Noise").

## Test Co-location Validation

Every task adds its tests in the same commit, in the target the matrix names. T10 is a guard: its tests are the deliverable. No task says `Tests: none`.

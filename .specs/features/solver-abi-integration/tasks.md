# Solver ABI Integration Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-abi-integration/design.md`
**Status**: Done
**Build**: `cargo build -p piperine-abi --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-analog-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-digital-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run solver**: `cargo test -p piperine-solver`
**Run guards**: `cargo test -p piperine-repository-guards --test boundaries crate_boundaries::`
**Run drivers**: `cargo test -p piperine-solver --test abi_devices --test transient`
**Run crossings**: `cargo test -p piperine-solver --test protocol protocol_crossings::`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`

Executors: Sonnet sub-agents by default, one commit per task; verifier: Opus. The orchestrator decides per batch whether Opus implements instead (a different Opus then verifies): the feature is large (25 tasks); Phases 1 to 3 are numerically and protocol delicate, Phase 4 is numerically delicate by volume (the port), Phase 5 is mechanical. Work in `/home/keslarek/Git/piperine-solver-completeness`, branch `feature/solver-completeness`, after `solver-abi-digital` and `solver-abi-analog` are both merged into it. Never pass `--workspace`, never run `cargo clean`, do not set `CARGO_TARGET_DIR`, do not push (the tag of T10 is local).

Rules every task follows:

- **The numeric core is not edited.** No change to `crates/piperine-analog-solver/src/math/`, `convergence.rs`, `tolerances.rs`, `config.rs`. If a task seems to need one, stop and report.
- **Adapters only.** This feature adds no logic to `src/protocol/` (other than `adapters.rs`), to `piperine-digital-solver` or to `crates/piperine-analog-solver/src/device/`. A defect found there is fixed in its crate with a test in its crate, in its own commit, and reported in the batch summary.
- **A ported test keeps its name, its asserted quantities and its tolerances.** If a `Device` cannot meet a tolerance, the device or the wiring is wrong: fix it, never the tolerance. A numeric literal of `parity_baseline.rs`, `live_params.rs`, `pss.rs`, `sens.rs`, `adjoint_*.rs`, `transient_reentry.rs` is never edited.
- **A test is removed only by the rule** of design "Test disposition": every assertion in it is about a removed item. Its name and that item go into `.specs/features/solver-abi-integration/removed.txt` (`name<TAB>item`) in the same commit, and the commit body lists it.
- **Numerically sensitive tasks** (`Numerically sensitive: yes`) run `cargo test -p piperine-solver --test parity_baseline` and, while it exists, `--test differential` before anything else in their gate, and report the two results in the commit body.
- **The `Element` branch is frozen** until Phase 5: a task adds the device branch beside it and does not edit the old one.
- **abi.md is the contract. No macro** for devices, tables or helpers. `unsafe` stays in `piperine-abi`.
- Tests derive from the spec's criteria, abi.md's rules and today's asserted numbers, never from the new implementation. Test functions have exactly the names the checks cite. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on the four crates.
- Commit message as given per task, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("zero warnings is the bar"; the numeric oracles), `.specs/STATE.md` MD-28, MD-31, MD-33, MD-60, MD-61; user: unit tests only; "a paridade com ngspice é um strenght bem alto".

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Owner bind, adapters, host surface | unit | each AC of the Protocol story and each edge case, on hand-written `Device`s over the real sub-solvers | `crates/piperine-solver/tests/{bind,operating_point,protocol_real,protocol_crossings,host_surface,host_set}.rs` | `cargo test -p piperine-solver` |
| Drivers on devices (dc, transient, ac, noise, sens, adjoint, pss, tf, pz, sp) | unit | the literals of the existing tests reproduced on devices; the differential oracle while `Element` exists | `crates/piperine-solver/tests/{abi_dc,abi_tran,abi_ac,abi_noise,abi_sens,differential}.rs`, unit tests in `src/analyses/` | `cargo test -p piperine-solver` |
| Test devices | unit | every function passes `verify_partials`; every shape passes `validate` | `crates/piperine-solver/tests/helpers/{abi_devices,abi_mixed}.rs`, `tests/abi_devices.rs` | `cargo test -p piperine-solver --test abi_devices` |
| Ported targets | unit | name, quantities and tolerances unchanged; removed tests listed | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Structural guards, removed names, instruction files, `.specs/STATE.md` | unit | one guard test per removed family and per boundary | `crates/piperine-solver/tests/{removed_surface,crate_boundaries,workspace_shape}.rs` | `cargo test -p piperine-solver --test removed_surface --test crate_boundaries --test workspace_shape` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | never alone | `cargo test -p piperine-solver --test <the task's target>` |
| Full | every task | `cargo test -p piperine-solver`; `cargo test -p piperine-analog-solver`; `cargo test -p piperine-digital-solver` |
| Build | last task of each phase | Full, plus `cargo test -p piperine-abi`, plus the four `**Build**` commands (zero warnings), plus `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Phases are ordered and run sequentially. Five batches, one per phase (4, 6, 5, 6, 4 tasks). Each batch leaves the four crates green: the `Element` branch runs every unported test until Phase 5. Sensitive tasks are marked in the breakdown: T3 to T10, T12, T14, T15, T17 to T19.

### Phase 1: Bind and the first two drivers

```
T1 → T2 → T3 → T4
```

### Phase 2: The differential oracle and the small-signal drivers

```
T5 → T6 → T7 → T8 → T9 → T10
```

### Phase 3: The protocol over the real sub-solvers

```
T11 → T12 → T13 → T14 → T15
```

### Phase 4: Every surviving test on devices

```
T16 → T17 → T18 → T19 → T20 → T21
```

### Phase 5: One contract

```
T22 → T23 → T24 → T25
```

---

## Task Breakdown

### T1: Bind in the owner

**What**: `CircuitBuilder::device`: create, bind pins by role, allocate unknowns, build the two plans.
**Where**: `crates/piperine-solver/src/core/bind.rs` (+ `builder.rs`, `circuit.rs` field `abi`, `Cargo.toml` dependency on `piperine-abi`, target `tests/bind.rs`)
**Depends on**: None
**Reuses**: design `bind`; `builder.rs` (`node`, `UnknownAllocator`); `piperine_digital_solver::abi::DigitalPlan`; `AnalogPlan`
**Requirement**: SAI-01, SAI-09
**Numerically sensitive**: no

**Done when**:

- [x] The owner depends on `piperine-abi`, `piperine-analog-solver` and `piperine-digital-solver`; `owner_depends_on_both_sub_solvers_only` becomes `owner_depends_on_the_abi_and_both_sub_solvers_only` and passes
- [x] Test `pins_bind_by_role` passes: ports by position, a param by name with the given value and with the prototype default, a global by constant, a `NAMED` global by name, a private pin allocated
- [x] Test `ports_on_one_net_share_one_unknown` passes, and a private continuous pin is an unknown of its own, allocated in instance order then pin order
- [x] Test `a_pin_abstol_tightens_its_node` passes through `node_vntol_overrides`
- [x] Test `a_discrete_net_an_analog_function_reads_is_flagged` passes
- [x] Test `a_circuit_mixing_elements_and_devices_is_refused` passes
- [x] Test `a_shared_param_net_feeds_every_instance_bound_to_it` passes (P5)
- [x] Every existing test of the crate passes unchanged
- [x] This is the first commit that compiles the protocol, the engine and the assembly together: if a facade does not fit its trait, stop and report before writing an adapter with logic in it
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): bind device instances to nets and unknowns`

---

### T2: The analog device library on `Device`

**What**: The doubles of `tests/helpers/devices.rs` as `Device`s with the same equations and constants.
**Where**: `crates/piperine-solver/tests/helpers/abi_devices.rs` (+ target `tests/abi_devices.rs`)
**Depends on**: T1
**Reuses**: `tests/helpers/devices.rs` (`R`, `ROpt`, `C`, `D`, `Vsrc`, `Vsine`, `Vramp`), each one's quoted Verilog-A comment; `piperine_abi::verify::verify_partials`
**Requirement**: SAI-15
**Numerically sensitive**: no

**Done when**:

- [x] `R`, `ROpt`, `C`, `D`, `Vsrc`, `Vsine`, `Vramp` exist as `Device`s; each keeps the quoted source comment of its `Element` twin; params have the same names and defaults; `ROpt`'s rebuild-class param is `STRUCTURAL`
- [x] Partials are declared against continuous pins and against every param the sensitivity tests differentiate
- [x] Test `every_device_function_passes_the_partials_verifier` walks a plain table of (device, function, operating points)
- [x] Test `every_device_shape_is_valid` passes
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): analog device library on the Device trait`

---

### T3: The DC driver stamps devices

**What**: `DcSystem` collects stamps from `Assembly` when the circuit is built from devices: `STATIC`, `GMIN`, `SOURCE_SCALE`, the limit veto.
**Where**: `crates/piperine-solver/src/analyses/dc.rs` (+ target `tests/abi_dc.rs`)
**Depends on**: T2
**Reuses**: design "The strangler"; `DcSystem::assemble`, `HomotopyDriver for DcSolver` (`set_gmin_extra`, `set_src_scale`, `checkpoint_devices`)
**Requirement**: SAI-11
**Numerically sensitive**: yes

**Done when**:

- [x] `assemble` branches once on `circuit.abi`; the `Element` branch is byte-identical to before
- [x] `any_limiting_report` answers from `Assembly::limiting_active` on the device branch; `apply_limiting_reports` does nothing there; `end_iteration` is called once per Newton iteration
- [x] Homotopy sets `GMIN` and `SOURCE_SCALE`; both are at rest in the last solve; the homotopy checkpoint and restore go through `Assembly::snapshot`
- [x] Test `divider_reads_the_analytic_ratio_on_devices` and test `diode_satisfies_its_own_equation_on_devices` pass with the literals and tolerances of `helper_devices.rs`
- [x] Test `source_stepping_scales_sources_through_the_ambient_input` passes
- [x] `cargo test -p piperine-solver` is green: every existing test unchanged
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): the DC driver assembles device instances`

---

### T4: The transient driver stamps devices

**What**: `TransientSystem` collects stamps from `Assembly`: stages, history push and snapshot with the Newton history, the hold from `Pin.init`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (+ target `tests/abi_tran.rs`)
**Depends on**: T3
**Reuses**: design "Mapping to today's code"; `TransientSystem::assemble`, `execute_timestep`, `attempt_step`, both reject paths, `UicClamp`
**Requirement**: SAI-11, SAI-01
**Numerically sensitive**: yes

**Done when**:

- [x] `assemble` branches once on `circuit.abi`; `Assembly::push_stage` follows each converged stage; `snapshot`/`restore` travel with `solver.state_snapshot()`/`restore_state`
- [x] Test `rc_step_reads_one_minus_exp_on_devices` passes with the literal and tolerance of `helper_devices.rs`
- [x] Test `a_continuous_init_starts_newton_and_is_held_through_the_operating_point` passes
- [x] Test `a_rejected_stage_restores_the_term_history` forces a Newton failure with a tiny iteration cap and compares histories
- [x] No discrete pin exists in these circuits: the protocol is wired in Phase 3; a device circuit with a discrete net an analog function reads is refused by the transient driver until then, with a test
- [x] `cargo test -p piperine-solver` is green: every existing test unchanged
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `feat(solver): the transient driver assembles device instances`

---

### T5: Differential oracle: DC and transient

**What**: One target that builds each circuit twice, from `Element` doubles and from `Device`s, and compares.
**Where**: `crates/piperine-solver/tests/differential.rs`
**Depends on**: None (phase order: starts after T4 closes the previous phase)
**Reuses**: `tests/helpers/devices.rs` and `abi_devices.rs`; the circuits of `helper_devices.rs`, `sens.rs`, `pss.rs` (sine RC), `parity_baseline.rs`
**Requirement**: SAI-13, SAI-02
**Numerically sensitive**: yes

**Done when**:

- [x] Test `dc_agrees_on_both_contracts` compares every unknown of the divider, the diode circuit and a diode with series resistance within 1e-12 relative
- [x] Test `transient_takes_the_same_steps_on_both_contracts` compares, for the RC step and the sine-driven RC, the accepted step count (equal), every accepted time (equal to 1e-15 relative) and every unknown at every step (1e-12 relative)
- [x] Test `newton_takes_the_same_iterations_on_both_contracts` compares `stats.newton_iterations` on the diode circuit
- [x] If a comparison fails, stop and report; no tolerance of this target is negotiable without the user
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): differential oracle for DC and transient`

---

### T6: The AC driver stamps devices

**What**: `AcSolver` collects complex stamps and the stimulus from `assemble_ac`.
**Where**: `crates/piperine-solver/src/analyses/ac.rs` (+ `abi_devices.rs` gains an AC source with a `STIM` input, target `tests/abi_ac.rs`)
**Depends on**: T5
**Reuses**: `AcSolver`; the AC doubles of `parity_baseline.rs`
**Requirement**: SAI-11
**Numerically sensitive**: yes

**Done when**:

- [x] The branch is one `if` on `circuit.abi`; the `Element` branch is unchanged
- [x] Test `an_rlc_peak_matches_the_baseline_on_devices` reads the peak magnitude and frequency literals of `parity_baseline.rs` with their tolerances
- [x] No reaction runs and no pin changes during a sweep (a counting device asserts it)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): the AC driver assembles device instances`

---

### T7: The noise driver stamps devices

**What**: `NoiseSolver` injects `NoiseColumn`s.
**Where**: `crates/piperine-solver/src/analyses/noise.rs` (+ target `tests/abi_noise.rs`; the inline `Element` double of `noise.rs` ported)
**Depends on**: T6
**Reuses**: `NoiseSolver`; `Assembly::noise_sources`
**Requirement**: SAI-11
**Numerically sensitive**: yes

**Done when**:

- [x] Output density is `Σ |H·b|²·PSD` over the columns; contributions keep their names in `NoiseContribution`
- [x] Test `a_resistor_divider_has_its_thermal_noise_on_devices` matches the analytic `4kT·(R1∥R2)` within the tolerance the existing noise unit tests use
- [x] Test `a_shared_noise_input_cancels_across_the_pair` passes through the driver
- [x] The unit tests of `noise.rs` pass with their double ported, assertions unchanged
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): the noise driver assembles device instances`

---

### T8: Sensitivities through declared partials

**What**: `ParamStampCollector` reads `Assembly::param_stamps`; a path with no partial falls back to a finite difference by re-evaluation.
**Where**: `crates/piperine-solver/src/analyses/sensitivity.rs` (+ `sens.rs`, `adjoint.rs`, target `tests/abi_sens.rs`)
**Depends on**: T7
**Reuses**: `sensitivity.rs` (`SensitivityEngine`, the stamp-perturbation fallback), `sens.rs`, `adjoint.rs`; abi.md §10 "Host procedures", F5
**Requirement**: SAI-10
**Numerically sensitive**: yes

**Done when**:

- [x] A param is addressed by instance path and pin name; partials against the param's pin are seeded only here
- [x] A derivative chains through the partials of a discrete function between the param and the term (a test device with a derived conductance)
- [x] Test `sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference` passes: equal results from both routes within 1e-6 relative on the divider
- [x] Test `a_structural_param_sensitivity_fails_loud` passes
- [x] The `Element` branch and its tests are unchanged
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): sensitivities through declared partials`

---

### T9: `tf`, `pz`, `sp` on devices

**What**: The three host procedures read the device branch of the AC assembly; their inline doubles are ported.
**Where**: `crates/piperine-solver/src/analyses/sp.rs` (+ `pz.rs`, `tf.rs`)
**Depends on**: T8
**Reuses**: `sp.rs`, `pz.rs`, `tf.rs` unit tests and their inline `impl Element` doubles (4, 5, 0)
**Requirement**: SAI-11, SAI-12
**Numerically sensitive**: yes

**Done when**:

- [x] Each inline double is a `Device` with the same values; each unit test keeps its name, literals and tolerances
- [x] The 19 unit tests of the three files pass
- [x] No assertion line of the three files changes (the commit body shows `git diff --stat` and states it)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): tf, pz and sp doubles on the Device trait`

---

### T10: Differential oracle: AC, noise, sensitivities; the record

**What**: Extend the oracle to the small-signal family and record its run, because the target is deleted with `Element`.
**Where**: `crates/piperine-solver/tests/differential.rs` (+ `.specs/features/solver-abi-integration/differential.txt`)
**Depends on**: T9
**Reuses**: the differential target of the first task of this phase
**Requirement**: SAI-13
**Numerically sensitive**: yes

**Done when**:

- [x] Test `ac_agrees_on_both_contracts`, test `noise_agrees_on_both_contracts` and test `sensitivities_agree_on_both_contracts` pass within 1e-12 relative
- [x] `.specs/features/solver-abi-integration/differential.txt` holds the full output of `cargo test -p piperine-solver --test differential` at this commit and the commit hash; the commit is tagged `solver-abi-integration-differential` (local tag, not pushed)
- [x] The file ends with a `test result: ok.` line counting six tests
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `test(solver): differential oracle for the small-signal family, recorded`

---

### T11: The two adapters and the operating point

**What**: `impl DiscreteSide` over `Digital`, `impl AnalogSide` over the DC and transient drivers; the device branch of the operating point goes through `Protocol::operating_point`.
**Where**: `crates/piperine-solver/src/protocol/adapters.rs` (+ `analyses/dc.rs`, mixed fixture devices in `tests/helpers/abi_mixed.rs`, target `tests/operating_point.rs`)
**Depends on**: None (phase order: starts after T10 closes the previous phase)
**Reuses**: design "The two adapters"; `protocol/sides.rs`; `Digital`, `Assembly`; `dc.rs` mixed loop; the comparator, flip-flop and DAC of abi.md §11.4 to §11.6
**Requirement**: SAI-06, SAI-09
**Numerically sensitive**: no

**Done when**:

- [x] The adapters convert and forward; neither holds a rule of its own (the commit body lists every method and the call it forwards to)
- [x] Test `a_mixed_loop_settles_at_its_operating_point` passes with a comparator, a flip-flop and a DAC: the consistent point is reached and a counting device sees `SETTLE` once
- [x] Test `operating_point_rounds_beyond_the_bound_are_e0506` passes with the loop wired as an oscillator, naming its nets
- [x] Test `globals_hold_the_session_values` reads `TEMPERATURE`, `RESOLUTION`, `DC` and a `NAMED` session parameter from inside a device
- [x] The `Element` branch of the mixed loop is unchanged (it is deleted in Phase 5)
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): adapters for the two sides and the operating point through the protocol`

---

### T12: The transient loop through the protocol

**What**: The device branch of `TransientSolver::solve` is driven by `Protocol::transient` over `TransientSide`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (+ `protocol/adapters.rs`, target `tests/protocol_real.rs`)
**Depends on**: T11
**Reuses**: design "The two adapters" (Mapping to today's code); `predict_step`, `attempt_step`, `assess_step`, `accept_step`, `propose_dt`, `reject_step`, `reject_lte_step`
**Requirement**: SAI-02, SAI-03, SAI-04
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_clock_nothing_analog_reads_adds_no_step` compares accepted step counts of an RC transient with and without a free-running `Timer`
- [x] Test `a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next` passes with a timer driving the DAC of abi.md §11.6: the solution at the landing uses the new code, the landing is exempt from `trtol`, the next stage runs with `prev_h = 0` — **superseded** by the user's order "ABI order: solve, then react (Recommended)": the landing is solved on the old code, the next step reads the new one and is exempt; the test asserts that
- [x] Test `a_rejected_step_leaves_no_trace` forces an LTE rejection and a Newton failure and compares the digital checkpoint, the assembly snapshot and the replayed instants
- [x] Test `a_design_without_analog_runs_every_instant_with_no_solve` passes
- [x] Test `a_commit_due_after_the_stop_time_stays_pending` and test `settle_fires_at_the_stop_time` pass
- [x] `assess_step`'s arithmetic and `propose_dt`'s controller call are not edited; the diff of those two functions is limited to where their inputs come from
- [x] `differential` (re-run on the device branch, now through the protocol), `parity_baseline`, `pss`, `transient_reentry` and `live_params` pass unchanged; if the differential step count moves, stop and report
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): the transient loop runs through the protocol`

---

### T13: The host surface: sink, waiters, `FINISH`

**What**: `Solver::with_sink` and `Solver::on_event` forward to the protocol; `FINISH` ends the analysis result at its instant.
**Where**: `crates/piperine-solver/src/analyses/solver.rs` (+ `transient.rs` result, target `tests/host_surface.rs`)
**Depends on**: T12
**Reuses**: `Protocol::{with_sink, on_event}`; `Recorder`, `Stub` and `Sampler` fixture devices of `solver-abi-digital` copied into `abi_mixed.rs`
**Requirement**: SAI-05, SAI-07
**Numerically sensitive**: no

**Done when**:

- [x] Test `the_sink_receives_everything_in_instant_order` passes on a real mixed run: changes, commits, a fired crossing and records carry path, analysis kind and time; a `TRACE` record carries its Newton iteration
- [x] Test `records_of_an_undone_instant_do_not_reach_the_sink` passes with a forced rejection
- [x] Test `an_irreversible_reaction_runs_once_at_an_accepted_time` passes with the `Stub` device across a rejected step
- [x] Test `finish_ends_the_analysis_at_an_accepted_time` passes: the last recorded time is the instant of the `FINISH`
- [x] Test `a_host_waiter_is_called_once_and_its_write_is_due_next_microstep` passes
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): sink, waiters and finish on the solver's host surface`

---

### T14: Host writes and `RESHAPE`

**What**: `set` and `schedule_set` on the device branch go through `Protocol::set`; a `Reshape` between analyses re-creates and re-binds the instance.
**Where**: `crates/piperine-solver/src/core/circuit.rs` (+ `transient.rs` `schedule_set`, `bind.rs` re-bind, target `tests/host_set.rs`)
**Depends on**: T13
**Reuses**: design "Host actions"; `set_element_param`, `apply_scheduled_sets`, `SetQueue`; `Protocol::set`
**Requirement**: SAI-08
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_false_check_refuses_a_set` passes and the next solve uses the old value
- [x] Test `reshape_is_refused_in_a_transient_and_rebinds_between_analyses` passes with a device whose internal node collapses below a threshold: every net keeps its value across the re-bind
- [x] Test `a_scheduled_host_write_lands_and_restarts_small` passes: the write's time is an accepted time and the next step is 10⁻³ of the last, floored at `dt_min`
- [x] Test `a_structural_set_during_a_run_is_refused` passes
- [x] The `Element` branch of `set_element_param` and `SetQueue` is unchanged; `live_params` passes unchanged
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(solver): host writes through the parameter cone`

---

### T15: Crossings, samples and step caps

**What**: The transient adapter passes fired crossings and samples to the protocol and bounds `propose` by the last `CAP`.
**Where**: `crates/piperine-solver/src/protocol/adapters.rs` (+ target `tests/protocol_crossings.rs`)
**Depends on**: T14
**Reuses**: `Assembly::{accept, samples}`; abi.md C6, R2, §4.3; the comparator of §11.5
**Requirement**: SAI-11, SAI-02
**Numerically sensitive**: yes

**Done when**:

- [x] Test `a_fired_crossing_is_a_commit_at_the_landing` passes: the comparator's `up` fires at microstep 0 of the first accepted time after the sign change, and `out` changes one microstep later
- [x] Test `a_reaction_reads_the_accepted_solution` passes with the `Sampler`
- [x] Test `a_cap_watch_bounds_the_next_step` passes: a sine with a 1 ns cap never takes a longer step
- [x] The crossing is not located inside the step: the test names `solver-crossings` in a comment and asserts the present behavior
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `feat(solver): crossings as commits, sampled reactions and step caps`

---

### T16: Mixed devices and `mixed_signal`

**What**: `Comparator`, `ToggleDff`, `D2A`, `ClockSrc` as `Device`s; `mixed_signal.rs` ported test by test.
**Where**: `crates/piperine-solver/tests/mixed_signal.rs` (+ `tests/helpers/abi_mixed.rs`, `.specs/features/solver-abi-integration/removed.txt`)
**Depends on**: None (phase order: starts after T15 closes the previous phase)
**Reuses**: `tests/helpers/{devices.rs,mod.rs}` (`Comparator`, `ToggleDff`, `ClockSrc`, `D2ADevice`, `A2DState`); design "Test disposition"
**Requirement**: SAI-12
**Numerically sensitive**: no

**Done when**:

- [x] `ClockSrc` is a timer reaction plus a discrete phase pin its analog function reads; its waveform values are those of `ClockSrc::at`
- [x] Each of the 25 tests is ported (name, quantities, tolerances unchanged) or removed by the rule; the commit body lists both sets
- [x] `.specs/features/solver-abi-integration/removed.txt` has one line per removed test: its name, a tab, the removed item it was about
- [x] Test `every_device_function_passes_the_partials_verifier` covers the new devices
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): mixed-signal devices and tests on the Device trait`

---

### T17: `parity_baseline` on devices

**What**: Port the exact-value oracle: 12 inline doubles become `Device`s.
**Where**: `crates/piperine-solver/tests/parity_baseline.rs`
**Depends on**: T16
**Reuses**: the 12 `impl Element` doubles of the file; design "Test disposition", "Numerically sensitive work"
**Requirement**: SAI-12, SAI-02
**Numerically sensitive**: yes

**Done when**:

- [x] Every test keeps its name; every numeric literal and tolerance is byte-identical (C41's proof compares them with the base commit)
- [x] The four `LogicValue` asserts of the digital-settle test become `HIGH`/`LOW` byte asserts on the same nets, same levels
- [x] The transient test still reports 161 steps
- [x] If any literal does not hold, stop and report; nothing is retuned
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): parity baselines on the Device trait`

---

### T18: Sensitivity, re-entry and PSS targets on devices

**What**: Port `sens.rs`, `adjoint_dc.rs`, `adjoint_vs_sens.rs`, `sensitivity_fallback.rs`, `transient_reentry.rs`, `pss.rs`.
**Where**: `crates/piperine-solver/tests/sens.rs` (+ the other five targets; `src/analyses/pss.rs` and `transient.rs` re-entry state)
**Depends on**: T17
**Reuses**: `abi_devices.rs`, `abi_mixed.rs`; `TransientStep` re-entry (`apply_reentry_state`, `collect_digital_hidden`)
**Requirement**: SAI-12, SAI-10
**Numerically sensitive**: yes

**Done when**:

- [x] The 17 tests keep their names, quantities and tolerances
- [x] Re-entry state is the protocol's checkpoint (digital checkpoint, assembly snapshot, Newton history): `reentry_from_captured_state_matches_continuous_run` passes unchanged
- [x] `digital_divider_reports_k_times_period` passes on `ClockSrc`, `Comparator`, `ToggleDff` devices
- [x] `sens_rebuild_class_param_fails_loud` passes on the `STRUCTURAL` param
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): sensitivity, re-entry and PSS targets on the Device trait`

---

### T19: Host-surface targets on devices

**What**: Port `live_params.rs`, `temperature.rs`, `probe_selection.rs`, `nature_abstol_inheritance.rs`.
**Where**: `crates/piperine-solver/tests/live_params.rs` (+ the other three targets; `.specs/features/solver-abi-integration/removed.txt`)
**Depends on**: T18
**Reuses**: the `set` of Phase 3; `VAR` watches for observables; the `TEMPERATURE` global; `Pin.abstol`
**Requirement**: SAI-12, SAI-08, SAI-09
**Numerically sensitive**: yes

**Done when**:

- [x] Each of the 29 tests is ported or removed by the rule; a test that asserted an `Invalidation` class asserts the observable effect instead only when it already asserted one; otherwise it is removed and listed
- [x] The numeric assertions of `live_params.rs` are byte-identical
- [x] `ProbeSelection` selects `VAR` watches and term values by instance path and name
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): host-surface targets on the Device trait`

---

### T20: Convergence, failure and entry targets on devices

**What**: Port `limiting_report.rs`, `failure_rules.rs`, `solver_entry.rs`, `stepper_fold.rs`, `helper_devices.rs`.
**Where**: `crates/piperine-solver/tests/limiting_report.rs` (+ the other four targets; `.specs/features/solver-abi-integration/removed.txt`)
**Depends on**: T19
**Reuses**: design "Test disposition" rows for these targets
**Requirement**: SAI-12, SAI-11
**Numerically sensitive**: no

**Done when**:

- [x] `active_limiter_vetoes_newton_convergence` passes on a `LIMIT` input; `newton_applies_limited_value_to_the_guess` is removed and listed; `a_limited_diode_reaches_the_unlimited_operating_point` is added
- [x] `failure_rules.rs`: the non-finite tests assert the instance and the cell's `origin`; the six tests keep their names
- [x] `helper_devices.rs` runs on `abi_devices.rs`: three tests unchanged, `param_stamps_match_a_central_difference` removed and listed (replaced by `every_device_function_passes_the_partials_verifier`)
- [x] `solver_entry.rs` and `stepper_fold.rs`: names and assertions unchanged
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): convergence, failure and entry targets on the Device trait`

---

### T21: Lifecycle, rollback and surface targets

**What**: Port or remove `lifecycle.rs`, `checkpoint_restore.rs`, `digital_settle_subscriber.rs`, `stamp_bypass.rs`, `integration_scheme_elements.rs`, `capabilities_contract.rs`, `composed_element.rs`; rewrite `abi_surface.rs` and `prelude_surface.rs`.
**Where**: `crates/piperine-solver/tests/lifecycle.rs` (+ the other eight targets; `.specs/features/solver-abi-integration/removed.txt`)
**Depends on**: T20
**Reuses**: design "Test disposition"; `src/abi.rs`, `src/prelude.rs`
**Requirement**: SAI-12
**Numerically sensitive**: no

**Done when**:

- [x] Each test is ported or removed by the rule; whole targets with no surviving test are deleted and every one of their tests is listed in `removed.txt`
- [x] `abi_surface.rs`: a device written against `piperine_solver::abi` alone builds and solves; `prelude_surface.rs`: a host written against `prelude` alone builds a circuit of devices and runs DC and transient
- [x] `removed.txt` is complete for Phase 4: the commit body gives the count of ported and removed tests per target
- [x] After this task no test target of the crate except `differential.rs` and the old-engine targets names `Element`
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `test(solver): lifecycle, rollback and surface targets on the Device trait`

---

### T22: Delete the `Element` contract

**What**: Remove `Element`, `AnalogDevice`, `Introspect`, the capability flags, device checkpoints, `LimitingReport`, `Invalidation`, `next_breakpoints`, the `Element` branch of every driver, the old test helpers and `differential.rs`.
**Where**: `crates/piperine-solver/src/core/element.rs` (+ `circuit.rs`, `builder.rs`, `introspect.rs`, every `analyses/*.rs` branch, `abi.rs`, `prelude.rs`, `tests/helpers/{devices.rs,mod.rs}`, `tests/differential.rs`)
**Depends on**: None (phase order: starts after T21 closes the previous phase)
**Reuses**: design "The strangler"; `removed_surface.rs` scan helpers
**Requirement**: SAI-14
**Numerically sensitive**: no

**Done when**:

- [x] No `if circuit.abi` branch remains: the drivers have one path
- [x] `core/introspect.rs` keeps only what a shape provides (names, units, origins, layouts, defaults, `STRUCTURAL`); its unit tests are ported or listed in `removed.txt`
- [x] `DcAnalysisState.digital`, `TransientAnalysisState.digital` and `accept_timestep`'s `&[LogicValue]` are gone
- [x] `tests/differential.rs` is deleted; `differential.txt` and the tag stay
- [x] Test `the_element_contract_is_gone` of `removed_surface.rs` scans the three solver crates for the names of the Port story's AC 5 that this task removes
- [x] The four crates' suites are green
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `refactor(solver): remove the Element contract`

---

### T23: Delete the old digital engine

**What**: Remove `drivers.rs`, `events.rs`, `interface.rs`, `scheduler.rs`, `state.rs`, `topology.rs`, the owner's `analyses/events.rs` (`EventQueue`) and their test targets; flatten `src/abi/` to the crate root.
**Where**: `crates/piperine-digital-solver/src/lib.rs` (+ the six deleted files, `crates/piperine-solver/src/analyses/events.rs`, `tests/{digital_driver_segregation,digital_topology,digital_events_unified,event_adapters,event_order,event_rollback}.rs`, `.specs/features/solver-abi-integration/removed.txt`)
**Depends on**: T22
**Reuses**: design "Test disposition" last rows; `solver-abi-digital` tests (they prove the same claims on the new engine)
**Requirement**: SAI-14
**Numerically sensitive**: no

**Done when**:

- [x] Every test of the deleted files and targets is listed in `removed.txt` with its item (`Strength`, `DriverTable`, `LogicValue`, the `seq`/`comb` contract, `DigitalState`, `EventQueue`)
- [x] `predict_crossing` and `interpolate_a2d_crossing` are deleted only if nothing uses them; if `solver-crossings` will need the secant step, it moves to the analog crate with its tests and the commit body says so
- [x] Test `the_old_digital_engine_is_gone` of `removed_surface.rs` passes
- [x] `cargo test -p piperine-digital-solver` is green with the tests of `solver-abi-digital`, their paths updated for the flattened modules and no assertion changed
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `refactor(digital): remove the strength table, LogicValue and the two-phase engine`

---

### T24: Guards over the result

**What**: Extend the structural guards: removed names, the three-crate boundary, the removed-test list.
**Where**: `crates/piperine-solver/tests/removed_surface.rs` (+ `crate_boundaries.rs`, `workspace_shape.rs`)
**Depends on**: T23
**Reuses**: the scan helpers of the three targets
**Requirement**: SAI-14, SAI-12, SAI-16
**Numerically sensitive**: no

**Done when**:

- [x] Test `removed_tests_are_listed_with_their_item` reads `removed.txt`: every line is `name<TAB>item`, and no test function of that name exists in the tree
- [x] `analog_sources_name_no_digital_type` and `digital_sources_name_no_analog_type` scan the final trees (C1, C46)
- [x] Test `owner_depends_on_the_abi_and_both_sub_solvers_only` passes
- [x] The scan's self-test still catches a planted name
- [x] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(solver): guards over the single device contract`

---

### T25: Instruction files, MD-62 and the umbrella's status

**What**: Record the decision and bring the documents in line.
**Where**: `.specs/STATE.md` (+ `CLAUDE.md`, `AGENTS.md`, `.specs/features/solver-completeness/spec.md` traceability statuses)
**Depends on**: T24
**Reuses**: MD-60 and MD-61 as the form; `workspace_shape.rs` instruction-file guard
**Requirement**: SAI-16
**Numerically sensitive**: no

**Done when**:

- [x] `.specs/STATE.md` carries MD-62 (the device ABI is the only device contract; the solver owns companion, limit iterate, histories, checkpoints; `Element` is gone; boundary delivery: solve at the instant on the left value, run the instant, then deliver with restart) and a handoff snapshot
- [x] `CLAUDE.md` and `AGENTS.md` describe the four crates and name no removed item; the instruction-file guard of `workspace_shape.rs` passes with `MD-62` added to what it asserts
- [x] The umbrella's traceability marks the requirements this feature met as `Verified` only after the Verifier's report; until then `Implementing`
- [x] `cargo test -p piperine-lang --test suite_hygiene` passes
- [x] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `docs(solver): the device ABI is the only device contract (MD-62)`

---

## Checks

**C1** - Pins bind by role: ports by position, params by name, globals by constant or name, private pins allocated (SAI-01, Protocol AC 1)
Proof: run solver | test pins_bind_by_role ... ok

**C2** - Continuous ports on one net share one unknown; a private continuous pin has its own (SAI-01, Protocol AC 2)
Proof: run solver | test ports_on_one_net_share_one_unknown ... ok

**C3** - A continuous `init` is Newton's start and is held through a transient's operating point (SAI-01, Protocol AC 3)
Proof: run solver | test a_continuous_init_starts_newton_and_is_held_through_the_operating_point ... ok

**C4** - A pin's `abstol` tightens its node's tests (SAI-01, Protocol AC 4)
Proof: run solver | test a_pin_abstol_tightens_its_node ... ok

**C5** - A clock nothing analog reads adds no analog step (SAI-02, Protocol AC 5)
Proof: run solver | test a_clock_nothing_analog_reads_adds_no_step ... ok

**C6** - A boundary change due at microstep 0 of a landing is not read by the step that arrives there, which is solved on the value before the instant (the left value); the instant runs, the change is delivered after it, and the next step restarts the integrator, exempt from the truncation test (SAI-03, Protocol AC 6 as superseded by the user's order "solve, then react")
Proof: run solver | test a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next ... ok

**C7** - A rejected or shortened step leaves store, queue, sink buffer and histories unchanged and replays the instants (SAI-04, Protocol AC 7)
Proof: run solver | test a_rejected_step_leaves_no_trace ... ok

**C8** - An irreversible reaction or host waiter runs once, at an accepted time (SAI-05, Protocol AC 8)
Proof: run solver | test an_irreversible_reaction_runs_once_at_an_accepted_time ... ok

**C9** - `FINISH` ends the analysis after its instant, an accepted time (SAI-05, Protocol AC 9)
Proof: run solver | test finish_ends_the_analysis_at_an_accepted_time ... ok

**C10** - A design with no continuous pin runs every instant and the stop time with no solve (SAI-02, Protocol AC 10)
Proof: run solver | test a_design_without_analog_runs_every_instant_with_no_solve ... ok

**C11** - A transient with no analog-read discrete pin takes the same accepted steps on both contracts (SAI-02, Protocol AC 11; SAI-13, Port AC 3)
Proof: `grep -q 'test transient_takes_the_same_steps_on_both_contracts ... ok' .specs/features/solver-abi-integration/differential.txt && grep -qE '^test result: ok\. 6 passed' .specs/features/solver-abi-integration/differential.txt && git rev-parse -q --verify refs/tags/solver-abi-integration-differential`

**C12** - A mixed loop of comparator, flip-flop and DAC settles at its operating point with `SETTLE` once (SAI-06, Protocol AC 12)
Proof: run solver | test a_mixed_loop_settles_at_its_operating_point ... ok

**C13** - More than 20 operating-point rounds fail with `E0506` naming the nets (SAI-06, Protocol AC 13)
Proof: run solver | test operating_point_rounds_beyond_the_bound_are_e0506 ... ok

**C14** - The sink receives changes, commits, crossings, records and traces in instant order with path, analysis kind and time (SAI-07, Protocol AC 14)
Proof: run solver | test the_sink_receives_everything_in_instant_order ... ok

**C15** - Records of an undone instant do not reach the sink (SAI-07, Protocol AC 15)
Proof: run solver | test records_of_an_undone_instant_do_not_reach_the_sink ... ok

**C16** - A `set` whose `check` is false on the shadow is refused (SAI-08, Protocol AC 16)
Proof: run solver | test a_false_check_refuses_a_set ... ok

**C17** - `RESHAPE` is refused inside a transient and re-creates and re-binds the instance between analyses (SAI-08, Protocol AC 17)
Proof: run solver | test reshape_is_refused_in_a_transient_and_rebinds_between_analyses ... ok

**C18** - A scheduled host write is a landing and the next step is 10⁻³ of the last (SAI-08, Protocol AC 18)
Proof: run solver | test a_scheduled_host_write_lands_and_restarts_small ... ok

**C19** - The global nets hold the session's values and a `NAMED` global the host's parameter (SAI-09, Protocol AC 19)
Proof: run solver | test globals_hold_the_session_values ... ok

**C20** - Sensitivities use declared partials against the param's pin and fall back to a finite difference (SAI-10, Protocol AC 20)
Proof: run solver | test sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference ... ok

**C21** - The drivers take stamps, the veto and the continuation values from the assembly (SAI-11, Protocol AC 21): the DC, transient, AC and noise drivers read the analytic values on devices, an active `LIMIT` vetoes convergence, and the gmin step and the source scale reach a device through `GMIN` and `SOURCE_SCALE`
Proof: run drivers | test divider_reads_the_analytic_ratio_on_devices ... ok
Proof: run drivers | test rc_step_reads_one_minus_exp_on_devices ... ok
Proof: run drivers | test an_rlc_peak_matches_the_baseline_on_devices ... ok
Proof: run drivers | test a_resistor_divider_has_its_thermal_noise_on_devices ... ok
Proof: run drivers | test active_limiter_vetoes_newton_convergence ... ok
Proof: run drivers | test gmin_stepping_reaches_a_device_through_the_ambient_input ... ok
Proof: run drivers | test source_stepping_scales_sources_through_the_ambient_input ... ok

**C22** - The smallest `CAP` bounds the next step and a fired `CROSS` is committed at the landing (SAI-11, Protocol AC 22)
Proof: run crossings | test a_cap_watch_bounds_the_next_step ... ok
Proof: run crossings | test a_fired_crossing_is_a_commit_at_the_landing ... ok

**C23** - The numeric assertions of `parity_baseline` are byte-identical to the base commit and the suite passes (SAI-12, Port AC 1)
Proof: `a=$(mktemp); b=$(mktemp); git show 98b30ad6:crates/piperine-solver/tests/parity_baseline.rs | grep -E 'abs\(\) <|res\.len\(\)' > "$a"; grep -E 'abs\(\) <|res\.len\(\)' crates/piperine-solver/tests/parity_baseline.rs > "$b"; test -s "$a" && diff "$a" "$b" && cargo test -p piperine-solver`

**C24** - Every removed test is listed in `removed.txt` with the item it was about, and none of them exists (SAI-12, Port AC 2) (retired by code-clarity T9: a guard against names a past refactor removed; no way back to them)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C25** - No source of the solver crates names an item of the `Element` contract (SAI-14, Port AC 5) (retired by code-clarity T9: a guard against names a past refactor removed; no way back to them)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C26** - No source names the strength table, `LogicValue`, the two-phase contract, `DigitalState` or `EventQueue` (SAI-14, Port AC 5) (retired by code-clarity T9: a guard against names a past refactor removed; no way back to them)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C27** - Every function of every test device passes the partials verifier (SAI-15, Port AC 4): the helper libraries at a table of operating points and at generic points, each target's own devices (`parity_baseline`'s among them) in that target, and a guard names every hand-written device of the suite in a check (a failure fixture is held to its declared failure)
Proof: run solver | test every_device_function_passes_the_partials_verifier ... ok
Proof: run solver | test every_helper_device_passes_at_generic_points ... ok
Proof: run solver | test every_hand_written_device_is_in_a_partials_check ... ok
Proof: run solver | test its_devices_pass_the_partials_verifier ... ok
Proof: `o=$(cargo test -p piperine-solver 2>&1); n=$(printf '%s\n' "$o" | grep -cE '^test ([A-Za-z0-9_]+::)*its_devices_pass_the_partials_verifier \.\.\. ok$'); m=$(grep -rhow --include='*.rs' 'fn its_devices_pass_the_partials_verifier' crates/piperine-solver/tests | wc -l); test "$m" -gt 20 && test "$n" -eq "$m" || { echo "$n of $m declarations"; exit 1; }`

**C28** - The analog sub-solver names no digital type (SAI-14, Port AC 6)
Proof: run guards | test analog_sources_name_no_digital_type ... ok

**C29** - The digital sub-solver names no analog type (SAI-14, Port AC 6)
Proof: run guards | test digital_sources_name_no_analog_type ... ok

**C30** - The four crates build with zero warnings and their suites pass (SAI-16, Port AC 7)
Proof: `cargo test -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver`

---

## Handoff

**Settled mid-build:** round-1 fix (orchestrator 2026-11-15): F7 lost capabilities are RESTORED, not recorded as removed - per-step digital net values in TransientStep (as bytes + net names) and re-entry from any captured step; F1 slack strictly under one tick and never past a CAP bound; F5/F6 restore the vanished/weakened tests or log them; F3/F4/F9 tighten proofs; F2/F8 docs; T5 divergence (orchestrator 2026-11-15): tolerances are NOT loosened; an Opus diagnosis finds the cause in the device transient path first (parity is the user priority); T5 resumes after; none
**Dispatched:** B1 (Sonnet) at 43876aec - returned (T1-T4 84457807..6fec3889 + docs e4d45ebf; numbers unchanged (parity_baseline 161 steps); instance created twice (one per side)); B2 (Sonnet) at 1aa57598 - returned (T5 c2d99474, 8d6a8b26 (Opus diagnosis: resistor double value vs partial products; bit-identical now)); B2 cont. T6+ (Sonnet) at d5aa7cf4 - returned (B2 T6-T10 fda2979d..a9a23f55 + param_stamps fbe7f2e8; pz device failure -> NaN matrix (silent)); B3 (Sonnet) at 2876b0b7 - returned (pz fail-loud 6a1bf00d; Phase 3 needs ownership decision); Phase 3 T11-T15 (Opus: protocol wiring is delicate) at 4526dca1 - returned (Phase 3 T11-T15 7f629a93..f4fe80a4 + digital waiter fix 4e4e3f4a; all oracles green); Phase 4-5 (Opus, continuing) at db10b406 - returned (decisions 6c17c608, cd460575; T16 97ab2477 (25 mixed_signal tests removed, subject deleted), T17 740424ff parity_baseline on devices 161 steps); T18 (fresh Opus) at b546cf4b - returned (T18 ab22f6fc, 2bb1ee46, 6ed35535 (.sens, re-entry, pss on devices); non-zero start without captured step fails loud (digital init only at tick 0) - accepted as a known gap, ROADMAP line in T25); T19-T25 (Sonnet) at 88e0ac00 - returned (T19-T25 f5b1e2ec..660b1a56; Element + old digital engine deleted; MD-62; live_params landing now asserts LEFT value (user order); floor_tick relative slack fix); Verifier (Opus) at 0c22ca7e - returned (FAIL: 9 findings, no number moved); Fix pass (Opus) at f928e331 - returned (fix pass 294bbc05..9806e693; F1-F9 closed; capabilities restored); Verifier round 2 at ec9c5007 - returned (PASS; N1-N3 to ROADMAP 88457a41)
**Settled mid-build:** one device instance per placement, shared by both sides (an external device such as an emulator holds state; two copies would diverge) - fix in B2 (orchestrator)
**Settled mid-build:** pz must fail loud on a device-evaluation failure (project rule: never silent); fix as first commit of B3 (orchestrator)
**Settled mid-build:** Phase 3 ownership: the run moves Digital out of Bound (mem::replace with an empty Digital) for its length and puts it back after, so Protocol<D, A> owns both sides by value; Phase 3 goes to an Opus implementer (delicate wiring of the numeric oracles) (orchestrator)
**Settled mid-build:** (a) AC bias point goes through the protocol operating point (a mixed design's bias includes its settled discrete side); abi_ac.rs's START-count window is this feature's own test and is corrected; (b) spec AC 6 and T12 criteria marked superseded by the user's order (solve on the left value); (c) tick-grid tolerance of the controller's proposal, the exempt step after a boundary change and full re-bind accepted (orchestrator)
**Walked:** not applicable - no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1 → T2 → T3 → T4
Phase 2:  T5 → T6 → T7 → T8 → T9 → T10
Phase 3:  T11 → T12 → T13 → T14 → T15
Phase 4:  T16 → T17 → T18 → T19 → T20 → T21
Phase 5:  T22 → T23 → T24 → T25
```

Batches: one per phase. Phase 2 starts after T4 because the oracle compares drivers; Phase 3 after T10 because the protocol must not move a number the oracle has recorded; Phase 4 after T15 because every port needs the protocol; Phase 5 after T21 because nothing may be deleted while a test still uses it.

## Task Granularity Check

Each task is one driver, one adapter, or one family of test targets ported under one rule. The port tasks (T16 to T21) and the two removal tasks (T22, T23) touch several files because their unit is the rule applied, not the file: splitting T22 would leave a tree that does not compile. T1 creates `bind.rs` and wires it into the builder and the circuit in one commit for the same reason.

## Test Co-location Validation

Every task that adds code adds its tests in the same commit. Port tasks are test changes by nature. T22 and T23 delete code and add the guard test that proves the deletion. T25 edits documents guarded by `workspace_shape.rs`, which it extends. No task says `Tests: none`.

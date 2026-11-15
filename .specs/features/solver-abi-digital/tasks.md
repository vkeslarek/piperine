# Solver ABI Digital Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-abi-digital/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-digital-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run digital**: `cargo test -p piperine-digital-solver`

Executors: Sonnet sub-agents by default, one commit per task; verifier: Opus. The orchestrator may have Opus implement instead (a different Opus then verifies): the feature is protocol-delicate (ordering, transport, look-ahead), not numerically delicate, 13 tasks. Work in `/home/keslarek/Git/piperine-solver-completeness`, branch `feature/solver-completeness`. Never pass `--workspace`, never run `cargo clean`, do not set `CARGO_TARGET_DIR`, do not push. Starts after `solver-abi-core` is done (the crate `piperine-abi`, MD-61 and the `DiscreteSide` trait exist). Runs in parallel with `solver-abi-analog`, in its own worktree branched from the last commit of `solver-abi-core`: **no file outside `crates/piperine-digital-solver` is edited**, so the two branches merge without conflict.

Rules every task follows:

- **abi.md is the contract.** Rule names (P, F, R, C, D) are its own; a test cites the rule it proves in a comment.
- **The old engine is not touched.** No edit to `src/{drivers,events,interface,scheduler,state,topology}.rs`; `limits.rs` and `error.rs` only gain items. The 27 existing unit tests pass unchanged after every commit.
- **No analog type, no `unsafe`, no owner.** The new modules name nothing of `piperine-analog-solver` or `piperine-solver`, and call devices only through `piperine_abi::caller::Handle`.
- **No macro** for tables, devices or helpers: data and plain functions.
- **New tests go in integration targets** under `crates/piperine-digital-solver/tests/`, so the library's own unit-test count stays 27 (check C45 reads that number).
- Tests derive from the spec's criteria and abi.md's rules, never from the implementation. Test functions have exactly the names the checks cite. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on `piperine-digital-solver` and `piperine-solver`.
- Commit message as given per task, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("zero warnings is the bar"), `.specs/STATE.md` MD-28, MD-31, MD-33, MD-60; user: unit tests only.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Engine state (`tick`, `plan`, `store`, `queue`, `dag`) | unit | each AC of SAD-02 to SAD-05 and SAD-09's ordering and transport; every bind error has a planted case | `crates/piperine-digital-solver/tests/{ticks,bind,store,queue,dag}.rs` | `cargo test -p piperine-digital-solver` |
| Engine execution (`run`, `instant`, `advance`, `engine`, `observe`) | unit | each AC of SAD-06 to SAD-22 and each edge case of the spec, on hand-written `Device`s | `crates/piperine-digital-solver/tests/{functions,reactions,instant,checkpoint,advance,host}.rs` | `cargo test -p piperine-digital-solver` |
| Fixture devices | unit | each device passes `piperine_abi::validate` | `crates/piperine-digital-solver/tests/support/devices.rs`, `crates/piperine-digital-solver/tests/devices.rs` | `cargo test -p piperine-digital-solver --test devices` |
| The old engine | unit | its 27 tests, unchanged | `crates/piperine-digital-solver/src/*.rs` (`#[cfg(test)]`) | `cargo test -p piperine-digital-solver --lib` |
| Boundary guard of the crate | unit | the manifest depends on `piperine-abi` only; the new modules name no analog or owner item and hold no `unsafe` | `crates/piperine-digital-solver/tests/boundary.rs` | `cargo test -p piperine-digital-solver --test boundary` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | never alone | `cargo test -p piperine-digital-solver --test <the task's target>` |
| Full | every task | `cargo test -p piperine-digital-solver` (27 old tests plus the new ones) |
| Build | last task of each phase | Full, plus `cargo test -p piperine-solver` unedited (the owner still runs on the old engine: nothing may move), plus the two `**Build**` commands, plus `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Phases are ordered and run sequentially. Two batches, one per phase (6 and 7 tasks). No task is numerically sensitive: the engine is discrete and the owner does not use it yet.

### Phase 1: State: ticks, nets, queue, order

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Execution: functions, reactions, instants, look-ahead

```
T7 → T8 → T9 → T10 → T11 → T12 → T13
```

---

## Task Breakdown

### T1: Ticks, typed errors and the dependency on `piperine-abi`

**What**: Integer time (`Tick`, `Resolution`, `Due`) and the typed variants of `DigitalError`.
**Where**: `crates/piperine-digital-solver/src/abi/tick.rs` (+ `crates/piperine-digital-solver/Cargo.toml` dependency, `src/lib.rs` `pub mod abi`, `src/error.rs` new variants, target `tests/ticks.rs`)
**Depends on**: None
**Reuses**: design `tick`, "Error Handling Strategy"; `src/error.rs` (its two variants and `Display` stay)
**Requirement**: SAD-01, SAD-02, SAD-03

**Done when**:

- [ ] `piperine-digital-solver` depends on `piperine-abi` and nothing else in the tree; `cargo test -p piperine-solver --test crate_boundaries` passes unedited (`sub_solvers_depend_on_the_abi_crate_only`)
- [ ] Test `a_delay_is_due_at_the_nearest_tick` passes for delays below, at and above half a tick (C1)
- [ ] Test `a_time_outside_the_tick_range_is_e0502` passes (C2)
- [ ] Test `a_nan_delay_is_e0435` and test `an_infinite_delay_never_comes_due` pass (C3, C4)
- [ ] Test `a_non_advancing_delay_is_the_next_microstep` passes for 0, negative and a quarter of a tick
- [ ] Test `the_default_tick_is_one_femtosecond` passes
- [ ] `error::tests::display_is_the_scheduler_message_text` still passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): integer ticks and typed run errors for the ABI engine`

---

### T2: The byte store

**What**: `Store`: bytes per net with `PREVIOUS`, `CHANGED`, `fired` and a microstep stamp.
**Where**: `crates/piperine-digital-solver/src/abi/store.rs` (+ target `tests/store.rs`)
**Depends on**: T1
**Reuses**: design `store`; `piperine_abi::layout::is_canonical` for the debug check
**Requirement**: SAD-04

**Done when**:

- [ ] Test `a_net_starts_as_zero_bytes` passes (C5)
- [ ] Test `a_write_of_equal_bytes_is_not_a_change` and test `a_change_keeps_the_previous_bytes` pass (C6); `0.0` to `-0.0` is a change and the same NaN bytes are not
- [ ] Test `changed_reads_one_for_exactly_one_microstep` passes (C7)
- [ ] Test `an_event_net_fires_without_bytes` passes
- [ ] Test `a_quiet_net_keeps_no_previous` passes
- [ ] `Store` is `Clone` and a clone compares equal byte for byte (used by the checkpoint)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): byte store with previous and changed parts`

---

### T3: The commit queue

**What**: `Queue`: pending commits ordered by due instant, with C2 order and C3 transport.
**Where**: `crates/piperine-digital-solver/src/abi/queue.rs` (+ target `tests/queue.rs`)
**Depends on**: T2
**Reuses**: design `queue`; abi.md §6 C1 to C3
**Requirement**: SAD-09

**Done when**:

- [ ] Test `commits_apply_in_made_statement_call_order` passes with three commits to overlapping bytes made in a shuffled order (C13)
- [ ] Test `transport_removes_later_commits_of_the_same_statement_only` passes: a later-due commit of the same statement and range goes, an earlier-due one stays, another statement's stays (C14)
- [ ] Test `replace_removes_every_pending_commit_of_the_statement` passes (C15)
- [ ] Test `disjoint_byte_ranges_of_one_statement_do_not_remove_each_other` passes
- [ ] Test `next_due_is_the_earliest_instant` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): commit queue with ordering and transport`

---

### T4: The plan and the discrete half of bind

**What**: `DigitalPlan::bind`: net specs, pin bindings, the writer of each net (P1), layout equality (P4), unbound globals, `LATER` against the resolution.
**Where**: `crates/piperine-digital-solver/src/abi/plan.rs` (+ target `tests/bind.rs`)
**Depends on**: T3
**Reuses**: design `plan`; `piperine_abi::{validate, layout::layout_eq, shape::ShapeBuf}` (fixtures are bare shapes with no device behind them where the rule needs none)
**Requirement**: SAD-05, SAD-10 (AC 14)

**Done when**:

- [ ] Test `two_writers_of_one_net_are_refused_naming_both` passes for function against function and function against statement (C8)
- [ ] Test `unequal_layouts_on_one_net_are_refused` passes (C9)
- [ ] Test `an_unbound_global_is_refused_unless_optional` passes, with the `OPTIONAL` pin reading zero bytes
- [ ] Test `later_is_dropped_when_its_bound_is_under_one_tick` passes at resolutions of 1 fs and 1 ns (C18)
- [ ] Test `a_shape_that_fails_validation_fails_bind` passes
- [ ] Test `a_net_nobody_writes_is_written_by_the_host` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): plan and discrete bind rules`

---

### T5: The levelized order

**What**: `Dag`: function levels, strongly connected components, `RELAX` groups, the cycle error.
**Where**: `crates/piperine-digital-solver/src/abi/dag.rs` (+ target `tests/dag.rs`)
**Depends on**: T4
**Reuses**: design `dag`; `src/topology.rs` read for the ordering idea only
**Requirement**: SAD-05 (AC 10), SAD-06, SAD-07

**Done when**:

- [ ] Test `a_function_runs_after_every_writer_of_its_reads` passes on a diamond and a chain of ten built in a shuffled order (C11, first half)
- [ ] Test `a_cycle_through_no_relax_net_is_refused_naming_its_functions` passes for two cross-coupled gates and for a self-loop (C10)
- [ ] Test `a_cycle_through_relax_nets_is_one_relax_step` passes
- [ ] Test `the_order_is_stable_across_runs` passes: the same input gives the same order (instance index, then function index, break ties)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): levelized function order with relax groups`

---

### T6: Fixture devices

**What**: Hand-written `Device`s for the engine's tests, and one helper that binds a list of them by net name.
**Where**: `crates/piperine-digital-solver/tests/support/devices.rs` (+ target `tests/devices.rs`)
**Depends on**: T5
**Reuses**: `crates/piperine-abi/tests/support/examples.rs` (flip-flop, comparator); abi.md §11.4, §11.7
**Requirement**: SAD-06, SAD-08

**Done when**:

- [ ] Devices: `Inverter` and `And2` (one function), `Dff` (abi.md §11.4), `Timer` (a `START` reaction and a delayed `REPLACE` commit to a private event pin, toggling a `BIT` output), `Counter` (state in a private pin), `Resolver` (two `RELAX` drive pins, one value pin, wired-or with unknown), `Recorder` (a `RECORD` pin), `Checked` (a param with a `check` pin and a setup function that returns `RESHAPE` above a threshold), `Sampler` (a reaction that reads a continuous pin), `Stub` (an `IRREVERSIBLE` reaction)
- [ ] `bind_by_name(devices, wires) -> DigitalPlan` is a plain function taking a list of (device, pin name to net name); no macro
- [ ] Test `every_fixture_shape_is_valid` passes
- [ ] Test `fixtures_bind_into_a_plan` binds a timer, a flip-flop and an inverter
- [ ] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `test(digital): fixture devices for the ABI engine`

---

### T7: Running functions to a fixed point

**What**: `run_functions` (D): dirty-driven, level by level, with relaxation of `RELAX` groups.
**Where**: `crates/piperine-digital-solver/src/abi/run.rs` (+ target `tests/functions.rs`)
**Depends on**: None (phase order: starts after T6 closes the previous phase)
**Reuses**: design `run` (D); `Dag`, `Store`, `caller::Handle`
**Requirement**: SAD-06, SAD-07, SAD-13

**Done when**:

- [ ] Test `a_function_runs_when_a_read_net_changed` and test `a_function_with_unchanged_inputs_is_not_called` pass, counting calls in the device (C11, C12)
- [ ] Test `every_function_runs_once_at_the_first_instant` passes
- [ ] Test `a_relax_group_settles_within_its_bound` passes on the resolver with two drivers (C16)
- [ ] Test `a_moving_known_field_is_zeroed_and_observed` and test `a_moving_field_without_a_known_byte_fails` pass (C17)
- [ ] Test `a_function_fault_carries_code_tick_and_instance` passes; `RESHAPE` from a function outside the cone is the same error (C25)
- [ ] Test `changed_is_zero_one_microstep_later` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): dirty-driven function evaluation with relaxation`

---

### T8: Running reactions and taking commits

**What**: `run_reactions` (R): the snapshot, the commit path with its checks, `FINISH`.
**Where**: `crates/piperine-digital-solver/src/abi/run.rs` (+ target `tests/reactions.rs`)
**Depends on**: T7
**Reuses**: design `run` (R); `Queue`, `Resolution::due`
**Requirement**: SAD-08, SAD-09 (AC 8), SAD-10, SAD-13

**Done when**:

- [ ] Test `a_reaction_runs_once_when_its_event_fired` passes (C19)
- [ ] Test `reactions_of_one_microstep_read_one_snapshot` passes: two flip-flops in a ring swap values (C20)
- [ ] Test `a_zero_delay_commit_applies_at_the_next_microstep` passes (C21)
- [ ] Test `a_commit_outside_its_statement_range_is_a_fault` and test `a_later_commit_below_its_bound_is_a_fault` pass (C22)
- [ ] Test `finish_marks_the_instant_final` passes (C26)
- [ ] Test `a_reaction_fault_carries_code_tick_and_instance` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): reactions on a snapshot and the commit path`

---

### T9: The microstep loop and the start

**What**: `Digital::start` and `run_instant`: apply, D, R, repeat; the bound; `QUIET`; `SETTLE`.
**Where**: `crates/piperine-digital-solver/src/abi/instant.rs` (+ `engine.rs` for the `Digital` struct, target `tests/instant.rs`)
**Depends on**: T8
**Reuses**: design `instant, advance, engine`, the instant flowchart; `limits.rs` (`max_delta_cycles`)
**Requirement**: SAD-11, SAD-12, SAD-16

**Done when**:

- [ ] Test `the_traced_instant_of_section_15_1` reproduces the table of `docs/spec/part_v_execution.md` §15.1 with `Timer`, `Dff` and a commit standing for the crossing: applied nets, fired events and commits per microstep (C23)
- [ ] Test `an_instant_that_never_goes_quiet_is_e0501_naming_the_nets` passes with a reaction that toggles its own trigger (C24)
- [ ] Test `a_quiet_commit_does_not_start_a_microstep` passes (C27)
- [ ] Test `start_commits_inits_and_fires_start` passes (C28)
- [ ] Test `a_settle_commit_with_no_delay_is_refused` passes (C29)
- [ ] Test `an_instant_reports_boundary_fired_and_final` passes with `Sampler` reading its sample (C33)
- [ ] Test `a_plan_with_no_code_applies_host_commits_only` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): the microstep loop, start and settle`

---

### T10: Checkpoint, restore and observations

**What**: `checkpoint`, `restore`, the `Observation` buffer and its order.
**Where**: `crates/piperine-digital-solver/src/abi/observe.rs` (+ `engine.rs`, target `tests/checkpoint.rs`)
**Depends on**: T9
**Reuses**: design `observe`, `engine`
**Requirement**: SAD-14, SAD-20

**Done when**:

- [ ] Test `restore_returns_store_queue_and_observations_byte_for_byte` passes and counts zero device calls during `restore` (C30)
- [ ] Test `two_runs_give_identical_observations` passes on a design with five instances built in two different insertion orders of equal paths (C31)
- [ ] Test `every_change_commit_and_record_is_observed_in_order` passes; eight records through one statement give eight `Record`s (C37)
- [ ] Test `observations_after_a_checkpoint_are_gone_after_restore` passes (C38)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): checkpoint, restore and the observation buffer`

---

### T11: Look-ahead: the time-point test and `advance`

**What**: Static reachability at bind, the dynamic test R4, `advance` with a sub-checkpoint per instant, `HORIZON`.
**Where**: `crates/piperine-digital-solver/src/abi/advance.rs` (+ `plan.rs` for the reachability set, target `tests/advance.rs`)
**Depends on**: T10
**Reuses**: design `advance` flowchart, R4 paragraph; abi.md §5 R4 to R6 and protocol step 2
**Requirement**: SAD-15, SAD-17

**Done when**:

- [ ] Test `advance_stops_before_the_first_time_point_and_leaves_it_untouched` passes: after `Landing(s)` the store, queue and observations equal their state before instant `s` (C32)
- [ ] Test `each_clause_of_the_time_point_test_stops_advance` passes with one case per clause: an analog-read net changes; a reaction with a sample read fires; a reaction with a non-`LATER` statement that reaches an analog-read net fires; an `IRREVERSIBLE` reaction fires (C34, first half)
- [ ] Test `a_free_running_clock_does_not_stop_advance` runs 1000 edges of a `Timer` nobody on the analog side reads inside one `advance` (C34, second half)
- [ ] Test `a_later_statement_does_not_make_its_instant_a_time_point` passes, and its due instant is tested on its own
- [ ] Test `a_landing_found_at_a_later_microstep_restores_the_whole_instant` passes (abi.md §5 step 2: a firing at microstep `n > 0`)
- [ ] Test `horizon_is_before_the_earliest_pending_commit_and_the_limit` and test `tick_is_the_instants_tick` pass (C35)
- [ ] Test `a_commit_due_after_the_target_stays_pending` and test `times_that_round_to_one_tick_are_one_instant` pass (C36)
- [ ] Test `advance_returns_final_at_a_finish` passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): time-point test, advance and horizon`

---

### T12: Host commits, the parameter cone and the shadow

**What**: `host_commit`, the cone computed at bind, `shadow`, `preview_boundary`.
**Where**: `crates/piperine-digital-solver/src/abi/engine.rs` (+ `plan.rs` for the cone, target `tests/host.rs`)
**Depends on**: T11
**Reuses**: design `plan` (cone), `engine`; abi.md P5, P6, C6
**Requirement**: SAD-18, SAD-19, SAD-22

**Done when**:

- [ ] Test `a_host_write_is_a_commit_at_microstep_zero` passes (C39)
- [ ] Test `a_structural_param_write_during_a_run_is_refused` passes (C40)
- [ ] Test `the_cone_is_the_functions_fed_by_params_and_globals_only` passes: a function with an input reachable from a statement or a written port is outside it
- [ ] Test `a_false_check_refuses_the_write_and_changes_nothing` passes with `Checked` (C41)
- [ ] Test `reshape_on_the_shadow_names_the_instance_and_changes_nothing` passes (C42)
- [ ] Test `preview_boundary_applies_microstep_zero_on_a_copy` passes: the returned bytes are the post-commit values, the store is unchanged, no reaction ran, nothing was observed (C43)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(digital): host commits, parameter cone and shadow`

---

### T13: Boundary guard and the facade's fit

**What**: A guard target of this crate: dependency, no analog or owner item, no `unsafe`, the old files untouched; and a compile-time proof that `Digital` offers every operation of `DiscreteSide`.
**Where**: `crates/piperine-digital-solver/tests/boundary.rs`
**Depends on**: T12
**Reuses**: the scan style of `crates/piperine-solver/tests/crate_boundaries.rs` (plain `std::fs`); `.specs/features/solver-abi-core/design.md` component `protocol` for the trait's method list
**Requirement**: SAD-01, SAD-21

**Done when**:

- [ ] Test `the_crate_depends_on_the_abi_crate_only` reads this crate's manifest
- [ ] Test `abi_modules_name_no_analog_or_owner_item` scans `src/abi/` for `piperine_analog_solver`, `piperine_solver`, `Netlist`, `Stamp`, `AnalogReference` (C44); test `abi_modules_hold_no_unsafe` passes
- [ ] Test `the_facade_offers_every_operation_of_the_discrete_side` calls each of `start`, `next_due`, `advance`, `preview_boundary`, `fire`, `run_instant`, `checkpoint`, `restore`, `shadow`, `host_commit`, `drain_observations` once on a small plan, with the argument and result shapes the trait of `solver-abi-core` uses after the obvious conversions (`Tick` and `i64`, `NetId` and `u32`)
- [ ] `git diff <base>..HEAD --stat -- . ':!crates/piperine-digital-solver'` is empty, and so is the diff of `src/{drivers,events,interface,scheduler,state,topology}.rs`, where `<base>` is the commit before T1; the commit body states the base hash
- [ ] `cargo test -p piperine-digital-solver --lib` reports the 27 old tests passing (C45), and `cargo test -p piperine-solver` is green
- [ ] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `test(digital): boundary guard and facade fit of the ABI engine`

---

## Checks

**C1** - A positive finite delay is due at the nearest tick (SAD-02, AC 3)
Proof: run digital | test a_delay_is_due_at_the_nearest_tick ... ok

**C2** - A time outside the tick range fails with `E0502` (SAD-02, AC 4)
Proof: run digital | test a_time_outside_the_tick_range_is_e0502 ... ok

**C3** - A NaN delay fails with `E0435` (SAD-03, AC 5)
Proof: run digital | test a_nan_delay_is_e0435 ... ok

**C4** - A `+inf` delay never comes due (SAD-03, AC 6)
Proof: run digital | test an_infinite_delay_never_comes_due ... ok

**C5** - A net is `layout.size` bytes that start all zero (SAD-04, AC 7)
Proof: run digital | test a_net_starts_as_zero_bytes ... ok

**C6** - A net changes exactly when its bytes differ, and `PREVIOUS` holds the bytes before (SAD-04, AC 7 and 8)
Proof: run digital | test a_change_keeps_the_previous_bytes ... ok

**C7** - `CHANGED` reads 1 in the microstep of the change and 0 in the next (SAD-04, AC 8)
Proof: run digital | test changed_reads_one_for_exactly_one_microstep ... ok

**C8** - Two writers of one net are refused naming both pins (SAD-05, AC 9)
Proof: run digital | test two_writers_of_one_net_are_refused_naming_both ... ok

**C9** - Unequal layouts on one net are refused (SAD-05, AC 9)
Proof: run digital | test unequal_layouts_on_one_net_are_refused ... ok

**C10** - A cycle through no `RELAX` net is refused naming its functions (SAD-05, AC 10)
Proof: run digital | test a_cycle_through_no_relax_net_is_refused_naming_its_functions ... ok

**C11** - A function runs in the microstep a read pin changed, after every writer of its reads (SAD-06, AC 1)
Proof: run digital | test a_function_runs_when_a_read_net_changed ... ok

**C12** - A function with unchanged inputs is not called (SAD-06, AC 2)
Proof: run digital | test a_function_with_unchanged_inputs_is_not_called ... ok

**C13** - Overlapping commits apply in (instant made, statement index, call order) and the last is the value (SAD-09, AC 9)
Proof: run digital | test commits_apply_in_made_statement_call_order ... ok

**C14** - Transport removes later-due commits of the same statement and range only (SAD-09, AC 10)
Proof: run digital | test transport_removes_later_commits_of_the_same_statement_only ... ok

**C15** - `REPLACE` removes every pending commit of its statement (SAD-09, AC 11)
Proof: run digital | test replace_removes_every_pending_commit_of_the_statement ... ok

**C16** - A `RELAX` cycle starts at zero bytes and settles within (nets + 1) passes (SAD-07, AC 3)
Proof: run digital | test a_relax_group_settles_within_its_bound ... ok

**C17** - A still-moving field with a known byte is zeroed and observed; one without fails (SAD-07, AC 4 and 5)
Proof: run digital | test a_moving_known_field_is_zeroed_and_observed ... ok

**C18** - `LATER` is dropped at bind when `min_after` is under one tick (SAD-10, AC 14)
Proof: run digital | test later_is_dropped_when_its_bound_is_under_one_tick ... ok

**C19** - A reaction runs once in a microstep in which an event it listens to fired (SAD-08, AC 6)
Proof: run digital | test a_reaction_runs_once_when_its_event_fired ... ok

**C20** - The reactions of one microstep read one snapshot (SAD-08, AC 7)
Proof: run digital | test reactions_of_one_microstep_read_one_snapshot ... ok

**C21** - A commit with no advancing delay applies at the next microstep (SAD-09, AC 8)
Proof: run digital | test a_zero_delay_commit_applies_at_the_next_microstep ... ok

**C22** - A commit outside its statement's range, or a `LATER` commit under its bound, is a fault naming the statement (SAD-10, AC 12 and 13)
Proof: run digital | test a_commit_outside_its_statement_range_is_a_fault ... ok

**C23** - The traced instant of §15.1 is reproduced microstep by microstep (SAD-08, SAD-09, Independent Test)
Proof: run digital | test the_traced_instant_of_section_15_1 ... ok

**C24** - An instant that does not go quiet in 1000 microsteps fails with `E0501` naming the nets (SAD-11, AC 15)
Proof: run digital | test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

**C25** - A device fault carries its code, the tick and the instance path; `RESHAPE` outside the cone is the same (SAD-13, AC 19 and 20)
Proof: run digital | test a_function_fault_carries_code_tick_and_instance ... ok

**C26** - `FINISH` marks the instant final (SAD-13, AC 21)
Proof: run digital | test finish_marks_the_instant_final ... ok

**C27** - A commit to a `QUIET` net applies as the instant exits and starts no microstep (SAD-11, AC 16)
Proof: run digital | test a_quiet_commit_does_not_start_a_microstep ... ok

**C28** - A start zeroes the store, commits every `init` at `(0, 0)` and fires `START` (SAD-12, AC 17)
Proof: run digital | test start_commits_inits_and_fires_start ... ok

**C29** - A settle commit with no advancing delay is refused as unsupported (SAD-12, AC 18)
Proof: run digital | test a_settle_commit_with_no_delay_is_refused ... ok

**C30** - Restore returns store, queue and observations byte for byte with no device call (SAD-14, AC 1)
Proof: run digital | test restore_returns_store_queue_and_observations_byte_for_byte ... ok

**C31** - Two runs of the same plan and host commits give identical observations (SAD-14, AC 2)
Proof: run digital | test two_runs_give_identical_observations ... ok

**C32** - `advance` stops before the first time point and leaves that instant untouched (SAD-15, AC 3)
Proof: run digital | test advance_stops_before_the_first_time_point_and_leaves_it_untouched ... ok

**C33** - An instant run with samples reports boundary nets, fired nets and finality, and a sampling reaction reads its sample (SAD-16, AC 6)
Proof: run digital | test an_instant_reports_boundary_fired_and_final ... ok

**C34** - Each clause of the time-point test stops `advance`, and an instant that reaches nothing analog does not (SAD-15, AC 4 and 5)
Proof: run digital | test each_clause_of_the_time_point_test_stops_advance ... ok

**C35** - `TICK` is the instant's tick and `HORIZON` is before the earliest pending commit and the owner's limit (SAD-17, AC 7)
Proof: run digital | test horizon_is_before_the_earliest_pending_commit_and_the_limit ... ok

**C36** - A commit due after the target stays pending; times that round to one tick are one instant (SAD-15, AC 8 and 9)
Proof: run digital | test a_commit_due_after_the_target_stays_pending ... ok

**C37** - Every change, commit and record is observed in (instant, instance path, statement) order (SAD-20, AC 6)
Proof: run digital | test every_change_commit_and_record_is_observed_in_order ... ok

**C38** - Observations made after a checkpoint are gone after restore (SAD-20, AC 7)
Proof: run digital | test observations_after_a_checkpoint_are_gone_after_restore ... ok

**C39** - A host write is a commit due at microstep 0 of its tick (SAD-19, AC 1)
Proof: run digital | test a_host_write_is_a_commit_at_microstep_zero ... ok

**C40** - A write to a `STRUCTURAL` param during a run is refused (SAD-19, AC 2)
Proof: run digital | test a_structural_param_write_during_a_run_is_refused ... ok

**C41** - A false `check` on the shadow refuses the write and changes nothing (SAD-18, AC 3 and 4)
Proof: run digital | test a_false_check_refuses_the_write_and_changes_nothing ... ok

**C42** - `RESHAPE` on the shadow names the instance and changes nothing (SAD-18, AC 5)
Proof: run digital | test reshape_on_the_shadow_names_the_instance_and_changes_nothing ... ok

**C43** - The boundary preview of a landing applies microstep 0 on a copy, runs no reaction and changes nothing (SAD-22, AC 10)
Proof: run digital | test preview_boundary_applies_microstep_zero_on_a_copy ... ok

**C44** - The digital crate depends on `piperine-abi` only and its new modules name no analog or owner item (SAD-01, AC 1)
Proof: run digital | test abi_modules_name_no_analog_or_owner_item ... ok

**C45** - The old engine's unit tests pass unchanged (SAD-21)
Proof: `cargo test -p piperine-digital-solver --lib 2>&1 | grep -E '^test result: ok\. 27 passed'`

---

## Handoff

**Settled mid-build:** none
**Dispatched:** none
**Walked:** not applicable - no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T7 → T8 → T9 → T10 → T11 → T12 → T13
```

Batches: Phase 1 (6 tasks), Phase 2 (7 tasks). Phase 2 starts after T6 because every execution test runs fixture devices on a bound plan.

## Task Granularity Check

Each task is one module of the engine with its own test target. T1 also adds the manifest dependency and the module root because `tick.rs` cannot compile without them. T8 extends `run.rs` (created by T7) with the reaction half: two tasks on one file because functions and reactions are separate rules (F against R and C) with separate tests. T9, T10 and T12 each add methods to `engine.rs`, the facade, next to the module that implements them.

## Test Co-location Validation

Every task adds its tests in the same commit, in the target the matrix names. T6 is test support with its own target (`devices.rs`). T13 is a guard inside this crate: its test is the deliverable. No task says `Tests: none`.

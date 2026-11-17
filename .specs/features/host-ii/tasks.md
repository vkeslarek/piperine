# Host II Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/host-ii/design.md`
**Status**: Approved
**Approved**: 2026-11-17 - user: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando"
**Build**: `cargo build -p piperine-solver -p piperine-plan -p piperine-transpile -p piperine-api -p piperine -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run solverbounds**: `cargo test -p piperine-solver --test session_bounds`
**Run solverhost**: `cargo test -p piperine-solver --test host_writes`
**Run baseline**: `cargo test -p piperine-solver --test parity_baseline`
**Run planlib**: `cargo test -p piperine-plan --lib`
**Run transpile**: `cargo test -p piperine-transpile --test derive`
**Run paritylib**: `cargo test -p piperine-parity --lib`
**Run parity**: `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test parity_grid --test parity_corpus`
**Run newparity**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity_temp --test parity_tf --test parity_pz --test parity_sens --test parity_pss -- --nocapture`
**Run apilib**: `cargo test -p piperine-api --lib`
**Run apioptions**: `cargo test -p piperine-api --test options`
**Run apianalyses**: `cargo test -p piperine-api --test analyses`
**Run apimeta**: `cargo test -p piperine-api --test metadata`
**Run apihost**: `cargo test -p piperine-api --test host_actions`
**Run apidigital**: `cargo test -p piperine-api --test digital_only`
**Run apiderived**: `cargo test -p piperine-api --test derived`
**Run clilib**: `cargo test -p piperine-cli --lib`
**Run clianalyses**: `cargo test -p piperine-cli --test analyses`
**Run cliinfo**: `cargo test -p piperine-cli --test info`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run clicommands**: `cargo test -p piperine-cli --test commands`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run root**: `cargo test --test host_ii`

Executors: every task is Sonnet (user rule). A different model verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources.

**Questions answered** (user, 2026-11-17: "Aceito todas (Recommended)"): T23 is Q1 (a), T27 is Q2 (a), T11 and T20 are Q3 (a); `tnom` stays out.

Rules every task follows:

- **Parity is untouched.** `tests/ngspice/` existing twins and goldens, `crates/piperine-cli/tests/goldens/`, the existing `parity_*` targets and every existing constant of `crates/piperine-parity/src/contract.rs` are not edited; new constants and new twins are added only by T8, T10, T14 to T16 and T19. No tolerance is retuned; a miss is reported to the user, never a moved bound.
- **Defaults do not move.** With no new key given a run builds `Context::default()` and the backend's own `circuit()`, and prints what it printed (HII-02 AC 1, HII-21).
- **Cross-lane touches are only those `design.md` names** (T2, T3, T24, T25: `piperine-solver`; T4, T33: `piperine-plan`; T34, T35: `piperine-transpile`). `piperine-lang`, `piperine-ir`, `piperine-interp`, `piperine-abi`, `piperine-analog-solver` and `piperine-digital-solver` are not edited; a task that needs one stops and reports.
- **The command line is a shell** (MD-65): every rule of what a run prints or judges is in `piperine-api` with a Rust test. Every new run key has its flag (MD-66).
- **Fail loud.** A value that cannot be computed, a path that names nothing, a refused write: a `Failure` with its code, never a default.
- **Tests state what the spec says, not what the code does.** Expected values come from the spec, ngspice, a closed form or an existing oracle (`Table::on_grid`), each cited at its assertion. Test functions have exactly the names `## Checks` cites.
- **Existing tests that change, and only these:** `every_run_key_has_its_flag` (T7, T20: lists extended), the usage test's command list (T23: one command added), and the Book's `Blinky` listing (T40, HII-17). No test is removed, weakened or skipped.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate of the `**Build**` line. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- **`lessons.py` is not run**: its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-28 and MD-38.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver host-facing doors (`piperine-solver` context, builder, circuit, transient) | integration | each new door: default unchanged, the new value observed; `parity_baseline` unedited | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver --test <target>` |
| Plan (`piperine-plan`) | unit | all branches of the new code | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-plan --lib` |
| Transpile (`piperine-transpile`) | integration | the generated source and the loaded function | `crates/piperine-transpile/tests/*.rs` | `cargo test -p piperine-transpile --test derive` |
| Host library rules (`piperine-api/src/*.rs`) | unit | all branches; 1:1 to the ACs and the listed edge cases | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-api --lib` |
| Host library runs (`Session`) | integration | every AC of HII-02, HII-04 to HII-07, HII-09 to HII-14, HII-17 to HII-19 | `crates/piperine-api/tests/*.rs` | `cargo test -p piperine-api --test <target>` |
| Command line (`piperine-cli`) | e2e | the real binary: happy path, failure, exit codes | `crates/piperine-cli/tests/*.rs` | `cargo test -p piperine-cli --test <target>` |
| Parity (`piperine-parity`) | integration | ngspice at test time under `ToleranceContract`; the skip rule | `crates/piperine-parity/tests/*.rs` | `cargo test -p piperine-parity --test <target>` |
| Root host surface | integration | the re-export runs the new surface | `tests/*.rs` | `cargo test --test host_ii` |
| The Book, `GAPS.md`, `ROADMAP.md`, `STATE.md` | e2e / integration | every listing equals the binary; each record claim read as text | `crates/piperine-cli/tests/{book,docs}.rs` | `cargo test -p piperine-cli --test book --test docs` |
| Feature records (`contract.md`) | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes what a run prints | Quick, then `cargo test -p piperine-api`; `Run twins`; `Run parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

**Batches** (~7 tasks, whole phases): B1 = Phases 1-2 (T1-T8), B2 = Phase 3 (T9-T16), B3 = Phases 4-5 (T17-T23), B4 = Phases 6-7 (T24-T32, two small phases of the host library), B5 = Phase 8 (T33-T38, the cross-lane X9 unit, dispatched after `msl-fixes` merges or rebased onto it), B6 = Phase 9 (T39-T42).

### Phase 1: Baseline

The tree is read against the design before any change.

```
T1
```

### Phase 2: Session options

Two small solver doors, one plan method, then the keys, the session and the flags; a temperature twin.

```
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
```

### Phase 3: Transfer function, pole-zero, sensitivity

The harness learns `print`, three new domains, then the api, then one twin each.

```
T8 → T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16
```

### Phase 4: Periodic steady state and the command line

```
T16 → T17 → T18 → T19 → T20
```

### Phase 5: Metadata

```
T20 → T21 → T22 → T23
```

### Phase 6: Host actions

Two solver doors for nets, then the library's drive, timed set and subscription.

```
T23 → T24 → T25 → T26 → T27 → T28
```

### Phase 7: Records and the digital-only run

```
T28 → T29 → T30 → T31 → T32
```

### Phase 8: Derived params (X9, cross-lane, isolated)

Every task touching lane A's area is here, and nothing before depends on it: the phase rebases on `msl-fixes` alone.

```
T32 → T33 → T34 → T35 → T36 → T37 → T38
```

### Phase 9: The root and the documents

```
T38 → T39 → T40 → T41 → T42
```

---

## Task Breakdown

### T1: Contract: the tree against the design

**What**: Read every location `design.md` cites at `HEAD` and write `contract.md`: each citation confirmed or moved, the names this feature takes from the code (`TransferFunctionAnalysisResult` fields, `PoleZeroResult`, `SensResult::get`, `PssResult::trace` and its `t0`, `TransientSolver::schedule_set`, `ChildValue::Expr` and who writes it, `Assembly::term_value`, `DigitalLimits` and `ProtocolOptions` construction sites), and the probe of S2 and of L56 rerun with their output. A citation that no longer holds and changes a task is reported to the orchestrator before T2.
**Where**: `.specs/features/host-ii/contract.md`
**Depends on**: None
**Reuses**: `.specs/features/long-run/contract.md` (the form)
**Requirement**: HII-21
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists every citation of `design.md` with `confirmed` or `moved: <where>`
- [x] The S2 probe prints 0.5 and the Blinky probe exits 1, as `GAPS.md` says, both outputs quoted
- [x] `cargo test -p piperine-cli --test twins` and the five `parity_*` targets pass at `HEAD` before any code change; their test counts are written in `contract.md`

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): host-ii - contract of the tree against the design`

---

### T2: `Context::max_rounds`

**What**: `Context` gains `max_rounds: u32` (default 20, today's value) and every place that builds `ProtocolOptions` takes it from the analysis's `Context`. Cross-lane touch (lane C), named in `design.md`.
**Where**: `crates/piperine-solver/src/analyses/context.rs`
**Depends on**: T1
**Reuses**: `ProtocolOptions::new` (`protocol/transient.rs:46`)
**Requirement**: HII-02
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-solver/tests/session_bounds.rs` (new, `//!` header) holds `max_rounds_defaults_to_twenty` and `an_operating_point_past_max_rounds_fails_naming_the_bound` (a design whose operating point needs two rounds, run with `max_rounds = 1`, fails with the `E0506` error naming 1)
- [ ] `Run baseline` passes unedited
- [ ] Gate check passes: Quick (`Run solverbounds`, `Run baseline`)
- [ ] Test count: 2 new tests, none removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the round bound of an operating point is a context option`

---

### T3: `CircuitBuilder::microsteps`

**What**: `CircuitBuilder::microsteps(n)` beside `resolution`, carried in the recipe to `bind`, which builds the digital engine with `DigitalLimits { max_delta_cycles: n, .. }` (default 1000, today's). A re-bind keeps it. Cross-lane touch (lane C).
**Where**: `crates/piperine-solver/src/core/builder.rs`
**Depends on**: T2
**Reuses**: `CircuitBuilder::resolution` (`core/builder.rs:127`)
**Requirement**: HII-02
**Executor**: Sonnet

**Done when**:

- [ ] `session_bounds.rs` gains `microsteps_default_to_a_thousand` and `an_instant_past_the_microstep_bound_fails_naming_it` (a zero-delay toggle bound at 5 fails naming 5)
- [ ] Gate check passes: Quick (`Run solverbounds`, `Run baseline`)
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the microstep bound is a binding option`

---

### T4: `Netlist::with_microsteps`

**What**: `piperine-plan`'s `Netlist` gains `with_microsteps(n)` beside `with_resolution`, passed to `CircuitBuilder::microsteps`. Cross-lane touch (lane A): one method, no other line of the file moves.
**Where**: `crates/piperine-plan/src/netlist.rs`
**Depends on**: T3
**Reuses**: `Netlist::with_resolution` (`netlist.rs:31`)
**Requirement**: HII-02
**Executor**: Sonnet

**Done when**:

- [ ] Inline test `netlist::tests::with_microsteps_reaches_the_builder`
- [ ] Gate check passes: Quick (`Run planlib`)
- [ ] Test count: 1 new test

**Tests**: unit
**Gate**: quick

**Commit**: `feat(plan): a netlist carries the microstep bound`

---

### T5: Option keys of a run

**What**: `RunSpec` gains `Options` (`reltol`, `abstol`, `vntol`, `trtol`, `temp`, `resolution`, `maxrounds`, `maxmicro`, `nodeset`) read for every analysis, with `Overrides` fields and the `P0010` rules of HII-01 AC 3 to 5; `RUN_KEYS` grows by nine.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T4
**Reuses**: `Keys`, `Given<T>`, `Source::fault`
**Requirement**: HII-01
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `request::tests::option_keys_are_read_for_every_analysis`, `tolerance_keys_must_be_positive`, `temp_must_be_above_absolute_zero`, `bounds_must_be_whole_numbers_of_at_least_one`, `a_flag_wins_over_its_option_key`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 5 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): session options are keys of a run`

---

### T6: The session applies its options

**What**: `Session::open` builds one `Context` from `Options` (`temp + 273.15`, `max_rounds`), binds with `Netlist::new(ir).with_resolution(..).with_microsteps(..).circuit(..)` only when either is given (the backend's own `circuit()` otherwise), resolves `nodeset` paths (`E0801`) and seeds every operating point with them; every analysis arm uses that `Context`.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T5
**Reuses**: `DcSolver::apply_initial_conditions` (`dc.rs:197`), `Netlist::with_resolution`
**Requirement**: HII-02
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/options.rs` (new) holds `no_option_gives_the_default_context`, `tolerance_keys_reach_the_context`, `temp_is_celsius_read_by_temperature`, `a_nodeset_seeds_the_operating_point_and_keeps_its_solution`, `resolution_rounds_an_at_to_its_tick`, `maxrounds_bounds_the_operating_point`, `maxmicro_bounds_an_instant`, `a_larger_trtol_takes_fewer_steps`, `a_nodeset_naming_nothing_is_e0801`
- [ ] `Run twins` and `Run parity` pass unedited
- [ ] Gate check passes: Full
- [ ] Test count: 9 new tests

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a session runs with its options`

---

### T7: The option flags

**What**: `piperine run` gains `--reltol`, `--abstol`, `--vntol`, `--trtol`, `--temp`, `--resolution`, `--maxrounds`, `--maxmicro` and the repeatable `--nodeset <path>=<value>`; `every_run_key_has_its_flag` grows its lists by these (no assertion removed).
**Where**: `crates/piperine-cli/src/cli.rs`
**Depends on**: T6
**Reuses**: the `--param` parser
**Requirement**: HII-01
**Executor**: Sonnet

**Done when**:

- [ ] `cli::tests::every_run_key_has_its_flag` passes with the nine flags added to its lists
- [ ] New inline test `cli::tests::a_nodeset_flag_without_equals_exits_2`
- [ ] Gate check passes: Quick (`Run clilib`, `Run twins`)
- [ ] Test count: 1 new test, 1 extended

**Tests**: unit
**Gate**: quick

**Commit**: `feat(cli): the session option flags`

---

### T8: The temperature twin

**What**: `tests/ngspice/temp_res.cir` (a source, a resistor with `tc1`, a load; `.options temp=75` and a run at 27) and its twin `temp_res.msl` (the resistor reads `temperature()` against 300.15 K), compared by `crates/piperine-parity/tests/parity_temp.rs` under `DC_VOLTAGE`.
**Where**: `crates/piperine-parity/tests/parity_temp.rs`
**Depends on**: T7
**Reuses**: `parity_op.rs`'s `compare`, `Reference::run`
**Requirement**: HII-03
**Executor**: Sonnet

**Done when**:

- [ ] `parity_temp.rs` (new) holds `fn temp_res()` (75 °C) and `fn temp_res_at_the_default_temperature()`; `parity_corpus` passes
- [ ] Gate check passes: Quick (`Run newparity`, `Run parity`), then Build (end of phase 2)
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): temperature against ngspice`

---

### T9: ngspice `print` lines in the harness

**What**: The harness parses ngspice's `name = value` and `name = re,im` lines of `print` (the `.tf`, `.pz` and `.sens` outputs) into named values, beside the `wrdata` reader.
**Where**: `crates/piperine-parity/src/ngspice.rs`
**Depends on**: T8
**Reuses**: the `wrdata` parser
**Requirement**: HII-04
**Executor**: Sonnet

**Done when**:

- [ ] Inline test `ngspice::tests::print_lines_parse_to_named_values` over the lines quoted in `contract.md`
- [ ] Gate check passes: Quick (`Run paritylib`)
- [ ] Test count: 1 new test

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): read ngspice print lines`

---

### T10: Three new tolerance domains

**What**: `ToleranceContract` gains `TF_RATIO`, `PZ_ROOT` and `SENSITIVITY`, each with its rationale; no existing constant changes.
**Where**: `crates/piperine-parity/src/contract.rs`
**Depends on**: T9
**Reuses**: `DC_VOLTAGE`'s form
**Requirement**: HII-21
**Executor**: Sonnet

**Done when**:

- [ ] Inline test `contract::tests::new_domains_carry_a_rationale`
- [ ] `git diff` shows no line of an existing constant changed
- [ ] Gate check passes: Quick (`Run paritylib`, `Run parity`)
- [ ] Test count: 1 new test

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): tolerance domains for tf, pz and sens`

---

### T11: `tf`, `pz` and `sens` requests

**What**: `Analysis::{Tf, Pz, Sens}` with `input`, `output`, `wrt`; `Analysis::NAMES` of eight (with `pss` reserved for T17's arm); the `P0010` rules of HII-04 AC 3, HII-06 AC 3, HII-08 AC 3.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T10
**Reuses**: `Analysis`, `ANALYSIS_KEYS`
**Requirement**: HII-08
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `request::tests::tf_and_pz_need_input_and_output`, `sens_needs_a_non_empty_wrt`, `the_eight_analysis_names`, `plot_is_refused_for_tf_pz_and_sens`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 4 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): tf, pz and sens requests`

---

### T12: Tables of the small-signal analyses

**What**: `Table::transfer`, `Table::roots`, `Table::sensitivities` with the columns and rows of `design.md`.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T11
**Reuses**: `Table::operating_point`
**Requirement**: HII-04
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `table::tests::a_transfer_table_has_three_rows_in_order`, `roots_list_poles_then_zeros`, `a_sensitivity_table_has_a_row_per_wrt`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 3 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): tables of tf, pz and sens`

---

### T13: The `tf`, `pz` and `sens` arms

**What**: `Session` resolves `input` to a voltage source's branch (`P0010` otherwise), `output` to a node (`E0801`), `wrt` to params (`E0801`; `P0010` for a param a child derives from, read from the IR's `ChildValue::Expr`), refuses a discrete net printed in `sens`, and runs the solver's drivers.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T12
**Reuses**: `Probe::Flow`'s branch rule, `ParamPath::resolve`
**Requirement**: HII-04
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/analyses.rs` gains `tf_input_must_be_a_voltage_source`, `tf_output_naming_nothing_is_e0801`, `pz_of_a_circuit_without_poles_fails`, `sens_wrt_naming_nothing_is_e0801`, `sens_of_a_discrete_net_is_refused`, `sens_wrt_a_derived_param_is_refused`
- [ ] Gate check passes: Full
- [ ] Test count: 6 new tests

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): run tf, pz and sens`

---

### T14: The `tf` twin

**What**: `tests/ngspice/tf_divider.{cir,msl}` (`.tf v(out) v1`), compared in `parity_tf.rs` under `TF_RATIO`.
**Where**: `crates/piperine-parity/tests/parity_tf.rs`
**Depends on**: T13
**Reuses**: T9's parser
**Requirement**: HII-04
**Executor**: Sonnet

**Done when**:

- [ ] `parity_tf.rs` (new) holds `fn tf_divider()`; `parity_corpus` passes
- [ ] Gate check passes: Quick (`Run newparity`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): transfer function against ngspice`

---

### T15: The `pz` twin

**What**: `tests/ngspice/pz_lowpass.{cir,msl}` (`.pz in 0 out 0 vol pz`), compared root for root in `parity_pz.rs` under `PZ_ROOT`.
**Where**: `crates/piperine-parity/tests/parity_pz.rs`
**Depends on**: T14
**Reuses**: T9's parser
**Requirement**: HII-05
**Executor**: Sonnet

**Done when**:

- [ ] `parity_pz.rs` (new) holds `fn pz_lowpass()`
- [ ] Gate check passes: Quick (`Run newparity`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): pole-zero against ngspice`

---

### T16: The `sens` twin

**What**: `tests/ngspice/sens_divider.{cir,msl}` (`.sens v(out)`), the derivatives with respect to both resistances and the source compared in `parity_sens.rs` under `SENSITIVITY`.
**Where**: `crates/piperine-parity/tests/parity_sens.rs`
**Depends on**: T15
**Reuses**: T9's parser
**Requirement**: HII-06
**Executor**: Sonnet

**Done when**:

- [ ] `parity_sens.rs` (new) holds `fn sens_divider()`
- [ ] Gate check passes: Quick (`Run newparity`, `Run parity`), then Build (end of phase 3)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): DC sensitivity against ngspice`

---

### T17: The `pss` request

**What**: `Analysis::Pss { period, tstab, max_step, print_step }` with the `P0010` rules of HII-07 AC 5; `plot` allowed.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T16
**Reuses**: `Analysis::Transient`'s keys
**Requirement**: HII-07
**Executor**: Sonnet

**Done when**:

- [ ] Inline test `request::tests::pss_needs_a_positive_period_and_a_non_negative_tstab`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 1 new test

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): the pss request`

---

### T18: The `pss` arm

**What**: `Session` runs `CircuitInstance::pss` with the session's `Context`, and gives `Table::transient` of the orbit, or `Table::on_grid(h, t0, t0 + T)`; the plot draws its rows.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T17
**Reuses**: `Table::transient`, `Table::on_grid`, `Plot::of`
**Requirement**: HII-07
**Executor**: Sonnet

**Done when**:

- [ ] `tests/analyses.rs` gains `pss_gives_one_period_of_rows`, `pss_on_a_grid_gives_the_grid_rows`, `pss_starts_its_orbit_at_tstab`, `pss_that_does_not_converge_fails`, `a_pss_plot_draws_its_rows`
- [ ] Gate check passes: Full
- [ ] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): run pss`

---

### T19: The `pss` twin

**What**: `tests/ngspice/pss_rc.{cir,msl}`: an RC driven by a square wave of period `T` (ngspice `PULSE` with 1 ps edges; in MSL a bench reaction with `every`, the source's contribution reading a param, or the stdlib `D2A`), `tstab` a multiple of `T`; `parity_pss.rs` compares each grid row of the orbit with ngspice's last period under `TRAN_VOLTAGE` and the end of the high half with `1/(1+exp(-T/(2RC)))`.
**Where**: `crates/piperine-parity/tests/parity_pss.rs`
**Depends on**: T18
**Reuses**: `parity_tran.rs`, the `wrdata` parser
**Requirement**: HII-07
**Executor**: Sonnet

**Done when**:

- [ ] `parity_pss.rs` (new) holds `fn pss_rc()`
- [ ] A miss of `TRAN_VOLTAGE` stops the task and is reported, never a moved bound
- [ ] Gate check passes: Quick (`Run newparity`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): periodic steady state against ngspice`

---

### T20: The new analyses on the command line

**What**: The command line accepts the eight analysis names and the keys `input`, `output`, `wrt`, `period`, `tstab` with their flags (`--input`, `--output`, `--wrt` repeatable, `--period`, `--tstab`; the pairing test extended); a new e2e target runs each new analysis.
**Where**: `crates/piperine-cli/tests/analyses.rs`
**Depends on**: T19
**Reuses**: `tests/run.rs`'s project helpers
**Requirement**: HII-08
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/analyses.rs` (new) holds `each_new_analysis_prints_header_records_and_table`, `each_new_analysis_writes_csv`
- [ ] `cli::tests::every_run_key_has_its_flag` passes with the five flags added
- [ ] Gate check passes: Quick (`Run clianalyses`, `Run clilib`, `Run twins`), then Build (end of phase 4)
- [ ] Test count: 2 new tests

**Tests**: e2e
**Gate**: build

**Commit**: `feat(cli): run tf, pz, sens and pss`

---

### T21: Printing a param and a flow contribution

**What**: `Probe::Param` (the param's value in every row) and `Probe::Term` (the one flow term of an instance with no potential branch, through `Assembly::term_value`); `E0801` for several flow terms.
**Where**: `crates/piperine-api/src/probe.rs`
**Depends on**: T20
**Reuses**: `Probe::resolve`
**Requirement**: HII-09
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/metadata.rs` (new) holds `a_param_prints_its_value_in_every_row`, `the_flow_of_a_single_flow_contribution_prints`, `a_flow_of_several_contributions_is_e0801`
- [ ] Gate check passes: Quick (`Run apimeta`, `Run twins`)
- [ ] Test count: 3 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): print a param and a flow contribution`

---

### T22: Metadata of a session

**What**: `Session::metadata()` gives `Metadata { params, ports, streams }` from the IR and the plan, with no analysis.
**Where**: `crates/piperine-api/src/metadata.rs`
**Depends on**: T21
**Reuses**: `Streams::of`, `Body::params`
**Requirement**: HII-10
**Executor**: Sonnet

**Done when**:

- [ ] `tests/metadata.rs` gains `metadata_lists_every_param_with_type_value_where_and_structural`, `metadata_lists_ports_and_streams`, `metadata_runs_no_analysis`
- [ ] Gate check passes: Quick (`Run apimeta`)
- [ ] Test count: 3 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): metadata of a session`

---

### T23: `piperine info`

**What**: The command `info [<run>]` (Q1 (a)) prints the top, the backend and the three tables; diagnostics as `check`. `--help` lists eight commands (the usage test's list grows by one).
**Where**: `crates/piperine-cli/src/commands/info.rs`
**Depends on**: T22
**Reuses**: `commands/check.rs`, `render/table.rs`
**Requirement**: HII-11
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/info.rs` (new) holds `info_prints_params_ports_and_streams`, `info_of_a_broken_design_prints_diagnostics_and_exits_1`
- [ ] Gate check passes: Quick (`Run cliinfo`, `Run clilib`), then Build (end of phase 5)
- [ ] Test count: 2 new tests

**Tests**: e2e
**Gate**: build

**Commit**: `feat(cli): piperine info`

---

### T24: `CircuitInstance::write_net`

**What**: A host write of bytes to a net between analyses, through the shadow, kept with the host's writes so every later analysis starts from it. Cross-lane touch (lane C).
**Where**: `crates/piperine-solver/src/core/circuit.rs`
**Depends on**: T23
**Reuses**: `CircuitInstance::set`'s shadow and `host_writes`
**Requirement**: HII-12
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-solver/tests/host_writes.rs` (new) holds `write_net_before_a_run_is_an_initial_value`
- [ ] Gate check passes: Quick (`Run solverhost`, `Run baseline`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): a host write to a net between analyses`

---

### T25: `TransientSolver::schedule_write`

**What**: A timed host write of bytes to a net, landed by `Protocol::set` at its time, beside `schedule_set`. Cross-lane touch (lane C).
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T24
**Reuses**: `schedule_set` and its `writes` loop
**Requirement**: HII-12
**Executor**: Sonnet

**Done when**:

- [ ] `host_writes.rs` gains `a_scheduled_write_commits_at_its_time`
- [ ] Gate check passes: Quick (`Run solverhost`, `Run baseline`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): a timed host write to a net`

---

### T26: `Session::drive`

**What**: Host drives of a top's discrete `in` ports, validated at once (`E0804`, `E0802`, `E0806`), applied by `write_net` at 0 and `schedule_write` later.
**Where**: `crates/piperine-api/src/host.rs`
**Depends on**: T25
**Reuses**: `Cell::decode`'s layouts, inverted
**Requirement**: HII-12
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/host_actions.rs` (new) holds `a_drive_at_zero_is_the_initial_value`, `a_drive_later_fires_a_reaction_at_its_time`, `driving_a_carrier_that_is_not_a_top_input_is_e0804`, `a_drive_of_the_wrong_type_is_e0802`, `an_undriven_top_input_is_unknown`, `a_negative_at_is_e0806`
- [ ] Gate check passes: Quick (`Run apihost`)
- [ ] Test count: 6 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): the host drives a top's inputs`

---

### T27: `Session::set_at`

**What**: Timed param sets (Q2 (a)): at 0 a `set` before the analysis, later `schedule_set`; `E0803` for a structural param, `E0802` for a value its `where` refuses, `E0806` for a negative time.
**Where**: `crates/piperine-api/src/host.rs`
**Depends on**: T26
**Reuses**: `TransientSolver::schedule_set`
**Requirement**: HII-13
**Executor**: Sonnet

**Done when**:

- [ ] `tests/host_actions.rs` gains `a_set_at_zero_applies_before_the_analysis`, `a_timed_set_applies_after_the_step_to_its_time`, `a_timed_set_of_a_structural_param_is_e0803`, `a_timed_set_failing_where_is_e0802`
- [ ] Gate check passes: Quick (`Run apihost`)
- [ ] Test count: 4 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): a param set at a time`

---

### T28: `Session::subscribe`

**What**: Narrowing a session's streams before its run; `E0805` after it.
**Where**: `crates/piperine-api/src/host.rs`
**Depends on**: T27
**Reuses**: `Streams::keep`
**Requirement**: HII-14
**Executor**: Sonnet

**Done when**:

- [ ] `tests/host_actions.rs` gains `subscribe_before_the_run_narrows_the_streams`, `subscribe_after_the_run_is_e0805`
- [ ] Gate check passes: Quick (`Run apihost`), then Build (end of phase 6)
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `feat(api): subscribe to streams before a run`

---

### T29: The verdict reads `sim` and keeps targets

**What**: `Verdict` gains `fatal()` (the `msg` of a `sim` record with `failed = true`) and `targets()` (the last margin per name, first-seen order); a margin never fails.
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: T28
**Reuses**: `Verdict::judge`
**Requirement**: HII-15
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `verdict::tests::a_failed_sim_record_is_fatal_with_its_msg`, `finish_alone_is_not_a_failure`, `the_last_record_of_each_target_is_kept_in_first_seen_order`, `a_negative_margin_does_not_fail`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 4 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a fatal record fails a run; targets report`

---

### T30: `piperine run` fails a fatal run

**What**: After a run whose verdict is fatal, `run` keeps what it printed, prints the `msg` on standard error and exits 1.
**Where**: `crates/piperine-cli/src/commands/run.rs`
**Depends on**: T29
**Reuses**: the failure path of CLI-07 AC 5
**Requirement**: HII-15
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/run.rs` gains `a_fatal_run_prints_its_msg_and_exits_1`
- [ ] Gate check passes: Quick (`Run clirun`)
- [ ] Test count: 1 new test

**Tests**: e2e
**Gate**: quick

**Commit**: `feat(cli): a fatal run exits 1`

---

### T31: `piperine test` prints fatal and targets

**What**: `FAIL <name> (fatal: <msg>)`, and one `  target <name> margin=<value>` line per target after a run's line.
**Where**: `crates/piperine-cli/src/commands/test.rs`
**Depends on**: T30
**Reuses**: the `PASS`/`FAIL` writer
**Requirement**: HII-16
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/commands.rs` gains `test_prints_fail_fatal_with_the_msg`, `test_prints_target_margins_after_the_run_line`
- [ ] Gate check passes: Quick (`Run clicommands`)
- [ ] Test count: 2 new tests

**Tests**: e2e
**Gate**: quick

**Commit**: `feat(cli): piperine test shows fatal runs and target margins`

---

### T32: A digital-only transient completes

**What**: The transient arm judges completion by the step observer's last time; a design with no analog unknown gives its table. If the observer does not see the stop, the task stops and reports (the fix would be the protocol's).
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T31
**Reuses**: `Recorder`, `Tap`
**Requirement**: HII-17
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/digital_only.rs` (new) holds `a_digital_only_transient_reaches_its_stop`, `a_digital_only_grid_holds_net_values`, `a_digital_only_run_that_ends_short_fails`
- [ ] Gate check passes: Full, then Build (end of phase 7)
- [ ] Test count: 3 new tests

**Tests**: integration
**Gate**: build

**Commit**: `fix(api): a digital-only transient runs to its stop`

---

### T33: `DerivePlan`

**What**: X9, lane A's area: the derived bindings of a design from `ChildValue::Expr`, a parent before its children, with the readers of each param.
**Where**: `crates/piperine-plan/src/derive.rs`
**Depends on**: T32
**Reuses**: `Body::child_params` (`piperine-ir/src/handle.rs:184`)
**Requirement**: HII-18
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `derive::tests::a_derived_child_param_is_a_binding_of_its_parent`, `a_chain_derives_top_down`, `a_constant_placement_is_no_binding`
- [ ] Gate check passes: Quick (`Run planlib`)
- [ ] Test count: 3 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(plan): the derived params of a design`

---

### T34: Generated derive functions

**What**: The transpiler prints `derive_<k>` per binding and the export `piperine_derive_1` into the design's source. Lane A's area.
**Where**: `crates/piperine-transpile/src/print/derive.rs`
**Depends on**: T33
**Reuses**: the expression printer of `print/value.rs`
**Requirement**: HII-18
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-transpile/tests/derive.rs` (new) holds `the_source_holds_one_derive_function_per_binding`
- [ ] Gate check passes: Quick (`Run transpile`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): derive functions in the generated source`

---

### T35: `Compiled::derive`

**What**: Loading and calling `piperine_derive_1`; a missing symbol with a non-empty plan is a load error naming it. Lane A's area.
**Where**: `crates/piperine-transpile/src/compiled.rs`
**Depends on**: T34
**Reuses**: `load.rs`'s symbol lookup
**Requirement**: HII-18
**Executor**: Sonnet

**Done when**:

- [ ] `tests/derive.rs` gains `compiled_derive_evaluates_the_placement_expression`
- [ ] Gate check passes: Quick (`Run transpile`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): call a design's derive functions`

---

### T36: The host's param table

**What**: `Params`: every instance's values, `set` with derivation through `Derive`, restore on refusal; the interpreter's refusal; a direct set of a derived param refused.
**Where**: `crates/piperine-api/src/params.rs`
**Depends on**: T35
**Reuses**: `CircuitInstance::set`
**Requirement**: HII-20
**Executor**: Sonnet

**Done when**:

- [ ] Inline tests `params::tests::a_set_derives_its_children`, `a_refused_child_restores_every_param`, `a_derived_param_cannot_be_set_directly`, `the_interpreter_refuses_a_set_others_derive_from`, `no_set_gives_the_folded_constants`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: 5 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a set recomputes the params derived from it`

---

### T37: Every `set` path goes through the param table

**What**: `--param`, the `dc` sweep, `Session::set_at` and `Probe::Param` read and write `Params`; a timed set schedules each changed pin at the same time.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T36
**Reuses**: T36
**Requirement**: HII-18
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/derived.rs` (new) holds `the_s2_probe_prints_three_quarters`, `a_sweep_recomputes_the_child_at_each_point`, `a_timed_set_recomputes_at_the_same_instant`, `a_child_failing_where_refuses_the_set`, `a_set_neither_elaborates_nor_compiles`
- [ ] `Run twins`, `Run parity` and the buck targets of `piperine-parity` pass unedited
- [ ] Gate check passes: Full
- [ ] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `fix(api): a parent's set reaches its children (S2)`

---

### T38: Lets and definitions follow a set

**What**: Proves HII-19 on the Rust backend; a failure stops the task and is reported (the fix would be elaboration, lane A's).
**Where**: `crates/piperine-api/tests/derived.rs`
**Depends on**: T37
**Reuses**: T37's fixtures
**Requirement**: HII-19
**Executor**: Sonnet

**Done when**:

- [ ] `tests/derived.rs` gains `a_body_let_reads_the_new_param`, `a_definition_is_recomputed_at_the_set`
- [ ] Gate check passes: Quick (`Run apiderived`), then Build (end of phase 8)
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `test(api): lets and definitions follow a set`

---

### T39: The root host surface

**What**: A root target opening a session through `piperine::` and running `tf`, a drive and a timed set.
**Where**: `tests/host_ii.rs`
**Depends on**: T38
**Reuses**: the root package's re-export
**Requirement**: HII-24
**Executor**: Sonnet

**Done when**:

- [ ] `tests/host_ii.rs` (new, `//!` header) holds `the_root_runs_tf_a_drive_and_a_timed_set`
- [ ] Gate check passes: Quick (`Run root`)
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: quick

**Commit**: `test: the root re-exports the host II surface`

---

### T40: The Book

**What**: The chapter "The command line" shows the option keys, one run of each new analysis, `piperine info` and the completing `Blinky`; every listing held to the binary; `book.rs` gains the three named tests.
**Where**: `docs/book/content.html`
**Depends on**: T39
**Reuses**: `crates/piperine-cli/tests/book.rs` attributes
**Requirement**: HII-22
**Executor**: Sonnet

**Done when**:

- [ ] `book.rs` gains `the_chapter_shows_the_session_options`, `the_chapter_runs_each_new_analysis`, `blinky_runs_to_its_stop_in_the_book`; `every_cli_listing_of_the_book_is_what_the_binary_prints` passes
- [ ] Gate check passes: Quick (`Run clibook`)
- [ ] Test count: 3 new tests

**Tests**: e2e
**Gate**: quick

**Commit**: `docs(book): host II in the command line chapter`

---

### T41: `GAPS.md` and `ROADMAP.md`

**What**: S2, L56, X3, X4, X7, X8, X9 removed; H1 to H4, H6 rewritten to what is left; the follow-up host feature and `root-e2e` in `ROADMAP.md`.
**Where**: `GAPS.md`
**Depends on**: T40
**Reuses**: `docs.rs`'s text checks
**Requirement**: HII-23
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/docs.rs` gains `gaps_drops_what_host_ii_closed`, `the_roadmap_lists_the_host_follow_ups`
- [ ] Gate check passes: Quick (`Run docs`)
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `docs: gaps and roadmap after host II`

---

### T42: `MD-67` and the instruction files

**What**: `.specs/STATE.md` gains `MD-67` (a host `set` recomputes derived params through generated functions; MD-18 unchanged); `CLAUDE.md` and `AGENTS.md` crate rows name the new api modules.
**Where**: `.specs/STATE.md`
**Depends on**: T41
**Reuses**: MD-66's form
**Requirement**: HII-23
**Executor**: Sonnet

**Done when**:

- [ ] `docs.rs` gains `state_records_the_derived_param_decision`
- [ ] Gate check passes: Build
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(specs): MD-67, derived params under a set`

---

## Checks

**C1** - The nine option keys are read for every analysis. (HII-01, AC 1; T5)
Proof: run apilib | test request::tests::option_keys_are_read_for_every_analysis ... ok

**C2** - Each option key has its flag and a flag wins. (HII-01, AC 2; T7)
Proof: run clilib | test cli::tests::every_run_key_has_its_flag ... ok

**C3** - A tolerance or resolution not above 0 is `P0010` naming the key. (HII-01, AC 3; T5)
Proof: run apilib | test request::tests::tolerance_keys_must_be_positive ... ok

**C4** - `temp` at or below -273.15 is `P0010`. (HII-01, AC 4; T5)
Proof: run apilib | test request::tests::temp_must_be_above_absolute_zero ... ok

**C5** - `maxrounds`/`maxmicro` below 1 or not an integer is `P0010`. (HII-01, AC 5; T5)
Proof: run apilib | test request::tests::bounds_must_be_whole_numbers_of_at_least_one ... ok

**C6** - A `nodeset` entry naming no node is `E0801` before any analysis. (HII-01, AC 6; T6)
Proof: run apioptions | test a_nodeset_naming_nothing_is_e0801 ... ok

**C7** - With no option key the session's context equals `Context::default()`. (HII-02, AC 1; T6)
Proof: run apioptions | test no_option_gives_the_default_context ... ok

**C8** - `reltol`, `abstol`, `vntol`, `trtol` replace the defaults. (HII-02, AC 2; T6)
Proof: run apioptions | test tolerance_keys_reach_the_context ... ok

**C9** - `temp = 75` makes `temperature()` read 348.15. (HII-02, AC 3; T6)
Proof: run apioptions | test temp_is_celsius_read_by_temperature ... ok

**C10** - A nodeset seeds the first iterate and the solution is unchanged. (HII-02, AC 4; T6)
Proof: run apioptions | test a_nodeset_seeds_the_operating_point_and_keeps_its_solution ... ok

**C11** - With `resolution = 1e-9` an `at(1.5e-9)` fires at a multiple of 1e-9. (HII-02, AC 5; T6)
Proof: run apioptions | test resolution_rounds_an_at_to_its_tick ... ok

**C12** - An operating point past `maxrounds` fails with `E0506` naming the bound. (HII-02, AC 6; T6)
Proof: run apioptions | test maxrounds_bounds_the_operating_point ... ok

**C13** - An instant past `maxmicro` fails naming the bound. (HII-02, AC 7; T6)
Proof: run apioptions | test maxmicro_bounds_an_instant ... ok

**C14** - `tran_rc` with `trtol = 70` takes fewer accepted steps than with none. (HII-02, AC 8; T6)
Proof: run apioptions | test a_larger_trtol_takes_fewer_steps ... ok

**C15** - `temp_res` at 75 °C agrees with ngspice under `DC_VOLTAGE`. (HII-03, AC 1; T8)
Proof: run newparity | test temp_res ... ok

**C16** - `temp_res` with no `temp` agrees with ngspice at 27 °C. (HII-03, AC 2; T8)
Proof: run newparity | test temp_res_at_the_default_temperature ... ok

**C17** - A `tf` table has `gain`, `input_impedance`, `output_impedance` in order. (HII-04, AC 1; T12)
Proof: run apilib | test table::tests::a_transfer_table_has_three_rows_in_order ... ok

**C18** - The three `tf` values of `tf_divider` agree with ngspice under `TF_RATIO`. (HII-04, AC 2; T14)
Proof: run newparity | test tf_divider ... ok

**C19** - A `tf` or `pz` run without `input` or `output` is `P0010`. (HII-04, AC 3; T11)
Proof: run apilib | test request::tests::tf_and_pz_need_input_and_output ... ok

**C20** - An `input` with no potential contribution is `P0010` naming it. (HII-04, AC 4; T13)
Proof: run apianalyses | test tf_input_must_be_a_voltage_source ... ok

**C21** - An `output` naming no node is `E0801`. (HII-04, AC 5; T13)
Proof: run apianalyses | test tf_output_naming_nothing_is_e0801 ... ok

**C22** - A `pz` table lists poles then zeros with `kind`, `real`, `imag`. (HII-05, AC 1; T12)
Proof: run apilib | test table::tests::roots_list_poles_then_zeros ... ok

**C23** - The poles and zeros of `pz_lowpass` agree with ngspice under `PZ_ROOT`. (HII-05, AC 2; T15)
Proof: run newparity | test pz_lowpass ... ok

**C24** - A circuit with no finite pole fails with no table. (HII-05, AC 3; T13)
Proof: run apianalyses | test pz_of_a_circuit_without_poles_fails ... ok

**C25** - `pz` applies the input and output rules of `tf`. (HII-05, AC 4; T11)
Proof: run apilib | test request::tests::tf_and_pz_need_input_and_output ... ok

**C26** - A `sens` table has a row per `wrt` entry and a `d(<path>)` column per signal. (HII-06, AC 1; T12)
Proof: run apilib | test table::tests::a_sensitivity_table_has_a_row_per_wrt ... ok

**C27** - The derivatives of `sens_divider` agree with ngspice under `SENSITIVITY`. (HII-06, AC 2; T16)
Proof: run newparity | test sens_divider ... ok

**C28** - A `sens` run without `wrt` or with it empty is `P0010`. (HII-06, AC 3; T11)
Proof: run apilib | test request::tests::sens_needs_a_non_empty_wrt ... ok

**C29** - A `wrt` entry naming no param is `E0801`. (HII-06, AC 4; T13)
Proof: run apianalyses | test sens_wrt_naming_nothing_is_e0801 ... ok

**C30** - A discrete net printed in `sens` is `P0010`. (HII-06, AC 5; T13)
Proof: run apianalyses | test sens_of_a_discrete_net_is_refused ... ok

**C31** - A `pss` table covers `t0` to `t0 + T`, one row per step. (HII-07, AC 1; T18)
Proof: run apianalyses | test pss_gives_one_period_of_rows ... ok

**C32** - With `print_step` the rows are `t0 + k*h`. (HII-07, AC 2; T18)
Proof: run apianalyses | test pss_on_a_grid_gives_the_grid_rows ... ok

**C33** - With `tstab = s` the orbit starts at `s`. (HII-07, AC 3; T18)
Proof: run apianalyses | test pss_starts_its_orbit_at_tstab ... ok

**C34** - The `pss_rc` orbit agrees with ngspice's last period and the closed form under `TRAN_VOLTAGE`. (HII-07, AC 4; T19)
Proof: run newparity | test pss_rc ... ok

**C35** - A missing or non-positive `period` or a negative `tstab` is `P0010`. (HII-07, AC 5; T17)
Proof: run apilib | test request::tests::pss_needs_a_positive_period_and_a_non_negative_tstab ... ok

**C36** - A shooting that does not converge fails naming the residual. (HII-07, AC 6; T18)
Proof: run apianalyses | test pss_that_does_not_converge_fails ... ok

**C37** - A `pss` plot draws the table's rows. (HII-07, AC 7; T18)
Proof: run apianalyses | test a_pss_plot_draws_its_rows ... ok

**C38** - Each new analysis prints header, records and table. (HII-08, AC 1; T20)
Proof: run clianalyses | test each_new_analysis_prints_header_records_and_table ... ok

**C39** - Each new analysis writes its table as CSV. (HII-08, AC 2; T20)
Proof: run clianalyses | test each_new_analysis_writes_csv ... ok

**C40** - `plot` on `tf`, `pz`, `sens` is `P0010`. (HII-08, AC 3; T11)
Proof: run apilib | test request::tests::plot_is_refused_for_tf_pz_and_sens ... ok

**C41** - The valid analyses are the eight names. (HII-08, AC 4; T11)
Proof: run apilib | test request::tests::the_eight_analysis_names ... ok

**C42** - A printed param shows its value in every row. (HII-09, AC 1; T21)
Proof: run apimeta | test a_param_prints_its_value_in_every_row ... ok

**C43** - `i(r1)` of a resistor prints its flow. (HII-09, AC 2; T21)
Proof: run apimeta | test the_flow_of_a_single_flow_contribution_prints ... ok

**C44** - `i()` of an instance with several flow terms is `E0801`. (HII-09, AC 3; T21)
Proof: run apimeta | test a_flow_of_several_contributions_is_e0801 ... ok

**C45** - Metadata lists every param with type, value, `where`, structural. (HII-10, AC 1; T22)
Proof: run apimeta | test metadata_lists_every_param_with_type_value_where_and_structural ... ok

**C46** - Metadata lists the top's ports and the streams with fields. (HII-10, AC 2; T22)
Proof: run apimeta | test metadata_lists_ports_and_streams ... ok

**C47** - Metadata runs no analysis. (HII-10, AC 3; T22)
Proof: run apimeta | test metadata_runs_no_analysis ... ok

**C48** - `piperine info` prints the three tables and exits 0. (HII-11, AC 1; T23)
Proof: run cliinfo | test info_prints_params_ports_and_streams ... ok

**C49** - `piperine info` of a broken design prints diagnostics and exits 1. (HII-11, AC 2; T23)
Proof: run cliinfo | test info_of_a_broken_design_prints_diagnostics_and_exits_1 ... ok

**C50** - A drive at 0 is the port's initial value. (HII-12, AC 1; T26)
Proof: run apihost | test a_drive_at_zero_is_the_initial_value ... ok

**C51** - A drive at `at > 0` fires the reaction at `at`. (HII-12, AC 2; T26)
Proof: run apihost | test a_drive_later_fires_a_reaction_at_its_time ... ok

**C52** - Driving anything but a top `in` port is `E0804`. (HII-12, AC 3; T26)
Proof: run apihost | test driving_a_carrier_that_is_not_a_top_input_is_e0804 ... ok

**C53** - A drive of the wrong type is `E0802`. (HII-12, AC 4; T26)
Proof: run apihost | test a_drive_of_the_wrong_type_is_e0802 ... ok

**C54** - An undriven top input reads unknown. (HII-12, AC 5; T26)
Proof: run apihost | test an_undriven_top_input_is_unknown ... ok

**C55** - A set at 0 applies before the analysis. (HII-13, AC 1; T27)
Proof: run apihost | test a_set_at_zero_applies_before_the_analysis ... ok

**C56** - A timed set applies after the step to its time is accepted. (HII-13, AC 2; T27)
Proof: run apihost | test a_timed_set_applies_after_the_step_to_its_time ... ok

**C57** - A timed set of a structural param is `E0803` before the run. (HII-13, AC 3; T27)
Proof: run apihost | test a_timed_set_of_a_structural_param_is_e0803 ... ok

**C58** - A negative `at` is `E0806`. (HII-13, AC 4; T26)
Proof: run apihost | test a_negative_at_is_e0806 ... ok

**C59** - A timed set that fails `where` is `E0802` and no analysis runs. (HII-13, AC 5; T27)
Proof: run apihost | test a_timed_set_failing_where_is_e0802 ... ok

**C60** - A subscription before the run narrows the streams. (HII-14, AC 1; T28)
Proof: run apihost | test subscribe_before_the_run_narrows_the_streams ... ok

**C61** - A subscription after the run is `E0805`. (HII-14, AC 2; T28)
Proof: run apihost | test subscribe_after_the_run_is_e0805 ... ok

**C62** - A fatal run keeps its records, prints its `msg` and exits 1. (HII-15, AC 1; T30)
Proof: run clirun | test a_fatal_run_prints_its_msg_and_exits_1 ... ok

**C63** - `piperine test` prints `FAIL <name> (fatal: <msg>)`. (HII-15, AC 2; T31)
Proof: run clicommands | test test_prints_fail_fatal_with_the_msg ... ok

**C64** - `finish` without `failed` is not a failure. (HII-15, AC 3; T29)
Proof: run apilib | test verdict::tests::finish_alone_is_not_a_failure ... ok

**C65** - The last record per target name is kept in first-seen order. (HII-16, AC 1; T29)
Proof: run apilib | test verdict::tests::the_last_record_of_each_target_is_kept_in_first_seen_order ... ok

**C66** - `piperine test` prints `  target <name> margin=<value>` after the run line. (HII-16, AC 2; T31)
Proof: run clicommands | test test_prints_target_margins_after_the_run_line ... ok

**C67** - A negative margin does not fail the run. (HII-16, AC 3; T29)
Proof: run apilib | test verdict::tests::a_negative_margin_does_not_fail ... ok

**C68** - A digital-only transient gives a table. (HII-17, AC 1; T32)
Proof: run apidigital | test a_digital_only_transient_reaches_its_stop ... ok

**C69** - Its grid rows hold each net's value. (HII-17, AC 2; T32)
Proof: run apidigital | test a_digital_only_grid_holds_net_values ... ok

**C70** - A digital-only run that ends short fails as CLI-05 AC 11. (HII-17, AC 3; T32)
Proof: run apidigital | test a_digital_only_run_that_ends_short_fails ... ok

**C71** - The Book's `Blinky` prints three records and a table, exit 0. (HII-17, AC 4; T40)
Proof: run clibook | test blinky_runs_to_its_stop_in_the_book ... ok

**C72** - A set before a run recomputes every derived descendant param. (HII-18, AC 1; T36)
Proof: run apilib | test params::tests::a_set_derives_its_children ... ok

**C73** - A `dc` sweep recomputes the child at each point. (HII-18, AC 2; T37)
Proof: run apiderived | test a_sweep_recomputes_the_child_at_each_point ... ok

**C74** - A timed set recomputes the derived params at the same instant. (HII-18, AC 3; T37)
Proof: run apiderived | test a_timed_set_recomputes_at_the_same_instant ... ok

**C75** - A recomputed child failing `where` refuses the set, nothing changes. (HII-18, AC 4; T37)
Proof: run apiderived | test a_child_failing_where_refuses_the_set ... ok

**C76** - A set neither elaborates nor runs `rustc`. (HII-18, AC 5; T37)
Proof: run apiderived | test a_set_neither_elaborates_nor_compiles ... ok

**C77** - The S2 probe prints 0.75. (HII-18, AC 6; T37)
Proof: run apiderived | test the_s2_probe_prints_three_quarters ... ok

**C78** - A body `let` reads the new param. (HII-19, AC 1; T38)
Proof: run apiderived | test a_body_let_reads_the_new_param ... ok

**C79** - A definition is recomputed at the set. (HII-19, AC 2; T38)
Proof: run apiderived | test a_definition_is_recomputed_at_the_set ... ok

**C80** - A set of a derived param is `E0802` naming the parent param. (HII-20, AC 1; T36)
Proof: run apilib | test params::tests::a_derived_param_cannot_be_set_directly ... ok

**C81** - The interpreter refuses a set others derive from, naming `--backend rust`. (HII-20, AC 2; T36)
Proof: run apilib | test params::tests::the_interpreter_refuses_a_set_others_derive_from ... ok

**C82** - With no set the values are the folded constants. (HII-20, AC 3; T36)
Proof: run apilib | test params::tests::no_set_gives_the_folded_constants ... ok

**C83** - The 7 twins print their bytes on both backends. (HII-21, AC 1; T1)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok

**C84** - The existing parity targets pass unedited and every twin has its test. (HII-21, AC 2; T10)
Proof: run parity | test every_twin_has_a_cir_and_a_test ... ok

**C85** - The new parity targets print the skip line when ngspice is disabled. (HII-21, AC 3; T8)
Proof: run newparity | test temp_res ... ok

**C86** - Every Book listing is what the binary prints. (HII-22, AC 1; T40)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C87** - The chapter shows the options, each new analysis and `info`. (HII-22, AC 2; T40)
Proof: run clibook | test the_chapter_runs_each_new_analysis ... ok

**C88** - The Book's `Blinky` listing shows the completing run. (HII-22, AC 3; T40)
Proof: run clibook | test blinky_runs_to_its_stop_in_the_book ... ok

**C89** - `GAPS.md` drops S2, L56, X3, X4, X7, X8, X9 and keeps L39. (HII-23, AC 1; T41)
Proof: run docs | test gaps_drops_what_host_ii_closed ... ok

**C90** - `ROADMAP.md` lists the host follow-up and `root-e2e`. (HII-23, AC 2; T41)
Proof: run docs | test the_roadmap_lists_the_host_follow_ups ... ok

**C91** - The root runs `tf`, a drive and a timed set through `piperine::`. (HII-24, AC 1; T39)
Proof: run root | test the_root_runs_tf_a_drive_and_a_timed_set ... ok

**C92** - The solver fails an operating point past `max_rounds` naming the bound. (HII-02, AC 6; T2)
Proof: run solverbounds | test an_operating_point_past_max_rounds_fails_naming_the_bound ... ok

**C93** - The solver fails an instant past the microstep bound naming it. (HII-02, AC 7; T3)
Proof: run solverbounds | test an_instant_past_the_microstep_bound_fails_naming_it ... ok

**C94** - The netlist hands the microstep bound to the builder. (HII-02, AC 7; T4)
Proof: run planlib | test netlist::tests::with_microsteps_reaches_the_builder ... ok

**C95** - A host write to a net before a run is its initial value. (HII-12, AC 1; T24)
Proof: run solverhost | test write_net_before_a_run_is_an_initial_value ... ok

**C96** - A scheduled host write commits at its time. (HII-12, AC 2; T25)
Proof: run solverhost | test a_scheduled_write_commits_at_its_time ... ok

**C97** - The solver's parity baseline passes unedited. (HII-21, AC 2; T2)
Proof: run baseline | test parity_divider_op_dc ... ok

**C98** - The derive plan orders a chain parent before child. (HII-18, AC 1; T33)
Proof: run planlib | test derive::tests::a_chain_derives_top_down ... ok

**C99** - The compiled derive function evaluates the placement's expression with no rustc call at set time. (HII-18, AC 5; T35)
Proof: run transpile | test compiled_derive_evaluates_the_placement_expression ... ok

**C100** - The harness reads ngspice's `print` lines. (HII-04, AC 2; T9)
Proof: run paritylib | test ngspice::tests::print_lines_parse_to_named_values ... ok

**C101** - The new tolerance domains carry a rationale and the old ones are unchanged. (HII-21, AC 2; T10)
Proof: run paritylib | test contract::tests::new_domains_carry_a_rationale ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** Batch B1 (phases 1-2, T1-T8) - Sonnet worker, 2026-11-17.
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1
Phase 2:  T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6 ------→ T7 ------→ T8
Phase 3:  T9 ------→ T10 ------→ T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16
Phase 4:  T17 ------→ T18 ------→ T19 ------→ T20
Phase 5:  T21 ------→ T22 ------→ T23
Phase 6:  T24 ------→ T25 ------→ T26 ------→ T27 ------→ T28
Phase 7:  T29 ------→ T30 ------→ T31 ------→ T32
Phase 8:  T33 ------→ T34 ------→ T35 ------→ T36 ------→ T37 ------→ T38
Phase 9:  T39 ------→ T40 ------→ T41 ------→ T42
```

Execution is strictly sequential - there is no intra-phase parallelism.

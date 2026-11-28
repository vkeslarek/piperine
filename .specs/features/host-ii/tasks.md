# Host II Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/host-ii/design.md`
**Status**: Approved
**Approved**: 2026-11-17 - user: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando"
**Build**: `cargo build -p piperine-solver -p piperine-plan -p piperine-transpile -p piperine-api -p piperine -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite suite_hygiene::`; `cargo test -p piperine-repository-guards --test boundaries`
**Run solverbounds**: `cargo test -p piperine-solver --test host session_bounds::`
**Run solverhost**: `cargo test -p piperine-solver --test host host_writes::`
**Run baseline**: `cargo test -p piperine-solver --test parity_baseline`
**Run planlib**: `cargo test -p piperine-plan --lib`
**Run transpile**: `cargo test -p piperine-transpile --test print derive::`
**Run paritylib**: `cargo test -p piperine-parity --lib`
**Run deviation**: `cargo test -p piperine-parity --test parity deviation::`
**Run parity**: `cargo test -p piperine-parity --test parity`
**Run newparity**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity -- --nocapture`
**Run apilib**: `cargo test -p piperine-api --lib`
**Run apioptions**: `cargo test -p piperine-api --test params options::`
**Run apianalyses**: `cargo test -p piperine-api --test analyses analyses::`
**Run apimeta**: `cargo test -p piperine-api --test session metadata::`
**Run apihost**: `cargo test -p piperine-api --test host host_actions::`
**Run apidigital**: `cargo test -p piperine-api --test records digital_only::`
**Run apiderived**: `cargo test -p piperine-api --test host derived::`
**Run apireach**: `cargo test -p piperine-api --test params option_reach::`
**Run clilib**: `cargo test -p piperine-cli --lib`
**Run clianalyses**: `cargo test -p piperine-cli --test commands analyses::`
**Run cliinfo**: `cargo test -p piperine-cli --test commands info::`
**Run clirun**: `cargo test -p piperine-cli --test commands run::`
**Run clicommands**: `cargo test -p piperine-cli --test commands commands::`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run root**: `cargo test --test host_ii`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`

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
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T43
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

- [x] `crates/piperine-solver/tests/session_bounds.rs` (new, `//!` header) holds `max_rounds_defaults_to_twenty` and `an_operating_point_past_max_rounds_fails_naming_the_bound` (a design whose operating point needs two rounds, run with `max_rounds = 1`, fails with the `E0506` error naming 1)
- [x] `Run baseline` passes unedited
- [x] Gate check passes: Quick (`Run solverbounds`, `Run baseline`)
- [x] Test count: 2 new tests, none removed or edited

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

- [x] `session_bounds.rs` gains `microsteps_default_to_a_thousand` and `an_instant_past_the_microstep_bound_fails_naming_it` (a zero-delay toggle bound at 5 fails naming 5)
- [x] Gate check passes: Quick (`Run solverbounds`, `Run baseline`)
- [x] Test count: 2 new tests

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

- [x] Inline test `netlist::tests::with_microsteps_reaches_the_builder`
- [x] Gate check passes: Quick (`Run planlib`)
- [x] Test count: 1 new test

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

- [x] Inline tests `request::tests::option_keys_are_read_for_every_analysis`, `tolerance_keys_must_be_positive`, `temp_must_be_above_absolute_zero`, `bounds_must_be_whole_numbers_of_at_least_one`, `a_flag_wins_over_its_option_key`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 5 new tests

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

- [x] `crates/piperine-api/tests/options.rs` (new) holds `no_option_gives_the_default_context`, `tolerance_keys_reach_the_context`, `temp_is_celsius_read_by_temperature`, `a_nodeset_seeds_the_operating_point_and_keeps_its_solution`, `resolution_rounds_an_at_to_its_tick`, `maxrounds_bounds_the_operating_point`, `maxmicro_bounds_an_instant`, `a_smaller_trtol_takes_more_steps`, `a_nodeset_naming_nothing_is_e0801`
- [x] `Run twins` and `Run parity` pass unedited
- [x] Gate check passes: Full
- [x] Test count: 9 new tests

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

- [x] `cli::tests::every_run_key_has_its_flag` passes with the nine flags added to its lists
- [x] New inline test `cli::tests::a_nodeset_flag_without_equals_exits_2`
- [x] Gate check passes: Quick (`Run clilib`, `Run twins`)
- [x] Test count: 1 new test, 1 extended

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

- [x] `parity_temp.rs` (new) holds `fn temp_res()` (75 °C) and `fn temp_res_at_the_default_temperature()`; `parity_corpus` passes
- [x] Gate check passes: Quick (`Run newparity`, `Run parity`), then Build (end of phase 2)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): temperature against ngspice`

---

### T43: `max_steps`

**What**: The run key `max_steps` and the flag `--max-steps` (an amendment of HII-01, orchestrator finding 2026-11-17): a positive integer that replaces the transient's budget of 2,000,000 attempts; absent, the budget is today's. It reaches every transient of the session (the PSS shots join it in T18).
**Where**: `crates/piperine-api/src/request.rs`, `crates/piperine-api/src/session.rs`, `crates/piperine-cli/src/cli.rs`, `crates/piperine-api/tests/options.rs`
**Depends on**: T8
**Reuses**: `TransientAnalysisOptions::with_max_steps`
**Requirement**: HII-01, HII-02
**Executor**: Sonnet

**Done when**:

- [x] `RUN_KEYS` holds `max_steps`; `max_steps = 0` or a fraction is `P0010` naming the key; the flag `--max-steps` exists and `every_run_key_has_its_flag` lists it
- [x] `options.rs` gains `a_run_past_max_steps_fails_and_succeeds_when_raised` (a transient that needs more attempts than a small budget fails with `exceeded the <budget> step budget`; the same run with the budget raised succeeds) and `max_steps_defaults_to_two_million`
- [x] Gate check passes: Quick (`Run apilib`, `Run apioptions`, `Run clilib`, `Run twins`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): the transient's step budget is a run key`

---

### T9: ngspice `print` lines in the harness

**What**: The harness parses ngspice's `name = value` and `name = re,im` lines of `print` (the `.tf`, `.pz` and `.sens` outputs) into named values, beside the `wrdata` reader.
**Where**: `crates/piperine-parity/src/ngspice.rs`
**Depends on**: T8
**Reuses**: the `wrdata` parser
**Requirement**: HII-04
**Executor**: Sonnet

**Done when**:

- [x] Inline test `ngspice::tests::print_lines_parse_to_named_values` over the lines quoted in `contract.md`
- [x] Gate check passes: Quick (`Run paritylib`)
- [x] Test count: 1 new test

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

- [x] Inline test `contract::tests::new_domains_carry_a_rationale`
- [x] `git diff` shows no line of an existing constant changed
- [x] Gate check passes: Quick (`Run paritylib`, `Run parity`)
- [x] Test count: 1 new test

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

- [x] Inline tests `request::tests::tf_and_pz_need_input_and_output`, `sens_needs_a_non_empty_wrt`, `the_eight_analysis_names`, `plot_is_refused_for_tf_pz_and_sens`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 4 new tests

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

- [x] Inline tests `table::tests::a_transfer_table_has_three_rows_in_order`, `roots_list_poles_then_zeros`, `a_sensitivity_table_has_a_row_per_wrt`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 3 new tests

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

- [x] `crates/piperine-api/tests/analyses.rs` gains `tf_input_must_be_a_voltage_source`, `tf_output_naming_nothing_is_e0801`, `pz_of_a_circuit_without_poles_fails`, `sens_wrt_naming_nothing_is_e0801`, `sens_of_a_discrete_net_is_refused`, `sens_wrt_a_derived_param_is_refused`
- [x] Gate check passes: Full
- [x] Test count: 6 new tests

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

- [x] `parity_tf.rs` (new) holds `fn tf_divider()`; `parity_corpus` passes
- [x] Gate check passes: Quick (`Run newparity`)
- [x] Test count: 1 new test

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

- [x] `parity_pz.rs` (new) holds `fn pz_lowpass()`
- [x] Gate check passes: Quick (`Run newparity`)
- [x] Test count: 1 new test

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

- [x] `parity_sens.rs` (new) holds `fn sens_divider()`
- [x] Gate check passes: Quick (`Run newparity`, `Run parity`), then Build (end of phase 3)
- [x] Test count: 1 new test

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

- [x] Inline test `request::tests::pss_needs_a_positive_period_and_a_non_negative_tstab`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 1 new test

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

- [x] `tests/analyses.rs` gains `pss_gives_one_period_of_rows`, `pss_on_a_grid_gives_the_grid_rows`, `pss_starts_its_orbit_at_tstab`, `pss_that_does_not_converge_fails`, `a_pss_plot_draws_its_rows`
- [x] Gate check passes: Full
- [x] Test count: 5 new tests

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

- [x] `parity_pss.rs` (new) holds `fn pss_rc()`
- [x] A miss of `TRAN_VOLTAGE` stops the task and is reported, never a moved bound
- [x] Gate check passes: Quick (`Run newparity`)
- [x] Test count: 1 new test

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

- [x] `crates/piperine-cli/tests/analyses.rs` (new) holds `each_new_analysis_prints_header_records_and_table`, `each_new_analysis_writes_csv`
- [x] `cli::tests::every_run_key_has_its_flag` passes with the five flags added
- [x] Gate check passes: Quick (`Run clianalyses`, `Run clilib`, `Run twins`), then Build (end of phase 4)
- [x] Test count: 2 new tests

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

- [x] `crates/piperine-api/tests/metadata.rs` (new) holds `a_param_prints_its_value_in_every_row`, `the_flow_of_a_single_flow_contribution_prints`, `a_flow_of_several_contributions_is_e0801`
- [x] Gate check passes: Quick (`Run apimeta`, `Run twins`)
- [x] Test count: 3 new tests

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

- [x] `tests/metadata.rs` gains `metadata_lists_every_param_with_type_value_where_and_structural`, `metadata_lists_ports_and_streams`, `metadata_runs_no_analysis`
- [x] Gate check passes: Quick (`Run apimeta`)
- [x] Test count: 3 new tests

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

- [x] `crates/piperine-cli/tests/info.rs` (new) holds `info_prints_params_ports_and_streams`, `info_of_a_broken_design_prints_diagnostics_and_exits_1`
- [x] Gate check passes: Quick (`Run cliinfo`, `Run clilib`), then Build (end of phase 5)
- [x] Test count: 2 new tests

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

- [x] `crates/piperine-solver/tests/host_writes.rs` (new) holds `write_net_before_a_run_is_an_initial_value`
- [x] Gate check passes: Quick (`Run solverhost`, `Run baseline`)
- [x] Test count: 1 new test

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

- [x] `host_writes.rs` gains `a_scheduled_write_commits_at_its_time`
- [x] Gate check passes: Quick (`Run solverhost`, `Run baseline`)
- [x] Test count: 1 new test

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

- [x] `crates/piperine-api/tests/host_actions.rs` (new) holds `a_drive_at_zero_is_the_initial_value`, `a_drive_later_fires_a_reaction_at_its_time`, `driving_a_carrier_that_is_not_a_top_input_is_e0804`, `a_drive_of_the_wrong_type_is_e0802`, `an_undriven_top_input_is_unknown`, `a_negative_at_is_e0806`
- [x] Gate check passes: Quick (`Run apihost`)
- [x] Test count: 6 new tests

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

- [x] `tests/host_actions.rs` gains `a_set_at_zero_applies_before_the_analysis`, `a_timed_set_applies_after_the_step_to_its_time`, `a_timed_set_of_a_structural_param_is_e0803`, `a_timed_set_failing_where_is_e0802`
- [x] Gate check passes: Quick (`Run apihost`)
- [x] Test count: 4 new tests

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

- [x] `tests/host_actions.rs` gains `subscribe_before_the_run_narrows_the_streams`, `subscribe_after_the_run_is_e0805`
- [x] Gate check passes: Quick (`Run apihost`), then Build (end of phase 6)
- [x] Test count: 2 new tests

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

- [x] Inline tests `verdict::tests::a_fatal_record_fails_the_verdict_with_its_message`, `a_finish_record_does_not_fail_the_verdict` (msl-fixes' tests, which absorbed host-ii's two duplicates of the same assertions), `the_last_record_of_each_target_is_kept_in_first_seen_order`, `a_negative_margin_does_not_fail`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 4 new tests

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

- [x] `crates/piperine-cli/tests/run.rs` gains `a_fatal_run_prints_its_msg_and_exits_1`
- [x] Gate check passes: Quick (`Run clirun`)
- [x] Test count: 1 new test

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

- [x] `crates/piperine-cli/tests/commands.rs` gains `test_prints_fail_fatal_with_the_msg`, `test_prints_target_margins_after_the_run_line`
- [x] Gate check passes: Quick (`Run clicommands`)
- [x] Test count: 2 new tests

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

- [x] `crates/piperine-api/tests/digital_only.rs` (new) holds `a_digital_only_transient_reaches_its_stop`, `a_digital_only_grid_holds_net_values`, and (B6, after `msl-fixes`) `a_digital_only_run_ended_by_finish_is_ok`; `a_digital_only_run_that_ends_short_fails` is restated (see Settled): no source ends a digital-only run short any more, its rule is held by `recorder::tests::the_recorder_reports_the_time_of_the_last_step_it_saw`
- [x] Gate check passes: Full, then Build (end of phase 7)
- [x] Test count: 2 new api tests, 1 recorder unit test, run.rs rewritten

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

- [x] Inline tests `derive::tests::a_derived_child_param_is_a_binding_of_its_parent`, `a_chain_derives_top_down`, `a_constant_placement_is_no_binding`
- [x] Gate check passes: Quick (`Run planlib`)
- [x] Test count: 3 new tests

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

- [x] `crates/piperine-transpile/tests/derive.rs` (new) holds `the_source_holds_one_derive_function_per_binding`
- [x] Gate check passes: Quick (`Run transpile`)
- [x] Test count: 1 new test

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

- [x] `tests/derive.rs` gains `compiled_derive_evaluates_the_placement_expression`
- [x] Gate check passes: Quick (`Run transpile`)
- [x] Test count: 1 new test

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

- [x] Inline tests `params::tests::a_set_derives_its_children`, `a_refused_child_restores_every_param`, `a_derived_param_cannot_be_set_directly`, `the_interpreter_refuses_a_set_others_derive_from`, `no_set_gives_the_folded_constants`
- [x] Gate check passes: Quick (`Run apilib`)
- [x] Test count: 5 new tests

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

- [x] `crates/piperine-api/tests/derived.rs` (new) holds `the_s2_probe_prints_three_quarters`, `a_sweep_recomputes_the_child_at_each_point`, `a_timed_set_recomputes_at_the_same_instant`, `a_child_failing_where_refuses_the_set`, `a_set_neither_elaborates_nor_compiles`
- [x] `Run twins`, `Run parity` and the buck targets of `piperine-parity` pass unedited
- [x] Gate check passes: Full
- [x] Test count: 5 new tests

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

- [x] `tests/derived.rs` gains `a_body_let_reads_the_new_param`, `a_definition_is_recomputed_at_the_set`
- [x] Gate check passes: Quick (`Run apiderived`), then Build (end of phase 8)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `test(api): lets and definitions follow a set`

---

### T39: The root host surface

**What**: A root target opening a session through `piperine::` and running `tf`, a drive and a timed set.
**Where**: `crates/piperine-api/tests/root_surface/host_ii.rs` (a `[[test]]` of the root package)
**Depends on**: T38
**Reuses**: the root package's re-export
**Requirement**: HII-24
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-api/tests/root_surface/host_ii.rs` (a `[[test]]` of the root package) (new, `//!` header) holds `the_root_runs_tf_a_drive_and_a_timed_set`
- [x] Gate check passes: Quick (`Run root`)
- [x] Test count: 1 new test

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

- [x] `book.rs` gains `the_chapter_shows_the_session_options`, `the_chapter_runs_each_new_analysis`, `blinky_runs_to_its_stop_in_the_book`; `every_cli_listing_of_the_book_is_what_the_binary_prints` passes
- [x] Gate check passes: Quick (`Run clibook`)
- [x] Test count: 3 new tests

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

- [x] `crates/piperine-cli/tests/docs.rs` gains `gaps_drops_what_host_ii_closed`, `the_roadmap_lists_the_host_follow_ups`
- [x] Gate check passes: Quick (`Run docs`)
- [x] Test count: 2 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `docs: gaps and roadmap after host II`

---

### T42: `MD-69` and the instruction files

**What**: `.specs/STATE.md` gains `MD-69` (a host `set` recomputes derived params through generated functions; MD-18 unchanged); `CLAUDE.md` and `AGENTS.md` crate rows name the new api modules.
**Where**: `.specs/STATE.md`
**Depends on**: T41
**Reuses**: MD-66's form
**Requirement**: HII-23
**Executor**: Sonnet

**Done when**:

- [x] `docs.rs` gains `state_records_the_derived_param_decision`
- [x] Gate check passes: Build
- [x] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(specs): MD-69, derived params under a set`

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

**C14** - `tran_rc` with `trtol = 0.001` takes more accepted steps than with none. (HII-02, AC 8; T6)
Proof: run apioptions | test a_smaller_trtol_takes_more_steps ... ok

**C14a** - A transient past the `max_steps` budget fails naming it, and succeeds with the budget raised; the default is 2,000,000. (HII-02, AC 9; T43)
Proof: run apioptions | test a_run_past_max_steps_fails_and_succeeds_when_raised ... ok

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
Proof: run apilib | test session::tests::set_at_refuses_a_structural_param_after_the_start_before_the_run ... ok

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

**C63** - `piperine test` prints `FAIL <name> (fatal at <t>: <msg>)`. (HII-15, AC 2; T31)
Proof: run clicommands | test test_prints_fail_fatal_with_the_msg ... ok

**C64** - `finish` without `failed` is not a failure. (HII-15, AC 3; T29)
Proof: run apilib | test verdict::tests::a_finish_record_does_not_fail_the_verdict ... ok

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

**C70** - A digital-only run ended by `finish` is a run ended ok, not "ended before its stop" (restated, see Settled: X4, and a genuine short end has no source since `msl-fixes`). (HII-17, AC 3; T32)
Proof: run apidigital | test a_digital_only_run_ended_by_finish_is_ok ... ok

**C71** - The Book's `Blinky` prints three records and a table, exit 0. (HII-17, AC 4; T40)
Proof: run clibook | test blinky_runs_to_its_stop_in_the_book ... ok

**C72** - A set before a run recomputes every derived descendant param. (HII-18, AC 1; T36)
Proof: run apilib | test params::tests::a_set_derives_its_children ... ok

**C73** - A `dc` sweep recomputes the child at each point. (HII-18, AC 2; T37)
Proof: run apiderived | test a_sweep_recomputes_the_child_at_each_point ... ok

**C74** - A timed set recomputes the derived params at the same instant. (HII-18, AC 3; T37)
Proof: run apiderived | test a_timed_set_recomputes_at_the_same_instant ... ok

**C75** - A recomputed child failing `where` refuses the set, nothing changes. (HII-18, AC 4; T37)
Proof: run apiderived | test a_refused_set_leaves_the_param_the_circuit_reads_as_it_was ... ok

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

**C89** - `GAPS.md` drops S2, L56, X3, X4, X7, X8, X9 and keeps L39. (HII-23, AC 1; T41) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C90** - `ROADMAP.md` lists the host follow-up and `root-e2e`. (HII-23, AC 2; T41) (retired by code-clarity T10: a pin on a sentence of a tracking file; the test was already gone)
Proof: run ledger | test every_retired_check_is_recorded ... ok

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

**C102** - A placement written with a method call derives through `--param`. (HII-18, AC 1 and 6; F1) 
Proof: run apiderived | test a_method_call_placement_derives_through_a_param_flag ... ok

**C103** - A placement written with a method call is recomputed at each `dc` sweep point. (HII-18, AC 2; F1) 
Proof: run apiderived | test a_method_call_placement_derives_at_each_sweep_point ... ok

**C104** - A placement written with a method call is recomputed at a timed set. (HII-18, AC 3; F1) 
Proof: run apiderived | test a_method_call_placement_derives_in_a_timed_set ... ok

**C105** - The interpreter refuses a set a method-call placement derives from. (HII-20, AC 2; F1) 
Proof: run apiderived | test the_interpreter_refuses_a_set_a_method_call_placement_derives_from ... ok

**C106** - The param walk reads every form of `Expr`, method calls, `known_or`, `is_known` and `high` included. (HII-18, AC 1; F1) 
Proof: run planlib | test derive::tests::every_form_that_holds_an_operand_reports_the_params_it_reads ... ok

**C107** - A timed set derives against the sets due before it, a set given later but at 0 included. (HII-18, AC 3; HII-13; F2) 
Proof: run apiderived | test a_timed_set_derives_against_the_sets_due_before_it ... ok

**C108** - Timed sets given out of time order derive in time order. (HII-18, AC 3; HII-13; F2) 
Proof: run apiderived | test a_timed_set_given_out_of_time_order_derives_in_time_order ... ok

**C109** - Metadata lists a derived param at the value the run uses, through `Session::metadata` and `Session::describe`. (HII-10, AC 1; HII-11; F3) 
Proof: run apiderived | test metadata_lists_a_derived_param_at_the_value_the_run_uses ... ok

**C110** - `print` of a param no device pin holds reads the session's table, at each time. (HII-09, AC 1; F4) 
Proof: run apiderived | test a_param_no_pin_holds_prints_the_value_the_run_used ... ok

**C111** - `temp` and `nodeset` reach a `op` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test an_operating_point_moves_with_temp_and_nodeset ... ok

**C112** - `temp` and `nodeset` reach a `dc` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test a_dc_sweep_moves_with_temp_and_nodeset ... ok

**C113** - `temp` and `nodeset` reach a `ac` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test an_ac_sweep_moves_with_temp_and_nodeset ... ok

**C114** - `temp` and `nodeset` reach a `transient` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test a_transient_moves_with_temp_and_nodeset ... ok

**C115** - `temp` and `nodeset` reach a `tf` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test a_transfer_function_moves_with_temp_and_nodeset ... ok

**C116** - `temp` and `nodeset` reach a `pz` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test the_poles_move_with_temp_and_nodeset ... ok

**C117** - `temp` and `nodeset` reach a `sens` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test a_sensitivity_moves_with_temp_and_nodeset ... ok

**C118** - `temp` and `nodeset` reach a `pss` run: its result moves with each, against a closed form. (HII-01, AC 1; HII-02, AC 2 to 4; F5, F6) 
Proof: run apireach | test a_periodic_steady_state_moves_with_temp_and_nodeset ... ok

**C119** - A pss orbit that does not repeat names the step as a possible cause, not only the drive. (HII-07, AC 6; F8) 
Proof: run apianalyses | test a_pss_orbit_that_does_not_close_at_the_step_says_so ... ok

**C120** - The twin `temp_res` run through a `Session` with its run keys agrees with ngspice under the same contract. (HII-03, AC 1 and 2; F9) 
Proof: run newparity | test temp_res_through_a_session ... ok

**C121** - The twin `tf_divider` run through a `Session` with its run keys agrees with ngspice under the same contract. (HII-04, AC 2; F9) 
Proof: run newparity | test tf_divider_through_a_session ... ok

**C122** - The twin `pz_lowpass` run through a `Session` with its run keys agrees with ngspice under the same contract. (HII-05, AC 2; F9) 
Proof: run newparity | test pz_lowpass_through_a_session ... ok

**C123** - The twin `sens_divider` run through a `Session` with its run keys agrees with ngspice under the same contract. (HII-06, AC 2; F9) 
Proof: run newparity | test sens_divider_through_a_session ... ok

**C124** - The twin `pss_rc` run through a `Session` with its run keys agrees with ngspice under the same contract. (HII-07, AC 4; F9) 
Proof: run newparity | test pss_rc_through_a_session ... ok

**C125** - The deviation table holds `temp_res` and `pss_rc`, and leaves out only `tf_divider`, `pz_lowpass` and `sens_divider`. (HII-21, AC 2; F11) 
Proof: run deviation | test the_table_covers_every_twin ... ok

**C126** - A timed set whose derived child fails `where` is refused at the call and leaves the run as it was. (HII-18, AC 3 and 4; HII-13, AC 5; F12) 
Proof: run apiderived | test a_timed_set_whose_derived_child_fails_where_is_refused ... ok

**C127** - A drive or a set after the transient's stop is `E0806` at the call; one at the stop is accepted. (HII-13, AC 4 extended; F13) 
Proof: run apihost | test an_action_past_the_stop_is_e0806_at_the_call ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (B1 worker)
- T2/T3 (E0501 text): the digital engine's `E0501` names the tick and the nets, not the bound; `piperine-digital-solver` is not edited, so `Bound::microsteps` carries the bound and `analyses/adapters.rs::instant_error` appends `(the bound is N microsteps)` to a `NotQuiet` from `run_instant` and `advance`.
- T5: `RUN_KEYS` grows 20 to 29, so the assertion `RUN_KEYS.len() == 20` of `a_run_table_reads_its_nineteen_keys` became 29 (the only edit of an existing assertion; the comment says why). A `nodeset` flag replaces the table's entry of the same path and keeps the others.
- T6: `nodeset` seeds `op` and every point of `dc` through `DcSolver::apply_initial_conditions`; `ac` and `transient` take their operating point inside the solver, where the host has no door, so a `nodeset` on them is `P0010` (fail loud) instead of ignored. One extra test covers it (`a_nodeset_on_a_run_that_takes_its_own_operating_point_is_p0010`).
- T6, HII-02 AC 8 amended (orchestrator ruling): `trtol` only gates rejection today (`assess_step`), so "`trtol = 70` takes fewer steps" is moved to solver-hygiene (LTE); host-ii owns that `trtol` reaches the step assessment: `tran_rc` takes 3241 steps at `trtol = 0.001` against 115 by default (`a_smaller_trtol_takes_more_steps`). HII-21 AC 2 amended: `parity_corpus.rs` may grow its twin count by one per new twin.
- T6/T8 temperature finding: the operating point of `v +> temperature()` reads 299.857 K (not 300.15) because Newton stops at `reltol * |x| + vntol` (1e-3 and 1e-6); with `--reltol 1e-9 --vntol 1e-12` it reads 300.15 exactly (and 348.15 at `--temp 75`). The test bound is that criterion, not a loosened constant; not a defect.
- T43 (orchestrator finding): `max_steps` is a run key and `--max-steps` flag (positive integer, default 2,000,000 unchanged), spec HII-01 AC 7 and HII-02 AC 9 amended; it reaches every transient of the session, and the PSS arm (T18) must pass it to its shots.
- T8: `parity_corpus.rs` twin count 7 to 8 (append-only edit); lane C made the same edit for `tran_dq`, expect a trivial conflict, the final count is the sum.
- (B2 worker) T9: `Ngspice::print_lines` and `Golden::printed_complex` read `name = re,im`. T10: `TF_RATIO` 1e-6/1e-12 (seven printed digits), `PZ_ROOT` 1e-5/1e-6 (six digits; the cir uses `set numdgt=10`), `SENSITIVITY` 1e-5/1e-12 (ngspice's `.sens` is itself a finite difference, 1e-6 off the exact value).
- T11: keys `input`, `output`, `wrt` and flags `--input`, `--output`, `--wrt` landed here (the existing flag-list test extended; RUN_KEYS 30 to 33, `RUN_KEYS[20..30]`); `pss` is in `Analysis::NAMES` and refused (P0010 "reserved") until T17. `--analysis` help lists tf, pz, sens.
- T12: `Table::sensitivities(probes, outputs: &[Net], wrt: &[ParamPath], result)` takes the solved nets too (design.md's three-argument form cannot look a key up: `SensResult` is keyed by `Net::label`). `num-complex` is a dev-dependency of piperine-api.
- T13: `nodeset` is P0010 on tf, pz and sens. BLOCKER for T14: `TransferFunctionSolver::input_is_voltage_source` (`piperine-solver/src/analyses/tf.rs:260`) classifies the input by `component.starts_with('V')`, so every design path (`Top.v1`) fails with "not a recognised voltage/current source label". Fixing it is a solver edit outside the allowed cross-lane touches; T14 (tf twin) is not done, and the tf happy path is untested.
- T15: the pz ngspice fixture omits the voltage source (ngspice reports "input signal is shorted" for a pinned input node); `vol pz` of the network behind `in` equals the source-driven transfer. `parity_corpus` twin count 8 to 9 (T15), 10 (T16).
- (B3 worker) T14: `TransferFunctionSolver::input_is_voltage_source` classifies by the resolved branch (`input_branch_ref.is_branch()`), not `starts_with('V')` (authorized cross-lane touch, `tf.rs`); a second defect in the same file fixed with it: `calculate_output_resistance` kept the operating point's right-hand side (a source's own value) and returned 742.5 ohm for 750 (the DC solution added to the test response); it now uses the Jacobian only, as the gain does. `tf_input_must_be_a_voltage_source` now also runs the happy path (`v1` and `Top.v1`). `parity_corpus` twin count 10 to 11 (T14), 12 (T19).
- T17: the keys `period` and `tstab` and their flags `--period`, `--tstab` landed with the request (the pairing test would be red otherwise): RUN_KEYS 33 to 35. `tstab` defaults to 0.
- T18: `PssAnalysisOptions` gains `dt_max` and `max_steps` (the session's `max_steps` and a run's `max_step` reach every shot); one struct literal of `failure_rules.rs` gets the two fields as `None`. The PSS arm refuses a `nodeset` (P0010, like transient). A solver defect seen, not fixed: a shot whose stop is 2e-5 s with a 1 ns step fails "a step to t = 1.9999999999999998e-5 s does not advance" (float end-snap of the transient); the tests use tstab 8 and 9 us.
- T19 PSS orbit: `pss_rc` (a `Pulse` source reading `now()` and its params, an RC, period 4 us, tstab 8 us) agrees with the last period of ngspice's ten-period `.tran` and the closed form under `TRAN_VOLTAGE` unchanged; worst deviation 2.73e-4 V (at phase 2.04 us, just after the falling edge; the bound there is 1.2e-3 V); closed form 0.880797 against orbit 0.880804. The source has 1 ps edges and no breakpoint, so the shot step is capped at 1 ns (`PssRun::STEP`): at 4 ns the orbit misses by 1.9e-4 on a 0.119 V sample (bound 1.2e-4), so the cap is the edge, not a moved tolerance. The drive is not the stdlib `D2A` with a discrete clock: a `transition`'s state keeps the absolute time of its last edge (`ramp.t0`), so the shooting's digital-periodicity check never closes (`digital state is not periodic at T`), and with the edges aligned the orbit did not repeat to 3.3e-4. Rust backend only (the interpreter does not run `now()`).
- T20: a new e2e target runs the four analyses and `--csv`; the lone-file usage message names the eight analyses.
- T21: `Probe::Param` (a param's value, op, dc and transient) and `Probe::Term` (op and dc only: `StepView` carries no device term, so a flow term in transient, ac, pss or sens is E0801/P0010 saying so; a decision for the user if it must print in a transient: it needs a `StepView` read of `Assembly::term_value`). `Cell::decode_param` reads a param net's raw bytes (no known byte).
- T22: `Body::param_check_text` is a new additive public method in `piperine-ir` (the printer's own `where` rendering); a param's value is the elaborated one or the request's `params`.
- T23: `piperine info` with no run takes the first run of the manifest. The `where` column is the printer's text (`r > 0.0`).
- Actual worst deviations against ngspice (print with `-- --nocapture`): tf_divider 0 (gain, output and input resistance exact at the printed digits); pz_lowpass pole(1) re 3.3e-7 (value 1333.33), im 0; sens_divider d/dr1 1.87e-9, d/dr2 6.2e-10, d/dv1 1.6e-12 (ngspice's `.sens` is a finite difference); pss_rc 2.7e-4 V.
- (B4 worker) T24/T25: the host value is a `ParamValue` (re-exported by `piperine-api`; `ParamValue::Bytes` is refused as `E0802`), not raw bytes: `CircuitInstance::write_net(net, value)` and `TransientSolver::schedule_write(t, net, value)` encode in the net's own layout (known byte set) through the solver's `encode`, and name a net by its path in the digital plan (`Top.clk`); a write at or before the start is a write before the run. New `Bound::net_bytes` beside `param_bytes`.
- T26: `Session::drive(port, ParamValue, at)`; `port` as written or relative to the top; `E0801` for a path naming nothing, `E0804` for an `out` port, a signal, a node or any carrier that is not a top `in` port, `E0802` for a value not of the port's type (integers range-checked, a bit takes a bool), `E0806` for a negative `at`, `P0010` for `at > 0` on a run that is not a transient, a plain failure after the run. An action at `at = 0` is applied at once (`write_net`/`set`); only `at > 0` waits in `Session::actions` for the transient. A port the host never drives reads unknown (§23.1 rule 9). `ParamValue` is the library's value type.
- T27: `Session::set_at(path, f64, at)`; `at = 0` is `CircuitInstance::set` at once (E0802 from the `where`); `at > 0` first runs a new non-mutating solver door `CircuitInstance::check_set` (a shadow check: type and `where`, E0802 at call time, no analysis run), then waits for `schedule_set`. The `E0803` rule is `ParamPath::refuse_timed`. FINDING: the elaborator never marks a param structural (`elab/body.rs:113` hardcodes `structural: false`; generation `for`/`if` in a module body is `P0001`), so no MSL design reaches `E0803` today; its test marks the IR through serde (`serde_json` is a dev-dependency of `piperine-api`) and calls `refuse_timed`.
- T28: `Session::subscribe(&[&str])` calls `Streams::keep` (a name or a full path; `E0801` for none matching; a second call replaces the first); after `run`, `E0805` naming the streams.
- T29/T30: `Verdict::fatal()` is the first `sim` record with `failed = true` (its `msg`, empty if absent) and `passed()` is false for it; `Verdict::targets()` keeps the last margin per name (an unknown margin is NaN so it shows and never reads as met; a target counts in `checks()`); new `Verdict::or_fatal(run)` and `Failure::Fatal(msg)` (stage `run`, displays the bare message): `piperine run` hands every record, shown or not, to a `Verdict` and a fatal wins over the run's own result (a `fatal` ends the analysis, so a short transient is its doing), no table, `error: <msg>`, exit 1.
- T31: `piperine test` prints `FAIL <run> (fatal: <msg>)` (failing check records, if any, follow) and, after everything of a run, `  target <name> margin=<value>` per target, the value in the table renderer's number format (`-2.500000e-1`); a fatal run is judged, not `did not run`, even if its transient ended short. New fixture project `crates/piperine-cli/tests/projects/fatal_and_target`.
- Lane A: its device side (`finish`/`fatal` ending the analysis) is not on this branch; T30/T31 read the `sim` record the stdlib already writes, and the tests run the bench to its stop with `fatal` called. After lane A merges, a `finish` that ends a transient early will meet the transient arm's "ended before its stop" failure; X4 says `finish` ends the run ok, so the arm must treat a run ended by a `finish` record as complete (not built here: nothing on this branch ends a run early to test it).
- T32 BLOCKED (not committed): the step observer does not see the stop of a digital-only run. `Protocol::transient` does call `observe_step(t_c)` at the stop, but `protocol/step.rs::observe` passes `analog.step_time().unwrap_or(time)` and `TransientSide::step_time` (`analyses/adapters.rs:420`) returns `Some(run.accepted_time())`, which is 0 when the design has no analog unknown, so the observer is told t = 0. The fix is lane C's one line, `self.has_unknowns().then(|| self.run.accepted_time())` in `analyses/adapters.rs`; with it plus an api check on `Recorder::reached()` (the last observed step time, in place of `result.iter().last()`) the Book's Blinky runs to 3e-6 and prints its table (`count` 0, 1, 2, 3 on the 1 us grid), and the solver tests `step_observer`, `protocol_real`, `protocol_transient`, `protocol_host` and `parity_baseline` stay green. Patch of the attempt: `/var/tmp/piperine-tmp-b/t32-attempt.patch` (api side and the adapters line, uncommitted, working tree reverted). The existing test `run.rs::a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot` asserts the opposite for Blinky and will have to change with it (a decision for the user: the task rules name only the Book listing as an existing test that changes).
- GAPS.md: H14 (flow term print only in op and dc), H15 (pss of a design with an absolute `transition` time), H16 (the 2e-5 s end-snap), added in `8cd4ef2a`.
- (B5 worker) T32: committed with the authorized one-line `TransientSide::step_time` fix in `piperine-solver/src/analyses/adapters.rs` (`self.has_unknowns().then(|| self.run.accepted_time())`) and `Recorder::reached()` in place of the `result.iter().last()` check. `run.rs::a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot` is rewritten as `a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot` (HII-17: count 0, 1, 2, 3 on the 1 us grid for table, csv, plot). C70 `a_digital_only_run_that_ends_short_fails` is NOT written: no MSL design ends a transient short on this branch (`finish`/`fatal` end nothing until lane A), so the test would need a fabricated source; its place is the recorder unit test and the test is due after msl-fixes merges. The Book: the Blinky listing (and the `monitor.msl` AdderTop listing, also digital-only) now exit 0 with their tables, a second Blinky listing shows `--print-step 1e-6`, and three prose passages that said a digital-only transient is refused are rewritten; the guard's `>= 29` listings count held by the added listing. GAPS.md L56 is marked closed but its row stays: `docs.rs::gaps_drops_what_closed_and_lists_what_is_left` pins its text, so T41 drops row and pin together.
- (B5 worker) T33/T34: `DerivePlan::of(&Ir)` also keeps per binding the child param's index and type and the expression id (the printer needs them), plus `is_empty` and `giving(instance, param)`. T34: the export is `piperine_derive_1(k: u32, params: List<'_, f64>) -> f64`, not the design's `(k, *const f64, n)`: `tests/load.rs::unsafe_lives_in_load_only` forbids any `unsafe` in the transpiler's sources, so the params cross as the ABI's own safe `List<f64>` (`List::from_slice` on the host side, `List::get` in the generated code); no change to `piperine-abi`. A parent whose params count differs from what the function was printed for, or an unknown `k`, answers NaN; a derived expression over an integer param or of an integer type is `Error::Unsupported` (L28); `ValuePrinter::of_placement` is the one additive constructor in `value.rs`, `piperine_derive_1` joined the reserved names of `text.rs`. A design that derives nothing prints exactly what it printed.
- (B5 worker) T36: `Params::set(instance, param, value, derive: Option<&dyn Derive>, write: &mut dyn FnMut(&Changed) -> Result<(), String>)` writes through the caller's writer (the session's `CircuitInstance::set`), instance order with the set param first, and undoes made writes through the same writer with the old values on a refusal; `Params::changes` is the same walk with nothing written, for a timed set. A set that leaves a recomputed value equal to the old one writes nothing for it. A param the host sets always travels as a real (as every `set` has), a recomputed `bool` as a bool. The interpreter has no `Derive` (`None`): a param with readers is refused naming `--backend rust`. A param that is derived is refused for any direct set (also a dc sweep of it).
- (B5 worker) T37: `Session::params` (a `Params` seeded at open) carries every `set`: `--param`, the `dc` sweep (`Session::set_param`, one door), `Session::set_at` and the timed set (`Params::changes`, each changed param checked with `check_set` and scheduled at the same `at`, as `HostAction::Set { value: Value }`). A param with no device pin is not written when it is itself derived or others derive from it (`Session::unwritten`): a pass-through param `Mid` never reads now takes a `set` and reaches its children (H5 narrows to a pinless param nothing derives from, still the circuit's loud error). The S2 probe of the contract, in its api form: `Top.m.rr=3000` gives the middle node 0.75, and the dc sweep over 1k, 2k, 3k gives 0.5, 0.666667, 0.75. Gate: piperine-api (all), twins, all of piperine-parity (buck targets included) green.
- (B5 worker) T38: no change outside the tests. A body `let` (`let g = 1.0 / rr` in `Mid`'s behavior) follows a `set` of `rr` with no recompute (it reads the param's pin on every evaluation): 0.5 then 0.75. A definition (`y = a * k`, a `Bit<f64>` carrier) is recomputed at a timed `set` of `k` at 1.5 us: the 1 us grid holds 8, 8, 20, 20 with `a` driven to 4. Both pass on the Rust backend with no elaboration change (lane A's area untouched). Phase 8 Build gate: the Build line (zero warnings), suite_hygiene, workspace_shape, crate_boundaries green.
- (B6 worker) After the rebase onto `msl-fixes`: ONE fatal path (main's `Verdict` keeps the first failing `sim` record, `fatal at <t>: <msg>` and `FAIL <name> (fatal at <t>: <msg>)`); `Failure::Fatal` and `Verdict::or_fatal` are dropped, the target margins stay; HII-15 ACs name the new format. C70 restated: a finish-ended digital-only transient is ok (`a_digital_only_run_ended_by_finish_is_ok`; main's `a_run_ended_by_finish_gives_its_rows_to_the_instant` holds it for the analog-free design too); main's `records.rs::a_short_run_without_finish_still_fails` premised the L56 defect (a digital-only top steps nothing) and is rewritten as `a_digital_only_run_without_finish_reaches_its_stop` (HII-17 supersedes FIX-02 AC 5 for that bench). H16 still reproduces on this branch (pss with tstab 2e-5 and max-step 1e-9 fails at t = 2.8e-5), so it stays in GAPS.
- (B6 worker) T39: the root target is `crates/piperine-api/tests/root_surface/host_ii.rs` with `[[test]] name = "host_ii"` in the root manifest: `workspace_shape` forbids any `.rs` under the root `tests/` and any second source file in the root `src/`, and the guards are not edited; a subdirectory is not discovered as an api test. `Run root` is unchanged.
- (B6 worker) T41: the existing pins of `docs.rs` that named what `host-ii` closed follow it, nothing else: `gaps_drops_what_closed_and_lists_what_is_left` no longer lists the digital-only refusal (L56) and its F2 assertion goes with it; `gaps_drops_what_msl_fixes_closed` no longer keeps `### S2.` and its counts move to 3 silent, 49 loud (`gaps_drops_what_host_ii_closed` holds the absence). H5 narrows (a param a child derives from is settable) and H9 lists E0803 as reachable only by an IR marking.
- (B6 worker, after solver-hygiene) H16 no longer reproduces: `pss --tstab 2e-5 --max-step 1e-9` runs to its orbit (the stop snap of solver-hygiene), so its row is dropped from GAPS. parity_corpus count is the sum of both lanes' twins.

**Dispatched:** Batch B1 (phases 1-2, T1-T8) - Sonnet worker, 2026-11-17. B1 done (c6c0f64b, + T43 max_steps). B2 (phase 3, T9-T16) - Sonnet worker, 2026-11-17. B2 done except T14 (fc0386fd). Ruling: tf.rs input classification by resolved branch authorized as a host-ii cross-lane touch. B3 (T14, then phases 4-5 T17-T23) - Sonnet worker, 2026-11-17. B3 done (3318f886, 2034/0). Rulings: tf output-resistance fix accepted; flow-term print refused in transient/ac/pss/sens accepted (loud, to GAPS); PSS with transition state (absolute ramp.t0) and the pss end-snap defect go to GAPS (owners: periodic-small-signal / solver-hygiene). B4 (phases 6-7, T24-T32) - Sonnet worker, 2026-11-18. B4 done T24-T31; T32 blocked on lane C (see Settled). B4 done T24-T31 (d25fe92b); T32 blocked. Rulings: T32 one-line adapters.rs step_time fix authorized as cross-lane touch; run.rs test asserting Blinky fails is superseded by HII-17 (L56) and is rewritten to assert it runs to its stop; finish-ended transient acceptance comes after msl-fixes merges. B5 (T32, then phase 8 T33-T38, X9) - Sonnet worker, 2026-11-18. B5 done T32-T38. B5 done (61f96008, 2076/0). msl-fixes merged to main 99d05af2. B6 (rebase onto main, finish-ended transient ok, C70 test, phase 9 T39-T43) - Sonnet worker, 2026-11-18. B6 done, rebased onto ec4d13b6 (79967703; 2263/0; 105 proofs; MD-69). Verifier (whole feature) - Opus, 2026-11-18. Verifier done (9ae0485b, FAIL, F1-F13). Fix round - Sonnet worker, 2026-11-18 (rulings: F6/F7/F8 code follows the spec, per user rule 'O código que diverge da spec tem que ser ajustado'). Fix round done: F1 72265b6c, F2-F4 a6854b73, F5-F6 fb5ab449, F7 6ce8257c, F8 stopped (partial 0610c6aa), F9 80d13acf, F10 d78517a4, F11 031b050a, F12 89857df0, F13 641cc2e8.
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1
Phase 2:  T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6 ------→ T7 ------→ T8 ------→ T43
Phase 3:  T9 ------→ T10 ------→ T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16
Phase 4:  T17 ------→ T18 ------→ T19 ------→ T20
Phase 5:  T21 ------→ T22 ------→ T23
Phase 6:  T24 ------→ T25 ------→ T26 ------→ T27 ------→ T28
Phase 7:  T29 ------→ T30 ------→ T31 ------→ T32
Phase 8:  T33 ------→ T34 ------→ T35 ------→ T36 ------→ T37 ------→ T38
Phase 9:  T39 ------→ T40 ------→ T41 ------→ T42
```

Execution is strictly sequential - there is no intra-phase parallelism.

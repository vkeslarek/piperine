# Analog Structure Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/analog-structure/design.md`
**Status**: Approved
**Approved**: 2026-11-18 - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piperine-parity -p piperine-solver -p piperine-analog-solver -p piperine-api -p piperine -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run irstruct**: `cargo test -p piperine-ir --test structure`
**Run plan**: `cargo test -p piperine-plan --test structure_shape`
**Run tstruct**: `cargo test -p piperine-transpile --test structure`
**Run tgolden**: `cargo test -p piperine-transpile --test golden`
**Run watch**: `cargo test -p piperine-analog-solver --test assembly_watch`
**Run frozen**: `cargo test -p piperine-interp --test frozen`
**Run typestruct**: `cargo test -p piperine-lang --test types_structure`
**Run elabstruct**: `cargo test -p piperine-lang --test elab_structure`
**Run sswitch**: `cargo test -p piperine-parity --test source_switch`
**Run sderiv**: `cargo test -p piperine-parity --test source_derivative`
**Run smodules**: `cargo test -p piperine-parity --test source_modules`
**Run sreach**: `cargo test -p piperine-parity --test source_reach`
**Run sparams**: `cargo test -p piperine-parity --test source_params`
**Run partran**: `cargo test -p piperine-parity --test parity_tran`
**Run parac**: `cargo test -p piperine-parity --test parity_ac`
**Run parop**: `cargo test -p piperine-parity --test parity_op`
**Run pardc**: `cargo test -p piperine-parity --test parity_dc`
**Run corpus**: `cargo test -p piperine-parity --test parity_corpus`
**Run apibind**: `cargo test -p piperine-api --test bindings`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run clibook**: `cargo test -p piperine-cli --test book`

Executors: every task is Sonnet (user rule: "Implementação sempre em Sonnet e verificação em Opus"); a different Opus verifies the feature. The worktree is `/home/keslarek/Git/piperine-analog-structure`, branch `feature/analog-structure`. Every cargo command runs with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-a`; where parallel worktrees share the target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources. If `df /` passes 90 %, stop and report.

The tasks follow the answers of the spec's Q1 to Q3, all (a) (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"). For the record, another answer would have changed: Q1 (b) drops `Terminal::Own` from T2 and T35 becomes an `Ir::check` relaxation; Q2 (b) moves AS-05 into this feature after `generics` and `params-and-io`, and drops the local `Integ`, `CapIc` and `DiodeRs`; Q3 (b) drops T28.

Rules every task follows:

- **Parity is untouched.** `crates/piperine-parity/src/contract.rs`, the existing files of `tests/ngspice/`, the existing functions of the `parity_*` targets, `crates/piperine-cli/tests/goldens/` and `crates/piperine-transpile/tests/golden/` are not edited. New twins (`uic_rc.msl`, `diode_rs.msl`, `diode_rs.cir`) and new test functions are appended. No tolerance is written outside `ToleranceContract`, and none is changed. A task whose change moves a twin's bytes or a generated golden stops and reports.
- **Cross-lane touches are only those `design.md` names**: `piperine-analog-solver/src/device/watch.rs` and the `CAP` row of `.specs/features/device-abi/abi.md` (T14); `piperine-api/src/session.rs` (T22); the new target `crates/piperine-api/tests/bindings.rs` (T22, T32, T33, T34). No other file of `piperine-api`, `piperine-cli`, `piperine-solver`, `piperine-analog-solver`, `piperine-digital-solver` or `piperine-abi` is edited, except `crates/piperine-cli/tests/docs.rs` (T36).
- **The interpreter is frozen (R1).** `piperine-interp` gains refusal rows only (T15) and one arm per new `Terminal` (T2); none of its existing targets is edited except `frozen.rs`, which gains tests.
- **Fail loud.** A form this feature does not run is `P0001` naming it, or the spec's code; never a default, a skipped statement or a value of 0.
- **Tests state what the spec says, not what the code does.** Expected values come from `docs/spec/`, the criterion's closed form or ngspice, with the derivation written beside the assertion. Test functions have exactly the names `## Checks` cites.
- **No existing test is removed, weakened or skipped.** A test that asserts a refusal this feature lifts (a row of `elab_unlowered.rs` or `types_subset.rs`, the `S4` line of `docs.rs`) is rewritten to assert the new behavior, and its commit says which test and why.
- **AGENTS.md idiom rules and the code-clarity rules bind** (`.specs/features/code-clarity/spec.md`): no free function (every function has an owner type), no macro of your own, modules named after what they do, no abbreviated names, no god method, every new type and method doc-commented, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run**: its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`. If it is run, `git status` afterwards and restore both files.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 placement), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits and their guards), `.specs/features/code-clarity/spec.md` (the user's cleanliness rules).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| IR (`piperine-ir`) | integration | each new form built, checked (every new invariant and its violation), printed and round-tripped | `crates/piperine-ir/tests/structure.rs` | `Run irstruct` |
| Plan (`piperine-plan`) | integration | each new pin, term, function and read in the shape; a body with no new form keeps its shape | `crates/piperine-plan/tests/structure_shape.rs` | `Run plan` |
| Printer (`piperine-transpile`) | integration | each new form printed, built and evaluated against its hand-derived value; the goldens byte-identical | `crates/piperine-transpile/tests/structure.rs`, `golden.rs` | `Run tstruct`, `Run tgolden` |
| Solver touch | integration | the one behavior the touch adds; `parity_baseline` unedited | `crates/piperine-analog-solver/tests/assembly_watch.rs` | `Run watch` |
| Interpreter (`piperine-interp`) | integration | one refusal per new form, by name | `crates/piperine-interp/tests/frozen.rs` | `Run frozen` |
| Type checker (`piperine-lang/src/types`) | integration | 1:1 to the spec's typing ACs; each code (`E0210`, `E0222`, `E0243`, `E0336`, `E0343`, `E0353`, `E0410`) named | `crates/piperine-lang/tests/types_structure.rs` | `Run typestruct` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; every refusal and code (`E0321`, `E0350`, `E0414`, `E0415`) by name | `crates/piperine-lang/tests/elab_structure.rs` | `Run elabstruct` |
| Runs from source (`piperine-parity` `Project`, `Bench`) | e2e | every runtime AC on the Rust backend, values from the spec's closed forms | `crates/piperine-parity/tests/source_{switch,derivative,modules,reach,params}.rs`, fixtures `tests/structure/*.msl` | `Run sswitch` and siblings |
| ngspice twins | e2e | the new twins against ngspice under the unchanged contract; the 7 twins unchanged | `crates/piperine-parity/tests/parity_*.rs`, `tests/ngspice/` | `Run partran`, `Run parac`, `Run parop`, `Run pardc`, `Run corpus` |
| Host library (`piperine-api`) | integration | before-run bindings, timed sets and their events | `crates/piperine-api/tests/bindings.rs` | `Run apibind` |
| Command line and the Book (`piperine-cli`, `docs/book`) | e2e | the twins' bytes; every listing equals its output | `crates/piperine-cli/tests/{twins,book}.rs` | `Run twins`, `Run clibook` |
| Records (`GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`) | integration | each claim of AS-18 AC 4 read as text | `crates/piperine-cli/tests/docs.rs` | `Run docs` |
| Feature records (`contract.md`) | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes generated code or what a run prints | Quick, then `cargo test -p <the crate the task edits>`, then `Run twins` and `Run tgolden` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Five batches: phase 1 (7 tasks), phase 2 (8), phase 3 (7), phase 4 (6), phases 5 and 6 (8).

### Phase 1: Contract and the IR

```
T1 → T2 → T3 → T4 → T5 → T6 → T7
```

### Phase 2: Plan, printer, solver and interpreter

```
T3 → T8 → T9 → T10
T4 → T9
T5 → T10
T6 → T10
T7 → T10
T8 → T11 → T12 → T13
T9 → T12
T10 → T13
T1 → T14
T7 → T15
```

### Phase 3: Control flow, switch branches and core sites in the front end

```
T3 → T16
T11 → T16 → T17
T16 → T18 → T19
T6 → T19
T18 → T22
T16 → T20 → T21
T12 → T20
T13 → T21
T14 → T21
T16 → T21
```

### Phase 4: The twins, and modules in expressions

```
T20 → T23
T19 → T24
T21 → T24
T1 → T25 → T27
T1 → T26 → T27
T17 → T27
T20 → T27
T27 → T28
```

### Phase 5: Reaching into a child, `inflow`, params and history

```
T1 → T29 → T30
T6 → T30
T13 → T30
T16 → T31
T19 → T31
T1 → T32
T7 → T33
T13 → T33
T22 → T33
T18 → T34
T22 → T34
T2 → T35
```

### Phase 6: The records

```
T28 → T36
T35 → T36
```

---

## Task Breakdown

### T1: Contract: the cited code and the solver facts the design rests on

**What**: `contract.md`: rebase on `main` once `host-ii` and `solver-hygiene` have merged (`git fetch` and `git rebase main`), confirm every file, line and name `design.md` cites against the rebased `HEAD`, re-run each row of `design.md` "What the code does today" as a probe and record its output, and confirm with short throwaway runs (not committed) eight facts: (1) a host `set` of a param during a transient marks the param pin's `CHANGED` byte for an event function at the set's instant, and a before-run binding fires nothing; (2) a param pin's `PREVIOUS` before any `set` holds its own value; (3) a `DDT` term's `abstol` pin is read by the solver (`solver-hygiene` V1); (4) the AC sweep re-runs an analog function that reads `STATIC` with `false` (`ac.rs:124`); (5) a device `Fault` reaches the run error with the instance's path and the device's message; (6) the residual test of a private flow pin's row; (7) whether the transpiler's library cache key holds instance params (`cache.rs`); (8) what refuses a `limit` or `ac_stim` whose input reads a carrier through `known_or` today (L50). Each difference gets the task that absorbs it; a fact that does not hold is reported to the orchestrator before phase 2.
**Where**: `.specs/features/analog-structure/contract.md`
**Depends on**: None
**Reuses**: `design.md`; the probes of `GAPS.md`
**Requirement**: AS-01 to AS-18
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists each cited location as confirmed or moved, each probe with its output, and the eight facts with their evidence
- [x] The branch is rebased on `main` with `host-ii` and `solver-hygiene` merged, and the `**Build**` line passes
- [x] No code is edited
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): analog-structure contract against the tree`

---

### T2: `Terminal::Own` in the IR

**What**: `Terminal::Own(u32)`: valid only in a `Child::nodes` slot of an own signal of the child's body (`Ir::check` names any other use and an index that is no signal of the child), printed `own:<name>` in the tree and `own<k>` in the arenas, serialized by serde; every exhaustive match over `Terminal` in `piperine-plan`, `piperine-transpile` and `piperine-interp` gains an arm that panics naming the `Ir::check` invariant (Q1, design C1).
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `print.rs`, and the one arm per match in `piperine-plan/src/plan.rs`, `piperine-transpile/src/print/*.rs`, `piperine-interp/src/*.rs`)
**Depends on**: T1
**Reuses**: `Terminal::Ground` printing; `tests/serde.rs` round trip
**Requirement**: AS-16
**Executor**: Sonnet

**Done when**:

- [x] Creates the target `crates/piperine-ir/tests/structure.rs` (`//!` header: the IR forms of `analog-structure`: built, checked, printed, round-tripped)
- [x] Tests with exactly these names pass in `Run irstruct`: `an_own_terminal_prints_and_round_trips`, `an_own_terminal_outside_a_child_is_broken`, `an_own_terminal_that_names_a_port_is_broken`
- [x] Gate check passes: Full (`Run irstruct`, `cargo test -p piperine-ir`, `Run twins`, `Run tgolden`)
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): an own terminal names a child's own node`

---

### T3: Switch branches and contributions under a condition in the IR

**What**: `BranchKind::Switch`; `Ir::check` accepts a `Stmt::Contribute` anywhere inside the analog block (an `if` arm's block, a nested block, a block's value) and keeps refusing one in a function, a closure or a reaction; both quantities may contribute to a `Switch` branch, a `Flow` or `Potential` branch keeps its one quantity; `Body::contributions()` lists every contribution with its enclosing conditions in source order; the `deps` walk attributes a nested contribution to its branch's `Flow`/`Charge` target and adds its enclosing conditions' sources (design mechanism 1, 2).
**Where**: `crates/piperine-ir/src/check.rs` (with `model.rs`, `deps.rs`, `handle.rs`, `print.rs`)
**Depends on**: T2
**Reuses**: `Walk`/`Ctx` of `check.rs`; the `If` and `Block` arms of the `deps` walk
**Requirement**: AS-01, AS-02, AS-03
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run irstruct`: `a_contribution_in_an_if_arm_passes_check`, `a_contribution_in_a_function_or_a_reaction_is_broken`, `a_switch_branch_takes_both_quantities`, `a_flow_branch_refuses_a_potential_contribution`, `contributions_lists_each_with_its_enclosing_conditions`, `a_nested_contribution_reads_its_conditions_in_its_branch_dependencies`
- [x] Gate check passes: Full (`Run irstruct`, `cargo test -p piperine-ir`, `Run twins`, `Run tgolden`)
- [x] Test count: the 6 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): switch branches and contributions under a condition`

---

### T4: Derivative sites in the IR

**What**: `BodyData::derivatives` (`Derivative { name, state, abstol, origin }`, serde default, printed only when present), `Expr::Derivative(u32)` (an `f64` read of site `k`'s rate, allowed in analog, a body `let`, an event argument and a reaction), `deps::Target::Derivative(k)` (the state's reads) and `Target::DerivativeAbstol(k)` (the `abstol`'s reads, which must read no probe, carrier or clock); `Ir::check` names a site out of range, a state that is not `f64` and an `abstol` that reads a probe (design mechanism 3).
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`, `handle.rs`)
**Depends on**: T3
**Reuses**: `Site` and `EventSource::Cross` tolerances (msl-fixes C11) as the pattern of an `abstol` expression
**Requirement**: AS-07
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run irstruct`: `a_derivative_site_prints_and_round_trips`, `a_derivative_read_out_of_range_is_broken`, `a_derivative_abstol_that_reads_a_probe_is_broken`, `a_derivative_state_reads_are_its_dependencies`
- [x] Gate check passes: Full (`Run irstruct`, `cargo test -p piperine-ir`, `Run twins`, `Run tgolden`)
- [x] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): derivative sites`

---

### T5: Guarded sites in the IR

**What**: `Site::enabled: Option<ExprId>` (serde default, skipped when `None`) and `Stmt::BoundStep::when: Option<ExprId>`; `Ir::check` requires a `limit` read under a condition to have `enabled` and one at the top to have none, accepts an `ac_stim` read under a condition, and keeps a `BoundStep` at the top of the analog block; `Target::Limit(k)` and `Target::BoundStep(k)` read `enabled` and `when` (design "Guarded core sites").
**Where**: `crates/piperine-ir/src/check.rs` (with `model.rs`, `deps.rs`, `print.rs`)
**Depends on**: T4
**Reuses**: `Checker::site` (`check.rs:1596`)
**Requirement**: AS-01, AS-06
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run irstruct`: `a_limit_under_a_condition_needs_enabled`, `a_top_level_limit_has_no_enabled`, `an_ac_stim_under_a_condition_passes_check`, `a_bound_step_with_when_reads_its_condition`
- [x] Gate check passes: Full (`Run irstruct`, `cargo test -p piperine-ir`, `Run twins`, `Run tgolden`)
- [x] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): guarded limit and bound_step sites`

---

### T6: Structure records and the builder's node merge and reach binding

**What**: `BodyData::hoisted` (`Hoisted`), `merges` (`Merge`), `reaches` (`Reach`, `ReachedItem`), `inflows` (`Inflow`); `NodeRole::Reached`, `CarrierRole::Reached`; `IrBuilder::merge_nodes(kept, merged)` (a union applied by `finish`, which rewrites every instance's node refs and compacts the node arena), `IrBuilder::bind_reached_node(instance, slot, NodeRef)` and `bind_reached_net(instance, carrier, NetId)`; `Ir::check` requires every instance of a body with a `Merge` to bind both ends to one ref, every `Reached` slot to be bound, and a reached carrier never to be written (design mechanisms 4, 5 and "Reaching into a child").
**Where**: `crates/piperine-ir/src/build.rs` (with `model.rs`, `check.rs`, `print.rs`)
**Depends on**: T5
**Reuses**: `IrBuilder::bind_nets`
**Requirement**: AS-02, AS-09, AS-11, AS-13
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run irstruct`: `merging_two_nodes_leaves_one_and_rewrites_every_instance`, `merging_a_node_into_ground_removes_it`, `an_instance_that_binds_a_merge_to_two_nodes_is_broken`, `a_reached_node_is_bound_to_the_childs_node`, `a_write_to_a_reached_carrier_is_broken`, `the_structure_records_print_and_round_trip`
- [x] Gate check passes: Full (`Run irstruct`, `cargo test -p piperine-ir`, `Run twins`, `Run tgolden`)
- [x] Test count: the 6 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): hoisted, merged, reached and inflow records`

---

### T7: Param events in the IR

**What**: `EventSource::ParamChanged(u32)` (fired when param `p`'s pin changes) and `Expr::ParamPrevious(u32)` (an `f64` or the param's type, never flagged: a param is known); `Body::param_changes(target)` and `Body::param_previous_reads(target)` list them by param as `carrier_reads` lists carriers; `Ir::check` names a param out of range (design "param history").
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`)
**Depends on**: T6
**Reuses**: `EventSource::Changed`, `Expr::Previous`
**Requirement**: AS-14
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run irstruct`: `a_param_changed_event_prints_and_round_trips`, `a_param_previous_read_is_typed_as_the_param`, `a_param_event_out_of_range_is_broken`
- [x] Gate check passes: Build
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(ir): changed and prev of a param`

---

### T8: Plan: switch branches and contributions under a condition

**What**: `BodyPlan` reads what each branch receives through `Body::contributions()` (nested ones included); a `Switch` branch gets the private flow pin and the `Current` and `Eq` terms of a potential branch, and a `Charge` term on its flow pin only for an unconditional whole `ddt` addend; the `terms` function reads the sources of every enclosing condition; a switch branch's `Eq` term is never affine; a body with no new form plans as today (design C2).
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T3
**Reuses**: `Received`, `Term::Current`/`Term::Eq`
**Requirement**: AS-01, AS-03
**Executor**: Sonnet

**Done when**:

- [x] Creates the target `crates/piperine-plan/tests/structure_shape.rs` (`//!` header: the shape of each `analog-structure` form)
- [x] Tests with exactly these names pass in `Run plan`: `a_switch_branch_has_a_flow_pin_and_an_equation_term`, `a_conditional_flow_contribution_reads_its_condition`, `a_switch_equation_is_not_affine`, `a_body_without_new_forms_keeps_its_shape`
- [x] Gate check passes: Full (`Run plan`, `cargo test -p piperine-plan`, `Run twins`, `Run tgolden`)
- [x] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): switch branches and conditional contributions`

---

### T9: Plan: derivative sites and their `abstol` pins

**What**: per derivative site `k`, a `PRIVATE` continuous pin `d<k>.rate` after the branch flow pins, the terms `d<k>.rate` (`FLOW {u, GROUND}`, `Term::DerivativeRate`) and `d<k>.state` (`DDT {u, GROUND}`, `Term::DerivativeState`) after the branch terms, written by `terms`; a site with `abstol` gets a `PRIVATE` discrete `F64` pin `d<k>.abstol` after the `etol` pins, written by `Mode::DerivativeAbstol(k)` in the parameter cone and named by its `DDT` term; `Source::Derivative(k)` is a differentiable pin read (design mechanism 3).
**Where**: `crates/piperine-plan/src/plan.rs` (with `digital.rs` for the pin order)
**Depends on**: T4, T8
**Reuses**: `Digital::tolerance_pins`, `Mode::Ttol`
**Requirement**: AS-07
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run plan`: `a_derivative_site_has_a_rate_pin_and_two_terms`, `a_derivative_abstol_names_its_pin`, `a_derivative_read_is_a_differentiable_pin_read`
- [x] Gate check passes: Full (`Run plan`, `cargo test -p piperine-plan`, `Run twins`, `Run tgolden`)
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): derivative sites and their abstol pins`

---

### T10: Plan: guarded sites, param events and reached carriers

**What**: `Mode::Limit(k)` reads the sources of the site's `enabled`; `Mode::Cap(k)` reads `when`; `Source::ParamChanged(p)` (the param pin's `CHANGED`, read by an event function) and `Source::ParamPrevious(p)` (its `PREVIOUS`); a `Reached` carrier is a `PORT` discrete pin as an `in` port; every new pin, function and read is appended after the existing ones (design C2).
**Where**: `crates/piperine-plan/src/plan.rs` (with `digital.rs`)
**Depends on**: T5, T6, T7, T9
**Reuses**: `Source::CarrierChanged`, `Source::CarrierPrevious`, `Digital::carrier_pins`
**Requirement**: AS-06, AS-01, AS-11, AS-14
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run plan`: `a_guarded_limit_reads_its_enabled_sources`, `a_guarded_cap_reads_its_when_sources`, `a_param_changed_event_reads_the_params_changed_byte`, `a_param_previous_reads_its_previous_part`, `a_reached_carrier_is_a_port_pin`
- [ ] Gate check passes: Full (`Run plan`, `cargo test -p piperine-plan`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 5 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): guarded sites, param events and reached carriers`

---

### T11: Printer: the structured terms block, switch equations and `E0418`

**What**: `ContributionSums` (`print/contribution.rs`) names each branch's sums and enable flags and the text of a nested `Stmt::Contribute`; `ValuePrinter::with_contribution_sums` prints a nested contribution as an accumulation; `AnalogPrinter::terms` prints the structured form for a body with a nested contribution or a switch branch (`let mut` sums, the block as written, `terms_block` returning `Result<[R; n], Fault>`), and today's form otherwise; a switch branch's `Eq` is selected per solve and both kinds enabled return `Fault::new(418, "E0418: ...")` naming the branch (design mechanisms 1, 2).
**Where**: `crates/piperine-transpile/src/print/contribution.rs` (with `print/analog.rs`, `print/value.rs`)
**Depends on**: T8
**Reuses**: `ValuePrinter::block_lines`, `AnalogPrinter::slice`
**Requirement**: AS-01, AS-03
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-transpile/tests/structure.rs` (`//!` header: each `analog-structure` form printed, built with `rustc` and evaluated against a hand-derived value)
- [ ] Tests with exactly these names pass in `Run tstruct`: `a_conditional_flow_contribution_adds_only_when_its_arm_runs`, `a_switch_equation_follows_its_enabled_kind`, `a_switch_with_neither_kind_has_zero_flow`, `both_kinds_enabled_return_e0418_naming_the_branch`
- [ ] `Run tgolden` passes unedited
- [ ] Gate check passes: Full (`Run tstruct`, `cargo test -p piperine-transpile`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): conditional contributions and switch branches`

---

### T12: Printer: derivative sites

**What**: `DerivativePrinter` (`print/derivative.rs`): the state `let`s of every site at the top of `terms_block`, outside any `if`; the writes `d<k>.rate` (`u`) and `d<k>.state` (`−x`); the read of `d<k>.rate` wherever `Expr::Derivative(k)` stands, in `terms`, a `cross` watch or a reaction; the `d<k>.abstol` function (design mechanism 3).
**Where**: `crates/piperine-transpile/src/print/derivative.rs` (with `print/analog.rs`, `print/value.rs`)
**Depends on**: T9, T11
**Reuses**: `AnalogPrinter::tolerance` (msl-fixes T26)
**Requirement**: AS-07
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run tstruct`: `a_derivative_state_is_written_outside_every_condition`, `a_derivative_read_is_its_rate_pin`, `a_derivative_abstol_function_writes_its_pin`
- [ ] Gate check passes: Full (`Run tstruct`, `cargo test -p piperine-transpile`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): derivative sites`

---

### T13: Printer: guarded sites, param events and reached carriers

**What**: the `limit:<k>` function writes `PROPOSED = LIMITED = LAST` when the site's `enabled` is false; the `cap:<site>` function writes `+∞` when `when` is false; an event function reads a param pin's `CHANGED` for `ParamChanged`, and `ParamPrevious` decodes its `PREVIOUS`; a reached carrier decodes like an `in` port (design "Guarded core sites").
**Where**: `crates/piperine-transpile/src/print/limit.rs` (with `print/site.rs`, `print/digital.rs`)
**Depends on**: T10, T12
**Reuses**: `LimitPrinter`, `AnalogPrinter::cap`
**Requirement**: AS-06, AS-01, AS-14, AS-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run tstruct`: `a_disabled_limit_writes_its_last_value`, `a_disabled_cap_writes_infinity`, `a_param_change_fires_its_event`, `a_param_previous_decodes_its_previous_bytes`
- [ ] Gate check passes: Full (`Run tstruct`, `cargo test -p piperine-transpile`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): guarded sites, param events and reached carriers`

---

### T14: Solver: a `CAP` watch of `+∞` caps nothing (cross-lane)

**What**: `device/watch.rs` reads `+∞` in a `CAP` watch as no cap; `−∞`, `NaN`, zero and negative values stay `E0509`; the `CAP` row of `abi.md` gains "A value of `+∞` caps nothing: a `bound_step` not executed at the accepted solution" (design "Guarded core sites").
**Where**: `crates/piperine-analog-solver/src/device/watch.rs` (and `.specs/features/device-abi/abi.md`)
**Depends on**: T1
**Reuses**: the `CROSS` `NaN` amendment of msl-fixes T23
**Requirement**: AS-01
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run watch`: `a_cap_of_infinity_caps_nothing`, `a_cap_of_minus_infinity_or_nan_is_e0509`
- [ ] `cargo test -p piperine-solver --test parity_baseline` passes unedited
- [ ] Gate check passes: Quick (`Run watch`), then `cargo test -p piperine-analog-solver`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(analog-solver): an infinite step cap caps nothing`

---

### T15: The interpreter refuses every new form by name

**What**: `Interpreter::amended_forms` gains one row per form (design C5): a contribution under a selector, a switch branch, a module placed in an expression with a node or an analog block, a `ddt` off a whole addend or with `abstol`, a reached node or carrier, `inflow()`, `changed()` or `prev()` on a param, a guarded `limit`, a `bound_step` under a selector (R1, AS-17).
**Where**: `crates/piperine-interp/src/lib.rs`
**Depends on**: T7
**Reuses**: the rows of msl-fixes T12
**Requirement**: AS-17
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `Run frozen`: `the_interpreter_refuses_each_analog_structure_form_by_name`, `an_analog_body_without_new_forms_still_runs_on_the_interpreter`
- [x] Gate check passes: Build
- [x] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(interp): refuse the analog-structure forms by name`

---

### T16: Contributions under `if`, `match` and a nested block; branch kinds

**What**: `elab/selectors.rs`: `Scope` keeps the stack of enclosing `ArmCondition`s (pushed by the `if` arm of `lower_expr`, by `arms_chain` for a `match` arm, by the right of `&&`/`||`); `lower_contribution` no longer refuses under a selector or in a nested block and records a `ContributionRecord`; after every analog block of the body is lowered, `BranchKinds::decide` fixes each branch's kind (`Flow`, `Potential`, `Switch`) and reports `E0415` for unselected potential and flow contributions; `potential_branch` scans nested contributions; `E0241` holds under an `if`; the rows `ConditionalContribution` and `NestedContribution` of `elab/unlowered.rs` go (design mechanisms 1, 2).
**Where**: `crates/piperine-lang/src/elab/selectors.rs` (with `contribute.rs`, `lower.rs`, `compound.rs`, `analog.rs`, `unlowered.rs`)
**Depends on**: T3, T11
**Reuses**: `lower_expr`'s `If` arm, `arms_chain`
**Requirement**: AS-01, AS-02, AS-03
**Executor**: Sonnet

**Done when**:

- [ ] Creates the targets `crates/piperine-lang/tests/elab_structure.rs` and `crates/piperine-parity/tests/source_switch.rs` (`//!` headers: the IR of each `analog-structure` form and its refusals; contributions under control flow, switch branches and guarded sites run from source on the Rust backend), fixtures under `crates/piperine-parity/tests/structure/`
- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_contribution_in_a_nested_block_lowers_as_at_the_top`, `a_contribution_in_a_closure_is_refused_at_parse`, `a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown`, `a_contribution_to_a_branch_from_a_node_to_itself_under_an_if_is_e0241`
- [ ] Tests with exactly these names pass in `Run sswitch`: `a_flow_contribution_under_an_if_on_a_probe_gives_one_volt`, `a_match_on_a_carrier_adds_only_the_selected_arm`, `an_untaken_contribution_leaves_the_branch_with_zero_flow`, `the_switch_shorts_at_one_microsecond`, `a_switch_changes_kind_at_the_step_after_the_instant`, `both_kinds_enabled_in_one_solve_fail_with_e0418`, `an_ac_sweep_takes_the_switch_kind_with_static_false`, `two_enabled_flow_arms_sum`
- [ ] The rows of `elab_unlowered.rs` that asserted the two lifted refusals are rewritten to assert the lowering, named in the commit
- [ ] Gate check passes: Full (`Run elabstruct`, `Run sswitch`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 12 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): contributions under if and match, switch branches`

---

### T17: The selector rule, `E0414`

**What**: after the branch kinds are decided, every contribution to a branch that has a potential contribution anywhere in the body (hoisted frames included) whose enclosing conditions read a probe, a branch flow, a derivative site or a `limit` value is `E0414` naming the branch and the probe; the check runs on the lowered conditions, so a hoisted frame is judged per placement after its value inputs are substituted (§11 rule 3).
**Where**: `crates/piperine-lang/src/elab/selectors.rs`
**Depends on**: T16
**Reuses**: `Elaboration::continuous`-style walks over lowered expressions
**Requirement**: AS-02
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414`, `a_probe_selecting_between_flow_contributions_is_accepted`
- [ ] Gate check passes: Quick (`Run elabstruct`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): the selector rule of a potential-capable branch (E0414)`

---

### T18: Key params and body variants

**What**: `elab/structural.rs`: `KeyParams::of` classifies a module's key params (structural: read by a params-only selector of a potential-capable branch; initial: read by a node's or carrier's initial value; transitive through a placement's param value), a fixpoint over the reached modules; `BodyKey { module, keys }`: an unkeyed module keeps its reached-order id and name, a keyed variant is allocated on request after every unkeyed body, named `Name~p=v`, lowered with its key params bound to constants; a structural key param is marked `Param::structural` (design mechanism 4).
**Where**: `crates/piperine-lang/src/elab/structural.rs` (with `elaborator.rs`, `body.rs`, `children.rs`, `place.rs`)
**Depends on**: T16
**Reuses**: `Variants` (`elab/variants.rs`), `ConstEval`, `Bindings`
**Requirement**: AS-02
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run elabstruct`: `params_only_selectors_fix_the_kind_and_mark_the_params_structural`, `two_instances_with_different_key_values_get_two_bodies`, `a_parent_param_feeding_a_key_param_is_a_key`, `a_design_without_key_params_keeps_its_body_ids`
- [ ] Gate check passes: Full (`Run elabstruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): body variants keyed by structural and initial params`

---

### T19: Params-only selectors fold; `E0415` per variant; the merge of a zero-potential branch

**What**: in a variant, a condition that reads only key params and constants folds and only the arm it selects is lowered; unselected potential and flow contributions left after the fold are `E0415`; a fixed `Potential` branch whose potential contributions are unselected literal zeros and whose flow nothing reads is dropped and recorded as `Merge`, and placement calls `IrBuilder::merge_nodes` for every instance of that body (design mechanism 4).
**Where**: `crates/piperine-lang/src/elab/selectors.rs` (with `place.rs`, `lower.rs`)
**Depends on**: T6, T18
**Reuses**: `ConstEval::value` over the variant's bindings
**Requirement**: AS-02
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_fixed_zero_potential_branch_merges_its_nodes`, `params_only_selectors_enabling_both_kinds_are_e0415`, `a_zero_potential_branch_whose_flow_is_read_is_not_merged`, `a_merge_with_ground_binds_the_node_to_ground`
- [ ] Gate check passes: Full (`Run elabstruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): fold params-only selectors and merge zero-potential branches`

---

### T20: `ddt` wherever a core module goes

**What**: `elab/derivatives.rs`: `DerivativeUses::of(module)` decides per `ddt` call whether it is a whole addend (written as a leaf of an unconditional contribution with no `abstol`, directly or through a body `let` read only so) or a site; a site's state (and `abstol`) is lowered standalone (no locals, block-local `let`s inlined, scrutinees re-read), one site per frame and call (`Frame::derivatives`); every read lowers to `Expr::Derivative(k)` in analog, body `let`s, events and reactions; `let y = ddt(f(y));` stays `E0353`; a `ddt` in a closure is `E0350`; the rows `DdtOffTop` and `DdtTolerance` go (design mechanism 3).
**Where**: `crates/piperine-lang/src/elab/derivatives.rs` (with `contribute.rs`, `analog.rs`, `lower.rs`, `hoist.rs`, `unlowered.rs`)
**Depends on**: T12, T16
**Reuses**: `Scope::no_locals`, `Subject::Reread`, `Frame::placements` as the memo pattern
**Requirement**: AS-07, AS-04
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-parity/tests/source_derivative.rs` (`//!` header: `ddt` off a whole addend, in a `let`, read by an event, with `abstol`, run from source on the Rust backend) and the fixtures `tests/structure/tran_rc_scaled.msl`, `ac_rc_scaled.msl` (the twins with `i(p, n) +> 2.0 * ddt(0.5 * c * v(p, n));`)
- [ ] Tests with exactly these names pass in `Run partran`: `tran_rc_with_a_scaled_derivative`; in `Run parac`: `ac_rc_with_a_scaled_derivative` (each against the existing `tran_rc` or `ac_rc` reference, Rust backend)
- [ ] Tests with exactly these names pass in `Run sderiv`: `a_derivative_times_a_probe_is_k_v_dv_dt`, `an_event_reads_the_derivative_of_the_step`, `a_looser_derivative_abstol_takes_fewer_steps`, `bounded_holds_its_state_at_the_bound`
- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_body_let_of_ddt_read_as_an_addend_gives_the_inline_rows`, `a_ddt_read_in_analog_and_in_an_event_is_one_site`, `a_ddt_feedback_through_its_own_result_is_e0353`
- [ ] Gate check passes: Full (`Run sderiv`, `Run elabstruct`, `Run partran`, `Run parac`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 9 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): ddt wherever a core module goes`

---

### T21: Guarded core sites: `limit`, `ac_stim` and `bound_step` under a selector, reading a carrier

**What**: a `limit` under a selector lowers its `x`, `f` and `seed` standalone and carries `enabled` (the conjunction of its enclosing conditions, standalone); an `ac_stim` under a selector lowers its magnitude and phase standalone; a `bound_step` under a selector moves to the top of the analog block with `when`; a `limit` or `ac_stim` input that reads a carrier through `known_or` or `level` is accepted (lifting the refusal T1 found); a `limit` in a closure is `E0350`; the rows `ConditionalLimit`, `ConditionalStim` and `ConditionalBoundStep` go (design "Guarded core sites").
**Where**: `crates/piperine-lang/src/elab/analog.rs` (with `unlowered.rs`, `derivatives.rs` for the shared standalone lowering)
**Depends on**: T13, T14, T16, T20
**Reuses**: the standalone lowering of T20
**Requirement**: AS-06, AS-01
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run sswitch`: `a_limit_under_an_untaken_branch_neither_vetoes_nor_moves`, `a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution`, `a_limit_reads_a_carrier_through_known_or`, `an_ac_stim_under_an_if_drives_only_when_enabled`, `an_ac_stim_reads_its_magnitude_from_a_carrier`, `a_bound_step_under_an_if_caps_only_the_steps_that_executed_it`
- [ ] Test with exactly this name passes in `Run elabstruct`: `a_limit_in_a_closure_is_e0350`
- [ ] Gate check passes: Full (`Run sswitch`, `Run elabstruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 7 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): limit, ac_stim and bound_step under a selector`

---

### T22: Before-run bindings of key params are elaboration inputs (cross-lane)

**What**: `Overrides` and `Elaborator::run_with(top, &Overrides)` (`run` is `run_with` with none): a binding of a key param, by instance path, selects that instance's variant at placement; a binding of any other param is ignored by the elaborator; `Session::open` passes the run's resolved before-run bindings and still writes the others as today (design mechanism 4, C6).
**Where**: `crates/piperine-lang/src/elab/structural.rs` (with `elaborator.rs`, `place.rs`, and the one call in `crates/piperine-api/src/session.rs`)
**Depends on**: T18
**Reuses**: `host-ii`'s `ParamPath::resolve`
**Requirement**: AS-15, AS-14, AS-02
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-api/tests/bindings.rs` (`//!` header: before-run bindings, timed sets and the events a set fires, through `Session`)
- [ ] Tests with exactly these names pass in `Run apibind`: `a_before_run_binding_of_a_structural_param_selects_its_variant`, `a_before_run_binding_of_a_plain_param_is_still_the_hosts_write`
- [ ] Test with exactly this name passes in `Run elabstruct`: `an_override_of_a_key_param_lowers_its_variant`
- [ ] `cargo test -p piperine-api` passes with no `host-ii` test edited
- [ ] Gate check passes: Build
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(api,elab): before-run bindings of key params elaborate`

---

### T23: The `uic_rc` twin and `CapIc`

**What**: `tests/ngspice/uic_rc.msl`: the circuit of `uic_rc.cir` (5 V, 1 kΩ) with a local `CapIc` (§16 H7, `c` = 1.5915494309189535 nF, `ic` = 2.0); `parity_tran.rs` gains `fn uic_rc()` (Rust backend; the interpreter's refusal is T15's) and its skip-line test; `source_switch.rs` gains the AC admittance of `CapIc` (§11.1.5).
**Where**: `tests/ngspice/uic_rc.msl` (with `crates/piperine-parity/tests/parity_tran.rs`, `source_switch.rs`)
**Depends on**: T20
**Reuses**: `compare_on` of `parity_tran.rs`; `Ngspice::skip_line`
**Requirement**: AS-03, AS-18
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run partran`: `uic_rc`, `uic_rc_runs_and_prints_the_skip_line_without_ngspice`
- [ ] Test with exactly this name passes in `Run sswitch`: `capic_has_the_admittance_of_a_capacitor_in_ac`
- [ ] `Run corpus` passes unedited (the twin has its `.cir` and its function)
- [ ] Gate check passes: Full (`Run partran`, `Run sswitch`, `Run corpus`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the uic_rc twin with a held capacitor`

---

### T24: The `diode_rs` twin and a local `DiodeRs`

**What**: `tests/ngspice/diode_rs.cir` (5 V, 1 kΩ, the `rdiode` diode with `rs=100`) and `diode_rs.msl` with a local `DiodeRs` whose `rs: f64` selects `i(a, ai) +> v(a, ai) / rs` (`rs > 0.0`) or `v(a, ai) +> 0.0` (the params-only pattern of the §18 `Diode` without `Option`, Q2); `parity_op.rs` gains `fn diode_rs()` (`rs` = 100 against `diode_rs.cir`) and a run with `rs` = 0 against `rdiode.cir`, whose bound circuit has one node fewer (AS-02 AC 4), and its skip-line test; `elab_structure.rs` gains the §18 `Diode` refused by name until `params-and-io` (AS-05 AC 3).
**Where**: `tests/ngspice/diode_rs.msl` (with `diode_rs.cir`, `crates/piperine-parity/tests/parity_op.rs`, `crates/piperine-lang/tests/elab_structure.rs`)
**Depends on**: T19, T21
**Reuses**: `OperatingPoint` of `piperine-parity`
**Requirement**: AS-02, AS-05, AS-18
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run parop`: `diode_rs`, `diode_rs_with_rs_zero_merges_and_matches_rdiode`, `diode_rs_runs_and_prints_the_skip_line_without_ngspice`
- [ ] Test with exactly this name passes in `Run elabstruct`: `the_section_18_diode_waits_for_params_and_io`
- [ ] `Run corpus` passes unedited
- [ ] Gate check passes: Full (`Run parop`, `Run elabstruct`, `Run corpus`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the diode_rs twin with a params-only series resistance`

---

### T25: Feedback through an instance; `E0353`

**What**: `Checker::self_read` types a body `let` whose value is a placement as the placed module's declared result type when every result on the cycle back to it has no direct dependence on its input (§8.3.5), and reports `E0353` naming the cycle's names when every result on it has one; the `P0001` "the feedback of .. through an instance" goes.
**Where**: `crates/piperine-lang/src/types/result.rs`
**Depends on**: T1
**Reuses**: `result_depends_directly`, `placement_type`
**Requirement**: AS-10
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-lang/tests/types_structure.rs` (`//!` header: the typing of each `analog-structure` form and its codes)
- [ ] Tests with exactly these names pass in `Run typestruct`: `feedback_through_an_unknown_of_a_module_is_accepted`, `a_cycle_with_a_direct_dependence_at_every_result_is_e0353`
- [ ] Gate check passes: Quick (`Run typestruct`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above, and no existing test removed

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): feedback through an instance with an unknown`

---

### T26: A continuous argument read by a callee's definition is `E0336`

**What**: at a placement of a module with a value input, an argument that is a continuous expression (§8.2) for an input the callee reads in a definition, directly or through a `let`, is `E0336` at the call naming the input (§8.3.9).
**Where**: `crates/piperine-lang/src/types/definition.rs`
**Depends on**: T1
**Reuses**: `Checker::analog_value_input` (`types/result.rs`) as the pattern; `reads_decl`
**Requirement**: AS-08
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run typestruct`: `a_continuous_argument_read_by_a_callee_definition_is_e0336`, `a_discrete_argument_read_by_a_callee_definition_is_accepted`
- [ ] Gate check passes: Quick (`Run typestruct`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a continuous value input read by a definition is E0336`

---

### T27: Modules in expressions: value inputs, results that hold a node, feedback

**What**: a module with a value input is hoisted like one with a result (`refuse_digital_forms` and the `ValueInput` row go); `Frame::nodes` maps a node port to the caller's terminal and a node `signal` to a body node named under the placement; `hoistable` accepts nodes and `analog` blocks; after the body's own `analog` blocks, each hoisted frame's blocks are lowered in frame order into the body's statements; `BodyData::hoisted` records each frame that adds a node or an analog statement (design mechanism 5).
**Where**: `crates/piperine-lang/src/elab/hoist.rs` (with `carriers.rs`, `analog.rs`, `body.rs`, `elaborator.rs`, `unlowered.rs`)
**Depends on**: T17, T20, T25, T26
**Reuses**: `Frame`, `frame_of`, `in_caller`, `hoisted_params`
**Requirement**: AS-08, AS-09, AS-10, AS-04, AS-02, AS-05
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-parity/tests/source_modules.rs` (`//!` header: value inputs, results that hold a node, feedback and the local `Integ`, run from source on the Rust backend)
- [ ] Tests with exactly these names pass in `Run smodules`: `a_value_input_of_a_module_without_a_result_drives_its_contribution`, `a_value_input_is_differentiated_in_ac`, `a_value_input_reads_the_callers_carrier_from_its_change`, `a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant`, `lag_follows_one_minus_exp`, `lag_is_a_first_order_low_pass_in_ac`, `a_lag_under_an_untaken_branch_keeps_integrating`, `a_named_lag_is_one_instance_for_analog_and_its_event`, `feedback_through_integ_decays_exponentially`, `integ_holds_ic_while_reset_then_integrates`, `integ_is_one_over_j_omega_in_ac`, `integ_holds_its_result_at_ic_at_the_operating_point`
- [ ] Tests with exactly these names pass in `Run elabstruct`: `two_placements_with_equal_arguments_are_two_instances`, `a_selector_through_a_value_input_is_judged_per_placement`, `the_stdlib_idt_waits_for_generics`, `the_water_model_waits_for_generics`
- [ ] Gate check passes: Full (`Run smodules`, `Run elabstruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 16 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): hoist modules with value inputs, nodes and analog blocks`

---

### T28: `slew` from the stdlib runs

**What**: `std::slew` (§22.2.10) elaborates and runs on the Rust backend: its `Option` value inputs, `let (r, f)`, the `ddt(x)` site read by its events, the `match` on its `Bit<Slewing>` carrier with a contribution in each arm, and its three reactions; any lowering gap it meets is closed here or reported before the task closes (Q3).
**Where**: `crates/piperine-parity/tests/source_modules.rs` (and the elaboration file the gap needs, named in the commit)
**Depends on**: T27
**Reuses**: everything of T16, T20, T27
**Requirement**: AS-09
**Executor**: Sonnet

**Done when**:

- [ ] Test with exactly this name passes in `Run smodules`: `slew_bounds_the_rate_and_tracks_at_the_operating_point`
- [ ] Gate check passes: Full (`Run smodules`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): the stdlib slew runs`

---

### T29: Typing `inst.name`; `E0343` and `E0222`

**What**: `types/reach.rs`: `c.x` where `c` names a placement of a module without a result types as the `pub` item `x` of the placed module (a node, a carrier's type, a param's type, a `let`'s type); an item that is not `pub` is `E0343` naming it; a commit or a definition of a child's carrier is `E0222` naming it; a field of anything else keeps the `field access` refusal, so the Book's alarm listing does not move.
**Where**: `crates/piperine-lang/src/types/reach.rs` (with `expr.rs`, `definition.rs`)
**Depends on**: T1
**Reuses**: `Checker::placement_type`; the `E0222` checks of `types/module.rs`
**Requirement**: AS-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run typestruct`: `inst_name_of_an_item_that_is_not_pub_is_e0343`, `a_commit_to_a_childs_carrier_is_e0222`, `inst_name_types_as_the_pub_item`
- [ ] `Run clibook` passes unedited
- [ ] Gate check passes: Quick (`Run typestruct`, `Run clibook`), then `cargo test -p piperine-lang`
- [ ] Test count: the 3 tests named above, and no existing test removed

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): inst.name on the pub items of a child`

---

### T30: Reaching into a child: nodes, carriers, params and `let`s

**What**: `elab/reach.rs`: a reached `pub` node is a parent body node (`NodeRole::Reached`), a reached carrier a parent carrier (`CarrierRole::Reached`), each listed in `BodyData::reaches` and bound by placement after the child is placed (`bind_reached_node`, `bind_reached_net`); a `pub` param reads the value its placement gives and is marked structural; a `pub let` lowers in a reach frame of the child (§8.2.25); a probe and a contribution may name a reached node.
**Where**: `crates/piperine-lang/src/elab/reach.rs` (with `analog.rs`, `place.rs`, `carriers.rs`)
**Depends on**: T29, T6, T13
**Reuses**: `Frame` (a reach frame), `lower_given`
**Requirement**: AS-11
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-parity/tests/source_reach.rs` (`//!` header: `inst.name` and `inflow()` run from source on the Rust backend)
- [ ] Tests with exactly these names pass in `Run sreach`: `the_parent_probes_a_pub_node_of_its_child`, `the_parent_loads_a_pub_node_of_its_child`, `the_parent_reads_a_pub_carrier_of_its_child`, `the_parent_reads_a_pub_param_and_a_pub_let_of_its_child`
- [ ] Gate check passes: Full (`Run sreach`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): reach a child's pub nodes, carriers, params and lets`

---

### T31: `p.inflow()`: the ammeter; `E0243` and `E0410`

**What**: the checker types `p.inflow()` on a port of a nature with `flow` as `f64`, `E0243` otherwise, `E0410` on the left of `+>`; `elab/inflow.rs` gives a module that reads `p.inflow()` an inside node `p.inflow`, moves every branch end and child connection written on `p` to it, adds the zero-potential branch `(p, p.inflow)` (never merged: its flow is read) and lowers the read as its `Expr::Flow`; `BodyData::inflows` records it (design "inflow()").
**Where**: `crates/piperine-lang/src/elab/inflow.rs` (with `types/call.rs`, `elab/analog.rs`, `elab/children.rs`)
**Depends on**: T16, T19
**Reuses**: `lower_flow_read`
**Requirement**: AS-13
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run sreach`: `inflow_sums_the_flows_of_the_modules_branches_at_the_port`, `inflow_includes_a_childs_branch_on_the_port`, `inflow_is_differentiated_in_ac`
- [ ] Tests with exactly these names pass in `Run typestruct`: `inflow_of_a_node_that_is_not_a_port_is_e0243`, `inflow_on_the_left_of_a_contribution_is_e0410`
- [ ] Gate check passes: Full (`Run sreach`, `Run typestruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 5 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): p.inflow() through an ammeter branch`

---

### T32: A body `let` as a param value

**What**: `ConstEval` evaluates a body `let` by its value expression (a cycle guard names the `let`), so a placement's param value that reads a body `let` over params is a `ChildValue::Expr` (recomputed by `host-ii`'s derive plan on a `set`); a `let` that reads a carrier, a probe, the environment or a result is `E0321` naming the `let` (§8.1.19).
**Where**: `crates/piperine-lang/src/elab/consts.rs` (with `children.rs`, `diagnose.rs`)
**Depends on**: T1
**Reuses**: `ConstFault::Dynamic`, `lower_given`
**Requirement**: AS-12
**Executor**: Sonnet

**Done when**:

- [ ] Creates the target `crates/piperine-parity/tests/source_params.rs` (`//!` header: param values, initial values and param history run from source on the Rust backend)
- [ ] Test with exactly this name passes in `Run sparams`: `a_body_let_as_a_param_value_sets_the_corner`
- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_param_value_let_may_read_another_let`, `a_param_value_let_that_reads_a_probe_is_e0321`
- [ ] Test with exactly this name passes in `Run apibind`: `a_set_of_r_recomputes_the_capacitance_it_derives`
- [ ] Gate check passes: Full (`Run sparams`, `Run elabstruct`, `Run apibind`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): a body let as a param value`

---

### T33: `changed()` and `prev()` on a param

**What**: the checker types `r.changed()` on a param as an `Event` and `r.prev()` as the param's type; the elaborator lowers them to `EventSource::ParamChanged(p)` and `Expr::ParamPrevious(p)`; a binding before the run fires nothing (if T1's fact 1 or 2 does not hold, the one solver function that writes a param pin's `CHANGED` or `PREVIOUS` is the cross-lane touch of this task, reported first).
**Where**: `crates/piperine-lang/src/elab/events.rs` (with `types/call.rs`, `types/event.rs`, `elab/lower.rs`)
**Depends on**: T7, T13, T22
**Reuses**: the `Changed` event and `Previous` read of msl-digital
**Requirement**: AS-14
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run apibind`: `a_timed_set_fires_changed_once_at_its_time`, `prev_of_a_param_reads_its_own_value_then_the_value_before_a_set`, `a_binding_before_the_run_fires_no_changed`
- [ ] Gate check passes: Full (`Run apibind`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 3 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): changed and prev of a param`

---

### T34: An initial value that reads a param

**What**: the checker accepts a node's or a carrier's initial value over params and constants (`E0210` for a carrier or a probe, as today); the variant of T18 folds it per instance into the body's init (`BodyNode::init`, `Carrier::init`) and the instance's net, so `signal s: Bit<f64> = k0;` holds `k0` from `(0, 0)` and `--param Top.k0=4.0` gives 4.0 through T22.
**Where**: `crates/piperine-lang/src/elab/body.rs` (with `carriers.rs`, `types/module.rs`, `types/carrier.rs`)
**Depends on**: T18, T22
**Reuses**: `KeyParams` (initial class)
**Requirement**: AS-15
**Executor**: Sonnet

**Done when**:

- [ ] Test with exactly this name passes in `Run apibind`: `an_initial_value_over_a_param_follows_its_binding`
- [ ] Test with exactly this name passes in `Run sparams`: `a_node_initial_value_over_a_param_holds_the_operating_point`
- [ ] Test with exactly this name passes in `Run typestruct`: `an_initial_value_that_reads_a_carrier_is_e0210`
- [ ] Gate check passes: Full (`Run apibind`, `Run sparams`, `Run typestruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 3 tests named above, and no existing test removed

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): an initial value that reads a param`

---

### T35: The authored child binds its own signals with `Own`

**What**: `lower_child` binds each own signal slot of the authored `Child` to `Terminal::Own(k)`, `k` the child body's node index, and the `SPEC_DEVIATION` note at `children.rs:112` goes (S4, AS-16).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T2
**Reuses**: T2's `Terminal::Own`
**Requirement**: AS-16
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run elabstruct`: `a_childs_own_signal_is_bound_to_own_in_the_authored_child`, `a_design_with_own_terminals_passes_check`
- [ ] Gate check passes: Full (`Run elabstruct`, `cargo test -p piperine-lang`, `Run twins`, `Run tgolden`)
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `fix(elab): the authored child names its own signals (S4)`

---

### T36: The records: `GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`

**What**: `GAPS.md` drops L26, L27, L34 to L38, L42, L50 and S4, keeps one L40 line for the three spec models of AS-05 (the stdlib `idt` and `Water` after `generics`, the §18 `Diode` after `params-and-io`), drops `slew` from L58, updates §1, the counts, §9 and the "Analog structure" cluster, and adds any gap the tasks found; `ROADMAP.md` marks queue item 7 delivered; `.specs/STATE.md` gets the project decision of `design.md` "Tech Decisions" with the next free MD number; `crates/piperine-cli/tests/docs.rs` gains the record tests and its `### S4.` assertion is rewritten (S4 closed), said in the commit.
**Where**: `GAPS.md` (with `ROADMAP.md`, `.specs/STATE.md`, `crates/piperine-cli/tests/docs.rs`)
**Depends on**: T28, T35
**Reuses**: `gaps_drops_what_msl_fixes_closed` of `docs.rs` as the pattern
**Requirement**: AS-18, AS-05
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `Run docs`: `gaps_drops_what_analog_structure_closed`, `gaps_keeps_l40_for_the_spec_models_that_wait`, `the_roadmap_and_state_record_analog_structure`
- [ ] `Run clibook`, `Run twins` and `Run tgolden` pass unedited
- [ ] Gate check passes: Build
- [ ] Test count: the 3 tests named above, and one rewritten assertion of `docs.rs`

**Tests**: integration
**Gate**: build

**Commit**: `docs: analog-structure closes L26, L27, L34-L38, L42, L50 and S4`

---

## Checks

**C1** - A flow contribution under `if v(a) > 0.5`, driven by 1 mA, gives `v(a)` = 1.0 V at the operating point within `DC_VOLTAGE`. (AS-01, AC 1; T16)
Proof: run sswitch | test a_flow_contribution_under_an_if_on_a_probe_gives_one_volt ... ok

**C2** - A `match` on a `Bit<Mode>` carrier adds, in each solve, only the contribution of the arm the settled value selects. (AS-01, AC 2; T16)
Proof: run sswitch | test a_match_on_a_carrier_adds_only_the_selected_arm ... ok

**C3** - A contribution in a nested plain block lowers as at the block's top level. (AS-01, AC 3; T16)
Proof: run elabstruct | test a_contribution_in_a_nested_block_lowers_as_at_the_top ... ok

**C4** - A branch with no enabled contribution has zero flow: 1000 V with `en` low, 0.999 V with `en` high, within `DC_VOLTAGE`. (AS-01, AC 4; T16)
Proof: run sswitch | test an_untaken_contribution_leaves_the_branch_with_zero_flow ... ok

**C5** - `if ctl.high() { bound_step(1n); }` caps at 1 ns every step whose accepted solution executed it, and no other. (AS-01, AC 5; T14, T21)
Proof: run sswitch | test a_bound_step_under_an_if_caps_only_the_steps_that_executed_it ... ok
Proof: run watch | test a_cap_of_infinity_caps_nothing ... ok

**C6** - A contribution in a closure is still refused at parse. (AS-01, AC 6; T16)
Proof: run elabstruct | test a_contribution_in_a_closure_is_refused_at_parse ... ok

**C7** - A selector of a potential-capable branch that reads a probe is `E0414` naming the branch and the probe. (AS-02, AC 1; T17)
Proof: run elabstruct | test a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414 ... ok

**C8** - A selector through a value input is judged per placement: bound to a probe it is `E0414`, bound to a param it is not. (AS-02, AC 2; T27)
Proof: run elabstruct | test a_selector_through_a_value_input_is_judged_per_placement ... ok

**C9** - Selectors over params only fix the branch's kind at elaboration and mark those params structural. (AS-02, AC 3; T18)
Proof: run elabstruct | test params_only_selectors_fix_the_kind_and_mark_the_params_structural ... ok

**C10** - A fixed zero-potential branch with no flow probe merges its nodes: one node fewer than with `v(a, b) +> 1.0`. (AS-02, AC 4; T19)
Proof: run elabstruct | test a_fixed_zero_potential_branch_merges_its_nodes ... ok
Proof: run parop | test diode_rs_with_rs_zero_merges_and_matches_rdiode ... ok

**C11** - Params-only selectors that enable both a potential and a flow contribution are `E0415` naming the branch. (AS-02, AC 5; T19)
Proof: run elabstruct | test params_only_selectors_enabling_both_kinds_are_e0415 ... ok

**C12** - A selector on a discrete carrier or the environment keeps the branch's flow unknown in every solve. (AS-02, AC 6; T16)
Proof: run elabstruct | test a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown ... ok

**C13** - `Switch` behind 1 kΩ from 1 V: `v(a)` = 1 V before 1 µs and 0 V after, the branch flow 0 before and 1 mA after, within `TRAN_VOLTAGE` and `DC_CURRENT`. (AS-03, AC 1; T16)
Proof: run sswitch | test the_switch_shorts_at_one_microsecond ... ok

**C14** - A switch changes kind only between solves: the solve at the instant uses the old kind, the next solve the new one. (AS-03, AC 2; T16)
Proof: run sswitch | test a_switch_changes_kind_at_the_step_after_the_instant ... ok

**C15** - A solve that enables both kinds on one branch fails the run with `E0418` naming the instance and the branch. (AS-03, AC 3; T11, T16)
Proof: run sswitch | test both_kinds_enabled_in_one_solve_fail_with_e0418 ... ok
Proof: run tstruct | test both_kinds_enabled_return_e0418_naming_the_branch ... ok

**C16** - An AC sweep linearizes a switch branch in the kind its selector gives with `analysis(Static)` false. (AS-03, AC 4; T16)
Proof: run sswitch | test an_ac_sweep_takes_the_switch_kind_with_static_false ... ok

**C17** - `uic_rc.msl` with `CapIc` agrees with `tests/ngspice/uic_rc.cir` under `TRAN_VOLTAGE` at every print row. (AS-03, AC 5; T23)
Proof: run partran | test uic_rc ... ok

**C18** - `CapIc` in AC behind 1 kΩ from 1 V gives `|v(out)|` = 1/√2 at 100 kHz within `AC_MAGNITUDE`. (AS-03, AC 6; T23)
Proof: run sswitch | test capic_has_the_admittance_of_a_capacitor_in_ac ... ok

**C19** - `Bounded` (`hi` = 1.0, ±1 kV): `pos(xs)` = 0.5 at 0.5 s, 1.0 from 1 s to 2 s, 0.5 at 2.5 s, within `TRAN_VOLTAGE`. (AS-04, AC 1; T20)
Proof: run sderiv | test bounded_holds_its_state_at_the_bound ... ok

**C20** - The local `Integ` with `x` = 1, `ic` = 0.5 and `reset` true until 1 ms gives 0.5, then `0.5 + (t − 1 ms)`, within `TRAN_VOLTAGE`. (AS-04, AC 2; T27)
Proof: run smodules | test integ_holds_ic_while_reset_then_integrates ... ok

**C21** - `Integ` with `ic` in AC gives `|result|` = 1/(2πf) within `AC_MAGNITUDE`. (AS-04, AC 3; T27)
Proof: run smodules | test integ_is_one_over_j_omega_in_ac ... ok

**C22** - `Integ` with `ic` holds its result at `ic` at an operating point. (AS-04, AC 4; T27)
Proof: run smodules | test integ_holds_its_result_at_ic_at_the_operating_point ... ok

**C23** - WHERE `generics` has merged the stdlib `idt` runs; until then it is refused by name as a generic module. (AS-05, AC 1; T27, Q2)
Proof: run elabstruct | test the_stdlib_idt_waits_for_generics ... ok

**C24** - WHERE `generics` has merged `Water` runs; until then it is refused by name as a generic module. (AS-05, AC 2; T27, Q2)
Proof: run elabstruct | test the_water_model_waits_for_generics ... ok

**C25** - WHERE `params-and-io` has merged the §18 `Diode` with `rs: Some(100.0)` runs `diode_rs`; until then it is refused by name and the twin runs the local `DiodeRs`. (AS-05, AC 3; T24, Q2)
Proof: run elabstruct | test the_section_18_diode_waits_for_params_and_io ... ok
Proof: run parop | test diode_rs ... ok

**C26** - WHERE `params-and-io` has merged the §18 `Diode` with `rs` left out merges `a` and `ai`; until then `GAPS.md` keeps one L40 line naming the three spec models and their dependencies. (AS-05, AC 4; T36, Q2)
Proof: run docs | test gaps_keeps_l40_for_the_spec_models_that_wait ... ok

**C27** - A `limit` under a branch not taken neither vetoes convergence nor moves its last value. (AS-06, AC 1; T21)
Proof: run sswitch | test a_limit_under_an_untaken_branch_neither_vetoes_nor_moves ... ok

**C28** - The `devices.msl` diode junction enabled by `en.high()` after the operating point converges to the `rdiode` solution within `DC_VOLTAGE`. (AS-06, AC 2; T21)
Proof: run sswitch | test a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution ... ok

**C29** - A `limit` whose value input reads a carrier through `known_or` is accepted and limits the sum. (AS-06, AC 3; T21)
Proof: run sswitch | test a_limit_reads_a_carrier_through_known_or ... ok

**C30** - `if en.high() { i(a, gnd) +> ac_stim(1m); }` with 1 kΩ gives `|v(a)|` = 1 V with `en` high and 0 V with `en` low. (AS-06, AC 4; T21)
Proof: run sswitch | test an_ac_stim_under_an_if_drives_only_when_enabled ... ok

**C31** - `ac_stim(mag.known_or(0.0))` on a carrier settled at 2m gives the phasor 2m. (AS-06, AC 5; T21)
Proof: run sswitch | test an_ac_stim_reads_its_magnitude_from_a_carrier ... ok

**C32** - The capacitor written `2.0 * ddt(0.5 * c * v(p, n))` agrees with ngspice on `tran_rc` under `TRAN_VOLTAGE` and on `ac_rc` under `AC_MAGNITUDE` and `AC_PHASE`. (AS-07, AC 1; T20)
Proof: run partran | test tran_rc_with_a_scaled_derivative ... ok
Proof: run parac | test ac_rc_with_a_scaled_derivative ... ok

**C33** - `k * v(c) * ddt(v(p, n))` gives `k · v(c) · dv/dt` in a transient and `k · v(c) · jω · v(p, n)` in AC, within their contracts. (AS-07, AC 2; T20)
Proof: run sderiv | test a_derivative_times_a_probe_is_k_v_dv_dt ... ok

**C34** - A body `let` bound to `ddt(x)` and read by `analog` as an addend gives the same rows as the inline form. (AS-07, AC 3; T20)
Proof: run elabstruct | test a_body_let_of_ddt_read_as_an_addend_gives_the_inline_rows ... ok

**C35** - An event reading a `ddt` result records 0 at equilibrium and the step's derivative after it: 1000 within 1e-6 relative on `v(a)` = 1 kV/s · t. (AS-07, AC 4; T20)
Proof: run sderiv | test an_event_reads_the_derivative_of_the_step ... ok

**C36** - `ddt(x, abstol)` hands `abstol` to the solver as its term's state tolerance, and an RC with `ddt(q, 1e-3)` takes fewer accepted steps than with `ddt(q, 1e-18)`. (AS-07, AC 5; T9, T20)
Proof: run plan | test a_derivative_abstol_names_its_pin ... ok
Proof: run sderiv | test a_looser_derivative_abstol_takes_fewer_steps ... ok

**C37** - `G(out, 2.0 * v(a))` with `v(a)` = 1 V gives `v(out)` = 2.0 V at the operating point. (AS-08, AC 1; T27)
Proof: run smodules | test a_value_input_of_a_module_without_a_result_drives_its_contribution ... ok

**C38** - That circuit in AC with 1 V at `a` gives `|v(out)|` = 2.0 within `AC_MAGNITUDE`. (AS-08, AC 2; T27)
Proof: run smodules | test a_value_input_is_differentiated_in_ac ... ok

**C39** - A value input reading a caller's `Bit<f64>` through `known_or` gives the callee the new value from the carrier's change at 1 µs. (AS-08, AC 3; T27)
Proof: run smodules | test a_value_input_reads_the_callers_carrier_from_its_change ... ok

**C40** - A continuous argument read by a callee's definition is `E0336` at the call. (AS-08, AC 4; T26)
Proof: run typestruct | test a_continuous_argument_read_by_a_callee_definition_is_e0336 ... ok

**C41** - A value input read in a callee's reaction is the caller's expression at the reaction's instant. (AS-08, AC 5; T27)
Proof: run smodules | test a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant ... ok

**C42** - `v(out) +> Lag(v(in));` on a 0 to 1 V step gives `1 − e^(−t/τ)` within `TRAN_VOLTAGE`. (AS-09, AC 1; T27)
Proof: run smodules | test lag_follows_one_minus_exp ... ok

**C43** - `Lag` in AC gives `|v(out)/v(in)|` = 1/√(1 + (2πfτ)²) within `AC_MAGNITUDE`. (AS-09, AC 2; T27)
Proof: run smodules | test lag_is_a_first_order_low_pass_in_ac ... ok

**C44** - Two placements with equal arguments are two instances, each with its own node. (AS-09, AC 3; T27)
Proof: run elabstruct | test two_placements_with_equal_arguments_are_two_instances ... ok

**C45** - A `Lag` under an untaken branch keeps integrating: `v(out)` = 1 − e^(−2) just after `en` rises at 2τ, within `TRAN_VOLTAGE`. (AS-09, AC 4; T27)
Proof: run smodules | test a_lag_under_an_untaken_branch_keeps_integrating ... ok

**C46** - `let l = Lag(v(in));` read by `analog` and `on above(l - 0.5)` is one instance, and the reaction fires at τ·ln 2 within the default `ttol`. (AS-09, AC 5; T27)
Proof: run smodules | test a_named_lag_is_one_instance_for_analog_and_its_event ... ok

**C47** - The stdlib `slew(v(in), 1M, 1M)` gives 0 V before 1 µs, a 1 V/µs ramp, 1 V from 2 µs, and `v(in)` at an operating point. (AS-09, AC 6; T28)
Proof: run smodules | test slew_bounds_the_rate_and_tracks_at_the_operating_point ... ok

**C48** - `let y = Integ(-y / 1m, 1.0, None);` gives 1.0 at the operating point and e^(−t/1 ms) after, within `TRAN_VOLTAGE`. (AS-10, AC 1; T27)
Proof: run smodules | test feedback_through_integ_decays_exponentially ... ok

**C49** - A cycle with a direct dependence at every result is `E0353` naming the cycle's names. (AS-10, AC 2; T25)
Proof: run typestruct | test a_cycle_with_a_direct_dependence_at_every_result_is_e0353 ... ok

**C50** - A cycle through an unknown of a module is not `P0001`. (AS-10, AC 3; T25)
Proof: run typestruct | test feedback_through_an_unknown_of_a_module_is_accepted ... ok

**C51** - `v(c.mid)` gives the potential of the child's node `mid`. (AS-11, AC 1; T30)
Proof: run sreach | test the_parent_probes_a_pub_node_of_its_child ... ok

**C52** - `i(c.mid, gnd) +> v(c.mid) / 1k;` adds the parent's own branch loading the child's node. (AS-11, AC 2; T30)
Proof: run sreach | test the_parent_loads_a_pub_node_of_its_child ... ok

**C53** - `c.state` of a child's `pub signal state: Bit<u8>` reads the child's value in a reaction, a definition and, through `known_or`, in `analog`. (AS-11, AC 3; T30)
Proof: run sreach | test the_parent_reads_a_pub_carrier_of_its_child ... ok

**C54** - `c.r` and `c.e` read the child's `pub` param and `pub let`, the `let` evaluated in the reader's context. (AS-11, AC 4; T30)
Proof: run sreach | test the_parent_reads_a_pub_param_and_a_pub_let_of_its_child ... ok

**C55** - `inst.name` on an item that is not `pub` is `E0343` naming it. (AS-11, AC 5; T29)
Proof: run typestruct | test inst_name_of_an_item_that_is_not_pub_is_e0343 ... ok

**C56** - A commit to or a definition of a child's carrier is `E0222` naming it. (AS-11, AC 6; T29)
Proof: run typestruct | test a_commit_to_a_childs_carrier_is_e0222 ... ok

**C57** - `let c = 1.0 / (2.0 * PI * r * fc); Capacitor(b, gnd) { c }` puts the corner at `fc`: `|v(b)|` = 1/√2 within `AC_MAGNITUDE`. (AS-12, AC 1; T32)
Proof: run sparams | test a_body_let_as_a_param_value_sets_the_corner ... ok

**C58** - A param-value `let` may read another body `let` over params and constants. (AS-12, AC 2; T32)
Proof: run elabstruct | test a_param_value_let_may_read_another_let ... ok

**C59** - A param-value `let` that reads a carrier, a probe, the environment or a result is `E0321` naming the `let`. (AS-12, AC 3; T32)
Proof: run elabstruct | test a_param_value_let_that_reads_a_probe_is_e0321 ... ok

**C60** - WHERE `host-ii` X9 has merged, a host `set` of `r` recomputes the child's `c`. (AS-12, AC 4; T32)
Proof: run apibind | test a_set_of_r_recomputes_the_capacitance_it_derives ... ok

**C61** - `v(m) +> p.inflow() * 1k;` gives `1k · (1 mA + v(p, n)/1 kΩ)`. (AS-13, AC 1; T31)
Proof: run sreach | test inflow_sums_the_flows_of_the_modules_branches_at_the_port ... ok

**C62** - `p.inflow()` includes a child's branch ending on `p` (G.59). (AS-13, AC 2; T31)
Proof: run sreach | test inflow_includes_a_childs_branch_on_the_port ... ok

**C63** - `p.inflow()` is differentiated in AC like any probe. (AS-13, AC 3; T31)
Proof: run sreach | test inflow_is_differentiated_in_ac ... ok

**C64** - `inflow()` on something that is not a port of a nature with `flow` is `E0243`. (AS-13, AC 4; T31)
Proof: run typestruct | test inflow_of_a_node_that_is_not_a_port_is_e0243 ... ok

**C65** - `p.inflow()` on the left of `+>` is `E0410`. (AS-13, AC 5; T31)
Proof: run typestruct | test inflow_on_the_left_of_a_contribution_is_e0410 ... ok

**C66** - WHERE `host-ii` HII-13 has merged, a host `set` of `r` at 1 ms fires `on r.changed()` once, at 1 ms. (AS-14, AC 1; T33)
Proof: run apibind | test a_timed_set_fires_changed_once_at_its_time ... ok

**C67** - `r.prev()` gives the param's own value before any `set`, and the value before the last `set` after one. (AS-14, AC 2; T33)
Proof: run apibind | test prev_of_a_param_reads_its_own_value_then_the_value_before_a_set ... ok

**C68** - A param bound before the run fires no `changed()`. (AS-14, AC 3; T33)
Proof: run apibind | test a_binding_before_the_run_fires_no_changed ... ok

**C69** - `signal s: Bit<f64> = k0;` gives 2.5 from `(0, 0)` with `k0` = 2.5 and 4.0 with `--param Top.k0=4.0`. (AS-15, AC 1; T34)
Proof: run apibind | test an_initial_value_over_a_param_follows_its_binding ... ok

**C70** - `signal a: Electrical = v0;` holds `a` at `v0` through a transient's operating point and starts Newton from it in an `op`. (AS-15, AC 2; T34)
Proof: run sparams | test a_node_initial_value_over_a_param_holds_the_operating_point ... ok

**C71** - An initial value that reads a carrier or a probe is `E0210`. (AS-15, AC 3; T34)
Proof: run typestruct | test an_initial_value_that_reads_a_carrier_is_e0210 ... ok

**C72** - The authored `Child` binds each own signal slot to `Own`, never to ground. (AS-16, AC 1; T35)
Proof: run elabstruct | test a_childs_own_signal_is_bound_to_own_in_the_authored_child ... ok

**C73** - An IR with an `Own` terminal prints it and round-trips to an equal IR. (AS-16, AC 2; T2)
Proof: run irstruct | test an_own_terminal_prints_and_round_trips ... ok

**C74** - A design whose children have own signals passes `Ir::check`. (AS-16, AC 3; T35)
Proof: run elabstruct | test a_design_with_own_terminals_passes_check ... ok

**C75** - The interpreter refuses each form this feature adds, naming it, and runs none. (AS-17, AC 1; T15)
Proof: run frozen | test the_interpreter_refuses_each_analog_structure_form_by_name ... ok

**C76** - Each of the 7 twins prints, on each backend, the bytes it printed before this feature; the generated goldens are unchanged. (AS-18, AC 1; every task)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
Proof: run twins | test the_two_backends_print_the_same_bytes ... ok
Proof: run tgolden | test the_five_m1_bodies_print_as_the_golden_files ... ok

**C77** - `parity_op`, `parity_dc`, `parity_ac`, `parity_tran` and `parity_corpus` pass with `contract.rs` unedited. (AS-18, AC 2; every task)
Proof: run partran | test tran_rc ... ok
Proof: run parac | test ac_rc ... ok
Proof: run parop | test divider ... ok
Proof: run pardc | test diode_iv ... ok
Proof: run corpus | test every_twin_has_a_cir_and_a_test ... ok
Proof: `git diff --exit-code main -- crates/piperine-parity/src/contract.rs`

**C78** - With the ngspice comparison disabled the new twins run and print the harness's skip line. (AS-18, AC 3; T23, T24)
Proof: run partran | test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
Proof: run parop | test diode_rs_runs_and_prints_the_skip_line_without_ngspice ... ok

**C79** - `GAPS.md` no longer lists L26, L27, L34 to L38, L42, L50 and S4, lists L40 only for the AS-05 models, and the Book's listings equal the binary's output. (AS-18, AC 4; T36)
Proof: run docs | test gaps_drops_what_analog_structure_closed ... ok
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (B1, IR phase; implementation choices the design left open) `Terminal::Own(k)` sits at slot `k` of a `Child::nodes` and names a `Signal` node of the callee; it prints `own:<name>` in the tree and `own<k>` in the arenas. `Body::contributions()` is `contributions.rs` (`Contribution`, `Condition { expr, holds }`); each shared expression or statement is walked once. A contribution under a non-`Constant` condition makes its branch non-affine; the condition's sources join the branch's `Flow`/`Charge` targets. `Ir::check` treats every `Switch` branch as driven (a flow probe reads its private flow). `Site::enabled` is required exactly when a `limit` is read under a condition (`ctx.conditional`); an `ac_stim` needs none. `Target::Limit(k)` also reads `enabled`, `Target::BoundStep(k)` also reads `when`. `Body::param_changes(target)` lists the params whose `ParamChanged` event `target` reads through `Expr::Fired` (the IR query, sorted); `param_previous_reads(target)` lists `Expr::ParamPrevious`. `Reach::child` indexes `instance.children` in the same order as `BodyData::children`; `Ir::check` requires a reached node or net to equal the child instance's. `IrBuilder::bind_reached_net` replaces a net `bind_nets` already put at that slot. `NodeRole::Reached`/`CarrierRole::Reached` bodies get a placeholder (`Ground`, a fresh net) from `place.rs` until T30. Plan, printer and interpreter arms for the new forms fail loud (`panic!` in `piperine-plan` and `print/analog.rs`, `Error::Unsupported` in the value printer) until T8 to T15 give them meaning: no elaborator emits a new form yet. Cross-lane note for T30: `piperine-api/src/metadata.rs:85` lists every non-`Signal` carrier as a port, so a `Reached` carrier would read as `out`.

**Contract (T1):** `contract.md`. Anchors moved by 1-2 lines only; `Frame` is in `carriers.rs`, `no_locals` in `compound.rs`/`drives.rs`. host-ii added `piperine-api/src/params.rs` (`Params`, `Derive`), `piperine-plan/src/derive.rs` (`DerivePlan`), `Context.nodeset`; solver-hygiene V1 (`TermPlan::abstol`) is merged. **Facts 1 and 2 do not hold**: a param pin's `init` and a before-run binding are committed at `(0, 0)` from a zeroed store, so `CHANGED` reads true at `(0, 0)` and `PREVIOUS` reads 0.0 until a later write; T33 must guard the start microstep (reported to the orchestrator before phase 5). Fact 8: the `limit`/`ac_stim` carrier refusal is in `piperine-transpile/src/print/analog.rs:583`, asserted by `tests/reject.rs`; T21 rewrites it.
**Dispatched:** B1 - Sonnet worker, 2026-11-18 (host-ii merged 1a3d69dc). B1 done (871ab0ea, 2317/0). B2 (phase 2, T8-T15) - Sonnet worker, 2026-11-18.
**Walked:** not applicable: no screen; every story's Independent Test is a run from source or a test target named in `## Checks`.

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1  T2  T3  T4  T5  T6  T7
Phase 2:  T8  T9  T10  T11  T12  T13  T14  T15
Phase 3:  T16  T17  T18  T19  T20  T21  T22
Phase 4:  T23  T24  T25  T26  T27  T28
Phase 5:  T29  T30  T31  T32  T33  T34  T35
Phase 6:  T36
```

Tasks run in the order listed; the dependencies are the diagrams of the Execution Plan. Execution is strictly sequential. Batches: phase 1 (7 tasks), phase 2 (8), phase 3 (7), phase 4 (6), phases 5 and 6 together (8).

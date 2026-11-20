# Composite Values Tasks (phases A and B)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/composite-values/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run irctl**: `cargo test -p piperine-ir --test control`
**Run iragg**: `cargo test -p piperine-ir --test aggregates`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run rt**: `cargo test -p piperine-transpile --lib rt::`
**Run tctl**: `cargo test -p piperine-transpile --test control`
**Run tagg**: `cargo test -p piperine-transpile --test aggregates`
**Run typesctl**: `cargo test -p piperine-lang --test types_control`
**Run typesagg**: `cargo test -p piperine-lang --test types_aggregates`
**Run elabctl**: `cargo test -p piperine-lang --test elab_control`
**Run elabagg**: `cargo test -p piperine-lang --test elab_aggregates`
**Run srcctl**: `cargo test -p piperine-parity --test source_control`
**Run srcagg**: `cargo test -p piperine-parity --test source_aggregates`
**Run plancomp**: `cargo test -p piperine-plan --test composite_shape`
**Run apicomp**: `cargo test -p piperine-api --test composites`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** Q1 (a), Q2 (c), Q3 (a), Q4 (integers (a), `f16` (a), `f128` (b)) and Q5 (b) (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"). Phases A and B use Q1, Q2 and Q5; Q3 and Q4 belong to phases C and D. The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D20; every task follows them as written.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-c CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-c` before any cargo command. Cargo with `-p <crate>`, never `--workspace`, never `cargo clean`. Commands run in the foreground; no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. Every buck target passes after every task that touches the printer, the runtime, the plan or the elaborator's lowering of an existing form.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same expression evaluated by Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`).** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; no abbreviated name; no god method and no needless abstraction; the data path evident. Every new type, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design.md "Components" names, never into `print/value.rs` or `types/expr.rs` beyond a dispatch arm.
- **Conventions.** New IR `Expr`/`Stmt`/`ValueType` variants are appended last; new IR fields carry `#[serde(default)]`. The interpreter refuses every new IR form by name (R1) in the same task that adds the form, and `crates/piperine-interp/tests/frozen.rs::each_composite_values_form_is_refused_by_name` grows with it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level tests put their models in `crates/piperine-parity/tests/control/*.msl` and `tests/aggregates/*.msl`.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-67 (Appendix H as IR rows).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| IR (`piperine-ir`) | integration | each new node and type built, typed, checked (valid and refused), printed, its dependencies | `crates/piperine-ir/tests/{control,aggregates}.rs` | `cargo test -p piperine-ir --test control` / `--test aggregates` |
| Interpreter refusals (`piperine-interp`) | integration | each new form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Runtime (`piperine-transpile/src/rt`) | unit | all branches; the bounds of Q5 and Appendix C.2 at and past their limit | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-transpile --lib rt::` |
| Printer (`piperine-transpile/src/print`) | integration | each form compiled by `rustc` from hand-built IR and run; generated text names its path | `crates/piperine-transpile/tests/{control,aggregates}.rs` | `cargo test -p piperine-transpile --test control` / `--test aggregates` |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each error code with its span and the name it gives | `crates/piperine-lang/tests/{types_control,types_aggregates}.rs` | `cargo test -p piperine-lang --test types_control` / `--test types_aggregates` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC; each error code; const contexts | `crates/piperine-lang/tests/{elab_control,elab_aggregates}.rs` | `cargo test -p piperine-lang --test elab_control` / `--test elab_aggregates` |
| Source-level runs (end to end) | integration | one test per run-time AC, from MSL source on the Rust backend, expected values computed by Rust in the test | `crates/piperine-parity/tests/{source_control,source_aggregates}.rs`, models in `tests/control/`, `tests/aggregates/` | `cargo test -p piperine-parity --test source_control` / `--test source_aggregates` |
| Plan (`piperine-plan`) | integration | each composite layout against §2 and abi.md P4 | `crates/piperine-plan/tests/composite_shape.rs` | `cargo test -p piperine-plan --test composite_shape` |
| Host library (`piperine-api`) | integration | decode, render, leaf-path set, enum names | `crates/piperine-api/tests/composites.rs` | `cargo test -p piperine-api --test composites` |
| Command line (`piperine-cli`) | integration | each output format with a composite | `crates/piperine-cli/tests/run.rs` | `cargo test -p piperine-cli --test run` |
| Spec text and repository records | integration | one assertion per decided fact or removed `GAPS.md` row | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, `crates/piperine-lang/tests/std_galleries.rs` | `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test std_galleries` |
| Contract re-check (T1) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the runtime, the printer, the plan, the host or the lowering of an existing form | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Contract

The tree after the rebase onto `main` with `analog-structure` merged.

```
T1
```

### Phase 2: IR control flow (phase A)

```
T2 → T3 → T4 → T5
```

### Phase 3: Runtime and printer for control flow

```
T6 → T7 → T8 → T9 → T10
```

### Phase 4: The checker for control flow, patterns and functions

```
T11 → T12 → T13 → T14 → T15 → T16 → T17 → T18
```

### Phase 5: The elaborator for control flow, patterns and functions

```
T19 → T20 → T21 → T22 → T23 → T24
```

### Phase 6: Analog unrolling and const contexts

```
T25 → T26 → T27
```

### Phase 7: Phase A closure

```
T28 → T29
```

### Phase 8: IR aggregates (phase B)

```
T30 → T31 → T32 → T33
```

### Phase 9: Runtime and printer for aggregates

```
T34 → T35 → T36 → T37
```

### Phase 10: The checker for aggregate types and expressions

```
T38 → T39 → T40 → T41 → T42 → T43
```

### Phase 11: The checker for derives, the library, `Repr` and composite positions

```
T44 → T45 → T46 → T47
```

### Phase 12: The elaborator for aggregate values

```
T48 → T49 → T50 → T51
```

### Phase 13: The elaborator for derives, iterators, `normal` and const aggregates

```
T52 → T53 → T54 → T55
```

### Phase 14: Carried composites, params, records and the host

```
T56 → T57 → T58 → T59 → T60 → T61 → T62
```

### Phase 15: Phase B closure

```
T63
```

---

## Task Breakdown

### T1: Rebase and re-read the anchors

**What**: Rebase `feature/composite-values` onto the current `main` once `analog-structure` has merged there (ROADMAP item 7; if it has not, stop and report). Re-read every anchor of design.md "What the code does today", "Components" and "Cross-lane touches" at the new `HEAD`: `Unsupported`, `Checker::pattern`/`exhaustive`, `tuple_let`, `enum_type`, `closure`, `EnumLayout`, `arms_chain`, `Known`, `Variants`, `ConstEval`, `Scope`, `elab/structural.rs` `Frame` and the hoisting names, `contribute.rs`, `ValuePrinter::merged`/`holds_effect`/`block_lines`, `rt::Merge`/`Commits::branch`/`run`, `Carried::to_levels`, `CarrierLayout`/`RecordLayout`, `deps.rs`'s recursion refusal, `Ir::check`'s recursion check, `Metadata::of`, `Cell`, the next free `MD-` number. Write every difference, and how each later task maps onto it, on the Handoff's `**Settled mid-build:**` line. No code change.
**Where**: `.specs/features/composite-values/tasks.md` (Handoff only)
**Depends on**: None
**Reuses**: design.md
**Requirement**: CV-35
**Executor**: Sonnet

**Done when**:

- [x] The rebase is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test` at the root pass
- [x] The Handoff names each anchor that moved and the task it affects

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): composite-values - contract re-read after the rebase`

---

### T2: IR mutable locals and assignment

**What**: `Local::mutable` (`#[serde(default)]`); `Stmt::Assign { place: Place, op: Option<BinaryOp>, value }` appended; `Place { local, path: Vec<Projection> }` with `Projection::Field(u32)` only (index projections come with T31). `typing.rs`, `print.rs` (`x = e`, `x += e`), `statements.rs` (an assignment is not an ABI statement), `deps.rs` (the local's abstract value joins the assigned one), `build.rs`; `Ir::check` refuses an assignment to a local that is not `mutable` and an `op` that does not apply to the type. The interpreter refuses `Assign` by name. Create `crates/piperine-ir/tests/control.rs` with its `//!` header.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Stmt`, `BinaryOp`, `Checker` of `check.rs`, `Walk` of `deps.rs`
**Requirement**: CV-01, CV-20
**Executor**: Sonnet

**Done when**:

- [x] `an_assignment_to_a_mutable_local_checks_and_prints` and `an_assignment_to_an_immutable_local_is_refused` pass under `**Run irctl**`
- [x] `each_composite_values_form_is_refused_by_name` (new in `frozen.rs`) passes under `**Run interp**` for `Assign`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): mutable locals and assignment`

---

### T3: IR loops, `break` and `continue`

**What**: `Expr::Loop { body }`, `Expr::While { cond, body }`, `Expr::ForRange { local, low, high, inclusive, reverse, body }`, `Expr::Break(Option<ExprId>)`, `Expr::Continue`, appended. A loop's type is its breaks' value type (`()` for `While`/`ForRange` and a value-less `break`). `Ir::check`: `break`/`continue` only inside a loop, `break value` only in `Loop`, `ForRange` bounds of one integer type. `statements.rs` walks a loop body once (§12.1.14: a commit in a loop is one statement). `deps.rs` iterates a loop body to a fixed point of its abstract sources. Interpreter refusals by name.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T2
**Reuses**: `Statements::of`, `Walk`
**Requirement**: CV-02, CV-20
**Executor**: Sonnet

**Done when**:

- [x] `loops_check_type_and_print`, `break_outside_a_loop_is_refused`, `a_commit_in_a_loop_is_one_statement` and `a_loop_reaching_a_probe_on_a_later_iteration_reads_it` pass under `**Run irctl**`
- [x] `each_composite_values_form_is_refused_by_name` covers the five forms under `**Run interp**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): loops, break and continue`

---

### T4: IR `return`

**What**: `Expr::Return(Option<ExprId>)`, appended: allowed only in a function body (`Ir::check` refuses it in a body's code); its value has the function's result type; it fits any expected type. `deps.rs` joins a return value into the function's result. Interpreter refusal.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T3
**Reuses**: `FunctionData::ret`
**Requirement**: CV-03
**Executor**: Sonnet

**Done when**:

- [x] `a_return_checks_against_the_result_type` and `a_return_outside_a_function_is_refused` pass under `**Run irctl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): return from a function`

---

### T5: IR function values and recursion

**What**: `Expr::FunctionRef(FunctionId)` (type `ValueType::Fn`) and `Expr::CallValue { callee, args }`, appended. `Ir::check` stops refusing recursion (V5) except for a function reached from an `analog` body (§3 rule 12: the elaborator unrolls those, T26), which stays refused naming the function. `deps.rs` replaces the recursion refusal with a fixed point over the function's abstract result. Interpreter refusals. The `check.rs` test "function calls itself" moves (design "may move").
**Where**: `crates/piperine-ir/src/check.rs`
**Depends on**: T4
**Reuses**: `Checker::recursion`, `Walk::returns`
**Requirement**: CV-06, CV-07
**Executor**: Sonnet

**Done when**:

- [x] `a_function_value_is_called_through_its_reference`, `recursion_is_accepted_outside_analog` and `recursion_reached_from_analog_is_refused` pass under `**Run irctl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): function values and recursion`

---

### T6: `rt::Trips` and `rt::Depth`

**What**: In `rt/control.rs`: `Trips::new(origin)` and `Trips::tick(&mut self)`, which raises `rt::Panic` naming the loop's origin on the iteration past 2^24 (Q5 (b)); `Depth::root()`, `Depth::deeper(self, function: &str) -> Depth`, which raises `rt::Panic` naming the function past a depth of 1 024. Unit tests at and past each bound.
**Where**: `crates/piperine-transpile/src/rt/control.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `rt::Panic`, `rt::Attempt`
**Requirement**: CV-07
**Executor**: Sonnet

**Done when**:

- [x] `rt::control::tests::trips_panic_past_two_to_the_twenty_four` and `rt::control::tests::depth_panics_past_one_thousand_twenty_four` pass under `**Run rt**`
- [x] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(rt): loop and call-depth bounds`

---

### T7: `rt::Unrolled` and the merge of written-local tuples

**What**: `Merge` for tuples of arities 1 to 12 in `rt/flag.rs` (one impl each, field by field). In `rt/control.rs`: `Step<S, B, R>` (`Next(S)`, `Exit(S, B)`, `Returned(R)`, and the union of two steps from an unknown decision), `Unrolled::run(state, body)`: a loop under unknown control as its unrolling, merged exit by exit; at 64 iterations (Appendix C.2) a still-running path is cut and its state becomes `Merge::unknown()`. Unit tests: a known loop gives Rust's result; an unknown exit at iteration 3 merges both exits; a never-known exit is cut at 64.
**Where**: `crates/piperine-transpile/src/rt/control.rs`
**Depends on**: T6
**Reuses**: `Merge`, `Decision`, `Attempt`
**Requirement**: CV-08
**Executor**: Sonnet

**Done when**:

- [x] `rt::control::tests::unrolled_merges_exit_by_exit` and `rt::control::tests::unrolled_cuts_at_sixty_four` and `rt::flag::tests::tuples_merge_field_by_field` pass under `**Run rt**`
- [x] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(rt): loops under unknown control, exit by exit`

---

### T8: Printer: mutation, loops and exits under known control

**What**: `print/control.rs` (`impl ValuePrinter`): `let mut`, `Stmt::Assign` (plain and compound), `Loop`/`While`/`ForRange` with one `rt::Trips` per loop, `Break`, `Continue`, `Return`. `value.rs` gains only the dispatch arms. Create `crates/piperine-transpile/tests/control.rs`: hand-built IR functions compiled by `rustc` and run, each compared with the same Rust written in the test.
**Where**: `crates/piperine-transpile/src/print/control.rs`
**Depends on**: T7
**Reuses**: `ValuePrinter::block_lines`, `Text::ident`
**Requirement**: CV-01, CV-02, CV-03
**Executor**: Sonnet

**Done when**:

- [ ] `known_control_prints_plain_rust_and_runs` passes under `**Run tctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): mutation, loops and exits under known control`

---

### T9: Printer: mutation and loops under unknown control

**What**: A merged `if` (`choose`, `commits_.branch`) whose arms write mutable locals returns the value and the written locals as a tuple and rebinds them (design "Control flow"); a loop whose condition, a guard of its body, a `break`, `continue` or `return` decision is `Flagged` prints through `rt::Unrolled` in pure code and, in a reaction, runs the rest of the loop after its first unknown decision on a non-definite `commits_` path. The written-local set comes from a walk of the IR (`Ir::written_locals`, `piperine-ir/src/statements.rs` sibling). Tests on hand-built IR.
**Where**: `crates/piperine-transpile/src/print/control.rs`
**Depends on**: T8
**Reuses**: `ValuePrinter::merged`, `holds_effect`, `Commits::branch`, `Commits::run`
**Requirement**: CV-08
**Executor**: Sonnet

**Done when**:

- [ ] `unknown_decisions_merge_written_locals_and_loop_exits` and `a_reaction_loop_committing_after_an_unknown_decision_commits_maybe` pass under `**Run tctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): mutation and loops under unknown control merge`

---

### T10: Printer: function values and recursion depth

**What**: `FunctionRef` prints the function's path (as a `fn` pointer; a generic `<R>` function is instantiated at the use), `CallValue` a call through it, a closure prints `move |..|` (§1.5.14, capture by value); a function on a call cycle takes a trailing `depth_: rt::Depth` and each of its calls passes `depth_.deeper("<name>")`; a non-recursive function keeps its text.
**Where**: `crates/piperine-transpile/src/print/control.rs`
**Depends on**: T9
**Reuses**: `ValuePrinter::function`, the environment-parameter mechanism of `value.rs`
**Requirement**: CV-05, CV-06, CV-07
**Executor**: Sonnet

**Done when**:

- [ ] `function_values_and_recursion_compile_and_run` and `a_recursion_past_the_depth_bound_faults_by_name` pass under `**Run tctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): function values, move closures and recursion depth`

---

### T11: Checker: places, `let mut` and assignment

**What**: `types/assign.rs` (`impl Checker`): `let mut x`, `x = e`, every compound operator of level 15 (§1.5.12), the place rules (a local declared `mut`, a field or element of a place); assignment to a param, a carrier, a `const`, a function parameter, an immutable local or a body `let` is `E0183` naming it; `let x;`/`let x: T;` with definite assignment before every read (§1.7.5, `E0183`). The `` `let mut` binding `` and `assignment` rows leave the refused set. Create `crates/piperine-lang/tests/types_control.rs`; pin `E0331` for `let mut` in a module body.
**Where**: `crates/piperine-lang/src/types/assign.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::let_statement`, `binary`, `poisoned`
**Requirement**: CV-01
**Executor**: Sonnet

**Done when**:

- [ ] `let_mut_and_every_compound_operator_type_check`, `assigning_a_non_place_is_e0183_naming_the_target`, `a_read_before_assignment_is_e0183` and `let_mut_in_a_module_body_is_e0331` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): let mut, places and assignment`

---

### T12: Checker: loops

**What**: `types/loops.rs` (`impl Checker`): `while c`, `while let p = e`, `loop` (its type the type of its `break` values, §1.8.8), `for p in e` over an integer range (arrays and iterators come with T45), `break`, `break value` (in `loop` only), `continue`; `E0185` for `break`/`continue` outside a loop and `break value` outside `loop`; `break`/`continue` do not cross a closure or a function (§1.8.12). A refutable pattern in `for` is `E0184`.
**Where**: `crates/piperine-lang/src/types/loops.rs`
**Depends on**: T11
**Reuses**: `Checker::condition`, `statements`
**Requirement**: CV-02
**Executor**: Sonnet

**Done when**:

- [ ] `every_loop_form_type_checks` and `break_or_continue_outside_a_loop_is_e0185` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): while, while let, loop, for, break and continue`

---

### T13: Checker: `return` and `?`

**What**: `types/exits.rs` (`impl Checker`): `return`/`return e` against the innermost function's or closure's result type; `e?` on an `Option` in a function or closure whose result is an `Option` (§1.5.15); `E0185` for `return` outside a function and a closure, and for `?` in one whose result is not an `Option`; `return`, `break`, `continue` fit any expected type (§1.5.16).
**Where**: `crates/piperine-lang/src/types/exits.rs`
**Depends on**: T12
**Reuses**: `Checker::owner`, `context`
**Requirement**: CV-03
**Executor**: Sonnet

**Done when**:

- [ ] `return_and_question_mark_type_check` and `return_outside_a_function_is_e0185` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): return and the question mark`

---

### T14: Checker: guards, range patterns, or-bindings and `@`

**What**: In `types/control.rs`: a `match` arm guard (`bool`, the pattern's names in scope, §1.8.3; a guarded arm does not count for exhaustiveness, §1.8.6); range patterns `lo..=hi`, `lo..hi`, `lo..`, `..=hi` over integers and floats with literal or `const` bounds (§1.6.4; an enum bound is `P0002`, D6; an empty range is `E0184`); or-patterns that bind names, each alternative binding the same names with the same types (`E0184` otherwise); `name @ p`; a name bound twice is `E0184`. Integer exhaustiveness by interval cover over the whole range of any width up to 64 (V2), floats needing `_` or a binding.
**Where**: `crates/piperine-lang/src/types/control.rs`
**Depends on**: T13
**Reuses**: `Covers`, `Checker::exhaustive`, `literal_covers`
**Requirement**: CV-04
**Executor**: Sonnet

**Done when**:

- [ ] `guards_ranges_or_bindings_and_at_type_check`, `alternatives_binding_differently_are_e0184` and `an_integer_match_covered_by_ranges_is_exhaustive` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): match guards, range patterns, or-bindings and at`

---

### T15: Checker: refutable patterns and `E0184`

**What**: `if let`, `while let` and `let … else` accept any refutable pattern of the types the checker knows (literal, range, tuple, `Some`/`None`, enum variant; aggregates join with T40 and T41); the `else` block of `let … else` must not complete (`E0185`, §1.7.6); irrefutability (§1.6.11): a refutable pattern in `let` without `else`, in `for` or in a closure parameter is `E0184`; a non-exhaustive `match` is `E0184` naming one missing pattern instead of today's `P0002` (V1; the old assertion moves, design "may move").
**Where**: `crates/piperine-lang/src/types/control.rs`
**Depends on**: T14
**Reuses**: `Checker::condition`, `pattern`
**Requirement**: CV-04
**Executor**: Sonnet

**Done when**:

- [ ] `refutable_patterns_type_check_in_if_let_while_let_and_let_else` and `a_non_exhaustive_match_or_refutable_let_is_e0184` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): refutable patterns, and E0184 for a non-exhaustive match`

---

### T16: Checker: closures anywhere

**What**: In `types/call.rs`: a closure bound by `let`, called, returned in a block, passed to an H row, `limit` or a `Fn`-typed parameter; parameter types from annotations, the expected `Fn` type, or the first call in the same body (D17; never called with unannotated parameters is `E0181`); `Ty::Closure` carries its captures and its function class (§3 rule 3); a call of it where its class is not allowed is `E0122`; a closure containing a form of §3 rule 6 inside a `fn` is the parser's `E0121` (pinned). The "closure outside an argument…" refusal leaves.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T15
**Reuses**: `Checker::closure_typed`, the function-class inference of `call.rs`
**Requirement**: CV-05
**Executor**: Sonnet

**Done when**:

- [ ] `closures_type_check_where_rust_allows_them`, `a_closure_called_outside_its_class_is_e0122` and `a_closure_with_an_excluded_form_in_a_fn_is_e0121` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): closures anywhere, with their function class`

---

### T17: Checker: function values

**What**: In `types/call.rs`: a `fn` item's name as a value has the type `fn(T..) -> U` (§1.2.10); a parameter of function-pointer type; a closure that captures nothing coerces to a function pointer, one that captures does not (as Rust); a function value or closure as a carried type is `E0112` (§1 rule 9). The "this name as a value" refusal leaves.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T16
**Reuses**: `Checker::function_value`, `Ty::Fn`
**Requirement**: CV-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_function_name_is_a_function_value`, `a_non_capturing_closure_is_a_function_pointer`, `a_capturing_closure_is_not_a_function_pointer` and `a_function_value_is_not_carried_e0112` pass under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): function values and function-pointer types`

---

### T18: Checker: recursion

**What**: In `types/call.rs`: a function that calls itself, directly or through others, type-checks (§3 rule 12); its function class is the fixed point of its body's classes. The "a function that calls itself" refusal leaves; whether a recursion reached from `analog` unrolls is the elaborator's (T26).
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T17
**Reuses**: the checker's worklist (`queued`), the class inference
**Requirement**: CV-07
**Executor**: Sonnet

**Done when**:

- [ ] `direct_and_mutual_recursion_type_check` passes under `**Run typesctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): recursion`

---

### T19: Elaborator: `let mut` and assignment

**What**: `elab/control.rs` (`impl Elaboration`): `let mut` lowers to a `mutable` local, assignments and compound assignments to `Stmt::Assign`; `let x;` to a local first assigned later. In `known.rs` a mutable local is flagged when its initializer or any assignment is (a loop to its fixed point). Create `crates/piperine-lang/tests/elab_control.rs` and `crates/piperine-parity/tests/source_control.rs` with their `//!` headers; the source test runs a function, a reaction and an `analog` block that use every compound operator and compares each with Rust.
**Where**: `crates/piperine-lang/src/elab/control.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Scope::locals`, `Elaboration::lower_let`, `Known`
**Requirement**: CV-01
**Executor**: Sonnet

**Done when**:

- [ ] `let_mut_lowers_to_mutable_locals_and_assignments` passes under `**Run elabctl**`
- [ ] `let_mut_and_every_compound_operator_give_rusts_values` passes under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): let mut and assignment`

---

### T20: Elaborator: loops

**What**: In `elab/control.rs`: `while`, `while let`, `loop` with `break value`, `continue`, `for` over an integer range (`ForRange`, `reverse` for `(a..b).rev()`) in functions, reactions and `analog` blocks with no contribution or placement inside (any trip count, §11 rule 12). A commit inside a reaction loop is one statement run several times (§12.1.14). Source tests: every loop form against Rust; a reaction committing in a loop, the last write winning per level (§12.1.2); an analog loop computing a current whose partials equal the written-out expression's; an empty range runs zero times, in `analog` too.
**Where**: `crates/piperine-lang/src/elab/control.rs`
**Depends on**: T19
**Reuses**: `reactions.rs`, `analog.rs` statement lowering
**Requirement**: CV-02
**Executor**: Sonnet

**Done when**:

- [ ] `every_loop_form_gives_rusts_values`, `a_commit_in_a_reaction_loop_is_one_statement_last_write_winning`, `an_analog_loop_without_contribution_is_differentiated_through` and `an_empty_range_runs_zero_times_in_analog_too` pass under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): while, loop and for over ranges`

---

### T21: Elaborator: `return` and `?`

**What**: In `elab/control.rs`: `return e` and `return` in a function body (from inside a loop, an `if`, a `match`), lowered to `Expr::Return`; `e?` on the `Option` pair lowers to `if !e.is { return None-pair } else { e.val }` for a function whose result is the pair (MD-67 item 2).
**Where**: `crates/piperine-lang/src/elab/control.rs`
**Depends on**: T20
**Reuses**: the `Option` pair helpers of `compound.rs`
**Requirement**: CV-03
**Executor**: Sonnet

**Done when**:

- [ ] `an_early_return_gives_the_caller_its_value` and `the_question_mark_on_none_returns_none` pass under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): return and the question mark`

---

### T22: Elaborator: pattern tests

**What**: `elab/patterns.rs` (`PatternTest`): one pattern as a boolean test over the scrutinee and its bindings: literals, ranges, or-patterns with bindings, `@`, `Some`/`None`, guards (`compound.rs`'s arm chain tests the guard after the pattern, the pattern's names bound); `if let`, `while let` and `let … else` with any refutable pattern. Aggregate patterns join in T49 and T50.
**Where**: `crates/piperine-lang/src/elab/patterns.rs`
**Depends on**: T21
**Reuses**: `arms_chain`, `pattern_test`, `option_test`
**Requirement**: CV-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_match_guard_takes_the_arm_only_when_true`, `range_patterns_match_as_rust`, `or_patterns_and_at_bindings_bind_as_rust` and `refutable_patterns_run_in_if_let_while_let_and_let_else` pass under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): guards, ranges, or-bindings and refutable patterns`

---

### T23: Elaborator: closures and function values

**What**: In `elab/control.rs`: a closure anywhere lowers to `Expr::Closure` with its captures copied at its definition (§3 rule 3); a call of a closure local to `CallValue`; a `fn` name as a value to `FunctionRef` (the variant of the all-known flag pattern; a flagged argument at a call through a value selects the flagged variant by the value's static target when known, else is `P0001` naming the call); in `analog` a closure call is differentiated as a direct call (§3 rule 1).
**Where**: `crates/piperine-lang/src/elab/control.rs`
**Depends on**: T22
**Reuses**: `closure_parts`, `Variants`
**Requirement**: CV-05, CV-06
**Executor**: Sonnet

**Done when**:

- [ ] `closures_run_and_capture_by_value`, `a_closure_in_analog_is_differentiated_through` and `a_function_value_calls_the_named_function` pass under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): closures and function values`

---

### T24: Elaborator: recursion

**What**: In `elab/variants.rs`: a variant is registered before its body is lowered, so a recursive call (direct or mutual) finds it; its result type is the declared result, flagged when the fixed point of its body says so. Source tests: a recursion from a reaction and a function runs (`fib`, mutual `even`/`odd`) against Rust; a runaway recursion and a runaway `loop` fail the analysis naming the function or loop, the instance and the instant, and the process survives (Q5 (b)).
**Where**: `crates/piperine-lang/src/elab/variants.rs`
**Depends on**: T23
**Reuses**: `Variants`, `lower_function`
**Requirement**: CV-07
**Executor**: Sonnet

**Done when**:

- [ ] `recursion_from_a_reaction_runs` and `runaway_loops_and_recursions_fail_the_analysis_by_name` pass under `**Run srcctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): recursion`

---

### T25: Elaborator: analog loops that contribute or place are unrolled

**What**: `elab/unroll.rs` (`Unrolling`): in `analog`, a loop whose body holds a contribution or a placement has its trip count folded by `ConstEval` and is expanded, each iteration the body lowered with the loop variable bound to its constant; each placement is a distinct instance named by the hoisting rule in unrolled order (D16, `Frame` of `elab/structural.rs`); a trip count that does not fold is `E0416` (contribution) or `E0351` (placement). Re-read `structural.rs`/`contribute.rs` at `HEAD` first and note any difference on the Handoff.
**Where**: `crates/piperine-lang/src/elab/unroll.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ConstEval`, `Frame`, `contribute.rs`
**Requirement**: CV-02
**Executor**: Sonnet

**Done when**:

- [ ] `an_analog_loop_that_contributes_or_places_is_unrolled` and `a_contributing_analog_loop_without_a_constant_trip_count_is_e0416` pass under `**Run elabctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): unroll analog loops that contribute or place`

---

### T26: Elaborator: recursion reached from `analog` is unrolled

**What**: In `elab/unroll.rs`: a call from `analog` (directly or through functions) of a function on a call cycle is inlined, each recursive call's controlling decisions folded by `ConstEval` over params and constants, until the recursion ends; a decision that does not fold is `E0126` naming the function. The IR then holds no recursive function reached from `analog` (T5's check holds).
**Where**: `crates/piperine-lang/src/elab/unroll.rs`
**Depends on**: T25
**Reuses**: the inlining of `records.rs`, `ConstEval`
**Requirement**: CV-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_recursion_reached_from_analog_is_unrolled` and `an_unfoldable_recursion_from_analog_is_e0126` pass under `**Run elabctl**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): unroll a recursion reached from analog`

---

### T27: `ConstEval` over phase A forms

**What**: In `elab/consts.rs`: an environment of locals (mutable included), assignments, `while`/`loop`/`for` over ranges, `break`/`continue`/`return`, the patterns of T22, closures and recursion, so a `const`, a `const fn`, a param default, a `where` and a generation range evaluate them with their run-time value (CV-36); the Q5 bounds (2^24 iterations, depth 1 024) and every panic are `E0104` naming the expression (D7).
**Where**: `crates/piperine-lang/src/elab/consts.rs`
**Depends on**: T26
**Reuses**: `ConstEval`, `Folded`, `ConstFault`, `diagnose.rs`
**Requirement**: CV-36
**Executor**: Sonnet

**Done when**:

- [ ] `phase_a_forms_fold_in_const_contexts` and `a_panic_in_a_const_evaluation_names_the_expression` pass under `**Run elabctl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): loops, mutation, exits and recursion in const contexts`

---

### T28: The Q5 bounds in the spec

**What**: `docs/spec/part_i_values.md` §1 rule 11 gains the Q5 sentence: one loop running more than 2^24 iterations in one evaluation, or a call depth over 1 024, is a panic; Appendix C.2.1's defaults table lists both. `spec_text.rs` guards each wording (MD-57).
**Where**: `docs/spec/part_i_values.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs`
**Requirement**: CV-07
**Executor**: Sonnet

**Done when**:

- [ ] `section_1_rule_11_states_the_loop_and_depth_bounds` passes under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): the loop and recursion bounds of section 1 rule 11`

---

### T29: Phase A bookkeeping

**What**: `GAPS.md` drops L5, L6, L9 (but for the `generics` row: a capturing closure to a user `fn`), L10, and US-04 AC 11 of L5's note; §9 Coverage updates I §1–§2 and I §3–§3.3; a gallery fence or Book listing of these forms that now checks is held to the checker in `std_galleries.rs` or `book.rs` (CV-37 AC 2); `docs.rs` asserts the rows left and the rows gone.
**Where**: `GAPS.md`
**Depends on**: T28
**Reuses**: `docs.rs`, `std_galleries.rs`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_composite_values_phase_a_closed` passes under `**Run docs**`
- [ ] `the_fences_composite_values_lifted_are_checked` passes under `**Run galleries**` for the phase A fences
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: composite-values phase A closes L5, L6, L9 and L10`

---

### T30: IR aggregate types and the type arena

**What**: `ValueType::{Tuple, Array, Slice, Struct(TypeId), Enum(TypeId), Option}` appended; `TypeId`, `TypeData`, `TypeKind`, `VariantData`, derives; `Ir::types` arena with `IrBuilder::type_data` and `Ir::type_data`; `Ir::check` (a type that contains itself, an array length, an enum's tag levels against its discriminants); the printer (`struct P { x: f64 }`, `[f64; 3]`, `(u8, bool)`, `Option<..>`). Create `crates/piperine-ir/tests/aggregates.rs`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ids.rs` id pattern, `build.rs`
**Requirement**: CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `the_type_arena_builds_checks_and_prints` passes under `**Run iragg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): aggregate value types and the type arena`

---

### T31: IR aggregate expressions and places

**What**: `Expr::{Tuple, Array, ArrayRepeat, StructOf, VariantOf, Field, Index, SliceOf, Length}` appended; `Projection::Index(ExprId)`; typing, `Ir::check` against the arena (field counts and types, index of an integer type, a slice only as a call argument or a method receiver), `print.rs`, `deps.rs`, `statements.rs`; interpreter refusals.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T30
**Reuses**: T2's `Place`
**Requirement**: CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `ir_check_types_every_aggregate_expression` passes under `**Run iragg**`
- [ ] `each_composite_values_form_is_refused_by_name` covers the forms under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): aggregate expressions and indexed places`

---

### T32: IR variant tests, `Option` packing and defaults

**What**: `Expr::{IsVariant, VariantField, OptionPack, OptionPart, Default}` appended; `Binary` `Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` on aggregates allowed by the arena's derives (H.8) and refused otherwise by `Ir::check`; interpreter refusals.
**Where**: `crates/piperine-ir/src/check.rs`
**Depends on**: T31
**Reuses**: `LevelPart`, `TypeData` derives
**Requirement**: CV-11, CV-12, CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `variant_tests_option_packs_and_comparisons_check` passes under `**Run iragg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): variant tests, option packing and aggregate comparison`

---

### T33: IR composite constants

**What**: `Constant::Aggregate(AggregateId)` (`Constant` stays `Copy`), an `Ir` arena of composite constants (`IrBuilder::aggregate`, `Ir::aggregate`), its check against a `ValueType`, its print, serde. Every exhaustive `match` on `Constant` gains its arm.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T32
**Reuses**: the arena pattern of T30
**Requirement**: CV-18, CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `composite_constants_check_print_and_round_trip` passes under `**Run iragg**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `feat(ir): composite constants`

---

### T34: Printer: Rust items of the type arena

**What**: `print/types.rs` (`TypeItems`): one Rust item per arena entry (struct, tuple struct, unit struct, enum with its fields), with the derives its source allows (D11: generic over `R: Real` when it holds an `f64`), each with a doc comment naming its MSL path and origin (R3); `ValuePrinter::rust_type` prints aggregate types. Create `crates/piperine-transpile/tests/aggregates.rs`.
**Where**: `crates/piperine-transpile/src/print/types.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Text::ident`, `rust_type`
**Requirement**: CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `each_source_type_prints_as_one_rust_item_naming_its_path` passes under `**Run tagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): the type arena as Rust items`

---

### T35: Printer: aggregate expressions and indexed places

**What**: `print/aggregate.rs` (`impl ValuePrinter`): construction, field reads, `Index` through `rt::Index` (out of range a panic naming the index and the origin, §1.5.25; an unknown index merges the elements it may name, §3.3.13), `SliceOf`, `Length`, variant tests and fields as `match`, `OptionPack`/`OptionPart`, `Default`, aggregate comparisons, and assignment through field and index projections. `rt::Index` lives in `rt/control.rs` with its unit tests.
**Where**: `crates/piperine-transpile/src/print/aggregate.rs`
**Depends on**: T34
**Reuses**: `ValuePrinter::piece`, `lift`
**Requirement**: CV-13, CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `aggregate_expressions_compile_and_run` passes under `**Run tagg**`
- [ ] `rt::control::tests::an_index_out_of_range_panics_naming_it` and `rt::control::tests::an_unknown_index_merges_the_elements_it_may_name` pass under `**Run rt**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): aggregate expressions and indexed places`

---

### T36: `rt::LevelWords` and composite `Carried`

**What**: In `rt/flag.rs`: `LevelWords` (level bits of any width); `Carried::to_levels`/`from_levels` take it; `level`, `recast`, `same` and `merge_bytes` work through it; `Carried` and `Levelwise` for tuples (arities 1 to 12), arrays and `Option<T>` per §2 (tuple field `.0` lowest; `Option` tag level 0 then the payload). Scalars' generated text does not change.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T35
**Reuses**: `Carried`, `Levels`, `Word`
**Requirement**: CV-17, CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::composite_levels_follow_the_section_2_table` passes under `**Run rt**`
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): level words and Carried for tuples, arrays and Option`

---

### T37: Printer: `Carried` for derived structs and enums

**What**: In `print/types.rs`: `impl rt::Carried` for each arena type that derives `Repr`: bytes per the composite layout (T56's order: fields first-lowest), levels per §2 (enum: the tag lowest, then the widest payload padded with false levels; `decode` of a tag naming no variant reads unknown, §2 rule 9). Round-trip tests over every value of small types, padding ignored on decode.
**Where**: `crates/piperine-transpile/src/print/types.rs`
**Depends on**: T36
**Reuses**: `TypeItems`, `LevelWords`
**Requirement**: CV-17, CV-20
**Executor**: Sonnet

**Done when**:

- [ ] `every_composite_value_crosses_the_abi_unchanged` and `narrow_payloads_pad_with_false_levels` pass under `**Run tagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): Carried for derived structs and enums`

---

### T38: Checker: aggregate types

**What**: `types/aggregate.rs` (`impl Checker`): `Ty::{Struct, Array, Slice, Ref}` and full `Ty::Tuple`; `lower_type` of struct paths (named, tuple, unit), `(T, U)`, `[T; N]` with `N` folded by `ConstEval` (unknown is `E0151`), `&[T]`, `&T`; type equality by §1.4.8; a duplicate field or variant and a type containing itself are `E0186` (§1.3.2, §1.3.8). A generic struct stays `P0001` (`generics`). Create `crates/piperine-lang/tests/types_aggregates.rs`.
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::lower_type`, `ConstEval`
**Requirement**: CV-09, CV-10, CV-13
**Executor**: Sonnet

**Done when**:

- [ ] `every_struct_form_is_a_value_type` and `a_self_containing_type_is_e0186` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): struct, tuple, array, slice and reference types`

---

### T39: Checker: struct expressions and field access

**What**: In `types/aggregate.rs`: struct expressions (named fields in any order, shorthand), a tuple struct built by a call, a unit struct by its name, `s.x`, `t.0`, nested reads, a borrow read through (§1.2.6); a missing, duplicate or unknown field and an unknown field access are `E0186` naming the field; struct-update syntax `..base` stays a parse error (D3, pinned).
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T38
**Reuses**: `FieldInit`, `StructClass`
**Requirement**: CV-09
**Executor**: Sonnet

**Done when**:

- [ ] `struct_expressions_and_field_reads_type_check`, `a_bad_struct_expression_is_e0186_naming_the_field` and `struct_update_syntax_is_a_parse_error` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): struct expressions and field access`

---

### T40: Checker: tuples everywhere

**What**: In `types/aggregate.rs`: tuple expressions in any position, returned, passed, nested, `.0`; tuple patterns in `let`, `match`, `if let`, a closure parameter and `for` (`tuple_let` generalized, its "value is not a tuple expression" refusal leaves); a tuple of one field `(T,)`.
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T39
**Reuses**: `Checker::tuple_let`, `pattern`
**Requirement**: CV-10
**Executor**: Sonnet

**Done when**:

- [ ] `tuples_and_tuple_patterns_type_check` passes under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): tuples everywhere`

---

### T41: Checker: payload enums, `#[repr]` and discriminants

**What**: `types/enums.rs` (`impl Checker`): tuple and struct variants built by a call or a struct expression, matched with bindings, patterns joining the exhaustiveness of T14; `#[repr(uN)]` and explicit discriminants (§1.3.7: only on enums without fields, else `E0186`); a discriminant that does not fit `N` levels or repeats is `E0111`; a fieldless enum `as` an integer; a one-variant enum; `Option<T>` of an aggregate. The refusals "with a variant with a payload", "with a `#[repr]` attribute", "with an explicit discriminant" leave.
**Where**: `crates/piperine-lang/src/types/enums.rs`
**Depends on**: T40
**Reuses**: `Checker::enum_type`, `derives`
**Requirement**: CV-11
**Executor**: Sonnet

**Done when**:

- [ ] `payload_enums_build_and_match` and `a_discriminant_that_does_not_fit_or_repeats_is_e0111` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): payload enums, repr and discriminants`

---

### T42: Checker: arrays, indexing and slices

**What**: In `types/aggregate.rs`: `[a, b, c]` (one element type), `[x; N]` (`x` `Copy`, `N` a const expression; a non-constant length is `P0002` naming it, D6), `e[i]` with `i` any integer type, `e[a..b]`/`&a[..]` as a borrowed call argument or a method receiver (§1.5.9), element assignment in a `let mut`, nested arrays, `[T; N]` passed to `&[T]` (§1.2.5); slice patterns `[a, .., b]`, `rest @ ..` (§1.6.7).
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T41
**Reuses**: T38's array type
**Requirement**: CV-13
**Executor**: Sonnet

**Done when**:

- [ ] `arrays_indexing_and_slices_type_check` and `a_non_constant_array_length_is_refused_naming_it` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): arrays, indexing and slices`

---

### T43: Checker: borrows and dereference

**What**: In `types/aggregate.rs`: `&` of an array, slice or string for a function parameter, and of any value as a call argument (§1 rule 2); anywhere else `E0102`; `*r` of a `Copy` value; the four read-through places of §1.2.6 (field, index, method receiver, scalar operator operand).
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T42
**Reuses**: `Ty::Ref`
**Requirement**: CV-14
**Executor**: Sonnet

**Done when**:

- [ ] `borrows_in_call_arguments_are_accepted` and `a_borrow_elsewhere_is_e0102` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): borrows and dereference`

---

### T44: Checker: derives, comparison, defaults and moves

**What**: `types/derives.rs` (`Derives`): the H.8 traits a type derives, their requirements (H.8 rule 6), the traits of built-in types (rule 7); `==`/`!=` need `PartialEq`, `<`/`<=`/`>`/`>=` need `PartialOrd` (§1.3.10), refused naming the trait (`P0002`, D6); `x.clone()`; `T::default()` for a type deriving `Default`; `#[derive(Default)]` on an enum refused naming `Default` (D4); `Debug` and `Hash` accepted with no effect; an unknown derive name `E0161`; a read of a moved non-`Copy` value `E0183` (§1.4.7, D14).
**Where**: `crates/piperine-lang/src/types/derives.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::derives`
**Requirement**: CV-12
**Executor**: Sonnet

**Done when**:

- [ ] `derived_traits_give_comparison_clone_and_default`, `comparing_without_the_trait_is_refused_naming_it`, `debug_and_hash_derives_are_accepted`, `an_enum_deriving_default_is_refused` and `a_read_after_a_move_is_e0183` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): derives, comparison, defaults and moves`

---

### T45: Checker: Appendix H.5 to H.7

**What**: In `types/catalogue.rs`: the H.5 rows on arrays and slices (`len`, `is_empty`, `iter`, `contains`, `first`, `last`, `get`), H.6 on ranges (`contains`, `is_empty`), H.7 on iterators (`map`, `filter`, `fold`, `sum`, `count`, `zip`, `enumerate`, `rev` where rule 3 allows, `any`, `all`) with `Ty::Range` and `Ty::Iterator`; `for` over an array, a slice and any chain (§1.8.9); any other method on these receivers is `E0161` naming method and type.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T44
**Reuses**: the catalogue rows, `Checker::closure_typed`
**Requirement**: CV-15
**Executor**: Sonnet

**Done when**:

- [ ] `the_rows_of_h5_to_h7_type_check` and `a_method_outside_appendix_h_is_e0161` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): arrays, ranges and iterators of Appendix H`

---

### T46: Checker: `Repr` of composites and composite carriers

**What**: In `types/derives.rs`: `#[derive(Repr)]` on a struct or an enum (every field `Repr`, else `E0110` naming the field); `Ty::has_repr` and `levels` for tuples, arrays, `Option`, derived structs and payload enums per the §2 table (V4); `Bit<T>` for those `T` (a type without `Repr` is `E0112`); `T::WIDTH` of a composite.
**Where**: `crates/piperine-lang/src/types/derives.rs`
**Depends on**: T45
**Reuses**: `Ty::has_repr`, `Ty::levels`, `types/carrier.rs`
**Requirement**: CV-17
**Executor**: Sonnet

**Done when**:

- [ ] `composite_carriers_type_check`, `deriving_repr_with_a_field_without_repr_is_e0110` and `a_carried_type_without_repr_is_e0112` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): Repr of tuples, arrays, Option and derived types`

---

### T47: Checker: composite params and record fields

**What**: In `types/module.rs`: a module param of struct, tuple or array type, with or without a default and a `where`; a placement binding it; in `types/record.rs`, an `emit!` field of a composite type, and two sites giving one field two composite types `E0605`. The forms of phases C and D (an array of carriers, a carrier of `f32`, `u128`) stay `P0001` naming the form (CV-34 AC 2).
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T46
**Reuses**: `Records`, the param typing of `module.rs`
**Requirement**: CV-18, CV-19, CV-34
**Executor**: Sonnet

**Done when**:

- [ ] `composite_params_and_record_fields_type_check`, `two_sites_giving_a_field_two_composite_types_is_e0605` and `phase_c_and_d_forms_stay_p0001_by_name` pass under `**Run typesagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): composite params and record fields`

---

### T48: Elaborator: the type arena, every enum an IR enum

**What**: `elab/types.rs` (`TypeArena`): each reached struct and enum declaration becomes one `TypeData` (path, origin, fields, variants with discriminants and tag levels, derives); `ValueTypes::of` gives `Struct`/`Enum`/`Tuple`/`Array`/`Option`; unit-variant enums leave `EnumLayout`'s `UInt` tag (V3): their values are `VariantOf`, their `match` tests `IsVariant`, `as` and `cast` read the discriminant. The buck targets and the elab fixtures under "may move" are re-captured in this commit. Create `crates/piperine-lang/tests/elab_aggregates.rs`.
**Where**: `crates/piperine-lang/src/elab/types.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `EnumLayout`, `ValueTypes`, `intrinsics.rs`
**Requirement**: CV-11, CV-20, CV-22
**Executor**: Sonnet

**Done when**:

- [ ] `every_reached_struct_and_enum_is_one_arena_type` passes under `**Run elabagg**`
- [ ] Every buck target passes with unchanged assertions
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(elab): every enum is an IR enum type`

---

### T49: Elaborator: structs and tuples

**What**: `elab/aggregate.rs` (`impl Elaboration`): struct expressions (evaluated in written order, §1.3.11), tuple structs, unit structs, tuples, field and tuple-index reads, assignment through field projections, struct and tuple patterns (T22's `PatternTest`). Create `crates/piperine-parity/tests/source_aggregates.rs`.
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: T48
**Reuses**: `PatternTest`, `TypeArena`
**Requirement**: CV-09, CV-10
**Executor**: Sonnet

**Done when**:

- [ ] `struct_expressions_give_rusts_values`, `field_reads_give_the_fields`, `assigning_a_field_changes_that_field_only`, `tuples_give_rusts_values` and `tuple_patterns_bind_each_component` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): structs and tuples`

---

### T50: Elaborator: payload enums and `Option` of aggregates

**What**: In `elab/aggregate.rs`: variant construction, variant patterns with bindings (`IsVariant`, `VariantField`), discriminants and `#[repr]` as the tag, `as` of a fieldless enum giving its discriminant; an `Option` of an aggregate held as `OptionPack` where one value is needed and as the pair elsewhere (D8).
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: T49
**Reuses**: `option_test`, `PatternTest`
**Requirement**: CV-11
**Executor**: Sonnet

**Done when**:

- [ ] `payload_variants_build_and_match`, `discriminants_are_the_tag_and_the_cast` and `options_of_aggregates_behave_as_enums` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): payload enums and options of aggregates`

---

### T51: Elaborator: arrays, indexing and slices

**What**: In `elab/aggregate.rs`: array literals, `[x; N]` (evaluated once), indexing (out of range a run panic naming instance, instant and index), element assignment, nested arrays, `&a` to a `&[T]` parameter, slices `&a[i..j]` and `&a[..]` (a start after the end panics), slice patterns; an index that may be unknown reads the merge of the elements it may name and writes by merging into each (§3.3.13, unknown-semantics US-04 AC 12).
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: T50
**Reuses**: T35's `rt::Index` through the printer
**Requirement**: CV-13
**Executor**: Sonnet

**Done when**:

- [ ] `arrays_give_rusts_values`, `slices_give_the_elements_in_range`, `an_index_out_of_range_panics_at_run_time`, `a_slice_whose_start_exceeds_its_end_panics` and `an_unknown_index_reads_and_writes_by_merge` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): arrays, indexing and slices`

---

### T52: Elaborator: derives and borrows

**What**: In `elab/aggregate.rs`: derived `==`, `!=`, `<`, `<=`, `>`, `>=` on structs, tuples, arrays, enums (enums by discriminant then fields, H.8 rule 3) and `Option`s (`None` lowest, §1.5.23); `clone()`; `T::default()` (`Expr::Default`); element borrows from `iter`, `first`, `last`, `get` read through by `*`, comparison and arithmetic (§1.2.6).
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Expr::Binary` on aggregates (T32)
**Requirement**: CV-12, CV-14
**Executor**: Sonnet

**Done when**:

- [ ] `derived_comparisons_and_defaults_give_rusts_results` and `element_borrows_read_as_the_element` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): derived comparisons, defaults and borrows`

---

### T53: Elaborator: Appendix H.5 to H.7 as loops

**What**: `elab/iterators.rs` (`IteratorChain`): H.5 rows (`len` a constant for an array, `Length` for a slice), H.6 rows as comparisons, H.7 chains and `for` over arrays, slices and chains as one `ForRange` over indices (D10), in Rust's order; an empty array (`len` 0, `first` `None`); `get` past the end is `None`. The value code of the stdlib `zi_nd` reaction body (`acc` over `num`, `xs`, `den`, `ys`) runs as a monomorphic `fn` over arrays against Rust (D18).
**Where**: `crates/piperine-lang/src/elab/iterators.rs`
**Depends on**: T52
**Reuses**: `elab/control.rs` loops, `PatternTest`
**Requirement**: CV-15
**Executor**: Sonnet

**Done when**:

- [ ] `every_row_of_h5_to_h7_gives_rusts_result`, `for_visits_elements_in_rusts_order`, `an_empty_array_has_length_zero_and_no_first` and `the_zi_nd_reaction_value_code_runs_as_a_function` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): arrays, ranges and iterators of Appendix H`

---

### T54: `normal` and the jitter clock

**What**: The stdlib `normal` (`std/analog.msl`) elaborates and runs: tuples from `uniform`, `ln`, `cos`. Source tests: `normal(rng, mean, sigma)` bit for bit equal to §22.2.17's SplitMix64 and Box-Muller text compiled by Rust in the test; `sigma = 0.0` gives `mean`; the §22.2.17 jitter example run as a clock gives the same edge times on every run with one seed. `GAPS.md` L58's "`normal` (tuples, `cos`)" entry goes in T63.
**Where**: `crates/piperine-parity/tests/source_aggregates.rs`
**Depends on**: T53
**Reuses**: `std/analog.msl`, T49 tuples
**Requirement**: CV-16
**Executor**: Sonnet

**Done when**:

- [ ] `normal_matches_the_spec_text_bit_for_bit`, `normal_with_zero_sigma_returns_the_mean` and `the_jitter_clock_repeats_with_its_seed` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(std): normal runs, with the jitter clock`

---

### T55: `ConstEval` over aggregates

**What**: In `elab/consts.rs`: `ConstValue` (a `Folded` scalar or a composite), struct/tuple/array/enum values, field and index reads and assignments, H.5 to H.7 rows, slices and borrows, so the stdlib's `poly_q`, `den_q` and `conjugate_closed` fold (CV-36); an index out of range in a const context is `E0104` naming the expression; a folded composite becomes `Constant::Aggregate`.
**Where**: `crates/piperine-lang/src/elab/consts.rs`
**Depends on**: T54
**Reuses**: T27's environment, `IrBuilder::aggregate`
**Requirement**: CV-36, CV-13
**Executor**: Sonnet

**Done when**:

- [ ] `aggregates_fold_in_const_contexts` and `an_index_out_of_range_in_a_const_is_e0104` pass under `**Run elabagg**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): aggregates in const contexts`

---

### T56: Plan: composite layouts

**What**: `piperine-plan/src/composite.rs` (`CompositeLayout`): the ABI `Layout` of an IR aggregate type: leaves at their alignment, dotted names (`p.x`, `t.0`, `tag`, `Data.n`), arrays as `count`, one known byte per leaf (`HAS_KNOWN`); used by `CarrierLayout`, `RecordLayout` and param pins; a one-variant enum lays out its payload only (§2 rule 5); an empty struct and `()` have zero levels. Create `crates/piperine-plan/tests/composite_shape.rs`.
**Where**: `crates/piperine-plan/src/composite.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `CarrierLayout`, `RecordLayout`, `ShapeBuf`
**Requirement**: CV-17, CV-20, CV-11
**Executor**: Sonnet

**Done when**:

- [ ] `composite_layouts_list_their_leaves`, `two_pins_of_a_composite_net_have_equal_layouts`, `a_one_variant_enum_lays_out_its_payload_only` and `an_empty_struct_is_carried_with_zero_levels` pass under `**Run plancomp**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): composite layouts`

---

### T57: Carried composites end to end

**What**: In `elab/carriers.rs`: a carrier of a derived struct, a derived enum, a tuple, an array or an `Option` of one: initial value (`Constant::Aggregate`), definitions, commits, reads, `changed()`, `prev()` over the whole value; `T::WIDTH` and `level(k)` per §2. Source tests through the printer's `Carried` impls.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T56
**Reuses**: `Structure`, T37's `Carried`
**Requirement**: CV-17
**Executor**: Sonnet

**Done when**:

- [ ] `derived_repr_layout_and_width_follow_section_2`, `composite_carriers_initialize_define_commit_and_change`, `committing_an_equal_composite_fires_no_change` and `a_tag_naming_no_variant_reads_unknown` pass under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): carried composites`

---

### T58: Composite params

**What**: In `elab/body.rs`: a param of struct, tuple or array type with its folded default (`Constant::Aggregate`) and its `where`; a placement binding it (constant or over the parent's params); a read in every block; its param pin laid out by `CompositeLayout`.
**Where**: `crates/piperine-lang/src/elab/body.rs`
**Depends on**: T57
**Reuses**: `children.rs` (`ChildValue`), `Param`
**Requirement**: CV-18
**Executor**: Sonnet

**Done when**:

- [ ] `composite_params_bind_and_read_everywhere` passes under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): composite params`

---

### T59: Host: setting a composite param by leaf path

**What**: In `piperine-api/src/host.rs`: a `set` (and `--param`) addresses a composite param's leaf, `Top.f.num[2]=0.5`, `Top.p.x=1.0`, `Top.t.0=true` (Q2 (c), D13); the param's `where` is checked on the whole value after the write (`E0317`); a path naming no leaf is refused naming it. Create `crates/piperine-api/tests/composites.rs`.
**Where**: `crates/piperine-api/src/host.rs`
**Depends on**: T58
**Reuses**: `Params`, `Derive`
**Requirement**: CV-18
**Executor**: Sonnet

**Done when**:

- [ ] `a_host_sets_a_composite_param_by_leaf_and_checks_its_where` passes under `**Run apicomp**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): set a composite param by leaf path`

---

### T60: Records with composite fields

**What**: In `elab/records.rs`: an `emit!` field of a composite value type is a stream field of that `ValueType`, laid out by `CompositeLayout`; the stream's metadata (§19 rule 13) names the field's type as written.
**Where**: `crates/piperine-lang/src/elab/records.rs`
**Depends on**: T59
**Reuses**: `Stream`, `RecordLayout`
**Requirement**: CV-19
**Executor**: Sonnet

**Done when**:

- [ ] `a_record_carries_a_composite_field_and_its_type` passes under `**Run srcagg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): composite record fields`

---

### T61: Host: composite cells and enum names

**What**: `piperine-api/src/composite.rs` (`CompositeCell`): decode a composite pin's bytes with the IR arena into `Cell::Composite`; render it as MSL literal syntax and as dotted leaves (D13); a carried enum (unit or payload) gives its variant name, read from the IR's `TypeData` with no change to a net's bytes (CV-22); `Metadata` names composite types as written.
**Where**: `crates/piperine-api/src/composite.rs`
**Depends on**: T60
**Reuses**: `Cell`, `Streams`, `Metadata::of`
**Requirement**: CV-21, CV-22
**Executor**: Sonnet

**Done when**:

- [ ] `a_carried_enum_reads_as_its_variant_name` and `enum_names_live_in_the_ir_not_in_the_net_bytes` pass under `**Run apicomp**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): composite cells and enum variant names`

---

### T62: CLI: composites in every output format

**What**: In `piperine-cli/src/render/`: the text table, record lines and JSON Lines print a composite in literal syntax (JSON Lines nested, D13); CSV and the plot one column per dotted leaf, an enum a variant column plus its payload leaves; a printed path naming a leaf (`Top.st.n`, `Top.v[2]`) prints that leaf alone.
**Where**: `crates/piperine-cli/src/render/mod.rs`
**Depends on**: T61
**Reuses**: `CompositeCell`
**Requirement**: CV-21
**Executor**: Sonnet

**Done when**:

- [ ] `composites_print_in_each_output_format` and `a_leaf_path_prints_that_leaf_alone` pass under `**Run clirun**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(cli): composites in every output format`

---

### T63: Phase B bookkeeping

**What**: `GAPS.md` drops L1, L2, L3, L4 (all but generic enums, which stay with `generics` GN-06), the H.5 to H.7 rows of L16, `normal` in L16 and L58, and the §4.7 note "Enum variant names are not carried"; §9 Coverage updates I §1–§2, I §3–§3.3, VII §22 and H; the fences and Book listings these forms lift are held to the checker (CV-37 AC 2); `.specs/STATE.md` gains the next free `MD-` entry of design.md "Tech Decisions"; the ROADMAP lane row says phases A–B delivered.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `docs.rs`, `std_galleries.rs`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_composite_values_phase_b_closed` passes under `**Run docs**`
- [ ] `the_fences_composite_values_lifted_are_checked` passes under `**Run galleries**` for the phase B fences
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: composite-values phase B closes L1 to L4, L16 and the enum names`

---

## Checks

**C1** - `let mut` and every compound operator give Rust's values in a function, a reaction and an `analog` block. (CV-01, AC 1; T19)
Proof: run srcctl | test let_mut_and_every_compound_operator_give_rusts_values ... ok

**C2** - Assigning to a binding without `mut`, a param, a carrier or a body `let` is `E0183` naming the target. (CV-01, AC 2; T11)
Proof: run typesctl | test assigning_a_non_place_is_e0183_naming_the_target ... ok

**C3** - `let mut` in a module body is `E0331`. (CV-01, AC 3; T11)
Proof: run typesctl | test let_mut_in_a_module_body_is_e0331 ... ok

**C4** - `for`, `while`, `while let`, `loop` with `break`, `break value` and `continue` give Rust's values. (CV-02, AC 1; T20)
Proof: run srcctl | test every_loop_form_gives_rusts_values ... ok

**C5** - An `analog` loop of constant trip count that contributes or places is unrolled, each iteration's placement a distinct instance. (CV-02, AC 2; T25)
Proof: run elabctl | test an_analog_loop_that_contributes_or_places_is_unrolled ... ok

**C6** - An `analog` loop holding a contribution with a non-constant trip count is `E0416`. (CV-02, AC 3; T25)
Proof: run elabctl | test a_contributing_analog_loop_without_a_constant_trip_count_is_e0416 ... ok

**C7** - An `analog` loop with no contribution accepts any trip count and its partials equal the written-out expression's. (CV-02, AC 4; T20)
Proof: run srcctl | test an_analog_loop_without_contribution_is_differentiated_through ... ok

**C8** - A commit in a reaction loop is one statement run several times, the last write winning per level. (CV-02, AC 5; T20)
Proof: run srcctl | test a_commit_in_a_reaction_loop_is_one_statement_last_write_winning ... ok

**C9** - `return e` from inside a loop, an `if` or a `match` gives the caller `e`. (CV-03, AC 1; T21)
Proof: run srcctl | test an_early_return_gives_the_caller_its_value ... ok

**C10** - `?` on `None` in a function returning `Option` returns `None`. (CV-03, AC 2; T21)
Proof: run srcctl | test the_question_mark_on_none_returns_none ... ok

**C11** - `return` outside a function body is `E0185` naming the position. (CV-03, AC 3; T13)
Proof: run typesctl | test return_outside_a_function_is_e0185 ... ok

**C12** - A guarded arm is taken only when its pattern matches and its guard is true. (CV-04, AC 1; T22)
Proof: run srcctl | test a_match_guard_takes_the_arm_only_when_true ... ok

**C13** - Range patterns `a..=b`, `a..b`, `a..` over integers and floats match as Rust. (CV-04, AC 2; T22)
Proof: run srcctl | test range_patterns_match_as_rust ... ok

**C14** - An integer `match` covered by ranges is exhaustive at any width up to 64. (CV-04, AC 2 and AC 5; T14)
Proof: run typesctl | test an_integer_match_covered_by_ranges_is_exhaustive ... ok

**C15** - Or-patterns with bindings and `name @ p` bind as Rust. (CV-04, AC 3; T22)
Proof: run srcctl | test or_patterns_and_at_bindings_bind_as_rust ... ok

**C16** - Or-pattern alternatives binding different names or types are `E0184`. (CV-04, AC 3; T14)
Proof: run typesctl | test alternatives_binding_differently_are_e0184 ... ok

**C17** - `if let`, `while let` and `let … else` with literal, range, tuple, variant and nested patterns run as Rust. (CV-04, AC 4; T22)
Proof: run srcctl | test refutable_patterns_run_in_if_let_while_let_and_let_else ... ok

**C18** - A non-exhaustive `match`, or a refutable `let` without `else`, is `E0184` naming one missing pattern. (CV-04, AC 5; T15)
Proof: run typesctl | test a_non_exhaustive_match_or_refutable_let_is_e0184 ... ok

**C19** - A closure bound by `let`, called, and passed to an H row or to `limit` runs, capturing by value (a later assignment is not seen). (CV-05, AC 1; T23)
Proof: run srcctl | test closures_run_and_capture_by_value ... ok

**C20** - A closure called where its class is not allowed is `E0122`. (CV-05, AC 2; T16)
Proof: run typesctl | test a_closure_called_outside_its_class_is_e0122 ... ok

**C21** - A closure called in `analog` is differentiated as a direct call. (CV-05, AC 3; T23)
Proof: run srcctl | test a_closure_in_analog_is_differentiated_through ... ok

**C22** - A closure with a form of §3 rule 6 inside a `fn` is `E0121`. (CV-05, AC 4; T16)
Proof: run typesctl | test a_closure_with_an_excluded_form_in_a_fn_is_e0121 ... ok

**C23** - A function's name used as a value, and a `fn(f64) -> f64` parameter, call the named function. (CV-06, AC 1; T23)
Proof: run srcctl | test a_function_value_calls_the_named_function ... ok

**C24** - A closure that captures nothing is accepted where a function pointer is expected. (CV-06, AC 2; T17)
Proof: run typesctl | test a_non_capturing_closure_is_a_function_pointer ... ok

**C25** - A capturing closure where a function pointer is expected is refused. (CV-06, AC 3; T17)
Proof: run typesctl | test a_capturing_closure_is_not_a_function_pointer ... ok

**C26** - A function value or a closure as a carried type is `E0112`. (CV-06, AC 4; T17)
Proof: run typesctl | test a_function_value_is_not_carried_e0112 ... ok

**C27** - Direct and mutual recursion from a reaction and a function run with Rust's values. (CV-07, AC 1; T24)
Proof: run srcctl | test recursion_from_a_reaction_runs ... ok

**C28** - A recursion reached from `analog` whose depth folds over params and constants is unrolled at elaboration. (CV-07, AC 2; T26)
Proof: run elabctl | test a_recursion_reached_from_analog_is_unrolled ... ok

**C29** - A recursion reached from `analog` that does not fold is `E0126` naming the function. (CV-07, AC 3; T26)
Proof: run elabctl | test an_unfoldable_recursion_from_analog_is_e0126 ... ok

**C30** - A call depth over 1 024 or a loop over 2^24 iterations fails the analysis naming the function or loop, the instance and the instant, and the process survives. (CV-07, AC 4; T24)
Proof: run srcctl | test runaway_loops_and_recursions_fail_the_analysis_by_name ... ok

**C31** - `Trips` panics on the iteration past 2^24 and not before. (CV-07, AC 4; T6)
Proof: run rt | test rt::control::tests::trips_panic_past_two_to_the_twenty_four ... ok

**C32** - `Depth` panics past 1 024 and not before. (CV-07, AC 4; T6)
Proof: run rt | test rt::control::tests::depth_panics_past_one_thousand_twenty_four ... ok

**C33** - §1 rule 11 states the loop and call-depth bounds. (CV-07, AC 4; T28)
Proof: run spectext | test section_1_rule_11_states_the_loop_and_depth_bounds ... ok

**C34** - An unknown loop condition, guard, `break` or `return` never faults: `unknown-semantics` has landed, so the fault branch is void (D2). (CV-08, AC 1; T9)
Proof: run tctl | test unknown_decisions_merge_written_locals_and_loop_exits ... ok

**C35** - Under unknown control a loop merges exit by exit and is cut at 64 iterations. (CV-08, AC 2; T7)
Proof: run rt | test rt::control::tests::unrolled_cuts_at_sixty_four ... ok

**C36** - A reaction loop committing after an unknown decision commits `MAYBE | MERGE`, as the merge rule of `unknown-semantics` says. (CV-08, AC 2; T9)
Proof: run tctl | test a_reaction_loop_committing_after_an_unknown_decision_commits_maybe ... ok

**C37** - Named, tuple and unit structs are value types. (CV-09, AC 1; T38)
Proof: run typesagg | test every_struct_form_is_a_value_type ... ok

**C38** - Struct expressions (shorthand, tuple-struct call, unit struct) give Rust's values. (CV-09, AC 2; T49)
Proof: run srcagg | test struct_expressions_give_rusts_values ... ok

**C39** - Struct-update syntax `..base` is a parse error (D3: §1.3.3, B.3.7). (CV-09, AC 2; T39)
Proof: run typesagg | test struct_update_syntax_is_a_parse_error ... ok

**C40** - `s.field`, `t.0` and nested fields read the field's value. (CV-09, AC 3; T49)
Proof: run srcagg | test field_reads_give_the_fields ... ok

**C41** - A struct expression leaving a field out, naming one twice or naming an unknown one is `E0186` naming the field. (CV-09, AC 4; T39)
Proof: run typesagg | test a_bad_struct_expression_is_e0186_naming_the_field ... ok

**C42** - Assigning `s.x` of a `let mut` struct changes that field only. (CV-09, AC 5; T49)
Proof: run srcagg | test assigning_a_field_changes_that_field_only ... ok

**C43** - Tuples built, returned, passed, nested and read with `.0` give Rust's values. (CV-10, AC 1; T49)
Proof: run srcagg | test tuples_give_rusts_values ... ok

**C44** - Tuple patterns in `let`, `match`, `if let`, a closure parameter and `for` bind each component. (CV-10, AC 2; T49)
Proof: run srcagg | test tuple_patterns_bind_each_component ... ok

**C45** - Tuple and struct variants are built and matched with their fields bound. (CV-11, AC 1; T50)
Proof: run srcagg | test payload_variants_build_and_match ... ok

**C46** - `#[repr(uN)]` and explicit discriminants are the tag, and `as` gives the discriminant. (CV-11, AC 2; T50)
Proof: run srcagg | test discriminants_are_the_tag_and_the_cast ... ok

**C47** - A discriminant that does not fit `N` levels, or repeats, is `E0111`. (CV-11, AC 3; T41)
Proof: run typesagg | test a_discriminant_that_does_not_fit_or_repeats_is_e0111 ... ok

**C48** - A one-variant enum has a tag of zero levels and its carrier holds the payload only. (CV-11, AC 4; T56)
Proof: run plancomp | test a_one_variant_enum_lays_out_its_payload_only ... ok

**C49** - `Option` of a struct, tuple, array or enum behaves as an enum with a payload. (CV-11, AC 5; T50)
Proof: run srcagg | test options_of_aggregates_behave_as_enums ... ok

**C50** - Derived `PartialEq`/`PartialOrd`/`Default` give `==`, `!=`, `<`, `<=`, `>`, `>=` and `T::default()` Rust's results. (CV-12, AC 1; T52)
Proof: run srcagg | test derived_comparisons_and_defaults_give_rusts_results ... ok

**C51** - An enum deriving `Default` is refused naming `Default` (D4: H.8 rule 4). (CV-12, AC 1; T44)
Proof: run typesagg | test an_enum_deriving_default_is_refused ... ok

**C52** - `==` or `<` on a type without the trait is refused naming the trait. (CV-12, AC 2; T44)
Proof: run typesagg | test comparing_without_the_trait_is_refused_naming_it ... ok

**C53** - `Debug` and `Hash` derives are accepted with no run-time effect. (CV-12, AC 3; T44)
Proof: run typesagg | test debug_and_hash_derives_are_accepted ... ok

**C54** - A read of a moved non-`Copy` value is `E0183` (§1.4.7). (CV-12, AC 1; T44)
Proof: run typesagg | test a_read_after_a_move_is_e0183 ... ok

**C55** - Arrays built, repeated, indexed, assigned by element, nested and passed to `&[T]` give Rust's values. (CV-13, AC 1; T51)
Proof: run srcagg | test arrays_give_rusts_values ... ok

**C56** - `&a[i..j]` and `&a[..]` give elements `i` to `j - 1` and all of them. (CV-13, AC 2; T51)
Proof: run srcagg | test slices_give_the_elements_in_range ... ok

**C57** - An index out of range during a run fails the analysis naming the instance, the instant and the index. (CV-13, AC 3; T51)
Proof: run srcagg | test an_index_out_of_range_panics_at_run_time ... ok

**C58** - An index out of range in a const context is `E0104` naming the expression. (CV-13, AC 3; T55)
Proof: run elabagg | test an_index_out_of_range_in_a_const_is_e0104 ... ok

**C59** - A slice whose start exceeds its end panics, as Rust. (CV-13, AC 3 and edge case; T51)
Proof: run srcagg | test a_slice_whose_start_exceeds_its_end_panics ... ok

**C60** - A non-constant array length is refused naming the length. (CV-13, AC 4; T42)
Proof: run typesagg | test a_non_constant_array_length_is_refused_naming_it ... ok

**C61** - An index that may be unknown reads the merge of the elements it may name and writes by merging into each (§3.3.13). (CV-13, `GAPS.md` L3 with unknown-semantics US-04 AC 12; T51)
Proof: run srcagg | test an_unknown_index_reads_and_writes_by_merge ... ok

**C62** - `&` of an array, slice or string for a parameter, and of any value as a call argument, is accepted. (CV-14, AC 1; T43)
Proof: run typesagg | test borrows_in_call_arguments_are_accepted ... ok

**C63** - `&` anywhere else is `E0102`. (CV-14, AC 2; T43)
Proof: run typesagg | test a_borrow_elsewhere_is_e0102 ... ok

**C64** - Element borrows from `iter`, `first`, `last`, `get` read as the element under `*`, comparison and arithmetic. (CV-14, AC 3; T52)
Proof: run srcagg | test element_borrows_read_as_the_element ... ok

**C65** - Every row of H.5, H.6 and H.7 gives Rust's result, `get` past the end `None`. (CV-15, AC 1; T53)
Proof: run srcagg | test every_row_of_h5_to_h7_gives_rusts_result ... ok

**C66** - A `for` over an array, a slice, a range or an adaptor chain visits elements in Rust's order. (CV-15, AC 2; T53)
Proof: run srcagg | test for_visits_elements_in_rusts_order ... ok

**C67** - A method outside Appendix H on an array, slice, range or iterator is `E0161`. (CV-15, AC 3; T45)
Proof: run typesagg | test a_method_outside_appendix_h_is_e0161 ... ok

**C68** - An empty array has `len()` 0 and `first()` `None`. (Edge case; T53)
Proof: run srcagg | test an_empty_array_has_length_zero_and_no_first ... ok

**C69** - The value code of the `zi_nd` reaction body runs as a function with Rust's values. (Phase A Independent Test, D18; T53)
Proof: run srcagg | test the_zi_nd_reaction_value_code_runs_as_a_function ... ok

**C70** - `normal(rng, mean, sigma)` equals §22.2.17's text compiled by Rust, bit for bit. (CV-16, AC 1; T54)
Proof: run srcagg | test normal_matches_the_spec_text_bit_for_bit ... ok

**C71** - `normal` with `sigma = 0.0` returns `mean`. (CV-16, AC 2; T54)
Proof: run srcagg | test normal_with_zero_sigma_returns_the_mean ... ok

**C72** - The §22.2.17 jitter clock gives the same edge times on every run with one seed. (CV-16, AC 3; T54)
Proof: run srcagg | test the_jitter_clock_repeats_with_its_seed ... ok

**C73** - A derived `Repr` lays out per §2 (fields first-lowest; tag lowest then the widest payload, padded) and `T::WIDTH` is that width. (CV-17, AC 1; T57)
Proof: run srcagg | test derived_repr_layout_and_width_follow_section_2 ... ok

**C74** - `#[derive(Repr)]` with a field without `Repr` is `E0110` naming the field. (CV-17, AC 2; T46)
Proof: run typesagg | test deriving_repr_with_a_field_without_repr_is_e0110 ... ok

**C75** - A `Bit<T>` of a derived struct, derived enum, tuple, array or `Option` of one initializes, is defined, committed, read, and fires `changed()` with `prev()` over the whole value. (CV-17, AC 3; T57)
Proof: run srcagg | test composite_carriers_initialize_define_commit_and_change ... ok

**C76** - Committing a composite equal to the current value fires no `changed()`. (CV-17, AC 4; T57)
Proof: run srcagg | test committing_an_equal_composite_fires_no_change ... ok

**C77** - A tag naming no variant reads as unknown on decode. (CV-17, AC 5; T57)
Proof: run srcagg | test a_tag_naming_no_variant_reads_unknown ... ok

**C78** - A narrower variant pads with false levels and decode ignores the padding. (Edge case; T37)
Proof: run tagg | test narrow_payloads_pad_with_false_levels ... ok

**C79** - A param of struct, tuple or array type, with a default and a `where`, binds at a placement and reads in every block. (CV-18, AC 1; T58)
Proof: run srcagg | test composite_params_bind_and_read_everywhere ... ok

**C80** - A host sets a composite param by leaf path and its `where` is checked (`E0317`). (CV-18, AC 2; T59)
Proof: run apicomp | test a_host_sets_a_composite_param_by_leaf_and_checks_its_where ... ok

**C81** - An `emit!` field of a composite type is delivered and its type is in the stream metadata. (CV-19, AC 1; T60)
Proof: run srcagg | test a_record_carries_a_composite_field_and_its_type ... ok

**C82** - Two sites giving one field two composite types is `E0605`. (CV-19, AC 2; T47)
Proof: run typesagg | test two_sites_giving_a_field_two_composite_types_is_e0605 ... ok

**C83** - The IR holds aggregate types and `Ir::check` types every aggregate expression. (CV-20, AC 1; T31)
Proof: run iragg | test ir_check_types_every_aggregate_expression ... ok

**C84** - Every value of small composite types crosses the device ABI unchanged. (CV-20, AC 2; T37)
Proof: run tagg | test every_composite_value_crosses_the_abi_unchanged ... ok

**C85** - Two pins of one composite net have equal layouts (abi.md P4). (CV-20, AC 3; T56)
Proof: run plancomp | test two_pins_of_a_composite_net_have_equal_layouts ... ok

**C86** - Each source struct or enum prints as one Rust item naming its IR and MSL path. (CV-20, AC 4; T34)
Proof: run tagg | test each_source_type_prints_as_one_rust_item_naming_its_path ... ok

**C87** - A composite carrier and a composite record field print in the text table, record lines, CSV, JSON Lines and plot as Q2 (c) chooses. (CV-21, AC 1; T62)
Proof: run clirun | test composites_print_in_each_output_format ... ok

**C88** - A printed path naming a field or element prints that leaf alone. (CV-21, AC 2; T62)
Proof: run clirun | test a_leaf_path_prints_that_leaf_alone ... ok

**C89** - A carried enum reads as its variant name, not its tag. (CV-22, AC 1; T61)
Proof: run apicomp | test a_carried_enum_reads_as_its_variant_name ... ok

**C90** - Enum variant and field names live in the IR's type arena and the net's bytes do not change. (CV-22, AC 2; T61)
Proof: run apicomp | test enum_names_live_in_the_ir_not_in_the_net_bytes ... ok

**C91** - The interpreter refuses every form of this feature by its name. (CV-34, AC 1; T2 to T33)
Proof: run interp | test each_composite_values_form_is_refused_by_name ... ok

**C92** - The forms of phases C and D stay `P0001` naming the form. (CV-34, AC 2; T47)
Proof: run typesagg | test phase_c_and_d_forms_stay_p0001_by_name ... ok

**C93** - The 7 ngspice twins print the bytes they printed before. (CV-35, AC 1; every phase)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C94** - The build gives no warning on the crates this feature touches, and the hygiene guard passes. (CV-35, AC 2; every phase)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

**C95** - Phase A forms in a `const`, a `const fn`, a param default, a `where` and a generation range evaluate to their run-time value. (CV-36, AC 1; T27)
Proof: run elabctl | test phase_a_forms_fold_in_const_contexts ... ok

**C96** - Aggregates and the stdlib's `poly_q`, `den_q`, `conjugate_closed` fold in const contexts. (CV-36, AC 1; T55)
Proof: run elabagg | test aggregates_fold_in_const_contexts ... ok

**C97** - A panic in a const evaluation is an elaboration error naming the expression. (CV-36, AC 2; T27)
Proof: run elabctl | test a_panic_in_a_const_evaluation_names_the_expression ... ok

**C98** - `GAPS.md` no longer lists L5, L6, L9 (but its `generics` row) and L10, and §9 is updated. (CV-37, AC 1; T29)
Proof: run docs | test gaps_drops_what_composite_values_phase_a_closed ... ok

**C99** - `GAPS.md` no longer lists L1 to L3, L4 (but generic enums), the H.5 to H.7 rows, `normal` and the §4.7 enum-names note. (CV-37, AC 1; T63)
Proof: run docs | test gaps_drops_what_composite_values_phase_b_closed ... ok

**C100** - Every gallery fence and Book listing these forms let check is held to the checker. (CV-37, AC 2; T29, T63)
Proof: run galleries | test the_fences_composite_values_lifted_are_checked ... ok

**C101** - An empty range runs its body zero times, in `analog` too. (Edge case; T20)
Proof: run srcctl | test an_empty_range_runs_zero_times_in_analog_too ... ok

**C102** - An empty struct and `()` are carried with zero levels. (Edge case; T56)
Proof: run plancomp | test an_empty_struct_is_carried_with_zero_levels ... ok

**C103** - The IR accepts a recursion outside `analog` and refuses one reached from an `analog` body, so no recursion reaches the solver un-unrolled (D12). (CV-07, AC 1 and AC 3; T5)
Proof: run irctl | test recursion_reached_from_analog_is_refused ... ok

**C104** - The IR's dependency walk sees a probe a loop reaches only on a later iteration (the fixed point of a loop). (CV-02, AC 4; T3)
Proof: run irctl | test a_loop_reaching_a_probe_on_a_later_iteration_reads_it ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (2026-11-20, batch B1, branch `feature/composite-values` at `c720b421`, already on `origin/main` `4f505e09` with `analog-structure` merged: no rebase needed, `**Build**` line green and zero warnings at that head). Anchors re-read at the new `HEAD`; everything design.md names exists, these moved or are not what the design calls them: (1) `Ir::check`'s recursion check is `Checker::recursion` in `check.rs` (was `check.rs:1052`) and `deps.rs`'s recursion refusal is `Walk::enter` (was `deps.rs:852`) - both T5; (2) the checker's refusals moved to `types/unsupported.rs` plus `types/call.rs:334` (`closure`, was `call.rs:333`) and `types/expr.rs` - T11 to T18 read them by name, not line; (3) `Known` is not a type: the `known_or`/`is_known`/`high` lowering is `Builtin::Known*` in `elab/known.rs` and the known/unknown decision is the `Flagged` type of the condition - T19 to T24 and T22 key on `ValueType::Flagged`; (4) `Frame` is not in `elab/structural.rs`: the carrier `Frame` is `elab/carriers.rs:48`, the placement frame is the `frame: usize` index of `elab/hoist.rs` (`hoisted_name`, `hoist`, `hoist_placements`) - T25 and T26 read `hoist.rs` and `structural.rs` again before editing; (5) `rt::Merge` is a trait (`rt/flag.rs:325`), not a struct, and `Carried::to_levels` is the trait method at `rt/flag.rs:38` - T6, T7, T36; (6) `ValuePrinter::merged` is `print/value.rs:1038`, `holds_effect` `:1057`, `block_lines` `:1085`; `Commits::branch` `rt/commits.rs:325`, `run` `:346`; `CarrierLayout` `plan/digital.rs:160`, `RecordLayout` `:252`; `Metadata` `api/metadata.rs:47`, `Cell` `api/table.rs:18` (all as design.md, line numbers only drifted); (7) the next free `MD-` number is `MD-73` (`MD-72` is taken by "the root package holds the integrated tests", STATE.md still says "MD-72 is the next free one" in a stale line) - T62. B1 note (T2): `crates/piperine-ir/tests/{model,print}.rs` read every file of `src/` flat; the new `deps/control.rs` needs a directory, so both now go through `tests/support/sources.rs` (`SourceTree`, recursive: a strengthening, not a weakening of the same assertions). `Local` gained `mutable`; every literal of `Local` in the tree (about 35 sites in tests and in `elab`) now writes `mutable: false`, no behaviour change. The interpreter refusal is `piperine-interp/src/composite.rs` (`CompositeForms`), one walk of everything a body evaluates, wired in `Interpreter::new` beside `amended_forms` and `structure_forms`; `Expr::operands`/`Expr::statements` (`model.rs`) are the one operand accessor the new walkers use. B1 defaults (T3 to T5, taken while the user is away, all within design.md): `Ir::check` refuses a contribution inside a loop (T25 unrolls it before the IR, section 11 rule 12), a `return` inside a closure (a closure `return` returns from the closure; T16 and T23 lift it when closures carry control) and an assignment with a field path (no aggregate type before T30 and T31); a `FunctionRef` counts as a call edge for the recursion and `analog`-reachability check, and the dependency walk takes it as a call with unknown arguments, so a call through a value is `Nonlinear`; the existing `check.rs` test `a_recursive_call_or_an_unbound_local_is_named` is NOT moved: its recursive functions are reached from the `analog` addend, so it still passes as written, and `recursion_reached_from_analog_is_refused` (`tests/control.rs`) is the acceptance test of the new rule beside it; `Restore` in `deps.rs` now keeps what a block assigned to an outer local (it only unbinds the locals the block bound), and `done` does not memoize an `if`, a block with statements or a loop. Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("Tu sempre assume o default mais razoável e depois me conta"); the defaults are design.md D1 to D20.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 15

Phase 1:  T1
Phase 2:  T2 ------→ T3 ------→ T4 ------→ T5
Phase 3:  T6 ------→ T7 ------→ T8 ------→ T9 ------→ T10
Phase 4:  T11 -----→ T12 -----→ T13 -----→ T14 -----→ T15 -----→ T16 -----→ T17 -----→ T18
Phase 5:  T19 -----→ T20 -----→ T21 -----→ T22 -----→ T23 -----→ T24
Phase 6:  T25 -----→ T26 -----→ T27
Phase 7:  T28 -----→ T29
Phase 8:  T30 -----→ T31 -----→ T32 -----→ T33
Phase 9:  T34 -----→ T35 -----→ T36 -----→ T37
Phase 10: T38 -----→ T39 -----→ T40 -----→ T41 -----→ T42 -----→ T43
Phase 11: T44 -----→ T45 -----→ T46 -----→ T47
Phase 12: T48 -----→ T49 -----→ T50 -----→ T51
Phase 13: T52 -----→ T53 -----→ T54 -----→ T55
Phase 14: T56 -----→ T57 -----→ T58 -----→ T59 -----→ T60 -----→ T61 -----→ T62
Phase 15: T63
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B1 | 1, 2 | T1 to T5 | starts after `analog-structure` is on `main` |
| B2 | 3 | T6 to T10 | |
| B3 | 4 | T11 to T18 | |
| B4 | 5 | T19 to T24 | |
| B5 | 6, 7 | T25 to T29 | the only batch that enters `elab/structural.rs`/`hoist.rs`; closes phase A (`GAPS.md` L5, L6, L9, L10) |
| B6 | 8, 9 | T30 to T37 | phase B starts |
| B7 | 10 | T38 to T43 | |
| B8 | 11, 12 | T44 to T51 | T48 moves the buck's generated text (enums), never its numbers |
| B9 | 13 | T52 to T55 | |
| B10 | 14, 15 | T56 to T63 | closes phase B; `generics` A starts on the merged tree |

**Later batch boundary (not tasked here):** phase C (CV-23 to CV-28, ROADMAP item 11) runs after `generics` A–B, from `design-c.md`/`tasks-c.md` (design.md "Phase C").

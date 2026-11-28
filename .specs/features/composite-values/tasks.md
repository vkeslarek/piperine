# Composite Values Tasks (phases A and B)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/composite-values/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite suite_hygiene::`
**Run irctl**: `cargo test -p piperine-ir --test model control::`
**Run iragg**: `cargo test -p piperine-ir --test model aggregates::`
**Run interp**: `cargo test -p piperine-interp --test interpreter frozen::`
**Run rt**: `cargo test -p piperine-transpile --lib rt::`
**Run tctl**: `cargo test -p piperine-transpile --test print control::`
**Run tagg**: `cargo test -p piperine-transpile --test print aggregates::`
**Run typesctl**: `cargo test -p piperine-lang --test types_control`
**Run typesagg**: `cargo test -p piperine-lang --test types_aggregates`
**Run elabctl**: `cargo test -p piperine-lang --test elab_control`
**Run elabagg**: `cargo test -p piperine-lang --test elab_aggregates`
**Run srcctl**: `cargo test -p piperine-parity --test source_control`
**Run errors**: `cargo test -p piperine-lang --test msl_errors`
**Run typesvs**: `cargo test -p piperine-lang --test types_value_semantics`
**Run srcvs**: `cargo test -p piperine-parity --test source_value_semantics`
**Run typescmp**: `cargo test -p piperine-lang --test types_compound`
**Run booktypes**: `cargo test -p piperine --test book_types`
**Run srcagg**: `cargo test -p piperine-parity --test source_aggregates`
**Run plancomp**: `cargo test -p piperine-plan --test shape composite_shape::`
**Run apicomp**: `cargo test -p piperine-api --test host composites::`
**Run clirun**: `cargo test -p piperine-cli --test commands run::`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`
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

**What**: Rebase `feature/composite-values` onto the current `main` once `analog-structure` has merged there (ROADMAP item 7; if it has not, stop and report). Re-read every anchor of design.md "What the code does today", "Components" and "Cross-lane touches" at the new `HEAD`: `Unsupported`, `Checker::pattern`/`exhaustive`, `tuple_let`, `enum_type`, `closure`, `EnumLayout`, `arms_chain`, `Known`, `Variants`, `ConstEval`, `Scope`, `elab/structural.rs` `Frame` and the hoisting names, `contribute.rs`, `ValuePrinter::merged`/`holds_effect`/`block_lines`, `rt::Merge`/`Commits::branch`/`run`, `Carried::to_levels`, `CarrierLayout`/`RecordLayout`, `deps.rs`'s recursion refusal, `Ir::check`'s recursion check, `Metadata::of`, `Cell`, the next free `MD-` number. Write every difference, and how each later task maps onto it, on the Handoff's `**Settled mid-build:**` line. No code change. B8 settled items (T44 to T51, batch B8): (1) `P0002` is retired (params-and-io, `no_source_spells_p0002`): a comparison on a type without the trait, a derive whose field lacks the trait and `#[derive(Default)]` on an enum are `E0180` naming the trait (D6 amended). (2) `T::default()` resolves in the POM as `Deferred(BuiltinTraitMember)` through a struct or an enum (no `BUILTINS` row: `the_core_rows_are_unchanged` pins the table) and the checker records its type (`Types::default_of`); `Option<T>` implements `Default` whatever `T` is (H.8 rule 4). (3) `Ty::Range { iterable }` and `Ty::Iterator { reversible }` carry H.6 rule 3 and H.7 rule 3 in the type; an open-bounded range types only for `contains`/`is_empty`, `..` is `P0001`; the `for` of a range written `a..b` keeps its element-typed path. (4) `Repr` levels are `Checker::repr_levels`, not `Ty::levels`; an enum still needs `#[derive(Repr)]` to be typed (the relaxation was not done). (5) Composite params and `emit!` fields take plain data (leaves `f64`, `bool`, integers, enums, `str` for a record); an `Option` param stays params-and-io's. (6) `Types` exposes `aggregate_types`, `fields`, `enum_tag`, `derives`, `integer_constant` (an array length or a range bound folds without a typed literal). (7) T48: the constant evaluator names a unit variant by its position, `Types::enum_tag` gives discriminants; a carried enum keeps the tag's bytes through `Ir::carried`, `CarrierLayout::in_ir`/`bytes_in`, `RecordLayout::in_ir`, and the api decodes through `Ir::carried` (variant names are T61); IR types `Flagged(Enum)`, `==`/`<` on enums, `as` of an enum to an integer and level counts through the arena; the arena's `Derives` always say `Clone`+`Copy` (every value copies) and `PartialEq` when `Repr` is derived; a known subject is tested by `IsVariant`, one that may be unknown by comparison. (8) T49: a struct or tuple part that may be unknown is refused by name; a call whose `Real` nothing fixes is named (`::<f64>`). (9) T50: an `Option` of an aggregate is the pair in value code and `OptionPack`/`OptionPart` where one value is needed; a type held by an `Option` gets a `Default` impl; the zero of an aggregate is the aggregate of zeros. (10) T51: an array with an element that may be unknown holds flagged elements, and an index that may be unknown over plain elements is refused by name (the printer already says so); a range index outside its array is `E0104` at elaboration; element assignment is a `Projection::Index` place.
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

- [x] `known_control_prints_plain_rust_and_runs` passes under `**Run tctl**`
- [x] Gate check passes: Full

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

- [x] `unknown_decisions_merge_written_locals_and_loop_exits` and `a_reaction_loop_committing_after_an_unknown_decision_commits_maybe` pass under `**Run tctl**`
- [x] Gate check passes: Full

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

- [x] `function_values_and_recursion_compile_and_run` and `a_recursion_past_the_depth_bound_faults_by_name` pass under `**Run tctl**`
- [x] Gate check passes: Full

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

- [x] `let_mut_and_every_compound_operator_type_check`, `assigning_a_non_place_is_e0183_naming_the_target`, `a_read_before_assignment_is_e0183` and `let_mut_in_a_module_body_is_e0331` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `every_loop_form_type_checks` and `break_or_continue_outside_a_loop_is_e0185` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `return_and_question_mark_type_check` and `return_outside_a_function_is_e0185` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `guards_ranges_or_bindings_and_at_type_check`, `alternatives_binding_differently_are_e0184` and `an_integer_match_covered_by_ranges_is_exhaustive` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `refutable_patterns_type_check_in_if_let_while_let_and_let_else` and `a_non_exhaustive_match_or_refutable_let_is_e0184` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `closures_type_check_where_rust_allows_them`, `a_closure_called_outside_its_class_is_e0122` and `a_closure_with_an_excluded_form_in_a_fn_is_e0121` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `a_function_name_is_a_function_value`, `a_non_capturing_closure_is_a_function_pointer`, `a_capturing_closure_is_not_a_function_pointer` and `a_function_value_is_not_carried_e0112` pass under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `direct_and_mutual_recursion_type_check` passes under `**Run typesctl**`
- [x] Gate check passes: Quick

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

- [x] `let_mut_lowers_to_mutable_locals_and_assignments` passes under `**Run elabctl**`
- [x] `let_mut_and_every_compound_operator_give_rusts_values` passes under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `every_loop_form_gives_rusts_values`, `a_commit_in_a_reaction_loop_is_one_statement_last_write_winning`, `an_analog_loop_without_contribution_is_differentiated_through` and `an_empty_range_runs_zero_times_in_analog_too` pass under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `an_early_return_gives_the_caller_its_value` and `the_question_mark_on_none_returns_none` pass under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `a_match_guard_takes_the_arm_only_when_true`, `range_patterns_match_as_rust`, `or_patterns_and_at_bindings_bind_as_rust` and `refutable_patterns_run_in_if_let_while_let_and_let_else` pass under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `closures_run_and_capture_by_value`, `a_closure_in_analog_is_differentiated_through` and `a_function_value_calls_the_named_function` pass under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `recursion_from_a_reaction_runs` and `runaway_loops_and_recursions_fail_the_analysis_by_name` pass under `**Run srcctl**`
- [x] Gate check passes: Full

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

- [x] `an_analog_loop_that_contributes_or_places_is_unrolled` and `a_contributing_analog_loop_without_a_constant_trip_count_is_e0416` pass under `**Run elabctl**`
- [x] Gate check passes: Full

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

- [x] `a_recursion_reached_from_analog_is_unrolled` and `an_unfoldable_recursion_from_analog_is_e0126` pass under `**Run elabctl**`
- [x] Gate check passes: Full

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

- [x] `phase_a_forms_fold_in_const_contexts` and `a_panic_in_a_const_evaluation_names_the_expression` pass under `**Run elabctl**`
- [x] Gate check passes: Quick

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

- [x] `section_1_rule_11_states_the_loop_and_depth_bounds` passes under `**Run spectext**`
- [x] Gate check passes: Quick

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

- [x] `gaps_drops_what_composite_values_phase_a_closed` passes under `**Run docs**`
- [x] `the_fences_composite_values_lifted_are_checked` passes under `**Run galleries**` for the phase A fences
- [x] Gate check passes: Build

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

- [x] `the_type_arena_builds_checks_and_prints` passes under `**Run iragg**`
- [x] Gate check passes: Quick

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

- [x] `ir_check_types_every_aggregate_expression` passes under `**Run iragg**`
- [x] `each_composite_values_form_is_refused_by_name` covers the forms under `**Run interp**`
- [x] Gate check passes: Quick

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

- [x] `variant_tests_option_packs_and_comparisons_check` passes under `**Run iragg**`
- [x] Gate check passes: Quick

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

- [x] `composite_constants_check_print_and_round_trip` passes under `**Run iragg**`
- [x] Gate check passes: Build

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

- [x] `each_source_type_prints_as_one_rust_item_naming_its_path` passes under `**Run tagg**`
- [x] Gate check passes: Full

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

- [x] `aggregate_expressions_compile_and_run` passes under `**Run tagg**`
- [x] `rt::control::tests::an_index_out_of_range_panics_naming_it` and `rt::control::tests::an_unknown_index_merges_the_elements_it_may_name` pass under `**Run rt**`
- [x] Gate check passes: Full

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

- [x] `rt::flag::tests::composite_levels_follow_the_section_2_table` passes under `**Run rt**`
- [x] Gate check passes: Full

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

- [x] `every_composite_value_crosses_the_abi_unchanged` and `narrow_payloads_pad_with_false_levels` pass under `**Run tagg**`
- [x] Gate check passes: Full

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

- [x] `every_struct_form_is_a_value_type` and `a_self_containing_type_is_e0186` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `struct_expressions_and_field_reads_type_check`, `a_bad_struct_expression_is_e0186_naming_the_field` and `struct_update_syntax_is_a_parse_error` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `tuples_and_tuple_patterns_type_check` passes under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `payload_enums_build_and_match` and `a_discriminant_that_does_not_fit_or_repeats_is_e0111` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `arrays_and_indexing_type_check` and `a_non_constant_array_length_is_refused_naming_it` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): arrays and indexing`

---

### T43: Checker: borrows and dereference (dropped by `value-semantics`: CV-14 is void, no borrow exists)

**What**: In `types/aggregate.rs`: `&` of an array, slice or string for a function parameter, and of any value as a call argument (§1 rule 2); anywhere else `E0102`; `*r` of a `Copy` value; the four read-through places of §1.2.6 (field, index, method receiver, scalar operator operand).
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T42
**Reuses**: `Ty::Ref`
**Requirement**: CV-14
**Executor**: Sonnet

**Done when**:

- [x] Void: CV-14 is dropped by `value-semantics` (VS-01), so there is no test and no code to deliver

**Tests**: none
**Gate**: none

**Commit**: none (void)

---

### T44: Checker: derives, comparison, defaults and moves

**What**: `types/derives.rs` (`Derives`): the H.8 traits a type derives, their requirements (H.8 rule 6), the traits of built-in types (rule 7); `==`/`!=` need `PartialEq`, `<`/`<=`/`>`/`>=` need `PartialOrd` (§1.3.10), refused naming the trait (`P0002`, D6); `T::default()` for a type deriving `Default`; `#[derive(Default)]` on an enum refused naming `Default` (D4); `Debug` and `Hash` accepted with no effect; an unknown derive name `E0161` (`Clone` and `Copy` are unknown names: value-semantics, no move and no `x.clone()`). The H.8 table of `Derives` holds seven traits (`Debug`, `Hash` accepted with no effect).
**Where**: `crates/piperine-lang/src/types/derives.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::derives`
**Requirement**: CV-12
**Executor**: Sonnet

**Done when**:

- [x] `derived_traits_give_comparison_and_default`, `comparing_without_the_trait_is_refused_naming_it`, `debug_and_hash_derives_are_accepted` and `an_enum_deriving_default_is_refused` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): derives, comparison and defaults`

---

### T45: Checker: Appendix H.5 to H.7

**What**: In `types/catalogue.rs`: the H.5 rows on arrays (`len`, `is_empty`, `iter`, `contains`, `first`, `last`, `get`), H.6 on ranges (`contains`, `is_empty`), H.7 on iterators (`map`, `filter`, `fold`, `sum`, `count`, `zip`, `enumerate`, `rev` where rule 3 allows, `any`, `all`) with `Ty::Range` and `Ty::Iterator`; `for` over an array and any chain (§1.8.9). Rows are by value (value-semantics): `iter` yields `T`, `first`/`last`/`get` give `Option<T>`, `contains(x: T)`, `filter`'s closure takes `T`; receivers are arrays only; no `Ty::Ref`; any other method on these receivers is `E0161` naming method and type.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T44
**Reuses**: the catalogue rows, `Checker::closure_typed`
**Requirement**: CV-15
**Executor**: Sonnet

**Done when**:

- [x] `the_rows_of_h5_to_h7_type_check` and `a_method_outside_appendix_h_is_e0161` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `composite_carriers_type_check`, `deriving_repr_with_a_field_without_repr_is_e0110` and `a_carried_type_without_repr_is_e0112` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `composite_params_and_record_fields_type_check`, `two_sites_giving_a_field_two_composite_types_is_e0605` and `phase_c_and_d_forms_stay_p0001_by_name` pass under `**Run typesagg**`
- [x] Gate check passes: Quick

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

- [x] `every_reached_struct_and_enum_is_one_arena_type` passes under `**Run elabagg**`
- [x] Every buck target passes with unchanged assertions
- [x] Gate check passes: Full

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

- [x] `struct_expressions_give_rusts_values`, `field_reads_give_the_fields`, `assigning_a_field_changes_that_field_only`, `tuples_give_rusts_values` and `tuple_patterns_bind_each_component` pass under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `payload_variants_build_and_match`, `discriminants_are_the_tag_and_the_cast` and `options_of_aggregates_behave_as_enums` pass under `**Run srcagg**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): payload enums and options of aggregates`

---

### T51: Elaborator: arrays, indexing and slices

**What**: In `elab/aggregate.rs`: array literals, `[x; N]` (evaluated once), indexing (out of range a run panic naming instance, instant and index), element assignment, nested arrays; a const range index lowers to the array of its elements, `rest @ ..` binds that array, and an array passed to a function is a copy (value-semantics: no `&a`, no slice); an index that may be unknown reads the merge of the elements it may name and writes by merging into each (§3.3.13, unknown-semantics US-04 AC 12).
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: T50
**Reuses**: T35's `rt::Index` through the printer
**Requirement**: CV-13
**Executor**: Sonnet

**Done when**:

- [x] `arrays_give_rusts_values`, `a_range_index_gives_the_array_of_its_elements`, `an_index_out_of_range_panics_at_run_time` and `an_unknown_index_reads_and_writes_by_merge` pass under `**Run srcagg**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): arrays, indexing and range indexes`

---

### T52: Elaborator: derives and defaults

**What**: In `elab/aggregate.rs`: derived `==`, `!=`, `<`, `<=`, `>`, `>=` on structs, tuples, arrays, enums (enums by discriminant then fields, H.8 rule 3) and `Option`s (`None` lowest, §1.5.23); `clone()`; `T::default()` (`Expr::Default`). No `clone()` and no element borrow (value-semantics: every value is copied).
**Where**: `crates/piperine-lang/src/elab/aggregate.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Expr::Binary` on aggregates (T32)
**Requirement**: CV-12
**Executor**: Sonnet

**Done when**:

- [x] `derived_comparisons_and_defaults_give_rusts_results` passes under `**Run srcagg**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): derived comparisons and defaults`

---

### T53: Elaborator: Appendix H.5 to H.7 as loops

**What**: `elab/iterators.rs` (`IteratorChain`): H.5 rows (`len` a constant), H.6 rows as comparisons, H.7 chains and `for` over arrays and chains (no slices; elements are values) as one `ForRange` over indices (D10), in Rust's order; an empty array (`len` 0, `first` `None`); `get` past the end is `None`. The value code of the stdlib `zi_nd` reaction body (`acc` over `num`, `xs`, `den`, `ys`) runs as a monomorphic `fn` over arrays against Rust (D18).
**Where**: `crates/piperine-lang/src/elab/iterators.rs`
**Depends on**: T52
**Reuses**: `elab/control.rs` loops, `PatternTest`
**Requirement**: CV-15
**Executor**: Sonnet

**Done when**:

- [x] `every_row_of_h5_to_h7_gives_rusts_result`, `for_visits_elements_in_rusts_order`, `an_empty_array_has_length_zero_and_no_first` and `the_zi_nd_reaction_value_code_runs_as_a_function` pass under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `normal_matches_the_spec_text_bit_for_bit`, `normal_with_zero_sigma_returns_the_mean` and `the_jitter_clock_repeats_with_its_seed` pass under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `aggregates_fold_in_const_contexts` and `an_index_out_of_range_in_a_const_is_e0104` pass under `**Run elabagg**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): aggregates in const contexts`

---

### T56: Plan: composite layouts

**What**: `piperine-plan/src/composite.rs` (`CompositeLayout`): the ABI `Layout` of an IR aggregate type as built: the value bytes packed at alignment 1 (an `f64` leaf may sit unaligned), one field per scalar leaf with a dotted name (`p.x`, `t.0`, `tag`, `Data.n`), an array one field per element (`num[2]`, no `count > 1` field), and one known byte for the whole composite (`HAS_KNOWN`); used by `CarrierLayout`, `RecordLayout` and param pins; a one-variant enum lays out its payload only (§2 rule 5); an empty struct and `()` have zero levels. Create `crates/piperine-plan/tests/composite_shape.rs`.
**Where**: `crates/piperine-plan/src/composite.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `CarrierLayout`, `RecordLayout`, `ShapeBuf`
**Requirement**: CV-17, CV-20, CV-11
**Executor**: Sonnet

**Done when**:

- [x] `composite_layouts_list_their_leaves`, `two_pins_of_a_composite_net_have_equal_layouts`, `a_one_variant_enum_lays_out_its_payload_only` and `an_empty_struct_is_carried_with_zero_levels` pass under `**Run plancomp**`
- [x] Gate check passes: Full

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

- [x] `derived_repr_layout_and_width_follow_section_2`, `composite_carriers_initialize_define_commit_and_change`, `committing_an_equal_composite_fires_no_change` and `a_tag_naming_no_variant_reads_unknown` pass under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `composite_params_bind_and_read_everywhere` passes under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `a_host_sets_a_composite_param_by_leaf_and_checks_its_where` passes under `**Run apicomp**`
- [x] Gate check passes: Full

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

- [x] `a_record_carries_a_composite_field_and_its_type` passes under `**Run srcagg**`
- [x] Gate check passes: Full

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

- [x] `a_carried_enum_reads_as_its_variant_name` and `enum_names_live_in_the_ir_not_in_the_net_bytes` pass under `**Run apicomp**`
- [x] Gate check passes: Full

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

- [x] `composites_print_in_each_output_format` and `a_leaf_path_prints_that_leaf_alone` pass under `**Run clirun**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(cli): composites in every output format`

---

### T63: Phase B bookkeeping

**What**: `GAPS.md` drops L1, L2, L3, L4 (all but generic enums, which stay with `generics` GN-06), the H.5 to H.7 rows of L16, `normal` in L16 and L58 (L3's "slices, `&`" is closed by `value-semantics`, not listed as delivered here; the Book status rows this edits are those left after its T14), and the §4.7 note "Enum variant names are not carried"; §9 Coverage updates I §1–§2, I §3–§3.3, VII §22 and H; the fences and Book listings these forms lift are held to the checker (CV-37 AC 2); `.specs/STATE.md` gains the next free `MD-` entry of design.md "Tech Decisions"; the ROADMAP lane row says phases A–B delivered.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `docs.rs`, `std_galleries.rs`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [x] `gaps_drops_what_composite_values_phase_b_closed` passes under `**Run docs**`
- [x] `the_fences_composite_values_lifted_are_checked` passes under `**Run galleries**` for the phase B fences
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: composite-values phase B closes L1 to L4, L16 and the enum names`

---

## Checks

**C1** - `let mut` and every compound operator give Rust's values in a function, a reaction and an `analog` block. (CV-01, AC 1; T19)
Proof: run srcctl | test let_mut_and_every_compound_operator_give_rusts_values ... ok

**C2** - Assigning to a name that is no place (a param, a const, a fn parameter, a carrier, a body `let`) is `E0183` naming the target; every local a `let` binds is assignable (value-semantics VS-02, VS-03). (CV-01, AC 2; T11)
Proof: run typesctl | test assigning_a_non_place_is_e0183_naming_the_target ... ok

**C3** - `let mut` in a module body is `E0903` with the help to write the plain form (CV-01 AC 3 amended by value-semantics VS-02 AC 2: `E0331` is retired). (CV-01, AC 3; T11)
Proof: run errors | test let_mut_in_a_module_body_is_e0903 ... ok

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

**C33** - §1 rule 11 states the loop and call-depth bounds. (CV-07, AC 4; T28) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run ledger | test every_retired_check_is_recorded ... ok

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

**C54** - A value is read twice and copied: there are no moves, so no read of a used value is refused (CV-12 AC 1 amended by value-semantics VS-04 AC 3, §1.4.7). (CV-12, AC 1; T44)
Proof: run srcvs | test a_value_is_read_twice ... ok

**C55** - Arrays built, repeated, indexed, assigned by element, nested and passed to `[T; N]` give Rust's values. (CV-13, AC 1; T51)
Proof: run srcagg | test arrays_give_rusts_values ... ok

**C56** - `a[i..j]` and `a[..]` give the array of elements `i` to `j - 1` and of all of them (value-semantics VS-06: there are no slices). (CV-13, AC 2; T51)
Proof: run srcagg | test a_range_index_gives_the_array_of_its_elements ... ok

**C57** - An index out of range during a run fails the analysis naming the instance, the instant and the index. (CV-13, AC 3; T51)
Proof: run srcagg | test an_index_out_of_range_panics_at_run_time ... ok

**C58** - An index out of range in a const context is `E0104` naming the expression. (CV-13, AC 3; T55)
Proof: run elabagg | test an_index_out_of_range_in_a_const_is_e0104 ... ok

**C59** - A range index whose start exceeds its end, or that ends past the array, is `E0104` at elaboration, where a slice once panicked (value-semantics VS-06 AC 3). (CV-13, AC 3 and edge case; T51)
Proof: run typesvs | test a_range_index_out_of_the_array_is_refused ... ok

**C60** - A non-constant array length is refused naming the length. (CV-13, AC 4; T42)
Proof: run typesagg | test a_non_constant_array_length_is_refused_naming_it ... ok

**C61** - An index that may be unknown reads the merge of the elements it may name and writes by merging into each (§3.3.13). (CV-13, `GAPS.md` L3 with unknown-semantics US-04 AC 12; T51)
Proof: run srcagg | test an_unknown_index_reads_and_writes_by_merge ... ok

Void, not checks: C62 to C64 (CV-14, borrows) are dropped with the requirement by value-semantics VS-01 (there are no borrows, `E0102` is retired, VS-02 AC 3); `&` and `*x` are `E0101` (value-semantics C1).

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
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-repository-guards --test suite suite_hygiene::`

**C95** - Phase A forms in a `const`, a `const fn`, a param default, a `where` and a generation range evaluate to their run-time value. (CV-36, AC 1; T27)
Proof: run elabctl | test phase_a_forms_fold_in_const_contexts ... ok

**C96** - Aggregates and the stdlib's `poly_q`, `den_q`, `conjugate_closed` fold in const contexts. (CV-36, AC 1; T55)
Proof: run elabagg | test aggregates_fold_in_const_contexts ... ok

**C97** - A panic in a const evaluation is an elaboration error naming the expression. (CV-36, AC 2; T27)
Proof: run elabctl | test a_panic_in_a_const_evaluation_names_the_expression ... ok

**C98** - `GAPS.md` no longer lists L5, L6, L9 (but its `generics` row) and L10, and §9 is updated. (CV-37, AC 1; T29) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C99** - `GAPS.md` no longer lists L1 to L3, L4 (but generic enums), the H.5 to H.7 rows, `normal` and the §4.7 enum-names note. (CV-37, AC 1; T63) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

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

**C105** - An enum without `#[derive(Repr)]` is a value type (built, matched, passed and returned), and a carrier of it is `E0112`. (CV-11, AC 1; finding 3)
Proof: run typesagg | test an_enum_without_repr_is_a_value_but_not_a_carried_type ... ok

**C106** - The Book's `area` over `Shape`, an enum with payload variants and no `Repr`, runs. (CV-11, AC 1; finding 3)
Proof: run booktypes | test an_enum_without_repr_runs_as_a_value ... ok

**C107** - A match arm over an `Option` takes `Some(1)`, `Some(2) | Some(3)`, a guard, `_`, `None`, a bound name (`whole @ Some(7)`) and a nested pattern as Rust does. (CV-04, AC 1 and AC 4; finding 4)
Proof: run srcctl | test option_arms_take_rusts_patterns ... ok

**C108** - A plain read of a composite carrier that holds no value yet fails the run naming the carrier; `u.prev()` before the first change is unknown. (CV-17, §3.3.2, §5.1.1, §5.1.8; finding 2)
Proof: run srcagg | test an_unknown_composite_is_refused_by_name ... ok

**C109** - Generic arguments written on a struct or enum with no generic parameter are `P0001` by name, never ignored. (CV-09, CV-11; found by the merge)
Proof: run typesagg | test generic_arguments_on_a_plain_type_are_refused ... ok

**C110** - A `match` or an `if let` whose value is an `Option` (arms by key, a guard over an `Option`, `if let Some(n) = o { Some(n + 1) } else { None }`) takes the arm Rust takes, flag and payload both. (CV-04, AC 1 and AC 4; GAPS L66, found by finding 6)
Proof: run srcctl | test a_match_or_if_let_yielding_an_option_runs_as_rust ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (2026-11-20, batch B1, branch `feature/composite-values` at `c720b421`, already on `origin/main` `4f505e09` with `analog-structure` merged: no rebase needed, `**Build**` line green and zero warnings at that head). Anchors re-read at the new `HEAD`; everything design.md names exists, these moved or are not what the design calls them: (1) `Ir::check`'s recursion check is `Checker::recursion` in `check.rs` (was `check.rs:1052`) and `deps.rs`'s recursion refusal is `Walk::enter` (was `deps.rs:852`) - both T5; (2) the checker's refusals moved to `types/unsupported.rs` plus `types/call.rs:334` (`closure`, was `call.rs:333`) and `types/expr.rs` - T11 to T18 read them by name, not line; (3) `Known` is not a type: the `known_or`/`is_known`/`high` lowering is `Builtin::Known*` in `elab/known.rs` and the known/unknown decision is the `Flagged` type of the condition - T19 to T24 and T22 key on `ValueType::Flagged`; (4) `Frame` is not in `elab/structural.rs`: the carrier `Frame` is `elab/carriers.rs:48`, the placement frame is the `frame: usize` index of `elab/hoist.rs` (`hoisted_name`, `hoist`, `hoist_placements`) - T25 and T26 read `hoist.rs` and `structural.rs` again before editing; (5) `rt::Merge` is a trait (`rt/flag.rs:325`), not a struct, and `Carried::to_levels` is the trait method at `rt/flag.rs:38` - T6, T7, T36; (6) `ValuePrinter::merged` is `print/value.rs:1038`, `holds_effect` `:1057`, `block_lines` `:1085`; `Commits::branch` `rt/commits.rs:325`, `run` `:346`; `CarrierLayout` `plan/digital.rs:160`, `RecordLayout` `:252`; `Metadata` `api/metadata.rs:47`, `Cell` `api/table.rs:18` (all as design.md, line numbers only drifted); (7) the next free `MD-` number is `MD-73` (`MD-72` is taken by "the root package holds the integrated tests", STATE.md still says "MD-72 is the next free one" in a stale line) - T62. B1 note (T2): `crates/piperine-ir/tests/{model,print}.rs` read every file of `src/` flat; the new `deps/control.rs` needs a directory, so both now go through `tests/support/sources.rs` (`SourceTree`, recursive: a strengthening, not a weakening of the same assertions). `Local` gained `mutable`; every literal of `Local` in the tree (about 35 sites in tests and in `elab`) now writes `mutable: false`, no behaviour change. The interpreter refusal is `piperine-interp/src/composite.rs` (`CompositeForms`), one walk of everything a body evaluates, wired in `Interpreter::new` beside `amended_forms` and `structure_forms`; `Expr::operands`/`Expr::statements` (`model.rs`) are the one operand accessor the new walkers use. B1 defaults (T3 to T5, taken while the user is away, all within design.md): `Ir::check` refuses a contribution inside a loop (T25 unrolls it before the IR, section 11 rule 12), a `return` inside a closure (a closure `return` returns from the closure; T16 and T23 lift it when closures carry control) and an assignment with a field path (no aggregate type before T30 and T31); a `FunctionRef` counts as a call edge for the recursion and `analog`-reachability check, and the dependency walk takes it as a call with unknown arguments, so a call through a value is `Nonlinear`; the existing `check.rs` test `a_recursive_call_or_an_unbound_local_is_named` is NOT moved: its recursive functions are reached from the `analog` addend, so it still passes as written, and `recursion_reached_from_analog_is_refused` (`tests/control.rs`) is the acceptance test of the new rule beside it; `Restore` in `deps.rs` now keeps what a block assigned to an outer local (it only unbinds the locals the block bound), and `done` does not memoize an `if`, a block with statements or a loop. Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("Tu sempre assume o default mais razoável e depois me conta"); the defaults are design.md D1 to D20. B2 settled items (T6 to T10, batch B2): (1) `Step::Fall(S)` and `Step::then` were added beyond T7 (a split inside an iteration runs the rest of the iteration on every fall-through path). (2) `Unrolled::run` takes the origin first; the struct is `Unrolled<S,B,R>`. (3) The 64-iteration cut counts iterations after the first unknown decision; always-known loops are bounded only by `Trips` (2^24). (4) `Trips` panics on iteration 2^24+1; `Depth` allows 1024 nested calls and panics on the 1025th. (5) A return inside an unknown loop: all paths return gives that value; some paths gives `R::unknown()` (a sound over-approximation). (6) `Commits::unrolled` in `rt/commits.rs`: commits up to and including the first-unknown iteration are definite, later ones MAYBE|MERGE. (7) A compound assignment prints as `x = x op e`; `ValuePrinter::operator` was extracted from `binary`. (8) `for` over rt integers counts raw `.get()` values and rebinds the loop local. (9) `Ir::written_locals` (`piperine-ir/src/written.rs`) and `Ir::expressions()` were added. (10) Designs with a loop or a recursive fn get the full rt files (`Transpiler::loops()` ORed with `digital()`). (11) Unknown-control blocks sit under `#[allow(unused_mut, unused_assignments, unreachable_code)]` on a `let`. (12) A fn type prints `&dyn Fn(A) -> R`; recursive fns take a trailing `depth_: rt::Depth` and cycle calls pass `depth_.deeper("name")`. Known refusals, failing loud by name: a for-loop with an exit under an unknown decision (T20 desugars to `while`); break/continue/return under an unknown `if` outside its loop's statements; written locals of non-maybe type; a loop value that cannot be unknown; more than 11 written locals; a fn returning a fn value. B3 settled items (T11 to T18, batch B3): new `types/` files assign, loops, exits, flow, patterns, cover, class, closures, and `Ty::Never`; there is no `Ty::Closure`: a closure's type is `Ty::Fn` with a `ClosureInfo` side table keyed by the closure expression, and a call through a closure-bound local is refused by name until T23; only the solve-reading and emitting function classes are inferred by reachability (the event class is not inferred); E0122 applies to closure calls, named fns keep E0601; a capturing closure coerced to a fn pointer is P0001 naming the form; a non-exhaustive match is E0184 naming one missing pattern (`Covers`/`Coverage::missing` in `cover.rs`, integer interval cover at any width); binders at any pattern depth, `match` over `f64` is typed; the immutable deferred `let x;` at-most-once check covers `while`/`for` bodies but not `loop` bodies; a module-body closure `let` is not deferred (an unannotated parameter is E0181); flow state is per body (§1.8.12); a `match` arm guard is refused by name in elab until T22 (`a5107a6c`); Book rows digital#6 and from-rust#1 no longer pin `let mut` / range-pattern refusals (`c893df43`); the elaborator's fallback refusal names the expression form. Open for later: non-Copy moves (§1.4.7) at T44; duplicate names inside tuple patterns at T40. B4 settled items (T19 to T24, batch B4): (1) flagged `let mut`: in unknown-capable code a let-mut scalar is always Flagged; a known assignment becomes `{ let t = v; merge(t, t) }`; plain code keeps plain locals (replaces the "fixed point" wording). (2) `let x;` lowers to an assignable IR local starting at zero (Unknown where flagged). (3) `for` stays ForRange unless bounds may be unknown or the body exits early in unknown-capable code, then a counting `while` taking the next value before the body; only exclusive ranges; `..=` there is refused by name; `(a..b).rev()` is not typed by the B3 checker so not lowered. (4) break/continue/return are typed `()` and rebuilt at the needed type via elab/exits.rs; a `loop` without break fits any type. (5) An Option-returning fn lowers to two IR functions `f.is`/`f.val` until T32; `e?` = `if e.is { e.val } else { return None }`; half-locals are spelled `_name`. (6) Fns with return/? get the signature result type, flagged where unknown-capable. (7) Closures copy read mutable locals at definition; a call through a fn-typed local is CallValue; a flagged argument only via a local bound to a fn name (calls the flagged variant), else P0001 naming the call; a closure type is recomputed from its body. (8) Recursion: the recursive call takes the declared result; the body is re-lowered once if the result differs. (9) Guards: pattern binds become lets before the first test; unguarded arms keep the body-block shape; a site under a guarded arm is refused via ArmCondition::Guarded; a guarded last arm is refused. (10) Contributions inside loops are refused by name until T25. Printer fixes: print/analog.rs keeps a top-level Assign and a loop/if Stmt::Expr writing a later-read local; print/value.rs puts `#[allow(unused_assignments)]` on every let mut. New modules elab/patterns.rs, closures.rs, returns.rs. Open gaps: rt::UInt is not emitted in an analog-only design (integer arithmetic there fails to compile); the checker refuses f64 %; recursion reached from analog fails at Ir::check as P0004 until T26; the runaway-loop fault names the loop by source place. B5 settled items (T25 to T29, batch B5): (1) L9 narrows to the capturing closure passed to a user `fn` (generics GN-01); the Book row analog#2 follows L58 and from-rust#1 follows L4. (2) Placement names in unrolled loops are numbered after the non-loop placements, in unrolled order. (3) A generation `for` is not elaborated (the checker refuses it); the same fold entry point serves it later. (4) Refused by name: `break`/`continue` inside an unrolled analog loop (the message says "outside a loop"), `?`/`Option` and tuple/struct patterns in const evaluation, a recursive `fn` reached from `analog` with an early return or an `Option` result. (5) `ConstFault::Leaves` and a `pub Exit` were added; the const call depth limit is 1024, not 256 (E0104). (6) T26 recursion needs constants or structural parameters (an ordinary module parameter depth is E0126). (7) `a_panic_in_a_const_evaluation_names_the_expression` takes about 22 s in debug. B6 settled items (T30 to T37, batch B6): (1) slices exist only as call arguments and method receivers (the length of a slice is its receiver's). (2) An array of length 0 is accepted in the IR. (3) An `Option` constant carries 0 or 1 parts. (4) A derive requires the trait on every field. (5) A zero-level tag exists only for a one-variant enum (§2 rule 5). (6) An aggregate comparison in code generic over `R` is refused by name. (7) A maybe-unknown index needs flagged elements, else it is refused by name. (8) A composite pin's byte layout is the parts' value bytes in order, then one trailing known byte (T56 and T57 may lay it out per leaf). (9) Arrays are `Carried` up to 32 elements (Rust's `Default` limit). (10) A `Repr` type must derive `Clone`, `Copy` and `PartialEq`, else it is refused by name. (11) Enums get a generated `Default` (the first variant). (12) The plan, the netlist, the host param table and `constant_expr` panic with a message on a composite constant, because the elaborator folds none yet. (13) A non-exhaustive `match` moved to `E0184` and an assignment to an immutable local to `E0183`. (14) The "outside a loop" wording fix was not applied. B7 settled items (T38 to T42, batch B7): T42 narrowed / T43 dropped by the value-semantics decision, 2026-11-20 (no borrows, slices, `&[T]`, Copy requirement or moves: `Ty::Slice`, `Ty::Ref`, `E0102` and the borrow expressions were never committed; a range index `e[a..b]` is refused by name as a slice). (1) §1.4.4 literal inference landed first, as its own commit: a local bound to a bare integer literal stays open until a use fixes it; Book `language#13` runs and GAPS L63 is gone. Left over: a read that requires no type closes the local at `i32`, a negative literal initializer is not tracked, a literal above `i32::MAX` keeps its old behaviour. (2) The checker folds array lengths and discriminants itself (`types/lengths.rs`: literals, `const` names, integer arithmetic), because `ConstEval` reads the finished type table. (3) An unknown field of a struct literal or a struct pattern is `E0186` (pom classifier, §1.3.3; row added to `docs/pom/resolution.md`). (4) An enum still needs `#[derive(Repr)]` to be typed (T46 relaxes it); a payload enum is kept out of carriers by `Checker::repr_ok` until T46; `Ty::levels` ignores `#[repr(uN)]` until T46. (5) A bundle (a struct with a carrier field) stays P0001 as a value type. (6) Coverage is a row judgement over constructors (`types/cover.rs`); `for` over an array is not typed yet (T45 H.7).
B9 settled items (T52 to T55, batch B9): (1) T52: a comparison of two `Option`s is built from the halves in `elab/derived.rs` (`==` is `a.is == b.is && (!a.is || a.val == b.val)`; `<` is `(!a.is && b.is) || (a.is && b.is && a.val < b.val)`, `<=` adds `==`, `>`/`>=` swap); an `Option` that may be unknown is refused by name. (2) `Option::<T>::default()` is accepted (owner `Option` with its turbofish, `types/derives.rs::option_owner_type`) and is `None`; a bare `Option::default()` is refused by name; `T::default()` of a struct, an enum or a scalar is `Expr::Default`. (3) `rt::Real` is now `PartialEq + PartialOrd` and `Dual` compares by value alone (a tangent is not part of the number), so a derived comparison of an aggregate that holds a real prints in code generic over the real (the B6 refusal of item 6 is lifted for that case). (4) A function whose only real is in an aggregate built and compared in an expression (no local, parameter or result holds it) is generic over the real (`ValuePrinter::computes_real_value`).(5) T53: `elab/iterators.rs` writes a chain out as one counting loop over the positions of its source (an array's `a[j]`, a range's `low + j`): `map`, `enumerate`, `zip` and `rev` are functions of the position, `filter` is a guard the rest of the body sits under, the consumers (`fold`, `sum`, `count`, `any`, `all`, `contains`) keep an accumulator and `any`, `all` and `contains` `break` at the first decider; `for` over an array, a range with adaptors or a chain is the same loop (the plain `a..b` keeps its `ForRange`). (6) A `let` of an iterator or a range binds no value: the name stands for the chain where it is read (`scope.let_sources`). (7) Refused by name: a `zip` or `rev` of a filtered chain, an iterator over values that may be unknown, a closure argument that is not a literal, a closure parameter other than a name, `_` or a tuple of them, `is_empty` of a range with a missing bound, a bare range `..` source. (8) `len`, `first`, `last` and `get` of an array use the type's length (`Length`, a constant `Index`, `i < N` guarding the element); `first` of an array of length 0 is `None`. (9) An empty array prints as `<[T; 0]>::default()` (no element names its type). (10) The `zi_nd` reaction body's value code needed no elaborator change: arrays, `for k in 1..N` and indexing already ran (T51); the test pins it with fixed `N = 3`, `D = 2`.(11) T54: `normal` needed no stdlib or elaborator change (tuples, `ln`, `cos` and the `u64` wrapping arithmetic ran as they were); the jitter clock keeps its state in a `Bit<u64>` read as `rng.known_or(7)` (a tuple part that may be unknown is refused by name, B8 item 8, so the spec fragment's bare `rng` is written with `known_or`), and a module param of type `u64` is `P0001` (not in this feature), so the seed is a literal in the model. The solver rounds each `after` delay to the nearest tick when it schedules it, so the expected edge times in the test accumulate the rounded delays. (12) A call of a function that computes a real in an aggregate, in reaction code, names `::<f64>` even when the result is an `f64` (`Flagged::known(f(x))` leaves the real open).(13) T55: the evaluator holds a composite as an entry of its own arena (`consts/composite.rs`), named in a `Folded` by a `Constant::Aggregate` handle that means nothing outside it; the public `folded`, `value`, `constant` and `folded_constant` refuse one (`P0001` naming it) and `ConstEval::tree`/`constant_tree` give it out as a `ConstTree` that `elab/consts/imported.rs` turns into an IR `Constant::Aggregate` (a param default, via `fold_param_value`) or into the expression that builds it (a composite `const` read in value code, `lower_path`). A composite param or value input bound from outside an evaluation is refused by name (its handle is an IR one; `generics` and phase C wire it). (14) Iterator adaptors in a const context run their closures on every element when they are called (an adaptor is eager there), a consumer (`any`, `all`, `contains`) stops at the element that decides; the shape H.7 rule 4 gives (a closure runs when asked) differs only in a closure that panics on an element a later stop would have skipped. (15) A unit variant of an enum with no field in any variant stays its tag (`Constant::Int`); an enum with a field in some variant holds every variant with its parts. (16) A range index bound written as a literal or a const name has no type of its own and reads through `Types::integer_constant`. (17) The stdlib's `poly_q`, `den_q` and `conjugate_closed` are generic over `const N`, which the checker refuses (`generics`): the test folds their bodies, read from `std/analog.msl`, at a fixed length. (18) A composite param default now elaborates, so `plan.rs`, `netlist.rs` and the host param table, which panic with a message on a composite constant, are reachable from source: T56 and T57 (B10) give them their layout.(19) Batch-end gates: the Book's `tests/book/types.rs` pinned types#7's `smoothed` over the table `TAPS` as refused (`this form in a const expression in a const`); it runs since T55, so that row moved to the new `a_table_const_runs_where_a_top_reaches_it` (2.0 = 0.25*1 + 0.5*2 + 0.25*3) and `mean(TAPS)` stays refused (generic over `const N`).
B10 settled items (T56 to T63, batch B10): (1) T56: a composite pin is the runtime's `Carried` bytes: the parts' value bytes one after another, unpadded, first lowest, then one known byte (`CompositeLayout`, alignment 1, size = width + 1); one ABI field per leaf, every field's known offset the one known byte; an array is one field per element (`num[2]`), not a `count` field, because one known byte cannot stand for `count` known bytes; a payload enum's variants overlap after the tag (`Data.n`), a one-variant enum has no `tag`; `PinLayout` is a carrier's scalar layout or a composite's and `RecordLayout.fields` holds `PinLayout`; the netlist passes a composite param as `ParamValue::Bytes`. (2) T57: the IR accepts a composite carrier (`Ir::check` `carriable`), a read of a tuple, array, struct or `Option` carrier is the value (an unknown one reads as the default, printed `.known_or(Default::default())`), and the receiver of a flag read (`known_or`, `is_known`, `level`) takes the carrier's flagged read (`Typing::read_of`, `lower_flag_receiver`); a payload enum is `Flagged(Enum)` as before; the flag reads and `cast` take any `Repr` type except `Option` (`types_unknowns` pins `Some(1.0).level(0)` as refused); `T::WIDTH` is a `constant_form`; fix found: `Flagged::recast` compared the whole word with the tags, now the lowest `tag_levels` levels. (3) T58: a struct param needs `#[derive(Repr, PartialEq)]` (its pin decodes through `Carried`); `Bindings::bind_composite` carries the tree the evaluator reads (`where`, values over the param); `Elaboration::fold_param_value` folds a placement value over the parent's params, the instance table keeps the constant; a composite with a real is lifted to the block's real in analog code (`print/lift.rs`); a composite param derived from the parent's params keeps its elaborated value (no derive function; a host `set` that would recompute it is refused by name); a plain `u8` param stays `P0001` (params-and-io). (4) T59: `ParamTarget` (a param or one leaf), the leaf encoded in its layout's bytes, the whole value's `where` checked before the write (`E0317`, §23.1's `E0802` stays for a scalar), a timed leaf write is `E0806`, `--param` reads `true` and `false`. (5) T60: an `emit!` field of a composite type is a stream field (`Ir::check` `carriable`), an `f64` field is the 16 bytes of `REAL`; `Ir::written` spells a type as written and `Metadata` uses it for stream fields and params; `E0605` already covered composite conflicts. (6) T61 and T62: `Probe::Shaped`/`Column::Shaped` for a net holding an enum, tuple, array, struct or `Option`; an enum with no payload is `Cell::Text`/`Field::Variant` (bare name), any other composite `Cell::Composite`/`Field::Composite`; leaf names are the layout's (`pair.a`, `v[1]`, `tx.Data.n`, an enum's variant column the bare column name, an `Option` `some` and `value`); CSV takes the union of leaves over the rows, an absent one empty; the plot keeps numeric leaves. Moved golden lines: the Book's levels (types#8, listing types#10) and monitor (values#10, listing values#13) printed an enum field as its tag and print its variant name; the two claims became types~a1ff299b and values~4f830aa0 (`tests/book/inventory.tsv` follows), and `docs.rs` pins moved with the GAPS counts (32 to 28 loud, L3 left the index-merge rows, L16 and `normal` left). (7) T63: `GAPS.md` keeps L4 for generic enums and for an enum used by value code without `#[derive(Repr)]` (owner `generics`); the Book rows types#7 and generics#4 wait on generic items (L7); MD-79 is `composite-values`' decision (MD-78 is `explicit-unknowns`'s). For `explicit-unknowns` phase A (the hidden known flag): the composite pin's one known byte (`CompositeLayout`, `CellShape::decode`, `Params` bytes), `Typing::read_of` and `lower_flag_receiver` (a flagged read of a composite carrier), the printer's `.known_or(Default::default())` for a plain composite read, `Flagged::recast`'s tag check and `a_tag_naming_no_variant_reads_unknown` (a payload enum cast is `Flagged(Enum)`) all read or write that flag and are the places it reaches. Left over: a composite param holding a real reaches analog code only through `Dual<N>` call sites (`RealLift`), an `Option` param, an array-of-`Option` literal and a derived composite param that a host `set` should recompute.
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

**Later batch boundary (not tasked here):** phase C (CV-23 to CV-28, ROADMAP item 11) runs after `generics` A–B, from `design-c.md`/`tasks-c.md` (design.md "Phase C"). Planned 2026-11-23: `tasks-c.md`, T64 to T90 in batches C1 to C4, checks C111 to C145.

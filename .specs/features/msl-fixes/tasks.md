# MSL Fixes Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-fixes/design.md`
**Status**: Approved
**Approved**: 2026-11-17 - user: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando"
**Build**: `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piperine-parity -p piperine-solver -p piperine-analog-solver -p piperine-api -p piperine -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run tmod**: `cargo test -p piperine-lang --test types_modules`
**Run tvalues**: `cargo test -p piperine-lang --test types_values`
**Run tint**: `cargo test -p piperine-lang --test types_integers`
**Run tcomp**: `cargo test -p piperine-lang --test types_compound`
**Run tevents**: `cargo test -p piperine-lang --test types_events`
**Run tanalog**: `cargo test -p piperine-lang --test types_analog`
**Run evalues**: `cargo test -p piperine-lang --test elab_values`
**Run eanalog**: `cargo test -p piperine-lang --test elab_analog`
**Run edig**: `cargo test -p piperine-lang --test elab_digital`
**Run iramend**: `cargo test -p piperine-ir --test amendments`
**Run plan**: `cargo test -p piperine-plan --test amended_shape`
**Run frozen**: `cargo test -p piperine-interp --test frozen`
**Run tlib**: `cargo test -p piperine-transpile --lib`
**Run watch**: `cargo test -p piperine-analog-solver --test assembly_watch`
**Run env**: `cargo test -p piperine-solver --test environment`
**Run sfix**: `cargo test -p piperine-parity --test source_fixes`
**Run scat**: `cargo test -p piperine-parity --test source_catalogue`
**Run senv**: `cargo test -p piperine-parity --test source_environment`
**Run records**: `cargo test -p piperine-api --test records`
**Run apilib**: `cargo test -p piperine-api --lib`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run clicmd**: `cargo test -p piperine-cli --test commands`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`

Executors: every task is Sonnet (user rule: "Implementação sempre em Sonnet e verificação em Opus"); a different Opus verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources.

The tasks follow the answers of the spec's Q1 to Q4, all (a) (user, 2026-11-17: "Aceito todas (Recommended)"). For the record, another answer would have changed: Q1 (b) adds a phase for S3; Q2 (b) turns T23 and T24 into one elaboration refusal and drops FIX-05 AC 2 to 4; Q3 (b) drops T35 and the analysis half of T33, T34 and T36; Q4 (b) adds conditional `bound_step` to T34 and T36.

Rules every task follows:

- **Parity is untouched.** `crates/piperine-parity/src/contract.rs`, `tests/ngspice/`, the `parity_*` targets, `crates/piperine-cli/tests/goldens/` and `crates/piperine-transpile/tests/golden/` are not edited. No tolerance is written outside `ToleranceContract`, and none is changed. A task whose change moves a twin's bytes or a generated golden stops and reports.
- **Cross-lane touches are only those `design.md` names**: `piperine-analog-solver/src/device/watch.rs` (T23), `piperine-solver/src/analyses/{dc,ac,noise,transient}.rs` (T35, the operating point's flag only), `piperine-api/src/{session,recorder,verdict}.rs` (T30, T31), `piperine-cli/src/commands/{run,test}.rs` (T32). Before T30, `git fetch` and rebase on `main` if `host-ii` has merged. No other file of `piperine-api`, `piperine-cli`, `piperine-solver`, `piperine-analog-solver`, `piperine-digital-solver` or `piperine-abi` is edited.
- **The interpreter is frozen (R1).** `piperine-interp` gains refusal arms only (T12); none of its existing targets is edited except `frozen.rs`, which gains tests.
- **Fail loud.** A form this feature does not run is `P0001` naming it, or the spec's code; never a default, a skipped statement or a value of 0.
- **Tests state what the spec says, not what the code does.** Expected values come from `docs/spec/` (Appendix H rows with Rust's own `f64`/integer methods as the oracle, H rule 3), the spec's criteria, or a hand derivation written beside the assertion. Test functions have exactly the names `## Checks` cites.
- **No existing test is removed, weakened or skipped.** A test that asserts a refusal this feature lifts (for example a `P0001` for `sin` or for a second `analog` block) is rewritten to assert the new behavior, and its commit says which test and why.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, modules named after what they do, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, ending with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- **`lessons.py` is not run**: its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`. If it is run, `git status` afterwards and restore both files.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 placement), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits and their guards).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Type checker (`piperine-lang/src/types`) | integration | 1:1 to the spec's ACs of typing and refusal; each code (`E0140`, `E0141`, `E0161`, `E0318`, `P0001`, `P0002`) named | `crates/piperine-lang/tests/types_*.rs` | `cargo test -p piperine-lang --test <target>` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; every refusal by name | `crates/piperine-lang/tests/elab_*.rs` | `cargo test -p piperine-lang --test <target>` |
| IR (`piperine-ir`) | integration | each new form typed, checked and printed | `crates/piperine-ir/tests/amendments.rs` | `cargo test -p piperine-ir --test amendments` |
| Plan (`piperine-plan`) | integration | each new pin, watch and read in the shape | `crates/piperine-plan/tests/amended_shape.rs` | `cargo test -p piperine-plan --test amended_shape` |
| Runtime and printer (`piperine-transpile/src/rt`, `print`) | unit | every row's value against Rust's method; every derivative against finite differences | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-transpile --lib` |
| Interpreter (`piperine-interp`) | integration | one refusal per new form | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Solver touches | integration | the one behavior each touch adds; `parity_baseline` unedited | `crates/piperine-analog-solver/tests/assembly_watch.rs`, `crates/piperine-solver/tests/environment.rs` | the `Run watch` and `Run env` lines |
| Runs from source (`piperine-parity` `Project`) | e2e | every runtime AC on the Rust backend, values from the spec | `crates/piperine-parity/tests/source_*.rs` | `cargo test -p piperine-parity --test <target>` |
| Host library (`piperine-api`) | integration | FIX-02 AC 1 and 5 | `crates/piperine-api/tests/records.rs`, inline | the `Run records` and `Run apilib` lines |
| Command line and the Book (`piperine-cli`, `docs/book`) | e2e | the real binary; every listing equals its output | `crates/piperine-cli/tests/{run,commands,book}.rs` | the `Run clirun`, `Run clicmd`, `Run clibook` lines |
| Records (`GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`) | integration | each claim of DOC-01 read as text | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| Feature records (`contract.md`) | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes generated code or what a run prints | Quick, then `cargo test -p <the crate the task edits>`, then the `Run twins` line and `cargo test -p piperine-transpile --test golden` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Six batches: phases 1 to 5 hold 7 tasks each, phase 6 holds 4.

### Phase 1: Contract and front-end fixes

```
T1 → T2
T1 → T3
T1 → T4
T1 → T5 → T6
T5 → T7
```

### Phase 2: IR, plan and runtime

```
T1 → T8 → T9 → T10 → T11
T9 → T12
T1 → T13 → T14
```

### Phase 3: The catalogue

```
T1 → T15 → T16 → T17
T5 → T17
T8 → T18
T15 → T18
T16 → T18
T9 → T19
T17 → T19
T13 → T20
T14 → T20
T18 → T20
T19 → T20
T20 → T21
```

### Phase 4: Fixes through the backend

```
T3 → T22
T1 → T23 → T24
T22 → T24
T7 → T25
T9 → T25
T11 → T26
T25 → T26
T24 → T26
T9 → T27
T11 → T27
T27 → T28
T26 → T28
```

### Phase 5: `finish`, and the environment through the front end

```
T28 → T29 → T30 → T31 → T32
T7 → T33 → T34
T9 → T34
T10 → T34
T1 → T35
```

### Phase 6: The environment runs, the Book and the records

```
T34 → T36
T35 → T36
T11 → T36
T21 → T37
T36 → T37
T37 → T38 → T39
```

---

## Task Breakdown

### T1: Contract: the cited code and the solver facts the design rests on

**What**: `contract.md`: confirm every file, line and name `design.md` cites against this task's `HEAD`; re-run each probe of `design.md` "What the code does today" and record its output; confirm on the solver, with a short throwaway run (not committed), three facts the design rests on: (1) a reaction may read an event pin's `CHANGED` byte (C12); (2) the `DC`, `TRAN`, `AC`, `NOISE` globals are bound to a body that declares them, and an AC run's operating point builds its side with `global::DC` today (C10); (3) a `CROSS` watch of NaN fails at `device/watch.rs:100` today (C8). Each difference gets the task that absorbs it; a fact that does not hold is reported to the orchestrator before phase 2.
**Where**: `.specs/features/msl-fixes/contract.md`
**Depends on**: None
**Reuses**: `design.md`; the probes of `GAPS.md`
**Requirement**: FIX-01, FIX-05, FIX-11, FIX-13
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists each cited location as confirmed or moved, each probe with its output, and the three solver facts with their evidence
- [x] No code is edited
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): msl-fixes contract against the tree`

---

### T2: A `where` that reads a later param is `E0318`

**What**: `Checker::param` reports `E0318` naming the later param when a `where` reads a param declared after its own (design C2).
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T1
**Reuses**: `check_as(.., "E0318")` of the same function
**Requirement**: FIX-03
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_modules`: `a_where_that_reads_a_later_param_is_e0318` (both `d = 0.0` and `d = 5.0`), `a_where_over_its_own_and_earlier_params_is_accepted`
- [x] Gate check passes: Quick (`Run tmod`), then `cargo test -p piperine-lang`
- [x] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a where that reads a later param is E0318`

---

### T3: Enums that derive `PartialOrd` compare in declaration order

**What**: `binary` gives `<`, `<=`, `>`, `>=` an enum arm requiring `PartialOrd` (`P0002` otherwise), lowered as the comparison of the tags (design C4). Creates the e2e target `crates/piperine-parity/tests/source_fixes.rs` (`//!` header: correctness fixes run from source on the Rust backend, one test per criterion).
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T1
**Reuses**: the `PartialEq` arm of `==`; `piperine_parity::project::Project` as `tests/project.rs` uses it
**Requirement**: FIX-07
**Executor**: Sonnet

**Done when**:

- [x] `comparing_an_enum_without_partial_ord_is_p0002` passes in `cargo test -p piperine-lang --test types_values`
- [x] `an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction` passes in `cargo test -p piperine-parity --test source_fixes` (`Lo < Hi` true, `Hi <= Lo` false, read from committed `Bit<bool>` carriers)
- [x] Gate check passes: Quick (`Run tvalues`, `Run sfix`)
- [x] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `fix(types): enums that derive PartialOrd compare in declaration order`

---

### T4: Several `analog` blocks act as one

**What**: `elab/analog.rs` lowers every `analog` block of a module in source order into the one analog body, each in a fresh local scope; the `SecondAnalogBlock` refusal goes (design C5).
**Where**: `crates/piperine-lang/src/elab/analog.rs`
**Depends on**: T1
**Reuses**: the lowering of one block; `elab/unlowered.rs`
**Requirement**: FIX-08
**Executor**: Sonnet

**Done when**:

- [x] `two_analog_blocks_keep_their_locals_apart` passes in `cargo test -p piperine-lang --test elab_analog`
- [x] `two_analog_blocks_act_as_one` passes in `cargo test -p piperine-parity --test source_fixes` (two `i(gnd, a) +> 0.5m;` into 1 kΩ: `v(a)` = 1.0 V)
- [x] An existing test asserting the `P0001` of a second block, if any, is rewritten to the new behavior and named in the commit
- [x] Gate check passes: Quick (`Run eanalog`, `Run sfix`), then `cargo test -p piperine-lang`
- [x] Test count: the 2 tests named above

**Tests**: e2e
**Gate**: quick

**Commit**: `feat(elab): several analog blocks of one module act as one`

---

### T5: The §3.2 binder of arguments

**What**: `Checker::apply` binds positional, left-out and named arguments to parameters as §3.2 says (optional run, `Some` wrapping, `E0140`, `E0141`) and stores the binding per call in `Types` (design C3).
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T1
**Reuses**: `check_argument`; the core primitives' own optional handling in `types/analog.rs:192` as the pattern
**Requirement**: FIX-06
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `a_left_out_optional_argument_types`, `a_named_optional_argument_types` (also `f(1.0, z: 3.0, y: 2.0)`), `a_named_argument_that_is_not_optional_or_bound_is_e0140`, `a_wrong_argument_count_is_e0141`, `an_option_before_a_required_parameter_is_positional_only`
- [x] An existing test asserting `P0002` for an argument count is rewritten to `E0141` and named in the commit
- [x] Gate check passes: Quick (`Run tvalues`), then `cargo test -p piperine-lang`
- [x] Test count: the 5 tests named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): optional and named arguments bind as section 3.2 says`

---

### T6: `Option` parameters lower as pairs

**What**: An `Option<T>` parameter of a function lowers to the parameters `name.is` and `name.val`; a call lowers its binding slots in parameter order: left out as `(false, default)`, a wrapped `T` as `(true, v)`, an `Option` argument as its pair (design C3).
**Where**: `crates/piperine-lang/src/elab/functions.rs`
**Depends on**: T5
**Reuses**: `lower_option`, `lower_option_let` (`elab/compound.rs`)
**Requirement**: FIX-06
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`, each with `f` of spec FIX-06: `left_out_optional_arguments_are_none` (111.0), `a_named_optional_argument_binds_by_name` (14.0 and 6.0 for the edge case), `a_plain_value_for_an_option_is_some` (103.0), `none_and_some_pass_positionally` (14.0)
- [x] Gate check passes: Quick (`Run sfix`), then `cargo test -p piperine-lang`
- [x] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `feat(elab): Option parameters of a function lower as pairs`

---

### T7: `cross` takes `ttol` and `etol`, positionally and by name

**What**: The core-primitive call checker uses T5's binder: `cross(e, ttol, etol)` with named arguments; the `P0001` "a ttol or etol given to cross" and "named argument of a core primitive" go; the tolerances are recorded per call for the elaborator.
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T5
**Reuses**: `CORE_ROWS` (`cross` already lists two optional `Option<f64>`); `Checker::event_arguments`
**Requirement**: FIX-12
**Executor**: Sonnet

**Done when**:

- [x] `cross_takes_ttol_and_etol_by_position_and_by_name` passes in `cargo test -p piperine-lang --test types_events`
- [x] An existing test asserting the removed refusals is rewritten and named in the commit
- [x] Gate check passes: Quick (`Run tevents`), then Build (end of phase 1)
- [x] Test count: the 1 test named above

**Tests**: integration
**Gate**: build

**Commit**: `feat(types): cross takes ttol and etol by position and by name`

---

### T8: The IR's method rows of Appendix H

**What**: `Method` gains the rows of design C6 (f64 and integer); `typing::method` gives each its arity, operand types (`Powi`: `Int(32)`; `Pow`: `UInt(32)`) and result (`bool` for `IsNan`, `IsFinite`, `IsInfinite` and the `*Overflows` rows); `print.rs` prints each by its MSL name.
**Where**: `crates/piperine-ir/src/typing.rs`
**Depends on**: T1
**Reuses**: the rows of `Min`, `Max`, `Clamp`, `Floor`
**Requirement**: CAT-01, CAT-02
**Executor**: Sonnet

**Done when**:

- [x] `the_new_method_rows_type_their_operands_and_results` passes in `cargo test -p piperine-ir --test amendments`
- [x] Gate check passes: Quick (`Run iramend`), then `cargo test -p piperine-ir`
- [x] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): the method rows of Appendix H`

---

### T9: The IR's environment reads, `BoundStep`, `Fired`, `Unwrap` and cross tolerances

**What**: `Env::Freq`, `Env::Analysis(AnalysisKind)` (appended last), `Stmt::BoundStep { site, value }`, `Expr::Fired(event)`, `Expr::Unwrap { is, value, message }`, `EventSource::Cross { ttol, etol }` (design "Data Models"); typing, `Ir::check()` and the printer for each.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T8
**Reuses**: `Env::Static`, `Stmt::Contribute` and `Expr::KnownOr` as patterns
**Requirement**: FIX-09, FIX-10, FIX-11, FIX-12, FIX-13, CAT-03
**Executor**: Sonnet

**Done when**:

- [x] `the_environment_bound_step_fired_and_unwrap_forms_print_and_check` passes in `cargo test -p piperine-ir --test amendments`
- [x] `cargo test -p piperine-ir` passes with every existing target unedited (the printed text of existing forms does not move)
- [x] Gate check passes: Quick (`Run iramend`)
- [x] Test count: the 1 test named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): environment reads, bound_step, fired, unwrap and cross tolerances`

---

### T10: The plan reads `FREQ` and the analysis globals

**What**: A body that reads `Env::Freq` reads the `FREQ` ambient in its analog functions; one that reads `Env::Analysis(k)` declares the `GLOBAL` pin of `k` (`global::DC`, `TRAN`, `AC`, `NOISE`) and reads it, as `temperature` does (design C10).
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T9
**Reuses**: `reads_temperature` and its `GLOBAL` pin (`plan.rs:388`); the `STATIC` ambient read
**Requirement**: FIX-10, FIX-11
**Executor**: Sonnet

**Done when**:

- [x] `freq_reads_the_ambient_and_analysis_reads_its_global_pin` passes in `cargo test -p piperine-plan --test amended_shape`
- [x] The shape of every body that reads neither is unchanged: `cargo test -p piperine-plan` passes with every existing target unedited
- [x] Gate check passes: Quick (`Run plan`)
- [x] Test count: the 1 test named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): freq reads the ambient, analysis reads its global pin`

---

### T11: The plan's `CAP` watches, tolerance pins and fired reads

**What**: One `CAP` watch per `BoundStep` site, its value written by the analog function; for a `Cross` with `ttol`/`etol`, a `PRIVATE` discrete `F64` pin per tolerance written by a discrete function in the parameter cone and named in the watch; an `Expr::Fired(e)` read as the `CHANGED` byte of the event's pin (design C9, C11, C12).
**Where**: `crates/piperine-plan/src/plan.rs` (modify)
**Depends on**: T10
**Reuses**: the `where` check pins (`plan.rs:396`); the `CROSS` watch of an event
**Requirement**: FIX-09, FIX-12, FIX-13
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `bound_step_is_a_cap_watch_and_cross_tolerances_are_pins`, `a_fired_read_names_the_event_pin`
- [x] Gate check passes: Quick (`Run plan`), then `cargo test -p piperine-plan`
- [x] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): cap watches, cross tolerance pins and fired reads`

---

### T12: The interpreter refuses each new form by name

**What**: Every exhaustive match of `piperine-interp` over the IR gets an arm for each form of T8 and T9 that refuses naming it (design C13); nothing else of the interpreter changes.
**Where**: `crates/piperine-interp/src/walk.rs`
**Depends on**: T9
**Reuses**: the walk's present refusal ("the IR form .. is not executed by this walk")
**Requirement**: FIX-14
**Executor**: Sonnet

**Done when**:

- [x] `the_interpreter_refuses_each_new_form_by_name` passes in `cargo test -p piperine-interp --test frozen`, one case per form, each message naming it
- [x] Every other target of `piperine-interp` passes unedited
- [x] Gate check passes: Quick (`Run frozen`), then `cargo test -p piperine-interp`
- [x] Test count: the 1 test named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(interp): refuse the new IR forms by name`

---

### T13: `Real` rows with their derivatives

**What**: `Real` and `Dual<N>` gain the f64 rows of design C6 with their derivatives; `f64`'s impl calls Rust's own method.
**Where**: `crates/piperine-transpile/src/rt/real.rs`
**Depends on**: T1
**Reuses**: `chain` and the existing `abs`/`min` impls and tests
**Requirement**: CAT-01
**Executor**: Sonnet

**Done when**:

- [x] Unit tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `rt::real::tests::the_differentiable_rows_agree_with_finite_differences` (each row at several points, central differences, the step and the bound written beside the assertion), `rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one`, `rt::real::tests::each_row_has_the_value_of_rusts_method` (bit-equal)
- [x] Gate check passes: Quick (`Run tlib`)
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(transpile): real rows of Appendix H with their derivatives`

---

### T14: Integer rows and `unwrap` in the runtime

**What**: `UInt<N>`/`Int<N>` gain `abs` and `signum` (signed), `pow` (wrapping), `rem_euclid` (panics naming it on zero), `saturating_*`, and the `*_overflows` tests; `rt::unwrap(is, val, message)` panics with Rust's `unwrap` text or the `expect` message.
**Where**: `crates/piperine-transpile/src/rt/int.rs`
**Depends on**: T13
**Reuses**: `panic_of` and the division-by-zero panic of the same file
**Requirement**: CAT-02, CAT-03
**Executor**: Sonnet

**Done when**:

- [x] Unit tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `rt::int::tests::integer_rows_wrap_saturate_and_test_overflow` (every width 1, 8, 13, 64; against Rust's `wrapping_*`, `saturating_*`, `checked_*` on the matching native type), `rt::int::tests::rem_euclid_by_zero_panics_naming_it`, `rt::int::tests::unwrap_panics_with_rusts_text_or_the_message`
- [x] Gate check passes: Quick (`Run tlib`), then Build (end of phase 2)
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(transpile): integer rows of Appendix H and unwrap`

---

### T15: The H.2 rows of `f64` in the type checker

**What**: The catalogue row carries argument and result types (design C6); `F64_METHODS` holds the 32 rows of H.2; a miss on `f64` is `E0161` naming method and type.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T1
**Reuses**: `Catalogue::method`; `types/table.rs` `Method`
**Requirement**: CAT-01
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `every_h2_row_types_with_its_signature`, `a_method_outside_h2_on_f64_is_e0161` (`x.cbrt()`)
- [x] An existing test asserting `P0001` for `sin` or another H.2 row is rewritten and named in the commit
- [x] Gate check passes: Quick (`Run tvalues`), then `cargo test -p piperine-lang`
- [x] Test count: the 2 tests named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the f64 rows of Appendix H.2`

---

### T16: The H.1 rows of the integer types in the type checker

**What**: `INTEGER_METHODS` holds the 13 rows of H.1 (`pow` takes `u32`, `checked_*` give `Option<Self>`, `abs` and `signum` on `Int` only); a miss is `E0161`.
**Where**: `crates/piperine-lang/src/types/catalogue.rs` (modify)
**Depends on**: T15
**Reuses**: T15's row type
**Requirement**: CAT-02
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_integers`: `every_h1_row_types_with_its_signature`, `abs_and_signum_on_an_unsigned_integer_are_e0161`
- [x] `types_integers`'s existing row test is extended, not edited, if it lists rows
- [x] Gate check passes: Quick (`Run tint`), then `cargo test -p piperine-lang`
- [x] Test count: the 2 tests named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the integer rows of Appendix H.1`

---

### T17: The H.3 and H.4 rows, with closure arguments

**What**: `Option` gets `unwrap`, `expect` (a string literal, else `P0001`), `unwrap_or_else`, `map`, `and_then`, `or`; `bool` gets `then` and `then_some`; a closure argument of these rows is typed with the payload as its parameter and its body's type as its result; a miss on `Option` or `bool` is `E0161`.
**Where**: `crates/piperine-lang/src/types/call.rs` (modify)
**Depends on**: T16, T5
**Reuses**: `Checker::closure`; `Catalogue::option`
**Requirement**: CAT-03
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_compound`: `every_option_and_bool_row_types_with_a_closure_or_value`, `expect_with_a_non_literal_message_is_p0001`, `a_method_outside_h3_h4_is_e0161`
- [x] Gate check passes: Quick (`Run tcomp`), then `cargo test -p piperine-lang`
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the Option and bool rows of Appendix H, with closures`

---

### T18: Numeric rows lower to IR methods

**What**: Every f64 and integer row lowers to `Expr::Method`; `checked_op` lowers to the pair `(!OpOverflows, wrapping op)` (design C6).
**Where**: `crates/piperine-lang/src/elab/lower.rs`
**Depends on**: T8, T15, T16
**Reuses**: the `Min | Max | Clamp | Floor` arm of `lower_expr`
**Requirement**: CAT-01, CAT-02
**Executor**: Sonnet

**Done when**:

- [x] `numeric_rows_lower_to_methods_and_checked_ops_to_pairs` passes in `cargo test -p piperine-lang --test elab_values`
- [x] Gate check passes: Quick (`Run evalues`), then `cargo test -p piperine-lang`
- [x] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): numeric rows of Appendix H lower to IR methods`

---

### T19: `Option` and `bool` rows lower on the pair

**What**: `lower_option_method` lowers `unwrap`/`expect` to `Unwrap`, `unwrap_or_else`, `map`, `and_then`, `or`, `then`, `then_some` as design C6 says, each closure body inlined with its parameter bound to the payload.
**Where**: `crates/piperine-lang/src/elab/compound.rs`
**Depends on**: T9, T17
**Reuses**: `lower_option`, `lower_option_let`, `lower_if_let`
**Requirement**: CAT-03
**Executor**: Sonnet

**Done when**:

- [x] `option_rows_lower_on_the_pair` passes in `cargo test -p piperine-lang --test elab_values`
- [x] Gate check passes: Quick (`Run evalues`), then `cargo test -p piperine-lang`
- [x] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): Option and bool rows lower on the pair`

---

### T20: The printer's arms for the new rows

**What**: `Printer::method` prints each row plain (on `R` or an integer) and flagged (`.map`/`.zip`/`.zip3`); `Expr::Unwrap` prints a call of `rt::unwrap`.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: T13, T14, T18, T19
**Reuses**: the arms of `Floor`, `Min`, `Clamp`
**Requirement**: CAT-01, CAT-02, CAT-03
**Executor**: Sonnet

**Done when**:

- [x] `print::value::tests::each_new_row_prints_plain_and_flagged` passes in `cargo test -p piperine-transpile --lib`
- [x] `cargo test -p piperine-transpile --test golden` passes unedited
- [x] Gate check passes: Full
- [x] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): print the method rows of Appendix H`

---

### T21: The catalogue runs from source

**What**: New target `crates/piperine-parity/tests/source_catalogue.rs` (`//!` header: Appendix H rows run from source on the Rust backend, Rust's own methods as the oracle): one design per receiver type, values read from an operating point (analog) and from committed carriers (reactions).
**Where**: `crates/piperine-parity/tests/source_catalogue.rs`
**Depends on**: T20
**Reuses**: `piperine_parity::project::Project`; `tests/project.rs`
**Requirement**: CAT-01, CAT-02, CAT-03, CAT-04
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_catalogue`: `f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction`, `a_row_over_an_unknown_value_is_unknown`, `pow_wraps`, `saturating_rows_clamp`, `checked_rows_give_none_on_overflow`, `rem_euclid_by_zero_faults_naming_it`, `signed_abs_signum_rem_euclid_follow_rust`, `unwrap_and_expect_of_some_give_the_payload`, `unwrap_of_none_faults_naming_unwrap`, `expect_of_none_faults_with_its_message`, `unwrap_or_else_evaluates_its_default_for_none_only`, `map_and_and_then_bind_the_payload`, `or_takes_the_first_some`, `then_and_then_some_follow_the_bool`, `db10_and_db20_run`, `db10_and_db20_type_check_from_the_stdlib`
- [x] Gate check passes: Quick (`Run scat`), then Build (end of phase 3)
- [x] Test count: the 16 tests named above

**Tests**: e2e
**Gate**: build

**Commit**: `test(parity): the Appendix H rows run from source`

---

### T22: B1: a contribution that reads nothing builds

**What**: The analog terms printer names the block's type when its argument list is empty (design C7); bodies with a read print as before.
**Where**: `crates/piperine-transpile/src/print/analog.rs`
**Depends on**: T3
**Reuses**: the `call` closure of `print/analog.rs:494`
**Requirement**: FIX-04
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`: `a_literal_flow_source_builds_and_runs` (1.0 V on both backends), `a_literal_potential_source_builds_and_runs` (2.0 V), `a_carrier_driven_current_builds_and_runs` (2.0 V after 1 ns)
- [ ] `cargo test -p piperine-transpile --test golden` and the `Run twins` line pass unedited
- [ ] Gate check passes: Full
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `fix(transpile): a contribution that reads nothing builds`

---

### T23: A NaN `CROSS` watch is unknown (cross-lane: `piperine-analog-solver`)

**What**: The finite check of a `CROSS` watch admits NaN and keeps refusing ±infinity; abi.md A4 gains the sentence of design C8. Nothing else of the solver changes.
**Where**: `crates/piperine-analog-solver/src/device/watch.rs`
**Depends on**: T1
**Reuses**: the existing `CROSS` comparisons (`before <= 0.0 && now > 0.0`)
**Requirement**: FIX-05
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-analog-solver --test assembly_watch`: `a_nan_cross_watch_is_unknown_and_never_fires` (NaN then positive does not fire; negative then NaN then positive does not fire at either step), `an_infinite_cross_watch_fails_the_solve`
- [ ] `cargo test -p piperine-solver --test parity_baseline --test crossing_rules` passes unedited
- [ ] Gate check passes: Quick (`Run watch`), then `cargo test -p piperine-analog-solver`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(analog-solver): a NaN cross watch is unknown`

---

### T24: B2: a `cross` over a value that may be unknown

**What**: The watch block of a `cross` whose argument is `Flagged` computes on `Flagged<R>` and its function writes `known_or(NaN)` and zero partials for an unknown result (design C8).
**Where**: `crates/piperine-transpile/src/print/cross.rs`
**Depends on**: T23, T22
**Reuses**: `Flagged::map`, `zip` (`rt/flag.rs`)
**Requirement**: FIX-05
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`: `a_cross_over_a_maybe_unknown_carrier_builds`, `absdelta_fires_every_two_tenths_of_a_volt` (records at 0, 0.2, 0.4, 0.6, 0.8 µs within the default `ttol`)
- [ ] Gate check passes: Full
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `fix(transpile): a cross over a value that may be unknown builds`

---

### T25: Cross tolerances lower to the event

**What**: The elaborator lowers the `ttol`/`etol` of a `cross` into `EventSource::Cross`; a tolerance that reads a carrier, a probe or a local is `P0001` naming "a `cross` tolerance that is not a constant or a param" (design C11).
**Where**: `crates/piperine-lang/src/elab/events.rs`
**Depends on**: T7, T9
**Reuses**: `disjuncts`; the continuity table of the same file
**Requirement**: FIX-12
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `cross_tolerances_lower_to_the_event`, `a_cross_tolerance_that_is_not_constant_is_p0001`
- [ ] Gate check passes: Quick (`Run edig`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): cross tolerances lower to the event`

---

### T26: Cross tolerances run

**What**: The transpiler emits the discrete function writing each tolerance pin and declares the watch with them (design C11).
**Where**: `crates/piperine-transpile/src/print/shape.rs`
**Depends on**: T11, T25, T24
**Reuses**: the check-pin functions of `print/site.rs`
**Requirement**: FIX-12
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`: `cross_with_tolerances_runs` (the three spellings of FIX-12 AC 1), `a_cross_with_ttol_fires_within_it` (RC charge, `ttol` 1 ns, firing within 1 ns of `RC · ln 2`)
- [ ] Gate check passes: Full
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(transpile): cross tolerances reach the watch`

---

### T27: A split head is one reaction

**What**: `lower_reaction`'s split branch becomes one reaction on every disjunct's event, its body under the guard of `Expr::Fired` reads and continuous conjuncts; the refusal over an emitting body goes (design C12).
**Where**: `crates/piperine-lang/src/elab/events.rs` (modify)
**Depends on**: T9, T11
**Reuses**: `Disjunct`, `add_event`, `reaction_body`
**Requirement**: FIX-13
**Executor**: Sonnet

**Done when**:

- [ ] `a_split_head_over_an_emitting_body_is_one_reaction` passes in `cargo test -p piperine-lang --test elab_digital`
- [ ] The existing test `an_or_head_is_one_reaction_or_one_per_disjunct`, whose second row this changes, is rewritten to the one-reaction form and named in the commit
- [ ] Gate check passes: Quick (`Run edig`), then `cargo test -p piperine-lang`
- [ ] Test count: the 1 test named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): a head split per disjunct is one reaction`

---

### T28: `Fired` in the generated reaction

**What**: The reaction printer reads `Expr::Fired(e)` as the event pin's `CHANGED` byte.
**Where**: `crates/piperine-transpile/src/print/reaction.rs`
**Depends on**: T27, T26
**Reuses**: the reaction's `binds`
**Requirement**: FIX-13
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`: `above_with_an_emit_writes_one_record_at_its_crossing`, `above_true_from_the_start_writes_one_record_at_zero`, `two_disjuncts_of_one_microstep_run_the_body_once`
- [ ] The buck targets of `piperine-parity` pass unedited (`cargo test -p piperine-parity --test buck_records --test buck_closed_loop`)
- [ ] Gate check passes: Full, then Build (end of phase 4)
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: build

**Commit**: `feat(transpile): a reaction reads which event fired`

---

### T29: `finish` and `fatal` end the analysis

**What**: A reaction holding an `emit!` on `sim` returns `Flow::Finish` after its commits when that record's `finish` field is known true (design C1).
**Where**: `crates/piperine-transpile/src/print/reaction.rs` (modify)
**Depends on**: T28
**Reuses**: the protocol's `Flow::Finish` (`protocol/transient.rs:344`, `operating_point.rs:71`)
**Requirement**: FIX-01
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_fixes`: `finish_ends_the_transient_at_its_instant` (the probe of FIX-01 AC 1: last step at 1 ns, no `late` record, `k` 0), `fatal_ends_the_transient_at_its_instant`, `a_finish_field_false_or_unknown_runs_to_stop`, `finish_ends_an_operating_point`
- [ ] Gate check passes: Full
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `fix(transpile): finish and fatal end the analysis`

---

### T30: A run ended by `finish` is not short (cross-lane: `piperine-api`)

**What**: `Session::analysis`'s transient arm takes the instant of the last `sim` record whose `finish` is known true as the run's end: the short-run check compares with it, and the recorder's grid ends there (design C1). Rebase on `main` first if `host-ii` has merged.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T29
**Reuses**: `Delivery`; `Recorder::with_grid`
**Requirement**: FIX-02
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-api --test records`: `a_run_ended_by_finish_gives_its_rows_to_the_instant` (with and without `print_step`), `a_short_run_without_finish_still_fails`
- [ ] Gate check passes: Quick (`Run records`), then `cargo test -p piperine-api`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `fix(api): a run ended by finish is not a short run`

---

### T31: The verdict reads `fatal` (cross-lane: `piperine-api`)

**What**: `Verdict` keeps the first `sim` record whose `failed` is known true; `passed` is false with one; `Verdict::fatal()` returns it.
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: T30
**Reuses**: `impl RecordSink for Verdict`
**Requirement**: FIX-02
**Executor**: Sonnet

**Done when**:

- [ ] Unit tests with exactly these names pass in `cargo test -p piperine-api --lib`: `verdict::tests::a_fatal_record_fails_the_verdict_with_its_message`, `verdict::tests::a_finish_record_does_not_fail_the_verdict`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `fix(api): a fatal record fails the verdict`

---

### T32: `run` and `test` print `fatal` (cross-lane: `piperine-cli`)

**What**: `piperine run` prints `fatal at <t:e>: <msg>` on standard error and exits 1 after a fatal run; `piperine test` prints `FAIL <name> (fatal at <t:e>: <msg>)` for it (design C1).
**Where**: `crates/piperine-cli/src/commands/test.rs`
**Depends on**: T31
**Reuses**: `Verdict::fatal`; `crate::exit::Exit` of `commands/run.rs`
**Requirement**: FIX-02
**Executor**: Sonnet

**Done when**:

- [ ] `run_prints_the_fatal_line_and_exits_1` passes in `cargo test -p piperine-cli --test run`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `test_prints_fail_with_the_fatal_message`, `test_passes_a_run_ended_by_finish`
- [ ] Gate check passes: Quick (`Run clirun`, `Run clicmd`), then `cargo test -p piperine-cli`
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `fix(cli): run and test report a fatal run`

---

### T33: `freq`, the analysis kinds and `bound_step` in the type checker

**What**: `CORE_ROWS` gains `freq` (continuous, everywhere but definitions and `at`), `bound_step` (an `analog` statement of `f64`), and `analysis` accepts `Dc`, `Transient`, `Ac`, `Noise` (discrete); `simparam` stays `P0001`.
**Where**: `crates/piperine-lang/src/types/catalogue.rs` (modify)
**Depends on**: T7
**Reuses**: the rows of `now` and `analysis(Static)`; `core_call`
**Requirement**: FIX-09, FIX-10, FIX-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_analog`: `freq_analysis_kinds_and_bound_step_type`, `simparam_stays_p0001`
- [ ] Existing tests asserting the lifted `P0001`s are rewritten and named in the commit
- [ ] Gate check passes: Quick (`Run tanalog`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): freq, the analysis kinds and bound_step`

---

### T34: `freq`, the analysis kinds and `bound_step` lower

**What**: `freq()` lowers to `Env::Freq` in analog and to 0.0 in a reaction or a discrete function; `analysis(k)` to `Env::Analysis(k)`; `bound_step(dt)` to `Stmt::BoundStep` at the top of an analog body, `P0001` under an `if` or a `match` (design C9, C10).
**Where**: `crates/piperine-lang/src/elab/analog.rs` (modify)
**Depends on**: T33, T9, T10
**Reuses**: the lowering of `now()` and `analysis(Static)`
**Requirement**: FIX-09, FIX-10, FIX-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_analog`: `freq_analysis_and_bound_step_lower`, `bound_step_under_an_if_is_p0001`
- [ ] Gate check passes: Quick (`Run eanalog`), then `cargo test -p piperine-lang`
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): freq, the analysis kinds and bound_step lower`

---

### T35: An AC or noise operating point carries its analysis flag (cross-lane: `piperine-solver`)

**What**: `DcSolver::under(circuit, context, analysis)` builds the operating point's digital side with the caller's analysis global; `ac.rs`, `noise.rs` and (where its operating point is a `DcSolver`) `transient.rs` call it with `global::AC`, `global::NOISE` and `global::TRAN`; `DcSolver::new` and its other callers are unchanged (design C10).
**Where**: `crates/piperine-solver/src/analyses/dc.rs`
**Depends on**: T1
**Reuses**: `DigitalSide::new(digital, bound, analysis)`
**Requirement**: FIX-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-solver --test environment`: `an_ac_operating_point_carries_the_ac_flag`, `a_noise_operating_point_carries_the_noise_flag`, `a_transient_operating_point_carries_the_tran_flag`
- [ ] `cargo test -p piperine-solver --test parity_baseline` and the parity targets pass unedited
- [ ] Gate check passes: Quick (`Run env`), then `cargo test -p piperine-solver`, then Build (end of phase 5)
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `fix(solver): an AC or noise operating point carries its analysis flag`

---

### T36: The environment and `bound_step` run

**What**: The printer reads `Env::Freq` (the `FREQ` ambient) and `Env::Analysis(k)` (its global pin) and writes each `BoundStep` site's `CAP` watch. New target `crates/piperine-parity/tests/source_environment.rs` (`//!` header: `freq`, `analysis(kind)` and `bound_step` run from source on the Rust backend).
**Where**: `crates/piperine-parity/tests/source_environment.rs`
**Depends on**: T34, T35, T11
**Reuses**: `print/analog.rs`'s environment reads; `Project`
**Requirement**: FIX-09, FIX-10, FIX-11
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test source_environment`: `bound_step_caps_every_step`, `the_smaller_of_two_caps_wins`, `a_non_positive_bound_step_is_e0509`, `freq_follows_the_ac_sweep`, `freq_is_zero_outside_a_sweep`, `freq_in_a_reaction_is_zero`, `analysis_dc_holds_in_an_op`, `analysis_transient_holds_in_a_transient`, `analysis_ac_holds_in_an_ac_run`, `a_reaction_reads_the_analysis_kind`
- [ ] `cargo test -p piperine-transpile --test golden` and the `Run twins` line pass unedited
- [ ] Gate check passes: Full
- [ ] Test count: the 10 tests named above

**Tests**: e2e
**Gate**: full

**Commit**: `feat(transpile): freq, the analysis kinds and bound_step run`

---

### T37: The Book's cycle counter runs; the clipper's refusal is current

**What**: The cycle counter's `Run it` listing becomes the binary's output (its command prints `count` on a grid so the table is short, its prose says what is printed and no longer says "once `sin` is compiled"); the clipper's refusal listing becomes the binary's present first refusal and its prose names what it still waits for (`limexp`, `settle`). `crates/piperine-cli/tests/book.rs` holds the two claims.
**Where**: `docs/book/content.html`
**Depends on**: T21, T36
**Reuses**: `every_cli_listing_of_the_book_is_what_the_binary_prints`; `docs/book/build.py`
**Requirement**: CAT-05
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test book`: `the_cycle_counter_prints_three_pulses` (records `n=1` to `n=3`, each within 1 µs of 15.9 µs plus a whole millisecond, `count` 3 in the last row), `the_clipper_refusal_no_longer_names_sin`
- [ ] `every_cli_listing_of_the_book_is_what_the_binary_prints` passes unedited
- [ ] Gate check passes: Quick (`Run clibook`)
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `docs(book): the cycle counter runs`

---

### T38: `GAPS.md` drops what closed

**What**: `GAPS.md` removes S1, S5, B1, B2, L11, L14, L15, L17, L41, L46, L48, L54, L55, the `Option`/`bool` half of L16 and `db10`/`db20` from L58; L47 keeps `simparam`; §9 and §10 counts follow; S3, H.5 to H.7, `normal`, `limexp` stay.
**Where**: `GAPS.md`
**Depends on**: T37
**Reuses**: `gaps_drops_what_closed_and_lists_what_is_left` as the pattern, unedited
**Requirement**: DOC-01
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_msl_fixes_closed` passes in `cargo test -p piperine-cli --test docs`
- [ ] Every existing test of `docs.rs` passes unedited
- [ ] Gate check passes: Quick (`Run docs`)
- [ ] Test count: the 1 test named above

**Tests**: integration
**Gate**: quick

**Commit**: `docs(gaps): drop what msl-fixes closed`

---

### T39: `ROADMAP.md` and MD-67

**What**: `ROADMAP.md` records `msl-fixes` as delivered; `.specs/STATE.md` gets MD-67 as `design.md` ("Tech Decisions") words it, with Q2's amendment of abi.md A4 if (a) was chosen.
**Where**: `ROADMAP.md`
**Depends on**: T38
**Reuses**: `the_roadmap_marks_the_cli_item_done` and `state_records_the_cli_decision` as patterns, unedited
**Requirement**: DOC-01
**Executor**: Sonnet

**Done when**:

- [ ] `the_roadmap_and_state_record_msl_fixes` passes in `cargo test -p piperine-cli --test docs`
- [ ] Gate check passes: Quick (`Run docs`), then Build (end of the feature)
- [ ] Test count: the 1 test named above

**Tests**: integration
**Gate**: build

**Commit**: `docs(state): the inherited library and the msl-fixes record (MD-67)`

---

## Checks

**C1** - A `finish` record known true ends the transient at its instant: last step at 1 ns, no `late` record, `k` 0. (FIX-01, AC 1; T29)
Proof: run sfix | test finish_ends_the_transient_at_its_instant ... ok

**C2** - `fatal` ends the transient at its instant. (FIX-01, AC 2; T29)
Proof: run sfix | test fatal_ends_the_transient_at_its_instant ... ok

**C3** - A `finish` field false or unknown lets the run go to `stop`. (FIX-01, AC 3; T29)
Proof: run sfix | test a_finish_field_false_or_unknown_runs_to_stop ... ok

**C4** - A `finish` record ends an operating point. (FIX-01, AC 4; T29)
Proof: run sfix | test finish_ends_an_operating_point ... ok

**C5** - A run ended by `finish` gives its rows to the instant and is not short. (FIX-02, AC 1; T30)
Proof: run records | test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok

**C6** - `piperine run` prints `fatal at <t>: <msg>` and exits 1 after `fatal`. (FIX-02, AC 2; T32)
Proof: run clirun | test run_prints_the_fatal_line_and_exits_1 ... ok

**C7** - `piperine test` prints `FAIL` with the fatal message. (FIX-02, AC 3; T32)
Proof: run clicmd | test test_prints_fail_with_the_fatal_message ... ok

**C8** - `piperine test` passes a run ended by `finish()`. (FIX-02, AC 4; T32)
Proof: run clicmd | test test_passes_a_run_ended_by_finish ... ok

**C9** - A run short of `stop` with no `finish` still fails as CLI-05 AC 11. (FIX-02, AC 5; T30)
Proof: run records | test a_short_run_without_finish_still_fails ... ok

**C10** - A `where` that reads a later param is `E0318`, whatever the values. (FIX-03, AC 1; T2)
Proof: run tmod | test a_where_that_reads_a_later_param_is_e0318 ... ok

**C11** - A `where` over its own and earlier params is accepted. (FIX-03, AC 2; T2)
Proof: run tmod | test a_where_over_its_own_and_earlier_params_is_accepted ... ok

**C12** - `i(gnd, a) +> 1m;` builds and gives 1.0 V into 1 kΩ. (FIX-04, AC 1; T22)
Proof: run sfix | test a_literal_flow_source_builds_and_runs ... ok

**C13** - `v(a, gnd) +> 2.0;` builds and gives 2.0 V. (FIX-04, AC 2; T22)
Proof: run sfix | test a_literal_potential_source_builds_and_runs ... ok

**C14** - A current read from a carrier only builds and gives 2.0 V after 1 ns. (FIX-04, AC 3; T22)
Proof: run sfix | test a_carrier_driven_current_builds_and_runs ... ok

**C15** - The generated text of every golden body and every twin is unchanged. (FIX-04, AC 4; T22)
Proof: `cargo test -p piperine-transpile --test golden && cargo test -p piperine-cli --test twins && git diff --exit-code 4293de6b -- crates/piperine-transpile/tests/golden crates/piperine-cli/tests/goldens`

**C16** - A `cross` over a carrier that may be unknown builds. (FIX-05, AC 1; T24)
Proof: run sfix | test a_cross_over_a_maybe_unknown_carrier_builds ... ok

**C17** - `absdelta(v(a), 0.2)` on a 1 V/µs ramp fires at 0, 0.2, 0.4, 0.6, 0.8 µs. (FIX-05, AC 2; T24)
Proof: run sfix | test absdelta_fires_every_two_tenths_of_a_volt ... ok

**C18** - An unknown `cross` value fires neither at its solution nor at the next. (FIX-05, AC 3; T23)
Proof: run watch | test a_nan_cross_watch_is_unknown_and_never_fires ... ok

**C19** - An infinite `cross` value fails the solve naming the watch. (FIX-05, AC 4; T23)
Proof: run watch | test an_infinite_cross_watch_fails_the_solve ... ok

**C20** - `f(1.0)` gives 111.0: left-out optionals are `None`. (FIX-06, AC 1; T6)
Proof: run sfix | test left_out_optional_arguments_are_none ... ok

**C21** - `f(1.0, z: 3.0)` gives 14.0. (FIX-06, AC 2; T6)
Proof: run sfix | test a_named_optional_argument_binds_by_name ... ok

**C22** - `f(1.0, 2.0)` gives 103.0. (FIX-06, AC 3; T6)
Proof: run sfix | test a_plain_value_for_an_option_is_some ... ok

**C23** - `f(1.0, None, Some(3.0))` gives 14.0. (FIX-06, AC 4; T6)
Proof: run sfix | test none_and_some_pass_positionally ... ok

**C24** - A named argument that is not optional or already bound is `E0140`. (FIX-06, AC 5; T5)
Proof: run tvalues | test a_named_argument_that_is_not_optional_or_bound_is_e0140 ... ok

**C25** - A wrong argument count is `E0141`, not `P0002`. (FIX-06, AC 6; T5)
Proof: run tvalues | test a_wrong_argument_count_is_e0141 ... ok

**C26** - An `Option` parameter before a required one is positional only. (FIX-06, AC 7; T5)
Proof: run tvalues | test an_option_before_a_required_parameter_is_positional_only ... ok

**C27** - A `PartialOrd` enum compares in declaration order in a reaction. (FIX-07, AC 1; T3)
Proof: run sfix | test an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction ... ok

**C28** - `<` on an enum without `PartialOrd` is `P0002` naming it. (FIX-07, AC 2; T3)
Proof: run tvalues | test comparing_an_enum_without_partial_ord_is_p0002 ... ok

**C29** - Two `analog` blocks act as one: 1.0 V. (FIX-08, AC 1; T4)
Proof: run sfix | test two_analog_blocks_act_as_one ... ok

**C30** - Two `analog` blocks keep their locals apart. (FIX-08, AC 2; T4)
Proof: run eanalog | test two_analog_blocks_keep_their_locals_apart ... ok

**C31** - `bound_step(0.02 / f)` keeps every step of a 1 kHz sine at most 20 µs. (FIX-09, AC 1; T36)
Proof: run senv | test bound_step_caps_every_step ... ok

**C32** - Of two caps the smaller wins. (FIX-09, AC 2; T36)
Proof: run senv | test the_smaller_of_two_caps_wins ... ok

**C33** - A non-positive or non-finite `bound_step` fails with `E0509`. (FIX-09, AC 3; T36)
Proof: run senv | test a_non_positive_bound_step_is_e0509 ... ok

**C34** - `bound_step` under an `if` is `P0001` by name. (FIX-09, AC 4; T34)
Proof: run eanalog | test bound_step_under_an_if_is_p0001 ... ok

**C35** - `freq()` in AC gives `|v(a)|` = 1m / (1m + f · 1n) at each point. (FIX-10, AC 1; T36)
Proof: run senv | test freq_follows_the_ac_sweep ... ok

**C36** - `freq()` is 0 in an operating point and a transient step. (FIX-10, AC 2; T36)
Proof: run senv | test freq_is_zero_outside_a_sweep ... ok

**C37** - `freq()` is 0 in a reaction. (FIX-10, AC 3; T36)
Proof: run senv | test freq_in_a_reaction_is_zero ... ok

**C38** - `simparam` stays `P0001`. (FIX-10, AC 4; T33)
Proof: run tanalog | test simparam_stays_p0001 ... ok

**C39** - In `op`, `analysis(Dc)` is true and the others false. (FIX-11, AC 1; T36)
Proof: run senv | test analysis_dc_holds_in_an_op ... ok

**C40** - In a transient, `analysis(Transient)` is true in its operating point and its steps. (FIX-11, AC 2; T36)
Proof: run senv | test analysis_transient_holds_in_a_transient ... ok

**C41** - In AC, `analysis(Ac)` is true in its operating point and sweep, `analysis(Dc)` false. (FIX-11, AC 3; T36)
Proof: run senv | test analysis_ac_holds_in_an_ac_run ... ok

**C42** - The noise analysis's operating point carries the `NOISE` flag. (FIX-11, AC 4; T35)
Proof: run env | test a_noise_operating_point_carries_the_noise_flag ... ok

**C43** - A reaction reads the kind of its analysis. (FIX-11, AC 5; T36)
Proof: run senv | test a_reaction_reads_the_analysis_kind ... ok

**C44** - `cross` with `ttol`/`etol`, positional and named, builds and runs. (FIX-12, AC 1; T26)
Proof: run sfix | test cross_with_tolerances_runs ... ok

**C45** - A `cross` with tolerances declares tolerance pins holding them. (FIX-12, AC 2; T11)
Proof: run plan | test bound_step_is_a_cap_watch_and_cross_tolerances_are_pins ... ok

**C46** - A `cross` with `ttol` 1 ns fires within 1 ns of `RC · ln 2`. (FIX-12, AC 3; T26)
Proof: run sfix | test a_cross_with_ttol_fires_within_it ... ok

**C47** - A tolerance that reads a carrier, a probe or a local is `P0001` by name. (FIX-12, AC 4; T25)
Proof: run edig | test a_cross_tolerance_that_is_not_constant_is_p0001 ... ok

**C48** - `on above(v(a) - 0.5) { emit!(..) }` writes one record at about 0.5 µs. (FIX-13, AC 1; T28)
Proof: run sfix | test above_with_an_emit_writes_one_record_at_its_crossing ... ok

**C49** - Started above, it writes one record at 0. (FIX-13, AC 2; T28)
Proof: run sfix | test above_true_from_the_start_writes_one_record_at_zero ... ok

**C50** - Two disjuncts firing in one microstep run the body once. (FIX-13, AC 3; T28)
Proof: run sfix | test two_disjuncts_of_one_microstep_run_the_body_once ... ok

**C51** - The interpreter refuses each new form, naming it. (FIX-14, AC 1; T12)
Proof: run frozen | test the_interpreter_refuses_each_new_form_by_name ... ok

**C52** - Each H.2 row types with its signature. (CAT-01, AC 1; T15)
Proof: run tvalues | test every_h2_row_types_with_its_signature ... ok

**C53** - Each H.2 row gives Rust's value bit for bit, in analog and in a reaction. (CAT-01, AC 2; T21)
Proof: run scat | test f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction ... ok

**C54** - The differentiable rows' partials agree with finite differences. (CAT-01, AC 3; T13)
Proof: run tlib | test rt::real::tests::the_differentiable_rows_agree_with_finite_differences ... ok

**C55** - The piecewise rows have derivative 0, `fract` and `rem_euclid` 1. (CAT-01, AC 4; T13)
Proof: run tlib | test rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one ... ok

**C56** - A row over an unknown value is unknown exactly when an operand is. (CAT-01, AC 5; T21)
Proof: run scat | test a_row_over_an_unknown_value_is_unknown ... ok

**C57** - `x.cbrt()` is `E0161` naming the method and `f64`. (CAT-01, AC 6; T15)
Proof: run tvalues | test a_method_outside_h2_on_f64_is_e0161 ... ok

**C58** - Each H.1 row types with its signature. (CAT-02, AC 1; T16)
Proof: run tint | test every_h1_row_types_with_its_signature ... ok

**C59** - `2u8.pow(9)` is 0. (CAT-02, AC 2; T21)
Proof: run scat | test pow_wraps ... ok

**C60** - `saturating_*` clamp: 255 and -128. (CAT-02, AC 3; T21)
Proof: run scat | test saturating_rows_clamp ... ok

**C61** - `checked_*` give `None` on overflow and `Some` otherwise. (CAT-02, AC 4; T21)
Proof: run scat | test checked_rows_give_none_on_overflow ... ok

**C62** - `rem_euclid` by zero faults the run naming it. (CAT-02, AC 5; T21)
Proof: run scat | test rem_euclid_by_zero_faults_naming_it ... ok

**C63** - Signed `abs`, `signum`, `rem_euclid` follow Rust: -128, -1, 2. (CAT-02, AC 6; T21)
Proof: run scat | test signed_abs_signum_rem_euclid_follow_rust ... ok

**C64** - `abs`/`signum` on an unsigned integer is `E0161`. (CAT-02, AC 7; T16)
Proof: run tint | test abs_and_signum_on_an_unsigned_integer_are_e0161 ... ok

**C65** - `unwrap` and `expect` of `Some(v)` give `v`. (CAT-03, AC 1; T21)
Proof: run scat | test unwrap_and_expect_of_some_give_the_payload ... ok

**C66** - `unwrap` of `None` faults naming `unwrap`. (CAT-03, AC 2; T21)
Proof: run scat | test unwrap_of_none_faults_naming_unwrap ... ok

**C67** - `expect("msg")` of `None` faults with `msg`. (CAT-03, AC 3; T21)
Proof: run scat | test expect_of_none_faults_with_its_message ... ok

**C68** - `unwrap_or_else` evaluates its default for `None` only. (CAT-03, AC 4; T21)
Proof: run scat | test unwrap_or_else_evaluates_its_default_for_none_only ... ok

**C69** - `map` and `and_then` bind the payload. (CAT-03, AC 5; T21)
Proof: run scat | test map_and_and_then_bind_the_payload ... ok

**C70** - `a.or(b)` takes the first `Some`. (CAT-03, AC 6; T21)
Proof: run scat | test or_takes_the_first_some ... ok

**C71** - `then` and `then_some` follow the `bool`. (CAT-03, AC 7; T21)
Proof: run scat | test then_and_then_some_follow_the_bool ... ok

**C72** - `expect` with a non-literal message is `P0001`. (CAT-03, AC 8; T17)
Proof: run tcomp | test expect_with_a_non_literal_message_is_p0001 ... ok

**C73** - A method outside H.3/H.4 on `bool` or `Option` is `E0161`. (CAT-03, AC 9; T17)
Proof: run tcomp | test a_method_outside_h3_h4_is_e0161 ... ok

**C74** - `db10(100.0)` and `db20(-10.0)` give 20.0. (CAT-04, AC 1; T21)
Proof: run scat | test db10_and_db20_run ... ok

**C75** - `db10` and `db20` type-check from the stdlib. (CAT-04, AC 2; T21)
Proof: run scat | test db10_and_db20_type_check_from_the_stdlib ... ok

**C76** - The Book's cycle counter prints three pulses at about 15.9 µs past each millisecond and `count` 3. (CAT-05, AC 1; T37)
Proof: run clibook | test the_cycle_counter_prints_three_pulses ... ok

**C77** - Every CLI listing of the Book is what the binary prints. (CAT-05, AC 2; T37)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C78** - The clipper's refusal no longer names `sin` or `bound_step`. (CAT-05, AC 3; T37)
Proof: run clibook | test the_clipper_refusal_no_longer_names_sin ... ok

**C79** - `GAPS.md` drops what closed and keeps what is left. (DOC-01, AC 1; T38)
Proof: run docs | test gaps_drops_what_msl_fixes_closed ... ok

**C80** - `ROADMAP.md` and `.specs/STATE.md` record the feature and MD-67. (DOC-01, AC 2; T39)
Proof: run docs | test the_roadmap_and_state_record_msl_fixes ... ok

**C81** - Each twin prints the bytes of its golden on each backend. (PAR-01, AC 1)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok

**C82** - Every parity target passes with its sources, the contract and the transpile goldens unedited. (PAR-01, AC 2)
Proof: `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test parity_corpus --test parity_grid && cargo test -p piperine-solver --test parity_baseline && git diff --exit-code 4293de6b -- crates/piperine-parity/src/contract.rs crates/piperine-parity/tests/parity_op.rs crates/piperine-parity/tests/parity_dc.rs crates/piperine-parity/tests/parity_ac.rs crates/piperine-parity/tests/parity_tran.rs crates/piperine-parity/tests/parity_corpus.rs crates/piperine-parity/tests/parity_grid.rs crates/piperine-solver/tests/parity_baseline.rs tests/ngspice crates/piperine-transpile/tests/golden`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (batch 1, worker, 2026-11-17) (1) T4's e2e `two_analog_blocks_act_as_one` drives a param (`i(gnd, p) +> ia;`, `ia` = 0.5m) instead of the literal `0.5m`, because the literal form is B1 (T22) and does not build yet; FIX-04 covers the literal in T22. (2) T5/T7 keep a core primitive's own count check as `P0002` (`ac_stim()`, `temperature(1.0)` assert it in `types_analog`); `E0141` is the function's code and the binder's for a required core parameter left unbound by name. (3) A named argument for a required parameter, or an `Option` one before a required one, is `E0140` (§3.2.8); a name bound twice is `E0140`. (4) T7 removes the row "tolerance given to cross" of `types_subset.rs` and the two cross rows of `types_events.rs::event_expressions_are_typed` (the `settle` row stays): the checker no longer refuses them; the elaborator still refuses a non-`None` tolerance (`no_tolerance`, `elab/events.rs`) until T25. (5) Test runs set `TMPDIR` off the full `/tmp` tmpfs (leaked `.tmp*` test directories filled it). (batch 2, worker, 2026-11-17) (6) `EventSource::Cross` is a struct variant `{e, ttol, etol}`, so constructors/patterns in existing tests were updated mechanically (no assertion changed); `ir/tests/model.rs` gained arms for the new forms. (7) T9/T10/T11 left interim refusals or skips in the plan, interpreter and transpile printer (named `T12`, `T19`, `T26`, `T28`, `T36` in their messages); the printer's method-name match refuses a row it does not write until T20. (8) T11 added IR dependency targets `BoundStep(site)`, `Ttol(k)`, `Etol(k)` and `Body::fired_reads`; plan modes `Cap`/`Ttol`/`Etol`, targets `Cap`/`Ttol`/`Etol`, sources `Freq`, `Analysis(kind)`, `Fired(k)` (all appended last), pins `analysis:<kind>`, `ttol:<k>`, `etol:<k>`, watches `cap:<site>`. (9) T12 refuses at `Interpreter::new` by name (freq, analysis(kind), bound_step, fired, ttol/etol) and in the walk (method rows by MSL name, unwrap, expect); `Method::name()` is the one source of MSL names. (10) T14: no free function allowed (AGENTS), so unwrap is `rt::Unwrap::of(is, value, message: Option<&str>)`, re-exported at the digital `rt` root. (11) `Dual::rem_euclid` carries the divisor term (minus the integer quotient); `powf`/`atan2`/`hypot` skip a zero-tangent part so `inf*0` never reaches a tangent. (batch 3, worker, 2026-11-17) (12) T15: a method miss on a complete receiver type (f64 now; integers from T16, Option and bool from T17) is E0161 naming method and type, except a name the project defines in an impl/trait or a B.1 carrier method (known, known_or, is_known, high, changed, prev, rise, fall, others, inflow, level), which stay P0001; `Catalogue::method` returns `Lookup::{Row, Absent, Uncatalogued}`; the row type is `Row{name, method, params: &[Arg], result: Res}`. (13) T16: `abs`/`signum` are signed-only rows (an unsigned receiver is E0161); `checked_*` are types-level methods `CheckedAdd/Sub/Mul` returning `Option<Self>`, lowered by T18 to the pair; rewritten tests: `min_max_clamp_and_floor_are_typed` (`floor` on u8 is E0161), `an_uncatalogued_method_is_e0161_by_name`, `types_carriers` leading_zeros. (14) T17: `Lookup::Payload(Method)` marks a row typed by `Checker::payload_row` (closure rows: `map`/`and_then`/`then` need a closure literal else P0001, `unwrap_or_else` takes any `fn() -> T`; `expect` a string literal else P0001 naming expect); `closure_typed(expr, params, Option<result>)` infers a body when no result is given and records the closure as `Fn(params, body)`; `then_some(7)` with no position is `Option<i32>`; rewritten: `option_values_and_their_methods_are_typed` (`flatten` is E0161) and `catalogue_methods_on_f64_are_f64` (bool `exp` is E0161). (15) T18: also touched `elab/consts.rs` (integer `abs`/`signum`/`pow`/`rem_euclid`/`saturating_*` fold over constants with wrapping, a constant `rem_euclid` by zero is E0104, so `2u8.pow(9)` folds to 0) and `elab/compound.rs` (the `checked_*` pair arm of `lower_option`); numeric rows map through `Elaboration::ir_method`; f64 `exp`/`ln`/`sqrt`/`abs` stay `Math`. (16) T19: a closure of a row runs only for `Some`/true, so its body sits under `if is` (`map`, `and_then`, `then`) with the zero as the other arm; `unwrap`/`expect` of an Option whose presence may be unknown is P0001; a closure argument that is not a literal is P0001 in lowering; `then_some(v)` is the plain pair `(c, v)`. (17) T20: `ValuePrinter::method` prints every row by `Method::name()`; `powi`'s exponent prints `y.get() as i32`; `Expr::Unwrap` prints `rt::Unwrap::of(is, value, None|Some("msg"))` (message Debug-escaped). (18) T21: found and fixed two printer defects in `print/value.rs` (T20's file): an `if` as the condition of an `if` printed parenthesized (rustc `unused_parens` warning, so the generated source was not warning-free) and a literal `powi` exponent printed `rt::Int::<32>::new(n)`, which analog-only designs lack (now `n_i32`); `inf` is a std const, so a `let inf` is a path pattern (P0001 'binding'); an initial value of a reaction's output goes on the declaring module's `out` port, never on the signal (E0223); an analog contribution that reads no param or probe is still B1 (T22), so the f64 analog rows read params. (19) Known red until T37: `piperine-cli --test book every_cli_listing_of_the_book_is_what_the_binary_prints` (the Book's two clipper/cycles listings still show the `sin` refusal that T15 lifted; T37 rewrites them).
**Dispatched:** Batch 1 (phase 1, T1-T7) - Sonnet worker, 2026-11-17. Batch 1 done (430714a2). Batch 2 (phase 2, T8-T14) - Sonnet worker, 2026-11-17. Batch 2 done (fb4343d8). Batch 3 (phase 3, T15-T21) - Sonnet worker, 2026-11-17. Batch 3 done (6f8b93bf; Book listing red until fixed). Batch 4 (phase 4, T22-T28, plus the Book clipper/cycles listing pulled forward from T37) - Sonnet worker, 2026-11-17.
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1  T2  T3  T4  T5  T6  T7
Phase 2:  T8  T9  T10  T11  T12  T13  T14
Phase 3:  T15  T16  T17  T18  T19  T20  T21
Phase 4:  T22  T23  T24  T25  T26  T27  T28
Phase 5:  T29  T30  T31  T32  T33  T34  T35
Phase 6:  T36  T37  T38  T39
```

Tasks run in the order listed; the dependencies are the diagrams of the Execution Plan. Execution is strictly sequential. Batches: one per phase (7, 7, 7, 7, 7, 4 tasks).

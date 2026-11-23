# Generics Tasks (phases A and B)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesmodel**: `cargo test -p piperine-lang --test types_model`
**Run typesgen**: `cargo test -p piperine-lang --test types_generics`
**Run typestraits**: `cargo test -p piperine-lang --test types_traits`
**Run typeshandles**: `cargo test -p piperine-lang --test types_handles`
**Run elabgen**: `cargo test -p piperine-lang --test elab_generics`
**Run elabhandles**: `cargo test -p piperine-lang --test elab_handles`
**Run irgen**: `cargo test -p piperine-ir --test generics`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run tgen**: `cargo test -p piperine-transpile --test generics`
**Run plancomp**: `cargo test -p piperine-plan --test composite_shape`
**Run srcgen**: `cargo test -p piperine-parity --test source_generics`
**Run srchandles**: `cargo test -p piperine-parity --test source_handles`
**Run parityproject**: `cargo test -p piperine-parity --test project`
**Run apiproject**: `cargo test -p piperine-api --test project`
**Run apisession**: `cargo test -p piperine-api --test session`
**Run clicommands**: `cargo test -p piperine-cli --test commands`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** Q1 (c) (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"): a generic body is checked once at its definition against its bounds; `where`, array lengths and const values per instantiation. The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D17; every task follows them as written.

**Base and order.** Batch B1 (phase 1, T1 to T6) starts from `main` once `analog-structure` has merged there, and may run while `composite-values` is still in flight. Every later batch starts at T7, after `composite-values` phases A and B are on `main`: T7 rebases B1's commits onto that tree. design.md "Scheduling against the other lanes" says why.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-c CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-c` before any cargo command. Cargo with `-p <crate>`, never `--workspace`, never `cargo clean`. Commands run in the foreground; no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. Every buck target passes after every task that touches the printer, the plan, the hosts or the elaborator's lowering of an existing form.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same expression evaluated by Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`).** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; no abbreviated name; no god method and no needless abstraction; the data path evident. Every new type, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design.md "Components" names, never into `types/expr.rs`, `types/call.rs` or `print/value.rs` beyond a dispatch arm.
- **Conventions.** No new IR `Expr`/`Stmt` variant; new IR fields carry `#[serde(default)]`. The interpreter refuses every new IR marker by name (R1) in the task that adds it, and `crates/piperine-interp/tests/frozen.rs::each_generics_form_is_refused_by_name` grows with it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level tests put their models in `crates/piperine-parity/tests/generics/*.msl` and `tests/handles/*.msl`.
- **Cross-lane files** (design.md "Scheduling against the other lanes"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-64 (hoisting, pure IR functions).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Whole-model check (`piperine-lang/src/types/model.rs`) | integration | every item kind unreached; each code as when reached; `std` left out | `crates/piperine-lang/tests/types_model.rs` | `cargo test -p piperine-lang --test types_model` |
| Checker, generics (`piperine-lang/src/types`) | integration | every AC the checker decides; each error code with its span and the names it gives; definition vs instantiation split | `crates/piperine-lang/tests/{types_generics,types_traits,types_handles}.rs` | `cargo test -p piperine-lang --test types_generics` / `--test types_traits` / `--test types_handles` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC; instance identity and names; each code; the POM untouched | `crates/piperine-lang/tests/{elab_generics,elab_handles}.rs` | `cargo test -p piperine-lang --test elab_generics` / `--test elab_handles` |
| IR (`piperine-ir`) | integration | each new field built, checked (valid and refused), printed | `crates/piperine-ir/tests/generics.rs` | `cargo test -p piperine-ir --test generics` |
| Interpreter refusals (`piperine-interp`) | integration | each new marker refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Printer (`piperine-transpile/src/print`) | integration | instance names compile and do not collide; a hand-written `Repr` round-trips through `rustc` | `crates/piperine-transpile/tests/generics.rs` | `cargo test -p piperine-transpile --test generics` |
| Plan (`piperine-plan`) | integration | a hand-written `Repr` layout of `WIDTH` levels | `crates/piperine-plan/tests/composite_shape.rs` | `cargo test -p piperine-plan --test composite_shape` |
| Source-level runs (end to end) | integration | one test per run-time AC, from MSL source on the Rust backend, expected values computed by Rust in the test | `crates/piperine-parity/tests/{source_generics,source_handles,project}.rs`, models in `tests/generics/`, `tests/handles/` | `cargo test -p piperine-parity --test source_generics` / `--test source_handles` / `--test project` |
| Host library (`piperine-api`) | integration | library and dependency checks; a run failing as the check does | `crates/piperine-api/tests/{project,session}.rs` | `cargo test -p piperine-api --test project` / `--test session` |
| Command line (`piperine-cli`) | integration | `check`, `build`, `run` on a library fixture with one error per item kind | `crates/piperine-cli/tests/commands.rs`, fixture `tests/projects/unreached_errors/` | `cargo test -p piperine-cli --test commands` |
| Spec text and repository records | integration | one assertion per decided fact or removed `GAPS.md` row | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, `crates/piperine-lang/tests/std_galleries.rs` | `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test std_galleries` |
| Contract re-check (T7) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the printer, the plan, the hosts or the lowering of an existing form | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins --test commands`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: The whole-model check (S3; may run before `composite-values` merges)

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Contract

The tree after the rebase onto `main` with `composite-values` A–B merged.

```
T7
```

### Phase 3: IR, plan and printer for instances

```
T8 → T9 → T10 → T11 → T12
```

### Phase 4: The checker's type parameters and the definition check

```
T13 → T14 → T15 → T16 → T17 → T18 → T19
```

### Phase 5: Generic calls

```
T20 → T21 → T22 → T23
```

### Phase 6: `impl`, `self` and the declaration rules

```
T24 → T25 → T26 → T27
```

### Phase 7: Traits, `Repr` and generic value types in the checker

```
T28 → T29 → T30 → T31
```

### Phase 8: The elaborator's instances

```
T32 → T33 → T34 → T35
```

### Phase 9: Dispatch, callable arguments and generic types in the elaborator

```
T36 → T37 → T38 → T39 → T40
```

### Phase 10: Phase A closure

```
T41 → T42 → T43
```

### Phase 11: Handles in the checker (phase B)

```
T44 → T45 → T46 → T47 → T48
```

### Phase 12: Handles in the elaborator and phase B closure

```
T49 → T50 → T51 → T52
```

---

## Task Breakdown

### T1: `ModelCheck`: every item of every non-std crate

**What**: New `types/model.rs` with `ModelCheck` (and `Types::check_model(design) -> Result<(), Vec<DesignDiagnostic>>` in `table.rs`): one `Checker` seeded with every module, function, `const` and `impl` function of every crate but `std` (D2), in declaration order, whose table is dropped and whose diagnostics are returned in source order; a design that already holds diagnostics gives them back. `Types::check` is unchanged (D3). Create `crates/piperine-lang/tests/types_model.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/model.rs`
**Depends on**: None
**Reuses**: `Checker::new`, `reach`, the worklist of `types/check.rs`
**Requirement**: GN-08
**Executor**: Sonnet

**Done when**:

- [x] `every_item_kind_of_every_crate_is_checked_unreached`, `an_unreached_function_with_an_error_fails_the_model_check`, `an_unplaced_module_calling_ddt_from_a_function_is_e0350`, `an_unreached_item_has_the_code_it_has_when_reached` and `the_standard_library_is_left_to_its_own_check` pass under `**Run typesmodel**`
- [x] `types_subset.rs::unreached_items_are_not_reported` still passes (the reach of one top is unchanged)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the whole-model check`

---

### T2: `Project::check` types a library package and its dependencies

**What**: `Project::check` on a package with no run calls `Types::check_model` on the package's crate graph and reports each diagnostic as a `typeck` failure; `Checked::typed` is true. The `done(0, false)` path leaves.
**Where**: `crates/piperine-api/src/project.rs`
**Depends on**: T1
**Reuses**: `Project::check`, `Failure::Diagnostics`
**Requirement**: GN-08
**Executor**: Sonnet

**Done when**:

- [x] `a_library_package_with_no_run_is_type_checked` and `a_dependency_crate_is_type_checked` pass under `**Run apiproject**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): check types a library package`

---

### T3: `Session::elaborate` runs the model check before the top

**What**: `Session::elaborate` calls `Types::check_model` after the parse and before `Types::check(&design, &top)`; a model error is a `typeck` failure with the same diagnostic `check` gives (GN-09).
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T2
**Reuses**: `Session::diagnosed`
**Requirement**: GN-09
**Executor**: Sonnet

**Done when**:

- [x] `a_run_fails_on_an_unreached_error_with_the_check_diagnostic` passes under `**Run apisession**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a run fails on an unreached item's error`

---

### T4: The parity source runner checks the whole model

**What**: `piperine-parity`'s `Project` runs `Types::check_model` before the top, as `Session::elaborate` does, so a source test sees what a host sees. Any parity fixture with an unreached refused item it surfaces is listed on the Handoff and handled under design.md "Goldens, pins and tests that may move".
**Where**: `crates/piperine-parity/src/project.rs`
**Depends on**: T3
**Reuses**: `Project::diagnosed`
**Requirement**: GN-09
**Executor**: Sonnet

**Done when**:

- [x] `the_source_runner_checks_the_whole_model` passes under `**Run parityproject**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(parity): the source runner checks the whole model`

---

### T5: `piperine check`, `build` and `run` on a library with unreached errors

**What**: Fixture `crates/piperine-cli/tests/projects/unreached_errors/` (a library crate with one error per item kind: a `fn`, a module, a behavior block, a `const`, an `impl` function; and a variant of it with a `[run]` table). `check` prints each diagnostic and exits 1; `build` and `run` exit 1 with the same diagnostic text. The `no_run` expectation `type checking needs a run` moves (design "may move"): the `no_run` package is now typed and prints the new `ok:` line.
**Where**: `crates/piperine-cli/tests/commands.rs`
**Depends on**: T4
**Reuses**: `piperine_in`, `fixture`
**Requirement**: GN-08, GN-09
**Executor**: Sonnet

**Done when**:

- [x] `check_reports_one_error_per_unreached_item_kind` and `build_and_run_fail_with_the_check_diagnostic` pass under `**Run clicommands**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(cli): check, build and run see an unreached item's error`

---

### T6: `GAPS.md` narrows S3 to generic items

**What**: `GAPS.md` S3 keeps only "a generic body is checked per Q1 (c) with X6"; the library-check sentence and the `E0610`/`E0612`/`E0613` note leave once T1 to T5 prove them; §9 Coverage I §3.4–§3.6 is updated.
**Where**: `GAPS.md`
**Depends on**: T5
**Reuses**: `docs.rs`
**Requirement**: GN-31
**Executor**: Sonnet

**Done when**:

- [x] `gaps_narrows_s3_to_generic_items` passes under `**Run docs**`
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: the whole-model check narrows S3`

---

### T7: Rebase and re-read the anchors

**What**: Rebase `feature/generics` onto the current `main` once `composite-values` phases A and B have merged there (if they have not, stop and report). Re-run T1 to T6's gates. Re-read every anchor of design.md "Code Reuse Analysis" and "Components" at the new `HEAD`: `Ty::Struct`, `Ty::Enum`, `Ty::Array`, `Ty::Closure` and their match sites, `Derives`, the function-class inference and `E0122` of composite-values T16, `TypeArena`, `TypeData`, `CompositeLayout`, `TypeItems`, `Variants`' recursion registration, `FunctionRef`/`CallValue`, `ConstEval`'s environment, `Unrolling`, `analog-structure`'s `Frame` and `types/reach.rs`, `Unsupported`'s rows, the interpreter's `form_name`, the next free `MD-` number. Write every difference, and how each later task maps onto it, on the Handoff's `**Settled mid-build:**` line. No code change.
**Where**: `.specs/features/generics/tasks.md` (Handoff only)
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md
**Requirement**: GN-30
**Executor**: Sonnet

**Done when**:

- [ ] The rebase is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test` at the root pass
- [ ] The Handoff names each anchor that moved and the task it affects

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): generics - contract re-read after the rebase`

---

### T8: IR `FunctionData::instance`

**What**: `GenericInstance { item, arguments }` and `FunctionData::instance: Option<GenericInstance>` (`#[serde(default)]`), set through `FunctionSpec`; `print.rs` prints it in the function's header comment; `Ir::check` refuses an instance whose function name is not `item::<arguments>` (plus an optional half suffix, `.is` or `.val`, which `elab/variants.rs` gives each half of an `Option` result; a value has no flag, so there is no flag-pattern suffix). The interpreter refuses a function with an `instance` by name ("a generic instantiation `swap::<f64>`"). Create `crates/piperine-ir/tests/generics.rs` with its `//!` header and `frozen.rs::each_generics_form_is_refused_by_name`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `FunctionSpec`, `IrBuilder::function`, `walk.rs` `form_name`
**Requirement**: GN-02, GN-29
**Executor**: Sonnet

**Done when**:

- [ ] `a_function_instance_is_built_checked_and_printed` and `an_instance_whose_name_disagrees_is_refused` pass under `**Run irgen**`
- [ ] `each_generics_form_is_refused_by_name` passes under `**Run interp**` for a function instance
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): a function records the generic instance it is`

---

### T9: IR `TypeData::instance` and `TypeData::repr`

**What**: `TypeData::instance: Option<GenericInstance>` and `TypeData::repr: ReprForm::{Derived, HandWritten { width, encode, decode }}`, both `#[serde(default)]`; `Ir::check`: a hand-written entry's `encode` takes the type and returns `[bool; width]`, its `decode` takes `[bool; width]` and returns `Option` of the type, `width` agrees with the layout; `print.rs` prints both. The interpreter refuses a hand-written `Repr` type by name.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T8
**Reuses**: composite-values `TypeData`, `IrBuilder::type_data`
**Requirement**: GN-05, GN-06, GN-29
**Executor**: Sonnet

**Done when**:

- [ ] `a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed` and `a_hand_written_repr_with_wrong_signatures_is_refused` pass under `**Run irgen**`
- [ ] `each_generics_form_is_refused_by_name` covers the hand-written `Repr` under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): generic type instances and hand-written Repr`

---

### T10: Rust names of instances

**What**: `Text::spelled` maps `::<` and `<` to `_g`, `, ` to `_c`, `>` to `_e`, `-` to `_m` (D8), its doc comment listing the new marks; the collision refusal applies (`Pair<Pair<f64>, u8>` and `Pair<Pair<f64, u8>>` spell apart). Create `crates/piperine-transpile/tests/generics.rs` with its `//!` header: IR built by hand with instance names compiles through `rustc` and runs.
**Where**: `crates/piperine-transpile/src/print/text.rs`
**Depends on**: T9
**Reuses**: `Text::spelled`, the transpile test support
**Requirement**: GN-02
**Executor**: Sonnet

**Done when**:

- [ ] `instance_names_compile_and_nested_arguments_spell_apart` passes under `**Run tgen**`
- [ ] The inline unit tests of `print/text.rs` pass (`cargo test -p piperine-transpile --lib print::text`)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): Rust names of generic instances`

---

### T11: Plan: the layout of a hand-written `Repr`

**What**: `CompositeLayout` lays a type whose `repr` is `HandWritten { width, .. }` out as one leaf of `width` levels (§2: the encoding is the type's), with no known byte (the ABI has none, MD-78); two pins of one such net get equal layouts.
**Where**: `crates/piperine-plan/src/composite.rs`
**Depends on**: T10
**Reuses**: composite-values `CompositeLayout`
**Requirement**: GN-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_hand_written_repr_is_one_leaf_of_its_width` passes under `**Run plancomp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): layout of a hand-written Repr`

---

### T12: Printer: `Carried` through `encode` and `decode`

**What**: `TypeItems` prints the `rt::Carried` impl of a hand-written `Repr` type through its IR `encode` and `decode` functions: to levels by `encode`, from levels by `decode`; a `None` from `decode` is returned to the caller, as `Option<T>` (§2 rule 9 as MD-78 amends it), and no flag moves with the levels. A one-hot type built by hand in IR round-trips through `rustc`.
**Where**: `crates/piperine-transpile/src/print/types.rs`
**Depends on**: T11
**Reuses**: composite-values `TypeItems`, `rt::Carried`, `rt::LevelWords`
**Requirement**: GN-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller` passes under `**Run tgen**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): Carried through a hand-written encode and decode`

---

### T13: `Ty::Param`, `GenericValue` and `ConstTerm`

**What**: In `types/ty.rs`: `Ty::Param(DeclId)` (opaque; `name` prints the parameter's name; `has_repr` and `levels` answer through its bounds once T19 lands, `None` before), `GenericValue::{Type, Const, Function, Closure}`, `ConstTerm::{Value, Bool, Param, Expr}`; `Ty` derives `Ord` for instance keys. "There is no type variable" in the `Ty` doc comment becomes the rule of Q1 (c). Create `crates/piperine-lang/tests/types_generics.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Ty::name`
**Requirement**: GN-01, GN-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_type_parameter_prints_its_name` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): type parameters and generic values`

---

### T14: `Ty::Struct` and `Ty::Enum` carry their generic arguments

**What**: Refactor composite-values' `Ty::Struct(DeclId)`/`Ty::Enum(DeclId)` to `Ty::Struct(Nominal)`/`Ty::Enum(Nominal)`, `Nominal { decl, arguments }` (empty for a non-generic type), at every match site. No behaviour change.
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: T13
**Reuses**: composite-values `Ty::Struct`, `Ty::Enum`
**Requirement**: GN-06
**Executor**: Sonnet

**Done when**:

- [ ] `cargo test -p piperine-lang --test types_aggregates --test elab_aggregates` and `cargo test -p piperine-parity --test source_aggregates` (composite-values' targets) pass unchanged
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `refactor(types): nominal types carry their generic arguments`

---

### T15: `Ty::Array` carries a `Length`

**What**: Refactor composite-values' `Ty::Array(Box<Ty>, u64)` to `Ty::Array(Box<Ty>, Length)`, `Length::{Fixed(u64), Generic(ConstTerm)}`, at every match site; a `Generic` length is legal only inside a generic item (D14). No behaviour change.
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: T14
**Reuses**: composite-values `Ty::Array`
**Requirement**: GN-01
**Executor**: Sonnet

**Done when**:

- [ ] `cargo test -p piperine-lang --test types_aggregates --test elab_aggregates` and `cargo test -p piperine-parity --test source_aggregates` (composite-values' targets) pass unchanged
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `refactor(types): array lengths may name a const parameter`

---

### T16: `GenericSignature`

**What**: New `types/generics.rs`: `GenericSignature::of(design, item)` reads an item's generic parameters (type, nature, const with its integer or `bool` type), their bounds (`Bound::{Trait, Library, Repr, Nature, Callable}`, the `Fn(A) -> R` sugar from `GenericArgsNode::Paren`) and defaults; refuses a defaulted parameter before an undefaulted one (§3.8.4), a const parameter of another type (§3.8.3), a bound name that is no trait (`E0161`, H.8 intro), and a wrong number of generic arguments at a use (`E0187`). A trait takes type parameters with defaults (`PartialEq<Rhs = Self, Out = bool>`, `Resolve<X, V = X>`): the default of an omitted argument is read here, and `Self` may appear in a default. `Default` is the table of explicit-unknowns D4 (`Defaults`), not a flag-era rule: `bool`, the integers and the floats do not satisfy `T: Default` unless that table says so.
**Where**: `crates/piperine-lang/src/types/generics.rs`
**Depends on**: T15
**Reuses**: `GenericParamData`, `GenericArgsNode`, composite-values `Derives` (H.8 names)
**Requirement**: GN-01, GN-06
**Executor**: Sonnet

**Done when**:

- [ ] `generic_parameter_lists_are_read_with_their_bounds_and_defaults` and `a_wrong_number_of_generic_arguments_is_e0187` pass under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): generic parameter lists`

---

### T17: `Substitution` and written generic types

**What**: New `types/substitution.rs`: `Substitution` (parameter to `GenericValue`), `apply(&Ty) -> Ty`, composition. `lower_type_node` lowers a written use of a generic struct or enum (`Pair<f64>`, `Maybe<T>`), a type parameter (`T` to `Ty::Param`) and an array length naming a const parameter, recording the use's `Instantiation` (`Types::written_instantiation`). The `generic parameter used as a type` refusal leaves.
**Where**: `crates/piperine-lang/src/types/substitution.rs`
**Depends on**: T16
**Reuses**: `Checker::lower_type`, `GenericSignature`
**Requirement**: GN-01, GN-06
**Executor**: Sonnet

**Done when**:

- [ ] `written_generic_types_lower_with_their_arguments` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): substitution and written generic types`

---

### T18: The definition check of a generic function

**What**: `signature_parts` no longer refuses a generic function: its parameters are typed with `Ty::Param`, its `where` is checked as a `bool` const expression over its const parameters (§3 rule 8), its body once against its declared result (Q1 (c)). The `types_subset.rs` row `generic function` moves to this task's test (design "may move").
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T17
**Reuses**: `Checker::function`, `function_body`
**Requirement**: GN-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_generic_body_is_checked_once_at_its_definition` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a generic function is checked at its definition`

---

### T19: What a type parameter's bounds give, and §3 rule 9

**What**: New `types/bounds.rs`: `BoundCheck::satisfies(ty, bound)` (H.8 traits through `Derives`, `Repr` through `Ty::has_repr`, a project trait through the `ImplTable` once T24 lands, `Fn` through a callable's signature); an `impl Checker` block giving a `Ty::Param` the members its bounds offer: H.8 operators (`==` from `PartialEq`, `<` from `PartialOrd`), `Repr`'s `WIDTH`, `encode`, `decode` (no `level`, no flag read: a value has none, MD-78), `.clone()`; `T: Default` holds where the `Defaults` table (explicit-unknowns D4) says; a method no bound gives is `E0161` naming the method, the parameter and the bound (D4); an operator is `P0002` naming the trait. `docs/spec/part_i_values.md` §3 rule 9 gains the sentence of design.md "The two checks of a generic item", with its `spec_text` guard (GN-07 AC 4, MD-57).
**Where**: `crates/piperine-lang/src/types/bounds.rs`
**Depends on**: T18
**Reuses**: composite-values `Derives`, `Defaults` (explicit-unknowns), `Catalogue`
**Requirement**: GN-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_member_no_bound_gives_is_refused_at_the_definition` and `bounds_give_operators_width_and_codecs` pass under `**Run typesgen**`
- [ ] `rule_9_splits_definition_and_instantiation_errors` passes under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a type parameter offers what its bounds give`

---

### T20: `Inference`

**What**: New `types/inference.rs`: at a call of a generic function, explicit turbofish arguments first (`f::<u8>(x)`, §3.8.5), then unification of each parameter type with its argument's type and of the result with the expected type (`call_expected`), a const from an array argument's length, then defaults; a parameter left open is `E0181` naming it (D6). The method-call `generics` refusal (`method x` with a turbofish) leaves.
**Where**: `crates/piperine-lang/src/types/inference.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Substitution`, `Checker::infer`, `check_as`
**Requirement**: GN-01
**Executor**: Sonnet

**Done when**:

- [ ] `generic_arguments_are_explicit_or_inferred` and `an_uninferred_generic_argument_is_e0181_naming_the_parameter` pass under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): inference of generic arguments`

---

### T21: Bounds at a call and the recorded instantiation

**What**: In `types/call.rs` (`function_call`, a dispatch arm into `Inference` and `BoundCheck`): each inferred argument is checked against its bounds (`E0187` naming the argument and the bound); the call's `Instantiation` is recorded (`Types::instantiation(expr)`); the call is typed by the substituted signature and its arguments bound by `bind_arguments`.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T20
**Reuses**: `bind_arguments`, `BoundCheck`
**Requirement**: GN-01, GN-02
**Executor**: Sonnet

**Done when**:

- [ ] `an_argument_that_misses_a_bound_is_e0187` and `each_generic_call_records_its_instantiation` pass under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): bounds at a generic call and its instantiation`

---

### T22: `Fn` bounds take closures and function names

**What**: In `types/call.rs`: an argument for a parameter of type `F` with `F: Fn(A, ..) -> R` may be a capturing closure, a non-capturing closure or a `fn` name whose signature is `(A, ..) -> R` (§3.8.6); it becomes `GenericValue::Closure(expr)` or `GenericValue::Function(decl)` in the instantiation (D9); in the body, `f(x)` on such a parameter is a positional call (§3.2.11; a named argument is `E0140`). A capturing closure passed to a user `fn` (composite-values' remaining refusal) leaves.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T21
**Reuses**: composite-values `Ty::Closure`, `closure_typed`
**Requirement**: GN-01
**Executor**: Sonnet

**Done when**:

- [ ] `fn_bounds_take_closures_and_function_names` and `a_call_through_an_fn_bound_is_positional_only` pass under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): Fn bounds take closures and function names`

---

### T23: `Obligations`: what an instantiation must check

**What**: New `types/obligations.rs`: the checks a definition defers (Q1 (c)): a function's `where`, an array-length equality where a `Length::Generic` meets another length, a const expression built from const parameters; recorded per item (`Types::obligations(item)`), discharged by the elaborator (T33). A `Generic` length never mismatches at the definition.
**Where**: `crates/piperine-lang/src/types/obligations.rs`
**Depends on**: T22
**Reuses**: `Checker::mismatch`
**Requirement**: GN-07
**Executor**: Sonnet

**Done when**:

- [ ] `array_lengths_and_where_are_deferred_to_the_instantiation` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): obligations deferred to each instantiation`

---

### T24: `ImplTable`

**What**: New `types/traits.rs`: `ImplTable::new(design)` collects every trait (items, supertraits), inherent `impl`, trait `impl` and blanket `impl<T: B> Tr for T`; `inherent(ty, name)`, `visible(ty, name, from)` (§3.7.8: in scope, G.67 when two traits give it), `implementation(trait_item, ty)` (the `impl`'s own item, else the trait's default under `Self = ty`), `implements(ty, trait)`. Built once in `Checker::new`; `project_methods` is replaced by it. Create `crates/piperine-lang/tests/types_traits.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/traits.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `impls_in_order`, `impls_of`, `members.rs`'s scope rule
**Requirement**: GN-03, GN-04
**Executor**: Sonnet

**Done when**:

- [ ] `the_impl_table_answers_inherent_visible_and_implementation` passes under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the table of traits and impls`

---

### T25: `self`, methods, `Self` and associated paths

**What**: New `types/methods.rs` (`impl Checker`): a `self` parameter in an `impl` or a trait is the `impl`'s type (`Self`); `self` outside an `impl` or a trait, or not first, is `E0187`; a method call on a value type resolves through `ImplTable::inherent` then `visible` (§3.7.8) and records `Callee::Method`; `Type::f(..)` and `Type::NAME` resolve to associated functions and constants (`Logic::wired`, `Logic::Z`). The refusals `` `self` parameter `` and "method `x` defined by the project" leave; the `types_subset.rs` row "struct with an impl" moves (design "may move").
**Where**: `crates/piperine-lang/src/types/methods.rs`
**Depends on**: T24
**Reuses**: `method_call` (dispatch arm), `ImplTable`
**Requirement**: GN-03
**Executor**: Sonnet

**Done when**:

- [ ] `methods_associated_functions_and_constants_type_check` and `self_outside_an_impl_or_not_first_is_e0187` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): self, methods and associated items`

---

### T26: The declaration rules of `impl`

**What**: New `types/coherence.rs` (`Coherence`), run on every `impl` the check sees (the model check: all of them): a method named `changed`, `prev`, `others` or `inflow` (the carrier methods that remain; `level` is gone with the flag, MD-78) is `E0131` (§3.1.3); an item that is not a function in an inherent `impl` on a nature or bundle is `E0132`; `impl Tr for` a bundle is `E0134` (§3.1.9), while `impl Tr for` a nature is accepted (traits are capabilities, MD-78: `impl Resolve<Driven> for Wire`), its `self` being a handle (phase B) and its items the trait's. A module type takes trait impls too (user, 2026-11-22), but only at GN-21: `E0130` stays the classifier's for an `impl` on a module until then. `E0130` stays the classifier's and gets its test here.
**Where**: `crates/piperine-lang/src/types/coherence.rs`
**Depends on**: T25
**Reuses**: `pom/resolve/classify.rs` (`E0130`)
**Requirement**: GN-03, GN-04, GN-10
**Executor**: Sonnet

**Done when**:

- [ ] `an_impl_on_a_module_is_e0130`, `a_method_named_like_a_carrier_method_is_e0131`, `a_non_function_item_in_an_impl_on_a_nature_is_e0132`, `impl_trait_for_a_bundle_is_e0134` and `impl_trait_for_a_nature_is_accepted` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the declaration rules of impl blocks`

---

### T27: The rules of a trait `impl`

**What**: In `types/coherence.rs`: `impl Tr for T` defines every item of `Tr` without a default and no item `Tr` lacks, each with the trait's signature under `Self = T` (§3.7.4); `T` implements every supertrait (§3.7.3); the `impl` is in the crate of `Tr` or of `T` (§3.7.7, value types and natures; D10); `Tr` may take type parameters with defaults and the `impl` names its arguments (`impl Resolve<Driven> for Wire`), each item checked under them; no `pub` on its items (§3.7.11). Each is `E0187` naming the trait and the type (D5).
**Where**: `crates/piperine-lang/src/types/coherence.rs`
**Depends on**: T26
**Reuses**: `ImplTable`, `Substitution`
**Requirement**: GN-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type` and `extra_items_signatures_supertraits_crate_and_pub_are_e0187` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the rules of a trait impl`

---

### T28: Trait methods on values and through bounds

**What**: In `types/methods.rs`: a trait method on a concrete value records `Callee::Method` of the `impl`'s item or of the trait's default; on a `Ty::Param` through a bound it records `Callee::Bound { item }`; `Tr::f(x)` and `T::f(x)` paths call trait items without the trait in scope (§3.7.8). `BoundCheck` answers project traits through `ImplTable::implements`, a trait with arguments by those arguments too (`W: Nature + Resolve<Driven>` is satisfied by `impl Resolve<Driven> for W`, a nature included). The `types_subset.rs` row `trait` moves (design "may move").
**Where**: `crates/piperine-lang/src/types/methods.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ImplTable::visible`, `implementation`
**Requirement**: GN-04
**Executor**: Sonnet

**Done when**:

- [ ] `trait_methods_resolve_on_values_through_bounds_and_by_path` passes under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): trait methods on values and through bounds`

---

### T29: Blanket `impl`s and overlap

**What**: In `types/traits.rs`: a blanket `impl<T: B> Tr for T` applies to every type that satisfies `B` (§3.7.5; `impl<T: Repr> Known for T {}` included); two `impl`s of one trait that both apply to some type are `E0187` naming the trait and the type, used or not (§3.7.6); the overlap rule compares the trait's arguments too, so `impl Resolve<A> for N` and `impl Resolve<B> for N` do not overlap.
**Where**: `crates/piperine-lang/src/types/traits.rs`
**Depends on**: T28
**Reuses**: `BoundCheck`
**Requirement**: GN-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_blanket_impl_applies_to_every_type_its_bound_admits` and `overlapping_impls_are_e0187` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): blanket impls and overlap`

---

### T30: A hand-written `impl Repr`

**What**: New `types/repr.rs` (`impl Checker`): `impl Repr for T` with `const WIDTH: u32`, `fn encode(self) -> [bool; Self::WIDTH]` and `fn decode(levels: [bool; Self::WIDTH]) -> Option<Self>` (§2); `T` must be `Copy` (§2's supertrait); `Ty::has_repr` and `levels` answer through the `ImplTable` for such a type, so `Bit<T>` accepts it.
**Where**: `crates/piperine-lang/src/types/repr.rs`
**Depends on**: T29
**Reuses**: `ImplTable`, composite-values `Ty::has_repr`
**Requirement**: GN-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_hand_written_repr_type_checks_and_is_carried` and `a_hand_written_repr_missing_an_item_is_e0187` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): hand-written Repr`

---

### T31: Generic structs and enums in the checker

**What**: In composite-values' `types/aggregate.rs`: a struct or enum with type or const parameters is typed per `Nominal`: struct expressions, field access, variant construction and patterns substitute the declaration's field types; a derive holds for the arguments that implement it (H.8 rule 6); `#[derive(Repr)]` on an instance whose argument has no `Repr` is `E0110` naming the argument. The "generic struct/enum" refusal leaves.
**Where**: `crates/piperine-lang/src/types/aggregate.rs`
**Depends on**: T30
**Reuses**: composite-values struct/enum checks, `Substitution`, `Derives`
**Requirement**: GN-06
**Executor**: Sonnet

**Done when**:

- [ ] `generic_structs_and_enums_type_check_per_instance` and `a_generic_repr_type_over_an_argument_without_repr_is_e0110` pass under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): generic structs and enums`

---

### T32: The elaborator reads the type table through the substitution

**What**: `Scope::substitution` (empty outside an instance) and `Elaboration::typed(scope, expr)`, `typed_local(scope, decl)`, `typed_written(scope, ty)`: the type table's answer with the scope's substitution applied. Every elaborator read of `self.types.ty`, `local` and `written` goes through them. No behaviour change. Create `crates/piperine-lang/tests/elab_generics.rs` with its `//!` header and a source-walking test that no direct read remains outside `lower.rs`.
**Where**: `crates/piperine-lang/src/elab/lower.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Scope`, `Substitution`
**Requirement**: GN-01
**Executor**: Sonnet

**Done when**:

- [ ] `the_elaborator_reads_types_only_through_the_substitution` passes under `**Run elabgen**`
- [ ] Every buck target passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(elab): read the type table through the substitution`

---

### T33: `Instantiations`: fold, check and name

**What**: New `elab/instances.rs`: `Instantiations::concrete(scope, instantiation) -> Option<Instance>` folds every argument under the scope's substitution (`ConstEval` with the const parameters bound), discharges the item's `Obligations`: a false `where` is `E0127` naming the instance (`clamp::<u8>`), a length mismatch `P0002` and a panic `E0104`, each naming the item and its arguments (GN-07 AC 3); an instantiation chain past depth 1 024 is `E0341` naming the function (D7); `Instance::name` gives the turbofish spelling of D8.
**Where**: `crates/piperine-lang/src/elab/instances.rs`
**Depends on**: T32
**Reuses**: composite-values `ConstEval` environment, `Obligations`
**Requirement**: GN-01, GN-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_false_where_is_e0127_naming_the_instantiation`, `instantiation_errors_name_the_item_and_its_arguments` and `an_endless_instantiation_chain_is_e0341` pass under `**Run elabgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): fold, check and name an instantiation`

---

### T34: One IR function per distinct instance

**What**: `Variants` is keyed by `(DeclId, Instance, Vec<Flag>)`; `request` takes the call's `Instance`; `lower_function` lowers the body with `Scope::substitution` set and fills `FunctionSpec::instance`; `Variants::name` is the instance's name plus the flag suffix. Equal arguments reach one instance (§3.8.10).
**Where**: `crates/piperine-lang/src/elab/variants.rs`
**Depends on**: T33
**Reuses**: `Variants`, `lower_function`
**Requirement**: GN-01, GN-02
**Executor**: Sonnet

**Done when**:

- [ ] `one_ir_function_per_distinct_instantiation_named_by_its_arguments` and `equal_arguments_make_one_instance` pass under `**Run elabgen**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): one IR function per generic instance`

---

### T35: Methods and associated items in the elaborator

**What**: New `elab/methods.rs` (`impl Elaboration`): `self` lowers as the first parameter; a `Callee::Method` call is a `Call` of the method's variant with the receiver first; `Type::f(..)` a `Call`; `Type::NAME` the folded constant; a generic `impl`'s item is instantiated with the `impl`'s arguments. Source test from MSL with `Point::new`, `p.norm()`, `Logic::Z`.
**Where**: `crates/piperine-lang/src/elab/methods.rs`
**Depends on**: T34
**Reuses**: `request`, `ConstEval`
**Requirement**: GN-03
**Executor**: Sonnet

**Done when**:

- [ ] `methods_associated_functions_and_constants_run_with_rusts_meaning` passes under `**Run srcgen**` (new target `crates/piperine-parity/tests/source_generics.rs` with its `//!` header)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): methods and associated items`

---

### T36: Static trait dispatch

**What**: In `elab/methods.rs`: a `Callee::Bound { item }` call inside an instance resolves with `ImplTable::implementation(item, concrete self)` to the `impl`'s item or the trait's default body (instantiated with `Self` bound), blanket `impl`s included; a trait method on a concrete value calls its `impl` statically; the `impl` is chosen by the trait's arguments as well as the type's.
**Where**: `crates/piperine-lang/src/elab/methods.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ImplTable::implementation`, `request`
**Requirement**: GN-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_trait_method_calls_its_impl_or_its_default_body`, `a_bound_method_calls_the_instances_implementation` and `a_blanket_impl_applies_at_run_time` pass under `**Run srcgen**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): static trait dispatch`

---

### T37: Closures and function names as generic arguments

**What**: New `elab/callable.rs` (`CallableArgument`): a `GenericValue::Function` makes `f(x)` in the instance a direct call of the named function's variant; a `GenericValue::Closure` inlines the closure's body at `f(x)` with its parameters bound to the arguments, its captures copied at the closure's definition (§3 rule 3) and passed as trailing parameters `f.<name>` of the instance (D9); the instance is named with `closure$k` (D8).
**Where**: `crates/piperine-lang/src/elab/callable.rs`
**Depends on**: T36
**Reuses**: composite-values `closure_parts`, `Variants`
**Requirement**: GN-01
**Executor**: Sonnet

**Done when**:

- [ ] `a_capturing_closure_a_plain_closure_and_a_function_name_pass_for_an_fn_bound` passes under `**Run srcgen**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): closures and function names as generic arguments`

---

### T38: Generic types in the type arena

**What**: In composite-values' `elab/types.rs`: `TypeArena` keys an entry by the concrete `Ty` (a `Nominal` with folded arguments), names it `Pair<f64>` with `TypeData::instance`, and lays a derived `Repr` instance out per §2 with the arguments' layouts. Every composite-values construct runs on an instance (GN-06 AC 1).
**Where**: `crates/piperine-lang/src/elab/types.rs`
**Depends on**: T37
**Reuses**: composite-values `TypeArena`
**Requirement**: GN-06
**Executor**: Sonnet

**Done when**:

- [x] `generic_structs_and_enums_instantiate_per_argument_list` and `a_generic_repr_instance_is_laid_out_per_section_2` pass under `**Run srcgen**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): generic struct and enum instances`

---

### T39: A hand-written `Repr` carried end to end

**What**: In `elab/types.rs`: a type with a hand-written `Repr` gets `TypeData::repr = HandWritten { width, encode, decode }`, `encode` and `decode` lowered as IR functions; a carrier of it commits by `encode` and reads by `decode`. Source tests: a one-hot and a Gray-coded `Bit<T>` carry their own layouts; a `decode` returning `None` is returned to the caller as `None` (§2 rule 9 as MD-78 amends it): the carrier holds no unknown level.
**Where**: `crates/piperine-lang/src/elab/types.rs`
**Depends on**: T38
**Reuses**: T9, T11, T12
**Requirement**: GN-05
**Executor**: Sonnet

**Done when**:

- [x] `a_hand_written_repr_carries_its_own_layout` and `a_decode_returning_none_is_returned_to_the_caller` pass under `**Run srcgen**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): hand-written Repr carried end to end`

---

### T40: The Independent Test of phase A

**What**: Source tests of the spec's Independent Test: a generic function instantiated at `u8`, `f64` and a struct, with explicit and inferred arguments, against Rust-evaluated values; a const generic of 0 runs its loops zero times (edge case); the POM after elaboration holds the generic function as written and no instance (GN-02 AC 2, MD-25).
**Where**: `crates/piperine-parity/tests/source_generics.rs`
**Depends on**: T39
**Reuses**: the source-test support of `piperine-parity`
**Requirement**: GN-01, GN-02
**Executor**: Sonnet

**Done when**:

- [x] `generic_functions_run_with_explicit_and_inferred_arguments` and `a_const_generic_of_zero_runs_its_loops_zero_times` pass under `**Run srcgen**`
- [x] `the_pom_keeps_the_generic_function_as_written` passes under `**Run elabgen**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): generic functions at u8, f64 and a struct`

---

### T41: Gallery fences and Book listings that now check

**What**: `std/values.msl` holds `same` and `cast` only (`level` and `merge` are gone with the flag, MD-78; the intrinsics' removal is explicit-unknowns T48's, not this task's). Every Appendix D–F fence and Book listing that phase A lets check (generic functions, `impl`, traits, `Pair<T>`, `apply`) is held to the checker in its existing test (GN-31 AC 2); an expectation of a green `check` over an unreached refused item (S3) becomes that item's diagnostic, listed in `GAPS.md` (design "may move").
**Where**: `crates/piperine-lang/tests/std_galleries.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `std_galleries.rs`, `tests/book/inventory.tsv`
**Requirement**: GN-31
**Executor**: Sonnet

**Done when**:

- [ ] `the_fences_generics_lifted_are_checked` passes under `**Run galleries**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): hold the fences generics lifted to the checker`

---

### T42: Later phases stay refused by name

**What**: A test pins that the forms of phases C, D and E stay `P0001` naming the form: a generic module, generation `for`/`if`, a template value, `m`/`dtemp` bound, a placement through a template (GN-29 AC 2).
**Where**: `crates/piperine-lang/tests/types_generics.rs`
**Depends on**: T41
**Reuses**: `Unsupported`
**Requirement**: GN-29
**Executor**: Sonnet

**Done when**:

- [ ] `phase_c_d_and_e_forms_stay_p0001_by_name` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(types): later generics phases stay refused by name`

---

### T43: Phase A bookkeeping

**What**: `GAPS.md` drops L7, the generic-enum part of L4, S3 and X6; §9 Coverage updates I §3–§3.9 and the generics row; L58 items stay under their group (phase E moves them). The ROADMAP lane row says phase A delivered.
**Where**: `GAPS.md`
**Depends on**: T42
**Reuses**: `docs.rs`
**Requirement**: GN-31
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_generics_phase_a_closed` passes under `**Run docs**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: generics phase A closes L7, S3 and X6`

---

### T44: Handles in the checker

**What**: New `types/handles.rs` (`impl Checker`): a function parameter of a nature or bundle type, and `self` of an `impl` on a nature, is a handle (`Types::handle(param)`); a handle may be read (a discrete one gives its value, §3.1.6), probed (`v(a)`, `temp(self)`), passed to another function and have carrier methods called on it (§3.1.5). The `E0450` of a probe in a function leaves for a probe whose operands are handles (GN-12 AC 3); a probe of anything else in a function keeps it. Create `crates/piperine-lang/tests/types_handles.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/handles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::probe` (`types/analog.rs`), `Ty::Node`, `Ty::Carrier`
**Requirement**: GN-11, GN-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_handle_is_read_probed_passed_on_and_has_carrier_methods_in_the_checker` and `a_probe_of_a_handle_in_a_function_is_not_e0450` pass under `**Run typeshandles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): handles`

---

### T45: `E0133`

**What**: In `types/handles.rs`: a handle returned (a nature-typed result), stored in a value (a `let`, a struct field, a tuple or array element, a closure capture, D12) or written through is `E0133` naming the handle.
**Where**: `crates/piperine-lang/src/types/handles.rs`
**Depends on**: T44
**Reuses**: composite-values `assign.rs` places
**Requirement**: GN-11
**Executor**: Sonnet

**Done when**:

- [ ] `a_handle_returned_stored_or_written_is_e0133` passes under `**Run typeshandles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a handle is never returned or stored`

---

### T46: `impl` on a nature and the lookup order

**What**: In `types/methods.rs`: a method call on a carrier expression follows §3.1.7: the carrier methods, then the `impl`s of its nature, inherent and trait (`impl Resolve<Driven> for Wire` is a capability of the nature; a project nature or a prelude one: `Thermal`, `Electrical`, `Bit<T>`, `impl Bit` being `impl Bit<bool>` by §3.1.8), then the methods of the value read; `self` is a handle. `docs/spec/part_i_values.md` §3.1 gains the Resolution of D10 (an inherent `impl` on a nature may be written in any crate) with its `spec_text` guard.
**Where**: `crates/piperine-lang/src/types/methods.rs`
**Depends on**: T45
**Reuses**: `members.rs`'s §3.1.7 order, `ImplTable`
**Requirement**: GN-10
**Executor**: Sonnet

**Done when**:

- [ ] `a_carrier_method_call_follows_the_lookup_order` passes under `**Run typeshandles**`
- [ ] `an_inherent_impl_on_a_nature_may_be_written_in_any_crate` passes under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): impl on a nature and the lookup order`

---

### T47: Function classes and `E0122`

**What**: New `types/classes.rs` (`FunctionClasses`), extending composite-values T16's class inference: a function is solve-reading when its body probes, reads `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`, or calls a solve-reading function (a fixed point over recursion); a call of it in a const context or a definition is `E0122` naming the function and the position (D13); `analog`, event expressions, reaction bodies, body `let`s and value inputs accept it.
**Where**: `crates/piperine-lang/src/types/classes.rs`
**Depends on**: T46
**Reuses**: composite-values T16's classes, `Context`
**Requirement**: GN-12
**Executor**: Sonnet

**Done when**:

- [ ] `solve_reading_functions_are_accepted_where_the_table_allows` and `a_solve_reading_call_in_a_const_context_is_e0122` pass under `**Run typeshandles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the solve-reading class and E0122`

---

### T48: A function generic over a nature

**What**: In `types/handles.rs`: `fn f<N: Nature>(a: N)` types `a` as a handle of `Ty::Param(N)`; probes and carrier methods are the ones `Nature` gives and a bound such as `W: Nature + Resolve<Driven>` is satisfied by the nature's trait `impl`s (functions and, in phase C, modules); the call's nature is inferred from the carrier passed and recorded in its `Instantiation`.
**Where**: `crates/piperine-lang/src/types/handles.rs`
**Depends on**: T47
**Reuses**: `Inference`, `BoundCheck`
**Requirement**: GN-11
**Executor**: Sonnet

**Done when**:

- [ ] `a_nature_generic_handle_function_type_checks_and_records_its_nature` passes under `**Run typeshandles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): functions generic over a nature`

---

### T49: Handle functions inlined at elaboration

**What**: New `elab/handles.rs` (`impl Elaboration`, `HandleBinding`): a call of a function with a handle parameter is inlined (as `records.rs` inlines an emitting function), its value parameters bound to locals holding the arguments and each handle bound in `Scope::handles` to the caller's terminal or carrier, resolved in the caller's `Frame`; a probe of a handle lowers to the bound terminals, a read of a discrete handle to `Expr::Carrier` with its flags; a recursion through it reached from `analog` unrolls through `Unrolling` or is `E0126`. Create `crates/piperine-lang/tests/elab_handles.rs` and `crates/piperine-parity/tests/source_handles.rs`, each with its `//!` header.
**Where**: `crates/piperine-lang/src/elab/handles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `records.rs` (`inline_emitting`), `Scope::texts`, `analog-structure` `Frame`, composite-values `Unrolling`
**Requirement**: GN-11, GN-29
**Executor**: Sonnet

**Done when**:

- [ ] `a_handle_call_lowers_to_its_written_out_body` passes under `**Run elabhandles**`
- [ ] `a_handle_is_read_probed_passed_on_and_has_carrier_methods` and `a_discrete_handle_reads_the_value_with_its_flags` pass under `**Run srchandles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): handle functions are inlined`

---

### T50: `impl Thermal { fn celsius(self) }` runs

**What**: Source test of the spec's Independent Test: §3.1's `impl Thermal { pub fn celsius(self) -> f64 { temp(self) - 273.15 } }` in a thermal circuit gives `temp - 273.15` at every point of a sweep; a project nature's `impl` runs the same way.
**Where**: `crates/piperine-parity/tests/source_handles.rs`
**Depends on**: T49
**Reuses**: models in `crates/piperine-parity/tests/handles/`
**Requirement**: GN-10
**Executor**: Sonnet

**Done when**:

- [ ] `impl_thermal_celsius_reads_temp_minus_273_15` passes under `**Run srchandles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): impl Thermal celsius`

---

### T51: Probing functions from `analog` and a reaction, and per nature

**What**: Source tests: `fn diff(a: Electrical, b: Electrical) -> f64 { v(a) - v(b) }` called from `analog` and from a reaction gives the solved difference (GN-12 AC 1); `fn f<N: Nature>(a: N)` called with an `Electrical` and a `Thermal` carrier instantiates per nature (GN-11 AC 4).
**Where**: `crates/piperine-parity/tests/source_handles.rs`
**Depends on**: T50
**Reuses**: models in `crates/piperine-parity/tests/handles/`
**Requirement**: GN-11, GN-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_probing_function_runs_from_analog_and_a_reaction` and `a_nature_generic_handle_function_instantiates_per_nature` pass under `**Run srchandles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): probing functions from analog, a reaction and per nature`

---

### T52: Phase B bookkeeping

**What**: `GAPS.md` drops L8 and notes that L53 now waits only on `unknown-semantics-events`; §9 Coverage updates I §3.1 and the handles row; `.specs/STATE.md` gains the next free `MD-` entry of design.md "Tech Decisions"; the ROADMAP lane row says phases A–B delivered.
**Where**: `GAPS.md`
**Depends on**: T51
**Reuses**: `docs.rs`
**Requirement**: GN-31
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_generics_phase_b_closed` and `state_records_the_generics_decision` pass under `**Run docs**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: generics phase B closes L8`

---

## Checks

**C1** - A generic function with trait, `Repr`, project-trait, `Fn` and const parameters runs with explicit and inferred arguments as Rust gives. (GN-01, AC 1; T40)
Proof: run srcgen | test generic_functions_run_with_explicit_and_inferred_arguments ... ok

**C2** - A capturing closure, a plain closure and a function name each pass for an `F: Fn(f64) -> f64` parameter and are called positionally. (GN-01, AC 2; T37)
Proof: run srcgen | test a_capturing_closure_a_plain_closure_and_a_function_name_pass_for_an_fn_bound ... ok

**C3** - A generic argument that cannot be inferred is `E0181` naming the parameter. (GN-01, AC 3; T20)
Proof: run typesgen | test an_uninferred_generic_argument_is_e0181_naming_the_parameter ... ok

**C4** - A `where` false for an instantiation is `E0127` naming the instantiation. (GN-01, AC 4; T33)
Proof: run elabgen | test a_false_where_is_e0127_naming_the_instantiation ... ok

**C5** - Each distinct instantiation is one IR function named from the function and its arguments. (GN-02, AC 1; T34)
Proof: run elabgen | test one_ir_function_per_distinct_instantiation_named_by_its_arguments ... ok
Proof: run irgen | test a_function_instance_is_built_checked_and_printed ... ok

**C6** - The POM keeps the generic function as written, with no instance added to `Design`. (GN-02, AC 2; T40)
Proof: run elabgen | test the_pom_keeps_the_generic_function_as_written ... ok

**C7** - Methods with `self`, associated functions and associated constants of an `impl` on a struct or enum run with Rust's meaning. (GN-03, AC 1; T35)
Proof: run srcgen | test methods_associated_functions_and_constants_run_with_rusts_meaning ... ok

**C8** - An `impl` on a module is `E0130`. (GN-03, AC 2; T26)
Proof: run typestraits | test an_impl_on_a_module_is_e0130 ... ok

**C9** - A method named `changed`, `prev`, `others` or `inflow` is `E0131`. (GN-03, AC 3; T26)
Proof: run typestraits | test a_method_named_like_a_carrier_method_is_e0131 ... ok

**C10** - A trait method calls its `impl` statically, or the default body when the `impl` does not override it. (GN-04, AC 1; T36)
Proof: run srcgen | test a_trait_method_calls_its_impl_or_its_default_body ... ok

**C11** - A bound's method in a generic body calls the instantiation's implementation. (GN-04, AC 2; T36)
Proof: run srcgen | test a_bound_method_calls_the_instances_implementation ... ok

**C12** - A blanket `impl<T: Bound> Trait for T` applies to every type its bound admits. (GN-04, AC 3; T29, T36)
Proof: run typestraits | test a_blanket_impl_applies_to_every_type_its_bound_admits ... ok
Proof: run srcgen | test a_blanket_impl_applies_at_run_time ... ok

**C13** - Two `impl`s of one trait applying to one type, and an `impl` leaving an item without a default, are `E0187` naming the trait and the type. (GN-04, AC 4; T27, T29)
Proof: run typestraits | test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok
Proof: run typestraits | test overlapping_impls_are_e0187 ... ok

**C14** - `impl Trait for` a bundle is `E0134`; for a nature it is accepted. (GN-04, AC 5; T26)
Proof: run typestraits | test impl_trait_for_a_bundle_is_e0134 ... ok
Proof: run typestraits | test impl_trait_for_a_nature_is_accepted ... ok

**C15** - A hand-written one-hot or Gray `impl Repr` carries `Bit<T>` with that layout. (GN-05, AC 1; T9, T11, T39)
Proof: run srcgen | test a_hand_written_repr_carries_its_own_layout ... ok
Proof: run plancomp | test a_hand_written_repr_is_one_leaf_of_its_width ... ok
Proof: run irgen | test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok

**C16** - A hand-written `decode` returning `None` is returned to the caller. (GN-05, AC 2; T39)
Proof: run srcgen | test a_decode_returning_none_is_returned_to_the_caller ... ok

**C17** - A generic struct or enum instantiates per argument list and takes every composite-values construct on each instance. (GN-06, AC 1; T38)
Proof: run srcgen | test generic_structs_and_enums_instantiate_per_argument_list ... ok

**C18** - A generic `#[derive(Repr)]` type is laid out per §2 for an argument with `Repr`, and is `E0110` for one without. (GN-06, AC 2; T31, T38)
Proof: run srcgen | test a_generic_repr_instance_is_laid_out_per_section_2 ... ok
Proof: run typesgen | test a_generic_repr_type_over_an_argument_without_repr_is_e0110 ... ok

**C19** - A generic body is checked once at its definition with its parameters opaque (Q1 (c)). (GN-07, AC 1; T18)
Proof: run typesgen | test a_generic_body_is_checked_once_at_its_definition ... ok

**C20** - A method or operator no bound gives is refused at the definition, naming the missing bound, with nothing instantiating it. (GN-07, AC 2; T19)
Proof: run typesgen | test a_member_no_bound_gives_is_refused_at_the_definition ... ok

**C21** - An instantiation that makes a `where`, an array length or a const expression invalid is reported at the instantiation, naming the item and its arguments. (GN-07, AC 3; T33)
Proof: run elabgen | test instantiation_errors_name_the_item_and_its_arguments ... ok

**C22** - §3 rule 9 says which errors belong to a definition and which to an instantiation. (GN-07, AC 4; T19)
Proof: run spectext | test rule_9_splits_definition_and_instantiation_errors ... ok

**C23** - The model check types every function, module, behavior block, `const` and `impl` of every crate, reached or not, and of the package's dependencies. (GN-08, AC 1; T1, T2)
Proof: run typesmodel | test every_item_kind_of_every_crate_is_checked_unreached ... ok
Proof: run apiproject | test a_dependency_crate_is_type_checked ... ok

**C24** - A library package with no `[run]` table is type-checked whole, each error with the code it has when reached. (GN-08, AC 2; T1, T2, T5)
Proof: run apiproject | test a_library_package_with_no_run_is_type_checked ... ok
Proof: run typesmodel | test an_unreached_item_has_the_code_it_has_when_reached ... ok
Proof: run clicommands | test check_reports_one_error_per_unreached_item_kind ... ok

**C25** - `fn broken(x: f64) -> bool { x + 1.0 }`, called by nothing, fails `piperine check` naming it. (GN-08, AC 3; T1)
Proof: run typesmodel | test an_unreached_function_with_an_error_fails_the_model_check ... ok

**C26** - A module nothing places that calls `ddt` from a function fails the check with `E0350`. (GN-08, AC 4; T1)
Proof: run typesmodel | test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok

**C27** - `piperine run` and `piperine build` fail with the diagnostic `piperine check` gives for an unreached item. (GN-09, AC 1; T3, T4, T5)
Proof: run parityproject | test the_source_runner_checks_the_whole_model ... ok
Proof: run apisession | test a_run_fails_on_an_unreached_error_with_the_check_diagnostic ... ok
Proof: run clicommands | test build_and_run_fail_with_the_check_diagnostic ... ok

**C28** - `impl Thermal { fn celsius(self) }` gives `temp - 273.15` on a thermal carrier. (GN-10, AC 1; T50)
Proof: run srchandles | test impl_thermal_celsius_reads_temp_minus_273_15 ... ok

**C29** - A method call on a carrier is looked up in §3.1.7 order: carrier methods, the nature's `impl`s, the value's methods. (GN-10, AC 2; T46)
Proof: run typeshandles | test a_carrier_method_call_follows_the_lookup_order ... ok

**C30** - A non-function item in an `impl` on a nature is `E0132`. (GN-10, AC 3; T26)
Proof: run typestraits | test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok

**C31** - A handle parameter is bound to the carrier passed and is read, probed, passed on and has carrier methods called. (GN-11, AC 1; T44, T49)
Proof: run srchandles | test a_handle_is_read_probed_passed_on_and_has_carrier_methods ... ok

**C32** - A handle returned, stored in a value or written through is `E0133`. (GN-11, AC 2; T45)
Proof: run typeshandles | test a_handle_returned_stored_or_written_is_e0133 ... ok

**C33** - A discrete handle reads the carrier's value with its flags. (GN-11, AC 3; T49)
Proof: run srchandles | test a_discrete_handle_reads_the_value_with_its_flags ... ok

**C34** - A function generic over a nature instantiates per nature. (GN-11, AC 4; T48, T51)
Proof: run srchandles | test a_nature_generic_handle_function_instantiates_per_nature ... ok

**C35** - A probing function is solve-reading and runs from `analog` and from a reaction. (GN-12, AC 1; T47, T51)
Proof: run srchandles | test a_probing_function_runs_from_analog_and_a_reaction ... ok
Proof: run typeshandles | test solve_reading_functions_are_accepted_where_the_table_allows ... ok

**C36** - A solve-reading call where that class is not allowed is `E0122`. (GN-12, AC 2; T47)
Proof: run typeshandles | test a_solve_reading_call_in_a_const_context_is_e0122 ... ok

**C37** - A probe of a handle in a function body is not `E0450`. (GN-12, AC 3; T44)
Proof: run typeshandles | test a_probe_of_a_handle_in_a_function_is_not_e0450 ... ok

**C38** - The interpreter refuses a generic instance and a hand-written `Repr` by name; a handle call lowers to the IR of its written-out body (D11). (GN-29, AC 1; T8, T9, T49)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run elabhandles | test a_handle_call_lowers_to_its_written_out_body ... ok

**C39** - The forms of phases C, D and E stay `P0001` naming the form. (GN-29, AC 2; T42)
Proof: run typesgen | test phase_c_d_and_e_forms_stay_p0001_by_name ... ok

**C40** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; every phase)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C41** - The build gives no warning on the crates this feature touches, and the hygiene guard passes. (GN-30, AC 2; every phase)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

**C42** - `GAPS.md` narrows S3, then no longer lists L7, the generic-enum part of L4, S3, X6 and L8, and §9 is updated. (GN-31, AC 1; T6, T43, T52)
Proof: run docs | test gaps_narrows_s3_to_generic_items ... ok
Proof: run docs | test gaps_drops_what_generics_phase_a_closed ... ok
Proof: run docs | test gaps_drops_what_generics_phase_b_closed ... ok

**C43** - Every gallery fence and Book listing phase A lets check is held to the checker. (GN-31, AC 2; T41)
Proof: run galleries | test the_fences_generics_lifted_are_checked ... ok

**C44** - A generic item instantiated twice with equal arguments is one instance. (Edge case; T34)
Proof: run elabgen | test equal_arguments_make_one_instance ... ok

**C45** - A const generic of 0 elaborates, its loops running zero times. (Edge case; T40)
Proof: run srcgen | test a_const_generic_of_zero_runs_its_loops_zero_times ... ok

**C46** - An instantiation chain that never ends is `E0341` naming the function (D7). (Edge case; T33)
Proof: run elabgen | test an_endless_instantiation_chain_is_e0341 ... ok

**C47** - The elaborator reads the type table only through the scope's substitution, so no `Ty::Param` reaches the IR. (GN-01, AC 1; T32)
Proof: run elabgen | test the_elaborator_reads_types_only_through_the_substitution ... ok

**C48** - Instance names compile to Rust and nested argument lists spell apart. (GN-02, AC 1; T10)
Proof: run tgen | test instance_names_compile_and_nested_arguments_spell_apart ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (B1, 2026-11-20, user AFK, default taken) the whole-model check skips generic items (an item or an enclosing item with a `<..>` list) and trait functions until the generic forms land: a generic body stays refused `P0001` when reached, and GAPS S3 keeps it (T6). Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("Tu sempre assume o default mais razoável e depois me conta"); the defaults are design.md D1 to D17.
**Settled mid-build (T7, B2, 2026-11-22):** the contract was brought in by `git merge origin/main` (commit `chore: merge main`), not a rebase, so B1's commits keep their ids. Merge by hunk: GAPS.md and ROADMAP.md take main's tables and keep the Coverage row that names S3 as the unreached generic item; docs.rs keeps both sides; `types/mod.rs` registers `model`. Moved by main, not by this branch: an unreached `f64 + 1.0` in a `bool` function is `E0180` (it was `P0002`) in `types_model`, the api project/session tests, the cli commands test and the parity project test; the `gaps_narrows_s3_to_generic_items` test ends the S3 entry at the next `---` (S6 left GAPS); two Book tests (`a_function_has_the_class_its_body_gives` with `values#8`, `the_generic_listings_are_refused_where_a_top_reaches_them` with `generics#9`) run the listing without the item the whole-model check now refuses (`handshake`, `mid_ramp`). The model check also skips a method with a `self` parameter (refused `P0001` until T25 lands methods), as it skips generic items; T25 removes that skip. Anchors at the new head: `Ty` has `Struct(DeclId)`, `Enum(DeclId)`, `Array(Box<Ty>, u64)`, `Fn(Vec<Ty>, Box<Ty>)` (there is no `Ty::Closure`: a closure and a function name are `Ty::Fn`), `Range`, `Iterator`; T13 to T15 map `Ty::Closure` to `Ty::Fn` and make `Array`'s length a `Length`. `elab/variants.rs` names one IR function per `fn` and `Half` (`.is`, `.val` for an `Option` result): there is no flag pattern (`name~01`), so T8, T33 and T34 give names `item::<args>` plus that optional half suffix. The ABI has no known byte (T11), the checker has no `level`/`known_or`/`Unknown`, and `Default` is the table of explicit-unknowns D4. Next free `MD-` number: MD-80 (MD-79 is the last). Amended tasks (explicit-unknowns impact section, and the user's decisions of 2026-11-22): T8 (name suffix), T11 (no known byte), T12 (`None` from `decode` is returned to the caller; test `a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller`), T16 (trait type parameters with defaults; the `Default` table), T19 (no `level`, no flag reads; `Default` by D4; test `bounds_give_operators_width_and_codecs`), T26 (`E0131` lists four names; `impl Tr for` a nature is accepted, `E0134` for a bundle; tests `impl_trait_for_a_bundle_is_e0134` and `impl_trait_for_a_nature_is_accepted`; `E0130` for an `impl` on a module stays until GN-21, where a module type takes trait impls, the user's "looks like a Mosfet, behaves like a Mosfet"), T27 (arguments of a trait in an `impl`; orphan rule covers natures), T28 and T29 (bounds and overlap compare the trait's arguments), T36 (dispatch by the trait's arguments), T39 (`None` from `decode` returned; test `a_decode_returning_none_is_returned_to_the_caller`), T41 (`std/values.msl` holds `same` and `cast`; their intrinsics' removal is explicit-unknowns T48's), T46 and T48 (nature trait impls in the lookup; `W: Nature + Resolve<Driven>`); checks C14 and C16 and spec.md GN-03 AC 3, GN-04 AC 5 and GN-05 AC 2 follow. design.md's remaining flag-pattern wording (the Variants row, D15) is read through this line.
**Settled mid-build (T8 to T12, B2, 2026-11-22):** an instance is marked with `IrBuilder::mark_instance(function, instance)` after `function`, not through a `FunctionSpec` field (181 literals would change for one optional field); a type's instance is the `TypeData::instance` field (its 20 literals took `instance: None, repr: Default::default()`) and a hand-written repr is set with `IrBuilder::set_repr(type, repr)` once its `encode` and `decode` exist (they mention the type). `GenericInstance::name()` is `item::<a, b>` for a function and `type_name()` is `item<a, b>` for a type; `Ir::check` also holds a type instance to its `type_name()`. A hand-written entry's `width` must be at least one level and the array length of `encode` and `decode`; the layout of T11 is one unsigned-integer leaf of `width` bits (a `width` over 64 is a type no pin holds, and a constant of such a type has no bytes). The printer (T12) writes a `Carried` of `WIDTH` = the narrowest 1, 2, 4 or 8 bytes, `LEVELS` = `width`, level `k` = bit `k`; `try_from_levels` returns the author's `decode` result as is (a `None` goes to the caller), while `from_levels` and the byte `decode`, which cannot fail, give the type's default as an enum does for a bad tag; the IR `encode` and `decode` functions must be pure (an environment read is `Unsupported`) and may not be named with `::` (a name passes `Text::ident`; T39 names them `Type.encode`, printed `type_dencode`). The interpreter refuses an instance as ``a generic instantiation `swap::<f64>` `` and a type as `type T holds a hand-written Repr`. Two interp tests stale on main were fixed in T8 (`the_level_nodes_are_refused_by_name`'s `cast` is typed `Option<u64>`; `Structured::refused` cuts at `: `). Book rows flipped by T8 to T12: none (no listing runs on IR names, a layout or a printer); the hand-written `Repr` and generic listings flip at T39 and T40.
**Settled mid-build (T13 to T19, B3, 2026-11-22):** `Ty::structure`, `Ty::enumeration` and `Ty::array` build the plain types and a match site that needs a number reads `Length::Fixed(n)` (a generic length falls to the arm that refuses what is not an array of a known length until the elaborator substitutes, T32); a match site that needs only the declaration reads `Nominal { decl, .. }`. `GenericSignature::of(design, item)` is public and reads the POM only (`Bound::Library(&'static str)` names an H.8 trait; `Bound::Callable` keeps the written type ids, lowered by `Checker::callable_type` where a call or a bound is checked); the rules of a list are held by `Checker::generic_signature` (E0187 for a default before a non-default and for a const parameter that is neither an integer nor `bool`, both added to the E0187 row and the §3.8 error table of `part_i_values.md`; E0161 for a bound that names no trait). A written use of a generic struct or enum (`Checker::generic_nominal`) counts, lowers and defaults its arguments and checks each against its parameter's bounds (E0187 naming the argument and the bound); `Types::written_instantiation(type node)` derives from the written-type table; the fields of a generic type are not typed until T31, so a struct expression or a field read of a generic struct stays `P0001` and the written type is accepted in signatures and `let`s. A call or a function value of a generic function stays `P0001` ``generic function`` (the signature refusal moved from `signature_parts`, which the definition check uses, to `signature`, which uses go through) until T20 and T21; `types_subset.rs`'s generic function row therefore still holds. The whole-model check seeds a generic function that is not nested in another generic item (a generic module, struct, enum or a method of a generic `impl` stays left out). A const parameter read as a value has its declared type. `P0002` no longer exists on main (the type errors it named are `E0180`), so an operator a bound does not give is `E0180` naming the trait and the bound, and a method is `E0161`; `.clone()` is not given by any bound (§1.4.6: a value is copied; main answers it `E0161` with the copy help on every type) and the T19 text that listed it is superseded. `T::WIDTH` of a parameter with no `Repr` bound does not resolve in the POM (`E0161`); with it, the type is `u32` and `ConstTerm::Width(param)` (added here, beside the four of the design) is the length of `x.encode()` and of the argument of `T::decode`. A value of a type parameter bound by `Fn(A) -> R` is called through the bound (the POM classes that call as a placement because a module parameter has the same bound; the checker reads it as a function call when the callee's type is such a parameter). A bound that names a project trait is satisfied by a type parameter that has it until `ImplTable` (T24) answers for concrete types. The pin `types_aggregates.rs` held for the written generic struct (`P0001`) is now the acceptance of the written type. `Ty::fits` and `Length::fits` are the compatibility the definition check uses (equal types, where a length built from const parameters agrees with any other length until the instantiation compares it by value, T23 records the obligation); they replace `==` at the four assignment-compatibility sites (a function result, a checked expression, a let's annotation, an assignment). A bare `[x; N + 1]` repeat takes a generic length. Book rows that stay refused but changed message with this batch (tests moved, each still refused, none flipped to run): cheatsheet#10 (now `generic function`), generics#5 and the `width::<4>` smallest model (now `generic function`), natures#8 (a `Stream<u8>` port, `value input port of type struct Stream<u8>`), and the values#20 command listing, which lost its last diagnostic (`generic parameter used as a type`, so `typeck failed with 5 diagnostic(s)`; `inventory.tsv` carries the new fingerprint). Book rows flipped to run by B3: none.
**Settled mid-build (T20 to T27, B4, 2026-11-23):** a call of a generic function is typed by `Checker::generic_call` (`types/inference.rs`): turbofish arguments (fewer than the list are allowed, more is `E0187`), then unification of parameter types with argument types (an unsuffixed integer literal is read after the others, so it takes the type another argument gave), of the result with the expected type, a const parameter that is a whole array length from the array, the defaults, else `E0181` naming the parameter; each argument is checked against its bounds (`E0187`) and `Types::instantiation(call)` is the recorded `Instantiation` (a closure or a `fn` name passed for an `Fn` bound is `GenericValue::Closure(expr)` or `Function(decl)`, D9; a capturing closure passes for such a parameter, which is not a function pointer). The substituted signature folds the lengths the arguments decide (`Checker::substitute`), so `dot` of three and four elements is `E0180` at the call. A function used as a value (`let h = g;`) stays `P0001 generic function`; the three `types_subset`/`types_buck`/`types_values` pins of `g(1.0)` use `let h = g;`. `Types::obligations(item)` holds what a definition defers (`Obligation::{Where, SameLength, Constant}`, with a file and a span instead of the `ExprId` the design drew, since a function's result has no expression); `Ty::fits` plus `Checker::compatible` record each pair of lengths a generic length met. `ImplTable` (`types/traits.rs`) is public, built once in `Checker::new` (`Rc`), and answers `inherent`, `visible` (no scope filter: every trait of the design is visible and the first impl in declaration order wins a name two traits give, G.67), `implementation`, `implements`, `supertraits`; a blanket `impl` asks a `BoundOracle`. A method call on a struct or an enum resolves through the table before the Appendix H catalogue, on any other receiver after a catalogue miss; the receiver stands for `Self` of a trait's function; `self` in a trait is `Ty::Param(trait)`. `Callee::Method(function)` is typed; the elaborator refuses it by name ("a method call") until T35. `Coherence` runs on every `impl` (the model check seeds them all, generic ones included): `E0131`, `E0132` (a constant in an `impl` on a nature or a bundle), `E0134` (bundle only), and for a trait impl the `E0187` family of T27 (missing or extra item, signature under `Self = T` and the trait's arguments, a supertrait not implemented, an `impl` in a crate that declares neither the trait nor the type, `pub` on an item). `E0131`, `E0132`, `E0134` left `code_owners.tsv`. The pins `struct with an impl`, `trait` (types_subset) and `a user trait and impl` (types_buck) left their tables for `types_traits.rs`. `types/mod.rs` groups its re-exports (hygiene ceiling of 60 lines).
**Settled mid-build (T28 to T35, B5, 2026-11-23):** a trait's method on a concrete value records `Callee::Method` of the impl's item or the trait's default; through a bound of a type parameter, and on a trait's own `Self`, it records `Callee::Bound { item }` (the trait's function, resolved per concrete type by T36); `Tr::f(x)` and `T::f(x)` call by path with the first argument as `self`. `Bound::Trait` keeps its written arguments (`Bound::Trait { decl, arguments }`), met by an impl that names the same arguments. The helpers `types_generics` and `types_traits` both defined moved to `tests/support/generic_sources.rs` (`GenericSources`). Two impls of one trait that share a type and the trait's arguments are `E0187` at the later one (a blanket impl and a scalar its bound admits overlap; a blanket impl and a type its bound refuses do not); a struct or enum with no such method is `E0161` (the project's methods are typed, so a miss is absent). `impl Repr for T` is an `ImplEntry` with `builtin_trait: Some("Repr")` (the language's own trait has no declaration): it must define `WIDTH`, `encode`, `decode` and no other item, each with the signature of §2; the type then has `WIDTH` levels (`hand_written_levels` folds the impl's const), so `Bit<T>` accepts it and `T::WIDTH` and `x.encode()` read it; the `Copy` supertrait the task text names is not in §2 and is not checked. A generic struct or enum is typed over its own parameters and each use carries its arguments (`Nominal`); a constructor finds them from its fields then the expected type (`call_expected` now also reaches struct expressions and names), `E0181` if nothing gives one; a derive holds for an instance whose arguments implement it, `#[derive(Repr)]` over an argument without `Repr` is `E0110`. The elaborator reads the type table through `ConstEval::typed`, `typed_local`, `typed_written` (the substitution lives in `ConstEval` so the constant folder, which has no scope, shares it; the design drew it on `Scope`), and `elab_generics.rs` fails on a raw read outside `consts.rs`. `elab/instances.rs` folds a call's recorded instantiation under the enclosing substitution, discharges the obligations (`E0127` naming `clamp::<1>`, `E0180` for two lengths that differ, `E0104` for a const expression that does not fold; `Types::fold_with` folds a length over the arguments), and `Obligation::Constant` carries its expression; `Variants` is keyed by `(function, arguments, half)`, an instance is named `item::<args>` and records `GenericInstance` in the IR, a chain deeper than 1 024 is `E0341` (each body lowers on `stack::wide` so a debug build does not overflow). A method call lowers as a `Call` of the method's variant with the receiver first; a function a trait declares runs for the receiver's type (its one argument, bound to `Self`); a method of a generic impl and `Callee::Bound` stay refused by name until T36. `ConstEval` evaluates a method of the project. `E0127` joins `E0131`, `E0132` and `E0134` out of `code_owners.tsv` and GAPS section 10 (30 of 189). On-demand buck targets: `buck_dpwm`, `buck_control`, `buck_supervisor`, `buck_boundary` pass; `buck_whole`, `buck_power` and `buck_closed_loop` index a record's second byte (the known byte, gone from the ABI) and `tests/buck_scenarios.rs` does not compile against `Option<i128>`: these come from main's explicit-unknowns change, not from this feature, and no assertion moved.
**Settled mid-build (T36 to T40, B6, 2026-11-23):** a call through a bound (`Callee::Bound { item }`, from a method call or `Tr::f(x)` / `T::f(x)`) resolves in the instance, where the receiver's type is concrete (`ConstEval::typed`), with `ImplTable::implementation` to the impl's item or the trait's default body (`Self` bound, instance argument the receiver's type); the oracle that chooses among blanket impls is `Elaboration::bound_holds` (a trait bound is met through the table, every other bound was judged by the checker). When two impls give one trait for a type the first applies (the trait arguments do not yet choose at run time, `Resolve<A>` versus `Resolve<B>` through a bound stays for a later task). The check reaches every impl item of a trait function called through a bound (`ImplTable::implementations_of`), so their bodies are typed. A method of a generic `impl` is instantiated with the impl's arguments, found by `Types::impl_arguments` taking the impl's self type apart against the receiver (`impl<T: Shape> Describe for T` meeting `Sq` gives `T = Sq`); an `impl`'s parameters inherit the bounds of the type's declaration (`impl<T> Window<T>` with `Window<T: PartialOrd>`). IR names: an item of an `impl` with no parameters is `Type.name` (two types giving `area` stay two functions); items of a generic `impl` are told apart by their instance arguments; a generic instance name prints lowercase in Rust (`doubled::<Sq>` is `doubled_gsq_e`). T37 deviates from D9 (see its amendment in design.md): the callable is a function value, not inlined, and the instance name is `closure$<item>$k`. T38: `TypeArena` keys an entry by `Nominal` (declaration and folded arguments); a generic declaration has no entry of its own, each instance is `Pair<f64>` with `TypeData::instance`, a path `Pair<f64>` and fields from `Types::instance_fields`; the instances a non-generic body holds are registered with the arena (`Types::generic_instances`), those a generic body holds when its instance is lowered (`register_function_types`); the Rust name of an instance type is upper camel (`PairGf64E`). Book rows flipped to run: types#7, types#16, types#17, generics#1, generics#2, generics#4, generics#9 and the claim generics~872b633d holds; `generics#5` and the `width::<4>` smallest model are covered by the const-as-value change recorded in the commit that makes them run. `buck_whole`, `buck_power`, `buck_closed_loop` stay red from main's explicit-unknowns change; the coordinator has them fixed on main. T39 (design D18): a hand-written `Repr` type's initial value is the levels word `encode` gives for the folded initializer (`Constant::Int`), `Ir::carried` of a hand-written enum is `UInt(width)`, the checker reaches the `impl Repr` items when a carrier of the type is checked, the transpiler prints `encode`/`decode` for every hand-written type, and the elaborator lowers the author's `decode` as `Type.decode.is`/`.val` packed into one `Type.decode`. Const generics: a const parameter read as a value and an array length written as an expression over the const parameters (`[f64; N + 1]`, `[0.0; N + 1]`) fold per instance in `ConstEval` (`resolved`, `integer`), so generics#5 (`dot`, `mean_of`, `widened`) and the `width::<4>` smallest model now run; the status box of chapter 4 (`docs/book/content.html`, `generic functions, generic structs, const parameters`) still lists them and T41 rewrites it. T40: the Independent Test is `functions.msl` (`pick<T>` at `f64`, `u8`, a struct, inferred and `pick::<T>`, `count::<0>`, `table_total` of an empty table) and the POM test. Open transpiler gaps found and not fixed (outside this batch): an unused loop variable (`for k in 0..N` with `k` unread) and an unused `Some(_)` payload of a matched `Option` call each print a Rust `unused variable` warning the parity runner refuses, so the models read the variable.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 12

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6
Phase 2:  T7
Phase 3:  T8 ------→ T9 ------→ T10 -----→ T11 -----→ T12
Phase 4:  T13 -----→ T14 -----→ T15 -----→ T16 -----→ T17 -----→ T18 -----→ T19
Phase 5:  T20 -----→ T21 -----→ T22 -----→ T23
Phase 6:  T24 -----→ T25 -----→ T26 -----→ T27
Phase 7:  T28 -----→ T29 -----→ T30 -----→ T31
Phase 8:  T32 -----→ T33 -----→ T34 -----→ T35
Phase 9:  T36 -----→ T37 -----→ T38 -----→ T39 -----→ T40
Phase 10: T41 -----→ T42 -----→ T43
Phase 11: T44 -----→ T45 -----→ T46 -----→ T47 -----→ T48
Phase 12: T49 -----→ T50 -----→ T51 -----→ T52
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B1 | 1 | T1 to T6 | **may run before `composite-values` merges**, once `analog-structure` is on `main`; closes S3 for non-generic items (GN-08, GN-09) |
| B2 | 2, 3 | T7 to T12 | starts after `composite-values` A–B is on `main`; T7 rebases B1 |
| B3 | 4 | T13 to T19 | T14 and T15 are pure refactors of composite-values' types |
| B4 | 5, 6 | T20 to T27 | |
| B5 | 7, 8 | T28 to T35 | T32 is a pure refactor gated by every buck target |
| B6 | 9, 10 | T36 to T43 | closes phase A (`GAPS.md` L7, L4 generic enums, S3, X6) |
| B7 | 11 | T44 to T48 | phase B starts |
| B8 | 12 | T49 to T52 | closes phase B (L8) and records the `MD-` entry; `unknown-semantics-events` may then start |

**Later batch boundaries (not tasked here):** phases C, D and E (GN-13 to GN-28, ROADMAP item 11) run after `composite-values` phase C, from `design-c.md`/`design-d.md`/`design-e.md` and their tasks files (design.md "Phases C, D and E").

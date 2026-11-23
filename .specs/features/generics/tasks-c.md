# Generics Tasks (phase C: generic modules and generation)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-c.md`
**Status**: Draft
**Approved**: (written when the user approves)
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesmods**: `cargo test -p piperine-lang --test types_generic_modules`
**Run typesgenr**: `cargo test -p piperine-lang --test types_generation`
**Run typesgen**: `cargo test -p piperine-lang --test types_generics`
**Run typesmodel**: `cargo test -p piperine-lang --test types_model`
**Run typestraits**: `cargo test -p piperine-lang --test types_traits`
**Run elabgen**: `cargo test -p piperine-lang --test elab_generics`
**Run elabmods**: `cargo test -p piperine-lang --test elab_generic_modules`
**Run elabgenr**: `cargo test -p piperine-lang --test elab_generation`
**Run irgen**: `cargo test -p piperine-ir --test generics`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run tgen**: `cargo test -p piperine-transpile --test generics`
**Run srcgen**: `cargo test -p piperine-parity --test source_generics`
**Run srcmods**: `cargo test -p piperine-parity --test source_generic_modules`
**Run srcgenr**: `cargo test -p piperine-parity --test source_generation`
**Run apisession**: `cargo test -p piperine-api --test session`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookcorestd**: `cargo test -p piperine --test book_core_std`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Base and order.** Phase 13 (T53 to T58) starts from `main` as it is. T59 onward starts after `composite-values`
phase C is on `main`; T59 merges it and stops the batch if it is not there. The tasks of phases D and E are
`tasks-d.md` and `tasks-e.md`; the feature closes (check_proofs, the Verifier) only after `tasks-e.md`'s last task.

Rules every task follows (the user's rules, repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, the lane letter the coordinator gives. `/var/tmp/piperine-target` is a read-only trap and is never used. Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-21).** After each task: the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, then the `Run` lines its `Done when` names. Once at the end of each batch: `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves.
- **Only design-c.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; a divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same expression evaluated by Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules).** Readable at a glance; the module tree says where each thing is; each crate one purpose; **no free function, tests included** (every function has an owner type: a test helper is a method of a support type); **one purpose per type**; **OO names without abbreviations**; **no god method** and no needless abstraction; the data path evident; **a `///` doc comment on every new type, method, field and variant**, and a non-evident block cites its spec rule. No macro; no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design-c.md "Components" names, never into `types/expr.rs`, `types/call.rs` or `print/value.rs` beyond a dispatch arm.
- **Conventions.** No new IR `Expr`/`Stmt` variant; new IR fields carry `#[serde(default)]`. The interpreter refuses every new IR marker by name (R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models live in `crates/piperine-parity/tests/generic_modules/*.msl` and `tests/generation/*.msl`.
- **Book rows.** A Book test that asserts a refusal and starts to run flips in the task that makes it run: its test becomes the acceptance, `tests/book/inventory.tsv` takes `run` (or `holds`), the `GAPS.md` §11 row leaves, and the Book's own "today it refuses" prose is rewritten to what the binary now prints. A row that still needs another feature's form is narrowed to that form, and its §11 row takes that gap id and owner (recorded on the Handoff).
- **Cross-lane files.** `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `ROADMAP.md` rows of other features are not edited.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-38, MD-57, MD-63, MD-71, MD-81; phases A and B's `tasks.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each code with its span and the names it gives | `crates/piperine-lang/tests/{types_generic_modules,types_generation,types_generics,types_model}.rs` | `cargo test -p piperine-lang --test types_generic_modules` / `--test types_generation` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC; instance identity and names; each code; the POM untouched | `crates/piperine-lang/tests/{elab_generic_modules,elab_generation,elab_generics}.rs` | `cargo test -p piperine-lang --test elab_generic_modules` / `--test elab_generation` |
| IR (`piperine-ir`) | integration | the new field built, checked (valid and refused), printed | `crates/piperine-ir/tests/generics.rs` | `cargo test -p piperine-ir --test generics` |
| Interpreter refusals | integration | each new marker refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Printer (`piperine-transpile`) | integration | instance names compile and do not collide | `crates/piperine-transpile/tests/generics.rs` | `cargo test -p piperine-transpile --test generics` |
| Source-level runs | integration | one test per run-time AC on the Rust backend, expected values computed by Rust in the test | `crates/piperine-parity/tests/{source_generics,source_generic_modules,source_generation}.rs` | `cargo test -p piperine-parity --test source_generic_modules` / `--test source_generation` |
| Host (`piperine-api`) | integration | structural override and refusal | `crates/piperine-api/tests/session.rs` | `cargo test -p piperine-api --test session` |
| Book | integration | each owned row flipped to its acceptance | `tests/book/*.rs`, `tests/book/inventory.tsv` | `cargo test -p piperine --test book_<chapter>` |
| Repository records | integration | one assertion per removed `GAPS.md` row or decided fact | `crates/piperine-cli/tests/docs.rs`, `crates/piperine-lang/tests/std_galleries.rs` | `cargo test -p piperine-cli --test docs` |
| Contract re-read (T59) | none | - (build gate; findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After every task with tests | the `**Build**` line; `cargo test -p <crate>` for each crate the task touched; the `Run` lines its `Done when` names |
| Full | The last task of each batch | Quick, then `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets |
| Build | A task with `Tests: none` | the `**Build**` line, then `cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 13: The `Repr` residuals (L70, GN-32; may run before `composite-values` C merges)

```
T53 → T54 → T55 → T56 → T57 → T58
```

### Phase 14: Contract and the checker's generic modules

```
T59 → T60 → T61 → T62 → T63 → T64
```

### Phase 15: Generic module instances

```
T65 → T66 → T67 → T68 → T69 → T70 → T71
```

### Phase 16: Generation in a body

```
T72 → T73 → T74 → T75 → T76 → T77
```

### Phase 17: Generation in a `digital` block and phase C closure

```
T78 → T79 → T80 → T81 → T82
```

---

## Task Breakdown

### T53: `encode` and `decode` of every derived layout through the level cast

**What**: `elab/levels.rs` lowers `x.encode()` at `f64`, a signed integer, a derived struct, enum, tuple and array as `Expr::LevelCast` of `x` to `[bool; WIDTH]` (its `Option` unwrapped: every level pattern decodes to a `[bool; WIDTH]`), and `T::decode(levels)` into those types as `Expr::LevelCast` of `levels` to `T` (the `Option<T>` is the result, §2 rule 9); the two `P0001` refusals of L70 leave. The elaboration test replaces `encode_and_decode_of_a_derived_layout_are_refused_naming_the_call`, and `tasks.md` check C1's proof line names the new test.
**Where**: `crates/piperine-lang/src/elab/levels.rs`
**Depends on**: None
**Reuses**: `Expr::LevelCast`, `Typing::level_cast`, `Elaboration::with_hand_written_widths`
**Requirement**: GN-01 (AC 1 residual, GAPS L70)
**Executor**: Sonnet

**Done when**:

- [x] `encode_and_decode_of_every_layout_lower_through_the_level_cast` passes under `**Run elabgen**` (one case per layout: the IR holds a `LevelCast` of the right `to` and `Ir::check` accepts it)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): encode and decode of every layout through the level cast`

---

### T54: L70 runs on the Rust backend

**What**: Source test: a generic `roundtrip<T: Repr>(x: T) -> Option<T> { T::decode(x.encode()) }` and `levels_of<T: Repr>` instantiated at `f64` (`-1.5`, `inf`), `i8` (`-3`), a derived struct of three fields, a payload-free enum, a tuple and an array give back their value, and `encode` gives the levels Rust computes in the test (`f64::to_bits`, two's complement, field order of §2 rule 3); a `decode` of a tag no variant names is `None`. `GAPS.md` drops L70 (and its §10/§9 mentions), with the `docs.rs` assertion. A `rt::Carried` gap for `[bool; W]` found here is fixed in `crates/piperine-transpile/src/rt/` in this task.
**Where**: `crates/piperine-parity/tests/source_generics.rs`
**Depends on**: T53
**Reuses**: `crates/piperine-parity/tests/generics/*.msl`
**Requirement**: GN-01 (AC 1 residual), GN-31
**Executor**: Sonnet

**Done when**:

- [x] `encode_and_decode_run_at_every_layout` passes under `**Run srcgen**`
- [x] `gaps_drops_l70` passes under `**Run docs**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): encode and decode at every layout; GAPS drops L70`

---

### T55: `cast` and `same` compile from `std/values.msl`

**What**: Remove `Intrinsic::Cast` and `Intrinsic::Same` (`types/intrinsic.rs`, `elab/intrinsics.rs`, the `lower_cast` path) so `cast<A: Repr, B: Repr>` and `same<T: Repr>` type and lower from their source in `std/values.msl` through phase A's generic calls and T53's `encode`/`decode`; the `cast` to a tuple or an array holding an enum then runs. A differential test runs the source-compiled `cast` and `same` against the values the removed lowering gave (the same models, values computed in Rust: `Some` for a tag a variant names, `None` otherwise, level-by-level equality for `same`). The work moved here from `explicit-unknowns` phase B T48 (user, 2026-11-23); `GAPS.md` L58 drops its `cast`/`same` sentence, with the `docs.rs` assertion, and `explicit-unknowns/tasks.md` T48's Handoff notes the move.
**Where**: `crates/piperine-lang/src/types/intrinsic.rs`
**Depends on**: T54
**Reuses**: `std/values.msl`, `Checker::generic_call`, `elab/levels.rs`
**Requirement**: GN-27 (AC 1: `cast`, `same`), GN-01 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `cast_and_same_from_source_agree_with_the_removed_lowering` and `a_cast_to_a_tuple_holding_an_enum_runs` pass under `**Run srcgen**`
- [x] `no_intrinsic_stands_for_cast_or_same` passes under `**Run typesgen**`
- [x] `gaps_drops_the_cast_and_same_intrinsics` passes under `**Run docs**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(lang): cast and same compile from std/values.msl`

---

### T56: The checker takes a hand-written `Repr` on a generic type

**What**: `impl<T: Repr> Repr for Pair<T>` is accepted: its `WIDTH` is a const expression over the arguments' `WIDTH`s (`2 * T::WIDTH`), `encode` and `decode` are typed with `T` opaque, and `Pair<u8>` then satisfies `Repr` (so `Bit<Pair<u8>>` is a carried type); the `P0001` "a hand-written Repr on a generic type" leaves the checker.
**Where**: `crates/piperine-lang/src/types/repr.rs`
**Depends on**: T55
**Reuses**: `ImplTable::hand_written_repr`, `Checker::generic_signature`
**Requirement**: GN-32 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `a_hand_written_repr_on_a_generic_type_is_typed_per_argument` passes under `**Run typestraits**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a hand-written Repr on a generic type`

---

### T57: Each instance of a generic hand-written `Repr` gets its own `encode` and `decode`

**What**: When a type instance `Pair<f64>` whose `Repr` is a generic hand-written `impl` is registered with the `TypeArena`, the elaborator lowers that `impl`'s `encode` and `decode` under the instance's substitution (`Pair<f64>.encode`, `Pair<f64>.decode`) and sets `ReprForm::HandWritten` with the folded `WIDTH`; source test: a `Bit<Pair<u8>>` carrier committed and read back through its own Gray-like layout at `u8` and at `bool`.
**Where**: `crates/piperine-lang/src/elab/repr.rs`
**Depends on**: T56
**Reuses**: D18's lowering of `encode`/`decode`, `IrBuilder::set_repr`, `Types::generic_instances`
**Requirement**: GN-32 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `each_instance_of_a_generic_hand_written_repr_has_its_own_codec` passes under `**Run elabgen**`
- [x] `a_generic_hand_written_repr_is_carried_with_its_layout` passes under `**Run srcgen**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): encode and decode per instance of a generic hand-written Repr`

---

### T58: A carrier of a hand-written type starts at the levels of its `Default`

**What**: A signal of a hand-written `Repr` type that has a `Default` and no initial value starts at the levels `encode` gives for `T::default()`, folded in the constant evaluator as D18 folds an initializer (§3.3.4); the `P0001` "a carrier of a type with a hand-written Repr and no initial value" leaves; source test reads the start value before any commit. `GAPS.md` drops L69 with the `docs.rs` assertion.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T57
**Reuses**: `ConstEval`, D18's initial levels
**Requirement**: GN-32 (AC 2), GN-31
**Executor**: Sonnet

**Done when**:

- [x] `a_carrier_of_a_hand_written_type_starts_at_its_default_levels` passes under `**Run srcgen**`
- [x] `gaps_drops_l69` passes under `**Run docs**`
- [x] Gate check passes: Full (last task of batch B9)

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a hand-written carrier starts at its default levels; GAPS drops L69`

---

### T59: Contract: merge `composite-values` phase C and re-read the anchors

**What**: Check that `composite-values` phase C is on `main` (stop and report if not); `git merge origin/main` into the branch (never a rebase of published commits); re-read and record on the Handoff each anchor this phase builds on: `Ty::CarrierArray`, `Length::Generic` where CV-C still refuses it, `Carrier.count`, `NodeArray`, `CarrierWindow`, `EventSource::ElementChanged`, `HandleBinding::Element`, `Port::Window`, `KeyParams`/`BodyVariants`, `elab/templates.rs`; every task below whose text a moved anchor affects is amended on the Handoff.
**Where**: `.specs/features/generics/tasks-c.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design-c.md "What composite-values phase C adds"
**Requirement**: GN-30
**Executor**: Sonnet

**Done when**:

- [ ] The merge is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test --workspace` pass
- [ ] The Handoff names each anchor and the task it affects

**Tests**: none
**Gate**: build

**Commit**: `chore: merge main with composite-values phase C; generics C anchors`

---

### T60: A generic module's definition is checked once against its bounds

**What**: `Checker::module` no longer refuses a generic module: it types it once with its type and nature parameters opaque (`Ty::Param`) and its const parameters as `ConstTerm::Param`, including ports and signals of `Bit<T>`, of a nature parameter `S`, and `[Bit; ROWS]` (`Ty::CarrierArray` with `Length::Generic`); a method or operator no bound gives is `E0161`/`E0180` at the definition. The whole-model check seeds generic modules (`types/model.rs` stops skipping them), so `GAPS.md` drops S3 and its §9 row, with the `docs.rs` assertion; the `types_model`/`commands` pins of design-c.md move.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T59
**Reuses**: `Checker::generic_signature`, `BoundCheck`, `ModelCheck`
**Requirement**: GN-13 (AC 1), GN-07 (AC 2), GN-08, GN-31
**Executor**: Sonnet

**Done when**:

- [ ] `a_generic_module_body_is_checked_once_at_its_definition` passes under `**Run typesmods**`
- [ ] `an_unreached_generic_module_with_an_error_fails_the_model_check` passes under `**Run typesmodel**`
- [ ] `gaps_drops_s3` passes under `**Run docs**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): check a generic module once at its definition; GAPS drops S3`

---

### T61: Placing a generic module: explicit and inferred arguments

**What**: `Checker::generic_placement` types a placement of a generic module: turbofish arguments (`Ladder::<3>`, `Dff::<u8>`), else inference of a nature parameter from the natures of the connected carriers (`Short(a, b)`), a type parameter from a `Bit<T>` connection or a value input, a const parameter from an array argument's length, then defaults (`S: Nature = State`); `E0181` naming a parameter nothing gives, `E0187` for a bound or a count; the placement records its `Instantiation` (`Types::instantiation(placement)`). Two natures given to one nature parameter are `E0180` naming both (DC4).
**Where**: `crates/piperine-lang/src/types/module_generics.rs`
**Depends on**: T60
**Reuses**: `Inference`, `BoundCheck`, `Instantiation`
**Requirement**: GN-13 (AC 1), GN-16 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `generic_module_arguments_are_explicit_or_inferred` and `two_natures_for_one_nature_parameter_are_e0180_naming_both` pass under `**Run typesmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): placements of generic modules with inferred arguments`

---

### T62: Behavior blocks bind the module's generic names

**What**: An `analog<S> for idt<S>` or `digital<T> for Setup<T>` block is typed under its module's parameters (each name of its list bound to the module's parameter of the same position), so `signal y: S` and `Bit<T>` read inside it have the module's types; a list that differs in name, number or order stays `E0403` naming both lists, now proven on a generic module.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T61
**Reuses**: `pom/resolve/behaviors.rs` (`E0403`)
**Requirement**: GN-13 (AC 2, AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `a_behavior_block_binds_its_modules_generic_names` and `behavior_names_that_differ_are_e0403_naming_both_lists` pass under `**Run typesmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): behavior blocks typed under the module's generic names`

---

### T63: Const expressions in written types anywhere

**What**: A written type or const argument that is a const expression is accepted outside a generic function too (`UInt<{A + 1}>`, `[f64; N + 1]` and `[State; D - 1]` in a generic module's items, `Int< -1 >`), its length a `Length::Generic` folded per instance; the `P0001` of D14 leaves.
**Where**: `crates/piperine-lang/src/types/lengths.rs`
**Depends on**: T62
**Reuses**: `Length`, `ConstTerm`, `Ty::fits`
**Requirement**: GN-14 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `const_expressions_in_written_types_are_accepted_everywhere` passes under `**Run typesmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): const expressions in every written type`

---

### T64: `const fn` results and repeats over const generics in const contexts

**What**: The constant evaluator folds a `const fn` called in a const context with every form `composite-values` accepts (`poly_q(&zeros)` as a param value, loops, arrays) and an array repeat whose length reads a const generic argument (`[0.0; N + 1]` in a module's const context, `consts/composite.rs`).
**Where**: `crates/piperine-lang/src/elab/consts/composite.rs`
**Depends on**: T63
**Reuses**: `ConstEval`, `ConstEval::resolved`
**Requirement**: GN-14 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `a_const_fn_and_a_generic_repeat_fold_in_a_const_context` passes under `**Run elabgen**`
- [ ] Gate check passes: Full (last task of batch B10)

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): const fn results and generic repeats in const contexts`

---

### T65: One body per instance of a generic module

**What**: `BodyVariants` is keyed by `(module, Instance, key values)`: a placement's recorded `Instantiation` folds under the enclosing substitution into an `Instance`, the body is lowered under it (`ConstEval::typed`), its `where` is discharged per instance (`E0304` naming `Ladder::<0>`) and its length obligations too; equal arguments share one body; a chain of growing arguments is `E0341` naming the module at depth 1 024; the POM keeps the generic module and each placement as written.
**Where**: `crates/piperine-lang/src/elab/structural.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Instantiations::concrete`, `KeyParams`, the depth bound of D7
**Requirement**: GN-15 (AC 1), GN-13 (AC 4)
**Executor**: Sonnet

**Done when**:

- [ ] `one_body_per_distinct_module_instance_and_the_pom_as_written`, `a_false_module_where_is_e0304_naming_the_instance` and `a_chain_of_growing_module_arguments_is_e0341` pass under `**Run elabmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): one body per instance of a generic module`

---

### T66: IR `BodyData::instance`

**What**: `BodyData::instance: Option<GenericInstance>` (`#[serde(default)]`), set by the elaborator for an instance body; `Ir::check` holds the body's name to `name()` plus its key suffix; the IR printer shows it; the interpreter refuses a body with an `instance` by name (``a generic module instance `Ladder::<3>` ``), and `frozen.rs::each_generics_form_is_refused_by_name` grows with it.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T65
**Reuses**: `GenericInstance`, `IrBuilder::mark_instance`
**Requirement**: GN-15 (AC 2), GN-29 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `a_body_instance_is_built_checked_and_printed` and `a_body_whose_name_disagrees_with_its_instance_is_refused` pass under `**Run irgen**`
- [ ] `each_generics_form_is_refused_by_name` covers a body instance under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): a body records the generic instance it lowers`

---

### T67: Rust and library names of module instances

**What**: `Text::spelled` spells a body instance name (`Ladder::<3>` to `Ladder_g3_e`, `idt::<State>~r=1k`) so it compiles, two instances never collide (the existing refusal covers the rest), and the device library's module list shows the MSL spelling.
**Where**: `crates/piperine-transpile/src/print/text.rs`
**Depends on**: T66
**Reuses**: phase A's instance spellings
**Requirement**: GN-15 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `module_instance_names_compile_and_list_in_the_library` passes under `**Run tgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): names of generic module instances`

---

### T68: An instance's carriers, nodes and ports

**What**: A generic module's instance lowers its `Bit<T>` ports and signals at the concrete type, a `signal y: S` at the concrete nature, a `[Bit; ROWS]` port bound to a window of the caller's array (composite-values `CarrierWindow`), and its behavior blocks under the instance's substitution. Source tests: `Dff<T: Repr + Default>` at `u8` and at an enum, `M::<2>` with an array-of-carriers port, `idt` with the default nature `State`, a const generic of 0 (`Taps::<0>`, its loops running zero times) and two placements with equal arguments sharing one body.
**Where**: `crates/piperine-lang/src/elab/generic_bodies.rs`
**Depends on**: T67
**Reuses**: `CarrierArrays`, `CarrierWindow`, `elab/carriers.rs`
**Requirement**: GN-13 (AC 1, AC 2), GN-15 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `generic_modules_run_with_explicit_and_inferred_arguments`, `a_nature_parameter_takes_its_default` and `a_const_generic_of_zero_makes_an_empty_module` pass under `**Run srcmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): carriers, nodes and ports of a generic module instance`

---

### T69: `Short` merges two nodes of one nature

**What**: `Short(a, b)` of the prelude, placed over `Electrical`, `Thermal` and a project's nature, merges its two nodes at elaboration through the zero-potential merge; source test: a divider with a `Short` gives the same solution as the divider with the two nodes written as one, per nature; two natures are `E0180` (T61) at the placement, proven end to end.
**Where**: `crates/piperine-parity/tests/source_generic_modules.rs`
**Depends on**: T68
**Reuses**: `elab/structural.rs` merge
**Requirement**: GN-16 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `short_merges_two_nodes_of_each_nature` and `short_over_two_natures_is_refused_naming_both` pass under `**Run srcmods**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): Short merges nodes of one nature`

---

### T70: Structural params: generic arguments, array lengths, templates

**What**: `KeyParams` classifies as structural every param read by a const generic argument of a placement, an array length (composite-values QC2 kept) or a template value, through the placement fixpoint; a `--param` override of one elaborates with the value; a host `set` or a `dc` sweep of one during a run is `E0803` naming it as structural.
**Where**: `crates/piperine-lang/src/elab/structural.rs`
**Depends on**: T69
**Reuses**: `KeyParams`, `E0803` in `piperine-api/src/session.rs`
**Requirement**: GN-20 (AC 1, AC 2, AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `a_param_read_by_a_generic_argument_or_a_length_is_structural` passes under `**Run elabmods**`
- [ ] `a_structural_param_overrides_before_the_run_and_is_refused_during_it` passes under `**Run apisession**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): params read by generic arguments and lengths are structural`

---

### T71: The Book rows generic modules make run

**What**: Flip the rows that run once generic modules run: digital#21 (`Dff::<u8>`), cheatsheet#2 (`M::<2>`, its array of carriers run since composite-values C), core-std#8 (`Setup` in `Reg`), modules#6 and modules~8b927a42 (`lowpass` over the stdlib `idt`), modules#8 (`PiAw`); and the `elab_structure.rs` pins of `idt` and `Water`. Each test becomes the acceptance with values Rust computes in the test (the register's violation flag, the low-pass step response `1 - e^{-t/tau}`, the PI output limits); a row that still needs another feature's form is narrowed to it (design-c.md Risks). `inventory.tsv`, `GAPS.md` §11 and the Book's prose follow.
**Where**: `tests/book/inventory.tsv`
**Depends on**: T70
**Reuses**: `tests/book/support`, `Refusal`, `Chapter`
**Requirement**: GN-31 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `the_flip_flop_runs_for_u8` passes under `**Run bookdigital**`
- [ ] `the_module_card_runs_for_its_const_generic` passes under `**Run bookcheatsheet**`
- [ ] `the_register_with_a_setup_check_runs` passes under `**Run bookcorestd**`
- [ ] `the_lowpass_module_runs_over_the_generic_integrator` and `the_pi_controller_runs_with_its_generic_idt` pass under `**Run bookmodules**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Full (last task of batch B11)

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the generic module listings run`

---

### T72: The checker types generation in a module body

**What**: `types/generation.rs` (`impl Checker`) replaces the `P0001` of `Checker::module`: a generation range or condition is checked as a const expression (`E0340` otherwise, naming it); a param declared inside generation is `E0322`; an item declared in a generation `for` has the array type of its element (`Ty::CarrierArray` for a signal or node, `Ty::InstanceArray` for an instance, an array of its value for a `let`), nested loops nesting it (§8.2.23); an item of a generation `if` is typed whatever the arm.
**Where**: `crates/piperine-lang/src/types/generation.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Context::Const`, `Ty::CarrierArray`
**Requirement**: GN-17 (AC 1, AC 3, AC 5)
**Executor**: Sonnet

**Done when**:

- [ ] `generated_items_are_typed_as_arrays_by_ordinal`, `a_generation_range_that_is_not_const_is_e0340` and `a_param_inside_generation_is_e0322` pass under `**Run typesgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): generation in a module body`

---

### T73: The elaborator unrolls generation

**What**: `Generation` (`elab/generation.rs`) folds each range and condition, elaborates each iteration's items with the loop variable bound as a constant, stores each named item as an array indexed by ordinal from 0 (carriers and nodes as composite-values' arrays, instances as `name[k]` children), elaborates the taken arm only, makes an empty array for an empty range, and raises `E0342` for a reference into an arm that was not elaborated or an index outside a generated array; a param read by a range or a condition joins `KeyParams` (GN-20 AC 1, DC7).
**Where**: `crates/piperine-lang/src/elab/generation.rs`
**Depends on**: T72
**Reuses**: `Unrolling`, `CarrierArrays`, `KeyParams`
**Requirement**: GN-17 (AC 1, AC 2, AC 4), GN-20 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `generation_for_makes_arrays_and_if_keeps_the_taken_arm`, `an_untaken_arm_or_an_index_outside_is_e0342` and `an_empty_generation_range_makes_an_empty_array` pass under `**Run elabgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): generation unrolled into arrays and arms`

---

### T74: Recursion by generation

**What**: A module that places itself under a generation `if` whose condition ends the recursion unrolls into nested instances; one whose recursion does not end is `E0341` naming the module at depth 1 024.
**Where**: `crates/piperine-lang/src/elab/generation.rs`
**Depends on**: T73
**Reuses**: the depth bound of D7, `stack::wide`
**Requirement**: GN-18 (AC 1, AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `a_recursion_by_generation_unrolls` and `a_generation_recursion_that_does_not_end_is_e0341` pass under `**Run elabgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): recursion by generation`

---

### T75: Generated arrays run

**What**: Source tests: `for k in 0..N { pub let rk = Resistor(..) }` in parallel gives `R / N` (Rust computes it), host paths `Top.rk[2]` read one element, a generation `if` over a param places the taken arm only, nested loops make a nested array.
**Where**: `crates/piperine-parity/tests/source_generation.rs`
**Depends on**: T74
**Reuses**: `crates/piperine-parity/tests/generation/*.msl`
**Requirement**: GN-17 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `generated_instances_run_and_read_by_ordinal` and `a_generation_if_places_the_taken_arm` pass under `**Run srcgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): generated arrays and arms run`

---

### T76: The Book's RC ladder by generation

**What**: Source test: the Book's `Ladder<const N: usize>` (recursion by generation) at `N = 1, 3, 8` gives, at an operating point and at a transient instant, the same node voltages as the ladder written out section by section (both models in the test's folder; equal to the bit).
**Where**: `crates/piperine-parity/tests/source_generation.rs`
**Depends on**: T75
**Reuses**: `crates/piperine-parity/tests/generation/*.msl`
**Requirement**: GN-18 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `the_generic_ladder_equals_the_ladder_written_out` passes under `**Run srcgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the generic ladder equals the ladder written out`

---

### T77: The Book rows of the ladder and of generation in a body

**What**: Flip modules#1 and modules~b53307eb (the ladder for an operating point and a DC sweep, the output equal to the input at every point), modules~4bcb9f1d (`Ladder::<0>` now `E0304`), modules~4bd9131d (`for k in 0..count` with a signal now `E0340`) and cheatsheet#3 (the body items card with its generation `for`); rewrite the chapter's "Run it" box with what the binary prints; `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `tests/book/modules.rs`
**Depends on**: T76
**Reuses**: `Chapter`, `Faulty`, `Commands`
**Requirement**: GN-31 (AC 2), GN-13 (AC 4), GN-17 (AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `the_ladder_runs_for_an_operating_point_and_a_sweep`, `e0304_a_generic_ladder_of_zero_sections` and `e0340_a_generation_range_read_from_a_signal` pass under `**Run bookmodules**`
- [ ] `the_body_items_card_runs_with_its_generation` passes under `**Run bookcheatsheet**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Full (last task of batch B12)

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the ladder and generation listings run`

---

### T78: The checker types generation in a `digital` block

**What**: `types/generation.rs` gains the `digital` form: `for k in 0..N { on lines[k].rise() { .. } }` with a const range, each reaction typed with `k` a constant, the element event over `Ty::CarrierArray` (composite-values C); `E0340` for a range that is not const; the `P0001` of `types/event.rs` leaves.
**Where**: `crates/piperine-lang/src/types/generation.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `types/element.rs` (composite-values C)
**Requirement**: GN-19 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `generation_in_a_digital_block_is_typed_per_iteration` passes under `**Run typesgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): generation in a digital block`

---

### T79: One reaction per iteration

**What**: The elaborator makes one reaction per iteration of a generation `for` in a `digital` block, its event `EventSource::ElementChanged` of the element, each a distinct statement, kept in iteration order within source order (§12.1.2, §12.1.14).
**Where**: `crates/piperine-lang/src/elab/reactions.rs`
**Depends on**: T78
**Reuses**: `Generation`, composite-values `elab/elements.rs`
**Requirement**: GN-19 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `generated_reactions_are_one_per_iteration_in_source_order` passes under `**Run elabgenr**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): one reaction per iteration of digital generation`

---

### T80: A generated bank reacts per element; the `half` listings run

**What**: Source test: a bank of `N` flip-flops whose generated reactions each latch their own element on `lines[k].rise()`, and two reactions of one iteration committing in source order; flip modules#2, modules~87ce6f6e and modules~3f8e84ba (the `half` stream gives three records, `stage=1` to `stage=3`, in time order); `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `crates/piperine-parity/tests/source_generation.rs`
**Depends on**: T79
**Reuses**: `crates/piperine-parity/tests/generation/*.msl`, `tests/book/modules.rs`
**Requirement**: GN-19 (AC 1, AC 2), GN-31 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `a_generated_bank_reacts_per_element` passes under `**Run srcgenr**`
- [ ] `the_half_reactions_report_each_stage_in_time_order` passes under `**Run bookmodules**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test: a generated bank reacts per element; the half listings run`

---

### T81: Galleries, and what phases D and E still refuse

**What**: A gallery fence that writes a generic module or generation is held to the checker (`std_galleries`), none of its refusals naming a form phase C lifted; `phase_c_d_and_e_forms_stay_p0001_by_name` keeps only the phase D and E cases (a template outside a param, `m`/`dtemp`, a stdlib item still blocked), each `P0001` naming its form, under its new name.
**Where**: `crates/piperine-lang/tests/std_galleries.rs`
**Depends on**: T80
**Reuses**: `GenericDeclarations`
**Requirement**: GN-31 (AC 2), GN-29 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `the_fences_generics_phase_c_lifted_are_checked` passes under `**Run galleries**`
- [ ] `phase_d_and_e_forms_stay_p0001_by_name` passes under `**Run typesgen**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test: galleries of generic modules; phase D and E forms stay P0001`

---

### T82: Phase C bookkeeping

**What**: `GAPS.md` drops L30 and L31, moves each L58 item phase C unblocked to the id of what still blocks it (or out, when it runs), updates §9 (I §3.4, III §8–§8.2) and the §10 code list (`E0322`, `E0340`, `E0342` leave `code_owners.tsv`); `spec.md`'s traceability marks GN-13 to GN-20 and GN-32 phase C; `.specs/STATE.md` MD-81 gains item 6 (design-c.md "Tech Decisions"); `ROADMAP.md`'s `generics` C row says delivered.
**Where**: `GAPS.md`
**Depends on**: T81
**Reuses**: `docs.rs`
**Requirement**: GN-31 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_generics_phase_c_closed` and `state_records_generic_modules_and_generation` pass under `**Run docs**`
- [ ] Gate check passes: Full (last task of batch B13)

**Tests**: integration
**Gate**: full

**Commit**: `docs: generics phase C closes L30, L31, L69, L70 and S3`

---

## Checks

**C51** - `encode` and `decode` under `T: Repr` lower and run at `f64`, a signed integer, a derived struct, enum, tuple and array (GAPS L70). (GN-01, AC 1 residual; T53, T54)
Proof: run elabgen | test encode_and_decode_of_every_layout_lower_through_the_level_cast ... ok
Proof: run srcgen | test encode_and_decode_run_at_every_layout ... ok

**C52** - A hand-written `Repr` on a generic type carries each instance with that `impl`'s layout. (GN-32, AC 1; T56, T57)
Proof: run typestraits | test a_hand_written_repr_on_a_generic_type_is_typed_per_argument ... ok
Proof: run srcgen | test a_generic_hand_written_repr_is_carried_with_its_layout ... ok

**C53** - A carrier of a hand-written type with a `Default` and no initial value starts at the levels `encode` gives for the default. (GN-32, AC 2; T58)
Proof: run srcgen | test a_carrier_of_a_hand_written_type_starts_at_its_default_levels ... ok

**C54** - A generic module with type, nature (with a default) and const parameters elaborates each placement as its explicit or inferred arguments give. (GN-13, AC 1; T61, T68)
Proof: run typesmods | test generic_module_arguments_are_explicit_or_inferred ... ok
Proof: run srcmods | test generic_modules_run_with_explicit_and_inferred_arguments ... ok

**C55** - A behavior block repeating the module's generic names binds them to the instance's arguments. (GN-13, AC 2; T62)
Proof: run typesmods | test a_behavior_block_binds_its_modules_generic_names ... ok

**C56** - Behavior block names that differ from the module's are refused naming both lists. (GN-13, AC 3; T62)
Proof: run typesmods | test behavior_names_that_differ_are_e0403_naming_both_lists ... ok

**C57** - A module's `where` false for an instance is `E0304` naming the instance. (GN-13, AC 4; T65, T77)
Proof: run elabmods | test a_false_module_where_is_e0304_naming_the_instance ... ok
Proof: run bookmodules | test e0304_a_generic_ladder_of_zero_sections ... ok

**C58** - Const expressions in types and const arguments evaluate at elaboration. (GN-14, AC 1; T63)
Proof: run typesmods | test const_expressions_in_written_types_are_accepted_everywhere ... ok

**C59** - A `const fn` called in a const context folds with every composite form. (GN-14, AC 2; T64)
Proof: run elabgen | test a_const_fn_and_a_generic_repeat_fold_in_a_const_context ... ok

**C60** - One IR body per distinct instance of a generic module; the POM keeps the generic module and each placement as written. (GN-15, AC 1; T65)
Proof: run elabmods | test one_body_per_distinct_module_instance_and_the_pom_as_written ... ok

**C61** - Each instance is named deterministically in the IR, the generated Rust and the device library's module list. (GN-15, AC 2; T66, T67)
Proof: run irgen | test a_body_instance_is_built_checked_and_printed ... ok
Proof: run tgen | test module_instance_names_compile_and_list_in_the_library ... ok

**C62** - `Short(a, b)` over one continuous nature gives the solution of one node. (GN-16, AC 1; T69)
Proof: run srcmods | test short_merges_two_nodes_of_each_nature ... ok

**C63** - `Short` over two natures is refused naming both. (GN-16, AC 2; T61, T69)
Proof: run typesmods | test two_natures_for_one_nature_parameter_are_e0180_naming_both ... ok
Proof: run srcmods | test short_over_two_natures_is_refused_naming_both ... ok

**C64** - A generation `for` elaborates its items once per iteration, each named item an array by ordinal, nested loops nesting it. (GN-17, AC 1; T72, T73, T75)
Proof: run typesgenr | test generated_items_are_typed_as_arrays_by_ordinal ... ok
Proof: run srcgenr | test generated_instances_run_and_read_by_ordinal ... ok

**C65** - A generation `if` elaborates the taken arm only. (GN-17, AC 2; T73, T75)
Proof: run elabgenr | test generation_for_makes_arrays_and_if_keeps_the_taken_arm ... ok
Proof: run srcgenr | test a_generation_if_places_the_taken_arm ... ok

**C66** - A generation range or condition that is not const is `E0340`. (GN-17, AC 3; T72, T77)
Proof: run typesgenr | test a_generation_range_that_is_not_const_is_e0340 ... ok
Proof: run bookmodules | test e0340_a_generation_range_read_from_a_signal ... ok

**C67** - A reference into an untaken arm or an index outside a generated array is `E0342`. (GN-17, AC 4; T73)
Proof: run elabgenr | test an_untaken_arm_or_an_index_outside_is_e0342 ... ok

**C68** - A param declared inside generation is `E0322`. (GN-17, AC 5; T72)
Proof: run typesgenr | test a_param_inside_generation_is_e0322 ... ok

**C69** - A module placing itself under a terminating generation `if` unrolls. (GN-18, AC 1; T74)
Proof: run elabgenr | test a_recursion_by_generation_unrolls ... ok

**C70** - The Book's RC ladder at `N = 1, 3, 8` equals the ladder written out. (GN-18, AC 2; T76, T77)
Proof: run srcgenr | test the_generic_ladder_equals_the_ladder_written_out ... ok
Proof: run bookmodules | test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok

**C71** - A generation recursion that does not end is `E0341`. (GN-18, AC 3; T74)
Proof: run elabgenr | test a_generation_recursion_that_does_not_end_is_e0341 ... ok

**C72** - `for k in 0..N { on lines[k].rise() { .. } }` makes one reaction per iteration. (GN-19, AC 1; T78, T79, T80)
Proof: run typesgenr | test generation_in_a_digital_block_is_typed_per_iteration ... ok
Proof: run srcgenr | test a_generated_bank_reacts_per_element ... ok

**C73** - Generated reactions keep iteration order within source order, each a distinct statement. (GN-19, AC 2; T79, T80)
Proof: run elabgenr | test generated_reactions_are_one_per_iteration_in_source_order ... ok
Proof: run bookmodules | test the_half_reactions_report_each_stage_in_time_order ... ok

**C74** - A param read by generation, a const generic argument, an array length or a template is structural. (GN-20, AC 1; T70, T73)
Proof: run elabmods | test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok

**C75** - A host override of a structural param before a run elaborates with it; a set or sweep during the run is `E0803` naming it as structural. (GN-20, AC 2, AC 3; T70)
Proof: run apisession | test a_structural_param_overrides_before_the_run_and_is_refused_during_it ... ok

**C76** - A generic module's body is checked once at its definition, and an unreached one with an error fails `piperine check` (S3). (GN-07, AC 2; GN-08, AC 1; T60)
Proof: run typesmods | test a_generic_module_body_is_checked_once_at_its_definition ... ok
Proof: run typesmodel | test an_unreached_generic_module_with_an_error_fails_the_model_check ... ok

**C77** - The interpreter refuses a generic module instance by name. (GN-29, AC 1; T66)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok

**C78** - The forms of phases D and E stay `P0001` naming the form. (GN-29, AC 2; T81)
Proof: run typesgen | test phase_d_and_e_forms_stay_p0001_by_name ... ok

**C79** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C80** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

**C81** - `GAPS.md` drops L70, L69, S3, L30 and L31 in the commits that close them. (GN-31, AC 1; T54, T58, T60, T82)
Proof: run docs | test gaps_drops_l70 ... ok
Proof: run docs | test gaps_drops_l69 ... ok
Proof: run docs | test gaps_drops_s3 ... ok
Proof: run docs | test gaps_drops_what_generics_phase_c_closed ... ok

**C82** - Every Book row and gallery fence phase C lets run is held to it. (GN-31, AC 2; T71, T77, T80, T81)
Proof: run bookdigital | test the_flip_flop_runs_for_u8 ... ok
Proof: run bookcheatsheet | test the_module_card_runs_for_its_const_generic ... ok
Proof: run bookcheatsheet | test the_body_items_card_runs_with_its_generation ... ok
Proof: run bookcorestd | test the_register_with_a_setup_check_runs ... ok
Proof: run bookmodules | test the_lowpass_module_runs_over_the_generic_integrator ... ok
Proof: run bookmodules | test the_pi_controller_runs_with_its_generic_idt ... ok
Proof: run galleries | test the_fences_generics_phase_c_lifted_are_checked ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C83** - A chain of growing module arguments is `E0341` naming the module. (Edge case; T65)
Proof: run elabmods | test a_chain_of_growing_module_arguments_is_e0341 ... ok

**C84** - A nature parameter nothing fixes takes its default; a const generic of 0 elaborates with empty loops. (Edge cases; T68)
Proof: run srcmods | test a_nature_parameter_takes_its_default ... ok
Proof: run srcmods | test a_const_generic_of_zero_makes_an_empty_module ... ok

**C85** - An empty generation range makes an empty array. (Edge case; T73)
Proof: run elabgenr | test an_empty_generation_range_makes_an_empty_array ... ok

**C86** - `cast` and `same` compile from `std/values.msl`, agree with the lowering they replace, and a `cast` to a tuple holding an enum runs. (GN-27, AC 1; GN-01, AC 1; T55)
Proof: run srcgen | test cast_and_same_from_source_agree_with_the_removed_lowering ... ok
Proof: run srcgen | test a_cast_to_a_tuple_holding_an_enum_runs ... ok
Proof: run typesgen | test no_intrinsic_stands_for_cast_or_same ... ok
Proof: run docs | test gaps_drops_the_cast_and_same_intrinsics ... ok

---

## Handoff

The phase's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-23): the defaults of design-c.md DC1 to DC7. Batch B9 (T53 to T58, 2026-11-23):
(1) `encode`/`decode` at `f64`, signed integers and every derived layout are `Expr::LevelCast` to/from `[bool; WIDTH]`
(`elab/levels.rs`); `bool` and unsigned integers keep their inline shift/mask form. `T::WIDTH` of a derived layout is folded
by `Types::repr_levels` (a new public method; hand-written widths are read from the `impl`).
(2) **Spec amendment, E0701 retired.** `cast` is the generic function of `std/values.msl`, so a width mismatch is the false
`where A::WIDTH == B::WIDTH` of the instantiation: `E0127` naming `cast::<A, B>` (part VII 22.1.3 rule 2 amended, the
`E0701` row removed, `spec_text.rs` RETIRED_CODES gains it, GAPS section 10 note). `Length::fits` no longer rejects two
`T::WIDTH` of different parameters at a definition: the `SameLength` obligation compares them at the instantiation, and a
`where` is discharged under the instance's substitution.
(3) Gaps found and fixed on the way (each in the task that needed it): a generic function returning an `Option` runs per
instance (T55); a block-like statement before the tail of an `Option` function (a `for` in `decode`) was dropped (T57);
`rt::Carried` no longer requires `Default` (`[bool; 64]` is carried; `Carried::zeroed()` unpacks a `None` payload), and
tuples and arrays judge a tag inside (`try_from_levels`) (T54, T55); an instance name with a tuple or array argument spells
as one Rust identifier (`Text::spelled_cased`, T54); the const evaluator runs `x.encode()` at scalars (T57).
(4) A hand-written `Repr` on a generic type writes its signature as `[bool; 2 * T::WIDTH]`: `T::WIDTH` and expressions over it
are accepted as written lengths and deferred to the instantiation (checker `width_parameter_of`); T63 generalizes const
expressions in written types. A carrier of such a type still needs an initial value or a `Default` (`u8` has none).
(5) A carrier with a `Default` and no initial value starts at `encode(default)` (a unit enum with an `impl Default` included).
(6) Pre-existing, not mine: `cargo build ... --all-targets` prints one warning, `associated function run is never used`, in
`tests/book` (`book_types`).
**B10 (T59, 2026-11-23), anchors re-read after merging `origin/main` (composite-values phase C):** (1) there is **no
`Ty::CarrierArray`**: an array of carriers or of nodes is `Ty::Array(element, Length)` with `Ty::is_carrier_element()`
(`Ty::innermost`, `Ty::read_value`); T60/T63 type `[Bit; ROWS]` and `[E; N]` as that with `Length::Generic`. There is no
`Ty::InstanceArray` either (T72 adds it). (2) `Length::Generic` is accepted in the checker; the elaborator still returns
`LengthFault::NotConstant` for a non-`Expr` generic length (`elab/carrier_arrays.rs:110`) and `Unsupported` in
`elab/consts/parsed.rs:28`: T65/T68 fold it under the instance substitution. (3) `Carrier.count`, `NodeArray`,
`CarrierWindow`, `EventSource::ElementChanged`, `HandleBinding::Element`, `Port::Window` exist in `piperine-ir`/`-plan`/
`-solver` (`handle.rs`, `model.rs`, `build.rs`, `piperine-plan/src/{derive,netlist}.rs`). (4) `KeyParams`/`BodyVariants`
are `pub(super)` in `elab/structural.rs:48,78`; `elab/templates.rs` unchanged. (5) The merge conflicted in `GAPS.md`,
`docs.rs` and `print/text.rs::spelled_cased` only: brackets spell `_i`/`_j` (main), `; ( )` keep `_l`/`_t`/`_z`; loud count
22 (24 on main less L69 and L70). Pre-existing warning of `book_types` is gone. Nothing below is amended by a moved anchor
except (1).
**B10 stopped at T59 (2026-11-23, executor ran out of context); T60 to T62 are drafted on `origin/wip/generics-c-b10`
(commit on top of 8ec95747), NOT green, to be continued from there (`git merge origin/wip/generics-c-b10` or cherry-pick).**
What the draft holds and what it found:
(1) T60: `Checker::module` = `module_of` under `generic_item`, `generic_signature` at the definition; `model.rs` seeds generic
modules (a generic struct/enum is still skipped); `port_type` takes a nature parameter (`Ty::Param` with the `Nature` bound,
`carrier.rs`) as a node type; `GAPS.md` drops S3 (0 silent, 22 loud) and `docs.rs` follows (`gaps_drops_s3`).
(2) T61: `types/module_generics.rs` (`generic_placement`, `port_type_under_placement`; `Checker::placement_substitution`;
`Inference::unify` learns a carrier from a carrier; `explicit_arguments`/`argument_types` are `pub(super)`); `gnd` teaches no
nature; two natures for one nature parameter is `E0180` naming both. (3) T62: `E0403` now names both lists
(`pom/resolve/behaviors.rs`). Tests: `crates/piperine-lang/tests/types_generic_modules.rs` (6 tests, all pass in the draft,
the T63 one untried) and `types_model.rs::an_unreached_generic_module_with_an_error_fails_the_model_check`.
**Blocker the plan did not foresee:** every Book run calls the whole-model check, so seeding generic modules (T60) makes the
Book's own generic modules report their definition gaps instead of the `generic module is not supported yet` refusal that 14
Book tests pin (`book_modules` x6, `book_digital`, `book_core_std`, `book_cheatsheet` x2, `book_natures`, `book_generics`,
`piperine-cli --test book` ladder listing). Gaps seen: a value param of type `T`/`C` (`param of type T is not supported`),
a `struct` with a carrier field (bundle), generation in a body (T72), `let y` reading its own name (E0332, the `idt`/lowpass
pattern), `Setup` placed with 2 of 3 ports (E0312, a defaulted `out`), `_` inferred type, a named connection
(`reset: seat` in `idtmod`), and "`bool` does not implement `Default`" (E0187 on `signal clk: Bit` when the placement of a
generic module is typed: the drive-book that clears it is not filled for a generic placement; also `u8` has no `Default`).
Two ways forward: (a) make those definitions and placements check clean (T63 onward do most of it) and narrow each of the
14 Book refusals to the form still missing (allowed by design-c "Goldens, pins and tests that may move"), or (b) keep seeding
off for the Book run until T71. The draft keeps `Checker::refuse_generic_placements` (true for `Types::check`, false in
`run_model`); (b) needs it true in `run_model` too and a test-only way to turn it off.
Also: `suite_hygiene::mod_rs_declares_only` (types/mod.rs ceiling 60 lines) was fixed on the draft; the helper scripts of
`/var/tmp/piperine-tmp-d/helpers` did not exist, `check_commit.py` of the skill was used.
**Dispatched:** none yet.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 13 → Phase 14 → Phase 15 → Phase 16 → Phase 17

Phase 13:  T53 → T54 → T55 → T56 → T57 → T58
Phase 14:  T59 → T60 → T61 → T62 → T63 → T64
Phase 15:  T65 → T66 → T67 → T68 → T69 → T70 → T71
Phase 16:  T72 → T73 → T74 → T75 → T76 → T77
Phase 17:  T78 → T79 → T80 → T81 → T82
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B9 | 13 | T53 to T58 | starts on `main` as it is (no composite-values C); closes L70 and L69; T55 retires the `cast`/`same` intrinsics (user, 2026-11-23) |
| B10 | 14 | T59 to T64 | starts after `composite-values` phase C is on `main` (T59 merges it); closes S3 |
| B11 | 15 | T65 to T71 | generic module instances; the generic-module Book rows flip |
| B12 | 16 | T72 to T77 | generation in a body; the ladder rows flip |
| B13 | 17 | T78 to T82 | needs composite-values C's element events; closes L30 and L31 |

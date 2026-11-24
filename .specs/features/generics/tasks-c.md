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
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookgenerics**: `cargo test -p piperine --test book_generics`
**Run bookwelcome**: `cargo test -p piperine --test book_welcome`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run typesmodules**: `cargo test -p piperine-lang --test types_modules`
**Run elabresults**: `cargo test -p piperine-lang --test elab_results`
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
- **Book rows.** A Book test that asserts a refusal and starts to run flips in the task that makes it run: its test becomes the acceptance, `tests/book/inventory.tsv` takes `run` (or `holds`), the `GAPS.md` §11 row leaves, and the Book's own "today it refuses" prose is rewritten to what the binary now prints. A row that still needs another feature's form is narrowed to that form, and its §11 row takes that gap id and owner (recorded on the Handoff). **Re-pin rule (amendment 2026-11-23):** a task that changes what a refused Book row prints re-pins that row, in the same commit, to the exact refusals it now gives (code and message, the full list where the test lists them); a pin is never widened to "some refusal". A Book listing that contradicts `docs/spec` is corrected in the Book (a Book amendment, T116), never accommodated in the code.
- **The unfinished-forms gate (amendment 2026-11-23, design-c.md "Interim: the unfinished-forms gate").** `UnfinishedForms::IN_FORCE` lists the forms every production entry (`Types::check`, `Types::check_model`, so `piperine run`/`check`, the host, the Book) still refuses with today's `P0001`. A test of a task before the form's flip opts in through `Types::check_admitting`/`Types::check_model_admitting` (piperine-lang) or `Project::admitting` (piperine-parity) and names the form. Only the flip task of a form removes it from `IN_FORCE`: `GenericModules` in T71, `Generation` in T77, `DigitalGeneration` in T80 (which deletes the gate). No Book test changes because of the gate before its flip task.
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

### Phase 14: Contract, Book amendments and the checker's generic modules (behind the gate)

```
T59 → T116 → T60 → T61 → T117 → T62 → T63 → T64
```

### Phase 15: Generic module instances; the gate opens for generic modules

```
T65 → T66 → T67 → T68 → T71 → T69 → T70
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

### T116: Book amendments: the listings that contradict the spec

**What**: Correct the three Book listings the generic definition check showed to contradict `docs/spec` (design-c.md "Gap verdicts", Book amendment BA1 to BA3), with the prose that quotes them: core-std#8 `Reg` connects all three ports of `Setup` (`signal late: Bit; let chk = Setup(clk, d, late) { ts: 100p };`, §8.1.8: a carrier port is never optional), and the sentence of chapter "digital" that quotes `Setup(clk, d) { ts: 100p }` follows; cheatsheet#2 gives `signal s: [Bit; N]` its initial value (`= [false; N]`, §3.3.4: `bool` has no `Default`, MD-78); natures#8 bounds `Slice<T: Repr + Default>` as gallery E.6 does (§3.3.4: `signal buf: Bit<T>` starts at `T::default()`). `inventory.tsv` takes the new fingerprints; each refused row keeps its refusal (the gate is closed). Each correction gets a test that proves the old form is refused under the spec on a non-generic model, so the amendment is not taken on trust.
**Where**: `docs/book/content.html`
**Depends on**: T59
**Reuses**: `Chapter`, `Faulty`, `Inventory`
**Requirement**: GN-31 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was` passes under `**Run bookcorestd**`
- [x] `an_array_of_bits_with_no_initial_value_is_e0187` passes under `**Run bookcheatsheet**`
- [x] `the_slice_listing_bounds_t_by_default_as_gallery_e6_does` passes under `**Run booknatures**`
- [x] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): Reg connects every port of Setup; initial values the spec requires`

---

### T60: A generic module's definition is checked once against its bounds (behind the gate)

**What**: Adds the unfinished-forms gate (design-c.md "Interim"): `UnfinishedForm { GenericModules, Generation, DigitalGeneration }` and `UnfinishedForms::IN_FORCE` (all three) in `types/unfinished.rs`, read by both entries; `Types::check_admitting`/`Types::check_model_admitting` take the forms a test admits. With `GenericModules` admitted, `Checker::module` types a generic module once with its type and nature parameters opaque (`Ty::Param`) and its const parameters as `ConstTerm::Param`, including ports and signals of `Bit<T>`, of a nature parameter `S`, and `[Bit; ROWS]` (`Ty::Array` of a carrier element with `Length::Generic`, T59 anchor 1); a method or operator no bound gives is `E0161`/`E0180` at the definition; **a param whose type is a type parameter (`reset: T`, §8.1.15) is typed under its bounds**, its default and `where` checked as const expressions over it; a param whose type is a parameter bounded by `Fn(P)` (`cell: C`) stays `P0001` naming the template form (L32, phase D T88); a module's `T: Default` bound makes its carriers of `Bit<T>` with no initial value legal (§3.3.4), and one without it is `E0187` at the definition. The whole-model check seeds generic modules only when the form is admitted. Not admitted (production), every entry behaves as on `main`: S3 stays open in `GAPS.md` until T71.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T116
**Reuses**: `Checker::generic_signature`, `BoundCheck`, `ModelCheck`; the draft on `origin/wip/generics-c-b10` (its seeding moves behind the gate)
**Requirement**: GN-13 (AC 1), GN-07 (AC 2), GN-08, GN-31
**Executor**: Sonnet

**Done when**:

- [x] `a_generic_module_body_is_checked_once_at_its_definition` and `a_param_of_a_type_parameter_is_typed_under_its_bounds` pass under `**Run typesmods**`
- [x] `an_unreached_generic_module_with_an_error_fails_the_model_check` (admitting `GenericModules`) and `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` (production: an unreached faulty generic module passes, a placement of one is `P0001` `generic module`) pass under `**Run typesmodel**`
- [x] Every Book target passes unchanged
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): check a generic module once at its definition, behind the unfinished-forms gate`

---

### T61: Placing a generic module: explicit and inferred arguments

**What**: (gate: `GenericModules` admitted in its tests) `Checker::generic_placement` types a placement of a generic module: turbofish arguments (`Ladder::<3>`, `Dff::<Option<u8>>`), a `_` argument inferred as if left out (§1.2.12, spec index row 22), else inference of a nature parameter from the natures of the connected carriers (`Short(a, b)`), a type parameter from a `Bit<T>` connection or a value input, a const parameter from an array argument's length, then defaults (`S: Nature = State`); `E0181` naming a parameter nothing gives, `E0187` for a bound or a count (`Dff::<u8>`: `u8` does not implement `Default`, §3.3.4); the placement records its `Instantiation` (`Types::instantiation(placement)`). Two natures given to one nature parameter are `E0180` naming both (DC4). A generic placement takes part in what every placement does: it is pushed on `placing`, so a `let` that feeds back through its result is judged by §8.3.5 (`let y = idt((x - y) / tau);` is legal: `idt`'s result reads only its node), and a carrier handed to its `out` port is recorded in the `DriveBook` (`handed`), so that carrier needs no initial value of its own (§3.3.4).
**Where**: `crates/piperine-lang/src/types/module_generics.rs`
**Depends on**: T60
**Reuses**: `Inference`, `BoundCheck`, `Instantiation`
**Requirement**: GN-13 (AC 1), GN-16 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `generic_module_arguments_are_explicit_or_inferred`, `an_underscore_generic_argument_is_inferred` and `two_natures_for_one_nature_parameter_are_e0180_naming_both` pass under `**Run typesmods**`
- [x] `a_generic_placement_feeds_back_through_its_unknown` (the Book's `lowpass` and `PiAw` shapes over the stdlib `idt`: no `E0332`) and `a_carrier_handed_to_a_generic_out_port_needs_no_initial_value` pass under `**Run typesmods**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): placements of generic modules with inferred arguments`

---

### T117: A trailing `Option` value input passed by name

**What**: A placement passes a trailing `Option` value input by name after its positional connections (§8.1.9, §8.3.7): `idt::<S>(x, base.unwrap_or(start), reset: seat)` binds `reset` to `Some(seat)`, an input left out reads `None`; a name that is no trailing `Option` value input is `E0140`, a missing required one `E0141` (§3.2). Replaces the `P0001` "named connection" of `types/module.rs` (a loud gap with no `GAPS.md` row, found by the draft of T60 in `std/analog.msl`'s `idtmod`); generic-agnostic, so it is proven on a plain module and needs no gate.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T61
**Reuses**: `Checker::apply` (§3.2 binding of a call), `elab/results.rs`
**Requirement**: GN-13 (AC 1), GN-28 (AC 2, `idtmod`)
**Executor**: Sonnet

**Done when**:

- [x] `a_trailing_option_value_input_passed_by_name_binds_it` and `a_named_connection_that_is_no_trailing_option_input_is_e0140` pass under `**Run typesmodules**`
- [x] `a_value_input_passed_by_name_reads_its_argument` passes under `**Run elabresults**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types,elab): trailing Option value inputs passed by name`

---

### T62: Behavior blocks bind the module's generic names

**What**: (gate: `GenericModules` admitted in its tests) An `analog<S> for idt<S>` or `digital<T> for Setup<T>` block is typed under its module's parameters (each name of its list bound to the module's parameter of the same position), so `signal y: S` and `Bit<T>` read inside it have the module's types; a list that differs in name, number or order stays `E0403` naming both lists, now proven on a generic module.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T117
**Reuses**: `pom/resolve/behaviors.rs` (`E0403`)
**Requirement**: GN-13 (AC 2, AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `a_behavior_block_binds_its_modules_generic_names` and `behavior_names_that_differ_are_e0403_naming_both_lists` pass under `**Run typesmods**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): behavior blocks typed under the module's generic names`

---

### T63: Const expressions in written types anywhere

**What**: (gate: `GenericModules` admitted where a generic module is written) A written type or const argument that is a const expression is accepted outside a generic function too (`UInt<{A + 1}>`, `[f64; N + 1]` and `[State; D - 1]` in a generic module's items, `Int< -1 >`), its length a `Length::Generic` folded per instance; the `P0001` of D14 leaves. Covers the Book's `[Electrical; N + 1]` (modules#1) and `[false; N]` (cheatsheet#2 after T116).
**Where**: `crates/piperine-lang/src/types/lengths.rs`
**Depends on**: T62
**Reuses**: `Length`, `ConstTerm`, `Ty::fits`
**Requirement**: GN-14 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `const_expressions_in_written_types_are_accepted_everywhere` passes under `**Run typesmods**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): const expressions in every written type`

---

### T64: `const fn` results and repeats over const generics in const contexts

**What**: The constant evaluator folds a `const fn` called in a const context with every form `composite-values` accepts (`poly_q(&zeros)` as a param value, loops, arrays) and an array repeat whose length reads a const generic argument (`[0.0; N + 1]` in a module's const context, and an initial value `[false; N]` of a carrier array, `consts/composite.rs`).
**Where**: `crates/piperine-lang/src/elab/consts/composite.rs`
**Depends on**: T63
**Reuses**: `ConstEval`, `ConstEval::resolved`
**Requirement**: GN-14 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `a_const_fn_and_a_generic_repeat_fold_in_a_const_context` passes under `**Run elabgen**`
- [x] Gate check passes: Full (last task of batch B10)

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): const fn results and generic repeats in const contexts`

---

### T65: One body per instance of a generic module

**What**: (gate: `GenericModules` admitted in its tests) `BodyVariants` is keyed by `(module, Instance, key values)`: a placement's recorded `Instantiation` folds under the enclosing substitution into an `Instance`, the body is lowered under it (`ConstEval::typed`), its `where` is discharged per instance (`E0304` naming `Ladder::<0>`) and its length obligations too; equal arguments share one body; a chain of growing arguments is `E0341` naming the module at depth 1 024; the POM keeps the generic module and each placement as written.
**Where**: `crates/piperine-lang/src/elab/structural.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Instantiations::concrete`, `KeyParams`, the depth bound of D7
**Requirement**: GN-15 (AC 1), GN-13 (AC 4)
**Executor**: Sonnet

**Done when**:

- [x] `one_body_per_distinct_module_instance_and_the_pom_as_written`, `a_false_module_where_is_e0304_naming_the_instance` and `a_chain_of_growing_module_arguments_is_e0341` pass under `**Run elabmods**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): one body per instance of a generic module`

---

### T66: IR `BodyData::instance`

**What**: (gate: `GenericModules` admitted in its tests) `BodyData::instance: Option<GenericInstance>` (`#[serde(default)]`), set by the elaborator for an instance body; `Ir::check` holds the body's name to `name()` plus its key suffix; the IR printer shows it; the interpreter refuses a body with an `instance` by name (``a generic module instance `Ladder::<3>` ``), and `frozen.rs::each_generics_form_is_refused_by_name` grows with it.
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

**What**: (gate: `GenericModules` admitted in its tests) `Text::spelled` spells a body instance name (`Ladder::<3>` to `Ladder_g3_e`, `idt::<State>~r=1k`) so it compiles, two instances never collide (the existing refusal covers the rest), and the device library's module list shows the MSL spelling.
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

**What**: (gate: `GenericModules` admitted in its tests: `Project::admitting` in `crates/piperine-parity/src/project.rs`, a test-support option that T71 removes with the form) A generic module's instance lowers its `Bit<T>` ports and signals at the concrete type, a `signal y: S` at the concrete nature, a `[Bit; ROWS]` port bound to a window of the caller's array (composite-values `CarrierWindow`), and its behavior blocks under the instance's substitution. Source tests: `Dff<T: Repr + Default>` at `Option<u8>` and at an enum with a `Default` (`u8` has none, §3.3.4), `M::<2>` with an array-of-carriers port, `idt` with the default nature `State`, a const generic of 0 (`Taps::<0>`, its loops running zero times) and two placements with equal arguments sharing one body.
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

### T71: The gate opens for generic modules; their Book rows run or narrow

**What**: The named flip task of the interim strategy (design-c.md "Interim"). `GenericModules` leaves `UnfinishedForms::IN_FORCE`: every entry seeds generic modules in the whole-model check and types their placements, so `GAPS.md` drops S3 and its §9 row (`gaps_drops_s3`, moved here from T60), and the `types_model`/`commands` pins of design-c.md move; `Project::admitting(GenericModules)` and every test's opt-in for that form leave. In the same commit, per the Book rule and the re-pin rule:
(1) **Rows that run** flip to acceptances with values Rust computes in the test: digital#21 (`Dff` at `Option<u8>`, the harness's `in`-only signals given `= false`/`= None` as §3.3.4 requires; `Dff::<u8>` is the spec's `E0187` naming `u8` and `Default`), cheatsheet#2 (`M::<2>`), core-std#8 (`Reg` with `Setup(clk, d, late)` after T116: the violation flag), modules#6 and modules~8b927a42 (`lowpass` over the stdlib `idt`: the step response `1 - e^{-t/tau}`), modules#8 (`PiAw`: the output limits); the `elab_structure.rs` pins of `idt` and `Water` become acceptances or narrow to the form still missing.
(2) **Rows still refused** re-pin to their exact remaining refusals: modules#1, modules~b53307eb, modules~4bcb9f1d and the `piperine-cli --test book` ladder listing to the body generation `P0001` (L31, flips in T77; the chapter's "Run it" box is rewritten to what the binary now prints); modules#2 to the two generation `P0001`s (L31, T77/T80); modules#10 and cheatsheet#8 to the template form of `cell: C` (L32, phase D T88), the bundle `Cell` (L25, `resolved-nets-bundles` T21) and generation (L31, T77); natures#8 to the bundle `Stream` alone (L25, `resolved-nets-bundles` T21; harness at a type with a `Default`, T116).
(3) **Prose**: welcome~47af33d1 no longer lists `idt` and generic modules among the refused forms; the generics status box (generics~00f0fd4f) no longer lists generic modules (its smallest-model case leaves, per its own comment); `inventory.tsv`, `GAPS.md` §11 (each row with its new gap id and owner) and §4 L30 (narrowed to what phase C still owes) follow.
**Where**: `tests/book/inventory.tsv`
**Depends on**: T68
**Reuses**: `tests/book/support`, `Refusal`, `Chapter`, `Faulty`
**Requirement**: GN-07 (AC 2), GN-08, GN-31 (AC 1, AC 2), GN-13 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_s3` passes under `**Run docs**`; `an_unreached_generic_module_with_an_error_fails_the_model_check` (no opt-in) and `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` (now `Generation`, `DigitalGeneration`) pass under `**Run typesmodel**`
- [ ] `the_flip_flop_runs_for_a_type_with_a_default` and `e0187_the_flip_flop_at_u8_has_no_default` pass under `**Run bookdigital**`
- [ ] `the_module_card_runs_for_its_const_generic` and `the_template_card_is_refused_for_its_template_bundle_and_generation` pass under `**Run bookcheatsheet**`
- [ ] `the_register_with_a_setup_check_runs` passes under `**Run bookcorestd**`
- [ ] `the_lowpass_module_runs_over_the_generic_integrator`, `the_pi_controller_runs_with_its_generic_idt`, `the_ladder_is_refused_for_its_generation`, `the_half_reactions_are_refused_for_their_generation`, `the_memory_bank_is_refused_for_its_template_bundle_and_generation` and `e0304_waits_for_the_generation_of_the_generic_ladder` pass under `**Run bookmodules**`
- [ ] `a_bundle_is_refused_where_the_solver_reaches_it` (the bundle alone) passes under `**Run booknatures**`; `a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it` passes under `**Run bookgenerics**`; `the_status_paragraph_lists_what_is_still_refused_and_one_item_runs` passes under `**Run bookwelcome**`; `every_cli_listing_of_the_book_is_what_the_binary_prints` passes under `**Run clibook**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): generic modules leave the unfinished-forms gate; their Book rows run or narrow; GAPS drops S3`

---

### T69: `Short` merges two nodes of one nature

**What**: (no gate: `GenericModules` left `IN_FORCE` in T71) `Short(a, b)` of the prelude, placed over `Electrical`, `Thermal` and a project's nature, merges its two nodes at elaboration through the zero-potential merge; source test: a divider with a `Short` gives the same solution as the divider with the two nodes written as one, per nature; two natures are `E0180` (T61) at the placement, proven end to end.
**Where**: `crates/piperine-parity/tests/source_generic_modules.rs`
**Depends on**: T71
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
- [ ] Gate check passes: Full (last task of batch B11)

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): params read by generic arguments and lengths are structural`

---

### T72: The checker types generation in a module body

**What**: (gate: `Generation` admitted in its tests) `types/generation.rs` (`impl Checker`) replaces the `P0001` of `Checker::module`: a generation range or condition is checked as a const expression (`E0340` otherwise, naming it); a param declared inside generation is `E0322`; an item declared in a generation `for` has the array type of its element (`Ty::CarrierArray` for a signal or node, `Ty::InstanceArray` for an instance, an array of its value for a `let`), nested loops nesting it (§8.2.23); an item of a generation `if` is typed whatever the arm.
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

**What**: (gate: `Generation` admitted in its tests) `Generation` (`elab/generation.rs`) folds each range and condition, elaborates each iteration's items with the loop variable bound as a constant, stores each named item as an array indexed by ordinal from 0 (carriers and nodes as composite-values' arrays, instances as `name[k]` children), elaborates the taken arm only, makes an empty array for an empty range, and raises `E0342` for a reference into an arm that was not elaborated or an index outside a generated array; a param read by a range or a condition joins `KeyParams` (GN-20 AC 1, DC7).
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

**What**: (gate: `Generation` admitted in its tests) A module that places itself under a generation `if` whose condition ends the recursion unrolls into nested instances; one whose recursion does not end is `E0341` naming the module at depth 1 024.
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

**What**: (gate: `Generation` admitted through `Project::admitting`) Source tests: `for k in 0..N { pub let rk = Resistor(..) }` in parallel gives `R / N` (Rust computes it), host paths `Top.rk[2]` read one element, a generation `if` over a param places the taken arm only, nested loops make a nested array.
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

**What**: (gate: `Generation` admitted through `Project::admitting`) Source test: the Book's `Ladder<const N: usize>` (recursion by generation) at `N = 1, 3, 8` gives, at an operating point and at a transient instant, the same node voltages as the ladder written out section by section (both models in the test's folder; equal to the bit).
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

**What**: `Generation` leaves `UnfinishedForms::IN_FORCE` (its opt-ins and `Project::admitting(Generation)` leave). Flip modules#1 and modules~b53307eb (the ladder for an operating point and a DC sweep, the output equal to the input at every point), modules~4bcb9f1d (`Ladder::<0>` now `E0304`), modules~4bd9131d (`for k in 0..count` with a signal now `E0340`) and cheatsheet#3 (the body items card with its generation `for`); rewrite the chapter's "Run it" box with what the binary prints (`**Run clibook**`); re-pin modules#2 to the `digital` generation `P0001` alone and modules#10 and cheatsheet#8 without generation; `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `tests/book/modules.rs`
**Depends on**: T76
**Reuses**: `Chapter`, `Faulty`, `Commands`
**Requirement**: GN-31 (AC 2), GN-13 (AC 4), GN-17 (AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `the_ladder_runs_for_an_operating_point_and_a_sweep`, `e0304_a_generic_ladder_of_zero_sections` and `e0340_a_generation_range_read_from_a_signal` pass under `**Run bookmodules**`
- [ ] `the_body_items_card_runs_with_its_generation` and `the_template_card_is_refused_for_its_template_and_bundle` pass under `**Run bookcheatsheet**`
- [ ] `the_half_reactions_are_refused_for_their_digital_generation` and `the_memory_bank_is_refused_for_its_template_and_bundle` pass under `**Run bookmodules**`; `**Run clibook**` passes; `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` (now `DigitalGeneration`) passes under `**Run typesmodel**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Full (last task of batch B12)

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the ladder and generation listings run`

---

### T78: The checker types generation in a `digital` block

**What**: (gate: `DigitalGeneration` admitted in its tests) `types/generation.rs` gains the `digital` form: `for k in 0..N { on lines[k].rise() { .. } }` with a const range, each reaction typed with `k` a constant, the element event over `Ty::CarrierArray` (composite-values C); `E0340` for a range that is not const; the `P0001` of `types/event.rs` leaves.
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

**What**: (gate: `DigitalGeneration` admitted in its tests) The elaborator makes one reaction per iteration of a generation `for` in a `digital` block, its event `EventSource::ElementChanged` of the element, each a distinct statement, kept in iteration order within source order (§12.1.2, §12.1.14).
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

**What**: `DigitalGeneration` leaves `IN_FORCE`, the last form: `UnfinishedForms`, `Types::check_admitting`, `Types::check_model_admitting` and `Project::admitting` are deleted, and `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` is replaced by its superseding acceptance `generation_in_a_digital_block_runs_from_every_entry` (`**Run srcgenr**`, through the plain project entry; the commit body names both). Source test: a bank of `N` flip-flops whose generated reactions each latch their own element on `lines[k].rise()`, and two reactions of one iteration committing in source order; flip modules#2, modules~87ce6f6e and modules~3f8e84ba (the `half` stream gives three records, `stage=1` to `stage=3`, in time order); `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `crates/piperine-parity/tests/source_generation.rs`
**Depends on**: T79
**Reuses**: `crates/piperine-parity/tests/generation/*.msl`, `tests/book/modules.rs`
**Requirement**: GN-19 (AC 1, AC 2), GN-31 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `a_generated_bank_reacts_per_element` and `generation_in_a_digital_block_runs_from_every_entry` pass under `**Run srcgenr**`
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
Proof: run srcgenr | test generation_in_a_digital_block_runs_from_every_entry ... ok

**C73** - Generated reactions keep iteration order within source order, each a distinct statement. (GN-19, AC 2; T79, T80)
Proof: run elabgenr | test generated_reactions_are_one_per_iteration_in_source_order ... ok
Proof: run bookmodules | test the_half_reactions_report_each_stage_in_time_order ... ok

**C74** - A param read by generation, a const generic argument, an array length or a template is structural. (GN-20, AC 1; T70, T73)
Proof: run elabmods | test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok

**C75** - A host override of a structural param before a run elaborates with it; a set or sweep during the run is `E0803` naming it as structural. (GN-20, AC 2, AC 3; T70)
Proof: run apisession | test a_structural_param_overrides_before_the_run_and_is_refused_during_it ... ok

**C76** - A generic module's body is checked once at its definition, and an unreached one with an error fails `piperine check` (S3). (GN-07, AC 2; GN-08, AC 1; T60, T71)
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

**C81** - `GAPS.md` drops L70, L69, S3, L30 and L31 in the commits that close them. (GN-31, AC 1; T54, T58, T71, T82)
Proof: run docs | test gaps_drops_l70 ... ok
Proof: run docs | test gaps_drops_l69 ... ok
Proof: run docs | test gaps_drops_s3 ... ok
Proof: run docs | test gaps_drops_what_generics_phase_c_closed ... ok

**C82** - Every Book row and gallery fence phase C lets run is held to it. (GN-31, AC 2; T71, T77, T80, T81)
Proof: run bookdigital | test the_flip_flop_runs_for_a_type_with_a_default ... ok
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


**C120** - The Book listings that contradicted the spec are corrected, and each old form is refused as the spec says: a carrier port left out is `E0312` (§8.1.8), a `[Bit; N]` with no initial value is `E0187` (§3.3.4), `Slice` is bounded by `Default` as gallery E.6 is. (GN-31, AC 2; Book amendments BA1 to BA3; T116)
Proof: run bookcorestd | test a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was ... ok
Proof: run bookcheatsheet | test an_array_of_bits_with_no_initial_value_is_e0187 ... ok
Proof: run booknatures | test the_slice_listing_bounds_t_by_default_as_gallery_e6_does ... ok

**C121** - A trailing `Option` value input is passed by name after the positional connections; a name that is none is `E0140`. (§8.1.9, §8.3.7; GN-13, AC 1; T117)
Proof: run typesmodules | test a_trailing_option_value_input_passed_by_name_binds_it ... ok
Proof: run typesmodules | test a_named_connection_that_is_no_trailing_option_input_is_e0140 ... ok
Proof: run elabresults | test a_value_input_passed_by_name_reads_its_argument ... ok

**C122** - No form of phase C is left behind the interim gate: generation in a `digital` block, the last one, runs from the plain production entry, and the gate is gone. (Interim strategy; T60 adds the gate, T71, T77 and T80 empty it; the interim pin `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` holds each intermediate list and is replaced at T80 by this proof)
Proof: run srcgenr | test generation_in_a_digital_block_runs_from_every_entry ... ok

**C123** - A `let` that feeds back through the result of a generic placement whose result reads only its own carriers is legal (`lowpass`, `PiAw` over `idt`). (§8.3.5; GN-13, AC 1; T61)
Proof: run typesmods | test a_generic_placement_feeds_back_through_its_unknown ... ok

**C124** - A param whose type is a type parameter is typed under its bounds and elaborated per instance (`reset: T`). (§8.1.15; GN-13, AC 1; T60, T71)
Proof: run typesmods | test a_param_of_a_type_parameter_is_typed_under_its_bounds ... ok

**C125** - A generic argument that does not implement a bound the module's carriers need is `E0187` naming the type and the trait (`Dff::<u8>`); a carrier handed to a generic `out` port needs no initial value. (§3.3.4, §3.8; GN-13, AC 1; T61, T71)
Proof: run bookdigital | test e0187_the_flip_flop_at_u8_has_no_default ... ok
Proof: run typesmods | test a_carrier_handed_to_a_generic_out_port_needs_no_initial_value ... ok

**C126** - `_` as a generic argument of a placement is inferred as if left out. (§1.2.12; GN-13, AC 1; T61)
Proof: run typesmods | test an_underscore_generic_argument_is_inferred ... ok

**C127** - A Book row still refused after a task that changed its refusal is pinned to its exact remaining refusals and their owners. (GN-31, AC 2; re-pin rule; T71, T77)
Proof: run bookmodules | test the_ladder_is_refused_for_its_generation ... ok
Proof: run bookmodules | test the_memory_bank_is_refused_for_its_template_and_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_for_its_template_and_bundle ... ok
Proof: run bookmodules | test the_half_reactions_are_refused_for_their_digital_generation ... ok
Proof: run booknatures | test a_bundle_is_refused_where_the_solver_reaches_it ... ok
Proof: run bookgenerics | test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok
Proof: run bookwelcome | test the_status_paragraph_lists_what_is_still_refused_and_one_item_runs ... ok
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

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
**Planning amendment after the T60 blocker (2026-11-23, design-c.md "Interim: the unfinished-forms gate" and "Gap verdicts"):**
(1) Interim strategy (b): the whole-model check and the run check keep refusing generic modules (and generation, in a body
and in `digital`) with `main`'s `P0001` until each form's flip task, behind `UnfinishedForms::IN_FORCE`; tests opt in by
naming the form. Flips: `GenericModules` T71 (moved to run right after T68, before T69 and T70, which need the plain entry:
parity's `Short` and the host's structural override), `Generation` T77, `DigitalGeneration` T80 (deletes the gate). No Book
test changes before its flip; at a flip each still-refused row is re-pinned to its exact remaining refusals.
(2) Verdicts: param of type `T` = real (§8.1.15), T60; param of type `C` (`Fn(Cell)`) = phase D T88; bundle with a carrier
field = `resolved-nets-bundles` T21 (amended to cover modules#10, modules#11, cheatsheet#8); generation in a body = T72 to T77;
E0332 on `lowpass`/`PiAw` = real (§8.3.5), the generic placement was not pushed on `placing`, T61; E0312 `Setup` with 2 of 3
ports = Book bug (§8.1.8), T116; `_` generic argument = real (§1.2.12), T61, its inference from a template param phase D T88;
named connection `reset: seat` = real (§8.1.9, §8.3.7), generic-agnostic, new T117; E0187 `bool`/`u8` no `Default` = spec-
correct (§3.3.4, MD-78): the test harnesses (`signal clk: Bit;` read only by `in` ports) and the plan's `Dff::<u8>` were wrong,
corrected in T71 (`Dff` at `Option<u8>`, `Dff::<u8>` proven `E0187`), plus two more Book bugs the same rule exposes:
cheatsheet#2 `signal s: [Bit; N]` and natures#8 `Slice<T: Repr>` with `signal buf: Bit<T>` (T116).
(3) New tasks T116 (Book amendments) and T117 (named connection); ids continue after phase E's T115 so no id moves. Phase C
is 32 tasks (T53 to T82, T116, T117), checks C51 to C86 and C120 to C127. B10 = T59, T116, T60, T61, T117, T62, T63, T64.
(4) Continue from the draft: merge `origin/wip/generics-c-b10` into T60/T61's work, then put `ModelCheck`'s seeding and the
placement typing behind the gate (the draft's `refuse_generic_placements` becomes `UnfinishedForms`); its `GAPS.md` S3 drop
and `gaps_drops_s3` move to T71.
**B10 done (T59, T116, T60, T61, T117, T62, T63, T64; 2026-11-23).** Executed under the user's AFK standing instruction; the
approval of the 2026-11-23 amendment (gate, G1 to G9, T116, T117) is pending, no Approved line written.
(1) The draft was taken as code and tests only (specs, `GAPS.md` S3 drop and `gaps_drops_s3` left for T71). The gate is
`types/unfinished.rs` (`UnfinishedForm`, `UnfinishedForms::{closed, admitting, admits, IN_FORCE}`), `Types::check_admitting` and
`Types::check_model_admitting`, the `Checker.forms` field; the pin `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed`
is in `types_model.rs`; tests admit through `GenericSources::admitting_found` and `Admitted` in `types_generic_modules.rs`.
Generation and digital generation are not yet read from the gate (their checker `P0001` is unchanged); T72/T78 start reading
`self.forms.admits(..)`. (2) T61 pushes the placement on `placing` before typing its arguments (G5), `_` generic arguments are
skipped in `explicit_arguments`. `[S; N]` (an array of a nature parameter) as a port type is still `P0001` "value input port of
type `[S; N]`": owner T68 or the ladder tasks if the Book needs it. (3) T117: `pom::PortArguments` (crate-private) binds
positional then named arguments; the POM already reports `E0140` for every bad name, so the checker only counts; the
`hoist.rs` elaborator reads the same binding; the `types_subset` row "named argument of a result module" left. (4) T63 and T64
were already satisfied by T56 and the earlier phases: their tests pin it (T64 over plain generic `const fn`s); the half of
T64 that reads `N` of a generic *module* (`[0.0; N + 1]`, `[false; N]`) is proven in T65 to T68's tests. (5) Env: commands over
about 120 s are moved to the background by the harness and notify on completion; helper scripts in
`/var/tmp/piperine-tmp-d/helpers` (`gate.sh`, `build.sh`, `lang.sh`, `commit.sh`).
**Dispatched:** none.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 13 → Phase 14 → Phase 15 → Phase 16 → Phase 17

Phase 13:  T53 → T54 → T55 → T56 → T57 → T58
Phase 14:  T59 → T116 → T60 → T61 → T117 → T62 → T63 → T64
Phase 15:  T65 → T66 → T67 → T68 → T71 → T69 → T70
Phase 16:  T72 → T73 → T74 → T75 → T76 → T77
Phase 17:  T78 → T79 → T80 → T81 → T82
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B9 | 13 | T53 to T58 | starts on `main` as it is (no composite-values C); closes L70 and L69; T55 retires the `cast`/`same` intrinsics (user, 2026-11-23) |
| B10 | 14 | T59, T116, T60, T61, T117, T62, T63, T64 | starts after `composite-values` phase C is on `main` (T59 merges it); Book amendments BA1 to BA3; generic modules checked behind the gate |
| B11 | 15 | T65 to T68, T71, T69, T70 | generic module instances; T71 opens the gate for generic modules, closes S3, flips or re-pins the generic-module Book rows |
| B12 | 16 | T72 to T77 | generation in a body behind the gate; T77 opens it; the ladder rows flip |
| B13 | 17 | T78 to T82 | needs composite-values C's element events; T80 opens the last form and deletes the gate; closes L30 and L31 |

# Digital and Mixed MSL Through the Front End Tasks

**Approved**: 2026-11-15 - user: "Sim, por favor! Achei que já tivesse no escopo completo" (standing delegation)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-digital/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-ir --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-interp --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-plan --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-transpile --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run tint**: `cargo test -p piperine-lang --test types_integers`
**Run tcomp**: `cargo test -p piperine-lang --test types_compound`
**Run tcar**: `cargo test -p piperine-lang --test types_carriers`
**Run tev**: `cargo test -p piperine-lang --test types_events`
**Run tres**: `cargo test -p piperine-lang --test types_results`
**Run tbuck**: `cargo test -p piperine-lang --test types_buck`
**Run evalues**: `cargo test -p piperine-lang --test elab_values`
**Run edig**: `cargo test -p piperine-lang --test elab_digital`
**Run eres**: `cargo test -p piperine-lang --test elab_results`
**Run emix**: `cargo test -p piperine-lang --test elab_mixed`
**Run ebuck**: `cargo test -p piperine-lang --test elab_buck`
**Run iramend**: `cargo test -p piperine-ir --test amendments`
**Run frozen**: `cargo test -p piperine-interp --test frozen`
**Run planamend**: `cargo test -p piperine-plan --test amended_shape`
**Run amended**: `cargo test -p piperine-transpile --test amended`
**Run nets**: `cargo test -p piperine-transpile --test digital_circuit`
**Run project**: `cargo test -p piperine-parity --test project`
**Run bcommits**: `cargo test -p piperine-parity --test buck_commits`
**Run bdpwm**: `cargo test -p piperine-parity --test buck_dpwm`
**Run bcontrol**: `cargo test -p piperine-parity --test buck_control`
**Run bboundary**: `cargo test -p piperine-parity --test buck_boundary`
**Run btime**: `cargo test -p piperine-parity --test buck_time`
**Run brecords**: `cargo test -p piperine-parity --test buck_records`
**Run bpower**: `cargo test -p piperine-parity --test buck_power`
**Run bsupervisor**: `cargo test -p piperine-parity --test buck_supervisor`
**Run bwhole**: `cargo test -p piperine-parity --test buck_whole`
**Run bloop**: `cargo test -p piperine-parity --test buck_closed_loop`

Executors (S10): Sonnet implements; Opus implements the delicate tasks directly, those where a mistake gives a wrong run with no error: T24, T25, T27, T30, T33, T41, T42, T45, T57, T65. A different Opus verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` for each crate a command builds before running it.

**This feature starts after `ir-rust` phases A and B are merged into its worktree.** Every task names the `ir-rust` tasks it stands on in its `Needs from ir-rust` line; T1 confirms the merged code against the design before anything else. The feature has three parts: the front end (phases 2, 3, 5, 6, 7), the amendments behind the IR (phases 4 and 8), and behavior from source (phases 9 and 10). Phases 1 to 7 are mergeable alone: they give a type-checked, elaborated and pinned buck IR.

Rules every task follows:

- **The `ir-rust` design is the contract.** This feature emits the IR node set of `ir-rust` T24 to T28 and adds only the seven amendments of design.md. It does not reshape a node, a plan entry or a printer of `ir-rust`; a need to do so stops the task and is reported.
- **The POM is read, never written.** `Design` is borrowed shared; a hoisted module keeps its placement in the POM. `crates/piperine-lang/src/{msl,pom}/` are not edited: if a task seems to need a change there, stop and report.
- **`examples/buck/src` and `crates/piperine-lang/std` are the acceptance target and are not edited.** A form of theirs that does not check, elaborate or run is fixed in the compiler.
- **Subset by table.** What stays outside is refused through the exhaustive classifiers with no wildcard arm. A negative test of M1 whose form this feature makes legal is replaced, in the task that legalizes it, by a test of a form of DIG-07 AC 1 still refused; the commit body lists each replaced row. No assertion is loosened and no test is deleted without its replacement.
- **Codes.** A rule of `docs/spec/` carries its `Eppnn` code; a form not supported yet is `P0001` and names it; a plain type mismatch is `P0002`. Every diagnostic has a span.
- **Every `Ir` returned passes `Ir::check()`**; each test that gets an `Ir` asserts it.
- **Fail loud.** A form the code does not handle is a diagnostic, an `Unsupported` or an entry of `check()`; never a default value, a dropped statement or a skipped record.
- **The interpreter is frozen** (R1). `piperine-interp` changes only by refusal arms and in T22. None of its tests is edited.
- **`piperine-abi` and the three solver crates are not edited.** A defect found there stops the task and is reported with a hand-written ABI device that reproduces it.
- **No tolerance is written or retuned.** `crates/piperine-parity/src/contract.rs` keeps every bound; an analog comparison uses its constants. The one number of DIG-16 AC 3 is the SPEC's and is cited where it is asserted. A number you cannot explain: stop and report.
- **Tests state what the spec says, not what the code does.** Expected values are computed in the test from `docs/spec/`, abi.md or `examples/buck/SPEC.md`, each cited at its assertion. Test functions have exactly the names `## Checks` cites. No test sets an environment variable.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, modules named after what they do, no `unwrap()`/`expect()` on a user-input path. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `crates/piperine-parity/tests/parity_corpus.rs` (no literal tolerance), `.specs/features/codegen/context.md` (S8, S10, R1 to R4).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| The checker (`piperine-lang` `types/`) | unit | each AC of DIG-01 to DIG-07, one positive and one negative source per criterion, each code with its span | `crates/piperine-lang/tests/types_{integers,compound,carriers,events,results,buck}.rs` | `cargo test -p piperine-lang` |
| The elaborator (`piperine-lang` `elab/`) | unit | each AC of DIG-08 to DIG-10, DIG-11 (1 to 3), DIG-13 (1 to 3), DIG-14 (1, 2), DIG-15 (1, 2) and every edge case; printed IR read against the `.msl` | `crates/piperine-lang/tests/elab_{values,digital,results,mixed,buck}.rs`, `tests/fixtures/elab/buck.txt` | `cargo test -p piperine-lang` |
| The IR amendments | unit | each of A1 to A7: print, `check()` rule, dependency set; earlier goldens byte-identical | `crates/piperine-ir/tests/amendments.rs` | `cargo test -p piperine-ir --test amendments` |
| The interpreter's refusal | unit | each amended form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| The plan amendments | unit | each amended shape entry by entry; `piperine_abi::validate` | `crates/piperine-plan/tests/amended_shape.rs` | `cargo test -p piperine-plan --test amended_shape` |
| Generated code for the amended forms | integration | each AC of DIG-12 and the compiled halves of DIG-11 AC 4 and DIG-13 AC 4, on hand-built IR; the solver halves of DIG-09 AC 1 and 4 (T65) | `crates/piperine-transpile/tests/{amended,digital_circuit}.rs` | `cargo test -p piperine-transpile --test amended --test digital_circuit` |
| The project loader and behavior from source | integration | DIG-11 (4, 5), DIG-13 (4 to 7), DIG-14 (3 to 12), DIG-15 (3 to 7), DIG-16; expected values computed in the test from the specification | `crates/piperine-parity/tests/{project,buck_*}.rs`, `tests/buck/*.msl` | `cargo test -p piperine-parity` |
| Feature records, instruction and gap files | none | - (build gate only) | `.specs/`, `GAPS.md`, `ROADMAP.md`, `examples/buck/*.md`, `CLAUDE.md`, `AGENTS.md` | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | the task's `Run` line |
| Full | every task with tests | the task's `Run` line, then `cargo test -p <the crate the task edits>` |
| Build | the last task of each phase, every task that edits two crates, every task with `Tests: none` | Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Ten phases, run in order. Phases 1 to 7 are the front end and the IR; phases 8 to 10 run it.

### Phase 1: The contract and the loader

Executor: Sonnet.

```
T1 → T2
```

### Phase 2: Types: values

Executor: Sonnet.

```
T3 → T4 → T5 → T6 → T7 → T8
```

### Phase 3: Types: carriers, events, reactions, results

Executor: Sonnet.

```
T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16
```

### Phase 4: The IR amendments

Executor: Sonnet.

```
T17 → T18 → T19 → T20 → T61 → T21 → T64 → T22
```

### Phase 5: Elaboration: values

Executor: Opus for T24, T25, T27; Sonnet for the rest.

```
T23 → T24 → T25 → T26 → T27
```

### Phase 6: Elaboration: structure

Executor: Opus for T30, T33; Sonnet for the rest.

```
T28 → T29 → T30 → T31 → T32 → T33 → T34 → T35 → T36
```

### Phase 7: The buck through elaboration

Executor: Sonnet.

```
T37 → T38
```

### Phase 8: Plan and generated code for the amended forms

Executor: Opus for T41, T42, T45, T65; Sonnet for the rest. T62, T63 and T65 close the phase.

```
T39 → T40 → T41 → T42 → T43 → T44 → T45 → T46 → T62 → T63 → T65
```

### Phase 9: Behavior from source: the slice

Executor: Sonnet.

```
T47 → T48 → T49 → T50 → T51 → T52 → T53
```

### Phase 10: Behavior from source: the converter

Executor: Sonnet; T57 is Opus's.

```
T54 → T55 → T56 → T57 → T58 → T59 → T60
```

---

## Task Breakdown

## Phase 1 tasks

### T1: The contract with the merged ir-rust

**What**: Read the merged `piperine-ir`, `piperine-plan` and `piperine-transpile` and write `contract.md`: for every row of design "What `ir-rust` gives and what this feature adds" and "Dependencies on `ir-rust`", the actual type, variant, builder method, fixture and golden file name, and how `Text::ident` prints a name holding `.` or `$`.
**Where**: `.specs/features/msl-digital/contract.md`
**Depends on**: None
**Reuses**: `.specs/features/ir-rust/design.md` as merged; `crates/piperine-ir/src/{model,build}.rs`; `crates/piperine-ir/tests/support/digital.rs`
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T1 to ir-rust T46, merged
**Executor**: Sonnet

**Done when**:

- [x] Every name this design takes from the `ir-rust` design is confirmed or corrected, one row each
- [x] A node this design assumes and the merged IR lacks, or holds in another shape, is listed under "Differences"; a difference that changes a lowering of design.md stops the feature and is reported before T2
- [x] `ir-rust` phases A and B are merged into the worktree: `cargo test -p piperine-ir --test digital` and `cargo test -p piperine-transpile --test dpwm` pass
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): msl-digital contract with the merged ir-rust`

---

### T2: Project: a crate graph to a compiled circuit

**What**: `Project`: a root crate and named dependency crates from disk, `elaborate(top)` and `compile(top)` on the compiled backend, giving the `Ir` and the circuit (design "Project").
**Where**: `crates/piperine-parity/src/project.rs`
**Depends on**: T1
**Reuses**: `crates/piperine-parity/src/bench.rs` (how a rejection names its stage); `piperine_lang::pom::{CrateGraph, Design::load_crates}`; `Compiled::circuit`
**Requirement**: DIG-15
**Needs from ir-rust**: ir-rust T16, ir-rust T17, ir-rust T21
**Executor**: Sonnet

**Done when**:

- [x] The first test loads a two-crate analog fixture under `crates/piperine-parity/tests/buck/` (a root that places a resistor divider from a dependency crate) and reads its operating point
- [x] No test sets an environment variable; the cache directory is under `CARGO_TARGET_TMPDIR`
- [x] Tests with exactly these names pass in `cargo test -p piperine-parity --test project`: `a_project_with_a_dependency_crate_loads_and_runs`, `a_diagnostic_is_returned_with_its_stage`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: build

**Commit**: `feat(parity): Project loads a crate graph onto the compiled backend`

---

## Phase 2 tasks

### T3: Ty: integer types, literals and constants

**What**: `Ty::{UInt, Int}`, the aliases, integer `const` items, and local bidirectional literal typing with the `literal` side table (design "The checker's new side tables").
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: None in this phase (phase 1 complete)
**Reuses**: `crates/piperine-lang/src/types/{check,expr}.rs` (expected-type plumbing of M1)
**Requirement**: DIG-01
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] The second test has one case per expectation source of DIG-01 AC 2 and one with none (`i32`)
- [x] Rows of `tests/types_subset.rs` that assert `P0001` for an integer type are replaced by rows of DIG-07 AC 1 still refused; the commit body lists them
- [x] `Float<16>`, `Float<128>`, `f16` and `f128` are `P0001` naming the type as written, the span on the type where it is written (a param, a return, a `let`), as `u128` and `f32` are: the float half of `ir-rust` RS-10 AC 5 that the IR cannot hold (`ir-rust` verification F-B8, contract.md)
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_integers`: `integer_types_and_constants_have_types`, `a_literal_takes_the_expected_integer_type_else_i32`, `a_literal_out_of_range_is_e0909`, `an_unsupported_width_or_float_is_named`, `a_width_below_one_is_e0103`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): integer types, literals and constants`

---

### T4: Integer operators and casts

**What**: Arithmetic, bitwise, comparison and shift operators on integers, and `as` among integers and `f64`.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T3
**Reuses**: the `f64` operator arms of the same file
**Requirement**: DIG-01
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `rev3`, `dithered_width`, `target_codes`, `in_window` and `ovp_threshold` of the buck are among the positive cases, read from `examples/buck/src`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_integers`: `integer_operators_take_one_type`, `mixed_integer_operands_are_p0002`, `a_shift_takes_any_integer_amount`, `as_goes_between_integers_and_f64`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): integer operators and casts`

---

### T5: The catalogue: integer and float rows, and the report of G3

**What**: Catalogue rows `min`, `max`, `clamp` (integers and `f64`) and `floor`; a method call with no row is `P0001` naming the method and the receiver's type; every accepted call has a callee or a row.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T4
**Reuses**: the `F64_METHODS` rows and `Catalogue::method` of M1
**Requirement**: DIG-01, DIG-02
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] The second test checks `x.saturating_add(1)` on a `u4` value and reads `saturating_add` and `u4` in the message
- [x] The third test walks every `Call` and `MethodCall` of the reachable set of three accepted models and finds `Types::callee`, `Types::method` or `Types::builtin` for each
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_integers`: `min_max_clamp_and_floor_are_typed`, `an_uncatalogued_method_is_p0001_by_name`, `every_accepted_call_has_a_callee`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): catalogue rows for integers; an unknown method is reported`

---

### T6: Unit enums

**What**: `Ty::Enum`: an enum of unit variants deriving `Repr`, its variant paths, `==` and `!=`; a payload, a discriminant or `#[repr]` is `P0001`.
**Where**: `crates/piperine-lang/src/types/compound.rs`
**Depends on**: T5
**Reuses**: `crate::pom::DeclKind::{Enum, Variant}`; `Reflect` for derives
**Requirement**: DIG-03
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `Mode`, `Fault` of the buck and `Severity` of the stdlib are the positive cases
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_compound`: `unit_enums_and_their_variants_are_typed`, `an_enum_outside_the_subset_is_p0001`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): unit enums`

---

### T7: Option and tuples

**What**: `Ty::{Option, Tuple}`: `Some`, `None`, `is_some`, `is_none`, `unwrap_or`, the implicit `Some` of §3.2.3 at a call, and a tuple `let`.
**Where**: `crates/piperine-lang/src/types/compound.rs` (modify)
**Depends on**: T6
**Reuses**: T6; `types/call.rs` (argument checking)
**Requirement**: DIG-03, DIG-06
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] A tuple anywhere but the two sides of a `let` is `P0001`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_compound`: `option_values_and_their_methods_are_typed`, `a_tuple_let_names_each_component`, `a_value_for_an_option_parameter_is_some_of_it`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): Option and tuple lets`

---

### T8: match and if let

**What**: `match` over enums, `bool`, integers and `Option` with the six pattern forms, exhaustiveness, and `if let Some(x) = e`.
**Where**: `crates/piperine-lang/src/types/control.rs`
**Depends on**: T7
**Reuses**: T6, T7; the `if` arm of `types/expr.rs`
**Requirement**: DIG-03
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] The `match` of `Supervisor` and the `if let` of `SetpointReg` are positive cases, as functions over values
- [x] §1 rule 14: an identifier pattern that resolves to a unit variant or a constant is a path, not a binding
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_compound`: `match_is_typed_over_enums_bools_integers_and_options`, `a_match_that_is_not_exhaustive_is_p0002`, `if_let_some_binds_the_payload`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `feat(types): match and if let`

---

## Phase 3 tasks

### T9: Discrete carriers and the flag reads

**What**: `Ty::Carrier` for `Bit<T>` signals and ports, the read of a carrier, the built-ins `known`, `known_or`, `is_known`, `high` (only when resolved to the stdlib's own item), catalogue methods on a carrier receiver, and initial values.
**Where**: `crates/piperine-lang/src/types/carrier.rs`
**Depends on**: None in this phase (phase 2 complete)
**Reuses**: `crates/piperine-lang/std/{carriers,values}.msl`; `crate::pom::DeclKind::{Signal, Port}`; node typing in `types/module.rs`
**Requirement**: DIG-02, DIG-04
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `Types::builtin` is set for the four reads and for `prev`; a project that redefines `known_or` gets `P0001` at its use
- [x] `crates/piperine-lang/src/pom/` is not edited: a `Deferred` or `Unresolved` member is typed here or reported
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_carriers`: `carriers_of_bit_are_typed_by_their_value`, `a_carried_type_without_repr_is_e0112`, `a_carrier_read_has_its_value_type`, `the_flag_reads_are_typed`, `an_initial_value_of_the_wrong_kind_is_e0210`, `a_catalogue_method_on_a_carrier_is_typed_on_its_value`, `a_single_variant_enum_cannot_be_carried`, `prev_is_typed_on_a_carrier`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 8 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): discrete carriers and their flag reads`

---

### T10: Definitions

**What**: A definition `place = expr` with or without `after`: its type, its target, the discrete-expression rule through the `continuity` side table, one definition per carrier, initial values, loops.
**Where**: `crates/piperine-lang/src/types/definition.rs`
**Depends on**: T9
**Reuses**: T9; `crate::pom::DeclKind::Definition`
**Requirement**: DIG-04
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `Types::continuity` exists and marks a probe, `now()`, `analysis(Static)`, `gmin()`, `source_scale()` and anything that reads one as continuous
- [x] The second test has a case through a `let` and one through a function argument
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_carriers`: `a_definition_of_another_type_is_e0337`, `a_continuous_definition_is_e0336`, `a_target_the_module_may_not_drive_is_e0222`, `two_definitions_of_one_carrier_are_e0335`, `an_initial_value_under_an_undelayed_definition_is_e0338`, `a_loop_of_definitions_is_e0339`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 6 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): definitions and the discrete-expression rule`

---

### T11: Discrete ports at a placement

**What**: A placement's carrier arguments: a carrier or a discrete expression at an `in` port, a carrier the parent hands over at an `out` port, value types, one drive per carrier across the placements of a module.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T10
**Reuses**: the node and param checks of the same file (`E0312`, `E0251`, `E0320`)
**Requirement**: DIG-04
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `Controller` and `PowerStage` of the buck, checked as tops over stub children, are positive cases
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_carriers`: `a_continuous_expression_at_an_in_port_is_e0316`, `a_second_drive_is_e0220`, `an_initial_value_without_the_drive_is_e0223`, `an_out_port_takes_only_a_carrier_the_parent_hands_over`, `a_carrier_of_another_value_type_is_e0251`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): carrier ports at a placement`

---

### T12: Events

**What**: `Ty::Event`: `changed`, `rise`, `fall`, the core `cross` and `at`, `||`, `&&` with a guard, event positions.
**Where**: `crates/piperine-lang/src/types/event.rs`
**Depends on**: T11
**Reuses**: T9, T10 (`continuity`); the core rows of `types/catalogue.rs`
**Requirement**: DIG-05
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] A `ttol` or `etol` given to `cross` is `P0001`; `settle` is `P0001`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_events`: `event_expressions_are_typed`, `a_bool_head_is_e0430`, `a_misplaced_not_or_and_is_e0440`, `an_event_outside_an_event_position_is_e0441`, `a_continuous_at_is_e0442`, `cross_or_at_outside_an_event_position_is_e0443`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 6 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): events`

---

### T13: Reactions and commits

**What**: `digital for M` blocks: the head, the body's statements, commits with `after`, probes and environment reads in a reaction.
**Where**: `crates/piperine-lang/src/types/reaction.rs`
**Depends on**: T12
**Reuses**: T12; `crate::pom::{StmtKind::Commit, DeclKind::{Behavior, Reaction}}`; `Context` of `types/check.rs`
**Requirement**: DIG-04, DIG-05
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] The reactions of `Dpwm`, `Pid`, `RefRamp` and `Adc` of the buck are positive cases
- [x] The row `DigitalBlock` leaves `elab/unlowered.rs` only in T31; here `tests/types_subset.rs` rows for a `digital` block are replaced as T3 states
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_events`: `a_reaction_body_is_typed`, `a_commit_of_another_type_is_e0432`, `a_delay_that_is_not_f64_is_e0433`, `a_definition_and_a_commit_are_e0221`, `a_placement_in_a_reaction_is_e0350`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): reactions and commits`

---

### T14: emit! and the functions that reach it

**What**: `emit!` in a reaction, its field rules, the `emits` side table, and where a site may be reached from.
**Where**: `crates/piperine-lang/src/types/record.rs`
**Depends on**: T13
**Reuses**: T13; `crates/piperine-lang/std/io.msl`; Part VI §19 rules 10 to 15
**Requirement**: DIG-05
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `log` and `require` of the stdlib type-check when called from a reaction; `Ty::Str` exists for their `&str` parameters and is refused as a carried type and as a `let` in a module body
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_events`: `emit_is_typed_in_a_reaction`, `a_repeated_or_reserved_field_is_e0604`, `a_field_type_conflict_is_e0605`, `emit_where_it_is_not_allowed_is_e0601`, `a_check_reached_from_analog_is_e0603`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): emit! and its placement rules`

---

### T15: Modules with a result, time and the analysis

**What**: A module with value inputs and a result, its placement in the positions of §8.3.4, `now()`, `analysis(Static)`, the `E0150` check of §3.3.6 and the self-read checks.
**Where**: `crates/piperine-lang/src/types/result.rs`
**Depends on**: T14
**Reuses**: T7 (optional parameters); `Allowed` and the core rows of `types/catalogue.rs`; §8.3, §14.4
**Requirement**: DIG-06
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] `transition`, `above` and `init` of the stdlib are positive cases, placed as the buck and `A2D` place them
- [x] The `E0150` test has a direct read, one through a `let` and one through a value input, and reads the carrier's name in each message
- [x] `analysis` of any kind but `Static` is `P0001`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_results`: `a_module_with_a_result_is_typed`, `a_placement_has_its_result_type`, `trailing_option_inputs_may_be_left_out`, `a_placement_in_a_const_context_is_e0352`, `now_and_analysis_static_are_typed`, `an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150`, `a_let_that_reads_itself_is_e0332_or_e0353`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 7 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(types): modules with a result, now() and analysis(Static)`

---

### T16: What stays outside, and the buck

**What**: The subset tables after this feature: every form of DIG-07 AC 1 is refused by name, and `Buck` type-checks clean.
**Where**: `crates/piperine-lang/src/types/unsupported.rs`
**Depends on**: T15
**Reuses**: the exhaustive, wildcard-free `match`es of the same file
**Requirement**: DIG-07
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [x] The first test is a table with one negative source per construct of DIG-07 AC 1, each read for `P0001` and for the construct's name
- [x] The second test loads `examples/buck` from disk as `buck_example.rs` does and calls `Types::check(&design, "Buck")`
- [x] `buck_example.rs` stays green and unedited
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_buck`: `what_stays_outside_is_p0001_by_name`, `the_buck_type_checks_with_no_diagnostic`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `feat(types): the subset after msl-digital; the buck type-checks`

---

## Phase 4 tasks

### T17: IR: the time and the static reads

**What**: `Env::{Time, Static}` with builder, typing (`F64`, `Bool`), dependency sets, printer forms, and the `check()` rule that neither is read by a drive, an event guard or an `At` argument (design A1, A2).
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None in this phase (phase 3 complete)
**Reuses**: `Env::{Temperature, Gmin, SourceScale}` and their arms
**Requirement**: DIG-10
**Needs from ir-rust**: ir-rust T24 to ir-rust T28
**Executor**: Sonnet

**Done when**:

- [x] `piperine-interp` and `piperine-transpile` still build: each new variant gets a refusal arm (`UNEXECUTED`, `Unsupported`)
- [x] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `time_and_static_reads_print_and_check`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the time and the static reads`

---

### T18: IR: the at event

**What**: `EventSource::At(ExprId)` with builder, printer and `check()` (an `F64` argument that reads no probe).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T17
**Reuses**: `EventSource::{Changed, Cross}`
**Requirement**: DIG-10
**Needs from ir-rust**: ir-rust T27
**Executor**: Sonnet

**Done when**:

- [ ] The dependency set of an `At` event lists the carriers and params its argument reads
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `an_at_event_prints_and_checks`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the at event`

---

### T19: IR: emit sites and streams

**What**: `Stmt::Emit`, `EmitField`, `BodyData::streams`, `Body::record_site`, with builder, printer and `check()` (inside a reaction; a field index in range; one type per field of a stream).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T18
**Reuses**: `Stmt::Commit`; `Site` (how a body-level list is indexed)
**Requirement**: DIG-11
**Needs from ir-rust**: ir-rust T27
**Executor**: Sonnet

**Done when**:

- [ ] A reaction's statement index counts commits and emits together, in source order: it is the ABI statement index
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `an_emit_site_prints_and_checks`, `a_record_site_gives_its_stream_and_texts`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): emit sites and streams`

---

### T20: IR: the flow of a potential branch

**What**: `Expr::Flow { branch }` and a `Ddt` addend under a `Potential` contribution, with builder, printer, dependency sets and `check()` (the branch has a potential contribution in the body).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T19
**Reuses**: `Expr::Probe`, `Stmt::Contribute`, `Operator::Ddt`
**Requirement**: DIG-13
**Needs from ir-rust**: none (M1 nodes)
**Executor**: Sonnet

**Done when**:

- [ ] A potential contribution with a `Ddt` addend passes `check()` (contract.md D5: the merged `check()` has no rule against it to replace); a flow read of a branch with no potential contribution is one entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `a_flow_read_and_a_potential_ddt_print_and_check`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the flow of a potential branch and ddt under a potential`

---

### T61: IR: the previous value of a carrier

**What**: `Expr::Previous(u32)` of type `Flagged`, with builder, printer, dependency sets and `check()` (the carrier exists; in `analog` only under a flag read, as a carrier read) (design A7).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T20
**Reuses**: `Expr::Carrier` and its `check()` rules
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T26, ir-rust T28
**Executor**: Sonnet

**Done when**:

- [ ] `piperine-interp` and `piperine-transpile` still build: the new variant gets a refusal arm
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `a_previous_value_read_prints_and_checks`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the previous value of a carrier`

---

### T21: IR: a child param that reads the parent

**What**: `ChildValue::{Const, Expr}` for a child's param values, stored beside `Child` (contract.md D4: `Child::params` keeps its constants; `IrBuilder::child_param`, `Body::child_params`), with builder, printer and `check()` (the expression reads only params of the parent body).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T61
**Reuses**: `Child`, `Constant`
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T29
**Executor**: Sonnet

**Done when**:

- [ ] The second test prints every M1 fixture and every `ir-rust` T29 fixture and compares with `crates/piperine-ir/tests/golden/`, byte for byte
- [ ] `Child` gains no field: every literal `Child { .. }` of `piperine-ir`, `piperine-lang` and the frozen `piperine-interp` tests compiles unchanged
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `a_child_param_may_read_the_parents_params`, `the_earlier_prints_are_unchanged`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): child param values as expressions`

---

### T64: IR: the carrier bindings of a child

**What**: `Body::child_carriers(child)`: per carrier port of the child's body, in its order, the parent's carrier it is bound to or open; set by `IrBuilder::bind_child_carriers(body, child, Vec<Option<u32>>)`, stored beside `Child` (contract.md D4), printed only when present, and checked by `check()` (one entry per carrier port, an index in range, equal value types) (`ir-rust` "Left for msl-digital": `Child` records no carrier binding).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T21
**Reuses**: `Child::nodes` (the node twin); `IrBuilder::{carrier, bind_nets}` of `ir-rust` T26
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T26, ir-rust T28
**Executor**: Sonnet

**Done when**:

- [ ] The authored tree shows what each placement connects, nodes and carriers, as the POM-navigability rule of `CLAUDE.md` asks; `Child` gains no field
- [ ] Every M1 and T29 golden print stays byte-identical (no binding, no line)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test amendments`: `a_child_records_its_carrier_bindings`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the carrier bindings of a child`

---

### T22: The interpreter refuses the amended forms

**What**: `Interpreter::new` returns `Rejected` naming each body that holds a time or static read, an `At` event, an emit site, a flow read or a previous-value read.
**Where**: `crates/piperine-interp/src/lib.rs`
**Depends on**: T64
**Reuses**: the refusal of `ir-rust` T30
**Requirement**: DIG-10
**Needs from ir-rust**: ir-rust T30
**Executor**: Sonnet

**Done when**:

- [ ] No other line of `piperine-interp` changes beyond the refusal arms T17 to T20 and T61 added
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test frozen`: `a_body_with_an_amended_form_is_rejected_by_name`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): the amended forms are refused by name`

---

## Phase 5 tasks

### T23: Lowering integers, and folding them

**What**: Integer types, literals, operators, casts and methods to IR nodes; `ConstEval` over integers with wrapping, the G4 cast and the G5 shift; a constant panic is `E0104`.
**Where**: `crates/piperine-lang/src/elab/lower.rs`
**Depends on**: None in this phase (phase 4 complete)
**Reuses**: `crates/piperine-lang/src/elab/consts.rs` (`ConstEval`, `ConstFault`); the `f64` arms of `lower.rs`
**Requirement**: DIG-08
**Needs from ir-rust**: ir-rust T24
**Executor**: Sonnet

**Done when**:

- [ ] The third test asserts `300.0 as u8` is 255, `NaN as u8` is 0, `1u8 << 9` is 0, `-1i8 >> 9` is -1 and `511u9 + 1` is 0, each cited to G4, G5 or §1 rule 6
- [ ] Every `Ir` a test gets passes `Ir::check()`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_values`: `integers_lower_to_typed_ir`, `integer_constants_fold`, `folded_casts_shifts_and_sums_wrap_as_compiled_code_does`, `a_constant_division_by_zero_is_e0104`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): integers lower and fold`

---

### T24: Known: which values may be unknown

**What**: The `Known` analysis and the `Flagged` IR type of every lowered expression; `&&` and `||` over a flagged operand as `BitAnd` and `BitOr`, refused when the right operand is partial.
**Where**: `crates/piperine-lang/src/elab/known.rs`
**Depends on**: T23
**Reuses**: design "Unknown-ness: the `Known` analysis"; §3.3.1, §3.3.2, §3.3.12
**Requirement**: DIG-08
**Needs from ir-rust**: ir-rust T25, ir-rust T26
**Executor**: Opus

**Done when**:

- [ ] The first test reads the printed type of each row of the design's table
- [ ] An `if` on a flagged condition is lowered unchanged: the elaborator adds no default and no `known_or`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_values`: `a_carrier_read_is_flagged_and_the_flag_reads_are_not`, `a_short_circuit_over_a_flagged_operand_becomes_bitwise`, `a_partial_right_operand_under_a_flagged_short_circuit_is_p0001`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): the Known analysis and flagged types`

---

### T25: Variants: one function per flag pattern

**What**: `Variants`: an IR function per function and flag pattern reached, in first-reach order, the all-plain one under the source name.
**Where**: `crates/piperine-lang/src/elab/variants.rs`
**Depends on**: T24
**Reuses**: `crates/piperine-lang/src/elab/functions.rs`; T24
**Requirement**: DIG-08
**Needs from ir-rust**: ir-rust T24, ir-rust T25
**Executor**: Opus

**Done when**:

- [ ] The test elaborates a reaction calling `dithered_width(duty.known_or(0), frame + 1)` and reads the functions `dithered_width~01` and `rev3~1`, and no `dithered_width`
- [ ] Every M1 function prints under its source name: `cargo test -p piperine-lang --test elab_entry` is green and unedited
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_values`: `a_function_gets_one_variant_per_flag_pattern`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): function variants by flag pattern`

---

### T26: Enums and match

**What**: An enum as its tag, a variant as its literal, a `match` as an `if` chain over a scrutinee bound once.
**Where**: `crates/piperine-lang/src/elab/compound.rs`
**Depends on**: T25
**Reuses**: T24; design "From a source form to IR nodes the contract already has"; §2 rule 5
**Requirement**: DIG-08
**Needs from ir-rust**: ir-rust T24
**Executor**: Sonnet

**Done when**:

- [ ] `Mode` is `UInt(2)` with `Hiccup` as 3; `Fault` and `Severity` are `UInt(2)`; an or-pattern is a `BitOr` of comparisons when flagged, an `Or` when not
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_values`: `an_enum_is_its_tag_and_a_match_is_an_if_chain`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): enums as tags, match as an if chain`

---

### T27: Option, tuples and if let

**What**: `Lowered::Pair`: an `Option` as a `bool` and a payload through `Some`, `None`, `known()`, `is_some`, `is_none`, `unwrap_or`, `if let`, `match`; a tuple `let` as one local per component.
**Where**: `crates/piperine-lang/src/elab/compound.rs` (modify)
**Depends on**: T26
**Reuses**: T26
**Requirement**: DIG-08
**Needs from ir-rust**: ir-rust T25
**Executor**: Opus

**Done when**:

- [ ] The first test asserts that no printed type of the elaborated IR is an `Option`, over `SetpointReg`'s reaction and `Supervisor`'s `fault_now`
- [ ] `unwrap_or` on a literal `None` or `Some` folds to its value
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_values`: `an_option_is_a_flag_and_a_payload`, `a_tuple_let_is_one_local_per_component`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `feat(elab): Option as a pair, tuple lets`

---

## Phase 6 tasks

### T28: Carriers and nets

**What**: Each discrete port and signal as a carrier of its body; the net table and the carrier bindings of every instance; the discrete ports of a top as nets.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: None in this phase (phase 5 complete)
**Reuses**: `crates/piperine-lang/src/elab/{body,place}.rs` (nodes, `NodeSpec`, the placement walk)
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T26
**Executor**: Sonnet

**Done when**:

- [ ] `top_has_a_port` of `elab/elaborator.rs` refuses only a continuous port; its M1 test is re-pointed to one
- [ ] An initial value is folded with `ConstEval`; one that reads a param is `P0001`
- [ ] Every placement's carrier arguments are written with `IrBuilder::bind_child_carriers` (T64), so the authored `Child` shows them
- [ ] A net's `init` is the folded initial value of the carrier that declares it, `None` when it has none (`ir-rust` F-B2; T65 passes it to the solver)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `carriers_follow_ports_then_signals`, `every_signal_carrier_is_a_design_net`, `a_top_with_discrete_ports_gets_one_net_per_port`, `a_carrier_with_no_value_and_no_drive_is_kept`, `a_placement_records_its_carrier_bindings`, `a_net_takes_the_init_of_its_declaring_carrier`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 6 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): carriers and design nets`

---

### T29: Drives and anonymous definitions

**What**: A definition as a `Drive`, with `after`; an expression at an `in` port as a signal carrier of the parent named `<instance>.<port>` and its drive.
**Where**: `crates/piperine-lang/src/elab/drives.rs`
**Depends on**: T28
**Reuses**: T28; `elab/children.rs` (the placement's arguments)
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T26
**Executor**: Sonnet

**Done when**:

- [ ] `GateDriver`'s two delayed drives and `Controller`'s `ramp.target` are the cases
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `a_definition_is_a_drive`, `an_expression_at_an_in_port_is_a_carrier_and_a_drive`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): drives and anonymous definitions`

---

### T30: Events and heads

**What**: `lower_head`: a head in disjunctive form, each guard split by `continuity`, as one reaction or one per disjunct (design "Events and reaction heads").
**Where**: `crates/piperine-lang/src/elab/events.rs`
**Depends on**: T29
**Reuses**: T24; `Types::{continuity, builtin}`
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T27; and on amendment A7
**Executor**: Opus

**Done when**:

- [ ] The first test reads the two guards of design "From a source form to IR nodes the contract already has" term by term against `std/values.msl` `rise` and `fall`; `c.prev()` in a guard and in a reaction body lowers to `Previous`
- [ ] The third test has both rows: `on a.rise() || b.rise()` is one reaction on two events; `on cross(e) || (c.changed() && v(p) > 0.0)` is two reactions in source order
- [ ] A flagged guard is kept as written: the generated code decides it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `changed_rise_fall_and_cross_lower_to_events`, `a_guard_splits_into_its_discrete_and_continuous_parts`, `an_or_head_is_one_reaction_or_one_per_disjunct`, `a_head_the_ir_cannot_hold_is_p0001`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): events, guards and heads`

---

### T31: Reactions and commits

**What**: A `digital` block's reactions: `let`, `if`, commits with `after`, probes, calls, in source order.
**Where**: `crates/piperine-lang/src/elab/reactions.rs`
**Depends on**: T30
**Reuses**: T30; `elab/lower.rs`
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T27
**Executor**: Sonnet

**Done when**:

- [ ] `Unlowered::DigitalBlock` is removed and `tests/elab_unlowered.rs::a_digital_block_is_p0001` is replaced by a test of a form still refused in a `digital` block (a `for` loop); the commit body says so
- [ ] The printed reactions of `Pid` and `Dpwm` are read statement by statement against their `.msl`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `a_reaction_body_lowers_in_source_order`, `a_module_with_only_a_digital_block_has_no_analog_statements`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): reactions and commits`

---

### T32: Child params that read the parent

**What**: A placement's param value over the parent's params: `ChildValue::Expr` in the authored child, the constant per instance in the flat table, `where` checked per instance.
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T31
**Reuses**: `crates/piperine-lang/src/elab/{consts,place}.rs` (`Bindings`, `bind_params`)
**Requirement**: DIG-09
**Needs from ir-rust**: none; it stands on amendment A6
**Executor**: Sonnet

**Done when**:

- [ ] The test places `Divider { r_top: 30k, r_bot: 10k }` twice with different values and reads each `Resistor` instance's `r`; a false `where` of the child reads `E0317` at the placement in the parent
- [ ] The field-init shorthand `{ l, dcr }` of `Buck` lowers as `{ l: l, dcr: dcr }`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `a_child_param_reads_the_parents_value`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): child params over the parent's params`

---

### T33: Hoisting modules with a result

**What**: `Hoist`: frames, hoisted carriers, drives, events and reactions, the result inlined at each read, value inputs substituted, the core `at`, `now()` and `analysis(Static)` (design "Hoisting a module with a result").
**Where**: `crates/piperine-lang/src/elab/hoist.rs`
**Depends on**: T32
**Reuses**: T28 to T31; `crates/piperine-lang/std/{analog,values}.msl` (`transition`, `above`, `init`)
**Requirement**: DIG-10
**Needs from ir-rust**: ir-rust T26, ir-rust T27; and on amendments A1 to A3
**Executor**: Opus

**Done when**:

- [ ] The first test elaborates `PowerSwitch` of the buck and reads the six `gate.*` carriers, and `A2D` and reads `above$0.init$0.up` and `above$1.init$0.up`
- [ ] A name resolves through the frame chain: a value input read in the callee's reaction lowers the caller's expression in the caller's frame (§8.3.8)
- [ ] A module that places itself through results is `E0341`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_results`: `a_placement_is_hoisted_under_its_name`, `value_inputs_are_substituted_at_each_read`, `an_omitted_option_input_reads_as_none`, `two_placements_are_two_sets_of_carriers`, `a_result_module_gets_no_body_and_no_instance`, `an_expression_at_a_discrete_input_drives_the_hoisted_carrier`, `at_now_and_static_lower_to_their_nodes`, `a_result_module_is_hoisted_once_per_body`, `a_param_block_binds_the_hoisted_params`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 9 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): modules with a result are hoisted`

---

### T34: emit! and the functions that emit

**What**: `Stmt::Emit` from `emit!`, the stream table of the body, a function that emits inlined at its call, text fields kept in the site.
**Where**: `crates/piperine-lang/src/elab/records.rs`
**Depends on**: T33
**Reuses**: T31; `Types::emits`; A4 of T19
**Requirement**: DIG-11
**Needs from ir-rust**: none; it stands on amendment A4
**Executor**: Sonnet

**Done when**:

- [ ] `Never` of the stdlib lowers to one reaction with one emit site on `checks`, texts `kind = require` and `name = never`, one value field `pass`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_results`: `emit_lowers_to_a_site_with_its_texts`, `a_function_that_emits_is_inlined`, `a_text_field_that_is_not_a_literal_is_p0001`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): emit sites`

---

### T35: The analog forms of the power stage

**What**: A flow probe of a potential branch, `ddt` under a potential contribution, and `analog` reading a hoisted result, a carrier through its flag reads, `now()` and `analysis(Static)`.
**Where**: `crates/piperine-lang/src/elab/contribute.rs`
**Depends on**: T34
**Reuses**: `crates/piperine-lang/src/elab/analog.rs`; `Unlowered::DdtOffTop`; A5 of T20
**Requirement**: DIG-13
**Needs from ir-rust**: none; it stands on amendment A5
**Executor**: Sonnet

**Done when**:

- [ ] `Inductor`, `CurrentSense`, `PowerSwitch` and `Load` of the buck are the cases, their printed analog statements read against the `.msl`
- [ ] `tests/elab_unlowered.rs::ddt_off_the_top_of_a_flow_contribution_is_p0001` keeps every case that is still refused; the potential case moves to this target as a positive one, and the commit body says so
- [ ] The checker types `i(a, b)` as an expression in the same commit (`types/analog.rs`), refused where the branch has no potential contribution
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_mixed`: `a_flow_probe_of_a_potential_branch_lowers`, `a_potential_contribution_keeps_its_ddt`, `a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001`, `analog_reads_a_hoisted_result`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(elab): flow probes and ddt under a potential contribution`

---

### T36: Invariants of elaboration

**What**: Tests over the whole elaborator: determinism, the untouched design, `check()` on every IR, the M1 goldens.
**Where**: `crates/piperine-lang/tests/elab_digital.rs`
**Depends on**: T35
**Reuses**: `tests/elab_entry.rs` (the same invariants for M1); `tests/support/elab.rs`
**Requirement**: DIG-07, DIG-09
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [ ] Each test runs over the modules of `examples/buck/src` and `std/{convert,monitor}.msl` that elaborate, one top at a time
- [ ] The fourth test compares the seven twins' IR with `tests/fixtures/elab/*.txt`, which this feature never edits
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_digital`: `elaborating_twice_gives_the_same_print`, `the_design_is_not_written`, `every_ir_passes_check`, `the_m1_goldens_are_unchanged`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `test(elab): determinism, the untouched design, check() and the M1 goldens`

---

## Phase 7 tasks

### T37: The slice equals the hand-built fixtures

**What**: Each buck module `ir-rust` T29 transcribed, elaborated from `examples/buck` and the stdlib and compared in structural print with its fixture's golden text; the extra items of `Dpwm` and `A2D` as data in the test.
**Where**: `crates/piperine-lang/tests/elab_buck.rs`
**Depends on**: None in this phase (phase 6 complete)
**Reuses**: `crates/piperine-ir/tests/golden/` (names from `contract.md`); `tests/buck_example.rs` (loading the package)
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T29
**Executor**: Sonnet

**Done when**:

- [ ] The guard of each `rise` event is a listed difference: the fixture has `high()`, the source gives the stdlib guard (ruling 2). The test replaces exactly those guards and nothing else; the fixtures are not edited for it
- [ ] A difference outside the listed items is not edited away: the task stops and reports which side misreads the `.msl`; a fixture of `piperine-ir` changes only with `cargo test -p piperine-transpile` still green, and the batch summary names it
- [ ] The second test removes the listed items from the elaborated print and asserts equality with the fixture, and asserts each listed item is present
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_buck`: `six_slice_bodies_equal_their_hand_built_fixtures`, `dpwm_and_a2d_differ_only_as_listed`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `test(elab): the buck slice from source equals the hand-built IR`

---

### T38: The whole buck elaborates

**What**: `Buck` elaborated from `examples/buck`: zero diagnostics, an empty `check()`, and its structural print pinned.
**Where**: `crates/piperine-lang/tests/elab_buck.rs` (modify)
**Depends on**: T37
**Reuses**: T37
**Requirement**: DIG-15
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [ ] The golden text `tests/fixtures/elab/buck.txt` is written once from the elaborator and read through by the implementer against `examples/buck/src/top.msl`: 22 bodies (the 20 modules of the package and the stdlib `A2D` and `Never`; `transition`, `above` and `init` are hoisted and have none), the instance tree of `Buck`, every net of `Controller`
- [ ] A form of the buck that still gives `P0001` is fixed in the task of its phase, not excused here
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_buck`: `the_buck_elaborates_with_no_diagnostic_and_a_sound_ir`, `the_buck_ir_is_pinned`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: build

**Commit**: `test(elab): the whole buck elaborates to a pinned IR`

---

## Phase 8 tasks

### T39: Plan: the ambient reads

**What**: `Source::Ambient(TIME | STATIC)` among the reads of `terms`, `cross:*` and reactions (design A1, A2).
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: None in this phase (phase 7 complete)
**Reuses**: `Source`, `FunctionPlan`, `ReactionPlan`; abi.md §4.2
**Requirement**: DIG-12
**Needs from ir-rust**: ir-rust T31 to ir-rust T34
**Executor**: Sonnet

**Done when**:

- [ ] `TIME` in `terms` has no partial; every amended shape passes `piperine_abi::validate`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `time_and_static_are_ambient_reads`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): ambient time and static reads`

---

### T40: Generated code: now() and analysis(Static)

**What**: The printers read the two ambient cells in `terms`, `cross:*` and reactions.
**Where**: `crates/piperine-transpile/src/print/digital.rs`
**Depends on**: T39
**Reuses**: the reaction and `terms` printers; `Eval` and `React` of `piperine-abi`
**Requirement**: DIG-12
**Needs from ir-rust**: ir-rust T37 to ir-rust T41
**Executor**: Sonnet

**Done when**:

- [ ] The first test commits `now()` from a reaction fired at 15 ns and reads 15e-9, and reads a source `v(p) +> now()` at three accepted times
- [ ] The second commits `analysis(Static)` at `(0, 0)` and at a transient instant: true, then false
- [ ] `Text::ident` prints a hoisted name holding `.` or `$` as a Rust identifier (`.` as `_d`, `$` as `_s`, contract.md D9); the ABI pin keeps the IR name; a collision with another name of the body is refused as a variant collision is
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test amended`: `now_is_the_time_of_the_instant_and_of_the_solve`, `static_is_true_only_in_the_equilibrium`, `hoisted_names_print_as_rust_identifiers`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): now() and analysis(Static)`

---

### T41: Plan: timers

**What**: Per `At` event: the private event pin, a `changed` event function per carrier its argument reads, and the arming reaction on those and on the global `START` pin with one `REPLACE` statement (design A3).
**Where**: `crates/piperine-plan/src/plan.rs` (modify)
**Depends on**: T40
**Reuses**: the delayed-drive plan of `ir-rust` T32; abi.md R1, C1, C3
**Requirement**: DIG-12
**Needs from ir-rust**: ir-rust T31 to ir-rust T34
**Executor**: Opus

**Done when**:

- [ ] The shape of a body holding `at(t0 + dt)` is read entry by entry and passes `piperine_abi::validate`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): timers as arming reactions`

---

### T42: Generated code: arming a timer

**What**: The arming body: an unknown or past time commits with delay `+inf`, the current time with delay 0, a later one with `t − now`.
**Where**: `crates/piperine-transpile/src/print/digital.rs` (modify)
**Depends on**: T41
**Reuses**: T41; the reaction printer; `rt::Commits`
**Requirement**: DIG-12
**Needs from ir-rust**: ir-rust T39, ir-rust T42
**Executor**: Opus

**Done when**:

- [ ] The first test arms `at(t0 + dt)` with 10 ns and 5 ns and reads one firing, at 15 ns
- [ ] The second commits `dt <- 20n` at 12 ns: no firing at 15 ns, one at 30 ns
- [ ] The third reads no firing for a time of 0 armed at 5 ns and for an unknown time, and a firing at the next microstep for a time equal to now
- [ ] A solver behavior that contradicts abi.md C1 or C3 stops the task and is reported with a hand-written ABI device that reproduces it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test amended`: `at_fires_once_at_its_time`, `a_rearmed_at_drops_the_earlier_firing`, `a_past_or_unknown_time_does_not_fire`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): timers fire once and re-arm`

---

### T43: Plan: record pins

**What**: Per stream of a body: a private discrete `RECORD` pin with one flagged field per value field; one statement per emit site (design A4).
**Where**: `crates/piperine-plan/src/plan.rs` (modify)
**Depends on**: T42
**Reuses**: the carrier pin layout of `ir-rust` T31; abi.md C5
**Requirement**: DIG-11
**Needs from ir-rust**: ir-rust T31 to ir-rust T34
**Executor**: Sonnet

**Done when**:

- [ ] Two sites of one stream share the pin; a field absent from a site has its known byte 0 in that site's bytes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `a_stream_is_a_record_pin_with_one_field_per_value`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): record pins`

---

### T44: Generated code: emit

**What**: `rt::Commits::record` and the printer arm of `Stmt::Emit`.
**Where**: `crates/piperine-transpile/src/print/digital.rs` (modify)
**Depends on**: T43
**Reuses**: T43; `rt::Commits`; `piperine_solver::protocol::Sink`
**Requirement**: DIG-11
**Needs from ir-rust**: ir-rust T39, ir-rust T42
**Executor**: Sonnet

**Done when**:

- [ ] The test installs a `Sink`, fires a reaction with one commit and one emit, and reads one record with its field bytes and the instant it was made, and the same count of microsteps as without the emit
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test amended`: `an_emit_commits_one_record_and_adds_no_microstep`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): emit writes a record`

---

### T45: Plan: the flow of a potential branch

**What**: `Source::BranchFlow`: `Expr::Flow` as a differentiable read of the branch's flow pin; a `Ddt` addend of a potential contribution as a `DDT` term on the branch's equation row (design A5).
**Where**: `crates/piperine-plan/src/plan.rs` (modify)
**Depends on**: T44
**Reuses**: the potential branch of the M1 voltage source; `Term`, the affine flags
**Requirement**: DIG-13
**Needs from ir-rust**: ir-rust T1 (the shared plan)
**Executor**: Opus

**Done when**:

- [ ] The M1 shapes are unchanged: `cargo test -p piperine-plan --test shared` is green and unedited
- [ ] The affine flag of the inductor's equation row is computed as for any term
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): branch flow reads and ddt on an equation row`

---

### T46: Generated code: the flow read

**What**: The `terms` printer seeds a dual on the flow pin and writes the `DDT` term of an equation row.
**Where**: `crates/piperine-transpile/src/print/analog.rs`
**Depends on**: T45
**Reuses**: T45; the `terms` printer and the affine rule of `ir-rust` T7
**Requirement**: DIG-13
**Needs from ir-rust**: ir-rust T7, ir-rust T10
**Executor**: Sonnet

**Done when**:

- [ ] `piperine_abi::verify::verify_partials` with `rel_tol` 1e-6 passes for every function of the hand-built inductor and VCVS bodies
- [ ] No golden source of `ir-rust` changes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test amended`: `the_inductor_functions_pass_verify_partials`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): flow reads and ddt on an equation row`

---

### T62: Plan: the previous value of a carrier

**What**: `Source::CarrierPrevious`: the `PREVIOUS` part of the carrier's pin among the reads of an event function and of a reaction (design A7).
**Where**: `crates/piperine-plan/src/plan.rs` (modify)
**Depends on**: T46
**Reuses**: `Source::{Carrier, CarrierChanged}` of `ir-rust` T31, T32; abi.md §4.1, §11.4
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T31, ir-rust T32
**Executor**: Sonnet

**Done when**:

- [ ] The event function of a `rise` reads `VALUE`, `PREVIOUS` and `CHANGED` of its carrier, as abi.md §11.4; the shape passes `piperine_abi::validate`
- [ ] A carrier a previous value is read of is not `QUIET`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test amended_shape`: `a_previous_value_is_the_previous_part_of_the_pin`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): previous-value reads`

---

### T63: Generated code: the previous value

**What**: The event and reaction printers decode `Expr::Previous` as a `Flagged` value from the `PREVIOUS` bytes.
**Where**: `crates/piperine-transpile/src/print/digital.rs` (modify)
**Depends on**: T62
**Reuses**: T62; the carrier read of `ir-rust` T38, T39
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T38, ir-rust T39
**Executor**: Sonnet

**Done when**:

- [ ] The test commits 3 then 5 to a `Bit<u8>`: a reaction on its change commits `prev` and reads unknown after the first commit (§5.3.8) and 3 after the second
- [ ] No golden source of `ir-rust` changes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test amended`: `a_previous_value_is_read_in_a_guard_and_in_a_reaction`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): previous-value reads`

---

### T65: A parent's signal on its children's net, and a net's own init

**What**: Real support for the two wirings `Netlist::circuit` refuses today (`ir-rust` "Left for msl-digital" F-B1, F-B2). A `signal` carrier that a child's carrier port is bound to (`Body::child_carriers`, T64) is a `PORT` pin of its body's shape, bound by the netlist to its net with `Port::Net`, so the parent's drives and commits reach the children. A net's `init` reaches the solver as the `HAS_INIT` of the declaring carrier's pin, now bound to the net; a net whose init no bound pin carries is seeded by name through one additive `CircuitBuilder` method, added only if a test needs it and named in the commit body as the one solver change.
**Where**: `crates/piperine-plan/src/netlist.rs` (modify)
**Depends on**: T63
**Reuses**: T64; `Netlist::circuit` and its `refuse_unbound_forms`; the `Wiring::{ParentSignal, NetInit}` fixtures of `crates/piperine-transpile/tests/digital_circuit.rs`
**Requirement**: DIG-09
**Needs from ir-rust**: ir-rust T31, ir-rust T41
**Executor**: Opus

**Done when**:

- [ ] `a_parent_signal_its_children_meet_on_is_refused` and `a_net_with_its_own_init_is_refused` of `ir-rust` (C106, C107) are replaced by the two tests below, which run the same wirings; the commit body lists the replaced rows
- [ ] The first test drives the flip-flop's clock from a commit of the parent's `signal` and reads `q` true after the first rising edge; the negative control (the signal not bound to the child) leaves `q` unknown
- [ ] The second test reads the net's value at `(0, 0)` as its `init`, before any write
- [ ] A shape with a `signal` made a `PORT` pin passes `piperine_abi::validate`; an analog-only design's shape and every `ir-rust` golden source are unchanged
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test digital_circuit`: `a_parent_signal_reaches_its_childrens_ports`, `a_net_starts_at_its_own_init`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: build

**Commit**: `feat(plan): a parent's signal reaches its children; a net starts at its init`

---

## Phase 9 tasks

### T47: Commit semantics from source

**What**: Four MSL modules, each the example of its rule in `docs/spec/part_iv_behavior.md` §12.1, elaborated and run.
**Where**: `crates/piperine-parity/tests/buck_commits.rs`
**Depends on**: None in this phase (phase 8 complete)
**Reuses**: T2 (`Project`); `crates/piperine-transpile/tests/commits.rs` (how a net's changes are collected)
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The MSL sources are in `crates/piperine-parity/tests/buck/commits.msl`, each module under the spec paragraph it transcribes
- [ ] Expected instants and values are written from §12.1.1, .2, .4 and .18, each cited at its assertion
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_commits`: `two_commits_exchange_two_carriers`, `the_later_statement_wins`, `two_statements_make_a_pulse`, `an_unchanged_commit_fires_nothing`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): commit semantics from MSL source`

---

### T48: The oscillator and the DPWM from source

**What**: A bench crate that depends on the buck package: `Osc` alone for 100 ns, then into `Dpwm` with `run` true and duty word 1325 for 8 periods; the determinism run.
**Where**: `crates/piperine-parity/tests/buck_dpwm.rs`
**Depends on**: T47
**Reuses**: T47; `crates/piperine-transpile/tests/dpwm.rs`; buck SPEC §5, §9
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The bench modules are in `crates/piperine-parity/tests/buck/bench.msl` and place `buck::dpwm::{Osc, Dpwm}` from `examples/buck`, read from disk
- [ ] The expected widths, the 4-count dead times and the 396 limit are computed in the test from SPEC §5, not read back from the model
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_dpwm`: `the_oscillator_toggles_every_half_period`, `the_dpwm_dithers_keeps_dead_time_and_never_overlaps`, `two_runs_give_the_same_changes`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the oscillator and the DPWM run from source`

---

### T49: The ramp, the PID and the setpoint register from source

**What**: `RefRamp` and `Pid` under a 200-sample stimulus written in MSL (a sample counter and `if` chains: soft-start, a step of the measurement, saturation both ways, `run` dropped and raised) against the difference equations in plain `i32` Rust; `SetpointReg` with known, large and unknown data.
**Where**: `crates/piperine-parity/tests/buck_control.rs`
**Depends on**: T48
**Reuses**: T48; `crates/piperine-transpile/tests/pid.rs`; buck SPEC §6, §7
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The stimulus module and the Rust reference compute the same `meas` and `run` from the sample index; the test asserts that first
- [ ] Every committed `reference`, `duty`, `integ`, `deriv` and `e_prev` is compared, bit for bit, at every sample
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_control`: `the_ramp_and_the_pid_follow_their_difference_equations`, `the_setpoint_register_takes_known_data_and_saturates`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the ramp, the PID and the setpoint register run from source`

---

### T50: The ADC and the comparator from source

**What**: `Adc` on a held node at five voltages; the stdlib `A2D` on an RC charge and discharge, and with its input high at the operating point.
**Where**: `crates/piperine-parity/tests/buck_boundary.rs`
**Depends on**: T49
**Reuses**: T48; `crates/piperine-parity/tests/boundary.rs`; `ToleranceContract::TRAN_VOLTAGE`; buck SPEC §4.1
**Requirement**: DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The comparator's trip times are judged by the analytic RC voltage at each time against the threshold under `TRAN_VOLTAGE`; no time tolerance is written
- [ ] The third test reads `d` true at time 0 with no change of `d` after it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_boundary`: `the_adc_commits_the_rounded_code_after_tconv`, `the_comparator_trips_at_its_thresholds`, `the_comparator_is_right_at_time_zero`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the ADC and the comparator run from source`

---

### T51: Time and timers from source

**What**: The stdlib `every(t0, period)` placed in an `on` head, a reaction that commits `now()` and `analysis(Static)`, and `rise()`/`fall()` over edges to and from unknown.
**Where**: `crates/piperine-parity/tests/buck_time.rs`
**Depends on**: T50
**Reuses**: T48; T42
**Requirement**: DIG-12, DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The third test commits false, `Unknown`, true, `Unknown`, false and true to one `Bit` and counts the runs of a reaction on `rise()` and of one on `fall()` against §22.1: rise on false to unknown, unknown to true and false to true; fall on true to unknown, unknown to false
- [ ] `every(1u, 2u)` fires at 1, 3, 5, 7 and 9 µs in 10 µs and at no other time
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_time`: `every_from_the_stdlib_fires_once_per_period`, `a_reaction_reads_now_and_static_from_source`, `rise_and_fall_fire_on_edges_to_and_from_unknown`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): every(), now(), analysis(Static) and unknown edges run from source`

---

### T52: Records from source

**What**: The stdlib `Never` on a clock with `bad` held false, then true; `log` from a reaction; a `Sink` decoding with `Body::record_site`.
**Where**: `crates/piperine-parity/tests/buck_records.rs`
**Depends on**: T51
**Reuses**: T48; T44; `piperine_solver::protocol::Sink`
**Requirement**: DIG-11
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] One record per rising edge, each on `checks` with texts `kind = require`, `name = never` and `pass` known
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_records`: `never_reports_each_rising_edge`, `log_carries_its_severity_and_its_text`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): Never and log write records from source`

---

### T53: The gate driver and the power switch from source

**What**: `GateDriver` under scripted commands; `PowerSwitch` between a source and a resistor with its gate committed high at `t0`: the conductance ramp of the hoisted `transition` and its time points.
**Where**: `crates/piperine-parity/tests/buck_power.rs`
**Depends on**: T52
**Reuses**: T48; `crates/piperine-parity/tests/boundary.rs` (the switched RC of `ir-rust` T46)
**Requirement**: DIG-13, DIG-14
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The node voltage is compared with the divider of the analytic conductance at every accepted time from `t0 − tsw` to `t0 + 2 · tsw` under `TRAN_VOLTAGE`
- [ ] Records exist at `t0` and at `t0 + tsw`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_power`: `the_gate_driver_delays_and_interlocks`, `the_power_switch_ramps_its_conductance_in_tsw`, `a_gate_change_makes_two_time_points`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): the gate driver and the power switch run from source`

---

## Phase 10 tasks

### T54: The inductor and the current sense from source

**What**: A switched source into a resistor and `Inductor`; `CurrentSense` at an operating point.
**Where**: `crates/piperine-parity/tests/buck_power.rs` (modify)
**Depends on**: None in this phase (phase 9 complete)
**Reuses**: T53; `ToleranceContract::{TRAN_CURRENT, OP_VOLTAGE}` as `contract.rs` names them
**Requirement**: DIG-13
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The current is read through the 1 Ω series resistor's voltage and compared with `V / (R + dcr) · (1 − exp(−(t − t0)(R + dcr) / l))` at every accepted time after `t0`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_power`: `the_inductor_follows_the_rl_response`, `the_current_sense_scales_the_shunt_voltage`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the inductor and the current sense run from source`

---

### T55: The supervisor from source

**What**: `Supervisor` under an MSL stimulus of 2400 samples (enable, soft-start to the target, eight cut periods, the hiccup count, a retry, two over-voltage samples, power-good in and out of its window) against a plain Rust state machine written from buck SPEC §8.
**Where**: `crates/piperine-parity/tests/buck_supervisor.rs`
**Depends on**: T54
**Reuses**: T49 (the stimulus pattern); buck SPEC §8.1 to §8.6
**Requirement**: DIG-15
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The Rust reference is written from the SPEC table and text, not from `supervisor.msl`; `mode`, `fault`, `run` and `pgood` are compared at every sample
- [ ] The `log` record of the hiccup entry is read once per entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_supervisor`: `the_supervisor_follows_its_state_table`, `power_good_asserts_after_64_and_drops_after_4`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the supervisor follows its state table from source`

---

### T56: The whole buck compiles and idles

**What**: `Buck` as the top through `Project`: compiled with no warning, every shape valid, and 10 switching periods with `en` not driven.
**Where**: `crates/piperine-parity/tests/buck_whole.rs`
**Depends on**: T55
**Reuses**: T2, T38; `crates/piperine-transpile/tests/digital_circuit.rs` (the no-warning check)
**Requirement**: DIG-15
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Sonnet

**Done when**:

- [ ] The second test reads `mode` as 0 (`Off`), `pgood` false, `power.hs_gate` and `power.ls_gate` false through 20 µs, and `vin_ok` true from the first instant (the `init` half of `above` through three levels of hierarchy)
- [ ] A solver defect on this path stops the task and is reported with a hand-written ABI device that reproduces it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_whole`: `the_buck_compiles_with_no_warning_and_valid_shapes`, `a_buck_that_is_not_enabled_stays_off_with_vin_ok_high`, `no_never_record_fails_while_not_enabled`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): the whole buck compiles and idles from source`

---

### T57: The closed loop

**What**: A bench that writes setpoint code 0, raises `en`, and runs 600 µs at Vin 12 V into 6.6 Ω.
**Where**: `crates/piperine-parity/tests/buck_closed_loop.rs`
**Depends on**: T56
**Reuses**: T56; buck SPEC §2, §7.2, §8.6, §10 S1 and S10
**Requirement**: DIG-16
**Needs from ir-rust**: ir-rust T41 to ir-rust T46
**Executor**: Opus

**Done when**:

- [ ] The four tests share one run, made once per test binary
- [ ] The 10 mV bound is the SPEC's own (§2: ±1 % or ±10 mV, whichever is larger), cited at the assertion; the mean is over the accepted points of the last 20 periods, weighted by step
- [ ] A run that does not converge, or a result the SPEC does not explain, stops the task: the defect is reported with the smallest circuit of phases 9 and 10 that shows it. No solver crate is edited and no bound is widened
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test buck_closed_loop`: `the_buck_starts_and_reaches_regulate`, `power_good_is_high_and_no_fault_is_latched`, `the_output_settles_within_10_mv_of_0v8`, `no_shoot_through_during_the_start`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited beyond the replacements the rules allow

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the buck starts and regulates at 0.8 V from source`

---

### T58: GAPS.md after msl-digital

**What**: Root `GAPS.md`: the table of what runs, G3 removed from "Silent", the deviations this feature leaves, and `ROADMAP.md` queue item 2 marked done.
**Where**: `GAPS.md`
**Depends on**: T57
**Reuses**: the file's own ranking; spec DIG-17
**Requirement**: DIG-17
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [ ] "Silent" lists: a derived child param is not re-derived on a host `set`
- [ ] "Loud" lists: unknown control is a fault; every form of DIG-07 AC 1; enum variant names are not carried
- [ ] `ROADMAP.md` "MSL spec gaps" drops G3 and "Queue after ir-rust" marks item 2 delivered; item 5 is the orchestrator's to reword and is not touched
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`

**Tests**: none
**Gate**: build

**Commit**: `docs: GAPS.md and ROADMAP.md after msl-digital`

---

### T59: The buck's own README and GAPS

**What**: `examples/buck/README.md` status: what runs and how (`cargo test -p piperine-parity --test buck_whole`); `examples/buck/GAPS.md` G3 rewritten as closed (reported by name).
**Where**: `examples/buck/README.md`
**Depends on**: T58
**Reuses**: T58
**Requirement**: DIG-17
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [ ] No `.msl` file of `examples/buck/src` is edited by this feature: `git diff --stat main -- examples/buck/src` is empty
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`

**Tests**: none
**Gate**: build

**Commit**: `docs(examples): the buck runs from source`

---

### T60: MD-64 and the instruction files

**What**: `.specs/STATE.md` MD-64 (design "Tech Decisions"); the crate rows of `CLAUDE.md` and `AGENTS.md` for `piperine-lang` (types and elab cover digital and mixed forms).
**Where**: `.specs/STATE.md`
**Depends on**: T59
**Reuses**: MD-63 of `ir-rust`
**Requirement**: DIG-17
**Needs from ir-rust**: none
**Executor**: Sonnet

**Done when**:

- [ ] `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` and `cargo test -p piperine-lang --test suite_hygiene` are green
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-lang`, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): MD-64 - hoisted result modules, elaboration-time unknown-ness, record text in the IR`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`; a criterion with several observable halves has one per half, each naming its half. A test function has exactly the name its check cites.

**C1** - `UInt<N>`, `Int<N>`, `uN`, `iN` for `N` in 1..=64 and their `const` items have types. (DIG-01, AC 1; T3)
Proof: run tint | test integer_types_and_constants_have_types ... ok

**C2** - An integer literal takes the type expected at each of the six positions, and `i32` with none. (DIG-01, AC 2; T3)
Proof: run tint | test a_literal_takes_the_expected_integer_type_else_i32 ... ok

**C3** - `let x: u8 = 256;` is `E0909`. (DIG-01, AC 3; T3)
Proof: run tint | test a_literal_out_of_range_is_e0909 ... ok

**C4** - `+ - * / %`, unary `-`, `& | ^ !` and the six comparisons type on two operands of one integer type. (DIG-01, AC 4; T4)
Proof: run tint | test integer_operators_take_one_type ... ok

**C5** - A `u9` plus a `u12` is `P0002`. (DIG-01, AC 5; T4)
Proof: run tint | test mixed_integer_operands_are_p0002 ... ok

**C6** - `(x: u10) << (s: i32)` has type `u10`. (DIG-01, AC 6; T4)
Proof: run tint | test a_shift_takes_any_integer_amount ... ok

**C7** - `as` types between any two of the integer types and `f64`. (DIG-01, AC 7; T4)
Proof: run tint | test as_goes_between_integers_and_f64 ... ok

**C8** - `min`, `max`, `clamp` on integers and `f64`, and `floor` on `f64`, give the receiver's type. (DIG-01, AC 8; T5)
Proof: run tint | test min_max_clamp_and_floor_are_typed ... ok

**C9** - `u128`, `f32`, `f16`, `Float<16>` and `Float<128>` are `P0001` naming the type, at the type as written. (DIG-01, AC 9; T3)
Proof: run tint | test an_unsupported_width_or_float_is_named ... ok

**C10** - `UInt<0>` is `E0103`. (DIG-01, AC 10; T3)
Proof: run tint | test a_width_below_one_is_e0103 ... ok

**C11** - `d.min(SETPOINT_MAX)` with `d` a `Bit<u8>` carrier has type `u8`. (DIG-02, AC 1; T9)
Proof: run tcar | test a_catalogue_method_on_a_carrier_is_typed_on_its_value ... ok

**C12** - `oc_count.saturating_add(1)` on a `u4` is `P0001` naming `saturating_add` and `u4`. (DIG-02, AC 2; T5)
Proof: run tint | test an_uncatalogued_method_is_p0001_by_name ... ok

**C13** - Every call and method call of an accepted model has a callee, a catalogue row or a built-in. (DIG-02, AC 3; T5)
Proof: run tint | test every_accepted_call_has_a_callee ... ok

**C14** - A unit enum deriving `Repr`, its variant paths and `==`/`!=` are typed. (DIG-03, AC 1; T6)
Proof: run tcomp | test unit_enums_and_their_variants_are_typed ... ok

**C15** - A payload variant, an explicit discriminant and `#[repr(u2)]` are each `P0001` naming the form. (DIG-03, AC 2; T6)
Proof: run tcomp | test an_enum_outside_the_subset_is_p0001 ... ok

**C16** - `Option<T>`, `Some(e)`, `None`, `is_some`, `is_none` and `unwrap_or` are typed. (DIG-03, AC 3; T7)
Proof: run tcomp | test option_values_and_their_methods_are_typed ... ok

**C17** - `match` types over an enum, a `bool`, an integer and an `Option` with the six pattern forms. (DIG-03, AC 4; T8)
Proof: run tcomp | test match_is_typed_over_enums_bools_integers_and_options ... ok

**C18** - A `match` on `Mode` without `Hiccup` and without `_` is `P0002`. (DIG-03, AC 5; T8)
Proof: run tcomp | test a_match_that_is_not_exhaustive_is_p0002 ... ok

**C19** - `if let Some(d) = data.known()` binds `d: u8` in the first block. (DIG-03, AC 6; T8)
Proof: run tcomp | test if_let_some_binds_the_payload ... ok

**C20** - `let (m, r) = (a as u11, b as u11);` names `m` and `r` as `u11` in a function, a reaction and a module body. (DIG-03, AC 7; T7)
Proof: run tcomp | test a_tuple_let_names_each_component ... ok

**C21** - A `signal`, an `in` port and an `out` port of `Bit<T>` type for `T` a `bool`, an integer, `f64` and a unit enum. (DIG-04, AC 1; T9)
Proof: run tcar | test carriers_of_bit_are_typed_by_their_value ... ok

**C22** - `signal s: Bit<&str>` is `E0112`. (DIG-04, AC 2; T9)
Proof: run tcar | test a_carried_type_without_repr_is_e0112 ... ok

**C23** - A read of a `Bit<u9>` carrier in a definition, a guard and a reaction has type `u9`. (DIG-04, AC 3; T9)
Proof: run tcar | test a_carrier_read_has_its_value_type ... ok

**C24** - `known()` is `Option<T>`, `known_or(d)` is `T`, `is_known()` and `high()` are `bool`, on a carrier and on a value. (DIG-04, AC 4; T9)
Proof: run tcar | test the_flag_reads_are_typed ... ok

**C25** - An initial value of another type, or one that reads a carrier or a probe, is `E0210`. (DIG-04, AC 5; T9)
Proof: run tcar | test an_initial_value_of_the_wrong_kind_is_e0210 ... ok

**C26** - A `u8` expression defining a `Bit<u10>` carrier is `E0337`. (DIG-04, AC 6; T10)
Proof: run tcar | test a_definition_of_another_type_is_e0337 ... ok

**C27** - A definition or its delay that reads a probe, `now()` or `analysis(Static)`, directly or through a `let`, is `E0336`. (DIG-04, AC 7; T10)
Proof: run tcar | test a_continuous_definition_is_e0336 ... ok

**C28** - An expression that reads a probe, connected to an `in` port, is `E0316`. (DIG-04, AC 7; T11)
Proof: run tcar | test a_continuous_expression_at_an_in_port_is_e0316 ... ok

**C29** - A definition of an `in` port, of a child's carrier and of a node are each `E0222`. (DIG-04, AC 8; T10)
Proof: run tcar | test a_target_the_module_may_not_drive_is_e0222 ... ok

**C30** - Two definitions of one carrier in one module are `E0335`. (DIG-04, AC 9; T10)
Proof: run tcar | test two_definitions_of_one_carrier_are_e0335 ... ok

**C31** - A definition and a commit of one carrier in one module are `E0221`. (DIG-04, AC 10; T13)
Proof: run tev | test a_definition_and_a_commit_are_e0221 ... ok

**C32** - A carrier at two `out` ports, or at one while its module drives it, is `E0220`. (DIG-04, AC 11; T11)
Proof: run tcar | test a_second_drive_is_e0220 ... ok

**C33** - `signal up: Bit = false;` connected to an `out` port is `E0223`. (DIG-04, AC 12; T11)
Proof: run tcar | test an_initial_value_without_the_drive_is_e0223 ... ok

**C34** - `out y: Bit = false` with `y = a & b;` is `E0338`. (DIG-04, AC 13; T10)
Proof: run tcar | test an_initial_value_under_an_undelayed_definition_is_e0338 ... ok

**C35** - An expression, or an `in` port of the parent, connected to an `out` port is `E0314`. (DIG-04, AC 14; T11)
Proof: run tcar | test an_out_port_takes_only_a_carrier_the_parent_hands_over ... ok

**C36** - A `Bit<u8>` carrier connected to a `Bit<u10>` port is `E0251`. (DIG-04, AC 15; T11)
Proof: run tcar | test a_carrier_of_another_value_type_is_e0251 ... ok

**C37** - `a = !b; b = !a;` is `E0339`. (DIG-04, AC 16; T10)
Proof: run tcar | test a_loop_of_definitions_is_e0339 ... ok

**C38** - `changed`, `rise`, `fall`, `cross(e)`, `at(t)`, `a || b` and `ev && guard` have type `Event`. (DIG-05, AC 1; T12)
Proof: run tev | test event_expressions_are_typed ... ok

**C39** - `on clk { … }` with `clk: Bit` is `E0430` at `clk`. (DIG-05, AC 2; T12)
Proof: run tev | test a_bool_head_is_e0430 ... ok

**C40** - `!clk.rise()` as a head, and `en && clk.rise()`, are each `E0440`. (DIG-05, AC 3; T12)
Proof: run tev | test a_misplaced_not_or_and_is_e0440 ... ok

**C41** - An `Event` in a commit and in a definition is `E0441`. (DIG-05, AC 4; T12)
Proof: run tev | test an_event_outside_an_event_position_is_e0441 ... ok

**C42** - `at(now() + 1n)` and `at(v(p))` are each `E0442`. (DIG-05, AC 5; T12)
Proof: run tev | test a_continuous_at_is_e0442 ... ok

**C43** - `cross(e)` in a reaction body and in an `analog` block is `E0443`. (DIG-05, AC 6; T12)
Proof: run tev | test cross_or_at_outside_an_event_position_is_e0443 ... ok

**C44** - A reaction body with `let`, `if`, `match`, calls, commits with and without `after`, a probe and `now()` type-checks. (DIG-05, AC 7; T13)
Proof: run tev | test a_reaction_body_is_typed ... ok

**C45** - `count <- true;` with `count: Bit<u9>` is `E0432`. (DIG-05, AC 8; T13)
Proof: run tev | test a_commit_of_another_type_is_e0432 ... ok

**C46** - `q <- d after 5;` is `E0433`. (DIG-05, AC 9; T13)
Proof: run tev | test a_delay_that_is_not_f64_is_e0433 ... ok

**C47** - A module placed in a reaction body, in a `fn` and in a closure is each `E0350`. (DIG-05, AC 10; T13)
Proof: run tev | test a_placement_in_a_reaction_is_e0350 ... ok

**C48** - `emit!("conv", code = q, ok = true)` in a reaction type-checks. (DIG-05, AC 11; T14)
Proof: run tev | test emit_is_typed_in_a_reaction ... ok

**C49** - A repeated field name and a field named `maybe` are each `E0604`. (DIG-05, AC 12; T14)
Proof: run tev | test a_repeated_or_reserved_field_is_e0604 ... ok

**C50** - `pass = 1.0` on `checks`, and one field with two types on one stream, are each `E0605`. (DIG-05, AC 13; T14)
Proof: run tev | test a_field_type_conflict_is_e0605 ... ok

**C51** - `require(…)` reached from a definition, a module-body `let` and a `const` is each `E0601`. (DIG-05, AC 14; T14)
Proof: run tev | test emit_where_it_is_not_allowed_is_e0601 ... ok

**C52** - `require(…)` in an `analog` block is `E0603`. (DIG-05, AC 15; T14)
Proof: run tev | test a_check_reached_from_analog_is_e0603 ... ok

**C53** - A module with value inputs and `-> f64` types; a result of another type is `E0301`. (DIG-06, AC 1; T15)
Proof: run tres | test a_module_with_a_result_is_typed ... ok

**C54** - A placement in a body `let`, an `analog` block, an event position and an argument of another placement has the result's type. (DIG-06, AC 2; T15)
Proof: run tres | test a_placement_has_its_result_type ... ok

**C55** - `transition(x)` and `transition(x, 0.0, tr, tr)` are accepted; `transition()` and five arguments are `E0141`. (DIG-06, AC 3; T15)
Proof: run tres | test trailing_option_inputs_may_be_left_out ... ok

**C56** - A placement in a param default and in a bound param value is `E0352`. (DIG-06, AC 4; T15)
Proof: run tres | test a_placement_in_a_const_context_is_e0352 ... ok

**C57** - `now()` is `f64` and `analysis(Static)` is `bool` in a body, `analog` and a reaction; each is `E0323` in a param default. (DIG-06, AC 5; T15)
Proof: run tres | test now_and_analysis_static_are_typed ... ok

**C58** - `analog` reading a carrier bare, through a `let` and through a value input is each `E0150` naming the carrier. (DIG-06, AC 6; T15)
Proof: run tres | test an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150 ... ok

**C59** - `let a = a + 1.0;` is `E0332`; `let y = twice(y);` with `module twice(x: f64) -> f64 { 2.0 * x }` is `E0353`. (DIG-06, AC 7; T15)
Proof: run tres | test a_let_that_reads_itself_is_e0332_or_e0353 ... ok

**C60** - An `f64` argument for an `Option<f64>` parameter is accepted as `Some` of it. (DIG-06, AC 8; T7)
Proof: run tcomp | test a_value_for_an_option_parameter_is_some_of_it ... ok

**C61** - Each construct of DIG-07 AC 1 is `P0001` naming it, one negative source each. (DIG-07, AC 1; T16)
Proof: run tbuck | test what_stays_outside_is_p0001_by_name ... ok

**C62** - The seven M1 twins elaborate to their unedited golden texts. (DIG-07, AC 2; T36)
Proof: run edig | test the_m1_goldens_are_unchanged ... ok

**C63** - `Types::check` of `Buck` from `examples/buck` gives zero diagnostics. (DIG-07, AC 3; T16)
Proof: run tbuck | test the_buck_type_checks_with_no_diagnostic ... ok

**C64** - Integer types, literals, operators, casts and methods print as `UInt`, `Int`, integer literals, `BinaryOp`, `Cast` and `Method` nodes. (DIG-08, AC 1; T23)
Proof: run evalues | test integers_lower_to_typed_ir ... ok

**C65** - `N_PERIOD - 1` with `N_PERIOD: u9 = 400` is the literal 399 of `u9`. (DIG-08, AC 2; T23)
Proof: run evalues | test integer_constants_fold ... ok

**C66** - `300.0 as u8` folds to 255, `1u8 << 9` to 0 and `511u9 + 1` to 0. (DIG-08, AC 3; T23)
Proof: run evalues | test folded_casts_shifts_and_sums_wrap_as_compiled_code_does ... ok

**C67** - `const K: u8 = 1 / 0;` is `E0104`. (DIG-08, AC 4; T23)
Proof: run evalues | test a_constant_division_by_zero_is_e0104 ... ok

**C68** - A carrier read, `Unknown` and `count + 1` are `Flagged`; `known_or`, `is_known` and `high` are not. (DIG-08, AC 5; T24)
Proof: run evalues | test a_carrier_read_is_flagged_and_the_flag_reads_are_not ... ok

**C69** - `hs && oc.known_or(true)` with `hs` a carrier is `BitAnd`; `a && b` over two params is `And`. (DIG-08, AC 6; T24)
Proof: run evalues | test a_short_circuit_over_a_flagged_operand_becomes_bitwise ... ok

**C70** - `c && (x / n > 1)` with `c` a carrier and integer `/` is `P0001`. (DIG-08, AC 7; T24)
Proof: run evalues | test a_partial_right_operand_under_a_flagged_short_circuit_is_p0001 ... ok

**C71** - `dithered_width(duty.known_or(0), frame + 1)` lowers `dithered_width~01` and `rev3~1`; an all-known call keeps the source name. (DIG-08, AC 8; T25)
Proof: run evalues | test a_function_gets_one_variant_per_flag_pattern ... ok

**C72** - `Mode` is `UInt(2)`, `Mode::Hiccup` is 3, and a `match` is an `if` chain that reads its scrutinee once. (DIG-08, AC 9; T26)
Proof: run evalues | test an_enum_is_its_tag_and_a_match_is_an_if_chain ... ok

**C73** - An `Option` value is a `bool` and a payload through all eight forms, and no IR type is an `Option`. (DIG-08, AC 10; T27)
Proof: run evalues | test an_option_is_a_flag_and_a_payload ... ok

**C74** - `let (m, r) = (x, y);` is two locals. (DIG-08, AC 11; T27)
Proof: run evalues | test a_tuple_let_is_one_local_per_component ... ok

**C75** - `RefRamp` has the carriers `tick`, `run`, `target`, `reference`, `at_target` in that order, `reference` with initial value 0. (DIG-09, AC 1; T28)
Proof: run edig | test carriers_follow_ports_then_signals ... ok

**C76** - `hs_gate = (hs_cmd & !ls_cmd) after tpd;` is one drive with its `after`. (DIG-09, AC 2; T29)
Proof: run edig | test a_definition_is_a_drive ... ok

**C77** - `RefRamp(done, run, target_codes(setpoint.known_or(0)), …)` adds the carrier `ramp.target` and its drive to `Controller`. (DIG-09, AC 3; T29)
Proof: run edig | test an_expression_at_an_in_port_is_a_carrier_and_a_drive ... ok

**C78** - Each signal carrier of each instance is one net named by its path, and each carrier port is bound to a net of the parent. (DIG-09, AC 4; T28)
Proof: run edig | test every_signal_carrier_is_a_design_net ... ok

**C79** - A top with ports `en` and `pgood` has the nets `Top.en` and `Top.pgood`; a continuous port on the top is `P0001`. (DIG-09, AC 5; T28)
Proof: run edig | test a_top_with_discrete_ports_gets_one_net_per_port ... ok

**C80** - `changed`, `rise`, `fall`, `cross(e)` and `prev()` lower to `Changed`, `Changed` with the two-condition guards of §22.1, `Cross`, and `Previous`. (DIG-09, AC 6; T30)
Proof: run edig | test changed_rise_fall_and_cross_lower_to_events ... ok

**C81** - `on x.changed() && en.high() && analysis(Static)` keeps `en.high()` on the event and wraps the body in `if static`. (DIG-09, AC 7; T30)
Proof: run edig | test a_guard_splits_into_its_discrete_and_continuous_parts ... ok

**C82** - An `||` head is one reaction on two events, or two reactions in source order when a guard reads a probe. (DIG-09, AC 8; T30)
Proof: run edig | test an_or_head_is_one_reaction_or_one_per_disjunct ... ok

**C83** - A per-disjunct head whose body emits, and an event read as a `bool` in a body, are each `P0001`. (DIG-09, AC 9; T30)
Proof: run edig | test a_head_the_ir_cannot_hold_is_p0001 ... ok

**C84** - The reaction of `Adc` is one `let` and three commits with their `after`, in source order, the `let` holding a probe of `ain`. (DIG-09, AC 10; T31)
Proof: run edig | test a_reaction_body_lowers_in_source_order ... ok

**C85** - Two `Divider` placements with different values give their `Resistor` instances those values; a false child `where` is `E0317` at the placement. (DIG-09, AC 11; T32)
Proof: run edig | test a_child_param_reads_the_parents_value ... ok

**C86** - An authored child whose param reads the parent prints the expression (the IR half of DIG-09 AC 11). (DIG-09, AC 11; T21)
Proof: run iramend | test a_child_param_may_read_the_parents_params ... ok

**C87** - Two elaborations of one design print equal. (DIG-09, AC 12; T36)
Proof: run edig | test elaborating_twice_gives_the_same_print ... ok

**C88** - `Design::dump()` is equal before and after elaboration. (DIG-09, AC 13; T36)
Proof: run edig | test the_design_is_not_written ... ok

**C89** - Every `Ir` returned has an empty `check()`. (DIG-09, AC 14; T36)
Proof: run edig | test every_ir_passes_check ... ok

**C90** - `PowerSwitch` holds the six `gate.*` carriers; `A2D` holds `above$0.init$0.up` and `above$1.init$0.up`. (DIG-10, AC 1; T33)
Proof: run eres | test a_placement_is_hoisted_under_its_name ... ok

**C91** - A read of `gate` in `analog` is `transition`'s result expression with `x` replaced by the hoisted carrier and `tr` by `tsw`. (DIG-10, AC 2; T33)
Proof: run eres | test value_inputs_are_substituted_at_each_read ... ok

**C92** - `transition(x)` lowers `td.unwrap_or(0.0)` as the literal `0.0`. (DIG-10, AC 3; T33)
Proof: run eres | test an_omitted_option_input_reads_as_none ... ok

**C93** - `Load` holds `k_step.*` and `k_inject.*`, two sets of six carriers. (DIG-10, AC 4; T33)
Proof: run eres | test two_placements_are_two_sets_of_carriers ... ok

**C94** - An IR elaborated from `PowerSwitch` has no body and no instance named `transition`. (DIG-10, AC 5; T33)
Proof: run eres | test a_result_module_gets_no_body_and_no_instance ... ok

**C95** - `transition(if g.high() { 1.0 } else { 0.0 }, …)` is a drive of `gate.x` by that expression. (DIG-10, AC 6; T33)
Proof: run eres | test an_expression_at_a_discrete_input_drives_the_hoisted_carrier ... ok

**C96** - `at(t0 + dt)` is an `At` event, `now()` the time read, `analysis(Static)` the static read. (DIG-10, AC 7; T33)
Proof: run eres | test at_now_and_static_lower_to_their_nodes ... ok

**C97** - The time and static reads print, and `check()` refuses them in a drive, a guard and an `At` argument (the IR half of DIG-10 AC 7). (DIG-10, AC 7; T17)
Proof: run iramend | test time_and_static_reads_print_and_check ... ok

**C98** - An `At` event prints, and `check()` refuses a probe in its argument (the IR half of DIG-10 AC 7). (DIG-10, AC 7; T18)
Proof: run iramend | test an_at_event_prints_and_checks ... ok

**C99** - `emit!("checks", kind = "require", name = "never", pass = p)` is one emit site with two texts and one value field. (DIG-11, AC 1; T34)
Proof: run eres | test emit_lowers_to_a_site_with_its_texts ... ok

**C100** - An emit site prints, and `check()` refuses one outside a reaction (the IR half of DIG-11 AC 1). (DIG-11, AC 1; T19)
Proof: run iramend | test an_emit_site_prints_and_checks ... ok

**C101** - `Body::record_site` gives the stream and the texts of a site by reaction and statement (DIG-11 AC 5, the site half). (DIG-11, AC 5; T19)
Proof: run iramend | test a_record_site_gives_its_stream_and_texts ... ok

**C102** - A reaction calling `require` has the emit inline and the IR has no function `require`. (DIG-11, AC 2; T34)
Proof: run eres | test a_function_that_emits_is_inlined ... ok

**C103** - `log(sev, msg)` with `msg` a function parameter that is not a literal at the call is `P0001`. (DIG-11, AC 3; T34)
Proof: run eres | test a_text_field_that_is_not_a_literal_is_p0001 ... ok

**C104** - `Never` gives one `checks` record per rising edge with `pass` = `!bad`, known. (DIG-11, AC 4; T52)
Proof: run brecords | test never_reports_each_rising_edge ... ok

**C105** - An emit adds one record with its bytes and its instant, and no microstep (the compiled half of DIG-11 AC 4). (DIG-11, AC 4; T44)
Proof: run amended | test an_emit_commits_one_record_and_adds_no_microstep ... ok

**C106** - A stream is one `RECORD` pin with one flagged field per value field (the shape half of DIG-11 AC 4). (DIG-11, AC 4; T43)
Proof: run planamend | test a_stream_is_a_record_pin_with_one_field_per_value ... ok

**C107** - `log(Severity::Warning, "x")` gives one `log` record with `severity` 1 and the site text `x`. (DIG-11, AC 5; T52)
Proof: run brecords | test log_carries_its_severity_and_its_text ... ok

**C108** - `at(t0 + dt)` with 10 ns and 5 ns fires once, at 15 ns. (DIG-12, AC 1; T42)
Proof: run amended | test at_fires_once_at_its_time ... ok

**C109** - An `At` event is a private event pin, an arming reaction on `START` and its carriers' changes, and one `REPLACE` statement (the shape half of DIG-12 AC 1). (DIG-12, AC 1; T41)
Proof: run planamend | test an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement ... ok

**C110** - The stdlib `every(1u, 2u)` fires at 1, 3, 5, 7 and 9 µs in 10 µs (DIG-12 AC 1, from source). (DIG-12, AC 1; T51)
Proof: run btime | test every_from_the_stdlib_fires_once_per_period ... ok

**C111** - After `dt <- 20n` at 12 ns there is no firing at 15 ns and one at 30 ns. (DIG-12, AC 2; T42)
Proof: run amended | test a_rearmed_at_drops_the_earlier_firing ... ok

**C112** - A past time and an unknown time do not fire; a time equal to now fires at the next microstep. (DIG-12, AC 3; T42)
Proof: run amended | test a_past_or_unknown_time_does_not_fire ... ok

**C113** - `now()` is 15e-9 in a reaction fired at 15 ns and the solve's time in `analog`. (DIG-12, AC 4; T40)
Proof: run amended | test now_is_the_time_of_the_instant_and_of_the_solve ... ok

**C114** - `TIME` and `STATIC` are ambient reads of `terms` and of reactions, with no partial (the shape half of DIG-12 AC 4). (DIG-12, AC 4; T39)
Proof: run planamend | test time_and_static_are_ambient_reads ... ok

**C115** - A reaction elaborated from source commits `now()` and `analysis(Static)` with those values (DIG-12 AC 4, from source). (DIG-12, AC 4; T51)
Proof: run btime | test a_reaction_reads_now_and_static_from_source ... ok

**C116** - `analysis(Static)` is true at `(0, 0)` and false at a transient instant. (DIG-12, AC 5; T40)
Proof: run amended | test static_is_true_only_in_the_equilibrium ... ok

**C117** - `Inductor` lowers `i(p, n)` as a flow read of its potential branch. (DIG-13, AC 1; T35)
Proof: run emix | test a_flow_probe_of_a_potential_branch_lowers ... ok

**C118** - A flow read and a `Ddt` under `Potential` print and pass `check()` (the IR half of DIG-13 AC 1 and 2). (DIG-13, AC 1; T20)
Proof: run iramend | test a_flow_read_and_a_potential_ddt_print_and_check ... ok

**C119** - `v(p, n) +> ddt(l * i(p, n)) + dcr * i(p, n)` is a potential contribution with a `Ddt` addend and a direct one. (DIG-13, AC 2; T35)
Proof: run emix | test a_potential_contribution_keeps_its_ddt ... ok

**C120** - A flow read is the branch's flow pin and the `Ddt` addend a `DDT` term on its equation row (the shape half of DIG-13 AC 2). (DIG-13, AC 2; T45)
Proof: run planamend | test a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term ... ok

**C121** - `i(a, b)` of a branch with only a flow contribution is `P0001`. (DIG-13, AC 3; T35)
Proof: run emix | test a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001 ... ok

**C122** - The RL current follows `V / (R + dcr) · (1 − exp(−(t − t0)(R + dcr) / l))` under the transient contract. (DIG-13, AC 4; T54)
Proof: run bpower | test the_inductor_follows_the_rl_response ... ok

**C123** - Every function of the compiled inductor and VCVS bodies passes `verify_partials` at `rel_tol` 1e-6 (the compiled half of DIG-13 AC 4). (DIG-13, AC 4; T46)
Proof: run amended | test the_inductor_functions_pass_verify_partials ... ok

**C124** - `v(pos)` is `gain · v(a, b)` and `v(neg)` its negative at the operating point. (DIG-13, AC 5; T54)
Proof: run bpower | test the_current_sense_scales_the_shunt_voltage ... ok

**C125** - The switch conductance is `1 / roff` before `t0`, linear in `k` through `tsw`, and `1 / ron` after. (DIG-13, AC 6; T53)
Proof: run bpower | test the_power_switch_ramps_its_conductance_in_tsw ... ok

**C126** - The `analog` block of `PowerSwitch` reads the hoisted result of `gate` (the elaboration half of DIG-13 AC 6). (DIG-13, AC 6; T35)
Proof: run emix | test analog_reads_a_hoisted_result ... ok

**C127** - Records exist at `t0` and at `t0 + tsw`. (DIG-13, AC 7; T53)
Proof: run bpower | test a_gate_change_makes_two_time_points ... ok

**C128** - `Osc`, `SetpointReg`, `RefRamp`, `Pid`, `GateDriver` and `Adc` from source print as their T29 fixtures, the guard of each `rise` event apart. (DIG-14, AC 1; T37)
Proof: run ebuck | test six_slice_bodies_equal_their_hand_built_fixtures ... ok

**C129** - `Dpwm` and `A2D` from source print as their fixtures plus only the listed items, each present. (DIG-14, AC 2; T37)
Proof: run ebuck | test dpwm_and_a2d_differ_only_as_listed ... ok

**C130** - `clk` changes at `k × 2.5 ns`, `k` = 1 to 40, and at no other time in 100 ns. (DIG-14, AC 3; T48)
Proof: run bdpwm | test the_oscillator_toggles_every_half_period ... ok

**C131** - Duty word 1325: `hs` is 166 counts in five periods of eight and 165 in three; `ls` runs from 4 counts after `hs` to 396; never both. (DIG-14, AC 4; T48)
Proof: run bdpwm | test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok

**C132** - 200 samples of `reference`, `duty`, `integ`, `deriv` and `e_prev` equal the difference equations, bit for bit. (DIG-14, AC 5; T49)
Proof: run bcontrol | test the_ramp_and_the_pid_follow_their_difference_equations ... ok

**C133** - `code` is `floor(V / vref × 1024 + 0.5)` clamped to 0..=1023 after `tconv`, at five voltages, with the `done` strobe. (DIG-14, AC 6; T50)
Proof: run bboundary | test the_adc_commits_the_rounded_code_after_tconv ... ok

**C134** - Two runs of the oscillator into the DPWM give equal sequences of pin changes. (DIG-14, AC 7; T48)
Proof: run bdpwm | test two_runs_give_the_same_changes ... ok

**C135** - `d` goes true where the RC voltage is `vth + hyst / 2` and false where it is `vth − hyst / 2`. (DIG-14, AC 8; T50)
Proof: run bboundary | test the_comparator_trips_at_its_thresholds ... ok

**C136** - With the input above `vth + hyst / 2` at the operating point, `d` is true at time 0. (DIG-14, AC 9; T50)
Proof: run bboundary | test the_comparator_is_right_at_time_zero ... ok

**C137** - A rising `wr` commits 100 for data 100, 168 for data 200, and nothing for unknown data. (DIG-14, AC 10; T49)
Proof: run bcontrol | test the_setpoint_register_takes_known_data_and_saturates ... ok

**C138** - `a <- b; b <- a;` from source exchanges the two values at the next microstep. (DIG-14, AC 11; T47)
Proof: run bcommits | test two_commits_exchange_two_carriers ... ok

**C139** - Two statements of one reaction to one carrier: the later one is the value. (DIG-14, AC 11; T47)
Proof: run bcommits | test the_later_statement_wins ... ok

**C140** - `p <- true after t; p <- false after t + w;` holds `p` true from `t` to `t + w`. (DIG-14, AC 11; T47)
Proof: run bcommits | test two_statements_make_a_pulse ... ok

**C141** - A commit of the value a carrier holds fires no `changed`. (DIG-14, AC 11; T47)
Proof: run bcommits | test an_unchanged_commit_fires_nothing ... ok

**C142** - `hs_gate` rises `tpd` after `hs_cmd` with `ls_cmd` low; with both commands high both gates are false. (DIG-14, AC 12; T53)
Proof: run bpower | test the_gate_driver_delays_and_interlocks ... ok

**C143** - `Buck` elaborates with zero diagnostics and an empty `check()`. (DIG-15, AC 1; T38)
Proof: run ebuck | test the_buck_elaborates_with_no_diagnostic_and_a_sound_ir ... ok

**C144** - The structural print of `Buck` is the text of `tests/fixtures/elab/buck.txt`. (DIG-15, AC 2; T38)
Proof: run ebuck | test the_buck_ir_is_pinned ... ok

**C145** - `Buck` compiles with zero `rustc` warnings and every shape passes `piperine_abi::validate`. (DIG-15, AC 3; T56)
Proof: run bwhole | test the_buck_compiles_with_no_warning_and_valid_shapes ... ok

**C146** - A two-crate project loads and runs on the compiled backend, and a diagnostic comes back with its stage (the loader DIG-15 AC 3 stands on). (DIG-15, AC 3; T2)
Proof: run project | test a_project_with_a_dependency_crate_loads_and_runs ... ok

**C147** - 2400 samples of `mode`, `fault` and `run` equal the state machine of buck SPEC §8. (DIG-15, AC 4; T55)
Proof: run bsupervisor | test the_supervisor_follows_its_state_table ... ok

**C148** - `pgood` rises at the 64th in-window sample and falls at the 4th out-of-window sample. (DIG-15, AC 5; T55)
Proof: run bsupervisor | test power_good_asserts_after_64_and_drops_after_4 ... ok

**C149** - With `en` not driven: `mode` 0, `pgood` false, both gates false for 20 µs, `vin_ok` true from the first instant. (DIG-15, AC 6; T56)
Proof: run bwhole | test a_buck_that_is_not_enabled_stays_off_with_vin_ok_high ... ok

**C150** - With `en` not driven no `checks` record has `pass` false. (DIG-15, AC 7; T56)
Proof: run bwhole | test no_never_record_fails_while_not_enabled ... ok

**C151** - `mode` goes 0, 1, 2 and `ctl.reference` reaches 128. (DIG-16, AC 1; T57)
Proof: run bloop | test the_buck_starts_and_reaches_regulate ... ok

**C152** - At 600 µs `pgood` is true and `fault` is 0. (DIG-16, AC 2; T57)
Proof: run bloop | test power_good_is_high_and_no_fault_is_latched ... ok

**C153** - The mean of `vout` over the last 20 periods is within 10 mV of 0.8 V. (DIG-16, AC 3; T57)
Proof: run bloop | test the_output_settles_within_10_mv_of_0v8 ... ok

**C154** - No `checks` record has `pass` false during the start. (DIG-16, AC 4; T57)
Proof: run bloop | test no_shoot_through_during_the_start ... ok

**C155** - `GAPS.md` lists the three deviations this feature leaves. (DIG-17, AC 1; T58)
Proof: `grep -q 'unknown control' GAPS.md && grep -q 're-derived' GAPS.md && grep -q 'variant names' GAPS.md`

**C156** - `GAPS.md` no longer lists G3 as silent. (DIG-17, AC 2; T58)
Proof: `! grep -q 'stays `Unresolved` with no diagnostic' GAPS.md`

**C157** - The buck's README no longer says nothing executes MSL. (DIG-17, AC 2; T59)
Proof: `! grep -q 'Nothing executes MSL yet' examples/buck/README.md`

**C158** - `c.prev()` on a `Bit<u9>` carrier has type `u9`; on a node it is `E0230`. (DIG-04, AC 17; T9)
Proof: run tcar | test prev_is_typed_on_a_carrier ... ok

**C159** - A previous-value read prints as `Flagged` and `check()` refuses one of a carrier the body lacks (the IR half of DIG-09 AC 6). (DIG-09, AC 6; T61)
Proof: run iramend | test a_previous_value_read_prints_and_checks ... ok

**C160** - A `rise` event function reads `VALUE`, `PREVIOUS` and `CHANGED` of its carrier (the shape half of DIG-09 AC 6). (DIG-09, AC 6; T62)
Proof: run planamend | test a_previous_value_is_the_previous_part_of_the_pin ... ok

**C161** - `prev` is unknown after a carrier's first commit and 3 after 3 then 5 (the compiled half of DIG-09 AC 6; §5.3.8). (DIG-09, AC 6; T63)
Proof: run amended | test a_previous_value_is_read_in_a_guard_and_in_a_reaction ... ok

**C162** - `rise()` runs on false to unknown, unknown to true and false to true, and on no other edge; `fall()` on true to unknown, unknown to false and true to false. (DIG-14, AC 13; T51)
Proof: run btime | test rise_and_fall_fire_on_edges_to_and_from_unknown ... ok

**C163** - A module with only a `digital` block has a body with no analog statements. (edge case 1; T31)
Proof: run edig | test a_module_with_only_a_digital_block_has_no_analog_statements ... ok

**C164** - `signal en: Bit;` with no drive is a carrier with no initial value. (edge case 2; T28)
Proof: run edig | test a_carrier_with_no_value_and_no_drive_is_kept ... ok

**C165** - `Bit<One>` with `enum One { A }` is `P0001`. (edge case 3; T9)
Proof: run tcar | test a_single_variant_enum_cannot_be_carried ... ok

**C166** - `PowerSwitch` placed twice gives one body with one set of `gate.*` carriers. (edge case 4; T33)
Proof: run eres | test a_result_module_is_hoisted_once_per_body ... ok

**C167** - A param block on a result placement binds the hoisted items' params as constants. (edge case 5; T33)
Proof: run eres | test a_param_block_binds_the_hoisted_params ... ok

**C168** - A body holding a time or static read, an `At` event, an emit site, a flow read or a previous-value read is `Rejected` by the interpreter, naming the body. (edge case 6; T22)
Proof: run frozen | test a_body_with_an_amended_form_is_rejected_by_name ... ok

**C169** - Every M1 and T29 golden print of `piperine-ir` is byte-identical after the amendments. (DIG-07, AC 2; T21)
Proof: run iramend | test the_earlier_prints_are_unchanged ... ok

**C170** - A rejected design comes back from `Project` with the stage that refused it (the loader DIG-15 AC 3 stands on). (DIG-15, AC 3; T2)
Proof: run project | test a_diagnostic_is_returned_with_its_stage ... ok

**C171** - Each placement's carrier arguments are bound in the authored `Child`, one entry per carrier port of the child (the authored-tree half of DIG-09 AC 4; `ir-rust` "Left for msl-digital"). (DIG-09, AC 4; T28)
Proof: run edig | test a_placement_records_its_carrier_bindings ... ok

**C172** - A net's `init` is the folded initial value of the carrier that declares it, and none when it has none (the elaboration half of `ir-rust` F-B2). (DIG-09, AC 1; T28)
Proof: run edig | test a_net_takes_the_init_of_its_declaring_carrier ... ok

**C173** - A hoisted carrier named `gate.x` or `above$0.init$0.up` compiles with no `rustc` warning and keeps its IR name on its ABI pin (the generated-code half of DIG-10 AC 1; contract.md D9). (DIG-10, AC 1; T40)
Proof: run amended | test hoisted_names_print_as_rust_identifiers ... ok

**C174** - `Body::child_carriers` gives each child's carrier bindings, prints them only when present, and `check()` refuses a wrong count, an index out of range and a value type mismatch, one entry each (the IR half of DIG-09 AC 4). (DIG-09, AC 4; T64)
Proof: run iramend | test a_child_records_its_carrier_bindings ... ok

**C175** - A parent's `signal` its children's carrier ports meet on reaches them on the solver: the flip-flop clocked by it reads `q` true (the solver half of DIG-09 AC 4; `ir-rust` F-B1). (DIG-09, AC 4; T65)
Proof: run nets | test a_parent_signal_reaches_its_childrens_ports ... ok

**C176** - A net with an `init` holds it at `(0, 0)` before any write (the solver half of DIG-09 AC 1; `ir-rust` F-B2). (DIG-09, AC 1; T65)
Proof: run nets | test a_net_starts_at_its_own_init ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (contract.md) - the merged ir-rust needs no redesign; nine differences D1-D9 are renames or where a field lives: widths are `u32`; `Expr::Int(i128)` takes its type from `ExprData`; per-body lists (carriers, drives, events, reactions, and A4's streams) are added by builder methods on a body id, never through literal-built structs; A6 keeps `Child::params` and stores the expression values beside the child (`IrBuilder::child_param`, `Body::child_params`) because `Child` is built literally in the frozen interpreter's tests; T20 has no `check()` rule against `ddt` under a potential to replace; `Deps` keeps its fields, so flow and previous-value reads are accessors beside `carrier_reads` and `At` gets `Target::At(k)`; the plan already has `Source::BranchFlow` and `Source::Static`, so A1 adds `Source::Time` only and A5 adds no `Source`; `Text::ident` does not map `.`/`$`, so T40 maps them (`_d`, `_s`) and the IR keeps DIG-10's names. ir-rust's "Left for msl-digital" items are owned here: F-B8 extends T3 (C9 widened); the `Child` carrier bindings are new task T64 (phase 4, C174) and are filled by T28 (C171); F-B2's elaboration half is T28 (C172); F-B1 and F-B2's solver halves are new task T65 (phase 8, Opus, C175, C176), which replaces ir-rust's two refusal tests and makes a parent's signal met by a child a `PORT` pin, with at most one additive `CircuitBuilder` method if a net's init needs it. T40 gains C173 (hoisted names in generated code). Phase 4 is now 8 tasks, phases 7-8 13. T2 - `Project::elaborate` returns `Result<Ir, Rejection>` (design wrote `Vec<DesignDiagnostic>`): a `Rejection` carries the stage with the diagnostics, which C170 asserts; `Project::cache(dir)` takes the build directory so a test passes one under `CARGO_TARGET_TMPDIR` and sets no environment variable; the root crate is named `bench` in the graph. T3 - C2's six positions are proven by the task that first types each: T3's test has the annotation, `const`, parameter, function result, both arms of an `if`, a suffix and none (`i32`); the binary operand is in T4's `integer_operators_take_one_type`, the method receiver in T5's `min_max_clamp_and_floor_are_typed`, the `match` arm in T8's `match_is_typed_over_enums_bools_integers_and_options`, the definition and commit targets in phase 3 (T10, T13), no test being edited after its task. `usize`/`isize` are typed as `UInt<64>`/`Int<64>` (part I §1 Resolution). The elaborator maps an integer `Ty` to no IR type until T23, so elaboration refuses integer code by name (`P0001`). T4 - `in_window` is no positive case of T4: its first statement is a tuple `let`, which T7 types; it is read from source among T7's cases (`a_tuple_let_names_each_component`), and T4 reads `rev3`, `dithered_width`, `target_codes` and `ovp_threshold`. `&`, `|`, `^` also type on `bool` (the buck's `Never(clk, hs & ls)`); `%` and the bit operators on `f64` stay `P0001`; `bool as` an integer type is accepted (the IR's `Cast` allows it). T5 - `Types::builtin` does not exist before T9 (it holds the carrier built-ins), so the third test finds `Types::callee` or `Types::method` for every call and method call, and a probe linked by its own path to its quantity (a probe records no callee: the POM resolves it); T9's tests add `builtin`. The elaborator refuses `min`/`max`/`clamp`/`floor` by name until T23. T6 - an enum that does not derive `Repr` is `P0001` as well (DIG-03 AC 1 types only one that does); `==` on an enum without `PartialEq` is `P0002`. T7 - `None` takes the type its position expects (an annotation, a parameter, the other arm of an `if`); with none it is `P0002`. The implicit `Some` of §3.2.3 is recorded per argument (`Types::implicit_some`) for the elaborator; it is applied at function arguments here, at value inputs in T15. A module param of `Option` type stays `P0001`. T8 - an integer `match` is exhaustive through `_`, or through literals naming every value of a type of at most 16 bits; a constant pattern covers no value for that count. A `match` whose pattern was refused is not also judged for coverage (one mistake, one diagnostic). A binding pattern is allowed inside `Some(..)` only. The `match` over an unsupported type reads ``a `match` expression over `f64` ``, so the M1 row naming `match` expression stands unedited. `Float<16>`'s message reads ``type `Float<16>` (`Float` of width 16; ...)``. T9 - a carrier is `Ty::Carrier { nature, value }` and a read of it has the type `value`; `Types::carrier(decl)` gives the declaration's type and `Types::builtin(expr)` the built-in a method call stands for (`Builtin::{Known, KnownOr, IsKnown, High, Changed, Prev, Rise, Fall}`). Only the std `Bit` is a carrier nature (any other discrete nature stays `P0001`). C23 names a read in a definition, a guard and a reaction: each is asserted by the task that first types it (T10, T12, T13), T9 asserts it through a module-body `let`, a function argument and an operand. `&str` lowers only inside `Bit<..>` (`E0112`) until T14 gives `Ty::Str`. A `Bit<T>` port without `in`/`out` and a node port with one are `E0310`. The checker orders a module body as declarations, `let`s, then definitions. A method call the POM left `Unresolved` on a carrier is typed against the catalogue on the value type (DIG-02 AC 1). A project that defines a method named like a flag read makes a value receiver `P0001` too. The elaborator refuses a carrier, a definition, a `digital` block or a module with a result by name (`Unlowered::{Carrier, Definition, ResultModule, DigitalBlock}`) before lowering anything, until phase 6 lowers them. T10 - `Types::continuity(expr)` (`Continuity::{Discrete, Continuous}`) is a syntactic analysis over the POM: a name for a value is followed to the expression that gave it (`let`, `if let`, `match` arm), a function to its body, a placement to its result. The core rows `now()` (`f64`) and `analysis(Static)` (`bool`; any other kind is `P0001` naming `analysis`) were laid here, not in T15, because `E0336` needs them typed; T15 asserts them. `drive_target` is shared by definitions and commits (T13). The target of a definition that is `inst.name` is `E0222`; `x[i]` and a field of a bundle are `P0001` "an element or field definition". T11 - a placement written `Module(nodes) { param: value }` (the form every placement of the buck uses) is typed; M1 refused it as a "param block on a call" before reaching the placement check. A node at a carrier port, and a carrier at a node port, are `E0250`; a carrier of another value type is `E0251`; an expression of another type at an `in` port is `E0337` (it is an anonymous definition). The drive checks of a hand-over (`E0220`, `E0223`) run at the end of the module, after its definitions and commits, so a definition written after the placement is seen; the commit half of C32 is asserted in T13. The positive cases `Controller` and `PowerStage` are checked over stub children that keep the ports of the buck children (copied from the source) and the params the parents set. T12 - an `Event` is judged where it stands: an event position hands the one expression that may be an `Event` down just before it types it (`event_slot`), the checker reports an `Event` anywhere else as `E0441`, or as `P0001` "an event read as a `bool`" inside a guard (part IV §13 rules 2 and 9, which the elaborator has no read for). The heads of reactions are typed in T12 (a `digital` block is accepted, its generation is `P0001`), so `E0430` has a test here; a reaction body is `P0001` "a reaction body" until T13 types it. `||` of an event and a `bool` is a plain `P0002` at the `bool` operand; `!` on an event is `E0440` and poisons the expression so no second diagnostic follows. `cross`'s second and third argument are accepted when they are an `Option<f64>` (a pass-through such as `above` writes: `cross(e, ttol, etol)`) or `None`, and `P0001` when a value is given (`ttol`/`etol`, `Some(..)` or a bare `f64`); the elaborator will refuse a `Some` bound to a pass-through param. `cross`/`at` outside an event position are `E0443` and return no type. `c.changed()`/`rise()`/`fall()` on a carrier; `rise`/`fall` only on `Bit<bool>`; `changed` on a param stays `P0001` (a param is not a carrier here). T13 - a reaction body is typed in `Context::Reaction` (probes and the environment allowed; a core module such as `ddt` is `E0350` like a placement); a commit is typed there only (elsewhere `P0001`; a `<-` in an `analog` block is already a parse error of the POM, `E0404`). The value of a commit is checked with `committing` set, so an `Event` committed is `E0441` (C41) while an `Event` read as a `bool` elsewhere in a reaction body is `P0001`. `E0220` for a carrier a module hands to an `out` port and also commits is reported at the hand-over (the commit half of C32). A placement in a `fn`, a closure or a reaction body is `E0350` at the placement, typed no further. The buck reactions of `Dpwm` (without its `Never` monitor, which is a stdlib module reached in T14/T16), `Pid`, `RefRamp`, `Adc` and `SetpointReg` type clean from the source text read off `examples/buck`. T14 - `Ty::Str` is the type of a string literal and of `&str`; a `let` that binds one (module body, block, tuple) is `P0001` "a string literal bound by a `let`". Which functions reach `emit!` is read off the source (`Types::emits(fn)`, the streams written directly or through calls), so a function not checked yet answers at its call. A call of an emitting `fn` where `emit!` may not stand is `E0601` (spec.md AC 14 uses `E0601` for the call as well, part VI's table says `E0122`); from `analog` it is `E0603` when the streams hold `sim` or `checks`. A stream belongs to a module: the sites of one module and of the functions it calls (transitively, callee first) share field types (`E0605`), two modules are two streams; the reserved fields `sim.{finish, failed: bool, msg: &str}` and `checks.{kind, name: &str, pass: bool, margin: f64}` are checked at the site. A field that is not a value is `E0606` (an `Event` included), a field value of type `Option` is `E0606` too. The stdlib's `Never` and the buck's `Supervisor` type clean. T15 - a placement of a module with a result has the type of its result (`E0301` when the trailing expression differs or is missing); a module with a value input or a result counts its arguments by the call rule (`E0141`, trailing `Option` inputs optional), any other keeps `E0312`; named arguments stay `P0001`. A value input takes no direction (`E0310`) and no initial value (`E0311`). `E0352` is a placement in a const context (param default, `where`, `const` item) and in a bound param value (a syntactic scan of the param block). `E0150` is a syntactic scan of each `analog` block that follows a `let` to its value, a placement to its result with the value inputs replaced by the caller's expressions, and a callee's own `analog` read of a value input to the argument at the placement; the receiver of a flag read (`known`, `known_or`, `is_known`, `high`) is the one carrier read it lets through, an `Event` form (`changed`, `rise`, `fall`) is left to `E0441`. `E0332`/`E0353` track the module-body `let`s being typed and the placements whose arguments are being typed (a core module counts as a direct dependence); a feedback through a result that reads only its own carriers is legal in the LRM and `P0001` here, because the type of the `let` is not known before its value. The elaborator refuses a value input by name (`Unlowered::ValueInput`). A module named `m` cannot be placed (`m` is the implicit param of §8.5). T16 - `Types::check(Buck)` is clean: it needed one rule beyond the tasks, the flow read of a branch the module drives with a potential (`v(p, n) +> ddt(l * i(p, n))`, the inductor): a flow probe read as a value is typed `f64` when an `analog` block of the module holds a potential contribution to the same nodes in the same order, `P0001` otherwise (a flow branch stays outside, DIG-07). The elaborator refuses that read by name (`lower_probe`) until T20/T35, since it would otherwise lower it as the potential, and refuses every digital form the checker accepts (`tests/elab_unlowered.rs::a_form_the_checker_accepts_for_msl_digital_is_p0001_until_it_is_lowered`). DIG-07 AC 1 is one table of 30 sources in `types_buck.rs`, most written in a reaction body.
**Dispatched:** ir-rust merged to main 9aa03fe8 (2026-11-15). Batch 1 (phases 1-2, T1-T8) - Opus worker. Batch 1 done. Batch 2 (phase 3, T9-T16) - Sonnet worker, 2026-11-15. Batch 2 done (T9-T16 committed, one commit each). Batch 2 done. Batch 3 (phase 4: T17-T22, T61, T64) - Sonnet worker, 2026-11-16.
**Walked:** none (no screen; each story's Independent Test is a check above)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10

Phase 1:  T1 ------→ T2
Phase 2:  T3 ------→ T4 ------→ T5 ------→ T6 ------→ T7 ------→ T8
Phase 3:  T9 ------→ T10 ------→ T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16
Phase 4:  T17 ------→ T18 ------→ T19 ------→ T20 ------→ T61 ------→ T21 ------→ T64 ------→ T22
Phase 5:  T23 ------→ T24 ------→ T25 ------→ T26 ------→ T27
Phase 6:  T28 ------→ T29 ------→ T30 ------→ T31 ------→ T32 ------→ T33 ------→ T34 ------→ T35 ------→ T36
Phase 7:  T37 ------→ T38
Phase 8:  T39 ------→ T40 ------→ T41 ------→ T42 ------→ T43 ------→ T44 ------→ T45 ------→ T46 ------→ T62 ------→ T63 ------→ T65
Phase 9:  T47 ------→ T48 ------→ T49 ------→ T50 ------→ T51 ------→ T52 ------→ T53
Phase 10:  T54 ------→ T55 ------→ T56 ------→ T57 ------→ T58 ------→ T59 ------→ T60
```

Execution is strictly sequential. T61 to T63 are amendment A7 (the previous value of a carrier), added by the orchestrator's ruling on `rise()` after the first 60 tasks were numbered: they keep their numbers and sit where the maps show. T64 and T65 are what `ir-rust` left to this feature (its tasks.md `## Handoff`, "Left for msl-digital"), added by T1 (contract.md). Batches for Execute, whole phases each: phases 1 and 2 (8 tasks), phase 3 (8), phase 4 (8), phase 5 (5), phase 6 (9), phases 7 and 8 (13), phase 9 (7), phase 10 (7).

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: The contract with the merged ir-rust | 1 record | ✅ Granular |
| T2: Project: a crate graph to a compiled circuit | 1 component | ✅ Granular |
| T3: Ty: integer types, literals and constants | 1 component | ✅ Granular |
| T4: Integer operators and casts | 1 component | ✅ Granular |
| T5: The catalogue: integer and float rows, and the report of G3 | 1 component | ✅ Granular |
| T6: Unit enums | 1 component | ✅ Granular |
| T7: Option and tuples | 1 component | ✅ Granular |
| T8: match and if let | 1 component | ✅ Granular |
| T9: Discrete carriers and the flag reads | 1 component | ✅ Granular |
| T10: Definitions | 1 component | ✅ Granular |
| T11: Discrete ports at a placement | 1 component | ✅ Granular |
| T12: Events | 1 component | ✅ Granular |
| T13: Reactions and commits | 1 component | ✅ Granular |
| T14: emit! and the functions that reach it | 1 component | ✅ Granular |
| T15: Modules with a result, time and the analysis | 1 component | ✅ Granular |
| T16: What stays outside, and the buck | 1 component | ✅ Granular |
| T17: IR: the time and the static reads | 1 component | ✅ Granular |
| T18: IR: the at event | 1 component | ✅ Granular |
| T19: IR: emit sites and streams | 1 component | ✅ Granular |
| T20: IR: the flow of a potential branch | 1 component | ✅ Granular |
| T61: IR: the previous value of a carrier | 1 component | ✅ Granular |
| T21: IR: a child param that reads the parent | 1 component | ✅ Granular |
| T64: IR: the carrier bindings of a child | 1 component | ✅ Granular |
| T22: The interpreter refuses the amended forms | 1 component | ✅ Granular |
| T23: Lowering integers, and folding them | 1 component | ✅ Granular |
| T24: Known: which values may be unknown | 1 component | ✅ Granular |
| T25: Variants: one function per flag pattern | 1 component | ✅ Granular |
| T26: Enums and match | 1 component | ✅ Granular |
| T27: Option, tuples and if let | 1 component | ✅ Granular |
| T28: Carriers and nets | 1 component | ✅ Granular |
| T29: Drives and anonymous definitions | 1 component | ✅ Granular |
| T30: Events and heads | 1 component | ✅ Granular |
| T31: Reactions and commits | 1 component | ✅ Granular |
| T32: Child params that read the parent | 1 component | ✅ Granular |
| T33: Hoisting modules with a result | 1 component | ✅ Granular |
| T34: emit! and the functions that emit | 1 component | ✅ Granular |
| T35: The analog forms of the power stage | 1 component | ✅ Granular |
| T36: Invariants of elaboration | 1 test target | ✅ Granular |
| T37: The slice equals the hand-built fixtures | 1 test target | ✅ Granular |
| T38: The whole buck elaborates | 1 test target | ✅ Granular |
| T39: Plan: the ambient reads | 1 component | ✅ Granular |
| T40: Generated code: now() and analysis(Static) | 1 component | ✅ Granular |
| T41: Plan: timers | 1 component | ✅ Granular |
| T42: Generated code: arming a timer | 1 component | ✅ Granular |
| T43: Plan: record pins | 1 component | ✅ Granular |
| T44: Generated code: emit | 1 component | ✅ Granular |
| T45: Plan: the flow of a potential branch | 1 component | ✅ Granular |
| T46: Generated code: the flow read | 1 component | ✅ Granular |
| T62: Plan: the previous value of a carrier | 1 component | ✅ Granular |
| T63: Generated code: the previous value | 1 component | ✅ Granular |
| T65: A parent's signal on its children's net, and a net's own init | 1 component | ✅ Granular |
| T47: Commit semantics from source | 1 test target | ✅ Granular |
| T48: The oscillator and the DPWM from source | 1 test target | ✅ Granular |
| T49: The ramp, the PID and the setpoint register from source | 1 test target | ✅ Granular |
| T50: The ADC and the comparator from source | 1 test target | ✅ Granular |
| T51: Time and timers from source | 1 test target | ✅ Granular |
| T52: Records from source | 1 test target | ✅ Granular |
| T53: The gate driver and the power switch from source | 1 test target | ✅ Granular |
| T54: The inductor and the current sense from source | 1 test target | ✅ Granular |
| T55: The supervisor from source | 1 test target | ✅ Granular |
| T56: The whole buck compiles and idles | 1 test target | ✅ Granular |
| T57: The closed loop | 1 test target | ✅ Granular |
| T58: GAPS.md after msl-digital | 1 record | ✅ Granular |
| T59: The buck's own README and GAPS | 1 record | ✅ Granular |
| T60: MD-64 and the instruction files | 1 record | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None in its phase | None in its phase | ✅ Match |
| T2 | T1 | T1 | ✅ Match |
| T3 | None in its phase | None in its phase | ✅ Match |
| T4 | T3 | T3 | ✅ Match |
| T5 | T4 | T4 | ✅ Match |
| T6 | T5 | T5 | ✅ Match |
| T7 | T6 | T6 | ✅ Match |
| T8 | T7 | T7 | ✅ Match |
| T9 | None in its phase | None in its phase | ✅ Match |
| T10 | T9 | T9 | ✅ Match |
| T11 | T10 | T10 | ✅ Match |
| T12 | T11 | T11 | ✅ Match |
| T13 | T12 | T12 | ✅ Match |
| T14 | T13 | T13 | ✅ Match |
| T15 | T14 | T14 | ✅ Match |
| T16 | T15 | T15 | ✅ Match |
| T17 | None in its phase | None in its phase | ✅ Match |
| T18 | T17 | T17 | ✅ Match |
| T19 | T18 | T18 | ✅ Match |
| T20 | T19 | T19 | ✅ Match |
| T61 | T20 | T20 | ✅ Match |
| T21 | T61 | T61 | ✅ Match |
| T64 | T21 | T21 | ✅ Match |
| T22 | T64 | T64 | ✅ Match |
| T23 | None in its phase | None in its phase | ✅ Match |
| T24 | T23 | T23 | ✅ Match |
| T25 | T24 | T24 | ✅ Match |
| T26 | T25 | T25 | ✅ Match |
| T27 | T26 | T26 | ✅ Match |
| T28 | None in its phase | None in its phase | ✅ Match |
| T29 | T28 | T28 | ✅ Match |
| T30 | T29 | T29 | ✅ Match |
| T31 | T30 | T30 | ✅ Match |
| T32 | T31 | T31 | ✅ Match |
| T33 | T32 | T32 | ✅ Match |
| T34 | T33 | T33 | ✅ Match |
| T35 | T34 | T34 | ✅ Match |
| T36 | T35 | T35 | ✅ Match |
| T37 | None in its phase | None in its phase | ✅ Match |
| T38 | T37 | T37 | ✅ Match |
| T39 | None in its phase | None in its phase | ✅ Match |
| T40 | T39 | T39 | ✅ Match |
| T41 | T40 | T40 | ✅ Match |
| T42 | T41 | T41 | ✅ Match |
| T43 | T42 | T42 | ✅ Match |
| T44 | T43 | T43 | ✅ Match |
| T45 | T44 | T44 | ✅ Match |
| T46 | T45 | T45 | ✅ Match |
| T62 | T46 | T46 | ✅ Match |
| T63 | T62 | T62 | ✅ Match |
| T65 | T63 | T63 | ✅ Match |
| T47 | None in its phase | None in its phase | ✅ Match |
| T48 | T47 | T47 | ✅ Match |
| T49 | T48 | T48 | ✅ Match |
| T50 | T49 | T49 | ✅ Match |
| T51 | T50 | T50 | ✅ Match |
| T52 | T51 | T51 | ✅ Match |
| T53 | T52 | T52 | ✅ Match |
| T54 | None in its phase | None in its phase | ✅ Match |
| T55 | T54 | T54 | ✅ Match |
| T56 | T55 | T55 | ✅ Match |
| T57 | T56 | T56 | ✅ Match |
| T58 | T57 | T57 | ✅ Match |
| T59 | T58 | T58 | ✅ Match |
| T60 | T59 | T59 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: The contract with the merged ir-rust | Feature records, instruction and gap files | none | none | ✅ OK |
| T2: Project: a crate graph to a compiled circuit | The project loader and behavior from source | integration | integration | ✅ OK |
| T3: Ty: integer types, literals and constants | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T4: Integer operators and casts | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T5: The catalogue: integer and float rows, and the report of G3 | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T6: Unit enums | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T7: Option and tuples | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T8: match and if let | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T9: Discrete carriers and the flag reads | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T10: Definitions | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T11: Discrete ports at a placement | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T12: Events | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T13: Reactions and commits | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T14: emit! and the functions that reach it | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T15: Modules with a result, time and the analysis | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T16: What stays outside, and the buck | The checker (`piperine-lang` `types/`) | unit | unit | ✅ OK |
| T17: IR: the time and the static reads | The IR amendments | unit | unit | ✅ OK |
| T18: IR: the at event | The IR amendments | unit | unit | ✅ OK |
| T19: IR: emit sites and streams | The IR amendments | unit | unit | ✅ OK |
| T20: IR: the flow of a potential branch | The IR amendments | unit | unit | ✅ OK |
| T61: IR: the previous value of a carrier | The IR amendments | unit | unit | ✅ OK |
| T21: IR: a child param that reads the parent | The IR amendments | unit | unit | ✅ OK |
| T64: IR: the carrier bindings of a child | The IR amendments | unit | unit | ✅ OK |
| T22: The interpreter refuses the amended forms | The interpreter's refusal | unit | unit | ✅ OK |
| T23: Lowering integers, and folding them | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T24: Known: which values may be unknown | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T25: Variants: one function per flag pattern | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T26: Enums and match | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T27: Option, tuples and if let | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T28: Carriers and nets | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T29: Drives and anonymous definitions | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T30: Events and heads | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T31: Reactions and commits | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T32: Child params that read the parent | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T33: Hoisting modules with a result | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T34: emit! and the functions that emit | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T35: The analog forms of the power stage | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T36: Invariants of elaboration | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T37: The slice equals the hand-built fixtures | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T38: The whole buck elaborates | The elaborator (`piperine-lang` `elab/`) | unit | unit | ✅ OK |
| T39: Plan: the ambient reads | The plan amendments | unit | unit | ✅ OK |
| T40: Generated code: now() and analysis(Static) | Generated code for the amended forms | integration | integration | ✅ OK |
| T41: Plan: timers | The plan amendments | unit | unit | ✅ OK |
| T42: Generated code: arming a timer | Generated code for the amended forms | integration | integration | ✅ OK |
| T43: Plan: record pins | The plan amendments | unit | unit | ✅ OK |
| T44: Generated code: emit | Generated code for the amended forms | integration | integration | ✅ OK |
| T45: Plan: the flow of a potential branch | The plan amendments | unit | unit | ✅ OK |
| T46: Generated code: the flow read | Generated code for the amended forms | integration | integration | ✅ OK |
| T62: Plan: the previous value of a carrier | The plan amendments | unit | unit | ✅ OK |
| T63: Generated code: the previous value | Generated code for the amended forms | integration | integration | ✅ OK |
| T65: A parent's signal on its children's net, and a net's own init | Generated code for the amended forms | integration | integration | ✅ OK |
| T47: Commit semantics from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T48: The oscillator and the DPWM from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T49: The ramp, the PID and the setpoint register from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T50: The ADC and the comparator from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T51: Time and timers from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T52: Records from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T53: The gate driver and the power switch from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T54: The inductor and the current sense from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T55: The supervisor from source | The project loader and behavior from source | integration | integration | ✅ OK |
| T56: The whole buck compiles and idles | The project loader and behavior from source | integration | integration | ✅ OK |
| T57: The closed loop | The project loader and behavior from source | integration | integration | ✅ OK |
| T58: GAPS.md after msl-digital | Feature records, instruction and gap files | none | none | ✅ OK |
| T59: The buck's own README and GAPS | Feature records, instruction and gap files | none | none | ✅ OK |
| T60: MD-64 and the instruction files | Feature records, instruction and gap files | none | none | ✅ OK |

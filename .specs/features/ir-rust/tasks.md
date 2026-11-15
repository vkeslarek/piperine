# IR to Rust Transpiler Tasks

**Approved**: 2026-11-15 - user: "Pode ir rodando os grandes passos" (standing delegation, R4)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/ir-rust/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-ir -p piperine-interp -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-plan || cargo build -p piperine-plan --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-transpile || cargo build -p piperine-transpile --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run plan**: `cargo test -p piperine-plan --test shared`
**Run reject**: `cargo test -p piperine-transpile --test reject`
**Run lib**: `cargo test -p piperine-transpile --lib`
**Run golden**: `cargo test -p piperine-transpile --test golden`
**Run rustc**: `cargo test -p piperine-transpile --test rustc`
**Run abisrc**: `cargo test -p piperine-transpile --test abi_source`
**Run cache**: `cargo test -p piperine-transpile --test cache`
**Run load**: `cargo test -p piperine-transpile --test load`
**Run compile**: `cargo test -p piperine-transpile --test compile`
**Run compiled**: `cargo test -p piperine-transpile --test compiled`
**Run differential**: `cargo test -p piperine-transpile --test differential`
**Run residue**: `cargo test -p piperine-transpile --test residue`
**Run session**: `cargo test -p piperine-transpile --test session`
**Run backend**: `cargo test -p piperine-parity --test backend`
**Run parity**: `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran`
**Run backends**: `cargo test -p piperine-parity --test backends`
**Run contract**: `cargo test -p piperine-parity --lib`
**Run irdigital**: `cargo test -p piperine-ir --test digital`
**Run frozen**: `cargo test -p piperine-interp --test frozen`
**Run planshape**: `cargo test -p piperine-plan --test digital_shape`
**Run reaction**: `cargo test -p piperine-transpile --test reaction`
**Run nets**: `cargo test -p piperine-transpile --test digital_circuit`
**Run commits**: `cargo test -p piperine-transpile --test commits`
**Run dpwm**: `cargo test -p piperine-transpile --test dpwm`
**Run pid**: `cargo test -p piperine-transpile --test pid`
**Run boundary**: `cargo test -p piperine-parity --test boundary`

Executors (S10): Sonnet implements; Opus implements the delicate tasks directly, those where a mistake gives a wrong number with no error or that hold `unsafe`: T3, T7, T8, T15, T26, T27, T31, T33, T35, T39. A different Opus verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` for each crate a command builds before running it.

The feature has two parts. **Phase A** (execution phases 1 to 4, T1 to T23) is the M1 analog subset through the transpiler, proven against the interpreter and ngspice; it is complete and mergeable alone. **Phase B** (execution phases 5 to 8, T24 to T46) is the digital and mixed slice of the buck, proven on hand-built IR against `docs/spec/` and `examples/buck/SPEC.md`.

Rules every task follows:

- **abi.md is the contract** (`.specs/features/device-abi/abi.md`). Generated code and the loader meet the solver through `piperine_abi_1`, shapes, `eval` and `react` only. `piperine-abi` and the three solver crates are not edited: a defect found there is reported with a hand-written ABI device that reproduces it, not patched here.
- **ngspice parity is the first priority and no tolerance is ever retuned.** `crates/piperine-parity/src/contract.rs` keeps every bound. A number you cannot explain: stop and report.
- **The interpreter is frozen** (R1). `piperine-interp` changes only in T1 (the move), in the refusal arms new IR variants force, and in T30. None of its tests is edited.
- **One plan.** Pins, read order, partial order and affine flags come from `piperine-plan` for both backends. The transpiler never derives a shape on its own.
- **The linear-term rule** (`context.md`, lesson of `solver-abi-integration` T5): a term affine in its continuous reads is written as `c + Σ partial·read`, folded from `0.0` in declared partial order, exactly as `Interpreted::terms` does.
- **The generated Rust is read by people** (R3): every item under a comment with its IR path and origin, MSL names, the printer's layout rules. No `unsafe`, no macro and no dependency but `piperine_abi` and `rt` in generated source.
- **Fail loud.** A form with no lowering is `Error::Unsupported` with form, path and origin; never a default value, never a skipped statement.
- **Tests state what the spec says, not what the code does.** Expected texts and values are written from the MSL source, `docs/spec/`, abi.md or `examples/buck/SPEC.md`, each cited at its assertion. Test functions have exactly the names `## Checks` cites. No test sets an environment variable.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, modules named after what they do, no `unwrap()`/`expect()` on a user-input path. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `crates/piperine-parity/tests/parity_corpus.rs` (a test per twin; no literal tolerance), `.specs/features/codegen/context.md` (S8, S10, R1 to R4).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| `piperine-plan` (the moved plan and netlist; the digital plan) | unit | each AC of RS-12; the M1 shapes unchanged; abi.md §11.4 and §11.5 entry by entry | `crates/piperine-plan/tests/{shared,digital_shape}.rs` | `cargo test -p piperine-plan` |
| The printer and the runtime source | unit | each AC of RS-02, RS-10, RS-11 (1 to 4); expected texts written from the MSL source; every operator and method of `rt` | unit tests in `crates/piperine-transpile/src/{print,rt}/*.rs`, `tests/{reject,golden}.rs` | `cargo test -p piperine-transpile --lib --test reject --test golden` |
| Settings, compiler, ABI sources, cache, loader | integration | each AC of RS-03, RS-04 (3, 4, 5, 8, 9), RS-05 (3, 4); every failure state with its error | `crates/piperine-transpile/tests/{rustc,abi_source,cache,load}.rs` | `cargo test -p piperine-transpile --test rustc --test abi_source --test cache --test load` |
| `Transpiler::compile`, `Compiled` | integration | RS-01 AC 1, RS-02 AC 4, RS-03 AC 3, RS-04 (1, 2, 5 to 7), RS-05 (1, 2), RS-08; compiler invocations counted by a wrapper | `crates/piperine-transpile/tests/{compile,compiled,session}.rs` | `cargo test -p piperine-transpile --test compile --test compiled --test session` |
| Compiled analog functions | integration | each AC of RS-06 (1 to 4, 7) over all five bodies; bit equality, never a tolerance | `crates/piperine-transpile/tests/{differential,residue}.rs` | `cargo test -p piperine-transpile --test differential --test residue` |
| Parity on either backend | integration | each AC of RS-07, RS-06 AC 5; the seven twins on both backends | `crates/piperine-parity/tests/{backend,backends,parity_*}.rs` | `cargo test -p piperine-parity` |
| The IR's digital node set | unit | each AC of RS-09 (1 to 5); golden prints of the ten buck-slice bodies | `crates/piperine-ir/tests/digital.rs` | `cargo test -p piperine-ir --test digital` |
| The interpreter's refusal | unit | RS-09 AC 6 | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Compiled reactions and circuits | integration | RS-11 (5, 6), RS-06 AC 6, each AC of RS-13; expected values computed in the test from the specification | `crates/piperine-transpile/tests/{reaction,digital_circuit,commits,dpwm,pid}.rs`, `crates/piperine-parity/tests/boundary.rs` | `cargo test -p piperine-transpile`; `cargo test -p piperine-parity --test boundary` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | the task's `Run` line |
| Full | every task | the task's `Run` line, then `cargo test -p <the crate the task edits>` |
| Build | the last task of each phase and every task that adds a crate, an `unsafe` block or a cross-crate edit | Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Eight phases, run in order, one batch each. Phases 1 to 4 are phase A of the spec; phases 5 to 8 are phase B.

### Phase 1: The shared plan, the crate, the runtime

Executor: Sonnet; T3 is Opus's.

```
T1 → T2 → T3 → T4
```

### Phase 2: Printing the analog subset

Executor: Sonnet; T7 and T8 are Opus's.

```
T5 → T6 → T7 → T8 → T9 → T10
```

### Phase 3: Build, cache, load

Executor: Sonnet; T15 is Opus's.

```
T11 → T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 4: Equal to the interpreter, equal to ngspice

Executor: Sonnet.

```
T18 → T19 → T20 → T21 → T22 → T23
```

### Phase 5: The IR's digital node set

Executor: Sonnet; T26 and T27 are Opus's.

```
T24 → T25 → T26 → T27 → T28 → T29 → T30
```

### Phase 6: The digital plan and runtime

Executor: Opus for T31, T33, T35; Sonnet for the rest.

```
T31 → T32 → T33 → T34 → T35 → T36
```

### Phase 7: Lowering digital and mixed forms

Executor: Sonnet; T39 is Opus's.

```
T37 → T38 → T39 → T40 → T41
```

### Phase 8: Behavior on the solver

Executor: Sonnet.

```
T42 → T43 → T44 → T45 → T46
```

---

## Task Breakdown

## Phase 1 tasks

### T1: Move the plan and the netlist to piperine-plan

**What**: The new crate `piperine-plan`: `plan.rs` moved from `piperine-interp` with `BodyPlan`, `FunctionPlan`, `Mode`, `Source`, `Target`, `Term` and `Pins` made `pub`; `netlist.rs` moved as `Netlist::new(&Ir).circuit(&impl Modules)` with the trait `Modules`; `piperine-interp` depends on it, re-exports `BodyPlan`, implements `Modules`, and `Interpreter::circuit` calls `Netlist`. A move: no logic changes.
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: None
**Reuses**: `crates/piperine-interp/src/{plan,netlist}.rs`; the manifest and guard edits of `ir-interp` T2
**Requirement**: RS-05
**Executor**: Sonnet

**Done when**:

- [x] `git diff --stat` shows no change under `crates/piperine-interp/tests/` and every test there passes
- [x] `DEFAULT_MEMBERS`, the workspace manifest and `default-members` gain `crates/piperine-plan`; `crate_boundaries` still passes (the crate sits above the solver)
- [x] Tests with exactly these names pass in `cargo test -p piperine-plan --test shared`: `the_interpreter_and_the_plan_crate_share_one_body_plan`, `the_netlist_binds_whatever_supplies_the_modules`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `refactor(plan): the shape plan and the netlist in piperine-plan`

---

### T2: The crate and Transpiler::new

**What**: The crate `piperine-transpile` (dependencies: `piperine-ir`, `piperine-abi`, `piperine-plan`, `sha2`, `libloading`; dev: `piperine-interp`, `tempfile`), `Error` (design Data Models), `Transpiler::new(Arc<Ir>)` that returns `Error::Rejected` with `check()`'s entries and plans each body that runs; MD-63 in `.specs/STATE.md`; the crate rows of `CLAUDE.md` and `AGENTS.md`.
**Where**: `crates/piperine-transpile/src/lib.rs`
**Depends on**: T1
**Reuses**: `piperine_interp::Interpreter::new` (the same acceptance rule)
**Requirement**: RS-01
**Executor**: Sonnet

**Done when**:

- [x] `DEFAULT_MEMBERS` and the manifests gain the crate; `workspace_shape` and `crate_boundaries` pass
- [x] MD-63 is written as the design states it
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test reject`: `an_ir_that_fails_check_gives_no_source`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(transpile): the crate, its errors and Transpiler::new`

---

### T3: rt: Real and Dual

**What**: `rt::Real` (`constant`, `value`, the four operators, `exp`, `ln`, `sqrt`, `abs`), implemented by `f64` and by `Dual<const N: usize> { v, d: [f64; N] }` with `seeded`, `constant` and the interpreter's formulas component by component (design "Dual numbers in generated code"). The file is a module of the crate and is embedded with `include_str!`.
**Where**: `crates/piperine-transpile/src/rt/real.rs`
**Depends on**: T2
**Reuses**: `crates/piperine-interp/src/dual.rs` (the formulas and its tests)
**Requirement**: RS-06
**Executor**: Opus

**Done when**:

- [x] The first test sweeps every operator and method over 200 points against `piperine_interp::Dual`: values with equal bits, tangents equal as `f64`
- [x] The file uses `core` only and compiles alone: `rustc --edition 2024 --crate-type rlib` on it exits 0
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `rt::real::tests::dual_gives_the_interpreters_values_and_tangents`, `rt::real::tests::abs_has_the_derivative_of_sign`, `rt::real::tests::f64_is_a_real`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): rt Real and Dual`

---

### T4: Text: the source writer

**What**: `print::Text`: `line`, `indented`, `comment(path, origin)`, `real(f64)` (shortest decimal that parses back to the same bits; `f64::INFINITY`, `f64::NEG_INFINITY`, `f64::NAN` by name) and `ident(&str)` (raw identifier for a Rust keyword).
**Where**: `crates/piperine-transpile/src/print/text.rs`
**Depends on**: T3
**Reuses**: `crates/piperine-ir/src/print.rs` (the IR printer's writer shape)
**Requirement**: RS-02
**Executor**: Sonnet

**Done when**:

- [x] `real` is checked over the subnormals, `-0.0`, `1e-14`, `0.1` and 1000 random bit patterns
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::text::tests::reals_round_trip_in_the_shortest_form`, `print::text::tests::keywords_become_raw_identifiers`, `print::text::tests::lines_are_indented_by_four_one_statement_each`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): the source writer`

---

## Phase 2 tasks

### T5: ValuePrinter: expressions and functions

**What**: `ValuePrinter`: every M1 value form (`Real`, `Bool`, `Unit`, `Local`, `Param`, `Unary`, `Binary`, `If`, `Block`, `Call`, `Math`, `Closure`) as a Rust expression, and an IR function as `fn name<R: Real>(…) -> R` under its comment; any other form is `Error::Unsupported` with the form's name, IR path and origin.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: None in this phase (phase 1 complete)
**Reuses**: `piperine_ir` handles; `Text`
**Requirement**: RS-01
**Executor**: Sonnet

**Done when**:

- [x] `pnjlim` and `vt` of the M1 fixtures print as the texts the test holds, written from the MSL source, not from the printer
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::value::tests::a_function_prints_as_a_generic_fn`, `print::value::tests::an_unsupported_form_is_named_with_path_and_origin`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): value expressions and functions as Rust`

---

### T6: ShapePrinter

**What**: `ShapePrinter::prototype(&Shape)`: any shape as `ShapeBuf` builder calls, one call per pin, term, input, watch, function and reaction, each under a comment with the entry's name and origin.
**Where**: `crates/piperine-transpile/src/print/shape.rs`
**Depends on**: T5
**Reuses**: `crates/piperine-abi/src/shape.rs` (`ShapeBuf`, the specs); `abi_devices.rs` for the look
**Requirement**: RS-02
**Executor**: Sonnet

**Done when**:

- [x] The test counts calls and comments against the entries of the diode's and the Vsource's plan shapes
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::shape::tests::every_shape_entry_is_one_call_under_its_comment`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): a shape as ShapeBuf calls`

---

### T7: AnalogPrinter: terms and the affine rule

**What**: `AnalogPrinter::terms`: the generic `terms_block<R: Real>` as the backward slice of the contributions, and the `terms` function that instantiates it at the iterate, at pins read as 0, or both, per the plan's `affine` flags, writing an affine term as `c + (0.0 + g0 * x0 + …)` in declared partial order (design "From an IR function to a Rust function").
**Where**: `crates/piperine-transpile/src/print/analog.rs`
**Depends on**: T6
**Reuses**: `Interpreted::terms` and `walk_terms` in `crates/piperine-interp/src/device.rs`; `FunctionPlan`
**Requirement**: RS-06
**Executor**: Opus

**Done when**:

- [x] Expected texts for `Resistor`, `Capacitor`, `Vsource` and `Diode` are written in the test from design.md's sample and the plan's read order
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::analog::tests::an_affine_term_is_printed_as_constant_plus_fold`, `print::analog::tests::a_slice_holds_only_what_its_outputs_reach`, `print::analog::tests::a_potential_branch_prints_its_flow_and_equation_terms`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): the terms function and the affine rule`

---

### T8: AnalogPrinter: limit

**What**: The `limit:<site>` function: the slice of `x`, `f` and `seed`; `PROPOSED` and its partials over `Dual<N>`; `old` from `LAST`, `AGE` and `STATIC`; `LIMITED` over plain `f64` with the closure printed as a Rust closure; the seed kept as INT-10 AC 3. Ported rule by rule from `walk.rs`.
**Where**: `crates/piperine-transpile/src/print/limit.rs`
**Depends on**: T7
**Reuses**: `crates/piperine-interp/src/walk.rs` (`Held`, the limit site); `crates/piperine-interp/tests/limit.rs` for the cases
**Requirement**: RS-06
**Executor**: Opus

**Done when**:

- [x] A comment in the generated function names each of the three rules (`old`, the seed start, the seed kept)
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::limit::tests::a_limit_function_reads_last_age_and_static`, `print::limit::tests::the_terms_function_reads_the_limit_value_only`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): the limit function`

---

### T9: Site printers: stim and check

**What**: The `stim:<site>` function (`RE`, `IM` from `mag` and `phase` over plain `f64`) and the `check:<param>` function (the `where` as a `bool`, written `HIGH`/`LOW`).
**Where**: `crates/piperine-transpile/src/print/site.rs`
**Depends on**: T8
**Reuses**: `Interpreted::{stim,check}`
**Requirement**: RS-01
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::site::tests::stim_and_check_print_plain_values`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): the stim and check functions`

---

### T10: LibraryPrinter and the golden sources

**What**: `LibraryPrinter::design`: the file header, `mod rt;`, one `mod` per body that runs with its `Device` impl (`prototype`, `create`, `eval` as a `match` with one commented arm per function, `react`), and `piperine_abi_1` over `thunk::library`; `Transpiler::source`. The five M1 bodies pinned under `tests/golden/`.
**Where**: `crates/piperine-transpile/src/print/library.rs`
**Depends on**: T9
**Reuses**: T5 to T9; `thunk::{module, library}`
**Requirement**: RS-01, RS-02
**Executor**: Sonnet

**Done when**:

- [x] The golden files are read by an Opus reviewer before the commit: each is followed from MSL line to Rust line with no line unexplained
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test golden`: `the_five_m1_bodies_print_as_the_golden_files`, `transpiling_twice_gives_the_same_bytes`, `every_item_sits_under_its_path_and_origin`, `each_function_is_a_generic_fn_and_each_body_a_device_mod`, `a_keyword_identifier_is_raw_and_the_abi_name_is_not`, `a_design_with_nothing_to_run_has_no_module`, `a_body_without_analog_prints_no_terms_function`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 7 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(transpile): the design as one Rust source, with golden files`

---

## Phase 3 tasks

### T11: Settings

**What**: `Settings::resolve(lookup)` and `from_environment()`: the compiler (`PIPERINE_RUSTC`, else `rustc` on `PATH`) and the cache directory (the four rules of RS-04 AC 9), pure over the lookup.
**Where**: `crates/piperine-transpile/src/settings.rs`
**Depends on**: None in this phase (phase 2 complete)
**Reuses**: none
**Requirement**: RS-03, RS-04
**Executor**: Sonnet

**Done when**:

- [x] No test calls `std::env::set_var`
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `settings::tests::the_cache_directory_follows_the_four_rules`, `settings::tests::the_compiler_is_the_variable_then_the_path`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): settings from the environment`

---

### T12: Rustc

**What**: `Rustc::locate`, `version_text`, `rlib`, `cdylib` with the flags of RS-Q4, the invocation counter, and the errors `NoCompiler` and `CompilerTooOld`.
**Where**: `crates/piperine-transpile/src/rustc.rs`
**Depends on**: T11
**Reuses**: `Settings`
**Requirement**: RS-03
**Executor**: Sonnet

**Done when**:

- [x] The old-compiler and no-cargo tests use shell wrappers written into a `tempdir` (`#[cfg(unix)]`): one prints an old `-vV`, one logs its own invocations while a `cargo` beside it logs any call
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test rustc`: `a_missing_compiler_is_named_and_the_interpreter_is_offered`, `a_compiler_below_1_85_is_refused_with_both_versions`, `the_compiler_is_run_directly_and_cargo_never`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): locating and running rustc`

---

### T13: AbiSource

**What**: `AbiSource::FILES` (each file of `crates/piperine-abi/src` by `include_str!`), `write_to(dir)`, and the `rlib` built from it by `Rustc::rlib`.
**Where**: `crates/piperine-transpile/src/abi_source.rs`
**Depends on**: T12
**Reuses**: `crates/piperine-abi/src/*.rs`
**Requirement**: RS-05
**Executor**: Sonnet

**Done when**:

- [x] The first test lists the directory and compares names and bytes with the table
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test abi_source`: `the_embedded_sources_are_the_abi_crate`, `the_abi_builds_with_rustc_alone`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): the ABI sources, embedded and built`

---

### T14: Cache

**What**: `Cache::at`, `Key::of`, `entry`, `build` (a staging sibling renamed into place; the loser of a race removes its staging), `Entry`, the manifest, `clear`.
**Where**: `crates/piperine-transpile/src/cache.rs`
**Depends on**: T13
**Reuses**: `sha2`
**Requirement**: RS-04
**Executor**: Sonnet

**Done when**:

- [x] The race test runs 8 threads through `build` on one key and lists the directory afterwards
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test cache`: `two_racing_builds_leave_one_entry_and_no_staging`, `an_unwritable_directory_is_named`, `the_key_changes_with_any_part`, `a_directory_is_created_private`
- [x] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): the cache`

---

### T15: Loaded

**What**: `Loaded::open(&Entry)`: the recorded hash checked, `libloading` open, `piperine_abi_1`, `Library.size` and `minor` checked, the handle leaked; `module(name)`. The only `unsafe` of the feature, each block with a `// SAFETY:` line citing abi.md.
**Where**: `crates/piperine-transpile/src/load.rs`
**Depends on**: T14
**Reuses**: `crates/piperine-abi/src/{raw,caller}.rs` (`SYMBOL`, `Library`, how a `Module` is read)
**Requirement**: RS-05, RS-04
**Executor**: Opus

**Done when**:

- [x] The test libraries are small sources compiled by T12's `Rustc` into a `tempdir`
- [x] `unsafe_lives_in_load_only` scans `src/` of the crate and the golden files
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test load`: `a_file_that_is_no_library_is_named`, `a_library_without_the_symbol_is_named`, `a_library_of_a_newer_minor_is_refused`, `a_library_whose_bytes_changed_is_not_loaded`, `unsafe_lives_in_load_only`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(transpile): loading a library through piperine_abi_1`

---

### T16: Transpiler::compile

**What**: `Transpiler::compile(&Settings)`: source, key, cache lookup, the ABI `rlib` once, the `cdylib`, the manifest, `Loaded::open`, one rebuild when the hash check fails; `Report`; the staging directory kept and named when `rustc` fails.
**Where**: `crates/piperine-transpile/src/compile.rs`
**Depends on**: T15
**Reuses**: T10 to T15
**Requirement**: RS-01, RS-02, RS-03, RS-04, RS-08
**Executor**: Sonnet

**Done when**:

- [x] Compiler invocations are counted by a wrapper script's log, not by the crate's own counter
- [x] The warning test reads `rustc`'s standard error for each of the five M1 bodies and requires it empty
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test compile`: `the_m1_design_compiles_with_no_error_and_no_warning`, `a_second_build_runs_no_compiler`, `a_changed_ir_is_another_entry`, `another_compiler_version_is_another_entry`, `a_compile_error_keeps_the_source_and_shows_stderr`, `a_corrupted_library_is_rebuilt`, `the_source_is_kept_beside_the_library_and_reported`, `a_build_reports_its_cost`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 8 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(transpile): compile, cache and load a design`

---

### T17: Compiled: modules and the circuit

**What**: `Compiled` implements `Modules`; `Compiled::circuit()` through `Netlist`; every loaded prototype compared entry by entry with the plan's shape (`Error::ShapeMismatch`).
**Where**: `crates/piperine-transpile/src/compiled.rs`
**Depends on**: T16
**Reuses**: `piperine_plan::Netlist`; `crates/piperine-interp/tests/circuit.rs` for the circuits
**Requirement**: RS-05
**Executor**: Sonnet

**Done when**:

- [x] The divider's `Dc` gives `mid` = half of `vin` to 1e-12 relative, as `ir-interp`'s circuit test does
- [x] Tests with exactly these names pass in `cargo test -p piperine-transpile --test compiled`: `modules_come_through_the_symbol_and_bind_like_the_interpreter`, `loaded_shapes_equal_the_plans_entry_by_entry`, `a_shape_that_differs_from_its_plan_is_refused`, `the_compiled_divider_solves`
- [x] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(transpile): the compiled design as a circuit`

---

## Phase 4 tasks

### T18: Function by function against the interpreter

**What**: The differential target: every function of the five M1 bodies evaluated through `caller::Handle` on the interpreter's module and on the compiled one with the same bytes: three bias points each, the six `AGE`/`STATIC` cases for the limit, seeds all on. `verify_partials` on every compiled function. A non-finite value.
**Where**: `crates/piperine-transpile/tests/differential.rs`
**Depends on**: None in this phase (phase 3 complete)
**Reuses**: `crates/piperine-interp/tests/{terms,limit,stim,partials}.rs` (the points and the calls)
**Requirement**: RS-06
**Executor**: Sonnet

**Done when**:

- [ ] A difference is a defect of T3, T7 or T8: it goes back to Opus with the function, the inputs and both outputs. No comparison is loosened
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test differential`: `every_m1_function_gives_the_interpreters_bytes`, `every_compiled_function_passes_verify_partials`, `a_non_finite_value_is_written_as_is`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): compiled functions equal the interpreter's`

---

### T19: The linear-term residue on compiled code

**What**: The interpreter's residue tests on compiled functions: the resistor over 1000 points (bits of `g*vp - g*vn`, right side exactly 0) and the Vsource `eq` term over 2000 points (within 1 ulp, exactly 0 at `dc = 0`, with inexact points present).
**Where**: `crates/piperine-transpile/tests/residue.rs`
**Depends on**: T18
**Reuses**: `crates/piperine-interp/tests/terms.rs` (C28, C29, C48)
**Requirement**: RS-06
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test residue`: `the_compiled_resistor_matches_the_hand_written_products_bit_for_bit`, `a_compiled_affine_constant_leaves_at_most_one_ulp`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): no residue on linear rows of compiled devices`

---

### T20: No compiler during a session

**What**: A session on the compiled RC and diode circuits with the counting wrapper: `Dc`, `Ac`, transient and a `set` of a non-structural param between analyses; the wrapper's log does not grow after the build.
**Where**: `crates/piperine-transpile/tests/session.rs`
**Depends on**: T19
**Reuses**: T16's wrapper; `crates/piperine-interp/tests/circuit.rs`
**Requirement**: RS-04
**Executor**: Sonnet

**Done when**:

- [ ] The `set` test reads the changed result of the next `Dc`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test session`: `no_analysis_runs_the_compiler`, `a_set_between_analyses_runs_no_compiler_and_applies`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): nothing compiles after the build`

---

### T21: Backend in piperine-parity

**What**: `Backend { Interp, Rust }`, `Backend::parse` (pure), `Backend::selected()`, `Bench::load_on(fixture, backend)` (the `Rust` arm goes through `Transpiler::compile`), `Point::with_backend`; `Bench::load` stays the interpreter.
**Where**: `crates/piperine-parity/src/backend.rs`
**Depends on**: T20
**Reuses**: `crates/piperine-parity/src/{bench,contract}.rs`
**Requirement**: RS-07
**Executor**: Sonnet

**Done when**:

- [ ] `contract.rs` changes by one field and one line of the rejection text; no bound moves
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test backend`: `the_variable_selects_one_backend`, `an_unknown_backend_is_refused_naming_both`, `a_rejection_names_the_backend`, `the_rust_bench_runs_loaded_modules`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(parity): a twin on either backend`

---

### T22: Every parity test on both backends

**What**: Each comparison of `parity_op`, `parity_dc`, `parity_ac` and `parity_tran` loops over `Backend::selected()` inside its test function, running ngspice once per twin and naming the backend at every point.
**Where**: `crates/piperine-parity/tests/parity_op.rs` and its three siblings
**Depends on**: T21
**Reuses**: T21; the four targets as they are
**Requirement**: RS-07
**Executor**: Sonnet

**Done when**:

- [ ] No tolerance, no row count and no expected value of the four targets changes; `parity_corpus` passes
- [ ] A twin that fails on the `Rust` backend only is a transpiler defect: it is reported with the point, never met by a looser bound
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran`: `divider`, `diode_series`, `isrc_r`, `diode_iv`, `diode_rev`, `ac_rc`, `tran_rc`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 7 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): the seven twins on the compiled backend`

---

### T23: The M1 circuits on both backends

**What**: A target that runs the divider, the diode series, the AC RC and the transient RC on both backends and compares every result under the contract of its domain.
**Where**: `crates/piperine-parity/tests/backends.rs`
**Depends on**: T22
**Reuses**: `Bench::load_on`; `ToleranceContract`
**Requirement**: RS-06, RS-07
**Executor**: Sonnet

**Done when**:

- [ ] The target has no numeric literal beside a comparison: every bound is a `ToleranceContract`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test backends`: `both_backends_agree_on_the_m1_circuits`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): the two backends agree on the M1 circuits`

---

## Phase 5 tasks

### T24: IR: integer types and operators

**What**: `ValueType::{UInt, Int}`, `Expr::{Int, Cast, Method}`, `BinaryOp::{Rem, BitAnd, BitOr, BitXor, Shl, Shr}`, `Constant::Int`, their builder methods, typing rules in `check()` and printer forms (design "Phase B: the IR additions").
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None in this phase (phase 4 complete)
**Reuses**: the M1 node set and its builder, check and printer
**Requirement**: RS-09
**Executor**: Sonnet

**Done when**:

- [ ] `piperine-interp` and `piperine-transpile` still build: each new variant gets a refusal arm (`UNEXECUTED`, `Unsupported`)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `integers_and_their_operators_print`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): integer types, casts and bit operators`

---

### T25: IR: unknown values

**What**: `ValueType::Flagged`, `Expr::{Unknown, KnownOr, IsKnown, High}` with builder, check and printer.
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T24
**Reuses**: T24
**Requirement**: RS-09
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `unknown_values_and_their_reads_print`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): unknown values and their reads`

---

### T26: IR: carriers, drives and nets

**What**: `Carrier`, `Drive`, `Expr::Carrier`, `BodyData::{carriers, drives}`, `Ir::nets` with `NetData`, `InstanceData::nets`, handles and builder methods, printer sections written only when not empty.
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T25
**Reuses**: `NodeData`, `BodyNode` and their handles (the continuous twins)
**Requirement**: RS-09
**Executor**: Opus

**Done when**:

- [ ] Dependency sets (`Deps`) cover carrier reads: a drive's and an analog block's
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `carriers_and_drives_print`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): discrete carriers, drives and nets`

---

### T27: IR: events, reactions and commits

**What**: `Event` (`Changed`, `Cross`, guard), `Reaction`, `Stmt::Commit`, `BodyData::{events, reactions}`, builder, printer, and the dependency set of a reaction (carriers, params and nodes its body reads).
**Where**: `crates/piperine-ir/src/model.rs` (modify)
**Depends on**: T26
**Reuses**: T26; `Site` (how a body-level list is indexed)
**Requirement**: RS-09
**Executor**: Opus

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `events_and_reactions_print`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): events, reactions and commits`

---

### T28: IR: check() for the digital forms

**What**: `check()` entries: a commit or drive on a carrier the body may not write (`In`); a carrier with both a drive and a commit; a carrier read in `analog` outside `known_or`, `is_known`, `high`; an event index out of range; a commit outside a reaction; a `Cross` over a non-real expression.
**Where**: `crates/piperine-ir/src/check.rs`
**Depends on**: T27
**Reuses**: the M1 rules of the same file
**Requirement**: RS-09
**Executor**: Sonnet

**Done when**:

- [ ] The second test prints every M1 fixture and compares with `crates/piperine-ir/tests/golden/`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `a_defective_digital_body_gives_one_entry_per_defect`, `the_m1_prints_are_unchanged`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): check() for carriers, drives and reactions`

---

### T29: The buck-slice fixtures

**What**: Hand-built bodies, each beside the `.msl` text it transcribes: `Osc`, `Dpwm` (with `rev3`, `dithered_width`), `SetpointReg`, `RefRamp`, `Pid`, `GateDriver`, `Adc`, `Comparator` (the two `cross` reactions of `A2D`), `Switch` (an `analog` block reading `g.high()`), and the D flip-flop of abi.md §11.4; their printed IR pinned as golden texts.
**Where**: `crates/piperine-ir/tests/support/digital.rs`
**Depends on**: T28
**Reuses**: `crates/piperine-ir/tests/support/devices.rs`; `examples/buck/src/*.msl`; `crates/piperine-lang/std/convert.msl`
**Requirement**: RS-09
**Executor**: Sonnet

**Done when**:

- [ ] `rise()` is written as `Changed` with the guard `high()`; `above(e)` as `Cross(e)`; no body uses a form outside spec RS-09
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test digital`: `the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `test(ir): the buck-slice bodies, hand-built`

---

### T30: The interpreter refuses digital bodies

**What**: `Interpreter::new` returns `Rejected` naming each body that has a carrier, a drive or a reaction (R1: the interpreter is frozen).
**Where**: `crates/piperine-interp/src/lib.rs`
**Depends on**: T29
**Reuses**: `Rejected`
**Requirement**: RS-09
**Executor**: Sonnet

**Done when**:

- [ ] No other line of `piperine-interp` changes beyond the refusal arms T24 to T27 added
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test frozen`: `a_digital_body_is_rejected_by_name`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): digital bodies are refused by name`

---

## Phase 6 tasks

### T31: Plan: carriers as pins

**What**: Carrier pins after the M1 pins: kind, role, the layout of RS-12 AC 1, `HAS_INIT` with canonical bytes; `Source::Carrier`, `Target::Carrier`.
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: None in this phase (phase 5 complete)
**Reuses**: `plan.rs` (`Pins`); `crates/piperine-abi/src/layout.rs` (`BIT`, `REAL`, `BYTE`)
**Requirement**: RS-12
**Executor**: Opus

**Done when**:

- [ ] Every M1 shape is unchanged: `cargo test -p piperine-interp --test shape` passes untouched
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test digital_shape`: `carriers_are_discrete_pins_with_a_known_byte`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): carriers as discrete pins`

---

### T32: Plan: drives and events

**What**: `Mode::{Drive, Event}`: a drive as a discrete function; a `Changed` event as an event pin and its function reading `VALUE`, `CHANGED` and the guard's pins; a drive with `after` as a private pin, an event and a one-statement reaction.
**Where**: `crates/piperine-plan/src/digital.rs` (modify)
**Depends on**: T31
**Reuses**: T31; abi.md §11.4
**Requirement**: RS-12
**Executor**: Sonnet

**Done when**:

- [ ] The flip-flop's shape equals abi.md §11.4 entry by entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test digital_shape`: `a_drive_is_a_discrete_function`, `an_event_is_an_event_pin_and_its_function`, `a_delayed_drive_is_a_pin_an_event_and_a_reaction`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): drives and events as discrete functions`

---

### T33: Plan: reactions and statements

**What**: `ReactionPlan`: the event pins, the reads (carriers, params, probed nodes), one `StatementPlan` per commit in source order with `LATER` and `min_after` only for a literal delay greater than 0.
**Where**: `crates/piperine-plan/src/digital.rs` (modify)
**Depends on**: T32
**Reuses**: T32; abi.md §5, §6 C1
**Requirement**: RS-12
**Executor**: Opus

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test digital_shape`: `a_reaction_lists_its_events_reads_and_statements`, `only_a_literal_positive_delay_is_later`, `a_probing_reaction_reads_the_continuous_pin`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(plan): reactions and their statements`

---

### T34: Plan: the two crossings

**What**: `Mode::Cross`: a `CROSS` watch, its event pin and the analog function with partials; a carrier read in `analog` as a non-differentiable read of `terms`; every buck-slice shape validated.
**Where**: `crates/piperine-plan/src/digital.rs` (modify)
**Depends on**: T33
**Reuses**: T33; abi.md §11.5, §11.6
**Requirement**: RS-12
**Executor**: Sonnet

**Done when**:

- [ ] The comparator's shape equals abi.md §11.5 entry by entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-plan --test digital_shape`: `a_cross_event_is_a_watch_with_partials`, `an_analog_read_of_a_carrier_is_a_read_of_terms`, `every_buck_slice_shape_validates`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(plan): cross watches and analog reads of carriers`

---

### T35: rt: UInt and Int

**What**: `rt::UInt<const N: u32>` and `rt::Int<const N: u32>` over `u64`/`i64`: `new`, the operators as trait impls masked to `N` bits, comparisons, `min`, `max`, `clamp`, casts among widths and signs, from and to `f64` (saturating, NaN to 0), shifts by any amount, division by zero a panic naming the operation.
**Where**: `crates/piperine-transpile/src/rt/int.rs`
**Depends on**: T34
**Reuses**: `rt/real.rs` (how a runtime file is embedded and tested)
**Requirement**: RS-10
**Executor**: Opus

**Done when**:

- [ ] The wrap test compares every operator with `i128` arithmetic reduced modulo `2^N` for N in {1, 3, 8, 9, 12, 16, 31, 32, 63, 64} at the boundaries and 1000 random pairs
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `rt::int::tests::arithmetic_wraps_at_every_width`, `rt::int::tests::casts_truncate_and_extend_by_the_source_sign`, `rt::int::tests::a_float_cast_saturates_and_nan_is_zero`, `rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): rt UInt and Int`

---

### T36: rt: Flagged and Commits

**What**: `rt::Flagged<T>`: decode from and encode to a pin's bytes (canonical zero when unknown), all-or-nothing operators, the `bool` level rules, `known_or`, `is_known`, `high`, `decide(origin)`; `rt::Commits`: collected commits flushed to `React` in order.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T35
**Reuses**: `crates/piperine-abi/src/layout.rs` (`HIGH`, `LOW`, the known byte)
**Requirement**: RS-11
**Executor**: Sonnet

**Done when**:

- [ ] The level-rule test is the full nine-case table of `&` and `|` and the three cases of `!`, written from §3.3.2
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `rt::flag::tests::an_unknown_operand_gives_an_unknown_result`, `rt::flag::tests::bool_operators_follow_the_level_rules`, `rt::flag::tests::known_or_is_known_and_high`, `rt::flag::tests::an_unknown_value_is_written_as_zero_bytes`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: build

**Commit**: `feat(transpile): rt Flagged and Commits`

---

## Phase 7 tasks

### T37: ValuePrinter: integers and unknown values

**What**: The phase B value forms as Rust over `rt::UInt`, `rt::Int` and `rt::Flagged`; `Error::Unsupported` naming `UInt<N>`/`Int<N>` with `N > 64`, `Float<16>` and `Float<128>`.
**Where**: `crates/piperine-transpile/src/print/value.rs` (modify)
**Depends on**: None in this phase (phase 6 complete)
**Reuses**: T5, T35, T36
**Requirement**: RS-10, RS-11
**Executor**: Sonnet

**Done when**:

- [ ] `rev3` and `dithered_width` print as the texts the test holds, written from `dpwm.msl`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::value::tests::integer_and_flagged_expressions_print`, `print::value::tests::a_type_rust_lacks_is_refused_by_name`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): integer and unknown-value expressions`

---

### T38: DigitalPrinter: drives and events

**What**: The `drive:<carrier>` and `event:<k>` functions as Rust over `Eval`: reads decoded as `Flagged`, the drive's value encoded to its pin, the event byte from `CHANGED` and the guard.
**Where**: `crates/piperine-transpile/src/print/digital.rs`
**Depends on**: T37
**Reuses**: T37; abi.md §11.4 `eval`
**Requirement**: RS-12
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::digital::tests::drives_and_events_print_as_discrete_functions`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): drive and event functions`

---

### T39: DigitalPrinter: reactions

**What**: A reaction as one Rust `fn` over `React`: reads decoded, `let` and `if` reprinted with `decide`, each commit pushed to `rt::Commits` with its statement index, value bytes and delay (an unknown delay commits `Unknown` at delay 0), flushed at the end.
**Where**: `crates/piperine-transpile/src/print/reaction.rs`
**Depends on**: T38
**Reuses**: T36 to T38; abi.md §5, §6
**Requirement**: RS-11, RS-06
**Executor**: Opus

**Done when**:

- [ ] Each test compiles a small hand-built body and calls its reaction through `caller::Handle` with a recording commit closure
- [ ] The panic is an integer division by zero in a drive; the next call on the same instance succeeds
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test reaction`: `a_reaction_commits_in_source_order`, `an_unknown_condition_faults_and_commits_nothing`, `an_unknown_delay_commits_unknown_at_the_next_microstep`, `a_known_or_over_an_unwritten_input_gives_the_default`, `a_panic_in_compiled_code_is_a_fault_and_the_instance_survives`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): reactions and commits`

---

### T40: AnalogPrinter: cross watches and carrier reads

**What**: The `cross:<k>` function (the watch's value and partials over `Dual<N>`) and a carrier read inside `terms_block` (decoded once, passed as a plain value).
**Where**: `crates/piperine-transpile/src/print/cross.rs`
**Depends on**: T39
**Reuses**: T7; abi.md §11.5, §11.6
**Requirement**: RS-12
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --lib`: `print::cross::tests::a_cross_function_prints_its_watch_and_partials`, `print::cross::tests::terms_reads_a_carrier_as_a_plain_value`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): cross watches and analog reads of carriers`

---

### T41: Netlist: discrete nets

**What**: `Netlist::circuit` binds each carrier port to its design net with `Port::Net` and sets the session resolution; an instance's carrier params and initial values reach their pins.
**Where**: `crates/piperine-plan/src/netlist.rs`
**Depends on**: T40
**Reuses**: `crates/piperine-solver/src/core/{builder,bind}.rs` (`Port::Net`, `resolution`)
**Requirement**: RS-05, RS-12
**Executor**: Sonnet

**Done when**:

- [ ] The first test runs the oscillator into the flip-flop and reads `q` by its net name
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test digital_circuit`: `carrier_ports_bind_to_design_nets`, `every_buck_slice_body_compiles_with_no_warning`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(plan): discrete nets in the netlist`

---

## Phase 8 tasks

### T42: Commit semantics on the solver

**What**: Four small compiled circuits, each the example of its rule in `docs/spec/part_iv_behavior.md` §12.1: the exchange, the later statement, the pulse, the unchanged commit.
**Where**: `crates/piperine-transpile/tests/commits.rs`
**Depends on**: None in this phase (phase 7 complete)
**Reuses**: T41; `crates/piperine-solver/tests/` (how a discrete net is read and an event awaited)
**Requirement**: RS-13
**Executor**: Sonnet

**Done when**:

- [ ] Expected instants and values are written from §12.1.1, .2, .4 and .18, each cited at its assertion
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test commits`: `two_commits_exchange_two_carriers`, `the_later_statement_wins`, `two_statements_make_a_pulse`, `an_unchanged_commit_fires_nothing`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): commit semantics of compiled reactions`

---

### T43: The oscillator and the DPWM

**What**: The compiled `Osc` alone for 100 ns, then into `Dpwm` with `run` true and duty word 1325 for 8 periods; the determinism run.
**Where**: `crates/piperine-transpile/tests/dpwm.rs`
**Depends on**: T42
**Reuses**: T29's fixtures; buck SPEC §5, §9
**Requirement**: RS-13
**Executor**: Sonnet

**Done when**:

- [ ] The expected widths (166 in the five periods whose bit-reversed frame index is below 5, else 165), the 4-count dead times and the 396 limit are computed in the test from SPEC §5, not read back from the model
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test dpwm`: `the_oscillator_toggles_every_half_period`, `the_dpwm_dithers_keeps_dead_time_and_never_overlaps`, `two_runs_give_the_same_changes`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): the oscillator and the DPWM behave as specified`

---

### T44: The reference ramp and the PID

**What**: The compiled `RefRamp` and `Pid` under a scripted sequence of 200 samples (soft-start, a step of the measurement, saturation both ways, `run` dropped and raised), against a reference written in the test from the difference equations of SPEC §6.2 to §6.4 and §7.2 in plain `i32` arithmetic.
**Where**: `crates/piperine-transpile/tests/pid.rs`
**Depends on**: T43
**Reuses**: T29's fixtures; buck SPEC §6, §7
**Requirement**: RS-13
**Executor**: Sonnet

**Done when**:

- [ ] Every committed `duty`, `integ`, `deriv`, `e_prev` and `reference` is compared, bit for bit, at every sample
- [ ] Tests with exactly these names pass in `cargo test -p piperine-transpile --test pid`: `the_ramp_and_the_pid_follow_their_difference_equations`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(transpile): the ramp and the PID match their equations`

---

### T45: Analog to digital

**What**: The compiled `Adc` on a held node at five voltages (0, one LSB, mid-scale, full scale, above full scale) and the compiled `Comparator` on an RC charge and discharge.
**Where**: `crates/piperine-parity/tests/boundary.rs`
**Depends on**: T44
**Reuses**: `Bench`-free: circuits from `Compiled::circuit`; `ToleranceContract::TRAN_VOLTAGE`; buck SPEC §4.1
**Requirement**: RS-13
**Executor**: Sonnet

**Done when**:

- [ ] The comparator's trip times are judged by the analytic RC voltage at each time against the threshold under `TRAN_VOLTAGE`; no time tolerance is written
- [ ] A solver defect on this path stops the task and is reported with a hand-written ABI device that reproduces it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test boundary`: `the_adc_commits_the_rounded_code_after_tconv`, `the_comparator_trips_at_its_thresholds`
- [ ] Gate check passes: the task's `Run` line, then `cargo test -p <the crate the task edits>`
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): compiled reactions sample and detect crossings`

---

### T46: Digital to analog

**What**: The compiled `Switch` between a source and an RC, its gate committed high at `t0` by a compiled reaction: `t0` is a time point and the response from `t0` is the analytic one.
**Where**: `crates/piperine-parity/tests/boundary.rs` (modify)
**Depends on**: T45
**Reuses**: T45; abi.md R4, §11.6
**Requirement**: RS-13
**Executor**: Sonnet

**Done when**:

- [ ] The node voltage is compared with the analytic response at every accepted time after `t0` under `TRAN_VOLTAGE`; a record exists at `t0` itself
- [ ] Tests with exactly these names pass in `cargo test -p piperine-parity --test boundary`: `a_carrier_change_is_a_time_point_and_starts_the_response`
- [ ] Gate check passes: Full, then the `**Build**` commands, `cargo test -p piperine-interp`, `cargo test -p piperine-lang --test suite_hygiene`, `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `test(parity): a carrier change starts the analog response`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`; a criterion with two observable halves has two. A test function has exactly the name its check cites.

**C1** - Supported `Ir` with an empty `check()`: one source text, `rustc` exit 0, standard error empty, for each of the five M1 bodies. (RS-01, AC 1; T16)
Proof: run compile | test the_m1_design_compiles_with_no_error_and_no_warning ... ok

**C2** - Each IR function is one `fn` generic over `Real`; each body that runs is one `mod` with a `Device` impl. (RS-01, AC 2; T10)
Proof: run golden | test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok

**C3** - Two transpilations of one `Ir` give byte-identical source. (RS-01, AC 3; T10)
Proof: run golden | test transpiling_twice_gives_the_same_bytes ... ok

**C4** - A form with no lowering gives no source and an error with the form, its IR path and its origin. (RS-01, AC 4; T5)
Proof: run lib | test print::value::tests::an_unsupported_form_is_named_with_path_and_origin ... ok

**C5** - An `Ir` that fails `check()` gives no source and the check's entries. (RS-01, AC 5; T2)
Proof: run reject | test an_ir_that_fails_check_gives_no_source ... ok

**C6** - Every `mod`, `fn`, shape entry and function arm has, on the line before it, a comment with its IR path and `file:line:col`. (RS-02, AC 1; T10)
Proof: run golden | test every_item_sits_under_its_path_and_origin ... ok

**C7** - Items carry their MSL identifiers; a Rust keyword is a raw identifier. (RS-02, AC 2; T4)
Proof: run lib | test print::text::tests::keywords_become_raw_identifiers ... ok

**C8** - 4-space indentation, one statement per line, reals in the shortest decimal that parses back to the same bits. (RS-02, AC 3; T4)
Proof: run lib | test print::text::tests::reals_round_trip_in_the_shortest_form ... ok

**C9** - A build keeps `design.rs` in the cache entry beside the library and reports its path. (RS-02, AC 4; T16)
Proof: run compile | test the_source_is_kept_beside_the_library_and_reported ... ok

**C10** - The five M1 bodies print as the golden files. (RS-02, AC 5; T10)
Proof: run golden | test the_five_m1_bodies_print_as_the_golden_files ... ok

**C11** - The compiler is `PIPERINE_RUSTC`, else `rustc` on `PATH`. (RS-03, AC 1; T11)
Proof: run lib | test settings::tests::the_compiler_is_the_variable_then_the_path ... ok

**C12** - A build runs `rustc` directly and never `cargo`. (RS-03, AC 1; T12)
Proof: run rustc | test the_compiler_is_run_directly_and_cargo_never ... ok

**C13** - No compiler: an error naming `rustc` and saying the interpreter needs none, before any analysis. (RS-03, AC 2; T12)
Proof: run rustc | test a_missing_compiler_is_named_and_the_interpreter_is_offered ... ok

**C14** - A failing compiler: an error with its standard error and the path of the source, which is still on disk. (RS-03, AC 3; T16)
Proof: run compile | test a_compile_error_keeps_the_source_and_shows_stderr ... ok

**C15** - A compiler below 1.85: an error with the version found and 1.85. (RS-03, AC 4; T12)
Proof: run rustc | test a_compiler_below_1_85_is_refused_with_both_versions ... ok

**C16** - A second build of an unchanged design invokes the compiler 0 times and loads the cached library. (RS-04, AC 1; T16)
Proof: run compile | test a_second_build_runs_no_compiler ... ok

**C17** - An `Ir` change that changes the source gives another entry. (RS-04, AC 2; T16)
Proof: run compile | test a_changed_ir_is_another_entry ... ok

**C18** - A different `rustc -vV` gives another entry. (RS-04, AC 3; T16)
Proof: run compile | test another_compiler_version_is_another_entry ... ok

**C19** - Eight builds racing on an empty cache all complete and leave one complete entry. (RS-04, AC 4; T14)
Proof: run cache | test two_racing_builds_leave_one_entry_and_no_staging ... ok

**C20** - A cached library whose bytes changed is not loaded. (RS-04, AC 5; T15)
Proof: run load | test a_library_whose_bytes_changed_is_not_loaded ... ok

**C21** - A corrupted entry is rebuilt and the build succeeds. (RS-04, AC 5; T16)
Proof: run compile | test a_corrupted_library_is_rebuilt ... ok

**C22** - `Dc`, `Ac` and a transient invoke the compiler 0 times. (RS-04, AC 6; T20)
Proof: run session | test no_analysis_runs_the_compiler ... ok

**C23** - A `set` of a non-structural param between analyses is applied with 0 compiler invocations. (RS-04, AC 7; T20)
Proof: run session | test a_set_between_analyses_runs_no_compiler_and_applies ... ok

**C24** - A cache directory that cannot be written: an error naming it. (RS-04, AC 8; T14)
Proof: run cache | test an_unwritable_directory_is_named ... ok

**C25** - The cache directory follows the four rules in order. (RS-04, AC 9; T11)
Proof: run lib | test settings::tests::the_cache_directory_follows_the_four_rules ... ok

**C26** - Modules are read through `piperine_abi_1` only and bound with `CircuitBuilder::device` as the interpreter's are. (RS-05, AC 1; T17)
Proof: run compiled | test modules_come_through_the_symbol_and_bind_like_the_interpreter ... ok

**C27** - Every loaded shape equals the plan's shape entry by entry, for the five M1 bodies. (RS-05, AC 2; T17)
Proof: run compiled | test loaded_shapes_equal_the_plans_entry_by_entry ... ok

**C28** - One `BodyPlan` type serves both backends. (RS-05, AC 2; T1)
Proof: run plan | test the_interpreter_and_the_plan_crate_share_one_body_plan ... ok

**C29** - A file that is not a library: an error naming the file. (RS-05, AC 3; T15)
Proof: run load | test a_file_that_is_no_library_is_named ... ok

**C30** - A library without the symbol: an error naming the file. (RS-05, AC 3; T15)
Proof: run load | test a_library_without_the_symbol_is_named ... ok

**C31** - A library of a newer minor: an error naming the file. (RS-05, AC 3; T15)
Proof: run load | test a_library_of_a_newer_minor_is_refused ... ok

**C32** - `unsafe` appears in `src/load.rs` and in no other source of the crate nor in a golden file, except the one export attribute `#[unsafe(no_mangle)]` on `piperine_abi_1` in generated source and goldens (orchestrator ruling, 2026-11-15: RS-05 AC 4 means no `unsafe` block or `unsafe fn` outside `load.rs`). (RS-05, AC 4; T15)
Proof: run load | test unsafe_lives_in_load_only ... ok

**C33** - Every function of the five M1 bodies: output cells with equal bits and partials equal as `f64` on the two backends, the six `AGE`/`STATIC` cases included. (RS-06, AC 1; T18)
Proof: run differential | test every_m1_function_gives_the_interpreters_bytes ... ok

**C34** - `rt::Dual` gives the interpreter's values and tangents for every operator and method. (RS-06, AC 1; T3)
Proof: run lib | test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok

**C35** - Every compiled function passes `verify_partials` at `rel_tol` 1e-6. (RS-06, AC 2; T18)
Proof: run differential | test every_compiled_function_passes_verify_partials ... ok

**C36** - The compiled resistor's value has the bits of `g*vp - g*vn` and a right side of exactly 0 over 1000 points. (RS-06, AC 3; T19)
Proof: run residue | test the_compiled_resistor_matches_the_hand_written_products_bit_for_bit ... ok

**C37** - An affine term is printed as `c + (0.0 + g0 * x0 + …)` in declared partial order. (RS-06, AC 3; T7)
Proof: run lib | test print::analog::tests::an_affine_term_is_printed_as_constant_plus_fold ... ok

**C38** - An affine constant `c ≠ 0` leaves a right side within 1 ulp of `max(|c|, |Σ|)` of `−c` over 2000 points. (RS-06, AC 4; T19)
Proof: run residue | test a_compiled_affine_constant_leaves_at_most_one_ulp ... ok

**C39** - Divider, diode series, AC RC and transient RC agree on the two backends under the contract of each domain. (RS-06, AC 5; T23)
Proof: run backends | test both_backends_agree_on_the_m1_circuits ... ok

**C40** - A panic in compiled code is a `FAULT` with the message; the next call on the instance succeeds. (RS-06, AC 6; T39)
Proof: run reaction | test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok

**C41** - A non-finite value is written as it is. (RS-06, AC 7; T18)
Proof: run differential | test a_non_finite_value_is_written_as_is ... ok

**C42** - `divider` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test divider ... ok

**C43** - `diode_series` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test diode_series ... ok

**C44** - `isrc_r` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test isrc_r ... ok

**C45** - `diode_iv` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test diode_iv ... ok

**C46** - `diode_rev` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test diode_rev ... ok

**C47** - `ac_rc` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test ac_rc ... ok

**C48** - `tran_rc` agrees with ngspice on both backends under its contract. (RS-07, AC 1 and 2; T22)
Proof: run parity | test tran_rc ... ok

**C49** - A rejection names the backend. (RS-07, AC 2; T21)
Proof: run backend | test a_rejection_names_the_backend ... ok

**C50** - The `Rust` bench runs modules of a loaded library, not the interpreter's. (RS-07, AC 1; T21)
Proof: run backend | test the_rust_bench_runs_loaded_modules ... ok

**C51** - The five contracts hold their M1 bounds. (RS-07, AC 3; T22)
Proof: run contract | test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok

**C52** - `PIPERINE_BACKEND` = `interp` or `rust` selects that backend only. (RS-07, AC 4; T21)
Proof: run backend | test the_variable_selects_one_backend ... ok

**C53** - Any other value is refused naming `interp` and `rust`. (RS-07, AC 5; T21)
Proof: run backend | test an_unknown_backend_is_refused_naming_both ... ok

**C54** - A build reports transpile, compile and load durations, the compiler invocations and the cache hit. (RS-08, AC 1; T16)
Proof: run compile | test a_build_reports_its_cost ... ok

**C55** - Integer types 1..=64, literals, arithmetic, comparisons, bit operators, shifts, casts and `min`/`max`/`clamp`/`floor` are IR nodes that print. (RS-09, AC 1; T24)
Proof: run irdigital | test integers_and_their_operators_print ... ok

**C56** - A flagged type, `Unknown`, `known_or`, `is_known` and `high` are IR nodes that print. (RS-09, AC 2; T25)
Proof: run irdigital | test unknown_values_and_their_reads_print ... ok

**C57** - Carriers and drives (with `after`) are body items that print. (RS-09, AC 3; T26)
Proof: run irdigital | test carriers_and_drives_print ... ok

**C58** - Events (`changed`, `cross`, guard) and reactions with `let`, `if` and commits are body items that print. (RS-09, AC 3; T27)
Proof: run irdigital | test events_and_reactions_print ... ok

**C59** - Each defect of a digital body is one `check()` entry with item and origin. (RS-09, AC 4; T28)
Proof: run irdigital | test a_defective_digital_body_gives_one_entry_per_defect ... ok

**C60** - The printed IR of every M1 fixture is byte-identical to its golden text. (RS-09, AC 5; T28)
Proof: run irdigital | test the_m1_prints_are_unchanged ... ok

**C61** - The ten buck-slice bodies pass `check()` and print as their golden texts. (RS-09, AC 3; T29)
Proof: run irdigital | test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

**C62** - `Interpreter::new` returns `Rejected` naming a body with a carrier, a drive or a reaction. (RS-09, AC 6; T30)
Proof: run frozen | test a_digital_body_is_rejected_by_name ... ok

**C63** - `+ - * / %` and unary `-` equal `i128` arithmetic modulo `2^N` at ten widths. (RS-10, AC 1; T35)
Proof: run lib | test rt::int::tests::arithmetic_wraps_at_every_width ... ok

**C64** - Integer casts truncate to the target and extend by the source's sign. (RS-10, AC 2; T35)
Proof: run lib | test rt::int::tests::casts_truncate_and_extend_by_the_source_sign ... ok

**C65** - A cast from `f64` saturates to the `N`-bit range; NaN gives 0. (RS-10, AC 3; T35)
Proof: run lib | test rt::int::tests::a_float_cast_saturates_and_nan_is_zero ... ok

**C66** - A shift by `N` or more is the mathematical result truncated to `N` bits. (RS-10, AC 4; T35)
Proof: run lib | test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok

**C67** - `UInt<65>`, `Float<16>` and `Float<128>` give no source and an error naming the type and its path. (RS-10, AC 5; T37)
Proof: run lib | test print::value::tests::a_type_rust_lacks_is_refused_by_name ... ok

**C68** - Arithmetic, a comparison or a cast with an unknown operand is unknown. (RS-11, AC 1; T36)
Proof: run lib | test rt::flag::tests::an_unknown_operand_gives_an_unknown_result ... ok

**C69** - `&`, `|`, `!` on `bool` follow the nine-case and three-case tables of §3.3.2. (RS-11, AC 2; T36)
Proof: run lib | test rt::flag::tests::bool_operators_follow_the_level_rules ... ok

**C70** - `known_or`, `is_known` and `high` give the value or default, the flag, and known-and-true. (RS-11, AC 3; T36)
Proof: run lib | test rt::flag::tests::known_or_is_known_and_high ... ok

**C71** - A compiled reaction over an unwritten input commits `known_or(7)` as 7. (RS-11, AC 3; T39)
Proof: run reaction | test a_known_or_over_an_unwritten_input_gives_the_default ... ok

**C72** - An unknown value is written with known byte 0 and value bytes 0. (RS-11, AC 4; T36)
Proof: run lib | test rt::flag::tests::an_unknown_value_is_written_as_zero_bytes ... ok

**C73** - An unknown `if` condition is a fault naming the `if`'s origin, with 0 commits made. (RS-11, AC 5; T39)
Proof: run reaction | test an_unknown_condition_faults_and_commits_nothing ... ok

**C74** - An unknown delay commits `Unknown` at the next microstep. (RS-11, AC 6; T39)
Proof: run reaction | test an_unknown_delay_commits_unknown_at_the_next_microstep ... ok

**C75** - Each carrier is a `DISCRETE` pin with role, layout, known byte and `HAS_INIT` as stated. (RS-12, AC 1; T31)
Proof: run planshape | test carriers_are_discrete_pins_with_a_known_byte ... ok

**C76** - A drive without `after` is one discrete function writing the carrier's pin. (RS-12, AC 2; T32)
Proof: run planshape | test a_drive_is_a_discrete_function ... ok

**C77** - An event is a `PRIVATE` `EVENT` pin written by one function reading `VALUE`, `CHANGED` and the guard's pins. (RS-12, AC 3; T32)
Proof: run planshape | test an_event_is_an_event_pin_and_its_function ... ok

**C78** - A reaction lists its event pins, its reads and one statement per commit in source order. (RS-12, AC 4; T33)
Proof: run planshape | test a_reaction_lists_its_events_reads_and_statements ... ok

**C79** - `LATER` with `min_after` is set for a literal delay greater than 0 and for nothing else. (RS-12, AC 4; T33)
Proof: run planshape | test only_a_literal_positive_delay_is_later ... ok

**C80** - A drive with `after` is a private pin, a `changed` event and a one-statement reaction. (RS-12, AC 5; T32)
Proof: run planshape | test a_delayed_drive_is_a_pin_an_event_and_a_reaction ... ok

**C81** - A `cross` event is a `CROSS` watch firing its event pin, with a partial per continuous read. (RS-12, AC 6; T34)
Proof: run planshape | test a_cross_event_is_a_watch_with_partials ... ok

**C82** - A carrier read in `analog` is a read of `terms`. (RS-12, AC 7; T34)
Proof: run planshape | test an_analog_read_of_a_carrier_is_a_read_of_terms ... ok

**C83** - A probing reaction reads the probed continuous pins. (RS-12, AC 8; T33)
Proof: run planshape | test a_probing_reaction_reads_the_continuous_pin ... ok

**C84** - Every buck-slice shape passes `piperine_abi::validate`. (RS-12, AC 9; T34)
Proof: run planshape | test every_buck_slice_shape_validates ... ok

**C85** - `clk` of the 200 MHz oscillator changes at `k × 2.5 ns`, `k` = 1 to 40, and at no other time in 100 ns. (RS-13, AC 1; T43)
Proof: run dpwm | test the_oscillator_toggles_every_half_period ... ok

**C86** - `a <- b; b <- a;` exchanges the two values at the next microstep. (RS-13, AC 2; T42)
Proof: run commits | test two_commits_exchange_two_carriers ... ok

**C87** - Two statements to one carrier in one microstep: the later statement's value stays. (RS-13, AC 3; T42)
Proof: run commits | test the_later_statement_wins ... ok

**C88** - `p <- true after t; p <- false after t + w;` holds `p` true from `t` to `t + w`. (RS-13, AC 4; T42)
Proof: run commits | test two_statements_make_a_pulse ... ok

**C89** - A commit of the value a carrier already holds fires no `changed`. (RS-13, AC 5; T42)
Proof: run commits | test an_unchanged_commit_fires_nothing ... ok

**C90** - Duty word 1325 over 8 periods: `hs` 166 counts in five periods and 165 in three, in bit-reversed order; `ls` from 4 counts after `hs` falls to count 396; never both. (RS-13, AC 6; T43)
Proof: run dpwm | test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok

**C91** - `RefRamp` and `Pid` commit, at each of 200 samples, the values of the SPEC's difference equations, bit for bit. (RS-13, AC 7; T44)
Proof: run pid | test the_ramp_and_the_pid_follow_their_difference_equations ... ok

**C92** - The ADC commits `floor(V / vref × 1024 + 0.5)` clamped to 0..=1023 after `tconv`, `done` true for `tstrobe`, at five voltages. (RS-13, AC 8; T45)
Proof: run boundary | test the_adc_commits_the_rounded_code_after_tconv ... ok

**C93** - The comparator's output changes where the analytic input equals `vth ± hyst / 2` under `TRAN_VOLTAGE`. (RS-13, AC 9; T45)
Proof: run boundary | test the_comparator_trips_at_its_thresholds ... ok

**C94** - A carrier change at `t0` is a time point and the response from `t0` is the analytic one under `TRAN_VOLTAGE`. (RS-13, AC 10; T46)
Proof: run boundary | test a_carrier_change_is_a_time_point_and_starts_the_response ... ok

**C95** - Two runs of the oscillator and DPWM circuit give the same pin changes, bit for bit and tick for tick. (RS-13, AC 11; T43)
Proof: run dpwm | test two_runs_give_the_same_changes ... ok

**C96** - The embedded ABI sources are the files of `crates/piperine-abi/src`, byte for byte. (RS-05, AC 1; T13)
Proof: run abisrc | test the_embedded_sources_are_the_abi_crate ... ok

**C97** - The ABI crate builds as an `rlib` with `rustc` alone. (RS-03, AC 1; T13)
Proof: run abisrc | test the_abi_builds_with_rustc_alone ... ok

**C98** - Carrier ports bind to design nets: the oscillator drives the flip-flop and `q` is read by its net name. (RS-12, AC 1; T41)
Proof: run nets | test carrier_ports_bind_to_design_nets ... ok

**C99** - Each of the ten buck-slice bodies compiles with `rustc` standard error empty. (RS-01, AC 1; T41)
Proof: run nets | test every_buck_slice_body_compiles_with_no_warning ... ok

**C100** - A design with no body that runs builds a library with 0 modules. (edge case 1; T10)
Proof: run golden | test a_design_with_nothing_to_run_has_no_module ... ok

**C101** - A body without `analog` generates no `terms` function. (edge case 2; T10)
Proof: run golden | test a_body_without_analog_prints_no_terms_function ... ok

**C102** - After a race the cache holds no staging directory. (edge case 3; T14)
Proof: run cache | test two_racing_builds_leave_one_entry_and_no_staging ... ok

**C103** - A keyword identifier is raw in Rust and unchanged as the ABI name. (edge case 4; T10)
Proof: run golden | test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 - the dispatch said "do not edit solver crates"; T1 and T2's Done-when require `DEFAULT_MEMBERS` (which lives in `crates/piperine-solver/tests/workspace_shape.rs`) to gain the new crate, or the guard fails. The one-line list entry is made there, as `ir-interp` T2 did; no solver source is touched. T2 likewise adds `piperine-plan` and `piperine-transpile` to that file's `KEPT_CRATES`, which `instructions_list_the_kept_crates` compares with the crate rows T2 adds to `CLAUDE.md` and `AGENTS.md`. T4 - RS-02 AC 2 says a keyword prints as a raw identifier, but Rust refuses `r#crate`, `r#self`, `r#Self` and `r#super`; `Text::ident` prints those four as `crate_`, `self_`, `Self_`, `super_` (the ABI name stays unchanged). T10 - exporting `piperine_abi_1` from an edition-2024 crate takes the attribute `#[unsafe(no_mangle)]` (RS-Q2 needs the symbol, RS-Q4 the edition); it is no `unsafe` block, and every unsafe operation stays in the ABI crate's thunks. C32's scan (T15, `unsafe_lives_in_load_only`) has to allow that one attribute line in generated source and the goldens, or RS-05 AC 4 reads as `unsafe` blocks only. T10 - the goldens were read MSL line to Rust line by the Opus batch worker that wrote the printer (no second Opus was available in the batch), and each of the five, with three more designs (keywords, RcHeld, DiodeCircuit), was compiled with rustc 1.98 `--edition 2024 --crate-type cdylib` against an rlib of `piperine-abi`: no error, no warning. T12 - `Rustc::invocations()` counts builds (`rlib`, `cdylib`) only, not the `-vV` probe; `cdylib` and `rlib` return the compiler's standard error on success. T14 - `Cache` gains `abi` (the ABI `rlib` entry, same staging-and-rename) and `remove` (a failed hash rebuilds once); `Cache::build` keeps its staging directory when the build closure fails, so `CompileFailed` can name the source on disk. T15 - the hash is checked before `dlopen`, on a cache directory created `0700`; a mapped library must never be rewritten in place (a test that did crashed the process), so a rebuild unlinks and writes a new file. T16 - `Report` gains `warnings` (the compiler's standard error of the build, empty on a hit) so the zero-warning check reads it from `compile`. T17 - the shape check runs inside `compile` (`Compiled::bind`), not in `circuit()`, so a mismatched library is refused before it is handed out; `piperine-transpile` gains the `piperine-solver` dependency for `CircuitInstance`; the task text says the divider gives `mid` half of `vin`, but the shared fixture is 10 V across 1 k and 3 k, so the test asserts the divider ratio (7.5 V) to 1e-12 relative and equality with the interpreter's value.
**Dispatched:** batch 1 (phases 1-2, T1-T10) - Opus worker, 2026-11-15, done. Batch 2 (phase 3, T11-T17) - Sonnet worker, 2026-11-15, done. C32 ruling: the export attribute is allowed (see C32).
**Walked:** none (no screen; each story's Independent Test is a check above)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4
Phase 2:  T5 ------→ T6 ------→ T7 ------→ T8 ------→ T9 ------→ T10
Phase 3:  T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16 ------→ T17
Phase 4:  T18 ------→ T19 ------→ T20 ------→ T21 ------→ T22 ------→ T23
Phase 5:  T24 ------→ T25 ------→ T26 ------→ T27 ------→ T28 ------→ T29 ------→ T30
Phase 6:  T31 ------→ T32 ------→ T33 ------→ T34 ------→ T35 ------→ T36
Phase 7:  T37 ------→ T38 ------→ T39 ------→ T40 ------→ T41
Phase 8:  T42 ------→ T43 ------→ T44 ------→ T45 ------→ T46
```

Execution is strictly sequential. Phases 1 to 4 (phase A) may be merged before phase 5 starts.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: Move the plan and the netlist to piperine-plan | 1 move (two files to one crate), no logic change | ✅ Granular |
| T2: The crate and Transpiler::new | 1 component | ✅ Granular |
| T3: rt: Real and Dual | 1 component | ✅ Granular |
| T4: Text: the source writer | 1 component | ✅ Granular |
| T5: ValuePrinter: expressions and functions | 1 component | ✅ Granular |
| T6: ShapePrinter | 1 component | ✅ Granular |
| T7: AnalogPrinter: terms and the affine rule | 1 component | ✅ Granular |
| T8: AnalogPrinter: limit | 1 component | ✅ Granular |
| T9: Site printers: stim and check | 1 component | ✅ Granular |
| T10: LibraryPrinter and the golden sources | 1 component | ✅ Granular |
| T11: Settings | 1 component | ✅ Granular |
| T12: Rustc | 1 component | ✅ Granular |
| T13: AbiSource | 1 component | ✅ Granular |
| T14: Cache | 1 component | ✅ Granular |
| T15: Loaded | 1 component | ✅ Granular |
| T16: Transpiler::compile | 1 component | ✅ Granular |
| T17: Compiled: modules and the circuit | 1 component | ✅ Granular |
| T18: Function by function against the interpreter | 1 test target | ✅ Granular |
| T19: The linear-term residue on compiled code | 1 test target | ✅ Granular |
| T20: No compiler during a session | 1 test target | ✅ Granular |
| T21: Backend in piperine-parity | 1 component | ✅ Granular |
| T22: Every parity test on both backends | 1 loop applied to four sibling targets | ✅ Granular |
| T23: The M1 circuits on both backends | 1 test target | ✅ Granular |
| T24: IR: integer types and operators | 1 component | ✅ Granular |
| T25: IR: unknown values | 1 component | ✅ Granular |
| T26: IR: carriers, drives and nets | 1 component | ✅ Granular |
| T27: IR: events, reactions and commits | 1 component | ✅ Granular |
| T28: IR: check() for the digital forms | 1 component | ✅ Granular |
| T29: The buck-slice fixtures | 1 component | ✅ Granular |
| T30: The interpreter refuses digital bodies | 1 component | ✅ Granular |
| T31: Plan: carriers as pins | 1 component | ✅ Granular |
| T32: Plan: drives and events | 1 component | ✅ Granular |
| T33: Plan: reactions and statements | 1 component | ✅ Granular |
| T34: Plan: the two crossings | 1 component | ✅ Granular |
| T35: rt: UInt and Int | 1 component | ✅ Granular |
| T36: rt: Flagged and Commits | 1 component | ✅ Granular |
| T37: ValuePrinter: integers and unknown values | 1 component | ✅ Granular |
| T38: DigitalPrinter: drives and events | 1 component | ✅ Granular |
| T39: DigitalPrinter: reactions | 1 component | ✅ Granular |
| T40: AnalogPrinter: cross watches and carrier reads | 1 component | ✅ Granular |
| T41: Netlist: discrete nets | 1 component | ✅ Granular |
| T42: Commit semantics on the solver | 1 test target | ✅ Granular |
| T43: The oscillator and the DPWM | 1 test target | ✅ Granular |
| T44: The reference ramp and the PID | 1 test target | ✅ Granular |
| T45: Analog to digital | 1 test target | ✅ Granular |
| T46: Digital to analog | 1 test target | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None in its phase | None in its phase | ✅ Match |
| T2 | T1 | T1 | ✅ Match |
| T3 | T2 | T2 | ✅ Match |
| T4 | T3 | T3 | ✅ Match |
| T5 | None in its phase | None in its phase | ✅ Match |
| T6 | T5 | T5 | ✅ Match |
| T7 | T6 | T6 | ✅ Match |
| T8 | T7 | T7 | ✅ Match |
| T9 | T8 | T8 | ✅ Match |
| T10 | T9 | T9 | ✅ Match |
| T11 | None in its phase | None in its phase | ✅ Match |
| T12 | T11 | T11 | ✅ Match |
| T13 | T12 | T12 | ✅ Match |
| T14 | T13 | T13 | ✅ Match |
| T15 | T14 | T14 | ✅ Match |
| T16 | T15 | T15 | ✅ Match |
| T17 | T16 | T16 | ✅ Match |
| T18 | None in its phase | None in its phase | ✅ Match |
| T19 | T18 | T18 | ✅ Match |
| T20 | T19 | T19 | ✅ Match |
| T21 | T20 | T20 | ✅ Match |
| T22 | T21 | T21 | ✅ Match |
| T23 | T22 | T22 | ✅ Match |
| T24 | None in its phase | None in its phase | ✅ Match |
| T25 | T24 | T24 | ✅ Match |
| T26 | T25 | T25 | ✅ Match |
| T27 | T26 | T26 | ✅ Match |
| T28 | T27 | T27 | ✅ Match |
| T29 | T28 | T28 | ✅ Match |
| T30 | T29 | T29 | ✅ Match |
| T31 | None in its phase | None in its phase | ✅ Match |
| T32 | T31 | T31 | ✅ Match |
| T33 | T32 | T32 | ✅ Match |
| T34 | T33 | T33 | ✅ Match |
| T35 | T34 | T34 | ✅ Match |
| T36 | T35 | T35 | ✅ Match |
| T37 | None in its phase | None in its phase | ✅ Match |
| T38 | T37 | T37 | ✅ Match |
| T39 | T38 | T38 | ✅ Match |
| T40 | T39 | T39 | ✅ Match |
| T41 | T40 | T40 | ✅ Match |
| T42 | None in its phase | None in its phase | ✅ Match |
| T43 | T42 | T42 | ✅ Match |
| T44 | T43 | T43 | ✅ Match |
| T45 | T44 | T44 | ✅ Match |
| T46 | T45 | T45 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: Move the plan and the netlist to piperine-plan | `piperine-plan` | unit | unit | ✅ OK |
| T2: The crate and Transpiler::new | the printer and the runtime source | unit | unit | ✅ OK |
| T3: rt: Real and Dual | the printer and the runtime source | unit | unit | ✅ OK |
| T4: Text: the source writer | the printer and the runtime source | unit | unit | ✅ OK |
| T5: ValuePrinter: expressions and functions | the printer and the runtime source | unit | unit | ✅ OK |
| T6: ShapePrinter | the printer and the runtime source | unit | unit | ✅ OK |
| T7: AnalogPrinter: terms and the affine rule | the printer and the runtime source | unit | unit | ✅ OK |
| T8: AnalogPrinter: limit | the printer and the runtime source | unit | unit | ✅ OK |
| T9: Site printers: stim and check | the printer and the runtime source | unit | unit | ✅ OK |
| T10: LibraryPrinter and the golden sources | the printer and the runtime source | unit | unit | ✅ OK |
| T11: Settings | settings (pure) | unit | unit | ✅ OK |
| T12: Rustc | compiler, ABI sources, cache, loader | integration | integration | ✅ OK |
| T13: AbiSource | compiler, ABI sources, cache, loader | integration | integration | ✅ OK |
| T14: Cache | compiler, ABI sources, cache, loader | integration | integration | ✅ OK |
| T15: Loaded | compiler, ABI sources, cache, loader | integration | integration | ✅ OK |
| T16: Transpiler::compile | `Transpiler::compile`, `Compiled` | integration | integration | ✅ OK |
| T17: Compiled: modules and the circuit | `Transpiler::compile`, `Compiled` | integration | integration | ✅ OK |
| T18: Function by function against the interpreter | compiled analog functions | integration | integration | ✅ OK |
| T19: The linear-term residue on compiled code | compiled analog functions | integration | integration | ✅ OK |
| T20: No compiler during a session | `Transpiler::compile`, `Compiled` | integration | integration | ✅ OK |
| T21: Backend in piperine-parity | parity on either backend | integration | integration | ✅ OK |
| T22: Every parity test on both backends | parity on either backend | integration | integration | ✅ OK |
| T23: The M1 circuits on both backends | parity on either backend | integration | integration | ✅ OK |
| T24: IR: integer types and operators | the IR's digital node set | unit | unit | ✅ OK |
| T25: IR: unknown values | the IR's digital node set | unit | unit | ✅ OK |
| T26: IR: carriers, drives and nets | the IR's digital node set | unit | unit | ✅ OK |
| T27: IR: events, reactions and commits | the IR's digital node set | unit | unit | ✅ OK |
| T28: IR: check() for the digital forms | the IR's digital node set | unit | unit | ✅ OK |
| T29: The buck-slice fixtures | the IR's digital node set | unit | unit | ✅ OK |
| T30: The interpreter refuses digital bodies | the interpreter's refusal | unit | unit | ✅ OK |
| T31: Plan: carriers as pins | `piperine-plan` | unit | unit | ✅ OK |
| T32: Plan: drives and events | `piperine-plan` | unit | unit | ✅ OK |
| T33: Plan: reactions and statements | `piperine-plan` | unit | unit | ✅ OK |
| T34: Plan: the two crossings | `piperine-plan` | unit | unit | ✅ OK |
| T35: rt: UInt and Int | the printer and the runtime source | unit | unit | ✅ OK |
| T36: rt: Flagged and Commits | the printer and the runtime source | unit | unit | ✅ OK |
| T37: ValuePrinter: integers and unknown values | the printer and the runtime source | unit | unit | ✅ OK |
| T38: DigitalPrinter: drives and events | the printer and the runtime source | unit | unit | ✅ OK |
| T39: DigitalPrinter: reactions | compiled reactions and circuits | integration | integration | ✅ OK |
| T40: AnalogPrinter: cross watches and carrier reads | the printer and the runtime source | unit | unit | ✅ OK |
| T41: Netlist: discrete nets | compiled reactions and circuits | integration | integration | ✅ OK |
| T42: Commit semantics on the solver | compiled reactions and circuits | integration | integration | ✅ OK |
| T43: The oscillator and the DPWM | compiled reactions and circuits | integration | integration | ✅ OK |
| T44: The reference ramp and the PID | compiled reactions and circuits | integration | integration | ✅ OK |
| T45: Analog to digital | compiled reactions and circuits | integration | integration | ✅ OK |
| T46: Digital to analog | compiled reactions and circuits | integration | integration | ✅ OK |

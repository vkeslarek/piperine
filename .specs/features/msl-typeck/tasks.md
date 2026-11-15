# MSL Type Checking (M1 subset) Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-typeck/design.md`
**Status**: Done
**Verdict**: PASS
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run entry**: `cargo test -p piperine-lang --test types_entry`
**Run values**: `cargo test -p piperine-lang --test types_values`
**Run modules**: `cargo test -p piperine-lang --test types_modules`
**Run analog**: `cargo test -p piperine-lang --test types_analog`
**Run subset**: `cargo test -p piperine-lang --test types_subset`

Executors: Sonnet implements every task; Opus verifies the feature. The worktree and branch are told at dispatch. This feature has no dependency on another M1 feature: it runs in wave 1. `msl-elab` is written against the `Types` interface of `design.md`; keep its names.

Rules every task follows:

- **The POM is read, never written.** `Design` is borrowed shared; every result is a side table or an `Ir`. A test that compares `Design::dump()` before and after guards it.
- **Subset by table.** What is outside M1 is refused through one exhaustive classifier with no wildcard arm; a new POM variant must fail to compile until it is classified.
- **Codes.** A rule of `docs/spec/` carries its `Eppnn` code; an unsupported construct is `P0001` and a plain type mismatch is `P0002` (Q5, decided). Every diagnostic has a span.
- The hand-written parser and the POM (`src/msl/`, `src/pom/`) are not edited. If a task seems to need a change there, stop and report.
- The M1 corpus is `tests/ngspice/devices.msl` and the six twins beside it, read from disk by tests (`CARGO_MANIFEST_DIR/../../tests/ngspice`). They are not edited.
- No unification and no type variable: checking is bidirectional (design "Tech Decisions"). Tables are `Vec` by id or `BTreeMap`, never `HashMap` (determinism, TYP-01 AC 4).
- **Fail loud.** A form the code does not handle is a diagnostic, a `Fault` or an entry of `check()`, never a silent default.
- **AGENTS.md idiom rules bind**: no loose function (every `fn` is a method of a struct or a trait), no macro of your own (`macro_rules!`, proc-macros; derives are fine), modules named after what they do, no `unwrap()`/`expect()` on a user-input path.
- Tests derive from the spec's criteria, never from the implementation. Test functions have exactly the names `## Checks` cites. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`, never set `CARGO_TARGET_DIR`.
- A numeric assertion or tolerance of an existing test is never edited. A number you cannot explain: stop and report, never retune.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the five Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `.specs/features/codegen/context.md` (S1 to S10; ngspice parity is the proof of M1).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Entry point, reach, determinism, the corpus | unit | each AC of TYP-01 and TYP-06; the six twins check clean | `crates/piperine-lang/tests/types_entry.rs` | `cargo test -p piperine-lang --test types_entry` |
| Value expressions, functions, closures, catalogue | unit | each AC of TYP-02 and the four edge cases; one positive and one negative source per rule | `crates/piperine-lang/tests/types_values.rs` | `cargo test -p piperine-lang --test types_values` |
| Modules, params, nodes, placements | unit | each AC of TYP-03, each spec code asserted with its span | `crates/piperine-lang/tests/types_modules.rs` | `cargo test -p piperine-lang --test types_modules` |
| Probes, contributions, core primitives | unit | each AC of TYP-04, both arities of `limit` and `ac_stim` | `crates/piperine-lang/tests/types_analog.rs` | `cargo test -p piperine-lang --test types_analog` |
| The subset classifier | unit | each AC of TYP-05; one `P0001` test row per construct of the spec's Out of Scope table | `crates/piperine-lang/tests/types_subset.rs` | `cargo test -p piperine-lang --test types_subset` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | `cargo test -p piperine-lang --test <the task's target>` |
| Full | every task | `cargo test -p piperine-lang` |
| Build | the last task of each phase | `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Two phases, run in order. Two batches, one per phase (5 and 4 tasks), both Sonnet.

### Phase 1: Tables, reach and value code

Executor: Sonnet.

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: Modules, analog, the subset, the corpus

Executor: Sonnet.

```
T6 → T7 → T8 → T9
```

---

## Task Breakdown

## Phase 1 tasks

### T1: `Ty`, the `Types` table and the entry point

**What**: `Ty` (design "Ty"), `Types<'d>` with its accessors, and `Types::check(design, top)` returning the design's own diagnostics or one diagnostic for an unknown top; the module `piperine_lang::types` is exported.
**Where**: `crates/piperine-lang/src/types/mod.rs`
**Depends on**: None
**Reuses**: `msl/diag.rs` (`Diagnostic`), `pom/design.rs` (`DesignDiagnostic`, `Design::module`)
**Requirement**: TYP-06
**Executor**: Sonnet

**Done when**:

- [x] `Types::check` on a design with diagnostics returns exactly those and no table
- [x] An unknown top, or a top outside the root crate, is one diagnostic naming it
- [x] `ty`, `method`, `callee`, `local`, `reached` exist with the signatures of design "Types" (returning nothing yet for an empty reachable set)
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_entry`: `a_design_with_diagnostics_gives_them_back`, `an_unknown_top_is_one_diagnostic`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): types module, Ty and the Types::check entry point`

---

### T2: The declaration worklist

**What**: `Checker` walks from the top: modules through placements, functions through calls, `const` items through paths; each declaration checked once, in first-reach order; recursion typed from the declared signature.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T1
**Reuses**: `pom/reflect/` handles (`Module`, `Function`, `Const`), `pom/resolution.rs` (`Resolution::Decl`, `CallClass`)
**Requirement**: TYP-01
**Executor**: Sonnet

**Done when**:

- [x] `reached()` lists modules, functions and constants in first-reach order
- [x] A `const` is typed by its declared type and its initializer checked once however many readers
- [x] Two calls on the same design give equal tables
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_entry`: `checking_twice_gives_equal_tables`, `a_const_is_typed_once_by_its_declared_type`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): demand-driven declaration worklist of the type checker`

---

### T3: Value expressions

**What**: `check(expr, expected)` and `infer(expr)` for literals, operators, `if`/`else`, blocks and `let`; a mismatch is one `P0002` at the expression naming both types, and the expression takes the expected type.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T2
**Reuses**: `pom/body.rs` (`ExprKind`, `StmtKind`), `src/stack.rs` (`stack::deep`)
**Requirement**: TYP-02
**Executor**: Sonnet

**Done when**:

- [x] SI-suffixed and `_f64` literals are `f64`; an unsuffixed integer literal where `f64` is required is `P0002`
- [x] Diagnostics of one call come ordered by file then span start, at most one per expression
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `literals_are_f64_or_bool`, `operators_type_as_rust_types_them`, `if_and_block_take_their_value_type`, `a_let_takes_its_initializer_type`, `a_mismatch_names_both_types`, `diagnostics_come_in_source_order_one_per_expression`, `a_let_without_type_or_value_is_p0001`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): bidirectional checking of value expressions`

---

### T4: Function calls and closures

**What**: Calls of reached functions (declared result, each argument checked, optional trailing parameters left out per §3.2) and closures typed from the function type of the parameter they are passed to.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T3
**Reuses**: T2 worklist, T3 checker
**Requirement**: TYP-02
**Executor**: Sonnet

**Done when**:

- [x] `callee(expr)` is `Callee::Function(decl)` for a call of a function
- [x] A closure anywhere but as an argument whose parameter has a function type is `P0001`
- [x] `pnjlim` and `vt` of the embedded `std` check with no diagnostic (SPEC_DEVIATION: `pnjlim` calls `.abs()`/`.ln()` (T5 catalogue) and `vt` calls `temperature()` (T7 core table), so neither can check clean at T4; `pnjlim` is proven in T5 by `the_std_pnjlim_checks_clean`, `vt` in T7 by `the_std_vt_checks_clean`)
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `a_call_takes_its_declared_result`, `a_closure_takes_its_types_from_the_parameter`, `a_recursive_function_is_typed_from_its_signature`, `a_closure_outside_an_argument_is_p0001`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): typed function calls and closures`

---

### T5: The method catalogue

**What**: `Catalogue::method(recv, name)` over the `const` table of `exp`, `ln`, `sqrt`, `abs` on `f64`; a `Deferred` method call records its `Method`; any other method is `P0001`.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T4
**Reuses**: `docs/spec/appendix_h_library.md` H.2 (the rows)
**Requirement**: TYP-01, TYP-02
**Executor**: Sonnet

**Done when**:

- [x] `method(expr)` of `(vd / nvt).exp()` in `tests/ngspice/devices.msl` is `Method::Exp` (SPEC_DEVIATION: the `Diode` of devices.msl also needs `limit`, `v(a, k)`, contributions and params (T6, T7), so the test proves the same expression form `(vd / nvt).exp()` in a function; the devices.msl instance is proven in T9 by `the_reachable_set_of_a_twin_is_typed`)
- [x] A method outside the table (`powf`) is `P0001` naming it
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `a_deferred_method_is_resolved`, `catalogue_methods_on_f64_are_f64`
- [x] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(lang): Appendix H method catalogue of the M1 subset`

---

## Phase 2 tasks

### T6: Modules, params, nodes and placements

**What**: `Checker::module` and `Checker::placement`: params `f64`/`bool` with default and `where`, ports and signals as nodes of a conservative nature, `gnd`, node initial values, placements with node arguments and a param block.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: None
**Reuses**: `pom/reflect/hardware.rs` (`Module::ports`, `params`, `signals`, `Instance`), `PortKind`, `NatureKind`
**Requirement**: TYP-03
**Executor**: Sonnet

**Done when**:

- [x] `E0210`, `E0251`, `E0312`, `E0318`, `E0320` each asserted with the span of the offending construct
- [x] A placement of a user module records `Callee::Module(decl)`; a placement of a core module is routed to T7's table by its `Resolution::Builtin`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_modules`: `params_are_f64_or_bool`, `a_default_of_the_wrong_type_is_rejected`, `a_where_that_is_not_bool_is_e0318`, `ports_and_signals_are_nodes_and_gnd_fits_any`, `a_node_init_that_is_not_f64_is_e0210`, `a_placement_checks_nodes_count_and_params`, `a_node_of_another_nature_is_e0251`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): type checking of modules, params and placements`

---

### T7: Analog forms and the core table

**What**: `Core::signature(BuiltinId)` for `ddt`, `limit`, `ac_stim`, `temperature`, `gmin`, `source_scale`, `potential`; probes through `access` aliases; contributions.
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T6
**Reuses**: `pom/builtin.rs` (`BUILTINS`), `CallClass::Probe`, `StmtKind::Contribution`
**Requirement**: TYP-04
**Executor**: Sonnet

**Done when**:

- [x] `limit(x, f)` and `limit(x, f, seed)`, `ac_stim(mag)` and `ac_stim(mag, phase)` all check
- [x] A core built-in with no row in `Core` (`delay`, `noise`, `ddx`, `cross`, `at`, `now`, `freq`, `analysis`, `simparam`) is `P0001`
- [x] `E0240`, `E0410`, `E0417`, `E0450` each asserted with its span
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_analog`: `potential_probes_are_f64`, `probe_nodes_of_two_natures_are_e0240`, `a_contribution_target_that_is_no_probe_is_e0410`, `a_contribution_that_is_not_f64_is_e0417`, `ddt_ac_stim_and_environment_are_f64`, `limit_checks_its_input_function_and_seed`, `a_core_primitive_in_a_fn_is_e0450`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): type checking of probes, contributions and core primitives`

---

### T8: The subset classifier

**What**: `Unsupported` with exhaustive `expr`, `stmt`, `pat`, `ty`, `item` functions and no wildcard arm; every construct outside TYP-02 to TYP-04 is one `P0001` that names it as the specification does.
**Where**: `crates/piperine-lang/src/types/unsupported.rs`
**Depends on**: T7
**Reuses**: `pom/body.rs`, `pom/decl.rs` enums
**Requirement**: TYP-05
**Executor**: Sonnet

**Done when**:

- [x] One test row per language construct of the spec's Out of Scope table (integers, `match`, `struct`, loops, `let mut`, generics, `digital`, `Option`, tuples, arrays, references, templates, value inputs, module results, `extern`)
- [x] An item holding such a construct that the top does not reach raises nothing
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_subset`: `a_construct_outside_the_subset_is_p0001`, `p0001_names_the_construct`, `unreached_items_are_not_reported`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): exhaustive classifier of constructs outside the M1 subset`

---

## Checks` cites. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`, never set `CARGO_TARGET_DIR`.
- A numeric assertion or tolerance of an existing test is never edited. A number you cannot explain: stop and report, never retune.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the five Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `.specs/features/codegen/context.md` (S1 to S10; ngspice parity is the proof of M1).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Entry point, reach, determinism, the corpus | unit | each AC of TYP-01 and TYP-06; the six twins check clean | `crates/piperine-lang/tests/types_entry.rs` | `cargo test -p piperine-lang --test types_entry` |
| Value expressions, functions, closures, catalogue | unit | each AC of TYP-02 and the four edge cases; one positive and one negative source per rule | `crates/piperine-lang/tests/types_values.rs` | `cargo test -p piperine-lang --test types_values` |
| Modules, params, nodes, placements | unit | each AC of TYP-03, each spec code asserted with its span | `crates/piperine-lang/tests/types_modules.rs` | `cargo test -p piperine-lang --test types_modules` |
| Probes, contributions, core primitives | unit | each AC of TYP-04, both arities of `limit` and `ac_stim` | `crates/piperine-lang/tests/types_analog.rs` | `cargo test -p piperine-lang --test types_analog` |
| The subset classifier | unit | each AC of TYP-05; one `P0001` test row per construct of the spec's Out of Scope table | `crates/piperine-lang/tests/types_subset.rs` | `cargo test -p piperine-lang --test types_subset` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | `cargo test -p piperine-lang --test <the task's target>` |
| Full | every task | `cargo test -p piperine-lang` |
| Build | the last task of each phase | `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Two phases, run in order. Two batches, one per phase (5 and 4 tasks), both Sonnet.

### Phase 1: Tables, reach and value code

Executor: Sonnet.

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: Modules, analog, the subset, the corpus

Executor: Sonnet.

```
T6 → T7 → T8 → T9
```

---

## Task Breakdown

## Phase 1 tasks

### T1: `Ty`, the `Types` table and the entry point

**What**: `Ty` (design "Ty"), `Types<'d>` with its accessors, and `Types::check(design, top)` returning the design's own diagnostics or one diagnostic for an unknown top; the module `piperine_lang::types` is exported.
**Where**: `crates/piperine-lang/src/types/mod.rs`
**Depends on**: None
**Reuses**: `msl/diag.rs` (`Diagnostic`), `pom/design.rs` (`DesignDiagnostic`, `Design::module`)
**Requirement**: TYP-06
**Executor**: Sonnet

**Done when**:

- [x] `Types::check` on a design with diagnostics returns exactly those and no table
- [x] An unknown top, or a top outside the root crate, is one diagnostic naming it
- [x] `ty`, `method`, `callee`, `local`, `reached` exist with the signatures of design "Types" (returning nothing yet for an empty reachable set)
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_entry`: `a_design_with_diagnostics_gives_them_back`, `an_unknown_top_is_one_diagnostic`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): types module, Ty and the Types::check entry point`

---

### T2: The declaration worklist

**What**: `Checker` walks from the top: modules through placements, functions through calls, `const` items through paths; each declaration checked once, in first-reach order; recursion typed from the declared signature.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T1
**Reuses**: `pom/reflect/` handles (`Module`, `Function`, `Const`), `pom/resolution.rs` (`Resolution::Decl`, `CallClass`)
**Requirement**: TYP-01
**Executor**: Sonnet

**Done when**:

- [x] `reached()` lists modules, functions and constants in first-reach order
- [x] A `const` is typed by its declared type and its initializer checked once however many readers
- [x] Two calls on the same design give equal tables
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_entry`: `checking_twice_gives_equal_tables`, `a_const_is_typed_once_by_its_declared_type`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): demand-driven declaration worklist of the type checker`

---

### T3: Value expressions

**What**: `check(expr, expected)` and `infer(expr)` for literals, operators, `if`/`else`, blocks and `let`; a mismatch is one `P0002` at the expression naming both types, and the expression takes the expected type.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T2
**Reuses**: `pom/body.rs` (`ExprKind`, `StmtKind`), `src/stack.rs` (`stack::deep`)
**Requirement**: TYP-02
**Executor**: Sonnet

**Done when**:

- [x] SI-suffixed and `_f64` literals are `f64`; an unsuffixed integer literal where `f64` is required is `P0002`
- [x] Diagnostics of one call come ordered by file then span start, at most one per expression
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `literals_are_f64_or_bool`, `operators_type_as_rust_types_them`, `if_and_block_take_their_value_type`, `a_let_takes_its_initializer_type`, `a_mismatch_names_both_types`, `diagnostics_come_in_source_order_one_per_expression`, `a_let_without_type_or_value_is_p0001`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): bidirectional checking of value expressions`

---

### T4: Function calls and closures

**What**: Calls of reached functions (declared result, each argument checked, optional trailing parameters left out per §3.2) and closures typed from the function type of the parameter they are passed to.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T3
**Reuses**: T2 worklist, T3 checker
**Requirement**: TYP-02
**Executor**: Sonnet

**Done when**:

- [x] `callee(expr)` is `Callee::Function(decl)` for a call of a function
- [x] A closure anywhere but as an argument whose parameter has a function type is `P0001`
- [x] `pnjlim` and `vt` of the embedded `std` check with no diagnostic (SPEC_DEVIATION: `pnjlim` calls `.abs()`/`.ln()` (T5 catalogue) and `vt` calls `temperature()` (T7 core table), so neither can check clean at T4; `pnjlim` is proven in T5 by `the_std_pnjlim_checks_clean`, `vt` in T7 by `the_std_vt_checks_clean`)
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `a_call_takes_its_declared_result`, `a_closure_takes_its_types_from_the_parameter`, `a_recursive_function_is_typed_from_its_signature`, `a_closure_outside_an_argument_is_p0001`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): typed function calls and closures`

---

### T5: The method catalogue

**What**: `Catalogue::method(recv, name)` over the `const` table of `exp`, `ln`, `sqrt`, `abs` on `f64`; a `Deferred` method call records its `Method`; any other method is `P0001`.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T4
**Reuses**: `docs/spec/appendix_h_library.md` H.2 (the rows)
**Requirement**: TYP-01, TYP-02
**Executor**: Sonnet

**Done when**:

- [x] `method(expr)` of `(vd / nvt).exp()` in `tests/ngspice/devices.msl` is `Method::Exp` (SPEC_DEVIATION: the `Diode` of devices.msl also needs `limit`, `v(a, k)`, contributions and params (T6, T7), so the test proves the same expression form `(vd / nvt).exp()` in a function; the devices.msl instance is proven in T9 by `the_reachable_set_of_a_twin_is_typed`)
- [x] A method outside the table (`powf`) is `P0001` naming it
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_values`: `a_deferred_method_is_resolved`, `catalogue_methods_on_f64_are_f64`
- [x] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(lang): Appendix H method catalogue of the M1 subset`

---

## Phase 2 tasks

### T6: Modules, params, nodes and placements

**What**: `Checker::module` and `Checker::placement`: params `f64`/`bool` with default and `where`, ports and signals as nodes of a conservative nature, `gnd`, node initial values, placements with node arguments and a param block.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: None
**Reuses**: `pom/reflect/hardware.rs` (`Module::ports`, `params`, `signals`, `Instance`), `PortKind`, `NatureKind`
**Requirement**: TYP-03
**Executor**: Sonnet

**Done when**:

- [x] `E0210`, `E0251`, `E0312`, `E0318`, `E0320` each asserted with the span of the offending construct
- [x] A placement of a user module records `Callee::Module(decl)`; a placement of a core module is routed to T7's table by its `Resolution::Builtin`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_modules`: `params_are_f64_or_bool`, `a_default_of_the_wrong_type_is_rejected`, `a_where_that_is_not_bool_is_e0318`, `ports_and_signals_are_nodes_and_gnd_fits_any`, `a_node_init_that_is_not_f64_is_e0210`, `a_placement_checks_nodes_count_and_params`, `a_node_of_another_nature_is_e0251`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): type checking of modules, params and placements`

---

### T7: Analog forms and the core table

**What**: `Core::signature(BuiltinId)` for `ddt`, `limit`, `ac_stim`, `temperature`, `gmin`, `source_scale`, `potential`; probes through `access` aliases; contributions.
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T6
**Reuses**: `pom/builtin.rs` (`BUILTINS`), `CallClass::Probe`, `StmtKind::Contribution`
**Requirement**: TYP-04
**Executor**: Sonnet

**Done when**:

- [x] `limit(x, f)` and `limit(x, f, seed)`, `ac_stim(mag)` and `ac_stim(mag, phase)` all check
- [x] A core built-in with no row in `Core` (`delay`, `noise`, `ddx`, `cross`, `at`, `now`, `freq`, `analysis`, `simparam`) is `P0001`
- [x] `E0240`, `E0410`, `E0417`, `E0450` each asserted with its span
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_analog`: `potential_probes_are_f64`, `probe_nodes_of_two_natures_are_e0240`, `a_contribution_target_that_is_no_probe_is_e0410`, `a_contribution_that_is_not_f64_is_e0417`, `ddt_ac_stim_and_environment_are_f64`, `limit_checks_its_input_function_and_seed`, `a_core_primitive_in_a_fn_is_e0450`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): type checking of probes, contributions and core primitives`

---

### T8: The subset classifier

**What**: `Unsupported` with exhaustive `expr`, `stmt`, `pat`, `ty`, `item` functions and no wildcard arm; every construct outside TYP-02 to TYP-04 is one `P0001` that names it as the specification does.
**Where**: `crates/piperine-lang/src/types/unsupported.rs`
**Depends on**: T7
**Reuses**: `pom/body.rs`, `pom/decl.rs` enums
**Requirement**: TYP-05
**Executor**: Sonnet

**Done when**:

- [x] One test row per language construct of the spec's Out of Scope table (integers, `match`, `struct`, loops, `let mut`, generics, `digital`, `Option`, tuples, arrays, references, templates, value inputs, module results, `extern`)
- [x] An item holding such a construct that the top does not reach raises nothing
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_subset`: `a_construct_outside_the_subset_is_p0001`, `p0001_names_the_construct`, `unreached_items_are_not_reported`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): exhaustive classifier of constructs outside the M1 subset`

---

### T9: The M1 corpus checks clean

**What**: The positive corpus: every twin of `tests/ngspice/` checks with no diagnostic, every reachable expression has a type, and the design is unchanged.
**Where**: `crates/piperine-lang/tests/types_entry.rs`
**Depends on**: T8
**Reuses**: `tests/ngspice/*.msl`; `tests/support/pom.rs`
**Requirement**: TYP-01, TYP-06
**Executor**: Sonnet

**Done when**:

- [ ] The six twins (`divider`, `diode_iv`, `diode_series`, `ac_rc`, `tran_rc`, `isrc_r`) each check clean
- [ ] For `DiodeIv`, `reached()` holds `DiodeIv`, `Vsource`, `Resistor`, `Diode`, `vt`, `pnjlim`, `K_B`, `Q` and nothing generic
- [ ] `Design::dump()` is byte-identical before and after
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test types_entry`: `the_reachable_set_of_a_twin_is_typed`, `the_design_is_unchanged_by_the_check`, `every_diagnostic_has_a_span_and_a_code`
- [ ] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `test(lang): the M1 twins type-check clean`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`. A test function has exactly the name its check cites.

**C1** - WHEN `Types::check` is called with a `Design` without diagnostics and the name of a module of its root crate THEN the system SHALL return a table that gives a type to every expression of that module, of every module it places, of every function they call and of every constant they read, transitively. (TYP-01, AC 1; T9)
Proof: run entry | test the_reachable_set_of_a_twin_is_typed ... ok

**C2** - The system SHALL take the `Design` by shared borrow and leave `Design::dump()` byte-identical after the call. (TYP-01, AC 2; T9)
Proof: run entry | test the_design_is_unchanged_by_the_check ... ok

**C3** - WHEN an expression of the reachable set is a method call or a field access the POM left `Deferred` THEN the system SHALL record what it resolves to. (TYP-01, AC 3; T5)
Proof: run values | test a_deferred_method_is_resolved ... ok

**C4** - WHEN `Types::check` is called twice with the same `Design` and top THEN the system SHALL return two tables that are equal. (TYP-01, AC 4; T2)
Proof: run entry | test checking_twice_gives_equal_tables ... ok

**C5** - The system SHALL type a float literal, with or without the suffix `_f64`, and an SI-suffixed literal as `f64`, and `true` and `false` as `bool`. (TYP-02, AC 1; T3)
Proof: run values | test literals_are_f64_or_bool ... ok

**C6** - The system SHALL type `+`, `-`, `*`, `/` and unary `-` over two `f64` as `f64`; `<`, `<=`, `>`, `>=`, `==`, `!=` over two `f64` as `bool`; and `&&`, `||` and `!` over `bool` as `bool`. (TYP-02, AC 2; T3)
Proof: run values | test operators_type_as_rust_types_them ... ok

**C7** - The system SHALL type an `if` with an `else` as the common type of its two blocks, and a block as the type of its last expression, or `()` when it ends with a statement. (TYP-02, AC 3; T3)
Proof: run values | test if_and_block_take_their_value_type ... ok

**C8** - WHEN a `let` has no type annotation THEN the system SHALL give its binding the type of its initializer. (TYP-02, AC 4; T3)
Proof: run values | test a_let_takes_its_initializer_type ... ok

**C9** - The system SHALL type a call of a function by its declared result type after checking each argument against the declared parameter type, optional trailing parameters left out included (§3.2). (TYP-02, AC 5; T4)
Proof: run values | test a_call_takes_its_declared_result ... ok

**C10** - The system SHALL type a call of `exp`, `ln`, `sqrt` or `abs` on an `f64` receiver as `f64`. (TYP-02, AC 6; T5)
Proof: run values | test catalogue_methods_on_f64_are_f64 ... ok

**C11** - WHEN a closure is passed where a parameter of function type is declared THEN the system SHALL type its parameters from that declaration and check its body against the declared result. (TYP-02, AC 7; T4)
Proof: run values | test a_closure_takes_its_types_from_the_parameter ... ok

**C12** - IF two operands, an argument and its parameter, a `let` and its annotation, or the two arms of an `if` have different types THEN the system SHALL reject the model with one diagnostic at the offending expression that names both types. (TYP-02, AC 8; T3)
Proof: run values | test a_mismatch_names_both_types ... ok

**C13** - The system SHALL type a param by its declared type, and SHALL accept `f64` and `bool` params only. (TYP-03, AC 1; T6)
Proof: run modules | test params_are_f64_or_bool ... ok

**C14** - IF a param default does not have the param's type THEN the system SHALL reject the model at the default. (TYP-03, AC 2; T6)
Proof: run modules | test a_default_of_the_wrong_type_is_rejected ... ok

**C15** - IF a `where` condition is not `bool` THEN the system SHALL reject the model with `E0318`. (TYP-03, AC 3; T6)
Proof: run modules | test a_where_that_is_not_bool_is_e0318 ... ok

**C16** - The system SHALL type a port or a `signal` whose type is a conservative nature as a node of that nature, and `gnd` as a node of any nature. (TYP-03, AC 4; T6)
Proof: run modules | test ports_and_signals_are_nodes_and_gnd_fits_any ... ok

**C17** - IF the `= value` of a node is not an `f64` THEN the system SHALL reject the model with `E0210`. (TYP-03, AC 5; T6)
Proof: run modules | test a_node_init_that_is_not_f64_is_e0210 ... ok

**C18** - WHEN a module is placed THEN the system SHALL check that each node argument is a node of the nature of its port, that the argument count equals the port count (`E0312`), and that each entry of the param block names a param of the module once (`E0320`) with a value of the param's type. (TYP-03, AC 6; T6)
Proof: run modules | test a_placement_checks_nodes_count_and_params ... ok

**C19** - IF a node argument has a nature different from its port's THEN the system SHALL reject the model with `E0251`. (TYP-03, AC 7; T6)
Proof: run modules | test a_node_of_another_nature_is_e0251 ... ok

**C20** - The system SHALL type a potential probe `v(a)` or `v(a, b)` through an `access` alias of the nature of its nodes, or through `potential`, as `f64`. (TYP-04, AC 1; T7)
Proof: run analog | test potential_probes_are_f64 ... ok

**C21** - IF the two nodes of a probe or of a contribution have different natures THEN the system SHALL reject the model with `E0240`. (TYP-04, AC 2; T7)
Proof: run analog | test probe_nodes_of_two_natures_are_e0240 ... ok

**C22** - IF the left side of `+>` is not a potential or flow probe of a branch THEN the system SHALL reject the model with `E0410`. (TYP-04, AC 3; T7)
Proof: run analog | test a_contribution_target_that_is_no_probe_is_e0410 ... ok

**C23** - IF the right side of `+>` is not `f64` THEN the system SHALL reject the model with `E0417`. (TYP-04, AC 4; T7)
Proof: run analog | test a_contribution_that_is_not_f64_is_e0417 ... ok

**C24** - The system SHALL type `ddt(x)` with `x: f64` as `f64`, `ac_stim(mag)` and `ac_stim(mag, phase)` over `f64` as `f64`, and `temperature()`, `gmin()` and `source_scale()` as `f64`. (TYP-04, AC 5; T7)
Proof: run analog | test ddt_ac_stim_and_environment_are_f64 ... ok

**C25** - The system SHALL type `limit(x, f)` and `limit(x, f, seed)` as `f64` after checking `x: f64`, `seed: f64` and `f` as a function or closure of type `fn(f64, f64) -> f64`. (TYP-04, AC 6; T7)
Proof: run analog | test limit_checks_its_input_function_and_seed ... ok

**C26** - IF a core primitive of the §14 table is written in a `fn` or a closure where the table does not allow it THEN the system SHALL reject the model with `E0450`, except a core module (`ddt`, `limit`), `E0350`. (TYP-04, AC 7; T7, F9)
Proof: run analog | test a_core_primitive_in_a_fn_is_e0450 ... ok

**C27** - IF an expression, statement, pattern, type or item of the reachable set is outside TYP-02 to TYP-04 THEN the system SHALL reject the model with one diagnostic of the code `P0001` at that construct. (TYP-05, AC 1; T8)
Proof: run subset | test a_construct_outside_the_subset_is_p0001 ... ok

**C28** - The system SHALL name in every `P0001` diagnostic the construct as the specification names it (for example "`match` expression", "generic module", "integer type `u32`"). (TYP-05, AC 2; T8)
Proof: run subset | test p0001_names_the_construct ... ok

**C29** - WHEN an item holds a construct outside the subset and no top module reaches it THEN the system SHALL report nothing for it. (TYP-05, AC 3; T8)
Proof: run subset | test unreached_items_are_not_reported ... ok

**C30** - IF the `Design` holds a diagnostic THEN the system SHALL return no table and SHALL return those diagnostics. (TYP-06, AC 1; T1)
Proof: run entry | test a_design_with_diagnostics_gives_them_back ... ok

**C31** - IF the named top is not a module of the root crate THEN the system SHALL return no table and one diagnostic naming it. (TYP-06, AC 2; T1)
Proof: run entry | test an_unknown_top_is_one_diagnostic ... ok

**C32** - The system SHALL return every diagnostic of one call ordered by file, then by span start, and SHALL report at most one diagnostic per expression. (TYP-06, AC 3; T3)
Proof: run values | test diagnostics_come_in_source_order_one_per_expression ... ok

**C33** - The system SHALL attach to every diagnostic the span of the offending construct and a code. (TYP-06, AC 4; T9)
Proof: run entry | test every_diagnostic_has_a_span_and_a_code ... ok

**C34** - WHEN a function of the reachable set calls itself THEN the system SHALL type it from its declared signature without looping. (TYP edge case 1; T4)
Proof: run values | test a_recursive_function_is_typed_from_its_signature ... ok

**C35** - IF a `let` has neither an annotation nor an initializer THEN the system SHALL reject the model with `P0001`. (TYP edge case 2; T3)
Proof: run values | test a_let_without_type_or_value_is_p0001 ... ok

**C36** - WHEN a name resolves to a `const` item THEN the system SHALL type it by its declared type and check its initializer once. (TYP edge case 3; T2)
Proof: run entry | test a_const_is_typed_once_by_its_declared_type ... ok

**C37** - IF a closure is written anywhere but as an argument whose parameter has a function type THEN the system SHALL reject the model with `P0001`. (TYP edge case 4; T4)
Proof: run values | test a_closure_outside_an_argument_is_p0001 ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** 2026-11-15 batches 1-2 done (0f7d03cf); proofs 38/38; Verifier rounds 1-2 (Opus) done; round-2 findings F7-F11 closed by 8a9228d5; no third round, ready to merge
**Walked:** not applicable - the feature puts no screen in front of a user; each story's Independent Test is a test named in `## Checks`

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1 (Sonnet):  T1 → T2 → T3 → T4 → T5
Phase 2 (Sonnet):  T6 → T7 → T8 → T9
```

Batches: B1 = Phase 1 (5 tasks, Sonnet); B2 = Phase 2 (4 tasks, Sonnet). The Verifier (Opus) runs after B2.

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: `Ty`, the `Types` table and the entry point | 1 module root + its entry point | ✅ Granular |
| T2: The declaration worklist | 1 worklist | ✅ Granular |
| T3: Value expressions | 1 expression checker | ✅ Granular |
| T4: Function calls and closures | 1 call checker | ✅ Granular |
| T5: The method catalogue | 1 table + lookup | ✅ Granular |
| T6: Modules, params, nodes and placements | 1 declaration checker | ✅ Granular |
| T7: Analog forms and the core table | 1 analog checker + 1 table | ✅ Granular |
| T8: The subset classifier | 1 classifier | ✅ Granular |
| T9: The M1 corpus checks clean | 1 corpus test target | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of its phase | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | None | first of its phase | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: `Ty`, the `Types` table and the entry point | Entry point | unit | unit | ✅ OK |
| T2: The declaration worklist | Entry point, reach | unit | unit | ✅ OK |
| T3: Value expressions | Value expressions | unit | unit | ✅ OK |
| T4: Function calls and closures | Value expressions, functions, closures | unit | unit | ✅ OK |
| T5: The method catalogue | Catalogue | unit | unit | ✅ OK |
| T6: Modules, params, nodes and placements | Modules, params, nodes, placements | unit | unit | ✅ OK |
| T7: Analog forms and the core table | Probes, contributions, core primitives | unit | unit | ✅ OK |
| T8: The subset classifier | The subset classifier | unit | unit | ✅ OK |
| T9: The M1 corpus checks clean | Entry point, the corpus | unit | unit | ✅ OK |

**C38** - IF the `where` of a module is not `bool` THEN the system SHALL reject the model with `E0304`; IF the `where` of a `fn` reads a value parameter THEN `P0001`, and IF it is not `bool` THEN `P0002`. (TYP-03, AC 8 and 9; verification F1, F7, F8)
Proof: run modules | test a_where_on_a_module_or_fn_is_bool ... ok

**C39** - IF a core primitive or a probe is written where the §14 "Allowed in" column does not list it (a module body, a param default or a `where`, besides a `fn` or a closure) THEN the system SHALL reject the model with `E0450`, except `ddt` and `limit` in a `fn` or a closure (`E0350`) or in a const context (`E0352`). (TYP-04, AC 8; verification F2, F9)
Proof: run modules | test a_core_primitive_is_checked_where_it_is_written ... ok

**C40** - IF an environment function is written in a const context THEN the system SHALL reject the model with `E0323`. (TYP-04, AC 9; verification F2)
Proof: run modules | test an_environment_function_in_a_const_context_is_e0323 ... ok

**C41** - IF a function is written as a value anywhere but as the argument itself where the parameter has a function type (not through an `if` or a block), or a call goes through a parameter or a local THEN the system SHALL reject the model with `P0001`. (TYP-05, AC 4; verification F3)
Proof: run values | test a_function_is_a_value_only_as_an_argument ... ok

**C42** - IF a construct is refused with `P0001` THEN the system SHALL still report the independent errors of the initializer, of a generic function's body, and of a missing result unless the last statement is a refused `return`. (TYP-05, AC 5; verification F6, F11)
Proof: run values | test an_unsupported_form_does_not_hide_an_independent_error ... ok

**C43** - The system SHALL refuse `Bit`, `Repr` and the known-flag method with `P0001` against the real standard library. (TYP-05, AC 1; verification F5)
Proof: run subset | test std_types_outside_the_subset_are_p0001 ... ok

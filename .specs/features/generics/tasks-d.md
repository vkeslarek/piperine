# Generics Tasks (phase D: templates and implicit params)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-d.md`
**Status**: Draft
**Approved**: (written when the user approves)
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typestempl**: `cargo test -p piperine-lang --test types_templates`
**Run typesimplicit**: `cargo test -p piperine-lang --test types_implicit`
**Run typestraits**: `cargo test -p piperine-lang --test types_traits`
**Run elabtempl**: `cargo test -p piperine-lang --test elab_templates`
**Run elabimplicit**: `cargo test -p piperine-lang --test elab_implicit`
**Run irgen**: `cargo test -p piperine-ir --test generics`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run srctempl**: `cargo test -p piperine-parity --test source_templates`
**Run srcimplicit**: `cargo test -p piperine-parity --test source_implicit`
**Run apisession**: `cargo test -p piperine-api --test session`
**Run apiderived**: `cargo test -p piperine-api --test derived`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Base and order.** Phase 18 starts after `tasks-c.md` T81 (phase C closed). The answers to design-d.md's open
questions DD1 to DD3 bind T82, T88, T92, T93 and T96; until the user answers, each takes its recommended default and the
answer is written on the Handoff. GN-24 AC 4 (noise) is `tasks-e.md` T111.

Rules every task follows (the user's rules, repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, the lane letter the coordinator gives. `/var/tmp/piperine-target` is a read-only trap and is never used. Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-21).** After each task: the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, then the `Run` lines its `Done when` names. Once at the end of each batch: `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` (except the one move design-d.md lists under DD1, in T93 and T96) and every parity target stay byte-identical; no tolerance moves.
- **Only design-d.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; a divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same expression evaluated by Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules).** Readable at a glance; the module tree says where each thing is; each crate one purpose; **no free function, tests included** (every function has an owner type: a test helper is a method of a support type); **one purpose per type**; **OO names without abbreviations**; **no god method** and no needless abstraction; the data path evident; **a `///` doc comment on every new type, method, field and variant**, and a non-evident block cites its spec rule. No macro; no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design-d.md "Components" names, never into `types/expr.rs`, `types/call.rs` or `print/value.rs` beyond a dispatch arm.
- **Conventions.** No new IR `Expr`/`Stmt` variant; new IR fields carry `#[serde(default)]`. The interpreter refuses every new IR marker by name (R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models live in `crates/piperine-parity/tests/templates/*.msl` and `tests/implicit/*.msl`.
- **Book rows.** A Book test that asserts a refusal and starts to run flips in the task that makes it run: its test becomes the acceptance, `tests/book/inventory.tsv` takes `run` (or `holds`), the `GAPS.md` §11 row leaves, and the Book's own "today it refuses" prose is rewritten to what the binary now prints. A row that still needs another feature's form is narrowed to that form, and its §11 row takes that gap id and owner (recorded on the Handoff).
- **Cross-lane files.** `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `ROADMAP.md` rows of other features are not edited.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-18, MD-38, MD-57, MD-69, MD-71, MD-75, MD-81.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each code with its span and the names it gives | `crates/piperine-lang/tests/{types_templates,types_implicit,types_traits}.rs` | `cargo test -p piperine-lang --test types_templates` / `--test types_implicit` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC; each code; the POM untouched | `crates/piperine-lang/tests/{elab_templates,elab_implicit}.rs` | `cargo test -p piperine-lang --test elab_templates` / `--test elab_implicit` |
| IR (`piperine-ir`) | integration | the new field built, checked (valid and refused), printed | `crates/piperine-ir/tests/generics.rs` | `cargo test -p piperine-ir --test generics` |
| Interpreter refusals | integration | `m`/`dtemp` other than 1 and 0 refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Source-level runs | integration | one test per run-time AC on the Rust backend, expected values computed by Rust in the test | `crates/piperine-parity/tests/{source_templates,source_implicit}.rs` | `cargo test -p piperine-parity --test source_templates` / `--test source_implicit` |
| Host (`piperine-api`) | integration | listing and setting `m`, `dtemp`; descendants re-derived | `crates/piperine-api/tests/{session,derived}.rs` | `cargo test -p piperine-api --test session` / `--test derived` |
| Spec text | integration | each amended sentence | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| Book | integration | each owned row flipped or narrowed | `tests/book/*.rs`, `tests/book/inventory.tsv` | `cargo test -p piperine --test book_<chapter>` |
| Repository records | integration | one assertion per removed `GAPS.md` row | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After every task with tests | the `**Build**` line; `cargo test -p <crate>` for each crate the task touched; the `Run` lines its `Done when` names |
| Full | The last task of each batch, and T96 (it moves every device's generated code) | Quick, then `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets |
| Build | A task with `Tests: none` | the `**Build**` line, then `cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 18: The spec amendment and templates

```
T82 → T83 → T84 → T85 → T86 → T87
```

### Phase 19: Module trait impls, `inst.name` through templates, the template Book rows

```
T88 → T89 → T90 → T91 → T92
```

### Phase 20: `m` and `dtemp`

```
T93 → T94 → T95 → T96 → T97 → T98 → T99 → T100
```

---

## Task Breakdown

### T82: The spec takes impls on module types

**What**: Amend `docs/spec` per the user's decision of 2026-11-22 and DD3: §3.7 lists a module type among the `impl Tr for` targets and §3.1 allows an inherent `impl` on one, its functions pure functions of the template (no node, quantity or time: `E0122`); the `E0130` row leaves §3.1's errors (DD3 (a)); §8.4 says a bound may combine a trait and `Fn(P)`. `spec.md` GN-03 AC 2 and `tasks.md` check C8 are amended to the decision (the approval line of the decision is quoted). One `spec_text` assertion per sentence.
**Where**: `docs/spec/part_i_values.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs`
**Requirement**: GN-23, GN-03 (AC 2 as amended)
**Executor**: Sonnet

**Done when**:

- [ ] `a_module_type_takes_trait_and_inherent_impls_in_the_spec` and `a_bound_combines_a_trait_and_fn_p_in_the_spec` pass under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): a module type takes impls; a bound combines a trait and Fn(P)`

---

### T83: Template values in the checker

**What**: `types/templates.rs` types `Name { params }` and `Name::<G> { params }` as a value of the module's type wherever a value goes (a `const`, a param value, a function result, a `let` of a const context), replacing the `P0001` of `types/expr.rs`; a required param left unbound is `E0319`; a template-valued expression that is not const is `E0362`; a template as a carried type (`Bit<Bsim4>`, a signal) is `E0112`.
**Where**: `crates/piperine-lang/src/types/templates.rs`
**Depends on**: T82
**Reuses**: `Context::Const`, `Checker::generic_placement`
**Requirement**: GN-21 (AC 1 to AC 4)
**Executor**: Sonnet

**Done when**:

- [ ] `a_template_is_a_value_of_its_modules_type`, `a_template_leaving_a_required_param_is_e0319`, `a_template_that_is_not_const_is_e0362` and `a_template_is_never_carried_e0112` pass under `**Run typestempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): templates are values`

---

### T84: Templates fold from consts and function results

**What**: `TemplateFolder` (`elab/templates.rs`) folds a template from a `const`, a param, a template literal of a generic module (its `Instance` part of the interning key) and the result of a function evaluated in `ConstEval` (a binning function returns a `TemplateValue`); two equal templates are one number; a template passed through two template-typed params folds to the innermost binding.
**Where**: `crates/piperine-lang/src/elab/templates.rs`
**Depends on**: T83
**Reuses**: `ConstEval`, template interning
**Requirement**: GN-21 (AC 1), GN-22 (AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `templates_fold_from_consts_functions_and_two_levels_of_params` passes under `**Run elabtempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): templates fold from consts and function results`

---

### T85: Placement through a template value in the checker

**What**: A call whose callee is a template-valued const expression (`cell(conns)`, `nch(l, w)(d, g, s, b)`, `C1(a, b)`) types as a placement of the template's module, its connections checked against its ports, replacing the `P0001` of `types/call.rs` and `types/module.rs`; a param block on it is `E0361`.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T84
**Reuses**: `Checker::placement`, the `E0361` arm
**Requirement**: GN-22 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `a_placement_through_a_template_or_a_function_types_as_its_module` and `a_param_block_on_a_template_placement_is_e0361` pass under `**Run typestempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): placement through a template value or a function`

---

### T86: Placement through any folded template

**What**: `elab/children.rs` places the module of any folded template with its folded params (identical to `Name(conns) { params }`, §8.4.9), replacing the `P0001` "a placement through a template value"; a binning function places the module and params its arguments choose; every param a template-valued expression reads is structural (phase C's `KeyParams`).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T85
**Reuses**: `template_values`, `KeyParams`
**Requirement**: GN-22 (AC 1, AC 3), GN-20 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `a_template_placement_is_identical_to_the_written_placement` and `a_binning_function_places_the_module_it_chooses` pass under `**Run elabtempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): placement through any template`

---

### T87: `Fn(P)` bounds on modules

**What**: `types/module_bounds.rs` checks a template bound to `C: Fn(P)`: the module's ports have the types and directions of `P`, in order, names not counting, a module with a result included (§8.4.5), else `E0360` naming the first port that differs; a bound `N: Tr + Fn(P)` also needs `impl Tr for` the module (`E0187` naming the module and the trait).
**Where**: `crates/piperine-lang/src/types/module_bounds.rs`
**Depends on**: T86
**Reuses**: `Bound::Callable`, `BoundCheck`
**Requirement**: GN-23 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `a_module_whose_ports_match_fits_fn_p_names_not_counting` and `a_module_whose_ports_differ_is_e0360_naming_the_first` pass under `**Run typestempl**`
- [ ] Gate check passes: Full (last task of batch B14)

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): Fn(P) bounds on modules`

---

### T88: Trait and inherent impls on a module type

**What**: `Coherence` accepts `impl Mosfet for Bsim4 { .. }` and `impl Bsim4 { .. }`: `self` is the template, functions may read the template's params and return values or templates; a function that probes, reads a quantity or time is `E0122` naming it (DD3); the `E0130` arm leaves the classifier; `an_impl_on_a_module_is_e0130` is replaced by `an_impl_on_a_module_type_is_accepted` (design-d.md pins). A module bound by `N: Mosfet + Fn(..)` without the impl is `E0187` (T87) proven here with a real trait.
**Where**: `crates/piperine-lang/src/types/coherence.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ImplTable`, `FunctionClasses`
**Requirement**: GN-23 (the decision of 2026-11-22), GN-03 (AC 2 as amended)
**Executor**: Sonnet

**Done when**:

- [ ] `an_impl_on_a_module_type_is_accepted` and `a_module_impl_function_that_reads_the_solve_is_e0122` pass under `**Run typestraits**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): trait and inherent impls on a module type`

---

### T89: Module `impl` functions run at elaboration

**What**: `TemplateFolder` and `ConstEval` call a module `impl` function on a template (`nmos.vth()` in a param default or a body `let`, a method returning another template for binning), with `self` bound to the folded template and its params readable as fields; static dispatch through a `N: Mosfet` bound picks the bound module's impl.
**Where**: `crates/piperine-lang/src/elab/templates.rs`
**Depends on**: T88
**Reuses**: `ImplTable::implementation`, `ConstEval`
**Requirement**: GN-23 (the decision of 2026-11-22)
**Executor**: Sonnet

**Done when**:

- [ ] `a_module_impl_function_runs_on_a_template_at_elaboration` passes under `**Run elabtempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): module impl functions on templates`

---

### T90: `inst.name` through a template-typed param

**What**: `inst.name` on an instance placed through a template-typed param (`bank.cells[3][5].sn` in shape) resolves at elaboration against the concrete module the template names (§8.4.8); a missing or non-`pub` item is `E0343`.
**Where**: `crates/piperine-lang/src/elab/reach_through_templates.rs`
**Depends on**: T89
**Reuses**: `analog-structure`'s `inst.name` resolution, `TemplateFolder`
**Requirement**: GN-23 (AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `inst_name_through_a_template_param_reads_the_concrete_module` and `a_missing_item_through_a_template_param_is_e0343` pass under `**Run elabtempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): inst.name through a template-typed param`

---

### T91: Templates run end to end

**What**: Source tests on the Rust backend: the `MemoryBank`/`Dram1T1C` shape of §8.4 over `Electrical` ports (`C: Fn(Electrical, Electrical)`, generated cells, a cell's `sn` read through `bank.cells[r][c].sn`); `nch` binning at two lengths gives the chosen `vth0` (read through a param probe); the swappable MOSFET (`trait Mosfet`, two analog stand-in models, `DiffPair<N: Mosfet + Fn(..)>`) gives each model's own operating point, computed in Rust in the test.
**Where**: `crates/piperine-parity/tests/source_templates.rs`
**Depends on**: T90
**Reuses**: `crates/piperine-parity/tests/templates/*.msl`
**Requirement**: GN-21, GN-22, GN-23
**Executor**: Sonnet

**Done when**:

- [ ] `a_memory_bank_places_its_cell_template`, `binning_places_the_chosen_model` and `the_swappable_mosfet_runs_each_model` pass under `**Run srctempl**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): templates, binning and the swappable MOSFET`

---

### T92: The template Book rows

**What**: Flip modules#12 (`nch(0.13u, 2u)(d, g, s, b)` places `Bsim4` with the binned `vth0`); add the swappable MOSFET listing to the modules chapter after the binning paragraph (DD6) with an inventory row `run` and its test; narrow modules#10 and cheatsheet#8 to the bundle `Cell` (their test asserts only the bundle's refusal) and hand their §11 rows to `resolved-nets-bundles` under L25 (DD2); `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `tests/book/modules.rs`
**Depends on**: T91
**Reuses**: `Chapter`, `Refusal`
**Requirement**: GN-31 (AC 2), GN-22, GN-23
**Executor**: Sonnet

**Done when**:

- [ ] `a_function_that_returns_a_template_places_its_result`, `the_swappable_mosfet_listing_runs` and `the_memory_bank_is_refused_only_for_its_bundle` pass under `**Run bookmodules**`
- [ ] `the_template_card_is_refused_only_for_its_bundle` passes under `**Run bookcheatsheet**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Full (last task of batch B15)

**Tests**: integration
**Gate**: full

**Commit**: `test(book): binning and the swappable MOSFET run; the bundle rows go to resolved-nets-bundles`

---

### T93: IR `BodyData::implicit`

**What**: `ImplicitParams { multiplicity, offset }` and `BodyData::implicit: Option<ImplicitParams>` (`#[serde(default)]`), the two indices naming `f64` param pins appended after the body's own params; `Ir::check` holds them in range and typed `f64`; the IR printer shows them; the interpreter refuses an instance whose `m` is not 1 or whose `dtemp` is not 0, by name. The elaborator gives every body the pair (DD1 (a)); every moved pin or golden is listed in the commit body.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Param`, `IrBuilder`
**Requirement**: GN-24, GN-25, GN-29 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `implicit_params_are_built_checked_and_printed` passes under `**Run irgen**`
- [ ] `each_generics_form_is_refused_by_name` covers `m` and `dtemp` under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): every body carries the implicit params m and dtemp`

---

### T94: The checker binds and reads `m` and `dtemp`

**What**: `types/implicit.rs`: `m` and `dtemp` may be bound in a placement's or a template's param block (`E0320` no longer); the names read inside a module are `f64`; a module item (port, param, signal, `let`) named `m` or `dtemp` is `E0375`; a const `m <= 0.0` is `E0317`.
**Where**: `crates/piperine-lang/src/types/implicit.rs`
**Depends on**: T93
**Reuses**: `BuiltinKind::ImplicitParam`
**Requirement**: GN-24 (AC 1), GN-25 (AC 2, AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `m_and_dtemp_are_bound_and_read_as_f64`, `an_item_named_m_or_dtemp_is_e0375` and `m_at_most_zero_is_e0317` pass under `**Run typesimplicit**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): bind and read m and dtemp`

---

### T95: Effective `m` and `dtemp` down the path

**What**: `elab/implicit.rs` binds a child's implicit params as `ChildValue::Expr`: multiplicity `parent.M × child.m`, offset `parent.dtemp + child.dtemp` (§8.5.3, §8.5.5), a top's `1.0` and `0.0`; a non-const `m` that is `<= 0.0` at elaboration is `E0317`; the name `m` reads `M` and `dtemp` the effective offset.
**Where**: `crates/piperine-lang/src/elab/implicit.rs`
**Depends on**: T94
**Reuses**: derived params (`ChildValue::Expr`, MD-69)
**Requirement**: GN-24 (AC 1, AC 2), GN-25 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `effective_m_is_the_product_and_dtemp_the_sum_along_the_path` passes under `**Run elabimplicit**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): effective m and dtemp along the path`

---

### T96: The IR scales flows by `M` and shifts the temperature

**What**: `ImplicitScaling` multiplies each flow contribution by the body's `M` and divides each flow probe by `M`, leaves potential contributions, potential probes, discrete carriers, events and reactions unchanged (§8.5.4), and lowers `temperature()` in the body as the analysis temperature plus the offset (§8.5.5); the transpile goldens move once (DD1 (a)), the twins and parity suites do not.
**Where**: `crates/piperine-lang/src/elab/implicit.rs`
**Depends on**: T95
**Reuses**: `elab/contribute.rs`, `elab/analog.rs`
**Requirement**: GN-24 (AC 3), GN-25 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `flows_scale_by_m_and_potentials_and_discretes_do_not` and `temperature_reads_the_analysis_value_plus_the_offset` pass under `**Run elabimplicit**`
- [ ] `every_twin_prints_the_bytes_it_printed_before_the_move` passes under `**Run twins**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): flows scale by m and temperature shifts by dtemp`

---

### T97: The host lists and sets `m` and `dtemp`

**What**: `piperine-api` lists `m` and `dtemp` among each instance's params (with the effective value) and treats a `set` of either as a param change, the descendants' effective values re-derived (X9, MD-69).
**Where**: `crates/piperine-api/src/params.rs`
**Depends on**: T96
**Reuses**: the derive plan, `Session::set`
**Requirement**: GN-26 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `an_instance_lists_m_and_dtemp` passes under `**Run apisession**`
- [ ] `setting_m_rederives_the_descendants` passes under `**Run apiderived**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): list and set m and dtemp`

---

### T98: `m` and `dtemp` run end to end

**What**: Source tests: a resistor with `m: 4.0` gives the solution of four in parallel (GN-24 AC 5); `m: 2.0` on a parent and `m: 3.0` on its child scale the child's current by 6; a flow probe reads the per-device current; a potential source with `m` is unchanged; `m` on a module with no flow contribution changes nothing; a diode with `dtemp: 25.0` equals the same diode at the analysis temperature plus 25 K (GN-25 AC 1); expected values computed in Rust in the test.
**Where**: `crates/piperine-parity/tests/source_implicit.rs`
**Depends on**: T97
**Reuses**: `crates/piperine-parity/tests/implicit/*.msl`
**Requirement**: GN-24 (AC 2, AC 3, AC 5), GN-25 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `a_resistor_with_m_four_is_four_in_parallel`, `m_multiplies_down_the_path_and_spares_potentials`, `m_without_a_flow_changes_nothing` and `a_diode_with_dtemp_runs_at_the_shifted_temperature` pass under `**Run srcimplicit**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): m and dtemp`

---

### T99: The Book's `m` and `dtemp` row

**What**: Flip modules~1faf8c12: `Resistor(a, b) { r: 1k, m: 4.0 }` runs as four resistors in parallel and the `dtemp` example as a shifted temperature; `binding_m_and_dtemp_is_refused_as_an_unknown_param` becomes `binding_m_and_dtemp_runs_as_the_book_says`; `inventory.tsv` takes `holds`, and the row leaves `GAPS.md` §11 (GN-26 AC 3).
**Where**: `tests/book/modules.rs`
**Depends on**: T98
**Reuses**: `Parts`, `Faulty`
**Requirement**: GN-26 (AC 3)
**Executor**: Sonnet

**Done when**:

- [ ] `binding_m_and_dtemp_runs_as_the_book_says` passes under `**Run bookmodules**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(book): m and dtemp run as the Book says`

---

### T100: Phase D bookkeeping

**What**: `GAPS.md` drops L29 and L32, updates §9 (III §8.4–§8.5) and the §10 code list (`E0360`, `E0362`, `E0375` leave `code_owners.tsv`); `spec.md`'s traceability marks GN-21 to GN-26 phase D; `.specs/STATE.md` MD-81 gains item 7 (design-d.md "Tech Decisions"); `ROADMAP.md`'s `generics` D row says delivered.
**Where**: `GAPS.md`
**Depends on**: T99
**Reuses**: `docs.rs`
**Requirement**: GN-31 (AC 1)
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_generics_phase_d_closed` and `state_records_templates_and_implicit_params` pass under `**Run docs**`
- [ ] Gate check passes: Full (last task of batch B16)

**Tests**: integration
**Gate**: full

**Commit**: `docs: generics phase D closes L29 and L32`

---

## Checks

**C86** - `Name { params }` and `Name::<G> { params }` as a `const`, a param value or a function result give a value of the module's type. (GN-21, AC 1; T83, T84)
Proof: run typestempl | test a_template_is_a_value_of_its_modules_type ... ok
Proof: run elabtempl | test templates_fold_from_consts_functions_and_two_levels_of_params ... ok

**C87** - A template leaving a required param unbound is `E0319`. (GN-21, AC 2; T83)
Proof: run typestempl | test a_template_leaving_a_required_param_is_e0319 ... ok

**C88** - A template-valued expression that is not const is `E0362`. (GN-21, AC 3; T83)
Proof: run typestempl | test a_template_that_is_not_const_is_e0362 ... ok

**C89** - A template is never carried: `E0112`. (GN-21, AC 4; T83)
Proof: run typestempl | test a_template_is_never_carried_e0112 ... ok

**C90** - A placement through a template value or a function result places an instance identical to the written placement. (GN-22, AC 1; T85, T86, T92)
Proof: run typestempl | test a_placement_through_a_template_or_a_function_types_as_its_module ... ok
Proof: run elabtempl | test a_template_placement_is_identical_to_the_written_placement ... ok
Proof: run bookmodules | test a_function_that_returns_a_template_places_its_result ... ok

**C91** - A param block on a placement through a template value is `E0361`. (GN-22, AC 2; T85)
Proof: run typestempl | test a_param_block_on_a_template_placement_is_e0361 ... ok

**C92** - A binning function places the module and params its arguments choose. (GN-22, AC 3; T86, T91)
Proof: run elabtempl | test a_binning_function_places_the_module_it_chooses ... ok
Proof: run srctempl | test binning_places_the_chosen_model ... ok

**C93** - A module whose port types and directions match `P` fits `C: Fn(P)`, names not counting. (GN-23, AC 1; T87, T91)
Proof: run typestempl | test a_module_whose_ports_match_fits_fn_p_names_not_counting ... ok
Proof: run srctempl | test a_memory_bank_places_its_cell_template ... ok

**C94** - A module whose ports differ from `P` is `E0360` naming the first port that differs. (GN-23, AC 2; T87)
Proof: run typestempl | test a_module_whose_ports_differ_is_e0360_naming_the_first ... ok

**C95** - `inst.name` through a template-typed param resolves against the concrete module, `E0343` for a missing or private item. (GN-23, AC 3; T90)
Proof: run elabtempl | test inst_name_through_a_template_param_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_missing_item_through_a_template_param_is_e0343 ... ok

**C96** - A module type takes trait and inherent impls of pure functions over its template, run at elaboration; a trait plus `Fn(P)` bound admits only a module that implements the trait; a function that reads the solve is `E0122`. (GN-23; the decision of 2026-11-22; T82, T88, T89, T91)
Proof: run spectext | test a_module_type_takes_trait_and_inherent_impls_in_the_spec ... ok
Proof: run spectext | test a_bound_combines_a_trait_and_fn_p_in_the_spec ... ok
Proof: run typestraits | test an_impl_on_a_module_type_is_accepted ... ok
Proof: run typestraits | test a_module_impl_function_that_reads_the_solve_is_e0122 ... ok
Proof: run elabtempl | test a_module_impl_function_runs_on_a_template_at_elaboration ... ok
Proof: run srctempl | test the_swappable_mosfet_runs_each_model ... ok

**C97** - `m` may be bound at a placement or a template, and `m <= 0.0` is `E0317`. (GN-24, AC 1; T94)
Proof: run typesimplicit | test m_and_dtemp_are_bound_and_read_as_f64 ... ok
Proof: run typesimplicit | test m_at_most_zero_is_e0317 ... ok

**C98** - The effective multiplicity is the product of `m` along the path and the name `m` reads it. (GN-24, AC 2; T95, T98)
Proof: run elabimplicit | test effective_m_is_the_product_and_dtemp_the_sum_along_the_path ... ok
Proof: run srcimplicit | test m_multiplies_down_the_path_and_spares_potentials ... ok

**C99** - Flow contributions scale by `M`, flow probes divide by it, potentials, discrete carriers, events and reactions are unchanged. (GN-24, AC 3; T96, T98)
Proof: run elabimplicit | test flows_scale_by_m_and_potentials_and_discretes_do_not ... ok

**C100** - A resistor with `m: 4.0` gives the solution of four in parallel. (GN-24, AC 5; T98)
Proof: run srcimplicit | test a_resistor_with_m_four_is_four_in_parallel ... ok

**C101** - The effective offset is the sum of `dtemp` along the path and `temperature()` returns the analysis temperature plus it. (GN-25, AC 1; T95, T96, T98)
Proof: run elabimplicit | test temperature_reads_the_analysis_value_plus_the_offset ... ok
Proof: run srcimplicit | test a_diode_with_dtemp_runs_at_the_shifted_temperature ... ok

**C102** - A module reads the name `dtemp` as its effective offset. (GN-25, AC 2; T94, T95)
Proof: run elabimplicit | test effective_m_is_the_product_and_dtemp_the_sum_along_the_path ... ok

**C103** - A module item named `m` or `dtemp` is `E0375`. (GN-25, AC 3; T94)
Proof: run typesimplicit | test an_item_named_m_or_dtemp_is_e0375 ... ok

**C104** - A host `set` of `m` or `dtemp` is a param change and the descendants follow. (GN-26, AC 1; T97)
Proof: run apiderived | test setting_m_rederives_the_descendants ... ok

**C105** - A host lists `m` and `dtemp` among an instance's params. (GN-26, AC 2; T97)
Proof: run apisession | test an_instance_lists_m_and_dtemp ... ok

**C106** - The Book's `m` and `dtemp` sentence runs and its row leaves `GAPS.md` §11. (GN-26, AC 3; T99)
Proof: run bookmodules | test binding_m_and_dtemp_runs_as_the_book_says ... ok

**C107** - The interpreter refuses `m` other than 1 and `dtemp` other than 0 by name; a template leaves no IR form. (GN-29, AC 1; T93, T86)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run irgen | test implicit_params_are_built_checked_and_printed ... ok

**C108** - `m` on a module with no flow contribution changes nothing. (Edge case; T98)
Proof: run srcimplicit | test m_without_a_flow_changes_nothing ... ok

**C109** - `GAPS.md` drops L29 and L32 and §9 and §10 follow. (GN-31, AC 1; T100)
Proof: run docs | test gaps_drops_what_generics_phase_d_closed ... ok

**C110** - The template Book rows flip or narrow to the bundle they still need, and the swappable MOSFET listing runs. (GN-31, AC 2; T92)
Proof: run bookmodules | test the_swappable_mosfet_listing_runs ... ok
Proof: run bookmodules | test the_memory_bank_is_refused_only_for_its_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_only_for_its_bundle ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C111** - The 7 ngspice twins print the bytes they printed before, with every body carrying `m` and `dtemp`. (GN-30, AC 1; T96, every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C112** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

---

## Handoff

The phase's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none yet. Planning (2026-11-23): the defaults of design-d.md DD4 to DD6; open questions DD1 to DD3 take their recommended default until the user answers.
**Dispatched:** none yet.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 18 → Phase 19 → Phase 20

Phase 18:  T82 → T83 → T84 → T85 → T86 → T87
Phase 19:  T88 → T89 → T90 → T91 → T92
Phase 20:  T93 → T94 → T95 → T96 → T97 → T98 → T99 → T100
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B14 | 18 | T82 to T87 | after phase C closes (T81) |
| B15 | 19 | T88 to T92 | the swappable MOSFET; modules#10 and cheatsheet#8 handed to `resolved-nets-bundles` (DD2) |
| B16 | 20 | T93 to T100 | `m` and `dtemp`; T96 moves the transpile goldens once (DD1); closes L29 and L32 |

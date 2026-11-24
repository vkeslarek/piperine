# Generics Tasks (phase D: templates, `temperature()` and `multiplicity()`)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-d.md`
**Status**: Draft
**Approved**: 2026-11-23 - user: "Sim. E aceito os outros"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typestempl**: `cargo test -p piperine-lang --test types_templates`
**Run typesimplicit**: `cargo test -p piperine-lang --test types_implicit`
**Run typestraits**: `cargo test -p piperine-lang --test types_traits`
**Run elabtempl**: `cargo test -p piperine-lang --test elab_templates`
**Run elabimplicit**: `cargo test -p piperine-lang --test elab_implicit`
**Run irgen**: `cargo test -p piperine-ir --test generics`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run tgen**: `cargo test -p piperine-transpile --test generics`
**Run srctempl**: `cargo test -p piperine-parity --test source_templates`
**Run srcimplicit**: `cargo test -p piperine-parity --test source_implicit`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run elabparams**: `cargo test -p piperine-lang --test elab_params`
**Run typeshandles**: `cargo test -p piperine-lang --test types_handles`
**Run elabhandles**: `cargo test -p piperine-lang --test elab_handles`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookanalog**: `cargo test -p piperine --test book_analog`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Base and order.** Phase 18 starts after `tasks-c.md` T82 (phase C closed). The user answered DD1 to DD3 on
2026-11-23 (design-d.md "Questions answered"): phase 20 implements `temperature()` and `multiplicity()` as run settings the
model reads; DQ1 to DQ3 answered the same day (one multiplicity per run, the instance decides with its own param).
**Amended (user decision 2026-11-23, DQ4):** "É, melhor voltar Option" — `simparam(name) -> Option<f64>` (no default, `E0456`
retired), delivered by `analog-operators`' session-parameter task, not here. Refined the same day (user decision
2026-11-23): `multiplicity()` is an ordinary stdlib function written in MSL, `pub fn multiplicity() -> Option<f64> {
simparam("multiplicity") }`, nothing in the core (no `CORE_ROWS` row, no `Env` read, no ABI field), with
`DEFAULT_MULTIPLICITY = 1.0` (ngspice's default `m`) beside `AMBIENT_TEMPERATURE`; the run sets it as any `simparam`
entry (no dedicated key or flag, no `E0802`). T99 is now that stdlib task. **T99 to T101 need that `simparam` on
`main`:** T99 checks it first and stops the batch if it is missing.

Rules every task follows (the user's rules, repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, the lane letter the coordinator gives. `/var/tmp/piperine-target` is a read-only trap and is never used. Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-21).** After each task: the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, then the `Run` lines its `Done when` names. Once at the end of each batch: `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves.
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
| Interpreter refusals | integration | the run-setting reads refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Source-level runs | integration | one test per run-time AC on the Rust backend, expected values computed by Rust in the test | `crates/piperine-parity/tests/{source_templates,source_implicit}.rs` | `cargo test -p piperine-parity --test source_templates` / `--test source_implicit` |
| Spec text | integration | each amended sentence | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| Book | integration | each owned row flipped or narrowed | `tests/book/*.rs`, `tests/book/inventory.tsv` | `cargo test -p piperine --test book_<chapter>` |
| Repository records | integration | one assertion per removed `GAPS.md` row | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After every task with tests | the `**Build**` line; `cargo test -p <crate>` for each crate the task touched; the `Run` lines its `Done when` names |
| Full | The last task of each batch, and T96 and T97 (the ABI, and every `temperature()` caller) | Quick, then `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets |
| Build | A task with `Tests: none` | the `**Build**` line, then `cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 18: The spec amendment and templates

```
T83 → T84 → T85 → T86 → T87 → T88
```

### Phase 19: Module trait impls, `inst.name` through templates, the template Book rows

```
T89 → T90 → T91 → T92 → T93
```

### Phase 20: `temperature()` and `multiplicity()` (user decisions 2026-11-23)

```
T94 → T95 → T96 → T97 → T98 → T99 → T100 → T101 → T102
```

---

## Task Breakdown

### T83: The spec takes impls on module types

**What**: Amend `docs/spec` per the user's decision of 2026-11-22 and DD3: §3.7 lists a module type among the `impl Tr for` targets and §3.1 allows an inherent `impl` on one, its functions pure functions of the template (no node, quantity or time: `E0122`); the `E0130` row leaves §3.1's errors (DD3 (a)); §8.4 says a bound may combine a trait and `Fn(P)`. `spec.md` GN-03 AC 2 was amended at planning (user, 2026-11-23); `tasks.md` check C8's claim is amended here and its proof in T89. One `spec_text` assertion per sentence.
**Where**: `docs/spec/part_i_values.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs`
**Requirement**: GN-23, GN-03 (AC 2 as amended)
**Executor**: Sonnet

**Done when**:

- [x] `a_module_type_takes_trait_and_inherent_impls_in_the_spec` and `a_bound_combines_a_trait_and_fn_p_in_the_spec` pass under `**Run spectext**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): a module type takes impls; a bound combines a trait and Fn(P)`

---

### T84: Template values in the checker

**What**: `types/templates.rs` types `Name { params }` and `Name::<G> { params }` as a value of the module's type wherever a value goes (a `const`, a param value, a function result, a `let` of a const context), replacing the `P0001` of `types/expr.rs`; a required param left unbound is `E0319`; a template-valued expression that is not const is `E0362`; a template as a carried type (`Bit<Bsim4>`, a signal) is `E0112`.
**Where**: `crates/piperine-lang/src/types/templates.rs`
**Depends on**: T83
**Reuses**: `Context::Const`, `Checker::generic_placement`
**Requirement**: GN-21 (AC 1 to AC 4)
**Executor**: Sonnet

**Done when**:

- [x] `a_template_is_a_value_of_its_modules_type`, `a_template_leaving_a_required_param_is_e0319`, `a_template_that_is_not_const_is_e0362` and `a_template_is_never_carried_e0112` pass under `**Run typestempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): templates are values`

---

### T85: Templates fold from consts and function results

**What**: `TemplateFolder` (`elab/templates.rs`) folds a template from a `const`, a param, a template literal of a generic module (its `Instance` part of the interning key) and the result of a function evaluated in `ConstEval` (a binning function returns a `TemplateValue`); two equal templates are one number; a template passed through two template-typed params folds to the innermost binding.
**Where**: `crates/piperine-lang/src/elab/templates.rs`
**Depends on**: T84
**Reuses**: `ConstEval`, template interning
**Requirement**: GN-21 (AC 1), GN-22 (AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `templates_fold_from_consts_functions_and_two_levels_of_params` passes under `**Run elabtempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): templates fold from consts and function results`

---

### T86: Placement through a template value in the checker

**What**: A call whose callee is a template-valued const expression (`cell(conns)`, `nch(l, w)(d, g, s, b)`, `C1(a, b)`) types as a placement of the template's module, its connections checked against its ports, replacing the `P0001` of `types/call.rs` and `types/module.rs`; a param block on it is `E0361`.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T85
**Reuses**: `Checker::placement`, the `E0361` arm
**Requirement**: GN-22 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `a_placement_through_a_template_or_a_function_types_as_its_module` and `a_param_block_on_a_template_placement_is_e0361` pass under `**Run typestempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): placement through a template value or a function`

---

### T87: Placement through any folded template

**What**: `elab/children.rs` places the module of any folded template with its folded params (identical to `Name(conns) { params }`, §8.4.9), replacing the `P0001` "a placement through a template value"; a binning function places the module and params its arguments choose; every param a template-valued expression reads is structural (phase C's `KeyParams`).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T86
**Reuses**: `template_values`, `KeyParams`
**Requirement**: GN-22 (AC 1, AC 3), GN-20 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `a_template_placement_is_identical_to_the_written_placement` and `a_binning_function_places_the_module_it_chooses` pass under `**Run elabtempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): placement through any template`

---

### T88: `Fn(P)` bounds on modules

**What**: `types/module_bounds.rs` checks a template bound to `C: Fn(P)`: the module's ports have the types and directions of `P`, in order, names not counting, a module with a result included (§8.4.5), else `E0360` naming the first port that differs; a bound `N: Tr + Fn(P)` also needs `impl Tr for` the module (`E0187` naming the module and the trait). **Amended 2026-11-23 (generics C gap verdicts G2, G7):** a param whose type is a parameter bounded by `Fn(P)` (`cell: C`, §8.4.1) holds a template, replacing the `P0001` naming the template form that T60 leaves and T71 pins on modules#10 and cheatsheet#8; that parameter, written `_` or left out, is inferred from the template the placement binds to the param (`MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } }` infers `C = Dram1T1C`), else `E0181` naming it.
**Where**: `crates/piperine-lang/src/types/module_bounds.rs`
**Depends on**: T87
**Reuses**: `Bound::Callable`, `BoundCheck`
**Requirement**: GN-23 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `a_module_whose_ports_match_fits_fn_p_names_not_counting` and `a_module_whose_ports_differ_is_e0360_naming_the_first` pass under `**Run typestempl**`
- [x] `a_fn_bounded_parameter_is_inferred_from_its_template_param` passes under `**Run typestempl**`
- [x] Gate check passes: Full (last task of batch B14)

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): Fn(P) bounds on modules`

---

### T89: Trait and inherent impls on a module type

**What**: `Coherence` accepts `impl Mosfet for Bsim4 { .. }` and `impl Bsim4 { .. }`: `self` is the template, functions may read the template's params and return values or templates; a function that probes, reads a quantity or time is `E0122` naming it (DD3); the `E0130` arm leaves the classifier; `an_impl_on_a_module_is_e0130` is replaced by `an_impl_on_a_module_type_is_accepted` (design-d.md pins). A module bound by `N: Mosfet + Fn(..)` without the impl is `E0187` (T88) proven here with a real trait.
**Where**: `crates/piperine-lang/src/types/coherence.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ImplTable`, `FunctionClasses`
**Requirement**: GN-23 (the decision of 2026-11-22), GN-03 (AC 2 as amended)
**Executor**: Sonnet

**Done when**:

- [x] `an_impl_on_a_module_type_is_accepted` and `a_module_impl_function_that_reads_the_solve_is_e0122` pass under `**Run typestraits**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): trait and inherent impls on a module type`

---

### T90: Module `impl` functions run at elaboration

**What**: `TemplateFolder` and `ConstEval` call a module `impl` function on a template (`nmos.vth()` in a param default or a body `let`, a method returning another template for binning), with `self` bound to the folded template and its params readable as fields; static dispatch through a `N: Mosfet` bound picks the bound module's impl.
**Where**: `crates/piperine-lang/src/elab/templates.rs`
**Depends on**: T89
**Reuses**: `ImplTable::implementation`, `ConstEval`
**Requirement**: GN-23 (the decision of 2026-11-22)
**Executor**: Sonnet

**Done when**:

- [x] `a_module_impl_function_runs_on_a_template_at_elaboration` passes under `**Run elabtempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): module impl functions on templates`

---

### T91: `inst.name` through a template-typed param

**What**: `inst.name` on an instance placed through a template-typed param (`bank.cells[3][5].sn` in shape) resolves at elaboration against the concrete module the template names (§8.4.8); a missing or non-`pub` item is `E0343`.
**Where**: `crates/piperine-lang/src/elab/reach_through_templates.rs`
**Depends on**: T90
**Reuses**: `analog-structure`'s `inst.name` resolution, `TemplateFolder`
**Requirement**: GN-23 (AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `inst_name_through_a_template_param_reads_the_concrete_module` and `a_missing_item_through_a_template_param_is_e0343` pass under `**Run elabtempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): inst.name through a template-typed param`

---

### T92: Templates run end to end

**What**: Source tests on the Rust backend: the `MemoryBank`/`Dram1T1C` shape of §8.4 over `Electrical` ports (`C: Fn(Electrical, Electrical)`, generated cells, a cell's `sn` read through `bank.cells[r][c].sn`); `nch` binning at two lengths gives the chosen `vth0` (read through a param probe); the swappable MOSFET (`trait Mosfet`, two analog stand-in models, `DiffPair<N: Mosfet + Fn(..)>`) gives each model's own operating point, computed in Rust in the test.
**Where**: `crates/piperine-parity/tests/source_templates.rs`
**Depends on**: T91
**Reuses**: `crates/piperine-parity/tests/templates/*.msl`
**Requirement**: GN-21, GN-22, GN-23
**Executor**: Sonnet

**Done when**:

- [x] `a_memory_bank_places_its_cell_template`, `binning_places_the_chosen_model` and `the_swappable_mosfet_runs_each_model` pass under `**Run srctempl**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): templates, binning and the swappable MOSFET`

---

### T93: The template Book rows

**What**: Flip modules#12 (`nch(0.13u, 2u)(d, g, s, b)` places `Bsim4` with the binned `vth0`); add the swappable MOSFET listing to the modules chapter after the binning paragraph (DD6) with an inventory row `run` and its test; narrow modules#10 and cheatsheet#8 to the bundle `Cell` (their test asserts only the bundle's refusal; from the template and bundle pins T71/T77 of phase C leave) and hand their §11 rows to `resolved-nets-bundles` under L25 (DD2); `inventory.tsv` and `GAPS.md` §11 follow.
**Where**: `tests/book/modules.rs`
**Depends on**: T92
**Reuses**: `Chapter`, `Refusal`
**Requirement**: GN-31 (AC 2), GN-22, GN-23
**Executor**: Sonnet

**Done when**:

- [x] `a_function_that_returns_a_template_places_its_result`, `the_swappable_mosfet_listing_runs` and `the_memory_bank_is_refused_only_for_its_bundle` pass under `**Run bookmodules**`
- [x] `the_template_card_is_refused_only_for_its_bundle` passes under `**Run bookcheatsheet**`
- [x] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [x] Gate check passes: Full (last task of batch B15)

**Tests**: integration
**Gate**: full

**Commit**: `test(book): binning and the swappable MOSFET run; the bundle rows go to resolved-nets-bundles`

---

### T94: The spec: `temperature()` and `multiplicity()` are run settings the model reads

**What**: Amend `docs/spec` to the user's decision of 2026-11-23 (spec.md GN-24 to GN-26 as amended): §8.5.1 to §8.5.6 are replaced by "an instance has no implicit params; `temperature() -> Option<f64>` is `Some(t)`, in kelvin, when the run sets its temperature and `None` otherwise; `multiplicity() -> Option<f64>` is a stdlib function, `simparam("multiplicity")`: `Some(m)` when the run sets the session parameter `multiplicity`, `None` otherwise (DQ4, nothing in the core); nothing scales or offsets a model: the model decides whether to use the value as its temperature all along, as an initial temperature, or to ignore it"; §14.4's `temperature()` row (environment, constant during an analysis; `multiplicity()` is not a core row); §22's constants gain `AMBIENT_TEMPERATURE = 300.15` (27 °C, ngspice's default) and `DEFAULT_MULTIPLICITY = 1.0` (ngspice's default `m`), with the sentences that `temperature().unwrap_or(AMBIENT_TEMPERATURE)` and `multiplicity().unwrap_or(DEFAULT_MULTIPLICITY)` are the recommended fallbacks, `vt()`'s entry says it uses the temperature fallback, and §22 gains the `multiplicity()` entry (`simparam("multiplicity")`); §23's run request gains no key: the multiplicity is a `simparam` entry (`simparam = { multiplicity = 4.0 }`, `--simparam multiplicity=4.0`), one per run (no per-instance setting), and §8.5 shows the instance pattern: an ordinary param `m: Option<f64>` combined in a body `let` as `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)` (not in the param's default, a const context); §8.4.2's sentence on `m` and `dtemp` in a template, the `E0375` row and §8.5's `E0317` row for `m` leave; §8.5.12 and the `"osdi"` contract row give the descriptor the run's values when set and its defaults otherwise; Appendix G.54 is marked superseded. One `spec_text` assertion per sentence.
**Where**: `docs/spec/part_iii_structure.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs`
**Requirement**: GN-24, GN-25, GN-26 (amended, user decision 2026-11-23)
**Executor**: Sonnet

**Done when**:

- [x] `instances_have_no_implicit_params_in_the_spec`, `temperature_and_multiplicity_are_run_settings_in_the_spec` and `the_recommended_fallback_is_the_ambient_temperature_in_the_spec` pass under `**Run spectext**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): temperature() and multiplicity() are run settings the model reads`

---

### T95: IR read of whether the run set its temperature

**What**: `Env::TemperatureGiven` (a `bool`) is appended to `Env`, constant during an analysis; `Env::Temperature` keeps its meaning (the run's temperature, the solver's 300.15 K when the run sets none), so `temperature()` lowers to the `Option` pair `(TemperatureGiven, Temperature)`; `Ir::check` types it, the IR printer shows it, the interpreter refuses it by name (R1). No `Env` read for the multiplicity: the stdlib `multiplicity()` reads `simparam`'s session parameters (`analog-operators`, DQ4).
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T94
**Reuses**: `Env`, `Typing`, the `Option` pair of explicit-unknowns
**Requirement**: GN-24 (AC 1), GN-25 (AC 1), GN-29 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `run_setting_reads_are_built_checked_and_printed` passes under `**Run irgen**`
- [x] `each_generics_form_is_refused_by_name` covers the read under `**Run interp**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): the read of whether the run set its temperature`

---

### T96: The device reads whether the run set its temperature

**What**: The device ABI's environment gives whether the run set its temperature (appended in place at `MINOR` 0, MD-78 item 4; `abi.md` amended in the same task); the solver fills it from the run's options (`temp` given or not); the transpiler prints the read (an `rt` environment accessor). A body that does not read it prints as before. Nothing for the multiplicity (DQ4).
**Where**: `crates/piperine-abi/src/raw.rs`
**Depends on**: T95
**Reuses**: the environment record of `piperine-abi`, `piperine-analog-solver/src/tolerances.rs`, `piperine-transpile/src/rt/`
**Requirement**: GN-24 (AC 1), GN-25 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `a_device_reads_whether_the_run_set_its_temperature` passes under `**Run tgen**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(abi): the environment says whether the run set its temperature`

---

### T97: The checker and the elaborator: `temperature()` an option, every caller migrated

**What**: `temperature()` is typed `Option<f64>` (`CORE_ROWS`), environment class "constant during an analysis" (allowed in `analog`, reactions, body `let`s and value inputs; a const context stays `E0188`), lowered per T95; no `multiplicity()` row is added (it is a stdlib function, T99); the POM's `BuiltinKind::ImplicitParam` rows `m` and `dtemp` leave, so the names are ordinary and binding `m` at a placement is `E0320` like any unknown param. `std/constants.msl` gains `pub const AMBIENT_TEMPERATURE: f64 = 300.15;` with its doc comment (27 °C, ngspice's default, the recommended fallback of `temperature()`), and every in-tree caller of `temperature()` outside the Book and `docs/spec` (`std/analog.msl`'s `vt()` and models (user decision 2026-11-23, "Sim. E aceito os outros": `vt()` becomes `vt(t: f64) -> f64` and a model passes the temperature it chose, `vt(temperature().unwrap_or(AMBIENT_TEMPERATURE))`; the spice-faithful models, the Book, the tests and the spec's `vt()` entry follow, and the twins keep their numbers), the spice-faithful models under `std/`, `tests/e2e/twins/temp_res.msl`, examples, the `piperine-lang`, `piperine-api` and `piperine-parity` test sources) reads `temperature().unwrap_or(AMBIENT_TEMPERATURE)`; every twin prints the same bytes and every parity target passes with `ToleranceContract` unedited.
**Where**: `crates/piperine-lang/src/types/catalogue.rs`
**Depends on**: T96
**Reuses**: `CoreSig`, `elab/analog.rs`
**Requirement**: GN-24 (AC 2), GN-25 (AC 1, AC 2, AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `temperature_is_an_option_of_the_run` and `m_and_dtemp_are_ordinary_names` pass under `**Run typesimplicit**`
- [x] `the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled` passes under `**Run elabimplicit**`
- [x] `every_twin_prints_the_bytes_it_printed_before_the_move` passes under `**Run twins**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): temperature() is an option; callers fall back to the ambient temperature`

---

### T98: The Book and the spec examples use the fallback

**What**: Every Book listing (`docs/book/content.html`) and `docs/spec` example that reads `temperature()` writes `temperature().unwrap_or(AMBIENT_TEMPERATURE)` (or uses the `Option` as its model chooses); the Book's environment section says what `temperature()` and `multiplicity()` return and recommends the ambient fallback, ngspice's 27 °C, and `multiplicity().unwrap_or(DEFAULT_MULTIPLICITY)`; the Book tests of those listings (`tests/book/analog.rs`, `tests/book/solver.rs`, …) and `inventory.tsv` fingerprints follow; every row that ran still runs.
**Where**: `docs/book/content.html`
**Depends on**: T97
**Reuses**: `Chapter`
**Requirement**: GN-25 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `the_temperature_listings_run_with_the_ambient_fallback` passes under `**Run bookanalog**`
- [x] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): temperature() is an option with the ambient fallback`

---

### T99: The stdlib `multiplicity()` over `simparam`

**What**: First check that `analog-operators`' `simparam(name) -> Option<f64>` (no default argument) and the `simparam` table and `--simparam` flag are on `main`; if not, stop the batch and report. Then (user decision 2026-11-23, "É, melhor voltar Option", refined the same day: a stdlib function, nothing in the core) `std/analog.msl` gains `pub fn multiplicity() -> Option<f64> { simparam("multiplicity") }` beside `vt()`, and `std/constants.msl` gains `pub const DEFAULT_MULTIPLICITY: f64 = 1.0;` beside `AMBIENT_TEMPERATURE`, each with its doc comment (ngspice's default `m`; the recommended fallback `multiplicity().unwrap_or(DEFAULT_MULTIPLICITY)`). No `CORE_ROWS` row, no `Env` read, no ABI field, no `Request` field, no `--multiplicity` flag, no `E0802`: the run sets it as any `simparam` entry (`simparam = { multiplicity = 4.0 }`, `--simparam multiplicity=4.0`).
**Where**: `crates/piperine-lang/std/analog.msl`
**Depends on**: T98
**Reuses**: `simparam` (`analog-operators`), `std/constants.msl`
**Requirement**: GN-24 (AC 1, AC 3, AC 6)
**Executor**: Sonnet

**Done when**:

- [x] `multiplicity_is_a_stdlib_function_over_simparam` passes under `**Run typesimplicit**`
- [x] `multiplicity_lowers_to_the_session_parameter` passes under `**Run elabimplicit**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(std): multiplicity() over simparam and DEFAULT_MULTIPLICITY`

---

### T100: Temperature and multiplicity run end to end

**What**: Source tests: a resistor that declares its own `m: Option<f64>` and contributes `count * v(a, b) / r` with `let count = m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY);` in its body (DQ1: a body `let`, since its default is a const context where `multiplicity()` is `E0188` through `simparam`, which the test also asserts) gives four in parallel placed with `m: Some(4.0)`, follows the run's `simparam = { multiplicity = 3.0 }` with no `m`, prefers its own `m` over the run's, and with nothing set gives one resistor; a second resistor in the same run with no `m` reads the same run multiplicity; three models, one using the run's temperature all along, one using it as the initial temperature of a thermal state, one ignoring it, each give the value Rust computes in the test, with `temp` set and with it unset (then `None`, and the fallback model runs at 300.15 K); the run's `temp` and `multiplicity` entry hold for the whole run, and there is no placement or `set` spelling of either (GN-26 AC 1, AC 2).
**Where**: `crates/piperine-parity/tests/source_implicit.rs`
**Depends on**: T99
**Reuses**: `crates/piperine-parity/tests/implicit/*.msl`, the stdlib `multiplicity()` (T99)
**Requirement**: GN-24 (AC 1, AC 4, AC 5), GN-25 (AC 1, AC 2), GN-26 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel`, `multiplicity_in_a_param_default_is_e0321`, `nothing_set_reads_none`, `a_model_chooses_how_to_use_its_temperature` and `the_run_settings_hold_for_the_whole_run` pass under `**Run srcimplicit**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): temperature() and multiplicity() as the model uses them`

---

### T101: The Book's multiplicity row, rewritten

**What**: Rewrite the modules chapter's "`m` and `dtemp`" item to the new form (a model reads `multiplicity()` and `temperature()`, the run sets them, nothing is implicit), with a listing (a resistor with its own `m: Option<f64>` combined in its body as `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)`, placed as `Resistor(a, b) { r: 1k, m: Some(4.0) }`, and a sentence that a run's `--simparam multiplicity=<v>` reaches every instance that leaves `m` unset) and its "Run it" output; re-own claim modules~1faf8c12 to that text (`holds`, test `the_multiplicity_listing_runs_as_four_in_parallel`), and its row leaves `GAPS.md` §11 (GN-26 AC 3).
**Where**: `tests/book/modules.rs`
**Depends on**: T100
**Reuses**: `Chapter`, `Commands`
**Requirement**: GN-26 (AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `the_multiplicity_listing_runs_as_four_in_parallel` passes under `**Run bookmodules**`
- [x] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): multiplicity as the model reads it`

---

### T102: Phase D bookkeeping

**What**: `GAPS.md` drops L29 (closed by the run settings of the user's decision) and L32, updates §9 (III §8.4–§8.5) and the §10 code list (`E0360`, `E0362` leave `code_owners.tsv`; `E0375` leaves with its spec row); `spec.md`'s traceability marks GN-21 to GN-26 phase D; `.specs/STATE.md` MD-81 gains item 7 (design-d.md "Tech Decisions"); `ROADMAP.md`'s `generics` D row says delivered.
**Where**: `GAPS.md`
**Depends on**: T101
**Reuses**: `docs.rs`
**Requirement**: GN-31 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `gaps_drops_what_generics_phase_d_closed` and `state_records_templates_and_run_settings` pass under `**Run docs**`
- [x] Gate check passes: Full (last task of batch B16)

**Tests**: integration
**Gate**: full

**Commit**: `docs: generics phase D closes L29 and L32`

---

## Checks

**C87** - `Name { params }` and `Name::<G> { params }` as a `const`, a param value or a function result give a value of the module's type. (GN-21, AC 1; T84, T85)
Proof: run typestempl | test a_template_is_a_value_of_its_modules_type ... ok
Proof: run elabtempl | test templates_fold_from_consts_functions_and_two_levels_of_params ... ok
Proof: run elabtempl | test a_template_of_a_generic_module_names_its_arguments ... ok
Proof: run elabtempl | test two_templates_of_one_generic_module_with_different_arguments_are_two_values ... ok
Proof: run elabtempl | test a_template_of_a_generic_module_obeys_its_where_and_names_its_missing_arguments ... ok

**C88** - A template leaving a required param unbound is `E0319`; an `Option<T>` param with no default is not required (§8.1.17), one rule with the placement's (`ParamRequirement`; fix round 3, finding 13). (GN-21, AC 2; T84)
Proof: run typestempl | test a_template_leaving_a_required_param_is_e0319 ... ok
Proof: run typestempl | test a_template_leaves_an_option_param_out_as_a_placement_does ... ok
Proof: run elabtempl | test a_template_that_leaves_an_option_param_out_places_none ... ok
Proof: run srctempl | test a_const_template_leaves_an_option_param_out ... ok

**C89** - A template-valued expression that is not const is `E0362`. (GN-21, AC 3; T84)
Proof: run typestempl | test a_template_that_is_not_const_is_e0362 ... ok

**C90** - A template is never carried: `E0112`. (GN-21, AC 4; T84)
Proof: run typestempl | test a_template_is_never_carried_e0112 ... ok

**C91** - A placement through a template value or a function result places an instance identical to the written placement. (GN-22, AC 1; T86, T87, T93)
Proof: run typestempl | test a_placement_through_a_template_or_a_function_types_as_its_module ... ok
Proof: run elabtempl | test a_template_placement_is_identical_to_the_written_placement ... ok
Proof: run bookmodules | test a_function_that_returns_a_template_places_its_result ... ok

**C92** - A param block on a placement through a template value is `E0361`. (GN-22, AC 2; T86)
Proof: run typestempl | test a_param_block_on_a_template_placement_is_e0361 ... ok

**C93** - A binning function places the module and params its arguments choose. (GN-22, AC 3; T87, T92)
Proof: run elabtempl | test a_binning_function_places_the_module_it_chooses ... ok
Proof: run srctempl | test binning_places_the_chosen_model ... ok

**C94** - A module whose port types and directions match `P` fits `C: Fn(P)`, names not counting. (GN-23, AC 1; T88, T92)
Proof: run typestempl | test a_module_whose_ports_match_fits_fn_p_names_not_counting ... ok
Proof: run srctempl | test a_memory_bank_places_its_cell_template ... ok
Proof: run typestempl | test a_fn_bounded_parameter_is_inferred_from_its_template_param ... ok

**C95** - A module whose ports differ from `P` is `E0360` naming the first port that differs. (GN-23, AC 2; T88)
Proof: run typestempl | test a_module_whose_ports_differ_is_e0360_naming_the_first ... ok

**C96** - `inst.name` through a template-typed param resolves against the concrete module, in the module that places the template and from its parent, `E0343` for a missing or private item; an item of any kind (a `pub` param, a `pub let`, a carrier) is read as a value with the type its context wants, checked against the concrete item at elaboration (`E0180` naming both; `E0181` where no context gives one: a method's receiver, the operand of `as`, a `let` with no type that nothing reads, fix round 3 findings 14 and 15), also from the parent (`bank.c0.cs`) and at a generated iteration (`bank.cells[1].cs`); an out-of-range generated cell is `E0342` naming its element and the length of its own loop. (GN-23, AC 3; T91; fix rounds 1 and 2, L78)
Proof: run elabtempl | test inst_name_through_a_template_param_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_missing_item_through_a_template_param_is_e0343 ... ok
Proof: run elabtempl | test inst_name_in_the_module_that_places_the_template_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_non_node_item_through_a_template_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_non_node_item_through_a_template_of_another_type_is_e0180_at_elaboration ... ok
Proof: run elabtempl | test a_generated_cells_param_reads_the_iteration_its_indexes_name ... ok
Proof: run typestempl | test a_non_node_item_through_a_template_takes_the_type_its_context_wants ... ok
Proof: run typestempl | test a_cast_of_a_template_item_is_e0181 ... ok
Proof: run elabtempl | test a_cast_of_a_template_item_is_e0181_not_a_node_conversion ... ok
Proof: run typestempl | test an_unread_let_of_a_template_item_is_e0181 ... ok
Proof: run elabtempl | test an_unread_let_of_a_template_item_is_e0181_not_an_elaboration_refusal ... ok
Proof: run srctempl | test the_items_of_a_template_placed_child_read_as_values ... ok
Proof: run spectext | test a_template_item_takes_the_type_its_context_wants_in_the_spec ... ok
Proof: run docs | test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok
Proof: run elabtempl | test an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop ... ok

**C97** - A module type takes trait and inherent impls of pure functions over its template, run at elaboration; a trait plus `Fn(P)` bound admits only a module that implements the trait; a function that reads the solve is `E0122`. (GN-23; the decision of 2026-11-22; T83, T89, T90, T92)
Proof: run spectext | test a_module_type_takes_trait_and_inherent_impls_in_the_spec ... ok
Proof: run spectext | test a_bound_combines_a_trait_and_fn_p_in_the_spec ... ok
Proof: run typestraits | test an_impl_on_a_module_type_is_accepted ... ok
Proof: run typestraits | test a_module_impl_function_that_reads_the_solve_is_e0122 ... ok
Proof: run elabtempl | test a_module_impl_function_runs_on_a_template_at_elaboration ... ok
Proof: run elabtempl | test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok
Proof: run srctempl | test the_swappable_mosfet_runs_each_model ... ok

**C98** - `multiplicity()` returns `Some` of the run's `multiplicity` session parameter and `None` when the run sets none; nothing is scaled by the system. (GN-24, AC 1, AC 2; amended twice 2026-11-23, DQ4; T97, T100)
Proof: run elabimplicit | test the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled ... ok
Proof: run srcimplicit | test nothing_set_reads_none ... ok

**C99** - `multiplicity()` is a stdlib function, `simparam("multiplicity")`, with `DEFAULT_MULTIPLICITY = 1.0` beside it: no core row, no dedicated key, flag or check, no environment read (`E0802` for it withdrawn; user decision 2026-11-23). (GN-24, AC 3, AC 6; T99)
Proof: run typesimplicit | test multiplicity_is_a_stdlib_function_over_simparam ... ok
Proof: run elabimplicit | test multiplicity_lowers_to_the_session_parameter ... ok
Proof: run elabimplicit | test a_function_that_reaches_the_session_parameter_at_any_depth_reads_it_in_the_body ... ok

**C100** - A resistor with its own `m: Option<f64>` combined with `multiplicity()` in its body is four in parallel placed with `m: Some(4.0)` and follows the run's one multiplicity without it; reading `multiplicity()` in a param default is `E0321`, the code of §8.1.16 and §8.5.5 (`E0188` is the `const` item's). (GN-24, AC 4, AC 5; T100)
Proof: run srcimplicit | test an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel ... ok
Proof: run srcimplicit | test multiplicity_in_a_param_default_is_e0321 ... ok

**C101** - `temperature()` is `Some` of the run's temperature in kelvin or `None`, and the model chooses constant, initial or ignored; the docs recommend the ambient fallback. (GN-25, AC 1, AC 2; T94, T97, T98, T100)
Proof: run typesimplicit | test temperature_is_an_option_of_the_run ... ok
Proof: run srcimplicit | test a_model_chooses_how_to_use_its_temperature ... ok
Proof: run spectext | test the_recommended_fallback_is_the_ambient_temperature_in_the_spec ... ok
Proof: run bookanalog | test the_temperature_listings_run_with_the_ambient_fallback ... ok

**C102** - `m` and `dtemp` are ordinary names: no implicit param exists. (GN-25, AC 3; T94, T97)
Proof: run typesimplicit | test m_and_dtemp_are_ordinary_names ... ok
Proof: run spectext | test instances_have_no_implicit_params_in_the_spec ... ok
Proof: run spectext | test temperature_and_multiplicity_are_run_settings_in_the_spec ... ok

**C103** - The run sets its temperature and its `multiplicity` session parameter before it starts, they hold for the whole run, and neither has a placement or `set` spelling. (GN-26, AC 1, AC 2; T100)
Proof: run srcimplicit | test the_run_settings_hold_for_the_whole_run ... ok

**C104** - The Book's multiplicity listing runs as four in parallel and its row leaves `GAPS.md` §11. (GN-26, AC 3; T101)
Proof: run bookmodules | test the_multiplicity_listing_runs_as_four_in_parallel ... ok

**C105** - The interpreter reads the temperature-given flag (it is no ambient special case); a template leaves no IR form; the device reads whether the run set its temperature. (GN-29, AC 1; T95, T96, T87)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run irgen | test run_setting_reads_are_built_checked_and_printed ... ok
Proof: run tgen | test a_device_reads_whether_the_run_set_its_temperature ... ok

**C106** - `GAPS.md` drops L29 and L32 and §9 and §10 follow. (GN-31, AC 1; T102)
Proof: run docs | test gaps_drops_what_generics_phase_d_closed ... ok

**C107** - The template Book rows flip or narrow to the bundle they still need, and the swappable MOSFET listing runs. (GN-31, AC 2; T93)
Proof: run bookmodules | test the_swappable_mosfet_listing_runs ... ok
Proof: run bookmodules | test the_memory_bank_is_refused_only_for_its_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_only_for_its_bundle ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C108** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; T97, every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C110** - A param default reads the params declared before it, folded per instance over the values they take there, a template param's trait method included (`level: f64 = fet.gain()`); the params a later default reads are structural; a default that reads the param itself or a later one is `E0321`. (§8.1.2, §8.1.16; fix round 2, L80)
Proof: run typestempl | test a_param_default_reads_earlier_params_and_a_template_params_trait ... ok
Proof: run elabparams | test a_default_that_reads_earlier_params_is_folded_per_instance ... ok
Proof: run elabparams | test a_default_that_reads_itself_or_a_later_param_is_e0321 ... ok
Proof: run elabtempl | test a_param_default_dispatches_the_trait_of_a_template_param ... ok
Proof: run srctempl | test a_param_default_reads_the_params_before_it ... ok
Proof: run spectext | test a_param_default_reads_earlier_params_in_the_spec ... ok
Proof: run docs | test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok

**C111** - The type of a template of a generic module carries its arguments, `G<2>`: a template of other arguments where it is wanted is `E0180` naming both, an argument the template leaves out is the wanted type's, and a placement through a template (in place, a `const`, a param of type `S<E>`, a param bounded by `Fn(..)`) of a module whose ports are typed by its generic parameters reads them under those arguments (`E0251` for a node of another nature). (§8.4.3, §8.4.5; fix round 2, L79)
Proof: run typestempl | test the_type_of_a_generic_template_carries_its_arguments ... ok
Proof: run typestempl | test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok
Proof: run elabtempl | test a_placement_through_a_template_of_a_nature_generic_module_places_its_instance ... ok
Proof: run srctempl | test a_template_of_a_nature_generic_module_runs_through_its_type ... ok
Proof: run docs | test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok

**C112** - A placement through a template param bounded by `Fn(P)` takes at each node input what a placement of the module takes at that port: `gnd` or a node of a compatible nature, `E0251` for another nature. (§8.1.10, §8.4.5; fix round 2, coordinator follow-up)
Proof: run typestempl | test gnd_connects_through_a_callable_bound_as_at_a_placement ... ok
Proof: run elabtempl | test gnd_at_a_placement_through_a_callable_bound_is_ground ... ok
Proof: run srctempl | test gnd_connects_through_a_callable_bound ... ok

**C113** - A `let` bound to a node keeps the node's nature: `v(y)` after `let y = x;` probes `x` by its nature's access, in a plain module, through a child placed by name and through a template, and lowers to the node's probe. (§6 rule 4, §8.4.8; fix round 4, finding 16)
Proof: run typeshandles | test a_let_bound_to_a_node_keeps_its_nature_for_a_probe ... ok
Proof: run elabhandles | test a_let_bound_to_a_node_probes_the_node ... ok
Proof: run srctempl | test a_let_bound_to_a_node_probes_the_node ... ok

**C114** - A `let` bound to a node is a contribution target as the node is: `let y = x; v(y) +> 1.0;` lowers to `v(x) +> 1.0;`, and through a template to the reached node's potential; the role of an access the resolver left ambiguous is the checker's (`Types::access_role`). (§6 rule 4, §11; fix round 5, finding 17)
Proof: run typeshandles | test a_let_bound_to_a_node_is_a_contribution_target ... ok
Proof: run elabhandles | test a_contribution_through_a_let_bound_to_a_node_is_the_nodes ... ok
Proof: run srctempl | test a_let_bound_to_a_node_is_a_contribution_target ... ok

**C115** - Through a `let` bound to a node an access is judged on the node's nature as for the node named directly: one the nature lacks is `E0206`, a flow contribution on a nature without `flow` is `E0413`. (§6 rule 4, §6 rule 10, §11 rule 11; fix round 5, finding 18)
Proof: run typeshandles | test an_access_a_let_bound_nodes_nature_lacks_is_refused_as_for_the_node ... ok
Proof: run elabhandles | test an_access_a_let_bound_nodes_nature_lacks_is_e0206 ... ok

**C116** - Parentheses around a node argument name the node: `v((x))`, `v((y))`, `v((c0.sn))` and `v((q)) +> ..` probe and contribute as without them, also through a template. (§1.5, §6 rule 4; fix round 5, verifier's note)
Proof: run typeshandles | test parentheses_around_a_node_argument_name_the_node ... ok
Proof: run elabhandles | test parentheses_around_a_node_argument_lower_to_the_node ... ok

**C117** - A node chosen at elaboration (an `if`, a `match` or a block whose choice reads only params, constants, generation indexes and `let`s of those, whatever statements its arms hold; fix round 8, finding 22) names the node it folds to at a probe, a contribution target, a placement's connection and an `Fn` bound's, and its params are structural; a choice that reads a probe, a signal, a core primitive of the run or a variable whose value depends on the run is `E0133` alone, also written straight at a node argument, and so is one through a function that reads the run (elaboration). (§1.4.7, §8.1.2; user ruling 2026-11-24, finding 21; findings 20 and the round-5 note)
Proof: run typeshandles | test a_node_chosen_by_the_run_is_e0133 ... ok
Proof: run typeshandles | test a_node_chosen_at_elaboration_is_typed_as_the_node ... ok
Proof: run typeshandles | test a_node_chosen_at_a_node_argument_is_e0133_alone ... ok
Proof: run elabhandles | test a_node_chosen_at_run_time_is_e0133_not_an_elaboration_refusal ... ok
Proof: run elabhandles | test a_node_chosen_at_a_probe_or_a_target_is_e0133_alone ... ok
Proof: run elabhandles | test a_node_chosen_at_elaboration_folds_to_the_node ... ok
Proof: run elabhandles | test a_node_chosen_at_elaboration_connects_through_a_template ... ok
Proof: run elabhandles | test a_node_chosen_through_a_function_that_reads_the_run_is_e0133 ... ok
Proof: run elabhandles | test a_node_chosen_through_lets_and_arm_statements_folds_to_the_node ... ok
Proof: run srctempl | test a_param_picks_the_node_a_module_probes_and_drives ... ok
Proof: run spectext | test a_node_chosen_by_an_expression_is_e0133_in_the_spec ... ok

**C109** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

---

## Handoff

The phase's live state, written as it happens - a compaction gives no warning.

**Settled mid-build (B16, 2026-11-24, branch `feature/generics-b16`, T94 to T102 committed and pushed):** (1) `temperature().unwrap_or(AMBIENT_TEMPERATURE)` lowered to the temperature read itself through a peephole (`is_ambient_fallback`), REMOVED at the merge by the coordinator's ruling (no language special cases): it lowers as the ordinary `Option` (`temperature_given()` selects between `temperature()` and 300.15), the interpreter reads the given flag, and the diode goldens, pins and read lists moved with it while every twin's numbers stayed; before the removal only `vt(t)`'s own text moved (`Devices::vt` takes a param; ir goldens `diode.txt` and `full/diode.txt`, transpile golden `diode.rs`, `fixtures/elab/diode_*.txt`). Any other fallback reads the pair `(temperature_given, temperature)`. (2) The given flag is a `GLOBAL TEMPERATURE_GIVEN` (10) `BIT` pin of the device ABI (not an ambient cell; in place at MINOR 0, abi.md amended), seeded and written by `bind.rs`, set by `Tolerances::set_temperature` (the API `temp` key calls it; parity tests that wrote `tolerances.temperature` directly now call it). (3) `multiplicity()` is std MSL; a function whose body calls `simparam` is inlined at its call (`records.rs::is_inlined`), because the IR keeps session reads in bodies, never in functions. (4) Deviations: a `multiplicity()` read in a param default is `E0321` (the §8.1.16 code), not `E0188`; spec 8.5.5 and `multiplicity_in_a_param_default_is_e0321` (name kept) say so. Std items carry no doc comment (`std_excerpts` matches item text verbatim); the docs live in the spec table. B.1 now counts 60 constructs (rows 50 and 51 left; docs/pom/mapping.md renumbered). The Book listing is `modules#17` after the merge with B14 and B15 (it was `modules#15` on the branch). T102 drops L29 only: **L32 stays in `GAPS.md` until the template lane (T91 to T93) lands, whose merge drops the L32 row and its `modules#10`/`cheatsheet#8` rows** and the coordinator renumbers MD-81 item 7 if the other lane adds one. `book_compiler::a_compiler_code_says_what_the_chapter_says` reports `compiler~79f44db1` now runs (a template in a `let`, from T88 to T90, not this batch). Loud count 16 (L29 closed): the pinned counts in `crates/piperine-cli/tests/docs.rs` follow.

**Settled mid-build (B14 and B15 so far, 2026-11-24):** T83 to T90 are committed and pushed (head b1168fe50). Deviations: (1) `E0130` left the POM classifier, `pom_classify` and `docs/pom/resolution.md` in T84's commit, not T89's, because T83's spec edit turned `pom_docs` red; its tests were replaced (`impl_on_module_is_no_classifier_error`, `an_impl_on_a_module_type_is_accepted`). (2) T84 types a template of a non-generic module (`Ty::Module`); `Name::<G> { .. }` and the `Instance` part of the interning key (T85) wait for a generic module's template to be typed (nothing in phase D's checks needs it). (3) The template interner moved from `Elaboration` to `ConstEval` (`consts/templates.rs`), so a const, a param default and a function result fold to the same number as a placement does; `bound_implementation` moved to `consts/dispatch.rs` (Elaboration delegates). (4) T88 pulled the modules#12 flip forward (`a_function_that_returns_a_template_places_its_result`, inventory row `run`, GAPS row left) and narrowed modules#10 and cheatsheet#8 to what they print now (the bundle and one placement through `cell`); T93 finishes them and adds the swappable-MOSFET listing. (5) `E0360` and `E0362` left `code_owners.tsv` and GAPS section 10 (count 12 of 189). **T91 is open and is the hard one:** `bank.c0.sn` through `cell: C` (`C: Fn(..)`) needs (a) the checker to record a placement through a callable param (no `Callee` exists for it yet: `call_through_bound` returns the bound's result), (b) `types/template.rs`' `template_value` to `reach(module)` the template's module, (c) `Elaboration::is_module_param` and `children.rs::lower_children` to place a call whose module is only known from the folded template (`template_through` already folds the head; `lower_children` skips a call with no `Callee::Module`), (d) the POM `reached_access` and the checker's probe path to leave `v(bank.c0.sn)` deferred and the elaborator to resolve it against the concrete module (today: P0001 "probe through an access that names several natures"), `E0343` raised by the elaborator. T92 and T93 depend on it. Gate lesson: the shell tool moves any command past ~170 s to the background; poll a log file with a loop shorter than 170 s. Planning amendment (user decision 2026-11-23, DQ4, "É, melhor voltar Option"): `simparam(name) -> Option<f64>` (built by `analog-operators`), and, refined the same day (user decision 2026-11-23), `multiplicity()` is a stdlib MSL function over it (`pub fn multiplicity() -> Option<f64> { simparam("multiplicity") }`) with `DEFAULT_MULTIPLICITY = 1.0`: nothing in the core, no `multiplicity` key/flag, no `E0802`, no `Env::Multiplicity`/`MultiplicityGiven`, no ABI field; the pattern is `let count = m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY);`; T99 is the stdlib task; phase D stays 20 tasks. Planning (2026-11-23): the defaults of design-d.md DD4 to DD6. User, 2026-11-23: DD2 (a) and DD3 (a) approved; DD1 rejected — "Não temos a semântica de fazer set e de criar nada implícito em lugar algum... O que podemos fazer é um method chamado temperature() que retorna Option e multiplicity(). Eles retornam o que foi setado na simulação e o modelo que escolhe se vai usar como temperatura o tempo todo, se vai usar como temp inicial ou se vai ignorar"; then "é meio que um param do run (session)"; one `temperature() -> Option<f64>` replaces the old one, docs recommend the ambient fallback (ngspice's 27 °C); `multiplicity() -> Option<f64>`. DQ1 to DQ3 answered the same day: "Um por run e a instância decide no seu próprio parâmetro"; one temperature for every instance; `tnom` untouched.
**T91 (done):** `bank.c0.sn` is a read through a child of a child, which no elaboration supported (concrete or template): the module of `bank` now lowers a `Reached` node for each `child.item` the design reads (`elab/reach_through_templates.rs`, demands found once, slots right after the module's own nodes), the reader reaches that node, `place.rs` hands a reached node's binding up. Also: a generic module's module-param keys are classified (`classify_module_param_keys`), `is_module_param` holds for a `C: Fn(..)` param, `placed_module` names a call's module from the folded template, the checker `reach`es a template's module and types `v(c.x)` through a callable param as a potential probe (flow and contributions through it stay refused). A module declaring a node in generation cannot be read through (refused loud).
**T92 (done):** `bank.cells[r][c].sn` reads a child placed in generation: the bank's body lowers one reached node per iteration (`DemandEntry`), the reader finds its slot by expanding the bank's generation under the bank's keys (`entries_under`). The swappable MOSFET listing calls `SQUARE.gain()` in a param default of the rig (a method on a param template inside a `let` or a param default, `fet.gain()`, is not typed yet: `a param default that reads a param` / `an expression the type checker gave no value type`), and `DiffPair<N: Mosfet + Fn(..)>` places `fet`. T93 must write the Book listing in that shape.
**T93 (done):** modules#14 (the swappable MOSFET listing, run) and modules#15 (its placement, fragment) are new Book blocks after the binning fragment, so the extern block is now modules#16 (inventory, GAPS section 11 and its test follow); the Book counts 289 blocks (129 `msl`). modules#10 and cheatsheet#8 are narrowed to the bundle `Cell` and its literal (two `P0001`: the type and `struct expression`), their rows go to L25 / `resolved-nets-bundles`; the cascade `placement through a template value or a function` on a param whose `Fn` bound failed to type is gone (`fn_bound_failed`). The L32 row itself and the GAPS counts are left to T102. For the merge with B16: `tests/book/inventory_guard.rs` block counts, `GAPS.md` counts line and `docs/book/content.html` both lanes touch.
**Fix round 1 (2026-11-24, verification-d.md findings 1 to 12):** (1) `v(c0.sn)` in the module that places the template resolves against the folded template's module (`elab/reach.rs::template_item`). (2) A non-node item through a template cannot be typed before the template folds, so it is a named `P0001` (L78), never `E0180`/`E0417` (`Checker::template_reads`). (3) `fet.gain()` through `N: Mosfet + Fn(..)` runs at elaboration on the template (`lower.rs::is_template_method`) in a body `let` and in `analog`; the parity `DiffPair` drives `s` with it. A param default that reads the param `fet` is the general §8.1.16 gap (L80). (4) A template of a generic module names its arguments (`types/templates.rs`, `TemplateValue::arguments`, `placement_instance`); `Ty::Module` still carries none (L79). (5) `reads_session_parameter` follows calls transitively. (6) to (9), (12): records, decision log, check text, doc comments and the `#[link]` example amended. (10), (11): `E0342` names the loop that is out of range, `add_nested_node` takes a `NestedRead`. A placement of a generic module that records no instance is a named `P0001`, never a silent drop.
**Fix round 2 (2026-11-24, coordinator ruling: the spec forms run, nothing deferred):** (L80) a param default reads the params before it (§8.1.16): `ParamPlan::default_over_params` keeps a default that does not fold alone, `bind_params` folds it at each placement over the values bound before it (the host's key overrides now apply in the same pass, before the `where`), `key_value`, `placement_bindings` and the hoisted params do the same, and a param a later default reads is structural (§8.1.2 amended, AFK default pending user review: a host `set` of it would leave the default stale). A default that reads itself or a later param is `E0321`. (L79) `Ty::Module` is a `Nominal`: a template's type names its arguments (`template_value`, the expected type fills a left-out one), a written `G<2>` lowers through `written_arguments` (shared with structs and enums), a placement through a template of a generic module reads its ports under the type's arguments (`template_substitution`), and an `Fn(P)` bound compares the ports under them (`first_differing_port`). (L78) A read of an item of a child placed through a callable param takes the value type its context wants, as an open literal does (`types/template_items.rs`: a check against a value type, the other operand of an operator, a `-`, the first use of a `let` with no type; `E0181` where nothing gives one, a method's receiver), recorded as `Types::template_item`; the elaborator checks the concrete item has it (`E0180` naming both, `reach.rs::read_has_its_type`), a reach frame through a template binds every param of the template (`ConstEval::template_bindings`), and `bank.c0.x` (template or not, in generation too) folds a param or a `pub let` over params of the child's child (`elab/reach_values.rs`). The round-1 test `a_non_node_item_through_a_template_is_refused_by_name_never_as_a_type_error` is replaced by the acceptance tests of C96. §8.4 gains a Resolution saying so (AFK default pending user review). Left, and recorded as L81 (owner `spec-closure`): a `pub let` over the solve or a carrier of a child's child read through the child, which was an unnamed `P0001` for any child of a child. Found on the way and fixed after (coordinator follow-up): `cell(a, gnd)` through an `Fn(Electrical, Electrical)` bound was a false `E0180`; a call through the bound now connects each node input by the placement's own rule (`Checker::connect`, §8.1.10), C112. Also: the pre-format twins of `std/analog.msl` and `std/constants.msl` take the doc comments of fix round 1 (`msl_format` was red).
**Fix round 3 (2026-11-24, verification-d.md findings 13 to 15):** (13) one rule decides which params a template or a placement must bind, `types::ParamRequirement` (no default and no `Option<T>`, §8.1.17), used by `require_bound_params` and `absent_option`. (14) Decided by the spec: `as` gives its result a type and never its operand (§1.4.5), so `c0.n as f64` is `E0181` at the read (not a cast from the concrete type). (15) A `let` with no type bound to a template read that no expression names is `E0181` at the read (`report_unread_template_locals`). The §8.4 Resolution lists the three forms with no typing context.
**Fix round 4 (2026-11-24, finding 16):** fixed in code, no GAPS row: the checker reads the nature of an access name the resolver left ambiguous from the node its operand holds (`access_of_operand`), a `let` bound to a template read counts as that read for a potential access (`reads_through_callable`), and the elaborator lowers a `let` bound to a node as a name for the node (`scope.handles`, as a handle parameter is), so `let y = x; v(y)` runs in all three forms.
**Fix round 5 (2026-11-24, findings 17 to 19 and the verifier's notes):** (17) the checker records the role of an access the resolver left ambiguous (`Types::access_role`), which the elaborator's `quantity_role` reads, so `v(y) +> 1.0` through a `let` contributes to the node. (18) `access_of_operand` says why it finds nothing (`OperandAccess`), and a missing access is `E0206`/`E0413` as for the node named directly (`missing_access`). Parentheses around a node argument name the node (`lower_terminal`). The spec forbids a node chosen by an expression in spirit (§1.4.7, hardware is not a value) but gave no code: amended (AFK default pending user review) to `E0133`, which the checker reports at the `if`/`match`/block. (19) `names_only_potentials` has its doc comment back, and the access matching is one readable method (`is_accessed_as`, `names_as_path`, `reads_a_template_potential`).
**Fix round 7 (2026-11-24, finding 21, user ruling):** a node chosen at elaboration runs. The checker refuses only a choice that reads the run (`chooses_a_node` over `run_read`: a probe, a port, a signal, a variable, a core primitive; a block with statements); the elaborator folds the rest (`elab/node_choice.rs`, from `lower_terminal`, so probes, targets and connections alike) and classifies the params a choice reads as structural for every reached module, a generic one's too (`classify_folded_param_keys`, which also takes the defaults of §8.1.16 there); key values of a generic module's instance read its own plans (`key_value` with the instance). `gnd` fits a node of any nature in `Ty::fits` (§6 rule 1), so an arm may be `gnd`. A choice that does not fold (a function that reads the run) is `E0133` at elaboration. Amendments approved by the user: §1.4.7, §8.4 and §8.1.2/§8.1.16; ROADMAP `live-param-defaults` is option 2.
**Fix round 8 (2026-11-24, finding 22):** the only criterion for `E0133` is whether the choice's value depends on the run: the checker records every node choice and judges it once the design is typed (`judge_node_choices`, so every assignment is known), a `let` is a variable of the run only when its initializer reads the run or an assignment writes it (`run_variable`); the block-statement sub-rule is gone. The elaborator binds the `let`s a choice reads to their constants (`node_choice.rs::bind_read_lets`, `let_initializer`) and the params behind them are structural (`collect_choice_params`).
**Dispatched:** none yet.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 18 → Phase 19 → Phase 20

Phase 18:  T83 → T84 → T85 → T86 → T87 → T88
Phase 19:  T89 → T90 → T91 → T92 → T93
Phase 20:  T94 → T95 → T96 → T97 → T98 → T99 → T100 → T101 → T102
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B14 | 18 | T83 to T88 | after phase C closes (T82) |
| B15 | 19 | T89 to T93 | the swappable MOSFET; modules#10 and cheatsheet#8 handed to `resolved-nets-bundles` (DD2) |
| B16 | 20 | T94 to T102 | `temperature()` (core) and `multiplicity()` (stdlib over `simparam`) as run settings (user decisions 2026-11-23, DQ4); T99 to T101 need `analog-operators`' `simparam -> Option<f64>` on `main`; closes L29 and L32 |

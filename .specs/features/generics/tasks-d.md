# Generics Tasks (phase D: templates, `temperature()` and `multiplicity()`)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-d.md`
**Status**: Draft
**Approved**: 2026-11-23 - user: "Sim. E aceito os outros"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite suite_hygiene::`
**Run typestempl**: `cargo test -p piperine-lang --test types_generics templates::`
**Run typesimplicit**: `cargo test -p piperine-lang --test types_modules implicit::`
**Run typestraits**: `cargo test -p piperine-lang --test types_generics traits::`
**Run elabtempl**: `cargo test -p piperine-lang --test elab_structure templates::`
**Run elabimplicit**: `cargo test -p piperine-lang --test elab_values implicit::`
**Run irgen**: `cargo test -p piperine-ir --test model generics::`
**Run interp**: `cargo test -p piperine-interp --test interpreter frozen::`
**Run tgen**: `cargo test -p piperine-transpile --test print generics::`
**Run tcontrol**: `cargo test -p piperine-transpile --test print control::`
**Run typesgen**: `cargo test -p piperine-lang --test types_generics generics::`
**Run srctempl**: `cargo test -p piperine-parity --test source_structure templates::`
**Run srcimplicit**: `cargo test -p piperine-parity --test source_host implicit::`
**Run elabparams**: `cargo test -p piperine-lang --test elab_values params::`
**Run typeshandles**: `cargo test -p piperine-lang --test types_carriers handles::`
**Run elabhandles**: `cargo test -p piperine-lang --test elab_structure handles::`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookanalog**: `cargo test -p piperine --test book_analog`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`
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

**C91** - A placement through a template value (a `const`, a param, a body `let` bound to a template, written before or after its placement, or to a choice of templates that folds at elaboration) or a function result places an instance identical to the written placement; a choice of templates the run makes is `E0362`, calling an instance is `E0333` and calling a value that is neither a function nor a template is `E0180`. (GN-22, AC 1; T86, T87, T93; fix round 11, finding 29; fix round 12, findings 30 to 32)
Proof: run typestempl | test a_placement_through_a_template_or_a_function_types_as_its_module ... ok
Proof: run elabtempl | test a_template_placement_is_identical_to_the_written_placement ... ok
Proof: run bookmodules | test a_function_that_returns_a_template_places_its_result ... ok
Proof: run srctempl | test a_template_a_body_let_holds_places_its_module ... ok

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

**C96** - `inst.name` through a template-typed param resolves against the concrete module, in the module that places the template and from its parent, `E0343` for a missing or private item; an item of any kind (a `pub` param, a `pub let`, a carrier) is read as a value with the type its context wants, checked against the concrete item at elaboration (`E0180` naming both; `E0181` where no context gives one: a method's receiver, the operand of `as`, a `let` with no type that nothing reads, fix round 3 findings 14 and 15), also from the parent (`bank.c0.cs`) and at a generated iteration (`bank.cells[1].cs`); an out-of-range generated cell is `E0342` naming its element and the length of its own loop. (GN-23, AC 3; T91; fix rounds 1 and 2, L78) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
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
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run elabtempl | test an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop ... ok

**C97** - A module type takes trait and inherent impls of pure functions over its template, run at elaboration; a trait plus `Fn(P)` bound admits only a module that implements the trait; a function that reads the solve is `E0122`. (GN-23; the decision of 2026-11-22; T83, T89, T90, T92) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
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

**C101** - `temperature()` is `Some` of the run's temperature in kelvin or `None`, and the model chooses constant, initial or ignored; the docs recommend the ambient fallback. (GN-25, AC 1, AC 2; T94, T97, T98, T100) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typesimplicit | test temperature_is_an_option_of_the_run ... ok
Proof: run srcimplicit | test a_model_chooses_how_to_use_its_temperature ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run bookanalog | test the_temperature_listings_run_with_the_ambient_fallback ... ok

**C102** - `m` and `dtemp` are ordinary names: no implicit param exists. (GN-25, AC 3; T94, T97) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typesimplicit | test m_and_dtemp_are_ordinary_names ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C103** - The run sets its temperature and its `multiplicity` session parameter before it starts, they hold for the whole run, and neither has a placement or `set` spelling. (GN-26, AC 1, AC 2; T100)
Proof: run srcimplicit | test the_run_settings_hold_for_the_whole_run ... ok

**C104** - The Book's multiplicity listing runs as four in parallel and its row leaves `GAPS.md` §11. (GN-26, AC 3; T101)
Proof: run bookmodules | test the_multiplicity_listing_runs_as_four_in_parallel ... ok

**C105** - The interpreter reads the temperature-given flag (it is no ambient special case); a template leaves no IR form; the device reads whether the run set its temperature. (GN-29, AC 1; T95, T96, T87)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run irgen | test run_setting_reads_are_built_checked_and_printed ... ok
Proof: run tgen | test a_device_reads_whether_the_run_set_its_temperature ... ok

**C106** - `GAPS.md` drops L29 and L32 and §9 and §10 follow. (GN-31, AC 1; T102) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C107** - The template Book rows flip or narrow to the bundle they still need, and the swappable MOSFET listing runs. (GN-31, AC 2; T93)
Proof: run bookmodules | test the_swappable_mosfet_listing_runs ... ok
Proof: run bookmodules | test the_memory_bank_is_refused_only_for_its_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_only_for_its_bundle ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C108** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; T97, every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C110** - A param default reads the params declared before it, folded per instance over the values they take there, a template param's trait method included (`level: f64 = fet.gain()`); the params a later default reads are structural; a default that reads the param itself or a later one is `E0321`. (§8.1.2, §8.1.16; fix round 2, L80) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run typestempl | test a_param_default_reads_earlier_params_and_a_template_params_trait ... ok
Proof: run elabparams | test a_default_that_reads_earlier_params_is_folded_per_instance ... ok
Proof: run elabparams | test a_default_that_reads_itself_or_a_later_param_is_e0321 ... ok
Proof: run elabtempl | test a_param_default_dispatches_the_trait_of_a_template_param ... ok
Proof: run srctempl | test a_param_default_reads_the_params_before_it ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C111** - The type of a template of a generic module carries its arguments, `G<2>`: a template of other arguments where it is wanted is `E0180` naming both, an argument the template leaves out is the wanted type's, and a placement through a template (in place, a `const`, a param of type `S<E>`, a param bounded by `Fn(..)`) of a module whose ports are typed by its generic parameters reads them under those arguments (`E0251` for a node of another nature). (§8.4.3, §8.4.5; fix round 2, L79) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run typestempl | test the_type_of_a_generic_template_carries_its_arguments ... ok
Proof: run typestempl | test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok
Proof: run elabtempl | test a_placement_through_a_template_of_a_nature_generic_module_places_its_instance ... ok
Proof: run srctempl | test a_template_of_a_nature_generic_module_runs_through_its_type ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

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

**C117** - A node chosen at elaboration (an `if`, a `match` or a block whose choice reads only params, constants, generation indexes and `let`s of those, whatever statements its arms hold; fix round 8, finding 22) names the node it folds to at a probe, a contribution target, a placement's connection and an `Fn` bound's, and its params are structural; a choice that reads a probe, a signal, a core primitive of the run or a variable whose value depends on the run is `E0133` alone, also written straight at a node argument, and so is one through a function that reads the run (elaboration). (§1.4.7, §8.1.2; user ruling 2026-11-24, finding 21; findings 20 and the round-5 note) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
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
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C118** - A node of a child is a connection like any node: `Th(c0.sn)` with `c0` placed by name or through a template, and `Th(if sel { c0.sn } else { t })`, bind `Th`'s port to the child's node. (§8.1.10, §8.2.8; fix round 8, finding 23)
Proof: run typeshandles | test a_node_of_a_child_is_a_connection ... ok
Proof: run elabhandles | test a_node_of_a_child_connects_to_another_placement ... ok
Proof: run srctempl | test a_childs_node_connects_to_another_placement ... ok

**C119** - The items of a module body appear in any order: a `let` read before it is written (a placement, a node of a child, a value) is typed where it is read and elaborates as the forward body does; two `let`s that read each other's values (at the value inputs of placements without a result, or as plain values) are `E0332`. (§8.2, §8.2.16; fix round 8, coordinator follow-up; re-pinned in fix round 9, finding 25)
Proof: run typeshandles | test a_module_body_is_typed_in_any_order ... ok
Proof: run elabhandles | test a_body_written_in_reverse_order_elaborates_as_written_forward ... ok
Proof: run srctempl | test a_body_written_in_reverse_order_runs_as_forward ... ok

**C156** - A node connection is hardware, not a value: a placement connects to the node of a child written after it, and a cross-coupled pair, an instance wired to its own node, a ring of three and a bank fed from its child's node (`b.c0.sn`) type, bind each port to the node it names and settle where Kirchhoff's law puts them; the reverse body settles where the forward one does; a cycle through values is still `E0332`. (§8.2.14, §8.2.16, §8.3.5; fix round 9, findings 24 and 25, coordinator ruling)
Proof: run typeshandles | test a_cycle_of_node_connections_is_hardware ... ok
Proof: run elabhandles | test a_placement_binds_the_node_of_a_child_written_after_it ... ok
Proof: run elabhandles | test a_cycle_of_node_connections_binds_each_port_to_the_node_it_names ... ok
Proof: run srctempl | test a_placement_connects_to_the_node_of_a_child_written_after_it ... ok
Proof: run srctempl | test a_cycle_of_node_connections_runs_as_hardware ... ok
Proof: run srctempl | test a_cycle_through_values_is_refused ... ok

**C157** - A `let` is a constant of elaboration when its initializer, every value assigned to it and every condition around those assignments fold there: a node chosen by such a `let` (`let s = sel; s = !sel;`, a toggle under a condition over a param, in a `for` over a range) types, folds to the node the `let` holds where the choice reads it and runs; its params are structural; one whose assigned value or surrounding condition reads the run is `E0133` alone, naming that read. (§1.4.7; fix round 9, finding 26, coordinator ruling)
Proof: run typeshandles | test a_let_assigned_only_values_of_elaboration_chooses_a_node ... ok
Proof: run elabhandles | test a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there ... ok
Proof: run srctempl | test a_let_assigned_from_params_picks_the_node ... ok

**C158** - A node connection reached through any depth of children is hardware too: a cross pair and a self-connection through a template two levels deep (`p.c0.sn`) and through a generation (`p.cells[1].sn`) type, bind each port to the node it names and settle where Kirchhoff's law puts them; three levels deep (`p.mid.c0.sn`) the checker types the cycle and elaboration refuses it as it refuses the forward read, by name (L82); a value read through an open `let`'s child (`p.c0.cs` at a value input) is `E0332`. (§8.2.14, §8.2.16, §8.3.5; fix round 10, finding 27) (retired by code-clarity T10: a pin on a sentence of a tracking file; the test was already gone)
Proof: run typeshandles | test a_cycle_of_node_connections_through_any_depth_of_reach_is_hardware ... ok
Proof: run elabhandles | test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok
Proof: run srctempl | test a_cycle_of_node_connections_two_levels_deep_runs_as_hardware ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C159** - A choice whose condition assigns its `let` first (`if { s = !s; s }`, a `match` over such a block, a `while` over a `let` in the condition) folds at elaboration to the node the condition picks and ends, within a time bound at elaboration and from source; one whose condition assigns a value of the run is `E0133` alone, naming the read. (§1.4.7; fix round 10, finding 28)
Proof: run elabhandles | test a_choice_whose_condition_assigns_folds_and_ends ... ok
Proof: run srctempl | test a_condition_that_assigns_its_let_picks_the_node_and_ends ... ok

**C160** - A body `let` read before its text position is read as one written before it, at every site a name resolves to a module-body `let` (callee: closure, value `E0180`, instance `E0333`, template, function value; operand; argument; `as` operand; condition; method receiver; index base; field receiver, node and param; connection; template head through a choice; param value): the two orders give the same diagnostics, and a reverse-order closure callee runs. (§8.2; fix round 13, finding 33)
Proof: run typeshandles | test every_site_reads_a_body_let_written_later_as_one_written_before ... ok
Proof: run elabtempl | test every_callee_form_elaborates_to_what_it_names ... ok
Proof: run srctempl | test every_callee_form_runs_what_it_names ... ok

**C161** - Every alternative of Part III's `callee` (`path`, `call`, `'(' expr ')'`) is called by its value: a template (in place, a `const`, a choice by `if`, `match` or a block, a function result) places it with the params it holds, a function (a closure literal, a named function, a function's result, a choice of functions) is called, a value of any other type is `E0180`, and a choice the run makes is `E0362`; no form reaches a generic refusal. (Part III callee production, §1.5, §8.4.4, §8.4.6; fix round 13, finding 34)
Proof: run typestempl | test every_callee_form_the_grammar_admits_is_called_by_its_value ... ok
Proof: run elabtempl | test every_callee_form_elaborates_to_what_it_names ... ok
Proof: run srctempl | test every_callee_form_runs_what_it_names ... ok
Proof: run tcontrol | test a_function_value_returned_by_a_function_compiles_and_runs ... ok

**C162** - A module's bare name used as a value (`let t = R;`, the head `(R)(x)`, a param value, a function's result) is the template `R {}`, every param at its default: `let t = R; t(x)` and `(R)(x)` place the instance `R(x)` places; a required param it leaves unbound is `E0319` alone; the phase-C `P0001` "placement through a template value or a function" is gone, and no cascade follows a head the checker refused. (§8.4.1 amendment, AFK default 2026-11-24 pending user review; fix round 14, finding 35)
Proof: run typestempl | test a_bare_module_name_used_as_a_value_is_its_template_with_every_default ... ok
Proof: run elabtempl | test a_bare_module_name_used_as_a_value_places_its_template_with_every_default ... ok
Proof: run srctempl | test a_bare_module_name_places_its_template_with_every_default ... ok
Proof: run typesgen | test phase_d_and_e_forms_stay_p0001_by_name ... ok

**C163** - A template choice whose arms name two modules is one `E0180`; a call through the `let` that holds it adds no diagnostic, and a value of a module's type is never called "no function and no template". (§1.5.4, §8.4.4; fix round 15, finding 37)
Proof: run typestempl | test a_choice_of_two_modules_is_one_e0180_and_its_call_adds_none ... ok

**C164** - A template value handed to a param of module type is a const expression over the placing module's params, which become structural: a template literal over a param, a binning function over the parent's length and a choice over a param fold per instance, in the top module and in a placed one whose params its parent binds; one that reads a port is `E0362`. (§8.4.6; fix round 15, finding 36)
Proof: run typestempl | test a_template_param_value_may_read_the_placing_modules_params ... ok
Proof: run elabtempl | test a_template_param_value_over_the_placing_modules_params_folds_per_instance ... ok
Proof: run srctempl | test a_template_param_value_over_the_parents_params_places_per_instance ... ok

**C109** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-repository-guards --test suite suite_hygiene::`

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
**Fix round 8, finding 23:** a node of a child connected to another placement runs, made in code rather than a GAPS row: `lower_child` no longer refuses a reached terminal, and `place.rs::place_children` places a child whose connections read a reached node after the child that owns it, binding the node into the parent's slots at once (siblings and the parent's own parent see it). A name read before its `let` in a module body is typed where it is read (`type_module_let_read`, coordinator follow-up, C119): the checker types module-body `let`s in dependency order, and a `let` cycle through placements without a result is `E0332` (§8.2.16).
**Fix round 9 (2026-11-24, findings 24 to 26 of verification-d.md; the fixer stopped at a commit boundary because its context was large).** Coordinator rulings and the plan the fixer worked out, for the next fixer:
- **24 + 25 (one commit may hold both): node connections are hardware, not values (§8.2.14, §8.3.5).** Elaboration becomes two passes per body: (1) allocate every child's own nodes, recursively, so every reached slot of the body (`NodeRole::Reached`, including the nested `bank.c0.sn` chain of `reach_through_templates.rs`) is known; (2) place each child with its ports bound. Today `place.rs::place_instance` creates a child's signal nodes inside its own placement (`self.ir.node(NodeSpec { .. })` for `NodeRole::Signal`) and `place_children` orders children by readiness and refuses a cycle by name; with two passes `place_children`'s ordering and its cycle refusal go (one way, not two), and `let q = D2(p.sn); let p = D2(x);` (today `P0004`: the reached node is handed up before it is bound), `let p = D2(q.sn); let q = D2(p.sn);`, `let p = D2(p.sn);` and a ring of three run. Checker: `types/result.rs::self_read` reports `E0332` for a cycle through placements without a result (added in da0e5cf48); a cycle whose reads of the open `let` are only node connections (a field read of a node of the instance, `q.sn`, at a node port) is no value cycle and must type (the `let`'s type is the placement's module, known from the callee before the arguments: give it that type when the read is a node read through the instance name), while a cycle through values (value `let`s, value inputs, results) keeps `E0332`/`E0353` per §8.3.5. Tests to add: checker (cross pair, self, ring of three clean; a value cycle still `E0332`), elab (reverse pair binds as forward; ring binds), source (a cross-coupled pair and a ring of `D2`-like cells settling at Rust-computed values).
- **26: a `let`'s assignments are judged like its initializer.** `types/handles.rs::run_variable` treats any assigned `let` as a variable of the run. Record each assignment's value (`types/assign.rs::assignment`, beside `places.record`) and the conditions of the control flow around it, and call the `let` a constant only when its initializer, every assigned value and every enclosing condition fold at elaboration; otherwise name the run read in the message. The elaborator must then fold such a `let` to its value at the choice (today `node_choice.rs::bind_read_lets` binds only the initializer and skips an assigned `let`, `types.is_assigned`), so it needs the statements before the choice executed at elaboration (the const evaluator's `exec_block` does this for function bodies). Tests: `let s = sel; s = !sel; let y = if s { x } else { p };` runs and names the right node; an assignment of a probe, or one under a run-time condition, stays `E0133` naming the read.
**Fix round 9, findings 24 and 25 (done):** every reached slot of a body is a node made with the body's own nodes, before any child is placed (`place.rs`, named by the reader's path, `Top.p.sn`), and the child that owns the node is made one node with it once placed (`IrBuilder::merge_nodes`, the IR's existing join, so a reached port and a chain `b.c0.sn` bind alike); `place_children` places in source order, its readiness ordering and its cycle refusal are gone. Deviation from the ruling's wording: the child's own nodes are not allocated in a pass before placement (a node names its declaring instance, whose id exists only once placed, and a reached port is the parent's terminal, known only then); the reached slot is made ahead instead and joined, which gives the same hardware. The checker types a node read through an instance whose `let` is open (`q.sn`, `b.c0.sn`) as the placed module, known from the callee before the arguments (`types/reach.rs::open_instance`), so a cycle of connections is no `let` reading itself; a cycle through values keeps `E0332`/`E0353` (C119 re-pinned to value inputs). C156.
**Fix round 9, finding 26 (done):** the checker records every assignment (`Places::writes`) and `run_variable` names what makes a `let` depend on the run: its initializer, an assigned value, a condition around an assignment (an `if`, a `match`, a loop's condition or range, every condition inside a loop around it), or an assignment in a closure; the `E0133` message names that read (`the variable `s`, whose value depends on the run: it reads a probe`). The elaborator folds an assigned `let` where the choice reads it by running its block's statements from its `let` to the choice (`ConstEval::local_after` over `exec_block`, a statement that does not fold forgets what it assigns), and `collect_choice_params` makes the params of the assigned values and conditions structural. C157.
**Fix round 10, finding 27 (done):** the checker's open-instance read (`types/reach.rs::open_head`/`open_instance`) walks any chain of children, generated iterations (`p.cells[1]`) included, and a read through a template-placed child of an open `let` (`p.c0.sn`) is a node read; one a context then wants as a value is `E0332` (`Checker::open_template_reads`, `template_items.rs`). Elaboration already merged the reached nodes two levels deep. Three levels deep (`p.mid.c0.sn`) was an unrecorded `P0001` in any order; it is now L82 in `GAPS.md` (owner `spec-closure`; counts 17 loud, `docs.rs` pins follow). C158.
**Fix round 10, finding 28 (done):** the hang was `structural.rs::collect_choice_params` following a `let` that reads itself through its assignments 64 levels deep with a branching factor (exponential, not infinite). Every follow of a `let` in a choice is now once per `let`: `collect_choice_params` and `node_choice.rs::bind_read_lets` keep a set of followed `let`s instead of a depth, and the checker's `run_variable` judges each variable once. A choice's condition folds with locals of its own (`ConstEval::value_in_a_scope_of_its_own`), so what it assigns is read by the condition and gone after it. The const evaluator's loops stay bounded by 2^24 trips (`E0104`). C159.
**Fix round 11, finding 29 (done):** a body `let` bound to a template is a template holder like a `const`, a param or a function result: the POM classifier names the template's module through any holder (`classify.rs::template_module`, one path for a template written in place, a `const`, a param or value input of module type, a `let` bound to one and a function's result), and the checker reads such a `let` as a value of the module's type, E0333 being kept for a `let` of a placement (`Checker::binds_an_instance`). C91 gains the `let` forms at each level.
**Fix round 12 (2026-11-24, findings 30 to 32):** (30) the name of an instance without a result in a callee's position is `E0333` (§8.2.14), as at any value position (`types/closures.rs::local_call`). (31) a choice of templates of one module (`if`, `match`, a block) held by a `let` is a template holder: `classify.rs::template_module` names its module through the choice, the elaborator folds it over the key params, which the choice makes structural (`params_read_by_template_heads` follows the `let`s), and a choice the run makes is `E0362` (§8.4.6, `callee_is_template_of` reads the head through its `let`s with `run_read`). (32) a value of any other type called is `E0180` (§1.5: the callee's position requires a function or a template), naming its type; no spec amendment, E0180's definition covers it. The vacuous `elab_unlowered::a_form_the_checker_accepts_for_msl_digital_is_p0001_until_it_is_lowered` (an empty table since the merge) is deleted.
**Fix round 13 (2026-11-24, findings 33 and 34, by class):**
- *33, "a body `let` read before its text position".* Sites enumerated (every read of a local's type in the checker, `self.locals.get`): the path read (`expr.rs`, which every operand, argument, `as` operand, condition, method receiver, index base, generation bound, contribution target, event operand and connection reaches through `infer`), the field receiver (`reach.rs::infer_receiver`), the callee (`closures.rs::local_call`), the `Fn`-bound callee (`bounds.rs::callable_parameter`), the template head (`module.rs::callee_is_template_of` through `infer`, and the classifier's syntactic `template_module`), and the handle read (`handles.rs::handle_read`, now after the typing). They go through one entry, `Checker::binding_type` (types the `let` on demand, then reads its type). Left reading `locals` directly, on purpose: `handles.rs::is_node_handle`/`handle_type` (after a read typed the handle), `assign.rs` (a module-body `let` is no place), `reach.rs::item_type` (a child's `let`, through `check_ahead`), `Types::local` (the public table after checking). An `analog` block reads module-body `let`s after the body is typed, so order does not reach it.
- *34, "every callee form".* Part III `callee ::= path generic_args? | call | '(' expr ')'`. Forms: path to a module, a function, a local (closure, function value, template holder, instance `E0333`, value `E0180`); `call` (a function returning a template, a function returning a function); parenthesized: a template in place (its params were dropped by the elaborator, `placement_fields` now looks through parentheses), a `const`, a choice of templates by `if`/`match`/block (classified as a placement with no `via`, folded as a head, its params structural), a closure literal, a named function, a choice of functions, any other value (`E0180`). The checker's generic `call` refusal is replaced by `value_call` (a function type is called, any other type is `E0180`), and the elaborator lowers a function-valued callee expression as `Expr::CallValue` (`lower_expression_call`); the checker records a local callee's function type. Found and left: the Rust backend prints a function whose result is a function type as `&dyn Fn(R) -> R` (rustc refuses it); elaboration runs it, the source rig leaves that form out. Not a phase D form; reported to the coordinator.
**Transpile follow-up (coordinator request after round 13) — DONE (option (a), coordinator ruling):** every function value prints as one owned `std::rc::Rc<dyn Fn(A) -> R>` (param, local, result, capture, closure literal `std::rc::Rc::new(move |..| ..)`, named function `std::rc::Rc::new(f::<R>)`); a read of a function-valued local clones it, a call through a local calls it by name, and a closure clones each function value it captures from outside before its `move` (`print/control.rs::closure_value`); `print/types.rs`'s `Fn` arm looks into its params and result, so `fn doubler() -> fn(f64) -> f64` is generic over `R`; `rt::Real` is `'static` so an `Rc<dyn Fn(R) -> R>` holds it; the refusal of a function value returning a function value is gone. `RigCallees` runs `doubler()(0.3)` and `scaler(4.0)(0.35)`; `tests/control.rs::a_function_value_returned_by_a_function_compiles_and_runs` pins the text and runs it (C161). `piperine-interp` refuses both forms by name ("a function value", "a call through a function value": the interpreter is frozen at the M1 analog subset, `frozen.rs`). The handoff text that follows is the analysis it started from. The ask: a function whose result is a function type (`fn doubler() -> fn(f64) -> f64`, and a returned capturing closure) prints as Rust rustc accepts, with `doubler()(1.0)` and a returned capturing closure added to `crates/piperine-parity/tests/templates/callees.msl` (`RigCallees`, against Rust-computed values in `source_templates::every_callee_form_runs_what_it_names`), plus a transpile unit/golden test. One commit. What the fixer found:
- Function values print as borrowed `&dyn Fn(A) -> R` (`print/control.rs::function_type`, which already refuses a function value whose result is a function value; `print/value.rs:809` prints a closure as `&move |..| ..`; a local of one is annotated at `value.rs:1127`). Pinned by `crates/piperine-transpile/tests/control.rs:496-497` only.
- Two defects in `fn doubler() -> fn(f64) -> f64 { twice }`: (1) the signature prints `-> &dyn Fn(R) -> R` with no generic `R` in scope: `print/types.rs:109` says a `Fn` type holds no real, so `Printer::is_generic` (`value.rs:455`) does not make the function generic over `R` — the `Fn` arm should recurse into its params and result as `widths.rs:58` does; (2) a returned reference has no lifetime: `&twice` (a fn item) is const-promoted and could be `&'static`, but a returned capturing closure (`&move |y| y * k`) is a temporary.
- Options weighed: (a) function values as `std::rc::Rc<dyn Fn(A) -> R>` everywhere (owned, returnable, callable through autoderef; but a `move` closure that reads an `Rc` local moves it, so captures would need a clone prelude `{ let f = f.clone(); move |..| .. }` and reads `.clone()`); (b) keep `&dyn Fn` for params and locals and print a function *result* as `Box<dyn Fn(A) -> R>` with the returned value boxed (`Box::new(twice)`, `Box::new(move |y| ..)`), the call site `doubler()(1.0)` working on the box, a local bound to it annotated `&dyn Fn` taking `&*`; (b) is smaller but is two representations, which the coordinator asked to avoid ("no special case"); (a) is the uniform one.
**Fix round 14, finding 35 (done; coordinator ruling, AFK default 2026-11-24; **Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu"):** a module's bare name used as a value is the template `Name {}` (§8.4.1 amendment in `docs/spec/part_iii_structure.md`). One template path: the classifier's `template_module` names the module of a bare name (so `let t = R; t(x)` classifies as a placement through the `let`), the checker types the bare name through `template_value` with no fields (`types/expr.rs`, `Checker::bare_module`; `E0319` for a required param, as `R {}`), and the const evaluator folds it through `template_literal` with no fields (`elab/consts.rs`). The phase-C `P0001` "placement through a template value or a function" is gone: `types/call.rs` calls any other `Instance { module: None }` head by its type (`value_call`: a function, else `E0180`), and `types/module.rs` reports `E0180` for a head typed as something other than a template of the module and nothing for a head already refused (no cascade). C162. `phase_d_and_e_forms_stay_p0001_by_name` re-pinned: a placement through a value input of module type is refused by the input's type alone.
Remaining `P0001`s in the template/callee paths (grep of `unsupported(` in `types/{call,module,closures,templates}.rs`, `elab/{children,closures,lower}.rs`, `pom/resolve/classify.rs`), each user-reachable one a `GAPS.md` row (fix round 14 bookkeeping, coordinator request):
- Internal guards, no `GAPS.md` row: no source reaches them, because each sits behind a classifier or checker invariant. Owner `generics`; they stay as fail-loud checks.
  - `types/call.rs` "call that is not a function call": `callee_class` gives every call a class and never `Unclassified`.
  - `elab/children.rs` "a placement through a template value": it is reached only when the class disagrees with the module the checker named.
  - `elab/lower.rs` "a placement used as a value": the checker refuses it first, `E0180` "expected `f64`, found `module ..`" or `E0333` (probed).
  - `elab/lower.rs` "a call the type checker did not resolve".
  - `elab/closures.rs` "a call of a value that is not a function" and "a callee whose function value has no function type once lowered": the checker's `E0180` "is no function and no template" comes first (probed).
- L83 `types/call.rs` "call of this declaration" (an `enum`'s name called, `E(1.0)`): owner `spec-closure`.
- L84 `types/closures.rs` "a call of a value whose type is unknown" (`let f; f(1.0)`, which should be `E0183`): it is user-reachable, so it is not a guard. With it is "a deferred `let` of a type with no zero in the IR" (`let f; f = h; f(1.0)`). Owner `generics`.
- L85 `types/closures.rs` "a closure that captures where a function pointer `fn(..) -> R` is expected": owner `closures`, with `spec-closure` to give it a code.
- L86 a function with an `Option` parameter or result used as a value. The probe meets "a name whose type is not a value type" and "an expression the type checker gave no value type", and `elab/closures.rs`'s own message sits behind them. Owner `closures`.
- L87 `types/carrier.rs` "value input port of type `module ..`" (`module Top(cell: Cell)`): owner `params-and-io`.
`GAPS.md` counts go from 15 to 20 loud, L1 to L87; `crates/piperine-cli/tests/docs.rs` is re-pinned to them. `tests/book/inventory_guard.rs` counts no loud rows, so it is unchanged.
**Fix round 15, findings 36 to 38 and the `P0001` sweep (coordinator request after round 14):**
- **Finding 37, done** (77161aa0e): a body `let` whose template value was refused (a choice of two modules, `E0180`) holds no template (`types/module.rs`: the `let` is poisoned when typing its value reported a diagnostic and its type is a module's). Its call adds nothing, and `local_call` never calls a value of a module's type "no function and no template". C163.
- **Finding 36, done** (f6661fe60): a template value handed to a param of module type makes every param it reads structural, through the same read the head of a placement through a template uses (`elab/structural.rs::params_that_feed_key_params` -> `collect_choice_params`). It folds per instance, in the top module and in a placed one. C164 covers it at the checker, elaboration and source levels (`RigParamTemplates` in `callees.msl`: binning by the parent's length, with values computed in Rust).
- **Finding 38, `GAPS.md` row L88**, owner `generics` phase E. Running an array of templates needs three things: the classifier must name the module through an element, the const evaluator must fold the array and its index, and generation must carry it. That is a feature, not a fix. `docs.rs` is re-pinned to 21 loud rows, L1 to L88.
- **§8.4.1 amendment:** approved by the user ("ACeito os dois"), marker replaced. The §1.4.7 sentence carried no pending marker; its paragraph already holds the user's approval line.
- **The sweep: NOT STARTED, handed off at this commit boundary** because the context is large. The inventory to classify is every `unsupported(` / `ConstFault::Unsupported` site in `crates/piperine-lang/src/{types,elab}`: 492 sites in 80 files (`grep -rn "unsupported(\|Unsupported {" crates/piperine-lang/src/types crates/piperine-lang/src/elab`).
  - For each site, record three things: the message; whether source reaches it, probed with `piperine check <file>.msl` (binary at `$CARGO_TARGET_DIR/debug/piperine` after `cargo build -p piperine-cli --bin piperine`); and its disposition (fixed, a `GAPS.md` row, or an internal guard with the reason it cannot be reached).
  - A reachable site inside phase D's scope (templates, module impls, run settings, callables, generics) is fixed; one outside it gets a row.
  - The callee and template paths are already classified (fix round 14 above: L83 to L87 and the guards).
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

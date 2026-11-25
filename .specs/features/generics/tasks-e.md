# Generics Tasks (phase E: the stdlib built on generics)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-e.md`
**Status**: Draft
**Approved**: 2026-11-23 - user: "Sim. E aceito os outros"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesmodel**: `cargo test -p piperine-lang --test types_model`
**Run stdlibrary**: `cargo test -p piperine-lang --test std_library`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
**Run srcstd**: `cargo test -p piperine-parity --test source_stdlib_generic`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run elabgenr**: `cargo test -p piperine-lang --test elab_generation`
**Run srcgenr**: `cargo test -p piperine-parity --test source_generation`
**Run typesresults**: `cargo test -p piperine-lang --test types_results`
**Run elabresults**: `cargo test -p piperine-lang --test elab_results`
**Run elabtemplates**: `cargo test -p piperine-lang --test elab_templates`
**Run typestempl**: `cargo test -p piperine-lang --test types_templates`
**Run apisession**: `cargo test -p piperine-api --test session`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Base and order.** Phase 21 starts after `tasks-d.md` T102 (phase D closed) and, by design-e.md DE1, after
`analog-operators` B4–B7 is on `main`; T103 checks both and stops the batch if one is missing. After T115 the
orchestrator runs `check_proofs.py generics` (it reads the `checks.md` T115 writes) and dispatches the Verifier for
the whole feature.

Rules every task follows (the user's rules, repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, the lane letter the coordinator gives. `/var/tmp/piperine-target` is a read-only trap and is never used. Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-21).** After each task: the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, then the `Run` lines its `Done when` names. Once at the end of each batch: `cargo test --workspace`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` and the buck targets.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves.
- **Only design-e.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; a divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same expression evaluated by Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules).** Readable at a glance; the module tree says where each thing is; each crate one purpose; **no free function, tests included** (every function has an owner type: a test helper is a method of a support type); **one purpose per type**; **OO names without abbreviations**; **no god method** and no needless abstraction; the data path evident; **a `///` doc comment on every new type, method, field and variant**, and a non-evident block cites its spec rule. No macro; no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design-e.md "Components" names, never into `types/expr.rs`, `types/call.rs` or `print/value.rs` beyond a dispatch arm.
- **Conventions.** No new IR `Expr`/`Stmt` variant; new IR fields carry `#[serde(default)]`. The interpreter refuses every new IR marker by name (R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models live in `crates/piperine-parity/tests/stdlib_generic/*.msl`.
- **Book rows.** A Book test that asserts a refusal and starts to run flips in the task that makes it run: its test becomes the acceptance, `tests/book/inventory.tsv` takes `run` (or `holds`), the `GAPS.md` §11 row leaves, and the Book's own "today it refuses" prose is rewritten to what the binary now prints. A row that still needs another feature's form is narrowed to that form, and its §11 row takes that gap id and owner (recorded on the Handoff).
- **Cross-lane files.** `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `ROADMAP.md` rows of other features are not edited.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-38, MD-57, MD-78, MD-81.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | `std` under the model check; each item resolved to its source | `crates/piperine-lang/tests/{types_model,std_library}.rs` | `cargo test -p piperine-lang --test types_model` / `--test std_library` |
| Elaborator (`piperine-lang/src/elab`) | integration | the noise scaling of `m`; the spec's `Water` | `crates/piperine-lang/tests/elab_structure.rs` | `cargo test -p piperine-lang --test elab_structure` |
| Source-level runs | integration | one test per stdlib item against its §22 entry, values computed by Rust in the test | `crates/piperine-parity/tests/source_stdlib_generic.rs`, models in `tests/stdlib_generic/` | `cargo test -p piperine-parity --test source_stdlib_generic` |
| Book and galleries | integration | every row of a stdlib item flipped; fences held to the checker | `tests/book/*.rs`, `crates/piperine-lang/tests/std_galleries.rs` | `cargo test -p piperine --test book_core_std`, `cargo test -p piperine-lang --test std_galleries` |
| Repository records | integration | one assertion per removed `GAPS.md` row; the feature's `checks.md` | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| Contract re-read (T103) | none | - (build gate; findings on the Handoff) | - | build gate only |

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

### Phase 21: Contract, `std` under the model check, the digital monitors

```
T103 → T104 → T105 → T106 → T107 → T108
```

### Phase 22: The analog library items, `m` and noise, and the feature's closure

```
T109 → T110a → T110 → T111 → T112 → T118 → T119 → T120 → T113 → T114 → T115
```

---

## Task Breakdown

### T103: Contract: merge `main` and record what landed

**What**: Check that `generics` phase D and `analog-operators` B4–B7 are on `main` (stop and report if either is not, DE1); `git merge origin/main`; record on the Handoff how `delay` and `noise` lower (the IR forms T111 and T112 meet), and each L58 item's state against design-e.md's table; amend any task below whose text a finding affects.
**Where**: `.specs/features/generics/tasks-e.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design-e.md "What the code does today"
**Requirement**: GN-30
**Executor**: Sonnet

**Done when**:

- [x] The merge is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test --workspace` pass
- [x] The Handoff names each finding and the task it affects

**Tests**: none
**Gate**: build

**Commit**: `chore: merge main for generics phase E; anchors`

---

### T104: `std` joins the whole-model check

**What**: `ModelCheck` seeds every item of `std` (design.md D2 retired): each item of L58's generics group type-checks with no instantiation and no error (GN-27 AC 3); a library error the check finds is fixed in `std/` with the spec as authority, each fix named in the commit body (a fix that changes a §22 behaviour stops the task).
**Where**: `crates/piperine-lang/src/types/model.rs`
**Depends on**: T103
**Reuses**: `ModelCheck`
**Requirement**: GN-27 (AC 3)
**Executor**: Sonnet

**Done when**:

- [x] `the_standard_library_is_checked_whole_with_no_instantiation` passes under `**Run typesmodel**` (it replaces `the_standard_library_is_left_to_its_own_check`, named in the commit body)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the standard library joins the whole-model check`

---

### T105: The checker's stand-ins for library items leave

**What**: Every `Callee::Intrinsic` or `Builtin` that stands for an item of L58's generics group (design.md D15) is removed, so each item resolves to its declaration in `std/`; `same` and `cast` already compile from source (T55); the test walks the group and asserts each resolution.
**Where**: `crates/piperine-lang/src/types/table.rs`
**Depends on**: T104
**Reuses**: `types/intrinsic.rs`
**Requirement**: GN-27 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `each_generics_group_item_resolves_to_its_std_declaration` passes under `**Run stdlibrary**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(types): library items resolve to their std source`

---

### T106: `since` and `MinPulse` against §22

**What**: Source tests: `since(d)` reads `now - t` of `d`'s last change and `inf` before the first (§22.1.8); `MinPulse { w }` fails its `require` exactly for a pulse narrower than `w` (§22.5.7); edges placed by `at(..)`, expected instants computed in Rust in the test.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T105
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `since_reads_the_time_from_the_last_change` and `min_pulse_fails_exactly_for_a_narrow_pulse` pass under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): since and MinPulse against section 22`

---

### T107: `Setup`, `Hold` and `Stable` against §22

**What**: Source tests: `Setup { ts }` flags a violation exactly when `d` changed less than `ts` before a rising `clk` (and `viol` follows); `Hold { th }` exactly when `clk` was high less than `th` before `d` changed; `Stable` exactly when `d` changes while `en` holds and did not just change (§22.5.4 to §22.5.6), at `T = u8` and at an enum.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T106
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `setup_hold_and_stable_fail_exactly_on_their_violations` passes under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): Setup, Hold and Stable against section 22`

---

### T108: `inertial` and `Short` against §22

**What**: Source tests: `inertial(x, tpd)` gives `x` delayed by `tpd` and swallows a pulse shorter than `tpd` (§22.5.8), at `bool` and `u8`; `Short(a, b)` (§11) inside a library-level divider gives one node's voltage and the divider's current, over `Electrical` and `Thermal`.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T107
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `inertial_delays_and_swallows_short_pulses` and `short_makes_one_node_against_section_eleven` pass under `**Run srcstd**`
- [x] Gate check passes: Full (last task of batch B17)

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): inertial and Short against the spec`

---

### T109: `idt`, `idtmod` and the spec's `Water`

**What**: Source tests: `idt(x, ic, reset)` of a constant `x` gives `ic + x·t`, a `reset` holding `ic` (§22.2.8), `idt::<Thermal>` with an explicit nature; `idtmod` wraps into `[offset, offset + modulus)` (`rem_euclid` in Rust, §22.2.9); the Part IV §13 `Water` model boils at 373.15 K and vaporizes after `mass · hv` joules; the `elab_structure.rs` L40 pins of `idt`/`Water` left by T71 become these acceptances; `GAPS.md` closes L40's `idt`/`Water` part.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 2), GN-31 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `idt_and_idtmod_integrate_against_section_22` and `the_water_model_boils_and_vaporizes` pass under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): idt, idtmod and Water`

---

### T110a: Composite params at a hoisted placement

**What**: A param of a hoisted module that is an array or a tuple (`num: [f64; N]`, `poles: [(f64, f64); D]`, `table`) is folded to a constant of the IR's arena (`hoisted_params` through `composite_constant`, a given value under the caller's frame bindings and generic substitution), held in `Frame.params`, bound taken apart by `frame_bindings` (`bind_param`), and written out where read (`constant_expr` builds the tree); a node array `signal w: [State; D - 1]` of a hoisted module is its elements' nodes under the frame's prefix, and a loop that indexes an array of nodes is unrolled like one that contributes. Coordinator ruling 2026-11-24: in scope, hoisted placement (planned row L91), no GAPS row.
**Where**: `crates/piperine-lang/src/elab/hoist.rs`, `elab/lower.rs`, `elab/unroll.rs`
**Depends on**: T109
**Reuses**: `composite_constant`, `bind_param`, `tree_expr`, `add_node_array`
**Requirement**: GN-28 (AC 2)
**Executor**: Opus

**Done when**:

- [x] `a_composite_param_of_a_module_with_a_result_checks_at_its_placement` passes under `**Run typesresults**`
- [x] `a_composite_param_of_a_hoisted_placement_is_bound_to_its_constant` passes under `**Run elabresults**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(lang): composite params at a hoisted placement`

---

### T110: The `laplace_*` family

**What**: Source tests: `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp` of a first-order low-pass and a second-order section give the step response the closed form gives at sample instants (§22.2.14); `laplace_nd` with `N > D` is `E0304`; a complex root without its conjugate is `E0317`.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T110a
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `the_laplace_family_gives_the_closed_form_step_response` and `an_improper_or_unpaired_laplace_is_refused` pass under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the laplace family against section 22`

---

### T111: The `zi_*` family

**What**: Source tests: `zi_nd`, `zi_np`, `zi_zd`, `zi_zp` of a first-order filter sampled at `period` give, at each sample, the difference equation run in Rust over the same inputs (§22.2.12, §22.2.13), the steady history after `settle` included.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T110
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `the_zi_family_follows_its_difference_equation` passes under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the zi family against section 22`

---

### T112: `noise_table`

**What**: Source test: a `noise_table` source with a three-point table gives, in a noise analysis, the output PSD the table gives at the table's frequencies and `table_model`'s `1C` interpolation between them, computed in Rust (§22.2.3).
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T111
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `noise_table_gives_the_interpolated_psd` passes under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): noise_table against section 22`

---

### T118: Generation in the body of a hoisted module

**What**: A module with a result or a value input whose body holds generation (`for`/`if` over its items) is hoisted with its generation unrolled under the frame's params, as a body's own module is (`elab/generation.rs`): `GeneratedItems` per frame (keyed by the frame, its range and conditions folded over `frame_bindings`), its carriers, nodes, definitions, placements and `let`s named under the frame's prefix with the iteration's subscripts (`lag.s[1]`); the refusal `"a hoisted module that holds generation in its body,"` (`hoist.rs::hoistable`) leaves and `GAPS.md` L77 closes. Added at the coordinator's request after round 5 of phase C (finding 18: L77 is this feature's leftover, not a spec question); the stdlib's hoisted forms of this phase (`laplace_nd`, `zi_*`) are its first users.
**Where**: `crates/piperine-lang/src/elab/generation.rs`, `crates/piperine-lang/src/elab/hoist.rs`
**Depends on**: T112
**Reuses**: `GeneratedItems`, `Frame`, `PlacementKey`
**Requirement**: GN-17 (AC 1)
**Executor**: Opus

**Done when**:

- [x] `generation_in_a_hoisted_module_unrolls_under_its_frame` passes under `**Run elabgenr**`
- [x] `a_hoisted_module_with_generation_runs` passes under `**Run srcgenr**`
- [x] `GAPS.md` drops L77: the edit and the `docs.rs` counts it moves are made in T115 (the counts are pinned in one place and move once)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(lang): unroll generation in a hoisted module's body`

---

### T119: Arrays of templates

**What**: `[A {}, A { r: 2.0 }]` is an array of module templates and `fs[k](x)` places the template at an index, a generation index included: the classifier names the module through an element, and the const evaluator folds the array and the index (GAPS L88). Added by the coordinator 2026-11-24. The Book rows that use the form as their example of a refusal (`compiler~79f44db1`, welcome `~28a764f7`) are rewritten to a form that is still refused by name and re-pinned.
**Where**: `crates/piperine-lang/src/types`, `crates/piperine-lang/src/elab`
**Depends on**: T118
**Reuses**: `elab/templates.rs`
**Requirement**: GN-17
**Executor**: Opus

**Done when**:

- [x] `an_array_of_templates_places_the_element_at_an_index` (and `an_array_of_templates_of_a_module_with_a_result_gives_each_read_its_elements_params`) pass under `**Run elabtemplates**`
- [x] `a_template_array_element_placed_in_generation_runs` passes under `**Run srcgenr**`
- [x] `GAPS.md` drops L88 and the Book rows `compiler~79f44db1` and `welcome~28a764f7` (bundles branch, not on `main` yet) are rewritten: both in T115, which holds the counts; origin/main did not carry them at this task
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(lang): arrays of module templates`

---

### T120: An `Option` result of a hoisted module is read in a reaction

**What**: A module whose result is an `Option<T>` read in a reaction (`y.is_some()`, `y.unwrap_or(0)`, `let y = inertial(x, 5n)` at `T = Option<u8>`) lowers to the two halves of its result expression in the callee's frame (`lower_result_pair`, `elab/compound.rs`, `elab/hoist.rs`): GAPS L89 (found by T108) is closed without a row. Coordinator ruling 2026-11-24 (4): within callables/results scope and modest.
**Where**: `crates/piperine-lang/src/elab/compound.rs`, `crates/piperine-lang/src/elab/hoist.rs`
**Depends on**: T119
**Reuses**: `lower_result`, `lower_option`
**Requirement**: GN-28 (AC 1)
**Executor**: Opus

**Done when**:

- [x] `an_option_result_of_a_hoisted_module_is_read_as_the_pair_of_its_halves` passes under `**Run elabresults**`
- [x] `inertial_over_an_option_reads_its_result_in_a_reaction` passes under `**Run srcstd**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(lang): an Option result of a hoisted module is read in a reaction`

---

### T121: A param declared as an array of templates

**What**: `pair: [Cell; 2] = [Cell { }, Cell { cs: 2.0 }]` is a param (GAPS L89): the checker accepts `Ty::Array` of `Ty::Module` in `types/module.rs::param`; the elaborator treats it as a module param (`is_module_param` and its sites: `classify_module_param_keys`, `lower_param` type `[u32; N]`, `host_override` E0802, `children.rs:414`, `template_through`/`placed_module`) whose value is the array of the template numbers (a composite constant in the keys); `fold_template_element` reads the element of a param from `scope.keys`; a default, an override by a literal, a `const` or a choice, an indexed placement `pair[k](x)` and a generation index run, and every param read by such a value is structural. Coordinator ruling 2026-11-24 (after the proofs): in scope, T119's family. Done 2026-11-24.
**Where**: `crates/piperine-lang/src/types/module.rs`, `crates/piperine-lang/src/elab/{templates,structural,body,children}.rs`
**Depends on**: T119
**Reuses**: `fold_template_element`, `template_items`, `Bindings::bind_composite`
**Requirement**: GN-17 (AC 1)
**Executor**: Opus

**Done when**:

- [x] a checker test, an elab test (default, override by a literal, a `const` and a choice; `pair[k](x)` incl. a generation index; the params it reads are structural) and a source test (`source_generation`) pass
- [x] `GAPS.md` drops L89 and `docs.rs` re-pins the counts (1 silent, 2 broken, 18 loud); check C155 is added to the Checks with those tests, and `proofs-e.md` is regenerated last
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(lang): a param declared as an array of templates`

---

### T113: No item is refused because it is generic

**What**: `std_library.rs` walks L58's generics group (`Short`, `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `cast`, `same`, `MinPulse`) placing or calling each and asserts no refusal names "generic"; an item refused for another form names that form (none of the group is expected to be blocked once DE1 holds, so the test proves the rule on a dependency crate's generic module that places an `extern` module, L33: refused naming `extern` module, never "generic") and `GAPS.md` lists it under that form's id; GN-27's list in `spec.md` no longer names `merge` (struck at planning, DE2).
**Where**: `crates/piperine-lang/tests/std_library.rs`
**Depends on**: T112
**Reuses**: `Elab`, `Types::check_model`
**Requirement**: GN-27 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `no_generics_group_item_is_refused_because_generic` and `a_blocked_item_is_refused_naming_its_form` pass under `**Run stdlibrary**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): no stdlib item is refused because generic`

---

### T114: Galleries and Book rows of library items

**What**: Every gallery fence and Book row that writes a library item of the group is held to the checker (`std_galleries`) or flips (`inventory.tsv`, `GAPS.md` §11); none is left `refused` for a form of this feature.
**Where**: `crates/piperine-lang/tests/std_galleries.rs`
**Depends on**: T113
**Reuses**: `GenericDeclarations`, `tests/book/support`
**Requirement**: GN-31 (AC 2)
**Executor**: Sonnet

**Done when**:

- [x] `the_fences_of_the_generic_library_are_checked` passes under `**Run galleries**`
- [x] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test: the fences and rows of the generic library are checked`

---

### T115: Phase E and feature bookkeeping

**What**: `GAPS.md` drops L58's generics group (its other groups keep their ids) and L40's remaining part, updates §9 (VII §22, II §6) and §10; `spec.md` traceability marks every GN row with its phase and the Coverage line counts them; `.specs/STATE.md` MD-81 gains item 8; `ROADMAP.md`'s `generics` rows say delivered; `.specs/features/generics/checks.md` gathers the `**Build**` line, the union of the `Run` lines and every check of `tasks.md`, `tasks-c.md`, `tasks-d.md` and `tasks-e.md` with their proofs (the GN-30 checks once), so `check_proofs.py generics` runs the whole feature.
**Where**: `GAPS.md`
**Depends on**: T114
**Reuses**: `docs.rs`
**Requirement**: GN-31 (AC 1)
**Executor**: Sonnet

**Done when**:

- [x] `gaps_drops_the_generics_group_of_l58` and `the_generics_checks_file_holds_every_phase` pass under `**Run docs**`
- [x] Gate check passes: Quick and the Book targets (the user's gate policy of 2026-11-24: no full workspace suite per task; the proofs of the closing round run them)

**Tests**: integration
**Gate**: full

**Commit**: `docs: generics phase E closes the L58 generics group`

---

## Checks

**C142** - No item of L58's generics group is refused because it is generic. (GN-27, AC 1; T105, T113)
Proof: run stdlibrary | test each_generics_group_item_resolves_to_its_std_declaration ... ok
Proof: run stdlibrary | test no_generics_group_item_is_refused_because_generic ... ok

**C143** - An item that needs another feature's form is refused naming that form and listed under its id. (GN-27, AC 2; T113)
Proof: run stdlibrary | test a_blocked_item_is_refused_naming_its_form ... ok

**C144** - Every item of the group type-checks under the whole-model check with no instantiation and no error. (GN-27, AC 3; T104)
Proof: run typesmodel | test the_standard_library_is_checked_whole_with_no_instantiation ... ok

**C145** - `since`, `MinPulse`, `Setup`, `inertial` and `Short` run and give their §22 values. (GN-28, AC 1; T106, T107, T108)
Proof: run srcstd | test since_reads_the_time_from_the_last_change ... ok
Proof: run srcstd | test min_pulse_fails_exactly_for_a_narrow_pulse ... ok
Proof: run srcstd | test setup_hold_and_stable_fail_exactly_on_their_violations ... ok
Proof: run srcstd | test inertial_delays_and_swallows_short_pulses ... ok
Proof: run srcstd | test short_makes_one_node_against_section_eleven ... ok

**C146** - Each item whose last other form has landed runs against its §22 entry: `Stable`, `Hold`, `idt`, `idtmod`, `laplace_*`, `zi_*`, `noise_table`. (GN-28, AC 2; T107, T109 to T112)
Proof: run srcstd | test idt_and_idtmod_integrate_against_section_22 ... ok
Proof: run srcstd | test the_laplace_family_gives_the_closed_form_step_response ... ok
Proof: run srcstd | test an_improper_or_unpaired_laplace_is_refused ... ok
Proof: run srcstd | test the_zi_family_follows_its_difference_equation ... ok
Proof: run srcstd | test noise_table_gives_the_interpolated_psd ... ok

**C147** - The spec's `Water` model (Part IV §13) runs over the stdlib `idt`. (GN-28, AC 2; L40; T109)
Proof: run srcstd | test the_water_model_boils_and_vaporizes ... ok

**C148** - `GAPS.md` drops the L58 generics group and L40's last part; the feature's `checks.md` holds every phase. (GN-31, AC 1; T115)
Proof: run docs | test gaps_drops_the_generics_group_of_l58 ... ok
Proof: run docs | test the_generics_checks_file_holds_every_phase ... ok
Proof: run docs | test the_generics_check_ids_are_unique_across_its_phases ... ok

**C135** - A hoisted module whose body holds generation is unrolled under its frame and runs; `GAPS.md` has no L77. (GN-17, AC 1; T118, phase C round 5 finding 18)
Proof: run elabgenr | test generation_in_a_hoisted_module_unrolls_under_its_frame ... ok
Proof: run srcgenr | test a_hoisted_module_with_generation_runs ... ok

**C149** - Every gallery fence and Book row of a library item of the group is held to the checker. (GN-31, AC 2; T114)
Proof: run galleries | test the_fences_of_the_generic_library_are_checked ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C152** - A composite param (an array of coefficients, a table, a tuple) of a module with a result is bound at its placement, and `laplace_*` and `noise_table` read theirs. (GN-28, AC 2; T110a, T110, T112; coordinator ruling 2026-11-24)
Proof: run typesresults | test a_composite_param_of_a_module_with_a_result_checks_at_its_placement ... ok
Proof: run elabresults | test a_composite_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok
Proof: run elabresults | test an_option_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok
Proof: run elabtemplates | test a_template_carrying_composite_params_places_with_and_without_a_result ... ok
Proof: run elabtemplates | test two_templates_with_equal_composite_params_are_one_value ... ok
Proof: run srcgenr | test a_template_carrying_composite_params_runs_through_a_placement ... ok

**C153** - An array of module templates places the element a written, constant or generation index names, and a module with a result placed through a template takes the template's params. (GN-17, AC 1; T119, GAPS L88)
Proof: run elabtemplates | test an_array_of_templates_places_the_element_at_an_index ... ok
Proof: run elabtemplates | test an_array_of_templates_of_a_module_with_a_result_gives_each_read_its_elements_params ... ok
Proof: run typestempl | test an_array_of_templates_of_one_module_checks_and_places_its_element ... ok
Proof: run srcgenr | test a_template_array_element_placed_in_generation_runs ... ok
Proof: run elabtemplates | test equal_arrays_of_templates_share_one_body_keyed_by_value ... ok
Proof: run elabresults | test a_module_with_a_result_placed_through_a_callable_bound_param_takes_the_templates_params ... ok
Proof: run srcgenr | test a_module_with_a_result_through_a_callable_bound_param_runs ... ok
Proof: run elabtemplates | test a_template_param_of_a_module_with_a_result_places_the_templates_params ... ok

**C154** - An `Option` result of a module placed in a body is read in a reaction as the pair of its halves, and `inertial` runs at `Option<u8>`. (GN-28, AC 1; T120, GAPS L89 as found by T108)
Proof: run elabresults | test an_option_result_of_a_hoisted_module_is_read_as_the_pair_of_its_halves ... ok
Proof: run srcstd | test inertial_over_an_option_reads_its_result_in_a_reaction ... ok

**C155** - A param declared as an array of templates (`pair: [Cell; 2]`) runs: its default, an override by a literal, a `const` and a choice, `pair[k](x)` at a written and a generation index, the params its elements read are structural, and a host cannot give it a value (E0802). (GN-17, AC 1; T121, GAPS L89)
Proof: run typestempl | test a_param_declared_as_an_array_of_templates_checks_its_default_and_its_placement ... ok
Proof: run elabtemplates | test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
Proof: run elabtemplates | test the_params_an_array_of_templates_reads_become_structural ... ok
Proof: run srcgenr | test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
Proof: run apisession | test a_module_typed_param_is_not_set_by_text ... ok
Proof: run docs | test gaps_drops_the_generics_group_of_l58 ... ok

**C150** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C151** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

---

## Handoff

The phase's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T103 (2026-11-24): phase D (b60dcd9c6) and `analog-operators` (MD-82: L43-L45, L47, L49, L60, L72 closed; `delay` and `noise` and the noise analysis on `main`) are both on `main`; the merge of origin/main was a fast-forward and the Build line is green. Every L58 generics-group item exists in `crates/piperine-lang/std/` (`analog.msl`: `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`; `monitor.msl`: `Stable`, `Hold`, `Setup`, `MinPulse`, `inertial`; `values.msl`: `since`; `carriers.msl`: `Short`); findings on which fail at run time are appended per task. GAPS L88 (arrays of templates) is owned by phase E: added as T119 (after T118). Gate policy of the user (2026-11-24): per task the Build line and touched crates/targets only; no full workspace suite. Planning (2026-11-23): design-e.md DE3, DE4; DE1 to DE3 answered by the user 2026-11-23 (design-e.md).
**Amended (2026-11-24, coordinator, phase C round 5 finding 18):** T118 (generation in a hoisted module's body, GAPS L77) and check C135 join phase 22, after T112; the task was phase C's leftover (`tasks-c.md` Handoff, round 4 fix).
**T104 finding (2026-11-24):** `std` under the model check surfaced four stdlib errors, fixed in `std/` and mirrored in `docs/spec/part_vii_stdlib.md` and the preformat fixtures: (1) `zi_zp/zd/np` and `laplace_zp/zd/np` placed `zi_nd`/`laplace_nd` with no way to infer `N`/`D` (E0181): now `::<{ N + 1 }, D>` etc. (a coefficient array of `poly_q`/`poly_s` is one longer than its roots); (2) `laplace_nd` summed `pos(w[k])` inside a closure, which reads a carrier outside a handle (E0450, §3 rule 7): the output is now a node `y: State` driven in the `analog` block by a `for` sum (an extra algebraic row, same value); (3) `TribufDelayed` placed `inertial` without `T` (E0181): `inertial::<Driven>`. `std_library.rs::calls_named` strips a turbofish from the callee text before comparing (same assertion). T110/T111 must run these against closed forms.
**T106 finding:** design-e.md's table says `since` is `inf` "before the first change"; part VII §22.1.8 (rules 2-3 and its table) makes the first commit at (0, 0) a change, so a read after (0, 0) is `now - 0` and only a reaction on that first change reads `inf`. The tests follow the spec text; the oracle (`Section22`) is written from it. `MinPulse` therefore also checks at (0, 0) (width `inf`, passes).
**T108 findings (AFK default 2026-11-24; **Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu"):** (1) the task text says `inertial` at `bool` and `u8`; part VII §22.5.8 rule 7 and part I §3.3.4 say `bool`, `UInt`, `Int` and `Float` do **not** implement `Default` (on purpose), so `inertial` is E0187 at them. The spec stands; the proof runs at two enums with a `Default` (`Level`, `Mode`) and `inertial_at_a_type_without_default_is_refused` pins the refusal at `bool`. (2) A module result of type `Option<u8>` read in a reaction (`let y = inertial(x, 5n)` at `T = Option<u8>`, `y.is_some()`) is `P0001` "this form of an `Option` value" (`elab/compound.rs::lower_option`): not in GAPS. New row **L89** (loud, owner `spec-closure`) is added in T115 together with the counts (docs.rs pins every count, so the GAPS edit waits for T115/T118, with L77, L88 and the L58 group). `inertial` is read by sampling every ns (the result is a `T`, not a carrier with `changed()`).
**T109 findings (AFK default 2026-11-24; **Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu"):** (1) `Water` (Part IV §13) runs: two fixes made it. `types/analog.rs::contributes_potential_to` saw only top-level statements of the `analog` block, so `pwr(t)` beside `temp(t) +> tb` in a `match` arm was refused as a flow read (it now walks nested statements, §11 rule 3); and the test model drives the heater with `pwr(t, gnd)` (the node first). `elab_structure.rs::the_water_model_places_idt_and_waits_for_the_flow_probe` stays: its source has no `analog` block, so it still pins L73's form; the acceptance is `the_water_model_boils_and_vaporizes`. (2) transpile: an `Option<f64>` carrier read in the generic-real analog block was printed `Option<R>` / unlifted (`print/value.rs::pin_type`, `carrier_value`); both fixed (idtmod's `base`). (3) **idtmod's wrap does not happen**: the digital side is right (`seat` pulses, `base` = `Some(acc - modulus)` at the crossing) but the analog solve never sees the `seat` pulse (§22.2.9 rule 3 needs a jump solve between the microsteps; GAPS X1: "§15 describes a solver that is gone"), so the integral keeps growing past `offset + modulus`: a **silent wrong result**. The test holds idtmod to §22.2.9 before the first wrap only (marked SPEC_DEVIATION in the test). New GAPS row **L90** (the wrap of `idtmod`; owner `spec-closure`, solver) is added in T115. `idt` (State and `Thermal`, `reset`) matches the closed form.
**STOP at T110 (2026-11-24, executor, context budget; T103 to T109 done and pushed).** T110 (`laplace_*`), T111 (`zi_*`) and T112 (`noise_table`) are blocked by one gap, found by placing them from source (`laplace_nd::<1, 2>(step) { num: [1.0], den: [1.0, 1u] }`): `P0001` "a composite value where a scalar is required in a param value of a module with a result is not supported yet" (`elab/hoist.rs::hoisted_params` folds each param with `consts.value`, which refuses a composite; `Frame.params` is `BTreeMap<DeclId, Constant>`, scalar only, so the array params `num`, `den`, `zeros`, `poles` and `table` cannot be bound at a hoisted placement). Design-e.md's risk table said to give this its own GAPS row owned by `composite-values`: new row **L91** (loud, owner `composite-values`, found by `generics` phase E T110), to be added in T115 with L89 (Option module result read in a reaction, `spec-closure`) and L90 (idtmod wrap, `spec-closure`, solver). A fix needs: `hoisted_params` folding a composite to a `ConstTree`, `Frame.params` holding it, `frame_bindings` binding it (`Bindings::tree`), and the readers of a param array in a hoisted frame (`num[k]`) lowering through it. The closed-form step-response models and tests I drafted (`laplace.msl`: four forms at tau = 1 us and a second-order section with poles -1e6 +- j2e6; oracle `1 - exp(-t/tau)` and `1 - e^(-a t)(cos b t + (a/b) sin b t)`; refusals E0304 for `N > D`, E0317 for an unpaired root) were reverted, not committed. Remaining: T110, T111, T112 (blocked), T118 (generation in a hoisted body), T119 (arrays of templates, GAPS L88: not yet added to this file), T113, T114, T115. The `idtmod` wrap (L90) and the three gaps above also mean C146 (`laplace_*`, `zi_*`, `noise_table`, idtmod's wrap) cannot be met without them.
**Coordinator rulings (2026-11-24):** (1) T110a (composite params at a hoisted placement) is in scope, before T110; no GAPS row for it (L91 is not added). (2) T119 (arrays of templates, L88) after T118. (3) L90 (idtmod wrap) is fixed on main by another agent: T115 records what main has; merge origin/main before T115. (4) L89: make it run if modest, else a GAPS row in T115. (5) `inertial` default accepted (marked above); `since` follows the spec. The Book rows `compiler~79f44db1` and `welcome~28a764f7` use the array-of-templates form as a refusal example: T119 rewrites both after merging origin/main.
**T118 (2026-11-24):** each hoisted frame unrolls the generation of its own module (`Frame.generated`, parked and activated by `enter_frame`/`leave_frame`/`hoist_frame`; `GeneratedItems.frame` is the frame its lengths fold over); the `frame == 0` guards of carriers, elements, drives, definitions, `let`s, placement keys and nodes are gone; the refusal `generation in its body` left. The GAPS L77 row and the docs.rs counts move in T115.
**T119 (2026-11-24):** `[A {}, A { r: 2.0 }]` and `fs[k](x)` run for a `let`, a `const` and a generation index: the checker takes `Ty::Module` as an array element (`types/aggregate.rs`), the classifier names the module through an element (`array_template`, `callee_class`), `fold_template_element` folds array and index (`elab/templates.rs`). Found on the way, a **silent wrong result** fixed: a module with a result placed through a template value (`let t = Scale { r: 3.0 }; t(1.0)`) ignored the template's params and used the defaults; `hoist()` now takes them from `template_through`. Still refused (loud, to be recorded in T115 as new row L91, owner `generics`): a `param` declared `[Cell; 2]` (`param of type [module Cell; 2] is not supported yet`).
**T110a (2026-11-24):** the array/tuple param, the caller-frame fold (a given value reads the caller's params, loop variables and generic substitution) and `w[k]` in a hoisted frame needed four fixes: `hoisted_params` composite fold, `constant_expr` writing an aggregate out, `hoist_nodes` taking node arrays, and `held_by_loop` unrolling a loop that indexes a node array (`Held::NodeElement`, E0416 when its trip count does not fold).
**T115 (2026-11-24):** GAPS counts: 1 silent (S9, the wrap of `idtmod`: `fix/idtmod-wrap` is not on `main` yet; when it merges, S9 and the `SPEC_DEVIATION` guard in `idt_and_idtmod_integrate_against_section_22` leave and the counts become 0 silent, 2 broken, 19 loud), 19 loud (L40, L77, L88 closed, L89 found). Book: all 28 `book_*` and `cli book` targets pass; no Book row used a form this phase changed, so none flipped; the two bundles-branch rows (`compiler~79f44db1`, `welcome~28a764f7`) are not on this branch's base and are the merger's to re-pin if T119 makes their example run. Open for the proofs round: none.
**STOP before T121 (2026-11-24, context budget):** T121 is planned in full above and not started (no code). Sites found by reading: `types/module.rs:93` (the refusal), `elab/templates.rs::is_module_param` and its uses (`body.rs:258` the param's IR type is `UInt(32)` and must be `Array(UInt(32), N)`; `structural.rs:370, 601, 645, 994`; `children.rs:198, 250, 414`; `host_override` E0802), `classify.rs::array_template` already names the module through a `Param`/`Port` of array type, and `fold_template_element` reads only a literal, `let` or `const` array (a param's array must come from `scope.keys`).
**T121 (2026-11-24):** a param declared `[Cell; 2]` runs. Checker: `types/module.rs::param` takes an array of `Ty::Module`. Elab: `templates.rs` (`template_array_length`, `is_template_param`, `fold_template_array` over a literal, a `let`, a `const`, a param or an `if` choice; the value is an aggregate `[u32; N]` of template numbers in the keys; `fold_template_element` reads a param's element from the bindings); `is_module_param` now excludes an array (its type resolved to the element's module). The keys, the host E0802, the placed param fold and the IR type (`body.rs`) go through `is_template_param`. **Found and fixed on the way:** `sub_scope` (`drives.rs`) dropped `scope.keys`, so a placement through a key param read in a definition (`y[k] = fs[k](a)`) was a `P0001` unbound param; it keeps them now. GAPS: L89 closed, counts 1 silent, 2 broken, 18 loud.
**Dispatched:** none yet.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 21 → Phase 22

Phase 21:  T103 → T104 → T105 → T106 → T107 → T108
Phase 22:  T109 → T110a → T110 → T111 → T112 → T118 → T119 → T120 → T113 → T114 → T115
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B17 | 21 | T103 to T108 | after phase D and `analog-operators` B4–B7 (DE1); `std` joins the model check |
| B18 | 22 | T109 to T115, T118 (after T112) | closes the L58 generics group and the feature; then `check_proofs.py generics` and the Verifier |

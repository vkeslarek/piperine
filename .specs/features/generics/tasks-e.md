# Generics Tasks (phase E: the stdlib built on generics)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/generics/design-e.md`
**Status**: Draft
**Approved**: (written when the user approves)
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesmodel**: `cargo test -p piperine-lang --test types_model`
**Run stdlibrary**: `cargo test -p piperine-lang --test std_library`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
**Run srcstd**: `cargo test -p piperine-parity --test source_stdlib_generic`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
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
T109 → T110 → T111 → T112 → T113 → T114 → T115
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

- [ ] The merge is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test --workspace` pass
- [ ] The Handoff names each finding and the task it affects

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

- [ ] `the_standard_library_is_checked_whole_with_no_instantiation` passes under `**Run typesmodel**` (it replaces `the_standard_library_is_left_to_its_own_check`, named in the commit body)
- [ ] Gate check passes: Quick

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

- [ ] `each_generics_group_item_resolves_to_its_std_declaration` passes under `**Run stdlibrary**`
- [ ] Gate check passes: Quick

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

- [ ] `since_reads_the_time_from_the_last_change` and `min_pulse_fails_exactly_for_a_narrow_pulse` pass under `**Run srcstd**`
- [ ] Gate check passes: Quick

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

- [ ] `setup_hold_and_stable_fail_exactly_on_their_violations` passes under `**Run srcstd**`
- [ ] Gate check passes: Quick

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

- [ ] `inertial_delays_and_swallows_short_pulses` and `short_makes_one_node_against_section_eleven` pass under `**Run srcstd**`
- [ ] Gate check passes: Full (last task of batch B17)

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

- [ ] `idt_and_idtmod_integrate_against_section_22` and `the_water_model_boils_and_vaporizes` pass under `**Run srcstd**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): idt, idtmod and Water`

---

### T110: The `laplace_*` family

**What**: Source tests: `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp` of a first-order low-pass and a second-order section give the step response the closed form gives at sample instants (§22.2.14); `laplace_nd` with `N > D` is `E0304`; a complex root without its conjugate is `E0317`.
**Where**: `crates/piperine-parity/tests/source_stdlib_generic.rs`
**Depends on**: T109
**Reuses**: `crates/piperine-parity/tests/stdlib_generic/*.msl`
**Requirement**: GN-28 (AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `the_laplace_family_gives_the_closed_form_step_response` and `an_improper_or_unpaired_laplace_is_refused` pass under `**Run srcstd**`
- [ ] Gate check passes: Quick

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

- [ ] `the_zi_family_follows_its_difference_equation` passes under `**Run srcstd**`
- [ ] Gate check passes: Quick

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

- [ ] `noise_table_gives_the_interpolated_psd` passes under `**Run srcstd**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): noise_table against section 22`

---

### T113: No item is refused because it is generic

**What**: `std_library.rs` walks L58's generics group (`Short`, `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `cast`, `same`, `MinPulse`) placing or calling each and asserts no refusal names "generic"; an item refused for another form names that form (none of the group is expected to be blocked once DE1 holds, so the test proves the rule on a dependency crate's generic module that places an `extern` module, L33: refused naming `extern` module, never "generic") and `GAPS.md` lists it under that form's id; GN-27's list in `spec.md` no longer names `merge` (struck at planning, DE2).
**Where**: `crates/piperine-lang/tests/std_library.rs`
**Depends on**: T112
**Reuses**: `Elab`, `Types::check_model`
**Requirement**: GN-27 (AC 1, AC 2)
**Executor**: Sonnet

**Done when**:

- [ ] `no_generics_group_item_is_refused_because_generic` and `a_blocked_item_is_refused_naming_its_form` pass under `**Run stdlibrary**`
- [ ] Gate check passes: Quick

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

- [ ] `the_fences_of_the_generic_library_are_checked` passes under `**Run galleries**`
- [ ] `refused_rows_and_gaps_section_11_agree` passes under `**Run bookinventory**`
- [ ] Gate check passes: Quick

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

- [ ] `gaps_drops_the_generics_group_of_l58` and `the_generics_checks_file_holds_every_phase` pass under `**Run docs**`
- [ ] Gate check passes: Full (last task of batch B18)

**Tests**: integration
**Gate**: full

**Commit**: `docs: generics phase E closes the L58 generics group`

---

## Checks

**C110** - No item of L58's generics group is refused because it is generic. (GN-27, AC 1; T105, T113)
Proof: run stdlibrary | test each_generics_group_item_resolves_to_its_std_declaration ... ok
Proof: run stdlibrary | test no_generics_group_item_is_refused_because_generic ... ok

**C111** - An item that needs another feature's form is refused naming that form and listed under its id. (GN-27, AC 2; T113)
Proof: run stdlibrary | test a_blocked_item_is_refused_naming_its_form ... ok

**C112** - Every item of the group type-checks under the whole-model check with no instantiation and no error. (GN-27, AC 3; T104)
Proof: run typesmodel | test the_standard_library_is_checked_whole_with_no_instantiation ... ok

**C113** - `since`, `MinPulse`, `Setup`, `inertial` and `Short` run and give their §22 values. (GN-28, AC 1; T106, T107, T108)
Proof: run srcstd | test since_reads_the_time_from_the_last_change ... ok
Proof: run srcstd | test min_pulse_fails_exactly_for_a_narrow_pulse ... ok
Proof: run srcstd | test setup_hold_and_stable_fail_exactly_on_their_violations ... ok
Proof: run srcstd | test inertial_delays_and_swallows_short_pulses ... ok
Proof: run srcstd | test short_makes_one_node_against_section_eleven ... ok

**C114** - Each item whose last other form has landed runs against its §22 entry: `Stable`, `Hold`, `idt`, `idtmod`, `laplace_*`, `zi_*`, `noise_table`. (GN-28, AC 2; T107, T109 to T112)
Proof: run srcstd | test idt_and_idtmod_integrate_against_section_22 ... ok
Proof: run srcstd | test the_laplace_family_gives_the_closed_form_step_response ... ok
Proof: run srcstd | test an_improper_or_unpaired_laplace_is_refused ... ok
Proof: run srcstd | test the_zi_family_follows_its_difference_equation ... ok
Proof: run srcstd | test noise_table_gives_the_interpolated_psd ... ok

**C115** - The spec's `Water` model (Part IV §13) runs over the stdlib `idt`. (GN-28, AC 2; L40; T109)
Proof: run srcstd | test the_water_model_boils_and_vaporizes ... ok

**C116** - `GAPS.md` drops the L58 generics group and L40's last part; the feature's `checks.md` holds every phase. (GN-31, AC 1; T115)
Proof: run docs | test gaps_drops_the_generics_group_of_l58 ... ok
Proof: run docs | test the_generics_checks_file_holds_every_phase ... ok

**C117** - Every gallery fence and Book row of a library item of the group is held to the checker. (GN-31, AC 2; T114)
Proof: run galleries | test the_fences_of_the_generic_library_are_checked ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C118** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; every batch)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C119** - The build gives no warning on the crates this phase touches, and the hygiene guard passes. (GN-30, AC 2; every batch)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-lang --test suite_hygiene`

---

## Handoff

The phase's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none yet. Planning (2026-11-23): design-e.md DE3, DE4; DE1 to DE3 answered by the user 2026-11-23 (design-e.md).
**Dispatched:** none yet.
**Walked:** none yet.

---

## Phase Execution Map

```
Phase 21 → Phase 22

Phase 21:  T103 → T104 → T105 → T106 → T107 → T108
Phase 22:  T109 → T110 → T111 → T112 → T113 → T114 → T115
```

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B17 | 21 | T103 to T108 | after phase D and `analog-operators` B4–B7 (DE1); `std` joins the model check |
| B18 | 22 | T109 to T115 | closes the L58 generics group and the feature; then `check_proofs.py generics` and the Verifier |

# Unknown Semantics: Per-Level Flags Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/unknown-semantics-per-level/design.md`
**Status**: Approved
**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run abilevels**: `cargo test -p piperine-abi --test known_levels`
**Run load**: `cargo test -p piperine-transpile --test load`
**Run dslevels**: `cargo test -p piperine-digital-solver --test known_levels`
**Run solverbind**: `cargo test -p piperine-solver --test bind`
**Run rt**: `cargo test -p piperine-transpile --lib rt::flag`
**Run planshape**: `cargo test -p piperine-plan --test digital_shape`
**Run plancomp**: `cargo test -p piperine-plan --test composite_shape`
**Run irdigital**: `cargo test -p piperine-ir --test digital`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run tlevels**: `cargo test -p piperine-transpile --test levels`
**Run elabunknowns**: `cargo test -p piperine-lang --test elab_unknowns`
**Run sourcelevels**: `cargo test -p piperine-parity --test source_levels`
**Run apilevels**: `cargo test -p piperine-api --test levels`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run clisvg**: `cargo test -p piperine-cli --lib render::svg`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run bookvalues**: `cargo test -p piperine --test book_values`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run clibook**: `cargo test -p piperine-cli --test book`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** Q1 = C and Q2 = b (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"). The defaults taken while the user is away are design.md "Decisions taken (user AFK)" D1 to D20; every task follows them as written.

**Base and order.** Batches B1 to B4 (phases 1 to 8, T1 to T26) start from `main` at `856d2668` and need neither `composite-values` nor `value-semantics`. B5 and B6 (phases 9 to 12, T27 to T34) start only once `composite-values` phases A–B are on `main`; T27 rebases this branch onto that tree. If they are not on `main` when B4 closes, the feature pauses there and reports. The feature merges once, after T34 (D15).

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-b TMPDIR=/var/tmp/piperine-tmp-b CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command (or the lane the orchestrator assigns at dispatch; never a target directory another worktree builds in). Cargo with `-p <crate>`, never `--workspace` (`piperine-lang-server` does not build), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report. Never `git stash`.
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. Every buck target passes after every task that touches the runtime, the printer, the plan, the digital solver or the elaborator.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names, known bytes only, never a value byte. A task that finds another assertion that must move stops and lists it on the Handoff first. The commit body names each moved assertion.
- **The code follows the spec.** `docs/spec/` is already per level; no spec sentence changes. Where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites (§2, §3.3.2, §3.3.3, §3.3.11, §3.3.13, §22.1), or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function, test helpers included (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR enum variants are appended last. The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/levels/*.msl`, written in the subset both `main` and the value-semantics amendment accept (D11: no `let mut`, no `&`, no `Clone`/`Copy` derive).
- **Cross-lane files** (design.md "Cross-lane touches"): `docs.rs` is appended to; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No merge; push only the branch, as backup, when the orchestrator says so.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-61 (the device ABI), MD-63 (the compiled backend), MD-70 (unknown semantics in generated code).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Device ABI (`piperine-abi`) | integration | every UL-05 AC on hand-built layouts; each validation rule tripped by its own negative test | `crates/piperine-abi/tests/known_levels.rs` | `cargo test -p piperine-abi --test known_levels` |
| Loader (`piperine-transpile/src/load.rs`) | integration | the minor rule, its negative test | `crates/piperine-transpile/tests/load.rs` | `cargo test -p piperine-transpile --test load` |
| Digital solver | integration | D1 per level, `MERGE` per level, a minor-1 device unchanged | `crates/piperine-digital-solver/tests/known_levels.rs` | `cargo test -p piperine-digital-solver --test known_levels` |
| Solver bind | integration | host-written bytes carry the full mask | `crates/piperine-solver/tests/bind.rs` | `cargo test -p piperine-solver --test bind` |
| Runtime (`rt/flag.rs`) | unit | every row of design.md's operator table, every level count 1, 9, 64, signed and unsigned | `crates/piperine-transpile/src/rt/flag.rs` (`mod tests`) | `cargo test -p piperine-transpile --lib rt::flag` |
| Plan | integration | the flag and mask width per leaf, `bool` unchanged, init bytes | `crates/piperine-plan/tests/digital_shape.rs`, `composite_shape.rs` | `cargo test -p piperine-plan --test digital_shape` / `--test composite_shape` |
| IR | integration | the new forms built, checked, printed; the param refusal | `crates/piperine-ir/tests/digital.rs` | `cargo test -p piperine-ir --test digital` |
| Interpreter refusals | integration | each new form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Printer | integration | `Expr::Levels` and a partial init compile and cross the ABI | `crates/piperine-transpile/tests/levels.rs` | `cargo test -p piperine-transpile --test levels` |
| Elaborator and folding | integration | every UL-10 AC 1, 2, 4 row, one source expression per row | `crates/piperine-lang/tests/elab_unknowns.rs` | `cargo test -p piperine-lang --test elab_unknowns` |
| Source-level runs | integration | one test per run-time AC and edge case, from MSL source on the Rust backend; expected values from the rule | `crates/piperine-parity/tests/source_levels.rs`, models in `tests/levels/` | `cargo test -p piperine-parity --test source_levels` |
| Host API | integration | decode of partial, whole and unknown values, tables and records | `crates/piperine-api/tests/levels.rs` | `cargo test -p piperine-api --test levels` |
| CLI | integration (e2e) | every output format through the binary; the plot as unknown | `crates/piperine-cli/tests/run.rs`, `src/render/svg.rs` | `cargo test -p piperine-cli --test run`, `--lib render::svg` |
| The Book | integration | the new listing through the binary and the root package | `tests/book/values.rs`, `tests/book/inventory.tsv`, `crates/piperine-cli/tests/book.rs` | `cargo test -p piperine --test book_values`, `--test book_inventory`, `cargo test -p piperine-cli --test book` |
| Repository records | integration | one assertion per changed `GAPS.md` row and abi.md sentence | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| Rebase re-check (T27) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with one test target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the runtime, the printer, the plan, the digital solver or the elaborator's lowering of an existing form | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins --test commands`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: The ABI contract

```
T1 → T2 → T3 → T4
```

### Phase 2: Loader, digital solver, bind

```
T5 → T6 → T7
```

### Phase 3: The runtime value

```
T8 → T9 → T10 → T11
```

### Phase 4: Layouts and IR forms

```
T12 → T13 → T14
```

### Phase 5: The elaborator and its folding

```
T15 → T16
```

### Phase 6: Source-level proofs

```
T17 → T18 → T19 → T20 → T21
```

### Phase 7: The host

```
T22 → T23 → T24
```

### Phase 8: The Book and the scalar bookkeeping

```
T25 → T26
```

### Phase 9: Rebase (waits for `composite-values` A–B on `main`)

```
T27
```

### Phase 10: Aggregates

```
T28 → T29 → T30 → T31
```

### Phase 11: Enums

```
T32 → T33
```

### Phase 12: Closure

```
T34
```

---

## Task Breakdown

### T1: `KNOWN_LEVELS` and `FieldKnown`

**What**: `field_flag::KNOWN_LEVELS` (bit 1) and `MINOR = 2` in `raw.rs`, the `Field.known` doc naming both forms; `piperine_abi::known::FieldKnown` (design.md C1: `of`, `is_whole`, `is_none`, `level`, `fill`) reading a plain known byte or a mask (D1, D12).
**Where**: `crates/piperine-abi/src/known.rs`
**Depends on**: None
**Reuses**: `layout::leaf_size`, `raw::Field`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-abi/tests/known_levels.rs` created with a `//!` header; `a_levels_field_reads_one_mask_per_element` passes under `**Run abilevels**` (a `U16` field of count 2 with masks `0x0005` and `0x01FF` reads levels per element; a plain known byte reads as one level)
- [x] Code clarity: `FieldKnown` and every method carry `///` docs citing abi.md §3; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): the KNOWN_LEVELS field flag and FieldKnown`

---

### T2: Validation of a levels field

**What**: `validate` refuses `KNOWN_LEVELS` without `HAS_KNOWN` (`Rule::LevelsWithoutKnown`) and a mask past the layout's size (`Rule::MaskOutsideLayout`), each message naming the field; a `KNOWN_LEVELS` field inside the layout validates.
**Where**: `crates/piperine-abi/src/validate.rs`
**Depends on**: T1
**Reuses**: the known-byte rule at `validate.rs:374`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `levels_without_has_known_is_refused_by_name` and `a_mask_outside_the_layout_is_refused_by_name` pass under `**Run abilevels**`, each asserting the rule and the field's name in the message
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): validate levels fields`

---

### T3: P3, P4 and C7 per level

**What**: `is_canonical`, `layout_eq` and `Merge::merge_into` go through `FieldKnown`: canonical when no value bit of an unknown level is set; layouts with and without `KNOWN_LEVELS` on one field are unequal (D4); a merge keeps the levels both values hold known and equal (C7) and clears the rest, value bits included.
**Where**: `crates/piperine-abi/src/layout.rs`
**Depends on**: T2
**Reuses**: `FieldKnown`, the existing `Merge` and its error type
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `canonical_form_holds_per_level`, `a_merge_commit_keeps_the_levels_both_hold` and `pins_with_and_without_levels_are_unequal` pass under `**Run abilevels**`
- [ ] The existing `crates/piperine-abi/tests/layouts.rs` passes unchanged
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): canonical form, layout equality and merge per level`

---

### T4: abi.md amendments

**What**: `.specs/features/device-abi/abi.md`: §3 `Field` and the constant table (`KNOWN_LEVELS` 2), the version row (`MINOR` 2), P3 (a mask: zero is no value; an unknown level's value bit is zero), P4 (the flag counts), C7 and §9 D1 (per level), amendment row 6 closed by this feature; one `docs.rs` assertion per amended sentence.
**Where**: `.specs/features/device-abi/abi.md`
**Depends on**: T3
**Reuses**: the existing sentences, edited in place
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `abi_md_states_known_levels` passes under `**Run docs**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): per-level known masks`

---

### T5: The loader refuses a minor-1 library with the flag

**What**: where `Library.minor` is read, a library whose minor is below 2 and whose shape sets `KNOWN_LEVELS` on any field is refused, naming the field and the minor (D3).
**Where**: `crates/piperine-transpile/src/load.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the minor check at `load.rs:71`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_minor_one_library_with_levels_is_refused` passes under `**Run load**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): refuse levels fields from a minor-1 library`

---

### T6: D1 relaxation and `MERGE` per level in the digital solver

**What**: the D1 loop of `run.rs` clears, per element of a `KNOWN_LEVELS` field, exactly the levels whose value or mask bit still moves (`FieldKnown::clear_moving`), and writes one record to the resolver's log pin; a plain known byte keeps today's whole-field rule. `MERGE` commits on a levels field land per level (through T3).
**Where**: `crates/piperine-digital-solver/src/run.rs`
**Depends on**: T5
**Reuses**: the relaxation loop at `run.rs:380`, `FieldKnown`, `Merge`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-digital-solver/tests/known_levels.rs` created; `a_relaxation_clears_only_the_levels_still_moving` (two drivers in a `RELAX` loop whose level 3 keeps moving: levels 0 to 2 stay known, level 3 cleared, one log record), `a_merge_commit_lands_per_level` and `a_minor_one_byte_device_binds_and_runs_unchanged` (a `BYTE` device built against minor 1) pass under `**Run dslevels**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(digital-solver): relax and merge per level`

---

### T7: Host-written bytes carry the full mask

**What**: `bind.rs` writes the full mask of the field (`FieldKnown::fill`) after a host value, so a host-written `u8` is `[v, 255]` and a `bool` stays `[v, 1]`.
**Where**: `crates/piperine-solver/src/core/bind.rs`
**Depends on**: T6
**Reuses**: `FieldKnown::fill`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_host_value_fills_every_level_of_a_levels_field` passes under `**Run solverbind**`
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(solver): host values fill the known mask`

---

### T8: `rt::KnownLevels` and `Flagged` with a mask

**What**: `KnownLevels(u64)` (design.md C3, D13) and `Flagged<T> { value, known: KnownLevels }`; `Carried::encode`/`decode` write and read a mask of the value's width for types of more than one level and the plain byte for one level; `known`, `unknown`, `is_known`, `known_or`, `high` and every all-or-nothing operator use `is_whole`; every write normalized (P3). Moves the buck and `source_unknowns.rs` assertions design.md lists for T8, after grepping for unlisted ones.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Carried::{LEVELS, to_levels, from_levels}`, `Word`
**Requirement**: UL-02, UL-07
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::a_known_64_level_value_has_a_full_mask`, `rt::flag::tests::a_carried_mask_round_trips_for_1_9_and_64_levels` and `rt::flag::tests::all_or_nothing_operators_need_every_level` pass under `**Run rt**`
- [ ] Every buck target passes with only the listed assertions moved
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): a known mask per level in Flagged`

---

### T9: `&`, `|`, `^` and `!` per level

**What**: the four operators of design.md's table on `Flagged<T: Levels>`.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T8
**Reuses**: `KnownLevels`
**Requirement**: UL-01
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::bitwise_operators_follow_the_level_table` passes under `**Run rt**` (every pair of levels known 0, known 1, unknown, on `u4`)
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): bitwise operators per level`

---

### T10: Shifts per level

**What**: `<<` and `>>` by a known amount move the mask with the value; vacated levels known false, or for a signed `>>` the sign level and its flag; a shift by the width or more follows G5 per level; an unknown amount gives every level unknown.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T9
**Reuses**: `Amount`, `KnownLevels`
**Requirement**: UL-01
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::known_shifts_move_flags_with_levels` and `rt::flag::tests::an_unknown_sign_spreads_on_a_signed_right_shift` pass under `**Run rt**`
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): shifts per level`

---

### T11: `merge`, `same`, `level`, `cast` per level

**What**: `Merge::merge` and `merge_bytes` keep the levels both hold known and equal; `same` compares masks and known values; `level(k)` reads bit `k`; `recast` without tags moves the mask level for level; `with_levels(value, mask)` builds a partly known value.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T10
**Reuses**: `Carried::to_levels`
**Requirement**: UL-02, UL-04
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::merge_keeps_agreeing_levels`, `rt::flag::tests::level_and_same_read_the_mask` and `rt::flag::tests::cast_moves_the_mask_level_for_level` pass under `**Run rt**`
- [ ] `cargo test -p piperine-transpile --test commits` passes unchanged
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): merge, same, level and cast per level`

---

### T12: Carrier and record layouts with `KNOWN_LEVELS`

**What**: `CarrierLayout::of` gives every leaf of more than one level `HAS_KNOWN | KNOWN_LEVELS` with the mask at the old known offset (D1, D2); `RecordLayout` follows; `CarrierLayout::bytes` writes the full mask for a known constant.
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `CarrierLayout`, `RecordLayout`
**Requirement**: UL-05
**Executor**: Sonnet

**Done when**:

- [ ] `levels_carriers_declare_a_mask_of_the_leaf_width` and `bool_carriers_keep_the_plain_known_byte` pass under `**Run planshape**`; the existing rows of `digital_shape.rs` move as design.md lists
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): carriers of more than one level declare a known mask`

---

### T13: IR `Expr::Levels` and `Constant::Levels`

**What**: both appended last (D20); `Ir::check` refuses a `Constant::Levels` in a param slot and bits outside the type's levels; the structural printer prints them; the interpreter refuses both by name.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T12
**Reuses**: `Expr::Unknown`'s arms in `check.rs`, `print.rs`, `piperine-interp/src/walk.rs`
**Requirement**: UL-03, UL-10
**Executor**: Sonnet

**Done when**:

- [ ] `partly_known_constants_are_built_checked_and_printed` and `a_partly_known_param_slot_is_refused` pass under `**Run irdigital**`
- [ ] `partly_known_constants_are_refused_by_name` passes under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): partly known constants`

---

### T14: The printer of partly known constants

**What**: `Expr::Levels` prints `rt::Flagged::with_levels(value, mask)`; a carrier's `Constant::Levels` init is laid into its init bytes through `CarrierLayout::bytes`.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: T13
**Reuses**: the printing of `Expr::Unknown` and of carrier inits
**Requirement**: UL-03
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-transpile/tests/levels.rs` created; `a_partly_known_constant_compiles_and_crosses_the_abi` and `a_partly_known_init_starts_the_pin` pass under `**Run tlevels**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): print partly known constants`

---

### T15: `Folded::Levels`

**What**: the partial fold (design.md C6): `logic` with `x`; `&`, `|`, `^`, `!`, known shifts, `merge`, `cast` and an `if` on an unknown condition per level; a fully known result normalized to `Known`, a zero mask to `Unknown`.
**Where**: `crates/piperine-lang/src/elab/consts.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Folded`, `ConstEval::logic`, `Width`
**Requirement**: UL-10
**Executor**: Sonnet

**Done when**:

- [ ] `logic_with_x_folds_to_partly_known_levels` and `constant_operators_fold_per_level` pass under `**Run elabunknowns**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): fold per level`

---

### T16: Lowering of partial folds

**What**: `logic` and any partly known folded value lower to `Expr::Levels`; a carrier initialized by one gets `Constant::Levels`; a param that would fold partly known is the existing unknown-param refusal (D10).
**Where**: `crates/piperine-lang/src/elab/intrinsics.rs`
**Depends on**: T15
**Reuses**: `constant_expr`, `unknown_expr`
**Requirement**: UL-03, UL-10
**Executor**: Sonnet

**Done when**:

- [ ] `a_partly_known_param_is_refused_as_unknown` and `logic_with_x_lowers_to_partly_known_levels` pass under `**Run elabunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): lower partly known constants`

---

### T17: Source proofs of the operators

**What**: `crates/piperine-parity/tests/source_levels.rs` and `tests/levels/operators.msl`: the S6 probe, each operator's table over `u8` values built by `merge`, known and unknown shifts, the all-or-nothing operators, an `analog` read through `known_or`.
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the harness of `source_unknowns.rs` (as a struct with methods)
**Requirement**: UL-01
**Executor**: Sonnet

**Done when**:

- [ ] `the_s6_probe_gives_zero`, `and_keeps_each_level_a_known_false_decides`, `or_keeps_each_level_a_known_true_decides`, `xor_and_not_are_known_where_every_operand_is`, `a_known_shift_moves_each_level_with_its_flag`, `arithmetic_comparison_as_and_unknown_shifts_are_all_or_nothing`, `a_signed_right_shift_spreads_an_unknown_sign`, `a_shift_by_the_width_or_more_fills_by_the_rule` and `analog_reads_a_partly_known_value_as_its_default` pass under `**Run sourcelevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): operators per level from source`

---

### T18: Source proofs of `merge`, `if` and `level`

**What**: `tests/levels/merges.msl`: `merge`, the unknown `if` commit, the multiplexer, `level`, `known`/`known_or`/`is_known`, `same`.
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: T17
**Reuses**: T17's harness
**Requirement**: UL-02
**Executor**: Sonnet

**Done when**:

- [ ] `merge_keeps_the_levels_both_values_agree_on`, `an_unknown_if_commits_the_level_merge`, `a_multiplexer_with_an_unknown_select_keeps_six_levels`, `level_reads_each_level_of_a_partly_known_value`, `known_and_known_or_treat_a_partly_known_value_as_unknown` and `same_compares_levels_and_flags` pass under `**Run sourcelevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): merge and level per level from source`

---

### T19: Source proofs of patterns and `cast`

**What**: `tests/levels/patterns.msl`: `logic("10xx")`, the two `matches_bits` rows of §22.1.4, a carrier initialized by `logic`, `cast::<u8, i8>` of a partly known value.
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: T18
**Reuses**: T17's harness
**Requirement**: UL-03, UL-04
**Executor**: Sonnet

**Done when**:

- [ ] `logic_with_x_leaves_those_levels_unknown`, `matches_bits_x_matches_an_unknown_level`, `matches_bits_zero_or_one_does_not_match_an_unknown_level`, `a_carrier_starts_at_a_partly_known_logic_value` and `cast_keeps_each_level_and_its_flag_per_level` pass under `**Run sourcelevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): patterns and cast per level from source`

---

### T20: Source proofs of changes

**What**: `tests/levels/changes.msl`: commit `merge(0b1010, 0b1011)`, then `0b1010`, then `0b1010` again; count `changed()`.
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: T19
**Reuses**: T17's harness
**Requirement**: UL-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_change_of_one_levels_flag_is_a_change` and `an_equal_commit_of_a_partly_known_value_is_no_change` pass under `**Run sourcelevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): level changes fire changed`

---

### T21: Folded and compiled agree

**What**: `tests/levels/oracle.msl`: one table of expressions over partly known `u8` values, written once as `const` items (folded at elaboration) and once over carriers (compiled); the test compares level and flag row by row (D7).
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: T20
**Reuses**: T17's harness
**Requirement**: UL-10
**Executor**: Sonnet

**Done when**:

- [ ] `folded_and_compiled_levels_agree_row_by_row` passes under `**Run sourcelevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the folder is the reference of the compiled levels`

---

### T22: `Cell::Levels` and `Field::Levels`

**What**: `Cell::decode` reads the mask: whole gives today's cell, zero gives `Cell::Unknown`, anything else `Cell::Levels { bits, known, levels }` (D19); `Field` in `record.rs` follows.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `CarrierLayout`, `Cell::decode`
**Requirement**: UL-06
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/levels.rs` created; `cell_and_field_decode_a_partly_known_value`, `a_fully_known_value_decodes_as_before` and `an_all_unknown_value_decodes_as_unknown` pass under `**Run apilevels**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): decode partly known values`

---

### T23: Host drives write the full mask

**What**: the bytes a host drive or `set` writes for a carrier (`recorder.rs:510`) fill the mask through `FieldKnown::fill`.
**Where**: `crates/piperine-api/src/recorder.rs`
**Depends on**: T22
**Reuses**: `FieldKnown::fill`
**Requirement**: UL-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_host_drive_of_an_integer_is_known_at_every_level` passes under `**Run apilevels**`
- [ ] `cargo test -p piperine-api --test host_sources --test host_actions` pass unchanged
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): host drives fill the known mask`

---

### T24: The CLI prints `0b10xx`

**What**: `Number::text` of `Cell::Levels` is `0b` and one character per level, highest first, `x` where unknown; record lines and CSV use it; JSON Lines writes it quoted; the SVG plot draws it as unknown (D5, D6).
**Where**: `crates/piperine-cli/src/render/number.rs`
**Depends on**: T23
**Reuses**: `Number`, `json.rs`, `csv.rs`, `svg.rs`
**Requirement**: UL-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_partly_known_value_prints_as_a_level_pattern` (table, CSV and record line of one run of `tests/levels/merges.msl`'s top) and `json_lines_quote_a_partly_known_value` pass under `**Run clirun**`
- [ ] `render::svg::tests::a_plot_draws_a_partly_known_value_as_unknown` passes under `**Run clisvg**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(cli): print partly known values as level patterns`

---

### T25: The Book shows a partly known value

**What**: in "Known and unknown", a "Run it" listing of a multiplexer with an unknown select whose record prints a `0b` value, with its `inventory.tsv` row and the CLI chapter's sentence on the `0b10xx` form (D18); every status box that this feature makes true or false says so.
**Where**: `docs/book/content.html`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the listing pattern of `MonitorTop`; `tests/book/values.rs`
**Requirement**: UL-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_multiplexer_with_an_unknown_select_prints_its_known_levels` passes under `**Run bookvalues**`; `**Run bookinventory**` and `**Run clibook**` pass
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): a partly known value in a run`

---

### T26: Scalar bookkeeping

**What**: `GAPS.md` S6 narrowed to aggregates and enum `match` (scalars closed by this branch), G6 noted as decided (C); `consts.rs` and `rt/flag.rs` docs no longer call one flag per value provisional; `docs.rs` assertions updated; the Handoff records B1 to B4.
**Where**: `GAPS.md`
**Depends on**: T25
**Reuses**: the existing rows, edited in place
**Requirement**: UL-01
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_name_s6_for_aggregates_only` passes under `**Run docs**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: S6 narrowed to aggregates`

---

### T27: Rebase onto `composite-values` and re-read the anchors

**What**: rebase this branch onto `main` once `composite-values` phases A–B are merged; resolve `rt/flag.rs`, `consts.rs`, `digital.rs` and `model.rs` keeping both features' behaviour; re-read design.md's "Aggregates and enums" anchors (`LevelWords`, derived `Carried`, `CompositeLayout`, carried composites, IR enums, pattern tests) and record on the Handoff what moved.
**Where**: `.specs/features/unknown-semantics-per-level/tasks.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md "Cross-lane touches"
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] Every test of phases 1 to 8 passes on the rebased tree; the Handoff lists the anchors as found
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `chore: rebase per-level flags onto composite values`

---

### T28: Composite known masks in the runtime

**What**: `Carried::Known` (an associated type, `Copy`): `KnownLevels` for scalars, a fixed array of masks for tuples, arrays and `Option`; `level`, `merge`, `same`, `recast`, field access and indexing move masks; an unknown index merges the admitted elements (§3.3.13).
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `LevelWords`, `KnownLevels`
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::composite_masks_follow_the_section_2_table` and `rt::flag::tests::an_unknown_index_merges_the_admitted_elements` pass under `**Run rt**`
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(rt): known masks for composites`

---

### T29: Printer: masks of derived structs and enums

**What**: the generated `Carried` of each derived struct and enum carries a mask per leaf.
**Where**: `crates/piperine-transpile/src/print/types.rs`
**Depends on**: T28
**Reuses**: composite-values T37's printer
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] `a_derived_struct_crosses_the_abi_with_its_masks` passes under `**Run tlevels**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): derived types carry known masks`

---

### T30: Composite layouts with `KNOWN_LEVELS`

**What**: `CompositeLayout` gives each leaf of more than one level `HAS_KNOWN | KNOWN_LEVELS`.
**Where**: `crates/piperine-plan/src/composite.rs`
**Depends on**: T29
**Reuses**: T12's rule, composite-values T56's layout
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] `composite_leaves_carry_level_masks` passes under `**Run plancomp**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): composite leaves declare known masks`

---

### T31: Source proofs of aggregates

**What**: `tests/levels/aggregates.msl`: a carried `struct { a: u4, b: u4 }`, a tuple, an array and an `Option` built by `merge`; field access, known and unknown indexing, `cast` of a composite.
**Where**: `crates/piperine-parity/tests/source_levels.rs`
**Depends on**: T30
**Reuses**: T17's harness
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] `merge_of_structs_keeps_every_other_level`, `field_access_and_known_index_move_levels`, `an_unknown_index_merges_the_admitted_elements_from_source` and `cast_of_a_composite_keeps_each_level` pass under `**Run sourcelevels**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): aggregates per level from source`

---

### T32: `cast` to an enum of a partly known tag

**What**: the levels with their flags when every admitted tag names a variant; every level unknown otherwise (§22.1.3 rule 2 under §3.3.3).
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `recast`'s tag list
**Requirement**: UL-04
**Executor**: Sonnet

**Done when**:

- [ ] `cast_to_an_enum_merges_or_is_unknown_by_the_admitted_tags` passes under `**Run sourcelevels**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(rt): cast to an enum per level`

---

### T33: `match` on a partly known tag takes the admitted arms

**What**: an arm's pattern test on a partly known tag is known false when a known level contradicts the variant's tag, known true when every level is known and equal, unknown otherwise; arms known false are not taken (§3.3.3).
**Where**: `crates/piperine-lang/src/elab/patterns.rs`
**Depends on**: T32
**Reuses**: composite-values T22's pattern tests
**Requirement**: UL-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_match_on_a_partly_known_tag_takes_only_admitted_arms` passes under `**Run sourcelevels**` (two of four arms' records, each `maybe = true`)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): match takes the arms the known levels admit`

---

### T34: Closure

**What**: `GAPS.md` S6 and G6 removed (and the §5 and cluster rows that name them); `ROADMAP.md` item 11 marks the feature delivered; `.specs/STATE.md` gains the MD entry (next free number at merge, D16) stating Q1 = C, Q2 = b and the trade-off; `docs.rs` asserts S6 and G6 gone; the spec's traceability set to Verified.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the rows edited in T26
**Requirement**: UL-09
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_hold_neither_s6_nor_g6` passes under `**Run docs**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: per-level flags delivered`

---

## Checks

**C1** - A `KNOWN_LEVELS` field reads one mask per element of the leaf's width, bit `k` = level `k`. (UL-05, AC 1; T1)
Proof: run abilevels | test a_levels_field_reads_one_mask_per_element ... ok

**C2** - A `BYTE` device built against minor 1 binds and runs unchanged. (UL-05, AC 2; T6)
Proof: run dslevels | test a_minor_one_byte_device_binds_and_runs_unchanged ... ok

**C3** - `KNOWN_LEVELS` without `HAS_KNOWN` is refused by a rule naming the field. (UL-05, AC 3; T2)
Proof: run abilevels | test levels_without_has_known_is_refused_by_name ... ok

**C4** - A mask past the layout's size is refused by a rule naming the field. (UL-05, AC 3; T2)
Proof: run abilevels | test a_mask_outside_the_layout_is_refused_by_name ... ok

**C5** - A D1 relaxation whose level 3 keeps moving clears level 3 only and writes one log record. (UL-05, AC 4; T6)
Proof: run dslevels | test a_relaxation_clears_only_the_levels_still_moving ... ok

**C6** - P3 holds per level: no value bit of an unknown level is set, and all-zero bytes hold no value. (UL-05, AC 5; T3)
Proof: run abilevels | test canonical_form_holds_per_level ... ok

**C7** - A minor-1 library that sets `KNOWN_LEVELS` is refused, naming the field and the minor. (UL-05, AC 6; T5)
Proof: run load | test a_minor_one_library_with_levels_is_refused ... ok

**C8** - A `MERGE` commit keeps the levels both values hold known and equal and clears the others. (UL-05, AC 7; T3)
Proof: run abilevels | test a_merge_commit_keeps_the_levels_both_hold ... ok

**C9** - With `x: u9` unknown, `((x & 1) & 2).known_or(9)` is 0. (UL-01, AC 1; T17)
Proof: run sourcelevels | test the_s6_probe_gives_zero ... ok

**C10** - `&` makes a level known false where either operand's level is known false, known where both are known, unknown otherwise. (UL-01, AC 2; T17)
Proof: run sourcelevels | test and_keeps_each_level_a_known_false_decides ... ok

**C11** - `|` makes a level known true where either operand's level is known true, known where both are known, unknown otherwise. (UL-01, AC 3; T17)
Proof: run sourcelevels | test or_keeps_each_level_a_known_true_decides ... ok

**C12** - `^` and `!` give a level known exactly where every operand's level is known. (UL-01, AC 4; T17)
Proof: run sourcelevels | test xor_and_not_are_known_where_every_operand_is ... ok

**C13** - A shift by a known amount moves each level with its flag and fills known false, or the sign level for a signed `>>`. (UL-01, AC 5; T17)
Proof: run sourcelevels | test a_known_shift_moves_each_level_with_its_flag ... ok

**C14** - An unknown shift amount, arithmetic, a comparison or `as` meeting an unknown level gives every level unknown. (UL-01, AC 6; T17)
Proof: run sourcelevels | test arithmetic_comparison_as_and_unknown_shifts_are_all_or_nothing ... ok

**C15** - `merge(0b1010u4, 0b1011u4)` has levels 3, 2, 1 known 1, 0, 1 and level 0 unknown. (UL-02, AC 1; T18)
Proof: run sourcelevels | test merge_keeps_the_levels_both_values_agree_on ... ok

**C16** - An unknown `if` committing `0b1010u4` and `0b1011u4` commits the value of C15. (UL-02, AC 2; T18)
Proof: run sourcelevels | test an_unknown_if_commits_the_level_merge ... ok

**C17** - A multiplexer with `sel` unknown over `0b1100_0101` and `0b1100_1001` keeps levels 7, 6, 5, 4, 1, 0 known as 1, 1, 0, 0, 0, 1 and levels 3, 2 unknown. (UL-02, AC 3; T18)
Proof: run sourcelevels | test a_multiplexer_with_an_unknown_select_keeps_six_levels ... ok

**C18** - `level(k)` gives `Some(b)` for each known level and `None` for each unknown one. (UL-02, AC 4; T18)
Proof: run sourcelevels | test level_reads_each_level_of_a_partly_known_value ... ok

**C19** - `known()`, `known_or(d)` and `is_known()` treat a partly known value as unknown. (UL-02, AC 5; T18)
Proof: run sourcelevels | test known_and_known_or_treat_a_partly_known_value_as_unknown ... ok

**C20** - `same` is known `true` exactly when every level has the same flag and, where known, the same value. (UL-02, AC 6; T18)
Proof: run sourcelevels | test same_compares_levels_and_flags ... ok

**C21** - `logic::<u4>("10xx")` has levels 3, 2 known 1, 0 and levels 1, 0 unknown. (UL-03, AC 1; T19)
Proof: run sourcelevels | test logic_with_x_leaves_those_levels_unknown ... ok

**C22** - `matches_bits(s, "1x0x")` is known `true` on the `s` of UL-03 AC 2. (UL-03, AC 2; T19)
Proof: run sourcelevels | test matches_bits_x_matches_an_unknown_level ... ok

**C23** - `matches_bits(s, "1100")` is known `false` on the same `s`. (UL-03, AC 3; T19)
Proof: run sourcelevels | test matches_bits_zero_or_one_does_not_match_an_unknown_level ... ok

**C24** - `signal bus: Bit<u4> = logic("10xx");` starts with levels 3, 2 known 1, 0 and levels 1, 0 unknown. (UL-03, AC 4; T19)
Proof: run sourcelevels | test a_carrier_starts_at_a_partly_known_logic_value ... ok

**C25** - `cast::<u8, i8>(x)` keeps level `k` of `x` with its flag, for every `k`. (UL-04, AC 1; T19)
Proof: run sourcelevels | test cast_keeps_each_level_and_its_flag_per_level ... ok

**C26** - A `match` on a four-variant enum with level 1 known false and level 0 unknown takes the arms of tags 0 and 1 only, each record `maybe = true`. (UL-04, AC 2; T33)
Proof: run sourcelevels | test a_match_on_a_partly_known_tag_takes_only_admitted_arms ... ok

**C27** - `cast::<u2, E>` of a partly known tag keeps the levels when every admitted tag names a variant, and gives every level unknown when one does not. (UL-04, AC 3; T32)
Proof: run sourcelevels | test cast_to_an_enum_merges_or_is_unknown_by_the_admitted_tags ... ok

**C28** - A table cell, a CSV cell and a record field with some levels unknown print `0b` and one character per level, `x` where unknown. (UL-06, AC 1; T24)
Proof: run clirun | test a_partly_known_value_prints_as_a_level_pattern ... ok

**C29** - JSON Lines writes that pattern as a quoted string. (UL-06, AC 2; T24)
Proof: run clirun | test json_lines_quote_a_partly_known_value ... ok

**C30** - A fully known value decodes and prints as before this feature. (UL-06, AC 3; T22)
Proof: run apilevels | test a_fully_known_value_decodes_as_before ... ok

**C31** - An all-unknown value decodes as `Cell::Unknown`. (UL-06, AC 4; T22)
Proof: run apilevels | test an_all_unknown_value_decodes_as_unknown ... ok

**C32** - A plot draws a partly known value as it draws an unknown one. (UL-06, AC 5; T24)
Proof: run clisvg | render::svg::tests::a_plot_draws_a_partly_known_value_as_unknown ... ok

**C33** - A commit that changes only one level's flag fires `changed()`. (UL-07, AC 1; T20)
Proof: run sourcelevels | test a_change_of_one_levels_flag_is_a_change ... ok

**C34** - A commit that leaves every level and flag as it was does not fire `changed()`. (UL-07, AC 2; T20)
Proof: run sourcelevels | test an_equal_commit_of_a_partly_known_value_is_no_change ... ok

**C35** - A carrier of a struct, tuple, array or `Option` gives each leaf of more than one level a `KNOWN_LEVELS` mask. (UL-09, AC 1; T30)
Proof: run plancomp | test composite_leaves_carry_level_masks ... ok

**C36** - `merge` of two structs differing only in level 0 of one field keeps every other level known. (UL-09, AC 2; T31)
Proof: run sourcelevels | test merge_of_structs_keeps_every_other_level ... ok

**C37** - Field access and a known index give the leaf's levels with their flags. (UL-09, AC 3; T31)
Proof: run sourcelevels | test field_access_and_known_index_move_levels ... ok

**C38** - An unknown index gives the level-wise merge of the elements its known levels admit. (UL-09, AC 4; T31)
Proof: run sourcelevels | test an_unknown_index_merges_the_admitted_elements_from_source ... ok

**C39** - `cast` of a composite keeps level `k` with its flag. (UL-09, AC 5; T31)
Proof: run sourcelevels | test cast_of_a_composite_keeps_each_level ... ok

**C40** - A `const` folding `logic::<u4>("10xx")` holds levels 3, 2 known 1, 0 and levels 1, 0 unknown. (UL-10, AC 1; T15)
Proof: run elabunknowns | test logic_with_x_folds_to_partly_known_levels ... ok

**C41** - Constant `&`, `|`, `^`, `!`, known shifts, `merge`, `cast` and an unknown `if` fold per level. (UL-10, AC 2; T15)
Proof: run elabunknowns | test constant_operators_fold_per_level ... ok

**C42** - One table over partly known `u8` values gives equal levels and flags folded and compiled. (UL-10, AC 3; T21)
Proof: run sourcelevels | test folded_and_compiled_levels_agree_row_by_row ... ok

**C43** - A param that would fold partly known is refused as an unknown param is. (UL-10, AC 4; T16)
Proof: run elabunknowns | test a_partly_known_param_is_refused_as_unknown ... ok

**C44** - A `bool` carrier keeps the plain known byte and its pin bytes. (Edge case 1; T12)
Proof: run planshape | test bool_carriers_keep_the_plain_known_byte ... ok

**C45** - A fully known 64-level value has every mask bit set and gives the results it gave before. (Edge case 2; T8)
Proof: run rt | rt::flag::tests::a_known_64_level_value_has_a_full_mask ... ok

**C46** - `analog` reading a partly known value through `known_or(d)` gets `d`. (Edge case 4; T17)
Proof: run sourcelevels | test analog_reads_a_partly_known_value_as_its_default ... ok

**C47** - A signed `>>` of an `i8` with level 7 unknown makes every vacated level unknown. (Edge case 5; T17)
Proof: run sourcelevels | test a_signed_right_shift_spreads_an_unknown_sign ... ok

**C48** - A shift by the width or more gives every level known false, or the sign level with its flag for a signed `>>`. (Edge case 6; T17)
Proof: run sourcelevels | test a_shift_by_the_width_or_more_fills_by_the_rule ... ok

**C49** - `GAPS.md` holds neither S6 nor G6 once the feature closes. (Goals; T34)
Proof: run docs | test gaps_hold_neither_s6_nor_g6 ... ok

**C50** - A host value written into a levels field sets every level known (`u8`: `[v, 255]`), a `bool` stays `[v, 1]`. (UL-05, AC 1; T7)
Proof: run solverbind | test a_host_value_fills_every_level_of_a_levels_field ... ok

**C51** - `Expr::Levels` and `Constant::Levels` are built, checked and printed by the IR. (UL-03, AC 1; T13)
Proof: run irdigital | test partly_known_constants_are_built_checked_and_printed ... ok

**C52** - The interpreter refuses both partly known forms by name. (UL-10, AC 1; T13)
Proof: run interp | test partly_known_constants_are_refused_by_name ... ok

**C53** - A partly known constant compiles and crosses the ABI with its mask. (UL-03, AC 1; T14)
Proof: run tlevels | test a_partly_known_constant_compiles_and_crosses_the_abi ... ok

**C54** - The Book's multiplexer listing prints its known levels as a `0b` pattern, through the root package. (UL-06, AC 1; T25)
Proof: run bookvalues | test a_multiplexer_with_an_unknown_select_prints_its_known_levels ... ok

**C55** - The Book's inventory holds the new listing's row. (UL-06, AC 1; T25)
Proof: run bookinventory | test result: ok

**C56** - The Book's CLI listings match the binary, the new one included. (UL-06, AC 1; T25)
Proof: run clibook | test result: ok

Edge case 3 (`extern fn`) has no check: an `extern fn` call is refused by name until `foreign-models` (L12, L33); that feature carries its all-or-nothing rule.

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none yet. Planning (2026-11-21): the user is away and asked for the most reasonable default on every open point; the defaults are design.md D1 to D20. The spec of 2026-11-17 was amended, not rewritten.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 12

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4
Phase 2:  T5 ------→ T6 ------→ T7
Phase 3:  T8 ------→ T9 ------→ T10 -----→ T11
Phase 4:  T12 -----→ T13 -----→ T14
Phase 5:  T15 -----→ T16
Phase 6:  T17 -----→ T18 -----→ T19 -----→ T20 -----→ T21
Phase 7:  T22 -----→ T23 -----→ T24
Phase 8:  T25 -----→ T26
Phase 9:  T27
Phase 10: T28 -----→ T29 -----→ T30 -----→ T31
Phase 11: T32 -----→ T33
Phase 12: T34
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Starts | Notes |
| ----- | ------ | ----- | ------ | ----- |
| B1 | 1, 2 | T1 to T7 | now, from `main` at `856d2668` | the ABI, the loader, the digital solver, bind |
| B2 | 3, 4 | T8 to T14 | after B1 | T8 moves the listed buck assertions (known bytes only) |
| B3 | 5, 6 | T15 to T21 | after B2 | closes UL-01, UL-02, UL-03, UL-04 AC 1, UL-07, UL-10 |
| B4 | 7, 8 | T22 to T26 | after B3 | closes UL-06; S6 narrowed to aggregates |
| B5 | 9, 10 | T27 to T31 | waits: `composite-values` A–B on `main` | rebase, then UL-09 |
| B6 | 11, 12 | T32 to T34 | after B5 | UL-04 AC 2, 3; closes S6 and G6; then the Verifier |

# Composite Values Tasks (phase C: arrays of carriers)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/composite-values/design-c.md`
**Status**: Draft (executing on the launching agent's instruction)
**Approved**: pending - the launching agent relayed an instruction to implement T64-T90 with QC1-QC6 and DC1 at their defaults (2026-11-23); that relay is not the user's own approval, so no `user:` quote is written here
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run abi**: `cargo test -p piperine-abi --test array_pins`
**Run dsarr**: `cargo test -p piperine-digital-solver --test array_nets`
**Run bind**: `cargo test -p piperine-solver --test array_windows`
**Run rt**: `cargo test -p piperine-transpile --lib rt::array`
**Run irarr**: `cargo test -p piperine-ir --test carrier_arrays`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run planarr**: `cargo test -p piperine-plan --test carrier_arrays`
**Run tarr**: `cargo test -p piperine-transpile --test carrier_arrays`
**Run typesarr**: `cargo test -p piperine-lang --test types_carrier_arrays`
**Run elabarr**: `cargo test -p piperine-lang --test elab_carrier_arrays`
**Run srcarr**: `cargo test -p piperine-parity --test source_carrier_arrays`
**Run src**: `cargo test -p piperine-parity --test source_explicit`
**Run storage**: `cargo test -p piperine-api --test carrier_array_storage`
**Run apiarr**: `cargo test -p piperine-api --test carrier_arrays`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run book**: `cargo test -p piperine --test book_natures --test book_modules --test book_inventory`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** Q3 (a) (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"): sparse arrays of carriers. design-c.md QC1 to QC6 are the planner's defaults while the user is away; DC1 to DC6 are decisions that follow the spec as it reads today.

Rules every task follows:

- **Base.** Phase C starts on `main` with `generics` phases A and B merged (spec "Dependencies"). T64 checks it first; if `generics` A–B are not on `main`, the batch stops and reports.
- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-d CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-d` before any cargo command (never `/var/tmp/piperine-target`, never a target directory another worktree builds in). Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gates (user policy, 2026-11-21).** Per task: the `**Build**` line, then `cargo test -p <crate>` for every crate the task touched (its `Done when` names them). Once at the end of each batch: `cargo test --workspace`, the parity suite (`env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins`) and the buck targets above.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves.
- **Only design-c.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority (DC1: §5.1.8 and MD-78 over the older CV-24 wording). A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the spec's own value or the same expression evaluated by Rust in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (the user's rules, `.specs/features/code-clarity/spec.md`).** Readable at a glance; the module tree says where each thing is; each crate one purpose; **no free function, tests included** (every function has an owner type; a test's helpers are methods of a fixture type); one purpose per type; **no abbreviated name** (`element`, not `elem`; `index`, not `idx`; OO names); no god method and no needless abstraction; the data path evident. **Every new type, method, field and variant carries a `///` doc comment**; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches. New concerns go to the files design-c.md "Components" names, never into `print/value.rs` or `types/expr.rs` beyond a dispatch arm.
- **Conventions.** New IR `Expr`/`Stmt`/`EventSource` variants are appended last; new IR and ABI fields carry `#[serde(default)]` or are appended at the end of their `#[repr(C)]` struct. The interpreter refuses every new IR form by name (R1) in the same task that adds the form, and `crates/piperine-interp/tests/frozen.rs::each_composite_values_form_is_refused_by_name` grows with it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models live in `crates/piperine-parity/tests/carrier_arrays/*.msl`.
- **Cross-lane files**: `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `tests/book/inventory.tsv` changes only in the rows T89 names.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-38, MD-57, MD-62, MD-63, MD-78, MD-79; the phase A–B matrix of `tasks.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| ABI (`piperine-abi`) | integration | every new field and rule: valid shapes accepted, each invalid array pin refused by name | `crates/piperine-abi/tests/array_pins.rs` | `cargo test -p piperine-abi --test array_pins` |
| Digital engine (`piperine-digital-solver`) | integration | storage of written elements only, initial reads, `prev`/`changed` per element, windows, per-element order and transport, rewind | `crates/piperine-digital-solver/tests/array_nets.rs` | `cargo test -p piperine-digital-solver --test array_nets` |
| Binding (`piperine-solver`) | integration | a window binding an element and a sub-array; a count mismatch refused | `crates/piperine-solver/tests/array_windows.rs` | `cargo test -p piperine-solver --test array_windows` |
| Runtime (`piperine-transpile/src/rt`) | unit | all branches, the index bound at and past the end | inline `#[cfg(test)] mod tests` of `rt/array.rs` | `cargo test -p piperine-transpile --lib rt::array` |
| IR (`piperine-ir`) | integration | each new form built, typed, checked (valid and refused), printed, serialized | `crates/piperine-ir/tests/carrier_arrays.rs` | `cargo test -p piperine-ir --test carrier_arrays` |
| Interpreter refusals | integration | each new form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | the array pin's count, init form, element cells and statements; node arrays; windows | `crates/piperine-plan/tests/carrier_arrays.rs` | `cargo test -p piperine-plan --test carrier_arrays` |
| Printer (`piperine-transpile/src/print`) | integration | each form compiled by `rustc` from hand-built IR and run | `crates/piperine-transpile/tests/carrier_arrays.rs` | `cargo test -p piperine-transpile --test carrier_arrays` |
| Checker (`piperine-lang/src/types`) | integration | every checker-decided AC with its code, span and name | `crates/piperine-lang/tests/types_carrier_arrays.rs` | `cargo test -p piperine-lang --test types_carrier_arrays` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC and code; node arrays; structural params | `crates/piperine-lang/tests/elab_carrier_arrays.rs` | `cargo test -p piperine-lang --test elab_carrier_arrays` |
| Source-level runs | integration | one test per run-time AC on the Rust backend | `crates/piperine-parity/tests/source_carrier_arrays.rs`, models in `tests/carrier_arrays/` | `cargo test -p piperine-parity --test source_carrier_arrays` |
| Storage measurement | integration | CV-27 AC 1 with a positive control | `crates/piperine-api/tests/carrier_array_storage.rs` | `cargo test -p piperine-api --test carrier_array_storage` |
| Host library | integration | element and whole-array cells of discrete and node arrays | `crates/piperine-api/tests/carrier_arrays.rs` | `cargo test -p piperine-api --test carrier_arrays` |
| Command line | integration | each output format with an array of carriers | `crates/piperine-cli/tests/run.rs` | `cargo test -p piperine-cli --test run` |
| Book, spec text, repository records | integration | each narrowed Book row; each removed `GAPS.md` row; each lifted fence | `tests/book/{natures,modules}.rs`, `crates/piperine-cli/tests/docs.rs`, `crates/piperine-lang/tests/std_galleries.rs` | the `book`, `docs`, `galleries` Run lines |
| Contract re-check (T64) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase and the user's gate policy (2026-11-21) - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Task | After every task | the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, which includes the `Run` line(s) its `Done when` names |
| Batch | Once, after the last task of each batch (C1 to C4) | `cargo test --workspace`; `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`; `cargo test -p piperine-cli --test twins`; the buck targets |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Phase numbers continue `tasks.md` (phases 1 to 15).

### Phase 16: Contract

```
T64
```

### Phase 17: ABI, engine, binding and runtime

```
T65 → T66 → T67 → T68 → T69
```

### Phase 18: IR

```
T70 → T71
```

### Phase 19: Plan and printer

```
T72 → T73
```

### Phase 20: The checker

```
T74 → T75 → T76 → T77
```

### Phase 21: The elaborator

```
T78 → T79 → T80 → T81 → T82
```

### Phase 22: End to end

```
T83 → T84 → T85
```

### Phase 23: Host, Book and closure

```
T86 → T87 → T88 → T89 → T90
```

---

## Task Breakdown

### T64: Rebase and re-read the anchors

**What**: Create the phase C branch from `main` once `generics` phases A and B are merged there (stop and report if not); confirm the `**Build**` line is green with zero warnings; re-read every anchor design-c.md names (`Recv::CarrierArray` and `E0211` in `pom/resolve/members.rs`, `array_type`/`Length`, `drive_target`, generics' `HandleBinding` and `Length`, `Place`/`Projection`, `CompositeLayout`/`PinLayout`, `Unrolling`, `Statement { offset, len }`, `Store`/`Journal`, `core/bind.rs` `Port`, `ElementsBench`) and write on the Handoff what moved or differs from the design.
**Where**: `.specs/features/composite-values/tasks-c.md` (Handoff only)
**Depends on**: None
**Reuses**: phase B's T1 procedure
**Requirement**: CV-35
**Executor**: Sonnet

**Done when**:

- [x] `generics` A–B are ancestors of the branch head; the head is recorded on the Handoff
- [x] Every anchor of design-c.md is found or its difference is on the Handoff
- [x] Gate check passes: Task (Build line only)

**Tests**: none
**Gate**: task

**Commit**: `docs(composite-values): phase C contract re-read`

---

### T65: ABI: the array pin

**What**: Append `Pin.count: u64` (0 = one carrier), `Cell.element: u64` with `WHOLE = u64::MAX`, the `#[repr(C)]` `ArrayView` read view and `Host.commit_element` to `raw.rs`; `PinSpec.count` in `shape.rs`; `validate.rs` refuses an array pin that is not `DISCRETE`, an `init` whose length is neither one element nor `count` elements, and a cell element at or past `count`; `abi.md` §3, §3.1 (P4: equal `count`), §5 (reads of an array pin) and §6 (C2/C3 per element) amended in place, `MINOR` stays 0 (QC1, MD-78 item 4). Create `crates/piperine-abi/tests/array_pins.rs`.
**Where**: `crates/piperine-abi/src/raw.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `validate.rs` rule style, the `Host.size` append gate
**Requirement**: CV-27, CV-25
**Executor**: Sonnet

**Done when**:

- [x] `array_pins_validate_their_count_and_init` passes under `**Run abi**`
- [x] Every existing `piperine-abi` target passes (`count` 0 keeps every shape as it was)
- [x] Gate check passes: Task (`piperine-abi`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(abi): array pins, element cells and element commits`

---

### T66: Engine: the sparse array net

**What**: `array.rs` in `piperine-digital-solver`: `SparseArray` (one array net: count, element size, `ArrayInit::{Repeat, Each}`, a `BTreeMap<u64, ElementState>` of written elements), `ElementState` (value, previous, change stamp), `ElementUndo` journaled for `mark`/`rewind`; an element never written reads its initial bytes and its previous is the same (§5.1.8), and is changed only in the microstep of the `(0, 0)` initial commit (§13 rule 6); `NetSpec.count`/`array_init` route a net to it. Create `crates/piperine-digital-solver/tests/array_nets.rs` (its helpers are methods of a fixture type).
**Where**: `crates/piperine-digital-solver/src/array.rs`
**Depends on**: T65
**Reuses**: `Journal`, `Store`'s entry points
**Requirement**: CV-27, CV-24
**Executor**: Sonnet

**Done when**:

- [x] `a_sparse_array_stores_only_written_elements_and_rewinds` passes under `**Run dsarr**` (1 000 distinct writes to a `1 << 32` array hold 1 000 entries; a rewind restores the map)
- [x] `an_unwritten_element_reads_its_initial_value_and_previous` passes under `**Run dsarr**`
- [x] Gate check passes: Task (`piperine-digital-solver`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(digital-solver): sparse array nets`

---

### T67: Engine: element readers, windows and element commits

**What**: In the run loop: a cell naming one element is dirtied when that element changes, a `WHOLE` cell when any does; a reaction reads a `WHOLE` cell through an `ArrayView` over the microstep's snapshot (R2); `commit_element` queues a commit for one element with C2 (last write per overlapping byte, per element) and C3 (transport per statement, element and range); `NetWindow { net, first, length }` lets a scalar pin read and write one element and an array pin a run of elements.
**Where**: `crates/piperine-digital-solver/src/run.rs`
**Depends on**: T66
**Reuses**: `queue.rs` overlap order, `write_net`
**Requirement**: CV-24, CV-25, CV-23
**Executor**: Sonnet

**Done when**:

- [ ] `an_element_window_reads_and_writes_one_element` and `element_commits_keep_the_last_write_per_level_and_transport_per_element` pass under `**Run dsarr**`
- [ ] Gate check passes: Task (`piperine-digital-solver`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(digital-solver): element readers, windows and element commits`

---

### T68: Binding: windows of an array net

**What**: `Port::Window { net, first, length }` in `core/bind.rs` and `CircuitBuilder::array_net(name, count, init)`: a scalar pin bound to one element, an array pin bound to an array net of equal `count` (P4) or to a sub-array window; a pin of another count is a bind error naming both counts. Create `crates/piperine-solver/tests/array_windows.rs`.
**Where**: `crates/piperine-solver/src/core/bind.rs`
**Depends on**: T67
**Reuses**: `Port`, `CircuitBuilder::device`
**Requirement**: CV-23
**Executor**: Sonnet

**Done when**:

- [ ] `a_window_binds_a_pin_to_elements_of_an_array_net` passes under `**Run bind**`
- [ ] Gate check passes: Task (`piperine-solver`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(solver): bind a pin to a window of an array net`

---

### T69: Runtime: `rt::CarrierArray` and element commits

**What**: `rt/array.rs`: `CarrierArray<T: Carried>` over an `ArrayView` (`get`, `previous`, `changed`, `whole` returning `Box<[T; K]>`), an index at or past `count` raising `rt::Panic` naming the carrier and the index; `rt::Commits::commit_element(statement, index, value, after)` (same check, then `Host.commit_element`) and `commit_range(statement, offset, value, after)` for a field or value-array projection.
**Where**: `crates/piperine-transpile/src/rt/array.rs`
**Depends on**: T68
**Reuses**: `rt::pin::Field`, `rt::Panic`, `rt::Commits`
**Requirement**: CV-25, CV-24
**Executor**: Sonnet

**Done when**:

- [ ] `carrier_array_index_past_the_end_panics_naming_it` and `element_commits_reach_the_host_with_their_element` pass under `**Run rt**`
- [ ] Gate check passes: Task (`piperine-transpile`)

**Tests**: unit
**Gate**: task

**Commit**: `feat(transpile): rt carrier arrays and element commits`

---

### T70: IR: arrays of carriers and nodes, element reads and events

**What**: `Carrier.count: Option<u64>`, `Body.node_arrays: Vec<NodeArray>`, `Expr::{CarrierElement, ElementPrevious, ElementChanged}`, `EventSource::ElementChanged { carrier, element }` (all serde-default or appended); `Ir::check` in `check/carrier_array.rs` (an element form only on an array carrier, an integer index, a constant element below `count`, a whole read typed `[T; K]`); typing, printing, serde, `deps.rs` (an element read is a read of its carrier); the interpreter refuses each form by name. Create `crates/piperine-ir/tests/carrier_arrays.rs`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Expr::Carrier`, `Expr::Previous`, `EventSource::Changed`, `CompositeForms`
**Requirement**: CV-23, CV-24, CV-34
**Executor**: Sonnet

**Done when**:

- [ ] `element_forms_are_checked_against_the_array` passes under `**Run irarr**`
- [ ] `each_composite_values_form_is_refused_by_name` passes under `**Run interp**` with the three expressions and the event source added
- [ ] Gate check passes: Task (`piperine-ir`, `piperine-interp`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(ir): arrays of carriers and nodes, element reads and events`

---

### T71: IR: element and field commits, element drives, windows to children

**What**: `Stmt::Commit.target: CommitTarget { element, path }` (serde default: the whole carrier), `Drive.element: Option<u64>` and `Drive.path`, `CarrierWindow` in a child's carrier slots; `Ir::check`: the committed value has the type of the levels the target names (§12.1.9), a constant element below `count`, a window inside the array; `statements.rs` counts an element commit inside a loop as one statement (§12.1.14); the interpreter refuses each by name.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T70
**Reuses**: `Place`, `Projection`, `Statements::of`
**Requirement**: CV-25, CV-23, CV-34
**Executor**: Sonnet

**Done when**:

- [ ] `commit_targets_and_windows_are_checked` and `an_element_commit_in_a_loop_is_one_statement` pass under `**Run irarr**`
- [ ] `each_composite_values_form_is_refused_by_name` passes under `**Run interp**`
- [ ] Gate check passes: Task (`piperine-ir`, `piperine-interp`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(ir): element and field commits, element drives, windows`

---

### T72: Plan: the array pin, element cells and statements, windows

**What**: `array.rs` in `piperine-plan` (`ArrayPin`): an array carrier is one `DISCRETE` pin with `count`, the element's `PinLayout`, `init` as one element (`[x; K]`) or every element (`[a, b, c]`); reads of a constant element and `WHOLE` reads as cells; an element commit's statement over the element's levels and a field commit's over its byte range (`CompositeLayout` offsets); a node array is `count` continuous pins named `bl[k]`; `netlist.rs` passes `Port::Window` for a `CarrierWindow`. Create `crates/piperine-plan/tests/carrier_arrays.rs`.
**Where**: `crates/piperine-plan/src/array.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `PinLayout`, `CompositeLayout`, `RecordLayout` patterns
**Requirement**: CV-23, CV-25, CV-27
**Executor**: Sonnet

**Done when**:

- [ ] `an_array_of_carriers_is_one_array_pin` and `a_field_commit_is_a_statement_over_its_levels` pass under `**Run planarr**`
- [ ] Gate check passes: Task (`piperine-plan`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(plan): array pins, element cells and statements, windows`

---

### T73: Printer: element reads, history and commits

**What**: `print/carrier_array.rs` (`impl ValuePrinter`): `CarrierElement`, a whole read, `ElementPrevious`, `ElementChanged`, an element commit (`commits_.commit_element`), a field or value-array commit (`commits_.commit_range`) and an element event's guard read; generated items name the carrier's MSL path (R3); `print/value.rs` gains dispatch arms only. Create `crates/piperine-transpile/tests/carrier_arrays.rs` (hand-built IR, compiled by `rustc`, run on the engine).
**Where**: `crates/piperine-transpile/src/print/carrier_array.rs`
**Depends on**: T72
**Reuses**: `rt::CarrierArray`, `rt::Commits`, `ValuePrinter`
**Requirement**: CV-24, CV-25
**Executor**: Sonnet

**Done when**:

- [ ] `element_reads_and_commits_compile_and_run` passes under `**Run tarr**`
- [ ] The transpile goldens are byte-identical (`cargo test -p piperine-transpile --test golden`)
- [ ] Gate check passes: Task (`piperine-transpile`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(transpile): print element reads, history and commits`

---

### T74: Checker: arrays of carriers as signal and port types

**What**: `types/carrier_array.rs` (`impl Checker`): `Ty::CarrierArray { element, length: Length }` (generics' `Length`; `Length::Generic` stays `P0001` until `generics` GN-13) for a `signal` or a port of a discrete or a continuous nature, nested arrays included; the initial value is an array of the value type (`E0210`); an array of carriers as a function parameter or a `let` type stays `P0001` "an array of carriers as a function parameter" (QC3); `array_type` no longer refuses an array of `Bit`. Create `crates/piperine-lang/tests/types_carrier_arrays.rs`; `types_aggregates.rs::phase_c_and_d_forms_stay_p0001_by_name` keeps its phase D cases only (design-c.md "may move").
**Where**: `crates/piperine-lang/src/types/carrier_array.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `array_type`, `fold_length`, `Ty::Carrier`
**Requirement**: CV-23, CV-34
**Executor**: Sonnet

**Done when**:

- [ ] `arrays_of_carriers_type_as_signals_and_ports`, `an_array_initial_value_of_another_type_is_e0210` and `an_array_of_carriers_as_a_function_parameter_is_refused_by_name` pass under `**Run typesarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(types): arrays of carriers as signal and port types`

---

### T75: Checker: element reads, history, events and probes

**What**: `types/element.rs` (`impl Checker`): `x[i]` of an array of discrete carriers reads the element's value type, `x` whole reads `[T; K]`; `x[k].changed()`, `.prev()`, `.rise()`, `.fall()` and `v(bl[k])` on one element; a carrier method on the whole array reaches the user as `E0211` (§5.1.7), never masked by `P0001`; an element passed to a nature-typed parameter binds a handle (generics B, GN-11).
**Where**: `crates/piperine-lang/src/types/element.rs`
**Depends on**: T74
**Reuses**: `Recv::CarrierArray`, `types/carrier.rs` history reads, generics' handle checks
**Requirement**: CV-24
**Executor**: Sonnet

**Done when**:

- [ ] `element_reads_history_and_probes_type_per_element` and `a_carrier_method_on_a_whole_array_is_e0211` pass under `**Run typesarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(types): element reads, history, events and probes`

---

### T76: Checker: element and field targets of commits and definitions

**What**: `drive_target` takes `x[i]` (a commit's index any integer; a definition's index folds, else `P0002` naming it, QC4), a field `st.n` and a value-array element `word[2]` of one carrier, and their combinations (`mem[a].f`); the committed value has the type of the named levels (§12.1.9); the `defined`/`committed` books are keyed by `(carrier, element)`, so a definition and commits of one element are `E0221` and of two elements are accepted (§5.2.4); the `P0001` "an element or field commit/definition" leaves.
**Where**: `crates/piperine-lang/src/types/definition.rs`
**Depends on**: T75
**Reuses**: `drive_target`, `types/reaction.rs` `commit`
**Requirement**: CV-25
**Executor**: Sonnet

**Done when**:

- [ ] `element_and_field_targets_type_their_levels` and `a_definition_and_commits_of_one_element_are_e0221` pass under `**Run typesarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(types): element and field targets of commits and definitions`

---

### T77: Checker: connecting arrays and elements

**What**: `types/connection.rs` (`impl Checker`): an array of carriers to an array port of the same shape; an element `x[k]` (folding `k`) or a sub-array `x[r]` to a scalar or array port; another length or shape is `E0313` naming both (§8.1.10); an element to an `out` port is the child's drive of that element (§5.2.6), and `E0223` when the array signal has an initial value (DC4); a constant index past the end is `E0104` naming it.
**Where**: `crates/piperine-lang/src/types/connection.rs`
**Depends on**: T76
**Reuses**: `types/module.rs` placement checks
**Requirement**: CV-23
**Executor**: Sonnet

**Done when**:

- [ ] `arrays_and_elements_connect_to_ports_of_their_shape`, `an_array_argument_of_another_length_is_e0313_naming_both_lengths` and `an_initialized_array_with_an_element_on_an_out_port_is_e0223` pass under `**Run typesarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(types): connect arrays and elements of carriers`

---

### T78: Elaborator: arrays of carriers and of nodes declared

**What**: `elab/carrier_arrays.rs` (`CarrierArrays`): an array of discrete carriers is one IR carrier with `count` and its folded initial value as one element or every element (nested arrays row-major, DC5); an array of nodes is `count` `BodyNode`s `bl[0]` … plus a `NodeArray`; a length that reads a param marks that param structural (§8.1.2, QC2); the POM keeps the one declaration as written. Create `crates/piperine-lang/tests/elab_carrier_arrays.rs`.
**Where**: `crates/piperine-lang/src/elab/carrier_arrays.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `elab/carriers.rs` `Structure`, `ConstEval`
**Requirement**: CV-23, CV-36
**Executor**: Sonnet

**Done when**:

- [ ] `an_array_of_nodes_is_one_node_per_element`, `a_nested_array_of_carriers_is_row_major` and `an_array_length_read_from_a_param_makes_it_structural` pass under `**Run elabarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(elab): arrays of carriers and of nodes`

---

### T79: Elaborator: element reads, history and events

**What**: `elab/elements.rs` (`impl Elaboration`): element and whole reads, `prev()`, `changed()` as a `bool`, an event head on one element as `EventSource::ElementChanged` with its edge guard (the index folds, else `P0002` naming it, QC4), a discrete element read in `analog` (§11 rule 13), and an element bound to a handle parameter (`HandleBinding::Element`, extending generics B's `HandleBinding`).
**Where**: `crates/piperine-lang/src/elab/elements.rs`
**Depends on**: T78
**Reuses**: `elab/event_reads.rs`, `elab/events.rs`, generics `elab/handles.rs`
**Requirement**: CV-24
**Executor**: Sonnet

**Done when**:

- [ ] `element_reads_and_events_lower_per_element` and `an_element_binds_a_handle_parameter` pass under `**Run elabarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(elab): element reads, history and events`

---

### T80: Elaborator: node-array elements in `analog`

**What**: In `elab/unroll.rs`: a contribution or probe on `bl[k]` inside a loop of constant trip count folds `k` per iteration to the element's node (`Terminal::Node(first + k)`); an index that does not fold after unrolling is `P0002` naming it (CV-26 AC 2, QC4); a constant index past the end is `E0104`.
**Where**: `crates/piperine-lang/src/elab/unroll.rs`
**Depends on**: T79
**Reuses**: `Unrolling`, `elab/contribute.rs`
**Requirement**: CV-26
**Executor**: Sonnet

**Done when**:

- [ ] `node_array_contributions_unroll_to_their_nodes` and `an_analog_node_index_that_does_not_fold_is_refused_naming_it` pass under `**Run elabarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(elab): node-array elements in analog`

---

### T81: Elaborator: element and field commits, element drives

**What**: `elab/element_commits.rs` (`impl Elaboration`): a commit's `CommitTarget` (element index expression, projections) and an element or field definition's `Drive.element`/`Drive.path`; `elab/reactions.rs` dispatches to it.
**Where**: `crates/piperine-lang/src/elab/element_commits.rs`
**Depends on**: T80
**Reuses**: `elab/place.rs` projections, `elab/drives.rs`
**Requirement**: CV-25
**Executor**: Sonnet

**Done when**:

- [ ] `element_and_field_commits_lower_to_their_targets` and `an_element_definition_is_one_drive` pass under `**Run elabarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(elab): element and field commits, element drives`

---

### T82: Elaborator: connections of arrays and elements

**What**: `elab/children.rs`: a whole array, an element or a sub-array given to a child's port becomes a `CarrierWindow`; a node array or element gives the child its nodes; `elab/place.rs`'s net table names one net per array (`Top.lines`) and one node per element (`Top.bl[2]`).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T81
**Reuses**: `Child` carrier slots, `elab/place.rs`
**Requirement**: CV-23
**Executor**: Sonnet

**Done when**:

- [ ] `arrays_and_elements_bind_windows_of_child_ports` passes under `**Run elabarr**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(elab): connect arrays and elements to child ports`

---

### T83: End to end: registers and memories over arrays of carriers

**What**: Create `crates/piperine-parity/tests/source_carrier_arrays.rs` and `tests/carrier_arrays/*.msl`: §8.2's `UartShift` (`[Bit; 8]`, `sh[7] <- rx; for k in 0..7 { sh[k] <- sh[k + 1]; }`, `byte = cast(sh).unwrap()`) driven by a known bit sequence, against its expected bytes; E.5's `Ram` (`[Bit<u32>; 256]`, `mem[addr] <- wdata`, read-before-write); per-element events and history; element and field commits; overlapping commits of one microstep; a run-time index past the end; an element on a child's `out` port. Expected values are computed by Rust in the test.
**Where**: `crates/piperine-parity/tests/source_carrier_arrays.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `piperine-parity` source runners (`source_aggregates.rs` pattern)
**Requirement**: CV-23, CV-24, CV-25
**Executor**: Sonnet

**Done when**:

- [ ] `each_element_of_an_array_of_carriers_has_its_own_events`, `arrays_and_elements_connect_element_to_element`, `an_array_of_carriers_reads_whole_as_an_array_value`, `element_history_and_events_apply_to_that_element_only`, `an_element_never_committed_reads_the_arrays_initial_value`, `an_element_commit_changes_that_element_only`, `a_field_or_value_element_commit_changes_those_levels_only`, `overlapping_commits_in_one_microstep_keep_the_last_write_per_level`, `a_run_time_element_index_out_of_range_names_carrier_index_instance_and_instant`, `an_element_given_to_an_out_port_is_driven_by_the_child` and `the_shift_register_and_the_ram_give_their_expected_sequences` pass under `**Run srcarr**`
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): registers and memories over arrays of carriers`

---

### T84: End to end: a ladder over an array of nodes

**What**: In `source_carrier_arrays.rs`: an R-ladder of four sections over `[Electrical; 4]` contributed in `for k in 0..N { .. }`, solved for its operating point and a transient, against the same ladder written out node by node (identical solution, byte for byte) and against its closed-form node voltages computed by Rust in the test (DC3, QC6); a probe `v(bl[k])` per element.
**Where**: `crates/piperine-parity/tests/source_carrier_arrays.rs`
**Depends on**: T83
**Reuses**: the parity source runner
**Requirement**: CV-26, CV-24
**Executor**: Sonnet

**Done when**:

- [ ] `a_ladder_over_an_array_of_nodes_solves_as_written_out` passes under `**Run srcarr**`
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): a ladder over an array of nodes`

---

### T85: Storage proportional to what is written

**What**: Create `crates/piperine-api/tests/carrier_array_storage.rs`: a `HeapMeter` type that is the target's `#[global_allocator]` and records the peak; `signal mem: [Bit<u8>; 1 << 32]` with 1 000 distinct element commits run in process through `Session`, against the same design with `1 << 16`: the peak during the run does not depend on the declared length (CV-27 AC 1), and a positive control shows the meter sees the 1 000 written elements.
**Where**: `crates/piperine-api/tests/carrier_array_storage.rs`
**Depends on**: T84
**Reuses**: `Session`, the `memory.rs` test's measurement pattern
**Requirement**: CV-27
**Executor**: Sonnet

**Done when**:

- [ ] `peak_heap_does_not_depend_on_the_declared_length` passes under `**Run storage**`
- [ ] Gate check passes: Task (`piperine-api`)

**Tests**: integration
**Gate**: task

**Commit**: `test(api): arrays of carriers cost what is written`

---

### T86: `explicit-unknowns` EU-04 AC 3 on a real array

**What**: The model `ElementsBench` of `source_explicit.rs` becomes `signal xs: [Bit<Option<u8>>; 1 << 20]` with a few elements committed; `an_element_never_committed_reads_its_initial_value` (same name, design-c.md "may move") asserts every never-committed element read reads the initial value (the storage half of EU-04 AC 3 is C126's measurement, T85, which the test's comment names); `.specs/features/explicit-unknowns/tasks.md` C13 loses its DEFERRED note and names this commit.
**Where**: `crates/piperine-parity/tests/source_explicit.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ElementsBench`
**Requirement**: CV-24, CV-27
**Executor**: Sonnet

**Done when**:

- [ ] `an_element_never_committed_reads_its_initial_value` passes under `**Run src**` on the real array
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): EU-04 AC 3 on an array of carriers`

---

### T87: Host: element and whole-array cells

**What**: `array_probe.rs` in `piperine-api` (`ArrayProbe`): `Top.lines[3]` reads one element of an array net (a scalar or composite cell), `Top.lines` every element as `Cell::Composite` of an array, `Top.bl[2]` one node, `Top.bl` every node; metadata lists an array net with its count. Create `crates/piperine-api/tests/carrier_arrays.rs`.
**Where**: `crates/piperine-api/src/array_probe.rs`
**Depends on**: T86
**Reuses**: `CellShape`, `CompositeCell`, `Probe::Shaped`
**Requirement**: CV-28
**Executor**: Sonnet

**Done when**:

- [ ] `array_probes_read_an_element_and_the_whole_array` passes under `**Run apiarr**`
- [ ] Gate check passes: Task (`piperine-api`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(api): element and whole-array probes`

---

### T88: CLI: arrays of carriers in every output format

**What**: `piperine run` prints `Top.lines[3]` alone and `Top.lines` whole as Q2 (c) says: literal syntax `[false, true, …]` in the text table and record lines, a JSON array in JSON Lines, one column per element (`lines[0]` …) in CSV and the plot; the same for a node array.
**Where**: `crates/piperine-cli/tests/run.rs`
**Depends on**: T87
**Reuses**: phase B's T62 rendering
**Requirement**: CV-28, CV-21
**Executor**: Sonnet

**Done when**:

- [ ] `an_array_of_carriers_prints_by_element_and_whole_in_every_format` passes under `**Run clirun**`
- [ ] Gate check passes: Task (`piperine-cli`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(cli): arrays of carriers in every output format`

---

### T89: Book rows and fences the arrays lift

**What**: natures#2's test becomes `signals_of_every_family_are_refused_for_the_logic_and_the_net` (`[Bit; 8]` runs; `Bit<Logic<4>>` and `Sda` stay refused; owner `resolved-nets` unchanged) and modules#10's becomes `the_memory_bank_is_refused_for_its_generic_module` (owner `generics` unchanged); `tests/book/inventory.tsv` names the new tests; `GAPS.md` §11 "Today" of natures#2, modules#10 and cheatsheet#2 no longer names the array of carriers; the fences E.5 (`Ram`), §8.2 (`UartShift`) and §22.1.3 (`signal sh: [Bit; 8]`) are held to the checker (CV-37 AC 2).
**Where**: `tests/book/natures.rs`
**Depends on**: T88
**Reuses**: `Chapter`, `Refusal`, `std_galleries.rs` rows
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `signals_of_every_family_are_refused_for_the_logic_and_the_net` and `the_memory_bank_is_refused_for_its_generic_module` pass under `**Run book**`, and `book_inventory` passes
- [ ] `the_fences_composite_values_lifted_are_checked` passes under `**Run galleries**` with the phase C fences
- [ ] Gate check passes: Task (`piperine`, `piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `test(book): the arrays of carriers run in natures#2 and modules#10`

---

### T90: Phase C bookkeeping

**What**: `GAPS.md` drops L24 (its "also owes EU-04 AC 3" included) and updates §9 Coverage (II §5, III §8, IV §11–§12), H9 (a param read by an array length is structural, so `E0803` is reachable), the counts line and §5 gains the QC3 spec-hole row (an array of carriers as a function parameter, owner `user`); `.specs/STATE.md` MD-79 gains the phase C item of design-c.md "Tech Decisions"; `ROADMAP.md`'s lane row says phase C delivered; `spec.md`'s traceability marks CV-23 to CV-28 done; `abi.md` §15 notes QC1 for the user.
**Where**: `GAPS.md`
**Depends on**: T89
**Reuses**: `docs.rs`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_composite_values_phase_c_closed` passes under `**Run docs**`
- [ ] Gate check passes: Task (`piperine-cli`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `docs: composite-values phase C closes L24`

---

## Checks

**C111** - `signal x: [Bit<u8>; K]` makes `K` carriers, each with its own events: a commit to one fires that element's reaction only. (CV-23, AC 1; T83)
Proof: run srcarr | test each_element_of_an_array_of_carriers_has_its_own_events ... ok

**C112** - `signal bl: [Electrical; 4]` is four nodes `bl[0]` to `bl[3]`, the POM keeping one declaration. (CV-23, AC 1; T78)
Proof: run elabarr | test an_array_of_nodes_is_one_node_per_element ... ok

**C113** - An array passed to an array port, and `x[k]` passed to a scalar port, connect element to element: a commit by the child is seen by the parent at that element. (CV-23, AC 2; T83)
Proof: run srcarr | test arrays_and_elements_connect_element_to_element ... ok

**C114** - An array argument whose length differs from its port's is `E0313` naming both lengths. (CV-23, AC 3; T77)
Proof: run typesarr | test an_array_argument_of_another_length_is_e0313_naming_both_lengths ... ok

**C115** - An array of discrete carriers read whole is the array of its elements' values. (CV-24, AC 1 as DC1 reads it; T83)
Proof: run srcarr | test an_array_of_carriers_reads_whole_as_an_array_value ... ok

**C116** - `lines[k].changed()`, `.prev()` and `.rise()` apply to element `k` only. (CV-24, AC 2; T83)
Proof: run srcarr | test element_history_and_events_apply_to_that_element_only ... ok

**C117** - A carrier method on a whole array of carriers is `E0211`. (CV-24, AC 3; T75)
Proof: run typesarr | test a_carrier_method_on_a_whole_array_is_e0211 ... ok

**C118** - An element never committed reads the array's initial value for that element, and its `prev()` is that value (§5.1.8). (CV-24, AC 4 as DC1 reads it; T83)
Proof: run srcarr | test an_element_never_committed_reads_the_arrays_initial_value ... ok

**C119** - `bits[3] <- true;` and `bits[i] <- x;` with `i` computed at run time change that element only and fire `changed()` on it only. (CV-25, AC 1; T83)
Proof: run srcarr | test an_element_commit_changes_that_element_only ... ok

**C120** - `st.n <- 3;` on a struct carrier and `word[2] <- true;` on `Bit<[bool; 8]>` change those levels only. (CV-25, AC 2; T83)
Proof: run srcarr | test a_field_or_value_element_commit_changes_those_levels_only ... ok

**C121** - A definition and commits of one element are `E0221`; a definition of one element and commits of another are accepted. (CV-25, AC 3; T76)
Proof: run typesarr | test a_definition_and_commits_of_one_element_are_e0221 ... ok

**C122** - Two commits of one microstep writing overlapping levels of one carrier keep the last write per level. (CV-25, AC 4; T83)
Proof: run srcarr | test overlapping_commits_in_one_microstep_keep_the_last_write_per_level ... ok

**C123** - A run-time element index out of range fails the analysis naming the carrier, the index, the instance and the instant. (CV-25, AC 5; T83)
Proof: run srcarr | test a_run_time_element_index_out_of_range_names_carrier_index_instance_and_instant ... ok

**C124** - A ladder contributed over `[Electrical; 4]` in a loop of constant trip count gives the same solution as the ladder written out, and its closed-form node voltages. (CV-26, AC 1; T84)
Proof: run srcarr | test a_ladder_over_an_array_of_nodes_solves_as_written_out ... ok

**C125** - An `analog` index of a node array that does not fold after unrolling is refused naming the index. (CV-26, AC 2; T80)
Proof: run elabarr | test an_analog_node_index_that_does_not_fold_is_refused_naming_it ... ok

**C126** - `[Bit<u8>; 1 << 32]` with 1 000 distinct commits runs with a peak heap that does not depend on the declared length, against `1 << 16`. (CV-27, AC 1; T85)
Proof: run storage | test peak_heap_does_not_depend_on_the_declared_length ... ok

**C127** - `Top.lines[3]` prints that element and `Top.lines` every element, in the table, record lines, CSV, JSON Lines and the plot as Q2 (c) says. (CV-28, AC 1; T88)
Proof: run clirun | test an_array_of_carriers_prints_by_element_and_whole_in_every_format ... ok

**C128** - The host library reads one element and the whole array of an array net and of a node array. (CV-28, AC 1; T87)
Proof: run apiarr | test array_probes_read_an_element_and_the_whole_array ... ok

**C129** - The interpreter refuses each new IR form of phase C by its name. (CV-34, AC 1; T70, T71)
Proof: run interp | test each_composite_values_form_is_refused_by_name ... ok

**C130** - An array of carriers as a function parameter is refused by name (`P0001`), the one form of the area the spec leaves open (QC3). (CV-34, AC 2; T74)
Proof: run typesarr | test an_array_of_carriers_as_a_function_parameter_is_refused_by_name ... ok

**C131** - An element of an array of carriers given to a child's `out` port is driven by that child only. (Edge case "element passed to an `out` port"; T83)
Proof: run srcarr | test an_element_given_to_an_out_port_is_driven_by_the_child ... ok

**C132** - An element of `[Bit<Option<u8>>; 1 << 20]` never committed reads its initial value; its storage half is C126. (explicit-unknowns EU-04, AC 3, owed by L24; T86)
Proof: run src | test an_element_never_committed_reads_its_initial_value ... ok

**C133** - An array pin validates its count and its initial value's length; `count` 0 keeps every existing shape valid. (CV-27, AC 1, the ABI half; T65)
Proof: run abi | test array_pins_validate_their_count_and_init ... ok

**C134** - A sparse array net stores only the elements written and a rewind restores them. (CV-27, AC 1, the engine half; T66)
Proof: run dsarr | test a_sparse_array_stores_only_written_elements_and_rewinds ... ok

**C135** - A window binds a scalar pin to one element and an array pin to a run of elements of an array net. (CV-23, AC 2, the binding half; T68)
Proof: run bind | test a_window_binds_a_pin_to_elements_of_an_array_net ... ok

**C136** - `rt::CarrierArray` panics on an index at or past the end, naming the carrier and the index. (CV-25, AC 5, the runtime half; T69)
Proof: run rt | test carrier_array_index_past_the_end_panics_naming_it ... ok

**C137** - `Ir::check` accepts element forms on an array carrier only, with an integer index and a constant element in range. (CV-24, AC 2, the IR half; T70)
Proof: run irarr | test element_forms_are_checked_against_the_array ... ok

**C138** - An array of carriers is one array pin with its count and its initial value as one element or every element. (CV-27, AC 1, the plan half; T72)
Proof: run planarr | test an_array_of_carriers_is_one_array_pin ... ok

**C139** - Element reads, history and commits compile with `rustc` and run on the engine. (CV-25, AC 1, the printer half; T73)
Proof: run tarr | test element_reads_and_commits_compile_and_run ... ok

**C140** - An array length that reads a param is folded at elaboration and makes that param structural. (CV-36, AC 1; QC2; T78)
Proof: run elabarr | test an_array_length_read_from_a_param_makes_it_structural ... ok

**C141** - `GAPS.md` no longer lists L24 and its §9 coverage, counts and H9 follow. (CV-37, AC 1; T90)
Proof: run docs | test gaps_drops_what_composite_values_phase_c_closed ... ok

**C142** - The fences the arrays of carriers lift (E.5, §8.2's `UartShift`, §22.1.3) are held to the checker. (CV-37, AC 2; T89)
Proof: run galleries | test the_fences_composite_values_lifted_are_checked ... ok

**C143** - natures#2 and modules#10 no longer refuse for the array of carriers; their remaining refusals are named. (CV-37, AC 2, the Book half; T89)
Proof: run book | test signals_of_every_family_are_refused_for_the_logic_and_the_net ... ok
Proof: run book | test the_memory_bank_is_refused_for_its_generic_module ... ok

**C144** - The twins print the bytes they printed before phase C, with `ToleranceContract` unedited. (CV-35, AC 1; T90)
Proof: `cargo test -p piperine-cli --test twins`

**C145** - The build has no warning. (CV-35, AC 2; T90)
Proof: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-23): the user was away; design-c.md QC1 to QC6 and DC1 to DC6 are the defaults taken. Dependencies on `generics`: T64 (A–B merged on `main`), T74 (`Length`), T75/T79 (handles, `HandleBinding`), and `generics` C builds on this phase (GN-17, GN-19, `[Bit; ROWS]`).
**T64 re-read (2026-11-23):** `generics` A-B are ancestors of the head (merge `2d6b1128` of `origin/main`, MD-81). The Build line was red at the base by one dead-code warning, `Reached::run` of `tests/book/types.rs` (left by the generics merge); T64 removes it. Anchors found: `Recv::CarrierArray` and `E0211` (`pom/resolve/members.rs`), `array_type` (`types/aggregate.rs:131`), `Length` (`types/ty.rs:187`), `drive_target` (`types/definition.rs:37`, elab twin `elab/drives.rs:39`), `Place`/`Projection` (`piperine-ir/src/model.rs`), `CompositeLayout`/`PinLayout` (`piperine-plan/src/{composite,pin}.rs`), `Unrolling` (`elab/unroll.rs`, the type is the `Scope::unrolled` mechanism), `Statement { pin, offset, len, flags, min_after }` (`piperine-abi/src/raw.rs`), `Store`/`Journal` (`piperine-digital-solver/src/{store,journal}.rs`, the store keeps a per-net `first` flag and a net journal arena), `Port` (`piperine-solver/src/core/bind.rs`), `ElementsBench` (`piperine-parity/tests/explicit/initial.msl:80`). Differences from the design: (1) there is no `HandleBinding` enum: generics B binds a handle as `Scope::handles: DeclId -> ExprId` (`elab/handles.rs`), so T79 extends that map's target, not an enum; (2) `Cell.element` cannot be appended to `Cell`: the transpile goldens print `Cell { kind, index, part }` literals and must stay byte-identical, so element addressing is a parallel `elements` list on `Function` and `Reaction` (appended `List<u64>`, same length as `reads` or empty, `WHOLE = u64::MAX`), recorded as an amendment in abi.md by T65; (3) `PinSpec` is built with `..PinSpec::discrete(..)` in the goldens, so `PinSpec.count` is safe.
**T65 amendments:** (a) element addressing is `Function.elements`/`Reaction.elements` (appended `List<u64>`) instead of `Cell.element`, so the transpile goldens stay byte-identical (the goldens spell `Cell { kind, index, part }`); (b) `ArrayView.data` is named `context` (`contract.rs::raw_views_have_no_public_pointer_field` forbids a public `data:` field); (c) `contract.rs::the_host_has_one_commit` (EU-16 AC 2, 'Host is 16 bytes, one commit') is superseded by QC1: `Host` is 24 bytes with `commit_element`, the test now pins that; `raw.rs::struct_layouts_match_the_abi` and `tests/validate.rs::every_shape_local_rule_is_enforced` grew the new fields and three rule rows; (d) `Handle::react`'s `Commit` gained `element: Option<u64>`, `React::with_elements`/`commit_element` carry element commits through the lens.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 16 → Phase 17 → … → Phase 23

Phase 16: T64
Phase 17: T65 -----→ T66 -----→ T67 -----→ T68 -----→ T69
Phase 18: T70 -----→ T71
Phase 19: T72 -----→ T73
Phase 20: T74 -----→ T75 -----→ T76 -----→ T77
Phase 21: T78 -----→ T79 -----→ T80 -----→ T81 -----→ T82
Phase 22: T83 -----→ T84 -----→ T85
Phase 23: T86 -----→ T87 -----→ T88 -----→ T89 -----→ T90
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet; the Batch gate runs once at each batch's end):

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| C1 | 16, 17 | T64 to T69 | starts only when `generics` A–B are on `main`; the ABI, engine, binding and runtime under the language |
| C2 | 18, 19, 20 | T70 to T77 | IR, plan, printer, checker |
| C3 | 21, 22 | T78 to T85 | elaborator and the end-to-end proofs |
| C4 | 23 | T86 to T90 | host, Book, closure; closes L24 |

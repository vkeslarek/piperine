# Solver ABI Core Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-abi-core/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-abi --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run abi**: `cargo test -p piperine-abi`
**Run guards**: `cargo test -p piperine-solver --test crate_boundaries --test workspace_shape`

Executors: Opus sub-agents, one commit per task. Verifier: Fable. Work in `/home/keslarek/Git/piperine-solver-completeness`, branch `feature/solver-completeness`. Never pass `--workspace`, never run `cargo clean`, do not set `CARGO_TARGET_DIR`, do not push. Five kept crates do not compile (`piperine-project`, `piperine-cli`, `piperine-lang-server`, `piperine-api`, the root shell) and stay that way.

Rules every task follows:

- **abi.md is the contract.** Field order, names and meanings come from `.specs/features/device-abi/abi.md`. Where it is silent the design decides; where the compiler refuses it, stop and report unless the design already lists the change.
- **No macro** for tables, constants, impls or thunks (`core::mem::offset_of!`, `assert!` and the like are the standard library, not ours). Data and plain functions.
- **`unsafe` only in `raw.rs`, `thunk.rs` and `caller.rs`**, each block with a one-line `// SAFETY:` naming the abi.md sentence it relies on.
- Tests derive from the spec's criteria and abi.md's stated values, never from the implementation. Test functions have exactly the names the checks cite. Every target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on `piperine-abi` and `piperine-solver`.
- Commit message as given per task, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("zero warnings is the bar"; "Where the tests are"), `.specs/STATE.md` MD-28 (a target lives in the crate it exercises), MD-31 (an invariant lives in a gate), MD-33 (no file-scope lint suppression); user: unit tests only.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| `piperine-abi` contract types, layouts, shape builder | unit | every struct's size and field offsets; every constant's value; each AC of SAC-01, 02, 03, 07 | `crates/piperine-abi/tests/{contract,layouts,shape}.rs` | `cargo test -p piperine-abi` |
| `piperine-abi` Rust side (trait, lenses, thunks, caller) | unit | each AC of SAC-04, 05, 06, 08 and each edge case of the spec | `crates/piperine-abi/tests/{thunks,caller}.rs` | `cargo test -p piperine-abi` |
| `piperine-abi` checks (validate, verify) | unit | one planted defect per shape-local rule; exact, mismatching and skipped partials | `crates/piperine-abi/tests/{validate,verify}.rs` | `cargo test -p piperine-abi` |
| Worked examples | unit | abi.md §11.1 to §11.6: stated values and partials through the thunks | `crates/piperine-abi/tests/examples.rs` | `cargo test -p piperine-abi --test examples` |
| Workspace manifests, structural guards, instruction files, `.specs/STATE.md` | unit | covered by the guard tests the same task edits | `crates/piperine-solver/tests/{crate_boundaries,workspace_shape}.rs` | `cargo test -p piperine-solver --test crate_boundaries --test workspace_shape` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | never alone | `cargo test -p piperine-abi` |
| Full | every task | `cargo test -p piperine-abi`; from T10 also `cargo test -p piperine-solver --test crate_boundaries --test workspace_shape` |
| Build | last task of each phase | Full, plus the two `**Build**` commands (zero warnings), plus `cargo test -p piperine-lang --test suite_hygiene`, plus `cargo metadata --no-deps --format-version 1 > /dev/null` |

---

## Execution Plan

Phases are ordered and run sequentially. Two batches, one per phase (4 and 6 tasks). No task is numerically sensitive: this feature touches no solver code.

### Phase 1: The contract as data

```
T1 → T2 → T3 → T4
```

### Phase 2: A device in Rust and its checks

```
T5 → T6 → T7 → T8 → T9 → T10
```

---

## Task Breakdown

### T1: Crate `piperine-abi` with the `repr(C)` types and constants

**What**: Create the crate with `raw.rs`: every struct of abi.md §3, the constants, and stride-aware list reading.
**Where**: `crates/piperine-abi/src/raw.rs` (+ `Cargo.toml`, `src/lib.rs`, root `Cargo.toml` members, target `tests/contract.rs`)
**Depends on**: None
**Reuses**: abi.md §3 struct text; design `raw`
**Requirement**: SAC-01, SAC-02, SAC-12 (AC 1)

**Done when**:

- [ ] `crates/piperine-abi/Cargo.toml` has empty `[dependencies]` and no `[dev-dependencies]`; the crate is in `members` and `default-members` of the root manifest
- [ ] Every struct of abi.md §3 exists `#[repr(C)]` with the listed field order; every enumeration is a set of `pub const u32` starting at 1; `GROUND`, `DEFAULT`, `MAJOR`, `MINOR`, `SYMBOL` exist
- [ ] Test `struct_layouts_match_the_abi` asserts `size_of`, `align_of` and each field's `offset_of!` against a hand-written table for a 64-bit target (C1)
- [ ] Test `every_constant_has_its_pinned_value` asserts each constant's value and that no group contains 0 or a duplicate
- [ ] Test `list_reads_by_the_producers_stride` reads a `List<Pin>` built with a stride of `size_of::<Pin>() + 16` and gets every element (C2)
- [ ] Test `empty_list_yields_nothing` passes
- [ ] Test `crate_declares_no_dependency` reads the manifest (C3)
- [ ] `default_members_are_the_green_crates` of `workspace_shape.rs` is updated to include the crate and passes
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): piperine-abi crate with the repr(C) contract types`

---

### T2: Shipped layouts, layout equality and the canonical form

**What**: `BIT`, `REAL`, `BYTE`, `layout_eq` (P4) and `is_canonical` (P3).
**Where**: `crates/piperine-abi/src/layout.rs` (+ target `tests/layouts.rs`)
**Depends on**: T1
**Reuses**: abi.md §3.1 P3, P4; §11 (`BIT`, `LOW`, `HIGH`)
**Requirement**: SAC-03

**Done when**:

- [ ] `BIT` is `{ v: BOOL @0, known @1 }`, size 2, align 1, `LOW = [0,1]`, `HIGH = [1,1]` as abi.md §11 states; `REAL` and `BYTE` follow the design; their offsets are written into abi.md §3.1 P3 in this commit
- [ ] Test `shipped_layouts_have_their_stated_fields` passes (C4)
- [ ] Test `layout_equality_ignores_names` passes: equal but renamed fields are equal; a differing leaf, offset, count or known offset is unequal (C5)
- [ ] Test `a_valueless_field_with_value_bytes_is_not_canonical` passes, with the all-zero buffer canonical (C6)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): shipped layouts, layout equality and the canonical form`

---

### T3: Owned shape builder

**What**: `ShapeBuf` and `ShapeRef`: build a shape from plain specs and keep its lists alive.
**Where**: `crates/piperine-abi/src/shape.rs` (+ target `tests/shape.rs`)
**Depends on**: T2
**Reuses**: design `shape`; `raw::List`
**Requirement**: SAC-07

**Done when**:

- [ ] `ShapeBuf` has `pin`, `term`, `input`, `watch`, `function`, `reaction`, `freeze`; each adder returns the entry's index
- [ ] Test `resistor_shape_reads_back_through_raw_pointers` builds abi.md §11.1 and reads pins, term, function, reads, writes, partials, names and origins back through `as_raw()` (C7)
- [ ] Test `shape_outlives_its_builder_scope` drops every `ShapeRef` but one clone and still reads the shape; under `cargo miri` it is not required
- [ ] `ShapeRef` is `Send + Sync` (a compile-time assertion function in the test)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): owned shape builder`

---

### T4: Shape-local validation

**What**: `validate(&Shape)`: the shape-local rules of abi.md §9 with typed errors.
**Where**: `crates/piperine-abi/src/validate.rs` (+ target `tests/validate.rs`)
**Depends on**: T3
**Reuses**: abi.md §9 bind validation list, §3.1 tables, §4 F2, A3, P5, P7; `ShapeBuf` for fixtures
**Requirement**: SAC-09

**Done when**:

- [ ] `Rule` has one variant per rule of the spec's list (twelve); `ShapeError` carries rule, entry name and `origin`
- [ ] Test `every_shape_local_rule_is_enforced` walks a plain table of (rule, planted defect) with one row per rule and asserts the rule and the entry name in each error (C8)
- [ ] Test `a_zeroed_shape_entry_is_an_unknown_constant` passes
- [ ] Test `an_index_is_checked_before_it_is_followed` passes: a cell index of `u32::MAX` returns an error and reads nothing
- [ ] The valid resistor and flip-flop fixtures return `Ok`
- [ ] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `feat(abi): shape-local validation`

---

### T5: `Device` trait and the lenses

**What**: The trait of abi.md §7 with `Values`, `Fault`, `Flow`, `Pod` and the borrow-checked lenses `Eval` and `React`.
**Where**: `crates/piperine-abi/src/device.rs`
**Depends on**: None (phase order: starts after T4 closes the previous phase)
**Reuses**: design `device`; `raw::Eval`, `raw::React`
**Requirement**: SAC-04, SAC-06

**Done when**:

- [ ] The trait has exactly `prototype`, `create`, `eval`, `react` with the signatures of the design
- [ ] `Pod` is implemented by hand for the ten numeric leaves and `[T; N]`
- [ ] The lenses compute each cell's byte length from the shape entry they borrow; `get::<T>` returns `None` when `T` does not fit
- [ ] Unit tests in the module build a frame by hand over stack buffers and read `f64`, `u8`, `bytes`, `get`, write `set`, `out`, `jac`; `typed_access_that_does_not_fit_is_none` passes (C13)
- [ ] No `unsafe` outside the three files the preamble names: lenses take already-checked slices from the thunk
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): Device trait and the Eval and React lenses`

---

### T6: Generic thunks

**What**: `module::<D>(name)` and `library(..)`: the `extern "C"` entry points over a `Device`, with `catch_unwind`.
**Where**: `crates/piperine-abi/src/thunk.rs` (+ target `tests/thunks.rs`)
**Depends on**: T5
**Reuses**: design `thunk`; `device.rs`
**Requirement**: SAC-04, SAC-05, SAC-06

**Done when**:

- [ ] Test `module_pointers_call_the_device` creates, evaluates and destroys a counting device through the raw pointers (C9)
- [ ] Test `destroy_drops_the_device_and_releases_the_shape` observes the drop and the `Arc` count (C10)
- [ ] Test `a_panic_is_a_fault_with_its_message` passes and the test process does not abort (C11)
- [ ] Test `a_fault_carries_its_code_and_a_cut_message` passes with a 300-byte message that has a multi-byte character at the cut (C12)
- [ ] Test `commit_reaches_the_host_once` records the arguments of `Host.commit` (C14); `an_event_statement_commits_zero_bytes` and `a_refused_commit_is_an_error` pass
- [ ] Test `finish_is_a_status` passes (C15)
- [ ] Test `a_value_for_a_non_param_pin_is_refused` passes
- [ ] `library(..)` returns a `Library` with `size = size_of::<Library>()` and `minor = MINOR`
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): generic extern C thunks over Device`

---

### T7: Solver-side caller

**What**: `caller::Handle`: create, eval, react and destroy through a `Module`, with the commit closure trampoline.
**Where**: `crates/piperine-abi/src/caller.rs` (+ target `tests/caller.rs`)
**Depends on**: T6
**Reuses**: design `caller`; `thunk.rs` for the devices under test
**Requirement**: SAC-08

**Done when**:

- [ ] Test `handle_builds_the_frames_and_returns_results` evaluates a function and runs a reaction through `Handle` (C16)
- [ ] Test `handle_reports_fault_and_reshape_as_errors` passes for both statuses with code and message
- [ ] Test `dropping_a_handle_destroys_the_instance` passes
- [ ] `Handle` is `Send` and not `Sync` (compile-time assertion in the test for `Send`; a doc comment states why not `Sync`)
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): solver-side caller Handle`

---

### T8: Finite-difference verifier of partials

**What**: `verify_partials` of abi.md §8.
**Where**: `crates/piperine-abi/src/verify.rs` (+ target `tests/verify.rs`)
**Depends on**: T7
**Reuses**: design `verify`; `caller::Handle`
**Requirement**: SAC-10

**Done when**:

- [ ] Test `exact_partials_verify` passes on a resistor and a capacitor (C17)
- [ ] Test `a_wrong_partial_is_reported_with_its_index` flips one partial by 1 % and reads index, declared and finite values (C18)
- [ ] Test `a_point_where_the_limit_is_active_is_skipped` passes (C19)
- [ ] The verifier calls nothing but `Handle::eval`
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `feat(abi): finite-difference verifier of declared partials`

---

### T9: The worked examples of abi.md as fixtures

**What**: The devices of abi.md §11.1 to §11.6 as `impl Device`, tested through thunks, `validate` and `verify_partials`.
**Where**: `crates/piperine-abi/tests/examples.rs` (+ `tests/support/examples.rs` for the devices; abi.md §7 edited to the compiled signatures)
**Depends on**: T8
**Reuses**: abi.md §11.1 to §11.6 text, constant for constant
**Requirement**: SAC-11, SAC-09 (AC 2)

**Done when**:

- [ ] Resistor, diode with limiting and noise, capacitor, D flip-flop, comparator and DAC exist with the shapes their comments state
- [ ] Test `worked_examples_produce_their_stated_values` asserts each function's outputs and partials at one hand-computed point, and the flip-flop's and comparator's commits (C20)
- [ ] Test `worked_examples_pass_validation` passes (C21)
- [ ] Test `worked_examples_pass_the_partials_verifier` passes for every function with a partial
- [ ] abi.md §7 shows `ShapeRef` and the non-`const` `module`, with one sentence saying why; where an example of §11 does not compile as printed, abi.md is corrected in this commit and the commit body lists each correction
- [ ] Gate: full

**Tests**: unit
**Gate**: full
**Commit**: `test(abi): the worked examples of the ABI as fixtures`

---

### T10: Guards, MD-61 and the instruction files

**What**: Let a sub-solver depend on `piperine-abi` only, record MD-61 and list the crate.
**Where**: `crates/piperine-solver/tests/crate_boundaries.rs` (+ `workspace_shape.rs`, `.specs/STATE.md`, `CLAUDE.md`, `AGENTS.md`)
**Depends on**: T9
**Reuses**: the scan helpers of the two guard targets
**Requirement**: SAC-12

**Done when**:

- [ ] `sub_solvers_depend_on_no_workspace_crate` is renamed `sub_solvers_depend_on_the_abi_crate_only` and allows exactly `piperine-abi` (C22); its self-test still catches a planted dependency on the other sub-solver
- [ ] Test `abi_crate_is_a_leaf` asserts `piperine-abi` has no dependency (C23)
- [ ] Test `instruction_files_and_state_name_the_abi_crate` of `workspace_shape.rs` asserts `CLAUDE.md` and `AGENTS.md` name `piperine-abi` and `.specs/STATE.md` carries `### MD-61` (C24)
- [ ] MD-61 is written in the form of MD-60 (decision, the user's words where they exist, trade-off, scope, status) and says it amends MD-60's "no in-tree crate sits under any of them"; its status is `Proposed` until the user confirms
- [ ] `cargo test -p piperine-lang --test suite_hygiene` passes
- [ ] Gate: build

**Tests**: unit
**Gate**: build
**Commit**: `docs(solver): piperine-abi under the solver crates (MD-61) and its guards`

---

## Checks

**C1** - Every struct of abi.md §3 has its `repr(C)` size and field offsets (SAC-01, AC 2)
Proof: run abi | test struct_layouts_match_the_abi ... ok

**C2** - A list is read by the producer's stride (SAC-02, AC 3)
Proof: run abi | test list_reads_by_the_producers_stride ... ok

**C3** - `piperine-abi` declares no dependency (SAC-01, AC 1)
Proof: run abi | test crate_declares_no_dependency ... ok

**C4** - `BIT`, `REAL`, `BYTE` ship with their stated fields (SAC-03, AC 4)
Proof: run abi | test shipped_layouts_have_their_stated_fields ... ok

**C5** - Layout equality ignores names and nothing else (SAC-03, AC 5)
Proof: run abi | test layout_equality_ignores_names ... ok

**C6** - A field with known byte 0 and non-zero value bytes is not canonical (SAC-03, AC 6)
Proof: run abi | test a_valueless_field_with_value_bytes_is_not_canonical ... ok

**C7** - A frozen shape reads back through its raw pointers (SAC-07, AC 7)
Proof: run abi | test resistor_shape_reads_back_through_raw_pointers ... ok

**C8** - Each of the twelve shape-local rules refuses its planted defect, naming rule and entry (SAC-09, AC 1)
Proof: run abi | test every_shape_local_rule_is_enforced ... ok

**C9** - The pointers of `module::<D>` call the device (SAC-04, AC 1)
Proof: run abi | test module_pointers_call_the_device ... ok

**C10** - `destroy` drops the device and releases the shape (SAC-04, AC 2)
Proof: run abi | test destroy_drops_the_device_and_releases_the_shape ... ok

**C11** - A panic in a device is `FAULT` with its message and no unwinding (SAC-05, AC 3)
Proof: run abi | test a_panic_is_a_fault_with_its_message ... ok

**C12** - A `Fault` reaches the note with its code and a message cut to 248 bytes at a character boundary (SAC-05, AC 4)
Proof: run abi | test a_fault_carries_its_code_and_a_cut_message ... ok

**C13** - A typed access that does not fit its cell is `None` (SAC-06, AC 6)
Proof: run abi | test typed_access_that_does_not_fit_is_none ... ok

**C14** - `cx.commit` calls `Host.commit` once with the statement, offset 0, the length and `after` (SAC-06, AC 5)
Proof: run abi | test commit_reaches_the_host_once ... ok

**C15** - `Flow::Finish` is status `FINISH` (SAC-06, AC 7)
Proof: run abi | test finish_is_a_status ... ok

**C16** - `Handle` builds the frames and returns results (SAC-08, AC 8)
Proof: run abi | test handle_builds_the_frames_and_returns_results ... ok

**C17** - Exact partials verify (SAC-10, AC 3)
Proof: run abi | test exact_partials_verify ... ok

**C18** - A partial off by 1 % is reported with its index, declared value and finite difference (SAC-10, AC 4)
Proof: run abi | test a_wrong_partial_is_reported_with_its_index ... ok

**C19** - A point where a written limit is active is skipped (SAC-10, AC 5)
Proof: run abi | test a_point_where_the_limit_is_active_is_skipped ... ok

**C20** - The six worked examples produce the values and partials abi.md states (SAC-11, AC 9)
Proof: run abi | test worked_examples_produce_their_stated_values ... ok

**C21** - The six worked examples pass `validate` (SAC-09, AC 2)
Proof: run abi | test worked_examples_pass_validation ... ok

**C22** - A sub-solver may depend on `piperine-abi` and on no other workspace crate (SAC-12, AC 2)
Proof: run guards | test sub_solvers_depend_on_the_abi_crate_only ... ok

**C23** - The workspace lists the crate and it is a leaf (SAC-12, AC 1)
Proof: run guards | test abi_crate_is_a_leaf ... ok

**C24** - MD-61 and the instruction files name the crate (SAC-12, AC 3)
Proof: run guards | test instruction_files_and_state_name_the_abi_crate ... ok

---

## Handoff

**Settled mid-build:** none
**Dispatched:** none
**Walked:** not applicable - no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1:  T1 → T2 → T3 → T4
Phase 2:  T5 → T6 → T7 → T8 → T9 → T10
```

Batches: Phase 1 (4 tasks), Phase 2 (6 tasks). Phase 2 starts after T4 because the thunks' `create` and the examples call `validate` and `ShapeBuf`.

## Task Granularity Check

Each task is one module of the crate with its own test target, or one guard change. T1 also creates the manifest and the workspace entry because a module cannot exist without its crate. T10 touches two guard targets and three documents because they state one fact (where the crate sits) and a commit that changed only some of them would leave the guards red.

## Test Co-location Validation

Every task that adds code adds its tests in the same commit (matrix: unit). T5's lens tests are unit tests in the module because the lenses are exercised end to end only once T6 exists; T6 re-proves them through real thunks. No task says `Tests: none`.

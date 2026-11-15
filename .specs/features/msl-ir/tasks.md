# MSL IR (object model, M1 node set) Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-ir/design.md`
**Status**: Done - T1 to T8 delivered; Verifier rounds 1 and 2; **Verdict**: PASS after fixes (round-2 findings closed by 626bd373, 75f2885b, 7c63ce64, 336ff9e9, 94b14b89; see `verification.md`)
**Build**: `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-ir || cargo build -p piperine-ir --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run model**: `cargo test -p piperine-ir --test model`
**Run serde**: `cargo test -p piperine-ir --test serde`
**Run print**: `cargo test -p piperine-ir --test print`
**Run check**: `cargo test -p piperine-ir --test check`
**Run deps**: `cargo test -p piperine-ir --test deps`

Executors: Sonnet implements T1 to T7, Opus implements T8 (the dependency analysis decides the Jacobian pattern of every backend); Opus verifies the feature. The worktree and branch are told at dispatch. This feature has no dependency on another M1 feature: it runs in wave 1.

Rules every task follows:

- **The crate is a leaf.** `piperine-ir` depends on `serde` (workspace) only; `serde_json` is a dev-dependency. Nothing is imported from `piperine-lang`: its patterns are copied (`pom/ids.rs`, `pom/reflect/mod.rs`, `pom/check.rs`).
- **The data model of `design.md` is the contract.** Field and variant names are those of its Data Models section; `msl-elab` and `ir-interp` are written against them in parallel. A change you believe necessary: stop and report.
- **The hand-built fixtures are shared.** `crates/piperine-ir/tests/support/devices.rs` is `#[path]`-included by `piperine-interp`'s tests, and the golden texts of T6, in the structural form of IR-11 (`tests/golden/*.txt`), are what `msl-elab` must reproduce; the full text is pinned under `tests/golden/full/`. Build each body exactly as `tests/ngspice/devices.msl` reads, in source order.
- Ids are `u32` newtypes written out by hand, one per arena. Reals are stored and compared by bit pattern.
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
| Storage, ids, builder, handles, node set | unit | 1:1 to the ACs of IR-01, IR-02, IR-07, IR-08 and edge case 2, on hand-built IRs | `crates/piperine-ir/tests/model.rs`, `tests/support/devices.rs` | `cargo test -p piperine-ir --test model` |
| Serialization | unit | each AC of IR-03; `inf` and `-0.0` round-trip | `crates/piperine-ir/tests/serde.rs` | `cargo test -p piperine-ir --test serde` |
| Printer | unit | each AC of IR-04 and IR-11; one golden text per device body and one for the divider, in each form | `crates/piperine-ir/tests/print.rs`, `tests/golden/*.txt`, `tests/golden/full/*.txt` | `cargo test -p piperine-ir --test print` |
| `Ir::check` | unit | each AC of IR-06, edge cases 1 and 3; one broken IR per entry kind | `crates/piperine-ir/tests/check.rs` | `cargo test -p piperine-ir --test check` |
| Dependency analysis | unit | each AC of IR-09 on all five device bodies; every operator row of the affine rule | `crates/piperine-ir/tests/deps.rs` | `cargo test -p piperine-ir --test deps` |
| Workspace manifests and guards | unit | the existing guards stay green with the new member | `crates/piperine-solver/tests/workspace_shape.rs` | `cargo test -p piperine-solver --test workspace_shape` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | a task that touches one target | `cargo test -p piperine-ir --test <the task's target>` |
| Full | every task from T2 on | `cargo test -p piperine-ir` |
| Build | T1 and the last task of each phase | `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Three phases, run in order. Two batches: Phases 1 and 2 (T1 to T7, one Sonnet worker), Phase 3 (T8, Opus).

### Phase 1: Storage, builder, node set, handles

Executor: Sonnet.

```
T1 → T2 → T3 → T4
```

### Phase 2: Serialization, printer, check

Executor: Sonnet.

```
T5 → T6 → T7
```

### Phase 3: Static dependencies

Executor: Opus.

```
T8
```

---

## Task Breakdown

## Phase 1 tasks

### T1: Create the crate, the ids and the storage

**What**: The crate `piperine-ir` with `ids.rs` (one `u32` newtype per arena: `NatureId`, `BodyId`, `FunctionId`, `ExprId`, `StmtId`, `InstanceId`, `NodeId`) and `model.rs` (every struct and enum of design "Data Models", fields crate-private), joined to the workspace.
**Where**: `crates/piperine-ir/` (the new crate)
**Depends on**: None
**Reuses**: `crates/piperine-lang/src/pom/ids.rs` (the id pattern), design "Data Models"
**Requirement**: IR-01
**Executor**: Sonnet

**Done when**:

- [x] The crate has `Cargo.toml`, `src/lib.rs`, `src/ids.rs` and `src/model.rs`, and the root manifest lists it in `members` and `default-members`
- [x] `crates/piperine-solver/tests/workspace_shape.rs`: `RETIRED_CRATES` drops `piperine-ir` and `DEFAULT_MEMBERS` gains `crates/piperine-ir` (Q6, decided); the stale note in `crates/piperine-lang/tests/suite_hygiene.rs` about the name is updated
- [x] Every type of design "Data Models" exists with `Debug, Clone, PartialEq`; ids also `Copy, Eq, Hash, PartialOrd, Ord`
- [x] `Constant::Real` and `Expr::Real` compare by bit pattern
- [x] `cargo tree -p piperine-ir` lists no in-tree crate
- [x] `workspace_shape` and `suite_hygiene` are green
- [x] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `ids_are_u32_newtypes_one_per_arena`, `ir_is_send_and_sync`
- [x] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): the piperine-ir crate, typed ids and storage`

---

### T2: IrBuilder

**What**: `IrBuilder` with `new`, `nature`, `body`, `function`, `expr`, `stmt`, `node`, `instance`, `finish(top)`; ids handed out in call order; `Ir` has no `&mut self` method.
**Where**: `crates/piperine-ir/src/build.rs`
**Depends on**: T1
**Reuses**: T1 storage
**Requirement**: IR-01
**Executor**: Sonnet

**Done when**:

- [x] `finish` consumes the builder and returns an `Ir`
- [x] `a_finished_ir_has_no_mutating_operation` scans `crates/piperine-ir/src` and finds no `&mut self` method on `Ir` or a handle
- [x] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `a_finished_ir_has_no_mutating_operation`, `equal_builder_sequences_give_equal_irs`
- [x] Gate check passes: `cargo test -p piperine-ir`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): IrBuilder, the only writer`

---

### T3: The node set and the hand-built device bodies

**What**: Builder entry points for every node of IR-07, and the test support that builds `Resistor`, `Capacitor`, `Vsource`, `Isource` and `Diode` (with the functions `vt` and `pnjlim`) exactly as `tests/ngspice/devices.msl` reads.
**Where**: `crates/piperine-ir/tests/support/devices.rs`
**Depends on**: T2
**Reuses**: `tests/ngspice/devices.msl`, `crates/piperine-lang/std/analog.msl` (`vt`, `pnjlim`), `std/constants.msl` (`K_B`, `Q` folded to reals)
**Requirement**: IR-07
**Executor**: Sonnet

**Done when**:

- [ ] `Devices::resistor(&mut IrBuilder) -> BodyId` and its four siblings exist, each one function of a `Devices` struct
- [ ] Every `Expr`, `Stmt`, `Addend`, `Terminal` and `Site` variant of the design is constructed by at least one fixture or test
- [ ] The diode's contribution is two addends: direct `id + gmin() * vd` and `ddt` of `cj0 * vd`; its `limit` site is named `vd` with seed `vcrit`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `value_code_of_the_subset_is_representable`, `a_body_holds_params_nodes_branches_and_sites`, `a_probe_spans_two_terminals`, `environment_reads_are_nodes`, `limit_and_stim_sites_are_indexed`, `a_contribution_is_addends_direct_or_ddt`, `a_nature_carries_abstol_and_units`, `every_expression_records_its_type`, `a_body_without_analog_has_no_branches`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the M1 node set and the five hand-built device bodies`

---

### T4: Handles and path lookup

**What**: `Copy` handles `Body`, `Function`, `Instance`, `Node`, `Nature` with the methods of design "Handles", `Ir::instance_at`, `Ir::node_at`, and a hand-built `Divider` design (top, `v1`, `r1`, `r2`, nodes `vin`, `mid`) in the test support.
**Where**: `crates/piperine-ir/src/handle.rs`
**Depends on**: T3
**Reuses**: `crates/piperine-lang/src/pom/reflect/mod.rs` (the handle pattern); T3 fixtures
**Requirement**: IR-02, IR-08
**Executor**: Sonnet

**Done when**:

- [ ] `Devices::divider(&mut IrBuilder) -> InstanceId` builds the twin `tests/ngspice/divider.msl` with paths `Divider`, `Divider.v1`, `Divider.r1`, `Divider.r2`, `Divider.vin`, `Divider.mid`
- [ ] `Node::instances()` lists the instances bound to a node
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `handles_are_copy`, `contained_items_come_in_declaration_order`, `an_instance_reaches_its_parent_and_body`, `path_lookup_finds_instances_and_nodes`, `one_body_serves_every_instance_of_it`, `instances_are_stored_in_pre_order`, `the_node_table_names_nature_declarer_and_init`, `the_authored_tree_is_walkable`
- [ ] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): navigable handles and lookup by path`

---

## Phase 2 tasks

### T5: Serialization

**What**: `Serialize` and `Deserialize` for `Ir` and every storage type, reals written as their `u64` bit pattern.
**Where**: `crates/piperine-ir/tests/serde.rs`
**Depends on**: None
**Reuses**: `serde` derives on T1's types; `Design::from_json` as the precedent
**Requirement**: IR-03
**Executor**: Sonnet

**Done when**:

- [ ] `inf`, `-0.0` and `1e-14` survive a JSON round trip bit for bit
- [ ] Arbitrary JSON and a truncated IR both fail to deserialize
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test serde`: `storage_types_implement_serde`, `json_round_trip_gives_an_equal_ir`, `text_that_is_no_ir_fails_to_deserialize`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): serde for the whole model, reals by bit pattern`

---

### T6: The one-way printer and the golden texts

**What**: `impl Display for Ir` in the form of design "Printed form", and one golden file per device body plus one for the divider.
**Where**: `crates/piperine-ir/src/print.rs`
**Depends on**: T5
**Reuses**: `Design::dump` (`pom/design.rs`) as the style; golden-file pattern of `crates/piperine-lang/tests/msl_print.rs`
**Requirement**: IR-04
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-ir/tests/golden/{resistor,capacitor,vsource,isource,diode,divider}.txt` exist and the tests compare byte for byte
- [ ] `no_parser_of_the_text_exists` scans the crate's public API for a `FromStr` impl or a `parse` function and finds none
- [ ] Every origin prints as `file:line:col`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test print`: `printed_text_shows_every_item`, `printing_twice_is_byte_identical`, `no_parser_of_the_text_exists`, `unequal_irs_print_different_texts`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): deterministic one-way printer with golden texts`

---

### T7: Ir::check

**What**: `Ir::check(&self) -> Vec<Broken>` covering every invariant of IR-06 and the duplicated-path edge case; it never panics.
**Where**: `crates/piperine-ir/src/check.rs`
**Depends on**: T6
**Reuses**: `crates/piperine-lang/src/pom/check.rs` (a list of broken invariants)
**Requirement**: IR-06
**Executor**: Sonnet

**Done when**:

- [ ] Each test corrupts a serialized fixture (JSON edit) and reads the entry that names the item and its origin
- [ ] A site or a contribution under an `if`, in a function or in a closure is an entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test check`: `a_built_ir_checks_clean`, `an_out_of_range_id_is_named`, `an_instance_with_wrong_arity_is_named`, `a_misplaced_or_duplicated_site_is_named`, `a_wrong_type_in_an_analog_position_is_named`, `a_top_alone_prints_and_checks`, `a_duplicated_path_is_named`
- [ ] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): Ir::check lists broken invariants`

---

## Phase 3 tasks

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
| Storage, ids, builder, handles, node set | unit | 1:1 to the ACs of IR-01, IR-02, IR-07, IR-08 and edge case 2, on hand-built IRs | `crates/piperine-ir/tests/model.rs`, `tests/support/devices.rs` | `cargo test -p piperine-ir --test model` |
| Serialization | unit | each AC of IR-03; `inf` and `-0.0` round-trip | `crates/piperine-ir/tests/serde.rs` | `cargo test -p piperine-ir --test serde` |
| Printer | unit | each AC of IR-04 and IR-11; one golden text per device body and one for the divider, in each form | `crates/piperine-ir/tests/print.rs`, `tests/golden/*.txt`, `tests/golden/full/*.txt` | `cargo test -p piperine-ir --test print` |
| `Ir::check` | unit | each AC of IR-06, edge cases 1 and 3; one broken IR per entry kind | `crates/piperine-ir/tests/check.rs` | `cargo test -p piperine-ir --test check` |
| Dependency analysis | unit | each AC of IR-09 on all five device bodies; every operator row of the affine rule | `crates/piperine-ir/tests/deps.rs` | `cargo test -p piperine-ir --test deps` |
| Workspace manifests and guards | unit | the existing guards stay green with the new member | `crates/piperine-solver/tests/workspace_shape.rs` | `cargo test -p piperine-solver --test workspace_shape` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | a task that touches one target | `cargo test -p piperine-ir --test <the task's target>` |
| Full | every task from T2 on | `cargo test -p piperine-ir` |
| Build | T1 and the last task of each phase | `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Three phases, run in order. Two batches: Phases 1 and 2 (T1 to T7, one Sonnet worker), Phase 3 (T8, Opus).

### Phase 1: Storage, builder, node set, handles

Executor: Sonnet.

```
T1 → T2 → T3 → T4
```

### Phase 2: Serialization, printer, check

Executor: Sonnet.

```
T5 → T6 → T7
```

### Phase 3: Static dependencies

Executor: Opus.

```
T8
```

---

## Task Breakdown

## Phase 1 tasks

### T1: Create the crate, the ids and the storage

**What**: The crate `piperine-ir` with `ids.rs` (one `u32` newtype per arena: `NatureId`, `BodyId`, `FunctionId`, `ExprId`, `StmtId`, `InstanceId`, `NodeId`) and `model.rs` (every struct and enum of design "Data Models", fields crate-private), joined to the workspace.
**Where**: `crates/piperine-ir/` (the new crate)
**Depends on**: None
**Reuses**: `crates/piperine-lang/src/pom/ids.rs` (the id pattern), design "Data Models"
**Requirement**: IR-01
**Executor**: Sonnet

**Done when**:

- [x] The crate has `Cargo.toml`, `src/lib.rs`, `src/ids.rs` and `src/model.rs`, and the root manifest lists it in `members` and `default-members`
- [x] `crates/piperine-solver/tests/workspace_shape.rs`: `RETIRED_CRATES` drops `piperine-ir` and `DEFAULT_MEMBERS` gains `crates/piperine-ir` (Q6, decided); the stale note in `crates/piperine-lang/tests/suite_hygiene.rs` about the name is updated
- [x] Every type of design "Data Models" exists with `Debug, Clone, PartialEq`; ids also `Copy, Eq, Hash, PartialOrd, Ord`
- [x] `Constant::Real` and `Expr::Real` compare by bit pattern
- [x] `cargo tree -p piperine-ir` lists no in-tree crate
- [x] `workspace_shape` and `suite_hygiene` are green
- [x] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `ids_are_u32_newtypes_one_per_arena`, `ir_is_send_and_sync`
- [x] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): the piperine-ir crate, typed ids and storage`

---

### T2: IrBuilder

**What**: `IrBuilder` with `new`, `nature`, `body`, `function`, `expr`, `stmt`, `node`, `instance`, `finish(top)`; ids handed out in call order; `Ir` has no `&mut self` method.
**Where**: `crates/piperine-ir/src/build.rs`
**Depends on**: T1
**Reuses**: T1 storage
**Requirement**: IR-01
**Executor**: Sonnet

**Done when**:

- [x] `finish` consumes the builder and returns an `Ir`
- [x] `a_finished_ir_has_no_mutating_operation` scans `crates/piperine-ir/src` and finds no `&mut self` method on `Ir` or a handle
- [x] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `a_finished_ir_has_no_mutating_operation`, `equal_builder_sequences_give_equal_irs`
- [x] Gate check passes: `cargo test -p piperine-ir`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): IrBuilder, the only writer`

---

### T3: The node set and the hand-built device bodies

**What**: Builder entry points for every node of IR-07, and the test support that builds `Resistor`, `Capacitor`, `Vsource`, `Isource` and `Diode` (with the functions `vt` and `pnjlim`) exactly as `tests/ngspice/devices.msl` reads.
**Where**: `crates/piperine-ir/tests/support/devices.rs`
**Depends on**: T2
**Reuses**: `tests/ngspice/devices.msl`, `crates/piperine-lang/std/analog.msl` (`vt`, `pnjlim`), `std/constants.msl` (`K_B`, `Q` folded to reals)
**Requirement**: IR-07
**Executor**: Sonnet

**Done when**:

- [ ] `Devices::resistor(&mut IrBuilder) -> BodyId` and its four siblings exist, each one function of a `Devices` struct
- [ ] Every `Expr`, `Stmt`, `Addend`, `Terminal` and `Site` variant of the design is constructed by at least one fixture or test
- [ ] The diode's contribution is two addends: direct `id + gmin() * vd` and `ddt` of `cj0 * vd`; its `limit` site is named `vd` with seed `vcrit`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `value_code_of_the_subset_is_representable`, `a_body_holds_params_nodes_branches_and_sites`, `a_probe_spans_two_terminals`, `environment_reads_are_nodes`, `limit_and_stim_sites_are_indexed`, `a_contribution_is_addends_direct_or_ddt`, `a_nature_carries_abstol_and_units`, `every_expression_records_its_type`, `a_body_without_analog_has_no_branches`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): the M1 node set and the five hand-built device bodies`

---

### T4: Handles and path lookup

**What**: `Copy` handles `Body`, `Function`, `Instance`, `Node`, `Nature` with the methods of design "Handles", `Ir::instance_at`, `Ir::node_at`, and a hand-built `Divider` design (top, `v1`, `r1`, `r2`, nodes `vin`, `mid`) in the test support.
**Where**: `crates/piperine-ir/src/handle.rs`
**Depends on**: T3
**Reuses**: `crates/piperine-lang/src/pom/reflect/mod.rs` (the handle pattern); T3 fixtures
**Requirement**: IR-02, IR-08
**Executor**: Sonnet

**Done when**:

- [ ] `Devices::divider(&mut IrBuilder) -> InstanceId` builds the twin `tests/ngspice/divider.msl` with paths `Divider`, `Divider.v1`, `Divider.r1`, `Divider.r2`, `Divider.vin`, `Divider.mid`
- [ ] `Node::instances()` lists the instances bound to a node
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test model`: `handles_are_copy`, `contained_items_come_in_declaration_order`, `an_instance_reaches_its_parent_and_body`, `path_lookup_finds_instances_and_nodes`, `one_body_serves_every_instance_of_it`, `instances_are_stored_in_pre_order`, `the_node_table_names_nature_declarer_and_init`, `the_authored_tree_is_walkable`
- [ ] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): navigable handles and lookup by path`

---

## Phase 2 tasks

### T5: Serialization

**What**: `Serialize` and `Deserialize` for `Ir` and every storage type, reals written as their `u64` bit pattern.
**Where**: `crates/piperine-ir/tests/serde.rs`
**Depends on**: None
**Reuses**: `serde` derives on T1's types; `Design::from_json` as the precedent
**Requirement**: IR-03
**Executor**: Sonnet

**Done when**:

- [ ] `inf`, `-0.0` and `1e-14` survive a JSON round trip bit for bit
- [ ] Arbitrary JSON and a truncated IR both fail to deserialize
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test serde`: `storage_types_implement_serde`, `json_round_trip_gives_an_equal_ir`, `text_that_is_no_ir_fails_to_deserialize`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): serde for the whole model, reals by bit pattern`

---

### T6: The one-way printer and the golden texts

**What**: `impl Display for Ir` in the form of design "Printed form", and one golden file per device body plus one for the divider.
**Where**: `crates/piperine-ir/src/print.rs`
**Depends on**: T5
**Reuses**: `Design::dump` (`pom/design.rs`) as the style; golden-file pattern of `crates/piperine-lang/tests/msl_print.rs`
**Requirement**: IR-04
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-ir/tests/golden/{resistor,capacitor,vsource,isource,diode,divider}.txt` exist and the tests compare byte for byte
- [ ] `no_parser_of_the_text_exists` scans the crate's public API for a `FromStr` impl or a `parse` function and finds none
- [ ] Every origin prints as `file:line:col`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test print`: `printed_text_shows_every_item`, `printing_twice_is_byte_identical`, `no_parser_of_the_text_exists`, `unequal_irs_print_different_texts`
- [ ] Gate check passes: `cargo test -p piperine-ir`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(ir): deterministic one-way printer with golden texts`

---

### T7: Ir::check

**What**: `Ir::check(&self) -> Vec<Broken>` covering every invariant of IR-06 and the duplicated-path edge case; it never panics.
**Where**: `crates/piperine-ir/src/check.rs`
**Depends on**: T6
**Reuses**: `crates/piperine-lang/src/pom/check.rs` (a list of broken invariants)
**Requirement**: IR-06
**Executor**: Sonnet

**Done when**:

- [ ] Each test corrupts a serialized fixture (JSON edit) and reads the entry that names the item and its origin
- [ ] A site or a contribution under an `if`, in a function or in a closure is an entry
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test check`: `a_built_ir_checks_clean`, `an_out_of_range_id_is_named`, `an_instance_with_wrong_arity_is_named`, `a_misplaced_or_duplicated_site_is_named`, `a_wrong_type_in_an_analog_position_is_named`, `a_top_alone_prints_and_checks`, `a_duplicated_path_is_named`
- [ ] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): Ir::check lists broken invariants`

---

## Phase 3 tasks

### T8: Static dependencies and the affine test

**What**: `Body::dependencies(Target) -> Deps` and `Body::is_affine(branch)`: one memoized abstract walk per body with the lattice `Constant < Affine < Nonlinear` of design "Deps".
**Where**: `crates/piperine-ir/src/deps.rs`
**Depends on**: None
**Reuses**: T4 handles; design "Deps"
**Requirement**: IR-09
**Executor**: Opus

**Done when**:

- [ ] Resistor: flow of `(p, n)` depends on nodes `p`, `n`, param `r`; affine
- [ ] Capacitor: charge depends on `p`, `n`, `c`; Vsource: the potential branch depends on param `dc`, `ac`, env `SourceScale`, stim site 0
- [ ] Diode: flow depends on limit site 0, params `is`, `n`, env `Temperature`, `Gmin`; not affine; the limit site depends on nodes `a`, `k`, params `is`, `n`, env `Temperature`
- [ ] A callee's own reads (`vt` reads `Temperature`) reach the caller's set
- [ ] One test row per operator: `+`, `-`, unary `-`, `*` and `/` by a constant, `*` by a probe, a call, an `if` on a constant and on a probe
- [ ] Tests with exactly these names pass in `cargo test -p piperine-ir --test deps`: `dependencies_follow_locals_and_calls`, `dependency_sets_have_a_fixed_order`, `affine_means_probes_meet_only_linear_operators`
- [ ] Gate check passes: `cargo test -p piperine-ir`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(ir): static dependency sets and the affine test`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`. A test function has exactly the name its check cites.

**C1** - The system SHALL store every node in an arena owned by one `Ir` value and address it by a typed id that is a `u32` newtype, one id type per arena, each declared without a macro. (IR-01, AC 1; T1)
Proof: run model | test ids_are_u32_newtypes_one_per_arena ... ok

**C2** - WHEN `IrBuilder::finish` has returned an `Ir` THEN the system SHALL offer no operation that adds, moves or removes a node of it. (IR-01, AC 2; T2)
Proof: run model | test a_finished_ir_has_no_mutating_operation ... ok

**C3** - WHEN the same sequence of builder calls is made twice THEN the system SHALL produce two `Ir` values that are equal and print byte-identical texts. (IR-01, AC 3; T2)
Proof: run model | test equal_builder_sequences_give_equal_irs ... ok

**C4** - The system SHALL make `Ir` `Send + Sync`. (IR-01, AC 4; T1)
Proof: run model | test ir_is_send_and_sync ... ok

**C5** - The system SHALL expose bodies, functions, instances and nodes through `Copy` handles that pair a borrow of the `Ir` with an id. (IR-02, AC 1; T4)
Proof: run model | test handles_are_copy ... ok

**C6** - WHEN a handle is asked for the items it contains THEN the system SHALL return them in declaration order: a body's params, nodes, branches, sites and children; an instance's children. (IR-02, AC 2; T4)
Proof: run model | test contained_items_come_in_declaration_order ... ok

**C7** - WHEN an instance handle is asked for its parent or its body THEN the system SHALL return that handle. (IR-02, AC 3; T4)
Proof: run model | test an_instance_reaches_its_parent_and_body ... ok

**C8** - WHEN an instance or a node is looked up by its path THEN the system SHALL return its handle, or nothing when no item has that path. (IR-02, AC 4; T4)
Proof: run model | test path_lookup_finds_instances_and_nodes ... ok

**C9** - The system SHALL represent value code of the subset: `f64` and `bool` constants, locals, param reads, unary and binary operators, `if` with `else`, blocks with `let`, calls of functions by id, calls of the catalogue methods `exp`, `ln`, `sqrt` and `abs`, and a closure with its parameters and body. (IR-07, AC 1; T3)
Proof: run model | test value_code_of_the_subset_is_representable ... ok

**C10** - The system SHALL represent a body with its params (name, type, default, `where` expression, structural flag), its nodes (ports first, then signals, each with its nature and optional initial potential), its branches, its `limit` sites, its `ac_stim` sites and its `analog` block. (IR-07, AC 2; T3)
Proof: run model | test a_body_holds_params_nodes_branches_and_sites ... ok

**C11** - The system SHALL represent a potential probe between two terminals, each a node of the body or ground. (IR-07, AC 3; T3)
Proof: run model | test a_probe_spans_two_terminals ... ok

**C12** - The system SHALL represent the environment reads `temperature()`, `gmin()` and `source_scale()`. (IR-07, AC 4; T3)
Proof: run model | test environment_reads_are_nodes ... ok

**C13** - The system SHALL represent a `limit` site with its input, its function and its optional seed, and an `ac_stim` site with its magnitude and optional phase, each with an index unique in its body. (IR-07, AC 5; T3)
Proof: run model | test limit_and_stim_sites_are_indexed ... ok

**C14** - The system SHALL represent a contribution as a branch, the quantity contributed (flow or potential) and a list of addends, each a value expression that is either direct or under `ddt`, with its sign. (IR-07, AC 6; T3)
Proof: run model | test a_contribution_is_addends_direct_or_ddt ... ok

**C15** - The system SHALL represent a nature with its name and, for potential and flow, the `abstol` and `units` of the quantity. (IR-07, AC 7; T3)
Proof: run model | test a_nature_carries_abstol_and_units ... ok

**C16** - The system SHALL record the type (`f64`, `bool`, `()`, function) of every expression. (IR-07, AC 8; T3)
Proof: run model | test every_expression_records_its_type ... ok

**C17** - The system SHALL store one body per module variant and reference it from every instance that places it. (IR-08, AC 1; T4)
Proof: run model | test one_body_serves_every_instance_of_it ... ok

**C18** - The system SHALL store every instance of the elaborated design in one table in pre-order of the hierarchy, each with its path, its body, its parent, its children, one design node or ground per node of its body, and one bound value per param of its body. (IR-08, AC 2; T4)
Proof: run model | test instances_are_stored_in_pre_order ... ok

**C19** - The system SHALL store every node of the elaborated design in one table, each with its path, its nature, the instance that declares it and its initial potential when it has one. (IR-08, AC 3; T4)
Proof: run model | test the_node_table_names_nature_declarer_and_init ... ok

**C20** - The system SHALL keep the authored tree walkable: from the top instance through children to every leaf. (IR-08, AC 4; T4)
Proof: run model | test the_authored_tree_is_walkable ... ok

**C21** - The system SHALL implement `serde::Serialize` and `serde::Deserialize` for `Ir` and every storage type it holds. (IR-03, AC 1; T5)
Proof: run serde | test storage_types_implement_serde ... ok

**C22** - WHEN an `Ir` is serialized to JSON and deserialized THEN the system SHALL return an `Ir` equal to the original. (IR-03, AC 2; T5)
Proof: run serde | test json_round_trip_gives_an_equal_ir ... ok

**C23** - IF the input of a deserialization is not a serialized `Ir` THEN the system SHALL return an error and no `Ir`. (IR-03, AC 3; T5)
Proof: run serde | test text_that_is_no_ir_fails_to_deserialize ... ok

**C24** - WHEN an `Ir` is printed THEN the system SHALL return a text that shows every body, function, instance and node, and every expression and statement reachable from them. (IR-04, AC 1; T6)
Proof: run print | test printed_text_shows_every_item ... ok

**C25** - WHEN the same `Ir` is printed twice THEN the system SHALL return byte-identical texts. (IR-04, AC 2; T6)
Proof: run print | test printing_twice_is_byte_identical ... ok

**C26** - The system SHALL expose no function that builds an `Ir` from text. (IR-04, AC 3; T6)
Proof: run print | test no_parser_of_the_text_exists ... ok

**C27** - WHEN two `Ir` values are not equal THEN the system SHALL print texts that differ. (IR-04, AC 4; T6)
Proof: run print | test unequal_irs_print_different_texts ... ok

**C28** - WHEN `Ir::check` is called on an IR built by a correct builder sequence THEN the system SHALL return an empty list. (IR-06, AC 1; T7)
Proof: run check | test a_built_ir_checks_clean ... ok

**C29** - IF an id held by a node is out of range of its arena THEN the system SHALL return an entry naming the node and the id. (IR-06, AC 2; T7)
Proof: run check | test an_out_of_range_id_is_named ... ok

**C30** - IF an instance binds a number of nodes or params different from its body's THEN the system SHALL return an entry naming the instance's path. (IR-06, AC 3; T7)
Proof: run check | test an_instance_with_wrong_arity_is_named ... ok

**C31** - IF a `limit` or `ac_stim` site index is used by two expressions, or a site or a contribution appears outside the `analog` block of a body or under an `if`, THEN the system SHALL return an entry naming the body and the origin. (IR-06, AC 4; T7)
Proof: run check | test a_misplaced_or_duplicated_site_is_named ... ok

**C32** - IF a contribution's addend, a probe operand of an operator, or a `limit` input does not have the type its position requires THEN the system SHALL return an entry naming the expression's origin. (IR-06, AC 5; T7)
Proof: run check | test a_wrong_type_in_an_analog_position_is_named ... ok

**C33** - WHEN a body is asked for the dependencies of a branch's addends, of a `limit` site or of an `ac_stim` site THEN the system SHALL return the set of body nodes probed, params read, environment functions read, and `limit` and `ac_stim` sites read, through locals and function calls. (IR-09, AC 1; T8)
Proof: run deps | test dependencies_follow_locals_and_calls ... ok

**C34** - The system SHALL return each set in a fixed order: nodes by index, then params by index, then environment reads, then sites by index. (IR-09, AC 2; T8)
Proof: run deps | test dependency_sets_have_a_fixed_order ... ok

**C35** - WHEN a body is asked whether a branch's direct addends are affine in the node potentials THEN the system SHALL answer yes exactly when every probe reaches the sum only through `+`, `-`, unary `-`, and `*` or `/` by an expression that reads no probe and no `limit` site. (IR-09, AC 3; T8)
Proof: run deps | test affine_means_probes_meet_only_linear_operators ... ok

**C36** - WHEN an `Ir` holds no instance but its top THEN the system SHALL print and check it without error. (IR edge case 1; T7)
Proof: run check | test a_top_alone_prints_and_checks ... ok

**C37** - WHEN a body has no `analog` block THEN the system SHALL store it with no branches and no sites. (IR edge case 2; T3)
Proof: run model | test a_body_without_analog_has_no_branches ... ok

**C38** - IF two instances or two nodes have the same path THEN `Ir::check` SHALL return an entry naming the path. (IR edge case 3; T7)
Proof: run check | test a_duplicated_path_is_named ... ok

**C39** - WHEN a body is asked whether a branch's direct addends are affine THEN the system SHALL class a `limit` read as a probe and an `if` on a condition that reads no probe and no `limit` site as the join of its arms. (IR-10, AC 1 and AC 2; verification round 1)
Proof: run deps | test a_limit_read_and_a_constant_if_keep_the_class ... ok

**C40** - WHEN an `Ir` is printed in structural form (`Ir::print_structural`) THEN the system SHALL return the tree part of the printed text - every nature, body, function, instance and node, and every expression and statement reachable from them - with the type of every expression whose form does not imply it, and with no origin and no arena id. (IR-11, AC 1; verification round 2)
Proof: run print | test structural_text_is_the_tree_with_types ... ok

**C41** - WHEN two `Ir` values differ only in the origins they hold or in the order of their nature, body, function, expression, statement or node arenas THEN the system SHALL print byte-identical structural texts. (IR-11, AC 2; verification round 2)
Proof: run print | test structural_text_ignores_origins_and_arena_order ... ok

**C42** - WHEN two `Ir` values differ in an expression's form, operand, constant, terminal or type, in a bound value or in a local's type THEN the system SHALL print structural texts that differ. (IR-11, AC 3; verification round 2)
Proof: run print | test structural_text_shows_forms_and_types ... ok

**C43** - The system SHALL keep the full text of IR-04, origins and arena section included, as the `Display` form of `Ir`. (IR-11, AC 4; verification round 2)
Proof: run print | test display_stays_the_full_form ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** 2026-11-15 batches 1-2 done (b631738a); proofs 41/41; Verifier round 1 (Opus) in ../piperine-msl-ir; round-1 fixes; Verifier round 2 (Opus); round-2 fix worker (Opus) closed the Major and three Minor findings - no third round, the orchestrator merges
**Walked:** not applicable - the feature puts no screen in front of a user; each story's Independent Test is a test named in `## Checks`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3

Phase 1 (Sonnet):  T1 → T2 → T3 → T4
Phase 2 (Sonnet):  T5 → T6 → T7
Phase 3 (Opus):  T8
```

Batches: B1 = Phases 1 and 2 (7 tasks, Sonnet); B2 = Phase 3 (1 task, Opus). Batches run in order; the Verifier (Opus, not the author of T8) runs after B2.

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: Create the crate, the ids and the storage | 1 crate skeleton + its data model | ✅ Granular |
| T2: IrBuilder | 1 builder | ✅ Granular |
| T3: The node set and the hand-built device bodies | 1 fixture module over the node set | ✅ Granular |
| T4: Handles and path lookup | 1 handle module | ✅ Granular |
| T5: Serialization | 1 concern: serde | ✅ Granular |
| T6: The one-way printer and the golden texts | 1 printer | ✅ Granular |
| T7: Ir::check | 1 checker | ✅ Granular |
| T8: Static dependencies and the affine test | 1 analysis | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of its phase | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | None | first of its phase | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | None | first of its phase | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: Create the crate, the ids and the storage | Storage, ids; workspace guards | unit | unit | ✅ OK |
| T2: IrBuilder | Storage, builder | unit | unit | ✅ OK |
| T3: The node set and the hand-built device bodies | Node set | unit | unit | ✅ OK |
| T4: Handles and path lookup | Handles | unit | unit | ✅ OK |
| T5: Serialization | Serialization | unit | unit | ✅ OK |
| T6: The one-way printer and the golden texts | Printer | unit | unit | ✅ OK |
| T7: Ir::check | `Ir::check` | unit | unit | ✅ OK |
| T8: Static dependencies and the affine test | Dependency analysis | unit | unit | ✅ OK |

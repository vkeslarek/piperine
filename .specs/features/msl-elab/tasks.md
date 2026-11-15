# MSL Elaboration (POM to IR, M1 subset) Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-elab/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-ir || cargo build -p piperine-ir --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run entry**: `cargo test -p piperine-lang --test elab_entry`
**Run params**: `cargo test -p piperine-lang --test elab_params`
**Run instances**: `cargo test -p piperine-lang --test elab_instances`
**Run analog**: `cargo test -p piperine-lang --test elab_analog`
**Run unlowered**: `cargo test -p piperine-lang --test elab_unlowered`

Executors: Sonnet implements T1 to T6 and T8 to T10; Opus implements T7 (the contribution split: a wrong sign or a misplaced `ddt` is a silent wrong answer); Opus verifies the feature. The worktree and branch are told at dispatch. This feature runs in wave 2: it needs `msl-ir` (the crate `piperine-ir`, its `IrBuilder`, printer, `check()` and structural golden texts) and `msl-typeck` (`piperine_lang::types`) merged into the worktree first.

Rules every task follows:

- **The POM is read, never written.** `Design` is borrowed shared; every result is a side table or an `Ir`. A test that compares `Design::dump()` before and after guards it.
- **Subset by table.** What is outside M1 is refused through one exhaustive classifier with no wildcard arm; a new POM variant must fail to compile until it is classified.
- **Codes.** A rule of `docs/spec/` carries its `Eppnn` code; an unsupported construct is `P0001` and a plain type mismatch is `P0002` (Q5, decided). Every diagnostic has a span.
- The hand-written parser and the POM (`src/msl/`, `src/pom/`) are not edited. If a task seems to need a change there, stop and report.
- The M1 corpus is `tests/ngspice/devices.msl` and the six twins beside it, read from disk by tests (`CARGO_MANIFEST_DIR/../../tests/ngspice`). They are not edited.
- **Every `Ir` returned passes `Ir::check()`**; each test that gets an `Ir` asserts it.
- **Constants are folded, params never** (ELAB-02 AC 5 and 6): a host `set` must reach the solver without re-elaboration (MD-18).
- **The printed IR is the contract with `ir-interp`.** The bodies elaborated from `tests/ngspice/devices.msl`, printed in structural form (`Ir::print_structural`: no origins, arena order not significant), must equal `crates/piperine-ir/tests/golden/{resistor,capacitor,vsource,isource,diode}.txt` byte for byte. A difference is settled by reading `devices.msl`: whichever side misreads it is fixed, and a change to a golden of `piperine-ir` is reported in the batch summary.
- Tests compare printed IR, paths and diagnostics; they never reach into crate-private lowering state.
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
| Entry point, determinism, the corpus, golden IR | unit | each AC of ELAB-01 and ELAB-05; one golden file per twin; device bodies equal to `piperine-ir`'s structural goldens | `crates/piperine-lang/tests/elab_entry.rs`, `tests/fixtures/elab/*.txt` | `cargo test -p piperine-lang --test elab_entry` |
| Constants and params | unit | each AC of ELAB-02, each code with its span and path | `crates/piperine-lang/tests/elab_params.rs` | `cargo test -p piperine-lang --test elab_params` |
| Instances, nodes, paths | unit | each AC of ELAB-03 and edge cases 1 to 3 | `crates/piperine-lang/tests/elab_instances.rs` | `cargo test -p piperine-lang --test elab_instances` |
| Analog lowering | unit | each AC of ELAB-04 and edge case 4; every sign and parenthesis form of the split | `crates/piperine-lang/tests/elab_analog.rs` | `cargo test -p piperine-lang --test elab_analog` |
| Forms refused by M1 lowering | unit | each AC of ELAB-06, one negative source per form | `crates/piperine-lang/tests/elab_unlowered.rs` | `cargo test -p piperine-lang --test elab_unlowered` |
| Instruction files | none | - | `CLAUDE.md`, `AGENTS.md` | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | `cargo test -p piperine-lang --test <the task's target>` |
| Full | every task | `cargo test -p piperine-lang`; `cargo test -p piperine-ir` |
| Build | the last task of each phase | `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene` |

---

## Execution Plan

Four phases, run in order. Three batches: Phases 1 and 2 (T1 to T6, Sonnet), Phase 3 (T7, Opus), Phase 4 (T8 to T10, Sonnet).

### Phase 1: Entry, constants, bodies, placement

Executor: Sonnet.

```
T1 → T2 → T3 → T4
```

### Phase 2: Value and analog lowering

Executor: Sonnet.

```
T5 → T6
```

### Phase 3: Contributions

Executor: Opus.

```
T7
```

### Phase 4: Refusals, the corpus, the instruction files

Executor: Sonnet.

```
T8 → T9 → T10
```

---

## Task Breakdown

## Phase 1 tasks

### T1: The entry point and the pass skeleton

**What**: `Elaborator::new(&design).run(top)`: calls `Types::check`, returns front-end and type diagnostics unchanged, refuses an unknown top or a top with a port, and for a top with an empty body returns an `Ir` with one body and one instance; `piperine-lang` gains the dependency on `piperine-ir`.
**Where**: `crates/piperine-lang/src/elab/mod.rs`
**Depends on**: None (and, outside this feature: `msl-ir` and `msl-typeck` merged)
**Reuses**: `piperine_lang::types::Types`, `piperine_ir::IrBuilder`
**Requirement**: ELAB-01, ELAB-06
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-lang/Cargo.toml` depends on `piperine-ir` by path; `lib.rs` exports `elab`
- [x] `Origin` of every IR item is `file:line:col` computed from its span
- [x] Any diagnostic means `Err` and no `Ir`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_entry`: `front_end_diagnostics_are_returned_without_an_ir`, `an_unknown_or_ported_top_is_one_diagnostic`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): elab entry point over Types and IrBuilder`

---

### T2: ConstEval

**What**: `ConstEval::value(expr, bindings)` over the subset: `f64` and `bool` arithmetic and comparisons, `if`, blocks, calls of reached functions, the four catalogue methods, `const` items evaluated once; IEEE 754 throughout.
**Where**: `crates/piperine-lang/src/elab/consts.rs`
**Depends on**: T1
**Reuses**: `Types` (`callee`, `method`)
**Requirement**: ELAB-02
**Executor**: Sonnet

**Done when**:

- [x] `K_B`, `Q` and a `const` defined from them fold to the bit-exact `f64` Rust computes
- [x] `1.0 / 0.0` is `inf`, not an error
- [x] A read of a param with no binding is reported to the caller as not constant
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_params`: `a_const_read_is_folded_once`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): const evaluator of the M1 subset`

---

### T3: Body params and nodes

**What**: `BodyLowering` for a module's own declarations: params (type, folded default, `where` lowered as an expression over the body's params, never structural), nodes (ports first, then signals, nature, initial potential), natures.
**Where**: `crates/piperine-lang/src/elab/body.rs`
**Depends on**: T2
**Reuses**: `pom/reflect/hardware.rs`; `ConstEval`; `IrBuilder::{nature, body}`
**Requirement**: ELAB-02, ELAB-03
**Executor**: Sonnet

**Done when**:

- [x] `Electrical` is lowered once with `abstol` 1e-6 (potential) and 1e-12 (flow) and its units
- [x] A signal `r#out` is a node named `out`
- [x] A node initial value that reads a param is `P0001` at the value
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_params`: `a_where_is_recorded_over_the_body_params`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_instances`: `a_node_init_that_reads_a_param_is_p0001`, `an_empty_module_is_an_empty_body`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): lower module params, nodes and natures to an IR body`

---

### T4: Placement

**What**: `Placement`: the instance tree in pre-order, the node table, port binding to the parent's nodes or ground, bound param values with defaults, the `where` check on bound values, paths, `<module>#<k>` names, recursion detection.
**Where**: `crates/piperine-lang/src/elab/place.rs`
**Depends on**: T3
**Reuses**: `ConstEval`; `IrBuilder::{node, instance}`; `docs/spec/part_vi_io.md` §21 rules 1 and 3
**Requirement**: ELAB-02, ELAB-03
**Executor**: Sonnet

**Done when**:

- [x] `E0317` names the param's path (`Top.r1.r`); `E0319`, `E0321`, `E0341` each asserted with its span
- [x] A bound value that reads a param of the parent is `P0001`
- [x] `DiodeSeries.d1` and `DiodeSeries.d2` both bind `DiodeSeries.b`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_params`: `an_unset_param_takes_its_default`, `a_required_param_left_out_is_e0319`, `a_false_where_is_e0317_with_the_path`, `a_bound_value_that_is_not_constant_is_refused`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_instances`: `one_body_per_module_one_instance_per_placement`, `instances_come_in_pre_order`, `paths_start_at_the_top_module`, `signals_are_nodes_and_ports_bind_the_parents`, `a_node_init_is_recorded_on_body_and_design`, `an_unnamed_placement_is_module_hash_k`, `a_module_that_places_itself_is_e0341`, `a_module_under_two_parents_is_one_body`, `an_unconnected_node_stays_in_the_table`
- [x] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(lang): place instances, bind nodes and params, check where`

---

## Phase 2 tasks

### T5: Value and function lowering

**What**: Expressions, `let`s and blocks of an `analog` block and of reached functions lowered to the IR nodes of the same form; each function once (`FunctionId` memoized); a `const` read as its value, a param read as `Param(index)`, a catalogue method as `Math`.
**Where**: `crates/piperine-lang/src/elab/lower.rs`
**Depends on**: None
**Reuses**: `Types` (`ty`, `method`, `callee`, `local`); `IrBuilder::{expr, stmt, function}`
**Requirement**: ELAB-02, ELAB-04
**Executor**: Sonnet

**Done when**:

- [x] `pnjlim` is one `FunctionId` however many bodies call it
- [x] Every lowered expression carries its `ValueType` and origin
- [x] A param read inside an `analog` `let` prints as a param read, never as a number
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_params`: `a_param_read_is_never_folded`
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_analog`: `value_code_lowers_to_the_same_forms_and_functions_once`
- [x] Gate check passes: `cargo test -p piperine-lang`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): lower value code and functions to IR`

---

### T6: Probes, environment and sites

**What**: Potential probes as `Probe` over body terminals, `temperature()`, `gmin()`, `source_scale()` as `Env`, each `limit` placement and `ac_stim` call as a site numbered in source order, a `limit` site named after the `let` it initializes, its closure lowered as `Closure`.
**Where**: `crates/piperine-lang/src/elab/analog.rs`
**Depends on**: T5
**Reuses**: T5 lowering; `CallClass::Probe`; `Callee::Core`
**Requirement**: ELAB-04
**Executor**: Sonnet

**Done when**:

- [x] `v(a)` is a probe to ground; `v(a, k)` between two body nodes
- [x] The diode's site prints `limit L0 "vd"` with its seed
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_analog`: `probes_and_environment_lower_to_their_nodes`, `sites_are_numbered_in_source_order_and_named`
- [x] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(lang): lower probes, environment reads and limit/ac_stim sites`

---

## Phase 3 tasks

### T7: Contributions: branches and the split into addends

**What**: A contribution lowered to `Contribute`: one branch per unordered terminal pair (orientation from the first contribution, kind potential when any contribution to it is potential), the right side flattened at its top-level `+`/`-` into signed leaves, each `ddt(x)` leaf its own addend, the runs between them one direct addend each, a reversed pair negated.
**Where**: `crates/piperine-lang/src/elab/contribute.rs`
**Depends on**: None
**Reuses**: T6; design "What the split of a contribution looks like"; `.specs/features/device-abi/abi.md` A1
**Requirement**: ELAB-04
**Executor**: Opus

**Done when**:

- [ ] The diode prints `b0.flow +> direct (id + (gmin() * vd)), ddt (cj0 * vd)`
- [ ] `a - (b + ddt(x))` gives a direct addend `a - b` and a negated `ddt` addend `x`; `-ddt(x)` and `i(n, p) +> ddt(x)` both negate
- [ ] `E0241` and `E0415` each asserted with its span
- [ ] A `ddt` that is not a whole top-level leaf of a flow contribution is refused (the code and message land in T8's table)
- [ ] The five device bodies, printed in structural form, equal `crates/piperine-ir/tests/golden/*.txt` byte for byte
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_analog`: `a_branch_is_one_unordered_pair_with_its_kind`, `a_sum_splits_into_direct_and_ddt_addends`, `a_reversed_pair_negates_its_addends`, `a_branch_from_a_node_to_itself_is_e0241`, `potential_and_flow_on_one_branch_is_e0415`, `a_single_addend_is_one_direct_addend`
- [ ] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(lang): lower contributions to branches and addends`

---

## Phase 4 tasks

### T8: Forms refused by M1 lowering

**What**: `Unlowered`: one table of the forms the type checker accepts and M1 elaboration refuses, each `P0001` naming the form: a site or contribution under `if`, in a function or in a closure; `ddt` off the top of a flow contribution or in a potential contribution; `m`/`dtemp` given; a `digital` block; an `extern` module.
**Where**: `crates/piperine-lang/src/elab/unlowered.rs`
**Depends on**: None
**Reuses**: T7's refusal points; `msl-typeck` `Unsupported` as the pattern
**Requirement**: ELAB-06
**Executor**: Sonnet

**Done when**:

- [ ] One negative source per row of the table, asserting `P0001`, the form's name and its span
- [ ] Every such source returns `Err` and no `Ir`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_unlowered`: `a_site_or_contribution_under_if_is_p0001`, `ddt_off_the_top_of_a_flow_contribution_is_p0001`, `m_dtemp_or_extern_at_a_placement_is_p0001`, `a_digital_block_is_p0001`, `a_diagnostic_means_no_ir`
- [ ] Gate check passes: `cargo test -p piperine-lang`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(lang): refuse by name the forms M1 elaboration does not lower`

---

### T9: The corpus and its golden IR

**What**: The six twins elaborate with no diagnostic; one golden file per twin pins the printed IR; elaboration is deterministic and leaves the design unchanged.
**Where**: `crates/piperine-lang/tests/elab_entry.rs`
**Depends on**: T8
**Reuses**: `tests/ngspice/*.msl`; golden-file pattern of `tests/msl_print.rs`
**Requirement**: ELAB-01, ELAB-05
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-lang/tests/fixtures/elab/{divider,diode_iv,diode_series,ac_rc,tran_rc,isrc_r}.txt` exist and are compared byte for byte
- [ ] `device_bodies_match_the_ir_goldens` compares the five bodies, in structural form (`Ir::print_structural`), with `crates/piperine-ir/tests/golden/*.txt`
- [ ] Every `Ir` has an empty `check()`; `TranRc.out` has initial potential 0
- [ ] Every negative test of the feature asserts a code and a span inside the source, and a path where an instance is named
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test elab_entry`: `a_twin_elaborates_to_a_checked_ir`, `the_design_is_unchanged_by_elaboration`, `elaborating_twice_prints_the_same_text`, `every_diagnostic_has_code_span_and_path`, `the_twins_elaborate_without_diagnostics`
- [ ] Gate check passes: `cargo test -p piperine-lang`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `test(lang): the M1 twins elaborate to pinned IR`

---

### T10: Instruction files describe the new stages

**What**: `CLAUDE.md` and `AGENTS.md` say that `piperine-lang` type-checks and elaborates the M1 subset into `piperine-ir`, with the pipeline diagram and the crate table updated.
**Where**: `CLAUDE.md`
**Depends on**: T9
**Reuses**: the current crate table and pipeline diagram
**Requirement**: ELAB-01
**Executor**: Sonnet

**Done when**:

- [ ] The crate table has a row for `piperine-ir` and `piperine-lang`'s row names `types/` and `elab/`
- [ ] `AGENTS.md`'s workspace map and "Only ... compile today" sentence match
- [ ] No sentence still says type checking or elaboration is "not built yet"
- [ ] `crates/piperine-solver/tests/workspace_shape.rs` (which reads the instruction files) stays green
- [ ] Gate check passes: `cargo test -p piperine-lang`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: none
**Gate**: build

**Commit**: `docs: CLAUDE.md and AGENTS.md name the type checker, the elaborator and piperine-ir`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`. A test function has exactly the name its check cites.

**C1** - WHEN `Elaborator::run` is called with a `Design` without diagnostics and the name of a module of its root crate THEN the system SHALL return an `Ir` whose `check()` is empty. (ELAB-01, AC 1; T9)
Proof: run entry | test a_twin_elaborates_to_a_checked_ir ... ok

**C2** - The system SHALL take the `Design` by shared borrow and leave `Design::dump()` byte-identical after the call. (ELAB-01, AC 2; T9)
Proof: run entry | test the_design_is_unchanged_by_elaboration ... ok

**C3** - IF the `Design` holds a diagnostic, or type checking returns one, THEN the system SHALL return no `Ir` and SHALL return those diagnostics. (ELAB-01, AC 3; T1)
Proof: run entry | test front_end_diagnostics_are_returned_without_an_ir ... ok

**C4** - IF the named top does not exist, or has a port, THEN the system SHALL return no `Ir` and one diagnostic naming the module. (ELAB-01, AC 4; T1)
Proof: run entry | test an_unknown_or_ported_top_is_one_diagnostic ... ok

**C5** - WHEN `Elaborator::run` is called twice with the same `Design` and top THEN the system SHALL return two `Ir` values with byte-identical printed texts. (ELAB-01, AC 5; T9)
Proof: run entry | test elaborating_twice_prints_the_same_text ... ok

**C6** - WHEN a placement gives no value for a param THEN the system SHALL bind it to its default evaluated as a const expression. (ELAB-02, AC 1; T4)
Proof: run params | test an_unset_param_takes_its_default ... ok

**C7** - IF a param with no default is given no value THEN the system SHALL reject the model with `E0319` at the placement. (ELAB-02, AC 2; T4)
Proof: run params | test a_required_param_left_out_is_e0319 ... ok

**C8** - IF a bound param value makes the param's `where` condition false THEN the system SHALL reject the model with `E0317` and SHALL name the param's path. (ELAB-02, AC 3; T4)
Proof: run params | test a_false_where_is_e0317_with_the_path ... ok

**C9** - IF a bound param value or a default reads anything but constants THEN the system SHALL reject the model with `P0001` when it reads a param of the parent, and with `E0321` otherwise. (ELAB-02, AC 4; T4)
Proof: run params | test a_bound_value_that_is_not_constant_is_refused ... ok

**C10** - The system SHALL replace every read of a `const` item by its value, evaluated once. (ELAB-02, AC 5; T2)
Proof: run params | test a_const_read_is_folded_once ... ok

**C11** - The system SHALL keep every param as a value read at run time in the bodies: no param read is folded into an expression. (ELAB-02, AC 6; T5)
Proof: run params | test a_param_read_is_never_folded ... ok

**C12** - The system SHALL record each param's `where` condition in the body as an expression over the body's params. (ELAB-02, AC 7; T3)
Proof: run params | test a_where_is_recorded_over_the_body_params ... ok

**C13** - The system SHALL produce one body for each module the top reaches and one instance for each placement, the top included. (ELAB-03, AC 1; T4)
Proof: run instances | test one_body_per_module_one_instance_per_placement ... ok

**C14** - The system SHALL order the instance table in pre-order of the hierarchy, children in source order. (ELAB-03, AC 2; T4)
Proof: run instances | test instances_come_in_pre_order ... ok

**C15** - The system SHALL give each instance the path of §21 rule 1 starting with the top module's name, and each node the path of the instance that declares it followed by its name without `r#`. (ELAB-03, AC 3; T4)
Proof: run instances | test paths_start_at_the_top_module ... ok

**C16** - The system SHALL create one node of the node table for each continuous `signal` of each instance, and SHALL bind each port of a child to the node, or to ground, the parent wrote in that position. (ELAB-03, AC 4; T4)
Proof: run instances | test signals_are_nodes_and_ports_bind_the_parents ... ok

**C17** - WHEN a node is declared with `= value` THEN the system SHALL record the value as the node's initial potential, on the body node and on the design node. (ELAB-03, AC 5; T4)
Proof: run instances | test a_node_init_is_recorded_on_body_and_design ... ok

**C18** - IF a node's `= value` reads a param THEN the system SHALL reject the model with `P0001` at the value. (ELAB-03, AC 6; T3)
Proof: run instances | test a_node_init_that_reads_a_param_is_p0001 ... ok

**C19** - IF a placement has no `let` of its own THEN the system SHALL name the instance `<module>#<k>` as §21 rule 3 states. (ELAB-03, AC 7; T4)
Proof: run instances | test an_unnamed_placement_is_module_hash_k ... ok

**C20** - IF a module places itself, directly or through other modules, THEN the system SHALL reject the model with `E0341` and SHALL name the module. (ELAB-03, AC 8; T4)
Proof: run instances | test a_module_that_places_itself_is_e0341 ... ok

**C21** - The system SHALL lower each value expression, `let` and function of the type-checked set to the IR node of the same form, and each function once however many bodies call it. (ELAB-04, AC 1; T5)
Proof: run analog | test value_code_lowers_to_the_same_forms_and_functions_once ... ok

**C22** - The system SHALL lower a potential probe to a `Probe` over the terminals of its nodes, and `temperature()`, `gmin()` and `source_scale()` to their `Env` nodes. (ELAB-04, AC 2; T6)
Proof: run analog | test probes_and_environment_lower_to_their_nodes ... ok

**C23** - The system SHALL create one branch per unordered pair of terminals that receives a contribution, with the kind potential when any contribution to it is a potential contribution and flow otherwise (§11). (ELAB-04, AC 3; T7)
Proof: run analog | test a_branch_is_one_unordered_pair_with_its_kind ... ok

**C24** - WHEN a contribution's right side is a sum THEN the system SHALL split it at its top-level `+` and `-` into addends in source order, an addend that is exactly `ddt(x)` becoming an addend under `ddt` with value `x`, and every maximal run of other addends becoming one direct addend. (ELAB-04, AC 4; T7)
Proof: run analog | test a_sum_splits_into_direct_and_ddt_addends ... ok

**C25** - WHEN a contribution is written on the reversed pair of an existing branch THEN the system SHALL negate its addends (§11 rule 10). (ELAB-04, AC 5; T7)
Proof: run analog | test a_reversed_pair_negates_its_addends ... ok

**C26** - The system SHALL give each `limit` placement and each `ac_stim` call of a block its own site, numbered in source order, and SHALL name a `limit` site after the `let` it initializes when it has one. (ELAB-04, AC 6; T6)
Proof: run analog | test sites_are_numbered_in_source_order_and_named ... ok

**C27** - IF both nodes of a contribution are the same node THEN the system SHALL reject the model with `E0241`. (ELAB-04, AC 7; T7)
Proof: run analog | test a_branch_from_a_node_to_itself_is_e0241 ... ok

**C28** - IF a branch receives both potential and flow contributions THEN the system SHALL reject the model with `E0415`. (ELAB-04, AC 8; T7)
Proof: run analog | test potential_and_flow_on_one_branch_is_e0415 ... ok

**C29** - IF a contribution, a `limit` placement or an `ac_stim` call is written under an `if`, in a function or in a closure THEN the system SHALL reject the model with `P0001` at that form. (ELAB-06, AC 1; T8)
Proof: run unlowered | test a_site_or_contribution_under_if_is_p0001 ... ok

**C30** - IF `ddt` is written anywhere but as a whole addend of a flow contribution THEN the system SHALL reject the model with `P0001` at the `ddt`. (ELAB-06, AC 2; T8)
Proof: run unlowered | test ddt_off_the_top_of_a_flow_contribution_is_p0001 ... ok

**C31** - IF a placement sets `m` or `dtemp`, or places an `extern` module, THEN the system SHALL reject the model with `P0001` at the placement. (ELAB-06, AC 3; T8)
Proof: run unlowered | test m_dtemp_or_extern_at_a_placement_is_p0001 ... ok

**C32** - IF a module of the reachable set has a `digital` block THEN the system SHALL reject the model with `P0001` at the block. (ELAB-06, AC 4; T8)
Proof: run unlowered | test a_digital_block_is_p0001 ... ok

**C33** - The system SHALL return no `Ir` whenever it returns a diagnostic. (ELAB-06, AC 5; T8)
Proof: run unlowered | test a_diagnostic_means_no_ir ... ok

**C34** - The system SHALL attach to every diagnostic a code and the span of the construct, and to a diagnostic about an instance its path (§0.3 item 4). (ELAB-05, AC 1; T9)
Proof: run entry | test every_diagnostic_has_code_span_and_path ... ok

**C35** - WHEN a model of the subset meets none of the conditions of ELAB-02 to ELAB-06 THEN the system SHALL return an `Ir` and no diagnostic. (ELAB-05, AC 2; T9)
Proof: run entry | test the_twins_elaborate_without_diagnostics ... ok

**C36** - WHEN a module has no `analog` block and places nothing THEN the system SHALL produce a body with no branches and an instance with no children. (ELAB edge case 1; T3)
Proof: run instances | test an_empty_module_is_an_empty_body ... ok

**C37** - WHEN the same module is placed under two parents THEN the system SHALL produce one body and two instances. (ELAB edge case 2; T4)
Proof: run instances | test a_module_under_two_parents_is_one_body ... ok

**C38** - IF a node is connected to no instance THEN the system SHALL keep it in the node table. A floating node is the solver's to report (`E0503`). (ELAB edge case 3; T4)
Proof: run instances | test an_unconnected_node_stays_in_the_table ... ok

**C39** - WHEN a contribution has one addend that is not a sum THEN the system SHALL produce one direct addend. (ELAB edge case 4; T7)
Proof: run analog | test a_single_addend_is_one_direct_addend ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** 2026-11-15 batch 1 (T1-T6, Sonnet) in ../piperine-msl-elab
**Walked:** not applicable - the feature puts no screen in front of a user; each story's Independent Test is a test named in `## Checks`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1 (Sonnet):  T1 → T2 → T3 → T4
Phase 2 (Sonnet):  T5 → T6
Phase 3 (Opus):  T7
Phase 4 (Sonnet):  T8 → T9 → T10
```

Batches: B1 = Phases 1 and 2 (6 tasks, Sonnet); B2 = Phase 3 (1 task, Opus); B3 = Phase 4 (3 tasks, Sonnet). The Verifier (Opus, not the author of T7) runs after B3.

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: The entry point and the pass skeleton | 1 entry point | ✅ Granular |
| T2: ConstEval | 1 evaluator | ✅ Granular |
| T3: Body params and nodes | 1 lowering of declarations | ✅ Granular |
| T4: Placement | 1 placement pass | ✅ Granular |
| T5: Value and function lowering | 1 value lowering | ✅ Granular |
| T6: Probes, environment and sites | 1 analog lowering | ✅ Granular |
| T7: Contributions: branches and the split into addends | 1 lowering of one statement kind | ✅ Granular |
| T8: Forms refused by M1 lowering | 1 table | ✅ Granular |
| T9: The corpus and its golden IR | 1 corpus test target | ✅ Granular |
| T10: Instruction files describe the new stages | 2 instruction files, text only | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of its phase | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | None | first of its phase | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | None | first of its phase | ✅ Match |
| T8 | None | first of its phase | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: The entry point and the pass skeleton | Entry point | unit | unit | ✅ OK |
| T2: ConstEval | Constants and params | unit | unit | ✅ OK |
| T3: Body params and nodes | Constants and params; instances, nodes | unit | unit | ✅ OK |
| T4: Placement | Instances, nodes, paths | unit | unit | ✅ OK |
| T5: Value and function lowering | Analog lowering | unit | unit | ✅ OK |
| T6: Probes, environment and sites | Analog lowering | unit | unit | ✅ OK |
| T7: Contributions: branches and the split into addends | Analog lowering | unit | unit | ✅ OK |
| T8: Forms refused by M1 lowering | Forms refused by M1 lowering | unit | unit | ✅ OK |
| T9: The corpus and its golden IR | Entry point, the corpus, golden IR | unit | unit | ✅ OK |
| T10: Instruction files describe the new stages | Instruction files | none | none | ✅ OK |

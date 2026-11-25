# Nested Reach Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path.

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Status**: Implemented
**Approved**: 2026-11-25 - user: "write `.specs/features/nested-reach/{spec,tasks}.md` (criteria in EARS, one check per criterion with its proof test), then execute." (the delegation names the form of the tasks, the gates and the commit rule; the gate policy is the user's standing one: no full suite)
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesnested**: `cargo test -p piperine-lang --test types_nested_reach`
**Run elabnested**: `cargo test -p piperine-lang --test elab_nested_reach`
**Run elabhandles**: `cargo test -p piperine-lang --test elab_handles`
**Run elabtemplates**: `cargo test -p piperine-lang --test elab_templates`
**Run sourcenested**: `cargo test -p piperine-parity --test source_nested_reach`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`

Executors: Opus (the delegating agent's instruction).

Rules every task follows:

- **Environment.** `CARGO_TARGET_DIR=/var/tmp/piperine-target-d TMPDIR=/var/tmp/piperine-tmp-d CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` exported in the same command as every cargo call; cargo in the foreground; never `git stash`; stop if `df /` passes 90 %.
- **Gate policy.** No full suite. Per task: the `**Build**` line (zero warnings), `cargo test -p piperine-lang --no-fail-fast`, the parity targets the task touched, `cargo test -p piperine-cli --test docs` and the root package.
- **Code rules** (CLAUDE.md, `.specs/features/code-clarity/spec.md`): no macro, no free function, no abbreviation, every new item doc-commented, no god method; a `P0001` is the way to refuse.
- One atomic commit per task, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; no merge.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are", MD-28), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-25.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | each item kind, the cycle and the private item at depth three and four | `crates/piperine-lang/tests/types_nested_reach.rs` | `cargo test -p piperine-lang --test types_nested_reach` |
| Elaborator (`piperine-lang/src/elab`) | integration | the reached node, carrier, `let` and param, the iteration, the order, the refusals, the cycle | `crates/piperine-lang/tests/elab_nested_reach.rs` | `cargo test -p piperine-lang --test elab_nested_reach` |
| Source-level runs | integration | one test per run-time criterion, values computed by Rust | `crates/piperine-parity/tests/source_nested_reach.rs` | `cargo test -p piperine-parity --test source_nested_reach` |
| The records | integration | the removed rows and the spec text | `crates/piperine-cli/tests/docs.rs`, `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test spec_text` |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | A task whose tests are one or two targets | the `**Build**` line, then the `Run` lines the task names |
| Full | A task that changes the elaborator | Quick, then `cargo test -p piperine-lang --no-fail-fast`, the touched parity targets, `cargo test -p piperine-cli --test docs`, `cargo test -p piperine` (the root package) |

---

## Execution Plan

### Phase 1: One path

```
T1 → T2 → T3
```

---

## Task Breakdown

### T1: The reach is one recursive path (done)

**What**: A read through children of children is one mechanism at every depth. A reach frame below another (`Frame::placement`) is made for each name of the path; the slot of a node or carrier of the last is the entry of the first hop's module that names it; the demands of a run are found by running it and run again with them (`Elaborator::run_with`). The two tables of one level (`nested_value`'s folded constants, `reach_grandchild`) are gone. A carrier a parent reaches is the child's net the grandchild's replaced (`Placed::nets`). Tests: the checker and elaboration targets; the two tests that pinned the old limit are amended (`elab_handles`, `elab_templates`).
**Where**: `crates/piperine-lang/src/elab/reach.rs`
**Depends on**: None
**Reuses**: `merge_nodes` of the reached slots, `with_generation_of`, the key params of `params_read_through_reaches`
**Requirement**: NR-01, NR-02, NR-03, NR-04

**Done when**:

- [x] The tests of `**Run typesnested**`, `**Run elabnested**`, `**Run elabhandles**` and `**Run elabtemplates**` pass
- [x] `reach_values.rs` is deleted and no `nested_terminal`/`nested_value` remains
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): inst.name reads through children of children at any depth`

---
### T2: Source runs of the reach (done)

**What**: The models of `tests/nested_reach/reach.msl` and `source_nested_reach.rs`: a cell read at depth three and four by name, through templates and in generation, and cycles of connections, against the values Rust computes.
**Where**: `crates/piperine-parity/tests/source_nested_reach.rs`
**Depends on**: T1
**Reuses**: `SourceRun`, the harness of `source_templates.rs`
**Requirement**: NR-02, NR-03

**Done when**:

- [x] The tests of `**Run sourcenested**` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the reach through children of children runs from source`

---
### T3: The records (done)

**What**: The rows L81 and L82 leave `GAPS.md` (counts 17 loud), `docs.rs` pins them, and §8.2.8 says each name of the path is `pub` (with its guard in `spec_text.rs`).
**Where**: `GAPS.md`
**Depends on**: T2
**Reuses**: the pins of `docs.rs`
**Requirement**: NR-05

**Done when**:

- [x] `gaps_drops_the_reach_rows_nested_reach_closed` passes under `**Run docs**` and the other pins of the counts hold
- [x] `section_8_2_8_reads_through_children_of_children_at_any_depth` passes under `**Run spectext**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs: close L81 and L82, nested reach (§8.2.8)`

---

## Checks

**C1** - A node `v(R.sn)` three and four levels below an instance placed by name is the cell's own node, named by the path (NR-01, AC 1)
Proof: run elabnested | test a_node_is_reached_three_and_four_levels_below_the_reader ... ok

**C2** - The same through templates reads the node of the module the template names, and the cell runs at its values (NR-01, AC 2)
Proof: run sourcenested | test a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_params ... ok

**C3** - The same in generation reads the iteration its index names (NR-01, AC 3)
Proof: run sourcenested | test a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names ... ok

**C4** - The models and the top written in either order bind the same nodes and nets (NR-01, AC 4)
Proof: run elabnested | test a_reach_is_the_same_wherever_the_models_and_the_top_are_written ... ok

**C5** - A `pub` param and a `pub let` over the params read the value of the placement, the iteration's in generation (NR-02, AC 1)
Proof: run elabnested | test a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_levels_below ... ok

**C6** - A `pub let` over the solve (`vs = v(sn)`) reaches the node it reads (NR-02, AC 2)
Proof: run elabnested | test a_let_over_the_solve_reaches_the_node_it_reads_three_and_four_levels_below_the_reader ... ok

**C7** - A carrier of the cell is a reached carrier of the reader bound to the cell's net (NR-02, AC 3)
Proof: run elabnested | test a_carrier_is_reached_three_and_four_levels_below_the_reader ... ok

**C8** - Node, `let`, carrier, param and `let` over the params read the values Rust computes for the cell, by name (NR-02, AC 4)
Proof: run sourcenested | test a_cell_three_and_four_levels_down_by_name_reads_its_node_lets_carrier_and_params ... ok

**C9** - Two placements wired to each other's node, or one to its own, at depth three and four, bind each port to the node it names, in either text order (NR-03, AC 1)
Proof: run elabnested | test a_cycle_of_node_connections_three_and_four_levels_deep_binds_each_port ... ok

**C10** - Such designs settle where Kirchhoff's law puts them (NR-03, AC 2)
Proof: run sourcenested | test a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware ... ok

**C11** - The same through a generation, four levels deep, settles where Kirchhoff's law puts it (NR-03, AC 2)
Proof: run sourcenested | test a_cycle_of_node_connections_through_a_generation_four_levels_deep_runs_as_hardware ... ok

**C12** - Every item kind of a cell types three and four levels below the reader (NR-04, AC 1)
Proof: run typesnested | test every_kind_of_item_types_three_and_four_levels_below_the_reader ... ok

**C13** - A cycle of node connections at depth three and four types as hardware (NR-04, AC 1)
Proof: run typesnested | test a_cycle_of_node_connections_three_and_four_levels_deep_types_as_hardware ... ok

**C14** - An item that is not `pub` is `E0343` at the checker at any depth (NR-04, AC 2)
Proof: run typesnested | test an_item_that_is_not_pub_is_refused_three_and_four_levels_below_the_reader ... ok

**C15** - A missing or private item is `E0343` from the checker or at elaboration through a template (NR-04, AC 2)
Proof: run elabnested | test a_missing_or_private_item_is_e0343_three_and_four_levels_below_the_reader ... ok

**C16** - `GAPS.md` has no row L81 or L82, section 9 does not name them, the counts are 17 loud (NR-05, AC 1)
Proof: run docs | test gaps_drops_the_reach_rows_nested_reach_closed ... ok

**C17** - Part III §8.2.8 says each name of the path is `pub` and the item reads as one level down (NR-05, AC 2)
Proof: run spectext | test section_8_2_8_reads_through_children_of_children_at_any_depth ... ok

**C18** - The reads two levels deep that ran before still run: the cycles by template and generation bind their ports (NR-01, AC 2; regression)
Proof: run elabhandles | test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok

**C19** - The reads of a `pub` param and `let` through a template still print what the parent reads (NR-02, AC 1; regression)
Proof: run elabtemplates | test a_non_node_item_through_a_template_reads_the_concrete_module ... ok

**C20** - A drive of a resolved carrier of the cell three levels down is held beside the cell's own: the net's resolver takes both drives (NR-02, AC 5)
Proof: run elabnested | test a_drive_of_a_resolved_carrier_of_a_cell_three_levels_down_is_held_beside_the_cells_own ... ok

**C21** - The same from source: the net holds the AND of the cell's drive and the top's (NR-02, AC 5)
Proof: run sourcenested | test a_parent_drives_a_resolved_carrier_of_a_cell_three_levels_down ... ok

**C22** - A read through generic templates (`C: Fn(Electrical)` handed from one wrapper to the next) runs at depth three and four, by template and in generation, and reads the values Rust computes (NR-01, AC 2; NR-02, AC 4)
Proof: run sourcenested | test a_cell_through_generic_templates_three_and_four_levels_down_reads_its_node_lets_carrier_and_params ... ok

**C23** - A reach refused for its cause (a node declared in generation, a hoisted module that holds a placement) reports that cause once, with no cascade (GAPS L94, L95)
Proof: run elabnested | test a_reach_refused_for_its_cause_reports_that_cause_once ... ok

---

## Handoff

**Settled mid-build:** the reach, spec and tests are written together; the design decision (demands found by running the elaboration again) is in `reach_through_templates.rs`'s module doc.
**Dispatched:** none

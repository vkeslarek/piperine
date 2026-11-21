# Resolved Nets Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/resolved-nets/design.md`
**Status**: Approved
**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesnatures**: `cargo test -p piperine-lang --test types_natures`
**Run elabnatures**: `cargo test -p piperine-lang --test elab_natures`
**Run sourcenatures**: `cargo test -p piperine-parity --test source_natures`
**Run relax**: `cargo test -p piperine-digital-solver --test relax`
**Run prototran**: `cargo test -p piperine-solver --test protocol_transient`
**Run irresolved**: `cargo test -p piperine-ir --test resolved`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run planshape**: `cargo test -p piperine-plan --test digital_shape`
**Run transresolve**: `cargo test -p piperine-transpile --test resolve`
**Run sourceresolved**: `cargo test -p piperine-parity --test source_resolved`
**Run apirecords**: `cargo test -p piperine-api --test records`
**Run sourcenets**: `cargo test -p piperine-parity --test source_nets`
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookcorestd**: `cargo test -p piperine --test book_core_std`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run speccodes**: `cargo test -p piperine-lang --test spec_codes`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** The spec's open question Q1 was answered 2026-11-18 (user: "Sobre as open questions, vou de aceito todas ok?"). The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D20; every task follows them as written.

**Base and order (design D1, D2, D20).** Batches B1 and B2 (phases 1 to 3, T1 to T10) start from `main` at `7df9e17c` and need no other feature. `resolved-nets-bundles` B1 runs after them on the same branch. B3 to B7 (phases 4 to 9, T11 to T36) start only once `composite-values` phases A–B (its T57 included), `value-semantics` and `generics` phase A are on `main`; T11 rebases the branch onto that tree. If they are not on `main` when B2 and bundles B1 close, the feature pauses there and reports. `resolved-nets-bundles` T15 waits for T22 here.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-a TMPDIR=/var/tmp/piperine-tmp-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command. Cargo with `-p <crate>`, never `--workspace` (`piperine-lang-server` is postponed and known broken), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report. Never `git stash` (the stash list is shared between worktrees).
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. A new comparison uses an existing contract as it is (D12). A fair twin uses the same step configuration on both sides, nothing else. Every buck target passes after every task that touches the printer, the plan, the digital solver, the owner protocol or the elaborator's lowering of an existing form.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A spec text that is wrong is amended explicitly with its `spec_text` guard (MD-57), never left. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites (§4, §5.2, §6, §8.2.3, §22.3, §22.4.3), or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR enum variants, `CarrierRole` and plan `Source` variants are appended last. The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/natures/*.msl`, `tests/resolved/*.msl` and `tests/nets/*.msl`.
- **The Book** (21 chapters, Part 1 Fundamentals and Part 2 In depth): its prose never defers to Rust, its tone is respectful, and a status box that pins a `P0001` message is edited with the test that pins it.
- **Code owners (D16).** A task that first raises `E0203`, `E0224`, `E0242` or `E0413` deletes the code's row from `crates/piperine-lang/tests/support/code_owners.tsv` in its commit when the file is on the branch; otherwise it writes the code on the Handoff and T36 deletes the row.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No merge; push only as the orchestrator asks.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-72 (root Book tests).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every static AC; each code with its span and the names it cites | `crates/piperine-lang/tests/types_natures.rs` | `cargo test -p piperine-lang --test types_natures` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; `E0242`, `E0339` across instances; the instances of `resolve` | `crates/piperine-lang/tests/elab_natures.rs` | `cargo test -p piperine-lang --test elab_natures` |
| IR (`piperine-ir`) | integration | the new forms built, checked, printed | `crates/piperine-ir/tests/resolved.rs` | `cargo test -p piperine-ir --test resolved` |
| Interpreter refusals | integration | each new IR form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | the value, drive and others pins; the drive layout; the resolver's binding | `crates/piperine-plan/tests/digital_shape.rs` | `cargo test -p piperine-plan --test digital_shape` |
| Printer (`piperine-transpile`) | integration | `Expr::Resolve` compiled and called per count | `crates/piperine-transpile/tests/resolve.rs` | `cargo test -p piperine-transpile --test resolve` |
| Digital solver | integration | relaxation with absent and unknown drives, the pass bound, no latch, one `Unsettled` per step | `crates/piperine-digital-solver/tests/relax.rs` | `cargo test -p piperine-digital-solver --test relax` |
| Owner protocol | integration | the unsettled observation reaches the owner once per step | `crates/piperine-solver/tests/protocol_transient.rs` | `cargo test -p piperine-solver --test protocol_transient` |
| Host (`piperine-api`) | integration | the `log` record of a loop, once per loop and analysis | `crates/piperine-api/tests/records.rs` | `cargo test -p piperine-api --test records` |
| Source-level runs | integration | one test per run-time AC and edge case, from MSL source on the Rust backend; expected values from the rule | `crates/piperine-parity/tests/{source_natures,source_resolved,source_nets}.rs` | `cargo test -p piperine-parity --test source_natures` / `source_resolved` / `source_nets` |
| The Book | integration | each row this feature changes, through the binary and the root package | `tests/book/*.rs`, `tests/book/inventory.tsv`, `crates/piperine-cli/tests/book.rs` | `cargo test -p piperine --test book_<chapter>`, `cargo test -p piperine-cli --test book` |
| Spec text and repository records | integration | one assertion per amended sentence, removed `GAPS.md` row or code owner row | `crates/piperine-lang/tests/{spec_text,spec_codes}.rs`, `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-lang --test spec_text`, `--test spec_codes`, `cargo test -p piperine-cli --test docs` |
| Contract re-check (T11) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the printer, the plan, the digital solver, the owner protocol or the lowering of an existing form | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins --test commands`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members; `piperine-lang-server` is not one) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

Batches (~7 tasks, whole phases): **B1** phase 1 (T1–T6) · **B2** phases 2–3 (T7–T10) · **B3** phases 4–5 (T11–T15) · **B4** phase 6 (T16–T21) · **B5** phase 7 (T22–T26) · **B6** phase 8 (T27–T33) · **B7** phase 9 (T34–T36). B1 and B2 now; B3 onward wait (see "Base and order").

### Phase 1: Project discrete natures, signal-flow nodes and `E0224` (independent)

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Relaxation below the language (independent)

```
T7 → T8 → T9
```

### Phase 3: Records of phases 1 and 2

```
T10
```

### Phase 4: Contract (waits for `composite-values` A–B, `value-semantics` and `generics` A on `main`)

```
T11
```

### Phase 5: IR, plan and printer for resolved carriers

```
T12 → T13 → T14 → T15
```

### Phase 6: Checker and elaborator for resolved carriers

```
T16 → T17 → T18 → T19 → T20 → T21
```

### Phase 7: Resolved carriers run from source

```
T22 → T23 → T24 → T25 → T26
```

### Phase 8: The stdlib nets of §22.3 and `Bidir`

```
T27 → T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 9: The Book and the records

```
T34 → T35 → T36
```

---

## Task Breakdown

### T1: Checker: carriers of any discrete nature

**What**: `Checker::bit_type` becomes `discrete_carrier_type`: every `NatureKind::Discrete` nature, the stdlib's or the project's, generic with defaults (§4 rule 12) or not, gives `Ty::Carrier { nature, value }`; `port_type`'s `Discrete` arm stops being `P0001`. `NatureRules::of` (design C2) answers "one driver, resolved, flow" for every kind and replaces the scattered `matches!` on `NatureKind` in `types/`. Create `crates/piperine-lang/tests/types_natures.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/carrier.rs`
**Depends on**: None
**Reuses**: `bit_type`, `carried_type` (`E0112`), the one-driver checks of `types/module.rs`
**Requirement**: RN-01
**Executor**: Sonnet

**Done when**:

- [x] `a_project_discrete_port_without_a_direction_is_e0310`, `connecting_byte_to_bit_u8_is_e0251_naming_both_natures`, `a_nature_value_without_repr_is_e0112`, `a_second_driver_of_a_project_discrete_carrier_is_e0220` and `a_generic_project_nature_takes_its_argument_or_its_default` pass under `**Run typesnatures**`
- [x] The `types_carriers.rs` test that pinned `discrete nature` as `P0001` is replaced by these (design "Goldens")
- [x] Code clarity: `NatureRules` and `discrete_carrier_type` doc-commented; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): carriers of every discrete nature`

---

### T2: A project discrete nature elaborates and runs

**What**: The elaborator lowers a carrier of a project discrete nature as it lowers `Bit<T>` (`elab/nature.rs`: the net's value type is the nature's `value`, with its type arguments); placement connects it to a port of the same nature. Create `crates/piperine-parity/tests/source_natures.rs` and `tests/natures/byte_counter.msl`.
**Where**: `crates/piperine-lang/src/elab/nature.rs`
**Depends on**: T1
**Reuses**: the `Bit<T>` path of `elab/carriers.rs`, `place.rs`
**Requirement**: RN-01
**Executor**: Sonnet

**Done when**:

- [x] `a_counter_over_a_project_byte_nature_commits_and_records` and `a_project_nature_carrier_connects_to_a_port_of_the_same_nature` pass under `**Run sourcenatures**`
- [x] Code clarity: every new method doc-commented; the data path from the nature to the net's type in one place
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a project's discrete nature runs`

---

### T3: Checker: `others()` on a one-driver carrier is `E0224`

**What**: `s.others()` on a carrier whose nature has no `resolve` is `E0224` naming the carrier (§5.2.7); on a resolved carrier it stays `P0001` until T17. Delete the `E0224` row of `code_owners.tsv` (D16).
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T2
**Reuses**: `NatureRules`, `CARRIER_METHODS`
**Requirement**: RN-04
**Executor**: Sonnet

**Done when**:

- [x] `others_on_a_one_driver_carrier_is_e0224` passes under `**Run typesnatures**`
- [x] Code clarity: the rule cites §5.2.7; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): others on a one-driver carrier is e0224`

---

### T4: Checker: signal-flow ports and signals, `E0413`, `E0206`

**What**: `port_type`'s `SignalFlow` arm gives `Ty::Node(nature)`; a flow contribution to a branch whose nature has no `flow` is `E0413` (§11 rule 11); a flow probe (`i`, `flow`) on it is `E0206` (§6 rule 10), whichever layer raises it today. Delete the `E0413` row of `code_owners.tsv` (D16).
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T3
**Reuses**: `NatureRules`, `E0240` of `types/analog.rs`
**Requirement**: RN-08
**Executor**: Sonnet

**Done when**:

- [x] `a_flow_contribution_to_a_signal_flow_node_is_e0413` and `a_flow_probe_of_a_signal_flow_node_is_e0206` pass under `**Run typesnatures**`
- [x] The `types_carriers.rs` test that pinned `signal-flow nature` as `P0001` is replaced (design "Goldens")
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): signal-flow nodes`

---

### T5: Elaborator: `SignalFlowCheck` and `E0242`

**What**: After placement, `SignalFlowCheck::run` (design C8, D10, D11) counts, for every node whose natures all lack `flow`, the instances holding a potential contribution to `(n, gnd)`; zero or more than one is `E0242` naming the node and each driver; a node where any nature has `flow` is conservative and not counted. Create `crates/piperine-lang/tests/elab_natures.rs`. Delete the `E0242` row of `code_owners.tsv` (D16).
**Where**: `crates/piperine-lang/src/elab/signal_flow.rs`
**Depends on**: T4
**Reuses**: the node table of `elab/place.rs`, the contribution list of `elab/contribute.rs`
**Requirement**: RN-08
**Executor**: Sonnet

**Done when**:

- [x] `a_signal_flow_node_with_no_or_two_drivers_is_e0242` and `a_node_meeting_a_nature_with_flow_is_not_counted` pass under `**Run elabnatures**`
- [x] Code clarity: `SignalFlowCheck` doc-commented citing §6 rule 2 and its Resolution
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): a signal-flow node has one potential driver`

---

### T6: The gain chain over `VSignal` runs in every analysis

**What**: Source tests of RN-08: two gain blocks `v(out) +> k * v(inp)` over `VSignal` nodes in `op`, `dc`, `ac` and `transient`, the expected value computed in Rust and compared under `ToleranceContract::DC_VOLTAGE` as it is (D12); a `VSignal` node connected to an `Electrical` port solves as a conservative node. Models in `tests/natures/gain_chain.msl`, `mixed_node.msl`.
**Where**: `crates/piperine-parity/tests/source_natures.rs`
**Depends on**: T5
**Reuses**: the `Project` runner of `piperine-parity`, `ToleranceContract`
**Requirement**: RN-08
**Executor**: Sonnet

**Done when**:

- [x] `a_signal_flow_node_with_one_driver_solves_and_loads_nothing`, `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` and `vsignal_connected_to_an_electrical_port_is_conservative` pass under `**Run sourcenatures**`
- [x] No contract or tolerance edited
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): a signal-flow gain chain runs in every analysis`

---

### T7: The drive presence field and relaxation below the language

**What**: abi.md §9 D1 gains the sentence of design D3 (a drive net's layout is the value's layout followed by `driving: BOOL` with a known byte; zero bytes are absent). Create `crates/piperine-digital-solver/tests/relax.rs`: stub devices built on `ShapeBuf` (drivers writing value and `driving`, a resolver reading `driving` and passing the present drives to a `wired`-like fold) prove the cycle starts absent, stops within the drive count plus one, keeps no earlier level once released, leaves an absent drive out and passes an unknown one.
**Where**: `crates/piperine-digital-solver/tests/relax.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `tests/support` devices of `piperine-digital-solver`, `ShapeBuf`, `relax` in `run.rs`
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] `a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes`, `a_released_loop_keeps_no_earlier_level` and `an_absent_drive_is_left_out_and_an_unknown_one_is_passed` pass under `**Run relax**`
- [x] The abi.md sentence is in §9 D1
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(digital): relaxation with absent and unknown drives`

---

### T8: Digital solver: one `Unsettled` per relaxation step

**What**: `Observation::Unsettled { nets: Vec<String>, at }`: `relax` pushes one per step still moving after its last pass, naming every `RELAX` net of the step in name order (design C9); the zeroing of moving known fields is unchanged.
**Where**: `crates/piperine-digital-solver/src/run.rs`
**Depends on**: T7
**Reuses**: `relax`, `Obs`
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] `an_unsettled_step_is_one_observation_naming_every_relax_net` passes under `**Run relax**`
- [x] Code clarity: the variant's doc cites §8.2.3 step 4 and abi.md D1
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(digital): one unsettled observation per relaxation step`

---

### T9: Owner: `ObservationKind::Unsettled`

**What**: `ObservationKind::Unsettled { nets }` appended; `adapters.rs` maps the digital observation to it instead of an empty `Record`; nothing else changes.
**Where**: `crates/piperine-solver/src/analyses/adapters.rs`
**Depends on**: T8
**Reuses**: the adapter's observation mapping
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] `an_unsettled_loop_reaches_the_owner_once_per_step` passes under `**Run prototran**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the owner sees an unsettled loop`

---

### T10: Records of phases 1 and 2

**What**: `GAPS.md` loses L22 and L23 and the §11 row `modules#8` keeps only the `idt` refusal (owner `generics`); `tests/book/modules.rs` and `inventory.tsv` follow; `docs.rs` asserts the removed rows. The Handoff lists any code of D16 still owed.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `inventory_guard.rs`, `docs.rs`
**Requirement**: RN-10
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_no_longer_lists_project_or_signal_flow_natures` passes under `**Run docs**`
- [ ] `the_pi_controller_is_refused_only_for_its_generic_idt` passes under `**Run bookmodules**`; `**Run bookinventory**` passes
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs(gaps): project and signal-flow natures run`

---

### T11: Contract: rebase and re-read the anchors

**What**: Rebase `feature/resolved-nets` onto `main` with `composite-values` A–B (T57 included), `value-semantics` and `generics` A; re-read every anchor of design.md (`types/carrier.rs`, `types/call.rs`, `Instantiations`, carried composites, `std/nets.msl` in the `[T; N]` spelling); write on the Handoff each anchor that moved and how the following tasks adapt. Stop and report if a dependency is missing (D14).
**Where**: `.specs/features/resolved-nets/tasks.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [ ] The rebase is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test` at the root pass
- [ ] The Handoff names every moved anchor
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): resolved-nets contract after the rebase`

---

### T12: IR: resolved carriers, `Others`, `Resolve`, the resolution of a net

**What**: Design C4: `CarrierRole::Resolved`, `Expr::Others(u32)`, `Expr::Resolve { by_count, drives, except }`, `NetData::resolution: Option<Resolution>`, appended; `Ir::check` (counts, signatures, `Others` on a `Resolved` carrier); the structural printer; the interpreter refuses the three forms by name. Create `crates/piperine-ir/tests/resolved.rs`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `IrBuilder`, `check.rs`, `print.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_resolved_net_is_built_checked_and_printed` and `a_resolve_with_a_missing_count_is_refused` pass under `**Run irresolved**`
- [ ] `resolved_forms_are_refused_by_name` passes under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): resolved carriers and their nets`

---

### T13: Plan: value, drive and others pins

**What**: Design C5: `DriveLayout::of(value)`; a `Resolved` carrier's value pin (read), drive pin (`RELAX`, the drive layout) and others pin (only when `Expr::Others` reads it); a definition or commit of it targets the drive pin.
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: T12
**Reuses**: `CarrierLayout`, `ShapeBuf`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_resolved_carrier_has_value_drive_and_others_pins` and `the_drive_layout_appends_a_known_driving_field` pass under `**Run planshape**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): pins of a resolved carrier`

---

### T14: Printer: `Expr::Resolve` and the drive write

**What**: Design C6: `ResolvePrinter` gathers the present drives in order and calls `by_count[k]`; a write to a drive pin sets `driving` known true. Create `crates/piperine-transpile/tests/resolve.rs` (an IR built by hand with a `sum` over `[f64; k]`).
**Where**: `crates/piperine-transpile/src/print/resolve.rs`
**Depends on**: T13
**Reuses**: `print/digital.rs`, `print/value.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `resolve_calls_the_instance_of_the_present_count` and `a_drive_write_marks_the_drive_present` pass under `**Run transresolve**`
- [ ] Transpile goldens byte-identical
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): resolve of the present drives`

---

### T15: Netlist: the resolver instance and its nets

**What**: `Netlist::circuit` binds each resolved net: the drive nets to the holders' drive pins and the resolver's inputs in order, the value net to every value pin, each others net to its holder's others pin; a non-holder's others pin to the value net (D7).
**Where**: `crates/piperine-plan/src/netlist.rs`
**Depends on**: T14
**Reuses**: `Netlist::circuit`
**Requirement**: RN-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_resolved_net_binds_drives_value_and_others` passes under `**Run planshape**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): the resolver binds its nets`

---

### T16: Checker: resolved natures and their carriers

**What**: `ResolveSignature::check` (design C3, D15): `resolve` of a pure function with one const parameter `N: usize`, one parameter `[T; N]`, result `T`, an associated function or a const of function type accepted, anything else `E0203` naming nature and function; `port_type`'s `Resolved` arm gives `Ty::Carrier`; `E0310` for `in`/`out` on a resolved port; `E0220` skipped for resolved carriers. Delete the `E0203` row of `code_owners.tsv` (D16).
**Where**: `crates/piperine-lang/src/types/nature.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `NatureRules`, `generics`' signature checks
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_const_generic_resolve_over_an_array_is_accepted`, `resolve_may_name_an_associated_function_or_a_const`, `a_resolve_of_another_shape_is_e0203`, `resolve_without_value_or_with_potential_is_e0201` and `a_direction_on_a_resolved_port_is_e0310` pass under `**Run typesnatures**`
- [ ] The `types_carriers.rs` test that pinned `resolved nature` as `P0001` is replaced (design "Goldens")
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): resolved natures`

---

### T17: Checker: `others()`, `E0221` and loops of resolved drives

**What**: `s.others()` on a resolved carrier has the carrier's value type, in definitions, reactions and `analog`; one module holding a definition and a commit of one resolved carrier is `E0221`; `E0339` counts a module-local loop only when it passes through a one-driver carrier (§8.2.3).
**Where**: `crates/piperine-lang/src/types/definition.rs`
**Depends on**: T16
**Reuses**: the loop walk of `definition.rs`, `types/call.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221` and `a_module_local_loop_of_resolved_drives_is_accepted` pass under `**Run typesnatures**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): others and loops of resolved drives`

---

### T18: Elaborator: a body's resolved carriers, drives and `others()`

**What**: A resolved port or signal is a carrier of role `Resolved`; the body's definition or commits of it are its drive; `s.others()` lowers to `Expr::Others`.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T17
**Reuses**: `Structure`, `Frame`, `elab/drives.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_resolved_port_lowers_to_a_resolved_carrier_and_its_drive` and `others_lowers_to_the_bodys_others_read` pass under `**Run elabnatures**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): resolved carriers of a body`

---

### T19: Elaborator: `ResolverBodies` and the instances of `resolve`

**What**: Design C7, D4, D5, D8: after placement, per resolved net, `DriveOrder::of` sorts the holders (§4 rule 2 Resolution, host first), the drive and others nets are added, `resolve::<k>` is instantiated for `k` in `0..=N` through `Instantiations`, one resolver body per (nature, `N`, others readers) and one resolver instance `<net path>$resolve` are placed; `NetData::resolution` is filled.
**Where**: `crates/piperine-lang/src/elab/resolvers.rs`
**Depends on**: T18
**Reuses**: `Instantiations`, `IrBuilder`, `elab/place.rs`
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [ ] `resolve_is_instantiated_once_per_drive_count`, `drives_are_ordered_by_instance_path_segment_by_segment` and `others_depends_on_the_other_drives_only` pass under `**Run elabnatures**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): resolvers of resolved nets`

---

### T20: Elaborator: initial values of resolved carriers

**What**: `= v` on a resolved signal or port is the declaring module's drive's first commit at `(0, 0)` (§5.2.3); a drive with no initial value is present and unknown from `(0, 0)` (§5.2.12).
**Where**: `crates/piperine-lang/src/elab/place.rs`
**Depends on**: T19
**Reuses**: the initial-value path of one-driver carriers
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [ ] `an_initial_value_is_the_declaring_modules_first_drive_commit` passes under `**Run elabnatures**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): initial values of resolved carriers`

---

### T21: Elaborator: loops across instances

**What**: `DriveLoops` over the placed drive graph: a loop that passes through a one-driver carrier is `E0339` naming its drives (today it may only fail at bind); a loop among resolved drives is accepted and left to relaxation (§8.2.3).
**Where**: `crates/piperine-lang/src/elab/loops.rs`
**Depends on**: T20
**Reuses**: the drive table of `NetData::resolution`, the definition dependencies of `elab/drives.rs`
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` and `a_loop_of_resolved_drives_across_instances_is_accepted` pass under `**Run elabnatures**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): loops across instances`

---

### T22: Drives combine through `resolve`, from source

**What**: Create `crates/piperine-parity/tests/source_resolved.rs` and `tests/resolved/*.msl`: three `SumNet`-like drivers of a project nature in instances named out of order; a project `resolve` that records its argument order; an undriven carrier; a drive change that leaves the value; an initial value; a drive with none.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the runner of `source_natures.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `three_sumnet_drivers_give_their_sum_in_the_microstep_any_changes`, `an_undriven_resolved_carrier_holds_resolve_of_no_drive`, `resolve_receives_the_drives_in_instance_path_order`, `a_drive_without_initial_value_is_unknown_until_its_first_commit`, `a_drive_change_that_leaves_the_value_fires_no_changed` and `an_initial_value_is_the_declaring_modules_first_drive` pass under `**Run sourceresolved**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): resolved drives from source`

---

### T23: `others()` from source

**What**: Three drivers each emitting its `others()`; a non-driver reading `others()`.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T22
**Reuses**: T22's models
**Requirement**: RN-04
**Executor**: Sonnet

**Done when**:

- [ ] `others_is_resolve_of_every_drive_but_the_readers` and `others_of_a_module_without_a_drive_is_the_value` pass under `**Run sourceresolved**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): others from source`

---

### T24: Relaxation from source

**What**: A ring of pass elements over a project wired-AND nature settles; a released ring keeps no level; a loop of two drivers that each drive the negation of `others()` does not settle and its levels are unknown.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T23
**Reuses**: T22's models
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_loop_of_resolved_drives_is_accepted_and_settles` and `a_released_ring_latches_nothing` pass under `**Run sourceresolved**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): relaxation from source`

---

### T25: Host: the `log` record of a loop that does not settle

**What**: `UnsettledLoops` (design C9, D9) in `piperine-api`: an `ObservationKind::Unsettled` becomes, the first time its set of drive nets is seen in an analysis, one record on `<first holder path>.log` with `severity = Warning` and `msg` naming every holder in drive order.
**Where**: `crates/piperine-api/src/record.rs`
**Depends on**: T24
**Reuses**: `Streams`, `Record`, `NetData::resolution`
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [ ] `an_unsettled_loop_is_one_log_record_per_analysis` passes under `**Run apirecords**`
- [ ] `a_loop_that_does_not_settle_is_unknown_and_logged_once` and `a_loop_settling_on_its_first_pass_writes_no_record` pass under `**Run sourceresolved**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a loop that does not settle is logged`

---

### T26: Resolved nets in every analysis

**What**: The per-analysis table of §22.3 over a project nature: definitions, `resolve`, `others()` and a reaction run at the microsteps of `(0, ·)` and of each `dc` sweep point; in `ac` the nets are constant.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T25
**Reuses**: the analyses of `piperine-api`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` passes under `**Run sourceresolved**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): resolved nets in every analysis`

---

### T27: `Logic`, `Strength`, `Wire`

**What**: Create `crates/piperine-parity/tests/source_nets.rs` and `tests/nets/*.msl`: `Wire` driven by timed commits gives the table of §22.3.2; `wired([x, x])` is `x` in any order. Fix in the compiler any stdlib form of `std/nets.msl` that does not check or run (CLAUDE.md: the stdlib is an acceptance target).
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: T22's runner
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `wired_gives_the_table_of_22_3_2` and `wired_of_equal_drives_is_that_drive_in_any_order` pass under `**Run sourcenets**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): wire and wired`

---

### T28: `Tri<T>` and generic discrete natures

**What**: A nature's type arguments are substituted into its value type and passed to `resolve` (`tri::<u8, k>`), design D19; `Tri<u8>` gives the table of §22.3.3.
**Where**: `crates/piperine-lang/src/elab/resolvers.rs`
**Depends on**: T27
**Reuses**: `ResolverBodies`, `Instantiations`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `tri_gives_the_table_of_22_3_3` passes under `**Run sourcenets**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): generic resolved natures`

---

### T29: `Tribuf`, `Pullup`, `Pulldown`

**What**: Source tests of §22.3.5: `Tribuf` with `oe` high, low and unknown; a pull that wins only while every stronger drive is released.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T28
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `tribuf_drives_strong_z_or_their_merge` and `a_pullup_wins_only_while_every_stronger_drive_is_released` pass under `**Run sourcenets**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): tribuf and pulls`

---

### T30: `Keeper`

**What**: Source test of §22.3.6: weak unknown before any stronger drive; holds the last level driven above `Weak` after release.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T29
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_keeper_holds_the_last_driven_level_weakly` passes under `**Run sourcenets**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): keeper`

---

### T31: `Tran`, `Tgate` and the conformance item of Appendix C.2.3

**What**: Source tests of §22.3.7, §22.3.8 and RN-05 AC 3: `Tran` with `en` high, low, unknown; `Tgate` conducting on `s` high or `sn` low; two parallel `Tran`s carry a strong level and stay two carriers with their own `changed()`; a ring of `Tran`s settles.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T30
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [ ] `tran_drives_each_side_with_the_others_of_the_other`, `tgate_joins_while_s_high_or_sn_low` and `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` pass under `**Run sourcenets**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): tran and tgate`

---

### T32: `Bidir`

**What**: A transient with a `Tribuf`, a `Bidir` and an RC load on its node toggling `oe` (§22.4.3 rows 1 to 3, rule 2); two `Bidir`s alone on one wire are unknown and logged once (rule 4).
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T31
**Reuses**: `tests/nets/*.msl`, T25's record
**Requirement**: RN-07
**Executor**: Sonnet

**Done when**:

- [ ] `bidir_drives_the_node_while_the_wire_is_driven`, `bidir_drives_the_wire_from_the_node_when_released`, `bidir_acts_as_driven_under_an_unknown_strength`, `a_change_of_driving_is_a_time_point_and_a_jump` and `two_bidirs_alone_make_the_wire_unknown_and_log_the_loop` pass under `**Run sourcenets**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): bidir`

---

### T33: Edge cases of resolved nets

**What**: The spec's edge cases: a top-level resolved port's host drive comes first (D8); a panic inside `resolve` is a run fault naming the nature and the carrier (D13); an unknown `SumNet` drive makes the sum unknown; a parent drives a child's `pub` resolved signal.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T32
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [ ] `the_hosts_drive_of_a_top_port_comes_first`, `a_panic_in_resolve_is_a_run_fault_naming_nature_and_carrier`, `an_unknown_sumnet_drive_makes_the_sum_unknown` and `a_parent_drives_a_childs_pub_resolved_signal` pass under `**Run sourcenets**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): edge cases of resolved nets`

---

### T34: The Book's alarm runs

**What**: The chapter "Natures and carriers": the alarm's listing becomes the binary's output, its status box and the "Today the command refuses the file" sentence go (prose respectful, never deferring to Rust); `tests/book/natures.rs` replaces `the_open_drain_line_is_refused_while_resolved_natures_are_not_compiled` by run tests; rows `natures#3`, `natures~e99c1920`, `natures~6a449552` move to `run` in `inventory.tsv`; `crates/piperine-cli/tests/book.rs` holds the listing.
**Where**: `docs/book/content.html`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Chapter`, `Refusal`, `inventory_guard.rs`
**Requirement**: RN-09
**Executor**: Sonnet

**Done when**:

- [ ] `the_alarm_prints_three_records_of_the_line` and `the_pulldown_alarm_prints_the_exercise_answer` pass under `**Run booknatures**`
- [ ] `every_cli_listing_of_the_book_is_what_the_binary_prints` passes under `**Run clibook**`; `**Run bookinventory**` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs(book): the alarm line runs`

---

### T35: The other Book rows of resolved nets

**What**: `digital#16` (`IrqSource`, `IrqController`, `Board`) and `core-std#6` run; `natures#2` keeps only the array refusal (L24, owner `composite-values`); tests and `inventory.tsv` follow.
**Where**: `tests/book/digital.rs`
**Depends on**: T34
**Reuses**: `Chapter`, `Refusal`
**Requirement**: RN-09
**Executor**: Sonnet

**Done when**:

- [ ] `the_irq_line_runs_on_its_resolved_wire` passes under `**Run bookdigital**`; `the_resolved_line_runs_on_its_wire` passes under `**Run bookcorestd**`; `signals_of_every_family_are_refused_for_the_arrays_only` passes under `**Run booknatures**`; `**Run bookinventory**` passes
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): the irq lines run`

---

### T36: Records: GAPS, spec, roadmap, decisions, code owners

**What**: `GAPS.md` loses L21 and the resolved-net group of L58 (`TribufDelayed` stays with `generics`) and its §11 rows; Appendix C.2.3 and G.38 say `Tran` on `Wire`s runs, guarded in `spec_text.rs`; abi.md D1's sentence guarded too; `ROADMAP.md` names the feature delivered; `.specs/STATE.md` gains the next free `MD-` entry (resolved nets below the POM, D3 to D9); every `code_owners.tsv` row of D16 still present is deleted.
**Where**: `GAPS.md`
**Depends on**: T35
**Reuses**: `docs.rs`, `spec_text.rs`, `spec_codes.rs`
**Requirement**: RN-10
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_lists_no_resolved_net_gap` passes under `**Run docs**`; `c_2_3_and_g_38_say_tran_on_wires_runs` passes under `**Run spectext**`; `no_owned_code_is_raised` passes under `**Run speccodes**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: resolved nets run`

---

## Checks

**C1** - A counter over a project `nature Byte { value: u8 }` commits, defines, fires `changed()`, reads `prev()` and records as `Bit<u8>` does (RN-01, AC 1)
Proof: run sourcenatures | test a_counter_over_a_project_byte_nature_commits_and_records ... ok

**C2** - A `Byte` carrier passed to a `Byte` port is connected (RN-01, AC 2)
Proof: run sourcenatures | test a_project_nature_carrier_connects_to_a_port_of_the_same_nature ... ok

**C3** - A `Byte` port without `in` or `out` is `E0310` (RN-01, AC 3)
Proof: run typesnatures | test a_project_discrete_port_without_a_direction_is_e0310 ... ok

**C4** - A `Byte` carrier on a `Bit<u8>` port is `E0251` naming `Byte` and `Bit<u8>` (RN-01, AC 4)
Proof: run typesnatures | test connecting_byte_to_bit_u8_is_e0251_naming_both_natures ... ok

**C5** - A nature whose `value` has no `Repr` is `E0112` (RN-01, AC 5)
Proof: run typesnatures | test a_nature_value_without_repr_is_e0112 ... ok

**C6** - A second module driving a `Byte` carrier is `E0220` (RN-01, AC 6)
Proof: run typesnatures | test a_second_driver_of_a_project_discrete_carrier_is_e0220 ... ok

**C7** - `Word<u16>` holds `u16` and `Word` holds its default `u8` (RN-01, AC 7)
Proof: run typesnatures | test a_generic_project_nature_takes_its_argument_or_its_default ... ok

**C8** - `resolve: f` with `fn f<const N: usize>(d: [T; N]) -> T`, pure, is accepted (RN-02, AC 1)
Proof: run typesnatures | test a_const_generic_resolve_over_an_array_is_accepted ... ok

**C9** - `resolve` naming `Logic::wired` or a const of function type is accepted (RN-02, AC 2)
Proof: run typesnatures | test resolve_may_name_an_associated_function_or_a_const ... ok

**C10** - A `resolve` with no const parameter, a non-array parameter, another result or an impure class is `E0203` naming nature and function (RN-02, AC 3)
Proof: run typesnatures | test a_resolve_of_another_shape_is_e0203 ... ok

**C11** - `resolve` without `value`, or beside `potential` or `flow`, is `E0201` (RN-02, AC 4)
Proof: run typesnatures | test resolve_without_value_or_with_potential_is_e0201 ... ok

**C12** - `in` or `out` on a resolved port is `E0310` (RN-02, AC 5)
Proof: run typesnatures | test a_direction_on_a_resolved_port_is_e0310 ... ok

**C13** - `signal s: SumNet = 2.0;` makes the declaring module's drive commit `2.0` at `(0, 0)` (RN-02, AC 6)
Proof: run sourceresolved | test an_initial_value_is_the_declaring_modules_first_drive ... ok

**C14** - A carrier with `N` drives instantiates `resolve::<k>` for exactly `k = 0..=N` (RN-02, AC 7)
Proof: run elabnatures | test resolve_is_instantiated_once_per_drive_count ... ok

**C15** - Three drivers' sum is the carrier's value in the microstep any of them changes (RN-03, AC 1)
Proof: run sourceresolved | test three_sumnet_drivers_give_their_sum_in_the_microstep_any_changes ... ok

**C16** - An undriven carrier holds `resolve([])`: `0.0` for a sum (RN-03, AC 2)
Proof: run sourceresolved | test an_undriven_resolved_carrier_holds_resolve_of_no_drive ... ok

**C17** - `resolve` receives the drives in segment-wise instance-path order, host first, whatever the evaluation order (RN-03, AC 3)
Proof: run sourceresolved | test resolve_receives_the_drives_in_instance_path_order ... ok

**C18** - A drive without initial value is passed as unknown from `(0, 0)` until its first commit (RN-03, AC 4)
Proof: run sourceresolved | test a_drive_without_initial_value_is_unknown_until_its_first_commit ... ok

**C19** - A drive change that leaves the resolved value fires no `changed()` (RN-03, AC 5)
Proof: run sourceresolved | test a_drive_change_that_leaves_the_value_fires_no_changed ... ok

**C20** - A definition and a commit of one resolved carrier in one module is `E0221` (RN-03, AC 6)
Proof: run typesnatures | test a_definition_and_a_commit_of_one_resolved_carrier_is_e0221 ... ok

**C21** - A driver's `others()` is `resolve` of every drive but its own (RN-04, AC 1)
Proof: run sourceresolved | test others_is_resolve_of_every_drive_but_the_readers ... ok

**C22** - A non-driver's `others()` equals the carrier's value (RN-04, AC 2)
Proof: run sourceresolved | test others_of_a_module_without_a_drive_is_the_value ... ok

**C23** - `others()` on a `Bit` is `E0224` (RN-04, AC 3)
Proof: run typesnatures | test others_on_a_one_driver_carrier_is_e0224 ... ok

**C24** - A driver's others net reads every drive net but its own (RN-04, AC 4)
Proof: run elabnatures | test others_depends_on_the_other_drives_only ... ok

**C25** - A loop of resolved drives with no reaction or `after` is accepted and settles by relaxation (RN-05, AC 1)
Proof: run sourceresolved | test a_loop_of_resolved_drives_is_accepted_and_settles ... ok

**C26** - A relaxing cycle starts with every loop drive absent and stops within drives plus one passes (RN-05, AC 2)
Proof: run relax | test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok

**C27** - Two parallel `Tran(a, b, true)` with `a` driven strong give `b` that level, and `a`, `b` keep their own `changed()` (RN-05, AC 3)
Proof: run sourcenets | test two_parallel_trans_carry_a_strong_level_and_stay_two_carriers ... ok

**C28** - A loop still moving after its last pass leaves its moving levels unknown and one `log` record on the first drive's instance naming every drive, once per analysis (RN-05, AC 4)
Proof: run sourceresolved | test a_loop_that_does_not_settle_is_unknown_and_logged_once ... ok

**C29** - A loop through a one-driver carrier across instances is `E0339` (RN-05, AC 5)
Proof: run elabnatures | test a_loop_through_a_one_driver_carrier_across_instances_is_e0339 ... ok

**C30** - A released loop keeps no level of an earlier microstep (RN-05, AC 6)
Proof: run relax | test a_released_loop_keeps_no_earlier_level ... ok

**C31** - In a relaxation pass `resolve` gets only the present drives, and an unknown drive is present (RN-05, AC 7)
Proof: run relax | test an_absent_drive_is_left_out_and_an_unknown_one_is_passed ... ok

**C32** - `Wire` gives the four rows of §22.3.2 (RN-06, AC 1)
Proof: run sourcenets | test wired_gives_the_table_of_22_3_2 ... ok

**C33** - `wired([x, x])` is `x`, and every permutation of drives gives one result (RN-06, AC 2)
Proof: run sourcenets | test wired_of_equal_drives_is_that_drive_in_any_order ... ok

**C34** - `Tri<u8>` gives `None`, `Some(x)`, `Some(Unknown)` per §22.3.3 (RN-06, AC 3)
Proof: run sourcenets | test tri_gives_the_table_of_22_3_3 ... ok

**C35** - `Tribuf` drives `Logic::strong(d)`, `Logic::Z`, or both merged for `oe` high, low, unknown (RN-06, AC 4)
Proof: run sourcenets | test tribuf_drives_strong_z_or_their_merge ... ok

**C36** - A `Pullup` gives way to a strong drive and returns when it releases (RN-06, AC 5)
Proof: run sourcenets | test a_pullup_wins_only_while_every_stronger_drive_is_released ... ok

**C37** - `Keeper` drives weak unknown before any strong drive and holds the last strong level weakly after (RN-06, AC 6)
Proof: run sourcenets | test a_keeper_holds_the_last_driven_level_weakly ... ok

**C38** - `Tran` with `en` high drives each side with the other's `others()`; low `Logic::Z`; unknown the merge (RN-06, AC 7)
Proof: run sourcenets | test tran_drives_each_side_with_the_others_of_the_other ... ok

**C39** - `Tgate` joins `a` and `y` while `s` is high or `sn` low, and drives neither otherwise (RN-06, AC 8)
Proof: run sourcenets | test tgate_joins_while_s_high_or_sn_low ... ok

**C40** - In `op` and `dc` resolved nets run at `(0, ·)` and each sweep point; in `ac` they are constant (RN-06, AC 9)
Proof: run sourceresolved | test resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac ... ok

**C41** - While the rest of the wire drives, `Bidir` drives the node as `D2A` and the wire with `Logic::Z` (RN-07, AC 1)
Proof: run sourcenets | test bidir_drives_the_node_while_the_wire_is_driven ... ok

**C42** - While the rest has released, `Bidir` contributes nothing to the node and drives the wire `Logic::strong(seen)` (RN-07, AC 2)
Proof: run sourcenets | test bidir_drives_the_wire_from_the_node_when_released ... ok

**C43** - An unknown strength of the rest makes `Bidir` act as driven (RN-07, AC 3)
Proof: run sourcenets | test bidir_acts_as_driven_under_an_unknown_strength ... ok

**C44** - A change of `driving` is a time point and a jump that `analog` reads (RN-07, AC 4)
Proof: run sourcenets | test a_change_of_driving_is_a_time_point_and_a_jump ... ok

**C45** - Two `Bidir`s alone on a wire make it unknown and log the loop (RN-07, AC 5)
Proof: run sourcenets | test two_bidirs_alone_make_the_wire_unknown_and_log_the_loop ... ok

**C46** - A `VSignal` node with one potential driver solves and its readers load it with no flow (RN-08, AC 1)
Proof: run sourcenatures | test a_signal_flow_node_with_one_driver_solves_and_loads_nothing ... ok

**C47** - Two gain blocks over `VSignal` give the product of the gains times the input in `op`, `dc`, `ac`, `transient`, within `DC_VOLTAGE` (RN-08, AC 2)
Proof: run sourcenatures | test a_gain_chain_over_vsignal_gives_the_product_of_the_gains ... ok

**C48** - A signal-flow node with zero or two potential drivers is `E0242` naming the node and the drivers (RN-08, AC 3)
Proof: run elabnatures | test a_signal_flow_node_with_no_or_two_drivers_is_e0242 ... ok

**C49** - A flow contribution to a `VSignal` node is `E0413` (RN-08, AC 4)
Proof: run typesnatures | test a_flow_contribution_to_a_signal_flow_node_is_e0413 ... ok

**C50** - A `VSignal` node on an `Electrical` port solves as conservative, with KCL (RN-08, AC 5)
Proof: run sourcenatures | test vsignal_connected_to_an_electrical_port_is_conservative ... ok

**C51** - `i(n)` or `flow(n)` on a `VSignal` node is `E0206` (RN-08, AC 6)
Proof: run typesnatures | test a_flow_probe_of_a_signal_flow_node_is_e0206 ... ok

**C52** - The alarm prints three records of `Alarm.watch.line` at 0, 1 µs, 3 µs and none at 2 µs (RN-09, AC 1)
Proof: run booknatures | test the_alarm_prints_three_records_of_the_line ... ok

**C53** - The Book's alarm listing is the binary's output (RN-09, AC 2)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C54** - With `Pulldown` the alarm prints the exercise answer's records (RN-09, AC 3)
Proof: run booknatures | test the_pulldown_alarm_prints_the_exercise_answer ... ok

**C55** - `GAPS.md` lists none of L21, L22, L23 nor the resolved-net group of L58 but `TribufDelayed` (RN-10, AC 1)
Proof: run docs | test gaps_lists_no_resolved_net_gap ... ok

**C56** - Appendix C.2.3 and G.38 say `Tran` on `Wire`s runs (RN-10, AC 2)
Proof: run spectext | test c_2_3_and_g_38_say_tran_on_wires_runs ... ok

**C57** - No row of `code_owners.tsv` names a code a source raises, `E0203`, `E0224`, `E0242`, `E0413` included (RN-10, AC 3)
Proof: run speccodes | test no_owned_code_is_raised ... ok


**C58** - The IR builds, checks and prints a resolved net, and refuses a `Resolve` missing a count (RN-03, AC 1, below the language)
Proof: run irresolved | test a_resolved_net_is_built_checked_and_printed ... ok

**C59** - The interpreter refuses `Others`, `Resolve` and a `Resolved` carrier by name (RN-03, AC 1, codegen context R1)
Proof: run interp | test resolved_forms_are_refused_by_name ... ok

**C60** - A resolved carrier's plan has a value pin, a `RELAX` drive pin of the drive layout and an others pin when read (RN-03, AC 1; RN-05, AC 7)
Proof: run planshape | test a_resolved_carrier_has_value_drive_and_others_pins ... ok

**C61** - The generated code calls the instance of the present count (RN-02, AC 7)
Proof: run transresolve | test resolve_calls_the_instance_of_the_present_count ... ok

**C62** - An unsettled relaxation step reaches the owner as one observation (RN-05, AC 4, below the language)
Proof: run prototran | test an_unsettled_loop_reaches_the_owner_once_per_step ... ok

**C63** - The host writes one `log` record per unsettled loop and analysis (RN-05, AC 4, in the host)
Proof: run apirecords | test an_unsettled_loop_is_one_log_record_per_analysis ... ok

**C64** - The Book's PI controller is refused only for its generic `idt` (RN-10, AC 1, phase 1 rows)
Proof: run bookmodules | test the_pi_controller_is_refused_only_for_its_generic_idt ... ok

**C65** - The Book's IRQ line runs on its `Wire` (RN-09, AC 2, the other rows)
Proof: run bookdigital | test the_irq_line_runs_on_its_resolved_wire ... ok

**C66** - The Book's resolved line of the core and stdlib chapter runs (RN-09, AC 2, the other rows)
Proof: run bookcorestd | test the_resolved_line_runs_on_its_wire ... ok

**C67** - The Book's inventory agrees with `GAPS.md` §11 after the rows move (RN-10, AC 1)
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** B1 (T1-T6) done. Defaults taken (user AFK): D16 - `code_owners.tsv` is not on this branch's base (it lives on `feature/params-and-io`), so the rows of `E0224` (T3), `E0413` (T4) and `E0242` (T5) are owed to T36, which deletes them. `NatureRules` carries a fourth field `carries_values` (discrete or resolved carrier versus node) beside the three of design C2, and answers `compatible` (§4 rule 13: a `VSignal` meets an `Electrical` port). `E0242` is reported at the top module's name, naming the node's path and each driver's instance path (the IR keeps origins, not spans). T2 needed no elaborator edit: the carrier's value type already flows from the checker's `Ty::Carrier`. The `types_carriers.rs` pins design "Goldens" names did not exist there; the pins actually replaced are listed in the T1, T3 and T4 commit bodies (`types_subset`, `types_modules`, `types_buck`, `elab_unlowered`, Book `modules#8`, whose test keeps only the `idt` generic-module refusal and is finished by T10). Owed to later tasks: `GAPS.md` L22 and L23, `inventory.tsv` and the Book status boxes (T10).
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → (wait) → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T7 → T8 → T9
Phase 3:  T10
Phase 4:  T11
Phase 5:  T12 → T13 → T14 → T15
Phase 6:  T16 → T17 → T18 → T19 → T20 → T21
Phase 7:  T22 → T23 → T24 → T25 → T26
Phase 8:  T27 → T28 → T29 → T30 → T31 → T32 → T33
Phase 9:  T34 → T35 → T36
```

Execution is strictly sequential.

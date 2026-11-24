# Resolved Nets Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/resolved-nets/design.md`
**Status**: In Progress - B1 and B2 (T1 to T10) done and on `main`; B3 onward (T11 to T39) re-planned 2026-11-22 for the explicit-unknowns model (MD-78) and waiting for the user's approval of this plan and of spec Q2 to Q4
**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13" (B1 and B2 as planned then; the re-plan of B3 onward has no approval yet)
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
**Run sourcenets**: `cargo test -p piperine-parity --test source_nets`
**Run typesgenmod**: `cargo test -p piperine-lang --test types_generic_modules`
**Run elabgenmod**: `cargo test -p piperine-lang --test elab_generic_modules`
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookcorestd**: `cargo test -p piperine --test book_core_std`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run speccodes**: `cargo test -p piperine-lang --test spec_codes`
Buck targets (only in a task whose `Done when` says "with the buck", MD-77): `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test source_commits --test source_records --test buck_supervisor --test source_time --test buck_boundary --test buck_closed_loop --test buck_power`, `cargo test -p piperine-cli --test buck`

Executors: **every task is Sonnet**. Opus verifies each batch and the feature.

**Answers.** The spec's Q1 was answered 2026-11-18 (user: "Sobre as open questions, vou de aceito todas ok?"). The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D20 as the amendment of 2026-11-22 leaves them. The amendment's rules (spec A6.1 to A6.8) are the user's decisions of 2026-11-21, quoted in spec.md. Spec Q2, Q3 and Q4 are open: this plan follows their recommendations (design D21, D25), and the tasks that depend on them say so.

**Base and order (design D2, D20, D25).** B1 and B2 (T1 to T10) and `resolved-nets-bundles` B1 are on `main`. The branch is `feature/resolved-nets`, kept current by merging `main` into it (by hunk; never one side of a whole file; documents and tests keep both intents).

| Batch | Tasks | Starts when | Delivers |
| ----- | ----- | ----------- | -------- |
| B3 | T11 to T15 (phases 4, 5) | `generics` phases A and B and `explicit-unknowns` phase B (B8) are on `main` | the contract re-read; the IR, plan, printer and netlist of a resolved net |
| B4 | T16, T17, T37, T18, T19, T38, T20, T21 (phase 6) | B3 | `impl Resolve` on a nature checked; drives, `others()`, `E0180`, `E0181`, `E0220`, `E0225`; resolvers; initial values; loops |
| B5 | T22 to T26 (phase 7) | B4 | resolved nets run from source on natures a test declares; `E0511` names the drives |
| B6 | T27 to T33 (phase 8) | B5 **and** `generics` phase C's generic modules (GN-13, GN-15) on `main` (spec Q2) | `Strength`, `Driven`, `strongest`, the §22.3 modules, `Bidir`, the edge cases |
| B7 | T34, T35, T39, T36 (phase 9) | B6; T39 also needs `composite-values` phase C (L24) on `main` (spec Q3) | the ten Book rows run; the records |

A batch whose dependency is not on `main` does not start: the feature pauses there and reports. `resolved-nets-bundles` T15 waits for T22 here.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-a TMPDIR=/var/tmp/piperine-tmp-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command; never a target directory another worktree is building in. Cargo with `-p <crate>`, or plain at the root only at the end of a batch; never `--workspace` (`piperine-lang-server` is postponed and known broken), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report. Never `git stash` (the stash list is shared between worktrees).
- **Gate policy (user, 2026-11-21: "Quero que aplique o dos gates mais leves apenas"; 2026-11-22: "Suite completa é só no fim dos lotes e ponto final!").** Per task: the `**Build**` line and the tests of the crates the task touched, nothing more. The full root suite, the integrated suites, the parity targets and the twins run once, at the end of each batch (the batch's last task carries `Gate: batch`), and never in a fix round or a verification, which run the targets their change touches. The buck runs only in a task whose `Done when` says "with the buck".
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. A new comparison uses an existing contract as it is (D12).
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found", both tables). A spec text that is wrong is amended explicitly with its `spec_text` guard (MD-57), never left. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites (§3.7, §4, §5.2, §6, §8.2.3, §22.3, §22.4.3, Appendix F.4), or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table of design.md. Every model a test runs declares its own net nature with its `impl Resolve` (design D27): no test names `Wire`, `Tri`, `SumNet` or `Logic::wired`.
- **Proof lines are real lines.** A check's expected line is confirmed against a real run in the task that first makes it pass; a line that differs from what the runner prints is corrected in that task's commit, never the test renamed to fit.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. A removal removes the whole concept, with no dead field, no unused variant and no comment about what used to be there. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR enum variants, `CarrierRole` and plan `Source` variants are appended last. The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/natures/*.msl`, `tests/resolved/*.msl` and `tests/nets/*.msl`.
- **The Book** (21 chapters, Part 1 Fundamentals and Part 2 In depth): its prose never defers to Rust, its tone is respectful, every listing has a row in `tests/book/inventory.tsv` and a test. When a refused row starts to run its test fails with "now runs": the row becomes `run`, its `GAPS.md` §11 line is deleted, and a sentence that described the refusal is rewritten to what the binary prints.
- **Code owners (D16).** A task that first raises `E0203` or `E0225` deletes the code's row from `crates/piperine-lang/tests/support/code_owners.tsv` and the code from the `GAPS.md` §10 list in its commit.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given (lowercase conventional subject of at most 72 characters), ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No merge; push only as the orchestrator asks.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-72 (root Book tests), MD-77 (the buck runs on demand).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every static AC; each code with its span and the names it cites | `crates/piperine-lang/tests/types_natures.rs` | `cargo test -p piperine-lang --test types_natures` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; `E0225`, `E0242`, `E0339` across instances, `E0187` of a drive; the instances of `resolve` | `crates/piperine-lang/tests/elab_natures.rs` | `cargo test -p piperine-lang --test elab_natures` |
| IR (`piperine-ir`) | integration | the new forms built, checked, printed | `crates/piperine-ir/tests/resolved.rs` | `cargo test -p piperine-ir --test resolved` |
| Interpreter refusals | integration | each new IR form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | the value, drive and others pins; the drive layout; the resolver's binding | `crates/piperine-plan/tests/digital_shape.rs` | `cargo test -p piperine-plan --test digital_shape` |
| Printer (`piperine-transpile`) | integration | `Expr::Resolve` compiled and called per count | `crates/piperine-transpile/tests/resolve.rs` | `cargo test -p piperine-transpile --test resolve` |
| Digital solver | integration | relaxation with absent and present drives, the pass bound, no latch | `crates/piperine-digital-solver/tests/relax.rs` | `cargo test -p piperine-digital-solver --test relax` |
| Owner protocol | integration | the loop that does not settle reaches the owner as one `E0511` fault | `crates/piperine-solver/tests/protocol_transient.rs` | `cargo test -p piperine-solver --test protocol_transient` |
| Source-level runs | integration | one test per run-time AC and edge case, from MSL source on the Rust backend; expected values from the rule; the host's `E0511` message | `crates/piperine-parity/tests/{source_natures,source_resolved,source_nets}.rs` | `cargo test -p piperine-parity --test source_natures` / `source_resolved` / `source_nets` |
| The Book | integration | each of the ten rows, through the binary and the root package | `tests/book/*.rs`, `tests/book/inventory.tsv`, `crates/piperine-cli/tests/book.rs` | `cargo test -p piperine --test book_<chapter>`, `cargo test -p piperine-cli --test book` |
| Spec text and repository records | integration | one assertion per amended sentence, removed `GAPS.md` row or code owner row | `crates/piperine-lang/tests/{spec_text,spec_codes}.rs`, `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-lang --test spec_text`, `--test spec_codes`, `cargo test -p piperine-cli --test docs` |

## Gate Check Commands

> Generated from codebase - confirm before Execute. The levels follow the user's gate policy (design D28): nothing heavier than `batch` runs anywhere but a batch's last task.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task whose tests are one or two targets of one crate | the `**Build**` line, then the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the checker, the elaborator, the IR, the plan, the printer, an engine, the host or `std` | Quick, then `cargo test -p <crate>` for every crate the task touched (for the root package, only the `book_*` targets it touched, with `book_inventory` and `book_claims`); "with the buck" adds the buck targets |
| Batch | The last task of each batch (T15, T21, T26, T33, T36), once | Full, then `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, `cargo test -p piperine --test integrated`, `cargo test -p piperine-cli --test integrated`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` |
| Build | After a task with `Tests: none` | the `**Build**` line |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

Batches (whole phases): **B1** phase 1 (T1–T6, done) · **B2** phases 2–3 (T7–T10, done) · **B3** phases 4–5 (T11–T15) · **B4** phase 6 (T16, T17, T37, T18, T19, T38, T20, T21) · **B5** phase 7 (T22–T26) · **B6** phase 8 (T27–T33) · **B7** phase 9 (T34, T35, T39, T36). See "Base and order" for when each starts.

### Phase 1: Project discrete natures, signal-flow nodes and `E0224` (done)

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Relaxation below the language (done)

```
T7 → T8 → T9
```

### Phase 3: Records of phases 1 and 2 (done)

```
T10
```

### Phase 4: Contract (waits for `generics` A–B and `explicit-unknowns` B on `main`)

```
T11
```

### Phase 5: IR, plan and printer for resolved carriers

```
T12 → T13 → T14 → T15
```

### Phase 6: Checker and elaborator for natures that implement `Resolve`

```
T16 → T17 → T37 → T18 → T19 → T38 → T20 → T21
```

### Phase 7: Resolved carriers run from source

```
T22 → T23 → T24 → T25 → T26
```

### Phase 8: The library nets of §22.3 and `Bidir` (waits for `generics` C's generic modules, spec Q2)

```
T27 → T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 9: The Book and the records

```
T34 → T35 → T39 → T36
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

- [x] `gaps_no_longer_lists_project_or_signal_flow_natures` passes under `**Run docs**`
- [x] `the_pi_controller_is_refused_only_for_its_generic_idt` passes under `**Run bookmodules**`; `**Run bookinventory**` passes
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs(gaps): project and signal-flow natures run`

---

### T11: Contract: merge `main` and re-read the anchors

**What**: Merge `main` into `feature/resolved-nets` (by hunk) once `generics` A–B and `explicit-unknowns` B are on it; re-read every anchor of design.md and its amendment: `types/carrier.rs`, `types/call.rs`, `NatureRules`, trait `impl`s on a nature and the lookup order (`generics` T26, T27, T46, T48 as `explicit-unknowns` D17 amended them), `Instantiations` of a trait function, `std/nets.msl` (`Strength`, `Driven`, `strongest`, `sum`, the generic modules, no net nature: `explicit-unknowns` T23) and `std/logic.msl` (its T44), the drive pin layout and the `E0511` fault (its T36, T37), `E0187` (its T30), and whether `pom_bundles.rs` still uses the library's `Wire`. Re-point the checks of B2 that `explicit-unknowns` T37 touched: in `relax.rs`, `an_absent_drive_is_left_out_and_an_unknown_one_is_passed` becomes `an_absent_drive_is_left_out_and_a_present_one_is_passed` (a present drive holds a value of its type; no drive is unknown), and `protocol_transient.rs`'s `an_unsettled_loop_reaches_the_owner_once_per_step` becomes `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault`; where T37 already replaced either, C31 and C62 name its successor instead. Write on the Handoff each anchor that moved and how the following tasks adapt. Stop and report if a dependency is missing (D25).
**Where**: `crates/piperine-digital-solver/tests/relax.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md and its amendment; `explicit-unknowns` design "`resolved-nets`"
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] The merge is clean or its conflicts are resolved by hunk without changing behaviour; the `**Build**` line passes
- [x] `an_absent_drive_is_left_out_and_a_present_one_is_passed` passes under `**Run relax**`; `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault` passes under `**Run prototran**` (or C31 and C62 name the successors T37 left, the commit body names both)
- [x] The Handoff names every moved anchor
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): resolved-nets contract on the explicit model`

---

### T12: IR: resolved carriers, `Others`, `Resolve`, the resolution of a net

**What**: Design C4 and the amendment's data model: `CarrierRole::Resolved { driven }`, `Expr::Others(u32)`, `Expr::Resolve { by_count, drives, except }`, `NetData::resolution: Option<Resolution>` with its `driven` type, appended; `Ir::check` (counts; each instance takes `[X; k]` and returns `V`; `Others` on a `Resolved` carrier, of type `V`); the structural printer; the interpreter refuses the three forms by name. Create `crates/piperine-ir/tests/resolved.rs`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `IrBuilder`, `check.rs`, `print.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `a_resolved_net_is_built_checked_and_printed` and `a_resolve_with_a_missing_count_or_another_type_is_refused` pass under `**Run irresolved**`
- [x] `resolved_forms_are_refused_by_name` passes under `**Run interp**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): resolved carriers and their nets`

---

### T13: Plan: value, drive and others pins

**What**: Design C5 as amended: `DriveLayout::of(driven)`, the fields of `X` then a plain `driving: BOOL`; a `Resolved` carrier's value pin of `V` (read), drive pin of the drive layout (`RELAX`) and others pin of `V` (only when `Expr::Others` reads it); a definition or commit of it targets the drive pin.
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: T12
**Reuses**: `CarrierLayout`, `ShapeBuf`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `a_resolved_carrier_has_value_drive_and_others_pins` and `the_drive_layout_appends_a_plain_driving_byte` pass under `**Run planshape**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): pins of a resolved carrier`

---

### T14: Printer: `Expr::Resolve` and the drive write

**What**: Design C6 as amended: `ResolvePrinter` gathers the present drives (`driving` 1) in order into `[X; k]` and calls `by_count[k]`; a write to a drive pin sets `driving` to 1. Create `crates/piperine-transpile/tests/resolve.rs` (an IR built by hand with a `sum` over `[f64; k]` and a resolution whose `X` differs from `V`).
**Where**: `crates/piperine-transpile/src/print/resolve.rs`
**Depends on**: T13
**Reuses**: `print/digital.rs`, `print/value.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `resolve_calls_the_instance_of_the_present_count` and `a_drive_write_marks_the_drive_present` pass under `**Run transresolve**`
- [x] Transpile goldens byte-identical
- [x] Gate check passes: Full

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

- [x] `a_resolved_net_binds_drives_value_and_others` passes under `**Run planshape**`
- [x] Gate check passes: Batch (end of B3), with the buck

**Tests**: integration
**Gate**: batch

**Commit**: `feat(plan): the resolver binds its nets`

---

### T16: Checker: natures that implement `Resolve`

**What**: Design C3 as amended, D15, D23. Remove the `resolve` field: `NatureFieldKind::Resolve` and its parser arm, `NatureFieldNode::Resolve`, `Nature::resolve_fn`, and the kind `Resolved` that the field gave, with the statements of `docs/pom/reflection.md`, `model.md`, `mapping.md` and `resolution.md` that describe the field and their `pom_docs::DocAheadOfTheCode::WAITING` entries (explicit-unknowns F14); a `resolve:` line is then the parser's error for any word that is not a nature field. `ResolveImpls::check` at each `impl Resolve<X, V> for N`: `N` discrete, `V` its `value`, `resolve` pure with one const parameter `N: usize`, one parameter `[X; N]`, result `V`; anything else `E0203` naming nature and `impl`. `NatureRules::of` answers "resolved" from the nature's `impl`s; `port_type`'s resolved arm gives `Ty::Carrier`; `E0310` for `in`/`out` on a resolved port; `E0220` skipped for resolved carriers; `resolved-nets-bundles`' `E0261` (`pom/resolve/shape.rs`) asks the same capability. `pom_bundles.rs` fixtures that still use the library's `Wire` declare a nature with `impl Resolve<bool>` (design "Goldens"). Delete the `E0203` row of `code_owners.tsv` and of `GAPS.md` §10 (D16).
**Where**: `crates/piperine-lang/src/types/nature.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `NatureRules`, `generics`' trait `impl` checks
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [x] `an_impl_resolve_makes_a_discrete_nature_resolved`, `resolve_may_call_a_library_or_project_function`, `an_impl_resolve_of_another_shape_is_e0203`, `a_resolve_field_is_not_a_nature_field` and `a_direction_on_a_resolved_port_is_e0310` pass under `**Run typesnatures**`
- [x] `cargo test -p piperine-lang --test pom_bundles --test pom_docs` passes with the `Wire` fixtures replaced (both names in the commit body)
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): a nature is resolved by its impl of resolve`

---

### T17: Checker: `others()`, `E0221`, `E0220` and loops of resolved drives

**What**: `s.others()` on a resolved carrier has the nature's `value` type `V`, in definitions, reactions and `analog`; one module holding a definition and a commit of one resolved carrier is `E0221`; a second drive of a carrier whose nature does not implement `Resolve` is `E0220` whose diagnostic names the nature and `Resolve` (§5.2.1); `E0339` counts a module-local loop only when it passes through a one-driver carrier (§8.2.3).
**Where**: `crates/piperine-lang/src/types/definition.rs`
**Depends on**: T16
**Reuses**: the loop walk of `definition.rs`, `types/call.rs`, `types/module.rs` (`E0220`)
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221`, `a_module_local_loop_of_resolved_drives_is_accepted` and `a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve` pass under `**Run typesnatures**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): others, e0220 and loops of resolved drives`

---

### T37: Checker: a drive picks its `impl` (`E0180`, `E0181`)

**What**: `ResolveImpls::pick`: the type of a definition or commit to a resolved carrier picks the nature's `impl Resolve<X, V>` whose `X` it is (§5.2.15); a type no `impl` accepts is `E0180` naming the nature and the driven type (a `Logic` into `AndWire`); a literal several `impl`s accept is `E0181`. The drive's type is recorded for the elaborator.
**Where**: `crates/piperine-lang/src/types/nature.rs`
**Depends on**: T17
**Reuses**: `ResolveImpls`, the literal inference of `types/`
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [x] `a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181` and `a_drive_picks_the_impl_of_its_type` pass under `**Run typesnatures**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a drive picks its impl of resolve`

---

### T18: Elaborator: a body's resolved carriers, drives and `others()`

**What**: A resolved port or signal is a carrier of role `Resolved { driven }`; the body's definition or commits of it are its drive, of type `X`; `s.others()` lowers to `Expr::Others`.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T37
**Reuses**: `Structure`, `Frame`, `elab/drives.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `a_resolved_port_lowers_to_a_resolved_carrier_and_its_drive` and `others_lowers_to_the_bodys_others_read` pass under `**Run elabnatures**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): resolved carriers of a body`

---

### T19: Elaborator: `ResolverBodies` and the instances of `resolve`

**What**: Design C7, D4, D5, D8, D21: after placement, per resolved net, `DriveOrder::of` sorts the holders (§4 rule 2 Resolution, host first), the drive and others nets are added, the net's `impl` is the one its drives picked (for a net nobody drives, and for the host's drive, D21: the only `impl`, else `impl Resolve<V>`, else `E0181`; spec Q4), its `resolve::<k>` is instantiated for `k` in `0..=N` through `Instantiations`, one resolver body per (nature, `impl`, `N`, others readers) and one resolver instance `<net path>$resolve` are placed; `NetData::resolution` is filled.
**Where**: `crates/piperine-lang/src/elab/resolvers.rs`
**Depends on**: T18
**Reuses**: `Instantiations`, `IrBuilder`, `elab/place.rs`
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [x] `resolve_is_instantiated_once_per_drive_count`, `drives_are_ordered_by_instance_path_segment_by_segment` and `others_depends_on_the_other_drives_only` pass under `**Run elabnatures**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): resolvers of resolved nets`

---

### T38: Elaborator: one driven type per net (`E0225`)

**What**: Design D22: in `ResolverBodies`, two drives of one net whose driven types differ are `E0225` naming both holders' instance paths and both types (§5.2.15). Delete the `E0225` row of `code_owners.tsv` (owner `explicit-unknowns`) and the code from the `GAPS.md` §10 list (D16).
**Where**: `crates/piperine-lang/src/elab/resolvers.rs`
**Depends on**: T19
**Reuses**: the drive types T37 records
**Requirement**: RN-02
**Executor**: Sonnet

**Done when**:

- [x] `two_driven_types_on_one_net_are_e0225` passes under `**Run elabnatures**`
- [x] `no_owned_code_is_raised` passes under `**Run speccodes**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): one driven type per resolved net`

---

### T20: Elaborator: initial values of resolved carriers

**What**: §5.2.3 and §5.2.12 as amended: `= v` on a resolved signal or port is the declaring module's drive's first commit at `(0, 0)`, of type `X`; a resolved carrier with no drive of the declaring module needs no initial value; a drive with no initial value starts at `Default::default()` of `X`, and a type without `Default` whose driver gives none is `E0187` naming the carrier and `Default` (an undelayed definition gives one, explicit-unknowns Handoff).
**Where**: `crates/piperine-lang/src/elab/place.rs`
**Depends on**: T38
**Reuses**: the initial-value path of one-driver carriers, `explicit-unknowns`' `E0187`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `an_initial_value_is_the_declaring_modules_first_drive_commit` and `a_resolved_drive_without_default_or_initial_value_is_e0187` pass under `**Run elabnatures**`
- [x] Gate check passes: Quick

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

- [x] `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` and `a_loop_of_resolved_drives_across_instances_is_accepted` pass under `**Run elabnatures**`
- [x] Gate check passes: Batch (end of B4), with the buck

**Tests**: integration
**Gate**: batch

**Commit**: `feat(elab): loops across instances`

---

### T22: Drives combine through `resolve`, from source

**What**: Create `crates/piperine-parity/tests/source_resolved.rs` and `tests/resolved/*.msl`, each model declaring its nature (D27): three drivers of `Total` (`impl Resolve<f64> for Total` with `sum`) in instances named out of order; a project `resolve` that records its argument order; an undriven `Total` and an undriven `AndWire` (F.4.1); a drive change that leaves the value; an initial value; a drive with none, of a type with `Default` (`Option<u8>` into a nature of `impl Resolve<Option<u8>, u8>`).
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the runner of `source_natures.rs`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `three_summing_drivers_give_their_sum_in_the_microstep_any_changes`, `an_undriven_resolved_carrier_holds_resolve_of_no_drive`, `resolve_receives_the_drives_in_instance_path_order`, `a_drive_without_initial_value_is_its_types_default_until_its_first_commit`, `a_drive_change_that_leaves_the_value_fires_no_changed` and `an_initial_value_is_the_declaring_modules_first_drive` pass under `**Run sourceresolved**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): resolved drives from source`

---

### T23: `others()` from source

**What**: Three drivers of `Total` each emitting its `others()`; a non-driver reading `others()`.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T22
**Reuses**: T22's models
**Requirement**: RN-04
**Executor**: Sonnet

**Done when**:

- [x] `others_is_resolve_of_every_drive_but_the_readers` and `others_of_a_module_without_a_drive_is_the_value` pass under `**Run sourceresolved**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): others from source`

---

### T24: Relaxation from source

**What**: A ring of pass elements over `AndWire` (each drives what the rest of its net drives, through `others()`) settles; a released ring keeps no level.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T23
**Reuses**: T22's models
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] `a_loop_of_resolved_drives_is_accepted_and_settles` and `a_released_ring_latches_nothing` pass under `**Run sourceresolved**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): relaxation from source`

---

### T25: Host: `E0511` names the loop's drives

**What**: Design C9 as amended, D24: `LoopDrives` in the host's failure path maps the drive nets of the `E0511` fault (`explicit-unknowns` T37) to the instance paths that hold them through `NetData::resolution`, in drive order, the host's drive as `host`, and writes them in the message. Source tests: two drivers of `AndWire` that each drive the negation of `others()` stop the run with `E0511` naming both; a loop settling on its first pass raises nothing.
**Where**: `crates/piperine-api/src/failure.rs`
**Depends on**: T24
**Reuses**: `Failure`, `NetData::resolution`
**Requirement**: RN-05
**Executor**: Sonnet

**Done when**:

- [x] `a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives` and `a_loop_settling_on_its_first_pass_raises_nothing` pass under `**Run sourceresolved**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): e0511 names the drives of the loop`

---

### T26: Resolved nets in every analysis

**What**: The per-analysis table of §22.3 over `Total` and a module with a reaction: definitions, `resolve`, `others()` and a reaction run at the microsteps of `(0, ·)` and of each `dc` sweep point; in `ac` the nets are constant.
**Where**: `crates/piperine-parity/tests/source_resolved.rs`
**Depends on**: T25
**Reuses**: the analyses of `piperine-api`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` passes under `**Run sourceresolved**`
- [x] Gate check passes: Batch (end of B5)

**Tests**: integration
**Gate**: batch

**Commit**: `test(parity): resolved nets in every analysis`

---

### T27: `Strength`, `Driven`, `Driven::strongest`

**What**: Waits for `generics` phase C's generic modules on `main` (spec Q2, D25); merge `main` first and stop if GN-13 is missing. Create `crates/piperine-parity/tests/source_nets.rs` and `tests/nets/*.msl`: a nature `Line { value: Driven }` with `impl Resolve<Driven> for Line { Driven::strongest }` driven by timed commits gives the table of §22.3.2; `strongest` of any permutation is one result and `strongest([x, x])` is `x`. Fix in the compiler any form of `std/nets.msl` that does not check or run (CLAUDE.md: the library is an acceptance target).
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: T22's runner; `std/nets.msl`, `std/logic.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `strongest_gives_the_table_of_22_3_2` and `strongest_of_equal_drives_is_that_drive_in_any_order` pass under `**Run sourcenets**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): strengths and strongest`

---

### T28: A nature with two `impl`s, and a typed bus

**What**: Design D19 as amended, spec RN-02 AC 9, RN-06 AC 3: a nature `Bus { value: bool }` with `impl Resolve<bool> for Bus` (high when every drive is) and `impl Resolve<u8, bool> for Bus` (high when every drive is non-zero) resolves each carrier by its drives' type (the resolver body's key carries `X`); `DataBus` with `impl Resolve<Option<u8>, Data>` (Appendix F.4.4) gives `Free`, `Byte(b)`, `Contended`.
**Where**: `crates/piperine-lang/src/elab/resolvers.rs`
**Depends on**: T27
**Reuses**: `ResolverBodies`, `ResolveImpls`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type` and `a_typed_bus_resolves_to_free_byte_or_contended` pass under `**Run sourcenets**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a resolved nature with several impls`

---

### T29a: A generic module over a resolved nature parameter (`W: Nature + Resolve<Driven>`)

**What**: Added 2026-11-24 by the coordinator's ruling: the library of §22.3 (`Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir`) is generic over a *discrete* nature, which T27 found the checker could not express (a port typed by a nature parameter was always a node; `Resolve<Driven>` as a bound was `E0161`). `Bound::Resolve { arguments }` in `GenericSignature::bound`; a nature parameter with it is a resolved carrier in the generic body (`Ty::Carrier { nature: <the parameter>, value: V }`, `V` the second argument or `X`; drives typed `X`; `others()` typed `V`; `NatureRules` says resolved, `ResolveImpls::of` answers from the bound); `satisfies` asks `ResolveImpls::of` of the argument nature (`E0187` naming the argument and the bound); `Substitution::apply` replaces the carrier's nature; the placement's inference learns the nature from a carrier meeting a carrier of a parameter; the elaborator reads the instance's carrier (`typed_carrier`) and picks the concrete `impl` by `X` (`concrete_implementation`). A bound with no, or three, arguments or with no `Nature` is `E0187`.
**Where**: `crates/piperine-lang/src/types/` (`generics.rs`, `bounds.rs`, `carrier.rs`, `nature.rs`, `nature_rules.rs`, `substitution.rs`, `inference.rs`, `module_generics.rs`), `crates/piperine-lang/src/elab/carriers.rs`, `body.rs`
**Depends on**: T28
**Reuses**: `ResolveImpls`, `NatureRules`, the generic module instances of `generics` C
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body`, `a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound` and `a_resolve_bound_takes_one_or_two_arguments_and_a_nature` pass under `**Run typesgenmod**`
- [x] `a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature` passes under `**Run elabgenmod**`
- [x] `a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature` passes under `**Run sourcenets**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): a generic module over a resolved nature parameter`

---

### T29: `Tribuf`, `Pullup`, `Pulldown`

**What**: Source tests of §22.3.5 on `Line`: `Tribuf` with `oe` high and low; a pull that wins only while every stronger drive is released, for `Pullup` and `Pulldown`. A library module placed on a nature without `impl Resolve<Driven>` (`AndWire`) is `E0187` naming the nature and the bound.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T28
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `tribuf_drives_strong_or_high_impedance` and `a_pull_wins_only_while_every_stronger_drive_is_released` pass under `**Run sourcenets**`
- [x] `a_library_module_on_a_nature_without_resolve_driven_is_e0187` passes under `**Run typesnatures**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): tribuf and pulls`

---

### T30: `Keeper`

**What**: Source test of §22.3.6 on `Line`: weak X before any 0 or 1 has been seen; holds the last level the others drove, weakly, after they release; a contended X is not recorded. Amend the fence of §22.3.6 and `std/nets.msl` to `digital<W> for Keeper<W>` (§10 rule 6, D26), with a `spec_text` guard.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T29
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it` passes under `**Run sourcenets**`
- [x] `keeper_and_bidir_blocks_repeat_their_generic_names` passes under `**Run spectext**` for `Keeper`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): keeper`

---

### T31: `Tran`, `Tgate` and the conformance item of Appendix C.2.3

**What**: Source tests of §22.3.7, §22.3.8 and RN-05 AC 3 on `Line`: `Tran` with `en` high and low; `Tgate` conducting on `s` high or `sn` low; two parallel `Tran`s carry a strong level and stay two carriers with their own `changed()`; a ring of `Tran`s settles.
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T30
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-06
**Executor**: Sonnet

**Done when**:

- [x] `tran_drives_each_side_with_the_others_of_the_other`, `tgate_joins_while_s_high_or_sn_low` and `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` pass under `**Run sourcenets**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): tran and tgate`

---

### T32: `Bidir`

**What**: A transient with a `Tribuf`, a `Bidir` and an RC load on its node toggling `oe` (§22.4.3 rows 1 and 2, rules 1 to 3); a contended net (X at `Strong`) makes the pad act as driven toward mid-rail; two `Bidir`s alone on one net stop the run with `E0511` naming both (rule 4). Amend the fences of §22.4.3 and `std/convert.msl` to `analog<W> for Bidir<W>` and `digital<W> for Bidir<W>` (D26).
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T31
**Reuses**: `tests/nets/*.msl`, T25's message
**Requirement**: RN-07
**Executor**: Sonnet

**Done when**:

- [x] `bidir_drives_the_node_while_the_wire_is_driven`, `bidir_drives_the_wire_from_the_node_when_released`, `bidir_acts_as_driven_on_a_contended_net`, `a_change_of_driving_is_a_time_point_and_a_jump` and `two_bidirs_alone_stop_the_run_with_e0511` pass under `**Run sourcenets**`
- [x] `keeper_and_bidir_blocks_repeat_their_generic_names` passes under `**Run spectext**` for both modules
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): bidir`

---

### T33: Edge cases of resolved nets

**What**: The spec's edge cases: a top-level resolved port's host drive comes first (D8, with the `impl` of D21); a panic inside `resolve` is a run fault naming the nature and the carrier (D13); a drive at `Strength::HighZ` of any level is left out; one drive still goes through `resolve` (RN-03 AC 7); a parent drives a child's `pub` resolved signal; a net nobody drives on a nature with two `impl`s follows D21 (spec Q4).
**Where**: `crates/piperine-parity/tests/source_nets.rs`
**Depends on**: T32
**Reuses**: `tests/nets/*.msl`
**Requirement**: RN-03
**Executor**: Sonnet

**Done when**:

- [x] `the_hosts_drive_of_a_top_port_comes_first`, `a_panic_in_resolve_is_a_run_fault_naming_nature_and_carrier`, `a_high_impedance_drive_of_any_level_is_left_out`, `one_drive_still_goes_through_resolve` and `an_undriven_net_of_a_nature_with_two_impls_follows_its_rule` pass under `**Run sourcenets**`
- [x] `a_parent_drives_a_childs_pub_resolved_signal` passes under `**Run sourcenets**` (follow-up commit: the checker accepts a resolved child field as a drive target, the elaborator makes a `Resolved`-role reach carrier that holds a drive of the child's net)
- [x] Gate check passes: Batch (end of B6), without the buck (user's gate policy)

**Tests**: integration
**Gate**: batch

**Commit**: `test(parity): edge cases of resolved nets`

---

### T34: The Book's alarm runs (natures#3, natures~e99c1920, natures~91c3bffe)

**What**: The chapter "Natures and carriers": the alarm's CLI listing becomes the binary's output; the sentence natures~e99c1920 ("Today the command refuses the file and runs nothing:") is rewritten to introduce the output (prose respectful, never deferring to Rust); `tests/book/natures.rs` replaces `the_open_drain_line_is_refused_until_a_nature_can_implement_resolve` by run tests; rows natures#3 and natures~91c3bffe move to `run`/`holds` in `inventory.tsv` and their `GAPS.md` §11 lines go; `crates/piperine-cli/tests/book.rs` holds the listing.
**Where**: `docs/book/content.html`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Chapter`, `Refusal`, `inventory_guard.rs`
**Requirement**: RN-09
**Executor**: Sonnet

**Done when**:

- [x] `the_alarm_prints_three_records_of_the_line`, `the_pulldown_alarm_prints_the_exercise_answer` and `the_book_no_longer_says_the_alarm_is_refused` pass under `**Run booknatures**`
- [x] `every_cli_listing_of_the_book_is_what_the_binary_prints` passes under `**Run clibook**`; `**Run bookinventory**` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs(book): the alarm line runs`

---

### T35: The IRQ line, the four worked nets and the library's line (digital#16 to digital#20, core-std#6)

**What**: `tests/book/digital.rs` replaces `the_irq_line_and_the_four_nets_are_refused_until_a_nature_can_implement_resolve` by run tests: digital#16 (`IrqSource`, `IrqController`, `Board` over `IrqLine`) gives the outcomes of Appendix E.7; digital#17 to digital#20 give the tables of Appendix F.4.1 to F.4.4. `tests/book/core_std.rs` replaces `the_resolved_line_is_refused_until_a_nature_can_implement_resolve` by a run of core-std#6. The six rows move to `run` in `inventory.tsv`; their `GAPS.md` §11 lines go; any sentence of "Driving with strength" that described the refusal is rewritten to what the binary prints.
**Amendment (main, 2026-11-23):** natures#2 is judged by elaboration, not by an operating point (gap L21): its nodes `mid` and `vout` float, so the operating point is singular (`E0503`) whatever this feature does; T39's flip is that the block elaborates with `Sda` declared.
**Where**: `tests/book/digital.rs`
**Depends on**: T34
**Reuses**: `Chapter`, `Commands`, Appendix E.7 and F.4 tables
**Requirement**: RN-09
**Executor**: Sonnet

**Done when**:

- [x] `the_irq_line_runs_on_its_resolved_nature` and `the_four_worked_nets_give_the_tables_of_f_4` pass under `**Run bookdigital**`; `the_resolved_line_runs_on_its_nature` passes under `**Run bookcorestd**`; `**Run bookinventory**` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs(book): the irq line and the worked nets run`

---

### T39: The signal families of chapter 5 (natures#2)

**What**: Waits for `composite-values` phase C (L24) on `main` (spec Q3); merge `main` first and stop if L24 is open. `tests/book/natures.rs` replaces `signals_of_every_family_are_refused_for_the_arrays_the_option_the_logic_and_the_net` by a test that elaborates the block (with the chapter's `Sda`) and reads each carrier's initial value: `en` false, `last` `None`, `count` 0, `level` 0.0, `st` `Tx::Idle`, `lines` eight `false`, `bus` `10xx`, `sda` `Driven::Z` (no drive). natures#2 moves to `run`; its `GAPS.md` §11 line goes.
**Where**: `tests/book/natures.rs`
**Depends on**: T35
**Reuses**: `Chapter`, `Model`
**Requirement**: RN-09
**Executor**: Sonnet

**Done when**:

- [ ] `signals_of_every_family_run` passes under `**Run booknatures**`; `**Run bookinventory**` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs(book): the signal families run`

---

### T36: Records: GAPS, spec, roadmap, decisions, code owners

**What**: `GAPS.md` loses L21 and the resolved-net group of L58 (`TribufDelayed` stays with `generics`), with §9 and the counts; the §11 list holds no net row (T34, T35, T39 removed them); Appendix C.2.3 and G.38 say `Tran` on nets of a `Resolve<Driven>` nature runs, guarded in `spec_text.rs`; if Appendix E.7 and F.4 are still `rust,fragment` (explicit-unknowns Handoff), they become `rust` and `std_galleries` builds 18 fences; `ROADMAP.md` names the feature delivered; `.specs/STATE.md` gains the next free `MD-` entry (resolved nets below the POM: D4 to D8, D21 to D24); every `code_owners.tsv` row this feature raises is gone.
**Where**: `GAPS.md`
**Depends on**: T39
**Reuses**: `docs.rs`, `spec_text.rs`, `spec_codes.rs`
**Requirement**: RN-10
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_lists_no_resolved_net_gap` and `gaps_section_11_holds_no_net_row` pass under `**Run docs**`; `c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs` passes under `**Run spectext**`; `no_owned_code_is_raised` passes under `**Run speccodes**`
- [ ] Gate check passes: Batch (end of B7)

**Tests**: integration
**Gate**: batch

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

**C8** - A discrete nature with `impl Resolve<X, V>` whose `resolve` is pure `fn<const N: usize>([X; N]) -> V` is resolved; `impl Resolve<X>` means `V = X` (RN-02, AC 1)
Proof: run typesnatures | test an_impl_resolve_makes_a_discrete_nature_resolved ... ok

**C9** - A `resolve` body that calls `Driven::strongest` or `sum` is accepted (RN-02, AC 2)
Proof: run typesnatures | test resolve_may_call_a_library_or_project_function ... ok

**C10** - An `impl Resolve` whose `V` is not the nature's `value`, on a continuous nature, or with no const parameter, a non-array parameter, another result or an impure class is `E0203` naming nature and `impl` (RN-02, AC 3)
Proof: run typesnatures | test an_impl_resolve_of_another_shape_is_e0203 ... ok

**C11** - A nature field named `resolve` is refused as any non-field word is (RN-02, AC 4)
Proof: run typesnatures | test a_resolve_field_is_not_a_nature_field ... ok

**C12** - `in` or `out` on a resolved port is `E0310` (RN-02, AC 5)
Proof: run typesnatures | test a_direction_on_a_resolved_port_is_e0310 ... ok

**C13** - `signal s: Total = 2.0;` makes the declaring module's drive commit `2.0` at `(0, 0)`, and a resolved signal without `= v` needs none (RN-02, AC 6)
Proof: run sourceresolved | test an_initial_value_is_the_declaring_modules_first_drive ... ok

**C14** - A carrier with `N` drives instantiates its `impl`'s `resolve::<k>` for exactly `k = 0..=N` (RN-02, AC 7)
Proof: run elabnatures | test resolve_is_instantiated_once_per_drive_count ... ok

**C15** - Three drivers' sum is the carrier's value in the microstep any of them changes (RN-03, AC 1)
Proof: run sourceresolved | test three_summing_drivers_give_their_sum_in_the_microstep_any_changes ... ok

**C16** - An undriven carrier holds `resolve` of the empty array: `0.0` for `Total`, `true` for `AndWire` (RN-03, AC 2)
Proof: run sourceresolved | test an_undriven_resolved_carrier_holds_resolve_of_no_drive ... ok

**C17** - `resolve` receives the drives in segment-wise instance-path order, host first, whatever the evaluation order (RN-03, AC 3)
Proof: run sourceresolved | test resolve_receives_the_drives_in_instance_path_order ... ok

**C18** - A drive without initial value is passed as `Default::default()` of its type from `(0, 0)` until its first commit (RN-03, AC 4)
Proof: run sourceresolved | test a_drive_without_initial_value_is_its_types_default_until_its_first_commit ... ok

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

**C28** - A loop still moving after its last pass stops the run with `E0511` naming every drive by its holder's instance path, in drive order (RN-05, AC 4)
Proof: run sourceresolved | test a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives ... ok

**C29** - A loop through a one-driver carrier across instances is `E0339` (RN-05, AC 5)
Proof: run elabnatures | test a_loop_through_a_one_driver_carrier_across_instances_is_e0339 ... ok

**C30** - A released loop keeps no level of an earlier microstep (RN-05, AC 6)
Proof: run relax | test a_released_loop_keeps_no_earlier_level ... ok

**C31** - In a relaxation pass `resolve` gets only the present drives, in order; a present drive holds a value of its type and is passed (RN-05, AC 7)
Proof: run relax | test an_absent_drive_is_left_out_and_a_present_one_is_passed ... ok

**C32** - A nature resolved by `Driven::strongest` gives the four rows of §22.3.2 (RN-06, AC 1)
Proof: run sourcenets | test strongest_gives_the_table_of_22_3_2 ... ok

**C33** - `strongest([x, x])` is `x`, and every permutation of drives gives one result (RN-06, AC 2)
Proof: run sourcenets | test strongest_of_equal_drives_is_that_drive_in_any_order ... ok

**C34** - `DataBus` gives `Free` for no drive or only `None`s, `Byte(b)` for one `Some(b)`, `Contended` for two `Some` (RN-06, AC 3)
Proof: run sourcenets | test a_typed_bus_resolves_to_free_byte_or_contended ... ok

**C35** - `Tribuf` drives `Driven::strong(d)` with `oe` high and `Driven::Z` with `oe` low (RN-06, AC 4)
Proof: run sourcenets | test tribuf_drives_strong_or_high_impedance ... ok

**C36** - A `Pullup` gives way to a strong drive and returns when it releases; `Pulldown` the same with a low (RN-06, AC 5)
Proof: run sourcenets | test a_pull_wins_only_while_every_stronger_drive_is_released ... ok

**C37** - `Keeper` drives X at `Weak` before any 0 or 1, holds the last level the others drove at `Weak` after they release, and does not record an X (RN-06, AC 6)
Proof: run sourcenets | test a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it ... ok

**C38** - `Tran` with `en` high drives each side with the other's `others()`; low `Driven::Z` (RN-06, AC 7)
Proof: run sourcenets | test tran_drives_each_side_with_the_others_of_the_other ... ok

**C39** - `Tgate` joins `a` and `y` while `s` is high or `sn` low, and drives neither otherwise (RN-06, AC 8)
Proof: run sourcenets | test tgate_joins_while_s_high_or_sn_low ... ok

**C40** - In `op` and `dc` resolved nets run at `(0, ·)` and each sweep point; in `ac` they are constant (RN-06, AC 9)
Proof: run sourceresolved | test resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac ... ok

**C41** - While the rest of the net drives, `Bidir` drives the node as `D2A` and the net with `Driven::Z` (RN-07, AC 1)
Proof: run sourcenets | test bidir_drives_the_node_while_the_wire_is_driven ... ok

**C42** - While the rest has released, `Bidir` contributes nothing to the node and drives the net `Driven::strong(seen)` (RN-07, AC 2)
Proof: run sourcenets | test bidir_drives_the_wire_from_the_node_when_released ... ok

**C43** - A contended net (X at `Strong`) makes `Bidir` act as driven toward mid-rail (RN-07, AC 3)
Proof: run sourcenets | test bidir_acts_as_driven_on_a_contended_net ... ok

**C44** - A change of `driving` is a time point and a jump that `analog` reads (RN-07, AC 4)
Proof: run sourcenets | test a_change_of_driving_is_a_time_point_and_a_jump ... ok

**C45** - Two `Bidir`s alone on a net stop the run with `E0511` naming both drives (RN-07, AC 5)
Proof: run sourcenets | test two_bidirs_alone_stop_the_run_with_e0511 ... ok

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

**C52** - The alarm prints three records of `Alarm.watch.line`: `level = 1, strength = Pull` at 0, `level = 0, strength = Strong` at 1 µs, `level = 1, strength = Pull` at 3 µs, and none at 2 µs (RN-09, AC 1)
Proof: run booknatures | test the_alarm_prints_three_records_of_the_line ... ok

**C53** - The Book's alarm listing is the binary's output (RN-09, AC 2)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C54** - With `Pulldown` the alarm prints the exercise answer's records (RN-09, AC 3)
Proof: run booknatures | test the_pulldown_alarm_prints_the_exercise_answer ... ok

**C55** - `GAPS.md` lists none of L21, L22, L23 nor the resolved-net group of L58 but `TribufDelayed` (RN-10, AC 1)
Proof: run docs | test gaps_lists_no_resolved_net_gap ... ok

**C56** - Appendix C.2.3 and G.38 say `Tran` on nets of a `Resolve<Driven>` nature runs (RN-10, AC 2)
Proof: run spectext | test c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs ... ok

**C57** - No row of `code_owners.tsv` names a code a source raises, `E0203` and `E0225` included (RN-10, AC 3)
Proof: run speccodes | test no_owned_code_is_raised ... ok

**C58** - The IR builds, checks and prints a resolved net, and refuses a `Resolve` missing a count or of another type (RN-03, AC 1, below the language)
Proof: run irresolved | test a_resolved_net_is_built_checked_and_printed ... ok

**C59** - The interpreter refuses `Others`, `Resolve` and a `Resolved` carrier by name (RN-03, AC 1, codegen context R1)
Proof: run interp | test resolved_forms_are_refused_by_name ... ok

**C60** - A resolved carrier's plan has a value pin of `V`, a `RELAX` drive pin of `X` then a plain `driving` byte, and an others pin of `V` when read (RN-03, AC 1; RN-05, AC 7)
Proof: run planshape | test a_resolved_carrier_has_value_drive_and_others_pins ... ok

**C61** - The generated code calls the instance of the present count (RN-02, AC 7)
Proof: run transresolve | test resolve_calls_the_instance_of_the_present_count ... ok

**C62** - A relaxation step that does not settle reaches the owner as one `E0511` fault (RN-05, AC 4, below the language)
Proof: run prototran | test an_unsettled_loop_reaches_the_owner_as_one_e0511_fault ... ok

**C63** - A drive of a type without `Default`, with no initial value from its driver, is `E0187` naming the carrier and `Default` (RN-03, AC 4)
Proof: run elabnatures | test a_resolved_drive_without_default_or_initial_value_is_e0187 ... ok

**C64** - The Book's PI controller is refused only for its generic `idt` (RN-10, AC 1, phase 1 rows)
Proof: run bookmodules | test the_pi_controller_is_refused_only_for_its_generic_idt ... ok

**C65** - The Book's IRQ line (digital#16) runs on its `IrqLine` nature with the outcomes of Appendix E.7 (RN-09, AC 4)
Proof: run bookdigital | test the_irq_line_runs_on_its_resolved_nature ... ok

**C66** - The library chapter's resolved line (core-std#6) runs (RN-09, AC 4)
Proof: run bookcorestd | test the_resolved_line_runs_on_its_nature ... ok

**C67** - The Book's inventory agrees with `GAPS.md` §11 after the rows move (RN-09, AC 7)
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C68** - An undriven `VSignal` port of the top elaborates, and a run with no host source on it is `E0810` (RN-08, AC 7)
Proof: run sourcenatures | test an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run ... ok

**C69** - A second drive of a carrier whose nature does not implement `Resolve` is `E0220` naming the nature and `Resolve` (RN-02, AC 8)
Proof: run typesnatures | test a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve ... ok

**C70** - A nature with two `impl`s resolves each carrier by its drives' type (RN-02, AC 9; RN-06, AC 3)
Proof: run sourcenets | test a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type ... ok

**C71** - A `Logic` driven into `AndWire` is `E0180` naming the nature and the type; a literal two `impl`s accept is `E0181` (RN-02, AC 10)
Proof: run typesnatures | test a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181 ... ok

**C72** - Two drives of one net with different driven types are `E0225` naming both modules and both types (RN-02, AC 11)
Proof: run elabnatures | test two_driven_types_on_one_net_are_e0225 ... ok

**C73** - With one drive the carrier is still `resolve` of that drive (RN-03, AC 7)
Proof: run sourcenets | test one_drive_still_goes_through_resolve ... ok

**C74** - A library module of §22.3 on a nature without `impl Resolve<Driven>` is `E0187` naming the nature and the bound (RN-06, AC 10)
Proof: run typesnatures | test a_library_module_on_a_nature_without_resolve_driven_is_e0187 ... ok

**C75** - The Book no longer holds the sentence "Today the command refuses the file and runs nothing:" before the alarm's output (RN-09, AC 2)
Proof: run booknatures | test the_book_no_longer_says_the_alarm_is_refused ... ok

**C76** - The four worked nets of "Driving with strength" (digital#17 to digital#20) give the tables of Appendix F.4.1 to F.4.4 (RN-09, AC 5)
Proof: run bookdigital | test the_four_worked_nets_give_the_tables_of_f_4 ... ok

**C77** - The chapter's block of signal families (natures#2) elaborates and each carrier starts at the value its line states, `sda` at `Driven::Z` (RN-09, AC 6)
Proof: run booknatures | test signals_of_every_family_run ... ok

**C78** - `GAPS.md` §11 holds no row for natures#2, natures#3, natures~e99c1920, natures~91c3bffe, digital#16 to digital#20 or core-std#6 (RN-09, AC 7)
Proof: run docs | test gaps_section_11_holds_no_net_row ... ok

**C79** - A nature parameter bound by `Resolve<X, V>` is a resolved carrier in a generic body: drives typed `X`, `others()` typed `V` (RN-06, AC 10)
Proof: run typesgenmod | test a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body ... ok

**C80** - A placement on a nature that does not satisfy the `Resolve` bound is `E0187` naming nature and bound (RN-06, AC 10)
Proof: run typesgenmod | test a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound ... ok

**C81** - `Tribuf` of the library runs on a declared resolved nature (RN-06, AC 1)
Proof: run sourcenets | test a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** B1 (T1-T6) done. Defaults taken (user AFK): D16 - `code_owners.tsv` is not on this branch's base (it lives on `feature/params-and-io`), so the rows of `E0224` (T3), `E0413` (T4) and `E0242` (T5) are owed to T36, which deletes them. `NatureRules` carries a fourth field `carries_values` (discrete or resolved carrier versus node) beside the three of design C2, and answers `compatible` (§4 rule 13: a `VSignal` meets an `Electrical` port). `E0242` is reported at the top module's name, naming the node's path and each driver's instance path (the IR keeps origins, not spans). T2 needed no elaborator edit: the carrier's value type already flows from the checker's `Ty::Carrier`. The `types_carriers.rs` pins design "Goldens" names did not exist there; the pins actually replaced are listed in the T1, T3 and T4 commit bodies (`types_subset`, `types_modules`, `types_buck`, `elab_unlowered`, Book `modules#8`, whose test keeps only the `idt` generic-module refusal and is finished by T10). Owed to later tasks: `GAPS.md` L22 and L23, `inventory.tsv` and the Book status boxes (T10).
**Settled B2:** B2 (T7-T10) done, rebased onto origin/main (856d2668; two conflicts, hand-merged: `types/result.rs` import, `elab/elaborator.rs` where main had removed `top_has_a_port`). Defaults taken (user AFK): abi.md D1 amended as design D3 says (`driving: BOOL` after the value, zero bytes absent), in `.specs/features/device-abi/abi.md`; `Observation::Unsettled { nets, at }` lists every RELAX net of the step in name order; the owner's `ObservationKind::Unsettled { nets }` carries the first net as its path; relax tests use stub devices (`Driver`, `Keeper`, `Resolver`, `Oscillator`) in the test target. `code_owners.tsv` arrived with the rebase: the rows of E0224, E0413, E0242 and the GAPS section 10 list were deleted in the T10 commit. T10 moved the docs.rs loud count 38 to 36 (params-and-io had already moved it to 38). No Book prose or status box pins a P0001 of project or signal-flow natures, so none changed. The T9 test added a partials check for its stub (abi_devices guard). Full root `cargo test --no-fail-fast` green after the rebase except that guard, fixed.
**Settled re-plan (2026-11-22):** B3 onward re-planned for the explicit-unknowns model (MD-78; spec amendment A6, design "Amendment of 2026-11-22"). T11 to T36 keep their numbers where their deliverable survives; T37 (`E0180`, `E0181`), T38 (`E0225`) and T39 (natures#2) are new. Superseded: the `log` record of a loop (T25 now names the drives of `E0511`), every unknown arm (T22, T29 to T33), `Wire`, `Tri`, `SumNet` (every test declares its nature). Open for the user: spec Q2 (B6 and B7 wait for `generics` C's generic modules), Q3 (T39 waits for `composite-values` C), Q4 (the `impl` of an undriven net, T19, T33); the plan follows each recommendation. The branch was fast-forwarded to `origin/main` at `20c285b8` (no conflict: it was already contained in `main`).
**Owed by explicit-unknowns (2026-11-23, finding 55):** `GAPS.md` L71, a host write of a `Logic` param or of a four-state pattern (`"10xz"` for a `Logic<4>` cell). The host's drive of a resolved `Logic` carrier is its first caller: parse the pattern with the rules of §22.1.4 (`E0702` for a bad character or length) and write the two planes of `abi.md` P3.
**Settled T11 (2026-11-23):** `origin/feature/resolved-nets` carried one unmerged commit (a73c9664, the re-plan), so the branch is based on it and `origin/main` (ddc9edb1) is merged (one conflict in this file, by hunk). Build green; `an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value` (relax, 5 pass) and `an_unsettled_loop_is_e0511_naming_its_drives` (protocol_transient, 26 pass) are the successors explicit-unknowns T37 left: C31 and C62 name them. Anchors as of main: `Resolve` is NOT a builtin trait yet (`pom/builtin.rs` rows, `IMPLEMENTABLE_BUILTIN_TRAITS` in `types/traits.rs` hold ten names without it): T16 adds the row and the name, and the `impl Resolve<X, V> for N` entry carries `builtin_trait: Some("Resolve")` with `trait_arguments`. `NatureRules` still has `resolved` from `NatureKind::Resolved` (the `resolve` field: `NatureFieldKind::Resolve`, `NatureFieldNode::Resolve`, `pom/lower/hardware.rs`) and `one_driver`; T16 replaces both sources. `std/nets.msl` still holds `Drive`, `Wire`, `Tri`, `SumNet` (explicit-unknowns T23 did not land `Driven`; T27 does, over `Drive`) and `std/logic.msl` holds `Logic<N>`. The drive pin layout and `E0511` (`Observation::Unsettled`, `ObservationKind::Unsettled`) are on main as B2 built them; `E0187` is the `Repr`/`Default` code (T11 finds no clash with this feature). `pom_bundles.rs` still uses the library's `Wire` (lines 4, 78-80): T16 re-declares. Generics phase C (generic modules) is not on main (`feature/generics` at b0d6148e): B3 to B5 need none of it; B6 (T27 onward) stops there, as does T39 (composite-values C, check at B7).
**Settled B3 (2026-11-23):** B3 (T11-T15) done on `feature/resolved-nets` (based on `origin/feature/resolved-nets`, which held the unmerged re-plan commit, with `origin/main` merged in). Amendments recorded against the design: (1) `CarrierRole::Resolved { driven: ValueType }` carries the driven type itself (the IR has no `TypeId` for a scalar), so `CarrierRole` is no longer `Copy` (the three call sites that copied it clone). (2) `NetData::resolution` holds `Resolution { resolver, driven: ValueType, drives, holders, others }` (`driven` added to design C4's struct); `IrBuilder::resolve_net(net, resolution)` records it and `Ir::check` validates it (`check/resolved.rs`). (3) `Body::targets()`, `Body::reads_others(c)`, `Body::resolve_drive_carriers()` are new on the IR handle (`others_reads`, `resolve_drive_reads` on `Deps`). (4) The plan appends the pins `<carrier>.drive` (PORT, RELAX, `DriveLayout` of `X`: its fields then `driving` at offset `size(X)`) and `<carrier>.others` (PORT, `V`, only when the body reads `others()`) after the element-drive pins (`Pins::resolved`, `ResolvedPins`); a drive or commit of a `Resolved` carrier targets `Target::ResolvedDrive(c)`, `others()` reads `Source::Others(c)`; a resolver body's carriers listed in an `Expr::Resolve` are read through a `DriveLayout` pin (the value then `driving`), so every pin of a drive net has one layout (P4). The delayed `.now` pin of a delayed drive of a resolved carrier also has the drive layout. (5) The transpiler prints `Expr::Resolve` as a block gathering the present drives (`<carrier>_driving_` variable bound beside the value) and `match`ing the count to `by_count[k]`; a drive of a resolved carrier writes `rt::Carried::write_drive` (value then `driving` 1); `rt::Carried::is_driving` reads the byte. Reaction commits to a resolved carrier (plan side done: `StatementPlan` targets the drive pin with the drive layout's size) are not printed or run yet: T22 owns the first source test that commits one. (6) `Netlist::circuit` binds a holder's drive pin to its drive net (`Resolution::holders` position), its others pin to that drive's others net, a non-holder's others pin to the value net (D7), both `Open` when the net records no resolution; the check is `a_resolved_net_binds_drives_value_and_others` with stub devices serving the plan's shapes. The interpreter names `others() of a resolved carrier` and `a resolve of drives` among the forms it refuses. Owed to B4: `elab/place.rs` has a placeholder arm (`CarrierRole::Resolved` gets no parent net, like `Signal`) that T18 and T19 replace when the elaborator produces the role; `Resolve` is not a builtin trait yet (T16 adds the `pom/builtin.rs` row and the `IMPLEMENTABLE_BUILTIN_TRAITS` name). Gotcha: after adding an IR variant, `cargo clean -p piperine-ir` was needed once because dependents compiled against a stale rlib. Gates: Build line (zero warnings) plus `cargo test -p` ir (153), plan (64), interp (71), transpile (163), lang (1321, T12) all green; suite_hygiene green.
**Settled B5 (2026-11-24):** B5 (T22-T26) done. T22 fixed three defects found on the way: a commit of a resolved carrier writes its drive pin (value then `driving` byte, `Commits::commit_drive`); `resolve` of no drive types its empty array; two natures' `impl` methods named `resolve` no longer print alike (`impl_qualified_name` uses the nature; the `elab_natures` golden names follow). T23: a resolver body's names with `+` (the `others` readers) print as `_p`. T24: `relax()` ran the drives before the resolver, so the first pass read the `others` net an earlier relaxation left; each pass now runs the functions that write no RELAX net first (§8.2.3 steps 1 and 2). T25: `E0511` names the loop's drives by their holders (the instance path, `host` for the host's drive), read by `LoopDrives` in `piperine-api/src/failure.rs` from the fault itself: `piperine_solver::Error::Unsettled { domain, tick, drives }` (the digital adapter maps `DigitalError::Unsettled` to it; `Error::unsettled()` reads it; the text is unchanged) - no message parsing. `Failure::from_fault(analysis, error, time, &Ir)` takes the design. The `$resolve` function name now carries the nature. Deviations, T26: in `ac` the test can only assert the sweep runs (a discrete net has no phasor, `E0801`); a `dc` sweep refuses record streams (`P0010`), so T26 observes `n` and `echo.seen` through the dc table. Gates B5: lang 1342, transpile 163, digital-solver 159, api 290 (801 across analog-solver, solver and api after the T25 follow-up), plan 64, ir 153, and the parity targets `source_resolved`, `source_natures`, `source_logic`, `source_events`, `source_commits`, `source_explicit`, `source_host`, `source_session` all green. B6 (T27 on) waits for `generics` C on `main`.
**Settled B6 (2026-11-24, stopped at T28; T29 to T33 and B7 not started):** `origin/main` (generics C, 0bf08c637) merged (24 conflicts, hand-merged by hunk; `Resolved` and `Internal` carrier roles both kept; `child_carrier_ids` counts `Resolved` among the ports; E0203 and E0225 rows left `code_owners.tsv` and GAPS section 10 again). T27: `std/nets.msl` is Part VII 22.3 (fences formatted by `piperine fmt`, spec fences re-synced), `Wire`, `SumNet`, `Drive` gone, `Bidir` and `TribufDelayed` rewritten over `W: Nature + Resolve<Driven>`; the three behavior blocks repeat the generic names (D26, `keeper_and_bidir_blocks_repeat_their_generic_names`, done early because the library does not check without it). Compiler fixes on the way: a struct that holds a generic instance (`Driven` holds `Logic<1>`) got no arena entry for it, so its constant did not fold (`TypeArena` now numbers every reached instance before building entries, `reached_instances`); T28: an array whose elements are `Option`s is an IR array of packed options (`ValueTypes::packed_option`) and `d[k]` unpacks to its pair (`lower_option`), which the typed `DataBus` of F.4.4 needs (its `resolve` is written with a `for` over `drives[k]`, the closure-and-tuple-match form of the fragment still has no lowering: `Option` closure parameters and a tuple scrutinee holding an `Option`); the transpiler no longer parenthesizes an `if` used as the condition of an `OptionPack`. Tests that named the library `Wire` or `Drive` declare their own nature (pom_bundles, pom_classify, pom_reflect, pom_resolve, std_library, book.rs lists emptied; `.pre` fixtures of the three std files re-made). **Blocker for T29 on (a real gap, not a defect of this batch):** a generic module over a *discrete* nature, `Tribuf<W: Nature + Resolve<Driven>>(w: W, ..)`. Today a port or signal typed by a `Nature` parameter is a node (`GenericSignature::is_nature_parameter`, "a node of the nature a use gives"), so the body is checked with `w` a node: `w = Driven::strong(d)` is `E0222` ("the target of a definition is a node"), `Resolve<Driven>` as a bound is `E0161` ("`Resolve` is not a trait, so it cannot bound `W`", `GenericSignature::bound` knows `Repr`, `Nature`, `Fn`, the H.8 traits and project traits only). Needed: a `Bound::Resolve { arguments }`; a nature parameter bound by it is a resolved discrete carrier in the generic body (value `V` = the second argument or `X`, drives typed `X`, `others()` of type `V`); `satisfies` for an argument nature through `ResolveImpls::of`; `E0187` naming nature and bound when it does not (T29's third check); the per-instance elaboration then sees the concrete nature. The Book targets `book_core_std`, `book_natures` and `book_digital` are red since T27 (their pinned refusals named the missing `Driven`; the refusal is now that `Resolve` bound), by design until B7 flips them. Gates run: Build zero warnings; lang, transpile green after T28 (see commits); parity `source_nets` 4 pass, `source_resolved`, `source_natures`, `source_logic`, `source_modules` green after the merge.
**Settled B6 (2026-11-24, second pass, T29a to T33):** T29a done (the blocker): `Bound::Resolve { arguments }`; a nature parameter bound by it is `Ty::Carrier { nature: <the parameter decl>, value: V }` in the generic body (`Checker::resolved_parameter_carrier`, `NatureRules::of` answers resolved for a `GenericParam` with the bound, `ResolveImpls::of` answers from the bound with the parameter itself as `implementation`, `resolve_tables` keyed by the parameter); `Substitution::apply` replaces the nature of a `Carrier` and of a `Node` through `replaced_nature` (a nested placement `Tran::<W>` inside `Tgate<W>` follows the enclosing instance); `Inference::unify` binds the nature parameter of a carrier meeting a carrier; `BoundCheck::satisfies(Bound::Resolve)` asks `ResolveImpls::of` of the argument nature (`E0187` names the nature and `Resolve<Driven>`); a bound with no or three arguments or with no `Nature` is `E0187`; the elaborator reads the instance's carrier (`consts.typed_carrier`) and `Elaboration::concrete_implementation` maps the parameter's pick to the instance nature's `impl` whose `X` equals the bound's. T30 to T32 needed two more compiler fixes: an analog block reading `others()` of a resolved carrier read `Source::Carrier` instead of `Source::Others` (`transpile/print/analog.rs`, terms_block had no input for it: Bidir), and `E0181` of an undriven net now names the carrier (D21). T33: the host's drive of a resolved top port is `Session::drive` (`api/host.rs`: the top's `Resolved` carrier, drive net `<net>$drive0` when the net's first holder is the host) and `net_bytes` (`solver/core/bind.rs`) sets the `driving` byte of a drive net on a host write. **Not done, escalate:** `a_parent_drives_a_childs_pub_resolved_signal` (RN-03 edge case, spec line 326): needs the checker to accept a definition or commit whose target is a child's `pub` resolved signal and the elaborator to make a `Resolved`-role reach carrier that is a holder of the child's net (a new `ReachedItem` that drives); it is a design decision (`CarrierRole::Reached` is read-only today), so the box is left open and the test unwritten. T33's other five tests pass. L71 is not started (T35+). Gates: Build zero warnings; lang 1404 pass (before the T31 to T33 edits; the E0181 message change re-run in source_nets); transpile 167; parity `source_nets` 21 pass.
**Stopped at the end of B6 (2026-11-24), B7 not started.** Next: T34 (alarm), T35 (IRQ line and the four worked nets, core-std#6), T39, T36, L71 (host write of a `Logic` pattern: `Session::drive` now reaches a resolved top port, so the pattern parser of §22.1.4 plugs in at `api/host.rs` `drive`; `E0702` for a bad character or length; the two planes of abi.md P3), and the open parent-drives-child item of T33. `book_core_std` is red on `the_resolved_line_is_refused_until_a_nature_can_implement_resolve`: the listing now gets past `Driven` and `Resolve` and stops at `E0187` ("the `in` port `req0` of the top `Board` has no initial value"), which T35 flips to a run; `book_natures` and `book_digital` were not re-run after T29a. Gates after T33: solver + api 697 pass, parity `source_resolved`/`source_natures`/`source_logic`/`source_modules`/`source_nets` 78 pass, transpile 167, `suite_hygiene` 23.
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → (wait) → Phase 4 → Phase 5 → Phase 6 → Phase 7 → (wait) → Phase 8 → Phase 9

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6            (done)
Phase 2:  T7 → T8 → T9                           (done)
Phase 3:  T10                                    (done)
Phase 4:  T11                                    (done)
Phase 5:  T12 → T13 → T14 → T15                  (done)
Phase 6:  T16 → T17 → T37 → T18 → T19 → T38 → T20 → T21
Phase 7:  T22 → T23 → T24 → T25 → T26
Phase 8:  T27 → T28 → T29 → T30 → T31 → T32 → T33
Phase 9:  T34 → T35 → T39 → T36
```

Execution is strictly sequential.

- **From `explicit-unknowns` phase B (2026-11-23).** `Logic<N>` runs from `std/logic.msl` (operator traits, `From`, `logic`, `matches_bits`, the host cell `10xz`). Its spec is to be revised for `Resolve<X, V>` before B3; T27 starts from `Driven`. The earlier strength struct of `nets` is called `Drive` in `std` (its fields `v: bool`, `s: Strength`) until `Driven` replaces it; the Book's refusals of `Driven` and `Resolve` (E0161) stand.

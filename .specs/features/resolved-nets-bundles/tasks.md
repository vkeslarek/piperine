# Resolved Nets: Bundles Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/resolved-nets-bundles/design.md`
**Status**: In Progress - B1 (T1 to T4) done; B2 started 2026-11-24 on `feature/resolved-nets-bundles`; B2 onward (T5 to T21) amended 2026-11-22 for the explicit-unknowns model (MD-78) and waiting for the user's approval of this plan
**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13" (B1 as planned then; the amendment of B2 onward has no approval yet)
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run pombundles**: `cargo test -p piperine-lang --test pom_bundles`
**Run typesbundles**: `cargo test -p piperine-lang --test types_bundles`
**Run elabbundles**: `cargo test -p piperine-lang --test elab_bundles`
**Run sourcebundles**: `cargo test -p piperine-parity --test source_bundles`
**Run apimetadata**: `cargo test -p piperine-api --test metadata`
**Run pomreflect**: `cargo test -p piperine-lang --test pom_reflect`
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run speccodes**: `cargo test -p piperine-lang --test spec_codes`
Buck targets (only in a task whose `Done when` says "with the buck", MD-77): `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test source_commits --test source_records --test buck_supervisor --test source_time --test buck_boundary --test buck_closed_loop --test buck_power`, `cargo test -p piperine-cli --test buck`

Executors: **every task is Sonnet**. Opus verifies each batch and the feature.

**Answers.** The spec's delivery question follows `resolved-nets` Q1 (B, user 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"). The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D10 and `resolved-nets` design D2, D16, D20. The amendment of 2026-11-22 (spec A6) applies the user's decisions of 2026-11-21, quoted in spec.md.

**Base and order.** Branch `feature/resolved-nets`, kept current by merging `main` into it (by hunk; never one side of a whole file; documents and tests keep both intents). B1 (T1–T4) is on `main`.

| Batch | Tasks | Starts when | Delivers |
| ----- | ----- | ----------- | -------- |
| B2 | T5 to T8 (phases 2, 3) | `explicit-unknowns` phase A on `main` (`composite-values` A–B already is) | the contract; bundle types, views, literals, `E0211` |
| B3 | T9 to T16 (phases 4, 5) | B2; T15 also needs `resolved-nets` T22 on the branch | bundles elaborate and run; host and POM names |
| B4 | T17 to T21 (phases 6 to 8) | B3, `generics` B and `unknown-semantics-events` B5 (T17, T18), `generics` A and C and `composite-values` C (T19, T20), `generics` C (T21's `Slice<T>`) | functions on a bundle; generic and array bundles; the records |

A task whose dependency is not on `main` pauses the feature and is reported.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-a TMPDIR=/var/tmp/piperine-tmp-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command; never a target directory another worktree is building in. Cargo with `-p <crate>`, or plain at the root only at the end of a batch; never `--workspace` (`piperine-lang-server` is postponed and known broken), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report. Never `git stash` (the stash list is shared between worktrees).
- **Gate policy (user, 2026-11-21: "Quero que aplique o dos gates mais leves apenas"; 2026-11-22: "Suite completa é só no fim dos lotes e ponto final!").** Per task: the `**Build**` line and the tests of the crates the task touched, nothing more. The full root suite, the integrated suites, the parity targets and the twins run once, at the end of each batch (the batch's last task carries `Gate: batch`), and never in a fix round or a verification. The buck runs only in a task whose `Done when` says "with the buck".
- **Explicit model (MD-78).** A one-driver field is a field of a discrete nature that does not implement `Resolve`; every fixture with a `Bit` field or signal gives it an initial value (`E0187` otherwise); a test that needs a resolved field declares its own nature with `impl Resolve<..>`; no test names `Wire`.
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. A new comparison uses an existing contract as it is (D12). A fair twin uses the same step configuration on both sides, nothing else. Every buck target passes after every task that touches the printer, the plan, the digital solver, the owner protocol or the elaborator's lowering of an existing form.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A spec text that is wrong is amended explicitly with its `spec_text` guard (MD-57), never left. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites (§3.1, §5, §7), or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Proof lines are real lines.** A check's expected line is confirmed against a real run in the task that first makes it pass; a line that differs from what the runner prints is corrected in that task's commit, never the test renamed to fit.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR enum variants, `CarrierRole` and plan `Source` variants are appended last. The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/bundles/*.msl`.
- **The Book** (21 chapters, Part 1 Fundamentals and Part 2 In depth): its prose never defers to Rust, its tone is respectful, and a status box that pins a `P0001` message is edited with the test that pins it.
- **Code owners (`resolved-nets` D16).** A task that first raises `E0260`, `E0261`, `E0262` or `E0264` deletes the code's row from `crates/piperine-lang/tests/support/code_owners.tsv` in its commit when the file is on the branch; otherwise it writes the code on the Handoff and T21 deletes the row.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given (lowercase conventional subject of at most 72 characters), ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No merge; push only as the orchestrator asks.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-25 (the POM mirrors the source), MD-57, MD-72, MD-77 (the buck runs on demand), MD-78 (the explicit model).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| POM checks (`piperine-lang/src/pom/resolve`) | integration | each declaration code with its span and the names it cites | `crates/piperine-lang/tests/pom_bundles.rs` | `cargo test -p piperine-lang --test pom_bundles` |
| Checker (`piperine-lang/src/types`) | integration | every view rule and each code | `crates/piperine-lang/tests/types_bundles.rs` | `cargo test -p piperine-lang --test types_bundles` |
| Elaborator (`piperine-lang/src/elab`) | integration | the leaf carriers and nodes, connections per field, `E0220` per field | `crates/piperine-lang/tests/elab_bundles.rs` | `cargo test -p piperine-lang --test elab_bundles` |
| Source-level runs | integration | one test per run-time AC, from MSL source on the Rust backend | `crates/piperine-parity/tests/source_bundles.rs` | `cargo test -p piperine-parity --test source_bundles` |
| Host and POM reflection | integration | names by path; the port as written | `crates/piperine-api/tests/metadata.rs`, `crates/piperine-lang/tests/pom_reflect.rs` | `cargo test -p piperine-api --test metadata`, `cargo test -p piperine-lang --test pom_reflect` |
| The Book and records | integration | each row changed; each removed `GAPS.md` row and code owner row | `tests/book/natures.rs`, `inventory.tsv`, `crates/piperine-cli/tests/docs.rs`, `spec_codes.rs` | `cargo test -p piperine --test book_natures`, `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test spec_codes` |
| Contract re-check (T5) | integration | the fixtures given initial values still assert what they asserted; the moved anchors on the Handoff | the targets of the crates it touches | `cargo test -p piperine-lang` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task whose tests are one or two targets of one crate | the `**Build**` line, then the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the checker, the elaborator's lowering or placement, or the host | Quick, then `cargo test -p <crate>` for every crate the task touched (for the root package, only the `book_*` targets it touched, with `book_inventory` and `book_claims`); "with the buck" adds the buck targets |
| Batch | The last task of each batch (T8, T16, T21), once | Full, then `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, `cargo test -p piperine --test integrated`, `cargo test -p piperine-cli --test integrated`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` |
| Build | After a task with `Tests: none` | the `**Build**` line |

---

## Execution Plan

Batches (whole phases): **B1** phase 1 (T1–T4, done) · **B2** phases 2–3 (T5–T8) · **B3** phases 4–5 (T9–T16) · **B4** phases 6–8 (T17–T21). See "Base and order" for when each starts.

### Phase 1: Declaration checks (done)

```
T1 → T2 → T3 → T4
```

### Phase 2: Contract (waits for `explicit-unknowns` phase A on `main`)

```
T5
```

### Phase 3: Checker

```
T6 → T7 → T8
```

### Phase 4: Elaborator

```
T9 → T10 → T11
```

### Phase 5: Bundles run from source

```
T12 → T13 → T14 → T15 → T16
```

### Phase 6: Functions on a bundle (waits for `generics` B, `unknown-semantics-events` B5)

```
T17 → T18
```

### Phase 7: Generic bundles and arrays of bundles (waits for `generics` A and C, `composite-values` C)

```
T19 → T20
```

### Phase 8: Records

```
T21
```

---

## Task Breakdown

### T1: POM: a bundle mixes no value field (`E0260`)

**What**: `BundleShape` (design C1) in stage 8: a struct with a carrier field and a value field is `E0260` naming one field of each kind. Create `crates/piperine-lang/tests/pom_bundles.rs` with its `//!` header. Delete the `E0260` row of `code_owners.tsv`.
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs`
**Depends on**: None
**Reuses**: `bundle_structs` of `classify.rs`, the `E0201` check
**Requirement**: BN-01
**Executor**: Sonnet

**Done when**:

- [x] `a_struct_of_carriers_is_a_bundle` and `a_struct_mixing_carrier_and_value_fields_is_e0260` pass under `**Run pombundles**`
- [x] Code clarity: `BundleShape` doc-commented citing §7 rule 1; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(pom): a bundle holds only carriers`

---
### T2: POM: directions of bundle fields (`E0261`)

**What**: A one-driver field without `in`/`out`, or a direction on a field of any other nature, is `E0261` naming the field (§7 rule 2). Delete the `E0261` row.
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs`
**Depends on**: T1
**Reuses**: `BundleShape`, the nature kinds of `pom/lower/hardware.rs`
**Requirement**: BN-01
**Executor**: Sonnet

**Done when**:

- [x] `one_driver_fields_carry_a_direction_and_no_other_field_does` and `a_misplaced_or_missing_field_direction_is_e0261` pass under `**Run pombundles**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(pom): directions of bundle fields`

---
### T3: POM: `flip` and directions of bundle ports and nested fields (`E0262`)

**What**: `flip` on a type that is not a bundle, `out` on a bundle port, `in`/`out` on a nested bundle field are `E0262` (§7 rules 5, 6). Delete the `E0262` row.
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs`
**Depends on**: T2
**Reuses**: `BundleShape`, `PortKind::Bundle`
**Requirement**: BN-01
**Executor**: Sonnet

**Done when**:

- [x] `flip_out_and_nested_directions_are_e0262` passes under `**Run pombundles**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(pom): flip and bundle port directions`

---
### T4: POM: a bundle is not a value at a declaration (`E0264`)

**What**: A bundle as a param type, a nature's type argument or a function result, and a bundle signal or port with an initial value, are `E0264` (§7 rules 9, 10; design D7). Delete the `E0264` row once T7 also raises the checker positions, or now if the guard only counts spellings (it does: PI-13).
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs`
**Depends on**: T3
**Reuses**: `BundleShape`
**Requirement**: BN-01
**Executor**: Sonnet

**Done when**:

- [x] `a_bundle_declared_as_a_value_is_e0264` passes under `**Run pombundles**`
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `feat(pom): a bundle is not a value`

---
### T5: Contract: merge `main` and re-read the anchors

**What**: Merge `main` (with `composite-values` A–B and `explicit-unknowns` phase A) into the branch, by hunk; re-read `types/carrier.rs` (`port_type`), field access and struct types as `composite-values` left them, `resolved-nets`' `NatureRules` and `pom/resolve/shape.rs` (`E0261`: still the `resolve` field, or the capability if `resolved-nets` T16 has run); give every fixture of this feature's targets that declares a `Bit` field or signal with no initial value one (`E0187` otherwise, design "Goldens"); write the moved anchors on the Handoff. Stop and report if `explicit-unknowns` phase A is not on `main`.
**Where**: `crates/piperine-lang/tests/pom_bundles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md
**Requirement**: BN-02
**Executor**: Sonnet

**Done when**:

- [x] The merge is clean or its conflicts are resolved by hunk without changing behaviour; the `**Build**` line passes
- [x] `**Run pombundles**` passes with the fixtures given initial values (each named in the commit body)
- [x] The Handoff names every moved anchor
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `docs(specs): bundles contract on the explicit model`

---
### T6: Checker: bundle types and views

**What**: Design C2: a bundle port or signal is `Ty::Bundle { decl, view }`, matched before the value-struct arm; `flip` and the observer view set the view; `tx.valid` is a carrier with the view's direction; writing an input field is `E0222`; a field's direction is required exactly when its nature is one-driver (does not implement `Resolve`, §7 rule 2 as amended), and fields without a direction are drivable in every view (D8). Create `crates/piperine-lang/tests/types_bundles.rs`; its fixtures use one-driver and continuous fields only (a resolved field is T15's).
**Where**: `crates/piperine-lang/src/types/bundle.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `port_type`, `NatureRules`, `types/definition.rs` (`E0222`)
**Requirement**: BN-02
**Executor**: Sonnet

**Done when**:

- [x] `a_producer_drives_out_fields_and_reads_in_fields`, `flip_swaps_every_one_driver_direction_nested_included`, `an_observer_reads_every_one_driver_field`, `driving_an_input_field_is_e0222`, `a_bundle_port_is_not_a_value_input`, `a_bundle_field_accepts_carrier_writes_its_view_allows` and `a_carrier_method_on_a_field_applies_to_that_field` pass under `**Run typesbundles**`
- [x] The `P0001` pins of design "Goldens" are replaced
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): bundle types and views`

---
### T7: Checker: literals and bundles in value positions

**What**: A bundle literal's field given a carrier of another nature is `E0263`; a literal written anywhere but a connection, and a bundle as an operand of a value operator, are `E0264` (design D7); two bundles of different types with the same field names do not connect (`E0251`).
**Where**: `crates/piperine-lang/src/types/bundle.rs`
**Depends on**: T6
**Reuses**: C2, `E0263` of `classify.rs`
**Requirement**: BN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_literal_field_of_another_nature_is_e0263`, `a_literal_outside_a_connection_or_a_bundle_operand_is_e0264` and `bundles_of_different_types_do_not_connect` pass under `**Run typesbundles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): bundle literals`

---
### T8: POM: a carrier method on a whole bundle is `E0211`

**What**: `Recv::Bundle` joins the `E0211` rule of `members.rs`, the message naming the bundle and the method (design D5).
**Where**: `crates/piperine-lang/src/pom/resolve/members.rs`
**Depends on**: T7
**Reuses**: the `on_array` path
**Requirement**: BN-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_carrier_method_on_a_whole_bundle_is_e0211` passes under `**Run pombundles**`
- [ ] Gate check passes: Batch (end of B2)

**Tests**: integration
**Gate**: batch

**Commit**: `feat(pom): a carrier method applies to a field, not a bundle`

---
### T9: Elaborator: `BundleFields` flattens ports and signals

**What**: Design C3, D3: each bundle port and `signal a: Stream;` becomes one carrier or node per leaf field named by dotted path; the declaring module drives none of its one-driver fields unless it writes them (§7 rule 7). Create `crates/piperine-lang/tests/elab_bundles.rs`.
**Where**: `crates/piperine-lang/src/elab/bundles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Structure`, `elab/carriers.rs`
**Requirement**: BN-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path` and `a_bundle_signal_declares_every_carrier_and_drives_none_it_does_not_write` pass under `**Run elabbundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): bundle fields`

---
### T10: Elaborator: connections and literals field by field

**What**: `BundleConnection::bind`: a bundle carrier passed to a bundle port, and a literal `Cell { wl: w, bl: b }`, connect each leaf field to the one of the same name (§7 rules 8, 14).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T9
**Reuses**: `BundleFields`, `place.rs`
**Requirement**: BN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_bundle_connection_binds_each_field_by_name` and `a_literal_binds_each_named_field_to_its_carrier` pass under `**Run elabbundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): bundle connections`

---
### T11: Elaborator: one driver per field after views

**What**: Design C4: after placement, a one-driver leaf field that two instances see as `out` is `E0220` naming the field's path (§7 rule 15); an observer adds no drive.
**Where**: `crates/piperine-lang/src/elab/place.rs`
**Depends on**: T10
**Reuses**: the net table of placement
**Requirement**: BN-02
**Executor**: Sonnet

**Done when**:

- [ ] `two_producers_of_one_field_is_e0220_naming_its_path` and `an_observer_view_adds_no_drive` pass under `**Run elabbundles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): one driver per bundle field`

---
### T12: A valid/ready handshake from source

**What**: Create `crates/piperine-parity/tests/source_bundles.rs` and `tests/bundles/stream.msl`: a producer, a consumer and an observer on one `Stream` signal run three beats; a reaction on `rx.valid.changed()` and a definition of `tx.data`.
**Where**: `crates/piperine-parity/tests/source_bundles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the runner of `source_natures.rs`
**Requirement**: BN-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_producer_consumer_and_observer_run_three_beats` and `field_commits_definitions_and_methods_follow_section_5` pass under `**Run sourcebundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): a stream bundle handshake`

---
### T13: A differential pair and a cell literal from source

**What**: An op of a differential amplifier over a `Diff { p, n }` port (probes and contributions on `d.p`, `d.n`); a `Cell` connected by literal from two buses.
**Where**: `crates/piperine-parity/tests/source_bundles.rs`
**Depends on**: T12
**Reuses**: `tests/bundles/*.msl`
**Requirement**: BN-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_diff_bundle_connects_its_nodes_like_ports`, `a_bundle_carrier_connects_field_by_field` and `a_cell_literal_connects_each_named_field` pass under `**Run sourcebundles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): diff and cell bundles`

---
### T14: Nested bundles with `flip`

**What**: A bundle with a plain and a flipped nested bundle runs with the views applied recursively (§7 rule 5).
**Where**: `crates/piperine-parity/tests/source_bundles.rs`
**Depends on**: T13
**Reuses**: `tests/bundles/*.msl`
**Requirement**: BN-06
**Executor**: Sonnet

**Done when**:

- [ ] `nested_bundles_apply_flip_recursively` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): nested bundles`

---
### T15: A bundle field of a resolved nature

**What**: Waits for `resolved-nets` T22 on the branch. The fixture declares `pub nature Sda { value: Driven }` with `impl Resolve<Driven> for Sda { Driven::strongest }` and `struct Bus { sda: Sda, out scl: Bit }`: every module that sees the bundle may drive `sda`, the observer included (D8), and `sda` is `strongest` of the drives.
**Where**: `crates/piperine-parity/tests/source_bundles.rs`
**Depends on**: T14
**Reuses**: `resolved-nets` resolvers
**Requirement**: BN-06
**Executor**: Sonnet

**Done when**:

- [ ] `every_view_may_drive_a_resolved_field` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): a resolved bundle field`

---
### T16: Host and POM names of bundle fields

**What**: `--print Top.p.tx.valid`, a record and `Metadata` name a field by its dotted path (D4); `pom/reflect` shows a `flip Stream` port as one port with its type and view.
**Where**: `crates/piperine-api/tests/metadata.rs`
**Depends on**: T15
**Reuses**: `Metadata`, `pom/reflect/hardware.rs`
**Requirement**: BN-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_bundle_field_is_named_by_its_dotted_path` passes under `**Run apimetadata**`
- [ ] `a_bundle_port_reflects_its_type_and_view` passes under `**Run pomreflect**`
- [ ] Gate check passes: Batch (end of B3)

**Tests**: integration
**Gate**: batch

**Commit**: `feat(api): bundle fields by path`

---
### T17: Functions on a bundle

**What**: Waits for `generics` B on `main`. `impl Diff { fn vd(self) -> f64 }`: `self` is a read-only handle (§3.1.1, §1.4.7); `d.vd()` in `analog` gives `v(d.p, d.n)`; a write through it is `E0133` naming the field; `impl Trait for` a bundle stays `E0134` (§3.1.9 as `explicit-unknowns` narrowed it). Merge `main` first.
**Where**: `crates/piperine-lang/src/types/bundle.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `generics` handles
**Requirement**: BN-05
**Executor**: Sonnet

**Done when**:

- [ ] `self_on_a_bundle_is_a_read_only_handle` and `a_write_through_a_bundle_handle_is_e0133` pass under `**Run typesbundles**`
- [ ] `vd_on_a_diff_bundle_gives_the_differential_voltage` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): functions on a bundle`

---
### T18: An event function on a bundle

**What**: Waits for `unknown-semantics-events` B5 on `main`. `on rx.beat(clk)` with `fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid && self.ready }` runs exactly on the rising edges where both are true.
**Where**: `crates/piperine-parity/tests/source_bundles.rs`
**Depends on**: T17
**Reuses**: `unknown-semantics-events` UE-04
**Requirement**: BN-05
**Executor**: Sonnet

**Done when**:

- [ ] `beat_fires_on_rising_edges_with_valid_and_ready` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): an event function on a bundle`

---
### T19: Generic bundles

**What**: Waits for `generics` A (generic value types) on `main`. `struct Stream<T: Repr> { .. }` instantiated per type argument, each instance flattened as in T9.
**Where**: `crates/piperine-lang/src/elab/bundles.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Instantiations`
**Requirement**: BN-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_generic_bundle_is_instantiated_per_type_argument` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): generic bundles`

---
### T20: Arrays of bundles

**What**: Waits for `composite-values` C (L24) on `main`. `[flip Stream<u8>; 4]` connects each element field by field; `rx[k].valid` is a carrier.
**Where**: `crates/piperine-lang/src/elab/bundles.rs`
**Depends on**: T19
**Reuses**: arrays of carriers
**Requirement**: BN-06
**Executor**: Sonnet

**Done when**:

- [ ] `an_array_of_bundles_connects_each_element_field_by_field` passes under `**Run sourcebundles**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): arrays of bundles`

---
### T21: Records: Book, GAPS, code owners

**What**: The Book's `natures#8` and `natures~52790298` run (with `Slice<T>` once `generics` C is on `main`; otherwise they keep only that refusal, owner `generics`; `Slice<T: Repr + Default>` since generics T116, harness at a type with a `Default`); **amended 2026-11-23 (generics C gap verdict G3):** modules#10 with modules#11 (`MemoryBank` of `Dram1T1C` cells over the bundle `Cell`) and cheatsheet#8 (the template card), which `generics` phase D T93 hands over refused only for the bundle `Cell` (L25), run too (waits for `generics` D T93 on `main`; otherwise they keep only the bundle's refusal); `GAPS.md` loses L25 and §9 gives Part II §7 its share; `tests/book/natures.rs`, `inventory.tsv`, `docs.rs` follow; every `code_owners.tsv` row still owed is deleted.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `docs.rs`, `inventory_guard.rs`, `spec_codes.rs`
**Requirement**: BN-08
**Executor**: Sonnet

**Done when**:

- [ ] `the_memory_bank_of_bundle_cells_runs` passes under `**Run bookmodules**` and `the_template_card_runs` under `**Run bookcheatsheet**`
- [ ] `the_stream_bundle_and_its_slice_run` passes under `**Run booknatures**`; `gaps_no_longer_lists_bundles` passes under `**Run docs**`; `no_owned_code_is_raised` passes under `**Run speccodes**`; `**Run bookinventory**` passes
- [ ] Gate check passes: Batch (end of B4)

**Tests**: integration
**Gate**: batch

**Commit**: `docs: bundles run`

---

## Checks

**C1** - A struct of carriers, arrays of carriers or bundles is a bundle (BN-01, AC 1)
Proof: run pombundles | test a_struct_of_carriers_is_a_bundle ... ok

**C2** - Directions on one-driver fields (a discrete nature that does not implement `Resolve`) only are accepted (BN-01, AC 2)
Proof: run pombundles | test one_driver_fields_carry_a_direction_and_no_other_field_does ... ok

**C3** - A struct mixing carrier and value fields is `E0260` naming one of each (BN-01, AC 3)
Proof: run pombundles | test a_struct_mixing_carrier_and_value_fields_is_e0260 ... ok

**C4** - A one-driver field without a direction, or a field of a nature that implements `Resolve` or of `Electrical` with one, is `E0261` naming it (BN-01, AC 4)
Proof: run pombundles | test a_misplaced_or_missing_field_direction_is_e0261 ... ok

**C5** - `flip` on a non-bundle, `out` on a bundle port, a direction on a nested bundle field are `E0262` (BN-01, AC 5)
Proof: run pombundles | test flip_out_and_nested_directions_are_e0262 ... ok

**C6** - A bundle as a param type, nature argument, function result, or with an initial value is `E0264` (BN-01, AC 6)
Proof: run pombundles | test a_bundle_declared_as_a_value_is_e0264 ... ok

**C7** - `tx: Stream` drives the `out` fields and reads the `in` fields (BN-02, AC 1)
Proof: run typesbundles | test a_producer_drives_out_fields_and_reads_in_fields ... ok

**C8** - `flip` swaps `in` and `out` on every one-driver field, nested included, and leaves undirected fields (BN-02, AC 2)
Proof: run typesbundles | test flip_swaps_every_one_driver_direction_nested_included ... ok

**C9** - `in s: Stream` makes every one-driver field an input (BN-02, AC 3)
Proof: run typesbundles | test an_observer_reads_every_one_driver_field ... ok

**C10** - Two modules seeing one one-driver field as `out` is `E0220` naming the field path (BN-02, AC 4)
Proof: run elabbundles | test two_producers_of_one_field_is_e0220_naming_its_path ... ok

**C11** - Driving a field the view makes an input is `E0222` (BN-02, AC 5)
Proof: run typesbundles | test driving_an_input_field_is_e0222 ... ok

**C12** - `signal a: Stream;` joining a producer and a consumer declares every carrier and drives none it does not write; a handshake runs (BN-02, AC 6)
Proof: run sourcebundles | test a_producer_consumer_and_observer_run_three_beats ... ok

**C13** - A bundle carrier passed to a bundle port connects each field to the field of the same name (BN-03, AC 1)
Proof: run sourcebundles | test a_bundle_carrier_connects_field_by_field ... ok

**C14** - A literal `Cell { wl: w, bl: b }` connects each named field to its carrier (BN-03, AC 2)
Proof: run sourcebundles | test a_cell_literal_connects_each_named_field ... ok

**C15** - A literal field given a carrier of another nature is `E0263` (BN-03, AC 3)
Proof: run typesbundles | test a_literal_field_of_another_nature_is_e0263 ... ok

**C16** - A literal written anywhere but a connection is `E0264` (BN-03, AC 4)
Proof: run typesbundles | test a_literal_outside_a_connection_or_a_bundle_operand_is_e0264 ... ok

**C17** - `Diff { p, n }` connects its nodes like ports, and probes and contributions on `d.p`, `d.n` work (BN-03, AC 5)
Proof: run sourcebundles | test a_diff_bundle_connects_its_nodes_like_ports ... ok

**C18** - Commits and definitions of a field its view lets it drive follow §5 (BN-04, AC 1)
Proof: run sourcebundles | test field_commits_definitions_and_methods_follow_section_5 ... ok

**C19** - A carrier method on a field applies to that field (BN-04, AC 2)
Proof: run typesbundles | test a_carrier_method_on_a_field_applies_to_that_field ... ok

**C20** - A carrier method on a whole bundle is `E0211` naming bundle and method (BN-04, AC 3)
Proof: run pombundles | test a_carrier_method_on_a_whole_bundle_is_e0211 ... ok

**C21** - `self` on a bundle is a read-only handle (BN-05, AC 1)
Proof: run typesbundles | test self_on_a_bundle_is_a_read_only_handle ... ok

**C22** - `d.vd()` in `analog` gives `v(d.p, d.n)` (BN-05, AC 2)
Proof: run sourcebundles | test vd_on_a_diff_bundle_gives_the_differential_voltage ... ok

**C23** - `on rx.beat(clk)` runs exactly at the rising edges with `valid` and `ready` true (BN-05, AC 3)
Proof: run sourcebundles | test beat_fires_on_rising_edges_with_valid_and_ready ... ok

**C24** - A write through a bundle handle is `E0133` naming the field (BN-05, AC 4)
Proof: run typesbundles | test a_write_through_a_bundle_handle_is_e0133 ... ok

**C25** - Nested bundles, plain or flipped, take the views recursively (BN-06, AC 1)
Proof: run sourcebundles | test nested_bundles_apply_flip_recursively ... ok

**C26** - A field of a nature with `impl Resolve<Driven>` (`Sda`) may be driven by every module that sees the bundle, the observer included, and holds `strongest` of the drives (BN-06, AC 2)
Proof: run sourcebundles | test every_view_may_drive_a_resolved_field ... ok

**C27** - `struct Stream<T: Repr>` is instantiated per type argument (BN-06, AC 3)
Proof: run sourcebundles | test a_generic_bundle_is_instantiated_per_type_argument ... ok

**C28** - An array of bundles connects each element field by field (BN-06, AC 4)
Proof: run sourcebundles | test an_array_of_bundles_connects_each_element_field_by_field ... ok

**C29** - A bundle field is named by its dotted path in `--print`, records and metadata (BN-07, AC 1)
Proof: run apimetadata | test a_bundle_field_is_named_by_its_dotted_path ... ok

**C30** - The POM shows a bundle port with its type and view, not one port per field (BN-07, AC 2)
Proof: run pomreflect | test a_bundle_port_reflects_its_type_and_view ... ok

**C31** - `GAPS.md` lists no L25 and §9 gives Part II §7 its share (BN-08, AC 1)
Proof: run docs | test gaps_no_longer_lists_bundles ... ok

**C32** - No row of `code_owners.tsv` names a raised code, `E0260` to `E0262` and `E0264` included (BN-08, AC 2)
Proof: run speccodes | test no_owned_code_is_raised ... ok

**C33** - A bundle port is never a value input (BN-02, AC 1; §7 rule 9)
Proof: run typesbundles | test a_bundle_port_is_not_a_value_input ... ok

**C34** - The Book's `Stream` bundle and its `Slice` run (BN-08, AC 1; Book natures#8)
Proof: run booknatures | test the_stream_bundle_and_its_slice_run ... ok

**C37** - The Book's `MemoryBank` over the bundle `Cell` and the template card run (BN-08, AC 1; Book modules#10, modules#11, cheatsheet#8; generics C verdict G3)
Proof: run bookmodules | test the_memory_bank_of_bundle_cells_runs ... ok
Proof: run bookcheatsheet | test the_template_card_runs ... ok

**C35** - The Book inventory agrees with `GAPS.md` §11 (BN-08, AC 1)
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C36** - A bundle port lowers to one carrier per leaf field named by path (BN-02, AC 6; §7 rule 14)
Proof: run elabbundles | test a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Settled T5 (B2, 2026-11-24):** branch `feature/resolved-nets-bundles` at `dfa149812` (= `feature/resolved-nets`), `origin/main` merged (already up to date). Orchestrator instruction of 2026-11-24 ("Implement feature resolved-nets-bundles, B2 onward") stands as the go-ahead for B2 onward. Anchors as the code has them today: (1) `types/carrier.rs::port_type` (lower_type first, then `Ty::Node`/`Ty::Carrier`/arrays/value inputs) and `types/aggregate.rs::declare_struct` (the `P0001` "`struct` item used as a type" for a bundle, the pin T6 replaces); `TypeKind::Flip` is `P0001` in `types/check.rs::lower_type_node` via `Unsupported::ty`. (2) A field read of a port is `ExprKind::Field { res: Resolution::Decl(decl) }` read by `declared_port`; carriers are keyed by `DeclId` in the checker (`carriers`) and the elaborator (`Frame::carriers`/`nodes`), so one-leaf-one-declaration. (3) `NatureRules::{one_driver, resolved}` exist (`types/nature_rules.rs`); `pom/resolve/shape.rs` already asks `design.is_resolved_nature` (the capability), and `pom_bundles.rs` declares its own `Wire` with `impl Resolve` (resolved-nets T16 done) - nothing to move. (4) `E0187`/`Default` rules are on the branch; no fixture of this feature's POM target declares a signal, so none needed an initial value (the POM target runs no checker); the checker/elab fixtures T6/T9 write carry initial values. (5) `ports.zip(args)` in `elab/children.rs::lower_child` and `PortArguments::bind` bind by the module's `children`; a bundle port is one argument.
**Settled B1:** B1 (T1-T4) done. Defaults taken (user AFK): `code_owners.tsv` is on the branch after the rebase onto origin/main, so each task deleted its row (E0260 to E0262, E0264) with the GAPS section 10 list and count, and added its row to docs/pom/resolution.md (the `pom_docs` guard). One-driver means a discrete nature without `resolve` (spec part II 5.2), so a signal-flow field takes no direction. A flipped generic or unresolved type, and a field of a generic nature, are not judged by the POM. `E0264` at the declaration covers a param type, a nature's type argument, a function result (not a module result) and an initial value of a bundle signal or port. Two fixtures that declared an invalid bundle were corrected (pom_classify `Port2`, types_buck `Bn`), named in the T1 and T2 bodies. `is_carrier_type` and `has_unresolved_bound` of classify.rs became `pub(super)` for shape.rs.
**Settled re-plan (2026-11-22):** B2 onward amended for the explicit-unknowns model (MD-78; spec amendment A6, design "Amendment of 2026-11-22"): T5 merges `main` after `explicit-unknowns` phase A and gives the fixtures initial values; T6 states the direction rule on the capability; T15 declares `Sda` with `impl Resolve<Driven>`; T17 keeps `E0134` for `impl Trait for` a bundle; gates follow the user's policy (batch-end tasks T8, T16, T21). `resolved-nets` T16 moves `E0261`'s question and the `Wire` fixtures of `pom_bundles.rs`. No question is open.
**Settled T6:** see design.md "Amendment of 2026-11-24" (D11): fields are leaf declarations of `pom/bundles.rs`, no `Ty::Bundle`. `types_buck`/`types_subset` bundle rows left their tables; `std_galleries` and Book natures#8 re-pinned to the new exact refusals (generic bundle `Stream<T>`: T19). `book_claims::every_value_sentence_has_a_claim_row` (natures~1197ebef) fails on the branch before and after T6 (not this feature's).
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → (wait) → Phase 2 → Phase 3 → Phase 4 → Phase 5 → (wait) → Phase 6 → (wait) → Phase 7 → Phase 8

Phase 1:  T1 → T2 → T3 → T4    (done)
Phase 2:  T5
Phase 3:  T6 → T7 → T8
Phase 4:  T9 → T10 → T11
Phase 5:  T12 → T13 → T14 → T15 → T16
Phase 6:  T17 → T18
Phase 7:  T19 → T20
Phase 8:  T21
```

Execution is strictly sequential.

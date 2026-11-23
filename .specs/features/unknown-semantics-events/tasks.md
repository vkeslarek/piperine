# Unknown Semantics: Events Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/unknown-semantics-events/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run irdigital**: `cargo test -p piperine-ir --test digital`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run planshape**: `cargo test -p piperine-plan --test digital_shape`
**Run reaction**: `cargo test -p piperine-transpile --test reaction`
**Run instant**: `cargo test -p piperine-digital-solver --test instant`
**Run checkpoint**: `cargo test -p piperine-digital-solver --test checkpoint`
**Run prototran**: `cargo test -p piperine-solver --test protocol_transient`
**Run protoop**: `cargo test -p piperine-solver --test protocol_operating_point`
**Run typesevents**: `cargo test -p piperine-lang --test types_events`
**Run typesanalog**: `cargo test -p piperine-lang --test types_analog`
**Run elabdigital**: `cargo test -p piperine-lang --test elab_digital`
**Run sourceevents**: `cargo test -p piperine-parity --test source_events`
**Run sourcelimexp**: `cargo test -p piperine-parity --test source_limexp`
**Run paritytran**: `cargo test -p piperine-parity --test parity_tran`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run bookanalog**: `cargo test -p piperine --test book_analog`
**Run bookmixed**: `cargo test -p piperine --test book_mixed`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookvalues**: `cargo test -p piperine --test book_values`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** The spec's open questions were answered 2026-11-18 (user: "Sobre as open questions, vou de aceito todas ok?"). The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D19; every task follows them as written.

**Base and order.** Batches B1 to B4 (phases 1 to 6, T1 to T23) start from `main` at `494a7f75` and need neither `composite-values` nor `generics`. B5 (phases 7 to 9, T24 to T31) starts only once `composite-values` phases A–B and `generics` phases A–B (its T52, L8 closed) are on `main`; T24 rebases B1 to B4 onto that tree. If they are not on `main` when B4 closes, the feature pauses there and reports.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-a TMPDIR=/var/tmp/piperine-tmp-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command. Cargo with `-p <crate>`, never `--workspace` (`piperine-lang-server` is postponed and known broken), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. A new parity comparison uses an existing contract as it is. Every buck target passes after every task that touches the printer, the plan, the digital solver, the owner protocol or the elaborator's lowering of an existing form.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A spec text that is wrong is amended explicitly with its `spec_text` guard, never left. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites (§13, §15, §19, §22.2.4), or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR enum variants and plan `Source` variants are appended last (msl-digital convention). The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/events/*.msl` and `tests/limexp/*.msl`.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-70 (unknown semantics in generated code), MD-72 (root Book tests).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| IR (`piperine-ir`) | integration | the new source built, checked, printed | `crates/piperine-ir/tests/digital.rs` | `cargo test -p piperine-ir --test digital` |
| Interpreter refusals | integration | the new source refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | the `SETTLE` pin and its read; the order of an event function reading another event | `crates/piperine-plan/tests/digital_shape.rs` | `cargo test -p piperine-plan --test digital_shape` |
| Printer (`piperine-transpile`) | integration | the event function compiles and fires as abi.md §4.1 says | `crates/piperine-transpile/tests/reaction.rs` | `cargo test -p piperine-transpile --test reaction` |
| Digital solver | integration | every settle-phase rule and every held-commit transition (hold, land, drop, restore) | `crates/piperine-digital-solver/tests/{instant,checkpoint}.rs` | `cargo test -p piperine-digital-solver --test instant` / `--test checkpoint` |
| Owner protocol (`piperine-solver`) | integration | S1 to S6 on scripted stubs, each a test | `crates/piperine-solver/tests/{protocol_transient,protocol_operating_point}.rs` | `cargo test -p piperine-solver --test protocol_transient` / `--test protocol_operating_point` |
| Checker (`piperine-lang/src/types`) | integration | every static AC; each code with its span | `crates/piperine-lang/tests/{types_events,types_analog}.rs` | `cargo test -p piperine-lang --test types_events` / `--test types_analog` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to | `crates/piperine-lang/tests/elab_digital.rs` | `cargo test -p piperine-lang --test elab_digital` |
| Source-level runs | integration | one test per run-time AC and edge case, from MSL source on the Rust backend; expected values from the rule | `crates/piperine-parity/tests/{source_events,source_limexp}.rs`, models in `tests/events/`, `tests/limexp/` | `cargo test -p piperine-parity --test source_events` / `--test source_limexp` |
| ngspice parity | integration | each comparison under an existing contract | `crates/piperine-parity/tests/{parity_tran,source_limexp}.rs`, `tests/e2e/twins/` | `cargo test -p piperine-parity --test parity_tran` / `--test source_limexp` |
| The Book | integration | each row this feature changes, through the binary and the root package | `tests/book/*.rs`, `tests/book/inventory.tsv`, `crates/piperine-cli/tests/book.rs` | `cargo test -p piperine --test book_<chapter>`, `cargo test -p piperine-cli --test book` |
| Spec text and repository records | integration | one assertion per amended sentence or removed `GAPS.md` row | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs` |
| Contract re-check (T24) | none | - (build gate; its findings on the Handoff) | - | build gate only |

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

### Phase 1: `settle` below the language

```
T1 → T2 → T3 → T4
```

### Phase 2: `settle` from source

```
T5 → T6
```

### Phase 3: Settle commits (§15 S1 to S7)

```
T7 → T8 → T9 → T10 → T11
```

### Phase 4: Events read as `bool`

```
T12 → T13 → T14 → T15 → T16
```

### Phase 5: `limexp`

```
T17 → T18 → T19
```

### Phase 6: The Book's clipper and buck, and the early bookkeeping

```
T20 → T21 → T22 → T23
```

### Phase 7: Contract (waits for `composite-values` A–B and `generics` A–B on `main`)

```
T24
```

### Phase 8: The phase detector

```
T25 → T26
```

### Phase 9: Functions that return `Event`, and closure

```
T27 → T28 → T29 → T30 → T31
```

---

## Task Breakdown

### T1: IR `EventSource::Settle`

**What**: `EventSource::Settle` appended last (the event `settle` of a body, §13 rule 17); `Ir::check` accepts it with or without a guard; the structural printer prints `settle`; the interpreter refuses it by name (R1).
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None
**Reuses**: the `Changed` arms of `check.rs`, `print.rs`; the refusal pattern of `piperine-interp/src/walk.rs`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_event_is_built_checked_and_printed` passes under `**Run irdigital**`
- [x] `a_settle_event_is_refused_by_name` passes under `**Run interp**`
- [x] Code clarity: the variant and every arm it adds carry `///` docs citing §13 rule 17; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): the settle event source`

---

### T2: Plan: the `GLOBAL` `SETTLE` pin and its read

**What**: `Digital::settle_pin(s, body)` adds the `GLOBAL` `SETTLE` event pin when a body has a `Settle` event (as `start_pin` does for `at`); `Source::Settle` (appended last) is the `CHANGED` byte of that pin, read by the event function of each `Settle` event with its guard's sources (D5).
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: T1
**Reuses**: `Digital::start_pin`, `ReadSet::target(Query::Guard)`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_event_reads_the_global_settle_pin` and `a_body_without_settle_has_no_settle_pin` pass under `**Run planshape**`
- [x] Code clarity: `settle_pin` and `Source::Settle` doc-commented; no free function; the data path from event to pin readable in one place
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): the settle pin`

---

### T3: Printer: the event function of `settle`

**What**: `print/digital.rs` prints the event function of a `Settle` event: `fired_ = settle_changed && guard`, the guard printed as for `Changed` (unknown guards under MD-70). Every transpile golden stays byte-identical.
**Where**: `crates/piperine-transpile/src/print/digital.rs`
**Depends on**: T2
**Reuses**: the `EventSource::Changed` arm of the event-function printer
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_event_function_fires_on_the_settle_pin_and_its_guard` passes under `**Run reaction**`
- [x] `cargo test -p piperine-transpile --test golden` unchanged
- [x] Code clarity: the new arm is one dispatch arm into a named method with a `///` doc; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): the settle event function`

---

### T4: The settle phase runs as §15 step 4 says

**What**: In `instant.rs`, after firing the `SETTLE` nets the engine runs the discrete functions of their cone (V2), then the reactions whose event pins fired; a reaction that runs in the settle phase is `on_settle` whatever its head names (§13 rule 14, V3, D4; `run.rs` `fired_reactions` stops deciding it by pin); the phase's records are stamped `(t, n)`, `n` the last microstep run (§19 Resolution, V1, D3).
**Where**: `crates/piperine-digital-solver/src/instant.rs`
**Depends on**: T3
**Reuses**: `run_functions`, `run_reactions`, `fire_net`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_guarded_settle_event_fires_in_the_settle_phase`, `a_reaction_in_the_settle_phase_is_on_settle_whatever_its_head` and `settle_records_carry_the_last_microstep_index` pass under `**Run instant**`
- [x] Code clarity: the settle phase is its own method of the engine (no growth of `microsteps` past one screen), doc-commented with §15 step 4; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(digital-solver): the settle phase runs its functions and stamps its last microstep`

---

### T5: `settle` typed and lowered

**What**: The checker types `settle` as `Event` in every event position (a new `settle_event` method of `types/event.rs`, reached from the `Resolution::Builtin` arm of `types/expr.rs`), `E0441` elsewhere; `elab/events.rs` lowers it to `EventSource::Settle`, alone, guarded (`settle && g`) and in a disjunction (`settle || ev`, §13 rule 14). The Book targets that pin the `settle` refusal go red here and are restored by T6 in the same phase.
**Where**: `crates/piperine-lang/src/elab/events.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Disjunct`, `Checker::infer_event_position`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `settle_is_an_event_in_every_event_position` and `settle_outside_an_event_position_is_e0441` pass under `**Run typesevents**`
- [x] `settle_lowers_to_the_settle_event_source` passes under `**Run elabdigital**`
- [x] Code clarity: the checker and elaborator additions are methods with `///` docs citing §13 rules 10, 14, 17; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): settle is an event`

---

### T6: The Book rows of `settle`

**What**: Every Book listing whose output `settle` changes is the binary's output again: the buck's `piperine run buck.msl ..` listing (mixed#10) is the run, the clipper's refusal listing (analog#3) keeps only the two `limexp` diagnostics; `inventory.tsv` rows follow (the guard prints them); the pins of design "may move" for T6 move: `welcome~a17f566a`'s `on settle` line asserts it runs, cheatsheet#5 runs (cheatsheet#7 keeps its L49 half), mixed#10 and mixed~a6b3f8ed run, the clipper's tests assert the `limexp` refusal alone. Prose is left for T21 and T22.
**Where**: `docs/book/content.html`
**Depends on**: T5
**Reuses**: `Refusal`, `Chapter`, `Commands`
**Requirement**: UE-06
**Executor**: Sonnet

**Done when**:

- [x] `every_cli_listing_of_the_book_is_what_the_binary_prints` and `the_buck_block_is_what_the_binary_prints` pass under `**Run clibook**`
- [x] `cargo test -p piperine --test book_welcome --test book_cheatsheet --test book_analog --test book_mixed` passes
- [x] `every_block_of_the_book_has_one_row` and `refused_rows_and_gaps_section_11_agree` pass under `**Run bookinventory**`
- [x] Code clarity: test helpers stay methods of the support types; each moved assertion cites its Book row
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `test(book): the settle rows run`

---

### T7: `SettleCommits`: a settle commit is held

**What**: New `settle.rs` (`SettleCommits`): a commit with no advancing delay made in the settle phase is held instead of refused (`run.rs`'s "settle commit" refusal leaves); `Outcome::settle_boundary` gives the analog-read nets and the bytes the held commits will give them; `Digital::land_settle_commits(tick)` makes them due at `(tick, 0)`, where `changed()` fires (S1); `Digital::drop_settle_commits()`; `Digital::holds_settle_commits()`; a commit with an advancing delay is an ordinary delayed commit (S7). Replaces `a_settle_commit_with_no_delay_is_refused` (design "may move").
**Where**: `crates/piperine-digital-solver/src/settle.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Planned`, `Queue`, `Resolution::due`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_commit_is_held_until_it_is_landed`, `a_landed_settle_commit_fires_changed_at_microstep_0`, `a_dropped_settle_commit_never_applies` and `a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit` pass under `**Run instant**`
- [x] Code clarity: `SettleCommits` has one purpose and `///` docs on the type, its field and each method, citing §15 S1, S6, S7; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): settle commits are held for the next time point`

---

### T8: A restore brings back the held settle commits

**What**: `Checkpoint` carries `SettleCommits`, so a rejected step that restores the checkpoint keeps the commit for the retried time point (S5) and a step that landed them and is rejected takes the landing back.
**Where**: `crates/piperine-digital-solver/src/engine.rs`
**Depends on**: T7
**Reuses**: `Checkpoint`, `Digital::restore`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_restore_brings_back_the_held_settle_commits` and `a_restore_takes_back_a_landing` pass under `**Run checkpoint**`
- [x] Code clarity: the new field documented; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): a checkpoint holds the settle commits`

---

### T9: The transient owner lands settle commits

**What**: In `protocol/transient.rs`: after an instant whose outcome has a `settle_boundary`, deliver it to the analog side with `Restart::Integrator` and attempt the next step exempt from the truncation test (S2, S3); when a step is accepted and its instant is a time point, call `land_settle_commits(tick_c)` before running it (S1; a forced landing that is not a time point does not land, abi.md R6); the discrete instants in between run on the old value (S4); a rejected step restores the checkpoint (S5); at the stop time held commits are dropped (S6). `DiscreteSide` gains `land_settle_commits`/`drop_settle_commits`, its adapter in `analyses/adapters.rs`.
**Where**: `crates/piperine-solver/src/protocol/transient.rs`
**Depends on**: T8
**Reuses**: `Restart`, `Attempt`, `Advance::Landing { time_point }`
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_commit_lands_at_the_next_time_point`, `the_step_after_a_settle_commit_is_exempt_and_restarts`, `an_event_time_before_the_next_time_point_reads_the_old_value`, `a_rejected_step_moves_the_settle_commit` and `a_settle_commit_at_the_stop_time_is_dropped` pass under `**Run prototran**`
- [x] Code clarity: the landing is a named method of the protocol (the step loop gains one call, not a block), `///` docs citing S1 to S6; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the transient lands settle commits at the next time point`

---

### T10: The operating point and `Dc` points land settle commits

**What**: In `protocol/operating_point.rs`: held settle commits from the previous sweep point land at microstep 0 of the next point's first instant (S6); those of the operating point before a transient land at its first time point (O4, through T9's path); after the last point of a sweep they are dropped.
**Where**: `crates/piperine-solver/src/protocol/operating_point.rs`
**Depends on**: T9
**Reuses**: T9's `DiscreteSide` methods
**Requirement**: UE-03
**Executor**: Sonnet

**Done when**:

- [x] `a_settle_commit_of_a_dc_point_lands_at_the_next_point` and `a_settle_commit_of_the_last_point_is_dropped` pass under `**Run protoop**`
- [x] Code clarity: one named method per rule, doc-commented with §15 S6 and §16 O4; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): sweep points land settle commits`

---

### T11: The Independent Test of `on settle`

**What**: New `crates/piperine-parity/tests/source_events.rs` (with its `//!` header) and models in `tests/events/`: an RC transient with `on settle { emit!(..) }` compared with the run's accepted steps; a settle commit read by a `changed()` reaction at the next time point; a `dc` sweep and an operating point; records ordered after every microstep; `settle || ev`; a design with nothing analog; a probing settle reaction; a jump.
**Where**: `crates/piperine-parity/tests/source_events.rs`
**Depends on**: T10
**Reuses**: `piperine_parity::project::Project`, the `source_unknowns.rs` harness shape
**Requirement**: UE-03, UE-01
**Executor**: Sonnet

**Done when**:

- [x] `settle_runs_once_per_accepted_time_point`, `settle_does_not_run_at_an_event_time_that_is_not_a_time_point`, `settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant`, `settle_records_follow_every_microstep_of_their_time_point`, `settle_combined_with_an_event_fires_by_rule_1`, `a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record`, `settle_runs_once_per_time_point_across_a_jump`, `a_probing_settle_reaction_reads_the_time_point_solution` and `settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop` pass under `**Run sourceevents**`
- [x] `a_settle_commit_is_read_at_the_next_time_point`, `an_event_after_the_landing_reads_the_settle_commit`, `a_delayed_settle_commit_is_ordinary` and `a_settle_commit_at_the_stop_time_is_dropped_from_source` pass under `**Run sourceevents**`
- [x] `a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it` passes under `**Run sourceevents**` (needs events read as `bool`: moved to T14, Settled B2 (9); added with T14)
- [x] Code clarity: test fixtures are methods of a named scenario type; no free test helper
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): settle and settle commits from source`

---

### T12: Checker: an event read as a `bool` in a reaction body

**What**: In `types/event.rs`: in a reaction body an `Event` where a `bool` is expected types as `bool` and is recorded as an event read (`Types::event_read(expr)`), a commit's value onto a `bool` carrier included (D2); an `Event` where another type is expected is `E0441`. `docs/spec/part_iv_behavior.md` §13 gains the Resolution of D2, with its `spec_text` guard.
**Where**: `crates/piperine-lang/src/types/event.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `event_out_of_place`, `Checker::committing`
**Requirement**: UE-01, UE-02
**Executor**: Sonnet

**Done when**:

- [x] `an_event_reads_as_a_bool_in_a_reaction_body`, `an_event_committed_onto_a_bool_carrier_reads_as_a_bool` and `an_event_where_another_type_is_expected_is_e0441` pass under `**Run typesevents**`
- [x] `the_commit_of_an_event_read_as_a_bool_is_stated` passes under `**Run spectext**`
- [x] Code clarity: `event_out_of_place` is split, not grown: one method per outcome, each doc-commented with its rule
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): an event reads as a bool in a reaction body`

---

### T13: Checker: guards read events

**What**: In `types/event.rs`: inside a guard an `Event` subexpression and `!ev` read as `bool` (§13 rule 9), recorded as event reads; `!` on an `Event` outside a guard or a reaction body, and `bool && Event` in an event position, stay `E0440`.
**Where**: `crates/piperine-lang/src/types/event.rs`
**Depends on**: T12
**Reuses**: `negated_event`, `logical`
**Requirement**: UE-02
**Executor**: Sonnet

**Done when**:

- [x] `a_guard_reads_an_event_as_a_bool`, `negating_an_event_outside_a_guard_or_body_is_e0440` and `a_bool_and_an_event_in_an_event_position_is_e0440` pass under `**Run typesevents**`
- [x] Code clarity: no method grows past one screen; `///` docs cite §13 rules 1 and 9
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): guards read events`

---

### T14: Elaborator: an event read lowers to `fired(k)`

**What**: An event read in a reaction body lowers to `Expr::Fired(k)`, `k` the equal event of the body (the head's first) or a new event appended to the body's events and to no reaction's `on` (D7), in `elab/events.rs` or a new `elab/event_reads.rs` (`EventReads`) when `events.rs` would pass 600 lines. Replaces `a_head_the_ir_cannot_hold_is_p0001` (design "may move").
**Where**: `crates/piperine-lang/src/elab/events.rs`
**Depends on**: T13
**Reuses**: `Expr::Fired`, `Disjunct`
**Requirement**: UE-01, UE-02
**Executor**: Sonnet

**Done when**:

- [x] `an_event_read_as_a_bool_in_a_body_lowers_to_fired` passes under `**Run elabdigital**`
- [x] `an_event_read_as_a_bool_says_whether_it_fired`, `two_events_in_one_microstep_run_the_reaction_once_both_true`, `an_event_not_in_the_head_reads_whether_it_fired` and `a_committed_event_read_commits_whether_it_fired` pass under `**Run sourceevents**`
- [x] Code clarity: the lookup-or-append is one named type or method with a `///` doc citing §13 rule 2; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): an event read as a bool is fired`

---

### T15: Elaborator and plan: a guard reads events

**What**: The event subexpressions of a guard lower to `fired(j)` of a body event; the plan orders event `j`'s function before the function of the event whose guard reads it (D8) where its dependency set does not already.
**Where**: `crates/piperine-lang/src/elab/events.rs`
**Depends on**: T14
**Reuses**: T14's lookup, `ReadSet`
**Requirement**: UE-02
**Executor**: Sonnet

**Done when**:

- [x] `a_guard_reading_events_fires_only_when_d_changed_with_en_high_and_still` and `a_guard_reads_what_the_reaction_would_read` pass under `**Run sourceevents**`
- [x] `an_event_function_reading_another_event_runs_after_it` passes under `**Run planshape**`
- [x] Code clarity: `///` docs cite §13 rule 9; no free function
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): guards read events`

---

### T16: The Book rows of events read as `bool`

**What**: mixed#6 (`Comparator`, `on up || dn { q <- up; }`) and mixed#15 run or, where another gap still refuses them, are re-pointed to it (D15); digital#6 (`Pfd3`) is now refused for `let mut` alone and is re-pointed to L5 until T26; their `inventory.tsv` rows and `tests/book/{mixed,digital}.rs` follow (design "may move").
**Where**: `tests/book/mixed.rs`
**Depends on**: T15
**Reuses**: `Refusal`, `Chapter`
**Requirement**: UE-01, UE-02
**Executor**: Sonnet

**Done when**:

- [x] `the_comparator_without_hysteresis_reads_its_event_as_a_bool` passes under `**Run bookmixed**`
- [x] `**Run bookdigital**`, `**Run bookinventory**` and `**Run clibook**` pass
- [x] Code clarity: each moved assertion cites its Book row and rule
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `test(book): the rows of events read as bools`

---

### T17: Checker: closure bindings in a module

**What**: The module item walk of `types/check.rs` skips `DeclKind::Binding` children (closure parameters), typed with their closure (D9); the "binding inside a module" refusal leaves for them.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the `GenericParam` skip arm
**Requirement**: UE-05
**Executor**: Sonnet

**Done when**:

- [x] `limexp_checks_with_its_closure_bindings` passes under `**Run typesanalog**`
- [x] Code clarity: the arm carries a comment citing §22.2.4 and D9
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a closure's bindings are not module items`

---

### T18: `limexp` runs from source, against `pnjlim` and ngspice

**What**: New `crates/piperine-parity/tests/source_limexp.rs` (with its `//!` header) and models in `tests/limexp/`: `limexp` elaborates from the stdlib; a diode written with `limexp(v / nvt)` and one with `exp` under `pnjlim` agree at the operating point under `DC_VOLTAGE`; the `diode_series` twin with its diodes written with `limexp` agrees with ngspice's `diode_series.cir` under `DC_VOLTAGE` (D11).
**Where**: `crates/piperine-parity/tests/source_limexp.rs`
**Depends on**: T17
**Reuses**: `parity_op.rs`'s `OperatingPoint` compare, `Ngspice`, `Project`
**Requirement**: UE-05
**Executor**: Sonnet

**Done when**:

- [x] `limexp_elaborates_from_the_stdlib_with_its_closure`, `limexp_and_pnjlim_diodes_agree_at_the_operating_point` and `the_limexp_diode_series_twin_agrees_with_ngspice` pass under `**Run sourcelimexp**`
- [x] No `ToleranceContract` edited
- [x] Code clarity: the comparison is a method of a named fixture type; no free test helper
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): limexp runs and agrees with ngspice`

---

### T19: `limexp` limits and differentiates as §22.2.4 says

**What**: Through the compiled diode's ABI call (D10): with `old = 0` the argument after `limit` for `new = 30` is 1.0 (rule 3); at a converged point the partial of the current is `is · limexp(x) / nvt` (rule 5).
**Where**: `crates/piperine-parity/tests/source_limexp.rs`
**Depends on**: T18
**Reuses**: the ABI caller of `crates/piperine-abi/tests/caller.rs`
**Requirement**: UE-05
**Executor**: Sonnet

**Done when**:

- [x] `limexp_moves_its_argument_at_most_one_per_iteration` and `the_derivative_of_limexp_is_limexp` pass under `**Run sourcelimexp**`
- [x] Code clarity: expected values computed by Rust in the test from §22.2.4
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): limexp limits its argument and differentiates through limit`

---

### T20: The clipper's transient twin

**What**: `tests/e2e/twins/tran_clipper.cir` (2 V, 1 kHz sine, 1 kΩ, two antiparallel `D` diodes, `.tran` to 2 ms) and `tran_clipper.msl` (the Book's analog part with `limexp` diodes), no `.command` (D11); `parity_tran.rs` gains `tran_clipper`, `v(vout)` under `TRAN_VOLTAGE` on the compiled backends.
**Where**: `crates/piperine-parity/tests/parity_tran.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `TranRun`, `compare_with`
**Requirement**: UE-06
**Executor**: Sonnet

**Done when**:

- [x] `tran_clipper` passes under `**Run paritytran**`
- [x] `cargo test -p piperine-cli --test twins` still counts sixteen twins
- [x] Code clarity: the new test reuses `TranRun`; no free helper added
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the clipper agrees with ngspice`

---

### T21: The Book's clipper runs

**What**: The clipper's "Run it" listing (analog#3) is the binary's output: two `clip` records, one per period, `vin` within 50 mV of 0.72 V; the sentence "That is, once `settle` and `limexp` are compiled .. runs nothing" is rewritten to what runs (UE-07 AC 2); analog#2 and analog#3 rows and their tests follow (design "may move", D15 for what L49 still refuses); `the_clipper_refusal_no_longer_names_sin` becomes `the_clipper_runs_as_the_book_prints`.
**Where**: `tests/book/analog.rs`
**Depends on**: T20
**Reuses**: `Chapter`, `Commands::clipper`
**Requirement**: UE-06, UE-07
**Executor**: Sonnet

**Done when**:

- [x] `the_clipper_prints_one_clip_record_per_period_near_0_72_volts` passes under `**Run bookanalog**`
- [x] `the_clipper_runs_as_the_book_prints` and `every_cli_listing_of_the_book_is_what_the_binary_prints` pass under `**Run clibook**`
- [x] `**Run bookinventory**` passes
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs(book): the clipper runs`

---

### T22: The Book's chapter-8 buck check runs

**What**: `the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28` asserts every `regulated` check record after 10 ms is `pass = true` (UE-06 AC 2; if one is not, D16: the row becomes `diverges` with the measured margin and the task reports it); the Book's sentences that say `settle` is not compiled (the buck's "Run it", the chapter status paragraph, the introduction's list of refusals) are rewritten to what runs (UE-07 AC 2).
**Where**: `tests/book/mixed.rs`
**Depends on**: T21
**Reuses**: `Commands::buck`
**Requirement**: UE-06, UE-07
**Executor**: Sonnet

**Done when**:

- [x] `the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28` passes under `**Run bookmixed**`
- [x] `the_book_no_longer_says_settle_is_not_compiled` passes under `**Run docs**`
- [x] `**Run clibook**` and `**Run bookinventory**` pass
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs(book): the buck's check runs`

---

### T23: Early bookkeeping

**What**: `GAPS.md` drops L52 and `limexp` from L58, §1's "stdlib that runs" names `settle` and `limexp`, §11 rows follow T6/T16/T21/T22, §9 Coverage updates IV §13 and V §15; `docs.rs` asserts the rows gone and left; `.specs/STATE.md` gains the next free `MD-` entry (design "Tech Decisions", D17); the ROADMAP row says B1–B4 delivered and B5 waits.
**Where**: `GAPS.md`
**Depends on**: T22
**Reuses**: `docs.rs` row helpers
**Requirement**: UE-07
**Executor**: Sonnet

**Done when**:

- [x] `gaps_no_longer_lists_settle_or_limexp` passes under `**Run docs**`
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: settle and limexp run`

---

### T24: Rebase and re-read the anchors

**What**: Once `composite-values` A–B and `generics` A–B (L8, its T52) are on `main` (if not, stop and report), rebase `feature/unknown-semantics-events` onto it and re-read every anchor B5 uses: `let mut`/assignment in a reaction, `types/handles.rs`, `elab/handles.rs` (`HandleBinding`, `Scope::handles`), `Types::handle`, the function-class inference, `elab/events.rs` after B1–B4, the next free `MD-` number. Write each difference and how T25 to T31 map onto it on the Handoff's `**Settled mid-build:**` line. No code change.
**Where**: `.specs/features/unknown-semantics-events/tasks.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md "What the code does today"
**Requirement**: UE-01, UE-04
**Executor**: Sonnet

**Done when**:

- [x] Rebased, every B1–B4 check still passes under its `Run` line
- [x] The Handoff names each moved anchor
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): unknown-semantics-events B5 anchors after the rebase`

---

### T25: `Pfd3` runs

**What**: The `Pfd3` of §13, verbatim, in `tests/events/pfd3.msl`, driven by timed commits that rise apart and together; `up`/`dn` printed; rising together leaves `state` unchanged (UE-01 AC 3).
**Where**: `crates/piperine-parity/tests/source_events.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: T11's scenario type
**Requirement**: UE-01
**Executor**: Sonnet

**Done when**:

- [x] `pfd3_rising_together_leaves_state_unchanged` and `pfd3_rising_apart_steps_up_then_down` pass under `**Run sourceevents**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the three-state phase detector`

---

### T26: The Book's `Pfd3` row

**What**: digital#6 runs; its row, test and `inventory.tsv` entry follow. (Amended at T24: `composite-values` T11 already made the row `run` under `the_phase_detector_runs_assignment_and_events_read_as_bools`; this task verifies it, and the Book text is the `Pfd3` T25 runs from source verbatim.)
**Where**: `tests/book/digital.rs`
**Depends on**: T25
**Reuses**: `Chapter`
**Requirement**: UE-01
**Executor**: Sonnet

**Done when**:

- [x] `the_phase_detector_runs_assignment_and_events_read_as_bools` passes under `**Run bookdigital**`
- [x] `**Run bookinventory**` passes
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `test(book): the phase detector runs`

---

### T27: Checker: functions that return `Event`

**What**: In `types/event.rs`: a `fn` whose result is `Event` has its result in an event position; its call is an `Event` in an event position, a guard and a reaction body (read as `bool` there, T12/T13); elsewhere `E0441`; the "method `rise` on this receiver" refusal leaves for a handle receiver (generics' handles).
**Where**: `crates/piperine-lang/src/types/event.rs`
**Depends on**: None (its phase follows the previous one)
**Amended (T27)**: the checker already accepts and types every form: the handle receiver of `req.rise()` (generics A-B: `carrier_named`/`declared_port` of a handle parameter) and the `Event` result of a function needed no change, so this task is the tests that pin it (anchor note 2b of Settled B5 is moot for the checker); the refusal lives in the elaborator (T28).
**Reuses**: `Types::handle`, `infer_event_position`
**Requirement**: UE-04
**Executor**: Sonnet

**Done when**:

- [x] `a_function_returning_event_is_accepted_in_every_event_position` and `a_function_returning_event_outside_an_event_position_is_e0441` pass under `**Run typesevents**`
- [x] Code clarity: `///` docs cite §13 rule 7; no free function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): functions that return Event`

---

### T28: Elaborator: an `Event` function is inlined where it stands

**What**: A call of a `fn -> Event` is inlined through `HandleBinding` (D14), its result lowered by `elab/events.rs` into the caller's head, body `let`, guard or body read; a nested `Event` function nests the inlining.
**Where**: `crates/piperine-lang/src/elab/event_functions.rs` (new, called from the `Call` arm of `disjuncts` in `elab/events.rs`; no `HandleBinding`, see Settled B5 (2b): the parameters bind in `Scope::handles`/`texts`/`values` under `Environment`)
**Depends on**: T27
**Reuses**: `HandleBinding`, `Scope::handles`
**Requirement**: UE-04
**Executor**: Sonnet

**Done when**:

- [x] `an_event_function_call_lowers_to_its_body_event` passes under `**Run elabdigital**`
- [x] Code clarity: `///` docs; no free function; no god method
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): event functions are inlined`

---

### T29: The Independent Test of `Event` functions

**What**: `tests/events/handshake.msl`: `handshake` in a head, a body `let`, a guard and a body read, driven by timed commits; a function calling another.
**Where**: `crates/piperine-parity/tests/source_events.rs`
**Depends on**: T28
**Reuses**: T11's scenario type
**Requirement**: UE-04
**Executor**: Sonnet

**Done when**:

- [x] `handshake_fires_when_req_rises_with_ack_high`, `an_event_function_in_every_event_position`, `an_event_function_read_as_a_bool_says_whether_it_fired` and `an_event_function_calling_another_fires_under_the_outer_guard` pass under `**Run sourceevents**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): event functions from source`

---

### T30: The Book's `handshake` rows

**What**: values#2 and values~c389077a run; their rows, tests and `inventory.tsv` entries follow.
**Where**: `tests/book/values.rs`
**Depends on**: T29
**Reuses**: `Chapter`
**Requirement**: UE-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_function_has_the_class_its_body_gives` passes under `**Run bookvalues**`
- [ ] `**Run bookinventory**` passes
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `test(book): event functions run`

---

### T31: Closing bookkeeping

**What**: `GAPS.md` drops L51 and L53, §9 and §11 follow; `docs.rs` asserts it; the `MD-` entry of T23 is completed with event functions; the ROADMAP row says delivered.
**Where**: `GAPS.md`
**Depends on**: T30
**Reuses**: `docs.rs` row helpers
**Requirement**: UE-07
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_no_longer_lists_l51_or_l53` passes under `**Run docs**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: unknown-semantics-events closes L51 and L53`

---

## Checks

**C1** - An event read as a `bool` in a reaction body is `true` exactly when it fired in that microstep. (UE-01, AC 1; T14)
Proof: run sourceevents | test an_event_read_as_a_bool_says_whether_it_fired ... ok

**C2** - Two events of one head firing in one microstep run the reaction once, both read as `true`. (UE-01, AC 2; T14)
Proof: run sourceevents | test two_events_in_one_microstep_run_the_reaction_once_both_true ... ok

**C3** - `Pfd3` with both clocks rising in one microstep leaves `state` unchanged. (UE-01, AC 3; T25)
Proof: run sourceevents | test pfd3_rising_together_leaves_state_unchanged ... ok

**C4** - In the settle phase an event read as a `bool` is `true` only if `settle` makes it fire; `x.changed()` there is `false`. (UE-01, AC 4; edge case; T11)
Proof: run sourceevents | test a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it ... ok

**C5** - `on d.changed() && en && !en.changed()` fires only when `d` changed, `en` is true and did not change. (UE-02, AC 1; T15)
Proof: run sourceevents | test a_guard_reading_events_fires_only_when_d_changed_with_en_high_and_still ... ok

**C6** - A guard reading an event reads what the reaction would read in that microstep. (UE-02, AC 2; T15)
Proof: run sourceevents | test a_guard_reads_what_the_reaction_would_read ... ok

**C7** - `!` on an `Event` outside a guard or a reaction body is `E0440`. (UE-02, AC 3; T13)
Proof: run typesevents | test negating_an_event_outside_a_guard_or_body_is_e0440 ... ok

**C8** - `bool && Event` in an event position is `E0440`. (UE-02, AC 3; T13)
Proof: run typesevents | test a_bool_and_an_event_in_an_event_position_is_e0440 ... ok

**C9** - An `Event` where a type other than `bool` is expected, or outside an event position, a guard or a body, is `E0441`. (UE-02, AC 4; T12)
Proof: run typesevents | test an_event_where_another_type_is_expected_is_e0441 ... ok

**C10** - `on up || dn { q <- up; }` commits whether `up` fired. (UE-02, AC 5; T14)
Proof: run sourceevents | test a_committed_event_read_commits_whether_it_fired ... ok

**C11** - §13 states that a commit of an event read as a `bool` commits the `bool` (D2). (UE-02, AC 5; T12)
Proof: run spectext | test the_commit_of_an_event_read_as_a_bool_is_stated ... ok

**C12** - Reactions on `settle` run once per accepted time point, reading the final state and the solution. (UE-03, AC 1; T11)
Proof: run sourceevents | test settle_runs_once_per_accepted_time_point ... ok

**C13** - `settle` does not run at an event time that is not a time point. (UE-03, AC 2; T11)
Proof: run sourceevents | test settle_does_not_run_at_an_event_time_that_is_not_a_time_point ... ok

**C14** - A settle commit with no delay is due at microstep 0 of the next time point and read by the step that arrives there. (UE-03, AC 3; T9, T11)
Proof: run sourceevents | test a_settle_commit_is_read_at_the_next_time_point ... ok
Proof: run prototran | test a_settle_commit_lands_at_the_next_time_point ... ok

**C15** - The step after a settle commit is exempt from the truncation test and restarts the integrator. (UE-03, AC 3; T9)
Proof: run prototran | test the_step_after_a_settle_commit_is_exempt_and_restarts ... ok

**C16** - An event time strictly between a settle commit and the next time point reads the old value. (UE-03, AC 4; T9, T11)
Proof: run prototran | test an_event_time_before_the_next_time_point_reads_the_old_value ... ok

**C17** - A settle-phase commit with a delay that advances time is an ordinary delayed commit. (UE-03, AC 5; T7, T11)
Proof: run instant | test a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit ... ok
Proof: run sourceevents | test a_delayed_settle_commit_is_ordinary ... ok

**C18** - A rejected step moves the settle commit with the retried time point. (UE-03, AC 6; T8, T9)
Proof: run prototran | test a_rejected_step_moves_the_settle_commit ... ok
Proof: run checkpoint | test a_restore_brings_back_the_held_settle_commits ... ok

**C19** - A settle commit made at the stop time is dropped. (UE-03, AC 7; T9, T11)
Proof: run prototran | test a_settle_commit_at_the_stop_time_is_dropped ... ok
Proof: run sourceevents | test a_settle_commit_at_the_stop_time_is_dropped_from_source ... ok

**C20** - Settle-phase records follow every microstep of their time point, stamped with `t` and the last microstep's index. (UE-03, AC 8; T4, T11)
Proof: run instant | test settle_records_carry_the_last_microstep_index ... ok
Proof: run sourceevents | test settle_records_follow_every_microstep_of_their_time_point ... ok

**C21** - `settle` runs at the operating point, at each `Dc` point and at the last instant; a `Dc` point's settle commit lands at the next point. (UE-03, AC 9; T10, T11)
Proof: run sourceevents | test settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant ... ok
Proof: run protoop | test a_settle_commit_of_a_dc_point_lands_at_the_next_point ... ok

**C22** - `settle || ev` and `settle && g` fire by §13 rule 1, and the phase decides when commits are due. (UE-03, AC 10; T4, T11)
Proof: run sourceevents | test settle_combined_with_an_event_fires_by_rule_1 ... ok
Proof: run instant | test a_reaction_in_the_settle_phase_is_on_settle_whatever_its_head ... ok

**C23** - A function returning `Event` is accepted in every event position. (UE-04, AC 1; T27, T29)
Proof: run typesevents | test a_function_returning_event_is_accepted_in_every_event_position ... ok
Proof: run sourceevents | test an_event_function_in_every_event_position ... ok

**C24** - `handshake` heads a reaction that runs exactly when `req` rises with `ack` high. (UE-04, AC 2; T29)
Proof: run sourceevents | test handshake_fires_when_req_rises_with_ack_high ... ok

**C25** - An `Event` function read as a `bool` in a body says whether it fired. (UE-04, AC 3; T29)
Proof: run sourceevents | test an_event_function_read_as_a_bool_says_whether_it_fired ... ok

**C26** - An `Event` function called outside an event position, a guard or a body is `E0441`. (UE-04, AC 4; T27)
Proof: run typesevents | test a_function_returning_event_outside_an_event_position_is_e0441 ... ok

**C27** - `limexp` checks and elaborates from the stdlib with its closure. (UE-05, AC 1; T17, T18)
Proof: run typesanalog | test limexp_checks_with_its_closure_bindings ... ok
Proof: run sourcelimexp | test limexp_elaborates_from_the_stdlib_with_its_closure ... ok

**C28** - A `limexp` diode and an `exp` diode under `pnjlim` agree at the operating point under `DC_VOLTAGE`. (UE-05, AC 2; T18)
Proof: run sourcelimexp | test limexp_and_pnjlim_diodes_agree_at_the_operating_point ... ok

**C29** - `limexp` moves its argument by at most 1.0 per iteration. (UE-05, AC 3; T19)
Proof: run sourcelimexp | test limexp_moves_its_argument_at_most_one_per_iteration ... ok

**C30** - The derivative of `limexp(x)` is `limexp(x)`. (UE-05, AC 4; T19)
Proof: run sourcelimexp | test the_derivative_of_limexp_is_limexp ... ok

**C31** - The `diode_series` twin written with `limexp` agrees with ngspice under `DC_VOLTAGE`. (UE-05, AC 5; T18)
Proof: run sourcelimexp | test the_limexp_diode_series_twin_agrees_with_ngspice ... ok

**C32** - The Book's clipper prints one `clip` record per period with `vin` within 50 mV of 0.72 V, and one `wave` record per time point. (UE-06, AC 1; T21)
Proof: run bookanalog | test the_clipper_prints_one_clip_record_per_period_near_0_72_volts ... ok

**C33** - The Book's chapter-8 buck completes with every `regulated` record `pass = true`. (UE-06, AC 2; T22)
Proof: run bookmixed | test the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28 ... ok

**C34** - Every Book listing of these runs is the binary's output. (UE-06, AC 3; T6, T21)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok
Proof: run clibook | test the_clipper_runs_as_the_book_prints ... ok

**C35** - The clipper's `v(vout)` agrees with its ngspice twin under `TRAN_VOLTAGE`. (UE-06, AC 4; T20)
Proof: run paritytran | test tran_clipper ... ok

**C36** - `GAPS.md` no longer lists L52 or `limexp`, then L51 and L53, and §1 names `settle` and `limexp`. (UE-07, AC 1; T23, T31)
Proof: run docs | test gaps_no_longer_lists_settle_or_limexp ... ok
Proof: run docs | test gaps_no_longer_lists_l51_or_l53 ... ok

**C37** - The Book no longer says `settle` is not compiled. (UE-07, AC 2; T22)
Proof: run docs | test the_book_no_longer_says_settle_is_not_compiled ... ok

**C38** - A time point with no settle reaction adds no microstep and no record. (Edge case; T11)
Proof: run sourceevents | test a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record ... ok

**C39** - Across a jump `settle` runs once per time point, reading that time point's solution. (Edge case; T11)
Proof: run sourceevents | test settle_runs_once_per_time_point_across_a_jump ... ok

**C40** - An `Event` function calling another fires under the outer guard. (Edge case; T29)
Proof: run sourceevents | test an_event_function_calling_another_fires_under_the_outer_guard ... ok

**C41** - A probing settle reaction reads its time point's solution and makes no time point. (Edge case; T11)
Proof: run sourceevents | test a_probing_settle_reaction_reads_the_time_point_solution ... ok

**C42** - In a design with nothing analog `settle` runs at the operating point and at the stop. (Edge case; T11)
Proof: run sourceevents | test settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop ... ok

**C43** - A body read of an event not in the head says whether it fired. (Edge case; T14)
Proof: run sourceevents | test an_event_not_in_the_head_reads_whether_it_fired ... ok

**C44** - `settle` is an IR event source, refused by name by the interpreter, given the `GLOBAL` `SETTLE` pin by the plan and an event function by the printer; it is typed `Event` in every event position and lowered to that source. (UE-03, AC 1; T1, T2, T3, T5)
Proof: run irdigital | test a_settle_event_is_built_checked_and_printed ... ok
Proof: run interp | test a_settle_event_is_refused_by_name ... ok
Proof: run planshape | test a_settle_event_reads_the_global_settle_pin ... ok
Proof: run reaction | test a_settle_event_function_fires_on_the_settle_pin_and_its_guard ... ok
Proof: run typesevents | test settle_is_an_event_in_every_event_position ... ok
Proof: run elabdigital | test settle_lowers_to_the_settle_event_source ... ok

**C45** - An event read as a `bool` lowers to `fired(k)`; a guard's event function runs after the event it reads. (UE-01, AC 1; UE-02, AC 2; T14, T15)
Proof: run elabdigital | test an_event_read_as_a_bool_in_a_body_lowers_to_fired ... ok
Proof: run planshape | test an_event_function_reading_another_event_runs_after_it ... ok
Proof: run elabdigital | test an_event_function_call_lowers_to_its_body_event ... ok

**C46** - The Book's rows of this feature hold: `Pfd3` and `handshake` run, every block keeps one row and §11 agrees with the refused rows. (UE-01, AC 3; UE-04, AC 2; UE-06, AC 3; T16, T26, T30)
Proof: run bookdigital | test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok
Proof: run bookvalues | test a_function_has_the_class_its_body_gives ... ok
Proof: run bookmixed | test the_comparator_without_hysteresis_reads_its_event_as_a_bool ... ok
Proof: run bookinventory | test every_block_of_the_book_has_one_row ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("always take the most reasonable default and tell later"); the defaults are design.md D1 to D19.
**Settled B1 (T1 to T6, 2026-11-20, user AFK):** (1) between T1 and T3 the plan and the transpile printer failed loud or planned nothing for `EventSource::Settle` so each commit builds; T2 and T3 replaced them. (2) the interpreter names `settle` among its `amended_forms`. (3) the `SETTLE` pin is the last pin of a body (after the derivative abstol pins), so no existing shape moves. (4) `fired_reactions` and `run_reactions` take a `Phase` (`Microstep` or `Settle`); the two call sites of `tests/reactions.rs` pass `Phase::Microstep`. (5) beyond the tasks: a commit to a RECORD pin in the settle phase is no settle commit and applies as the instant exits (found by the Book buck check, §19 rules 7 and 8; commit `fix(digital-solver): a record from the settle phase is no settle commit`, with its tests). (6) refusal pins superseded by acceptance tests and named in the T5 commit body: the `settle` rows of `types_events`, `types_buck`, `types_subset`, `elab_unlowered`. (7) Book: the cheat card of cheatsheet#5 commits its pulse to `q` (a `Bit`), not to the node `p` the check probes (E0222 otherwise); the buck listing of mixed#13 holds the whole 1217-line output, the page shows lines 1 to 12; tests renamed (`the_clipper_is_refused_for_limexp`, `the_digital_card_runs`, `the_debug_prints_card_is_refused_for_the_analog_trace`, `the_math_methods_and_the_settle_event_run`) and their inventory rows and the GAPS section 11 rows follow (analog#3 and analog~773a3851 re-pointed to L9, mixed#10, mixed~9d07c4b9 and cheatsheet#5 left). (8) prose left stale for T21 and T22 as the task says: the welcome status sentence, analog~773a3851 ("three diagnostics"), the clipper and buck "not compiled yet" sentences, mixed~a6b3f8ed. (9) the buck check holds: no failing `regulated` record from 10 ms to 12 ms (D16 not triggered). (10) commits end with the trailer tasks.md prescribes (Claude Opus 5.5).
**Settled B2 (T7 to T11, 2026-11-20, user AFK):** (1) `SettleCommits` holds `queue::Pending` plus the REPLACE flag (not `Planned`, which lacks writer and call); it is `pub` because `run_reactions`, a public function, takes it (`&mut SettleCommits`, ninth parameter; the two call sites of `tests/reactions.rs` pass `SettleCommits::default()`). (2) A held commit's `Observation::Commit` reports `due: None` (it has no instant until landed). (3) `Mark` (the journal rewind the owner protocol actually uses) holds the settle commits as well as `Checkpoint` (T8 says Checkpoint only; the protocol restores through `Mark`). (4) `Digital::start_at` keeps the held commits: a sweep point's commits land at the next point's `operating_point` (S6); they are dropped by `Protocol::end_sweep`, called by `DcSolver::solve` unless `followed_by_another_point(true)`, which the api's `Dc` sweep driver passes for every point but the last; a lone operating point, `Ac`, noise and the other analyses that take one therefore drop theirs at its end; a transient's operating point keeps them for the first time point (O4). (5) The transient drops held commits at the end of `transient()` (the stop), so the continuation case of S6 ("unless the transient is continued") is not implemented (G.34 stays open). (6) `Protocol::exempt_next` carries the S3 exemption across a rejected retry and clears it when a step is accepted. (7) `DiscreteSide` gained `land_settle_commits`/`drop_settle_commits` as required methods (stub and `DigitalSide` implement them); `Outcome::settle_boundary` (protocol side) is `Vec<Boundary>`, the adapter moves the analog copy as for `boundary`. (8) The removed test `a_settle_commit_with_no_delay_is_refused` is listed in `solver-abi-integration/removed.txt` (a guard requires it). (9) T11: the criterion `a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it` cannot pass in T11: it reads `settle` as a `bool` in a reaction body, which the checker refuses (`E0606`) until T12 to T14; it is NOT in `source_events.rs` and is added by T14 with the other event reads (D2). All other 13 T11 tests are in. Source-level tests compare accepted steps with records to one tick (the analog side's own step time and a record's tick round a tick apart). (10) Environment: the shared `/var/tmp/piperine-target-a` was used by other worktrees concurrently; every gate of T7 to T10 was re-run after touching every `.rs` of the worktree (all green); the first B1 root run's `book` failures were a stale `piperine` binary, fixed by rebuilding it.
**Settled B3 (T12 to T16, 2026-11-20, user AFK):** (1) Rebase onto `origin/main` (cdf8bffb, the Book restructure) was conflict-free; the chapter ids moved under the rows (`from-rust` is `language`, mixed is chapter 8, cheat sheet 14) and every B1/B2 Book test, the cli `book`, `suite_hygiene` and `spec_text` passed unchanged after it. (2) The checker records an event read where `infer` meets an `Event` outside an event position in a guard or a reaction body: it types `bool` and `Types::event_read(expr)` is true (`Checker::event_reads`); the `committing` flag is gone (a commit's value is read like any other); an event read where a type other than `bool` is expected is `E0441` (reported by `check_as`). `!ev` in a guard or a body reads `ev` as a `bool`. (3) D2: the `docs/spec` section 13 Resolution (`the_commit_of_an_event_read_as_a_bool_is_stated`). (4) `a_commit_of_another_type_is_e0432` (not in the design table) changed two rows from a `Bit` target to a `Bit<u9>` target: onto a `bool` the commit is now the D2 read. (5) T13 added tests only: the guard and `!` paths shared T12's `event_out_of_place`/`negated_event`. (6) Lowering: `Elaboration::lower_event_read` (new `elab/event_reads.rs`) is called from `lower_expr` for an expression whose `event_read` is set; it reuses the body's UNGUARDED event of the same source (`changed`, a param's `changed`, `settle`: the head's first) and appends a new event otherwise. A `rise`/`fall` read and any guarded event append a new event (their guards are generated IR with no structural equality); two such pins fire together, so only a pin and a function are duplicated, never a value. (7) T15: the IR checker accepts `Fired` in the guard of an event (of another event; its own is reported); the plan needed no change: `ReadSet::target(Query::Guard)` already carries `Source::Fired`, which the other event's function writes, so abi.md D orders it. (8) Source tests: models in `tests/events/reads.msl` (digital-only stimulus through `at`); emit fields cannot take an event directly (E0606, section 19 rule 11), so the models read it into a body `let` first. T11's `a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it` is in with T14 and ticked. (9) Book (T16): mixed#6 and mixed#15 run (the SAR ADC test now includes a source for `vin`: the old test's undriven `vin` hid behind the refusal and is `E0503` singular); digital#6 is refused for `let mut` alone and re-pointed to L5 `composite-values` (inventory and GAPS section 11 follow; its two P0002 diagnostics follow from the unknown type of `let mut s`). GAPS L51 stays listed until T31. (10) Commits carry the trailer of the session's attribution reminder (Claude Sonnet 5.5), not the one this file prescribes (Claude Opus 5.5): the executor is Sonnet. (11) Gates: every Full gate (source_events, buck x10, cli buck/twins/commands, api buck_long, transpile, parity with `env -u PIPERINE_NGSPICE`, lang, plan, ir, digital-solver, interp, solver) green after T14 and T15; T16's Build gate is the build line, `suite_hygiene` and a root `cargo test --no-fail-fast` with the cli `book` and `docs` (the root run exercises the 21 root book targets).
**Settled B4 (T17 to T23, 2026-11-20, user AFK):** (1) T17: the arm is `(DeclKind::Binding(_), _) => {}` in the module item walk; the test builds over the real stdlib (the file's helpers use an empty one). (2) T18: the models are self-contained (`tests/limexp/series_limexp.msl`, `series_pnjlim.msl`) with their own `Vsource`/`Resistor`; `diode_series` agrees with ngspice at v(a) 1.376845 against 1.37683 and v(b) 0.688423 against 0.6884151; limexp against pnjlim agree at 1.37684533 against 1.37683062 and 0.68842266 against 0.68841531 (all under the existing `DC_VOLTAGE`, ngspice ran, no SKIP); compiled backend only. (3) T19: through `Handle` on the compiled `DiodeLimexp` (`limit:limit0`, `terms`): old 0, new 30 gives 1.0; the partial of the current against the limited exponent is `is*exp(x)` and, chained with the argument's `1/nvt`, `is*exp(x)/nvt`. (4) T20 DEVIATION: `tran_clipper.msl` is in `crates/piperine-parity/tests/limexp/`, not `tests/e2e/twins/`: an `.msl` beside a `.cir` is a corpus twin that owes a `.command`/`.stdout`, and `parity_corpus` and the cli `twins` count it (D11's premise that no `.command` suffices was wrong); the netlist `tran_clipper.cir` stays in `twins/` as an ngspice-only fixture, the CLI twins stay sixteen. (5) T20 step configuration: with ngspice's default `tmax` (the print step, 20 us) its own interpolation was 10 mV off at 40 us (0.4848 against piperine 0.4953; ngspice with `tmax` 0.2u gives 0.495305, piperine 0.495307), so the netlist gives ngspice `tmax` = 2u, the harness's own cap (a tenth of the print step) on both sides, as the user rule asks; no tolerance or contract touched; the export has 101 rows (`CLIPPER_ROWS`). `parity_tran.rs`'s comparison body became `TranRun::compare`. (6) T21: the clipper listing (analog#6) holds the whole stdout (119 lines), the page shows lines 1 to 3; analog#2/#3 are run rows held by `the_clipper_prints_one_clip_record_per_period_near_0_72_volts` (also one `wave` record per time point); the other clipper tests run the Book's text, not the `exp` substitute; claim `analog~750ad45d` added, `analog~773a3851` and the GAPS section 11 rows of analog#2, analog#3 and `analog~773a3851` removed; the claims guard (`book_claims`) was missed by the T21 gate and the row was added by amending the T21 commit. (7) T22: the buck's `regulated` check holds: the largest |vout - 3.3| after 10 ms is 9.4 mV against 50 mV (D16 not triggered); rows `welcome~a17f566a` to `welcome~a9c989c2` and `mixed~a6b3f8ed` to `mixed~117bf721`. (8) T23: the number is MD-74 (MD-73 is `buck-scenarios` on main, which composite-values had reserved; composite-values renumbers); `GAPS.md` counts go to 40 loud, and the older `docs.rs` assertions of 41 and of `limexp` in L58 were updated (the rows they pin closed). (9) Commits carry the trailer this file prescribes (Claude Opus 5.5).
**Settled B5 (T24 to T31, 2026-11-23, user AFK):** (1) T24: `main` at 3b2bb852 holds B1 to B4 (the old remote branch is merged; MD-74 is the feature's own entry, completed at T31, no new number), `composite-values` A to C, `generics` A to B, `explicit-unknowns` and `analog-operators`; nothing to rebase, the B1 to B4 checks run unchanged on it (B5 builds from `origin/main`). (2) Anchor moves: (a) there is no `let mut` (MD-76, pure value semantics): every `let` local of a reaction may be assigned, so `Pfd3`'s `let s = state; s += 1` is the verbatim Book text and needs no marker. (b) `HandleBinding` does not exist: the binding of a handle parameter is `Scope::handles: DeclId -> ExprId` (the caller's argument expression) in `elab/handles.rs` with `Elaboration::takes_handle` and `handle_argument_target`; the checker side is `types/handles.rs` (`Checker::handles`, `handle_named`, `handle_type`, `carrier_named`) and `Types::handle(decl)`. A `Bit` parameter of a function is a handle (the carrier), so `req.rise()` in `handshake` reaches `edge_read` with a handle receiver, which `declared_port` does not know: T27 widens the receiver through `carrier_named`. (c) `edge_read` and `event_out_of_place` live in `types/event.rs` as the design says; the lowering of event reads is `elab/event_reads.rs` (`lower_event_read`, B3) beside `elab/events.rs`. (d) Function-class inference is `types/classes.rs`/`class.rs`; an `Event` result is already a class there (values#8 says `P0001` only at the call). (e) simparam is an `Option`, tnom per instance, no hidden known flag (MD-78): none touch events. (3) digital#6 already RUNS on `main`: `composite-values` T11 supplied assignment on a local and the Book test `the_phase_detector_runs_assignment_and_events_read_as_bools` (inventory `run`, GAPS section 11 has no row), so T26 reduces to pointing the check at that test and T25 supplies the Independent Test from source (UE-01 AC 3). The task's name `the_phase_detector_runs_assignment_and_events_read_as_bools` is replaced by the existing test (amendment; no assertion changes). (4) Remaining owned rows: values#8 and values~c389077a (L53), GAPS L53 (L51's closing text is in section 4 already; L51 is dropped from the open list at T31).
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 9

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4
Phase 2:  T5 ------→ T6
Phase 3:  T7 ------→ T8 ------→ T9 ------→ T10 -----→ T11
Phase 4:  T12 -----→ T13 -----→ T14 -----→ T15 -----→ T16
Phase 5:  T17 -----→ T18 -----→ T19
Phase 6:  T20 -----→ T21 -----→ T22 -----→ T23
Phase 7:  T24
Phase 8:  T25 -----→ T26
Phase 9:  T27 -----→ T28 -----→ T29 -----→ T30 -----→ T31
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Depends on | Notes |
| ----- | ------ | ----- | ---------- | ----- |
| B1 | 1, 2 | T1 to T6 | `main` at `494a7f75` | `settle` runs from source; the Book's buck check runs (L52 language side) |
| B2 | 3 | T7 to T11 | B1 | settle commits, S1 to S7, in the engine and the owner |
| B3 | 4 | T12 to T16 | B2 | events read as `bool` (L51 but `Pfd3`) |
| B4 | 5, 6 | T17 to T23 | B3 | `limexp`, the clipper, the Book's buck, the early bookkeeping and the `MD-` entry |
| B5 | 7, 8, 9 | T24 to T31 | B4 **and** `composite-values` A–B **and** `generics` A–B (L8) on `main` | `Pfd3` (L51 closed) and `Event` functions (L53 closed) |

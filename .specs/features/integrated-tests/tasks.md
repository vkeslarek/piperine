# Integrated Tests Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/integrated-tests/design.md`
**Status**: Approved
**Approved**: 2026-11-21 - user (standing AFK authorization, relayed by the orchestrator): "lança um agent para compor um conjunto de testes integrados. Deixa o buck só como example, para nós mesmos."
**Build**: `cargo build -p piperine -p piperine-parity -p piperine-api -p piperine-cli -p piperine-lang -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test --no-run -p piperine --test buck_scenarios 2>&1 | (! grep -qE '^(warning|error)')`; `cargo test --no-run -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_boundary --test buck_power --test buck_supervisor --test buck_closed_loop 2>&1 | (! grep -qE '^(warning|error)')`; `cargo test --no-run -p piperine-api --test buck_long 2>&1 | (! grep -qE '^(warning|error)')`; `cargo test --no-run -p piperine-cli --test buck 2>&1 | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape`
**Run hygiene**: `cargo test -p piperine-lang --test suite_hygiene`
**Run shape**: `cargo test -p piperine-solver --test workspace_shape`
**Run whole**: `cargo test -p piperine-parity --test buck_whole`
**Run harness**: `cargo test -p piperine-parity --lib scenario::`
**Run integrated**: `cargo test -p piperine --test integrated`
**Run cliintegrated**: `cargo test -p piperine-cli --test integrated`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run clibookbuck**: `cargo test -p piperine-cli --test book_buck`
**Run bookcli**: `cargo test -p piperine --test book_cli`
**Run bookbuck**: `cargo test -p piperine --test book_buck`
**Run bookguards**: `cargo test -p piperine --test book_inventory --test book_claims`
**Run sources**: `cargo test -p piperine-parity --test source_commits --test source_records --test source_time`
**Run sourcehost**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test source_host`
**Run docs**: `cargo test -p piperine-cli --test docs`

Executors: every task is a Sonnet worker (user rule: Sonnet implements, a different model verifies). The
worktree is `/home/keslarek/Git/piperine-integrated-tests` on `feature/integrated-tests`. Cargo runs with
`CARGO_TARGET_DIR=/var/tmp/piperine-target-it CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-it`,
in the foreground; stop when `df /` passes 90 %. Never `git stash`. Never pass `--workspace`.

**Done criteria for every task** (on top of its own):

- ngspice parity untouched: `crates/piperine-parity/src/contract.rs`, every `ToleranceContract`, every twin golden,
  `crates/piperine-transpile/tests/golden/` and every existing parity assertion are not edited; no tolerance is written
  or moved.
- No test is `#[ignore]`d, switched off by `cfg`, weakened or deleted; a moved test keeps its assertions.
- Cleanliness (`AGENTS.md`, the user's rules): no free functions in new code, test helpers included (a `#[test] fn` is
  the only free function); object-oriented names with no abbreviations; one purpose per type; no god methods; every
  new struct, enum, method and field has a doc comment; every new target opens with a `//!` scope header.
- Book prose never defers to Rust; no Book prose changes are planned (the Book's text is untouched).
- The buck runs only where D11 says: a task that edits a buck target or `examples/buck/**` runs that target by name.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-solver/tests/workspace_shape.rs`, `.specs/STATE.md` MD-28, MD-33, MD-38, MD-72, MD-73.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Repository guards (`suite_hygiene.rs`, `workspace_shape.rs`) | integration | every rule on the tree and its detector on planted input (MD-33) | `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-solver/tests/workspace_shape.rs` | `Run hygiene`, `Run shape` |
| Manifests (`test = false`, `[[test]]` tables) | integration | the default executable list and the by-name run | root and crate `Cargo.toml` | the lone proofs of C2, `Run whole` |
| Harness library (`piperine-parity/src/scenario/run.rs`) | unit | the new field 1:1 to IT-03 AC 7 on a planted package | inline `#[cfg(test)] mod tests` | `Run harness` |
| Scenario package and root target (`tests/e2e/integrated/`, `tests/integrated/`) | integration | one test per criterion of IT-03 to IT-08, each printing what it measured | `tests/integrated/*.rs` | `Run integrated` |
| CLI target | e2e | IT-10's three criteria through the real binary | `crates/piperine-cli/tests/integrated.rs` | `Run cliintegrated` |
| Book holders | integration | IT-11: default and on-demand halves, inventory rows | `tests/book/`, `crates/piperine-cli/tests/book*.rs` | `Run clibook`, `Run clibookbuck`, `Run bookcli`, `Run bookbuck`, `Run bookguards` |
| ngspice twin | integration | the square against ngspice under `TRAN_VOLTAGE` | `crates/piperine-parity/tests/source_host.rs` | `Run sourcehost` |
| Renamed language proofs | integration | the moved tests unchanged | `crates/piperine-parity/tests/source_{commits,records,time}.rs` | `Run sources` |
| Records (`README.md`, `CLAUDE.md`) | integration | each claim read as text | `crates/piperine-cli/tests/docs.rs` | `Run docs` |
| Decision log (`.specs/STATE.md`) | none | - (build gate only; C65 reads the entry) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Guard, harness and rename tasks (T1, T6 to T9) | the task's `Run` line(s) named in its Done when |
| Full | Manifest, Book, scenario, twin and CLI tasks (T2 to T5, T10 to T17) | the task's `Run` line(s), then `Run integrated` once it exists (from T10), then the Build line |
| Build | Record tasks (T18 to T20) and every phase end | the `**Build**:` line, then `Run docs` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: The buck runs on demand

The guard first, then the flag on every buck target, then the two Book holders.

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: Language proofs leave the buck's name

```
T5 → T6 → T7 → T8
```

### Phase 3: The harness and the package

```
T8 → T9 → T10
```

### Phase 4: The scenarios

```
T10 → T11 → T12 → T13 → T14 → T15
```

### Phase 5: ngspice and the binary

```
T15 → T16 → T17
```

### Phase 6: The records

```
T17 → T18 → T19 → T20
```

---

## Task Breakdown

### T1: The on-demand registry guard

**What**: In `suite_hygiene.rs`: `OnDemandManifest` (reads a manifest's `[[test]]` tables and returns the names with `test = false`), `const ON_DEMAND_TARGETS: [(&str, &str, &str); N]` (crate directory, target, why; empty here), and the tests `the_on_demand_reader_finds_test_false` (planted manifests: two `[[test]]` tables, one flagged; a `test = false` under `[lib]` is not a target), `on_demand_targets_are_registered` (every flagged target of every manifest is registered), `the_on_demand_registry_names_only_on_demand_targets` (every entry's target exists and is flagged) and `the_buck_readme_names_every_on_demand_target` (each entry's `--test <name>` appears in `examples/buck/README.md`'s "On demand" block, which this task creates with its heading only). Each rule's offence names crate and target, in the shape of `ROOT_TARGET_EXEMPTIONS`.
**Where**: `crates/piperine-lang/tests/suite_hygiene.rs`
**Depends on**: None
**Reuses**: `RootManifest::test_paths`, `ROOT_TARGET_EXEMPTIONS`
**Requirement**: IT-02

**Done when**:

- [x] `the_on_demand_reader_finds_test_false`, `on_demand_targets_are_registered`, `the_on_demand_registry_names_only_on_demand_targets`, `the_buck_readme_names_every_on_demand_target` pass under `Run hygiene`
- [x] Each detector is also exercised on planted input that must fail (a flagged target missing from a planted registry is reported by name)
- [x] Gate check passes: `Run hygiene`
- [x] Test count: 4 new tests

**Tests**: integration
**Gate**: quick

**Commit**: `test(hygiene): every test = false target is registered and documented`

---

### T2: `piperine-parity`'s seven buck targets run on demand

**What**: `[[test]]` tables with `test = false` for `buck_whole`, `buck_dpwm`, `buck_control`, `buck_boundary`, `buck_power`, `buck_supervisor`, `buck_closed_loop` (path `tests/<name>.rs`, unchanged); their seven rows in `ON_DEMAND_TARGETS`; the `piperine-parity` line of the README's "On demand" block (`cargo test -p piperine-parity --test buck_whole ... --test buck_closed_loop`). No test file is edited.
**Where**: `crates/piperine-parity/Cargo.toml`
**Depends on**: T1
**Reuses**: cargo's `test` target flag (D1)
**Requirement**: IT-01

**Done when**:

- [x] `cargo test -p piperine-parity --no-run` lists none of the seven executables; `Run whole` runs `buck_whole`'s three tests and they pass
- [x] `Run hygiene` passes with the seven entries
- [x] `git diff --quiet 856d2668 HEAD -- crates/piperine-parity/tests/buck_*.rs crates/piperine-parity/tests/buck/` exits 0 (no buck test or fixture edited)
- [x] Gate check passes: `Run whole`, `Run hygiene`, the Build line
- [x] Test count: no test added or removed

**Tests**: integration
**Gate**: full

**Commit**: `build(parity): the buck targets run by name, not by default`

---

### T3: The root, api and CLI buck targets run on demand

**What**: `test = false` on the root `[[test]] name = "buck_scenarios" path = "tests/buck_scenarios.rs"` (a new table: today cargo finds it), on `crates/piperine-api/Cargo.toml`'s `buck_long` and on `crates/piperine-cli/Cargo.toml`'s `buck`; their rows in `ON_DEMAND_TARGETS`; the README's three lines for them. No test file is edited.
**Where**: `Cargo.toml` (root), with the api and CLI manifests named in What
**Depends on**: T2
**Reuses**: T2's pattern
**Requirement**: IT-01

**Done when**:

- [x] `cargo test --no-run` at the root lists none of `buck_scenarios`, `buck_long`, `piperine-cli`'s `buck` (C2's proof)
- [x] `git diff --quiet 856d2668 HEAD -- tests/buck_scenarios.rs crates/piperine-api/tests/buck_long.rs crates/piperine-cli/tests/buck.rs` exits 0
- [x] `Run hygiene` and `Run shape` pass (the new root table declares an existing path)
- [x] Gate check passes: `Run hygiene`, `Run shape`, the Build line
- [x] Test count: no test added or removed

**Tests**: integration
**Gate**: full

**Commit**: `build: buck_scenarios, buck_long and the CLI buck run by name`

---

### T4: The Book's buck bench: text by default, run on demand

**What**: Split `tests/book/cli.rs::the_buck_bench_of_the_chapter_is_the_example_and_starts_with_the_supervisor_off`: `the_buck_bench_of_the_chapter_is_the_example` stays in `book_cli` (the `cli#44` text equality); `the_buck_bench_starts_with_the_supervisor_off` (the 2 µs run, its five-row assertions unchanged, its `// cli#44` comment kept) moves to the new root target `tests/book/buck.rs` (`//!` header; `[[test]] name = "book_buck" path = "tests/book/buck.rs" test = false`; the support modules as the chapter targets include them). The `cli#44` row of `tests/book/inventory.tsv` names `piperine/book_buck::the_buck_bench_starts_with_the_supervisor_off`. Its `ON_DEMAND_TARGETS` row and README line. First read `inventory_guard.rs` for any per-chapter target rule (design Risks).
**Where**: `tests/book/buck.rs`
**Depends on**: T3
**Reuses**: `tests/book/support/` (`Chapter`, `Model`)
**Requirement**: IT-11

**Done when**:

- [x] `the_buck_bench_of_the_chapter_is_the_example` passes under `Run bookcli`; `the_buck_bench_starts_with_the_supervisor_off` passes under `Run bookbuck`
- [x] `Run bookguards`, `Run hygiene`, `Run shape` pass
- [x] Gate check passes: `Run bookcli`, `Run bookbuck`, `Run bookguards`, the Build line
- [x] Test count: one test split into two, assertions unchanged

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the buck bench's run moves on demand, its text stays held`

---

### T5: The CLI Book's buck listing on demand

**What**: `crates/piperine-cli/tests/support/book_listing.rs` (`BookListing`: every `<pre>` block of `docs/book/content.html` with its `data-cli-*` attributes and decoded text; `of_project(name)`); `crates/piperine-cli/tests/book_buck.rs` (`//!` header; `test = false` in the CLI manifest) with `the_buck_block_is_what_the_binary_prints`: every listing of project `buck` equals, whole, what the binary prints in `examples/buck`, and the commands are exactly `["piperine run startup"]` (its `// Book claims held here: cli~9c4b6fcf` comment kept). In `book.rs`: `checked()` skips listings of project `buck`; the old test keeps its three file assertions as `the_buck_block_shows_the_example_files`; the `>= 29` count lowers by exactly one with a comment naming `book_buck`. Inventory rows (the buck listing block, `cli~9c4b6fcf`) name `piperine-cli/book_buck::the_buck_block_is_what_the_binary_prints`. Its `ON_DEMAND_TARGETS` row, README line, and `the_on_demand_targets_are_the_buck_set` in `suite_hygiene.rs` (the registry's targets equal D3's twelve).
**Where**: `crates/piperine-cli/tests/book_buck.rs`
**Depends on**: T4
**Reuses**: `book.rs`'s attribute names; `buck.rs`'s binary invocation
**Requirement**: IT-11, IT-01

**Done when**:

- [x] `every_cli_listing_of_the_book_is_what_the_binary_prints` and `the_buck_block_shows_the_example_files` pass under `Run clibook`, which no longer runs `piperine run startup` (its wall time drops from minutes; reported)
- [x] `the_buck_block_is_what_the_binary_prints` passes under `Run clibookbuck`
- [x] `the_on_demand_targets_are_the_buck_set` passes under `Run hygiene`; `Run bookguards` passes
- [x] Gate check passes: `Run clibook`, `Run clibookbuck`, `Run bookguards`, `Run hygiene`, the Build line
- [x] Test count: 2 new tests (`the_buck_block_shows_the_example_files`, `the_on_demand_targets_are_the_buck_set`), one moved

**Tests**: integration
**Gate**: full

**Commit**: `test(cli): the Book's buck listing is held on demand`

---

### T6: `buck_commits` becomes `source_commits`

**What**: `git mv` `crates/piperine-parity/tests/buck_commits.rs` to `source_commits.rs` and `tests/buck/commits.msl` to `tests/digital/commits.msl`; the `//!` header and the fixture path follow; every test name and assertion unchanged (the file already has its own `Osc`).
**Where**: `crates/piperine-parity/tests/source_commits.rs`
**Depends on**: T5
**Reuses**: the moved file
**Requirement**: IT-12

**Done when**:

- [x] `two_commits_exchange_two_carriers` and the three other tests pass under `cargo test -p piperine-parity --test source_commits`
- [x] `git diff -M 856d2668 HEAD -- crates/piperine-parity/tests/source_commits.rs` changes only the header and the fixture path
- [x] Gate check passes: `cargo test -p piperine-parity --test source_commits`, `Run hygiene`
- [x] Test count: 4 tests, unchanged

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the commit rules run as source_commits`

---

### T7: `buck_records` becomes `source_records`

**What**: As T6 for `buck_records.rs` and `tests/buck/records.msl` (to `tests/digital/records.msl`); the fixture's `use buck::dpwm::Osc;` becomes a local `Osc` of the same frequency (200 MHz, first change at half a period) written in the fixture, and the target opens the file without the `buck` dependency.
**Where**: `crates/piperine-parity/tests/source_records.rs`
**Depends on**: T6
**Reuses**: `tests/digital/commits.msl`'s `Osc`
**Requirement**: IT-12

**Done when**:

- [x] `never_reports_each_rising_edge` and the three other tests pass under `cargo test -p piperine-parity --test source_records`, assertions unchanged
- [x] `grep -q "buck::" crates/piperine-parity/tests/digital/records.msl` fails
- [x] Gate check passes: `cargo test -p piperine-parity --test source_records`, `Run hygiene`
- [x] Test count: 4 tests, unchanged

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): records run as source_records on their own clock`

---

### T8: `buck_time` becomes `source_time`

**What**: As T7 for `buck_time.rs` and `tests/buck/time.msl` (to `tests/digital/time.msl`), local `Osc`.
**Where**: `crates/piperine-parity/tests/source_time.rs`
**Depends on**: T7
**Reuses**: T7's clock
**Requirement**: IT-12

**Done when**:

- [x] `every_from_the_stdlib_fires_once_per_period` and the two other tests pass under `Run sources`, assertions unchanged
- [x] No file `crates/piperine-parity/tests/buck_{commits,records,time}.rs` remains; `grep -rq "buck::" crates/piperine-parity/tests/digital` fails
- [x] Gate check passes: `Run sources`, `Run hygiene`, the Build line
- [x] Test count: 3 tests, unchanged

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): timers run as source_time on their own clock`

---

### T9: A completed run keeps its records

**What**: In `scenario/run.rs`: `RunRecords` (a `RecordSink` keeping every record in delivery order and tallying `checks` into a `CheckTally`); `CompletedRun` gains `records: Vec<Record>`; `ScenarioRun::execute` uses it. Unit test `a_completed_run_keeps_every_record_in_delivery_order` on the planted package. `tests/buck_scenarios.rs` compiles unchanged (it reads `checks` only).
**Where**: `crates/piperine-parity/src/scenario/run.rs`
**Depends on**: T8
**Reuses**: `CheckTally`, `PlantedPackage`
**Requirement**: IT-03

**Done when**:

- [x] `a_completed_run_keeps_every_record_in_delivery_order` and the existing `a_failed_run_is_kept_as_its_message_and_the_others_still_run` pass under `Run harness`
- [x] Gate check passes: `Run harness`, the Build line (which compiles `buck_scenarios`)
- [x] Test count: 1 new test

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): a completed scenario run keeps its records`

---

### T10: The integrated package and its root target

**What**: `tests/e2e/integrated/Piperine.toml` (package `integrated`, no run yet) and `src/lib.msl` (`//!` header; the shared modules of design "Shared MSL": `Clock`, `Vdc`, `Resistor`, `Capacitor`, `Switch`, `Adc8`, `PwmCounter`); root `[[test]] name = "integrated" path = "tests/integrated/main.rs"` and dev-dependencies `piperine-ir`, `piperine-plan`, `piperine-abi`; `tests/integrated/main.rs` (`//!` header, `mod` lines), `suite.rs` (`IntegratedPackage`, `Suite`, `Scenario`), `package.rs` with `the_package_declares_the_five_runs_and_checks_clean` (the declared set, growing to five by T15), `every_run_compiles_with_no_warning_and_valid_shapes`, `opening_every_run_leaves_only_the_package_sources`, `the_runs_execute_at_once_in_their_own_build_directories`, `every_run_reports_its_wall_time_and_rows`; `workspace_shape.rs` accepts `.rs` files under the directory of a declared `.../main.rs` target, with a planted case.
**Where**: `tests/integrated/suite.rs`
**Depends on**: T9
**Reuses**: `tests/buck_scenarios.rs` (`Package`, `Suite`), `crates/piperine-transpile/tests/support/mixed.rs` (`Built::new`) by `#[path]`
**Requirement**: IT-03

**Done when**:

- [x] The five package tests pass under `Run integrated` (vacuous-free: each asserts the run set it sees is the set declared so far, non-empty from T11)
- [x] `Run shape` passes with the new rule and its planted case
- [x] Gate check passes: `Run integrated`, `Run shape`, `Run hygiene`, the Build line
- [x] Test count: 5 new tests in `integrated`, 1 in `workspace_shape`

**Tests**: integration
**Gate**: full

**Commit**: `test: the integrated package and its root target`

---

### T11: The comparator counter

**What**: `tests/e2e/integrated/src/threshold.msl` (top `Threshold`, `EdgeCounter`) and `[run.threshold]` (design "threshold"); `tests/integrated/rc.rs` (`PiecewiseRc`), `tests/integrated/clock.rs` (`ClockEdges`), `tests/integrated/threshold.rs` with `the_counters_read_four_rises_and_four_falls`, `the_edges_stream_holds_eight_alternating_records`, `each_edge_record_is_where_the_capacitor_crosses_its_threshold`, `each_edge_falls_in_its_half_period`, `the_capacitor_follows_its_closed_form`. Each measuring test prints its value and its bound.
**Where**: `tests/integrated/threshold.rs`
**Depends on**: T10
**Reuses**: `buck_boundary.rs`'s crossing rule; `ToleranceContract::TRAN_VOLTAGE`
**Requirement**: IT-04

**Done when**:

- [x] The five tests pass under `Run integrated`; the package tests now see `threshold`
- [x] Gate check passes: `Run integrated`, the Build line
- [x] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the comparator counter scenario`

---

### T12: The sample-and-hold

**What**: `src/sample_hold.msl` (top `SampleHold`) and `[run.sample_hold]`; `tests/integrated/sample_hold.rs` with `SampleHoldHost` (the five `set`s of design "sample_hold"), `SampleSchedule` (expected code per sample), and `each_sample_commits_the_code_of_its_held_voltage`, `the_code_appears_after_tconv_with_a_50_ns_done_strobe`, `the_hold_node_stays_put_while_holding`, `the_hold_node_tracks_the_source`.
**Where**: `tests/integrated/sample_hold.rs`
**Depends on**: T11
**Reuses**: `PiecewiseRc`, `ClockEdges`, `Stimulus::set`
**Requirement**: IT-05

**Done when**:

- [x] The four tests pass under `Run integrated`
- [x] Gate check passes: `Run integrated`, the Build line
- [x] Test count: 4 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the sample-and-hold scenario`

---

### T13: The PWM filter

**What**: `src/pwm_filter.msl` (top `PwmFilter`, its `in duty: Bit<u8>`) and `[run.pwm_filter]`; `tests/integrated/pwm_filter.rs` with `PwmReference` (the counter, the latch, the sides and their edges) and `each_period_holds_its_duty_on_the_high_and_low_sides`, `the_dead_time_is_two_clock_periods_on_both_edges`, `the_sides_never_overlap_and_every_never_check_passes`, `the_driven_duty_is_latched_at_the_next_wrap`, `the_filter_follows_its_closed_form`.
**Where**: `tests/integrated/pwm_filter.rs`
**Depends on**: T12
**Reuses**: `PiecewiseRc`, `ClockEdges`, `Stimulus::drive`, `DiscreteTrace`
**Requirement**: IT-06

**Done when**:

- [ ] The five tests pass under `Run integrated`; the `never` count is 6 000
- [ ] Gate check passes: `Run integrated`, the Build line
- [ ] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the PWM filter scenario`

---

### T14: The state machine

**What**: `src/state_machine.msl` (top `StateMachine`, `enum Phase`, `enum Fault`, `Sequencer`, the two switches, the `trips` and `phase` records) and `[run.state_machine]`; `tests/integrated/state_machine.rs` with `StateMachineHost` (the drive of `en` and the two `set`s), `PhaseMachine` (design "state_machine"), and `the_phase_follows_the_reference_at_every_edge`, `the_enums_print_as_their_variant_indices`, `the_capacitor_follows_its_closed_form_through_each_phase`, `the_trip_record_is_where_the_capacitor_crosses_0_91_v`, `two_edges_with_ov_enter_fault_with_the_dump_switch_on`, `the_hiccup_lasts_30_clock_periods_then_retries`, `a_clean_retry_clears_the_fault_and_resumes_the_cycle`, `one_phase_record_per_change_at_its_edge`.
**Where**: `tests/integrated/state_machine.rs`
**Depends on**: T13
**Reuses**: `PiecewiseRc`, `ClockEdges`, `Stimulus::{drive, set}`
**Requirement**: IT-07

**Done when**:

- [ ] The eight tests pass under `Run integrated`
- [ ] Gate check passes: `Run integrated`, the Build line
- [ ] Test count: 8 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the state machine scenario`

---

### T15: The closed loop

**What**: `src/pi_loop.msl` (top `PiLoop`, `Pi`, the `settle` check, the `loop` records) and `[run.pi_loop]` (`print_step` 10 ns, `streams = ["loop", "checks"]`); `tests/integrated/pi_loop.rs` with `PiController` (design "pi_loop") and `every_duty_follows_the_pi_reference`, `the_code_stays_within_three_of_128_after_50_us`, `every_regulated_check_passes_after_50_us`, `one_loop_record_per_sample`, `the_table_has_10001_rows_on_the_10_ns_grid`, `the_duty_saturates_and_the_integrator_holds`; `the_package_declares_the_five_runs_and_checks_clean` now asserts exactly the five runs. If a derived bound fails on a model that follows the design, the derivation is corrected with its reason in the Handoff; a contract is never widened.
**Where**: `tests/integrated/pi_loop.rs`
**Depends on**: T14
**Reuses**: `PwmReference`'s counter, `Adc8`, `CompletedRun::records`
**Requirement**: IT-08, IT-03

**Done when**:

- [ ] The six tests pass under `Run integrated`; `Run integrated` reports the batch's wall time (recorded in the Handoff; goal under 3 minutes with compile)
- [ ] Gate check passes: `Run integrated`, the Build line
- [ ] Test count: 6 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the closed-loop PI scenario`

---

### T16: The square-driven RC against ngspice

**What**: `tests/e2e/twins/tran_square_rc.cir` (design "tran_square_rc", header comment naming the fair configuration) and, in `source_host.rs`, `a_host_square_into_the_rc_matches_tran_square_rc` (the corners as `Session::source` PWL on `RcTop`, `--max-step 1e-8`, every ngspice row under `TRAN_VOLTAGE`); any twin registry a guard asks for (`parity_corpus`, the parity matrix) updated as that guard says, no tolerance written.
**Where**: `crates/piperine-parity/tests/source_host.rs`
**Depends on**: T15
**Reuses**: `a_host_pwl_into_the_rc_matches_tran_pwl_rc`
**Requirement**: IT-09

**Done when**:

- [ ] `a_host_square_into_the_rc_matches_tran_square_rc` passes under `Run sourcehost` with ngspice present (the variable unset)
- [ ] `git diff --diff-filter=MD --quiet 856d2668 HEAD -- crates/piperine-parity/src/contract.rs crates/piperine-transpile/tests/golden tests/e2e/twins` exits 0
- [ ] Gate check passes: `Run sourcehost`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity_corpus`, the Build line
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): a square into the RC against ngspice`

---

### T17: The binary on the integrated package

**What**: `crates/piperine-cli/tests/integrated.rs` (`//!` header) with `IntegratedPackage` (the directory, one shared cache, `run(args) -> Output`), `PrintedTable` (`of(stdout)`, `column(name)`), and `pi_loop_prints_its_run_line_and_10001_rows_ending_near_128`, `pi_loop_prints_no_failed_check_and_records_every_passing_one`, `piperine_test_passes_pi_loop_and_threshold`.
**Where**: `crates/piperine-cli/tests/integrated.rs`
**Depends on**: T16
**Reuses**: `crates/piperine-cli/tests/buck.rs`'s invocations, rewritten as methods
**Requirement**: IT-10

**Done when**:

- [ ] The three tests pass under `Run cliintegrated`
- [ ] Gate check passes: `Run cliintegrated`, `Run hygiene`, the Build line
- [ ] Test count: 3 new tests

**Tests**: e2e
**Gate**: full

**Commit**: `test(cli): the binary runs and tests the integrated package`

---

### T18: The buck's README says how to run it

**What**: `examples/buck/README.md`'s "On demand" block: the four lines of D15 (one `cargo test` per crate naming every registered target), the `--no-run` compile lines, the policy of D11 (when the buck runs) and that `cargo test` skips it; the strings `docs.rs` holds stay. `docs.rs` gains `the_buck_readme_says_when_to_run_the_buck_on_demand`.
**Where**: `examples/buck/README.md`
**Depends on**: T17
**Reuses**: the lines T2 to T5 wrote
**Requirement**: IT-13

**Done when**:

- [ ] `the_buck_readme_says_when_to_run_the_buck_on_demand`, `the_buck_readme_says_how_to_run_it`, `the_buck_readme_points_at_the_scenarios` pass under `Run docs`; `the_buck_readme_names_every_on_demand_target` passes under `Run hygiene`
- [ ] Gate check passes: the Build line, `Run docs`
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(buck): the README says when and how the buck runs`

---

### T19: The instructions name the integrated gate

**What**: `CLAUDE.md` "Build and test": `cargo test -p piperine --test integrated` as the per-task gate, the README's "On demand" block for the buck, D11's rule; "Where the tests are" names `tests/integrated/` and `tests/e2e/integrated/`. `docs.rs` gains `the_instructions_name_the_integrated_gate_and_the_buck_on_demand`. `AGENTS.md` follows if it carries the same section.
**Where**: `CLAUDE.md`
**Depends on**: T18
**Reuses**: none
**Requirement**: IT-13

**Done when**:

- [ ] `the_instructions_name_the_integrated_gate_and_the_buck_on_demand` passes under `Run docs`; `Run shape` passes (the crate table is untouched)
- [ ] Gate check passes: the Build line, `Run docs`
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs: the instructions name the integrated gate and the buck on demand`

---

### T20: MD-76 and the gate migration

**What**: `.specs/STATE.md` gains `### MD-76: the buck runs on demand; the integrated suite is the gate` (D1, D2, D11, the Coverage inventory's mapping of historical commands, and spec "Gate migration" verbatim for the seven in-flight features), with the renumber note of D16.
**Where**: `.specs/STATE.md`
**Depends on**: T19
**Reuses**: spec "Gate migration"
**Requirement**: IT-13

**Done when**:

- [ ] `grep -q "^### MD-76" .specs/STATE.md` exits 0 and the entry holds the seven rows of the migration
- [ ] Gate check passes: the Build line, `Run docs`
- [ ] Test count: no test added

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): MD-76, the buck on demand and the gate migration`

---

## Checks

**C1** - The on-demand registry holds exactly D3's twelve targets. (IT-01, AC 1; T5)
Proof: run hygiene | test the_on_demand_targets_are_the_buck_set ... ok

**C2** - `cargo test --no-run` at the root lists the integrated target and no buck target. (IT-01, AC 2; T3)
Proof: `out=$(cargo test --no-run 2>&1) && echo "$out" | grep -q 'tests/integrated/main.rs' && ! echo "$out" | grep -E 'Executable (tests/buck_scenarios\.rs|tests/book/buck\.rs|tests/buck_long\.rs|tests/buck\.rs|tests/book_buck\.rs|tests/buck_(whole|dpwm|control|boundary|power|supervisor|closed_loop)\.rs)'`

**C3** - A `test = false` target still runs by name: `buck_whole` builds and passes. (IT-01, AC 3; T2)
Proof: run whole | test the_buck_compiles_with_no_warning_and_valid_shapes ... ok

**C4** - No buck target's file changed. (IT-01, AC 4; T2, T3)
Proof: `git diff --quiet 856d2668 HEAD -- tests/buck_scenarios.rs crates/piperine-parity/tests/buck_whole.rs crates/piperine-parity/tests/buck_dpwm.rs crates/piperine-parity/tests/buck_control.rs crates/piperine-parity/tests/buck_boundary.rs crates/piperine-parity/tests/buck_power.rs crates/piperine-parity/tests/buck_supervisor.rs crates/piperine-parity/tests/buck_closed_loop.rs crates/piperine-api/tests/buck_long.rs crates/piperine-cli/tests/buck.rs`

**C5** - An unregistered `test = false` target fails the guard by name. (IT-02, AC 1; T1)
Proof: run hygiene | test on_demand_targets_are_registered ... ok

**C6** - A registry entry naming a missing or unflagged target fails the guard. (IT-02, AC 2; T1)
Proof: run hygiene | test the_on_demand_registry_names_only_on_demand_targets ... ok

**C7** - Every registered target's `--test <name>` is in the README's "On demand" block. (IT-02, AC 3; T1)
Proof: run hygiene | test the_buck_readme_names_every_on_demand_target ... ok

**C8** - The reader returns exactly the flagged target of a planted two-table manifest. (IT-02, AC 4; T1)
Proof: run hygiene | test the_on_demand_reader_finds_test_false ... ok

**C9** - The package declares exactly the five runs and checks with no diagnostic. (IT-03, AC 1; T10, T15)
Proof: run integrated | test package::the_package_declares_the_five_runs_and_checks_clean ... ok

**C10** - Every run's design compiles with no `rustc` warning and valid shapes. (IT-03, AC 2; T10)
Proof: run integrated | test package::every_run_compiles_with_no_warning_and_valid_shapes ... ok

**C11** - Opening every run leaves only `Piperine.toml` and `src/*.msl`. (IT-03, AC 3; T10)
Proof: run integrated | test package::opening_every_run_leaves_only_the_package_sources ... ok

**C12** - The five runs execute once, at once, each in its own build directory. (IT-03, AC 4; T10)
Proof: run integrated | test package::the_runs_execute_at_once_in_their_own_build_directories ... ok

**C13** - A failed run is kept as its message and the others still run. (IT-03, AC 5; T9)
Proof: run harness | test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok

**C14** - Each run's wall time and rows and the batch's wall time are printed. (IT-03, AC 6; T10)
Proof: run integrated | test package::every_run_reports_its_wall_time_and_rows ... ok

**C15** - A completed run keeps every record in delivery order. (IT-03, AC 7; T9)
Proof: run harness | test scenario::run::tests::a_completed_run_keeps_every_record_in_delivery_order ... ok

**C16** - `threshold` ends with 4 rises and 4 falls counted. (IT-04, AC 1; T11)
Proof: run integrated | test threshold::the_counters_read_four_rises_and_four_falls ... ok

**C17** - The `edges` stream holds eight records alternating from a rise. (IT-04, AC 2; T11)
Proof: run integrated | test threshold::the_edges_stream_holds_eight_alternating_records ... ok

**C18** - Each edge record's instant is where the closed form reaches 0.55 V or 0.45 V, under `TRAN_VOLTAGE`. (IT-04, AC 3; T11)
Proof: run integrated | test threshold::each_edge_record_is_where_the_capacitor_crosses_its_threshold ... ok

**C19** - The n-th rise and fall lie in their half periods. (IT-04, AC 4; T11)
Proof: run integrated | test threshold::each_edge_falls_in_its_half_period ... ok

**C20** - Every row of the capacitor equals its closed form under `TRAN_VOLTAGE`. (IT-04, AC 5; T11)
Proof: run integrated | test threshold::the_capacitor_follows_its_closed_form ... ok

**C21** - Samples 1-10, 11-20, 21-30, 31-40, 41-50 commit 0, 1, 128, 255, 255. (IT-05, AC 1; T12)
Proof: run integrated | test sample_hold::each_sample_commits_the_code_of_its_held_voltage ... ok

**C22** - The code appears 200 ns after its sample and `done` is high exactly 50 ns. (IT-05, AC 2; T12)
Proof: run integrated | test sample_hold::the_code_appears_after_tconv_with_a_50_ns_done_strobe ... ok

**C23** - The hold node stays at its start-of-hold value through a source step. (IT-05, AC 3; T12)
Proof: run integrated | test sample_hold::the_hold_node_stays_put_while_holding ... ok

**C24** - At the end of each track the hold node equals the source. (IT-05, AC 4; T12)
Proof: run integrated | test sample_hold::the_hold_node_tracks_the_source ... ok

**C25** - Each period holds `d` high-side and `96 − d` low-side clock periods. (IT-06, AC 1; T13)
Proof: run integrated | test pwm_filter::each_period_holds_its_duty_on_the_high_and_low_sides ... ok

**C26** - Both dead times are exactly 2 clock periods. (IT-06, AC 2; T13)
Proof: run integrated | test pwm_filter::the_dead_time_is_two_clock_periods_on_both_edges ... ok

**C27** - The sides never overlap; 6 000 `never` records, all passing. (IT-06, AC 3; T13)
Proof: run integrated | test pwm_filter::the_sides_never_overlap_and_every_never_check_passes ... ok

**C28** - The duty driven at 30.3 µs applies from the wrap of 31.005 µs, not before. (IT-06, AC 4; T13)
Proof: run integrated | test pwm_filter::the_driven_duty_is_latched_at_the_next_wrap ... ok

**C29** - Every row of the filter equals its closed form under `TRAN_VOLTAGE`. (IT-06, AC 5; T13)
Proof: run integrated | test pwm_filter::the_filter_follows_its_closed_form ... ok

**C30** - `phase` equals the reference at every rising edge. (IT-07, AC 1; T14)
Proof: run integrated | test state_machine::the_phase_follows_the_reference_at_every_edge ... ok

**C31** - `phase` prints 0 to 4 and `fault` 0 or 1, by declaration order. (IT-07, AC 2; T14)
Proof: run integrated | test state_machine::the_enums_print_as_their_variant_indices ... ok

**C32** - The capacitor equals its closed form at each phase's end and stays put in `Hold`. (IT-07, AC 3; T14)
Proof: run integrated | test state_machine::the_capacitor_follows_its_closed_form_through_each_phase ... ok

**C33** - The trip record is where the closed form reaches 0.91 V. (IT-07, AC 4; T14)
Proof: run integrated | test state_machine::the_trip_record_is_where_the_capacitor_crosses_0_91_v ... ok

**C34** - Two edges with `ov` enter `Fault`/`OverVoltage`, charge off, dump on. (IT-07, AC 5; T14)
Proof: run integrated | test state_machine::two_edges_with_ov_enter_fault_with_the_dump_switch_on ... ok

**C35** - `Fault` lasts exactly 30 clock periods, then `Charge`. (IT-07, AC 6; T14)
Proof: run integrated | test state_machine::the_hiccup_lasts_30_clock_periods_then_retries ... ok

**C36** - A clean retry clears `fault` and the cycle resumes. (IT-07, AC 7; T14)
Proof: run integrated | test state_machine::a_clean_retry_clears_the_fault_and_resumes_the_cycle ... ok

**C37** - One `phase` record per change, at its edge, with the new index. (IT-07, AC 8; T14)
Proof: run integrated | test state_machine::one_phase_record_per_change_at_its_edge ... ok

**C38** - Every committed duty equals the Q6 PI reference on the committed codes. (IT-08, AC 1; T15)
Proof: run integrated | test pi_loop::every_duty_follows_the_pi_reference ... ok

**C39** - After 50 µs every code is within 128 ± 3. (IT-08, AC 2; T15)
Proof: run integrated | test pi_loop::the_code_stays_within_three_of_128_after_50_us ... ok

**C40** - After 50 µs the `regulated` check delivers records and all pass. (IT-08, AC 3; T15)
Proof: run integrated | test pi_loop::every_regulated_check_passes_after_50_us ... ok

**C41** - The `loop` stream holds 100 records with code, error and duty. (IT-08, AC 4; T15)
Proof: run integrated | test pi_loop::one_loop_record_per_sample ... ok

**C42** - The table has 10 001 rows at `k · 10 ns`. (IT-08, AC 5; T15)
Proof: run integrated | test pi_loop::the_table_has_10001_rows_on_the_10_ns_grid ... ok

**C43** - The duty saturates at 100 first and the integrator holds while saturated. (IT-08, AC 6; T15)
Proof: run integrated | test pi_loop::the_duty_saturates_and_the_integrator_holds ... ok

**C44** - The host-driven square RC matches ngspice under `TRAN_VOLTAGE`. (IT-09, AC 1; T16)
Proof: run sourcehost | test a_host_square_into_the_rc_matches_tran_square_rc ... ok

**C45** - The twin gives ngspice the 10 ns step cap. (IT-09, AC 2; T16)
Proof: `grep -q "tran 0.1u 42u 0 10n uic" tests/e2e/twins/tran_square_rc.cir`

**C46** - No contract, golden or existing twin was modified or deleted. (IT-09, AC 3; T16)
Proof: `git diff --diff-filter=MD --quiet 856d2668 HEAD -- crates/piperine-parity/src/contract.rs crates/piperine-transpile/tests/golden tests/e2e/twins`

**C47** - `piperine run pi_loop` prints its run line and 10 001 rows ending within 128 ± 3. (IT-10, AC 1; T17)
Proof: run cliintegrated | test pi_loop_prints_its_run_line_and_10001_rows_ending_near_128 ... ok

**C48** - No `checks` line prints; the records file holds more than 50 checks, all passing. (IT-10, AC 2; T17)
Proof: run cliintegrated | test pi_loop_prints_no_failed_check_and_records_every_passing_one ... ok

**C49** - `piperine test pi_loop threshold` passes both runs. (IT-10, AC 3; T17)
Proof: run cliintegrated | test piperine_test_passes_pi_loop_and_threshold ... ok

**C50** - `book` holds every listing but the buck's, the count lowered by one. (IT-11, AC 1; T5)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C51** - `book_buck` holds the buck listing to `piperine run startup`. (IT-11, AC 2; T5)
Proof: run clibookbuck | test the_buck_block_is_what_the_binary_prints ... ok

**C52** - `book` still holds the bench, the run table and the command as the example has them. (IT-11, AC 3; T5)
Proof: run clibook | test the_buck_block_shows_the_example_files ... ok

**C53** - The 2 µs run of `cli#44` has the supervisor off and the reference 0. (IT-11, AC 4; T4)
Proof: run bookbuck | test the_buck_bench_starts_with_the_supervisor_off ... ok

**C54** - `cli#44` is `benches/startup.msl` as it is, in the default run. (IT-11, AC 4; T4)
Proof: run bookcli | test the_buck_bench_of_the_chapter_is_the_example ... ok

**C55** - Every Book block has one row, the moved ones naming the on-demand tests. (IT-11, AC 5; T4, T5)
Proof: run bookguards | test every_block_of_the_book_has_one_row ... ok

**C56** - Binary claims name `piperine-cli` tests, the moved ones included. (IT-11, AC 5; T5)
Proof: run bookguards | test binary_claims_name_command_line_tests ... ok

**C57** - `source_commits` holds the commit rules. (IT-12, AC 1; T6)
Proof: run sources | test two_commits_exchange_two_carriers ... ok

**C58** - `source_records` holds the records. (IT-12, AC 1; T7)
Proof: run sources | test never_reports_each_rising_edge ... ok

**C59** - `source_time` holds the timers. (IT-12, AC 1; T8)
Proof: run sources | test every_from_the_stdlib_fires_once_per_period ... ok

**C60** - No `buck_commits`, `buck_records` or `buck_time` target remains. (IT-12, AC 1; T8)
Proof: `! test -e crates/piperine-parity/tests/buck_commits.rs && ! test -e crates/piperine-parity/tests/buck_records.rs && ! test -e crates/piperine-parity/tests/buck_time.rs`

**C61** - The fixtures live under `tests/digital/` and name no `buck` crate. (IT-12, AC 2; T8)
Proof: `test -f crates/piperine-parity/tests/digital/time.msl && ! grep -rq "buck::" crates/piperine-parity/tests/digital`

**C62** - The README's "On demand" block gives the four lines and when to run them. (IT-13, AC 1; T18)
Proof: run docs | test the_buck_readme_says_when_to_run_the_buck_on_demand ... ok

**C63** - The README keeps what `docs.rs` already holds it to. (IT-13, AC 1; T18)
Proof: run docs | test the_buck_readme_points_at_the_scenarios ... ok

**C64** - `CLAUDE.md` names the integrated gate and the buck on demand. (IT-13, AC 2; T19)
Proof: run docs | test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok

**C66** - The root `tests/` guard accepts the modules of the declared `tests/integrated/main.rs` target and still refuses an undeclared `.rs`. (IT-03, AC 1; T10)
Proof: run shape | test root_tests_hold_no_end_to_end_target ... ok

**C65** - `.specs/STATE.md` records MD-76 with the gate migration. (IT-13, AC 3; T20)
Proof: `grep -q "^### MD-76" .specs/STATE.md && grep -q "unknown-semantics-per-level" .specs/STATE.md`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-21, user AFK, defaults D1-D16 in spec.md). Measured on `main` `856d2668`
(`CARGO_TARGET_DIR=/var/tmp/piperine-target-it`, `CARGO_PROFILE_DEV_DEBUG=0`, debug build, one target at a time):
`buck_time` 4 s, `buck_power` 7 s, `buck_whole` 16 s, `buck_dpwm` 5 s, `buck_boundary` 7 s, `buck_commits` 6 s,
`buck_records` 10 s, `buck_control` 5 s, `buck_closed_loop` 330 s, `buck_supervisor` 5 s, `piperine-cli` `buck`
404 s, `piperine-cli` `book` 269 s (its shared fixture runs the 600 µs start), root `book_cli` 17 s (whole target), `book_mixed` 15 s (the Book's own averaged buck: stays, D4 of Out of Scope), `piperine-api` `buck_long` 167 s; `buck_scenarios` not re-measured (25 to 90 min recorded by `buck-scenarios`, six reds). The buck's share of the default run: about 20 min plus `buck_scenarios`. Cargo's
`test = false` verified on a scratch package: skipped by `cargo test`, `--tests` and `--all-targets`, run by
`--test <name>`.
**Settled B1 (T1-T8):** full root `cargo test --no-fail-fast` green, 17m15s wall (no buck simulation target; its longest remaining target 83 s). Defaults: `test = false` entries and README lines added task by task; the `source_*` targets keep `buck/support.rs` and open their fixture by path under `tests/digital/`; `book_buck`'s `BookListing::printed` compares the binary's whole stdout to the block; `the_on_demand_targets_are_the_buck_set` pins D3's twelve; the approval lines of spec/tasks are the relayed standing authorization (validate_tasks --execute flags their wording only; the Build line was run by hand, green).
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1 → T2 → T3 → T4 → T5          (the buck on demand)
Phase 2:  T6 → T7 → T8                     (renamed language proofs)
Phase 3:  T9 → T10                         (harness, package)
Phase 4:  T11 → T12 → T13 → T14 → T15      (scenarios)
Phase 5:  T16 → T17                        (ngspice, binary)
Phase 6:  T18 → T19 → T20                  (records)
```

Execution is strictly sequential. Batches (every worker Sonnet): B1 = Phases 1-2 (T1-T8, 8 tasks),
B2 = Phases 3-4 (T9-T15, 7 tasks), B3 = Phases 5-6 (T16-T20, 5 tasks). After B3: `check_proofs.py`, then the
Verifier (a different model), then the buck run once (D11), its result reported.

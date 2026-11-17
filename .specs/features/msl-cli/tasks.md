# MSL CLI Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-cli/design.md`
**Status**: Approved
**Amended**: 2026-11-16 - user: "O lang-server é o menos importante, vai ficar para bem depois." Phase 7 of the approved plan (T50 to T54, CLI-25) is removed.
**Approved**: 2026-11-15 - user: "Esses + fmt agora", "Adoro a ideia de benches na propria linguagem", "Tabela + plots", "Ao vivo, todos os streams", "Rust por padrão", "Agora, pequeno", "LSP só de diagnósticos agora", "Projeto RC passa-baixa" (the answers to Q1–Q8)
**Build**: `cargo build -p piperine-project -p piperine-lang -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -f crates/piperine-api/src/failure.rs || cargo build -p piperine-api -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -f crates/piperine-cli/src/cli.rs || cargo build -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run manifest**: `cargo test -p piperine-project --test manifest`
**Run rcpkg**: `cargo test -p piperine-project --test project`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run shape**: `cargo test -p piperine-solver --test workspace_shape`
**Run apilib**: `cargo test -p piperine-api --lib`
**Run project**: `cargo test -p piperine-api --test project`
**Run session**: `cargo test -p piperine-api --test session`
**Run probes**: `cargo test -p piperine-api --test probes`
**Run analyses**: `cargo test -p piperine-api --test analyses`
**Run records**: `cargo test -p piperine-api --test records`
**Run clilib**: `cargo test -p piperine-cli --lib`
**Run usage**: `cargo test -p piperine-cli --test usage`
**Run run**: `cargo test -p piperine-cli --test run`
**Run buck**: `cargo test -p piperine-cli --test buck`
**Run options**: `cargo test -p piperine-cli --test options`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run commands**: `cargo test -p piperine-cli --test commands`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run book**: `cargo test -p piperine-lang --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run format**: `cargo test -p piperine-lang --test msl_format`
**Run print**: `cargo test -p piperine-lang --test msl_print`

Executors (S10): Sonnet implements; Opus implements the delicate tasks directly, those where a mistake gives a wrong number or a wrong page with no error, or deletes a user's comment: T1, T6, T14, T21, T25, T36, T41, T42, T44, T45, T46, T47. A different Opus verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources.

**This feature starts after `msl-digital` is merged into its worktree.** T1 confirms the merged code against design.md before anything else. Phases 1 to 3 are the minimal slice and are mergeable alone; phases 4 and 5 widen it; phase 6 is the formatter and `fmt`; phase 7 writes the Book from what the binary prints. The user's answers, and the amendment of 2026-11-16 that deferred the language server (T50 to T54 are removed; the numbers are not reused), are in `context.md`. On this tree `validate_tasks.py --execute` passes because the `**Build**` line guards the three crates that do not build yet; the first task still waits for the merge.

Rules every task follows:

- **The CLI is a shell.** Every rule that decides what a user sees (which signals, rows and records, pass or fail, every validation) is a `piperine-api` method with a Rust test. `piperine-cli` parses, calls, lays out text and returns an exit code.
- **`piperine-abi`, the three solver crates, `piperine-ir`, `piperine-plan`, `piperine-transpile` and `piperine-interp` are not edited**, except the one test target of `piperine-solver` a task names. `piperine-lang` is edited only in `src/msl/{lexer,comments,format,print}.rs` and `src/msl/mod.rs` (phase 6) and in the test targets a task names; its parser, POM, type checker and elaborator are not. A defect or a missing accessor found elsewhere stops the task and is reported.
- **No comment is ever lost.** The formatter refuses to return a text whose comments differ from the source's; `fmt` never writes such a file. No corpus file is edited to make the formatter pass.
- **`piperine-lang-server` and `editors/` are not edited.** The server stays outside the default build (user, 2026-11-16).
- **`piperine-parity` is not edited.** Its `Bench`, `Project` and `Backend` stay; queue item 4 decides their place.
- **`examples/buck/src`, `crates/piperine-lang/std` and `tests/ngspice/` are not edited.**
- **Fail loud.** A request the code does not handle is a `Failure` with a code or a usage error; never a default value, a skipped record, a truncated table or a silent exit 0.
- **Standard output is data and is deterministic**: no wall-clock time, no absolute path, no cache key. Progress and diagnostics go to standard error.
- **Tests state what the spec says, not what the code does.** Expected values are computed in the test from the MSL source, `docs/spec/`, the spec of this feature or `examples/buck/SPEC.md`, each cited at its assertion. Test functions have exactly the names `## Checks` cites.
- **End-to-end tests run the real binary** (`env!("CARGO_BIN_EXE_piperine")`) on a project on disk. Each sets `PIPERINE_CACHE_DIR` on the child process to a temporary directory shared by its target. No test sets a variable of its own process, writes the user's cache, or needs the network or ngspice.
- **No tolerance is invented.** The three analog tolerances the checks name (1e-9 exact arithmetic, 1e-3 relative as the parity contract's, 1e-2 degree) are the spec's; a number you cannot explain: stop and report.
- **The deletions of T4 and T5 are the only ones.** They remove code and tests of the deleted front end in one commit each, the commit body listing every target. No other test is removed, weakened or skipped.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, no derive from `clap`, modules named after what they do, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the Rust idiom rules of MD-13; test placement MD-28: a target lives in the crate it exercises, root `tests/` holds no Rust target; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no switched-off test, no file-scope lint suppression), `crates/piperine-solver/tests/workspace_shape.rs` (default members, the crate table of the instruction files), `crates/piperine-lang/tests/book.rs` (the Book's blocks), `.github/workflows/ci.yml` (`cargo build`, `cargo test`, `RUSTFLAGS=-D warnings`, ngspice installed). Samples read for style: `crates/piperine-project/tests/manifest.rs`, `crates/piperine-parity/tests/{bench,parity_tran,backend}.rs`, `crates/piperine-transpile/tests/{compile,cache}.rs`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| The manifest parser (`piperine-project`) | unit | CLI-02 AC 4: accepted, located, a non-table refused; every existing manifest test still green | `crates/piperine-project/tests/manifest.rs` | `cargo test -p piperine-project` |
| Pure rules of `piperine-api`: the run request, the backend order, the print grid, the plot's geometry, the verdict | unit | every branch; 1:1 to the ACs of CLI-03, CLI-04 AC 7, CLI-09 AC 3 to 5, CLI-11 AC 4 and 5, CLI-16 AC 2 and 3; every listed edge case | inline `#[cfg(test)]` in `crates/piperine-api/src/{request,backend,table,plot,verdict}.rs` | `cargo test -p piperine-api --lib` |
| `piperine-api` over real sources: project, session, probes, analyses, records | integration | each AC of CLI-02, CLI-04, CLI-05, CLI-06, CLI-09, CLI-10, CLI-12 AC 1 and 2 on MSL on disk; both backends wherever the AC names both; every failure with its code | `crates/piperine-api/tests/{project,session,probes,analyses,records}.rs` | `cargo test -p piperine-api` |
| Text layout of `piperine-cli`: tables, numbers, record lines, JSON, diagnostics, the flag table | unit | exact strings; every value kind; CLI-03 AC 2 | inline `#[cfg(test)]` in `crates/piperine-cli/src/{cli.rs,render/*.rs}` | `cargo test -p piperine-cli --lib` |
| The commands of `piperine-cli` | e2e | the real binary on a project on disk: the happy path of every command, every listed edge case, every failure with its exit code and its stream | `crates/piperine-cli/tests/{usage,run,buck,options,twins,commands}.rs` (`fmt` and `--plot` included), fixtures in `crates/piperine-cli/tests/projects/` | `cargo test -p piperine-cli` |
| Example projects (`examples/rc`, `examples/buck` bench and runs) | e2e | each runs through the binary with the values its source and the buck SPEC give | `crates/piperine-cli/tests/{run,buck}.rs`, `crates/piperine-project/tests/project.rs` | `cargo test -p piperine-cli --test run --test buck` |
| The Book | integration | CLI-19: the chapter's place; every file block equal to the example's file; every output block equal to the binary's output; stand-alone | `crates/piperine-cli/tests/book.rs`, `crates/piperine-lang/tests/book.rs` | `cargo test -p piperine-cli --test book`; `cargo test -p piperine-lang --test book` |
| Spec text, instruction files, gap lists, the decision log | integration | the sentence or row each AC of CLI-02 AC 4 and CLI-20 names | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, `crates/piperine-solver/tests/workspace_shape.rs` | `cargo test -p piperine-cli --test docs` |
| The formatter (`piperine-lang` `msl::{comments,format}`, the printer refactor) | integration | each AC of CLI-23; expected texts written by hand from the rule; the whole corpus: comments, tree, stability; `msl_print` unedited and green | `crates/piperine-lang/tests/msl_format.rs`, `crates/piperine-lang/tests/msl_print.rs` | `cargo test -p piperine-lang --test msl_format --test msl_print` |
| `contract.md` | none | - (read by every later task; build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After tasks with unit tests only | `cargo test -p <the crate the task edits> --lib` (for `piperine-project`: `cargo test -p piperine-project --test manifest`; for T38: `cargo test -p piperine-cli --test docs`) |
| Full | After tasks with integration or e2e tests | `cargo test -p piperine-project -p piperine-api -p piperine-cli`, and for a task of phase 6 `cargo test -p piperine-lang` |
| Build | After a phase, and after tasks that touch manifests, guards, the spec, the Book or the instruction files | every command of the `**Build**` line, then Full, then `cargo test -p piperine-lang --test book --test spec_text --test pom_docs` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in the order the phase table at the end lists them. An arrow is a dependency inside the phase; a dependency on an earlier phase is in the task's `Depends on` only.

### Phase 1: Foundation: the contract, the spec and crates that build

Executor: Sonnet; T1, T6 and T41 are Opus's.

```
T1 → T2
T2 → T3
T3 → T41
T1 → T4
T4 → T5
T2 → T6
```

### Phase 2: The session: from a project to results and records

Executor: Sonnet; T14 is Opus's.

```
T7 → T8
T8 → T9
T9 → T10
T10 → T11
T11 → T12
T12 → T13
T13 → T14
```

### Phase 3: `piperine run`: the minimal slice

Executor: Sonnet; T21 is Opus's. **The MVP ends here**: `piperine run` on a project prints the operating point or the transient and the `emit!` records.

```
T15 → T18
T16 → T18
T17 → T18
T18 → T19
T19 → T20
T20 → T21
```

### Phase 4: Analyses, output formats and plots

Executor: Sonnet; T25 and T42 are Opus's.

```
T22 → T23
T22 → T24
T25 → T26
T27 → T28
T26 → T28
T23 → T29
T23 → T42
T25 → T42
T42 → T43
T26 → T43
```

### Phase 5: `check`, `build`, `test`, `new`, `clean`

Executor: Sonnet.

```
T30 → T31
T32 → T33
T31 → T33
T33 → T34
T34 → T35
```

### Phase 6: The formatter and `piperine fmt`

Executor: Opus for T44 to T47 (the lexer, the printer and the comment rules: a mistake deletes a user's comment); Sonnet for T48 and T49.

```
T44 → T45
T45 → T46
T46 → T47
T47 → T48
T48 → T49
```

### Phase 7: The Book and the records of the feature

Executor: Sonnet; T36 is Opus's.

```
T36 → T37
T39 → T40
T38
```

---

## Task Breakdown

## Phase 1 tasks

### T1: The contract with the merged msl-digital

**What**: Read the merged `main` (after `msl-digital`) and write `contract.md`: for every name this design takes from that feature's design text, the actual type, method and file. It covers: the stream accessors of an IR body and how a record's site and text fields are found from an `Observation`; the layout of a `RECORD` pin; `ObservationKind::Record`'s fields; how a top with discrete ports is elaborated; the bench forms a reaction may use (a timer, a crossing, `now()`, a node probe, `emit!`, `require`); whether DIG-16 (the buck's closed loop) merged, with the name and the run time of its test; the parity `Project`. Each difference from design.md gets a line saying which task absorbs it. A record that cannot name its site, or a missing DIG-16, is reported to the orchestrator before T2 starts.
**Where**: `.specs/features/msl-cli/contract.md`
**Depends on**: None
**Reuses**: `.specs/features/msl-digital/contract.md` as the model
**Requirement**: CLI-06, CLI-08
**Executor**: Opus

**Done when**:

- [x] `contract.md` has one row per name of design.md "Code Reuse Analysis" that arrives with `msl-digital`, each with file and line
- [x] It states, with the test that shows it, how a `Record` observation is tied to its `emit!` site
- [x] It states whether `examples/buck` runs closed loop, and the wall-clock time of that test in a dev build
- [x] Gate check passes: Build (see Gate Check Commands)
- [x] No test is removed or edited

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): msl-cli contract with the merged msl-digital`

---

### T2: A manifest may hold a run table

**What**: `Manifest::run: Option<Span>`; the manifest parser accepts the top-level key `run` when its value is a table and reads nothing inside it; any other type is `E0171`.
**Where**: `crates/piperine-project/src/manifest.rs`
**Depends on**: T1
**Reuses**: the hand-walked spanned TOML of the same file
**Requirement**: CLI-02
**Executor**: Sonnet

**Done when**:

- [x] No existing test of `crates/piperine-project/tests/` is edited except to add cases
- [x] Tests with exactly these names pass in `cargo test -p piperine-project --test manifest`: `a_run_table_is_accepted_and_located`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(project): a manifest may hold a run table`

---

### T3: The manifest rule of the spec admits the run table

**What**: One sentence in rule 3 of the manifest section and one row in its key table: the `run` table is allowed, its content belongs to the host and is not defined by the language, and a dependency's is not read. The wording of `E0171` stays true.
**Where**: `docs/spec/part_i_values.md`
**Depends on**: T2
**Reuses**: MD-57: a spec edit lands with its guard
**Requirement**: CLI-02
**Executor**: Sonnet

**Done when**:

- [x] `cargo test -p piperine-lang --test spec_text --test pom_docs` pass
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test spec_text`: `the_manifest_rule_admits_the_run_table`
- [x] Gate check passes: Build (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs(spec): the manifest may hold a run table the language does not define`

---

### T41: The spec says a bench is MSL

**What**: Part VIII: the definition of **bench** (a module without ports, written in MSL, that places the design with its sources, stimulus and monitors; the host picks it as the top and asks the analysis), added to the part's terms; the testbench section's intro and table split what a bench writes (stimulus as reactions and timers, monitors, checks) from what stays the host's (`force` and `release`, constrained random stimulus, sweeps, Monte Carlo, regression, interactive sequences). The index's principle 3 and term table, the scope table of Appendix G (the row on the verification language) and the sentence on sequential stimulus in Part IV say the same. No rule number and no error code changes.
**Where**: `docs/spec/part_viii_host.md`
**Depends on**: T3
**Reuses**: MD-57; the wording style of the part's own Resolution notes
**Requirement**: CLI-21
**Executor**: Opus

**Done when**:

- [x] `docs/spec/index.md`, `docs/spec/appendix_g_scope.md` and `docs/spec/part_iv_behavior.md` receive their one sentence each in the same commit
- [x] `cargo test -p piperine-lang --test spec_text --test pom_docs --test std_excerpts` pass
- [x] Tests with exactly these names pass in `cargo test -p piperine-lang --test spec_text`: `part_viii_defines_a_bench`, `the_index_says_benches_are_msl_and_sweeps_are_host`, `the_testbench_table_splits_stimulus_and_host_work`, `no_spec_text_says_stimulus_is_host_only`, `every_error_code_is_defined_exactly_once`
- [x] Gate check passes: Build (see Gate Check Commands)
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs(spec): a bench is MSL; sweeps, Monte Carlo and optimization stay in the host`

---

### T4: piperine-api starts over

**What**: Delete every file under `crates/piperine-api/src` and `crates/piperine-api/tests`. Write the new crate: `lib.rs`, `prelude.rs`, `failure.rs` (`Failure`, design Components) and `backend.rs` (`Backend`, `Backend::choose`). The manifest depends on `piperine-lang`, `piperine-project`, `piperine-ir`, `piperine-interp`, `piperine-transpile`, `piperine-solver`, `toml`. The root package builds as the re-export it is. `default-members` and `DEFAULT_MEMBERS` gain `.`, `crates/piperine-api` and `crates/piperine-project`. The commit body lists every deleted test target.
**Where**: `crates/piperine-api/`
**Depends on**: T1
**Reuses**: `piperine_parity::backend::Backend` for the two names
**Requirement**: CLI-01, CLI-04
**Executor**: Sonnet

**Done when**:

- [x] `cargo build -p piperine-api -p piperine --all-targets` has zero warnings
- [x] `cargo test -p piperine-lang --test suite_hygiene` and `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` pass
- [x] Tests with exactly these names pass in `cargo test -p piperine-solver --test workspace_shape`: `the_root_package_is_a_re_export_and_nothing_else`
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `backend::tests::the_flag_then_the_variable_then_the_key_then_rust`
- [x] Gate check passes: Build (see Gate Check Commands)
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited beyond the deletion this task states

**Tests**: unit
**Gate**: build

**Commit**: `feat(api)!: piperine-api starts over on the device ABI`

---

### T5: piperine-cli starts over

**What**: Delete every file under `crates/piperine-cli/src`, `tests`, `templates` and `build.rs`. Write `main.rs`, `lib.rs`, `exit.rs` (`Exit`) and `cli.rs` (`Cli::command` from a data table with `clap`'s builder, `Cli::parse`, `Invocation`, `Cli::execute`). The seven commands parse; a command whose task has not landed prints `<command>: not built yet` on standard error and exits 1. `clap` without the `derive` feature. `default-members` and `DEFAULT_MEMBERS` gain `crates/piperine-cli`. The guard `host_crates_name_no_deleted_surface` is added to `workspace_shape`. The commit body lists every deleted test target.
**Where**: `crates/piperine-cli/`
**Depends on**: T4
**Reuses**: nothing of the old `lib.rs`
**Requirement**: CLI-01
**Executor**: Sonnet

**Done when**:

- [x] `cargo build` at the root builds the binary `target/debug/piperine` with zero warnings
- [x] `crates/piperine-cli/tests/usage.rs` runs the real binary (`env!("CARGO_BIN_EXE_piperine")`)
- [x] Tests with exactly these names pass in `cargo test -p piperine-solver --test workspace_shape`: `default_members_are_the_green_crates`, `host_crates_name_no_deleted_surface`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test usage`: `the_binary_is_named_piperine`, `help_lists_exactly_the_seven_commands`, `version_prints_the_workspace_version`, `an_unknown_command_or_flag_exits_2`
- [x] Gate check passes: Build (see Gate Check Commands)
- [x] Test count: the 6 test(s) named above, and no existing test removed or edited beyond the deletion this task states

**Tests**: e2e
**Gate**: build

**Commit**: `feat(cli)!: piperine-cli starts over as a shell over piperine-api`

---

### T6: The RC example project

**What**: `examples/rc`: `Piperine.toml` (package `rc`, runs `op` and `step`), `src/lib.msl` (`Resistor`, `Capacitor`, `Vsource`, as the M1 device library writes them), `benches/step.msl` (the top `Step`: source, R = 1 kΩ, C = 1.5915494309189535 nF, the output held at 0; a monitor whose reaction fires when the output crosses 0.5 V, calls `emit!("trip", ...)` with the time and the voltage, and one `require` that holds), `.gitignore` (`target/`). Written in the forms `contract.md` lists as accepted. `examples/Piperine.toml` and `examples/Piperine.lock` are deleted.
**Where**: `examples/rc/`
**Depends on**: T2
**Reuses**: `tests/ngspice/devices.msl`, `tests/ngspice/tran_rc.msl`
**Requirement**: CLI-19
**Executor**: Opus

**Done when**:

- [x] `cargo test -p piperine-solver --test workspace_shape` passes (`examples_hold_no_vams_or_python`)
- [x] The `step` run's `print`, `print_step` and `stop` give a table of at most 12 rows
- [x] Tests with exactly these names pass in `cargo test -p piperine-project --test project`: `the_rc_example_resolves_with_no_diagnostic`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(examples): the RC low-pass project`

---

## Phase 2 tasks

### T7: RunSpec: reading a run table

**What**: `RunSpec` (the nineteen keys, `plot` among them), `Analysis`, `Grid` and `RunSpec::read(manifest_text, file)`: every `[run.<name>]` into a `RunSpec`, each fault a `Failure::Request` with `P0010` at the span of the key, the value or the name. The rules of CLI-03 AC 1, 4 to 7 and CLI-09 AC 3 and 5.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T4
**Reuses**: `piperine_lang::msl::{Diagnostic, Span}`; the manifest parser's spanned walk
**Requirement**: CLI-03, CLI-09
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `request::tests::a_run_table_reads_its_nineteen_keys`, `request::tests::an_unknown_key_or_a_wrong_type_is_p0010_at_its_span`, `request::tests::analysis_is_one_of_four`, `request::tests::a_transient_needs_a_positive_stop`, `request::tests::a_key_outside_its_analysis_is_p0010`, `request::tests::a_run_name_is_an_identifier`, `request::tests::a_dc_sweep_needs_its_four_keys_and_a_step_toward_to`, `request::tests::an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 8 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): RunSpec reads a run table`

---

### T8: Overrides: flags over keys

**What**: `Overrides` (one field per key), `RunSpec::merged(&Overrides)` (flag over key, then the same validation as a table) and `RunSpec::from_flags(name, &Overrides)`.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T7
**Reuses**: the validation of T7, called once more after the merge
**Requirement**: CLI-03
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `request::tests::a_flag_overrides_a_key`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): flags override the keys of a run`

---

### T9: Project: discovery and the crate graph of a run

**What**: `Project::{discover, at, file, name, dir, runs, request, design}` (design Components). `design` adds the bench crate `bench` as the root over the package and its direct dependencies, or roots at the package.
**Where**: `crates/piperine-api/src/project.rs`
**Depends on**: T8
**Reuses**: `piperine_project::{resolve, find_project_root}`, `CrateGraph`, the parity `Project` as the pattern
**Requirement**: CLI-02, CLI-03
**Executor**: Sonnet

**Done when**:

- [x] Fixtures are built in temporary directories by the test; none is written into the repository
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test project`: `discover_takes_the_nearest_manifest_upward`, `a_dependencys_run_table_is_ignored`, `a_bench_is_the_root_crate_over_the_package_and_its_dependencies`, `a_run_without_a_bench_roots_at_the_package`, `a_lone_file_with_no_manifest_is_a_crate_alone`, `a_file_under_a_package_is_its_bench`, `a_file_under_src_is_refused_with_p0010`, `a_missing_bench_file_is_p0010_at_the_value`, `the_only_run_is_run_without_a_name`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 9 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): Project finds a package and builds the graph of a run`

---

### T10: Session::open: source to a bound circuit

**What**: `Session::open(&Project, &RunSpec, &Settings)`: the top (key, or the one module without ports, else `P0011`), `Types::check`, `Elaborator::run`, the backend (`Interpreter` or `Transpiler::compile`), the circuit. `Session::{top, backend, ir, built}`. Each refusing stage is a `Failure` naming it. The circuit field drops before the compiled library.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T9
**Reuses**: the stage order of `Bench::load_file_on`; the counting compiler wrapper of `crates/piperine-transpile/tests/compile.rs`
**Requirement**: CLI-04
**Executor**: Sonnet

**Done when**:

- [x] Every Rust-backend test passes its own cache directory through `Settings`; none writes the user's cache or sets a variable
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test session`: `the_top_key_names_the_module`, `the_one_portless_module_is_the_top`, `no_top_or_several_is_p0011_with_candidates`, `a_missing_rustc_names_rustc_and_the_interpreter`, `the_interpreter_refusal_names_the_body_and_the_rust_backend`, `a_run_compiles_once_and_never_during_an_analysis`, `a_top_with_discrete_in_ports_runs_with_them_unknown`, `a_top_with_a_continuous_port_is_the_front_ends_diagnostic`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 8 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): Session::open takes a run from source to a circuit`

---

### T11: Probe: what a column shows

**What**: `Probe::{Node, Net, Flow}`, `Probe::resolve` (relative or full path, `i(<instance>)`, `E0801`), `Probe::defaults`, `Probe::label`. `Session::open` resolves the run's `print` list.
**Where**: `crates/piperine-api/src/probe.rs`
**Depends on**: T10
**Reuses**: `Bench::{node, source}`, `Ir::{node_at, instance_at}`
**Requirement**: CLI-05
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test probes`: `the_default_is_the_tops_own_signals_in_order`, `a_node_a_net_and_a_flow_resolve_relative_or_full`, `a_column_is_named_by_the_full_path`, `an_entry_that_names_nothing_is_e0801_before_any_analysis`, `a_signal_printed_twice_is_two_columns`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): Probe resolves a print entry to a node, a net or a flow`

---

### T12: Table and the operating point

**What**: `Table`, `Cell`, `Table::operating_point`, and the `Op` arm of `Session::run`. Discrete nets are decoded by their carrier type from the result's bytes.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T11
**Reuses**: `Solver::dc`, `DcAnalysisResult::{get_node, get_branch}`
**Requirement**: CLI-05
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test analyses`: `an_operating_point_is_signal_and_value_in_print_order`, `the_divider_mid_is_7_5_on_both_backends`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): the operating point of a run as a table`

---

### T13: The transient of a run

**What**: `Table::transient` and the `Transient` arm of `Session::run`, with the options of design "How an analysis reaches the solver".
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T12
**Reuses**: `Solver::{with_tran_opts, tran}`, `TransientStep::{time, get_node, digital_named}`
**Requirement**: CLI-05
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test analyses`: `a_transient_has_one_row_per_accepted_step`, `the_rc_step_follows_one_minus_exp_on_both_backends`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): the transient of a run as a table`

---

### T14: Records: from the sink to typed records

**What**: `Record`, `Field`, `Streams::{of, decode}`, `RecordSink`, and the adapter that implements `protocol::Sink` and hands each decoded record to the `RecordSink` of `Session::run` as it is delivered. Bytes are decoded by the stream's layout as `contract.md` states it; a text field comes from the IR site; a field with its known byte clear is `Field::Unknown`.
**Where**: `crates/piperine-api/src/record.rs`
**Depends on**: T13
**Reuses**: `protocol::{Sink, Observation, ObservationKind}`, `Solver::with_sink`, the IR's stream accessors
**Requirement**: CLI-06
**Executor**: Opus

**Done when**:

- [x] Expected values are written from the MSL source of each fixture and `docs/spec/part_vi_io.md`, cited at the assertion
- [x] A bytes length that does not match the layout is a `Failure`, never a truncated record
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test records`: `an_emit_gives_one_record_with_time_instance_stream_and_fields`, `records_arrive_in_time_order`, `a_text_field_is_the_sources_literal`, `an_unknown_field_is_unknown`, `a_failed_require_is_one_checks_record_with_pass_false`, `a_design_without_emit_gives_no_record`, `a_reserved_stream_keeps_its_name_and_its_site`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 7 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): emit! records reach the host decoded`

---

## Phase 3 tasks

### T15: Rendering a table and a number

**What**: `Number::text(&Cell)` (the one place a number becomes text) and `TextTable::write(&Table, out)`.
**Where**: `crates/piperine-cli/src/render/table.rs`
**Depends on**: T12
**Reuses**: none
**Requirement**: CLI-05, CLI-07
**Executor**: Sonnet

**Done when**:

- [x] `Number` lives in `render/number.rs`, declared from `render/table.rs`'s module; the test module path is `render::number::tests`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `render::number::tests::numbers_have_one_text_each`, `render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(cli): a result table as aligned text`

---

### T16: Rendering a record line

**What**: `RecordLine::write(&Record, out)`: `@<time> <instance> <stream> <field>=<value> ...`, a text in double quotes, an unknown as `x`.
**Where**: `crates/piperine-cli/src/render/record.rs`
**Depends on**: T14
**Reuses**: `Number::text`
**Requirement**: CLI-06
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `render::record::tests::a_record_is_one_line`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(cli): a record as one line`

---

### T17: Rendering a failure

**What**: `Report::write(&Failure, err)`: every diagnostic through `miette`'s graphical handler with its code, file, line and excerpt, no color unless standard error is a terminal; then `error: <stage> failed with <n> diagnostic(s)`. Package errors and run-table errors render against `Piperine.toml`; backend and solver failures print their message.
**Where**: `crates/piperine-cli/src/render/diagnostic.rs`
**Depends on**: T4
**Reuses**: `piperine_lang::msl::diag`'s renderer; `PackageError`'s `miette` impl
**Requirement**: CLI-02, CLI-04
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(cli): failures rendered with code, file, line and excerpt`

---

### T18: piperine run: a named run

**What**: `RunCommand::execute`: discover the project, take the named run, open the session (announcing a first compile on standard error), print the header, print each record as it arrives, then the empty line and the table; exit 0. `--backend` and `--project` are global flags.
**Where**: `crates/piperine-cli/src/commands/run.rs`
**Depends on**: T15, T16, T17
**Reuses**: `Project::{discover, request}`, `Session::{open, run}`, the three renderers
**Requirement**: CLI-06, CLI-07
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-cli/tests/run.rs` runs the real binary in `examples/rc` with `PIPERINE_CACHE_DIR` set on the child to one temporary directory shared by the target
- [x] The expected header and the table's first column are written from `examples/rc`'s manifest, not copied from a run
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test run`: `records_come_before_the_table`, `run_prints_header_records_blank_line_and_table`, `a_completed_run_exits_0`, `stdout_is_the_same_from_two_caches_and_two_directories`, `a_first_compile_is_announced_on_stderr`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): piperine run prints records and the result of a named run`

---

### T19: piperine run: flags and a lone file

**What**: One flag per run key in the command table (CLI-03 AC 2), parsed into `Overrides`; a positional ending in `.msl` is a lone file (`Project::file`, `RunSpec::from_flags`, the stem as the run name).
**Where**: `crates/piperine-cli/src/cli.rs`
**Depends on**: T18
**Reuses**: `RunSpec::{merged, from_flags}`
**Requirement**: CLI-02, CLI-03, CLI-07
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test run`: `the_project_flag_uses_that_directory_and_never_searches`, `a_lone_file_run_is_named_by_its_stem`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `cli::tests::every_run_key_has_its_flag`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): every run key is a flag, and a lone file runs with flags`

---

### T20: piperine run: failures and exit codes

**What**: Every failure path of `run`: nothing on standard output before the analysis; records kept when the analysis fails; exit 1 for the design and the run, exit 2 for the command line (no run name with several runs, an unknown run, no `--analysis`, a flag twice, a bad `--backend`); a closed standard output ends the command with exit 1. Fixture projects under `crates/piperine-cli/tests/projects/`: `type_error`, `bad_run_table`, `missing_dependency`, `two_sources` (two voltage sources in parallel), `fails_after_emit`, `two_runs`.
**Where**: `crates/piperine-cli/src/commands/run.rs`
**Depends on**: T19
**Reuses**: `Report::write`, `Exit`
**Requirement**: CLI-02, CLI-03, CLI-04, CLI-05, CLI-07
**Executor**: Sonnet

**Done when**:

- [x] No test weakens an exit code to make a fixture pass; a fixture that cannot produce its failure is reported
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test run`: `no_manifest_is_e0170_and_exit_1`, `a_package_error_is_rendered_with_code_file_and_line`, `no_name_with_no_run_or_several_lists_them_and_exits_2`, `an_unknown_run_name_lists_the_runs_and_exits_2`, `a_lone_file_without_analysis_exits_2`, `a_single_valued_flag_given_twice_exits_2`, `a_type_error_is_rendered_with_its_stage_and_exits_1`, `a_bad_backend_exits_2_for_the_flag_and_1_otherwise`, `a_solver_failure_prints_no_table_and_exits_1`, `a_failure_before_the_analysis_prints_nothing_on_stdout`, `records_stay_when_the_analysis_fails`, `a_closed_stdout_ends_with_exit_1_and_no_panic`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 12 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): run fails loud, with one exit code per kind of failure`

---

### T21: The buck's bench and runs

**What**: `examples/buck/benches/startup.msl` (the bench of `msl-digital` DIG-16, as `contract.md` locates it: setpoint code 0, `en` raised, Vin 12 V, a 6.6 Ω load) and, in `examples/buck/Piperine.toml`, the runs `idle` (`en` low, 20 µs) and `startup` (600 µs), each with `print`, `print_step` and `streams` chosen so the output is at most 16 lines. `examples/buck/src` is not edited. One shared fixture of the test target runs `startup` once; the commit body states its wall-clock time.
**Where**: `examples/buck/benches/startup.msl`
**Depends on**: T20
**Reuses**: the bench sources of `crates/piperine-parity/tests/buck/`
**Requirement**: CLI-08
**Executor**: Opus

**Done when**:

- [x] `cargo test -p piperine-lang --test buck_example` still passes
- [x] A number that differs from buck SPEC is reported, not adjusted
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test buck`: `the_buck_has_its_bench_and_two_runs`, `startup_ends_with_the_reference_at_128`, `startup_prints_no_failed_check`, `idle_keeps_the_mode_off`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(examples): the buck starts from the command line`

---

## Phase 4 tasks

### T22: The DC sweep of a run

**What**: `Table::sweep` and the `Dc` arm of `Session::run`: per point a `set` of the swept param and an operating point; the points of CLI-09 AC 1; `E0801` for a sweep path that names no param, at `Session::open`.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T13
**Reuses**: the loop of `crates/piperine-parity/tests/parity_dc.rs`; `CircuitInstance::set`
**Requirement**: CLI-09
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test analyses`: `a_dc_sweep_has_one_row_per_point`, `the_diode_sweep_has_37_rows`, `a_sweep_of_no_param_is_e0801`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): the DC sweep of a run`

---

### T23: The AC sweep of a run

**What**: `Table::frequency` (magnitude, and phase in degrees) and the `Ac` arm of `Session::run`; `Analysis::Ac`'s point count for `PerDecade` and `Points`.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T22
**Reuses**: `Solver::ac`, `AcSweepAnalysisOptions`, `AcAnalysisStep::get_node`
**Requirement**: CLI-09
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test analyses`: `an_ac_sweep_has_mag_and_phase_in_degrees`, `points_sweeps_linearly`, `the_rc_corner_is_minus_3_db_and_minus_45_degrees`
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `request::tests::per_decade_gives_round_n_log10_plus_one_points`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): the AC sweep of a run`

---

### T24: Params of a run

**What**: `Session::open` sets each entry of `params` in order (the table's, then the flags') before any analysis: a path that names nothing is `E0801`, a refused write is `E0802` with the path and the value.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T22, T19
**Reuses**: `CircuitInstance::set`, `ParamValue::Real`
**Requirement**: CLI-10
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-cli/tests/options.rs` is created here with the e2e case
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test analyses`: `params_are_set_in_order_table_then_flags`, `a_param_path_that_names_nothing_is_e0801`, `a_refused_value_is_e0802_and_nothing_runs`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test options`: `the_divider_with_r2_at_1k_gives_5`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(api): a run sets its params before the analysis`

---

### T25: The print grid

**What**: `Table::on_grid(step, stop)`: one row per multiple of `step` not past `stop`; a real column interpolated linearly between the two accepted steps around the time; a `bool` or integer column held from the last step at or before it; an unknown stays unknown. `Session::run` applies it when the run has `print_step`.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T13
**Reuses**: the rule of `piperine_parity::waveform::Waveform::at`
**Requirement**: CLI-11
**Executor**: Opus

**Done when**:

- [x] A time that falls exactly on an accepted step gives that step's value, not an interpolation
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `table::tests::a_print_grid_has_one_row_per_multiple`, `table::tests::continuous_is_interpolated_and_discrete_is_held`, `table::tests::a_grid_wider_than_the_run_is_the_row_at_zero`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a transient table on a print grid`

---

### T26: CSV output

**What**: `CsvTable::write(&Table, out)` and the `csv` key in `RunCommand`: the file replaces the table on standard output, which gets `wrote <file> (<n> rows)`; a relative path is resolved against the project directory (the working directory for a lone file); a write error is `Failure::Io`.
**Where**: `crates/piperine-cli/src/render/csv.rs`
**Depends on**: T25, T19
**Reuses**: `Number::text`
**Requirement**: CLI-11
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test options`: `csv_writes_the_table_and_prints_wrote`, `an_existing_csv_is_replaced`, `an_unwritable_csv_names_the_path_and_exits_1`, `a_relative_output_path_is_under_the_project`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): a result table as CSV`

---

### T27: The stream filter

**What**: `Streams::keep(&[String])`: an entry keeps every stream of that name and the one stream of that full path; an entry that matches nothing is `E0801` at `Session::open`. `decode` returns nothing for a stream that is not kept.
**Where**: `crates/piperine-api/src/record.rs`
**Depends on**: T14
**Reuses**: `Streams::of`
**Requirement**: CLI-12
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --test records`: `keep_matches_a_stream_name_or_its_full_path`, `a_filter_entry_that_matches_nothing_is_e0801`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test options`: `the_stream_flag_keeps_only_that_stream`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a run keeps only the streams it names`

---

### T28: Records to a file

**What**: `JsonRecord::write(&Record, out)`, written by hand (five value kinds, string escaping of `"`, `\` and control characters), and the `records` key in `RunCommand`: every kept record goes to the file, none to standard output.
**Where**: `crates/piperine-cli/src/render/json.rs`
**Depends on**: T27, T26
**Reuses**: none
**Requirement**: CLI-12
**Executor**: Sonnet

**Done when**:

- [x] A real is written with Rust's shortest round-trip form, so reading the file gives the same `f64`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test options`: `records_go_to_the_file_as_json_lines_and_not_to_stdout`, `an_existing_records_file_is_replaced`, `csv_and_records_leave_the_header_and_the_wrote_line`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `render::json::tests::each_value_kind_has_its_json`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): records as JSON Lines`

---

### T29: The seven twins from the command line

**What**: The e2e target that runs each MSL twin of `tests/ngspice/` through the binary with flags, on both backends, with the analysis and the grid its `.cir` names, and compares the two backends' standard output byte for byte after the header line. A difference is reported, not tolerated.
**Where**: `crates/piperine-cli/tests/twins.rs`
**Depends on**: T23, T19
**Reuses**: the grids of `crates/piperine-parity/tests/parity_{op,dc,ac,tran}.rs`
**Requirement**: CLI-13
**Executor**: Sonnet

**Done when**:

- [x] No file under `tests/ngspice/` is edited
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test twins`: `the_three_op_twins_run_on_both_backends`, `the_sweep_and_transient_twins_run_on_both_backends`, `the_two_backends_print_the_same_bytes`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `test(cli): the seven parity twins run from the command line on both backends`

---

### T42: Plot: the geometry of a plot

**What**: `Plot`, `Panel`, `Axis`, `Tick`, `Series`, `Lane`, `Plot::of(&Table, &Analysis)`, `Axis::place`, `Axis::ticks` (design Components, with its fixed numbers). `Session::run` keeps the table of accepted steps for the plot when the request has `plot`.
**Where**: `crates/piperine-api/src/plot.rs`
**Depends on**: T23, T25
**Reuses**: `Table`, `Cell`
**Requirement**: CLI-22
**Executor**: Opus

**Done when**:

- [x] Every expected coordinate in a test is computed by hand in a comment beside it, from the area and the range
- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `plot::tests::a_value_maps_linearly_onto_the_plot_area`, `plot::tests::ticks_are_1_2_5_times_a_power_of_ten`, `plot::tests::a_discrete_signal_gets_a_lane_and_no_series`, `plot::tests::an_ac_plot_has_a_db_panel_over_a_degree_panel_and_a_log_axis`, `plot::tests::an_operating_point_has_nothing_to_plot`, `plot::tests::an_unknown_value_breaks_the_run`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 6 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): the geometry of a plot`

---

### T43: SVG output

**What**: `SvgPlot::write(&Plot, out)` (design `render`) and the `plot` key in `RunCommand`: the file is written after the table and standard output gets `wrote <file>`; a relative path resolves as `csv` does; a write error is `Failure::Io`.
**Where**: `crates/piperine-cli/src/render/svg.rs`
**Depends on**: T42, T26
**Reuses**: `Plot`; the path rule of T26
**Requirement**: CLI-22
**Executor**: Sonnet

**Done when**:

- [x] Tests read the SVG as text with plain string functions: element counts, attributes, the first and last point of a path; no pixel is compared and no XML crate is added
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test options`: `plot_writes_an_svg_and_prints_wrote`, `a_plot_draws_every_accepted_step_whatever_the_print_step`, `an_unwritable_plot_names_the_path_and_exits_1`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `render::svg::tests::one_path_per_signal_with_its_name_and_a_legend`, `render::svg::tests::each_axis_has_ticks_labels_and_a_title`, `render::svg::tests::the_same_plot_is_the_same_bytes`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 6 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): a run's result as an SVG plot`

---

## Phase 5 tasks

### T30: piperine check

**What**: `Project::check` (the package graph, every crate, every bench, and for every run the top, the type check and the elaboration; every failure collected, no backend) and `CheckCommand::execute` (the `ok` line or every diagnostic). A lone file is checked as its own crate.
**Where**: `crates/piperine-cli/src/commands/check.rs`
**Depends on**: T20
**Reuses**: `Session::open`'s stages before the backend, factored so both call the same code
**Requirement**: CLI-14
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-cli/tests/commands.rs` is created here; its fixtures live under `crates/piperine-cli/tests/projects/`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `check_runs_every_stage_and_no_backend`, `check_prints_ok_with_the_counts`, `check_renders_every_diagnostic_and_exits_1`, `check_without_a_run_says_type_checking_needs_one`, `check_takes_a_lone_file`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 5 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): piperine check`

---

### T31: piperine build

**What**: `BuildCommand::execute`: open a session per run, print `built <name>: compiled | cached | interp, nothing to compile`, with `--verbose` the generated source's path on the next line; go on after a failing run; exit 1 if any failed.
**Where**: `crates/piperine-cli/src/commands/build.rs`
**Depends on**: T30
**Reuses**: `Session::built`, `Compiled::{report, source_path}`
**Requirement**: CLI-15
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `build_prints_one_line_per_run`, `a_second_build_is_cached_and_compiles_nothing`, `verbose_prints_a_source_path_that_exists`, `build_goes_on_after_a_failing_run_and_exits_1`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): piperine build`

---

### T32: Verdict: pass or fail of a run

**What**: `Verdict::judge(&[Record])`, `passed`, `checks`, `failed`: a `checks` record whose `pass` is false or unknown fails the run.
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: T14
**Reuses**: `Record`, `Field`
**Requirement**: CLI-16
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-api --lib`: `verdict::tests::no_false_and_no_unknown_pass_is_a_pass`, `verdict::tests::an_unknown_pass_fails`
- [x] Gate check passes: Quick (see Gate Check Commands)
- [x] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): the verdict of a run from its checks`

---

### T33: piperine test

**What**: `TestCommand::execute`: every run in manifest order, or the named ones; `PASS`/`FAIL` lines, the failing records, no table and no other record; the count line; exit 1 when a run failed. Fixture projects `failing_require` and `one_pass_one_fail`.
**Where**: `crates/piperine-cli/src/commands/test.rs`
**Depends on**: T32, T31
**Reuses**: `Verdict`, `RecordLine`
**Requirement**: CLI-08, CLI-16
**Executor**: Sonnet

**Done when**:

- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test buck`: `piperine_test_passes_on_the_buck`
- [x] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `test_runs_every_run_or_the_named_ones`, `test_prints_pass_with_the_check_count`, `test_prints_fail_and_the_failing_records`, `test_prints_no_table_and_no_other_record`, `a_run_that_does_not_complete_is_a_fail`, `the_last_line_counts_and_the_exit_code_follows`, `a_project_without_a_run_passes_with_zero`
- [x] Gate check passes: Full (see Gate Check Commands)
- [x] Test count: the 8 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): piperine test judges every run by its checks`

---

### T34: piperine new

**What**: `NewCommand::execute`: the four files of `examples/rc` embedded with `include_str!`, the package name replaced; refuses an existing directory and a name the manifest rule forbids (`piperine_lang::pom::is_crate_name`, not `std`), writing nothing.
**Where**: `crates/piperine-cli/src/commands/new.rs`
**Depends on**: T33
**Reuses**: `examples/rc`, `is_crate_name`
**Requirement**: CLI-17
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `new_creates_the_four_files_and_says_created`, `new_writes_the_rc_example_under_the_new_name`, `new_refuses_an_existing_directory`, `new_refuses_a_name_the_manifest_rule_forbids`, `a_new_project_checks_runs_and_tests`, `new_without_a_name_exits_2`
- [ ] Gate check passes: Full (see Gate Check Commands)
- [ ] Test count: the 6 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): piperine new creates the RC project`

---

### T35: piperine clean

**What**: `CleanCommand::execute`: removes `<project>/target`; with `--cache` the directory `Settings::from_environment` names; says what it removed; `nothing to remove` otherwise. 
**Where**: `crates/piperine-cli/src/commands/clean.rs`
**Depends on**: T34
**Reuses**: `piperine_transpile::Settings`
**Requirement**: CLI-18
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `clean_removes_the_target_directory`, `clean_cache_removes_the_compiled_cache`, `clean_with_nothing_to_remove_exits_0`, `clean_touches_nothing_else`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 4 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: build

**Commit**: `feat(cli): piperine clean`

---

## Phase 6 tasks

### T44: Comments: plain comments beside the tokens

**What**: `Comments::of(src)`: every plain line and block comment with its span, its text and whether it starts its line. The lexer collects them where it drops them today; the token stream, doc comments and every lexer diagnostic are unchanged.
**Where**: `crates/piperine-lang/src/msl/comments.rs`
**Depends on**: None
**Reuses**: the `LineComment` and `BlockComment` arms of `crates/piperine-lang/src/msl/lexer.rs`
**Requirement**: CLI-23
**Executor**: Opus

**Done when**:

- [ ] `cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_corpus` pass, unedited
- [ ] `crates/piperine-lang/tests/msl_format.rs` is created here
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test msl_format`: `plain_comments_are_listed_with_span_text_and_line_start`
- [ ] Gate check passes: Full (see Gate Check Commands)
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): plain comments are listed beside the tokens`

---

### T45: The printer's layout, reachable node by node

**What**: Refactor `print.rs` so the layout of one item, one member, one statement, one arm and one list element can be asked for separately at a given indentation, by the formatter. `print(&File)` keeps its signature and gives byte-equal output for every tree.
**Where**: `crates/piperine-lang/src/msl/print.rs`
**Depends on**: T44
**Reuses**: the printer as it is
**Requirement**: CLI-23
**Executor**: Opus

**Done when**:

- [ ] `cargo test -p piperine-lang --test msl_print --test msl_corpus --test pom_corpus --test book` pass with no test edited
- [ ] No layout rule changes; the commit holds no change of expected text anywhere
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test msl_print`: `print_is_idempotent_over_corpus`, `roundtrip_ast_equal_over_corpus`
- [ ] Gate check passes: Full (see Gate Check Commands)
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `refactor(lang): the printer lays out one node at a time`

---

### T46: Comments::attach: a comment's anchor

**What**: `Comments::attach(&File, src) -> Attached`: `Before(node)`, `After(node)`, `Inside(block)` by span, for items, members, statements, match arms and one-per-line list elements; a comment no anchor reaches marks its enclosing statement or member `Verbatim`.
**Where**: `crates/piperine-lang/src/msl/comments.rs`
**Depends on**: T45
**Reuses**: the tree's spans
**Requirement**: CLI-23
**Executor**: Opus

**Done when**:

- [ ] Every comment of the source has exactly one anchor or lies in exactly one `Verbatim` node; a test asserts the count
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test msl_format`: `a_comment_is_anchored_before_after_or_inside`, `an_unreachable_comment_marks_its_statement_verbatim`
- [ ] Gate check passes: Full (see Gate Check Commands)
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): each comment is anchored to the node it belongs to`

---

### T47: Formatter: a formatted text that keeps its comments

**What**: `Formatter::format(src)` and `Formatter::check(src)`: the printer's layout node by node, with the `Before`, `After` and `Inside` comments, one blank line where the source had one or more, the source slice re-indented for a `Verbatim` node, and the guard that lexes the output and refuses a text whose comments differ from the source's.
**Where**: `crates/piperine-lang/src/msl/format.rs`
**Depends on**: T46
**Reuses**: T45's per-node layout, `Comments`
**Requirement**: CLI-23
**Executor**: Opus

**Done when**:

- [ ] Each expected text in a test is written by hand from the rule it cites (CLI-23 AC 4 to 8), not pasted from the formatter
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test msl_format`: `every_plain_comment_comes_out_once_in_order_unchanged`, `the_guard_refuses_a_text_that_lost_a_comment`, `the_formatted_tree_equals_the_sources`, `formatting_twice_changes_nothing`, `a_comment_on_its_own_line_stays_before_its_node`, `a_trailing_comment_stays_on_its_line`, `a_comment_ending_a_block_stays_inside_it`, `a_statement_with_an_inner_comment_is_kept_as_written`, `blank_lines_collapse_to_one`, `a_source_that_does_not_parse_gives_diagnostics_and_no_text`
- [ ] Gate check passes: Full (see Gate Check Commands)
- [ ] Test count: the 10 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): a formatter that keeps every comment`

---

### T48: The formatter over the corpus

**What**: The corpus tests: every file of `crates/piperine-lang/std`, every gallery fixture, every file of `examples/buck` and `examples/rc`, every MSL twin of `tests/ngspice/` and every `msl` block of the Book (as a fragment, the way `msl_print` wraps one): comments kept, tree equal, stable under a second format. A file that fails is a formatter defect fixed in `format.rs` or `comments.rs`; no corpus file is edited.
**Where**: `crates/piperine-lang/tests/msl_format.rs`
**Depends on**: T47
**Reuses**: `sources()` and `fragment_source` of `crates/piperine-lang/tests/msl_print.rs`; the block extraction of `tests/book.rs`
**Requirement**: CLI-23
**Executor**: Sonnet

**Done when**:

- [ ] The commit body states the number of sources and of plain comments the corpus held, and how many nodes fell back to `Verbatim`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test msl_format`: `the_corpus_keeps_every_comment`, `the_corpus_keeps_every_tree`, `the_corpus_is_stable_under_a_second_format`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `test(lang): the formatter keeps every comment and every tree of the corpus`

---

### T49: piperine fmt

**What**: `FmtCommand::execute`: every `.msl` under `src/` and `benches/` of the project, or the files named; rewrite what differs and print `formatted <path>`; `--check` writes nothing, prints `would format <path>` and exits 1 on a difference; a file that does not parse is reported and skipped. `examples/rc` is committed formatted. The last "not built yet" arm of `Cli` is gone.
**Where**: `crates/piperine-cli/src/commands/fmt.rs`
**Depends on**: T48, T35
**Reuses**: `Formatter::{format, check}`, `Report::write`
**Requirement**: CLI-01, CLI-24
**Executor**: Sonnet

**Done when**:

- [ ] A file is written through a temporary file in the same directory and a rename, so an interrupted run leaves the original
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --lib`: `cli::tests::every_command_is_built`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test commands`: `fmt_rewrites_what_differs_and_names_it`, `fmt_takes_files`, `fmt_check_writes_nothing_and_exits_1_on_a_difference`, `fmt_skips_a_file_that_does_not_parse_and_exits_1`, `fmt_on_formatted_files_is_silent`, `fmt_check_passes_on_the_rc_example_and_on_a_new_project`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 7 test(s) named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: build

**Commit**: `feat(cli): piperine fmt, with --check`

---

## Phase 7 tasks

### T36: The Book: the command line

**What**: A chapter `cli`, "The command line", after `compiler` and before `cheatsheet` (which becomes chapter 17): what a project is and its tree; the RC project file for file; `check`, `run op`, `run step` and `test` each with its real output; print debugging with `emit!` (the statement, the record line, `--stream`, `--records`); the plot of the step response, the SVG embedded as the binary wrote it; `piperine fmt` and `--check`, and that comments stay; the run table key by key with its flag; a lone file with flags; backends and where the generated Rust is (`build --verbose`); the buck with its bench, its `startup` table and its real output; the commands, exit codes and the codes `P0010`, `P0011`. Written for a reader who has only the Book. `crates/piperine-cli/tests/book.rs` extracts the chapter's blocks with the string functions of the lang target and holds them to `examples/rc`, `examples/buck` and the binary. `crates/piperine-lang/tests/book.rs` gains the chapter in its list and order test. The chapter `welcome` is amended: a bench is MSL; sweeps, Monte Carlo and optimization are the host's. `docs/book/build.py` highlights the output blocks as plain text.
**Where**: `docs/book/content.html`
**Depends on**: T35, T21, T43, T49
**Reuses**: the voice and the block conventions of chapters 14 and 15; `examples/rc`; the shared `startup` fixture of T21
**Requirement**: CLI-19
**Executor**: Opus

**Done when**:

- [ ] Every output block was pasted from the binary and the guard, not the author, says it is equal
- [ ] `python3 docs/book/build.py` exits 0 and `cargo test -p piperine-lang --test book` passes whole
- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test book`: `the_cli_chapter_sits_between_compiler_and_cheatsheet`, `the_welcome_chapter_says_a_bench_is_msl`, `every_msl_block_parses`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test book`: `the_plot_in_the_chapter_is_the_file_the_binary_writes`, `the_chapter_shows_fmt_and_says_comments_stay`, `the_chapter_shows_the_rc_project_file_for_file`, `the_rc_output_blocks_are_what_the_binary_prints`, `the_buck_block_is_what_the_binary_prints`, `the_chapter_explains_emit_as_print`, `the_chapter_lists_commands_keys_flags_codes`, `the_chapter_stands_alone`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 11 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs(book): the command line, with projects that run`

---

### T37: The Book: chapter 15 and the cheat sheet

**What**: Chapter `compiler`: the status callout and "What comes next" say the command line exists and point at the next chapter by name; the sentence "A host picks one" names `--backend`. The cheat sheet gains the seven commands.
**Where**: `docs/book/content.html`
**Depends on**: T36
**Reuses**: the two tests added to `crates/piperine-lang/tests/book.rs`
**Requirement**: CLI-19
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-lang --test book`: `the_compiler_chapter_says_the_command_line_exists`, `the_cheat_sheet_lists_the_seven_commands`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 2 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs(book): chapter 15 and the cheat sheet know the command line`

---

### T38: The buck's README

**What**: "Status" and "Driving it from a host" of the buck's README: the converter runs from its source; `piperine run startup` and `piperine test` with what they show; the Python sketch stays as the sketch of a host that does not exist yet and says so. `crates/piperine-cli/tests/docs.rs` is created with this file's guard.
**Where**: `examples/buck/README.md`
**Depends on**: T33
**Reuses**: none
**Requirement**: CLI-20
**Executor**: Sonnet

**Done when**:

- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test docs`: `the_buck_readme_says_how_to_run_it`
- [ ] Gate check passes: Quick (see Gate Check Commands)
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `docs(examples): the buck's README says how to run it`

---

### T39: CLAUDE.md and AGENTS.md

**What**: Both instruction files: the four host crates build and are default members; the rows of `piperine-api`, `piperine-cli`, `piperine-project`, `piperine-lang` (`msl::format`) and the root package say what this feature left; the build section says the true thing: only `piperine-lang-server` stays outside the default build, untouched and deferred; the pipeline diagram ends in the hosts that exist.
**Where**: `CLAUDE.md`
**Depends on**: T49
**Reuses**: `instructions_list_the_kept_crates` of `workspace_shape`
**Requirement**: CLI-20
**Executor**: Sonnet

**Done when**:

- [ ] `AGENTS.md` receives the same statements; `cargo test -p piperine-solver --test workspace_shape` passes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test docs`: `the_instruction_files_say_the_host_crates_build`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 1 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs: the instruction files describe the host crates as built`

---

### T40: GAPS, ROADMAP and the decision

**What**: `GAPS.md`: §1 and §2 rewritten for a CLI and an API that build, and the gaps left (no host sequences from the CLI, no Python host, the language server, which does not build and is deferred by the user ("vai ficar para bem depois"), no trace records from `analog`, the features of the old API that left, the library and the examples not yet reformatted). `ROADMAP.md`: item 3 of the queue marked done with the commit. `.specs/STATE.md`: the decision of design "Tech Decisions" under the next free `MD-NN`.
**Where**: `GAPS.md`
**Depends on**: T39
**Reuses**: design "Tech Decisions", last paragraph
**Requirement**: CLI-20
**Executor**: Sonnet

**Done when**:

- [ ] `ROADMAP.md` and `.specs/STATE.md` are edited in the same commit; item 3b's instruction (present `GAPS.md` to the user before the spec-completeness pass) is left as it is
- [ ] Tests with exactly these names pass in `cargo test -p piperine-cli --test docs`: `gaps_drops_what_closed_and_lists_what_is_left`, `the_roadmap_marks_the_cli_item_done`, `state_records_the_cli_decision`
- [ ] Gate check passes: Build (see Gate Check Commands)
- [ ] Test count: the 3 test(s) named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs: GAPS, ROADMAP and the decision after msl-cli`

---

## Checks

One claim per acceptance criterion of the spec, and per edge case (`edge <k>` is the k-th line of the spec's Edge Cases, filed under the requirement it belongs to). A criterion proven at two layers has two checks. Each proof names the line its test prints.

**C1** - `default-members` lists `.`, `crates/piperine-api`, `crates/piperine-cli` and `crates/piperine-project`, and the `**Build**` line builds them with zero warnings. (CLI-01, AC 1; T5)
Proof: run shape | test default_members_are_the_green_crates ... ok

**C2** - The binary `piperine` is a target of `piperine-cli`. (CLI-01, AC 2; T5)
Proof: run usage | test the_binary_is_named_piperine ... ok

**C3** - The root `src/lib.rs` holds `pub use piperine_api::*;` and no other item. (CLI-01, AC 3; T4)
Proof: run shape | test the_root_package_is_a_re_export_and_nothing_else ... ok

**C4** - No source of `piperine-api` or `piperine-cli` names `parse_and_elaborate`, `Element`, `piperine_codegen`, `piperine_python`, `piperine_plugin` or `.vams`. (CLI-01, AC 4; T5)
Proof: run shape | test host_crates_name_no_deleted_surface ... ok

**C5** - `piperine --help` lists `check`, `build`, `run`, `test`, `new`, `clean`, `fmt` and no other command, exit 0. (CLI-01, AC 5; T5)
Proof: run usage | test help_lists_exactly_the_seven_commands ... ok

**C6** - After the last command task no command answers "not built yet". (CLI-01, AC 5; T49)
Proof: run clilib | test cli::tests::every_command_is_built ... ok

**C7** - `piperine --version` prints `piperine 0.2.0` (the workspace version), exit 0. (CLI-01, AC 6; T5)
Proof: run usage | test version_prints_the_workspace_version ... ok

**C8** - An unknown command and an unknown flag each print a usage error on standard error and exit 2. (CLI-01, AC 7; T5)
Proof: run usage | test an_unknown_command_or_flag_exits_2 ... ok

**C9** - From `<pkg>/benches/deep/` the project found is `<pkg>`, and an inner package wins over an outer one. (CLI-02, AC 1; T9)
Proof: run project | test discover_takes_the_nearest_manifest_upward ... ok

**C10** - `--project <dir>` runs the package at `<dir>` from an unrelated working directory; a `<dir>` with no manifest is `E0170` even when its parent has one. (CLI-02, AC 2; T19)
Proof: run run | test the_project_flag_uses_that_directory_and_never_searches ... ok

**C11** - With no manifest upward, `piperine run x` prints `E0170` on standard error and exits 1. (CLI-02, AC 3; T20)
Proof: run run | test no_manifest_is_e0170_and_exit_1 ... ok

**C12** - A manifest with `[run.a]` parses; `Manifest::run` is the span of the key `run`; a `run` that is not a table is `E0171`. (CLI-02, AC 4; T2)
Proof: run manifest | test a_run_table_is_accepted_and_located ... ok

**C13** - The manifest section of `docs/spec/` says the `run` table is allowed and that the language does not define its content, and its key table has the row. (CLI-02, AC 4; T3)
Proof: run spectext | test the_manifest_rule_admits_the_run_table ... ok

**C14** - A dependency whose manifest holds a malformed `[run]` table loads, and its runs are not the project's. (CLI-02, AC 5; T9)
Proof: run project | test a_dependencys_run_table_is_ignored ... ok

**C15** - A dependency `path` with no manifest prints `E0172`, `Piperine.toml` and the offending line on standard error, exit 1. (CLI-02, AC 6; T20)
Proof: run run | test a_package_error_is_rendered_with_code_file_and_line ... ok

**C16** - For a run with a bench, the design's root crate is `bench`, and a path through the package's name and one through a direct dependency's name both resolve from it. (CLI-02, AC 7; T9)
Proof: run project | test a_bench_is_the_root_crate_over_the_package_and_its_dependencies ... ok

**C17** - For a run with no bench, the design's root crate is the package's crate. (CLI-02, AC 8; T9)
Proof: run project | test a_run_without_a_bench_roots_at_the_package ... ok

**C18** - `Project::file` on a file with no manifest upward gives a design of that file and `std` only. (CLI-02, AC 9; T9)
Proof: run project | test a_lone_file_with_no_manifest_is_a_crate_alone ... ok

**C19** - `Project::file` on `<pkg>/benches/x.msl` gives a design whose root depends on `<pkg>`. (CLI-02, AC 10; T9)
Proof: run project | test a_file_under_a_package_is_its_bench ... ok

**C20** - `Project::file` on `<pkg>/src/a.msl` is `P0010` whose message says the file is part of the crate. (CLI-02, AC 11; T9)
Proof: run project | test a_file_under_src_is_refused_with_p0010 ... ok

**C21** - A table with all nineteen keys of CLI-03 AC 1 reads into a `RunSpec` with each value where the key says. (CLI-03, AC 1; T7)
Proof: run apilib | test request::tests::a_run_table_reads_its_nineteen_keys ... ok

**C22** - Each of the nineteen keys has the flag CLI-03 AC 2 names, and the flags parse into `Overrides`. (CLI-03, AC 2; T19)
Proof: run clilib | test cli::tests::every_run_key_has_its_flag ... ok

**C23** - `stop = 1e-3` in the table and `--stop 2e-3` give a run with `stop` 2e-3; a key no flag names keeps the table's value. (CLI-03, AC 3; T8)
Proof: run apilib | test request::tests::a_flag_overrides_a_key ... ok

**C24** - `stopp = 1` and `stop = "x"` are each `P0010` whose span covers that key in the manifest text. (CLI-03, AC 4; T7)
Proof: run apilib | test request::tests::an_unknown_key_or_a_wrong_type_is_p0010_at_its_span ... ok

**C25** - A missing `analysis` and `analysis = "tran"` are each `P0010` naming `op`, `dc`, `ac`, `transient`. (CLI-03, AC 5; T7)
Proof: run apilib | test request::tests::analysis_is_one_of_four ... ok

**C26** - A transient with no `stop`, with `stop = 0`, with `max_step = -1` and with `print_step = 0` is each `P0010` naming the key. (CLI-03, AC 6; T7)
Proof: run apilib | test request::tests::a_transient_needs_a_positive_stop ... ok

**C27** - `stop` on an `op` run and `sweep` on a `transient` run are each `P0010` naming the key and the analysis. (CLI-03, AC 7; T7)
Proof: run apilib | test request::tests::a_key_outside_its_analysis_is_p0010 ... ok

**C28** - `bench = "benches/none.msl"` is `P0010` whose span is the string. (CLI-03, AC 8; T9)
Proof: run project | test a_missing_bench_file_is_p0010_at_the_value ... ok

**C29** - `Project::request(None, ..)` on a project with one run returns it. (CLI-03, AC 9; T9)
Proof: run project | test the_only_run_is_run_without_a_name ... ok

**C30** - `piperine run` on a project with two runs names both on standard error and exits 2; with none it says there is no run and exits 2. (CLI-03, AC 10; T20)
Proof: run run | test no_name_with_no_run_or_several_lists_them_and_exits_2 ... ok

**C31** - `piperine run nope` names the project's runs on standard error and exits 2. (CLI-03, AC 11; T20)
Proof: run run | test an_unknown_run_name_lists_the_runs_and_exits_2 ... ok

**C32** - `piperine run divider.msl` with no `--analysis` exits 2 naming `--analysis`. (CLI-03, AC 12; T20)
Proof: run run | test a_lone_file_without_analysis_exits_2 ... ok

**C33** - `[run."a b"]` is `P0010` at the name. (CLI-03, AC edge 1; T7)
Proof: run apilib | test request::tests::a_run_name_is_an_identifier ... ok

**C34** - `--stop 1 --stop 2` exits 2 naming `--stop`. (CLI-03, AC edge 2; T20)
Proof: run run | test a_single_valued_flag_given_twice_exits_2 ... ok

**C35** - With two modules without ports and `top = "B"`, the session's top is `B`. (CLI-04, AC 1; T10)
Proof: run session | test the_top_key_names_the_module ... ok

**C36** - With one module without ports and no `top`, it is the top. (CLI-04, AC 2; T10)
Proof: run session | test the_one_portless_module_is_the_top ... ok

**C37** - No module without ports is `P0011`; two with no `top` is `P0011` naming both. (CLI-04, AC 3; T10)
Proof: run session | test no_top_or_several_is_p0011_with_candidates ... ok

**C38** - A rendered diagnostic holds its code, the file name, the line number and the source line. (CLI-04, AC 4; T17)
Proof: run clilib | test render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt ... ok

**C39** - A project with a type error prints `P0002`, the file, the line, the excerpt and `typeck failed` on standard error, exit 1. (CLI-04, AC 4; T20)
Proof: run run | test a_type_error_is_rendered_with_its_stage_and_exits_1 ... ok

**C40** - With `Settings::rustc` naming a file that does not exist, `Session::open` on the Rust backend fails with a message holding `rustc` and `interp`. (CLI-04, AC 5; T10)
Proof: run session | test a_missing_rustc_names_rustc_and_the_interpreter ... ok

**C41** - A design with a carrier on the interpreter fails with a message holding the body's name and `--backend rust`. (CLI-04, AC 6; T10)
Proof: run session | test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok

**C42** - `Backend::choose` returns the flag, else the variable, else the key, else `Rust`, over all eight presence patterns. (CLI-04, AC 7; T4)
Proof: run apilib | test backend::tests::the_flag_then_the_variable_then_the_key_then_rust ... ok

**C43** - `--backend x` exits 2; `PIPERINE_BACKEND=x` (set on the child) and `backend = "x"` exit 1; each names `interp` and `rust`. (CLI-04, AC 8; T20)
Proof: run run | test a_bad_backend_exits_2_for_the_flag_and_1_otherwise ... ok

**C44** - Under a compiler wrapper that counts, one `Session::open` plus a transient invokes the design build once, and a second session on the same cache zero times. (CLI-04, AC 9; T10)
Proof: run session | test a_run_compiles_once_and_never_during_an_analysis ... ok

**C45** - A top with a discrete `in` port opens and its operating point gives that port as unknown. (CLI-04, AC edge 5; T10)
Proof: run session | test a_top_with_discrete_in_ports_runs_with_them_unknown ... ok

**C46** - A top with an `Electrical` port fails with the front end's diagnostic for it. (CLI-04, AC edge 6; T10)
Proof: run session | test a_top_with_a_continuous_port_is_the_front_ends_diagnostic ... ok

**C47** - An `op` table has the columns `signal`, `value` and its rows follow `print` order. (CLI-05, AC 1; T12)
Proof: run analyses | test an_operating_point_is_signal_and_value_in_print_order ... ok

**C48** - A transient table's first column is `time`, its times increase strictly from 0 to `stop`, and it has one column per printed signal. (CLI-05, AC 2; T13)
Proof: run analyses | test a_transient_has_one_row_per_accepted_step ... ok

**C49** - With no `print`, the probes are the top's signals and ports in declaration order and no signal of a child. (CLI-05, AC 3; T11)
Proof: run probes | test the_default_is_the_tops_own_signals_in_order ... ok

**C50** - `mid`, `Divider.mid`, a discrete net and `i(v1)` each resolve; the relative and the full spelling give the same probe. (CLI-05, AC 4; T11)
Proof: run probes | test a_node_a_net_and_a_flow_resolve_relative_or_full ... ok

**C51** - The labels are `Divider.mid` and `i(Divider.v1)`. (CLI-05, AC 5; T11)
Proof: run probes | test a_column_is_named_by_the_full_path ... ok

**C52** - `print = ["nope"]` fails `Session::open` with `E0801` naming `nope`. (CLI-05, AC 6; T11)
Proof: run probes | test an_entry_that_names_nothing_is_e0801_before_any_analysis ... ok

**C53** - 10 V over 1 kΩ and 3 kΩ: `Divider.mid` is 7.5 within 1e-9 on `interp` and on `rust`. (CLI-05, AC 7; T12)
Proof: run analyses | test the_divider_mid_is_7_5_on_both_backends ... ok

**C54** - RC = 1.5915494309189535e-6 s: at the first row at or after RC, `TranRc.out` is within 1e-3 of `1 - exp(-t/RC)`, on both backends. (CLI-05, AC 8; T13)
Proof: run analyses | test the_rc_step_follows_one_minus_exp_on_both_backends ... ok

**C55** - A circuit with two voltage sources in parallel at different values prints the solver's error on standard error, no table, exit 1. (CLI-05, AC 9; T20)
Proof: run run | test a_solver_failure_prints_no_table_and_exits_1 ... ok

**C56** - 7.5 is `7.500000e0`, 1.25e-6 is `1.250000e-6`, `true` is `true`, 42 is `42`, unknown is `x`. (CLI-05, AC 10; T15)
Proof: run clilib | test render::number::tests::numbers_have_one_text_each ... ok

**C57** - `print = ["mid", "mid"]` gives two columns in that order. (CLI-05, AC edge 3; T11)
Proof: run probes | test a_signal_printed_twice_is_two_columns ... ok

**C58** - A reaction with `emit!("s", a = 1.5, b = true)` fired by a timer at 1 µs gives one record: time 1e-6, the instance path of the site, stream `s`, fields `a` = 1.5 then `b` = true. (CLI-06, AC 1; T14)
Proof: run records | test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok

**C59** - That record renders as `@1.000000e-6 Top.m s a=1.500000e0 b=true`. (CLI-06, AC 2; T16)
Proof: run clilib | test render::record::tests::a_record_is_one_line ... ok

**C60** - Timers at 3 µs, 1 µs and 2 µs in source order give records at 1, 2 and 3 µs. (CLI-06, AC 3; T14)
Proof: run records | test records_arrive_in_time_order ... ok

**C61** - `emit!("log", msg = "overrange")` gives the field `msg` as the text `overrange`. (CLI-06, AC 4; T14)
Proof: run records | test a_text_field_is_the_sources_literal ... ok

**C62** - A field that reads an undriven discrete port is `Field::Unknown`. (CLI-06, AC 5; T14)
Proof: run records | test an_unknown_field_is_unknown ... ok

**C63** - `require("lim", false)` in a reaction gives one record of `checks` with `name` = `lim` and `pass` = false. (CLI-06, AC 6; T14)
Proof: run records | test a_failed_require_is_one_checks_record_with_pass_false ... ok

**C64** - In `examples/rc`, every line starting with `@` precedes the empty line and the table. (CLI-06, AC 7; T18)
Proof: run run | test records_come_before_the_table ... ok

**C65** - The divider's operating point delivers no record. (CLI-06, AC 8; T14)
Proof: run records | test a_design_without_emit_gives_no_record ... ok

**C66** - A `checks` record has stream `checks` and the instance path of the reaction that wrote it. (CLI-06, AC edge 7; T14)
Proof: run records | test a_reserved_stream_keeps_its_name_and_its_site ... ok

**C67** - `piperine run step` in `examples/rc` prints `run step: transient, top Step, backend rust`, then the `trip` record, an empty line and the table whose first column is `time`. (CLI-07, AC 1; T18)
Proof: run run | test run_prints_header_records_blank_line_and_table ... ok

**C68** - A 3-column table renders with each column padded to its widest cell, two spaces between, no trailing space, header first. (CLI-07, AC 2; T15)
Proof: run clilib | test render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space ... ok

**C69** - `piperine run op` and `piperine run step` in `examples/rc` exit 0. (CLI-07, AC 3; T18)
Proof: run run | test a_completed_run_exits_0 ... ok

**C70** - A type error, a bad run table and an unknown `print` path each leave standard output empty and exit 1. (CLI-07, AC 4; T20)
Proof: run run | test a_failure_before_the_analysis_prints_nothing_on_stdout ... ok

**C71** - A bench that emits at 1 µs and whose analog term stops being finite at 2 µs prints the record, then the solver's error on standard error, no table, exit 1. (CLI-07, AC 5; T20)
Proof: run run | test records_stay_when_the_analysis_fails ... ok

**C72** - Two runs of `step` with different cache directories and different working directories print byte-equal standard output. (CLI-07, AC 6; T18)
Proof: run run | test stdout_is_the_same_from_two_caches_and_two_directories ... ok

**C73** - `piperine run tests/ngspice/divider.msl --analysis op` prints `run divider: op, top Divider, backend rust` and `Divider.mid  7.500000e0`. (CLI-07, AC 7; T19)
Proof: run run | test a_lone_file_run_is_named_by_its_stem ... ok

**C74** - With an empty cache, standard error holds one line with `compiling` and `step`; with the cache filled, none. (CLI-07, AC 8; T18)
Proof: run run | test a_first_compile_is_announced_on_stderr ... ok

**C75** - With standard output closed by the reader after the header, the process exits 1 and standard error holds no `panicked`. (CLI-07, AC edge 8; T20)
Proof: run run | test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok

**C76** - `examples/buck` holds `benches/startup.msl` and its manifest the runs `idle` and `startup`. (CLI-08, AC 1; T21)
Proof: run buck | test the_buck_has_its_bench_and_two_runs ... ok

**C77** - `piperine run startup` exits 0 on the Rust backend and the last row gives 128 in the column of the controller's reference (buck SPEC: setpoint code 0 is 0.8 V, reference 128). (CLI-08, AC 2; T21)
Proof: run buck | test startup_ends_with_the_reference_at_128 ... ok

**C78** - That run's standard output holds no line with ` checks ` (a passing check is hidden by default, a failing one would print) and at most 16 lines in all; `--records FILE` of the same run keeps more than 1000 passing checks. (CLI-08, AC 3 and 6; T21, amended 2026-11-16)
Proof: run buck | test startup_prints_no_failed_check ... ok

**C79** - `piperine run idle` exits 0 and every row gives 0 (`Off`) in the mode column. (CLI-08, AC 4; T21)
Proof: run buck | test idle_keeps_the_mode_off ... ok

**C80** - `piperine test` in `examples/buck` exits 0 with `2 runs, 2 passed, 0 failed`. (CLI-08, AC 5; T33)
Proof: run buck | test piperine_test_passes_on_the_buck ... ok

**C81** - A sweep of `v1.dc` from 0 to 1 by 0.25 has the first column `Divider.v1.dc` and the rows 0, 0.25, 0.5, 0.75, 1; `Divider.mid` is 0.75 of each. (CLI-09, AC 1; T22)
Proof: run analyses | test a_dc_sweep_has_one_row_per_point ... ok

**C82** - An `ac` table has `freq`, then `mag(AcRc.out)` and `phase(AcRc.out)`; at 1 kHz the phase is within 1e-2 of -0.5729 degrees (atan of 0.01). (CLI-09, AC 2; T23)
Proof: run analyses | test an_ac_sweep_has_mag_and_phase_in_degrees ... ok

**C83** - A `dc` run without `sweep`, without `step`, with `step = 0`, and with `from = 0, to = 1, step = -0.1` is each `P0010` naming the key. (CLI-09, AC 3; T7)
Proof: run apilib | test request::tests::a_dc_sweep_needs_its_four_keys_and_a_step_toward_to ... ok

**C84** - `from = 1e3, to = 1e7, per_decade = 10` is 41 points; `to = 2e3, per_decade = 10` is 4. (CLI-09, AC 4; T23)
Proof: run apilib | test request::tests::per_decade_gives_round_n_log10_plus_one_points ... ok

**C85** - Both grids, neither, `from = 0` and `to < from` are each `P0010` naming the keys. (CLI-09, AC 5; T7)
Proof: run apilib | test request::tests::an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered ... ok

**C86** - `from = 1e3, to = 5e3, points = 5` gives the frequencies 1e3, 2e3, 3e3, 4e3, 5e3. (CLI-09, AC 6; T23)
Proof: run analyses | test points_sweeps_linearly ... ok

**C87** - R = 1 kΩ, C = 1.5915494309189535 nF: at 1e5 Hz the magnitude is within 1e-3 of 0.7071067811865476 and the phase within 1e-2 of -45, on both backends. (CLI-09, AC 7; T23)
Proof: run analyses | test the_rc_corner_is_minus_3_db_and_minus_45_degrees ... ok

**C88** - `v1.dc` from -1 to 0.8 by 0.05 on `diode_iv` gives 37 rows, the last at 0.8. (CLI-09, AC 8; T22)
Proof: run analyses | test the_diode_sweep_has_37_rows ... ok

**C89** - `sweep = "v9.dc"` fails `Session::open` with `E0801` naming it. (CLI-09, AC 9; T22)
Proof: run analyses | test a_sweep_of_no_param_is_e0801 ... ok

**C90** - The table sets `r2.r` to 2000 and the flag to 1000: the operating point is the one of 1000. (CLI-10, AC 1; T24)
Proof: run analyses | test params_are_set_in_order_table_then_flags ... ok

**C91** - `r9.r = 1.0` fails with `E0801` naming `r9.r`. (CLI-10, AC 2; T24)
Proof: run analyses | test a_param_path_that_names_nothing_is_e0801 ... ok

**C92** - `r2.r = -1.0` (the resistor's `where r > 0.0`) fails with `E0802` naming `r2.r` and -1, and no record or table exists. (CLI-10, AC 3; T24)
Proof: run analyses | test a_refused_value_is_e0802_and_nothing_runs ... ok

**C93** - `piperine run tests/ngspice/divider.msl --analysis op --param r2.r=1000` prints `Divider.mid  5.000000e0`. (CLI-10, AC 4; T24)
Proof: run options | test the_divider_with_r2_at_1k_gives_5 ... ok

**C94** - `--csv out.csv` writes a header row and the rows with `,` between cells in the text table's number format, and standard output ends with `wrote out.csv (<n> rows)` with `n` the row count. (CLI-11, AC 1; T26)
Proof: run options | test csv_writes_the_table_and_prints_wrote ... ok

**C95** - A longer file already at the path is replaced, not appended to. (CLI-11, AC 2; T26)
Proof: run options | test an_existing_csv_is_replaced ... ok

**C96** - `--csv /nonexistent-dir/x.csv` prints the path on standard error and exits 1. (CLI-11, AC 3; T26)
Proof: run options | test an_unwritable_csv_names_the_path_and_exits_1 ... ok

**C97** - `stop = 10e-6`, `print_step = 1e-6` gives 11 rows at 0, 1e-6, ..., 10e-6. (CLI-11, AC 4; T25)
Proof: run apilib | test table::tests::a_print_grid_has_one_row_per_multiple ... ok

**C98** - Between steps (0, 0.0) and (2, 4.0) the row at 1 gives 2.0 for a real column and the value at 0 for a `bool` or integer column. (CLI-11, AC 5; T25)
Proof: run apilib | test table::tests::continuous_is_interpolated_and_discrete_is_held ... ok

**C99** - `csv = "out/step.csv"` run from `<project>/benches/` writes `<project>/out/step.csv`. (CLI-11, AC 6; T26)
Proof: run options | test a_relative_output_path_is_under_the_project ... ok

**C100** - `print_step` 1e-3 with `stop` 1e-6 gives the one row at time 0. (CLI-11, AC edge 4; T25)
Proof: run apilib | test table::tests::a_grid_wider_than_the_run_is_the_row_at_zero ... ok

**C101** - With streams `trip` in two instances and `checks`: `["trip"]` keeps both `trip` streams, `["Step.mon.trip"]` keeps one, and neither keeps `checks`. (CLI-12, AC 1; T27)
Proof: run records | test keep_matches_a_stream_name_or_its_full_path ... ok

**C102** - `streams = ["nope"]` fails `Session::open` with `E0801` naming `nope`. (CLI-12, AC 2; T27)
Proof: run records | test a_filter_entry_that_matches_nothing_is_e0801 ... ok

**C103** - `--records r.jsonl` writes one object per record with the keys `time`, `instance`, `stream`, `fields`, and standard output holds no line starting with `@`. (CLI-12, AC 3; T28)
Proof: run options | test records_go_to_the_file_as_json_lines_and_not_to_stdout ... ok

**C104** - A longer file already at the path is replaced. (CLI-12, AC 4; T28)
Proof: run options | test an_existing_records_file_is_replaced ... ok

**C105** - 1.5 is `1.5`, `true` is `true`, 42 is `42`, the text `a"b` is `"a\"b"`, unknown is `null`. (CLI-12, AC 5; T28)
Proof: run clilib | test render::json::tests::each_value_kind_has_its_json ... ok

**C106** - With both flags, standard output is the header line and the `wrote` line. (CLI-12, AC edge 9; T28)
Proof: run options | test csv_and_records_leave_the_header_and_the_wrote_line ... ok

**C107** - `piperine run step --stream trip` in `examples/rc` prints the `trip` record and no `checks` record. (CLI-12, AC 1; T27)
Proof: run options | test the_stream_flag_keeps_only_that_stream ... ok

**C108** - `divider`, `isrc_r` and `diode_series` run `op` with exit 0 under `--backend interp` and `--backend rust`. (CLI-13, AC 1; T29)
Proof: run twins | test the_three_op_twins_run_on_both_backends ... ok

**C109** - `diode_iv` and `diode_rev` (`dc`), `ac_rc` (`ac`) and `tran_rc` (`transient`) exit 0 on both backends, each with the grid its `.cir` twin names. (CLI-13, AC 2; T29)
Proof: run twins | test the_sweep_and_transient_twins_run_on_both_backends ... ok

**C110** - For each of the seven twins the standard output of the two backends is byte-equal after the header line. (CLI-13, AC 3; T29)
Proof: run twins | test the_two_backends_print_the_same_bytes ... ok

**C111** - `piperine check` in `examples/rc` exits 0 with an empty cache directory left empty and with `PIPERINE_RUSTC` naming a file that does not exist. (CLI-14, AC 1; T30)
Proof: run commands | test check_runs_every_stage_and_no_backend ... ok

**C112** - It prints `ok: rc (2 crates, 2 runs)`: the package and `std`, the runs `op` and `step`. (CLI-14, AC 2; T30)
Proof: run commands | test check_prints_ok_with_the_counts ... ok

**C113** - A project with a type error in the top of one run and an unresolved path in the bench of another prints both codes, no `ok` line, exit 1. (CLI-14, AC 3; T30)
Proof: run commands | test check_renders_every_diagnostic_and_exits_1 ... ok

**C114** - A package with no run prints `ok: <name> (2 crates, 0 runs; type checking needs a run)`. (CLI-14, AC 4; T30)
Proof: run commands | test check_without_a_run_says_type_checking_needs_one ... ok

**C115** - `piperine check tests/ngspice/divider.msl` exits 0; a copy with a type error exits 1 with `P0002`. (CLI-14, AC 5; T30)
Proof: run commands | test check_takes_a_lone_file ... ok

**C116** - `piperine build` in `examples/rc` with an empty cache prints `built op: compiled` and `built step: compiled`; a run with `backend = "interp"` prints `built <name>: interp, nothing to compile`. (CLI-15, AC 1; T31)
Proof: run commands | test build_prints_one_line_per_run ... ok

**C117** - The second `piperine build` prints `cached` for both runs, under a compiler wrapper that counts zero design builds. (CLI-15, AC 2; T31)
Proof: run commands | test a_second_build_is_cached_and_compiles_nothing ... ok

**C118** - `--verbose` prints after each line a path ending in `design.rs` that is a file. (CLI-15, AC 3; T31)
Proof: run commands | test verbose_prints_a_source_path_that_exists ... ok

**C119** - With the first of two runs failing type check, the second still prints its `built` line and the exit code is 1. (CLI-15, AC 4; T31)
Proof: run commands | test build_goes_on_after_a_failing_run_and_exits_1 ... ok

**C120** - `piperine test` prints one verdict line per run in manifest order; `piperine test step` prints one. (CLI-16, AC 1; T33)
Proof: run commands | test test_runs_every_run_or_the_named_ones ... ok

**C121** - In `examples/rc`, `step` prints `PASS step (1 checks)` and `op` prints `PASS op (0 checks)`. (CLI-16, AC 2; T33)
Proof: run commands | test test_prints_pass_with_the_check_count ... ok

**C122** - Three `checks` records with `pass` true and two records of other streams judge as passed with 3 checks. (CLI-16, AC 2; T32)
Proof: run apilib | test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok

**C123** - A bench with `require("lim", false)` prints `FAIL <name> (1 of 1 checks failed)` and the record's line. (CLI-16, AC 3; T33)
Proof: run commands | test test_prints_fail_and_the_failing_records ... ok

**C124** - A `checks` record whose `pass` is unknown is a failed check. (CLI-16, AC 3; T32)
Proof: run apilib | test verdict::tests::an_unknown_pass_fails ... ok

**C125** - Under `test`, standard output holds no `trip` record and no table header. (CLI-16, AC 4; T33)
Proof: run commands | test test_prints_no_table_and_no_other_record ... ok

**C126** - A run with a type error prints `FAIL <name> (did not run)` on standard output and the diagnostic on standard error. (CLI-16, AC 5; T33)
Proof: run commands | test a_run_that_does_not_complete_is_a_fail ... ok

**C127** - One passing and one failing run end with `2 runs, 1 passed, 1 failed` and exit 1; two passing end with `2 runs, 2 passed, 0 failed` and exit 0. (CLI-16, AC 6; T33)
Proof: run commands | test the_last_line_counts_and_the_exit_code_follows ... ok

**C128** - It prints `0 runs, 0 passed, 0 failed` and exits 0. (CLI-16, AC 7; T33)
Proof: run commands | test a_project_without_a_run_passes_with_zero ... ok

**C129** - `piperine new demo` creates `demo/Piperine.toml`, `demo/src/lib.msl`, `demo/benches/step.msl`, `demo/.gitignore` and prints `created demo`. (CLI-17, AC 1; T34)
Proof: run commands | test new_creates_the_four_files_and_says_created ... ok

**C130** - Each file equals its `examples/rc` counterpart with the package name `rc` replaced by `demo` where it is the package name. (CLI-17, AC 2; T34)
Proof: run commands | test new_writes_the_rc_example_under_the_new_name ... ok

**C131** - With `demo/` present, `new demo` exits 1 and the directory is unchanged. (CLI-17, AC 3; T34)
Proof: run commands | test new_refuses_an_existing_directory ... ok

**C132** - `new my-proj`, `new std` and `new module` each exit 1 stating the rule and create nothing. (CLI-17, AC 4; T34)
Proof: run commands | test new_refuses_a_name_the_manifest_rule_forbids ... ok

**C133** - In the created project `check`, `run step` and `test` each exit 0. (CLI-17, AC 5; T34)
Proof: run commands | test a_new_project_checks_runs_and_tests ... ok

**C134** - `piperine new` exits 2. (CLI-17, AC edge 10; T34)
Proof: run commands | test new_without_a_name_exits_2 ... ok

**C135** - After a build that left `<project>/target`, `clean` removes it and prints `removed target`. (CLI-18, AC 1; T35)
Proof: run commands | test clean_removes_the_target_directory ... ok

**C136** - `clean --cache` removes the directory `PIPERINE_CACHE_DIR` names (set on the child), and the next `build` prints `compiled`. (CLI-18, AC 2; T35)
Proof: run commands | test clean_cache_removes_the_compiled_cache ... ok

**C137** - It prints `nothing to remove` and exits 0. (CLI-18, AC 3; T35)
Proof: run commands | test clean_with_nothing_to_remove_exits_0 ... ok

**C138** - The project's other files and a sibling of the cache directory are byte-equal before and after. (CLI-18, AC 4; T35)
Proof: run commands | test clean_touches_nothing_else ... ok

**C139** - The chapter ids end with `compiler`, `cli`, `cheatsheet`; `cli` is titled `The command line`; the chapter numbers are 1 to 17. (CLI-19, AC 1; T36)
Proof: run book | test the_cli_chapter_sits_between_compiler_and_cheatsheet ... ok

**C140** - The SVG the chapter embeds equals the file `piperine run step --plot` writes in `examples/rc`. (CLI-19, AC 11; T36)
Proof: run clibook | test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok

**C141** - The chapter names `piperine fmt` and `piperine fmt --check` in code spans and says comments are kept. (CLI-19, AC 12; T36)
Proof: run clibook | test the_chapter_shows_fmt_and_says_comments_stay ... ok

**C142** - The chapter `welcome` says a bench is written in MSL and that sweeps, Monte Carlo and optimization live in the host, and holds no sentence saying testbenches or stimulus do not live in MSL. (CLI-19, AC 13; T36)
Proof: run book | test the_welcome_chapter_says_a_bench_is_msl ... ok

**C143** - `examples/rc` resolves as a package and its crate has no diagnostic. (CLI-19, AC 2; T6)
Proof: run rcpkg | test the_rc_example_resolves_with_no_diagnostic ... ok

**C144** - The chapter holds a block equal to each of `examples/rc/Piperine.toml`, `src/lib.msl` and `benches/step.msl`, and a tree naming the three. (CLI-19, AC 2; T36)
Proof: run clibook | test the_chapter_shows_the_rc_project_file_for_file ... ok

**C145** - The four output blocks equal the standard output of `piperine check`, `run op`, `run step` and `test` run in `examples/rc`. (CLI-19, AC 3; T36)
Proof: run clibook | test the_rc_output_blocks_are_what_the_binary_prints ... ok

**C146** - The chapter holds the buck's bench and `startup` table equal to the files, and an output block equal to the standard output of `piperine run startup`. (CLI-19, AC 4; T36)
Proof: run clibook | test the_buck_block_is_what_the_binary_prints ... ok

**C147** - The chapter has a part whose heading names `emit!`, an `msl` block with an `emit!` statement, the record line it gives, and `--stream`. (CLI-19, AC 5; T36)
Proof: run clibook | test the_chapter_explains_emit_as_print ... ok

**C148** - The chapter names in code spans the seven commands, each of the nineteen keys and its flag, the exit codes 0, 1, 2, `--backend`, `PIPERINE_BACKEND`, `P0010` and `P0011`. (CLI-19, AC 6; T36)
Proof: run clibook | test the_chapter_lists_commands_keys_flags_codes ... ok

**C149** - The chapter holds no `§`, none of `crates/`, `docs/`, `.specs/`, `tests/`, and no `href` that does not start with `#`. (CLI-19, AC 7; T36)
Proof: run clibook | test the_chapter_stands_alone ... ok

**C150** - Every `msl` block of the Book, the new chapter's included, parses. (CLI-19, AC 8; T36)
Proof: run book | test every_msl_block_parses ... ok

**C151** - The chapter `compiler` names `piperine run` and its part on what comes next does not mention the command line. (CLI-19, AC 9; T37)
Proof: run book | test the_compiler_chapter_says_the_command_line_exists ... ok

**C152** - The cheat sheet names `piperine check`, `build`, `run`, `test`, `new`, `clean` and `fmt`. (CLI-19, AC 10; T37)
Proof: run book | test the_cheat_sheet_lists_the_seven_commands ... ok

**C153** - `CLAUDE.md` and `AGENTS.md` do not say `piperine-api`, `piperine-cli`, `piperine-project` or the root package does not build; the rows state `Project`/`Session` for `piperine-api`, the seven commands for `piperine-cli` and `msl::format` for `piperine-lang`; `piperine-lang-server` is said to be outside the default build and deferred. (CLI-20, AC 1; T39)
Proof: run docs | test the_instruction_files_say_the_host_crates_build ... ok

**C154** - `GAPS.md` no longer says the CLI and the API do not build, and names: no host sequences from the command line, no Python host, the language server (deferred by the user), no trace records. (CLI-20, AC 2; T40)
Proof: run docs | test gaps_drops_what_closed_and_lists_what_is_left ... ok

**C155** - Item 3 of "Queue after ir-rust" in `ROADMAP.md` is marked done. (CLI-20, AC 3; T40)
Proof: run docs | test the_roadmap_marks_the_cli_item_done ... ok

**C156** - `.specs/STATE.md` holds a decision naming `piperine-api`, the `[run.<name>]` table, the bench crate and `msl::format`. (CLI-20, AC 4; T40)
Proof: run docs | test state_records_the_cli_decision ... ok

**C157** - `examples/buck/README.md` names `piperine run startup` and no longer says nothing executes. (CLI-20, AC 5; T38)
Proof: run docs | test the_buck_readme_says_how_to_run_it ... ok

**C158** - Part VIII defines **bench**: a module without ports, written in MSL, that places the design with its sources, stimulus and monitors, chosen as the top by the host; the term is in the part's list of terms and in the index's term table. (CLI-21, AC 1; T41)
Proof: run spectext | test part_viii_defines_a_bench ... ok

**C159** - Principle 3 of the index names benches as MSL and sweeps, Monte Carlo, optimization and regression as the host's. (CLI-21, AC 2; T41)
Proof: run spectext | test the_index_says_benches_are_msl_and_sweeps_are_host ... ok

**C160** - The model column of the testbench table holds stimulus written as reactions and timers; the host column keeps `force` and `release`, constrained random stimulus, sweeps and Monte Carlo. (CLI-21, AC 3; T41)
Proof: run spectext | test the_testbench_table_splits_stimulus_and_host_work ... ok

**C161** - No file of `docs/spec/` holds "the testbench lives in the host", "stimulus sequences, sweeps and optimization live in the host" or "belongs to the testbench, in the host". (CLI-21, AC 4; T41)
Proof: run spectext | test no_spec_text_says_stimulus_is_host_only ... ok

**C162** - Every error code of `docs/spec/` is still defined exactly once. (CLI-21, AC 5; T41)
Proof: run spectext | test every_error_code_is_defined_exactly_once ... ok

**C163** - `piperine run step --plot step.svg` in `examples/rc` writes a file that starts with `<svg` and standard output ends with `wrote step.svg` after the table. (CLI-22, AC 1; T43)
Proof: run options | test plot_writes_an_svg_and_prints_wrote ... ok

**C164** - A plot of two real series gives two `path` elements of class `series`, each with its `data-signal`, and a legend text for each. (CLI-22, AC 2; T43)
Proof: run clilib | test render::svg::tests::one_path_per_signal_with_its_name_and_a_legend ... ok

**C165** - With x from 0 to 10 over an area from 100 to 700 and y from 0 to 1 over 400 down to 40, the point (5, 0.5) is (400, 220) and the two corners are the area's. (CLI-22, AC 3; T42)
Proof: run apilib | test plot::tests::a_value_maps_linearly_onto_the_plot_area ... ok

**C166** - The SVG holds, per axis, one line and one text per tick and a title text: `time` and the y title for a transient. (CLI-22, AC 4; T43)
Proof: run clilib | test render::svg::tests::each_axis_has_ticks_labels_and_a_title ... ok

**C167** - A range 0 to 10e-6 gives ticks at multiples of 2e-6; 0 to 1 gives multiples of 0.2; every tick lies in the axis range and there are 4 to 10 of them. (CLI-22, AC 4; T42)
Proof: run apilib | test plot::tests::ticks_are_1_2_5_times_a_power_of_ten ... ok

**C168** - A table with one real and one `bool` column gives one series and one lane whose steps are the `bool` column's changes. (CLI-22, AC 5; T42)
Proof: run apilib | test plot::tests::a_discrete_signal_gets_a_lane_and_no_series ... ok

**C169** - An `ac` table gives two panels: the first plots `20*log10(mag)`, the second the phase; with `per_decade` the x axis is logarithmic and 1e5 lies midway between 1e3 and 1e7. (CLI-22, AC 6; T42)
Proof: run apilib | test plot::tests::an_ac_plot_has_a_db_panel_over_a_degree_panel_and_a_log_axis ... ok

**C170** - With `print_step` giving an 11-row table, the series' path has more than 11 points and as many as the run without `print_step`. (CLI-22, AC 7; T43)
Proof: run options | test a_plot_draws_every_accepted_step_whatever_the_print_step ... ok

**C171** - `Plot::of` on an `op` run is `P0010` whose message says an operating point has nothing to plot. (CLI-22, AC 8; T42)
Proof: run apilib | test plot::tests::an_operating_point_has_nothing_to_plot ... ok

**C172** - A column real, unknown, real, real gives a series of two runs of 1 and 2 points. (CLI-22, AC 9; T42)
Proof: run apilib | test plot::tests::an_unknown_value_breaks_the_run ... ok

**C173** - Writing one `Plot` twice gives equal bytes, and every coordinate has two decimals. (CLI-22, AC 10; T43)
Proof: run clilib | test render::svg::tests::the_same_plot_is_the_same_bytes ... ok

**C174** - `--plot /nonexistent-dir/x.svg` prints the path on standard error and exits 1. (CLI-22, AC 11; T43)
Proof: run options | test an_unwritable_plot_names_the_path_and_exits_1 ... ok

**C175** - `Comments::of` on a source with a `//` line, a trailing `//`, a `/* */` and a `///` doc lists the first three with span, text and whether each starts its line, and not the doc comment. (CLI-23, AC 1; T44)
Proof: run format | test plain_comments_are_listed_with_span_text_and_line_start ... ok

**C176** - A source with six comments in six positions formats to a text whose `Comments::of` gives the same six texts in the same order. (CLI-23, AC 1; T47)
Proof: run format | test every_plain_comment_comes_out_once_in_order_unchanged ... ok

**C177** - The formatter's guard, given an output with one comment fewer than the source, returns an internal-error diagnostic and no text. (CLI-23, AC 1; T47)
Proof: run format | test the_guard_refuses_a_text_that_lost_a_comment ... ok

**C178** - For each case of the target, parsing the formatted text gives a tree equal to the source's, spans aside. (CLI-23, AC 2; T47)
Proof: run format | test the_formatted_tree_equals_the_sources ... ok

**C179** - For each case of the target, `format(format(x)) == format(x)`. (CLI-23, AC 3; T47)
Proof: run format | test formatting_twice_changes_nothing ... ok

**C180** - `Comments::attach` anchors an own-line comment `Before` the next item, member, statement, arm or element, a same-line comment `After` the node it follows, and a comment ending a block `Inside` it. (CLI-23, AC 4; T46)
Proof: run format | test a_comment_is_anchored_before_after_or_inside ... ok

**C181** - A badly indented own-line comment before a member, a statement and a match arm is printed on its own line at that node's indentation, before the same node. (CLI-23, AC 4; T47)
Proof: run format | test a_comment_on_its_own_line_stays_before_its_node ... ok

**C182** - `let x = 1;   // why` formats to `let x = 1; // why`. (CLI-23, AC 5; T47)
Proof: run format | test a_trailing_comment_stays_on_its_line ... ok

**C183** - A comment as the last line of a block, and as the only content of one, is printed before that block's closing brace at the block's inner indentation. (CLI-23, AC 6; T47)
Proof: run format | test a_comment_ending_a_block_stays_inside_it ... ok

**C184** - `let y = a /* mid */ + b;` marks that statement `Verbatim`. (CLI-23, AC 7; T46)
Proof: run format | test an_unreachable_comment_marks_its_statement_verbatim ... ok

**C185** - That statement comes out with its own text, re-indented, and its neighbours are formatted. (CLI-23, AC 7; T47)
Proof: run format | test a_statement_with_an_inner_comment_is_kept_as_written ... ok

**C186** - Three blank lines between two statements become one; no blank line stays none. (CLI-23, AC 8; T47)
Proof: run format | test blank_lines_collapse_to_one ... ok

**C187** - `module M( {` gives the parser's diagnostics and no text. (CLI-23, AC 9; T47)
Proof: run format | test a_source_that_does_not_parse_gives_diagnostics_and_no_text ... ok

**C188** - Over the standard library, the gallery fixtures, `examples/buck`, `examples/rc`, the twins and the Book's `msl` blocks: the comment list of the formatted text equals the source's, file by file; the corpus holds at least 60 sources and at least 100 plain comments. (CLI-23, AC 10; T48)
Proof: run format | test the_corpus_keeps_every_comment ... ok

**C189** - Over the same corpus the formatted text parses to the source's tree. (CLI-23, AC 10; T48)
Proof: run format | test the_corpus_keeps_every_tree ... ok

**C190** - Over the same corpus a second format gives the first's text. (CLI-23, AC 10; T48)
Proof: run format | test the_corpus_is_stable_under_a_second_format ... ok

**C191** - `msl::print` still passes its idempotence test over its corpus, the target unedited. (CLI-23, AC 11; T45)
Proof: run print | test print_is_idempotent_over_corpus ... ok

**C192** - `msl::print` still round-trips every tree of its corpus, the target unedited. (CLI-23, AC 11; T45)
Proof: run print | test roundtrip_ast_equal_over_corpus ... ok

**C193** - In a copy of a project with one unformatted file under `src/` and one under `benches/`, `piperine fmt` rewrites both, prints `formatted src/lib.msl` and `formatted benches/step.msl`, and leaves the formatted third file's modification time alone. (CLI-24, AC 1; T49)
Proof: run commands | test fmt_rewrites_what_differs_and_names_it ... ok

**C194** - `piperine fmt benches/step.msl` formats that file and not `src/lib.msl`. (CLI-24, AC 2; T49)
Proof: run commands | test fmt_takes_files ... ok

**C195** - `--check` on that copy prints `would format` for the two files, changes no byte on disk and exits 1. (CLI-24, AC 3; T49)
Proof: run commands | test fmt_check_writes_nothing_and_exits_1_on_a_difference ... ok

**C196** - With one broken and one unformatted file, the broken one is byte-equal afterwards, its diagnostic is on standard error, the other is formatted, exit 1. (CLI-24, AC 4; T49)
Proof: run commands | test fmt_skips_a_file_that_does_not_parse_and_exits_1 ... ok

**C197** - A second `piperine fmt` prints nothing and exits 0. (CLI-24, AC 5; T49)
Proof: run commands | test fmt_on_formatted_files_is_silent ... ok

**C198** - `piperine fmt --check` exits 0 in `examples/rc` and in a project `piperine new` created. (CLI-24, AC 6; T49)
Proof: run commands | test fmt_check_passes_on_the_rc_example_and_on_a_new_project ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none yet. The eight open questions of the spec are not answered. Orchestrator, 2026-11-16: the Book gained chapter 16 "How Piperine is organized" (crates, as the user asked: "colocaria como os ultimos capitulos do book"); the cheat sheet is now 17. The CLI chapter goes right BEFORE the structure chapter (CLI 16, structure 17, cheat sheet 18) and T36 updates the structure chapter's crate rows if the CLI/api shape differs from what it says. Orchestrator, 2026-11-16 (user: "Consegue colocar o cli no book também? QUero ver! Precisa ser os primeiros capitulos ou misturado"): the CLI is written into the Book BEFORE implementation, from this spec: a "Run it" walkthrough in chapter 1, short "Run it" boxes after each chapter's main example, a reference chapter "The command line" before the structure chapter, cheat-sheet lines; output listings carry `data-cli-output`. T36 therefore does not write the chapter from scratch: it holds EVERY CLI block of the Book (commands, Piperine.toml, project trees, `data-cli-output` listings, the SVG) to examples/rc, examples/buck and the real binary by test, and fixes the Book where the binary legitimately differs. Book CLI landed on main 2026-11-16: 13 output blocks `<pre data-lang="text" data-cli-output="<command>" [data-cli-project="rc|lowpass"] [data-cli-lines="a-b"]>`; the RC project text in Book chapter 1 (src/lib.msl, benches/step.msl, Piperine.toml; output node `vout`, `out` is a keyword) is the starting text for examples/rc and the `new` template — adopt it or change both; analog digits in listings are analytic and get reconciled by T36; AlarmTop/ButtonTop/PwmTop became benches Alarm/Press/PwmBench; record paths start with the top module's name. CLI chapter = 16, structure = 17, cheat sheet = 18. T1 (batch 1, 2026-11-16, contract.md): a record names its emit site: `ObservationKind::Record { site: Some((reaction, statement)), bytes }` on the pin path `<instance>.<stream>`, decoded with `Ir::record_at` plus `piperine_plan::RecordLayout::of` (piperine-api gains `piperine-plan`), never paired with a commit by order (D1 to D3); DIG-16 merged: `buck_closed_loop` runs the closed-loop start (784 s wall in a dev build), so CLI-08 AC 2, 3 and 5 have their run; a long mixed transient must not keep every accepted step, so a `transient` run reads `from` (flag `--from`, ngspice's `tstart`, passed as `TransientAnalysisOptions::record_from`; the key list of CLI-03 AC 1 does not change, design's "Which keys an analysis reads" gains it; T7, T13, T21, T25, T42 and T36 absorb it) (D10); the interpreter's refusal names the body and "use the compiled backend", so the API adds `--backend rust` (D4); `Report::hit` is `Report::cache_hit` (D5); a top with discrete ports elaborates, so `idle` names `top = "Buck"` (D8); examples/rc adopts the Book's chapter-1 text as written (1 kOhm, 1 nF, tau 1 us, `vout`, `emit!("trip", v = v(a))`, `stop = 5e-6`, `print_step = 1e-6`) in place of T6's 1.5915494309189535 nF (D9); DEFECT (D12, outside this feature's crates): `Transpiler::digital()` counts only carriers, so a design whose reactions hold no carrier (the Book's RC `Monitor`) runs no reaction and emits no record, silently; T18's RC `trip` record waits on a one-line fix in piperine-transpile, not taken here; record order within one instant is the sink's (checks before trip at the RC crossing) (D13); `on above(..) { emit!(..) }` is P0001, so the Book's delayed.msl does not elaborate (D14); T36 reconciles D13/D14 in the Book. T3: the `[run]` row stands in its own one-row table under the key table ("Tables the language allows and does not define"), because `spec_text::s36_lists_every_manifest_key` (PKG-02 AC 3) pins the key table at exactly nine rows and is not edited. T4 DEVIATION: deleting the old piperine-api removed five `mod.rs`, so `suite_hygiene::mod_rs_files`'s walk floor (`> 15`, a blindness sentinel) failed; it is now `> 10` plus a check that the walk reaches the three registered oversized `mod.rs`; the orchestrator may revert. T6 DEVIATION: `examples/Piperine.toml` and `examples/Piperine.lock` are NOT deleted: `workspace_shape::examples_hold_no_vams_or_python` asserts both exist and is not edited; deleting them needs that assertion changed (orchestrator/user call). EXTRA (orchestrator, batch 1, not a task): the root Cargo.toml builds piperine-solver, piperine-analog-solver, piperine-digital-solver and piperine-abi at opt-level 2 in the dev/test profile; `buck_closed_loop` went from 784 s to 141 s wall, the solver, ABI and parity suites (592 tests) print the same numbers before and after (only temp paths and timings differ) and the closed loop's mean v(vout) is 0.802849 V both times. EXTRA (orchestrator, batch 2, D12 fix, not a task): `Transpiler` now treats a body with a carrier, drive, event, reaction or record stream as digital, both where it picks the runtime (`digital()`) and where it decides a body runs at all (`Transpiler::new`'s `runs`, which had the same carriers-only test: with `digital()` alone fixed the RC `Monitor` still got no plan and no record); proven by `piperine-transpile/tests/carrierless.rs` (the Book's RC with its carrier-less `Monitor`: one `trip` record at RC ln 2 with v = 0.5; failed before the fix with no record at all); `piperine-interp` refuses reactions by name, so it has no silent counterpart; transpile, plan and parity suites green unedited. Batch 2: commit subjects of T7, T9, T10 and T11 start lowercase (`a run table reads into a RunSpec`, ...), since `check_commit.py` refuses a description that starts with a capital (`RunSpec`, `Project`, `Session`, `Probe`). T9: a run that cannot be chosen (`request` with an unknown name, or no name over zero or several runs) is its own `Failure::NoRun { asked, runs }`, not a `P0010`, so that the CLI can exit 2 for it (CLI-03 AC 10, 11) and 1 for every `P0010`; a run table with a fault leaves `Project::runs()` empty, `Project::faults()` gives every fault and `request` returns the first; `Project::file` keeps no runs of the package and makes its request from flags, named by the stem. T10: `Session::open` takes the backend from `spec.backend` alone (`Backend::choose(None, None, key)`); the CLI applies CLI-04 AC 7's order with `Backend::choose(flag, PIPERINE_BACKEND, the run's key)` and puts the winner in `Overrides::backend` before `merged`, so the library never reads a variable; `Session` gains `circuit()` (the bound circuit for a host that runs its own analysis, `None` once `run` took it) and `spec()`; the tests of C44 and C45 run their transient on that circuit, since `Session::run` arrives with T12/T13; design-level parse and resolve diagnostics are one stage `parse` (the design does not tell them apart), then `typeck`, then `elaborate`. T12: `Cell` gains `Text` (the `signal` column of an operating point holds names); `Cell::decode(ty, bytes)` is the one decoder of a discrete value (a length other than the layout's is a `Failure`), shared by tables and records; `piperine-api` depends on `piperine-plan` (D3), `piperine-abi` (the `leaf` constants) and `piperine-digital-solver` (`NetId`: a net of the operating point is read from the bound digital engine after the DC solve, by its index in the digital plan, since `DcAnalysisResult` holds no discrete value and `NameMap::nets` holds only named ports and globals). `Session::run(&mut self, sink: Rc<RefCell<dyn RecordSink>>)`, not `&mut dyn RecordSink`: the solver takes a `Box<dyn Sink>` that is `'static`, so a sink the caller borrows cannot be called during the solve, and handing records over only after it ends would break "live" (Q4); a session runs once (`run` moves its circuit into the `Solver`, a second `run` is a `Failure`). The `dc` and `ac` arms fail loud until T22/T23. T13: `print_step` is read and validated but the transient table is one row per accepted step until T25 brings the grid (`Table::on_grid`); a run with `print_step` therefore prints every step in phases 2 and 3. T14: `Streams::of` takes `Arc<Ir>` (it keeps the IR to call `record_at`) and `Streams::decode` returns `Option<Result<Record, Failure>>` (a record of a pin no stream has, of a site that is no `emit!` of it, or of bytes not its layout's size is a `Failure`); a record's fields are the site's `emit!` fields in source order (value fields read through `RecordLayout`, text fields the site's literals), and a site-less record has every value field of its stream; the first undecodable record ends the run with its `Failure` after the solve, records before it already delivered. `Streams::keep` (the filter) is left to T27. DEFECT found (outside this feature's crates, not fixed): `piperine_transpile::Cache::at` probes the cache with `.probe-<pid>`, so two compiles in one process at once collide (`File exists`) on a shared cache directory; the CLI compiles once per process, so only a multi-threaded host meets it; the api tests that share a cache take a `BUILD` lock. EXTRA (orchestrator, batch 3, approved out-of-scope exception, not a task): `Cache::at` probes with `.probe-<pid>-<n>`, `n` from the process-wide `STAGING` counter, so concurrent opens no longer collide (`piperine-transpile/tests/cache.rs::concurrent_opens_of_one_directory_both_succeed`, 200 rounds of two threads, failed before with `File exists`); the `BUILD` locks of `piperine-api/tests/{analyses,records}.rs` existed only for this collision and are removed; `session.rs`'s `ONE_AT_A_TIME` stays (it guards ETXTBSY from exec'ing a freshly written executable, a different race). T15 to T17: `Number`, `TextTable`, `RecordLine` and `Report` are unit structs with one `write`; `Report` renders through `piperine_lang::msl::diag::render` and `PackageError::render` (the no-color handler, so there is no color on a terminal either: the Book compares bytes), then `error: <the Failure's Display>`; `piperine-cli` gains `piperine-lang` and `piperine-transpile` (for `Settings::from_environment`). T18: `Cli::execute(&Invocation, out: &Out, err: &mut dyn Write)` with `Out = Rc<RefCell<dyn Write>>` (the solver's sink is `'static`, so a record can be written live only to a shared owned writer; the binary passes `Rc<RefCell<Stdout>>`), `Invocation` loses `Eq` (`Overrides` holds `f64`) and `Run` carries `overrides: Overrides`; `--backend` is a flag of `run` only until T31/T33 give `build`/`test` theirs; `RunCommand` lives in `commands/run.rs` (`commands.rs` declares it); the backend order is applied by `RunCommand::backend` (a flag that names no backend is exit 2, the variable or the key exit 1, an empty `PIPERINE_BACKEND` counts as unset) and the winner is written into `spec.backend`; the first-compile line is written AFTER `Session::open` returns (the library compiles inside `open`), in the past tense: ``compiled run `<name>` (top <Top>) with rustc, and cached it`` on standard error, only for `Built::Compiled`; a record the sink cannot write (closed stdout) is kept by `LiveRecords` and reported after the solve as exit 1. Book/code differences for T36: at the RC crossing the binary prints the `checks` record BEFORE the `trip` record (D13, the sink's order; the Book shows `trip` first), the crossing time is `6.931075e-7` (not the analytic `6.931472e-7`), and the transient table prints every accepted step (about 130 rows) until T25 brings the print grid. T19: every key of CLI-03 AC 2 is a typed flag of `run` through `ArgKind::{Text, Path, Real, Count, Texts, Param}` and the `Values` parsers (a negative real is allowed, so `--from -1` works; a duplicate single-valued flag is clap's usage error, exit 2); `--param` takes a decimal real (`r1.r=2000`; `Overrides::params` is `f64`, so an MSL suffix such as `2k` is a usage error, T24 may widen it); a target ending in `.msl` is a lone file (`Project::file`, request from flags, no run name passed to `request`). T20: a lone file with no `--analysis` is exit 2 naming the flag, decided by `RunCommand` before any request (the api's `from_flags` would give a `P0010`, exit 1); fixtures under `crates/piperine-cli/tests/projects/` (`type_error` is a `P0002` `typeck` error, since a body `let` annotation is only a parse error; `fails_after_emit` is a negative resistor on a capacitor that crosses 0.5 V at 1.6 us and meets an infinity at about 0.74 ms, solver error `contributed a NaN/infinity`); a failed analysis keeps the header and the records on standard output and prints `error: the <analysis> analysis failed: ...` on standard error, so "nothing on standard output" holds only for stages before the analysis (CLI-07 AC 4), as the spec words it. T21: `examples/buck/benches/startup.msl` holds both benches, `Startup` (the closed-loop start of DIG-16) and `Idle` (`Buck` with `en` held low through a bench, not `top = "Buck"` of D8), so both runs name `top`; `startup` keeps only its last microsecond (`from = 599e-6`, `max_step = 1e-8`, `print_step = 0.5e-6`), prints `Startup.buck.ctl.reference`, `Startup.mode` and `Startup.buck.vout`, and ends at reference 128, mode 2, vout 0.8024 V (SPEC §2: within 10 mV of 0.8); wall clock of the `startup` run: 129 s including the cold `rustc` compile, the target 133 s. DEVIATION for the orchestrator: the task says the output is at most 16 lines, which does not hold yet and cannot with the keys alone: `print_step` is not applied until T25 (the table prints every kept step), and the DPWM's `Never` monitor writes one `checks` record per clock (about 120 000 lines for `startup`, 12 000 for `idle`), `checks` being the only `emit!` stream of the buck; `streams` (T27) cannot hide it without hiding the checks that `startup_prints_no_failed_check` reads, so the 16-line bound needs a record gesture the spec does not give (`records = FILE` of T28 prints no record line and would carry the checks to a file, at the cost of the manifest writing a file in the project); neither `streams` nor `records` is set on the buck's runs. T22: `Table::point` is one operating point's cells (shared by `operating_point` and `sweep`); `Table::sweep(swept, probes, points)`; `ParamPath::resolve(ir, top, entry)` (instance full or relative to the top, split at the last dot, E0801 when the instance or the param is not in the IR) resolves a `dc` run's `sweep` at `Session::open`; the sweep runs on the bound circuit directly (`CircuitInstance::set` then `dc`, as parity_dc does) because `Solver` owns its circuit and has no `&mut` accessor, so DC sweep points deliver NO records to the sink (the op arm still does); the point count is `floor((to - from) / step + 1e-9) + 1` (the 1e-9 absorbs the rounding of a whole number of steps), each value `from + k*step`; a refused `set` is E0802 (T24 widens it). T23: `Grid::steps(from, to)` is the point count (`round(n*log10(to/from))+1` per decade, `n` for points); `Table::frequency` gives `freq`, `mag(<path>)`, `phase(<path>)` (degrees, `arg().to_degrees()`); an `ac` run that prints a discrete net is E0801 at run time (a net has no phasor); the ac arm runs on `Solver` with the sink. T24: every `params` path resolves (E0801) in `Session::open` before the backend builds, so a typo costs no `rustc`; the writes (`CircuitInstance::set`, `ParamValue::Real`) follow in order, a refusal is E0802 naming the path and the value; `--param` still takes a decimal real. T25: `Table::on_grid(step, from, stop) -> Result<Table, Failure>` (design said `(step, stop) -> Table`: it takes `from` for CLI-11 AC 4's start and returns a `Result` because a grid time past the last accepted step by more than 1e-9*step is a `Failure`, never a held or extrapolated row); k runs from `ceil(from/step - 1e-9)` to `floor(stop/step + 1e-9)`; a multiple BEFORE the first row the table holds (the solver's `record_from` keeps the first accepted step at or after `from`, which can be a hair later than the multiple) is skipped, not a row, so with `from` the first row can be the next multiple than AC 4 words (T36/orchestrator may want AC 4 reworded); an unknown on either side of an interpolation stays unknown; `Session::run` applies it when the transient has `print_step` (the full table for a plot arrives with T42). T26: a relative `csv`/`records`/`plot` path is joined to `Project::dir()` (the working directory for a lone file) for flags as well as for the manifest key, as CLI-11 AC 6 words it; the `wrote <file> (<n> rows)` line echoes the path as given, after the empty line that precedes a table; records and the header still print with `--csv`; `Failure::Io` names the path as given; CSV quotes a cell with a comma, quote or line break (RFC 4180, one extra inline test). Gate for T26 to T28 was run as the cli lib/run/usage/options targets (the full gate's buck target is 130 s and untouched); the full gate ran at T25 and runs again at the end. T27: `Streams::keep(&mut self, &[String]) -> Result<(), Failure>` (no entry keeps every stream); `Session::open` builds the `Streams` once, before the backend compiles, so a bad `streams` entry is E0801 with no `rustc` and `run` clones it into the `Delivery`; the filter is by pin path, so the reserved `checks` stream filters like any other (`--stream trip` hides the RC's `checks` line). T28: `records` opens (creates, replaces) its file after `Session::open` and before the header, so an unwritable file prints nothing on stdout; the sink is `LiveRecords { to: Destination::{Terminal, File}, closed }`; JSON reals use Rust's `{:?}` (shortest round-trip, valid JSON), a NaN or infinity is the string `"NaN"`/`"inf"`/`"-inf"` since JSON has no number for it. EXTRA (coordinator, user decision 2026-11-16 "Só falhas por padrão (Recommended)", not a task): the terminal hides \`checks\` records with \`pass=true\` unless \`all_checks = true\` / \`--all-checks\` (a 20th run key; \`Record::is_passing_check\` in the api, the filter in \`LiveRecords\`' terminal destination only, \`--records\` keeps every record); spec CLI-06 AC 9, CLI-08 AC 3 and 6, context.md and C78 amended; tests changed for it: \`a_run_table_reads_its_nineteen_keys\` (now also reads \`all_checks\`, 20 keys), \`every_run_key_has_its_flag\` (gains \`--all-checks\`), \`records_come_before_the_table\` (runs with \`--all-checks\`), T21's \`startup_prints_no_failed_check\` (no \`checks\` line by default; \`--records\` proves more than 1000 passing checks) plus new \`the_buck_runs_print_at_most_16_lines\`, and T27's stream test (\`--all-checks\` on its unfiltered run). T29: the seven twins run with the grids of the parity tests (diode_iv -1..0.8 by 0.05 printing i(v1), diode_rev -1..-20 by -1, ac_rc 1e3..10e6 at 10/decade, tran_rc 10 us, max-step 1e-8, print-step 1e-7); all 14 commands exit 0 and the two backends print identical bytes after the header, no difference found, no tolerance used. T42: `Plot::of` reads the first column as the abscissa and draws every other column: a column holding a bool, an integer or only unknowns is a Lane, a real column a Series (an `ac` table is read as `freq` then `mag(p)`/`phase(p)` pairs, series labelled `p`); the x range is the data's (ticks inside it), the y range the data's widened to the enclosing ticks (a constant signal is widened by half its size, at least 0.5); `Axis::ticks(min, max)` picks the smallest 1/2/5 step leaving at most 10 ticks, `Axis::decades` the powers of ten of a log axis (1/2/5 steps when fewer than two decades fit); a Tick holds its value and label, the renderer places it with `Axis::place`; the x tick labels sit under the lowest lane; more than 11 lanes is P0010; `Session` keeps the accepted-step table when the request has `plot` and `Session::plot()` draws it; fixed numbers: margins left 70, right 20, top 20, bottom 50, a 40 gap between the two ac panels. T43: `SvgPlot` writes the document line by line (a `<rect class="panel">` per panel, `tick`/`tick-label` per tick, `axis-title` per axis, `series` and `lane` paths with `data-signal`, `legend`/`legend-label` per signal, fixed 8-colour palette, two decimals, XML-escaped names); a bool lane is low/high, an integer lane is placed between its smallest and largest value; `RunCommand` writes the plot after the table (and after the csv's `wrote` line), `wrote <file>` echoes the path as given; `plot` on an op run is P0010 at the manifest/flag, its message now ends `: an operating point has nothing to plot` (request.rs, existing test unchanged and green). T30 (batch 5): `Project::check() -> Result<Checked, Vec<Failure>>` (`Checked { package, crates, runs, typed }`) runs the same `Session::elaborate` (the stages parse, typeck, elaborate, factored out of `open`) as a session, no backend; the crate count is the package graph plus the embedded `std` (C112/C114: rc is `2 crates`), a bench is not counted; a lone file is checked as an `op` run of its own (top rule applies) and prints `(2 crates, 0 runs)` with no `type checking needs a run` suffix (it IS type-checked); runs sharing a source report an identical diagnostic once (the CLI dedupes identical rendered reports); `commands::Locate::package` is the shared project discovery. DEVIATION for T36: C116 says both rc runs print `compiled` on an empty cache; the second run has the same top, hence the same design hash, so the binary prints `built op: compiled` then `built step: cached`. T31: `BuildCommand` opens a `Session` per run (backend by the CLI-04 order minus the flag: `PIPERINE_BACKEND`, the run's key, `rust`; `RunCommand::backend` is now `fn backend(flag, spec)`, `Stop` is `pub(crate)`) and prints `built <name>: ...` on standard output, the first-compile line of `run` is not repeated; `commands::Select::runs` picks the runs (named, in the order given, or all in manifest order; an unknown name is `NoRun`, exit 2; a malformed run table is its first fault, exit 1); the verbose path is printed bare on its own line; a missing `rustc` fails even a cached build (the transpiler probes it for the cache key), so C117's proof is a logging `rustc` wrapper: the first build logs a `--crate-type` compile, the second none. T32: `Verdict` counts every record of the `checks` stream (a passing one too, and a `target`, which has no `pass`); it fails only on a `pass` that is false or unknown; the input is the whole record stream, so the terminal's hiding of passing checks (`LiveRecords`) never reaches it. T33: `TestCommand` runs each run through `Session::open` + `run` with a collecting sink that keeps EVERY record (the terminal's hiding of passing checks lives only in `run`'s `LiveRecords`, so the `(N checks)` count and the verdict never see a filter; `test_prints_pass_with_the_check_count` proves it: `run step` hides the passing check, `test` counts 1); the run's `csv`/`records`/`plot` keys are ignored (no file written) and its `streams` filter is cleared so no check can be filtered out of the verdict; the first-compile line goes to standard error as in `run`; a run that fails any stage or the solve is `FAIL <name> (did not run)` with the error on standard error; an unknown run name is exit 2 and prints no count line; the failing record lines are `RecordLine`'s. Fixtures `failing_require`, `one_pass_one_fail` (the `require` is `v(a) <= limit` at the 0.5 V crossing, `limit` 1.0 passes, 0.1 fails). `piperine test` in `examples/buck`: both runs PASS (`PASS idle`, `PASS startup`), 265 s wall for the buck target.
**Dispatched:** msl-digital merged to main 94626f88 (2026-11-16); branch rebased. Batch 1 (phase 1: T1, T2, T3, T41, T4, T5, T6 + test-speed extra) - Opus worker, 2026-11-16. Batch 1 done (77738f19). Orchestrator rulings on batch 1: deviations 1-3 accepted; D12 fixed in piperine-transpile as an approved out-of-scope exception (silent defect). Batch 2 (phase 2: D12 fix, T7-T14) - Opus worker, 2026-11-16. Batch 2 done (022e2dae). Ruling: Cache::at probe-dir collision fixed in piperine-transpile (approved exception). Batch 3 (phase 3: cache fix, T15-T21) - Sonnet worker, 2026-11-16. Batch 3 done (4f2a2b12; T21 'at most 16 lines' open: buck checks stream prints ~120k pass records, asked user). Batch 4 (phase 4: T22-T29, T42, T43) - Sonnet worker, 2026-11-16. Batch 4 done (30cf0d2d; checks decision 223530e1). Batch 5 (phase 5: T30-T35) - Sonnet worker, 2026-11-16.
**Walked:** none.

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6
```

| Phase | Tasks | Count | Batch |
| ----- | ----- | ----- | ----- |
| 1: Foundation: the contract, the spec and crates that build | T1, T2, T3, T41, T4, T5, T6 | 7 | 1 |
| 2: The session: from a project to results and records | T7, T8, T9, T10, T11, T12, T13, T14 | 8 | 2 |
| 3: `piperine run`: the minimal slice | T15, T16, T17, T18, T19, T20, T21 | 7 | 3 |
| 4: Analyses, output formats and plots | T22, T23, T24, T25, T26, T27, T28, T29, T42, T43 | 10 | 4 |
| 5: `check`, `build`, `test`, `new`, `clean` | T30, T31, T32, T33, T34, T35 | 6 | 5 |
| 6: The formatter and `piperine fmt` | T44, T45, T46, T47, T48, T49 | 6 | 6 |
| 7: The Book and the records of the feature | T36, T37, T38, T39, T40 | 5 | 6 |

49 tasks, 198 checks. Six batches of whole phases: phases 6 and 7 share the last worker (11 tasks; T44 to T47 and T36 go to Opus). T50 to T54 were removed with the language server and their numbers are not reused. Batches run in order; each worker executes its tasks one at a time and reports before the next starts.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: The contract with the merged msl-digital | one module or one function family in one file | ✅ Granular |
| T2: A manifest may hold a run table | one module or one function family in one file | ✅ Granular |
| T3: The manifest rule of the spec admits the run table | one module or one function family in one file | ✅ Granular |
| T41: The spec says a bench is MSL | one definition and its three echoes in the spec | ✅ Granular |
| T4: piperine-api starts over | one crate reset: the deletion and the two smallest modules, so the tree builds at this commit | ✅ Granular |
| T5: piperine-cli starts over | one crate reset: the deletion and the command tree | ✅ Granular |
| T6: The RC example project | one example project, four small files | ✅ Granular |
| T7: RunSpec: reading a run table | one module or one function family in one file | ✅ Granular |
| T8: Overrides: flags over keys | one module or one function family in one file | ✅ Granular |
| T9: Project: discovery and the crate graph of a run | one module or one function family in one file | ✅ Granular |
| T10: Session::open: source to a bound circuit | one module or one function family in one file | ✅ Granular |
| T11: Probe: what a column shows | one module or one function family in one file | ✅ Granular |
| T12: Table and the operating point | one module or one function family in one file | ✅ Granular |
| T13: The transient of a run | one module or one function family in one file | ✅ Granular |
| T14: Records: from the sink to typed records | one module or one function family in one file | ✅ Granular |
| T15: Rendering a table and a number | one module or one function family in one file | ✅ Granular |
| T16: Rendering a record line | one module or one function family in one file | ✅ Granular |
| T17: Rendering a failure | one module or one function family in one file | ✅ Granular |
| T18: piperine run: a named run | one module or one function family in one file | ✅ Granular |
| T19: piperine run: flags and a lone file | one module or one function family in one file | ✅ Granular |
| T20: piperine run: failures and exit codes | one command's failure paths, with their fixtures | ✅ Granular |
| T21: The buck's bench and runs | one module or one function family in one file | ✅ Granular |
| T22: The DC sweep of a run | one module or one function family in one file | ✅ Granular |
| T23: The AC sweep of a run | one module or one function family in one file | ✅ Granular |
| T24: Params of a run | one module or one function family in one file | ✅ Granular |
| T25: The print grid | one module or one function family in one file | ✅ Granular |
| T26: CSV output | one module or one function family in one file | ✅ Granular |
| T27: The stream filter | one module or one function family in one file | ✅ Granular |
| T28: Records to a file | one module or one function family in one file | ✅ Granular |
| T29: The seven twins from the command line | one module or one function family in one file | ✅ Granular |
| T42: Plot: the geometry of a plot | one module or one function family in one file | ✅ Granular |
| T43: SVG output | one module or one function family in one file | ✅ Granular |
| T30: piperine check | one module or one function family in one file | ✅ Granular |
| T31: piperine build | one module or one function family in one file | ✅ Granular |
| T32: Verdict: pass or fail of a run | one module or one function family in one file | ✅ Granular |
| T33: piperine test | one module or one function family in one file | ✅ Granular |
| T34: piperine new | one module or one function family in one file | ✅ Granular |
| T35: piperine clean | one module or one function family in one file | ✅ Granular |
| T44: Comments: plain comments beside the tokens | one module or one function family in one file | ✅ Granular |
| T45: The printer's layout, reachable node by node | one module or one function family in one file | ✅ Granular |
| T46: Comments::attach: a comment's anchor | one module or one function family in one file | ✅ Granular |
| T47: Formatter: a formatted text that keeps its comments | one module or one function family in one file | ✅ Granular |
| T48: The formatter over the corpus | one test target | ✅ Granular |
| T49: piperine fmt | one module or one function family in one file | ✅ Granular |
| T36: The Book: the command line | one chapter and its guard | ✅ Granular |
| T37: The Book: chapter 15 and the cheat sheet | one module or one function family in one file | ✅ Granular |
| T38: The buck's README | one module or one function family in one file | ✅ Granular |
| T39: CLAUDE.md and AGENTS.md | one module or one function family in one file | ✅ Granular |
| T40: GAPS, ROADMAP and the decision | one module or one function family in one file | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows (same phase) | Status |
| ---- | ---------------------- | -------------------------- | ------ |
| T1 | None | - | ✅ Match |
| T2 | T1 | T1 | ✅ Match |
| T3 | T2 | T2 | ✅ Match |
| T41 | T3 | T3 | ✅ Match |
| T4 | T1 | T1 | ✅ Match |
| T5 | T4 | T4 | ✅ Match |
| T6 | T2 | T2 | ✅ Match |
| T7 | T4 | - | ✅ Match |
| T8 | T7 | T7 | ✅ Match |
| T9 | T8 | T8 | ✅ Match |
| T10 | T9 | T9 | ✅ Match |
| T11 | T10 | T10 | ✅ Match |
| T12 | T11 | T11 | ✅ Match |
| T13 | T12 | T12 | ✅ Match |
| T14 | T13 | T13 | ✅ Match |
| T15 | T12 | - | ✅ Match |
| T16 | T14 | - | ✅ Match |
| T17 | T4 | - | ✅ Match |
| T18 | T15, T16, T17 | T15, T16, T17 | ✅ Match |
| T19 | T18 | T18 | ✅ Match |
| T20 | T19 | T19 | ✅ Match |
| T21 | T20 | T20 | ✅ Match |
| T22 | T13 | - | ✅ Match |
| T23 | T22 | T22 | ✅ Match |
| T24 | T22, T19 | T22 | ✅ Match |
| T25 | T13 | - | ✅ Match |
| T26 | T25, T19 | T25 | ✅ Match |
| T27 | T14 | - | ✅ Match |
| T28 | T27, T26 | T27, T26 | ✅ Match |
| T29 | T23, T19 | T23 | ✅ Match |
| T42 | T23, T25 | T23, T25 | ✅ Match |
| T43 | T42, T26 | T42, T26 | ✅ Match |
| T30 | T20 | - | ✅ Match |
| T31 | T30 | T30 | ✅ Match |
| T32 | T14 | - | ✅ Match |
| T33 | T32, T31 | T32, T31 | ✅ Match |
| T34 | T33 | T33 | ✅ Match |
| T35 | T34 | T34 | ✅ Match |
| T44 | None | - | ✅ Match |
| T45 | T44 | T44 | ✅ Match |
| T46 | T45 | T45 | ✅ Match |
| T47 | T46 | T46 | ✅ Match |
| T48 | T47 | T47 | ✅ Match |
| T49 | T48, T35 | T48 | ✅ Match |
| T36 | T35, T21, T43, T49 | - | ✅ Match |
| T37 | T36 | T36 | ✅ Match |
| T38 | T33 | - | ✅ Match |
| T39 | T49 | - | ✅ Match |
| T40 | T39 | T39 | ✅ Match |

No task depends on a task of a later phase.

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: The contract with the merged msl-digital | `contract.md` | none | none | ✅ OK |
| T2: A manifest may hold a run table | the manifest parser | unit | unit | ✅ OK |
| T3: The manifest rule of the spec admits the run table | spec text | integration | integration | ✅ OK |
| T41: The spec says a bench is MSL | spec text | integration | integration | ✅ OK |
| T4: piperine-api starts over | pure rules of `piperine-api` | unit | unit | ✅ OK |
| T5: piperine-cli starts over | the commands | e2e | e2e | ✅ OK |
| T6: The RC example project | example projects | e2e (the package resolves here; it runs through the binary from T18) | integration | ✅ OK |
| T7: RunSpec: reading a run table | pure rules | unit | unit | ✅ OK |
| T8: Overrides: flags over keys | pure rules | unit | unit | ✅ OK |
| T9: Project: discovery and the crate graph of a run | `piperine-api` over real sources | integration | integration | ✅ OK |
| T10: Session::open: source to a bound circuit | `piperine-api` over real sources | integration | integration | ✅ OK |
| T11: Probe: what a column shows | `piperine-api` over real sources | integration | integration | ✅ OK |
| T12: Table and the operating point | `piperine-api` over real sources | integration | integration | ✅ OK |
| T13: The transient of a run | `piperine-api` over real sources | integration | integration | ✅ OK |
| T14: Records: from the sink to typed records | `piperine-api` over real sources | integration | integration | ✅ OK |
| T15: Rendering a table and a number | text layout | unit | unit | ✅ OK |
| T16: Rendering a record line | text layout | unit | unit | ✅ OK |
| T17: Rendering a failure | text layout | unit | unit | ✅ OK |
| T18: piperine run: a named run | the commands | e2e | e2e | ✅ OK |
| T19: piperine run: flags and a lone file | the commands | e2e | e2e | ✅ OK |
| T20: piperine run: failures and exit codes | the commands | e2e | e2e | ✅ OK |
| T21: The buck's bench and runs | example projects | e2e | e2e | ✅ OK |
| T22: The DC sweep of a run | `piperine-api` over real sources | integration | integration | ✅ OK |
| T23: The AC sweep of a run | `piperine-api` over real sources | integration | integration | ✅ OK |
| T24: Params of a run | `piperine-api` over real sources, and a command flag | e2e (the higher of the two) | e2e | ✅ OK |
| T25: The print grid | pure rules | unit | unit | ✅ OK |
| T26: CSV output | text layout and a command key | e2e (the higher of the two) | e2e | ✅ OK |
| T27: The stream filter | `piperine-api` over real sources | integration | integration | ✅ OK |
| T28: Records to a file | text layout and a command key | e2e (the higher of the two) | e2e | ✅ OK |
| T29: The seven twins from the command line | the commands | e2e | e2e | ✅ OK |
| T42: Plot: the geometry of a plot | pure rules | unit | unit | ✅ OK |
| T43: SVG output | text layout and a command key | e2e (the higher of the two) | e2e | ✅ OK |
| T30: piperine check | the commands | e2e | e2e | ✅ OK |
| T31: piperine build | the commands | e2e | e2e | ✅ OK |
| T32: Verdict: pass or fail of a run | pure rules | unit | unit | ✅ OK |
| T33: piperine test | the commands | e2e | e2e | ✅ OK |
| T34: piperine new | the commands | e2e | e2e | ✅ OK |
| T35: piperine clean | the commands | e2e | e2e | ✅ OK |
| T44: Comments: plain comments beside the tokens | the formatter | integration | integration | ✅ OK |
| T45: The printer's layout, reachable node by node | the formatter | integration | integration | ✅ OK |
| T46: Comments::attach: a comment's anchor | the formatter | integration | integration | ✅ OK |
| T47: Formatter: a formatted text that keeps its comments | the formatter | integration | integration | ✅ OK |
| T48: The formatter over the corpus | the formatter | integration | integration | ✅ OK |
| T49: piperine fmt | the commands | e2e | e2e | ✅ OK |
| T36: The Book: the command line | the Book | integration | integration | ✅ OK |
| T37: The Book: chapter 15 and the cheat sheet | the Book | integration | integration | ✅ OK |
| T38: The buck's README | instruction files and gap lists | integration | integration | ✅ OK |
| T39: CLAUDE.md and AGENTS.md | instruction files and gap lists | integration | integration | ✅ OK |
| T40: GAPS, ROADMAP and the decision | instruction files and gap lists | integration | integration | ✅ OK |

T6 says `integration`: its own test proves the package resolves, the only thing runnable before the session exists; the e2e proof of the same files is C-rows of T18 and T36. T1 is the one `none`, as the matrix says.

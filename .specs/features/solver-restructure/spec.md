# Solver Restructure Specification

**Approved**: 2026-11-14 - user: "Approve and run (Recommended)"

Feature (A) of `.specs/features/solver-completeness/`. Structure only: no solver logic changes.

## Problem Statement

`piperine-solver` cannot run one test: its dev-dependencies are the dead Verilog-AMS frontend. It also carries surface that MSL has no construct for, in one crate where analog numerics, the digital scheduler and the mixed drivers share a namespace. The workspace still holds five crates the user has retired. The logic work that follows (feature B, the event boundary) needs a small, green, three-crate structure to land on, and a verifier needs to be able to hold this step to "nothing changed".

## Goals

- [ ] `cargo test -p piperine-solver`, `-p piperine-analog-solver` and `-p piperine-digital-solver` each build and pass with no other crate of the tree.
- [ ] The workspace holds none of `piperine-ir`, `piperine-codegen`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`, and still loads (`cargo metadata` succeeds).
- [ ] The solver is three crates; the two sub-solver crates depend on nothing in the tree and name none of each other's types.
- [ ] Every test that passed on the first green run of this feature, and whose subject is not removed, passes at the end with its assertions unchanged.
- [ ] `piperine-lang` and the three solver crates build all targets with zero warnings.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Any change of solver behavior: scheduling order, resolution, the discontinuity path, tolerances, error texts | this feature is structure only; logic is feature (B), built by Opus and verified by Fable |
| Removing the strength table and `DriverTable`, four-state `LogicValue`, the `seq_phase`/`comb_phase` contract, the A2D sampling flags (`analysis.md` S2, S4, S12) | they are the live digital logic; removing them changes results. Feature (B) |
| Removing `discontinuity`, `initial_conditions`, `start_analysis`/`finish_analysis`, runtime banks (S8, S9, S10, S13) | part of the discontinuity and start-up paths the user keeps (Q7); revisited in `solver-time-point` |
| Moving `AnalogDevice`, the per-analysis state types and the analysis drivers into the analog crate | their signatures carry the digital net snapshot. They move when feature (B) replaces the snapshot with events |
| An event-only boundary between the sub-solvers | feature (B). Here the boundary is only a crate boundary: neither sub-solver crate can name the other's types |
| Reducing `piperine-api` to a solver-backed surface (`Session` over Rust-built circuits) | new behavior, its own feature. Here only its manifest loses the removed crates |
| Making `piperine-project`, `piperine-cli`, `piperine-lang-server`, `piperine-api` or the root `piperine` shell compile | each is kept and each has its own later feature; they do not compile today either |
| New tests of solver behavior beyond the ported ones and the structural guards | unit tests of the present behavior exist; this feature keeps them green |
| Editing `docs/spec/` | no rule changes here; the amendments of Q7 and Q9 land in feature (B) |
| `mkdocs`, the VS Code extension, release workflows | not part of the build of the kept crates; touched only where a workflow names a removed crate |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Crate names | `piperine-solver` (event owner, and the public face), `piperine-analog-solver`, `piperine-digital-solver`, under `crates/` | the user's words: "o dono dos eventos, o digital-solver e o analog-solver". The owner keeps the existing name so `use piperine_solver::prelude::*` and `piperine_solver::abi::*` stay the two public paths (MD-17) | y - planner decision; the brief leaves names to the planner |
| What the analog crate holds in this feature | everything with no digital knowledge: `math/` (linear system, faer backend, Newton, TR-BDF2 and LTE, history buffer, initial values, scalars), the `Netlist` naming, `Tolerances` and `Policy`, the tunables of `config.rs`, the convergence strategies and plan, the PI stepper, the non-finite stamp check, and the error type | it is the code that compiles with no digital type today. The analog device contract follows in feature (B) | y - planner decision |
| What the digital crate holds | `digital/` as it stands after cleanup (events, logic values, driver table, sink, state, topology, scheduler) and the `DigitalDevice` trait | the scheduler needs only that trait plus one "is this element digital" question, answered through a small trait the owner implements for `dyn Element` | y - planner decision |
| Where `Error` and `SolverDomain` live | in the analog crate, re-exported by the owner under the same paths. The digital crate returns its own two-variant error, which the owner converts to the same `Error::Simple { domain: Digital, .. }` with the same text | three crates, and neither sub-solver may depend on the other. The numeric code raises most errors and the continuation strategies pass the driver's errors through. The taxonomy keeps its `Digital` variant in the analog crate until feature (B) replaces string errors with typed run errors | y - planner decision; a wart, disclosed |
| `piperine-ir` | removed with the crate; the one function the solver uses (`predict_crossing`, with `CrossDir` and `Prediction`) is copied into the digital crate with its two tests | Q10 | y - user: "Vamos remover o IR, cranelift e Python por agora" |
| Solver test targets deleted | the eight targets whose every assertion is about removed surface: `check_posture`, `margin_eval_points`, `validation_findings`, `validation_gating`, `validation_postures`, `specify_timing_check_enforcement`, `scheduler_region_queue_parity`, `vams_stratified_regions` (30 tests) | Q3, Q4 | y - user: "o que precisar apagar, pode apagar" |
| Test functions deleted inside kept targets | only a function whose every assertion is about a removed item (for example the `HAS_DISTO2` rows of `capabilities_contract`); each is named in its commit body | Q3; the rule keeps the verifier able to audit | y - follows from Q3 |
| Root package tests | every `tests/*.rs` of the root package is deleted; `tests/spice/` and every `.vams` under `tests/` go; `tests/ngspice/*.cir` stay as the oracles of the later parity runs. `tests/suite_hygiene.rs` is not deleted: it moves, with `tests/support/sources.rs`, into `crates/piperine-lang/tests/` and keeps enforcing over the whole repository's kept sources | user: remove "the root package's VAMS e2e tests"; round 3: "Move guard, delete py" | y |
| `examples/` | `examples/*.vams`, `examples/*.py`, the plot images (`examples/*.png`) and `examples/requirements.txt` are deleted. `examples/Piperine.toml` and `Piperine.lock` stay | user, round 3: "Move guard, delete py"; `requirements.txt` lists only what the Python drivers import | y |
| Workspace `default-members` | set to `piperine-lang` and the three solver crates, so a bare `cargo build` or `cargo test` at the root covers exactly the crates expected green | five kept crates are expected broken; without this a bare `cargo` command at the root fails | y - planner decision |
| Baseline | the first task that makes the suite run records the list of passing tests per target; that list minus the announced removals is what must pass at the end. A test that fails on that first run stops the feature for the user | nobody has run this suite since the frontend was deleted | y - planner decision |
| Unit tests that sit in a module being moved | they move with the module and keep their assertions. The unit tests of `math/integration.rs` that build a full `Element` move to the owner crate | they need the composed `Element`, which stays in the owner | y - planner decision |
| Lint gate | `cargo build --all-targets` with zero warnings; no `#![allow]` at file scope (MD-33) | the repo's bar ("zero warnings is the bar") | y - settled by the repo |
| Who builds | Sonnet implements, Opus verifies | user, round 2 | y |

**Open questions:** none. The two rows that were open (the hygiene guard, the Python examples) were answered by the user in round 3 and are recorded above.

**Implicit-requirement dimensions.** Input validation & bounds: N/A because no interface gains an input. Failure / partial failure: SR-01 AC 5 (a red baseline stops the feature), SR-07 AC 6 (a red guard stops it). Idempotency / retry: N/A because nothing runs twice. Auth and rate limits: N/A because the solver is an in-process library. Concurrency / ordering: SR-05 AC 2 (event order unchanged when the region key goes). Data lifecycle: N/A because no data is stored. Observability: N/A because no logging changes. External-dependency failure: SR-02 AC 3 (the workspace loads with the crates gone). State-transition integrity: SR-05 (behavior preserved across every move).

---

## User Stories

Role: the **solver developer** is whoever implements features (B) onward; the **verifier** is the Opus agent that judges this feature.

### P1: The solver's suite runs alone ⭐ MVP

**User Story**: As a solver developer, I want the solver's tests to build and run without any other crate of the tree, so that every later change is proven.

**Why P1**: Nothing else in this feature, or after it, can be verified without it.

**Acceptance Criteria**:

1. The manifest of `piperine-solver` SHALL list no crate of this workspace under `[dev-dependencies]`, and no `rayon`, `rand_distr` or `piperine-ir` under `[dependencies]`.
2. WHEN `cargo test -p piperine-solver` runs THEN every test target SHALL build and every test SHALL pass.
3. WHEN the targets `sens`, `adjoint_dc`, `adjoint_vs_sens`, `transient_reentry` and `pss` run THEN each of their 15 test functions SHALL exist under its present name and assert the same quantities with the same tolerances as at e129b869, on circuits built from hand-written Rust elements.
4. WHEN the hand-written test elements are checked alone THEN a two-resistor divider SHALL read `vin * r2 / (r1 + r2)` within 1e-9 V, an RC step SHALL read `1 - exp(-t / rc)` within 1e-3 at `t = rc`, and a diode fed through a resistor SHALL satisfy its own current equation within 1e-9 A.
5. IF a test that this feature does not rewrite fails on the first run of the suite THEN the work SHALL stop and the failure SHALL be reported to the user before any further task.

**Independent Test**: `cargo test -p piperine-solver` is green on this worktree, where `piperine-codegen` does not compile.

---

### P1: The retired crates and the VAMS suite are gone ⭐ MVP

**User Story**: As a solver developer, I want the workspace to hold only the kept crates, so that nothing in the tree refers to Cranelift, the old IR, Python or the plugin system.

**Why P1**: The user's decision (Q10), and the solver split must not carry dependents on dead crates.

**Acceptance Criteria**:

1. The directories `crates/piperine-ir`, `crates/piperine-codegen`, `crates/piperine-python`, `crates/piperine-plugin` and `crates/piperine-plugin-macros` SHALL not exist.
2. No `Cargo.toml` of the tree SHALL name one of those five crates, and the root manifest SHALL hold no `[profile.dev.package.*]` entry for a Cranelift crate.
3. WHEN `cargo metadata --no-deps` runs at the root THEN it SHALL succeed.
4. The root `tests/` directory SHALL hold no `.rs` file and no `.vams` file, and SHALL still hold the `.cir` files of `tests/ngspice/`.
5. The `examples/` directory SHALL hold no `.vams`, `.py` or `.png` file and no `requirements.txt`.
6. The root manifest SHALL set `default-members` to `piperine-lang` and the three solver crates.
7. No workflow under `.github/workflows/` SHALL build or test a removed crate.

**Independent Test**: `cargo metadata --no-deps` succeeds and lists eight workspace members plus the root package; `ls crates` shows eight directories.

---

### P1: The solver carries no Verilog-AMS-only surface that is safe to remove ⭐ MVP

**User Story**: As a solver developer, I want the surface that MSL has no construct for, and that changes no surviving result, removed, so that the split moves less code and the contract is smaller.

**Why P1**: Q3, Q4, Q5; and each item would otherwise have to be assigned to one of the three crates.

**Acceptance Criteria**:

1. The solver SHALL hold no distortion analysis: no `analyses/disto.rs`, no `load_disto2`, `load_disto3`, `Disto2`, `Disto3`, `DistoOptions`, `DistoSolver`, `DistoResult`, and no `HAS_DISTO2`, `HAS_DISTO3` or `NUMERIC_JACOBIAN` capability.
2. The solver SHALL hold no text or file output: no `core/output.rs`, no `SimOutput`, `StdoutSink`, `BufferSink`, `FileTable`, `McdTable`, `Channel`, `FileId`, `FileError`, `MemRadix`, `parse_memory_text`, and no `io` field on `Context`.
3. The solver SHALL hold no verification channel: no `core/validation.rs`, `core/margin.rs`, `core/coverage.rs`, no `CheckPosture`, `CoverPosture`, `RequireScope`, no `EMITS_VALIDATION`, and no `Element` method among `validation_reports`, `evaluate_constraints`, `margin_report`, `coverage_report`, `reset_margin_tracking`.
4. The solver SHALL hold no `specify` timing surface: no `digital/delays.rs`, `DelayTable`, `PathDelay`, `TimingCheckKind`, `TimingCheckLimit`, `TimingCheckTable`, and no `check_timing_violations`.
5. The solver SHALL hold no stratified regions: no `digital/regions.rs`, `Region`, `RegionAction`, `RegionEvent`, `RegionOutcome`, `RegionQueue`, and no `region` field on `DigitalEvent`.
6. The solver SHALL hold no `simprobe` and no `SimprobeError`.
7. WHEN the surface guard test runs THEN it SHALL fail if `prelude.rs` or `abi.rs` of the owner crate names any item of criteria 1 to 6.

**Independent Test**: the guard test lists every removed name and passes; `grep` for each name under `crates/piperine-*solver*/src` finds nothing.

---

### P1: Three crates with a boundary the compiler enforces ⭐ MVP

**User Story**: As a solver developer, I want the analog numerics and the digital scheduler each in a crate with its exclusive code, under an owner crate that composes them, so that feature (B) changes the boundary in one visible place.

**Why P1**: It is the user's structure ("três crates... cada um com seu código exclusivo").

**Acceptance Criteria**:

1. The workspace SHALL hold the crates `piperine-solver`, `piperine-analog-solver` and `piperine-digital-solver`.
2. The manifests of `piperine-analog-solver` and `piperine-digital-solver` SHALL list no crate of this workspace, under any dependency table.
3. The manifest of `piperine-solver` SHALL depend on both sub-solver crates and on no other crate of this workspace.
4. No source file of `piperine-analog-solver` SHALL contain the identifiers `LogicValue`, `DigitalNet`, `DigitalEvent`, `DigitalState` or `DigitalDevice`.
5. No source file of `piperine-digital-solver` SHALL contain the identifiers `Stamp`, `Netlist`, `AnalogReference`, `Tolerances` or `NewtonRaphsonSolver`.
6. `piperine-analog-solver` SHALL hold the linear-system vocabulary and the faer backend, the Newton engine, the integration scheme, the history buffer, the initial values, the `Netlist` naming, `Tolerances`, `Policy`, the tunables, the convergence strategies and plan, the stepper, the stamp check, `Error` and `SolverDomain`.
7. `piperine-digital-solver` SHALL hold the digital events and values, the driver table, the event sink, the digital state with its checkpoint, the topology, the scheduler and the `DigitalDevice` trait.
8. WHEN a test imports through `piperine_solver::prelude` or `piperine_solver::abi` a name that those modules exported before the split and that criteria of the previous story do not remove THEN it SHALL compile unchanged.
9. WHEN `cargo test -p piperine-analog-solver` and `cargo test -p piperine-digital-solver` run THEN each SHALL build and pass with the unit tests that lived in the modules it received.

**Independent Test**: the boundary guard test reads the three manifests and the two source trees and passes; the surface tests `abi_surface` and `prelude_surface` pass.

---

### P1: Nothing changed ⭐ MVP

**User Story**: As the verifier, I want proof that the restructure moved and removed code without changing what the solver computes, so that feature (B) starts from the behavior that earned the ngspice parity.

**Why P1**: The user chose to keep the present algorithms because they work; a structural step must not disturb them.

**Acceptance Criteria**:

1. WHEN the feature ends THEN every test in the baseline list recorded by its first green run, except those of the removed targets and the removed test functions named in commit bodies, SHALL pass.
2. WHEN events are ordered in the digital queue THEN their order SHALL be `(time, seq)`, which is the order `(time, region, seq)` gave when every event carried the same region.
3. The file `crates/piperine-solver/tests/parity_baseline.rs` SHALL be byte-identical to its content at e129b869 and its 6 tests SHALL pass.
4. WHEN a test file that existed at e129b869, other than the five ported targets, is edited THEN no line containing an assertion SHALL be added to it or changed in it; lines SHALL only be removed, or changed where they name a moved or removed item outside an assertion.
5. WHEN `cargo build --all-targets` runs on `piperine-lang`, `piperine-solver`, `piperine-analog-solver` and `piperine-digital-solver` THEN it SHALL print no warning.

**Independent Test**: the three `cargo test` commands are green; `git diff e129b869 -- crates/piperine-solver/tests/parity_baseline.rs` is empty.

---

### P1: The hygiene guard keeps enforcing ⭐ MVP

**User Story**: As a solver developer, I want the suite-hygiene guard to run in a crate that builds, so that switched-off tests, ignored tests, missing scope headers and file-scope lint suppression are still refused while the root package does not compile.

**Why P1**: The guard is the repository's gate (MD-31). Left in the root package it compiles with nothing and enforces nothing. User, round 3: "Move guard, delete py".

**Acceptance Criteria**:

1. The guard SHALL live at `crates/piperine-lang/tests/suite_hygiene.rs`, with its walk helper beside it under `crates/piperine-lang/tests/support/`, and no `suite_hygiene.rs` SHALL remain under the root `tests/`.
2. WHEN `cargo test -p piperine-lang --test suite_hygiene` runs THEN all of its tests SHALL pass.
3. WHEN the guard walks the repository THEN it SHALL read the root `src/`, every `crates/*/src` and every `crates/*/tests` of the kept crates, the three solver crates included, starting from the repository root and not from the crate that hosts it.
4. The moved guard SHALL hold each of the 11 test functions it has at e129b869, under the same names.
5. The moved guard SHALL hold no registration or exemption that names a path under a removed crate or a deleted file.
6. IF a test of the guard fails on a kept source for a reason that predates this feature THEN the work SHALL stop and the failure SHALL be reported to the user; no detector SHALL be loosened to pass.

**Independent Test**: plant `#[ignore]` on one solver test locally; `cargo test -p piperine-lang --test suite_hygiene` names the file and line; remove it and the guard is green.

---

### P2: The repository says what it now is

**User Story**: As anyone opening the repository, I want the instructions and the decision log to describe the workspace that exists, so that no agent tries to build a crate that is gone.

**Why P2**: `CLAUDE.md` and `AGENTS.md` describe the Verilog-AMS pipeline and are loaded into every session; wrong guidance costs every later feature.

**Acceptance Criteria**:

1. `CLAUDE.md` and `AGENTS.md` SHALL list the eight kept crates with their roles, name the three solver crates and their dependency direction, and state which kept crates do not compile yet.
2. The crate table of `CLAUDE.md` and the workspace map of `AGENTS.md` SHALL hold no entry for a removed crate.
3. `.specs/STATE.md` SHALL hold one decision that records the three-crate solver, the event-only boundary as the target, opaque carriers, the kept discontinuity algorithm and tolerance mapping, and the removed crates, with the user's words.

**Independent Test**: reading the crate table of `CLAUDE.md` against `ls crates` shows the same eight names.

---

## Edge Cases

- IF removing a retired crate would leave a kept manifest unable to load THEN that manifest SHALL lose the dependency line, and the crate SHALL be left failing at compile time, not at manifest load.
- WHEN a kept test builds a `DigitalEvent` literal with a `region` field THEN only that field SHALL be dropped from the literal.
- IF a moved item needs a type from the other sub-solver crate to compile THEN the item SHALL stay in the owner crate, and the design's list of what stays SHALL say so.
- WHEN `cargo test -p piperine-solver` is run between the task that drops the dev-dependencies and the task that ports the last of the five targets THEN only the targets not yet ported SHALL fail to build, and each task's gate SHALL name the targets it runs.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SR-01 | P1: The solver's suite runs alone | Tasks | In Tasks |
| SR-02 | P1: The retired crates and the VAMS suite are gone | Tasks | In Tasks |
| SR-03 | P1: No Verilog-AMS-only surface that is safe to remove | Tasks | In Tasks |
| SR-04 | P1: Three crates with a boundary the compiler enforces | Tasks | In Tasks |
| SR-05 | P1: Nothing changed | Tasks | In Tasks |
| SR-06 | P2: The repository says what it now is | Tasks | In Tasks |
| SR-07 | P1: The hygiene guard keeps enforcing | Tasks | In Tasks |

**Coverage:** 7 total, 7 mapped to tasks, 0 unmapped.

---

## Sources

- `.specs/features/solver-completeness/context.md`, "User answers" rounds 1 and 2: Q1 (three crates), Q3 to Q6 (removals, ported tests), Q10 (crates removed and kept), the split into (A) structure and (B) logic.
- `.specs/features/solver-completeness/analysis.md` section 3 (surplus S1 to S14) and section 7 (which feature removes which item).
- `.specs/STATE.md` MD-01 (one `Element` ABI), MD-17 (`prelude` + `abi`), MD-28 (test placement), MD-31 (an invariant lives in a gate), MD-33 (no file-scope lint suppression).
- `CLAUDE.md`: "zero warnings is the bar"; never `--workspace` while other crates are broken.

---

## Success Criteria

- [ ] Three green commands: `cargo test -p piperine-solver`, `cargo test -p piperine-analog-solver`, `cargo test -p piperine-digital-solver`.
- [ ] `ls crates` shows eight directories; `cargo metadata --no-deps` succeeds.
- [ ] `parity_baseline.rs` is untouched and green.
- [ ] The verifier finds no assertion weakened and no behavior changed.

# Analog Operators Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/analog-operators/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piperine-parity -p piperine-solver -p piperine-analog-solver -p piperine-api -p piperine -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run numerics**: `cargo test -p piperine-analog-solver --test noise_numerics`
**Run report**: `cargo test -p piperine-solver --test noise_report`
**Run abinoise**: `cargo test -p piperine-solver --test abi_noise`
**Run retake**: `cargo test -p piperine-solver --test jump_retake`
**Run digital**: `cargo test -p piperine-digital-solver --test instant`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run irops**: `cargo test -p piperine-ir --test operators`
**Run planops**: `cargo test -p piperine-plan --test operators_shape`
**Run tops**: `cargo test -p piperine-transpile --test operators`
**Run tgolden**: `cargo test -p piperine-transpile --test golden`
**Run frozen**: `cargo test -p piperine-interp --test frozen`
**Run typeops**: `cargo test -p piperine-lang --test types_analog_operators`
**Run elabops**: `cargo test -p piperine-lang --test elab_operators`
**Run subset**: `cargo test -p piperine-lang --test types_subset`
**Run spartial**: `cargo test -p piperine-parity --test source_partial`
**Run sdelay**: `cargo test -p piperine-parity --test source_delay`
**Run snoise**: `cargo test -p piperine-parity --test source_noise`
**Run ssession**: `cargo test -p piperine-parity --test source_session`
**Run straces**: `cargo test -p piperine-parity --test source_traces`
**Run sslew**: `cargo test -p piperine-parity --test source_slew`
**Run partran**: `cargo test -p piperine-parity --test parity_tran`
**Run parac**: `cargo test -p piperine-parity --test parity_ac`
**Run parop**: `cargo test -p piperine-parity --test parity_op`
**Run parnoise**: `cargo test -p piperine-parity --test parity_noise`
**Run corpus**: `cargo test -p piperine-parity --test parity_corpus`
**Run contract**: `cargo test -p piperine-parity --lib contract`
**Run apinoise**: `cargo test -p piperine-api --test noise`
**Run apioptions**: `cargo test -p piperine-api --test options`
**Run apisession**: `cargo test -p piperine-api --test session_parameters`
**Run apinatures**: `cargo test -p piperine-api --test natures`
**Run apitraces**: `cargo test -p piperine-api --test traces`
**Run clianalyses**: `cargo test -p piperine-cli --test analyses`
**Run clioptions**: `cargo test -p piperine-cli --test options`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run inventory**: `cargo test -p piperine --test book_inventory`

Executors: every task is Sonnet (user rule: "Implementação sempre em Sonnet e verificação em Opus"); a different Opus verifies the feature. The worktree is `/home/keslarek/Git/piperine-analog-operators`, branch `feature/analog-operators`. Every cargo command runs with `CARGO_TARGET_DIR=/var/tmp/piperine-target-b CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-b`, in the foreground; never `pkill` by pattern; where parallel worktrees share the target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources. If `df /` passes 90 %, stop and report.

The tasks follow the spec's Q1 to Q6, all their recommended option (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"), and the defaults D1 to D20 of `design.md` "Decisions taken (user AFK)". For the record, another answer would have changed: Q1 (b) drops the totals of T3 and T13 and the `sqrt` of the densities; Q2 (b) adds one column per group to T14; Q3 (b) renames the keys of T11; Q4 (b) or (c) changes the session set of T36; Q5 (b) drops the subscription filter of T38; Q6 (b) changes the `tnom` default of T15.

**Scheduling.** Batches B1, B2 and B3 (phases 1 to 4) touch only the solver crates, `docs/spec/part_v_execution.md`, `piperine-api`, `piperine-cli` and new test targets: no file `composite-values` or `generics` edits. They run now, while those lanes are in flight. Batches B4 to B7 (phases 5 to 8) edit `piperine-ir`, `piperine-lang`, `piperine-plan`, `piperine-transpile` and `piperine-interp`, the files both lanes rewrite: they start only after `composite-values` B10 (phase B) and `generics` B8 (phase B) are on `main` (ROADMAP item 9 precedes item 10), and T17 rebases the branch first.

Rules every task follows:

- **Parity is untouched.** The existing constants of `crates/piperine-parity/src/contract.rs`, the existing files of `tests/e2e/twins/` (`devices.msl`, `noise_rc.cir` and `noise_switch.cir` included), the existing functions of the `parity_*` targets, `crates/piperine-cli/tests/goldens/` and `crates/piperine-transpile/tests/golden/` are not edited. New twins, new contract constants (T40 only) and new test functions are appended. No tolerance is written outside `ToleranceContract`, and none is changed. A task whose change moves a twin's bytes or a generated golden stops and reports. One generated golden line moved, by T10 (761db13e), and is kept: see design.md D18.
- **Cross-lane touches are only those `design.md` names**: `piperine-analog-solver` (`math/integration.rs`, `device/assembly.rs`), `piperine-solver` (`analyses/noise.rs`, `protocol/transient.rs`, the one literal of `tests/abi_noise.rs`), `docs/spec/part_v_execution.md` §15.2. No other file of the solver crates or `piperine-abi` is edited.
- **The interpreter is frozen (R1).** `piperine-interp` gains refusal rows only (T31); none of its existing targets is edited except `frozen.rs`, which gains tests.
- **Fail loud.** A form this feature does not run is `P0001` naming it, or the spec's code; never a default, a skipped statement or a value of 0.
- **Tests state what the spec says, not what the code does.** Expected values come from `docs/spec/`, the criterion's closed form or ngspice, with the derivation written beside the assertion. Test functions have exactly the names `

---

### T46: `simparam(name)` is an `Option<f64>`

**Depends on**: T45
**What**: As the decision above says. **Done when**: [x] the IR, plan, printer, elaborator and host read `simparam(name)` as an `Option<f64>`; [x] `E0456` and the `required` flag are gone; [x] the spec table of Part IV §14.4 says so.

**Tests**: integration
**Gate**: full

**Commit**: `refactor: simparam(name) is an Option<f64>`.

---

### T45: Withdraw the run `tnom`; every spice-faithful model reads its own `tnom` param

**What**: Remove the run key `tnom` and the flag `--tnom` (`piperine-api` `request.rs`, `piperine-cli`), `Tolerances::tnom` and its context plumbing (`context_of`), the `tnom` rows of Part V and Appendix C, AO-08, and the Book rows of the key table; each `headers/spice` model that reads the solver's nominal temperature reads its own param `tnom: f64 = 300.15` instead; the ngspice twins keep their numbers (unset is 300.15 K, as ngspice). `tnom_res` (T43) becomes a twin of a per-instance `tnom` param. T15's tests are rewritten or removed with the key, the commit says which and why. Per user decision 2026-11-23.
**Where**: `crates/piperine-api`, `crates/piperine-cli`, `crates/piperine-analog-solver/src/tolerances.rs`, `crates/piperine-lang/std` and spice models, `docs/spec`, `docs/book`
**Depends on**: T36
**Requirement**: AO-08 (withdrawn)
**Executor**: Sonnet

**Done when**:

- [x] no `tnom` key or flag exists and a run table naming it is `P0010`
- [x] every spice-faithful model has its own `tnom` param and the parity twins pass unchanged
- [x] Gate check passes: Build, the touched crates' tests, `Run twins`, `Run parop`

**Tests**: integration
**Gate**: full

**Commit**: `refactor: tnom is a per-instance model param, not a run key`

---

## Checks` cites.
- **No existing test is removed, weakened or skipped.** A test that asserts a refusal this feature lifts (the rows of `types_subset.rs`, the `L47` assertion of `docs.rs`) is rewritten to assert the new behaviour, and its commit says which test and why. A struct literal that gains a field gains it as its "absent" value, with no assertion changed.
- **AGENTS.md idiom rules and the code-clarity rules bind** (`.specs/features/code-clarity/spec.md`): no free function (every function has an owner type), no macro of your own, modules named after what they do, no abbreviated names, one purpose per type, no god method, every new struct, enum and method doc-commented, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run**: it rewrites `.specs/lessons.json` and `.specs/LESSONS.md`. If it is run, `git status` afterwards and restore both files.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 placement), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `crates/piperine-parity/tests/parity_corpus.rs` (the twin corpus guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits and their guards), MD-72 (the Book's inventory), `.specs/features/code-clarity/spec.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Analog solver (`piperine-analog-solver`) | integration | the power-law integral on closed forms; each new assembly read against a hand-built device | `crates/piperine-analog-solver/tests/noise_numerics.rs` | `Run numerics` |
| Solver (`piperine-solver`) | integration | the band total, `inoise`, each output form, the per-source series; the jump retake and its bound | `crates/piperine-solver/tests/{noise_report,jump_retake}.rs` | `Run report`, `Run retake` |
| The spec's text | integration | the rewritten §15.2 rule 4, word for word | `crates/piperine-lang/tests/spec_text.rs` | `Run spectext` |
| IR (`piperine-ir`) | integration | each new form built, checked (every new invariant and its violation), printed and round-tripped; the differentiator 1:1 to §14.2.3's table | `crates/piperine-ir/tests/operators.rs` | `Run irops` |
| Plan (`piperine-plan`) | integration | each new pin, term, input, watch and function in the shape; a body with no new form keeps its shape | `crates/piperine-plan/tests/operators_shape.rs` | `Run planops` |
| Printer (`piperine-transpile`) | integration | each new form printed, built and evaluated against its hand-derived value; each fault; the goldens byte-identical | `crates/piperine-transpile/tests/operators.rs`, `golden.rs` | `Run tops`, `Run tgolden` |
| Interpreter (`piperine-interp`) | integration | one refusal per new form, by name | `crates/piperine-interp/tests/frozen.rs` | `Run frozen` |
| Type checker (`piperine-lang/src/types`) | integration | 1:1 to the spec's typing ACs; `E0450`, `E0451` named | `crates/piperine-lang/tests/types_analog_operators.rs`, `types_subset.rs` | `Run typeops`, `Run subset` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; every refusal and code (`E0452`, `E0603`, `P0001`) by name | `crates/piperine-lang/tests/elab_operators.rs` | `Run elabops` |
| Runs from source (`piperine-parity` `Project`) | e2e | every runtime AC on the Rust backend, values from the spec's closed forms | `crates/piperine-parity/tests/source_{partial,delay,noise,session,traces,slew}.rs`, fixtures `tests/operators/*.msl` | `Run spartial` and siblings |
| ngspice twins | e2e | the new twins against ngspice under the contract; the existing twins unchanged | `crates/piperine-parity/tests/parity_*.rs`, `tests/e2e/twins/` | `Run partran`, `Run parac`, `Run parop`, `Run parnoise`, `Run corpus` |
| Host library (`piperine-api`) | integration | the noise request, its report and groups; `tnom` and `simparam`; session parameters (no `E0456`: `simparam` is an `Option`); traces as records; nature tolerances | `crates/piperine-api/tests/{noise,options,session_parameters,traces,natures}.rs` | `Run apinoise` and siblings |
| Command line and the Book (`piperine-cli`, `docs/book`) | e2e | the keys, the flags, the noise table; the twins' bytes; every listing equals its output | `crates/piperine-cli/tests/{analyses,options,twins,book}.rs`, `tests/book/` | `Run clianalyses`, `Run clioptions`, `Run twins`, `Run clibook`, `Run inventory` |
| Records (`GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`) | integration | each claim of AO-12 AC 3 read as text | `crates/piperine-cli/tests/docs.rs` | `Run docs` |
| Feature records (`contract.md`) | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes generated code, a solver path or what a run prints | Quick, then `cargo test -p <the crate the task edits>`, then `Run twins` and `Run tgolden` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Seven batches: phases 1 and 2 (6 tasks), phase 3 (4), phase 4 (6), phase 5 (7), phase 6 (8), phase 7 (7), phase 8 (6). B1 to B3 run now; B4 to B7 wait for `composite-values` and `generics` phases A and B on `main`.

### Phase 1: Contract

```
T1
```

### Phase 2: The noise analysis in the solver

```
T1 → T2 → T3 → T5 → T6
T1 → T4 → T5
```

### Phase 3: The jump retake, and nature tolerances from source

```
T1 → T7 → T8 → T9
T1 → T10
```

### Phase 4: The noise run, `tnom` and `simparam` on the host

```
T1 → T11 → T12 → T13 → T14
T6 → T12
T11 → T15 → T16
```

### Phase 5: IR forms and the checker's rows

```
T1 → T17 → T18 → T19 → T20 → T21
T17 → T22 → T23
```

### Phase 6: Plan, printer and interpreter

```
T17 → T24 → T28
T18 → T25 → T29
T19 → T26 → T30
T20 → T27 → T30
T21 → T31
```

### Phase 7: Lowering from source

```
T21 → T32
T22 → T32
T23 → T33
T28 → T33
T13 → T34
T23 → T34
T29 → T34
T23 → T35
T30 → T35
T16 → T36
T26 → T36
T35 → T36
T23 → T37
T30 → T37
T37 → T38
```

### Phase 8: The twins and the records

```
T33 → T39
T14 → T40
T34 → T40
T40 → T41 → T42
T36 → T43
T36 → T45
T45 → T46
T9 → T44
T10 → T44
T31 → T44
T32 → T44
T38 → T44
T39 → T44
T42 → T44
T43 → T44
```

---

## Task Breakdown

### T1: Contract: the cited code and the solver facts the design rests on

**What**: `contract.md`: confirm every file, line and name `design.md` cites against `HEAD`, re-run each row of "What the code does today" as a probe and record its output, and confirm with short throwaway runs (not committed) these facts: (1) a `DELAY` term's history holds the operating point's value at time 0, so a step before `td` reads it (`device/delay.rs` `Delayed::at`, the assembly's `accept` at the operating point); (2) the noise columns pass through `DELAY` and `DDT` term responses (`noise_columns`); (3) a `Fault` returned by an analog function in a noise sweep, and by a parameter-cone function, reaches the run error with its code and the instance path; (4) the operating point and the transient drain `TRACE` records and the AC and noise sweeps write none; (5) `encode` of a `ParamValue::Real` into a flagged `F64` layout sets the known byte, and an absent `OPTIONAL` `NAMED` pin reads all zero; (6) whether the binder can confuse a `GLOBAL NAMED` pin and a `PARAM` pin of one name; (7) whether a potential branch's private flow pin carries its nature's flow `abstol` (U7); (8) which existing targets (`buck_*`, `protocol_*`, the twins) take a step that reads a jump and fires a crossing whose reaction changes what `analog` reads (the AO-13 retake would move them): count them with a probe; (9) the analog-solver `NoiseColumn` name format and whether two `NOISE` inputs may share a name. Each difference gets the task that absorbs it; a fact that does not hold is reported to the orchestrator before phase 2.
**Where**: `.specs/features/analog-operators/contract.md`
**Depends on**: None
**Reuses**: `design.md`; the probes of `GAPS.md`
**Requirement**: AO-01 to AO-13
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists each cited location as confirmed or moved, each probe with its output, and the nine facts with their evidence
- [x] No code is edited
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): analog-operators contract against the tree`

---

### T2: The power-law integral of a density over a band

**What**: `Integrator::power_law(frequencies, densities) -> f64`: the integral of a density that is a power law between successive points, `N₁·f₁/(a+1)·((f₂/f₁)^(a+1) − 1)` with `a = ln(N₂/N₁)/ln(f₂/f₁)`, `N₁·f₁·ln(f₂/f₁)` at `a = −1`, a plain `N₁·(f₂ − f₁)` where either density is 0 (D8). New target `noise_numerics.rs` proves it exact on `N = c`, `N = c/f`, `N = c·f⁻²` and on a two-pole sum against a fine trapezoid.
**Where**: `crates/piperine-analog-solver/src/math/integration.rs`
**Depends on**: T1
**Reuses**: `Integrator::trapezoid`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `the_power_law_integral_is_exact_on_power_laws` passes, with each closed form written beside its assertion
- [x] Gate check passes: `Run numerics`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(analog-solver): the power-law integral of a noise density`

---

### T3: The band total and the per-source series of the noise analysis

**What**: `NoiseAnalysisResult::output_total` (the root of `Integrator::power_law` over `out_noise_sq`, V or A RMS); `NoiseSolver::record` sums a second value for one key at one frequency instead of pushing it. The trapezoid `integrated_noise` and its test in `abi_noise.rs` stay as they are. New target `noise_report.rs`: a resistor divider's total against the closed form `√(4kT·R∥·Δf)`; two noise inputs of one instance with one name give one series equal to their sum, one value per frequency.
**Where**: `crates/piperine-solver/src/analyses/noise.rs`
**Depends on**: T2
**Reuses**: the hand-built devices of `tests/helpers/abi_devices.rs`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `the_band_total_integrates_the_density_as_ngspice_does` and `two_inputs_sharing_a_name_sum_into_one_series` pass
- [x] `Run abinoise` passes unedited
- [x] Gate check passes: `Run report`, `Run abinoise`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the noise band total and one series per source name`

---

### T4: The stimulus columns and a term's linearization in the assembly

**What**: `Assembly::stimulus_columns(op, amb, instance) -> Vec<NoiseColumn>`: one column per `STIM` input of the instance, its partials through every term's response, with its phase (`atan2(IM, RE)`) in place of a density; `Assembly::term_linearization(op, amb, instance, term) -> TermLinearization`: the term's partials against the unknowns and against each `NOISE` input, at the operating point. Tests in `noise_numerics.rs` on a hand-built source and resistor.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T1
**Reuses**: `Assembly::noise_sources`, `noise_columns`, `linear`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `a_stimulus_column_carries_its_partials_and_phase` and `a_term_linearization_carries_its_unknown_and_noise_partials` pass
- [x] Gate check passes: `Run numerics`, then `cargo test -p piperine-analog-solver`

**Tests**: integration
**Gate**: full

**Commit**: `feat(analog-solver): stimulus columns and term linearization for the noise sweep`

---

### T5: The input-referred noise

**What**: `NoiseAnalysisOptions::input_source_name`, read now: the instance path of the input; per frequency `H_in = Σ_s (z·g_s)·e^{jφ_s}` over the instance's stimulus columns (D11), `input_noise_sq = out_noise_sq / |H_in|²`, `input_total` by the power-law integral; an input with no `STIM` input fails naming it. Tests in `noise_report.rs`: an RC's `inoise` is its `onoise` over `|1/(1 + jωRC)|`.
**Where**: `crates/piperine-solver/src/analyses/noise.rs`
**Depends on**: T3, T4
**Reuses**: `Assembly::stimulus_columns` (T4), the adjoint solve
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `the_input_referred_density_divides_by_the_transfer_from_the_input` and `an_input_with_no_stimulus_is_refused_by_name` pass
- [x] Gate check passes: `Run report`, `Run abinoise`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): input-referred noise`

---

### T6: Flow and differential outputs of the noise analysis

**What**: `NoiseAnalysisOptions::output_flow: Option<FlowOutput>`: `FlowOutput::Branch` puts the adjoint's unit at the branch's flow unknown; `FlowOutput::Term` takes the term's linearization (T4) as the adjoint right side and adds its direct noise partials to each source's transfer. The differential `output_node`/`reference_node` pair is proven. The literal of `abi_noise.rs` gains `output_flow: None`. Tests in `noise_report.rs`: the current of a noisy resistor shorted to ground is `√(4kT/R)` A/√Hz; a differential output across a divider's resistor.
**Where**: `crates/piperine-solver/src/analyses/noise.rs` (and the one literal of `crates/piperine-solver/tests/abi_noise.rs`)
**Depends on**: T5
**Reuses**: `Assembly::term_linearization` (T4)
**Requirement**: AO-05 (edge case: a flow output)
**Executor**: Sonnet

**Done when**:

- [x] `a_flow_output_reads_amperes_per_root_hertz` and `a_differential_output_reads_the_pair` pass
- [x] Gate check passes: `Run report`, `Run abinoise`, then `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): flow and differential noise outputs`

---

### T7: §15.2 rule 4: a crossing made by a jump retakes the step that read it

**What**: Rewrite Part V §15.2 dynamic rule 4 to design mechanism 7 (D16): the crossing still fires at the time point of the step that reads the change, after that step's solve; when its reactions change a carrier or a param that `analog` reads, the step is retaken to the same time with the change and the retaken solution replaces the first; the reactions are not run again; a crossing the retaken solution makes fires at the next microstep and may retake again; past the round bound the step fails as `E0504`. One function appended to `spec_text.rs` holds the wording.
**Where**: `docs/spec/part_v_execution.md`
**Depends on**: T1
**Reuses**: the host `set` retake paragraph of §15.2
**Requirement**: AO-13
**Executor**: Sonnet

**Done when**:

- [x] `part_v_retakes_the_step_a_jump_crossing_changes` passes
- [x] Gate check passes: `Run spectext`

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): a crossing made by a jump retakes the step that read it`

---

### T8: The owner retakes the step that read a jump

**What**: `JumpRetake` in the owner's transient loop: it knows whether the step being taken reads a delivery; after the instant, when the step read a jump, a crossing of this step fired and the instant's boundary is not empty, it restores the analog side to the step's checkpoint, keeps the discrete side and the sink, delivers the boundary with `Restart::Integrator` and attempts the same time again, exempt; crossings the retaken solution makes and the first did not fire at the next microstep; more retakes than `max_rounds` fail as `E0504` naming the instant. New target `jump_retake.rs` on hand-built devices (a slew-like follower whose mode a crossing of its own rate sets).
**Where**: `crates/piperine-solver/src/protocol/transient.rs`
**Depends on**: T7
**Reuses**: `Protocol::checkpoint`, `restore`, `AnalogSide::deliver`
**Requirement**: AO-13
**Executor**: Sonnet

**Done when**:

- [x] `a_crossing_made_by_a_jump_retakes_the_step`, `a_step_that_reads_no_jump_never_retakes` and `endless_retakes_fail_with_e0504` pass
- [x] every `buck_*` target of `piperine-parity`, `Run twins` and every `protocol_*` target of `piperine-solver` pass unedited (a target that moves stops the task)
- [x] Gate check passes: `Run retake`, then `cargo test -p piperine-solver`, then Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): retake the step that read a jump when its crossings change what analog reads`

---

### T9: `slew` limits an ideal step, from source

**What**: New target `source_slew.rs` with the fixture `tests/operators/slew_step.msl`: a source whose value is a `Bit<f64>` set from 0 to 1 by `at(1u)`, into the stdlib `slew(v(in), 1M, 1M)`. The output is 0 V before 1 µs, `t − 1 µs` V/µs between, 1 V from 2 µs, within `TRAN_VOLTAGE` (AO-13).
**Where**: `crates/piperine-parity/tests/source_slew.rs`
**Depends on**: T8
**Reuses**: `Project`, `ToleranceContract::TRAN_VOLTAGE`
**Requirement**: AO-13
**Executor**: Sonnet

**Done when**:

- [x] `slew_limits_an_ideal_step_to_its_rate` passes
- [x] Gate check passes: `Run sslew`

**Tests**: e2e
**Gate**: quick

**Commit**: `test(parity): slew limits an ideal step`

---

### T10: Nature tolerances from source (U7)

**What**: New target `crates/piperine-api/tests/natures.rs`: a node of a nature whose potential `abstol` is 1m, a branch of a nature whose flow `abstol` is 1n, a compatible `HvElectrical` meeting `Electrical` at one node, each read from the bound circuit (AO-10 AC 1 to 3); a divider of one resistor of each nature equals the all-`Electrical` divider within `DC_VOLTAGE`; natures whose flow names differ at a port are `E0251` (AC 4). When T1 found a branch flow pin without its nature's flow `abstol` (D18), the pin gets it here (`piperine-plan/src/plan.rs`), and the commit says so.
**Where**: `crates/piperine-api/tests/natures.rs`
**Depends on**: T1
**Reuses**: `Session::circuit`, the solver's per-unknown tolerances
**Requirement**: AO-10
**Executor**: Sonnet

**Done when**:

- [x] the four tests of AO-10 named in `## Checks` pass
- [x] Gate check passes: `Run apinatures`

**Tests**: integration
**Gate**: quick

**Commit**: `test(api): a user nature's abstol and compatible natures from source`

---

### T11: The noise request

**What**: `Analysis::Noise { output, input, from, to, grid }` in `RunSpec`: `analysis = "noise"` reads `output`, `input`, `from`, `to`, `per_decade` and `points` (Q3), the grid rules of `ac`; `P0010` for a missing `output` or sweep and for a key of another analysis (AO-05 AC 4, 6); `Analysis::NAMES` lists `noise`; `--output` and `--input` win over the keys (AC 3). Tests in the new target `crates/piperine-api/tests/noise.rs`.
**Where**: `crates/piperine-api/src/request.rs`
**Depends on**: T1
**Reuses**: the `tf` and `ac` arms of `RunSpec::spec`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `a_noise_run_without_output_or_sweep_is_p0010`, `a_key_of_another_analysis_is_p0010_in_a_noise_run` and `the_output_and_input_flags_win_over_the_keys` pass
- [x] Gate check passes: `Run apinoise`, then `cargo test -p piperine-api`

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): the noise request`

---

### T12: The output and the input of a noise run

**What**: New `crates/piperine-api/src/noise.rs`, `NoiseProbe`: `output` is a node, `v(a, b)` or `i(<instance>)` (D10, the grammar of `Probe::resolve`), resolved to the solver's output node pair or `FlowOutput`; `input` is an instance whose body has an `ac_stim`; anything else is `E0801` naming the path, before anything compiles (AO-05 AC 5).
**Where**: `crates/piperine-api/src/noise.rs`
**Depends on**: T11, T6
**Reuses**: `Probe::resolve`, `Session::small_signal_of`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `an_output_or_input_that_names_nothing_is_e0801` passes
- [x] Gate check passes: `Run apinoise`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): resolve the output and input of a noise run`

---

### T13: The noise run in a session, its report and its table

**What**: `Session` runs `Analysis::Noise` through `NoiseSolver`; `NoiseReport` (in `noise.rs`) holds the densities as `sqrt` of the solver's powers, the totals, the unit (V/√Hz or A/√Hz) and each group's density (a source's name, else its instance path: Q2, D7); `Table::noise`: columns `freq`, `onoise`, and `inoise` with an input; `Table::summary` gains the lines `onoise_total` and `inoise_total` (Q1). A design with no noise source prints `onoise` = 0 and succeeds. Tests in `tests/noise.rs` on resistor and capacitor designs (densities 0; the shape of the table, the summary and the report).
**Where**: `crates/piperine-api/src/session.rs` (with `table.rs`)
**Depends on**: T12
**Reuses**: the `Ac` arm of `Session::analysis`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `a_noise_run_prints_freq_onoise_and_the_total`, `an_input_adds_inoise_and_its_total`, `the_library_returns_rows_totals_and_group_densities` and `a_design_with_no_noise_source_prints_zero_and_succeeds` pass
- [x] Gate check passes: `Run apinoise`, then `cargo test -p piperine-api`

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): run a noise analysis and report it`

---

### T14: The noise run on the command line

**What**: `piperine run --analysis noise --output .. [--input ..] --from .. --to .. --per-decade ..`: the text table prints the rows, then each summary line as `name  value`; CSV writes the rows only. Tests appended to `crates/piperine-cli/tests/analyses.rs`.
**Where**: `crates/piperine-cli/src/render/table.rs`
**Depends on**: T13
**Reuses**: `TextTable`
**Requirement**: AO-05
**Executor**: Sonnet

**Done when**:

- [x] `a_noise_run_prints_one_row_per_frequency_then_the_total` passes
- [x] Gate check passes: `Run clianalyses`, then `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `feat(cli): print a noise run`

---

### T15: The `tnom` session option

**What**: The key `tnom` and the flag `--tnom` (°C, 27 when absent: Q6); `P0010` for a value not above −273.15 (AO-08 AC 1, 2). `Options::tnom`, `Overrides::tnom`. Tests appended to `crates/piperine-api/tests/options.rs` and `crates/piperine-cli/tests/options.rs`.
**Where**: `crates/piperine-api/src/request.rs` (and the flag in `crates/piperine-cli/src/cli.rs`)
**Depends on**: T11
**Reuses**: the `temp` key and its check
**Requirement**: AO-08
**Executor**: Sonnet

**Done when**:

- [x] `tnom_key_and_flag_default_to_27` and `tnom_at_or_below_absolute_zero_is_p0010` pass
- [x] Gate check passes: `Run apioptions`, `Run clioptions`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api,cli): the tnom session option`

---

### T16: The `simparam` table and flag

**What**: The key `simparam = { name = value }` and the repeatable flag `--simparam <name>=<value>`, a flag entry winning over the table's entry of the same name; `P0010` for a value that is not a number, and for an entry naming `reltol`, `abstol`, `vntol`, `trtol`, `temp` or `tnom` (D13) (AO-07 AC 5). `Options::session_parameters`. Tests appended to `tests/options.rs` of both crates.
**Where**: `crates/piperine-api/src/request.rs` (and the flag in `crates/piperine-cli/src/cli.rs`)
**Depends on**: T15
**Reuses**: the `params` key's entries
**Requirement**: AO-07
**Executor**: Sonnet

**Done when**:

- [x] `simparam_flag_is_repeatable_and_a_non_number_is_p0010` and `a_simparam_entry_naming_an_option_is_p0010` pass
- [x] Gate check passes: `Run apioptions`, `Run clioptions`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api,cli): the simparam table and flag`

---

### T17: Rebase, re-anchor, and delay sites in the IR

**What**: First, once `composite-values` B10 and `generics` B8 are on `main`: `git fetch`, `git rebase main`, re-read every location `design.md` cites in the IR, the front end, the plan and the printer, and append what moved to `contract.md`. Then `DelaySite { name, input, time, maximum, origin }`, `BodyData::delays`, `Expr::Delay(k)` (appended last); `Ir::check` names a `k` out of range and a `Delay` read in a function; `deps::Target::{DelayInput, DelayTime, DelayMaximum}`; printed `delay<k>(x, td, max)` and round-tripped. New target `crates/piperine-ir/tests/operators.rs`.
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`, `build.rs`, `handle.rs`)
**Depends on**: T1
**Reuses**: `Derivative` and its walk
**Requirement**: AO-02
**Executor**: Sonnet

**Done when**:

- [x] the branch is rebased on `main` with both lanes' phases A and B merged, and `contract.md` records the moved anchors
- [x] `a_delay_site_is_built_checked_printed_and_round_tripped` passes
- [x] Gate check passes: `Run irops`, then `cargo test -p piperine-ir`

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): delay sites`

---

### T18: Noise sources in the IR

**What**: `NoiseSource { name, group, density, origin }`, `BodyData::noise_sources`, `Expr::Noise(k)`; `Ir::check` names a `k` out of range and a `Noise` read in a function; `deps::Target::NoiseDensity`, `Deps::noise_sources` (a differentiable read, like `stims`); printed `noise<k>(psd, "name")`.
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`, `build.rs`, `handle.rs`)
**Depends on**: T17
**Reuses**: `Site` for `ac_stim`
**Requirement**: AO-03
**Executor**: Sonnet

**Done when**:

- [x] `a_noise_source_is_built_checked_printed_and_round_tripped` passes
- [x] Gate check passes: `Run irops`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): noise sources`

---

### T19: Session parameters in the IR

**What**: `SessionParameter { name, origin }`, `BodyData::session_parameters` (one per name), `Expr::SessionParameter { parameter, default }` (an `f64`; `default` an `f64` expression on the parameter cone); `Deps::session_parameters` (a discrete read, constant during an analysis); printed `simparam("name", default)`.
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`, `build.rs`, `handle.rs`)
**Depends on**: T18
**Reuses**: `Env::Temperature`'s reads
**Requirement**: AO-07
**Executor**: Sonnet

**Done when**:

- [x] `a_session_parameter_is_built_checked_printed_and_round_tripped` passes
- [x] Gate check passes: `Run irops`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): session parameters`

---

### T20: Trace sites in the IR

**What**: `TraceSite { stream, origin }`, `BodyData::traces`, `Stmt::Trace { site, fields }`; `Ir::check` allows `Trace` in the analog block only (a reaction keeps `Emit`, a function neither), the fields typed as the stream's (§19 rule 12); `deps::Target::Trace(k)`; printed `trace<k> "stream" { .. }`.
**Where**: `crates/piperine-ir/src/model.rs` (with `check.rs`, `deps.rs`, `print.rs`, `build.rs`)
**Depends on**: T19
**Reuses**: `Stmt::Emit`, `Stream`
**Requirement**: AO-09
**Executor**: Sonnet

**Done when**:

- [x] `a_trace_site_is_built_checked_printed_and_round_tripped` passes
- [x] Gate check passes: `Run irops`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): trace sites`

---

### T21: Symbolic differentiation in the IR

**What**: New `differentiate.rs`: `Differentiation::of(expr) -> Result<ExprId, NotDifferentiable>` against a `Variable` (a potential probe, a flow, a function argument), by the table of design mechanism 1 (D1, D2); derived functions memoized by `IrBuilder::derived_function`; `PartialDerivative`, `BodyData::partial_derivatives`. Tests in `operators.rs`: each row of the table against its closed form, evaluated through the IR printer's text, the reversed probe, other branches held constant, `limit` transparent, other instances' results held constant, a call, a block with `let`s, a second derivative, and a loop or an assignment refused by name.
**Where**: `crates/piperine-ir/src/differentiate.rs` (with `model.rs`, `build.rs`)
**Depends on**: T20
**Reuses**: `IrBuilder::expr`, `expr_kind`
**Requirement**: AO-01
**Executor**: Sonnet

**Done when**:

- [x] `each_form_differentiates_by_the_rule_of_its_row`, `probes_of_other_branches_are_held_constant`, `limit_is_transparent_to_ddx`, `results_of_other_instances_are_held_constant` and `a_form_with_no_rule_is_not_differentiable_by_name` pass
- [x] Gate check passes: `Run irops`, then `cargo test -p piperine-ir`

**Tests**: integration
**Gate**: full

**Commit**: `feat(ir): symbolic differentiation`

---

### T22: The checker's `ddx` row

**What**: `CORE_ROWS` gains `ddx` (`Allowed::Analog`, the placement rule of `ac_stim`), `CoreParam::Probe`: the second argument is a potential or flow probe of a branch of this module, else `E0451` (`p.inflow()` included) (AO-01 AC 7, 8). The elaborator still refuses `ddx` with `P0001` until T32. New target `crates/piperine-lang/tests/types_analog_operators.rs`; the `ddx` row of `types_subset.rs` moves to the elaborator's refusal and the commit says so.
**Where**: `crates/piperine-lang/src/types/catalogue.rs` (with `types/analog.rs`)
**Depends on**: T17
**Reuses**: `Checker::probe`, the `ac_stim` row
**Requirement**: AO-01
**Executor**: Sonnet

**Done when**:

- [x] `ddx_against_a_non_probe_is_e0451` and `ddx_outside_analog_or_read_by_a_definition_is_e0450` pass
- [x] Gate check passes: `Run typeops`, `Run subset`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the ddx row and E0451`

---

### T23: The checker's rows for `delay`, `noise`, `simparam` and `emit!` in `analog`

**What**: `CORE_ROWS` gains `delay` and `noise` (`Allowed::Instances`, as `ddt`), `simparam` (`Allowed::Everywhere`, as `temperature`); `CoreParam::Text` and `CoreParam::OptionText` take a string literal (`simparam`'s name, `noise`'s name); `types/record.rs` no longer refuses an `emit!` in `analog` (`E0603` unchanged). The elaborator refuses each with `P0001` until its lowering task. The rows of `types_subset.rs` move as in T22.
**Where**: `crates/piperine-lang/src/types/catalogue.rs` (with `types/record.rs`)
**Depends on**: T22
**Reuses**: the `ddt` and `temperature` rows
**Requirement**: AO-02, AO-03, AO-07, AO-09
**Executor**: Sonnet

**Done when**:

- [x] `delay_noise_and_simparam_type_and_wait_for_lowering` and `a_site_reached_from_analog_writing_sim_or_checks_is_e0603` pass
- [x] Gate check passes: `Run typeops`, `Run subset`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): the delay, noise and simparam rows; emit! in analog types`

---

### T24: Plan: delay sites

**What**: Per delay site: the private continuous pin `delay<k>.value` after the derivative pins (`Pins::delays`), `Term::DelayOutput(k)` (`FLOW {u, GROUND}`) and `Term::DelayHistory(k)` (`DELAY {u, GROUND}`, its `max` the pin `delay<k>.max`) after the derivative terms, `Target::DelayTime(k)` written by `terms`; the `PRIVATE` discrete `F64` pin `delay<k>.max` after the derivative `abstol` pins, written by `Mode::DelayMaximum(k)` (`td` itself without `max`, D4); `Source::Delay(k)` a differentiable read. New target `crates/piperine-plan/tests/operators_shape.rs`; a body with no delay keeps its shape.
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T17
**Reuses**: `Term::DerivativeRate`, `Mode::DerivativeAbstol`
**Requirement**: AO-02
**Executor**: Sonnet

**Done when**:

- [x] `a_delay_site_has_its_pin_terms_and_maximum` and `a_body_without_a_new_form_keeps_its_shape` pass
- [x] Gate check passes: `Run planops`, then `cargo test -p piperine-plan`, then `Run tgolden`

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): delay sites`

---

### T25: Plan: noise inputs

**What**: One `NOISE` input `noise:<k>` per source after the `STIM` inputs; `Source::NoiseValue(k)`, differentiable, after `StimValue` in read order; `Mode::Noise(k)` writes `Target::NoiseDensity(k)` (`PSD`) and reads the density's cone and `FREQ`. Tests in `operators_shape.rs`.
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T18
**Reuses**: `Mode::Stim`, `Target::StimRe`
**Requirement**: AO-03
**Executor**: Sonnet

**Done when**:

- [x] `a_noise_source_is_an_input_with_its_density_function` passes
- [x] Gate check passes: `Run planops`, `Run tgolden`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): noise inputs`

---

### T26: Plan: session parameter pins, and the netlist's session

**What**: One `GLOBAL NAMED` pin per session parameter of a body, named the parameter, with a flagged `F64` layout, `OPTIONAL` when every read gives a default (D12), after every existing pin group (`Pins::session_parameters`); `Source::SessionParameter(k)`. `Netlist::with_session_parameters(&[(String, f64)])` hands each to `CircuitBuilder::session_param`. A name that collides with a param pin of the same body is `P0001` naming both, when T1 found the binder confuses them. Tests in `operators_shape.rs`.
**Where**: `crates/piperine-plan/src/plan.rs` (with `netlist.rs`)
**Depends on**: T19
**Reuses**: the `TEMPERATURE` pin, `CircuitBuilder::session_param`
**Requirement**: AO-07
**Executor**: Sonnet

**Done when**:

- [x] `a_session_parameter_is_a_named_global_optional_with_a_default` and `the_netlist_hands_its_session_parameters_to_the_builder` pass
- [x] Gate check passes: `Run planops`, `Run tgolden`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): session parameter pins`

---

### T27: Plan: trace watches

**What**: One `TRACE` watch per trace site, after the `CAP` watches, with the stream's record layout (`RecordLayout`, its `maybe` byte included); `Target::Trace(k)` is a write of the `terms` function. Tests in `operators_shape.rs`.
**Where**: `crates/piperine-plan/src/plan.rs` (with `digital.rs`)
**Depends on**: T20
**Reuses**: `RecordLayout`, `Pins::caps`
**Requirement**: AO-09
**Executor**: Sonnet

**Done when**:

- [x] `a_trace_site_is_a_trace_watch_written_by_terms` passes
- [x] Gate check passes: `Run planops`, `Run tgolden`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(plan): trace watches`

---

### T28: Printer: delay sites

**What**: New `print/delay.rs`, `DelayPrinter`: the two delay terms in `terms_block`, the `TIME` write `min(td, max)` (D5), `Fault` 453 for `td < 0` from `terms` and for `max ≤ 0` from `delay:<k>.max`; `Expr::Delay` reads the pin. New target `crates/piperine-transpile/tests/operators.rs`: built and evaluated against hand-derived values.
**Where**: `crates/piperine-transpile/src/print/delay.rs`
**Depends on**: T24
**Reuses**: `print/derivative.rs`
**Requirement**: AO-02
**Executor**: Sonnet

**Done when**:

- [x] `the_delay_terms_clamp_td_to_max` and `the_delay_function_faults_453` pass
- [x] Gate check passes: `Run tops`, `Run tgolden`, then `Run twins`

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): delay sites`

---

### T29: Printer: noise sources

**What**: New `print/noise.rs`, `NoisePrinter`: `noise:<k>` writes `PSD` and returns `Fault` 454 naming the source and the frequency for a negative or non-finite density; `Expr::Noise` reads the input's `VALUE` in `analog` and prints `0.0` in a reaction or a guard. Tests in `operators.rs`.
**Where**: `crates/piperine-transpile/src/print/noise.rs`
**Depends on**: T25
**Reuses**: `print/site.rs` (`stim:<k>`)
**Requirement**: AO-03
**Executor**: Sonnet

**Done when**:

- [x] `the_noise_function_writes_its_density_and_faults_454` and `a_reaction_reads_noise_as_zero` pass
- [x] Gate check passes: `Run tops`, `Run tgolden`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): noise sources`

---

### T30: Printer: session parameters and traces

**What**: `ValuePrinter` prints `Expr::SessionParameter` as the pin's `known_or(default)` (`known_or(0.0)` with no default); new `print/trace.rs`, `TracePrinter`: a `Stmt::Trace` writes its watch's record bytes where it stands in `terms_block`, the `maybe` byte false. Tests in `operators.rs`.
**Where**: `crates/piperine-transpile/src/print/trace.rs` (with `print/value.rs`)
**Depends on**: T26, T27
**Reuses**: the record printing of `print/reaction.rs`
**Requirement**: AO-07, AO-09
**Executor**: Sonnet

**Done when**:

- [x] `a_session_parameter_reads_its_pin_or_its_default` and `a_trace_writes_its_watch_only_where_it_executes` pass
- [x] Gate check passes: `Run tops`, `Run tgolden`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(transpile): session parameters and traces`

---

### T31: The interpreter refuses every new form by name

**What**: New `crates/piperine-interp/src/operators.rs`, `OperatorForms`: rows `ddx` (`partial_derivatives`), `delay`, `noise`, `simparam`, `emit! in analog`; chained into `Interpreter::new` beside `amended_forms` and `structure_forms`. Tests appended to `frozen.rs`.
**Where**: `crates/piperine-interp/src/operators.rs`
**Depends on**: T21
**Reuses**: `Interpreter::structure_forms`
**Requirement**: AO-11
**Executor**: Sonnet

**Done when**:

- [x] `the_interpreter_refuses_each_analog_operator_by_name` passes
- [x] Gate check passes: `Run frozen`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(interp): refuse the analog operators by name`

---

### T32: `ddx` from source

**What**: New `elab/partial.rs`: `ddx(e, probe)` lowers `e` standalone and differentiates it (T21); a `NotDifferentiable` is `P0001` naming the form; a `PartialDerivative` is recorded. New target `crates/piperine-lang/tests/elab_operators.rs`; new target `crates/piperine-parity/tests/source_partial.rs` with fixtures under `tests/operators/`, every runtime AC of AO-01 on the Rust backend against its closed form.
**Where**: `crates/piperine-lang/src/elab/partial.rs`
**Depends on**: T21, T22
**Reuses**: the standalone lowering of `elab/derivatives.rs`
**Requirement**: AO-01
**Executor**: Sonnet

**Done when**:

- [x] the `spartial` tests of AO-01 named in `## Checks` pass, and `ddx_of_an_expression_that_reads_no_probe_is_zero`
- [x] Gate check passes: `Run elabops`, `Run spartial`, then Full

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): ddx`

---

### T33: `delay` from source

**What**: New `elab/delays.rs`: one site per `delay` call per frame (`Frame::delays`); `x`, `td`, `max` standalone through `known_in_analog` (`E0150`); `E0452` for a `td` that is not constant during an analysis without `max`, naming the placement; a `max` off the parameter cone is `P0001` (D6). When T1 found the operating point missing from the history, the assembly's `accept` gains it here and the commit says so. New target `source_delay.rs`.
**Where**: `crates/piperine-lang/src/elab/delays.rs`
**Depends on**: T23, T28
**Reuses**: `Frame::derivatives`, `known_in_analog`
**Requirement**: AO-02
**Executor**: Sonnet

**Done when**:

- [x] the `sdelay` and `elabops` tests of AO-02 named in `## Checks` pass, and `a_delay_reading_an_unknown_value_is_e0150`
- [x] Gate check passes: `Run elabops`, `Run sdelay`, then Full

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): delay`

---

### T34: `noise`, `white_noise` and `flicker_noise` from source

**What**: New `elab/noise.rs`: one source per `noise` call per frame (`Frame::noise_sources`); the density standalone through `known_in_analog`; the name a literal after inlining, else `P0001`; the stdlib `white_noise` and `flicker_noise` run; `noise_table` stays refused by name (D17). New target `source_noise.rs` running noise sweeps through `piperine-api` (T13) against `4kT/R` and the other closed forms.
**Where**: `crates/piperine-lang/src/elab/noise.rs`
**Depends on**: T13, T23, T29
**Reuses**: `Frame::derivatives`, the hoisting of result modules
**Requirement**: AO-03, AO-04, AO-05
**Executor**: Sonnet

**Done when**:

- [x] the `snoise` and `elabops` tests of AO-03, AO-04 and AO-05 AC 7 named in `## Checks` pass, and `a_noise_density_reading_an_unknown_value_is_e0150`
- [x] Gate check passes: `Run elabops`, `Run snoise`, then Full

**Tests**: e2e
**Gate**: full

**Commit**: `feat(elab): noise sources and the stdlib noise modules`

---

### T35: `simparam` from source

**What**: New `elab/session.rs`: `simparam("name", default)` lowers to `Expr::SessionParameter`, one `SessionParameter` per name per body; allowed in every run-time position, definitions and selectors included (§14.4.1). Tests in `elab_operators.rs`.
**Where**: `crates/piperine-lang/src/elab/session.rs`
**Depends on**: T23, T30
**Reuses**: the lowering of `temperature()`
**Requirement**: AO-07
**Executor**: Sonnet

**Done when**:

- [x] `simparam_lowers_to_one_session_parameter_per_name` passes
- [x] Gate check passes: `Run elabops`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): simparam`

---

### T36: The session's parameters reach the circuit; `E0456`

**What**: `Session` builds through `Netlist::with_session_parameters` whenever the design reads one: `reltol`, `abstol`, `vntol`, `trtol` (the run's, else the solver's defaults), `temp` and `tnom` in °C, then the `simparam` entries (D13); a read with no default of a name the session lacks is `E0456` naming it, before binding. New target `crates/piperine-api/tests/session_parameters.rs`; new target `crates/piperine-parity/tests/source_session.rs`.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T16, T26, T35
**Reuses**: `Session::netlist_of`, `Session::context_of`
**Requirement**: AO-07, AO-08
**Executor**: Sonnet

**Done when**:

- [x] the `ssession` and `apisession` tests of AO-07 and AO-08 named in `## Checks` pass
- [x] Gate check passes: `Run apisession`, `Run ssession`, then Full

**Tests**: e2e
**Gate**: full

**Commit**: `feat(api): session parameters reach the circuit; E0456`

---

### T37: `emit!` in `analog` from source

**What**: New `elab/traces.rs`: an `emit!` in the analog block lowers to `Stmt::Trace` where it stands, one `TraceSite` per site; its stream and fields follow §19 rules 11 and 12 as a reaction's; an `emit!` reached from `analog` through a `fn` is `P0001` naming the `fn` (D14). Tests in `elab_operators.rs`.
**Where**: `crates/piperine-lang/src/elab/traces.rs`
**Depends on**: T23, T30
**Reuses**: `elab/records.rs`
**Requirement**: AO-09
**Executor**: Sonnet

**Done when**:

- [x] `an_emit_in_analog_lowers_to_a_trace_site` and `a_field_type_conflict_across_trace_sites_is_e0605` pass
- [x] Gate check passes: `Run elabops`

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): emit! in analog is a trace`

---

### T38: Traces reach the host as records

**What**: `Record` gains `iteration: Option<u32>` and `analysis`; `Observation::Trace` decodes to a record of its stream, with its site's text fields, the instance path, the instant and the Newton iteration; only a stream named in `streams` is recorded (Q5, D15). New target `crates/piperine-api/tests/traces.rs`; new target `crates/piperine-parity/tests/source_traces.rs` over a diode whose Newton path is known.
**Where**: `crates/piperine-api/src/recorder.rs` (with `record.rs`)
**Depends on**: T37
**Reuses**: the decoding of reaction records
**Requirement**: AO-09
**Executor**: Sonnet

**Done when**:

- [x] the `straces` and `apitraces` tests of AO-09 named in `## Checks` pass
- [x] Gate check passes: `Run apitraces`, `Run straces`, then Full

**Tests**: e2e
**Gate**: full

**Commit**: `feat(api): traces are records of their stream`

---

### T39: The `delay_tline` twin

**What**: New `tests/e2e/twins/delay_tline.cir` (a sine source with `AC 1`, 50 Ω, a lossless `T` of `Z0` = 50 Ω and `TD` = 1 µs, 50 Ω load; a `tran` and an `ac` analysis), `delay_tline.msl` (`v(out) +> 0.5 * delay(v(src), 1u);` with the same source), `.command`, `.stdout`; `fn delay_tline()` appended to `parity_tran.rs` (TRAN_VOLTAGE at every print row) and `fn delay_tline_ac()` to `parity_ac.rs` (`0.5·exp(−jω·1 µs)` under `AC_MAGNITUDE` and `AC_PHASE`); the twin listed in `COMPILED_ONLY` of `crates/piperine-cli/tests/twins.rs`.
**Where**: `tests/e2e/twins/delay_tline.msl` (with its `.cir`, `.command`, `.stdout`, and the appended functions)
**Depends on**: T33
**Reuses**: `ac_rc`'s and `tran_rc`'s comparison functions
**Requirement**: AO-02
**Executor**: Sonnet

**Done when**:

- [x] `delay_tline` and `delay_tline_ac` pass with ngspice present
- [x] Gate check passes: `Run partran`, `Run parac`, `Run corpus`, `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the delay_tline twin`

---

### T40: The noise contract, the harness and the `noise_rc` twin

**What**: `ToleranceContract::NOISE` (reltol 1e-3, abstol 1e-20, its rationale: a linear solve per frequency at a frozen operating point, totals integrated by ngspice's rule, the floor far under any density the twins print) (D9), with its own appended unit test; the harness reads the `wrdata` pair (`<twin>_psd`, `<twin>_total`) of a `.noise` run; new target `parity_noise.rs`; `noise_rc.msl` (the `.cir`'s circuit, its resistor carrying `white_noise(4.0 * K_B * temperature() / r, "thermal")`), `.command` (`--temp 27`), `.stdout`; `fn noise_rc()` compares `onoise` at every frequency and `onoise_total`; `noise_rc` in `COMPILED_ONLY`. A total that does not agree stops the task and is reported: no second constant without the orchestrator.
**Where**: `crates/piperine-parity/tests/parity_noise.rs` (with `src/contract.rs`, `src/golden.rs`, `tests/e2e/twins/noise_rc.msl`, `.command`, `.stdout`)
**Depends on**: T14, T34
**Reuses**: `Ngspice::run`, `Golden`
**Requirement**: AO-06, AO-12
**Executor**: Sonnet

**Done when**:

- [x] `noise_rc`, `the_noise_contract_carries_its_bound_and_rationale` and `noise_twins_run_and_print_the_skip_line_without_ngspice` pass; `the_five_contracts_carry_bounds_and_rationale` passes unedited
- [x] Gate check passes: `Run parnoise`, `Run contract`, `Run corpus`, `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the noise contract and the noise_rc twin`

---

### T41: The `noise_switch` twin

**What**: `noise_switch.msl`: the `.cir`'s circuit, a resistive switch whose resistance follows its control (`ron` 1 Ω above `vt` 1 V, `roff` 10 kΩ below) and whose thermal source follows that resistance, and a noisy `R1`; `.command`, `.stdout`; `fn noise_switch()` in `parity_noise.rs`; listed in `COMPILED_ONLY`.
**Where**: `tests/e2e/twins/noise_switch.msl` (with `.command`, `.stdout`, the appended function)
**Depends on**: T40
**Reuses**: `noise_rc`'s comparison
**Requirement**: AO-06
**Executor**: Sonnet

**Done when**:

- [x] `noise_switch` passes with ngspice present
- [x] Gate check passes: `Run parnoise`, `Run corpus`, `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the noise_switch twin`

---

### T42: The `noise_diode` twin

**What**: New `noise_diode.cir`: a diode with `kf`, `af` biased by a current source with `AC 1`, `.noise v(a) I1 ..`, writing `onoise_spectrum`, `onoise_total`, `inoise_spectrum` and `inoise_total`; `noise_diode.msl`: a diode carrying `white_noise(2.0 * Q * id.abs(), "shot")` and `flicker_noise(kf * id.abs().powf(af), 1.0, "flicker")`; `.command` with `--input i1`, `.stdout`; `fn noise_diode()` compares `onoise`, `inoise` and both totals, with the density at one frequency derived by hand beside it; listed in `COMPILED_ONLY`.
**Where**: `tests/e2e/twins/noise_diode.msl` (with its `.cir`, `.command`, `.stdout`, the appended function)
**Depends on**: T41
**Reuses**: `noise_rc`'s comparison
**Requirement**: AO-06, AO-05
**Executor**: Sonnet

**Done when**:

- [x] `noise_diode` passes with ngspice present
- [x] Gate check passes: `Run parnoise`, `Run corpus`, `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the noise_diode twin`

---

### T43: The `tnom_res` twin

**What**: New `tnom_res.cir` (`temp_res`'s divider with `.options tnom=50 temp=27`), `tnom_res.msl` (the resistor `r * (1.0 + tc1 * (temperature() - tnom))` with its own per-instance param `tnom: f64 = 300.15`, set to 323.15 on the instance: T45, no run key), `.command` (`--temp 27`), `.stdout`; `fn tnom_res()` appended to `parity_op.rs` under `DC_VOLTAGE`. The twin reads no `simparam` and no `analysis`, so both backends run it and it is not in `COMPILED_ONLY`.
**Where**: `tests/e2e/twins/tnom_res.msl` (with its `.cir`, `.command`, `.stdout`, the appended function)
**Depends on**: T36
**Reuses**: `temp_res`'s comparison
**Requirement**: AO-08
**Executor**: Sonnet

**Done when**:

- [x] `tnom_res` passes with ngspice present
- [x] Gate check passes: `Run parop`, `Run corpus`, `Run twins`

**Tests**: e2e
**Gate**: full

**Commit**: `test(parity): the tnom_res twin`

---

### T44: The records

**What**: `GAPS.md` drops L43, L44, L45 (but `noise_table`, kept with its three dependencies), L49, `simparam` and `tnom` of L47, L60, the noise row of H2 and U7, and adds the D14 row (an `emit!` reached from `analog` through a `fn`); `ROADMAP.md` marks `analog-operators` delivered; `.specs/STATE.md` gains the decision of `design.md` with the next free `MD-` number; the Book's command-line chapter shows one noise run (`data-cli-output`, held to the binary), the `noise`, `tnom` and `simparam` rows of its key table, and the inventory rows of `tests/book/inventory.tsv` follow; `crates/piperine-cli/tests/docs.rs` gains `gaps_and_roadmap_follow_analog_operators` and its `L47` assertion is rewritten (L47 closed), which the commit says.
**Where**: `GAPS.md` (with `ROADMAP.md`, `.specs/STATE.md`, `docs/book/content.html`, `tests/book/inventory.tsv`, `crates/piperine-cli/tests/docs.rs`)
**Depends on**: T9, T10, T31, T32, T38, T39, T42, T43
**Reuses**: analog-structure's T36
**Requirement**: AO-12
**Executor**: Sonnet

**Done when**:

- [x] `gaps_and_roadmap_follow_analog_operators` passes; `every_cli_listing_of_the_book_is_what_the_binary_prints` and the inventory guard pass
- [x] Gate check passes: `Run docs`, `Run clibook`, `Run inventory`, then Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: analog-operators closes L43, L44, L45, L47, L49, L60, H2 noise and U7`

---

## Checks

**C1** - `ddx(id, v(a, k))` of `is · (exp(v(a, k)/nvt) − 1)` at `v(a, k)` = 0.6 V gives `is/nvt · exp(0.6/nvt)` within 1e-9 relative. (AO-01, AC 1; T32)
Proof: run spartial | test ddx_of_the_diode_current_is_its_conductance ... ok

**C2** - `ddx(v(k, a), v(a, k))` is −1. (AO-01, AC 2; T21, T32)
Proof: run spartial | test ddx_against_the_reversed_branch_is_minus_one ... ok

**C3** - `ddx(v(g) - v(s), v(g, s))` is 0: a probe of another branch is held constant. (AO-01, AC 3; T21, T32)
Proof: run irops | test probes_of_other_branches_are_held_constant ... ok
Proof: run spartial | test ddx_of_single_node_probes_against_a_branch_is_zero ... ok

**C4** - `ddx(x * i(a, b), i(a, b))` is `x`. (AO-01, AC 4; T32)
Proof: run spartial | test ddx_against_a_flow_probe_gives_its_factor ... ok

**C5** - `ddx` differentiates through `limit(v(a, k), f)` as through its argument. (AO-01, AC 5; T21, T32)
Proof: run irops | test limit_is_transparent_to_ddx ... ok
Proof: run spartial | test ddx_differentiates_through_limit ... ok

**C6** - The result of another instance read by `e` is held constant. (AO-01, AC 6; T21)
Proof: run irops | test results_of_other_instances_are_held_constant ... ok

**C7** - A second argument that is not a potential or flow probe of a branch of the module (`p.inflow()`, any other expression) is `E0451`. (AO-01, AC 7; T22)
Proof: run typeops | test ddx_against_a_non_probe_is_e0451 ... ok

**C8** - `ddx` outside `analog`, or in a body `let` or value input a definition or a reaction reads, is `E0450`. (AO-01, AC 8; T22)
Proof: run typeops | test ddx_outside_analog_or_read_by_a_definition_is_e0450 ... ok

**C9** - An AC sweep of `i(o, gnd) +> ddx(id, v(a, k)) * v(in);` gives `|v(o)/v(in)|` = the operating point's `ddx` times the load of `o`, within `AC_MAGNITUDE`. (AO-01, AC 9; T32)
Proof: run spartial | test an_ac_sweep_takes_ddx_at_the_operating_point ... ok

**C10** - `delay_tline.msl` agrees with `delay_tline.cir` (a matched lossless line, `TD` = 1 µs) under `TRAN_VOLTAGE` at every print row. (AO-02, AC 1; T39)
Proof: run partran | test delay_tline ... ok

**C11** - Before `td` the delay reads the value `x` had at the operating point. (AO-02, AC 2; T33)
Proof: run sdelay | test before_td_the_delay_reads_the_operating_point ... ok

**C12** - An operating point gives `delay(x, td)` = `x`, and `td` = 0 gives `x` in any solve. (AO-02, AC 3; T33)
Proof: run sdelay | test an_operating_point_and_a_zero_delay_give_the_input ... ok

**C13** - With `max`, a varying `td` reads `x` as it was `td` ago, and `max` ago while `td` exceeds `max` (edge case: `td` exactly `max` reads `max` ago). (AO-02, AC 4; T28, T33)
Proof: run sdelay | test a_varying_delay_reads_back_td_and_max_while_td_exceeds_it ... ok
Proof: run tops | test the_delay_terms_clamp_td_to_max ... ok

**C14** - `delay` without `max` whose `td` is not constant during an analysis after value inputs are substituted is `E0452` naming the placement. (AO-02, AC 5; T33)
Proof: run elabops | test a_delay_without_max_whose_td_varies_is_e0452 ... ok

**C15** - A negative `td`, or a `max` that is not positive, fails the run with `E0453` naming the instance. (AO-02, AC 6; T28, T33)
Proof: run sdelay | test a_negative_td_or_a_non_positive_max_fails_with_e0453 ... ok
Proof: run tops | test the_delay_function_faults_453 ... ok

**C16** - `delay_tline` in AC gives `0.5 · exp(−jω · 1 µs)` and agrees with ngspice under `AC_MAGNITUDE` and `AC_PHASE`. (AO-02, AC 7; T39)
Proof: run parac | test delay_tline_ac ... ok

**C17** - `i(a, gnd) +> noise(-1.0);` runs `op`, `transient` and `ac` with the values of the design without it, and its density is never read there. (AO-03, AC 1; T34)
Proof: run snoise | test a_noise_source_is_zero_outside_a_noise_sweep_and_never_reads_its_density ... ok

**C18** - One `noise` instance read by `i(a, mid) +> n; i(mid, b) +> -n;` is one fully correlated source. (AO-03, AC 2; T34)
Proof: run snoise | test one_source_read_twice_is_fully_correlated ... ok

**C19** - Two `noise` instances feeding one output add their output power densities. (AO-03, AC 3; T34)
Proof: run snoise | test two_sources_add_their_powers ... ok

**C20** - A density that reads `freq()` is evaluated at each sweep frequency and never at `freq()` = 0. (AO-03, AC 4; T34)
Proof: run snoise | test a_density_reading_freq_is_evaluated_at_each_sweep_frequency ... ok

**C21** - A noise result through `ddt(c * n)` takes the response `jω·c`, and through `delay` the response `exp(−jω·td)`. (AO-03, AC 5; T34)
Proof: run snoise | test noise_through_ddt_and_delay_takes_their_response ... ok

**C22** - A negative or non-finite density in a noise sweep fails the analysis with `E0454` naming the instance and the frequency. (AO-03, AC 6; T29, T34)
Proof: run snoise | test a_negative_density_fails_with_e0454_naming_the_frequency ... ok
Proof: run tops | test the_noise_function_writes_its_density_and_faults_454 ... ok

**C23** - A reaction or an event argument reads a `noise` result as 0. (AO-03, AC 7; T29, T34)
Proof: run snoise | test a_reaction_or_an_event_reads_noise_as_zero ... ok
Proof: run tops | test a_reaction_reads_noise_as_zero ... ok

**C24** - Two sources sharing a `name` stay independent: their powers add. (AO-03, AC 8; T3, T34)
Proof: run snoise | test two_sources_sharing_a_name_stay_independent ... ok
Proof: run report | test two_inputs_sharing_a_name_sum_into_one_series ... ok

**C25** - `white_noise(psd, name)` is a source of density `psd`: a lone resistor's `4kT/R`. (AO-04, AC 1; T34)
Proof: run snoise | test white_noise_is_a_source_of_its_density ... ok

**C26** - `flicker_noise(k, exp, name)` has density `k / f^exp` at each frequency. (AO-04, AC 2; T34)
Proof: run snoise | test flicker_noise_falls_as_one_over_f_to_the_exponent ... ok

**C27** - WHERE `generics`, `composite-values` and `params-and-io` have merged, `noise_table` gives its piecewise density; until then it is refused by name, naming its dependencies, and `GAPS.md` L58 holds it with the generics group (no row of its own: L45 closed with the noise sources). (AO-04, AC 3; T34, T44, D17)
Proof: run elabops | test noise_table_waits_for_its_dependencies ... ok
Proof: run subset | test p0001_names_the_construct ... ok
**C28** - A noise run with `output`, `from`, `to` and `per_decade` prints one row per frequency with `freq` and `onoise`, then the line `onoise_total`. (AO-05, AC 1; T13, T14)
Proof: run apinoise | test a_noise_run_prints_freq_onoise_and_the_total ... ok
Proof: run clianalyses | test a_noise_run_prints_one_row_per_frequency_then_the_total ... ok

**C29** - With `input`, the run adds the column `inoise` and the line `inoise_total`; `inoise` is `onoise` over the transfer from the input. (AO-05, AC 2; T5, T13)
Proof: run apinoise | test an_input_adds_inoise_and_its_total ... ok
Proof: run report | test the_input_referred_density_divides_by_the_transfer_from_the_input ... ok

**C30** - `--output` and `--input` win over the keys. (AO-05, AC 3; T11)
Proof: run apinoise | test the_output_and_input_flags_win_over_the_keys ... ok

**C31** - A noise run with no `output` or no sweep is `P0010` naming the missing key. (AO-05, AC 4; T11)
Proof: run apinoise | test a_noise_run_without_output_or_sweep_is_p0010 ... ok

**C32** - An `output` that names no node, or an `input` that names no instance with an `ac_stim`, is `E0801` naming the path before the analysis runs. (AO-05, AC 5; T12)
Proof: run apinoise | test an_output_or_input_that_names_nothing_is_e0801 ... ok

**C33** - A noise run that gives `stop` or `sweep` is `P0010`. (AO-05, AC 6; T11)
Proof: run apinoise | test a_key_of_another_analysis_is_p0010_in_a_noise_run ... ok

**C34** - `analysis(Noise)` is true in a noise run's operating point and sweep, and `ac_stim` is 0 in the sweep. (AO-05, AC 7; T34)
Proof: run snoise | test analysis_noise_is_true_and_ac_stim_zero_in_a_noise_run ... ok

**C35** - `piperine-api` returns the rows, the totals and each group's density. (AO-05, AC 8; T13)
Proof: run apinoise | test the_library_returns_rows_totals_and_group_densities ... ok
Proof: run apinoise | test report::the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals ... ok

**C36** - `noise_rc.msl` agrees with ngspice's `onoise_spectrum` at every frequency and its `onoise_total` under `NOISE`. (AO-06, AC 1; T40)
Proof: run parnoise | test noise_rc ... ok

**C37** - `noise_switch.msl` agrees with `noise_switch.cir` under `NOISE`. (AO-06, AC 2; T41)
Proof: run parnoise | test noise_switch ... ok

**C38** - `noise_diode.msl` (shot `2qI`, flicker `kf·I^af/f`) agrees with ngspice's `D` under `NOISE`. (AO-06, AC 3; T42)
Proof: run parnoise | test noise_diode ... ok

**C39** - With the ngspice comparison disabled, the three noise twins still run and print the harness's skip line. (AO-06, AC 4; T40)
Proof: run parnoise | test noise_twins_run_and_print_the_skip_line_without_ngspice ... ok

**C40** - `simparam("vth0")` is `Some(0.7)` when the run gives `simparam = { vth0 = 0.7 }` and `None` when it gives none, so `.unwrap_or(0.5)` reads 0.7 and 0.5 (the user decision of 2026-11-23: no default argument; edge case: a default and a run value give the run's). (AO-07, AC 1; T36, T46)
Proof: run ssession | test simparam_reads_the_run_entry_else_its_default ... ok
**C41** - `simparam("reltol")` is `Some` of the run's `reltol`, or of the solver's default (1e-3) when the run gives none: the session options read by their key names. (AO-07, AC 2; T36, T46)
Proof: run ssession | test simparam_reads_the_options_by_their_key_names ... ok
**C42** - `simparam(name)` of a name the run lacks is `None`: `E0456` is retired, the session opens and the model takes its own default. (AO-07, AC 3 as amended; T36, T46)
Proof: run apisession | test a_name_the_run_lacks_reads_none ... ok
**C43** - `simparam` runs in a definition and a selector, with one value for the whole analysis. (AO-07, AC 4; T35, T36, T46)
Proof: run ssession | test simparam_runs_in_a_definition_and_a_selector_with_one_value ... ok
**C44** - `--simparam <name>=<value>` is repeatable; a value that is not a number is `P0010` naming the entry. (AO-07, AC 5; T16)
Proof: run clioptions | test simparam_flag_is_repeatable_and_a_non_number_is_p0010 ... ok
Proof: run apioptions | test a_simparam_entry_naming_an_option_is_p0010 ... ok

**C45** - A run table that gives the key `tnom` is `P0010` naming the key: the run has no `tnom`. (AO-08, AC 1 as amended; T45)
Proof: run apioptions | test a_run_key_tnom_is_refused ... ok

**C46** - The command line has no flag `--tnom`: `piperine run ... --tnom 50` is a usage error naming it and prints nothing. (AO-08, AC 1 as amended; T45)
Proof: run clioptions | test a_tnom_flag_is_refused ... ok

**C47** - There is no run `tnom`: `simparam("tnom")` is `None`, and the nominal temperature is a per-instance model param, which the twin `tnom_res` sets on its resistor (323.15 K against ngspice's `.options tnom=50`) and holds to ngspice under `DC_VOLTAGE`. (AO-08 withdrawn, user 2026-11-23; T43, T45)
Proof: run apisession | test the_session_hands_its_options_and_entries_to_the_circuit ... ok
Proof: run parop | test tnom_res ... ok
**C48** - `tnom_res` with its resistor's `tnom` param at 323.15 K and `temp = 27` agrees with ngspice's `.options tnom=50 temp=27` under `DC_VOLTAGE`. (AO-08, AC 3; T43)
Proof: run parop | test tnom_res ... ok

**C49** - `emit!("conv", v = v(a));` in `analog` writes one record per evaluation in every equilibrium solve, transient step and jump solve, with the instance path, the analysis, the instant and the Newton iteration. (AO-09, AC 1; T30, T38)
Proof: run straces | test a_trace_writes_one_record_per_evaluation_with_path_analysis_instant_and_iteration ... ok

**C50** - The trace records written during a rejected step stay: the instants that hold records and no accepted row has are exactly as many as the step attempts the solver counts rejected. (AO-09, AC 2; T38)
Proof: run straces | test traces_of_a_rejected_step_stay ... ok

**C51** - An AC or noise sweep writes no trace record: the run delivers its operating point's records (an equilibrium solve, AC 1) and nothing from the sweep. (AO-09, AC 1 and 3; T38)
Proof: run straces | test an_ac_or_noise_sweep_writes_no_trace_beyond_its_operating_point ... ok

**C52** - An `emit!` under an `if` writes a record only at the evaluations that execute it. (AO-09, AC 4; T30, T38)
Proof: run straces | test a_trace_under_an_if_writes_only_when_executed ... ok
Proof: run tops | test a_trace_writes_its_watch_only_where_it_executes ... ok

**C53** - A site reached from `analog`, directly or through a `fn`, that writes `sim` or `checks` (`require(..)` included) is `E0603` naming the site. (AO-09, AC 5; T23)
Proof: run typeops | test a_site_reached_from_analog_writing_sim_or_checks_is_e0603 ... ok

**C54** - A run that does not name a trace stream in `streams` records none of it, and naming another stream delivers none of it either; one that names it receives every record. (AO-09, AC 6; T38)
Proof: run apitraces | test traces_are_recorded_only_for_a_named_stream ... ok

**C55** - A trace changes no carrier, solve or event: the run with and without it gives the same solution. (AO-09, AC 7; T38)
Proof: run straces | test a_trace_changes_no_solution ... ok

**C56** - A node of a nature whose potential `abstol` is 1m carries 1m, and its update and residual tolerance is the smaller of that and the session's `vntol`: the default 1 µV is not loosened, `--vntol` 1e-2 loosens it to 1m, 1e-9 tightens it. (AO-10, AC 1, AC 6; T10)
Proof: run apinatures | test a_potential_abstol_of_one_millivolt_reaches_its_node ... ok

**C57** - A branch of a nature whose flow `abstol` is 1n gives its flow unknown and its row the tolerance 1n. (AO-10, AC 2; T10)
Proof: run apinatures | test a_flow_abstol_of_one_nanoampere_reaches_its_branch ... ok

**C71** - A branch's flow unknown and its row take the smaller of the session `abstol` and the nature's flow `abstol`: session 1e-6 on an `Electrical` source gives 1e-12, session 1e-15 gives 1e-15. (AO-10, AC 5; T10)
Proof: run apinatures | test a_nature_only_tightens_the_session_abstol_of_a_branch ... ok

**C58** - `Electrical` and `HvElectrical` at one node give it 1 µV, and the mixed divider equals the all-`Electrical` one within `DC_VOLTAGE`. (AO-10, AC 3; T10)
Proof: run apinatures | test compatible_natures_take_the_smaller_abstol_and_the_divider_holds ... ok

**C59** - Natures whose flow names differ meeting at a port are `E0251`. (AO-10, AC 4; T10)
Proof: run apinatures | test natures_with_different_flow_names_at_a_port_are_e0251 ... ok

**C60** - The interpreter refuses a design holding `ddx`, `delay`, `noise`, `simparam` or `emit!` in `analog`, naming the form. (AO-11, AC 1; T31)
Proof: run frozen | test the_interpreter_refuses_each_analog_operator_by_name ... ok

**C61** - Each of the existing twins prints, on each backend, the bytes it printed before this feature, and the generated goldens are the ones of before except one sanctioned line: `vsource.rs` prints `abstol: [1e-6, 1e-12]` on the private flow pin `b0.flow` (T10, D18, verification Finding 2). (AO-12, AC 1; every task)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
Proof: run tgolden | test the_five_m1_bodies_print_as_the_golden_files ... ok
Proof: run planops | test a_body_without_a_new_form_keeps_its_shape ... ok

**C62** - `parity_op`, `parity_dc`, `parity_ac`, `parity_tran` and `parity_corpus` pass with the existing constants of `contract.rs` unchanged. (AO-12, AC 2; T40)
Proof: run contract | test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok
Proof: run corpus | test every_twin_has_a_cir_and_a_test ... ok

**C63** - `GAPS.md` no longer lists L43, L44, L45 (but `noise_table`), L49, `simparam` of L47, the noise row of H2 and U7; the Book shows one noise run held to the binary. (AO-12, AC 3; T44)
Proof: run docs | test gaps_and_roadmap_follow_analog_operators ... ok
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok
Proof: run inventory | test every_block_of_the_book_has_one_row ... ok

**C64** - The stdlib `slew(v(in), 1M, 1M)` after a 0 to 1 V ideal step at 1 µs gives 0 V before 1 µs, 1 V/µs and 1 V from 2 µs, within `TRAN_VOLTAGE`. (AO-13, AC 1; T7, T8, T9)
Proof: run sslew | test slew_limits_an_ideal_step_to_its_rate ... ok
Proof: run retake | test a_crossing_made_by_a_jump_retakes_the_step ... ok
Proof: run spectext | test part_v_retakes_the_step_a_jump_crossing_changes ... ok

**C65** - A noise run whose `output` is a flow (`i(<instance>)`) prints `onoise` in A/√Hz. (Edge case; T6, T13)
Proof: run report | test a_flow_output_reads_amperes_per_root_hertz ... ok

**C66** - A design with no noise source prints `onoise` = 0 at every frequency and succeeds. (Edge case; T13)
Proof: run apinoise | test a_design_with_no_noise_source_prints_zero_and_succeeds ... ok

**C67** - `ddx` of an expression that does not read the probe is 0. (Edge case; T32)
Proof: run spartial | test ddx_of_an_expression_that_reads_no_probe_is_zero ... ok

**C68** - A trace site whose field types differ from another site of the same stream is `E0605`. (Edge case; T37)
Proof: run elabops | test a_field_type_conflict_across_trace_sites_is_e0605 ... ok

**C69** - A value of a type other than `f64` given to `delay` or `noise` is the checker's type mismatch `E0180` at the argument; `E0150` is retired with the explicit model (explicit-unknowns EU-02), so no unknown value reaches them. (§14 errors; T23, T33, T34)
Proof: run typeops | test a_value_of_another_type_given_to_delay_or_noise_is_a_type_mismatch ... ok
**C72** - A step that reads a settle commit retakes when its crossing changes what `analog` reads, and a retaken instant settles once and delivers that phase's settle commits. (AO-13, AC 2; fix round)
Proof: run retake | test a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it ... ok
Proof: run retake | test a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits ... ok
Proof: run retake | test the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order ... ok
Proof: run digital | test a_reopened_instant_settles_at_its_last_microstep ... ok
Proof: run digital | test a_firing_after_a_reopened_instant_is_due_at_the_next_microstep ... ok

**C70** - The band total integrates the density by ngspice's power-law rule, exact on power laws. (AO-05, AC 1, D8; T2, T3)
Proof: run numerics | test the_power_law_integral_is_exact_on_power_laws ... ok
Proof: run report | test the_band_total_integrates_the_density_as_ngspice_does ... ok
Proof: run abinoise | test test_per_source_noise_reporting_and_conservation ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("Tu sempre assume o default mais razoável e depois me conta"); the defaults are design.md D1 to D20. T9 blocked (orchestrator AFK defaults tried and not applied): (a) restarting the BDF2 stage as backward Euler over the whole step after Restart::Integrator (TrBdf2::stage_coeffs `(1/h, 0, -1/h)` at prev_h = 0, with Part V section 15.2.6 and Appendix C.2 reworded) makes `slew_limits_an_ideal_step_to_its_rate` pass, but it moves existing results: `piperine-analog-solver` `assembly_ddt` (hand-derived BDF2 current at prev_h = 0), `piperine-cli` `book` (3 listings), `buck` (`startup` 0.8025 to 0.8029), `options` (records) and `twins` (3 targets), so it was reverted per the rule; the parity/buck_* targets of piperine-parity were not reached before the stop. (b) the stdlib direction rule from sign(input - output) cannot work alone: in Track `pos(y) = x`, so the error is 0 when the reaction runs. T9 (test and fixture kept in the session scratchpad, uncommitted) needs a decision: accept (a) with the listed goldens regenerated, or give `slew` a direction that does not read the sign of `ddt`. B1 settled items: (1) per-source totals (T3): `onoise_total`/`inoise_total` sum each source's power-law integral then take the root (matches ngspice: two-source RC 1.999884e-6 both; integral-of-sum gives 2.000949e-6); same-name inputs are merged before integration. (2) D8: a non-positive endpoint density gives N1*df, no 1e-38 floor; an interval starting at a non-positive frequency is a trapezoid. (3) T4: new types `StimulusColumn` (name, phase, rows) and `TermLinearization` (unknowns, noise by input position); `stimulus_columns` returns `Result`; `NoiseColumn` gains `instance` and `input`. (4) T5: an input with no STIM is refused at solver build naming it; a zero transfer at a frequency fails naming input and frequency. (5) T6: `FlowOutput` enum {`Branch(BranchIdentifier)`, `Term{instance, term}`} in the prelude; `output_flow` replaces the node pair; `NoiseSolver`'s `out_ref`/`ref_ref` are replaced by a private `OutputTarget`. (6) `output_flow: None` added mechanically to five literals (`abi_noise` x2, `environment`, `failure_rules` x2).
**User decision 2026-11-23 (direct):** "dtemp seria um signal e o tnom seria por instância, o que deixa o nosso core apenas com temp". The core keeps only the run temperature (`temp`, `temperature()`); `tnom` is an ordinary per-instance model param (spice-faithful models declare `tnom`, default 27 C = 300.15 K) and `dtemp` is no core mechanism. Effect here: T36 gives the circuit `reltol`, `abstol`, `vntol`, `trtol`, `temp` and the `simparam` entries, no `tnom`; AO-08 and the run `tnom` key of T15 are withdrawn by the new task T45 (appended below), which removes the key, the flag, the context field and the spec text and makes each spice-faithful model read its own `tnom` param.
**User decision 2026-11-23 (direct, second):** "É, melhor voltar Option". `simparam(name) -> Option<f64>`: `Some(v)` when the run set it, `None` otherwise; no default argument; `E0456` is retired. Models write `.unwrap_or(..)` or a `match`. It replaces the planned `multiplicity()` (generics phase D reads `simparam("multiplicity")`). Landed as T46 (below, done): IR `Expr::SessionParameter { parameter }` typed `Option<f64>` (no `default`, no `required`), the checker row, the elaborator's `lower_session_pair` over `OptionPart`, an always-`OPTIONAL` pin, no host fault. Checks C40, C41, C42, C43 and C47 read in that form (C42 is retired: a name the run lacks is `None`, asserted by `a_name_the_run_lacks_reads_none`; C47 is retired with `tnom`).
**B4 settled items (T17 to T23):** (1) printed forms: `delay<k>(x, td[, max]) "name"` with the read `delay<k>.value`; `noise<k>(psd, "group"|none) "name"` with `noise<k>.value`; `session S<k> "name"` with `simparam("name"[, default])`; `trace<k> "stream"` with the statement `trace<k> {f0 = .., text = ".."}`. (2) Deps classes: a delay read is an affine unknown, a noise read a constant of the sum, a session parameter constant unless its default is not. (3) Differentiation folds zeros, treats function parameters as locals, names derived functions `<f>$d<i>` (memoized in `IrBuilder`), refuses a recursive function, a block with `let` outside a function and a local outside a function. (4) Checker: `ddx_call` in `types/analog.rs`; second argument must be a probe call (flow probes accepted), else `E0451`; `noise`'s name is `Option<str>`, `simparam`'s default `OptionF64`. (5) `emit!` in `analog` types from T23; one reached through a `fn` stays `P0001` naming the `fn` (D14); the elaborator refuses a direct one until T37 (SPEC_DEVIATION in `elab/records.rs`). (6) Until T24 to T31 the printer refuses `Expr::Delay`, `Expr::Noise`, `Expr::SessionParameter` and `Stmt::Trace` with `Unsupported` (SPEC_DEVIATION in `print/value.rs`). (7) Rewritten tests: `types_subset.rs` rows (ddx, delay, noise, simparam), `types_events.rs` (`an_emit_in_analog_is_p0001_by_name`, `a_check_reached_from_analog_is_e0603`), the `Deps` literal of `piperine-ir/tests/deps.rs`. (8) `contract.md` has the B4 re-anchor note. (9) A pre-existing warning (`crates/piperine/tests/book/types.rs` `Reached::run`) is untouched.
**B5 settled items (T24 to T31):** (1) `SessionParameter` first carried `required` (some read gives no default) so the plan set `OPTIONAL` on the pin only when every read had a default (D12); T46 withdrew it: the pin is always `OPTIONAL` and the read an `Option<f64>`. (2) The transpiler emits delay sites, noise sources, session parameters and trace watches as the printer's forms; a `delay` site reads its `max` pin as unbounded until the parameter cone has written it (the operating point's first solve precedes the cone's first run), and a zero slot is no bound. (3) The interpreter refuses each new form by name (`P0001`), frozen R1. (4) A direct `emit!` in `analog` stays a checker `P0001` until T37 lowers it; the elaborator's refusal of an unlowered core primitive says `core primitive X` as the checker did (Book cheatsheet card).
**B6 settled items (T32 to T38):** (1) `E0453` and `E0454` are raised (their `code_owners.tsv` rows and the `GAPS.md` section 10 entries left; `E0451` before them). (2) A noise input is named by its source's group; a density always reads `FREQ` so an `E0454` fault names the frequency. (3) The session gives `reltol`, `abstol`, `vntol`, `trtol`, `temp` and the run's `simparam` entries, no `tnom`; the pin of a session parameter is `{ v: F64 @0, known: BOOL @8 }` and the solver sets `known` when it binds a value. (4) `emit!` in `analog` lowers in `elab/records.rs` (`lower_emit`: a `Stmt::Trace` and its `TraceSite` when the scope is the analog block); one reached through a `fn` stays `P0001` naming the `fn` (D14, GAPS L72). (5) `Record` gains `iteration` and `analysis`; a trace is delivered only for a stream the run names (`--stream`), and the run-key count of `request.rs` is 36 after T45. (6) Rewritten tests: `tests/book/analog.rs` `the_noise_source_runs_and_reads_as_zero_outside_a_noise_sweep` (analog#11 runs), the welcome status paragraph, the Book's analog#7 and cheatsheet#7 (they run, so their inventory and `GAPS.md` section 11 rows left).
**B7 settled items (T39 to T44):** (1) `delay_tline` has `TD` = 1.05 us, not 1 us: with a print point on `TD` itself ngspice's print interpolation reads 0.75 mV at the kink where the wave first arrives, a reference artifact, so the kink sits between two print points. (2) `NOISE` is reltol 1e-3, abstol 1e-20 and covers the densities and the totals (no second constant); `Coordinate::Band` names a total's band. (3) ngspice 46 and 47 read the switch state vector at 0 in `.noise`: a switch the operating point closed carries the open switch's noise and transfer (`noise_switch.cir` prints 3.8819e-9, the open value of `noise_switch_off.cir`, with `v(a)` = 0.999 mV), so the closed `noise_switch` is held to its closed form `sqrt(4 k T (1k || ron))` and the same twin with its control opened agrees with `noise_switch_off.cir` (AO-06 AC 2 amended). (4) `tnom_res` reads no `simparam`, so both backends run it and it is not `COMPILED_ONLY`; the four noise and delay twins are. (5) The noise twins are `OWN_TARGET` of the deviation table; the corpus count guard went 18 to 23; every twin also has a test in `tests/scenarios.rs` (`Comparison::Noise`, `FlatNoise`). (6) The pre-existing warning of `tests/book/types.rs` (`Reached::run`) is untouched. (7) `E0180` is the checker's code for a type mismatch (not `P0002`).
**User approvals (2026-11-22):** T9's moved Book numbers ("Numeros do Book mudaram então acerta o book"), Part V §15.2 rule 6 ("Ok"), and the fix of the settle/retake defect, verification Finding 3 ("Corrige"); see t9-decision.md.
**User decision (Finding 18, 2026-11-22, "Menor dos dois"):** nodes and branches both take the minimum of the session value and the nature's `abstol`; see AO-10 AC 6 and design.md D18.
**Dispatched:** none
**Walked:** none (no screen; every story's Independent Test is a test target)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 8

Phase 1:  T1
Phase 2:  T2, T3, T4, T5, T6
Phase 3:  T7, T8, T9, T10
Phase 4:  T11, T12, T13, T14, T15, T16
Phase 5:  T17, T18, T19, T20, T21, T22, T23
Phase 6:  T24, T25, T26, T27, T28, T29, T30, T31
Phase 7:  T32, T33, T34, T35, T36, T37, T38
Phase 8:  T39, T40, T41, T42, T43, T44
```

Tasks run in the order listed; the dependencies are the diagrams of the Execution Plan. Execution is strictly sequential.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | When |
| ----- | ------ | ----- | ---- |
| B1 | 1, 2 | T1 to T6 | **now**: solver crates only, disjoint from `composite-values` and `generics` |
| B2 | 3 | T7 to T10 | **now**: the owner protocol, Part V §15.2, new parity and api targets |
| B3 | 4 | T11 to T16 | **now**: `piperine-api` (`request.rs`, `session.rs`, `table.rs`, new `noise.rs`) and `piperine-cli` (`cli.rs`, `render/table.rs`); `composite-values` B10 edits other files of both crates (`host.rs`, a new `composite.rs`, the render module): a textual conflict there is resolved at rebase, not avoided |
| B4 | 5 | T17 to T23 | **waits** for `composite-values` B10 and `generics` B8 on `main`; T17 rebases |
| B5 | 6 | T24 to T31 | waits (after B4) |
| B6 | 7 | T32 to T38 | waits (after B5) |
| B7 | 8 | T39 to T44 | waits (after B6); closes the feature |

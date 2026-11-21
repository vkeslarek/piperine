# Integrated Tests Specification

**Approved**: 2026-11-21 - user (standing AFK authorization, relayed by the orchestrator): "lança um agent para compor um conjunto de testes integrados. Deixa o buck só como example, para nós mesmos." Defaults D1-D16 below are the planner's, to be reviewed by the user.

## Problem Statement

`cargo test` at the root takes 45 to 90 minutes, and most of it is the buck converter of `examples/buck`:
`tests/buck_scenarios.rs` (25 to 90 min in a debug build, six of its tests red on the model since
`buck-scenarios`), `piperine-parity`'s `buck_closed_loop` (330 s measured on `main` at `856d2668`),
`piperine-api`'s `buck_long` (213 s in `long-run`'s proofs), `piperine-cli`'s `buck` (the 600 µs start through
the binary) and `piperine-cli`'s `book`, whose shared listing fixture runs that same start for every test of
the target. Every batch worker runs the full suite at its batch end, every Verifier and every rebase runs it
again: the buck is the main reason implementation is slow. The capabilities those targets prove (closed-loop
mixed-signal regulation, PWM timing, ADC sampling, a controller in reactions, an enum state machine, host
stimulus, records, `settle`, long transients on a grid, the CLI's tables, the Book's listings) are proven
nowhere else at the level of a whole design.

## Goals

- [ ] `cargo test` at the root runs no target that simulates `examples/buck` or `tests/e2e/buck`; every such
      target still runs by name, with one documented command.
- [ ] A small integrated suite proves the same capabilities on circuits that run in seconds; its root target
      completes in under 3 minutes of wall time in a debug build on the worker's machine, compile included.
- [ ] Every claim of the suite has an exact pass criterion: a closed form under an existing
      `ToleranceContract`, a reference model compared bit for bit, or an exact count; analog-only parts are
      cross-checked against ngspice under the existing contracts.
- [ ] No test is ignored, switched off by `cfg`, weakened or deleted; ngspice parity and every tolerance are
      untouched.
- [ ] The in-flight features have one replacement gate line each, recorded where they will read it at
      their next rebase.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Fixing the six red `buck_scenarios` tests (C22, C50, C68 to C71 of `buck-scenarios`) | They judge the model against SPEC; this feature moves the target, it does not change the converter. They stay red, on demand, and are reported |
| Making the buck faster (multi-rate, averaged models) | `multi-rate` (V12), `surrogate-models` (V13) |
| Editing the in-flight branches' `tasks.md` | Those worktrees are active; the replacement lines go into `.specs/STATE.md` (MD-77) and each branch adopts them at its next rebase onto `main` (D12) |
| The static front-end proofs of the buck (`buck_example`, `elab_buck`, `types_buck`, `fmt --check` in `examples/buck`, the README/SPEC text tests of `docs.rs`) | They do not simulate; together they cost seconds and keep the example from rotting (D3) |
| The Book's own averaged buck of chapter 10 (`tests/book/mixed.rs`, `buck.msl` of the Book) | It is a chapter model, not `examples/buck`; it stays in `book_mixed` (D4) |
| The hand-built "buck slice" IR bodies of `piperine-ir`, `piperine-plan`, `piperine-transpile` (`dpwm.rs`, `pid.rs`, `digital_circuit.rs`, ...) and `piperine-parity`'s `boundary.rs` | They are small hand-built bodies, not the example; they already run in seconds |
| A Python host, a CI schedule for the buck | Not asked; the buck runs on demand by hand (D11) |
| Changing a tolerance or a `ToleranceContract` | MD-38 and the user's rule |

---

## Decisions taken (user AFK)

The user authorized the planner to decide ("take the most reasonable defaults, record D-numbered defaults").
Every row is a default; each is reported and each can be overturned.

| # | Question | Default taken | Why |
| - | -------- | ------------- | --- |
| D1 | How the buck leaves the default run | Each buck target gets an explicit `[[test]]` table with `test = false` in its own crate's manifest (cargo: excluded from `cargo test`, `--tests` and `--all-targets`; still run by `cargo test -p <crate> --test <name>`). Verified on a scratch package with cargo of this machine | No `#[ignore]`, no `cfg`, no feature flag, no file moved: every path, test name, `covered_by` row of `tests/book/inventory.tsv`, `CARGO_BIN_EXE_piperine` of the CLI target and every historical `**Run**` line of a closed feature stays valid. A crate outside `default-members` would move the CLI target away from its binary; `required-features` adds a feature with no product meaning |
| D2 | What keeps `test = false` from becoming a way to switch tests off | A registry `ON_DEMAND_TARGETS` in `crates/piperine-lang/tests/suite_hygiene.rs` (crate, target, reason); the guard fails for a `test = false` target that is not registered, for a registered entry whose target is gone or runs by default, and for one that `examples/buck/README.md`'s on-demand block does not name | The registry-plus-exhaustiveness shape the guard already uses (MD-33, `ROOT_TARGET_EXEMPTIONS`) |
| D3 | Which buck targets leave | Every target that simulates the buck or its blocks from `examples/buck`: root `buck_scenarios`; `piperine-parity` `buck_whole`, `buck_dpwm`, `buck_control`, `buck_boundary`, `buck_power`, `buck_supervisor`, `buck_closed_loop`; `piperine-api` `buck_long`; `piperine-cli` `buck`; and the two Book holders below. The static proofs stay | User: "Deixa o buck só como example". The block targets are cheap (5 to 16 s each, measured) but prove the buck's SPEC, which is the example's business; their capabilities are re-proven by the suite |
| D4 | The three `piperine-parity` targets named `buck_*` that prove language rules, not the buck (`buck_commits`: §12.1 commits; `buck_records`: `Never`, `log`; `buck_time`: `every`, `now()`, edges) | Renamed `source_commits`, `source_records`, `source_time` (the `source_*` family: "from MSL source on the Rust backend"), fixtures moved from `tests/buck/` to `tests/digital/`, the `buck::dpwm::Osc` they borrow replaced by a local clock of the same frequency; every assertion unchanged | They are not the buck and stay in the default run; the rename keeps "buck" out of the default target list |
| D5 | The Book's `cli#44` (the buck's bench, run 2 µs) | `book_cli` keeps the text equality (cheap); the short run moves to an on-demand root target `book_buck` (`tests/book/buck.rs`, `test = false`), which the inventory row names | The run compiles the whole buck (about 15 s, like `buck_whole`); the inventory's fingerprint still catches any edit of the block in the default run |
| D6 | The Book's buck listing held by `piperine-cli`'s `book` (`data-cli-project="buck"`, `piperine run startup`) | `book`'s shared listing fixture skips listings of project `buck`; `the_buck_block_is_what_the_binary_prints` moves, unchanged, to an on-demand target `crates/piperine-cli/tests/book_buck.rs`; `book` keeps a cheap test that the chapter shows `benches/startup.msl`, the `[run.startup]` table and the command as the example has them | The fixture of `book` ran the 600 µs start for every test of the target. The listing's text is fingerprinted by `book_inventory` in the default run, so an edit cannot slip; its output is held on demand |
| D7 | Where the suite lives | One root target `integrated` (`tests/integrated/main.rs`, declared in the root manifest), one module per scenario beside it; one scenario package `tests/e2e/integrated/` (`Piperine.toml` with one `[run.<scenario>]` per scenario, `src/lib.msl` and one `src/<scenario>.msl` per scenario) | MD-72: the root holds the integrated tests; MD-73: a host scenario is a package under `tests/e2e/`. One target lets every run execute at once in one `ScenarioBatch`, so the wall time is the longest run's |
| D8 | The harness | `piperine_parity::scenario` (`ScenarioRun`, `ScenarioBatch`, `Stimulus`, `Trace`, `DiscreteTrace`, `CheckTally`), extended so a `CompletedRun` keeps every record of its run | The buck's harness, already unit-tested; with `buck_scenarios` on demand it keeps a default consumer |
| D9 | How exact the criteria are | Digital: bit for bit against a Rust reference model written from this spec, on a print grid whose rows never coincide with a clock edge. Analog: the closed form evaluated at each row or at a record's instant, compared under `ToleranceContract::TRAN_VOLTAGE` (no time tolerance is written: a crossing is judged by the analytic voltage at its instant, as `buck_boundary` does). Counts: exact | The user's rule: closed forms where possible, existing contracts only |
| D10 | The ngspice cross-check | One analog-only twin, `tests/e2e/twins/tran_square_rc.cir`: the RC of `rc_top.msl` driven by a piecewise-linear square (four periods of 10 µs, 20 ps edges), `.tran 0.1u 42u 0 10n uic`; the piperine side drives `RcTop`'s node port with `Session::source` and the same corners, `--max-step 1e-8`, in `piperine-parity`'s `source_host.rs` beside `tran_pwl_rc` | The square drive is the analog half of the comparator and PWM scenarios; `tran_pwl_rc` is the precedent for a host-driven analog-only twin with the same step cap on both sides |
| D11 | When the buck runs | On demand: once at a feature's close (before its merge) for a feature that changes the runtime, the printer, the plan, the solvers or the lowering of an existing form, and in every task that edits `examples/buck/**`, `tests/e2e/buck/**` or a buck target. Per-task and per-batch gates use the suite | The buck stays the strongest oracle, paid once per feature instead of once per batch |
| D12 | The in-flight features | Their replacement gate lines are written in this spec (Gate migration) and in `.specs/STATE.md` MD-77; each branch applies them at its next rebase onto `main` | Editing seven active branches from here would conflict with their workers |
| D13 | The closed loop | A PI (not a PID) in reactions, Q6 integers, the zero cancelling the plant pole; the plant is the PWM filter's RC | The user's words: "a short closed loop with a PI in reactions"; a pole-zero cancellation gives a loop whose settling is derived, not tuned |
| D14 | The CLI's proof of a whole mixed design | A target `crates/piperine-cli/tests/integrated.rs` running `piperine run pi_loop` and `piperine test pi_loop threshold` in `tests/e2e/integrated` | MD-72: the binary's own output is held in `piperine-cli`; both runs need no host stimulus |
| D15 | The command to run the buck | One block in `examples/buck/README.md` ("On demand") and the same lines in `CLAUDE.md`'s "Build and test", four `cargo test` lines, one per crate, no wrapper script | A script would be one more thing to keep honest; the lines are what a proof cites |
| D16 | Decision number | MD-77 (MD-75 is `params-and-io`'s, the last on `main`); renumbered at merge if another lane takes it | The `.specs/STATE.md` convention |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1 to D16 above | as written there | delegated by the user ("take the most reasonable defaults") | n - default (user AFK) |
| A clock's edges | A clock of frequency `f` first changes at half a period: rising edge `k` (k ≥ 1) at `(k - 1/2)/f`, falling edge `k` at `k/f` | The convention of `examples/buck`'s `Osc` and `tests/buck/commits.msl`'s `Osc` | n - default (user AFK) |
| A value read from a table at a clock edge | A row at `k·h` shows a net as it was after the last instant at or before `k·h` (`crates/piperine-api/src/recorder.rs`), so a row on an edge holds the value the edge committed; grids are chosen so a width in rows is a width in clock periods | The recorder's own rule, read at `856d2668` | n - default (user AFK) |
| The D2A edge | `std::convert::D2A` ramps its output over `tr` = 20 ps; the closed forms treat it as a step; the error it makes is below 1e-5 relative, two decades under `TRAN_VOLTAGE`'s reltol | `std/convert.msl` | n - default (user AFK) |
| Switches of the state machine and the sample-and-hold | A conductance `goff + (gon - goff) · transition(level, tr: 1 ps)`, the pattern of `examples/buck`'s `PowerSwitch`; the closed forms treat the 1 ps ramp as a step | Same bound as the D2A | n - default (user AFK) |
| Rounding in the ADC | Mid-tread: `code = clamp(floor(v / vref · 256 + 0.5), 0, 255)`, 8 bits, `vref` = 1 V; held inputs sit at code centres (`k / 256` V), half an LSB (1.95 mV) away from any decision | The buck's `Adc` rule at 8 bits; a centre makes the expected code independent of the solver's last microvolt | n - default (user AFK) |
| The suite's wall-time goal | Measured and reported by the target (each run's wall time and the batch's); not asserted by a test | A wall-time assertion would fail on a loaded machine and prove nothing about the code | n - default (user AFK) |

**Open questions:** none - all resolved or logged above (each a default the user delegated).

**Implicit-requirement dimensions.** Input validation and bounds: N/A because the feature adds tests and
manifest flags, no input surface. Failure and partial-failure states: IT-03 AC 5 (a run that fails is
reported by each of its tests, once, never re-run). Idempotency and retry: IT-03 AC 5 (each run executes
once per target). Auth and rate limits: N/A, local tests. Concurrency and ordering: IT-03 AC 4 (runs in
parallel threads, each in its own build directory). Data lifecycle: IT-03 AC 3 (the runs leave no file in the
tree). Observability: IT-03 AC 6 (each run prints its wall time and rows). External-dependency failure: IT-09
runs ngspice as `tran_pwl_rc` does (an unset `PIPERINE_NGSPICE`); the transpiled backend's `rustc` is the
existing prerequisite. State-transition integrity: IT-07 (the state machine against its reference).

---

## Coverage inventory

What each buck target proves today (read from each target's `//!` header and tests at `856d2668`), its cost,
and the scenario of the suite that re-proves the capability. Times: measured with
`CARGO_TARGET_DIR=/var/tmp/piperine-target-it CARGO_PROFILE_DEV_DEBUG=0` on this machine (see Handoff of
`tasks.md`), else as recorded in the feature's `proofs.md`.

| Target | What it proves | Wall | Re-proven by |
| ------ | -------------- | ---- | ------------ |
| `tests/buck_scenarios.rs` (root) | SPEC §10 S2 to S10 under a Rust host: setpoint slew and settling (S2, S2b), load and line steps (S3, S4), overcurrent hiccup and OVP (S5, S6), brown-out UVLO and disable (S7, S8), steady state at nine corners (S9), shoot-through monitor (S10); host `Session::drive` and `Session::set_at`, supply staircases (`Stimulus::ramp`); `Trace` measurements (settling, overshoot, crossings, decay constant); five families at once in their own build directories (`ScenarioBatch`); a run leaves no file | 25-90 min; 6 tests red | IT-03 (batch, build directories, tree), IT-05 (staircase `set_at`), IT-06 (drive-free timed change, duty latch), IT-07 (enum state machine, OVP, hiccup, retry, recovery, host drive and `set_at`), IT-08 (settling, regulation band, `settle` check) |
| `piperine-parity` `buck_closed_loop` | the whole `Buck` starts and regulates at 0.8 V (soft-start, power-good, no shoot-through), 600 µs | 330 s | IT-08 (closed loop to regulation, settling, monitors) |
| `piperine-parity` `buck_supervisor` | the supervisor's state table, power-good delay and drop, hiccup length, against a Rust state machine at every sample | 5 s | IT-07 |
| `piperine-parity` `buck_control` | `RefRamp` and `Pid` difference equations bit for bit, saturation, anti-windup; `SetpointReg` takes known data, saturates, ignores unknown | 5 s | IT-08 (PI difference equations bit for bit, saturation, conditional integration) |
| `piperine-parity` `buck_dpwm` | counter PWM at 200 MHz, dither, dead time, never overlapping, release count; two runs equal | 5 s | IT-06 (counter PWM, dead time, no overlap, duty latched at the wrap) |
| `piperine-parity` `buck_boundary` | the `Adc` commits the rounded code after `tconv` on held voltages; `A2D` trips at its thresholds (analytic RC, `TRAN_VOLTAGE`) and is right at time 0 | 7 s | IT-04 (A2D thresholds on an analytic RC), IT-05 (ADC: rounding, clamp, `tconv`, `done` strobe) |
| `piperine-parity` `buck_power` | gate driver delays and interlock; a switch ramps its conductance through `transition`; a gate change makes two time points; inductor RL response; current sense | 7 s | IT-05, IT-07 (a commit drives an analog switch through `transition`; closed-form RC through the switch) |
| `piperine-parity` `buck_whole` | the buck compiles with no `rustc` warning, every shape valid; idles with `en` undriven; one `Never` record per edge, all passing | 16 s | IT-03 AC 2 (every run's design compiles with no warning, valid shapes), IT-06 (`Never` record per edge) |
| `piperine-parity` `buck_commits` | the four commit rules of §12.1 | 6 s | stays in the default run as `source_commits` (IT-12) |
| `piperine-parity` `buck_records` | `Never` records, `log` severity and text, records of two reactions told apart | 10 s | stays as `source_records` (IT-12) |
| `piperine-parity` `buck_time` | `every` from the stdlib, `now()` and `analysis(Static)` in a reaction, `rise()`/`fall()` through unknown | 4 s | stays as `source_time` (IT-12) |
| `piperine-api` `buck_long` | the 600 µs start through the host library: 60 001 rows on a 10 ns grid, every check passes, peak live heap under 1 GiB, overshoot, settling, regulation with power-good, ripple | 213 s (`long-run`) | IT-08 (a 100 µs closed loop on a 10 ns grid: exact row count, checks, settling, ripple band); memory bound stays with `memory.rs` (examples/rc) |
| `piperine-cli` `buck` | the binary runs `idle` and `startup`: run line, table on its 0.5 µs grid (1 201 rows), last row reference 128 and vout within 3 mV, no failed check printed while every check record passes, at most 16 lines for `idle`, `piperine test` passes both runs | minutes (the 600 µs start) | IT-10 (the binary on `pi_loop` and `threshold`) |
| `piperine-cli` `book` (`the_buck_block_is_what_the_binary_prints` and the shared fixture) | the Book's buck listing is what `piperine run startup` prints; the chapter shows the bench and the run table as the example has them | minutes, paid by every test of the target | IT-11 (moved on demand; text equality stays) |
| `piperine` `book_cli` (`the_buck_bench_of_the_chapter_is_the_example_and_starts_with_the_supervisor_off`) | `cli#44` is `benches/startup.msl`; a 2 µs run has the supervisor off and the reference 0 | about 15 s (a whole-buck compile) | IT-11 (text equality stays; the run moves on demand) |

**Feature proofs that cite buck targets.** Closed features on `main` cite them in `**Run**` lines and proofs:
`buck-scenarios` (7), `msl-cli` (16), `long-run` (15), `msl-digital` (23), `msl-fixes` (3), `value-semantics`
(2), `params-and-io` (2), `root-e2e` (1), `unknown-semantics-events` (4). With D1 every one of those commands
still runs verbatim (`cargo test -p <crate> --test <name>` selects a `test = false` target), except those that
name `the_buck_block_is_what_the_binary_prints` under `cargo test -p piperine-cli --test book` (`msl-cli`;
`unknown-semantics-events` T5's done-when, already checked), which now runs under `--test book_buck`, and
those that name `buck_commits`, `buck_records`, `buck_time` (`msl-digital`), now `source_commits`,
`source_records`, `source_time`. Closed features are not re-proven; the mapping is recorded in MD-77.

---

## The suite

One scenario package, five runs, one root target that executes the five at once, one CLI target and one
ngspice twin. Every circuit below is small enough that its run takes seconds; the compile of each run's
design (`rustc`) dominates.

| Scenario (run) | Circuit | Simulated | Replaces |
| -------------- | ------- | --------- | -------- |
| `threshold` | a 100 kHz square through `D2A` (100 Ω) and 900 Ω into 1 nF (τ = 1 µs); `A2D` at 0.5 V ± 0.05 V; a counter of the comparator's edges emitting one record per edge | 42 µs | ADC/comparator boundary, located crossings, records, analog to digital |
| `sample_hold` | a DC source the host steps with `set_at`; a switch (10 Ω on, 1 GΩ off) into a 1 nF hold capacitor, tracking while a 1 MHz clock is high; an 8-bit ADC sampling the hold node at each falling edge, code after 200 ns, 50 ns `done` strobe | 51 µs | ADC sampling, a commit driving an analog switch, host `set_at`, staircase |
| `pwm_filter` | a 100 MHz clock, a counter wrapping at 100 (1 MHz PWM), duty latched at the wrap, high side and low side with 2 counts of dead time, `Never` on both high; the high side through `D2A` and 900 Ω into 10 nF (τ = 10 µs); the host drives the duty from 25 to 60 at 30.3 µs | 60 µs | DPWM timing, dead time, monitors, records, host drive, digital to analog |
| `state_machine` | a 10 MHz clock; `enum Phase { Idle, Charge, Hold, Dump, Fault }` and `enum Fault { Clear, OverVoltage }` carriers; charge switch from a source through 1 kΩ, dump switch through 1 kΩ, a 1 nF capacitor; `A2D` at 0.9 V ± 10 mV as over-voltage; the host raises `en` and steps the source to 2 V and back | 22 µs | supervisor state machine, enum carriers, OVP, hiccup, retry, recovery, host drive and `set_at` |
| `pi_loop` | `pwm_filter`'s plant; the ADC samples the filter at each wrap; a Q6 PI in a reaction on `done` sets the duty; target 0.5 V (code 128); an `on settle` check and one `loop` record per sample; printed on a 10 ns grid | 100 µs | closed-loop mixed-signal regulation, controller in reactions, `settle`, long transient on a grid |

The closed forms, the reference models and the numbers each criterion uses are derived in `design.md`.

---

## User Stories

### P1: The buck leaves the default run ⭐ MVP

**User Story**: As a worker, I want `cargo test` to skip the buck so that a batch gate takes minutes, not an hour.

**Why P1**: The reason implementation is slow.

**Acceptance Criteria**:

1. The workspace SHALL declare each target of D3 (`buck_scenarios`, `buck_whole`, `buck_dpwm`, `buck_control`, `buck_boundary`, `buck_power`, `buck_supervisor`, `buck_closed_loop`, `buck_long`, `piperine-cli`'s `buck`, root `book_buck`, `piperine-cli`'s `book_buck`) as a `[[test]]` table with `test = false` in its own crate's manifest.
2. WHEN `cargo test --no-run` runs at the root THEN its list of test executables SHALL name none of the targets of AC 1.
3. WHEN `cargo test -p piperine-parity --test buck_whole` runs THEN the target SHALL build and its three tests SHALL pass.
4. The targets of AC 1 SHALL keep their file paths, their test names and their assertions.

**Independent Test**: `cargo test --no-run` at the root lists no buck executable; `cargo test -p piperine-parity --test buck_whole` runs.

---

### P1: No test is switched off unregistered ⭐ MVP

**User Story**: As a reviewer, I want every target excluded from the default run to be registered and documented so that `test = false` cannot hide a test.

**Why P1**: `suite_hygiene` forbids switched-off code; `test = false` would otherwise be a quiet way to do it.

**Acceptance Criteria**:

1. IF a crate's manifest declares a `[[test]]` with `test = false` that `ON_DEMAND_TARGETS` does not register THEN `suite_hygiene` SHALL fail naming the crate and the target.
2. IF an entry of `ON_DEMAND_TARGETS` names a target that is missing or has no `test = false` THEN `suite_hygiene` SHALL fail naming the entry.
3. IF `examples/buck/README.md`'s on-demand block does not name a registered target's `--test <name>` THEN `suite_hygiene` SHALL fail naming the target.
4. WHEN the detector reads a planted manifest with two `[[test]]` tables, one with `test = false` THEN it SHALL return exactly that target.

**Independent Test**: `cargo test -p piperine-lang --test suite_hygiene` passes on the tree and its detector tests pass on planted manifests.

---

### P1: The integrated package and its runs ⭐ MVP

**User Story**: As a worker, I want one package of small designs run at once by one target so that the whole suite costs its longest run.

**Why P1**: The vehicle of every scenario.

**Acceptance Criteria**:

1. The package `tests/e2e/integrated/` SHALL declare exactly the runs `pi_loop`, `pwm_filter`, `sample_hold`, `state_machine`, `threshold`, and SHALL check with no diagnostic.
2. WHEN each run's design is elaborated and compiled THEN `rustc` SHALL print no warning and every body that runs SHALL have a shape `piperine_abi::validate` accepts.
3. WHEN every run has been opened THEN `tests/e2e/integrated/` SHALL hold only `Piperine.toml` and its `src/*.msl` files.
4. The target `integrated` SHALL execute the five runs once, at once, each in a build directory of its own.
5. IF a run fails THEN each test that reads it SHALL fail with the run's message, and the run SHALL NOT execute again.
6. WHEN the batch completes THEN the target SHALL print each run's wall time and rows and the batch's wall time.
7. WHEN a run ends THEN its `CompletedRun` SHALL hold every record its streams delivered, in delivery order, beside its `CheckTally`.

**Independent Test**: `cargo test -p piperine --test integrated -- --nocapture` prints five wall times and passes.

---

### P1: The comparator counter (`threshold`) ⭐ MVP

**User Story**: As a maintainer, I want a comparator watching an RC so that the analog-to-digital boundary and located crossings are proven in seconds.

**Why P1**: Replaces `buck_boundary`'s comparator and the buck's comparators.

**Acceptance Criteria**:

1. WHEN `threshold` ends at 42 µs THEN the counter of rising comparator edges SHALL read 4 and the counter of falling edges SHALL read 4.
2. The `edges` stream SHALL hold exactly eight records, alternating rising and falling, the first rising.
3. WHEN a rising record is delivered at time `t` THEN the closed-form capacitor voltage at `t` SHALL equal 0.55 V under `TRAN_VOLTAGE`, and a falling record's SHALL equal 0.45 V.
4. The `n`-th rising record SHALL fall inside the `n`-th high half period of the square, and the `n`-th falling record inside the `n`-th low half period after it.
5. WHILE `threshold` runs, the printed capacitor voltage at every row SHALL equal its closed form under `TRAN_VOLTAGE`.

**Independent Test**: `cargo test -p piperine --test integrated threshold::`.

---

### P1: The sample-and-hold (`sample_hold`) ⭐ MVP

**User Story**: As a maintainer, I want a clocked sample-and-hold and ADC so that sampling, quantization and a commit driving an analog switch are proven in seconds.

**Why P1**: Replaces `buck_boundary`'s ADC and the buck's measurement chain.

**Acceptance Criteria**:

1. WHEN the host sets the source to 0 V at 0, 1/256 V at 10.25 µs, 0.5 V at 20.25 µs, 255/256 V at 30.25 µs and 1.2 V at 40.25 µs THEN the code committed for the samples at 1 to 10 µs SHALL be 0, at 11 to 20 µs 1, at 21 to 30 µs 128, at 31 to 40 µs 255 and at 41 to 50 µs 255.
2. WHEN the sample at falling edge `k` is taken THEN its code SHALL appear `tconv` = 200 ns later and `done` SHALL be high for exactly 50 ns from that instant.
3. WHILE the clock is low (hold), the hold node SHALL equal the value it had at the start of the hold under `TRAN_VOLTAGE`, even across a source step inside the hold.
4. WHEN a track phase ends THEN the hold node SHALL equal the source under `TRAN_VOLTAGE`.

**Independent Test**: `cargo test -p piperine --test integrated sample_hold::`.

---

### P1: The PWM filter (`pwm_filter`) ⭐ MVP

**User Story**: As a maintainer, I want a counter PWM with dead time into an RC so that DPWM timing and the digital-to-analog drive are proven in seconds.

**Why P1**: Replaces `buck_dpwm` and the buck's power stage drive.

**Acceptance Criteria**:

1. WHILE a period runs with duty `d` latched, the high side SHALL be high for exactly `d` clock periods, and the low side for exactly `100 - d - 4`.
2. WHEN the high side falls or before it rises THEN both sides SHALL be low for exactly 2 clock periods.
3. The high side and the low side SHALL never be high in the same row, and the `checks` stream SHALL hold one passing `never` record per rising clock edge and no failing one.
4. WHEN the host drives the duty from 25 to 60 at 30.3 µs THEN the period that starts at the wrap of 30.005 µs SHALL still use 25, and every period from the wrap of 31.005 µs on SHALL use 60.
5. WHILE `pwm_filter` runs, the printed filter voltage at every row SHALL equal its piecewise-exponential closed form under `TRAN_VOLTAGE`.

**Independent Test**: `cargo test -p piperine --test integrated pwm_filter::`.

---

### P1: The state machine (`state_machine`) ⭐ MVP

**User Story**: As a maintainer, I want an enum-carried state machine driving analog switches with an over-voltage hiccup so that the supervisor's mechanisms are proven in seconds.

**Why P1**: Replaces `buck_supervisor`, S5 to S8 of `buck_scenarios` and `buck_power`'s switch.

**Acceptance Criteria**:

1. WHEN each rising clock edge passes THEN `phase` SHALL equal the reference state machine of `design.md` fed with `en` and `ov` as the table holds them at that edge, at every edge of the run.
2. The table SHALL print `phase` as the declaration index of its variant (`Idle` 0 to `Fault` 4) and `fault` as `Clear` 0 or `OverVoltage` 1.
3. WHEN a `Charge`, a `Hold` or a `Dump` phase ends THEN the capacitor voltage SHALL equal its closed form under `TRAN_VOLTAGE`, and WHILE in `Hold` it SHALL stay at its value at the start of the phase under `TRAN_VOLTAGE`.
4. WHEN the host has stepped the source to 2 V THEN the `trips` record SHALL be delivered at a time where the closed-form capacitor voltage equals 0.91 V under `TRAN_VOLTAGE`.
5. WHEN `ov` has been high at two consecutive rising edges THEN `phase` SHALL be `Fault` and `fault` `OverVoltage` from the second, with the charge switch off and the dump switch on.
6. WHILE in `Fault`, the phase SHALL last exactly 30 clock periods and then enter `Charge` (the retry).
7. WHEN a retry's `Charge` ends with no over-voltage after the host stepped the source back to 1 V THEN `fault` SHALL be `Clear` from that edge and the cycle `Charge`, `Hold`, `Dump` SHALL resume.
8. The `phase` stream SHALL hold one record per change of `phase`, at the edge that made it, carrying the new phase's index.

**Independent Test**: `cargo test -p piperine --test integrated state_machine::`.

---

### P1: The closed loop (`pi_loop`) ⭐ MVP

**User Story**: As a maintainer, I want a short closed loop with a PI in reactions so that closed-loop mixed-signal regulation is proven in seconds.

**Why P1**: Replaces `buck_closed_loop`, `buck_long`, `buck_control` and the regulation half of `buck_scenarios`.

**Acceptance Criteria**:

1. WHEN each sample's code is committed THEN the duty committed for it SHALL equal the Q6 PI reference of `design.md` fed with the committed codes, for every sample, bit for bit.
2. WHILE `pi_loop` is past 50 µs, every committed code SHALL be within 128 ± 3.
3. WHILE `pi_loop` is past 50 µs, the `settle` check `regulated` SHALL deliver records, and every one SHALL pass.
4. The `loop` stream SHALL hold exactly one record per sample (100), each with the sample's code, error and duty.
5. WHEN `pi_loop` ends THEN its table SHALL have exactly 10 001 rows, row `k` at `k · 10 ns`.
6. WHEN the first samples see the full error THEN the duty SHALL saturate at 100 and the integrator SHALL hold while the output is saturated in the error's direction.

**Independent Test**: `cargo test -p piperine --test integrated pi_loop::`.

---

### P1: The analog half against ngspice ⭐ MVP

**User Story**: As a maintainer, I want the square-driven RC cross-checked against ngspice so that the analog response the scenarios rely on is independently held.

**Why P1**: "ngspice parity untouched", and the suite's closed forms need an independent check of the integrator on square edges.

**Acceptance Criteria**:

1. WHEN `RcTop`'s node port is driven by `Session::source` with the corners of `tran_square_rc.cir` and run to 42 µs with `--max-step 1e-8` THEN `v(out)` at every ngspice row SHALL equal ngspice's under `TRAN_VOLTAGE`.
2. The twin `tran_square_rc.cir` SHALL give ngspice the step cap 10 ns (`TMAX`) that the piperine run uses.
3. The feature SHALL edit no `ToleranceContract`, no twin golden, no transpile golden and no existing parity assertion.

**Independent Test**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test source_host`.

---

### P1: The binary on a whole mixed design ⭐ MVP

**User Story**: As a CLI user, I want the binary held on a closed-loop design so that its tables and checks are proven without the buck.

**Why P1**: Replaces `piperine-cli`'s `buck`.

**Acceptance Criteria**:

1. WHEN `piperine run pi_loop` runs in `tests/e2e/integrated` THEN it SHALL exit 0, print `run pi_loop: transient, top PiLoop, backend rust` first, and print a table of exactly 10 001 rows whose last row's code is within 128 ± 3.
2. WHEN `piperine run pi_loop --records <file>` runs THEN no `checks` line SHALL print on standard output and every `checks` record of the file SHALL pass, with more than 50 of them.
3. WHEN `piperine test pi_loop threshold` runs THEN it SHALL exit 0 and print `PASS pi_loop (`, `PASS threshold (` and `2 runs, 2 passed, 0 failed`.

**Independent Test**: `cargo test -p piperine-cli --test integrated`.

---

### P1: The Book stays honest ⭐ MVP

**User Story**: As a reader, I want every Book listing still held to what the binary does so that moving the buck does not let the Book drift.

**Why P1**: The Book guards (MD-72) must hold after the move.

**Acceptance Criteria**:

1. WHEN `piperine-cli`'s `book` builds its listing fixture THEN it SHALL skip every listing of project `buck`, and `every_cli_listing_of_the_book_is_what_the_binary_prints` SHALL hold every other listing, their count lowered by exactly the one listing moved.
2. WHEN `cargo test -p piperine-cli --test book_buck` runs THEN `the_buck_block_is_what_the_binary_prints` SHALL hold every listing of project `buck` to what the binary prints in `examples/buck`, whole, and their commands SHALL be exactly `piperine run startup`, as the moved assertion says.
3. `piperine-cli`'s `book` SHALL keep a test that the chapter shows `examples/buck/benches/startup.msl`, the `[run.startup]` table and `piperine run startup` as the example has them.
4. WHEN `cargo test -p piperine --test book_buck` runs THEN the 2 µs run of `cli#44` SHALL show the supervisor off and the reference 0 in its five rows, and `book_cli` SHALL keep `cli#44`'s text equality.
5. The rows of `tests/book/inventory.tsv` for `cli#44`, the buck listing block and the claims they held SHALL name the on-demand tests, and `book_inventory` and `book_claims` SHALL pass.

**Independent Test**: `cargo test -p piperine-cli --test book` and `cargo test -p piperine --test book_inventory --test book_claims` pass in minutes.

---

### P2: Language proofs leave the buck's name ⭐

**User Story**: As a maintainer, I want the commit, record and time proofs to stand on their own clock so that no default target depends on running the buck package.

**Why P2**: They already run fast; the rename only separates them from the buck.

**Acceptance Criteria**:

1. `piperine-parity` SHALL have targets `source_commits`, `source_records` and `source_time` with the tests of `buck_commits`, `buck_records` and `buck_time`, names and assertions unchanged, and no `buck_commits`, `buck_records` or `buck_time` target.
2. Their fixtures SHALL live under `crates/piperine-parity/tests/digital/` and SHALL NOT name the crate `buck`.

**Independent Test**: `cargo test -p piperine-parity --test source_commits --test source_records --test source_time`.

---

### P2: The documents say how to run the buck ⭐

**User Story**: As a maintainer, I want the on-demand command written where I look so that "for ourselves" stays possible.

**Why P2**: The buck is kept to be run.

**Acceptance Criteria**:

1. `examples/buck/README.md` SHALL hold an "On demand" block with the four `cargo test` lines of D15 naming every target of `ON_DEMAND_TARGETS`, and SHALL keep the strings `docs.rs` already holds it to.
2. `CLAUDE.md`'s "Build and test" SHALL name the integrated target as the per-task gate and point at the README's on-demand block for the buck.
3. `.specs/STATE.md` SHALL record MD-77: the mechanism (D1, D2), the policy of D11, and the gate migration lines of this spec.

**Independent Test**: `cargo test -p piperine-cli --test docs` passes; MD-77 reads as written.

---

## Gate migration

The replacement lines each in-flight feature applies to its `tasks.md` at its next rebase onto `main` (D12).
"Integrated targets" is the line `cargo test -p piperine --test integrated; cargo test -p piperine-cli --test
integrated; cargo test -p piperine-parity --test source_commits --test source_records --test source_time`.
"The buck run" is the README's on-demand block.

| Feature (branch) | Lines that cite the buck | Replacement |
| ---------------- | ------------------------ | ----------- |
| every one below | the header line `Buck targets: cargo test -p piperine-parity --test buck_whole ... --test buck_power` | `Integrated targets: <the line above>` and `The buck run (on demand): examples/buck/README.md, "On demand"` |
| every one below | the "Parity is untouchable" sentence "Every buck target passes after every task that touches ..." | "Every integrated target passes after every task that touches ...; the buck run passes once at the feature's close, before merge" |
| every one below | the `Full` gate row "Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins [--test commands]`, `cargo test -p piperine-api --test buck_long`, ..." | "Quick, then the integrated targets, `cargo test -p piperine-cli --test twins [--test commands]`, ..." (the rest of the row as it is) |
| `composite-values` | T26 (line 1192) "Every buck target passes with unchanged assertions"; B8 note on T48 | "Every integrated target passes with unchanged assertions"; T48's note stays (`elab_buck`'s pin is a default static proof) |
| `resolved-nets` | lines 36, 47, 89 | the three common rows |
| `unknown-semantics-per-level` | T8 (line 334) "Every buck target passes with only the listed assertions moved"; B2 note | T8 edits assertions inside buck targets, so its gate is the integrated targets **and** the buck targets it edits, run by name (D11) |
| `analog-operators` | line 359 (done) "every `buck_*` target ... pass unedited" | done, historical; B4 to B7 gate with the integrated targets |
| `generics` | T32 (line 840) "Every buck target passes"; B5 note | "Every integrated target passes"; the buck run at the feature's close |
| `params-and-io` B5, B6 | T26 (line 777: it edits `examples/buck/src`), line 846 (rebase), line 913 (byte-identical) | T26 keeps the buck run (D11: it edits the example); line 846: "the `**Build**` line and the integrated targets pass at the rebased `HEAD`"; line 913: "the transpile goldens are byte-identical and every integrated target passes" |
| `unknown-semantics-events` B5 | lines 37, 48, 88 (B5 = T24 to T31) | the three common rows; T5's done-when citing `the_buck_block_is_what_the_binary_prints` under `clibook` is done and historical (the test now runs under `--test book_buck`) |

---

## Edge Cases

- IF a run of the batch fails THEN its tests SHALL report the failure once each, with no second run (IT-03 AC 5).
- IF a measured event never happens (no comparator edge, no trip, no `Fault`) THEN its test SHALL fail naming the event, never pass vacuously (IT-04 AC 2, IT-07 AC 4).
- IF a record stream is empty THEN its test SHALL fail, never pass vacuously (IT-06 AC 3, IT-08 AC 3).
- WHEN the ngspice binary is absent THEN `source_host` SHALL behave as for `tran_pwl_rc` today (IT-09).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| IT-01 | P1: The buck leaves the default run | Tasks | Pending |
| IT-02 | P1: No test is switched off unregistered | Tasks | Pending |
| IT-03 | P1: The integrated package and its runs | Tasks | Pending |
| IT-04 | P1: The comparator counter | Tasks | Pending |
| IT-05 | P1: The sample-and-hold | Tasks | Pending |
| IT-06 | P1: The PWM filter | Tasks | Pending |
| IT-07 | P1: The state machine | Tasks | Pending |
| IT-08 | P1: The closed loop | Tasks | Pending |
| IT-09 | P1: The analog half against ngspice | Tasks | Pending |
| IT-10 | P1: The binary on a whole mixed design | Tasks | Pending |
| IT-11 | P1: The Book stays honest | Tasks | Pending |
| IT-12 | P2: Language proofs leave the buck's name | Tasks | Pending |
| IT-13 | P2: The documents say how to run the buck | Tasks | Pending |

**Coverage:** 13 requirements, 13 mapped to tasks, 0 unmapped.

---

## Sources

- The user's words (relayed): "lança um agent para compor um conjunto de testes integrados. Deixa o buck só como example, para nós mesmos."
- Every buck target's `//!` header and tests at `main` `856d2668` (Coverage inventory).
- `.specs/STATE.md` MD-28, MD-33, MD-38, MD-72, MD-73 - placement, hygiene, tolerances, the root's integrated tests, scenario packages.
- `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-solver/tests/workspace_shape.rs` - the guards.
- `crates/piperine-parity/src/scenario/` - the harness; `crates/piperine-parity/tests/buck_boundary.rs` - the crossing-by-voltage rule; `crates/piperine-parity/tests/source_host.rs`, `tests/e2e/twins/tran_pwl_rc.cir` - the host-driven twin precedent.
- `crates/piperine-lang/std/convert.msl` (`D2A`, `A2D`), `std/analog.msl` (`transition`), `std/monitor.msl` (`Never`).
- Cargo's `test = false` target flag, verified on a scratch package with this machine's cargo.
- The in-flight `tasks.md` of `composite-values`, `resolved-nets`, `unknown-semantics-per-level`, `analog-operators`, `generics`, `params-and-io`, `unknown-semantics-events` at their branch heads (Gate migration).

---

## Success Criteria

- [ ] `cargo test` at the root lists and runs no buck simulation target.
- [ ] `cargo test -p piperine --test integrated` passes in under 3 minutes of wall time in a debug build, compile included (reported in the Handoff).
- [ ] The buck run of the README executes every on-demand target by name (its six known `buck_scenarios` reds unchanged and reported).
- [ ] No tolerance, contract, golden or parity assertion changed.

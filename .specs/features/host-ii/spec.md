# Host II Specification

**Approved**: 2026-11-17 - user: "Topo" (lanes) and "Aceito todas (Recommended)" (Q1-Q3, tnom)

## Questions answered (user, 2026-11-17)

The user's decisions X3, X4, X7, X8 and X9 are settled (`context.md`). The three surface questions and the `tnom` departure were answered "Aceito todas (Recommended)".

| # | Question | Decided |
| - | -------- | ------- |
| Q1 | How a user lists a design's metadata from the command line | (a) a new command `piperine info [<run>]` (HII-11) |
| Q2 | Is a timed `set` also a run key | (a) library only, `Session::set_at` (HII-13) |
| Q3 | Run keys of the small-signal analyses | (a) `input` and `output` for `tf` and `pz`; `wrt` for `sens`, whose outputs are `print` |
| - | `tnom` | stays out until `simparam` (L47) lands |

-------- | ------- | -------------- |
| Q1 | How does a user list a design's metadata (params with type, default, `where` and structural flag; ports; streams) from the command line? | (a) a new command `piperine info [<run>]` that elaborates the run's top and prints three tables, exit 0, nothing simulated; (b) a flag `piperine check --list`; (c) the library only (`Session::metadata`), no command-line surface in this feature | **(a)**: metadata is a host operation of §21 of its own; `check` stays the pass or fail of a project, and the Book gets one short listing. HII-11 assumes (a); with (c) HII-11 is dropped and HII-10 stands |
| Q2 | Is a timed `set` (a param changed at a time during a transient) also a run key with its flag, or only a library call like the drive (X8)? | (a) library only: `Session::set_at`, used by Rust host tests; (b) also a run key `sets = [{ at = 1e-3, path = "r1.r", value = 2e3 }]` with a repeatable flag `--set <path>=<value>@<time>` | **(a)**: it matches X8 ("benches in MSL stay the command-line path"); a bench already changes its own behavior over time, and the run table stays the observation and session policy. With (b) one task is added to Phase 6 and HII-13 gains a criterion |
| Q3 | Which run keys name the small-signal analyses' input, output and params? | (a) `input` (the instance path of a voltage source) and `output` (a node path) for `tf` and `pz`; `wrt` (a list of param paths) for `sens`, whose outputs are the run's `print`; (b) ngspice's spellings: `src` and `out`, and `params` for `sens` (which collides with the existing `params` key of overrides) | **(a)**: plain words, no collision with `params`, and `print` already means "the outputs" |

---

## Problem Statement

The command line and `piperine-api` run four analyses (`op`, `dc`, `ac`, `transient`) with the solver's default options. The solver also has transfer function, pole-zero, sensitivity and periodic steady state (`piperine-solver/src/core/circuit.rs:111` to `:172`), and Part VIII §23 gives the host session options, metadata reads, drives and timed `set`s that no host offers (`GAPS.md` §7, H1 to H4, H9). Three silent or loud defects sit on the same path: a `set` of a param that a parent passes down changes nothing below it (S2), a digital-only transient is refused (L56), and `piperine test` reads neither a `fatal` nor a `target` margin (S1's host half, X3, X4).

## Goals

- [ ] A run sets `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `nodeset`, `resolution`, `maxrounds` and `maxmicro` from its table or its flags, and the 7 twins print what they printed with none given.
- [ ] `analysis = "tf"`, `"pz"`, `"sens"` and `"pss"` run from a bench and from the command line, each proven against ngspice on a twin of its own.
- [ ] A Rust host drives a top's discrete `in` port and changes a param at a time during a transient, refused with the spec's `E08xx` codes where §23.1 says.
- [ ] A host `set` of a param recomputes every value derived from it, on the compiled design, with no re-elaboration and no recompile; S2 is closed.
- [ ] `piperine test` fails a run that called `fatal`, and prints each `target`'s last margin without failing on it.
- [ ] A digital-only transient runs to its stop.
- [ ] The Book shows the new keys, analyses and command, held to the binary.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Integrated tests of every Book example and claim at the root; the root `tests/e2e/` with the ngspice twins moved under it (ROADMAP items 4 and 4b, `GAPS.md` H11, U10) | The follow-up feature `root-e2e`, planned in this branch as a spec (`.specs/features/root-e2e/spec.md`). Host II alone is over 30 tasks |
| `force` and `release` (§23.1 rule 3) | The digital engine has no override of a carrier's drives (`piperine-digital-solver/src/engine.rs:199` queues a host commit only); a new engine mechanism in the solver core. Follow-up host feature |
| A host wait on an event of the model (§23.1 rule 2) | The engine's waiter (`engine.rs` `wait`) fires on event nets only, and the events of a model are private pins (`event:<k>`, `piperine-plan/src/digital.rs:339`) a host cannot name. Follow-up host feature |
| Continued transients and an analysis at a transient's last instant (H6, §16.4) | Re-entry is approximate today (`GAPS.md` V8, lane C) and §16.4 rule 2 needs an operating point taken from the transient's final state in the protocol. Follow-up host feature |
| Operating-point variables (`--print Top.g.id`, H4) | No backend exports a named `let` of `analog` as a `VAR` watch (`piperine-plan` emits `CROSS` watches only); that is plan and transpile work, lane A's area. Follow-up |
| Noise analysis (H2) | MSL has no noise source (`GAPS.md` L45): every density would be zero |
| S-parameters, adjoint gradients, Monte Carlo, optimization (H2, H7, H8) | Need ports of their own (`sp.rs` still documents PHDL's `@rfport`, V6) or the gradient machinery of §23.3 |
| A `drive` key or flag in the manifest | X8: the host drives a top only through `piperine-api` |
| `tnom` as a run key | No MSL construct reads it: `simparam` is refused (`GAPS.md` L47), so the key would change nothing, silently. It lands with L47 (user, 2026-11-17: "Aceito todas (Recommended)") |
| The default `ttol` of `cross` as a session option | The solver has no session-level default; each `CROSS` watch carries its own `DEFAULT` pin (`piperine-analog-solver/src/device/watch.rs:49`) |
| Ending the analysis at `finish` or `fatal` | Lane A (`msl-fixes`, S1). Host II reads the `sim` record whatever the analysis does after it |
| A top with a continuous port (L39) | X8 leaves it: the host drives discrete ports only; L39 stays in `GAPS.md` |
| Python (B4), the language server (B3) | User: out of this lane |
| Solver numerics V1 to V11, the naming of solver errors | Lane C (`solver-hygiene`) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| X9 Derived params under a `set` | recomputed by a function compiled into the design (generated Rust); no re-elaboration, no recompile | "Recalcular no código gerado (Recommended)" | y - user |
| X8 Driving the top | `piperine-api` only, discrete `in` ports of the top; no manifest key | "Só pela API host (Recommended)" | y - user |
| X3 A `target` | `piperine test` prints each target's last margin at the end and never fails on it | "Só informa (Recommended)" | y - user |
| X4 `finish` and `fatal` | a `sim` record with `failed = true` fails the run (exit 1) and `piperine test` prints `FAIL` with its `msg`; `finish` alone is not a failure | "Encerra e falha (Recommended)" | y - user |
| X7 Option names | `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `nodeset` as ngspice spells them, lowercase; `resolution`, `maxrounds`, `maxmicro` for the three options ngspice has no name for | the orchestrator's proposal, not objected to; ngspice has no time resolution and no bound on rounds or microsteps, so the names are the spec's own terms, lowercase, no separator, as ngspice writes `reltol` | y - user (names); n - planner (the last three) |
| `tnom` | not a key in this feature | Nothing reads it (L47); a key that changes nothing is a silent one | y - user: "Aceito todas (Recommended)" |
| `temp` unit | degrees Celsius, as ngspice's `.options temp`; the solver gets `temp + 273.15` kelvin | the key is ngspice's, so its unit is too | n - planner |
| Q1 Metadata surface | `piperine info [<run>]` | recommended option | y - user: "Aceito todas (Recommended)" |
| Q2 Timed `set` surface | library only | recommended option | y - user: "Aceito todas (Recommended)" |
| Q3 Key names | `input`, `output`, `wrt` | recommended option | y - user: "Aceito todas (Recommended)" |
| Which analyses join | `tf`, `pz`, `sens`, `pss` | each runs on the device ABI today (`tests/pss.rs`, `abi_sens.rs`, `pz_device_fault.rs` in `piperine-solver`) and ngspice has the same quantity: `.tf`, `.pz ... vol`, `.sens`, and a long `.tran` for a steady state (probed on ngspice-47, 2026-11-17) | n - planner |
| PSS against ngspice | the last period of an ngspice transient long enough to settle, on ngspice's own print grid, plus the closed form of a square-driven RC | ngspice's `.pss` is built for autonomous oscillators: on a driven RC it moved the fundamental to 900 Hz (probe, 2026-11-17) | n - planner |
| `tf` and `pz` input | a voltage source: an instance whose `analog` writes a potential contribution; a current input is refused naming the instance | MD-14 (TF is voltage-source only); the solver's PZ takes a branch the same way (`pz.rs:53`) | n - planner |
| New parity tolerances | three new `ToleranceContract` constants for the new domains (transfer ratio and impedance, pole and zero, sensitivity), each with a rationale; no existing constant moves | MD-38: each domain has its own documented bound. PSS and temperature reuse `TRAN_VOLTAGE` and `DC_VOLTAGE` | n - planner |
| The rows of a digital-only transient | the grid's rows with `print_step`; without it the start and the stop, the two times the solver observes | The solver jumps a design with no analog unknown from instant to instant and observes the step at the stop only (`protocol/transient.rs:252`) | n - planner |
| The new parity twins | `temp_res`, `tf_divider`, `pz_lowpass`, `sens_divider`, `pss_rc`, each a `.msl` and a `.cir` under `tests/ngspice/` with a test function of its name in a `parity_*` target, as `parity_corpus` requires | the existing twin rule; the 7 twins and their goldens are not touched | n - planner |
| Who drives a top `in` port | the host holds its one drive (§23.1 rule 1); a port the host never drives stays unknown (rule 9); any other carrier is `E0804` | the spec | n - planner |
| Time of a host action | `at = 0` is an initial value (rule 4), `at > 0` a commit at that time; a negative `at` is `E0806` | A session runs once and synchronously, so "a time already past" can only be a time before 0 | n - planner |
| A `set` of a param that a parent derives | refused (`E0802`) naming the parent param to set | The child's value is the parent's function now; a write to it would be overwritten at the next change of the parent | n - planner |
| The interpreter under X9 | keeps the folded constants; a `set` of a param that a child derives from is refused there (`E0802`) naming `--backend rust` | the interpreter is frozen at the M1 subset (R1); a refusal is loud, a stale child is not | n - planner |

**Open questions:** none - all resolved or logged above. Q1 to Q3 were answered on 2026-11-17 (top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: HII-01 AC 3 to 6, HII-04 AC 4 and 5, HII-07 AC 5, HII-12 AC 3 and 4, HII-13 AC 3 to 5. Failure and partial-failure states: HII-15 (a `fatal` run), HII-17 AC 3, HII-02 AC 6 and 7 (a bound reached). Idempotency and retry: N/A because a session runs once (`msl-cli` CLI-04); a second `run` is refused as today. Auth boundaries and rate limits: N/A because the tool is local. Concurrency and ordering: HII-13 AC 2 (a timed `set` applies after the step to its time is accepted, §23.1 rule 4), HII-12 AC 2. Data lifecycle: HII-16 AC 1 (the last record of a target is kept, §23.2 rule 2). Observability: HII-10, HII-11, HII-16. External-dependency failure: HII-21 AC 3 (ngspice absent, the harness's skip rule). State-transition integrity: HII-14 (no subscription after the run starts), HII-18 AC 4 (a refused `set` changes nothing).

---

## User Stories

### P1: Session options from the run ⭐ MVP

**User Story**: As a designer, I want to set the solver's tolerances, the temperature, nodesets, the time resolution and the bounds of a run in its table or by flags, so that I can tighten a run or reproduce an ngspice deck without editing the model.

**Why P1**: §23 lists these as the host's and none is reachable (H3, X7).

**Acceptance Criteria** (HII-01, the keys):

1. The system SHALL accept in a run table the keys `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `resolution`, `maxrounds`, `maxmicro` and `nodeset`, for every analysis.
2. The system SHALL give each of those keys a flag of `piperine run` (`--reltol`, `--abstol`, `--vntol`, `--trtol`, `--temp`, `--resolution`, `--maxrounds`, `--maxmicro`, and `--nodeset <path>=<value>` repeatable), and a flag SHALL win over the key it shares.
3. IF `reltol`, `abstol`, `vntol`, `trtol` or `resolution` is not a number greater than 0 THEN the system SHALL report `P0010` naming the key.
4. IF `temp` is not a number greater than -273.15 THEN the system SHALL report `P0010` naming the key.
5. IF `maxrounds` or `maxmicro` is not an integer of at least 1 THEN the system SHALL report `P0010` naming the key.
6. IF a `nodeset` entry names no node of the elaborated design THEN the system SHALL report `E0801` naming the entry before any analysis runs.

**Acceptance Criteria** (HII-02, what they change):

1. WHEN a run gives none of the HII-01 keys THEN the system SHALL run every analysis with the options it used before this feature.
2. WHEN a run gives `reltol`, `abstol`, `vntol` or `trtol` THEN the system SHALL hand the solver those values in place of the defaults, for every analysis of the run.
3. WHEN a run gives `temp = c` THEN the system SHALL make `temperature()` read `c + 273.15` in every instance.
4. WHEN a run gives `nodeset` THEN the system SHALL seed the first Newton iterate of the operating point with those node values, and the operating point SHALL still converge to the circuit's solution.
5. WHEN a run gives `resolution = r` THEN the system SHALL bind the design with a tick of `r` seconds, so that an `at(t)` fires at `t` rounded to a multiple of `r`.
6. WHEN an operating point needs more rounds than `maxrounds` THEN the system SHALL fail the run with the solver's `E0506` error naming the bound.
7. WHEN an instant needs more microsteps than `maxmicro` THEN the system SHALL fail the run with an error naming the bound.
8. WHEN `tran_rc` runs to 10 µs with `trtol = 0.001` THEN the system SHALL take more accepted steps than with no `trtol`: `trtol` reaches the solver's step assessment, and a smaller value rejects at least as many steps. (Amended 2026-11-17: the original wording, `trtol = 70` taking fewer accepted steps, moved to `solver-hygiene` (LTE), because today `trtol` only gates rejection and does not shape the step.)

**Acceptance Criteria** (HII-03, temperature against ngspice):

1. WHEN the twin `temp_res` (a resistor with a first-order temperature coefficient driven by a source) runs `op` with `temp = 75` THEN `v` of its output node SHALL agree with ngspice's `.options temp=75` operating point under `ToleranceContract::DC_VOLTAGE`.
2. WHEN `temp_res` runs `op` with no `temp` THEN its output SHALL agree with ngspice at 27 °C under the same contract.

**Independent Test**: `piperine run tests/ngspice/temp_res.msl --analysis op --temp 75` prints the value ngspice prints.

---

### P1: Transfer function, pole-zero and DC sensitivity ⭐ MVP

**User Story**: As a designer, I want the small-signal analyses the solver already has, so that I read a gain, an impedance, a pole or a sensitivity without a second simulator.

**Why P1**: H2; the solver runs them on the device ABI today and the host cannot ask for them.

**Acceptance Criteria** (HII-04, `tf`):

1. WHEN a run gives `analysis = "tf"`, `input` (the instance path of a voltage source) and `output` (a node path) THEN the system SHALL give a table with the columns `quantity` and `value` and the rows `gain`, `input_impedance` and `output_impedance`, in that order.
2. WHEN the twin `tf_divider` runs `tf` from `v1` to its output node THEN each of the three values SHALL agree with ngspice's `.tf` (`transfer_function`, `v1#input_impedance`, `output_impedance_at_v(out)`) under the new transfer constant of `ToleranceContract`.
3. IF a `tf` run lacks `input` or `output` THEN the system SHALL report `P0010` naming the key.
4. IF `input` names an instance with no potential contribution (a current source, a resistor) THEN the system SHALL report `P0010` naming the instance and saying that the input must be a voltage source.
5. IF `output` names no node of the design THEN the system SHALL report `E0801` naming it.

**Acceptance Criteria** (HII-05, `pz`):

1. WHEN a run gives `analysis = "pz"`, `input` and `output` THEN the system SHALL give a table with the columns `kind`, `real` and `imag` (radians per second), one row per pole with `kind` `pole`, then one row per zero with `kind` `zero`, in the solver's order.
2. WHEN the twin `pz_lowpass` (a voltage source, a series resistor, a shunt resistor and a shunt capacitor) runs `pz` THEN its poles and zeros SHALL agree, root for root, with ngspice's `.pz <in> 0 <out> 0 vol pz` under the new pole-zero constant of `ToleranceContract`.
3. IF the circuit has no finite pole THEN the system SHALL fail the run with the solver's error, and print no table.
4. The system SHALL apply HII-04 AC 3 to 5 to a `pz` run.

**Acceptance Criteria** (HII-06, `sens`):

1. WHEN a run gives `analysis = "sens"` and `wrt` (a list of param paths) THEN the system SHALL give a table whose first column is `param` and which has, per printed signal, a column `d(<path>)`, one row per `wrt` entry in order, each cell the derivative of the signal with respect to the param at the operating point.
2. WHEN the twin `sens_divider` runs `sens` of its output with respect to both resistances and the source's value THEN each derivative SHALL agree with ngspice's `.sens` of the same output under the new sensitivity constant of `ToleranceContract`.
3. IF a `sens` run lacks `wrt` or `wrt` is empty THEN the system SHALL report `P0010` naming the key.
4. IF a `wrt` entry names no param THEN the system SHALL report `E0801` naming it.
5. IF a printed signal of a `sens` run is a discrete net THEN the system SHALL report `P0010` naming it: a sensitivity is of a node or a flow.

**Independent Test**: `piperine run tests/ngspice/tf_divider.msl --analysis tf --input v1 --output out` prints the three rows ngspice prints.

---

### P1: Periodic steady state ⭐ MVP

**User Story**: As a designer of a switching circuit, I want `analysis = "pss"` with `period` and `tstab`, so that I get one period of the steady state without simulating the whole settling.

**Why P1**: the user asked for PSS in a bench and on the command line by name.

**Acceptance Criteria** (HII-07):

1. WHEN a run gives `analysis = "pss"` and `period = T` THEN the system SHALL give a table whose first column is `time` and whose rows cover one period of the converged orbit, from its start `t0` to `t0 + T`, one row per accepted step of that period, with one column per printed signal.
2. WHEN a `pss` run gives `print_step = h` THEN the system SHALL give one row per `t0 + k*h` up to `t0 + T`, by the rules of the transient grid (`long-run` LR-05).
3. WHEN a `pss` run gives `tstab = s` THEN the system SHALL run a transient over `[0, s]` before the first shot, and `t0` SHALL be `s`.
4. WHEN the twin `pss_rc` (an RC driven by a square wave of period `T`, made by a reaction of the bench) runs `pss` with `print_step` equal to ngspice's print step THEN each row's output SHALL agree, under `ToleranceContract::TRAN_VOLTAGE`, with the row of ngspice's transient at the same phase of its last period, and with the closed form `1 / (1 + exp(-T / (2RC)))` at the end of the high half-period.
5. IF a `pss` run lacks `period`, or `period` is not greater than 0, or `tstab` is negative THEN the system SHALL report `P0010` naming the key.
6. IF the shooting does not converge THEN the system SHALL fail the run with the solver's error naming the residual, and print no table.
7. WHEN a `pss` run gives `plot` THEN the system SHALL draw the rows of its table, as a transient's plot.

**Independent Test**: in a bench, `[run.steady] analysis = "pss" period = 1e-3 tstab = 5e-3`, then `piperine run steady`.

---

### P1: The new analyses on the command line ⭐ MVP

**User Story**: As a user of `piperine run`, I want the new analyses printed like the others.

**Why P1**: the command line is the user's entry point (MD-65).

**Acceptance Criteria** (HII-08):

1. WHEN a `tf`, `pz`, `sens` or `pss` run completes THEN the system SHALL print the header line, the record lines and the table as `msl-cli` CLI-07 AC 1 says, with the analysis named `tf`, `pz`, `sens` or `pss`.
2. WHEN a `tf`, `pz`, `sens` or `pss` run gives `csv` THEN the system SHALL write its table as CSV.
3. IF a `tf`, `pz` or `sens` run gives `plot` THEN the system SHALL report `P0010` naming the key and the analysis.
4. The system SHALL list `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens` and `pss` as the valid values of `analysis`.

**Independent Test**: `cargo test -p piperine-cli --test analyses`.

---

### P2: Metadata

**User Story**: As a host author, I want to read back a param's value, a flow through any instance, and the list of a design's params, ports and streams, so that I know what I can set and print.

**Why P2**: §21's host operations; the runs work without them.

**Acceptance Criteria** (HII-09, printing):

1. WHEN `print` names `<instance>.<param>` THEN the system SHALL print the param's value as the run used it (the elaborated value, or the value a `set` gave), in every row.
2. WHEN `print` names `i(<instance>)` of an instance with no potential contribution and exactly one flow contribution THEN the system SHALL print that contribution's value at each solution.
3. IF `print` names `i(<instance>)` of an instance with several flow contributions and no potential one THEN the system SHALL report `E0801` naming the instance and its contributions.

**Acceptance Criteria** (HII-10, the library's metadata):

1. WHEN a host asks a session for its metadata THEN the system SHALL give every param of every instance with its full path, type, value, `where` text (or none) and whether it is structural, in instance order.
2. WHEN a host asks a session for its metadata THEN the system SHALL give the top's ports with their direction and type, in declaration order, and every record stream with its field names and types.
3. The system SHALL give the metadata without running an analysis.

**Acceptance Criteria** (HII-11, the command; Q1 (a)):

1. WHEN `piperine info [<run>]` runs THEN the system SHALL print the run's top and backend, then the tables `params`, `ports` and `streams` of HII-10, and exit 0, simulating nothing.
2. IF the run cannot be elaborated THEN the system SHALL print the diagnostics as `piperine check` does and exit 1.

**Independent Test**: `piperine info` in `examples/rc` lists `r` and `c` of the low-pass.

---

### P2: Host drives and timed sets from the library

**User Story**: As a Rust host author, I want to drive a top's discrete inputs and change a param at a time during a transient, so that a host test can play a scenario the bench does not write.

**Why P2**: H1 and H9; the user restricted it to the host library (X8). The buck's scenarios S1b to S9 need it.

**Acceptance Criteria** (HII-12, drive):

1. WHEN a host drives a discrete `in` port of the top with a value at `at = 0` before the run THEN the system SHALL give the port that value as its initial value (§23.1 rule 4).
2. WHEN a host drives such a port at `at > 0` THEN the system SHALL commit the value at microstep 0 of `at`, and a reaction on the port's change SHALL fire at that instant.
3. IF a host drives a carrier that is not a discrete `in` port of the top THEN the system SHALL refuse with `E0804` naming the carrier, and change nothing.
4. IF a host drives with a value whose type is not the port's THEN the system SHALL refuse with `E0802` naming the port and the value.
5. WHILE a discrete `in` port of the top is never driven the system SHALL give it the unknown value (§23.1 rule 9).

**Acceptance Criteria** (HII-13, timed set; Q2 (a)):

1. WHEN a host sets a param at `at = 0` before the run THEN the system SHALL apply it as a `set` before the analysis.
2. WHEN a host sets a param at `at > 0` THEN the system SHALL apply it after the step to `at` is accepted (§23.1 rule 4), so that a node read by the param's behavior keeps its old value at `at` and changes after it.
3. IF a host sets a structural param at `at > 0` THEN the system SHALL refuse with `E0803` naming the param, before the run starts.
4. IF a host action gives a negative `at` THEN the system SHALL refuse with `E0806` naming the action.
5. IF a timed set's value fails the param's `where` THEN the system SHALL refuse it with `E0802` and run no analysis.

**Acceptance Criteria** (HII-14, subscription):

1. WHEN a host narrows a session's streams before the run THEN the system SHALL deliver the records of those streams only.
2. IF a host subscribes to a stream after the session has run THEN the system SHALL refuse with `E0805` naming the stream.

**Independent Test**: `cargo test -p piperine-api --test host_actions`.

---

### P1: What `piperine test` reads ⭐ MVP

**User Story**: As a user of `piperine test`, I want a run that called `fatal` to fail and my targets' margins shown, so that a regression that dies is red and an objective is visible.

**Why P1**: S1's host half is silent today: a `fatal` run passes.

**Acceptance Criteria** (HII-15, `fatal`):

1. WHEN a run delivers a `sim` record with `failed = true` THEN the system SHALL fail the run, print the record's `msg` on standard error, keep the records already printed, and exit 1.
2. WHEN `piperine test` judges a run that delivered a `sim` record with `failed = true` THEN the system SHALL print `FAIL <name> (fatal: <msg>)`.
3. WHEN a run delivers a `sim` record with `finish = true` and no `failed = true` THEN the system SHALL NOT count it as a failure.

**Acceptance Criteria** (HII-16, targets):

1. WHEN `piperine test` judges a run THEN the system SHALL keep the last `checks` record of kind `target` of each target name.
2. WHEN `piperine test` has printed the `PASS` or `FAIL` line of a run that delivered targets THEN the system SHALL print one line per target name, in the order of their first record: `  target <name> margin=<value>`.
3. The system SHALL NOT fail a run because of a target's margin, whatever its sign.

**Independent Test**: a bench calling `fatal("brownout")` makes `piperine test` print `FAIL <run> (fatal: brownout)` and exit 1.

---

### P2: A digital-only transient

**User Story**: As a designer of a digital block, I want a transient of a design with no analog node to run to its stop.

**Why P2**: L56; the Book's `Blinky` is refused today.

**Acceptance Criteria** (HII-17):

1. WHEN a transient of a design with no analog unknown reaches its stop THEN the system SHALL give a table and exit 0.
2. WHEN such a run gives `print_step` THEN the system SHALL give the grid's rows, each discrete signal holding its value after the last instant at or before the row (LR-05 AC 4).
3. IF such a run ends before its stop THEN the system SHALL fail it as CLI-05 AC 11 says.
4. WHEN the Book's `Blinky` runs to 3 µs THEN the system SHALL print its three `edge` records and a table, and exit 0.

**Independent Test**: `piperine run blinky.msl --analysis transient --stop 3e-6 --print-step 1e-6`.

---

### P1: A `set` recomputes what derives from it ⭐ MVP

**User Story**: As a designer who passes params down a hierarchy, I want a `set`, an override or a sweep of a parent's param to reach every child that reads it, so that the answer is not the unswept one.

**Why P1**: S2 is silent and reachable from the command line; the user decided X9.

**Acceptance Criteria** (HII-18, child params):

1. WHEN a host sets a param `p` of an instance before a run, on the Rust backend, THEN the system SHALL give every param of every descendant that the authored placements derive from `p` the value its expression gives with the new `p`, before the analysis.
2. WHEN `dc` sweeps a param from which a child's param derives THEN the system SHALL recompute the child's param at each point.
3. WHEN a host sets such a param at `at > 0` THEN the system SHALL recompute the derived params at the same instant as the set.
4. IF a recomputed child param fails its `where` THEN the system SHALL refuse the `set` with `E0802` naming the child param, and change nothing.
5. The system SHALL neither re-elaborate nor invoke `rustc` for a `set`.
6. WHEN `Mid { rr }` places `R { r: rr }` under 1 V across a 1 kΩ resistor in series, and `--param Top.m.rr=3000` is given THEN the system SHALL print 0.75 for the middle node.

**Acceptance Criteria** (HII-19, inside a body):

1. WHEN a host sets a param that a body-level `let` of the same module reads THEN the system SHALL give the `let` its value from the new param in every later evaluation.
2. WHEN a host sets a param that a definition of the same module reads THEN the system SHALL recompute the definition at the instant of the set.

**Acceptance Criteria** (HII-20, refusals):

1. IF a host sets a param whose value its parent's placement derives from the parent's params THEN the system SHALL refuse with `E0802` naming the parent param that derives it.
2. IF a host sets, on the interpreter, a param from which another param derives THEN the system SHALL refuse with `E0802` naming the derived params and `--backend rust`.
3. The system SHALL give, with no `set`, the same values as the folded constants of the elaborated design, so that every existing run prints what it printed.

**Independent Test**: `cargo test -p piperine-api --test derived`, the probe of `GAPS.md` S2.

---

### P1: Parity does not move ⭐ MVP

**User Story**: As the owner of the parity claim, I want every existing comparison untouched.

**Why P1**: ngspice parity is the top priority.

**Acceptance Criteria** (HII-21):

1. WHEN each of the 7 twins runs from the command line with the flags of `crates/piperine-cli/tests/twins.rs` THEN the system SHALL print, on each backend, the bytes it printed before this feature.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran`, `parity_grid`, `parity_corpus` and `piperine-solver`'s `parity_baseline` with their sources unedited and the existing constants of `ToleranceContract` unchanged, except that `parity_corpus.rs` may have its twin count extended by one per new twin (append-only; amended 2026-11-17).
3. WHERE the ngspice comparison is disabled the new `parity_*` targets SHALL still run their twin and print the harness's skip line.

**Independent Test**: `cargo test -p piperine-cli --test twins` and the five `parity_*` targets.

---

### P3: The documents and the root say what is true

**User Story**: As a reader of the Book, `GAPS.md` and `ROADMAP.md`, I want them to describe this feature's surface.

**Why P3**: no behavior depends on it; the Book's listings are held to the binary.

**Acceptance Criteria** (HII-22, the Book):

1. The system SHALL keep every command-line listing of the Book equal to the binary's output.
2. WHEN this feature merges THEN the Book's chapter "The command line" SHALL show the session option keys, one run of each new analysis, and `piperine info` (Q1 (a)), each a listing held to the binary.
3. WHEN this feature merges THEN the Book's `Blinky` listing SHALL show the run that completes (HII-17 AC 4).

**Acceptance Criteria** (HII-23, the records):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list S2, L56, X3, X4, X7, X8 and X9, SHALL list what H1, H2, H3, H4 and H6 still miss, and SHALL keep L39.
2. WHEN this feature merges THEN `ROADMAP.md` SHALL list the follow-up host feature (`force`, waits, continued transients, operating-point variables) and `root-e2e`.

**Acceptance Criteria** (HII-24, the root):

1. The root package `piperine` SHALL re-export the new host surface, and a root integration target SHALL open a session through `piperine::` and run `tf`, a drive and a timed `set`.

**Independent Test**: `cargo test -p piperine-cli --test book --test docs` and `cargo test --test host_ii` at the root.

---

## Edge Cases

- WHEN `nodeset` and `resolution` are given to a `dc` run THEN the system SHALL apply the nodeset at every point's operating point and the resolution to the binding.
- IF `--nodeset` is given without `=` THEN the system SHALL exit 2 naming the flag.
- WHEN a `pss` run's design is purely analog THEN the system SHALL still verify periodicity over the continuous unknowns only.
- IF a `pss` run's digital state closes only after `k` periods THEN the system SHALL fail naming `k` times the period, as the solver does.
- IF a `sens` run's `wrt` names a param from which a child's param derives (HII-18) THEN the system SHALL report `P0010` naming the param and the derived params: the solver's finite difference writes one instance only, and a derivative that skips the children would be wrong with no error.
- IF two host actions name the same port and the same `at` THEN the system SHALL apply them in the order they were given, the last value holding.
- WHEN `piperine test` judges a run with no target THEN the system SHALL print no target line.
- IF a `sim` record with `failed = true` arrives in a run that also has failing checks THEN the system SHALL print the `fatal` form of the `FAIL` line.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| HII-01 | P1: Session options | Tasks | In Tasks |
| HII-02 | P1: Session options | Tasks | In Tasks |
| HII-03 | P1: Session options | Tasks | In Tasks |
| HII-04 | P1: tf, pz, sens | Tasks | In Tasks |
| HII-05 | P1: tf, pz, sens | Tasks | In Tasks |
| HII-06 | P1: tf, pz, sens | Tasks | In Tasks |
| HII-07 | P1: Periodic steady state | Tasks | In Tasks |
| HII-08 | P1: The new analyses on the command line | Tasks | In Tasks |
| HII-09 | P2: Metadata | Tasks | In Tasks |
| HII-10 | P2: Metadata | Tasks | In Tasks |
| HII-11 | P2: Metadata | Tasks | In Tasks |
| HII-12 | P2: Host drives and timed sets | Tasks | In Tasks |
| HII-13 | P2: Host drives and timed sets | Tasks | In Tasks |
| HII-14 | P2: Host drives and timed sets | Tasks | In Tasks |
| HII-15 | P1: What `piperine test` reads | Tasks | In Tasks |
| HII-16 | P1: What `piperine test` reads | Tasks | In Tasks |
| HII-17 | P2: A digital-only transient | Tasks | In Tasks |
| HII-18 | P1: A `set` recomputes | Tasks | In Tasks |
| HII-19 | P1: A `set` recomputes | Tasks | In Tasks |
| HII-20 | P1: A `set` recomputes | Tasks | In Tasks |
| HII-21 | P1: Parity does not move | Tasks | In Tasks |
| HII-22 | P3: The documents and the root | Tasks | In Tasks |
| HII-23 | P3: The documents and the root | Tasks | In Tasks |
| HII-24 | P3: The documents and the root | Tasks | In Tasks |

**Coverage:** 24 total, 24 mapped to tasks, 0 unmapped.

---

## Sources

- The user, 2026-11-16 and 2026-11-17, through the orchestrator: scope and the decisions X3, X4, X7, X8, X9 (`context.md`).
- `GAPS.md` (audit of 2026-11-17): S1, S2, L39, L45, L47, L56, X3, X4, X7, X8, X9, V8, H1 to H11, U5, U10.
- `docs/spec/part_viii_host.md` §23, §23.1 (rules 1 to 12, `E0801` to `E0806`), §23.2 rule 2; `docs/spec/part_vi_io.md` §21; `docs/spec/part_v_execution.md` §16.3, §16.4.
- `.specs/STATE.md` MD-14, MD-18, MD-28, MD-38, MD-63, MD-65, MD-66; `.specs/features/msl-cli/spec.md` CLI-03 to CLI-10; `.specs/features/long-run/spec.md` LR-05.
- `crates/piperine-solver/src/analyses/{tf,pz,sens,pss}.rs`, `src/protocol/host.rs`, `src/core/circuit.rs`; `crates/piperine-ir/src/model.rs` `ChildValue`.
- ngspice-47 probes of 2026-11-17 (`.pz ... vol`, `.tf`, `.sens`, `.pss`).

---

## Success Criteria

- [ ] `piperine run` runs `tf`, `pz`, `sens` and `pss`, and each new `parity_*` target agrees with ngspice under the contract.
- [ ] The probe of S2 prints 0.75.
- [ ] `piperine test` fails a `fatal` run and prints target margins.
- [ ] The 7 twins print the bytes they printed before; the existing parity targets pass unedited.
- [ ] `cargo build` has no warning and `cargo test` passes at the root.

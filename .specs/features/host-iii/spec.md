# Host III Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

**Size:** L, about 32 tasks (this spec). The whole host-iii group (this spec and the four sibling specs below) is several features: about 150 tasks.

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

None of these is decided. Each lists the options and the planner's recommendation; the criteria below are written for the recommended option and name what changes otherwise.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does a Rust host sequence a run: wait for something, act, go on? | (a) **pull**: `Session::run_until(&[Wait]) -> Stop` runs the transient until the first wait fires (or the stop time), suspends it at that instant and returns; the host reads, drives, sets, forces, then calls again; (b) **push**: callbacks registered before the run (`on(wait, \|cx\| ..)`) run inside the instant, as the solver's `HostCx` does today; (c) both, push written on top of pull | **(a)**. One mechanism gives waits, reads at an instant, `fork`/`join` (a list of waits, the first that fires), and the continued transient of §16.4 (a later `until` time). It maps onto Python's `await` (`python-host`) and keeps host code out of the solver's instant. With (b), HIII-05 to HIII-08 are rewritten as callbacks and HIII-09 stays |
| Q2 | What can a host wait on? §23.1 rule 2 says "any event of the model", but §21 gives no path to an event: a reaction or a `cross` has no name | (a) carriers, records and time: a change of a carrier, a rising or falling edge of a `Bit<bool>`, a record on a stream, a time, and a crossing of a probe (`v(<path>)` through a level, with a direction) armed by the host and located like `cross`; §23.1 rule 2 is amended to say so; (b) also the model's own events, named by a new rule of §21 (for example `<instance>.on#<k>` in source order, or the reaction's `///` name); (c) (a) now, and (b) later as a spec change of its own | **(c)**. (a) is what the buck's scenarios and the spec's UART sketch actually wait on (`dv`, `pgood`, `mode`); (b) needs a naming rule the spec does not have and should be written in `spec-closure` first. With (b) now, HIII-06 gains a criterion and §21 gains a rule |
| Q3 | When may a host force a **node** (a potential source to `gnd`, §23.1's second resolution)? | (a) any node, at any time: the bind adds a switchable host source on every node, one more MNA row each; (b) a node is forced only if the host names it before the run (`Session::forceable(path)`, or a force scheduled before the run), which binds its source at open; a force of an unnamed node during the run is refused naming the node and `forceable`; a discrete carrier can be forced at any time | **(b)**. (a) grows every matrix by one row per node to serve a debug action. (b) keeps the bind fixed (MD-18) and costs nothing to a run that forces nothing |
| Q4 | The three open points of §16.4 (G.33, G.34, G.35) | G.33, is `analysis(Static)` true in an analysis run at a transient's last instant: (a) false, (b) true. G.34, which commits made during that analysis survive: (a) none, the transient's state is restored after it, (b) all. G.35, can a transient be continued after a `finish` record: (a) no, refused naming the record, (b) yes | **G.33 (a)**: §16.4 says the analysis "has no operating point of its own". **G.34 (a)**: an analysis at the last instant observes; the continuation is then the same with or without it (§23.1 rule 2, reading never changes the result). **G.35 (a)**: `finish` ends the run (X4), a continuation would resurrect it. Each answer becomes a sentence of `docs/spec/part_v_execution.md` §16.4 in this feature |

## Problem Statement

`host-ii` gives a Rust host drives of the top's discrete `in` ports and timed `set`s, both scheduled before the run (`Session::drive`, `Session::set_at`). Part VIII §23.1 also gives the host `force` and `release` of any carrier, waits on the running design, reads at an instant, and (§16.4) a transient continued to a later stop or followed by an analysis at its last instant. None of that exists: the digital engine has no override of a carrier's drives (`piperine-digital-solver/src/engine.rs:199` queues a host commit only), the events of a model are private pins a host cannot name (`event:<k>`, `piperine-plan/src/digital.rs:339`), a session runs once (`msl-cli` CLI-04), and re-entry is approximate (`GAPS.md` V8, owned by `solver-hygiene`). The buck's scenarios S1 to S9 (`examples/buck/README.md`) need exactly these waits and forces.

This spec also opens the host-iii group: it holds the "Gaps closed" table for every gap assigned to host-iii and points at the sibling specs that close the rest.

## The host-iii group

| Spec | Closes | Size |
| ---- | ------ | ---- |
| `host-iii` (this) | H1 rest (`force`, `release`, waits, reads at an instant), H6 | L, about 32 tasks |
| `host-analyses` | H2 rest (S-parameters, Fourier, distortion), operating-point variables | M, about 24 tasks |
| `sweeps-montecarlo-optimization` | H2 rest (more than one sweep axis), H7 | L, about 34 tasks |
| `gradients` | H8 | L, about 40 tasks |
| `cli-packages` | H10 | M, about 26 tasks |

## Gaps closed

Every id the coverage map gives host-iii, with the requirement that closes it. No id is left open.

| Gap (`GAPS.md`) | What of it | Closed by |
| --------------- | ---------- | --------- |
| H1 | `force` and `release` of a discrete carrier | HIII-01, HIII-03, HIII-04 |
| H1 | `force` and `release` of a node | HIII-02, HIII-04 |
| H1 | a host wait on an event of the running design; `fork`/`join` | HIII-05, HIII-06, HIII-07 |
| H1 | read at an instant | HIII-08 |
| H1 | `drive` of a top port, `set` at a time (scheduled before the run) | `host-ii` HII-12, HII-13; during a suspended run: HIII-09 |
| H1 | "The buck's scenarios S1 to S9 have no host" | the host is HIII-05 to HIII-09; the scenarios themselves are `buck-scenarios` (U3) |
| H2 | S-parameters | `host-analyses` HAN-01 to HAN-04 |
| H2 | Fourier and distortion (moved out of `host-ii`, unassigned elsewhere) | `host-analyses` HAN-05 to HAN-08 |
| H2 | more than one sweep axis (moved out of `host-ii`, unassigned elsewhere) | `sweeps-montecarlo-optimization` SMO-01 to SMO-03 |
| H2 | `Noise` | not here: `analog-operators` (blocked by L45). HIII-12 runs it at a transient's last instant once it exists |
| H6 | continued transients | HIII-10, HIII-11 |
| H6 | an analysis at a transient's last instant | HIII-12, HIII-13 |
| H7 | Monte Carlo, optimization, objectives and constraints from `target`s, bounds from `where` | `sweeps-montecarlo-optimization` SMO-04 to SMO-14 |
| H8 | gradient params, `∂(record)/∂p`, saltation, `E0807` to `E0809` | `gradients` GRD-01 to GRD-14 |
| H10 | `add`, `remove`, `tree`, `update`, `--locked`, `--offline`, plugin scripts, a REPL, ngspice raw output, a terminal or PNG plot | `cli-packages` CLP-01 to CLP-12 |
| H4 (part) | operating-point variables (`--print Top.g.id` is `E0801`) | `host-analyses` HAN-09 to HAN-11 |

## Goals

- [ ] A Rust host forces and releases a discrete carrier and a node during a transient, each change on `log`, as §23.1's resolution says.
- [ ] A Rust host runs a transient until a carrier changes, an edge, a record, a time or a crossing, reads anything at that instant, acts, and goes on; the first of several waits wins.
- [ ] A transient continued to a later stop gives the same rows as one run to that stop, apart from the old stop being a time point.
- [ ] `op`, `dc` and `ac` run at a transient's last instant, from its final state, and the transient continues after them unchanged.
- [ ] The buck's S1 (start-up timing) runs as a Rust host test through these calls.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Drives and timed `set`s scheduled before the run, `E0803` to `E0806` | `host-ii` (HII-12 to HII-14). This spec reuses them during a suspended run |
| Noise analysis, also at a transient's last instant | `analog-operators` (L45, H2 `Noise`). HIII-12 AC 4 runs it there once it exists |
| S-parameters, Fourier, distortion, operating-point variables | `host-analyses` |
| Sweeps over more than one axis, Monte Carlo, optimization | `sweeps-montecarlo-optimization` |
| Gradients | `gradients` |
| The buck's scenarios S1 to S9 as tests of record | `buck-scenarios` (U3). This spec proves S1 only, as the end-to-end check of its calls |
| Python's `async`/`await` surface | `python-host`, over the calls of this spec |
| Waiting on the model's own reactions by name | Q2 (c): a spec change of `spec-closure` first |
| A top with a continuous port | L39, `params-and-io` (X8 left it) |
| A `force` or a wait from the command line or the manifest | X8: the host acts on a running design through `piperine-api` only; benches stay the command-line path |
| Exact re-entry of the solver | `solver-hygiene` HYG-04, a dependency |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Sequencing model | pull: `run_until` suspends and returns (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| What a wait names | carrier change, edge, record, time, host-armed crossing (Q2 (c)) | Q2 recommendation | n - open (Q2) |
| Node force | named before the run (Q3 (b)) | Q3 recommendation | n - open (Q3) |
| §16.4 open points | G.33 false, G.34 none survive, G.35 refused (Q4) | Q4 recommendation | n - open (Q4) |
| A suspended run's host actions | a drive, a `set`, a `force` or a `release` made while suspended at `(t, n)` is due at `(t, n + 1)` | §23.1's first resolution: an action in response to an observed event, with no wait, is due at the next microstep | n - planner |
| Refusals with no spec code | a host failure naming the item, with an implementation code `P00xx` assigned in design and listed in the Book's error table | §23.1 gives `E0801` to `E0806` only; a refusal is loud with or without a spec code | n - planner |
| Where a wait is located | a carrier change, edge or record at the exact instant `(t, n)` it happens; a host crossing by the same secant location as `cross` (`piperine-analog-solver/src/crossing.rs`), with the run's `ttol` | the engine's host waiter already stops an instant (`protocol/host.rs` `on_event`); a crossing must be a time point (§17.2.6) | n - planner |
| How a continued transient keeps its state | the session keeps the live solver suspended; when a session was closed, it re-enters from the last step's re-entry state (`solver-hygiene` HYG-13 `Reentry::LastStep`) | a live solver is exact by construction; re-entry is exact once HYG-04 lands | n - planner |
| The forced value of a node | a voltage, `f64`, through an ideal source to `gnd` | §23.1's second resolution | y - spec |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: HIII-04, HIII-06 AC 4, HIII-10 AC 3. Failure and partial-failure states: HIII-07 AC 3 (a wait never fires), HIII-02 AC 4 (a singular force). Idempotency and retry: HIII-01 AC 4 (a second force of the same carrier replaces the first). Auth boundaries and rate limits: N/A because the host is a local library. Concurrency and ordering: HIII-07 (several waits, the first wins; ties by the order given), HIII-09 AC 1. Data lifecycle: HIII-11 AC 2 (the rows of a continuation append). Observability: HIII-03 (force on `log`). External-dependency failure: N/A because no external service is called. State-transition integrity: HIII-10 AC 3 and AC 4 (no continuation after a failure or a `finish`), HIII-13 (the state is restored after an analysis at the last instant).

---

## User Stories

### P1: Force and release a discrete carrier ⭐ MVP

**User Story**: As a verification engineer, I want to force a carrier to a value and later release it, so that I can inject a fault or hold a control line without editing the model.

**Why P1**: §23.1 rule 3; a debug action every HDL host has, and the buck's fault scenarios need it.

**Acceptance Criteria**:

1. WHEN a host forces a discrete carrier to a value at time `t` THEN the system SHALL give the carrier the forced value from microstep 0 of `t`, whatever its drives write, until it is released (HIII-01)
2. WHEN a forced carrier's value changes at the force or at the release THEN the system SHALL fire `changed()` of that carrier at that instant (HIII-01)
3. WHEN a host releases a forced carrier THEN the system SHALL give it the value its drives resolve to at that instant, the drives having gone on updating while it was forced (HIII-01)
4. WHEN a host forces a carrier that is already forced THEN the system SHALL replace the forced value with the new one (HIII-01)
5. WHEN a host forces or releases a carrier THEN the system SHALL write one record on the `log` stream naming the action, the path, the value and the time (HIII-03)
6. IF a host releases a carrier that is not forced THEN the system SHALL refuse with a host failure naming the carrier, and change nothing (HIII-04)
7. IF a host forces a path that is not in the metadata THEN the system SHALL refuse with `E0801` naming the path (HIII-04)
8. IF a host forces a carrier with a value whose type is not the carrier's THEN the system SHALL refuse with `E0802` naming the carrier and the value (HIII-04)
9. IF a host schedules a force or a release at a time before the current time of the run THEN the system SHALL refuse with `E0806` naming the action (HIII-04)

**Independent Test**: a counter whose enable is forced low from 1 µs to 2 µs holds its count over that interval and counts again after the release, and `log` has two records.

---

### P1: Force and release a node ⭐ MVP

**User Story**: As a designer, I want to pin a node to a voltage for a while, so that I can break a loop or hold a reference during a test.

**Why P1**: §23.1's second resolution gives the node form.

**Acceptance Criteria**:

1. WHEN a host forces a node named before the run (Q3 (b)) to a voltage at time `t` THEN the system SHALL hold `v` of the node at that voltage from `t` until the release, through a potential source to `gnd` (HIII-02)
2. WHEN a node force is applied or released THEN the system SHALL make that time a time point of the transient and treat it as a jump (§15.2) (HIII-02)
3. WHEN a node force is released THEN the system SHALL remove the source's branch from the equations from that time on (HIII-02)
4. IF a node force closes a loop of potential contributions THEN the system SHALL fail the run with `E0503` naming the node (HIII-02)
5. IF a host forces, during a run, a node not named before the run THEN the system SHALL refuse with a host failure naming the node and `forceable` (HIII-04)
6. The system SHALL give a design in which no node is named forceable the same matrix and the same rows as before this feature (HIII-02)

**Independent Test**: an RC whose capacitor node is forced to 0 V from 1 ms to 2 ms reads 0 V over that interval and recharges after with the RC time constant.

---

### P1: Wait on the running design ⭐ MVP

**User Story**: As a verification engineer, I want to run a transient until something happens, look, act and continue, so that I can write the interactive sequences of §23.1 in a host test.

**Why P1**: §23.1 rule 2 and the host column of its table; the buck's scenarios wait on `mode` and `pgood`.

**Acceptance Criteria**:

1. WHEN a host calls `run_until` with waits THEN the system SHALL run the transient until the first instant at which one of the waits fires, suspend it there and return which wait fired and the instant `(t, n)` (HIII-05)
2. WHEN a host calls `run_until` again on a suspended session THEN the system SHALL continue the same transient from the suspended instant, with no step repeated and none skipped (HIII-05)
3. WHEN a run is suspended and resumed any number of times with no host action THEN the system SHALL give the same rows and records as the same run with no wait (HIII-05)
4. WHEN a host waits on a carrier's change THEN the system SHALL fire the wait at the instant the carrier's value changes (HIII-06)
5. WHEN a host waits on a rising or a falling edge of a `Bit<bool>` carrier THEN the system SHALL fire the wait at the instant the value goes from false to true, or from true to false (HIII-06)
6. WHEN a host waits on a stream THEN the system SHALL fire the wait at the instant a record of that stream is emitted, and give the record (HIII-06)
7. WHEN a host waits on a time `t` THEN the system SHALL fire the wait at microstep 0 of `t`, making it a time point (HIII-06)
8. WHEN a host waits on a crossing of `v(<path>)` through a level with a direction THEN the system SHALL locate the crossing time within the run's `ttol` as `cross` does, make it a time point, and fire the wait there (HIII-06)
9. IF a wait names a path that is not in the metadata THEN the system SHALL refuse with `E0801` naming the path (HIII-06)
10. WHEN several waits fire at the same instant THEN the system SHALL report the first in the order the host gave them (HIII-07)
11. WHEN a host gives `run_until` a stop time and no wait fires before it THEN the system SHALL suspend the transient at that time and report that the stop was reached (HIII-07)
12. IF the transient fails while a host waits THEN the system SHALL return the solver's failure, and refuse any later `run_until` on that session naming the failure (HIII-07)

**Independent Test**: the buck host test waits on `mode == Regulate`, then on a rising edge of `pgood`, and checks the two times against `examples/buck/SPEC.md` S1.

---

### P1: Read and act at an instant ⭐ MVP

**User Story**: As a host author, I want to read any carrier and probe any node while the run is suspended, and drive, set or force there, so that the host reacts to what it saw.

**Why P1**: §23.1 rules 2 and 4 and the first resolution.

**Acceptance Criteria**:

1. WHEN a host reads a carrier or probes a node of a suspended session THEN the system SHALL give its value at the suspended instant `(t, n)` (HIII-08)
2. The system SHALL leave the rows, records and later values of a run unchanged by any number of reads (HIII-08)
3. WHEN a host drives a top `in` port, sets a param, or forces or releases a carrier while suspended at `(t, n)` THEN the system SHALL apply it at `(t, n + 1)` (HIII-09)
4. WHEN a host schedules such an action at a later time while suspended THEN the system SHALL apply it as `host-ii` HII-12 and HII-13 say for that time (HIII-09)
5. IF a host sets a structural param while suspended THEN the system SHALL refuse with `E0803` naming the param (HIII-09)

**Independent Test**: suspended at a rising edge of `dv`, the host reads `byte` and finds the value the bench sent.

---

### P1: Continued transients ⭐ MVP

**User Story**: As a designer, I want to run a transient to one stop, look at it, and continue it to a later stop, so that a long run can be inspected in pieces.

**Why P1**: H6; §16.4 rule 1.

**Acceptance Criteria**:

1. WHEN a host continues a finished transient to a later stop THEN the system SHALL go on from the last instant with its time, event queue, discrete state, instance state and analog history, commit no initial value again, and fire neither `changed()` nor `init()` for the continuation itself (HIII-10)
2. WHEN a transient is continued from stop `t1` to `t2` THEN every row and record after `t1` SHALL equal the same run's from `t1` to `t2` run in one go with `t1` made a time point, under `ToleranceContract::TRAN_VOLTAGE` for analog values and exactly for discrete values and records (HIII-11)
3. IF a host continues a transient with a stop not after the last instant THEN the system SHALL refuse with `E0806` naming the stop (HIII-10)
4. IF a host continues a transient that failed, or that delivered a `finish` record (Q4, G.35 (a)) THEN the system SHALL refuse with a host failure naming the failure or the record (HIII-10)
5. WHEN a host set a param or drove a port between the two runs THEN the system SHALL apply it as a commit collected at the old stop's instant, not as an initial value (§23.1 rule 11) (HIII-10)
6. WHEN a continued transient runs THEN `analysis(Static)` SHALL be false in it (HIII-10)
7. WHEN a run with `print_step` is continued THEN the system SHALL append the rows of the continuation to the same table, on the same grid, with no row repeated (HIII-11)

**Independent Test**: `ClockGen` run to 1 µs then continued to 3 µs prints the same edges as one run to 3 µs.

---

### P2: An analysis at a transient's last instant

**User Story**: As a designer, I want an operating point, a DC sweep or an AC sweep taken from the state a transient reached, so that I linearize around where the circuit actually is.

**Why P2**: H6; §16.4 rule 2.

**Acceptance Criteria**:

1. WHEN a host runs `op` at a transient's last instant THEN the system SHALL solve one more instant at that time, `now()` staying, starting Newton from the transient's final solution, with nothing in the queue coming due (HIII-12)
2. WHEN a host runs `dc` or `ac` at a transient's last instant THEN the system SHALL linearize around, or sweep from, that instant's solution, and give the table as `host-ii` gives it (HIII-12)
3. WHEN an analysis runs at a transient's last instant THEN `analysis(Static)` SHALL be false in it (Q4, G.33 (a)) (HIII-12)
4. WHERE `analog-operators` has given the `Noise` analysis the system SHALL also run it at a transient's last instant by the same rule (HIII-12)
5. WHEN an analysis at a transient's last instant ends THEN the system SHALL restore the transient's state, so that a continuation after it gives the same rows as a continuation without it (Q4, G.34 (a)) (HIII-13)
6. WHEN `tran_rc` is run to 5 time constants and then `ac` runs at its last instant THEN the AC table SHALL agree with ngspice's `.ac` at the operating point of the same circuit under `ToleranceContract`'s AC constant (HIII-12)

**Independent Test**: a diode circuit driven by a ramp, `ac` at the last instant, equals `ac` around an operating point at the ramp's final value.

---

### P2: The buck's first scenario through the host

**User Story**: As the owner of the buck prototype, I want S1 written as a Rust host test, so that the calls of this spec are proven on the flagship design.

**Why P2**: H1's "the buck's scenarios have no host"; the rest is `buck-scenarios`.

**Acceptance Criteria**:

1. WHEN a Rust host test drives `en` high at 100 µs, waits on `mode == Regulate` and on a rising edge of `pgood` THEN the system SHALL report the two times within the limits of `examples/buck/SPEC.md` §10 S1 (HIII-14)
2. The system SHALL hold the host test in `crates/piperine-api/tests/` and run it in the default `cargo test` (HIII-14)

**Independent Test**: `cargo test -p piperine-api --test buck_s1`.

---

### P3: The documents say what is true

**User Story**: As a reader of the spec, the Book, `GAPS.md` and `ROADMAP.md`, I want them to describe these host calls.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN `docs/spec/part_v_execution.md` §16.4 SHALL state the answers to G.33, G.34 and G.35, and `docs/spec/part_viii_host.md` §23.1 rule 2 SHALL state what a host waits on (Q2) (HIII-15)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list H6 nor the `force`, wait and read parts of H1 (HIII-15)
3. WHEN this feature merges THEN the Book SHALL show one Rust host test that waits, reads and forces, with its output held to a test (HIII-15)

**Independent Test**: `cargo test -p piperine-cli --test docs`.

---

## Edge Cases

- WHEN a wait fires at the instant a run is first started (time 0) THEN the system SHALL suspend at `(0, n)` after the initial values are committed.
- IF a host calls `run_until` with no wait and no stop time THEN the system SHALL refuse with a host failure naming the missing stop.
- WHEN a forced carrier is a top `in` port that the host also drives THEN the system SHALL show the forced value until the release and the driven value after it.
- IF a force targets an `out` port of the top THEN the system SHALL force the carrier, as any other carrier.
- WHEN the stop time of `run_until` equals a wait's time THEN the system SHALL report the wait.
- IF a host reads a session that was never run THEN the system SHALL give each carrier's initial value and each node's operating-point value, or a host failure naming the missing operating point when none was taken.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| HIII-01 | P1: Force a discrete carrier | - | Pending |
| HIII-02 | P1: Force a node | - | Pending |
| HIII-03 | P1: Force a discrete carrier | - | Pending |
| HIII-04 | P1: Force (refusals) | - | Pending |
| HIII-05 | P1: Wait on the running design | - | Pending |
| HIII-06 | P1: Wait on the running design | - | Pending |
| HIII-07 | P1: Wait on the running design | - | Pending |
| HIII-08 | P1: Read and act at an instant | - | Pending |
| HIII-09 | P1: Read and act at an instant | - | Pending |
| HIII-10 | P1: Continued transients | - | Pending |
| HIII-11 | P1: Continued transients | - | Pending |
| HIII-12 | P2: An analysis at a transient's last instant | - | Pending |
| HIII-13 | P2: An analysis at a transient's last instant | - | Pending |
| HIII-14 | P2: The buck's first scenario | - | Pending |
| HIII-15 | P3: The documents | - | Pending |

**Coverage:** 15 total, 0 mapped to tasks (spec only).

---

## Dependencies

| Depends on | For |
| ---------- | --- |
| `host-ii` (in flight) | `Session::drive`, `Session::set_at`, the host-action list, `E0802` to `E0806`, the session options (`ttol` default), metadata (`E0801`) |
| `solver-hygiene` HYG-04, HYG-13 (in flight) | exact re-entry, so a continuation from a closed session equals the continuous run (HIII-11) |
| `msl-fixes` S1 (in flight) | `finish` ends the analysis, which HIII-10 AC 4 reads |
| `analog-operators` (spec) | `Noise`, for HIII-12 AC 4 only |

Solver touches named for design: an override layer on the digital engine's resolved values (`piperine-digital-solver`), a switchable host source on a named node (`piperine-solver` bind), a suspend point of the transient driver at a host waiter (`protocol/host.rs` already stops an instant), and host-armed crossing watches (`piperine-analog-solver/src/crossing.rs`).

## Sources

- The user, 2026-11-17 (through the orchestrator): "gostaria que abrisse feature e escrevesse a spec para todos esses gaps [...] Preciso que TODOS os gaps estejam endereçados".
- `GAPS.md` (audit of 2026-11-17): H1, H2, H4, H6, H7, H8, H10, V8, L39, U3; the coverage map of 2026-11-17.
- `.specs/features/host-ii/spec.md` and `context.md` (branch `feature/host-ii`): what host-ii covers and its Deferred Ideas; X3, X4, X7, X8, X9.
- `docs/spec/part_viii_host.md` §23, §23.1 (rules 1 to 12 and the three resolutions); `docs/spec/part_v_execution.md` §15, §16.4; `docs/spec/part_vi_io.md` §19 rule 2, §21.
- `.specs/STATE.md` MD-18, MD-38, MD-65, MD-66.
- `crates/piperine-digital-solver/src/engine.rs:199` (host commit), `crates/piperine-plan/src/digital.rs:339` (private event pins), `crates/piperine-solver/src/protocol/host.rs` (host waiters).
- `examples/buck/README.md` (the Python sketch of S1, S3, S5).

---

## Success Criteria

- [ ] The buck's S1 runs as a Rust host test with the two times inside SPEC §10's limits.
- [ ] A continued transient equals the one-go run under the contract.
- [ ] No existing row moves: the 7 twins print the bytes they printed.

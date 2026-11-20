# Buck Scenarios Specification

**Approved**: pending - the user is away; the planner took every default below (see "Decisions taken (user AFK)").

ROADMAP execution item 6, lane B (`buck-scenarios`). Closes `GAPS.md` U3 and the buck's part of H1.

## Problem Statement

The buck converter of `examples/buck` is proven closed-loop at one operating point only: setpoint 0.8 V,
Vin 12 V, 6.6 Ω, 600 µs (`GAPS.md` U3). Its SPEC §10 lists nine bench scenarios with pass criteria; none
runs, because they need host stimulus during a run. `host-ii` delivered that stimulus for a Rust host
(`Session::drive`, `Session::set_at`), and `long-run` made a multi-millisecond transient affordable in memory
(`Recorder`, `Keep::Last`, the print grid). Nothing yet uses them to judge the converter against its SPEC.

## Goals

- [ ] SPEC §10 S2 to S9 each run as a scenario run and are judged by Rust host tests against SPEC's own pass criteria, at Vin 12 V and setpoint 3.3 V unless the scenario says otherwise.
- [ ] Every scenario run passes S10: its `checks` stream holds records and none failed.
- [ ] The whole suite simulates 16.6 ms of converter time in five runs that execute in parallel, so its wall time is the longest run's (about 4.7 ms simulated), not the sum.
- [ ] Every buck behaviour that diverges from its SPEC is fixed in the model, not documented away.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| S1 (start-up) and S1b (UVLO start-up) | Not in the U3 scope the user set (S2 to S9). S1 at 0.8 V is held by `buck_long` and `piperine-cli/tests/buck.rs`; each 3.3 V run here checks its own start only as a precondition (BS-03 AC 1) |
| A Python host | User, 2026-11-18: Rust first, Python later (`python-host`, B4) |
| Host sequences from the command line | Decision X8: the command line takes no host stimulus; a scenario run from `piperine run` drives nothing |
| `force`, `release`, a wait for an event, a read at an instant (rest of H1) | `host-iii`; the scenarios schedule every stimulus at a time fixed by SPEC's own timing |
| Faster runs (multi-rate stepping, averaged models, resuming a run) | `multi-rate` (V12), `surrogate-models` (V13), H6; this feature budgets around today's speed |
| Divider and ADC-reference tolerances of §4.3 | The model has ideal resistors and reference; S9 judges the model against the §4.3 totals, which include them |
| Startup into a pre-biased output | SPEC §7.2 declares it unsupported; S7's restart is judged only up to the entry into `SoftStart` |
| Changing a tolerance | MD-38 and the user's rule: a tolerance is SPEC's, never retuned to pass |

---

## Decisions taken (user AFK)

The user is away and said: "Tu sempre assume o default mais razoável e depois me conta". Every row below is a
default the planner took; each is reported to the user and each can be overturned.

| # | Question | Default taken | Why |
| - | -------- | ------------- | --- |
| D1 | Where the scenario runs live | A scenario package `tests/e2e/buck/` (`Piperine.toml`, `benches/scenarios.msl`) with a path dependency on `examples/buck` | MD-72 makes `tests/e2e/` the one home of scenarios. Adding runs to `examples/buck/Piperine.toml` would change CLI-08's two-run list and the Book's "2 runs, 2 passed", and make `piperine test` there cost about an hour |
| D2 | Where the stimulus lives | In the Rust host test, through `Session::drive` and `Session::set_at`; the bench holds none | S4, S7 and S9 change `vin_nom` and `r_load` during a run, which no MSL bench can do (X8). One mechanism for every scenario keeps each timeline beside the windows it measures |
| D3 | What the bench and the run tables describe | The bench places `Buck` under a top `Bench` whose `in` ports are `Buck`'s; each run table names what is sampled (`print`, on a 10 ns `print_step`) and emitted (`streams = ["checks"]`); the host's `Recorder` samples | The user's rule: "the bench describes what is sampled/emitted, sampling is host-side" |
| D4 | How the runs are shared | Five runs, one per family: `setpoint` (S2, S2b), `load_line` (S3, S4, S8), `faults` (S5, S6), `brownout` (S7), `steady_state` (S9). One test target holds all five; each family runs once, and the five run in parallel threads | A 3.3 V start costs 1.25 ms of simulated time; sharing it cuts the CPU time, and parallel families cut the wall time to the longest run |
| D5 | A "ramp" of Vin | A staircase of `set_at` on `buck.vin_nom` in 0.1 V steps: S4 and S9 line steps take 10 µs (0.25 µs per step for 4 V); S7's brown-out falls and rises at 50 mV/µs (0.1 V every 2 µs) and holds 7.5 V for 60 µs | The supply is an ideal source behind 20 mΩ (SPEC §2) with no slew; SPEC gives S4's 10 µs and no S7 rate |
| D6 | S4's 2 A load | `buck.r_load` set to 1.65 Ω (3.3 V / 2 A) 200 µs before the line step, and back to 6.6 Ω 150 µs before S8 | SPEC §10 S4 says "2 A load"; `r_load` is a non-structural param |
| D7 | S5's "retry every 2 ms while the overload stays" | One retry is observed with the overload still on; the overload is then removed during that retry's soft-start, which must reach `Regulate` and clear `fault` | A second retry costs 2.7 ms more of simulated time (about 10 min); the 2 ms period is measured exactly on the one hiccup |
| D8 | A value SPEC writes with "≈" | Reported by the test, not judged: S2b's ≈ −0.6 A, S6's ≈ 7 mV/µs and ≈ 80 to 120 µs. S6 is judged on its mechanism instead: `OverVoltage` within two samples of `vout` passing 3.81 V | SPEC §10: "the tolerances are the bench's pass criteria"; an "≈" is an estimate. S6's estimates assume the sink current holds −3 A on average, while §8.3's cut ends the low-side pulse and lets the current relax each period, so the model may rise faster; reported to the user |
| D9 | S2b's settling has no tolerance in SPEC ("at 1.29 ms") | ± 0.05 ms, as S1 and S2 | SPEC gives ± 0.05 ms for the two other ramps; the SPEC row gains it (BS-09) |
| D10 | S9's DC-error bound at 3.3 V (§4.3 has columns for 0.8 V and 5.0 V only) | ± 14.7 mV, from §4.3's own rows at 3.3 V: quantization 3.1, aliasing 1.7, divider 6.6, reference 3.3 mV; SPEC §4.3 gains the column (BS-09) | Each row of §4.3 is computed for 3.3 V by the rule §4.3 states; nothing is interpolated |
| D11 | Bounds SPEC leaves to the bench: the current limits (S5, S6), S8's decay, the sample-time resolution | Written into a new SPEC §10.1 "How the bench measures" with each derivation (BS-09) | A test cites SPEC; a bound that lives only in a test is a tolerance nobody approved |
| D12 | SPEC names the cleared fault code `None`, the model `Clear` | SPEC is corrected to `Clear` | A name, not a behaviour; `None` is `Option`'s variant in the reactions that latch the fault. Reported to the user as a spec edit |
| D13 | The hiccup lasts 1001 samples in the model (2.002 ms); SPEC §8.5 says 1000 (2 ms) | Fixed in the model (`supervisor.msl`) and in the reference model of `piperine-parity/tests/buck_supervisor.rs`, which mirrors the same off-by-one | Code follows spec |
| D14 | The step budget | `max_steps = 15000000` in every run | The 600 µs start takes about 675 000 accepted steps; the longest run (4.7 ms) needs about 5.3 M, above the default budget of 2 M attempts |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1 to D14 above | as written there | user AFK, defaults delegated ("Tu sempre assume o default mais razoável") | n - default (user AFK) |
| The start of a 3.3 V family | `en` rises at 1 µs with setpoint 100 (the reset value); the first stimulus is at 1.25 ms, after `Regulate` (≈ 1.06 ms) and `pgood` (≈ 1.19 ms, S1) | SPEC §7.2 and §8.6 timing | n - default (user AFK) |
| A setpoint write | `sp_data` is driven 1 µs before `sp_wr` rises; `sp_wr` falls 9 µs later; the scenario's edge is the rise of `sp_wr` | SPEC §7.1: the register takes `sp_data` on a rising `sp_wr` | n - default (user AFK) |
| "Both switches off" | both gate commands of the controller, `buck.hs_cmd` and `buck.ls_cmd`, low | the commands are the controller's decision; the 10 ns driver delay is §5.3's and is held by `buck_power` | n - default (user AFK) |
| Inductor current | `buck.isense` divided by 0.1 V/A | SPEC §2: the sense chain is 0.1 V/A, bidirectional, in series with L | n - default (user AFK) |
| The deviation instant of S5 | the first row where the ADC code is outside `reference ± reference/16`, the supervisor's own window (§8.6) | the supervisor sees the deviation through the ADC, not the node | n - default (user AFK) |
| S9's settling before each window | 200 µs after the last ramp or line step, then a 64 µs window (32 periods, 4 dither frames) | SPEC §10 S3/S4 recovery is ≤ 200 µs; the window spans whole dither frames | n - default (user AFK) |
| S7 after the restart | the restart meets a pre-biased output (≈ 3.1 V) and, with the OVP threshold on the ramped reference (§8.4), trips `OverVoltage` two samples into `SoftStart`; S7 is judged up to the entry into `SoftStart` | SPEC §7.2: pre-bias is not supported; reported to the user | n - default (user AFK) |

**Open questions:** none - all resolved or logged above (each a default the user delegated while away).

**Implicit-requirement dimensions.** Input validation and bounds: N/A because the feature adds tests over an existing host API that validates its own input (host-ii). Failure and partial-failure states: BS-08 AC 2 (a family that fails is reported once, never re-run by the next test). Idempotency and retry: BS-08 AC 2 (each family runs once per target). Auth and rate limits: N/A, a local test. Concurrency and ordering: BS-08 AC 1 (families in parallel threads, each with its own build directory). Data lifecycle: BS-01 AC 6 (the runs leave no file in the tree). Observability: BS-08 AC 3 (each family prints its wall time and every measured value). External-dependency failure: N/A because the transpiled backend's `rustc` is the existing test prerequisite of `buck_long`. State-transition integrity: the supervisor's transitions are the subject of BS-05 and BS-06.

---

## Scenario budget

Simulated time per run, the accepted steps that implies (≈ 1.125 M per ms, from the 600 µs start's 675 000), and the
wall time at today's dev-build speed (130 s to 180 s per 600 µs, `long-run` and the CLI). All five runs execute in
parallel threads of one target.

| Run | Scenarios | Simulated | Steps (est.) | Wall (est.) |
| --- | --------- | --------- | ------------ | ----------- |
| `setpoint` | S2, S2b | 3.40 ms | 3.8 M | 12 to 17 min |
| `load_line` | S3, S4, S8 | 2.92 ms | 3.3 M | 11 to 15 min |
| `faults` | S5, S6 | 4.70 ms | 5.3 M | 17 to 24 min |
| `brownout` | S7 | 1.52 ms | 1.7 M | 6 to 8 min |
| `steady_state` | S9 | 4.07 ms | 4.6 M | 15 to 20 min |
| **Target** | S2 to S10 | 16.61 ms (CPU) | 18.7 M | 17 to 24 min wall |

---

## User Stories

### P1: The scenario runs ⭐ MVP

**User Story**: As the buck's designer, I want SPEC §10's scenarios as named runs over the unmodified converter so that a host can drive each one and judge what it prints.

**Why P1**: Every other story runs through them.

**Acceptance Criteria**:

1. The system SHALL hold the package `tests/e2e/buck/` whose `Piperine.toml` depends on `examples/buck` by path and declares exactly the runs `brownout`, `faults`, `load_line`, `setpoint` and `steady_state`.
2. The system SHALL give each scenario run the bench `benches/scenarios.msl`, the top `Bench`, a transient with `max_step = 1e-8`, `print_step = 1e-8`, `streams = ["checks"]` and `max_steps = 15000000`, and the print list `buck.vout`, `buck.vin`, `buck.isense`, `buck.ctl.reference`, `buck.code`, `mode`, `fault`, `pgood`, `buck.hs_cmd`, `buck.ls_cmd`, `buck.oc`, `buck.sink_oc`, `buck.ctl.cut`.
3. WHEN a host opens any scenario run THEN the system SHALL accept a drive at time 0 of each of `en`, `sp_wr`, `sp_data`, `load_step` and `inject`, and a timed set of `buck.vin_nom`, `buck.r_load` and `buck.r_step`.
4. WHEN the scenario package is checked THEN the system SHALL elaborate every run with no diagnostic.
5. The system SHALL keep `examples/buck/Piperine.toml` with exactly its two runs `idle` and `startup`.
6. WHEN a scenario run has been opened THEN `tests/e2e/buck/` SHALL hold only `Piperine.toml` and `benches/scenarios.msl` (no lock file, no build output).

**Independent Test**: open `tests/e2e/buck` with `Project::at`, check it, open one run and drive its ports.

---

### P1: Measurements defined once ⭐ MVP

**User Story**: As the reviewer of a scenario, I want each measurement (settling, deviation, ripple, a mode's first time) defined once and proven on known data so that a pass means SPEC's criterion and not a quirk of a helper.

**Why P1**: Every scenario check is one of these measurements.

**Acceptance Criteria**:

1. WHEN a settling time into a band is asked of a window THEN the system SHALL give the time of the first row from which every later row of the window is inside the band, and none when the last row is outside.
2. WHEN a deviation from a nominal value is asked of a window THEN the system SHALL give the largest absolute difference between a row and the nominal, with the time of that row.
3. WHEN a peak-to-peak value is asked of a window THEN the system SHALL give its largest row value minus its smallest.
4. WHEN a mean is asked of a window THEN the system SHALL give the arithmetic mean of its rows.
5. WHEN the first time of a level crossing is asked THEN the system SHALL give the time of the first row at or past the level in the asked direction, after a given time.
6. WHEN the first time a discrete column takes a value is asked THEN the system SHALL give the time of the first row holding it, after a given time.
7. WHEN a discrete column is asked to hold a value over a window THEN the system SHALL name the first row that does not, with its time and value, and pass when none.
8. WHEN a staircase ramp of a param from `a` to `b` over a duration in steps of a size is asked THEN the system SHALL schedule one set per step, equally spaced, the last at the end of the duration and equal to `b`.
9. IF a table has no column of the asked name THEN the system SHALL panic naming the name and the table's columns.

**Independent Test**: unit tests over hand-built tables in `piperine-parity`.

---

### P1: Setpoint changes, S2 and S2b ⭐ MVP

**User Story**: As the buck's designer, I want the setpoint register's writes judged against SPEC §10 S2 and S2b so that the slewed reference and the loop's tracking are proven up and down.

**Why P1**: U3 names S2.

**Acceptance Criteria** (run `setpoint`; S2's write at 1.25 ms, S2b's at 2.00 ms):

1. WHEN the first stimulus time of a 3.3 V family (1.25 ms) is reached THEN the system SHALL show `mode` = `Regulate`, `pgood` high and `vout` within 3.3 V ± 1 %.
2. WHEN 168 is written at 1.25 ms THEN the system SHALL show `buck.ctl.reference` first at 800 codes 0.544 ms ± 2 µs after the write.
3. WHEN 168 is written THEN the system SHALL settle `vout` into 5.0 V ± 1 % at 0.54 ms ± 0.05 ms after the write.
4. WHILE the S2 window (1.25 ms to 2.00 ms) runs the system SHALL keep `vout` at most 15 mV above 5.0 V.
5. WHILE the S2 window runs the system SHALL keep `pgood` high in every row.
6. WHEN 8 is written at 2.00 ms THEN the system SHALL show `buck.ctl.reference` first at 160 codes 1.28 ms ± 2 µs after the write.
7. WHEN 8 is written THEN the system SHALL settle `vout` into 1.0 V ± 1 % at 1.29 ms ± 0.05 ms after the write.
8. WHILE the S2b window (2.00 ms to 3.40 ms) runs the system SHALL drive the inductor current below 0 A at least once and report its minimum.
9. WHILE the S2b window runs the system SHALL keep `buck.sink_oc` low in every row (the −3 A limit is never hit).
10. WHILE the S2b window runs the system SHALL keep `fault` = `Clear` and `mode` = `Regulate` in every row (no OVP).

**Independent Test**: the `setpoint` run's tests in `tests/buck_scenarios.rs`.

---

### P1: Load and line steps, S3 and S4, at 3.3 V ⭐ MVP

**User Story**: As the buck's designer, I want the load and line steps judged at 3.3 V so that the compensator's transient response meets SPEC §2 and §10.

**Why P1**: U3: "load tests at 3.3 V".

**Acceptance Criteria** (run `load_line`; the load step up at 1.25 ms, down at 1.55 ms; `r_load` 1.65 Ω at 1.85 ms; the line step up from 2.05 ms, down from 2.35 ms):

1. WHEN `load_step` rises at 1.25 ms THEN the system SHALL keep `vout` within 165 mV of 3.3 V until 1.55 ms.
2. WHEN `load_step` falls at 1.55 ms THEN the system SHALL keep `vout` within 165 mV of 3.3 V until 1.85 ms.
3. WHEN `load_step` rises THEN the system SHALL settle `vout` into 3.3 V ± 1 % no later than 200 µs after the edge.
4. WHEN `load_step` falls THEN the system SHALL settle `vout` into 3.3 V ± 1 % no later than 200 µs after the edge.
5. WHILE the S3 windows (1.25 ms to 1.85 ms) run the system SHALL keep `pgood` high in every row.
6. WHILE the S3 windows run the system SHALL keep `buck.oc` low in every row (no current limit).
7. WHEN Vin steps 12 V to 16 V from 2.05 ms THEN the system SHALL keep `vout` within 165 mV of 3.3 V until 2.35 ms.
8. WHEN Vin steps 16 V to 12 V from 2.35 ms THEN the system SHALL keep `vout` within 165 mV of 3.3 V until 2.65 ms.
9. WHEN Vin steps up THEN the system SHALL settle `vout` into 3.3 V ± 1 % no later than 200 µs after the step's start.
10. WHEN Vin steps down THEN the system SHALL settle `vout` into 3.3 V ± 1 % no later than 200 µs after the step's start.
11. WHILE the S4 windows (2.05 ms to 2.65 ms) run the system SHALL keep `fault` = `Clear` and `mode` = `Regulate` in every row.

**Independent Test**: the `load_line` run's S3 and S4 tests.

---

### P1: Overcurrent and over-voltage, S5 and S6 ⭐ MVP

**User Story**: As the buck's designer, I want the protections judged against SPEC §8 and §10 so that a fault enters `Hiccup` on time and a retry recovers.

**Why P1**: The protections are the supervisor's purpose.

**Acceptance Criteria** (run `faults`; `r_step` 0.3 Ω from time 0; `load_step` rises at 1.25 ms and falls at 3.32 ms; `inject` rises at 4.50 ms):

1. WHILE the converter is current-limited, from the first rise of `buck.ctl.cut` to the entry into `Hiccup`, the system SHALL keep `buck.ctl.cut` high in every row (every period is cut).
2. WHILE the converter is current-limited the system SHALL keep the inductor current at or below 7.6 A (7.5 A plus SPEC §10.1's reaction bound).
3. WHEN `load_step` rises at 1.25 ms THEN the system SHALL show `mode` = `Hiccup` with `fault` = `OverCurrent` first between 16 µs and 20 µs after the edge.
4. WHEN the ADC code first leaves `reference ± reference/16` after the overload THEN the system SHALL show `pgood` low no later than 8 µs after.
5. WHILE the converter is in `Hiccup` the system SHALL keep both gate commands low in every row.
6. WHEN the converter has entered `Hiccup` THEN the system SHALL show `mode` = `SoftStart` first exactly 2.000 ms (1000 samples) later, within one grid row, with `load_step` still high.
7. WHEN the overload is removed during the retry's soft-start THEN the system SHALL reach `mode` = `Regulate` and show `fault` = `Clear` from that row on.
8. WHEN `inject` rises at 4.50 ms THEN the system SHALL show `buck.sink_oc` high at least once before the entry into `Hiccup`.
9. WHILE the back-drive runs, from `inject`'s edge to the entry into `Hiccup`, the system SHALL keep the minimum inductor current within −3.0 A ± 0.1 A.
10. WHEN `vout` first exceeds 3.81 V after `inject` rises THEN the system SHALL show `mode` = `Hiccup` with `fault` = `OverVoltage` first no earlier than that row and no later than 4.52 µs after it (two consecutive samples over the threshold, SPEC §8.4 and §10.1).
11. WHEN the over-voltage `Hiccup` is entered THEN the system SHALL report the time from `inject`'s edge (SPEC ≈ 80 to 120 µs) and the mean rise rate of `vout` from 3.4 V to 3.8 V (SPEC ≈ 7 mV/µs).
12. WHILE the over-voltage `Hiccup` lasts to the end of the run the system SHALL keep both gate commands low in every row from one grid row after the entry.

**Independent Test**: the `faults` run's S5 and S6 tests.

---

### P1: Brown-out and disable, S7 and S8 ⭐ MVP

**User Story**: As the buck's designer, I want the converter's response to losing its input or its enable judged so that it stops switching at once and keeps its fault code.

**Why P1**: SPEC §8's "leaving the enabled state goes to `Off` from any mode, at once".

**Acceptance Criteria** (run `brownout`: Vin falls 12 V to 7.5 V from 1.25 ms, holds, rises to 12 V from 1.40 ms; run `load_line`: `r_load` back to 6.6 Ω at 2.65 ms, `en` falls at 2.80 ms):

1. WHEN `buck.vin` falls through 8.0 V THEN the system SHALL show `mode` = `Off` first within 2.02 µs after the crossing (the next sample), and not before it.
2. WHILE `mode` = `Off` in the brown-out the system SHALL keep both gate commands low in every row from one grid row after its entry.
3. WHILE `mode` = `Off` in the brown-out the system SHALL keep `pgood` low in every row.
4. WHILE `mode` = `Off` in the brown-out the system SHALL keep `fault` at the value it held before the brown-out (`Clear`).
5. WHEN `buck.vin` rises through 8.5 V THEN the system SHALL show `mode` = `SoftStart` first within 2.02 µs after the crossing, and `Off` in every row between the entry into `Off` and that crossing.
6. WHEN `en` falls at 2.80 ms in `Regulate` THEN the system SHALL show `mode` = `Off` first within 2.02 µs after the edge.
7. WHEN `en` has fallen THEN the system SHALL keep both gate commands low in every row from one grid row after the entry into `Off` to the end of the run.
8. WHEN `en` has fallen THEN the system SHALL show `pgood` low from the entry into `Off` on.
9. WHEN `en` has fallen THEN the system SHALL make `vout` decrease in every row from 10 µs after the entry into `Off` to the end of the run, with the time constant of the load (6.6 Ω ∥ 40 kΩ times 220 µF, 1.452 ms) within ± 2 % over that span.

**Independent Test**: the `brownout` run's tests and the `load_line` run's S8 tests.

---

### P1: Steady state at nine corners, S9 ⭐ MVP

**User Story**: As the buck's designer, I want the regulation, ripple and idle-tone claims judged at every corner of SPEC's operating range so that §4.3, §3.3 and §6.5 hold where they matter.

**Why P1**: S9 is the converter's DC specification.

**Acceptance Criteria** (run `steady_state`: full load, 5 A, at each setpoint; setpoint 0.8 V at Vin 9, 12, 16 V, then 3.3 V at 16, 12, 9 V, then 5.0 V at 9, 12, 16 V; each corner measured over a 64 µs window that starts 200 µs after the last ramp or line step):

1. WHEN a corner's window is measured THEN the system SHALL keep the mean of `vout` within the §4.3 total of its setpoint: ± 6.5 mV at 0.8 V, ± 14.7 mV at 3.3 V, ± 20 mV at 5.0 V.
2. WHEN a corner's window is measured THEN the system SHALL keep the peak-to-peak of `vout` at or below 10 mV.
3. WHEN a corner's window is measured THEN the system SHALL keep the peak-to-peak of `buck.code` at or below 1 code (no sub-harmonic idle tone above 1 code).
4. IF the means of a window's two halves differ by more than one code (6.25 mV) THEN the test SHALL fail naming the corner as not settled, before judging its DC error.

**Independent Test**: the `steady_state` run's tests.

---

### P1: Shoot-through and the fixtures' integrity, S10 ⭐ MVP

**User Story**: As the reviewer, I want every run held to the no-shoot-through monitor and every family run exactly once so that a pass covers the whole suite and a failure is reported without a second simulation.

**Why P1**: S10 applies to every scenario; a re-run on failure would cost minutes per test.

**Acceptance Criteria**:

1. The system SHALL run the five families once each, in parallel threads of one target, each with its own build directory.
2. IF a family's run fails THEN every test of that family SHALL fail with the run's failure message without running it again.
3. WHEN a family's run ends THEN the system SHALL print its wall time and every value its tests measure.
4. WHEN any scenario run ends THEN its `checks` stream SHALL hold at least one record and no failed record.

**Independent Test**: the five `<family>_passes_every_check` tests.

---

### P1: The buck follows its SPEC ⭐ MVP

**User Story**: As the buck's designer, I want the model and its SPEC to agree before the scenarios judge it so that a failure means the model and not the text.

**Why P1**: The user's rule: code follows spec.

**Acceptance Criteria**:

1. WHEN the supervisor enters `Hiccup` THEN the model SHALL hold `Hiccup` for exactly 1000 samples (2 ms) and enter `SoftStart` at the 1000th sample after the entry (SPEC §8.5).
2. The SPEC SHALL give §4.3 a 3.3 V column whose rows are quantization ± 3.1 mV, aliasing ± 1.7 mV, divider ± 6.6 mV, reference ± 3.3 mV and total ± 14.7 mV.
3. The SPEC SHALL give §10 S2b the tolerance "at 1.29 ms ± 0.05 ms".
4. The SPEC SHALL hold a §10.1 "How the bench measures" that defines the stimulus shapes, the settling time, the deviation, the sample-time resolutions (2.02 µs for the next sample, 4.52 µs for two consecutive samples), the current-limit reaction bound (0.1 A) and S8's decay bound (± 2 %), each with its derivation.
5. The SPEC SHALL name the cleared fault code `Clear` wherever it names it.

**Independent Test**: `buck_supervisor` holds the hiccup length; `grep` holds the SPEC text.

---

### P2: The documents say the scenarios run

**User Story**: As a reader, I want the buck's README and `GAPS.md` to say where the scenarios run so that U3 reads closed.

**Why P2**: Documentation follows the delivery.

**Acceptance Criteria**:

1. The `examples/buck/README.md` SHALL point at `tests/buck_scenarios.rs` and `tests/e2e/buck/` for S2 to S10 and SHALL NOT say that no host exists.
2. The `GAPS.md` SHALL mark U3 closed by `buck-scenarios` and SHALL NOT say that the buck's scenarios have no host.

**Independent Test**: `grep` over the two files.

---

## Edge Cases

- IF a family's run fails THEN its tests SHALL report the failure once each, with no second run (BS-08 AC 2).
- IF a scenario's measured event never happens (no `Hiccup`, no crossing) THEN its test SHALL fail naming the event and the window, never pass vacuously (BS-02 AC 5 and AC 6 return none).
- WHEN a window's last row is outside the band THEN the settling time SHALL be none and the test SHALL fail (BS-02 AC 1).
- IF S9's window is not steady THEN the test SHALL say so before judging the DC error (BS-07 AC 4).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| BS-01 | P1: The scenario runs | Tasks | Pending |
| BS-02 | P1: Measurements defined once | Tasks | Pending |
| BS-03 | P1: Setpoint changes, S2 and S2b | Tasks | Pending |
| BS-04 | P1: Load and line steps, S3 and S4 | Tasks | Pending |
| BS-05 | P1: Overcurrent and over-voltage, S5 and S6 | Tasks | Pending |
| BS-06 | P1: Brown-out and disable, S7 and S8 | Tasks | Pending |
| BS-07 | P1: Steady state, S9 | Tasks | Pending |
| BS-08 | P1: Shoot-through and fixtures, S10 | Tasks | Pending |
| BS-09 | P1: The buck follows its SPEC | Tasks | Pending |
| BS-10 | P2: The documents | Tasks | Pending |

Note: BS-03 AC 1 (the precondition at 1.25 ms) is written once and holds for every 3.3 V family; the tasks give it one check per family.

**Coverage:** 10 requirements, 10 mapped to tasks, 0 unmapped.

---

## Sources

- `examples/buck/SPEC.md` §2, §4.3, §7, §8, §10 - every expected value and tolerance.
- `examples/buck/README.md` "Driving it from a host" - the knobs (`en`, `sp_wr`, `sp_data`, `load_step`, `inject`, `vin_nom`, `r_step`) and the probe points.
- `GAPS.md` U3, H1 - the gap this closes; `ROADMAP.md` lane B row `buck-scenarios`.
- `.specs/features/host-ii/` - `Session::drive`, `Session::set_at`, `max_steps`, `E0801` to `E0806`.
- `.specs/features/long-run/` - the `Recorder`, `Keep::Last`, the print grid, the 675 000 steps of the 600 µs start.
- `.specs/features/root-e2e/`, MD-72 - `tests/e2e/` as the scenarios' home, the root naming rule.
- `crates/piperine-api/tests/buck_long.rs`, `crates/piperine-cli/tests/buck.rs` - the existing 600 µs start fixtures.

---

## Success Criteria

- [ ] `cargo test -p piperine --test buck_scenarios` passes with every SPEC §10 S2 to S10 criterion judged.
- [ ] The target's wall time is that of its longest run (≈ 4.7 ms simulated), printed by the target.
- [ ] No tolerance of SPEC is loosened; every bound the bench adds is in SPEC §10.1.

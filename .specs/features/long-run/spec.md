# Long Run Specification

**Approved**: 2026-11-17 - user: "fecha o msl-cli, faz o long-run e suas dependencias, faz um teste mais completo do buck" (answers Q1-Q4 all recommended)

The approval and the four answers reached the planner through the orchestrator on 2026-11-17; they are recorded in `context.md`.

## Questions answered (user, 2026-11-17)

| # | Question | The user's answer | Decided |
| - | -------- | ----------------- | ------- |
| Q1 | What does the plot of a run with `print_step` draw? | "As linhas da grade (Recommended)" | The plot draws the run's rows: the grid when `print_step` is given, every accepted step when it is not. `msl-cli` CLI-22 AC 7 is amended |
| Q2 | Which run of `examples/buck/Piperine.toml` is the long one? | "startup inteira (Recommended)" | `startup` drops `from` and prints 0 to 600 µs on its 0.5 µs grid (1 201 rows). The Book's listing and `crates/piperine-cli/tests/buck.rs` are reconciled. The host test takes this run and overrides `print_step` |
| Q3 | Is a new run key wanted now? | "Nenhuma (Recommended)" | None. `print`, `print_step`, `from` and `stop` are the observation policy; the sampling rule of decision 3 is not selectable |
| Q4 | Which limits judge the 0.8 V start? | "Carregar S1 para 0,8 V (Recommended)" | Overshoot at most 15 mV above 0.8 V; settled inside ±10 mV between 207.5 and 307.5 µs; ripple at most 10 mV peak to peak over the last 40 µs. A miss is a finding reported to the user, never a moved limit |

## Problem Statement

A transient keeps one full snapshot per accepted step: every unknown, every discrete net's bytes and a re-entry state, about 58 KB for the buck (`GAPS.md` §6). The buck takes about 675 000 accepted steps in its 600 µs start (average step 0.9 ns), so its full waveform needs about 39 GB; the run was stopped at 25 GB. The only bound is `from` (`record_from`), which throws the start away: `examples/buck` keeps its last microsecond. The print grid (`print_step`) is applied after the run, over every kept step, so it does not bound anything.

The buck is the flagship mixed-signal prototype and its start has never been seen whole. Its SPEC limits on overshoot, settling and ripple are untested.

## Goals

- [ ] A transient run holds, per accepted step, only the signals it prints; memory does not grow with the number of accepted steps, only with the rows of the result.
- [ ] `print_step` is applied while the run goes, ngspice TSTEP-like: analog interpolated linearly, digital held at its last committed value, no row extrapolated.
- [ ] The buck's whole 600 µs start runs with the full `vout` waveform in under 1 GiB, and a host test in Rust measures overshoot, settling and ripple against `examples/buck/SPEC.md`.
- [ ] The 7 ngspice twins print the same bytes as before; the streamed grid is cross-checked against ngspice's TSTEP output; no tolerance changes.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Multi-rate stepping (digital edges that do not cut analog steps) | Future (user, decision 6). The 675 000 steps stay; this feature bounds memory, not wall time |
| Surrogate or averaged models | Future (user, decision 6) |
| Periodic steady state | The next feature (user, decision 6). This feature keeps the last step's re-entry state so that it can start from one |
| `on above(..) { emit!(..) }` (`P0001`) | Future (user, decision 6). It stays listed in `GAPS.md` §4 |
| Measurements in MSL, or a `.meas` in the manifest | Decision 4: the host test computes them |
| A change to what `piperine test` judges | Decision 4: MSL check records only |
| New MSL syntax or a new builtin | Decision 1: observation is not language |
| The memory of `op`, `dc` and `ac` runs | Their results do not grow with time: one solution, one row per point, one row per frequency |
| A digital-only transient | Refused today (`GAPS.md` §4); the refusal stays |
| Rows written to a file while the run goes, so that even the rows are not held | Decision 2 excepts the output rows. Deferred |
| The buck's scenarios S1b to S9 | They need host stimulus during a run (`ROADMAP.md` queue item 4) |
| A cap on the number of rows | The rows are what the user asked for; no limit is invented |
| A change to the solver's default result | Every solver, transpile and parity test reads the full steps; the new behaviour is opt-in |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Layering | observation policy is the run's (`[run.<name>]` and flags), never MSL | decision 1, "Coloca no manifest também" | y - user |
| What a run records | its printed signals only; memory constant in the run's length except for the rows | decision 2 | y - user |
| The grid | applied while the run goes; analog linear, digital held, no extrapolation; the internal step stays the solver's | decision 3 | y - user |
| Measurements | a host test in Rust through `piperine-api`; `piperine test` unchanged in what it judges | decision 4 | y - user |
| Parity | twins byte-identical, grid cross-checked against ngspice TSTEP, no tolerance retuned | decision 7 | y - user |
| "Held" for a discrete signal on the grid | the net's value after the last instant at or before the row's time, read from the solver's net-change observations; not the value at the last accepted analog step, which `msl-cli` CLI-11 AC 5 says | decision 3 says "last committed value". A digital instant that the analog side does not read runs between two accepted steps, so the last step's value can be stale by up to one analog step. The two rules agree at every accepted step | y - user, with the answers of 2026-11-17; amends CLI-11 AC 5 |
| A discrete net of a real type (`Bit<f64>`) on the grid | held, like every discrete net | It is a carrier, not a node: it changes by commits. `Table::on_grid` interpolates it today because it looks at the cell's type, not at the signal's kind | n - planner; a fix |
| The first row of a gridded run with `from` | the first `k*print_step` not before `from`, interpolated between the steps around it | CLI-11 AC 4 says exactly this. Today the row is skipped when no accepted step lies between `from` and it, because steps before `from` are dropped first. A run with `from` gains that row; the buck's `startup` no longer has `from` (Q2) | y - user, with the answers of 2026-11-17; follows CLI-11 AC 4 |
| No `print_step` | one row per accepted step, as today (CLI-05 AC 2), holding only the printed signals | Approved behaviour; about 160 B per row for three signals instead of 58 KB | n - planner |
| The fine grid of the buck test | 10 ns (`print_step = 1e-8`), 60 001 rows | It equals the run's `max_step`, so no accepted step is longer than a grid cell; 200 rows per switching period keep the interpolation error of the ripple under 0.1 mV | n - planner |
| The ripple window | the last 20 switching periods (40 µs), as `buck_closed_loop` reads | Well after power-good (about 388 µs) | n - planner |
| The settling time | the time of the first row from which every later row is inside 0.8 V ± 10 mV | "settling into ±10 mV" (decision 4) | n - planner |
| When `en` rises in the bench | 1.5 µs: `Host` raises it at the second rising edge of its 1 MHz clock (`benches/startup.msl`) | Read from the bench | n - planner |
| The memory cap of the buck test | 1 GiB of peak live heap during the run | About 40 times under what the run needs today, and far above the expected figure (about 10 MB of rows). The measured peak is printed. A cap derived from a measurement would be tuned to the implementation | n - planner |
| The memory-scaling fixture | `examples/rc` run to 5 µs and to 50 µs with the same 101 rows; the two peaks differ by at most 256 KiB | Small and fast; with one snapshot per step the longer run holds megabytes more | n - planner |
| How memory is measured | a counting allocator in the test target, peak live bytes minus the bytes live when the run starts | Exact and portable; peak resident memory counts the loaded library and the allocator's slack | n - planner |
| Wall time | printed on standard error by the test, never asserted | decision 5 | y - user |
| `piperine test` | its verdict counts checks and keeps the failed records as they arrive, not every record | The buck writes 120 000 check records per run; keeping them all grows with the run's length. What it judges and prints does not change | n - planner |
| Limits of the 0.8 V start | S1's, carried to 0.8 V; a miss is a finding, never a moved limit | "Carregar S1 para 0,8 V (Recommended)" | y - user |
| The plot of a gridded run | the run's rows | "As linhas da grade (Recommended)" | y - user |
| The long run of the manifest | `startup` itself, without `from` | "startup inteira (Recommended)" | y - user |
| New run keys | none | "Nenhuma (Recommended)" | y - user |

**Open questions:** none - all resolved or logged above. Q1 to Q4 were answered on 2026-11-17 (top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: LR-03 AC 3 (a printed signal with no value fails before the first step), LR-05 AC 6 (no row past the last step); the keys' own validation is `msl-cli` CLI-03 and does not change. Failure and partial-failure states: LR-05 AC 7 and the edge cases (a run that ends short gives no table; the records already delivered stay). Idempotency and retry: N/A because a session runs once (`msl-cli` CLI-04). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: LR-01 AC 3 and 4 (observer order against the sink; a rejected attempt is never observed). Data lifecycle: LR-02 (only the last step is kept, with its re-entry state), LR-04 (nothing per step is held). Observability: LR-12 AC 2 (wall time and peak memory printed). External-dependency failure: LR-14 AC 2 (ngspice absent or skipped, as the parity harness already rules). State-transition integrity: LR-02 AC 2 (re-entry from the kept step equals re-entry from a full run's last step).

---

## User Stories

### P1: A transient hands each accepted step to the host and keeps none ⭐ MVP

**User Story**: As a host of the solver, I want each accepted step given to me as it becomes final, and the result to keep only the last one, so that a long run costs no memory per step.

**Why P1**: The 58 KB per step is the solver's; nothing above it can bound memory without this.

**Acceptance Criteria** (LR-01, the step observer):

1. WHEN a transient accepts a step and the instant at its time has run THEN the solver SHALL call the installed step observer once with the step's time, its solution and a reader of every discrete net's bytes.
2. WHEN a transient starts, from an operating point or from a captured step, THEN the solver SHALL call the installed step observer once with the start time before it attempts a step.
3. The solver SHALL call the step observer in increasing time order, each call after the sink has received every observation whose time is not after the call's time.
4. IF an attempt is rejected or retaken at an earlier time THEN the solver SHALL NOT call the step observer for that attempt.
5. WHERE no step observer is installed the solver SHALL give the same result and the same sink stream as before this feature.

**Acceptance Criteria** (LR-02, keeping the last step):

1. WHERE the options ask to keep only the last step the solver SHALL give a result that holds exactly one step, the last accepted, with its solution, its discrete nets and its re-entry state.
2. WHEN a transient re-enters from that kept step THEN the solver SHALL give the same steps as re-entering from the last step of a run that kept every step.
3. WHERE the options do not ask for it the solver SHALL keep every accepted step, as before this feature.

**Independent Test**: A solver test installs an observer on a small mixed circuit, counts its calls against `stats.steps_accepted + 1`, and compares each call's values with the steps of a second run that kept everything.

---

### P1: A run records only what it prints ⭐ MVP

**User Story**: As a user of `piperine run` or `piperine-api`, I want a transient to hold only the signals I print, so that a run's memory depends on what I ask for and not on how many steps the solver took.

**Why P1**: This is the goal: the buck's full waveform fits.

**Acceptance Criteria** (LR-03, the rows of a run with no grid):

1. WHEN a `transient` run with no `print_step` completes THEN the system SHALL give a table whose first column is `time`, with one column per printed signal and one row per accepted step whose time is not before `from`, the start included when `from` is 0.
2. The system SHALL give in that table the same cells as `Table::transient` gives over a run of the same request that kept every step.
3. IF a printed signal has no unknown and no discrete net in the bound circuit THEN the system SHALL fail before the first step with a `Failure` that names the signal.
4. WHEN a `transient` run completes THEN the system SHALL have asked the solver to keep only the last step.

**Acceptance Criteria** (LR-04, the memory of a run):

1. WHEN `examples/rc` runs a transient to 5 µs with `print_step = 5e-8` and to 50 µs with `print_step = 5e-7`, both with `max_step = 1e-8` and the same printed signals, THEN the peak live heap of the two `Session::run` calls SHALL differ by at most 256 KiB.
2. WHEN the same two runs complete THEN each table SHALL hold 101 rows.

**Independent Test**: `cargo test -p piperine-api --test memory`, and the per-step table compared with the old path on `tran_rc`.

---

### P1: The print grid is applied while the run goes ⭐ MVP

**User Story**: As a user, I want `print_step` to be my output step and nothing else, applied as the run goes, so that a fine internal step costs no memory and the rows are what ngspice's TSTEP would give.

**Why P1**: Without it a gridded run still needs every step first.

**Acceptance Criteria** (LR-05, the grid):

1. WHEN a `transient` run gives `print_step = h` THEN the system SHALL give one row per `k*h`, from the first `k*h` not before `from` (0 when absent) to the last `k*h` not past `stop`, whose first cell is `k*h`.
2. WHILE a print grid is in force the system SHALL give a node or a flow its value interpolated linearly between the two accepted steps around the row's time.
3. WHEN a row's time equals the time of an accepted step THEN the system SHALL give a node or a flow that step's value.
4. WHILE a print grid is in force the system SHALL give a discrete net the value it holds after the last instant at or before the row's time.
5. WHILE a print grid is in force the system SHALL make each row from the two accepted steps around it and the net changes between them, holding no earlier step.
6. IF a row's time is past the last accepted step by more than `1e-9 * h` THEN the system SHALL give no row for it and fail the run as it does today for a grid past the last step.
7. IF the transient ends before its `stop` THEN the system SHALL give no table and report the failure `msl-cli` CLI-05 AC 11 names.

**Acceptance Criteria** (LR-06, the same rows as the grid applied afterwards):

1. WHEN a run prints only nodes and flows THEN the system SHALL give a gridded table equal, cell for cell and bit for bit, to `Table::on_grid(h, from, stop)` applied to the per-step table of the same request with `from` 0.
2. WHEN a run prints a discrete net that changes only at accepted steps THEN the system SHALL give that column the same cells as `Table::on_grid` gives.
3. WHEN a discrete net changes at a time strictly between two accepted steps and a row's time lies between the change and the later step THEN the system SHALL give the row the changed value.

**Independent Test**: `cargo test -p piperine-api --test grid`: `tran_rc` on both backends against `Table::on_grid`, and a small clocked design whose counter changes between analog steps.

---

### P1: The observation policy is the run's ⭐ MVP

**User Story**: As a user, I want what a run observes to be written in its `[run.<name>]` table or given by flags, so that the model says what it is and the run says what to look at.

**Why P1**: Decision 1. It fixes where every later observation option goes.

**Acceptance Criteria** (LR-07):

1. The system SHALL give every key of a run table a flag of `piperine run` that sets it, and no flag that sets a run's observation without a key.
2. WHEN the same `print`, `print_step`, `from` and `stop` are given once by a run table and once by flags THEN the system SHALL give equal tables.
3. The system SHALL read the observation policy of a transient from its `RunSpec` only.

**Independent Test**: `cargo test -p piperine-cli --lib` (the key and flag pairing test) and one `piperine-api` test that runs `tran_rc` both ways.

---

### P1: The buck's whole start, measured ⭐ MVP

**User Story**: As the designer of the buck, I want its whole 600 µs start on a fine grid, measured against my SPEC by a Rust test, so that overshoot, settling and ripple are numbers a regression can break.

**Why P1**: The user's goal, decision 5.

**Acceptance Criteria** (LR-08, the run):

1. WHEN the run `startup` of `examples/buck` runs through `piperine-api` with `print_step` overridden to `1e-8` and `print` overridden to `buck.vout`, `buck.ctl.reference`, `mode` and `pgood` (a transient to 600 µs, `max_step = 1e-8`, no `from`) THEN the system SHALL give a table of 60 001 rows whose times are `k * 1e-8` for `k` from 0 to 60 000.
2. WHEN that run completes THEN every record of its `checks` stream SHALL have `pass` true, and the stream SHALL hold at least one record.

**Acceptance Criteria** (LR-09, overshoot):

1. WHEN that run completes THEN the largest `buck.vout` of the table SHALL be at most 0.815 V (0.8 V plus the 15 mV of SPEC §10 S1).

**Acceptance Criteria** (LR-10, settling):

1. WHEN that run completes THEN the settling time, the time of the first row from which every later row has `buck.vout` inside 0.8 V ± 10 mV (SPEC §2), SHALL be at least 207.5 µs and at most 307.5 µs (`en` at 1.5 µs, the ramp of 0.256 ms of SPEC §7.2, the ±0.05 ms of S1).
2. WHEN that run completes THEN the last row SHALL hold `buck.ctl.reference` 128, `mode` 2 and `pgood` true (SPEC §7.1, §8, §8.6).

**Acceptance Criteria** (LR-11, ripple):

1. WHEN that run completes THEN the difference between the largest and the smallest `buck.vout` of the rows of the last 40 µs SHALL be greater than 0 and at most 10 mV (SPEC §2).

**Acceptance Criteria** (LR-12, the long-run checks):

1. WHEN that run completes THEN the peak live heap during `Session::run`, less the heap live when it started, SHALL be at most 1 GiB.
2. WHEN that run completes THEN the test SHALL print the wall time of the run and the measured peak on standard error, asserting neither against a time.

**Acceptance Criteria** (LR-13, the manifest):

1. WHEN `piperine run startup` runs in `examples/buck` THEN the system SHALL print 1 201 rows, from time 0 to 600 µs on the run's `print_step` of 0.5 µs.
2. WHEN `piperine test` runs in `examples/buck` THEN the system SHALL print `PASS idle` and `PASS startup` lines and `2 runs, 2 passed, 0 failed`, as before this feature.

**Independent Test**: `cargo test -p piperine-api --test buck_long` (one shared run, about two and a half minutes), and `cargo test -p piperine-cli --test buck`.

---

### P1: Parity holds, and the grid is checked against ngspice ⭐ MVP

**User Story**: As the owner of the parity claim, I want every existing comparison untouched and the new grid compared with ngspice's own print grid, so that the feature cannot move a number unseen.

**Why P1**: Decision 7: parity is the top priority.

**Acceptance Criteria** (LR-14, what does not move):

1. WHEN each of the 7 twins runs from the command line with the flags of `crates/piperine-cli/tests/twins.rs` THEN the system SHALL print, on each backend, the bytes it printed before this feature.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran`, `parity_corpus` and `piperine-solver`'s `parity_baseline` with their sources and `crates/piperine-parity/src/contract.rs` unedited.

**Acceptance Criteria** (LR-15, the grid against ngspice TSTEP):

1. WHEN `tran_rc` runs through `piperine-api` to 10 µs with `print_step` equal to ngspice's print step (0.1 µs) and `max_step` a tenth of it THEN the system SHALL give, for every row of ngspice's export, a row whose time is within `1e-9 * print_step` of it.
2. WHEN those rows are paired THEN `v(out)` of each pair SHALL agree under `ToleranceContract::TRAN_VOLTAGE`, with no interpolation done by the test.
3. WHERE the ngspice comparison is disabled (`skip`) the test SHALL still run the twin and print the harness's skip line.

**Independent Test**: `cargo test -p piperine-parity --test parity_grid`, and `cargo test -p piperine-cli --test twins` with the byte goldens captured before any code changes.

---

### P2: `piperine test` judges a long run without keeping its records

**User Story**: As a user of `piperine test`, I want a run's verdict made as its records arrive, so that 120 000 passing checks cost nothing.

**Why P2**: The run's table is bounded by P1; the test command's own list of records is the next thing that grows with length. The command works today without it.

**Acceptance Criteria** (LR-16):

1. WHEN records reach a verdict one at a time THEN the system SHALL count the records of the `checks` stream and keep only those whose `pass` is not true.
2. WHEN `piperine test` judges a run THEN the system SHALL print the same `PASS` or `FAIL` line, the same failed records and the same summary as before this feature.
3. The system SHALL judge a run by its `checks` records only.

**Independent Test**: `cargo test -p piperine-api verdict` and `cargo test -p piperine-cli --test commands`.

---

### P2: A plot of a streamed run

**User Story**: As a user, I want `--plot` to keep working on a long run without holding every step.

**Why P2**: The plot exists and must not break the bound (Q1: it draws the grid rows).

**Acceptance Criteria** (LR-17):

1. WHEN a `transient` run with `plot` and `print_step` completes THEN the system SHALL draw the rows of its table.
2. WHEN a `transient` run with `plot` and no `print_step` completes THEN the system SHALL draw every accepted step of the printed signals.

**Independent Test**: `cargo test -p piperine-api plot`.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md`, the buck's README and the Book, I want them to describe the bounded run.

**Why P3**: No behaviour depends on it; the Book's listings are held to the binary by test.

**Acceptance Criteria** (LR-18):

1. The system SHALL keep every command-line listing of the Book equal to the binary's output, the Book edited where the output changed.
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer say that a host run keeps every accepted step, SHALL still list `on above(..) { emit!(..) }` under what is refused, and `ROADMAP.md` SHALL list multi-rate stepping, surrogate models and periodic steady state as future work.
3. WHEN this feature merges THEN `examples/buck/README.md` and the comments of `examples/buck/Piperine.toml` SHALL describe the runs as they are.

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN a transient's `print_step` is greater than `stop` and `from` is 0 THEN the system SHALL give the single row at time 0.
- WHEN `from` is greater than 0 and no `print_step` is given THEN the system SHALL give no row for the start.
- WHEN a row's time is past the last accepted step by at most `1e-9 * print_step` THEN the system SHALL give the row the last step's values.
- WHEN a discrete net changes at a time within `1e-9 * print_step` of a row's time THEN the system SHALL give the row the changed value.
- WHEN a discrete net's known byte is clear at a row's time THEN the system SHALL give the cell as unknown.
- IF the design has no analog unknown THEN the system SHALL refuse the transient as it does today.
- IF a record cannot be decoded during a streamed run THEN the system SHALL report that failure first, as it does today.
- WHEN a printed signal is named twice THEN the system SHALL give two equal columns.

---

## Amendments to `msl-cli` criteria

`msl-cli` is approved. This feature changes what three of its criteria say; the user accepted the three with the answers of 2026-11-17.

| `msl-cli` says | This feature says | Why | Confirmed |
| -------------- | ----------------- | --- | --------- |
| CLI-11 AC 5: a discrete signal on the grid takes the value of the last accepted step at or before the row | the value after the last instant at or before the row (LR-05 AC 4) | decision 3: "last committed value" | y - user, 2026-11-17 |
| CLI-22 AC 7: a plot of a gridded run still draws every accepted step | it draws the run's rows (LR-17 AC 1) | Q1: "As linhas da grade (Recommended)" | y - user, 2026-11-17 |
| Assumption "Transient rows": with `from`, only the steps at or after it are kept | no step is kept; rows start at the first grid time not before `from`, interpolated from the steps around it (LR-05 AC 1) | the window is the recorder's, not the solver's | y - user, 2026-11-17 |

CLI-03 AC 1 (the list of keys) does not change (Q3: no new key). CLI-05 AC 2 (one row per accepted step with no grid) stands.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| LR-01 | P1: the solver hands each step to the host | Tasks | In Tasks |
| LR-02 | P1: the solver hands each step to the host | Tasks | In Tasks |
| LR-03 | P1: a run records only what it prints | Tasks | In Tasks |
| LR-04 | P1: a run records only what it prints | Tasks | In Tasks |
| LR-05 | P1: the print grid while the run goes | Tasks | In Tasks |
| LR-06 | P1: the print grid while the run goes | Tasks | In Tasks |
| LR-07 | P1: the observation policy is the run's | Tasks | In Tasks |
| LR-08 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-09 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-10 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-11 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-12 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-13 | P1: the buck's whole start, measured | Tasks | In Tasks |
| LR-14 | P1: parity holds | Tasks | In Tasks |
| LR-15 | P1: parity holds | Tasks | In Tasks |
| LR-16 | P2: `piperine test` without keeping records | Tasks | In Tasks |
| LR-17 | P2: a plot of a streamed run | Tasks | In Tasks |
| LR-18 | P3: the documents say what is true | Tasks | In Tasks |

**Coverage:** 18 total, 18 mapped to tasks, 0 unmapped.

---

## Sources

- The user, 2026-11-17, through the orchestrator: the goal and decisions 1 to 7 (`context.md`). Verbatim: "Coloca no manifest também".
- `examples/buck/SPEC.md` §2, §3.3, §7.1, §7.2, §8.6, §10 S1: the limits of LR-09 to LR-11.
- `GAPS.md` §6: the 58 KB per accepted step and the 25 GB run.
- `.specs/features/msl-cli/spec.md` CLI-03, CLI-05, CLI-11, CLI-22 and `contract.md` D10: what the command line and `piperine-api` do today.
- `.specs/STATE.md` MD-28, MD-38, MD-60, MD-62, MD-65.
- `tests/ngspice/tran_rc.cir`: ngspice's TSTEP grid (`.options interp`, `tran 0.1u 10u uic`).

---

## Success Criteria

- [ ] `cargo test -p piperine-api --test buck_long` passes: 60 001 rows of the buck's start, overshoot, settling and ripple inside the SPEC's limits, peak heap under 1 GiB, wall time printed.
- [ ] The memory of a run does not depend on its length at equal row count (LR-04).
- [ ] The 7 twins print the bytes they printed before, and `parity_grid` agrees with ngspice's TSTEP rows under the unchanged `TRAN_VOLTAGE`.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

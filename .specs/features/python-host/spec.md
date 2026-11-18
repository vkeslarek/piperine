# Python Host Specification

> **Deferred (2026-11-18).** User, 2026-11-18, verbatim: "Eu acho que podemos fazer tudo em RUST por enquanto e DEPOIS pensamos em integrar python. Senão cada mod da api do host vai tomar mais tempo ainda". Every host feature is built and tested in Rust first (`piperine-api`); this feature starts only after the host features it wraps have landed, and none of them waits on it or carries Python work.

**Size:** L, about 35 tasks (P1 alone about 20). The host-action and analysis stories grow with `host-iii` and its siblings; each is gated on its dependency by a WHERE criterion.

## Open questions (user decisions, asked 2026-11-17)

The shape of the Python host is the user's. In the Verilog-AMS era the user said "Python is the host surface"; the crate `piperine-python` was then removed with the old front end (user, `solver-restructure` decision 5: "Vamos remover o IR, cranelift e Python por agora"). The spec (Part VIII) sketches its host in Python and `examples/buck/README.md` writes the buck's scenarios as Python with no implementation (`GAPS.md` B4). Every criterion below is written for the recommended options.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How is Python bound to the Rust library? | (a) a PyO3 extension crate `crates/piperine-python` over `piperine-api`, built with maturin as an `abi3` wheel, plus a typed pure-Python package `piperine` over it; (b) a pure-Python package that runs the `piperine` binary and reads JSON Lines; (c) a C ABI on `piperine-api` and `cffi` | **(a)**. Waits and drives during a run (`host-iii`) need an in-process session; (b) cannot suspend a run, (c) duplicates what PyO3 generates. This is the shape the removed crate had (MD-27: native `_piperine` plus a typed facade) |
| Q2 | What surface does `import piperine` give? | (a) a mirror of `piperine-api`, same names (`Project`, `Session`, `Table`, `Record`, `Failure`), nothing else; (b) the spec's sketch: `pp.Session("<path>", top=.., params=.., time_resolution=..)`, `sim.drive`, `await sim.wait`; (c) both in two layers: the native module `_piperine` mirrors `piperine-api` one to one and is held to it by a surface parity test (MD-22, MD-27), and the facade `piperine` gives the sketch's form over it | **(c)**. One rule set (MD-65 item 1: every rule is a `piperine-api` method with a Rust test), and the Python users read in the spec works as written |
| Q3 | How does Python wait on a running design (`host-iii`)? | (a) `asyncio`: `await sim.rise("dv")`, `await sim.wait(t)`, `asyncio.gather` for `fork`/`join`; (b) blocking calls only: `sim.run_until([...])`; (c) both: the blocking call is the core (it is `host-iii`'s `run_until`), and an `asyncio` layer schedules coroutines over it, one session per event loop | **(c)**. The spec's sketch is `async`; a script or a notebook cell wants a plain call. Both are thin over one pull primitive |
| Q4 | What does a table give Python? | (a) `numpy` as a required dependency: a column is a `numpy` array, `to_pandas()` and `to_polars()` when those are installed; (b) no required dependency: lists, with the conversions optional; (c) Apache Arrow (`pyarrow`) | **(a)**. Every user of a waveform reaches for `numpy`; a copy into lists of a 60 001-row run is waste. `pandas` and `polars` stay optional |
| Q5 | Plotting | (a) none: the user plots columns with what they like; (b) `table.plot()` drawing with `matplotlib` when installed (an extra `piperine[plot]`), and the Rust SVG plot shown inline in Jupyter (`_repr_svg_`); (c) a Piperine plotting library | **(b)**. Two small conveniences over libraries users have; nothing to maintain beyond them |
| Q6 | Distribution | (a) wheels on PyPI for Linux, macOS and Windows, built by CI; (b) install from the repository only (`pip install ./crates/piperine-python`, which needs a Rust toolchain); (c) (b) now, (a) when the project publishes a release | **(c)**. The compiled backend runs `rustc` at run time anyway (MD-63), so a Python user of mixed designs needs a Rust toolchain today; PyPI wheels are a release decision |
| Q7 | How are Python testbenches run? | (a) by `pytest`: a testbench is a test function (with `pytest-asyncio` for `async` ones); `piperine test` keeps running MSL runs only; (b) `piperine test` also discovers `tests/*_tb.py` and runs them | **(a)**. `pytest` is what Python users run; a second discovery rule in the CLI duplicates it |

## Problem Statement

There is no Python host (`GAPS.md` B4). The spec's host sketches (§23.1, §23.2) and `examples/buck/README.md`'s scenarios S1, S3 and S5 are Python with no implementation. `piperine-api` is the Rust host library (MD-20, MD-65): `Project`, `RunSpec`, `Session`, `Table`, `Record`, `Plot`, `Verdict`, `Failure`; `host-ii` adds session options, metadata, drives and timed `set`s, and the `host-iii` group adds waits, forces, continued transients, more analyses, sweeps, Monte Carlo, optimization and gradients. A designer who works in notebooks and scripts has none of it.

## Goals

- [ ] `pip install` of the repository's Python package, then `import piperine`, opens a project, runs a run and gives its table as `numpy` columns and its records as Python objects.
- [ ] Every result a Python host gets equals the Rust host's and the command line's for the same run (same CSV bytes on the 7 twins).
- [ ] Every `Failure` of `piperine-api` is a Python exception with its code and message.
- [ ] As `host-ii` and the `host-iii` group land, their calls are in Python with the same names, the waits as coroutines; the buck's S1 runs as a Python test.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Python devices or Python plugins (the PHDL era's `@pip.device`) | The plugin system was removed (user); a device is MSL compiled by `piperine-transpile` |
| Rules computed in Python that Rust does not have | MD-65 item 1: every rule is a `piperine-api` method; the facade only reshapes |
| PyPI wheels | Q6 (c): a release decision |
| A Python REPL launcher in the CLI | `cli-packages` CLP-10 (its Q2 (b)) |
| A notebook widget or a GUI | Not in `GAPS.md` |
| Running without a Rust toolchain on the compiled backend | MD-63: the backend runs `rustc`; refused loud (PYH-14) |
| The buck's scenarios S2 to S9 as tests of record | `buck-scenarios` (U3) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Binding | PyO3 and maturin, `abi3` (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| Surface | two layers held by a parity test (Q2 (c)) | Q2 recommendation | n - open (Q2) |
| Waits | blocking core and an `asyncio` layer (Q3 (c)) | Q3 recommendation | n - open (Q3) |
| Tables | `numpy` required (Q4 (a)) | Q4 recommendation | n - open (Q4) |
| Plotting | `matplotlib` optional, SVG in Jupyter (Q5 (b)) | Q5 recommendation | n - open (Q5) |
| Distribution | from the repository now (Q6 (c)) | Q6 recommendation | n - open (Q6) |
| Testbenches | `pytest` (Q7 (a)) | Q7 recommendation | n - open (Q7) |
| Python version | 3.10 and later, one `abi3` build | the oldest Python still maintained upstream in 2026 | n - planner |
| The crate and the build | `crates/piperine-python`, outside the root's `default-members` (an extension module needs a Python to link), built and tested by `maturin develop` and `pytest` in its own CI job | `cargo build` at the root must not need Python | n - planner |
| Long runs | the native call releases the GIL while the solver runs; `Ctrl-C` cancels the run at the next accepted step and raises `KeyboardInterrupt` | a 130 s buck start must not freeze the interpreter | n - planner |
| Names | the facade's names are Python's spelling of the Rust names (`Session.set_at`, `Table.columns`); the sketch's verbs (`drive`, `wait`, `rise`, `read`, `probe`) map to `host-ii` and `host-iii` calls | MD-22 | n - planner |

**Open questions:** Q1 to Q7 at the top of this file, awaiting the user.

**Implicit-requirement dimensions.** Input validation and bounds: PYH-03 AC 3, PYH-06. Failure and partial-failure states: PYH-06 (every failure an exception), PYH-13 AC 2 (cancel). Idempotency and retry: PYH-03 AC 4 (a second `run` on one session is refused as in Rust). Auth and rate limits: N/A, a local library. Concurrency and ordering: PYH-10 AC 3 (one session per event loop), PYH-13 AC 1 (GIL released). Data lifecycle: PYH-04 AC 2 (a table owns its arrays after the session is closed). Observability: PYH-05 (records), PYH-06 AC 2 (diagnostics rendered). External-dependency failure: PYH-14 (`rustc` missing), PYH-12 AC 2 (`matplotlib` missing). State-transition integrity: PYH-10 AC 4 (actions on a closed or failed session refused).

---

## User Stories

### P1: Open and run from Python ⭐ MVP

**User Story**: As a designer in a notebook, I want to open my project and run a run, so that I work with Piperine where I already analyse data.

**Why P1**: B4.

**Acceptance Criteria**:

1. WHEN a user runs `pip install ./crates/piperine-python` with a Rust toolchain present THEN the system SHALL build and install a package that `import piperine` loads (PYH-01)
2. WHEN `piperine.Project.open(path)` is called on a directory with a `Piperine.toml` or on a lone `.msl` file THEN the system SHALL give a project with its runs listed by name (PYH-02)
3. WHEN `project.session(run, **overrides)` is called THEN the system SHALL open a session of that run with the overrides applied as the same keys of `[run.<name>]`, a keyword winning over the table as a flag does (PYH-03)
4. WHEN `session.run()` is called THEN the system SHALL run the analysis and give a `Table` (PYH-03)
5. IF an override key is not a run key THEN the system SHALL raise `piperine.RequestError` with code `P0010` naming the key (PYH-03)
6. IF `session.run()` is called a second time THEN the system SHALL raise the failure `piperine-api` gives for a second run (PYH-03)

**Independent Test**: `pytest crates/piperine-python/tests/test_run.py` opens `examples/rc` and runs its run.

---

### P1: Tables and records ⭐ MVP

**User Story**: As a designer, I want the results as arrays and the records as objects.

**Why P1**: B4; a run's output is a table and records (MD-66).

**Acceptance Criteria**:

1. WHEN a user reads `table.columns` and `table["<column>"]` THEN the system SHALL give the column names in the table's order and each column as a `numpy` array of `float64`, an unknown value as `nan` (PYH-04)
2. The system SHALL keep a table's arrays valid after its session is closed (PYH-04)
3. WHERE `pandas` is installed `table.to_pandas()` SHALL give a data frame with the same columns and values; WHERE it is not, the call SHALL raise `ImportError` naming `pandas` (PYH-04)
4. WHEN `table.to_csv(path)` is called THEN the system SHALL write the bytes `piperine run --csv` writes for the same run (PYH-04)
5. WHEN a run emits records THEN `session.records` SHALL give them in emission order, each with its stream, time and fields as Python values (`bool`, `int`, `float`, `str`, `None` for unknown) (PYH-05)
6. WHEN a run is judged as `piperine test` judges it THEN `session.verdict()` SHALL give pass or fail, the failing checks, the `fatal` message (X4) and each target's last margin (X3) (PYH-05)

**Independent Test**: the RC run's `v(out)` column equals the CSV the binary writes.

---

### P1: Failures as exceptions ⭐ MVP

**User Story**: As a script author, I want every failure as a typed exception, so that I catch what I expect.

**Why P1**: a library that prints is not a library (`failure.rs`: "a failure is data").

**Acceptance Criteria**:

1. The system SHALL raise, for each variant of `piperine-api`'s `Failure`, one subclass of `piperine.Failure` (`PackageError`, `DiagnosticsError`, `RequestError`, `NoRunError`, `HostError`, `BackendError`, `SolverError`, `IoError`) carrying its stage, its code where it has one, and its message (PYH-06)
2. WHEN a `DiagnosticsError` is printed THEN its text SHALL be the diagnostics as `piperine check` renders them (PYH-06)

**Independent Test**: a source with a type error raises `DiagnosticsError` whose text equals `piperine check`'s output.

---

### P1: Python and Rust agree ⭐ MVP

**User Story**: As the owner of the host surface, I want Python held to Rust mechanically, so that the two never drift (MD-22, MD-27).

**Why P1**: the removed crate drifted (MD-27 records Rust running analyses Python had no type for).

**Acceptance Criteria**:

1. The system SHALL hold a parity test that enumerates `piperine-api`'s public types and methods and `_piperine`'s, and fails naming each one present on one side only, unless it is listed in a file of declared exceptions with a reason (PYH-07)
2. WHEN each of the 7 twins runs from Python THEN its CSV SHALL equal the bytes `piperine run --csv` writes, on each backend (PYH-07)

**Independent Test**: `pytest crates/piperine-python/tests/test_parity.py`.

---

### P1: Params, metadata and sweeps ⭐ MVP

**User Story**: As a designer, I want to set params, list what I can set and print, and sweep, from Python.

**Why P1**: the first things a script does.

**Acceptance Criteria**:

1. WHEN `params={"<path>": value}` is given to a session THEN the system SHALL apply each as a `set` before the run, as `--param` does (PYH-08)
2. WHEN `session.metadata()` is called THEN the system SHALL give `host-ii`'s metadata (params with type, value, `where`, structural flag; ports; streams) as Python objects (PYH-08)
3. WHEN a `dc` sweep is run THEN the table SHALL have the sweep column first, as the Rust table does (PYH-09)
4. WHERE `sweeps-montecarlo-optimization` has landed the system SHALL give its sweeps, Monte Carlo and optimization runs from Python with the same keys, results and failures (PYH-09)

**Independent Test**: the S2 probe of `GAPS.md` (a parent param set from Python) prints 0.75.

---

### P2: Host actions and waits

**User Story**: As a verification engineer, I want the spec's Python sketch to run: drive, set, force, wait and read during a transient.

**Why P2**: §23.1's sketch; it depends on `host-ii` and `host-iii`.

**Acceptance Criteria**:

1. WHERE `host-ii` has landed `session.drive(port, value, at=t)` and `session.set_at(path, value, at=t)` SHALL schedule the same host actions as the Rust calls, with the same refusals as exceptions (`E0802` to `E0806`) (PYH-10)
2. WHERE `host-iii` has landed `session.run_until(waits, stop=None)` SHALL suspend and return as the Rust call does, and `force`, `release`, `read` and `probe` SHALL act at the suspended instant (PYH-10)
3. WHERE `host-iii` has landed `await sim.wait(t)`, `await sim.rise(path)`, `await sim.fall(path)`, `await sim.changed(path)` and `await sim.record(stream)` SHALL complete at the instant the wait fires, with several coroutines of one session scheduled by one event loop and `asyncio.gather` giving `fork`/`join` (PYH-10)
4. IF a host action is called on a session that failed or was closed THEN the system SHALL raise `HostError` naming the session's state (PYH-10)
5. WHEN the buck's S1 of `examples/buck/README.md` runs as a `pytest` test THEN the two times SHALL be within `examples/buck/SPEC.md` §10's limits (PYH-11)

**Independent Test**: `pytest examples/buck/tests/test_s1.py`.

---

### P2: Analyses and gradients from Python

**User Story**: As a designer, I want every analysis and every result of the Rust host from Python.

**Why P2**: they land in their own features; Python follows each.

**Acceptance Criteria**:

1. WHERE an analysis is run by `piperine-api` (`tf`, `pz`, `sens`, `pss` of `host-ii`; `sp`, Fourier and operating-point variables of `host-analyses`) the system SHALL run it from Python with the same table (PYH-09)
2. WHERE `gradients` has landed `session.gradients()` SHALL give each derivative or the `E0807`/`E0808`/`E0809` diagnostic that replaces it (PYH-09)

**Independent Test**: `sens_divider` from Python gives the table the binary prints.

---

### P2: Long runs and the toolchain

**User Story**: As a user of long mixed runs, I want Python to stay responsive and a missing toolchain to say so.

**Why P2**: the buck's start takes about 130 s in a dev build (`GAPS.md` V12).

**Acceptance Criteria**:

1. WHILE a session runs the system SHALL release the GIL, so that other Python threads run (PYH-13)
2. WHEN the user presses `Ctrl-C` during a run THEN the system SHALL stop at the next accepted step and raise `KeyboardInterrupt`, leaving the session failed (PYH-13)
3. IF a run on the compiled backend needs `rustc` and none is found THEN the system SHALL raise `BackendError` naming `rustc` and saying that the interpreter backend runs the M1 analog subset without it (PYH-14)

**Independent Test**: a thread counter advances while a 1 s transient runs from Python.

---

### P3: Plotting

**User Story**: As a notebook user, I want a quick plot.

**Why P3**: convenience (Q5 (b)).

**Acceptance Criteria**:

1. WHERE `matplotlib` is installed `table.plot()` SHALL draw every printed signal against the first column, one line per signal, the column names as labels (PYH-12)
2. WHERE `matplotlib` is not installed `table.plot()` SHALL raise `ImportError` naming `piperine[plot]` (PYH-12)
3. WHEN a session's plot is displayed in Jupyter THEN the system SHALL show the SVG the Rust `Plot` draws (PYH-12)

**Independent Test**: `table.plot()` on the RC run returns a figure with one line labelled `v(out)`.

---

### P3: The documents

**User Story**: As a reader of the Book and of the buck's README, I want the Python host documented and the sketches made real.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book SHALL have a chapter "Python" whose examples run in a test (PYH-15)
2. WHEN this feature merges THEN `examples/buck/README.md`'s Python SHALL be the code of a test, not an illustration, for every scenario whose calls exist (PYH-15)
3. WHEN this feature merges THEN `GAPS.md` SHALL no longer list B4 (PYH-15)

**Independent Test**: `pytest docs/book/python/`.

---

## Edge Cases

- WHEN a column has no row (a run stopped before its first row) THEN `table["<column>"]` SHALL be an empty array.
- IF a column name is not in the table THEN the system SHALL raise `KeyError` listing the columns.
- WHEN a record field is a composite (once `composite-values` lands) THEN the system SHALL give a `dict` or a `tuple` of its leaves.
- IF two event loops wait on one session THEN the system SHALL raise `HostError` naming the second loop.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PYH-01 | P1: Open and run | - | Pending |
| PYH-02 | P1: Open and run | - | Pending |
| PYH-03 | P1: Open and run | - | Pending |
| PYH-04 | P1: Tables and records | - | Pending |
| PYH-05 | P1: Tables and records | - | Pending |
| PYH-06 | P1: Failures as exceptions | - | Pending |
| PYH-07 | P1: Python and Rust agree | - | Pending |
| PYH-08 | P1: Params, metadata and sweeps | - | Pending |
| PYH-09 | P1: Params, metadata and sweeps; P2: Analyses | - | Pending |
| PYH-10 | P2: Host actions and waits | - | Pending |
| PYH-11 | P2: Host actions and waits | - | Pending |
| PYH-12 | P3: Plotting | - | Pending |
| PYH-13 | P2: Long runs | - | Pending |
| PYH-14 | P2: Long runs | - | Pending |
| PYH-15 | P3: The documents | - | Pending |

**Coverage:** 15 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| B4 | no Python crate | PYH-01 to PYH-07 |
| B4 | `examples/buck/README.md`'s Python has no implementation | PYH-10, PYH-11, PYH-15 |
| B4 | the sketches of §23 have no implementation | PYH-09, PYH-10 (§23.1), PYH-09 AC 4 (§23.2's `Optimize`) |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `piperine-api` as it is | P1 (open, run, tables, records, failures) |
| `host-ii` (in flight) | metadata, drives, timed `set`s, the new analyses |
| `host-iii` (spec) | `run_until`, waits, `force`, continued transients |
| `host-analyses`, `sweeps-montecarlo-optimization`, `gradients` (specs) | the WHERE criteria of PYH-09 |

## Sources

- `GAPS.md` B4; the coverage map of 2026-11-17.
- `docs/spec/part_viii_host.md` §23.1 and §23.2 (the Python sketches, not normative).
- `examples/buck/README.md` (S1, S3, S5 in Python).
- `.specs/STATE.md` MD-20, MD-22, MD-27, MD-63, MD-65, MD-66; the `solver-restructure` decision 5 removing `piperine-python`.
- `.specs/features/python-bindings/` (the PHDL-era plan, not carried over); `crates/piperine-api/src/failure.rs`.

---

## Success Criteria

- [ ] `pip install ./crates/piperine-python` then `import piperine` runs `examples/rc`.
- [ ] The 7 twins give the same CSV bytes from Python as from the binary.
- [ ] The surface parity test passes with its exceptions file reviewed.

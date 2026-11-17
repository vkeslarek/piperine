# Host Analyses Specification

**Size:** M, about 24 tasks. Part of the host-iii group (`.specs/features/host-iii/spec.md` holds the group's "Gaps closed" table).

## Open questions (user decisions, asked 2026-11-17)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does a run name the ports of an S-parameter analysis? The solver's `.sp` takes ports as nodes with a `z0` (`piperine-solver/src/analyses/sp.rs`), and its doc still names PHDL's `@rfport` (V6) | (a) run keys: `ports = ["out", "in"]` (node paths, in the order the matrix indexes them) and `z0 = 50` (one value, or one per port); nothing in the model; (b) a stdlib module `RfPort { num, z0 }` placed in the bench, found by the host; (c) both | **(a)**. The ports are analysis-time, as `sp.rs` already says; a bench stays physics; MD-66 puts observation in the run. With (b), HAN-01 is rewritten around the module and `analog-structure` owns the module |
| Q2 | What closes "distortion" (H2)? ngspice's `.disto` is a small-signal Volterra method that needs the second and third partials of every device; the device ABI (MD-61) carries first partials only | (a) `.disto` on the ABI: a new partials order in the shape, every device compiled with it, parity with ngspice's `.disto`; (b) harmonic and intermodulation distortion from the steady state: the `fourier` key on a `pss` (or long transient) run gives each harmonic, HD2, HD3 and THD; `analysis = "disto"` is refused naming (b); (c) (b) now and (a) as a later ABI feature | **(b)**. It answers the question a designer asks (how much second and third harmonic), runs on what exists (`host-ii` PSS, HAN-05), and has an ngspice oracle (`.four`). (a) is an ABI change for one analysis. With (a) or (c), an ABI feature is opened and HAN-08 changes |
| Q3 | Is the Fourier table a run key, given that `long-run` decided measurements are host code (MD-66 item 3)? | (a) a run key `fourier` with `harmonics`, printing an ngspice `.four` table after the run's own, plus the library function it calls; (b) the library function only, called by host tests | **(a)**. A Fourier of the last period is an analysis of §16.3's family and an ngspice dot command, not a project-specific measurement like overshoot; the function stays in `piperine-api` (MD-65 item 1) and the key is its shell (MD-66 item 2) |

## Problem Statement

`host-ii` gives `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens` and `pss`. `GAPS.md` H2 also lists S-parameters, Fourier and distortion, which `host-ii` moved out: the solver has `.sp` (`core/circuit.rs:172`) with no way to name a port from MSL, and has neither Fourier nor distortion. H4 lists operating-point variables: `--print Top.g.id` is `E0801` because no backend exports a named `let` of `analog` (§11 rule 6) as a watch (`piperine-plan` emits `CROSS` watches only).

## Goals

- [ ] `analysis = "sp"` gives the S-matrix over a frequency sweep, agreeing with ngspice's `.sp` on a twin.
- [ ] A transient or `pss` run with `fourier` prints each printed signal's harmonics and THD, agreeing with ngspice's `.four`.
- [ ] Distortion is answered by HD2, HD3 and THD from the steady state (Q2 (b)).
- [ ] `print` reads a named `let` of `analog` by path after `op`, `dc`, `ac` and in a transient; `piperine info` lists them.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| `Noise` | `analog-operators` (L45) |
| `.disto` as a Volterra small-signal analysis | Q2 (b): refused loud; an ABI change if the user picks (a) |
| A differential S-parameter port (a node pair) | Not in `GAPS.md`; the solver's ports are node to `gnd`. A pair is refused naming the port |
| An `extern "osdi"` model's operating-point variables (§21 rule 9) | `foreign-models` (L12, L33) |
| Sweeps over more than one axis | `sweeps-montecarlo-optimization` |
| Operating-point variables on the interpreter | The interpreter is frozen at the M1 subset (R1); refused naming `--backend rust` |
| Measurements other than Fourier (overshoot, settling, `.meas`) | MD-66 item 3: host code |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| S-parameter ports | run keys `ports`, `z0` (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| Distortion | Fourier of the steady state (Q2 (b)) | Q2 recommendation | n - open (Q2) |
| Fourier surface | run key plus library function (Q3 (a)) | Q3 recommendation | n - open (Q3) |
| S table form | `freq`, then `mag(S<i><j>)` and `phase(S<i><j>)` per entry, row-major, phase in degrees | the AC table's form (`table.rs:120`) | n - planner |
| Fourier window and grid | the last period `[stop - 1/f, stop]` on 200 evenly spaced points interpolated from the rows, ngspice's `fourgridsize` default; `harmonics` default 9 | ngspice `.four`, so the twin compares like with like | n - planner |
| Fourier of a `pss` run | the converged period, no window choice | the period is exact there | n - planner |
| An operating-point variable in a transient | one value per row, interpolated like an analog signal | a named `let` of `analog` is a function of the solution at each step; printing it per row is what a user expects of `print` | n - planner |
| An operating-point variable's path | `<instance>.<let name>`; a name shared by a param and a `let` cannot occur (one namespace in a body) | §21 rule 1 | y - spec |

**Open questions:** Q1 to Q3 at the top of this file, awaiting the user.

**Implicit-requirement dimensions.** Input validation and bounds: HAN-02, HAN-06, HAN-11. Failure states: HAN-02 AC 4 (a singular port matrix), HAN-06 AC 3 (a run shorter than one period). Idempotency and retry: N/A because a session runs once (CLI-04). Auth and rate limits: N/A, a local tool. Concurrency and ordering: N/A, no concurrent access. Data lifecycle: N/A, results live in the run's table. Observability: HAN-10 (metadata lists the variables). External-dependency failure: HAN-04 AC 2 (ngspice absent, the harness's skip rule). State-transition integrity: N/A, no state machine added.

---

## User Stories

### P1: S-parameters ⭐ MVP

**User Story**: As an RF or interconnect designer, I want the S-matrix of my circuit between named ports, so that I read match and gain without a second simulator.

**Why P1**: H2; the solver already computes it.

**Acceptance Criteria**:

1. WHEN a run gives `analysis = "sp"`, `ports` (node paths) and the frequency keys of `ac` THEN the system SHALL give a table with the column `freq` and, for each pair `(i, j)` of ports in order, the columns `mag(S<i><j>)` and `phase(S<i><j>)`, one row per frequency (HAN-01)
2. WHEN `z0` is one number THEN the system SHALL use it for every port (HAN-01)
3. WHEN `z0` is a list THEN the system SHALL use its k-th entry for the k-th port (HAN-01)
4. IF an `sp` run lacks `ports`, or `ports` is empty THEN the system SHALL report `P0010` naming the key (HAN-02)
5. IF a `ports` entry names no node THEN the system SHALL report `E0801` naming it (HAN-02)
6. IF `z0` is not greater than 0, or is a list whose length differs from `ports` THEN the system SHALL report `P0010` naming the key (HAN-02)
7. IF a `ports` entry names a node pair THEN the system SHALL report `P0010` naming the entry and saying that a port is a node to `gnd` (HAN-02)
8. IF the port-terminated matrix is singular at a frequency THEN the system SHALL fail the run with the solver's error naming the frequency (HAN-02)
9. WHEN an `sp` run completes on the command line THEN the system SHALL print and write CSV as `host-ii` HII-08 says, with the analysis named `sp` (HAN-03)
10. WHEN the twin `sp_lpf` (a two-port RC low-pass between two 50 Ω ports) runs `sp` THEN each `S<i><j>` SHALL agree with ngspice's `.sp` of the same circuit under a new S-parameter constant of `ToleranceContract`, with its rationale (HAN-04)
11. WHERE the ngspice comparison is disabled the twin SHALL still run and print the harness's skip line (HAN-04)
12. WHEN this feature merges THEN `sp.rs`'s module doc SHALL describe ports given by the run, not PHDL's `@rfport` (closes V6) (HAN-03)

**Independent Test**: `piperine run tests/ngspice/sp_lpf.msl --analysis sp --ports in,out --z0 50 --from 1e3 --to 1e9 --per-decade 10`.

---

### P1: Fourier ⭐ MVP

**User Story**: As a designer, I want the harmonics of a signal over its last period, so that I read THD as ngspice's `.four` gives it.

**Why P1**: H2 lists Fourier.

**Acceptance Criteria**:

1. WHEN a `transient` run gives `fourier = f` THEN the system SHALL give, after the run's table, one Fourier table per printed analog signal, with the columns `harmonic`, `freq`, `mag`, `phase`, `norm_mag`, `norm_phase` and the rows 0 to `harmonics`, and a `thd` value in percent (HAN-05)
2. WHEN a `pss` run gives `fourier = f` THEN the system SHALL compute the tables over the converged period (HAN-05)
3. IF `fourier` is not greater than 0, or `harmonics` is not an integer of at least 1 THEN the system SHALL report `P0010` naming the key (HAN-06)
4. IF the run is shorter than one period `1/f` THEN the system SHALL report `P0010` naming `fourier` and `stop` (HAN-06)
5. IF `fourier` is given to an analysis other than `transient` and `pss` THEN the system SHALL report `P0010` naming the key and the analysis (HAN-06)
6. WHEN a printed signal is a discrete carrier THEN the system SHALL compute no Fourier table for it and print none (HAN-05)
7. The system SHALL offer the same computation as a `piperine-api` function over a table, a signal, a fundamental and a harmonic count (HAN-05)
8. WHEN the twin `four_clip` (a sine through a diode clipper) runs a transient with `fourier` THEN each harmonic's magnitude and the THD SHALL agree with ngspice's `.four` under a new Fourier constant of `ToleranceContract`, with its rationale (HAN-07)

**Independent Test**: `piperine run tests/ngspice/four_clip.msl --analysis transient --stop 5e-3 --fourier 1e3`.

---

### P1: Distortion from the steady state ⭐ MVP

**User Story**: As an analog designer, I want HD2, HD3 and THD of a stage at steady state, so that I size for linearity.

**Why P1**: H2 lists distortion (Q2 (b)).

**Acceptance Criteria**:

1. WHEN a `pss` or `transient` run gives `fourier` THEN each Fourier table SHALL also give `hd2` and `hd3`, the magnitudes of harmonics 2 and 3 over harmonic 1, in percent (HAN-08)
2. WHEN a bench drives two tones whose frequencies are integer multiples of `fourier` THEN the table SHALL give the intermodulation products at the harmonic rows they fall on (HAN-08)
3. IF a run gives `analysis = "disto"` THEN the system SHALL report `P0010` naming the analysis and saying that distortion is the `fourier` key on a `pss` run (HAN-08)

**Independent Test**: a diode-loaded amplifier with a 1 kHz sine, `pss` with `fourier = 1e3`, gives `hd2` within the contract of ngspice's `.four` on a settled transient.

---

### P2: Operating-point variables

**User Story**: As a device author, I want to read the named `let`s of my `analog` block (a transistor's `gm`, `id`) after an analysis, so that I check a bias without adding a port.

**Why P2**: H4's last part; §21 rule 6.

**Acceptance Criteria**:

1. WHEN `print` names `<instance>.<name>` of a named `let` of `analog` THEN the system SHALL print its value at the solution: once for `op`, per sweep point for `dc`, at the operating point for `ac`, and per row for a transient (HAN-09)
2. WHEN the `let` is not defined in a solution because a branch did not assign it THEN the system SHALL print the unknown value for that row (HAN-09)
3. WHEN a host asks a session for its metadata THEN the system SHALL list every operating-point variable with its path and type, in instance order, and `piperine info` SHALL print them in a table `opvars` (HAN-10)
4. IF `print` names a `let` of a reaction or a function, not of `analog` THEN the system SHALL report `E0801` naming it and saying that only a named `let` of `analog` is an operating-point variable (HAN-11)
5. IF a run on the interpreter prints an operating-point variable THEN the system SHALL report `P0010` naming it and `--backend rust` (HAN-11)
6. WHEN the twin `dio_op` prints the diode's current `let` THEN its value SHALL agree with ngspice's `@d1[id]` under `ToleranceContract::DC_CURRENT` (HAN-09)

**Independent Test**: `piperine run tests/ngspice/dio_op.msl --analysis op --print d1.id`.

---

### P3: The documents

**User Story**: As a reader of the Book and `GAPS.md`, I want the new analyses and keys shown.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book's chapter "The command line" SHALL show one `sp` run, one `fourier` run and one operating-point variable, each a listing held to the binary (HAN-12)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list S-parameters, Fourier and distortion under H2, nor operating-point variables under H4, nor V6 (HAN-12)

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN `ports` names the same node twice THEN the system SHALL report `P0010` naming the node.
- WHEN a Fourier signal is constant THEN the system SHALL give a THD of 0 and every `norm_phase` of 0.
- IF the fundamental's magnitude is 0 THEN the system SHALL print the unknown value for `norm_mag`, `hd2`, `hd3` and `thd`, not a division by zero.
- WHEN an operating-point variable is printed in an `sp` run THEN the system SHALL print its value at the operating point in every row.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| HAN-01 | P1: S-parameters | - | Pending |
| HAN-02 | P1: S-parameters | - | Pending |
| HAN-03 | P1: S-parameters | - | Pending |
| HAN-04 | P1: S-parameters | - | Pending |
| HAN-05 | P1: Fourier | - | Pending |
| HAN-06 | P1: Fourier | - | Pending |
| HAN-07 | P1: Fourier | - | Pending |
| HAN-08 | P1: Distortion | - | Pending |
| HAN-09 | P2: Operating-point variables | - | Pending |
| HAN-10 | P2: Operating-point variables | - | Pending |
| HAN-11 | P2: Operating-point variables | - | Pending |
| HAN-12 | P3: The documents | - | Pending |

**Coverage:** 12 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| H2 | S-parameters | HAN-01 to HAN-04 |
| H2 | Fourier | HAN-05 to HAN-07 |
| H2 | distortion | HAN-08 (Q2 (b); `.disto` refused loud) |
| H4 (part) | operating-point variables | HAN-09 to HAN-11 |
| V6 (incidentally) | `sp.rs` documents PHDL's `@rfport` | HAN-03 AC 12; V6 stays owned by `solver-hygiene`, which may close it first |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `host-ii` (in flight) | the analysis arms, run keys and flags, `pss` (HAN-05 AC 2, HAN-08), `piperine info` (HAN-10) |
| `piperine-plan`, `piperine-transpile` | a `VAR` watch per named `let` of `analog` (lane A's area; named in design) |

## Sources

- `GAPS.md` H2, H4, V6; `host-ii` spec's Out of Scope (S-parameters, operating-point variables).
- `docs/spec/part_vi_io.md` §21 rules 6 and 9; `docs/spec/part_iv_behavior.md` §11 rule 6; `docs/spec/part_v_execution.md` §16.3.
- `crates/piperine-solver/src/analyses/sp.rs`; `crates/piperine-api/src/table.rs:120`.
- `.specs/STATE.md` MD-38, MD-61, MD-65, MD-66.

---

## Success Criteria

- [ ] `sp_lpf`, `four_clip` and `dio_op` agree with ngspice under the contract.
- [ ] The 7 twins print the bytes they printed before.

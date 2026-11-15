# MSL Parity Harness (M1) Specification

**Status**: Draft. Not approved. New feature of the end-to-end slice (`.specs/features/codegen/context.md` S1, S9).

## Problem Statement

The old ngspice cross-check ran on the retired Verilog-AMS front end and was deleted with it (`16bddaae`); its fixtures under `tests/ngspice/*.cir` remain. With `msl-typeck`, `msl-ir`, `msl-elab` and `ir-interp` an MSL model reaches the solver for the first time, and nothing proves the answer is right. This feature is that proof for M1: each fixture is an `.msl` file and its `.cir` twin, both are run, and the results are compared under one written tolerance contract.

## Goals

- [ ] Five twins run end to end from `.msl` source and agree with ngspice: `divider` and `diode_series` (operating point), `diode_iv` (DC sweep), `ac_rc` (AC), `tran_rc` (transient).
- [ ] Every numeric tolerance of the comparison is written once, in `ToleranceContract`, with the reason for its value.
- [ ] A failure names the fixture, the quantity, the point, both values and the bound.
- [ ] A parity claim cannot go green without ngspice having run, unless the person running the suite asked for that.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The other fixtures of `tests/ngspice/` (BJT, MOS, JFET, noise, pole-zero, URC, temperature, homotopy, `uic_rc`) | their devices or analyses are outside M1 (S1) |
| Noise, `tf`, `pz`, `sp`, `sens`, `pss` parity | not M1 analyses |
| A device library for users (`std` or a `spice` crate) | Q4: in M1 the models are a fixture beside the twins |
| The `ngspice_parity_matrix` of the old suite (every device against every analysis) | it gated a model library; M1 has five twins and PAR-07 gates those |
| Changing a `.cir` fixture | the corpus is reused as it is |
| Installing ngspice, or pinning its version | the environment's; the harness prints the version it ran |
| A host session API | `piperine-api` does not build; the harness calls the solver's own drivers |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by code or by a run made while writing this spec. Rows marked `n` wait for the user in `.specs/features/e2e-m1/questions.md`.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Parity is its own small feature: a twin per fixture, one contract per domain, one test per fixture | as stated | S9, user: "Concordo e pode disparar" | y |
| The five fixtures | `divider`, `diode_iv`, `diode_series`, `ac_rc`, and `tran_rc` as the RC transient | S1; `tests/ngspice/tran_rc.cir` exists and ngspice-47 runs all five (checked 2026-11-15) | y |
| Hosting crate | a new crate `crates/piperine-parity` (`publish = false`): the harness is its library, the fixture tests its integration targets | the root package does not compile; MD-28 puts a target in the crate it exercises, and this is the only crate that exercises the whole chain; keeping process spawning out of `piperine-interp` | y |
| Where the twins live | `tests/ngspice/<name>.msl` beside `<name>.cir`, with the shared `devices.msl` | one directory per corpus; `mod devices;` in a twin finds the file by MD-55's on-disk rule | y |
| The MSL models | `.specs/features/e2e-m1/models/*.msl`, moved unchanged | they build on today's POM with zero diagnostics | y |
| No `uic` in MSL | the transient twin declares `out` as a held node (`= 0.0`) | §16 H6: "There is no `uic`"; H1 gives the same start as ngspice's `uic` with a discharged capacitor | y |
| The diode model | Shockley current plus `gmin() * vd`, no series resistance, `cj0 = 0` | ngspice shunts every junction with `gmin`: at −1 V it reports 1.01e-12 A, which is `gmin·1 V + is`; its reverse-bias cubic differs from this by less than 1e-18 A there | y |
| Physical constants | the prelude's exact SI values; ngspice's differ in the sixth digit of `kT/q` | the effect on the diode current at 0.8 V is about 2e-5 relative, under the contract's 1e-3 | y |
| ngspice is absent (Q2) | the parity tests fail, naming the binary they looked for; `PIPERINE_NGSPICE=skip` turns each into a pass that prints `SKIP` | the old harness passed silently without ngspice, which made the parity claim unfalsifiable on such a machine | n |
| The contract's values (Q3) | voltages `1e-3` relative, `1e-6` V absolute; currents `1e-3`, `1e-12` A; AC magnitude `1e-3`, `1e-12`; AC phase `1e-4` rad absolute, compared where the magnitude clears its floor; transient as voltages | the old contract's values (`16bddaae^:tests/support/ngspice.rs`), except the current floor: it said "ngspice's abstol for currents" and wrote `1e-9`; ngspice's `abstol` is `1e-12` | n |
| A sixth twin for the current source (Q7) | `isrc_r.cir` and `isrc_r.msl`: a 1 mA source into 1 kΩ, operating point | `Isource` is in M1 by S1 and no existing fixture places one | n |
| Signs | the flow of a `Vsource`'s branch is compared with ngspice's `i(v1)` unchanged | both are the current from `+` to `−` through the source: ngspice gives −4.59e-2 A at +0.8 V | y |

**Open questions:** Q2, Q3, Q4 and Q7 of `.specs/features/e2e-m1/questions.md`. The criteria below are written with the recommended answers.

**Implicit-requirement dimensions.** Input validation and bounds: PAR-03 AC 4 (an export row that does not parse), PAR-06 AC 5 (grids that differ). Failure states: PAR-03 AC 3, PAR-04. Idempotency: PAR-03 AC 2 (a scratch directory per run). Auth and rate limits: N/A because everything is local. Concurrency and ordering: PAR-03 AC 2 (tests run in parallel). Data lifecycle: PAR-03 AC 2 (the scratch directory is removed). Observability: PAR-05 AC 3. External-dependency failure: PAR-03 AC 3, PAR-04. State-transition integrity: N/A because a run keeps no state.

---

## User Stories

### P1: The twins ⭐ MVP

**User Story**: As a maintainer, I want each fixture as an MSL file beside its SPICE twin so that the pair is read and reviewed together.

**Why P1**: S9; the corpus is the claim.

**Requirement**: PAR-01

**Acceptance Criteria**:

1. The system SHALL hold `tests/ngspice/devices.msl` with the modules `Resistor`, `Capacitor`, `Vsource`, `Isource` and `Diode`, and one `<name>.msl` beside `<name>.cir` for `divider`, `diode_iv`, `diode_series`, `ac_rc` and `tran_rc`.
2. WHEN a twin is built by `Design::builder().root_file(..)` THEN the system SHALL give a `Design` with no diagnostic.
3. The system SHALL name each node of a twin's top module as its `.cir` names it, so that `v(mid)` and `<Top>.mid` are the same quantity.
4. The system SHALL leave every `.cir` file of `tests/ngspice/` byte-identical.

**Independent Test**: build each twin and assert zero diagnostics; `git diff --stat tests/ngspice/*.cir` is empty.

---

### P1: From a twin to a circuit ⭐ MVP

**User Story**: As a test author, I want one call that takes a twin's name and gives a bound circuit so that a test is the comparison and nothing else.

**Why P1**: the end-to-end path is the thing under test.

**Requirement**: PAR-02

**Acceptance Criteria**:

1. WHEN `Bench::load` is given a twin's name THEN the system SHALL build the `Design` from `tests/ngspice/<name>.msl`, elaborate its one module that has no port, interpret the `Ir` and return the bound `CircuitInstance`.
2. IF any stage returns diagnostics or an error THEN the system SHALL fail with the stage's name and every diagnostic rendered with its code and span.
3. The system SHALL give access to a node's value by the name the `.cir` uses and to a source's branch flow by the instance's name.

**Independent Test**: `Bench::load("divider")`, run `dc()`, read `mid`: 7.5 V. No ngspice involved.

---

### P1: Running ngspice ⭐ MVP

**User Story**: As a test author, I want the `.cir` run and its output parsed strictly so that the golden side is as trustworthy as the binary.

**Why P1**: the reference.

**Requirement**: PAR-03

**Acceptance Criteria**:

1. WHEN a fixture is run THEN the system SHALL execute `ngspice -b <absolute path of the .cir>` with a new, empty scratch directory as its working directory.
2. The system SHALL use a different scratch directory for every run and SHALL remove it when the run's result is dropped.
3. IF ngspice exits with a non-zero status, or an expected export or printed value is missing, THEN the system SHALL fail with the fixture's name and ngspice's captured output.
4. IF a row of a `wrdata` export or a printed `name = value` line does not parse as numbers THEN the system SHALL fail naming the file and the row.
5. The system SHALL read a `wrdata` export under the name the `.cir` gives it, with or without the suffix `.data`.
6. WHEN a comparison fails THEN the system SHALL include the first line of `ngspice --version` in the message.

**Independent Test**: run `divider.cir` and read `v(mid) = 7.5`; feed the parser a row `1.0 abc` and read an error naming the row.

---

### P1: ngspice is absent ⭐ MVP

**User Story**: As a maintainer, I want a missing ngspice to be visible so that a green run always means the comparison happened.

**Why P1**: an unfalsifiable parity claim is the failure this feature exists to prevent. The policy is Q2.

**Requirement**: PAR-04

**Acceptance Criteria**:

1. The system SHALL look for the binary named by the environment variable `PIPERINE_NGSPICE` when it is set to a path, else for `ngspice` on `PATH`.
2. IF no binary is found THEN each parity test SHALL fail with a message that names what it looked for and the opt-out.
3. WHERE `PIPERINE_NGSPICE` is set to `skip` each parity test SHALL pass without running ngspice and SHALL print one line `SKIP <fixture>: ngspice comparison disabled by PIPERINE_NGSPICE=skip`.
4. WHERE `PIPERINE_NGSPICE` is set to `skip` the system SHALL still build, elaborate and solve the MSL side of each twin, and SHALL fail when that side fails.

**Independent Test**: run one parity test with `PATH` emptied and read the failure; run it with `PIPERINE_NGSPICE=skip` and read the `SKIP` line.

---

### P1: One tolerance contract ⭐ MVP

**User Story**: As a reviewer, I want every bound of the comparison in one table with its reason so that nobody widens a tolerance inside a test to make it pass.

**Why P1**: the pattern the old harness established and S9 keeps.

**Requirement**: PAR-05

**Acceptance Criteria**:

1. The system SHALL define the contracts `DC_VOLTAGE`, `DC_CURRENT`, `AC_MAGNITUDE`, `AC_PHASE` and `TRAN_VOLTAGE`, each with a relative bound, an absolute bound and a rationale text.
2. The system SHALL accept a pair when `|golden − actual| <= abstol + reltol · max(|golden|, |actual|)` and reject it otherwise.
3. WHEN a pair is rejected THEN the system SHALL fail with the fixture, the quantity, the sweep value, time or frequency of the point, both values, the bound and the contract's rationale.
4. The system SHALL compare a phase only at points where the golden magnitude is above the absolute bound of `AC_MAGNITUDE`.
5. The system SHALL hold no numeric tolerance outside the contract's definition: no parity test target compares a difference with a literal.

**Independent Test**: unit tests of `holds` at, just inside and just outside each bound; the guard of PAR-07 scans the test targets.

---

### P1: One test per fixture ⭐ MVP

**User Story**: As a maintainer, I want a named test per fixture so that a regression says which circuit broke.

**Why P1**: S9; this is how M1 is proven done.

**Requirement**: PAR-06

**Acceptance Criteria**:

1. WHEN the test `divider` runs THEN the system SHALL compare `v(vin)` and `v(mid)` of a `Dc` solve with ngspice's operating point under `DC_VOLTAGE`.
2. WHEN the test `diode_series` runs THEN the system SHALL compare `v(a)` and `v(b)` of a `Dc` solve with ngspice's operating point under `DC_VOLTAGE`.
3. WHEN the test `diode_iv` runs THEN the system SHALL set `v1.dc` to each of the 37 sweep values of the export, solve `Dc` at each, and compare the flow of `v1` with ngspice's `i(v1)` under `DC_CURRENT`.
4. WHEN the test `ac_rc` runs THEN the system SHALL run an `Ac` sweep over the 41 frequencies of the export and compare the magnitude of `v(out)` under `AC_MAGNITUDE` and its phase under `AC_PHASE`.
5. IF the solver's frequency or sweep grid differs from the export's by more than 1e-9 relative at any point THEN the system SHALL fail naming the point.
6. WHEN the test `tran_rc` runs THEN the system SHALL run a transient to 10 µs, interpolate `v(out)` linearly at each of the 100 times of the export, and compare under `TRAN_VOLTAGE`.
7. The system SHALL run the transient with a maximum step of one tenth of the export's time step, so that the interpolation error stays below a tenth of the contract's bound.

**Independent Test**: `cargo test -p piperine-parity` with ngspice installed: five tests green.

---

### P1: The corpus cannot drift ⭐ MVP

**User Story**: As a maintainer, I want a guard over the corpus so that a twin without a test, or a tolerance outside the contract, fails the suite.

**Why P1**: the old suite's `ngspice_parity_matrix` existed for this reason.

**Requirement**: PAR-07

**Acceptance Criteria**:

1. IF a `<name>.msl` under `tests/ngspice/` other than `devices.msl` has no `<name>.cir`, or no test function named `<name>` in a parity target, THEN the guard SHALL fail naming the file.
2. IF a parity test target contains a comparison of an absolute difference with a numeric literal THEN the guard SHALL fail naming the file and the line.
3. WHEN the guard's two checks are given text written in the test that breaks each rule THEN each SHALL report it.

**Independent Test**: the guard is green on the tree and red on the planted text.

---

### P2: The current source has a twin

**User Story**: As a maintainer, I want `Isource` compared with ngspice so that every M1 device is covered by a twin.

**Why P2**: S1 lists current sources; the five named fixtures place none. Adding a fixture is Q7.

**Requirement**: PAR-08

**Acceptance Criteria**:

1. The system SHALL hold `tests/ngspice/isrc_r.cir` (a 1 mA current source into 1 kΩ, operating point, `print v(a)`) and its twin `isrc_r.msl`.
2. WHEN the test `isrc_r` runs THEN the system SHALL compare `v(a)` with ngspice's under `DC_VOLTAGE`.

**Independent Test**: the test `isrc_r` is green and both sides read 1 V in magnitude with the same sign.

---

## Edge Cases

- WHEN the golden value and the actual value are both exactly 0 THEN the system SHALL accept the pair.
- IF an export has a number of rows different from the one the test expects (37, 41, 100) THEN the system SHALL fail naming the fixture and both counts.
- IF a time of the export lies outside the solver's transient result THEN the system SHALL fail naming the time; it SHALL not extrapolate.
- WHEN two parity tests run at the same time THEN each SHALL use its own scratch directory.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PAR-01 | P1: The twins | Design | Pending |
| PAR-02 | P1: From a twin to a circuit | Design | Pending |
| PAR-03 | P1: Running ngspice | Design | Pending |
| PAR-04 | P1: ngspice is absent | Design | Pending |
| PAR-05 | P1: One tolerance contract | Design | Pending |
| PAR-06 | P1: One test per fixture | Design | Pending |
| PAR-07 | P1: The corpus cannot drift | Design | Pending |
| PAR-08 | P2: The current source has a twin | Design | Pending |

**Coverage:** 8 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` S1, S9 - the slice and this feature.
- `tests/ngspice/{divider,diode_iv,diode_series,ac_rc,tran_rc}.cir` - the golden side, unchanged.
- `git show 16bddaae^:tests/support/ngspice.rs` - the old harness: the `ToleranceContract` pattern, its values, the scratch directory per run.
- `.specs/features/e2e-m1/models/*.msl` - the twins as drafted and checked against today's front end.
- `docs/spec/` §16 (H1, H6), §18, Appendix C.2.1 - held nodes, limiting, the solver's own defaults.
- ngspice-47 run of the five fixtures on 2026-11-15 - export names, row counts, signs.

---

## Success Criteria

- [ ] `cargo test -p piperine-parity` is green with ngspice installed: five fixture tests, the guard, the harness's unit tests.
- [ ] With ngspice removed from `PATH`, the five fixture tests are red and say why.
- [ ] No `.cir` file changed.

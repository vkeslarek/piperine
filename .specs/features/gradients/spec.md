# Gradients Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

**Size:** L, about 40 tasks (at the split limit: if design finds more, the event rules GRD-07 to GRD-10 become a feature of their own). Part of the host-iii group (`.specs/features/host-iii/spec.md` holds the group's "Gaps closed" table).

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does the sensitivity of a real **carrier** (§23.3 rule 4: `s_q` held with the value and read by every reader) travel? The device ABI (MD-61) carries values and first partials against unknowns and params, nothing per carrier per param | (a) an ABI extension: each real net carries a sensitivity lane per gradient param, and the generated code of a reaction computes `∂x/∂p` and `ẋ` of what it commits (`.specs/features/device-abi/abi.md` amended, MD-61 amended); (b) no ABI change: analytic sensitivities for the analog part only, and every event-path effect by whole-run finite differences (two runs per param), `E0807`/`E0808` detected by comparing the event lists of the perturbed runs; (c) (b) now, (a) later | **(a)**. §23.3's rules 2 to 4 are exact; finite differences cannot tell a grazing crossing from a steep one (`E0807`) and cost two full runs per param. (b) remains the test oracle in every case. With (b), GRD-09 is rewritten as finite differences and GRD-10's detection becomes a comparison |
| Q2 | Forward or adjoint sensitivities along a transient? | (a) forward: one sensitivity system per gradient param, solved with the step's own factored matrix, cost proportional to the number of params, works with MD-66's last-step memory; (b) adjoint: one backward pass per record field, cost proportional to the number of outputs, needs the whole trajectory kept (against MD-66); (c) forward for transients, the existing adjoint (`analyses/adjoint.rs`) for an operating point | **(c)**. Forward fits events (saltation is a forward jump) and the memory rule of `long-run`; the DC adjoint already exists and is the cheap case for many params |
| Q3 | Which quantities get a gradient, and where does a user ask? | (a) a record's real fields only (§23.3's definition), from `piperine-api` after a run; (b) (a) plus the printed analog signals per row, as columns `d(<signal>)/d(<param>)`; (c) (b) plus a run key `gradients = [param paths]` and its flag, printing both | **(c)**. Records are the spec's gradient; per-row sensitivities are its "sensitivity" (a quantity of the trajectory) and are what a plot needs; the key follows MD-65 item 2 |

## Problem Statement

§21 rule 7 defines gradient params and §23.3 defines the gradient of a record across events: between events the solver's sensitivities, a located event that moves with the params (saltation), scheduled times that move, and commits of real values that carry a sensitivity, with `E0807` to `E0809` where no gradient exists. None of it is reached from MSL (`GAPS.md` H8). The solver has a DC adjoint (`piperine-solver/src/analyses/adjoint.rs`) and a finite-difference `.sens` (`sens.rs`, run by `host-ii`), both at an operating point, with `∂F/∂p` from the device ABI's partials or by differencing (`sensitivity.rs`). Nothing differentiates a transient, an event time or a record.

## Goals

- [ ] Elaboration finds the gradient params and the host lists them, each with why it has none when it has none.
- [ ] After `op`, `dc` and `transient`, a host reads `∂(record)/∂p` for every real field of every record and every gradient param.
- [ ] Every gradient agrees with a central finite difference of whole runs under a new gradient constant of `ToleranceContract`.
- [ ] A grazing crossing, an event that appears or disappears, and a param with no gradient give `E0807`, `E0808` and `E0809` instead of a number.
- [ ] Computing gradients leaves the run's values, event times and records bit-identical.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The periodic small-signal and periodic noise response (§23.3 rule 4h) | `periodic-small-signal` (V16); it reuses rule 4 with a phasor in place of `dp` |
| Gradient-based optimization | `sweeps-montecarlo-optimization` (its Q4 (c)); this feature gives the gradients |
| Gradients of an `ac`, `noise`, `sp`, `tf` or `pz` result | Not in §23.3, whose gradient is of a record |
| Gradients on the interpreter | Frozen at the M1 subset (R1); refused naming `--backend rust` |
| Gradients through an `extern "osdi"` model | `foreign-models`; refused naming the instance with `E0809` until then |
| Gradients of a composite record field other than its float leaves | §23.3's resolution: each float leaf carries its own; `composite-values` gives the composites |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Carrier sensitivities | ABI lanes (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| Transient method | forward; DC adjoint at an operating point (Q2 (c)) | Q2 recommendation | n - open (Q2) |
| Quantities and surface | records, per-row signals, run key (Q3 (c)) | Q3 recommendation | n - open (Q3) |
| The oracle | central finite difference of whole runs at `p(1 ± 1e-6)`, compared under a new `ToleranceContract::GRADIENT` with its rationale; for an operating point also ngspice's `.sens` | MD-38; the test never reads the implementation's derivative code | n - planner |
| Excluded params | a session option `exclude_gradients = [paths]` (§23's table), with its flag | §23 lists it as a session option | n - planner |
| Gradient params when none is asked | none computed: a run with no `gradients` costs what it costs today | the trajectory rule and MD-66 | n - planner |
| Grazing threshold | `de/dt` at `t*` smaller in magnitude than the crossing's own `ttol`-scaled slope bound | a crossing whose time cannot be located within `ttol` has no defined `dt*/dp` | n - planner |
| Which record a diagnostic replaces | every record whose value depends, through the dependency analysis of §21, on the named event | §23.3: "`E0807` and `E0808` replace the number for the records that depend on the named event" | y - spec |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: GRD-03, GRD-10 AC 3. Failure and partial-failure states: GRD-10 (no gradient for some records, numbers for the rest). Idempotency and retry: GRD-11 (the run with and without gradients is the same). Auth and rate limits: N/A, a local tool. Concurrency and ordering: GRD-07 AC 3 (events at one instant in microstep order). Data lifecycle: GRD-12 AC 3 (sensitivities kept as rows, not steps, MD-66). Observability: GRD-02 (why a param has no gradient). External-dependency failure: GRD-13 AC 3 (ngspice absent). State-transition integrity: GRD-11.

---

## User Stories

### P1: Which params have a gradient ⭐ MVP

**User Story**: As a host author, I want to know which params have a gradient and why the others do not, so that I ask only for what exists.

**Why P1**: §21 rules 7 and 10, §23.3 rule 6.

**Acceptance Criteria**:

1. The system SHALL mark as a gradient param every `f64` param that reaches a contribution, a probe-reading expression or an event time, found by dependency analysis at elaboration (GRD-01)
2. The system SHALL mark as having no gradient every param that only selects (`match`, `if`, generation) or only feeds discrete values that are not real (GRD-01)
3. WHEN a host asks a session for its metadata THEN each param SHALL carry its gradient flag and, when it has none, the reason (selects, discrete only, structural, not `f64`) (GRD-02)
4. WHEN `piperine info` runs THEN its `params` table SHALL have a column `gradient` (GRD-02)
5. WHEN a run gives `exclude_gradients` THEN the system SHALL treat those params as having no gradient for that session (GRD-03)
6. IF an `exclude_gradients` entry names no param THEN the system SHALL report `E0801` naming it (GRD-03)

**Independent Test**: `piperine info` on a module with a `where`-only param, a selecting param and a resistance lists one gradient param and two reasons.

---

### P1: Gradients at an operating point ⭐ MVP

**User Story**: As a designer, I want `∂(record)/∂p` and `∂v/∂p` at an operating point for every gradient param, so that I size a bias network.

**Why P1**: the adjoint exists; this connects it.

**Acceptance Criteria**:

1. WHEN an `op` or `dc` run gives `gradients` THEN the system SHALL give, per printed signal and per real field of every record emitted at the operating point, its derivative with respect to each named param, at each sweep point (GRD-04)
2. WHEN more than one param is named at an operating point THEN the system SHALL compute the derivatives of one output by one transpose solve, whatever the number of params (GRD-04)
3. WHEN a record's field is an expression of probes and params THEN its derivative SHALL be the chain rule through that expression, from the derivatives of the probes and the param's own partial (GRD-05)
4. WHEN the twin `sens_divider` runs `op` with `gradients` of both resistances THEN each derivative SHALL agree with ngspice's `.sens` under `host-ii`'s sensitivity constant (GRD-13)

**Independent Test**: a divider's output record `ratio` differentiated with respect to `r1.r` equals `-r2 / (r1 + r2)^2`.

---

### P1: Gradients along a transient ⭐ MVP

**User Story**: As a designer of a switching circuit, I want the derivative of a record emitted at the end of a transient with respect to my params, across the events of the run.

**Why P1**: H8's core: §23.3 rules 1 to 4.

**Acceptance Criteria**:

1. WHEN a `transient` run gives `gradients` THEN the system SHALL integrate the sensitivity of every analog unknown with respect to each named param along the run, with the run's own steps and integration method (GRD-06)
2. WHEN a `cross` of `e` is located at `t*` THEN the system SHALL give `dt*/dp = -(∂e/∂p) / (de/dt)`, with `∂e/∂p` including the state's own sensitivity (GRD-07)
3. WHEN the right-hand side changes across a located event THEN the system SHALL add the saltation correction (the change of the right-hand side times `dt*/dp`) to the state's sensitivity at `t*` (GRD-07)
4. WHEN several events occur at one instant THEN the system SHALL apply their corrections in microstep order (GRD-07)
5. WHEN an `at` or `after` time is computed from params THEN its `dt/dp` SHALL be the derivative of the expression that gives it (GRD-08)
6. WHEN a reaction commits a real value `x` to a carrier `q` THEN `q` SHALL hold `s_q = ∂x/∂p + ẋ · dt*/dp` until its next commit, and every reader SHALL differentiate through `q` with `s_q` (GRD-09)
7. WHEN a reaction commits a value that is not real THEN it SHALL set no sensitivity, and its effect on a record SHALL be the motion of the event times it causes (GRD-09)
8. WHEN `q.known_or(d)` is read THEN its sensitivity SHALL be `s_q` where `q` is known and the sensitivity of `d` where it is not (GRD-09)
9. WHEN the twin `cmp_edge` (the `Comparator` of §13 driven by a ramp, threshold `vth`) runs THEN the edge time's derivative SHALL equal `1 / v'(inp)(t*)` within `ToleranceContract::GRADIENT` (GRD-13)
10. WHEN a record of a run that crosses events is differentiated THEN each gradient SHALL agree with the central finite difference of whole runs under `ToleranceContract::GRADIENT` (GRD-13)
11. WHEN a buck with its closed loop runs 50 µs with `gradients` of the output capacitance THEN the gradient of a record of the output voltage at 50 µs SHALL agree with the finite-difference oracle under `ToleranceContract::GRADIENT` (GRD-13)

**Independent Test**: an RC charged until a `cross` sets a `Bit<f64>` sample; the sample's gradient with respect to `r` matches the finite difference.

---

### P1: No gradient, named ⭐ MVP

**User Story**: As a host author, I want a diagnostic naming the event when a gradient does not exist, so that I never optimize on a wrong number.

**Why P1**: §23.3 rule 5; `E0807` to `E0809`.

**Acceptance Criteria**:

1. IF a `cross` is located with `de/dt` below the grazing threshold at `t*` THEN the system SHALL give `E0807` naming the `cross` instance and `t*` in place of the gradient of every record that depends on it (GRD-10)
2. IF an event appears or disappears between the run and the run with a param moved by the oracle's step THEN the system SHALL give `E0808` naming the event in place of the gradient of every record that depends on it (GRD-10)
3. IF a gradient is requested with respect to a param that has none, or that the session excludes THEN the system SHALL refuse with `E0809` naming the param and the reason of GRD-02 (GRD-10)
4. WHEN some records depend on a named event and others do not THEN the system SHALL give numbers for the others (GRD-10)

**Independent Test**: a ramp that just touches a threshold gives `E0807` naming the crossing; a param feeding only a `match` gives `E0809`.

---

### P1: The trajectory does not move ⭐ MVP

**User Story**: As the owner of every result, I want gradients to be read-only.

**Why P1**: §23.3's dynamic semantics.

**Acceptance Criteria**:

1. The system SHALL give, with `gradients` and without, the same accepted times, values, event times and records, bit for bit (GRD-11)
2. The system SHALL compute nothing for gradients in a run that names none (GRD-11)

**Independent Test**: the buck's 50 µs run with and without `gradients` prints the same bytes.

---

### P2: Reading gradients

**User Story**: As a host author, I want gradients from the library and from a run key.

**Why P2**: Q3 (c).

**Acceptance Criteria**:

1. WHEN a session has run with `gradients` THEN `piperine-api` SHALL give, per record, per real field and per param, the derivative or the diagnostic that replaces it (GRD-12)
2. WHEN a run with `gradients` completes on the command line THEN the system SHALL print, after the table, one gradient table with the columns `record`, `field`, `param` and `value`, a diagnostic printed in place of `value` (GRD-12)
3. WHEN a transient with `gradients` prints signals THEN the system SHALL add the columns `d(<signal>)/d(<param>)` on the run's rows, keeping rows and never steps (MD-66) (GRD-12)
4. IF `gradients` is given on the interpreter THEN the system SHALL report `P0010` naming `--backend rust` (GRD-12)

**Independent Test**: `piperine run cmp_edge --gradients cmp.vth` prints the edge record's derivative.

---

### P3: The documents

**User Story**: As a reader of the Book and `GAPS.md`, I want gradients shown.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book SHALL show one gradient run of the `Comparator` example of §23.3, a listing held to the binary (GRD-14)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list H8, and `E0807` to `E0809` SHALL no longer be in its list of codes no source mentions (GRD-14)

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN a param reaches only an `after` delay THEN it SHALL be a gradient param (an event time) and its gradient SHALL be carried by GRD-08.
- WHEN a commit is a settle commit at a time point chosen by step control THEN its `dt*/dp` SHALL be 0 (§23.3 rule 4e).
- WHEN a delayed commit `q <- x after d` is made THEN its sensitivity SHALL be taken at the instant it was made and its due time SHALL move by `dt*/dp + ∂d/∂p` (rule 4d).
- IF a record has an unknown real field THEN its gradient SHALL be unknown, not 0 (rule 4g).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| GRD-01 | P1: Which params have a gradient | - | Pending |
| GRD-02 | P1: Which params have a gradient | - | Pending |
| GRD-03 | P1: Which params have a gradient | - | Pending |
| GRD-04 | P1: Gradients at an operating point | - | Pending |
| GRD-05 | P1: Gradients at an operating point | - | Pending |
| GRD-06 | P1: Gradients along a transient | - | Pending |
| GRD-07 | P1: Gradients along a transient | - | Pending |
| GRD-08 | P1: Gradients along a transient | - | Pending |
| GRD-09 | P1: Gradients along a transient | - | Pending |
| GRD-10 | P1: No gradient, named | - | Pending |
| GRD-11 | P1: The trajectory does not move | - | Pending |
| GRD-12 | P2: Reading gradients | - | Pending |
| GRD-13 | P1: Gradients (oracles) | - | Pending |
| GRD-14 | P3: The documents | - | Pending |

**Coverage:** 14 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| H8 | gradient params | GRD-01 to GRD-03 |
| H8 | `∂(record)/∂p` | GRD-04 to GRD-06, GRD-12 |
| H8 | saltation | GRD-07 to GRD-09 |
| H8 | `E0807` to `E0809` | GRD-10 |
| H8 | "the solver's `adjoint` and `sens` are not reached from MSL" | GRD-04 (adjoint), GRD-13 AC (sens parity); `sens` itself is `host-ii` HII-06 |
| §10 reference list | `E0807`, `E0808`, `E0809` mentioned by no source | GRD-10, GRD-14 |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `host-ii` (in flight) | metadata (HII-10), `piperine info`, `sens` and its parity constant |
| `piperine-transpile`, `piperine-plan` | generated derivatives of record fields, reaction commits and event expressions (lane A's area) |
| the device ABI (MD-61) | Q1 (a): sensitivity lanes on real nets; an ABI amendment recorded in `abi.md` |
| `solver-hygiene` (in flight) | exact steps (the oracle runs the same steps) |

## Sources

- `GAPS.md` H8 and §10's list of codes; the coverage map of 2026-11-17.
- `docs/spec/part_viii_host.md` §23.2 (gradients row), §23.3 (rules 1 to 6, rule 4 in detail, `E0807` to `E0809`, the `Comparator` example); `docs/spec/part_vi_io.md` §21 rules 7 and 10.
- `crates/piperine-solver/src/analyses/{adjoint,sens,sensitivity}.rs`.
- `.specs/STATE.md` MD-38, MD-61, MD-66; `.specs/features/dv-gradients/spec.md` (the PHDL-era plan, not carried over).

---

## Success Criteria

- [ ] Every gradient of the oracle targets agrees with finite differences under the contract.
- [ ] The three diagnostics fire on their probes.
- [ ] No run without `gradients` changes a byte.

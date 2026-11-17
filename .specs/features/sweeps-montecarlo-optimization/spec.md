# Sweeps, Monte Carlo and Optimization Specification

**Size:** L, about 34 tasks. Part of the host-iii group (`.specs/features/host-iii/spec.md` holds the group's "Gaps closed" table).

## Open questions (user decisions, asked 2026-11-17)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | Where do multi-axis sweeps, Monte Carlo and optimization live? MD-65 item 3 puts them "in the host", which is both the library and the command line | (a) `piperine-api` only (Rust host code; Python later); (b) `piperine-api`, plus run keys of `[run.<name>]` with their flags (`sweeps`, `mc`, `vary`, `optimize`), so `piperine run` and `piperine test` run them; (c) (a) plus new commands `piperine mc` and `piperine optimize` | **(b)**. MD-65 item 2 already says every run option is a key with its flag; a regression over 200 draws is a run a project keeps. The library function is the rule, the key its shell. With (a), SMO-02, SMO-06 and SMO-11 lose their key criteria |
| Q2 | A **structural** param (§8.1.2) in a sweep, a Monte Carlo draw or an optimization: MD-18 says a sweep never re-elaborates; §23.1 rule 7 and §23.2 rule 6 say varying a structural param re-elaborates | (a) refused: only non-structural params vary, each point a `set` on the compiled design; (b) allowed: each distinct structural value is elaborated and compiled once, cached by its hash (the transpile cache already keys by hash), and the non-structural params of the point are then `set`; MD-18 is amended to say "non-structural" | **(b)**. The spec allows it and a ladder with `N` sections swept over `N` is a real use; MD-18's intent (never re-elaborate per point for a value change) stays, since one structure compiles once. With (a), SMO-03 AC 2 and SMO-05 AC 6 become refusals naming the param |
| Q3 | How are several objectives combined? §23.2 says each `target` the host picks as an objective is maximized | (a) the weighted sum, weights given per objective (default 1); (b) maximize the smallest objective margin (worst case), so every objective improves together; (c) lexicographic, in the order given | **(b)**. A margin is already normalized by its author (positive is good), so the minimum is meaningful without weights and has no tuning; (a) is offered as an option `combine = "sum"` |
| Q4 | Which optimizer runs first? | (a) derivative-free (bounded Nelder-Mead with constraint handling by penalty), which works on every analysis now; (b) gradient-based only (sequential quadratic programming over the gradients of `gradients`), so it waits for that feature; (c) (a) now, and (b) selected by `method = "gradient"` once `gradients` lands | **(c)**. Optimization is usable before gradients exist, and the gradient method is a later option, not a rewrite |

## Problem Statement

A run sweeps one param of a `dc` analysis (`sweep`, `from`, `to`, `step`) and nothing else. `GAPS.md` H2 lists "more than one sweep axis", which `host-ii` moved out, and H7 lists Monte Carlo, optimization, objectives and constraints from `target`s and bounds from `where` (§23.1 rule 5, §23.2). The language already states an optimization problem (params with `where`, `target` margins, `require`s); no host reads it. The user decided (X3) that a `target` only reports in `piperine test`, so optimization reads targets in the host.

## Goals

- [ ] Any analysis runs over a grid of params, outer axes first, one leading column per axis.
- [ ] A Monte Carlo run draws params by path from normal or uniform distributions, per instance or globally, reproducibly from a seed, and reports each draw's targets and requires and the yield.
- [ ] An optimization varies named params inside the bounds their `where`s give, maximizes the chosen targets, keeps the constraint targets at or above zero and the `require`s passing, and never simulates a candidate that fails a `where`.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Gradients (`∂(record)/∂p`) | `gradients`; Q4 (c) uses them when they exist |
| Measurements over a waveform (overshoot, settling) | MD-66 item 3: host code; an optimization reads `target` records, which the model computes |
| Correlated distributions, a covariance matrix, process corners from a foundry file | Not in §23; a later request |
| Worst-case distance and high-sigma methods | Not in §23; the old `dv-gradients` driver policies are not carried over |
| A distributed (cluster) Monte Carlo | Local threads only |
| A `target` failing `piperine test` | X3: a target only reports |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Surface | library plus run keys and flags (Q1 (b)) | Q1 recommendation | n - open (Q1) |
| Structural params | allowed, compiled once per distinct structure (Q2 (b)) | Q2 recommendation | n - open (Q2) |
| Several objectives | maximize the smallest margin (Q3 (b)); `combine = "sum"` optional | Q3 recommendation | n - open (Q3) |
| Optimizer | derivative-free now, gradient later (Q4 (c)) | Q4 recommendation | n - open (Q4) |
| The axis keys | `sweeps = [{ param, from, to, step }, { param, values = [..] }, { param, from, to, points, scale = "log" }]`, outermost first; the existing `dc` keys stay the innermost axis of a `dc` run | the `dc` keys keep working unchanged (CLI-03) | n - planner |
| Distributions | `normal` (`sigma` or `rel`, mean the param's value) and `uniform` (`lo`, `hi`, or `rel` around the value) | the spec's sketch (`gauss(rel=0.05)`); enough for mismatch and tolerance | n - planner |
| Per instance or global | a path to one instance's param draws for that instance; `<Module>::<param>` with `per = "instance"` draws once per instance of the module, with `per = "global"` once for all of them | §23.1 rule 5 | n - planner |
| Random generator | a counter-based generator seeded by `seed` and the draw index, so draw `k` does not depend on how many threads ran | reproducibility (§23.1 rule 12) under parallel draws | n - planner |
| A draw that fails a `where` | not simulated, reported with the `E0802` message, counted as a failed draw in the yield | §23.1 rule 6 refuses the `set`; a part outside its bounds is a failing part | n - planner |
| Draws in parallel | yes, on threads, each on its own session over the same compiled library; results ordered by draw index | no draw reads another | n - planner |
| Bounds from `where` | a `where` that is a conjunction of comparisons of the param with constants gives a box; any other `where` is a feasibility check evaluated before each candidate | §23.2 rule 5 | n - planner |
| A record with `maybe = true` | not counted as a pass, hit or margin; the draw or candidate is reported as unknown | §23.2 rule 4 | y - spec |

**Open questions:** Q1 to Q4 at the top of this file, awaiting the user.

**Implicit-requirement dimensions.** Input validation and bounds: SMO-02, SMO-05, SMO-11. Failure and partial-failure states: SMO-07 AC 3 (one draw fails, the others go on), SMO-12 AC 4 (no feasible start). Idempotency and retry: SMO-06 AC 1 (same seed, same results). Auth and rate limits: N/A, a local tool. Concurrency and ordering: SMO-07 AC 2 (parallel draws, ordered results). Data lifecycle: SMO-08 (what each draw keeps: last target record, require verdicts). Observability: SMO-08, SMO-13 (each iteration reported). External-dependency failure: N/A, no external service. State-transition integrity: SMO-12 AC 5 (a candidate failing `where` is never simulated).

---

## User Stories

### P1: Sweeps over several axes ⭐ MVP

**User Story**: As a designer, I want an AC or transient run repeated over a grid of params, so that I see a family of curves in one table.

**Why P1**: H2's "more than one sweep axis".

**Acceptance Criteria**:

1. WHEN a run of any analysis gives `sweeps` THEN the system SHALL run the analysis at every point of the product of the axes, outermost axis first, and give one table with one leading column per axis, named by the param's path, before the analysis's own columns (SMO-01)
2. WHEN a `dc` run gives `sweeps` and its own `sweep` keys THEN the system SHALL make the `dc` sweep the innermost axis (SMO-01)
3. WHEN an axis gives `values` THEN the system SHALL take them in the order given (SMO-01)
4. WHEN an axis gives `points` and `scale = "log"` THEN the system SHALL take `points` values evenly spaced in the logarithm from `from` to `to`, both included (SMO-01)
5. The system SHALL give each axis of `sweeps` a repeatable flag `--sweeps <path>=<from>:<to>:<step>` or `--sweeps <path>=<v1>,<v2>,..`, a flag list replacing the key (SMO-02)
6. IF an axis names no param THEN the system SHALL report `E0801` naming it (SMO-02)
7. IF an axis has no point, a `step` that does not move toward `to`, or a `log` scale with a bound not greater than 0 THEN the system SHALL report `P0010` naming the axis (SMO-02)
8. WHEN a point's values fail a param's `where` THEN the system SHALL skip the point, print one record line naming the point and the `E0802` message, and go on (SMO-02)
9. The system SHALL change a non-structural param between points by a `set` on the compiled design, with no re-elaboration and no `rustc` (MD-18) (SMO-03)
10. WHEN an axis varies a structural param THEN the system SHALL elaborate and compile each distinct structure once, reuse it for every point that has it, and `set` the other params (Q2 (b)) (SMO-03)

**Independent Test**: `ac_rc` swept over `r1.r = 1e3, 2e3, 4e3` gives three families of rows whose corners agree with ngspice's `.step`-free reruns of the same circuit under the AC constant.

---

### P1: Monte Carlo ⭐ MVP

**User Story**: As a designer, I want to draw param values from distributions and see each draw's targets and requires and the yield, so that I know how my design behaves across tolerances.

**Why P1**: H7; §23.1 rule 5.

**Acceptance Criteria**:

1. WHEN a run gives `mc = { draws = N, seed = s }` and `vary` THEN the system SHALL run the analysis `N` times, each with the params of `vary` drawn from their distributions, and give one row per draw (SMO-04)
2. WHEN a `vary` entry is `normal` with `rel = r` THEN the system SHALL draw the param from a normal distribution with the param's value as mean and `r` times its magnitude as standard deviation (SMO-04)
3. WHEN a `vary` entry is `uniform` with `lo` and `hi` THEN the system SHALL draw the param uniformly in `[lo, hi]` (SMO-04)
4. WHEN a `vary` entry names `<Module>::<param>` with `per = "global"` THEN every instance of that module SHALL receive the same drawn value in a draw (SMO-04)
5. WHEN a `vary` entry names `<Module>::<param>` with `per = "instance"` THEN each instance of that module SHALL receive its own draw (SMO-04)
6. IF a `vary` entry names no param, or a distribution other than `normal` and `uniform` THEN the system SHALL report `E0801` or `P0010` naming the entry (SMO-05)
7. IF `draws` is not an integer of at least 1, `sigma` or `rel` is negative, or `lo` is greater than `hi` THEN the system SHALL report `P0010` naming the key (SMO-05)
8. WHEN two runs give the same source, the same params, the same `seed` and the same `draws` THEN the system SHALL give the same drawn values and the same results, bit for bit, whatever the number of threads (SMO-06)
9. WHEN the draws run on several threads THEN the system SHALL give the rows in draw order (SMO-07)
10. IF one draw's analysis fails THEN the system SHALL report that draw as failed with the solver's message and run the other draws (SMO-07)
11. WHEN a draw fails a param's `where` THEN the system SHALL not simulate it and SHALL report it as failed with the `E0802` message (SMO-07)
12. WHEN a Monte Carlo run completes THEN each row SHALL give the draw index, every drawn value, each `target`'s last margin and each `require`'s verdict (pass, fail or unknown) (SMO-08)
13. WHEN a Monte Carlo run completes THEN the system SHALL give the yield: the share of draws in which every `require` passed and every constraint target is at or above zero, with no record `maybe = true` counted as a pass (SMO-08)
14. WHEN `piperine test` runs a Monte Carlo run THEN the system SHALL fail it when any draw has a failing `require` and print the yield, the targets only reporting (X3) (SMO-08)

**Independent Test**: a divider with two 1 % resistors, 1000 draws, gives an output standard deviation within 5 % of the closed form `sqrt(2) * 0.01 / 4` of the nominal.

---

### P1: Optimization ⭐ MVP

**User Story**: As a designer, I want the host to vary my params inside their `where` bounds to maximize my targets while keeping my constraints, so that sizing is a run, not a hand loop.

**Why P1**: H7; §23.2.

**Acceptance Criteria**:

1. WHEN a run gives `optimize = { vary = [paths], objectives = [target names], constraints = [target names] }` THEN the system SHALL search values of the `vary` params that maximize the objectives combined by Q3 while every constraint target's last margin is at or above zero and every `require` passes (SMO-09)
2. The system SHALL read each varied param's bounds from its `where`, and SHALL give a param with no box bound from its `where` no bound (SMO-10)
3. WHEN a varied param's `where` is not a box THEN the system SHALL evaluate it before each candidate and never simulate a candidate that fails it (§23.2 rule 5) (SMO-10)
4. WHEN the optimization ends THEN the system SHALL give the best feasible values, each objective's and constraint's margin there, the number of candidates simulated and the reason it stopped (converged, `max_iter`, no progress) (SMO-11)
5. IF a name of `objectives` or `constraints` is not a `target` the run emits THEN the system SHALL report `E0801` naming it (SMO-11)
6. IF a `vary` path names a param with no gradient while `method = "gradient"` THEN the system SHALL report `E0809` naming it (SMO-11)
7. IF no candidate tried is feasible THEN the system SHALL end with a failure naming the constraint with the most negative margin at the best candidate (SMO-12)
8. WHEN a candidate's run has a record with `maybe = true` on an objective or a constraint THEN the system SHALL treat that candidate as infeasible (SMO-12)
9. WHEN an optimization runs THEN the system SHALL print one record line per candidate with its values and margins (SMO-13)
10. WHEN the twin optimization of an RC low-pass maximizes a target `bw_margin` under a constraint `area_margin` THEN the system SHALL reach the closed-form optimum within 1 % (SMO-14)

**Independent Test**: the RC sizing run, `piperine run size_rc`, prints the closed-form `r` and `c` within 1 %.

---

### P3: Yield optimization

**User Story**: As a designer, I want to optimize the yield of a Monte Carlo run, so that I center a design (§23.2's "yield" row).

**Why P3**: the last row of §23.2's table; it composes SMO-04 and SMO-09.

**Acceptance Criteria**:

1. WHEN `optimize` gives `objective = "yield"` and the run gives `mc` THEN the system SHALL maximize the Monte Carlo yield of SMO-08 over the `vary` params, with the same seed at every candidate (SMO-15)

**Independent Test**: a comparator with an offset spec centers its threshold param to the middle of the window.

---

### P3: The documents

**User Story**: As a reader of the Book and `GAPS.md`, I want sweeps, Monte Carlo and optimization shown.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book SHALL show a two-axis sweep, a Monte Carlo run and an optimization, each a listing held to the binary (SMO-16)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list H7 nor H2's "more than one sweep axis", and MD-18 SHALL say what Q2 decided (SMO-16)

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN `sweeps` and `mc` are both given THEN the system SHALL run the Monte Carlo at every sweep point, the sweep outermost.
- WHEN a `vary` entry and `params` name the same param THEN the system SHALL draw around the value `params` gives.
- IF `optimize` and `mc` are given without `objective = "yield"` THEN the system SHALL report `P0010` naming the two keys.
- WHEN a normal draw lands outside a box bound THEN the system SHALL treat it as SMO-07 AC 11 says, not clip it.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SMO-01 | P1: Sweeps | - | Pending |
| SMO-02 | P1: Sweeps | - | Pending |
| SMO-03 | P1: Sweeps | - | Pending |
| SMO-04 | P1: Monte Carlo | - | Pending |
| SMO-05 | P1: Monte Carlo | - | Pending |
| SMO-06 | P1: Monte Carlo | - | Pending |
| SMO-07 | P1: Monte Carlo | - | Pending |
| SMO-08 | P1: Monte Carlo | - | Pending |
| SMO-09 | P1: Optimization | - | Pending |
| SMO-10 | P1: Optimization | - | Pending |
| SMO-11 | P1: Optimization | - | Pending |
| SMO-12 | P1: Optimization | - | Pending |
| SMO-13 | P1: Optimization | - | Pending |
| SMO-14 | P1: Optimization | - | Pending |
| SMO-15 | P3: Yield optimization | - | Pending |
| SMO-16 | P3: The documents | - | Pending |

**Coverage:** 16 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| H2 | more than one sweep axis | SMO-01 to SMO-03 |
| H7 | Monte Carlo | SMO-04 to SMO-08 |
| H7 | optimization | SMO-09, SMO-11 to SMO-14 |
| H7 | objectives and constraints from `target`s | SMO-09, SMO-12 |
| H7 | bounds from `where` | SMO-10 |
| H7 / §23.2 | yield | SMO-08 AC 13, SMO-15 |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `host-ii` (in flight) | `set` on the compiled design, derived params recomputed (X9, HII-18), metadata with `where` and the structural flag, `target` records kept (HII-16) |
| `msl-fixes` S5 (in flight) | a `where` that reads a later param is refused, so a `where` is a function of the params before it |
| `gradients` (spec) | `method = "gradient"` only (Q4 (c)) |

## Sources

- `GAPS.md` H2, H7; X3; the coverage map of 2026-11-17.
- `docs/spec/part_viii_host.md` §23 (session options), §23.1 rules 5 to 7 and 12, §23.2 rules 1 to 6.
- `.specs/STATE.md` MD-18, MD-65 (item 3), MD-66.
- `crates/piperine-api/src/request.rs` (the `dc` keys).

---

## Success Criteria

- [ ] The divider Monte Carlo matches its closed form; the RC sizing reaches its closed-form optimum.
- [ ] A swept run changes no existing run's output.

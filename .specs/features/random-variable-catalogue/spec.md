# Random-Variable Catalogue — Distribution Sampling Specification

## Problem Statement

Monte-Carlo / yield / centering are dark: PHDL's per-parameter `tol
gauss/uniform` clauses died with the value layer (MD-45), and the POM
still carries only the vestige (`Param::distribution`, written `None`
unconditionally). 17 blocked tests across `piperine-api`
(`monte_carlo_sampling`, `monte_carlo_report`, `monte_carlo_compile_once`,
`high_sigma_sampling`, `center_policy`) exercise precisely this surface.

The architectural direction is locked in
`constraints-on-vams/context.md`'s Deferred Ideas (user's call): uncertainty
becomes a **first-class catalogue of random variables on the POM**, not a
per-parameter `Option`, because the four hard facts are all
cross-parameter: sample-vector enumeration, correlation +
global-vs-mismatch scoping, reproducible replay by stable index, and
MD-18 compile-once (an N-sample run is N restamps of one compiled
circuit). Sampling re-anchors on the LRM §9.13.2 `$rdist_*` functions
piperine already implements — no new math.

## Goals

- [ ] A designer declares statistical spread on parameters
      (`tol gauss(sigma = …) [global]`, `tol uniform(…)`) and the design
      carries a typed, ordered random-variable catalogue.
- [ ] An N-sample run draws, applies through the ordinary restamp path
      (`Session::set`), and never recompiles (MD-18 — asserted).
- [ ] `global` scope: one draw shared across every instance of the
      module. Mismatch (default): independent draws per instance.
- [ ] A sample is reproducible from `(seed, index)` — stable catalogue
      order is the contract.
- [ ] Sampling math reuses the `$rdist_*` implementation — one PRNG
      shape, not a second convention.

## Out of Scope

| Feature | Reason |
|---|---|
| Correlation groups beyond global/mismatch (named covariances) | The catalogue's shape leaves the slot; the grammar and driver follow with demand. |
| Importance sampling / high-sigma estimators | `high_sigma_sampling`'s 3 tests stay red — they need the estimator machinery on top of the catalogue (named blocker). |
| Design-centering optimizer | P7; the catalogue is its joint, not its implementation. |
| `∂margin/∂(random var)` | dv-gradients follow-up. |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Declaration syntax | PHDL's clause restored verbatim: `parameter real r = 1e3 tol gauss(sigma = 10.0) global;` | The blocked tests author exactly this; the user's direction re-anchors the *sampling*, not the spelling. | y (fixtures) |
| Catalogue shape | `Design.random_vars: Vec<RvDecl>` in first-declaration order — `{module, param, dist, scope}` with a stable index | The recorded direction (enumeration, replay, compile-once). | y (context.md) |
| Draw application | Host driver: per sample, per RV → `Session::set(instance, param, value)` | The ordinary restamp path — compile-once for free, `monte_carlo_compile_once` asserts it. | y |
| Scope semantics | `global`: one value per RV per sample, applied to every instance; default (mismatch): one value per (RV × instance) | dv-core's original semantics, recorded in context.md. | y |
| PRNG | The `$rdist_normal`/`$rdist_uniform` implementation extracted to a public `piperine-lang` sampling helper; the driver consumes it | Re-anchoring on the implemented LRM functions — one convention. | y (direction) |
| `sigma_rel` | `gauss(sigma_rel = v)` → sigma = |nominal|·v, resolved at draw time against the instance's nominal | dv-core's relative-sigma form (fixtures use it). | y (fixtures) |

**Open questions:** none.

## User Stories

### P1: Declare and draw ⭐

1. `tol gauss(sigma = …)` / `tol gauss(sigma_rel = …)` / `tol uniform(lo_offset = …, hi_offset = …)` / `tol uniform(rel = …)` clauses parse on `parameter` declarations and land on the catalogue.
2. A driver draws N samples and applies each through `Session::set`; `rebuilds == 0` across the whole run.
3. Nominal analysis (no sampling) is untouched — the clauses are inert at solve (the `tol_inert` contract).

### P2: Scoping and replay

1. A `global` RV holds one value per sample across all instances of its module (measured: instances of the same module agree at every sample).
2. A mismatch RV draws independently per instance (measured: they differ).
3. Sample `k` from seed `s` replays byte-identically in a fresh session.

### P3: Reporting

1. The worst sample (min worst-margin) is named and replayable.
2. Yield = share of samples with every require margin ≥ 0.
3. `sigma_reports…` — the spread of a constraint's margin across samples.

## Edge Cases

- An RV on a param with `from` range: a drawn value outside the range is reported, not clamped silently (fail loud on apply? → **no**: sampling legitimately explores; range checks apply to *authored* values, draws report). Named decision.
- Two RVs on the same param: loud duplicate.
- An RV on a param of an uninstantiated module: draws exist, apply to nothing.

## Requirement Traceability

| ID | Story | Status |
|---|---|---|
| RVCAT-01 | P1: clauses parse → catalogue (stable order) | Verified |
| RVCAT-02 | P1: driver draws + applies via restamp, zero rebuilds | Verified |
| RVCAT-03 | P1: inert at nominal solve | Verified |
| RVCAT-04 | P2: global scope shared; mismatch independent | Verified |
| RVCAT-05 | P2: (seed, index) replay byte-identical | Verified |
| RVCAT-06 | P3: worst sample named + replayable; yield fraction; margin spread | Verified |
| RVCAT-07 | Edge: duplicate RV loud; out-of-range draw reported | Verified |
| RVCAT-08 | `tol_inert` ×5 + `monte_carlo_{sampling,report,compile_once}` + `center_policy` unblocked; `high_sigma_sampling` stays red with named blocker | Verified |
| RVCAT-09 | Docs: Part I parameters section + ROADMAP distributions bullet resolved | Verified |

## Success Criteria

- [ ] `Param::distribution` vestige either carries the declaration or is replaced by the catalogue — no dead dual surface.
- [ ] The 5 `tol_inert` tests green (bit-identical nominal + staged override + invalidation class).
- [ ] MD-18 asserted: 1000 samples cost zero additional compiles.

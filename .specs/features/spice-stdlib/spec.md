# spice-stdlib Specification

Feature: the ngspice-faithful PHDL device models become part of Piperine's
**builtin standard library** (`crates/piperine-lang/headers/spice/`), backed by
an in-repo ngspice cross-validation harness, with the known transistor
correctness debt fixed.

This is **phase 1** of the larger "SPICE completeness" program
(see memory `spice-stdlib-migration`): migrate → validate → fix. Waves A/B/C
(new devices: mos2/3/6/9, vbic, mes…, transmission lines, BSIM families) are
**separate future features** and out of scope here.

## Problem Statement

The SPICE models live in an external package (`~/Git/plugins/piperine-spice`,
with a stale fork at `~/Git/piperine-spice`), so a Piperine user cannot
`use spice::diode;` out of the box, the models drift between copies, and the
ngspice validation harness is not part of piperine's test suite. Additionally,
three known correctness gaps (MOS1 drain current ~1.5× high, JFET ~15 mV off,
BJT fails to reach saturation / current-mirror non-convergence) violate the
project rule that everything simulable in ngspice simulates **correctly** in
Piperine.

## Goals

- [ ] `use spice::<file>;` works in any Piperine project with no `Piperine.toml`
      dependency — models ship in `headers/spice/`.
- [ ] ngspice cross-validation harness runs as part of piperine's test suite
      (skips cleanly when `ngspice` binary is absent).
- [ ] All 8 existing validation circuits pass within tolerance
      (reltol 1e-3, abstol 1e-6 V), including the 4 currently failing.
- [ ] `cargo test --workspace` and `piperine test` on examples stay green,
      zero warnings.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Wave A devices (jfet2, mes/mesa, hfet1/2, mos2/3/6/9, vbic, vdmos) | Separate future feature |
| Wave B transmission lines (urc/tra/ltra/txl/cpl) | Needs solver features (delay history, convolution) |
| Wave C compact models (BSIM*, HiSIM*, soi3) | Separate future feature (user chose full PHDL route) |
| asrc (B-source), CIDER numerical devices, adms | Excluded by user decision 2026-07-16 |
| `piperine-spice` plugin crate (`@spice_model` attribute) | Plugin face stays external; only PHDL models migrate |
| ngspice netlist importer (`piperine spice`) | Future script-tier feature |
| Native `.dc` sweep analysis | Bench-loop staging + `$op` is sufficient for validation |
| Breakpoint-exact transient parity | DC/OP validation only in this phase |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Source copy | `~/Git/plugins/piperine-spice` | Newer (2026-07-11), `Real?` optional params, `pub` visibility, validation/ | y (user) |
| External repos afterwards | Retire; `headers/spice/` is single source of truth | User choice | y (user) |
| `spice` namespace registration | `add_namespace("spice", headers_dir/"spice")` next to the `piperine` namespace in `piperine-project/src/source_map.rs:57` (+ `SourceMap::dummy`) | Same mechanism as existing builtin headers | n (agent) |
| Harness location | `crates/piperine-bench/tests/ngspice_validation.rs` (Rust test driving ngspice + BenchRunner), circuits in `crates/piperine-bench/tests/ngspice/` | Rust test > python script for `cargo test` integration; bench crate already has e2e infra | n (agent) |
| ngspice absent (CI) | Test prints skip notice and passes | External binary can't be a hard dep | n (agent) |
| Tolerances | reltol 1e-3, abstol 1e-6 V (per-node: `|Δ| ≤ abstol + reltol·max(|a|,|b|)`) | ngspice's own vntol/reltol defaults; existing harness contract | n (agent) |
| CSV requirement (user rule 2) | OP-point compare via parsed stdout now; ngspice `wrdata` CSV export becomes the pattern for sweep-based tests introduced by the MOS1/JFET fixes | User asked for CSV validation; sweeps are where CSV pays off | n (agent) |
| BJT saturation fix location | Solver homotopy (source stepping completion), not model equations | Diagnosis 2026-07-12: model correct, convergence path wrong | n (agent) |
| Package name collision | A project whose `Piperine.toml` declares its own `spice` package/dependency shadows the builtin (project namespaces win) | Matches existing precedence: project name registered after builtins | n (agent) |

**Open questions:** none — all resolved or logged above.

## User Stories

### P1: Builtin `use spice::` ⭐ MVP

**User Story**: As a Piperine user, I want `use spice::diode;` to work in any
project without declaring a dependency, so SPICE-equivalent circuits need no setup.

**Acceptance Criteria**:

1. WHEN a `.phdl` file contains `use spice::sources;` / `use spice::passives;`
   / `use spice::diode;` (etc.) and is checked/run via CLI or LSP THEN the
   system SHALL resolve the module from `crates/piperine-lang/headers/spice/`.
2. WHEN the migrated models are elaborated THEN elaboration SHALL succeed for
   every file in `headers/spice/` (no parse/elab errors).
3. WHEN `piperine test` runs the migrated smoke tests (junction + validate,
   relocated in-repo) THEN all SHALL pass.
4. WHEN a project defines its own `spice` package THEN the project's package
   SHALL shadow the builtin.

**Independent Test**: fresh scratch dir, single `.phdl` with `use spice::diode;`
+ bench `$op`, `piperine test` passes with no `Piperine.toml`.

### P1: ngspice cross-validation in the test suite ⭐ MVP

**User Story**: As a maintainer, I want every model change guarded by a
piperine-vs-ngspice comparison inside `cargo test`, so correctness regressions
fail CI on my machine.

**Acceptance Criteria**:

1. WHEN `cargo test -p piperine-bench ngspice` runs with `ngspice` on PATH
   THEN each circuit pair (`.cir` golden vs `.phdl`) SHALL be simulated and
   every shared node voltage compared within reltol 1e-3 / abstol 1e-6.
2. WHEN `ngspice` is not on PATH THEN the test SHALL pass with an explicit
   skip message (not silently, not fail).
3. WHEN any node differs beyond tolerance THEN the test SHALL fail naming the
   circuit, node, both values, and the delta.
4. WHEN a sweep-based circuit runs THEN ngspice results SHALL be exported via
   `wrdata` CSV and compared point-by-point within the same tolerance.

**Independent Test**: `cargo test -p piperine-bench ngspice -- --nocapture`
shows per-circuit PASS lines; renaming ngspice binary away shows SKIP.

### P1: Transistor correctness debt fixed ⭐ MVP

**User Story**: As a circuit designer, I want MOS1/JFET/BJT operating points to
match ngspice, so simulations are trustworthy.

**Acceptance Criteria**:

1. WHEN `nmos_load` runs THEN piperine v(d) SHALL match ngspice (3.0 V) within
   tolerance (fix the ~1.5× Shichman-Hodges drain-current error).
2. WHEN an NMOS Id–Vgs / Id–Vds sweep (linear + saturation regions, ≥10 points
   each) is compared via CSV THEN every point SHALL match within reltol 1e-3
   + abstol 1e-9 A.
3. WHEN `jfet_bias` runs THEN the bias node SHALL match ngspice within
   tolerance (close the ~15 mV gap).
4. WHEN `bjt_ce` runs THEN piperine SHALL converge to the saturated point
   (Vce ≈ 0.11 V, matching ngspice within tolerance) — not the KCL-violating
   active point.
5. WHEN `bjt_mirror` runs THEN the DC solve SHALL converge and match ngspice
   within tolerance.

**Independent Test**: validation harness green on all 8 circuits + new sweep
circuits.

### P2: Retirement of external copies

**User Story**: As the maintainer, I want one source of truth so models cannot
drift again.

**Acceptance Criteria**:

1. WHEN migration lands THEN `headers/spice/` SHALL contain the complete model
   set and both external repos SHALL be marked deprecated (README pointer to
   piperine) — piperine SHALL NOT reference either path.
2. WHEN docs are read THEN CLAUDE.md/README/spec SHALL describe `spice` as a
   builtin stdlib namespace.

## Edge Cases

- WHEN a model uses `Real?` optional params THEN elaboration through the
  builtin-header path SHALL behave identically to package resolution (guard:
  junction smoke test).
- WHEN ngspice output format differs (locale/version) THEN the harness SHALL
  fail loud on unparseable output, never compare an empty node set (minimum:
  ≥1 shared node or error).
- WHEN both ngspice and piperine agree but on 0 shared nodes THEN the test
  SHALL fail (contract violation).
- WHEN a validation circuit fails to converge in piperine THEN the failure
  SHALL name the circuit and the solver error (not a parse-noise mismatch).

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| SPICE-01 | P1 builtin use | Done | Verified |
| SPICE-02 | P1 builtin use (elab clean) | Done | Verified |
| SPICE-03 | P1 builtin use (smoke tests) | Done | Verified |
| SPICE-04 | P1 builtin use (shadowing) | Done | Verified |
| SPICE-05 | P1 validation (harness compare) | Done | Verified |
| SPICE-06 | P1 validation (skip w/o ngspice) | Done | Verified |
| SPICE-07 | P1 validation (loud failure) | Done | Verified |
| SPICE-08 | P1 validation (CSV sweeps) | Done | Verified |
| SPICE-09 | P1 correctness (MOS1 op) | Done | Verified |
| SPICE-10 | P1 correctness (MOS1 sweeps) | Done | Verified |
| SPICE-11 | P1 correctness (JFET) | Done | Verified |
| SPICE-12 | P1 correctness (BJT saturation) | Done | Verified |
| SPICE-13 | P1 correctness (BJT mirror) | Done | Verified |
| SPICE-14 | P2 retirement | Done | Verified |
| SPICE-15 | P2 docs | Done | Verified |

**Coverage:** 15 total, 0 mapped to tasks, 15 unmapped ⚠️

## Success Criteria

- [ ] Scratch project with only `use spice::diode;` simulates via `piperine test`.
- [ ] `cargo test --workspace` green, zero warnings; examples green.
- [ ] All validation circuits (8 existing + new sweeps) pass vs ngspice-46.
- [ ] No piperine reference to `~/Git/piperine-spice` or `~/Git/plugins/piperine-spice` remains.

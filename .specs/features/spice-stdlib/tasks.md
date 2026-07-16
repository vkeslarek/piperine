# spice-stdlib Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. If the skill cannot be activated, STOP and tell the user.

---

**Design**: `.specs/features/spice-stdlib/design.md`
**Status**: DONE — Phases 1–3 (T1–T12) complete, batch 2 (2026-07-16)

## Batch 2 results (T7–T12)

- T7 `271454b` fix(spice): mos1 drain current matches ngspice — root cause was
  a JIT gap: bare `I(a,b)` in a force value read as a branch *voltage*, so the
  diode-style `V = R·I` fix collapsed to a short. New series-impedance force
  terms (`FlatForce::current_terms`, mirrors flux machinery) stamp `−R` on the
  branch-current column in DC/AC/tran. + nmos_id_vgs/nmos_id_vds sweeps.
- T8 `5293274` fix(spice): jfet model matches ngspice (same penalty-force
  class; equations were already line-faithful) + jfet_id_vds sweep.
- T9 `b4ace22` feat(solver): ngspice-style source stepping homotopy —
  `SimCtx.srcfact`/`$simparam("sourceScaleFactor")` scales isrc DC injection
  (cktop.c CKTsrcFact); SourceStepping unit tests (ramp/back-off/give-up/
  warm-start). bjt_ce's actual blocker was the penalty-force pattern in
  bjt.phdl (RC/RE/RB) — fixed there; converges to Vce ≈ 0.111 V.
- T10 `c7b21c1` fix(solver): bjt current mirror dc convergence — un-ignore
  only; the T9 model fix already made it converge (no homotopy composition
  needed).
- T12 `e70ddcb` feat(solver): compile-once parameter sweeps (restamp, no
  re-JIT) — `CircuitInstance::set_element_param` + `SimSession::run_op_sweep`
  + `AnalogKernel::compile_count`; harness sweeps rewired (~30 s → ~1.5 s);
  MD-18 enforcement test in its own test binary.
- T11: piperine-cli warnings fixed (cfg-split `python_setup.rs`); SOLVER_GAPS
  source-stepping entry and ROADMAP convergence entry checked off; zero
  `#[ignore]` in the harness.
- Workspace: 445 passed, 0 failed (baseline 432); build zero warnings.

## Batch 1 results (T1–T6)

- T1 `1492a04` feat(lang): spice models as builtin stdlib headers
- T2 `1d985b5` test(bench): spice stdlib smoke tests (junction + validate)
- T3 `2eb89b1` feat(project): project packages shadow builtin spice namespace
- T4 `d6cf8e8` docs (stale fork `~/Git/piperine-spice` absent on disk; only
  `~/Git/plugins/piperine-spice` got the deprecation README, committed there `47dc869`)
- T5 `ba370f3` fix(solver): per-variable DC device-bypass threshold (harness-exposed
  bug: whole-vector `reltol·max|v|` froze small nodes) + `42f549f` harness
- T6 `99c1dad` fix(spice): dio series resistance as exact `V=R*I` force branch
  (conditional-force penalty form `select(g,1e12,1e-12)` floors accuracy ~1e-5 A)
  + `9653b3f` sweep comparison
- Workspace: 432 passed, 0 failed.

**Intel for batch 2 (T7–T11):**
- `nmos_fixed` also `#[ignore]`d — fails `Newton: Linear solver returned NaN/Inf` (MOS1).
- Conditional-force pattern (`V(a,b) <- 0.0` under `if`) exists in bjt/mos/jfet —
  likely implicated in T7–T10 failures; JIT penalty path silently degrading accuracy
  is a flagged follow-up.
- Sweeps re-elaborate/re-JIT per point (~30 s / 37 points); fine unless MOS sweeps slow.
- Pre-existing `piperine-cli` warnings (unreachable statement, unused `project_path`)
  will block T11 zero-warnings gate — fix there.

---

## Test Coverage Matrix

> Generated from codebase + guidelines. Guidelines found: `CLAUDE.md`
> ("Build and test" — zero warnings bar, `cargo test --workspace`, tests of
> record listed per crate), `AGENTS.md` (MD-13 idiom rules).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `headers/spice/*.phdl` models | integration (elab + simulate) | every migrated file elaborates; smoke circuits `$op` correct | `crates/piperine-bench/tests/spice_smoke.rs` | `cargo test -p piperine-bench spice` |
| SourceMap / namespace resolution | unit | builtin resolution + project-shadowing branch (SPICE-01,04) | `crates/piperine-project/src/source_map.rs` (mod tests) or `tests/` | `cargo test -p piperine-project` |
| ngspice harness | integration | 1:1 to SPICE-05..08 ACs incl. skip path, 0-shared-nodes failure, loud mismatch | `crates/piperine-bench/tests/ngspice_validation.rs` | `cargo test -p piperine-bench ngspice` |
| Model equation fixes (mos/jfet PHDL) | integration (golden) | op + sweep circuits match ngspice within tolerance (SPICE-09..11) | `crates/piperine-bench/tests/ngspice/` circuit pairs | `cargo test -p piperine-bench ngspice` |
| Solver homotopy (source stepping) | integration + unit | bjt_ce/bjt_mirror converge to ngspice point (SPICE-12,13); strategy unit-testable per MD-05 | `crates/piperine-solver/` + validation circuits | `cargo test -p piperine-solver && cargo test -p piperine-bench ngspice` |
| Docs / deprecation READMEs | none | — (build gate only) | — | build gate only |

## Gate Check Commands

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | task-local crate tests | `cargo test -p <crate>` |
| Full | cross-crate behavior (harness, fixes) | `cargo test --workspace` |
| Build | phase completion | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

Baseline: **391 tests green** (STATE.md 2026-07-16). Never below baseline.

---

## Execution Plan

### Phase 1: Migration (T1 → T2 → T3 → T4)

### Phase 2: Harness (T5 → T6)

### Phase 3: Correctness (T7 → T8 → T9 → T10 → T11)

---

## Task Breakdown

### T1: headers/spice/ + namespace registration

**What**: Copy the 10 `.phdl` models from `~/Git/plugins/piperine-spice/src/`
(verbatim; exclude `.bak`/`.experiment`) into
`crates/piperine-lang/headers/spice/`; register namespace `spice` →
`headers/spice` in `piperine-project/src/source_map.rs` (insert-if-absent so
project packages win) and in `piperine-lang` `SourceMap::dummy`.
**Where**: `crates/piperine-lang/headers/spice/*.phdl`,
`crates/piperine-project/src/source_map.rs`,
`crates/piperine-lang/src/source_map.rs`
**Depends on**: None
**Reuses**: existing `add_namespace("piperine", …)` at `source_map.rs:57`
**Requirement**: SPICE-01, SPICE-02

**Done when**:
- [ ] Unit test: `use spice::diode;` resolves through the builtin path with no `Piperine.toml`
- [ ] Unit test: every `headers/spice/*.phdl` parses + elaborates cleanly
- [ ] Gate quick: `cargo test -p piperine-project && cargo test -p piperine-lang`

**Tests**: unit/integration · **Gate**: quick
**Commit**: `feat(lang): spice models as builtin stdlib headers`

### T2: Spice smoke tests in-repo

**What**: Port `tests/junction.phdl` + `tests/validate.phdl` (working-subset
benches) from the source repo into an in-process bench test.
**Where**: `crates/piperine-bench/tests/spice_smoke.rs` (+ fixture `.phdl`)
**Depends on**: T1
**Reuses**: `piperine-bench/tests/bench.rs` `elab` helper pattern
**Requirement**: SPICE-03

**Done when**:
- [ ] Junction devices (dio/bjt/mos1/jfet) converge via builtin `use spice::…`
- [ ] Passives/sources/controlled/switches smoke circuits pass ($op/$tran/$ac per validate.phdl)
- [ ] Gate quick: `cargo test -p piperine-bench spice`

**Tests**: integration · **Gate**: quick
**Commit**: `test(bench): spice stdlib smoke tests (junction + validate)`

### T3: Project-package shadowing

**What**: Test (and fix ordering if needed) that a project/dependency named
`spice` shadows the builtin namespace.
**Where**: `crates/piperine-project/src/source_map.rs` (+ its tests)
**Depends on**: T1
**Requirement**: SPICE-04

**Done when**:
- [ ] Test: `Piperine.toml` project named `spice` → its `src/` wins over `headers/spice`
- [ ] Gate quick: `cargo test -p piperine-project`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(project): project packages shadow builtin spice namespace`

### T4: Docs + retire external repos

**What**: Document `spice` as builtin (CLAUDE.md crate table/headers note,
README, `docs/spec/` stdlib section if it lists headers); write deprecation
READMEs in `~/Git/piperine-spice` and `~/Git/plugins/piperine-spice` pointing
at piperine (repos untouched otherwise).
**Where**: `CLAUDE.md`, `README.md`, `docs/spec/`, external repos' READMEs
**Depends on**: T1
**Requirement**: SPICE-14, SPICE-15

**Done when**:
- [ ] No piperine code/doc references external repo paths
- [ ] Gate build: `cargo build --workspace` zero warnings

**Tests**: none (matrix: docs) · **Gate**: build
**Commit**: `docs: spice is builtin stdlib; deprecate external model repos`

### T5: NgspiceHarness + OP circuits

**What**: `NgspiceHarness` struct (detect → skip; run `ngspice -b`; parse
`v(node) = …`; in-process piperine OP via bench session; tolerance compare
`|Δ| ≤ abstol + reltol·max`); port all 8 circuit pairs from
`~/Git/plugins/piperine-spice/validation/circuits/`. Currently-failing
circuits (`bjt_ce`, `bjt_mirror`, `nmos_load`, `jfet_bias`) registered but
`#[ignore]`d with a pointer to T7–T10.
**Where**: `crates/piperine-bench/tests/ngspice_validation.rs`,
`crates/piperine-bench/tests/ngspice/*.{cir,phdl}`
**Depends on**: T2
**Reuses**: validation/run.py contract; bench session/result objects
**Requirement**: SPICE-05, SPICE-06, SPICE-07

**Done when**:
- [ ] divider/rdiode/diode_series pass vs live ngspice-46
- [ ] No-binary path: skip notice + pass (test by PATH manipulation)
- [ ] Failure modes loud: 0 shared nodes fails; mismatch names circuit/node/values/Δ
- [ ] Gate full: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(bench): ngspice golden cross-validation harness`

### T6: Sweep cases (wrdata CSV)

**What**: `sweep_case` — ngspice `.dc` + `wrdata` export parsed point-by-point
vs piperine bench-loop (stage swept source, OP per point, compile once); first
sweep circuit: diode I–V (known-good model) as the pattern.
**Where**: `ngspice_validation.rs` + `tests/ngspice/diode_iv.*`
**Depends on**: T5
**Requirement**: SPICE-08

**Done when**:
- [ ] Diode I–V sweep (≥20 points fwd/rev) matches within reltol 1e-3 + abstol 1e-9 A
- [ ] wrdata file parsed strictly (loud on malformed)
- [ ] Gate quick: `cargo test -p piperine-bench ngspice`

**Tests**: integration · **Gate**: quick
**Commit**: `test(bench): dc-sweep golden comparison via ngspice wrdata`

### T7: MOS1 drain-current fix

**What**: Add `nmos_id_vgs` + `nmos_id_vds` sweep pairs; bisect divergence
region; line-diff `headers/spice/mos.phdl` vs ngspice `mos1load.c`/`mos1temp.c`;
fix the equation(s); un-ignore `nmos_load`.
**Where**: `crates/piperine-lang/headers/spice/mos.phdl`,
`tests/ngspice/nmos_*.{cir,phdl}`
**Depends on**: T6
**Requirement**: SPICE-09, SPICE-10

**Done when**:
- [ ] `nmos_load` passes (v(d) = 3.0 V ± tol)
- [ ] Id–Vgs and Id–Vds sweeps (≥10 pts each, linear + saturation) pass
- [ ] Gate full: `cargo test --workspace`

**Tests**: integration (golden) · **Gate**: full
**Commit**: `fix(spice): mos1 drain current matches ngspice`

### T8: JFET bias fix

**What**: JFET Id sweep pair; bisect the ~15 mV discrepancy vs `jfetload.c`;
fix `headers/spice/jfet.phdl`; un-ignore `jfet_bias`.
**Where**: `crates/piperine-lang/headers/spice/jfet.phdl`, `tests/ngspice/jfet_*`
**Depends on**: T6
**Requirement**: SPICE-11

**Done when**:
- [ ] `jfet_bias` passes; JFET sweep passes
- [ ] Gate full: `cargo test --workspace`

**Tests**: integration (golden) · **Gate**: full
**Commit**: `fix(spice): jfet model matches ngspice`

### T9: Source stepping completion (solver)

**What**: Complete the PARTIAL source-stepping `HomotopyStrategy` (ngspice
`cktop.c` ramp semantics: scale all independent sources, warm-start, back-off)
so deep-saturation BJT circuits converge; MD-05/MD-13 conformant.
**Where**: `crates/piperine-solver/src/solver/convergence.rs` (+ `dc.rs` wiring)
**Depends on**: T5
**Requirement**: SPICE-12

**Done when**:
- [ ] Strategy unit tests (ramp schedule, back-off, warm-start reuse)
- [ ] `bjt_ce` un-ignored, converges to saturated point (Vce ≈ 0.11 V ± tol)
- [ ] Gate full: `cargo test --workspace` (solver baseline intact)

**Tests**: unit + integration (golden) · **Gate**: full
**Commit**: `feat(solver): ngspice-style source stepping homotopy`

### T10: BJT mirror convergence

**What**: Un-ignore `bjt_mirror`; if T9 alone insufficient, compose
gmin+source strategies per ngspice order (gmin → source); if still failing,
STOP and escalate with diagnosis (design risk bound).
**Where**: solver convergence plan + `tests/ngspice/bjt_mirror.*`
**Depends on**: T9
**Requirement**: SPICE-13

**Done when**:
- [ ] `bjt_mirror` converges and matches ngspice within tolerance
- [ ] Gate full: `cargo test --workspace`

**Tests**: integration (golden) · **Gate**: full
**Commit**: `fix(solver): bjt current mirror dc convergence`

### T12: Compile-once sweeps (MD-18)

**What**: Sweep loops must not re-elaborate/re-JIT per point (user directive
2026-07-16, `.specs/STATE.md` MD-18). Add a solver-level restamp/staging path:
elaborate + compile once, update the swept parameter value on the compiled
circuit, re-run DC per point. Rewire the harness `sweep_case` piperine side
onto it.
**Where**: `crates/piperine-solver` (param restamp on compiled circuit),
`crates/piperine-bench` (session sweep path), `ngspice_validation.rs`
**Depends on**: T6
**Requirement**: MD-18 (project decision); supports SPICE-08, SPICE-10

**Done when**:
- [ ] Test proves single JIT compilation across a multi-point sweep (e.g. compile counter/instrumentation or API shape that makes re-JIT impossible)
- [ ] Diode + MOS sweeps produce identical results to the old path, within tolerance vs ngspice
- [ ] Gate full: `cargo test --workspace`

**Tests**: unit + integration · **Gate**: full
**Commit**: `feat(solver): compile-once parameter sweeps (restamp, no re-JIT)`

### T11: Full green + zero ignores

**What**: Remove all remaining `#[ignore]`s in the harness; final sweep of
docs (`SOLVER_GAPS.md` items fixed here checked off); baseline audit.
**Where**: `ngspice_validation.rs`, `SOLVER_GAPS.md`, `ROADMAP.md`
**Depends on**: T7, T8, T10
**Requirement**: SPICE-05..13 closure

**Done when**:
- [ ] All 8 original + new sweep circuits run un-ignored and pass
- [ ] Gate build: `cargo build --workspace` zero warnings + `cargo test --workspace` ≥ baseline 391 + new tests
- [ ] Examples still green (`run_examples.rs`)

**Tests**: integration · **Gate**: build
**Commit**: `chore(specs): spice-stdlib validation complete`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3

Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4
Phase 2:  T5 ──→ T6
Phase 3:  T7 ──→ T8 ──→ T9 ──→ T10 ──→ T11
```

(Sequential; T3/T4 depend only on T1 but run in order; T9 depends on T5 and
runs after T8 within Phase 3.)

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram | Status |
|---|---|---|---|
| T1 | none | phase start | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T1 | after T2 (order), dep backward | ✅ |
| T4 | T1 | after T3 (order), dep backward | ✅ |
| T5 | T2 | phase 2 after phase 1 | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | T6 | phase 3 after phase 2 | ✅ |
| T8 | T6 | after T7 (order), dep backward | ✅ |
| T9 | T5 | after T8 (order), dep backward | ✅ |
| T10 | T9 | T9→T10 | ✅ |
| T11 | T7,T8,T10 | last | ✅ |

## Test Co-location Validation

| Task | Layer | Matrix Requires | Task Says | Status |
|---|---|---|---|---|
| T1 | models + resolution | unit/integration | unit/integration | ✅ |
| T2 | models | integration | integration | ✅ |
| T3 | resolution | unit | unit | ✅ |
| T4 | docs | none | none | ✅ |
| T5 | harness | integration | integration | ✅ |
| T6 | harness | integration | integration | ✅ |
| T7 | model fix | integration golden | integration golden | ✅ |
| T8 | model fix | integration golden | integration golden | ✅ |
| T9 | solver | unit + integration | unit + integration | ✅ |
| T10 | solver | integration golden | integration golden | ✅ |
| T11 | closure | integration | integration | ✅ |

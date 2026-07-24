# Spectral & Small-Signal Analyses Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill
files by filesystem path. The skill is the source of truth for the full flow
(per-task cycle, sub-agent delegation, adequacy review, Verifier,
discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Design**: `.specs/features/spectral-analyses/design.md`
**Status**: T1–T14 committed; T15 (ngspice cross-checks) gate in progress; T16 (docs) next

---

## Test Coverage Matrix

> Generated from codebase + project guidelines + spec — confirm before
> Execute. Guidelines found: `AGENTS.md` (§Test placement, §Hard rules — no
> macros, no loose fns), `CLAUDE.md` (§Build and test, §Tests of record).
> No coverage-threshold config → strong defaults applied (every AC + every
> listed edge case).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver analysis driver (`pz.rs`/`sp.rs`/`disto.rs`) | unit | All branches; 1:1 to spec ACs; every listed edge case (fail-loud paths) | `crates/piperine-solver/src/analyses/*.rs` `#[cfg(test)]` + root `tests/{pz,sp,disto}.rs` | `cargo test -p piperine-solver` + `cargo test -p piperine <name>` |
| Host API result/waveform (`piperine-api`) | unit | All ACs for the host surface (FOUR-*, result shapes) | root `tests/four.rs`, `piperine-api` inline `#[cfg(test)]` | `cargo test -p piperine four` / `cargo test -p piperine-api` |
| Codegen JIT kernel (`disto` derivatives) | unit | 2nd/3rd derivative value-for-value vs symbolic reference; fail-loud on unlowerable | `crates/piperine-codegen/tests/analog_jit.rs` | `cargo test -p piperine-codegen` |
| Frontend attribute (`@rfport`) | unit | `@rfport` resolves to `(num, z0, node)`; fail-loud on bad args | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Python facade parity (MD-22) | integration | Same call shape + same values Rust==Py for every new analysis | `crates/piperine-python/tests/*_parity.rs` | `cargo test -p piperine-python` |
| ngspice cross-check | integration | Where ngspice supports the analysis: `.four`/`.disto`/`.pz` within tolerance | root `tests/ngspice_validation.rs` (+`tests/ngspice/`) | `cargo test -p piperine ngspice` |
| Docs (ROADMAP/STATE) | none | build gate only | `ROADMAP.md`, `.specs/STATE.md` | build gate |

## Gate Check Commands

> Generated from codebase — confirm before Execute. Zero rustc warnings is the
> bar (CLAUDE.md).

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task touching one crate | `cargo test -p <crate>` (the crate the task changed) |
| Full | After a task crossing crates (host wiring, parity) | `cargo test --workspace` |
| Build | After phase completion / docs-only tasks | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

Phases ordered, run sequentially; tasks within a phase run in order.

### Phase 1: `.four` Fourier (both hosts) — foundational MD-22 proof

```
T1 → T2
```

### Phase 2: `.pz` Pole-Zero (both hosts)

```
T3 → T4 → T5 → T6
```

### Phase 3: `.sp` S-Parameters (`@rfport` attribute + both hosts)

```
T7 → T8 → T9
```

### Phase 4: `.disto` Volterra distortion (codegen + both hosts)

```
T10 → T11 → T12 → T13 → T14
```

### Phase 5: Cross-validation + docs

```
T15 → T16
```

---

## Task Breakdown

### T1: `.four` — `FourierResult` + `Waveform::fourier` (Rust)

**What**: Add `FourierComponent`/`FourierResult` types and
`Waveform::fourier(f0, n_harmonics)` computing the DFT (window → resample via
`Waveform::at` → direct DFT at `k·f0` → magnitude/phase/norm + THD).
**Where**: `crates/piperine-api/src/waveform.rs`, `results.rs` (or a new
`fourier.rs` owned by the waveform module — no loose fns, MD-13)
**Depends on**: None
**Reuses**: `Waveform::at` interpolation; `num_complex`
**Requirement**: FOUR-01, FOUR-02, FOUR-03, FOUR-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `Waveform::fourier` returns DC + `n_harmonics-1` components (freq, mag,
      phase, norm_mag, norm_phase) + THD
- [x] Synthesized `sin(2πf0 t)+0.1 sin(2π·3f0 t)` → HD3≈0.1, THD≈0.1, DC≈0
      (unit test, ≤1e-6 vs analytic)
- [x] Fail loud: `f0≤0`, span<1 period, `n_harmonics<2` (typed error)
- [x] Non-uniform input resampled before DFT (test with jittered grid)
- [x] Gate passes: `cargo test -p piperine-api` / `cargo test -p piperine four`
- [x] Test count: ≥4 new tests pass (no silent deletions)

**Tests**: unit · **Gate**: quick
**Commit**: `feat(api): .four Fourier post-processing on Waveform (FOUR-01..04)`

---

### T2: `.four` — Python `Waveform.fourier` + parity (MD-22)

**What**: Expose `waveform.fourier(f0, n_harmonics)` on the Python `Waveform`
facade (numpy-backed) returning the same fields; add a Rust==Python parity
test.
**Where**: `crates/piperine-python/python/piperine/__init__.py`,
`crates/piperine-python/src/…` (binding), `crates/piperine-python/tests/four_parity.rs`
**Depends on**: T1
**Reuses**: `sens_parity.rs` parity-test pattern; the T1 result shape
**Requirement**: FOUR-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `waveform.fourier(...)` exists on the Python facade, same field names as
      Rust
- [x] Parity test: identical values Rust vs Python on the synthesized signal
      (≤1e-9)
- [x] Gate passes: `cargo test -p piperine-python`
- [x] Test count: ≥1 parity test passes

**Tests**: integration · **Gate**: full
**Commit**: `feat(python): .four Waveform.fourier + Rust parity (FOUR-05)`

---

### T3: `.pz` — solver skeleton + G/C matrix extraction

**What**: New `analyses/pz.rs`: `PoleZeroOptions` (input source, output
node/pair), `PoleZeroSolver::new` (DC point via `DcSolver`, dense `G` from the
`tf.rs` `assemble_dc_stamps` pattern, dense `C = Im(Y(jω0))/ω0` from one
`load_ac`, plus the two-ω linearity guard) and `PoleZeroResult` in `result.rs`.
**Where**: `crates/piperine-solver/src/analyses/pz.rs` (new),
`analyses/mod.rs` (register), `result.rs` (result type), `error.rs`
(`SolverDomain::Pz`)
**Depends on**: None
**Reuses**: `DcSolver`, `tf.rs::assemble_dc_stamps`, `Element::load_ac`,
`FaerSparseLinearSystem`
**Requirement**: PZ-06 (linearity guard)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `G` and `C` dense matrices extracted; unit test on an RC asserts the
      known 2×2 `G`/`C` entries
- [x] Two-ω guard: a synthetic frequency-nonlinear stamp → fail loud (PZ-06)
- [x] `//!` module doc states the descriptor-system algorithm
- [x] Gate passes: `cargo test -p piperine-solver`
- [x] Test count: ≥2 new tests pass

**Tests**: unit · **Gate**: quick
**Commit**: `feat(solver): .pz G/C extraction + linearity guard (PZ-06)`

---

### T4: `.pz` — poles via QZ + no-reactive fail-loud

**What**: Poles = finite generalized eigenvalues of `(−G, C)` via faer
`Mat::generalized_eigen`; filter infinite (`|β|≈0`) eigenvalues; snap real /
pair conjugates; fail loud when no finite pole and no reactive stamp.
**Where**: `crates/piperine-solver/src/analyses/pz.rs`
**Depends on**: T3
**Reuses**: faer `generalized_eigen` (`S_a`/`S_b`), `num_complex`
**Requirement**: PZ-01, PZ-03, PZ-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] RC (`R=1k,C=1µ`) → one real pole at `−1000 rad/s` (≤1e-6 rel)
- [x] series RLC → conjugate pair at analytic `−R/2L ± j√(1/LC−(R/2L)²)`
- [x] resistor-only circuit → fail loud (no finite poles, PZ-05)
- [x] infinite eigenvalues filtered; conjugates paired, reals snapped (PZ-03)
- [x] Gate passes: `cargo test -p piperine-solver` + root `cargo test -p piperine pz`
- [x] Test count: ≥3 new tests pass

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver): .pz poles via QZ (PZ-01,03,05)`

---

### T5: `.pz` — zeros via Rosenbrock system pencil

**What**: Zeros = finite generalized eigenvalues of the bordered pencil
`([−G, b; lᵀ, 0], [C, 0; 0, 0])`, `b`=input excitation column, `l`=output
selector; same QZ + infinite-filter.
**Where**: `crates/piperine-solver/src/analyses/pz.rs`
**Depends on**: T4
**Reuses**: T3 `b`/`l` construction, T4 QZ + filter helper
**Requirement**: PZ-02, PZ-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Bridged-T / RC-with-zero network → known transmission zero (≤1e-6 rel)
- [x] zeros conjugate-paired / real-snapped like poles
- [x] `//!` doc documents the Rosenbrock pencil derivation
- [x] Gate passes: `cargo test -p piperine-solver` + `cargo test -p piperine pz`
- [x] Test count: ≥2 new tests pass

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver): .pz zeros via Rosenbrock pencil (PZ-02)`

---

### T6: `.pz` — host wiring both sides + MD-22 parity

**What**: Rust object-model `Module::pz(...)` + Python `module.pz(...)`
returning the same `PoleZeroResult` shape; parity test.
**Where**: `crates/piperine-api/src/session.rs` (Rust host), `prelude.rs`,
`crates/piperine-python/…` (binding + facade), `tests/pz.rs`,
`crates/piperine-python/tests/pz_parity.rs`
**Depends on**: T5
**Reuses**: `pss`/`sens` host-wiring pattern (`run_pss`/`module.pss`)
**Requirement**: PZ-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `.pz` callable on both hosts, same names/shape (MD-22)
- [x] Parity test: identical poles/zeros Rust vs Python on RLC (≤1e-9)
- [x] Gate passes: `cargo test --workspace`
- [x] Test count: ≥2 new tests pass

**Tests**: integration · **Gate**: full
**Commit**: `feat(api,python): .pz uniform host surface (PZ-07)`

---

### T7: `.sp` — `@rfport` attribute schema + POM plumbing

**What**: Register the stdlib `@rfport(num, z0)` attribute schema; resolve it on
a node/wire during elaboration to `(num, z0, node_ref)` reachable by the host;
fail loud on bad args.
**Where**: `crates/piperine-lang/src/…` (attribute schema registration; follow
Part VI `@schema`/`@attribute`), `crates/piperine-lang/tests/*.rs`
**Depends on**: None
**Reuses**: existing attribute-schema machinery (Part VI, `piperine-plugin`
attr schemas), POM node attribute storage
**Requirement**: SP-01 (declaration), SP-05 (bad-arg fail-loud)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `@rfport(num=1, z0=50) wire p;` elaborates; host reads `(1, 50.0, p)`
- [x] Fail loud: `z0≤0`, duplicate `num`, unknown node (SP-05)
- [x] Gate passes: `cargo test -p piperine-lang`
- [x] Test count: ≥3 new tests pass

**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): @rfport attribute for .sp ports (SP-01,05)`

---

### T8: `.sp` — solver: per-port Thévenin excitation + S-matrix

**What**: New `analyses/sp.rs`: `SpOptions` (ports + sweep), `SpSolver` — per
frequency, add each port's `z0` termination, drive one port at a time with a
`1V` Thévenin source behind `z0`, solve the AC complex system, compute power
waves `a/b` and `S_ij = b_i/a_j`; `SpResult` in `result.rs`;
`SolverDomain::Sp`.
**Where**: `crates/piperine-solver/src/analyses/sp.rs` (new),
`analyses/mod.rs`, `result.rs`, `error.rs`
**Depends on**: T7
**Reuses**: `AcSystem`/`load_ac` complex solve, `FaerSparseLinearSystem`,
Kurokawa power-wave formula
**Requirement**: SP-02, SP-03, SP-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Matched series-R attenuator → analytic `S11`,`S21` (≤1e-6)
- [x] Reciprocal L-C section → `S12==S21`, `|S_ii|≤1` (SP-03)
- [x] Fail loud: zero ports / degenerate coincident port nodes (SP-05)
- [x] `//!` doc documents the power-wave excitation algorithm
- [x] Gate passes: `cargo test -p piperine-solver` + `cargo test -p piperine sp`
- [x] Test count: ≥3 new tests pass

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver): .sp S-parameters via power-wave ports (SP-02,03,05)`

---

### T9: `.sp` — host wiring both sides + validation

**What**: Rust `Module::sp(...)` + Python `module.sp(...)`, same `SpResult`
shape; shunt-C low-pass `S21` roll-off validation; parity test.
**Where**: `piperine-api` host + `prelude.rs`, `piperine-python` binding+facade,
`tests/sp.rs`, `crates/piperine-python/tests/sp_parity.rs`
**Depends on**: T8
**Reuses**: `.pz` host-wiring pattern from T6
**Requirement**: SP-04, SP-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `.sp` callable both hosts, same shape (MD-22, SP-06)
- [x] shunt-C low-pass `S21` matches closed-form roll-off (SP-04)
- [x] Parity test Rust==Python (≤1e-9)
- [x] Gate passes: `cargo test --workspace`
- [x] Test count: ≥2 new tests pass

**Tests**: integration · **Gate**: full
**Commit**: `feat(api,python): .sp uniform host surface (SP-04,06)`

---

### T10: `.disto` — 2nd-derivative JIT kernel (`disto2`)

**What**: Emit a 2nd-order derivative kernel per nonlinear contribution by
reapplying `diff::d_dv` to the already-differentiated `Expr` (resistive +
charge), referencing the shared value-tape (no inline tree blow-up); fail loud
(`CodegenError::Unsupported`, names the device) when a higher derivative can't
lower.
**Where**: `crates/piperine-codegen/src/lower/diff.rs`,
`crates/piperine-codegen/src/jit/analog.rs`, `flatten.rs`
**Depends on**: None
**Reuses**: `diff::d_dv`/`d_dnode`, the temp-tape flattener
([[flattener-temp-tape]]), the residual/Jacobian emit skeleton
**Requirement**: DISTO-03, DISTO-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `disto2` kernel value-for-value vs a hand-derived 2nd derivative on a
      polynomial device (analog_jit-style test)
- [x] unlowerable higher-derivative path → `CodegenError::Unsupported` naming
      the device (DISTO-04)
- [x] Gate passes: `cargo test -p piperine-codegen`
- [x] Test count: ≥2 new tests pass

**Tests**: unit · **Gate**: quick
**Commit**: `feat(codegen): 2nd-derivative disto2 kernel (DISTO-03,04)`

---

### T11: `.disto` — single-tone HD2 driver

**What**: New `analyses/disto.rs`: `DistoOptions` (F1, amplitude, output),
`DistoSolver` — linear solve at F1, 2nd-order nonlinear currents from `disto2`
× first-order responses, solve at `2·F1`, `HD2 = |X2|/|X1|`; `DistoResult`;
`SolverDomain::Disto`.
**Where**: `crates/piperine-solver/src/analyses/disto.rs` (new),
`analyses/mod.rs`, `result.rs`, `error.rs`
**Depends on**: T10
**Reuses**: `AcSystem` complex solve, symbolic LU reuse across the mix
frequencies
**Requirement**: DISTO-01 (HD2 half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] cubic VCCS `i=g1 v+g2 v²+g3 v³` at bias → `HD2=½(g2/g1)A` (≤1e-3 rel)
- [x] `//!` doc documents the nonlinear-currents (Volterra) algorithm
- [x] Gate passes: `cargo test -p piperine-solver` + `cargo test -p piperine disto`
- [x] Test count: ≥1 new test passes

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver): .disto single-tone HD2 (DISTO-01)`

---

### T12: `.disto` — 3rd-derivative kernel + HD3

**What**: Emit `disto3` (3rd derivative, reapplied `d_dv`); extend the driver
with the 3rd-order nonlinear current `(1/6)f'''X1³ + ½f''(2 X1⊙X2)`, solve at
`3·F1`, `HD3 = |X3|/|X1|`.
**Where**: `crates/piperine-codegen/src/lower/diff.rs`, `jit/analog.rs`,
`crates/piperine-solver/src/analyses/disto.rs`
**Depends on**: T11
**Reuses**: T10 `disto2` machinery, T11 driver
**Requirement**: DISTO-01 (HD3 half), DISTO-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] cubic VCCS → `HD3=¼(g3/g1)A²` (≤1e-3 rel, DISTO-05)
- [x] `disto3` kernel value-for-value on the polynomial device
- [x] Gate passes: `cargo test --workspace`
- [x] Test count: ≥2 new tests pass

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver,codegen): .disto HD3 via 3rd derivative (DISTO-01,05)`

---

### T13: `.disto` — two-tone IM2/IM3

**What**: Extend `DistoSolver` for two-tone (F1, F2, `skw2`/`refpow`):
first-order at F1 & F2, mix currents at `F1±F2` (IM2) and `2F1±F2`/`2F2±F1`
(IM3); report IM2/IM3.
**Where**: `crates/piperine-solver/src/analyses/disto.rs`
**Depends on**: T12
**Reuses**: T10/T12 kernels, T11/T12 driver
**Requirement**: DISTO-02

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] two-tone polynomial stage → analytic IM2/IM3 (≤1e-3 rel)
- [x] `//!` doc documents the intermod product bookkeeping
- [x] Gate passes: `cargo test -p piperine-solver` + `cargo test -p piperine disto`
- [x] Test count: ≥1 new test passes

**Tests**: unit · **Gate**: full
**Commit**: `feat(solver): .disto two-tone IM2/IM3 (DISTO-02)`

---

### T14: `.disto` — host wiring both sides + MD-22 parity

**What**: Rust `Module::disto(...)` + Python `module.disto(...)`, same
`DistoResult` shape; parity test.
**Where**: `piperine-api` host + `prelude.rs`, `piperine-python` binding+facade,
`tests/disto.rs`, `crates/piperine-python/tests/disto_parity.rs`
**Depends on**: T13
**Reuses**: `.pz`/`.sp` host-wiring pattern
**Requirement**: DISTO-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `.disto` callable both hosts, same shape (MD-22, DISTO-06)
- [x] Parity test Rust==Python on the cubic stage (≤1e-9)
- [x] Gate passes: `cargo test --workspace`
- [x] Test count: ≥2 new tests pass

**Tests**: integration · **Gate**: full
**Commit**: `feat(api,python): .disto uniform host surface (DISTO-06)`

---

### T15: ngspice cross-validation decks

**What**: Add ngspice reference decks + assertions for the analyses ngspice
supports (`.four`, `.disto`, `.pz`) to the cross-check harness; skip `.sp`
gracefully if the local ngspice lacks it.
**Where**: `tests/ngspice_validation.rs`, `tests/ngspice/`
**Depends on**: T2, T6, T14
**Reuses**: existing `ngspice_op`/`compare_op` harness pattern
**Requirement**: Success Criteria (ngspice cross-checks)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `.four`/`.disto`/`.pz` piperine-vs-ngspice within tolerance (or the
      guarded skip when ngspice absent, matching existing `detect()`)
- [x] Gate passes: `cargo test -p piperine ngspice`
- [x] Test count: ≥3 new cross-checks pass (or skip cleanly)

**Note (2026-07-19):** the `nmos2`/`nmos3` op-point cases in this same suite
surfaced a real regression from the DISTO-01..06 work: `compile_disto2`/
`compile_disto3` unrolled every controlling-branch combination into one
Cranelift function, which never terminated compiling for a many-branch
device (MOSFET). Fixed alongside T15 (not a separate task — found by this
gate, per the tlc-spec-driven Execute discipline): symbolic redundancy
removed (`d_dv_once_more_named`/`d_dv_thrice_from_twice` in `diff.rs`), one
Cranelift function per branch combination instead of one unrolled function,
a dev-profile `opt-level = 3` override for the `cranelift-*` crates
(Cargo.toml — Cranelift's own codegen is prohibitively slow unoptimized),
and a `compile_disto: bool` flag threaded `AnalogKernel` →
`CompiledModule` → `CircuitCompiler` → `SimSession::build_circuit`
(default skip; only `run_disto` opts in) so non-`.disto` analyses pay zero
cost for kernels they never use. Full ngspice suite: 370s (bounded but
slow) → 5s. `cargo test --workspace`: 582 passed, 0 failed, 5 ignored.

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): cross-check .four/.pz/.disto (Success Criteria)`

---

### T16: Docs — ROADMAP + STATE update

**What**: Flip the ROADMAP P1 `.four`/`.pz`/`.disto`/`.sp` line to done (with
task refs); append a `spectral-analyses` handoff snapshot + any decision note
to `.specs/STATE.md`; note `@rfport` as a reserved stdlib attribute.
**Where**: `ROADMAP.md`, `.specs/STATE.md`, attribute-schema doc
**Depends on**: T15
**Reuses**: existing ROADMAP `[x]` entry style
**Requirement**: Success Criteria (documented)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] ROADMAP P1 Analyses row updated; backlog table row removed/updated
- [x] STATE.md handoff snapshot added
- [x] Gate passes: `cargo build --workspace` (zero warnings) + `cargo test --workspace`

**Tests**: none · **Gate**: build
**Commit**: `docs(specs): spectral-analyses done — ROADMAP P1 + STATE`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1 ──→ T2
Phase 2:  T3 ──→ T4 ──→ T5 ──→ T6
Phase 3:  T7 ──→ T8 ──→ T9
Phase 4:  T10 ─→ T11 ─→ T12 ─→ T13 ─→ T14
Phase 5:  T15 ─→ T16
```

Execution is strictly sequential — one task at a time, in order.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1 Fourier types + method | 1 module concept | ✅ Granular |
| T2 Python four + parity | 1 binding + 1 test | ✅ Granular |
| T3 G/C extraction | 1 concept (matrix build) | ✅ Granular |
| T4 poles QZ | 1 function path | ✅ Granular |
| T5 zeros pencil | 1 function path | ✅ Granular |
| T6 pz host wiring | 1 surface (both hosts, cohesive) | ✅ Granular |
| T7 @rfport attribute | 1 attribute schema | ✅ Granular |
| T8 sp solver | 1 analysis driver | ✅ Granular |
| T9 sp host wiring | 1 surface | ✅ Granular |
| T10 disto2 kernel | 1 kernel | ✅ Granular |
| T11 HD2 driver | 1 driver path | ✅ Granular |
| T12 disto3 + HD3 | 1 kernel + driver ext (cohesive) | ✅ Granular |
| T13 two-tone IM | 1 driver ext | ✅ Granular |
| T14 disto host wiring | 1 surface | ✅ Granular |
| T15 ngspice decks | test-only | ✅ Granular |
| T16 docs | docs-only | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
| ---- | ----------------- | ------------- | ------ |
| T1 | None | (phase start) | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | None | (phase start) | ✅ |
| T4 | T3 | T3→T4 | ✅ |
| T5 | T4 | T4→T5 | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | None | (phase start) | ✅ |
| T8 | T7 | T7→T8 | ✅ |
| T9 | T8 | T8→T9 | ✅ |
| T10 | None | (phase start) | ✅ |
| T11 | T10 | T10→T11 | ✅ |
| T12 | T11 | T11→T12 | ✅ |
| T13 | T12 | T12→T13 | ✅ |
| T14 | T13 | T13→T14 | ✅ |
| T15 | T2, T6, T14 | (cross-phase, backward) | ✅ |
| T16 | T15 | T15→T16 | ✅ |

All dependencies point backward or within-phase. ✅

---

## Test Co-location Validation

| Task | Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | ---------------------- | --------------- | --------- | ------ |
| T1 | Host API waveform | unit | unit | ✅ |
| T2 | Python facade parity | integration | integration | ✅ |
| T3 | Solver analysis driver | unit | unit | ✅ |
| T4 | Solver analysis driver | unit | unit | ✅ |
| T5 | Solver analysis driver | unit | unit | ✅ |
| T6 | Python facade parity | integration | integration | ✅ |
| T7 | Frontend attribute | unit | unit | ✅ |
| T8 | Solver analysis driver | unit | unit | ✅ |
| T9 | Python facade parity | integration | integration | ✅ |
| T10 | Codegen JIT kernel | unit | unit | ✅ |
| T11 | Solver analysis driver | unit | unit | ✅ |
| T12 | Codegen + solver | unit | unit | ✅ |
| T13 | Solver analysis driver | unit | unit | ✅ |
| T14 | Python facade parity | integration | integration | ✅ |
| T15 | ngspice cross-check | integration | integration | ✅ |
| T16 | Docs | none | none | ✅ |

All ✅ — no violations.

---

## Tools (all tasks)

Pure Rust/Python cargo workspace, no MCP servers configured. Every task uses
the standard Read/Edit/Bash(cargo) tools; no per-task MCP or Skill. If a task
needs library-API confirmation (faer QZ, PyO3), use the Knowledge
Verification Chain (codebase → docs → context7 → web), not guesswork.
</content>

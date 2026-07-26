# Spectral & Small-Signal Analyses Specification

`.four`, `.pz`, `.disto`, `.sp` — the remaining P1 analyses a working SPICE
user expects. Closes the last "Analyses" row of ROADMAP P1
(`.four` / `.pz` / `.disto` / `.sp`, previously "niche, post-V1"; promoted
to V1 by user 2026-07-19).

**Status: DELIVERED 2026-07-19** — T1–T16 complete. All four analyses on
both hosts, ngspice-validated where a reference exists (`.four`/`.pz`/
`.disto`), `cargo test --workspace` green (582 passed, 0 failed, 5 ignored).
T15's gate surfaced a real `.disto` compile-explosion regression on
many-branch devices (MOSFETs) — fixed as part of closing this feature (see
`tasks.md` T15 note): symbolic-differentiation redundancy removed,
per-branch-combination Cranelift functions instead of one unrolled
function, a dev-profile Cranelift opt-level fix, and a `compile_disto`
flag so non-`.disto` analyses skip the kernel entirely (default off).

## Problem Statement

The native solver ships `.dc`/`.ac`/`.tran`/`.noise`/`.tf`/`.sens`/PSS but
still lacks the four classic frequency-domain / spectral analyses SPICE users
reach for: Fourier decomposition of a transient (`.four`), pole-zero location
of the linearized network (`.pz`), small-signal distortion (`.disto`), and
scattering parameters (`.sp`). Without them "everything I can do in SPICE, I
can do here" (P1 north star) is unmet. Our structure — DC operating point,
AC linearization, symbolic differentiation (`diff.rs`), the `Element` AC
stamp (`G + jωC` form) — already carries the machinery; these analyses are
the assembly, not new primitives.

## Goals

- [x] `.four`: Fourier decomposition of a transient waveform (DC + harmonics
      magnitude/phase, THD) on **both hosts** (MD-22), computed as
      post-processing on `Waveform` — no new solver analysis.
- [x] `.pz`: poles and zeros of a linearized input/output transfer function,
      via a generalized eigenvalue problem on the `(G, C)` MNA pencil, both
      hosts.
- [x] `.disto`: full Volterra small-signal distortion — HD2/HD3 (single tone)
      and IM2/IM3 (two tone) from symbolic 2nd/3rd-order device derivatives,
      both hosts.
- [x] `.sp`: N-port scattering parameters over the AC engine with a PHDL
      **port primitive** (format follows Verilog-AMS / Spectre `port`, agreed
      in Design before any frontend edit), both hosts.
- [x] Every analysis carries a documented algorithm block (`design.md` + a
      module `//!` doc) — the algorithm is legible from the source, not folklore.
- [x] Every gap that cannot be faithfully computed **fails loud**
      (`SolverDomain` error), never a silent `0.0`.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| AC `.four` (Fourier of an AC sweep) | `.four` is defined on a time-domain waveform; AC is already spectral. ngspice `.four` is transient-only. |
| Noise-figure / mixed S+noise (`.sp` `donoise`) | Post-V1; `.sp` ships S-parameters only. Noise metadata is a P2 ABI item. |
| Y/Z/H/ABCD parameter export | Derivable from S post-hoc; ship S-parameters, add conversions as host helpers later if asked. |
| Autonomous / large-signal (HB) distortion | `.disto` is *small-signal* Volterra (bias-point perturbation), not harmonic balance. HB is a separate post-V1 analysis. |
| Multi-dimensional port sweeps of Z0 | Z0 is fixed per port per run; sweeping Z0 is a host-loop concern. |
| `.pz` of a circuit with no reactive elements | No finite poles exist — fail loud, not a degenerate empty result. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Coordination with in-flight `solver-simplification` | Same branch `feature/bench-removal`; new analysis files added, no edits to files that refactor touched | User: other AI finished, only spec rewrites remain | y (user) |
| `.disto` scope | Full Volterra: HD2/HD3 + IM2/IM3 | User: "Full Volterra .disto now" | y (user) |
| `.four` host coverage | Both hosts, `Waveform` post-processing | User chose "Both hosts (waveform.fft)"; MD-22 uniformity | y (user) |
| `.sp` port definition | `@rfport(num, z0)` **attribute** on a node/wire (Part VI attribute-schema path); `.sp` adds `z0` termination at setup. Not a stdlib device, no `IS_PORT` capability | User 2026-07-19: "é o @rfport que decidi, não stdlib" — most Piperine-idiomatic, reuses existing attribute machinery | y (user) |
| Reactive matrix `C` extraction for `.pz` | `G` = real DC Jacobian; `C` = `Im(Y(jω₀))/ω₀` from one AC stamp at a probe frequency ω₀, valid because every stamp is linear in `jω` (`G + jωC`); fail loud if a device reports a frequency-nonlinear AC stamp | Reuses existing `load_ac`; avoids a new device ABI method | y (agent default) |
| `.pz` eigensolver | Dense QZ generalized eigenvalue on `(G, C)` via faer; circuits small enough (`.pz` is a bench/characterization analysis, not inner-loop) | Faithful and simpler than ngspice's Müller root-finding; poles = eig(−G, C), zeros = eig of the input↔output-swapped pencil | y (agent default) |
| `.four` non-uniform grid handling | Resample the last full fundamental period(s) onto a uniform grid via `Waveform.at` interpolation before the DFT | TR-BDF2 output is non-uniform; matches ngspice `.four` interpolation | y (agent default) |
| `.four` default harmonic count | 10 (DC + 9 harmonics), THD over harmonics ≥ 2 | ngspice default | y (agent default) |
| `.disto` device-derivative source | Symbolic: repeated `diff.rs` (2nd, 3rd order) emitted as JIT kernels, per fail-loud policy | Faithful; numeric perturbation would violate the "never silently approximate" convention | y (agent default) |

**Open questions:** none — `.sp` port syntax resolved to the `@rfport`
attribute (user 2026-07-19). All items resolved or logged above.

---

## User Stories

### P1: `.four` Fourier post-processing ⭐ MVP

**User Story**: As a designer, I want the Fourier spectrum (DC + harmonic
magnitudes/phases and THD) of a transient output, on both Python and Rust,
so that I can quantify harmonic content and distortion of a waveform.

**Why P1**: Highest value / lowest cost; pure post-processing on an existing
result; no solver or frontend change; proves the MD-22 uniform-surface shape
for the whole feature.

**Acceptance Criteria**:

1. WHEN a user calls `waveform.fourier(f0, n_harmonics)` (default
   `n_harmonics = 10`) on a transient result THEN the system SHALL return, for
   `k = 0..n_harmonics-1`, the frequency `k·f0`, complex magnitude, phase,
   normalized magnitude (relative to the fundamental), and normalized phase.
2. WHEN the fundamental and its harmonics are computed THEN the system SHALL
   return THD = `sqrt(Σ_{k≥2} mag_k²) / mag_1`, matching a numpy-FFT
   reference on a synthesized multi-tone signal to ≤ 1e-6 relative.
3. WHEN the transient samples are non-uniform (TR-BDF2 adaptive grid) THEN the
   system SHALL resample the last full period `[t_end − 1/f0, t_end]` onto a
   uniform grid (via waveform interpolation) before the DFT.
4. WHEN `f0 ≤ 0`, or the transient span is shorter than one fundamental
   period, or `n_harmonics < 2` THEN the system SHALL fail loud with a typed
   error, never return partial/zero spectra.
5. WHEN the same waveform is analyzed on the Rust and Python hosts THEN both
   SHALL return identical values (same call shape, same result fields;
   MD-22).

**Independent Test**: Feed a synthesized `1·sin(2πf0 t) + 0.1·sin(2π·3f0 t)`
waveform; assert HD3 ≈ 0.1, THD ≈ 0.1, DC ≈ 0; cross-check against numpy FFT.

---

### P1: `.pz` pole-zero analysis ⭐ MVP

**User Story**: As a designer, I want the poles and zeros of a circuit's
small-signal input→output transfer function so that I can reason about
stability and bandwidth.

**Why P1**: Core characterization analysis; builds directly on the DC point
and the existing AC stamps; feeds the optimizer/stability story.

**Acceptance Criteria**:

1. WHEN a user requests `.pz` with an input source and an output node/pair
   THEN the system SHALL compute the DC operating point, extract the real
   `G` and reactive `C` MNA matrices, and return the poles as the generalized
   eigenvalues of the `(G, C)` pencil (complex `s`, rad/s).
2. WHEN zeros are requested THEN the system SHALL return the zeros of the
   input→output transfer function via the input↔output-swapped pencil.
3. WHEN a computed pole/zero has a near-zero imaginary part (below tolerance)
   THEN the system SHALL report it as a real root; complex roots SHALL appear
   in conjugate pairs.
4. WHEN a single-pole RC (`R·C`) is analyzed THEN the system SHALL return one
   real pole at `s = −1/(RC)` within ≤ 1e-6 relative; an RLC SHALL return the
   expected complex-conjugate pair.
5. WHEN the circuit has no reactive elements (no `C` contribution) THEN the
   system SHALL fail loud (no finite poles), not return an empty success.
6. WHEN a device reports a frequency-nonlinear AC stamp (not `G + jωC`) THEN
   the system SHALL fail loud rather than silently mis-extract `C`.
7. WHEN run on Rust and Python THEN both hosts SHALL expose the same call
   shape and return the same poles/zeros (MD-22).

**Independent Test**: RC low-pass with `R=1k`, `C=1µ` → one pole at
`−1000 rad/s`; series RLC → conjugate pair at the analytic
`−R/2L ± j·sqrt(1/LC − (R/2L)²)`.

---

### P2: `.sp` S-parameter analysis

**User Story**: As an RF-minded designer, I want N-port scattering parameters
over a frequency sweep so that I can characterize matching networks, filters,
and passives.

**Why P2**: High value but gated on a frontend port-primitive whose syntax
must be agreed first (user). Ships after `.four`/`.pz`.

**Acceptance Criteria**:

1. WHEN ports are declared (each: node pair + reference impedance `z0`,
   default 50 Ω + port number) following the agreed PHDL port syntax THEN the
   `.sp` analysis SHALL recognize them as the network ports.
2. WHEN `.sp` runs over a frequency sweep THEN for each frequency the system
   SHALL excite one port at a time (all others matched-terminated) and compute
   the full `S` matrix `S_ij = b_i/a_j`, using power-wave normalization with
   real `z0`.
3. WHEN a reciprocal passive network (e.g. an L-C matching section) is
   analyzed THEN `S_12 == S_21` within tolerance and `|S_ii| ≤ 1`.
4. WHEN a matched resistive divider / attenuator with known S is analyzed THEN
   the computed S-matrix SHALL match the analytic values to ≤ 1e-6.
5. WHEN fewer than one port, or a port with non-positive `z0`, or a port
   referencing an unknown node is declared THEN the system SHALL fail loud.
6. WHEN run on Rust and Python THEN both hosts SHALL expose the same call
   shape and S-matrix result type (MD-22).

**Independent Test**: A 50 Ω through-line / series-R attenuator with analytic
S; a shunt-C low-pass and check `S_21` roll-off vs the closed form.

---

### P2: `.disto` Volterra distortion analysis

**User Story**: As an analog designer, I want small-signal harmonic (HD2/HD3)
and intermodulation (IM2/IM3) distortion so that I can size devices for
linearity.

**Why P2**: Highest cost (needs symbolic 2nd/3rd-order device derivatives
emitted as new JIT kernels). Lands last; phased in Design/Tasks.

**Acceptance Criteria**:

1. WHEN `.disto` runs single-tone at `F1` THEN the system SHALL compute the
   linear response, the 2nd-order response at `2·F1` from nonlinear currents
   built from 2nd derivatives and first-order responses, and the 3rd-order
   response at `3·F1`, returning HD2 and HD3.
2. WHEN `.disto` runs two-tone (`F1`, `F2` with `skw2`/`refpow` per the
   ngspice convention) THEN the system SHALL compute the intermodulation
   products (`F1±F2`, `2F1±F2`, `2F2±F1`) and return IM2/IM3.
3. WHEN a device's nonlinear contribution is lowered THEN its 2nd- and
   3rd-order derivatives SHALL be produced by symbolic differentiation
   (`diff.rs` applied repeatedly) and emitted as JIT kernels — never numeric
   perturbation, never a silent `0`.
4. WHEN a device in the circuit has a nonlinearity whose higher derivative
   cannot be lowered THEN the system SHALL fail loud
   (`CodegenError::Unsupported` / `SolverDomain`), naming the device.
5. WHEN a single nonlinear stage with a known cubic transfer
   (e.g. `i = g1·v + g2·v² + g3·v³`) is analyzed THEN HD2 and HD3 SHALL match
   the closed-form Volterra prediction
   (`HD2 = ½·(g2/g1)·(A/…)`, `HD3 = ¼·(g3/g1)·A²…`) to ≤ 1e-3 relative.
6. WHEN run on Rust and Python THEN both hosts SHALL expose the same call
   shape and distortion result type (MD-22).

**Independent Test**: A polynomial voltage-controlled current source with
known `g1,g2,g3` biased at a chosen point → HD2/HD3 vs the analytic
weakly-nonlinear formulas.

---

## Edge Cases

- WHEN `.four` fundamental period exceeds the transient span → fail loud.
- WHEN `.pz` eigensolver returns infinite / NaN generalized eigenvalues
  (singular `C`, i.e. fewer dynamic states than nodes) → filter the
  infinities (they are "poles at infinity"), report only finite poles; if
  none finite → fail loud.
- WHEN `.sp` port nodes coincide (degenerate zero-length port) → fail loud.
- WHEN `.disto` bias point does not converge → surface the DC error, do not
  attempt distortion.
- WHEN any analysis is requested on a purely digital circuit (no analog
  unknowns) → fail loud with a clear "no analog network" message.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| FOUR-01 | P1 `.four` | T1 | Done |
| FOUR-02 | P1 `.four` | T1 | Done |
| FOUR-03 | P1 `.four` | T1 | Done |
| FOUR-04 | P1 `.four` | T1 | Done |
| FOUR-05 | P1 `.four` | T2 | Done |
| PZ-01 | P1 `.pz` | T4 | Done |
| PZ-02 | P1 `.pz` | T5 | Done |
| PZ-03 | P1 `.pz` | T4/T5 | Done |
| PZ-04 | P1 `.pz` | T4 | Done |
| PZ-05 | P1 `.pz` | T4 | Done |
| PZ-06 | P1 `.pz` | T3 | Done |
| PZ-07 | P1 `.pz` | T6 | Done |
| SP-01 | P2 `.sp` | T7 | Done |
| SP-02 | P2 `.sp` | T8 | Done |
| SP-03 | P2 `.sp` | T8 | Done |
| SP-04 | P2 `.sp` | T9 | Done |
| SP-05 | P2 `.sp` | T7/T8 | Done |
| SP-06 | P2 `.sp` | T9 | Done |
| DISTO-01 | P2 `.disto` | T11/T12 | Done |
| DISTO-02 | P2 `.disto` | T13 | Done |
| DISTO-03 | P2 `.disto` | T10 | Done |
| DISTO-04 | P2 `.disto` | T10 | Done |
| DISTO-05 | P2 `.disto` | T12 | Done |
| DISTO-06 | P2 `.disto` | T14 | Done |

**ID format:** `[ANALYSIS]-[NUMBER]`

**Coverage:** 24 total, 24 mapped to tasks (T1–T14, all committed).

---

## Success Criteria

- [x] All four analyses callable on both hosts with the same shape (MD-22).
- [x] Each analysis validated against a closed-form reference (RC/RLC pole,
      cubic-stage HD2/HD3, analytic attenuator S, synthesized-tone THD) to the
      tolerances above.
- [x] Zero silent approximations: every unlowerable path fails loud.
- [x] Each analysis has a documented algorithm block (module `//!` +
      `design.md`).
- [x] `cargo test --workspace` green; ngspice cross-checks where an ngspice
      reference exists (`.four`, `.pz`, `.disto`; `.sp` has no ngspice
      reference — skipped per Out of Scope).
</content>
</invoke>

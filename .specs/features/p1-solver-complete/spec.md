# p1-solver-complete Specification

Close ROADMAP pillar **P1 — Solver complete**: the remaining analyses
(`.sens`, PSS, `.dc` host-proof), engine operator gaps (`table`,
`transition`, `idt` AC, multiple `ac_stim`, `@initial` branch force), digital
network JIT integration, the missing SPICE models (MOS 2/3, transmission
lines, transformer, `T?` migration), convergence/integration parity
(fetlim/limvds, UIC hold, TR dual, `IntegrationMethod` removal, temperature),
and the engine-hygiene leftovers.

**Sequencing:** executes AFTER `api-crate` lands (host-facing surfaces —
sens/PSS results — are born in `piperine-api`).

## Problem Statement

V1 requires "every analysis a working SPICE user expects, plus PSS". The
solver core (TR-BDF2, breakpoints, homotopy, bypass) is done; what remains is
a long tail of analyses, operators, and models that each block a class of
real circuits. `.sens` additionally feeds the P6 optimizer.

## Goals

- [ ] `.sens` (DC sensitivity) and PSS ship as native analyses with Python
      surface and validation against reference values.
- [ ] Host-level `.dc` proven equivalent (nested + source sweeps) — item
      closed without a solver-side analysis.
- [ ] All listed operator gaps closed (or explicitly deferred: `laplace_*`,
      `zi_*` stay fail-loud).
- [ ] Fused digital-network JIT active in real circuits.
- [ ] Model set: MOS 2/3, lossless tline, urc, transformer block; stdlib off
      sentinel params.
- [ ] fetlim/limvds real; UIC hold enforced; temperature flow uniform;
      vestigial `IntegrationMethod` gone; hygiene leftovers cleared.

## Out of Scope

| Feature | Reason |
|---------|--------|
| BSIM-class models | Separate epic (user 2026-07-18) — hand-ported PHDL, level by level |
| `laplace_*`, `zi_*` operators | Stay fail-loud (user 2026-07-18); language backlog |
| Native `.dc` solver analysis | Host-level restamp proven sufficient (user 2026-07-18) |
| `.pz`, `.disto`, `.sp` | Niche, post-V1 |
| LTRA (lossy tline, full convolution) | urc covers the practical lossy case; LTRA logged as backlog |
| AC sensitivity | DC first; AC `.sens` follow-up if the optimizer needs it |
| Optimizer itself | P6, user still studying shape |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| `.sens` method | Finite-difference stamp perturbation + one extra linear solve per (element,param) reusing the run's LU — `A·(dx/dp) = −(∂R/∂p)` | No new codegen; exact symbolic `∂R/∂p` is a later upgrade behind the same API | n (agent) |
| `.sens` param set | Caller lists `(label, param)` pairs explicitly; params whose `Invalidation` is `Rebuild` → loud error | Fail loud beats silently wrong sensitivities | n (agent) |
| PSS method | Single shooting: transient over one period from full state `x₀` (analog + digital nets + hidden states), Newton on `g(x₀)=x(T)−x₀` (continuous vars only), Jacobian FD/Broyden; optional `tstab` pre-roll. Digital periodicity is a post-convergence verification (loud error, with a "period appears to be k·T" diagnostic for dividers). Inefficient but sufficient for V1; time-domain collocation logged as analog-only alternative backend (backlog) | Every shot is an ordinary transient → mixed signal works unchanged; collocation cannot stamp discrete digital events | y (user, 2026-07-18) |
| PSS drive | Period `T` supplied by the user (driven circuits); autonomous-oscillator period detection out of scope this round | Autonomous PSS needs phase conditions — real research; log backlog | n (agent) |
| `table` modes | Spec Part V §2 signature; implement 1-D linear interpolation with end-point clamp; non-interpolating "closest" mode if Part V defines it | Match the spec text at implementation time — the task reads Part V first | n (agent) |
| `transition` semantics | Verilog-AMS: output walks to the new target over rise/fall time from the moment of change; state = (start value, target, t_change); breakpoint declared at ramp ends | Standard semantics; reuses the runtime-operator state machinery (`delay`/`slew`) and `next_breakpoints` | n (agent) |
| `@initial` branch force | Compile `V(a,b) <- ic` in `@initial` into the existing t=0 initial-condition path (extends `FlatAnalog.initial_conditions` to branch constraints) + the UIC hold clamp below | One mechanism for both seed and hold | n (agent) |
| tline model | Ideal lossless line in PHDL over the existing `delay` runtime operator (Branin/method-of-characteristics: two controlled sources + delayed cross-terms) | No solver change needed; validated vs ngspice `tra` golden | n (agent) |
| urc model | Lumped RC ladder expansion (N segments, geometric per ngspice) as a PHDL module with `param n` | Matches ngspice urc semantics without convolution | n (agent) |
| Transformer | One combined device `xfmr` (L1, L2, k) in `headers/spice/passives.phdl` using the mutual-flux engine | Engine constraint: both windings must be one device (documented 2026-07-12) | y (documented) |
| fetlim/limvds | Port ngspice `DEVfetlim`/`DEVlimvds` formulas into `emit_analog_limit` (same slot machinery as pnjlim) | Parity; machinery exists | n (agent) |
| Temperature | Verify `tnom` rescaling per model + add a host-level `.temp` sweep test (host restamps `temp`); no new solver analysis | Analysis-level sweep is host-side by design | n (agent) |
| Digital clocked fusing | Comb-cone integration first (SC-13); clocked-member fusing lands only if the comb integration leaves the scheduler seam clean — else logged follow-up | Comb is the proven scaffold; clocked touches NBA semantics | n (agent) |
| Baseline | Whatever `api-crate` closes at (≥449 passed / 5 ignored) | Sequencing | y |

**Open questions:** none — all resolved or logged above.

**Implicit-dimension sweep (Large):** input validation → SC-02/SC-19 (bad
param/net names, non-monotonic table axes are loud errors); failure states →
every non-convergence is a named `SolverDomain` error (PSS: `Pss` domain);
idempotency N/A (pure functions of design+config); auth N/A; concurrency N/A
(single-threaded solve, unchanged); data lifecycle N/A; observability →
existing `SolverStats` extended to PSS/sens (iterations, residual);
external-dependency failure → ngspice harness skips when binary absent
(existing pattern); state-transition integrity → PSS/`transition`/UIC states
covered by their ACs.

## User Stories

### P1: Sensitivity analysis (`.sens`) ⭐ MVP

**User Story**: As a designer (and as the future optimizer), I want
d(output)/d(param) at the operating point so I can center designs without
finite-differencing whole simulations by hand.

**Acceptance Criteria**:

1. WHEN `run_sens(outs, params, config)` runs on a solved DC point THEN it
   SHALL return ∂V(out)/∂p for every requested `(label, param)` pair,
   matching a two-sided finite-difference reference (`(V(p+h)−V(p−h))/2h`,
   relative tolerance 1e-3) on a voltage divider and a diode-loaded divider.
2. WHEN a requested param's invalidation class is `Rebuild` (or the param
   does not exist) THEN the analysis SHALL fail loud naming the param.
3. WHEN the Python host calls `module.sens(...)` THEN it SHALL receive the
   same values as the Rust API (shape: map (out, label.param) → f64).

**Independent Test**: divider `R1/R2`: ∂V(mid)/∂R2 analytic
`V·R1/(R1+R2)²` matched to 1e-6 relative.

### P1: Periodic steady state (PSS) ⭐ MVP

**Acceptance Criteria**:

1. WHEN `run_pss(period, config)` runs on a driven RC (sine source, τ ≫ T)
   THEN the returned single-period trace SHALL satisfy
   `|x(T) − x(0)| < shoot_tol` (default 1e-6; the adaptive integrator's
   per-period reproducibility floor is ~1e-7 — spec-precision corrected
   2026-07-18, abstol-tight bounds spin at the noise floor) per state and
   match the analytic steady-state phasor amplitude within 1 % (vs the transient-until-settled reference).
2. WHEN shooting Newton fails to converge within its iteration cap THEN the
   analysis SHALL fail loud (`SolverDomain::Pss`, iterations + final
   residual in the message) — never return a non-periodic trace.
3. WHEN `tstab > 0` is given THEN the shooting SHALL start from the state at
   `t = tstab` (pre-roll transient), and the result SHALL be identical
   (within tolerance) to `tstab = 0` on circuits where both converge.
4. WHEN a full-wave rectifier + RC filter runs under PSS THEN the ripple
   waveform SHALL match a long settled transient within `10·reltol`.
5. WHEN a mixed-signal circuit converges in the analog residual but the
   digital state (nets + hidden states) at `T` differs from the state at `0`
   THEN the analysis SHALL fail loud; WHEN the digital state closes after
   `k ≤ 4` periods (divider case) THEN the error SHALL name "circuit period
   appears to be k·T".

**Independent Test**: driven RC case (AC-1) — closed-form comparison.

### P1: `.dc` host-proof ⭐ MVP

**Acceptance Criteria**:

1. WHEN a nested two-param sweep (outer `vs.dc`, inner `r.r`) runs via the
   host restamp path THEN every point SHALL equal an independent fresh-build
   solve of the same values (exact equality of solved voltages) and the
   compile count SHALL be 1.
2. WHEN a *source* value is swept THEN the same equality SHALL hold
   (sources restamp like any param — proven, not assumed).

**Independent Test**: extends `compile_once_sweep.rs`.

### P1: Operator completeness ⭐ MVP

**Acceptance Criteria**:

1. WHEN `table(x, xs, ys)` is called in an analog body THEN it SHALL resolve
   (registered operator), interpolate linearly with end clamp, and its
   Jacobian SHALL be the segment slope; non-monotonic `xs` SHALL be a loud
   elaboration/codegen error.
2. WHEN `transition(expr, td, rise, fall)` drives a contribution THEN the
   output SHALL ramp linearly from the pre-change value to the target over
   rise/fall, declare breakpoints at ramp start/end, and a step input into
   an RC SHALL show the ramped edge (not an instantaneous jump) in the
   trace.
3. WHEN `idt(x)` appears in a contribution under AC THEN the stamp SHALL be
   `X/(jω)` (validated: an idt-based integrator shows −20 dB/dec and −90°
   phase across 4 decades).
4. WHEN a contribution carries two or more `ac_stim` terms THEN AC SHALL sum
   them (magnitude/phase superposition validated against the equivalent
   two-source circuit).
5. WHEN `@initial { V(a,b) <- ic; }` targets a branch THEN t=0 SHALL start
   from `ic` exactly (no longer a loud error), and WHEN UIC semantics are
   requested THEN the value SHALL hold through the first solve (clamp
   released after t=0): cap pre-charged to 5 V discharging through R matches
   `5·e^(−t/RC)` within `10·reltol`.

**Independent Test**: per-operator kernel tests in `analog_jit.rs` +
circuit-level cases in `spec_simulation.rs`.

### P1: Fused digital network active ⭐ MVP

**Acceptance Criteria**:

1. WHEN a circuit contains a pure-combinational digital cone THEN
   `CircuitCompiler`/`CircuitInstance` SHALL evaluate it through
   `DigitalNetwork` (one fused JIT call), with per-device fallback for
   clocked/analog-sampling members — proven by an instrumentation counter or
   capability assertion in the test, not by timing.
2. WHEN the existing digital suites run (exhaustive adder/mux/multiplier/
   comparator examples, cross-module NBA test) THEN results SHALL be
   bit-identical to the per-device path.

**Independent Test**: `digital_topology.rs` + examples 17–20 green with
fusion active.

### P1: Model set expanded ⭐ MVP

**Acceptance Criteria**:

1. WHEN MOS level 2 and level 3 devices run the existing MOS validation
   topologies THEN DC operating points SHALL match ngspice within the
   harness tolerances (new golden cases, live when ngspice present).
2. WHEN an ideal lossless tline (Z0, td) terminates in its characteristic
   impedance THEN a pulse SHALL arrive delayed by `td` with no reflection
   (< 1 % residual); WHEN open-terminated THEN the reflected doubling SHALL
   appear at `2·td`.
3. WHEN `urc(n)` bridges a step source and a load THEN the delay/rise SHALL
   match ngspice's urc golden within harness tolerance.
4. WHEN `xfmr(l1, l2, k)` couples two loops THEN voltage ratio ≈
   `k·√(L2/L1)` in AC (validated on an ideal-ish k=0.999 case) and the
   coupled-LC energy-transfer test SHALL pass.
5. WHEN the stdlib is grepped THEN no model SHALL use sentinel defaults
   (`1e99`, `$param_given`) where `T?`/`.get_or` expresses the same —
   behavior unchanged (existing model suites green).

**Independent Test**: new `tests/ngspice/` circuits + goldens per model.

### P2: Convergence & integration parity

**Acceptance Criteria**:

1. WHEN `$limit("fetlim", …)`/`$limit("limvds", …)` are lowered THEN they
   SHALL implement the ngspice `DEVfetlim`/`DEVlimvds` formulas (unit-tested
   against reference values from the C source), and MOS validation stays
   green.
2. WHEN `Context` temperature is set per-analysis THEN every stdlib model's
   `tnom` rescaling SHALL flow consistently (host-level `.temp` sweep test:
   diode forward drop shifts ≈ −2 mV/°C).
3. WHEN the TR stage of TR-BDF2 processes an inductor flux THEN the
   previous-voltage dual form SHALL be used (regression: coupled-LC and RL
   corner cases unchanged or tighter).
4. WHEN the workspace is grepped THEN `IntegrationMethod` SHALL be gone
   (TR-BDF2 implicit); `suggest_transient_step` loses its `method` param;
   all suites green.

### P3: Engine hygiene

**Acceptance Criteria**:

1. WHEN `digital/scheduler.rs` is split into `topology/state/scheduler`
   modules and `SignalBridge` is extracted from
   `CircuitInstance::accept_and_run_digital` THEN behavior SHALL be
   unchanged (full suite green; no public-surface change beyond module
   paths).
2. WHEN `DcAnalysisResult::as_iv` is re-homed and noise integration goes
   through a shared `Integrator` THEN call sites SHALL compile against the
   new signatures with identical numeric results.
3. WHEN `Trace.i` is requested on a device with runtime state/vars THEN
   (opt-in recording enabled) it SHALL return the recomputed current; with
   recording off it SHALL keep the current loud error.
4. WHEN `Context::default()` is constructed THEN it SHALL NOT trigger global
   init; the first solver build SHALL.

## Edge Cases

- WHEN `.sens` is asked for a ground/non-addressable output net THEN loud
  `Measurement` error (existing pattern).
- WHEN PSS `period <= 0` or `tstab < 0` THEN loud options-validation error.
- WHEN `table` receives `xs`/`ys` of different lengths THEN loud error.
- WHEN `transition` rise/fall = 0 THEN behaves as an instantaneous step with
  a declared breakpoint (no divide-by-zero).
- WHEN tline `td` ≤ 0 or `Z0` ≤ 0 THEN loud model parameter error.
- WHEN ngspice is absent THEN new golden cases SKIP with notice (existing
  harness pattern), never silently pass.

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| SC-01 | P1 sens values vs FD reference | - | Pending |
| SC-02 | P1 sens loud errors | - | Pending |
| SC-03 | P1 sens python surface | - | Pending |
| SC-04 | P1 PSS converged periodic trace | - | Pending |
| SC-05 | P1 PSS loud non-convergence | - | Pending |
| SC-06 | P1 PSS tstab + rectifier case | - | Pending |
| SC-07 | P1 .dc host-proof (nested + source) | - | Pending |
| SC-08 | P1 table operator | - | Pending |
| SC-09 | P1 transition operator | - | Pending |
| SC-10 | P1 idt AC stamp | - | Pending |
| SC-11 | P1 multiple ac_stim | - | Pending |
| SC-12 | P1 @initial branch force + UIC hold | - | Pending |
| SC-13 | P1 digital network fused + identical results | - | Pending |
| SC-14 | P1 MOS2 + MOS3 ngspice parity | - | Pending |
| SC-15 | P1 tline ideal | - | Pending |
| SC-16 | P1 urc | - | Pending |
| SC-17 | P1 transformer block | - | Pending |
| SC-18 | P1 stdlib off sentinels (T?) | - | Pending |
| SC-19 | P2 fetlim/limvds | T19 | Done |
| SC-20 | P2 temperature uniform + .temp sweep | T20 | Done |
| SC-21 | P2 inductor TR dual | T21 | Done |
| SC-22 | P2 IntegrationMethod removal | T22 | Done |
| SC-23 | P3 scheduler split + SignalBridge | T23+T24 | Done |
| SC-24 | P3 as_iv + Integrator re-home | T25 | Done |
| SC-25 | P3 Trace.i state recording (opt-in) | - | Pending |
| SC-26 | P3 init_global ownership | T25 | Done |

**Coverage:** 26 total, 0 mapped to tasks (mapping happens in tasks.md).

## Success Criteria

- [ ] ROADMAP P1 section: every checkbox either checked or explicitly moved
      to a named backlog line (laplace/zi, LTRA, autonomous PSS, AC sens).
- [ ] `cargo test --workspace` green, zero warnings; ngspice harness live
      with the new golden cases.
- [ ] `.sens` + PSS callable from Python with docstrings + part VIII docs.

# Periodic Small-Signal Specification

Periodic small-signal (PAC), periodic transfer function (PXF) and periodic noise (PNoise), linearized around the periodic steady state that `host-ii` brings.

## Open questions for the user

Nothing below is decided. Each question has options and the planner's recommendation.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | Which periodic analyses, in what order? | **A.** PAC and PNoise as P1, PXF as P2. **B.** PAC only; PNoise and PXF later. **C.** A, plus periodic S-parameters | **A.** PNoise is the reason a designer of a sampled or switched circuit wants this at all; PXF is PAC's adjoint and costs little more. Periodic S-parameters wait for `host-iii`'s S-parameters (H2) |
| Q2 | How is a result proven when ngspice has no PAC, PXF or PNoise? | **A.** Three oracles: (1) an LTI reduction, where PAC sideband 0 must equal ngspice `.ac` and PNoise must equal ngspice `.noise`; (2) a transient-perturbation cross-check in ngspice: the switched twin is run with a small sine of amplitude ε added, in `.tran`, and the Fourier components at `f + k·f0`, divided by ε, must equal the PAC sidebands; (3) closed forms: the zero-order hold of `SampleHold`, an ideal switching mixer's conversion gains, a PWM modulator's gain, sampled `kT/C` noise. **B.** Closed forms only. **C.** A, plus the same transient perturbation run on Piperine's own transient for mixed designs ngspice cannot express | **C.** (1) and (3) are exact; (2) ties the switched case to an independent simulator; Piperine's own perturbed transient is the only oracle for designs with `D2A`/`A2D` and reactions |
| Q3 | Which run keys? | **A.** `analysis = "pac"`, `"pxf"`, `"pnoise"`, each with the `pss` keys (`period`, `tstab`), `sidebands = K`, and the `ac` sweep keys for the input frequency; `pxf` and `pnoise` add `output`. Each key has its flag (MD-65). **B.** One `analysis = "pss"` with a nested small-signal table | **A.** It follows the existing pattern: one analysis per run, flat keys |
| Q4 | How are sidebands numbered? | **A.** An input at `f` reaches the output at `f + k·f0` for `k` in `-K..=K`; sideband `k` is that output (the usual convention of PAC tools). **B.** Output frequency fixed, input sidebands enumerated (the PXF view only) | **A** for PAC and PNoise, **B** for PXF, which is the adjoint view by definition |
| Q5 | Spec text: G.36 says the table of §11.1 does not state that a periodic analysis is `Transient` for its trajectory and `Ac`/`Noise` for its linearization. Amend it? | **A.** Amend §11.1's table with that exception and close G.36. **B.** Leave G.36 open | **A.** The behaviour is already normative in §16.3; the table should not contradict it |

## Problem Statement

Part V §16.3 defines periodic small-signal and periodic noise as host analyses: a periodic steady state, then the small-signal rule (§11.1.4) at each point of it, with real carriers carrying the perturbation and event times moving (§23.3 rule 4h, §11.1.7). Appendix C.2 says the linearization is not implemented, and G.37 says no example traces one (`GAPS.md` V16). `host-ii` brings `analysis = "pss"` (HII-07), so the orbit exists, but a designer of a sampled, switched or clocked circuit cannot ask for its transfer or its noise. Ordinary `ac` and `noise` see a sampled block as constant (§11.1.5) and give a wrong answer for it with no warning.

## Goals

- [ ] `analysis = "pac"` gives, for each input frequency and each sideband up to `K`, the response of every printed signal, linearized around the `pss` orbit.
- [ ] `analysis = "pnoise"` gives the output noise density of a periodically operated circuit, folded over `K` sidebands.
- [ ] `analysis = "pxf"` gives the transfer from every source to one output.
- [ ] Each analysis is proven against ngspice where ngspice can express the case and against closed forms where it cannot (Q2), and `SampleHold`'s zero-order hold comes out with nothing written in the model (§11.1.7).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Periodic steady state itself | `host-ii` (HII-07) |
| The `noise` primitive and the `Noise` analysis (L45, H2's noise row) | `analog-operators`; PNoise needs them |
| Autonomous-oscillator PSS and phase noise | `ROADMAP.md` backlog: needs period detection with phase conditions |
| Harmonic balance, quasi-periodic analyses (QPSS, QPAC) | Not in the spec |
| Periodic S-parameters and periodic stability | Q1; `host-iii` owns S-parameters |
| Gradients and `∂(record)/∂p` (H8) | `host-iii`. It shares §23.3 rule 4 with this feature (Dependencies) |
| Transient-noise Monte Carlo as an oracle | Statistical; the closed forms and ngspice `.noise` are exact |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The orbit | the `pss` run's own: same `period`, `tstab`, start `t0` and steps (HII-07) | one orbit per run; §16.3 "a periodic steady state, then the small-signal rule" | n - planner |
| The stimulus of PAC | the design's `ac_stim` phasors, as in `ac` (§16.2: `ac_stim` is the phasor in `Ac`) | The model sees `Ac` for the linearization (§16.3) | n - planner |
| What the model sees | `analysis(Transient)` along the trajectory; `analysis(Ac)` in PAC and PXF, `analysis(Noise)` in PNoise, during the linearization; `now()` the point's time (§16.3 rules 1 to 3) | §16.3's table | n - planner |
| A real carrier and a moving event | carry the perturbation and move under §23.3 rules 2 and 4a to 4h | the spec's own rule; it gives `SampleHold` its zero-order hold | n - planner |
| New tolerance constants | `PAC_MAGNITUDE` and `PAC_PHASE` for the transient-perturbation cross-check, each with a rationale (finite ε, Fourier leakage over a whole common period); the LTI reductions reuse `AC_MAGNITUDE`, `AC_PHASE` and the noise constant `analog-operators` adds | MD-38: each domain its documented bound; no existing constant moves | n - planner |
| Closed-form tolerances | relative 1e-3 on magnitudes and 1e-3 rad on phases, written once in the contract with their rationale (the integrator's error at the run's `max_step`) | MD-38 | n - planner |
| Perturbation amplitude ε of the cross-check | 1e-4 of the signal's full scale, the tone commensurate with `f0` (`f = (m/N)·f0`) so that both tones share a period of `N/f0` | small enough to stay linear, large enough above the solver's `reltol` | n - planner |
| Sideband count | `sidebands = K`, default 0 when absent; IF negative or not an integer THEN `P0010` | sideband 0 alone is a valid PAC | n - planner |
| Spec text | G.36 amended (Q5 A), G.37 closed with a traced example of `SampleHold` under PAC | awaiting Q5 | n - planner |

**Open questions:** Q1 to Q5 at the top of this file are open; the rows above follow the recommendations until the user answers.

**Implicit-requirement dimensions.** Input validation: PER-08 AC 1 to 3 (keys). Failure states: PER-08 AC 4 to 6 (the orbit did not converge, a grazing event, an irreversible reaction). Idempotency: N/A because a run is one run. Auth and rate limits: N/A, local tool. Concurrency: N/A, no new concurrency. Data lifecycle: N/A because results are a table, as `ac`'s. Observability: PER-01 AC 4 (the orbit's statistics printed). External dependency: PER-03 AC 3 (ngspice absent: the harness's skip line). State transitions: PER-07 (the flags a model reads in each phase).

---

## User Stories

### P1: PAC around the orbit ⭐ MVP

**User Story**: As a designer of a switched or sampled circuit, I want its small-signal response around its periodic operation, per sideband, so that I can read a mixer's conversion gain or a sampler's transfer.

**Why P1**: The gap's core; PNoise and PXF are built on it.

**PER-01** (the run)

**Acceptance Criteria**:
1. WHEN a run gives `analysis = "pac"` with `period`, the `ac` sweep keys and `sidebands = K` THEN the system SHALL first converge the orbit as `pss` does (HII-07) and then give a table with one row per input frequency `f` of the sweep and sideband `k` in `-K..=K`.
2. WHEN that run completes THEN each row SHALL hold `f`, `k`, the output frequency `f + k·f0` and, for each printed signal, its response at that output frequency to the design's `ac_stim` phasors, in the form `ac` gives its cells.
3. WHEN the run gives `plot` THEN the system SHALL draw one magnitude curve per printed signal and sideband against `f`.
4. WHEN the run completes THEN the system SHALL print the orbit's shooting iterations and residual, as `pss` does.

**PER-02** (the LTI reduction)

**Acceptance Criteria**:
1. WHEN `ac_rc` with a square-wave drive added in series with its source runs `pac` with `K = 2` THEN sideband 0 SHALL agree with ngspice `.ac` of `ac_rc.cir` under `AC_MAGNITUDE` and `AC_PHASE`.
2. WHEN that run completes THEN every other sideband's magnitude SHALL be at most 1e-6 of sideband 0's at the same `f`.

**PER-03** (a switched circuit against ngspice)

**Acceptance Criteria**:
1. WHEN the twin `pac_mixer` (a resistor switched by a 50 % clock of frequency `f0` in series with a resistive load, `.msl` and `.cir`) runs `pac` with `K = 3` THEN each sideband's response SHALL agree, under `PAC_MAGNITUDE` and `PAC_PHASE`, with the Fourier component at `f + k·f0` of ngspice's `.tran` of the twin with a sine of amplitude ε at `f` added, divided by ε.
2. WHEN that run completes THEN sideband `k`'s response SHALL equal the `k`-th Fourier coefficient, over one period, of the divider ratio `R_load / (R_load + R_switch(t))`, within the closed-form tolerance.
3. WHERE the ngspice comparison is disabled the test SHALL still run the twin and print the harness's skip line.

**PER-04** (a sampled circuit: §11.1.7)

**Acceptance Criteria**:
1. WHEN `SampleHold` (§12.1) clocked at period `T` runs `pac` with `K = 3` THEN sideband `k`'s magnitude at input frequency `f` SHALL equal `|sin(π·(f + k/T)·T) / (π·(f + k/T)·T)|` within the closed-form tolerance.
2. The system SHALL give that response with no line of `SampleHold` changed from §12.1.

**PER-05** (a moving event: §23.3 rule 2)

**Acceptance Criteria**:
1. WHEN a natural-sampling PWM modulator (a comparator of a control voltage against a ramp of peak-to-peak `Vpp` at `f0`, driving a `D2A` of level `Vdd`) runs `pac` at `f = f0 / 100` with the stimulus on the control voltage THEN sideband 0's magnitude at the `D2A` output SHALL be `Vdd / Vpp` within 1 %.
2. WHEN that run completes THEN sideband 0 SHALL agree with Piperine's own transient of the modulator perturbed by ε at `f`, under `PAC_MAGNITUDE` and `PAC_PHASE`.

**Independent Test**: `cargo test -p piperine-parity --test parity_pac` (PER-02, PER-03) and `cargo test -p piperine-api --test pac` (PER-04, PER-05).

---

### P1: PNoise around the orbit ⭐ MVP

**User Story**: As a designer of a sampled front end, I want its output noise with the noise folded by the sampling, so that `kT/C` and mixer noise come out right.

**Why P1**: Q1 A; ordinary `noise` gives a sampler's noise wrong with no warning.

**PER-06** (the run and its proofs)

**Acceptance Criteria**:
1. WHEN a run gives `analysis = "pnoise"` with `period`, `output`, `sidebands = K` and the sweep keys THEN the system SHALL give one row per frequency with the output noise density summed over every noise source and every sideband `-K..=K`, and one column per source group named by the `noise` primitive's name (§11.1.8).
2. WHEN `noise_rc` with a square-wave drive added runs `pnoise` THEN the density SHALL agree with ngspice `.noise` of `noise_rc.cir` under the noise constant of `analog-operators`.
3. WHEN a switch of on-resistance `R` sampling a capacitor `C` at `f0`, held by `SampleHold`, runs `pnoise` with `K = 20` THEN the integral of the held output's one-sided density over `0..(K + 1/2)·f0` SHALL be `kT/C` within 5 %.
4. WHEN the mixer of PER-03 with one white noise current source of density `S` at its input, and noiseless resistors, runs `pnoise` THEN the output density at `f` SHALL equal `S · Σ_k |H_k(f + k·f0)|²` within relative 1e-6, `H_k` being the PAC sidebands from that source's position to the output.

**Independent Test**: `cargo test -p piperine-parity --test parity_pnoise` and `cargo test -p piperine-api --test pnoise`.

---

### P2: PXF

**User Story**: As a designer, I want the transfer from every source to one output in one run, so that supply and clock feedthrough are read without one PAC per source.

**Why P2**: PAC answers the same question one source at a time.

**PER-07** (the adjoint, and the flags)

**Acceptance Criteria**:
1. WHEN a run gives `analysis = "pxf"` with `output` THEN the system SHALL give, for each input sideband `k` and each `ac_stim` and noise source, the transfer to the output at `f`.
2. WHEN the mixer of PER-03 runs `pxf` THEN each transfer SHALL equal the PAC of the same source at the same pair of frequencies within relative 1e-6.
3. WHILE the trajectory runs the model SHALL read `analysis(Transient)` true and `analysis(Ac)` and `analysis(Noise)` false.
4. WHILE the linearization of `pac` or `pxf` runs the model SHALL read `analysis(Ac)` true, and WHILE that of `pnoise` runs it SHALL read `analysis(Noise)` true.
5. WHILE the linearization runs `zi_nd` SHALL be in its `delay` form (§16.3 rule 3).

**Independent Test**: `cargo test -p piperine-api --test pxf`, and a model whose contribution depends on the flags.

---

### P2: Refusals name their cause

**User Story**: As a user, I want a periodic analysis that cannot give a number to say why.

**Why P2**: The P1 stories run on valid input; this story bounds the rest.

**PER-08** (refusals)

**Acceptance Criteria**:
1. IF a `pac`, `pxf` or `pnoise` run lacks `period` THEN the system SHALL report `P0010` naming the key.
2. IF `sidebands` is negative or not an integer THEN the system SHALL report `P0010` naming the key.
3. IF a `pxf` or `pnoise` run lacks `output`, or names a signal the design does not have THEN the system SHALL report `P0010` naming it.
4. IF the orbit does not converge THEN the system SHALL fail with the `pss` error and give no table.
5. IF a crossing on the orbit is grazing (`de/dt = 0` at its time) THEN the system SHALL report `E0807` naming the `cross` instance and its time, in place of the rows that depend on it (§23.3 rule 5).
6. IF the design has an `IRREVERSIBLE` reaction THEN the system SHALL refuse the linearization naming the instance.

**Independent Test**: one `piperine-api` test per criterion.

---

### P3: The specification says what is true

**User Story**: As a reader of the spec, I want §11.1, Appendix C.2 and Appendix G to describe the periodic analyses as they are.

**Why P3**: No behaviour depends on it.

**PER-09** (documents)

**Acceptance Criteria**:
1. WHEN this feature merges THEN §11.1's table SHALL state that a periodic analysis is `Transient` for its trajectory and `Ac` or `Noise` for its linearization, and G.36 SHALL be closed.
2. WHEN this feature merges THEN the spec SHALL hold a traced example of `SampleHold` under PAC with the values of PER-04, and G.37 SHALL be closed.
3. WHEN this feature merges THEN Appendix C.2's row "analyses the host builds" SHALL have no "not implemented" status, `GAPS.md` SHALL no longer list V16, and `ROADMAP.md` SHALL mark the feature delivered with its commit.

**Independent Test**: `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs`.

---

## Edge Cases

- WHEN `K` is 0 THEN the system SHALL give sideband 0 only.
- WHEN an input frequency `f` makes `f + k·f0` equal 0 THEN the system SHALL give that row the DC response of the sideband.
- WHEN the design has no `ac_stim` THEN `pac` SHALL give every response 0.
- WHEN the design has no noise source THEN `pnoise` SHALL give a density of 0 and name no group.
- WHEN a carrier of a non-real type changes on the orbit THEN it SHALL carry no perturbation, and only the event times it causes SHALL move (§23.3 rule 4f).

---

## Gaps closed

| `GAPS.md` id | Requirement(s) | How |
| ------------ | -------------- | --- |
| V16 | PER-01 to PER-09 | PER-01 to PER-05 give PAC and prove it four ways (LTI against ngspice `.ac`, a switched twin against ngspice's perturbed `.tran`, the closed forms of the zero-order hold and the PWM gain); PER-06 gives PNoise against ngspice `.noise` and `kT/C`; PER-07 gives PXF and the flags of §16.3; PER-08 bounds the refusals; PER-09 closes G.36 and G.37 |

---

## Dependencies

- `host-ii` (HII-07): the `pss` run and its orbit.
- `analog-operators` (L45, H2's noise row): the `noise` primitive, the `Noise` analysis and its tolerance constant, for PER-06 and PXF's noise sources.
- `msl-fixes` (L48): `analysis(Ac)`, `analysis(Noise)` and `analysis(Transient)` in MSL, for PER-07 AC 3 to 5.
- `host-iii` (H8) shares §23.3 rule 4 (a commit carries a sensitivity, event times move). Whichever lands first builds that propagation; the other reuses it. Neither blocks the other.

## Size

L: about 20 tasks. The periodic linearization on the device ABI (5), carriers and moving events (3), PAC run and table (2), PAC proofs (3), PNoise (3), PXF (2), refusals (1), spec text (1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PER-01 | P1: PAC around the orbit | - | Pending |
| PER-02 | P1: PAC around the orbit | - | Pending |
| PER-03 | P1: PAC around the orbit | - | Pending |
| PER-04 | P1: PAC around the orbit | - | Pending |
| PER-05 | P1: PAC around the orbit | - | Pending |
| PER-06 | P1: PNoise around the orbit | - | Pending |
| PER-07 | P2: PXF | - | Pending |
| PER-08 | P2: refusals name their cause | - | Pending |
| PER-09 | P3: the specification says what is true | - | Pending |

**Coverage:** 9 total, 0 mapped to tasks, 9 unmapped (spec stage).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps… Preciso que TODOS os gaps estejam endereçados".
- `GAPS.md` §6 V16; §7 H2.
- `docs/spec/part_v_execution.md` §16.2 rule 2, §16.3 (the host analyses table and rules 1 to 3); `part_iv_behavior.md` §11.1.4 to §11.1.8, §12.1 (`SampleHold`); `part_viii_host.md` §23.3 rules 2, 4, 4h, 5; `appendix_c_implementation.md` C.2; `appendix_g_scope.md` G.36, G.37.
- `.specs/features/host-ii/spec.md` HII-07 (on `feature/host-ii`): the `pss` run and its ngspice proof by a long `.tran`.
- `crates/piperine-solver/src/analyses/pss.rs`: single shooting, finite-difference then Broyden Jacobian.
- ngspice-47 (installed): `.ac`, `.noise`, `.tran`; no PAC, PXF or PNoise.

---

## Success Criteria

- [ ] `piperine run` runs `pac`, `pnoise` and `pxf`.
- [ ] PAC sideband 0 of an LTI circuit equals ngspice `.ac`; the switched twin equals ngspice's perturbed transient; `SampleHold` gives its zero-order hold.
- [ ] PNoise equals ngspice `.noise` on an LTI circuit and `kT/C` on a sampler.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

# buck-scenarios: diagnosis of the six red scenario tests

**HEAD diagnosed**: 14496800 (feature/buck-scenarios). **Date**: 2026-11-20. Author: diagnosis agent (user AFK,
"assume o default mais razoável e depois me conta"). Nothing in product code or tests was changed; the probes
below are scratch and are not committed.

## Verdicts

| Check | Test | Measured | Cause | Verdict |
| --- | --- | --- | --- | --- |
| C22 | `s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms` | 1.360 ms (bound 1.29 ±0.05 ms) | §5.3 dead time: the inductor current changes sign at the end of the down ramp, a ≈ 0.27 V step at the switch node that the integrator recovers in ≈ 100 µs | **SPEC**: the 1.286 ms estimate comes from an averaged model without dead time; with SPEC's own 20 ns dead time and 0.7 V body diode it is ≈ 1.38 ms |
| C50 | `s5_hiccup_with_over_current_16_to_20_us_after_the_step` | 28.5 µs (bound 16 to 20 µs) | the window counts only the 8 cut periods; the first cut needs the current to climb 7 A first | **SPEC**: infeasible on SPEC's numbers (22.5 µs even at the 90 % clamp from the first sample); the §6.2 loop gives 28.5 µs |
| C68 to C71 | the four S9 corner tests | corner 6 unsettled (drift 8.48 mV); ripple 16.1 / 11.4 / 12.5 mV and code swing 2 at corners 6 / 8 / 9 | the windows open 200 µs after a 200 µs line ramp, ≈ 2 time constants of the loop's slowest closed-loop pole pair (≈ 6 kHz, τ ≈ 107 µs at 9 V); the residual of the ramp's tracking lag is still 1 to 2 codes | **SPEC §10.1 (bench timing)**: SPEC fixes no wait before an S9 window; the plan's 200 µs was borrowed from S3/S4's ±1 % recovery, a criterion 5 times looser than S9's. The model and SPEC's §6.5 claim hold: every corner is a constant code once settled |

No verdict is "model": the MSL controller is faithful to SPEC §5 to §8 in every respect these tests touch, and
SPEC's stated design parameters (gains, dead time, current limit, LC) meet SPEC's performance claims once the
three bench bounds are derived from those same parameters. No tolerance is widened: two estimates are
recomputed from SPEC's own numbers (S2b, S5), and one measurement instant SPEC never fixed is derived (S9).
No fix touches the loop, so none can move S3 or S4 (both within 2 to 3 % of their limits).

## How the numbers were checked

A scratch cycle-accurate model of SPEC §2 to §9 (Python, not committed): RK4 on the 5 ns clock over the
inductor, Cout with ESR, Cin with ESR behind the 20 mΩ source, Ron on both switches; the exact Q4 PID of §6.3
(arithmetic shifts, clamps, conditional integration); the mid-tread ADC sampled at count 0 with the 500 ns
conversion; the bit-reversed 3-bit dither; dead time with the body diodes (Vf 0.7 V, 20 mΩ) and, as a variant,
the 2 × 200 pF Coss; the 7.5 A / −3 A cuts and the supervisor's cut counter. Three variants of the power stage
were run: averaged (SPEC's own tool, §6.1), switched without dead time, switched with §5.3's dead time.
A linear z-domain model (ZOH power stage, one sample of delay, C(z) of §6.2) gave the closed-loop poles.

Sanity: the averaged variant reproduces SPEC's own S2b estimate (1.284 ms against SPEC's 1.286 ms, −0.61 A
against −0.62 A), and the switched variant reproduces the MSL run's S5 timing to the period (first cut period
12 to 14 µs, `Hiccup` at 28.5 µs) and its S9 symptoms at 200 µs (corner 6: ripple 15.5 mV, code swing 2).

---

## 1. C22, S2b settling (1.360 ms against 1.29 ±0.05 ms)

### Cause

During the 800 → 160 code ramp the converter sinks: `Cout · slew` = 0.69 A out of the capacitor, minus the
0.15 A load, so the inductor current sits at ≈ −0.6 A (§7.2), with a ripple of 0.39 A p-p at 1 V / 12 V
(§3.2): both its peak (−0.44 A) and its valley (−0.83 A) are negative. In both 20 ns dead-time intervals of
each period (§5.3) the negative current freewheels through the **high-side** body diode, so the switch node
sits at Vin + 0.7 V: the dead time adds `2 · 20 ns / 2 µs · (12 + 0.7) V` = **+0.25 V** to the average
switch-node voltage.

When the ramp stops, the current returns to the load's +0.15 A (peak +0.35 A, valley ≈ −0.04 A). The dead
time now freewheels through the **low-side** body diode (−0.7 V) or sits near 0 V, so the same duty word gives
≈ 0.27 V less at the switch node: ≈ 70 duty LSBs at 12 V (3.75 mV per LSB, §5.2), 1 100 Q4 units of
integrator. With Ki = 8 Q4 per code per sample (§6.3) and an error of 2 to 3 codes, the integrator needs
≈ 50 samples, 100 µs, to absorb it. Meanwhile Vout dips 15 to 23 mV under 1.0 V (MSL: 0.9855 V, code 158),
out of the ±10 mV band, and settles only when the integrator has caught up.

SPEC's "model 1.286 ms" comes from its averaged model (preamble, §6.1), which has no dead time.

| Model of SPEC §2 to §9 | S2b settling | Lowest Vout after the ramp |
| --- | --- | --- |
| averaged (SPEC's tool) | 1.284 ms | 0.993 V |
| switched, no dead time | 1.285 ms | 0.994 V |
| averaged + §5.3 dead-time term | 1.389 ms | 0.977 V |
| switched + dead time | 1.386 ms | 0.977 V |
| switched + dead time + 2 × 200 pF Coss | 1.376 ms | 0.978 V |
| the MSL model (the bench) | 1.360 ms | 0.9855 V |

Removing the dead time alone brings the switched model back to 1.285 ms: the dead time is the whole
difference.

### Verdict: SPEC

The model implements §5.3 (4 counts at each edge, body diode 0.7 V) and §6.2 to §6.4 faithfully. SPEC
specifies no dead-time compensation, so its own design cannot settle at 1.29 ms: the estimate is the defect.
(A dead-time compensator in the DPWM would restore ≈ 1.29 ms, but it is a new design element SPEC does not
have, and any loop change endangers S3/S4; not recommended.)

### Fix

`examples/buck/SPEC.md` §10, row S2b, expected outcome and model column:

> | S2b | Setpoint down | from 5.0 V, write 8 (1.0 V) | ramp of 1.28 ms; within ±1 % of 1.0 V at **1.38 ms ±0.05 ms**; inductor current goes negative (≈ −0.6 A, FCCM sinking) without hitting the −3 A limit; no OVP | **1.38 ms** (1.284 ms without dead time), −0.62 A |

and one paragraph in §10.1 (after "Settling time."):

> **Dead time at the end of a down ramp (S2b).** While the output ramps down the inductor sinks ≈ −0.6 A and
> both 20 ns dead times of §5.3 freewheel through the high-side body diode (switch node at Vin + 0.7 V); when
> the ramp ends the current returns to the load's +0.15 A and they freewheel through the low side (−0.7 V).
> The same duty word then gives ≈ 2 · 20 ns / 2 µs · 13.4 V = 0.27 V less at the switch node, ≈ 70 duty
> LSBs at 12 V, which the integrator (Ki = 0.5, §6.2) absorbs in ≈ 100 µs while Vout dips 15 to 25 mV under
> 1.0 V. The averaged model with this dead-time term settles at 1.389 ms (1.284 ms without it); the bound is
> 1.38 ms ±0.05 ms.

Bench: `tests/buck_scenarios.rs` C22 renamed `s2b_vout_settles_into_1_v_within_1_percent_at_1_38_ms`, the
constant `1.29e-3` → `1.38e-3` and its comment; `crates/piperine-cli/tests/docs.rs`
`the_buck_spec_holds_the_bench_bounds` (BS-09 AC 3) asserts `within ±1 % of 1.0 V at 1.38 ms ±0.05 ms`;
spec.md D9 and BS-03 AC 7, tasks.md/proofs.md C22 follow the new name.

Expected: measured 1.360 ms is inside 1.33 to 1.43 ms (the window was not fitted to it: every SPEC-parameter
model above, 1.376 to 1.389 ms, is inside it too). Risk to green tests: none (SPEC text and one bench bound;
no model change).

---

## 2. C50, S5 hiccup timing (28.5 µs against 16 to 20 µs)

### Cause

`Hiccup` is entered at the `done` strobe (count 100, §9) of the period after the 8th cut period: **16.5 µs
after the start of the first cut period** (8 × 2 µs + 500 ns, §8.2, §4.1). SPEC's 16 to 20 µs window is that
16.5 µs plus up to 1.75 µs for the first cut, as if the first period after the edge were already cut. It cannot
be: the inductor current must first climb from 0.5 A to 7.5 A.

Lower bound on SPEC's own numbers (the bench edge is at a period start, 1.25 ms = 625 Ts): the load ramps in
4 µs (§10, `t_step`); the first sample that can see a droop is at +2 µs and its duty applies at +4 µs (§9).
Even granting a duty at the 90 % clamp from +2 µs (the first period any decision after the edge can set), the
current rises by ≈ (0.9 · 12 − 3.3) V / 4.7 µH ≈ 1.6 A/µs on average, its peak reaches 7.5 A at +6.7 µs, so the
first cut period starts at +6 µs and `Hiccup` comes at 6 + 16.5 = **22.5 µs** at best (simulated with the duty
forced to 2880: 22.5 µs). No design with SPEC's
90 % clamp, current limit and LC meets 20 µs.

With the §6.2 loop (switched model, duty word per period):

| Period start after the edge | 0 | 2 µs | 4 µs | 6 µs | 8 µs | 10 µs | 12 µs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Duty word (Kp 4, Kd 64) | 849 | 860 | 1339 | 1959 | 2166 | 2070 | 1894 |
| Duty | 27 % | 27 % | 42 % | 61 % | 68 % | 65 % | 59 % |
| Current gain in the period, (D·12 − 3.2) V · 2 µs / 4.7 µH | 0 | ≈ 0 | 0.75 A | 1.8 A | 2.1 A | 2.0 A | first cut |

The average current passes ≈ 7 A at +12 µs; its peak reaches 7.5 A in the period that starts at +12 µs, the
first cut period, so `Hiccup` comes at 12 + 16.5 = **28.5 µs**, exactly what the MSL run measures.

### Verdict: SPEC

The window is infeasible on SPEC's own numbers (best case 22.5 µs > 20 µs); the model's counting is §8.2's.

### Fix

`examples/buck/SPEC.md` §10, row S5, expected outcome and model column:

> ... `fault` = OverCurrent and `mode` = Hiccup **16.5 µs after the start of the first cut period (§8.2, 8
> periods and the 500 ns conversion) and 26.5 to 30.5 µs after the step** ... | **28.5 µs (first cut period
> 12 µs after the step); 22.5 µs at best with the duty at its 90 % clamp from the first period after the step** |

and one paragraph in §10.1:

> **Overcurrent timing (S5).** `Hiccup` comes at the `done` strobe after the 8th cut period: 8 × 2 µs + 500 ns
> = 16.5 µs after the start of the first cut period. Before it, the inductor current climbs from 0.5 A to the
> 7.5 A limit under the loop of §6.2: the step's edge is at a period start, the duty word first moves in the
> period 4 µs after it (one sample to see the droop, one of computational delay, §9) and rises to ≈ 68 % by
> +8 µs; the current gains 0.75, 1.8, 2.1 and 2.0 A in the periods from +4 µs, and its peak reaches 7.5 A in
> the period that starts at +12 µs. `Hiccup` is then at 28.5 µs; the bench allows one period either way, 26.5
> to 30.5 µs, the resolution at which the first cut lands. Even with the duty at its 90 % clamp from the first
> period after the edge, the first cut period starts at +6 µs: no run can be faster than 22.5 µs.

Bench: `tests/buck_scenarios.rs` C50 renamed `s5_hiccup_with_over_current_26_5_to_30_5_us_after_the_step`,
range `16e-6..=20e-6` → `26.5e-6..=30.5e-6` and its comment. Optional, recommended (the mechanism, exact):
assert `Hiccup` − (wrap that starts the first cut period) = 16.5 µs ± 20 ns (`cut` rises at the wrap that
ends the first cut period, so that wrap − 2 µs is its start). design.md's Risks row for S5 and tasks/proofs C50
follow. Expected: 28.5 µs is inside. Risk to green tests: none.

---

## 3. C68 to C71, S9 steady state

### Cause

The windows open `SETTLE` = 200 µs after each 200 µs supply ramp or setpoint move (spec.md assumption row
"S9's settling before each window", justified by S3/S4's "back within ±1 % in ≤ 200 µs"). That borrows the
wrong criterion: ±1 % is ±33 mV at 3.3 V, while S9 judges 10 mV p-p and one code (6.25 mV).

The §6.2 loop has a slow closed-loop complex pole pair next to the compensator's 5.97 kHz zero pair (the
classic type-III dipole), from the linear model of §6.1 / §6.2:

| Vin | slowest closed-loop pair (all Vout, Iout corners) | time constant τ |
| --- | --- | --- |
| 9 V | \|z\| = 0.9815 to 0.9820 at 5.2 to 6.3 kHz | 106 to 110 µs |
| 12 V | \|z\| = 0.9797 to 0.9809 at 5.3 to 6.2 kHz | 97 to 104 µs |
| 16 V | \|z\| = 0.9783 to 0.9802 at 5.4 to 6.1 kHz | 91 to 100 µs |

The worker's "≈ 7 kHz oscillation of 30 mV" at corner 6 is this pair, not a limit cycle. A 200 µs ramp
leaves a tracking lag of `Δu / (N · Ki)` codes (the integrator alone ramps the duty; one integrator in the
loop): 12 V → 9 V at 3.3 V / 5 A moves the duty word from 933 to 1244 in N = 100 samples, a lag of
3.1 / 0.5 = **6.2 codes (39 mV)**. Excited by it, the slow pair decays by e only every ≈ 107 µs: 200 µs
after the ramp (≈ 1.9 τ) 1 to 2 codes remain, which is the code swing of 2, the 11 to 16 mV ripple and the
8.5 mV half-window drift. The same holds after the 9 → 12 → 16 V ramps at 5.0 V (corners 8 and 9).

The switched model, run through the bench's exact sequence, reproduces the reds at 200 µs and shows them gone
later; started directly at each corner it holds a **constant code** at all nine (ripple 1.2 to 4.8 mV), as
SPEC §6.5 claims:

| Corner (Vin, Vout, 5 A) | at 200 µs: ripple, code swing | at 300 µs | at 450 µs |
| --- | --- | --- | --- |
| 5 (12 V, 3.3 V) | 10.7 mV, 2 | 4.0 mV, 0 | 3.8 mV, 0 |
| 6 (9 V, 3.3 V) | 16.6 mV, 3 (drift 7.3 mV) | 7.7 mV, 0 | 5.3 mV, 0 (drift 1.4 mV) |
| 8 (12 V, 5.0 V) | 15.2 mV, 2 | 7.8 mV, 0 | 4.3 mV, 0 |
| 9 (16 V, 5.0 V) | 11.4 mV, 1 | 9.2 mV, 0 | 5.4 mV, 0 |
| all nine | 4 fail | pass, corner 9 at 9.2 mV | pass, worst 5.4 mV, every code constant |

Corner 6 in a longer trace: constant code from 320 µs after its ramp onwards (duty word 1258, integrator
1258), DC error +0.4 mV.

**Confirmed on the MSL model itself** (scratch target, a copy of `tests/buck_scenarios.rs` with `SETTLE` =
450 µs and only the `steady_state` run, stop 7.46 ms, removed after the run; 1 551 s wall, 746 001 rows): all
four S9 tests pass.

| Corner | DC error (bound) | ripple (10 mV) | code swing (1) | half-window drift (6.25 mV) |
| --- | --- | --- | --- | --- |
| 1 (9 V, 0.8 V) | 0.70 mV (6.5) | 1.26 mV | 0 | 0.05 mV |
| 2 (12 V, 0.8 V) | 0.63 mV (6.5) | 1.47 mV | 0 | 0.11 mV |
| 3 (16 V, 0.8 V) | 1.36 mV (6.5) | 1.36 mV | 0 | 0.01 mV |
| 4 (16 V, 3.3 V) | 2.94 mV (14.7) | 3.95 mV | 0 | 0.17 mV |
| 5 (12 V, 3.3 V) | 3.51 mV (14.7) | 6.18 mV | 1 | 0.42 mV |
| 6 (9 V, 3.3 V) | 1.61 mV (14.7) | 4.04 mV | 0 | 0.62 mV |
| 7 (9 V, 5.0 V) | 0.38 mV (20) | 3.02 mV | 0 | 0.00 mV |
| 8 (12 V, 5.0 V) | 1.20 mV (20) | 5.09 mV | 0 | 0.64 mV |
| 9 (16 V, 5.0 V) | 1.42 mV (20) | 8.64 mV | 1 | 2.78 mV |

Corner 9 is the tightest (8.64 mV against 10 mV; its 16 V ramp is the last and its own switching ripple is
the largest, §3.3's 6.1 mV worst corner): at 450 µs it still carries ≈ 2.5 mV of residue, consistent with
3.2 τ being the minimum. 450 µs is kept rather than pushed further so the run stays inside its step budget;
500 µs would add 0.45 ms more run time for ≈ 1 mV more margin at corner 9 (an option for the user).

### Verdict: SPEC §10.1 (the bench's measurement instant), not the model, not SPEC's performance claims

SPEC §10 S9 says "steady state" and §10.1 fixes the ramp between corners but not how long after it the
output is steady. The model meets §3.3 (ripple ≤ 6.1 mV) and §6.5 (constant code) once steady; DC errors are
within §4.3 at every corner. The bounds (10 mV, 1 code, one-code drift, §4.3) do not move.

### Fix

`examples/buck/SPEC.md` §10.1, a row after "Supply ramp between the corners of S9":

> | Wait before an S9 window | **450 µs** after the last supply ramp or setpoint move, then a 64 µs window
> (32 periods, 4 dither frames). The slowest closed-loop poles of §6.2 are a pair at 5 to 6 kHz with
> \|z\| ≤ 0.982, τ ≤ 110 µs (at 9 V). A 200 µs ramp leaves a tracking lag of Δu / (N · Ki) codes, at most
> 6.2 codes (12 V → 9 V at 3.3 V, 5 A); it decays under ¼ code (1.6 mV, so the §3.3 ripple of 6.1 mV plus
> the residue stays under 10 mV and within one code) after τ · ln(6.2 / 0.25) = 3.2 τ = 350 µs; 450 µs is
> ≈ 4 τ. S3 and S4's 200 µs is a recovery into ±1 %, not into one code |

and §10 row S9's stimulus cell: "9 corners: ... at full load, each measured after the wait of §10.1".

Bench: `tests/buck_scenarios.rs` `SteadyStatePlan::SETTLE` `200e-6` → `450e-6` with its doc citing §10.1;
`tests/e2e/buck/Piperine.toml` `[run.steady_state] stop` `5.20e-3` → `7.46e-3` (nine waits of +250 µs;
≈ 8.5 M steps, inside `max_steps = 15000000`); spec.md's assumption row, BS-07's preamble, design.md
"Timelines" (window times) and its `SETTLE` line follow; optionally `docs.rs` AC 4 holds "450 µs" in §10.1.

Expected (measured on the MSL model, table above): all nine corners settled, ripple ≤ 8.64 mV, code swing ≤ 1, drift ≤ 2.78 mV, DC errors within §4.3
(C68 to C71 green). Cost: the `steady_state` run grows from 5.20 to 7.46 ms simulated (+43 %); it is the
longest family, so the suite's wall time grows by about the same fraction (the probe's `steady_state` alone took 1 551 s ≈ 26 min wall for 7.46 ms). A cheaper
alternative with the same derivation (slower ramps, smaller lag) costs the same time in ramps and is not
better. Risk to green tests: none for S2 to S8 (another run); C72 (`steady_state_passes_every_check`) runs
longer and stays a `Never` check.

---

## What the user is asked to confirm (AFK defaults)

1. S2b: SPEC's bound becomes 1.38 ms ±0.05 ms (dead time made explicit), rather than adding a dead-time
   compensator to the design.
2. S5: SPEC's window becomes 26.5 to 30.5 µs after the step, with the mechanism (16.5 µs after the first cut
   period) stated.
3. S9: SPEC §10.1 gains a 450 µs wait before each S9 window, derived from the loop's slowest pole; the run
   gets 2.26 ms longer.

# Transient Performance - Measured Diagnosis

Measured 2026-11-20 on `origin/main` 494a7f75, `examples/buck` run `startup`, the first 100 µs
(`--stop 100e-6`; the converter is enabled at 1.5 µs, so 49 switching periods). Backend `rust`.
The counters were a throwaway probe (static atomics, printed at the end of `Protocol::transient`),
reverted before this commit. Step counts are deterministic and repeat exactly; wall times are on a
shared 12-core machine under load and vary by about 20 %.

## 1. Verdict

The buck is slow for four reasons, in this order:

1. **The solver lands on every edge of the 200 MHz clock**, 40 000 landings per 100 µs, although
   only 367 of them change anything the analog side reads and 50 sample it. Half of those landings
   (every falling edge) exist only because the oscillator's own commit `clk <- !clk after 0.5 / f`
   has a param expression for its delay, which the static rule cannot prove to be a tick or more.
2. **A landing shortens the step and the controller regrows from the shortened step at 1.5x**, so
   the step never gets back to what the error allows: 3.2 accepted steps per 2.5 ns landing interval.
3. **Each accepted step costs about 120 to 160 µs** for a circuit of a few tens of unknowns. The
   time goes to per-call allocation, shape validation on every device evaluation, four clock
   readings per Newton iteration, and an instant run at every accepted step.
4. **`max_step = 1e-8`** is the floor once 1 and 2 are fixed: 10 000 steps per 100 µs. It is
   needed for the printed waveform (rows are linear interpolation between accepted steps), not for
   the solution's accuracy.

The speed of the build is not a cause: the release build (fat LTO) and the dev build take the same
time, because the solver crates are already at `opt-level = 2` in dev.

A fifth finding is about accuracy, not speed, and it bounds every fix above: **two kinds of step are
exempt from the truncation-error test, and they are accurate today only because every step is tiny.**
Removing the landings without closing this makes the buck's `vout` wrong by 1 to 5 mV (section 5).

## 2. Baseline counts (100 µs, `max_step = 1e-8`)

| Quantity | Value |
| --- | --- |
| Accepted steps | 127 608 (0.78 ns average; 1 276 per µs, 6.0 M for 4.7 ms) |
| Rejected attempts | 387, all by the truncation test; 0 by Newton |
| Crossing retakes (`LandedEarlier`) | 0 |
| Steps ending on a landing | 39 999 (one per 2.5 ns: both clock edges) |
| Steps ending on the controller's proposal | 87 609 |
| Steps at `max_step` | 0: the step never reaches 10 ns |
| Jumps delivered to the analog side | 367 (7.5 per 2 µs period: 4 gate ramps, 2 corners each) |
| Newton solves / iterations | 255 992 / 301 264 (1.18 per solve; 83 % converge in 1) |
| Instants run by look-ahead / at an accepted step | 39 999 / 127 611 (one per accepted step) |

### Landings by R4 cause

| Cause | Count | Needed? |
| --- | --- | --- |
| A firing reaction is flagged as reaching the analog side, nothing analog-read changed | 39 815 | no |
| The same, and an analog-read net changed at microstep 0 | 184 | yes |
| An analog-read net changed (alone) | 0 | - |
| `IRREVERSIBLE` reaction, host waiter, `FINISH` | 0 | - |

Which reactions are flagged, from the first 400 landings (3 µs):

| Reaction | Fires | Flagged | Why it is flagged |
| --- | --- | --- | --- |
| `Startup.buck.osc` `on clk.changed() { clk <- !clk after 0.5 / f; }` | every edge | yes | its delay is a param expression, so the statement carries no `LATER`; `clk` reaches the analog side through `Dpwm`; the static rule then says the oscillator "can change, at that instant" a reaching net |
| `Startup.buck.ctl.pwm` (`Dpwm`, `on clk.rise()`) | rising edges | yes | commits `hs` and `ls` at every edge; they change 4 times per 400 edges |
| `...pwm.no_shoot_through` (`Never`) | rising edges | no | - |

So the falling edges land only because of the oscillator's own delayed commit (§17.2 rule 2 exempts
"a commit whose delay always advances time by at least one tick"; the plan sets `LATER` only for a
delay it can prove, and `0.5 / f` is not one), and the rising edges land because of the static
second bullet of rule 2 (`multi-rate` Q1). `Dpwm`'s `on clk.rise()` does not fire on a falling
edge: that part of the rule already holds.

### Step-size histogram (accepted steps)

| Step | Count |
| --- | --- |
| under 4 ps | 908 |
| 4 to 260 ps | 28 800 |
| 260 ps to 1.05 ns | 58 177 |
| 1.05 to 2.1 ns | 36 618 |
| 2.1 to 2.5 ns | 3 105 |
| over 2.5 ns | 0 |

## 3. Time split

Clean binary (no probe), 100 µs: 15.3 to 24 s wall depending on machine load, 120 to 190 µs per
accepted step. Release and dev within noise of each other (20.0 s and 20.3 s in the same minute,
probed binaries).

`perf` is not usable on the machine (`perf_event_paranoid = 3`); the profile is 220 stack samples of
the clean binary taken with `gdb -p` (about ±3 points per row).

| Where (inclusive) | Share |
| --- | --- |
| Newton solve, both stages | 72 % |
| - assembling: device evaluation (`Assembly::evaluate_for`) | 39 % |
| -- `Handle::eval`, of which `cell_lens` (shape walked and two `Vec`s built per call) is 10 points | 26 % |
| -- stamping the rows | 4 % |
| - `Assembly::push_stage` after each converged stage (a full re-evaluation) | 21 % |
| - sparse LU (faer: numeric factorization and solve) | 12 % |
| - `clock_gettime` (four `Instant` readings per Newton iteration for `assembly_ns`/`solve_ns`) | 12 % |
| The instant run at every accepted step (`run_whole_instant`, `run_reactions`) | 11 % |
| Sink flush and step observer | 5 to 6 % |
| Checkpoint, release, look-ahead, propose, accept | about 8 % |
| `malloc`/`free`/`realloc` as leaf (spread over the rows above) | about 15 % |

The 12 % of clock readings is a property of this machine (`clocksource = hpet`, so every reading is
a system call of about 1.5 µs); on a TSC machine it is under 1 %. It is still pure cost: nothing
reads those two totals during a run.

The print grid costs little: 100 µs with `print_step` 0.5 µs and with 10 ns differ by 0 to 10 %,
inside the noise. The recorder interpolates between accepted steps; `print_step` forces no step.

## 4. Levers, measured

Each row is a run of the same 100 µs with a throwaway switch (environment variable) on the probe
build. "Error" is the largest difference of `buck.vout` over the 201 printed rows against a
reference run with `max_step = 1e-10` (1 012 673 steps).

| # | Variant | Accepted steps | vs baseline | Error |
| --- | --- | --- | --- | --- |
| 0 | Baseline | 127 608 | 1x | 0.104 mV |
| 1 | Baseline, uncapped (`max_step = 1e-6`) | 127 608 | 1x | 0.104 mV |
| 2 | Keep the controller's step across a landing | 40 367 | 3.2x | 1.66 mV |
| 3 | Dynamic landings (land only on a changed analog-read net or a reaction that itself reads a continuous pin) | 22 488 | 5.7x | 2.33 mV |
| 4 | 2 + 3 | 15 772 | 8.1x | 3.50 mV |
| 5 | 2 + 3, uncapped | 831 | 154x | 1.05 mV (0 rejections: the error test never ran on the steps that mattered) |
| 6 | 2 + 3 + landing steps tested for truncation error | 25 110 | 5.1x | 2.74 mV |
| 7 | 6, uncapped | 16 069 | 7.9x | 2.76 mV |
| 8 | 6 with `trtol = 1` | 47 268 | 2.7x | 2.73 mV |
| 9 | 6 with `reltol = 1e-4` | 50 472 | 2.5x | 1.03 mV |
| 10 | 6 with `max_step = 1e-9` | 113 513 | 1.1x | 0.101 mV |
| 11 | **6 + the step that reads a jump at most 25 ps** | **26 906** | **4.7x** | **0.002 mV** |
| 12 | **11, uncapped** | **17 915** | **7.1x** | **0.009 mV** |
| 13 | 3 + landing steps tested + jump step 25 ps (no 2) | 30 852 | 4.1x | 0.003 mV |
| 14 | 2 + 3 + jump step 25 ps, landing steps still exempt | 15 869 | 8.0x | 4.88 mV |
| 15 | Baseline + jump step 25 ps | 129 921 | 1.0x | 0.018 mV |
| 16 | 12 with growth cap 2 (and 4, rejection ÷ 2) | 16 341 (14 160) | 7.8x (9.0x) | 0.026 mV (0.029 mV) |

Digital columns (`reference`, `mode`) are identical in every variant, row for row.

What the table says:

- **Landings are the cause** (rows 2 to 4). With dynamic landings 417 remain per 100 µs: 367 jumps
  and 50 sampling instants.
- **`max_step` is not what makes the baseline accurate** (rows 1, 10): the baseline never reaches it.
- **The error of the fast variants is not a truncation-tolerance matter** (rows 8, 9): `trtol` does
  not move it. It comes from two exempt steps:
  - the step that **reads a jump** is first order and sized `0.1 x min(last step, gap to the next
    scheduled time)`. With last steps of 0.5 ns it is 50 ps; with last steps of 10 ns it is 250 ps
    (the gap to the next clock edge is 2.5 ns). The switch node moves 12 V in the first picoseconds
    of a gate ramp, so a backward-Euler step of 250 ps misplaces the edge by up to 250 ps: 12 V x
    0.25 ns / 2 µs = 1.5 mV of average output per edge. Rows 11 to 13 and 15 show it: bound that
    step and the error falls to microvolts, below the baseline's.
  - the step that **ends on a landing** is exempt whatever its size (row 14 against 11): with large
    steps it crosses a whole 5 ns gate ramp untested.
- **Keeping the controller's step across a landing** (lever 2) is worth 13 % once landings are
  dynamic (row 13 against 11), and it moves the time grid after every landing in every circuit.
- **The growth cap** is worth 9 to 21 % (row 16).
- **What is left is the model's**: about 45 accepted steps per gate-ramp corner (8 to 260 ps, the
  truncation test on the 200 pF `coss` against a 20 mΩ channel), 85 % of the remaining steps.

## 5. The two exemptions

`device_attempt` passes `landed_on_breakpoint: exempt` to `assess_step`, and `exempt` is true for
every step that ends on a `Landing` or a `Final`. Under the protocol's order (abi.md §5: solve on
the left value, then run the instant) that step reads no change: it is an ordinary step over a
smooth interval. Part V §15.2 exempts "the step that reads the change", which in the protocol is
the next one (`post_set_step`). The first exemption has no reason left; the second has one (Milne's
estimate would measure the jump) but leaves the step with no error control at all.

`docs/spec/part_v_execution.md` §15.2 "Before the solve of `t`" still describes the older order
(what is due at `(t, 0)` applied before the step that arrives at `t`). It does not match
`protocol/transient.rs` on main ("the order of steps 3 and 5 is the user's, 2026-11-15"). The
feature's amendment states the exemption by what a step reads, which holds under either order; the
stale paragraph is reported to the user, not rewritten here.

## 6. Expected gain per lever

| Lever | Requirement | Expected effect on the buck | Evidence |
| --- | --- | --- | --- |
| Land only where something observable happens | TP-02, TP-03 | 127 608 to about 30 000 steps per 100 µs (4.1x) with `max_step = 1e-8`, with TP-04 and TP-05 | rows 3, 13 |
| Test the landing step; error-control the jump step | TP-04, TP-05 | error from 0.104 mV to under 0.02 mV; without them the 4.1x costs 2 to 5 mV | rows 6, 11, 13, 14 |
| Per-step cost (validation once, no per-call allocation, no clock reading, no idle instant) | TP-07, TP-08 | 1.5x to 2x per step: about 10 + 12 + 11 points of the profile are removable outright, allocation another 5 to 10 | section 3 |
| Both | - | about 6x to 8x wall: a 21 min run in about 3 min | product of the two |
| Remove `max_step` (needs a higher-order print interpolant) | deferred, TP-D1 | a further 1.5x (26 906 to 17 915) | rows 11, 12 |
| Sample-only instants read an interpolant instead of landing (the user's idea) | deferred, TP-D2 | 50 landings of about 27 000 steps: under 0.5 % for the buck | row 3 |
| Keep the controller's step across a landing | rejected | 13 %, moves every pinned time grid | row 13 |
| Growth cap 2 | rejected | 9 %, moves every pinned time grid | row 16 |
| Release build | none | 0 % | section 3 |

## 7. The interpolation idea

The premise is right: digital instants inside what should be one analog step are what makes the
buck slow (40 000 of them per 100 µs). The measured cure is different from the proposed one, for
three reasons.

1. **39 583 of the 40 000 instants neither read nor write anything analog.** They need no
   interpolated state, because they read no state. They need not to be landed on at all, and the
   look-ahead already runs them before the analog solve, under a sub-checkpoint. Only the test that
   decides "landing" is too coarse.
2. **No backtracking is needed for writes.** The protocol runs the digital side first: an instant
   that turns out to change an analog-read net is rewound (a discrete restore) and becomes the
   step's end before any Newton solve is spent. Measured: 0 analog retakes, 2 ms of restore in 20 s.
   Solving first and backtracking the analog step would throw away solves that today are never made.
3. **Reading the interpolant saves only the sampling instants**: 50 per 100 µs for the buck (the
   ADC, once per 2 µs period, both edges of `sample`), under 0.5 % of the remaining steps. Accepting
   a truncated step from interpolated state (phase 2 of the idea) saves less still, and a jump must
   be landed on exactly whatever is interpolated (§15.2).

The idea pays in a different design: one whose samplers fire at a rate near the analog step (a
200 MHz sampled comparator, a sigma-delta). TR-BDF2 gives the interpolant for free: the quadratic
through the step's start, its `γh` stage and its end. The same interpolant would let the print grid
drop `max_step`. Both are specified as deferred items (TP-D1, TP-D2) with the statistic that tells
when they are worth building: sampling landings as a share of accepted steps (TP-01 reports it).

## 8. Baseline of the 600 µs start

Measured by `buck_long` (`cargo test -p piperine-api --test buck_long -- --nocapture`) on the
statistics of T1 to T5, on `feature/transient-performance` merged with main `754784a1` (so with
`analog-operators` T9: the step that reads a jump is first order), before any landing or test
changes. The rule is the declared one, the only one there is until T15. Wall time 151.9 s (printed,
never asserted), peak live heap 11.3 MiB.

| Count | 600 µs start |
| ----- | ------------ |
| accepted steps | 804 136 |
| landings, an analog-read net changed | 1 182 |
| landings, a firing reaction reads a continuous pin | 0 |
| landings, declared reach only | 238 803 |
| landings, `IRREVERSIBLE` or host waiter | 0 |
| steps ended at a located crossing | 0 |
| steps ended at `max_step` | 0 |
| steps ended at a `bound_step` cap | 0 |
| steps ended at the stop | 1 |
| steps ended at the controller's proposal | 564 150 |
| attempts rejected by the truncation test | 2 062 |
| attempts rejected by Newton | 0 |
| steps retaken toward a crossing | 0 |
| steps that read a jump | 2 363 |
| jump steps retaken | 0 |
| steps at `max_step` (analog side) | 0 |
| instants run | 804 136 |
| sampling share | 0.000000 |

The accepted steps are 804 136, not the 675 000 the plan (section 2 and the goals) quotes: the
plan's count predates `analog-operators` T9 on main. The gate of 300 000 is unchanged. A landing is
counted under the first cause that holds, in this order: an analog-read net changed, a firing
reaction reads a continuous pin, the declared reach only. The share of sampling landings is 0 here;
that the ADC's sampling instants (50 per 100 µs, section 7) do not show as `continuous read` is not yet
explained (the cause is picked after the analog-read test, so they may be counted there) and T15's
tests settle it.

## 9. Progress of `buck_long` (600 µs start, `max_step = 1e-8`, same machine as section 8)

Run once after each optimisation task. The wall time is printed, never asserted, and varies by
about 20 % with the load of the machine.

| After | Accepted steps | Wall time | Rejected by truncation | Jump steps | Note |
| ----- | -------------- | --------- | ---------------------- | ---------- | ---- |
| baseline (section 8) | 804 136 | 151.9 s | 2 062 | 2 363 | |
| T9, the landing step is tested | 806 491 | 143.6 s | 2 110 | 2 367 | 48 more rejections, +0.3 % steps: the landings are still 240 000, so the tested steps are the small ones |
| T10, the jump step is error-controlled | 814 058 | 129.0 s | 1 707 | 2 367 | 769 jump retakes (0 before); every jump step starts at a tenth of the last step and is cut to an eighth of the failed step until it passes |

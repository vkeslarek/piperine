# Verification: `fix/idtmod-wrap`

Independent Opus verifier, 2026-11-24. Branch head `18d2b98ff` (7906fde0b, 9e0468e7f, 109af5175, 18d2b98ff on
origin/main 7cf35a4f7).

## Verdict: FAIL

The root cause is right and the fix is generic: no `idtmod` special case. Any analog-read net that holds a value
at the end of one microstep and loses it later in a transient instant is recorded (`passing.rs`) and read by a
retake. But the record lives only for one call of the microstep loop. When a host waiter suspends the instant,
the record is split across the calls, so a pulse can still be lost (shown below with a test) and the settle
phase can run twice at `t`. The brief named host waiters as an axis to check, and this is a silent wrong result
of the very class the fix closes, so it blocks.

The full suite was **not run**, because the verdict is FAIL (procedure: the full suite runs only on PASS).
Targeted runs, all green: `piperine-digital-solver --test instant` (26), `piperine-solver --test
protocol_transient` (27), `piperine-parity --test source_wraps` (1).

## Findings

### 1. HIGH: a host-waiter suspension splits the passing record. A pulse is lost, or the settle phase runs twice

`crates/piperine-digital-solver/src/instant.rs:128-129`: `let mut passing = PassingValues::default();` is a local of
`microsteps`. An instant that a host waiter suspends (`instant.rs:182-189`) runs again from `suspended`, with an
empty record. `protocol/host.rs:126-132` (`run_whole_instant`) concatenates each part's `passing`.

- **(a) A pulse split by a suspension is lost.** Reproduced with a temporary engine test (reverted, not
  committed). `Pulser p (go -> seat)` and `Pulser q (go2 -> seat2)`, `seat` read by the analog side, with
  `wait(go)` and `wait(go2)`. Fire `go` at tick 7. The host commits `go2` during the first suspension. The output:
  ```
  part1 boundary=[]              passing=[] host=[go]
  part  boundary=[(seat, [1])]   passing=[] host=[go2] settled=false
  part  boundary=[(seat, [0])]   passing=[] host=[]    settled=true
  ```
  `seat` goes high at the end of part 2 and low in part 3. No part reports it as passing. The owner delivers
  `[seat=1, seat=0]` to the step after `t`, and the solve never reads the high. Failure scenario: a host
  `on_event` waiter fires at the microstep where `idtmod`'s `seat` is high, and that wrap is silently missed.
  This is the X1 bug again.
- **(b) The settle phase runs twice.** The engine runs the settle phase when `mode.settle &&
  out.passing.is_empty()` (`instant.rs:224-226`). That test sees only the last part. Take a part that passed a
  value, followed by a part that passed none. The last part runs the settle phase inside the engine, before the
  retake, on the samples of the first solution. Then `protocol/transient.rs:643` (`passed && rounds.settle`)
  reopens the instant and runs it again. `on settle` reactions run twice at `t` and their settle commits are made
  twice. This breaks §15.2 rule 4 ("The settle phase runs once, after the last round"). The current fixture
  devices cannot build this case, because no device emits a waited event in the middle of a part. An MSL reaction
  that emits an event net can.
- **Fix direction:** keep `PassingValues` on `Digital` for the whole instant. Clear it only when a new instant
  starts at microstep 0, not on a resume from `suspended`. Base the settle gate on the record for the whole
  instant. Add an engine test and a protocol test for a suspension between the rise and the fall.

### 2. MEDIUM: `retake_passing` does not retake when a crossing the retake made changes a net once; `retake_step` does

In `protocol/retake.rs:537-569`, the loop continues only while `whole.passing` is non-empty. Suppose a crossing
that only the retaken solution makes runs reactions that change an analog-read net once. That change goes to the
step after `t` (`whole.boundary.extend`). `retake_step` (`retake.rs:75-105`) retakes again on exactly that
change (`whole.boundary` non-empty). The retaken step read a jump (the values passed through), and a crossing of
its own fired, which is the jump case of rule 4. So the same situation is treated two ways depending on what
started the retake. The amended text ("a value they pass through retakes again") matches the code, but it
contradicts the jump paragraph of the same rule. Either unify the two loops behind one predicate, or state the
exception in §15.2. The two loops also duplicate about 30 lines: retake, deliver, attempt, traces, accept, and
the reopen/fire/rerun. The cleanliness rules ask for one readable path. The `made` non-empty branch of
`retake_passing` has no test.

### 3. MEDIUM (for the user's review of the AFK default): the retake integrates a zero-width pulse over the whole step

The rule takes in every net that changes twice in one instant. That includes monotone changes (0→1→2: the point
at `t` shows 1, which is neither the left value 0 nor the right value 2) and zero-delay glitches. The retaken step
runs from `t_prev` to `t`, restarts the integrator, and reads the passed value at `t`. Any analog *integrand*
that reads the net therefore gains an area of about `h·A`, which depends on the step size. This covers `idt(x)`,
the charge of a `ddt`, and an RC driven by a source that a digital code sets. A zero-width pulse has no area.
This is correct for reads that force state (`idt` reset, `idtmod` seat) and is the point of the fix. For
integrand reads it is new, step-dependent behavior: a glitch that used to be invisible now shows as a spike at
`t`. §15.2 rule 4 as amended (`docs/spec/part_v_execution.md`) does not state this consequence. This does not
block on its own, but MD-84's review should decide it, for example: only the last passed value, or only when
the step is otherwise retaken, or documented as intended.

### 4. LOW: retakes for passing values are invisible in the statistics

`retake_passing` records neither a retake nor a distinct `StepEnd`/`InstantCause`, so a design that pulses
analog-read nets on every instant (the performance risk in the brief) cannot be seen from the run statistics.
Code review shows the cost is bounded: a net changed once per instant does not retake. The test
`a_net_that_changes_once_passes_through_nothing` covers this, and so do `tracks_passing = !lookahead &&
!mode.is_static` and `retake_state` returning empty. The cost on the buck was **not measured**, because the
suite did not run.

### 5. LOW: style

- `crates/piperine-digital-solver/src/lib.rs:4`: the `//!` line was lengthened to about 125 columns and not
  rewrapped.
- The duplication in finding 2.
- Otherwise the code follows the rules: no free functions, full-word names, doc comments on every new item, and
  `retake_passing` is about 45 lines.

## What holds

- **Root cause.** Only `Outcome::boundary` (end state, filtered against the analog copy) reached the solve, so
  `seat`'s rise and fall cancelled out. MD-84 §1 is accurate.
- **The class.** The fix is generic: any analog-read carrier or param, recorded in the digital engine.
  Nothing names `idtmod`.
- **Operating point and DC.** `tracks_passing` is false in a static round, which the engine test covers. A
  host `set` between runs ignores `passing` (`host.rs:96-104`). `restore_passed` only re-adds bytes the copy
  already holds, so nothing changes there.
- **LTE exemption and event location.** The retake uses `attempt(.., landing = true)` with `StepKind::Retake`,
  the same path as `retake_step`. §17.3 rule 8 (a step that reads a jump does not locate its crossings) holds.
  A retake that is rejected is an `E0504` "does not converge", as before.
- **Checkpoints and the integrator.** The retake restores to `c.analog` (the step's mark) and delivers with
  `Restart::Integrator`. After it, `restore_passed` gives the analog copy the value the instant left behind, so
  the step after `t` reads a jump (`JumpRetake::delivered`). The protocol test pins this call order.
- **The `E0504` bound.** `max_rounds` is enforced and the reworded message is correct.
- **Determinism.** The record uses `BTreeMap`, so the order is deterministic.
- **Tests.** `source_wraps.rs` is strong: a closed-form sawtooth computed in Rust, over the top and under the
  bottom (from an `ic` outside the range), three wraps, a tolerance of 1e-8, and a range check on every accepted
  point. The protocol test pins the call order of the retake and the settle phase that follows it. The engine
  tests cover a pulse, a single change and a static round. Missing: a suspension during a pulse, a retake round
  that makes a crossing, and an integrand that reads a pulsing net.
- **Spec.** The changes to §15 step 3.4, §15.2, rule 4 and the §22.2.9 rule-3 table agree with each other, apart
  from finding 2 (and finding 3 is not stated). The resolution note replaces the stale X1 note correctly.
- **`7906fde0b`** is the verbatim generics phase E hunk for `pin_type` and the `Option<f64>` carrier. It is fine.

## Suite

| Gate | Result |
| --- | --- |
| `piperine-digital-solver --test instant` | green (26) |
| `piperine-solver --test protocol_transient` | green (27) |
| `piperine-parity --test source_wraps` | green (1) |
| Full suite (build, workspace tests, parity, buck release, CLI twins) | not run: FAIL |

---

# Round 2 (head `715083c34`): FAIL

Fixes reviewed: `696706482` (F1: the passing record on `Digital` for the whole instant, saved in marks and
checkpoints, settle phase once), `4f5f181e8` (F2 and F3: a passed value is read by a zero-width solve,
`AnalogSide::zero_width`, one loop `read_instant`, §15.2 rule 4 rewritten), `715083c34` (F4 and F5: statistics,
the doc line).

## Status of the round 1 findings

| Finding | Status |
| --- | --- |
| F1: a suspension splits the record | **closed.** The record lives on `Digital`, is reset only for a new instant, and is kept in `Mark` and `Checkpoint`. The settle gate uses the record of the whole instant. Two engine tests (`a_pulse_split_by_a_host_suspension_is_still_passing`, `a_part_after_the_pulse_leaves_the_settle_phase_to_the_owner`) cover both of my failure scenarios. |
| F2: two loops, inconsistent retakes | **closed.** There is one loop, `read_instant`/`read_round`. A retake happens until a zero-width solve has been made, and a zero-width solve after that. The spec states it. |
| F3: a glitch gains area | **closed.** A zero-width solve replaces the retake. `a_zero_delay_glitch_gains_no_area_at_any_step` checks two step caps, both `< 1e-12`. |
| F4: statistics | **closed** (`ZeroWidthCause`, `zero_width_for`). |
| F5: doc line | **closed.** |

## New findings

### R2-1. HIGH: after `reopen`, a single change counts as passing the value already read. A zero-width solve then reads a stale value and the change goes unread at `t`

`crates/piperine-digital-solver/src/passing.rs` (`consume`) clears `passed` but keeps `held`.
`Digital::reopen` (`engine.rs:432-436`) calls it before every continued round of `read_instant`. Suppose a net
changed once in round 1 (held at v1, which the retake or the zero-width solve of round 1 read) and changes once
more, to v2, in the continued round. `PassingValues::changed` then records v1 as passed. Round 2 reports
`passing = [(net, v1)]` for a net that passed through nothing in that round.

I reproduced this with a temporary engine test, since reverted. A Pulser drives `clk`, a `Dff` with `d = !q`
toggles `q`, and `q` is read by the analog side. Round 1 is `fire(go)`. Round 2 is `reopen(7, true)` followed by
`fire(go)`:
```
round1 q=[1] boundary=[(q, [1])] passing=[]
round2 q=[0] boundary=[(q, [0])] passing=[(q, [1])]   <- q changed once; reported as passing v1
```
Consequence in `protocol/retake.rs` (`Readings::next`): `passing` takes priority, so the round becomes
`ZeroWidth([q = 1], PassedValue)`.
- It re-reads the value already delivered and does not read `q = 0` at `t`.
- That breaks the rewritten rule 4: "When the reactions of that crossing … change a carrier … another zero-width
  solve at `t` reads it".
- It **regresses the jump-case retake already on main**. There, `retake_step` retook the step with the round-2
  change. Now, for a step that read a jump with a crossing whose reactions change an analog-read net twice over
  two rounds, the second change reaches the solve only at the next step.
- The statistics also count it as a `PassedValue` solve.
- The protocol tests use stub sides, so nothing covers this path at the engine level.

Fix direction: once the owner has read the record, `consume` should forget `held` as well, so only what a later
round passes through is passed. Add an engine test of a single change after `reopen`, and a multi-round protocol
case.

### R2-2. LOW: the "zero-width" solve has a finite width, while the spec says no time passes

`device_zero_width` (`crates/piperine-solver/src/analyses/transient.rs:1838-1878`) runs the trapezoidal phase
with `prev_h = 0` and `JumpPosition::Reads`. That gives `jump_coeffs`, so it is backward Euler over `γh =
1e-6 tick` (about 1e-21 s at a 1 fs tick). It is an O(W) approximation of the algebraic solve with states held,
and it touches no device type, so it is not a hidden special case. The stack's own conventions apply: `slew` is
limited by its rate times W, and the `ddt` history row is replaced through the normal accept.

I stress-tested the conditioning with a temporary MSL bench, since deleted. A 1 mF capacitor at 100 V through
1 Ω gives a companion conductance of about 1e18. An `idt` with `ic = 1000` and a 1 MV/s integrand gives a unit
`ddt` of about 1e21. A zero-delay glitch at 1 µs, at step caps of 50 ns and 200 ns: both converged, with one
zero-width solve each. The states drifted by at most about 1e-15 relative: 1000.0000000000007 and
100.00000000000009. There was no spike at `t`.

So it is sound in practice. However, the spec text ("a solve at `t` in which no time passes") and
`AnalogSide::zero_width`'s doc ("with no time passing") overstate what the code does. The code's own doc gives
the width. Either state the width in §15.2 rule 4, or replace it with an algebraic solve. An algebraic solve is
not needed for correctness at this conditioning.

### R2-3. INFO: checked and sound

- **`Delayed::push`** (`crates/piperine-analog-solver/src/device/delay.rs:28-33`) replaces a point at the same
  time. It is reached through `assembly.rs:247` (`accept`) with `amb.time = origin + current_time`, the same
  expression the normal accept uses, so exact equality holds. Without the replacement there would be two
  history points at one time, a zero-length interpolation segment.
- **The `ddt` history** gains a second row at `t`. Only the next step uses it, as its first lookback, and that
  step reads a jump and uses first-order `jump_coeffs`. By the step after, the duplicate row is beyond the
  lookbacks it reads.
- **LTE:** the zero-width solve is not tested for truncation error. The next step reads a jump
  (`sets_just_applied`), which keeps the jump LTE exemption.
- **Crossings:** the watches compare against the solution at `t`. A crossing the zero-width solution makes fires
  at the next microstep, and nothing is located, since the solve is not an attempt.
- **Operating point:** `EquilibriumSide::zero_width` is unreachable there, because `tracks_passing` is false in
  a static round.
- **Performance:** I counted zero-width solves on the closed-loop buck (`LoopBench`) with a temporary test. Over
  40 µs: 1 solve (PassedValue), 0 CrossingChange, 0 retakes, 11495 accepted steps, 402 instants. Over 200 µs:
  still 1 solve, against 64481 steps and 1118 instants. The count does not grow with the digital activity.
- **Determinism:** the record is a `BTreeMap` throughout.
- **Code rules:** `Reading`, `Round` and `Readings` have methods and doc comments, the names are full words, and
  `read_instant` is about 35 lines. `read_round` is a two-arm match. No free functions were added.

## Suite (round 2)

| Gate | Result |
| --- | --- |
| Targeted runs: the round 1 tests, plus scratch tests for R2-1 (red, the defect) and R2-2 and the buck count (green), all removed afterwards | as above |
| Full suite | not run: FAIL |

---

# Round 3 (head `deea3204d`): PASS

Fixes reviewed: `911f27711` (R2-1: `consume` now clears `held` as well as `passed`, so a `reopen` starts the
record from the values the nets hold) and `deea3204d` (R2-2: the spec, the docs and MD-84 now describe the solve
as a backward-Euler step of width ε = 10⁻⁶ tick).

- **R2-1 closed.** I re-ran my R2-1 reproduction (temporary, reverted): a Pulser clocks a Dff with `d = !q`, and
  `q` is read by the analog side. Round 2, after `reopen`, now gives `boundary=[(q, 0)]` and `passing=[]`
  (before the fix: `passing=[(q, 1)]`). The branch's own engine test covers it, as does the real-sides source
  test `wraps/rounds.msl`, which counts exactly one zero-width solve per cause per wrap (PassedValue == 3,
  CrossingChange == 3).
- **R2-2 closed.** §15.2 rule 4 and its Resolution now give the ε width and why it is not a solve with every
  state fixed, and they match `device_zero_width`.
- No new findings. `origin/main` has not moved (`7cf35a4f7`), so nothing was merged.

## Final full suite (one run, `PIPERINE_NGSPICE` unset, lane B target)

| Gate | Result |
| --- | --- |
| `build --workspace --exclude piperine-lang-server --all-targets` | builds. **1 warning, already on main:** `constant NS is never used` (`crates/piperine-parity/tests/buck_whole.rs:29`). The file is identical on `origin/main` (last touched by `ffe17a87a`), so this branch did not cause it. Red against the zero-warning bar; not fixed here. |
| piperine-abi | 58 passed, 0 failed |
| piperine-numbers | 22 passed, 0 failed |
| piperine-plan | 67 passed, 0 failed |
| piperine-interp | 72 passed, 0 failed |
| piperine-digital-solver | 165 passed, 0 failed |
| piperine-analog-solver | 110 passed, 0 failed |
| piperine-project | 53 passed, 0 failed |
| piperine-lang | 1504 passed, 0 failed |
| piperine-ir | 162 passed, 0 failed |
| piperine-transpile | 170 passed, 0 failed |
| piperine-solver | 404 passed, 0 failed |
| piperine-api | 305 passed, 0 failed |
| piperine-cli | 205 passed, 0 failed |
| piperine (root) | 422 passed, 0 failed |
| piperine-parity `--nocapture` | 588 passed, 0 failed |
| the seven buck targets, release, `-j 1` | 23 passed, 0 failed (closed loop: mean v(vout) 0.802938 V, 24 s wall clock) |
| buck_scenarios, release, `-j 1` | 61 passed, 0 failed |
| cli twins | 6 passed, 0 failed |

`target-b/release` was deleted after the run.

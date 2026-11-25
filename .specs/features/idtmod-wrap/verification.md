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

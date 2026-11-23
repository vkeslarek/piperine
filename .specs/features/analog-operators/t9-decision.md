# T9 decision: `slew` on an ideal step (AO-13, claim C64)

**Status**: decided 2026-11-20 by the planner. The user was away and asked for the most reasonable spec-following default ("Tu sempre assume o default mais razoável e depois me conta"). This file replaces the open question in the "Settled mid-build" line of `tasks.md` (cf4ce04d). The user should confirm it later (see "To confirm").

## The defect

After the 0 to 1 V ideal step, the stdlib `slew` ramps down at about 1 V/µs and never catches up. The cause is in the integrator, not in `slew`.

The step that reads a jump (in the code, the first step after a delivery: `Restart::Integrator`, `prev_h = 0`, `sets_just_applied`) is a TR-BDF2 step whose state history is discontinuous. Here `q_n = 0` (the left value) and `q_{n+γ} = q_{n+1} = 1` (the right value). The TR stage restarts as backward Euler over `γh`, so `i_{n+γ} = 1/(γh) > 0`. The BDF2 stage then gives

`i_{n+1} = c0·q_{n+1} + c1·q_{n+γ} + c2·q_n = −c2 = −(1−γ)/(γh) ≈ −0.707/h`.

That is −7.07e8 for a 1 ns step. The step that read the rising jump ends with a **negative** derivative. BDF2 differentiates the quadratic through `(t_n, 0)`, `(t_{n+γ}, 1)` and `(t_{n+1}, 1)`, and that quadratic has a falling slope at its end. Events and reactions read the end of a step, so `above(-dx - f)` fires and `slew` enters `Down`.

The same artefact also affects regular steps. The TR stage of the step *after* the jump step does not use the stored derivative of the step before it. It re-derives that derivative with the BDF2 coefficients over the last three rows (`device/ddt.rs` `companion_current`). If the jump step's second stage changes, that re-derivation recreates the old value unless it changes too. Probe 1 below shows this.

## Decision: an engine fix (direction 1, narrowed)

**The step that reads a jump is first order from end to end, and the derivative it ends with is the step's secant.** Two changes:

1. **The jump step**: in the step that reads a jump, the second stage runs as backward Euler over the whole step from the step's start, with coefficients `(1/h, 0, −1/h)` on `(q_{n+1}, q_{n+γ}, q_n)`. The derivative at its end is `(q_{n+1} − q_n)/h`, the secant of the step. The TR stage keeps the existing restart (backward Euler over `γh`).
2. **The step after it**: its TR stage subtracts the derivative the jump step actually ended with, which is the stored companion current `History::current_back(1)`. It does not re-derive that derivative with the BDF2 coefficients `phase_coeffs(Bdf2, prev_h)`.

All other steps are unchanged. That includes the first step after the operating point, which reads no jump and whose history is smooth, every regular step, and every restart that no delivery causes.

### Why this direction

- **§14.5.4 already asks for it.** "Read in a reaction or an event argument, the result is … the derivative of the step that read the jump." A value of the opposite sign to the step's own change is not "the derivative of the step"; it is an interpolation artefact. The secant is the only derivative that belongs to the step as a whole. §15.2 rule 1 ("to the accuracy of the step") also excludes a sign error.
- **It is ngspice's discipline.** After a breakpoint ngspice drops to order 1 and the step is backward Euler (`dctran.c`, `CKTorder = 1`), so its derivative at the end of that step is `ΔQ/h`. §15.2 rule 6 already cites `dctran.c` for the size of the same step. The change moves the step that reads a jump closer to the parity reference.
- **It moves no ngspice parity value.** Probe 3 passes `parity_op`, `parity_dc`, `parity_ac`, `parity_tran` (including `tran_jump` and `tran_corner`, compared to the installed `/usr/bin/ngspice`, with no SKIP) and `parity_corpus`. It also passes both `twins` targets, `source_derivative` (the closed-form `bounded` included) and every `buck_*` target of `piperine-parity`, all with their constants unchanged.
- **It keeps D16 and the AO-13 design whole.** Design §7 rejected a stdlib-only fix because "a stdlib fix would not cover the next model with the same pattern". Any model that reads `ddt` of a jumped state in an event (a comparator on a capacitor current, a `slew`-like limiter) has the same sign error today.

### Rejected

| Direction | Verdict |
| --- | --- |
| (1) as attempted on cf4ce04d: backward Euler for the BDF2 stage at every `prev_h = 0` | Rejected. It also changes the first step after the operating point, which has no jump, and that moves `assembly_ddt`, `options` and 3 `twins` targets. It also kept the BDF2 re-derivation in the next TR stage (see probe 1). |
| (2) the direction from the sign of the input's change across the step (`x(t_n) − x(t_{n−1})`) | Rejected. A reaction reads only the latest solution, where `x` and `pos(y)` are both the right value. The left value can only be held in a `Bit` updated at events, and no event happens before the jump. Holding it would need a new "hold" mode and a second retake round (hold, then pick the direction from `x − pos(y)`). That adds state and a retake to every jump, and it fixes `slew` alone. |
| (3) `slew` reading the input's delta at its own `cross` | Rejected for the same reason: a crossing reads the solution after the step, and the step's left value is not there. |
| (4) leave T9 as a `GAPS.md` entry | Rejected. AO-13 is an accepted P2 requirement (user, 2026-11-19), and the earlier user decision gave the ideal step to AO-13. A gap entry would leave C64 unmet. |
| Sign of `input − output` alone (cf4ce04d (b)) | Rejected: in `Track`, `pos(y) = x`, so the sign is 0 when the reaction runs. |
| Feed events a secant `ddt` while the stamped companion stays BDF2 | Rejected. The same `let dx = ddt(x)` would have two values, one for `analog` and one for events, and that needs a codegen split. Not minimal. |

## Spec text to amend

Each wording below replaces the text it names. `crates/piperine-lang/tests/spec_text.rs` keeps passing: `jump_has_no_solve_of_its_own` needs "backward Euler" and "drops the previous derivative" in rule 6, and `part_v_states_the_solver_hygiene_rules` needs the restart-step sentence. Both stay. Add one function, `the_step_that_reads_a_jump_is_first_order`, that asserts the two new sentences of §15.2 rule 6 and the new §14.5.4 sentence.

**Part IV §14.5.4** (`docs/spec/part_iv_behavior.md`). Replace the bullet with:

> - **§14.5.4** Read in a reaction or an event argument, the result is the derivative in the
>   solution read: 0 at an equilibrium solution; after the step that read a jump, that step's
>   derivative, which is the change of `x` across the step divided by the step (§15.2.6), so it
>   has the sign of the change.

**Part V §15.2 rule 6** (`docs/spec/part_v_execution.md`). Replace the sentence "Restarting, the trapezoidal stage drops the previous derivative and runs as backward Euler over its sub-step." with:

> Restarting, the trapezoidal stage drops the previous derivative and runs as backward Euler
> over its sub-step. **The step that reads a jump is first order throughout: its second stage
> also runs as backward Euler, over the whole step from the step's start, so the derivative it
> ends with is the change of each state across the step divided by the step. The step after it
> takes that derivative as its previous derivative.** The first step after the operating point
> reads no jump and keeps its BDF2 stage.

The other sentences of rule 6 stay as they are: the restart triggers, the 10⁻³ size after a host `set`, the bold step-size sentence and "In every other case …".

**Appendix C.2, row "restart (§15.2)"** (`docs/spec/appendix_c_implementation.md:80`). After "backward Euler over the `γh` sub-step)", insert:

> ; the step that reads a delivered jump (`sets_just_applied`) also runs its second stage as
> backward Euler over the whole step (`Stage::jump`, `TrBdf2::jump_coeffs`: `(1/h, 0, −1/h)`), and
> the trapezoidal stage of the step after it subtracts the stored derivative of that step
> (`History::current_back(1)`) instead of re-deriving it with the BDF2 coefficients, as ngspice
> runs the step after a breakpoint at order 1 (`dctran.c`)

**Appendix F.5, "The step" row** (`docs/spec/appendix_f_gallery_mixed.md:366`). Replace "The BDF2 stage has the same form over the charges of `t − h`, the stage point and `t`, and conserves the same sum." with:

> The second stage, backward Euler over the whole step from the charges of `t − h` (§15.2.6),
> has the same form and conserves the same sum.

The 0.25 V and the conservation row do not change, because both stages conserve charge.

**Part VII §22.2.10** stays unchanged. Its rule 2 ("flipped by located crossings", "the result is exact") now holds for the ideal step as well.

## Code changes

`crates/piperine-analog-solver/src/math/integration.rs`:
- `TrBdf2::jump_coeffs(phase, h) -> (f64, f64, f64)`: `Trapezoidal` gives `(1/(γh), −1/(γh), 0)` (the existing restart) and `Bdf2` gives `(1/h, 0, −1/h)`. Its doc cites §15.2.6. `stage_coeffs` and `phase_coeffs` are unchanged, so `assembly_ddt` and the `integration.rs` unit tests keep their hand-derived values.

`crates/piperine-analog-solver/src/device/ddt.rs`:
- New `pub enum JumpPosition { None, Reads, After }`: where the step stands relative to a jump. `Stage` gains `pub jump: JumpPosition`.
- `impl Stage { fn coefficients(&self) }`: returns `TrBdf2::jump_coeffs` when `jump == Reads`, otherwise `TrBdf2::stage_coeffs`. `companion_current` and `stamp_ddt` both call it.
- `companion_current`'s previous-derivative correction (a TR stage with `prev_h > 0`): with `jump == After` it subtracts `hist.current_back(1)`; otherwise it re-derives the derivative as today.
- Every `Stage { .. }` literal in the tests gains `jump: JumpPosition::None`: `assembly_ddt.rs`, `assembly_delay.rs`, `assembly_noise.rs` and `boundary.rs` of `piperine-analog-solver`.

`crates/piperine-solver/src/analyses/transient.rs`:
- `TransientSystem` gains `jump: JumpPosition` (fed into `Stage` by `assemble_device`) and `last_read_jump: bool`.
- `device_attempt`, just before `execute_timestep`: `jump = Reads` if `st.sets_just_applied`, else `After` if `last_read_jump`, else `None`.
- `device_accept`: next to `self.system.prev_h = dt_actual`, set `self.system.last_read_jump = st.sets_just_applied`.
- **`last_read_jump` is time-loop state.** It goes into `AcceptUndo` (restored by `device_unaccept`) and into `core::result::AnalogResume` (`new`, a getter, and the restore at `transient.rs:1350`). Probe 3 left it out, and `piperine-solver` `transient_reentry::a_reentry_from_every_recorded_step_continues_bit_for_bit` failed (158 against 160 steps). That is a defect of the probe, not a moved value.

No change to `std/analog.msl`, the protocol (`protocol/transient.rs`, `retake.rs`), the plan or the ABI.

## What moves, and why it is legitimate

Probe 3 (the two changes, without the resume state) ran the whole default-member suite. These are its only differences from HEAD:

| Target | Before | After | Why it may move |
| --- | --- | --- | --- |
| `piperine-parity` `source_slew::slew_limits_an_ideal_step_to_its_rate` (new, T9) | fails (ramps down) | passes | AO-13 AC 1 |
| Root `book_welcome::the_delayed_bench_steps_at_one_microsecond` and the cli `book` listing `piperine run benches/delayed.msl …` | 2 µs: 6.321468e-1; 3 µs: 8.646881e-1; 4 µs: 9.502266e-1; 5 µs: 9.816913e-1; record at 1.999927e-6 | 6.321445e-1; 8.646873e-1; 9.502263e-1; 9.816912e-1; record at 1.999933e-6 | The RC step response, whose closed form is `1 − e^{−k}` = 6.321206e-1, 8.646647e-1, 9.502129e-1, 9.816844e-1. **Every printed value moves toward the closed form.** This is not an ngspice value. The Book listing and `tests/book/welcome.rs`'s printed number are regenerated from the binary. |
| cli `book::the_buck_block_is_what_the_binary_prints` (the `piperine run startup` listing), cli `buck::startup_ends_within_3_mv_of_0_8_volts` | last `vout` 0.8025 | 0.8026 | The buck reads a jump at every switching edge. The claim it backs ("within 3 mV of 0.8 V") still holds, and only the printed digit moves. This is not an ngspice value. Regenerate the listing in `docs/book/content.html`, the `"0.8025"` in `crates/piperine-cli/tests/buck.rs` and the "(0.8025)" prose of chapter 16. |

Nothing else moved. All ngspice parity targets are unchanged and green: `parity_op/dc/ac/tran/corpus`, the twins (both crates), `source_derivative` (closed-form `bounded`), `buck_*` of `piperine-parity`, `jump_retake`, `assembly_ddt` and `options`. These move only because the spec, amended as above, now requires the secant derivative of the step that reads a jump. Nothing listed is ngspice-checked and no tolerance changes. `book_cli::each_mistake_of_the_run_table_is_p0010` fails on HEAD as well, without the probe. It is pre-existing, out of scope for T9, and reported here only so it is not blamed on this change.

### The probes (all reverted)

1. **Jump step only.** BE second stage, with the BDF2 re-derivation kept in the next TR stage. T9 passed, but `source_derivative::bounded_holds_its_state_at_the_bound` failed on its closed form: 0.499448 against 0.5 at 2.5 s, which is outside `TRAN_VOLTAGE` (5.01e-4). The next step's TR stage re-derived the jump step's derivative with BDF2 coefficients over rows that a BE stage had produced. On HEAD the same test is off by +3.3e-4 for the same reason, which passes. `book_mixed` "the error is under 1 mV at about 7 ms" also failed. Change 2 exists because of this probe.
2. **Stored derivative in every TR stage.** T9 and `bounded` passed, but `assembly_ddt::a_ddt_term_stamps_the_capacitor_companion` (hand-derived) moved. That change is too broad, so it is narrowed to `After`.
3. **The decision.** Results are in the table above. The probe diff (without the resume state) is saved at `/tmp/claude-1000/-home-keslarek-Git-piperine/570f7032-f229-4af2-937c-a6378b382406/scratchpad/t9-probe-v3.diff`. That location is not durable; the code section above is the record.

## Task changes

- **T9** gains the engine work above, and its Where becomes: `crates/piperine-analog-solver/src/{math/integration.rs,device/ddt.rs}`, `crates/piperine-solver/src/{analyses/transient.rs,core/result.rs}`, the four `piperine-analog-solver` test files, `crates/piperine-parity/tests/source_slew.rs` and `crates/piperine-parity/tests/operators/slew_step.msl`. It also covers the spec amendments, the `spec_text.rs` function, and the Book and test regeneration for the three moved values. Executor: Sonnet. Verifier: Opus.
- Done when: `slew_limits_an_ideal_step_to_its_rate` passes, the gates below pass, and the moved values are exactly those in the table.

## Gate commands

Lane B env first:

```sh
export CARGO_TARGET_DIR=/var/tmp/piperine-target-b TMPDIR=/var/tmp/piperine-tmp-b CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piperine-parity -p piperine-solver -p piperine-analog-solver -p piperine-api -p piperine -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')
cargo test -p piperine-parity --test source_slew --test source_derivative --test parity_tran --test parity_op --test parity_dc --test parity_ac --test parity_corpus --test twins
cargo test -p piperine-analog-solver --test assembly_ddt --test assembly_delay --test assembly_noise --test boundary --lib
cargo test -p piperine-solver --test jump_retake --test transient_reentry
cargo test -p piperine-lang --test spec_text --test suite_hygiene
cargo test -p piperine-cli --test book --test buck --test twins --test options
cargo test --test book_welcome --test book_mixed
cargo test --no-fail-fast   # the whole default-member suite: only book_cli's pre-existing P0010 may fail
```

`PIPERINE_NGSPICE` stays unset, so the parity targets compare against the installed ngspice.

## To confirm with the user later

1. Moving the three Book numbers (delayed RC, buck startup 0.8025 → 0.8026) on the strength of the amended §14.5.4 and §15.2.6. The delayed RC moves toward its closed form.
2. The amended §15.2 rule 6 says the step that reads a jump is first order throughout. That matches ngspice after a breakpoint, but it is a Part V semantic change and not only an implementation detail.
3. The pre-existing `book_cli` P0010 failure on HEAD, which is unrelated and needs an owner.

## Appendix: the T9 test and fixture (kept here because they were uncommitted)

`crates/piperine-parity/tests/operators/slew_step.msl`:

```rust
//! An ideal step into the stdlib `slew` (analog-operators AO-13, Part VII §22.2.10): a source whose
//! value is a `Bit<f64>` carrier, set from 0 to 1 at 1 us, so the step has no edge at all.

/// Ideal resistor.
module Resistor(p: Electrical, n: Electrical) {
    r: f64 where r > 0.0;
}
analog for Resistor { i(p, n) +> v(p, n) / r; }

/// A potential that follows the carrier `level`.
module Follower(p: Electrical, in level: Bit<f64>) {}
analog for Follower { v(p, gnd) +> level.known_or(0.0); }

/// Sets the carrier to 1 at 1 us: 0 before, 1 after, nothing between.
module Stepper(out level: Bit<f64> = 0.0) {}
digital for Stepper { on at(1u) { level <- 1.0; } }

module SlewStepBench() {
    signal level: Bit<f64>;
    signal inp: Electrical;
    signal o: Electrical;
    let stepper = Stepper(level);
    let source = Follower(inp, level);
    let load = Resistor { r: 1k }(o, gnd);
}
analog for SlewStepBench { v(o) +> slew(v(inp), 1M, 1M); }
```

`crates/piperine-parity/tests/source_slew.rs` (drop the `DBG` loop when committing):

```rust
//! `slew` limits an ideal step, run from MSL source on the Rust backend (analog-operators AO-13):
//! the fixture `tests/operators/slew_step.msl` sets a `Bit<f64>` source from 0 to 1 at 1 us, with no
//! edge, into the stdlib `slew(v(inp), 1M, 1M)`. The expected values are the criterion's, with
//! the closed form beside each assertion, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::NodeIdentifier;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::build;

/// `v(o)` of `SlewStepBench` over a transient to `stop` with the step cap `step`.
fn output(stop: f64, step: f64) -> Waveform {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/slew_step.msl");
    let built = build(&Project::new(&path), "SlewStepBench");
    let out = NodeIdentifier::Anonymous(built.ir.node_at("SlewStepBench.o").expect("node o").id().index());
    let (_, result) = built.run_with(stop, step);
    let steps: Vec<_> = result.iter().collect();
    Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(o)")).collect())
}

#[test]
fn slew_limits_an_ideal_step_to_its_rate() {
    // AO-13: the step 0 to 1 V at 1 us has no edge; at 1 V/us the output is 0 V before 1 us,
    // `(t - 1 us) * 1e6` V between 1 us and 2 us, and 1 V from 2 us.
    let waveform = output(3e-6, 0.01e-6);
    for (time, wanted) in [(0.5e-6, 0.0), (0.9e-6, 0.0), (1.25e-6, 0.25), (1.5e-6, 0.5), (1.75e-6, 0.75), (2.5e-6, 1.0), (3.0e-6, 1.0)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("slew step", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}
```

## User confirmation (2026-11-22)

The user confirmed this decision and its consequences after the B1 to B3 verification:

- The Book numbers T9 moved are accepted and the Book is corrected to what the binary prints: "Numeros do Book mudaram então acerta o book".
- Part V section 15.2 rule 6 (the step that reads a jump is first order in both stages) is approved: "Ok".
- The settle/retake defect found by the verifier (Finding 3) is to be fixed: "Corrige".

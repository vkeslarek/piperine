# Solver ABI Analog verification

**Verdict**: PASS on the numerics; one documentation finding open (F1)
**Round**: 1
**Proofs**: `proofs.md` at 20d6b2db (25 pass); judged at eaf20e99 (docs-only on top)
**Verifier**: independent sub-agent (author != verifier)
**Range**: 503b587c..eaf20e99 (feature commits on `crates/piperine-analog-solver` and `.specs/`; the rest of the range is the solver-completeness/core merges)

Probes were a throwaway test target (`crates/piperine-solver/tests/zz_verify_probe.rs`, with a
temporary dev-dependency on `piperine-abi`), run and removed; the manifest and lock file were
restored. Source kept in the session scratchpad `verify-abi-analog/probe.rs`. Nothing is committed
but this file.

## The old path against the new one

The probe built one circuit twice - a 5 V source, 1 kΩ, a plain diode (`is` 1e-14, VT 0.02585) and
1 µF at the diode node - once from the genuine old `Element` code (`R`, `C`, `D` of
`crates/piperine-solver/tests/helpers/devices.rs`, called through `load_dc`/`load_transient` with a
real `DcAnalysisState`/`TransientAnalysisState`; the old `Vsrc` formula `branch_force`, inlined
because `Vsrc` needs `CircuitBuilder` to allocate its branch) and once from the ABI fixtures through
`Assembly`. Both ran on the unchanged `NewtonRaphsonSolver::solve_with_strategy` with
`DampedNewton` and the default tolerances.

| Probe | Result |
| --- | --- |
| Stamps at one iterate over a four-row history: DC, TR `prev_h = 0` (restart), TR `prev_h > 0`, BDF2 both | matrix and right side **bit-identical** in all five |
| DC operating point from zeros | **bit-identical** solution, 75 iterations each |
| 12 TR-BDF2 steps, h from 1e-6 to 5e-4, two restarts (`prev_h = 0`), two rejected steps restored with `snapshot`/`restore` beside `state_snapshot`/`restore_state` | worst relative difference 4.4e-17 (last bit), 5 of 12 steps bit-identical, Newton iteration counts equal at every step |
| Pattern of a limited diode + source + resistor over DC and transient solves at three iterates (zero, 5 V, near the root) | the same set every time, equal to `AnalogPlan::pattern()` |
| Limit veto at `x = 0`: offsets 1e-6, ±1.0000001e-6 | not active, active, active: the bound `1e-6 + 1e-4·abs(x)` is strict and symmetric |

The spec's promise (1e-12 relative, equal step counts; bit identity not promised) is met with five
orders of margin. `git diff 503b587c -- crates/piperine-analog-solver/src/{math,convergence.rs,tolerances.rs,config.rs}`
is empty; the crate's only non-`device` edits are `lib.rs` (`pub mod device`) and the
`piperine-abi` dependency. `cargo tree -p piperine-analog-solver --depth 1` shows `piperine-abi` as
its only workspace crate; no digital type is named under `src/device` (C1).

**The limited-diode test at reltol 1e-10 is legitimate.** At the default tolerances the probe
measured: limited diode from 5 V stops at vd = 0.69249850 (23 iterations, KCL residual 1.4e-6 A,
inside the default current tolerance on 4.3 mA); the plain diode stops at 0.69249057 from 0 V and
0.69249039 from 5 V. The plain diode alone, from two starts, already differs by 2e-7: the spread is
where Newton stops, not an assembly error. Under `tight()` the limited and plain solves agree to
1.1e-16 and both satisfy KCL to 1e-17. The test compares fixed points, which is what limiting must
not move.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `boundary.rs` `device_modules_name_no_digital_or_owner_item` | yes - scans every `src/device` file with a planted positive; `the_crate_depends_on_the_abi_crate_only` pins the manifest; `cargo tree` agrees |
| C2 | `assembly_flow.rs` `a_flow_term_stamps_its_norton_form` | yes - hand-computed diode Norton stamps at 1e-14, signs on both rows, the ground case |
| C3 | `assembly_ddt.rs` `a_ddt_term_is_silent_in_an_equilibrium_solve` | yes - zero matrix and right side, positions kept; `h ≤ 0` too |
| C4 | `assembly_ddt.rs` `a_ddt_term_stamps_the_capacitor_companion` | yes - recomputes `C::load_transient` from `stage_coeffs`/`phase_coeffs` over a three-row history, both phases with and without `prev_h`; the probe shows the stamps bit-identical to the old capacitor |
| C5 | `assembly_ddt.rs` `a_restore_returns_histories_and_limit_state` | yes - the moved stamps differ, the restored ones are equal; `LAST`/`AGE` come back; the facade test reruns a rejected step bit for bit |
| C6 | `assembly_limit.rs` `a_limit_reader_is_linearized_at_the_held_iterate` | yes - `g·p` and `g·(x − x̂)` by hand at 1e-14; `x̂ = x` reduces to plain Norton |
| C7 | `assembly_limit.rs` `an_active_limit_vetoes_convergence` | yes - both sides of the bound at x = 0.7, signs both ways; the probe adds x = 0 |
| C8 | `assembly_limit.rs` `owns_correction_gets_no_correction_and_keeps_the_veto` | yes - exact matrix, right side at 1e-14, veto on and off |
| C9 | `assembly_delay.rs` `a_delay_term_reads_its_history_at_t_minus_td` | yes - interpolation inside the history (weight 0 on the current value) and inside the current step (weight 0.5) |
| C10 | `assembly_delay.rs` `a_delay_beyond_its_max_fails_naming_the_term` | yes - names instance, term, origin; `td = max` passes and reads a pruned-but-kept point |
| C11 | `assembly_eval.rs` `a_clean_function_is_not_called` | yes - call counts on recording fixtures, one reader and one non-reader of the moved pin |
| C12 | `assembly_eval.rs` `a_non_finite_value_names_instance_cell_and_origin` | yes - NaN value, inf seeded partial, and an unseeded inf partial that must not fail |
| C13 | `plan.rs` `the_pattern_is_terms_times_partials` | yes - the limited diode reaches its pins through `PROPOSED` only, the cubic keeps positions whose partials vanish; DC only, the probe extends it to transient |
| C14 | `assembly_small_signal.rs` `small_signal_weights_each_operator` | yes - 1, `jω·b`, `exp(−jω·td)·c` at two frequencies, no cross terms |
| C15 | `assembly_noise.rs` `one_noise_input_on_two_terms_is_correlated` | yes - the shared input cancels to an exact 0, independent inputs add to 4kTR |
| C16 | `assembly_watch.rs` `a_var_watch_is_readable_at_an_accepted_solution` | yes - absent before accept, `gd` by hand after, unknown names `None` |
| C17 | `assembly_watch.rs` `a_cross_watch_fires_on_a_rising_sign_change` | yes - no previous value, `≤ 0 → 0`, `0 → > 0`, `> 0 → > 0`, a fall and a rise on the mirrored watch |
| C18 | `assembly_watch.rs` `accept_reports_the_smallest_cap` | yes - the minimum switches between the two watches; infinite with none |
| C19 | `assembly_watch.rs` `a_trace_record_survives_a_rejected_step` | yes - iteration numbers 0 and 1, the unwritten iteration absent, after a restore |
| C20 | `assembly_eval.rs` `ambient_inputs_carry_the_solve_coordinates` | yes - each ambient in each solve kind, by value |
| C21 | `assembly_eval.rs` `setting_a_discrete_slot_makes_its_readers_dirty` | yes - same bytes stay clean, new bytes rerun the reader only |
| C22 | `boundary.rs` `the_facade_offers_what_an_analog_side_needs` | yes - drives every listed method on an RC with a DAC through the real Newton loop, `1 − 1/e` to 1e-3, and a rejected step rerun bit-identical |
| C23 | `cargo test -p piperine-analog-solver --lib` (24 passed) | yes - re-run here: 24 passed, the library tests untouched |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 - design's `param_stamps` was not built and the "As built" line does not say so | design.md `device::assembly` interfaces list `param_stamps(&mut self, x, param, sink) -> bool`; `grep param_stamps crates/piperine-analog-solver/src/device` is empty; no spec criterion or task names it, so no check fails | add to the "As built" line that `param_stamps` is not built and moves to `solver-abi-integration` (the sensitivity path falls back to a finite difference until then) | |

Observations, not findings (each is the spec's own rule or integration's wiring):

- `push_stage` must be called at every converged solve where the Newton buffer gains a row (the
  initial-condition row included), or the `DDT` ring and `states.view(k)` drift apart; the probe
  shows them aligned when it is. Integration owns that wiring.
- A `DDT` term whose value reads a limit is recorded at `push_stage` as `v(x̂) + g·(x − x̂)`; with
  `pnjlim`, `x̂ = x` exactly once no limiting happens, so this equals `q(x)` at convergence.
- A `CROSS` value that lands at exactly 0.0 fires at the next accepted solution (`≤ 0 → > 0`), as
  AC 17 states. A landing that `solver-crossings` places exactly on the root then reports one step late.

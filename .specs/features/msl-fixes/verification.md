# MSL Fixes verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at c2510880 (85 pass); judged over `HEAD` f243e5bc, diff `4293de6b..f243e5bc`
**Verifier**: independent sub-agent (author != verifier)

Three silent defects and three checks whose tests pass with the claim false. Parity is untouched:
`git diff --exit-code 4293de6b` is clean over `contract.rs`, the `parity_*` targets,
`parity_baseline.rs`, `tests/ngspice`, the transpile goldens, the twin goldens and the buck
fixtures. `env -u PIPERINE_NGSPICE` runs of `parity_op/dc/ac/tran/corpus/grid` and
`parity_baseline` are green; the only `SKIP` line printed is the one `parity_grid`'s own
`a_skipped_reference_still_runs_the_twin` asks for with `Lookup::Skip`. No tolerance was retuned.
The buck targets (A2D's reactions 4 -> 2) and `boundary` pass unedited.

Mutations run in the tree, each reverted (tree clean at the end):

| Mutation | Result |
| --- | --- |
| `tolerance()` returns the default for every pin (`watch.rs`) | `cross_with_tolerances_runs`, `a_cross_with_ttol_fires_within_it` still pass |
| CAP watches ignored (`acc.cap` not lowered) | `bound_step_caps_every_step` still passes; `the_smaller_of_two_caps_wins` fails |
| `finish_ \|= v.known_or(true)` (an unknown `finish` ends the run) | all four FIX-01 tests still pass; a probe with an unknown `finish` ends at 1 ns |

Own examples run at `HEAD` (a throwaway `piperine-parity` target, not committed): 15 `f64` edge
values in a reaction (`-0.0.signum()`, `atan2` at `±0`/`-inf`, `rem_euclid` by a negative
divisor, `powi(0)` of NaN, `1.powf(NaN)`, `NaN.clamp`, `round` of halves, `powi` overflow to
`inf`, `fract(-inf)`, `hypot(-inf, NaN)`, `sqrt(-0.0)`) all equal Rust's bits; optional
arguments forwarded through a second function, named `Some(..)`, named `None`, and in an
`analog` block all give §3.2's values; `analysis(Ac)` inside a function called from analog is
true in AC; two distinct events firing in one microstep through the `fired` guard write one
record; a param `ttol` of `-1n` fails with `E0510` (the tolerance pin is written in a real run);
explicit enum discriminants and `#[repr]` stay loud `P0001`. Three examples broke (F1, F2, F3).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `source_fixes.rs` `finish_ends_the_transient_at_its_instant` | yes - last step at 1 ns, no record after it, `k` never 9 |
| C2 | `source_fixes.rs` `fatal_ends_the_transient_at_its_instant` | yes |
| C3 | `source_fixes.rs` `a_finish_field_false_or_unknown_runs_to_stop` | no - only `finish = false` is written; the fixture's comment claims an unknown record that is not there, and `known_or(true)` survives (F4) |
| C4 | `source_fixes.rs` `finish_ends_an_operating_point` | yes - the transient's operating point (`init()`), last step 0 |
| C5 | `records.rs` `a_run_ended_by_finish_gives_its_rows_to_the_instant` | yes - with and without a print grid |
| C6 | `run.rs` `run_prints_the_fatal_line_and_exits_1` | yes - exit 1, exact stderr line, table ends at 2 us |
| C7 | `commands.rs` `test_prints_fail_with_the_fatal_message` | yes - exact stdout and exit 1 |
| C8 | `commands.rs` `test_passes_a_run_ended_by_finish` | yes |
| C9 | `records.rs` `a_short_run_without_finish_still_fails` | yes |
| C10 | `types_modules.rs` `a_where_that_reads_a_later_param_is_e0318` | yes - code and span `d` for both values; `message.contains('d')` is vacuous (F8) |
| C11 | `types_modules.rs` `a_where_over_its_own_and_earlier_params_is_accepted` | yes - accepted; "evaluate it as today" is not asserted |
| C12 | `source_fixes.rs` `a_literal_flow_source_builds_and_runs` | yes |
| C13 | `source_fixes.rs` `a_literal_potential_source_builds_and_runs` | yes |
| C14 | `source_fixes.rs` `a_carrier_driven_current_builds_and_runs` | yes |
| C15 | `golden` + `twins` + `git diff --exit-code 4293de6b` | yes - re-checked: unedited |
| C16 | `source_fixes.rs` `a_cross_over_a_maybe_unknown_carrier_builds` | yes |
| C17 | `source_fixes.rs` `absdelta_fires_every_two_tenths_of_a_volt` | yes - five records within 1 ps |
| C18 | `assembly_watch.rs` `a_nan_cross_watch_is_unknown_and_never_fires` | yes - at the NaN and at the next solution; does not cover a NaN that is no unknown (F2) |
| C19 | `assembly_watch.rs` `an_infinite_cross_watch_fails_the_solve` | yes - both infinities |
| C20 | `source_fixes.rs` `left_out_optional_arguments_are_none` | yes |
| C21 | `source_fixes.rs` `a_named_optional_argument_binds_by_name` | yes - with the out-of-order edge case |
| C22 | `source_fixes.rs` `a_plain_value_for_an_option_is_some` | yes |
| C23 | `source_fixes.rs` `none_and_some_pass_positionally` | yes |
| C24 | `types_values.rs` `a_named_argument_that_is_not_optional_or_bound_is_e0140` | yes for the code; "naming it" is not asserted (F8) |
| C25 | `types_values.rs` `a_wrong_argument_count_is_e0141` | yes |
| C26 | `types_values.rs` `an_option_before_a_required_parameter_is_positional_only` | yes |
| C27 | `source_fixes.rs` `an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction` | yes |
| C28 | `types_values.rs` `comparing_an_enum_without_partial_ord_is_p0002` | yes - all four operators |
| C29 | `source_fixes.rs` `two_analog_blocks_act_as_one` | yes - through a param, not the literal of AC 1 (settled 1) |
| C30 | `elab_analog.rs` `two_analog_blocks_keep_their_locals_apart` | yes |
| C31 | `source_environment.rs` `bound_step_caps_every_step` | no - the sine steps under 17 us on its own; passes with every cap ignored (F6) |
| C32 | `source_environment.rs` `the_smaller_of_two_caps_wins` | yes - fails when caps are ignored and would fail on the larger cap |
| C33 | `source_environment.rs` `a_non_positive_bound_step_is_e0509` | yes for 0 and -1 us; the non-finite half of the check is untested (F7) |
| C34 | `elab_analog.rs` `bound_step_under_an_if_is_p0001` | yes - `if`, `match`, nested block |
| C35 | `source_environment.rs` `freq_follows_the_ac_sweep` | yes for `freq()` written in the block; a `freq()` in a function the block calls is 0 (F1) |
| C36 | `source_environment.rs` `freq_is_zero_outside_a_sweep` | yes |
| C37 | `source_environment.rs` `freq_in_a_reaction_is_zero` | yes |
| C38 | `types_analog.rs` `simparam_stays_p0001` | yes |
| C39 | `source_environment.rs` `analysis_dc_holds_in_an_op` | yes |
| C40 | `source_environment.rs` `analysis_transient_holds_in_a_transient` | yes - first and last point |
| C41 | `source_environment.rs` `analysis_ac_holds_in_an_ac_run` | yes - the nonlinear load separates the operating point from the sweep |
| C42 | `environment.rs` `a_noise_operating_point_carries_the_noise_flag` | yes |
| C43 | `source_environment.rs` `a_reaction_reads_the_analysis_kind` | yes |
| C44 | `source_fixes.rs` `cross_with_tolerances_runs` | yes for "type-check, build and run"; nothing shows the values reach the solver (F5) |
| C45 | `amended_shape.rs` `bound_step_is_a_cap_watch_and_cross_tolerances_are_pins` | yes - pins, watch fields and writer functions |
| C46 | `source_fixes.rs` `a_cross_with_ttol_fires_within_it` | no - the default `ttol` (1 ps) is inside 1 ns, so it passes with the pin ignored (F5) |
| C47 | `elab_digital.rs` `a_cross_tolerance_that_is_not_constant_is_p0001` | yes - carrier, `known_or`, local |
| C48 | `source_fixes.rs` `above_with_an_emit_writes_one_record_at_its_crossing` | yes - also catches a `fired` that is always true |
| C49 | `source_fixes.rs` `above_true_from_the_start_writes_one_record_at_zero` | yes |
| C50 | `source_fixes.rs` `two_disjuncts_of_one_microstep_run_the_body_once` | yes - both disjuncts on one event; my distinct-event probe also writes one record |
| C51 | `frozen.rs` `the_interpreter_refuses_each_new_form_by_name` | yes |
| C52 | `types_values.rs` `every_h2_row_types_with_its_signature` | yes - 32 rows |
| C53 | `source_catalogue.rs` `f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction` | yes on its points (one in-domain point per row, valid `clamp` only); a crossed `clamp` in analog departs from Rust (F3) |
| C54 | `rt::real::tests::the_differentiable_rows_agree_with_finite_differences` | yes - a real central difference, bound stated and derived; a hand oracle, not `verify_partials` as AC 3 words it |
| C55 | `rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one` | yes - and the divisor term of `rem_euclid` |
| C56 | `source_catalogue.rs` `a_row_over_an_unknown_value_is_unknown` | yes - unary, either side of binary, bool and integer rows, plus known controls |
| C57 | `types_values.rs` `a_method_outside_h2_on_f64_is_e0161` | yes |
| C58 | `types_integers.rs` `every_h1_row_types_with_its_signature` | yes - 13 on `i8`, 11 on `u8` |
| C59 | `source_catalogue.rs` `pow_wraps` | yes - through a function, not folded |
| C60 | `source_catalogue.rs` `saturating_rows_clamp` | yes |
| C61 | `source_catalogue.rs` `checked_rows_give_none_on_overflow` | yes |
| C62 | `source_catalogue.rs` `rem_euclid_by_zero_faults_naming_it` | yes |
| C63 | `source_catalogue.rs` `signed_abs_signum_rem_euclid_follow_rust` | yes |
| C64 | `types_integers.rs` `abs_and_signum_on_an_unsigned_integer_are_e0161` | yes |
| C65 | `source_catalogue.rs` `unwrap_and_expect_of_some_give_the_payload` | yes |
| C66 | `source_catalogue.rs` `unwrap_of_none_faults_naming_unwrap` | yes |
| C67 | `source_catalogue.rs` `expect_of_none_faults_with_its_message` | yes |
| C68 | `source_catalogue.rs` `unwrap_or_else_evaluates_its_default_for_none_only` | yes - a faulting default proves laziness both ways |
| C69 | `source_catalogue.rs` `map_and_and_then_bind_the_payload` | yes |
| C70 | `source_catalogue.rs` `or_takes_the_first_some` | yes - all four cases |
| C71 | `source_catalogue.rs` `then_and_then_some_follow_the_bool` | yes |
| C72 | `types_compound.rs` `expect_with_a_non_literal_message_is_p0001` | yes, loosely - the message is an `f64`, not a non-literal `&str` (F9) |
| C73 | `types_compound.rs` `a_method_outside_h3_h4_is_e0161` | yes |
| C74 | `source_catalogue.rs` `db10_and_db20_run` | yes |
| C75 | `source_catalogue.rs` `db10_and_db20_type_check_from_the_stdlib` | yes - reaction and analog |
| C76 | `book.rs` `the_cycle_counter_prints_three_pulses` | yes |
| C77 | `book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |
| C78 | `book.rs` `the_clipper_refusal_no_longer_names_sin` | yes |
| C79 | `docs.rs` `gaps_drops_what_msl_fixes_closed` | yes |
| C80 | `docs.rs` `the_roadmap_and_state_record_msl_fixes` | yes |
| C81 | `twins.rs` `every_twin_prints_the_bytes_it_printed_before_the_long_run` | yes - goldens unedited |
| C82 | parity targets + `parity_baseline` + `git diff --exit-code 4293de6b` | yes - re-run with `env -u PIPERINE_NGSPICE`, no skip but the explicit one |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (silent, high; FIX-10). `freq()` inside a `fn` that an `analog` block calls is the constant 0 in an AC sweep | `elab/functions.rs` `lower_function` builds `Scope::new` (`analog: false`), so `elab/analog.rs` lowers `freq()` there to `Real(0.0)`. Probe: `fn g(x: f64) -> f64 { x + freq() }`, load `v * (1m + g(v*0) * 1n)` under a 1 mA `ac_stim`: \|v\| = 1.0 at 1 kHz, 100 kHz and 10 MHz; the same load with `freq()` inline gives 0.999, 0.909, 0.0909 | lower `freq()` in a function to `Env::Freq` and let the plan read `FREQ` for analog callers (0 in discrete ones), or refuse `freq()` in a function by name (`P0001`) until it does; add the function case to `freq_follows_the_ac_sweep` | |
| F2 (silent, high; FIX-05, Q2). Every NaN `CROSS` watch is "unknown", not only an unknown carrier: a defect NaN that failed loud (A4) now never fires | `watch.rs` admits any NaN; design.md's risk row says "only the generated `known_or(NaN)` writes it from an unknown", which the solver cannot tell. Probe: `on cross((0.0 - v(a) - 1.0).ln())` (NaN at every solution, plain `R` watch) runs to its stop with no error and no firing; before this feature it failed the solve | in `print/cross.rs` write NaN only for `!known_`; a computed NaN (plain watch, or a known flagged value) writes an infinity or faults, so A4 still fails it; add a test with a plain `cross` over a NaN expression | |
| F3 (silent, medium; CAT-01 AC 2). `f64::clamp` with `lo > hi` in an `analog` block gives `hi`; H.2 and the spec's assumptions say it panics, and the reaction path does panic | `rt/real.rs` `Real::clamp` is `min(max(x, lo), hi)` for `f64` and `Dual`; a reaction calls the inherent `f64::clamp`. Probe: `x.clamp(lo, hi)` with params 1.0, 2.0, 0.5 drives 0.5 mA and the operating point solves (0.5 V) | `Real::clamp` panics on `lo > hi` (and on a NaN bound, as Rust) naming `clamp`, like the integer `clamp`; add the crossed case to C53's design as a fault | |
| F4 (C3). The "unknown" half of FIX-01 AC 3 is untested | `fixes/finish.msl` `Idle` writes only `finish = false` though its comment names an unknown record; mutation `known_or(true)` passes every FIX-01 test, and a probe with `let u: bool = Unknown; emit!("sim", finish = u);` then ends at 1 ns | add the unknown `sim` record to `Idle` (or a second bench) and keep the 4 ns and `k` = 9 assertions | |
| F5 (C44, C46). No test shows a `cross` tolerance reaches the solver | mutation: `tolerance()` returning the default for every pin leaves both tests green; the RC test's 1 ns bound is wider than the default 1 ps | add an end-to-end case whose outcome depends on the pin, e.g. a param `ttol` of `-1n` failing with `E0510` (verified to fail today, so the pin is written), and a `ttol` loose enough to move the located instant measurably | |
| F6 (C31). `bound_step_caps_every_step` passes with every cap ignored | mutation in `watch.rs` (cap not lowered): C31 green, C32 red; the test's own comment says the sine steps under 17 us unaided | make the cap binding (a cap below the sine's own step, or assert against the same run without `bound_step`) | |
| F7 (C33, low). A non-finite `bound_step` is untested | only `BadCapBench` (0) and `NegativeCapBench` (-1 us) | add a cap of `1.0 / 0.0` and of NaN expecting `E0509` | |
| F8 (C10, C24, low). Two assertions do not hold "naming it" | `types_modules.rs` `message.contains('d')` is true of "reads"; `types_values.rs` E0140 test asserts the code only | assert the backticked name in the message | |
| F9 (C72, low). The non-literal `expect` message is an `f64`, not a `&str` | `o.expect(m)` with `m = 2.5` | use a `&str` parameter: `fn h(o: Option<u8>, s: &str) -> u8 { o.expect(s) }` | |
| F10 (broken, low; outside the checks). A zero-argument function called from `analog` fails `rustc` | probe `fn g() -> f64 { freq() }` used in a contribution: `error[E0283]: type annotations needed` at `g()` (`fn g<R: Real>() -> R`); same class as B1, not checked whether it predates this feature | print `g::<R>()` (or the block's type) for a call with no argument that fixes `R`, as T22 did for `terms_block` | |

Not judged: the Book's prose beyond the listings C76-C78 hold; `piperine run` of `op`/`dc`/`ac`
with a `finish` record (only the transient arm reads it, which FIX-02 asks for); integer
`rem_euclid(MIN, -1)` wraps to 0 where Rust's `rem_euclid` panics on overflow, which H.1's
"arithmetic wraps" reads as allowed.

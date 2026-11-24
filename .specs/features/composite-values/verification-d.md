# Composite Values verification (phase D, number widths, T91 to T126)

**Verdict**: PASS (round 3 at ce94b0e5: findings 1 to 6 are closed by commits, every proof passes - 50 rows, 47 checks - and the final full suite is green: round 2's run at d71404f6 was green but for `piperine-solver`, whose rerun at ce94b0e5 is 401 passed, 0 failed)
**Round 3**: re-verification at ce94b0e5 (fix ce94b0e5 for Finding 6: `crates/piperine-numbers` in `DEFAULT_MEMBERS` and `piperine-numbers` in `KEPT_CRATES` of `crates/piperine-solver/tests/workspace_shape.rs`, the two lines that finding named and nothing else); `cargo test -p piperine-solver --no-fail-fast` 401 passed, 0 failed; `proofs-d.md` rerun at ce94b0e5 (50 pass, 0 fail). `origin/main` is still a8b1f34f, merged.
**Round 2**: re-verification at d71404f6 (merge aebc4f0c of `origin/main` a8b1f34f; fixes d57714cc, 36120d5d, 52d52e6b, d71404f6); `proofs-d.md` rerun at d71404f6 (50 pass, 0 fail). `origin/main` has not moved since a8b1f34f, which is merged.
**Round 1**: at 023d47af: every proof passed, the independent oracles agreed, Findings 1 (major) to 5 (minor) were opened.
**Proofs**: `proofs-d.md` at ce94b0e5 (check_proofs.py over `tasks-d.md`: 50 pass, 0 fail, tree clean, Build and `suite_hygiene` green)
**Verifier**: independent sub-agent (author != verifier), Opus, 2026-11-23/24, branch `feature/composite-values-d` (round 1 at 023d47af over `3b2bb852..HEAD`; round 2 at d71404f6 over `7f3aaf0a..d71404f6`)
**Scope**: phase D only, checks C146 to C192 of `tasks-d.md`. The scripts read `tasks.md`/`checks.md`/`proofs.md`/`verification.md` of a feature directory, so they ran against an untracked scratch directory holding `tasks-d.md` as `tasks.md` and `checks.md` (and this file as `verification.md`); the results are copied here and into `proofs-d.md`.
**Gate**: `validate_verification.py` over the scratch directory at the round-3 proofs commit: 0 errors.

**Also checked at HEAD (targeted, per the gate policy):** `git diff 3b2bb852 HEAD -- crates/piperine-transpile/tests/golden crates/piperine-parity/src tests/support/ngspice.rs crates/piperine-abi` holds only what the merge of `main` brought; the four fix commits touch none of them. The twins pass (C190). `tasks-d.md` carries the user's approval (`"Sim. E aceito os outros"`, 023d47af).

## Round 2 (d71404f6)

**Finding 1 (d57714cc).** The lexer now refuses an infinite `f64` only for a suffixed float (`float_suffix` is non-empty: an SI suffix, or `f32`/`f64`, both of a fixed width of at most 64 bits). An unsuffixed literal is left to the checker. Typed by its position, `check_float_literal` reads it in that format (expr.rs:95-98). Untyped, the checker now reads it as an `f64` (expr.rs:500-506). Checked at HEAD:

- `1e400` as `f128` is `0x452fb4ec7f91973ff3cb1ccf26fbc178`, and the largest finite `f128` written as `1.18973149535723176508575932662800702e4932` is `0x7ffeffff…ffff`. My `Fraction` oracle gives the same two patterns.
- `1.1897314953572317650857593266280071e4932` is above max + half an ulp, so `E0909` is right for it. `1.2e4932` is also `E0909`.
- `1e999` stays `E0909` in every reached `f64` position I tried: a reached `const`, a param default, a function argument, a negated literal, an untyped `let` and the operand of `1e999 as f128`. In that last case the literal is an `f64`, so `E0909` is right.
- `1e999f64` is still a lexer `E0909`.
- The srcw vector table now draws its 50 rows per method from every row of each file, not only from the rows within `f64`'s range. `VectorTable::writable` is gone, and infinities and NaN are written as divisions. C153 passes in release.

A.2 rule 8 and D12 are reworded. One small mismatch in wording, not a finding: A.2 rule 8 and D12 name only the SI-suffixed literal as the lexer's, but the code also refuses an `f32`- or `f64`-suffixed literal at the lexer. The code `E0909` is the same and A.8 rule 8 requires it, so only the stage differs.

**The three moved tests (`msl_errors.rs`, `msl_lexer.rs`, `pom_reflect.rs`) follow the amended rule and are not weakened.**

- Each needs a *lexer-stage* `E0909`. `msl_errors` lists design-level fixture codes, `pom_reflect` needs a diagnostic on a design that has not been checked, and `msl_lexer` tests the lexer alone.
- Under the amended A.2 rule 8, an unsuffixed `1e999` no longer has a lexer error. The 310-digit `T` literal is the rule's own example of one that does.
- `msl_lexer` now asserts that `1e999` lexes clean, which is the amended rule, and keeps the `T` case.
- The `f64` refusal of an unsuffixed `1e999`/`1e400` that these tests used to carry is now asserted where it lives, in the checker: `a_literal_out_of_its_type_is_e0909` has the new rows `let y: f64 = 1e400;` and `let y = 1e400;`.

**One behaviour change, judged and accepted.** An unsuffixed `1e999` in an item no top reaches (`const A: f64 = 1e999;` alone, or `fn g() -> f64 { 1e999 }` never called) is now accepted, where the lexer used to refuse it. This is the checker's general rule, not something phase D added: at d71404f6, an unreached `const B: u8 = 256;`, `const B: f64 = true;` and `const B: f16 = 70000.0;` are all accepted the same way. It is no residue of L13.

**Finding 2 (d57714cc).** `an_f32_where_an_f64_is_required_is_a_mismatch` now asserts `v(p, n) +> a` as `E0417` and `v(p, n) > a` as `E0180`, each with its span, over a nature written in the fixture. Holds.

**Finding 3 (36120d5d).** The five free functions are gone. `integer_shape`, `is_wide_integer` and `is_number_width` are documented methods of `Ty`, `float_format` is `FloatText::format`, and the duplicate `is_other_float` is replaced by `Ty::is_other_float`. The round-2 diff adds no module-level function outside `#[test]`s. Holds.

**Finding 4 (52d52e6b).** `inferred_wide.msl` now drives its clock with `Osc`, and the test asserts that the first commit of `t` (which starts false) is `[1]`. Holds.

**Finding 5 (d71404f6 and d57714cc).** The doc comments of `Transpiler::number_widths` and `RuntimeForms::numbers` now include `f32` and say why. The `a_carried_f128` comment says `2.5 + 0.1`. Holds.

### Final full suite at d71404f6 (cleared `/var/tmp/piperine-tmp-a`, `PIPERINE_NGSPICE` unset, debug, package by package)

| Command | Result |
| --- | --- |
| `cargo build --workspace --exclude piperine-lang-server --all-targets` | exit 0, no warning, no error |
| `cargo test -p <package> --no-fail-fast` for abi, analog-solver, api, digital-solver, interp, ir, plan | exit 0; 58 + 109 + 292 + 159 + 71 + 155 + 64 passed, 0 failed |
| `cargo test -p piperine-solver --no-fail-fast` | **exit 101; 399 passed, 2 failed**: `workspace_shape::default_members_are_the_green_crates` and `workspace_shape::instructions_list_the_kept_crates` (Finding 6) |
| `cargo test -p <package> --no-fail-fast` for lang, numbers, transpile, project, parity, cli, piperine (root) | exit 0; 1327 + 22 + 164 + 53 + 491 + 191 + 415 passed, 0 failed |
| `cargo test -p piperine-parity -- --nocapture` | exit 0; 491 passed, 0 failed |
| the seven buck targets (`buck_whole`, `buck_dpwm`, `buck_control`, `buck_supervisor`, `buck_boundary`, `buck_closed_loop`, `buck_power`) | exit 0; 3 + 3 + 2 + 3 + 3 + 4 + 5 passed |
| `cargo test -p piperine --test buck_scenarios` | exit 0; 61 passed |
| `cargo test -p piperine-cli --test twins` | exit 0; 6 passed |

The workspace part printed 405 `test result` lines; the only failed ones are the two above.

**Round 3 (ce94b0e5).** The fix adds exactly the two list entries; no other file moved. `cargo test -p piperine-solver --no-fail-fast` (lane A env, `PIPERINE_NGSPICE` unset): 401 passed, 0 failed (`workspace_shape` 15 passed). The rest of the round-2 suite stands, the fix touching only that test file. The feature's final full suite is therefore green.

## Independent oracles (round 1, at 023d47af)

**`f128` reference vectors, recomputed without `mpmath`.** `piperine-numbers` is tested bit for bit against `tests/vectors/*.tsv`, which the implementer generated with `mpmath`. I recomputed a random sample of 120 rows each of `exp`, `ln`, `log10`, `log2`, `sin`, `cos`, `tan`, `atan`, `sinh`, `cosh` and `tanh` with my own throwaway script: Python's `decimal` module at 80 and then 200 significant digits (π by Machin's formula, sin and cos by Taylor series after reduction with as many extra digits as the argument has integer digits, atan by argument halving), rounded to binary128 by exact `Fraction` comparison, with a row counted only when both ends of a ±10^-75 relative bracket round to the same value. **1 351 rows decided, 0 mismatches** (the 29 skipped are rows whose result overflows, or where `decimal` raised). The sample includes `sin`/`cos`/`tan` at arguments up to the largest finite value. Directly from source, `const B: f128 = 1e300; p: f128 = B.sin()` folds to `0xbffeefca9749760d666d0c25320248d4`, the value my oracle gives. `1.0 / 3.0` gives `0x3ffd5555…5555`. `(2.0 as f128).sqrt()` gives `0x3fff6a09e667f3bcc908b2fb1366ea95`. Both are correct.

**`f16` transcendentals and `sqrt`, from the library itself.** I wrote a throwaway Rust probe that printed `Float16::{exp, ln, log10, sin, cos, atan, tanh, sinh, asin, sqrt}` for 3 000 pseudo-random finite inputs. The same `decimal`/`Fraction` oracle, rounding to binary16, decided **23 101 results with 0 mismatches**. It skipped the rows outside a function's domain, the zeros and the infinities.

**Wide integers and edges, from source (const folds):** `Int<127>` `(2^126 - 1) + 1` wraps to `-2^126`; `UInt<128>` `0 - 1` and `UInt<130>` `0 - 1` give all ones (`[…, 3]` for 130 bits); `Int<65>` `(2^64 - 1) * 2` gives `-2`; `Int<100>` `MIN / -1` is `MIN` and `MIN % -1` is 0 (§1.5.19); `-MIN` is `MIN`; `1 << 199` on `UInt<200>` sets bit 199 and `<< 200` gives 0 (§1.5.21); `-8 >> 2` on `Int<100>` is `-2`; `(2^200 - 1) as f128` is `2^200`; `(70000 as UInt<100>) as f16` is `+inf`; `(1e30 as f128) as UInt<100>` is the exact integer of `1e30f64`. All of these hold.

**Literal range (A.8 rule 8, QD5):** `f16` `65519.99` is `0x7bff`. `65520.0` is `E0909` "beyond the range of `f16`". This is correct: the midpoint rounds to infinity. `f16` `1e-8` is `+0` and `f128` `1e-400` is the nearest binary128 (`0x3ace2bfc…`), read from the literal's text. `f128` `1e400` and the largest finite `f128` `1.18973149535723176508575932662800702e4932` are both `E0909` "a float literal beyond the range of `f64`" (Finding 1).

**CV-29 AC 3:** an `f32` contributed (`v(p, n) +> a`) is `E0417` "expected `f64`, found `f32`" and an `f32` compared with a probe is `E0180`. Both hold. The `f64` control is clean. C148's test does not assert either case (Finding 2).

## Judged as asked (round 1, at 023d47af; Findings 1 to 5 are closed in round 2)

- **f128 transcendentals and f16 correctness**: confirmed by the independent oracles above.
- **The lexer limit on float literals (A.2 rule 8)**: it diverges from the spec (Finding 1). A.2 is the section on SI-suffixed reals, which are always `f64`, so its "nearest `f64`" wording binds those literals. A.8 rule 8 reads a float literal's value "of its type". §1 rule 7 says "a literal out of its type's range is `E0909`". QD5, which the user approved, says to read the range "per format". The checker already does that (`70000.0` as `f16` is `E0909`), so the lexer's `f64` test is a second rule, and a wrong one for `f128`.
- **`f16` 65504 printed as `65500`**: spec-consistent. QD6 asks for the shortest decimal that reads back in that format. The nearest `f16` to 65500 is 65504, at a distance of 4 against 28 to 65472, and no two-digit decimal reads back to it (66000 overflows, 65000 reads as 65024). The smallest subnormal prints as `6e-8`, which also reads back.
- **`Logic<N>` for N > 64**: `a_logic_of_a_hundred_levels_runs` runs `Logic<100>` and checks `& | ^ ! <<` level by level. That holds. The inferred `Logic<70>` test does not run (Finding 4).
- **L13 closed with no residue**: the `GAPS.md` row and its §9 hole are gone (C189), but Finding 1 is a residue that no row records.
- **Params of the new types**: `a_param_of_each_width_holds_its_exact_default` (srcw) commits `f32`, `f16`, `f128` and `UInt<200>` params through their own pin bytes. The analog side reads them through `pin_value` (`print/analog.rs:424`). This holds.
- **Byte-identical goldens**: `Runtime::files_for(RuntimeForms { numbers: false, .. })` is `Runtime::files(..)` (C186), and no golden moved in the diff. This holds. `NumberTypes::used_by` also counts `f32`, so a design with `f32` embeds the library. No design in the corpus uses `f32`, so nothing moves, but two doc comments still say "`f32` excepted" (Finding 5).
- **`piperine-numbers` crate rules**: there is no dependency (`num-bigint` is a dev-dependency only), no `unsafe`, no macro, no `panic!`/`unwrap`/`expect` and no module-level function in `src/`. The purity scan (C176) enforces this, and `grep` agrees. The only `assert!` is a compile-time `const LAYOUT` in `wide.rs:22`, and its fallbacks are total (`unwrap_or_default` on infallible divisions). This holds.
- **`[profile.dev.package]` opt-level = 3** for `piperine-numbers`, `num-bigint`, `num-integer` and `num-traits`: accepted. Dev builds keep `overflow-checks`, results do not depend on it, and the cost is a slower compile of one leaf crate in every dev build that links it (`lang`, `plan`, `api`, `transpile`). No other profile moved.
- **Commits that bundle tasks**: 6ad639c1 (T117 and T118), 57adf2aa (T119 and T120), 7a88cb3d (T123 and T124, plus the Book status-box text of T125) and 0b44e6a8 (T111, with the plan sources that T112 needed in order to build). These break the skill contract's "one atomic commit per task". They are disclosed in the Handoff, and the history is pushed. I record this as a process note, not a finding: a rewrite of pushed history costs more than the split would buy, and each commit's gate still ran.
- **Code cleanliness**: five module-level functions are new in library code (Finding 3). The other new files hold owner types with doc comments. I found no god method beyond the existing test benches (`FloatTable::bench` is long but is one table).
- **Tolerance and parity contracts**: `ToleranceContract` (`tests/support/ngspice.rs`) and `crates/piperine-parity/src` are unedited. This holds.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C146 | `source_number_widths.rs` `f32_computes_what_rust_f32_computes` | yes - every operator, comparison, H.2 method and `as` from source against Rust's `f32` computed in the test; const folds against the run |
| C147 | `source_number_widths.rs` `a_carried_f32_is_the_f32_leaf` | yes - pin is `(F32, 1)` and the commit is `(2.5f32 + 0.1f32).to_le_bytes()` |
| C148 | `types_number_widths.rs` `an_f32_where_an_f64_is_required_is_a_mismatch` | yes - round 2: adds the contribution (`E0417`) and the probe comparison (`E0180`); round 1 lacked them (Finding 2) |
| C149 | `source_number_widths.rs` `f16_is_ieee_binary16_bit_for_bit` | yes - exact rows by `ExactOracle` (independent of the library), transcendentals by the `f128` library with a near-midpoint guard |
| C150 | `half.rs` `every_unary_f16_operation_is_correctly_rounded_over_all_inputs` | yes - all 65 536 inputs against exact rationals |
| C151 | `half.rs` `binary_f16_operations_are_correctly_rounded` | yes - special pairs plus 2^22 random pairs (`rem_euclid`/`hypot` on 2^19), exact oracle |
| C152 | `transcendental.rs` `every_f16_transcendental_is_correctly_rounded_over_all_inputs` | yes - every input against the `f128` library rounded once, ambiguity guard; my 23 101-row `decimal` sample agrees |
| C153 | `source_number_widths.rs` `f128_is_ieee_binary128_bit_for_bit_in_every_operation_and_method` | yes - round 2: the vector rows are drawn from every row of each file, those past `f64`'s range included (round 1 filtered them out, Finding 1) |
| C154 | `quad.rs` `f128_arithmetic_and_square_root_are_correctly_rounded` | yes |
| C155 | `quad.rs` `f128_exact_methods_follow_h2` | yes |
| C156 | `conversions.rs` `every_as_conversion_follows_the_table_of_1_4_5` | yes - includes the double-rounding case 1 + 2^-11 + 2^-40 |
| C157 | `transcendental.rs` `exp_and_logarithms_match_the_reference_vectors_bit_for_bit` | yes - vectors independently confirmed (sample above) |
| C158 | `transcendental.rs` `powers_match_the_reference_vectors_bit_for_bit` | yes |
| C159 | `transcendental.rs` `circular_functions_match_the_reference_vectors_bit_for_bit` | yes - up to the largest finite argument; confirmed by my sample |
| C160 | `transcendental.rs` `inverse_circular_functions_match_the_reference_vectors_bit_for_bit` | yes |
| C161 | `transcendental.rs` `hyperbolic_functions_match_the_reference_vectors_bit_for_bit` | yes |
| C162 | `transcendental.rs` `the_reference_vectors_are_complete_and_well_formed` | yes |
| C163 | `source_number_widths.rs` `wide_integers_wrap_modulo_two_to_the_n_in_every_operation` | yes - `num-bigint` modulo 2^N |
| C164 | `integers.rs` `wide_integers_up_to_128_bits_agree_with_u128_and_i128` | yes |
| C165 | `integers.rs` `wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n` | yes |
| C166 | `types_number_widths.rs` `a_literal_out_of_its_type_is_e0909` | yes - round 2: `f64`/untyped `1e400`, `f128` past max + half an ulp are `E0909`; `f128` `1e400`, `-1e400` and the largest finite value are accepted |
| C167 | `source_number_widths.rs` `a_wide_literal_past_its_type_is_e0909` | yes |
| C168 | `number_widths.rs` (api) `the_host_reads_each_width_as_one_exact_value` | yes - `2^199 + 1` as one decimal |
| C169 | `run.rs` (cli) `number_widths_print_exactly_in_every_format` | yes - text, record, JSON Lines, CSV, plot and `--param` |
| C170 | `types_number_widths.rs` `a_width_outside_the_rules_is_e0103` | yes - widths 0, `Float<8>`, `Float<256>`, 2^32 and beyond, naming C.2 |
| C171 | `types_number_widths.rs` `every_float_and_integer_width_type_checks` | yes - includes const-expression widths (VD1) |
| C172 | `frozen.rs` `each_number_width_is_refused_by_name` | yes |
| C173 | `elab_number_widths.rs` `const_contexts_compute_every_width_as_the_run_does` | yes - folds against the embedded library; the run-side twin is srcw's `k_*` rows |
| C174 | `elab_number_widths.rs` `a_const_panic_of_a_new_width_names_the_expression` | yes |
| C175 | `elab_number_widths.rs` `number_widths_lower_to_their_ir_types_and_constants` | yes - `0.1` as f16 `0x2e66`, as f128 `0x3ffb…999a`, the double-rounding tie and above-tie literals |
| C176 | `purity.rs` `the_numeric_crate_is_embeddable_source` | yes |
| C177 | `integers.rs` `limbs_agree_with_big_integers_on_every_operation` | yes |
| C178 | `formats.rs` `rounding_to_every_format_is_nearest_ties_to_even` | yes |
| C179 | `formats.rs` `decimal_text_reads_to_the_nearest_value_and_prints_the_shortest` | yes |
| C180 | `precision.rs` `directed_rounding_brackets_the_exact_result` | yes |
| C181 | `precision.rs` `reference_constants_contain_their_published_digits` | yes |
| C182 | `precision.rs` `reduction_keeps_full_precision_for_every_finite_argument` | yes |
| C183 | `precision.rs` `ziv_rounding_raises_precision_until_the_bounds_agree` | yes |
| C184 | `number_widths.rs` (ir) `number_width_types_and_constants_are_checked_printed_and_serialized` | yes |
| C185 | `number_widths.rs` (plan) `each_new_scalar_has_its_leaf_count_and_bytes` | yes |
| C186 | `number_widths.rs` (transpile) `a_design_without_number_widths_gets_todays_runtime_files` | yes - with the unchanged goldens |
| C187 | `number_widths.rs` (transpile) `number_widths_compile_and_compute_as_their_library` | yes |
| C188 | `book/language.rs` `a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it`, `floats_of_every_width_and_wide_integers_run` | yes |
| C189 | `docs.rs` (cli) `gaps_drops_what_composite_values_phase_d_closed` | yes - with Finding 1 closed, no L13 residue remains |
| C190 | `cargo test -p piperine-cli --test twins` | yes |
| C191 | the Build line | yes - zero warnings |
| C192 | `rt/widths.rs` `widths::tests::the_runtime_raises_msl_panics_and_carries_every_width` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. **Major.** A float literal in an `f128` position whose `f64` value is infinite is refused, although its nearest `f128` is finite. `let x: f128 = 1e400;` and the largest finite `f128` written as a literal (`1.18973149535723176508575932662800702e4932`) are both `E0909` "a float literal beyond the range of `f64`". Every `f128` value above `f64::MAX` therefore cannot be written exactly in source. This diverges from A.8 rule 8 ("the value of its type nearest to the exact decimal number"), §1 rule 7, and QD5 as the user approved it ("read per format"). The Handoff knew about it ("an f128 literal is limited to the f64 range"). Instead of a fix or a `GAPS.md` row, the srcw vector table filters those rows out, so CV-31 AC 1 is proven only below `f64::MAX`, and C189's "no residue" is false. | `crates/piperine-lang/src/msl/lexer.rs:682` (`Token::Float { value, raw } if value.is_infinite()`). My probe at 023d47af: `fn q() -> f128 { 1e400 }` gives `E0909`. `source_number_widths.rs` `VectorTable::writable` drops the rows above `f64::MAX` | Lift the lexer's infinite-`f64` test from unsuffixed float literals. Keep it for `f64`-suffixed and SI-suffixed literals, which are `f64` by A.2 rule 1. Leave the range to the checker's per-format test (`types/widths.rs`), which already gives `E0909` for `70000.0` as `f16`. Amend A.2 rule 8's wording to "nearest value of its type" for non-SI literals. Add rows to C166's test: `let y: f128 = 1e400;` and the largest finite `f128` are accepted with their exact bits, `let y: f128 = 1.2e4932;` and `let y: f64 = 1e400;` stay `E0909`. Drop the `f64::MAX` filter from `VectorTable::writable` | d57714cc - C166 (`f128` rows), C175 (`1e400` and the largest finite `f128` lowered to their bits), C153 (every vector row from source) |
| 2. **Minor.** C148's test asserts the mismatch on an `f64` parameter, `f32 + f16` and `let` positions, but not the contribution or the probe comparison that CV-29 AC 3 and C148 name. The code refuses both (my probe: `v(p, n) +> a` with `a: f32` is `E0417`, and `v(p, n) > a` is `E0180`), but no test would fail if either were accepted | `crates/piperine-lang/tests/types_number_widths.rs:150-163` | Add the two rows to `an_f32_where_an_f64_is_required_is_a_mismatch`, over the real stdlib (an `Electrical` branch), asserting `E0417` and `E0180` and their spans | d57714cc - C148 |
| 3. **Minor.** Five module-level free functions are new in library code, against the user's code-cleanliness rule (no free functions) and MD-13 rule 2. `is_other_float` duplicates the method `Ty::is_other_float` that T115 added (`types/ty.rs:353`) | `crates/piperine-lang/src/elab/consts/numbers.rs:30-63` (`float_format`, `integer_shape`, `is_other_float`, `is_wide_integer`, `is_number_width`) | Move them to methods of `Ty` (or of an owner type in that file), and use the existing `Ty::is_other_float` | 36120d5d - C173, C175 (the evaluator after the refactor) |
| 4. **Minor.** `an_inferred_logic_of_seventy_levels_runs` only elaborates. `Bench` has no oscillator, so `on clk.rise()` never fires, and nothing asserts the `t <- a.is_high()` commit, although the test name and the fixture header ("it elaborates and runs … `is_high` of 70 ones commits true") claim it. T126 removed the refusal test that this one replaced, so the inferred `Logic<70>` path that L13's closure relies on has no run-time proof | `crates/piperine-parity/tests/source_logic.rs:329-334`, `crates/piperine-parity/tests/logic/inferred_wide.msl` | Drive `clk` (as `hundred.msl` does with `Osc`) and assert that the first commit of `t` is `true` | 52d52e6b - `source_logic.rs` `an_inferred_logic_of_seventy_levels_runs` asserts the commit (run in the final suite, parity green) |
| 5. **Minor.** Stale documentation. `Transpiler::number_widths` and `RuntimeForms::numbers` say "`f32` excepted / `f32` is Rust's own", but `NumberTypes::used_by` counts `F32`, and the Handoff says "a design that uses f32 embeds the numeric runtime". `a_carried_f128_is_a_u64_field_of_count_two` says "`1 + 1` committed reads back as the pattern of 2.0", but it asserts `2.5 + 0.1` | `crates/piperine-transpile/src/lib.rs:135-136`, `crates/piperine-transpile/src/rt.rs` (`RuntimeForms::numbers` doc), `crates/piperine-transpile/src/print/widths.rs:54`, `crates/piperine-parity/tests/source_number_widths.rs:631-632` | Make the doc comments say what the code does (`f32` included, or exclude `f32` from `used_by` if its `Carried`/`Convert` impls are not needed), and fix the test comment | d71404f6 (doc comments), d57714cc (test comment) |
| 6. **Major (final suite).** `piperine-solver`'s `workspace_shape` guard is red. Phase D added `crates/piperine-numbers` to the root `default-members` (T91) and `piperine-numbers` to the CLAUDE.md and AGENTS.md crate tables (T126), but did not update the guard's lists, so `default_members_are_the_green_crates` ("default-members drifted") and `instructions_list_the_kept_crates` ("CLAUDE.md crate table") fail. The batch gates ran only the touched crates' tests, and `piperine-solver` was not one of them | `crates/piperine-solver/tests/workspace_shape.rs:258` and `:340` (the lists `DEFAULT_MEMBERS` and `KEPT_CRATES`); `cargo test -p piperine-solver --test workspace_shape` at d71404f6 gives 13 passed, 2 failed | Add `crates/piperine-numbers` to `DEFAULT_MEMBERS` and `piperine-numbers` to `KEPT_CRATES`, as the leaf crate MD-79's phase D item records. Then rerun `workspace_shape` and the feature's final full suite | ce94b0e5 - `workspace_shape` `default_members_are_the_green_crates` and `instructions_list_the_kept_crates`; `cargo test -p piperine-solver` 401 passed, 0 failed at ce94b0e5 |

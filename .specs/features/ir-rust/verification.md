# IR to Rust Transpiler verification (phase A: T1 to T23)

**Verdict**: FAIL (phase A: 0 blocker, 3 major, 6 minor; no proof is red)
**Proofs**: the phase-A `Run` lines of tasks.md run by the Verifier at `244bb9e4` (rustc 1.98.0, ngspice at `/usr/bin/ngspice`, `PIPERINE_BACKEND` unset); `proofs.md` was not written for this round because `check_proofs.py` also runs the phase-B proofs, which do not exist yet
**Verifier**: independent sub-agent (author != verifier), Opus

## Proof runs at `244bb9e4`

| Run | Result |
| --- | --- |
| Build (the three `**Build**` commands, `grep '^(warning\|error)'`) | no warning, no error |
| plan | 2 passed |
| reject | 1 passed |
| lib | 18 passed |
| golden | 7 passed |
| rustc | 3 passed |
| abisrc | 2 passed |
| cache | 4 passed |
| load | 5 passed |
| compile | 8 passed |
| compiled | 4 passed |
| differential | 3 passed |
| residue | 2 passed |
| session | 2 passed |
| backend | 4 passed |
| parity (op 3, dc 2, ac 1, tran 2) | 8 passed |
| backends | 1 passed |
| contract | 15 passed |
| gate extras: `parity_corpus`, `suite_hygiene`, `workspace_shape` + `crate_boundaries`, `piperine-interp`, `piperine-plan` | all passed |

## Mutation runs (each reverted; tree clean afterwards)

| Mutation | Red | Green |
| --- | --- | --- |
| `rt::Dual::exp` value times 1.01 (compiled backend only) | `parity_dc::diode_iv` with `backend: rust` in the rejection; `rt::real` unit tests; C33 | `diode_rev`, `divider`, `diode_series`, `isrc_r`, C39 (`both_backends_agree_on_the_m1_circuits`) |
| `PIPERINE_BACKEND=interp` under the same mutation | - | parity_dc green (the interpreter alone) |
| `PIPERINE_BACKEND=bogus` | both parity_dc tests, naming `interp` and `rust` | - |
| affine value printed `c + 0.0 + g0*x0 + g1*x1` (association changed) | C38 | C33 (`every_m1_function_gives_the_interpreters_bytes`), C36, C39, parity |
| limit rule `from_seed = AGE == 0 \|\| (STATIC && AGE >= 1)` | C33, C39 | - |

The unset variable really runs both backends, and the seven twins really run generated Rust: a defect in the runtime the compiled diode uses turns a twin red on the `rust` backend only. `contract.rs` gained one field and one rejection line and no bound; no row count, expected value or `tests/ngspice` file changed against `origin/main`.

## Own examples

- The diode's `limit:vd` and `terms` on both backends over 14 inputs (-25 V to 40 V) x 9 `LAST` (-30 V to 5 V) x the six `AGE`/`STATIC` cases x two `is`: 1512 limit calls, each followed by `terms` at the limited value, all bit-equal (every `pnjlim` branch is reached; C33's own points reach two).
- A design with no running body built, loaded (0 modules, no warning) and gave a circuit.
- A panic injected into a compiled `terms` (through a `rustc` wrapper) came back as `fault (status 2, code 0): panic: boom at 200`, and the next call on the same instance returned its value; the backtrace named the generated file and line.
- Six staggered `Transpiler::compile` calls of one design on one empty cache all completed with one entry.
- MSL names that collide with names the printer generates: see F2 and F3.

## Independent read of the five goldens (T10)

Each golden followed from `tests/ngspice/devices.msl` and `crates/piperine-lang/std/analog.msl` to the Rust line by line: `vt` is `K_B * T / Q` with the exact CODATA values, left to right; `pnjlim` is the std text branch by branch (`-old - 1.0`, `2.0 * old - 1.0`, both `ln` arms, comparisons on `.value()`); the diode's `nvt`, `vcrit` (`2.0_f64.sqrt()` as `R::constant(2.0).sqrt()`), `vd = limit(...)`, `id` and the split of `id + gmin() * vd + ddt(cj0 * vd)` into a flow and a charge term are faithful; the three limit rules are those of `Walk::limit_site` (seed start on `AGE 0` or `STATIC && AGE 1`, `old` the seed there else `LAST`, `LIMITED` raised to the seed only when starting from it); Resistor, Capacitor (`c * (p - n)` as a charge), Isource and Vsource (`p - n - (dc * source_scale + stim_ac)` as `eq`) follow their contributions; the affine terms are `c + (0.0 + g0*x0 + g1*x1)` from the evaluation at 0, pin partials from it and the stimulus partial from the iterate; precedence is the value printer's (`a - (b + c)`, `a / (b * c)`, `-x.exp()` vs `(-x).exp()` all parenthesized where Rust needs it). No SI literal occurs in the M1 bodies; the reals print shortest. No line is unexplained.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `compile.rs` `the_m1_design_compiles_with_no_error_and_no_warning` | yes (but see F2: a valid IR with a colliding name does not compile) |
| C2 | `golden.rs` `each_function_is_a_generic_fn_and_each_body_a_device_mod` | yes |
| C3 | `golden.rs` `transpiling_twice_gives_the_same_bytes` | yes |
| C4 | `print::value::tests::an_unsupported_form_is_named_with_path_and_origin` | yes |
| C5 | `reject.rs` `an_ir_that_fails_check_gives_no_source` | yes |
| C6 | `golden.rs` `every_item_sits_under_its_path_and_origin` | yes |
| C7 | `print::text::tests::keywords_become_raw_identifiers` | yes for keywords; names that collide with generated ones are not covered (F2, F3) |
| C8 | `print::text::tests::reals_round_trip_in_the_shortest_form` | partly - the cited test proves the real literals only; indentation and one statement per line are asserted by `lines_are_indented_by_four_one_statement_each`, which no check cites (F8) |
| C9 | `compile.rs` `the_source_is_kept_beside_the_library_and_reported` | yes |
| C10 | `golden.rs` `the_five_m1_bodies_print_as_the_golden_files` | yes (goldens read independently, above) |
| C11 | `settings::tests::the_compiler_is_the_variable_then_the_path` | yes |
| C12 | `rustc.rs` `the_compiler_is_run_directly_and_cargo_never` | yes |
| C13 | `rustc.rs` `a_missing_compiler_is_named_and_the_interpreter_is_offered` | yes |
| C14 | `compile.rs` `a_compile_error_keeps_the_source_and_shows_stderr` | yes |
| C15 | `rustc.rs` `a_compiler_below_1_85_is_refused_with_both_versions` | yes (1.70 refused, 1.85.0 and 1.86 nightly accepted) |
| C16 | `compile.rs` `a_second_build_runs_no_compiler` | yes for builds; the test filters out the `-vV` probe that every warm build still runs (F6) |
| C17 | `compile.rs` `a_changed_ir_is_another_entry` | yes |
| C18 | `compile.rs` `another_compiler_version_is_another_entry` | yes |
| C19 | `cache.rs` `two_racing_builds_leave_one_entry_and_no_staging` | partly - races `Cache::build`, not `Transpiler::compile`, whose unconditional `cache.remove` can delete a racer's fresh entry (F5) |
| C20 | `load.rs` `a_library_whose_bytes_changed_is_not_loaded` | yes (the tampered file is still a loadable library, so only the hash can refuse it) |
| C21 | `compile.rs` `a_corrupted_library_is_rebuilt` | yes |
| C22 | `session.rs` `no_analysis_runs_the_compiler` | yes |
| C23 | `session.rs` `a_set_between_analyses_runs_no_compiler_and_applies` | yes (the next `Dc` and `Ac` read the new value) |
| C24 | `cache.rs` `an_unwritable_directory_is_named` | yes |
| C25 | `settings::tests::the_cache_directory_follows_the_four_rules` | yes |
| C26 | `compiled.rs` `modules_come_through_the_symbol_and_bind_like_the_interpreter` | yes |
| C27 | `compiled.rs` `loaded_shapes_equal_the_plans_entry_by_entry` | yes (and `Compiled::bind` runs the full `ShapeCheck` on every compile) |
| C28 | `shared.rs` `the_interpreter_and_the_plan_crate_share_one_body_plan` | yes |
| C29 | `load.rs` `a_file_that_is_no_library_is_named` | yes |
| C30 | `load.rs` `a_library_without_the_symbol_is_named` | yes |
| C31 | `load.rs` `a_library_of_a_newer_minor_is_refused` | partly - the `Library.size` refusal RS-05 AC 3 names has no test (F7) |
| C32 | `load.rs` `unsafe_lives_in_load_only` | yes, under the C32 ruling (`#[unsafe(no_mangle)]` only) |
| C33 | `differential.rs` `every_m1_function_gives_the_interpreters_bytes` | weak - three tidy bias points: a changed association of the affine fold stays green here (caught only by C38), and four of `pnjlim`'s branches are never reached; the code is right (own 1512-case sweep) but the test would not see it wrong (F4) |
| C34 | `rt::real::tests::dual_gives_the_interpreters_values_and_tangents` | yes |
| C35 | `differential.rs` `every_compiled_function_passes_verify_partials` | yes |
| C36 | `residue.rs` `the_compiled_resistor_matches_the_hand_written_products_bit_for_bit` | yes |
| C37 | `print::analog::tests::an_affine_term_is_printed_as_constant_plus_fold` | yes |
| C38 | `residue.rs` `a_compiled_affine_constant_leaves_at_most_one_ulp` | yes (caught the association mutation) |
| C39 | `backends.rs` `both_backends_agree_on_the_m1_circuits` | yes as the criterion is worded; a 1 % error in the compiled diode's `exp` stays green here (F9) |
| C40 | not in scope, phase B (T39) | - see F1: its requirement is phase A's |
| C41 | `differential.rs` `a_non_finite_value_is_written_as_is` | yes |
| C42 | `parity_op.rs` `divider` | yes |
| C43 | `parity_op.rs` `diode_series` | yes |
| C44 | `parity_op.rs` `isrc_r` | yes |
| C45 | `parity_dc.rs` `diode_iv` | yes (red on `rust` only under the runtime mutation) |
| C46 | `parity_dc.rs` `diode_rev` | yes |
| C47 | `parity_ac.rs` `ac_rc` | yes |
| C48 | `parity_tran.rs` `tran_rc` | yes |
| C49 | `backend.rs` `a_rejection_names_the_backend` | yes |
| C50 | `backend.rs` `the_rust_bench_runs_loaded_modules` | yes |
| C51 | `contract::tests::the_five_contracts_carry_bounds_and_rationale` | yes (literal bounds; `contract.rs` bounds unchanged against `origin/main`) |
| C52 | `backend.rs` `the_variable_selects_one_backend` | yes |
| C53 | `backend.rs` `an_unknown_backend_is_refused_naming_both` | yes |
| C54 | `compile.rs` `a_build_reports_its_cost` | yes |
| C55 | not in scope, phase B (T24) | - |
| C56 | not in scope, phase B (T25) | - |
| C57 | not in scope, phase B (T26) | - |
| C58 | not in scope, phase B (T27) | - |
| C59 | not in scope, phase B (T28) | - |
| C60 | not in scope, phase B (T28) | - |
| C61 | not in scope, phase B (T29) | - |
| C62 | not in scope, phase B (T30) | - |
| C63 | not in scope, phase B (T35) | - |
| C64 | not in scope, phase B (T35) | - |
| C65 | not in scope, phase B (T35) | - |
| C66 | not in scope, phase B (T35) | - |
| C67 | not in scope, phase B (T37) | - |
| C68 | not in scope, phase B (T36) | - |
| C69 | not in scope, phase B (T36) | - |
| C70 | not in scope, phase B (T36) | - |
| C71 | not in scope, phase B (T39) | - |
| C72 | not in scope, phase B (T36) | - |
| C73 | not in scope, phase B (T39) | - |
| C74 | not in scope, phase B (T39) | - |
| C75 | not in scope, phase B (T31) | - |
| C76 | not in scope, phase B (T32) | - |
| C77 | not in scope, phase B (T32) | - |
| C78 | not in scope, phase B (T33) | - |
| C79 | not in scope, phase B (T33) | - |
| C80 | not in scope, phase B (T32) | - |
| C81 | not in scope, phase B (T34) | - |
| C82 | not in scope, phase B (T34) | - |
| C83 | not in scope, phase B (T33) | - |
| C84 | not in scope, phase B (T34) | - |
| C85 | not in scope, phase B (T43) | - |
| C86 | not in scope, phase B (T42) | - |
| C87 | not in scope, phase B (T42) | - |
| C88 | not in scope, phase B (T42) | - |
| C89 | not in scope, phase B (T42) | - |
| C90 | not in scope, phase B (T43) | - |
| C91 | not in scope, phase B (T44) | - |
| C92 | not in scope, phase B (T45) | - |
| C93 | not in scope, phase B (T45) | - |
| C94 | not in scope, phase B (T46) | - |
| C95 | not in scope, phase B (T43) | - |
| C96 | `abi_source.rs` `the_embedded_sources_are_the_abi_crate` | yes |
| C97 | `abi_source.rs` `the_abi_builds_with_rustc_alone` | yes |
| C98 | not in scope, phase B (T41) | - |
| C99 | not in scope, phase B (T41) | - |
| C100 | `golden.rs` `a_design_with_nothing_to_run_has_no_module` | partly - asserts the source text only; the edge case says a library is built and a circuit with zero devices results (both hold at HEAD, own example) (F8) |
| C101 | `golden.rs` `a_body_without_analog_prints_no_terms_function` | yes |
| C102 | `cache.rs` `two_racing_builds_leave_one_entry_and_no_staging` | yes for `Cache::build` (see F5) |
| C103 | `golden.rs` `a_keyword_identifier_is_raw_and_the_abi_name_is_not` | yes |

Phase A: 59 checks - 50 hold, 9 weak or partial (C7, C8, C16, C19, C31, C33, C39, C100, and C1 through F2), 0 red. Phase B: 44 checks (C40, C55 to C95, C98, C99) not in scope.

## Findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| F1 - RS-06 AC 6 (a panic in compiled code is a fault, the instance survives) is a phase-A criterion with no phase-A proof: its only check, C40, sits on T39 (phase B), while the spec says phase A is complete and mergeable alone | major | spec "Phases of the feature": A is RS-01 to RS-08; tasks.md C40 "(RS-06, AC 6; T39)". Behaviour holds at HEAD (own example: a panic injected into a compiled `terms` gives `fault (status 2, code 0): panic: boom at 200`, the next call returns its value) but a merge of phase A carries it unproven | append a phase-A check (new ID) on a test in `crates/piperine-transpile/tests/` that builds a resistor through a `rustc` wrapper injecting a conditional `panic!` (as `a_shape_that_differs_from_its_plan_is_refused` injects a rename) and asserts the `FAULT` with the message and a successful next call | |
| F2 - an MSL identifier equal to a name the printer generates makes a valid `Ir` fail to compile, against RS-01 AC 1 | major | own example: a resistor whose param or node is named `cx` gives `CompileFailed` (`no method named set found for type f64`, `print/analog.rs` binds `let <name> = cx.f64(i)` in `fn terms(cx: &mut Eval)`); a node `self` beside a param `self_` gives E0415 (the T4 mapping `self` to `self_` collides); `ValuePrinter::names` suffixes duplicates as `<name>_<i>`, which can collide with an MSL local of that spelling. Failure scenario: any model with a param `cx` is refused as "a transpiler defect" | reserve the generated names: name every generated binding in a namespace an MSL identifier cannot take (for example a `__` prefix, or check each MSL name against the set of generated names and suffix on collision), and add a test with params/nodes named `cx`, `at_zero`, `at_iterate`, `proposed`, `seed`, `self`+`self_` that compiles and matches the interpreter | |
| F3 - a param named `<limit>_last` (here `limit_vd_last`) silently replaces the param by `LAST` in the compiled limit function: a wrong number with no error | major | own example (diode fixture with param `is` renamed `limit_vd_last`): `print/limit.rs` emits `let limit_vd_last = cx.f64(2);` then `let limit_vd_last = cx.f64(5);`, Rust accepts the shadowing, the shape is unchanged so `ShapeCheck` passes, and `LIMITED` is 0.0918 compiled vs 0.8213 interpreted at `a = 0.9 V`, `LAST = 0.7` | the same fix as F2 (generated names out of the MSL namespace); a test that the limit function of a body with a param named `<site>_last`, `<site>_age` and `is_static` equals the interpreter's bytes | |
| F4 - C33's differential uses three tidy bias points and so does not hold "every function gives the interpreter's bytes" where it costs most | minor | mutation: printing affine values as `c + 0.0 + g0*x0 + g1*x1` (association changed, the INT-12 rule broken) leaves `every_m1_function_gives_the_interpreters_bytes` green (only C38 goes red); `pnjlim`'s `arg <= 0`, `old <= 0` and both `new < 0` branches are never reached by the three points (own 1512-case sweep shows the code is right) | add a seeded random sweep (the residue test's xorshift, a few hundred points per function, negative and large biases, `LAST` on both sides of `vcrit` and of 0) to the same test, keeping bit equality | |
| F5 - `Transpiler::compile` calls `cache.remove(&key)` on every miss, not only after a failed open; a build that finds no entry can delete the entry a concurrent build has just placed, and that build's `Loaded::open` then fails | minor | `crates/piperine-transpile/src/compile.rs:52-55` (`None => { cache.remove(&key)?; ... }`). Interleaving: B's `cache.entry(&key)` returns `None`; A renames its staging into place; B's `remove` deletes it; A's `Loaded::open(&entry)` reads no manifest: `Error::Cache`, against RS-04 AC 4. C19 races `Cache::build` only | remove only in the branch where an entry existed and failed to open (keep the opened error to report), and race `Transpiler::compile` itself in C19's test | |
| F6 - "invokes the compiler zero times" (RS-04 AC 1, Goal 3) is implemented and tested as "zero builds": every warm build still runs `rustc -vV` to form the key | minor | `compile.rs:30` `Rustc::locate` on every compile; `compile.rs` test helper `builds()` filters `-vV`; tasks Handoff T12 settled it. The key of RS-Q3 needs the `-vV` text, so the spec contradicts itself; consequence: a machine with a warm cache and no `rustc` cannot load the design | amend RS-04 AC 1 to say "runs no build" (a recorded decision, as the spec process requires for an approved criterion), or cache the `-vV` text by compiler path and mtime so a hit runs nothing | |
| F7 - RS-05 AC 3's `Library.size` refusal has no test | minor | `load.rs:62-64` checks `size < size_of::<Library>()`; `load.rs` tests cover no-library, no-symbol and minor only; a flipped comparison would stay green | add a test library that sets `library.size` below the host's `size_of::<Library>()` and expect `Error::Load` naming the file and `Library.size` | |
| F8 - two checks cite a test that proves only part of their criterion | minor | C8 cites `reals_round_trip_in_the_shortest_form`, but its indentation and one-statement halves are in `lines_are_indented_by_four_one_statement_each`; C100's test asserts source text, while the edge case says a library is built and the circuit has zero devices | cite both tests for C8 (or rename C8's proof to a group); extend C100's test to compile the empty design and assert `loaded().names()` is empty and `circuit()` binds no device (it does at HEAD) | |
| F9 - C39 compares two backends that should be bit-identical under the ngspice tolerance, so it cannot see a compiled defect the contract absorbs | minor (advisory; the criterion as worded holds) | mutation: compiled diode `exp` value times 1.01 leaves `both_backends_agree_on_the_m1_circuits` green (diode series moves by `nvt*ln(1.01)`, inside `DC_VOLTAGE`) | keep the contract assertion and add an exact comparison of the two backends' operating points and AC phasors (the functions are bit-equal, so the Newton paths are too); a difference would then be a defect, not noise | |

## Settled mid-build items, judged

- `workspace_shape.rs` (solver crate, test file): two list entries added to `DEFAULT_MEMBERS` and `KEPT_CRATES`; the guard is extended, nothing loosened. Faithful.
- `crate`, `self`, `Self`, `super` printed as `crate_`, `self_`, `Self_`, `super_`: forced by Rust; the ABI name stays. Faithful in intent, but the suffix can collide (F2); RS-02 AC 2's text should record the four exceptions.
- C32 ruling (`#[unsafe(no_mangle)]` allowed): accepted as instructed; the scan allows exactly that attribute and finds every other form.
- `piperine-transpile` depends on `piperine-solver` (for `CircuitInstance` from `Compiled::circuit`): `piperine-plan` already depends on it per the design; generated code still links `piperine-abi` only. Faithful, no scope cut.
- `Rustc::invocations` counts builds, not `-vV`: a reading of RS-04 AC 1 the spec text does not state (F6).
- Hash before `dlopen`, handle leaked (abi.md §12), a mapped library replaced by unlink and write, never rewritten in place: correct; `unsafe` is confined to `load.rs` with `// SAFETY:` lines citing abi.md. Note: `Cache::at` creates `0700` only when it creates; an existing cache or `rust` directory owned by another user or group-writable is used as is (consistent with RS-Q9's "created", worth a later ownership check).

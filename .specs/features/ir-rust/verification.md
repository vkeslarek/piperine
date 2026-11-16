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
| F1 - RS-06 AC 6 (a panic in compiled code is a fault, the instance survives) is a phase-A criterion with no phase-A proof: its only check, C40, sits on T39 (phase B), while the spec says phase A is complete and mergeable alone | major | spec "Phases of the feature": A is RS-01 to RS-08; tasks.md C40 "(RS-06, AC 6; T39)". Behaviour holds at HEAD (own example: a panic injected into a compiled `terms` gives `fault (status 2, code 0): panic: boom at 200`, the next call returns its value) but a merge of phase A carries it unproven | append a phase-A check (new ID) on a test in `crates/piperine-transpile/tests/` that builds a resistor through a `rustc` wrapper injecting a conditional `panic!` (as `a_shape_that_differs_from_its_plan_is_refused` injects a rename) and asserts the `FAULT` with the message and a successful next call |  `99971e3d` - C105 `a_panic_in_compiled_code_is_a_fault_and_the_instance_survives` (run compiled, T17) |
| F2 - an MSL identifier equal to a name the printer generates makes a valid `Ir` fail to compile, against RS-01 AC 1 | major | own example: a resistor whose param or node is named `cx` gives `CompileFailed` (`no method named set found for type f64`, `print/analog.rs` binds `let <name> = cx.f64(i)` in `fn terms(cx: &mut Eval)`); a node `self` beside a param `self_` gives E0415 (the T4 mapping `self` to `self_` collides); `ValuePrinter::names` suffixes duplicates as `<name>_<i>`, which can collide with an MSL local of that spelling. Failure scenario: any model with a param `cx` is refused as "a transpiler defect" | reserve the generated names: name every generated binding in a namespace an MSL identifier cannot take (for example a `__` prefix, or check each MSL name against the set of generated names and suffix on collision), and add a test with params/nodes named `cx`, `at_zero`, `at_iterate`, `proposed`, `seed`, `self`+`self_` that compiles and matches the interpreter |  `630913ac` - C104 `msl_names_that_equal_generated_names_give_the_interpreters_bytes` (run differential); lib `an_msl_name_never_prints_as_a_generated_one` |
| F3 - a param named `<limit>_last` (here `limit_vd_last`) silently replaces the param by `LAST` in the compiled limit function: a wrong number with no error | major | own example (diode fixture with param `is` renamed `limit_vd_last`): `print/limit.rs` emits `let limit_vd_last = cx.f64(2);` then `let limit_vd_last = cx.f64(5);`, Rust accepts the shadowing, the shape is unchanged so `ShapeCheck` passes, and `LIMITED` is 0.0918 compiled vs 0.8213 interpreted at `a = 0.9 V`, `LAST = 0.7` | the same fix as F2 (generated names out of the MSL namespace); a test that the limit function of a body with a param named `<site>_last`, `<site>_age` and `is_static` equals the interpreter's bytes |  `630913ac` - C104 (the `Limiter` body: params `limit_vd_last`, `vd_last_`, `is_static`, limit function bit-equal over the six `AGE`/`STATIC` cases) |
| F4 - C33's differential uses three tidy bias points and so does not hold "every function gives the interpreter's bytes" where it costs most | minor | mutation: printing affine values as `c + 0.0 + g0*x0 + g1*x1` (association changed, the INT-12 rule broken) leaves `every_m1_function_gives_the_interpreters_bytes` green (only C38 goes red); `pnjlim`'s `arg <= 0`, `old <= 0` and both `new < 0` branches are never reached by the three points (own 1512-case sweep shows the code is right) | add a seeded random sweep (the residue test's xorshift, a few hundred points per function, negative and large biases, `LAST` on both sides of `vcrit` and of 0) to the same test, keeping bit equality |  `46f4b34a` - C33 `every_m1_function_gives_the_interpreters_bytes` (300 seeded points, all eight `pnjlim` arms; red under the association mutation) |
| F5 - `Transpiler::compile` calls `cache.remove(&key)` on every miss, not only after a failed open; a build that finds no entry can delete the entry a concurrent build has just placed, and that build's `Loaded::open` then fails | minor | `crates/piperine-transpile/src/compile.rs:52-55` (`None => { cache.remove(&key)?; ... }`). Interleaving: B's `cache.entry(&key)` returns `None`; A renames its staging into place; B's `remove` deletes it; A's `Loaded::open(&entry)` reads no manifest: `Error::Cache`, against RS-04 AC 4. C19 races `Cache::build` only | remove only in the branch where an entry existed and failed to open (keep the opened error to report), and race `Transpiler::compile` itself in C19's test |  `69a0d938` - C19 `two_racing_builds_leave_one_entry_and_no_staging` (eight `Transpiler::compile` racing); the fix holds by construction, the microsecond window cannot be forced without a hook |
| F6 - "invokes the compiler zero times" (RS-04 AC 1, Goal 3) is implemented and tested as "zero builds": every warm build still runs `rustc -vV` to form the key | minor | `compile.rs:30` `Rustc::locate` on every compile; `compile.rs` test helper `builds()` filters `-vV`; tasks Handoff T12 settled it. The key of RS-Q3 needs the `-vV` text, so the spec contradicts itself; consequence: a machine with a warm cache and no `rustc` cannot load the design | amend RS-04 AC 1 to say "runs no build" (a recorded decision, as the spec process requires for an approved criterion), or cache the `-vV` text by compiler path and mtime so a hit runs nothing |  `de38bd29` - C16 `a_second_build_runs_no_compiler` (RS-04 AC 1 amended by orchestrator ruling 2026-11-15: only `rustc -vV`, zero builds) |
| F7 - RS-05 AC 3's `Library.size` refusal has no test | minor | `load.rs:62-64` checks `size < size_of::<Library>()`; `load.rs` tests cover no-library, no-symbol and minor only; a flipped comparison would stay green | add a test library that sets `library.size` below the host's `size_of::<Library>()` and expect `Error::Load` naming the file and `Library.size` |  `6596e970` - C31 `a_library_smaller_than_the_host_reads_is_refused` (run load) |
| F8 - two checks cite a test that proves only part of their criterion | minor | C8 cites `reals_round_trip_in_the_shortest_form`, but its indentation and one-statement halves are in `lines_are_indented_by_four_one_statement_each`; C100's test asserts source text, while the edge case says a library is built and the circuit has zero devices | cite both tests for C8 (or rename C8's proof to a group); extend C100's test to compile the empty design and assert `loaded().names()` is empty and `circuit()` binds no device (it does at HEAD) |  `0074958c` - C8 (second proof `lines_are_indented_by_four_one_statement_each`), C100 `a_design_with_nothing_to_run_has_no_module` (builds, 0 modules, no device) |
| F9 - C39 compares two backends that should be bit-identical under the ngspice tolerance, so it cannot see a compiled defect the contract absorbs | minor (advisory; the criterion as worded holds) | mutation: compiled diode `exp` value times 1.01 leaves `both_backends_agree_on_the_m1_circuits` green (diode series moves by `nvt*ln(1.01)`, inside `DC_VOLTAGE`) | keep the contract assertion and add an exact comparison of the two backends' operating points and AC phasors (the functions are bit-equal, so the Newton paths are too); a difference would then be a defect, not noise |  `0e4d8ef5` - C39 `both_backends_agree_on_the_m1_circuits` (bit equality added beside the contracts; red under the 1 % `exp` mutation) |

## Fix round (F1 to F9)

Each finding closed by its own commit (F2 and F3 together), named in `Closed`. Two checks were appended: C104 (spec edge case 5, F2/F3) and C105 (RS-06 AC 6 in phase A, F1). The phase-A `Run` lines of tasks.md were run afterwards at `0e4d8ef5` with `PIPERINE_BACKEND` unset (rustc 1.98.0, ngspice at `/usr/bin/ngspice`):

| Run | Result |
| --- | --- |
| Build (the three `**Build**` commands) | no warning, no error |
| plan | 2 passed |
| reject | 1 passed |
| lib | 19 passed (+1: `an_msl_name_never_prints_as_a_generated_one`) |
| golden | 7 passed |
| rustc | 3 passed |
| abisrc | 2 passed |
| cache | 4 passed |
| load | 6 passed (+1: `a_library_smaller_than_the_host_reads_is_refused`) |
| compile | 8 passed |
| compiled | 5 passed (+1: C105) |
| differential | 4 passed (+1: C104) |
| residue | 2 passed |
| session | 2 passed |
| backend | 4 passed |
| parity (op 3, dc 2, ac 1, tran 2) | 8 passed |
| backends | 1 passed |
| contract | 15 passed |
| gate extras: `parity_corpus` 4, `suite_hygiene` 12, `workspace_shape` 8 + `crate_boundaries` 12, `piperine-interp` (all targets), `piperine-plan` (all targets) | all passed |

Mutations re-run against the fixed tests (each reverted): the affine value printed `c + 0.0 + g0*x0 + g1*x1` turns C33 red (F4); the compiled `exp` value times 1.01 turns C39 red (F9); the environment variable printed `temperature` again turns C104 red (F3). The old unconditional `remove` stayed green under C19's `Transpiler::compile` race in three runs: the window is the microseconds between the lookup and the remove, so F5 is closed by construction and the race pins RS-04 AC 4 on the real path.

`validate_verification.py` still wants `proofs.md`, which `check_proofs.py` writes only once the phase-B proofs exist.

## Settled mid-build items, judged

- `workspace_shape.rs` (solver crate, test file): two list entries added to `DEFAULT_MEMBERS` and `KEPT_CRATES`; the guard is extended, nothing loosened. Faithful.
- `crate`, `self`, `Self`, `super` printed as `crate_`, `self_`, `Self_`, `super_`: forced by Rust; the ABI name stays. Faithful in intent, but the suffix can collide (F2); RS-02 AC 2's text should record the four exceptions.
- C32 ruling (`#[unsafe(no_mangle)]` allowed): accepted as instructed; the scan allows exactly that attribute and finds every other form.
- `piperine-transpile` depends on `piperine-solver` (for `CircuitInstance` from `Compiled::circuit`): `piperine-plan` already depends on it per the design; generated code still links `piperine-abi` only. Faithful, no scope cut.
- `Rustc::invocations` counts builds, not `-vV`: a reading of RS-04 AC 1 the spec text does not state (F6).
- Hash before `dlopen`, handle leaked (abi.md §12), a mapped library replaced by unlink and write, never rewritten in place: correct; `unsafe` is confined to `load.rs` with `// SAFETY:` lines citing abi.md. Note: `Cache::at` creates `0700` only when it creates; an existing cache or `rust` directory owned by another user or group-writable is used as is (consistent with RS-Q9's "created", worth a later ownership check).

---

# Phase B verification (T24 to T46)

**Verdict**: FAIL (phase B: 0 blocker, 2 major, 7 minor; no runnable proof is red; 6 checks blocked on solver-crossings)
**Proofs**: every phase-B `Run` line of tasks.md run by the Verifier at `2b189f7f` (rustc 1.98.0, ngspice at `/usr/bin/ngspice`, `PIPERINE_NGSPICE` and `PIPERINE_CACHE_DIR` unset), plus plain `cargo test` at the root (default members: 170 targets, exit 0). The parked T42/T45 files were copied into place, run and removed.
**Verifier**: independent sub-agent (author != verifier), Opus

## Proof runs at `2b189f7f`

| Run | Result |
| --- | --- |
| Build (the three `**Build**` commands) | no warning, no error |
| irdigital | 7 passed |
| frozen | 1 passed |
| planshape | 10 passed |
| lib | 32 passed |
| reaction | 5 passed |
| nets | 2 passed |
| commits | no target (T42 parked). Parked file run in place: 3 passed, `the_later_statement_wins` FAILED (x shows `[1,1]` then `[2,1]` at 2.5 ns and again at 7.5 ns), as the Handoff reports |
| dpwm | 3 passed |
| pid | 1 passed |
| boundary | 1 passed (T46). Parked T45 file run in place: ADC passed, comparator FAILED under `TRAN_VOLTAGE`, as reported |
| `cargo test -p` piperine-ir, -plan, -interp, -transpile, -parity (all targets, ngspice really run) | all passed |
| gate extras: `suite_hygiene` 12, `workspace_shape` 8 + `crate_boundaries` 12 | passed |
| root `cargo test` (default members) | 170 targets ok |

Phase A unchanged: no diff in `crates/piperine-transpile/tests/golden/`, `crates/piperine-ir/tests/golden/*.txt`, `rt/real.rs`, `contract.rs`, `piperine-abi`, or the three solver crates since `e4f767dd`. For a design with no carrier, `Runtime::files(false)` gives `[("rt.rs", REAL)]` and the key parts are again `[source, REAL, rustc -vV, flags, ABI files]`, the ABI key `parts[2..]`: both keys and every generated file are bit-identical to phase A.

## Mutations (each reverted; tree clean)

Killed: `&`/`|` level shortcut removed (C68, C69); unknown delay keeps the value (C72); `flush` reversed (C72); `decide` unknown gives `Ok(false)` (C70); unknown `<<` by N not known 0 (C68); signed `>>` by N gives 0 (C66); float cast without saturation, signed and unsigned (C65); UInt division by 0 gives 0 (C63); remainder by `rem_euclid` (C63); negative shift gives 0 (C66); signed pin decode without sign extension (C91); `min_after` for a literal `>= 0` (C79); U16 known byte at 3 (C75); signal as `PORT` (C75); the event guard dropped from `fired_` (C98); the analog read of a carrier decoded as unknown (C94).
Survived: plan statement walk visiting `else` before `then` (F-B3). Equivalent: value bytes encoded while unknown (an unknown `Flagged` always holds `T::default()`).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C55 | `digital.rs` `integers_and_their_operators_print` | yes |
| C56 | `digital.rs` `unknown_values_and_their_reads_print` | yes |
| C57 | `digital.rs` `carriers_and_drives_print` | yes |
| C58 | `digital.rs` `events_and_reactions_print` | yes |
| C59 | `digital.rs` `a_defective_digital_body_gives_one_entry_per_defect` | yes - seven defects, item, file and line each, total count pinned |
| C60 | `digital.rs` `the_m1_prints_are_unchanged` | yes - goldens untouched since `e4f767dd` |
| C61 | `digital.rs` `the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts` | yes |
| C62 | `frozen.rs` `a_digital_body_is_rejected_by_name` | yes |
| C63 | `rt::int::tests::arithmetic_wraps_at_every_width` | yes - i128 reference at ten widths, edges and 1000 pairs; div/rem by 0 panic named |
| C64 | `rt::int::tests::casts_truncate_and_extend_by_the_source_sign` | yes |
| C65 | `rt::int::tests::a_float_cast_saturates_and_nan_is_zero` | yes |
| C66 | `rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result` | yes |
| C67 | `print::value::tests::a_type_rust_lacks_is_refused_by_name` | partly - `UInt<65>`/`Int<65>`/`UInt<0>` by name and path; the `Float<16>`/`Float<128>` half is vacuous (the IR has no such type) and must land where MSL is lowered to IR (F-B8) |
| C68 | `rt::flag::tests::an_unknown_operand_gives_an_unknown_result` | yes, for one flag per value (F-B4) |
| C69 | `rt::flag::tests::bool_operators_follow_the_level_rules` | yes - the nine and three cases written from §3.3.2 |
| C70 | `rt::flag::tests::known_or_is_known_and_high` | yes |
| C71 | `reaction.rs` `a_known_or_over_an_unwritten_input_gives_the_default` | yes |
| C72 | `rt::flag::tests::an_unknown_value_is_written_as_zero_bytes` | yes |
| C73 | `reaction.rs` `an_unknown_condition_faults_and_commits_nothing` | yes - a commit made before the `if` is not flushed; the instance answers the next call |
| C74 | `reaction.rs` `an_unknown_delay_commits_unknown_at_the_next_microstep` | yes |
| C75 | `digital_shape.rs` `carriers_are_discrete_pins_with_a_known_byte` | yes |
| C76 | `digital_shape.rs` `a_drive_is_a_discrete_function` | yes |
| C77 | `digital_shape.rs` `an_event_is_an_event_pin_and_its_function` | yes - T34's rewrite of T32's five write assertions to `CHANGED` follows abi.md §4.1 ("one byte: nonzero fires it") and `validate`; a correction, not a weakening. §11.4's `PREVIOUS` read is legitimately absent (IR `rise` = changed && high) |
| C78 | `digital_shape.rs` `a_reaction_lists_its_events_reads_and_statements` | no - a plan walk with `else` before `then` stays green (F-B3) |
| C79 | `digital_shape.rs` `only_a_literal_positive_delay_is_later` | yes |
| C80 | `digital_shape.rs` `a_delayed_drive_is_a_pin_an_event_and_a_reaction` | yes (shape); behavior was checked by the Verifier's own example below |
| C81 | `digital_shape.rs` `a_cross_event_is_a_watch_with_partials` | yes (shape); the compiled watch's behavior is C93, blocked |
| C82 | `digital_shape.rs` `an_analog_read_of_a_carrier_is_a_read_of_terms` | yes |
| C83 | `digital_shape.rs` `a_probing_reaction_reads_the_continuous_pin` | yes |
| C84 | `digital_shape.rs` `every_buck_slice_shape_validates` | yes |
| C85 | `dpwm.rs` `the_oscillator_toggles_every_half_period` | yes - the whole change list, 40 instants |
| C86 | parked `two_commits_exchange_two_carriers` | blocked on solver-crossings; the parked test holds (both nets, both edges, the wrong reading named) |
| C87 | parked `the_later_statement_wins` | blocked on solver-crossings; the parked test holds and is red today for the reported solver defect |
| C88 | parked `two_statements_make_a_pulse` | blocked on solver-crossings; holds (both statement orders) |
| C89 | parked `an_unchanged_commit_fires_nothing` | blocked on solver-crossings; holds, slightly loose: the counter's value after the move is not asserted, only one move at 2.5 ns |
| C90 | `dpwm.rs` `the_dpwm_dithers_keeps_dead_time_and_never_overlaps` | yes - widths, dead times and 396 from SPEC §5.2/§5.3; full `hs` and `ls` change lists |
| C91 | `pid.rs` `the_ramp_and_the_pid_follow_their_difference_equations` | yes - an independent i32 model of SPEC §6.2-§6.4, §7.2; script reaches both rails, windup hold and resets |
| C92 | parked `the_adc_commits_the_rounded_code_after_tconv` | blocked on solver-crossings (passes in place); holds |
| C93 | parked `the_comparator_trips_at_its_thresholds` | blocked on solver-crossings; holds (trip judged by the analytic voltage, no time tolerance) |
| C94 | `boundary.rs` `a_carrier_change_is_a_time_point_and_starts_the_response` | yes, under a 1 ns step cap (F-B7) |
| C95 | `dpwm.rs` `two_runs_give_the_same_changes` | yes |
| C98 | `digital_circuit.rs` `carrier_ports_bind_to_design_nets` | yes - with a negative control |
| C99 | `digital_circuit.rs` `every_buck_slice_body_compiles_with_no_warning` | yes |

## Own examples (throwaway, not committed)

- `GateDriver` with `hs_cmd` held true, `ls_cmd` false, `tpd` 10 ns: `hs_gate` becomes `HIGH` and `ls_gate` `LOW` at exactly 10 ns, the first commit made at `(0, 0)` as part III §8.2.4 says. Holds.
- `(x & 1u9) & 2u9` with `x` unknown: §3.3.2 per level gives known 0; the runtime gives unknown (F-B4).

## Findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| F-B1 A `signal` carrier of a parent that its children's ports meet on is silently a private pin of the parent, disconnected from the children's net: the parent's drives and commits never reach them, with no error | major | `crates/piperine-plan/src/netlist.rs:66` skips every `Signal`; buck `top.msl:28` (`target_codes(setpoint.known_or(0))` at an `in` port) is exactly this shape in M2 | refuse now in `Netlist::circuit` when a signal's net is bound by another instance (`Error`, naming carrier and net); the real binding lands in msl-digital | `e2dde174` - C106 `a_parent_signal_its_children_meet_on_is_refused` (run nets; refused in `Netlist::circuit`, real binding in tasks Handoff "Left for msl-digital") |
| F-B2 A net's own `init` is dropped silently | major | `netlist.rs` doc comment ("a net's own `init` is not passed"); `NetSpec.init` is never read | refuse a net with `init` and no carrier init until `CircuitBuilder` can take it (msl-digital) | `bf8add18` - C107 `a_net_with_its_own_init_is_refused` (run nets; refused in `Netlist::circuit`, passing it is msl-digital's) |
| F-B3 C78 does not hold its order: the plan's commit walk can visit `else` before `then` and every test stays green; the printer's guard compares carriers only, so a disagreement between plan and printer on two statements of one carrier goes unseen and gives a statement the wrong `LATER`/`min_after` | minor | mutation `Expr::If … each(&[*cond, *otherwise, *then])` in `crates/piperine-plan/src/digital.rs` survives planshape and reaction | a fixture with `if c { b <- 2 after 1e-9 } else { b <- 3 after 2e-9 }` asserting each statement's `min_after`; have the printer compare `(carrier, min_after)` with the plan | `ba83b4b8` - C78 second proof `an_if_lists_its_then_statement_before_its_else` (run planshape; red under the `else`-first walk); the reaction printer compares `(carrier, min_after)` with the plan |
| F-B4 One known flag per value (batch 5) diverges from §3.3.2 on partial masks, and the feature spec's own row disagrees with the code. §3.3.2: "`&`, `\|`, `^`, `!` work per level: `unknown & false == false`, `unknown \| true == true`, otherwise unknown"; §3.3.11 "A shift by a known amount fills with known levels". The code is per level only when one operand is all-false/all-true; `(x & 1) & 2` is unknown where the spec gives known 0, so a later `known_or(d)` returns `d` instead of 0. spec.md's row "every other operator is all-or-nothing" says integers `&`/`\|` are all-or-nothing, which the code (rightly, per §3.3.2) does not do | minor | `rt/flag.rs:318-340`; own example above; spec.md "One known flag per scalar" row | amend the spec.md row to what T36 built and state the rounding (fewer known levels than §3.3.2 for partial masks); record it in ROADMAP "MSL spec gaps" until per-level flags exist | `370b06eb` - spec row "One known flag per scalar" amended to what T36 built (orchestrator ruling 2026-11-15, user may revisit); deviation recorded as G6 in ROADMAP "MSL spec gaps" and `GAPS.md` §5; C68 holds for one flag per value |
| F-B5 A carrier initial value that is not a value of its type panics in `BodyPlan::new` instead of a `check()` entry | minor | `crates/piperine-plan/src/digital.rs:186-189`; `check.rs` validates no carrier `init` | add the `check()` entry (type and range of `Carrier::init`); due before msl-digital produces IR | `ef390c53` - C108 `a_carrier_initial_value_outside_its_type_is_an_entry` (run irdigital; `check()` entry per carrier) |
| F-B6 Forms with no lowering surface late, not as `Error::Unsupported` by name: a reaction reading `temperature`/`gmin`/`source_scale` is planned as an `AMBIENT` read that `piperine_abi::validate` refuses for reactions (only at circuit build); a `limit`/`stim` reading a carrier is unhandled (Handoff T40) | minor | `print/digital.rs:96`, `validate.rs:420`; Handoff T40 | refuse both by name in the printer (or `BodyPlan`), now; note §12.1.12 lets a reaction read the environment, so the ABI's reaction ambient set needs a ruling | `1e9aaf23` - C110 `a_limit_or_stim_function_reading_a_carrier_is_refused_by_name` (run reject). Env reads: refused in `1e9aaf23`, then by user ruling 2026-11-15 ("temperature e gmin precisa estar wired") the refusal reverted in `b7414de6` and the wiring pinned in `957e7327` - C109 `reactions_and_drives_read_the_environment_as_analog_code_does` (run reaction: planned as the temperature pin and the `GMIN`/`SOURCE_SCALE` ambients, compiled); the run waits on solver-crossings' ABI change (tasks Handoff "Left for after solver-crossings", `GAPS.md` §4) |
| F-B7 T46's 1 ns step cap: with the parity harness's own policy (a 10 ns cap here) the response fails `TRAN_VOLTAGE` 5.55 ns after `t0` (first step after the jump); the cap is 20x tighter than `parity_tran`'s. Not a transpiler defect (the switch's terms are proven), but it hides that the solver's restart after a discrete-to-analog jump does not meet the contract at the harness's step | minor | `boundary.rs` with `run_with(8.9e-6, 1e-8)`: `parity failure: switch_rc / v(x) at time 3.00925e-6 s` | report to the solver (restart step after a boundary event); keep the cap, cite this finding at it | `ef2a0606` - 1 ns cap kept, cited at `boundary.rs` and C94; restart step reported in `GAPS.md` §6 |
| F-B8 RS-10 AC 5's `Float<16>`/`Float<128>` half cannot be held by the IR, so C67 proves it vacuously | minor | `print/value.rs` test comment | carry the refusal to msl-elab/typeck (the lowering into IR), with its own check there | `e06aad93` - C67 states it proves the integer half only; the float refusal handed to msl-digital (tasks Handoff, `GAPS.md` §4) |
| F-B9 Hygiene: tasks.md holds a spliced duplicate of phases 1-8 (line 1224 starts mid-sentence "## Checks` cites."; T24-T45 appear twice, T46 once); the parked T42 file keeps a dummy `let _ = UnaryOp::Not;`; a negative shift amount panics (`rt/int.rs:197`) with no spec rule (G5 covers `N` or more only) | minor | `tasks.md:715` and `:1883` | remove the duplicate block; drop the dummy import; add the negative amount to ROADMAP G5 | `f7e012f2` - one copy of tasks phases 1-8 (T46 marked done), dummy `UnaryOp::Not` dropped from the parked T42 file, negative shift amount added to G5 (ROADMAP, `GAPS.md` §5); `validate_tasks.py` exit 0 |

### Item 7 classification (gaps that panic or refuse late)

- Carrier init type mismatch: F-B5, fix in `check()` before msl-digital.
- Digital code reading gmin/source_scale/temperature: F-B6, refuse by name now; the probe read in a reaction is legal (§12.1.12) and planned correctly (C83).
- `limit`/`stim` reading a carrier: F-B6, refuse by name now.
- Parent signal as private pin: F-B1, refuse now, real binding in msl-digital.
- Net `init` not passed: F-B2, refuse now, pass in msl-digital.

## Fix round (F-B1 to F-B9)

Each finding closed by its own commit, named in `Closed` (F-B6 in three: the carrier refusal, then the user's 2026-11-15 ruling reverting the environment refusal and pinning the wiring). Checks appended: C106 to C110 (edge cases 6 to 10 of spec.md); C78 gained a second proof; C67 and C94 state what they prove. Mutations re-run (each reverted): the plan walk visiting `else` before `then` turns C78's new proof red (F-B3); disabling the limit/stim carrier refusal turns C110 red (F-B6). After the round: the three `**Build**` commands with no warning; `suite_hygiene` 12, `workspace_shape` 8 + `crate_boundaries` 12 passed; `validate_tasks.py` exit 0; plain `cargo test` at the root (ngspice at `/usr/bin/ngspice`) green.

# msl-typeck verification

**Verdict**: FAIL (round 2: F1-F6 closures hold; F7-F11 open)
**Proofs**: `proofs.md` at 0f7d03cf (38 pass); HEAD 9a5aadca adds only `.specs/` text
**Verifier**: independent sub-agent (author != verifier)

Method: read spec.md, design.md, tasks.md `## Checks`, proofs.md, `.specs/features/msl-elab/design.md` and spec.md, `docs/spec/` §8.1 (errors), §11 (errors), §14 (table and errors), §3 errors, Appendix G D11/D14, all of `crates/piperine-lang/src/types/` and the five test targets. Ran 38 adversarial sources at HEAD through `support::build` + `Types::check` from a throwaway target (`tests/zz_verifier_scratch.rs`, removed; a copy is in the session scratchpad `verify-typeck/`). Cargo ran with `-p piperine-lang` only.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `types_entry.rs` `the_reachable_set_of_a_twin_is_typed` | yes - every expression owned by a reached decl of all six twins, callee names excluded; exact reached list for `DiodeIv` |
| C2 | `types_entry.rs` `the_design_is_unchanged_by_the_check` | yes - `dump()` before and after, six twins |
| C3 | `types_values.rs` `a_deferred_method_is_resolved` | yes |
| C4 | `types_entry.rs` `checking_twice_gives_equal_tables` | yes |
| C5 | `types_values.rs` `literals_are_f64_or_bool` | yes |
| C6 | `types_values.rs` `operators_type_as_rust_types_them` | yes |
| C7 | `types_values.rs` `if_and_block_take_their_value_type` | yes |
| C8 | `types_values.rs` `a_let_takes_its_initializer_type` | yes |
| C9 | `types_values.rs` `a_call_takes_its_declared_result` | yes - user `fn`s have no optional parameter in M1 (`Option` is out of scope); the optional trailing forms are the core rows, held by C24/C25 |
| C10 | `types_values.rs` `catalogue_methods_on_f64_are_f64` | yes |
| C11 | `types_values.rs` `a_closure_takes_its_types_from_the_parameter` | yes |
| C12 | `types_values.rs` `a_mismatch_names_both_types` | yes |
| C13 | `types_modules.rs` `params_are_f64_or_bool` | yes |
| C14 | `types_modules.rs` `a_default_of_the_wrong_type_is_rejected` | yes |
| C15 | `types_modules.rs` `a_where_that_is_not_bool_is_e0318` | yes for the param `where` it tests; the module and `fn` `where` are not checked at all (F1) |
| C16 | `types_modules.rs` `ports_and_signals_are_nodes_and_gnd_fits_any` | yes |
| C17 | `types_modules.rs` `a_node_init_that_is_not_f64_is_e0210` | yes |
| C18 | `types_modules.rs` `a_placement_checks_nodes_count_and_params` | yes - `E0320` comes from the POM and is given back unchanged |
| C19 | `types_modules.rs` `a_node_of_another_nature_is_e0251` | yes |
| C20 | `types_analog.rs` `potential_probes_are_f64` | yes |
| C21 | `types_analog.rs` `probe_nodes_of_two_natures_are_e0240` | yes |
| C22 | `types_analog.rs` `a_contribution_target_that_is_no_probe_is_e0410` | yes |
| C23 | `types_analog.rs` `a_contribution_that_is_not_f64_is_e0417` | yes |
| C24 | `types_analog.rs` `ddt_ac_stim_and_environment_are_f64` | yes |
| C25 | `types_analog.rs` `limit_checks_its_input_function_and_seed` | yes |
| C26 | `types_analog.rs` `a_core_primitive_in_a_fn_is_e0450` | yes for `fn` and closure, which is what AC 7 names; other contexts the §14 table forbids pass (F2) |
| C27 | `types_subset.rs` `a_construct_outside_the_subset_is_p0001` | no - rows for `bound_step`, traits, `Repr`/`Bit`/known flags and commits/events of the Out of Scope table are missing (F5) |
| C28 | `types_subset.rs` `p0001_names_the_construct` | yes, over the rows C27 has |
| C29 | `types_subset.rs` `unreached_items_are_not_reported` | yes |
| C30 | `types_entry.rs` `a_design_with_diagnostics_gives_them_back` | yes |
| C31 | `types_entry.rs` `an_unknown_top_is_one_diagnostic` | yes |
| C32 | `types_values.rs` `diagnostics_come_in_source_order_one_per_expression` | no - all three mistakes sit in one analog block and are generated in source order; deleting the sort at `check.rs:69` keeps the test green (F4) |
| C33 | `types_entry.rs` `every_diagnostic_has_a_span_and_a_code` | yes |
| C34 | `types_values.rs` `a_recursive_function_is_typed_from_its_signature` | yes |
| C35 | `types_values.rs` `a_let_without_type_or_value_is_p0001` | yes |
| C36 | `types_entry.rs` `a_const_is_typed_once_by_its_declared_type` | yes |
| C37 | `types_values.rs` `a_closure_outside_an_argument_is_p0001` | yes |

## Specific questions

1. **Classifier exhaustive?** `Unsupported` is exhaustive over the POM enums, but three paths accept a construct without classifying it: `ModuleData::where_` and `FnData::where_` are never read (F1), a function used as a value outside an argument is accepted and its indirect call records no callee (F3), and the §14 "Allowed in" column is enforced only for `fn`/closure (F2).
2. **Codes.** Contribution outside `analog`: the spec has `E0404`/`E0121` (parse phase) and the POM already reports them (probed: closure in `analog` gives `E0404`, `fn` body gives `E0121`), so `analog.rs:13`'s `P0001` is unreachable, not mis-coded. Flow probe read as a value: no spec code exists (out of scope, M2); `P0001` is right. The spec does own `E0323` (environment function in a const context) and `E0450` outside a `fn`, and neither is raised (F2).
3. **Poisoning.** It hides second real errors in three places (F6).
4. **Output for msl-elab.** `Types::check` → `Result<Types<'d>, Vec<DesignDiagnostic>>` with `ty`/`method`/`callee`/`local`/`reached` matches msl-elab design ("the reachable set, every expression's type, resolved methods and callees"). The gap: an indirect call has no `callee` entry and a `Ty::Fn` local has no form elab's `Unlowered` table names (F3).

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Major) A module or `fn` `where` is never checked: `module Top() where 1.0 { }` and `fn f(x: f64) -> f64 where x { x }` type-check clean; the expression is left untyped and elab gets a construct the checker claimed to understand | `check.rs:266` (`module`) and `check.rs:197` (`signature`) never read `ModuleData::where_` / `FnData::where_`; scratch probe "module where non-bool", "fn where non-bool": ACCEPTED | `check_as(where_, Bool, "E0318")` on a module `where`, and `P0001` (or E0318) for a `fn` `where`; a test per form |  e4a2d88c |
| F2 (Major) §14 "Allowed in" and §8.1.21 are enforced only inside `fn`/closure: `ac_stim` in a module-body `let`, in a param default and in a param `where`; `ddt`/`limit` in a param default; `temperature()` in a param default (const context, `E0323`) all type-check clean. Neither this design nor msl-elab's owns `E0323` or `E0450` outside a `fn` | `catalogue.rs:49` (`Allowed::Analog` documented as "analog block or module body", while §14 row `ac_stim` says `analog` only); `module.rs:60` checks defaults and `where` in `Context::Module`; scratch probes "ac_stim module let", "ac_stim default", "ac_stim in where", "ddt in default", "limit default", "temperature in param default": ACCEPTED | a `Context::Const` for param defaults and `where` (refuse env fns `E0323`, core modules and `ac_stim` `E0450`), and `ac_stim` refused in a module body; tests per context |  e4a2d88c |
| F3 (Major) A function used as a value outside an argument passes: `let f = h; let y = f(1.0);` type-checks clean (the spec's edge case refuses only closures there), and `indirect_call` records no `Callee`, so msl-elab (which lowers "calls of reached functions" through `callee`) has nothing to lower and its `Unlowered` table does not list it | `expr.rs:214` (`DeclKind::Fn` path gives `Ty::Fn` anywhere); `call.rs:73` (`indirect_call` never inserts into `callees`); scratch probe "fn as let value": ACCEPTED | refuse a `fn` path as a value except as an argument whose parameter has a function type (`P0001`, as for closures); record what an indirect call reaches or have elab's `Unlowered` name it; a test |  e4a2d88c |
| F4 (Minor) C32 does not hold the sort it claims: its three mistakes are generated in source order already | `types_values.rs` `diagnostics_come_in_source_order_one_per_expression`; `check.rs:69` | add a case where the worklist meets the later span first (a mistake in `Top`'s analog block after a mistake in a `fn` declared above it), and a second file for "ordered by file" |  e4a2d88c |
| F5 (Minor) The success criterion "every Out of Scope language construct has a negative `P0001` test" is not met | `types_subset.rs` `ROWS` has no row for `bound_step`, a reached trait/`impl` method, `Repr`/`Bit`/known flags (the support std is empty, so they do not resolve), commits/events; `bound_step` is refused correctly at HEAD (probed) | add the rows, building those that need `std` with the real library |  e4a2d88c |
| F6 (Minor) Poisoning hides independent real errors: `let x: u32 = 1.0 + true;` reports only `u32` (the initializer is never checked); a generic `fn` body with `x + true` reports only "generic function"; `fn f(a: f64) -> f64 { let y = a + true; }` reports `true` but not the missing `f64` result | `expr.rs:288-293` (`let_statement` returns before `init`); `check.rs:203` + `function` (body skipped when the signature fails); `check.rs:252-259` (`function_body` drops the result mismatch whenever the body raised anything) | infer the initializer after a refused annotation; compare the body's result whenever the last statement is not an expression, regardless of earlier diagnostics; tests |  e4a2d88c |

## Round 2

**Proofs**: `proofs.md` at 01bae5c9 (44 pass); HEAD d8967f89 adds only `proofs.md`
**Verifier**: independent sub-agent (author != verifier), round 2

Method: read the round-1 diff `e4a2d88c` (code, the two edited tests, the spec amendments AC 3.8, AC 4.8, AC 4.9, AC 5.4, AC 5.5, checks C38-C43) and `docs/spec/` Part I §3 (errors), Part III §8 and §8.1 (errors), §8.3.4, Part IV §14 (table, rules, errors). Ran 31 new adversarial sources and every module of `devices.msl` as a top at HEAD from a throwaway target (`tests/zz_verifier_r2.rs`, removed; copy in the session scratchpad `verify-typeck/`). Re-ran `types_entry`, `types_values`, `types_modules`, `types_analog`, `types_subset`: 45 pass. The six M1 twins check clean (`the_reachable_set_of_a_twin_is_typed`, `the_design_is_unchanged_by_the_check`), and `Resistor`, `Capacitor`, `Vsource`, `Isource`, `Diode` of `tests/ngspice/devices.msl` each check clean as a top.

### Closures of round 1

| Finding | Probe at HEAD | Result | Holds |
| --- | --- | --- | --- |
| F1 | `module Top() where 1.0`, `fn f(x: f64) -> f64 where x`, a probe or `ac_stim` in a module `where` | each refused | yes for "is read"; the code chosen for a module `where` and the `fn` `where` it accepts contradict docs/spec (F7, F8) |
| F2 | `ac_stim` in a module let, a param default, a param or module `where`, a placement param value; `ddt`/`limit` in a default, a `where`, a closure in a default; env functions in a default, a param/module/`fn` `where`, a `const` (also via another `const`); context restored after a closure and after a reached `fn` | all refused; the run-time positions stay clean | yes; E0323 is the spec's code; E0450 is not the spec's code for a core module (F9) |
| F3 | `let f = h;`, `k(h)` with `k(x: f64)`, a call through a `let` closure or a `fn` parameter | refused `P0001` (closure: two diagnostics, one per form) | yes, but a `fn` chosen by an `if` inside a limiter argument passes (F10) |
| F4 | C32 now has a `fn` above `Top` and a two-file case | the sort is needed for both | yes |
| F5 | rows for `bound_step`, `ddx`, `delay`, `noise`, `cross`, `at`, `simparam`, `now`, `freq`, `inflow`, flow probe, trait, commit; `Bit`, `Repr`, known flag against the real std | present, one `P0001` each, construct named | yes |
| F6 | `let x: u32 = 1.0 + true;`; generic `fn` body with `x + true`; generic `fn` called twice and passed as a limiter; `{ let y = a + true; }` | independent errors reported, the generic refused once | yes for the three cases tested; the exception hides a missing result (F11) |

### Judgements asked for

1. **`Context::Const`, E0450/E0323/E0318 against docs/spec.** `E0323` is right: §8.1.21 and §14 rule 4, Part III §8.1 errors and Part IV §14 errors both name it. `E0318` is right for a param `where` (§8.1 errors) and **wrong for a module `where`**: Part III §8 errors give `E0304` ("the `where` after the port list is false, or is not a `bool` const expression"). A `fn` `where` has no non-`bool` code; §3 rule 8 makes it "a boolean const expression over its const generics and associated consts", which M1 has none of. `E0450` is right for `ac_stim` and probes outside their rows (a probe in a default could also be `E0321`/`E0318`, more specific, but §14's table covers it literally). For `ddt` and `limit`, which §14 calls core **modules**, the §14 errors table routes "a core module placed in a `fn`, a closure or a reaction body" to `E0350`, and §8.3.4 gives `E0352` for "a placement in a const context"; `E0450` is not the spec's code for either (F9).
2. **F6 exception.** Not sound as written. It is right only for a refused `return` (the value the result needs may be in it). `last_statement_refused` suppresses the result whenever any `P0001` lies inside the last statement's span, so `fn f(a: f64) -> f64 { let y = 1; }` and `{ a % 2.0; }` report the `P0001` and drop the missing `f64` result, an independent mistake. That contradicts AC 5.5 as amended ("the missing result of a body whose last statement is not itself refused": here the statement is not refused, a literal inside it is). No test pins the boundary (F11).
3. **Edited tests.** Both legitimate follow-ons. `apply`'s body was never reached in C9 (the test asserts it is not), and a call through a parameter is now refused by AC 5.4, so `f(x, x)` would test a refusal C41 already owns; C11 still passes closures to `apply` and types them from its declaration. The four `types_subset` rows now give `1.0` because F6 checks the initializer after a refused annotation, so `= 1` would add a second, independent `P0001` (the integer literal has its own row); the row still proves the type is named and refused. Neither weakens a criterion.
4. **M1 twins and `devices.msl`.** Still clean (above).

### New checks

| Check | Test | Holds | Note |
| --- | --- | --- | --- |
| C38 | `types_modules.rs` `a_where_on_a_module_or_fn_is_bool` | no | asserts `E0318` on a module `where` where Part III §8 gives `E0304`; its positive case `fn f(x: f64) -> f64 where x > 0.0` accepts a `where` over a value parameter, which §3 rule 8 forbids (F7, F8) |
| C39 | `types_modules.rs` `a_core_primitive_is_checked_where_it_is_written` | yes | against AC 4.8 as amended; the code it pins for `ddt`/`limit` is F9 |
| C40 | `types_modules.rs` `an_environment_function_in_a_const_context_is_e0323` | yes | four const contexts and two run-time positions; the `fn` `where` is untested but refused (probed) |
| C41 | `types_values.rs` `a_function_is_a_value_only_as_an_argument` | yes | a `fn` value inside an `if` in the argument is not covered (F10) |
| C42 | `types_values.rs` `an_unsupported_form_does_not_hide_an_independent_error` | yes | for its three cases; the exception's boundary is untested (F11) |
| C43 | `types_subset.rs` `std_types_outside_the_subset_are_p0001` | yes | real std, one `P0001` each, construct named |

### Round 2 findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F7 (Major) A module `where` that is not `bool` is reported `E0318`, the param code; Part III §8 errors give `E0304` for "the `where` after the port list ... is not a `bool` const expression". AC 3.8 (amendment) encodes the wrong code | `check.rs:332`; `types_modules.rs` `a_where_on_a_module_or_fn_is_bool`; `docs/spec/part_iii_structure.md:80` | `E0304` for a module `where`; amend AC 3.8 and C38 | |
| F8 (Major) A `fn` `where` over a value parameter type-checks clean: `fn f(x: f64) -> f64 where x > 0.0 { x }` is C38's positive case. §3 rule 8 allows only const generics and associated consts there, and M1 has neither | `check.rs:276-280` (params are locals before the `where`); `docs/spec/part_i_values.md:249` | refuse a `fn` `where` in M1 with `P0001` (as the round-1 fix proposed), or refuse a read of a value parameter in it; replace the positive case | |
| F9 (Minor) `ddt` and `limit` out of place give `E0450`; for core modules the spec's codes are `E0350` (placed in a `fn`, a closure or a reaction body, §14 errors row "§8.3") and `E0352` (placed in a const context, §8.3.4). The approved AC 4.7 already chose `E0450` for `fn`/closure; AC 4.8 extends it | `analog.rs:101-115`, `catalogue.rs:53` and `catalogue.rs:70-71`; `docs/spec/part_iv_behavior.md:797`, `docs/spec/part_iii_structure.md:561-563` | `E0350`/`E0352` for `Allowed::Instances` out of place, or record the departure in the spec's Differences with the user's confirmation | |
| F10 (Minor) A function chosen by an `if` inside a limiter argument passes: `limit(v(a), if true { h } else { h })` type-checks clean, so elab receives a limiter that is no named function or closure; `check` pushes the expected `fn` type into the branches and `function_value` accepts each `h` | `expr.rs:34` (`ExprKind::Path` arm under an expected `Ty::Fn`) reached through `expr.rs:35-38` (the `if` arm checks each branch against it); scratch probe "fn value in if": ACCEPTED | accept a `fn` path as a value only when it is the argument itself; a test | |
| F11 (Minor) The F6 exception hides a missing result: `fn f(a: f64) -> f64 { let y = 1; }` reports only the `P0001` at `1`; `{ a % 2.0; }` only the `P0001` (whose span is the left operand `a`, not the `%` expression: `expr.rs:152`, pre-existing) | `check.rs:299` and `check.rs:306-313` (any `P0001` inside the last statement's span suppresses the result); scratch probes "F6 last let int lit", "F6 last expr stmt with %" | suppress only when the last statement is itself the refused form (a `return`), and put the `%`/bitwise `P0001` on the whole binary expression; tests for both | |

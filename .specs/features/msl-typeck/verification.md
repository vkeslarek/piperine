# msl-typeck verification

**Verdict**: FAIL
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
| F1 (Major) A module or `fn` `where` is never checked: `module Top() where 1.0 { }` and `fn f(x: f64) -> f64 where x { x }` type-check clean; the expression is left untyped and elab gets a construct the checker claimed to understand | `check.rs:266` (`module`) and `check.rs:197` (`signature`) never read `ModuleData::where_` / `FnData::where_`; scratch probe "module where non-bool", "fn where non-bool": ACCEPTED | `check_as(where_, Bool, "E0318")` on a module `where`, and `P0001` (or E0318) for a `fn` `where`; a test per form | |
| F2 (Major) §14 "Allowed in" and §8.1.21 are enforced only inside `fn`/closure: `ac_stim` in a module-body `let`, in a param default and in a param `where`; `ddt`/`limit` in a param default; `temperature()` in a param default (const context, `E0323`) all type-check clean. Neither this design nor msl-elab's owns `E0323` or `E0450` outside a `fn` | `catalogue.rs:49` (`Allowed::Analog` documented as "analog block or module body", while §14 row `ac_stim` says `analog` only); `module.rs:60` checks defaults and `where` in `Context::Module`; scratch probes "ac_stim module let", "ac_stim default", "ac_stim in where", "ddt in default", "limit default", "temperature in param default": ACCEPTED | a `Context::Const` for param defaults and `where` (refuse env fns `E0323`, core modules and `ac_stim` `E0450`), and `ac_stim` refused in a module body; tests per context | |
| F3 (Major) A function used as a value outside an argument passes: `let f = h; let y = f(1.0);` type-checks clean (the spec's edge case refuses only closures there), and `indirect_call` records no `Callee`, so msl-elab (which lowers "calls of reached functions" through `callee`) has nothing to lower and its `Unlowered` table does not list it | `expr.rs:214` (`DeclKind::Fn` path gives `Ty::Fn` anywhere); `call.rs:73` (`indirect_call` never inserts into `callees`); scratch probe "fn as let value": ACCEPTED | refuse a `fn` path as a value except as an argument whose parameter has a function type (`P0001`, as for closures); record what an indirect call reaches or have elab's `Unlowered` name it; a test | |
| F4 (Minor) C32 does not hold the sort it claims: its three mistakes are generated in source order already | `types_values.rs` `diagnostics_come_in_source_order_one_per_expression`; `check.rs:69` | add a case where the worklist meets the later span first (a mistake in `Top`'s analog block after a mistake in a `fn` declared above it), and a second file for "ordered by file" | |
| F5 (Minor) The success criterion "every Out of Scope language construct has a negative `P0001` test" is not met | `types_subset.rs` `ROWS` has no row for `bound_step`, a reached trait/`impl` method, `Repr`/`Bit`/known flags (the support std is empty, so they do not resolve), commits/events; `bound_step` is refused correctly at HEAD (probed) | add the rows, building those that need `std` with the real library | |
| F6 (Minor) Poisoning hides independent real errors: `let x: u32 = 1.0 + true;` reports only `u32` (the initializer is never checked); a generic `fn` body with `x + true` reports only "generic function"; `fn f(a: f64) -> f64 { let y = a + true; }` reports `true` but not the missing `f64` result | `expr.rs:288-293` (`let_statement` returns before `init`); `check.rs:203` + `function` (body skipped when the signature fails); `check.rs:252-259` (`function_body` drops the result mismatch whenever the body raised anything) | infer the initializer after a refused annotation; compare the body's result whenever the last statement is not an expression, regardless of earlier diagnostics; tests | |

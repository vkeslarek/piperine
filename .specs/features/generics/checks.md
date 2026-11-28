# Checks of the generics feature, every phase

The `**Build**` line, the union of the `**Run**` lines and every check of `tasks.md` (phases A and B), `tasks-c.md`, `tasks-d.md` and `tasks-e.md` with its proofs; the GN-30 checks (the ngspice twins, the warning-free build) stand once for every phase. `check_proofs.py generics` runs this file.

**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite suite_hygiene::`
**Run typesmodel**: `cargo test -p piperine-lang --test types_model`
**Run typesgen**: `cargo test -p piperine-lang --test types_generics`
**Run typestraits**: `cargo test -p piperine-lang --test types_traits`
**Run typeshandles**: `cargo test -p piperine-lang --test types_handles`
**Run elabgen**: `cargo test -p piperine-lang --test elab_generics`
**Run elabhandles**: `cargo test -p piperine-lang --test elab_handles`
**Run irgen**: `cargo test -p piperine-ir --test model generics::`
**Run interp**: `cargo test -p piperine-interp --test interpreter frozen::`
**Run tgen**: `cargo test -p piperine-transpile --test print generics::`
**Run plancomp**: `cargo test -p piperine-plan --test shape composite_shape::`
**Run srcgen**: `cargo test -p piperine-parity --test source_generics`
**Run srchandles**: `cargo test -p piperine-parity --test source_handles`
**Run parityproject**: `cargo test -p piperine-parity --test project`
**Run apiproject**: `cargo test -p piperine-api --test session project::`
**Run apisession**: `cargo test -p piperine-api --test session session::`
**Run clicommands**: `cargo test -p piperine-cli --test commands commands::`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run galleries**: `cargo test -p piperine-lang --test std galleries::`
**Run typesmods**: `cargo test -p piperine-lang --test types_generic_modules`
**Run typesgenr**: `cargo test -p piperine-lang --test types_generation`
**Run elabmods**: `cargo test -p piperine-lang --test elab_generic_modules`
**Run elabgenr**: `cargo test -p piperine-lang --test elab_generation`
**Run srcmods**: `cargo test -p piperine-parity --test source_generic_modules`
**Run srcgenr**: `cargo test -p piperine-parity --test source_generation`
**Run bookmodules**: `cargo test -p piperine --test book_modules`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookcorestd**: `cargo test -p piperine --test book_core_std`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookgenerics**: `cargo test -p piperine --test book_generics`
**Run bookwelcome**: `cargo test -p piperine --test book_welcome`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run typescarrarr**: `cargo test -p piperine-lang --test types_carrier_arrays`
**Run elabcarrarr**: `cargo test -p piperine-lang --test elab_carrier_arrays`
**Run paritycarrarr**: `cargo test -p piperine-parity --test source_carrier_arrays`
**Run dag**: `cargo test -p piperine-digital-solver --test plan dag::`
**Run apicarrarr**: `cargo test -p piperine-api --test host carrier_arrays::`
**Run bookcompiler**: `cargo test -p piperine --test book_compiler`
**Run typesmodules**: `cargo test -p piperine-lang --test types_modules`
**Run elabresults**: `cargo test -p piperine-lang --test elab_results`
**Run typestempl**: `cargo test -p piperine-lang --test types_templates`
**Run typesimplicit**: `cargo test -p piperine-lang --test types_implicit`
**Run elabtempl**: `cargo test -p piperine-lang --test elab_templates`
**Run elabimplicit**: `cargo test -p piperine-lang --test elab_implicit`
**Run tcontrol**: `cargo test -p piperine-transpile --test print control::`
**Run srctempl**: `cargo test -p piperine-parity --test source_templates`
**Run srcimplicit**: `cargo test -p piperine-parity --test source_implicit`
**Run elabparams**: `cargo test -p piperine-lang --test elab_params`
**Run bookanalog**: `cargo test -p piperine --test book_analog`
**Run stdlibrary**: `cargo test -p piperine-lang --test std library::`
**Run srcstd**: `cargo test -p piperine-parity --test source_stdlib_generic`
**Run typesresults**: `cargo test -p piperine-lang --test types_results`
**Run elabtemplates**: `cargo test -p piperine-lang --test elab_templates`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`
**Run suite**: `cargo test -p piperine-repository-guards --test suite`

## Checks

**C1** - A generic function with trait, `Repr`, project-trait, `Fn` and const parameters runs with explicit and inferred arguments as Rust gives. (GN-01, AC 1; T40)
Proof: run srcgen | test generic_functions_run_with_explicit_and_inferred_arguments ... ok
Proof: run srcgen | test a_function_bounded_by_an_h8_trait_or_repr_runs_per_instance ... ok
Proof: run srcgen | test a_hand_written_repr_runs_through_a_repr_bound ... ok
Proof: run elabgen | test encode_and_decode_of_every_layout_lower_through_the_level_cast ... ok

**C2** - A capturing closure, a plain closure and a function name each pass for an `F: Fn(f64) -> f64` parameter and are called positionally. (GN-01, AC 2; T37)
Proof: run srcgen | test a_capturing_closure_a_plain_closure_and_a_function_name_pass_for_an_fn_bound ... ok

**C3** - A generic argument that cannot be inferred is `E0181` naming the parameter. (GN-01, AC 3; T20)
Proof: run typesgen | test an_uninferred_generic_argument_is_e0181_naming_the_parameter ... ok

**C4** - A `where` false for an instantiation is `E0127` naming the instantiation. (GN-01, AC 4; T33)
Proof: run elabgen | test a_false_where_is_e0127_naming_the_instantiation ... ok

**C5** - Each distinct instantiation is one IR function named from the function and its arguments. (GN-02, AC 1; T34)
Proof: run elabgen | test one_ir_function_per_distinct_instantiation_named_by_its_arguments ... ok
Proof: run irgen | test a_function_instance_is_built_checked_and_printed ... ok

**C6** - The POM keeps the generic function as written, with no instance added to `Design`. (GN-02, AC 2; T40)
Proof: run elabgen | test the_pom_keeps_the_generic_function_as_written ... ok

**C7** - Methods with `self`, associated functions and associated constants of an `impl` on a struct or enum run with Rust's meaning. (GN-03, AC 1; T35)
Proof: run srcgen | test methods_associated_functions_and_constants_run_with_rusts_meaning ... ok

**C8** - An `impl` on a module is `E0130`. (GN-03, AC 2; T26)
Proof: run typestraits | test an_impl_on_a_module_is_e0130 ... ok

**C9** - A method named `changed`, `prev`, `others` or `inflow` is `E0131`. (GN-03, AC 3; T26)
Proof: run typestraits | test a_method_named_like_a_carrier_method_is_e0131 ... ok

**C10** - A trait method calls its `impl` statically, or the default body when the `impl` does not override it. (GN-04, AC 1; T36)
Proof: run srcgen | test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok

**C11** - A bound's method in a generic body calls the instantiation's implementation. (GN-04, AC 2; T36)
Proof: run srcgen | test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok

**C12** - A blanket `impl<T: Bound> Trait for T` applies to every type its bound admits. (GN-04, AC 3; T29, T36)
Proof: run typestraits | test a_blanket_impl_applies_to_every_type_its_bound_admits ... ok
Proof: run srcgen | test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok

**C13** - Two `impl`s of one trait applying to one type, and an `impl` leaving an item without a default, are `E0187` naming the trait and the type. (GN-04, AC 4; T27, T29)
Proof: run typestraits | test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok
Proof: run typestraits | test overlapping_impls_are_e0187 ... ok

**C14** - `impl Trait for` a bundle is `E0134`; for a nature it is accepted. (GN-04, AC 5; T26)
Proof: run typestraits | test impl_trait_for_a_bundle_is_e0134 ... ok
Proof: run typestraits | test impl_trait_for_a_nature_is_accepted ... ok

**C15** - A hand-written one-hot or Gray `impl Repr` carries `Bit<T>` with that layout. (GN-05, AC 1; T9, T11, T39)
Proof: run srcgen | test a_hand_written_repr_carries_its_own_layout ... ok
Proof: run plancomp | test a_hand_written_repr_is_one_leaf_of_its_width ... ok
Proof: run irgen | test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok

**C16** - A hand-written `decode` returning `None` is returned to the caller. (GN-05, AC 2; T39)
Proof: run srcgen | test a_decode_returning_none_is_returned_to_the_caller ... ok

**C17** - A generic struct or enum instantiates per argument list and takes every composite-values construct on each instance. (GN-06, AC 1; T38)
Proof: run srcgen | test generic_structs_and_enums_instantiate_per_argument_list ... ok

**C18** - A generic `#[derive(Repr)]` type is laid out per §2 for an argument with `Repr`, and is `E0110` for one without. (GN-06, AC 2; T31, T38)
Proof: run srcgen | test a_generic_repr_instance_is_laid_out_per_section_2 ... ok
Proof: run typesgen | test a_generic_repr_type_over_an_argument_without_repr_is_e0110 ... ok

**C19** - A generic body is checked once at its definition with its parameters opaque (Q1 (c)). (GN-07, AC 1; T18)
Proof: run typesgen | test a_generic_body_is_checked_once_at_its_definition ... ok

**C20** - A method or operator no bound gives is refused at the definition, naming the missing bound, with nothing instantiating it. (GN-07, AC 2; T19)
Proof: run typesgen | test a_member_no_bound_gives_is_refused_at_the_definition ... ok

**C21** - An instantiation that makes a `where`, an array length or a const expression invalid is reported at the instantiation, naming the item and its arguments. (GN-07, AC 3; T33)
Proof: run elabgen | test instantiation_errors_name_the_item_and_its_arguments ... ok

**C22** - §3 rule 9 says which errors belong to a definition and which to an instantiation. (GN-07, AC 4; T19) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C23** - The model check types every function, module, behavior block, `const` and `impl` of every crate, reached or not, and of the package's dependencies. (GN-08, AC 1; T1, T2)
Proof: run typesmodel | test every_item_kind_of_every_crate_is_checked_unreached ... ok
Proof: run apiproject | test a_dependency_crate_is_type_checked ... ok

**C24** - A library package with no `[run]` table is type-checked whole, each error with the code it has when reached. (GN-08, AC 2; T1, T2, T5)
Proof: run apiproject | test a_library_package_with_no_run_is_type_checked ... ok
Proof: run typesmodel | test an_unreached_item_has_the_code_it_has_when_reached ... ok
Proof: run clicommands | test check_reports_one_error_per_unreached_item_kind ... ok

**C25** - `fn broken(x: f64) -> bool { x + 1.0 }`, called by nothing, fails `piperine check` naming it. (GN-08, AC 3; T1)
Proof: run typesmodel | test an_unreached_function_with_an_error_fails_the_model_check ... ok

**C26** - A module nothing places that calls `ddt` from a function fails the check with `E0350`. (GN-08, AC 4; T1)
Proof: run typesmodel | test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok

**C27** - `piperine run` and `piperine build` fail with the diagnostic `piperine check` gives for an unreached item. (GN-09, AC 1; T3, T4, T5)
Proof: run parityproject | test the_source_runner_checks_the_whole_model ... ok
Proof: run apisession | test a_run_fails_on_an_unreached_error_with_the_check_diagnostic ... ok
Proof: run clicommands | test build_and_run_fail_with_the_check_diagnostic ... ok

**C28** - `impl Thermal { fn celsius(self) }` gives `temp - 273.15` on a thermal carrier. (GN-10, AC 1; T50)
Proof: run srchandles | test impl_thermal_celsius_reads_temp_minus_273_15 ... ok

**C29** - A method call on a carrier is looked up in §3.1.7 order: carrier methods, the nature's `impl`s, the value's methods. (GN-10, AC 2; T46)
Proof: run typeshandles | test a_carrier_method_call_follows_the_lookup_order ... ok

**C30** - A non-function item in an `impl` on a nature is `E0132`. (GN-10, AC 3; T26)
Proof: run typestraits | test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok

**C31** - A handle parameter is bound to the carrier passed and is read, probed, passed on and has carrier methods called. (GN-11, AC 1; T44, T49)
Proof: run srchandles | test a_handle_is_read_probed_passed_on_and_has_carrier_methods ... ok

**C32** - A handle returned, stored in a value or written through is `E0133`. (GN-11, AC 2; T45) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typeshandles | test a_handle_returned_stored_or_written_is_e0133 ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C33** - A discrete handle reads the carrier's value with its flags. (GN-11, AC 3; T49)
Proof: run srchandles | test a_discrete_handle_reads_the_value_with_its_flags ... ok

**C34** - A function generic over a nature instantiates per nature. (GN-11, AC 4; T48, T51)
Proof: run srchandles | test a_nature_generic_handle_function_instantiates_per_nature ... ok

**C35** - A probing function is solve-reading and runs from `analog` and from a reaction. (GN-12, AC 1; T47, T51)
Proof: run srchandles | test a_probing_function_runs_from_analog_and_a_reaction ... ok
Proof: run typeshandles | test solve_reading_functions_are_accepted_where_the_table_allows ... ok

**C36** - A solve-reading call where that class is not allowed is `E0122`. (GN-12, AC 2; T47) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typeshandles | test a_solve_reading_call_in_a_const_context_is_e0122 ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C37** - A probe of a handle in a function body is not `E0450`. (GN-12, AC 3; T44)
Proof: run typeshandles | test a_probe_of_a_handle_in_a_function_is_not_e0450 ... ok

**C38** - The interpreter refuses a generic instance and a hand-written `Repr` by name; a handle call lowers to the IR of its written-out body (D11). (GN-29, AC 1; T8, T9, T49)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run elabhandles | test a_handle_call_lowers_to_its_written_out_body ... ok

**C39** - The forms of phases C, D and E stay `P0001` naming the form. (GN-29, AC 2; T42)
Proof: run typesgen | test phase_c_d_and_e_forms_stay_p0001_by_name ... ok

**C40** - The 7 ngspice twins print the bytes they printed before. (GN-30, AC 1; every phase)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C41** - The build gives no warning on the crates this feature touches, and the hygiene guard passes. (GN-30, AC 2; every phase)
Proof: `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | (! grep -qE '^(warning|error)') && cargo test -p piperine-repository-guards --test suite suite_hygiene::`

**C42** - `GAPS.md` narrows S3, then no longer lists L7, the generic-enum part of L4, S3, X6 and L8, and §9 is updated. (GN-31, AC 1; T6, T43, T52) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C43** - Every gallery fence and Book listing phase A lets check is held to the checker. (GN-31, AC 2; T41)
Proof: run galleries | test the_fences_generics_lifted_are_checked ... ok

**C44** - A generic item instantiated twice with equal arguments is one instance. (Edge case; T34)
Proof: run elabgen | test equal_arguments_make_one_instance ... ok

**C45** - A const generic of 0 elaborates, its loops running zero times. (Edge case; T40)
Proof: run srcgen | test a_const_generic_of_zero_runs_its_loops_zero_times ... ok

**C46** - An instantiation chain that never ends is `E0341` naming the function (D7). (Edge case; T33)
Proof: run elabgen | test an_endless_instantiation_chain_is_e0341 ... ok

**C47** - The elaborator reads the type table only through the scope's substitution, so no `Ty::Param` reaches the IR. (GN-01, AC 1; T32)
Proof: run elabgen | test the_elaborator_reads_types_only_through_the_substitution ... ok

**C48** - Instance names compile to Rust and nested argument lists spell apart. (GN-02, AC 1; T10)
Proof: run tgen | test instance_names_compile_and_nested_arguments_spell_apart ... ok

**C49** - A call of a generic `extern "intrinsic"` function types through the generic path (`E0181` for a parameter nothing gives, `E0187` for a missing bound) and elaboration refuses it by name, never as "generic function". (GN-01, AC 1; fix round, finding 10)
Proof: run typesgen | test a_call_of_a_generic_extern_intrinsic_types_through_the_generic_path ... ok

**C50** - A call whose lowering is unsupported is never dropped from a reaction, `analog` or a body `let`, whatever the position in the function that holds it (tail, `if` arm, block tail, `match` arm). (GN-29; fix round 2, finding 12)
Proof: run elabgen | test a_generic_extern_call_is_never_dropped_in_any_position ... ok
Proof: run elabgen | test a_logic_call_in_a_function_tail_is_refused_by_name ... ok

**C51** - `encode` and `decode` under `T: Repr` lower and run at `f64`, a signed integer, a derived struct, enum, tuple and array (GAPS L70). (GN-01, AC 1 residual; T53, T54)
Proof: run elabgen | test encode_and_decode_of_every_layout_lower_through_the_level_cast ... ok
Proof: run srcgen | test encode_and_decode_run_at_every_layout ... ok

**C52** - A hand-written `Repr` on a generic type carries each instance with that `impl`'s layout. (GN-32, AC 1; T56, T57)
Proof: run typestraits | test a_hand_written_repr_on_a_generic_type_is_typed_per_argument ... ok
Proof: run srcgen | test a_generic_hand_written_repr_is_carried_with_its_layout ... ok

**C53** - A carrier of a hand-written type with a `Default` and no initial value starts at the levels `encode` gives for the default. (GN-32, AC 2; T58)
Proof: run srcgen | test a_carrier_of_a_hand_written_type_starts_at_its_default_levels ... ok

**C54** - A generic module with type, nature (with a default) and const parameters elaborates each placement as its explicit or inferred arguments give. (GN-13, AC 1; T61, T68)
Proof: run typesmods | test generic_module_arguments_are_explicit_or_inferred ... ok
Proof: run srcmods | test generic_modules_run_with_explicit_and_inferred_arguments ... ok

**C55** - A behavior block repeating the module's generic names binds them to the instance's arguments. (GN-13, AC 2; T62)
Proof: run typesmods | test a_behavior_block_binds_its_modules_generic_names ... ok

**C56** - Behavior block names that differ from the module's are refused naming both lists. (GN-13, AC 3; T62)
Proof: run typesmods | test behavior_names_that_differ_are_e0403_naming_both_lists ... ok

**C57** - A module's `where` false for an instance is `E0304` naming the instance. (GN-13, AC 4; T65, T77)
Proof: run elabmods | test a_false_module_where_is_e0304_naming_the_instance ... ok
Proof: run bookmodules | test e0304_a_generic_ladder_of_zero_sections ... ok

**C58** - Const expressions in types and const arguments evaluate at elaboration. (GN-14, AC 1; T63)
Proof: run typesmods | test const_expressions_in_written_types_are_accepted_everywhere ... ok

**C59** - A `const fn` called in a const context folds with every composite form. (GN-14, AC 2; T64)
Proof: run elabgen | test a_const_fn_and_a_generic_repeat_fold_in_a_const_context ... ok

**C60** - One IR body per distinct instance of a generic module; the POM keeps the generic module and each placement as written. (GN-15, AC 1; T65)
Proof: run elabmods | test one_body_per_distinct_module_instance_and_the_pom_as_written ... ok

**C61** - Each instance is named deterministically in the IR, the generated Rust and the device library's module list. (GN-15, AC 2; T66, T67)
Proof: run irgen | test a_body_instance_is_built_checked_and_printed ... ok
Proof: run tgen | test module_instance_names_compile_and_list_in_the_library ... ok

**C62** - `Short(a, b)` over one continuous nature gives the solution of one node. (GN-16, AC 1; T69)
Proof: run srcmods | test short_merges_two_nodes_of_each_nature ... ok

**C63** - `Short` over two natures is refused naming both. (GN-16, AC 2; T61, T69)
Proof: run typesmods | test two_natures_for_one_nature_parameter_are_e0180_naming_both ... ok
Proof: run srcmods | test short_over_two_natures_is_refused_naming_both ... ok

**C64** - A generation `for` elaborates its items once per iteration, each named item an array by ordinal, nested loops nesting it. (GN-17, AC 1; T72, T73, T75)
Proof: run typesgenr | test generated_items_are_typed_as_arrays_by_ordinal ... ok
Proof: run srcgenr | test generated_instances_run_and_read_by_ordinal ... ok

**C65** - A generation `if` elaborates the taken arm only. (GN-17, AC 2; T73, T75)
Proof: run elabgenr | test generation_for_makes_arrays_and_if_keeps_the_taken_arm ... ok
Proof: run srcgenr | test a_generation_if_places_the_taken_arm ... ok

**C66** - A generation range or condition that is not const is `E0340`. (GN-17, AC 3; T72, T77)
Proof: run typesgenr | test a_generation_range_that_is_not_const_is_e0340 ... ok
Proof: run bookmodules | test e0340_a_generation_range_read_from_a_signal ... ok

**C67** - A reference into an untaken arm or an index outside a generated array is `E0342`. (GN-17, AC 4; T73)
Proof: run elabgenr | test an_untaken_arm_or_an_index_outside_is_e0342 ... ok

**C68** - A param declared inside generation is `E0322`. (GN-17, AC 5; T72)
Proof: run typesgenr | test a_param_inside_generation_is_e0322 ... ok

**C69** - A module placing itself under a terminating generation `if` unrolls. (GN-18, AC 1; T74)
Proof: run elabgenr | test a_recursion_by_generation_unrolls ... ok

**C70** - The Book's RC ladder at `N = 1, 3, 8` equals the ladder written out. (GN-18, AC 2; T76, T77)
Proof: run srcgenr | test the_generic_ladder_equals_the_ladder_written_out ... ok
Proof: run bookmodules | test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok

**C71** - A generation recursion that does not end is `E0341`. (GN-18, AC 3; T74)
Proof: run elabgenr | test a_generation_recursion_that_does_not_end_is_e0341 ... ok

**C72** - `for k in 0..N { on lines[k].rise() { .. } }` makes one reaction per iteration. (GN-19, AC 1; T78, T79, T80)
Proof: run typesgenr | test generation_in_a_digital_block_is_typed_per_iteration ... ok
Proof: run srcgenr | test a_generated_bank_reacts_per_element ... ok
Proof: run srcgenr | test generation_in_a_digital_block_runs_from_every_entry ... ok

**C73** - Generated reactions keep iteration order within source order, each a distinct statement. (GN-19, AC 2; T79, T80)
Proof: run elabgenr | test generated_reactions_are_one_per_iteration_in_source_order ... ok
Proof: run bookmodules | test the_half_reactions_report_each_stage_in_time_order ... ok

**C74** - A param read by generation, a const generic argument, an array length or a template is structural. (GN-20, AC 1; T70, T73)
Proof: run elabmods | test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok

**C75** - A host override of a structural param before a run elaborates with it; a set or sweep during the run is `E0803` naming it as structural. (GN-20, AC 2, AC 3; T70)
Proof: run apisession | test a_structural_param_overrides_before_the_run_and_is_refused_during_it ... ok

**C76** - A generic module's body is checked once at its definition, and an unreached one with an error fails `piperine check` (S3). (GN-07, AC 2; GN-08, AC 1; T60, T71)
Proof: run typesmods | test a_generic_module_body_is_checked_once_at_its_definition ... ok
Proof: run typesmodel | test an_unreached_generic_module_with_an_error_fails_the_model_check ... ok

**C77** - The interpreter refuses a generic module instance by name. (GN-29, AC 1; T66)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok

**C78** - The forms of phases D and E stay `P0001` naming the form. (GN-29, AC 2; T81)
Proof: run typesgen | test phase_d_and_e_forms_stay_p0001_by_name ... ok

**C81** - `GAPS.md` drops L70, L69, S3, L30 and L31 in the commits that close them. (GN-31, AC 1; T54, T58, T71, T82) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C82** - Every Book row and gallery fence phase C lets run is held to it. (GN-31, AC 2; T71, T77, T80, T81)
Proof: run bookdigital | test the_flip_flop_runs_for_a_type_with_a_default ... ok
Proof: run bookcheatsheet | test the_module_card_runs_for_its_const_generic_without_its_value_input ... ok
Proof: run bookcheatsheet | test the_module_card_runs_with_its_value_input_and_its_out_carrier ... ok
Proof: run bookcheatsheet | test the_body_items_card_runs_with_its_generation ... ok
Proof: run bookcorestd | test the_register_with_a_setup_check_runs ... ok
Proof: run bookmodules | test the_lowpass_module_runs_over_the_generic_integrator ... ok
Proof: run bookmodules | test the_pi_controller_runs_with_its_generic_idt ... ok
Proof: run galleries | test the_fences_generics_phase_c_lifted_are_checked ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C83** - A chain of growing module arguments is `E0341` naming the module. (Edge case; T65)
Proof: run elabmods | test a_chain_of_growing_module_arguments_is_e0341 ... ok

**C84** - A nature parameter nothing fixes takes its default; a const generic of 0 elaborates with empty loops. (Edge cases; T68)
Proof: run srcmods | test a_nature_parameter_takes_its_default ... ok
Proof: run srcmods | test a_const_generic_of_zero_makes_an_empty_module ... ok

**C85** - An empty generation range makes an empty array. (Edge case; T73)
Proof: run elabgenr | test an_empty_generation_range_makes_an_empty_array ... ok

**C86** - `cast` and `same` compile from `std/values.msl`, agree with the lowering they replace, and a `cast` to a tuple holding an enum runs. (GN-27, AC 1; GN-01, AC 1; T55) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run srcgen | test cast_and_same_from_source_agree_with_the_removed_lowering ... ok
Proof: run srcgen | test a_cast_to_a_tuple_holding_an_enum_runs ... ok
Proof: run typesgen | test no_intrinsic_stands_for_cast_or_same ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C120** - The Book listings that contradicted the spec are corrected, and each old form is refused as the spec says: a carrier port left out is `E0312` (§8.1.8), a `[Bit; N]` with no initial value is `E0187` (§3.3.4) and the rule holds per element of an array (one element defined, one handed to an `out` port: `E0187` names the rest; every element named: clean; an initial value beside some element definitions is the others' value, `E0338` once all are defined), a refused whole-array definition is the driver and raises no `E0187`, `Slice` is bounded by `Default` as gallery E.6 is. (GN-31, AC 2; Book amendments BA1 to BA3; T116)
Proof: run bookcorestd | test a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was ... ok
Proof: run bookcheatsheet | test an_array_of_bits_with_no_initial_value_is_e0187 ... ok
Proof: run booknatures | test the_slice_listing_bounds_t_by_default_as_gallery_e6_does ... ok
Proof: run typescarrarr | test an_array_of_carriers_without_a_default_is_judged_per_element ... ok
Proof: run typescarrarr | test an_initial_value_is_the_value_of_the_elements_no_undelayed_definition_names ... ok
Proof: run typescarrarr | test a_refused_whole_array_definition_is_the_driver_and_raises_no_e0187 ... ok
Proof: run paritycarrarr | test an_element_given_to_an_out_port_is_driven_by_the_child ... ok

**C121** - A trailing `Option` value input is passed by name after the positional connections; a name that is none is `E0140`. (§8.1.9, §8.3.7; GN-13, AC 1; T117)
Proof: run typesmodules | test a_trailing_option_value_input_passed_by_name_binds_it ... ok
Proof: run typesmodules | test a_named_connection_that_is_no_trailing_option_input_is_e0140 ... ok
Proof: run elabresults | test a_value_input_passed_by_name_reads_its_argument ... ok

**C122** - No form of phase C is left behind the interim gate: generation in a `digital` block, the last one, runs from the plain production entry, and the gate is gone. (Interim strategy; T60 adds the gate, T71, T77 and T80 empty it; the interim pin `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed` holds each intermediate list and is replaced at T80 by this proof)
Proof: run srcgenr | test generation_in_a_digital_block_runs_from_every_entry ... ok

**C123** - A `let` that feeds back through the result of a generic placement whose result reads only its own carriers is legal (`lowpass`, `PiAw` over `idt`). (§8.3.5; GN-13, AC 1; T61)
Proof: run typesmods | test a_generic_placement_feeds_back_through_its_unknown ... ok

**C124** - A param whose type is a type parameter is typed under its bounds and elaborated per instance (`reset: T`). (§8.1.15; GN-13, AC 1; T60, T71)
Proof: run typesmods | test a_param_of_a_type_parameter_is_typed_under_its_bounds ... ok

**C125** - A generic argument that does not implement a bound the module's carriers need is `E0187` naming the type and the trait (`Dff::<u8>`); a carrier handed to a generic `out` port needs no initial value. (§3.3.4, §3.8; GN-13, AC 1; T61, T71)
Proof: run bookdigital | test e0187_the_flip_flop_at_u8_has_no_default ... ok
Proof: run typesmods | test a_carrier_handed_to_a_generic_out_port_needs_no_initial_value ... ok

**C126** - `_` as a generic argument of a placement is inferred as if left out. (§1.2.12; GN-13, AC 1; T61)
Proof: run typesmods | test an_underscore_generic_argument_is_inferred ... ok

**C127** - A Book row still refused after a task that changed its refusal is pinned to its exact remaining refusals and their owners. The ladder and the half reactions run (C70, C73), so they leave this list. (GN-31, AC 2; re-pin rule; T71, T77)
Proof: run bookmodules | test the_memory_bank_is_refused_for_its_template_and_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_for_its_template_and_bundle ... ok
Proof: run booknatures | test a_bundle_is_refused_where_the_solver_reaches_it ... ok
Proof: run bookgenerics | test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok
Proof: run bookwelcome | test the_status_paragraph_lists_what_is_still_refused_and_one_item_runs ... ok
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C128** - The per-element `E0187` holds when the array length is a generic argument or a param and when elements are named through a generation index: `[Bit; N]` with only `b[0]` placed as `M::<4>` is `E0187` naming `b[1..4]` and `M::<4>`; a param length `[Bit; n]` at `n = 4` the same (`Wide~n=4`); an element handed in a generic module the same; `for k in 0..2` over `[Bit; 4]` names `y[2..4]`; every element named through the loop or a length the drive covers is clean. (§3.3.4, §8.2.20; round 3 finding 12; design-c DC11)
Proof: run elabcarrarr | test a_generic_length_array_is_judged_per_element_at_each_instance ... ok
Proof: run elabcarrarr | test a_param_length_array_is_judged_per_element_at_each_variant ... ok
Proof: run elabcarrarr | test a_generation_index_names_the_elements_it_reaches ... ok

**C129** - An initial value of an array of carriers is the value of the elements no `out` port holds: it is `E0223` only once every element is handed (a generic length judged per instance), and an array with one element handed and another committed by the parent runs with its initial value. (§5.2.6, §8.2.21; round 3 finding 13; design-c DC11)
Proof: run typescarrarr | test an_initial_value_is_the_value_of_the_elements_no_out_port_holds_and_e0223_once_all_are_handed ... ok
Proof: run elabcarrarr | test an_initial_value_over_elements_no_out_port_holds_is_clean_and_over_every_element_is_e0223 ... ok
Proof: run paritycarrarr | test an_element_given_to_an_out_port_is_driven_by_the_child ... ok

**C130** - An initial value beside undelayed definitions is the value of the elements no definition names: for a generic or param length `E0338` is judged per instance, naming it, once the definitions name every element (`M::<2>` with `b[0]` and `b[1]` defined), and `M::<4>` of the same source is clean. (§8.2.21; coordinator ruling after round 3; design-c DC11)
Proof: run elabcarrarr | test an_initial_value_beside_definitions_of_every_element_is_e0338_at_the_instance_that_folds_so ... ok

**C131** - Every item kind a generation `for`/`if` holds is generated (§8.2.23): a carrier declared in a `for` is an array of one carrier per iteration, each with its own iteration's initial value, its name inside the loop the iteration's element; a definition is one drive per iteration its arm takes; a placement of a module with a value input or a result is one hoisted frame per iteration (`y[0]`, `y[1]`); a `let` read outside its loops is the array of its iterations' values (`c[1][2]`), and one of an arm not taken is `E0342`; the element coverage and the arrays of carriers apply to them, and they run (a commit by index from outside the loop, three hoisted loads in parallel). An item only some iterations make (L76) and generation in a hoisted module (L77) are refused by name. (§8.2.23, §8.2.24; GN-17, AC 1; round 4 finding 14)
Proof: run elabgenr | test a_carrier_declared_in_a_generation_for_is_an_array_of_one_element_per_iteration ... ok
Proof: run elabgenr | test a_definition_in_generation_is_one_drive_per_iteration_its_arm_takes ... ok
Proof: run elabgenr | test a_hoisted_placement_in_generation_is_one_frame_per_iteration_and_its_let_an_array ... ok
Proof: run elabgenr | test a_let_in_generation_read_outside_its_loops_is_the_array_of_its_values ... ok
Proof: run elabgenr | test an_item_only_some_iterations_elaborate_and_generation_in_a_hoisted_module_are_refused_by_name ... ok
Proof: run elabcarrarr | test a_generation_index_names_the_elements_it_reaches ... ok
Proof: run srcgenr | test carriers_definitions_and_hoisted_placements_in_generation_run ... ok
Proof: run srcgenr | test a_hoisted_placement_with_contributions_runs_once_per_iteration ... ok

**C132** - Two drives of one carrier are judged against each other where every index is folded and every generation unrolled: a nested array with a generic inner length (`g[1][0]` and `g[0][1]` of `[[Bit; N]; 2]`) and definitions written in generation beside fixed ones (`for k in 0..2 { b[k] = a; } b[3] = !a;`) are clean, as are definitions in exclusive arms of a generation `if`; a shared element is `E0335`, `E0221` or `E0220` at the instance, naming the element; an initial value beside a definition of an arm not taken is clean, and of the taken arm `E0338`. (§5.2, §5.2.6, §8.2.20, §8.2.21, §8.2.24; round 4 finding 15)
Proof: run elabcarrarr | test a_nested_array_with_a_generic_inner_length_judges_shared_elements_per_instance ... ok
Proof: run elabcarrarr | test drives_written_in_generation_are_judged_where_generation_is_unrolled ... ok

**C133** - A reader of an element of an array of carriers is recomputed when that element changes, whichever drives it: a definition reading an element a definition drives (`total = y[1] + 1`), a chain over two arrays and over two elements of one array (`z[1] = z[0] + 1`, no cycle), a reader of the whole array (`sum3(y)`), a reader at an element decided at run time, a reaction on the element's change, and an `analog` block (through a mirror carrier per element read). (§3.3.6, §5.1.8, §8.2.1; abi.md §9 D1 at element granularity; found by the round 4 fix, coordinator request)
Proof: run paritycarrarr | test a_definition_reading_an_element_a_definition_drives_follows_it ... ok
Proof: run paritycarrarr | test whole_array_run_time_element_and_reaction_readers_follow_the_elements ... ok
Proof: run paritycarrarr | test an_analog_block_reads_an_element_of_an_array_of_carriers ... ok
Proof: run dag | test definitions_of_different_elements_of_one_array_net_chain_without_a_cycle ... ok

**C134** - A `pub` member of a placement written in generation is read inside its loop as the iteration's instance's (`w[k] = c.m` reads `c[k].m`) and outside by its ordinals (`c[1].m`, `c[1][0].m`); an ordinal the generation never made is `E0342`. (§8.2.8, §8.2.23; coordinator request after the round 4 fix)
Proof: run elabgenr | test a_member_of_a_generated_instance_is_read_by_its_iteration_or_its_ordinal ... ok
Proof: run srcgenr | test carriers_definitions_and_hoisted_placements_in_generation_run ... ok

**C136** - The mirror of an element of an array of carriers the `analog` block reads is internal (`CarrierRole::Internal`): the default probes, the op table and the metadata of a design whose `analog` block reads `x[0]` and `x[1]` list only `Top.x` and `Top.a`, and the host can name no internal carrier. (§21, part VIII; round 5 finding 16)
Proof: run apicarrarr | test an_analog_read_of_an_element_lists_no_internal_net ... ok

**C137** - A definition reading an array of carriers at a run-time element reads one view of the array (a `WHOLE` cell, ordered after every element's writer), never one read per element: `q = mem[at]` over 65 536 elements elaborates to `drive q = mem[at]` in well under a second, and builds and runs, following its address and its element, in seconds. (§5.1.8, abi.md §3 amended; round 5 finding 17)
Proof: run elabcarrarr | test a_run_time_element_read_in_a_definition_is_one_read_of_the_array ... ok
Proof: run paritycarrarr | test a_ram_read_port_of_65536_elements_builds_and_follows_its_address_and_its_element ... ok
Proof: run paritycarrarr | test whole_array_run_time_element_and_reaction_readers_follow_the_elements ... ok

**C138** - L77 (generation in a hoisted module's body) is owned by `generics` phase E, task T118 in `tasks-e.md`, not by a spec lane. (round 5 finding 18) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C139** - The Book's `P0001` claim (compiler~79f44db1, `holds`) is shown on a form still unfinished, a module template as a value (`R { r: 2.0 }`, phase D, L32): its refusal names `module template`; its example was a generic module until phase C ran those. (GN-31, AC 2; final suite after the main merge)
Proof: run bookcompiler | test a_compiler_code_says_what_the_chapter_says ... ok

**C140** - A print entry naming an internal carrier (`x$analog[0]`, in full or relative to the top) names nothing: `E0801` with the usual message. (§21, part VIII; round 6 low 19)
Proof: run apicarrarr | test an_internal_net_is_no_name_a_probe_can_print ... ok

**C141** - A loop of undelayed definitions through elements of arrays of carriers (`b[1] = b[2] + 1; b[2] = b[1];`), through a read at a run-time element, across two arrays or through a carrier defined in generation, is `E0339` at elaboration naming the elements and the instance; a chain through elements is clean. (§8.2.3; round 6 low 20)
Proof: run elabcarrarr | test a_loop_of_definitions_through_elements_is_e0339_naming_the_elements ... ok

**C87** - `Name { params }` and `Name::<G> { params }` as a `const`, a param value or a function result give a value of the module's type. (GN-21, AC 1; T84, T85)
Proof: run typestempl | test a_template_is_a_value_of_its_modules_type ... ok
Proof: run elabtempl | test templates_fold_from_consts_functions_and_two_levels_of_params ... ok
Proof: run elabtempl | test a_template_of_a_generic_module_names_its_arguments ... ok
Proof: run elabtempl | test two_templates_of_one_generic_module_with_different_arguments_are_two_values ... ok
Proof: run elabtempl | test a_template_of_a_generic_module_obeys_its_where_and_names_its_missing_arguments ... ok

**C88** - A template leaving a required param unbound is `E0319`; an `Option<T>` param with no default is not required (§8.1.17), one rule with the placement's (`ParamRequirement`; fix round 3, finding 13). (GN-21, AC 2; T84)
Proof: run typestempl | test a_template_leaving_a_required_param_is_e0319 ... ok
Proof: run typestempl | test a_template_leaves_an_option_param_out_as_a_placement_does ... ok
Proof: run elabtempl | test a_template_that_leaves_an_option_param_out_places_none ... ok
Proof: run srctempl | test a_const_template_leaves_an_option_param_out ... ok

**C89** - A template-valued expression that is not const is `E0362`. (GN-21, AC 3; T84)
Proof: run typestempl | test a_template_that_is_not_const_is_e0362 ... ok

**C90** - A template is never carried: `E0112`. (GN-21, AC 4; T84)
Proof: run typestempl | test a_template_is_never_carried_e0112 ... ok

**C91** - A placement through a template value (a `const`, a param, a body `let` bound to a template, written before or after its placement, or to a choice of templates that folds at elaboration) or a function result places an instance identical to the written placement; a choice of templates the run makes is `E0362`, calling an instance is `E0333` and calling a value that is neither a function nor a template is `E0180`. (GN-22, AC 1; T86, T87, T93; fix round 11, finding 29; fix round 12, findings 30 to 32)
Proof: run typestempl | test a_placement_through_a_template_or_a_function_types_as_its_module ... ok
Proof: run elabtempl | test a_template_placement_is_identical_to_the_written_placement ... ok
Proof: run bookmodules | test a_function_that_returns_a_template_places_its_result ... ok
Proof: run srctempl | test a_template_a_body_let_holds_places_its_module ... ok

**C92** - A param block on a placement through a template value is `E0361`. (GN-22, AC 2; T86)
Proof: run typestempl | test a_param_block_on_a_template_placement_is_e0361 ... ok

**C93** - A binning function places the module and params its arguments choose. (GN-22, AC 3; T87, T92)
Proof: run elabtempl | test a_binning_function_places_the_module_it_chooses ... ok
Proof: run srctempl | test binning_places_the_chosen_model ... ok

**C94** - A module whose port types and directions match `P` fits `C: Fn(P)`, names not counting. (GN-23, AC 1; T88, T92)
Proof: run typestempl | test a_module_whose_ports_match_fits_fn_p_names_not_counting ... ok
Proof: run srctempl | test a_memory_bank_places_its_cell_template ... ok
Proof: run typestempl | test a_fn_bounded_parameter_is_inferred_from_its_template_param ... ok

**C95** - A module whose ports differ from `P` is `E0360` naming the first port that differs. (GN-23, AC 2; T88)
Proof: run typestempl | test a_module_whose_ports_differ_is_e0360_naming_the_first ... ok

**C96** - `inst.name` through a template-typed param resolves against the concrete module, in the module that places the template and from its parent, `E0343` for a missing or private item; an item of any kind (a `pub` param, a `pub let`, a carrier) is read as a value with the type its context wants, checked against the concrete item at elaboration (`E0180` naming both; `E0181` where no context gives one: a method's receiver, the operand of `as`, a `let` with no type that nothing reads, fix round 3 findings 14 and 15), also from the parent (`bank.c0.cs`) and at a generated iteration (`bank.cells[1].cs`); an out-of-range generated cell is `E0342` naming its element and the length of its own loop. (GN-23, AC 3; T91; fix rounds 1 and 2, L78) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run elabtempl | test inst_name_through_a_template_param_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_missing_item_through_a_template_param_is_e0343 ... ok
Proof: run elabtempl | test inst_name_in_the_module_that_places_the_template_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_non_node_item_through_a_template_reads_the_concrete_module ... ok
Proof: run elabtempl | test a_non_node_item_through_a_template_of_another_type_is_e0180_at_elaboration ... ok
Proof: run elabtempl | test a_generated_cells_param_reads_the_iteration_its_indexes_name ... ok
Proof: run typestempl | test a_non_node_item_through_a_template_takes_the_type_its_context_wants ... ok
Proof: run typestempl | test a_cast_of_a_template_item_is_e0181 ... ok
Proof: run elabtempl | test a_cast_of_a_template_item_is_e0181_not_a_node_conversion ... ok
Proof: run typestempl | test an_unread_let_of_a_template_item_is_e0181 ... ok
Proof: run elabtempl | test an_unread_let_of_a_template_item_is_e0181_not_an_elaboration_refusal ... ok
Proof: run srctempl | test the_items_of_a_template_placed_child_read_as_values ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run elabtempl | test an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop ... ok

**C97** - A module type takes trait and inherent impls of pure functions over its template, run at elaboration; a trait plus `Fn(P)` bound admits only a module that implements the trait; a function that reads the solve is `E0122`. (GN-23; the decision of 2026-11-22; T83, T89, T90, T92) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run typestraits | test an_impl_on_a_module_type_is_accepted ... ok
Proof: run typestraits | test a_module_impl_function_that_reads_the_solve_is_e0122 ... ok
Proof: run elabtempl | test a_module_impl_function_runs_on_a_template_at_elaboration ... ok
Proof: run elabtempl | test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok
Proof: run srctempl | test the_swappable_mosfet_runs_each_model ... ok

**C98** - `multiplicity()` returns `Some` of the run's `multiplicity` session parameter and `None` when the run sets none; nothing is scaled by the system. (GN-24, AC 1, AC 2; amended twice 2026-11-23, DQ4; T97, T100)
Proof: run elabimplicit | test the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled ... ok
Proof: run srcimplicit | test nothing_set_reads_none ... ok

**C99** - `multiplicity()` is a stdlib function, `simparam("multiplicity")`, with `DEFAULT_MULTIPLICITY = 1.0` beside it: no core row, no dedicated key, flag or check, no environment read (`E0802` for it withdrawn; user decision 2026-11-23). (GN-24, AC 3, AC 6; T99)
Proof: run typesimplicit | test multiplicity_is_a_stdlib_function_over_simparam ... ok
Proof: run elabimplicit | test multiplicity_lowers_to_the_session_parameter ... ok
Proof: run elabimplicit | test a_function_that_reaches_the_session_parameter_at_any_depth_reads_it_in_the_body ... ok

**C100** - A resistor with its own `m: Option<f64>` combined with `multiplicity()` in its body is four in parallel placed with `m: Some(4.0)` and follows the run's one multiplicity without it; reading `multiplicity()` in a param default is `E0321`, the code of §8.1.16 and §8.5.5 (`E0188` is the `const` item's). (GN-24, AC 4, AC 5; T100)
Proof: run srcimplicit | test an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel ... ok
Proof: run srcimplicit | test multiplicity_in_a_param_default_is_e0321 ... ok

**C101** - `temperature()` is `Some` of the run's temperature in kelvin or `None`, and the model chooses constant, initial or ignored; the docs recommend the ambient fallback. (GN-25, AC 1, AC 2; T94, T97, T98, T100) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typesimplicit | test temperature_is_an_option_of_the_run ... ok
Proof: run srcimplicit | test a_model_chooses_how_to_use_its_temperature ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run bookanalog | test the_temperature_listings_run_with_the_ambient_fallback ... ok

**C102** - `m` and `dtemp` are ordinary names: no implicit param exists. (GN-25, AC 3; T94, T97) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typesimplicit | test m_and_dtemp_are_ordinary_names ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C103** - The run sets its temperature and its `multiplicity` session parameter before it starts, they hold for the whole run, and neither has a placement or `set` spelling. (GN-26, AC 1, AC 2; T100)
Proof: run srcimplicit | test the_run_settings_hold_for_the_whole_run ... ok

**C104** - The Book's multiplicity listing runs as four in parallel and its row leaves `GAPS.md` §11. (GN-26, AC 3; T101)
Proof: run bookmodules | test the_multiplicity_listing_runs_as_four_in_parallel ... ok

**C105** - The interpreter reads the temperature-given flag (it is no ambient special case); a template leaves no IR form; the device reads whether the run set its temperature. (GN-29, AC 1; T95, T96, T87)
Proof: run interp | test each_generics_form_is_refused_by_name ... ok
Proof: run irgen | test run_setting_reads_are_built_checked_and_printed ... ok
Proof: run tgen | test a_device_reads_whether_the_run_set_its_temperature ... ok

**C106** - `GAPS.md` drops L29 and L32 and §9 and §10 follow. (GN-31, AC 1; T102) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C107** - The template Book rows flip or narrow to the bundle they still need, and the swappable MOSFET listing runs. (GN-31, AC 2; T93)
Proof: run bookmodules | test the_swappable_mosfet_listing_runs ... ok
Proof: run bookmodules | test the_memory_bank_is_refused_only_for_its_bundle ... ok
Proof: run bookcheatsheet | test the_template_card_is_refused_only_for_its_bundle ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C110** - A param default reads the params declared before it, folded per instance over the values they take there, a template param's trait method included (`level: f64 = fet.gain()`); the params a later default reads are structural; a default that reads the param itself or a later one is `E0321`. (§8.1.2, §8.1.16; fix round 2, L80) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run typestempl | test a_param_default_reads_earlier_params_and_a_template_params_trait ... ok
Proof: run elabparams | test a_default_that_reads_earlier_params_is_folded_per_instance ... ok
Proof: run elabparams | test a_default_that_reads_itself_or_a_later_param_is_e0321 ... ok
Proof: run elabtempl | test a_param_default_dispatches_the_trait_of_a_template_param ... ok
Proof: run srctempl | test a_param_default_reads_the_params_before_it ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C111** - The type of a template of a generic module carries its arguments, `G<2>`: a template of other arguments where it is wanted is `E0180` naming both, an argument the template leaves out is the wanted type's, and a placement through a template (in place, a `const`, a param of type `S<E>`, a param bounded by `Fn(..)`) of a module whose ports are typed by its generic parameters reads them under those arguments (`E0251` for a node of another nature). (§8.4.3, §8.4.5; fix round 2, L79) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run typestempl | test the_type_of_a_generic_template_carries_its_arguments ... ok
Proof: run typestempl | test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok
Proof: run elabtempl | test a_placement_through_a_template_of_a_nature_generic_module_places_its_instance ... ok
Proof: run srctempl | test a_template_of_a_nature_generic_module_runs_through_its_type ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C112** - A placement through a template param bounded by `Fn(P)` takes at each node input what a placement of the module takes at that port: `gnd` or a node of a compatible nature, `E0251` for another nature. (§8.1.10, §8.4.5; fix round 2, coordinator follow-up)
Proof: run typestempl | test gnd_connects_through_a_callable_bound_as_at_a_placement ... ok
Proof: run elabtempl | test gnd_at_a_placement_through_a_callable_bound_is_ground ... ok
Proof: run srctempl | test gnd_connects_through_a_callable_bound ... ok

**C113** - A `let` bound to a node keeps the node's nature: `v(y)` after `let y = x;` probes `x` by its nature's access, in a plain module, through a child placed by name and through a template, and lowers to the node's probe. (§6 rule 4, §8.4.8; fix round 4, finding 16)
Proof: run typeshandles | test a_let_bound_to_a_node_keeps_its_nature_for_a_probe ... ok
Proof: run elabhandles | test a_let_bound_to_a_node_probes_the_node ... ok
Proof: run srctempl | test a_let_bound_to_a_node_probes_the_node ... ok

**C114** - A `let` bound to a node is a contribution target as the node is: `let y = x; v(y) +> 1.0;` lowers to `v(x) +> 1.0;`, and through a template to the reached node's potential; the role of an access the resolver left ambiguous is the checker's (`Types::access_role`). (§6 rule 4, §11; fix round 5, finding 17)
Proof: run typeshandles | test a_let_bound_to_a_node_is_a_contribution_target ... ok
Proof: run elabhandles | test a_contribution_through_a_let_bound_to_a_node_is_the_nodes ... ok
Proof: run srctempl | test a_let_bound_to_a_node_is_a_contribution_target ... ok

**C115** - Through a `let` bound to a node an access is judged on the node's nature as for the node named directly: one the nature lacks is `E0206`, a flow contribution on a nature without `flow` is `E0413`. (§6 rule 4, §6 rule 10, §11 rule 11; fix round 5, finding 18)
Proof: run typeshandles | test an_access_a_let_bound_nodes_nature_lacks_is_refused_as_for_the_node ... ok
Proof: run elabhandles | test an_access_a_let_bound_nodes_nature_lacks_is_e0206 ... ok

**C116** - Parentheses around a node argument name the node: `v((x))`, `v((y))`, `v((c0.sn))` and `v((q)) +> ..` probe and contribute as without them, also through a template. (§1.5, §6 rule 4; fix round 5, verifier's note)
Proof: run typeshandles | test parentheses_around_a_node_argument_name_the_node ... ok
Proof: run elabhandles | test parentheses_around_a_node_argument_lower_to_the_node ... ok

**C117** - A node chosen at elaboration (an `if`, a `match` or a block whose choice reads only params, constants, generation indexes and `let`s of those, whatever statements its arms hold; fix round 8, finding 22) names the node it folds to at a probe, a contribution target, a placement's connection and an `Fn` bound's, and its params are structural; a choice that reads a probe, a signal, a core primitive of the run or a variable whose value depends on the run is `E0133` alone, also written straight at a node argument, and so is one through a function that reads the run (elaboration). (§1.4.7, §8.1.2; user ruling 2026-11-24, finding 21; findings 20 and the round-5 note) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run typeshandles | test a_node_chosen_by_the_run_is_e0133 ... ok
Proof: run typeshandles | test a_node_chosen_at_elaboration_is_typed_as_the_node ... ok
Proof: run typeshandles | test a_node_chosen_at_a_node_argument_is_e0133_alone ... ok
Proof: run elabhandles | test a_node_chosen_at_run_time_is_e0133_not_an_elaboration_refusal ... ok
Proof: run elabhandles | test a_node_chosen_at_a_probe_or_a_target_is_e0133_alone ... ok
Proof: run elabhandles | test a_node_chosen_at_elaboration_folds_to_the_node ... ok
Proof: run elabhandles | test a_node_chosen_at_elaboration_connects_through_a_template ... ok
Proof: run elabhandles | test a_node_chosen_through_a_function_that_reads_the_run_is_e0133 ... ok
Proof: run elabhandles | test a_node_chosen_through_lets_and_arm_statements_folds_to_the_node ... ok
Proof: run srctempl | test a_param_picks_the_node_a_module_probes_and_drives ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C118** - A node of a child is a connection like any node: `Th(c0.sn)` with `c0` placed by name or through a template, and `Th(if sel { c0.sn } else { t })`, bind `Th`'s port to the child's node. (§8.1.10, §8.2.8; fix round 8, finding 23)
Proof: run typeshandles | test a_node_of_a_child_is_a_connection ... ok
Proof: run elabhandles | test a_node_of_a_child_connects_to_another_placement ... ok
Proof: run srctempl | test a_childs_node_connects_to_another_placement ... ok

**C119** - The items of a module body appear in any order: a `let` read before it is written (a placement, a node of a child, a value) is typed where it is read and elaborates as the forward body does; two `let`s that read each other's values (at the value inputs of placements without a result, or as plain values) are `E0332`. (§8.2, §8.2.16; fix round 8, coordinator follow-up; re-pinned in fix round 9, finding 25)
Proof: run typeshandles | test a_module_body_is_typed_in_any_order ... ok
Proof: run elabhandles | test a_body_written_in_reverse_order_elaborates_as_written_forward ... ok
Proof: run srctempl | test a_body_written_in_reverse_order_runs_as_forward ... ok

**C156** - A node connection is hardware, not a value: a placement connects to the node of a child written after it, and a cross-coupled pair, an instance wired to its own node, a ring of three and a bank fed from its child's node (`b.c0.sn`) type, bind each port to the node it names and settle where Kirchhoff's law puts them; the reverse body settles where the forward one does; a cycle through values is still `E0332`. (§8.2.14, §8.2.16, §8.3.5; fix round 9, findings 24 and 25, coordinator ruling)
Proof: run typeshandles | test a_cycle_of_node_connections_is_hardware ... ok
Proof: run elabhandles | test a_placement_binds_the_node_of_a_child_written_after_it ... ok
Proof: run elabhandles | test a_cycle_of_node_connections_binds_each_port_to_the_node_it_names ... ok
Proof: run srctempl | test a_placement_connects_to_the_node_of_a_child_written_after_it ... ok
Proof: run srctempl | test a_cycle_of_node_connections_runs_as_hardware ... ok
Proof: run srctempl | test a_cycle_through_values_is_refused ... ok

**C157** - A `let` is a constant of elaboration when its initializer, every value assigned to it and every condition around those assignments fold there: a node chosen by such a `let` (`let s = sel; s = !sel;`, a toggle under a condition over a param, in a `for` over a range) types, folds to the node the `let` holds where the choice reads it and runs; its params are structural; one whose assigned value or surrounding condition reads the run is `E0133` alone, naming that read. (§1.4.7; fix round 9, finding 26, coordinator ruling)
Proof: run typeshandles | test a_let_assigned_only_values_of_elaboration_chooses_a_node ... ok
Proof: run elabhandles | test a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there ... ok
Proof: run srctempl | test a_let_assigned_from_params_picks_the_node ... ok

**C158** - A node connection reached through any depth of children is hardware too: a cross pair and a self-connection through a template two levels deep (`p.c0.sn`) and through a generation (`p.cells[1].sn`) type, bind each port to the node it names and settle where Kirchhoff's law puts them; three levels deep (`p.mid.c0.sn`) the checker types the cycle and elaboration refuses it as it refuses the forward read, by name (L82); a value read through an open `let`'s child (`p.c0.cs` at a value input) is `E0332`. (§8.2.14, §8.2.16, §8.3.5; fix round 10, finding 27) (retired by code-clarity T10: a pin on a sentence of a tracking file; the test was already gone)
Proof: run typeshandles | test a_cycle_of_node_connections_through_any_depth_of_reach_is_hardware ... ok
Proof: run elabhandles | test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok
Proof: run srctempl | test a_cycle_of_node_connections_two_levels_deep_runs_as_hardware ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C159** - A choice whose condition assigns its `let` first (`if { s = !s; s }`, a `match` over such a block, a `while` over a `let` in the condition) folds at elaboration to the node the condition picks and ends, within a time bound at elaboration and from source; one whose condition assigns a value of the run is `E0133` alone, naming the read. (§1.4.7; fix round 10, finding 28)
Proof: run elabhandles | test a_choice_whose_condition_assigns_folds_and_ends ... ok
Proof: run srctempl | test a_condition_that_assigns_its_let_picks_the_node_and_ends ... ok

**C160** - A body `let` read before its text position is read as one written before it, at every site a name resolves to a module-body `let` (callee: closure, value `E0180`, instance `E0333`, template, function value; operand; argument; `as` operand; condition; method receiver; index base; field receiver, node and param; connection; template head through a choice; param value): the two orders give the same diagnostics, and a reverse-order closure callee runs. (§8.2; fix round 13, finding 33)
Proof: run typeshandles | test every_site_reads_a_body_let_written_later_as_one_written_before ... ok
Proof: run elabtempl | test every_callee_form_elaborates_to_what_it_names ... ok
Proof: run srctempl | test every_callee_form_runs_what_it_names ... ok

**C161** - Every alternative of Part III's `callee` (`path`, `call`, `'(' expr ')'`) is called by its value: a template (in place, a `const`, a choice by `if`, `match` or a block, a function result) places it with the params it holds, a function (a closure literal, a named function, a function's result, a choice of functions) is called, a value of any other type is `E0180`, and a choice the run makes is `E0362`; no form reaches a generic refusal. (Part III callee production, §1.5, §8.4.4, §8.4.6; fix round 13, finding 34)
Proof: run typestempl | test every_callee_form_the_grammar_admits_is_called_by_its_value ... ok
Proof: run elabtempl | test every_callee_form_elaborates_to_what_it_names ... ok
Proof: run srctempl | test every_callee_form_runs_what_it_names ... ok
Proof: run tcontrol | test a_function_value_returned_by_a_function_compiles_and_runs ... ok

**C162** - A module's bare name used as a value (`let t = R;`, the head `(R)(x)`, a param value, a function's result) is the template `R {}`, every param at its default: `let t = R; t(x)` and `(R)(x)` place the instance `R(x)` places; a required param it leaves unbound is `E0319` alone; the phase-C `P0001` "placement through a template value or a function" is gone, and no cascade follows a head the checker refused. (§8.4.1 amendment, AFK default 2026-11-24 pending user review; fix round 14, finding 35)
Proof: run typestempl | test a_bare_module_name_used_as_a_value_is_its_template_with_every_default ... ok
Proof: run elabtempl | test a_bare_module_name_used_as_a_value_places_its_template_with_every_default ... ok
Proof: run srctempl | test a_bare_module_name_places_its_template_with_every_default ... ok
Proof: run typesgen | test phase_d_and_e_forms_stay_p0001_by_name ... ok

**C163** - A template choice whose arms name two modules is one `E0180`; a call through the `let` that holds it adds no diagnostic, and a value of a module's type is never called "no function and no template". (§1.5.4, §8.4.4; fix round 15, finding 37)
Proof: run typestempl | test a_choice_of_two_modules_is_one_e0180_and_its_call_adds_none ... ok

**C164** - A template value handed to a param of module type is a const expression over the placing module's params, which become structural: a template literal over a param, a binning function over the parent's length and a choice over a param fold per instance, in the top module and in a placed one whose params its parent binds; one that reads a port is `E0362`. (§8.4.6; fix round 15, finding 36)
Proof: run typestempl | test a_template_param_value_may_read_the_placing_modules_params ... ok
Proof: run elabtempl | test a_template_param_value_over_the_placing_modules_params_folds_per_instance ... ok
Proof: run srctempl | test a_template_param_value_over_the_parents_params_places_per_instance ... ok

**C142** - No item of L58's generics group is refused because it is generic. (GN-27, AC 1; T105, T113)
Proof: run stdlibrary | test each_generics_group_item_resolves_to_its_std_declaration ... ok
Proof: run stdlibrary | test no_generics_group_item_is_refused_because_generic ... ok

**C143** - An item that needs another feature's form is refused naming that form and listed under its id. (GN-27, AC 2; T113)
Proof: run stdlibrary | test a_blocked_item_is_refused_naming_its_form ... ok

**C144** - Every item of the group type-checks under the whole-model check with no instantiation and no error. (GN-27, AC 3; T104)
Proof: run typesmodel | test the_standard_library_is_checked_whole_with_no_instantiation ... ok

**C145** - `since`, `MinPulse`, `Setup`, `inertial` and `Short` run and give their §22 values. (GN-28, AC 1; T106, T107, T108)
Proof: run srcstd | test since_reads_the_time_from_the_last_change ... ok
Proof: run srcstd | test min_pulse_fails_exactly_for_a_narrow_pulse ... ok
Proof: run srcstd | test setup_hold_and_stable_fail_exactly_on_their_violations ... ok
Proof: run srcstd | test inertial_delays_and_swallows_short_pulses ... ok
Proof: run srcstd | test short_makes_one_node_against_section_eleven ... ok

**C146** - Each item whose last other form has landed runs against its §22 entry: `Stable`, `Hold`, `idt`, `idtmod`, `laplace_*`, `zi_*`, `noise_table`. (GN-28, AC 2; T107, T109 to T112)
Proof: run srcstd | test idt_and_idtmod_integrate_against_section_22 ... ok
Proof: run srcstd | test the_laplace_family_gives_the_closed_form_step_response ... ok
Proof: run srcstd | test an_improper_or_unpaired_laplace_is_refused ... ok
Proof: run srcstd | test the_zi_family_follows_its_difference_equation ... ok
Proof: run srcstd | test noise_table_gives_the_interpolated_psd ... ok

**C147** - The spec's `Water` model (Part IV §13) runs over the stdlib `idt`. (GN-28, AC 2; L40; T109)
Proof: run srcstd | test the_water_model_boils_and_vaporizes ... ok

**C148** - `GAPS.md` drops the L58 generics group and L40's last part; the feature's `checks.md` holds every phase. (GN-31, AC 1; T115) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok
Proof: run suite | test repository_records::the_generics_checks_file_holds_every_phase ... ok
Proof: run suite | test repository_records::the_generics_check_ids_are_unique_across_its_phases ... ok

**C135** - A hoisted module whose body holds generation is unrolled under its frame and runs; `GAPS.md` has no L77. (GN-17, AC 1; T118, phase C round 5 finding 18)
Proof: run elabgenr | test generation_in_a_hoisted_module_unrolls_under_its_frame ... ok
Proof: run srcgenr | test a_hoisted_module_with_generation_runs ... ok

**C149** - Every gallery fence and Book row of a library item of the group is held to the checker. (GN-31, AC 2; T114)
Proof: run galleries | test the_fences_of_the_generic_library_are_checked ... ok
Proof: run bookinventory | test refused_rows_and_gaps_section_11_agree ... ok

**C152** - A composite param (an array of coefficients, a table, a tuple) of a module with a result is bound at its placement, and `laplace_*` and `noise_table` read theirs. (GN-28, AC 2; T110a, T110, T112; coordinator ruling 2026-11-24)
Proof: run typesresults | test a_composite_param_of_a_module_with_a_result_checks_at_its_placement ... ok
Proof: run elabresults | test a_composite_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok
Proof: run elabresults | test an_option_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok
Proof: run elabtemplates | test a_template_carrying_composite_params_places_with_and_without_a_result ... ok
Proof: run elabtemplates | test two_templates_with_equal_composite_params_are_one_value ... ok
Proof: run srcgenr | test a_template_carrying_composite_params_runs_through_a_placement ... ok

**C153** - An array of module templates places the element a written, constant or generation index names, and a module with a result placed through a template takes the template's params; an array repeat of a template (`[Scale { r: 7.0 }; 2]`) is that many equal templates, and a callable param of a module that is itself hoisted places its template, forwarded to a second generic module too. (GN-17, AC 1; T119, GAPS L88; fix round 2, findings 9 and 10)
Proof: run elabtemplates | test an_array_of_templates_places_the_element_at_an_index ... ok
Proof: run elabtemplates | test an_array_of_templates_of_a_module_with_a_result_gives_each_read_its_elements_params ... ok
Proof: run typestempl | test an_array_of_templates_of_one_module_checks_and_places_its_element ... ok
Proof: run srcgenr | test a_template_array_element_placed_in_generation_runs ... ok
Proof: run elabtemplates | test equal_arrays_of_templates_share_one_body_keyed_by_value ... ok
Proof: run elabresults | test a_module_with_a_result_placed_through_a_callable_bound_param_takes_the_templates_params ... ok
Proof: run srcgenr | test a_module_with_a_result_through_a_callable_bound_param_runs ... ok
Proof: run elabtemplates | test a_template_param_of_a_module_with_a_result_places_the_templates_params ... ok
Proof: run elabtemplates | test an_array_of_templates_written_as_a_repeat_is_that_many_equal_templates ... ok
Proof: run elabresults | test a_callable_bound_param_of_a_hoisted_module_places_its_template ... ok

**C154** - An `Option` result of a module placed in a body is read in a reaction as the pair of its halves, and `inertial` runs at `Option<u8>`. (GN-28, AC 1; T120, GAPS L89 as found by T108)
Proof: run elabresults | test an_option_result_of_a_hoisted_module_is_read_as_the_pair_of_its_halves ... ok
Proof: run srcstd | test inertial_over_an_option_reads_its_result_in_a_reaction ... ok

**C155** - A param declared as an array of templates (`pair: [Cell; 2]`) runs: its default, an override by a literal, a `const` and a choice, `pair[k](x)` at a written and a generation index, the params its elements read are structural, and a host cannot give it a value (E0802); all of it in a module without a result and in one with a result (§8.3.12, its frame holds the array as a key). (GN-17, AC 1; T121, GAPS L89; fix round 2, finding 8) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run typestempl | test a_param_declared_as_an_array_of_templates_checks_its_default_and_its_placement ... ok
Proof: run elabtemplates | test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
Proof: run elabtemplates | test the_params_an_array_of_templates_reads_become_structural ... ok
Proof: run srcgenr | test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
Proof: run apisession | test a_module_typed_param_is_not_set_by_text ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

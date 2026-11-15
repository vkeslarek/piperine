# msl-elab verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at e01e8537 (41 pass, 0 fail); judged over the tree at 1b7a2869 (docs only since e01e8537)
**Verifier**: independent sub-agent (author != verifier), round 1

## Checks

| Check | Test | Holds | Note |
| --- | --- | --- | --- |
| C1 | `elab_entry.rs` `a_twin_elaborates_to_a_checked_ir` | yes | six twins, `check()` empty, byte-compared with `tests/fixtures/elab/*.txt`. A non-twin model can still reach an ill-formed `Ir` (finding 1) |
| C2 | `elab_entry.rs` `the_design_is_unchanged_by_elaboration` | yes | `dump()` and diagnostics compared before and after for all six twins; `&Design` is enforced by the signature |
| C3 | `elab_entry.rs` `front_end_diagnostics_are_returned_without_an_ir` | yes | front-end diagnostics compared by equality; the type-check path asserts the code |
| C4 | `elab_entry.rs` `an_unknown_or_ported_top_is_one_diagnostic` | yes | one diagnostic each, names the module, span on the name |
| C5 | `elab_entry.rs` `elaborating_twice_prints_the_same_text` | yes | compares the structural text and the full text, origins included |
| C6 | `elab_params.rs` `an_unset_param_takes_its_default` | yes | the given value and the default, per param |
| C7 | `elab_params.rs` `a_required_param_left_out_is_e0319` | yes | code, path `Top.l`, span on the placement |
| C8 | `elab_params.rs` `a_false_where_is_e0317_with_the_path` | yes | code, `Top.r1.r`, span on `-1.0`. Own probe: nested `Top.m.l.r` is named the same way |
| C9 | `elab_params.rs` `a_bound_value_that_is_not_constant_is_refused` | yes | P0001 at `k`, E0321 at `temperature()`, both spans asserted |
| C10 | `elab_params.rs` `a_const_read_is_folded_once` | yes | bit-exact against Rust `f64`, evaluation count pinned |
| C11 | `elab_params.rs` `a_param_read_is_never_folded` | yes | asserts `Expr::Param(0)`, not only the text |
| C12 | `elab_params.rs` `a_where_is_recorded_over_the_body_params` | yes | asserts the condition reads `Param(0)` |
| C13 | `elab_instances.rs` `one_body_per_module_one_instance_per_placement` | yes | counts plus shared body id |
| C14 | `elab_instances.rs` `instances_come_in_pre_order` | yes | the full ordered path list |
| C15 | `elab_instances.rs` `paths_start_at_the_top_module` | yes | `Top.out` without `r#`, `Top.l.inner` declared by `Top.l` |
| C16 | `elab_instances.rs` `signals_are_nodes_and_ports_bind_the_parents` | yes | port to parent node, port to ground, own signal, table size |
| C17 | `elab_instances.rs` `a_node_init_is_recorded_on_body_and_design` | yes | both places |
| C18 | `elab_instances.rs` `a_node_init_that_reads_a_param_is_p0001` | yes | code and span on the value |
| C19 | `elab_instances.rs` `an_unnamed_placement_is_module_hash_k` | yes | `Leaf#0`, `Leaf#2`: the ordinal counts the named placement, as part_vi_io §21 rule 3 states |
| C20 | `elab_instances.rs` `a_module_that_places_itself_is_e0341` | partly | only direct self-placement; the criterion's "through other modules" has no case (finding 4). Own probe: `A -> B -> A` gives E0341 through `Top.a.b.a` |
| C21 | `elab_analog.rs` `value_code_lowers_to_the_same_forms_and_functions_once` | yes | one `FunctionId`, `pnjlim` equal to the `piperine-ir` golden block |
| C22 | `elab_analog.rs` `probes_and_environment_lower_to_their_nodes` | yes | asserts `Expr::Probe` terminals, not only text |
| C23 | `elab_analog.rs` `a_branch_is_one_unordered_pair_with_its_kind` | yes | reversed pair, `i(a)` = `i(a, gnd)`, potential kind |
| C24 | `elab_analog.rs` `a_sum_splits_into_direct_and_ddt_addends` | yes | six sign and parenthesis forms. IEEE reading: a chain with no `ddt` is one leaf (bit-identical to the source); inside a `ddt` chain the runs stay in source order, and a negated run `-(a - b)` equals `(-a) + b` exactly under round-to-nearest except for the sign of an exact zero. The split itself changes grouping (`X - ddt(q) + c + d` gives direct `c + d`, not `(X + c) + d`), as AC 4 requires. No addend is reordered |
| C25 | `elab_analog.rs` `a_reversed_pair_negates_its_addends` | yes | direct, ddt, mixed and pre-negated cases |
| C26 | `elab_analog.rs` `sites_are_numbered_in_source_order_and_named` | yes | `L0 "vd"`, `L1 "w"`, unnamed `limit2`, two stims |
| C27 | `elab_analog.rs` `a_branch_from_a_node_to_itself_is_e0241` | yes | port pair and `(gnd, gnd)`, spans asserted |
| C28 | `elab_analog.rs` `potential_and_flow_on_one_branch_is_e0415` | yes | on the reversed second contribution, span asserted |
| C29 | `elab_unlowered.rs` `a_site_or_contribution_under_if_is_p0001` | partly | covers `if` only. A `limit` or `ac_stim` on the right of `&&` or `&#124;&#124;` is conditional and is not refused (finding 1). Function and closure cases go to the front end (E0121, E0404, E0450), per amendment (d) |
| C30 | `elab_unlowered.rs` `ddt_off_the_top_of_a_flow_contribution_is_p0001` | yes | product, potential, `let`, tolerance. Own probes: `ddt` in an `if` arm or a block is P0001 at the `ddt` |
| C31 | `elab_unlowered.rs` `m_dtemp_or_extern_at_a_placement_is_p0001` | no | asserts E0320 for `m`/`dtemp` and the `extern` span at the declaration: the test holds amendment (d), not the criterion's P0001 at the placement (finding 6) |
| C32 | `elab_unlowered.rs` `a_digital_block_is_p0001` | yes | refused by the type checker, code, name and span asserted |
| C33 | `elab_unlowered.rs` `a_diagnostic_means_no_ir` | yes | two refusals, `Err` |
| C34 | `elab_entry.rs` `every_diagnostic_has_code_span_and_path` | partly | the E0319 case asserts "Top" (also the module name) and the E0341 case asserts "Loop" (the module name, not the path `Top.l.again`): either would pass with no path in the message (finding 3) |
| C35 | `elab_entry.rs` `the_twins_elaborate_without_diagnostics` | yes | all six twins |
| C36 | `elab_instances.rs` `an_empty_module_is_an_empty_body` | yes | every collection of the body and the instance |
| C37 | `elab_instances.rs` `a_module_under_two_parents_is_one_body` | yes | one body, two instances |
| C38 | `elab_instances.rs` `an_unconnected_node_stays_in_the_table` | yes | |
| C39 | `elab_analog.rs` `a_single_addend_is_one_direct_addend` | yes | |

**Golden fixtures (`tests/fixtures/elab/*.txt`).** Checked by hand against `tests/ngspice/{divider,diode_iv,diode_series,ac_rc,tran_rc,isrc_r}.msl`, `devices.msl` and `std/{analog,constants}.msl`: every body, branch, addend, site, child, bound and defaulted param, path, node and `init` matches the source and the spec. `K_B`/`Q` are folded in `vt`, params stay reads, the Vsource/Isource right side (no `ddt`) is one direct addend, the Capacitor is one `ddt` addend, the diode is `direct (id + (gmin() * vd)), ddt (cj0 * vd)`, `TranRc.out` has `init 0.0`, and `IsrcR.i1` binds `(gnd, IsrcR.a)` as the twin writes it. The fixtures are correct against the spec; they are not just a record of current output.

**Origins.** The full text of `DiodeIv` was compared with the source columns: each expression starts where its POM span starts (`(new - old).abs()` at 22:23, its peeled inner `new - old` at 22:24, `K_B` folded at 10:22, the direct addend `id + gmin() * vd` at 41:16). A run synthesized by the split takes its first leaf's origin, which the origin rule allows. Idempotency holds on the full text (C5).

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| Major: a `limit` or `ac_stim` on the right of `&&` or `&#124;&#124;` is not refused, and elaboration builds an ill-formed `Ir`. A debug build panics; a release build returns `Ok` with a non-empty `check()`. This breaks ELAB-01 AC 1 and the fail-loud rule of ELAB-06 | `elab/analog.rs:68` and `:83` gate only on `scope.nesting`; `elab/lower.rs:112` (the `Binary` arm) never raises it for `And`/`Or`; `elab/elaborator.rs:91` is only a `debug_assert!`. Probe: `let c = v(p) > 0.0 && limit(v(p), &#124;a, b&#124; a) > 0.0;` panics with `Broken ... "limit under an if or a short-circuit operator"`. The `&#124;&#124; ac_stim(1.0) > 0.0` case fails the same way | raise `nesting` around the right operand of `&&`/`&#124;&#124;` (or refuse there with `ConditionalLimit`/`ConditionalStim`); add both cases to `a_site_or_contribution_under_if_is_p0001` | 4ed25b62 (C29) |
| Minor: an `Ir` that fails `check()` is returned as `Ok` in release builds, so any future lowering gap reaches the backend without a diagnostic | `elab/elaborator.rs:91` `debug_assert!(ir.check().is_empty(), ...)` | turn a non-empty `check()` into an internal-error diagnostic and `Err` | 6141e926 (unit test `an_ill_formed_ir_becomes_an_internal_error_and_no_ir`) |
| Minor: C34's path assertion is looser than ELAB-05 AC 1. Two of its three path cases match the module name, not an instance path | `tests/elab_entry.rs:171` expects `Top` (a top-level E0319, where path and module name are the same); `:172` expects `Loop`, the module name, while the path is `Top.l.again` | put the E0319 case on a nested placement and assert `Top.l`; assert `Top.l.again` for E0341 | 71576aed (C34) |
| Minor: C20 tests only direct self-placement; "through other modules" has no case | `tests/elab_instances.rs:120` | add `A -> B -> A` and assert E0341 naming `A` (it passes today) | 908f4608 (C20) |
| Minor: the `Child` padding SPEC_DEVIATION is recorded only in a code comment. In the authored-tree view, a child's own signal is printed as bound to `gnd`, which is false. A backend that binds from `Child::nodes` would short that signal to ground | `elab/children.rs:100`; probe `Leaf(p) { signal inner }` prints `child l: Leaf  nodes (s, gnd)` while the instance binds `Top.l.inner`; `piperine-ir/src/check.rs:184` forces one slot per body node; no spec entry and no test | record it in spec.md as an amendment, then either restrict `Child::nodes` to ports in `msl-ir`'s check or add a `Terminal` for "own node"; pin it with a test | 1cafa493 |
| Minor: amendment (d) changes ELAB-06 AC 3 (`m`/`dtemp` give E0320, not P0001; `extern` is refused at its declaration, not at the placement) without a recorded user approval, and C31's test asserts the amendment, not the approved criterion | `spec.md` "Amendment (T8)" has no `Confirmed`/approval line; `tests/elab_unlowered.rs` `m_dtemp_or_extern_at_a_placement_is_p0001` asserts `["E0320"]` | have the user confirm the amendment in spec.md, or change the criterion through a new requirement ID | d7ebbd36 (C31) |
| Minor (docs): tasks.md holds its protocol, matrix, plan and task breakdown twice. The copy starts at a broken heading | `.specs/features/msl-elab/tasks.md:347` `## Checks\` cites. ...`; lines 347-681 repeat lines 5-346 | delete the duplicate block | 46d0a20d |

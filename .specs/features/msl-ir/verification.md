# MSL IR verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at b631738a
**Verifier**: independent sub-agent (author != verifier)

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `model.rs` `ids_are_u32_newtypes_one_per_arena` | yes - size 4 per id, seven types; "no macro" holds by reading `ids.rs` |
| C2 | `model.rs` `a_finished_ir_has_no_mutating_operation` | yes - a text scan for `&mut self` on `Ir` and handle impls; it would miss `self: &mut Self` or a free `fn(&mut Ir)`, but no such writer exists and every field is `pub(crate)` |
| C3 | `model.rs` `equal_builder_sequences_give_equal_irs` | yes - weak sequence (no expression), but the builder has no hash-ordered state; `print.rs` repeats it on the divider |
| C4 | `model.rs` `ir_is_send_and_sync` | yes - compile-time bound; the `OnceLock` memo keeps it true |
| C5 | `model.rs` `handles_are_copy` | yes |
| C6 | `model.rs` `contained_items_come_in_declaration_order` | partly - params, nodes, children are ordered on lists of 2 to 3; branches and sites are only ever checked on 1-element lists (see findings) |
| C7 | `model.rs` `an_instance_reaches_its_parent_and_body` | yes |
| C8 | `model.rs` `path_lookup_finds_instances_and_nodes` | yes - hit, miss, and an instance path refused as a node path |
| C9 | `model.rs` `value_code_of_the_subset_is_representable` | yes |
| C10 | `model.rs` `a_body_holds_params_nodes_branches_and_sites` | yes |
| C11 | `model.rs` `a_probe_spans_two_terminals` | yes - node/node and node/ground |
| C12 | `model.rs` `environment_reads_are_nodes` | yes - all three `Env` variants reached |
| C13 | `model.rs` `limit_and_stim_sites_are_indexed` | yes - with and without seed/phase; uniqueness is C31's |
| C14 | `model.rs` `a_contribution_is_addends_direct_or_ddt` | yes - direct, ddt, negated, both quantities |
| C15 | `model.rs` `a_nature_carries_abstol_and_units` | yes |
| C16 | `model.rs` `every_expression_records_its_type` | yes - `_ => true` for locals, params, `if`, blocks, but the type is a stored field of every `ExprData` |
| C17 | `model.rs` `one_body_serves_every_instance_of_it` | yes |
| C18 | `model.rs` `instances_are_stored_in_pre_order` | yes |
| C19 | `model.rs` `the_node_table_names_nature_declarer_and_init` | partly - asserts `init` is `None` on every node; no design node with an initial potential is ever built (see findings) |
| C20 | `model.rs` `the_authored_tree_is_walkable` | yes |
| C21 | `serde.rs` `storage_types_implement_serde` | yes - compile-time bound on every storage type |
| C22 | `serde.rs` `json_round_trip_gives_an_equal_ir` | yes - `inf`, `-inf`, `-0.0`, NaN, subnormal edge round-trip; `reals_survive_bit_for_bit` pins the bits |
| C23 | `serde.rs` `text_that_is_no_ir_fails_to_deserialize` | yes - seven bad inputs incl. a truncated IR |
| C24 | `print.rs` `printed_text_shows_every_item` | yes - byte-for-byte goldens for the five devices and the divider |
| C25 | `print.rs` `printing_twice_is_byte_identical` | yes |
| C26 | `print.rs` `no_parser_of_the_text_exists` | yes - text scan of `src/` |
| C27 | `print.rs` `unequal_irs_print_different_texts` | no - one pair differing in a param value; the printer omits expression origins and types, so unequal IRs print equal texts (see findings) |
| C28 | `check.rs` `a_built_ir_checks_clean` | yes on the fixtures - but a correct IR with an expression nested deeper than 256 is reported as a cycle (see findings) |
| C29 | `check.rs` `an_out_of_range_id_is_named` | yes - function, body, stmt, nature and parent ids |
| C30 | `check.rs` `an_instance_with_wrong_arity_is_named` | yes - nodes and params |
| C31 | `check.rs` `a_misplaced_or_duplicated_site_is_named` | yes - duplicate, under `if`, in a closure, in a function, contribution under `if` |
| C32 | `check.rs` `a_wrong_type_in_an_analog_position_is_named` | yes - probe, addend, `limit` input, with origins |
| C33 | `deps.rs` `dependencies_follow_locals_and_calls` | yes - `Temperature` reaches the diode only through `vt`, `n` only through `nvt`; resistor, capacitor (charge), Vsource (potential branch, stim), Isource, diode flow/charge/limit all pinned |
| C34 | `deps.rs` `dependency_sets_have_a_fixed_order` | yes - reads made in reverse order come back sorted |
| C35 | `deps.rs` `affine_means_probes_meet_only_linear_operators` | partly - every listed operator has a row, but no row pins how a `limit` read is classed, and the `if`-on-a-constant row asserts affine although the criterion's "exactly when" list does not include `if` (see findings) |
| C36 | `check.rs` `a_top_alone_prints_and_checks` | yes |
| C37 | `model.rs` `a_body_without_analog_has_no_branches` | yes |
| C38 | `check.rs` `a_duplicated_path_is_named` | yes - instances and nodes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| Major - IR-04 AC 4 does not hold: two unequal IRs print the same text | `print.rs:261-317` never prints `ExprData.origin` or `ExprData.ty`. Throwaway at HEAD: the resistor IR with `exprs[0].origin.line` changed to 999 compares unequal (`assert_ne!` passes) and prints a byte-identical text; two NaN payloads in a param value do the same. C27's test changes only a param value | print each expression's origin (or a stable `@file:line:col` per statement/expression) and its type where the form does not fix it, and NaN by bits; add a C27 row that changes only an expression origin and only an expression type | |
| Major - `Ir::check` reports a correct deep expression as a cycle (breaks IR-06 AC 1) | `check.rs:34` `MAX_DEPTH = 256`, `check.rs:439-442`. Throwaway at HEAD: a direct addend `v(a) + 1.0 + ... + 1.0` (300 left-associated `+`) built through `IrBuilder` gets `"expressions nest deeper than 256: a cycle"`. `ir-interp` refuses an IR that fails `check`, so a large lowered model would be refused | detect cycles with a visited/in-progress mark per `ExprId` (and `StmtId`) instead of a depth bound; add a C28 row with a nesting deeper than 256 | |
| Major - the printer aborts the process on a cyclic IR, against its own contract "never panics on a broken IR" | `print.rs:5-6` (the claim), `print.rs:261` `expr` recurses with no guard. Throwaway at HEAD: deserialized resistor JSON with a `Binary.lhs` pointing at itself; `check()` names it (258 entries), `ir.to_string()` dies with `stack overflow, aborting` (SIGABRT). A debugger printing an IR before checking it loses the process, not a test | guard the printer like `check` (an in-progress set per expression, printing `<cycle expr N>`); add a test that prints a self-referencing IR | |
| Minor - the class of a `limit` read is unpinned and departs from the letter of IR-09 AC 3; so does `if` on a constant | `deps.rs:403-405` gives a `limit` read `Affine`; `deps.rs:355` joins `if` arms on a constant condition. Throwaway: `gmin() * limit(v(a), &#124;new, old&#124; new)` answers `is_affine == true`, deps `{env: [Gmin], limits: [0]}`. AC 3 says yes "exactly when every probe reaches the sum only through `+`, `-`, unary `-`, `*`/`/` by ..." - the probe here reaches the sum through `limit`, and the `if` row reaches it through `if`. Changing `deps.rs:405` to `Nonlinear` leaves every test green (the diode is non-affine through `exp` anyway). The choice (affine in the ABI reads, the limited iterate being a `LIMIT` input) is the one `ir-interp` INT-12 needs, but nothing holds it | append a requirement stating affinity is over the declared reads (probes and `limit` sites, abi.md A2) and that an `if` on a constant condition keeps the class; add rows `k * limit(v)` affine and `limit(v) * limit(v)` not affine | |
| Minor - an IR that passes `check` panics in `Body::dependencies` | `check.rs` follows no call edge; `deps.rs:443-445` panics on recursion, `deps.rs:311` on a local read before its `let`. Throwaway: `fn f(x) = f(x)` called from an addend: `check()` returns `[]`, `dependencies(Flow(0))` panics | have `check` report call cycles and reads of a local before its `let` (or before a closure binds it); add a C29-style row for each | |
| Minor - `check`, the printer and the deps walk are exponential on shared expression ids | `check.rs:433` (and `print.rs:261`, `deps.rs:305`) re-walk a shared `ExprId` per path. Throwaway: 41 expressions, `acc = acc + acc` 40 times: `check()` did not return in 30 s | memoize per `ExprId` in `check` and `deps` (the printer may keep tree form but must terminate), or have `check` refuse sharing outright and say so in the docs | |
| Minor - C19 and C6 hold their criteria on degenerate data | C19's test asserts `node.init().is_none()` on every design node; no fixture builds a design node with an initial potential, so a builder that dropped `NodeSpec.init` would pass. C6's branch and site order is only seen on 1-element lists | add a design node with `init: Some(..)` to the divider fixture (or a test body) and assert it; add a body with two branches and two sites and assert their order | |

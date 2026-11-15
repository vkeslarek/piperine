# MSL IR verification

**Verdict**: PASS after fixes - round-1 findings verified closed in round 2; the round-2 Major and three Minor closed by 626bd373 (and 75f2885b for `msl-elab`'s wording), 7c63ce64, 336ff9e9, 94b14b89 without a third round (orchestrator)
**Proofs**: `proofs.md` after the round-2 fixes (round 2 read d6a95fa7; round 1 read b631738a)
**Verifier**: independent sub-agent (author != verifier), rounds 1 and 2

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `model.rs` `ids_are_u32_newtypes_one_per_arena` | yes - size 4 per id, seven types; "no macro" holds by reading `ids.rs` |
| C2 | `model.rs` `a_finished_ir_has_no_mutating_operation` | yes - a text scan for `&mut self` on `Ir` and handle impls; it would miss `self: &mut Self` or a free `fn(&mut Ir)`, but no such writer exists and every field is `pub(crate)` |
| C3 | `model.rs` `equal_builder_sequences_give_equal_irs` | yes - weak sequence (no expression), but the builder has no hash-ordered state; `print.rs` repeats it on the divider |
| C4 | `model.rs` `ir_is_send_and_sync` | yes - compile-time bound; the `OnceLock` memo keeps it true |
| C5 | `model.rs` `handles_are_copy` | yes |
| C6 | `model.rs` `contained_items_come_in_declaration_order` | yes (round 2) - the `Lists` body orders three branches, two `limit` and two `ac_stim` sites declared out of name order |
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
| C19 | `model.rs` `the_node_table_names_nature_declarer_and_init` | yes (round 2) - `Lists.y` carries `init` 0.25 and is asserted; a builder dropping `NodeSpec.init` fails it |
| C20 | `model.rs` `the_authored_tree_is_walkable` | yes |
| C21 | `serde.rs` `storage_types_implement_serde` | yes - compile-time bound on every storage type |
| C22 | `serde.rs` `json_round_trip_gives_an_equal_ir` | yes - `inf`, `-inf`, `-0.0`, NaN, subnormal edge round-trip; `reals_survive_bit_for_bit` pins the bits |
| C23 | `serde.rs` `text_that_is_no_ir_fails_to_deserialize` | yes - seven bad inputs incl. a truncated IR |
| C24 | `print.rs` `printed_text_shows_every_item` | yes - byte-for-byte goldens for the five devices and the divider |
| C25 | `print.rs` `printing_twice_is_byte_identical` | yes |
| C26 | `print.rs` `no_parser_of_the_text_exists` | yes - text scan of `src/` |
| C27 | `print.rs` `unequal_irs_print_different_texts` | yes (round 2) - eight single-field edits (origin line, origin file, type, NaN payload, instance name, top, unreachable expr, a name with a line break); a throwaway mutation of every JSON leaf of the corpus IR (3479 unequal mutants over 1569 leaves) found no two unequal IRs printing alike |
| C28 | `check.rs` `a_built_ir_checks_clean` | yes (round 2) - 300- and 10 000-term sums and `acc + acc` x40 check clean; the walk is iterative and memoized per (expr, context) |
| C29 | `check.rs` `an_out_of_range_id_is_named` | yes - function, body, stmt, nature and parent ids |
| C30 | `check.rs` `an_instance_with_wrong_arity_is_named` | yes - nodes and params |
| C31 | `check.rs` `a_misplaced_or_duplicated_site_is_named` | yes - duplicate, under `if`, in a closure, in a function, contribution under `if` |
| C32 | `check.rs` `a_wrong_type_in_an_analog_position_is_named` | yes - probe, addend, `limit` input, with origins |
| C33 | `deps.rs` `dependencies_follow_locals_and_calls` | yes - `Temperature` reaches the diode only through `vt`, `n` only through `nvt`; resistor, capacitor (charge), Vsource (potential branch, stim), Isource, diode flow/charge/limit all pinned |
| C34 | `deps.rs` `dependency_sets_have_a_fixed_order` | yes - reads made in reverse order come back sorted |
| C35 | `deps.rs` `affine_means_probes_meet_only_linear_operators` | yes (round 2) - with IR-10 reading "probe" as an ABI read, the `limit` and `if` rows are C39's and pinned there |
| C36 | `check.rs` `a_top_alone_prints_and_checks` | yes |
| C37 | `model.rs` `a_body_without_analog_has_no_branches` | yes |
| C38 | `check.rs` `a_duplicated_path_is_named` | yes - instances and nodes |
| C39 | `deps.rs` `a_limit_read_and_a_constant_if_keep_the_class` | yes - added by the round 1 fix (finding 4); mutating the `limit` class or the `if` join fails it |
| C40 | `print.rs` `structural_text_is_the_tree_with_types` | yes (judged by the fix author; no third round) - added by the round 2 fix (Major); structural goldens for the five devices and the divider byte for byte, no `@`, no arena section, every item by name, the diode's five statements as `devices.msl` reads them with no type annotation |
| C41 | `print.rs` `structural_text_ignores_origins_and_arena_order` | yes (judged by the fix author; no third round) - added by the round 2 fix (Major); every origin moved and every arena but the instance table reversed (ids renumbered, `check()` clean) print the same structural text for all six cases while the full text differs; a shared `acc + acc` is numbered by the tree in both orders |
| C42 | `print.rs` `structural_text_shows_forms_and_types` | yes (judged by the fix author; no third round) - added by the round 2 fix (Major); a type, an operator, a constant, a terminal, a bound value and a local's type each change the structural text and show in it |
| C43 | `print.rs` `display_stays_the_full_form` | yes (judged by the fix author; no third round) - added by the round 2 fix (Major); `Display` keeps origins and the arena section; the full goldens moved unchanged to `tests/golden/full/` and C24 still compares them |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| Major - IR-04 AC 4 does not hold: two unequal IRs print the same text | `print.rs:261-317` never prints `ExprData.origin` or `ExprData.ty`. Throwaway at HEAD: the resistor IR with `exprs[0].origin.line` changed to 999 compares unequal (`assert_ne!` passes) and prints a byte-identical text; two NaN payloads in a param value do the same. C27's test changes only a param value | print each expression's origin (or a stable `@file:line:col` per statement/expression) and its type where the form does not fix it, and NaN by bits; add a C27 row that changes only an expression origin and only an expression type | 39db898c - C27 `unequal_irs_print_different_texts` (origin, file, type, NaN payload, instance name, top, unreachable expr, odd name rows) |
| Major - `Ir::check` reports a correct deep expression as a cycle (breaks IR-06 AC 1) | `check.rs:34` `MAX_DEPTH = 256`, `check.rs:439-442`. Throwaway at HEAD: a direct addend `v(a) + 1.0 + ... + 1.0` (300 left-associated `+`) built through `IrBuilder` gets `"expressions nest deeper than 256: a cycle"`. `ir-interp` refuses an IR that fails `check`, so a large lowered model would be refused | detect cycles with a visited/in-progress mark per `ExprId` (and `StmtId`) instead of a depth bound; add a C28 row with a nesting deeper than 256 | 0af69acb - C28 `a_built_ir_checks_clean` (300- and 10 000-term sums) |
| Major - the printer aborts the process on a cyclic IR, against its own contract "never panics on a broken IR" | `print.rs:5-6` (the claim), `print.rs:261` `expr` recurses with no guard. Throwaway at HEAD: deserialized resistor JSON with a `Binary.lhs` pointing at itself; `check()` names it (258 entries), `ir.to_string()` dies with `stack overflow, aborting` (SIGABRT). A debugger printing an IR before checking it loses the process, not a test | guard the printer like `check` (an in-progress set per expression, printing `<cycle expr N>`); add a test that prints a self-referencing IR | 39db898c - `print.rs` `a_cyclic_deep_or_shared_ir_prints` (run print) |
| Minor - the class of a `limit` read is unpinned and departs from the letter of IR-09 AC 3; so does `if` on a constant | `deps.rs:403-405` gives a `limit` read `Affine`; `deps.rs:355` joins `if` arms on a constant condition. Throwaway: `gmin() * limit(v(a), &#124;new, old&#124; new)` answers `is_affine == true`, deps `{env: [Gmin], limits: [0]}`. AC 3 says yes "exactly when every probe reaches the sum only through `+`, `-`, unary `-`, `*`/`/` by ..." - the probe here reaches the sum through `limit`, and the `if` row reaches it through `if`. Changing `deps.rs:405` to `Nonlinear` leaves every test green (the diode is non-affine through `exp` anyway). The choice (affine in the ABI reads, the limited iterate being a `LIMIT` input) is the one `ir-interp` INT-12 needs, but nothing holds it | append a requirement stating affinity is over the declared reads (probes and `limit` sites, abi.md A2) and that an `if` on a constant condition keeps the class; add rows `k * limit(v)` affine and `limit(v) * limit(v)` not affine | c1804e9d - C39 `a_limit_read_and_a_constant_if_keep_the_class` (IR-10, spec amendment under IR-09 AC 3) |
| Minor - an IR that passes `check` panics in `Body::dependencies` | `check.rs` follows no call edge; `deps.rs:443-445` panics on recursion, `deps.rs:311` on a local read before its `let`. Throwaway: `fn f(x) = f(x)` called from an addend: `check()` returns `[]`, `dependencies(Flow(0))` panics | have `check` report call cycles and reads of a local before its `let` (or before a closure binds it); add a C29-style row for each | 0af69acb - `check.rs` `a_recursive_call_or_an_unbound_local_is_named` (run check) |
| Minor - `check`, the printer and the deps walk are exponential on shared expression ids | `check.rs:433` (and `print.rs:261`, `deps.rs:305`) re-walk a shared `ExprId` per path. Throwaway: 41 expressions, `acc = acc + acc` 40 times: `check()` did not return in 30 s | memoize per `ExprId` in `check` and `deps` (the printer may keep tree form but must terminate), or have `check` refuse sharing outright and say so in the docs | 0af69acb, 934c5132, 39db898c - C28 row `acc + acc` x40, `deps.rs` `a_shared_expression_is_walked_once`, `a_cyclic_deep_or_shared_ir_prints` |
| Minor - C19 and C6 hold their criteria on degenerate data | C19's test asserts `node.init().is_none()` on every design node; no fixture builds a design node with an initial potential, so a builder that dropped `NodeSpec.init` would pass. C6's branch and site order is only seen on 1-element lists | add a design node with `init: Some(..)` to the divider fixture (or a test body) and assert it; add a body with two branches and two sites and assert their order | c5262eb2 - C6 `contained_items_come_in_declaration_order`, C19 `the_node_table_names_nature_declarer_and_init` |
| Major (round 2) - the goldens pin origins no elaborator reading `devices.msl` can produce, so the byte-for-byte contract with `msl-elab` fails on its first comparison | Every expression of a statement shares one hand-picked `Ex::at` origin, and the columns land mid-token or on whitespace: `@devices.msl:6:17` (resistor `where`) is the space before `r`; `8:32` is the `>` of `+>`; `21:30` is the `+` of `+>`; `32:17` is inside `1e-14`; `39:28` is the `k` of `v(a, k)`; `analog.msl:10:24` is inside `K_B`. Since round 1 the arena prints every expression's origin (IR-04 AC 4), so these are part of the text `msl-elab` T8 must reproduce byte for byte (`msl-elab/tasks.md:33`, `:283`). Neither `design.md` nor the spec states what origin an expression carries, nor the arena order (functions before the `where` checks before the `analog` block, as the goldens fix it) | state the rule in `design.md` (e.g. an expression's origin is the 1-based line and column of the first character of its own span; the arena order is the order `msl-elab` lowers in) and rebuild `tests/support/devices.rs` to it, one origin per sub-expression; regenerate the six goldens and re-read them against `devices.msl` / `std/analog.msl` | 626bd373 - IR-11 (spec amendment): `Ir::print_structural`; the goldens `tests/golden/*.txt` are structural, the full ones moved to `tests/golden/full/`; origin rule stated in spec.md and design.md; C40 to C43 (run print); `msl-elab` wording in 75f2885b |
| Minor (round 2) - `Body::dependencies` aborts the process on an expression a few hundred deep, which `check` and the printer accept | `deps.rs` `Walk::expr`/`walk_expr` recurse per operand. Throwaway at HEAD: a left-associated `v(p, n) + 1.0 + ...` on a 2 MiB thread (the test-harness default): 300 terms ok, 500 terms `fatal runtime error: stack overflow, aborting` (SIGABRT) in the debug profile; release survives 700. `check()` and `to_string()` pass the same IR at 10 000. No M1 model nests past ~20, so M1 never meets it, but `ir-interp` calls the walk on load and its tests run in debug on 2 MiB threads | make the walk iterative like `check` and the printer (an explicit stack of pending operands and a results stack), or failing that bound the depth with a `panic!` naming the origin and the bound, documented in rustdoc; add a C33-style row: the 10 000-term sum's dependencies on a spawned 2 MiB thread | 7c63ce64 - iterative walk; `deps.rs` `a_deep_expression_is_walked_without_native_stack` (10 000 terms on a 2 MiB thread, run deps) |
| Minor (round 2) - the dependency walk is still exponential through calls: round-1 finding 6 was closed for shared ids within one owner, not across calls | `deps.rs` `Walk::call` builds a fresh `Scope` (and memo) per call. Throwaway: 25 functions `f_i(x) = f_{i+1}(x) + f_{i+1}(x)`, `f_24(x) = x`, called once from an addend: `check()` 1.2 ms, `to_string()` 1.4 ms, `dependencies(Flow(0))` 222 s (debug) | memoize a call's result per (function, argument values), e.g. a `HashMap<(FunctionId, Vec<Value>), Value>` on `Walk` (`Value` needs `Eq + Hash` or a canonical key); add a row with a doubling call chain under a time bound | 336ff9e9 - callee values memoized per (function, argument values); `deps.rs` `a_doubling_call_chain_is_walked_once_per_signature` (25 levels under 1 s, run deps) |
| Minor (round 2) - `check`'s unbound-local test is by evaluation order, not by scope: an IR that reads a local its path never binds passes | `check.rs` `Walk::bound` is one flat flag per local, set by any earlier `let` or closure entry and never cleared. Throwaway at HEAD: `if true { { let x0 = 1.0; x0 } } else { x0 }` gives `check() == []` (the else arm reads `x0`, bound only in the then arm), and `dependencies` answers from the then-arm binding without a panic; a closure parameter read after the closure, outside it, also checks clean. Round-1 finding 5's fix ("reads of a local before its `let`") holds only along one evaluation order | scope the bound set: restore it after each `if` arm (bound after the `if` = bound in both arms), after a block's statements (its `let`s end with it) and after a closure's body (its parameters end with it); add rows to `a_recursive_call_or_an_unbound_local_is_named` for the other-arm read and the read after a closure | 94b14b89 - locals bound by block and closure scope in `check` and the deps walk; rows in `check.rs` `a_recursive_call_or_an_unbound_local_is_named` for the other-arm read and the read after a closure (run check) |

## Round 2

Read at `021d043f` (proofs at `d6a95fa7`; the commit after is docs only). The crate's six test targets pass. Probes ran as a throwaway test target, deleted, not committed.

### Round-1 closures

| Round-1 finding | Closing commit | Holds | Evidence (round 2) |
| --- | --- | --- | --- |
| Two unequal IRs print the same text | 39db898c | yes | the arena section prints every expression and statement with its type, form by ids and origin, plus every id-level reference; NaN by bits, `-0.0` distinct; names quoted when not plain. Every-leaf mutation of the corpus IR: 3479 unequal mutants, no equal text |
| `check` reports a correct deep expression as a cycle | 0af69acb | yes | iterative walk with an `on_stack` set; 10 000-term sum checks clean; self-loop and block-let-self cycles are named once |
| The printer aborts on a cyclic IR | 39db898c | yes | iterative render; a `Binary` reading itself and a block whose `let` reads the block print `<cycle eN>`; 10 000 deep and `acc + acc` x40 print in ms |
| The class of a `limit` read and of a constant `if` is unpinned | c1804e9d | yes | IR-10 appended (not an edit of IR-09's ACs); C39 holds `k * limit(v)`, `limit(v) + v`, `limit * limit`, `gmin() * limit(v)` deps (site, not nodes), constant/nonconstant `if`; see C39 row |
| An IR that passes `check` panics in `dependencies` | 0af69acb | partly | direct and mutual recursion are named; a read before its `let` in evaluation order is named; a read in the other `if` arm or after a closure is not (new Minor finding) |
| `check`, printer and deps are exponential on shared ids | 0af69acb, 934c5132, 39db898c | partly | holds within one owner (`acc + acc` x40 under 5 s in all three); across calls the deps walk is still exponential (new Minor finding) |
| C6 and C19 on degenerate data | c5262eb2 | yes | see C6 and C19 rows |

### C39 / IR-10

IR-10 is the right shape: the class rule the round-1 finding described, appended as a new requirement with its own check, IR-09 AC 3 left as approved with an amendment note. C39's test would fail if a `limit` read were classed `Nonlinear` (the `k * limit(v)` row) or `Constant` (the `limit * limit` row), if the read counted its input's nodes (the `gmin() * limit(v)` deps row), or if `if` ignored its condition (the `limit(v) > 0` condition row). It holds.

### The goldens as a contract

Deterministic: yes (C25; no hash-ordered state reaches the text, and `printed`/`on_stack` decide only what a deterministic walk already visits in order). Complete: yes (every field equality compares is printed; the mutation probe above). Readable: the tree part reads like the source, one statement per line; two weak spots, not findings: a function whose body is one `if` expression prints on a single line (`pnjlim` is ~600 characters), and two locals of one name (`arg`, `arg`) are told apart only by the arena's `local 4` / `local 5`. Reproducible by `msl-elab`: no, for the origins and the unstated arena order (Major finding above).

### `Body::dependencies` recursion (the open note)

Severity for M1: Minor. Measured, it is worse than the note says: the debug profile aborts at 300 to 500 levels on a 2 MiB thread, not 10 000, and it aborts the process rather than panicking. No M1 model comes near (the diode's deepest expression nests under 20, functions are called not inlined), so M1's results are not at stake; but `check` now accepts what `dependencies` kills, and `ir-interp` runs the walk on load in debug test threads. It is a finding to close before the IR serves lowered models beyond M1; the fix is the iterative walk the other two already use.

### Not findings

- A body with `analog: None` and branches/sites (the `Lists` fixture) builds and checks clean; the edge case "a body without an `analog` block has no branches and no sites" binds what `msl-elab` stores, and nothing in the spec asks `check` to refuse the other.

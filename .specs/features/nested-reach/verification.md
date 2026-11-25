# Nested Reach verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 13c4c1357 (21 pass, `check_proofs.py` rerun by the Verifier at that HEAD: 21 pass, 0 fail; `validate_tasks.py` 0 errors)
**Verifier**: independent sub-agent (author != verifier), Opus

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `elab_nested_reach.rs` `a_node_is_reached_three_and_four_levels_below_the_reader` | yes - asserts the reader's reached node is named by the path and bound to `Top.<R>.sn`, over all nine forms |
| C2 | `source_nested_reach.rs` `a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_params` | yes - `k = 2` and `k = 3` through `Block`/`Wing`, values from `CellOracle` |
| C3 | `source_nested_reach.rs` `a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names` | yes - `cells[1]` at `k = 4, 5` and the by-name iteration `k = 1 + 1` |
| C4 | `elab_nested_reach.rs` `a_reach_is_the_same_wherever_the_models_and_the_top_are_written` | yes - compares node and net bindings of both orders over the nine forms |
| C5 | `elab_nested_reach.rs` `a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_levels_below` | no - asserts the cell instance's `param(0)`, not what the reader's `R.k`/`R.twice` lowers to (finding 3); the claim is held by C8 at run time |
| C6 | `elab_nested_reach.rs` `a_let_over_the_solve_reaches_the_node_it_reads_three_and_four_levels_below_the_reader` | yes - the reader gains the reached `sn` for `vs` (probe: Top lowers `probe(p.mid.c0.sn, gnd)`) |
| C7 | `elab_nested_reach.rs` `a_carrier_is_reached_three_and_four_levels_below_the_reader` | yes - the reached carrier's net is `Top.<R>.q` |
| C8 | `source_nested_reach.rs` `a_cell_three_and_four_levels_down_by_name_reads_its_node_lets_carrier_and_params` | yes - five values against Rust's oracle |
| C9 | `elab_nested_reach.rs` `a_cycle_of_node_connections_three_and_four_levels_deep_binds_each_port` | yes - pair, self, both orders, six shapes |
| C10 | `source_nested_reach.rs` `a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware` | yes - KCL solved by Rust |
| C11 | `source_nested_reach.rs` `a_cycle_of_node_connections_through_a_generation_four_levels_deep_runs_as_hardware` | yes |
| C12 | `types_nested_reach.rs` `every_kind_of_item_types_three_and_four_levels_below_the_reader` | yes |
| C13 | `types_nested_reach.rs` `a_cycle_of_node_connections_three_and_four_levels_deep_types_as_hardware` | yes |
| C14 | `types_nested_reach.rs` `an_item_that_is_not_pub_is_refused_three_and_four_levels_below_the_reader` | yes (a private intermediate name, probed, is `E0343` too) |
| C15 | `elab_nested_reach.rs` `a_missing_or_private_item_is_e0343_three_and_four_levels_below_the_reader` | yes |
| C16 | `docs.rs` `gaps_drops_the_reach_rows_nested_reach_closed` | yes |
| C17 | `spec_text.rs` `section_8_2_8_reads_through_children_of_children_at_any_depth` | yes |
| C18 | `elab_handles.rs` `a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port` | yes |
| C19 | `elab_templates.rs` `a_non_node_item_through_a_template_reads_the_concrete_module` | yes - the re-pin `(2.0 + (2.0 * 2.0))` is the form the one-level read `c0.twice` already printed (row 2 of the same test); no golden of `piperine-ir/tests/golden` holds a reach |

## What was probed (throwaway tests, not committed)

- Depth 5 by name (`p.x.top.mid.c0.{sn,vs,q,k,twice}`), both text orders: runs, one IR node per reached node, `ir.node_at("Top.p.x.top.mid.c0.sn")` resolves.
- Depth 5 mixed (by name over templates over generation, `p.plane.grid.row.cells[i]`): runs, the cell holds the template's `k = 7`.
- A three-way ring at depth 3 and a pair at depth 5 in reverse text order: every port bound to the node it names.
- A reader that is not the top, two variants (`Reader { cell: Cell { k: 2/3 } }`): each variant reaches its own cell.
- Six cross reads at depth 4 between six wrappers: one reached node each, correctly bound.
- Out-of-range iteration at depth 4: `E0342`; a missing index: the named `P0001` "index every loop"; a field of a non-placement `let`: `E0186`; a parent's plain drive at depth 2 and 3: `E0222` (§8.2.10, a language rule).
- The rerun loop (`elaborator.rs:51`): terminates (each pass adds at least one demand not known; a demand is (module, `let`, path of source names), a finite set), deterministic (slot order is the order of `known`, identical within a pass), diagnostics of the discarded passes are dropped (only the last pass's are returned), the POM is read through `&Design` only - the UNBREAKABLE rule holds.
- Elaboration time (debug build, median of 5, `Elaborator::run` only), HEAD vs the merge base a8968de11 (elaborator sources swapped in, nothing else):

  | Design | a8968de11 | HEAD |
  | --- | --- | --- |
  | buck `LoopBench` (closed_loop.msl) | 149.5 ms | 147.3 ms |
  | 200 x `Level2`, no reach | 18.6 ms | 18.5 ms |
  | 200 x `Level4`, no reach | 23.3 ms | 23.4 ms |
  | 200 x `Level2`, `v(wN.c0.sn)` (depth 2, supported before) | 38.2 ms | 65.0 ms |
  | 200 x `Level4`, `v(wN.top.mid.c0.sn)` (depth 4) | refused (L82) | 165.5 ms |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. **Medium** - an unowned reachable `P0001`: a reach through a generic module that places a generic module with its own type parameter is refused "a generic argument that is still a parameter". `module GBank<C: Fn(Electrical)>(a) { cell: C; pub let c0 = cell(a); }`, `module GBlock<C: Fn(Electrical)>(a) { cell: C; pub let bank = GBank::<_>(a) { cell: cell }; }`, Top `let p = GBlock::<_>(a) { cell: Cell { } }` reading `v(p.bank.c0.sn)` is refused; the same design without the read elaborates. No GAPS row, not in the spec's Out of Scope; NR-01 AC 2 ("placed through a template") covers it. | `reach.rs:247-248`: the nested frame's `placement_instance(within, ..)` folds the argument under the body's substitution (`instances.rs:142-144`), not the parent reach frame's `frames[parent].substitution`. Every fixture's template param is module-typed (`cell: Cell`), never a type parameter, so no check exercises it. | apply the parent reach frame's substitution when folding the placement's generic arguments of a frame below a reach frame; add the `GBlock<C>`/`GBank<C>` form to `READS`, or own it with a GAPS row | |
| 2. **Medium** - performance: demands are discovered by re-running the whole elaboration, one pass per hop, so a read through a child of a child (`bank.c0.sn`), which ran in one pass before, now costs two full elaborations (1.7x on 200 instances: 38.2 -> 65.0 ms), and a depth-n read costs n+1 passes of the whole design (depth 4: 165 ms against 23 ms with no read). A large design with a single deep read pays the multiplier on everything. Buck and designs without nested reads are unchanged (table above). | `elaborator.rs:51-57`, `NestedDemands::require` (`reach_through_templates.rs`); the module doc argues a scan cannot see a template's module nor a `pub let`'s reads, but only the hop whose receiver does not resolve to a `Decl` (a type-parameter template, `reach.rs:144` and `reach.rs:168`) needs the run. | seed `known` statically before the first pass: walk each `Field` chain whose hops resolve to `Resolution::Decl` (the deleted `find_nested_demands`, generalised to every hop) and close over the field reads in the bodies of the `pub let`s each path ends on; keep the rerun only as the fallback for a hop through a type-parameter template. The previously supported depth-2 forms then stay one pass, and by-name chains of any depth too. | |
| 3. **Low** - C5's test is looser than its criterion: it asserts the placed cell's `param(0)`, which the placement fixes whether or not the reader reads it; a reader that lowered `R.k` from the wrong frame (e.g. the module default 1.0 in the four by-name forms) would pass. `R.twice` is never asserted at elaboration. | `elab_nested_reach.rs` `a_param_and_a_let_over_the_params_...`: `assert_eq!(cell.param(0), ...)` | assert what the reader lowers (the printed `Top` body, as `elab_templates.rs` does) for `R.k` and `R.twice`, the iteration's value in generation | |
| 4. **Low** - unowned refusal and a cascade: "a node declared in generation of a module a parent reads through" (`reach.rs:350-358`) is reachable at depth 2 and more (`module GenNode(a) { for j in 0..2 { pub signal s: Electrical; } pub let mid = Level2(a); }`, read `v(p.mid.c0.sn)`), the spec's Out of Scope names it but no GAPS row owns it, and it is followed by a second `P0001` "a name of a child that is not one of its nodes, as a node" (`reach.rs:55`) for the same cause. | probe output: `["P0001 a node declared in generation of a module a parent reads through ...", "P0001 a name of a child that is not one of its nodes, as a node ..."]` | add a GAPS row with an owner; suppress the second diagnostic when the first was reported | |
| 5. **Low** - the spec's Out of Scope row "A drive of a carrier of a grandchild (`p.mid.c0.line = v`)" contradicts the code: a resolved carrier of a grandchild driven from the top (`p.mid.c0.line = false` on an `AndWire`) elaborates, with a resolver over two drives (`Top#0`, `Top.p.mid.c0#0`) - an accepted, untested path. | probe IR: `net Top.p.mid.c0.line ... resolved by Top.p.mid.c0.line$resolve of bool drives (..$drive0, ..$drive1)` | either add a check (a source run of the resolved value at depth 3) or correct the Out of Scope row to say what is refused | |
| 6. **Low** (pre-existing, not introduced here) - a read through a hoisted placement is masked by the unowned `P0001` "a hoisted module that holds a placement of a module that is not hoisted", followed by the cascade "a field of a child of a child" (`reach.rs:215-223`); neither has a GAPS row. | probe `module HW(x: f64) -> f64 { signal n: Electrical; pub let w = Level2(n); x }`, read `v(h.w.c0.sn)` | add a GAPS row for the hoisted-holds-placement refusal; drop the cascade | |

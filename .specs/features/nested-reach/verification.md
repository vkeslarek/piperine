# Nested Reach verification

**Verdict**: FAIL (round 2, at 1c69c6df2)
**Round**: 2
**Proofs**: `proofs.md` at 1c69c6df2 (`check_proofs.py` rerun by the Verifier: 25 pass, 0 fail; `validate_tasks.py` 0 errors). Round 1 was at 13c4c1357 (21 pass).
**Verifier**: independent sub-agent (author != verifier), Opus

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `elab_nested_reach.rs` `a_node_is_reached_three_and_four_levels_below_the_reader` | yes - asserts the reader's reached node is named by the path and bound to `Top.<R>.sn`, over all nine forms |
| C2 | `source_nested_reach.rs` `a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_params` | yes - `k = 2` and `k = 3` through `Block`/`Wing`, values from `CellOracle` |
| C3 | `source_nested_reach.rs` `a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names` | yes - `cells[1]` at `k = 4, 5` and the by-name iteration `k = 1 + 1` |
| C4 | `elab_nested_reach.rs` `a_reach_is_the_same_wherever_the_models_and_the_top_are_written` | yes - compares node and net bindings of both orders over the nine forms |
| C5 | `elab_nested_reach.rs` `a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_levels_below` | yes (round 2) - asserts the reader lowers `direct k` and `direct (k * 2.0)` at the placement's value over eight forms, generic and generation included (round 1: no, finding 3) |
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
| C20 | `elab_nested_reach.rs` `a_drive_of_a_resolved_carrier_of_a_cell_three_levels_down_is_held_beside_the_cells_own` | yes - asserts the resolver's two drives and both holders |
| C21 | `source_nested_reach.rs` `a_parent_drives_a_resolved_carrier_of_a_cell_three_levels_down` | yes - false with the top's drive, true without it (`QuietBench`) |
| C22 | `source_nested_reach.rs` `a_cell_through_generic_templates_three_and_four_levels_down_reads_its_node_lets_carrier_and_params` | yes - `GBlock`/`GWing`/`GPlane` at `k = 2, 3, 5` against the oracle |
| C23 | `elab_nested_reach.rs` `a_reach_refused_for_its_cause_reports_that_cause_once` | yes - exactly one diagnostic, `P0001` naming the cause, for L94 (depth 1 and 2) and L95 |

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
| 1. **Medium** - an unowned reachable `P0001`: a reach through a generic module that places a generic module with its own type parameter is refused "a generic argument that is still a parameter". `module GBank<C: Fn(Electrical)>(a) { cell: C; pub let c0 = cell(a); }`, `module GBlock<C: Fn(Electrical)>(a) { cell: C; pub let bank = GBank::<_>(a) { cell: cell }; }`, Top `let p = GBlock::<_>(a) { cell: Cell { } }` reading `v(p.bank.c0.sn)` is refused; the same design without the read elaborates. No GAPS row, not in the spec's Out of Scope; NR-01 AC 2 ("placed through a template") covers it. | `reach.rs:247-248`: the nested frame's `placement_instance(within, ..)` folds the argument under the body's substitution (`instances.rs:142-144`), not the parent reach frame's `frames[parent].substitution`. Every fixture's template param is module-typed (`cell: Cell`), never a type parameter, so no check exercises it. | apply the parent reach frame's substitution when folding the placement's generic arguments of a frame below a reach frame; add the `GBlock<C>`/`GBank<C>` form to `READS`, or own it with a GAPS row | f01a3643f - C22, and the `G*` forms of `READS` under C1, C4, C6, C7, C9 |
| 2. **Medium** - performance: demands are discovered by re-running the whole elaboration, one pass per hop, so a read through a child of a child (`bank.c0.sn`), which ran in one pass before, now costs two full elaborations (1.7x on 200 instances: 38.2 -> 65.0 ms), and a depth-n read costs n+1 passes of the whole design (depth 4: 165 ms against 23 ms with no read). A large design with a single deep read pays the multiplier on everything. Buck and designs without nested reads are unchanged (table above). | `elaborator.rs:51-57`, `NestedDemands::require` (`reach_through_templates.rs`); the module doc argues a scan cannot see a template's module nor a `pub let`'s reads, but only the hop whose receiver does not resolve to a `Decl` (a type-parameter template, `reach.rs:144` and `reach.rs:168`) needs the run. | seed `known` statically before the first pass: walk each `Field` chain whose hops resolve to `Resolution::Decl` (the deleted `find_nested_demands`, generalised to every hop) and close over the field reads in the bodies of the `pub let`s each path ends on; keep the rerun only as the fallback for a hop through a type-parameter template. The previously supported depth-2 forms then stay one pass, and by-name chains of any depth too. | partly, 4169b043a - a node, carrier, param or `let` over the params is one pass at any depth (measured); a `pub let` over the cell's own nodes still is not: round-2 finding 7 |
| 3. **Low** - C5's test is looser than its criterion: it asserts the placed cell's `param(0)`, which the placement fixes whether or not the reader reads it; a reader that lowered `R.k` from the wrong frame (e.g. the module default 1.0 in the four by-name forms) would pass. `R.twice` is never asserted at elaboration. | `elab_nested_reach.rs` `a_param_and_a_let_over_the_params_...`: `assert_eq!(cell.param(0), ...)` | assert what the reader lowers (the printed `Top` body, as `elab_templates.rs` does) for `R.k` and `R.twice`, the iteration's value in generation | d231ad45d - C5 |
| 4. **Low** - unowned refusal and a cascade: "a node declared in generation of a module a parent reads through" (`reach.rs:350-358`) is reachable at depth 2 and more (`module GenNode(a) { for j in 0..2 { pub signal s: Electrical; } pub let mid = Level2(a); }`, read `v(p.mid.c0.sn)`), the spec's Out of Scope names it but no GAPS row owns it, and it is followed by a second `P0001` "a name of a child that is not one of its nodes, as a node" (`reach.rs:55`) for the same cause. | probe output: `["P0001 a node declared in generation of a module a parent reads through ...", "P0001 a name of a child that is not one of its nodes, as a node ..."]` | add a GAPS row with an owner; suppress the second diagnostic when the first was reported | 816f3fddc - L94, C23 |
| 5. **Low** - the spec's Out of Scope row "A drive of a carrier of a grandchild (`p.mid.c0.line = v`)" contradicts the code: a resolved carrier of a grandchild driven from the top (`p.mid.c0.line = false` on an `AndWire`) elaborates, with a resolver over two drives (`Top#0`, `Top.p.mid.c0#0`) - an accepted, untested path. | probe IR: `net Top.p.mid.c0.line ... resolved by Top.p.mid.c0.line$resolve of bool drives (..$drive0, ..$drive1)` | either add a check (a source run of the resolved value at depth 3) or correct the Out of Scope row to say what is refused | 209af49cd - C20, C21 |
| 6. **Low** (pre-existing, not introduced here) - a read through a hoisted placement is masked by the unowned `P0001` "a hoisted module that holds a placement of a module that is not hoisted", followed by the cascade "a field of a child of a child" (`reach.rs:215-223`); neither has a GAPS row. | probe `module HW(x: f64) -> f64 { signal n: Electrical; pub let w = Level2(n); x }`, read `v(h.w.c0.sn)` | add a GAPS row for the hoisted-holds-placement refusal; drop the cascade | 816f3fddc - L95, C23 |

## Round 2 (at 1c69c6df2)

Pass counts were read by a throwaway `eprintln!` in the `run_with` loop (reverted, not committed), over the twelve `READS` forms x
the five items, depth 5, a reader below the top in two variants, a `let` chain through a wrapper, a template placed through a
template, cycles at depth 4 and through templates in generation:

| Read | Passes |
| --- | --- |
| `R.sn`, `R.q`, `R.k`, `R.twice` at depth 3 and 4, by name, templates, generation, template of template | 1 |
| a `pub let` that itself reads a deep path (`pub let deep = v(p.mid.c0.sn)`), directly or through a wrapper | 1 |
| cycles at depth 4, by name and through templates in generation | 1 |
| `R.vs` (`pub let vs = v(sn)`) at depth 3 / 4 / 5, every form, generic or not | 3 / 4 / 5 |
| any read through a type-parameter template (`GPlane::<_>`), the designed fallback | 4 at depth 4 |

Elaboration time (debug, median of 5, `Elaborator::run` only; round-1 baseline a8968de11 in brackets):

| Design | 1c69c6df2 |
| --- | --- |
| buck `LoopBench` | 147.3 ms (149.5) |
| 200 x `Level2`, no read | 20.3 ms (18.6) |
| 200 x `Level4`, no read | 25.3 ms (23.3) |
| 200 x `Level2`, `v(W.c0.sn)` | 44.5 ms (38.2) |
| 200 x `Level2`, `W.c0.vs` | 71.5 ms (refused, L81) |
| 200 x `Level4`, `v(W.top.mid.c0.sn)` | 64.2 ms (refused, L82) |
| 200 x `Level4`, `W.top.mid.c0.vs` | 203 ms (refused) |

Buck is unchanged. A design with no nested read pays the static scan (+9 %, about 2 ms on 200 instances), and the depth-2 node
read, one pass now, is 16 % over the old single-pass table: both are noted, not findings.

`unsupported_after_no_refusal` (`elaborator.rs`): it drops the refusal whenever the run holds any diagnostic, not only the one
of its cause. Nothing turns silent - a run with a diagnostic is refused (`elaborate`: `Some(instance) if
self.diagnostics.is_empty()`) - but an unrelated earlier error hides a real second refusal at the four sites until the first is
fixed: finding 9.

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 7. **Medium** - the static seed misses the L81 headline: a `pub let` of the cell over its own nodes (`pub let vs = v(sn)`, read as `p.mid.c0.vs`) is not seeded, so the read still costs one full elaboration per hop (3, 4, 5 passes at depth 3, 4, 5), by name and through module-typed templates alike - non-generic reads that need a rerun. 200 x `W.top.mid.c0.vs` elaborates in 203 ms against 64 ms for `v(W.top.mid.c0.sn)`. | `reach_seed.rs` `add_chain`: the closure follows only the field chains (`ReadChain`, `by_owner`) inside the `let`'s value; `vs`'s value reads `sn` as a plain path, which is no chain, so the demand `(module, child, [.., "sn"])` is left to `NestedDemands::require`. | in the closure, also seed the plain-path reads of the `let`'s value that resolve to a port or signal of the holder (and recurse through the `let`s it reads): each becomes the chain's prefix plus that name | |
| 8. **Low** - no test pins the pass count, though it is cheap to expose (the `run_with` loop): finding 7 slipped through green proofs because only time showed it. | `elaborator.rs` `run_with`; no target asserts a count | expose the count of passes (e.g. a crate-visible counter or an `Elaborator` method the tests reach) and assert one pass for every non-generic form of `READS` x every item, and the fallback count for the type-parameter forms | |
| 9. **Low** - `unsupported_after_no_refusal` keys on "any diagnostic so far", not on the cause it means to follow: an unrelated earlier error (another module's, or a sibling read's) suppresses a real `P0001` at `reach.rs` (`reached_terminal`, the three `make_placement_frame` sites). The design stays refused, so nothing is silent, but the second cause surfaces only after the first is fixed. | `elaborator.rs` `unsupported_after_no_refusal`: `if self.diagnostics.is_empty()` | record the refused causes (the module `declares_in_generation` refused, the hoisted module `hoist.rs` refused) and suppress only a read whose frame's module is in that set | |

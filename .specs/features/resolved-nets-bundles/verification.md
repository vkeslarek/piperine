# Resolved Nets: Bundles verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at the commit that follows this file (re-run by `check_proofs.py` after it; the run at 9e9312cec was 64 pass, 0 fail)
**Verifier**: independent sub-agent (author != verifier), Opus, 2026-11-24, over `feature/resolved-nets-bundles` at 51bd25e74
**Scope**: the whole feature (T1 to T23, design D11 and D12), the merge of `main` at e9fefe239 (the `Fn(Cell)` bound fix) and the solver change 9e9312cec. The B1 round (verdict PASS at 73cef16b, finding F0 closed by cf54677e) is kept at the end.

`origin/main` has moved by one commit (7cf35a4f7, `test(book): drop the unused Refusal import in book_values`) since the merge at e9fefe239. It is not merged here: the verdict is FAIL, so the final full suite was not run.

The `**Build**` line is clean at 51bd25e74 (no warning). `validate_tasks.py` exits 0 (two granularity warnings, T22 and T23).

## What was probed

These are my own probes at 51bd25e74, run with the debug `piperine` binary (`run --top Top`). They are throwaway and not committed.

- **The analog side reads an element or a window of an array net during a transient (9e9312cec).** In the probe, `Cell(gates[1], a)` and `Cell(gates[2], b)` read elements of `[Bit; 4]`, and `LaneW(lanes[1], l)` reads `lane.bits[1]`. `lanes` is `[Lane; 2]` with `bits: [Bit; 2]`, so the leaf `lanes.bits` is `[[Bit; 2]; 2]` and the port gets a window of it. Commits land at 2, 4, 6 and 8 ns. Every node switches one print row after its commit, the same row as a scalar `Bit` control (`p2`), and nothing switches for an element outside the window. Event delivery for elements and windows is correct.
- **Neighbouring carrier kinds and forms.** Node fields (`Diff`), array-of-carrier fields, `Bit<u8>`, resolved fields and nested `flip` are covered by C17, C25, C26 and C39, and I sampled their tests. `[Lane; N]`, `N + 1`, `[flip Stream<u8>; N]` and generation loops (C41, C42) assert exact per-lane records and exact element names.
- **A bundle through a template (`C: Fn(Pipe)`).** This probe fails. See F1.
- **A large sparse array in a transient after 9e9312cec.** This probe fails. See F2.
- **E0187 on an `out` field of a bundle port driven by commits.** The diagnostic is unactionable. See F3.
- **POM navigability.** Leaves are arena declarations that are no child of any module. `Design::decls()` filters them out (`pom/reflect/mod.rs:401`), and `Port::bundle_view` reflects the view. A field access resolves to the struct's own field. The authored hierarchy is intact (C30 asserts it).
- **Book rows.** natures#8 and modules#10/#11 compute their expected values in Rust (`(v(bl)-1)/1k`, the half-voltage divider through two `rleak`), and cheatsheet#8 runs. `compiler~79f44db1` and `welcome~28a764f7` now refuse an array of module templates, owned by GAPS L88 (`generics` phase E). GAPS L25 is closed, and the §9 and §11 counts agree with `refused_rows_and_gaps_section_11_agree` (C35).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `pom_bundles.rs` `a_struct_of_carriers_is_a_bundle` | yes - asserts `StructData::bundle == Some(true)` for the three shapes (B1 F0, closed cf54677e) |
| C2 | `pom_bundles.rs` `one_driver_fields_carry_a_direction_and_no_other_field_does` | yes |
| C3 | `pom_bundles.rs` `a_struct_mixing_carrier_and_value_fields_is_e0260` | yes |
| C4 | `pom_bundles.rs` `a_misplaced_or_missing_field_direction_is_e0261` | yes |
| C5 | `pom_bundles.rs` `flip_out_and_nested_directions_are_e0262` | yes |
| C6 | `pom_bundles.rs` `a_bundle_declared_as_a_value_is_e0264` | yes |
| C7 | `types_bundles.rs` `a_producer_drives_out_fields_and_reads_in_fields` | yes |
| C8 | `types_bundles.rs` `flip_swaps_every_one_driver_direction_nested_included` | yes |
| C9 | `types_bundles.rs` `an_observer_reads_every_one_driver_field` | yes |
| C10 | `elab_bundles.rs` `two_producers_of_one_field_is_e0220_naming_its_path` | yes |
| C11 | `types_bundles.rs` `driving_an_input_field_is_e0222` | yes - placed directly. A placement through an `Fn(P)` bound escapes the rule (F1), and no check names that path |
| C12 | `source_bundles.rs` `a_producer_consumer_and_observer_run_three_beats` | yes - exact data `[1, 2, 3]` for the consumer and the observer, and the `valid` change instants |
| C13 | `source_bundles.rs` `a_bundle_carrier_connects_field_by_field` | yes |
| C14 | `source_bundles.rs` `a_cell_literal_connects_each_named_field` | yes |
| C15 | `types_bundles.rs` `a_literal_field_of_another_nature_is_e0263` | yes |
| C16 | `types_bundles.rs` `a_literal_outside_a_connection_or_a_bundle_operand_is_e0264` | yes |
| C17 | `source_bundles.rs` `a_diff_bundle_connects_its_nodes_like_ports` | yes |
| C18 | `source_bundles.rs` `field_commits_definitions_and_methods_follow_section_5` | yes |
| C19 | `types_bundles.rs` `a_carrier_method_on_a_field_applies_to_that_field` | yes |
| C20 | `pom_bundles.rs` `a_carrier_method_on_a_whole_bundle_is_e0211` | yes |
| C21 | `types_bundles.rs` `self_on_a_bundle_is_a_read_only_handle` | yes |
| C22 | `source_bundles.rs` `vd_on_a_diff_bundle_gives_the_differential_voltage` | yes |
| C23 | `source_bundles.rs` `beat_fires_on_rising_edges_with_valid_and_ready` | partly - asserts the beats' data `[1, 2, 3]` and "none at the other edges" by count, not the instants of the edges |
| C24 | `types_bundles.rs` `a_write_through_a_bundle_handle_is_e0133` | yes |
| C25 | `source_bundles.rs` `nested_bundles_apply_flip_recursively` | yes |
| C26 | `source_bundles.rs` `every_view_may_drive_a_resolved_field` | yes |
| C27 | `source_bundles.rs` `a_generic_bundle_is_instantiated_per_type_argument` | yes |
| C28 | `source_bundles.rs` `an_array_of_bundles_connects_each_element_field_by_field` | yes - exact record `(1, 0)` |
| C29 | `metadata.rs` `a_bundle_field_is_named_by_its_dotted_path` | yes |
| C30 | `pom_reflect.rs` `a_bundle_port_reflects_its_type_and_view` | yes |
| C31 | `docs.rs` `gaps_no_longer_lists_bundles` | yes |
| C32 | `spec_codes.rs` `no_owned_code_is_raised` | yes |
| C33 | `types_bundles.rs` `a_bundle_port_is_not_a_value_input` | yes |
| C34 | `book/natures.rs` `the_stream_bundle_and_its_slice_run` | yes - two transfers of 5 at 3 us and 5 us |
| C35 | `book/inventory` `refused_rows_and_gaps_section_11_agree` | yes |
| C36 | `elab_bundles.rs` `a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path` | yes |
| C37 | `book/modules.rs` `the_memory_bank_of_bundle_cells_runs`, `book/cheatsheet.rs` `the_template_card_runs` | yes - `bl` at 1 V, row-0 storage nodes at 1 V, low-word-line nodes at 0.5 V, all to 1e-6. The template it runs is a producer-view `Cell`, which is the case F1 leaves correct |
| C38 | `types_bundles.rs`, `elab_bundles.rs`, `source_bundles.rs` loop-variable index tests | yes |
| C39 | `types_bundles.rs`, `elab_bundles.rs`, `source_bundles.rs` array-of-carriers field tests | yes |
| C40 | `types_bundles.rs`, `elab_bundles.rs`, `source_bundles.rs` const-parameter length tests | yes |
| C41 | `types_bundles.rs` `an_array_of_bundles_of_a_generic_length_is_typed_per_instance`, `elab_bundles.rs` generic and computed length, `source_bundles.rs` the four runs | yes - exact per-lane records for N = 2, N = 3, `N + 1` and `[flip Stream<u8>; N]` |
| C42 | `types_bundles.rs` `two_producers_of_one_element_of_an_array_of_bundles_is_e0220_naming_it`, `an_element_of_an_array_of_bundles_no_producer_drives_is_e0187_naming_it`, the two `elab_bundles.rs` generic-instance tests | yes - names `lanes[1].a` and asserts that element 0 is not named |
| C43 | `metadata.rs` `a_field_of_an_element_of_an_array_of_bundles_is_named_element_first`, `elab_bundles.rs` node-field and loop-index tests | yes - exact names `Top.lanes[1].a` and `Top.taps[1].p` with values |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| **F1. Blocker - an `Fn(P)` bound over a bundle ignores the view: a `flip` or observer module is accepted, and a placement through the template silently drives the parent's `in` port, or leaves a `Bit` with no driver at `false`.** The `Fn(Cell)` fix in the merge (e9fefe239) compares the port types as `Ty::Struct` and never compares `BundleView`. `connect_input_field` hands over a literal's field by the direction written in the struct (`data.dir == Some(Dir::Out)`), not by the template module's view. | `crates/piperine-lang/src/types/module_bounds.rs:40` (`Some(Some(want)) if want == have`) and `crates/piperine-lang/src/types/bundle.rs:90`. Scenario: `struct Pipe { out q: Bit<u8>, in d: Bit<u8> }` with `module Snk(p: flip Pipe) { p.d = p.q + 2; }` and `module Bank<C: Fn(Pipe)>(in d: Bit<u8>) { cell: C; signal s: Bit<u8>; let one = cell(Pipe { q: s, d: d }); }`. `Top` holds `signal d: Bit<u8> = 5` and places both `Bank { cell: Drv {} }` and `Bank { cell: Snk {} }`. `piperine run --analysis op` gives `Top.a.s = 3`, where `Drv` alone gives 6: `Snk` wrote `Top.d` from 5 to 2 through the `in d` port of `Bank`, with no diagnostic. The same `Snk` placed directly is `E0314` (an `in` port at an `out` port) and `E0187`. With `module Obs(in p: Pipe)` and `s: Bit`, `Top.a.s` is `false` with no `E0187`, although nobody drives `s` and `bool` has no `Default` | make the view part of what the bound compares: a module whose bundle port's view differs from the bound input's is `E0360` naming the port, or the bound spells the view (`Fn(flip Cell)`); hand over a literal's fields by the template's view. Add a check with a `flip` and an `in` module through `Fn(P)` (E0360, or E0314/E0187 as direct placement gives) | 271f33816: the bound's input carries its view; a module of another view is E0360, a literal's and a carrier's fields are handed over by the view (E0314 as placing directly); tests `types_bundles` (three), `elab_bundles::a_template_placed_through_a_bound_has_the_bundle_in_the_bounds_view`, `source_bundles::a_bundle_goes_through_a_template_in_the_view_its_bound_writes`. An observer is refused at the bound (E0360), not by E0187 at the body. |
| **F2. Major - 9e9312cec makes every transient materialise every array net densely, the arrays no analog function reads included. This defeats the sparse storage of §5.1.8 (CV-27).** `DigitalSide::mark` (with `settled`), `DigitalSide::state` and the run-end re-entry state now call `read_dense` for every net. `Store::dense_value` builds `count x size` bytes, one `Vec` per element. Before this commit an array net gave its empty `NetBytes`. | `crates/piperine-solver/src/analyses/adapters.rs:155`, `:268`, `crates/piperine-solver/src/analyses/transient.rs:1311`, `crates/piperine-digital-solver/src/store.rs:232`. Scenario: a resistor cell plus `signal mem: [Bit<u8>; K] = [7; K]` that nobody reads, 4 ns transient. K = 1024 takes 1.3 s and 43 MB. K = 2^26 takes 5.5 s and 110 MB. K = 2^28 takes 17.5 s and 306 MB. K = 2^32 does not finish in 150 s, while its `op` finishes at once. A gdb sample stops in `Store::dense_value` under `DigitalSide::mark`. The cost does not grow with the number of steps. The analog boundary also rebuilds a whole array on each change of an element that is read (`instant.rs:181`, `:223`) | copy only the windows the analog copy has slots for (`window_slots`), and keep a net no analog function reads out of the dense bytes (empty, as before). Build the dense bytes of one net only when it has slots, and cut each window straight from `SparseArray` without the whole array. Add a check: a transient with `[Bit<u8>; 1 << 32]` that nobody reads runs, with bounded memory | 8a7abe6cf: `NetSpec::analog_windows`, `Store::analog_value`/`patch_analog`; test `source_carrier_arrays::a_transient_with_an_array_no_analog_function_reads_stays_sparse` (an unread `[Bit<u8>; 2^32]`, a read element switches at 2 ns, bound 30 s: 2 s alone) |
| **F3. Minor - E0187 on an `out` field of a bundle port tells the user to write something the language forbids.** For `module Drv(p: Pipe)` that commits `p.q <- ..` with `q: Bit`, the help reads "write `= value` after the type of `p.q`". §7 rule 10 (`E0264`) forbids an initial value on a bundle port, and the struct field grammar has no initialiser (`struct Pipe { out q: Bit = false }` is a parse error). A registered `Bit` field of a bundle can only be written as a definition from an initial-valued signal, the idiom of E.6 and the Book harness, and nothing says so. | `crates/piperine-lang/src/types/module.rs:699` (the help), `types/nature.rs:285`; spec part II §7 rule 10 with §5.2.6 and §5.2.8 | for a bundle leaf, say how the value is given: a definition from an initial-valued signal, or an `Option` field. If a registered bundle output should be writable, that is a spec question for the user. Give it a GAPS row with an owner | 74b31a963: struct field `= expr` (part II 7 rule 10, appendix B, E0261 for a misplaced one); tests `msl_parser`, `pom_bundles`, `types_bundles`, `elab_bundles`, `source_bundles::a_bit_field_with_a_default_is_written_by_commits`. AFK default, pending user review. |
| **F4. Minor - `NameMap::window_bytes` silently gives empty bytes when a window falls outside its net's bytes.** This happens on a length mismatch between `init`, `size` and `count`. The analog copy then reads an empty slot and nothing reports it, which breaks the fail-loud rule. | `crates/piperine-solver/src/core/bind.rs:155` (`.unwrap_or_default()`) | return a bind error naming the net and the window, or assert the invariant | 8a7abe6cf: `NameMap::window_bytes` is a `Result` naming the net and the window (no seed gives none); test `bind::a_window_outside_its_nets_bytes_is_an_error_naming_the_net_and_the_window` |

## Round B1 (2026-11-21, verdict PASS at 73cef16b)

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F0. C1's test did not assert that the struct is a bundle | `pom_bundles.rs` `a_struct_of_carriers_is_a_bundle` asserted only `codes == []` | assert `StructData::bundle == Some(true)` | cf54677e: C1 asserts `StructData::bundle` is `Some(true)` for `Stream`, an array-of-carriers struct and a nested bundle |

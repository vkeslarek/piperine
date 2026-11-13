# msl-pom verification

**Verdict**: PASS (round 2; round 1 was FAIL)
**Proofs**: `proofs.md` at 5965ea85 (round 2, 104 pass; round 1 at 8616c95b)
**Verifier**: independent sub-agent (author != verifier)

Scope judged: spec.md POM-01..POM-15, tasks.md `## Checks` C1-C95, the diff f871dcac..8616c95b, `docs/pom/`, and the user's intent ("faithful to the language's own structures ... ALL the code ... like Java reflection") plus the CLAUDE.md rule "POM navigability mirrors the source". I wrote probes as a throwaway test target (removed, never committed): glob and re-export cycles, `super` past the root, prelude names in type and expression positions, privacy through expressions and private namespaces, behavior blocks in inline `mod`s, named connections out of order, `use` in a function body, shadowing by ports, closures and locals, `mod` in a function body, qualified names, multi-file serde and determinism, E0330 across generation arms, templates and bundle literals, writers, access aliases on generic natures, `crate`/`super` from loaded files, doc comments across `mod b;`, 12 malformed sources (no panic, `check()` clean), nature-kind edge cases, commits at every depth, serde and `dump()` determinism over all 110 corpus fences, and depth-256 serde and `to_ast`.

What holds: the declaration tree follows the source (signals stay in their module or generation, behavior blocks stay file items linked to their module, generation bodies are not unrolled, templates are not expanded), resolution only fills slots (C79), there are no hash maps in `pom/` and both `dump()` and the JSON are byte-stable, and nothing panicked. All 185 methods in the reflection.md tables exist, the 49 `pom::guide` doctests each assert, and the design's `Expr::resolution`/`Path::target` appear nowhere in the docs.

On the Handoff items:
- **B3 (T12-T17 in one commit, tests written after the code).** The tests are adequate. They assert values, order and parentage, and they would fail if `signals()` were shallow, the arms were reordered, a reaction lost its `Generation` parent, or the nature kinds were mapped differently. My probes of the cases nobody named (`{flow}` alone, `{flow, potential}`, commits under match/for/block/loop/while) agree with the spec.
- **B5 (`lowering_codes`).** Legitimate scoping. The lowering fixtures name things they do not declare, and the 8 codes it drops are exactly the binding codes the spec assigns to resolution. The `pom_resolve`/`pom_classify` targets assert those codes on their own.
- **T24 narrowing.** Covered by `fixture_prelude_builds_clean`.
- **`Design::module(name)`.** The prelude fallback is documented. It returns the first match in arena order, so a nested `a::M` is returned before a root-level `M`. This is documented as "first", so it is not a finding.
- **B4, B5, B6 and B7 gaps.** Covered by the findings below.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `pom_design` `build_retains_name_text_and_ast` | yes |
| C2 | `pom_design` `parse_diagnostics_pass_through_with_file` | yes |
| C3 | `pom_design` `two_builds_dump_identical` | yes - also pins the line order |
| C4 | `pom_design` `design_is_send_and_sync` | yes |
| C5 | `pom_design` `mod_loads_sibling_file_or_mod_file` | yes |
| C6 | `pom_design` `mod_without_file_is_empty_namespace_e0161` | yes |
| C7 | `pom_tree` `root_children_mirror_items_in_source_order` | yes |
| C8 | `pom_tree` `signal_is_declared_in_its_module_only` | yes |
| C9 | `pom_tree` `generation_for_holds_its_signal_once` | yes |
| C10 | `pom_tree` `generation_if_chain_exposes_three_arms` | yes |
| C11 | `pom_tree` `inline_mod_is_a_namespace` | yes |
| C12 | `pom_tree` `item_in_fn_body_is_declared_in_the_fn` | yes |
| C13 | `pom_tree` `ports_in_order_with_direction_type_default` | yes |
| C14 | `pom_tree` `params_type_default_where_required` | yes |
| C15 | `pom_tree` `signal_pub_type_init` | yes |
| C16 | `pom_tree` `body_let_pattern_names_each_component` | yes |
| C17 | `pom_tree` `definition_place_value_after` | yes |
| C18 | `pom_tree` `extern_module_abi` | yes |
| C19 | `pom_tree` `module_result_and_result_type` | yes |
| C20 | `pom_tree` `module_generics_and_where` | yes |
| C21 | `pom_tree` `nature_quantities_and_kind` | yes |
| C22 | `pom_tree` `struct_fields_and_enum_variants` | yes |
| C23 | `pom_tree` `fn_abi_params_ret_body` | yes |
| C24 | `pom_tree` `impl_and_trait_parts` | yes |
| C25 | `pom_tree` `use_tree_flattens_to_imports` | yes |
| C26 | `pom_tree` `const_type_and_value` | yes |
| C27 | `pom_tree` `analog_block_is_file_item_linked_to_module` | yes |
| C28 | `pom_tree` `nested_contribution_is_listed` | yes - covers if, for and match |
| C29 | `pom_tree` `reactions_and_deep_commits` | yes - one level of nesting only; my probe found commits under match/for/block/loop |
| C30 | `pom_tree` `reaction_in_digital_generation` | yes |
| C31 | `pom_tree` `behavior_generic_names` | yes |
| C32 | `pom_corpus` `every_spec_block_round_trips_to_its_ast` | yes for its criterion (95 single-file fences, spans compared through `Debug`); the claim does not extend to multi-file designs (finding 1) |
| C33 | `pom_body` `node_text_is_authored_text_and_enclosing_is_nearest_decl` | yes - audits every node of every arena |
| C34 | `pom_body` `every_expression_form_has_its_kind` | yes - 30 forms by `contains`; `Error` covered separately |
| C35 | `pom_tree` `doc_comments_reach_docs` | yes for one file; does not cover a `mod b;` file (finding 1) |
| C36 | `pom_tree` `attribute_arguments_are_exposed` | yes |
| C37 | `pom_resolve` `block_reads_resolve_to_module_members` | yes |
| C38 | `pom_resolve` `body_names_are_order_independent` | yes |
| C39 | `pom_resolve` `shadowed_local_resolves_to_previous_binding` | yes |
| C40 | `pom_resolve` `binders_scope_their_names` | yes - inside and outside each binder |
| C41 | `pom_resolve` `type_paths_resolve_to_declarations` | yes |
| C42 | `pom_resolve` `builtin_names_resolve_to_table_entries` | yes |
| C43 | `pom_resolve` `use_and_qualified_paths_resolve` | yes for namespace-level `use`; a `use` in a block is not covered and fails (finding 2) |
| C44 | `pom_resolve` `prelude_source_items_are_visible_everywhere` | no - every prelude name the test reads sits in a `use` item, so prelude visibility that only import resolution sees would still pass (my probe shows type and expression positions do work) |
| C45 | `pom_resolve` `unresolved_path_is_one_e0161` | yes |
| C46 | `pom_resolve` `private_item_links_and_reports_e0161` | yes |
| C47 | `pom_resolve` `associated_paths_resolve_to_variant_const_fn` | no - `S::X` (trait-impl const) and `Tr::run` are asserted only as `Decl(_)`, so the wrong declaration would pass |
| C48 | `pom_resolve` `self_type_resolves` | yes |
| C49 | `pom_resolve` `core_name_redefined_is_e0457` | yes |
| C50 | `pom_resolve` `duplicate_member_name_is_e0330` | yes |
| C51 | `pom_resolve` `diagnostics_are_ordered` | yes |
| C52 | `pom_classify` `port_kinds` | yes for the written cases; a generic is classified by the text of its bound (finding 6) |
| C53 | `pom_classify` `struct_with_carrier_field_is_bundle` | yes |
| C54 | `pom_classify` `named_instance_with_connections_and_params` | yes |
| C55 | `pom_classify` `anonymous_instance_in_analog_and_plain_call` | yes |
| C56 | `pom_classify` `struct_expression_classes` | yes |
| C57 | `pom_classify` `instance_through_template_value` | yes |
| C58 | `pom_classify` `behavior_without_module_is_e0401` | yes |
| C59 | `pom_classify` `behavior_for_extern_module_is_e0402` | yes |
| C60 | `pom_classify` `behavior_names_mismatch_is_e0403` | yes |
| C61 | `pom_classify` `access_alias_resolves_by_first_argument` | yes |
| C62 | `pom_classify` `impl_on_module_is_e0130` | yes |
| C63 | `pom_classify` `instance_pub_member_resolves` | yes |
| C64 | `pom_classify` `instance_private_member_is_e0343` | yes |
| C65 | `pom_classify` `bundle_field_resolves` | yes |
| C66 | `pom_classify` `methods_resolve_to_impl_fn_or_builtin` | yes |
| C67 | `pom_classify` `unknown_receiver_is_deferred_without_diagnostic` | yes |
| C68 | `pom_reflect` `lookups_by_name_and_path` | yes |
| C69 | `pom_reflect` `qualified_names` | yes for the criterion's cases (finding 7 covers cases it does not name) |
| C70 | `pom_reflect` `casts_match_kind_only` | yes - every declaration answers exactly one cast |
| C71 | `pom_reflect` `carrier_nature_through_array_and_flip` | yes |
| C72 | `pom_reflect` `instance_connection_by_port` | yes - my probe of out-of-order named connections also holds |
| C73 | `pom_reflect` `handle_equality` | yes |
| C74 | `pom_reflect` `references_in_source_order` | yes |
| C75 | `pom_reflect` `writers_are_definitions_and_commits` | yes |
| C76 | `pom_reflect` `instantiated_by_lists_instances` | yes |
| C77 | `pom_classify` `generation_body_is_not_unrolled` | yes |
| C78 | `pom_classify` `self_placement_under_generation_if` | yes |
| C79 | `pom_reflect` `resolution_creates_no_node` | yes - per-file node counts and `to_ast`, with and without the prelude, over the galleries |
| C80 | `pom_docs` `every_public_pom_type_has_a_heading` | yes |
| C81 | `pom_docs` `every_reflection_method_is_documented` | yes - also requires the method under its own type heading |
| C82 | `pom_docs` `every_b3_production_is_mapped` | yes |
| C83 | `pom_docs` `msl_fences_parse_clean` | yes |
| C84 | `cargo test -p piperine-lang --doc pom::guide` (49 passed) | yes - every fence asserts |
| C85 | `pom_docs` `mkdocs_nav_lists_every_pom_page` | yes |
| C86 | `pom_corpus` `every_spec_block_builds_with_consistent_invariants` | yes - 110 fences; `check_reports_every_kind_of_broken_link` shows `check()` is not vacuous |
| C87 | `pom_body` `error_nodes_are_kept` | yes |
| C88 | `pom_body` `depth_256_builds_on_default_stack` | yes |
| C89 | `pom_design` `mod_with_two_candidates_is_e0161` | yes |
| C90 | `pom_design` `nested_mod_files_follow_rust_layout` | yes |
| C91 | `pom_design` `mod_cycle_is_e0161_and_not_reloaded` | yes |
| C92 | `pom_design` `load_from_disk_equals_in_memory_build` | no - the named test proves only the first half; the unreadable-root half is in `unreadable_root_is_an_empty_root_with_e0161`, which the proof does not name |
| C93 | `pom_reflect` `design_and_storage_types_implement_serde` | yes |
| C94 | `pom_reflect` `gallery_design_round_trips_through_json` | yes - `Design: PartialEq` is derived over every arena; my probe extends this to all 110 fences |
| C95 | `pom_reflect` `json_is_deterministic` | yes |
| C96 | `pom_design` `docs_keep_kind_and_file_across_mod_files` | yes - six pairs with overlapping doc texts, `to_ast() == parse` per file; it does not assert the new `Doc.inner`/`Doc.file` fields directly (my probe: `(false, 0), (true, 1)`, correct) |
| C97 | `pom_resolve` `use_in_a_block_resolves_with_block_scope` | yes - use before the item, alias, glob, enum glob, no leak out of a nested block, one `E0161` for the unresolvable import; my probe in an `analog` block and under an `if` also scopes correctly |
| C98 | `pom_reflect` `references_navigate_to_declarations` | yes - exact declarations through `Design::decl`, `Path::target`, `Expr::{resolution,target,as_path}` for a read, a commit place, a type path and two member accesses; a literal has no target |
| C99 | `pom_classify` `generic_carrier_follows_the_resolved_bound` | yes - unresolved bound gives `Unknown`, a user `x::Nature` gives `ValueInput`, the prelude bound gives `Carrier` (and through an array) |
| C100 | `pom_reflect` `qualified_names_are_unambiguous_and_findable` | yes - exact strings for impl and behavior-block items, two `X` distinct, and every printed name round-trips through `find` |
| C101 | `pom_classify` `named_binding_without_a_target_is_e0161` | yes - exact spans for 5 kinds (connection, param binding, struct field, template field, fn argument) and the unresolved/linked slots |
| C102 | `pom_classify` `missing_instance_member_is_e0343` | yes - one `E0343` at `nope`, nothing else reported, and the member stays `Unresolved` |
| C103 | `pom_resolve` `access_name_resolves_to_its_quantity` | yes - every `v` path (the `access:` declaration and the `v(p)` probe) is the quantity |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1 (Major) `to_ast` is not lossless for a `mod b;` file, and a loaded namespace's `docs()` cannot say which comment is `///` and which is `//!` | Probe: `"/// x\nmod b;"` + b.msl `"//! x"` makes both `top.msl` and `b.msl` rebuild with wrong `inner_docs`/item docs. 5 of 5 pairs whose doc texts overlap are lossy, e.g. `"/// y\nmod b;"` + `"//! x\n//! y"`. `to_ast.rs::file` tells them apart with `written_as(file.text(), d, ..)`, reading a parent-file `Doc.span` against the child file's text (B4 heuristic). `Doc` carries neither an inner/outer flag nor a file id | Store the inner/outer distinction (or the file) on `Doc` and drop the text-at-span heuristic; add a multi-file `to_ast` round trip with overlapping doc texts | 4d56a937 |
| 2 (Major) a `use` inside a function body is not resolved, and a valid read of the imported name reports a false `E0161` (violates POM-08 AC 7 and AC 9) | Probe: `mod a { pub fn x() {} } fn f() { use a::x; x(); }` - the path `a::x` stays `[Unresolved, Unresolved]` with no diagnostic, and `x` gets `E0161 cannot find x`. resolution.md (scope order 1) says block items are in scope | Resolve block-level `use` items in the lexical scope stack; add a check under POM-08 AC 7 | e5cd6aa8 |
| 3 (Major) a reference cannot be followed to a declaration handle: there is no public `DeclId -> Decl` and no `Path::target()`/`Expr::resolution()` | `Decl::new` is `pub(crate)` (reflect/mod.rs:30). design.md:248 lists `Expr::resolution()`, `as_path()` and `Path::resolution()`, `target()`, none of which exist. A caller holding `Resolution::Decl(id)` must index `design.decls()` (it builds a Vec of every declaration). This fails the POM-08 story ("navigate from use to definition") and the user's "like Java reflection" | Add `Design::decl(DeclId) -> Decl` and `Path::target()`/`Expr::resolution()` (or record the design change in design.md), and document them in reflection.md | 11b8f98a |
| 4 (Minor) the C44 test never reads a prelude name outside a `use` item | `pom_resolve.rs` `prelude_source_items_are_visible_everywhere`: `use Mode::A;`, `mod m { use Shared; }`, `use Electrical;` only | Also read a prelude type in a signature and a prelude const in an expression, from a nested namespace | 75fd9b5c |
| 5 (Minor) C47 asserts trait-impl and trait items only as `Decl(_)` | `pom_resolve.rs` `associated_paths_resolve_to_variant_const_fn`: `matches!(x[0][1], Resolution::Decl(_))` for `S::X` and `Tr::run`. They are correct today (probe: impl `X`, trait `run`) | Assert the exact declarations | 75fd9b5c |
| 6 (Minor) a generic bounded by `Nature` is classified `Carrier` by the bound's spelling, even when the bound is unresolved or names another trait | Probe: `module M<N: Nature>(g: N)` with no declaration of `Nature` gives `E0161` on `Nature` and `g.kind() == Carrier`, which is a guess where POM-09 AC 1 gives `Unknown` for unresolved. With a user `trait Nature` in another namespace, `x::Nature` still gives `Carrier` (B6 note) | Classify by the resolved bound; `Unknown` when the bound is unresolved | 75fd9b5c |
| 7 (Minor) `qualified_name()` is ambiguous for impl items and items in behavior blocks, and `find()` does not accept what `qualified_name()` prints | Probe: `impl Tr for S { const X }` gives `"X"`; `analog for M { signal z }` gives `"z"`; `find("a::E::A")` works while `qualified_name()` gives `"a::E.A"`, and `find("a::E.A")` returns none | Prefix impl items with their self type and behavior-block items with their module (or document it); make `find` accept the `.` member form | efb1a9b9 |
| 8 (Minor) some names stay `Unresolved` with no diagnostic, which contradicts resolution.md ("the slot stays `Unresolved`, one `E0161` is reported") | Probe: nature `access: v` paths are `Unresolved` silently; `R(q: x)` with no port `q` and `R(x) { nope: 1.0 }` with no param `nope` report nothing; `c.nope` on an instance of a known module reports nothing, while part III §8.4.8 says "A missing or non-`pub` item is `E0343`" | Report `E0343` for a missing member of a known instance, and either diagnose or document the other silent cases | 75fd9b5c |
| 9 (Minor) the C92 proof names only half of its criterion | tasks.md C92 `Proof: run design / load_from_disk_equals_in_memory_build`; the unreadable-root half lives in `unreadable_root_is_an_empty_root_with_e0161` | Name both tests in the proof (or merge them) | 678a5aca |
| 10 (Cosmetic) stale doc text | `Design::diagnostics` says "parse diagnostics first, in file order", but they are sorted by file/offset/code (design.rs). reflection.md `### Reference` repeats "A node that names a declaration: see `Reflect::references`." twice | Fix both sentences | 11b8f98a, 678a5aca |

## Round 2

Verifier re-check at 5965ea85 (fix commits 4d56a937..678a5aca). I read each fix's test and ran throwaway probes (removed, not committed).

- 1 confirmed - `Doc` now carries `inner` and `file`, and C96 rebuilds every file of six overlapping pairs; on `/// x` + `//! x` my probe gets `docs()` = `[(outer, file 0), (inner, file 1)]`.
- 2 confirmed - C97 covers the block scope; my probes in an `analog` block, under an `if` and in a nested block resolve the import inside and not outside it.
- 3 confirmed - `Design::decl`, `Path::target`/`resolution` and `Expr::resolution`/`target`/`as_path` exist and C98 pins the exact declarations.
- 4 confirmed - C44's test now reads `Shared` as a parameter type, `Mode` as a type and in a path, and `LIMIT` in an expression, from `m::deep`, and asserts the exact prelude declarations.
- 5 confirmed - C47 now asserts `S::X` is the inherent impl's const (distinct from the trait impl's) and `Tr::run` is the trait's own `fn`.
- 6 confirmed - C99.
- 7 confirmed - C100: `impl S.X`, `impl Tr for S.X` and `analog for Z.z` are distinct, and `find` takes every printed name.
- 8 confirmed - C101, C102 and C103 diagnose or resolve each silent case I listed.
- 9 confirmed - the C92 proof now requires both tests to report `ok` (count of 2).
- 10 confirmed - the `Design::diagnostics` doc and the duplicated `Reference` sentence are fixed in both the source and reflection.md.

No finding is reopened. One residual note, not a finding: C96 proves the `inner`/`file` fields only through `to_ast` equality, never by reading them; my probe shows they hold the right values.


# Value Semantics Design

**Spec**: `.specs/features/value-semantics/spec.md` (VS-01 to VS-12)
**Language text**: `docs/spec/` at `cc3599bf` (Appendix G.71, V1 to V7). The text is already amended; this feature changes code, sources and records.

## Approach

Three moves, in this order, so the tree is green after every task:

1. **Accept the amended forms** (phase 2). The checker and the elaborator take a `let` without `mut` as assignable, `str` as a value type, and a range index as an array. The earlier forms still parse.
2. **Rewrite the tree** (phase 3). `std`, the Part VII fences, the examples, the fixtures and every test source move to the amended spelling.
3. **Refuse and remove** (phase 4). The parser reports the removed forms, and the syntax tree, the object model, the printer, the checker and the IR lose what represented them. Phase 5 closes the records.

Refusing first would break `std` and half the test corpus in one commit; rewriting first is impossible while `let x = 0; x += 1;` is `E0183`.

## Where it runs: sequencing with `composite-values` and `generics`

`feature/composite-values` (`42d183db`) is mid-batch B7 (T38 to T43, the checker for aggregate types). It has built `let mut`, places, assignment and loops end to end (T11, T19: `types/assign.rs`, `elab/control.rs`, IR `Local::mutable`), IR slices (T31: `Expr::SliceOf`, `ValueType::Slice`) and, in B7, part of T42 (slices) and T43 (borrows). Nothing of this is on `main`: on `main` an assignment is `P0001`.

**Decision: one line of history.** `composite-values` pauses after B7. `feature/value-semantics` (spec amendment, Book, this plan) is rebased onto the B7 tip (T1), runs its two batches there, and `composite-values` B8 to B10 resume on top of it with the amended tasks below. Folding these tasks into B8 to B10 instead would put a parser change, a `std` rewrite and a corpus rewrite inside batches whose gates are aggregate elaboration; running first costs one rebase and keeps each batch one concern. `generics` phase A already starts after `composite-values` A–B merge, so it starts on the amended language and only its task text changes.

```
main ── composite-values B1..B7 ── value-semantics B1, B2 ── composite-values B8', B9', B10' ── merge ── generics
```

If B7 has committed T42 or T43 before it pauses, T12 removes what they added; if not, B7 ends at T41 and T42 is amended as below. T1 records which on the Handoff.

## What exists, and what changes

| Concern | Today (`composite-values` B7 tip) | After |
| ------- | --------------------------------- | ----- |
| Keywords | `mut`, `ref`, `move` are value keywords the lexer and `msl/parser/pat.rs`, `expr.rs` accept | reserved words: `E0903` at the word, one help per word (`msl/parser/entry.rs`, the table that already reports `type` and `static`) |
| `let mut` in a module body | `E0331` (`msl/parser/check.rs`) | the rule is deleted; the word is `E0903` |
| Reference syntax | `ast::Type::Ref { static_lifetime, inner }`, `Type::Slice`, `Pat::Ref`, `Pat::Ident { by_ref, mutable, .. }`, `UnOp::Deref`, `UnOp::Ref`, a closure's `move` flag; mirrored in `pom/body.rs` (`TypeData::Ref`, `TypeData::Slice`, `PatData::Ref`, `PatData::Ident { by_ref, mutable }`) | all removed from `msl/ast.rs`, `pom/body.rs`, `pom/lower`, `pom/to_ast.rs`, `pom/reflect`, `msl/print.rs`, `msl/format.rs`; the parser reports the form (`E0101`) and recovers by reading the operand, so one use is one diagnostic |
| Places | `types/assign.rs`: `Checker::mutable` holds the bindings declared `mut`; an assignment to another binding is `E0183` | every binding of `DeclKind::Binding` is a place; `DeclKind::FnParam`, a closure parameter, a captured name and a body `let` are not. `Checker::mutable` goes; a new `Checker::assigned` records the bindings a body assigns, for the elaborator |
| IR locals | `piperine_ir::Local::mutable`: "the source wrote `let mut`" | the same field, set from `Checker::assigned`: "the body assigns it". The printer's rule is unchanged (`let mut` in the generated Rust exactly for a `mutable` local), so no golden moves and rustc reports no `unused_mut` |
| Pattern-bound locals that are assigned | not possible (`mut x` in a pattern was not lowered) | the elaborator copies the binding into a fresh assignable local at the head of the arm or of the loop body (`elab/patterns.rs`); the loop's own counter is never touched |
| `str` | `Ty::Ref` of `Ty::Str`; `&str` in `std/io.msl`, `values.msl`, `analog.msl` | `Ty::Str`, a value type without `Repr`; intrinsic signatures in `types/intrinsic.rs` and `elab/intrinsics.rs` match on `str` |
| Range index | `Expr::SliceOf` in the IR, `ValueType::Slice`; the checker's slice rules in `types/aggregate.rs` | `types/lengths.rs` folds the bounds and gives `[T; b - a]`; the elaborator will lower it as the array of its elements (`composite-values` T51'); `SliceOf` and `Slice` leave `piperine-ir` (`model.rs`, `check/aggregates.rs`, `print.rs`, `deps.rs`, `build.rs`), `piperine-interp`'s refusal list and `piperine-transpile/src/print/aggregate.rs` |
| `Clone`, `Copy` | rows of `pom/builtin.rs`; nothing reads them but name resolution | rows deleted; `pom/resolve` reports `E0161` for the name as for any other, and `msl/diag` adds the help when the unresolved name is `Clone` or `Copy` |
| Generated Rust | values are Rust values; every generated struct and enum derives `Clone, Copy` in `print/derive.rs` | unchanged: every MSL value type is plain data, so the Rust type is always `Copy`. This is why "every value copies" costs the backend nothing |
| `resolve` | `fn(&[T]) -> T` in `std/nets.msl`; a resolved nature is `P0001` (L21) | `fn wired<const N: usize>(d: [Logic; N])`, `tri`, `sum` alike. Still `P0001`: running it is `resolved-nets` |

## The spelling map (phase 3)

| Earlier | Amended |
| ------- | ------- |
| `let mut x = e;` | `let x = e;` |
| `&str`, `&'static str` | `str` |
| `fn f<const N: usize>(xs: &[T; N])`, call `f(&xs)` | `fn f<const N: usize>(xs: [T; N])`, call `f(xs)` |
| `fn f(d: &[T])` | `fn f<const N: usize>(d: [T; N])` |
| `*x` on an element from `iter`, `first`, `last`, `get` | `x` |
| `(a..=b).contains(&n)`, `xs.contains(&x)` | `contains(n)`, `contains(x)` |
| `#[derive(.., Clone, Copy, ..)]` | the list without the two names; a derive left empty is deleted |
| `T: Copy`, `T: X + Copy`, `trait Repr: Copy` | no bound, `T: X`, `trait Repr` |
| `move \|x\| e` | `\|x\| e` |
| `ref x`, `mut x`, `&p` in a pattern | `x`, `x`, `p` |

`std/nets.msl` after the map (the target text, also the Part VII §22.3 fences):

```rust
pub fn wired<const N: usize>(d: [Logic; N]) -> Logic {
    d.iter().fold(Logic::Z, |a, x|
        if x.s > a.s { x } else if x.s < a.s { a } else { Logic { v: merge(a.v, x.v), s: a.s } })
}
pub fn tri<T: Repr, const N: usize>(d: [Option<T>; N]) -> Option<T> {
    d.iter().fold(None, |a, x| match (a, x) { (None, y) | (y, None) => y, _ => Some(Unknown) })
}
pub fn sum<const N: usize>(d: [f64; N]) -> f64 { d.iter().sum() }
```

## Components

Every new type and method carries a `///` comment; no free function; names are whole words.

| Component | File | Purpose |
| --------- | ---- | ------- |
| `Places` (replaces the `mutable` set) | `crates/piperine-lang/src/types/assign.rs` | decides whether a target is a place and names it when it is not (`E0183`); records `assigned` |
| `Checker::range_index` | `crates/piperine-lang/src/types/lengths.rs` | folds the bounds of `e[a..b]`, gives the array type, `E0188` for a bound that is not const, the out-of-range refusal |
| `Ty::Str` | `crates/piperine-lang/src/types/ty.rs` | the string value type |
| `RemovedForm` rows | `crates/piperine-lang/src/msl/parser/entry.rs` | the data table of removed forms: gains the reference row and the three markers, loses `&self`, `mut` parameters and double borrows as rows of their own |
| `PatternLocal` | `crates/piperine-lang/src/elab/patterns.rs` | a pattern-bound name the body assigns, lowered as a copy into an assignable local |
| `UnknownName` help | `crates/piperine-lang/src/pom/resolve/` (the unit that builds `E0161`) | the help line for `Clone`, `Copy` and `clone` |

## Tests: where each criterion is proven

| Layer | Target | New or existing |
| ----- | ------ | --------------- |
| Parser diagnostics | `crates/piperine-lang/tests/msl_errors.rs` | existing; fixtures replaced, the three "ahead" lists deleted |
| Checker | `crates/piperine-lang/tests/types_value_semantics.rs` | new |
| Runs from source | `crates/piperine-parity/tests/source_value_semantics.rs`, models in `tests/value_semantics/*.msl` | new |
| IR | `crates/piperine-ir/tests/aggregates.rs`, `control.rs` | existing, amended |
| Standard library | `std_excerpts.rs`, `std_library.rs`, `std_galleries.rs` | existing |
| Tree hygiene | `crates/piperine-lang/tests/suite_hygiene.rs` (a walk for removed forms in `.msl` files) | existing, one test added |
| Records | `spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, `pom_resolve.rs` | existing |
| Book | `tests/book/language.rs`, `types.rs`, `generics.rs`, `modules.rs`, `inventory.tsv` | existing |

## Goldens, pins and tests that may move

| What | Task | Rule |
| ---- | ---- | ---- |
| `msl_errors.rs` fixtures of `&self`, `mut` parameters, double borrows, `E0331` | T9, T10 | replaced by the fixtures of the rows that supersede them; `ROWS_AHEAD_OF_THE_PARSER`, `FIXTURES_BEHIND_THE_SPEC`, `RETIRED_AHEAD_OF_THE_PARSER` deleted |
| `types_control.rs::let_mut_in_a_module_body_is_e0331`, `assigning_a_non_place_is_e0183_naming_the_target` | T2, T9 | the first is replaced by the `E0903` test; the second loses "a local without `mut`" and gains the parameter, `self`-free, capture cases |
| Book tests that pin the earlier spellings (`the_earlier_let_mut_spelling_is_still_known_and_stopped_by_name`, `the_earlier_borrow_and_copy_spellings_are_still_known`, `e0331_a_body_holds_no_variables`, `e0101_there_are_no_mutable_references`) | T14 | replaced by tests of the codes the parser now reports; the status-box sentences they cite leave the Book |
| `pom_resolve.rs` line adding `Clone`, `Copy` to the expected built-ins | T13 | deleted |
| Transpile goldens, twin goldens, `contract.rs` | none | never |

## Changes to `composite-values` (remaining tasks, B7 tail to B10)

| Task | Verdict | Change |
| ---- | ------- | ------ |
| T42 (B7) Checker: arrays, indexing and slices | **change** | drop "`e[a..b]`/`&a[..]` as a borrowed call argument or a method receiver", "`[T; N]` passed to `&[T]`" and "`x` `Copy`" for `[x; N]`; keep array literals, repetition, indexing, element assignment, nested arrays, array patterns. Range indexes and `rest @ ..` as arrays are this feature's T5. Rename the test `arrays_indexing_and_slices_type_check` to `arrays_and_indexing_type_check`; commit `feat(types): arrays and indexing` |
| T43 (B7) Checker: borrows and dereference | **drop** | CV-14 is void (no borrow exists). If already committed, T12 here removes it |
| T44 (B8) Checker: derives, comparison, defaults and moves | **change** | drop `x.clone()` and "a read of a moved non-`Copy` value `E0183`"; drop `Clone`/`Copy` from the H.8 table of `Derives` (seven traits; `Debug`, `Hash` accepted with no effect stays). Tests: drop `a_read_after_a_move_is_e0183`; `derived_traits_give_comparison_clone_and_default` becomes `derived_traits_give_comparison_and_default`. Commit `feat(types): derives, comparison and defaults` |
| T45 (B8) Checker: Appendix H.5 to H.7 | **change** | rows by value: `iter` yields `T`, `first`/`last`/`get` give `Option<T>`, `contains(x: T)`, `filter`'s closure takes `T`; receivers are arrays only ("and slices" goes); no `Ty::Ref` |
| T46, T47 (B8) | **keep** | `Repr` has no `Copy` requirement to check (§2); nothing else moves |
| T48 to T50 (B8) | **keep** | add to T49's source test one copy case per aggregate kind: assign `b = a`, change `b.x`, `a.x` unchanged; pass to a function, caller unchanged (VS-04 for aggregates) |
| T51 (B8) Elaborator: arrays, indexing and slices | **change** | drop "`&a` to a `&[T]` parameter, slices `&a[i..j]` and `&a[..]` (a start after the end panics), slice patterns"; add "a const range index lowers to the array of its elements; `rest @ ..` binds that array; an array passed to a function is a copy". Tests: `slices_give_the_elements_in_range` becomes `a_range_index_gives_the_array_of_its_elements`; `a_slice_whose_start_exceeds_its_end_panics` is dropped (it is the checker's refusal, T5 here). Commit `feat(elab): arrays, indexing and range indexes` |
| T52 (B9) Elaborator: derives and borrows | **change** | drop `clone()` and "element borrows from `iter`, `first`, `last`, `get` read through by `*`"; requirement CV-12 only; test `element_borrows_read_as_the_element` dropped. Commit `feat(elab): derived comparisons and defaults` |
| T53 (B9) Elaborator: H.5 to H.7 as loops | **change** | "`len` a constant for an array, `Length` for a slice" becomes "`len` a constant"; `for` over arrays and chains (no slices); elements are values |
| T54, T55 (B9), T56 to T62 (B10) | **keep** | |
| T63 (B10) bookkeeping | **change** | L3's "slices, `&`" is closed by this feature, not listed as delivered there; the Book status rows it edits are those left after T14 here |
| spec.md CV-01 AC 2, AC 3 | **change** | AC 2: "a binding declared without `mut`" leaves the list (it is a place now); AC 3 (`E0331`) is replaced by VS-02 AC 2 |
| spec.md CV-12 | **change** | derives without `Clone`/`Copy`; no move |
| spec.md CV-13 AC 1, AC 2 | **change** | "passed to a `&[T]` parameter" becomes "passed to a `[T; N]` parameter"; AC 2 is the range index as an array |
| spec.md CV-14 | **drop** | references |
| Done work that stays as built | T2, T8, T9, T19 (IR and printer of assignable locals), T5/T10 "`move` closures" in the printer (the generated Rust still writes `move`; the source word is what goes) | no rework: the IR flag and the generated code are unchanged |

## Changes to `generics` (`origin/feature/generics`, not started beyond B1)

| Task | Verdict | Change |
| ---- | ------- | ------ |
| T16 `GenericSignature` | **change** | `Bound::Library(H8Trait)` has seven traits; a bound naming `Clone` or `Copy` is `E0161` through the same path as any unknown bound name |
| T19 what a bound gives | **change** | nothing is granted by `Copy`: a value of a type parameter is always usable twice. Drop any "used twice needs `Copy`" check |
| T25, T35 `self` and methods | **change** | add: `self` is a copy and not a place; `self.x = e` is `E0183` naming `self` (§3.1.10); a source test of `s = s.step(x)` |
| T30 hand-written `impl Repr` | **change** | drop "`T` must be `Copy` (§2's supertrait)" |
| T18, T20 to T23 generic functions | **keep, one addition** | the `std` functions `wired`, `tri`, `sum`, `conjugate_closed`, `poly_q`, `den_q`, `poly_s` and `table_model` now take arrays by value under `const N`; they are instances of what these tasks already build |
| T41 gallery fences and Book listings | **change** | the listings are in the amended spelling (no `Copy` bound, `mean<const N: usize>`) |
| Phase B (handles, T44 to T52) | **keep** | a handle is the one reference (§1.4.7); nothing changes |
| `resolved-nets` (not tasked yet) | **note for its spec** | `resolve` is a const-generic function instantiated per drive count: all drives, the drives outside a relaxing loop, and zero (§4 rule 8, §8.2.3) |

## Risks

| Risk | Where | Signal | Answer |
| ---- | ----- | ------ | ------ |
| A prefix `&`/`*` reported where a binary operator was meant | `msl/parser/expr.rs` | `a & -b`, `a * *b` fixtures | the operand position decides; fixtures for both sides in T10 |
| Intrinsics matched by the earlier signature | `types/intrinsic.rs`, `elab/intrinsics.rs` | `table_model`, `logic` twins or source tests red after T6 | T6 moves the signature and its match together; gate Full |
| The corpus rewrite changes a test's meaning | `crates/*/tests/**` | a test that asserted a refusal of an immutable assignment | T8 lists every such test on the Handoff and replaces it by the criterion that supersedes it |
| The Book and `GAPS.md` conflict at the rebase | T1 | `content.html`, `inventory.tsv`, `tests/book/*.rs` | the amended wording wins for the language; the "runs today" facts of `composite-values` win for status |

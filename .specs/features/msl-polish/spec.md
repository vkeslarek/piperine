# MSL Polish Specification

**Approved**: 2026-11-14 - user: "sim"

## Problem Statement

An independent audit of the MSL stack (`.specs/features/msl-audit/report.md`, 46 findings, 3 BLOCKER, 18 MAJOR, 25 MINOR) found the specification, the parser and the object model each sound alone and not yet consistent with one another. The specification's own models report `E0161` on core names, an identifier pattern that names a constant is silently turned into a binding, the parser accepts forms the grammar forbids and rejects forms the specification prescribes, and the clause "everything else is Rust" is wider than any parser or type checker can be held to. The type checker and the standard library are next; both need the three layers to agree first.

## Goals

- [ ] Every audit finding except A-03 is closed by a requirement of this specification, with a test that fails if it reopens (traceability table below: 45 of 46 mapped, A-03 deferred).
- [ ] `docs/spec/` states the sixteen decisions D1 to D3 and D5 to D17 normatively; a test target (`spec_text`) keeps the wording.
- [ ] The parser accepts exactly what the amended grammar prescribes: each form B.3.6 removes gets one targeted diagnostic, each form the decisions take in parses, prints and round-trips.
- [ ] The POM resolves the language's own names, reports structural errors under the specification's codes, and offers body navigation and validated deserialization.
- [ ] `docs/pom/` maps the 65 constructs of B.1 and every code the POM reports.
- [ ] `cargo test -p piperine-lang` is green at every commit, the spec corpus (`msl_corpus`, `pom_corpus`) included.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| D4 and A-03: a normative standard library `crates/piperine-lang/std/*.msl`, Part VII's fences as excerpts of it, `pub` on its exports, the gate "every gallery of Appendices D, E, F builds with zero diagnostics" | the user scoped it as the next feature, `msl-stdlib`. This feature ships what unblocks it: the core names (D3) and the catalogue names (D2) in the built-in table |
| The part of A-42 that needs the standard library: removing the stand-ins `State`, `Logic` and `init` from `tests/fixtures/pom/prelude.msl`, and a test that builds a gallery model clean | they are standard-library items; `msl-stdlib` replaces the fixture prelude by the real one |
| Type checking and inference: resolving a method of Appendix H on a value, checking `E0102`, `E0904`, `E0906`, the `Fn(P)` bound, argument types | the type checker is a later feature. A value method stays `Deferred(ReceiverTypeUnknown)`; the catalogue (POL-02) is what that checker will be written against |
| `E0375` (a model item named `m` or `dtemp`), `E0141`, the missing-field half of `E0263`, and every other elaboration code not listed in POL-18 and POL-19 | not in the audit; they need const values or types, or belong to elaboration proper |
| Elaboration, const evaluation, monomorphization, the flat netlist | unchanged from `msl-pom`: the POM mirrors the authored code |
| The open items of Appendix G other than G.14 and G.15 | no decision was taken on them |
| The other workspace crates (`piperine-codegen`, `piperine-api`, the CLI, the LSP) | they do not build against the MSL frontend today; `cargo` runs with `-p piperine-lang` only |

---

## Assumptions & Open Questions

The decisions D1 to D17 are those of the audit's table "Spec decisions needed". The user accepted all seventeen recommendations as written on 2026-11-14 (user: "sim") and moved D4 to the next feature.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1 Which Rust forms are in the language (A-17) | B.3.6 is exhaustive. Out: type aliases, associated types, trait-bound `where`, `impl Trait`, qualified paths, restricted `pub(..)`, labels, pattern and `mut` parameters, `&self`. In: function-pointer types `fn(T) -> U` and a negative literal as a const generic argument | audit recommendation D1 | y - user: "sim" |
| D2 Which Rust library names exist (A-18) | a closed catalogue appendix (Appendix H): numeric methods of floats and integers, `Option` combinators, arrays and slices, ranges, the ten iterator adaptors, derivable traits. Everything else is `E0161` | audit recommendation D2 | y - user: "sim" |
| D3 `gnd`, `Nature`, `m`, `dtemp`, `str` (A-01) | core names in the built-in table; §6 rule 11 reworded | audit recommendation D3 | y - user: "sim" |
| D4 Is the stdlib a normative source (A-03) | yes, as `std/*.msl` with the galleries gated on it, in the next feature `msl-stdlib`; out of scope here | audit recommendation D4; scoping by the user | y - user: "sim" |
| D5 Duplicate names, colliding imports, ambiguous globs (A-10, A-40) | one new code in §3.4 with Rust's rules: one name per table per namespace; an item or explicit `use` beats a glob; two globs are an error only when the name is used | audit recommendation D5 | y - user: "sim" |
| D6 Generic names of a behavior block (A-20) | the module's names, in the same order; the Resolution says `repeated`, not `bound positionally` | audit recommendation D6 | y - user: "sim" |
| D7 `&` in argument position (A-19) | a shared borrow of any value is allowed as a call argument; `E0102` narrows to the rest | audit recommendation D7 | y - user: "sim" |
| D8 What `E0910` counts (A-25) | nesting only (delimiters, blocks, unary and type nesting); flat binary, method and `else if` chains are not nesting and get a separate, larger bound | audit recommendation D8 | y - user: "sim" |
| D9 Where docs and attributes may be written (A-07) | everywhere Rust takes an outer attribute, plus reactions, definitions, generation and nature quantities; docs on a quantity and on a reaction are metadata (§21), docs on a statement are kept and ignored | audit recommendation D9 | y - user: "sim" |
| D10 Identifier patterns (A-02) | Rust's rule, stated in Part I: a plain identifier pattern that resolves to a constant, a unit variant or a unit struct is that path, not a binding | audit recommendation D10 | y - user: "sim" |
| D11 Structural checks the POM can do (A-14, A-38) | the POM reports `E0320`, `E0140`, `E0370` and `E0201` under the spec's codes | audit recommendation D11 | y - user: "sim" |
| D12 Out-of-range literals (A-11, A-37) | a parse error under a new code: a float literal whose nearest `f64` is infinite; an integer literal past its suffix type, or past 128 bits without a suffix | audit recommendation D12 | y - user: "sim" |
| D13 Where items may sit (A-21) | namespace scope: files and inline `mod`s; a block and its module in the same file, any namespace of it | audit recommendation D13 | y - user: "sim" |
| D14 Codes of misplaced statements (A-24, A-36) | `E0404` for `<-`, `+>`, `on`, `signal` in any block or body that is not theirs, a module body included; `E0121` only for a `fn` body, where it wins | audit recommendation D14 | y - user: "sim" |
| D15 G.15 raw identifiers | `r#name` is accepted for the seven added keywords | audit recommendation D15 | y - user: "sim" |
| D16 G.14 field-init shorthand (A-23) | `{ r }` is accepted in param blocks and templates as in Rust struct expressions, and the tree keeps the authored form | audit recommendation D16 | y - user: "sim" |
| D17 `(s)` as a place (A-39) | no: a place is a path with indexes and fields; parentheses are rejected in definitions and commits | audit recommendation D17 | y - user: "sim" |
| Number of the D5 code | `E0162` | the next free code after `E0161` in §3.4's table; D5 says `E016x` | y - planner default inside D5 |
| Number of the D12 code | `E0909` | free between `E0908` and `E0910`; D12 offers `E0900` or a new `E0909`, and a row of its own keeps `E0900` the catch-all | y - planner default inside D12 |
| The separate bound of D8 | 4096 chain links on one path of an expression (`MAX_CHAIN`), reported as `E0910` with its own message; nesting stays 256 | D8 says `a separate, larger bound or none`. None is unsafe: the tree of a chain is as deep as it is long, and the printer, the lowering and `Drop` recurse over it. 4096 is 13 times the 300-term sum the audit names | y - planner default inside D8; say so if another bound is wanted |
| Letter and file of the D2 catalogue | `docs/spec/appendix_h_library.md`, Appendix H | H is the next free letter | y - planner default |
| Range of a suffixed signed literal (D12) | `iN` admits up to `2^(N-1)`, the magnitude of its minimum | the minus sign is an operator, not part of the literal (A.2 rule 7), so `-128i8` must lex | y - planner default inside D12 |
| Which words take `r#` (D15) | the seven added keywords only; `r#in`, `r#extern` and `r#type` are `E0900` | D15 names the seven; `in` and `extern` are Rust keywords with a new use and stay reserved as in Rust | y - follows D15 as written |
| A-34: an `extern "C" fn` with a body, a plain `fn` without one | the `E0125` row is extended and the POM reports it | the audit offers `E0125` or a new row; a new code needs a decision nobody took, and the check is structural like those of D11 | y - planner default; the audit names this option |
| A-35: a contribution or commit in a block used as a value | allowed, and B.3.4 says so | B.3.4 item 1 already lists plain `{ }` and the blocks of `if` among the nested blocks that take them; the parser accepts it; no behavior changes | y - settled by B.3.4 item 1; stated explicitly. The audit's other option (forbid it) would be a language change: say so if wanted |
| A-31: `Generation::items()` on an `if` | the items of every arm, in source order | the audit offers this or `none`; one meaning for `for` and `if` (everything directly under the generation) is the least surprising | y - planner default; the audit names this option |
| A-12: the one inclusion rule | typed collections list project files only; `prelude_files()` and `decls()` reach the prelude; by-name lookups prefer the project | §22 rule 3 lets a project replace a stdlib item, so the project's view is the common case | y - planner default; the audit names this option |
| A-14: POM codes or a documented divergence | the spec's codes | D11 decides it for `E0320` and `E0140`; `E0263` (bundle literal) follows the same rule and needs no types | y - follows D11 |
| A-15, A-16: silent `Unresolved` or a diagnostic | `E0206` and `E0211`, reported only when the declared types make the error certain; otherwise `Deferred` | a silent `Unresolved` breaks the project rule `fail loud`; both checks read declared types only, like those of D11 | y - planner default; the audit allows `report` |
| A-09: members through a built-in bound | `Repr`'s three members are built-in rows; any other built-in trait answers `Deferred(BuiltinTraitMember)` | §2 gives `Repr` a closed member list; the members of a catalogue trait need the type checker | y - planner default; the audit names both options |
| Order of the work | spec text first, then the parser, then the POM, then `docs/pom`; a parse-phase error row, a new `rust` fence and a new B.3.2 production land in the commit that makes the tests accept them | three existing guards tie the spec to the code (`every_parse_phase_row_has_a_fixture`, `msl_corpus`, `every_b3_production_is_mapped`); every commit must stay green | y - design decision (MD-57) |
| Pace | one task per commit; Sonnet workers implement, an Opus Verifier judges | the orchestrator's brief | y - settled by the brief |

**Open questions:** none - D1 to D17 were answered by the user on 2026-11-14 and are recorded above; the planner defaults are listed with their reason and are open to correction.

**Implicit-requirement dimensions.** Input validation & bounds: POL-13 (literal range, escapes), POL-14 (places, misplaced statements), POL-15 (nesting and chain bounds). Failure / partial failure: POL-16 (recovery keeps the item), POL-21 (`from_json` fails with a list, never a panic); the parser and `Design::build` still always return a tree and a design. Idempotency: POL-10 to POL-12 (print then parse then print is stable); determinism of `Design::build` is unchanged (`two_builds_dump_identical`). Concurrency / ordering: N/A because nothing here adds shared state; `Design` stays `Send + Sync`, diagnostics stay in source order. State-transition integrity: POL-06 AC 3 (resolution adds links and never restructures: the `Binding` node stays). Data lifecycle / expiry: POL-13 AC 8 and 9 and POL-21 (the serialized form). Observability: N/A because the only output channel is diagnostics, covered by POL-09, POL-14 and POL-18. Auth boundaries & rate limits: N/A because the frontend is an in-process library. External-dependency failure: N/A because this feature adds no dependency on a file, a network or a service (`stacker` is a stack-growth library already built with chumsky).

---

## User Stories

### P1: The Rust boundary is closed ⭐ MVP

**User Story**: As a model author and as the author of the type checker, I want the specification to say exactly which Rust forms and which library names exist so that nothing is accepted or rejected by accident.

**Why P1**: the audit's first MAJOR spec findings (A-17, A-18): no checker or conformance test can be written against "everything else is Rust".

**Acceptance Criteria** (POL-01, B.3.6 is the complete list of what is removed from Rust; findings A-17, A-28; decisions D1):

1. The specification SHALL hold in Appendix B.3.6 a table of removed Rust forms with one row for each of: type aliases, associated types, `static` items, trait-bound `where` clauses, `impl Trait` types, qualified paths `<T as Tr>::X`, restricted visibility `pub(..)`, loop labels, `&self` and `&mut self` receivers, `mut` and pattern parameters, half-open range patterns `..x`, a leading `::` in a path, inner attributes `#![..]`, the never type `!`, raw-pointer types, `union` items and `extern` blocks; and SHALL state that this table, together with §1 rule 2, §3.2 and A.1, is the complete list of what a front end rejects at parse.
2. The specification SHALL state in Appendix B.3.6 that function-pointer types `fn(T, ..) -> U` and a negative literal as a const generic argument (`Int< -1 >`) are part of the language.
3. The specification SHALL give in each row of that table the code the front end reports (`E0101`, `E0900`, `E0903` or `E0905`) and the form to write instead, with the code in a cell other than the first.

**Acceptance Criteria** (POL-02, The inherited library is a closed catalogue; findings A-18; decisions D2):

1. The specification SHALL contain `docs/spec/appendix_h_library.md`, titled `Appendix H. Inherited library` and marked normative, with one table for each group: integer types, float types, `bool`, `Option`, arrays and slices, ranges, iterator adaptors (`map`, `filter`, `fold`, `sum`, `count`, `zip`, `enumerate`, `rev`, `any`, `all`), traits a model may name in a bound or a derive, and types.
2. WHEN a `rust` or `rust,fragment` fence of `docs/spec/*.md` calls a method (`.name(`) THEN the specification SHALL list `name` in Appendix H or among the carrier and value methods of B.1, or declare `fn name` in one of its fences.
3. The specification SHALL state in §0.4 and in Appendix H that the catalogue is closed: a method, trait or type of Rust's library that Appendix H does not list does not resolve (`E0161`).
4. The specification SHALL list Appendix H in the file map of `index.md` (§0.8) and in the `nav` of `mkdocs.yml`.
5. WHEN a bound, a `#[derive(..)]` or a type position names a trait or a type that Appendix H lists (`fn f<T: PartialOrd>(a: T) {}`, `#[derive(Default)] struct S;`) THEN the POM SHALL resolve the name to a built-in row and report no `E0161`.
6. The POM built-in table SHALL hold one row of kind `Trait` or `Type` for each trait and type Appendix H lists, and no `Trait` row other than those, `Repr`, `Nature` and `Fn`.

**Acceptance Criteria** (POL-09, The parser draws the Rust boundary where B.3.6 does; findings A-06, A-17, A-28; decisions D1):

1. WHEN a generic argument is written `Int< -1 >` or `Int<{ -1 }>` THEN the parser SHALL report no diagnostic, hold a const expression `-1` as the argument, and `print` SHALL give each spelling back.
2. IF a generic argument is written `Int<-1>` THEN the parser SHALL report a diagnostic at the `<-` token.
3. WHEN a type is written `fn(f64) -> f64`, `fn(&[T]) -> T` or `fn()` in a function parameter, a struct field, a `const` or a `let` annotation THEN the parser SHALL report no diagnostic, hold `TypeKind::FnPtr` with its parameter types and return type, and `print` SHALL give the text back.
4. WHEN a design is built from a source with a function-pointer type THEN the POM SHALL hold a `TypeKind::FnPtr` node and `to_ast()` SHALL equal the parsed file.
5. IF a source uses a form of the removed-forms table of B.3.6 THEN the parser SHALL report exactly one diagnostic for it, with the code of its row, whose message names the form and whose help names what to write instead (`type T = f64;` gives `E0903` "type aliases are not part of the language"; `fn h(&self)` gives `E0900` with help "write `self`").
6. The removed-form fixtures of `msl_errors` SHALL hold one entry per row of the removed-forms table of B.3.6, matched by the row's first cell.
7. IF a module-body `let` carries a type annotation (`let y: f64 = 1.0;`) THEN the parser SHALL report one `E0900` whose message says that a body `let` takes no type annotation.

**Independent Test**: parse `type T = f64;` and read "type aliases are not part of the language"; build `fn f<T: PartialOrd>(a: T) {}` with no diagnostic.

---

### P1: The language's own names resolve ⭐ MVP

**User Story**: As a model author, I want the names the language itself provides, my imports and my patterns to mean what the specification says so that a valid model builds with no false error and a `match` is never silently rewritten.

**Why P1**: two of the three BLOCKERs (A-01, A-02) and the silent duplicate names (A-10).

**Acceptance Criteria** (POL-03, Core names are built in; findings A-01, A-42; decisions D3):

1. The specification SHALL call `gnd`, `Nature`, `m`, `dtemp` and `str` core names in §6 rule 11, A.1 rule 8 and §3.4.2, and SHALL NOT call `gnd` a prelude name.
2. WHEN a source holds `const S: &'static str = "x";` and no prelude is registered THEN the POM SHALL resolve `str` to a built-in of kind `Type` and report no diagnostic.
3. WHEN a module declares `<N: Nature>` and a port `g: N` and no prelude is registered THEN the POM SHALL resolve `Nature` to a built-in of kind `Trait`, classify the port as `PortKind::Carrier` and report no diagnostic.
4. WHEN `gnd` is read in a module body, in an `analog` block or in a reaction THEN the POM SHALL resolve it to a built-in of kind `Node`.
5. WHEN `m` or `dtemp` is read in a module body or in a behavior block bound to a module THEN the POM SHALL resolve it to a built-in of kind `ImplicitParam`.
6. IF `m` or `dtemp` is read in a `fn` or a `const` at namespace scope and no declaration of that name is in scope THEN the POM SHALL leave it `Unresolved` and report one `E0161` at the name.
7. WHEN a local `let m` or a closure parameter `m` is in scope inside a behavior block THEN the POM SHALL resolve `m` to that binding.
8. IF a namespace declares an item named `gnd`, `Nature` or `str` THEN the POM SHALL report one `E0457` at the item's name.
9. The test prelude `tests/fixtures/pom/prelude.msl` SHALL declare no item named `Nature`, `gnd`, `m`, `dtemp` or `str`.

**Acceptance Criteria** (POL-04, One name per table per namespace; findings A-10, A-40, A-46; decisions D5):

1. The specification SHALL define `E0162` in the error table of §3.4, phase elaboration, for: a name declared twice in one table (type or value) of one namespace; an explicit `use` whose name collides with an item or with another explicit `use` of the same namespace; and a name that is used while only two glob imports provide it, for different declarations.
2. The specification SHALL state in §3.4 that an item or an explicit `use` shadows a glob import without a diagnostic, that a hardware module and a unit struct occupy both tables, and that two functions of one name remain `E0143`.
3. IF a namespace declares two items of one name in one table (`module M() {} module M() {}`, `struct M; const M: f64 = 1.0;`, `mod a {} mod a {}`) THEN the POM SHALL report one `E0162` at the name of each declaration after the first, keep every declaration in `children()`, and resolve the name to the first.
4. IF an explicit `use a::K;` and an item `const K` (or a second explicit `use b::K;`) are written in one namespace THEN the POM SHALL report one `E0162` at the later of the two in source order.
5. WHEN two glob imports provide one name for different declarations and the name is never used THEN the POM SHALL report no diagnostic; IF the name is used THEN the POM SHALL report one `E0162` at the use, leave the slot `Unresolved` and report no `E0161` for it.
6. WHEN two glob imports provide one name for the same declaration THEN the POM SHALL resolve a use of the name to that declaration and report no diagnostic.
7. IF a namespace declares two `fn` items of one name THEN the design SHALL carry the parser's `E0143` for the second and no `E0162`.
8. WHEN a namespace declares a type and a value of one name in different tables (`struct S { a: f64 }` and `fn S() {}`) THEN the POM SHALL report no diagnostic.

**Acceptance Criteria** (POL-06, Part I value rules: borrows and identifier patterns; findings A-02, A-19, A-42; decisions D7, D10):

1. The specification SHALL say in §1 rule 2 and in the `E0102` row that a shared borrow of any value is allowed as a call argument, and that `E0102` covers every other `&`.
2. The specification SHALL state in Part I that a plain identifier pattern (no `mut`, `ref` or `@`) whose name resolves to a constant, a unit variant or a unit struct is a path to it and binds nothing.
3. WHEN a function holds `match o { None => 0, Some(x) => x }` THEN the POM SHALL give the pattern `None` the resolution `Builtin(None)`, answer `is_binding() == false` for its `Binding`, and enter no name `None` in the arm's scope.
4. WHEN a plain identifier pattern names a `const` (`ZERO`), a unit variant in scope (`use E::*;` then `A`), a unit struct (`U`) or a variant of `Analysis` (`Dc`) THEN the POM SHALL resolve the pattern to that declaration or built-in and list the pattern in the `references()` of the declaration.
5. WHEN an identifier pattern has `mut`, `ref` or `@`, or its name resolves to nothing, to a local, to a function or to a variant with fields THEN the POM SHALL keep it a binding (`is_binding() == true`) that later uses resolve to.

**Acceptance Criteria** (POL-17, Imports and members of built-in traits; findings A-08, A-09, A-43):

1. WHEN a file holds `mod a { pub const K: f64 = 1.0; } use a::{self, K}; const Z: f64 = a::K + K;` THEN the POM SHALL resolve `a::K` and `K` to the constant and report no diagnostic.
2. WHEN a file holds `use a::{self as aa, n::{self, J as JJ}};` THEN the POM SHALL resolve `aa::K`, `n::J` and `JJ` and report no diagnostic.
3. WHEN the path of a `use` leaf is read THEN `text()` SHALL be the leaf's own written segments, with no `{`, `}` or `,`, and `span()` SHALL cover exactly that text.
4. WHEN a function is written `fn cast<A: Repr, B: Repr>(a: A) -> B where A::WIDTH == B::WIDTH { B::decode(a.encode()).unwrap() }` THEN the POM SHALL resolve `A::WIDTH`, `B::WIDTH` and `B::decode` to built-in member rows of `Repr` and report no diagnostic.
5. IF an associated name is written through a generic bounded only by `Repr` and `Repr` has no such member (`A::NOPE`) THEN the POM SHALL report one `E0161` at the name.
6. WHEN an associated name is written through a generic bounded by a catalogue trait of Appendix H (`T: PartialOrd`, `T::foo`) THEN the POM SHALL answer `Deferred(BuiltinTraitMember)` and report no diagnostic.

**Independent Test**: build `module M<N: Nature>(a: N) { let r = R(a, gnd); }` with no prelude and `match o { None => 0, Some(x) => x }`, then `design.dump()`.

---

### P1: Statements and places sit where the grammar says ⭐ MVP

**User Story**: As a model author, I want a statement in the wrong place, or a commit to something that is not a carrier, to be an error at the statement so that no write goes nowhere.

**Why P1**: the parser accepts forms the grammar forbids (A-04, A-05): the POM then holds writers of nothing and signals of no module.

**Acceptance Criteria** (POL-05, Behavior blocks: generic names and where items sit; findings A-20, A-21; decisions D6, D13):

1. The specification SHALL say in §10 rule 6, in its Resolution and in B.3.3 item 14 that a behavior block repeats the generic names of its module, the same names in the same order, and the word `positionally` SHALL NOT occur in §10.
2. The specification SHALL say in B.2.1 note 3, §8 rule 1 and §3.4.3 that `nature`, `module`, `analog for` and `digital for` are items at namespace scope (a file or an inline `mod`), and that a block may sit in any namespace of its module's file.
3. IF the generic names of a behavior block differ from its module's generic parameters in name or in order (`digital<N, T> for M<N, T>` on `module M<T, const N: u32>`) THEN the POM SHALL report `E0403` and resolve each name of the block to the module's generic parameter of the same name, or leave it `Unresolved` when the module has none; it SHALL never link by position.
4. WHEN a module is declared in `mod a` of a file and a behavior block for it is written at the root of that file as `a::M`, or in a sibling `mod b` as `super::a::M` THEN the POM SHALL bind the block to the module and report no diagnostic.

**Acceptance Criteria** (POL-07, Statement placement and places, as text; findings A-35, A-36, A-39; decisions D14, D17):

1. The specification SHALL say in the `E0404` row and in §10 rule 8 that `E0404` covers `<-`, `+>`, `bound_step`, `on` and `signal` in any block or body that is not theirs, a module body included, and SHALL say in both the `E0404` and the `E0121` rows that inside a `fn` body `E0121` is the code reported.
2. The specification SHALL state in B.3.4 that a `contribution` or a `commit` may be written in a nested block that is used as a value (`let x = { i(a) +> 1.0; 2.0 };`).
3. The specification SHALL state in B.3.3 that a `place` takes no parentheses, in a definition and in a commit alike.

**Acceptance Criteria** (POL-14, The parser rejects what the grammar forbids; findings A-04, A-05, A-22, A-24, A-29, A-42; decisions D14, D17):

1. IF the left side of a commit is not a `place` (`-q <- 1;`, `a + b <- 2;`, `f(q) <- 3;`, `q.x() <- 4;`, `*q <- 6;`, `q as u8 <- 7;`) THEN the parser SHALL report one `E0900` over the left side whose message starts with `expected a place` and names a carrier, `x[i]` and `x.f`; `q <- 1;`, `q[0] <- 1;`, `st.n <- 1;` and `a.b[2].c <- 1;` SHALL be clean.
2. IF a place is parenthesized (`(q) = 1;` in a module body, `(q) <- 1;` in a reaction) THEN the parser SHALL report one `E0900` with the same message in both forms.
3. IF `signal` is written in an `analog` block, in a reaction or in a closure outside a function THEN the parser SHALL report one `E0404` over the statement and no `E0121`.
4. IF `q <- 1;`, `i(a) +> 1.0;` or `on e { }` is written in a module body THEN the parser SHALL report exactly one diagnostic, `E0404` over the statement, with a help that says where the statement belongs.
5. IF `+>`, `<-`, `on`, `signal` or `bound_step` is written in a `fn` body, a closure inside it included THEN the parser SHALL report `E0121` over the statement and no `E0404`.
6. IF `pub` precedes `analog for` or `digital for` THEN the parser SHALL report one `E0900` at `pub` with a help saying a behavior block has no visibility, and SHALL still hold the block as a `Behavior` item.
7. IF a source holds `nature A { }`, `signal g: (Bit, Bit);` or `signal h: &Bit;` THEN the parser SHALL report one `E0900` for each.
8. IF a `signal` statement was reported `E0404` or `E0121` THEN the POM SHALL enter its name in no scope: a later use of the name SHALL be `Unresolved` with `E0161`, and no module SHALL list it in `signals()`.

**Independent Test**: parse `digital for M { on e { f(q) <- 3; signal t: E; } }` and read one `E0900` and one `E0404`.

---

### P2: The surface the decisions add

**User Story**: As a model author, I want raw identifiers, the field shorthand, doc comments on every statement and literals checked for range so that everyday models read naturally and keep their authored form.

**Why P2**: decided now (D9, D12, D15, D16) because the parser already half-accepts them; none blocks the three P1 stories.

**Acceptance Criteria** (POL-08, Docs, check phases, raw identifiers and shorthand, as text; findings A-34, A-38; decisions D9, D11, D15, D16):

1. The specification SHALL allow `attrs` in A.4 rule 1 and in the productions of B.3.2 before every statement, `reaction`, definition, `gen<X>`, `nature_field`, `contribution` and `commit`.
2. The specification SHALL say in A.4 and §21 that the doc comments of a quantity and of a reaction are metadata, and that those of a statement are kept in the tree and have no other effect.
3. The specification SHALL name in B.3.3 the phase of each constraint 8 to 14: item 8 at parse (`E0142`), the others at elaboration during name resolution.
4. The specification SHALL extend the `E0125` row and §3 rule 2 to an `extern` function with a body and to a function without a body that is neither `extern` nor a trait item.
5. The specification SHALL say in A.1 rule 6 that `r#name` is an identifier for each of the seven added keywords and for no other word, and that metadata lists the name without `r#`; Appendix G SHALL mark G.15 Closed citing A.1.
6. The specification SHALL write the fields of `instance` and `template` in B.3.2 as `IDENT ( ':' expr )?` and say that `{ r }` means `{ r: r }`; Appendix G SHALL mark G.14 Closed citing B.3.
7. The specification SHALL define every error code it mentions in exactly one error-table row whose first cell is that code.
8. The specification SHALL keep the count of B.1 at 65 constructs, with 65 rows in B.1.2.
9. The specification SHALL list in Appendix G, under `Closed in this specification`, one row for each decision D1 to D17 except D4, naming the deciding section, and SHALL keep G.38 Open.

**Acceptance Criteria** (POL-10, Raw identifiers for the added keywords; findings A-28; decisions D15):

1. WHEN `r#module`, `r#nature`, `r#signal`, `r#on`, `r#out`, `r#flip` or `r#after` is written where an identifier is expected THEN the lexer SHALL yield an identifier whose name is the word without `r#` and whose `raw` flag is true, and `signal r#out: Electrical;` followed by a read of `r#out` SHALL parse with no diagnostic.
2. IF `r#` precedes any other word (`r#type`, `r#in`, `r#foo`) THEN the parser SHALL report one `E0900` whose message lists the seven words raw identifiers exist for.
3. WHEN a file that uses `r#out` is printed THEN the printer SHALL write `r#out` and the reparsed tree SHALL equal the first.
4. WHEN a module declares `signal r#out: Bit;` and reads `r#out` THEN the POM SHALL report `name() == Some("out")` for the signal, resolve the read to it, and `to_ast()` SHALL equal the parsed file.

**Acceptance Criteria** (POL-11, Field-init shorthand keeps its form; findings A-23; decisions D16):

1. WHEN a param block or a template is written with a shorthand field (`R(p, n) { r }`, `R { r, c: 1p }`) THEN the parser SHALL report no diagnostic and hold the field `r` with `shorthand == true` and the path `r` as its value; a field written `r: r` SHALL hold `shorthand == false`.
2. WHEN such a file is printed THEN the printer SHALL write `{ r }` for the shorthand field and `{ r: r }` for the long one.
3. WHEN a design is built from `let x = R(a, b) { r };` in a module that declares `r` THEN the POM SHALL expose the field with `is_shorthand() == true`, bind it to the param `r` of `R`, resolve its value to the `r` in scope, and `to_ast()` SHALL equal the parsed file.

**Acceptance Criteria** (POL-12, Doc comments and attributes wherever D9 allows them; findings A-07; decisions D9):

1. WHEN a doc comment or an attribute precedes a `let`, an expression statement, a contribution or a commit inside a `fn` body, an `analog` block or a reaction THEN the parser SHALL report no diagnostic and hold them, in order, in the metadata of that statement.
2. IF a doc comment is followed by `}` or by the end of input THEN the parser SHALL report one `E0900` labelled at the comment whose message says the doc comment documents nothing.
3. WHEN a doc comment or an attribute precedes a reaction, a definition, a generation `for` or `if` (in a module body or in a `digital` block) or a nature field THEN the parser SHALL report no diagnostic and hold them, in order, in the metadata of that node.
4. WHEN a file with docs in every position of the two criteria above is printed THEN the reparsed tree SHALL equal the first and a second print SHALL equal the first print.
5. WHEN a design is built from that file THEN `to_ast()` SHALL equal the parsed file.
6. WHEN docs precede a reaction, a nature quantity, a definition, a generation or a statement THEN the POM SHALL return their text, one entry per comment in order, from `Reaction::docs()`, `Quantity::docs()`, `Definition::docs()`, `Generation::docs()` and `Stmt::docs()`.

**Acceptance Criteria** (POL-13, Literals: range, escapes, raw strings, serialization; findings A-11, A-27, A-37, A-42; decisions D12):

1. The specification SHALL define `E0909` in A.6, phase parse, for a float literal whose nearest `f64` is infinite and for an integer literal that exceeds its suffix type or, without a suffix, 128 bits, and SHALL state the rule in A.2 and §1 rule 7.
2. IF a float literal, with an exponent or an SI suffix, has an infinite nearest `f64` (`1e999`, a 310-digit number followed by `T`) THEN the lexer SHALL report one `E0909` at the literal; `1.7976931348623157e308` SHALL be clean.
3. IF an integer literal exceeds `2^N - 1` with a suffix `uN`, `2^(N-1)` with a suffix `iN`, or `2^128 - 1` without a suffix (`256u8`, `2u1`, `129i8`, `340282366920938463463374607431768211456`) THEN the lexer SHALL report one `E0909` at the literal; `255u8`, `128i8` and `340282366920938463463374607431768211455` SHALL be clean.
4. The parse-phase fixture table of `msl_errors` SHALL hold a fixture for `E0909`.
5. IF a string literal holds an escape that Rust does not define (`"\q"`, `"\x9"`, `"\u{110000}"`) THEN the lexer SHALL report one `E0900` at the escape.
6. WHEN a raw string `r#"a"b\c"#` is parsed and printed THEN the printer SHALL write `r#"a"b\c"#`.
7. The literal type SHALL expose `str_value()` (escapes processed; a raw string verbatim), `int_value()` (the `u128` of a decimal, hex, octal or binary literal, underscores and suffix removed) and `suffix()`.
8. WHEN a design whose source holds `1e999`, `1e308` and `5e-324` is serialized with `serde_json` and read back THEN the read design SHALL have the same `dump()` and no step SHALL fail.
9. The serialized form of a float literal SHALL hold its source text and no numeric value.

**Independent Test**: print `module M(a: E) { signal r#out: E; /// drive\n r#out = a; let x = R(a, gnd) { r }; }` and get the same text back.

---

### P2: Limits and recovery

**User Story**: As a user of generated code and of an editor, I want long flat expressions to parse and one syntax error to cost one diagnostic so that the tool stays usable on real input.

**Why P2**: generated sums hit `E0910` today and one bad port loses a whole module (A-25, A-26).

**Acceptance Criteria** (POL-15, `E0910` counts nesting; flat chains have their own bound; findings A-25; decisions D8):

1. The specification SHALL define `E0910` in A.6 as nesting deeper than 256 levels (delimiters, blocks, prefix operators, closures, `return` and `break` operands, type arguments), or more than 4096 chain links (binary operators, postfix `.`, call, index, `as` and `?`, `else if` arms) on one path of an expression.
2. WHEN a source holds a sum of 4096 terms, 4096 chained method calls, 4096 `as` casts or an `if` with 4096 `else if` arms THEN the parser SHALL report no diagnostic, and `print`, `Design::build` and `to_ast()` SHALL complete on an 8 MiB stack in a debug build.
3. IF a flat chain has more than 4096 links (4097 and 20000 terms) THEN the parser SHALL report exactly one `E0910` whose message names the 4096-link bound, and SHALL NOT overflow the stack.
4. WHILE a source nests prefix operators, closures, `return` operands or generic arguments deeper than 256 levels the parser SHALL report `E0910` and SHALL NOT overflow an 8 MiB stack.

**Acceptance Criteria** (POL-16, Recovery keeps the item and loses no error; findings A-26):

1. IF a module head is written `module M<const N: u32>() where N > 0, N < 5 { }` THEN the parser SHALL report exactly one diagnostic, labelled at the `,` of the `where`, and SHALL hold `M` as a `Module` item with its generic `N`.
2. IF a function is written `fn f() { let a = ; foo(; let c = 3; }` THEN the parser SHALL report a diagnostic at the first `;`, SHALL label no diagnostic on `let c = 3;`, and SHALL hold exactly one item, the function `f`, whose body holds the statement `let c = 3;`.
3. IF a module is written `module M(in a: , b: Bit) { signal s: Bit; }` THEN the parser SHALL report exactly one diagnostic, at the `,`, and SHALL hold `M` as a `Module` item with the port `b` and the signal `s`.
4. WHEN a design is built from that module and from another module that places `M(x, y)` THEN the POM SHALL return the module from `design.module("M")`, classify the placement as an instance of it, and report no `E0161` and no `E0401`.

**Independent Test**: parse a 1000-term sum with no diagnostic; parse `module M(in a: , b: Bit) {}` and still find `M`.

---

### P2: The POM reports the spec's codes and never guesses

**User Story**: As a tool author, I want the POM's diagnostics to carry the codes the specification assigns and its links to be right or absent so that a later stage never re-reports or trusts a wrong link.

**Why P2**: `E0161` stands today for four other spec errors, and two classifications link to the wrong declaration (A-14, A-15, A-16).

**Acceptance Criteria** (POL-18, The POM reports structural errors under the spec's codes; findings A-14, A-29, A-34, A-38, A-45; decisions D11):

1. IF the param block of an instance or the field list of a template names no param of the module, or names one twice (`R(a, b) { zz: 1.0 }`, `R { r: 1.0, r: 2.0 }`) THEN the POM SHALL report one `E0320` at the name and no `E0161` for it.
2. IF a named argument of a call or of an instance names no parameter whose declared type is written `Option<..>`, or one an earlier argument bound (`g(1.0, d: 3.0)`, `R(a, b, q: b)`, `g(1.0, k: 1, k: 2)`) THEN the POM SHALL report one `E0140` at the name and no `E0161` for it.
3. IF a bundle literal names a field the bundle lacks or names one twice THEN the POM SHALL report one `E0263` at the name and no `E0161` for it.
4. IF a struct literal (`S { a: 1.0, c: 2.0 }`) or a struct pattern (`S { a, q }`) names a field the struct lacks THEN the POM SHALL report one `E0161` at the name, in both.
5. IF an `extern` module has a body item other than a param, or a result (`-> type`), or a value input THEN the POM SHALL report one `E0370` for each, at the item, at the result type or at the port.
6. IF an `extern "C" fn` has a body, or a function that is neither `extern` nor a trait item has none THEN the POM SHALL report one `E0125` at the function's name.
7. IF a nature repeats a field, mixes discrete fields (`value`, `resolve`) with continuous ones (`potential`, `flow`), or has neither `value` nor `potential` THEN the POM SHALL report one `E0201` at the nature's name and answer `kind() == NatureKind::Invalid`.

**Acceptance Criteria** (POL-19, Classification never guesses; findings A-15, A-16, A-30; decisions D11):

1. IF the first argument of an access call has a declared nature that declares no access of that name (`i(t)` with `t: Th`, where `Th` declares `v` and `pwr`) THEN the POM SHALL leave the callee `Unresolved`, not classify the call as `Probe`, and report one `E0206` at the callee.
2. IF a method is called on a carrier whose declared type is an array (`arr.rise()` with `signal arr: [Bit; 2];`) and the name is a method of the element nature's `impl` or a carrier method of B.1 THEN the POM SHALL leave the method `Unresolved` and report one `E0211`; `arr[0].rise()` SHALL resolve to the `impl` function.
3. WHEN a parent module writes `c.w = true;` or `c.w <- true;`, where `c` is an instance of `C` and `w` a `pub signal` of `C` THEN the POM SHALL list that definition or commit in the `writers()` of `C.w`.

**Independent Test**: build `R(a, b) { zz: 1.0 }` and read `E0320`; build `i(t)` on a thermal port and read `E0206`.

---

### P2: Reflection is complete and robust

**User Story**: As a tool author, I want to walk a body, list every item kind and read a design back from JSON without matching raw storage or risking a panic.

**Why P2**: every consumer otherwise re-implements the recursion over `ExprKind` (A-13) and a corrupted JSON panics (A-33).

**Acceptance Criteria** (POL-20, Reflection walks everything by handle; findings A-12, A-13, A-31, A-44):

1. WHEN a prelude source and a project file both declare an item of one name (`nature Bit`, `module Buf`, `struct S`, `fn f`) THEN `Design::nature`, `module`, `struct_named`, `function` and `find` SHALL each return the project's declaration.
2. The collections `modules()`, `natures()`, `structs()`, `enums()`, `traits()`, `consts()`, `impls()`, `functions()` and `namespaces()` SHALL list the declarations of project files only, in source order, and `Design::prelude_files()` SHALL return the prelude's `SourceFile`s.
3. The `Design` SHALL offer `enums()`, `traits()`, `consts()`, `impls()`, `struct_named(name)` and `function(name)`, each returning typed handles.
4. WHEN `Expr::children()` is called on a node of any `ExprKind` THEN it SHALL return that node's direct sub-expressions in source order, and `Expr::parent()` of each SHALL be that node; `parent()` of an expression that no expression contains SHALL be none.
5. WHEN `Decl::exprs()` is called on a function, a module, a behavior block or a reaction THEN it SHALL return every expression of that declaration's own body in pre-order source order, and none that belongs to a nested item.
6. The POM SHALL offer a `Block` handle (`stmts()`, `value()`), `Stmt::children()` (the statement's direct expressions) and `Expr::stmt()` (the nearest enclosing statement), and `Function::body()` and `Reaction::body()` SHALL return the handle.
7. The POM SHALL offer handles for a call argument and for a field initializer with `name()`, `value()` and `target()` (the port, param, parameter or field it binds; none when unknown), reached from `Expr::args()`, `Expr::fields()` and `Instance::param_bindings()`.
8. WHEN an instance places a core module (`cross(x)`, `ddt(x)`) THEN `Instance::builtin()` SHALL return its `BuiltinId`; for a user module it SHALL return none.
9. WHEN a struct, an enum or a trait has `impl` blocks THEN `Struct::impls()`, `Enum::impls()` and `Trait::impls()` (its implementors) SHALL list them in source order, and `Struct::method(name)` and `Enum::method(name)` SHALL return the function of that name.
10. WHEN `Generation::items()` is called on a generation `if` THEN it SHALL return the items of every arm in source order; `arms()` SHALL be unchanged.

**Acceptance Criteria** (POL-21, A design read back is validated; name tables scale; findings A-32, A-33):

1. WHEN `Design::from_json(text)` reads the JSON of a design THEN it SHALL return `Ok` with a design whose `dump()` equals the original's.
2. IF the JSON holds an id out of range or a link `check()` rejects THEN `Design::from_json` SHALL return `Err` listing the problems of `check()` and SHALL NOT panic; text that is not a design SHALL return `Err` as well.
3. WHEN one module declares 8000 signals THEN `Design::build` SHALL take less than 8 times as long as for 2000 signals (best of three runs each).
4. WHEN `Design::module(name)` and `Module::signal(name)` are called 20000 times THEN the time at 4000 modules (or signals) SHALL be less than 4 times the time at 500 (best of three runs each).

**Independent Test**: `design.function("f").unwrap().exprs()` lists every call of `f` in source order.

---

### P3: Reference docs and guards

**User Story**: As a reader of `docs/pom`, I want every construct of B.1 and every code the POM reports to have a row so that the reference is complete by test.

**Why P3**: documentation debt (A-41); nothing depends on it.

**Acceptance Criteria** (POL-22, `docs/pom` maps every construct and every code; findings A-39, A-41; decisions D17):

1. The page `docs/pom/mapping.md` SHALL hold a table with one row for each construct 1 to 65 of B.1.2, each row naming a POM type or method in code font.
2. The pages `docs/pom/mapping.md` and `docs/pom/reflection.md` SHALL NOT list a parenthesized expression as a `place`.
3. The Diagnostics table of `docs/pom/resolution.md` SHALL hold one row for each error code a `Diagnostic::new` of `src/pom` reports, with the spec section that defines it.
4. The built-in table of `docs/pom/resolution.md` SHALL name every row of `BUILTINS`.
5. The page `docs/pom/reflection.md` SHALL document every public reflection method this feature adds, under the heading of its type.

**Acceptance Criteria** (POL-23, The corpus stays green; findings A-42):

1. WHILE the specification carries this feature's edits, every `rust` fence of `docs/spec/*.md` SHALL parse with no diagnostic and every `rust,fragment` fence SHALL parse in one of its contexts.
2. WHILE the specification carries this feature's edits, every file-parsable fence SHALL convert back from the POM to its parsed tree.

**Independent Test**: `cargo test -p piperine-lang --test pom_docs`.

---

## Edge Cases

Each is covered by the criterion named.

- IF `r#` precedes a Rust keyword (`r#type`) THEN the parser SHALL report `E0900`, not accept a raw identifier (POL-10 AC 2).
- WHEN an integer literal is exactly the magnitude of its signed type's minimum (`128i8`) THEN the lexer SHALL accept it (POL-13 AC 3).
- WHEN two glob imports reach the same declaration THEN the POM SHALL report nothing (POL-04 AC 6).
- IF a commit target is rejected THEN the parser SHALL still keep the commit node, so one fault is one diagnostic (POL-14 AC 1).
- WHEN a local named `m` is in scope THEN the POM SHALL resolve `m` to the local, not to the implicit param (POL-03 AC 7).
- IF a chain of 4097 links sits inside 10 nested parentheses THEN the parser SHALL report `E0910` for the chain: links are summed along one path (POL-15 AC 3).
- WHEN an identifier pattern names a local variable THEN the POM SHALL keep it a binding that shadows the local (POL-06 AC 5).
- IF a design's JSON is not JSON at all THEN `Design::from_json` SHALL return `Err` (POL-21 AC 2).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| POL-01 | P1: The Rust boundary is closed | Tasks | In Tasks |
| POL-02 | P1: The Rust boundary is closed | Tasks | In Tasks |
| POL-03 | P1: The language's own names resolve | Tasks | In Tasks |
| POL-04 | P1: The language's own names resolve | Tasks | In Tasks |
| POL-05 | P1: Statements and places sit where the grammar says | Tasks | In Tasks |
| POL-06 | P1: The language's own names resolve | Tasks | In Tasks |
| POL-07 | P1: Statements and places sit where the grammar says | Tasks | In Tasks |
| POL-08 | P2: The surface the decisions add | Tasks | In Tasks |
| POL-09 | P1: The Rust boundary is closed | Tasks | In Tasks |
| POL-10 | P2: The surface the decisions add | Tasks | In Tasks |
| POL-11 | P2: The surface the decisions add | Tasks | In Tasks |
| POL-12 | P2: The surface the decisions add | Tasks | In Tasks |
| POL-13 | P2: The surface the decisions add | Tasks | In Tasks |
| POL-14 | P1: Statements and places sit where the grammar says | Tasks | In Tasks |
| POL-15 | P2: Limits and recovery | Tasks | In Tasks |
| POL-16 | P2: Limits and recovery | Tasks | In Tasks |
| POL-17 | P1: The language's own names resolve | Tasks | In Tasks |
| POL-18 | P2: The POM reports the spec's codes and never guesses | Tasks | In Tasks |
| POL-19 | P2: The POM reports the spec's codes and never guesses | Tasks | In Tasks |
| POL-20 | P2: Reflection is complete and robust | Tasks | In Tasks |
| POL-21 | P2: Reflection is complete and robust | Tasks | In Tasks |
| POL-22 | P3: Reference docs and guards | Tasks | In Tasks |
| POL-23 | P3: Reference docs and guards | Tasks | In Tasks |

**Coverage:** 23 requirements, 129 acceptance criteria, 23 mapped to tasks, 0 unmapped.

### Requirements, their criteria and their tasks

| Requirement | What | Criteria | Tasks |
| ----------- | ---- | -------- | ----- |
| POL-01 | B.3.6 is the complete list of what is removed from Rust | 3 | T1 |
| POL-02 | The inherited library is a closed catalogue | 6 | T2, T28 |
| POL-03 | Core names are built in | 9 | T3, T27 |
| POL-04 | One name per table per namespace | 8 | T4, T31 |
| POL-05 | Behavior blocks: generic names and where items sit | 4 | T6, T33 |
| POL-06 | Part I value rules: borrows and identifier patterns | 5 | T5, T32 |
| POL-07 | Statement placement and places, as text | 3 | T7 |
| POL-08 | Docs, check phases, raw identifiers and shorthand, as text | 9 | T1, T8, T9, T10, T49 |
| POL-09 | The parser draws the Rust boundary where B.3.6 does | 7 | T11, T12, T22 |
| POL-10 | Raw identifiers for the added keywords | 4 | T13 |
| POL-11 | Field-init shorthand keeps its form | 3 | T14 |
| POL-12 | Doc comments and attributes wherever D9 allows them | 6 | T15, T16, T44 |
| POL-13 | Literals: range, escapes, raw strings, serialization | 9 | T17, T18, T45 |
| POL-14 | The parser rejects what the grammar forbids | 8 | T19, T20, T21, T33 |
| POL-15 | `E0910` counts nesting; flat chains have their own bound | 4 | T23 |
| POL-16 | Recovery keeps the item and loses no error | 4 | T24, T25, T26 |
| POL-17 | Imports and members of built-in traits | 6 | T29, T30 |
| POL-18 | The POM reports structural errors under the spec's codes | 7 | T34, T35, T36 |
| POL-19 | Classification never guesses | 3 | T37, T38, T39 |
| POL-20 | Reflection walks everything by handle | 10 | T40, T41, T42, T43, T44 |
| POL-21 | A design read back is validated; name tables scale | 4 | T45, T46 |
| POL-22 | `docs/pom` maps every construct and every code | 5 | T47, T48 |
| POL-23 | The corpus stays green | 2 | T49 |

### Audit finding to requirement to task

Every finding of `.specs/features/msl-audit/report.md`. A-03 is the one finding deferred whole; A-42 is deferred in the part that needs the standard library.

| Finding | What | Requirement | Tasks |
| ------- | ---- | ----------- | ----- |
| A-01 | core names of B.1 in no table | POL-03 | T3, T27 |
| A-02 | identifier pattern naming a constant becomes a binding | POL-06 | T5, T32 |
| A-03 | no compilable standard library | deferred to `msl-stdlib` (D4, Out of Scope) | - |
| A-04 | a commit takes any expression as its target | POL-14 | T19 |
| A-05 | `signal` accepted in `analog` and in a reaction | POL-14 | T20, T33 |
| A-06 | `Int< -1 >` does not parse | POL-09 | T11 |
| A-07 | docs and attributes before statements and hardware items | POL-12 | T8, T15, T16, T44 |
| A-08 | `use a::{self, K}` not resolved | POL-17 | T29 |
| A-09 | associated item through a built-in bound | POL-17 | T30 |
| A-10 | two items of one name: no rule, no code | POL-04 | T4, T31 |
| A-11 | overflowing float literal breaks the JSON round trip | POL-13 | T17, T45 |
| A-12 | `Design::nature` and the collections disagree about the prelude | POL-20 | T40 |
| A-13 | no way to walk a body by handle | POL-20 | T41, T42, T43 |
| A-14 | `E0161` where the spec assigns other codes | POL-18 | T34, T35 |
| A-15 | access call resolves to another nature's quantity | POL-19 | T37 |
| A-16 | method on an array of carriers resolves to the element's `impl` | POL-19 | T38 |
| A-17 | `everything else is Rust` wider than the grammar | POL-01, POL-09 | T1, T12, T22 |
| A-18 | inherited library unbounded | POL-02 | T2, T28 |
| A-19 | `contains(&n)` contradicts §1 rule 2 | POL-06 | T5 |
| A-20 | may a block rename the module's generics | POL-05 | T6, T33 |
| A-21 | where a module, a nature and a block may be written | POL-05 | T6, T33 |
| A-22 | `pub analog for` parses | POL-14 | T21 |
| A-23 | `{ r }` shorthand accepted and rewritten | POL-11 | T10, T14 |
| A-24 | commit or contribution in a module body: wrong code | POL-14 | T20 |
| A-25 | `E0910` fires on flat chains | POL-15 | T23 |
| A-26 | recovery loses items and errors | POL-16 | T24, T25, T26 |
| A-27 | escapes unchecked, raw strings cooked, huge integers clean | POL-13 | T18 |
| A-28 | misleading messages (`static`, `r#out`, body `let` annotation) | POL-01, POL-09, POL-10 | T1, T13, T22 |
| A-29 | leniency: empty nature, tuple and reference signal types, fn bodies | POL-14, POL-18 | T21, T36 |
| A-30 | a parent's drive is not a writer of the child's carrier | POL-19 | T39 |
| A-31 | `Generation::items()` returns the first arm only | POL-20 | T44 |
| A-32 | quadratic member tables, linear lookups | POL-21 | T46 |
| A-33 | a design read from JSON is not validated | POL-21 | T45 |
| A-34 | no rule for an `extern` fn with a body or a plain fn without one | POL-08, POL-18 | T9, T36 |
| A-35 | contribution or commit inside a value block | POL-07 | T7 |
| A-36 | `E0121` versus `E0404` precedence | POL-07 | T7, T20 |
| A-37 | out-of-range literals have no rule | POL-13 | T17 |
| A-38 | phase of the extern-module check | POL-08, POL-18 | T9, T36 |
| A-39 | `(s)` as a place: docs and grammar disagree | POL-07, POL-22 | T7, T19, T48 |
| A-40 | ambiguous globs and colliding imports undefined | POL-04 | T4, T31 |
| A-41 | mapping.md does not map the 65 constructs | POL-22 | T47 |
| A-42 | test gaps: stand-ins, commit place, serde literal, identifier pattern | POL-03, POL-06, POL-13, POL-14, POL-23 (stand-ins `State`, `Logic`, `init` and the gallery-clean test: `msl-stdlib`) | T19, T27, T32, T45 |
| A-43 | `Path::text()` of a use leaf | POL-17 | T29 |
| A-44 | no `Instance::builtin()` | POL-20 | T43 |
| A-45 | unknown field in a struct pattern is silent | POL-18 | T35 |
| A-46 | `E0143` is the only duplicate-name check | POL-04 | T4, T31 |

### Decision to requirement

| Decision | Requirement |
| -------- | ----------- |
| D1 | POL-01, POL-09 |
| D2 | POL-02 |
| D3 | POL-03 |
| D4 | out of scope: `msl-stdlib` |
| D5 | POL-04 |
| D6 | POL-05 |
| D7 | POL-06 |
| D8 | POL-15 |
| D9 | POL-08, POL-12 |
| D10 | POL-06 |
| D11 | POL-08, POL-18, POL-19 |
| D12 | POL-13 |
| D13 | POL-05 |
| D14 | POL-07, POL-14 |
| D15 | POL-08, POL-10 |
| D16 | POL-08, POL-11 |
| D17 | POL-07, POL-14, POL-22 |

---

## Sources

- `.specs/features/msl-audit/report.md` - the 46 findings and the table of decisions D1 to D17
- The user's answer of 2026-11-14 ("sim") - accepts the seventeen recommendations as written; D4 becomes the next feature `msl-stdlib`
- `docs/spec/` - the specification this feature amends (Appendix A, Appendix B, Parts I to IV, Part VI, Appendix G)
- `.specs/features/msl-spec/STYLE.md` - how `docs/spec/` is written (numbered rules, `Resolution` notes, error tables)
- `.specs/features/msl-parser/`, `.specs/features/msl-pom/` - the two features whose output is polished
- `.specs/STATE.md` MD-25, MD-54, MD-55 - the POM rules this feature conforms to; MD-56, MD-57 - its own decisions

---

## Success Criteria

- [ ] All 129 checks of `tasks.md` are green at `HEAD` (`check_proofs.py msl-polish`).
- [ ] `cargo test -p piperine-lang` reports 0 failed, with no test removed and no `#[ignore]`.
- [ ] The seven sources of A-01's evidence, the five of A-02's and the eight of A-04's each give the diagnostics this specification states and no other.
- [ ] The build line prints no warning.

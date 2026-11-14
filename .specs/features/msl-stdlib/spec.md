# MSL Standard Library Specification

## Problem Statement

Part VII of `docs/spec/` describes a standard library that does not exist as source. `crates/piperine-lang/std/` holds a root and an empty `prelude` (`msl-packages`, PKG-06), so every gallery model of Appendices D, E and F fails to resolve: 0 of 18 gallery fences build clean, with 250 `E0161`. The front end has only ever resolved names against a hand-written test fixture of stand-ins (`tests/fixtures/pom/prelude.msl`, audit findings A-03 and A-42).

Decision D4 (user, 2026-11-14: "sim") makes the library a normative source: `std/*.msl` ships, Part VII's fences are excerpts of it, every export is `pub`, and "the galleries build clean" is a gate. This feature delivers that, the things the library needs from the language and the POM to be writable at all, and a chapter of The MSL Book that tells a reader what is core and what is `std`.

## Goals

- [ ] `crates/piperine-lang/std/*.msl` holds every item of Part VII and the prelude items of §1, §4 and §11, and builds with zero diagnostics.
- [ ] Every item MSL can express has an MSL body. Five functions, the ones that read the characters of a string, are declared `extern "intrinsic"`, and a test holds the list to exactly those five.
- [ ] Every `rust` fence of Part VII is, character for character, a block of a `std` file, and every item of `std` is shown in a `rust` fence of `docs/spec/`. Two tests hold both directions.
- [ ] All 18 gallery fences of Appendices D, E and F build clean against the embedded library, with no stand-in.
- [ ] The analysis kind is `Transient` everywhere; `Tran` is the pass switch of §22.3.7 and nothing else.
- [ ] The built-in table holds core names only: the six prelude constants leave it, and no `std` export shares a name with a core row.
- [ ] The MSL Book's source is in `docs/book/`, has a chapter "Core and standard library", and a test ties that chapter to the built-in table and to `std`.
- [ ] `cargo test -p piperine-lang` is green at every commit.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Type checking, elaboration, simulation of the library | no such phase exists; the front end stops at parse and POM resolution. "Clean" is defined on those two (STD-14). The bodies this feature writes are parsed and resolved, never run |
| Binding an intrinsic to an engine implementation | there is no elaborator or code generator for MSL yet. This feature fixes the list of names (STD-08 AC 3) |
| `extern "intrinsic"` on a module | Q1's answer allows it; after Q2 no module of `std` needs it, so the form is not specified or implemented now (Assumptions) |
| `str` methods in Appendix H, character or byte literals | they would let the five intrinsics be written in MSL; a language decision of its own |
| A device library (`Resistor`, `Capacitor`, `Switch`, SPICE models) in `std` | they are not Part VII items; the galleries take them from §8, §11, §12, §13. They are supplied to the gate as checked excerpts (STD-13) |
| The fences of Parts I to VI built clean against `std` | most are pieces that need context; the gate D4 names is the galleries |
| An improper `laplace_nd` (`N > D`) | the body written here needs `N <= D`; a general form differentiates the input more than once (Assumptions) |
| `piperine-project`, the CLI, the LSP, the Python binding | `std` is embedded in `piperine-lang`; no host changes |
| Publishing The MSL Book (the artifact `https://claude.ai/artifact/6nvqf1rKtF3TteqjEv6z26`) | done by the orchestrator after the merge, from `docs/book/`; no task publishes. This feature delivers the source and its checks (STD-18) |
| Rewriting the Book's existing chapters | only the new chapter, the packages note of chapter 9 and the cheat sheet change; an existing block is edited only where STD-18 AC 9 or AC 13 finds it wrong |
| `git push`, merge to `main` | needs the user's go-ahead for that action |

---

## Assumptions & Open Questions

D4, D3, U2 and Q1 to Q5 are the user's decisions (`context.md`, "Answers"). The other rows are the planner's choices inside them; each is open to correction and is listed in the planner's hand-off.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D4 The library is a normative source | `std/*.msl` ships; Part VII's fences are excerpts; `pub` on exports; the galleries build clean | user decision | y - user: "sim" (audit D4, MD-56) |
| D3 Core names | `gnd`, `Nature`, `m`, `dtemp`, `str` and the §14 primitives stay in the built-in table and out of `std` | user decision | y - user: "sim" (MD-56) |
| U2 Where `std` lives | embedded in `piperine-lang`, `include_str!` of `crates/piperine-lang/std/*.msl` | user decision | y - user: "std fica embutida o binário." |
| Q1 An item the engine implements | `extern "intrinsic"`, legal only in the crate `std`; no grammar change, no new code | user decision | y - user: "extern \"intrinsic\" (Recommended)" |
| Q2 Bodies now or intrinsic | every item MSL can express today gets an MSL body with Part VII's semantics, though nothing runs it yet; only what MSL cannot express is `extern "intrinsic"` | user decision | y - user: "As many as possible" |
| Q3 `Tran` names a module and an analysis kind | the module keeps `Tran`; the analysis kind is renamed; a guard keeps every `std` export off the core names | user decision | y - user: "Rename the analysis kind" |
| Q4 Public namespaces | `std::{constants, carriers, values, analog, nets, convert, monitor, io}`, one file each, all re-exported by `std::prelude` | user decision | y - user: "Yes, both (Recommended)" |
| Q5 Prelude constants | `Q`, `K_B`, `EPS0`, `PI`, `E`, `inf` become `pub const` items of `std::constants`; `inf` is `1.0 / 0.0` | user decision | y - user: "Yes, both (Recommended)" |
| The new name of the analysis kind | `Transient` | a whole word, as `Noise` and `Static` are; the host's analysis is already called "transient" in Part V and Part VIII | y - planner default inside Q3 |
| The intrinsic set | `matches_bits`, `logic`, `table_model`, `parse_table`, `parse_hex`: the five functions that read the characters of a `str` | MSL has no operation on the content of a string: Appendix H lists none, and character and byte literals are not tokens (`E0905`). Everything else is written in MSL (`design.md`, "The split") | y - planner default inside Q2 |
| `extern "intrinsic"` on a module | not specified now | Q1 allows it, but after Q2 all seven "head and params only" modules have bodies; a form no source uses is dead surface | y - planner default; reported to the user |
| Errors of the root forms | `E0707` is removed; a complex root without its conjugate and a zero denominator are a false `where` of a param, `E0317` | an MSL body has no way to raise a code of its own; a `where` on the param is the language's form for it | y - planner default inside Q2; reported to the user |
| `laplace_nd` | requires `D >= 1` and `N <= D` (`E0304`); one `State` per pole, controllable canonical form | Part VII fixes the transfer function and the number of states, not the form; `N > D` needs a second derivative of the input | n - follows from Q2; reported to the user as a new question |
| The generator of `normal` | SplitMix64 for two uniform draws, Box-Muller for the sample; the source is its documentation | §22.2.17 leaves the generator implementation-defined and requires it documented; a body in `std` documents it (closes the `normal` half of G.64) | y - planner default inside Q2; reported to the user |
| `pnjlim`, `fetlim`, `limvds` | transliterated from `crates/piperine-ir/src/limit.rs` | the in-tree functions are the ngspice ones, already checked against ngspice | y - planner default inside Q2 |
| What "clean" means | zero diagnostics in every file of the design, `Design::check()` empty, no `Unresolved` member slot, and each `std` name the gallery uses linked to a declaration of the crate `std` | only parse and POM resolution exist; a silent `Unresolved` is the one way a POM build hides a miss | y - planner default |
| Models the galleries take from other parts | a test fixture, one file per model, each checked to be text of `docs/spec/`; a table says which gallery needs which | the audit's fix for A-03; twelve models, none of them a Part VII item | y - planner default |
| Usage examples inside Part VII | they become `rust,fragment`; in Part VII a `rust` fence is an excerpt of `std` | no new fence class (`msl_corpus` rejects unknown classes); a fragment is already "a piece of a file" | y - planner default |
| Derivative of an intrinsic function | the engine supplies it; `#[derivative]` is neither needed nor allowed | `#[derivative]` is defined on `extern "C" fn` (B.3.2 `attr_derivative`) | y - planner default inside Q1 |
| Codes | none new: `E0125` gains one condition | §0.2: one code per condition family; the row already covers ABI and shape of an `extern` function | y - planner default inside Q1 |
| The MSL Book | its source moves to `docs/book/` (the three files); the built `index.html` is not committed; the new chapter is chapter 10, after "Files, modules and packages" | user requirement, 2026-11-14; a generated file in git drifts from its source | y - user: "atualizar o book, criar uma session sobre o que tem no core + std lib" (location and build output: planner default) |
| The fixture prelude | deleted; the tests that used it build against the embedded library | A-42: a stand-in hides what the real text does | y - planner default |
| Project decision record | `MD-60` is appended to `.specs/STATE.md` when the spec is approved | the approval is the user's | y - planner default |

**Open questions:** none - all resolved or logged above. One row is `n`: the `laplace_nd` restriction, a consequence of Q2 the user has not seen; the spec is written with it and it is the first line of the hand-off.

**Implicit-requirement dimensions.** Input validation & bounds: STD-04 (the intrinsic form is rejected outside `std` and with a body); STD-08 AC 9 (root lists are checked by a `where`). Failure / partial failure: STD-10 AC 4 (a replaced `std` that lacks an item reports `E0161`, the build still completes). Idempotency / retry: N/A because a build is a pure function of the sources. Auth boundaries & rate limits: N/A because nothing is called. Concurrency / ordering: N/A because the `Design` is immutable and built in one pass; STD-07 AC 4 fixes the file order. Data lifecycle: N/A because the library is compiled into the binary. Observability: every failure of a guard names the file, line and item (STD-11 AC 4, STD-12 AC 4, STD-14 AC 6, STD-18 AC 12). External-dependency failure: STD-18 AC 2 needs `python3` to build the Book; the library and its guards need nothing. State-transition integrity: N/A because nothing has state.

---

## User Stories

### P1: The specification says the library is source, and how it declares what the engine implements ⭐ MVP

**User Story**: As a reader of `docs/spec/`, I want Part VII to say that its listings are the text of `std`, to show engine-implemented items in a form the language accepts, and to use one name for one thing, so that what I read is what the compiler loads.

**Why P1**: MD-57: the specification leads. Every other story writes text this one makes legal.

**Acceptance Criteria** (STD-01, The specification makes the library a normative source and renames the analysis kind):

1. The section §22 SHALL state that the standard library is the crate `std`, that its source files are normative, and that each `rust` fence of Part VII is an excerpt of one of them.
2. The section §22 SHALL hold a table "Sources" that maps each of §22.1 to §22.6 to one namespace of `std`, and one row each for `std::constants` (§1) and `std::carriers` (§4, §11).
3. The section §22 SHALL NOT contain the Resolution that reads listings without `pub` "as if written with `pub`".
4. The entry G.38 of Appendix G SHALL have the status `Closed` and name the two excerpt guards and the gallery gate.
5. The line of Appendix C that says no part of the specification has been compiled SHALL say instead that the library and the galleries are parsed and resolved, and that nothing is executed.
6. The table "Prelude constants" of §1 SHALL say that the six constants are `pub const` items of `std::constants`.
7. The lists of analysis kinds in §11.1.1 and in row 23 of Appendix B.1 SHALL be `Dc`, `Transient`, `Ac`, `Noise`, `Static`.
8. No file of `docs/spec/` SHALL contain the text `analysis(Tran)`.
9. WHEN the token `Tran` occurs in a `rust` or `rust,fragment` fence of `docs/spec/` THEN the next character SHALL be `(`.
10. Appendix G SHALL record the five decisions Q1 to Q5 in its decision table, as S1 to S5.

**Acceptance Criteria** (STD-02, The specification defines `extern "intrinsic"` and what follows from writing the library in MSL):

1. §3 rule 10 SHALL list two ABI strings for a function, `"C"` and `"intrinsic"`, and SHALL say that `"intrinsic"` is accepted only in the crate `std`.
2. §3 SHALL state that an `extern "intrinsic" fn` has no body, is not restricted to scalar types, takes no `#[derivative]`, and has the contract its entry in Part VII gives.
3. The `E0125` row of §3 SHALL include the condition "the ABI `"intrinsic"` outside the crate `std`".
4. §22 SHALL hold a table "Implemented by the engine" whose rows are exactly `matches_bits`, `logic`, `table_model`, `parse_table` and `parse_hex`, each with its entry and a reason.
5. §22 rule 4 SHALL say that an item given as a signature and a contract is declared `extern "intrinsic"`.
6. The count of constructs in Appendix B.1 SHALL stay 65.
7. The error table of Part VII SHALL have no `E0707` row, and §22.2.13 and §22.2.14 SHALL give `E0317` for a complex root without its conjugate and for a zero denominator.
8. §22.2.14 SHALL state that `laplace_nd` requires `D >= 1` and `N <= D`, and that a placement that breaks it is `E0304`.
9. The entry G.64 of Appendix G SHALL say that the generator of `normal` is the one written in `std::analog`.

**Independent Test**: `cargo test -p piperine-lang --test spec_text` passes with one test per criterion above; each new test fails on the specification as it is today.

---

### P1: The POM builds the library as written ⭐ MVP

**User Story**: As the author of `std`, I want the POM to accept the three forms the library uses and the fixture never exercised, so that the library builds with zero diagnostics and its methods are linked.

**Why P1**: measured on Part VII's own text: 13 `E0125`, 1 `E0161`, and every `known` call left `Unresolved` without a diagnostic.

**Acceptance Criteria** (STD-04, Intrinsic functions in the POM):

1. WHEN a file of the crate `std` declares `extern "intrinsic" fn f(..) -> T;` THEN the POM SHALL report no diagnostic for it.
2. IF a file of any other crate declares an `extern "intrinsic" fn` THEN the POM SHALL report `E0125` at the function's name.
3. IF an `extern "intrinsic" fn` has a body THEN the POM SHALL report `E0125` at the function's name.
4. The handle `Function` SHALL answer `is_intrinsic()`, true exactly when the ABI string is `intrinsic`.
5. WHEN a file of any crate declares `extern "C" fn g(x: f64) -> f64;` THEN the POM SHALL report no diagnostic and `is_intrinsic()` SHALL be false.
6. WHEN a model calls an intrinsic function of `std` THEN the POM SHALL classify the call `CallClass::Function` and resolve the callee to that declaration.

**Acceptance Criteria** (STD-05, An associated name through a supertrait):

1. WHEN a trait declared `trait Known: Repr` writes `Self::WIDTH` in a method body THEN the POM SHALL resolve it to the built-in member `Repr::WIDTH` and report no diagnostic.
2. WHEN a trait with a user supertrait writes `Self::K`, where the supertrait declares `const K`, THEN the POM SHALL resolve it to that item.
3. IF `Self::NAME` in a trait names no item of the trait or of any supertrait THEN the POM SHALL report `E0161` at the path.

**Acceptance Criteria** (STD-06, A method of a blanket implementation on a carrier):

1. WHEN a method is called on a carrier, no carrier built-in and no `impl` of the nature or of its value type has that name, and a visible trait has it and has an implementation `impl<T: Repr> Tr for T`, THEN the POM SHALL resolve the call to the trait's method.
2. WHEN `self.known()` is called inside an `impl` of a nature THEN the POM SHALL resolve it to `Known::known` by the same rule.
3. WHEN a nature's own `impl` and a blanket implementation both have the method name THEN the POM SHALL resolve the call to the nature's function.
4. IF no rule of the member table links the method THEN the slot SHALL stay `Unresolved` for a known receiver and `Deferred` otherwise, with no diagnostic.
5. WHEN a `known`, `known_or` or `is_known` call is linked THEN the trait's method SHALL list the call in `references()`.

**Independent Test**: build `trait Known: Repr { fn known(self) -> Option<Self> { for k in 0..Self::WIDTH { self.level(k)?; } Some(self) } } impl<T: Repr> Known for T {}` plus `nature Bit { value: bool }` and `module M(in d: Bit) { let k = d.known(); }` as one source: zero diagnostics, and `k`'s method call resolves to `Known::known`.

---

### P1: `std` ships ⭐ MVP

**User Story**: As a model author, I want `Bit`, `Electrical`, `idt`, `transition`, `Wire`, `require` and the rest of Part VII to exist without an import, written in the language wherever the language can say it, so that a model written from the specification resolves and I can read what a library item does.

**Why P1**: D4 and Q2. It is the feature.

**Acceptance Criteria** (STD-07, The layout of the crate `std`):

1. The root `std/lib.msl` SHALL declare these `pub` namespaces and no other item: `constants`, `carriers`, `values`, `analog`, `nets`, `convert`, `monitor`, `io`, `prelude`.
2. The file of each namespace SHALL hold the items of its section: `constants` §1 rule 3; `carriers` the natures `Bit`, `Electrical`, `Thermal`, `VSignal` (§4) and the module `Short` (§11); `values` §22.1; `analog` §22.2; `nets` §22.3; `convert` §22.4; `monitor` §22.5; `io` §22.6.
3. The file `std/prelude.msl` SHALL hold one `pub use crate::<namespace>::*;` per namespace of rule 1 other than `prelude`, and no other item.
4. The embedded list of `std` files SHALL name the root first and every other file once.
5. WHEN a `.msl` file exists under `crates/piperine-lang/std/` that the embedded list does not name THEN a test SHALL fail and name the file.

**Acceptance Criteria** (STD-08, The content of the library):

1. WHEN the §22 Index names an item THEN the crate `std` SHALL declare it in the namespace of its entry's section.
2. WHEN a function of `std` is not in the table "Implemented by the engine" THEN it SHALL have a body, and every module of `std` with a result type SHALL have a result expression.
3. The set of functions of `std` for which `is_intrinsic()` is true SHALL equal the rows of the table "Implemented by the engine" of §22.
4. WHEN a function of `std` is intrinsic THEN it SHALL have a parameter of type `&str`.
5. WHEN the §22 Index names an item THEN its declaration in `std` SHALL carry `pub`.
6. The crate `std` SHALL declare no item named like a core name of §14 rule 1.
7. The six constants of `std::constants` SHALL have the values of the table of §1: `Q` 1.602176634e-19, `K_B` 1.380649e-23, `EPS0` 8.8541878128e-12, `PI` and `E` equal to Rust's `f64` constants, `inf` written `1.0 / 0.0`.
8. The modules `zi_np`, `zi_zd` and `zi_zp` SHALL each place exactly one `zi_nd`, and `laplace_np`, `laplace_zd` and `laplace_zp` exactly one `laplace_nd`.
9. WHEN a param of a `zi_*` or `laplace_*` module is named `zeros` or `poles` THEN it SHALL carry a `where` condition.
10. The module `laplace_nd` SHALL declare a signal that is an array of `State` and SHALL have one `analog` block.
11. The module `noise_table` SHALL place `noise` and call `table_model`.
12. The bodies of `same`, `deltalim`, `pnjlim`, `fetlim`, `limvds`, `db10`, `db20` and `normal` SHALL name no core primitive of §14 other than `level`.
13. The module of §22.3.7 SHALL be declared `Tran`, and `Tgate` SHALL place two `Tran`.

**Acceptance Criteria** (STD-09, The prelude is the library, and only the library):

1. WHEN a file of any crate uses by its single name a `pub` item of a namespace of `std` THEN the POM SHALL resolve it to that item through `std::prelude`.
2. The set of names `std::prelude` exports SHALL equal the names of the first column of the §22 Index, plus `Bit`, `Electrical`, `Thermal`, `VSignal`, `Short` and the six constants.
3. WHEN a path is written `std::nets::Wire` THEN the POM SHALL resolve it to the same declaration as `Wire`.
4. IF a file of another crate names a private item of a `std` namespace (`Slewing`) THEN the POM SHALL report `E0161`.
5. No name exported by `std::prelude` SHALL equal the name of a row of the built-in table.
6. WHEN a model calls `clk.rise()` on a port `clk: Bit` THEN the POM SHALL resolve the call to the function `rise` of the `impl Bit` of `std::values`.
7. WHEN a model calls `d.known()` on a port `d: Bit<u8>` THEN the POM SHALL resolve the call to the method `known` of the trait `Known` of `std::values`.
8. WHEN a model places `D2A(d, a, vdd)` and `A2D(a, q)` THEN the POM SHALL link each instance to the module of `std::convert`.
9. WHEN a model places `Setup(clk, d)` with a param block and reads `inertial(x, tpd)` THEN the POM SHALL link each to the module of `std::monitor`.

**Acceptance Criteria** (STD-10, The embedded library builds clean):

1. WHEN a design is built from one empty source THEN `Design::diagnostics()` SHALL be empty.
2. WHEN that design is built THEN no `Field` or `MethodCall` slot of a `std` file SHALL be `Unresolved`.
3. WHEN that design is built THEN `Design::check()` SHALL return no problem.
4. IF `std_source` replaces the library with one that lacks an item THEN a model that names the item SHALL get `E0161` and the build SHALL complete.
5. WHEN a model writes `analysis(Transient)` and places `Tran(a, b, en)` THEN `Transient` SHALL resolve to the built-in variant and `Tran` to the module of `std::nets`.

**Independent Test**: `Design::builder().source("top.msl", "module M(in clk: Bit, a: Electrical, w: Wire) { let e = above(v(a) - 0.5, None, None); }").build()` has no diagnostics, and `above`, `Bit`, `Electrical` and `Wire` each resolve to a declaration whose crate `is_std()`.

---

### P1: Part VII's fences are excerpts of `std` ⭐ MVP

**User Story**: As a maintainer, I want a test to fail when Part VII and `std` drift apart, in either direction, so that the specification cannot describe a library other than the one that ships.

**Why P1**: D4, "make Part VII's fences excerpts of it". Without the guard the two texts diverge at the first edit.

**Acceptance Criteria** (STD-11, Every fence of Part VII is in `std`):

1. WHEN a fence of Part VII has the class `rust` THEN its text, with trailing whitespace of each line and leading and trailing blank lines removed, SHALL occur as consecutive whole lines of the `std` file its section maps to.
2. WHEN a fence of Part VII declares a module `Pi` or `MeasuredDiode`, or names `boot.hex`, THEN the fence SHALL have the class `rust,fragment`.
3. The guard SHALL count the fences it checked and fail when that count is under 45.
4. IF a `rust` fence of Part VII is not in its file THEN the guard SHALL fail and name the spec line of the fence and its first line that has no match.
5. IF the control text of the guard, a fence that differs from the file by one character, is checked THEN the guard SHALL report it as not found.

**Acceptance Criteria** (STD-12, Every item of `std` is in the specification):

1. WHEN a `std` file other than `lib.msl` and `prelude.msl` declares an item at its top level THEN the source text of the item, attributes and doc comments included, SHALL occur verbatim inside a `rust` fence of `docs/spec/`.
2. WHEN a `std` file holds a behavior block THEN the text of the block SHALL occur verbatim inside a `rust` fence of Part VII.
3. The fences of §1, §4 and §11 that show `Q` to `inf`, the four natures and `Short` SHALL show them with `pub`.
4. IF an item of `std` is in no fence THEN the guard SHALL fail and name the `std` file, the line and the item.
5. The guard SHALL count the items it checked and fail when that count is under 80.

**Independent Test**: change one character of `std/analog.msl` inside `idt`: STD-11 fails naming the fence of §22.2.8, and STD-12 fails naming `idt`.

---

### P1: The galleries build clean ⭐ MVP

**User Story**: As a maintainer, I want every gallery model of Appendices D, E and F to build against the real library with nothing reported and nothing silently unlinked, so that name resolution is proven on the specification's own models and not on fixtures written for it.

**Why P1**: D4's gate. Until it exists, G.38 stays open.

**Acceptance Criteria** (STD-13, The models the galleries take from other parts):

1. The fixture directory `tests/fixtures/gallery/` SHALL hold one file for each of `Resistor`, `Capacitor`, `Switch`, `SampleHold`, `Comparator`, `Cell`, `Dram1T1C`, `MemoryBank`, `Stream`, `Tx`, `Vco`, `RSwitch`.
2. WHEN a fixture file is read THEN each of its lines SHALL occur in `docs/spec/`, in a `rust` fence or in a code span of the table "Models referenced from the normative parts" of Appendix D.
3. The gate SHALL hold a table with one row for each of the 18 gallery sections, naming the fixture models appended to its fence.
4. IF the table names a model a gallery does not use, or a model the gallery defines itself, THEN the gate SHALL fail and name the pair.

**Acceptance Criteria** (STD-14, The gate):

1. WHEN a `rust` fence of Appendix D, E or F is built as the root file of a crate, with its fixture models appended and the embedded `std`, THEN `Design::diagnostics()` SHALL be empty.
2. WHEN that design is built THEN `Design::check()` SHALL return no problem.
3. WHEN that design is built THEN no `Field` or `MethodCall` slot of the root file SHALL be `Unresolved`.
4. WHEN a gallery names a `std` item the gate's table lists for it THEN that item's `references()` SHALL include a reference in the root file, and the item's crate SHALL answer `is_std()`.
5. The gate SHALL build 18 fences and fail when it finds another number.
6. IF a gallery is not clean THEN the gate SHALL fail and print, per fence, the spec file and line, each diagnostic's code and source text, and each unresolved member's name.
7. IF the same fences are built against a `std` of an empty `prelude` THEN the gate's control SHALL observe at least one `E0161` in every fence.
8. The gate's table of rule 4 SHALL hold at least one `std` item for each of the 18 fences.

**Independent Test**: `cargo test -p piperine-lang --test std_galleries` passes; deleting `pub` from `nature Bit` in `std/carriers.msl` makes it fail with `E0161` on `Bit`.

---

### P2: The built-in table shrinks, the stand-ins go, the reference follows

**User Story**: As a consumer of the POM, I want a name to come from one place, the core table or a declaration of `std`, and the reference documentation to say which, so that `Resolution::Builtin` means "core" and nothing else.

**Why P2**: the library works without it, but two sources for `K_B` and a fixture that shadows the real library would hide the next drift.

**Acceptance Criteria** (STD-15, The built-in table holds core names only):

1. The built-in table SHALL hold no row for `Q`, `K_B`, `EPS0`, `PI`, `E` or `inf`.
2. WHEN a model names `K_B` THEN the POM SHALL resolve it to `Resolution::Decl` of the constant of `std::constants`.
3. The built-in table SHALL hold a row `Transient` of kind `Variant` and no row `Tran`.
4. The rows of kind `Type`, `Trait`, `Fn`, `Module`, `Event`, `Statement`, `Node`, `ImplicitParam`, `Member` and `Method`, and the variants `Some`, `None`, `Dc`, `Ac`, `Noise`, `Static`, SHALL be the rows the table has today.
5. IF a model declares a namespace item named `Q` THEN the POM SHALL report no `E0457`, and the model's item SHALL hide the prelude's.

**Acceptance Criteria** (STD-16, The fixture prelude is retired):

1. The file `tests/fixtures/pom/prelude.msl` SHALL NOT exist, and no test file of `piperine-lang` SHALL name it.
2. The targets `pom_classify`, `pom_reflect` and `pom_resolve` SHALL hold at least 39, 31 and 48 tests.

**Acceptance Criteria** (STD-17, The reference documentation):

1. The page `docs/pom/resolution.md` SHALL list the built-in table without a `Const` row and say that the six constants are items of `std::constants`.
2. The page `docs/pom/resolution.md` SHALL name `extern "intrinsic"` and `is_intrinsic` where it describes `E0125`.
3. The page `docs/pom/resolution.md` SHALL say that `Self::NAME` in a trait resolves through the supertraits.
4. The Members table of `docs/pom/resolution.md` SHALL list the blanket implementation as the last step for a carrier.
5. WHEN the examples of `docs/pom/` run as doctests THEN every example SHALL pass against the embedded library.
6. The file comment of `std/lib.msl` SHALL name the nine namespaces and SHALL NOT say the content is future work.

**Independent Test**: `builtin("K_B")` is `None`; `Design::builder().source("t.msl", "const X: f64 = K_B;").build()` has no diagnostics and `K_B` resolves to a declaration in `std/constants.msl`.

---

### P2: The MSL Book explains what is core and what is `std`

**User Story**: As a newcomer reading The MSL Book, I want one chapter that says which names the language itself provides and which come from the standard library, namespace by namespace, so that I know where a name comes from and what I get without an import.

**Why P2**: user requirement (2026-11-14: "Bora colocar na spec do std lib para atualizar o book, criar uma session sobre o que tem no core + std lib"). The Book's source lives only in a session scratchpad today; this story also makes it durable and tested.

**Acceptance Criteria** (STD-18, The Book has a chapter "Core and standard library" and lives in the repository):

1. The directory `docs/book/` SHALL hold the Book's source, `content.html`, `template.html` and `build.py`, with the twelve chapters it has today.
2. WHEN `python3 docs/book/build.py` runs THEN it SHALL write `docs/book/index.html`, and that file SHALL be ignored by git.
3. The file `docs/book/content.html` SHALL hold a chapter with the id `core-std` and the title "Core and standard library", placed after the chapter `packages`, and the chapter numbers SHALL be consecutive from 1.
4. WHEN a name is a core name (a row of the built-in table of kind `Fn`, `Module`, `Event`, `Statement`, `Method`, `Node` or `ImplicitParam`, or `Nature`, `str`, `emit!`, `include_str!`) THEN the chapter SHALL name it in a code span.
5. WHEN a namespace of `std` is `pub` THEN the chapter SHALL name it as `std::<name>` in a code span inside a heading of its own.
6. WHEN a name is exported by `std::prelude` THEN the chapter SHALL name it in a code span.
7. The chapter SHALL name `std::prelude` and each of the five items declared `extern "intrinsic"` under a heading that contains the word "engine".
8. WHEN a block of the chapter is marked `data-lang="msl"` THEN its text SHALL build as one file against the embedded library with zero diagnostics.
9. WHEN a block of any chapter is marked `data-lang="msl"` THEN its text, HTML entities decoded, SHALL parse with zero diagnostics as a file or as one of the fragment forms `msl_corpus` accepts.
10. The chapter `packages` SHALL name `std::prelude` in a code span, and the chapter `cheatsheet` SHALL name every `pub` namespace of `std` as `std::<name>`.
11. The check SHALL count the `msl` blocks of the Book and fail when it finds fewer than 40.
12. IF the chapter omits a core name or an export THEN the check SHALL fail and list every missing name.
13. No block of the Book SHALL contain `analysis(Tran)`.

**Independent Test**: `cargo test -p piperine-lang --test book` passes; adding a `pub fn extra() {}` to `std/values.msl` (and to Part VII) makes it fail naming `extra`.

---

## Edge Cases

- IF a model declares its own `idt` THEN the POM SHALL resolve `idt` to the model's item and report nothing (§22 rule 3; STD-15 AC 5 for a constant).
- IF a model declares `extern "intrinsic" fn f();` THEN the POM SHALL report `E0125` (STD-04 AC 2).
- WHEN `Tran` is written in a model THEN the POM SHALL resolve it to the module of `std::nets`, and `Transient` to the analysis kind (STD-10 AC 5).
- WHEN a `std` file names an item of another `std` namespace by its single name THEN the POM SHALL resolve it through the prelude, as §3.5 rule 9 says (STD-09 AC 1).
- IF two namespaces of `std` export one name THEN the prelude's two globs SHALL make each use of it `E0162`, which STD-10 AC 1 turns into a failing build.
- WHEN a Part VII fence ends without a newline or carries trailing spaces THEN the guard SHALL still match it (STD-11 AC 1).
- IF a gallery fence defines a model that is also a fixture THEN the gate's table SHALL not append the fixture to that fence (STD-13 AC 4).
- WHEN `std_source` replaces the library in a test THEN the six constants SHALL be absent unless the replacement declares them (STD-15 AC 1).
- WHEN `laplace_nd` is placed with one denominator coefficient (`D == 1`) THEN the module SHALL have no state and its result SHALL be `num[0] * x / den[0]` (STD-08 AC 10; `design.md`).
- IF a list of roots holds a complex root without its conjugate THEN the `where` of the param SHALL be false (STD-08 AC 9).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| STD-01 | P1: The specification says the library is source | Tasks | Pending |
| STD-02 | P1: The specification says the library is source | Tasks | Pending |
| STD-04 | P1: The POM builds the library as written | Tasks | Pending |
| STD-05 | P1: The POM builds the library as written | Tasks | Pending |
| STD-06 | P1: The POM builds the library as written | Tasks | Pending |
| STD-07 | P1: `std` ships | Tasks | Pending |
| STD-08 | P1: `std` ships | Tasks | Pending |
| STD-09 | P1: `std` ships | Tasks | Pending |
| STD-10 | P1: `std` ships | Tasks | Pending |
| STD-11 | P1: Part VII's fences are excerpts of `std` | Tasks | Pending |
| STD-12 | P1: Part VII's fences are excerpts of `std` | Tasks | Pending |
| STD-13 | P1: The galleries build clean | Tasks | Pending |
| STD-14 | P1: The galleries build clean | Tasks | Pending |
| STD-15 | P2: The built-in table shrinks | Tasks | Pending |
| STD-16 | P2: The built-in table shrinks | Tasks | Pending |
| STD-17 | P2: The built-in table shrinks | Tasks | Pending |
| STD-18 | P2: The MSL Book explains what is core and what is `std` | Tasks | Pending |

`STD-03` is not used.

**Coverage:** 17 requirements, 113 criteria, 113 mapped to checks and tasks (`tasks.md`).

---

## Sources

- `.specs/features/msl-audit/report.md`: decision D4, findings A-03 and A-42. What the feature must deliver.
- `.specs/features/msl-stdlib/context.md`, "Answers": Q1 to Q5 in the user's words.
- `.specs/STATE.md` MD-56 (D3, D4), MD-57 (spec edits and their guards), MD-59 (`std` embedded).
- `.specs/features/msl-packages/{context,spec,design}.md`: U1, U2, PKG-06. Where `std` lives and what the prelude is.
- `docs/spec/part_vii_stdlib.md`: the items, and §22 rules 1 to 5.
- `docs/spec/part_iv_behavior.md` §11.1.1 (the analysis kinds), §14 (the core primitives, which stay out of `std`).
- `docs/spec/part_i_values.md` §1 rule 3, §3 rules 2 and 10, §3.4.2, §3.5 rules 4 and 9; `part_iii_structure.md` §8.1 (`E0317`, `E0304`).
- `crates/piperine-ir/src/limit.rs`: `pnjlim`, `fetlim`, `limvds` as checked against ngspice.
- The MSL Book, `https://claude.ai/artifact/6nvqf1rKtF3TteqjEv6z26`; its source at planning time: `<session scratchpad>/manual/build/{content.html,template.html,build.py}` (12 chapters, 53 code blocks, 43 of them `msl`).
- `docs/spec/appendix_g_scope.md` G.38, G.64. `docs/pom/resolution.md`, "The built-in table".

---

## Success Criteria

- [ ] `cargo test -p piperine-lang` passes, with the targets `std_library`, `std_excerpts`, `std_galleries` and `book` in it.
- [ ] 18 of 18 gallery fences are clean against the embedded library; the same fences against an empty prelude are not.
- [ ] Exactly five functions of `std` have no body, and each takes a `&str`.
- [ ] A one-character edit of `std` or of a Part VII fence fails a guard that names the place.
- [ ] `analysis(Tran)` occurs nowhere in `docs/spec/`, `std` or the Book.
- [ ] `tests/fixtures/pom/prelude.msl` is gone and no test count dropped.
- [ ] G.38 is `Closed` in Appendix G.
- [ ] `docs/book/` builds the Book with `python3 docs/book/build.py`, and the chapter "Core and standard library" names every core name and every `std` export.

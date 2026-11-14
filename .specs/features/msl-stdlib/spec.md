# MSL Standard Library Specification

## Problem Statement

Part VII of `docs/spec/` describes a standard library that does not exist as source. `crates/piperine-lang/std/` holds a root and an empty `prelude` (`msl-packages`, PKG-06), so every gallery model of Appendices D, E and F fails to resolve: 0 of 18 gallery fences build clean, with 250 `E0161`. The front end has only ever resolved names against a hand-written test fixture of stand-ins (`tests/fixtures/pom/prelude.msl`, audit findings A-03 and A-42).

Decision D4 (user, 2026-11-14: "sim") makes the library a normative source: `std/*.msl` ships, Part VII's fences are excerpts of it, every export is `pub`, and "the galleries build clean" is a gate. This feature delivers that, and the three things the library needs from the language and the POM to be writable at all.

## Goals

- [ ] `crates/piperine-lang/std/*.msl` holds every item of Part VII and the prelude items of §1, §4 and §11, and builds with zero diagnostics.
- [ ] Every `rust` fence of Part VII is, character for character, a block of a `std` file, and every item of `std` is shown in a `rust` fence of `docs/spec/`. Two tests hold both directions.
- [ ] All 18 gallery fences of Appendices D, E and F build clean against the embedded library, with no stand-in.
- [ ] `docs/spec/` says how `std` declares an item the engine implements, and the POM accepts that form in `std` only.
- [ ] The built-in table holds core names only: the six prelude constants leave it, and no `std` export shares a name with a core row.
- [ ] The MSL Book's source is in `docs/book/`, has a chapter "Core and standard library", and a test ties that chapter to the built-in table and to `std`.
- [ ] `cargo test -p piperine-lang` is green at every commit.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Type checking, elaboration, simulation of the library | no such phase exists; the front end stops at parse and POM resolution. "Clean" is defined on those two (STD-14) |
| MSL bodies for the items declared `extern "intrinsic"` | a body nobody can run should not become normative (Q2). A later feature replaces each once an evaluator exists |
| Binding an intrinsic to an engine implementation | there is no elaborator or code generator for MSL yet. This feature fixes the list of names (STD-04 AC 6) |
| A device library (`Resistor`, `Capacitor`, `Switch`, SPICE models) in `std` | they are not Part VII items; the galleries take them from §8, §11, §12, §13. They are supplied to the gate as checked excerpts (STD-13) |
| The fences of Parts I to VI built clean against `std` | most are pieces that need context; the gate D4 names is the galleries |
| `str` methods, const-generic arithmetic, any other new library or language surface | not needed under Q1 option A |
| `piperine-project`, the CLI, the LSP, the Python binding | `std` is embedded in `piperine-lang`; no host changes |
| Publishing The MSL Book (the artifact `https://claude.ai/artifact/6nvqf1rKtF3TteqjEv6z26`) | done by the orchestrator after the merge, from `docs/book/`; no task publishes. This feature delivers the source and its checks (STD-18) |
| Rewriting the Book's existing chapters | only the new chapter, the packages note of chapter 9 and the cheat sheet change; an existing block is edited only where STD-18 AC 9 finds it does not parse |
| `git push`, merge to `main` | needs the user's go-ahead for that action |

---

## Assumptions & Open Questions

D4, D3, U1, U2 are the user's decisions (`context.md`). Q1 to Q5 are open: each row gives the recommended answer the spec is written under, and is `n` until the user answers. The remaining rows are the planner's choices inside the decisions.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D4 The library is a normative source | `std/*.msl` ships; Part VII's fences are excerpts; `pub` on exports; the galleries build clean | user decision | y - user: "sim" (audit D4, MD-56) |
| D3 Core names | `gnd`, `Nature`, `m`, `dtemp`, `str` and the §14 primitives stay in the built-in table and out of `std` | user decision | y - user: "sim" (MD-56) |
| U2 Where `std` lives | embedded in `piperine-lang`, `include_str!` of `crates/piperine-lang/std/*.msl` | user decision | y - user: "std fica embutida o binário." |
| Q1 An item the engine implements | `extern "intrinsic"` on `fn` and `module`, accepted only in the crate `std`; no grammar change, no new code | reuses `extern STRING`, which B.3.2 already has on both; keeps §14 and §22 rule 3 as they are | n - recommended, awaiting the user |
| Q2 Bodies now or intrinsic | MSL bodies for `same`, `deltalim`, `db10`, `db20`, `noise_table`; `extern "intrinsic"` for the other 9 functions and 7 modules | those five have their definition in Part VII's prose already; the rest could not be run | n - recommended, awaiting the user |
| Q3 `Tran` names a module and an analysis kind | the module of §22.3.7 is renamed `TranIf`; `Tran` stays the analysis kind | the prelude is looked up before the core names, so today `analysis(Tran)` would name the module, silently | n - recommended, awaiting the user |
| Q4 Public namespaces | `std::{constants, carriers, values, analog, nets, convert, monitor, io}`, one file each, and `std::prelude` re-exporting all of them | one file per §22.x section makes the excerpt rule a table of six rows | n - recommended, awaiting the user |
| Q5 Prelude constants | `Q`, `K_B`, `EPS0`, `PI`, `E`, `inf` become `pub const` items of `std::constants`; `inf` is `1.0 / 0.0` | §1 rule 3 already says "the prelude has" them; a literal out of range is `E0909` | n - recommended, awaiting the user |
| What "clean" means | zero diagnostics in every file of the design, `Design::check()` empty, no `Unresolved` member slot, and each `std` name the gallery uses linked to a declaration of the crate `std` | only parse and POM resolution exist; a silent `Unresolved` is the one way a POM build hides a miss | y - planner default |
| Models the galleries take from other parts | a test fixture, one file per model, each checked to be text of `docs/spec/`; a table says which gallery needs which | the audit's fix for A-03; nine models, none of them a Part VII item | y - planner default |
| Usage examples inside Part VII | they become `rust,fragment`; in Part VII a `rust` fence is an excerpt of `std` | no new fence class (`msl_corpus` rejects unknown classes); a fragment is already "a piece of a file" | y - planner default |
| Derivative of an intrinsic function | the engine supplies it; `#[derivative]` is neither needed nor allowed | `#[derivative]` is defined on `extern "C" fn` (B.3.2 `attr_derivative`) | y - planner default inside Q1 |
| Codes | none new: `E0125` (function) and `E0370` (module) gain one condition each | §0.2: one code per condition family; both rows already cover ABI and shape of `extern` items | y - planner default inside Q1 |
| The MSL Book | its source moves to `docs/book/` (the three files); the built `index.html` is not committed; the new chapter is chapter 10, after "Files, modules and packages" | user requirement, 2026-11-14; a generated file in git drifts from its source | y - user: "atualizar o book, criar uma session sobre o que tem no core + std lib" (location and build output: planner default) |
| The fixture prelude | deleted; the tests that used it build against the embedded library | A-42: a stand-in hides what the real text does | y - planner default |
| Project decision record | `MD-60` is appended to `.specs/STATE.md` when the spec is approved, not before | Q1 is a language decision the user has not taken | y - planner default |

**Open questions:** Q1 to Q5, listed in `context.md` with options and a recommendation each. They block approval of this spec; they do not block reading it.

**Implicit-requirement dimensions.** Input validation & bounds: STD-04 (the intrinsic form is rejected outside `std`, with a body, with a behavior block). Failure / partial failure: STD-10 AC 4 (a replaced `std` that lacks an item reports `E0161`, the build still completes). Idempotency / retry: N/A because a build is a pure function of the sources. Auth boundaries & rate limits: N/A because nothing is called. Concurrency / ordering: N/A because the `Design` is immutable and built in one pass; STD-07 AC 4 fixes the file order. Data lifecycle: N/A because the library is compiled into the binary. Observability: every failure of a guard names the file, line and item (STD-11 AC 4, STD-12 AC 4, STD-14 AC 6). External-dependency failure: STD-18 AC 2 needs `python3` to build the Book; the library and its guards need nothing. State-transition integrity: N/A because nothing has state.

---

## User Stories

### P1: The specification says the library is source, and how it declares what the engine implements ⭐ MVP

**User Story**: As a reader of `docs/spec/`, I want Part VII to say that its listings are the text of `std` and to show engine-implemented items in a form the language accepts, so that what I read is what the compiler loads.

**Why P1**: MD-57: the specification leads. Every other story writes text this one makes legal.

**Acceptance Criteria** (STD-01, The specification makes the library a normative source):

1. The section §22 SHALL state that the standard library is the crate `std`, that its source files are normative, and that each `rust` fence of Part VII is an excerpt of one of them.
2. The section §22 SHALL hold a table that maps each of §22.1 to §22.6 to one namespace of `std`, and one row each for `std::constants` (§1) and `std::carriers` (§4, §11).
3. The section §22 SHALL NOT contain the Resolution that reads listings without `pub` "as if written with `pub`".
4. The entry G.38 of Appendix G SHALL have the status `Closed` and name the two guards and the gallery gate.
5. The line of Appendix C that says no part of the specification has been compiled SHALL say instead that the library and the galleries are parsed and resolved, and that nothing is executed.
6. The table "Prelude constants" of §1 SHALL say that the six constants are `pub const` items of `std::constants`.
7. WHEN the specification names the pass switch of §22.3.7 THEN it SHALL name it `TranIf` in every part, index and gallery.

**Acceptance Criteria** (STD-02, The specification defines `extern "intrinsic"`):

1. §3 rule 10 SHALL list two ABI strings for a function, `"C"` and `"intrinsic"`, and SHALL say that `"intrinsic"` is accepted only in the crate `std`.
2. §3 SHALL state that an `extern "intrinsic" fn` has no body, is not restricted to scalar types, takes no `#[derivative]`, and has the contract its entry in Part VII gives.
3. The `E0125` row of §3 SHALL include the condition "the ABI `"intrinsic"` outside the crate `std`".
4. §8.5 SHALL state that an `extern "intrinsic" module` may have a result type and value inputs, holds params only in its body, takes no behavior block and no `#[link]`, and follows §14 rule 3 as a core module does.
5. The `E0370` row of §8.5 SHALL include the condition "the ABI `"intrinsic"` on a module outside the crate `std`".
6. §22 SHALL hold a table "Implemented by the engine" that lists every `extern "intrinsic"` item of `std` by name and entry.
7. §22 rule 4 SHALL say that an item given as a signature and a contract is declared `extern "intrinsic"`.
8. The count of constructs in Appendix B.1 SHALL stay 65.
9. Appendix G SHALL record the five decisions Q1 to Q5 in its decision table, as S1 to S5.

**Independent Test**: `cargo test -p piperine-lang --test spec_text` passes with one test per criterion above; each test fails on the specification as it is today.

---

### P1: The POM builds the library as written ⭐ MVP

**User Story**: As the author of `std`, I want the POM to accept the three forms the library uses and the fixture never exercised, so that the library builds with zero diagnostics and its methods are linked.

**Why P1**: measured on Part VII's own text: 13 `E0125`, 1 `E0161`, and every `known` call left `Unresolved` without a diagnostic.

**Acceptance Criteria** (STD-04, Intrinsic items in the POM):

1. WHEN a file of the crate `std` declares `extern "intrinsic" fn f(..) -> T;` THEN the POM SHALL report no diagnostic for it.
2. IF a file of any other crate declares an `extern "intrinsic" fn` THEN the POM SHALL report `E0125` at the function's name.
3. IF an `extern "intrinsic" fn` has a body THEN the POM SHALL report `E0125` at the function's name.
4. WHEN a file of the crate `std` declares an `extern "intrinsic" module` with a result type, value inputs and a body of params only THEN the POM SHALL report no diagnostic for it.
5. IF a file of any other crate declares an `extern "intrinsic" module` THEN the POM SHALL report `E0370` at the module's name.
6. IF an `extern "intrinsic" module` has a body item that is not a param THEN the POM SHALL report `E0370` at that item.
7. IF a behavior block targets an `extern "intrinsic" module` THEN the POM SHALL report `E0402` at the target path.
8. WHEN a call names an `extern "intrinsic" module` THEN the POM SHALL classify it `CallClass::Instance` with that module.
9. The handles `Function` and `Module` SHALL each answer `is_intrinsic()`, true exactly when the ABI string is `intrinsic`.
10. WHEN an `extern "osdi"` or `extern "cosim"` module has a result type or a value input THEN the POM SHALL still report `E0370`.

**Acceptance Criteria** (STD-05, An associated name through a supertrait):

1. WHEN a trait declared `trait Known: Repr` writes `Self::WIDTH` in a method body THEN the POM SHALL resolve it to the built-in member `Repr::WIDTH` and report no diagnostic.
2. WHEN a trait with a user supertrait writes `Self::K`, where the supertrait declares `const K` THEN the POM SHALL resolve it to that item.
3. IF `Self::NAME` in a trait names no item of the trait or of any supertrait THEN the POM SHALL report `E0161` at the path.

**Acceptance Criteria** (STD-06, A method of a blanket implementation on a carrier):

1. WHEN a method is called on a carrier, no carrier built-in and no `impl` of the nature or of its value type has that name, and a trait in scope has it and has an implementation `impl<T: Repr> Tr for T` THEN the POM SHALL resolve the call to the trait's method.
2. WHEN `self.known()` is called inside an `impl` of a nature THEN the POM SHALL resolve it to `Known::known` by the same rule.
3. WHEN a nature's own `impl` and a blanket implementation both have the method name THEN the POM SHALL resolve the call to the nature's function.
4. IF no rule of the member table links the method THEN the slot SHALL stay as the POM leaves it today (`Unresolved` for a known receiver, `Deferred` otherwise), with no diagnostic.
5. WHEN a `known`, `known_or` or `is_known` call is linked THEN `Known`'s method SHALL list the call in `references()`.

**Independent Test**: build `trait Known: Repr { fn known(self) -> Option<Self> { for k in 0..Self::WIDTH { self.level(k)?; } Some(self) } } impl<T: Repr> Known for T {}` plus `nature Bit { value: bool }` and `module M(in d: Bit) { let k = d.known(); }` as one source: zero diagnostics, and `k`'s method call resolves to `Known::known`.

---

### P1: `std` ships ⭐ MVP

**User Story**: As a model author, I want `Bit`, `Electrical`, `idt`, `transition`, `Wire`, `require` and the rest of Part VII to exist without an import, so that a model written from the specification resolves.

**Why P1**: D4. It is the feature.

**Acceptance Criteria** (STD-07, The layout of the crate `std`):

1. The root `std/lib.msl` SHALL declare these `pub` namespaces and no other item: `constants`, `carriers`, `values`, `analog`, `nets`, `convert`, `monitor`, `io`, `prelude`.
2. The file of each namespace SHALL hold the items of its section: `constants` §1 rule 3; `carriers` the natures `Bit`, `Electrical`, `Thermal`, `VSignal` (§4) and the module `Short` (§11); `values` §22.1; `analog` §22.2; `nets` §22.3; `convert` §22.4; `monitor` §22.5; `io` §22.6.
3. The file `std/prelude.msl` SHALL hold one `pub use crate::<namespace>::*;` per namespace of rule 1 other than `prelude`, and no other item.
4. The embedded list of `std` files SHALL name the root first and every other file once.
5. WHEN a `.msl` file exists under `crates/piperine-lang/std/` that the embedded list does not name THEN a test SHALL fail and name the file.

**Acceptance Criteria** (STD-08, The content of the library):

1. WHEN the §22 Index names an item THEN the crate `std` SHALL declare it in the namespace of its entry's section.
2. The crate `std` SHALL declare `same`, `deltalim`, `db10`, `db20` and `noise_table` with a body.
3. The crate `std` SHALL declare `extern "intrinsic"` exactly the items of the table "Implemented by the engine" of §22.
4. WHEN an item is exported by Part VII THEN its declaration in `std` SHALL carry `pub`.
5. The crate `std` SHALL declare no item named like a core name of §14 rule 1.
6. The six constants of `std::constants` SHALL have the values of the table of §1: `Q` 1.602176634e-19, `K_B` 1.380649e-23, `EPS0` 8.8541878128e-12, `PI` and `E` equal to Rust's `f64` constants, `inf` positive infinity.
7. The module of §22.3.7 SHALL be declared `TranIf`, and `Tgate` SHALL place two `TranIf`.

**Acceptance Criteria** (STD-09, The prelude is the library, and only the library):

1. WHEN a file of any crate uses by its single name a `pub` item of a namespace of `std` THEN the POM SHALL resolve it to that item through `std::prelude`.
2. The set of names `std::prelude` exports SHALL equal the names of the first column of the §22 Index, plus `Bit`, `Electrical`, `Thermal`, `VSignal`, `Short` and the six constants.
3. WHEN a path is written `std::nets::Wire` THEN the POM SHALL resolve it to the same declaration as `Wire`.
4. IF a name is a private item of a `std` namespace (`Slewing`) THEN a file of another crate that uses it SHALL get `E0161`.
5. No name exported by `std::prelude` SHALL equal the name of a row of the built-in table.

**Acceptance Criteria** (STD-10, The embedded library builds clean):

1. WHEN a design is built from one empty source THEN `Design::diagnostics()` SHALL be empty.
2. WHEN that design is built THEN no `Field` or `MethodCall` slot of a `std` file SHALL be `Unresolved`.
3. WHEN that design is built THEN `Design::check()` SHALL return no problem.
4. IF `std_source` replaces the library with one that lacks an item THEN a model that names the item SHALL get `E0161` and the build SHALL complete.
5. WHEN `analysis(Tran)` is written in a `std` file or in a model THEN `Tran` SHALL resolve to the built-in variant.

**Independent Test**: `Design::builder().source("top.msl", "module M(in clk: Bit, a: Electrical, w: Wire) { let e = above(v(a) - 0.5, None, None); }").build()` has no diagnostics, and `above`, `Bit`, `Electrical` and `Wire` each resolve to a declaration whose crate `is_std()`.

---

### P1: Part VII's fences are excerpts of `std` ⭐ MVP

**User Story**: As a maintainer, I want a test to fail when Part VII and `std` drift apart, in either direction, so that the specification cannot describe a library other than the one that ships.

**Why P1**: D4, "make Part VII's fences excerpts of it". Without the guard the two texts diverge at the first edit.

**Acceptance Criteria** (STD-11, Every fence of Part VII is in `std`):

1. WHEN a fence of Part VII has the class `rust` THEN its text, with trailing whitespace of each line and leading and trailing blank lines removed, SHALL occur as consecutive whole lines of the `std` file its section maps to.
2. WHEN a fence of Part VII shows a use of the library and no declaration of it THEN the fence SHALL have the class `rust,fragment`.
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
3. The gate SHALL hold a table that names, for each gallery section, the fixture models appended to its fence.
4. IF the table names a model a gallery does not use, or a gallery defines itself THEN the gate SHALL fail and name the pair.

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
3. The enum `BuiltinKind` SHALL have no variant `Const`.
4. The rows of kind `Type`, `Variant`, `Trait`, `Fn`, `Module`, `Event`, `Statement`, `Node`, `ImplicitParam`, `Member` and `Method` SHALL be the rows the table has today.
5. IF a model declares a namespace item named `Q` THEN the POM SHALL report no `E0457`, and the model's item SHALL hide the prelude's.

**Acceptance Criteria** (STD-16, The fixture prelude is retired):

1. The file `tests/fixtures/pom/prelude.msl` SHALL NOT exist.
2. WHEN a test of `pom_classify`, `pom_reflect` or `pom_resolve` needs `Bit`, `Electrical`, `idt`, `above` or `transition` THEN it SHALL build against the embedded library.
3. The number of tests in `pom_classify`, `pom_reflect` and `pom_resolve` SHALL be no lower than before this feature.
4. WHEN a test needs a library of its own to prove a rule of the prelude THEN it SHALL pass it with `std_source` in the test's own text.

**Acceptance Criteria** (STD-17, The reference documentation):

1. The page `docs/pom/resolution.md` SHALL list the built-in table without the `Const` row and say that the six constants are items of `std::constants`.
2. The page `docs/pom/resolution.md` SHALL describe `extern "intrinsic"` items, `is_intrinsic()`, the supertrait rule of STD-05 and the blanket rule of STD-06 in the tables they extend.
3. WHEN `pom_docs` runs the examples of `docs/pom/` THEN every example SHALL pass against the embedded library.
4. The file comment of `std/lib.msl` SHALL name the namespaces and SHALL NOT say the content is future work.

**Independent Test**: `builtin("K_B")` is `None`; `Design::builder().source("t.msl", "const X: f64 = K_B;").build()` has no diagnostics and `K_B` resolves to a declaration in `std/constants.msl`.

---

### P2: The MSL Book explains what is core and what is `std`

**User Story**: As a newcomer reading The MSL Book, I want one chapter that says which names the language itself provides and which come from the standard library, namespace by namespace, so that I know where a name comes from and what I get without an import.

**Why P2**: user requirement (2026-11-14: "Bora colocar na spec do std lib para atualizar o book, criar uma session sobre o que tem no core + std lib"). The Book's source lives only in a session scratchpad today; this story also makes it durable and tested.

**Acceptance Criteria** (STD-18, The Book has a chapter "Core and standard library" and lives in the repository):

1. The directory `docs/book/` SHALL hold the Book's source, `content.html`, `template.html` and `build.py`, moved from the session scratchpad with every existing chapter kept.
2. WHEN `python3 docs/book/build.py` runs THEN it SHALL write `docs/book/index.html`, and that file SHALL be ignored by git.
3. The file `docs/book/content.html` SHALL hold a chapter with the id `core-std` and the title "Core and standard library", placed after the chapter `packages`, and the chapter numbers SHALL be consecutive from 1.
4. WHEN a name is a core name (a row of the built-in table of kind `Fn`, `Module`, `Event`, `Statement`, `Method`, `Node` or `ImplicitParam`, or `Nature`, `str`, `emit!`, `include_str!`) THEN the chapter SHALL name it in a code span.
5. WHEN a namespace of `std` is `pub` THEN the chapter SHALL name it as `std::<name>` in a code span, in a part of the chapter of its own.
6. WHEN a name is exported by `std::prelude` THEN the chapter SHALL name it in a code span.
7. The chapter SHALL state that `std::prelude` is implied in every file, and SHALL list the items declared `extern "intrinsic"` as implemented by the engine.
8. WHEN a block of the chapter is marked `data-lang="msl"` THEN its text SHALL build as one file against the embedded library with zero diagnostics.
9. WHEN a block of any chapter is marked `data-lang="msl"` THEN its text, HTML entities decoded, SHALL parse with zero diagnostics as a file or as one of the fragment forms `msl_corpus` accepts.
10. The chapter `packages` SHALL name `std::prelude` in a code span, and the chapter `cheatsheet` SHALL name every `pub` namespace of `std` as `std::<name>`.
11. The check SHALL count the `msl` blocks of the Book and fail when it finds fewer than 40.
12. IF the chapter omits a core name or an export THEN the check SHALL fail and list every missing name.

**Independent Test**: `cargo test -p piperine-lang --test book` passes; adding a `pub fn extra() {}` to `std/values.msl` (and to Part VII) makes it fail naming `extra`.

---

## Edge Cases

- IF a model declares its own `idt` THEN the POM SHALL resolve `idt` to the model's item and report nothing (§22 rule 3; STD-15 AC 5 for a constant).
- IF a model declares `extern "intrinsic" fn f();` THEN the POM SHALL report `E0125` (STD-04 AC 2).
- WHEN `Tran` is written where a value is expected THEN the POM SHALL resolve it to the analysis kind, in `std` and in models (STD-10 AC 5).
- WHEN a `std` file names an item of another `std` namespace by its single name THEN the POM SHALL resolve it through the prelude, as §3.5 rule 9 says (STD-09 AC 1).
- IF two namespaces of `std` export one name THEN the prelude's two globs SHALL make each use of it `E0162`, which STD-10 AC 1 turns into a failing build.
- WHEN a Part VII fence ends without a newline or carries trailing spaces THEN the guard SHALL still match it (STD-11 AC 1).
- IF a gallery fence defines a model that is also a fixture THEN the gate's table SHALL not append the fixture to that fence (STD-13 AC 4).
- WHEN `std_source` replaces the library in a test THEN the six constants SHALL be absent unless the replacement declares them (STD-15 AC 1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| STD-01 | P1: The specification says the library is source | Design | Pending |
| STD-02 | P1: The specification says the library is source | Design | Pending |
| STD-04 | P1: The POM builds the library as written | Design | Pending |
| STD-05 | P1: The POM builds the library as written | Design | Pending |
| STD-06 | P1: The POM builds the library as written | Design | Pending |
| STD-07 | P1: `std` ships | Design | Pending |
| STD-08 | P1: `std` ships | Design | Pending |
| STD-09 | P1: `std` ships | Design | Pending |
| STD-10 | P1: `std` ships | Design | Pending |
| STD-11 | P1: Part VII's fences are excerpts of `std` | Design | Pending |
| STD-12 | P1: Part VII's fences are excerpts of `std` | Design | Pending |
| STD-13 | P1: The galleries build clean | Design | Pending |
| STD-14 | P1: The galleries build clean | Design | Pending |
| STD-15 | P2: The built-in table shrinks | Design | Pending |
| STD-16 | P2: The built-in table shrinks | Design | Pending |
| STD-17 | P2: The built-in table shrinks | Design | Pending |
| STD-18 | P2: The MSL Book explains what is core and what is `std` | Design | Pending |

`STD-03` is not used: the constants' rule is STD-01 AC 6 and STD-08 AC 6.

**Coverage:** 17 requirements, 103 criteria, 0 mapped to tasks (no `tasks.md` yet).

---

## Sources

- `.specs/features/msl-audit/report.md`: decision D4, findings A-03 and A-42. What the feature must deliver.
- `.specs/STATE.md` MD-56 (D3, D4), MD-57 (spec edits and their guards), MD-59 (`std` embedded).
- `.specs/features/msl-packages/{context,spec,design}.md`: U1, U2, PKG-06. Where `std` lives and what the prelude is.
- `docs/spec/part_vii_stdlib.md`: the items, and §22 rules 1 to 5.
- `docs/spec/part_iv_behavior.md` §14: the core primitives, which stay out of `std`.
- `docs/spec/part_i_values.md` §1 rule 3, §3 rules 2 and 10, §3.4.2, §3.5 rules 4 and 9.
- The MSL Book, `https://claude.ai/artifact/6nvqf1rKtF3TteqjEv6z26`; its source at planning time: `<session scratchpad>/manual/build/{content.html,template.html,build.py}` (12 chapters, 53 code blocks).
- `docs/spec/appendix_g_scope.md` G.38, G.64. `docs/pom/resolution.md`, "The built-in table".

---

## Success Criteria

- [ ] `cargo test -p piperine-lang` passes, with the targets `std_excerpts` and `std_galleries` in it.
- [ ] 18 of 18 gallery fences are clean against the embedded library; the same fences against an empty prelude are not.
- [ ] A one-character edit of `std` or of a Part VII fence fails a guard that names the place.
- [ ] `tests/fixtures/pom/prelude.msl` is gone and no test count dropped.
- [ ] G.38 is `Closed` in Appendix G.
- [ ] `docs/book/` builds the Book with `python3 docs/book/build.py`, and the chapter "Core and standard library" names every core name and every `std` export.

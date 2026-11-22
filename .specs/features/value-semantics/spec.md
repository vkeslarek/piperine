# Value Semantics Specification

**Approved**: 2026-11-20 - user: "Topo!" to pure value semantics (no borrows, pointers or moves; every `let` reassignable), and, on D1, D2, D5 and D7, the recommended answers: "Não, são fixos", "Erro com dica", "Erro", "Genérica em N". D3, D4, D6 and D8 follow from those answers and are accepted with the plan.

## Decision (user, 2026-11-20)

"o nosso maior problema não é erros de borrowing nem nada parecido. O nosso maior problema é bugs que emergem e estrutura. Não vejo muito ganho termos pass by value e pass by reference juntos, borrowing, mutability, etc... Tudo isso custa muitas regras e muito esforço do compiler onde não tem tanto ganho." The user chose "Todo `let` é reatribuível" and, after weighing explicit pointers, "só valores, sem ponteiros" ("Topo!").

## Assumptions & Open Questions

The defaults taken where the decision left the choice open. Each waits for the user's confirmation.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1: `#[derive(Clone, Copy)]`: kept as words that do nothing, or removed | Removed. `Clone` and `Copy` leave Appendix H.8, so a derive or a bound naming one is `E0161` by the rule every other unknown name follows; the diagnostic's help says to delete it. `clone()` goes the same way | No special case (CC-11): an accepted no-op is a rule of its own, forever | y |
| D2: `let mut`: error, warning, or still parsed | Error. `mut`, `ref` and `move` become reserved words: `E0903` at the word, with a help naming the plain form. `E0331` is retired | The reserved-word rule already exists; the language has no warnings (G.0) | y |
| D3: Slices | None. An array is a value; a function over any length takes `const N: usize`; `e[a..b]` is the array `[T; b - a]` and its bounds are const expressions; `name @ ..` in an array pattern binds an array | One sequence type. A slice that is a value needs storage whose size the type does not give | y |
| D4: A method that changes a value: any shorthand | None. `self` is a copy; the method returns the new value; `s = s.step(x);` | The decision's own default | y |
| D5: Is a parameter a place | No: a parameter of a function or of a closure, `self` included, and a name a closure captures cannot be assigned (`E0183`). Every local can: a name bound by `let` or by the pattern of a `match` arm, `if let`, `while let` or `for` | `fn bump(self) { self.count += 1; }` would compile and change nothing. The user's stated problem is bugs that emerge | y |
| D6: Strings | `str` is a value type, the text of a literal. `&str`, `&'static str` and the lifetime `'static` go | Without it `&` survives in one type | y |
| D7: `resolve` of a nature, typed `fn(&[T]) -> T` | A function with one const parameter, `fn f<const N: usize>(drives: [T; N]) -> T`, instantiated with the number of drives of each call | Follows D3. It asks `generics` for const-generic functions and `resolved-nets` for the instantiation | y |
| D8: Codes | `E0101` covers every reference form, at parse. `E0102` and `E0331` are retired, never reused. `E0183` keeps "read before assigned" and "not a place" | The fewest codes | y |

**Open questions:** none blocks design. D1 to D8 are defaults for the user to confirm; the spec is not approved until they are.

## Problem Statement

The specification was amended on 2026-11-20 (Appendix G.71, decisions V1 to V7, commit `cc3599bf`): data are values and always copy, hardware is reached by identity, every local may be assigned, and the language has no reference, slice, move, `Clone`, `Copy`, `mut`, `ref` or `move`. The implementation is still the earlier language (`GAPS.md` S7): the parser accepts every removed form, `std` and `examples/buck` are written with them, `composite-values` has built `let mut` and part of the borrow rules, and three guards carry lists of what is ahead of the code (`msl_errors.rs`, `pom_resolve.rs`). This feature brings the front end, the IR, the generated Rust, `std`, the examples, the tests and the Book's status boxes to the amended text, and tells `composite-values` and `generics` which of their remaining tasks change.

## Goals

- [ ] A model written to the amended Part I parses, type-checks and runs wherever the same model ran with the earlier spelling.
- [ ] Every removed form is refused at parse with the code of its B.3.6 row and a help that names what to write.
- [ ] `crates/piperine-lang/std/*.msl`, `examples/`, the gallery fixtures, the test corpora and the Part VII fences are in the amended spelling, and no guard lists anything as ahead of the parser.
- [ ] The 7 ngspice twins print the same bytes as before; no tolerance and no contract changes.

## Out of Scope

| Item | Where it goes |
| ---- | ------------- |
| Running aggregates, derives and Appendix H.5 to H.7 by value | `composite-values` B8 to B10, with the task amendments of design.md "Changes to `composite-values`" |
| `self` methods, generic functions with `const N`, bounds | `generics`, with the amendments of design.md "Changes to `generics`" |
| Running a resolved nature (`Wire`, `resolve`) | `resolved-nets` (L21); this feature rewrites the `std` text only |
| The language server | deferred by the user; it is not built |
| New language forms (a struct-update form, an in-place method form) | none planned: D4 |

**Implicit-requirement dimensions.** Input validation: VS-01, VS-02 and VS-06 (every removed form has a diagnostic). Failure states: VS-03 AC 2, VS-05 AC 3 and 4. Persistence and state: N/A because a model keeps state only in carriers, which this feature does not touch. Concurrency and ordering: N/A because elaboration and a run are deterministic functions of the source. Auth, payments, external calls: N/A, a local compiler. Migration: VS-09 and VS-10 (every source in the tree moves to the amended spelling in the same feature that refuses the earlier one).

---

## User Stories

### P1: One kind of local, one way to pass data ⭐ MVP

**User Story**: As a model author, I want every `let` to be assignable and every value to be copied where I use it, so that I never choose between a value and a reference or mark a variable before I may change it.

**Why P1**: It is the decision itself.

**Acceptance Criteria** (VS-01, reference forms are refused):

1. IF a source writes a reference in a type (`&T`, `&mut T`, `&'static str`), in an expression (`&x`, `&&x`, `*x`), in a pattern (`&p`) or as a receiver (`&self`, `&mut self`) THEN the system SHALL report `E0101` at that form, once, with a help that says the value is passed itself.
2. IF a source writes a lifetime token, `'static` included, THEN the system SHALL report `E0905` at the token.
3. IF a source writes a slice type `[T]` THEN the system SHALL report a parse error at the type.
4. WHEN a source uses `&`, `&&` and `*` as binary operators THEN the system SHALL parse them as before.

**Acceptance Criteria** (VS-02, the markers are reserved words):

1. IF a source writes `mut` in a `let`, in a pattern or on a parameter, `ref` in a pattern, or `move` before a closure THEN the system SHALL report `E0903` at the word, once, with a help that names the plain form.
2. IF a module body writes `let mut` THEN the system SHALL report `E0903` and SHALL NOT report `E0331`.
3. The system SHALL raise `E0102` and `E0331` nowhere.

**Acceptance Criteria** (VS-03, places):

1. WHEN a body assigns a name bound by `let` (`x = e;`, every compound operator of §1.5, a field or an element of it) THEN the system SHALL type-check, elaborate and run it with the value the assignment gives, with no marker on the `let`.
2. IF an assignment targets a parameter of a function or of a closure, `self`, a name a closure captures, a body `let` of a module, a `const`, a param or a carrier THEN the system SHALL refuse it with `E0183` naming the target.
3. WHEN a body assigns a name bound by the pattern of a `match` arm, an `if let`, a `while let` or a `for` THEN the system SHALL accept it and change that binding only.
4. WHEN `let x;` is assigned more than once on a path THEN the system SHALL accept it; IF it is read before it is assigned on some path THEN the system SHALL refuse it with `E0183`.

**Acceptance Criteria** (VS-04, copies):

1. WHEN a local is assigned after a closure captured it THEN the closure SHALL keep the value it captured.
2. WHEN a local is passed to a function and assigned afterwards, or assigned inside the function through a `let` of the same value, THEN the caller's local SHALL keep its value.
3. WHEN a value of any type is read twice THEN the system SHALL accept both reads: no read is refused as a use after a move.

**Independent Test**: a reaction that sums `0..4` into `let s = 0;`, a function that takes `x: f64`, rebinds `let y = x; y += 1.0;` and returns `y`, and a closure captured before its local is assigned, run from source on the Rust backend against the same computation in Rust.

### P1: No trait for copying, no slice, a string that is a value

**User Story**: As a model author, I want to derive only what changes behavior and to pass a table or a text as it is, so that no declaration and no call carries a word that does nothing.

**Why P1**: These are the special cases the amendment removed; leaving them accepted keeps two languages alive.

**Acceptance Criteria** (VS-05, `Clone` and `Copy` are gone):

1. IF a derive or a bound names `Clone` or `Copy` THEN the system SHALL report `E0161` at the name, with a help that says every value is copied and the name is deleted.
2. WHEN an enum or a struct derives `Repr` without `Copy` THEN the system SHALL carry it, compare it and print it as it did with `Clone, Copy` in the derive.
3. IF a source calls `clone()` on a value THEN the system SHALL report `E0161` naming the method and the type.

**Acceptance Criteria** (VS-06, arrays and range indexes):

1. WHEN `e[a..b]`, `e[a..=b]`, `e[..b]`, `e[a..]` or `e[..]` indexes an array with const bounds THEN the system SHALL give it the type `[T; b - a]` of the elements `a` to `b - 1`.
2. IF a bound of a range index is not a const expression THEN the system SHALL refuse it with `E0188` naming the bound.
3. IF a const range index ends beyond the length or starts after its end THEN the system SHALL refuse it at elaboration.
4. WHEN an array pattern holds `name @ ..` THEN the system SHALL bind `name` to an array of the elements not written; IF a pattern has more sub-patterns than the array has elements THEN the system SHALL refuse it with `E0180`.
5. WHEN a function declared `fn f(xs: [f64; 3])` is called with an array THEN the system SHALL accept the array itself as the argument.

**Acceptance Criteria** (VS-07, `str`):

1. WHEN `str` is the type of a function parameter, a `const`, a local, the payload of an `Option` or a record field THEN the system SHALL accept a string literal there and SHALL compare two strings with `==` and `!=`.
2. WHEN the standard library's `log`, `fatal`, `require`, `cover` and `target` are called with string literals THEN the system SHALL emit the records it emitted before.

**Independent Test**: `#[derive(Repr, PartialEq)] enum Mode { Off, On }` carried on a `Bit<Mode>` and compared; `fn tag(name: str) -> bool { name == "a" }`; `const TAPS: [f64; 4]` with `TAPS[1..3]` passed to a `fn(xs: [f64; 2])`.

### P1: The tree is written in the language

**User Story**: As a maintainer, I want `std`, the examples, the fixtures, the tests and the Book to use only the amended spelling, so that the parser can refuse the earlier one and no guard carries an exception.

**Why P1**: The refusal of VS-01 and VS-02 cannot land while the tree uses what it refuses.

**Acceptance Criteria** (VS-08, the standard library and Part VII):

1. The system SHALL ship `std/*.msl` with no `&`, `*x`, `mut`, `Clone`, `Copy` or lifetime, and with `wired`, `tri` and `sum` taking `[T; N]` under a const parameter `N`.
2. WHEN `std` is built THEN the system SHALL report no diagnostic, and every `rust` fence of Part VII SHALL be consecutive lines of its `std` file.
3. The specification SHALL no longer say in §22 that `std` is in the earlier spelling.
4. WHEN `table_model`, `parse_table`, `parse_hex`, `matches_bits`, `logic`, `noise_table`, `laplace_zp` and `zi_zp` are used by a model that ran before THEN the system SHALL give the results it gave before.

**Acceptance Criteria** (VS-09, examples, fixtures and tests):

1. The system SHALL hold no `.msl` file under `examples/` and `crates/*/tests/` that uses a removed form, other than a test that asserts its refusal.
2. WHEN `examples/buck` and `examples/rc` run THEN the system SHALL print what they printed before.
3. WHEN `piperine fmt` formats a source THEN the system SHALL print no `mut`, `ref`, `move`, `&` borrow or `*` dereference, and the object model SHALL expose no field for them.

**Acceptance Criteria** (VS-10, guards, records and the Book):

1. The test targets SHALL hold no list of specification rows or codes that are ahead of the parser or of the built-in table.
2. `GAPS.md` SHALL no longer hold S7, and Appendix G.71 SHALL no longer say that the reference implementation does not meet it.
3. WHEN the Book's chapters 2 and 3 describe a removed spelling THEN they SHALL state the code the binary reports for it (`E0903`, `E0101`, `E0161`), each held by a test that runs the binary's front end.

### P1: Nothing else moves

**User Story**: As a maintainer, I want the numbers and the generated code of every existing model unchanged, so that this feature changes spelling and rules, never results.

**Acceptance Criteria** (VS-11, parity and generated code):

1. WHEN the parity targets run THEN the 7 twins SHALL print the bytes they printed before, with `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` unedited.
2. WHEN the transpile goldens are regenerated THEN they SHALL be byte-identical: a local the body never assigns is printed without `mut`.
3. WHEN every buck target runs THEN it SHALL pass.

**Acceptance Criteria** (VS-12, the IR holds no slice):

1. The IR SHALL hold no slice type and no slice expression, and `Ir::check` SHALL have no rule for them.
2. WHEN a local is assigned by its body THEN its IR local SHALL be marked assignable, and otherwise not, whatever the source wrote.

---

## Edge Cases

- WHEN a name is bound by `for k in 0..3` and assigned in the body THEN the next iteration SHALL start from the range's next element, not from the assigned value.
- WHEN a `let` of the same name follows an assigned local THEN it SHALL declare a new variable (§1.7.3), which may have another type.
- IF `mut` is used as an identifier (`let mut = 1;`) THEN the system SHALL report `E0903`.
- WHEN a struct has a field of type `str` THEN the system SHALL accept the type and SHALL refuse `#[derive(Repr)]` on it with `E0110`.
- WHEN `a & b` is written with `b` in parentheses or as a unary expression (`a & -b`, `a && !b`) THEN the system SHALL parse a binary operator.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| VS-01 | P1: one kind of local, one way to pass data | - | Pending |
| VS-02 | P1: one kind of local, one way to pass data | - | Pending |
| VS-03 | P1: one kind of local, one way to pass data | - | Pending |
| VS-04 | P1: one kind of local, one way to pass data | - | Pending |
| VS-05 | P1: no trait for copying, no slice, a string that is a value | - | Pending |
| VS-06 | P1: no trait for copying, no slice, a string that is a value | - | Pending |
| VS-07 | P1: no trait for copying, no slice, a string that is a value | - | Pending |
| VS-08 | P1: the tree is written in the language | - | Pending |
| VS-09 | P1: the tree is written in the language | - | Pending |
| VS-10 | P1: the tree is written in the language | - | Pending |
| VS-11 | P1: nothing else moves | - | Pending |
| VS-12 | P1: nothing else moves | - | Pending |

**Coverage:** 12 total, 12 mapped to tasks (tasks.md).

---

## Sources

- The user, 2026-11-20 (the decision quoted at the top).
- `docs/spec/` at `cc3599bf`: `part_i_values.md` §1 rule 2, §1.2.5 to §1.2.9, §1.4.6, §1.4.7, §1.5.9, §1.5.12, §1.5.14, §1.6, §1.7.5, §1.7.7, §3.1.10; `part_ii_carriers.md` §4 rule 8; `part_iii_structure.md` §8.2.12; `appendix_a_lexical.md` A.1, A.6; `appendix_b_grammar.md` B.3.6, B.3.7; `appendix_h_library.md` H.4 to H.9; `appendix_g_scope.md` G.71, V1 to V7.
- `GAPS.md` S7.
- `.specs/features/composite-values/` (`spec.md` CV-01, CV-12 to CV-15; `tasks.md` T11, T19, T42 to T45, T51 to T53) on `feature/composite-values` at `42d183db`, mid-batch B7.
- `.specs/features/generics/` on `origin/feature/generics` at `a9ac90db` (`tasks.md` T16, T19, T25, T30, T35).
- `.specs/features/code-clarity/spec.md` (the user's cleanliness rules, CC-11).

---

## Success Criteria

- [ ] Every Independent Test runs from source on the Rust backend with Rust's values.
- [ ] `grep` finds no removed form in `std`, `examples` or a fixture, and no "ahead of the parser" list in a test.
- [ ] The 7 twins' bytes, the transpile goldens and every buck target are unchanged.

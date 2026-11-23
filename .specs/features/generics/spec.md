# Generics Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 (X6) | What does `piperine check` owe a generic item: is its body checked once against its bounds, or only per instantiation? §3 rule 9 says "instantiated at elaboration. An error inside an instantiation is an elaboration error", which reads as the latter; §0.4 says generics are Rust's, which check against bounds. S3 depends on the answer | (a) **Once, against the bounds (Rust).** A generic body is type-checked at its definition with each type parameter opaque: it may use only what its bounds give (`T: Repr` gives `WIDTH`, `level`, `encode`, `decode`; `T: PartialOrd` gives `<`). A library's error is reported where it is written, even if nothing instantiates it. Const-generic arithmetic (`[f64; N + 1]` against `[f64; D]`) must then be reasoned about symbolically, which Rust itself leaves unstable. (b) **Per instantiation only (templates).** A generic body is resolved at its definition and type-checked only when instantiated. Simpler. A library's generic item with an error passes `check` until a user instantiates it, and the error is reported at the user's use. (c) **Both, split as Rust's stable compiler does.** Types, methods and trait use are checked once against the bounds at the definition; const-generic arithmetic, array-length equality and every `where` are checked per instantiation | (c). A library is checked where it is written, as Rust users expect, without symbolic const arithmetic. §3 rule 9 gains one sentence saying which errors belong to the definition and which to an instantiation. The stdlib must then state every bound its bodies use (e.g. `tri<T: Repr>` comparing `Option<T>` needs `T: PartialEq`); the audit is part of phase A |

## Decision (user, 2026-11-22): a module type takes trait impls — "looks like a Mosfet, behaves like a Mosfet"

From an exercise on swapping transistor models (BSIM3, BSIM4, MOS1 behind one circuit). `Fn(Electrical, Electrical, Electrical, Electrical)` alone lets any four-terminal module fit a MOSFET slot, which the user wants to avoid, and the user refused new concepts for it (no `interface`, no named signature types: "Tudo isso é conceitos a mais em uma linguagem que já é razoavelmente complexa"). The user chose, verbatim: "Gostei do 2. Diz basicamente: Looks like a Mosfet, behaves like a Mosfet".

- A template's type is its module's name (§8.4 rule 3), so a trait may be implemented for it: `trait Mosfet { fn polarity() -> Polarity; }`, `impl Mosfet for Bsim4 { .. }`. This extends "traits are capabilities" (MD-78, natures take trait impls) to modules; no new keyword.
- A slot is bounded by both: `module DiffPair<N: Mosfet + Fn(Electrical, Electrical, Electrical, Electrical)>(.., nmos: N = nch(180n, 2u))`. `Fn(P)` checks the shape (§8.4.5); the trait is nominal (only a module that declares `impl Mosfet` fits).
- Each model is its own module with its own card (`Bsim4 { card: Bsim4Card, l, w }`); binning stays an ordinary function returning a template (§8.4.7).
- What an `impl` on a module may do (user, 2026-11-22: "Ok, pode registrar"): the `impl` is on the template, an elaboration value (§8.4.6, §8.4.9), so its functions are pure functions of the bound params. `self` is the template: `fn vth(self) -> f64 { self.card.vth0 }`. They may read params, compute and return values or another template (binning); they may NOT read nodes, quantities or time — an instance's state stays reachable through `inst.name` on its `pub` items (§8.4.8). Called at elaboration (`let vgs = nmos.vth() + 0.2;`). An inherent `impl Bsim4 { fn .. }` with no trait is allowed too, like on any type.
- To do when this feature reaches templates (GN-21/GN-23): amend the spec (§3.7 which `impl Tr for` targets are allowed: add a module type; §8.4: a bound may combine a trait and `Fn(P)`), plan the tasks, and a Book example (the swappable MOSFET). The `ngspice-models` idea uses this pattern for its device models.

## Problem Statement

Generic items are refused with `P0001` (`GAPS.md` L7, L30): generic functions, `self` methods, `impl` on value types, user traits, generic modules and const generics. With them go handles (L8), generation `for`/`if` (L31), templates and `Fn(P)` bounds (L32), and the implicit params `m` and `dtemp` (L29). The prelude's own `Short<N>` is refused, and with it a third of the stdlib (L58: `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `merge`, `cast`, `same`). Separately, the type checker checks only what a run's top reaches (S3): a library with no run is never type-checked, so `piperine check` is green for code that cannot compile. The user asked for every gap closed, those that limit the language's expressiveness first.

## Goals

- [ ] Generic functions, methods, `impl` on value types and natures, user traits and generic value types type-check, elaborate and run on the Rust backend, as Rust (§0.4) and §3, §3.1 say.
- [ ] Generic modules with type, nature and const parameters, generation `for` and `if`, templates and `Fn(P)` bounds, `m` and `dtemp` elaborate and run as Part III §8.2.7, §8.4 and §8.5 say.
- [ ] `piperine check` type-checks every item of every crate, reached or not, and a library package with no run.
- [ ] Every stdlib item that L58 refuses for generics either runs or is refused by the name of the gap that still blocks it, never by "generic".
- [ ] Each of L7, L8, L29 to L32, L58's generics group, S3 and X6 leaves `GAPS.md` in the commit that closes it.
- [ ] The 7 ngspice twins print the same bytes as before.

## Phases

Five sub-features, each designed and tasked on its own (`design-<phase>.md`, `tasks-<phase>.md` in this folder).

| Phase | Name | Ids | Requirements | Size |
| ----- | ---- | --- | ------------ | ---- |
| A | Generic items and the whole-model check | L7, L4 (generic enums), S3, X6 | GN-01 to GN-09 | L, about 35 tasks |
| B | Handles | L8 | GN-10 to GN-12 | M, about 12 tasks |
| C | Generic modules and generation | L30, L31 | GN-13 to GN-20 | L, about 32 tasks |
| D | Templates and implicit params | L32, L29 | GN-21 to GN-26 | M, about 20 tasks |
| E | The stdlib built on generics | L58 (generics group) | GN-27 to GN-28 | M, about 15 tasks |
| all | Cross-cutting | — | GN-29 to GN-31 | carried by each phase |

Total: L, about 114 tasks.

## Dependencies

| On | What it gives this feature | Needed by |
| -- | -------------------------- | --------- |
| `composite-values` phase A | loops, `let mut`, closures, recursion | stdlib bodies (`zi_nd`, `poly_q`); generation bodies |
| `composite-values` phase B | structs, enums with payload, arrays, tuples | generic value types (GN-06), hand-written `impl Repr` (`[bool; WIDTH]`, GN-05), const generics over arrays (GN-14) |
| `composite-values` phase C | arrays of carriers | generated arrays of signals and instances (GN-17), `[Bit; ROWS]` ports, `zi_nd`, `laplace_nd` |
| `params-and-io` | params of module type (L28) | templates as params, `cell: C` (GN-21, GN-23); a `u32` param (`AckWithin`) |
| `analog-structure` | `inst.name` on a child's `pub` item (L36); a node in a result (L35); a contribution under `if` (L40) | `bank.cells[3][5].sn` (GN-23 AC 3); `idt`, `idtmod`, `laplace_*` (GN-28) |
| `analog-operators` | `delay` (L44), `noise` (L45) | `zi_*`, `noise_table`; the noise rows of `m` (GN-24) |
| `unknown-semantics` | `Unknown`, `level` (L18), an event read as a `bool` (L51), `settle` (L52), a function returning `Event` (L53) | `merge`, `cast`, `same`, `Stable`, `Hold`, `zi_*`; L53 is "with L8" |
| `host-ii` | a derived param re-derived on a `set` (S2, X9); the host code `E0803` for a structural set (H9) | GN-20 AC 3, GN-26 |
| `msl-fixes` (in flight) | `Option` methods (L16 H.3/H.4), `rem_euclid` (L15/L14) | `idt`, `idtmod` |

Order inside the two features: CV-A, CV-B, GN-A, GN-B, CV-C, GN-C, GN-D, GN-E (`composite-values` spec, "Dependencies"). GN-A's generic functions over scalars do not wait for CV-B; its generic value types do.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Bundles and generic bundles (§7 rule 11) | `resolved-nets` (L25); it reuses this feature's instantiation |
| Params of integer, enum, `Option` or module type | `params-and-io` (L28); templates need them and wait for them |
| `inst.name` on a child's `pub` item | `analog-structure` (L36) |
| `extern` modules, which `m` and `dtemp` also reach | `foreign-models` (L33) |
| `dyn`, `impl Trait`, associated types, trait-bound `where` | removed by §1 rule 2 and B.3.6 (D1); they stay parse errors |
| A function returning `Event` | `unknown-semantics` (L53), on top of this feature's handles |
| The host codes `E0803` to `E0806` | `host-ii` (H9). This feature refuses a structural set through the path that exists and adopts `E0803` when `host-ii` adds it |
| Extending the interpreter | R1: it refuses each new form by name (GN-29) |
| The spec codes of `GAPS.md` §10 for constructs that already run | `params-and-io`. Codes that only a construct of this feature can raise (`E0127`, `E0130` to `E0134`, `E0304`, `E0340` to `E0342`, `E0360` to `E0362`, `E0375`) are raised here |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Check of a generic body | open: Q1 (X6) | §3 rule 9 and §0.4 read two ways | n - user |
| Check of a non-generic unreached item | every check that needs no param or generic value runs on every item (types, names, classes, placement, `E0121`, `E0122`, `E0350`); checks that need a value (`where` on a default, generation, unrolling, `E0126`) run per instance | A module with required params cannot be elaborated without them; §0.3 puts every static check in elaboration "of the whole model", and the value-free checks are the ones that can be | n - planner |
| `piperine run` and `build` with an error in an unreached item | fail with the same diagnostic as `check` | Rust's rule: a crate that does not type-check does not build. S3's impact is a green `check` for a library that cannot compile | n - planner |
| Instantiation | one IR body per distinct instantiation of a module (codegen S6) and one IR function per distinct instantiation of a function (S7); the POM keeps the generic item and the placement as written | The navigability rule (CLAUDE.md, MD-25); MD-64's hoisting already names variants in the IR only | y - MD-25, S6, S7 |
| Inference of generic arguments | as Rust: from the arguments and the expected type; a nature parameter from the natures of the connected carriers (`Short(a, b)`); a const from an array argument's length | §0.4; `Short(a, b)` and `since(d)` are written without arguments in the stdlib | n - planner |
| Trait coherence | Rust's: one `impl` of a trait per type, a blanket `impl<T: Bound> Tr for T` allowed in the crate that declares the trait | `impl<T: Repr> Known for T {}` in `std/values.msl` | n - planner |
| Structural params | a param read by generation, a const generic argument, an array length or a template is structural (§8.1.2): an override before a run is an elaboration input; a `set` or a sweep of it during a run is refused | §8.1.2, MD-18 | y - spec |
| `m` on a host `set` | effective multiplicity follows the rule `host-ii` sets for derived params (X9) | `m` is inherited down the path (§8.5.3); a child's effective `m` is a derived value | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: GN-01 AC 4 (`where` per instantiation, `E0127`), GN-13 AC 4 (`E0304`), GN-17 AC 3 (`E0340`), GN-18 AC 3 (`E0341`). Failure states: refusal by name while a dependency is open (GN-27 AC 2, GN-29). Idempotency and retry: N/A, elaboration is a deterministic function of the source and the overrides. Auth and rate limits: N/A, a local tool. Concurrency and ordering: GN-19 AC 2 (generated reactions keep source order, §12.1.14). Data lifecycle: N/A, no state outlives a session. Observability: GN-15 AC 2 (variant names in the IR and generated Rust); GN-08 (diagnostics for unreached items). External-dependency failure: N/A. State-transition integrity: GN-20 (a structural param cannot change during a run).

---

## User Stories

### P1: Generic functions, methods and traits (phase A) ⭐ MVP

**User Story**: As a model author, I want generic functions, methods with `self`, `impl` blocks on my value types and my own traits, so that I write a routine once for every type it fits.

**Why P1**: L7 is the root of L30, L32 and most of L58; the stdlib's `Known` is a trait with a blanket `impl`.

**Acceptance Criteria** (GN-01, generic functions):

1. WHEN a function has type parameters with bounds (an H.8 trait, `Repr`, a project's trait, `Fn(A, ..) -> R`) or const parameters, and is called with explicit arguments (`f::<u8>(x)`) or inferred ones THEN the system SHALL instantiate and run it with the meaning Rust gives.
2. WHEN a generic function takes a parameter `f: F` with `F: Fn(f64) -> f64` THEN the system SHALL accept a capturing closure, a non-capturing closure or a function name for it, and call it positionally (§3.2.11).
3. IF generic arguments cannot be inferred THEN the system SHALL refuse the call naming the parameter that is not inferred.
4. IF a function's `where` is false for an instantiation THEN the system SHALL refuse it with `E0127` naming the instantiation.

**Acceptance Criteria** (GN-02, instantiation):

1. The system SHALL make one IR function per distinct instantiation of a generic function, named deterministically from the function and its arguments.
2. The system SHALL keep the generic function in the POM as written, with no instantiation added to `Design`.

**Acceptance Criteria** (GN-03, `impl` on a value type):

1. WHEN an `impl` on a struct or an enum declares methods with `self`, associated functions (`Point::new`) and associated constants THEN the system SHALL accept method-call syntax, `Self`, and `Type::name` paths with Rust's meaning.
2. IF an `impl` is written on a module THEN the system SHALL refuse it with `E0130`.
3. IF a method is named `changed`, `prev`, `others` or `inflow` THEN the system SHALL refuse it with `E0131`.

**Acceptance Criteria** (GN-04, user traits):

1. WHEN a project declares a trait with method signatures, default bodies and associated constants, and implements it with `impl Trait for Type` THEN the system SHALL call the implementation statically, the default body when the `impl` does not override it.
2. WHEN a generic body calls a method of a bound (`x.known()` under `T: Known`) THEN the system SHALL call the instantiation's implementation.
3. WHEN a crate declares a blanket `impl<T: Bound> Trait for T` THEN the system SHALL apply it to every type that satisfies the bound.
4. IF two `impl`s of one trait apply to one type, or an `impl` leaves a method without a default unimplemented THEN the system SHALL refuse it naming the trait and the type.
5. IF `impl Trait for` names a bundle THEN the system SHALL refuse it with `E0134`; a nature takes trait impls (traits are capabilities, MD-78).

**Acceptance Criteria** (GN-05, hand-written `Repr`):

1. WHEN a project writes `impl Repr for T` with `WIDTH`, `encode` and `decode` (a one-hot or Gray encoding) THEN the system SHALL carry `Bit<T>` with that layout.
2. WHEN a hand-written `decode` returns `None` THEN the system SHALL return the `None` to the caller (§2 rule 9 as MD-78 amends it).

**Acceptance Criteria** (GN-06, generic value types):

1. WHEN a struct or an enum has type or const parameters (`enum Maybe<T> { No, Yes(T) }`, `struct Pair<T> { a: T, b: T }`) THEN the system SHALL instantiate it per argument list and accept every construct of `composite-values` phase B on each instance.
2. WHEN a generic type derives `Repr` and is instantiated with an argument that has `Repr` THEN the system SHALL lay it out per §2, and IF the argument has no `Repr` THEN the system SHALL refuse it with `E0110`.

**Acceptance Criteria** (GN-07, the check of a generic body, X6):

1. The system SHALL check a generic body in the way Q1 chooses.
2. WHERE Q1 is answered (a) or (c), IF a generic body uses a method or operator that none of its bounds gives THEN `piperine check` SHALL refuse it at the definition, naming the missing bound, with nothing instantiating it.
3. WHERE Q1 is answered (b) or (c), IF an instantiation makes a `where`, an array length or a const expression invalid THEN the system SHALL report the error at the instantiation, naming the item and its generic arguments.
4. WHEN Q1 is answered THEN §3 rule 9 SHALL say which errors belong to a definition and which to an instantiation, in the same change that implements it.

**Acceptance Criteria** (GN-08, the whole-model check, S3):

1. WHEN `piperine check` runs on a package THEN the system SHALL type-check every function, module, behavior block, `const` and `impl` of every crate of the package and of its dependencies, reached from a run's top or not.
2. WHEN a library package with no `[run]` table is checked THEN the system SHALL type-check all of it and report each error with the code it has when reached.
3. WHEN `fn broken(x: f64) -> bool { x + 1.0 }` is in a crate and nothing calls it THEN `piperine check` SHALL fail naming it.
4. WHEN a module that nothing places calls `ddt` from a function THEN `piperine check` SHALL fail with `E0350`.

**Acceptance Criteria** (GN-09, runs see the same errors):

1. WHEN a crate of a package has a check error in an unreached item THEN `piperine run` and `piperine build` SHALL fail with the same diagnostic as `piperine check`.

**Independent Test**: `piperine-lang` tests per form; a library-only fixture package with one error per item kind, checked with `piperine check`; transpile tests of a generic function instantiated at `u8`, `f64` and a struct, against Rust-evaluated expected values.

---

### P1: Handles (phase B) ⭐ MVP

**User Story**: As a model author, I want functions and `impl` methods that take a carrier or a node and read or probe it, so that `t.celsius()` and `fn diff(a: Electrical, b: Electrical) -> f64 { v(a) - v(b) }` work.

**Why P1**: §3 rule 7 makes a handle the only way a function reads a carrier; L53 (`Event` functions) builds on it.

**Acceptance Criteria** (GN-10, `impl` on a nature):

1. WHEN a project writes `impl N { fn m(self) .. }` on a nature it declares or on a prelude nature (`Thermal`, `Electrical`, `Bit<T>`) THEN the system SHALL accept `x.m()` on a carrier of that nature, with `self` a handle (§3.1.1).
2. WHEN a method call is made on a carrier expression THEN the system SHALL look it up in the order of §3.1.7: carrier methods, then the nature's `impl`s, then the methods of the value read.
3. IF an `impl` on a nature holds an item that is not a function THEN the system SHALL refuse it with `E0132`.

**Acceptance Criteria** (GN-11, handle parameters):

1. WHEN a function parameter has a nature type THEN the system SHALL bind it at elaboration to the carrier passed, and the body SHALL read it, probe it (`v(a)`, `temp(self)`), pass it on, and call carrier methods on it.
2. IF a handle is returned, stored in a value, written through, or committed to THEN the system SHALL refuse it with `E0133`.
3. WHEN a handle of a discrete carrier is read THEN the system SHALL give the carrier's value with its flags (§3.1.6).
4. WHEN a function parameter is generic over a nature (`fn f<N: Nature>(a: N)`) THEN the system SHALL instantiate it per nature.

**Acceptance Criteria** (GN-12, function classes):

1. WHEN a function probes a handle or reads the solve's environment THEN the system SHALL class it solve-reading (§3), and accept its calls in `analog`, event expressions, reaction bodies, body `let`s and value inputs.
2. IF a solve-reading function is called where that class is not allowed THEN the system SHALL refuse the call with `E0122`.
3. The system SHALL no longer report `E0450` for a probe in a function body whose probed carrier is a handle.

**Independent Test**: `impl Thermal { fn celsius(self) -> f64 }` of §3.1 run in a thermal circuit against `temp - 273.15`; a probing function called from `analog` and from a reaction.

---

### P1: Generic modules and generation (phase C) ⭐ MVP

**User Story**: As a designer, I want modules generic over types, natures and constants, and `for`/`if` generation in a body and in a `digital` block, so that a ladder of `N` sections, a `Short` of any nature or a monitor over any carried type is written once.

**Why P1**: L30 and L31 are the prelude's own `Short<N>`, the Book's ladder and every stdlib module of L58.

**Acceptance Criteria** (GN-13, generic modules):

1. WHEN a module declares type parameters (`T: Repr`), nature parameters with or without a default (`S: Nature = State`) and const parameters THEN the system SHALL elaborate each placement as the instantiation its arguments, explicit or inferred, give.
2. WHEN a behavior block repeats its module's generic names (`analog<S> for idt<S>`) THEN the system SHALL bind them to the instantiation's arguments (§10 rule 6, D6).
3. IF a behavior block's generic names differ from its module's in name, number or order THEN the system SHALL refuse it naming both lists.
4. IF the `where` after a module's port list is false or not a `bool` const expression THEN the system SHALL refuse it with `E0304`.

**Acceptance Criteria** (GN-14, const expressions in types):

1. WHEN a type or a const argument is a const expression (`UInt<{A + 1}>`, `[f64; N + 1]`, `[State; D - 1]`, `Int<-1>` written `Int< -1 >`) THEN the system SHALL evaluate it at elaboration.
2. WHEN a `const fn` is called in a const context (`poly_q(&zeros)` as a param value) THEN the system SHALL evaluate it at elaboration with every form `composite-values` accepts.

**Acceptance Criteria** (GN-15, instantiation of modules):

1. The system SHALL make one IR body per distinct instantiation of a generic module (codegen S6) and keep the generic module and each placement in the POM as written.
2. The system SHALL name each instantiation deterministically in the IR, in the generated Rust and in the device library's module list.

**Acceptance Criteria** (GN-16, `Short`):

1. WHEN `Short(a, b)` connects two nodes of one continuous nature (`Electrical`, `Thermal`, a project's nature) THEN the system SHALL merge them at elaboration, giving the same solution as one node.
2. IF the two nodes have different natures THEN the system SHALL refuse the placement naming both natures.

**Acceptance Criteria** (GN-17, generation in a body):

1. WHEN a module body holds `for k in range { items }` with a const range THEN the system SHALL elaborate the items once per iteration, each named item an array indexed by iteration ordinal from 0 (§8.2.23), nested loops nesting the array.
2. WHEN a module body holds `if c { .. } else if .. { .. } else { .. }` with const conditions THEN the system SHALL elaborate the taken arm only.
3. IF a generation range or condition is not a const expression THEN the system SHALL refuse it with `E0340`.
4. IF an item of an arm that was not elaborated is referenced, or a generated array is indexed out of range THEN the system SHALL refuse it with `E0342`.
5. IF a param is declared inside generation THEN the system SHALL refuse it with `E0322`.

**Acceptance Criteria** (GN-18, recursion by generation):

1. WHEN a module places itself under a generation `if` whose condition ends the recursion THEN the system SHALL unroll it at elaboration.
2. WHEN that recursion is the Book's RC ladder of `N` sections THEN the system SHALL give the same solution as the ladder written out.
3. IF a generation recursion does not end THEN the system SHALL refuse it with `E0341`.

**Acceptance Criteria** (GN-19, generation in a `digital` block):

1. WHEN a `digital` block holds `for k in 0..N { on lines[k].rise() { .. } }` THEN the system SHALL make one reaction per iteration (§12 rule 1).
2. The system SHALL keep generated reactions in iteration order within source order (§12.1.2, §12.1.14), each a distinct statement.

**Acceptance Criteria** (GN-20, structural params):

1. The system SHALL classify as structural every param read by generation, a const generic argument, an array length or a template (§8.1.2).
2. WHEN a host overrides a structural param before a run THEN the system SHALL elaborate with the overridden value.
3. IF a host sets or sweeps a structural param during a run THEN the system SHALL refuse it with a diagnostic naming the param as structural (`E0803` once `host-ii` adds it).

**Independent Test**: `Short` across `Electrical` and `Thermal`; the Book's ladder at `N = 1, 3, 8` against written-out netlists; a generated bank of `N` flip-flops reacting per element; a structural override from `--param`.

---

### P1: Templates and implicit params (phase D) ⭐ MVP

**User Story**: As a designer, I want a module with its params bound as a value, placement through a template or a binning function, `Fn(P)` bounds, and `m` and `dtemp` on every instance, so that a memory bank takes its cell as a param and a device scales by multiplicity.

**Why P1**: §8.4 and §8.5.1 to §8.5.6 run at 0 % today (`GAPS.md` §9).

**Acceptance Criteria** (GN-21, templates):

1. WHEN `Name { params }` or `Name::<G> { params }` is written as a `const`, a param value or a function result THEN the system SHALL give a value of type `Name` (or `Name<G>`) with those params bound.
2. IF a template leaves a required param unbound THEN the system SHALL refuse it with `E0319`.
3. IF a template-valued expression is not a const expression THEN the system SHALL refuse it with `E0362`.
4. The system SHALL NOT carry a template (§8.4.1), refusing it as a carried type with `E0112`.

**Acceptance Criteria** (GN-22, placement through a template or a function):

1. WHEN a placement's head is a template value (`cell(conns)`, `nch(l, w)(d, g, s, b)`) THEN the system SHALL place an instance identical to `Name(conns) { params }` (§8.4.9).
2. IF such a placement carries a param block THEN the system SHALL refuse it with `E0361`.
3. WHEN a binning function returns a template chosen by its arguments THEN the system SHALL place the chosen module with the chosen params.

**Acceptance Criteria** (GN-23, `Fn(P)` bounds):

1. WHEN a generic module has `C: Fn(P)` and a template of a module whose port types and directions are `P`, in order, is bound to it THEN the system SHALL accept it, port names not counting (§8.4.5).
2. IF the bound module's ports do not match `P` THEN the system SHALL refuse it with `E0360` naming the first port that differs.
3. WHEN `inst.name` reads an item of an instance placed through a template-typed param THEN the system SHALL resolve it against the concrete module (§8.4.8), with `E0343` for a missing or private item.

**Acceptance Criteria** (GN-24, `m`):

1. WHEN a placement or a template binds `m` THEN the system SHALL accept it on every instance, and IF `m <= 0.0` THEN the system SHALL refuse it with `E0317`.
2. The system SHALL give an instance an effective multiplicity `M` equal to the product of `m` along its path (§8.5.3), and the name `m` inside a module SHALL read `M`.
3. WHEN an instance has effective multiplicity `M` THEN the system SHALL multiply its flow contributions by `M`, divide its flow probes by `M`, leave potential contributions and potential probes unchanged, and leave discrete carriers, events and reactions unchanged (§8.5.4).
4. WHERE `analog-operators` has landed `noise`, WHEN an instance with multiplicity `M` contributes noise THEN the system SHALL multiply noise power on a flow by `M` and divide noise power on a potential by `M`.
5. WHEN a resistor placed with `m: 4.0` is in a circuit THEN the system SHALL give the same solution as four of them in parallel.

**Acceptance Criteria** (GN-25, `dtemp`):

1. WHEN a placement or a template binds `dtemp` THEN the system SHALL give the instance an effective offset equal to the sum of `dtemp` along its path, and `temperature()` inside it SHALL return the analysis temperature plus that offset (§8.5.5).
2. The system SHALL let a module read the name `dtemp` as its effective offset.
3. IF a module declares a port, param, signal or `let` named `m` or `dtemp` THEN the system SHALL refuse it with `E0375`.

**Acceptance Criteria** (GN-26, a host and the implicit params):

1. WHEN a host sets `m` or `dtemp` of an instance THEN the system SHALL treat it as a param change (§8.5.6), the effective values of its descendants following the derived-param rule of `host-ii` (X9).
2. WHEN a host lists an instance's params THEN the system SHALL include `m` and `dtemp`.

**Independent Test**: the `MemoryBank`/`Dram1T1C` pair of §8.4 elaborated (once `params-and-io` and `resolved-nets` give module-typed params and the `Cell` bundle; meanwhile the same shape over `Electrical` ports); `nch` binning; a resistor with `m` against parallel copies; a diode with `dtemp` against the same diode at a higher temperature.

---

### P2: The stdlib built on generics (phase E)

**User Story**: As a user of the standard library, I want every item refused today "because generic" to run, or to be refused by the name of what still blocks it, so that the refusal tells me which feature to wait for.

**Why P2**: The machinery of A to D is the P1; this phase proves it on the stdlib and records what other features still owe.

**Acceptance Criteria** (GN-27, every item of L58's generics group):

1. WHEN a model places or calls `Short`, `since`, `idt`, `idtmod`, `zi_nd`, `zi_np`, `zi_zd`, `zi_zp`, `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `merge`, `cast`, `same` or `MinPulse` THEN the system SHALL no longer refuse it because it is generic.
2. IF one of them uses a form that another feature has not landed THEN the system SHALL refuse it naming that form, and `GAPS.md` SHALL list the item under that form's id.
3. The system SHALL type-check each of them under GN-07 with no instantiation, and with no error.

**Acceptance Criteria** (GN-28, the items that run after this feature):

1. WHEN `since`, `MinPulse`, `Setup`, `inertial` and `Short` are placed THEN each SHALL run and give the values its §22 entry gives, tested against them.
2. WHEN another feature lands the last form an item of GN-27 needs THEN the same change SHALL add that item's test against its §22 entry.

The forms each item is expected to need beyond this feature, from its source in `std/` (to be verified at design):

| Item | Also needs |
| ---- | ---------- |
| `Short`, `since`, `MinPulse`, `Setup`, `inertial` | nothing |
| `Stable`, `Hold` | an event read as a `bool` (L51) |
| `idt`, `idtmod` | a node in a result (L35), a contribution under `if` (L40), `Option` methods (`msl-fixes`) |
| `zi_nd`, `zi_np`, `zi_zd`, `zi_zp` | `delay` (L44), `settle` (L52), a contribution under `if` (L40), `composite-values` A to C |
| `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp` | a contribution under `if` (L40), `composite-values` A to C |
| `noise_table` | `noise` (L45), `table_model` (L57), `freq` (`msl-fixes`) |
| `merge`, `cast`, `same` | `Unknown` and `level` (L18) |

**Independent Test**: one `piperine-api` test per runnable item against its §22 entry; one refusal test per blocked item asserting the blocking form's name.

---

### P1: Every phase keeps the rest intact

**User Story**: As the maintainer, I want each phase to land without moving a number and to say plainly what it still refuses.

**Why P1**: Parity and the refusal-by-name rule are the project's guarantees.

**Acceptance Criteria** (GN-29, refusal by name):

1. WHEN the interpreter backend meets a generic instantiation, a handle, generation, a template or `m`/`dtemp` other than 1 and 0 THEN the system SHALL refuse it by name (R1).
2. WHILE a phase has not landed, IF a model uses one of its forms THEN the system SHALL keep refusing it with `P0001` naming the form.

**Acceptance Criteria** (GN-30, no regression):

1. WHEN each phase merges THEN the 7 ngspice twins SHALL print the bytes they printed before it, and the parity suites SHALL pass with `ToleranceContract` unedited.
2. WHEN each phase merges THEN `cargo build` SHALL give no warning and `cargo test` SHALL pass.

**Acceptance Criteria** (GN-31, bookkeeping):

1. WHEN a phase closes a `GAPS.md` id THEN the same commit SHALL remove it, update `§9 Coverage`, and move each L58 item it unblocks to the id of what still blocks it.
2. WHEN a phase lets a gallery fence (Appendices D to F) or a Book example check that was refused before THEN the system SHALL hold it to the checker in the existing gallery or Book test.

**Independent Test**: `cargo test` with the twins' goldens; the interpreter's refusal test; `GAPS.md` in each closing commit.

---

## Edge Cases

- WHEN a generic item is instantiated twice with the same arguments THEN the system SHALL make one instantiation.
- WHEN a const generic is 0 (`zi_nd::<0, 1>`, `[f64; 0]`) THEN the system SHALL elaborate it, its loops running zero times.
- WHEN a generation `for` has an empty range THEN the system SHALL make an empty array, and an index into it SHALL be `E0342`.
- WHEN a nature parameter has a default and nothing else fixes it (`idt(x)`) THEN the system SHALL use the default (`State`).
- IF a generic module instantiates itself with growing arguments without end THEN the system SHALL refuse it with `E0341`, naming the module.
- WHEN `m` is bound on a module with no flow contribution THEN the system SHALL accept it with no effect on the solution.
- WHEN a template is passed through two levels of params THEN the system SHALL place the module the innermost binding names.

---

## Gaps closed

| Id | What | Closed by |
| -- | ---- | --------- |
| L7 | generic functions, `self` parameters, `impl` on a value type, user traits | GN-01, GN-02, GN-03, GN-04, GN-05 |
| L4 (part) | generic enums (and generic structs) | GN-06 |
| L8 | `impl` on a nature; a function that takes a carrier or a node and probes it | GN-10, GN-11, GN-12 |
| L29 | `m` and `dtemp`: binding and reading | GN-24, GN-25, GN-26 |
| L30 | generic modules, const generics, `Short<N>` and the stdlib modules on them | GN-13, GN-14, GN-15, GN-16 |
| L31 | generation `for` and `if` in a body and in a `digital` block | GN-17, GN-18, GN-19, GN-20 |
| L32 | templates, placement through a template or a function, `Fn(P)` bounds | GN-21, GN-22, GN-23 |
| L58 | the generics group (`Short`, `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `merge`, `cast`, `same`; `MinPulse` through `since`) | GN-27, GN-28. Each item runs once its other forms land (table under GN-28); until then it is refused by that form's name, and `GAPS.md` lists it under that form's id |
| S3 | the type checker checks only what a run's top reaches | GN-08, GN-09; for generic items with GN-07 |
| X6 | what `check` owes a generic body | GN-07 and Q1 (the user's decision); the spec sentence in GN-07 AC 4 |

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| GN-01 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-02 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-03 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-04 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-05 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-06 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-07 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-08 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-09 | P1: generic functions, methods and traits (A) | - | Pending |
| GN-10 | P1: handles (B) | - | Pending |
| GN-11 | P1: handles (B) | - | Pending |
| GN-12 | P1: handles (B) | - | Pending |
| GN-13 | P1: generic modules and generation (C) | - | Pending |
| GN-14 | P1: generic modules and generation (C) | - | Pending |
| GN-15 | P1: generic modules and generation (C) | - | Pending |
| GN-16 | P1: generic modules and generation (C) | - | Pending |
| GN-17 | P1: generic modules and generation (C) | - | Pending |
| GN-18 | P1: generic modules and generation (C) | - | Pending |
| GN-19 | P1: generic modules and generation (C) | - | Pending |
| GN-20 | P1: generic modules and generation (C) | - | Pending |
| GN-21 | P1: templates and implicit params (D) | - | Pending |
| GN-22 | P1: templates and implicit params (D) | - | Pending |
| GN-23 | P1: templates and implicit params (D) | - | Pending |
| GN-24 | P1: templates and implicit params (D) | - | Pending |
| GN-25 | P1: templates and implicit params (D) | - | Pending |
| GN-26 | P1: templates and implicit params (D) | - | Pending |
| GN-27 | P2: the stdlib built on generics (E) | - | Pending |
| GN-28 | P2: the stdlib built on generics (E) | - | Pending |
| GN-29 | P1: every phase keeps the rest intact | - | Pending |
| GN-30 | P1: every phase keeps the rest intact | - | Pending |
| GN-31 | P1: every phase keeps the rest intact | - | Pending |

**Coverage:** 31 total, 0 mapped to tasks (spec only).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps ... Preciso que TODOS os gaps estejam endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"
- `GAPS.md` (audit of 2026-11-17): L4, L7, L8, L29 to L32, L58, S3, X6, §9.
- `docs/spec/part_i_values.md` §2, §3 (rules 7 to 9), §3.1, §3.2.11; `part_iii_structure.md` §8 (rules 4, 8), §8.1.2, §8.2.7, §8.2.23, §8.2.24, §8.4, §8.5.1 to §8.5.6; `part_iv_behavior.md` §10 rule 6, §12 rule 1, §12.1.14; `part_vii_stdlib.md` §22 index; `appendix_h_library.md` H.8; `appendix_g_scope.md` D1, D6; `appendix_c_implementation.md` (generics by monomorphization).
- `crates/piperine-lang/std/*.msl`: the generic stdlib items and the forms they use (table under GN-28).
- `.specs/features/codegen/context.md` S6, S7, R1 to R4; `.specs/STATE.md` MD-18, MD-25, MD-56, MD-63, MD-64.
- The Book's RC ladder (`docs/book/content.html`, "A complete example: an RC ladder").

---

## Success Criteria

- [ ] Every program of the Independent Tests type-checks, elaborates and runs on the Rust backend.
- [ ] `piperine check` fails on a library package with an error in an unreached item.
- [ ] `GAPS.md` no longer lists L7, L8, L29 to L32, S3, X6; no L58 item is refused "because generic".
- [ ] The 7 twins' bytes and every parity test are unchanged.

# Resolved Nets Specification

**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13" (the amendment of 2026-11-21 below; the criteria of 2026-11-18 stand under the user's "Sobre as open questions, vou de aceito todas ok?")
**Amended**: 2026-11-22 - the explicit-unknowns model (MD-78), amendment A6 below. Every rule A6 applies is a decision the user took on 2026-11-21 and is quoted there; Q2 to Q4 are new and wait for the user.

Natures other than `Bit` and the continuous ones with a flow: discrete natures a project
declares (L22), natures that implement the capability `Resolve<X, V>` (L21: several drives,
`others()`, relaxation of loops, `E0511`), and signal-flow natures (L23, `VSignal`). With them
come the library of §22.3 (`Strength`, `Driven`, `Driven::strongest`, `sum`, and the modules
`Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, each generic over
`W: Nature + Resolve<Driven>`), `Bidir` of §22.4.3, the four worked nets of Appendix F.4 and the
Book's rows on nets. The library declares no net nature. Bundles (L25) are the companion spec
`resolved-nets-bundles`.

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | The stdlib nets are written with forms other features deliver: `Logic` is a `struct` (`composite-values`, L1), `resolve` takes a slice `&[T]` and folds over it (L3, L9, L16), `impl Logic` and `Tri<T>` are generic or `impl` on a value type (`generics`, L7), `Logic::Z` and `Logic::wired` use `Unknown` and `merge` (`unknown-semantics`). How is this feature delivered against them? | **A. One delivery after every dependency.** Nothing ships until `composite-values`, `generics` and `unknown-semantics` have merged. **B. Three phases, each when its dependencies are in.** Phase 1, no dependency: project discrete natures (L22) and signal-flow natures (L23). Phase 2, after `composite-values` and `unknown-semantics`: the resolution machinery (drives, the generated resolver, `others()`, relaxation, the `log` record), proven with project natures over scalars (`fn sum(d: &[f64]) -> f64`, a wired-AND over `bool`). Phase 3, after `generics`: `Logic`, `Wire`, `Tri`, the §22.3 modules, `Bidir` and the alarm. **C. Built-in nets first.** The elaborator treats `Logic`, `Strength`, `Wire` and `Logic::wired` as built-ins now, so the alarm runs before its dependencies; the stdlib source replaces them later. | **B.** Each phase is useful alone, and none of them duplicates the stdlib. C contradicts MD-56 D4 (the stdlib is a normative source) and leaves a special case to delete. A holds L22 and L23, which need nothing, behind the largest features of the queue. |

Q1 = B still holds under A6: the phases are re-cut by the dependencies of A6 (design D1, D25).

## Amendment of 2026-11-21 (planning, user AFK)

Written when design and tasks were planned (ROADMAP execution order, item 12). Each change is a
D-numbered default of `design.md` "Decisions taken (user AFK)".

| # | What changed since 2026-11-18 | Effect on this spec |
| - | ----------------------------- | ------------------- |
| A1 | Pure value semantics (G.71, `value-semantics` D7, commit `cc3599bf`): there is no slice and no borrow. `resolve` names `fn f<const N: usize>(drives: [T; N]) -> T`, instantiated at elaboration with `N` the number of drives a call receives (§4 rule 8). `value-semantics` rewrites `std/nets.msl` to that spelling; this feature owns the instantiation | RN-02 AC 1 and AC 3, RN-06 AC 2 and the edge cases are written with arrays; RN-05 AC 7 is added: what `resolve` receives in a relaxation pass |
| A2 | Delivered since: `msl-fixes` (L17, `<` on a `PartialOrd` enum), `analog-structure` (L40, a contribution under an `if`), `host-ii` (L56, a digital-only transient), `unknown-semantics` (`Unknown`, `merge`) | Phase 3 waits on `generics` phase A only; RN-09 AC 1 loses its WHERE (design D18) |
| A3 | Q1 = B becomes four steps (design D1): phase 1 now, beside and below the language (RN-01, RN-08, `E0224`, the drive presence and the record of a loop that does not settle below the language); phases 2 and 3 wait for `composite-values` A–B and `generics` A on `main`; phase 4 is the Book and the records | Dependencies and Size rewritten |
| A4 | The spec-code guard of `params-and-io` (`crates/piperine-lang/tests/support/code_owners.tsv`, PI-13, not yet on `main`) assigns `E0203`, `E0224`, `E0242`, `E0413` to this feature and `E0260`, `E0261`, `E0262`, `E0264` to `resolved-nets-bundles` | RN-10 AC 3 is added |
| A5 | `resolved-nets` and `resolved-nets-bundles` are planned on one branch, `feature/resolved-nets`, each with its own folder, checks and verification (design D2) | none |

## Amendment of 2026-11-22 (A6: the explicit-unknowns model, MD-78)

`explicit-unknowns` (MD-78, on `main` at `20c285b8`) rewrote Part I §3.3, Part II §4 and §5,
§8.2.3, Part VII §22.3 and §22.4.3, Appendix F.4 and the Book. This feature's B1 and B2 (T1 to
T10, on `main`) stand; B3 onward is re-planned on the amended text. Every row below applies a
rule the user decided on 2026-11-21 (`.specs/features/explicit-unknowns/spec.md`, "Decision" and
"Answers"); none is a default of this plan.

| # | The user's decision (2026-11-21) | Effect on this spec |
| - | -------------------------------- | ------------------- |
| A6.1 | "Permitir impls em natures é uma boa... Aí tira-se o campo e passa a exigir que nós discretos só podem ser ligados a multiplos outputs com a trait Resolve" and "Traits como CAPACIDADES" (MD-78 item 5, explicit-unknowns D17, D21) | A nature is resolved when it implements `Resolve`; there is no `resolve` field. RN-02 is rewritten (AC 1 to 5); RN-02 AC 8 adds `E0220` naming the nature and `Resolve` |
| A6.2 | "Recomendo que resolve seja até genérico para Resolve<X: Repr>" and "impl Resolve<Bit> for AndWire ... deixaria fazer o drive de um bit nesse wire" (explicit-unknowns D18, D19) | `Resolve<X: Repr, V: Repr = X>` with `fn resolve<const N: usize>(drives: [X; N]) -> V`; a drive picks its `impl` by the type it drives. RN-02 AC 1, 3 and 7 are written over `X` and `V`; RN-02 AC 9 to 11 add `E0180`, `E0181` and `E0225` |
| A6.3 | "pq tem Wire? Pq eu simplesmente não declaro a Nature ABC com um resolve específico e uso o tipo dela como normal?" (explicit-unknowns D12, D20) | The library declares no net nature: `Wire`, `Tri<T>`, `tri` and `SumNet` are not built. RN-03 AC 2, RN-06, RN-07 and every test declare their own nature; RN-06 AC 3 becomes a nature with two `impl`s and the typed bus `DataBus` of F.4.4 |
| A6.4 | "Q3. Sim, instabilidade é erro" (MD-78 item 5, explicit-unknowns Q3) | A loop that still moves after its bounded passes stops the run with `E0511` naming its drives. RN-05 AC 4 and RN-07 AC 5 are rewritten; the `log` record of design D9 is not built |
| A6.5 | "Bit é binary digit, não tem outros. Logic pode ser 0, 1, Z ou X" and "tudo tem que ser explícito" (MD-78 items 1 to 3, explicit-unknowns R1, R4, D14) | No drive is unknown. A drive with no initial value starts at `Default::default()` of its driven type, or is `E0187`; an absent drive is the engine's (`driving` 0), never a value. RN-03 AC 4, RN-05 AC 7, RN-06 AC 4 and AC 7, RN-07 AC 3 and the edge cases lose every unknown arm |
| A6.6 | "Aceito os recomendados" (Q4: no separate strength trait; Q5: set B, five levels) | `Strength` is `HighZ < Weak < Pull < Strong < Supply`; `Driven { v: Logic, s: Strength }`; `Driven::strongest` is the resolution. RN-06 AC 1 and AC 2 are written on them |
| A6.7 | "ABI pode continuar major 1 pq não fizemos release ainda" (MD-78 item 4, explicit-unknowns D13, D14) | A drive pin is the bytes of `X` followed by a plain `driving: BOOL`; value and others pins are `V`. Design D3 and D17 are superseded |
| A6.8 | The user's rule of 2026-11-21 recorded in ROADMAP.md: "as listagens que não rodam precisam rodar [...] Tem que colocar em alguma feature ANTES do spec closure" | RN-09 names the exact Book rows this feature flips to running: natures#2, natures#3 with natures~e99c1920 and natures~91c3bffe, digital#16 to digital#20, core-std#6 |

---

## Problem Statement

The language has four kinds of nature (§4) and, since B1 and B2, runs three: discrete natures
with one driver (the prelude's `Bit<T>` and a project's own), continuous natures with a flow, and
signal-flow nodes. A discrete nature that implements `Resolve` cannot be declared: `impl Trait for`
a nature is refused, and the code still knows a `resolve` field that the specification removed. So
a net with several drivers cannot be built: no open-drain line, no tri-state bus, no pass gate,
no summing net, no pad (`Bidir`), and none of the library modules of §22.3 that drive such a net
(L58). Ten rows of the Book (the alarm line, the IRQ line, the four worked nets, the chapter's
signal families and the library chapter's resolved line), two galleries (E.7, F.4) and one item of
the conformance set of Appendix C.2.3 (`Tran` on nets of a `Resolve<Driven>` nature) are blocked.

The solver side exists: the device ABI lowers a resolved carrier to one-driver nets plus a
resolver instance, and the digital sub-solver relaxes cycles through `RELAX` nets and, after
`explicit-unknowns` phase A, stops a cycle that does not settle with `E0511`. The gap is the
checker, the elaborator, the IR and the generated Rust.

## Goals

- [ ] A project declares and uses its own discrete natures, with or without `impl Resolve`.
- [ ] A resolved carrier takes any number of drives and holds `resolve` of the present ones, recomputed in the microstep any drive changes; `others()` and relaxation of loops follow §5.2 and §8.2.3, and a loop that does not settle stops the run with `E0511`.
- [ ] Every item of §22.3 and `Bidir` of §22.4.3 elaborates from its library source and gives the tables of the specification.
- [ ] A signal-flow node has one potential driver and no flow, checked at elaboration.
- [ ] The Book's ten net rows run.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Bundles: a `struct` of carriers, `flip`, bundle literals, `impl` on a bundle (L25) | `resolved-nets-bundles` |
| Arrays of carriers, `[Bit; 8]` (L24) | `composite-values` phase C; the Book row natures#2 needs it (Q3) |
| Structs, arrays, iterators, closures (L1, L3, L9, L16) | `composite-values`; this feature uses them |
| Generic functions, trait `impl`s, `impl Trait for` a nature, bounds over a nature's capabilities | `generics` phases A and B, as `explicit-unknowns` amended them (D17); this feature uses them |
| Generic modules (`Pullup<W: Nature + Resolve<Driven>>`) and their instantiation (L30) | `generics` phase C (GN-13, GN-15); this feature's library modules use them (Q2) |
| `Logic<N>`, the operator traits, `Default` and `E0187` | `explicit-unknowns` phases A and B; `Driven` holds a `Logic` |
| The engine's `E0511` and the plain `driving` byte | `explicit-unknowns` T36, T37; this feature names the drives by their holders (RN-05 AC 4) |
| A library net nature (`Wire`, `Tri<T>`, `SumNet`) | removed by MD-78 (A6.3); a model declares its own |
| `TribufDelayed` (§22.5.9) | built on `inertial`, a monitor of `generics` phase E; its group in L58 is `generics`' |
| Running the galleries E.7 and F.4 as gates (U4) | `conformance`; this feature makes their forms elaborate |
| `p.inflow()` (L26) | `analog-structure` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1: delivery against the dependencies | B, in steps re-cut by A6 (design D1, D25) | Answered 2026-11-18; the steps follow the dependencies delivered since | y |
| How a resolved carrier reaches the solver | abi.md §9 D1 as `explicit-unknowns` rewrote it: one drive net per drive in instance-path order, flagged `RELAX`, laid out as `X` then a plain `driving: BOOL`; a resolver instance generated from the nature's `impl Resolve<X, V>`, with one value pin of `V` and one `others` pin of `V` per reader that needs it | The ABI defines it and the user kept it at major 1 (A6.7) | y - user 2026-11-21: "ABI pode continuar major 1 pq não fizemos release ainda" |
| The POM | Keeps every resolved carrier, drive, `impl Resolve` and `others()` call as written; the resolver instance exists in the IR only | The navigability rule (MD-25, MD-54) | y - AFK default (design D5) |
| The bound on relaxation passes | The loop's drive count plus one (§8.2.3 step 3, Appendix C.2) | The spec fixes it | y |
| What a loop that does not settle does | The run stops with `E0511` naming the loop's drives by the instance paths that hold them, in drive order (§8.2.3 step 4) | The user: "Sim, instabilidade é erro" (A6.4) | y - user 2026-11-21 |
| A signal-flow node that meets a nature with `flow` at one net | Conservative, with KCL (§4 rule 13 Resolution, §6 rule 2 Resolution) | The spec fixes it | y |
| Q2: the library modules are generic modules | **Recommended: A.** Phases 4 to 7 (the machinery, proven with modules and natures a test declares) start once `generics` A–B and `explicit-unknowns` B are on `main`; phases 8 and 9 (the §22.3 modules, `Bidir` and the Book rows, every one of which places `Pullup`, `Tribuf`, `Keeper` or `Bidir` except digital#17 and digital#20) wait for `generics` phase C's generic modules (GN-13, GN-15). B: this feature instantiates a module generic over one nature parameter itself, ahead of `generics` C. C: everything waits for `generics` C | `Pullup<W: Nature + Resolve<Driven>>` is a generic module (§22.3.5), which is L30, `generics` phase C; the planned `generics` tasks cover phases A and B only, and the Book's `Dff<T>` (digital#21) is refused today for the same reason. A starts the machinery as early as its own dependencies allow and duplicates nothing; B builds a second module instantiation that `generics` C would replace; C idles phases 4 to 7 for nothing | n - waits for the user |
| Q3: natures#2 needs arrays of carriers | **Recommended: A.** The task that flips natures#2 (T39) waits for `composite-values` phase C (L24, `signal lines: [Bit; 8]`); if it is not on `main` when the other rows close, the feature pauses before T39 and reports. B: natures#2 moves to `composite-values` C as its owner, and this feature flips the other nine rows | The block declares `[Bit; 8]`, `Bit<Option<u8>>`, `Bit<Logic<4>>` and `signal sda: Sda`; only the last is this feature's, but the user's rule makes the whole row run before the spec closes, and `explicit-unknowns` names `resolved-nets` its owner | n - waits for the user |
| Q4: the `impl` of a net no module drives | **Recommended: A.** When nobody drives a carrier (and for the host's drive of a top-level resolved port) the `impl` is the nature's only one; with several, the one whose `X` is `V` (`impl Resolve<V> for N`, the trait's default argument); with several and none of them that, `E0181` naming the carrier and the `impl`s. B: `E0181` whenever a nature with several `impl`s has a carrier with no drive. C: the first `impl` in source order | §5.2.15 picks the `impl` by the driven type, and §4 rule 2 calls `resolve` with an empty array for a carrier nobody drives: with two `impl`s the empty array has no type. A keeps the common case (`impl Resolve<Driven> for Sda`) silent and needs one sentence in §5.2.15; C makes the result depend on source order | n - waits for the user |

**Open questions:** Q2, Q3 and Q4 above wait for the user. Each recommendation is what tasks.md plans; the tasks they touch say so (T27 to T39 for Q2, T39 for Q3, T19 and T33 for Q4).

**Implicit-requirement dimensions.** Input validation: RN-01 AC 4 and 5, RN-02 AC 3 to 5 and 8 to 11, RN-04 AC 3, RN-06 AC 10, RN-08 AC 3 and 4 (the error codes of §3.7 and §4 to §6). Failure states: RN-05 AC 4 and RN-07 AC 5 (`E0511`), the edge case of a panic in `resolve`. Idempotency and retry: N/A because a run is deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: RN-03 AC 3 (drives in instance-path order, independent of evaluation order). Data lifecycle: RN-03 AC 4 (a drive exists from `(0, 0)`). Observability: RN-05 AC 4 (the drives `E0511` names), RN-09 (the alarm's stream). External dependencies: N/A. State transitions: RN-03 AC 5 (`changed()` follows the resolved value, not the drives).

---

## User Stories

### P1: A project declares its own discrete nature ⭐ MVP

**User Story**: As a model author, I want `nature Byte { value: u8 }` to be a carrier type like `Bit<u8>`, so that two kinds of byte bus cannot be wired to each other by mistake.

**Why P1**: L22, size S, and needs nothing else (phase 1 under Q1 = B). Delivered in B1.

**Acceptance Criteria** (RN-01):

1. WHEN a module declares a port `out b: Byte = 0` and a signal `signal s: Byte;` of a project's discrete nature THEN the system SHALL elaborate and run them as it runs `Bit<u8>`: commits, definitions, `changed()`, `prev()`, records.
2. WHEN a carrier of a project's discrete nature is connected to a port of the same nature THEN the system SHALL connect them.
3. WHEN a module declares a port of a project's discrete nature without `in` or `out` THEN the system SHALL report `E0310`.
4. IF a carrier of one discrete nature is connected to a port of another (`Byte` to `Bit<u8>`) THEN the system SHALL report `E0251` naming both natures.
5. IF a discrete nature's `value` type does not implement `Repr` THEN the system SHALL report `E0112`.
6. WHEN a second module drives a carrier of a discrete nature that does not implement `Resolve` THEN the system SHALL report `E0220`, as for `Bit`.
7. WHEN a project declares a generic discrete nature (`nature Word<T: Repr = u8> { value: T }`) THEN the system SHALL accept `Word<u16>` and `Word` (its default) as carrier types holding `u16` and `u8`, as for `Bit<T>` (§4 rule 12).

**Independent Test**: A compiled run of a counter over a `Byte` carrier, and elaboration tests for `E0251`, `E0310` and `E0220`.

---

### P1: A nature that implements `Resolve` is declared and checked ⭐ MVP

**User Story**: As a model author, I want to give my own nature the capability of several drives with an `impl Resolve`, so that wired logic and summing nets are code I write, not language features (A6.1).

**Why P1**: §4 rule 8 and §5.2; everything else of L21 stands on it.

**Acceptance Criteria** (RN-02):

1. WHEN a discrete nature `N` with `value: V` has `impl Resolve<X, V> for N` whose `resolve` is a pure function `fn resolve<const N: usize>(drives: [X; N]) -> V` THEN the system SHALL accept `N` as a resolved nature (§4 rule 8), `impl Resolve<X> for N` meaning `V = X`.
2. WHEN the body of `resolve` calls a library or project function (`Driven::strongest(drives)`, `sum(drives)`) THEN the system SHALL accept it as any pure body (§4 rule 8).
3. IF an `impl Resolve<X, V> for N` has a `V` that is not `N`'s `value` type, is for a continuous nature, or has a `resolve` that is not a pure function with one const parameter `N: usize`, one parameter `[X; N]` and the result `V` THEN the system SHALL report `E0203` naming the nature and the `impl`.
4. IF a nature declares a field named `resolve` THEN the system SHALL refuse it as it refuses any word that is not a nature field (§4 syntax: `potential`, `flow` or `value`).
5. IF a port of a resolved nature is written with `in` or `out` THEN the system SHALL report `E0310`.
6. WHEN a signal of a resolved nature is declared with `= v` THEN the system SHALL make the declaring module hold a drive of it whose first commit is `v`, of the driven type `X`, at `(0, 0)` (§5.2.3); a resolved signal with no `= v` SHALL need no initial value of its own.
7. WHEN a resolved carrier is elaborated THEN the system SHALL instantiate the `resolve` of the `impl` its drives pick once per drive count its resolver can receive, `0` to the number of drives of the carrier, and SHALL lower no other instance (§4 rule 8, design D4).
8. IF a second module drives a carrier whose nature does not implement `Resolve` THEN the system SHALL report `E0220` with a diagnostic that names the nature and `Resolve` (§5.2.1).
9. WHEN a nature implements `Resolve` for several `X` THEN the system SHALL resolve each carrier by the `impl` whose `X` is the type its drives hold (§5.2.15).
10. IF a drive's type is accepted by no `impl` of the nature (a `Logic` driven into `AndWire`) THEN the system SHALL report `E0180` naming the nature and the driven type; IF a literal is accepted by several THEN the system SHALL report `E0181` (§5.2.15).
11. IF two drives of one resolved carrier hold different driven types THEN the system SHALL report `E0225` naming both modules and both types (§5.2.15).

**Independent Test**: Elaboration tests over one accepted and one refused declaration per AC.

---

### P1: Drives combine through `resolve` ⭐ MVP

**User Story**: As a model author, I want every module that sees a resolved carrier to drive it, and the carrier to hold what its nature's `impl` makes of the drives.

**Why P1**: §5.2.2, §5.2.10 to §5.2.12; the definition of a resolved net.

**Acceptance Criteria** (RN-03):

1. WHEN several modules drive one resolved carrier, each by a definition or by commits THEN the system SHALL give the carrier `resolve` of all the present drives, recomputed in the microstep any drive changes (§5.2.10).
2. WHEN nobody drives a resolved carrier THEN the system SHALL give it `resolve` of the empty array, the instance with `N = 0`: `0.0` for a nature whose `impl` is `sum`, `true` for `AndWire` (F.4.1), `Driven::Z` for a nature whose `impl` is `Driven::strongest`.
3. The system SHALL pass the drives to `resolve` in the lexicographic order of the instance paths that hold them, with the host's drive of a top-level carrier first, whatever the order of evaluation (§4 rule 2, Resolution).
4. WHEN a drive has no initial value THEN the system SHALL pass it to `resolve` as `Default::default()` of its driven type from `(0, 0)` until its first commit, and IF that type does not implement `Default` and the driver gives none THEN the system SHALL report `E0187` naming the carrier and `Default` (§5.2.12).
5. WHEN a drive changes and the resolved value does not THEN the system SHALL NOT fire the carrier's `changed()` (§5.2.10).
6. IF one module holds both a definition and a commit of one resolved carrier THEN the system SHALL report `E0221`.
7. WHEN exactly one module drives a resolved carrier THEN the system SHALL still give the carrier `resolve` of that one drive (§5.2.10).

**Independent Test**: A compiled run of three drivers of a project summing nature in instances named out of order, printing the sum and the order seen by a project `resolve` that records its argument.

---

### P1: `others()` ⭐ MVP

**User Story**: As a model author, I want a module to read what the rest of a net drives, so that a bidirectional element can look through itself.

**Why P1**: §5.2.7, §5.2.11; `Tran`, `Keeper` and `Bidir` need it.

**Acceptance Criteria** (RN-04):

1. WHEN a module that drives `s` reads `s.others()` THEN the system SHALL give `resolve` of every present drive of `s` except this module's own, a `V`.
2. WHEN a module that holds no drive of `s` reads `s.others()` THEN the system SHALL give the value of `s`.
3. IF `others()` is called on a one-driver carrier THEN the system SHALL report `E0224`.
4. The system SHALL track dependencies per drive, so that `s.others()` depends on the other drives of `s` and not on this module's own (§8.2.3).

**Independent Test**: A compiled run with three drivers of a summing nature, each emitting its `others()`.

---

### P1: Loops among resolved drives settle by relaxation ⭐ MVP

**User Story**: As a model author, I want pass gates in a ring or a mesh to settle, and the run to stop and name the drives when they cannot, so that switch-level logic runs without latching on itself.

**Why P1**: §8.2.3; the conformance item `Tran` on nets of a `Resolve<Driven>` nature (Appendix C.2.3).

**Acceptance Criteria** (RN-05):

1. WHEN drives of resolved carriers form a loop with no reaction and no `after` THEN the system SHALL accept it and settle it by relaxation in each microstep where an input of the loop changed.
2. WHEN a loop relaxes THEN the system SHALL start every drive of the loop absent from what `resolve` receives and repeat passes, each from the values the previous pass left, until no drive changes, at most the loop's drive count plus one passes.
3. WHEN two `Tran(a, b, true)` gates join nets `a` and `b` of one nature that implements `Resolve<Driven>` in parallel and one module drives `a` strong THEN the system SHALL give `b` that level, and SHALL keep `a` and `b` two carriers, each with its own `changed()`.
4. IF drives still move after the last pass THEN the system SHALL stop the run with `E0511`, whose diagnostic names every drive of the loop by the instance path that holds it, in drive order (§8.2.3 step 4).
5. IF a loop passes through a one-driver carrier THEN the system SHALL report `E0339`, as today.
6. WHEN a loop is released (no drive left in it) THEN the system SHALL NOT keep a level from an earlier microstep: a loop cannot latch.
7. WHILE a relaxation pass has drives of the loop not yet evaluated THEN the system SHALL pass `resolve` only the drives present, in instance-path order; absence SHALL NOT be a value of the driven type, and a present drive that holds `Driven::Z` SHALL be passed (§8.2.3 step 1, §4 rule 15, §22.3.1 rule 2).

**Independent Test**: The conformance item of Appendix C.2.3: `Tran` gates in parallel and in a ring on nets of a `Resolve<Driven>` nature, and two drivers that each drive the negation of `others()` for AC 4.

---

### P1: The library nets of §22.3 ⭐ MVP

**User Story**: As a model author, I want `Strength`, `Driven`, `Driven::strongest` and the modules built on them from the library, generic over any nature I give the capability, so that I do not write IEEE 1364 net resolution myself and the library imposes no net of its own (A6.3).

**Why P1**: L58 lists the library items refused for this reason.

**Acceptance Criteria** (RN-06):

1. WHEN a nature whose `impl Resolve<Driven>` is `Driven::strongest` is resolved THEN the system SHALL give the table of §22.3.2: no drive or only `HighZ` drives `Driven::Z`; one strongest drive that drive; several of the greatest strength with the same level that level and strength; several of the greatest strength with different levels `Logic::X` at that strength.
2. WHEN `Driven::strongest` is evaluated over any permutation of its drives THEN the system SHALL give one result, and `strongest([x, x])` SHALL be `x`.
3. WHEN a nature implements `Resolve` for two driven types THEN the system SHALL resolve each of its carriers by the `impl` of its drives' type; and WHEN `DataBus` with `impl Resolve<Option<u8>, Data>` (Appendix F.4.4) is resolved THEN the system SHALL give `Data::Free` for no drive or only `None`s, `Data::Byte(b)` for exactly one `Some(b)`, and `Data::Contended` for two or more `Some`.
4. WHEN `Tribuf` has `oe` high or low THEN its drive SHALL be `Driven::strong(d)` or `Driven::Z` (§22.3.5).
5. WHEN `Pullup` and a strong driver drive one net THEN the system SHALL give the strong drive while it drives and `Driven::pull(true)` while every stronger drive is released; `Pulldown` the same with `Driven::pull(false)`.
6. WHEN `Keeper` sees the rest of its net drive a 0 or a 1 and then release THEN the system SHALL hold that level at `Weak`; before any 0 or 1 has been seen it SHALL drive `Logic::X` at `Weak`, and it SHALL NOT record an X its net shows (§22.3.6).
7. WHEN `Tran(a, b, en)` has `en` high THEN the system SHALL drive each side with the other side's `others()`; with `en` low, `Driven::Z` (§22.3.7).
8. WHEN `Tgate(a, y, s, sn)` has `s` high or `sn` low THEN the system SHALL join `a` and `y`; otherwise it SHALL drive neither (§22.3.8).
9. WHEN an analysis has no time (`op`, `dc`) THEN the system SHALL run definitions, `resolve`, `others()` and `Keeper`'s reaction at the microsteps of `(0, ·)` and of each sweep point; in a small-signal sweep the nets SHALL be constant (§22.3, per-analysis table).
10. IF a library module of §22.3 is placed on a carrier whose nature does not implement `Resolve<Driven>` THEN the system SHALL report `E0187` naming the nature and `Resolve<Driven>` (§3.7, a generic argument that does not satisfy its bound).

**Independent Test**: One compiled test per item with the tables of §22.3, through drivers made of timed commits, each on a nature the test declares.

---

### P1: A signal-flow node ⭐ MVP

**User Story**: As a model author, I want a node that holds a potential with no current, driven by exactly one module, so that a block diagram of ideal amplifiers and filters needs no load resistors.

**Why P1**: L23; needs nothing else (phase 1 under Q1 = B). Delivered in B1.

**Acceptance Criteria** (RN-08):

1. WHEN a node of a nature without `flow` (`VSignal`) has exactly one module contributing potential to `(n, gnd)` THEN the system SHALL elaborate and solve it, and readers that probe `v(n)` SHALL load it with no flow.
2. WHEN a chain of two gain blocks `v(out) +> k * v(inp)` over `VSignal` nodes is solved in `op`, `dc`, `ac` and `transient` THEN the output SHALL equal the product of the gains times the input, within `ToleranceContract::DC_VOLTAGE` in `op` and `dc`.
3. IF a signal-flow node has no potential driver, or more than one THEN the system SHALL report `E0242` naming the node and its drivers.
4. IF a module contributes a flow to a node of a nature without `flow` THEN the system SHALL report `E0413`.
5. WHEN a `VSignal` node is connected to an `Electrical` port THEN the system SHALL accept the connection and treat the net as conservative, with KCL (§4 rule 13).
6. IF `i(n)` or `flow(n)` probes a node of a nature without `flow` THEN the system SHALL report `E0206`.
7. WHEN a `VSignal` port of the top has no driver in the design THEN the system SHALL elaborate it, because the host drives the top's node ports (`params-and-io`, §23.1 rule 1), and a run that gives it no host source SHALL be `E0810`. (Added by the fix round of the partial merge, 2026-11-21; AC 4 covers a contribution spelled `flow(n)` or with another nature's access name, as `i(n)`.)

**Independent Test**: A compiled op, ac and transient of the gain chain, and elaboration tests for `E0242`, `E0413` and `E0206`.

---

### P1: `Bidir` ⭐ MVP

**User Story**: As a designer of a mixed-signal pad, I want the library `Bidir`, so that one net of a `Resolve<Driven>` nature and one node meet at a pad that drives whichever side is not driven.

**Why P1**: §22.4.3; the only converter between a resolved net and a node.

**Acceptance Criteria** (RN-07):

1. WHILE the rest of the net drives (`others().s` is not `HighZ`) the system SHALL make `Bidir` drive the node as `D2A` does from `others().v`, and drive the net with `Driven::Z`.
2. WHILE the rest of the net has released, the system SHALL make `Bidir` contribute nothing to the node (zero flow) and drive the net with `Driven::strong(seen)`, `seen` thresholded from the node with the hysteresis of `A2D`.
3. WHILE the rest of the net is contended (`Logic::X` at a strength above `HighZ`) the system SHALL make `Bidir` act as driven and drive the node toward mid-rail (`to_volts` reads X as half of its argument, §22.1.1 rule 5).
4. WHEN `driving` changes THEN the system SHALL make that instant a time point and a jump that `analog` reads.
5. WHEN two `Bidir`s are alone on one net THEN the system SHALL stop the run with `E0511` naming both pads' drives (§22.4.3 rule 4).

**Independent Test**: A compiled transient with a `Tribuf`, a `Bidir` and an RC load on its node, toggling `oe`.

---

### P2: The Book's net rows run

**User Story**: As a reader of the Book, I want every listing about nets to run and print what the Book says.

**Why P2**: The proof on the Book's own source, and the user's rule that every Book listing runs before the specification closes (A6.8).

**Acceptance Criteria** (RN-09):

1. WHEN `piperine run alarm.msl --analysis transient --stop 4e-6 --stream line` runs THEN the system SHALL print three records of `Alarm.watch.line`: at 0 `level = 1, strength = Pull`; at 1 µs `level = 0, strength = Strong`; at 3 µs `level = 1, strength = Pull`; and none at 2 µs (natures#3).
2. WHEN the alarm runs THEN the Book's listing SHALL be the binary's output, the sentence natures~e99c1920 ("Today the command refuses the file and runs nothing:") SHALL no longer be in the Book, and `piperine-cli/tests/book.rs` SHALL pass.
3. WHEN `Pullup` is replaced by `Pulldown` THEN the system SHALL print the records of the Book's exercise answer, natures~91c3bffe: one record at 0 `level = 0, strength = Pull`, `strength = Strong` at 1 µs with the level still 0, `strength = Pull` at 3 µs.
4. WHEN the IRQ line of the digital chapter (digital#16) and the library chapter's resolved line (core-std#6) run THEN the system SHALL give the outcomes the Book's prose and Appendix E.7 state: `pending` follows a request at the next clock edge, and stays low with every source released.
5. WHEN the four worked nets of "Driving with strength" (digital#17 to digital#20) run THEN the system SHALL give the tables of Appendix F.4.1 to F.4.4.
6. WHEN the chapter's block of signal families (natures#2) is elaborated THEN the system SHALL accept every declaration in it, `signal sda: Sda;` included, and give each carrier the initial value its line states.
7. WHEN the rows of AC 1 to AC 6 run THEN `tests/book/inventory.tsv` SHALL hold none of natures#2, natures#3, natures~e99c1920, natures~91c3bffe, digital#16 to digital#20 and core-std#6 as `refused`, and `GAPS.md` §11 SHALL hold no row for them.

**Independent Test**: `cargo test -p piperine --test book_natures --test book_digital --test book_core_std` and `cargo test -p piperine-cli --test book`.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md`, the Book and the spec, I want them to say that these natures run.

**Why P3**: No behaviour depends on it.

**Acceptance Criteria** (RN-10):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L21, L22, L23, nor the resolved-net group of L58 except `TribufDelayed`, whose other dependency (`inertial`) is still open.
2. WHEN this feature merges THEN Appendix C.2.3 and Appendix G.38 SHALL say that `Tran` on nets of a `Resolve<Driven>` nature runs, with a `spec_text` guard (MD-57).
3. WHEN a task first raises a code that `crates/piperine-lang/tests/support/code_owners.tsv` assigns to `resolved-nets` or that this feature raises first (`E0203`; `E0225`, owned by `explicit-unknowns`) THEN that commit SHALL delete the code's row, so the guard `spec_codes` passes (design D16).

**Independent Test**: `cargo test -p piperine-lang --test spec_text` and `cargo test -p piperine-cli --test docs`.

---

## Edge Cases

- WHEN a resolved carrier is a top-level `in` port THEN the system SHALL pass the host's drive first (§4 rule 2, Resolution), with the driven type the other drives pick (Q4).
- WHEN `resolve` itself panics (an index into the empty array of a carrier nobody drives) THEN the system SHALL report the panic as a run fault naming the nature and the carrier.
- WHEN a drive holds `Driven::Z`, or any level at `Strength::HighZ` THEN `Driven::strongest` SHALL leave it out (§22.3.2).
- WHEN a resolved carrier is a child's `pub` signal THEN the parent SHALL be allowed to drive it (§5.2.2: any module that sees it).
- WHEN a loop's drives settle on the first pass THEN the system SHALL raise nothing.
- WHEN nobody drives a carrier whose nature has several `impl`s THEN the system SHALL apply Q4.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| RN-01 | P1: A project declares its own discrete nature | T1–T2 | Verified |
| RN-02 | P1: A nature that implements `Resolve` is declared and checked | T16, T17, T19, T20, T28, T37, T38 | Delivered (T16-T20, T28, T37, T38) |
| RN-03 | P1: Drives combine through `resolve` | T12–T14, T17, T18, T20, T22, T33 | Delivered (T22, T33) |
| RN-04 | P1: `others()` | T3, T15, T19, T23 | Verified |
| RN-05 | P1: Loops among resolved drives settle by relaxation | T7–T9, T11, T21, T24, T25, T31 | Delivered (T24, T25, T31) |
| RN-06 | P1: The library nets of §22.3 | T26–T31 | Delivered (T26-T31) |
| RN-07 | P1: `Bidir` | T32 | Delivered (T32) |
| RN-08 | P1: A signal-flow node | T4–T6 | Verified |
| RN-09 | P2: The Book's net rows run | T34, T35, T39 | Delivered (T34, T35, T39) |
| RN-10 | P3: The documents say what is true | T10, T36, T38 | Delivered (T36) |

**Coverage:** 10 total, 10 mapped to tasks (tasks.md, T1–T39), 0 unmapped.

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| L21 | natures that implement `Resolve<X, V>`; `others()`; relaxation of loops; `E0511` | RN-02, RN-03, RN-04, RN-05, RN-06 |
| L22 | a discrete nature the project declares | RN-01 |
| L23 | signal-flow natures (`VSignal`) | RN-08 |
| L25 | bundles | `resolved-nets-bundles` |
| L58, resolved-net group | `Strength`, `Driven`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir` | RN-06, RN-07 (`TribufDelayed`: `generics`, with `inertial`) |
| §11, the net rows | natures#2, natures#3, natures~e99c1920, natures~91c3bffe, digital#16 to digital#20, core-std#6 | RN-09 |

## Dependencies

- Phases 1 to 3 (RN-01, RN-08, RN-04 AC 3, relaxation below the language): delivered (B1, B2, on `main`).
- Phases 4 to 7 (RN-02 to RN-05 over natures and modules a test declares): `generics` phases A and B on `main` (trait `impl`s, `impl Trait for` a nature as `explicit-unknowns` D17 amended `generics` T26, T27, T46, T48; static dispatch; `Instantiations` of a trait function; handles), and `explicit-unknowns` phase B on `main` (B8: `Logic<N>` from source, the operator traits; phase A before it: `Default`, `E0187`, the plain `driving` byte, `E0511`, `std/nets.msl` with `Strength`, `Driven`, `strongest`, `sum`).
- Phases 8 and 9 (RN-06, RN-07, RN-09): phases 4 to 7, and `generics` phase C's generic modules (GN-13, GN-15) under Q2 = A. T39 (natures#2) also waits for `composite-values` phase C (L24) under Q3 = A.
- Already delivered: `composite-values` A–B, `value-semantics`, `msl-fixes` L17, `analog-structure` L40, `host-ii` L56.
- Downstream: `resolved-nets-bundles` (a bundle field of a resolved nature has no direction), on the same branch.

## Size

**L.** 39 tasks in 9 phases: 10 delivered (B1, B2), 29 re-planned in 5 batches (tasks.md, B3 to B7).

## Sources

- `GAPS.md` §4.2 L21, L22, L23; L58; §11 (the net rows).
- `docs/spec/part_i_values.md` §3.1.9, §3.7 (traits as capabilities, `E0187`), §3.3.4 (`Default`); `part_ii_carriers.md` §4 (rules 1 to 16, errors), §5.1, §5.2 (§5.2.1 to §5.2.15, `E0220`, `E0225`), §6 (rules 2, 10, errors), §6.1; `part_iii_structure.md` §8.1 (`E0310`), §8.2.3 (`E0511`); `part_iv_behavior.md` §10 rule 6 (`E0403`); `part_vii_stdlib.md` §22.3, §22.4.3; `appendix_c_implementation.md` C.2 and C.2.3; `appendix_e_gallery_digital.md` E.7; `appendix_f_gallery_mixed.md` F.4.
- `.specs/features/explicit-unknowns/{spec,design,tasks}.md` (MD-78; design "`resolved-nets` (B1, B2 on `main`; B3 onward waiting: T11 to T36)"; tasks Handoff "Rows other features must flip").
- `.specs/features/device-abi/abi.md` §9 D1, `RELAX`; `crates/piperine-digital-solver/src/dag.rs`, `run.rs`; `crates/piperine-lang/src/types/carrier.rs`; `crates/piperine-lang/std/nets.msl`, `carriers.msl`.
- `docs/book/content.html`, chapters "Natures and carriers", "Digital behavior" ("Many drivers", "Driving with strength") and "Core and standard library".
- `ROADMAP.md` (the rule "Every Book listing runs before item 13"); `.specs/STATE.md` MD-25, MD-54, MD-56 D4, MD-57, MD-78.

---

## Success Criteria

- [ ] No `P0001` remains for a project discrete nature, a nature that implements `Resolve`, a signal-flow nature or a §22.3 item.
- [ ] `Tran` on nets of a `Resolve<Driven>` nature (Appendix C.2.3) runs and settles; a loop that does not settle stops with `E0511` naming its drives.
- [ ] The Book's ten net rows run.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

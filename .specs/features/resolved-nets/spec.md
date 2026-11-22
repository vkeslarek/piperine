# Resolved Nets Specification

**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13" (the amendment of 2026-11-21 below; the criteria of 2026-11-18 stand under the user's "Sobre as open questions, vou de aceito todas ok?")

Natures other than `Bit` and the continuous ones with a flow: discrete natures a project
declares (L22), natures with `resolve` (L21: `Wire`, `Tri`, `SumNet`, `others()`, relaxation of
loops), and signal-flow natures (L23, `VSignal`). With them come the stdlib of §22.3 (`Logic`,
`Strength`, `Wire`, `Tri`, `SumNet`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`),
`Bidir` of §22.4.3, and the Book's alarm line. Bundles (L25) are the companion spec
`resolved-nets-bundles`.

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | The stdlib nets are written with forms other features deliver: `Logic` is a `struct` (`composite-values`, L1), `resolve` takes a slice `&[T]` and folds over it (L3, L9, L16), `impl Logic` and `Tri<T>` are generic or `impl` on a value type (`generics`, L7), `Logic::Z` and `Logic::wired` use `Unknown` and `merge` (`unknown-semantics`). How is this feature delivered against them? | **A. One delivery after every dependency.** Nothing ships until `composite-values`, `generics` and `unknown-semantics` have merged. **B. Three phases, each when its dependencies are in.** Phase 1, no dependency: project discrete natures (L22) and signal-flow natures (L23). Phase 2, after `composite-values` and `unknown-semantics`: the resolution machinery (drives, the generated resolver, `others()`, relaxation, the `log` record), proven with project natures over scalars (`fn sum(d: &[f64]) -> f64`, a wired-AND over `bool`). Phase 3, after `generics`: `Logic`, `Wire`, `Tri`, the §22.3 modules, `Bidir` and the alarm. **C. Built-in nets first.** The elaborator treats `Logic`, `Strength`, `Wire` and `Logic::wired` as built-ins now, so the alarm runs before its dependencies; the stdlib source replaces them later. | **B.** Each phase is useful alone, and none of them duplicates the stdlib. C contradicts MD-56 D4 (the stdlib is a normative source) and leaves a special case to delete. A holds L22 and L23, which need nothing, behind the largest features of the queue. |

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

---

## Problem Statement

The language has four kinds of nature (§4) and runs two: discrete natures with one driver, only
as the prelude's `Bit<T>`, and continuous natures with a flow. A project's own discrete nature
(`nature Byte { value: u8 }`) is `P0001`. A nature with `resolve` is `P0001`, so a wire with
several drivers cannot be built: no open-drain line, no tristate bus, no pass gate, no summing
net, no pad (`Bidir`), and none of the thirteen stdlib items built on them (L58). A signal-flow
nature such as `VSignal` is `P0001`, so a block diagram with ideal voltage outputs needs a
current-carrying node. The Book's alarm line, two galleries (E.7, F.4) and one item of the
conformance set of Appendix C.2.3 (`Tran` on `Wire`s) are blocked.

The solver side exists: the device ABI lowers a resolved carrier to one-driver nets plus a
resolver instance, and the digital sub-solver relaxes cycles through `RELAX` nets
(abi.md §9 D1, `piperine-digital-solver` `dag.rs`, `run.rs`). The gap is the type checker, the
elaborator, the IR and the generated Rust.

## Goals

- [ ] A project declares and uses its own discrete natures, with or without `resolve`.
- [ ] A resolved carrier takes any number of drives and holds `resolve` of them, recomputed in the microstep any drive changes; `others()` and relaxation of loops follow §5.2 and §8.2.3.
- [ ] Every item of §22.3 and `Bidir` of §22.4.3 elaborates from its stdlib source and gives the tables of the spec.
- [ ] A signal-flow node has one potential driver and no flow, checked at elaboration.
- [ ] The Book's alarm prints its three records.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Bundles: a `struct` of carriers, `flip`, bundle literals, `impl` on a bundle (L25) | `resolved-nets-bundles` |
| Arrays of carriers, `[Wire; 8]` (L24) | `composite-values` |
| Structs, arrays, iterators, closures (L1, L3, L9, L16) | `composite-values`; this feature uses them |
| Generic functions, const parameters, `impl` on a value type (L7) | `generics` phase A; `resolve`, `tri` and `impl Logic` use them. A generic discrete nature (`Tri<T>`, a project's `nature Word<T: Repr>`) is this feature's (RN-01 AC 7, RN-06 AC 3) |
| `Unknown`, `merge`, the merge of unknown control | `unknown-semantics` |
| Per-level flags: which levels of a non-settling loop become unknown | `unknown-semantics-per-level`. With one flag per value the whole field becomes unknown |
| A digital-only transient (L56) | `host-ii` (lane B); the alarm needs it |
| A contribution under an `if` (L40) | `analog-structure`; `Bidir` needs it |
| `<` and `>` on an enum that derives `PartialOrd` (L17) | `msl-fixes` (lane A); `Logic::wired` and `Keeper` compare strengths |
| `TribufDelayed` (§22.5.9) | A monitor built on generics and `inertial`; its group in L58 is `generics`' |
| Running the galleries E.7 and F.4 as gates (U4) | `conformance`; this feature makes their forms elaborate |
| `p.inflow()` (L26) | `analog-structure` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1: delivery against the dependencies | B, in four steps (amendment A3, design D1) | Answered 2026-11-18; the steps follow the dependencies delivered since | y |
| How a resolved carrier reaches the solver | abi.md §9 D1 as written: one-driver nets per drive in instance-path order, flagged `RELAX`, and a resolver instance generated from the nature, with one value pin and one `others` pin per reader that needs it | The ABI already defines it and the digital sub-solver implements the relaxation; how an absent drive differs from an unknown one is design D3 | y - AFK default (design D3 to D6) |
| The POM | Keeps every resolved carrier, drive and `others()` call as written; the resolver instance exists in the IR only | The navigability rule (MD-25, MD-54) | y - AFK default (design D5) |
| The bound on relaxation passes | The loop's drive count plus one (§8.2.3 step 3, Appendix C.2) | The spec fixes it | y |
| Where the record of a loop that does not settle goes | The `log` stream of the instance of the loop's first drive in instance-path order, once per loop and analysis, naming every drive of the loop (§8.2.3 step 4, §19 rule 2) | The spec fixes it | y |
| A signal-flow node that meets a nature with `flow` at one net | Conservative, with KCL (§4 rule 13 Resolution, §6 rule 2 Resolution) | The spec fixes it | y |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation: RN-01 AC 4 and 5, RN-02 AC 3 to 5, RN-04 AC 3, RN-08 AC 3 and 4 (the error codes of §4 to §6). Failure states: RN-05 AC 4 (a loop that does not settle is not an error; it is unknown and logged). Idempotency and retry: N/A because a run is deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: RN-03 AC 3 (drives in instance-path order, independent of evaluation order). Data lifecycle: N/A because nothing is stored. Observability: RN-05 AC 4 (the `log` record), RN-09 (the alarm's stream). External dependencies: N/A. State transitions: RN-03 AC 5 (`changed()` follows the resolved value, not the drives).

---

## User Stories

### P1: A project declares its own discrete nature ⭐ MVP

**User Story**: As a model author, I want `nature Byte { value: u8 }` to be a carrier type like `Bit<u8>`, so that two kinds of byte bus cannot be wired to each other by mistake.

**Why P1**: L22, size S, and needs nothing else (phase 1 under Q1 = B).

**Acceptance Criteria** (RN-01):

1. WHEN a module declares a port `out b: Byte = 0` and a signal `signal s: Byte;` of a project's discrete nature THEN the system SHALL elaborate and run them as it runs `Bit<u8>`: commits, definitions, `changed()`, `prev()`, records.
2. WHEN a carrier of a project's discrete nature is connected to a port of the same nature THEN the system SHALL connect them.
3. WHEN a module declares a port of a project's discrete nature without `in` or `out` THEN the system SHALL report `E0310`.
4. IF a carrier of one discrete nature is connected to a port of another (`Byte` to `Bit<u8>`) THEN the system SHALL report `E0251` naming both natures.
5. IF a discrete nature's `value` type does not implement `Repr` THEN the system SHALL report `E0112`.
6. WHEN a second module drives a carrier of a discrete nature without `resolve` THEN the system SHALL report `E0220`, as for `Bit`.
7. WHEN a project declares a generic discrete nature (`nature Word<T: Repr = u8> { value: T }`) THEN the system SHALL accept `Word<u16>` and `Word` (its default) as carrier types holding `u16` and `u8`, as for `Bit<T>` (§4 rule 12).

**Independent Test**: A compiled run of a counter over a `Byte` carrier, and elaboration tests for `E0251`, `E0310` and `E0220`.

---

### P1: A nature with `resolve` is declared and checked ⭐ MVP

**User Story**: As a model author, I want to declare a nature whose drivers combine through my own function, so that wired logic and summing nets are library code, not language features.

**Why P1**: §4 rule 2; everything else of L21 stands on it.

**Acceptance Criteria** (RN-02):

1. WHEN a nature declares `value: T` and `resolve: f` with `f` a pure function with one const parameter, `fn f<const N: usize>(drives: [T; N]) -> T` THEN the system SHALL accept it as a resolved nature (§4 rule 8).
2. WHEN `resolve` names an associated function (`Logic::wired`) or a const of function type THEN the system SHALL accept it (§4 rule 8).
3. IF `resolve` is not a pure function with exactly one const parameter `N: usize`, one parameter `[T; N]` and the result `T` THEN the system SHALL report `E0203` naming the nature and the function.
7. WHEN a resolved carrier is elaborated THEN the system SHALL instantiate its nature's `resolve` once per drive count its resolver can receive, `0` to the number of drives of the carrier, and SHALL lower no other instance (§4 rule 8, design D4).
4. IF a nature declares `resolve` without `value`, or mixes it with `potential` or `flow` THEN the system SHALL report `E0201`.
5. IF a port of a resolved nature is written with `in` or `out` THEN the system SHALL report `E0310`.
6. WHEN a signal of a resolved nature is declared with `= v` THEN the system SHALL make the declaring module hold a drive of it whose first commit is `v` at `(0, 0)` (§5.2.3).

**Independent Test**: Elaboration tests over one accepted and one refused declaration per AC.

---

### P1: Drives combine through `resolve` ⭐ MVP

**User Story**: As a model author, I want every module that sees a resolved carrier to drive it, and the carrier to hold what its nature makes of the drives.

**Why P1**: §5.2.2, §5.2.10 to §5.2.12; the definition of a resolved net.

**Acceptance Criteria** (RN-03):

1. WHEN several modules drive one resolved carrier, each by a definition or by commits THEN the system SHALL give the carrier `resolve` of all the drives, recomputed in the microstep any drive changes (§5.2.10).
2. WHEN nobody drives a resolved carrier THEN the system SHALL give it `resolve([])`, the instance with `N = 0`: `0.0` for `SumNet`, `Logic::Z` for `Wire`.
3. The system SHALL pass the drives to `resolve` in the lexicographic order of the instance paths that hold them, with the host's drive of a top-level carrier first, whatever the order of evaluation (§4 rule 2, Resolution).
4. WHEN a drive has no initial value THEN the system SHALL pass it to `resolve` as unknown from `(0, 0)` until its first commit (§5.2.12).
5. WHEN a drive changes and the resolved value does not THEN the system SHALL NOT fire the carrier's `changed()` (§5.2.10).
6. IF one module holds both a definition and a commit of one resolved carrier THEN the system SHALL report `E0221`.

**Independent Test**: A compiled run of three `SumNet` drivers in instances named out of order, printing the sum and the order seen by a project `resolve` that records its argument.

---

### P1: `others()` ⭐ MVP

**User Story**: As a model author, I want a module to read what the rest of a wire drives, so that a bidirectional element can look through itself.

**Why P1**: §5.2.7, §5.2.11; `Tran` and `Bidir` need it.

**Acceptance Criteria** (RN-04):

1. WHEN a module that drives `s` reads `s.others()` THEN the system SHALL give `resolve` of every drive of `s` except this module's own.
2. WHEN a module that holds no drive of `s` reads `s.others()` THEN the system SHALL give the value of `s`.
3. IF `others()` is called on a one-driver carrier THEN the system SHALL report `E0224`.
4. The system SHALL track dependencies per drive, so that `s.others()` depends on the other drives of `s` and not on this module's own (§8.2.3).

**Independent Test**: A compiled run with three drivers of a `SumNet`, each emitting its `others()`.

---

### P1: Loops among resolved drives settle by relaxation ⭐ MVP

**User Story**: As a model author, I want pass gates in a ring or a mesh to settle, and to be told when they cannot, so that switch-level logic runs without latching on itself.

**Why P1**: §8.2.3; the conformance item `Tran` on `Wire`s (Appendix C.2.3).

**Acceptance Criteria** (RN-05):

1. WHEN drives of resolved carriers form a loop with no reaction and no `after` THEN the system SHALL accept it and settle it by relaxation in each microstep where an input of the loop changed.
2. WHEN a loop relaxes THEN the system SHALL start every drive of the loop as not driving and repeat passes, each from the values the previous pass left, until no drive changes, at most the loop's drive count plus one passes.
3. WHEN two `Tran(a, b, true)` gates join wires `a` and `b` in parallel and one module drives `a` strong THEN the system SHALL give `b` that level, and SHALL keep `a` and `b` two carriers, each with its own `changed()`.
4. IF drives still move after the last pass THEN the system SHALL make the moving levels unknown and write one record naming the loop's drives on the `log` stream of the first drive's instance, once per loop and analysis (§8.2.3 step 4).
5. IF a loop passes through a one-driver carrier THEN the system SHALL report `E0339`, as today.
6. WHEN a loop is released (no drive left in it) THEN the system SHALL NOT keep a level from an earlier microstep: a loop cannot latch.
7. WHILE a relaxation pass has drives of the loop not yet evaluated THEN the system SHALL pass `resolve` only the drives present, in instance-path order, and SHALL pass a drive whose value is unknown as present (§8.2.3 step 1, §5.2.12; design D3).

**Independent Test**: The conformance item of Appendix C.2.3: `Tran` gates in parallel and in a ring on `Wire`s, and two `Bidir` pads alone on a wire for AC 4.

---

### P1: The stdlib nets of §22.3 ⭐ MVP

**User Story**: As a model author, I want `Logic`, `Wire`, `Tri`, `SumNet` and the modules built on them from the stdlib, so that I do not write IEEE 1364 net resolution myself.

**Why P1**: L58 lists thirteen stdlib items refused for this reason.

**Acceptance Criteria** (RN-06):

1. WHEN `Wire` is resolved THEN the system SHALL give the table of §22.3.2: no drive `Logic::Z`; one strongest drive that drive; several of the greatest strength with the same level that level and strength; several of the greatest strength with different levels an unknown level with that strength.
2. WHEN `wired([x, x])` is evaluated THEN the system SHALL give `x`, and SHALL give the same result for any order of the drives.
3. WHEN `Tri<T>` is resolved THEN the system SHALL give `None` for no drive or only `None`s, `Some(x)` for exactly one `Some(x)`, and `Some(Unknown)` for two or more `Some` (§22.3.3).
4. WHEN `Tribuf` has `oe` high, low or unknown THEN its drive SHALL be `Logic::strong(d)`, `Logic::Z`, or the merge of the two (strength and level unknown) (§22.3.5).
5. WHEN `Pullup` and a strong driver drive one `Wire` THEN the system SHALL give the strong drive while it drives and the pull while every stronger drive is released.
6. WHEN `Keeper` sees its wire driven above `Weak` and then released THEN the system SHALL hold the last driven level weakly; before any stronger drive it SHALL drive a weak unknown level (§22.3.6).
7. WHEN `Tran(a, b, en)` has `en` high THEN the system SHALL drive each side with the other side's `others()`; with `en` low, `Logic::Z`; with `en` unknown, the merge of the two arms (§22.3.7).
8. WHEN `Tgate(a, y, s, sn)` has `s` high or `sn` low THEN the system SHALL join `a` and `y`; otherwise it SHALL drive neither (§22.3.8).
9. WHEN an analysis has no time (`op`, `dc`) THEN the system SHALL run definitions, `resolve`, `others()` and `Keeper`'s reaction at the microsteps of `(0, ·)` and of each sweep point; in a small-signal sweep the nets SHALL be constant (§22.3, per-analysis table).

**Independent Test**: One compiled test per item with the tables of §22.3, through drivers made of timed commits.

---

### P1: A signal-flow node ⭐ MVP

**User Story**: As a model author, I want a node that holds a potential with no current, driven by exactly one module, so that a block diagram of ideal amplifiers and filters needs no load resistors.

**Why P1**: L23; needs nothing else (phase 1 under Q1 = B).

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

**User Story**: As a designer of a mixed-signal pad, I want the stdlib `Bidir`, so that one `Wire` and one node meet at a pad that drives whichever side is not driven.

**Why P1**: §22.4.3; the only converter between a resolved net and a node.

**Acceptance Criteria** (RN-07):

1. WHILE the rest of the wire drives (`others().s` is not `HighZ`) the system SHALL make `Bidir` drive the node as `D2A` does from `others().v`, and drive the wire with `Logic::Z`.
2. WHILE the rest of the wire has released, the system SHALL make `Bidir` contribute nothing to the node (zero flow) and drive the wire with `Logic::strong(seen)`, `seen` thresholded from the node with the hysteresis of `A2D`.
3. WHILE the strength of the rest of the wire is unknown, the system SHALL make `Bidir` act as if driven.
4. WHEN `driving` changes THEN the system SHALL make that instant a time point and a jump that `analog` reads.
5. WHEN two `Bidir`s are alone on one wire THEN the system SHALL make the wire unknown and name the loop on `log` (§22.4.3 rule 4).

**Independent Test**: A compiled transient with a `Tribuf`, a `Bidir` and an RC load on its node, toggling `oe`.

---

### P2: The Book's alarm line

**User Story**: As a reader of the Book, I want the alarm example of the chapter on natures and carriers to print what the Book says.

**Why P2**: The proof on the Book's own source; it also needs a digital-only transient (`host-ii`, L56).

**Acceptance Criteria** (RN-09):

1. WHEN `piperine run alarm.msl --analysis transient --stop 4e-6 --stream line` runs THEN the system SHALL print three records of `Alarm.watch.line`: at 0 `level = true, strength = Pull`; at 1 µs `level = false, strength = Strong`; at 3 µs `level = true, strength = Pull`; and none at 2 µs.
2. WHEN the alarm runs THEN the Book's listing SHALL be the binary's output and `piperine-cli/tests/book.rs` SHALL pass.
3. WHEN `Pullup` is replaced by `Pulldown` THEN the system SHALL print the records of the Book's exercise answer.

**Independent Test**: `cargo test -p piperine-cli --test book`.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md`, the Book and the spec, I want them to say that these natures run.

**Why P3**: No behaviour depends on it.

**Acceptance Criteria** (RN-10):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L21, L22, L23, nor the resolved-net group of L58 except the items whose other dependency is still open.
2. WHEN this feature merges THEN Appendix C.2.3 and Appendix G.38 SHALL say that `Tran` on `Wire`s runs, with a `spec_text` guard (MD-57).
3. WHEN a task first raises a code that `crates/piperine-lang/tests/support/code_owners.tsv` assigns to `resolved-nets` (`E0203`, `E0224`, `E0242`, `E0413`) THEN that commit SHALL delete the code's row, so the guard `spec_codes` passes (design D16).

**Independent Test**: `cargo test -p piperine-lang --test spec_text` and `cargo test -p piperine-cli --test docs`.

---

## Edge Cases

- WHEN a resolved carrier is a top-level `in` port THEN the system SHALL pass the host's drive first (§4 rule 2, Resolution).
- WHEN `resolve` itself panics (`xs[0]` on the empty array of a carrier nobody drives) THEN the system SHALL report the panic as a run fault naming the nature and the carrier.
- WHEN a `SumNet` drive is unknown THEN the system SHALL make the sum unknown (§22.3.4 rule 3).
- WHEN a resolved carrier is a child's `pub` signal THEN the parent SHALL be allowed to drive it (§5.2.2: any module that sees it).
- WHEN a loop's drives settle on the first pass THEN the system SHALL write no `log` record.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| RN-01 | P1: A project declares its own discrete nature | T1–T2 | In Tasks |
| RN-02 | P1: A nature with `resolve` is declared and checked | T11, T16, T19, T20 | In Tasks |
| RN-03 | P1: Drives combine through `resolve` | T12–T14, T17, T18, T22, T33 | In Tasks |
| RN-04 | P1: `others()` | T3, T15, T23 | In Tasks |
| RN-05 | P1: Loops among resolved drives settle by relaxation | T7–T9, T21, T24, T25 | In Tasks |
| RN-06 | P1: The stdlib nets of §22.3 | T26–T31 | In Tasks |
| RN-07 | P1: `Bidir` | T32 | In Tasks |
| RN-08 | P1: A signal-flow node | T4–T6 | In Tasks |
| RN-09 | P2: The Book's alarm line | T34, T35 | In Tasks |
| RN-10 | P3: The documents say what is true | T10, T36 | In Tasks |

**Coverage:** 10 total, 10 mapped to tasks (tasks.md, T1–T36), 0 unmapped.

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| L21 | natures with `resolve`: `Wire`, `Tri`, `SumNet`; `others()`; relaxation of loops | RN-02, RN-03, RN-04, RN-05, RN-06 |
| L22 | a discrete nature the project declares | RN-01 |
| L23 | signal-flow natures (`VSignal`) | RN-08 |
| L25 | bundles | `resolved-nets-bundles` |
| L58, resolved-net group | `Logic`, `Strength`, `Wire`, `Tri`, `SumNet`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir` | RN-06, RN-07 (`TribufDelayed`: `generics`, with `inertial`) |
| U5, the alarm row | the alarm line shown with its refusal | RN-09 (the rest of U5 is `root-e2e`'s) |

## Dependencies

- Phase 1 (RN-01, RN-08, RN-04 AC 3, RN-05 AC 2, AC 6 and AC 7 below the language): none; starts from `main` at `7df9e17c`.
- Phase 2 (RN-02 to RN-05 over `SumNet` and project natures): `composite-values` phases A–B (structs, arrays `[T; N]`, iterators H.7, closures, carried composites T57) and `generics` phase A (const-generic functions and their `Instantiations`), both on `main`; `value-semantics` (on `main` with `composite-values`) for the `std/nets.msl` spelling.
- Phase 3 (RN-06, RN-07): phase 2, and `generics` phase A's `impl` on a value type with associated constants (`impl Logic`, `Logic::Z`).
- Phase 4 (RN-09, RN-10): phases 1 to 3.
- Already delivered: `msl-fixes` L17, `analog-structure` L40, `host-ii` L56, `unknown-semantics` US-01 to US-04.
- Benefits from, does not wait for: `unknown-semantics-per-level` (exact levels in RN-05 AC 4).
- Downstream: `resolved-nets-bundles` (a bundle field of a resolved nature has no direction), on the same branch.

## Size

**L.** 36 tasks in 9 phases, 7 batches (tasks.md): 10 independent (phases 1 to 3, T1 to T10), 23 waiting for `composite-values` A–B and `generics` A (phases 4 to 8), 3 for the Book and the records (phase 9).

## Sources

- `GAPS.md` §4.2 L21, L22, L23; L58; §8 U5 (the alarm).
- `docs/spec/part_ii_carriers.md` §4 (rules 1 to 16, errors), §5.2, §6 (rules 2, 10, errors), §6.1; `part_iii_structure.md` §8.1 (`E0310`), §8.2.3; `part_vii_stdlib.md` §22.3, §22.4.3; `part_vi_io.md` §19 rule 2; `appendix_c_implementation.md` C.2 and C.2.3; `appendix_e_gallery_digital.md` E.7; `appendix_f_gallery_mixed.md` F.4.
- `.specs/features/device-abi/abi.md` §9 D1, `RELAX`; `crates/piperine-digital-solver/src/dag.rs`, `run.rs`; `crates/piperine-lang/src/types/carrier.rs` (today's refusals); `crates/piperine-lang/std/nets.msl`, `carriers.msl`.
- `docs/book/content.html`, chapter "Natures and carriers": the alarm and its exercise.
- `.specs/STATE.md` MD-25, MD-54, MD-56 D4, MD-57.

---

## Success Criteria

- [ ] No `P0001` remains for a project discrete nature, a resolved nature, a signal-flow nature or a §22.3 item.
- [ ] `Tran` on `Wire`s (Appendix C.2.3) runs and settles; two `Bidir`s alone are unknown and logged.
- [ ] The Book's alarm prints three records.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

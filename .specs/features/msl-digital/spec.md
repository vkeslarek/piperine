# Digital and Mixed MSL Through the Front End Specification

**Approved**: 2026-11-15 - user: "Sim, por favor! Achei que já tivesse no escopo completo" (standing delegation)

## Problem Statement

M1 runs analog MSL only: a digital block, a commit, an integer or an enum is `P0001` at type check. `ir-rust` phase B gives the IR, the plan and the generated Rust their digital and mixed forms, but proves them on IR built by hand. Nothing turns digital or mixed MSL source into that IR, so `examples/buck`, the flagship model, parses and resolves and does not run. This feature is the front end for those forms: type check and elaboration of the digital and mixed constructs the buck uses, into the IR node set `ir-rust` defines, so that the buck runs from its `.msl` on the transpiled backend.

The buck reaches further than the `ir-rust` slice. It uses enums and `match`, `Option`, the stdlib modules `transition`, `A2D`, `above`, `init` and `Never`, the stdlib functions `log` and `require`, timers, `now()`, `analysis(Static)`, and an inductor written with a flow probe. Each is in scope here, with the IR, plan and transpiler additions it needs stated as amendments to `ir-rust`.

## Goals

- [ ] `examples/buck` type-checks and elaborates from source with zero diagnostics, and its IR passes `Ir::check()`.
- [ ] The buck modules that `ir-rust` T29 transcribed by hand elaborate from source to the same structural IR, or to a superset whose every difference is listed.
- [ ] The behaviors `ir-rust` phase 8 asserts on hand-built IR (oscillator, DPWM, ramp and PID, ADC, comparator, the two crossings) hold on IR elaborated from source.
- [ ] The whole buck compiles on the transpiled backend and its closed loop settles at the setpoint within the tolerance of `examples/buck/SPEC.md` §2.
- [ ] No method call stays unresolved without a diagnostic (buck `GAPS.md` G3).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The merge of effects under unknown control (§3.3.3) | `ir-rust` RS-11 AC 5 makes an unknown condition a run-time fault; the merge is M2. The buck gives every decision a `known_or` |
| Loops, `let mut`, assignment to locals, structs, arrays, indexing, closures other than the `limit` function, user traits and `impl`s, generic modules and functions, enums with a payload or an explicit discriminant | not used by the buck; each stays `P0001` by name (DIG-07) |
| Resolved carriers (`resolve`, `Wire`, `Tri`, `others()`), bundles, `flip`, `prev()`, `settle`, element and field commits | not used by the buck; `P0001` |
| The stdlib beyond what the buck reaches: `D2A`, `Bidir`, `slew`, `idt`, `idtmod`, the `zi_*` and `laplace_*` filters, `inertial`, `absdelta`, `last_crossing`, the monitors other than `Never`, the nets of §22.3 | M2 remainder (ROADMAP queue item 5). Those that use only forms in scope may elaborate; nothing here asserts it |
| `delay`, `noise`, `ddx`, `bound_step`, `freq()`, `simparam`, `analysis` of a kind other than `Static`, flow probes of a flow branch, selectors and switch branches, contributions under `if` | analog forms the buck does not use; `P0001` |
| Generation `for`/`if`, templates, `extern` modules, `m`, `dtemp` | not used by the buck; `P0001` |
| A `ttol` or `etol` given to `cross`; `at` whose argument reads a param the host `set`s during a run | the `ir-rust` `Cross` event carries neither tolerance, and re-arming on a `set` is host surface; a given tolerance is `P0001` |
| Extending the interpreter | R1. It refuses every new form by name |
| A host API, the CLI, `piperine-project`, Python | queue items 3 and 4. Tests drive the buck from MSL bench modules and Rust tests |
| The bench scenarios S1 to S9 of buck SPEC §10 at their stated operating points | queue item 4 (root e2e). This feature proves one closed-loop start at 0.8 V and S10 |
| Re-deriving a child's param when the host `set`s a parent param during a run | host surface; recorded in `GAPS.md` |
| Variant names of an enum on the wire | a `Bit<Mode>` carrier is `UInt<2>` in the IR and the ABI; names belong to the host surface |
| Spec text for G4 and G5 | owed to `docs/spec/`; the provisional rules of ROADMAP apply unchanged |
| Changes to `piperine-abi` and the three solver crates | a defect found there is reported with a reproducing ABI device, never patched here |

---

## Assumptions & Open Questions

Every gray area is settled under the user's standing delegation ("pode ir disparando implementações", "Pode ir rodando os grandes passos"). `y (delegation)` marks an answer given that way; plain `y` marks one a recorded decision already fixes.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Scope of "the buck runs from source" | the whole package, with every stdlib item it reaches elaborated from the embedded stdlib source | the user's sentence is the acceptance; the stdlib is ordinary source (Appendix C.1.1), so special-casing `transition` would be a second language | y (delegation) |
| IR forms the buck needs and `ir-rust` lacks | six amendments, built here in `piperine-ir`, `piperine-plan` and `piperine-transpile`: `now()`, `analysis(Static)`, the `at` event, the `emit!` site, a flow probe with `ddt` under a potential contribution, a child param that reads the parent's params | the brief: state it as an amendment, do not redesign the IR. Each mirrors one source form, as the `ir-rust` design asks of M2 | y (delegation) |
| Unknown-ness in the type system | types are the source's own (`u9`, not "maybe-unknown `u9`"); the elaborator computes which expressions may be unknown and gives those the IR type `Flagged` | §3.3: the flag travels with every value and is not part of its type | y (delegation) |
| Functions called with possibly unknown arguments | one IR function per function and flag pattern of its arguments; the all-known variant keeps the source name, the others are named `name~bits` | IR functions have fixed parameter types (S7 keeps functions as functions) | y (delegation) |
| Enums | an enum of unit variants with `#[derive(Repr)]` is its tag: `UInt<ceil(log2 V)>`, discriminants from 0 in declaration order; `match` is an `if` chain | §2 rule 5; no IR node is needed | y (delegation) |
| `Option` | never carried and never an IR type: the elaborator replaces an `Option<T>` value by a `bool` and a `T` | the buck uses `Option` only as a local and as `known()`; scalar replacement needs no IR node | y (delegation) |
| `&&` and `\|\|` with a possibly unknown operand | lowered to `&` and `\|`, which §3.3.12 says give the same result; refused (`P0001`) when the right operand can panic or emit | the IR's short-circuit forms take known operands | y (delegation) |
| Unknown control at run time | a fault, as `ir-rust` RS-11 AC 5; an unknown guard is the same fault | inherited | y |
| `rise()`, `fall()`, `known()`, `known_or()`, `is_known()`, `high()`, `changed()` | typed and lowered as built-ins when the name resolves to the stdlib's own item; a project's replacement is `P0001` | §3.3.7 names four of them as functions every implementation treats specially; their stdlib bodies use `level`, traits and loops, which are out of scope | y (delegation) |
| `rise()` and `fall()` on a change to unknown | `rise` is `changed` guarded by "known and true", `fall` by "known and false"; a change from false to unknown is not a rise | the `ir-rust` contract (its "rise(), fall() in the IR" row); the IR has no `prev()`. Recorded in `GAPS.md` as a deviation from §22.1 | y (delegation) |
| A module with a result | hoisted: its carriers, drives, events and reactions join the enclosing body under the placement's name; the result is inlined at each read; no IR body or instance is made for it | Appendix C.1.1 ("inlining at each read; their instances are hoisted to the module") and §8.3.12. Amends S6 and S7 for result modules only. The POM keeps the placement | y (delegation) |
| A guard that reads a probe, `now()` or `analysis(Static)` | becomes an `if` around the reaction body; the discrete part of the guard stays on the event | abi.md F2 and R1: only the non-probing part of a guard is a discrete function | y (delegation) |
| An `\|\|` head whose disjuncts carry such guards | one IR reaction per disjunct; refused (`P0001`) when the body holds `emit!` | reactions read one snapshot (§12.1.1), so two runs commit equal values; a record would be written twice | y (delegation) |
| An event read as a `bool` in a reaction body (§13 rule 2) | `P0001` | not used by the buck; needs an IR read the contract lacks | y (delegation) |
| Records | one `RECORD` pin per stream and instance; a `&str` field must be a literal after inlining and stays in the IR site, off the wire; `maybe` is always false | abi.md C5; the ABI has no string leaf; unknown control faults, so no record is reached under it | y (delegation) |
| A `fn` that calls `emit!` (`log`, `require`) | inlined at its call in a reaction; it gets no IR function | IR functions are pure values | y (delegation) |
| Integer literal typing | local: the expected type where one exists, else `i32` | §1 rule 7 "as in Rust", without inference across statements; a model Rust accepts through later-use inference is rejected with `P0002`, never mistyped | y (delegation) |
| A float `as` to an integer, and a shift by the width or more | G4 and G5 provisional rules, also when folded at elaboration | ROADMAP "MSL spec gaps" | y |
| Child param values that read the parent's params | evaluated per instance at elaboration; the flat table holds the constant | S6. A host `set` of the parent does not re-derive: host surface | y (delegation) |
| A top module with ports | discrete ports become design nets of the top instance, driven by the host; a continuous port on the top stays `P0001` | §5.2.3: the host drives a top-level `in` port | y (delegation) |
| Codes | the `Eppnn` code where `docs/spec/` names one; `P0001` for a form not supported yet; `P0002` for a plain type mismatch, a non-exhaustive `match` included | Q5 of M1 | y |
| Where run-time tests live | `crates/piperine-parity/tests/`, driven by MSL bench modules in a bench crate that depends on the buck package | the only crate that links the front end, the transpiler and the solver; no host API exists | y (delegation) |
| The oracle | `docs/spec/` and `examples/buck/SPEC.md`, written as expected values computed in each test; analytic responses under the unchanged `ToleranceContract` | R2 | y |
| Closed-loop operating point | setpoint code 0 (0.8 V), Vin 12 V, 6.6 Ω, 600 µs | the shortest start the SPEC defines: 128 samples of ramp plus 64 of power-good | y (delegation) |
| Who implements and verifies | Sonnet implements, Opus implements the delicate tasks, a different Opus verifies | S10 | y |
| Start | after `ir-rust` phases A and B merge; T1 reconciles this design with the merged code before any other task | the `ir-rust` design is the contract and its code is not merged yet | y |

**Open questions:** none - all resolved or logged above.

**Implicit-requirement dimensions.** Input validation and bounds: DIG-01 AC 3, 9 and 10, DIG-04 AC 2 and 5, DIG-07. Failure states: every `IF` criterion; DIG-08 AC 9; a solver defect stops the task (tasks.md rules). Idempotency: DIG-09 AC 12. Auth and rate limits: N/A because everything is local. Concurrency and ordering: DIG-09 AC 8 and 10 (source order), DIG-14 AC 7 (two runs). Data lifecycle: N/A because elaboration keeps no state. Observability: DIG-02 (no silent unresolved call), DIG-11 (records), `GAPS.md` (DIG-17). External-dependency failure: `rustc` and ngspice follow `ir-rust` and `msl-parity`. State-transition integrity: DIG-15 AC 4 and 5 (the supervisor's table).

---

## User Stories

### P1: Integers ⭐ MVP

**User Story**: As a model author, I want integer types, literals and operators checked so that a counter is typed as I wrote it.

**Why P1**: every digital module of the buck.

**Requirement**: DIG-01

**Acceptance Criteria**:

1. The system SHALL give a type to `UInt<N>`, `Int<N>`, `uN` and `iN` for `N` in 1..=64 and to `const` items of those types.
2. WHEN an integer literal stands where an integer type is expected (an annotation, a commit or definition target, a parameter, the other operand of a binary operator, another arm of the same `if` or `match`, the receiver of the method it is an argument of) THEN the system SHALL give it that type, and otherwise `i32`.
3. IF an integer literal is outside the range of its type THEN the system SHALL reject the model with `E0909`.
4. The system SHALL type `+`, `-`, `*`, `/`, `%`, unary `-`, `&`, `|`, `^`, `!` and the six comparisons on operands of one integer type.
5. IF the two operands of such an operator have different types THEN the system SHALL reject the model with `P0002`.
6. The system SHALL type `<<` and `>>` with a left operand of an integer type and a right operand of any integer type, the result having the left type.
7. The system SHALL type `as` between any two of the integer types and `f64`.
8. The system SHALL type `min`, `max` and `clamp` on the integer types and on `f64`, and `floor` on `f64`, each giving the receiver's type.
9. IF a reached type is `UInt<N>` or `Int<N>` with `N > 64`, or a float type other than `f64`, THEN the system SHALL reject the model with `P0001` naming the type.
10. IF an integer width is below 1 THEN the system SHALL reject the model with `E0103`.

**Independent Test**: check `fn f(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }` and read no diagnostic.

---

### P1: Every method call links or is reported ⭐ MVP

**User Story**: As a model author, I want a method on a carrier either resolved or refused so that a typo or a gap never runs as nothing.

**Why P1**: buck `GAPS.md` G3, a silent gap.

**Requirement**: DIG-02

**Acceptance Criteria**:

1. WHEN a method of the catalogue is called on a discrete carrier whose value type has it THEN the system SHALL type the call on the carrier's value.
2. IF a method call of the reachable set names no catalogue row of its receiver's type THEN the system SHALL reject the model with `P0001` naming the method and the receiver's type.
3. The system SHALL record a callee or a catalogue row for every call and method call of a model it accepts.

**Independent Test**: check a reaction holding `oc_count.saturating_add(1)` on a `Bit<u4>` and read `P0001` naming `saturating_add` and `u4`.

---

### P1: Enums, `Option`, tuples and `match` ⭐ MVP

**User Story**: As a model author, I want a state machine written with an enum and a `match` so that the supervisor reads like its table.

**Why P1**: `Supervisor`, `SetpointReg`, `transition`.

**Requirement**: DIG-03

**Acceptance Criteria**:

1. The system SHALL type an enum of unit variants with `#[derive(Repr)]`, its variant paths, and `==` and `!=` on it when it derives `PartialEq`.
2. IF a reached enum has a variant with a payload, an explicit discriminant or a `#[repr]` attribute THEN the system SHALL reject the model with `P0001` naming the form.
3. The system SHALL type `Option<T>`, `Some(e)`, `None`, and the methods `is_some`, `is_none` and `unwrap_or`.
4. The system SHALL type `match` over an enum, a `bool`, an integer or an `Option`, with path, literal, `Some(p)`, `None`, or- and wildcard patterns, the arms having one type.
5. IF a `match` does not cover every value of its scrutinee THEN the system SHALL reject the model with `P0002`.
6. The system SHALL type `if let Some(x) = e`, binding `x` to the payload in the first block.
7. The system SHALL type `let (a, b) = (x, y);` in a function, a reaction and a module body, naming each component.

**Independent Test**: check `Supervisor` of the buck alone and read no diagnostic.

---

### P1: Discrete carriers and their drives ⭐ MVP

**User Story**: As a model author, I want carriers, definitions and connections checked against the driver rules so that two writers or a wrong direction is an error at my line.

**Why P1**: Part II §5; every module boundary of the buck.

**Requirement**: DIG-04

**Acceptance Criteria**:

1. The system SHALL type a `signal`, an `in` port and an `out` port of the nature `Bit<T>` for `T` one of `bool`, an integer type, `f64` or a unit enum.
2. IF the carried type has no `Repr` THEN the system SHALL reject the model with `E0112`.
3. The system SHALL give a carrier read in a definition, a guard or a reaction the carrier's value type.
4. The system SHALL type `known()`, `known_or(d)`, `is_known()` and `high()` on a carrier and on a value, when the name resolves to the stdlib's own item.
5. IF the initial value of a carrier has another type, or reads a carrier or a probe, THEN the system SHALL reject the model with `E0210`.
6. IF a definition's expression has a type other than its target's THEN the system SHALL reject the model with `E0337`.
7. IF a definition, its delay or an expression connected to an `in` port reads a probe, `now()` or `analysis(Static)` THEN the system SHALL reject the model with `E0336`, or `E0316` at an `in` port.
8. IF the target of a definition or a commit is an `in` port, a child's carrier or not a discrete carrier THEN the system SHALL reject the model with `E0222`.
9. IF one module holds two definitions of one carrier THEN the system SHALL reject the model with `E0335`.
10. IF one module holds a definition and a commit of one carrier THEN the system SHALL reject the model with `E0221`.
11. IF a carrier is connected to two `out` ports, or to one while its module also drives it, THEN the system SHALL reject the model with `E0220`.
12. IF a `signal` with an initial value is connected to an `out` port THEN the system SHALL reject the model with `E0223`.
13. IF a carrier driven by an undelayed definition has an initial value THEN the system SHALL reject the model with `E0338`.
14. IF an expression, or a carrier the parent may not hand over, is connected to an `out` port THEN the system SHALL reject the model with `E0314`.
15. IF a carrier is connected to a port of another value type THEN the system SHALL reject the model with `E0251`.
16. IF undelayed definitions form a loop THEN the system SHALL reject the model with `E0339`.

**Independent Test**: check `GateDriver` of the buck; then give `hs_gate` a second definition and read `E0335`.

---

### P1: Events, reactions and commits ⭐ MVP

**User Story**: As a model author, I want `on` heads and reaction bodies checked so that a `bool` head or a commit of the wrong type is caught before a run.

**Why P1**: Part IV §12 and §13.

**Requirement**: DIG-05

**Acceptance Criteria**:

1. The system SHALL type `c.changed()`, `c.rise()`, `c.fall()`, `cross(e)`, `at(t)`, `a || b` and `ev && guard` as `Event`.
2. IF the head of an `on` is not an `Event` THEN the system SHALL reject the model with `E0430`.
3. IF `!` is applied to an `Event` outside a guard, or `&&` has a `bool` on the left and an `Event` on the right, THEN the system SHALL reject the model with `E0440`.
4. IF an `Event` is written outside an event position or committed THEN the system SHALL reject the model with `E0441`.
5. IF the argument of `at` reads a probe, `now()` or `analysis(Static)` THEN the system SHALL reject the model with `E0442`.
6. IF `cross` or `at` is placed outside an event position THEN the system SHALL reject the model with `E0443`.
7. The system SHALL type a reaction body holding `let`, `if`, `match`, calls, commits with or without `after`, probes and environment reads.
8. IF a committed expression has a type other than its target's THEN the system SHALL reject the model with `E0432`.
9. IF the delay of a commit or a definition is not `f64` THEN the system SHALL reject the model with `E0433`.
10. IF a module is placed in a reaction body, a `fn` or a closure THEN the system SHALL reject the model with `E0350`.
11. The system SHALL type `emit!` in a reaction, each field having a value type.
12. IF a field name is repeated in one `emit!` or is `maybe` THEN the system SHALL reject the model with `E0604`.
13. IF a field's type conflicts with another site of its stream, or with a reserved stream's field, THEN the system SHALL reject the model with `E0605`.
14. IF `emit!`, or a function that calls it, is reached from a definition, a module-body `let` or a const context THEN the system SHALL reject the model with `E0601`.
15. IF a site reached from `analog` writes `sim` or `checks` THEN the system SHALL reject the model with `E0603`.

**Independent Test**: check `on clk { q <- d; }` with `clk: Bit` and read `E0430` at `clk`.

---

### P1: Modules with a result, time and the analysis ⭐ MVP

**User Story**: As a model author, I want `transition(...)` and `above(...)` usable inside expressions so that a stateful operator is one call.

**Why P1**: `PowerSwitch`, `Load` and `A2D` of the buck.

**Requirement**: DIG-06

**Acceptance Criteria**:

1. The system SHALL type a module declared with value inputs and `-> T`, its last body expression having type `T`, and SHALL reject another type with `E0301`.
2. The system SHALL give a placement of such a module, in a body `let`, an `analog` block, an event position or an argument of another placement, the type `T`.
3. WHEN trailing `Option` value inputs are left out THEN the system SHALL accept the placement, and SHALL reject too few or too many arguments with `E0141`.
4. IF a module is placed in a const context or a bound param value THEN the system SHALL reject the model with `E0352`.
5. The system SHALL type `now()` as `f64` and `analysis(Static)` as `bool` in a module body, an `analog` block and a reaction, and SHALL reject either in a const context with `E0323`.
6. IF an `analog` block reads a discrete carrier, directly, through a `let` or through a value input, other than as the receiver of `known`, `known_or`, `is_known` or `high` THEN the system SHALL reject the model with `E0150` naming the carrier.
7. IF a module-body `let` reads its own name THEN the system SHALL reject the model with `E0332`, or with `E0353` when the cycle passes through a result that depends directly on its input.
8. WHEN an argument of type `T` is given for a parameter or value input of type `Option<T>` THEN the system SHALL read it as `Some` of it (§3.2.3).

**Independent Test**: check `PowerSwitch` of the buck; then read `g` bare in its `analog` block and read `E0150` naming `g`.

---

### P1: Nothing outside the subset passes ⭐ MVP

**User Story**: As a maintainer, I want every form still unsupported refused by name so that nothing runs with a guessed meaning.

**Why P1**: fail loud.

**Requirement**: DIG-07

**Acceptance Criteria**:

1. IF a reached construct is a loop, `let mut`, an assignment, a struct, an array or an index, a closure that is not a `limit` function, a user `trait` or `impl`, a generic module or function, a nature with `resolve`, a bundle, `prev()`, `others()`, `settle`, an element or field commit, `delay`, `noise`, `ddx`, `bound_step`, `freq()`, `simparam`, `analysis` of a kind other than `Static`, a generation `for` or `if`, a template or an `extern` module THEN the system SHALL reject the model with `P0001` naming the construct.
2. The system SHALL leave the printed IR of the seven M1 twins and their type-check results unchanged.
3. The system SHALL type-check `Buck` of `examples/buck` with zero diagnostics.

**Independent Test**: check a reaction holding a `for` loop and read `P0001` naming the loop.

---

### P1: Values become IR ⭐ MVP

**User Story**: As a maintainer, I want integer, enum and `Option` code lowered to the IR's own value nodes so that the backends see nothing new.

**Why P1**: the IR is the contract with `ir-rust`.

**Requirement**: DIG-08

**Acceptance Criteria**:

1. The system SHALL lower integer types, literals, operators, casts and methods to the IR's `UInt`, `Int`, `Int` literal, `BinaryOp`, `Cast` and `Method` nodes.
2. WHEN an integer expression reads only literals and constants THEN the system SHALL fold it: `N_PERIOD - 1` with `N_PERIOD: u9 = 400` is the literal 399 of `u9`.
3. The system SHALL fold `300.0 as u8` to 255, `1u8 << 9` to 0 and `511u9 + 1` to 0, the values the compiled code gives.
4. IF a constant expression divides an integer by zero THEN the system SHALL reject the model with `E0104`.
5. The system SHALL give a carrier read, `Unknown` and every operator result with such an operand the IR type `Flagged`, and SHALL give `known_or`, `is_known` and `high` an unflagged type.
6. WHEN an operand of `&&` or `||` is `Flagged` THEN the system SHALL lower the operator to `BitAnd` or `BitOr`, and otherwise to `And` or `Or`.
7. IF the right operand of an `&&` or `||` with a `Flagged` operand holds an integer `/` or `%` or a call that emits THEN the system SHALL reject the model with `P0001`.
8. WHEN a function is called with `Flagged` arguments THEN the system SHALL lower one IR function per function and flag pattern, the all-known one keeping the source name.
9. The system SHALL lower an enum of `V` unit variants to `UInt<ceil(log2 V)>`, variant `k` to the literal `k`, and a `match` to an `if` chain that evaluates its scrutinee once.
10. The system SHALL lower an `Option<T>` value to a `bool` and a `T`, with `Some`, `None`, `known()`, `is_some`, `is_none`, `unwrap_or`, `if let` and `match` written over the pair, and SHALL produce no `Option` type in the IR.
11. The system SHALL lower `let (a, b) = (x, y);` to one local per component.

**Independent Test**: elaborate a module holding `rev3` and `dithered_width` of the buck and print its functions.

---

### P1: Structure becomes IR ⭐ MVP

**User Story**: As a maintainer, I want carriers, drives, events and reactions lowered one to one so that the printed IR reads like the module.

**Why P1**: the node set of `ir-rust` T24 to T28 is the target.

**Requirement**: DIG-09

**Acceptance Criteria**:

1. The system SHALL lower each discrete port and signal of a module to one carrier of its body, ports first and then signals in declaration order, with name, value type, role and folded initial value.
2. The system SHALL lower each definition to one drive, with its `after` when written.
3. WHEN an expression is connected to an `in` port THEN the system SHALL add to the parent's body a signal carrier named `<instance>.<port>` and a drive of it, and SHALL bind the child's port to it.
4. The system SHALL make one design net per signal carrier of each instance, named by its path, and SHALL bind every carrier port of an instance to a net of its parent.
5. WHEN the top module has discrete ports THEN the system SHALL make one net per port, named `<Top>.<port>`, and SHALL reject a continuous port on the top with `P0001`.
6. The system SHALL lower `c.changed()` to a `Changed` event, `c.rise()` to `Changed` guarded by `c.high()`, `c.fall()` to `Changed` guarded by `c.is_known() & !c.known_or(true)`, and `cross(e)` to a `Cross` event.
7. WHEN a guard reads a probe, `now()` or `analysis(Static)` THEN the system SHALL lower that part as an `if` around the reaction body and SHALL keep the rest of the guard on the event.
8. WHEN a head is an `||` of events THEN the system SHALL lower one reaction listing each event, or one reaction per disjunct in source order when a disjunct's guard reads a probe, `now()` or `analysis(Static)`.
9. IF a head needs one reaction per disjunct and its body holds `emit!`, or a reaction body reads an event as a `bool`, THEN the system SHALL reject the model with `P0001`.
10. The system SHALL lower a reaction body to `let`, `if` and commit statements in source order, each commit with its `after`, and each probe to a probe of the body's node.
11. WHEN a placement gives a param a value that reads the parent's params THEN the system SHALL evaluate it per instance, SHALL write the constant in the instance table, and SHALL reject a false `where` with `E0317` at the placement.
12. WHEN one design is elaborated twice THEN the system SHALL give IRs with equal prints.
13. The system SHALL leave `Design::dump()` unchanged by elaboration.
14. The system SHALL return only IRs whose `check()` is empty.

**Independent Test**: elaborate `RefRamp` of the buck and read two carriers in, three out, one drive, one event and one reaction.

---

### P1: Result modules are hoisted ⭐ MVP

**User Story**: As a model author, I want a placed `transition` to be state of my module so that its result is an expression my `analog` block reads.

**Why P1**: §8.3.12; `PowerSwitch` and `A2D` depend on it.

**Requirement**: DIG-10

**Acceptance Criteria**:

1. WHEN a module with a result is placed THEN the system SHALL add its signals, drives, events and reactions to the enclosing body, named `<name>.<item>`, where `<name>` is the body `let` it initializes or else `<module>$<k>` with `k` counting that module's placements in source order from 0.
2. The system SHALL lower each read of a result to the callee's result expression with every value input replaced by the caller's expression.
3. WHEN a trailing `Option` value input is left out THEN the system SHALL read it as `None`, so that `td.unwrap_or(0.0)` is the literal `0.0`.
4. WHEN a module places the same result module twice THEN the system SHALL give each placement its own carriers.
5. The system SHALL make no IR body and no instance for a module with a result.
6. WHEN an expression is given to a discrete `in` value input of a result module THEN the system SHALL lower it as a drive of the hoisted carrier of that input.
7. The system SHALL lower `at(t)` in an event position to an `At` event, `now()` to the IR's time read and `analysis(Static)` to its static read.

**Independent Test**: elaborate `PowerSwitch` of the buck and read the carriers `gate.xd`, `gate.y0`, `gate.y1`, `gate.t0`, `gate.dt` and `gate.x` in its body.

---

### P1: Records ⭐ MVP

**User Story**: As a model author, I want `require` and `log` to write records so that the shoot-through monitor of the buck reports.

**Why P1**: buck SPEC §10 S10; `Never` and `log` are in the package.

**Requirement**: DIG-11

**Acceptance Criteria**:

1. The system SHALL lower `emit!("s", f = e, …)` in a reaction to one emit statement naming the stream and its fields in source order, a string literal field being kept as text in the site.
2. WHEN a reaction calls a function that calls `emit!` THEN the system SHALL inline the function at the call and SHALL make no IR function for it.
3. IF a `&str` field is not a literal after inlining THEN the system SHALL reject the model with `P0001`.
4. WHEN `Never(clk, bad)` sees a rising `clk` THEN the system SHALL deliver to the sink one record of the stream `checks` with `pass` known and equal to `!bad`, and SHALL add no microstep.
5. WHEN `log(Severity::Warning, "x")` runs in a reaction THEN the system SHALL deliver one record of the instance's stream `log` with `severity` 1, and the site SHALL give the text `x` for `msg`.

**Independent Test**: run `Never` on a clock with `bad` held true and count one failed record per rising edge.

---

### P1: Timers, time and the analysis at run time ⭐ MVP

**User Story**: As a model author, I want `at`, `now()` and `analysis(Static)` to behave as §13 and §14 say so that `transition` ramps and rests.

**Why P1**: `transition` is built on the three.

**Requirement**: DIG-12

**Acceptance Criteria**:

1. WHEN `at(t)` is armed at `(0, 0)` with `t` later than 0 THEN the system SHALL fire it once, at the tick nearest `t`.
2. WHEN a carrier in the argument of `at` changes its value THEN the system SHALL fire at the new time and SHALL not fire at the earlier one.
3. WHEN the argument of `at` is already past or unknown THEN the system SHALL not fire, and WHEN it equals the current time the system SHALL fire at the next microstep.
4. The system SHALL evaluate `now()` in a reaction as the time of its instant and in `analog` as the time of the solve.
5. The system SHALL evaluate `analysis(Static)` as true in an equilibrium solve and its instants and as false in a transient step.

**Independent Test**: a reaction on `at(t0 + dt)` with `t0` committed to 10 ns and `dt` to 5 ns fires at 15 ns.

---

### P1: The analog forms of the power stage ⭐ MVP

**User Story**: As a model author, I want an inductor written with its flow so that the power stage is plain MSL.

**Why P1**: `Inductor`, `CurrentSense`, `PowerSwitch` and `Load` of the buck.

**Requirement**: DIG-13

**Acceptance Criteria**:

1. The system SHALL type and lower `i(a, b)` read in a module that gives the branch `(a, b)` a potential contribution, as a read of that branch's flow.
2. The system SHALL lower a potential contribution with a `ddt` addend to a potential contribution holding a `Ddt` addend.
3. IF `i(a, b)` reads a branch the module gives no potential contribution THEN the system SHALL reject the model with `P0001`.
4. WHEN a source steps from 0 V to `V` at `t0` into a resistor `R` in series with `Inductor { l, dcr }` THEN the system SHALL give a current that agrees with `V / (R + dcr) · (1 − exp(−(t − t0) · (R + dcr) / l))` under the transient `ToleranceContract`.
5. The system SHALL give `CurrentSense` at an operating point `v(pos)` equal to `gain · v(a, b)` and `v(neg)` equal to its negative under the operating-point `ToleranceContract`.
6. WHEN the gate of `PowerSwitch` is committed high at `t0` THEN the system SHALL give a channel conductance of `1 / roff` before `t0`, `(1 − k) / roff + k / ron` at `t0 + k · tsw` for `k` in 0..=1, and `1 / ron` after, read through a series resistor under the transient `ToleranceContract`.
7. WHEN that gate changes THEN the system SHALL make `t0` and `t0 + tsw` time points.

**Independent Test**: run the RL circuit and compare the current at five accepted times.

---

### P1: The buck slice from source ⭐ MVP

**User Story**: As a maintainer, I want the hand-built fixtures of `ir-rust` reproduced from source so that the front end and the backend meet on one IR.

**Why P1**: the acceptance of the feature.

**Requirement**: DIG-14

**Acceptance Criteria**:

1. The system SHALL elaborate `Osc`, `SetpointReg`, `RefRamp`, `Pid`, `GateDriver` and `Adc` of the buck to bodies whose structural print equals the `ir-rust` T29 fixture of the same name.
2. The system SHALL elaborate `Dpwm` and the stdlib `A2D` to bodies that hold every item of their T29 fixture with an equal print, plus only the items the test lists: the `Never` placement of `Dpwm` with its carrier and drive, and the `init` carrier, event and reactions of `above` in `A2D`.
3. WHEN the oscillator elaborated from source (`f` = 200 MHz) runs for 100 ns THEN the system SHALL change `clk` at `k × 2.5 ns` for `k` = 1 to 40 and at no other time.
4. WHEN `Dpwm` from source runs with `run` true and duty word 1325 for 8 periods THEN the system SHALL hold `hs` true for 166 counts in the periods whose bit-reversed frame index is below 5 and 165 in the others, hold `ls` true from 4 counts after `hs` falls to count 396, and never hold both.
5. WHEN `RefRamp` and `Pid` from source run a scripted sequence of 200 samples THEN the system SHALL commit, sample by sample and bit for bit, the `reference`, `duty`, `integ`, `deriv` and `e_prev` of the difference equations of buck SPEC §6.2 to §6.4 and §7.2.
6. WHEN `Adc` from source sees a rising `sample` with its input held at `V` THEN the system SHALL commit `code` = `floor(V / vref × 1024 + 0.5)` clamped to 0..=1023 after `tconv`, with `done` true from `tconv` to `tconv + tstrobe`.
7. WHEN one circuit of the slice runs twice THEN the system SHALL give the same sequence of pin changes, bit for bit and tick for tick.
8. WHEN `A2D` from source watches an RC charge and then its discharge THEN the system SHALL set `d` true where the analytic input equals `vth + hyst / 2` and false where it equals `vth − hyst / 2`, each under the transient `ToleranceContract`.
9. WHEN the input of `A2D` is above `vth + hyst / 2` at the operating point THEN the system SHALL set `d` true in the first instant, before time advances.
10. WHEN `SetpointReg` from source sees a rising `wr` THEN the system SHALL commit `min(data, 168)` when `data` is known and SHALL commit nothing when it is unknown.
11. The system SHALL give, for MSL sources written from the examples of §12.1.1, §12.1.2, §12.1.4 and §12.1.18, the exchange, the later statement, the pulse and the silent unchanged commit those rules state.
12. WHEN `GateDriver` from source sees `hs_cmd` rise with `ls_cmd` low THEN the system SHALL set `hs_gate` true `tpd` later, and WHEN both commands are high THEN the system SHALL hold both gates false.

**Independent Test**: `cargo test -p piperine-lang --test elab_buck`.

---

### P1: The whole buck ⭐ MVP

**User Story**: As the user, I want `examples/buck` to go from its `.msl` to a compiled circuit so that the flagship model runs.

**Why P1**: the user's request.

**Requirement**: DIG-15

**Acceptance Criteria**:

1. The system SHALL elaborate `Buck` of `examples/buck` with zero diagnostics into an IR whose `check()` is empty.
2. The system SHALL print that IR, in structural form, as the text of `crates/piperine-lang/tests/fixtures/elab/buck.txt`.
3. The system SHALL compile that IR with zero `rustc` warnings and SHALL give every body a shape that passes `piperine_abi::validate`.
4. WHEN `Supervisor` from source runs a scripted sequence of `en`, `vin_ok`, `cut`, `meas`, `reference` and `at_target` THEN the system SHALL commit, sample by sample, the `mode`, `fault` and `run` of the state table of buck SPEC §8, written in the test as a plain Rust state machine.
5. WHEN `Supervisor` is in `Regulate` THEN the system SHALL set `pgood` true at the 64th consecutive in-window sample and false at the 4th consecutive out-of-window sample.
6. WHILE `en` is not driven the system SHALL hold `mode` at `Off`, `pgood` false and both gate commands false for 10 switching periods, with `vin_ok` true from the first instant.
7. WHILE `en` is not driven the system SHALL deliver no record of `checks` with `pass` false.

**Independent Test**: `cargo test -p piperine-parity --test buck_whole`.

---

### P2: The closed loop

**User Story**: As the user, I want the converter to start and regulate so that the model is shown to be a converter, not only a netlist.

**Why P2**: it rests on solver paths no MSL design has used; a solver defect there is reported, not fixed here, and must not hold the front end.

**Requirement**: DIG-16

**Acceptance Criteria**:

1. WHEN a bench writes setpoint code 0 and then raises `en`, with Vin 12 V and a 6.6 Ω load, THEN the system SHALL move `mode` from `Off` to `SoftStart` to `Regulate` and SHALL bring `ctl.reference` to 128.
2. WHEN 600 µs have run THEN the system SHALL hold `pgood` true and `fault` at `Clear`.
3. WHEN 600 µs have run THEN the system SHALL give a mean of `vout` over the last 20 switching periods within 10 mV of 0.8 V (buck SPEC §2: ±1 % or ±10 mV, whichever is larger).
4. The system SHALL deliver no record of `checks` with `pass` false during that run.

**Independent Test**: `cargo test -p piperine-parity --test buck_closed_loop`.

---

### P2: The gap list is true

**User Story**: As a user, I want `GAPS.md` to say what this feature closed and what it left so that I know what a model may use.

**Why P2**: the project's rule: remove an entry in the commit that closes it.

**Requirement**: DIG-17

**Acceptance Criteria**:

1. The system SHALL list in `GAPS.md` each deviation this feature leaves: a change to unknown is no `rise` or `fall`; unknown control is a fault; a derived child param is not re-derived on a host `set`; enum variant names are not carried.
2. The system SHALL remove from `GAPS.md` and from `examples/buck/GAPS.md` the entry G3 as a silent gap, and from `examples/buck/README.md` the statement that nothing executes MSL.

**Independent Test**: read `GAPS.md` §3 and §4.

---

## Edge Cases

- WHEN a module has a `digital` block and no `analog` block THEN the system SHALL elaborate a body with no analog statements.
- WHEN a carrier has no initial value and no drive THEN the system SHALL elaborate it as a carrier with no initial value (§5.2.8).
- IF a single-variant enum is carried THEN the system SHALL reject the model with `P0001`, its tag having zero levels.
- WHEN a result module is placed in a module that is itself placed twice THEN the system SHALL hoist it once into that module's one body.
- IF a placement of a result module gives a param block THEN the system SHALL bind those params as constants of the hoisted items.
- IF an `Ir` holding a time read, a static read, an `At` event, an emit site or a flow read is given to the interpreter THEN the system SHALL return `Rejected` naming the body.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| DIG-01 | P1: Integers | Tasks | In Tasks |
| DIG-02 | P1: Every method call links or is reported | Tasks | In Tasks |
| DIG-03 | P1: Enums, `Option`, tuples and `match` | Tasks | In Tasks |
| DIG-04 | P1: Discrete carriers and their drives | Tasks | In Tasks |
| DIG-05 | P1: Events, reactions and commits | Tasks | In Tasks |
| DIG-06 | P1: Modules with a result, time and the analysis | Tasks | In Tasks |
| DIG-07 | P1: Nothing outside the subset passes | Tasks | In Tasks |
| DIG-08 | P1: Values become IR | Tasks | In Tasks |
| DIG-09 | P1: Structure becomes IR | Tasks | In Tasks |
| DIG-10 | P1: Result modules are hoisted | Tasks | In Tasks |
| DIG-11 | P1: Records | Tasks | In Tasks |
| DIG-12 | P1: Timers, time and the analysis at run time | Tasks | In Tasks |
| DIG-13 | P1: The analog forms of the power stage | Tasks | In Tasks |
| DIG-14 | P1: The buck slice from source | Tasks | In Tasks |
| DIG-15 | P1: The whole buck | Tasks | In Tasks |
| DIG-16 | P2: The closed loop | Tasks | In Tasks |
| DIG-17 | P2: The gap list is true | Tasks | In Tasks |

**Coverage:** 17 total, 17 mapped to tasks, 0 unmapped.

---

## Sources

- The user, 2026-11-15: "Sim, por favor! Achei que já tivesse no escopo completo" - `msl-digital` is pulled ahead of the CLI; `ROADMAP.md` "Queue after ir-rust" item 2.
- `.specs/features/codegen/context.md` - C2 to C7, S6 to S10, R1 to R4.
- `.specs/features/ir-rust/{spec,design,tasks}.md` on `feature/ir-rust` - the contract: the digital node set (T24 to T28), the buck-slice fixtures (T29), the digital plan and runtime (T31 to T41), the behaviors of phase 8 (RS-13).
- `.specs/features/device-abi/abi.md` - F2, R1, R4, C1, C3, C5; ambient `TIME` and `STATIC`; `RECORD` pins; §11.4 to §11.6.
- `docs/spec/` - Part I §1, §2, §3.3; Part II §5; Part III §8.1 to §8.3; Part IV §11 to §14; Part VI §19; Appendix C.1; Appendix H. Their error tables give every `Eppnn` code cited above.
- `examples/buck/{SPEC,GAPS}.md` and `examples/buck/src/*.msl` - the acceptance target; G3, G4, G5.
- `crates/piperine-lang/std/{values,analog,convert,monitor,io}.msl` - the stdlib items the buck reaches, as source.
- `.specs/features/{msl-typeck,msl-elab,msl-ir}/` - the M1 checker, elaborator and IR this feature extends.

---

## Success Criteria

- [ ] `cargo test -p piperine-lang --test types_buck --test elab_buck` is green: the buck checks, elaborates and equals the T29 fixtures as DIG-14 AC 1 and 2 state.
- [ ] `cargo test -p piperine-parity` is green with the slice and whole-buck targets, `contract.rs` unchanged.
- [ ] The seven M1 twins still pass on both backends with unchanged goldens.
- [ ] `examples/buck` holds no member that is unresolved without a diagnostic, and `GAPS.md` lists what remains.

# MSL Type Checking (M1 subset) Specification

**Approved**: 2026-11-15 - user: "Concordo e pode disparar"; M1 questions: all recommended options accepted

**Status**: Approved. Scoped to M1 of the end-to-end slice (`.specs/features/codegen/context.md` S1, S5).

## Problem Statement

The POM resolves names and stops there. It gives no expression a type, and it leaves every member that needs one `Deferred`: in the M1 diode, `(vd / nvt).exp()` and `.ln()` are `Deferred(ReceiverTypeUnknown)` today. Elaboration cannot lower an expression whose operands it cannot type, and the language places type checking in the elaboration phase (§0.3). This feature types the part of MSL the five M1 fixture models use, and refuses everything else by name.

## Goals

- [ ] Every expression reachable from a top module has a type, or the model is rejected with a diagnostic at that expression.
- [ ] Every `Deferred` member of that reachable set is resolved against the catalogue of Appendix H.
- [ ] A construct outside the M1 subset is refused with a diagnostic that names it. Nothing is skipped.
- [ ] The `Design` is not modified: the types are a side table.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Integer types, `UInt<N>`/`Int<N>`, `f32`, `Float<N>`, `str`, tuples, arrays, slices, references | no M1 model uses them; M2 |
| `struct`, `enum`, `match`, `impl`, traits, generics, const generics, `Option` | M2 |
| Unknown values and known flags (§3.3), `Repr`, discrete natures, `Bit`, bundles | the digital side is M2 |
| `digital` blocks, reactions, events, commits, definitions, `emit!` | M2 |
| `while`, `loop`, `for`, `break`, `continue`, `return`, `let mut`, assignment, `let else`, `?` | no M1 model uses them; M2 |
| Core primitives `ddx`, `delay`, `noise`, `cross`, `at`, `bound_step`, `simparam`, `analysis`, `now`, `freq`, flow probes and `inflow()` | no M1 model uses them; M2 |
| Templates, value inputs, module results (`-> T`), `extern` modules, generation | M2 |
| Function classes of §3 (which function may be called where, `E0122`) | M1 calls only pure value functions from `analog`; the rule lands with the digital side |
| Checking items no top module reaches | the standard library holds constructs outside the subset; they are typed when a model reaches them |
| Writing types into the POM | the POM mirrors the source and is immutable (the navigability rule) |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by `docs/spec/` or by code. No row is left open: the user answered `.specs/features/e2e-m1/questions.md` on 2026-11-15.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Type checking is a feature of its own, scoped to M1 | this feature | context S5, user: "Concordo e pode disparar" | y |
| The subset is what the M1 models need | the constructs listed in TYP-02 to TYP-05 | derived by writing the models first: `tests/ngspice/*.msl` build on today's POM with zero diagnostics | y |
| Checking is demand-driven from one top module | the modules it places, the functions they call, the constants they read, transitively | `std/analog.msl` alone holds generics, traits and arrays; an eager pass would refuse the library | y |
| Types live in a side table keyed by POM ids | `Types`, borrowed with the `Design` | `pom/mod.rs` "The one rule"; C3 | y |
| Crate | `piperine_lang::types`, beside `pom` | it reads only the POM; §0.3 places type checking in elaboration, which is this crate's (ELAB-Q7) | y |
| Method catalogue | the rows of Appendix H.2 the models call: `exp`, `ln`, `sqrt`, `abs` on `f64` | Appendix H rule 1: the catalogue is closed; a table of rows, one line each to extend | y |
| Code of a construct outside the subset (Q5) | `P0001`, a compiler-owned code outside the `Eppnn` space | the specification has no code for "not supported yet", and its numbers are never reused | y - Q4 to Q8, user 2026-11-15: "Accept all (Recommended)" |
| Code of a plain type mismatch (Q5) | `P0002` until Part I owns one | no error table of `docs/spec/` has a row for a mismatched type; §0.4 leaves it to Rust | y - Q4 to Q8, user 2026-11-15: "Accept all (Recommended)" |
| Code of an unknown top | `P0003`, compiler-owned like `P0001` and `P0002` | consistent with Q5: compiler-owned P-codes outside the `Eppnn` space; orchestrator decision 2026-11-15 | y |
| Arguments of a function call | positional only; `name: value` on a call that is not a placement is `E0140` from the POM | the POM refuses it before the checker runs; orchestrator decision 2026-11-15 | y |
| Wrong argument count of a function or method call | `P0002` | a plain type mismatch of the call against its signature (Q5); orchestrator decision 2026-11-15 | y |
| An SI-suffixed literal and a float literal are `f64`; an unsuffixed integer literal is refused where an `f64` is required | as stated | Appendix A.2; §22 Resolution at `rout`: "`100`, an integer literal, which is a type error for an `f64` param" | y |

**Open questions:** none - all resolved or logged above. The eight questions of `.specs/features/e2e-m1/questions.md` were answered by the user on 2026-11-15, every one with the recommended option.

**Implicit-requirement dimensions.** Input validation and bounds: TYP-06 (a design with diagnostics, an unknown top). Failure states: TYP-05, TYP-06. Idempotency: TYP-01 AC 4. Auth and rate limits: N/A because it is an in-process function. Concurrency and ordering: TYP-06 AC 3 (diagnostics in source order). Data lifecycle: N/A because the table lives as long as its owner. Observability: TYP-05 AC 2 and TYP-06 (code and span on every diagnostic). External-dependency failure: N/A because nothing external is read. State-transition integrity: N/A because the table is built once and never changed.

---

## User Stories

### P1: A type for every reachable expression ⭐ MVP

**User Story**: As the elaborator, I want the type of every expression a top module reaches so that I can lower it without guessing.

**Why P1**: nothing downstream can run without it.

**Requirement**: TYP-01

**Acceptance Criteria**:

1. WHEN `Types::check` is called with a `Design` without diagnostics and the name of a module of its root crate THEN the system SHALL return a table that gives a type to every expression of that module, of every module it places, of every function they call and of every constant they read, transitively.
2. The system SHALL take the `Design` by shared borrow and leave `Design::dump()` byte-identical after the call.
3. WHEN an expression of the reachable set is a method call or a field access the POM left `Deferred` THEN the system SHALL record what it resolves to.
4. WHEN `Types::check` is called twice with the same `Design` and top THEN the system SHALL return two tables that are equal.

**Independent Test**: check `DiodeIv` of the M1 models; ask the type of `(vd / nvt).exp()` and read `f64`, and its method and read the catalogue row `exp`.

---

### P1: Value expressions ⭐ MVP

**User Story**: As a model author, I want arithmetic, conditions, `let`, calls and closures typed as Rust types them so that `pnjlim` and `vt` of the standard library check as written.

**Why P1**: the diode calls both.

**Requirement**: TYP-02

**Acceptance Criteria**:

1. The system SHALL type a float literal, with or without the suffix `_f64`, and an SI-suffixed literal as `f64`, and `true` and `false` as `bool`.
2. The system SHALL type `+`, `-`, `*`, `/` and unary `-` over two `f64` as `f64`; `<`, `<=`, `>`, `>=`, `==`, `!=` over two `f64` as `bool`; and `&&`, `||` and `!` over `bool` as `bool`.
3. The system SHALL type an `if` with an `else` as the common type of its two blocks, and a block as the type of its last expression, or `()` when it ends with a statement.
4. WHEN a `let` has no type annotation THEN the system SHALL give its binding the type of its initializer.
5. The system SHALL type a call of a function by its declared result type after checking each argument against the declared parameter type, optional trailing parameters left out included (§3.2).
6. The system SHALL type a call of `exp`, `ln`, `sqrt` or `abs` on an `f64` receiver as `f64`.
7. WHEN a closure is passed where a parameter of function type is declared THEN the system SHALL type its parameters from that declaration and check its body against the declared result.
8. IF two operands, an argument and its parameter, a `let` and its annotation, or the two arms of an `if` have different types THEN the system SHALL reject the model with one diagnostic at the offending expression that names both types.

**Independent Test**: check a function `fn f(a: f64) -> f64 { if a > 0.0 { a.ln() } else { true } }` and read one diagnostic at `true` naming `f64` and `bool`.

---

### P1: Modules, params, nodes and instances ⭐ MVP

**User Story**: As a model author, I want my module's params, ports, signals and placements checked so that a wrong connection or param is refused before any solve.

**Why P1**: §8.1; every twin is a module of placements.

**Requirement**: TYP-03

**Acceptance Criteria**:

1. The system SHALL type a param by its declared type, and SHALL accept `f64` and `bool` params only.
2. IF a param default does not have the param's type THEN the system SHALL reject the model at the default.
3. IF a `where` condition is not `bool` THEN the system SHALL reject the model with `E0318`.
4. The system SHALL type a port or a `signal` whose type is a conservative nature as a node of that nature, and `gnd` as a node of any nature.
5. IF the `= value` of a node is not an `f64` THEN the system SHALL reject the model with `E0210`.
6. WHEN a module is placed THEN the system SHALL check that each node argument is a node of the nature of its port, that the argument count equals the port count (`E0312`), and that each entry of the param block names a param of the module once (`E0320`) with a value of the param's type.
7. IF a node argument has a nature different from its port's THEN the system SHALL reject the model with `E0251`.
8. IF the `where` of a module or of a `fn` is not `bool` THEN the system SHALL reject the model with `E0318`.

> Amendment, verification round 1 (F1): the module and `fn` `where` were not read.

**Independent Test**: place `Resistor { r: true }(a, b)` and read a diagnostic at `true`; place `Resistor { r: 1k }(a)` and read `E0312`.

---

### P1: Analog forms ⭐ MVP

**User Story**: As a model author, I want probes, contributions and the core primitives of the subset checked so that an `analog` block means what §11 and §14 say.

**Why P1**: every M1 device is an `analog` block.

**Requirement**: TYP-04

**Acceptance Criteria**:

1. The system SHALL type a potential probe `v(a)` or `v(a, b)` through an `access` alias of the nature of its nodes, or through `potential`, as `f64`.
2. IF the two nodes of a probe or of a contribution have different natures THEN the system SHALL reject the model with `E0240`.
3. IF the left side of `+>` is not a potential or flow probe of a branch THEN the system SHALL reject the model with `E0410`.
4. IF the right side of `+>` is not `f64` THEN the system SHALL reject the model with `E0417`.
5. The system SHALL type `ddt(x)` with `x: f64` as `f64`, `ac_stim(mag)` and `ac_stim(mag, phase)` over `f64` as `f64`, and `temperature()`, `gmin()` and `source_scale()` as `f64`.
6. The system SHALL type `limit(x, f)` and `limit(x, f, seed)` as `f64` after checking `x: f64`, `seed: f64` and `f` as a function or closure of type `fn(f64, f64) -> f64`.
7. IF a core primitive of the §14 table is written in a `fn` or a closure where the table does not allow it THEN the system SHALL reject the model with `E0450`.
8. IF a core primitive or a probe is written in a context the §14 "Allowed in" column does not list (`ac_stim` outside an `analog` block; `ddt` and `limit` in a `fn`, a closure, a param default or a `where`; a probe in a `fn`, a closure, a param default or a `where`) THEN the system SHALL reject the model with `E0450`.
9. IF an environment function of §14.4 (`temperature`, `gmin`, `source_scale`) is written in a const context (a param default, a `where` of a module, `fn` or param, or a `const` initializer) THEN the system SHALL reject the model with `E0323`.

> Amendment, verification round 1 (F2): this feature owns the "Allowed in" column in every position, not only inside a `fn` or a closure, and owns `E0323` (§8.1.21).

**Independent Test**: check the `Diode` of the M1 models and read no diagnostic; replace its closure by `|new| new` and read one diagnostic at the closure.

---

### P1: Nothing outside the subset passes ⭐ MVP

**User Story**: As a maintainer, I want a construct this version does not type to stop the check so that a partial checker never accepts a model it did not understand.

**Why P1**: the project rule "fail loud"; M1 is a subset by decision.

**Requirement**: TYP-05

**Acceptance Criteria**:

1. IF an expression, statement, pattern, type or item of the reachable set is outside TYP-02 to TYP-04 THEN the system SHALL reject the model with one diagnostic of the code `P0001` at that construct.
2. The system SHALL name in every `P0001` diagnostic the construct as the specification names it (for example "`match` expression", "generic module", "integer type `u32`").
3. WHEN an item holds a construct outside the subset and no top module reaches it THEN the system SHALL report nothing for it.
4. IF a function is written as a value anywhere but as an argument whose parameter has a function type, or a call has a callee that is not a named function or a built-in (a call through a parameter or a local), THEN the system SHALL reject the model with one diagnostic of the code `P0001` at that construct.
5. IF a construct is refused with `P0001` THEN the system SHALL still check every part of the reachable set that does not depend on it: the initializer of a `let` whose annotation is refused, the body of a generic function, and the missing result of a body whose last statement is not itself refused.

> Amendment, verification round 1 (F3, F6): an indirect call has no callee to record, so the checker refuses the form and msl-elab's `Unlowered` table is unchanged. A generic `fn` is refused at its declaration (`P0001`, "generic function") and its body is checked with the parameters it could not type left silent.

**Independent Test**: check a top that places a module holding a `match`; read `P0001` at the `match`. Check the M1 `DiodeIv`, which reaches `pnjlim` and `vt` and none of the generic modules of `std::analog`; read no diagnostic.

---

### P1: Entry and diagnostics ⭐ MVP

**User Story**: As a host author, I want one call that gives me the table or the reasons there is none.

**Why P1**: the entry point elaboration calls.

**Requirement**: TYP-06

**Acceptance Criteria**:

1. IF the `Design` holds a diagnostic THEN the system SHALL return no table and SHALL return those diagnostics.
2. IF the named top is not a module of the root crate THEN the system SHALL return no table and one diagnostic naming it.
3. The system SHALL return every diagnostic of one call ordered by file, then by span start, and SHALL report at most one diagnostic per expression.
4. The system SHALL attach to every diagnostic the span of the offending construct and a code.

**Independent Test**: check a source with two independent mismatches and read two diagnostics in source order.

---

## Edge Cases

- WHEN a function of the reachable set calls itself THEN the system SHALL type it from its declared signature without looping.
- IF a `let` has neither an annotation nor an initializer THEN the system SHALL reject the model with `P0001`.
- WHEN a name resolves to a `const` item THEN the system SHALL type it by its declared type and check its initializer once.
- IF a closure is written anywhere but as an argument whose parameter has a function type THEN the system SHALL reject the model with `P0001`.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| TYP-01 | P1: A type for every reachable expression | Design | Pending |
| TYP-02 | P1: Value expressions | Design | Pending |
| TYP-03 | P1: Modules, params, nodes and instances | Design | Pending |
| TYP-04 | P1: Analog forms | Design | Pending |
| TYP-05 | P1: Nothing outside the subset passes | Design | Pending |
| TYP-06 | P1: Entry and diagnostics | Design | Pending |

**Coverage:** 6 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` S1, S5 - the slice and this feature's existence.
- `tests/ngspice/*.msl` - the models that fix the subset.
- `docs/spec/` §0.3, §1, §3.2, §4, §6, §8.1, §11, §14, Appendix A.2, Appendix H.2 - the rules checked.
- `crates/piperine-lang/src/pom/` (`body.rs`, `resolution.rs`, `builtin.rs`) - what the input holds.

---

## Success Criteria

- [ ] The five M1 twins and `devices.msl` check with no diagnostic.
- [ ] Every row of the Out of Scope table that is a language construct has a negative test that reads `P0001`.
- [ ] No test finds a `Design` changed by the check.

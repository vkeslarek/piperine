# IR Interpreter (reference backend, M1 subset) Specification

**Approved**: 2026-11-15 - user: "Concordo e pode disparar"; M1 questions: all recommended options accepted

**Status**: Approved. Rewritten for M1 of the end-to-end slice (`.specs/features/codegen/context.md` S1, S4, S8).

## Problem Statement

After `msl-elab` an MSL design exists as an `Ir`, and nothing executes it. The solver runs devices through one contract, the device ABI (MD-62), and every device in the tree today is written by hand in a test. This feature is the first backend: one generic device that runs any body of an `Ir` through the ABI, with Jacobians from dual numbers (S4). It is the reference: always available, with no compiler in the environment, and the oracle a faster backend is compared against (C6).

## Goals

- [ ] An elaborated M1 design runs on the solver from its `Ir` alone: operating point, DC, AC and transient.
- [ ] The interpreter meets the solver only through the device ABI: shapes, `eval`, nothing else.
- [ ] Every declared partial is exact: each function of each M1 body passes `piperine_abi::verify::verify_partials`.
- [ ] A term that is linear in the node potentials leaves no round-off on the solver's right side.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The IR model and its construction | features `msl-ir` and `msl-elab` |
| Reactions, commits, discrete functions other than a param's `where`, events, watches, `emit!` records | the digital side is M2 (S2); the M1 node set has none |
| `DELAY` terms, `NOISE` inputs, `CROSS`/`CAP`/`TRACE`/`VAR` watches, the ambients `TIME`, `FREQ`, `STATIC` as model reads | their source forms are outside the M1 subset |
| Partials against params | `sens` and `adjoint` are not M1 analyses; the solver falls back to finite differences where a path has no partial (abi.md §10) |
| The differential harness between two backends (first draft INT-06) | M3: it needs a second backend. S8 fixes how they will be compared |
| A debug trace or stepping (INT-Q2) | not in M1: the printed IR and the solver's trace flags are the debug surface |
| Interpreter speed work | the interpreter is the reference, not the fast path (C6) |
| A host session (`piperine-api`) | does not build yet; M1 exposes one function that returns a bound `CircuitInstance` |
| Time, steps, Newton, limiting tolerance, histories, checkpoints | the solver's (C9; abi.md §0 item 2) |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by `docs/spec/`, by `abi.md` or by code. No row is left open: the user answered `.specs/features/e2e-m1/questions.md` on 2026-11-15.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The interpreter is the reference backend and executes the IR | as stated | C3, C6 | y |
| Jacobians by forward-mode dual numbers | one tangent per differentiable read of the function | S4, user: "Concordo e pode disparar"; abi.md §8 | y |
| One generic device for every body (INT-Q5) | `Interpreted`, created by a per-body factory that holds the plan | abi.md §3: `Module.data` is "the module's own (an interpreter's IR, an OSDI descriptor)"; §7 "One path" | y |
| The ABI crate gains a constructor of a `Module` that carries data (Q1) | `piperine_abi::thunk::module_with::<F: Factory>(name, &'static F)`; `Device` and `module::<D>` stay as they are | `Device::prototype()` and `Device::create()` are static functions and `module::<D>` sets `data` to null (`thunk.rs:28`), so one Rust type cannot serve many bodies today. The alternative is `unsafe` thunks outside `piperine-abi`, which its own rule forbids | y - Q1, user 2026-11-15: "Add Factory to abi (Recommended)" |
| Crate (INT-Q6) | `piperine-interp`: depends on the IR crate, `piperine-abi` and `piperine-solver` | the solver crates may depend only on `piperine-abi` (MD-61), so the arrow can only point this way; the solver dependency is for `Interpreter::circuit` alone | y |
| Where the IR is turned into a solver circuit | `Interpreter::circuit`, in this crate, until a host crate builds | `piperine-api` does not compile; the function reads only the instance table and a body-to-module map, so moving it later changes no logic | y |
| Modules live for the process | one leaked `Module` and one leaked factory per body per `Interpreter` | `CircuitBuilder::device` and `Handle::create` take `&'static Module` (`builder.rs:99`, `caller.rs:130`); the count is bounded by the bodies of a design | y |
| Linear terms (the T5 lesson) | an affine `FLOW` term is written as `c + Σ partial·read`, summed as the assembly sums | `piperine-analog-solver/src/device/flow.rs:24`: the right side is `−(v − Σ g·x)`, folded from `0.0` in partial order | y |
| Every node of a body is a `PORT` pin | ports and signals alike; the netlist binds each to a design node | `msl-elab` owns every node; a `PORT` pin with `HAS_INIT` gives the hold of §16 H1 (`bind.rs:431`) | y |
| `limit` on its first iteration | from the seed, and never below it (§18 rule 2) | abi.md A2 gives the seed rule; its §11.2 example omits rule 2, the specification states it | y |
| Backend comparison (INT-Q3) | discrete bit for bit, analog within the session tolerances | S8; nothing to compare in M1 | y |

**Open questions:** none - all resolved or logged above. The eight questions of `.specs/features/e2e-m1/questions.md` were answered by the user on 2026-11-15, every one with the recommended option.

**Implicit-requirement dimensions.** Input validation and bounds: INT-01 AC 3 (an IR that fails `check()`), INT-05. Failure states: INT-05. Idempotency: INT-02 AC 5 (purity, abi.md F1). Auth and rate limits: N/A because it is an in-process library. Concurrency and ordering: INT-02 AC 5; the device is `Send` and keeps no state between calls. Data lifecycle: the leaked modules, stated above. Observability: INT-05 AC 2 (origins in faults); every pin, term, input and function carries its `origin`. External-dependency failure: N/A because nothing external is called. State-transition integrity: N/A because the device has no memory (abi.md §0 item 2).

---

## User Stories

### P1: Run a design from its IR ⭐ MVP

**User Story**: As a host author, I want an elaborated design bound to the solver with one call so that I can ask for an analysis.

**Why P1**: C6; the first end-to-end path.

**Requirement**: INT-01

**Acceptance Criteria**:

1. WHEN `Interpreter::new` is given an `Ir` whose `check()` is empty THEN the system SHALL return an interpreter that gives one `&'static Module` per body that has an `analog` block or a node with an initial potential.
2. WHEN `Interpreter::circuit` is called THEN the system SHALL return a `CircuitInstance` in which each such instance is one device bound by `CircuitBuilder::device` under its path, each design node is one solver node named by its path, and each param holds its bound value.
3. IF the `Ir` fails `check()` THEN the system SHALL return no interpreter and SHALL return the check's entries.
4. The system SHALL invoke no external program and read no file.
5. WHEN a body node has an initial potential THEN the system SHALL give its pin `HAS_INIT` with that value.

**Independent Test**: elaborate `Divider` of the M1 models, call `circuit()`, run `dc()`; `Divider.mid` reads 7.5 V within 1e-9.

---

### P1: The ABI carries a module's data ⭐ MVP

**User Story**: As the author of a device that serves many modules, I want to build a `Module` around a factory value so that one Rust type runs any number of bodies without `unsafe` of my own.

**Why P1**: without it the generic device cannot be expressed through `piperine-abi` (Q1).

**Requirement**: INT-08

**Acceptance Criteria**:

1. WHEN `thunk::module_with` is given a name and a `&'static` factory THEN the system SHALL return a `Module` whose `data` is that factory and whose `prototype` is the factory's.
2. WHEN `create` of that module is called THEN the system SHALL call the factory's `create` with the given values and SHALL validate the returned shape as `module::<D>` does.
3. WHEN `eval` or `react` of that module is called THEN the system SHALL run the instance the factory created, with a panic turned into a `FAULT` as in `module::<D>`.
4. The system SHALL leave `Device`, `module::<D>` and every existing test of `piperine-abi` and `piperine-solver` unchanged and green.

**Independent Test**: a test factory in `piperine-abi/tests` holding a gain `k` builds two modules with different `k`; one device type, two results.

---

### P1: A shape from a body ⭐ MVP

**User Story**: As the solver, I want each body described as a shape so that I bind it like any hand-written device.

**Why P1**: the shape is the whole interface (abi.md §3).

**Requirement**: INT-09

**Acceptance Criteria**:

1. The system SHALL declare the pins of a body in this order: one `PARAM` pin per param in declaration order with its default as `init`; one `PORT` continuous pin per body node in node order with the `abstol` of its nature's potential and flow; one `PRIVATE` continuous pin per potential branch; one `GLOBAL` `TEMPERATURE` pin when the body reads `temperature()`.
2. The system SHALL declare, per flow branch, a `FLOW` term when it has a direct addend and a `DDT` term when it has an addend under `ddt`, both on the branch's terminals.
3. The system SHALL declare, per potential branch with private pin `f`, a `FLOW` term on the branch's terminals whose value is `f`, and a `FLOW` term `{f, GROUND}` whose value is the potential difference minus the sum of the contributions (abi.md A1).
4. The system SHALL declare one `LIMIT` input per `limit` site and one `STIM` input per `ac_stim` site.
5. The system SHALL declare one function per `limit` site that writes its `PROPOSED` and `LIMITED`, one function per `ac_stim` site that writes its `RE` and `IM`, and one function that writes every term of the body.
6. The system SHALL declare as the reads of each function exactly the cells of its static dependency set (`msl-ir` IR-09), and as its partials each written term or `PROPOSED` against each continuous pin, `LIMIT` value and `STIM` value of that set.
7. The system SHALL give every pin, term, input and function the `origin` of its IR item.
8. WHEN a shape is produced for any M1 body THEN `piperine_abi::validate::validate` SHALL accept it.

**Independent Test**: the shape of `Diode` has pins `is, n, cj0, a, k, temperature`, one `LIMIT` input, terms `FLOW{a,k}` and `DDT{a,k}`, and two functions; it validates.

---

### P1: Value semantics ⭐ MVP

**User Story**: As a model author, I want value code to compute what the language defines so that `pnjlim` and `vt` behave as written.

**Why P1**: §0.4, Part I; every other story evaluates expressions.

**Requirement**: INT-02

**Acceptance Criteria**:

1. The system SHALL evaluate `f64` arithmetic, comparisons and the methods `exp`, `ln`, `sqrt` and `abs` with the results of Rust's `f64`.
2. WHEN a float is divided by zero THEN the system SHALL yield the IEEE 754 result and SHALL not fail (§1 rule 12).
3. The system SHALL evaluate `&&` and `||` without evaluating the right operand when the left decides, and an `if` by evaluating only the taken block.
4. WHEN a function is called THEN the system SHALL evaluate its body with its parameters bound to the argument values, and a closure with the locals of the block that wrote it.
5. WHEN a function is evaluated twice with the same inputs THEN the system SHALL write the same output bytes and the same partials.

**Independent Test**: evaluate `pnjlim` from the standard library at the six branch points of its source and compare with the same formula written in the test.

---

### P1: Analog evaluation with exact partials ⭐ MVP

**User Story**: As the analog sub-solver, I want each term's value and its partials at the iterate so that Newton assembles and solves.

**Why P1**: §11 rule 5, §11 rule 16; abi.md F5.

**Requirement**: INT-03

**Acceptance Criteria**:

1. WHEN the terms function is evaluated THEN the system SHALL execute the `analog` block once, with each probe reading the difference of its terminals' pins, ground reading 0, and SHALL write to each term the sum of its branch's addends with their signs.
2. The system SHALL write, for every seeded partial, the derivative of the written value with respect to the read cell, computed by dual numbers through operators, methods, function calls and locals.
3. The system SHALL read `temperature()` from the `TEMPERATURE` pin, `gmin()` from the ambient `GMIN` and `source_scale()` from the ambient `SOURCE_SCALE`.
4. WHEN a function of any M1 body is given to `verify_partials` at a point where its limits hold `LIMITED = PROPOSED` THEN the verifier SHALL pass every declared partial with `rel_tol` 1e-6.
5. The system SHALL give `abs` the derivative `sign(x)` and every other method of the catalogue its analytic derivative.

**Independent Test**: evaluate the diode's terms function at `vd = 0.6 V`, 300.15 K; the `FLOW` value equals `is·(exp(vd/vt) − 1) + gmin·vd` and its partial against the `LIMIT` value equals `is·exp(vd/vt)/vt + gmin`, both to 1e-12 relative.

---

### P1: Linear terms are exact ⭐ MVP

**User Story**: As the solver, I want a linear term's value to be the same products I subtract so that its right side is exactly zero.

**Why P1**: the lesson of `solver-abi-integration` T5: a last-bit residue per solve became 1e-12 relative drift in step times.

**Requirement**: INT-12

**Acceptance Criteria**:

1. WHEN a `FLOW` term's addends are affine in the node potentials (`msl-ir` IR-09 AC 3) THEN the system SHALL evaluate them with every continuous pin read as 0 to get the constant `c` and the partials `g`, and SHALL write the value `c` added to the sum of `g_j·x_j` folded from `0.0` in the order of the term's declared partials.
2. WHEN the `Resistor` body is evaluated at any `vp`, `vn` and `r` THEN the system SHALL write a value equal bit for bit to `g*vp - g*vn` with `g = 1.0/r`, and the partials `g` and `-g`.

**Independent Test**: for 1000 random `(vp, vn, r)`, the value minus the fold `0.0 + g*vp + (-g)*vn` is exactly `0.0`.

---

### P1: `limit` ⭐ MVP

**User Story**: As a model author, I want `limit` to bound Newton's step as §18 states so that a junction converges.

**Why P1**: the diode is in M1 by decision (S1).

**Requirement**: INT-10

**Acceptance Criteria**:

1. WHEN the function of a `limit` site is evaluated THEN the system SHALL write `PROPOSED` as the site's input `x` with its partials against the continuous pins, and `LIMITED` as `f(x, old)`.
2. WHEN `AGE` is 0, or the ambient `STATIC` is true and `AGE` is 1, THEN the system SHALL take `old` as the seed, or as `x` when the site has no seed; otherwise it SHALL take `old` as `LAST` (abi.md A2).
3. WHEN `old` is the seed and `f(x, seed)` is below the seed THEN the system SHALL write the seed as `LIMITED` (§18 rule 2).
4. WHEN the terms function reaches a `limit` site THEN the system SHALL use the input's `VALUE` as the site's result with derivative 1 against that read, and SHALL not evaluate `x`, `f` or the seed (§18 rule 3, abi.md A2).
5. The system SHALL declare no function that writes both a limit and a term (abi.md A3).

**Independent Test**: bind one `Diode` across a 0.8 V source through 1 Ω and run `dc()`; it converges, and the current equals the fixed point of the Shockley equation to 1e-9 relative.

---

### P1: `ac_stim` ⭐ MVP

**User Story**: As a model author, I want `ac_stim` to be the small-signal stimulus so that an `Ac` sweep has a source.

**Why P1**: AC is an M1 analysis (S1).

**Requirement**: INT-11

**Acceptance Criteria**:

1. WHEN the function of an `ac_stim` site is evaluated THEN the system SHALL write `RE` as `mag·cos(phase)` and `IM` as `mag·sin(phase)`, and `RE = mag`, `IM = 0` when the site has no phase.
2. WHEN the terms function reaches an `ac_stim` site THEN the system SHALL use the input's `VALUE` as the call's result with derivative 1 against that read.

**Independent Test**: bind `Vsource { ac: 1.0 }` to a 1 kΩ and 1.5915494309189535 nF low-pass and run an `Ac` point at 100 kHz; the output magnitude is `1/√2` to 1e-9.

---

### P1: Faults ⭐ MVP

**User Story**: As a model author, I want a failure to say what failed and where.

**Why P1**: §0.3 item 4; debuggability is the reason for the redesign.

**Requirement**: INT-05

**Acceptance Criteria**:

1. IF `create` is given a value for a param whose layout it does not match THEN the system SHALL return a `FAULT` naming the param.
2. IF an evaluation reaches an IR form the interpreter does not execute THEN the system SHALL return a `FAULT` whose message names the form and its origin, and SHALL not write a value.

**Independent Test**: a hand-built IR that passes `check()` and holds a closure outside a `limit` faults at evaluation with the closure's origin in the note.

---

### P2: A param's `where` guards a host `set`

**User Story**: As a host author, I want a `set` that breaks a param's `where` refused so that a running design never holds an illegal value.

**Why P2**: §8.1, abi.md P6. No M1 fixture sets a guarded param; without this a bad `set` would pass silently, so it is in M1 but after the parity path.

**Requirement**: INT-14

**Acceptance Criteria**:

1. WHEN a param has a `where` condition THEN the system SHALL declare a private discrete pin that holds a `BOOL` at byte 0, a discrete function that reads the params the condition names and writes it, and SHALL flag the param's pin `HAS_CHECK` with that pin as its `check`.
2. WHEN `CircuitInstance::set` writes a value that makes the condition false THEN the solver SHALL refuse the write.

**Independent Test**: `circuit.set("Top.r1", "r", Real(-1.0))` returns an error and a following `dc()` reads the old result.

---

## Edge Cases

- WHEN a body has an `analog` block that contributes nothing THEN the system SHALL give a shape with no term and no function, and the instance SHALL bind.
- WHEN a body has no `analog` block and a node with an initial potential THEN the system SHALL give a shape of `PORT` pins only.
- WHEN a param is not given at a placement THEN the system SHALL pass no value for it and the pin SHALL hold its default.
  - Reading (verification F2): the IR binds a value to every param of an instance and cannot tell "not given" from "given equal to the default"; a param is "not given" when its bound value equals its body's default. The check asserts the values the instance is created with (`CircuitInstance::given`).
- IF a contribution evaluates to a non-finite value THEN the system SHALL write it as it is; the solver fails the solve (abi.md A4, `E0507`).
- WHEN a transient's stop time is off the solver's tick grid THEN the run SHALL end with exactly one record at the stop, the integrated trajectory point, and no two records SHALL share a time. (verification F1)
- WHEN a function's walk passes code over a param or a cell the function does not read (its `Body::dependencies` leave it out) THEN the system SHALL run no arm of an `if`, `&&` or `||` decided by it and SHALL write the function's declared values exactly; IF a value the function writes or a decision it needs is taken from such a cell THEN the system SHALL fault (`Walk::UNDECLARED`). (verification F4)

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| INT-01 | P1: Run a design from its IR | Design | Pending |
| INT-02 | P1: Value semantics | Design | Pending |
| INT-03 | P1: Analog evaluation with exact partials | Design | Pending |
| INT-05 | P1: Faults | Design | Pending |
| INT-08 | P1: The ABI carries a module's data | Design | Pending |
| INT-09 | P1: A shape from a body | Design | Pending |
| INT-10 | P1: `limit` | Design | Pending |
| INT-11 | P1: `ac_stim` | Design | Pending |
| INT-12 | P1: Linear terms are exact | Design | Pending |
| INT-14 | P2: A param's `where` guards a host `set` | Design | Pending |

INT-04 (discrete evaluation) and INT-07 (debug mode) of the first draft are M2; INT-06 (differential oracle) is M3.

**Coverage:** 10 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` C3, C6, C9, S1, S4, S8, and the T5 lesson - the decisions.
- `.specs/features/device-abi/abi.md` §3, §4 (F1 to F5, A1 to A4), §7, §8, §11 - the contract implemented.
- `crates/piperine-abi/src/{device,thunk,shape,verify}.rs`, `crates/piperine-solver/src/core/{bind,builder}.rs`, `crates/piperine-solver/tests/helpers/abi_devices.rs` - how devices are built and bound today.
- `docs/spec/` §11, §14.3, §14.4, §14.7, §16 H1, §18 - the behavior.
- `.specs/features/msl-ir/design.md` - the input.

---

## Success Criteria

- [ ] The five M1 twins bind and run their analysis on the solver from the IR.
- [ ] Every function of every M1 body passes `verify_partials`.
- [ ] `piperine-interp` holds no `unsafe`.

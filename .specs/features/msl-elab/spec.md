# MSL Elaboration (POM to IR, M1 subset) Specification

**Approved**: 2026-11-15 - user: "Concordo e pode disparar"; M1 questions: all recommended options accepted

**Status**: Approved. Rewritten for M1 of the end-to-end slice (`.specs/features/codegen/context.md` S1, S6, S7).

## Problem Statement

A `Design` mirrors the source: nothing is evaluated and no instance is placed. The language defines an elaboration phase that evaluates constants, checks params and places every instance (§8 rule 8, §16 step 1). Nothing implements it for MSL, so no model reaches the solver. This feature is that phase for the M1 analog subset: a type-checked `Design` and a top module in, an `Ir` out (C3).

## Goals

- [ ] One call turns a `Design` and the name of a top module into an `Ir`, or into diagnostics and no `Ir`.
- [ ] The five M1 twins elaborate, and their printed IR is pinned by golden tests.
- [ ] A model that meets an elaboration-phase error of the subset is rejected with the specification's code and a span.
- [ ] A construct elaboration cannot lower fails with a diagnostic. Nothing is dropped.
- [ ] The `Design` is never modified.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Type checking and the refusal of constructs outside the subset | feature `msl-typeck` (S5); elaboration runs after it |
| The IR's storage, printer, `check()` and dependency analysis | feature `msl-ir` |
| Running the IR | feature `ir-interp` |
| Generics, const generics, monomorphization into several variants | M2; with none, each module has exactly one body, which is S7 with one variant |
| Generation `for` and `if`, templates, value inputs, module results, bundles | M2 |
| Param values given by the caller, structural params, re-elaboration on `set` | M2, with the host; in M1 every param is non-structural and a host `set` goes to the solver |
| A placement's param value that reads the parent's params | M2; it makes the child's param a run-time function of the parent's |
| Selectors: a contribution under `if` or `match`; switch branches; merged nodes (§11 rule 3) | M2; no M1 model has one |
| `ddt` anywhere but as a whole addend of a flow contribution; `ddt` in a potential contribution (an inductor) | M2: needs the hoisting of abi.md A1 |
| Implicit params `m` and `dtemp` given a value | M2; M1 elaborates with `m = 1` and `dtemp = 0` and refuses a placement that sets either |
| The `digital` side: definitions, reactions, events, boundary reachability, definition order | M2 (S2) |
| `extern` modules | refused (ELAB-Q6 closed); their runtime form is the ABI's and M1 has no foreign device |
| Operating-point variables, streams, gradient params and the rest of §21 beyond paths and params | M2, with the host |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by `docs/spec/` or by code. No row is left open: the user answered `.specs/features/e2e-m1/questions.md` on 2026-11-15.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Elaboration reads a `Design` and writes an `Ir`; the POM stays as written | shared borrow in, `Ir` out | C3; `pom/mod.rs` "The one rule" | y |
| One body per distinct variant; functions kept | as stated; one variant per module in M1 | S7, user: "Concordo e pode disparar" | y |
| Bodies per module and a flat instance table | as stated | S6, same answer | y |
| Type checking runs first | `Types::check` is called by the entry point; its diagnostics are returned as they are | S5 | y |
| Crate (ELAB-Q7) | `piperine_lang::elab`; `piperine-lang` gains a dependency on the IR crate | elaboration is a language phase (§0.3); `CLAUDE.md` names this crate "parser + elaborator"; the IR crate is a leaf, so the edge adds no front-end weight to a backend | y |
| The elaborator owns every node | every continuous port and signal of every instance is one entry of the IR's node table; a child's port is its parent's node | the solver binds a device's `PORT` pins to named nodes (`CircuitBuilder::device`); a path per node is §21 rule 1 | y |
| The top of a path | the top module's name: `Divider.mid`, `Divider.r1` | §21 rule 1: "from the top module down"; its example `top.adc.dac.r[3].r` starts with the top | y |
| The top module of M1 | has no port; its params take their defaults | a port of a top has nothing to connect to; caller-given values are M2 | y |
| A node's `= value` | a const expression that reads no param; evaluated at elaboration | abi.md §3: a shape "depends on structure only, never on a non-structural value", and the hold is part of the shape; `E0210` already forbids reading a carrier or a probe | y |
| Code of a construct inside the type-checked subset that elaboration does not lower (Q5) | `P0001`, as in `msl-typeck` | same rule, same code | y - Q4 to Q8, user 2026-11-15: "Accept all (Recommended)" |
| Code of the internal "elaboration built an ill-formed IR" diagnostic | `P0004`, a compiler-owned code beside `P0001` to `P0003` (`E0999` is not spec-owned: `docs/spec/index.md` §0.2 gives `E09xx` to Appendix A) | conditions the compiler owns, not the language, take `P` codes (Q5) | y - Q5 applied by the orchestrator, msl-elab verification round 2 |

**Open questions:** none - all resolved or logged above. The eight questions of `.specs/features/e2e-m1/questions.md` were answered by the user on 2026-11-15, every one with the recommended option.

**Implicit-requirement dimensions.** Input validation and bounds: ELAB-02 (params), ELAB-01 AC 3 and 4. Failure states: ELAB-06. Idempotency: ELAB-01 AC 5. Auth and rate limits: N/A because elaboration is an in-process function. Concurrency and ordering: ELAB-03 AC 2 (instance order), ELAB-04 AC 4 (addend order). Data lifecycle: N/A because the `Ir` is owned by the caller. Observability: every diagnostic carries a code and a span (ELAB-05). External-dependency failure: N/A because nothing external is read in M1 (`include_str!` is outside the subset). State-transition integrity: N/A because elaboration is one pure function.

---

## User Stories

### P1: One call from a design to an IR ⭐ MVP

**User Story**: As a host author, I want one call that elaborates a top module so that I get an IR or the reasons there is none.

**Why P1**: C3; the entry point of the chain.

**Requirement**: ELAB-01

**Acceptance Criteria**:

1. WHEN `Elaborator::run` is called with a `Design` without diagnostics and the name of a module of its root crate THEN the system SHALL return an `Ir` whose `check()` is empty.
2. The system SHALL take the `Design` by shared borrow and leave `Design::dump()` byte-identical after the call.
3. IF the `Design` holds a diagnostic, or type checking returns one, THEN the system SHALL return no `Ir` and SHALL return those diagnostics.
4. IF the named top does not exist, or has a port, THEN the system SHALL return no `Ir` and one diagnostic naming the module.
5. WHEN `Elaborator::run` is called twice with the same `Design` and top THEN the system SHALL return two `Ir` values with byte-identical printed texts.

**Independent Test**: elaborate `Divider` of the M1 models and compare the printed IR with a golden file.

---

### P1: Params and constants ⭐ MVP

**User Story**: As a model author, I want params bound, defaulted and checked at elaboration so that a bad value is refused before any solve.

**Why P1**: §8 rule 8, §8.1.

**Requirement**: ELAB-02

**Acceptance Criteria**:

1. WHEN a placement gives no value for a param THEN the system SHALL bind it to its default evaluated as a const expression.
2. IF a param with no default is given no value THEN the system SHALL reject the model with `E0319` at the placement.
3. IF a bound param value makes the param's `where` condition false THEN the system SHALL reject the model with `E0317` and SHALL name the param's path.
4. IF a bound param value or a default reads anything but constants THEN the system SHALL reject the model with `P0001` when it reads a param of the parent, and with `E0321` otherwise.
5. The system SHALL replace every read of a `const` item by its value, evaluated once.
6. The system SHALL keep every param as a value read at run time in the bodies: no param read is folded into an expression.
7. The system SHALL record each param's `where` condition in the body as an expression over the body's params.

**Independent Test**: place `Resistor { r: -1.0 }(a, gnd)` and read `E0317` naming `Top.r1.r`; elaborate `DiodeIv` and read the bound `is` of `DiodeIv.d1` as `1e-14`.

---

### P1: Instances and nodes ⭐ MVP

**User Story**: As a model author, I want every placement turned into an instance bound to nodes so that the elaborated netlist is the one I wrote.

**Why P1**: §8 rule 8; S6.

**Requirement**: ELAB-03

**Acceptance Criteria**:

1. The system SHALL produce one body for each module the top reaches and one instance for each placement, the top included.
2. The system SHALL order the instance table in pre-order of the hierarchy, children in source order.
3. The system SHALL give each instance the path of §21 rule 1 starting with the top module's name, and each node the path of the instance that declares it followed by its name without `r#`.
4. The system SHALL create one node of the node table for each continuous `signal` of each instance, and SHALL bind each port of a child to the node, or to ground, the parent wrote in that position.
5. WHEN a node is declared with `= value` THEN the system SHALL record the value as the node's initial potential, on the body node and on the design node.
6. IF a node's `= value` reads a param THEN the system SHALL reject the model with `P0001` at the value.
7. IF a placement has no `let` of its own THEN the system SHALL name the instance `<module>#<k>` as §21 rule 3 states.
8. IF a module places itself, directly or through other modules, THEN the system SHALL reject the model with `E0341` and SHALL name the module.

**Amendment (T4, orchestrator, msl-elab verification round 1)**: a child's own signals are shown as `gnd` in the authored `Child::nodes`. `Ir::check` forces one slot per node of the child's body and `Terminal` has no "own node", so the authored view pads them with `Ground`; the instance table (`InstanceSpec::nodes`, `Instance::node`) carries the real node (`Top.l.inner`), and backends bind from it. No M1 device has a signal. Pinned by `a_childs_own_signal_is_gnd_in_the_authored_child_and_real_in_the_instance_table`; follow-up in `ROADMAP.md` ("M1 follow-ups").

**Independent Test**: elaborate `DiodeSeries`; list the paths `DiodeSeries.v1`, `.r1`, `.d1`, `.d2` in that order, and read that `DiodeSeries.d1` and `DiodeSeries.d2` both bind `DiodeSeries.b`.

---

### P1: The analog block ⭐ MVP

**User Story**: As a backend author, I want each `analog` block lowered to the IR's analog nodes so that I execute it without reading the POM.

**Why P1**: every M1 device is an `analog` block.

**Requirement**: ELAB-04

**Acceptance Criteria**:

1. The system SHALL lower each value expression, `let` and function of the type-checked set to the IR node of the same form, and each function once however many bodies call it.
2. The system SHALL lower a potential probe to a `Probe` over the terminals of its nodes, and `temperature()`, `gmin()` and `source_scale()` to their `Env` nodes.
3. The system SHALL create one branch per unordered pair of terminals that receives a contribution, with the kind potential when any contribution to it is a potential contribution and flow otherwise (§11).
4. WHEN a contribution's right side is a sum THEN the system SHALL split it at its top-level `+` and `-` into addends in source order, an addend that is exactly `ddt(x)` becoming an addend under `ddt` with value `x`, and every maximal run of other addends becoming one direct addend.
5. WHEN a contribution is written on the reversed pair of an existing branch THEN the system SHALL negate its addends (§11 rule 10).
6. The system SHALL give each `limit` placement and each `ac_stim` call of a block its own site, numbered in source order, and SHALL name a `limit` site after the `let` it initializes when it has one.
7. IF both nodes of a contribution are the same node THEN the system SHALL reject the model with `E0241`.
8. IF a branch receives both potential and flow contributions THEN the system SHALL reject the model with `E0415`.

**Amendment (T8)**: (a) only a chain that holds a `ddt` leaf is split at its `+`/`-`; a chain with no `ddt` stays one direct addend as written, so no floating-point sum is re-associated. (b) A leading-negative run becomes an addend with `negated` set, its first leaf's sign. (c) Refused with `P0001` (table `elab/unlowered.rs`): a `ddt` that is not a whole addend of a flow contribution, a `ddt` with a tolerance, a `ddt` in a potential contribution. (d) ELAB-06 AC 3 and AC 1 in part are met earlier than written: `m`/`dtemp` in a placement are `E0320` from the front end, a `limit`/`ac_stim` in a function or closure is `E0450`, and an `extern` module is a type-check `P0001` at its declaration. **Confirmed**: accepted by the orchestrator under the user's standing delegation ("Ok, deixo tu ir tocando..."), msl-elab verification round 1: the model is refused either way, with no `Ir` and nothing silent. (e) The right operand of `&&`/`||` is conditional: a `limit` or an `ac_stim` there is `P0001` like one under an `if`.

**Independent Test**: elaborate the `Diode`; its one contribution prints as `b0.flow +> direct (id + (gmin() * vd)), ddt (cj0 * vd)` and its site as `limit L0 "vd"`.

---

### P1: Nothing is dropped silently ⭐ MVP

**User Story**: As a maintainer, I want a form elaboration cannot lower to stop it so that a partial implementation never gives a wrong simulation.

**Why P1**: project rule "fail loud"; M1 is a subset by decision.

**Requirement**: ELAB-06

**Acceptance Criteria**:

1. IF a contribution, a `limit` placement or an `ac_stim` call is written under an `if`, in a function or in a closure THEN the system SHALL reject the model with `P0001` at that form.
2. IF `ddt` is written anywhere but as a whole addend of a flow contribution THEN the system SHALL reject the model with `P0001` at the `ddt`.
3. IF a placement sets `m` or `dtemp`, or places an `extern` module, THEN the system SHALL reject the model with `P0001` at the placement.
4. IF a module of the reachable set has a `digital` block THEN the system SHALL reject the model with `P0001` at the block.
5. The system SHALL return no `Ir` whenever it returns a diagnostic.

**Independent Test**: one negative fixture per criterion, each asserting the code and the span.

---

### P1: Static checks of the subset ⭐ MVP

**User Story**: As a model author, I want the elaboration-phase rules that apply to my model checked so that a non-conforming model never runs.

**Why P1**: §0.3 item 3.

**Requirement**: ELAB-05

**Acceptance Criteria**:

1. The system SHALL attach to every diagnostic a code and the span of the construct, and to a diagnostic about an instance its path (§0.3 item 4).
2. WHEN a model of the subset meets none of the conditions of ELAB-02 to ELAB-06 THEN the system SHALL return an `Ir` and no diagnostic.

**Independent Test**: the five twins elaborate with no diagnostic; each negative fixture of ELAB-02 to ELAB-06 carries a code and a span inside the fixture's text.

---

## Edge Cases

- WHEN a module has no `analog` block and places nothing THEN the system SHALL produce a body with no branches and an instance with no children.
- WHEN the same module is placed under two parents THEN the system SHALL produce one body and two instances.
- IF a node is connected to no instance THEN the system SHALL keep it in the node table. A floating node is the solver's to report (`E0503`).
- WHEN a contribution has one addend that is not a sum THEN the system SHALL produce one direct addend.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| ELAB-01 | P1: One call from a design to an IR | Design | Pending |
| ELAB-02 | P1: Params and constants | Design | Pending |
| ELAB-03 | P1: Instances and nodes | Design | Pending |
| ELAB-04 | P1: The analog block | Design | Pending |
| ELAB-05 | P1: Static checks of the subset | Design | Pending |
| ELAB-06 | P1: Nothing is dropped silently | Design | Pending |

ELAB-07 (metadata), ELAB-08 (boundary reachability) and ELAB-09 (definition order) of the first draft are M2.

**Coverage:** 6 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` C3, S1, S5, S6, S7 - the decisions.
- `tests/ngspice/*.msl` - the models that must elaborate.
- `docs/spec/` §0.3, §6, §8, §8.1, §11, §14, §16, §21 - the elaboration phase of the subset.
- `.specs/features/device-abi/abi.md` §3, A1 - why the split into addends and the node ownership.
- `.specs/features/msl-ir/design.md` - the output.

---

## Success Criteria

- [ ] The five twins elaborate, and one golden file per twin pins the printed IR.
- [ ] The bodies of the five devices, printed in structural form (`Ir::print_structural`: no origins, arena order not significant), equal the structural golden texts `msl-ir` pins for its hand-built ones: that form is the contract `ir-interp` is tested on.
- [ ] Every error code named in this spec has a negative fixture asserting the code and the span.
- [ ] No test finds a `Design` changed by elaboration.

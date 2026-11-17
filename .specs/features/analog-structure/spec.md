# Analog Structure Specification

## Open questions for the user

None of these is decided. Each has a recommendation; the spec is written to the recommendation and marks where an answer changes it.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does the authored `Child` record of the IR name a child's own signal (S4)? | (a) a new terminal `Own(k)` in `piperine-ir` (`model.rs`), printed and parsed, so the authored child names its own node `k`; (b) `Child::nodes` binds the ports only and `Ir::check` stops asking for a slot per own signal; the instance table stays the only place that lists the child's own nodes | **(a)**: the authored tree stays a full view of the source (the navigability rule), and the IR text says what it means. (b) is smaller but leaves a reader of the IR to cross-reference a second table. AS-16 is written to (a) |
| Q2 | Three spec models that need this feature also need another one: the stdlib `idt` and `Water` (generic `idt::<S>`, `generics` L30) and the `Diode` of §18 (`rs: Option<f64>`, `params-and-io` L28). Where do their runs land? | (a) this feature proves each mechanism on non-generic local copies written from the spec text (`Integ` over `State`, a `Switch`, `CapIc`, `Bounded`), and AS-05 carries the three spec models as `WHERE <feature> has merged` criteria, verified by whichever of the two features merges last; (b) this feature waits for `generics` and `params-and-io` and runs the spec's own modules only | **(a)**: L40 blocks most of the analog stdlib, so it should not wait for two L-sized features. Under (a) `GAPS.md` keeps one line for the three models until the last dependency merges |
| Q3 | `slew` (§22.2.10) is refused only because its result holds a node (L35), yet `GAPS.md` L58 (owned by `generics`) lists it. Who closes the `slew` row? | (a) this feature: AS-09 AC 6 runs it, and `generics` drops it from its L58 list; (b) `generics`, as the coverage map says | **(a)**: `slew` is not generic; it is the stdlib's own example of L35 |

## Problem Statement

A Piperine `analog` block is straight-line code today. A contribution under an `if` or a `match`, a `ddt` anywhere but a whole addend, a value input of a module without a result, a module whose result reads its own node, `inst.name` on a child, and a body `let` passed as a param value are all `P0001` (`GAPS.md` §4.2 to §4.4). Part IV §11 and Part III §8.3 build the analog half of the language on exactly these forms: switch branches (`Switch`, `CapIc`, the bounded state of §11.2, `Water`), every stateful operator a user or the stdlib writes (`idt`, `slew`, filters), and the §18 diode. Until they lower, a user cannot write an ideal switch, an integrator, an initial condition or a series resistance that a param removes.

One silent defect sits on the same path: the authored `Child` record of the IR pads a child's own signals with ground (S4).

## Goals

- [ ] Every form of §11 rules 1 to 4 and 18 runs: a contribution under `if`, `match` and a nested block; switch branches selected per solve; kinds fixed at elaboration by params, with the merge of a constant-zero potential branch.
- [ ] Every stateful operator written as a module with a result that reads its own node runs, with feedback through it (`let y = Integ(f(y), ..)`), and `slew` from the stdlib runs.
- [ ] Value inputs of a module without a result, `inst.name` on `pub` items, a body `let` as a param value, `p.inflow()`, `changed()` and `prev()` on a param, and an initial value that reads a param run.
- [ ] `ddt` is accepted wherever §14.5 places a core module, with its `abstol`.
- [ ] The 7 ngspice twins print the bytes they print today; two new twins (`uic_rc`, `diode_rs`) agree with ngspice under the existing tolerances.
- [ ] `GAPS.md` no longer lists L26, L27, L34 to L38, L40, L42, L50 or S4 (Q2 (a): one line remains for the three spec models until their dependencies merge).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Contributions and placements inside `for` with a constant trip count (§11 rule 12, `E0416`) | Loops are L5, owned by `composite-values` |
| Generic modules, so the stdlib `idt`, `idtmod`, `Short<N>` and `Water` as the spec writes them | L30, owned by `generics` (Q2) |
| `Option`, integer, enum and module params (`rs: Option<f64>` of the §18 `Diode`) | L28, owned by `params-and-io` (Q2) |
| `m` and `dtemp`, and the effective multiplicity that divides `inflow()` (§8.5.4) | L29, owned by `generics`. This feature reads `inflow()` with multiplicity 1 |
| A host `set` of a param during a run (`Session::set_at`) and the recompute of derived params (S2) | `host-ii` (HII-13, X9). AS-12 AC 4 and AS-14 use them once merged |
| Naming the branches or the node of a singular solve (`E0503`) and other run errors | `solver-hygiene` (V15, `GAPS.md` §4.8). This feature fails the run; the naming is theirs |
| LTE on charge, so that the `abstol` of `ddt` shapes the step | `solver-hygiene` (V1). AS-07 AC 5 is verified once it merges |
| `ddx`, `delay`, `noise`, `simparam`, `emit!` in `analog` | `analog-operators` |
| An unknown selector (`if` on a carrier whose level is unknown) as a merge | L19, owned by `unknown-semantics`. A selector reads carriers through `level` as today (§11 rule 13) |
| Arrays, bundles and resolved nets of carriers on contribution nodes | `composite-values` (L24), `resolved-nets` (L25) |
| Operating-point variables read by the host | `host-iii` |
| The interpreter running any form this feature adds | R1: the interpreter is frozen at the M1 analog subset; it refuses by name (AS-17) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Backends | every form lands on the Rust backend (`piperine-transpile`); the interpreter refuses each by name | R1, R2 (`.specs/features/codegen/context.md`) | y - R1/R2 |
| The oracle | ngspice where a twin can exist (`uic_rc`, `diode_rs`, the rewritten capacitor on `tran_rc` and `ac_rc`); the spec's closed forms elsewhere | R2: where the interpreter lacks a form the oracle is the spec's expected behaviour and ngspice | y - R2 |
| Tolerances | no `ToleranceContract` constant changes; closed-form checks use `TRAN_VOLTAGE`, `AC_MAGNITUDE`, `AC_PHASE` or `DC_VOLTAGE` | MD-38: a tolerance is written once, in `contract.rs` | n - planner |
| How a switch branch reaches the ABI | design decides (a branch with both a flow and a potential row, selected per solve, or one per kind); the criteria state outcomes only | the spec is normative on behaviour, not on the lowering | n - planner |
| How a stateful core result anywhere in an expression lowers (`2.0 * ddt(q)`) | design decides (for instance an internal unknown carrying the result) | ABI `Term` ops are whole addends of a row (abi.md A1); the language allows `ddt` anywhere | n - planner |
| A param bound before the run (`--param`, a run table's `params`) | it is the elaborated value, not a change: `changed()` does not fire for it and `prev()` reads it | §5.3.1: on a param "a change is a host `set`"; a value given before the run is binding, not a `set` during it | n - planner |
| A placement under an untaken branch | it exists and its state advances; only its result is unused (§8.3.14) | the spec | y - spec |
| Q1 S4 terminal | `Own(k)` terminal | recommendation; see the top of this file | n - open |
| Q2 spec models needing other features | local copies here; the spec's models as `WHERE` criteria | recommendation; see the top of this file | n - open |
| Q3 `slew` | run here | recommendation; see the top of this file | n - open |

**Open questions:** Q1, Q2 and Q3 at the top of this file are open; the spec is written to each recommendation.

**Implicit-requirement dimensions.** Input validation and bounds: AS-02 AC 1, 2, 5 (`E0414`, `E0415`), AS-08 AC 4 (`E0336`), AS-10 AC 2 (`E0353`), AS-11 AC 5 and 6, AS-12 AC 3 (`E0321`), AS-13 AC 4 and 5, AS-15 AC 3. Failure states: AS-03 AC 3 (`E0418` at run). Idempotency and retry: N/A because elaboration and a run are deterministic and a session runs once. Auth and rate limits: N/A because the tool is local. Concurrency and ordering: AS-03 AC 2 (a kind changes only between solves, at a jump). Data lifecycle: AS-09 AC 4 (state of a placement under an untaken branch advances). Observability: AS-16 (the IR shows a child's own nodes). External-dependency failure: AS-18 AC 3 (the twins follow the harness's skip rule when ngspice is absent). State-transition integrity: AS-03 AC 2, AS-04 AC 1 and 2 (jumps keep the `ddt` state, §15.2.1).

---

## User Stories

### P1: Contributions under `if`, `match` and a nested block ⭐ MVP

**User Story**: As a model author, I want to write a contribution under an `if` or a `match`, so that one model chooses its equations by a param, a carrier or a probe as §11 allows.

**Why P1**: L40 blocks `Switch`, `CapIc`, the bounded state, `idt`, `slew` and the §18 diode: most of the analog stdlib.

**Acceptance Criteria** (AS-01, flow contributions under control flow):

1. WHEN `analog for M { if v(a) > 0.5 { i(a, gnd) +> v(a) / 1k; } else { i(a, gnd) +> v(a) / 2k; } }` is driven by a 1 mA current source into `a` THEN the system SHALL give `v(a)` = 1.0 V at the operating point within `ToleranceContract::DC_VOLTAGE`.
2. WHEN a `match` on a `Bit<Mode>` carrier holds a flow contribution in each arm THEN the system SHALL add, in each solve, only the contribution of the arm the carrier's settled value selects.
3. WHEN a contribution sits in a plain block nested in the `analog` block THEN the system SHALL lower it as if it were written at the block's top level.
4. WHEN no contribution to a branch is enabled in a solve THEN the system SHALL give that branch zero flow (§11 rule 4): `if en.high() { i(a, gnd) +> v(a) / 1k; }` beside a 1 MΩ resistor and a 1 mA source gives `v(a)` = 1000 V with `en` low and 0.999 V with `en` high, each within `DC_VOLTAGE`.
5. WHEN `if ctl.high() { bound_step(1n); }` runs in a transient THEN the system SHALL cap at 1 ns every step whose accepted solution executed the call, and SHALL cap no step whose accepted solution did not (§14.10.2; the case `msl-fixes` Q4 left to L40).
6. IF a contribution sits in the body of a closure THEN the system SHALL keep refusing it at parse (Appendix B.2 rule 3).

**Acceptance Criteria** (AS-02, the selector rule, §11 rule 3):

1. IF the selector of a contribution to a branch that has a potential contribution anywhere in the module depends on a probe, or on a result that reads a node, THEN the system SHALL report `E0414` at elaboration naming the branch and the probe.
2. WHEN a module's selector reads a value input THEN the system SHALL judge it per instance after substitution: a placement that binds the input to a probe gets `E0414`, a placement that binds it to a param does not.
3. WHEN every selector of a branch depends on params only THEN the system SHALL fix the branch's kind at elaboration and list those params as structural (§8.1.2).
4. WHEN a branch of fixed kind potential has only constant-zero potential contributions and no flow probe, directly or through `inflow()`, THEN the system SHALL merge its two nodes: the bound circuit has one node fewer than the same design with the contribution `v(a, b) +> 1.0`.
5. IF the selectors of a branch depend on params only and enable both a potential and a flow contribution THEN the system SHALL report `E0415` at elaboration naming the branch.
6. WHEN a branch's selector reads a discrete carrier or the environment of the solve THEN the system SHALL keep the branch's flow unknown in every solve.

**Independent Test**: lone `.msl` probes run on the Rust backend with `piperine run --analysis op|transient`, and one elaboration test per error code.

---

### P1: Switch branches ⭐ MVP

**User Story**: As a model author, I want a branch that is a short in some solves and open in others, so that I can write an ideal switch, an initial condition and a held state.

**Why P1**: §11 rule 2 and §11.2 are the language's only way to write an exact switch or bound.

**Acceptance Criteria** (AS-03, switch branches at run):

1. WHEN `Switch(a, gnd)` (§11) sits behind 1 kΩ from a 1 V source and its `ctl` rises at 1 µs THEN the system SHALL give `v(a)` = 1 V before 1 µs and 0 V after it, and the flow of `(a, gnd)` 0 before and 1 mA after, each within `TRAN_VOLTAGE` or `DC_CURRENT`.
2. WHEN a discrete carrier that a selector reads changes THEN the system SHALL change the branch's kind only between solves: the solve at that instant uses the old kind and the solve after the change uses the new one, as a jump (§11 rule 18, §15.2).
3. IF a solve enables both a potential and a flow contribution to one branch THEN the system SHALL fail the run with `E0418` naming the instance and the branch.
4. WHEN an AC sweep linearizes a switch branch THEN the system SHALL use the kind its selector gives with `analysis(Static)` false (§11.1.5).
5. WHEN `CapIc` (§16, `c` = 1.5915494309189535 nF, `ic` = 2.0) replaces the capacitor of `uic_rc` THEN the system SHALL agree with `tests/ngspice/uic_rc.cir` under `TRAN_VOLTAGE` at every print row (new twin `uic_rc.msl`).
6. WHEN `CapIc` is swept in AC THEN the system SHALL give it the admittance of a capacitor: behind 1 kΩ from a 1 V AC source, `|v(out)|` = 1/√2 at 100 kHz within `AC_MAGNITUDE`.

**Acceptance Criteria** (AS-04, the patterns of §11.2 and §22.2.8, written locally):

1. WHEN `Bounded` (§11.2, `hi` = 1.0) runs a transient with `v(p, n)` = +1 kV until 2 s and −1 kV after THEN the system SHALL give `pos(xs)` = 0.5 at 0.5 s, 1.0 from 1 s to 2 s, and 0.5 at 2.5 s, each within `TRAN_VOLTAGE`.
2. WHEN a local `Integ` (the text of §22.2.8 with `S` replaced by `State`) integrates `x` = 1.0 with `ic` = 0.5 and `reset` true until 1 ms THEN the system SHALL give 0.5 until 1 ms and `0.5 + (t − 1 ms)` after it, within `TRAN_VOLTAGE`.
3. WHEN that `Integ` with `ic` given is swept in AC with `x` = `v(in)` and 1 V AC at `in` THEN the system SHALL give `|result|` = 1/(2πf) at each frequency within `AC_MAGNITUDE`.
4. WHEN that `Integ` with `ic` given runs an operating point THEN the system SHALL hold its result at `ic`.

**Acceptance Criteria** (AS-05, the spec's own models that need another feature; Q2):

1. WHERE `generics` (L30) has merged the system SHALL run the stdlib `idt` (§22.2.8) and give the results of AS-04 AC 2 to 4 with `idt` in place of `Integ`.
2. WHERE `generics` (L30) has merged, WHEN `Water` (§13) is heated by 1 kW from 300 K THEN the system SHALL give `phase` = `Boiling` at the time `temp(t)` reaches 373.15 K, hold `temp(t)` at 373.15 K while `absorbed` < `mass · hv`, and give `phase` = `Vapor` after, each crossing within the default `ttol`.
3. WHERE `params-and-io` (L28) has merged, WHEN the §18 `Diode` with `rs: Some(100.0)` runs the circuit of a new twin `diode_rs` (`tests/ngspice/diode_rs.cir`: 5 V, 1 kΩ, a diode with `rs=100`) THEN the system SHALL agree with ngspice under `DC_VOLTAGE`.
4. WHERE `params-and-io` (L28) has merged, WHEN the §18 `Diode` has `rs` left out THEN the system SHALL merge `a` and `ai` (AS-02 AC 4) and agree with `tests/ngspice/rdiode.cir` under `DC_VOLTAGE`.

**Independent Test**: `cargo test -p piperine-parity --test parity_tran` for `uic_rc`; transpile tests of `Switch`, `CapIc`, `Bounded` and `Integ` against their closed forms.

---

### P1: Limiting, stimulus and step bounds under control flow, reading a carrier ⭐ MVP

**User Story**: As a model author, I want `limit` and `ac_stim` under an `if` and reading a carrier, so that a device whose junction or source a param or a carrier enables can still be limited and swept.

**Why P1**: L50 is the same lowering as L40 for the two held inputs of the ABI.

**Acceptance Criteria** (AS-06):

1. WHEN a `limit` instance sits under a branch not taken in an iteration THEN the system SHALL treat it as inactive: it SHALL NOT veto convergence and SHALL keep its last value (§14.7.4).
2. WHEN the junction of the `devices.msl` diode is enabled by `en.high()` and `en` rises after the operating point THEN the system SHALL converge to the `rdiode` solution after the rise within `DC_VOLTAGE`.
3. WHEN `limit` reads a discrete carrier through `known_or` or `level` in its value input THEN the system SHALL accept it and limit the sum.
4. WHEN `if en.high() { i(a, gnd) +> ac_stim(1m); }` is swept in AC with 1 kΩ from `a` to ground THEN the system SHALL give `|v(a)|` = 1 V with `en` high at the operating point and 0 V with `en` low.
5. WHEN `ac_stim(mag.known_or(0.0))` reads a `Bit<f64>` carrier settled at 2m THEN the system SHALL give the phasor 2m at the operating point.

**Independent Test**: transpile tests; the `rdiode` twin values as the expected numbers.

---

### P1: `ddt` wherever a core module goes ⭐ MVP

**User Story**: As a model author, I want `ddt` inside any expression, in a body `let` and in an event, with its `abstol`, so that a charge model is written as the physics reads.

**Why P1**: L42 refuses `2.0 * ddt(q)`, which most charge models contain.

**Acceptance Criteria** (AS-07):

1. WHEN the capacitor of the twins is written `i(p, n) +> 2.0 * ddt(0.5 * c * v(p, n));` THEN `tran_rc` SHALL agree with ngspice under `TRAN_VOLTAGE` and `ac_rc` under `AC_MAGNITUDE` and `AC_PHASE`.
2. WHEN `ddt` is a factor of a product with a probe (`i(p, n) +> k * v(c) * ddt(v(p, n));`) THEN the system SHALL give the current `k · v(c) · dv(p, n)/dt` in a transient and `k · v(c) · jω · v(p, n)` in AC, each within its contract.
3. WHEN a body `let` binds `ddt(x)` and `analog` reads it THEN the system SHALL give the same rows as the inline form.
4. WHEN a reaction or an event argument reads a `ddt` result THEN the system SHALL give 0 at an equilibrium solution and the step's derivative after it (§14.5.4): on `v(a)` = 1 kV/s · t, `on at(1m) { emit!("d", x = dv); }` with `let dv = ddt(v(a));` records 1000 within 1e-6 relative.
5. WHEN `ddt(x, abstol)` is given THEN the system SHALL hand the solver `abstol` as the state tolerance of that `ddt` term, and WHERE `solver-hygiene` V1 has merged an RC with `ddt(q, 1e-3)` SHALL take fewer accepted steps than the same RC with `ddt(q, 1e-18)`.

**Independent Test**: the twins with the rewritten capacitor; transpile tests for AC 2 to 4.

---

### P1: Modules in expressions: value inputs, results with a node, feedback ⭐ MVP

**User Story**: As a model author, I want modules that take value inputs and return results reading their own nodes, including feedback through them, so that I can write integrators, lags and controlled sources as §8.3 describes.

**Why P1**: §8.3 opens with "every operator with state is written this way".

**Acceptance Criteria** (AS-08, value inputs of a module without a result, L34):

1. WHEN `module G(o: Electrical, x: f64) {}` with `analog for G { v(o) +> x; }` is placed `G(out, 2.0 * v(a))` and `v(a)` = 1 V THEN the system SHALL give `v(out)` = 2.0 V at the operating point.
2. WHEN that circuit is swept in AC with 1 V AC at `a` THEN the system SHALL give `|v(out)|` = 2.0 within `AC_MAGNITUDE` (the callee's row differentiates what the input probes, §8.3.16).
3. WHEN a value input reads a `Bit<f64>` carrier of the caller through `known_or` and the carrier changes at 1 µs THEN the system SHALL give the callee the new value from that instant.
4. IF a continuous argument is read by a definition of the callee THEN the system SHALL report `E0336` at the call (§8.3.9).
5. WHEN a value input is read in a reaction of the callee THEN the system SHALL evaluate the caller's expression at the reaction's instant (§8.3.13).

**Acceptance Criteria** (AS-09, a result that holds a node, L35):

1. WHEN `module Lag(x: f64) -> f64 { tau: f64 = 1m; signal y: State; potential(y) }` with `analog for Lag { flow(y) +> tau * ddt(potential(y)) + potential(y) - x; }` is read as `v(out) +> Lag(v(in));` and `v(in)` steps from 0 to 1 V at 0 THEN the system SHALL give `v(out)` = 1 − e^(−t/τ) within `TRAN_VOLTAGE`.
2. WHEN that circuit is swept in AC THEN the system SHALL give `|v(out)/v(in)|` = 1/√(1 + (2πfτ)²) within `AC_MAGNITUDE`.
3. WHEN two placements with equal arguments are written THEN the system SHALL make two instances, each with its own node.
4. WHEN a placement sits under a branch not taken THEN the system SHALL still create the instance and advance its state (§8.3.14): with `en` rising at 2τ, `if en.high() { v(out) +> Lag(v(in)); } else { v(out) +> 0.0; }` gives `v(out)` = 1 − e^(−2) just after 2τ within `TRAN_VOLTAGE`.
5. WHEN a body `let l = Lag(v(in));` is read by `analog` and by `on above(l - 0.5)` THEN the system SHALL read one instance in both, and the reaction SHALL fire at τ·ln 2 within the default `ttol`.
6. WHEN the stdlib `slew(v(in), 1M, 1M)` follows a 0 to 1 V step at 1 µs THEN the system SHALL give 0 V before 1 µs, a ramp of 1 V/µs, and 1 V from 2 µs, within `TRAN_VOLTAGE`; and `v(in)` at an operating point (Q3).

**Acceptance Criteria** (AS-10, feedback through an instance, L38):

1. WHEN `let y = Integ(-y / 1m, 1.0, None);` (AS-04's `Integ`) runs a transient THEN the system SHALL give `y` = 1.0 at the operating point and e^(−t/1 ms) after it, within `TRAN_VOLTAGE`.
2. IF a cycle of named expressions, value inputs and results has a direct dependence at every result (`let y = ddt(f(y));`) THEN the system SHALL report `E0353` naming the names of the cycle.
3. The system SHALL NOT report the `P0001` "the feedback of .. through an instance" for a cycle that passes through an unknown of a module.

**Independent Test**: transpile tests against the closed forms; elaboration tests for `E0336` and `E0353`.

---

### P1: Reaching into a child ⭐ MVP

**User Story**: As a model author, I want to probe a child's public node, attach a branch to it and read its public carrier, param or named expression, so that a parent composes children as §8.2.8 allows.

**Why P1**: L36 forbids every `inst.name`; hierarchical models cannot read their parts.

**Acceptance Criteria** (AS-11, L36):

1. WHEN a child declares `pub signal mid: Electrical` and the parent writes `v(c.mid)` THEN the system SHALL give the potential of the child's node `mid`.
2. WHEN the parent writes `i(c.mid, gnd) +> v(c.mid) / 1k;` THEN the system SHALL add the branch as the parent's own, loading the child's node.
3. WHEN the parent reads `c.state` of a child's `pub signal state: Bit<u8>` in a reaction, a definition or, through `known_or`, in `analog` THEN the system SHALL give the child's carrier's value.
4. WHEN the parent reads `c.r` of a child's `pub` param or `c.e` of a child's `pub let` THEN the system SHALL give the child's value, the `let` evaluated in the reader's context (§8.2.25).
5. IF `inst.name` names an item that is not `pub` THEN the system SHALL report `E0343` naming the item.
6. IF the parent commits to or defines a child's carrier THEN the system SHALL report `E0222` naming the carrier.

**Independent Test**: transpile tests over a two-level design.

---

### P1: Params, initial values and history ⭐ MVP

**User Story**: As a model author, I want a computed `let` to set a child's param, a carrier to start at a param's value, and a param's change to be an event, so that a model is parameterized end to end.

**Why P1**: L37 and L27 refuse the commonest parameterization idioms; L26 refuses two probes of the spec.

**Acceptance Criteria** (AS-12, a body `let` as a param value, L37):

1. WHEN `let c = 1.0 / (2.0 * PI * r * fc); Capacitor(b, gnd) { c }` sits behind the module's `r` THEN the system SHALL give the low-pass corner at `fc`: `|v(b)|` = 1/√2 of the input at `fc` within `AC_MAGNITUDE`.
2. WHEN that `let` reads another body `let` over params and constants THEN the system SHALL accept it.
3. IF a `let` used as a param value reads a carrier, a probe, the environment or an instance result THEN the system SHALL report `E0321` naming the `let`.
4. WHERE `host-ii` (X9) has merged, WHEN a host sets `r` THEN the system SHALL recompute the child's `c`.

**Acceptance Criteria** (AS-13, `p.inflow()`, L26):

1. WHEN a module has `i(p, gnd) +> 1m;` and `i(p, n) +> v(p, n) / 1k;` and writes `v(m) +> p.inflow() * 1k;` on an internal node `m` THEN the system SHALL give `v(m)` = 1k · (1 mA + `v(p, n)`/1 kΩ).
2. WHEN a child of the module has a branch ending on the module's port `p` THEN the system SHALL include that branch's flow in `p.inflow()` (G.59).
3. WHEN that module is swept in AC THEN the system SHALL differentiate `inflow()` like any probe.
4. IF `inflow()` is applied to something other than a port of a nature with `flow` THEN the system SHALL report `E0243`.
5. IF `p.inflow()` is the left side of `+>` THEN the system SHALL report `E0410`.

**Acceptance Criteria** (AS-14, `changed()` and `prev()` on a param, L26):

1. WHERE `host-ii` (HII-13) has merged, WHEN a host sets `r` at 1 ms during a transient THEN `on r.changed()` SHALL fire once, at 1 ms.
2. WHEN `r.prev()` is read before any `set` THEN the system SHALL give the param's own value; after a `set`, the value before it.
3. WHEN a param is bound before the run (`--param`, a run table's `params`) THEN the system SHALL NOT fire `changed()` for it.

**Acceptance Criteria** (AS-15, an initial value that reads a param, L27):

1. WHEN `signal s: Bit<f64> = k0;` runs with `k0` = 2.5 THEN the system SHALL give `s` = 2.5 from `(0, 0)`, and with `--param Top.k0=4.0` SHALL give 4.0.
2. WHEN `signal a: Electrical = v0;` runs a transient THEN the system SHALL hold `a` at `v0` through the transient's operating point (§16) and start Newton from it in an `op`.
3. IF an initial value reads a carrier or a probe THEN the system SHALL report `E0210`.

**Independent Test**: transpile tests; the AC corner of AS-12 against its closed form.

---

### P2: The IR shows a child's own nodes

**User Story**: As a reader of the IR, I want the authored child record to name the child's own signals, so that walking the IR's children never shows ground where a node exists.

**Why P2**: The simulation is right today; the authored view is wrong (S4).

**Acceptance Criteria** (AS-16, S4; Q1):

1. WHEN a placed module declares an own `signal` THEN the system SHALL bind that slot of the authored `Child` record to the child's own node, never to ground.
2. WHEN the IR of such a design is printed and parsed back THEN the system SHALL give an equal IR.
3. The system SHALL pass `Ir::check` on every such design.

**Independent Test**: `cargo test -p piperine-ir` and an elaboration test reading the authored child.

---

### P2: The interpreter refuses, parity holds, the documents follow

**User Story**: As the owner of the parity claim and the M1 interpreter, I want every new form refused by name in the interpreter and every existing comparison untouched.

**Why P2**: R1 freezes the interpreter; a silent run there would be wrong.

**Acceptance Criteria** (AS-17):

1. IF a design that reaches the interpreter holds a form this feature adds (a conditional contribution, a switch branch, a value input, a result that holds a node, `ddt` off a whole addend or with `abstol`, `inst.name`, `inflow()`, `changed()` or `prev()` on a param) THEN the interpreter SHALL refuse it naming the form, never run it.

**Acceptance Criteria** (AS-18):

1. WHEN each of the 7 twins runs with the flags of `crates/piperine-parity/tests/twins.rs` THEN the system SHALL print, on each backend, the bytes it printed before this feature.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran` and `parity_corpus` with `crates/piperine-parity/src/contract.rs` unedited.
3. WHERE the ngspice comparison is disabled the new twins SHALL run and print the harness's skip line.
4. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L26, L27, L34 to L38, L42, L50 and S4, SHALL list L40 only for the models of AS-05 whose dependency has not merged, and the Book's listings SHALL stay equal to the binary's output.

**Independent Test**: `cargo test -p piperine-parity`, `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN a branch receives flow contributions from two arms that are both enabled THEN the system SHALL sum them (§11 rule 1).
- WHEN `i(b, a) +> x` sits under one arm and `i(a, b) +> y` under another THEN the system SHALL treat both as the branch `(a, b)`, the first as `−x`.
- WHEN a switch branch's selector is unchanged across an instant THEN the system SHALL make no jump for it.
- WHEN a switch branch with a potential arm sits inside a result module THEN the system SHALL apply the selector rule per instance (§11 rule 3, last sentence).
- IF a contribution under an `if` targets a branch `(a, a)` THEN the system SHALL report `E0241`.
- WHEN a placement inside a result module places another result module THEN the system SHALL hoist both under their placement paths (MD-64).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| AS-01 | P1: contributions under control flow | - | Pending |
| AS-02 | P1: contributions under control flow | - | Pending |
| AS-03 | P1: switch branches | - | Pending |
| AS-04 | P1: switch branches | - | Pending |
| AS-05 | P1: switch branches | - | Pending |
| AS-06 | P1: limit, stimulus, step bounds | - | Pending |
| AS-07 | P1: `ddt` anywhere | - | Pending |
| AS-08 | P1: modules in expressions | - | Pending |
| AS-09 | P1: modules in expressions | - | Pending |
| AS-10 | P1: modules in expressions | - | Pending |
| AS-11 | P1: reaching into a child | - | Pending |
| AS-12 | P1: params, initial values, history | - | Pending |
| AS-13 | P1: params, initial values, history | - | Pending |
| AS-14 | P1: params, initial values, history | - | Pending |
| AS-15 | P1: params, initial values, history | - | Pending |
| AS-16 | P2: the IR shows a child's own nodes | - | Pending |
| AS-17 | P2: interpreter, parity, documents | - | Pending |
| AS-18 | P2: interpreter, parity, documents | - | Pending |

**Coverage:** 18 total, 0 mapped to tasks, 18 unmapped (no tasks yet).

## Gaps closed

| `GAPS.md` id | What | Closed by |
| ------------ | ---- | --------- |
| L26 | `p.inflow()`; `changed()` on a param | AS-13 (`inflow`), AS-14 (`changed`, `prev`; AC 1 once `host-ii` HII-13 merges) |
| L27 | an initial value that reads a param | AS-15 |
| L34 | a value input of a module without a result | AS-08 |
| L35 | a module with a result that holds a node | AS-09 (with `slew`, Q3) |
| L36 | `inst.name` on a `pub` item | AS-11 |
| L37 | a body `let` as a param value | AS-12 (AC 4 once `host-ii` X9 merges) |
| L38 | feedback through an instance | AS-10 |
| L40 | a contribution under `if` or `match`: switch branches, `Switch`, `CapIc`, `Bounded`, `Water`, the §18 `Diode`, the reset of `idt` | AS-01, AS-02, AS-03 (`Switch`, `CapIc`), AS-04 (`Bounded`, the `idt` reset on `Integ`), AS-05 (`idt`, `Water` once `generics` merges; the §18 `Diode` once `params-and-io` merges) |
| L42 | `ddt` not a whole addend; `ddt` with `abstol` | AS-07 (AC 5's step count once `solver-hygiene` V1 merges) |
| L50 | `limit` or `ac_stim` under an `if`; either reading a carrier | AS-06 (and AS-01 AC 5 for `bound_step`, left to L40 by `msl-fixes` Q4) |
| S4 | a child's own signals read as `gnd` in the authored `Child` | AS-16 |

## Dependencies

| Feature | What this feature needs from it |
| ------- | ------------------------------- |
| `msl-fixes` | `analysis(Transient)` (L48, FIX-11) for `CapIc`; straight-line `bound_step` (L46, FIX-09) before AS-01 AC 5; a second `analog` block (L41) for nothing here, but merged first to avoid conflicts in `elab/` |
| `host-ii` | `Session::set_at` (HII-13) for AS-14 AC 1; derived-param recompute (X9, S2) for AS-12 AC 4 |
| `generics` | the stdlib `idt` and `Water` (AS-05 AC 1, 2) |
| `params-and-io` | `Option` params for the §18 `Diode` (AS-05 AC 3, 4) |
| `solver-hygiene` | V1 for AS-07 AC 5's step count |
| `analog-operators` | none; it depends on this one (`delay` anywhere shares AS-07's lowering of a stateful core result) |

## Size

**L.** About 30 to 35 tasks in five phases: conditional contributions and the selector rule (AS-01, AS-02, AS-06); switch branches at run and the local patterns (AS-03, AS-04); stateful results anywhere (AS-07, AS-09, AS-10, AS-08); structure (AS-11 to AS-16); refusals, twins, documents (AS-17, AS-18). AS-05 adds one task per dependency when it merges.

## Sources

- `GAPS.md` (audit of 2026-11-17): L26, L27, L34 to L38, L40, L42, L50, S4; §9 and the "Analog structure" cluster of §10.
- `docs/spec/part_iv_behavior.md` §11 (rules 1 to 4, 9 to 13, 17, 18), §11.1.5, §11.2, §14.5, §14.7, §14.10.
- `docs/spec/part_iii_structure.md` §8.1.13, §8.1.19, §8.2.8, §8.2.25, §8.3.1 to §8.3.18.
- `docs/spec/part_ii_carriers.md` §5.1.5, §5.3.1, §6 rules 5, 6, 10, G.59.
- `docs/spec/part_v_execution.md` §15.2, §16 (`CapIc`), §18 (`Diode`); `part_vii_stdlib.md` §22.2.8 (`idt`), §22.2.10 (`slew`).
- `.specs/STATE.md` MD-38, MD-62, MD-64; `.specs/features/codegen/context.md` R1, R2.
- `feature/msl-fixes` `spec.md` Q4 (`bound_step` under `if` left to L40); `feature/host-ii` `spec.md` (HII-13, X9).
- `tests/ngspice/uic_rc.cir`, `rdiode.cir`, `tran_rc.cir`, `ac_rc.cir`.

---

## Success Criteria

- [ ] `Switch`, `CapIc`, `Bounded`, `Integ`, `Lag` and `slew` run on the Rust backend and match their closed forms; `uic_rc` matches ngspice.
- [ ] Every `P0001` of `elab/unlowered.rs` that this feature owns (`ConditionalContribution`, `NestedContribution`, `ConditionalLimit`, `ConditionalStim`, `DdtOffTop`, `DdtTolerance`, `ValueInput`) and the feedback refusal of `types/result.rs` are gone.
- [ ] The 7 twins print the bytes they printed before.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

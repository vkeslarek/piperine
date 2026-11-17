# Conformance Specification

What runs today is proven on seven small analog circuits and on the buck's own SPEC. This feature gives every other claim an oracle that can falsify it: ngspice twins for every device and domain, an external oracle for digital and mixed behaviour, the galleries run against their traces, a test for every rule of Part V, AC and DC of mixed designs, and a stated meaning for backend agreement past the interpreter's subset.

## Open questions for the user

Nothing below is decided. Each question has options and the planner's recommendation.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | The MSL standard library has no device model: `crates/piperine-lang/std/` holds natures, carriers, the analog functions of §22 and converters, and `tests/ngspice/devices.msl` holds `Resistor`, `Capacitor`, `Vsource`, `Isource` and `Diode`. Where do the new MOS 1 to 3, BJT and JFET models live? | **A.** In the parity fixture library (`tests/ngspice/devices.msl`, or one file per device family beside it). **B.** In an MSL package of their own (`spice`) that users can depend on, the twins importing it. **C.** In `std` (a change of §22) | **A.** They exist to be compared. B makes them a user surface with its own versioning; it can lift them later unchanged |
| Q2 | What is the oracle for digital and mixed behaviour (U2)? | **A.** The spec's own traced examples, turned into golden records by hand (§15.1, F.5, the galleries' trace tables). **B.** External simulators: Icarus Verilog twins for digital-only designs (`iverilog` is installed) and ngspice XSPICE twins (digital code models with `adc_bridge`/`dac_bridge`; `digital.cm` ships with the installed ngspice-47) for mixed ones, compared by event times and values. **C.** A second implementation: extend the frozen interpreter to digital | **A and B.** A is exact but only as wide as the spec's examples; B is independent and as wide as the twins written. C doubles the cost of every language feature (Q3) |
| Q3 | What does "the two backends agree" mean past the interpreter's M1 analog subset (U9)? | **A.** The interpreter stays frozen. Inside the subset, every program of the tree runs on both backends and agrees bit for bit; outside it, the interpreter refuses by name, and the compiled backend's second opinion is the external oracles of Q2 and the ngspice twins. **B.** Extend the interpreter to the whole language. **C.** Retire the interpreter | **A.** It keeps the one second implementation that exists honest and gives every other construct an independent oracle instead |
| Q4 | `four_diode` and `disto_diode` need a host Fourier analysis (`GAPS.md` H2's Fourier and distortion row), which the coverage map assigns to no feature. Who owns it? | **A.** This feature: a host Fourier of transient rows, as §16.3 defines distortion and harmonic measures; `four_diode` against ngspice `fourier`, `disto_diode` against ngspice `.disto` at its stated small amplitude. **B.** Another feature (e.g. `host-iii`); the two twins stay listed as blocked by it | ~~A~~ **Resolved by the orchestrator (2026-11-17): B.** `host-analyses` owns Fourier and distortion (HAN-05 to HAN-08, written in parallel); `four_diode` and `disto_diode` wait on it and assert its refusal until then |
| Q5 | MSL has no `uic` (§16 H6: a held start is always solved). How are `uic_rc` and `uic_mos` twinned? | **A.** With held nodes (`= value`): asserted against ngspice where the held start equals ngspice's UIC start, and recorded as a deliberate deviation asserting Piperine's own value where it does not (MD-38 corollary). **B.** Drop them | **A.** MD-38: a deviation is documented, never silent |
| Q6 | How is "every rule of Part V has a test" kept true (U6)? | **A.** As MD-51 did for the LRM: a test declares `//! SPEC-COVERS: <rule ids>`, and a gate parses Part V's rule ids (T1 to T6, I1 to I8, S1 to S7, O1 to O5, H1 to H7, A1 to A5, the numbered rules of §15.2, §16.1, §17.1, §17.2, §17.3, §18, §19) and requires each to be declared or excluded with an owner. **B.** One test for each of the six rules U6 names, no gate | **A.** B closes the six and leaves the next unnamed rule unnoticed |

## Problem Statement

`GAPS.md` §8 lists six things that work and are not proven:

- **U1.** Parity covers 7 small analog circuits; 43 of the 50 `.cir` fixtures have no MSL twin (BJT, JFET, MOS 1 to 3, noise, distortion, homotopy, temperature, URC, UIC), and two twins sit near the tolerance floor. No twin switches a transistor or has a nonlinear capacitance.
- **U2.** Digital and mixed behaviour has no oracle; it is checked against the spec's expected values and the buck's SPEC only.
- **U4.** The 18 gallery fences of Appendices D, E and F are parsed and resolved only; none is type-checked, elaborated or run.
- **U6.** Part V rules with no test that names them: record order within a microstep (§19 rule 8), the tick rounding of T2 to T4, `E0502`, the round bound `E0506`, crossings between sweep points (§16.1.2), a rejected step leaving no record (§19 rule 7).
- **U8.** AC from MSL is proven on one RC; nothing checks an AC of a design with a digital part (§16.1.4) or a DC sweep that crosses a threshold (§16.1.2).
- **U9.** The two backends agree on the M1 subset only; everything else has one implementation and no second opinion.

A claim nothing can falsify is not a property of the product (MD-38).

## Goals

- [ ] Every `.cir` fixture has an MSL twin and a parity test; MOS, BJT, JFET, a nonlinear capacitance and a switching transient are compared with ngspice.
- [ ] Digital designs are compared with Icarus Verilog and mixed designs with ngspice XSPICE, event by event (Q2).
- [ ] The 18 gallery fences run, and each observable claim of their traces is asserted.
- [ ] Every rule of Part V is named by a test or excluded with its owner (Q6).
- [ ] AC of a mixed design and a DC sweep across a threshold are proven against ngspice and the spec.
- [ ] Backend agreement has a stated meaning past the subset, and a guard holds it (Q3).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The language constructs the galleries need (generics, templates, arrays, bundles, resolved nets, switch branches, `idt`) | `generics`, `composite-values`, `resolved-nets`, `analog-structure`. This feature runs the galleries once they land |
| The `noise` primitive and the `Noise` analysis | `analog-operators` (L45, H2's noise row, U7). The noise twins wait for them |
| Temperature as a session option, `pz`, `tf` | `host-ii` (H3, H2). The temperature and `pz_rc` twins wait for them |
| The buck's other operating points (U3) | `buck-scenarios` |
| Moving the twins to a root `tests/e2e/` and the Book's integrated tests (U5, U10, H11) | `root-e2e`. New twins land where that feature puts them, if it lands first |
| The `abstol` of a user nature and compatible natures at one node (U7) | `analog-operators` |
| Retuning any tolerance | MD-38: each domain's bound is documented; a twin that fails is a finding |
| A model library for users | Q1 recommends fixtures only |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Device models | Q1 A; ngspice-faithful to the level the fixtures exercise: MOS1 (Shichman-Hodges with Meyer capacitances), MOS2, MOS3, Gummel-Poon BJT with junction capacitances, JFET (level 1, the Shichman-Hodges form ngspice gives level 1), the diode extended with series resistance, depletion and diffusion capacitance | the 43 fixtures' `.model` cards name these levels | n - planner; awaiting Q1 |
| New twins beyond the 50 fixtures | `tran_inverter` (MOS1 CMOS inverter, a pulse in), `tran_rectifier` (diode half-wave rectifier, sine in, junction capacitance), `tran_bjt_switch` (saturating BJT switch), `ac_varactor` (reverse-biased diode at three biases), `ac_mos_cs` (common-source amplifier with Meyer capacitances) | U1 asks for nonlinear capacitance and switching transients; none of the 50 has them | n - planner |
| A pulse in an MSL twin | a digital clock with `transition` edges (`transition` runs today), equal to ngspice `PULSE` with the same rise and fall | no `Pwl` source runs before `generics` (D.1) | n - planner |
| Digital comparison | the change list of each named net: one `(time in ticks, value)` per instant at which its value changes; values bit-equal, an unknown level as `x`; microsteps are not compared | Verilog has no observable microstep numbering | n - planner; awaiting Q2 |
| Mixed comparison | analog under `TRAN_VOLTAGE`; each digital edge's time within the `cross` `ttol` plus one tick; XSPICE bridge and gate delays set to 1 ps and recorded in the twin | the bridges' delays are the twin's, not the model's | n - planner; awaiting Q2 |
| What a gallery claim is | every row of a walkthrough table that states an observable: time points, jumps, values, the operating point, `Dc`, `Ac`, `Noise`, results per period, unknowns. Rows that state static structure (which params are structural) are checked by elaboration or POM tests | the galleries say "the trace is the content" | n - planner |
| Time points in a test | the solver's landing report (`multi-rate` MR-01), or this feature's own if it lands first | §17.2.2: time points are observable through `settle` only, and `settle` is refused today (L52) | n - planner |
| A gallery that cannot run yet | its test asserts the exact refusal (`P0001` naming the construct) and the gap id of its owner; a guard fails when the fence runs and the test still expects the refusal | loud and falsifiable while a dependency is open, upgraded the day it closes | n - planner |
| Near-floor twins | the harness prints each twin's worst error over its tolerance; `diode_iv` and `tran_rc` get their cause recorded | U1; `solver-hygiene` V1 and V2 are the suspected causes | n - planner |
| Rule ids of Part V | Q6 A | MD-51's shape, proven on the LRM | n - planner; awaiting Q6 |
| External tools absent | each external comparison prints the harness's skip line and still runs the Piperine side, as the ngspice harness does | `piperine-parity` rule | n - planner |

**Open questions:** Q1 to Q6 at the top of this file are open; the rows above follow the recommendations until the user answers.

**Implicit-requirement dimensions.** Input validation: CONF-08 AC 3 (a gallery's refusal must name its construct). Failure states: CONF-04 AC 2 (a failing twin is a finding, never a moved bound). Idempotency: N/A because tests are deterministic runs. Auth and rate limits: N/A, local tools. Concurrency: N/A. Data lifecycle: CONF-02 AC 4 (fixture hashes recorded, the corpus frozen). Observability: CONF-04 AC 1 (margins printed). External dependency: CONF-05 AC 3, CONF-06 AC 3 (tool absent: skip line). State transitions: CONF-10 (each Part V rule).

---

## User Stories

### P1: Every fixture has a twin ⭐ MVP

**User Story**: As the owner of the parity claim, I want every ngspice fixture twinned in MSL, transistors included, so that the analog solver's claim covers the devices a designer uses.

**Why P1**: U1; seven RC and diode circuits prove little about a transistor.

**CONF-01** (device models)

**Acceptance Criteria**:
1. The system SHALL provide MSL models of MOS levels 1, 2 and 3, the Gummel-Poon BJT and the JFET, each with every `.model` parameter the fixtures set.
2. WHEN each model is placed in a twin THEN it SHALL type-check, elaborate and run on the compiled backend with no diagnostic.
3. The models SHALL use the stdlib's `pnjlim`, `fetlim` and `limvds` for limiting (§22.2.4), and no other limiting.

**CONF-02** (the 43 twins)

**Acceptance Criteria**:
1. WHEN this feature completes THEN each of the 50 `.cir` files of `tests/ngspice/` SHALL have an MSL twin and a test function of its name in a `parity_*` target, as `parity_corpus` requires.
2. WHEN each twin runs THEN its results SHALL agree with ngspice under the `ToleranceContract` constant of its domain, unedited.
3. WHERE a twin's analysis or primitive is owned by an open feature (noise: `analog-operators`; temperature and `pz`: `host-ii`; URC by generation: `generics`) the test SHALL assert the exact refusal and name the owning gap id until that feature lands.
4. The system SHALL keep each `.cir` byte-identical to the content `parity_corpus` records, new ones recorded when added.
5. WHEN `uic_rc` and `uic_mos` run THEN the system SHALL agree with ngspice where the held start equals ngspice's UIC start, and SHALL assert its own value where the deviation is recorded in the twin (Q5).

**CONF-03** (nonlinear capacitance and switching)

**Acceptance Criteria**:
1. WHEN `tran_inverter`, `tran_rectifier` and `tran_bjt_switch` run THEN every row of ngspice's export SHALL have a Piperine row within `1e-9` of its print step, agreeing under `TRAN_VOLTAGE`.
2. WHEN `ac_varactor` runs at each of its three biases THEN its response SHALL agree with ngspice `.ac` under `AC_MAGNITUDE` and `AC_PHASE`.
3. WHEN `ac_mos_cs` runs THEN its gain and phase SHALL agree with ngspice `.ac` under the same constants.
4. WHERE Q4 A holds, WHEN `four_diode` runs THEN the host Fourier of its transient rows SHALL give each harmonic's magnitude and phase in agreement with ngspice `fourier` under `TRAN_VOLTAGE` and `AC_PHASE`.
5. WHERE Q4 A holds, WHEN `disto_diode` runs as a transient at its stated amplitude THEN the second harmonic from the host Fourier SHALL agree with ngspice `.disto` under `AC_MAGNITUDE`.

**CONF-04** (margins)

**Acceptance Criteria**:
1. WHEN a parity target runs THEN the harness SHALL print, for each twin, its worst error divided by its tolerance.
2. IF a twin fails its tolerance THEN the test SHALL fail naming the twin, the quantity and both values, and the tolerance SHALL NOT change.
3. WHEN this feature completes THEN the twins `diode_iv` and `tran_rc` SHALL have the cause of their margin recorded in their test's header.

**Independent Test**: `cargo test -p piperine-parity` with ngspice installed; every new twin a function of its own name.

---

### P1: Digital and mixed behaviour have an oracle ⭐ MVP

**User Story**: As the owner of the digital and mixed claims, I want them compared with independent simulators, so that the event semantics are proven by something other than their own author.

**Why P1**: U2.

**CONF-05** (digital against Icarus Verilog)

**Acceptance Criteria**:
1. WHEN each digital twin (the `Counter` of §12.1, the flip-flop of E.1, the pipeline of E.3, the UART of E.4, the buck's `Dpwm` with a fixed duty) runs on Piperine and its Verilog twin runs on Icarus Verilog THEN the change list of every named net SHALL be equal, time in ticks and value bit for bit.
2. IF a change list differs THEN the test SHALL fail naming the net, the first differing time and both values.
3. WHERE `iverilog` is absent the test SHALL print the harness's skip line and still run the Piperine side.

**CONF-06** (mixed against ngspice XSPICE)

**Acceptance Criteria**:
1. WHEN each mixed twin (a comparator into a flip-flop into a `D2A` loaded by an RC; the delta-sigma modulator of D.5; a counter-driven DAC into an RC) runs THEN its analog rows SHALL agree with ngspice under `TRAN_VOLTAGE`.
2. WHEN those runs complete THEN each digital edge SHALL lie within the `cross` `ttol` plus one tick of the XSPICE edge.
3. WHERE ngspice or its XSPICE code models are absent the test SHALL print the harness's skip line and still run the Piperine side.

**CONF-07** (the spec's own traces)

**Acceptance Criteria**:
1. WHEN `Front` (§0) runs with `vin` crossing the upper threshold at 10 ns as `clk` rises THEN the records of each instant SHALL follow the rows of §15.1: `q` takes the old `d`, the crossing fires at `(10 ns, 2)`, `d` becomes high at `(10 ns, 3)`.
2. WHEN `ShareCharge` (F.5) closes its switch THEN `v(a)` and `v(b)` SHALL be 0.25 V at the first step that reads the jump, within `TRAN_VOLTAGE`.
3. WHEN `ShareCharge` closes its switch at time 0 THEN the operating point SHALL fail with `E0503` naming the holds and the switch.

**Independent Test**: `cargo test -p piperine-parity --test digital_oracle --test mixed_oracle --test spec_traces`.

---

### P1: Every rule of Part V has a test ⭐ MVP

**User Story**: As the owner of Part V, I want each rule named by a test, so that a regression of any rule fails a test that says which.

**Why P1**: U6; the execution semantics are the product.

**CONF-08** (the six named rules)

**Acceptance Criteria**:
1. WHEN two reactions of different instance paths emit to one stream at the same microstep THEN the records SHALL be ordered by instance path, then by source order, and the records of the settle phase SHALL follow every microstep of their time point (§19 rule 8).
2. WHEN a commit is due at a time exactly half a tick past a tick THEN it SHALL be due at the later tick, and the rounding SHALL apply to the absolute due time `t + d` (§15 T2).
3. WHEN a commit's positive delay rounds to no advance THEN it SHALL be due at `(t, n + 1)` (§15 T3).
4. WHEN a crossing is located between two ticks THEN the reactions on it SHALL run at the later tick (§15 T4).
5. IF a scheduled time lies outside the signed 64-bit tick range THEN the run SHALL fail with `E0502` naming the `at`, the commit or the host action.
6. IF the rounds of an operating point exceed the session's bound THEN the run SHALL fail with `E0506` naming the carriers still changing.
7. WHEN a `Dc` sweep moves an input across a `cross` threshold between two points THEN the reaction on it SHALL run at microstep 0 of the later point (§16.1.2).
8. WHEN a step is rejected after a reaction in it emitted a record THEN that record SHALL not exist, and a record written by `analog` in that step SHALL stay (§19 rule 7).

**CONF-09** (the rule gate)

**Acceptance Criteria**:
1. WHEN the gate runs THEN it SHALL parse every rule id of Part V and SHALL fail for each id that no test declares with `SPEC-COVERS` and no exclusion row lists.
2. The gate SHALL accept an exclusion row only when it names the owning feature and the `GAPS.md` id that blocks the rule.
3. IF a `SPEC-COVERS` line names an id that Part V does not define, or sits in a file with no test THEN the gate SHALL fail naming it.

**Independent Test**: `cargo test -p piperine-solver --test part_v_rules` and `cargo test -p piperine-lang --test spec_rule_coverage`.

---

### P1: AC and DC of mixed designs ⭐ MVP

**User Story**: As a designer of a mixed circuit, I want its AC and its DC sweep proven, so that a gain chosen by a comparator and a hysteresis loop are right.

**Why P1**: U8.

**CONF-10** (AC with a digital part, DC across a threshold)

**Acceptance Criteria**:
1. WHEN a design whose operating point sets a comparator's output, which selects one of two feedback resistors, runs `ac` THEN its response SHALL agree with ngspice `.ac` of the analog circuit with that resistor, under `AC_MAGNITUDE` and `AC_PHASE`.
2. WHEN that design runs `ac` THEN the comparator's discrete output SHALL have no small-signal response (§11.1.5).
3. IF the operating point of a design does not go quiet THEN `ac` SHALL fail with `E0501` or `E0506` and SHALL give no row (§16.1.4).
4. WHEN a Schmitt trigger (`A2D` with hysteresis) is swept up and then down by `dc` THEN its output SHALL rise at the first point past the upper threshold and fall at the first point past the lower one.
5. WHEN `ac` runs on the diode and MOS twins of CONF-02 THEN their small-signal responses SHALL agree with ngspice `.ac` under the same constants.

**Independent Test**: `cargo test -p piperine-parity --test parity_mixed_ac` and `cargo test -p piperine-api --test dc_hysteresis`.

---

### P1: Backend agreement has a meaning ⭐ MVP

**User Story**: As the owner of the compiled backend, I want every program the interpreter can run compared on both backends, and the rest refused by name, so that the second opinion is as wide as it can be and never silent.

**Why P1**: U9.

**CONF-11** (the differential suite)

**Acceptance Criteria**:
1. WHEN the differential suite runs THEN every program of the tree that the interpreter accepts (the twins, the std analog functions in its subset, the gallery fences in its subset) SHALL give results equal bit for bit on both backends.
2. IF a program the interpreter accepts is missing from the suite THEN a guard SHALL fail naming it.
3. IF a design outside the interpreter's subset is given to it THEN the interpreter SHALL refuse it naming the construct, and SHALL give no result.
4. The system SHALL state in Appendix C what agreement means past the subset: the ngspice twins, the external oracles and the spec traces of this feature.

**Independent Test**: `cargo test -p piperine-parity --test backends`.

---

### P2: The galleries run against their traces

**User Story**: As a reader of Appendices D, E and F, I want every gallery model to run and do what its trace says, so that the galleries teach what the product does.

**Why P2**: Most fences need constructs other features bring (Dependencies); each is upgraded as they land.

**CONF-12** (18 fences)

**Acceptance Criteria**:
1. WHEN this feature completes THEN each of the 18 gallery fences SHALL type-check, elaborate and run on a bench that drives it as its walkthrough says.
2. WHEN a gallery runs THEN each walkthrough row that states an observable SHALL be asserted by a test whose name gives the section and the row (for example: D.1 lands on every corner and nowhere else; D.6 gives `vout[n] = vout[n − 1] + 0.25 · vin[n]`; E.2's `SrLatch` keeps its state through a `Dc` sweep).
3. WHILE a gallery's dependency is open its test SHALL assert the exact refusal, naming the construct and the owning gap id.
4. IF a gallery runs while its test still expects a refusal THEN a guard SHALL fail naming the gallery.
5. WHEN this feature completes THEN `GAPS.md` §9 SHALL give the galleries' share as proven by these tests.

**Independent Test**: `cargo test -p piperine-parity --test galleries`; one test per section.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md` and `ROADMAP.md`, I want the unproven list to shrink as proofs land.

**Why P3**: No behaviour depends on it.

**CONF-13** (documents)

**Acceptance Criteria**:
1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list U1, U2, U4, U6, U8 and U9, and `ROADMAP.md` SHALL mark the feature delivered with its commit.
2. WHEN this feature merges THEN Appendix C.2.3's conformance set SHALL name the tests that run it.

**Independent Test**: `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test spec_text`.

---

## Edge Cases

- WHEN ngspice prints a quantity that Piperine names differently (a branch current) THEN the twin SHALL map it in its test, never in the contract.
- WHEN a Verilog twin and a Piperine model differ by a documented semantic (Verilog's nonblocking order against MSL's commits) THEN the twin SHALL be written so that the difference cannot show, and its header SHALL say how.
- IF a gallery's trace contradicts the normative parts THEN the test SHALL assert the normative behaviour and the finding SHALL be reported to the user as a spec defect.
- WHEN a twin's ngspice run fails THEN the test SHALL fail naming ngspice's message, as the harness does.

---

## Gaps closed

| `GAPS.md` id | Requirement(s) | How |
| ------------ | -------------- | --- |
| U1 | CONF-01, CONF-02, CONF-03, CONF-04 | MSL models of MOS 1 to 3, BJT and JFET; all 50 fixtures twinned; new nonlinear-capacitance and switching twins; margins printed and the near-floor causes recorded |
| U2 | CONF-05, CONF-06, CONF-07 | Icarus Verilog for digital, ngspice XSPICE for mixed, the spec's own traces as goldens |
| U4 | CONF-12 | The 18 fences run on benches; each observable row asserted; refusals asserted until their owners land |
| U6 | CONF-08, CONF-09 | One test per named rule, and a gate over every rule id of Part V |
| U8 | CONF-10 | AC of a design with a digital part against ngspice, the quiet-point failure, a DC sweep across hysteresis, AC of diode and MOS twins |
| U9 | CONF-11 | Every interpreter-accepted program compared bit for bit, refusals by name, and agreement past the subset defined as the external oracles |

---

## Dependencies

- `generics` (L7, L30, L31, L32): D.1 `Pwl<N>`, E.1, E.2, E.3, E.6, F.2 `Divider<N>`, F.3 templates; the URC twins built by generation.
- `composite-values` (L1 to L4, L16 `normal`, L24): D.2 (`normal`'s tuple), E.4, E.5, F.3, F.4 (arrays of carriers).
- `resolved-nets` (L21, L25): E.6 (`Stream` bundle), E.7 and F.4 (`Wire`, `Tran`).
- `analog-structure` (L35, L38, L40): D.3 (`idtmod`), F.5 and D.6 where the `Switch` of §11 is placed.
- `analog-operators` (L45, H2's noise row): the 11 noise twins and the `Noise` rows of the galleries.
- `unknown-semantics` (L51, L52): F.2's `Pfd` (an event read as a `bool`) and the `settle` rows; CONF-08 AC 1's settle half.
- `host-ii` (H3 temperature, H2 `pz`, `max_steps`): `temp_bjt`, `temp_diode`, `pz_rc`.
- `multi-rate` (MR-01): the landing report the time-point rows read, unless this feature lands first.
- `root-e2e` (U10, H11): where twins live, if it lands first.

## Size

L: about 55 tasks in six phases. Device models and the analog twins (16), the digital and mixed oracles (8), Part V rules and the gate (8), mixed AC and DC (3), the differential suite (3), the galleries as their dependencies land (18, one per fence). Phases 1 to 5 can start now; phase 6 follows its dependencies.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CONF-01 | P1: every fixture has a twin | - | Pending |
| CONF-02 | P1: every fixture has a twin | - | Pending |
| CONF-03 | P1: every fixture has a twin | - | Pending |
| CONF-04 | P1: every fixture has a twin | - | Pending |
| CONF-05 | P1: digital and mixed behaviour have an oracle | - | Pending |
| CONF-06 | P1: digital and mixed behaviour have an oracle | - | Pending |
| CONF-07 | P1: digital and mixed behaviour have an oracle | - | Pending |
| CONF-08 | P1: every rule of Part V has a test | - | Pending |
| CONF-09 | P1: every rule of Part V has a test | - | Pending |
| CONF-10 | P1: AC and DC of mixed designs | - | Pending |
| CONF-11 | P1: backend agreement has a meaning | - | Pending |
| CONF-12 | P2: the galleries run against their traces | - | Pending |
| CONF-13 | P3: the documents say what is true | - | Pending |

**Coverage:** 13 total, 0 mapped to tasks, 13 unmapped (spec stage).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps… Preciso que TODOS os gaps estejam endereçados ok?".
- `GAPS.md` §8 U1, U2, U4, U6, U8, U9; §9 (galleries 0 % proven); §7 H2.
- `tests/ngspice/` (50 `.cir`, 7 `.msl` twins, `devices.msl`); `crates/piperine-lang/std/` (no device model); `crates/piperine-parity/tests/{parity_corpus,backends,twins}.rs`; `crates/piperine-lang/tests/std_galleries.rs`.
- `docs/spec/part_v_execution.md` §15 (T1 to T6, I1 to I8, S1 to S7, §15.1), §16 (O1 to O5, H1 to H7, §16.1), §17; `part_vi_io.md` §19 rules 7, 8; `appendix_c_implementation.md` C.2.3; Appendices D, E, F.
- `.specs/STATE.md` MD-38 (parity is measured), MD-51 (conformance declared at the test).
- Installed tools, 2026-11-17: ngspice-47 with `digital.cm`, `iverilog`.

---

## Success Criteria

- [ ] All 50 fixtures and the five new twins agree with ngspice under the unchanged contract, or assert a named refusal whose owner is open.
- [ ] Digital twins match Icarus Verilog event for event; mixed twins match ngspice XSPICE.
- [ ] Every rule of Part V is declared by a test or excluded with an owner.
- [ ] The 18 galleries run and their observable rows are asserted.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

# Analog Operators Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

None of these is decided. Each has a recommendation; the spec is written to the recommendation and marks where an answer changes it.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | What does a noise run print? | (a) ngspice's quantities: one row per frequency with `freq`, `onoise` (the output density, V/√Hz for a potential output, A/√Hz for a flow output) and `inoise` (input-referred, only when `input` is given), then the band totals `onoise_total` and `inoise_total` in V or A; (b) power densities (V²/Hz) and no totals | **(a)**: the parity oracle is ngspice's `onoise_spectrum` and `onoise_total` (`tests/ngspice/noise_rc.cir` says ngspice reports V/√Hz), so (a) compares without a conversion. AO-05 is written to (a) |
| Q2 | Is the per-source breakdown (§11.1.8: "the name only groups sources in reports") on the command line? | (a) the library result carries each group's density (a group is a source's `name`, or its instance path when unnamed); the command line prints the totals only; (b) the command line also prints one column per group, chosen by `print` | **(a)**: the breakdown is for a host test or a plot; a column per group makes a wide table nobody asked for. (b) adds one task |
| Q3 | What are the run keys of a noise run? | (a) `analysis = "noise"`, `output` (a node path, or two for a differential output) and `input` (the instance path of a source, optional), with the AC sweep's `from`, `to`, `per_decade` or `points`: the names `host-ii` chose for `tf` and `pz`; (b) ngspice's spellings (`src`, `out`) | **(a)**: one vocabulary for every small-signal analysis (`host-ii` Q3) |
| Q4 | Which names does `simparam` read (G.66 leaves them to the host)? | (a) every session option of `host-ii` by its key name (`reltol`, `abstol`, `vntol`, `trtol`, `temp`), plus `tnom`, plus any name the run gives in a new key `simparam = { name = value }` (flag `--simparam name=value`, repeatable); (b) only the names the run gives; (c) a fixed list only | **(a)**: a model can read the session it runs in, and a project can pass its own constants without a param. `temp` and `tnom` read in °C, as their keys are written (VAMS `$simparam("tnom")` is in °C). AO-07 and AO-08 are written to (a) |
| Q5 | Are trace records (`emit!` in `analog`) recorded by every run? | (a) only when the run names the stream in `streams` (MD-66: observation is the run's); (b) always, like a reaction's records | **(a)**: a trace is written at every Newton iteration, rejected steps included (§19 rules 3, 7); a transient of a design with one trace site would hold millions of records unasked |
| Q6 | `tnom`'s default and unit | (a) 27 °C, as ngspice; (b) the session `temp` | **(a)**: the key is ngspice's, so its default and unit are too |

## Problem Statement

Five core primitives of Part IV §14 are `P0001`: `ddx` (L43), `delay` (L44), `noise` (L45) and so `white_noise`, `flicker_noise` and `noise_table`, `simparam` (L47), and `emit!` in `analog` (L49). The solver already has the analyses and the ABI vocabulary for them (`op::DELAY`, `input::NOISE`, `global::NAMED`, `watch::TRACE`, `analyses/noise.rs`), but nothing in the language reaches them, so no host can run a noise analysis (H2: `host-ii` left it out because every density would be zero) and `tnom` waits for `simparam` (`host-ii`). The convergence tolerance of a user nature and compatible natures at one node are never exercised from source (U7).

## Goals

- [ ] `ddx`, `delay`, `noise`, `simparam` and `emit!` in `analog` run on the Rust backend as §14 and §16.2 define them.
- [ ] `white_noise` and `flicker_noise` run; `noise_table` runs once its dependencies merge.
- [ ] `analysis = "noise"` runs from a run table and from the command line, and agrees with ngspice `.noise` on `noise_rc`, `noise_switch` and a new `noise_diode` twin.
- [ ] `tnom` is a session option, read by `simparam`, proven against ngspice on a new `tnom_res` twin.
- [ ] A user nature's `abstol` and compatible natures at one node are exercised from `.msl` source.
- [ ] The 7 existing twins print the bytes they print today.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| `freq()` and `analysis(kind)` | L47 (`freq`) and L48, owned by `msl-fixes` (FIX-10, FIX-11). This feature depends on them |
| `bound_step` | L46, `msl-fixes`; under an `if`, `analog-structure` |
| `ddt` off a whole addend; a stateful core result anywhere in an expression | L42, `analog-structure` (AS-07). `delay` reuses that lowering |
| Multiplicity `m` scaling a noise power (§14.8.4, §8.5.4) | L29, owned by `generics`. Noise here is at multiplicity 1 |
| `noise_table` as the stdlib writes it (`const N`, an array param, `table_model`) | needs `generics` (L30), `composite-values` (L3) and `params-and-io` (L57); AO-04 AC 3 is a `WHERE` criterion |
| Operating-point variables read by the host (`let gm = ddx(..)` read as `Top.d.gm`) | `host-iii` |
| Periodic noise and periodic small-signal (§16.3) | V16, `periodic-small-signal` |
| Transfer function, pole-zero, sensitivity, PSS, S-parameters | `host-ii` and `host-iii` (H2's other rows) |
| The session options `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `nodeset` | `host-ii` (HII-01). `simparam` reads them (Q4) |
| A trace record's `maybe` field under unknown control | L19, `unknown-semantics` |
| Noise models of devices not written in MSL (MOS, JFET, BJT twins of `tests/ngspice/noise_*.cir`) | U1, owned by `conformance` |
| The interpreter running any form this feature adds | R1; it refuses by name (AO-11) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Backends | every form lands on the Rust backend; the interpreter refuses each by name | R1, R2 | y - R1/R2 |
| Oracle | ngspice where it has the quantity (`.noise`, a matched lossless line `T` for `delay`, `tnom` on a resistor's `tc1`); the spec's closed forms elsewhere (`ddx`, `simparam`, traces) | R2, MD-38 | y - R2 |
| A noise tolerance | one new `ToleranceContract` constant for output noise density, and one for the band total if the density's does not cover it, each with its rationale in `contract.rs`; no existing constant moves | MD-38: each domain has its own documented bound; `contract.rs` has none for noise | n - planner |
| Temperature of the noise twins | the run gives `temp = 27`, ngspice's default, through `host-ii`'s key | thermal noise is `4kT/R`; both sides must use one `T` | n - planner |
| The `name` of a noise source | a string literal kept as metadata of the site, never a run-time value | §14.8.1 says it is a literal that only groups sources; MD-64 item 3 keeps a record's text in the IR the same way | n - planner |
| How `delay` reaches the ABI | design decides (the ABI's `DELAY` term op, through AS-07's lowering when the result is not a whole addend) | ABI `Term` ops are whole addends of a row | n - planner |
| `delay` time points | not asserted: §14.6.5 says `delay` adds none, and a test of an absent breakpoint would be tuned to the step controller | MD-38 | n - planner |
| `inoise` | the output density divided by the magnitude of the transfer from `input` to `output` at each frequency, as ngspice | ngspice's definition; the twin compares it | n - planner |
| Q1 to Q6 | the recommendation of each | see the top of this file | n - open |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: AO-01 AC 7 and 8 (`E0451`, `E0450`), AO-02 AC 5 and 6 (`E0452`, `E0453`), AO-03 AC 6 (`E0454`), AO-05 AC 4 to 6 (`P0010`, `E0801`), AO-07 AC 3 (`E0456`), AO-08 AC 2, AO-09 AC 5 (`E0603`). Failure states: AO-03 AC 6 and AO-07 AC 3 fail the analysis before or at the point named. Idempotency and retry: N/A because a session runs once. Auth and rate limits: N/A because the tool is local. Concurrency and ordering: AO-09 AC 2 (trace records of rejected steps stay, in evaluation order). Data lifecycle: AO-09 AC 6 (traces recorded only when subscribed, Q5). Observability: AO-09 (traces are the debugging surface of non-convergence). External-dependency failure: AO-06 AC 4 (ngspice absent, the harness's skip rule). State-transition integrity: AO-02 AC 2 (the history before `td` is the operating point's value).

---

## User Stories

### P1: `ddx` ⭐ MVP

**User Story**: As a model author, I want `ddx(e, probe)`, so that a model computes a small-signal conductance or capacitance from its own equations.

**Why P1**: The §18 `Nmos1` and every compact model that reports `gm` use it.

**Acceptance Criteria** (AO-01, L43):

1. WHEN `let id = is * ((v(a, k) / nvt).exp() - 1.0);` and `v(m) +> ddx(id, v(a, k));` are evaluated with `v(a, k)` = 0.6 V THEN the system SHALL give `v(m)` = `is / nvt · exp(0.6 / nvt)` within 1e-9 relative.
2. WHEN the second argument is the reversed branch (`ddx(v(k, a), v(a, k))`) THEN the system SHALL give −1.
3. WHEN `e` reads a probe of another branch THEN the system SHALL hold it constant: `ddx(v(g) - v(s), v(g, s))` is 0 (§14.2.3).
4. WHEN the second argument is a flow probe (`ddx(x * i(a, b), i(a, b))`) THEN the system SHALL give `x`.
5. WHEN `e` reads `limit(v(a, k), f)` THEN the system SHALL differentiate through `limit` as through its argument (§18 rule 3).
6. WHEN `e` reads the result of another instance THEN the system SHALL hold that result constant.
7. IF the second argument is not a potential or flow probe of a branch of the module (`p.inflow()`, any other expression) THEN the system SHALL report `E0451`.
8. IF `ddx` is written outside `analog`, or in a body `let` or value input that a definition or a reaction reads THEN the system SHALL report `E0450`.
9. WHEN an AC sweep linearizes a contribution `i(o, gnd) +> ddx(id, v(a, k)) * v(in);` THEN the system SHALL use the value of `ddx` at the operating point: the gain `|v(o)/v(in)|` equals that value times the load of `o`, within `AC_MAGNITUDE`.

**Independent Test**: transpile tests against the closed forms; elaboration tests for `E0450` and `E0451`.

---

### P1: `delay` ⭐ MVP

**User Story**: As a model author, I want `delay(x, td, max)`, so that I can write transport delay, a transmission line or a sampled transfer under §11.1.6.

**Why P1**: L44; `zi_*` of the stdlib is written on it.

**Acceptance Criteria** (AO-02, L44):

1. WHEN `v(out) +> 0.5 * delay(v(src), 1u);` runs a transient THEN the system SHALL agree with a new twin `delay_tline` (`tests/ngspice/delay_tline.cir`: the same source into a lossless line `T` with `Z0` = 50 Ω, `TD` = 1 µs, matched at both ends) under `TRAN_VOLTAGE` at every print row.
2. WHEN the time is less than `td` THEN the system SHALL give the value `x` had at the operating point.
3. WHEN an operating point evaluates `delay(x, td)` THEN the system SHALL give `x`, and WHEN `td` is 0 in any solve THEN `x`.
4. WHEN `delay(x, td, max)` is given a `td` that varies during the analysis THEN the system SHALL read `x` as it was `td` ago, and as it was `max` ago while `td` exceeds `max`.
5. IF `delay` has no `max` and its `td` is not constant during an analysis after value inputs are substituted THEN the system SHALL report `E0452` at elaboration naming the instance.
6. IF `td` is negative, or `max` is not positive, during a run THEN the system SHALL fail the run with `E0453` naming the instance.
7. WHEN `delay_tline` is swept in AC THEN the system SHALL give `0.5 · exp(−jω · 1 µs)` and agree with ngspice under `AC_MAGNITUDE` and `AC_PHASE`.

**Independent Test**: `cargo test -p piperine-parity --test parity_tran --test parity_ac` for `delay_tline`; elaboration tests for `E0452`.

---

### P1: Noise sources ⭐ MVP

**User Story**: As a model author, I want `noise(psd, name)`, `white_noise` and `flicker_noise`, so that my models carry thermal, shot and flicker noise.

**Why P1**: L45; without it a noise analysis has nothing to report.

**Acceptance Criteria** (AO-03, the core module, L45):

1. WHEN an operating point, a transient step or an AC sweep evaluates a `noise` instance THEN the system SHALL give 0 and SHALL NOT read its `psd`: `i(a, gnd) +> noise(-1.0);` runs `op`, `transient` and `ac` with the same values as without it.
2. WHEN one `noise` instance is read by two contributions (`i(a, mid) +> n; i(mid, b) +> -n;`) THEN the system SHALL treat both as one fully correlated source (§11.1.8).
3. WHEN two `noise` instances feed one output THEN the system SHALL add their output power densities.
4. WHEN `psd` reads `freq()` THEN the system SHALL evaluate it at each sweep frequency, and never at `freq()` = 0.
5. WHEN a noise result passes through `ddt` (`i(a, gnd) +> ddt(c * n);`) THEN the system SHALL give it the response `jω·c` (§11.1.9), and through `delay` the response `exp(−jω·td)`.
6. IF `psd` is negative or not finite at a frequency of a noise sweep THEN the system SHALL fail the analysis with `E0454` naming the instance and the frequency.
7. WHEN a reaction or an event argument reads a `noise` result THEN the system SHALL give 0.
8. WHEN two sources share a `name` THEN the system SHALL keep them independent: the name groups, it does not correlate.

**Acceptance Criteria** (AO-04, the stdlib noise modules):

1. WHEN `white_noise(psd, name)` is read in a contribution THEN the system SHALL give a source of density `psd`.
2. WHEN `flicker_noise(k, exp, name)` is read in a contribution THEN the system SHALL give a source of density `k / f^exp` at each frequency `f`.
3. WHERE `generics` (L30), `composite-values` (L3) and `params-and-io` (L57) have merged, WHEN `noise_table(name) { table: [(1k, 1e-18), (10k, 4e-18)] }` is read THEN the system SHALL give a density piecewise linear in frequency through the pairs, held at the end values outside them.

**Independent Test**: transpile tests with the solver's noise analysis on closed forms (a lone resistor's `4kT/R`, a capacitor-shaped source).

---

### P1: A noise analysis from the host ⭐ MVP

**User Story**: As a designer, I want `analysis = "noise"` in a run table and `--analysis noise` on the command line, so that I get the output and input-referred noise of a design as ngspice's `.noise` does.

**Why P1**: H2's noise row; the solver has the analysis and no host reaches it.

**Acceptance Criteria** (AO-05, the run, Q1 to Q3):

1. WHEN a run gives `analysis = "noise"`, `output`, `from`, `to` and `per_decade` (or `points`) THEN the system SHALL print one row per sweep frequency with the columns `freq` and `onoise`, then the line of `onoise_total` over the band.
2. WHEN the run also gives `input` THEN the system SHALL add the column `inoise` and the line of `inoise_total`.
3. The system SHALL give `output` and `input` the flags `--output` and `--input`, a flag winning over the key, as every run key (MD-65).
4. IF a noise run gives no `output` or no sweep THEN the system SHALL report `P0010` naming the missing key.
5. IF `output` names no node, or `input` names no instance that writes a potential or flow contribution with an `ac_stim`, THEN the system SHALL report `E0801` naming the path before the analysis runs.
6. IF a noise run gives a key of another analysis (`stop`, `sweep`) THEN the system SHALL report `P0010` as for the other analyses.
7. WHEN `analysis(Noise)` is evaluated in a noise run's operating point and sweep THEN the system SHALL give true, and `ac_stim` SHALL be 0 in the sweep.
8. WHEN a host calls `piperine-api` for a noise analysis THEN the system SHALL return the rows, the totals and each group's density (Q2).

**Acceptance Criteria** (AO-06, against ngspice `.noise`):

1. WHEN the twin `noise_rc.msl` (the `noise_rc.cir` circuit, its resistor carrying `white_noise(4.0 * K_B * temperature() / r, "thermal")`) runs THEN `onoise` SHALL agree with ngspice's `onoise_spectrum` at every frequency, and `onoise_total` with ngspice's, under the new noise contract.
2. WHEN the twin `noise_switch.msl` (a resistive switch whose thermal source follows its on resistance) runs THEN `onoise` SHALL agree with `noise_switch.cir` under the same contract.
3. WHEN a new twin `noise_diode` (a diode biased by a current source, with shot noise `2qI` and flicker noise `kf·I^af/f`, against ngspice's `D` with `kf`, `af`) runs THEN `onoise` SHALL agree under the same contract.
4. WHERE the ngspice comparison is disabled the three twins SHALL still run and print the harness's skip line.

**Independent Test**: `cargo test -p piperine-parity --test parity_noise` (a new `parity_*` target, so `parity_corpus` sees it), and `cargo test -p piperine-cli` for the keys and flags.

---

### P1: Session parameters ⭐ MVP

**User Story**: As a model author, I want `simparam(name, default)` and a `tnom` session option, so that a model reads its nominal temperature and other session values as Verilog-A models do.

**Why P1**: L47's other half; `host-ii` left `tnom` out until `simparam` lands.

**Acceptance Criteria** (AO-07, `simparam`, L47, Q4):

1. WHEN `simparam("vth0", Some(0.5))` is evaluated and the run gives `simparam = { vth0 = 0.7 }` THEN the system SHALL give 0.7, and 0.5 when the run gives none.
2. WHEN `simparam("reltol", None)` is evaluated THEN the system SHALL give the run's `reltol`, or the default the solver uses when the run gives none.
3. IF `simparam(name)` has no default and the session has no parameter `name` THEN the system SHALL fail at the start of the analysis with `E0456` naming `name`.
4. The system SHALL accept `simparam` in every run-time position, definitions and selectors included, and SHALL give it one value for the whole analysis (§14.4.1).
5. The system SHALL give the key `simparam` the repeatable flag `--simparam <name>=<value>`; IF a value is not a number THEN the system SHALL report `P0010` naming the entry.

**Acceptance Criteria** (AO-08, `tnom`, Q4, Q6):

1. The system SHALL accept the key `tnom` in a run table and the flag `--tnom`, in °C, 27 when absent.
2. IF `tnom` is not a number greater than −273.15 THEN the system SHALL report `P0010` naming the key.
3. WHEN `simparam("tnom", None)` is evaluated THEN the system SHALL give the run's `tnom` in °C.
4. WHEN a new twin `tnom_res` (a divider whose resistor has `tc1`, written `r * (1.0 + tc1 * (temperature() - 273.15 - simparam("tnom", None)))`, against ngspice with `.options tnom=50 temp=27`) runs with `tnom = 50` and `temp = 27` THEN the system SHALL agree with ngspice under `DC_VOLTAGE`.

**Independent Test**: transpile tests; `cargo test -p piperine-parity --test parity_op` for `tnom_res`.

---

### P1: Trace records from `analog` ⭐ MVP

**User Story**: As a model author debugging convergence, I want `emit!` in `analog` to write trace records with the Newton iteration, so that I can see the path Newton took.

**Why P1**: L49; §19 rule 3 names this as how non-convergence is debugged.

**Acceptance Criteria** (AO-09, L49):

1. WHEN an `analog` block executes `emit!("conv", v = v(a));` THEN the system SHALL write one record per evaluation in every equilibrium solve, transient step and jump solve, each carrying the instance path, the analysis, the instant `(t, n)` and the Newton iteration.
2. WHEN a step is rejected THEN the system SHALL keep the trace records written during it (§19 rule 7).
3. WHEN an AC or noise sweep runs THEN the system SHALL write no trace record.
4. WHEN the `emit!` sits under an `if` THEN the system SHALL write a record only at evaluations that execute it.
5. IF a site reached from `analog`, directly or through a `fn`, writes the stream `sim` or `checks` (`require(..)` in `analog` included) THEN the system SHALL report `E0603` naming the site.
6. WHEN a run does not name a trace stream in `streams` THEN the system SHALL record none of its records (Q5); WHEN it names one THEN the system SHALL deliver every record of it.
7. The system SHALL NOT let a trace record change any carrier, solve or event (§19 rule 9).

**Independent Test**: a transpile test over a diode whose Newton path is known, counting records per iteration; elaboration tests for `E0603`.

---

### P2: Tolerances of user natures, from source

**User Story**: As the owner of the convergence rule, I want a user nature's `abstol` and compatible natures at one node proven from `.msl` source, so that §17.1 is not only a solver unit test.

**Why P2**: U7; the behaviour exists and nothing exercises it from the language.

**Acceptance Criteria** (AO-10, U7):

1. WHEN a node is of a nature whose potential `abstol` is 1m THEN the bound circuit SHALL give that node's unknown the update and residual tolerance 1m (§17.1 rules 1, 2).
2. WHEN a branch is of a nature whose flow `abstol` is 1n THEN the bound circuit SHALL give its flow unknown and its row the tolerance 1n.
3. WHEN a port of `Electrical` and a port of a compatible nature `HvElectrical` (same potential and flow names, potential `abstol` 1m) meet at one node THEN the system SHALL give that node the smaller potential `abstol` (1 µV), and the operating point of a divider made of one resistor of each nature SHALL equal the all-`Electrical` divider within `DC_VOLTAGE`.
4. IF two natures whose flow names differ meet at a port THEN the system SHALL report `E0251`.
5. WHEN a branch's nature gives a flow `abstol` and the session gives `abstol` THEN the bound circuit SHALL give the branch's flow unknown and its row the smaller of the two (a nature can only tighten the session value; AC 2 reads the nature's value where it is the smaller), decided 2026-11-22.

**Independent Test**: a `piperine-api` test that binds the designs and reads the bound circuit's tolerances, plus the divider's operating point.

---

### P2: The interpreter refuses, parity holds, the documents follow

**User Story**: As the owner of the parity claim, I want every new form refused by name in the interpreter and every existing comparison untouched.

**Why P2**: R1.

**Acceptance Criteria** (AO-11):

1. IF a design that reaches the interpreter holds `ddx`, `delay`, `noise`, `simparam` or `emit!` in `analog` THEN the interpreter SHALL refuse it naming the form.

**Acceptance Criteria** (AO-12):

1. WHEN each of the 7 twins runs with the flags of `crates/piperine-parity/tests/twins.rs` THEN the system SHALL print, on each backend, the bytes it printed before this feature.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran` and `parity_corpus` with the existing constants of `contract.rs` unchanged.
3. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L43, L44, L45 (except `noise_table` while AO-04 AC 3 waits), L49, `simparam` in L47, the noise row of H2 and U7, and the Book SHALL show one noise run, held to the binary.

**Independent Test**: `cargo test -p piperine-parity`, `cargo test -p piperine-cli --test book --test docs`.

---

### P2: `slew` limits an ideal step

**Acceptance Criteria** (AO-13, from analog-structure, user 2026-11-19):

1. WHEN the stdlib `slew(v(in), 1M, 1M)` follows a 0 to 1 V ideal step (a carrier change at 1 µs, no edge) THEN the system SHALL give 0 V before 1 µs, a ramp of 1 V/µs and 1 V from 2 µs, within `TRAN_VOLTAGE`. Today §15.2 rule 4 fires a jump's crossing after the solve of the step that reads the jump, so the output passes the jump in one step; the fix is to retake the step when a jump-crossing flips a carrier that `analog` reads (a change to §15.2 rule 4 and the engine).

---

## Edge Cases

- WHEN a noise run's `output` is a flow (`i(Top.r1)`) THEN the system SHALL print `onoise` in A/√Hz.
- WHEN a design has no noise source THEN a noise run SHALL print `onoise` = 0 at every frequency and succeed.
- WHEN `delay`'s `td` is exactly `max` THEN the system SHALL read `x` as it was `max` ago.
- WHEN `ddx`'s `e` does not read the probe at all THEN the system SHALL give 0.
- WHEN `simparam` is called with a name the run gives and a default THEN the system SHALL give the run's value.
- WHEN a trace site's fields differ in type from another site of the same stream THEN the system SHALL report the error of §19 rule 12, as for reaction records.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| AO-01 | P1: `ddx` | - | Pending |
| AO-02 | P1: `delay` | - | Pending |
| AO-03 | P1: noise sources | - | Pending |
| AO-04 | P1: noise sources | - | Pending |
| AO-05 | P1: a noise analysis from the host | - | Pending |
| AO-06 | P1: a noise analysis from the host | - | Pending |
| AO-07 | P1: session parameters | - | Pending |
| AO-08 | P1: session parameters | - | Pending |
| AO-09 | P1: trace records | - | Pending |
| AO-10 | P2: tolerances of user natures | - | Pending |
| AO-11 | P2: interpreter, parity, documents | - | Pending |
| AO-12 | P2: interpreter, parity, documents | - | Pending |

**Coverage:** 12 total, 0 mapped to tasks, 12 unmapped (no tasks yet).

## Gaps closed

| `GAPS.md` id | What | Closed by |
| ------------ | ---- | --------- |
| L43 | `ddx` | AO-01 |
| L44 | `delay` | AO-02 |
| L45 | `noise`, `white_noise`, `flicker_noise`, `noise_table` | AO-03 (core), AO-04 AC 1, 2 (`white_noise`, `flicker_noise`), AO-04 AC 3 (`noise_table`, once `generics`, `composite-values` and `params-and-io` merge) |
| L47 (`simparam`; `tnom`, deferred by `host-ii`) | `simparam` and the `tnom` session option | AO-07, AO-08. `freq()` stays with `msl-fixes` |
| L49 | trace records: `emit!` in `analog` | AO-09 |
| H2 (noise analysis) | `analysis = "noise"` through `piperine-api` and the command line, proven against ngspice `.noise` | AO-05, AO-06 |
| U7 | a user nature's `abstol` in the convergence test; compatible natures at one node | AO-10 |

## Dependencies

| Feature | What this feature needs from it |
| ------- | ------------------------------- |
| `msl-fixes` | `freq()` in a sweep (FIX-10) for `flicker_noise`; `analysis(Noise)` true in the noise operating point (FIX-11 AC 4) |
| `host-ii` | the session option keys (HII-01: `temp` for the noise twins, `reltol` and the others for `simparam`) and the small-signal run-key names (`input`, `output`) |
| `analog-structure` | the lowering of a stateful core result anywhere in an expression (AS-07) for `0.5 * delay(..)`; a resistive switch for `noise_switch` needs only `if` expressions, which run today |
| `generics`, `composite-values`, `params-and-io` | `noise_table` (AO-04 AC 3) |
| `solver-hygiene` | none |

## Size

**L.** About 25 tasks in five phases: `ddx` and `simparam` (front end, plan and transpile; AO-01, AO-07); `delay` (AO-02, after AS-07's lowering); `noise` and the stdlib modules (AO-03, AO-04); the noise run and its twins, `tnom` (AO-05, AO-06, AO-08); traces, U7, refusals and documents (AO-09 to AO-12).

## Sources

- `GAPS.md` (audit of 2026-11-17): L43 to L45, L47, L49, H2, U7; §10 cluster "Analog operators".
- `docs/spec/part_iv_behavior.md` §11.1.8, §11.1.9, §14 (rules 1 to 9), §14.2, §14.4, §14.6, §14.8, §14.11; `part_v_execution.md` §16.2, §17.1; `part_vi_io.md` §19 rules 1 to 15; `part_ii_carriers.md` §4 rules 3, 5, 13, §6.1.3; `part_vii_stdlib.md` §22.2.3; Appendix G.66.
- `crates/piperine-abi/src/raw.rs` (`op::DELAY`, `input::NOISE`, `global::NAMED`, `watch::TRACE`); `crates/piperine-solver/src/analyses/noise.rs`.
- `feature/host-ii` `spec.md`: noise and `tnom` out of scope there ("It lands with L47"), Q3 key names, X7 option names.
- `feature/msl-fixes` `spec.md`: FIX-10 (`freq`), FIX-11 (`analysis` kinds).
- `tests/ngspice/noise_rc.cir`, `noise_switch.cir`; `.specs/STATE.md` MD-38, MD-64, MD-65, MD-66; `.specs/features/codegen/context.md` R1, R2.

---

## Success Criteria

- [ ] `piperine run --analysis noise` prints `onoise` for `noise_rc`, `noise_switch` and `noise_diode` that agrees with ngspice under one documented contract.
- [ ] `delay_tline` and `tnom_res` agree with ngspice; `ddx`, `simparam` and traces match their closed forms.
- [ ] The 7 existing twins print the bytes they printed before.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

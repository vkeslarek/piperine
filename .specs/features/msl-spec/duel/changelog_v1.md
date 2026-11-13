# Changelog v0 → v1 (builder, round 1)

Answers `critique_v0.md`. Every BLOCKER and MAJOR is fixed; no item is rebutted outright, three are fixed differently from the critic's proposal (marked †, with the reason).

## Items

| Item | Action | What changed | Where in `spec_v1.md` |
| --- | --- | --- | --- |
| **B1** multi-driver, strengths, tristate, `inout` | fixed, merged | A nature says how its drivers combine. `nature N { value: T, resolve: f }`; `Bit` is now the stdlib nature with no `resolve`. Each module holds one *drive*; the carrier is `resolve` of the drives. Z and strength are a stdlib value type (`Logic`, `Strength`). A resolved carrier has no direction (it is `inout`). `s.others()` (resolve of the other modules' drives) makes the pass gate (`Tran`) loop-free and expressible. Undriven / contended / kept are distinguishable by strength. Deleted: `TriBus`, `OpenDrainBus`, `WiredOr`, `SumNet`/`MaxNet`/`MinNet`/`AvgNet` modules, `Pin`, the "one enumerated nature" rule. Closes G.25. The critic's own sketch is F.4. Honest residue: a *ring* of pass gates is a combinational loop (new G.27). | 4, 5.2, 8.2, 22.3, E.7, F.4 |
| **B2** re-solve at `t` undefined | fixed | One rule: the right value is the limit of a backward-Euler step of length `h → 0` from the left value; `ddt(q)` reads `(q − q⁻)/h`. Six consequences listed (states = `ddt` arguments; forced jumps conserve the sum over the cut, impulsive flow unobservable; `ddt(c·v)` with `c` jumping conserves charge; second pass gives finite flows; other instance state untouched; what "restart" keeps). `ShareCharge` worked to 0.25 V. | 15.2, F.5, C.2 |
| **B3** unknown-ness of values | fixed | Unknown is a flag on every level of every value (not only carriers). Two operator classes (per-level; all-or-nothing). **One** control-flow rule: unknown control takes every path not excluded and merges every effect (yielded value, locals, commits) level by level. The yields/commits split and the `_`-arm row are gone; indices, guards, loops, closures, library code are covered by the same rule. `analog` has no unknowns: carriers are read there only through `level`/`known`/`known_or`, checked at elaboration. | 3.3, 22.1, E.1, E.4, E.5 |
| **M1** `idt`/`idtmod` wrong | fixed | `idt<S: Nature = State>`: holds `ic` in `Static` when given (VAMS 4.5.4), `x = 0` row with free output otherwise (feedback only, else a named failure); state lives on a node of nature `S`, so tolerances follow what is integrated (`idt::<Heat>` in `Water`). `idtmod` re-seats the integrator at each wrap through `idt`'s `ic`/`reset` (three microsteps, resting on B2), so the state never grows. `Nullor` gets a `Servo` nature so its row is tested in volts. Needs generic probes `potential`/`flow` (+2 names, the `Nature` bound). | 8.3, 11, 22.2 |
| **M2** collision rule; transport overtaking | fixed | Collision *error* deleted. Last write wins, ordered by instant made, then source order (all writers of a drive are in one module, one file). `collision` is a lint. The `Pfd` coincidence is shown not to be a collision at all (delayed commit applies at `(t,0)`, the edge's commit at `(t,1)`). Transport: a commit made for time `T` removes that level's pending commits for times `≥ T` (VHDL rule); D.6 `TwoPhase` now correct as written. Applies to `=`…`after` and `<-`…`after` alike. | 12.1, D.6, F.2 |
| **M3** time-0 consistency † | fixed | "Initial values are not changes": nothing fires at start except `init`; level-sensitive logic includes `init` (`DLatch`, `Keeper`, `Inertial`, `transition`). † Not the critic's first option (`changed()` at `(0,0)` for every known carrier): that also fires `rise()` on a clock that starts high and every monitor at time 0, which then need exceptions. Initial value of a carrier is given once, by its driver. | 13 rule 6, 5.2, E.2 |
| **M4** non-transient analyses | fixed | New 16.1: each sweep point is an operating point + `settle`; discrete state carries between sweep points (`init` once); `at` never fires, `after` never comes due; small-signal linearizes at the fixed point, fails if it does not go quiet. `cross` is defined as "between two successive solutions" (time points, sides of a jump, OP rounds, sweep points), which is what makes hysteresis work in a DC sweep. | 16.1, 13 rule 5 |
| **M5** three kinds of callable | fixed, merged | Two kinds: `fn` (pure, places no instance) and module (instance). `ddt`, `delay`, `slew`, `limit`, `noise`, `cross`, `at` are core modules with a result; "state per call site" sentence deleted. `limexp`, `white_noise`, `flicker_noise`, `above` are modules with a result; `reaches` removed (`above(temp(t) - k)` at the use). B.2 rows 7–9 → 2 rows. | 0, 3, 8.3 rule 6, 14, B.2 |
| **M6** call-by-name value ports | fixed | Stated as the rule of concept 6: a value input is the caller's expression, read in place; the result is the module's expression, read in place; a body `let` is the same thing inside one module ("`let` names an expression"). This one rule also replaces `impl` on modules and named events in `digital` blocks. `table_model` is a `fn`. | 8.2, 8.3 rules 1–2 |
| **M7** small-signal of sampled state | fixed | Rule in 11.1: small-signal linearizes `analog` with carriers constant; a sampled block states its transfer under `!analysis(Tran)` with `delay` (`exp(−jωT)`, identity in DC). `zi_nd` written out in full that way: `H(e^{jωT})` in `Ac`/`Noise`, `H(1)` in `Dc`. No new core. General sampled loops → G.30 (`Pss`). | 11.1, 22.2 |
| **M8** switch-branch selector | fixed | A branch's kind is constant during a solve: selector over params, carriers, `analysis` only; probe-dependent is an elaboration error; ideal diode = crossing + `Bit`. Param-only selector collapses the node at elaboration; otherwise the branch keeps its flow unknown. | 11 rule 3, 18 (`Diode`) |
| **M9** 17.2 rule 2 vs table | fixed | Rule 2 rewritten: the solver lands on scheduled instants whose reactions can reach the analog side (static dependency); other digital instants run in order between time points, never ahead of the last accepted one. Table and `settle` row aligned; `settle` runs only at time points. | 17.2 |
| **M10** `above`, `absdelta`, `transition` core | fixed (deleted from core) | All three are stdlib, written out. `transition` takes `in x: Bit<f64>` (an expression argument is an anonymous definition) and is built from `after`, `at` and four signals. Also deleted beyond the critic's list: `discontinuity()` (a discontinuity is a carrier change `analog` reads; `SweptResistor` rewritten), `Dir` (`cross` is rising; falling is `cross(-e)`), `finish()` (`finish!()` = `emit!` on the reserved stream `sim`). | 13, 14, 22.2, 9 |
| **M11** views | deleted | 5.5 and rule 6 gone; stdlib `cast`. | 8.2 rule 6, 22.1 |
| m1 attributes | fixed | Deleted: `#[opvar]` (every named `analog` `let` is one), `#[nondiff]` (inferred), `#[mc]` and `#![time_resolution]` (host), `#[units]`, `#[alias]`, `#[repr(onehot\|gray)]` (hand-written `impl Repr`), `#[osdi]`/`#[device]` (→ `extern "osdi"`/`"cosim"` + Rust's `#[link]`). `in range` on params deleted. 13 → 5. | 8.1, 8.5, B.1 |
| m2 `interface` | merged | A `struct` of carriers is a bundle; keyword and `module Name: Interface` head deleted; bound is `C: Fn(Cell)`. | 7, 8.4, F.3 |
| m3 parent contributes into child branch | deleted | Read/probe/attach own branch only. `Coupled` module shown; `Magnetic` port for N-way coupling. | 8.2, 11 |
| m4 grammar | fixed | `flip` is part of `sigtype`; nature fields; `after dt <= 0` is the next microstep in both forms. | B.3, 12.1 |
| m5 tick rounding | fixed | Crossings round *up*, solution evaluated at the tick; `since()` is seconds, an exact multiple of the tick. | 15, 5.3 |
| m6 `set` of a generation param | fixed | Structural params: `set` refused during a run, host re-elaborates. | 8.1 |
| m7 unknown read by `analog` | fixed | Elaboration rule (3.3 rule 5); every gallery model updated (`known_or`). Cost noted in G.29. | 3.3, galleries |
| m8 cosim / OSDI contract | fixed | Contract table: OSDI exposure; cosim is discrete-only, stepped at accepted instants, never rolled back. | 8.5 |
| m9 history on memories | fixed | Storage and history materialize at first commit. | 5.1 |

Also changed, unprompted, to keep one rule each: `held()` removed (`MinPulse` monitor instead of `glitch`); `at` fires once per value of its arguments (v0's "re-armed after it fires" looped); G.7 closed by a stdlib `Inertial`; `Bidir` written out on a `Wire`.

## Counts

**Concepts** (table of section 0):

| | claimed | honest |
| --- | --- | --- |
| v0 | 10 | 19 (critic's count) |
| v1 | 10 | 10: every item the critic counted separately is either deleted (view), merged into a stated rule (value port, result, template, core operators → *instance*; `Repr` → *value*; node/branch/bundle → *carrier*; generation/extern → *module*) or promoted to its own row (*param*, *host*). The "Covers" column lists them. |

**Constructs** (B.1):

| | critic's accounting (v0 B.1 categories) | including what v0 used but did not list |
| --- | --- | --- |
| v0 | 72 | 80 (+ `gnd`, `Node`, `self` on modules, struct-update, `m`, `dtemp`, the `: Interface` head, `in` ranges) |
| v1 | **58** | **65** |

Removed (22): `interface`, `Bit` as a built-in, `Dir`, `Node`, `above`, `absdelta`, `transition`, `discontinuity`, `finish`, `held`, views, `impl`/`self` on modules, `: Interface` head, `in` range, named events in `digital`, the collision error, 8 attributes (net).
Added (7), each paid for above: `value`, `resolve` (B1); `others` (pass gate); `Nature` bound and generic `potential`/`flow` probes (M1; the words already existed as nature fields); `Fn(P)` bound (replaces the interface head); ABI strings (replace two attributes).

Stdlib modules deleted: 7 bus modules + `Pin`. Stdlib added: `Logic`/`Strength`/`Wire`/`Tri`/`SumNet`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `above`, `absdelta`, `transition`, `zi_nd` (written out), `cast`, `merge`, `Known`, `Inertial`, `MinPulse`, `State`.

## Known weak points (for the critic)

1. Ring of `Tran`s is an elaboration error (G.27).
2. `known_or` is required in `analog` even on always-known own state (G.29).
3. Unknown index merges into every selectable word (G.28): exact, possibly expensive.
4. The jump solve is specified as a limit; the reference implementation uses `h` = one tick (C.2, G.6).
5. `idt` without `ic` outside a feedback loop fails the equilibrium solve (as in VAMS).
